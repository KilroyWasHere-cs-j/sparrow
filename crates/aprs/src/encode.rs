//! APRS packet encoding: Packet -> AX.25 frame bytes. Not yet implemented.

use crate::Packet;

/// Encode a packet into an AX.25 UI frame ready for transmission.
pub fn encode(_packet: &Packet) -> Vec<u8> {
    todo!("APRS/AX.25 encoding not yet implemented")
}
