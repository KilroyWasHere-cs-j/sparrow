//! FT8 message encoding: text -> audio samples. Not yet implemented.

use crate::Message;

/// Encode a message into FT8 baseband audio samples at the given sample rate.
pub fn encode(_message: &Message, _sample_rate_hz: u32) -> Vec<f32> {
    todo!("FT8 encoding not yet implemented")
}
