path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 5
// ============================================================

// --- Reverb Designer -------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ReverbAlgorithm {
    Schroeder,
    FDN,
    Plate,
    Room,
    Hall,
    Spring,
    Convolution,
}

impl ReverbAlgorithm {
    pub fn label(&self) -> &str {
        match self {
            ReverbAlgorithm::Schroeder => "Schroeder",
            ReverbAlgorithm::FDN => "FDN",
            ReverbAlgorithm::Plate => "Plate",
            ReverbAlgorithm::Room => "Room",
            ReverbAlgorithm::Hall => "Hall",
            ReverbAlgorithm::Spring => "Spring",
            ReverbAlgorithm::Convolution => "Convolution",
        }
    }
}

impl Default for ReverbAlgorithm { fn default() -> Self { ReverbAlgorithm::FDN } }

#[derive(Clone, Serialize, Deserialize)]
pub struct ReverbDesigner {
    pub algorithm: ReverbAlgorithm,
    pub pre_delay_ms: f32,
    pub decay_time_s: f32,
    pub room_size: f32,
    pub diffusion: f32,
    pub damping: f32,
    pub low_shelf_freq: f32,
    pub low_shelf_gain_db: f32,
    pub high_shelf_freq: f32,
    pub high_shelf_gain_db: f32,
    pub early_level_db: f32,
    pub late_level_db: f32,
    pub stereo_width: f32,
    pub modulation_rate: f32,
    pub modulation_depth: f32,
    pub tail_density: f32,
    pub enabled: bool,
    pub wet_db: f32,
    pub dry_db: f32,
    pub impulse_name: String,
}

impl Default for ReverbDesigner {
    fn default() -> Self {
        Self {
            algorithm: ReverbAlgorithm::FDN,
            pre_delay_ms: 15.0,
            decay_time_s: 2.0,
            room_size: 0.6,
            diffusion: 0.7,
            damping: 0.4,
            low_shelf_freq: 200.0,
            low_shelf_gain_db: 2.0,
            high_shelf_freq: 6000.0,
            high_shelf_gain_db: -3.0,
            early_level_db: -6.0,
            late_level_db: 0.0,
            stereo_width: 1.0,
            modulation_rate: 0.5,
            modulation_depth: 0.2,
            tail_density: 0.8,
            enabled: true,
            wet_db: -6.0,
            dry_db: 0.0,
            impulse_name: "hall_large.wav".into(),
        }
    }
}

pub fn show_reverb_designer(ui: &mut egui::Ui, state: &mut ReverbDesigner) {
    ui.heading("Reverb Designer");
    ui.separator();

    ui.horizontal(|ui| {
        for algo in &[ReverbAlgorithm::Schroeder, ReverbAlgorithm::FDN, ReverbAlgorithm::Plate,
                       ReverbAlgorithm::Room, ReverbAlgorithm::Hall, ReverbAlgorithm::Spring, ReverbAlgorithm::Convolution] {
            if ui.selectable_label(state.algorithm == *algo, algo.label()).clicked() { state.algorithm = algo.clone(); }
        }
        ui.separator();
        ui.checkbox(&mut state.enabled, "Enable");
    });
    ui.separator();

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new("Time & Space").strong());
        ui.horizontal(|ui| { ui.label("Pre-delay:"); ui.add(egui::Slider::new(&mut state.pre_delay_ms, 0.0..=100.0).suffix(" ms")); });
        ui.horizontal(|ui| { ui.label("Decay:"); ui.add(egui::Slider::new(&mut state.decay_time_s, 0.1..=20.0).suffix(" s").logarithmic(true)); });
        ui.horizontal(|ui| { ui.label("Room Size:"); ui.add(egui::Slider::new(&mut state.room_size, 0.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Diffusion:"); ui.add(egui::Slider::new(&mut state.diffusion, 0.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Damping:"); ui.add(egui::Slider::new(&mut state.damping, 0.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Tail Density:"); ui.add(egui::Slider::new(&mut state.tail_density, 0.0..=1.0)); });

        ui.separator();
        ui.label(egui::RichText::new("Tone EQ").strong());
        ui.horizontal(|ui| {
            ui.label("Low Shelf:");
            ui.add(egui::DragValue::new(&mut state.low_shelf_freq).speed(5.0).clamp_range(20.0..=500.0).suffix(" Hz"));
            ui.add(egui::DragValue::new(&mut state.low_shelf_gain_db).speed(0.1).clamp_range(-12.0..=12.0).suffix(" dB"));
        });
        ui.horizontal(|ui| {
            ui.label("High Shelf:");
            ui.add(egui::DragValue::new(&mut state.high_shelf_freq).speed(100.0).clamp_range(2000.0..=20000.0).suffix(" Hz"));
            ui.add(egui::DragValue::new(&mut state.high_shelf_gain_db).speed(0.1).clamp_range(-24.0..=6.0).suffix(" dB"));
        });

        let ui = &mut cols[1];
        ui.label(egui::RichText::new("Mix & Width").strong());
        ui.horizontal(|ui| { ui.label("Wet:"); ui.add(egui::Slider::new(&mut state.wet_db, -60.0..=6.0).suffix(" dB")); });
        ui.horizontal(|ui| { ui.label("Dry:"); ui.add(egui::Slider::new(&mut state.dry_db, -60.0..=6.0).suffix(" dB")); });
        ui.horizontal(|ui| { ui.label("Early:"); ui.add(egui::Slider::new(&mut state.early_level_db, -24.0..=6.0).suffix(" dB")); });
        ui.horizontal(|ui| { ui.label("Late:"); ui.add(egui::Slider::new(&mut state.late_level_db, -24.0..=6.0).suffix(" dB")); });
        ui.horizontal(|ui| { ui.label("Stereo Width:"); ui.add(egui::Slider::new(&mut state.stereo_width, 0.0..=2.0)); });

        ui.separator();
        ui.label(egui::RichText::new("Modulation").strong());
        ui.horizontal(|ui| { ui.label("Rate:"); ui.add(egui::Slider::new(&mut state.modulation_rate, 0.0..=10.0).suffix(" Hz")); });
        ui.horizontal(|ui| { ui.label("Depth:"); ui.add(egui::Slider::new(&mut state.modulation_depth, 0.0..=1.0)); });

        if state.algorithm == ReverbAlgorithm::Convolution {
            ui.separator();
            ui.label(egui::RichText::new("Impulse Response").strong());
            ui.horizontal(|ui| { ui.label("File:"); ui.text_edit_singleline(&mut state.impulse_name); });
            if ui.button("Browse IR...").clicked() {}
        }

        ui.separator();
        draw_reverb_decay_curve(ui, state);
    });
}

pub fn draw_reverb_decay_curve(ui: &mut egui::Ui, state: &ReverbDesigner) {
    let size = Vec2::new(200.0, 100.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(40, 45, 60)));

    let max_t = state.decay_time_s * 1.5;
    let pre_delay_x = rect.left() + (state.pre_delay_ms / 1000.0 / max_t) * rect.width();

    p.line_segment([Pos2::new(pre_delay_x, rect.top()), Pos2::new(pre_delay_x, rect.bottom())],
        Stroke::new(0.5, Color32::from_rgba_premultiplied(255, 180, 60, 100)));

    // Early reflections
    for i in 0..8 {
        let t = state.pre_delay_ms / 1000.0 + i as f32 * 0.01 * state.room_size;
        let amp = 0.8 * (-(i as f32 * 0.3)).exp();
        let x = rect.left() + (t / max_t) * rect.width();
        let y0 = rect.center().y;
        let y1 = rect.center().y - amp * rect.height() * 0.4;
        if x < rect.right() {
            p.line_segment([Pos2::new(x, y0), Pos2::new(x, y1)], Stroke::new(1.5, Color32::from_rgb(100, 180, 255)));
        }
    }

    // Late reverb tail
    let n = 100usize;
    let pts: Vec<Pos2> = (0..n).map(|i| {
        let t = state.pre_delay_ms / 1000.0 + i as f32 / n as f32 * state.decay_time_s * 1.2;
        let db = -60.0 * (t - state.pre_delay_ms / 1000.0) / state.decay_time_s;
        let amp = 10.0f32.powf(db / 20.0);
        let x = rect.left() + (t / max_t).min(1.0) * rect.width();
        Pos2::new(x, rect.center().y - amp * rect.height() * 0.4)
    }).collect();

    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(80, 220, 160)));
    }

    p.text(rect.left_top() + Vec2::new(2.0, 2.0), egui::Align2::LEFT_TOP,
        format!("T60={:.1}s", state.decay_time_s), FontId::monospace(8.0), Color32::GRAY);
}

// --- Oscilloscope ----------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum OscilloscopeMode {
    Lissajous,
    Waveform,
    XY,
    Goniometer,
}

impl OscilloscopeMode {
    pub fn label(&self) -> &str {
        match self {
            OscilloscopeMode::Lissajous => "Lissajous",
            OscilloscopeMode::Waveform => "Waveform",
            OscilloscopeMode::XY => "X/Y",
            OscilloscopeMode::Goniometer => "Goniometer",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioOscilloscope {
    pub mode: OscilloscopeMode,
    pub time_base_ms: f32,
    pub trigger_level: f32,
    pub trigger_rising: bool,
    pub gain: f32,
    pub persistence: f32,
    pub channel_l_color: [u8; 3],
    pub channel_r_color: [u8; 3],
    pub show_grid: bool,
    pub invert_r: bool,
}

impl Default for AudioOscilloscope {
    fn default() -> Self {
        Self {
            mode: OscilloscopeMode::Waveform,
            time_base_ms: 20.0,
            trigger_level: 0.0,
            trigger_rising: true,
            gain: 1.0,
            persistence: 0.0,
            channel_l_color: [80, 180, 255],
            channel_r_color: [255, 160, 60],
            show_grid: true,
            invert_r: false,
        }
    }
}

pub fn show_oscilloscope(ui: &mut egui::Ui, state: &mut AudioOscilloscope) {
    ui.heading("Oscilloscope");
    ui.separator();

    ui.horizontal(|ui| {
        for mode in &[OscilloscopeMode::Waveform, OscilloscopeMode::Lissajous,
                       OscilloscopeMode::XY, OscilloscopeMode::Goniometer] {
            if ui.selectable_label(state.mode == *mode, mode.label()).clicked() { state.mode = mode.clone(); }
        }
        ui.separator();
        ui.checkbox(&mut state.show_grid, "Grid");
        ui.checkbox(&mut state.invert_r, "Invert R");
    });

    ui.horizontal(|ui| {
        ui.label("Time Base:");
        ui.add(egui::Slider::new(&mut state.time_base_ms, 1.0..=200.0).suffix(" ms").logarithmic(true));
        ui.label("Gain:");
        ui.add(egui::Slider::new(&mut state.gain, 0.1..=10.0).logarithmic(true));
        ui.label("Persistence:");
        ui.add(egui::Slider::new(&mut state.persistence, 0.0..=0.99));
    });
    ui.separator();

    draw_oscilloscope(ui, state);
}

pub fn draw_oscilloscope(ui: &mut egui::Ui, state: &AudioOscilloscope) {
    let size = Vec2::new(ui.available_width().min(400.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(5, 8, 12));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(40, 50, 65)));

    if state.show_grid {
        for i in 1..4 {
            let t = i as f32 / 4.0;
            let x = rect.left() + t * rect.width();
            let y = rect.top() + t * rect.height();
            p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 60, 80, 100)));
            p.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 60, 80, 100)));
        }
        // Center cross
        p.line_segment([Pos2::new(rect.left(), rect.center().y), Pos2::new(rect.right(), rect.center().y)],
            Stroke::new(0.75, Color32::from_rgba_premultiplied(60, 80, 100, 160)));
        p.line_segment([Pos2::new(rect.center().x, rect.top()), Pos2::new(rect.center().x, rect.bottom())],
            Stroke::new(0.75, Color32::from_rgba_premultiplied(60, 80, 100, 160)));
    }

    let n = 512usize;
    let col_l = Color32::from_rgb(state.channel_l_color[0], state.channel_l_color[1], state.channel_l_color[2]);
    let col_r = Color32::from_rgb(state.channel_r_color[0], state.channel_r_color[1], state.channel_r_color[2]);

    match state.mode {
        OscilloscopeMode::Waveform => {
            let pts_l: Vec<Pos2> = (0..n).map(|i| {
                let t = i as f32 / n as f32;
                let phase = t * std::f32::consts::TAU * 3.0 + 0.0;
                let v = (phase.sin() * 0.7 + (phase * 2.0).sin() * 0.2) * state.gain;
                Pos2::new(rect.left() + t * rect.width(), rect.center().y - v * rect.height() * 0.4)
            }).collect();
            let pts_r: Vec<Pos2> = (0..n).map(|i| {
                let t = i as f32 / n as f32;
                let phase = t * std::f32::consts::TAU * 3.0 + 0.3;
                let inv = if state.invert_r { -1.0 } else { 1.0 };
                let v = (phase.sin() * 0.65 + (phase * 2.1).sin() * 0.25) * state.gain * inv;
                Pos2::new(rect.left() + t * rect.width(), rect.center().y - v * rect.height() * 0.4)
            }).collect();
            for w in pts_l.windows(2) { p.line_segment([w[0], w[1]], Stroke::new(1.0, col_l)); }
            for w in pts_r.windows(2) { p.line_segment([w[0], w[1]], Stroke::new(1.0, col_r)); }
        }
        OscilloscopeMode::Lissajous | OscilloscopeMode::XY => {
            let pts: Vec<Pos2> = (0..n).map(|i| {
                let t = i as f32 / n as f32;
                let lx = (t * std::f32::consts::TAU * 3.0).sin() * state.gain;
                let ly = (t * std::f32::consts::TAU * 4.0 + 0.5).sin() * state.gain;
                Pos2::new(
                    rect.center().x + lx * rect.width() * 0.4,
                    rect.center().y - ly * rect.height() * 0.4,
                )
            }).collect();
            for (i, w) in pts.windows(2).enumerate() {
                let alpha = (200.0 * (i as f32 / n as f32)) as u8 + 55;
                p.line_segment([w[0], w[1]], Stroke::new(1.0,
                    Color32::from_rgba_premultiplied(col_l.r(), col_l.g(), col_l.b(), alpha)));
            }
        }
        OscilloscopeMode::Goniometer => {
            for i in 0..n {
                let t = i as f32 / n as f32;
                let l = (t * std::f32::consts::TAU * 3.0).sin() * state.gain;
                let r = (t * std::f32::consts::TAU * 3.0 + 0.3).sin() * state.gain;
                let mid = (l + r) * 0.707;
                let side = (l - r) * 0.707;
                let px = rect.center().x + side * rect.width() * 0.4;
                let py = rect.center().y - mid * rect.height() * 0.4;
                p.circle_filled(Pos2::new(px, py), 1.0, Color32::from_rgba_premultiplied(80, 220, 160, 200));
            }
            // M/S labels
            p.text(rect.center_top() + Vec2::new(0.0, 4.0), egui::Align2::CENTER_TOP, "M", FontId::monospace(9.0), Color32::GRAY);
            p.text(rect.right_center() - Vec2::new(8.0, 0.0), egui::Align2::RIGHT_CENTER, "S+", FontId::monospace(9.0), Color32::GRAY);
            p.text(rect.left_center() + Vec2::new(4.0, 0.0), egui::Align2::LEFT_CENTER, "S-", FontId::monospace(9.0), Color32::GRAY);
        }
    }
}

// --- Loudness Meter (LUFS) --------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct LufsMeter {
    pub momentary_lufs: f32,
    pub short_term_lufs: f32,
    pub integrated_lufs: f32,
    pub loudness_range_lu: f32,
    pub true_peak_dbtp: f32,
    pub target_lufs: f32,
    pub target_ceiling_dbtp: f32,
    pub history_seconds: f32,
    pub history: Vec<f32>,
    pub enabled: bool,
}

impl Default for LufsMeter {
    fn default() -> Self {
        Self {
            momentary_lufs: -23.0,
            short_term_lufs: -22.5,
            integrated_lufs: -23.0,
            loudness_range_lu: 6.0,
            true_peak_dbtp: -1.0,
            target_lufs: -14.0,
            target_ceiling_dbtp: -1.0,
            history_seconds: 10.0,
            history: vec![-23.0; 100],
            enabled: true,
        }
    }
}

pub fn show_lufs_meter(ui: &mut egui::Ui, state: &mut LufsMeter) {
    ui.heading("Loudness Meter (EBU R128)");
    ui.separator();

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.enabled, "Enable");
        ui.separator();
        ui.label("Target:");
        ui.add(egui::DragValue::new(&mut state.target_lufs).speed(0.5).clamp_range(-40.0..=0.0).suffix(" LUFS"));
        ui.label("Ceiling:");
        ui.add(egui::DragValue::new(&mut state.target_ceiling_dbtp).speed(0.1).clamp_range(-6.0..=0.0).suffix(" dBTP"));
    });
    ui.separator();

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        egui::Grid::new("lufs_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
            ui.label(egui::RichText::new("Momentary").color(Color32::from_rgb(80, 200, 255)));
            ui.label(egui::RichText::new(format!("{:.1} LUFS", state.momentary_lufs)).strong());
            ui.end_row();
            ui.label(egui::RichText::new("Short-Term").color(Color32::from_rgb(80, 180, 255)));
            ui.label(egui::RichText::new(format!("{:.1} LUFS", state.short_term_lufs)).strong());
            ui.end_row();
            ui.label(egui::RichText::new("Integrated").color(Color32::from_rgb(80, 160, 255)));
            let int_col = if (state.integrated_lufs - state.target_lufs).abs() < 1.0 { Color32::from_rgb(80, 220, 100) } else { Color32::from_rgb(255, 160, 60) };
            ui.label(egui::RichText::new(format!("{:.1} LUFS", state.integrated_lufs)).strong().color(int_col));
            ui.end_row();
            ui.label(egui::RichText::new("LRA").color(Color32::from_rgb(160, 180, 255)));
            ui.label(egui::RichText::new(format!("{:.1} LU", state.loudness_range_lu)).strong());
            ui.end_row();
            ui.label(egui::RichText::new("True Peak").color(Color32::from_rgb(255, 140, 60)));
            let tp_col = if state.true_peak_dbtp > state.target_ceiling_dbtp { Color32::RED } else { Color32::from_rgb(80, 220, 100) };
            ui.label(egui::RichText::new(format!("{:.1} dBTP", state.true_peak_dbtp)).strong().color(tp_col));
            ui.end_row();
        });

        let ui = &mut cols[1];
        draw_lufs_history(ui, state);
    });
}

pub fn draw_lufs_history(ui: &mut egui::Ui, state: &LufsMeter) {
    let size = Vec2::new(180.0, 100.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let db_to_y = |db: f32| -> f32 {
        rect.bottom() - ((db + 40.0) / 40.0).clamp(0.0, 1.0) * rect.height()
    };

    // Target line
    let ty = db_to_y(state.target_lufs);
    p.line_segment([Pos2::new(rect.left(), ty), Pos2::new(rect.right(), ty)],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(80, 220, 100, 180)));
    p.text(Pos2::new(rect.right() - 2.0, ty - 2.0), egui::Align2::RIGHT_BOTTOM,
        format!("{:.0}", state.target_lufs), FontId::monospace(7.0), Color32::from_rgb(80, 220, 100));

    let n = state.history.len();
    let pts: Vec<Pos2> = state.history.iter().enumerate().map(|(i, &db)| {
        Pos2::new(rect.left() + i as f32 / n as f32 * rect.width(), db_to_y(db))
    }).collect();
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(80, 160, 255)));
    }

    // dB labels
    for &db in &[-40.0f32, -23.0, -14.0, 0.0] {
        let y = db_to_y(db);
        p.text(Pos2::new(rect.left() + 2.0, y), egui::Align2::LEFT_CENTER,
            format!("{:.0}", db), FontId::monospace(7.0), Color32::GRAY);
    }
}

// --- BPM Tap Tempo and Sync ------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ClockSource {
    Internal,
    MidiClock,
    Ableton,
    Manual,
}

impl ClockSource {
    pub fn label(&self) -> &str {
        match self {
            ClockSource::Internal => "Internal",
            ClockSource::MidiClock => "MIDI Clock",
            ClockSource::Ableton => "Ableton Link",
            ClockSource::Manual => "Manual",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BpmSyncState {
    pub bpm: f32,
    pub tap_times: Vec<f32>,
    pub clock_source: ClockSource,
    pub beat: f32,
    pub bar: u32,
    pub playing: bool,
    pub send_midi_clock: bool,
    pub ableton_link_peers: u32,
    pub nudge_amount_ms: f32,
    pub swing_amount: f32,
    pub swing_subdivide: u8,
}

impl Default for BpmSyncState {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            tap_times: vec![],
            clock_source: ClockSource::Internal,
            beat: 0.0,
            bar: 1,
            playing: false,
            send_midi_clock: false,
            ableton_link_peers: 0,
            nudge_amount_ms: 10.0,
            swing_amount: 0.0,
            swing_subdivide: 2,
        }
    }
}

impl BpmSyncState {
    pub fn tap(&mut self, current_time_ms: f32) {
        self.tap_times.push(current_time_ms);
        if self.tap_times.len() > 8 { self.tap_times.remove(0); }
        if self.tap_times.len() >= 2 {
            let n = self.tap_times.len();
            let avg_interval = (self.tap_times[n-1] - self.tap_times[0]) / (n - 1) as f32;
            self.bpm = 60000.0 / avg_interval.max(1.0);
        }
    }
}

pub fn show_bpm_sync(ui: &mut egui::Ui, state: &mut BpmSyncState) {
    ui.heading("BPM & Clock Sync");
    ui.separator();

    ui.horizontal(|ui| {
        for src in &[ClockSource::Internal, ClockSource::MidiClock, ClockSource::Ableton, ClockSource::Manual] {
            if ui.selectable_label(state.clock_source == *src, src.label()).clicked() {
                state.clock_source = src.clone();
            }
        }
        if state.clock_source == ClockSource::Ableton {
            ui.label(format!("Peers: {}", state.ableton_link_peers));
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut state.bpm).speed(0.1).clamp_range(20.0..=300.0).suffix(" BPM"));

        if state.clock_source == ClockSource::Internal {
            if ui.button("TAP").clicked() { state.tap(0.0); }
            if state.tap_times.len() >= 2 {
                ui.label(format!("{:.1}", state.bpm));
            }
        }

        if ui.button("◀◀").clicked() { state.bpm = (state.bpm - 1.0).max(20.0); }
        if ui.button("▶▶").clicked() { state.bpm = (state.bpm + 1.0).min(300.0); }
        ui.separator();
        if ui.button(if state.playing { "⏸ Stop" } else { "▶ Start" }).clicked() { state.playing = !state.playing; }
    });

    ui.horizontal(|ui| {
        ui.label(format!("Bar: {}  Beat: {:.2}", state.bar, state.beat));
        ui.separator();
        ui.label("Nudge:");
        ui.add(egui::DragValue::new(&mut state.nudge_amount_ms).speed(1.0).clamp_range(1.0..=100.0).suffix(" ms"));
        if ui.button("◀ Nudge").clicked() {}
        if ui.button("Nudge ▶").clicked() {}
    });

    ui.horizontal(|ui| {
        ui.label("Swing:");
        ui.add(egui::Slider::new(&mut state.swing_amount, 0.0..=100.0).suffix("%"));
        ui.label("Sub:");
        ui.add(egui::DragValue::new(&mut state.swing_subdivide).clamp_range(2u8..=8));
        ui.checkbox(&mut state.send_midi_clock, "Send MIDI Clock");
    });

    // Beat visualization
    let n_beats = 4usize;
    let size = Vec2::new(ui.available_width().min(300.0), 28.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let beat_w = rect.width() / n_beats as f32;
    for i in 0..n_beats {
        let br = Rect::from_min_size(
            Pos2::new(rect.left() + i as f32 * beat_w + 2.0, rect.top() + 2.0),
            Vec2::new(beat_w - 4.0, rect.height() - 4.0),
        );
        let current_beat_idx = (state.beat as usize) % n_beats;
        let active = i == current_beat_idx && state.playing;
        let col = if active { Color32::from_rgb(80, 220, 120) } else { Color32::from_rgb(30, 38, 50) };
        p.rect_filled(br, 3.0, col);
        p.text(br.center(), egui::Align2::CENTER_CENTER,
            format!("{}", i + 1), FontId::monospace(10.0),
            if active { Color32::BLACK } else { Color32::from_rgb(100, 110, 130) });
    }
}

// --- Audio Mixer Master State ----------------------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioMixerMasterState {
    pub dynamics: DynamicsEditorState,
    pub effect_chain: EffectChainState,
    pub event_system: AudioEventSystemState,
    pub spatialization: Spatialization3DState,
    pub scripting: AudioScriptingState,
    pub eq: MultibandEq,
    pub bus_routing: BusRoutingState,
    pub waveform_editor: WaveformEditor,
    pub mix_scenes: MixSceneManagerState,
    pub metering: MeteringGridState,
    pub timeline: AudioTimelineState,
    pub file_browser: AudioFileBrowserState,
    pub reverb: ReverbDesigner,
    pub oscilloscope: AudioOscilloscope,
    pub lufs: LufsMeter,
    pub bpm_sync: BpmSyncState,
    pub selected_panel: u8,
}

impl AudioMixerMasterState {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.spatialization = Spatialization3DState::new();
        s.timeline = AudioTimelineState::new();
        s.file_browser = AudioFileBrowserState::with_demo_files();
        s.bus_routing = BusRoutingState::default_buses();
        s.metering.meters = vec![
            BusMeter::new("Master"),
            BusMeter::new("Music"),
            BusMeter::new("SFX"),
            BusMeter::new("Voice"),
            BusMeter::new("Ambient"),
            BusMeter::new("UI"),
        ];
        s.mix_scenes.n_buses = 8;
        s
    }
}

pub fn show_audio_mixer_master(ui: &mut egui::Ui, state: &mut AudioMixerMasterState) {
    ui.heading("Audio Mixer Master");
    ui.separator();

    let panels = [
        "Dynamics", "Effects", "Events", "Spatial", "Scripting",
        "EQ", "Routing", "Waveform", "Scenes", "Meters",
        "Timeline", "Files", "Reverb", "Scope", "LUFS", "BPM",
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
        0 => show_dynamics_editor(ui, &mut state.dynamics),
        1 => show_effect_chain(ui, &mut state.effect_chain),
        2 => show_audio_event_system(ui, &mut state.event_system),
        3 => show_spatialization_editor(ui, &mut state.spatialization),
        4 => show_audio_scripting_editor(ui, &mut state.scripting),
        5 => show_multiband_eq(ui, &mut state.eq),
        6 => show_bus_routing(ui, &mut state.bus_routing),
        7 => show_waveform_editor(ui, &mut state.waveform_editor),
        8 => show_mix_scene_manager(ui, &mut state.mix_scenes),
        9 => show_metering_grid(ui, &mut state.metering),
        10 => show_audio_timeline(ui, &mut state.timeline),
        11 => show_audio_file_browser(ui, &mut state.file_browser),
        12 => show_reverb_designer(ui, &mut state.reverb),
        13 => show_oscilloscope(ui, &mut state.oscilloscope),
        14 => show_lufs_meter(ui, &mut state.lufs),
        15 => show_bpm_sync(ui, &mut state.bpm_sync),
        _ => {}
    }
}

// --- Audio Constants & Utilities -------------------------------------------------

pub const AUDIO_SAMPLE_RATE_44100: u32 = 44100;
pub const AUDIO_SAMPLE_RATE_48000: u32 = 48000;
pub const AUDIO_SAMPLE_RATE_96000: u32 = 96000;
pub const AUDIO_BUFFER_SIZE_DEFAULT: u32 = 512;
pub const AUDIO_MAX_BUSES: usize = 32;
pub const AUDIO_MAX_CHANNELS: usize = 8;
pub const AUDIO_MAX_POLYPHONY: usize = 256;
pub const AUDIO_MIDI_NOTE_COUNT: usize = 128;
pub const AUDIO_SPECTRUM_BINS_DEFAULT: usize = 2048;
pub const LUFS_GATE_THRESHOLD: f32 = -70.0;
pub const LUFS_SHORT_TERM_WINDOW_S: f32 = 3.0;
pub const LUFS_MOMENTARY_WINDOW_S: f32 = 0.4;
pub const EBU_TARGET_LUFS: f32 = -23.0;
pub const ATSC_TARGET_LUFS: f32 = -24.0;
pub const SPOTIFY_TARGET_LUFS: f32 = -14.0;
pub const YOUTUBE_TARGET_LUFS: f32 = -14.0;
pub const APPLE_TARGET_LUFS: f32 = -16.0;
pub const TIDAL_TARGET_LUFS: f32 = -14.0;

#[inline] pub fn db_to_linear(db: f32) -> f32 { 10.0f32.powf(db / 20.0) }
#[inline] pub fn linear_to_db(linear: f32) -> f32 { 20.0 * linear.max(1e-10).log10() }
#[inline] pub fn midi_note_to_freq(note: u8) -> f32 { 440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0) }
#[inline] pub fn freq_to_midi_note(freq: f32) -> f32 { 69.0 + 12.0 * (freq / 440.0).log2() }
#[inline] pub fn semitones_to_ratio(st: f32) -> f32 { 2.0f32.powf(st / 12.0) }
#[inline] pub fn cents_to_ratio(cents: f32) -> f32 { 2.0f32.powf(cents / 1200.0) }
#[inline] pub fn pan_to_lr(pan: f32) -> (f32, f32) {
    let t = (pan + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
    (t.cos(), t.sin())
}
#[inline] pub fn lufs_to_rms(lufs: f32) -> f32 { db_to_linear(lufs) }
#[inline] pub fn beats_to_seconds(beats: f32, bpm: f32) -> f32 { beats * 60.0 / bpm.max(1.0) }
#[inline] pub fn seconds_to_beats(secs: f32, bpm: f32) -> f32 { secs * bpm / 60.0 }
#[inline] pub fn samples_to_ms(samples: u32, sample_rate: u32) -> f32 { samples as f32 / sample_rate as f32 * 1000.0 }
#[inline] pub fn ms_to_samples(ms: f32, sample_rate: u32) -> u32 { (ms * sample_rate as f32 / 1000.0) as u32 }
#[inline] pub fn crossfade_linear(a: f32, b: f32, t: f32) -> f32 { a * (1.0 - t) + b * t }
#[inline] pub fn crossfade_equal_power(a: f32, b: f32, t: f32) -> f32 {
    let ta = ((1.0 - t) * std::f32::consts::FRAC_PI_2).cos();
    let tb = (t * std::f32::consts::FRAC_PI_2).cos();
    a * ta + b * tb
}
#[inline] pub fn soft_clip(x: f32, threshold: f32) -> f32 {
    let abs = x.abs();
    if abs < threshold { x } else {
        let sign = x.signum();
        sign * (threshold + (abs - threshold) / (1.0 + ((abs - threshold) / (1.0 - threshold)).powi(2)).sqrt())
    }
}
#[inline] pub fn hard_clip(x: f32, ceiling: f32) -> f32 { x.clamp(-ceiling, ceiling) }
#[inline] pub fn rms_window(samples: &[f32]) -> f32 {
    if samples.is_empty() { return 0.0; }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

pub fn freq_band_name(freq: f32) -> &'static str {
    if freq < 60.0 { "Sub Bass" }
    else if freq < 250.0 { "Bass" }
    else if freq < 500.0 { "Low Mid" }
    else if freq < 2000.0 { "Mid" }
    else if freq < 4000.0 { "High Mid" }
    else if freq < 8000.0 { "Presence" }
    else { "Air" }
}

pub fn note_name_full(midi: u8) -> String {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (midi as i32 / 12) - 1;
    format!("{}{}", names[(midi % 12) as usize], octave)
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
