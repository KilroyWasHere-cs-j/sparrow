# Decisions

Chronological log of architecture and dependency decisions for sparrow.
Each entry states what was decided, why, and current status if still open.
Detailed research behind an entry lives in `docs/research/` or `docs/hardware.md`;
this file stays short and links out.

## 2026-08-05: Project scaffold

- Stack: Tauri (Rust backend, Svelte frontend), single Cargo workspace at the repo root.
- Why: Tauri pairs naturally with Rust protocol/DSP crates, one language across app and libs.
- Layout: `src-tauri/` is the Tauri app (package `sparrow`), `crates/<name>/` holds protocol
  lib crates, root `Cargo.toml` is a virtual workspace manifest with no package of its own.

## 2026-08-05: FT8/FT4 via `ibelinp/ft8`

- Decided: depend on `ibelinp/ft8` (git dependency, not on crates.io) instead of writing
  FT8 encode/decode from scratch.
- Why: see `docs/research/ft8.md`. Short version: complete, tested, MIT, one dependency.
- Status: done. Wired into `src-tauri/Cargo.toml` as `ft8 = { git = "https://github.com/ibelinp/ft8" }`.

## 2026-08-05: APRS crate, still open

- Status: open. No single crate matches ft8's completeness or track record.
- Leaning toward `libaprs-engine` for the parser/encoder and its `aprs-transport-kiss` /
  `aprs-transport-ax25` crates for framing. See `docs/research/aprs.md` for what was
  actually verified by reading the source, and what's missing.
- Blocked on: the modem-ownership question below.

## 2026-08-05: Radio interface is a Digirig

- Decided: Digirig is the target hardware for connecting to a radio.
- Why: reliable, widely used in the ham community, "just works."
- Implication: Digirig is a dumb USB audio + PTT pipe. It does no protocol work itself.
  See `docs/hardware.md`.
- Open question this raises: does sparrow own the AFSK modem for APRS directly, reading
  raw audio off the Digirig itself (same shape as the FT8 path), or does it defer to
  Direwolf as an external software TNC and only speak KISS over the network/serial?
  This decides whether `aprs-transport-kiss` gets used at all, and whether we need to
  write our own AFSK modem in addition to the AX.25 encoder gap already identified.

## 2026-11-26: APRS receive uses our own modem

- Decided: `sparrow-core` demodulates the 1200 baud audio tones, unpacks the AX.25 frames and
  writes them as text itself. No Direwolf, and no APRS crate.
- Why: the goal is a function that takes audio and returns text, with no outside program to
  install or run. Receive needs no new dependencies.
- Status: done for receive (`sparrow/src/dig/aprs.rs`). Payload fields (position, messages) are
  not parsed. Transmit, and whether to use `libaprs-engine` for payload parsing, are still open.

## 2026-11-26: Decoders live in `sparrow-core`

- Decided: `decode_aprs` and `decode_ft8` are stateless library functions: one chunk of audio in,
  lines of text out.
- FT8 wraps `ibelinp/ft8`, now also a dependency of `sparrow-core`, and searches 140 candidates
  per slot instead of the crate shortcut's 30.
- Status: done. Not yet connected to the live radio or the UI.
