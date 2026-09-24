use super::super::win32::GetKeyNameTextW;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const RI_KEY_E0: u16 = 0x0002;
const RI_KEY_E1: u16 = 0x0004;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RawKeyboardEvent {
    pub(crate) make_code: u16,
    pub(crate) flags: u16,
    pub(crate) virtual_key: u16,
    pub(crate) pressed: bool,
}

impl RawKeyboardEvent {
    pub(crate) fn identity(self) -> String {
        format!(
            "{:04x}:{:04x}:{:04x}",
            self.make_code,
            self.flags & (RI_KEY_E0 | RI_KEY_E1),
            self.virtual_key
        )
    }
}

pub(crate) fn decode(payload: &[u8]) -> Option<RawKeyboardEvent> {
    let make_code = u16::from_ne_bytes(payload.get(0..2)?.try_into().ok()?);
    let flags = u16::from_ne_bytes(payload.get(2..4)?.try_into().ok()?);
    let virtual_key = u16::from_ne_bytes(payload.get(6..8)?.try_into().ok()?);
    let message = u32::from_ne_bytes(payload.get(8..12)?.try_into().ok()?);
    let pressed = match message {
        WM_KEYDOWN | WM_SYSKEYDOWN => true,
        WM_KEYUP | WM_SYSKEYUP => false,
        _ => return None,
    };
    Some(RawKeyboardEvent {
        make_code,
        flags,
        virtual_key,
        pressed,
    })
}

pub(crate) fn key_name(key: RawKeyboardEvent) -> String {
    let mut l_param = i32::from(key.make_code) << 16;
    if key.flags & RI_KEY_E0 != 0 {
        l_param |= 1 << 24;
    }
    let mut name = [0u16; 128];
    let length = unsafe { GetKeyNameTextW(l_param, name.as_mut_ptr(), name.len() as i32) };
    if length > 0 {
        return OsString::from_wide(&name[..length as usize])
            .to_string_lossy()
            .into_owned();
    }
    format!("VK 0x{:02X}", key.virtual_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_extended_press_and_release() {
        let press = [0x1d, 0, 2, 0, 0, 0, 0xa3, 0, 0x00, 0x01, 0, 0];
        let release = [0x1d, 0, 3, 0, 0, 0, 0xa3, 0, 0x01, 0x01, 0, 0];

        assert_eq!(
            decode(&press),
            Some(RawKeyboardEvent {
                make_code: 0x1d,
                flags: RI_KEY_E0,
                virtual_key: 0xa3,
                pressed: true,
            })
        );
        assert!(!decode(&release).unwrap().pressed);
    }

    #[test]
    fn rejects_short_and_non_key_messages() {
        assert_eq!(decode(&[0; 11]), None);
        assert_eq!(decode(&[0; 12]), None);
    }
}
