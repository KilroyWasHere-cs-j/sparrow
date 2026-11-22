# sparrow

A desktop app for field and off-grid communications over amateur radio. It is built with Rust and Tauri, with a Svelte frontend. The goal is one app that can listen to the airwaves with an RTL-SDR dongle and, later, send and receive digital modes (FT8/FT4, APRS, fldigi modes) through a radio.

> **Status: early prototype.** Only RTL-SDR receive and audio playback work today. The digital modes are researched and planned, not implemented. See [Roadmap](#roadmap).

## What works today

- Tune an RTL-SDR dongle and play the demodulated audio through the default audio device.
- Demodulation modes: WFM, NFM, AM, USB, LSB, CW.
- Live controls while the radio runs: frequency, mode, gain (auto or manual), PPM correction, volume and squelch. Device index and sample rate need a restart.
- Start and stop the radio from the UI. It is also stopped cleanly on window close, Ctrl+C and app exit, so the dongle is released.

The `FT8` and `APRS` pages in the UI are placeholders.

## Hardware

| Hardware | Used for | Status |
|---|---|---|
| RTL-SDR dongle (RTL2832U) | Receive | Supported |
| [Digirig](docs/hardware.md) | Audio and PTT interface to a transceiver (FT8, APRS, fldigi) | Planned |

The Digirig is a USB sound card plus PTT keying. It does no protocol work, so all encoding and decoding happens in sparrow. Details are in [docs/hardware.md](docs/hardware.md).

## Requirements

- [Rust](https://rustup.rs) (edition 2024, so a recent stable toolchain)
- [Node.js](https://nodejs.org) and npm
- An RTL-SDR dongle and its driver
- Tauri's [platform prerequisites](https://tauri.app/start/prerequisites/)

**Linux**
- WebKitGTK and the other Tauri system packages
- `librtlsdr` and ALSA development headers (`libasound2-dev` on Debian/Ubuntu)
- If the dongle won't open, the kernel's DVB driver may be holding it. Blacklist `dvb_usb_rtl28xxu`.

**Windows**
- WebView2 (included in Windows 11) and the MSVC build tools
- The RTL-SDR driver installed with [Zadig](https://zadig.akeo.ie/) (WinUSB), plus the `rtlsdr` DLLs available at build and run time

## Build and run

Install frontend dependencies once:

```sh
cd frontend && npm install
```

Run in development mode with hot reload, from the repo root:

```sh
./frontend/node_modules/.bin/tauri dev
```

Linux build helpers (they assume a Linux machine):

| Script | What it does |
|---|---|
| `scripts/build.sh` | Builds a release binary without bundling, then runs it |
| `bundle.sh` | Builds and packages `.deb` and `.rpm` installers (AppImage is skipped because it needs FUSE2) |
| `deploy.sh` | Bundles, then launches the release binary |

The scripts set `WEBKIT_DISABLE_DMABUF_RENDERER=1` to work around a WebKitGTK/Wayland crash on the author's machine. Remove it if you don't need it.

Start with the volume low, and pick a strong local station when testing.

## Repo layout

```
sparrow/      sparrow-core: the radio library (RTL-SDR capture, demodulation, audio output)
src-tauri/    Tauri app: exposes start_radio / update_radio / stop_radio to the UI
frontend/     SvelteKit UI (radio controls, plus FT8, APRS and settings pages)
docs/         Decisions log, hardware notes, per-protocol research
scripts/      Build helpers
```

Data flow for the radio:

```
dongle -> capture thread -> demodulator -> audio device
```

How the FM path works is explained in [docs/rtlsdr-fm-playback.md](docs/rtlsdr-fm-playback.md). That doc was written before the move to `sparrow-core`, so its file paths are out of date.

## Roadmap

| Item | State |
|---|---|
| RTL-SDR receive, demodulation, live controls | Done |
| FT8/FT4 using [`ibelinp/ft8`](https://github.com/ibelinp/ft8) | Decided, in [research](docs/research/ft8.md), not wired up |
| APRS (AFSK modem vs. Direwolf over KISS) | Open decision, see [decisions](docs/decisions.md) and [research](docs/research/aprs.md) |
| fldigi integration over XML-RPC | Researched, see [docs/research/fldigi.md](docs/research/fldigi.md) |
| Digirig audio and PTT (CM108 GPIO or serial) | Planned |

## Docs

- [docs/decisions.md](docs/decisions.md): chronological log of architecture and dependency decisions
- [docs/hardware.md](docs/hardware.md): radio interface hardware
- [docs/research/](docs/research): per-protocol research (FT8, APRS, fldigi)

## Contributing

This is a personal project in early development. Issues and ideas are welcome. Check [docs/decisions.md](docs/decisions.md) before proposing changes to the architecture.

## License

MIT, as declared in `frontend/package.json`. There is no `LICENSE` file in the repo yet.
