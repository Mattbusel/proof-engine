path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 12 (FINAL)
// ============================================================

// --- Convolution Engine Visualizer -----------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct ConvolutionEngineState {
    pub ir_loaded: bool,
    pub ir_name: String,
    pub ir_length_ms: f32,
    pub ir_sample_rate: u32,
    pub pre_delay_ms: f32,
    pub trim_start_ms: f32,
    pub trim_end_ms: f32,
    pub reverse: bool,
    pub stretch: f32,
    pub gain_db: f32,
    pub stereo_spread: f32,
    pub hpf_freq: f32,
    pub lpf_freq: f32,
    pub wet_db: f32,
    pub dry_db: f32,
    pub enabled: bool,
    pub partitioned_conv: bool,
    pub partition_size: u32,
    pub cpu_usage_percent: f32,
    pub impulse_preview: Vec<f32>,
}

impl Default for ConvolutionEngineState {
    fn default() -> Self {
        let impulse_preview = (0..128).map(|i| {
            let t = i as f32 / 128.0;
            let decay = (-t * 5.0).exp();
            let noise = ((i * 7919 + 12345) % 1000) as f32 / 500.0 - 1.0;
            noise * decay
        }).collect();
        Self {
            ir_loaded: true,
            ir_name: "hall_large.wav".into(),
            ir_length_ms: 3200.0,
            ir_sample_rate: 44100,
            pre_delay_ms: 10.0,
            trim_start_ms: 0.0,
            trim_end_ms: 3200.0,
            reverse: false,
            stretch: 1.0,
            gain_db: 0.0,
            stereo_spread: 1.0,
            hpf_freq: 20.0,
            lpf_freq: 20000.0,
            wet_db: -6.0,
            dry_db: 0.0,
            enabled: true,
            partitioned_conv: true,
            partition_size: 512,
            cpu_usage_percent: 3.5,
            impulse_preview,
        }
    }
}

pub fn show_convolution_engine(ui: &mut egui::Ui, state: &mut ConvolutionEngineState) {
    ui.heading("Convolution Engine");
    ui.separator();

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.enabled, "Enable");
        ui.checkbox(&mut state.partitioned_conv, "Partitioned");
        if state.partitioned_conv {
            ui.label("Part. Size:");
            for sz in &[256u32, 512, 1024, 2048] {
                if ui.selectable_label(state.partition_size == *sz, format!("{}", sz)).clicked() { state.partition_size = *sz; }
            }
        }
        ui.label(format!("CPU: {:.1}%", state.cpu_usage_percent));
    });

    ui.horizontal(|ui| {
        ui.label("IR:");
        ui.label(egui::RichText::new(&state.ir_name).color(Color32::from_rgb(100, 200, 255)));
        if ui.button("Browse").clicked() {}
        if ui.button("Clear").clicked() { state.ir_loaded = false; }
        ui.label(format!("{:.0}ms @ {}Hz", state.ir_length_ms, state.ir_sample_rate));
    });
    ui.separator();

    draw_ir_waveform(ui, state);
    ui.separator();

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.horizontal(|ui| { ui.label("Pre-delay:"); ui.add(egui::Slider::new(&mut state.pre_delay_ms, 0.0..=200.0).suffix(" ms")); });
        ui.horizontal(|ui| { ui.label("Trim Start:"); ui.add(egui::Slider::new(&mut state.trim_start_ms, 0.0..=state.ir_length_ms * 0.5)); });
        ui.horizontal(|ui| { ui.label("Trim End:"); ui.add(egui::Slider::new(&mut state.trim_end_ms, state.trim_start_ms..=state.ir_length_ms)); });
        ui.horizontal(|ui| { ui.label("Stretch:"); ui.add(egui::Slider::new(&mut state.stretch, 0.25..=4.0).logarithmic(true)); });
        ui.checkbox(&mut state.reverse, "Reverse IR");

        let ui = &mut cols[1];
        ui.horizontal(|ui| { ui.label("Gain:"); ui.add(egui::Slider::new(&mut state.gain_db, -24.0..=12.0).suffix(" dB")); });
        ui.horizontal(|ui| { ui.label("Stereo:"); ui.add(egui::Slider::new(&mut state.stereo_spread, 0.0..=2.0)); });
        ui.horizontal(|ui| { ui.label("HP:"); ui.add(egui::Slider::new(&mut state.hpf_freq, 20.0..=2000.0).logarithmic(true).suffix(" Hz")); });
        ui.horizontal(|ui| { ui.label("LP:"); ui.add(egui::Slider::new(&mut state.lpf_freq, 2000.0..=20000.0).logarithmic(true).suffix(" Hz")); });
        ui.horizontal(|ui| { ui.label("Wet:"); ui.add(egui::Slider::new(&mut state.wet_db, -60.0..=6.0).suffix(" dB")); });
        ui.horizontal(|ui| { ui.label("Dry:"); ui.add(egui::Slider::new(&mut state.dry_db, -60.0..=6.0).suffix(" dB")); });
    });
}

pub fn draw_ir_waveform(ui: &mut egui::Ui, state: &ConvolutionEngineState) {
    let size = Vec2::new(ui.available_width().min(480.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    if !state.ir_loaded { return; }

    let n = state.impulse_preview.len();
    let trim_s = state.trim_start_ms / state.ir_length_ms;
    let trim_e = state.trim_end_ms / state.ir_length_ms;
    let ts_x = rect.left() + trim_s * rect.width();
    let te_x = rect.left() + trim_e * rect.width();

    p.rect_filled(
        Rect::from_min_max(Pos2::new(ts_x, rect.top()), Pos2::new(te_x, rect.bottom())),
        0.0, Color32::from_rgba_premultiplied(40, 80, 120, 40),
    );

    for i in 0..n {
        let t = i as f32 / n as f32;
        let x = rect.left() + t * rect.width();
        let amp = state.impulse_preview[i];
        let h = amp.abs() * rect.height() * 0.45;
        let col = if t < trim_s || t > trim_e { Color32::from_rgba_premultiplied(60, 80, 100, 120) }
                  else { Color32::from_rgb(80, 180, 255) };
        p.line_segment([Pos2::new(x, rect.center().y - h), Pos2::new(x, rect.center().y + h)], Stroke::new(1.0, col));
    }

    let pre_x = rect.left() + (state.pre_delay_ms / state.ir_length_ms) * rect.width();
    p.line_segment([Pos2::new(pre_x, rect.top()), Pos2::new(pre_x, rect.bottom())],
        Stroke::new(1.0, Color32::from_rgb(255, 160, 60)));
    p.text(Pos2::new(pre_x + 2.0, rect.top() + 2.0), egui::Align2::LEFT_TOP,
        "Pre", FontId::monospace(7.0), Color32::from_rgb(255, 160, 60));
}

// --- Mastering Chain -------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct MasteringChainStage {
    pub name: String,
    pub stage_type: String,
    pub enabled: bool,
    pub bypass: bool,
    pub gain_in_db: f32,
    pub gain_out_db: f32,
    pub params: std::collections::HashMap<String, f32>,
    pub metering_pre_db: f32,
    pub metering_post_db: f32,
}

impl MasteringChainStage {
    pub fn new(name: &str, stage_type: &str) -> Self {
        Self {
            name: name.to_string(),
            stage_type: stage_type.to_string(),
            enabled: true,
            bypass: false,
            gain_in_db: 0.0,
            gain_out_db: 0.0,
            params: std::collections::HashMap::new(),
            metering_pre_db: -18.0,
            metering_post_db: -18.0,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct MasteringChainState {
    pub stages: Vec<MasteringChainStage>,
    pub input_level_db: f32,
    pub output_level_db: f32,
    pub target_lufs: f32,
    pub target_true_peak_dbtp: f32,
    pub format_preset: String,
    pub selected: Option<usize>,
}

impl MasteringChainState {
    pub fn standard() -> Self {
        let mut s = Self {
            target_lufs: -14.0,
            target_true_peak_dbtp: -1.0,
            format_preset: "Streaming".into(),
            ..Default::default()
        };
        let stage_defs: &[(&str, &str)] = &[
            ("High Pass", "EQ"),
            ("Multiband EQ", "EQ"),
            ("Stereo Width", "Stereo"),
            ("Soft Saturation", "Saturation"),
            ("Multiband Comp", "Dynamics"),
            ("Mid/Side EQ", "EQ"),
            ("Master Comp", "Dynamics"),
            ("Stereo Limiter", "Dynamics"),
            ("True Peak Limiter", "Limiter"),
        ];
        for (name, stype) in stage_defs {
            s.stages.push(MasteringChainStage::new(name, stype));
        }
        s
    }
}

pub fn show_mastering_chain(ui: &mut egui::Ui, state: &mut MasteringChainState) {
    ui.heading("Mastering Chain");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Target:");
        ui.add(egui::DragValue::new(&mut state.target_lufs).speed(0.5).clamp_range(-30.0..=0.0).suffix(" LUFS"));
        ui.add(egui::DragValue::new(&mut state.target_true_peak_dbtp).speed(0.1).clamp_range(-6.0..=0.0).suffix(" dBTP"));
        ui.label("Preset:");
        for preset in &["Streaming", "CD", "Vinyl", "Radio", "Film"] {
            if ui.selectable_label(state.format_preset == *preset, *preset).clicked() { state.format_preset = preset.to_string(); }
        }
        if ui.button("+ Stage").clicked() {
            state.stages.push(MasteringChainStage::new("New Stage", "Generic"));
        }
    });
    ui.separator();

    draw_mastering_signal_flow(ui, state);
    ui.separator();

    let mut remove = None;
    let n = state.stages.len();
    for (i, stage) in state.stages.iter_mut().enumerate() {
        let sel = state.selected == Some(i);
        ui.horizontal(|ui| {
            let col = if stage.bypass { Color32::GRAY } else { Color32::WHITE };
            if ui.selectable_label(sel, egui::RichText::new(format!("{:2}. {}", i + 1, stage.name)).color(col)).clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
            ui.label(egui::RichText::new(format!("[{}]", stage.stage_type)).small().color(Color32::from_rgb(120, 140, 200)));
            ui.add(egui::DragValue::new(&mut stage.gain_out_db).speed(0.1).clamp_range(-12.0..=12.0).suffix(" dB").prefix("G: "));
            let pre_t = ((stage.metering_pre_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let post_t = ((stage.metering_post_db + 60.0) / 60.0).clamp(0.0, 1.0);
            let (mr, _) = ui.allocate_exact_size(Vec2::new(40.0, 12.0), egui::Sense::hover());
            let mp = ui.painter_at(mr);
            mp.rect_filled(mr, 1.0, Color32::from_rgb(20, 22, 28));
            mp.rect_filled(Rect::from_min_size(mr.min, Vec2::new(pre_t * 40.0, 5.0)), 0.0, Color32::from_rgb(60, 140, 200));
            mp.rect_filled(Rect::from_min_size(Pos2::new(mr.left(), mr.top() + 7.0), Vec2::new(post_t * 40.0, 5.0)), 0.0, Color32::from_rgb(80, 220, 100));
            ui.checkbox(&mut stage.enabled, "");
            ui.checkbox(&mut stage.bypass, "BP");
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.stages.remove(ri); if state.selected == Some(ri) { state.selected = None; } }

    if let Some(idx) = state.selected {
        if idx < state.stages.len() {
            ui.separator();
            let stage = &mut state.stages[idx];
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut stage.name); });
            ui.horizontal(|ui| {
                ui.label("In Gain:"); ui.add(egui::DragValue::new(&mut stage.gain_in_db).speed(0.1).clamp_range(-24.0..=24.0).suffix(" dB"));
                ui.label("Out Gain:"); ui.add(egui::DragValue::new(&mut stage.gain_out_db).speed(0.1).clamp_range(-24.0..=24.0).suffix(" dB"));
            });
        }
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("In: {:.1} dB", state.input_level_db));
        ui.label("→");
        ui.label(format!("Out: {:.1} dB", state.output_level_db));
        let target_diff = state.output_level_db - state.target_lufs;
        let col = if target_diff.abs() < 1.0 { Color32::from_rgb(80, 220, 100) } else { Color32::YELLOW };
        ui.colored_label(col, format!("Target diff: {:+.1} LU", target_diff));
    });
}

pub fn draw_mastering_signal_flow(ui: &mut egui::Ui, state: &MasteringChainState) {
    let n = state.stages.len();
    if n == 0 { return; }
    let box_w = 56.0f32;
    let gap = 14.0f32;
    let total_w = (box_w + gap) * n as f32 + 40.0 + gap * 2.0;
    let size = Vec2::new(total_w.min(ui.available_width()), 36.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));

    let cy = rect.center().y;
    let mut x = rect.left() + 8.0;
    p.text(Pos2::new(x, cy), egui::Align2::LEFT_CENTER, "IN", FontId::monospace(9.0), Color32::GRAY);
    x += 20.0;

    let type_colors: &[(&str, Color32)] = &[
        ("EQ", Color32::from_rgb(80, 160, 255)),
        ("Dynamics", Color32::from_rgb(255, 140, 60)),
        ("Stereo", Color32::from_rgb(80, 220, 160)),
        ("Saturation", Color32::from_rgb(220, 80, 160)),
        ("Limiter", Color32::from_rgb(255, 60, 60)),
        ("Generic", Color32::GRAY),
    ];

    for stage in &state.stages {
        // Arrow
        p.line_segment([Pos2::new(x, cy), Pos2::new(x + gap - 4.0, cy)], Stroke::new(1.0, Color32::from_rgb(80, 90, 110)));
        p.line_segment([Pos2::new(x + gap - 4.0, cy), Pos2::new(x + gap - 8.0, cy - 3.0)], Stroke::new(1.0, Color32::from_rgb(80, 90, 110)));
        p.line_segment([Pos2::new(x + gap - 4.0, cy), Pos2::new(x + gap - 8.0, cy + 3.0)], Stroke::new(1.0, Color32::from_rgb(80, 90, 110)));
        x += gap;
        let col = type_colors.iter().find(|(t, _)| stage.stage_type == *t).map(|(_, c)| *c).unwrap_or(Color32::GRAY);
        let box_rect = Rect::from_min_size(Pos2::new(x, cy - 11.0), Vec2::new(box_w, 22.0));
        let alpha = if stage.bypass { 60u8 } else { 180u8 };
        p.rect_filled(box_rect, 3.0, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), alpha / 4));
        p.rect_stroke(box_rect, 3.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), alpha)));
        let lbl: String = stage.name.chars().take(7).collect();
        p.text(box_rect.center(), egui::Align2::CENTER_CENTER, lbl, FontId::monospace(8.0),
            if stage.bypass { Color32::GRAY } else { Color32::WHITE });
        x += box_w;
    }

    p.line_segment([Pos2::new(x, cy), Pos2::new(x + 16.0, cy)], Stroke::new(1.0, Color32::from_rgb(80, 90, 110)));
    p.text(Pos2::new(x + 18.0, cy), egui::Align2::LEFT_CENTER, "OUT", FontId::monospace(9.0), Color32::GRAY);
}

// --- Audio Notification System ---------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum AudioNotificationType {
    Info,
    Warning,
    Error,
    Success,
    Beat,
}

impl AudioNotificationType {
    pub fn color(&self) -> Color32 {
        match self {
            AudioNotificationType::Info => Color32::from_rgb(80, 160, 255),
            AudioNotificationType::Warning => Color32::YELLOW,
            AudioNotificationType::Error => Color32::RED,
            AudioNotificationType::Success => Color32::from_rgb(80, 220, 100),
            AudioNotificationType::Beat => Color32::from_rgb(255, 180, 60),
        }
    }
    pub fn icon(&self) -> &str {
        match self {
            AudioNotificationType::Info => "ℹ",
            AudioNotificationType::Warning => "⚠",
            AudioNotificationType::Error => "✗",
            AudioNotificationType::Success => "✓",
            AudioNotificationType::Beat => "♪",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioNotification {
    pub message: String,
    pub notif_type: AudioNotificationType,
    pub timestamp: f32,
    pub duration_s: f32,
    pub dismissed: bool,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioNotificationState {
    pub notifications: Vec<AudioNotification>,
    pub max_notifications: usize,
    pub show_info: bool,
    pub show_warnings: bool,
    pub show_errors: bool,
}

impl AudioNotificationState {
    pub fn new() -> Self {
        Self { max_notifications: 50, show_info: true, show_warnings: true, show_errors: true, ..Default::default() }
    }
    pub fn push(&mut self, msg: &str, t: AudioNotificationType, duration: f32) {
        if self.notifications.len() >= self.max_notifications { self.notifications.remove(0); }
        self.notifications.push(AudioNotification {
            message: msg.to_string(), notif_type: t, timestamp: 0.0, duration_s: duration, dismissed: false,
        });
    }
}

pub fn show_audio_notifications(ui: &mut egui::Ui, state: &mut AudioNotificationState) {
    ui.heading("Audio Notifications");
    ui.separator();
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_info, "Info");
        ui.checkbox(&mut state.show_warnings, "Warn");
        ui.checkbox(&mut state.show_errors, "Error");
        if ui.button("Clear All").clicked() { state.notifications.retain(|n| n.notif_type == AudioNotificationType::Error); }
        if ui.button("Dismiss All").clicked() { for n in &mut state.notifications { n.dismissed = true; } }
    });
    ui.separator();
    egui::ScrollArea::vertical().max_height(200.0).stick_to_bottom(true).show(ui, |ui| {
        for notif in state.notifications.iter_mut().rev() {
            if notif.dismissed { continue; }
            if !state.show_info && notif.notif_type == AudioNotificationType::Info { continue; }
            if !state.show_warnings && notif.notif_type == AudioNotificationType::Warning { continue; }
            if !state.show_errors && notif.notif_type == AudioNotificationType::Error { continue; }
            ui.horizontal(|ui| {
                ui.colored_label(notif.notif_type.color(), notif.notif_type.icon());
                ui.label(egui::RichText::new(&notif.message).small().color(notif.notif_type.color()));
                if ui.small_button("✗").clicked() { notif.dismissed = true; }
            });
        }
    });
}

// --- Complete Audio Mixer State (everything) ------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioMixerCompleteState {
    pub full: AudioMixerFullState,
    pub voice_leading: VoiceLeadingState,
    pub undo: AudioUndoState,
    pub sidechain: SidechainVisualizerState,
    pub session_manager: AudioSessionManager,
    pub preferences: AudioPreferences,
    pub debug_overlay: AudioDebugOverlay,
    pub convolution: ConvolutionEngineState,
    pub mastering: MasteringChainState,
    pub notifications: AudioNotificationState,
    pub selected_panel: u8,
}

impl AudioMixerCompleteState {
    pub fn new() -> Self {
        Self {
            full: AudioMixerFullState::new(),
            undo: AudioUndoState::new(),
            session_manager: AudioSessionManager::with_demo(),
            notifications: AudioNotificationState::new(),
            mastering: MasteringChainState::standard(),
            ..Default::default()
        }
    }
}

pub fn show_audio_mixer_complete(ui: &mut egui::Ui, state: &mut AudioMixerCompleteState) {
    ui.heading("Audio Mixer — Complete");
    ui.separator();

    let panels = [
        "Main", "Voice Lead", "Undo", "Sidechain", "Sessions",
        "Prefs", "Debug", "Convolution", "Mastering", "Notifications",
    ];

    ui.horizontal_wrapped(|ui| {
        for (i, panel) in panels.iter().enumerate() {
            if ui.selectable_label(state.selected_panel == i as u8, *panel).clicked() {
                state.selected_panel = i as u8;
            }
        }
    });
    ui.separator();

    match state.selected_panel {
        0 => show_audio_mixer_full(ui, &mut state.full),
        1 => show_voice_leading_analyzer(ui, &mut state.voice_leading),
        2 => show_audio_undo_panel(ui, &mut state.undo),
        3 => show_sidechain_visualizer(ui, &mut state.sidechain),
        4 => show_audio_session_manager(ui, &mut state.session_manager),
        5 => show_audio_preferences(ui, &mut state.preferences),
        6 => show_audio_debug_overlay(ui, &mut state.debug_overlay),
        7 => show_convolution_engine(ui, &mut state.convolution),
        8 => show_mastering_chain(ui, &mut state.mastering),
        9 => show_audio_notifications(ui, &mut state.notifications),
        _ => {}
    }
}

// --- Audio Mixer Constants Summary -----------------------------------------------

pub const AUDIO_MIXER_TOTAL_PANELS: usize = 43;
pub const AUDIO_MIXER_TOTAL_STRUCTS: usize = 95;
pub const AUDIO_MIXER_TOTAL_FUNCTIONS: usize = 120;
pub const AUDIO_MIXER_VERSION: &str = "2.0.0";
pub const AUDIO_MIXER_BUILD_DATE: &str = "2026-03-31";

pub fn audio_mixer_build_info() -> String {
    format!(
        "Audio Mixer v{} built {} | {} panels | {} structs | {} functions",
        AUDIO_MIXER_VERSION, AUDIO_MIXER_BUILD_DATE,
        AUDIO_MIXER_TOTAL_PANELS, AUDIO_MIXER_TOTAL_STRUCTS, AUDIO_MIXER_TOTAL_FUNCTIONS
    )
}

// Additional standalone utility functions for audio processing

pub fn compute_true_peak_oversample(samples: &[f32], oversample: u8) -> f32 {
    let factor = oversample.max(1) as usize;
    let mut max = 0.0f32;
    for i in 0..samples.len() * factor {
        let t = i as f32 / factor as f32;
        let idx = t as usize;
        let frac = t - idx as f32;
        let a = samples.get(idx).copied().unwrap_or(0.0);
        let b = samples.get(idx + 1).copied().unwrap_or(a);
        let v = a + (b - a) * frac;
        max = max.max(v.abs());
    }
    max
}

pub fn normalize_block(samples: &mut [f32], target_db: f32) {
    let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak > 1e-10 {
        let gain = db_to_linear(target_db) / peak;
        for s in samples { *s *= gain; }
    }
}

pub fn apply_fade_in(samples: &mut [f32], fade_len: usize) {
    for (i, s) in samples.iter_mut().take(fade_len).enumerate() {
        let t = i as f32 / fade_len as f32;
        *s *= t * t;
    }
}

pub fn apply_fade_out(samples: &mut [f32], fade_len: usize) {
    let n = samples.len();
    for i in 0..fade_len.min(n) {
        let t = 1.0 - i as f32 / fade_len as f32;
        samples[n - 1 - i] *= t * t;
    }
}

pub fn apply_gain_ramp(samples: &mut [f32], start_db: f32, end_db: f32) {
    let n = samples.len();
    if n == 0 { return; }
    let start_lin = db_to_linear(start_db);
    let end_lin = db_to_linear(end_db);
    for (i, s) in samples.iter_mut().enumerate() {
        let t = i as f32 / n as f32;
        let gain = start_lin + (end_lin - start_lin) * t;
        *s *= gain;
    }
}

pub fn stereo_to_ms(left: f32, right: f32) -> (f32, f32) {
    ((left + right) * 0.707, (left - right) * 0.707)
}

pub fn ms_to_stereo(mid: f32, side: f32) -> (f32, f32) {
    ((mid + side) * 0.707, (mid - side) * 0.707)
}

pub fn apply_stereo_width(left: &mut f32, right: &mut f32, width: f32) {
    let (m, s) = stereo_to_ms(*left, *right);
    let new_side = s * width;
    let (l, r) = ms_to_stereo(m, new_side);
    *left = l; *right = r;
}

pub fn biquad_lp_coefficients(freq: f32, q: f32, sample_rate: f32) -> [f32; 5] {
    let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let alpha = w0.sin() / (2.0 * q);
    let cos_w0 = w0.cos();
    let b0 = (1.0 - cos_w0) / 2.0;
    let b1 = 1.0 - cos_w0;
    let b2 = (1.0 - cos_w0) / 2.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

pub fn biquad_hp_coefficients(freq: f32, q: f32, sample_rate: f32) -> [f32; 5] {
    let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let alpha = w0.sin() / (2.0 * q);
    let cos_w0 = w0.cos();
    let b0 = (1.0 + cos_w0) / 2.0;
    let b1 = -(1.0 + cos_w0);
    let b2 = (1.0 + cos_w0) / 2.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

pub fn biquad_bell_coefficients(freq: f32, q: f32, gain_db: f32, sample_rate: f32) -> [f32; 5] {
    let a = 10.0f32.powf(gain_db / 40.0);
    let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let alpha = w0.sin() / (2.0 * q);
    let cos_w0 = w0.cos();
    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha / a;
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

pub fn biquad_process(x: f32, coeffs: &[f32; 5], state: &mut [f32; 2]) -> f32 {
    let y = coeffs[0] * x + coeffs[1] * state[0] + coeffs[2] * state[1]
          - coeffs[3] * state[0] - coeffs[4] * state[1];
    state[1] = state[0];
    state[0] = x;
    y
}

pub fn compressor_gain_computer(input_db: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
    let over = input_db - threshold;
    if over <= -knee * 0.5 {
        0.0
    } else if over < knee * 0.5 {
        let t = (over + knee * 0.5) / knee;
        (1.0 / ratio - 1.0) * over * t * 0.5
    } else {
        (1.0 / ratio - 1.0) * over
    }
}

pub fn expander_gain_computer(input_db: f32, threshold: f32, ratio: f32) -> f32 {
    let under = threshold - input_db;
    if under > 0.0 { -(ratio - 1.0) * under } else { 0.0 }
}

pub fn smooth_log_envelope(current_db: f32, target_db: f32, attack_coeff: f32, release_coeff: f32) -> f32 {
    if target_db > current_db {
        current_db + (target_db - current_db) * attack_coeff
    } else {
        current_db + (target_db - current_db) * release_coeff
    }
}

pub fn attack_release_coefficients(attack_ms: f32, release_ms: f32, sample_rate: f32) -> (f32, f32) {
    let attack = if attack_ms > 0.0 { 1.0 - (-1.0 / (attack_ms * 0.001 * sample_rate)).exp() } else { 1.0 };
    let release = if release_ms > 0.0 { 1.0 - (-1.0 / (release_ms * 0.001 * sample_rate)).exp() } else { 1.0 };
    (attack, release)
}

pub fn fft_magnitude_db(real: f32, imag: f32) -> f32 {
    linear_to_db((real * real + imag * imag).sqrt().max(1e-10))
}

pub fn hann_window(n: usize, i: usize) -> f32 {
    0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos())
}

pub fn hamming_window(n: usize, i: usize) -> f32 {
    0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos()
}

pub fn blackman_window(n: usize, i: usize) -> f32 {
    let t = 2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32;
    0.42 - 0.5 * t.cos() + 0.08 * (2.0 * t).cos()
}

pub fn audio_mixer_panel_list() -> Vec<(&'static str, &'static str)> {
    vec![
        ("MIDI Sequencer", "Piano roll, 88 keys, quantize, transport"),
        ("Automation Lanes", "Cubic hermite, linear, step curves"),
        ("Spectrum Analyzer", "128+ bins, waterfall, log/mel/linear scale"),
        ("Dynamics Processor", "Compressor/Limiter/Expander/NoiseGate/DeEsser/Transient"),
        ("Effect Chain", "VST/built-in chain with A/B compare, signal flow diagram"),
        ("Audio Events", "OnCollision/Beat/Timer/GameEvent triggers"),
        ("3D Spatialization", "AudioSource3D, attenuation models, doppler, HRTF"),
        ("Audio Scripting", "Lua/Wren scripting, API reference"),
        ("Multiband EQ", "7+ bands, log freq display, interactive curve"),
        ("Bus Routing", "Matrix view, pre/post fader routes"),
        ("Waveform Editor", "Cut/copy/paste/fade/normalize, zoom, time ruler"),
        ("Mix Scenes", "Snapshot/recall with crossfade"),
        ("Metering", "K-14/K-20/VU/PPM, vertical/horizontal/mini layouts"),
        ("Timeline", "Multi-track audio clip timeline with fade triangles"),
        ("File Browser", "WAV/OGG/MP3/FLAC with waveform preview, favorites"),
        ("Reverb Designer", "Schroeder/FDN/Plate/Room/Hall/Spring/Convolution"),
        ("Oscilloscope", "Lissajous/Waveform/XY/Goniometer modes"),
        ("LUFS Meter", "EBU R128, momentary/short-term/integrated, history plot"),
        ("BPM Sync", "Tap tempo, MIDI Clock, Ableton Link, beat display"),
        ("Granular Synth", "Grain cloud viz, position/size/density/window controls"),
        ("IR Browser", "22 IRs categorized, waveform preview, T60 display"),
        ("Delay Designer", "Mono/Stereo/PingPong/Haas/Multi-tap, tap visualizer"),
        ("Chord & Scale Analyzer", "Piano keyboard with scale overlay, diatonic chord chart"),
        ("Sampler", "Zone mapping, keyboard view, ADSR, loop modes"),
        ("Step Sequencer", "16-row, 8/16/32 steps, swing, pattern chain"),
        ("Chord Sequencer", "Chord steps, arpeggio, diatonic chord builder"),
        ("Bus Compressor Overview", "Per-bus GR meters in grid"),
        ("Pitch Shifter/Harmonizer", "Phase vocoder/WSOLA, multi-voice harmonizer"),
        ("DJ Mixer", "Deck A/B, crossfader curve, EQ, filter, FX sends"),
        ("Test Tones", "Sine/Square/Noise/Sweep/Dirac/Comb generator"),
        ("Room Acoustic Simulator", "Sabine RT60, room modes, ray tracing top view"),
        ("MIDI CC Mapper", "CC -> param mapping with curve, learn mode"),
        ("Ambisonic Panner", "1st/2nd/3rd order, sphere view, HRIR decode"),
        ("Plugin Rack", "VST2/3/AU/CLAP list, bypass, wet/dry, preset"),
        ("Voice Leading Analyzer", "Parallel fifths/octaves detection, motion stats"),
        ("Audio History/Undo", "100-level undo with panel/action tracking"),
        ("Sidechain Visualizer", "Source envelope + GR history dual strip"),
        ("Session Manager", "Multi-session with BPM/track/duration overview"),
        ("Audio Preferences", "Device/SR/buffer/plugin paths/theme/auto-save"),
        ("Audio Debug Overlay", "Voices/CPU/latency/xruns/MIDI activity HUD"),
        ("Convolution Engine", "IR waveform, trim/stretch/reverse, partition control"),
        ("Mastering Chain", "9-stage chain with signal flow + in/out metering"),
        ("Audio Notifications", "Info/Warn/Error/Success/Beat notification log"),
    ]
}

#[inline]
fn db_to_linear(db: f32) -> f32 { 10.0f32.powf(db / 20.0) }
#[inline]
fn linear_to_db(linear: f32) -> f32 { 20.0 * linear.max(1e-10).log10() }
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
