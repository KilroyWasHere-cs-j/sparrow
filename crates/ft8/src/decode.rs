//! FT8 message decoding: audio samples -> messages. Not yet implemented.

use crate::Message;

/// Decode FT8 messages from a buffer of audio samples at the given sample rate.
pub fn decode(_samples: &[f32], _sample_rate_hz: u32) -> Vec<Message> {
    todo!("FT8 decoding not yet implemented")
}
