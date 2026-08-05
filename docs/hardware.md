# Radio interface hardware

## Digirig

Decided 2026-08-05 as the target radio interface. Reasoning: reliable, widely
used in the ham community for USB digital-mode interfacing, "just works."

### What it actually is

A USB sound card plus a PTT keying interface (CM108 HID GPIO, and/or CAT via
a serial port depending on model and cable). It does not decode or encode any
protocol. No AFSK demod, no AX.25 framing, no KISS. It is a dumb pipe between
the radio's audio/PTT lines and the computer's USB port.

### What that means for sparrow

All protocol work happens in software, on both the FT8 and APRS paths:

- FT8: `ibelinp/ft8` already expects raw audio samples in and produces raw
  audio samples out. This maps directly onto reading/writing the Digirig's
  sound card device.
- APRS: two options, see `docs/decisions.md` for the open decision.
  1. Sparrow owns the AFSK modem directly, same shape as the FT8 path: read
     raw audio off the Digirig, demodulate Bell 202 tones in-process, hand
     bytes to `libaprs-engine`'s AX.25/APRS decode. Transmit is the reverse.
     Lighter DSP than FT8 (a PLL and tone discrimination, no FFT or LDPC), but
     it is still DSP we'd own.
  2. Sparrow defers to Direwolf (the standard open-source software TNC),
     pointed at the Digirig's audio device. Direwolf does the AFSK modem work
     and exposes a KISS endpoint over TCP or serial. Sparrow then only ever
     speaks KISS, which `aprs-transport-kiss` already covers both directions.
     Sidesteps writing an AFSK modem, at the cost of requiring Direwolf
     installed and running as an external process.

PTT keying (CM108 GPIO or serial RTS/DTR) is a separate concern from the audio
path and needs its own handling regardless of which APRS option is chosen.
