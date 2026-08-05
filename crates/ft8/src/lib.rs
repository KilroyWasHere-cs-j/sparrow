//! FT8 protocol library: encode/decode of the FT8 digital mode used in amateur radio.
//! Framework only — no protocol logic implemented yet.

pub mod decode;
pub mod encode;

/// A single decoded FT8 message: raw text plus the metrics a decoder would report.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub text: String,
    pub snr_db: f32,
    pub freq_hz: f32,
}
