# RTL-SDR FM playback: how it works and how to use it

Code: `crates/rtlsdr/src/rtlsdr.rs`. Test runner: `crates/testing-hook/src/main.rs`.

## What it does

Tunes the RTL-SDR dongle to one frequency, turns the radio signal into sound
(wideband FM, i.e. broadcast radio), and plays it out of the default audio
device.

## How to run it

```bash
cargo run -p testing-hook --release
```

- Edit the frequency in `crates/testing-hook/src/main.rs`
  (`RtlParams::new(0, 100_000_000, false, 0)`), then run again.
- Ctrl+C to quit.
- Start with the volume low. Pick a strong local FM station.
- Needs the dongle plugged in, `librtlsdr` installed, and ALSA dev headers to build.
- If the dongle won't open, the kernel's DVB driver may be holding it
  (`dvb_usb_rtl28xxu`); it has to be blacklisted.

## `RtlParams::new(index, freq_hz, agc, ppm)`

| Arg | Meaning |
|-----|---------|
| `device_usb_index` | Which dongle. 0 = first one found. |
| `center_frequency` | Tune-to frequency in Hz. 100 MHz = `100_000_000`. |
| `use_acg` | Turns on the dongle's automatic gain. (Field name has a typo, `acg`, in the code.) |
| `cystal_ppm` | Corrects the dongle's cheap clock. 0 if unknown. (Also a typo, `cystal`.) |

## How the signal flows

```
dongle --(raw bytes)--> capture thread --channel--> main thread --channel--> sound card callback
                                                    (demodulate)             (plays samples)
```

1. **Capture thread.** `read_async` hands over 32 KB blocks of raw bytes. The
   block is only valid inside the callback, so it is copied and sent over a
   channel.
2. **Main thread (the demodulator).** Loops over the blocks:
   - The bytes come in pairs: I, Q, I, Q... (the two parts of the radio
     signal). Each is centered around 0 and scaled to about -1.0 to 1.0.
   - **FM demodulation:** FM puts the sound in how fast the signal's angle
     is turning. For each pair we compare against the previous pair and take
     the angle between them (`atan2`). That angle *is* the audio.
   - **Downsampling:** the dongle runs at 240,000 samples/sec, audio needs
     48,000. Every 5 samples are averaged into 1.
   - Each audio sample is sent over a second channel.
3. **Sound card callback.** `cpal` calls it whenever the speakers want more
   audio. It pulls samples off the channel and copies each one to every
   channel (left and right). If there's nothing ready, it plays silence.

## Limits (by design, kept minimal)

- Wideband FM only. AM, narrowband FM and SSB would need a different demod step.
- No de-emphasis or proper filtering, so it sounds a bit hissy and bright.
- Mono only. Broadcast stereo is not decoded.
- Runs forever. There is no stop signal besides killing the program.
- The audio channel is unbounded. If the sound card falls behind, memory
  slowly grows.
- No volume control. The fixed gain is `1.5` in the decimation step.

## Ideas for next steps

- Take the frequency from a command-line argument so no rebuild is needed.
- Add a stop signal so a UI (Tauri) can start and stop playback.
- Add a de-emphasis filter (single-pole low-pass, about 50 or 75 us) for cleaner sound.
