//! APRS packet decoding: AX.25 frame bytes -> Packet. Not yet implemented.

use crate::Packet;

/// Decode an AX.25 UI frame into an APRS packet.
pub fn decode(_frame: &[u8]) -> Option<Packet> {
    todo!("APRS/AX.25 decoding not yet implemented")
}
