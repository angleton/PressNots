use std::ffi::c_void;

pub(crate) type Bool = i32;
pub(crate) type Dword = u32;
pub(crate) type Handle = *mut c_void;
pub(crate) type Hhook = Handle;
pub(crate) type Hinstance = Handle;
pub(crate) type Hrawinput = Handle;
pub(crate) type Hwnd = Handle;
pub(crate) type Lparam = isize;
pub(crate) type Lresult = isize;
pub(crate) type Uint = u32;
pub(crate) type Wparam = usize;

pub(crate) const WH_MOUSE_LL: i32 = 14;
pub(crate) const WM_DESTROY: Uint = 0x0002;
pub(crate) const WM_PAINT: Uint = 0x000f;
pub(crate) const WM_CONTEXTMENU: Uint = 0x007b;
pub(crate) const WM_INPUT: Uint = 0x00ff;
pub(crate) const WM_QUIT: Uint = 0x0012;
pub(crate) const WM_KEYDOWN: Uint = 0x0100;
pub(crate) const WM_LBUTTONDOWN: Uint = 0x0201;
pub(crate) const WM_APP_UPDATE_UI: Uint = 0x8001;
pub(crate) const RID_INPUT: Uint = 0x10000003;
pub(crate) const RIDI_DEVICENAME: Uint = 0x20000007;
pub(crate) const RIDI_DEVICEINFO: Uint = 0x2000000b;
pub(crate) const RIM_TYPEMOUSE: Dword = 0;
pub(crate) const RIM_TYPEKEYBOARD: Dword = 1;
pub(crate) const RIM_TYPEHID: Dword = 2;
pub(crate) const RIDEV_INPUTSINK: Dword = 0x00000100;
pub(crate) const ERROR_VALUE: Uint = Uint::MAX;
pub(crate) const WS_OVERLAPPEDWINDOW: Dword = 0x00cf0000;
pub(crate) const WS_CHILD: Dword = 0x40000000;
pub(crate) const WS_VISIBLE: Dword = 0x10000000;
pub(crate) const WS_BORDER: Dword = 0x00800000;
pub(crate) const ES_AUTOHSCROLL: Dword = 0x0080;
pub(crate) const EM_SETSEL: Uint = 0x00b1;
pub(crate) const VK_RETURN: Wparam = 0x0d;
pub(crate) const VK_ESCAPE: Wparam = 0x1b;
pub(crate) const MF_STRING: Uint = 0x0000;
pub(crate) const MF_GRAYED: Uint = 0x0001;
pub(crate) const TPM_RIGHTBUTTON: Uint = 0x0002;
pub(crate) const TPM_RETURNCMD: Uint = 0x0100;
pub(crate) const CW_USEDEFAULT: i32 = i32::MIN;
pub(crate) const SW_SHOW: i32 = 5;
pub(crate) const DT_LEFT: Uint = 0x0000;
pub(crate) const DT_CENTER: Uint = 0x0001;
pub(crate) const DT_VCENTER: Uint = 0x0004;
pub(crate) const DT_SINGLELINE: Uint = 0x0020;
pub(crate) const DT_END_ELLIPSIS: Uint = 0x8000;
pub(crate) const TRANSPARENT: i32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Point {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

#[repr(C)]
pub(crate) struct Msg {
    pub(crate) hwnd: Hwnd,
    pub(crate) message: Uint,
    pub(crate) w_param: Wparam,
    pub(crate) l_param: Lparam,
    pub(crate) time: Dword,
    pub(crate) pt: Point,
    pub(crate) l_private: Dword,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Rect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

#[repr(C)]
pub(crate) struct PaintStruct {
    pub(crate) dc: Handle,
    pub(crate) erase: Bool,
    pub(crate) paint: Rect,
    pub(crate) restore: Bool,
    pub(crate) incremental_update: Bool,
    pub(crate) reserved: [u8; 32],
}

pub(crate) type WndProc = Option<unsafe extern "system" fn(Hwnd, Uint, Wparam, Lparam) -> Lresult>;
pub(crate) type HookProc = Option<unsafe extern "system" fn(i32, Wparam, Lparam) -> Lresult>;
pub(crate) type SubclassProc =
    Option<unsafe extern "system" fn(Hwnd, Uint, Wparam, Lparam, usize, usize) -> Lresult>;

#[repr(C)]
pub(crate) struct WndClassW {
    pub(crate) style: Uint,
    pub(crate) wnd_proc: WndProc,
    pub(crate) cls_extra: i32,
    pub(crate) wnd_extra: i32,
    pub(crate) instance: Hinstance,
    pub(crate) icon: Handle,
    pub(crate) cursor: Handle,
    pub(crate) background: Handle,
    pub(crate) menu_name: *const u16,
    pub(crate) class_name: *const u16,
}

#[repr(C)]
pub(crate) struct MsllHookStruct {
    pub(crate) pt: Point,
    pub(crate) mouse_data: Dword,
    pub(crate) flags: Dword,
    pub(crate) time: Dword,
    pub(crate) extra_info: usize,
}

#[repr(C)]
pub(crate) struct RawInputDeviceList {
    pub(crate) device: Handle,
    pub(crate) kind: Dword,
}

#[repr(C)]
pub(crate) struct RawInputDevice {
    pub(crate) usage_page: u16,
    pub(crate) usage: u16,
    pub(crate) flags: Dword,
    pub(crate) target: Hwnd,
}

#[repr(C)]
pub(crate) struct RawInputHeader {
    pub(crate) kind: Dword,
    pub(crate) size: Dword,
    pub(crate) device: Handle,
    pub(crate) w_param: Wparam,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct RidDeviceInfoHid {
    pub(crate) vendor_id: Dword,
    pub(crate) product_id: Dword,
    pub(crate) version_number: Dword,
    pub(crate) usage_page: u16,
    pub(crate) usage: u16,
}

#[repr(C)]
pub(crate) union RidDeviceInfoData {
    pub(crate) hid: RidDeviceInfoHid,
    pub(crate) padding: [Dword; 6],
}

#[repr(C)]
pub(crate) struct RidDeviceInfo {
    pub(crate) size: Dword,
    pub(crate) kind: Dword,
    pub(crate) data: RidDeviceInfoData,
}

#[link(name = "user32")]
unsafe extern "system" {
    pub(crate) fn CallNextHookEx(
        hook: Hhook,
        code: i32,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult;
    pub(crate) fn CreateWindowExW(
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
    pub(crate) fn AppendMenuW(menu: Handle, flags: Uint, item_id: usize, text: *const u16) -> Bool;
    pub(crate) fn CreatePopupMenu() -> Handle;
    pub(crate) fn DestroyMenu(menu: Handle) -> Bool;
    pub(crate) fn DestroyWindow(hwnd: Hwnd) -> Bool;
    pub(crate) fn DefWindowProcW(
        hwnd: Hwnd,
        message: Uint,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult;
    pub(crate) fn DispatchMessageW(message: *const Msg) -> Lresult;
    pub(crate) fn DrawTextW(
        dc: Handle,
        text: *const u16,
        count: i32,
        rect: *mut Rect,
        format: Uint,
    ) -> i32;
    pub(crate) fn FillRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
    pub(crate) fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> Bool;
    pub(crate) fn GetCursorPos(point: *mut Point) -> Bool;
    pub(crate) fn GetKeyNameTextW(l_param: i32, text: *mut u16, size: i32) -> i32;
    pub(crate) fn GetMessageW(message: *mut Msg, hwnd: Hwnd, min: Uint, max: Uint) -> Bool;
    pub(crate) fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    pub(crate) fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max_count: i32) -> i32;
    pub(crate) fn GetRawInputData(
        input: Hrawinput,
        command: Uint,
        data: *mut c_void,
        size: *mut Uint,
        header_size: Uint,
    ) -> Uint;
    pub(crate) fn GetRawInputDeviceInfoW(
        device: Handle,
        command: Uint,
        data: *mut c_void,
        size: *mut Uint,
    ) -> Uint;
    pub(crate) fn GetRawInputDeviceList(
        list: *mut RawInputDeviceList,
        count: *mut Uint,
        size: Uint,
    ) -> Uint;
    pub(crate) fn InvalidateRect(hwnd: Hwnd, rect: *const Rect, erase: Bool) -> Bool;
    pub(crate) fn PostMessageW(hwnd: Hwnd, message: Uint, w_param: Wparam, l_param: Lparam)
    -> Bool;
    pub(crate) fn PostQuitMessage(exit_code: i32);
    pub(crate) fn RegisterClassW(class: *const WndClassW) -> u16;
    pub(crate) fn RegisterRawInputDevices(
        devices: *const RawInputDevice,
        count: Uint,
        size: Uint,
    ) -> Bool;
    pub(crate) fn ScreenToClient(hwnd: Hwnd, point: *mut Point) -> Bool;
    pub(crate) fn SendMessageW(
        hwnd: Hwnd,
        message: Uint,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult;
    pub(crate) fn SetFocus(hwnd: Hwnd) -> Hwnd;
    pub(crate) fn SetWindowsHookExW(
        kind: i32,
        proc: HookProc,
        module: Hinstance,
        thread_id: Dword,
    ) -> Hhook;
    pub(crate) fn SetWindowTextW(hwnd: Hwnd, text: *const u16) -> Bool;
    pub(crate) fn ShowWindow(hwnd: Hwnd, command: i32) -> Bool;
    pub(crate) fn TranslateMessage(message: *const Msg) -> Bool;
    pub(crate) fn TrackPopupMenu(
        menu: Handle,
        flags: Uint,
        x: i32,
        y: i32,
        reserved: i32,
        hwnd: Hwnd,
        rect: *const Rect,
    ) -> Uint;
    pub(crate) fn UnhookWindowsHookEx(hook: Hhook) -> Bool;
    pub(crate) fn UpdateWindow(hwnd: Hwnd) -> Bool;
    pub(crate) fn BeginPaint(hwnd: Hwnd, paint: *mut PaintStruct) -> Handle;
    pub(crate) fn EndPaint(hwnd: Hwnd, paint: *const PaintStruct) -> Bool;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    pub(crate) fn DefSubclassProc(
        hwnd: Hwnd,
        message: Uint,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult;
    pub(crate) fn RemoveWindowSubclass(hwnd: Hwnd, proc: SubclassProc, subclass_id: usize) -> Bool;
    pub(crate) fn SetWindowSubclass(
        hwnd: Hwnd,
        proc: SubclassProc,
        subclass_id: usize,
        reference_data: usize,
    ) -> Bool;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    pub(crate) fn CreateSolidBrush(color: Dword) -> Handle;
    pub(crate) fn DeleteObject(object: Handle) -> Bool;
    pub(crate) fn Ellipse(dc: Handle, left: i32, top: i32, right: i32, bottom: i32) -> Bool;
    pub(crate) fn SelectObject(dc: Handle, object: Handle) -> Handle;
    pub(crate) fn SetBkMode(dc: Handle, mode: i32) -> i32;
    pub(crate) fn SetTextColor(dc: Handle, color: Dword) -> Dword;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub(crate) fn GetLastError() -> Dword;
    pub(crate) fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
}
