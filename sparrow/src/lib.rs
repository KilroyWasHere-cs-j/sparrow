use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::radio::audio::Player;
use crate::radio::rtlsdr::RtlStream;

// The settings types, so the app can build a RadioConfig to pass to start_radio / update_radio.
pub use crate::radio::rtlsdr::{Gain, Mode, RadioConfig};

mod dig;
mod radio;

// What the rest of the library needs to reach the running radio: a flag to stop it, the monitor
// thread's handle to wait on, and a channel for sending it settings changes.
struct Running {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    cmd_tx: Sender<Command>,
}

// A change for the monitor thread to apply. The thread replies with the outcome, so the caller
// finds out whether the dongle accepted it.
enum Command {
    Update(RadioConfig, Sender<Result<(), String>>),
}

// The one running radio, or None when stopped.
static RADIO: Mutex<Option<Running>> = Mutex::new(None);

// Opens the dongle with `config` and starts playing. Fails if the config is invalid, a radio is
// already running, or the dongle can't be opened.
pub fn start_radio(config: RadioConfig) -> Result<(), Box<dyn Error>> {
    let mut slot = RADIO.lock().map_err(|_| "radio state is unavailable")?;

    // Refuse a second radio. A thread that already ended on its own (the dongle died) doesn't count.
    if slot
        .as_ref()
        .is_some_and(|running| !running.thread.is_finished())
    {
        return Err("radio is already running".into());
    }

    println!("{config:?}");

    // RtlStream::start checks the config with validate() before it touches the dongle
    let stream = RtlStream::start(config.clone())?;

    // monitor keeps running until told to stop, so it gets its own thread. The thread also owns
    // the stream: dropping a stream stops it, so it has to stay alive for as long as the radio runs.
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
    let thread = thread::spawn(move || monitor(stream, thread_stop, cmd_rx, config));

    *slot = Some(Running {
        stop,
        thread,
        cmd_tx,
    });
    Ok(())
}

// Changes the settings of the running radio without restarting it. Pass the full config; only
// what differs from the current settings is applied.
//
// Live: frequency, gain, ppm, volume (and mode, squelch, which nothing uses yet).
// Needs a restart: device index and sample rate. Asking to change them is an error.
pub fn update_radio(config: RadioConfig) -> Result<(), Box<dyn Error>> {
    config.validate()?;

    // Copy the sender out of the slot so the lock isn't held while waiting for the radio thread.
    let cmd_tx = {
        let slot = RADIO.lock().map_err(|_| "radio state is unavailable")?;
        match slot.as_ref() {
            Some(running) if !running.thread.is_finished() => running.cmd_tx.clone(),
            _ => return Err("radio is not running".into()),
        }
    };

    let (reply_tx, reply_rx) = mpsc::channel();
    cmd_tx
        .send(Command::Update(config, reply_tx))
        .map_err(|_| "radio is not running")?;

    // The thread picks changes up between chunks (about 70 ms apart), so the answer is quick.
    reply_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| "the radio did not respond")??;
    Ok(())
}

// Stops the radio and waits until the dongle is released. Does nothing if it isn't running.
pub fn stop_radio() {
    // take() empties the slot so the radio can be started again. The lock is released at the
    // end of this line, before the wait below.
    let running = RADIO.lock().ok().and_then(|mut slot| slot.take());

    if let Some(running) = running {
        // Ask the monitor thread to leave its loop...
        running.stop.store(true, Ordering::Relaxed);
        // ...then wait. When it leaves, the stream is dropped, which cancels the dongle's read.
        let _ = running.thread.join();
    }
}

// Settings that can't change while the radio runs: they decide how the dongle is opened and how
// fast samples arrive. Returns the name of the first one that differs.
fn restart_required(current: &RadioConfig, new: &RadioConfig) -> Option<&'static str> {
    if new.device_index != current.device_index {
        Some("device index")
    } else if new.sample_rate != current.sample_rate {
        Some("sample rate")
    } else {
        None
    }
}

// Applies `new` to the running radio. `current` is updated one setting at a time, only once the
// dongle has accepted it, so it always matches the hardware even if a later change fails.
fn apply_update(
    stream: &mut RtlStream,
    player: &mut Player,
    current: &mut RadioConfig,
    new: &RadioConfig,
) -> Result<(), String> {
    // Checked first so a request with a restart-only change applies nothing at all
    if let Some(name) = restart_required(current, new) {
        return Err(format!(
            "{name} can't change while the radio is running; stop it and start it again"
        ));
    }

    if new.frequency_hz != current.frequency_hz {
        stream.set_frequency(new.frequency_hz)?;
        current.frequency_hz = new.frequency_hz;
    }
    if new.gain != current.gain {
        stream.set_gain(new.gain)?;
        current.gain = new.gain;
    }
    if new.ppm != current.ppm {
        stream.set_ppm(new.ppm)?;
        current.ppm = new.ppm;
    }

    // Not dongle settings: volume goes to the audio player; mode and squelch are only remembered
    // until the audio code uses them.
    player.set_volume(new.volume);
    current.volume = new.volume;
    current.mode = new.mode;
    current.squelch = new.squelch;
    Ok(())
}

// Reads chunks until `stop` is set or the dongle stops sending them, applying settings changes
// as they arrive.
fn monitor(
    mut stream: RtlStream,
    stop: Arc<AtomicBool>,
    cmd_rx: Receiver<Command>,
    mut config: RadioConfig,
) {
    let mut player = Player::new(config.volume).expect("could not open audio output");
    let mut n = 0;

    while !stop.load(Ordering::Relaxed) {
        // Apply pending changes first, so a change never lands in the middle of a chunk
        for cmd in cmd_rx.try_iter() {
            match cmd {
                Command::Update(new, reply) => {
                    let result = apply_update(&mut stream, &mut player, &mut config, &new);
                    let _ = reply.send(result);
                }
            }
        }

        // Wake up every 100 ms even if no data arrives, so the stop flag is always noticed.
        match stream.rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => {
                println!(
                    "chunk {n}: {} bytes, first I/Q = {} {}",
                    chunk.len(),
                    chunk[0],
                    chunk[1]
                );
                player.play(&chunk);
                n += 1;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                eprintln!("radio stream ended: the dongle stopped sending samples");
                break;
            }
        }
    }
    // `stream` is dropped here, which stops the dongle.
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- No dongle needed ----

    // Stopping when nothing is running must be harmless, including more than once.
    #[test]
    fn stop_radio_without_start_does_nothing() {
        stop_radio();
        stop_radio();
    }

    #[test]
    fn update_radio_when_not_running_is_an_error() {
        let err = update_radio(RadioConfig::default())
            .unwrap_err()
            .to_string();
        assert!(err.contains("not running"), "{err}");
    }

    #[test]
    fn update_radio_rejects_a_bad_config_before_anything_else() {
        let config = RadioConfig {
            volume: 2.0,
            ..Default::default()
        };
        let err = update_radio(config).unwrap_err().to_string();
        assert!(err.contains("volume"), "{err}");
    }

    #[test]
    fn start_radio_rejects_a_bad_config_without_opening_the_dongle() {
        let config = RadioConfig {
            frequency_hz: 0,
            ..Default::default()
        };
        let err = start_radio(config).unwrap_err().to_string();
        assert!(err.contains("tuning range"), "{err}");
    }

    #[test]
    fn restart_is_required_for_device_and_sample_rate_only() {
        let current = RadioConfig::default();

        let device = RadioConfig {
            device_index: 1,
            ..current.clone()
        };
        assert_eq!(restart_required(&current, &device), Some("device index"));

        let rate = RadioConfig {
            sample_rate: 250_000,
            ..current.clone()
        };
        assert_eq!(restart_required(&current, &rate), Some("sample rate"));

        // Everything else can change live
        let live = RadioConfig {
            frequency_hz: 146_520_000,
            mode: Mode::Am,
            gain: Gain::Manual(300),
            ppm: 5,
            volume: 0.9,
            squelch: 0.3,
            ..current.clone()
        };
        assert_eq!(restart_required(&current, &live), None);
        assert_eq!(restart_required(&current, &current), None);
    }

    // ---- Need the real dongle. Run with: cargo test -- --ignored --test-threads=1 ----

    #[test]
    #[ignore = "opens the real dongle"]
    fn start_radio_does_not_panic() {
        let _ = start_radio(RadioConfig::default());
    }

    // Start, stop, then start again. If stop didn't release the dongle, the second start fails.
    #[test]
    #[ignore = "opens the real dongle"]
    fn radio_can_be_stopped_and_restarted() {
        start_radio(RadioConfig::default()).expect("first start");
        assert!(
            start_radio(RadioConfig::default()).is_err(),
            "a second start while running should be refused"
        );

        thread::sleep(Duration::from_millis(500));
        stop_radio();

        start_radio(RadioConfig::default()).expect("restart after stop");
        thread::sleep(Duration::from_millis(500));
        stop_radio();
    }

    // Settings change on the running radio: live ones are accepted, restart-only ones refused.
    #[test]
    #[ignore = "opens the real dongle"]
    fn radio_settings_can_change_while_running() {
        let base = RadioConfig::default();
        start_radio(base.clone()).expect("start");
        thread::sleep(Duration::from_millis(300));

        let retuned = RadioConfig {
            frequency_hz: 146_520_000,
            ..base.clone()
        };
        update_radio(retuned.clone()).expect("retune");
        thread::sleep(Duration::from_millis(300));

        let changed = RadioConfig {
            gain: Gain::Manual(300),
            ppm: 2,
            volume: 0.1,
            mode: Mode::Am,
            squelch: 0.2,
            ..retuned.clone()
        };
        update_radio(changed.clone()).expect("gain, ppm, volume, mode and squelch");
        thread::sleep(Duration::from_millis(300));

        update_radio(RadioConfig {
            gain: Gain::Auto,
            ..changed.clone()
        })
        .expect("back to automatic gain");

        // Restart-only changes are refused and leave the radio running
        let err = update_radio(RadioConfig {
            sample_rate: 250_000,
            ..changed.clone()
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("sample rate"), "{err}");

        let err = update_radio(RadioConfig {
            device_index: 1,
            ..changed
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("device index"), "{err}");

        thread::sleep(Duration::from_millis(300));
        stop_radio();
    }
}
