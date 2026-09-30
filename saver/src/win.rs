//! The small amount of Win32 the screensaver protocol needs.

#![allow(clippy::missing_safety_doc)]

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, RECT, BOOL};
use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// A monitor rectangle in virtual-screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub primary: bool,
}

unsafe extern "system" fn monitor_cb(hmon: HMONITOR, _hdc: HDC, _rect: *mut RECT, data: LPARAM) -> BOOL {
    let list = &mut *(data as *mut Vec<Monitor>);
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(hmon, &mut info) != 0 {
        let r = info.rcMonitor;
        list.push(Monitor {
            x: r.left,
            y: r.top,
            w: r.right - r.left,
            h: r.bottom - r.top,
            primary: info.dwFlags & 1 != 0, // MONITORINFOF_PRIMARY
        });
    }
    1
}

/// Every attached monitor, primary first.
pub fn monitors() -> Vec<Monitor> {
    let mut list: Vec<Monitor> = Vec::new();
    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(monitor_cb), &mut list as *mut _ as LPARAM);
    }
    list.sort_by_key(|m| (!m.primary, m.x, m.y));
    list
}

/// The bounding box of all monitors: `(x, y, w, h)`.
pub fn virtual_screen(mons: &[Monitor]) -> (i32, i32, i32, i32) {
    if mons.is_empty() {
        return (0, 0, 1920, 1080);
    }
    let x0 = mons.iter().map(|m| m.x).min().unwrap();
    let y0 = mons.iter().map(|m| m.y).min().unwrap();
    let x1 = mons.iter().map(|m| m.x + m.w).max().unwrap();
    let y1 = mons.iter().map(|m| m.y + m.h).max().unwrap();
    (x0, y0, x1 - x0, y1 - y0)
}

pub fn cursor_pos() -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    (p.x, p.y)
}

/// Every virtual key currently held, as a bitset. Mouse buttons included.
pub fn keys_down() -> [bool; 256] {
    let mut down = [false; 256];
    for vk in 1..255 {
        let s = unsafe { GetAsyncKeyState(vk) } as u16;
        down[vk as usize] = s & 0x8000 != 0;
    }
    down
}

/// The Win32 handle behind a winit window.
pub fn hwnd_of(window: &winit::window::Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
        _ => None,
    }
}

pub fn parse_hwnd(s: &str) -> Option<HWND> {
    let s = s.trim();
    let n: isize = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        isize::from_str_radix(hex, 16).ok()?
    } else {
        s.parse().ok()?
    };
    (n != 0).then_some(n as HWND)
}

pub fn is_window(h: HWND) -> bool {
    unsafe { IsWindow(h) != 0 }
}

/// Size of a window's client area.
pub fn client_size(h: HWND) -> (i32, i32) {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(h, &mut r) };
    (r.right - r.left, r.bottom - r.top)
}

/// Turn `child` into a borderless child window filling `parent`'s client
/// area: the Control Panel preview box. Returns false, and leaves `child`
/// alone (still hidden), if Windows refuses the new parent.
pub fn adopt_into(child: HWND, parent: HWND, show: bool) -> bool {
    unsafe {
        if GetParent(child) != parent && SetParent(child, parent).is_null() {
            return false;
        }
        let mut style = WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN;
        if show {
            style |= WS_VISIBLE;
        }
        SetWindowLongPtrW(child, GWL_STYLE, style as isize);
        SetWindowLongPtrW(child, GWL_EXSTYLE, 0);
        let (w, h) = client_size(parent);
        SetWindowPos(child, std::ptr::null_mut(), 0, 0, w.max(1), h.max(1), SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }
    true
}

/// For `--help` and friends: this is a GUI-subsystem program, so borrow the
/// console of whoever started it, if there is one.
pub fn attach_parent_console() {
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn message_box(owner: HWND, text: &str, title: &str, error: bool) {
    let (t, c) = (wide(text), wide(title));
    let icon = if error { MB_ICONERROR } else { MB_ICONINFORMATION };
    unsafe { MessageBoxW(owner, t.as_ptr(), c.as_ptr(), MB_OK | icon) };
}

/// Hidden test parent for the preview path: a plain popup window the size of
/// the Control Panel preview box, never shown.
pub fn hidden_test_parent(w: i32, h: i32) -> HWND {
    unsafe {
        let class = wide("Static");
        let title = wide("proof-saver preview test");
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    }
}
