use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::f32::consts::PI;
use std::sync::mpsc::{self, Sender};

const SDR_RATE: u32 = 240_000; // samples per second from the dongle
const AUDIO_RATE: u32 = 48_000; // samples per second to the sound card
const DECIMATION: usize = (SDR_RATE / AUDIO_RATE) as usize; // 5 radio samples per audio sample

/// Turns chunks of radio samples into sound. Make one with `new()`, then call `play` on every chunk.
pub struct Player {
    audio_tx: Sender<f32>,
    prev_i: f32,
    prev_q: f32,
    acc: f32,
    count: usize,
    volume: f32,
    // Must stay alive for the sound to keep playing; dropping it stops the audio.
    _stream: cpal::Stream,
}

impl Player {
    pub fn new(volume: f32) -> Result<Self, String> {
        // Demodulated samples travel to the sound card's callback through this channel
        let (audio_tx, audio_rx) = mpsc::channel::<f32>();

        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no audio output device found")?;
        let mut config: cpal::StreamConfig = device
            .default_output_config()
            .map_err(|e| e.to_string())?
            .into();
        config.sample_rate = AUDIO_RATE;
        let channels = config.channels as usize;

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _| {
                    // One frame = one sample per channel. Play silence if we're out of audio.
                    for frame in data.chunks_mut(channels) {
                        frame.fill(audio_rx.try_recv().unwrap_or(0.0));
                    }
                },
                |err| eprintln!("audio stream error: {err}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;

        Ok(Self {
            audio_tx,
            prev_i: 0.0,
            prev_q: 0.0,
            acc: 0.0,
            count: 0,
            volume,
            _stream: stream,
        })
    }

    /// Changes the loudness while playing (0.0 silent to 1.0 full scale).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Demodulates one chunk of narrowband FM (interleaved I, Q bytes) and queues it to play.
    pub fn play(&mut self, chunk: &[u8]) {
        for iq in chunk.chunks_exact(2) {
            // Bytes are 0-255 centered on 127.5; turn them into -1.0..1.0
            let i = (iq[0] as f32 - 127.5) / 127.5;
            let q = (iq[1] as f32 - 127.5) / 127.5;

            // FM: the audio is how far the signal's angle turned since the previous sample
            let re = i * self.prev_i + q * self.prev_q;
            let im = q * self.prev_i - i * self.prev_q;
            self.acc += im.atan2(re) / PI;
            self.prev_i = i;
            self.prev_q = q;

            // Average every 5 radio samples down to one 48 kHz audio sample
            self.count += 1;
            if self.count == DECIMATION {
                let sample = self.acc / DECIMATION as f32 * 20.0 * self.volume; // 20.0: NFM is quiet
                let _ = self.audio_tx.send(sample.clamp(-1.0, 1.0));
                self.acc = 0.0;
                self.count = 0;
            }
        }
    }
}

pub fn detect_speech() {}

pub fn save_sample() {}

pub fn record_sample() {}

// ---- Saving to WAV (reference, not compiled yet) ----
// Uses the `hound` crate. Save demodulated audio at 48_000 Hz with save_audio_wav, or a raw
// chunk from the dongle at 240_000 Hz with save_iq_wav. The rate passed in must match the data,
// or the file plays at the wrong speed. One chunk is only ~68 ms, so collect samples in a Vec
// and write once instead of writing a file per chunk.
//
// use hound::{SampleFormat, WavSpec, WavWriter};
//
// /// Writes demodulated audio (-1.0..1.0, mono) to a 16-bit WAV file. Use this to listen to it.
// fn save_audio_wav(path: &str, samples: &[f32], sample_rate: u32) -> Result<(), hound::Error> {
//     let spec = WavSpec {
//         channels: 1,
//         sample_rate,
//         bits_per_sample: 16,
//         sample_format: SampleFormat::Int,
//     };
//     let mut writer = WavWriter::create(path, spec)?;
//
//     for &sample in samples {
//         // f32 -1.0..1.0 -> i16 -32767..32767 (clamped so a loud peak can't wrap around)
//         let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
//         writer.write_sample(value)?;
//     }
//
//     // Writes the file header's final length; without this the file can be unreadable
//     writer.finalize()
// }
//
// /// Writes a raw radio chunk (interleaved I, Q bytes) to a 2-channel 16-bit WAV file.
// /// Left channel = I, right channel = Q. SDR programs can open this as an I/Q recording.
// fn save_iq_wav(path: &str, chunk: &[u8], sample_rate: u32) -> Result<(), hound::Error> {
//     let spec = WavSpec {
//         channels: 2,
//         sample_rate,
//         bits_per_sample: 16,
//         sample_format: SampleFormat::Int,
//     };
//     let mut writer = WavWriter::create(path, spec)?;
//
//     for &byte in chunk {
//         // 0..255 centered on 127.5 -> roughly -32767..32767
//         let value = ((byte as f32 - 127.5) / 127.5 * i16::MAX as f32) as i16;
//         writer.write_sample(value)?;
//     }
//     writer.finalize()
// }
