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
