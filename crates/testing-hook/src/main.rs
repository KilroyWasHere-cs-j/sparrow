use rtlsdr::rtlsdr::{RtlParams, open_and_stream};

fn main() {
    // First dongle, 100 MHz (change this to a strong local FM station), AGC off, no ppm correction.
    let params = RtlParams::new(0, 100_000_000, false, 0);

    // Blocks forever while the audio plays; Ctrl+C to quit.
    if let Err(e) = open_and_stream(params) {
        eprintln!("rtlsdr error: {e}");
    }
}
