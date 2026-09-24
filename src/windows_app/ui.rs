use super::aliases::AliasStore;
use super::process_raw_input;
use super::state::{ResetAlias, UiState};
use super::win32::*;
use std::mem::zeroed;
use std::ptr::null;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

static STATE: OnceLock<Mutex<UiState>> = OnceLock::new();
static ALIASES: OnceLock<Mutex<AliasStore>> = OnceLock::new();
static WINDOW: AtomicUsize = AtomicUsize::new(0);
static EDIT_WINDOW: AtomicUsize = AtomicUsize::new(0);
static EDIT_TARGET: AtomicUsize = AtomicUsize::new(0);
static FINISHING_EDIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LAST_RESET: OnceLock<Mutex<Option<ResetAlias>>> = OnceLock::new();

const ROW_HEIGHT: i32 = 28;
const ROWS_TOP: i32 = 40;
const MENU_RESET: Uint = 1;
const MENU_UNDO_RESET: Uint = 2;

pub(crate) fn initialize() -> Result<(), String> {
    ALIASES
        .set(Mutex::new(AliasStore::load_default()))
        .map_err(|_| "alias store was already initialized")?;
    LAST_RESET
        .set(Mutex::new(None))
        .map_err(|_| "reset history was already initialized")?;
    STATE
        .set(Mutex::new(UiState::default()))
        .map_err(|_| "UI state was already initialized".into())
}

pub(crate) fn set_window(hwnd: Hwnd) {
    WINDOW.store(hwnd as usize, Ordering::Release);
}

pub(crate) fn register_device(handle: usize, key: String, default_label: String) {
    let alias = ALIASES
        .get()
        .and_then(|aliases| aliases.lock().ok())
        .and_then(|aliases| aliases.get(&key).map(str::to_owned));
    if let Some(state) = STATE.get()
        && let Ok(mut state) = state.lock()
    {
        state.register_device(handle, key, default_label, alias.as_deref());
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
        WM_CONTEXTMENU => unsafe { show_device_menu(hwnd, l_param) },
        WM_LBUTTONDOWN => {
            let existing = EDIT_WINDOW.load(Ordering::Acquire) as Hwnd;
            if !existing.is_null() {
                unsafe { finish_rename(existing, true) };
            } else {
                unsafe { begin_rename(hwnd, l_param) };
            }
            return unsafe { DefWindowProcW(hwnd, message, w_param, l_param) };
        }
        WM_DESTROY => {
            WINDOW.store(0, Ordering::Release);
            unsafe { PostQuitMessage(0) };
        }
        _ => return unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
    }
    0
}

unsafe fn show_device_menu(hwnd: Hwnd, l_param: Lparam) {
    let edit = EDIT_WINDOW.load(Ordering::Acquire) as Hwnd;
    if !edit.is_null() {
        unsafe { finish_rename(edit, true) };
    }

    let screen = if l_param == -1 {
        let mut point: Point = unsafe { zeroed() };
        if unsafe { GetCursorPos(&mut point) } == 0 {
            return;
        }
        point
    } else {
        Point {
            x: (l_param as u16) as i16 as i32,
            y: ((l_param >> 16) as u16) as i16 as i32,
        }
    };
    let mut client = screen;
    if unsafe { ScreenToClient(hwnd, &mut client) } == 0 || client.y < ROWS_TOP {
        return;
    }
    let index = ((client.y - ROWS_TOP) / ROW_HEIGHT) as usize;
    let Some((handle, can_reset)) =
        STATE
            .get()
            .and_then(|state| state.lock().ok())
            .and_then(|state| {
                state
                    .devices
                    .get(index)
                    .map(|device| (device.handle, device.alias.is_some()))
            })
    else {
        return;
    };
    let can_undo = LAST_RESET
        .get()
        .and_then(|reset| reset.lock().ok())
        .is_some_and(|reset| reset.is_some());

    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return;
    }
    let reset_text = wide_null("Reset to default name");
    let undo_text = wide_null("Undo last reset");
    unsafe {
        AppendMenuW(
            menu,
            MF_STRING | if can_reset { 0 } else { MF_GRAYED },
            MENU_RESET as usize,
            reset_text.as_ptr(),
        );
        AppendMenuW(
            menu,
            MF_STRING | if can_undo { 0 } else { MF_GRAYED },
            MENU_UNDO_RESET as usize,
            undo_text.as_ptr(),
        );
    }

    let command = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            screen.x,
            screen.y,
            0,
            hwnd,
            null(),
        )
    };
    unsafe { DestroyMenu(menu) };
    match command {
        MENU_RESET => reset_device_alias(handle),
        MENU_UNDO_RESET => undo_last_reset(),
        _ => {}
    }
}

fn reset_device_alias(handle: usize) {
    let reset = STATE
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|mut state| state.reset_device(handle));
    let Some(reset) = reset else {
        return;
    };

    let result = ALIASES
        .get()
        .and_then(|aliases| aliases.lock().ok())
        .map_or_else(
            || Err(std::io::Error::other("alias store is unavailable")),
            |mut aliases| aliases.rename(&reset.key, ""),
        );
    if let Err(error) = result {
        if let Some(state) = STATE.get()
            && let Ok(mut state) = state.lock()
        {
            state.restore_alias(&reset);
        }
        eprintln!("Could not reset device alias: {error}");
        return;
    }
    if let Some(history) = LAST_RESET.get()
        && let Ok(mut history) = history.lock()
    {
        *history = Some(reset);
    }
    request_redraw();
}

fn undo_last_reset() {
    let reset = LAST_RESET
        .get()
        .and_then(|history| history.lock().ok())
        .and_then(|mut history| history.take());
    let Some(reset) = reset else {
        return;
    };

    let result = ALIASES
        .get()
        .and_then(|aliases| aliases.lock().ok())
        .map_or_else(
            || Err(std::io::Error::other("alias store is unavailable")),
            |mut aliases| aliases.rename(&reset.key, &reset.alias),
        );
    if let Err(error) = result {
        if let Some(history) = LAST_RESET.get()
            && let Ok(mut history) = history.lock()
        {
            *history = Some(reset);
        }
        eprintln!("Could not restore device alias: {error}");
        return;
    }
    if let Some(state) = STATE.get()
        && let Ok(mut state) = state.lock()
    {
        state.restore_alias(&reset);
    }
    request_redraw();
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
    let (has_pressed_inputs, last_button, devices) = STATE
        .get()
        .and_then(|state| state.lock().ok())
        .map(|state| {
            (
                state.has_pressed_inputs(),
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

    for (index, device) in devices.iter().enumerate() {
        let top = ROWS_TOP + index as i32 * ROW_HEIGHT;
        let mut row = Rect {
            left: 20,
            top,
            right: client.right - 20,
            bottom: top + ROW_HEIGHT - 3,
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

    let mut status = Rect {
        left: 0,
        top: (client.bottom - status_height).max(0),
        right: client.right,
        bottom: client.bottom,
    };
    unsafe { FillRect(dc, &status, status_brush) };
    let status_text = if has_pressed_inputs {
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
        DeleteObject(device_inactive_brush);
        DeleteObject(device_active_brush);
        DeleteObject(device_brush);
        DeleteObject(status_brush);
        DeleteObject(background);
        EndPaint(hwnd, &paint);
    }
}

unsafe fn begin_rename(hwnd: Hwnd, l_param: Lparam) {
    let y = ((l_param >> 16) as u16) as i16 as i32;
    if y < ROWS_TOP {
        return;
    }
    let index = ((y - ROWS_TOP) / ROW_HEIGHT) as usize;
    let Some((handle, label)) = STATE
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| {
            state
                .devices
                .get(index)
                .map(|device| (device.handle, device.label.clone()))
        })
    else {
        return;
    };

    let existing = EDIT_WINDOW.load(Ordering::Acquire) as Hwnd;
    if !existing.is_null() {
        unsafe { finish_rename(existing, true) };
    }

    let mut client: Rect = unsafe { zeroed() };
    unsafe { GetClientRect(hwnd, &mut client) };
    let edit_class = wide_null("EDIT");
    let text = wide_null(&label);
    let top = ROWS_TOP + index as i32 * ROW_HEIGHT;
    let edit = unsafe {
        CreateWindowExW(
            0,
            edit_class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
            50,
            top + 1,
            (client.right - 78).max(80),
            ROW_HEIGHT - 3,
            hwnd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            null(),
        )
    };
    if edit.is_null() {
        eprintln!(
            "Could not create device rename editor: Win32 error {}",
            unsafe { GetLastError() }
        );
        return;
    }

    EDIT_TARGET.store(handle, Ordering::Release);
    EDIT_WINDOW.store(edit as usize, Ordering::Release);
    if unsafe { SetWindowSubclass(edit, Some(edit_proc), 1, 0) } == 0 {
        eprintln!(
            "Could not subclass device rename editor: Win32 error {}",
            unsafe { GetLastError() }
        );
        unsafe { DestroyWindow(edit) };
        EDIT_WINDOW.store(0, Ordering::Release);
        EDIT_TARGET.store(0, Ordering::Release);
        return;
    }
    unsafe {
        SetFocus(edit);
        SendMessageW(edit, EM_SETSEL, 0, -1);
    }
}

unsafe extern "system" fn edit_proc(
    hwnd: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
    _subclass_id: usize,
    _reference_data: usize,
) -> Lresult {
    match message {
        WM_KEYDOWN if w_param == VK_RETURN => {
            unsafe { finish_rename(hwnd, true) };
            return 0;
        }
        WM_KEYDOWN if w_param == VK_ESCAPE => {
            unsafe { finish_rename(hwnd, false) };
            return 0;
        }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, message, w_param, l_param) }
}

unsafe fn finish_rename(hwnd: Hwnd, save: bool) {
    if FINISHING_EDIT.swap(true, Ordering::AcqRel) {
        return;
    }

    if save {
        let length = unsafe { GetWindowTextLengthW(hwnd) }.max(0) as usize;
        let mut text = vec![0u16; length + 1];
        let copied = unsafe { GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32) };
        let alias = String::from_utf16_lossy(&text[..copied.max(0) as usize]);
        let handle = EDIT_TARGET.load(Ordering::Acquire);
        let key = STATE
            .get()
            .and_then(|state| state.lock().ok())
            .and_then(|mut state| state.rename_device(handle, &alias));
        if let Some(key) = key
            && let Some(aliases) = ALIASES.get()
            && let Ok(mut aliases) = aliases.lock()
            && let Err(error) = aliases.rename(&key, &alias)
        {
            eprintln!("Could not save device alias: {error}");
        }
    }

    EDIT_WINDOW.store(0, Ordering::Release);
    EDIT_TARGET.store(0, Ordering::Release);
    unsafe {
        RemoveWindowSubclass(hwnd, Some(edit_proc), 1);
        DestroyWindow(hwnd);
    }
    FINISHING_EDIT.store(false, Ordering::Release);
    request_redraw();
}

const fn rgb(red: u8, green: u8, blue: u8) -> Dword {
    red as Dword | ((green as Dword) << 8) | ((blue as Dword) << 16)
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
