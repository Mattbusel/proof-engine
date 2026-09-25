path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 4
// ============================================================

// --- Mix Scene Manager -----------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct MixScene {
    pub id: usize,
    pub name: String,
    pub bus_volumes: Vec<f32>,
    pub bus_mutes: Vec<bool>,
    pub bus_solos: Vec<bool>,
    pub description: String,
    pub tags: Vec<String>,
    pub snapshot_time: f32,
}

impl MixScene {
    pub fn new(id: usize, n_buses: usize) -> Self {
        Self {
            id,
            name: format!("Scene_{}", id),
            bus_volumes: vec![0.0; n_buses],
            bus_mutes: vec![false; n_buses],
            bus_solos: vec![false; n_buses],
            description: String::new(),
            tags: vec![],
            snapshot_time: 0.0,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct MixSceneManagerState {
    pub scenes: Vec<MixScene>,
    pub active_scene: Option<usize>,
    pub preview_scene: Option<usize>,
    pub crossfade_time_ms: f32,
    pub crossfading: bool,
    pub crossfade_progress: f32,
    pub n_buses: usize,
    pub search: String,
}

pub fn show_mix_scene_manager(ui: &mut egui::Ui, state: &mut MixSceneManagerState) {
    ui.heading("Mix Scene Manager");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ Snapshot").clicked() {
            let id = state.scenes.len();
            state.scenes.push(MixScene::new(id, state.n_buses.max(8)));
        }
        ui.separator();
        ui.label("Crossfade:");
        ui.add(egui::DragValue::new(&mut state.crossfade_time_ms).speed(50.0).clamp_range(0.0..=5000.0).suffix(" ms"));
        if state.crossfading {
            ui.add(egui::ProgressBar::new(state.crossfade_progress).desired_width(100.0));
        }
        ui.separator();
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search);
    });
    ui.separator();

    let search_lower = state.search.to_lowercase();
    let mut remove = None;
    for (i, scene) in state.scenes.iter().enumerate() {
        if !search_lower.is_empty() && !scene.name.to_lowercase().contains(&search_lower) { continue; }
        let is_active = state.active_scene == Some(i);
        let col = if is_active { Color32::from_rgb(80, 220, 100) } else { Color32::WHITE };
        ui.horizontal(|ui| {
            if ui.selectable_label(is_active, egui::RichText::new(&scene.name).color(col)).clicked() {
                state.active_scene = Some(i);
            }
            if ui.small_button("Preview").clicked() { state.preview_scene = Some(i); }
            if ui.small_button("Recall").clicked() {
                state.active_scene = Some(i);
                state.crossfading = state.crossfade_time_ms > 0.0;
                state.crossfade_progress = 0.0;
            }
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove {
        state.scenes.remove(ri);
        if state.active_scene == Some(ri) { state.active_scene = None; }
    }

    if let Some(idx) = state.active_scene {
        if idx < state.scenes.len() {
            let scene = &mut state.scenes[idx];
            ui.separator();
            ui.label(egui::RichText::new("Scene Settings").strong());
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut scene.name); });
            ui.horizontal(|ui| { ui.label("Notes:"); ui.text_edit_singleline(&mut scene.description); });
            ui.label("Bus Volumes:");
            for (i, vol) in scene.bus_volumes.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("Bus {}", i));
                    ui.add(egui::Slider::new(vol, -60.0..=12.0).suffix(" dB"));
                    ui.checkbox(&mut scene.bus_mutes[i], "M");
                    ui.checkbox(&mut scene.bus_solos[i], "S");
                });
            }
        }
    }
}

// --- Audio Metering Grid ---------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct BusMeter {
    pub bus_name: String,
    pub rms_db: f32,
    pub peak_db: f32,
    pub peak_hold_db: f32,
    pub peak_hold_timer: f32,
    pub clip: bool,
    pub clip_count: u32,
    pub balance: f32,
}

impl BusMeter {
    pub fn new(name: &str) -> Self {
        Self {
            bus_name: name.to_string(),
            rms_db: -40.0,
            peak_db: -40.0,
            peak_hold_db: -40.0,
            peak_hold_timer: 0.0,
            clip: false,
            clip_count: 0,
            balance: 0.0,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct MeteringGridState {
    pub meters: Vec<BusMeter>,
    pub show_rms: bool,
    pub show_peak_hold: bool,
    pub peak_hold_time_s: f32,
    pub scale_mode: MeterScaleMode,
    pub layout: MeterLayout,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum MeterScaleMode {
    #[default]
    K14,
    K20,
    VU,
    PPM,
}

impl MeterScaleMode {
    pub fn label(&self) -> &str {
        match self {
            MeterScaleMode::K14 => "K-14",
            MeterScaleMode::K20 => "K-20",
            MeterScaleMode::VU => "VU",
            MeterScaleMode::PPM => "PPM",
        }
    }
    pub fn reference_db(&self) -> f32 {
        match self {
            MeterScaleMode::K14 => -14.0,
            MeterScaleMode::K20 => -20.0,
            MeterScaleMode::VU => -18.0,
            MeterScaleMode::PPM => -9.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum MeterLayout {
    #[default]
    Vertical,
    Horizontal,
    Mini,
}

impl MeterLayout {
    pub fn label(&self) -> &str {
        match self { MeterLayout::Vertical => "Vertical", MeterLayout::Horizontal => "Horizontal", MeterLayout::Mini => "Mini" }
    }
}

pub fn show_metering_grid(ui: &mut egui::Ui, state: &mut MeteringGridState) {
    ui.heading("Audio Metering");
    ui.separator();

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_rms, "RMS");
        ui.checkbox(&mut state.show_peak_hold, "Peak Hold");
        if state.show_peak_hold {
            ui.add(egui::DragValue::new(&mut state.peak_hold_time_s).speed(0.1).clamp_range(0.5..=10.0).suffix("s"));
        }
        ui.separator();
        for mode in &[MeterScaleMode::K14, MeterScaleMode::K20, MeterScaleMode::VU, MeterScaleMode::PPM] {
            if ui.selectable_label(state.scale_mode == *mode, mode.label()).clicked() { state.scale_mode = mode.clone(); }
        }
        ui.separator();
        for layout in &[MeterLayout::Vertical, MeterLayout::Horizontal, MeterLayout::Mini] {
            if ui.selectable_label(state.layout == *layout, layout.label()).clicked() { state.layout = layout.clone(); }
        }
        if ui.button("Reset Peaks").clicked() {
            for m in &mut state.meters { m.clip = false; m.clip_count = 0; m.peak_hold_db = m.rms_db; }
        }
    });
    ui.separator();

    match state.layout {
        MeterLayout::Vertical => draw_meters_vertical(ui, state),
        MeterLayout::Horizontal => draw_meters_horizontal(ui, state),
        MeterLayout::Mini => draw_meters_mini(ui, state),
    }
}

pub fn draw_meters_vertical(ui: &mut egui::Ui, state: &MeteringGridState) {
    let meter_w = 36.0f32;
    let meter_h = 160.0f32;
    let gap = 4.0f32;
    let n = state.meters.len();
    if n == 0 { return; }
    let total_w = (meter_w + gap) * n as f32;
    let size = Vec2::new(total_w.min(ui.available_width()), meter_h + 20.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));

    let ref_db = state.scale_mode.reference_db();

    for (i, meter) in state.meters.iter().enumerate() {
        let mx = rect.left() + i as f32 * (meter_w + gap);
        let mr = Rect::from_min_size(Pos2::new(mx, rect.top() + 4.0), Vec2::new(meter_w - gap, meter_h));
        p.rect_filled(mr, 2.0, Color32::from_rgb(15, 18, 25));

        // dB to pixel: 0 dBFS at top, -60 at bottom
        let db_to_y = |db: f32| -> f32 {
            let t = 1.0 - ((db + 60.0) / 60.0).clamp(0.0, 1.0);
            mr.top() + t * mr.height()
        };

        let rms_y = db_to_y(meter.rms_db);
        let peak_y = db_to_y(meter.peak_db);

        // RMS bar
        if state.show_rms {
            let bar = Rect::from_min_max(Pos2::new(mr.left() + 1.0, rms_y), Pos2::new(mr.right() - 1.0, mr.bottom()));
            let t = (meter.rms_db + 60.0) / 60.0;
            let col = if meter.rms_db > -3.0 { Color32::from_rgb(255, 60, 60) }
                      else if meter.rms_db > -9.0 { Color32::from_rgb(255, 200, 60) }
                      else { Color32::from_rgb(60, 200, 80) };
            p.rect_filled(bar, 1.0, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 180));
        }

        // Peak line
        p.line_segment(
            [Pos2::new(mr.left() + 1.0, peak_y), Pos2::new(mr.right() - 1.0, peak_y)],
            Stroke::new(1.5, Color32::WHITE),
        );

        // Peak hold
        if state.show_peak_hold && meter.peak_hold_db > -58.0 {
            let hold_y = db_to_y(meter.peak_hold_db);
            p.line_segment(
                [Pos2::new(mr.left() + 1.0, hold_y), Pos2::new(mr.right() - 1.0, hold_y)],
                Stroke::new(1.5, Color32::from_rgb(255, 220, 60)),
            );
        }

        // 0 dBFS line
        let zero_y = db_to_y(0.0);
        p.line_segment([Pos2::new(mr.left(), zero_y), Pos2::new(mr.right(), zero_y)],
            Stroke::new(1.0, Color32::from_rgb(200, 80, 80)));

        // Reference line
        let ref_y = db_to_y(ref_db);
        p.line_segment([Pos2::new(mr.left(), ref_y), Pos2::new(mr.right(), ref_y)],
            Stroke::new(0.5, Color32::from_rgba_premultiplied(255, 255, 255, 80)));

        // Clip indicator
        if meter.clip {
            p.rect_filled(Rect::from_min_size(mr.min, Vec2::new(mr.width(), 6.0)),
                1.0, Color32::RED);
        }

        let short: String = meter.bus_name.chars().take(4).collect();
        p.text(Pos2::new(mr.center().x, mr.bottom() + 10.0), egui::Align2::CENTER_CENTER,
            short, FontId::monospace(7.0), Color32::GRAY);
        p.text(Pos2::new(mr.center().x, peak_y - 8.0), egui::Align2::CENTER_CENTER,
            format!("{:.0}", meter.peak_db), FontId::monospace(6.0), Color32::from_rgb(200, 200, 200));
    }
}

pub fn draw_meters_horizontal(ui: &mut egui::Ui, state: &MeteringGridState) {
    for meter in &state.meters {
        ui.horizontal(|ui| {
            let label: String = meter.bus_name.chars().take(8).collect();
            ui.label(egui::RichText::new(label).font(FontId::monospace(9.0)).color(Color32::GRAY));
            let t = ((meter.rms_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let col = if meter.rms_db > -3.0 { Color32::from_rgb(255, 60, 60) }
                      else if meter.rms_db > -9.0 { Color32::from_rgb(255, 200, 60) }
                      else { Color32::from_rgb(60, 200, 80) };
            ui.add(egui::ProgressBar::new(t).fill(col).desired_width(160.0).show_percentage(false));
            ui.label(egui::RichText::new(format!("{:5.1}", meter.peak_db)).font(FontId::monospace(9.0)));
        });
    }
}

pub fn draw_meters_mini(ui: &mut egui::Ui, state: &MeteringGridState) {
    ui.horizontal_wrapped(|ui| {
        for meter in &state.meters {
            let t = ((meter.rms_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let col = if meter.rms_db > -3.0 { Color32::RED }
                      else if meter.rms_db > -9.0 { Color32::YELLOW }
                      else { Color32::from_rgb(60, 200, 80) };
            ui.vertical(|ui| {
                let size = Vec2::new(18.0, 40.0);
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                let p = ui.painter_at(rect);
                p.rect_filled(rect, 2.0, Color32::from_rgb(15, 18, 25));
                let bar_h = t * rect.height();
                p.rect_filled(
                    Rect::from_min_size(Pos2::new(rect.left(), rect.bottom() - bar_h), Vec2::new(rect.width(), bar_h)),
                    2.0, col,
                );
                let short: String = meter.bus_name.chars().take(3).collect();
                ui.label(egui::RichText::new(short).font(FontId::monospace(7.0)).color(Color32::GRAY));
            });
        }
    });
}

// --- Audio Timeline --------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioClipInstance {
    pub id: usize,
    pub clip_name: String,
    pub track: usize,
    pub start_beat: f32,
    pub duration_beats: f32,
    pub volume_db: f32,
    pub pitch_st: f32,
    pub fade_in_beats: f32,
    pub fade_out_beats: f32,
    pub color: [u8; 3],
    pub muted: bool,
    pub locked: bool,
    pub loop_clip: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioTimelineTrack {
    pub id: usize,
    pub name: String,
    pub muted: bool,
    pub solo: bool,
    pub volume_db: f32,
    pub color: [u8; 3],
    pub height: f32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioTimelineState {
    pub tracks: Vec<AudioTimelineTrack>,
    pub clips: Vec<AudioClipInstance>,
    pub bpm: f32,
    pub time_sig_num: u8,
    pub time_sig_den: u8,
    pub total_beats: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub zoom_x: f32,
    pub zoom_y: f32,
    pub cursor_beat: f32,
    pub loop_start: f32,
    pub loop_end: f32,
    pub playing: bool,
    pub loop_enabled: bool,
    pub selected_clip: Option<usize>,
    pub snap_beats: f32,
}

impl AudioTimelineState {
    pub fn new() -> Self {
        let mut s = Self {
            bpm: 120.0,
            time_sig_num: 4,
            time_sig_den: 4,
            total_beats: 64.0,
            zoom_x: 40.0,
            zoom_y: 1.0,
            loop_end: 16.0,
            snap_beats: 0.25,
            ..Default::default()
        };
        s.tracks = vec![
            AudioTimelineTrack { id: 0, name: "Music".into(), muted: false, solo: false, volume_db: 0.0, color: [80, 140, 220], height: 40.0 },
            AudioTimelineTrack { id: 1, name: "SFX".into(), muted: false, solo: false, volume_db: 0.0, color: [220, 140, 60], height: 40.0 },
            AudioTimelineTrack { id: 2, name: "Voice".into(), muted: false, solo: false, volume_db: 0.0, color: [80, 200, 120], height: 40.0 },
            AudioTimelineTrack { id: 3, name: "Ambient".into(), muted: false, solo: false, volume_db: 0.0, color: [160, 80, 220], height: 40.0 },
        ];
        s.clips = vec![
            AudioClipInstance { id: 0, clip_name: "music_loop.ogg".into(), track: 0, start_beat: 0.0, duration_beats: 16.0, volume_db: 0.0, pitch_st: 0.0, fade_in_beats: 0.5, fade_out_beats: 0.5, color: [80, 140, 220], muted: false, locked: false, loop_clip: true },
            AudioClipInstance { id: 1, clip_name: "intro_sting.wav".into(), track: 1, start_beat: 2.0, duration_beats: 4.0, volume_db: -3.0, pitch_st: 0.0, fade_in_beats: 0.0, fade_out_beats: 0.25, color: [220, 140, 60], muted: false, locked: false, loop_clip: false },
            AudioClipInstance { id: 2, clip_name: "narration_01.ogg".into(), track: 2, start_beat: 4.0, duration_beats: 12.0, volume_db: 0.0, pitch_st: 0.0, fade_in_beats: 0.25, fade_out_beats: 0.5, color: [80, 200, 120], muted: false, locked: false, loop_clip: false },
            AudioClipInstance { id: 3, clip_name: "forest_ambient.ogg".into(), track: 3, start_beat: 0.0, duration_beats: 32.0, volume_db: -12.0, pitch_st: 0.0, fade_in_beats: 2.0, fade_out_beats: 2.0, color: [160, 80, 220], muted: false, locked: true, loop_clip: true },
        ];
        s
    }
}

pub fn show_audio_timeline(ui: &mut egui::Ui, state: &mut AudioTimelineState) {
    ui.heading("Audio Timeline");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button(if state.playing { "⏸" } else { "▶" }).clicked() { state.playing = !state.playing; }
        if ui.button("⏹").clicked() { state.playing = false; state.cursor_beat = 0.0; }
        ui.separator();
        ui.label("BPM:");
        ui.add(egui::DragValue::new(&mut state.bpm).speed(0.5).clamp_range(20.0..=300.0));
        ui.label(format!("{}/{}", state.time_sig_num, state.time_sig_den));
        ui.separator();
        ui.checkbox(&mut state.loop_enabled, "Loop");
        if state.loop_enabled {
            ui.label(format!("{:.1}-{:.1}", state.loop_start, state.loop_end));
        }
        ui.separator();
        ui.label("Snap:");
        for &snap in &[0.0625f32, 0.125, 0.25, 0.5, 1.0] {
            let lbl = if snap < 0.5 { format!("1/{}", (1.0 / snap) as u32) } else if snap == 1.0 { "1".into() } else { format!("1/{}", (1.0 / snap) as u32) };
            if ui.selectable_label((state.snap_beats - snap).abs() < 0.001, lbl).clicked() { state.snap_beats = snap; }
        }
        ui.separator();
        ui.label("Zoom:");
        ui.add(egui::DragValue::new(&mut state.zoom_x).speed(1.0).clamp_range(10.0..=200.0).suffix("px/beat"));
    });

    ui.separator();
    draw_audio_timeline(ui, state);
    ui.separator();

    if let Some(idx) = state.selected_clip {
        if idx < state.clips.len() {
            let clip = &mut state.clips[idx];
            ui.label(egui::RichText::new("Clip Settings").strong());
            ui.horizontal(|ui| { ui.label("File:"); ui.text_edit_singleline(&mut clip.clip_name); });
            ui.horizontal(|ui| {
                ui.label("Start:");
                ui.add(egui::DragValue::new(&mut clip.start_beat).speed(0.125).clamp_range(0.0..=1000.0).suffix(" beats"));
                ui.label("Dur:");
                ui.add(egui::DragValue::new(&mut clip.duration_beats).speed(0.125).clamp_range(0.125..=1000.0).suffix(" beats"));
            });
            ui.horizontal(|ui| {
                ui.label("Vol:");
                ui.add(egui::DragValue::new(&mut clip.volume_db).speed(0.5).clamp_range(-60.0..=12.0).suffix(" dB"));
                ui.label("Pitch:");
                ui.add(egui::DragValue::new(&mut clip.pitch_st).speed(0.1).clamp_range(-24.0..=24.0).suffix(" st"));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut clip.muted, "Muted");
                ui.checkbox(&mut clip.locked, "Locked");
                ui.checkbox(&mut clip.loop_clip, "Loop");
                ui.label("Fade In:");
                ui.add(egui::DragValue::new(&mut clip.fade_in_beats).speed(0.05).clamp_range(0.0..=8.0).suffix(" b"));
                ui.label("Fade Out:");
                ui.add(egui::DragValue::new(&mut clip.fade_out_beats).speed(0.05).clamp_range(0.0..=8.0).suffix(" b"));
            });
        }
    }
}

pub fn draw_audio_timeline(ui: &mut egui::Ui, state: &AudioTimelineState) {
    let track_label_w = 80.0f32;
    let ruler_h = 20.0f32;
    let total_track_h: f32 = state.tracks.iter().map(|t| t.height).sum::<f32>() + state.tracks.len() as f32 * 2.0;
    let size = Vec2::new(ui.available_width(), (total_track_h + ruler_h + 8.0).min(300.0));
    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let content_rect = Rect::from_min_max(
        Pos2::new(rect.left() + track_label_w, rect.top()),
        rect.max,
    );

    let beat_to_x = |beat: f32| -> f32 {
        content_rect.left() + (beat - state.scroll_x) * state.zoom_x
    };

    // Ruler
    let ruler_rect = Rect::from_min_size(content_rect.min, Vec2::new(content_rect.width(), ruler_h));
    p.rect_filled(ruler_rect, 0.0, Color32::from_rgb(16, 18, 24));

    let beat_step = if state.zoom_x > 40.0 { 1.0 } else if state.zoom_x > 20.0 { 2.0 } else { 4.0 };
    let mut beat = (state.scroll_x / beat_step).floor() * beat_step;
    while beat < state.scroll_x + content_rect.width() / state.zoom_x {
        let x = beat_to_x(beat);
        if x >= content_rect.left() && x <= content_rect.right() {
            let is_bar = (beat / state.time_sig_num as f32).fract().abs() < 0.001;
            p.line_segment([Pos2::new(x, ruler_rect.top()), Pos2::new(x, ruler_rect.bottom())],
                Stroke::new(if is_bar { 1.0 } else { 0.5 },
                    if is_bar { Color32::from_rgb(80, 85, 100) } else { Color32::from_rgb(40, 45, 60) }));
            if is_bar {
                let bar_num = (beat / state.time_sig_num as f32) as u32 + 1;
                p.text(Pos2::new(x + 2.0, ruler_rect.center().y), egui::Align2::LEFT_CENTER,
                    format!("{}", bar_num), FontId::monospace(8.0), Color32::from_rgb(140, 145, 160));
            }
        }
        beat += beat_step;
    }

    // Loop region
    if state.loop_enabled {
        let lx = beat_to_x(state.loop_start);
        let rx = beat_to_x(state.loop_end);
        if rx > content_rect.left() && lx < content_rect.right() {
            p.rect_filled(
                Rect::from_min_max(Pos2::new(lx.max(content_rect.left()), ruler_rect.top()),
                                   Pos2::new(rx.min(content_rect.right()), ruler_rect.bottom())),
                0.0, Color32::from_rgba_premultiplied(80, 200, 255, 30),
            );
        }
    }

    // Track rows
    let mut track_y = rect.top() + ruler_h;
    for track in &state.tracks {
        let tr = Rect::from_min_size(Pos2::new(rect.left(), track_y), Vec2::new(rect.width(), track.height));
        let tcol = Color32::from_rgba_premultiplied(track.color[0], track.color[1], track.color[2], 30);
        p.rect_filled(tr, 0.0, tcol);

        // Track label
        let label_rect = Rect::from_min_size(tr.min, Vec2::new(track_label_w, track.height));
        p.rect_filled(label_rect, 0.0, Color32::from_rgb(16, 18, 26));
        let col = Color32::from_rgb(track.color[0], track.color[1], track.color[2]);
        p.text(Pos2::new(label_rect.left() + 4.0, label_rect.center().y), egui::Align2::LEFT_CENTER,
            &track.name, FontId::monospace(9.0), col);
        if track.muted {
            p.text(Pos2::new(label_rect.right() - 2.0, label_rect.center().y), egui::Align2::RIGHT_CENTER,
                "M", FontId::monospace(9.0), Color32::YELLOW);
        }

        // Beat lines
        let mut beat = (state.scroll_x / state.time_sig_num as f32).floor() * state.time_sig_num as f32;
        while beat < state.scroll_x + content_rect.width() / state.zoom_x {
            let x = beat_to_x(beat);
            if x >= content_rect.left() && x <= content_rect.right() {
                p.line_segment([Pos2::new(x, tr.top()), Pos2::new(x, tr.bottom())],
                    Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 45, 60, 80)));
            }
            beat += state.time_sig_num as f32;
        }

        // Track separator
        p.line_segment([Pos2::new(tr.left(), tr.bottom()), Pos2::new(tr.right(), tr.bottom())],
            Stroke::new(1.0, Color32::from_rgb(25, 28, 38)));

        track_y += track.height + 2.0;
    }

    // Clips
    let mut track_y = rect.top() + ruler_h;
    for (ti, track) in state.tracks.iter().enumerate() {
        for clip in state.clips.iter().filter(|c| c.track == ti) {
            let cx0 = beat_to_x(clip.start_beat);
            let cx1 = beat_to_x(clip.start_beat + clip.duration_beats);
            let cy0 = track_y + 2.0;
            let cy1 = track_y + track.height - 2.0;

            if cx1 < content_rect.left() || cx0 > content_rect.right() {
                continue;
            }
            let clip_rect = Rect::from_min_max(
                Pos2::new(cx0.max(content_rect.left()), cy0),
                Pos2::new(cx1.min(content_rect.right()), cy1),
            );

            let base_col = clip.color;
            let alpha = if clip.muted { 80u8 } else { 180u8 };
            p.rect_filled(clip_rect, 3.0,
                Color32::from_rgba_premultiplied(base_col[0], base_col[1], base_col[2], alpha));
            p.rect_stroke(clip_rect, 3.0,
                Stroke::new(if state.selected_clip == Some(clip.id) { 2.0 } else { 0.5 },
                    Color32::from_rgb(base_col[0], base_col[1], base_col[2])));

            // Fade in
            if clip.fade_in_beats > 0.0 {
                let fade_x = beat_to_x(clip.start_beat + clip.fade_in_beats).min(clip_rect.right());
                p.line_segment([clip_rect.left_bottom(), Pos2::new(fade_x, clip_rect.top())],
                    Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 100)));
            }
            // Fade out
            if clip.fade_out_beats > 0.0 {
                let fade_x = beat_to_x(clip.start_beat + clip.duration_beats - clip.fade_out_beats).max(clip_rect.left());
                p.line_segment([Pos2::new(fade_x, clip_rect.top()), clip_rect.right_bottom()],
                    Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 100)));
            }

            // Label
            let clip_w = clip_rect.width();
            if clip_w > 20.0 {
                let lbl: String = clip.clip_name.chars().take((clip_w / 7.0) as usize).collect();
                p.text(Pos2::new(clip_rect.left() + 3.0, clip_rect.center().y), egui::Align2::LEFT_CENTER,
                    lbl, FontId::monospace(8.0), Color32::WHITE);
            }

            // Loop indicator
            if clip.loop_clip && clip_w > 12.0 {
                p.text(Pos2::new(clip_rect.right() - 2.0, clip_rect.top() + 2.0), egui::Align2::RIGHT_TOP,
                    "↻", FontId::monospace(8.0), Color32::from_rgba_premultiplied(255, 255, 255, 160));
            }
        }
        track_y += track.height + 2.0;
    }

    // Playhead
    let px = beat_to_x(state.cursor_beat);
    if px >= content_rect.left() && px <= content_rect.right() {
        p.line_segment([Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
            Stroke::new(1.5, Color32::from_rgb(255, 220, 60)));
        p.circle_filled(Pos2::new(px, rect.top() + ruler_h * 0.5), 5.0, Color32::from_rgb(255, 220, 60));
    }
}

// --- Audio File Browser ----------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum AudioFileType {
    Wav,
    Ogg,
    Mp3,
    Flac,
    Aif,
    Unknown,
}

impl AudioFileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "wav" => AudioFileType::Wav,
            "ogg" => AudioFileType::Ogg,
            "mp3" => AudioFileType::Mp3,
            "flac" => AudioFileType::Flac,
            "aif" | "aiff" => AudioFileType::Aif,
            _ => AudioFileType::Unknown,
        }
    }
    pub fn label(&self) -> &str {
        match self {
            AudioFileType::Wav => "WAV",
            AudioFileType::Ogg => "OGG",
            AudioFileType::Mp3 => "MP3",
            AudioFileType::Flac => "FLAC",
            AudioFileType::Aif => "AIF",
            AudioFileType::Unknown => "???",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            AudioFileType::Wav => Color32::from_rgb(80, 180, 255),
            AudioFileType::Ogg => Color32::from_rgb(80, 220, 100),
            AudioFileType::Mp3 => Color32::from_rgb(255, 180, 60),
            AudioFileType::Flac => Color32::from_rgb(160, 100, 255),
            AudioFileType::Aif => Color32::from_rgb(255, 100, 160),
            AudioFileType::Unknown => Color32::GRAY,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioFileEntry {
    pub name: String,
    pub path: String,
    pub file_type: AudioFileType,
    pub duration_s: f32,
    pub sample_rate: u32,
    pub channels: u8,
    pub size_kb: u32,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub waveform_preview: Vec<f32>,
}

impl AudioFileEntry {
    pub fn new(name: &str, path: &str, file_type: AudioFileType, duration_s: f32) -> Self {
        let waveform_preview = (0..32).map(|i| {
            let t = i as f32 / 32.0;
            (t * std::f32::consts::TAU * 3.0).sin() * 0.7 + (t * std::f32::consts::TAU * 7.0).sin() * 0.3
        }).collect();
        Self {
            name: name.to_string(),
            path: path.to_string(),
            file_type,
            duration_s,
            sample_rate: 44100,
            channels: 2,
            size_kb: (duration_s * 44100.0 * 4.0 / 1024.0) as u32,
            tags: vec![],
            favorite: false,
            waveform_preview,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioFileBrowserState {
    pub files: Vec<AudioFileEntry>,
    pub selected: Option<usize>,
    pub search: String,
    pub filter_type: Option<AudioFileType>,
    pub sort_by: FileSortMode,
    pub show_waveforms: bool,
    pub show_favorites_only: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum FileSortMode {
    #[default]
    Name,
    Duration,
    Size,
    Type,
}

impl FileSortMode {
    pub fn label(&self) -> &str {
        match self { FileSortMode::Name => "Name", FileSortMode::Duration => "Duration",
                     FileSortMode::Size => "Size", FileSortMode::Type => "Type" }
    }
}

impl AudioFileBrowserState {
    pub fn with_demo_files() -> Self {
        let mut s = Self { show_waveforms: true, ..Default::default() };
        s.files = vec![
            AudioFileEntry::new("kick_drum.wav", "sfx/percussion/kick_drum.wav", AudioFileType::Wav, 0.45),
            AudioFileEntry::new("snare_crack.wav", "sfx/percussion/snare_crack.wav", AudioFileType::Wav, 0.38),
            AudioFileEntry::new("hihat_closed.wav", "sfx/percussion/hihat_closed.wav", AudioFileType::Wav, 0.12),
            AudioFileEntry::new("bass_loop_120.ogg", "music/loops/bass_loop_120.ogg", AudioFileType::Ogg, 4.0),
            AudioFileEntry::new("piano_c4.wav", "music/instruments/piano_c4.wav", AudioFileType::Wav, 2.1),
            AudioFileEntry::new("explosion_large.wav", "sfx/combat/explosion_large.wav", AudioFileType::Wav, 1.8),
            AudioFileEntry::new("footstep_grass.ogg", "sfx/foley/footstep_grass.ogg", AudioFileType::Ogg, 0.31),
            AudioFileEntry::new("ambient_forest.ogg", "sfx/ambient/ambient_forest.ogg", AudioFileType::Ogg, 30.0),
            AudioFileEntry::new("ui_click.wav", "sfx/ui/ui_click.wav", AudioFileType::Wav, 0.05),
            AudioFileEntry::new("narration_intro.flac", "voice/narration_intro.flac", AudioFileType::Flac, 12.5),
            AudioFileEntry::new("theme_main.mp3", "music/theme_main.mp3", AudioFileType::Mp3, 180.0),
            AudioFileEntry::new("glass_break.wav", "sfx/destruction/glass_break.wav", AudioFileType::Wav, 0.75),
        ];
        s
    }
}

pub fn show_audio_file_browser(ui: &mut egui::Ui, state: &mut AudioFileBrowserState) {
    ui.heading("Audio File Browser");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search);
        ui.separator();
        ui.checkbox(&mut state.show_waveforms, "Waveforms");
        ui.checkbox(&mut state.show_favorites_only, "Favorites");
        ui.separator();
        ui.label("Sort:");
        for mode in &[FileSortMode::Name, FileSortMode::Duration, FileSortMode::Size, FileSortMode::Type] {
            if ui.selectable_label(state.sort_by == *mode, mode.label()).clicked() { state.sort_by = mode.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Filter:");
        if ui.selectable_label(state.filter_type.is_none(), "All").clicked() { state.filter_type = None; }
        for ft in &[AudioFileType::Wav, AudioFileType::Ogg, AudioFileType::Mp3, AudioFileType::Flac] {
            let sel = state.filter_type.as_ref() == Some(ft);
            if ui.selectable_label(sel, egui::RichText::new(ft.label()).color(ft.color())).clicked() {
                state.filter_type = if sel { None } else { Some(ft.clone()) };
            }
        }
    });
    ui.separator();

    let search_lower = state.search.to_lowercase();
    egui::ScrollArea::vertical().max_height(300.0).id_source("file_browser").show(ui, |ui| {
        egui::Grid::new("files").num_columns(if state.show_waveforms { 6 } else { 5 })
            .spacing([4.0, 2.0]).striped(true).show(ui, |ui| {
            for (i, file) in state.files.iter_mut().enumerate() {
                if !search_lower.is_empty() && !file.name.to_lowercase().contains(&search_lower) { continue; }
                if state.show_favorites_only && !file.favorite { continue; }
                if let Some(ref ft) = state.filter_type {
                    if file.file_type != *ft { continue; }
                }
                let sel = state.selected == Some(i);
                ui.colored_label(file.file_type.color(), file.file_type.label());
                if ui.selectable_label(sel, &file.name).clicked() { state.selected = Some(i); }
                ui.label(egui::RichText::new(format!("{:.2}s", file.duration_s)).small().color(Color32::GRAY));
                ui.label(egui::RichText::new(format!("{} KB", file.size_kb)).small().color(Color32::GRAY));
                if ui.small_button(if file.favorite { "★" } else { "☆" }).clicked() { file.favorite = !file.favorite; }
                if state.show_waveforms {
                    let size = Vec2::new(60.0, 18.0);
                    let (wr, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    let wp = ui.painter_at(wr);
                    wp.rect_filled(wr, 1.0, Color32::from_rgb(12, 14, 20));
                    let n = file.waveform_preview.len();
                    let col = file.file_type.color();
                    for (j, &amp) in file.waveform_preview.iter().enumerate() {
                        let x = wr.left() + j as f32 / n as f32 * wr.width();
                        let h = amp.abs() * wr.height() * 0.45;
                        wp.line_segment([Pos2::new(x, wr.center().y - h), Pos2::new(x, wr.center().y + h)],
                            Stroke::new(1.0, col));
                    }
                }
                if ui.small_button("▶").clicked() {}
                ui.end_row();
            }
        });
    });

    if let Some(idx) = state.selected {
        if idx < state.files.len() {
            let f = &state.files[idx];
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&f.name).strong());
                ui.colored_label(f.file_type.color(), f.file_type.label());
                ui.label(format!("{:.2}s  {}Hz  {}ch  {} KB", f.duration_s, f.sample_rate, f.channels, f.size_kb));
            });
            if !f.tags.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Tags:");
                    for t in &f.tags { ui.label(egui::RichText::new(t).small().color(Color32::from_rgb(100, 200, 255))); }
                });
            }
            ui.horizontal(|ui| {
                if ui.button("Add to Timeline").clicked() {}
                if ui.button("Add to Event").clicked() {}
                if ui.button("Open in Waveform Editor").clicked() {}
            });
        }
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
