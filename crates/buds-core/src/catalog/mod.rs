//! Per-model facts: what each earbuds model supports. Ported from `Sources/Devices.swift`.
//! The 105 generated entries live in `generated.rs` (see `tools/convert_catalog.py`).

#[rustfmt::skip] // generated; keeps `tools/convert_catalog.py` output byte-identical
mod generated;

pub use generated::MODELS;

use crate::types::{Brand, DeviceFeature};

/// What one earbuds model supports.
#[derive(Debug, Clone, Copy)]
pub struct DeviceModel {
    /// Empty when the source data has no name for the model.
    pub name: &'static str,
    pub brand: Brand,
    /// Noise modes the model offers, as HeyMelody modeTypes.
    pub mode_types: &'static [u8],
    /// `(bit, modeType)` layout used to *set* a mode (proven on real buds by oppo-pods).
    pub set_bits: &'static [(u8, u8)],
    /// Every `(bit, modeType)` the buds may *report*; a superset of `set_bits` on some models.
    pub reported_bits: &'static [(u8, u8)],
    /// Built-in EQ presets: `(preset ID, EQ modeType)`.
    pub eq_presets: &'static [(u8, u8)],
    /// `None` when the source data doesn't say; the buds' replies decide then.
    pub features: Option<&'static [DeviceFeature]>,
}

impl DeviceModel {
    /// The bit to send for a mode type: the proven set layout first (lowest matching bit), then
    /// the reported layout.
    pub fn bit_for_type(&self, mode_type: u8) -> Option<u8> {
        let lowest = |pairs: &[(u8, u8)]| {
            pairs
                .iter()
                .filter(|(_, t)| *t == mode_type)
                .map(|(bit, _)| *bit)
                .min()
        };
        lowest(self.set_bits).or_else(|| lowest(self.reported_bits))
    }

    /// The mode type a reported noise bit stands for.
    pub fn reported_type(&self, bit: u8) -> Option<u8> {
        self.reported_bits
            .iter()
            .find(|(b, _)| *b == bit)
            .map(|(_, t)| *t)
    }

    /// True if the model lists mode type `t` among the modes it offers.
    pub fn offers_mode_type(&self, t: u8) -> bool {
        self.mode_types.contains(&t)
    }

    /// `false` only when the catalog knows the model's feature list and `feature` isn't in it.
    pub fn allows(&self, feature: DeviceFeature) -> bool {
        self.features.is_none_or(|list| list.contains(&feature))
    }
}

/// Buds the catalog doesn't know: the layout most HeyMelody models share, limited to
/// Off / Transparency / ANC because unknown models' ANC levels can't be trusted.
pub static GENERIC: DeviceModel = DeviceModel {
    name: "",
    brand: Brand::Oppo,
    mode_types: &[1, 2, 5],
    set_bits: &[(0, 1), (1, 5), (2, 2), (3, 1), (8, 2)],
    reported_bits: &[
        (0, 1),
        (1, 5),
        (2, 2),
        (3, 1),
        (4, 4),
        (5, 8),
        (6, 3),
        (7, 7),
        (8, 2),
    ],
    eq_presets: &[],
    features: None,
};

/// realme Link uses one layout for all its earbuds: off, transparency, mild, max, moderate, smart
/// (oppo-pods; matches Gadgetbridge's realme codes).
pub static GENERIC_REALME: DeviceModel = DeviceModel {
    name: "",
    brand: Brand::Realme,
    mode_types: &[1, 2, 3, 4, 7, 8],
    set_bits: &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 8), (5, 7)],
    reported_bits: &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 8), (5, 7)],
    eq_presets: &[],
    features: None,
};

/// The catalog entry for an exact product ID (`"065414"`), if known.
pub fn lookup(product_id: &str) -> Option<&'static DeviceModel> {
    MODELS
        .binary_search_by(|(pid, _)| (*pid).cmp(product_id))
        .ok()
        .map(|i| &MODELS[i].1)
}

/// The model for the connected buds: the catalog entry, or a safe generic one. realme product IDs
/// end in 12 or 52 (OnePlus 14/54, OPPO 10/50).
pub fn model(product_id: Option<&str>, device_name: Option<&str>) -> &'static DeviceModel {
    if let Some(found) = product_id.and_then(lookup) {
        return found;
    }
    let realme = device_name.is_some_and(|n| n.to_lowercase().contains("realme"))
        || product_id.is_some_and(|p| p.ends_with("12") || p.ends_with("52"));
    if realme { &GENERIC_REALME } else { &GENERIC }
}
