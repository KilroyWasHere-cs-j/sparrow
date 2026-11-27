//! FT8 receive, using the `ft8` crate for the signal processing.

use ::ft8::{Monitor, MonitorConfig};

use super::SAMPLE_RATES;

// How hard to search each slot. These are the values ft8_lib's own decoder uses. The crate's
// shortcut function only checks the 30 strongest candidates, which misses stations on a busy band.
const MAX_CANDIDATES: usize = 140;
const MIN_SYNC_SCORE: i16 = 10; // how much a candidate must look like FT8's timing pattern
const LDPC_ITERATIONS: usize = 25; // error correction passes per candidate

/// Decodes one 15 second FT8 slot and returns every message found, e.g. `CQ K1ABC FN42`.
///
/// The chunk must start on a UTC slot boundary (:00, :15, :30, :45), give or take about two
/// seconds. Each call stands alone, so nothing carries over to the next one.
///
/// # Args
/// - `samples`: mono audio from -1.0 to 1.0. Only the first 15 seconds are used; a shorter
///   chunk is searched as far as it goes, but a transmission lasts 12.64 seconds.
/// - `sample_rate`: samples per second, 8000 to 384000.
///
/// # Returns
/// - One line per distinct message, strongest candidates first. Message types the `ft8` crate
///   can't turn into text appear as a label such as `<contest>`, and callsigns sent only as a
///   hash appear as `<...>`.
/// - Empty if nothing decodes, or if the audio is empty or the sample rate is out of range.
pub fn decode_ft8(samples: &[f32], sample_rate: u32) -> Vec<String> {
    if samples.is_empty() || !SAMPLE_RATES.contains(&sample_rate) {
        return Vec::new();
    }

    let mut monitor = Monitor::new(MonitorConfig {
        sample_rate: sample_rate as f32,
        ..MonitorConfig::default()
    });

    // The monitor takes one symbol (0.16 s) at a time. It ignores a leftover piece shorter than
    // that, and anything after the slot is full.
    for symbol in samples.chunks(monitor.block_size()) {
        monitor.process(symbol);
    }

    monitor
        .decode_all(MAX_CANDIDATES, MIN_SYNC_SCORE, LDPC_ITERATIONS)
        .iter()
        .map(|decode| decode.text().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dig::test_signal::Noise;
    use ::ft8::{Message, Protocol, modulate};

    // A full 15 s slot of silence with each message mixed in at its own audio frequency,
    // starting 0.5 s in like an on-air transmission.
    fn slot(messages: &[(&str, &str, &str, f32)], sample_rate: u32) -> Vec<f32> {
        let rate = sample_rate as f32;
        let mut audio = vec![0.0; (rate * 15.0) as usize];
        let start = (rate * 0.5) as usize;

        for &(to, from, extra, tone_hz) in messages {
            let message = Message::encode_std(to, from, extra).expect("valid message");
            let signal = modulate(&message.payload, Protocol::Ft8, tone_hz, rate);
            for (out, s) in audio[start..].iter_mut().zip(signal) {
                *out += s * 0.4;
            }
        }
        audio
    }

    #[test]
    fn decodes_a_message_at_common_sample_rates() {
        for rate in [8_000, 11_025, 12_000, 22_050, 44_100, 48_000, 96_000] {
            let audio = slot(&[("CQ", "K1ABC", "FN42", 1500.0)], rate);
            assert_eq!(decode_ft8(&audio, rate), ["CQ K1ABC FN42"], "{rate} Hz");
        }
    }

    #[test]
    fn decodes_overlapping_stations_once_each() {
        let audio = slot(
            &[
                ("CQ", "K1ABC", "FN42", 800.0),
                ("K1ABC", "W9XYZ", "-11", 1700.0),
                ("W9XYZ", "K1ABC", "RR73", 2400.0),
            ],
            12_000,
        );
        let mut lines = decode_ft8(&audio, 12_000);
        lines.sort();
        assert_eq!(
            lines,
            ["CQ K1ABC FN42", "K1ABC W9XYZ -11", "W9XYZ K1ABC RR73"]
        );
    }

    #[test]
    fn decodes_through_heavy_noise() {
        let mut audio = slot(&[("CQ", "K1ABC", "FN42", 1234.0)], 12_000);
        // Noise swinging well past the signal's own 0.4 peak
        Noise::new(1).add_to(&mut audio, 0.5);
        assert_eq!(decode_ft8(&audio, 12_000), ["CQ K1ABC FN42"]);
    }

    #[test]
    fn ignores_audio_after_the_slot() {
        let mut audio = slot(&[("CQ", "K1ABC", "FN42", 1500.0)], 12_000);
        audio.extend(vec![0.0; 12_000 * 5]);
        assert_eq!(decode_ft8(&audio, 12_000), ["CQ K1ABC FN42"]);
    }

    #[test]
    fn silence_and_noise_decode_to_nothing() {
        let mut audio = vec![0.0; 12_000 * 15];
        assert!(decode_ft8(&audio, 12_000).is_empty());

        for seed in 1..=5 {
            audio.fill(0.0);
            Noise::new(seed).add_to(&mut audio, 0.5);
            assert!(decode_ft8(&audio, 12_000).is_empty(), "seed {seed}");
        }
    }

    #[test]
    fn unusable_input_decodes_to_nothing() {
        assert!(decode_ft8(&[], 12_000).is_empty());
        assert!(decode_ft8(&[0.1; 10], 12_000).is_empty()); // shorter than one symbol
        assert!(decode_ft8(&[0.1; 24_000], 12_000).is_empty()); // far shorter than a slot
        assert!(decode_ft8(&[f32::NAN; 24_000], 12_000).is_empty());
        for rate in [0, 1, 7_999, 384_001, u32::MAX] {
            assert!(decode_ft8(&[0.1; 1_000], rate).is_empty(), "{rate} Hz");
        }
    }
}
