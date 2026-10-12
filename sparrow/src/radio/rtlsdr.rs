use rtlsdr_mt::Controller;
use std::sync::mpsc::{self, Receiver, TrySendError};
use std::thread::{self, JoinHandle};

/// A running RTL-SDR stream. Read `rx` for chunks of raw samples (interleaved I, Q bytes).
/// Dropping this stops the stream and waits for the reader thread to finish.
pub struct RtlStream {
    // Not read yet; the code that turns samples into audio will consume it.
    #[allow(dead_code)]
    pub rx: Receiver<Vec<u8>>,
    ctl: Controller,
    thread: Option<JoinHandle<()>>,
}

impl RtlStream {
    /// Opens the dongle and starts streaming raw samples to `rx` on a background thread.
    /// The config must pass `validate()`; if it doesn't, the dongle is never touched.
    ///
    /// # Args
    /// - `radio_config`: every setting in one value, so the caller can't pass a frequency
    ///   or sample rate that disagrees with the rest. These fields reach the dongle:
    ///   - `device_index`: which dongle to open. 0 (the first one found) is the default;
    ///     it only matters when several dongles are plugged in.
    ///   - `gain`: `Auto` turns on the dongle's automatic gain, `Manual(n)` fixes it at
    ///     `n` tenths of a dB. Only one is applied, since each switches the other off.
    ///   - `ppm`: corrects the dongle's crystal error so the tuned frequency is accurate.
    ///   - `sample_rate`: how many samples per second arrive on `rx`.
    ///   - `frequency_hz`: the frequency to tune to.
    ///
    ///   `mode`, `volume` and `squelch` are ignored. They shape the audio after the
    ///   samples arrive, not the dongle itself.
    ///
    /// # Returns
    /// - A running `Stream`. Raw samples (interleaved I, Q bytes) arrive on `rx`. If the
    ///   reader falls behind, new chunks are dropped instead of piling up in memory (the
    ///   channel holds 64). Dropping the `Stream` stops the read and waits for the thread
    ///   to finish.
    ///
    /// # Errors
    /// - The config fails `validate()`. The message says which field is bad.
    /// - The dongle can't be opened: not plugged in, or another program is using it.
    /// - The dongle rejects a setting (gain, ppm, sample rate or frequency). The message
    ///   names the setting and its value.
    pub fn start(radio_config: RadioConfig) -> Result<Self, String> {
        radio_config.validate()?;

        // Controller changes settings; Reader pulls samples. They can live on different threads.
        let (mut ctl, mut reader) = rtlsdr_mt::open(radio_config.device_index).map_err(|_| {
            format!(
                "could not open RTL-SDR #{} (plugged in? used by another program?)",
                radio_config.device_index
            )
        })?;

        apply_gain(&mut ctl, radio_config.gain)?;

        ctl.set_ppm(radio_config.ppm)
            .map_err(|_| format!("could not set ppm {}", radio_config.ppm))?;
        ctl.set_sample_rate(radio_config.sample_rate)
            .map_err(|_| format!("could not set sample rate {}", radio_config.sample_rate))?;
        ctl.set_center_freq(radio_config.frequency_hz)
            .map_err(|_| format!("could not tune to {} Hz", radio_config.frequency_hz))?;

        // Bounded channel (64 chunks): if main falls behind, memory can't grow without limit.
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(64);

        let thread = thread::spawn(move || {
            // Blocks here until ctl.cancel_async_read() is called (or the dongle fails).
            let _ = reader.read_async(4, 32768, |bytes| {
                // `bytes` is a buffer librtlsdr reuses on the next callback, so copy it out.
                match tx.try_send(bytes.to_vec()) {
                    Ok(()) => {}
                    // Main is slow: drop this chunk instead of blocking the dongle's callback.
                    Err(TrySendError::Full(_)) => {}
                    // Main has hung up; nothing to do, Stream::drop will stop the read.
                    Err(TrySendError::Disconnected(_)) => {}
                }
            });
        });

        Ok(Self {
            rx,
            ctl,
            thread: Some(thread),
        })
    }
}

impl RtlStream {
    /// Retunes while streaming. Chunks already queued came from the old frequency, so they
    /// are dropped.
    pub fn set_frequency(&mut self, hz: u32) -> Result<(), String> {
        retry_once(|| self.ctl.set_center_freq(hz))
            .map_err(|_| format!("could not tune to {hz} Hz"))?;
        let _ = self.ctl.reset_buffer();
        while self.rx.try_recv().is_ok() {}
        Ok(())
    }

    /// Changes the gain while streaming (see `Gain`).
    pub fn set_gain(&mut self, gain: Gain) -> Result<(), String> {
        apply_gain(&mut self.ctl, gain)
    }

    /// Changes the crystal error correction while streaming.
    pub fn set_ppm(&mut self, ppm: i32) -> Result<(), String> {
        retry_once(|| self.ctl.set_ppm(ppm)).map_err(|_| format!("could not set ppm {ppm}"))
    }
}

/// Applies a gain setting. Manual gain and automatic gain (AGC) exclude each other in the
/// library: setting one switches the other off. So exactly one of them is called.
fn apply_gain(ctl: &mut Controller, gain: Gain) -> Result<(), String> {
    match gain {
        Gain::Auto => retry_once(|| ctl.enable_agc()).map_err(|_| "could not enable AGC".to_string()),
        Gain::Manual(tenths_db) => retry_once(|| ctl.set_tuner_gain(tenths_db))
            .map_err(|_| format!("could not set tuner gain {tenths_db}")),
    }
}

/// Runs a dongle command, and once more if it fails.
///
/// On the test machine the first command sent after streaming starts fails with a USB "pipe
/// stalled" error (-9) every time, and the same command works when repeated. Whatever the command
/// is (tune, gain, ppm), only the first one is affected. The cause is not known; this is a
/// workaround. A command that is really invalid fails both times and still reports the error.
fn retry_once<T, E>(mut command: impl FnMut() -> Result<T, E>) -> Result<T, E> {
    match command() {
        Ok(value) => Ok(value),
        Err(_) => command(),
    }
}

impl Drop for RtlStream {
    fn drop(&mut self) {
        // Makes read_async return, which ends the thread.
        self.ctl.cancel_async_read();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// How the radio signal is turned into sound.
// Not every mode is constructed yet; the radio code that uses them is still to come.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Wfm, // wideband FM: broadcast radio
    Nfm, // narrowband FM: two-way radio, APRS
    Am,  // airband
    Usb, // upper sideband: ham voice above 10 MHz
    Lsb, // lower sideband: ham voice below 10 MHz
    Cw,  // Morse
}

/// Tuner gain: let the dongle decide, or fix it by hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gain {
    Auto,
    /// Manual gain in tenths of a dB (300 = 30.0 dB). The tuner snaps to the nearest step it has.
    Manual(i32),
}

/// Every setting the radio can have. One value of this describes the whole radio.
#[derive(Clone, Debug, PartialEq)]
pub struct RadioConfig {
    /// Which dongle to open (0 = first one found).
    pub device_index: u32,
    /// Tune-to frequency in Hz (100 MHz = 100_000_000).
    pub frequency_hz: u32,
    /// How many samples per second the dongle sends. 240 kHz is a safe value.
    pub sample_rate: u32,
    pub mode: Mode,
    pub gain: Gain,
    /// Crystal error correction in parts per million (0 if unknown).
    pub ppm: i32,
    /// Output loudness, 0.0 (silent) to 1.0 (full scale).
    pub volume: f32,
    /// Mute audio when the signal is quieter than this (0.0 = never mute).
    pub squelch: f32,
}

impl Default for RadioConfig {
    fn default() -> Self {
        Self {
            device_index: 0,
            frequency_hz: 100_000_000,
            sample_rate: 240_000,
            mode: Mode::Wfm,
            gain: Gain::Auto,
            ppm: 0,
            volume: 0.2,
            squelch: 0.0,
        }
    }
}

impl RadioConfig {
    /// Checks the values are usable. Returns a readable message for the first bad one.
    pub fn validate(&self) -> Result<(), String> {
        // The usual RTL-SDR (R820T) tuner range
        if !(24_000_000..=1_700_000_000).contains(&self.frequency_hz) {
            return Err(format!(
                "{} Hz is outside the 24-1700 MHz tuning range",
                self.frequency_hz
            ));
        }
        if !(225_001..=3_200_000).contains(&self.sample_rate) {
            return Err(format!(
                "sample rate {} is outside 225001-3200000",
                self.sample_rate
            ));
        }
        if !(0.0..=1.0).contains(&self.volume) {
            return Err(format!(
                "volume {} must be between 0.0 and 1.0",
                self.volume
            ));
        }
        if !(0.0..=1.0).contains(&self.squelch) {
            return Err(format!(
                "squelch {} must be between 0.0 and 1.0",
                self.squelch
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These run without a dongle: a bad config must be rejected before the dongle is opened.
    #[test]
    fn start_rejects_a_bad_config_before_opening_the_dongle() {
        let config = RadioConfig {
            frequency_hz: 0,
            ..Default::default()
        };
        match RtlStream::start(config) {
            Err(msg) => assert!(msg.contains("tuning range"), "{msg}"),
            Ok(_) => panic!("a 0 Hz config should not start"),
        }
    }

    // Every kind of bad value must be rejected up front, with a message naming the problem.
    // None of these reach the dongle.
    #[test]
    fn start_rejects_each_kind_of_bad_config() {
        let cases = [
            (
                RadioConfig {
                    frequency_hz: 1_700_000_001,
                    ..Default::default()
                },
                "tuning range",
            ),
            (
                RadioConfig {
                    sample_rate: 100,
                    ..Default::default()
                },
                "sample rate",
            ),
            (
                RadioConfig {
                    volume: 1.5,
                    ..Default::default()
                },
                "volume",
            ),
            (
                RadioConfig {
                    squelch: -0.5,
                    ..Default::default()
                },
                "squelch",
            ),
        ];

        for (config, expected) in cases {
            match RtlStream::start(config.clone()) {
                Err(msg) => assert!(msg.contains(expected), "{config:?}: {msg}"),
                Ok(_) => panic!("{config:?} should not start"),
            }
        }
    }

    #[test]
    fn start_reports_the_same_error_as_validate() {
        let config = RadioConfig {
            volume: 2.0,
            ..Default::default()
        };
        let expected = config.validate().unwrap_err();
        match RtlStream::start(config) {
            Err(msg) => assert_eq!(msg, expected),
            Ok(_) => panic!("should not start"),
        }
    }

    #[test]
    fn default_config_uses_the_first_device() {
        assert_eq!(RadioConfig::default().device_index, 0);
    }
}
