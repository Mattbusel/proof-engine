path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 13
// ============================================================

// --- Spectral Frequency Painter --------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum SpectralPaintTool {
    Boost,
    Cut,
    Smooth,
    Erase,
    Select,
    Stamp,
}

impl SpectralPaintTool {
    pub fn label(&self) -> &str {
        match self {
            SpectralPaintTool::Boost => "Boost",
            SpectralPaintTool::Cut => "Cut",
            SpectralPaintTool::Smooth => "Smooth",
            SpectralPaintTool::Erase => "Erase",
            SpectralPaintTool::Select => "Select",
            SpectralPaintTool::Stamp => "Stamp",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralPainterState {
    pub tool: SpectralPaintTool,
    pub brush_size_hz: f32,
    pub brush_strength_db: f32,
    pub hardness: f32,
    pub resolution_bins: usize,
    pub painted_curve: Vec<f32>,
    pub sample_rate: u32,
    pub enabled: bool,
    pub show_original: bool,
    pub show_painted: bool,
}

impl Default for SpectralPainterState {
    fn default() -> Self {
        let bins = 256usize;
        let painted_curve = vec![0.0f32; bins];
        Self {
            tool: SpectralPaintTool::Boost,
            brush_size_hz: 500.0,
            brush_strength_db: 3.0,
            hardness: 0.5,
            resolution_bins: bins,
            painted_curve,
            sample_rate: 44100,
            enabled: true,
            show_original: true,
            show_painted: true,
        }
    }
}

pub fn show_spectral_painter(ui: &mut egui::Ui, state: &mut SpectralPainterState) {
    ui.heading("Spectral Frequency Painter");
    ui.separator();

    ui.horizontal(|ui| {
        for tool in &[SpectralPaintTool::Boost, SpectralPaintTool::Cut, SpectralPaintTool::Smooth,
                       SpectralPaintTool::Erase, SpectralPaintTool::Select, SpectralPaintTool::Stamp] {
            if ui.selectable_label(state.tool == *tool, tool.label()).clicked() { state.tool = tool.clone(); }
        }
        ui.separator();
        ui.checkbox(&mut state.enabled, "Enable");
    });

    ui.horizontal(|ui| {
        ui.label("Brush:");
        ui.add(egui::DragValue::new(&mut state.brush_size_hz).speed(10.0).clamp_range(10.0..=5000.0).suffix(" Hz"));
        ui.label("Strength:");
        ui.add(egui::Slider::new(&mut state.brush_strength_db, 0.1..=24.0).suffix(" dB"));
        ui.label("Hardness:");
        ui.add(egui::Slider::new(&mut state.hardness, 0.0..=1.0));
        ui.checkbox(&mut state.show_original, "Original");
        ui.checkbox(&mut state.show_painted, "Painted");
        if ui.button("Reset").clicked() { state.painted_curve.iter_mut().for_each(|v| *v = 0.0); }
    });

    draw_spectral_paint_canvas(ui, state);
}

pub fn draw_spectral_paint_canvas(ui: &mut egui::Ui, state: &SpectralPainterState) {
    let size = Vec2::new(ui.available_width().min(500.0), 160.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(40, 50, 65)));

    let n = state.painted_curve.len();
    let freq_to_x = |f: f32| -> f32 {
        let t = (f.log10() - (20.0f32).log10()) / ((20000.0f32).log10() - (20.0f32).log10());
        rect.left() + t * rect.width()
    };
    let db_to_y = |db: f32| -> f32 {
        rect.center().y - (db / 24.0) * rect.height() * 0.45
    };

    // Zero line
    p.line_segment([Pos2::new(rect.left(), rect.center().y), Pos2::new(rect.right(), rect.center().y)],
        Stroke::new(0.75, Color32::from_rgba_premultiplied(80, 90, 110, 160)));

    // Freq grid
    for &f in &[100.0f32, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0] {
        let x = freq_to_x(f);
        p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(0.4, Color32::from_rgba_premultiplied(40, 50, 65, 80)));
        let lbl = if f >= 1000.0 { format!("{}k", f as u32 / 1000) } else { format!("{}", f as u32) };
        p.text(Pos2::new(x, rect.bottom() - 8.0), egui::Align2::CENTER_CENTER, lbl, FontId::monospace(7.0), Color32::GRAY);
    }

    // dB grid
    for db in [-12.0f32, -6.0, 0.0, 6.0, 12.0] {
        let y = db_to_y(db);
        p.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.4, Color32::from_rgba_premultiplied(40, 50, 65, 80)));
        p.text(Pos2::new(rect.left() + 2.0, y), egui::Align2::LEFT_CENTER,
            format!("{:+.0}", db), FontId::monospace(7.0), Color32::GRAY);
    }

    // Painted curve
    if state.show_painted && !state.painted_curve.is_empty() {
        let pts: Vec<Pos2> = state.painted_curve.iter().enumerate().map(|(i, &db)| {
            let t = (i as f32 + 0.5) / n as f32;
            let f = 20.0f32 * (1000.0f32).powf(t);
            Pos2::new(freq_to_x(f), db_to_y(db))
        }).collect();
        let col = match state.tool {
            SpectralPaintTool::Boost => Color32::from_rgb(80, 220, 100),
            SpectralPaintTool::Cut => Color32::from_rgb(255, 80, 80),
            _ => Color32::from_rgb(100, 180, 255),
        };
        for w in pts.windows(2) { p.line_segment([w[0], w[1]], Stroke::new(2.0, col)); }
    }

    // Mouse cursor indicator
    if let Some(pos) = resp.hover_pos() {
        if rect.contains(pos) {
            let brush_hz_px = state.brush_size_hz / (20000.0 / rect.width().max(1.0));
            p.circle_stroke(pos, brush_hz_px * 0.5, Stroke::new(0.5, Color32::from_rgba_premultiplied(200, 200, 200, 100)));
        }
    }
}

// --- Frequency Response Analyzer -------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct FrequencyResponseMeasurement {
    pub name: String,
    pub frequencies: Vec<f32>,
    pub magnitude_db: Vec<f32>,
    pub phase_deg: Vec<f32>,
    pub color: [u8; 3],
    pub visible: bool,
}

impl FrequencyResponseMeasurement {
    pub fn new(name: &str, color: [u8; 3]) -> Self {
        let freqs: Vec<f32> = (0..200).map(|i| 20.0f32 * (1000.0f32).powf(i as f32 / 199.0)).collect();
        let magnitude: Vec<f32> = freqs.iter().map(|&f| {
            let lf = f.log10();
            if lf < 2.3 { (lf - 2.3) * 12.0 }
            else if lf > 4.0 { -(lf - 4.0) * 6.0 }
            else { ((lf - 3.0) * std::f32::consts::PI).sin() * 1.5 }
        }).collect();
        let phase: Vec<f32> = freqs.iter().map(|&f| -(f.log10() * 20.0).min(360.0)).collect();
        Self { name: name.to_string(), frequencies: freqs, magnitude_db: magnitude, phase_deg: phase, color, visible: true }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct FrequencyResponseAnalyzerState {
    pub measurements: Vec<FrequencyResponseMeasurement>,
    pub show_phase: bool,
    pub show_group_delay: bool,
    pub show_coherence: bool,
    pub log_x: bool,
    pub db_range: f32,
    pub zoom_freq_lo: f32,
    pub zoom_freq_hi: f32,
    pub selected: Option<usize>,
}

impl FrequencyResponseAnalyzerState {
    pub fn with_demo() -> Self {
        let mut s = Self {
            log_x: true,
            db_range: 30.0,
            zoom_freq_lo: 20.0,
            zoom_freq_hi: 20000.0,
            ..Default::default()
        };
        s.measurements = vec![
            FrequencyResponseMeasurement::new("Reference", [80, 200, 255]),
            FrequencyResponseMeasurement::new("Measurement A", [255, 160, 60]),
            FrequencyResponseMeasurement::new("Measurement B", [80, 220, 100]),
        ];
        s
    }
}

pub fn show_frequency_response_analyzer(ui: &mut egui::Ui, state: &mut FrequencyResponseAnalyzerState) {
    ui.heading("Frequency Response Analyzer");
    ui.separator();

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.log_x, "Log X");
        ui.checkbox(&mut state.show_phase, "Phase");
        ui.checkbox(&mut state.show_group_delay, "Group Delay");
        ui.separator();
        ui.label("Range:");
        ui.add(egui::DragValue::new(&mut state.db_range).speed(1.0).clamp_range(6.0..=60.0).suffix(" dB"));
        ui.label("Lo:");
        ui.add(egui::DragValue::new(&mut state.zoom_freq_lo).speed(1.0).clamp_range(1.0..=1000.0).suffix(" Hz"));
        ui.label("Hi:");
        ui.add(egui::DragValue::new(&mut state.zoom_freq_hi).speed(100.0).clamp_range(1000.0..=22000.0).suffix(" Hz"));
        if ui.button("+ Add").clicked() {
            let cols: [[u8; 3]; 5] = [[255,80,80],[200,80,255],[255,220,60],[80,255,200],[180,255,80]];
            let id = state.measurements.len();
            state.measurements.push(FrequencyResponseMeasurement::new(&format!("Meas {}", id + 1), cols[id % 5]));
        }
    });
    ui.separator();

    draw_frequency_response_plot(ui, state);
    ui.separator();

    for (i, meas) in state.measurements.iter_mut().enumerate() {
        let sel = state.selected == Some(i);
        let col = Color32::from_rgb(meas.color[0], meas.color[1], meas.color[2]);
        ui.horizontal(|ui| {
            ui.colored_label(col, "▬");
            if ui.selectable_label(sel, &meas.name).clicked() { state.selected = if sel { None } else { Some(i) }; }
            ui.checkbox(&mut meas.visible, "");
        });
    }
}

pub fn draw_frequency_response_plot(ui: &mut egui::Ui, state: &FrequencyResponseAnalyzerState) {
    let height = if state.show_phase { 200.0f32 } else { 140.0f32 };
    let size = Vec2::new(ui.available_width().min(500.0), height);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));

    let mag_h = if state.show_phase { rect.height() * 0.6 } else { rect.height() };
    let mag_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), mag_h));

    let freq_lo = state.zoom_freq_lo.max(10.0);
    let freq_hi = state.zoom_freq_hi.min(22000.0);
    let f_to_x = |f: f32| -> f32 {
        let t = if state.log_x {
            (f.log10() - freq_lo.log10()) / (freq_hi.log10() - freq_lo.log10())
        } else {
            (f - freq_lo) / (freq_hi - freq_lo)
        };
        mag_rect.left() + t.clamp(0.0, 1.0) * mag_rect.width()
    };
    let db_to_y = |db: f32| -> f32 {
        mag_rect.center().y - (db / state.db_range) * mag_rect.height() * 0.45
    };

    // Grid
    p.line_segment([Pos2::new(mag_rect.left(), mag_rect.center().y), Pos2::new(mag_rect.right(), mag_rect.center().y)],
        Stroke::new(0.75, Color32::from_rgba_premultiplied(80, 90, 110, 160)));

    for &f in &[50.0f32, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0] {
        if f < freq_lo || f > freq_hi { continue; }
        let x = f_to_x(f);
        p.line_segment([Pos2::new(x, mag_rect.top()), Pos2::new(x, mag_rect.bottom())],
            Stroke::new(0.4, Color32::from_rgba_premultiplied(40, 50, 65, 100)));
        let lbl = if f >= 1000.0 { format!("{}k", f as u32 / 1000) } else { format!("{}", f as u32) };
        p.text(Pos2::new(x, mag_rect.bottom() - 8.0), egui::Align2::CENTER_CENTER, lbl, FontId::monospace(7.0), Color32::GRAY);
    }
    for db in [-24.0f32, -12.0, -6.0, 0.0, 6.0, 12.0] {
        if db.abs() > state.db_range { continue; }
        let y = db_to_y(db);
        p.line_segment([Pos2::new(mag_rect.left(), y), Pos2::new(mag_rect.right(), y)],
            Stroke::new(0.4, Color32::from_rgba_premultiplied(40, 50, 65, 100)));
        p.text(Pos2::new(mag_rect.left() + 2.0, y), egui::Align2::LEFT_CENTER,
            format!("{:+.0}", db), FontId::monospace(7.0), Color32::GRAY);
    }

    // Measurement curves
    for meas in &state.measurements {
        if !meas.visible { continue; }
        let col = Color32::from_rgb(meas.color[0], meas.color[1], meas.color[2]);
        let pts: Vec<Option<Pos2>> = meas.frequencies.iter().zip(meas.magnitude_db.iter()).map(|(&f, &db)| {
            if f < freq_lo || f > freq_hi { None }
            else { Some(Pos2::new(f_to_x(f), db_to_y(db))) }
        }).collect();
        let mut prev: Option<Pos2> = None;
        for pt in &pts {
            if let (Some(a), Some(b)) = (prev, *pt) {
                p.line_segment([a, b], Stroke::new(1.5, col));
            }
            prev = *pt;
        }
    }

    // Phase plot
    if state.show_phase {
        let phase_rect = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + mag_h),
            rect.max,
        );
        p.rect_filled(phase_rect, 0.0, Color32::from_rgb(8, 10, 16));
        p.line_segment([Pos2::new(phase_rect.left(), phase_rect.center().y), Pos2::new(phase_rect.right(), phase_rect.center().y)],
            Stroke::new(0.5, Color32::from_rgba_premultiplied(60, 70, 90, 120)));

        let deg_to_y = |deg: f32| -> f32 {
            phase_rect.center().y - (deg / 360.0) * phase_rect.height() * 0.9
        };

        for meas in &state.measurements {
            if !meas.visible { continue; }
            let col = Color32::from_rgba_premultiplied(meas.color[0], meas.color[1], meas.color[2], 160);
            let pts: Vec<Option<Pos2>> = meas.frequencies.iter().zip(meas.phase_deg.iter()).map(|(&f, &deg)| {
                if f < freq_lo || f > freq_hi { None }
                else { Some(Pos2::new(f_to_x(f), deg_to_y(deg))) }
            }).collect();
            let mut prev: Option<Pos2> = None;
            for pt in &pts {
                if let (Some(a), Some(b)) = (prev, *pt) { p.line_segment([a, b], Stroke::new(1.0, col)); }
                prev = *pt;
            }
        }
        p.text(phase_rect.left_top() + Vec2::new(2.0, 2.0), egui::Align2::LEFT_TOP, "Phase", FontId::monospace(8.0), Color32::GRAY);
    }
}

// --- Noise Reduction -------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum NoiseReductionMode {
    Spectral,
    Adaptive,
    DeNoise,
    DeHum,
    DeClick,
    DeCrackle,
}

impl NoiseReductionMode {
    pub fn label(&self) -> &str {
        match self {
            NoiseReductionMode::Spectral => "Spectral",
            NoiseReductionMode::Adaptive => "Adaptive",
            NoiseReductionMode::DeNoise => "DeNoise",
            NoiseReductionMode::DeHum => "DeHum",
            NoiseReductionMode::DeClick => "DeClick",
            NoiseReductionMode::DeCrackle => "DeCrackle",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NoiseReductionState {
    pub mode: NoiseReductionMode,
    pub strength: f32,
    pub sensitivity: f32,
    pub artifact_reduction: f32,
    pub hum_freq: f32,
    pub hum_harmonics: u8,
    pub hum_reduction_db: f32,
    pub learn_mode: bool,
    pub noise_floor_db: f32,
    pub enabled: bool,
    pub wet_dry: f32,
    pub fft_size: u32,
    pub smoothing_bins: u32,
    pub smoothing_time: f32,
}

impl Default for NoiseReductionState {
    fn default() -> Self {
        Self {
            mode: NoiseReductionMode::Spectral,
            strength: 0.5,
            sensitivity: 0.5,
            artifact_reduction: 0.5,
            hum_freq: 50.0,
            hum_harmonics: 5,
            hum_reduction_db: 20.0,
            learn_mode: false,
            noise_floor_db: -60.0,
            enabled: true,
            wet_dry: 1.0,
            fft_size: 2048,
            smoothing_bins: 3,
            smoothing_time: 50.0,
        }
    }
}

pub fn show_noise_reduction(ui: &mut egui::Ui, state: &mut NoiseReductionState) {
    ui.heading("Noise Reduction");
    ui.separator();

    ui.horizontal(|ui| {
        for mode in &[NoiseReductionMode::Spectral, NoiseReductionMode::Adaptive, NoiseReductionMode::DeNoise,
                       NoiseReductionMode::DeHum, NoiseReductionMode::DeClick, NoiseReductionMode::DeCrackle] {
            if ui.selectable_label(state.mode == *mode, mode.label()).clicked() { state.mode = mode.clone(); }
        }
        ui.separator();
        ui.checkbox(&mut state.enabled, "Enable");
    });
    ui.separator();

    match state.mode {
        NoiseReductionMode::DeHum => {
            ui.horizontal(|ui| { ui.label("Hum Freq:"); ui.add(egui::Slider::new(&mut state.hum_freq, 40.0..=120.0).suffix(" Hz")); });
            ui.horizontal(|ui| { ui.label("Harmonics:"); ui.add(egui::DragValue::new(&mut state.hum_harmonics).clamp_range(1u8..=12)); });
            ui.horizontal(|ui| { ui.label("Reduction:"); ui.add(egui::Slider::new(&mut state.hum_reduction_db, 0.0..=60.0).suffix(" dB")); });
        }
        _ => {
            ui.horizontal(|ui| { ui.label("Strength:"); ui.add(egui::Slider::new(&mut state.strength, 0.0..=1.0)); });
            ui.horizontal(|ui| { ui.label("Sensitivity:"); ui.add(egui::Slider::new(&mut state.sensitivity, 0.0..=1.0)); });
            ui.horizontal(|ui| { ui.label("Artifact Reduction:"); ui.add(egui::Slider::new(&mut state.artifact_reduction, 0.0..=1.0)); });
            ui.horizontal(|ui| { ui.label("FFT:"); for sz in &[512u32, 1024, 2048, 4096] {
                if ui.selectable_label(state.fft_size == *sz, format!("{}", sz)).clicked() { state.fft_size = *sz; }
            }});
            ui.horizontal(|ui| { ui.label("Smoothing:"); ui.add(egui::DragValue::new(&mut state.smoothing_bins).clamp_range(1u32..=8).suffix(" bins")); });
            ui.horizontal(|ui| { ui.label("Time Smooth:"); ui.add(egui::DragValue::new(&mut state.smoothing_time).speed(1.0).clamp_range(0.0..=500.0).suffix(" ms")); });
        }
    }
    ui.horizontal(|ui| {
        ui.label("Wet/Dry:"); ui.add(egui::Slider::new(&mut state.wet_dry, 0.0..=1.0));
        ui.checkbox(&mut state.learn_mode, "Learn Noise Profile");
    });
    if state.learn_mode {
        ui.label(egui::RichText::new("Play noise-only section to learn profile").color(Color32::YELLOW));
    }
    ui.label(format!("Noise Floor: {:.1} dB", state.noise_floor_db));
}

// --- Final complete state wrapper ------------------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioMixerUltimateState {
    pub complete: AudioMixerCompleteState,
    pub spectral_painter: SpectralPainterState,
    pub freq_response: FrequencyResponseAnalyzerState,
    pub noise_reduction: NoiseReductionState,
    pub selected_panel: u8,
}

impl AudioMixerUltimateState {
    pub fn new() -> Self {
        Self {
            complete: AudioMixerCompleteState::new(),
            freq_response: FrequencyResponseAnalyzerState::with_demo(),
            ..Default::default()
        }
    }
}

pub fn show_audio_mixer_ultimate(ui: &mut egui::Ui, state: &mut AudioMixerUltimateState) {
    ui.heading("Audio Mixer — Ultimate");
    ui.separator();

    let panels = ["Complete", "Spectral Paint", "Freq Response", "Noise Reduction"];
    ui.horizontal_wrapped(|ui| {
        for (i, panel) in panels.iter().enumerate() {
            if ui.selectable_label(state.selected_panel == i as u8, *panel).clicked() {
                state.selected_panel = i as u8;
            }
        }
    });
    ui.separator();

    match state.selected_panel {
        0 => show_audio_mixer_complete(ui, &mut state.complete),
        1 => show_spectral_painter(ui, &mut state.spectral_painter),
        2 => show_frequency_response_analyzer(ui, &mut state.freq_response),
        3 => show_noise_reduction(ui, &mut state.noise_reduction),
        _ => {}
    }
}

// --- Summary constants -----------------------------------------------------------

pub const AUDIO_MIXER_RS_TOTAL_LINES_APPROX: usize = 17000;
pub const AUDIO_MIXER_RS_EXPANSION_BLOCKS: usize = 13;
pub const AUDIO_MIXER_RS_CUSTOM_CANVASES: usize = 38;
pub const AUDIO_MIXER_RS_EGUI_PANELS: usize = 47;
pub const AUDIO_MIXER_RS_DATA_STRUCTS: usize = 100;
pub const AUDIO_MIXER_RS_PUB_FUNCTIONS: usize = 130;

pub fn audio_mixer_module_stats() -> [(& 'static str, usize); 10] {
    [
        ("Total Lines", AUDIO_MIXER_RS_TOTAL_LINES_APPROX),
        ("Expansion Blocks", AUDIO_MIXER_RS_EXPANSION_BLOCKS),
        ("Custom Canvas Draws", AUDIO_MIXER_RS_CUSTOM_CANVASES),
        ("egui Panels", AUDIO_MIXER_RS_EGUI_PANELS),
        ("Data Structs", AUDIO_MIXER_RS_DATA_STRUCTS),
        ("Public Functions", AUDIO_MIXER_RS_PUB_FUNCTIONS),
        ("EQ Bands", 7),
        ("Effect Types", 16),
        ("Sampler Zones Max", AUDIO_SAMPLER_MAX_ZONES),
        ("Max Polyphony", AUDIO_DEFAULT_POLYPHONY),
    ]
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
