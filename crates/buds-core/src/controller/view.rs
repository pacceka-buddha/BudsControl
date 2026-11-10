//! Values derived from the controller state (ported from the computed properties of
//! `BudsController.swift`) and the published [`Snapshot`].

use super::*;
use crate::catalog::{self, DeviceModel};

impl<P: Platform> Controller<P> {
    /// What this model supports: the catalog entry for its product ID, or a safe generic one.
    pub(super) fn model(&self) -> &'static DeviceModel {
        catalog::model(self.product_id.as_deref(), Some(&self.device_name))
    }

    pub(super) fn available_modes(&self) -> Vec<NoiseMode> {
        let model = self.model();
        NoiseMode::ALL
            .into_iter()
            .filter(|mode| {
                model
                    .mode_types
                    .iter()
                    .any(|t| mode_type::mode(*t) == *mode)
            })
            .collect()
    }

    pub(super) fn available_levels(&self) -> Vec<AncLevel> {
        let model = self.model();
        AncLevel::ALL
            .into_iter()
            .filter(|level| model.mode_types.contains(&level.mode_type()))
            .collect()
    }

    /// Built-in presets for this model followed by the custom EQs stored on the buds.
    pub(super) fn eq_choices(&self) -> Vec<EqPreset> {
        self.model()
            .eq_presets
            .iter()
            .map(|(id, t)| EqPreset {
                id: *id,
                name: eq_preset_name(*t),
            })
            .chain(self.custom_eqs.iter().map(|e| EqPreset {
                id: e.id,
                name: e.name.clone(),
            }))
            .collect()
    }

    pub(super) fn active_custom_eq(&self) -> Option<&CustomEq> {
        self.custom_eqs.iter().find(|e| Some(e.id) == self.eq_id)
    }

    /// Lowest earbud level, for the tray ring.
    pub(super) fn buds_battery(&self) -> Option<u8> {
        [self.left, self.right]
            .into_iter()
            .flatten()
            .map(|b| b.level)
            .min()
    }

    /// Whether to show a setting: nothing may rule it out (catalog, capability bit), and the buds
    /// must have answered for it, so a control never appears for something the model can't do.
    pub(super) fn supports(&self, feature: DeviceFeature) -> bool {
        let model = self.model();
        if !model.allows(feature) {
            return false;
        }
        if let Some(caps) = &self.capabilities
            && !caps.contains(&feature.capability_bit())
        {
            return false;
        }
        match feature {
            DeviceFeature::NoiseControl => {
                self.mode.is_some() && !self.available_modes().is_empty()
            }
            DeviceFeature::Equalizer => self.eq_id.is_some() && !self.eq_choices().is_empty(),
            DeviceFeature::CustomEq => !self.custom_eqs.is_empty(),
            DeviceFeature::LowLatency => self.game_mode.is_some(),
            DeviceFeature::Gestures => !self.gestures.is_empty(),
            // No query to confirm it, so it needs positive word from the catalog or the capability bits.
            DeviceFeature::FindBuds => model.features.is_some() || self.capabilities.is_some(),
        }
    }

    /// Gesture actions for this brand (realme has its own assistant code and no noise-control
    /// gesture), plus noise control / low latency only when the buds have them.
    fn gesture_actions(&self, current: GestureAction) -> Vec<GestureAction> {
        use GestureAction::*;
        let realme = self.model().brand == Brand::Realme;
        let mut actions = if realme {
            vec![
                None,
                PlayPause,
                Previous,
                Next,
                VolumeUp,
                VolumeDown,
                AssistantRealme,
            ]
        } else {
            vec![
                None, PlayPause, Assistant, Previous, Next, VolumeUp, VolumeDown,
            ]
        };
        if !realme && self.supports(DeviceFeature::NoiseControl) {
            actions.push(NoiseControl);
        }
        if self.supports(DeviceFeature::LowLatency) {
            actions.push(LowLatency);
        }
        if !actions.contains(&current) {
            actions.push(current);
        }
        actions
    }

    fn gesture_state(&self) -> GestureState {
        let side = |side: EarSide| {
            let map = self.gestures.get(&side)?;
            let mut out = SideGestures::default();
            for gesture in Gesture::ALL {
                if let Some(&current) = map.get(&gesture) {
                    let options = self
                        .gesture_actions(current)
                        .into_iter()
                        .map(|action| ActionOption {
                            action,
                            title: action.title().to_string(),
                        })
                        .collect();
                    out.set(gesture, GestureSlot { current, options });
                }
            }
            Some(out)
        };
        GestureState {
            left: side(EarSide::Left),
            right: side(EarSide::Right),
        }
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        let model = self.model();
        let mut layout: Vec<LayoutBit> = model
            .reported_bits
            .iter()
            .map(|(bit, t)| LayoutBit {
                bit: *bit,
                mode_type: *t,
                name: mode_type::name(*t),
            })
            .collect();
        layout.sort_by_key(|l| l.bit);
        let supports = Supports {
            noise_control: self.supports(DeviceFeature::NoiseControl),
            equalizer: self.supports(DeviceFeature::Equalizer),
            custom_eq: self.supports(DeviceFeature::CustomEq),
            low_latency: self.supports(DeviceFeature::LowLatency),
            find_buds: self.supports(DeviceFeature::FindBuds),
            gestures: self.supports(DeviceFeature::Gestures),
        };
        Snapshot {
            link: self.link,
            device_name: self.device_name.clone(),
            product_id: self.product_id.clone(),
            address: self.address.clone(),
            channel: self.channel_id,
            mode: self.mode,
            level: self.level,
            noise_bit: self.noise_bit,
            left: self.left,
            right: self.right,
            case: self.case_battery,
            buds_battery: self.buds_battery(),
            low_latency: self.game_mode,
            eq_id: self.eq_id,
            custom_eqs: self.custom_eqs.clone(),
            active_custom_eq: self.active_custom_eq().cloned(),
            eq_choices: self.eq_choices(),
            gestures: self.gesture_state(),
            finding_buds: self.finding_buds,
            hosts: self.hosts.clone(),
            wear_left: self.wear.get(&1).copied(),
            wear_right: self.wear.get(&2).copied(),
            capabilities_count: self.capabilities.as_ref().map(|c| c.len() as u32),
            model_name: model.name.to_string(),
            in_catalog: self
                .product_id
                .as_deref()
                .and_then(catalog::lookup)
                .is_some(),
            brand: model.brand,
            available_modes: self.available_modes(),
            available_levels: self.available_levels(),
            supports,
            has_more_controls: [
                DeviceFeature::Equalizer,
                DeviceFeature::LowLatency,
                DeviceFeature::FindBuds,
                DeviceFeature::Gestures,
            ]
            .into_iter()
            .any(|f| self.supports(f)),
            layout,
            host_label: self.platform.host_label().to_string(),
        }
    }

    /// Publishes the snapshot if it changed.
    pub(super) fn publish(&self) {
        let snapshot = self.snapshot();
        self.state_tx.send_if_modified(|current| {
            if *current == snapshot {
                false
            } else {
                *current = snapshot;
                true
            }
        });
    }
}
