use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::error::Error;
use std::sync::mpsc;
use std::thread;

// SDR runs at 240 kHz; averaging every 5 samples gives 48 kHz audio.
const SDR_SAMPLE_RATE: u32 = 240_000;
const AUDIO_SAMPLE_RATE: u32 = 48_000;
const DECIMATION: usize = (SDR_SAMPLE_RATE / AUDIO_SAMPLE_RATE) as usize;

pub struct RtlParams {
    device_usb_index: u32,
    center_frequency: u32,
    use_acg: bool,
    cystal_ppm: i32,
}

impl RtlParams {
    /// Builds the settings for one dongle.
    /// - `device_usb_index`: which dongle to open (0 = first one found)
    /// - `center_frequency`: tune-to frequency in Hz (100 MHz = 100_000_000)
    /// - `use_acg`: turn on the dongle's automatic gain control
    /// - `cystal_ppm`: crystal error correction in parts per million (0 if unknown)
    pub fn new(
        device_usb_index: u32,
        center_frequency: u32,
        use_acg: bool,
        cystal_ppm: i32,
    ) -> Self {
        Self {
            device_usb_index,
            center_frequency,
            use_acg,
            cystal_ppm,
        }
    }
}

pub fn open_and_stream(params: RtlParams) -> Result<(), Box<dyn Error>> {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    // Open the first RTL-SDR dongle (index 0). Returns a (Controller, Reader) pair —
    // Controller changes settings, Reader pulls samples, and they can live on separate threads.
    let (mut ctl, mut reader) = rtlsdr_mt::open(params.device_usb_index).unwrap();

    if params.use_acg {
        ctl.enable_agc().unwrap();
    }
    ctl.set_ppm(params.cystal_ppm).unwrap();
    ctl.set_center_freq(params.center_frequency).unwrap(); // in Hz
    ctl.set_sample_rate(SDR_SAMPLE_RATE).unwrap();

    // Audio side: demodulated samples go through this channel to the sound card callback.
    let (audio_tx, audio_rx) = mpsc::channel::<f32>();

    // Open the default output device (your headphones/speakers) at 48 kHz, f32 samples.
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no audio output device found")?;
    let mut config: cpal::StreamConfig = device.default_output_config()?.into();
    config.sample_rate = AUDIO_SAMPLE_RATE;
    let channels = config.channels as usize;

    let stream = device.build_output_stream(
        config,
        move |data: &mut [f32], _| {
            // One frame = one sample per channel. Copy the mono sample to every channel,
            // and play silence if the SDR hasn't produced enough audio yet.
            for frame in data.chunks_mut(channels) {
                let sample = audio_rx.try_recv().unwrap_or(0.0);
                frame.fill(sample);
            }
        },
        |err| eprintln!("audio stream error: {err}"),
        None,
    )?;
    stream.play()?;

    let capture_thread = thread::spawn(move || {
        reader
            .read_async(4, 32768, |bytes| {
                // bytes is a borrowed slice reused by librtlsdr on every call —
                // must copy it out before it can cross a thread boundary.
                if tx.send(bytes.to_vec()).is_err() {
                    // caller dropped rx — nothing to do, but read_async has no way
                    // to stop itself from inside the closure, so this buffer is lost
                    // and we just wait for cancel_async_read() to unblock the call.
                }
            })
            .unwrap();
    });

    // FM demodulator state, kept across buffers so no sample pair is lost at the edges.
    let (mut prev_i, mut prev_q) = (0.0f32, 0.0f32);
    // Running total for the decimation average.
    let mut acc = 0.0f32;
    let mut count = 0usize;

    // Caller side: consume buffers as they arrive (runs until the capture thread stops).
    for buf in rx.iter() {
        // Samples arrive as interleaved unsigned bytes: I, Q, I, Q, ...
        // Center them around 0 and scale to roughly -1.0..1.0.
        for iq in buf.chunks_exact(2) {
            let i = (iq[0] as f32 - 127.5) / 127.5;
            let q = (iq[1] as f32 - 127.5) / 127.5;

            // FM demod: the angle between this sample and the previous one is the
            // instantaneous frequency, i.e. the audio. Computed as the phase of
            // current * conjugate(previous).
            let re = i * prev_i + q * prev_q;
            let im = q * prev_i - i * prev_q;
            let audio = im.atan2(re) / std::f32::consts::PI;
            prev_i = i;
            prev_q = q;

            // Average every DECIMATION samples down to the audio rate (crude low-pass).
            acc += audio;
            count += 1;
            if count == DECIMATION {
                // Fixed gain bump; wideband FM swings only about +/-0.6 at this rate.
                let _ = audio_tx.send((acc / DECIMATION as f32) * 1.5);
                acc = 0.0;
                count = 0;
            }
        }
    }

    // Stop the stream from this thread, then wait for the capture thread to exit.
    ctl.cancel_async_read();
    capture_thread.join().unwrap();
    Ok(())
}
