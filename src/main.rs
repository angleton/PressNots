#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("PressNots only runs on Windows.");
}

#[cfg(target_os = "windows")]
mod windows_app {
    use std::collections::{BTreeSet, HashMap};
    use std::ffi::{OsString, c_void};
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStringExt;
    use std::ptr::{null, null_mut};
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Sender};
    use std::thread;

    type Bool = i32;
    type Dword = u32;
    type Handle = *mut c_void;
    type Hhook = Handle;
    type Hinstance = Handle;
    type Hrawinput = Handle;
    type Hwnd = Handle;
    type Lparam = isize;
    type Lresult = isize;
    type Uint = u32;
    type Wparam = usize;

    const WH_MOUSE_LL: i32 = 14;
    const WM_INPUT: Uint = 0x00ff;
    const WM_QUIT: Uint = 0x0012;
    const WM_LBUTTONDOWN: Uint = 0x0201;
    const WM_LBUTTONUP: Uint = 0x0202;
    const WM_RBUTTONDOWN: Uint = 0x0204;
    const WM_RBUTTONUP: Uint = 0x0205;
    const WM_MBUTTONDOWN: Uint = 0x0207;
    const WM_MBUTTONUP: Uint = 0x0208;
    const WM_XBUTTONDOWN: Uint = 0x020b;
    const WM_XBUTTONUP: Uint = 0x020c;
    const RID_INPUT: Uint = 0x10000003;
    const RIDI_DEVICENAME: Uint = 0x20000007;
    const RIDI_DEVICEINFO: Uint = 0x2000000b;
    const RIM_TYPEKEYBOARD: Dword = 1;
    const RIM_TYPEHID: Dword = 2;
    const RIDEV_INPUTSINK: Dword = 0x00000100;
    const HWND_MESSAGE: Hwnd = -3isize as Hwnd;
    const ERROR_VALUE: Uint = Uint::MAX;

    static EVENT_TX: OnceLock<Sender<Event>> = OnceLock::new();
    static BLOCK_MOUSE: AtomicBool = AtomicBool::new(false);

    const CUSTOM_HID_BUTTONS: &[HidButtonDefinition] = &[];

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MouseButtonEvent {
        button: &'static str,
        action: &'static str,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct HidButtonDefinition {
        name: &'static str,
        device_name_contains: &'static str,
        report_id: Option<u8>,
        byte_offset: usize,
        bit_mask: u8,
        active_low: bool,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct HidPayload<'a> {
        report_size: u32,
        report_count: u32,
        bytes: &'a [u8],
    }

    #[derive(Debug)]
    enum Event {
        Mouse {
            button: &'static str,
            action: &'static str,
            x: i32,
            y: i32,
        },
        Hid {
            device: usize,
            report_size: u32,
            report_count: u32,
            bytes: Vec<u8>,
        },
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: Hwnd,
        message: Uint,
        w_param: Wparam,
        l_param: Lparam,
        time: Dword,
        pt: Point,
        l_private: Dword,
    }

    type WndProc = Option<unsafe extern "system" fn(Hwnd, Uint, Wparam, Lparam) -> Lresult>;
    type HookProc = Option<unsafe extern "system" fn(i32, Wparam, Lparam) -> Lresult>;

    #[repr(C)]
    struct WndClassW {
        style: Uint,
        wnd_proc: WndProc,
        cls_extra: i32,
        wnd_extra: i32,
        instance: Hinstance,
        icon: Handle,
        cursor: Handle,
        background: Handle,
        menu_name: *const u16,
        class_name: *const u16,
    }

    #[repr(C)]
    struct MsllHookStruct {
        pt: Point,
        mouse_data: Dword,
        flags: Dword,
        time: Dword,
        extra_info: usize,
    }

    #[repr(C)]
    struct RawInputDeviceList {
        device: Handle,
        kind: Dword,
    }

    #[repr(C)]
    struct RawInputDevice {
        usage_page: u16,
        usage: u16,
        flags: Dword,
        target: Hwnd,
    }

    #[repr(C)]
    struct RawInputHeader {
        kind: Dword,
        size: Dword,
        device: Handle,
        w_param: Wparam,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct RidDeviceInfoHid {
        vendor_id: Dword,
        product_id: Dword,
        version_number: Dword,
        usage_page: u16,
        usage: u16,
    }

    #[repr(C)]
    union RidDeviceInfoData {
        hid: RidDeviceInfoHid,
        padding: [Dword; 6],
    }

    #[repr(C)]
    struct RidDeviceInfo {
        size: Dword,
        kind: Dword,
        data: RidDeviceInfoData,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn CallNextHookEx(hook: Hhook, code: i32, w_param: Wparam, l_param: Lparam) -> Lresult;
        fn CreateWindowExW(
            ex_style: Dword,
            class_name: *const u16,
            window_name: *const u16,
            style: Dword,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: Hwnd,
            menu: Handle,
            instance: Hinstance,
            param: *const c_void,
        ) -> Hwnd;
        fn DefWindowProcW(hwnd: Hwnd, message: Uint, w_param: Wparam, l_param: Lparam) -> Lresult;
        fn DispatchMessageW(message: *const Msg) -> Lresult;
        fn GetMessageW(message: *mut Msg, hwnd: Hwnd, min: Uint, max: Uint) -> Bool;
        fn GetRawInputData(
            input: Hrawinput,
            command: Uint,
            data: *mut c_void,
            size: *mut Uint,
            header_size: Uint,
        ) -> Uint;
        fn GetRawInputDeviceInfoW(
            device: Handle,
            command: Uint,
            data: *mut c_void,
            size: *mut Uint,
        ) -> Uint;
        fn GetRawInputDeviceList(
            list: *mut RawInputDeviceList,
            count: *mut Uint,
            size: Uint,
        ) -> Uint;
        fn RegisterClassW(class: *const WndClassW) -> u16;
        fn RegisterRawInputDevices(devices: *const RawInputDevice, count: Uint, size: Uint)
        -> Bool;
        fn SetWindowsHookExW(
            kind: i32,
            proc: HookProc,
            module: Hinstance,
            thread_id: Dword,
        ) -> Hhook;
        fn TranslateMessage(message: *const Msg) -> Bool;
        fn UnhookWindowsHookEx(hook: Hhook) -> Bool;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLastError() -> Dword;
        fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
    }

    unsafe extern "system" fn mouse_hook(code: i32, w_param: Wparam, l_param: Lparam) -> Lresult {
        if code >= 0 {
            let message = w_param as Uint;
            // SAFETY: Windows supplies an MSLLHOOKSTRUCT for WH_MOUSE_LL callbacks.
            let data = unsafe { &*(l_param as *const MsllHookStruct) };
            if let Some(event) = decode_mouse_message(message, data.mouse_data) {
                if let Some(sender) = EVENT_TX.get() {
                    let _ = sender.send(Event::Mouse {
                        button: event.button,
                        action: event.action,
                        x: data.pt.x,
                        y: data.pt.y,
                    });
                }
                if BLOCK_MOUSE.load(Ordering::Relaxed) {
                    return 1;
                }
            }
        }
        unsafe { CallNextHookEx(null_mut(), code, w_param, l_param) }
    }

    fn decode_mouse_message(message: Uint, mouse_data: Dword) -> Option<MouseButtonEvent> {
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

    unsafe extern "system" fn window_proc(
        hwnd: Hwnd,
        message: Uint,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult {
        if message == WM_INPUT {
            unsafe { process_raw_input(l_param as Hrawinput) };
        }
        unsafe { DefWindowProcW(hwnd, message, w_param, l_param) }
    }

    unsafe fn process_raw_input(input: Hrawinput) {
        let mut byte_count = 0;
        let header_size = size_of::<RawInputHeader>() as Uint;
        if unsafe { GetRawInputData(input, RID_INPUT, null_mut(), &mut byte_count, header_size) }
            != 0
            || byte_count < header_size
        {
            return;
        }

        let mut buffer = vec![0u8; byte_count as usize];
        if unsafe {
            GetRawInputData(
                input,
                RID_INPUT,
                buffer.as_mut_ptr().cast(),
                &mut byte_count,
                header_size,
            )
        } == ERROR_VALUE
        {
            return;
        }

        // SAFETY: GetRawInputData populated at least one complete RAWINPUTHEADER.
        let header = unsafe { &*(buffer.as_ptr().cast::<RawInputHeader>()) };
        if header.kind != RIM_TYPEHID || buffer.len() < header_size as usize + 8 {
            return;
        }
        if let Some(payload) = parse_hid_payload(&buffer[header_size as usize..])
            && let Some(sender) = EVENT_TX.get()
        {
            let _ = sender.send(Event::Hid {
                device: header.device as usize,
                report_size: payload.report_size,
                report_count: payload.report_count,
                bytes: payload.bytes.to_vec(),
            });
        }
    }

    fn parse_hid_payload(payload: &[u8]) -> Option<HidPayload<'_>> {
        let header = payload.get(..8)?;
        let report_size = u32::from_ne_bytes(header[0..4].try_into().ok()?);
        let report_count = u32::from_ne_bytes(header[4..8].try_into().ok()?);
        if report_size == 0 || report_count == 0 {
            return None;
        }
        let data_len = report_size.checked_mul(report_count)? as usize;
        let bytes = payload.get(8..8usize.checked_add(data_len)?)?;
        Some(HidPayload {
            report_size,
            report_count,
            bytes,
        })
    }

    fn decode_hid_buttons<'a>(
        definitions: &'a [HidButtonDefinition],
        device_name: &str,
        report: &[u8],
    ) -> Vec<(&'a str, bool)> {
        definitions
            .iter()
            .filter(|definition| device_name.contains(definition.device_name_contains))
            .filter(|definition| {
                definition
                    .report_id
                    .is_none_or(|id| report.first() == Some(&id))
            })
            .filter_map(|definition| {
                let value = report.get(definition.byte_offset)? & definition.bit_mask != 0;
                Some((definition.name, value != definition.active_low))
            })
            .collect()
    }

    unsafe fn register_raw_devices(hwnd: Hwnd) -> Result<(), String> {
        let mut count = 0;
        let list_size = size_of::<RawInputDeviceList>() as Uint;
        if unsafe { GetRawInputDeviceList(null_mut(), &mut count, list_size) } == ERROR_VALUE {
            return Err("GetRawInputDeviceList(size) failed".into());
        }

        let mut list: Vec<RawInputDeviceList> = Vec::with_capacity(count as usize);
        let result = unsafe { GetRawInputDeviceList(list.as_mut_ptr(), &mut count, list_size) };
        if result == ERROR_VALUE {
            return Err("GetRawInputDeviceList(data) failed".into());
        }
        // SAFETY: Windows initialized `result` entries in the allocated buffer.
        unsafe { list.set_len(result as usize) };

        let mut usages = BTreeSet::from([(0x01u16, 0x02u16)]);
        for entry in &list {
            if entry.kind == RIM_TYPEKEYBOARD {
                continue;
            }
            if entry.kind == RIM_TYPEHID {
                let mut info: RidDeviceInfo = unsafe { zeroed() };
                info.size = size_of::<RidDeviceInfo>() as Dword;
                let mut info_size = info.size;
                if unsafe {
                    GetRawInputDeviceInfoW(
                        entry.device,
                        RIDI_DEVICEINFO,
                        (&mut info as *mut RidDeviceInfo).cast(),
                        &mut info_size,
                    )
                } != ERROR_VALUE
                {
                    // SAFETY: `kind` identifies the active RID_DEVICE_INFO union member.
                    let hid = unsafe { info.data.hid };
                    if hid.usage_page != 0 && hid.usage != 0 {
                        usages.insert((hid.usage_page, hid.usage));
                    }
                }
            }
        }

        let registrations: Vec<_> = usages
            .into_iter()
            .map(|(usage_page, usage)| RawInputDevice {
                usage_page,
                usage,
                flags: RIDEV_INPUTSINK,
                target: hwnd,
            })
            .collect();
        if unsafe {
            RegisterRawInputDevices(
                registrations.as_ptr(),
                registrations.len() as Uint,
                size_of::<RawInputDevice>() as Uint,
            )
        } == 0
        {
            let error = unsafe { GetLastError() };
            let usages = registrations
                .iter()
                .map(|item| format!("{:04x}:{:04x}", item.usage_page, item.usage))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "RegisterRawInputDevices failed with Win32 error {error}; usages: {usages}"
            ));
        }

        println!("Monitoring {} Raw Input usage(s):", registrations.len());
        for registration in registrations {
            println!(
                "  usage page 0x{:04x}, usage 0x{:04x}",
                registration.usage_page, registration.usage
            );
        }
        Ok(())
    }

    unsafe fn device_name(device: usize) -> Option<String> {
        let mut char_count = 0;
        if unsafe {
            GetRawInputDeviceInfoW(
                device as Handle,
                RIDI_DEVICENAME,
                null_mut(),
                &mut char_count,
            )
        } == ERROR_VALUE
            || char_count == 0
        {
            return None;
        }
        let mut name = vec![0u16; char_count as usize];
        if unsafe {
            GetRawInputDeviceInfoW(
                device as Handle,
                RIDI_DEVICENAME,
                name.as_mut_ptr().cast(),
                &mut char_count,
            )
        } == ERROR_VALUE
        {
            return None;
        }
        let end = name
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(name.len());
        Some(
            OsString::from_wide(&name[..end])
                .to_string_lossy()
                .into_owned(),
        )
    }

    fn print_events(receiver: mpsc::Receiver<Event>) {
        let mut button_states: HashMap<(usize, &'static str), bool> = HashMap::new();
        for event in receiver {
            match event {
                Event::Mouse {
                    button,
                    action,
                    x,
                    y,
                } => {
                    println!("mouse button={button} action={action} x={x} y={y}");
                }
                Event::Hid {
                    device,
                    report_size,
                    report_count,
                    bytes,
                } => {
                    let name = unsafe { device_name(device) }.unwrap_or_else(|| "unknown".into());
                    let hex = bytes
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!(
                        "hid device=0x{device:x} name={name:?} report_size={report_size} report_count={report_count} data=[{hex}]"
                    );
                    for report in bytes.chunks_exact(report_size as usize) {
                        for (button, pressed) in
                            decode_hid_buttons(CUSTOM_HID_BUTTONS, &name, report)
                        {
                            let previous = button_states.insert((device, button), pressed);
                            if previous.is_some_and(|value| value != pressed)
                                || previous.is_none() && pressed
                            {
                                let action = if pressed { "down" } else { "up" };
                                println!(
                                    "hid-button device=0x{device:x} button={button} action={action}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain([0]).collect()
    }

    pub fn run() -> Result<(), String> {
        BLOCK_MOUSE.store(
            std::env::args().any(|arg| arg == "--block-mouse"),
            Ordering::Relaxed,
        );
        let (sender, receiver) = mpsc::channel();
        EVENT_TX
            .set(sender)
            .map_err(|_| "event channel was already initialized")?;
        thread::spawn(move || print_events(receiver));

        let instance = unsafe { GetModuleHandleW(null()) };
        let class_name = wide_null("PressNotsRawInputWindow");
        let window_class = WndClassW {
            style: 0,
            wnd_proc: Some(window_proc),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu_name: null(),
            class_name: class_name.as_ptr(),
        };
        if unsafe { RegisterClassW(&window_class) } == 0 {
            return Err("RegisterClassW failed".into());
        }
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                instance,
                null(),
            )
        };
        if hwnd.is_null() {
            return Err("CreateWindowExW failed".into());
        }
        unsafe { register_raw_devices(hwnd)? };

        let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), instance, 0) };
        if hook.is_null() {
            return Err("SetWindowsHookExW failed".into());
        }

        println!(
            "Listening globally. Press Ctrl+C to stop. Mouse suppression: {}",
            BLOCK_MOUSE.load(Ordering::Relaxed)
        );
        let mut message: Msg = unsafe { zeroed() };
        loop {
            let status = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
            if status <= 0 || message.message == WM_QUIT {
                break;
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        unsafe { UnhookWindowsHookEx(hook) };
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn decodes_middle_button_click() {
            assert_eq!(
                decode_mouse_message(WM_MBUTTONDOWN, 0),
                Some(MouseButtonEvent {
                    button: "middle",
                    action: "down",
                })
            );
            assert_eq!(
                decode_mouse_message(WM_MBUTTONUP, 0),
                Some(MouseButtonEvent {
                    button: "middle",
                    action: "up",
                })
            );
        }

        #[test]
        fn decodes_x1_and_x2_from_high_word_only() {
            assert_eq!(
                decode_mouse_message(WM_XBUTTONDOWN, (1 << 16) | 0xffff),
                Some(MouseButtonEvent {
                    button: "x1",
                    action: "down",
                })
            );
            assert_eq!(
                decode_mouse_message(WM_XBUTTONUP, 2 << 16),
                Some(MouseButtonEvent {
                    button: "x2",
                    action: "up",
                })
            );
        }

        #[test]
        fn preserves_an_unrecognized_x_button_identifier() {
            assert_eq!(
                decode_mouse_message(WM_XBUTTONDOWN, 7 << 16),
                Some(MouseButtonEvent {
                    button: "x-unknown",
                    action: "down",
                })
            );
        }

        #[test]
        fn ignores_non_button_mouse_messages() {
            assert_eq!(decode_mouse_message(0x020a, 120 << 16), None);
            assert_eq!(decode_mouse_message(0x020e, 120 << 16), None);
            assert_eq!(decode_mouse_message(0x0200, 0), None);
        }

        #[test]
        fn parses_multiple_hid_reports() {
            let payload = [2, 0, 0, 0, 3, 0, 0, 0, 1, 0x80, 1, 0, 1, 0x40];
            assert_eq!(
                parse_hid_payload(&payload),
                Some(HidPayload {
                    report_size: 2,
                    report_count: 3,
                    bytes: &payload[8..],
                })
            );
        }

        #[test]
        fn rejects_truncated_zero_and_overflowing_hid_payloads() {
            assert_eq!(parse_hid_payload(&[1, 2, 3]), None);
            assert_eq!(parse_hid_payload(&[0, 0, 0, 0, 1, 0, 0, 0]), None);
            assert_eq!(parse_hid_payload(&[1, 0, 0, 0, 0, 0, 0, 0]), None);
            assert_eq!(
                parse_hid_payload(&[0xff, 0xff, 0xff, 0xff, 2, 0, 0, 0]),
                None
            );
            assert_eq!(parse_hid_payload(&[4, 0, 0, 0, 1, 0, 0, 0, 1, 2]), None);
        }

        #[test]
        fn decodes_high_bit_and_active_low_custom_buttons() {
            let definitions = [
                HidButtonDefinition {
                    name: "paddle-4",
                    device_name_contains: "VID_1234",
                    report_id: Some(5),
                    byte_offset: 2,
                    bit_mask: 0x80,
                    active_low: false,
                },
                HidButtonDefinition {
                    name: "safety-release",
                    device_name_contains: "VID_1234",
                    report_id: Some(5),
                    byte_offset: 3,
                    bit_mask: 0x04,
                    active_low: true,
                },
            ];

            assert_eq!(
                decode_hid_buttons(&definitions, r"\\?\HID#VID_1234&PID_5678", &[5, 0, 0x80, 0]),
                vec![("paddle-4", true), ("safety-release", true)]
            );
        }

        #[test]
        fn custom_button_decoder_filters_device_report_id_and_short_reports() {
            let definitions = [HidButtonDefinition {
                name: "auxiliary",
                device_name_contains: "VID_ABCD&PID_0001",
                report_id: Some(9),
                byte_offset: 4,
                bit_mask: 0x02,
                active_low: false,
            }];

            assert!(decode_hid_buttons(&definitions, "VID_OTHER", &[9, 0, 0, 0, 2]).is_empty());
            assert!(
                decode_hid_buttons(&definitions, "VID_ABCD&PID_0001", &[8, 0, 0, 0, 2]).is_empty()
            );
            assert!(decode_hid_buttons(&definitions, "VID_ABCD&PID_0001", &[9, 0]).is_empty());
        }
    }
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = windows_app::run() {
        eprintln!("PressNots failed: {error}");
        std::process::exit(1);
    }
}
