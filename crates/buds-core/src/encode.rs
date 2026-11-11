//! Builders for request payloads (the part after the frame header).

use crate::protocol::{feature, push};
use crate::types::{CustomEq, EarSide, Gesture, GestureAction};

/// 0x010C payload: ask for the current noise mode.
pub const NOISE_QUERY: [u8; 2] = [0x01, 0x01];
/// 0x0205 payload: subscribe to battery (1), wear (2) and noise-mode (3) pushes. The first byte
/// is the number of subscription ids, not a byte count.
pub const SUBSCRIBE: [u8; 4] = [0x03, push::BATTERY, push::WEAR, push::NOISE];
/// 0x0122 payload: list the custom EQs.
pub const EQ_LIST_QUERY: [u8; 2] = [0x01, 0x05];
/// 0x010D payload: read the low-latency switch.
pub const LOW_LATENCY_QUERY: [u8; 2] = [0x01, feature::LOW_LATENCY];

/// 0x0404 payload: `01 01 <little-endian bitmask with only the mode's bit set>`.
/// The mask is `bit / 8 + 1` bytes long. Built byte by byte, so any bit (the Developer tab can send
/// up to 255) gives a valid mask instead of overflowing a shift.
pub fn set_noise(bit: u8) -> Vec<u8> {
    let byte = usize::from(bit) / 8;
    let mut payload = vec![0x01, 0x01];
    payload.resize(2 + byte + 1, 0);
    payload[2 + byte] = 1 << (bit % 8);
    payload
}

/// 0x0403 payload: `<feature id> <0|1>`.
pub fn set_switch(feature_id: u8, on: bool) -> Vec<u8> {
    vec![feature_id, u8::from(on)]
}

/// 0x0406 payload: select a preset.
pub fn set_eq(id: u8) -> Vec<u8> {
    vec![id]
}

/// 0x0418 payload (HeyMelody setEqInfo, action 2 = apply existing):
/// `02 <min> <max> <id> <name len> <name> <band count> (<Hz lo> <Hz hi> <gain>)*`.
pub fn set_custom_eq(eq: &CustomEq) -> Vec<u8> {
    let name: Vec<u8> = eq.name.as_bytes().iter().copied().take(255).collect();
    let mut payload = vec![
        0x02,
        eq.min_gain.clamp(-128, 127) as i8 as u8,
        eq.max_gain.clamp(-128, 127) as i8 as u8,
        eq.id,
        name.len() as u8,
    ];
    payload.extend_from_slice(&name);
    payload.push(eq.bands.len().min(255) as u8);
    for band in eq.bands.iter().take(255) {
        payload.push((band.hz & 0xFF) as u8);
        payload.push(((band.hz >> 8) & 0xFF) as u8);
        payload.push(band.gain.clamp(-128, 127) as i8 as u8);
    }
    payload
}

/// Copies `gains` onto the EQ's bands (clamped to the EQ's range); extra values are ignored and
/// bands without a value keep theirs. Written as max-then-min (not `clamp`) so a nonsensical range
/// from the buds can't panic.
#[allow(clippy::manual_clamp)]
pub fn apply_gains(eq: &mut CustomEq, gains: &[i32]) {
    let (lo, hi) = (eq.min_gain, eq.max_gain);
    for (band, &gain) in eq.bands.iter_mut().zip(gains) {
        band.gain = gain.max(lo).min(hi);
    }
}

/// 0x0400 payload: start / stop the find-my-earbuds sound.
pub fn find_buds(start: bool) -> Vec<u8> {
    vec![u8::from(start)]
}

/// 0x0401 payload: `01 <side> <context 01 = music> <gesture> <action>`.
pub fn set_gesture(side: EarSide, gesture: Gesture, action: GestureAction) -> Vec<u8> {
    vec![0x01, side.wire(), 0x01, gesture.wire(), action.wire()]
}
