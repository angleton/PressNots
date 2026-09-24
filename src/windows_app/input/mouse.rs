const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_XBUTTONDOWN: u32 = 0x020b;
const WM_XBUTTONUP: u32 = 0x020c;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MouseButtonEvent {
    pub(crate) button: &'static str,
    pub(crate) action: &'static str,
}

pub(crate) fn decode_message(message: u32, mouse_data: u32) -> Option<MouseButtonEvent> {
    let (button, action) = match message {
        WM_LBUTTONDOWN => ("left", "down"),
        WM_LBUTTONUP => ("left", "up"),
        WM_RBUTTONDOWN => ("right", "down"),
        WM_RBUTTONUP => ("right", "up"),
        WM_MBUTTONDOWN => ("middle", "down"),
        WM_MBUTTONUP => ("middle", "up"),
        WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let button = match mouse_data >> 16 {
                1 => "x1",
                2 => "x2",
                _ => "x-unknown",
            };
            let action = if message == WM_XBUTTONDOWN {
                "down"
            } else {
                "up"
            };
            (button, action)
        }
        _ => return None,
    };
    Some(MouseButtonEvent { button, action })
}

pub(crate) fn decode_raw_buttons(payload: &[u8]) -> Vec<MouseButtonEvent> {
    let Some(flags) = payload
        .get(4..6)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map(u16::from_ne_bytes)
    else {
        return Vec::new();
    };
    [
        (0x0001, "left", "down"),
        (0x0002, "left", "up"),
        (0x0004, "right", "down"),
        (0x0008, "right", "up"),
        (0x0010, "middle", "down"),
        (0x0020, "middle", "up"),
        (0x0040, "x1", "down"),
        (0x0080, "x1", "up"),
        (0x0100, "x2", "down"),
        (0x0200, "x2", "up"),
    ]
    .into_iter()
    .filter(|(mask, _, _)| flags & mask != 0)
    .map(|(_, button, action)| MouseButtonEvent { button, action })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_middle_button_click() {
        assert_eq!(
            decode_message(WM_MBUTTONDOWN, 0),
            Some(MouseButtonEvent {
                button: "middle",
                action: "down",
            })
        );
        assert_eq!(
            decode_message(WM_MBUTTONUP, 0),
            Some(MouseButtonEvent {
                button: "middle",
                action: "up",
            })
        );
    }

    #[test]
    fn decodes_x1_and_x2_from_high_word_only() {
        assert_eq!(
            decode_message(WM_XBUTTONDOWN, (1 << 16) | 0xffff),
            Some(MouseButtonEvent {
                button: "x1",
                action: "down",
            })
        );
        assert_eq!(
            decode_message(WM_XBUTTONUP, 2 << 16),
            Some(MouseButtonEvent {
                button: "x2",
                action: "up",
            })
        );
    }

    #[test]
    fn preserves_an_unrecognized_x_button_identifier() {
        assert_eq!(
            decode_message(WM_XBUTTONDOWN, 7 << 16),
            Some(MouseButtonEvent {
                button: "x-unknown",
                action: "down",
            })
        );
    }

    #[test]
    fn ignores_non_button_mouse_messages() {
        assert_eq!(decode_message(0x020a, 120 << 16), None);
        assert_eq!(decode_message(0x020e, 120 << 16), None);
        assert_eq!(decode_message(0x0200, 0), None);
    }

    #[test]
    fn decodes_simultaneous_and_rare_raw_mouse_buttons() {
        let payload = [0, 0, 0, 0, 0x41, 0x01];
        assert_eq!(
            decode_raw_buttons(&payload),
            vec![
                MouseButtonEvent {
                    button: "left",
                    action: "down",
                },
                MouseButtonEvent {
                    button: "x1",
                    action: "down",
                },
                MouseButtonEvent {
                    button: "x2",
                    action: "down",
                },
            ]
        );
        assert!(decode_raw_buttons(&[0, 0, 0xff]).is_empty());
    }
}
