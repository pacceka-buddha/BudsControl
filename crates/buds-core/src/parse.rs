//! Pure decoders for reply and push payloads (`p` = frame payload, `p[0]` is usually a status byte
//! where 0 means success). No state: the controller applies the results.
//!
//! Ported from the `parse*` methods of `BudsController.swift`. Every decoder keeps the Swift
//! length guards, so truncated or odd payloads degrade the same way (stop early, never panic).

use std::collections::BTreeSet;

use crate::catalog::DeviceModel;
use crate::types::{
    AncLevel, Battery, CustomEq, EarSide, EqBand, Gesture, GestureAction, Host, NoiseMode, Wear,
    mode_type,
};

/// Reply to 0x0103: `<status 0> <id byte 2> <id byte 1> <id byte 0>` -> `"ABCDEF"` (bytes reversed).
pub fn product_id(p: &[u8]) -> Option<String> {
    if p.len() >= 4 && p[0] == 0 {
        Some(format!("{:02X}{:02X}{:02X}", p[3], p[2], p[1]))
    } else {
        None
    }
}

/// Reply to 0x0100: `<status> <bitmap, LSB first>`. Each set bit says a group of commands is
/// supported; the bit index is `byte_index * 8 + bit` over the bytes after the status.
pub fn capabilities(p: &[u8]) -> Option<BTreeSet<u16>> {
    if p.len() < 2 || p[0] != 0 {
        return None;
    }
    let mut bits = BTreeSet::new();
    for (byte_index, byte) in p[1..].iter().enumerate() {
        for bit in 0..8 {
            if byte & (1 << bit) != 0 {
                bits.insert((byte_index * 8 + bit) as u16);
            }
        }
    }
    Some(bits)
}

/// What a battery payload says. `None` = not mentioned; for the case, `Some(None)` = mentioned
/// but empty (level 0 and not charging), which clears the displayed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatteryUpdate {
    pub left: Option<Battery>,
    pub right: Option<Battery>,
    pub case: Option<Option<Battery>>,
}

/// `<status> <count> (<id> <level | 0x80 if charging>)*`; id 1 left, 2 right, 3 case.
/// Also used for `0x0204` pushes of type 1 (the type byte takes the status slot).
pub fn battery(p: &[u8]) -> Option<BatteryUpdate> {
    if p.len() < 2 {
        return None;
    }
    let count = usize::from(p[1]);
    let mut update = BatteryUpdate::default();
    let mut i = 2;
    for _ in 0..count {
        if i + 1 >= p.len() {
            break;
        }
        let b = Battery {
            level: p[i + 1] & 0x7F,
            charging: p[i + 1] & 0x80 != 0,
        };
        match p[i] {
            1 => update.left = Some(b),
            2 => update.right = Some(b),
            3 => {
                update.case = Some(if b.level > 0 || b.charging {
                    Some(b)
                } else {
                    None
                })
            }
            _ => {}
        }
        i += 2;
    }
    Some(update)
}

/// `<status> <count> (<side> <0 in case | 1 out of ear | 3 in ear>)*`. A value we don't know maps
/// to `None`, which removes that side's entry (the Swift code assigned `nil` into a dictionary).
pub fn wear(p: &[u8]) -> Option<Vec<(u8, Option<Wear>)>> {
    if p.len() < 2 {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 2;
    for _ in 0..usize::from(p[1]) {
        if i + 1 >= p.len() {
            break;
        }
        out.push((p[i], Wear::from_wire(p[i + 1])));
        i += 2;
    }
    Some(out)
}

/// The noise-mode mask from a 0x010C reply or a type-3 push: the payload contains
/// `01 01 <mask lo> [<mask hi>]`; the first such group with a non-zero mask wins.
pub fn noise_mask(p: &[u8]) -> Option<u16> {
    if p.len() < 3 {
        return None;
    }
    for idx in 0..(p.len() - 2) {
        if p[idx] == 0x01 && p[idx + 1] == 0x01 {
            let mut mask = u16::from(p[idx + 2]);
            if idx + 3 < p.len() {
                mask |= u16::from(p[idx + 3]) << 8;
            }
            if mask != 0 {
                return Some(mask);
            }
        }
    }
    None
}

/// A decoded noise-mode report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseReport {
    /// No usable mask in the payload.
    None,
    /// A bit this model's reported layout doesn't know.
    UnknownBit(u8),
    Known {
        bit: u8,
        mode_type: u8,
        mode: NoiseMode,
        /// Set when the type is one of the named ANC levels; otherwise the controller keeps its
        /// current level (for plain ANC) or clears it.
        level: Option<AncLevel>,
    },
}

/// Decodes a noise report through the model's *reported* bit layout. Models can report a different
/// bit than the one used to set a mode (Transparency is set with bit 2 but reported as bit 8 on the
/// Buds 4; Buds Pro 3 report levels in bits 12-15).
pub fn noise_report(model: &DeviceModel, p: &[u8]) -> NoiseReport {
    let Some(mask) = noise_mask(p) else {
        return NoiseReport::None;
    };
    let bit = mask.trailing_zeros() as u8;
    match model.reported_type(bit) {
        None => NoiseReport::UnknownBit(bit),
        Some(t) => NoiseReport::Known {
            bit,
            mode_type: t,
            mode: mode_type::mode(t),
            level: AncLevel::from_mode_type(t),
        },
    }
}

/// `<status> <count> (<feature id> <0 off | 1 on>)*` for a 0x010D reply.
pub fn switches(p: &[u8]) -> Option<Vec<(u8, bool)>> {
    if p.len() < 2 || p[0] != 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 2;
    for _ in 0..usize::from(p[1]) {
        if i + 1 >= p.len() {
            break;
        }
        out.push((p[i], p[i + 1] == 1));
        i += 2;
    }
    Some(out)
}

/// Reply to 0x010F: `<status 0> <preset id>`.
pub fn eq_selected(p: &[u8]) -> Option<u8> {
    if p.len() >= 2 && p[0] == 0 {
        Some(p[1])
    } else {
        None
    }
}

/// `<status> <count> (<selected> <min dB> <max dB> <id> <name len> <name> <band count>
/// (<Hz lo> <Hz hi> <gain dB>)*)*`. Only custom EQs are listed; built-in presets aren't.
/// An empty name becomes "Custom".
pub fn eq_list(p: &[u8]) -> Option<Vec<CustomEq>> {
    if p.len() < 2 || p[0] != 0 {
        return None;
    }
    let mut result = Vec::new();
    let mut i = 2;
    for _ in 0..usize::from(p[1]) {
        if i + 5 > p.len() {
            break;
        }
        let min_gain = i32::from(p[i + 1] as i8);
        let max_gain = i32::from(p[i + 2] as i8);
        let id = p[i + 3];
        let name_len = usize::from(p[i + 4]);
        i += 5;
        if i + name_len >= p.len() {
            break;
        }
        let name = String::from_utf8_lossy(&p[i..i + name_len]).into_owned();
        i += name_len;
        let count = usize::from(p[i]);
        i += 1;
        if i + count * 3 > p.len() {
            break;
        }
        let bands = (0..count)
            .map(|n| EqBand {
                hz: u32::from(p[i + n * 3]) | (u32::from(p[i + n * 3 + 1]) << 8),
                gain: i32::from(p[i + n * 3 + 2] as i8),
            })
            .collect();
        i += count * 3;
        result.push(CustomEq {
            id,
            name: if name.is_empty() {
                "Custom".into()
            } else {
                name
            },
            min_gain,
            max_gain,
            bands,
        });
    }
    Some(result)
}

/// `<status> <count> (<side> <context> <gesture> <action>)*`. Only music-context (01) double tap /
/// triple tap / hold with an action we know are surfaced; calls and the fixed gestures are left alone.
pub fn gestures(p: &[u8]) -> Option<Vec<(EarSide, Gesture, GestureAction)>> {
    if p.len() < 2 || p[0] != 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 2;
    for _ in 0..usize::from(p[1]) {
        if i + 3 >= p.len() {
            break;
        }
        if let (Some(side), 0x01, Some(gesture), Some(action)) = (
            EarSide::from_wire(p[i]),
            p[i + 1],
            Gesture::from_wire(p[i + 2]),
            GestureAction::from_wire(p[i + 3]),
        ) {
            out.push((side, gesture, action));
        }
        i += 4;
    }
    Some(out)
}

/// `<status> <count> (<address, reversed> <reserved> <2 = connected> <1 = this host> <name len>
/// <name>)*`. Pushes of type 6 use the same layout with the type byte in the status slot.
pub fn hosts(p: &[u8]) -> Option<Vec<Host>> {
    if p.len() < 2 {
        return None;
    }
    let mut result = Vec::new();
    let mut i = 2;
    for _ in 0..usize::from(p[1]) {
        if i + 10 > p.len() {
            break;
        }
        let name_len = usize::from(p[i + 9]);
        if i + 10 + name_len > p.len() {
            break;
        }
        let address = p[i..i + 6]
            .iter()
            .rev()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":");
        let name = String::from_utf8_lossy(&p[i + 10..i + 10 + name_len]).into_owned();
        result.push(Host {
            address,
            name,
            connected: p[i + 7] == 2,
            is_this_host: p[i + 8] == 1,
        });
        i += 10 + name_len;
    }
    Some(result)
}
