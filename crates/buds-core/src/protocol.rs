//! HeyMelody earbuds protocol (OPPO / OnePlus / realme), spoken over classic Bluetooth RFCOMM.
//!
//! Frame layout:
//!
//! ```text
//! AA  <len = payload + 7>  00 00  <cmd lo> <cmd hi>  <seq>  <payload len lo> <payload len hi>  <payload...>
//! ```
//!
//! Responses use the request command with the high bit set (0x0106 -> 0x8106).
//! See `docs/PROTOCOL.md`.

/// Command numbers (from Gadgetbridge's OPPO support and RawS-Music's HeyMelody port).
pub mod cmd {
    pub const HELLO: u16 = 0x0100;
    pub const PRODUCT_ID: u16 = 0x0103;
    pub const BATTERY: u16 = 0x0106;
    pub const WEAR_STATUS: u16 = 0x0109;
    pub const NOISE_QUERY: u16 = 0x010C;
    pub const MULTIPOINT_QUERY: u16 = 0x0112;
    pub const SUBSCRIBE: u16 = 0x0205;
    pub const STATUS_PUSH: u16 = 0x0204;
    pub const SET_SWITCH: u16 = 0x0403;
    pub const SET_NOISE: u16 = 0x0404;
    pub const MULTIPOINT_SET: u16 = 0x040B;
    pub const FIND_BUDS: u16 = 0x0400;
    pub const GESTURE_QUERY: u16 = 0x0108;
    pub const SET_GESTURE: u16 = 0x0401;
    pub const SWITCH_QUERY: u16 = 0x010D;
    pub const EQ_QUERY: u16 = 0x010F;
    pub const EQ_LIST_QUERY: u16 = 0x0122;
    pub const SET_EQ: u16 = 0x0406;
    pub const SET_CUSTOM_EQ: u16 = 0x0418;
}

/// Set on a command number to form the matching reply (0x0106 -> 0x8106).
pub const REPLY_BIT: u16 = 0x8000;

/// The reply command for a request command.
pub const fn reply(command: u16) -> u16 {
    command | REPLY_BIT
}

/// On/off features set with `0x0403 [id, on]` and read with `0x010D [count, ids...]`.
pub mod feature {
    /// HeyMelody "game mode".
    pub const LOW_LATENCY: u8 = 6;
}

/// Subscription / push types: the first payload byte of a `0x0204` push (and the ids sent with
/// the `0x0205` subscribe request).
pub mod push {
    pub const BATTERY: u8 = 0x01;
    pub const WEAR: u8 = 0x02;
    pub const NOISE: u8 = 0x03;
    pub const GAME_MODE: u8 = 0x05;
    pub const HOSTS: u8 = 0x06;
}

/// Frames longer than this (after the length field) are treated as line noise, not waited for.
/// The longest real frame seen is a multipoint device list, well under a kilobyte.
const MAX_FRAME_LEN: usize = 0xFFFF;

/// One decoded protocol frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub command: u16,
    pub seq: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(command: u16, seq: u8, payload: Vec<u8>) -> Frame {
        Frame {
            command,
            seq,
            payload,
        }
    }

    /// Wire bytes. The length is written as a single byte (`payload + 7`, truncated), exactly like
    /// the macOS app; every command we send has a short payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = vec![
            0xAA,
            ((self.payload.len() + 7) & 0xFF) as u8,
            0x00,
            0x00,
            (self.command & 0xFF) as u8,
            (self.command >> 8) as u8,
            self.seq,
            (self.payload.len() & 0xFF) as u8,
            (self.payload.len() >> 8) as u8,
        ];
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// True if this is the reply to `command`.
    pub fn is_reply_to(&self, command: u16) -> bool {
        self.command == reply(command)
    }
}

/// Accumulates bytes from the RFCOMM channel and splits them into frames.
///
/// The length after `AA` is a LEB128 varint (7 bits per byte, high bit = more), so bulk packets such
/// as the multipoint device list can be longer than 127 bytes.
#[derive(Debug, Default)]
pub struct FrameParser {
    buffer: Vec<u8>,
}

impl FrameParser {
    pub fn new() -> FrameParser {
        FrameParser::default()
    }

    /// Bytes buffered while waiting for the rest of a frame.
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }

    /// Feeds received bytes and returns every complete frame now available.
    ///
    /// Differences from the Swift parser, both for malformed input only: a length varint longer
    /// than 4 bytes, or a length above [`MAX_FRAME_LEN`], makes the `AA` count as noise and the
    /// scan moves on (the Swift code would wait on it forever).
    pub fn feed(&mut self, data: &[u8]) -> Vec<Frame> {
        self.buffer.extend_from_slice(data);
        let buf = &self.buffer;
        let mut frames = Vec::new();
        let mut i = 0;
        while i < buf.len() {
            if buf[i] != 0xAA {
                i += 1;
                continue;
            }
            // LEB128 length.
            let mut pos = i + 1;
            let mut total_len: usize = 0;
            let mut shift = 0;
            let mut complete = false;
            let mut malformed = false;
            while pos < buf.len() {
                let byte = buf[pos];
                total_len |= ((byte & 0x7F) as usize) << shift;
                pos += 1;
                if byte & 0x80 == 0 {
                    complete = true;
                    break;
                }
                shift += 7;
                if shift > 21 {
                    malformed = true;
                    break;
                }
            }
            if malformed || (complete && total_len > MAX_FRAME_LEN) {
                i += 1;
                continue;
            }
            if !complete {
                break; // length not fully received yet
            }
            let frame_end = pos + total_len;
            if frame_end > buf.len() {
                break; // body not fully received yet
            }

            let header = pos + 2; // skip the two zero bytes after the length
            if header + 5 <= frame_end {
                let command = u16::from(buf[header]) | (u16::from(buf[header + 1]) << 8);
                let seq = buf[header + 2];
                let payload_len =
                    usize::from(buf[header + 3]) | (usize::from(buf[header + 4]) << 8);
                let payload_start = header + 5;
                let payload_end = (payload_start + payload_len).min(frame_end);
                frames.push(Frame {
                    command,
                    seq,
                    payload: buf[payload_start..payload_end].to_vec(),
                });
            }
            i = frame_end;
        }
        self.buffer.drain(..i.min(self.buffer.len()));
        frames
    }
}

// Hex helpers (packet log, raw commands)

/// `AA 0B 00 ...` (uppercase, space separated).
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for (n, b) in bytes.iter().enumerate() {
        if n > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02X}"));
    }
    out
}

/// Parses hex digits, ignoring everything that isn't a hex digit (spaces, `0x`-less separators).
/// A trailing odd digit is dropped, like the Swift raw-command field.
pub fn parse_hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .chars()
        .filter_map(|c| c.to_digit(16).map(|d| d as u8))
        .collect();
    let (pairs, _dropped_odd_digit) = digits.as_chunks::<2>();
    pairs.iter().map(|[hi, lo]| (hi << 4) | lo).collect()
}
