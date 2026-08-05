# APRS: crate research

Status as of 2026-08-05: open, no crate chosen yet.

## Summary

Unlike FT8, no single Rust crate matches the completeness or track record of
`ibelinp/ft8`. The APRS ecosystem on crates.io is fragmented across parse-only,
encode-only, and internet-only (APRS-IS) crates. Closest fit found so far is
`libaprs-engine`, but it has a real gap and a short track record. Details below.

## `libaprs-engine`

https://github.com/SoloSentryOrg/libaprs-engine, MIT, current stable v2.6.0
(v3.0.0-rc.2 in prerelease). First published to crates.io April 2026, so about
four months old at time of writing, modest download counts. Nowhere near ft8's
field-testing.

Structured as a workspace: a core parser/encoder crate (`libaprs-engine`) plus
separate optional transport crates for KISS, AX.25, APRS-IS, TCP, UDP, serial,
MQTT, HTTP, file, and a few others.

### What was actually verified (by reading source, not just the README)

- `crates/libaprs-engine/src/encoder.rs`: real semantic packet construction,
  not just round-tripping already-parsed packets. Confirmed functions:
  `encode_status`, `encode_uncompressed_position`, `encode_message` (also
  covers ack/reject/bulletin/announcement via the same path), `encode_telemetry`,
  `encode_telemetry_metadata`, `encode_object`, `encode_item`. Each validates
  its fields (callsign shape, lat/lon format, message ID length, etc.) before
  emitting bytes.
- `crates/aprs-transport-kiss/src/lib.rs`: has both `encode_data_frame` and
  `decode_frames`/`decode_frames_with_limit`. Genuinely bi-directional at the
  KISS framing layer (this is the layer that talks to a TNC or software modem
  like Direwolf).
- `crates/aprs-transport-ax25/src/lib.rs`: only `decode_ax25_ui_frame` /
  `decode_ax25_ui_frame_with_limit`. **No AX.25 UI frame encoder.**

### The gap

The real wire path for transmitting over RF is: APRS payload, wrapped in an
AX.25 UI frame (7-byte shifted-ASCII callsigns with SSID and command bits, a
control byte, a PID byte), wrapped in a KISS frame, sent to the TNC. The
semantic-payload layer and the KISS layer are covered both directions. The
AX.25 addressing layer in the middle only decodes.

Practically: this crate can fully receive real RF/KISS traffic today. To
transmit, something has to build the AX.25 UI frame. That's a small, mechanical,
well-specified piece of code (not open-ended like FT8's DSP), most likely
worth writing ourselves rather than waiting on upstream.

### Other gaps

The parser supports compressed position format and Mic-E; the encoder does
not. Only uncompressed position ('!'/'=' data type identifier) can be built
right now, no compressed format, no Mic-E, no timestamped position variant
('@'/'/' identifiers).

## Alternatives considered, not chosen

- `aprs-parser` (Turbo87, https://github.com/Turbo87/aprs-parser-rs). Decode
  only, text/APRS-IS format. Established, well-known author, but doesn't cover
  encode or the RF/AX.25 path.
- `aprs-encode`. Encode only, no linked repository on crates.io, last updated
  2021. Couldn't verify quality without pulling the source package directly,
  no matching decode counterpart.
- `simple-aprs` (https://github.com/welshdave/simple-aprs). Genuinely
  bi-directional, but scoped to APRS-IS (internet gateway) only. No RF,
  no AX.25, no KISS. Would be the right pick if sparrow only needs to talk to
  APRS-IS and never touches a radio directly for APRS.

## Open question this feeds into

See `docs/decisions.md`: whether sparrow owns the AFSK modem directly (reading
raw audio off the Digirig, same shape as FT8) or defers to Direwolf as an
external KISS-speaking TNC. That decides whether `aprs-transport-kiss` is used
at all, on top of the AX.25 encoder gap that needs closing either way.
