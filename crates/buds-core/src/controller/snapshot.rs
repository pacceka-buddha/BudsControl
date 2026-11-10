//! The read-only view of the controller that the UI, tray and CLI render. Everything derived
//! (which modes exist, which settings to show, which gesture actions to offer) is computed in Rust
//! so the frontends never re-implement catalog logic.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::*;

/// Which optional settings to show right now (see `Controller::supports`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Supports {
    pub noise_control: bool,
    pub equalizer: bool,
    pub custom_eq: bool,
    pub low_latency: bool,
    pub find_buds: bool,
    pub gestures: bool,
}

impl Supports {
    pub fn get(&self, feature: DeviceFeature) -> bool {
        match feature {
            DeviceFeature::NoiseControl => self.noise_control,
            DeviceFeature::Equalizer => self.equalizer,
            DeviceFeature::CustomEq => self.custom_eq,
            DeviceFeature::LowLatency => self.low_latency,
            DeviceFeature::FindBuds => self.find_buds,
            DeviceFeature::Gestures => self.gestures,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionOption {
    pub action: GestureAction,
    pub title: String,
}

/// One gesture on one earbud: what it does now and what it can be changed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GestureSlot {
    pub current: GestureAction,
    pub options: Vec<ActionOption>,
}

/// The gestures the buds reported for one side (absent = not reported, so not shown).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SideGestures {
    pub double_tap: Option<GestureSlot>,
    pub triple_tap: Option<GestureSlot>,
    pub hold: Option<GestureSlot>,
}

impl SideGestures {
    pub fn get(&self, gesture: Gesture) -> Option<&GestureSlot> {
        match gesture {
            Gesture::DoubleTap => self.double_tap.as_ref(),
            Gesture::TripleTap => self.triple_tap.as_ref(),
            Gesture::Hold => self.hold.as_ref(),
        }
    }

    pub(crate) fn set(&mut self, gesture: Gesture, slot: GestureSlot) {
        match gesture {
            Gesture::DoubleTap => self.double_tap = Some(slot),
            Gesture::TripleTap => self.triple_tap = Some(slot),
            Gesture::Hold => self.hold = Some(slot),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GestureState {
    pub left: Option<SideGestures>,
    pub right: Option<SideGestures>,
}

impl GestureState {
    pub fn side(&self, side: EarSide) -> Option<&SideGestures> {
        match side {
            EarSide::Left => self.left.as_ref(),
            EarSide::Right => self.right.as_ref(),
        }
    }
}

/// One entry of the model's noise-mode bit layout (Developer tab).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayoutBit {
    pub bit: u8,
    pub mode_type: u8,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Snapshot {
    pub link: LinkState,
    pub device_name: String,
    pub product_id: Option<String>,
    /// `AA:BB:CC:DD:EE:FF`
    pub address: Option<String>,
    /// RFCOMM channel in use.
    pub channel: Option<u8>,
    pub mode: Option<NoiseMode>,
    pub level: Option<AncLevel>,
    pub noise_bit: Option<u8>,
    pub left: Option<Battery>,
    pub right: Option<Battery>,
    pub case: Option<Battery>,
    /// Lowest earbud level, for the tray ring.
    pub buds_battery: Option<u8>,
    pub low_latency: Option<bool>,
    pub eq_id: Option<u8>,
    pub custom_eqs: Vec<CustomEq>,
    pub active_custom_eq: Option<CustomEq>,
    /// Built-in presets for this model followed by the custom EQs stored on the buds.
    pub eq_choices: Vec<EqPreset>,
    pub gestures: GestureState,
    pub finding_buds: bool,
    pub hosts: Vec<Host>,
    pub wear_left: Option<Wear>,
    pub wear_right: Option<Wear>,
    /// Number of capability bits the buds reported; `None` until (or unless) they send a bitmap.
    pub capabilities_count: Option<u32>,
    /// Catalog name for the model, empty when unknown.
    pub model_name: String,
    pub in_catalog: bool,
    pub brand: Brand,
    pub available_modes: Vec<NoiseMode>,
    pub available_levels: Vec<AncLevel>,
    pub supports: Supports,
    pub has_more_controls: bool,
    /// Every bit -> modeType the buds may report, sorted by bit.
    pub layout: Vec<LayoutBit>,
    /// "PC", "Mac" or "computer".
    pub host_label: String,
}

impl Snapshot {
    /// The state before anything is known.
    pub fn initial(host_label: &str) -> Snapshot {
        Snapshot {
            link: LinkState::Searching,
            device_name: "Earbuds".into(),
            product_id: None,
            address: None,
            channel: None,
            mode: None,
            level: None,
            noise_bit: None,
            left: None,
            right: None,
            case: None,
            buds_battery: None,
            low_latency: None,
            eq_id: None,
            custom_eqs: Vec::new(),
            active_custom_eq: None,
            eq_choices: Vec::new(),
            gestures: GestureState::default(),
            finding_buds: false,
            hosts: Vec::new(),
            wear_left: None,
            wear_right: None,
            capabilities_count: None,
            model_name: String::new(),
            in_catalog: false,
            brand: Brand::Oppo,
            available_modes: Vec::new(),
            available_levels: Vec::new(),
            supports: Supports::default(),
            has_more_controls: false,
            layout: Vec::new(),
            host_label: host_label.to_string(),
        }
    }

    /// Short status for the header / tooltip: `"80% · ANC · Max"`, `"Connecting…"`, `"Not connected"`.
    pub fn subtitle(&self) -> String {
        match self.link {
            LinkState::Connected => {
                let mut status = self
                    .mode
                    .map_or("Connected".to_string(), |m| m.short_title().to_string());
                if self.mode == Some(NoiseMode::Anc)
                    && let Some(level) = self.level
                {
                    status = format!("{status} · {}", level.title());
                }
                match self.buds_battery {
                    Some(b) => format!("{b}% · {status}"),
                    None => status,
                }
            }
            LinkState::Connecting => "Connecting…".into(),
            LinkState::Searching => "Not connected".into(),
        }
    }
}
