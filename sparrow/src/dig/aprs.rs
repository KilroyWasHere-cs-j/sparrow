//! APRS receive on VHF: 1200 baud audio tones -> AX.25 packets -> text.
//!
//! The layers, in the order the audio goes through them:
//! 1. Tones: each bit time holds either the "mark" tone (1200 Hz) or "space" tone (2200 Hz).
//! 2. Bits: a bit is 1 if the tone stayed the same as the bit before, 0 if it changed (NRZI).
//! 3. Frames: packets are fenced by the flag byte 0x7E. So data can't look like a flag, the
//!    sender inserts a 0 after any five 1s in a row ("bit stuffing"), which we remove.
//! 4. AX.25: addresses, two type bytes, the payload and a checksum.

use std::f64::consts::TAU;

use super::SAMPLE_RATES;

// ---- Bell 202 tones ----

const BAUD: f64 = 1200.0; // bits per second
const MARK_HZ: f64 = 1200.0;
const SPACE_HZ: f64 = 2200.0;

// FM radios often make one tone much louder than the other. Rather than guess by how much, the
// audio is decoded once per entry, scaling the space tone by it before comparing the two.
const SPACE_GAINS: [f32; 7] = [0.5, 0.7, 1.0, 1.4, 2.0, 2.8, 4.0];

// How far the bit clock moves toward each tone change it sees (0 = never, 1 = jumps to it).
const CLOCK_CORRECTION: f32 = 0.18;

// Cut-off for removing any constant offset in the audio, well below both tones.
const HIGH_PASS_HZ: f64 = 100.0;

// Two decodes of the same text closer together than this are one packet found twice. A packet
// takes at least 144 bit times to send, so a repeated transmission is always further apart.
const SAME_PACKET_BITS: f64 = 64.0;

// ---- AX.25 frame layout ----

const FLAG: u8 = 0x7E;
const ADDRESS_LEN: usize = 7; // 6 callsign characters + 1 byte of SSID and flag bits
const MAX_ADDRESSES: usize = 10; // destination, source and up to 8 digipeaters (relays)
const FCS_LEN: usize = 2; // checksum at the end of the frame
const CONTROL_UI: u8 = 0x03; // "unnumbered information": a one-off broadcast packet
const POLL_BIT: u8 = 0x10; // may be set on a UI frame; doesn't change what it is
const PID_NO_LAYER_3: u8 = 0xF0; // the protocol ID APRS uses
// Shortest possible: two addresses, control, PID, checksum
const MIN_FRAME_LEN: usize = 2 * ADDRESS_LEN + 2 + FCS_LEN;
// Comfortably above the longest legal frame (about 330 bytes); anything longer is noise
const MAX_FRAME_LEN: usize = 400;

/// Decodes every APRS packet in a chunk of audio.
///
/// Each call stands alone: a packet cut off at either end of the chunk is lost, so pass chunks a
/// few seconds long or more.
///
/// # Args
/// - `samples`: mono audio from -1.0 to 1.0, as it comes out of an FM receiver.
/// - `sample_rate`: samples per second, 8000 to 384000.
///
/// # Returns
/// - One line per packet, in the order received, in the usual monitor format:
///   `SOURCE>DESTINATION,DIGI1*,DIGI2:payload`. `*` marks the last digipeater that has relayed
///   the packet. The payload is not interpreted further; bytes that can't be printed appear as
///   `<0x1c>`, and a trailing line break is removed.
/// - Only packets with a correct checksum are returned. Empty if there are none, or if the audio
///   is empty or the sample rate is out of range.
pub fn decode_aprs(samples: &[f32], sample_rate: u32) -> Vec<String> {
    if samples.is_empty() || !SAMPLE_RATES.contains(&sample_rate) {
        return Vec::new();
    }
    let rate = f64::from(sample_rate);

    let audio = prepare(samples, rate);
    let mark = tone_strength(&audio, MARK_HZ, rate);
    let space = tone_strength(&audio, SPACE_HZ, rate);

    // Every gain setting gets its own pass; most good packets are found by several of them
    let found = SPACE_GAINS
        .iter()
        .flat_map(|&gain| receive(&mark, &space, gain, rate))
        .collect();

    merge_duplicates(found, (SAME_PACKET_BITS * rate / BAUD) as usize)
}

/// Cleans up the audio before tone detection.
fn prepare(samples: &[f32], sample_rate: f64) -> Vec<f32> {
    // A simple high-pass filter: it follows changes in the input but lets a constant offset
    // (such as from a slightly mistuned FM receiver) fade to zero.
    let keep = (1.0 - TAU * HIGH_PASS_HZ / sample_rate) as f32;
    let mut previous_in = 0.0;
    let mut previous_out = 0.0;

    // Two extra bit times of silence, so a packet ending right at the end of the chunk still
    // gets its last bits read
    let tail = 2 * (sample_rate / BAUD).ceil() as usize;

    samples
        .iter()
        .copied()
        .chain(std::iter::repeat_n(0.0, tail))
        .map(|sample| {
            // One NaN or infinity would otherwise poison every value after it
            let input = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            previous_out = input - previous_in + keep * previous_out;
            previous_in = input;
            previous_out
        })
        .collect()
}

/// How strong one tone is at each sample, measured over the bit time ending there.
fn tone_strength(audio: &[f32], tone_hz: f64, sample_rate: f64) -> Vec<f32> {
    let window = (sample_rate / BAUD).round() as usize;
    let step = TAU * tone_hz / sample_rate;

    // A sample multiplied by a reference cosine and sine at the tone's frequency. Summed over a
    // window, these grow large only if the audio contains that tone, whatever its phase.
    let mix = |n: usize| {
        let (sin, cos) = (step * n as f64).sin_cos();
        let x = f64::from(audio[n]);
        (x * cos, x * sin)
    };

    // Running sums: add the sample entering the window, subtract the one leaving it
    let (mut sum_cos, mut sum_sin) = (0.0, 0.0);
    (0..audio.len())
        .map(|n| {
            let (cos, sin) = mix(n);
            sum_cos += cos;
            sum_sin += sin;

            if n >= window {
                let (cos, sin) = mix(n - window);
                sum_cos -= cos;
                sum_sin -= sin;
            }
            f64::hypot(sum_cos, sum_sin) as f32
        })
        .collect()
}

/// Turns tone strengths into packets for one space gain setting. Returns each packet's text with
/// the sample number where it ended.
fn receive(mark: &[f32], space: &[f32], space_gain: f32, sample_rate: f64) -> Vec<(usize, String)> {
    // The bit clock runs from 0.0 to 1.0 once per bit. A bit is read when it wraps around, so
    // tone changes belong halfway, at 0.5.
    let step = (BAUD / sample_rate) as f32;
    let mut clock = 0.0f32;

    let mut previous_level = 0.0f32;
    let mut previous_tone_is_mark = false;
    let mut deframer = Deframer::default();
    let mut found = Vec::new();

    for (n, (&mark, &space)) in mark.iter().zip(space).enumerate() {
        // Above zero means mark, otherwise space
        let level = mark - space_gain * space;
        let clock_before = clock;
        clock += step;

        // The senders' clock and ours drift apart, so each tone change pulls ours back in line
        if (level > 0.0) != (previous_level > 0.0) {
            // Estimate where between the two samples the level crossed zero
            let fraction = previous_level / (previous_level - level);
            let changed_at = clock_before + step * fraction;
            clock -= (changed_at - 0.5) * CLOCK_CORRECTION;
        }
        previous_level = level;

        if clock >= 1.0 {
            clock -= 1.0;

            // Same tone as the last bit = 1, changed = 0
            let tone_is_mark = level > 0.0;
            let bit = tone_is_mark == previous_tone_is_mark;
            previous_tone_is_mark = tone_is_mark;

            if let Some(line) = deframer.push(bit).and_then(|frame| format_frame(&frame)) {
                found.push((n, line));
            }
        }
    }
    found
}

/// Sorts the packets by time and drops the extra copies found by the other gain settings.
fn merge_duplicates(mut found: Vec<(usize, String)>, same_packet_samples: usize) -> Vec<String> {
    found.sort();

    let mut kept: Vec<(usize, String)> = Vec::new();
    for (at, line) in found {
        // Only recently kept packets can match, so search from the newest and stop early
        let duplicate = kept
            .iter()
            .rev()
            .take_while(|(kept_at, _)| at - kept_at < same_packet_samples)
            .any(|(_, kept_line)| *kept_line == line);

        if !duplicate {
            kept.push((at, line));
        }
    }
    kept.into_iter().map(|(_, line)| line).collect()
}

/// Picks frames out of a stream of bits: finds the flags and removes the stuffed zeros.
#[derive(Default)]
struct Deframer {
    last_eight: u8,  // the newest 8 bits, for spotting a flag
    ones: u32,       // how many 1s in a row have just arrived
    bits: Vec<bool>, // the frame collected so far
    in_frame: bool,  // true once a flag has been seen and nothing has gone wrong since
}

impl Deframer {
    /// Takes the next bit. Returns the frame's bytes when this bit completes the closing flag.
    fn push(&mut self, bit: bool) -> Option<Vec<u8>> {
        // Bytes are sent lowest bit first, so new bits enter at the top
        self.last_eight = (self.last_eight >> 1) | (u8::from(bit) << 7);

        if self.last_eight == FLAG {
            let frame = self.finish();
            // A closing flag also opens the next frame
            self.bits.clear();
            self.ones = 0;
            self.in_frame = true;
            return frame;
        }

        if bit {
            self.ones += 1;
            // Seven 1s never occur inside a frame: the signal was lost or the sender gave up
            if self.ones >= 7 {
                self.in_frame = false;
            }
        } else {
            let stuffed = self.ones == 5;
            self.ones = 0;
            if stuffed {
                return None;
            }
        }

        if self.in_frame {
            self.bits.push(bit);
            if self.bits.len() > MAX_FRAME_LEN * 8 {
                self.in_frame = false;
            }
        }
        None
    }

    /// Packs the collected bits into bytes, if they can be a whole frame.
    fn finish(&self) -> Option<Vec<u8>> {
        if !self.in_frame {
            return None;
        }
        // The first 7 bits of the closing flag were collected before we could know it was a flag
        let len = self.bits.len().checked_sub(7)?;
        if len % 8 != 0 || len < MIN_FRAME_LEN * 8 {
            return None;
        }
        let bytes = self.bits[..len]
            .chunks(8)
            .map(|byte| {
                byte.iter()
                    .rev()
                    .fold(0, |value, &bit| (value << 1) | u8::from(bit))
            })
            .collect();
        Some(bytes)
    }
}

/// The AX.25 checksum (CRC-16/X-25).
fn checksum(data: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for &byte in data {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            let low_bit_set = crc & 1 != 0;
            crc >>= 1;
            if low_bit_set {
                crc ^= 0x8408;
            }
        }
    }
    !crc
}

/// Checks a frame and writes it as text. Returns None if the checksum is wrong or it isn't an
/// APRS-style packet.
fn format_frame(frame: &[u8]) -> Option<String> {
    if frame.len() < MIN_FRAME_LEN {
        return None;
    }
    let (body, fcs) = frame.split_at(frame.len() - FCS_LEN);
    if checksum(body).to_le_bytes() != fcs {
        return None;
    }

    // Addresses repeat until one has the lowest bit of its last byte set
    let mut addresses = Vec::new();
    let mut rest = body;
    loop {
        let (address, after) = rest.split_at_checked(ADDRESS_LEN)?;
        addresses.push(Address::parse(address)?);
        rest = after;

        if address[ADDRESS_LEN - 1] & 1 != 0 {
            break;
        }
        if addresses.len() == MAX_ADDRESSES {
            return None;
        }
    }

    let [control, pid, payload @ ..] = rest else {
        return None;
    };
    if control & !POLL_BIT != CONTROL_UI || *pid != PID_NO_LAYER_3 {
        return None;
    }

    // On the air the destination comes first; in text the source does
    let [destination, source, digipeaters @ ..] = addresses.as_slice() else {
        return None;
    };
    let mut line = format!("{}>{}", source.text, destination.text);

    let last_used = digipeaters.iter().rposition(|digi| digi.high_bit);
    for (i, digipeater) in digipeaters.iter().enumerate() {
        line.push(',');
        line.push_str(&digipeater.text);
        if last_used == Some(i) {
            line.push('*');
        }
    }

    line.push(':');
    push_printable(&mut line, payload);
    Some(line)
}

/// One callsign from a frame's address list.
struct Address {
    text: String,   // callsign, with "-SSID" when the SSID isn't zero
    high_bit: bool, // on a digipeater: it has already relayed this packet
}

impl Address {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let (&ssid_byte, characters) = bytes.split_last()?;

        // Callsign characters are stored shifted up one bit, padded with spaces
        let padded: String = characters.iter().map(|&b| char::from(b >> 1)).collect();
        let mut text = padded.trim_end().to_string();
        if text.is_empty() || !text.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }

        // The SSID (0-15) tells apart several stations under one callsign
        let ssid = (ssid_byte >> 1) & 0x0F;
        if ssid != 0 {
            text.push_str(&format!("-{ssid}"));
        }
        Some(Self {
            text,
            high_bit: ssid_byte & 0x80 != 0,
        })
    }
}

/// Appends the payload as text. Readable characters (including UTF-8) pass through; control
/// characters and bytes that aren't text are written as `<0xNN>`.
fn push_printable(line: &mut String, payload: &[u8]) {
    // Packets usually end with a carriage return, which isn't part of the message
    let end = payload
        .iter()
        .rposition(|&b| b != b'\r' && b != b'\n')
        .map_or(0, |last| last + 1);

    for chunk in payload[..end].utf8_chunks() {
        for c in chunk.valid().chars() {
            if c.is_control() {
                let mut buffer = [0; 4];
                for byte in c.encode_utf8(&mut buffer).bytes() {
                    line.push_str(&format!("<0x{byte:02x}>"));
                }
            } else {
                line.push(c);
            }
        }
        for byte in chunk.invalid() {
            line.push_str(&format!("<0x{byte:02x}>"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dig::test_signal::Noise;

    // ---- Building test packets and audio ----

    // One address field. `last` marks the end of the address list.
    fn address(call: &str, ssid: u8, high_bit: bool, last: bool) -> Vec<u8> {
        let mut out: Vec<u8> = format!("{call:<6}").bytes().map(|b| b << 1).collect();
        let flags = if high_bit { 0x80 } else { 0 };
        out.push(flags | 0x60 | (ssid << 1) | u8::from(last));
        out
    }

    fn with_checksum(mut body: Vec<u8>) -> Vec<u8> {
        let fcs = checksum(&body);
        body.extend(fcs.to_le_bytes());
        body
    }

    // A frame from N0CALL-9 to APRS through two digipeaters, the first already used
    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut body = [
            address("APRS", 0, false, false),
            address("N0CALL", 9, false, false),
            address("WIDE1", 1, true, false),
            address("WIDE2", 1, false, true),
        ]
        .concat();
        body.extend([CONTROL_UI, PID_NO_LAYER_3]);
        body.extend(payload);
        with_checksum(body)
    }

    fn line(payload: &str) -> String {
        format!("N0CALL-9>APRS,WIDE1-1*,WIDE2-1:{payload}")
    }

    fn byte_bits(byte: u8) -> impl Iterator<Item = bool> {
        (0..8).map(move |i| byte & (1 << i) != 0)
    }

    // The bits as sent: leading flags, the stuffed frames separated by flags, closing flags
    fn on_air_bits(frames: &[Vec<u8>], leading: usize, closing: usize) -> Vec<bool> {
        let mut bits: Vec<bool> = (0..leading).flat_map(|_| byte_bits(FLAG)).collect();
        for (i, frame) in frames.iter().enumerate() {
            if i > 0 {
                bits.extend(byte_bits(FLAG));
            }
            let mut ones = 0;
            for bit in frame.iter().flat_map(|&b| byte_bits(b)) {
                bits.push(bit);
                ones = if bit { ones + 1 } else { 0 };
                if ones == 5 {
                    bits.push(false);
                    ones = 0;
                }
            }
        }
        bits.extend((0..closing).flat_map(|_| byte_bits(FLAG)));
        bits
    }

    // What the test transmitter can get wrong on purpose
    #[derive(Clone, Copy)]
    struct Tx {
        sample_rate: u32,
        baud: f64,
        mark_level: f32,
        space_level: f32,
        closing_flags: usize,
    }

    impl Default for Tx {
        fn default() -> Self {
            Self {
                sample_rate: 48_000,
                baud: BAUD,
                mark_level: 0.5,
                space_level: 0.5,
                closing_flags: 3,
            }
        }
    }

    impl Tx {
        // Frames sent back to back as one transmission
        fn send(self, frames: &[Vec<u8>]) -> Vec<f32> {
            let mut mark = true;
            let tones: Vec<bool> = on_air_bits(frames, 30, self.closing_flags)
                .into_iter()
                .map(|bit| {
                    // A 0 bit changes the tone, a 1 bit keeps it
                    mark ^= !bit;
                    mark
                })
                .collect();

            let rate = f64::from(self.sample_rate);
            let samples = (tones.len() as f64 * rate / self.baud) as usize;
            let mut phase = 0.0f64;
            (0..samples)
                .map(|n| {
                    let mark = tones[((n as f64 * self.baud / rate) as usize).min(tones.len() - 1)];
                    let (hz, level) = if mark {
                        (MARK_HZ, self.mark_level)
                    } else {
                        (SPACE_HZ, self.space_level)
                    };
                    // Keeping the phase running avoids clicks where the tone changes
                    phase += TAU * hz / rate;
                    phase.sin() as f32 * level
                })
                .collect()
        }
    }

    // ---- Checksum and framing ----

    #[test]
    fn checksum_matches_the_published_value() {
        // The standard check value for CRC-16/X-25
        assert_eq!(checksum(b"123456789"), 0x906E);
    }

    #[test]
    fn deframer_removes_stuffed_zeros() {
        // Payload full of long runs of 1s and bytes that look like flags
        let original = frame(&[0xFF, 0xFF, 0x7E, 0x7E, 0x3F, 0xF8, 0x1F]);
        let mut deframer = Deframer::default();
        let frames: Vec<_> = on_air_bits(std::slice::from_ref(&original), 3, 3)
            .into_iter()
            .filter_map(|bit| deframer.push(bit))
            .collect();
        assert_eq!(frames, [original]);
    }

    #[test]
    fn deframer_drops_an_aborted_frame() {
        let mut bits = on_air_bits(&[frame(b"hello")], 3, 3);
        // Eight 1s in the middle: an abort
        bits.splice(100..100, [true; 8]);
        let mut deframer = Deframer::default();
        assert!(bits.into_iter().all(|bit| deframer.push(bit).is_none()));
    }

    // ---- AX.25 to text ----

    #[test]
    fn formats_addresses_and_payload() {
        assert_eq!(
            format_frame(&frame(b"!4903.50N/07201.75W-Test")),
            Some(line("!4903.50N/07201.75W-Test"))
        );
    }

    #[test]
    fn marks_only_the_last_used_digipeater() {
        let mut body = [
            address("APRS", 0, true, false), // the high bit on these two means something else
            address("N0CALL", 0, true, false),
            address("DIGI1", 0, true, false),
            address("DIGI2", 15, true, false),
            address("DIGI3", 0, false, true),
        ]
        .concat();
        body.extend([CONTROL_UI, PID_NO_LAYER_3, b'x']);
        assert_eq!(
            format_frame(&with_checksum(body)).as_deref(),
            Some("N0CALL>APRS,DIGI1,DIGI2-15*,DIGI3:x")
        );
    }

    #[test]
    fn formats_a_packet_without_digipeaters_or_payload() {
        let mut body = [
            address("APRS", 0, false, false),
            address("N0CALL", 0, false, true),
        ]
        .concat();
        body.extend([CONTROL_UI | POLL_BIT, PID_NO_LAYER_3]);
        assert_eq!(
            format_frame(&with_checksum(body)).as_deref(),
            Some("N0CALL>APRS:")
        );
    }

    #[test]
    fn escapes_unprintable_bytes_and_trims_line_breaks() {
        assert_eq!(
            format_frame(&frame(b"a\rb\x1c\xff c\r\n")),
            Some(line("a<0x0d>b<0x1c><0xff> c"))
        );
        assert_eq!(
            format_frame(&frame("café 73°".as_bytes())),
            Some(line("café 73°"))
        );
        assert_eq!(format_frame(&frame(b"\r\n")), Some(line("")));
    }

    #[test]
    fn rejects_any_single_damaged_bit() {
        let good = frame(b"hello");
        for bit in 0..good.len() * 8 {
            let mut damaged = good.clone();
            damaged[bit / 8] ^= 1 << (bit % 8);
            assert_eq!(format_frame(&damaged), None, "bit {bit}");
        }
    }

    #[test]
    fn rejects_frames_that_are_not_aprs() {
        let addresses = [
            address("APRS", 0, false, false),
            address("N0CALL", 0, false, true),
        ]
        .concat();
        let build = |tail: &[u8]| with_checksum([addresses.as_slice(), tail].concat());

        // Not a UI frame, then the wrong protocol ID, then nothing after the addresses
        assert_eq!(format_frame(&build(&[0x00, PID_NO_LAYER_3, b'x'])), None);
        assert_eq!(format_frame(&build(&[CONTROL_UI, 0xCF, b'x'])), None);
        assert_eq!(format_frame(&build(&[])), None);
    }

    #[test]
    fn rejects_bad_address_lists() {
        let build = |addresses: Vec<u8>| {
            with_checksum([addresses.as_slice(), &[CONTROL_UI, PID_NO_LAYER_3, b'x']].concat())
        };

        // Only one address
        assert_eq!(format_frame(&build(address("APRS", 0, false, true))), None);
        // A callsign with a character that isn't allowed
        let odd = [
            address("APRS", 0, false, false),
            address("N0>AL", 0, false, true),
        ];
        assert_eq!(format_frame(&build(odd.concat())), None);
        // More addresses than the protocol allows
        let mut long: Vec<u8> = (0..10)
            .flat_map(|_| address("DIGI", 0, false, false))
            .collect();
        long.extend(address("DIGI", 0, false, true));
        assert_eq!(format_frame(&build(long)), None);
        // The end-of-list bit is never set
        assert_eq!(format_frame(&build(address("APRS", 0, false, false))), None);
        // Too short to be anything
        assert_eq!(format_frame(&[]), None);
    }

    // ---- Audio to text ----

    #[test]
    fn decodes_at_common_sample_rates() {
        for sample_rate in [8_000, 11_025, 12_000, 22_050, 44_100, 48_000, 96_000] {
            let tx = Tx {
                sample_rate,
                ..Default::default()
            };
            let audio = tx.send(&[frame(b">status text")]);
            assert_eq!(
                decode_aprs(&audio, sample_rate),
                [line(">status text")],
                "{sample_rate} Hz"
            );
        }
    }

    #[test]
    fn decodes_the_longest_payload() {
        let payload = "x".repeat(256);
        let audio = Tx::default().send(&[frame(payload.as_bytes())]);
        assert_eq!(decode_aprs(&audio, 48_000), [line(&payload)]);
    }

    #[test]
    fn decodes_through_noise() {
        for seed in 1..=20 {
            let mut audio = Tx::default().send(&[frame(b">noisy channel test")]);
            // Noise peaking as high as the signal itself
            Noise::new(seed).add_to(&mut audio, 0.5);
            assert_eq!(
                decode_aprs(&audio, 48_000),
                [line(">noisy channel test")],
                "seed {seed}"
            );
        }
    }

    #[test]
    fn decodes_with_one_tone_much_louder() {
        // Up to 12 dB difference in either direction
        for (mark_level, space_level) in
            [(0.8, 0.2), (0.6, 0.3), (0.3, 0.6), (0.2, 0.8), (0.02, 0.02)]
        {
            let tx = Tx {
                mark_level,
                space_level,
                ..Default::default()
            };
            let mut audio = tx.send(&[frame(b">uneven tones")]);
            Noise::new(3).add_to(&mut audio, mark_level.min(space_level) * 0.2);
            assert_eq!(
                decode_aprs(&audio, 48_000),
                [line(">uneven tones")],
                "mark {mark_level}, space {space_level}"
            );
        }
    }

    #[test]
    fn decodes_when_the_sender_clock_is_off() {
        let payload = "a long payload so that timing error has time to build up ".repeat(3);
        for percent in [-2.0, -1.0, 1.0, 2.0] {
            let tx = Tx {
                baud: BAUD * (1.0 + percent / 100.0),
                ..Default::default()
            };
            let audio = tx.send(&[frame(payload.as_bytes())]);
            assert_eq!(
                decode_aprs(&audio, 48_000),
                [line(&payload)],
                "{percent}% off"
            );
        }
    }

    #[test]
    fn decodes_with_a_constant_offset_and_clipping() {
        let clean = Tx::default().send(&[frame(b">offset")]);

        let offset: Vec<f32> = clean.iter().map(|s| s * 0.5 + 0.6).collect();
        assert_eq!(decode_aprs(&offset, 48_000), [line(">offset")]);

        let clipped: Vec<f32> = clean.iter().map(|s| (s * 20.0).clamp(-1.0, 1.0)).collect();
        assert_eq!(decode_aprs(&clipped, 48_000), [line(">offset")]);

        let inverted: Vec<f32> = clean.iter().map(|s| -s).collect();
        assert_eq!(decode_aprs(&inverted, 48_000), [line(">offset")]);
    }

    #[test]
    fn decodes_a_chunk_that_ends_right_after_one_closing_flag() {
        for sample_rate in [8_000, 11_025, 48_000] {
            let tx = Tx {
                sample_rate,
                closing_flags: 1,
                ..Default::default()
            };
            let audio = tx.send(&[frame(b">tight")]);
            assert_eq!(
                decode_aprs(&audio, sample_rate),
                [line(">tight")],
                "{sample_rate} Hz"
            );
        }
    }

    #[test]
    fn returns_several_packets_in_order() {
        let tx = Tx::default();
        // Two frames sharing one transmission, then a third after a pause
        let mut audio = tx.send(&[frame(b">first"), frame(b">second")]);
        audio.extend(vec![0.0; 24_000]);
        audio.extend(tx.send(&[frame(b">third")]));

        assert_eq!(
            decode_aprs(&audio, 48_000),
            [line(">first"), line(">second"), line(">third")]
        );
    }

    #[test]
    fn keeps_a_packet_that_was_sent_twice() {
        // Shortest possible packet, back to back: the hardest case for duplicate removal
        let mut body = [
            address("APRS", 0, false, false),
            address("N0CALL", 0, false, true),
        ]
        .concat();
        body.extend([CONTROL_UI, PID_NO_LAYER_3]);
        let short = with_checksum(body);

        let audio = Tx::default().send(&[short.clone(), short]);
        assert_eq!(
            decode_aprs(&audio, 48_000),
            ["N0CALL>APRS:", "N0CALL>APRS:"]
        );
    }

    #[test]
    fn a_cut_off_or_damaged_packet_decodes_to_nothing() {
        let audio = Tx::default().send(&[frame(b">this one gets cut")]);
        let half = audio.len() / 2;
        assert!(decode_aprs(&audio[..half], 48_000).is_empty());
        assert!(decode_aprs(&audio[half..], 48_000).is_empty());

        let mut bad = frame(b">damaged");
        bad[30] ^= 0x01;
        assert!(decode_aprs(&Tx::default().send(&[bad]), 48_000).is_empty());
    }

    #[test]
    fn silence_and_noise_decode_to_nothing() {
        let mut audio = vec![0.0; 48_000 * 10];
        assert!(decode_aprs(&audio, 48_000).is_empty());

        for seed in 1..=6 {
            audio.fill(0.0);
            Noise::new(seed).add_to(&mut audio, 1.0);
            assert!(decode_aprs(&audio, 48_000).is_empty(), "seed {seed}");
        }
    }

    #[test]
    fn unusable_input_decodes_to_nothing() {
        assert!(decode_aprs(&[], 48_000).is_empty());
        assert!(decode_aprs(&[0.5], 48_000).is_empty());
        assert!(decode_aprs(&[f32::NAN; 1_000], 48_000).is_empty());
        for rate in [0, 1, 7_999, 384_001, u32::MAX] {
            assert!(decode_aprs(&[0.1; 1_000], rate).is_empty(), "{rate} Hz");
        }
    }

    #[test]
    fn a_stray_bad_sample_does_not_spoil_the_rest() {
        let mut audio = vec![f32::NAN, f32::INFINITY];
        audio.extend(Tx::default().send(&[frame(b">still fine")]));
        assert_eq!(decode_aprs(&audio, 48_000), [line(">still fine")]);
    }
}
