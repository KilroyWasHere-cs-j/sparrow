//! Digital mode decoders: a chunk of audio goes in, text comes out.

mod aprs;
mod ft8;

pub use self::aprs::decode_aprs;
pub use self::ft8::decode_ft8;

use std::ops::RangeInclusive;

// Sample rates the decoders accept. Lower can't carry the tones well; higher is surely a mistake
// and would make the decoders allocate very large buffers.
const SAMPLE_RATES: RangeInclusive<u32> = 8_000..=384_000;

// Helpers for building test audio.
#[cfg(test)]
mod test_signal {
    /// A tiny repeatable random number generator, so noisy tests give the same result every run.
    pub struct Noise(u64);

    impl Noise {
        pub fn new(seed: u64) -> Self {
            // The generator gets stuck at zero, so mix the seed into a non-zero constant
            Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }

        /// The next value, evenly spread over -1.0..1.0 (xorshift).
        pub fn sample(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 40) as f32 / (1u32 << 23) as f32 - 1.0
        }

        /// Adds noise reaching up to `peak` on top of `audio`.
        pub fn add_to(&mut self, audio: &mut [f32], peak: f32) {
            for s in audio {
                *s += self.sample() * peak;
            }
        }
    }
}
