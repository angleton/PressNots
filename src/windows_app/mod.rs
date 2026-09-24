mod input;
mod state;
mod ui;
mod win32;

use input::hid::{CUSTOM_BUTTONS, decode_buttons, parse_payload};
use input::keyboard::{RawKeyboardEvent, decode as decode_keyboard, key_name};
use input::mouse::{decode_message as decode_mouse_message, decode_raw_buttons};
use std::collections::{BTreeSet, HashMap};
use std::ffi::OsString;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStringExt;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use ui::{
    initialize as initialize_ui, register_device as register_ui_device,
    set_window as set_ui_window, update_device as update_device_ui, update_global as update_ui,
    window_proc,
};
use win32::*;

static EVENT_TX: OnceLock<Sender<Event>> = OnceLock::new();
static BLOCK_MOUSE: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
enum Event {
    Mouse {
        button: &'static str,
        action: &'static str,
        x: i32,
        y: i32,
    },
    DeviceButton {
        device: usize,
        button: &'static str,
        pressed: bool,
    },
    Keyboard {
        device: usize,
        key: RawKeyboardEvent,
    },
    Hid {
        device: usize,
        report_size: u32,
        report_count: u32,
        bytes: Vec<u8>,
    },
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

unsafe fn process_raw_input(input: Hrawinput) {
    let mut byte_count = 0;
    let header_size = size_of::<RawInputHeader>() as Uint;
    if unsafe { GetRawInputData(input, RID_INPUT, null_mut(), &mut byte_count, header_size) } != 0
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
    if header.kind == RIM_TYPEMOUSE {
        if let Some(sender) = EVENT_TX.get() {
            for event in decode_raw_buttons(&buffer[header_size as usize..]) {
                let _ = sender.send(Event::DeviceButton {
                    device: header.device as usize,
                    button: event.button,
                    pressed: event.action == "down",
                });
            }
        }
        return;
    }
    if header.kind == RIM_TYPEKEYBOARD {
        if let Some(key) = decode_keyboard(&buffer[header_size as usize..])
            && let Some(sender) = EVENT_TX.get()
        {
            let _ = sender.send(Event::Keyboard {
                device: header.device as usize,
                key,
            });
        }
        return;
    }
    if header.kind != RIM_TYPEHID || buffer.len() < header_size as usize + 8 {
        return;
    }
    if let Some(payload) = parse_payload(&buffer[header_size as usize..])
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

    let mut usages = BTreeSet::from([(0x01u16, 0x02u16), (0x01u16, 0x06u16)]);
    for entry in &list {
        let kind = match entry.kind {
            RIM_TYPEMOUSE => "Mouse",
            RIM_TYPEKEYBOARD => "Keyboard",
            RIM_TYPEHID => "HID",
            _ => "Input device",
        };
        let name = unsafe { device_name(entry.device as usize) }
            .unwrap_or_else(|| format!("device 0x{:x}", entry.device as usize));
        register_ui_device(entry.device as usize, format!("{kind}  {name}"));
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
                update_ui(format!("mouse:{button}"), button, action == "down");
            }
            Event::DeviceButton {
                device,
                button,
                pressed,
            } => {
                let action = if pressed { "down" } else { "up" };
                println!("raw-mouse device=0x{device:x} button={button} action={action}");
                update_device_ui(device, button, pressed);
            }
            Event::Keyboard { device, key } => {
                let name = key_name(key);
                update_ui(
                    format!("keyboard:{device:x}:{}", key.identity()),
                    &name,
                    key.pressed,
                );
                update_device_ui(device, &key.identity(), key.pressed);
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
                    for (button, pressed) in decode_buttons(CUSTOM_BUTTONS, &name, report) {
                        let previous = button_states.insert((device, button), pressed);
                        if previous.is_some_and(|value| value != pressed)
                            || previous.is_none() && pressed
                        {
                            let action = if pressed { "down" } else { "up" };
                            println!(
                                "hid-button device=0x{device:x} button={button} action={action}"
                            );
                            update_ui(format!("hid:{device:x}:{button}"), button, pressed);
                            update_device_ui(device, button, pressed);
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

pub(crate) fn run() -> Result<(), String> {
    BLOCK_MOUSE.store(
        std::env::args().any(|arg| arg == "--block-mouse"),
        Ordering::Relaxed,
    );
    let (sender, receiver) = mpsc::channel();
    EVENT_TX
        .set(sender)
        .map_err(|_| "event channel was already initialized")?;
    initialize_ui()?;
    thread::spawn(move || print_events(receiver));

    let instance = unsafe { GetModuleHandleW(null()) };
    let class_name = wide_null("PressNotsWindow");
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
    let title = wide_null("PressNots - Waiting for a button press");
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            760,
            null_mut(),
            null_mut(),
            instance,
            null(),
        )
    };
    if hwnd.is_null() {
        return Err("CreateWindowExW failed".into());
    }
    set_ui_window(hwnd);
    unsafe { register_raw_devices(hwnd)? };

    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), instance, 0) };
    if hook.is_null() {
        return Err("SetWindowsHookExW failed".into());
    }
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
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
