// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn set_call_sign(callsign: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", callsign)
}

use serde::Deserialize;
use sparrow_core::{Gain, Mode, RadioConfig};

// The radio settings as the page sends them (camelCase). Units are the ones the page shows:
// MHz, dB and percent. into_config converts them to what the radio library uses.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RadioSettings {
    device_index: u32,
    frequency_mhz: f64,
    sample_rate: u32,
    mode: String,
    gain_db: Option<f32>, // None = automatic gain
    ppm: i32,
    volume_percent: f32,
    squelch_percent: f32,
}

impl RadioSettings {
    fn into_config(self) -> Result<RadioConfig, String> {
        let mode = match self.mode.as_str() {
            "wfm" => Mode::Wfm,
            "nfm" => Mode::Nfm,
            "am" => Mode::Am,
            "usb" => Mode::Usb,
            "lsb" => Mode::Lsb,
            "cw" => Mode::Cw,
            other => return Err(format!("unknown mode '{other}'")),
        };

        // The radio wants gain in tenths of a dB (30.0 dB = 300)
        let gain = match self.gain_db {
            Some(db) => Gain::Manual((db * 10.0).round() as i32),
            None => Gain::Auto,
        };

        Ok(RadioConfig {
            device_index: self.device_index,
            // Out-of-range values saturate (negative -> 0, huge -> u32::MAX), which the radio's
            // own range check then rejects with a readable message
            frequency_hz: (self.frequency_mhz * 1_000_000.0).round() as u32,
            sample_rate: self.sample_rate,
            mode,
            gain,
            ppm: self.ppm,
            volume: self.volume_percent / 100.0,
            squelch: self.squelch_percent / 100.0,
        })
    }
}

// Starts the radio with the settings from the page. Called by the "Start radio" button.
#[tauri::command]
fn start_radio(settings: RadioSettings) -> Result<(), String> {
    // The error text goes back to the page, which shows it under the controls.
    sparrow_core::start_radio(settings.into_config()?).map_err(|e| e.to_string())
}

// Changes the settings of the running radio. async so it runs off the main thread: it waits for
// the radio to answer, and the window shouldn't freeze while it does.
#[tauri::command]
async fn update_radio(settings: RadioSettings) -> Result<(), String> {
    sparrow_core::update_radio(settings.into_config()?).map_err(|e| e.to_string())
}

// Stops the radio started by start_radio. Does nothing if it isn't running. Called by the
// "Stop radio" button.
#[tauri::command]
fn stop_radio() {
    sparrow_core::stop_radio();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Ctrl+C, `kill` and a closing terminal arrive as signals, which would otherwise end the
    // process on the spot. Stop the radio first, then exit.
    ctrlc::set_handler(|| {
        sparrow_core::stop_radio();
        std::process::exit(0);
    })
    .expect("could not install the Ctrl+C handler");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // The X button: stop the radio before the window goes away, so the dongle is released
        // cleanly instead of being cut off when the process exits. Does nothing if it isn't running.
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                sparrow_core::stop_radio();
            }
        })
        .invoke_handler(tauri::generate_handler![greet, set_call_sign, start_radio, update_radio, stop_radio])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // The app is ending, however that was triggered (X button, Ctrl+Q, a quit menu).
            // Harmless if the radio is already stopped.
            if let tauri::RunEvent::Exit = event {
                sparrow_core::stop_radio();
            }
        });
}
