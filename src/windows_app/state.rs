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
    pub(super) label: String,
    pub(super) pressed: HashSet<String>,
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

    pub(super) fn light_is_on(&self) -> bool {
        !self.pressed.is_empty()
    }

    pub(super) fn register_device(&mut self, handle: usize, label: String) {
        if let Some(device) = self
            .devices
            .iter_mut()
            .find(|device| device.handle == handle)
        {
            device.label = label;
        } else {
            self.devices.push(DeviceUiState {
                handle,
                label,
                pressed: HashSet::new(),
            });
        }
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
    fn light_stays_on_until_every_button_is_released() {
        let mut state = UiState::default();
        state.record("mouse:left".into(), "left", true);
        state.record("mouse:x1".into(), "x1", true);
        state.record("mouse:left".into(), "left", false);

        assert!(state.light_is_on());
        assert_eq!(state.last_button, "x1");

        state.record("mouse:x1".into(), "x1", false);
        assert!(!state.light_is_on());
        assert_eq!(state.last_button, "x1");
    }

    #[test]
    fn device_light_tracks_only_its_own_buttons() {
        let mut state = UiState::default();
        state.register_device(10, "Mouse A".into());
        state.register_device(20, "Mouse B".into());
        state.record_device(10, "left", true);

        assert!(!state.devices[0].pressed.is_empty());
        assert!(state.devices[1].pressed.is_empty());

        state.record_device(10, "left", false);
        assert!(state.devices[0].pressed.is_empty());
    }
}
