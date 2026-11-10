//! "Remember settings between connections": the buds reset their noise mode and low latency on
//! their own (for example after going back in the case), so the app can put back what was last
//! picked in it. Ported from the "Remembered settings" section of `BudsController.swift`.

use super::*;
use crate::settings::Remembered;

impl<P: Platform> Controller<P> {
    /// Records a pick made in the app for the current buds. Always recorded (even with the feature
    /// off) so turning it on later has something to restore.
    pub(super) fn remember(&mut self, change: impl FnOnce(&mut Remembered)) {
        let Some(address) = self.address.clone() else {
            return;
        };
        self.settings
            .update(|s| change(s.remembered.entry(address).or_default()));
    }

    /// Once the buds have said what they're set to, changes back only what differs from the
    /// remembered settings, so a link that comes back as it was sends nothing.
    pub(super) fn restore_settings(&mut self) {
        if !(self.restore_noise || self.restore_low_latency) {
            return;
        }
        let saved = self
            .address
            .as_ref()
            .and_then(|a| self.settings.read(|s| s.remembered.get(a).cloned()));
        let Some(saved) = saved.filter(|_| self.link == LinkState::Connected) else {
            self.restore_noise = false;
            self.restore_low_latency = false;
            return;
        };

        if self.restore_noise
            && self.noise_fresh
            && let Some(current) = self.mode
        {
            self.restore_noise = false;
            if let Some(saved_mode) = saved.mode
                && self.available_modes().contains(&saved_mode)
            {
                let levels = self.available_levels();
                let saved_level = saved.level.filter(|l| levels.contains(l));
                if saved_mode != current
                    || (saved_mode == NoiseMode::Anc
                        && saved_level.is_some()
                        && saved_level != self.level)
                {
                    let suffix = saved_level.map_or(String::new(), |l| format!(" ({})", l.key()));
                    self.info(&format!("restoring {}{suffix}", saved_mode.key()));
                    self.set_mode(saved_mode, saved_level);
                }
            }
        }
        if self.restore_low_latency
            && self.switch_fresh
            && let Some(current) = self.game_mode
        {
            self.restore_low_latency = false;
            if let Some(on) = saved.low_latency
                && on != current
                && self.supports(DeviceFeature::LowLatency)
            {
                self.info(&format!(
                    "restoring low latency {}",
                    if on { "on" } else { "off" }
                ));
                self.set_game_mode(on);
            }
        }
    }
}
