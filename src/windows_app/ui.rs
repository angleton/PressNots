use super::process_raw_input;
use super::state::UiState;
use super::win32::*;
use std::mem::zeroed;
use std::ptr::null;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

static STATE: OnceLock<Mutex<UiState>> = OnceLock::new();
static WINDOW: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn initialize() -> Result<(), String> {
    STATE
        .set(Mutex::new(UiState::default()))
        .map_err(|_| "UI state was already initialized".into())
}

pub(crate) fn set_window(hwnd: Hwnd) {
    WINDOW.store(hwnd as usize, Ordering::Release);
}

pub(crate) fn register_device(handle: usize, label: String) {
    if let Some(state) = STATE.get()
        && let Ok(mut state) = state.lock()
    {
        state.register_device(handle, label);
    }
}

pub(crate) fn update_global(key: String, button: &str, pressed: bool) {
    if let Some(state) = STATE.get()
        && let Ok(mut state) = state.lock()
    {
        state.record(key, button, pressed);
    }
    request_redraw();
}

pub(crate) fn update_device(device: usize, button: &str, pressed: bool) {
    if let Some(state) = STATE.get()
        && let Ok(mut state) = state.lock()
    {
        state.record_device(device, button, pressed);
    }
    request_redraw();
}

fn request_redraw() {
    let hwnd = WINDOW.load(Ordering::Acquire) as Hwnd;
    if !hwnd.is_null() {
        unsafe { PostMessageW(hwnd, WM_APP_UPDATE_UI, 0, 0) };
    }
}

pub(crate) unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match message {
        WM_INPUT => {
            unsafe { process_raw_input(l_param as Hrawinput) };
            return unsafe { DefWindowProcW(hwnd, message, w_param, l_param) };
        }
        WM_APP_UPDATE_UI => unsafe { refresh_window(hwnd) },
        WM_PAINT => unsafe { paint_window(hwnd) },
        WM_DESTROY => {
            WINDOW.store(0, Ordering::Release);
            unsafe { PostQuitMessage(0) };
        }
        _ => return unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
    }
    0
}

unsafe fn refresh_window(hwnd: Hwnd) {
    let last_button = STATE
        .get()
        .and_then(|state| state.lock().ok())
        .map(|state| state.last_button.clone())
        .unwrap_or_else(|| "Waiting for a button press".into());
    let title = wide_null(&format!("PressNots - {last_button}"));
    unsafe {
        SetWindowTextW(hwnd, title.as_ptr());
        InvalidateRect(hwnd, null(), 0);
    }
}

unsafe fn paint_window(hwnd: Hwnd) {
    let mut paint: PaintStruct = unsafe { zeroed() };
    let dc = unsafe { BeginPaint(hwnd, &mut paint) };
    if dc.is_null() {
        return;
    }

    let mut client: Rect = unsafe { zeroed() };
    unsafe { GetClientRect(hwnd, &mut client) };
    let (light_is_on, last_button, devices) = STATE
        .get()
        .and_then(|state| state.lock().ok())
        .map(|state| {
            (
                state.light_is_on(),
                state.last_button.clone(),
                state.devices.clone(),
            )
        })
        .unwrap_or_else(|| (false, "Waiting for a button press".into(), Vec::new()));

    let background = unsafe { CreateSolidBrush(rgb(24, 27, 32)) };
    let status_brush = unsafe { CreateSolidBrush(rgb(39, 44, 52)) };
    let device_brush = unsafe { CreateSolidBrush(rgb(31, 35, 42)) };
    let device_active_brush = unsafe { CreateSolidBrush(rgb(255, 48, 48)) };
    let device_inactive_brush = unsafe { CreateSolidBrush(rgb(75, 33, 36)) };
    let light_brush = unsafe {
        CreateSolidBrush(if light_is_on {
            rgb(255, 32, 32)
        } else {
            rgb(84, 28, 31)
        })
    };
    unsafe { FillRect(dc, &client, background) };

    let status_height = 58;
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, rgb(245, 247, 250));
    }
    let mut heading = Rect {
        left: 20,
        top: 10,
        right: client.right - 20,
        bottom: 38,
    };
    let heading_text = wide_null("Detected input devices");
    unsafe {
        DrawTextW(
            dc,
            heading_text.as_ptr(),
            -1,
            &mut heading,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }

    let row_height = 28;
    let rows_top = 40;
    for (index, device) in devices.iter().enumerate() {
        let top = rows_top + index as i32 * row_height;
        let mut row = Rect {
            left: 20,
            top,
            right: client.right - 20,
            bottom: top + row_height - 3,
        };
        unsafe { FillRect(dc, &row, device_brush) };

        let lamp_brush = if device.pressed.is_empty() {
            device_inactive_brush
        } else {
            device_active_brush
        };
        let previous_brush = unsafe { SelectObject(dc, lamp_brush) };
        unsafe {
            Ellipse(dc, 28, top + 6, 42, top + 20);
            SelectObject(dc, previous_brush);
        }

        row.left = 52;
        row.right -= 8;
        let label = wide_null(&device.label);
        unsafe {
            DrawTextW(
                dc,
                label.as_ptr(),
                -1,
                &mut row,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
        }
    }

    let devices_bottom = rows_top + devices.len() as i32 * row_height;
    let light_top = devices_bottom + 14;
    let light_bottom = (client.bottom - status_height - 14).max(light_top);
    let available_height = (light_bottom - light_top).max(0);
    let diameter = (client.right - 160).min(available_height).clamp(0, 200);
    let left = (client.right - diameter) / 2;
    let top = light_top + (available_height - diameter) / 2;
    let previous_brush = unsafe { SelectObject(dc, light_brush) };
    unsafe {
        Ellipse(dc, left, top, left + diameter, top + diameter);
        SelectObject(dc, previous_brush);
    }

    let mut status = Rect {
        left: 0,
        top: (client.bottom - status_height).max(0),
        right: client.right,
        bottom: client.bottom,
    };
    unsafe { FillRect(dc, &status, status_brush) };
    let status_text = if light_is_on {
        format!("Pressed: {last_button}")
    } else {
        format!("Last button: {last_button}")
    };
    let status_text = wide_null(&status_text);
    unsafe {
        DrawTextW(
            dc,
            status_text.as_ptr(),
            -1,
            &mut status,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        DeleteObject(light_brush);
        DeleteObject(device_inactive_brush);
        DeleteObject(device_active_brush);
        DeleteObject(device_brush);
        DeleteObject(status_brush);
        DeleteObject(background);
        EndPaint(hwnd, &paint);
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> Dword {
    red as Dword | ((green as Dword) << 8) | ((blue as Dword) << 16)
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
