use std::collections::HashSet;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct UiState {
    pressed: HashSet<String>,
    pub(super) last_button: String,
    pub(super) devices: Vec<DeviceUiState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DeviceUiState {
    pub(super) handle: usize,
    pub(super) key: String,
    pub(super) default_label: String,
    pub(super) label: String,
    pub(super) alias: Option<String>,
    pub(super) pressed: HashSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResetAlias {
    pub(super) key: String,
    pub(super) alias: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            pressed: HashSet::new(),
            last_button: "Waiting for a button press".into(),
            devices: Vec::new(),
        }
    }
}

impl UiState {
    pub(super) fn record(&mut self, key: String, button: &str, pressed: bool) {
        if pressed {
            self.pressed.insert(key);
            self.last_button = button.to_owned();
        } else {
            self.pressed.remove(&key);
        }
    }

    pub(super) fn has_pressed_inputs(&self) -> bool {
        !self.pressed.is_empty()
    }

    pub(super) fn register_device(
        &mut self,
        handle: usize,
        key: String,
        default_label: String,
        alias: Option<&str>,
    ) {
        let alias = alias.map(str::to_owned);
        let label = alias.as_deref().unwrap_or(&default_label).to_owned();
        if let Some(device) = self
            .devices
            .iter_mut()
            .find(|device| device.handle == handle)
        {
            device.key = key;
            device.default_label = default_label;
            device.label = label;
            device.alias = alias;
        } else {
            self.devices.push(DeviceUiState {
                handle,
                key,
                default_label,
                label,
                alias,
                pressed: HashSet::new(),
            });
        }
    }

    pub(super) fn rename_device(&mut self, handle: usize, alias: &str) -> Option<String> {
        let device = self
            .devices
            .iter_mut()
            .find(|device| device.handle == handle)?;
        let alias = alias.trim();
        if alias.is_empty() {
            device.alias = None;
            device.label = device.default_label.clone();
        } else {
            device.alias = Some(alias.to_owned());
            device.label = alias.to_owned();
        }
        Some(device.key.clone())
    }

    pub(super) fn reset_device(&mut self, handle: usize) -> Option<ResetAlias> {
        let device = self
            .devices
            .iter_mut()
            .find(|device| device.handle == handle)?;
        let alias = device.alias.take()?;
        device.label = device.default_label.clone();
        Some(ResetAlias {
            key: device.key.clone(),
            alias,
        })
    }

    pub(super) fn restore_alias(&mut self, reset: &ResetAlias) -> bool {
        let Some(device) = self
            .devices
            .iter_mut()
            .find(|device| device.key == reset.key)
        else {
            return false;
        };
        device.alias = Some(reset.alias.clone());
        device.label = reset.alias.clone();
        true
    }

    pub(super) fn record_device(&mut self, handle: usize, button: &str, pressed: bool) {
        let Some(device) = self
            .devices
            .iter_mut()
            .find(|device| device.handle == handle)
        else {
            return;
        };
        if pressed {
            device.pressed.insert(button.to_owned());
        } else {
            device.pressed.remove(button);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressed_state_remains_until_every_button_is_released() {
        let mut state = UiState::default();
        state.record("mouse:left".into(), "left", true);
        state.record("mouse:x1".into(), "x1", true);
        state.record("mouse:left".into(), "left", false);

        assert!(state.has_pressed_inputs());
        assert_eq!(state.last_button, "x1");

        state.record("mouse:x1".into(), "x1", false);
        assert!(!state.has_pressed_inputs());
        assert_eq!(state.last_button, "x1");
    }

    #[test]
    fn device_light_tracks_only_its_own_buttons() {
        let mut state = UiState::default();
        state.register_device(10, "mouse-a".into(), "Mouse A".into(), None);
        state.register_device(20, "mouse-b".into(), "Mouse B".into(), None);
        state.record_device(10, "left", true);

        assert!(!state.devices[0].pressed.is_empty());
        assert!(state.devices[1].pressed.is_empty());

        state.record_device(10, "left", false);
        assert!(state.devices[0].pressed.is_empty());
    }

    #[test]
    fn alias_replaces_and_empty_alias_restores_default_label() {
        let mut state = UiState::default();
        state.register_device(10, "stable-key".into(), "Keyboard device".into(), None);

        assert_eq!(
            state.rename_device(10, "Editing Keyboard"),
            Some("stable-key".into())
        );
        assert_eq!(state.devices[0].label, "Editing Keyboard");

        state.rename_device(10, "");
        assert_eq!(state.devices[0].label, "Keyboard device");
    }

    #[test]
    fn reset_alias_can_be_undone() {
        let mut state = UiState::default();
        state.register_device(
            10,
            "stable-key".into(),
            "Keyboard device".into(),
            Some("Editing Keyboard"),
        );

        let reset = state.reset_device(10).unwrap();
        assert_eq!(state.devices[0].label, "Keyboard device");
        assert_eq!(state.devices[0].alias, None);

        assert!(state.restore_alias(&reset));
        assert_eq!(state.devices[0].label, "Editing Keyboard");
        assert_eq!(state.devices[0].alias.as_deref(), Some("Editing Keyboard"));
    }
}
