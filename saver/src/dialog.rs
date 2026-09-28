//! The settings dialog (`/c`, or no arguments): plain Win32 controls.

use crate::scene::SCENES;
use crate::settings::{OtherMonitors, Settings};
use crate::win::wide;
use std::cell::RefCell;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::Xps::PrintWindow;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const ID_SCENE: i32 = 101;
const ID_SECONDS: i32 = 102;
const ID_SPEED: i32 = 103;
const ID_FPS: i32 = 104;
const ID_MONITORS: i32 = 105;
const ID_CAPTION: i32 = 106;
const ID_TRY: i32 = 107;
const ID_OK: i32 = 1; // IDOK
const ID_CANCEL: i32 = 2; // IDCANCEL

const SECONDS: &[(u32, &str)] = &[(20, "20 seconds"), (45, "45 seconds"), (90, "90 seconds"), (180, "3 minutes"), (600, "10 minutes")];
const SPEEDS: &[(f32, &str)] = &[(0.5, "Slow"), (1.0, "Normal"), (1.6, "Fast")];
const FPS: &[(u32, &str)] = &[(24, "24 fps (lightest)"), (30, "30 fps"), (60, "60 fps")];
const MONITORS: &[(OtherMonitors, &str)] = &[
    (OtherMonitors::Different, "Each shows its own attractor"),
    (OtherMonitors::Same, "All show the same attractor"),
    (OtherMonitors::Black, "Stay black"),
];

struct State {
    settings: Settings,
    controls: Vec<(i32, HWND)>,
    saved: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn ctl(id: i32) -> HWND {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .and_then(|st| st.controls.iter().find(|(i, _)| *i == id).map(|(_, h)| *h))
            .unwrap_or(std::ptr::null_mut())
    })
}

fn names<T>(o: &'static [(T, &'static str)]) -> Vec<&'static str> {
    o.iter().map(|(_, n)| *n).collect()
}

fn nearest<T: Copy>(options: &[(T, &str)], dist: impl Fn(T) -> f32) -> usize {
    let mut best = 0;
    for (i, (v, _)) in options.iter().enumerate() {
        if dist(*v) < dist(options[best].0) {
            best = i;
        }
    }
    best
}

unsafe fn combo_sel(id: i32) -> usize {
    let r = SendMessageW(ctl(id), CB_GETCURSEL, 0, 0);
    if r < 0 { 0 } else { r as usize }
}

/// Read the controls back into a Settings.
unsafe fn read_controls(base: &Settings) -> Settings {
    let mut s = base.clone();
    let scene = combo_sel(ID_SCENE);
    s.scene = if scene == 0 { "cycle".into() } else { SCENES[scene - 1].key.into() };
    s.scene_seconds = SECONDS[combo_sel(ID_SECONDS).min(SECONDS.len() - 1)].0;
    s.speed = SPEEDS[combo_sel(ID_SPEED).min(SPEEDS.len() - 1)].0;
    s.fps_cap = FPS[combo_sel(ID_FPS).min(FPS.len() - 1)].0;
    s.other_monitors = MONITORS[combo_sel(ID_MONITORS).min(MONITORS.len() - 1)].0;
    s.show_equations = SendMessageW(ctl(ID_CAPTION), BM_GETCHECK, 0, 0) == 1;
    s.sanitize();
    s
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wp & 0xffff) as i32;
            match id {
                ID_OK => {
                    let base = STATE.with(|s| s.borrow().as_ref().map(|st| st.settings.clone())).unwrap_or_default();
                    let new = read_controls(&base);
                    match new.save() {
                        Ok(_) => {
                            STATE.with(|s| {
                                if let Some(st) = s.borrow_mut().as_mut() {
                                    st.settings = new;
                                    st.saved = true;
                                }
                            });
                            DestroyWindow(hwnd);
                        }
                        Err(e) => crate::win::message_box(hwnd, &format!("Could not save settings:\n{e}"), "Proof Saver", true),
                    }
                    0
                }
                ID_CANCEL => {
                    DestroyWindow(hwnd);
                    0
                }
                ID_TRY => {
                    // Save first, then open the live-art window with them.
                    let base = STATE.with(|s| s.borrow().as_ref().map(|st| st.settings.clone())).unwrap_or_default();
                    let new = read_controls(&base);
                    let _ = new.save();
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).arg("--window").spawn();
                    }
                    0
                }
                _ => DefWindowProcW(hwnd, msg, wp, lp),
            }
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Build the dialog. With `visible` false it is laid out but never shown,
/// which is how `--settings-snapshot` draws it for a check.
unsafe fn build(owner: HWND, visible: bool) -> HWND {
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    let hinst = GetModuleHandleW(std::ptr::null());
    let class = wide("ProofSaverSettings");
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: LoadIconW(std::ptr::null_mut(), IDI_APPLICATION),
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
        hbrBackground: (COLOR_BTNFACE + 1) as usize as HBRUSH,
        lpszMenuName: std::ptr::null(),
        lpszClassName: class.as_ptr(),
    };
    RegisterClassW(&wc);

    let title = wide("Proof Saver settings");
    let style = WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_CLIPCHILDREN;
    let hwnd = CreateWindowExW(
        WS_EX_DLGMODALFRAME,
        class.as_ptr(),
        title.as_ptr(),
        style,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        480,
        400,
        owner,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    // Everything is laid out in 96-dpi units and scaled.
    let dpi = GetDpiForWindow(hwnd).max(96) as i32;
    let px = |v: i32| v * dpi / 96;

    // The system message font, at this window's dpi.
    let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
    ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
    SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, ncm.cbSize, &mut ncm as *mut _ as *mut _, 0);
    let mut lf = ncm.lfMessageFont;
    lf.lfHeight = -(9 * dpi / 72);
    let font = CreateFontIndirectW(&lf);

    let mut controls: Vec<(i32, HWND)> = Vec::new();
    let mk = |class: &str, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: i32, controls: &mut Vec<(i32, HWND)>| -> HWND {
        let c = wide(class);
        let t = wide(text);
        let h = CreateWindowExW(
            0,
            c.as_ptr(),
            t.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            px(x),
            px(y),
            px(w),
            px(h),
            hwnd,
            id as isize as HMENU,
            hinst,
            std::ptr::null(),
        );
        SendMessageW(h, WM_SETFONT, font as WPARAM, 1);
        controls.push((id, h));
        h
    };

    let s = Settings::load();
    let (lx, cx, cw) = (16, 150, 290);
    let mut y = 16;
    let combo = |label: &str, id: i32, items: &[&str], sel: usize, y: i32, controls: &mut Vec<(i32, HWND)>| {
        mk("STATIC", label, 0, lx, y + 4, cx - lx - 8, 20, 0, controls);
        let c = mk("COMBOBOX", "", CBS_DROPDOWNLIST as u32 | WS_TABSTOP | WS_VSCROLL, cx, y, cw, 300, id, controls);
        for it in items {
            let w = wide(it);
            SendMessageW(c, CB_ADDSTRING, 0, w.as_ptr() as LPARAM);
        }
        SendMessageW(c, CB_SETCURSEL, sel, 0);
    };

    let mut scene_items: Vec<String> = vec!["Cycle through all ten".into()];
    scene_items.extend(SCENES.iter().map(|d| format!("{} only", if d.key == "rossler" { "Rossler" } else { d.name })));
    let scene_refs: Vec<&str> = scene_items.iter().map(|s| s.as_str()).collect();
    let scene_sel = if s.scene == "cycle" { 0 } else { crate::scene::find(&s.scene).map(|i| i + 1).unwrap_or(0) };
    combo("Attractor", ID_SCENE, &scene_refs, scene_sel, y, &mut controls);
    y += 36;
    combo("Time on each", ID_SECONDS, &names(SECONDS), nearest(SECONDS, |v| (v as f32 - s.scene_seconds as f32).abs()), y, &mut controls);
    y += 36;
    let speed_names: Vec<&str> = SPEEDS.iter().map(|(_, n)| *n).collect();
    combo("Motion speed", ID_SPEED, &speed_names, nearest(SPEEDS, |v| (v - s.speed).abs()), y, &mut controls);
    y += 36;
    combo("Frame rate cap", ID_FPS, &names(FPS), nearest(FPS, |v| (v as f32 - s.fps_cap as f32).abs()), y, &mut controls);
    y += 36;
    let mon_names: Vec<&str> = MONITORS.iter().map(|(_, n)| *n).collect();
    let mon_sel = MONITORS.iter().position(|(m, _)| *m == s.other_monitors).unwrap_or(0);
    combo("Other monitors", ID_MONITORS, &mon_names, mon_sel, y, &mut controls);
    y += 40;
    let cb = mk("BUTTON", "Show each attractor's equations for a few seconds", BS_AUTOCHECKBOX as u32 | WS_TABSTOP, lx, y, 420, 22, ID_CAPTION, &mut controls);
    SendMessageW(cb, BM_SETCHECK, if s.show_equations { 1 } else { 0 }, 0);
    y += 34;
    let path = Settings::path();
    mk("STATIC", &format!("Saved to {}", path.display()), 0x8000u32 /* SS_PATHELLIPSIS */, lx, y, 424, 20, 0, &mut controls);
    y += 20;
    mk("STATIC", "Proof Engine, MIT. github.com/Mattbusel/proof-engine", 0, lx, y, 424, 20, 0, &mut controls);
    y += 36;
    mk("BUTTON", "Try it in a window", WS_TABSTOP, lx, y, 150, 30, ID_TRY, &mut controls);
    mk("BUTTON", "OK", BS_DEFPUSHBUTTON as u32 | WS_TABSTOP, 250, y, 90, 30, ID_OK, &mut controls);
    mk("BUTTON", "Cancel", WS_TABSTOP, 350, y, 90, 30, ID_CANCEL, &mut controls);
    y += 30 + 16;

    // Size the window around the client area.
    let mut r = RECT { left: 0, top: 0, right: px(456), bottom: px(y) };
    AdjustWindowRectExForDpi(&mut r, style, 0, WS_EX_DLGMODALFRAME, dpi as u32);
    let (ww, wh) = (r.right - r.left, r.bottom - r.top);
    // Centre on the owner if there is one, else on the primary monitor.
    let (ox, oy, ow, oh) = if !owner.is_null() {
        let mut orc: RECT = std::mem::zeroed();
        GetWindowRect(owner, &mut orc);
        (orc.left, orc.top, orc.right - orc.left, orc.bottom - orc.top)
    } else {
        (0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
    };
    SetWindowPos(hwnd, std::ptr::null_mut(), ox + (ow - ww) / 2, oy + (oh - wh) / 2, ww, wh, SWP_NOZORDER | SWP_NOACTIVATE);

    STATE.with(|st| *st.borrow_mut() = Some(State { settings: s, controls, saved: false }));
    if visible {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(ctl(ID_SCENE));
    }
    hwnd
}

/// Show the dialog and block until it closes. Returns true if saved.
pub fn run(owner: HWND) -> bool {
    unsafe {
        if !owner.is_null() {
            EnableWindow(owner, 0);
        }
        let hwnd = build(owner, true);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if !owner.is_null() {
            EnableWindow(owner, 1);
            SetForegroundWindow(owner);
        }
    }
    STATE.with(|s| s.borrow().as_ref().map(|st| st.saved).unwrap_or(false))
}

/// Lay the dialog out without showing it and write a picture of it to a
/// 24-bit BMP, so the layout can be checked on a machine someone is using.
pub fn snapshot(path: &str) -> std::io::Result<()> {
    unsafe {
        let hwnd = build(std::ptr::null_mut(), false);
        let mut r: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut r);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let screen = GetDC(std::ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp);
        // PW_RENDERFULLCONTENT
        PrintWindow(hwnd, mem, 2);
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = h;
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 24;
        let stride = ((w * 3 + 3) & !3) as usize;
        let mut pixels = vec![0u8; stride * h as usize];
        GetDIBits(mem, bmp, 0, h as u32, pixels.as_mut_ptr() as *mut _, &mut bi, DIB_RGB_COLORS);
        SelectObject(mem, old);
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        DestroyWindow(hwnd);

        let mut out = Vec::with_capacity(54 + pixels.len());
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&((54 + pixels.len()) as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&54u32.to_le_bytes());
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&24u16.to_le_bytes());
        for _ in 0..6 {
            out.extend_from_slice(&0u32.to_le_bytes());
        }
        out.extend_from_slice(&pixels);
        std::fs::write(path, out)
    }
}
