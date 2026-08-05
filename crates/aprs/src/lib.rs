//! APRS (Automatic Packet Reporting System) library: encode/decode of AX.25-framed
//! APRS packets used in amateur radio for position, status, and messaging traffic.
//! Framework only — no protocol logic implemented yet.

pub mod decode;
pub mod encode;

/// A single decoded APRS packet: source/destination callsigns, digipeater path, and payload.
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub source: String,
    pub destination: String,
    pub path: Vec<String>,
    pub information: String,
}
