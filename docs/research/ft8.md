# FT8/FT4: crate research

## What we're using

`ibelinp/ft8`, https://github.com/ibelinp/ft8, added as a git dependency (not published
to crates.io as of 2026-08-05).

## What it is

A pure-Rust port of `ft8_lib` (the MIT-licensed C reference implementation, by Kārlis
Goba), covering both FT8 and FT4.

Receive chain: Hann-windowed FFT and spectrogram (6.25 Hz bins, one block per 0.16s
symbol), Costas correlation sync search across time and frequency, soft-bit extraction,
LDPC(174,91) decode, CRC-14, message unpacking (standard type 1/2 messages, free text,
telemetry).

Transmit chain: message packing, CRC, LDPC, Costas insertion, GFSK-shaped modulation
(not naive on/off FSK, the shaping keeps energy from splattering into adjacent channels).

One dependency (`rustfft`), no C toolchain, runs on `wasm32`.

## Why this one

Per the author's own README: every other FT8 decoder that runs in a browser is either
GPL (WSJT-X-derived, or `ft8ts`) or has no license at all, which rules it out for
anything that can't be open-sourced. `ibelinp/ft8` is a permissively-licensed port
specifically to close that gap. That matches sparrow's situation: Tauri app, want a
dependency we can actually redistribute.

The author reports testing against real 20m traffic: 14-17 stations decoded per slot.
Tests synthesize real waveforms (phase-continuous 8-FSK) and check round-trip decode,
including two overlapping stations, noise at 3x signal amplitude, and recovery from six
flipped bits in the LDPC tables (a transposed or off-by-one LDPC matrix still decodes
clean signals, it only shows up as degraded weak-signal performance, so that's the test
that actually catches it).

## Known gaps

- Contest and nonstandard-callsign message types are identified but rendered as
  `<contest>` rather than decoded to text.
- No callsign hash table, hashed callsigns show as `<...>` (same behavior WSJT-X has
  before it's heard the full callsign).
- FT4 verified end to end against synthesized waveforms, far less exercised than FT8,
  no live-signal testing reported.
- SNR estimate not calibrated against a reference receiver. Spread and ordering are
  reported as correct, treat the absolute dB figure as good to a few dB only.

## License

MIT. Ships `LICENSE-ft8_lib` (Kārlis Goba, also MIT) alongside its own license, that
file needs to travel with any copy per the upstream terms. The LDPC matrices originate
in WSJT-X source but are treated as protocol parameters published in the FT8 spec, not
copied WSJT-X code, so no GPL obligation attaches per the author's note.
