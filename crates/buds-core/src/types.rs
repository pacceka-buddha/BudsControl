//! Domain types shared by the protocol code, the controller and the UI.
//!
//! Ported from `Sources/Protocol.swift`, `Sources/Devices.swift` and the value types nested in
//! `BudsController.swift`. Everything the UI sees derives `Serialize` + `ts_rs::TS`; running
//! `cargo test` regenerates the TypeScript in `src/lib/bindings/`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// MARK: - Noise control

/// The three user-facing noise-control modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoiseMode {
    Off,
    Transparency,
    Anc,
}

impl NoiseMode {
    pub const ALL: [NoiseMode; 3] = [NoiseMode::Off, NoiseMode::Transparency, NoiseMode::Anc];

    /// Stable lowercase name (CLI argument, settings file).
    pub fn key(self) -> &'static str {
        match self {
            NoiseMode::Off => "off",
            NoiseMode::Transparency => "transparency",
            NoiseMode::Anc => "anc",
        }
    }

    pub fn from_key(key: &str) -> Option<NoiseMode> {
        NoiseMode::ALL.into_iter().find(|m| m.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            NoiseMode::Off => "Off",
            NoiseMode::Transparency => "Transparency",
            NoiseMode::Anc => "Noise Cancellation",
        }
    }

    pub fn short_title(self) -> &'static str {
        match self {
            NoiseMode::Off => "Off",
            NoiseMode::Transparency => "Transparency",
            NoiseMode::Anc => "ANC",
        }
    }
}

/// ANC strength levels some models offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AncLevel {
    Smart,
    Max,
    Moderate,
    Mild,
    Adaptive,
}

impl AncLevel {
    /// Declaration order of the Swift enum (`allCases`); the UI lists levels in this order.
    pub const ALL: [AncLevel; 5] = [
        AncLevel::Smart,
        AncLevel::Max,
        AncLevel::Moderate,
        AncLevel::Mild,
        AncLevel::Adaptive,
    ];

    /// HeyMelody "modeType" for this level.
    pub fn mode_type(self) -> u8 {
        match self {
            AncLevel::Mild => 3,
            AncLevel::Max => 4,
            AncLevel::Smart => 7,
            AncLevel::Moderate => 8,
            AncLevel::Adaptive => 10,
        }
    }

    pub fn from_mode_type(mode_type: u8) -> Option<AncLevel> {
        AncLevel::ALL
            .into_iter()
            .find(|l| l.mode_type() == mode_type)
    }

    pub fn key(self) -> &'static str {
        match self {
            AncLevel::Smart => "smart",
            AncLevel::Max => "max",
            AncLevel::Moderate => "moderate",
            AncLevel::Mild => "mild",
            AncLevel::Adaptive => "adaptive",
        }
    }

    pub fn from_key(key: &str) -> Option<AncLevel> {
        AncLevel::ALL.into_iter().find(|l| l.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            AncLevel::Smart => "Smart",
            AncLevel::Max => "Max",
            AncLevel::Moderate => "Moderate",
            AncLevel::Mild => "Mild",
            AncLevel::Adaptive => "Adaptive",
        }
    }
}

/// HeyMelody "modeType": what a noise-mode bit means. Which bit carries which type is per model
/// (see [`crate::catalog::DeviceModel`]).
pub mod mode_type {
    use super::NoiseMode;

    pub const OFF: u8 = 1;
    pub const TRANSPARENCY: u8 = 2;
    /// ANC without a level, on models that have no levels.
    pub const ANC: u8 = 5;

    pub fn mode(mode_type: u8) -> NoiseMode {
        match mode_type {
            1 => NoiseMode::Off,
            2 | 6 => NoiseMode::Transparency,
            _ => NoiseMode::Anc,
        }
    }

    /// Human-readable name, for the developer view.
    pub fn name(mode_type: u8) -> String {
        match mode_type {
            1 => "Off".into(),
            2 => "Transparency".into(),
            3 => "ANC · Mild".into(),
            4 => "ANC · Max".into(),
            5 => "ANC".into(),
            6 => "Voice enhancement".into(),
            7 => "ANC · Smart".into(),
            8 => "ANC · Moderate".into(),
            10 => "ANC · Adaptive".into(),
            other => format!("Unknown (type {other})"),
        }
    }
}

// MARK: - Equalizer

/// A selectable EQ preset (built-in or custom).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqPreset {
    pub id: u8,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqBand {
    pub hz: u32,
    pub gain: i32,
}

/// A user EQ stored on the buds: bands of (frequency in Hz, gain in dB) within `min_gain..=max_gain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CustomEq {
    pub id: u8,
    pub name: String,
    pub min_gain: i32,
    pub max_gain: i32,
    pub bands: Vec<EqBand>,
}

/// English names for HeyMelody's EQ "modeType"s (translated from the app's Chinese titles, via
/// RawS-Music). Built-in presets aren't reported by the buds, so the catalog lists each model's.
pub fn eq_preset_name(mode_type: u8) -> String {
    match mode_type {
        1 => "Classic".into(),
        2 => "Bass boost".into(),
        3 => "Clear vocals".into(),
        4 => "Bright".into(),
        5 | 35 => "Default".into(),
        6 | 36 => "Dynaudio Natural".into(),
        7 | 37 => "Dynaudio Warm".into(),
        8 | 38 => "Dynaudio Bold".into(),
        9 | 39 => "Dynaudio Pure".into(),
        10 => "Joe Hisaishi".into(),
        11 | 17 => "Balanced".into(),
        12 => "Bass".into(),
        13 => "Bright".into(),
        14 => "Vocal".into(),
        15 => "Mellow".into(),
        16 => "Classic".into(),
        18 => "Reno Dawn".into(),
        19 => "Hans Zimmer".into(),
        20 => "Natural".into(),
        21 => "Reno Sunrise".into(),
        22 => "Pure".into(),
        23 => "Bold".into(),
        24 => "Spacious".into(),
        25 => "Reno Galaxy".into(),
        26 => "Ultimate".into(),
        27 => "Detailed".into(),
        28 => "Pure vocals".into(),
        29 => "Big bass".into(),
        30 => "Dynaudio".into(),
        31 => "Punchy bass".into(),
        32 => "Warm vocals".into(),
        33 => "Galaxy".into(),
        34 => "Vibrant".into(),
        40 => "Dynaudio Vocal".into(),
        41 => "Clear".into(),
        other => format!("Preset {other}"),
    }
}

// MARK: - Gestures

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EarSide {
    Left,
    Right,
}

impl EarSide {
    pub const ALL: [EarSide; 2] = [EarSide::Left, EarSide::Right];

    /// Byte used on the wire.
    pub fn wire(self) -> u8 {
        match self {
            EarSide::Left => 1,
            EarSide::Right => 2,
        }
    }

    pub fn from_wire(byte: u8) -> Option<EarSide> {
        match byte {
            1 => Some(EarSide::Left),
            2 => Some(EarSide::Right),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            EarSide::Left => "left",
            EarSide::Right => "right",
        }
    }

    pub fn from_key(key: &str) -> Option<EarSide> {
        EarSide::ALL.into_iter().find(|s| s.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            EarSide::Left => "Left",
            EarSide::Right => "Right",
        }
    }
}

/// Touch gestures in the music context. On the wire a gesture is two bytes: context (01 = music,
/// 06 = during a call) then the gesture code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Gesture {
    DoubleTap,
    TripleTap,
    Hold,
}

impl Gesture {
    pub const ALL: [Gesture; 3] = [Gesture::DoubleTap, Gesture::TripleTap, Gesture::Hold];

    pub fn wire(self) -> u8 {
        match self {
            Gesture::DoubleTap => 2,
            Gesture::TripleTap => 3,
            Gesture::Hold => 4,
        }
    }

    pub fn from_wire(byte: u8) -> Option<Gesture> {
        match byte {
            2 => Some(Gesture::DoubleTap),
            3 => Some(Gesture::TripleTap),
            4 => Some(Gesture::Hold),
            _ => None,
        }
    }

    /// CLI / settings spelling (`doubleTap`, `tripleTap`, `hold`).
    pub fn key(self) -> &'static str {
        match self {
            Gesture::DoubleTap => "doubleTap",
            Gesture::TripleTap => "tripleTap",
            Gesture::Hold => "hold",
        }
    }

    pub fn from_key(key: &str) -> Option<Gesture> {
        Gesture::ALL.into_iter().find(|g| g.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            Gesture::DoubleTap => "Double tap",
            Gesture::TripleTap => "Triple tap",
            Gesture::Hold => "Touch & hold",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GestureAction {
    None,
    PlayPause,
    Assistant,
    AssistantRealme,
    Previous,
    Next,
    NoiseControl,
    VolumeUp,
    VolumeDown,
    LowLatency,
}

impl GestureAction {
    pub const ALL: [GestureAction; 10] = [
        GestureAction::None,
        GestureAction::PlayPause,
        GestureAction::Assistant,
        GestureAction::AssistantRealme,
        GestureAction::Previous,
        GestureAction::Next,
        GestureAction::NoiseControl,
        GestureAction::VolumeUp,
        GestureAction::VolumeDown,
        GestureAction::LowLatency,
    ];

    pub fn wire(self) -> u8 {
        match self {
            GestureAction::None => 0x00,
            GestureAction::PlayPause => 0x01,
            GestureAction::Assistant => 0x03,
            GestureAction::AssistantRealme => 0x04,
            GestureAction::Previous => 0x05,
            GestureAction::Next => 0x06,
            GestureAction::NoiseControl => 0x08,
            GestureAction::VolumeUp => 0x0B,
            GestureAction::VolumeDown => 0x0C,
            GestureAction::LowLatency => 0x11,
        }
    }

    pub fn from_wire(byte: u8) -> Option<GestureAction> {
        GestureAction::ALL.into_iter().find(|a| a.wire() == byte)
    }

    /// CLI spelling, same as the Swift case names (`playPause`, `volumeUp`, ...).
    pub fn key(self) -> &'static str {
        match self {
            GestureAction::None => "none",
            GestureAction::PlayPause => "playPause",
            GestureAction::Assistant => "assistant",
            GestureAction::AssistantRealme => "assistantRealme",
            GestureAction::Previous => "previous",
            GestureAction::Next => "next",
            GestureAction::NoiseControl => "noiseControl",
            GestureAction::VolumeUp => "volumeUp",
            GestureAction::VolumeDown => "volumeDown",
            GestureAction::LowLatency => "lowLatency",
        }
    }

    pub fn from_key(key: &str) -> Option<GestureAction> {
        GestureAction::ALL.into_iter().find(|a| a.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            GestureAction::None => "None",
            GestureAction::PlayPause => "Play / Pause",
            GestureAction::Assistant | GestureAction::AssistantRealme => "Voice assistant",
            GestureAction::Previous => "Previous track",
            GestureAction::Next => "Next track",
            GestureAction::NoiseControl => "Noise control",
            GestureAction::VolumeUp => "Volume up",
            GestureAction::VolumeDown => "Volume down",
            GestureAction::LowLatency => "Low latency",
        }
    }
}

// MARK: - Devices

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Brand {
    Oneplus,
    Oppo,
    Realme,
}

/// Settings that only some models have. The UI shows one only when nothing rules it out: the
/// catalog lists it (when the catalog knows the model), the buds' capability reply has its bit set
/// (when they send one), and the buds have answered a query for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DeviceFeature {
    NoiseControl,
    Equalizer,
    CustomEq,
    LowLatency,
    FindBuds,
    Gestures,
}

impl DeviceFeature {
    pub const ALL: [DeviceFeature; 6] = [
        DeviceFeature::NoiseControl,
        DeviceFeature::Equalizer,
        DeviceFeature::CustomEq,
        DeviceFeature::LowLatency,
        DeviceFeature::FindBuds,
        DeviceFeature::Gestures,
    ];

    /// Bit in the buds' capability bitmap (reply to 0x0100) for the commands this feature uses,
    /// per HeyMelody's capability map (via RawS-Music).
    pub fn capability_bit(self) -> u16 {
        match self {
            DeviceFeature::Gestures => 3,     // 0x0108 / 0x0401
            DeviceFeature::FindBuds => 5,     // 0x0400
            DeviceFeature::LowLatency => 7,   // 0x0403
            DeviceFeature::NoiseControl => 8, // 0x010C / 0x0404
            DeviceFeature::Equalizer => 10,   // 0x010F / 0x0406
            DeviceFeature::CustomEq => 34,    // 0x0122 / 0x0418
        }
    }

    /// Name used by the `shows:` line of the CLI status block (matches the Swift case names).
    pub fn key(self) -> &'static str {
        match self {
            DeviceFeature::NoiseControl => "noiseControl",
            DeviceFeature::Equalizer => "equalizer",
            DeviceFeature::CustomEq => "customEQ",
            DeviceFeature::LowLatency => "lowLatency",
            DeviceFeature::FindBuds => "findBuds",
            DeviceFeature::Gestures => "gestures",
        }
    }
}

// MARK: - Live state

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LinkState {
    Searching,
    Connecting,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Battery {
    /// 0..=100
    pub level: u8,
    pub charging: bool,
}

/// Wear status of one earbud (command 0x0109 / push type 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Wear {
    InCase,
    OutOfEar,
    InEar,
}

impl Wear {
    pub fn from_wire(byte: u8) -> Option<Wear> {
        match byte {
            0 => Some(Wear::InCase),
            1 => Some(Wear::OutOfEar),
            3 => Some(Wear::InEar),
            _ => None,
        }
    }
}

/// A device the buds are paired with (multipoint), from command 0x0112 / push type 6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Host {
    /// `AA:BB:CC:DD:EE:FF`
    pub address: String,
    pub name: String,
    pub connected: bool,
    /// True for the entry that is the computer we're running on.
    pub is_this_host: bool,
}
