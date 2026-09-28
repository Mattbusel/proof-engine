//! proof-saver: a Windows screensaver and live-art app built on Proof Engine.
//!
//! Windows runs a screensaver as `name.scr` with one of these arguments:
//!
//! | Argument | Meaning |
//! | --- | --- |
//! | `/s` | Run fullscreen until the mouse moves, a key is pressed or a button is clicked. |
//! | `/p <hwnd>` | Draw into the little preview box in the Screen Saver Settings dialog. |
//! | `/c` or `/c:<hwnd>` | Show the settings dialog. |
//! | none | Show the settings dialog (this is what double-clicking the .scr does). |
//!
//! The same program runs as live art with `--window` or `--fullscreen`, and
//! can write its frames to disk without showing anything with `--record`.

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod dialog;
mod scene;
mod settings;
mod win;

use proof_engine::prelude::*;
use scene::{Slot, SCENES};
use settings::{OtherMonitors, Settings};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::HWND;

const HELP: &str = "\
proof-saver: strange attractors from Proof Engine, as a screensaver or live art.

Screensaver protocol (what Windows passes to proof-saver.scr):
  /s                 run fullscreen on every monitor; any key, click or
                     mouse movement ends it
  /p <hwnd>          draw into the preview box of Screen Saver Settings
  /c[:<hwnd>]        settings dialog
  (no arguments)     settings dialog

Live art:
  --window           in a window. Esc quits, F11 toggles fullscreen,
                     Right arrow skips to the next attractor
  --fullscreen       the same, starting fullscreen; the mouse does not end it

Capture without showing a window (for making GIFs and videos):
  --record <dir>     write frames to <dir>/frame_0000.bmp, frame_0001.bmp, ...
    --frames <n>       how many frames (default 180)
    --fps <n>          simulated frames per second (default 30)
    --size <WxH>       frame size in pixels (default 1280x720)
    --skip <n>         frames to run before the first capture (default 60)
    --every <n>        capture every n-th frame (default 1)
  --preview-test <dir>   run the /p preview path inside a hidden 152x112
                     parent window and capture 30 frames from it
  --settings-snapshot <file.bmp>
                     draw the settings dialog without showing it
  --layout           print the monitors and the window /s would cover

Options for every mode:
  --scene <name>     cycle, or one of: lorenz aizawa thomas halvorsen rossler
                     dadras chen rabinovich sprott burke
  --seconds <n>      seconds on each attractor when cycling
  --speed <x>        simulated-time multiplier (0.25 to 3)
  --no-equations     never draw the name and equations

Settings file: %APPDATA%\\ProofSaver\\settings.toml
Source: https://github.com/Mattbusel/proof-engine (MIT)
";

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Saver,
    Preview(HWND),
    PreviewTest(String),
    Settings(HWND),
    Live { fullscreen: bool },
    Record(String),
    Snapshot(String),
    Layout,
    Help,
    Version,
}

#[derive(Debug, Clone)]
struct Opts {
    frames: u32,
    fps: u32,
    size: (u32, u32),
    skip: u32,
    every: u32,
}

impl Default for Opts {
    fn default() -> Self {
        Self { frames: 180, fps: 30, size: (1280, 720), skip: 60, every: 1 }
    }
}

fn parse(args: &[String], settings: &mut Settings, opts: &mut Opts) -> Result<Mode, String> {
    let mut mode: Option<Mode> = None;
    let mut i = 0;
    let next = |i: &mut usize, name: &str| -> Result<String, String> {
        *i += 1;
        args.get(*i).cloned().ok_or_else(|| format!("{name} needs a value"))
    };
    while i < args.len() {
        let raw = args[i].clone();
        let lower = raw.to_ascii_lowercase();
        // Windows' own switches: /s, /p 123, /p:123, /c, /c:123; '-' works too.
        let sw = if (lower.starts_with('/') || lower.starts_with('-')) && !lower.starts_with("--") {
            Some(lower[1..].to_string())
        } else {
            None
        };
        if let Some(sw) = sw {
            let (key, inline) = match sw.split_once(':') {
                Some((k, v)) => (k.to_string(), Some(v.to_string())),
                None => (sw.clone(), None),
            };
            match key.as_str() {
                "s" => mode = Some(Mode::Saver),
                "p" | "l" => {
                    let v = match inline {
                        Some(v) => v,
                        None => next(&mut i, "/p")?,
                    };
                    let h = win::parse_hwnd(&v).ok_or_else(|| format!("/p: '{v}' is not a window handle"))?;
                    mode = Some(Mode::Preview(h));
                }
                "c" => {
                    let owner = inline.and_then(|v| win::parse_hwnd(&v)).unwrap_or(std::ptr::null_mut());
                    mode = Some(Mode::Settings(owner));
                }
                "h" | "?" => mode = Some(Mode::Help),
                _ => return Err(format!("unknown switch {raw}")),
            }
            i += 1;
            continue;
        }
        match lower.as_str() {
            "--help" => mode = Some(Mode::Help),
            "--version" => mode = Some(Mode::Version),
            "--window" => mode = Some(Mode::Live { fullscreen: false }),
            "--fullscreen" => mode = Some(Mode::Live { fullscreen: true }),
            "--layout" => mode = Some(Mode::Layout),
            "--record" => mode = Some(Mode::Record(next(&mut i, "--record")?)),
            "--preview-test" => mode = Some(Mode::PreviewTest(next(&mut i, "--preview-test")?)),
            "--settings-snapshot" => mode = Some(Mode::Snapshot(next(&mut i, "--settings-snapshot")?)),
            "--scene" => {
                let v = next(&mut i, "--scene")?.to_ascii_lowercase();
                if v != "cycle" && scene::find(&v).is_none() {
                    return Err(format!("unknown scene '{v}'"));
                }
                settings.scene = v;
            }
            "--seconds" => settings.scene_seconds = next(&mut i, "--seconds")?.parse().map_err(|_| "--seconds needs a number")?,
            "--speed" => settings.speed = next(&mut i, "--speed")?.parse().map_err(|_| "--speed needs a number")?,
            "--no-equations" => settings.show_equations = false,
            "--frames" => opts.frames = next(&mut i, "--frames")?.parse().map_err(|_| "--frames needs a number")?,
            "--fps" => opts.fps = next(&mut i, "--fps")?.parse().map_err(|_| "--fps needs a number")?,
            "--skip" => opts.skip = next(&mut i, "--skip")?.parse().map_err(|_| "--skip needs a number")?,
            "--every" => opts.every = next(&mut i, "--every")?.parse().map_err(|_| "--every needs a number")?,
            "--size" => {
                let v = next(&mut i, "--size")?;
                let (w, h) = v
                    .to_ascii_lowercase()
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
                    .ok_or_else(|| format!("--size wants WxH, got '{v}'"))?;
                opts.size = (w, h);
            }
            _ => return Err(format!("unknown argument {raw}")),
        }
        i += 1;
    }
    settings.sanitize();
    // No arguments at all is the settings dialog, as Windows expects.
    Ok(mode.unwrap_or(Mode::Settings(std::ptr::null_mut())))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    install_panic_log();

    // Record mode starts from the defaults so captures are reproducible;
    // everything else starts from what the person saved.
    let recording = args.iter().any(|a| {
        let a = a.to_ascii_lowercase();
        a == "--record" || a == "--preview-test"
    });
    let mut settings = if recording { Settings::default() } else { Settings::load() };
    let mut opts = Opts::default();

    let mode = match parse(&args, &mut settings, &mut opts) {
        Ok(m) => m,
        Err(e) => {
            win::attach_parent_console();
            eprintln!("proof-saver: {e}\n\n{HELP}");
            std::process::exit(2);
        }
    };

    match mode {
        Mode::Help => {
            win::attach_parent_console();
            println!("{HELP}");
        }
        Mode::Version => {
            win::attach_parent_console();
            println!("proof-saver {} (proof-engine {})", env!("CARGO_PKG_VERSION"), "0.2.1");
        }
        Mode::Layout => {
            win::attach_parent_console();
            let mons = win::monitors();
            for (i, m) in mons.iter().enumerate() {
                println!("monitor {i}: {}x{} at ({}, {}){}", m.w, m.h, m.x, m.y, if m.primary { " primary" } else { "" });
            }
            let (x, y, w, h) = win::virtual_screen(&mons);
            if mons.len() == 1 {
                println!("/s: borderless fullscreen on the one monitor, {w}x{h}");
            } else {
                println!("/s: one borderless topmost window spanning {w}x{h} at ({x}, {y}), one attractor per monitor");
            }
        }
        Mode::Settings(owner) => {
            dialog::run(owner);
        }
        Mode::Snapshot(path) => {
            win::attach_parent_console();
            match dialog::snapshot(&path) {
                Ok(()) => println!("wrote {path}"),
                Err(e) => {
                    eprintln!("proof-saver: {e}");
                    std::process::exit(1);
                }
            }
        }
        Mode::Record(dir) => {
            win::attach_parent_console();
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("proof-saver: cannot create {dir}: {e}");
                std::process::exit(1);
            }
            std::env::set_var("PROOF_HIDDEN", "1");
            std::env::set_var("PROOF_FIXED_DT", opts.fps.max(1).to_string());
            std::env::set_var("PROOF_SHOT", format!("{}/frame_{{n}}.bmp", dir.trim_end_matches(['/', '\\'])));
            std::env::set_var("PROOF_SHOT_AT", opts.skip.to_string());
            std::env::set_var("PROOF_SHOT_COUNT", opts.frames.max(1).to_string());
            std::env::set_var("PROOF_SHOT_EVERY", opts.every.max(1).to_string());
            run(Mode::Record(dir.clone()), settings, opts.clone());
            println!("wrote {} frames to {dir}", opts.frames);
        }
        Mode::PreviewTest(dir) => {
            win::attach_parent_console();
            let _ = std::fs::create_dir_all(&dir);
            std::env::set_var("PROOF_HIDDEN", "1");
            std::env::set_var("PROOF_SHOT", format!("{}/preview_{{n}}.bmp", dir.trim_end_matches(['/', '\\'])));
            std::env::set_var("PROOF_SHOT_AT", "20");
            std::env::set_var("PROOF_SHOT_COUNT", "30");
            let parent = win::hidden_test_parent(152, 112);
            if parent.is_null() {
                eprintln!("proof-saver: could not create the hidden test parent");
                std::process::exit(1);
            }
            run(Mode::Preview(parent), settings, opts);
            println!("preview path ran; frames in {dir}");
        }
        m @ (Mode::Saver | Mode::Preview(_) | Mode::Live { .. }) => {
            // Created hidden in every case; each mode shows it once it is
            // placed, so nothing flashes at the wrong size or position.
            std::env::set_var("PROOF_HIDDEN", "1");
            run(m, settings, opts);
        }
    }
}

/// A screensaver has no console, so a crash would vanish without a trace.
fn install_panic_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let path = Settings::path().with_file_name("last-error.txt");
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, format!("{info}\n"));
        default(info);
    }));
}

/// Watches for the input that ends a screensaver.
struct Wake {
    armed_at: Instant,
    cursor: Option<(i32, i32)>,
    prev_keys: [bool; 256],
}

/// Pixels the mouse may drift before the saver ends. A desk bump or an
/// optical sensor's jitter should not wake the screen.
const WAKE_PIXELS: i32 = 12;

impl Wake {
    fn new() -> Self {
        Self { armed_at: Instant::now() + Duration::from_millis(900), cursor: None, prev_keys: win::keys_down() }
    }

    /// True when the person has come back.
    fn woke(&mut self) -> bool {
        let keys = win::keys_down();
        let now = Instant::now();
        if now < self.armed_at {
            self.prev_keys = keys;
            return false;
        }
        let pos = win::cursor_pos();
        let base = *self.cursor.get_or_insert(pos);
        let moved = (pos.0 - base.0).abs() > WAKE_PIXELS || (pos.1 - base.1).abs() > WAKE_PIXELS;
        let pressed = keys.iter().zip(self.prev_keys.iter()).any(|(now, before)| *now && !*before);
        self.prev_keys = keys;
        moved || pressed
    }
}

fn run(mode: Mode, settings: Settings, opts: Opts) {
    let preview = matches!(mode, Mode::Preview(_));
    let saver = mode == Mode::Saver;
    let recording = matches!(mode, Mode::Record(_));
    let live_full = matches!(mode, Mode::Live { fullscreen: true });

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Saver".to_string(),
        window_width: if recording { opts.size.0 } else { 1280 },
        window_height: if recording { opts.size.1 } else { 720 },
        audio: proof_engine::config::AudioConfig { enabled: false, ..Default::default() },
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.1,
            bloom_threshold: 0.35,
            tonemap: 1.0,
            exposure: 1.1,
            persistence: 0.55,
            vignette: 0.35,
            global_illumination: false,
            volumetric_fog: false,
            vsync: !recording,
            ..Default::default()
        },
        ..Default::default()
    });

    let order: Vec<usize> = if settings.scene == "cycle" {
        (0..SCENES.len()).collect()
    } else {
        vec![scene::find(&settings.scene).unwrap_or(0)]
    };
    let scene_len = settings.scene_seconds as f32;
    let speed = settings.speed;
    let fps_cap = if preview { 20 } else { settings.fps_cap };
    let mons = win::monitors();
    let (vx, vy, vw, vh) = win::virtual_screen(&mons);
    let spanning = saver && mons.len() > 1;

    let mut slots: Vec<Slot> = Vec::new();
    let mut frame: u64 = 0;
    let mut last_frame = Instant::now();
    let mut wake = Wake::new();
    let mut shown = false;
    let mut is_full = live_full;
    let mut last_size = (0u32, 0u32);
    let mut preview_size = (0, 0);

    engine.run_ui(move |engine, dt| {
        frame += 1;

        // Hold the frame-rate cap by sleeping, so a screensaver left running
        // all night does not keep a GPU at full tilt. Captures run flat out.
        if !recording {
            let target = Duration::from_secs_f64(1.0 / fps_cap.max(1) as f64);
            let spent = last_frame.elapsed();
            if spent < target {
                std::thread::sleep(target - spent);
            }
            last_frame = Instant::now();
        }

        let Some(window) = engine.window() else { return };

        // Place and show the window on the first frames.
        if frame == 1 {
            match &mode {
                Mode::Saver => {
                    if spanning {
                        window.set_decorations(false);
                        window.set_resizable(false);
                        window.set_outer_position(winit::dpi::PhysicalPosition::new(vx, vy));
                        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(vw as u32, vh as u32));
                    } else {
                        let m = window.primary_monitor().or_else(|| window.current_monitor());
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(m)));
                    }
                    window.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
                }
                Mode::Preview(parent) => {
                    if let Some(me) = win::hwnd_of(window) {
                        win::adopt_into(me, *parent, true);
                        preview_size = win::client_size(*parent);
                    }
                }
                Mode::Live { fullscreen } => {
                    if *fullscreen {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    }
                }
                Mode::Record(_) => {
                    // The engine sizes windows in logical pixels; ask for the
                    // exact physical size so the frames are what was asked for.
                    let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(opts.size.0, opts.size.1));
                }
                _ => {}
            }
        }
        if frame == 3 && !shown {
            shown = true;
            match &mode {
                Mode::Saver => {
                    window.set_visible(true);
                    window.set_cursor_visible(false);
                    window.focus_window();
                    wake = Wake::new();
                }
                Mode::Live { .. } => {
                    window.set_visible(true);
                    window.set_cursor_visible(!live_full);
                    window.focus_window();
                }
                _ => {}
            }
        }

        // Screensaver: end on input. Preview: end when the box goes away.
        if saver && shown && wake.woke() {
            engine.request_quit();
            return;
        }
        if let Mode::Preview(parent) = &mode {
            if !win::is_window(*parent) {
                engine.request_quit();
                return;
            }
            let now = win::client_size(*parent);
            if now != preview_size {
                preview_size = now;
                if let Some(me) = win::hwnd_of(window) {
                    win::adopt_into(me, *parent, true);
                }
            }
        }

        // Live-art keys.
        if matches!(mode, Mode::Live { .. }) {
            if engine.input.just_pressed(Key::Escape) {
                engine.request_quit();
                return;
            }
            if engine.input.just_pressed(Key::F11) {
                is_full = !is_full;
                window.set_fullscreen(if is_full { Some(winit::window::Fullscreen::Borderless(None)) } else { None });
                window.set_cursor_visible(!is_full);
            }
            if engine.input.just_pressed(Key::Right) {
                for s in slots.iter_mut() {
                    s.skip(scene_len);
                }
            }
        }

        // (Re)build the slots when the framebuffer size changes.
        let (w, h) = engine.render_size();
        if (w, h) != last_size || slots.is_empty() {
            last_size = (w, h);
            let rects: Vec<([f32; 4], bool)> = if spanning && frame > 1 {
                // Monitor rectangles, relative to the window, scaled in case
                // the window did not get exactly the size it asked for.
                let (sx, sy) = (w as f32 / vw.max(1) as f32, h as f32 / vh.max(1) as f32);
                mons.iter()
                    .map(|m| ([(m.x - vx) as f32 * sx, (m.y - vy) as f32 * sy, m.w as f32 * sx, m.h as f32 * sy], m.primary))
                    .collect()
            } else {
                vec![([0.0, 0.0, w as f32, h as f32], true)]
            };
            // Keep the running scenes if only the size changed.
            let keep = slots.len() == rects.len();
            if keep {
                for (s, (r, _)) in slots.iter_mut().zip(rects.iter()) {
                    s.rect = *r;
                }
            } else {
                slots = rects
                    .iter()
                    .enumerate()
                    .map(|(i, (r, primary))| {
                        let pos = match settings.other_monitors {
                            OtherMonitors::Different => i * 3,
                            _ => 0,
                        };
                        let n = scene::points_for_area(r[2], r[3], preview);
                        let mut s = Slot::new(*r, order.clone(), pos, n, settings.show_equations && !preview);
                        s.blank = settings.other_monitors == OtherMonitors::Black && !*primary;
                        s
                    })
                    .collect();
            }
        }

        for s in slots.iter_mut() {
            s.tick(dt, speed, scene_len);
        }
        for s in &slots {
            // Text sized to the monitor it is on.
            let ts = (s.rect[3] / 1080.0).clamp(0.6, 1.6);
            s.draw(&mut engine.ui, scene_len, ts);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Mode, String> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse(&args, &mut Settings::default(), &mut Opts::default())
    }

    #[test]
    fn windows_switches() {
        assert_eq!(p(&["/s"]).unwrap(), Mode::Saver);
        assert_eq!(p(&["/S"]).unwrap(), Mode::Saver);
        assert_eq!(p(&["-s"]).unwrap(), Mode::Saver);
        assert_eq!(p(&["/p", "1234"]).unwrap(), Mode::Preview(1234 as HWND));
        assert_eq!(p(&["/p:1234"]).unwrap(), Mode::Preview(1234 as HWND));
        assert_eq!(p(&["/c"]).unwrap(), Mode::Settings(std::ptr::null_mut()));
        assert_eq!(p(&["/c:5678"]).unwrap(), Mode::Settings(5678 as HWND));
        assert_eq!(p(&[]).unwrap(), Mode::Settings(std::ptr::null_mut()));
        assert!(p(&["/p"]).is_err());
        assert!(p(&["/x"]).is_err());
    }

    #[test]
    fn options() {
        let mut s = Settings::default();
        let mut o = Opts::default();
        let args: Vec<String> = ["--record", "out", "--scene", "Aizawa", "--size", "720x720", "--frames", "12"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(parse(&args, &mut s, &mut o).unwrap(), Mode::Record("out".into()));
        assert_eq!(s.scene, "aizawa");
        assert_eq!(o.size, (720, 720));
        assert_eq!(o.frames, 12);
        assert!(p(&["--scene", "nope"]).is_err());
    }
}
