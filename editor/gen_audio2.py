path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 2
// ============================================================

// --- Dynamics Processor ----------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum DynamicsMode {
    Compressor,
    Limiter,
    Expander,
    NoiseGate,
    DeEsser,
    Transient,
}

impl Default for DynamicsMode {
    fn default() -> Self { DynamicsMode::Compressor }
}

impl DynamicsMode {
    pub fn label(&self) -> &str {
        match self {
            DynamicsMode::Compressor => "Compressor",
            DynamicsMode::Limiter => "Limiter",
            DynamicsMode::Expander => "Expander",
            DynamicsMode::NoiseGate => "Noise Gate",
            DynamicsMode::DeEsser => "De-Esser",
            DynamicsMode::Transient => "Transient",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DynamicsProcessor {
    pub mode: DynamicsMode,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub makeup_gain_db: f32,
    pub enabled: bool,
    pub auto_gain: bool,
    pub sidechain_enabled: bool,
    pub sidechain_freq: f32,
    pub lookahead_ms: f32,
    pub hold_ms: f32,
    pub rms_window_ms: f32,
    pub peak_mode: bool,
    // De-esser specific
    pub deesser_freq: f32,
    pub deesser_bandwidth: f32,
    pub deesser_reduction_db: f32,
    // Transient specific
    pub attack_gain_db: f32,
    pub sustain_gain_db: f32,
    // Metering
    pub gain_reduction_db: f32,
    pub input_level_db: f32,
    pub output_level_db: f32,
}

impl Default for DynamicsProcessor {
    fn default() -> Self {
        Self {
            mode: DynamicsMode::Compressor,
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 3.0,
            makeup_gain_db: 0.0,
            enabled: true,
            auto_gain: false,
            sidechain_enabled: false,
            sidechain_freq: 8000.0,
            lookahead_ms: 0.0,
            hold_ms: 0.0,
            rms_window_ms: 10.0,
            peak_mode: true,
            deesser_freq: 6500.0,
            deesser_bandwidth: 2000.0,
            deesser_reduction_db: 6.0,
            attack_gain_db: 0.0,
            sustain_gain_db: 0.0,
            gain_reduction_db: 0.0,
            input_level_db: -20.0,
            output_level_db: -20.0,
        }
    }
}

impl DynamicsProcessor {
    pub fn compute_gain_reduction(&self, input_db: f32) -> f32 {
        match self.mode {
            DynamicsMode::Compressor | DynamicsMode::Limiter => {
                let ratio = if self.mode == DynamicsMode::Limiter { 1000.0 } else { self.ratio };
                let over = input_db - self.threshold_db;
                if over <= -self.knee_db * 0.5 {
                    0.0
                } else if over < self.knee_db * 0.5 {
                    let t = (over + self.knee_db * 0.5) / self.knee_db;
                    (1.0 / ratio - 1.0) * over * t * 0.5
                } else {
                    (1.0 / ratio - 1.0) * over
                }
            }
            DynamicsMode::Expander | DynamicsMode::NoiseGate => {
                let ratio = if self.mode == DynamicsMode::NoiseGate { 100.0 } else { self.ratio };
                let under = self.threshold_db - input_db;
                if under > 0.0 { -(ratio - 1.0) * under } else { 0.0 }
            }
            _ => 0.0,
        }
    }

    pub fn transfer_curve_points(&self, n: usize) -> Vec<[f32; 2]> {
        let min_in = -80.0f32;
        let max_in = 0.0f32;
        (0..n).map(|i| {
            let x = min_in + (max_in - min_in) * i as f32 / (n - 1) as f32;
            let gr = self.compute_gain_reduction(x);
            let y = (x + gr + self.makeup_gain_db).clamp(-80.0, 6.0);
            [x, y]
        }).collect()
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct DynamicsEditorState {
    pub processor: DynamicsProcessor,
    pub show_transfer: bool,
    pub show_meter: bool,
    pub history_gr: Vec<f32>,
    pub history_in: Vec<f32>,
    pub history_out: Vec<f32>,
}

pub fn show_dynamics_editor(ui: &mut egui::Ui, state: &mut DynamicsEditorState) {
    ui.heading("Dynamics Processor");
    ui.separator();

    ui.horizontal(|ui| {
        for mode in &[DynamicsMode::Compressor, DynamicsMode::Limiter, DynamicsMode::Expander,
                      DynamicsMode::NoiseGate, DynamicsMode::DeEsser, DynamicsMode::Transient] {
            if ui.selectable_label(state.processor.mode == *mode, mode.label()).clicked() {
                state.processor.mode = mode.clone();
            }
        }
        ui.separator();
        ui.checkbox(&mut state.processor.enabled, "Enable");
    });
    ui.separator();

    let p = &mut state.processor;

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        match p.mode {
            DynamicsMode::DeEsser => {
                ui.label("De-Esser Frequency:");
                ui.add(egui::Slider::new(&mut p.deesser_freq, 2000.0..=16000.0).suffix(" Hz").logarithmic(true));
                ui.label("Bandwidth:");
                ui.add(egui::Slider::new(&mut p.deesser_bandwidth, 100.0..=5000.0).suffix(" Hz"));
                ui.label("Reduction:");
                ui.add(egui::Slider::new(&mut p.deesser_reduction_db, 0.0..=20.0).suffix(" dB"));
                ui.label("Threshold:");
                ui.add(egui::Slider::new(&mut p.threshold_db, -60.0..=0.0).suffix(" dB"));
                ui.label("Attack:");
                ui.add(egui::Slider::new(&mut p.attack_ms, 0.1..=50.0).suffix(" ms").logarithmic(true));
                ui.label("Release:");
                ui.add(egui::Slider::new(&mut p.release_ms, 1.0..=500.0).suffix(" ms").logarithmic(true));
            }
            DynamicsMode::Transient => {
                ui.label("Attack Gain:");
                ui.add(egui::Slider::new(&mut p.attack_gain_db, -24.0..=24.0).suffix(" dB"));
                ui.label("Sustain Gain:");
                ui.add(egui::Slider::new(&mut p.sustain_gain_db, -24.0..=24.0).suffix(" dB"));
                ui.label("Speed:");
                ui.add(egui::Slider::new(&mut p.attack_ms, 0.1..=100.0).suffix(" ms"));
            }
            _ => {
                ui.label("Threshold:");
                ui.add(egui::Slider::new(&mut p.threshold_db, -60.0..=0.0).suffix(" dB"));
                ui.label(format!("Ratio: {:.1}:1", p.ratio));
                ui.add(egui::Slider::new(&mut p.ratio, 1.0..=20.0));
                ui.label("Knee:");
                ui.add(egui::Slider::new(&mut p.knee_db, 0.0..=12.0).suffix(" dB"));
                ui.label("Attack:");
                ui.add(egui::Slider::new(&mut p.attack_ms, 0.1..=200.0).suffix(" ms").logarithmic(true));
                ui.label("Release:");
                ui.add(egui::Slider::new(&mut p.release_ms, 1.0..=2000.0).suffix(" ms").logarithmic(true));
                ui.label("Hold:");
                ui.add(egui::Slider::new(&mut p.hold_ms, 0.0..=500.0).suffix(" ms"));
                ui.label("Lookahead:");
                ui.add(egui::Slider::new(&mut p.lookahead_ms, 0.0..=20.0).suffix(" ms"));
                ui.label("Makeup Gain:");
                ui.add(egui::Slider::new(&mut p.makeup_gain_db, -12.0..=24.0).suffix(" dB"));
                ui.checkbox(&mut p.auto_gain, "Auto Gain");
                ui.checkbox(&mut p.peak_mode, "Peak Mode");
                ui.checkbox(&mut p.sidechain_enabled, "Sidechain");
                if p.sidechain_enabled {
                    ui.label("Sidechain HP:");
                    ui.add(egui::Slider::new(&mut p.sidechain_freq, 20.0..=20000.0).suffix(" Hz").logarithmic(true));
                }
            }
        }

        let ui = &mut cols[1];
        ui.checkbox(&mut state.show_transfer, "Transfer Curve");
        if state.show_transfer {
            draw_transfer_curve(ui, &state.processor);
        }
        ui.checkbox(&mut state.show_meter, "GR Meter");
        if state.show_meter {
            draw_gain_reduction_meter(ui, p.gain_reduction_db, p.input_level_db, p.output_level_db);
        }
    });
}

pub fn draw_transfer_curve(ui: &mut egui::Ui, proc: &DynamicsProcessor) {
    let size = Vec2::new(200.0, 200.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(12, 14, 20));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(60, 60, 80)));

    // Grid
    for i in 0..=4 {
        let t = i as f32 / 4.0;
        let x = rect.left() + t * rect.width();
        let y = rect.top() + t * rect.height();
        p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(0.5, Color32::from_rgba_premultiplied(60, 60, 80, 120)));
        p.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.5, Color32::from_rgba_premultiplied(60, 60, 80, 120)));
    }

    // 1:1 line
    p.line_segment([rect.left_bottom(), rect.right_top()],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 100, 140, 160)));

    let db_to_px = |db: f32, range: f32, length: f32| -> f32 {
        ((db + range) / range * length).clamp(0.0, length)
    };
    let range = 80.0f32;
    let pts = proc.transfer_curve_points(64);
    let screen_pts: Vec<Pos2> = pts.iter().map(|[x, y]| {
        Pos2::new(
            rect.left() + db_to_px(*x, range, rect.width()),
            rect.bottom() - db_to_px(*y, range, rect.height()),
        )
    }).collect();

    if screen_pts.len() >= 2 {
        for w in screen_pts.windows(2) {
            p.line_segment([w[0], w[1]], Stroke::new(2.0, Color32::from_rgb(80, 200, 120)));
        }
    }

    // Threshold marker
    let tx = rect.left() + db_to_px(proc.threshold_db, range, rect.width());
    p.line_segment([Pos2::new(tx, rect.top()), Pos2::new(tx, rect.bottom())],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 100, 80, 200)));
    p.text(Pos2::new(tx + 2.0, rect.top() + 4.0), egui::Align2::LEFT_TOP,
        format!("{:.0}dB", proc.threshold_db), FontId::monospace(8.0), Color32::from_rgb(255, 100, 80));

    // Labels
    p.text(rect.left_bottom() + Vec2::new(2.0, -2.0), egui::Align2::LEFT_BOTTOM,
        "Input", FontId::monospace(8.0), Color32::GRAY);
    p.text(rect.left_top() + Vec2::new(2.0, 2.0), egui::Align2::LEFT_TOP,
        "Out", FontId::monospace(8.0), Color32::GRAY);
}

pub fn draw_gain_reduction_meter(ui: &mut egui::Ui, gr_db: f32, in_db: f32, out_db: f32) {
    let size = Vec2::new(180.0, 80.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(12, 14, 20));

    let label_w = 28.0;
    let meter_w = rect.width() - label_w;
    let row_h = 22.0;

    let draw_meter = |p: &egui::Painter, y: f32, label: &str, db: f32, col: Color32| {
        let lbl_rect = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(label_w, row_h - 2.0));
        p.text(lbl_rect.center(), egui::Align2::CENTER_CENTER, label, FontId::monospace(9.0), Color32::GRAY);
        let bar_rect = Rect::from_min_size(Pos2::new(rect.left() + label_w, y), Vec2::new(meter_w, row_h - 2.0));
        p.rect_filled(bar_rect, 2.0, Color32::from_rgb(20, 20, 30));
        let t = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let fill = Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width() * t, bar_rect.height()));
        p.rect_filled(fill, 2.0, col);
        p.text(Pos2::new(bar_rect.right() - 2.0, bar_rect.center().y), egui::Align2::RIGHT_CENTER,
            format!("{:.1}", db), FontId::monospace(8.0), Color32::WHITE);
    };

    draw_meter(&p, rect.top() + 4.0, "IN", in_db, Color32::from_rgb(60, 180, 255));
    draw_meter(&p, rect.top() + 4.0 + row_h, "OUT", out_db, Color32::from_rgb(80, 220, 100));
    draw_meter(&p, rect.top() + 4.0 + row_h * 2.0, "GR", -gr_db.abs(), Color32::from_rgb(255, 100, 80));
}

// --- Effect Chain ----------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum AudioEffectType {
    Gain,
    EqBand,
    Compressor,
    Reverb,
    Delay,
    Chorus,
    Flanger,
    Phaser,
    Distortion,
    Bitcrusher,
    Filter,
    Convolution,
    Tremolo,
    Vibrato,
    Stereo,
    Custom(String),
}

impl AudioEffectType {
    pub fn label(&self) -> &str {
        match self {
            AudioEffectType::Gain => "Gain",
            AudioEffectType::EqBand => "EQ Band",
            AudioEffectType::Compressor => "Compressor",
            AudioEffectType::Reverb => "Reverb",
            AudioEffectType::Delay => "Delay",
            AudioEffectType::Chorus => "Chorus",
            AudioEffectType::Flanger => "Flanger",
            AudioEffectType::Phaser => "Phaser",
            AudioEffectType::Distortion => "Distortion",
            AudioEffectType::Bitcrusher => "Bitcrusher",
            AudioEffectType::Filter => "Filter",
            AudioEffectType::Convolution => "Convolution",
            AudioEffectType::Tremolo => "Tremolo",
            AudioEffectType::Vibrato => "Vibrato",
            AudioEffectType::Stereo => "Stereo",
            AudioEffectType::Custom(n) => n.as_str(),
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            AudioEffectType::Gain => Color32::from_rgb(180, 180, 180),
            AudioEffectType::EqBand => Color32::from_rgb(100, 200, 255),
            AudioEffectType::Compressor => Color32::from_rgb(255, 160, 80),
            AudioEffectType::Reverb => Color32::from_rgb(160, 100, 255),
            AudioEffectType::Delay => Color32::from_rgb(255, 220, 80),
            AudioEffectType::Chorus => Color32::from_rgb(80, 220, 160),
            AudioEffectType::Flanger => Color32::from_rgb(80, 255, 200),
            AudioEffectType::Phaser => Color32::from_rgb(200, 255, 80),
            AudioEffectType::Distortion => Color32::from_rgb(255, 80, 80),
            AudioEffectType::Bitcrusher => Color32::from_rgb(255, 80, 160),
            AudioEffectType::Filter => Color32::from_rgb(80, 160, 255),
            AudioEffectType::Convolution => Color32::from_rgb(200, 160, 255),
            _ => Color32::from_rgb(160, 160, 160),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChainedEffect {
    pub id: usize,
    pub name: String,
    pub effect_type: AudioEffectType,
    pub enabled: bool,
    pub bypass: bool,
    pub wet_dry: f32,
    pub gain_in_db: f32,
    pub gain_out_db: f32,
    pub params: std::collections::HashMap<String, f32>,
}

impl ChainedEffect {
    pub fn new(id: usize, effect_type: AudioEffectType) -> Self {
        let name = effect_type.label().to_string();
        let mut params = std::collections::HashMap::new();
        match &effect_type {
            AudioEffectType::Reverb => {
                params.insert("room_size".into(), 0.5);
                params.insert("decay".into(), 1.5);
                params.insert("pre_delay_ms".into(), 10.0);
                params.insert("diffusion".into(), 0.7);
                params.insert("damping".into(), 0.5);
                params.insert("low_cut".into(), 80.0);
                params.insert("high_cut".into(), 8000.0);
            }
            AudioEffectType::Delay => {
                params.insert("time_ms".into(), 250.0);
                params.insert("feedback".into(), 0.35);
                params.insert("high_cut".into(), 6000.0);
                params.insert("sync".into(), 0.0);
                params.insert("ping_pong".into(), 0.0);
            }
            AudioEffectType::Chorus => {
                params.insert("rate_hz".into(), 1.0);
                params.insert("depth_ms".into(), 5.0);
                params.insert("voices".into(), 3.0);
                params.insert("spread".into(), 0.5);
            }
            AudioEffectType::Distortion => {
                params.insert("drive".into(), 0.5);
                params.insert("tone".into(), 0.5);
                params.insert("asymmetry".into(), 0.0);
                params.insert("type".into(), 0.0);
            }
            AudioEffectType::Bitcrusher => {
                params.insert("bits".into(), 8.0);
                params.insert("sample_rate_div".into(), 4.0);
                params.insert("dither".into(), 0.0);
            }
            AudioEffectType::Filter => {
                params.insert("freq".into(), 1000.0);
                params.insert("q".into(), 0.707);
                params.insert("type".into(), 0.0);
            }
            _ => {}
        }
        Self { id, name, effect_type, enabled: true, bypass: false, wet_dry: 1.0,
               gain_in_db: 0.0, gain_out_db: 0.0, params }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct EffectChainState {
    pub effects: Vec<ChainedEffect>,
    pub selected: Option<usize>,
    pub ab_mode: bool,
    pub chain_a: Vec<ChainedEffect>,
    pub chain_b: Vec<ChainedEffect>,
    pub active_chain: u8,
    pub drag_from: Option<usize>,
    pub bypass_all: bool,
}

pub fn show_effect_chain(ui: &mut egui::Ui, state: &mut EffectChainState) {
    ui.heading("Effect Chain");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ Add").clicked() {
            let id = state.effects.len();
            state.effects.push(ChainedEffect::new(id, AudioEffectType::Gain));
        }
        ui.separator();
        if ui.button("A/B").clicked() { state.ab_mode = !state.ab_mode; }
        if state.ab_mode {
            if ui.selectable_label(state.active_chain == 0, "Chain A").clicked() {
                if state.active_chain != 0 {
                    state.chain_b = state.effects.clone();
                    state.effects = state.chain_a.clone();
                    state.active_chain = 0;
                }
            }
            if ui.selectable_label(state.active_chain == 1, "Chain B").clicked() {
                if state.active_chain != 1 {
                    state.chain_a = state.effects.clone();
                    state.effects = state.chain_b.clone();
                    state.active_chain = 1;
                }
            }
        }
        ui.separator();
        ui.checkbox(&mut state.bypass_all, "Bypass All");
    });
    ui.separator();

    draw_signal_flow(ui, &state.effects);
    ui.separator();

    let mut remove = None;
    let mut move_up = None;
    let mut move_dn = None;
    let n = state.effects.len();

    for (i, eff) in state.effects.iter_mut().enumerate() {
        let sel = state.selected == Some(i);
        let col = if eff.bypass || state.bypass_all { Color32::GRAY } else { eff.effect_type.color() };

        ui.horizontal(|ui| {
            ui.colored_label(col, format!("{:2}. {}", i + 1, eff.name));
            ui.checkbox(&mut eff.enabled, "");
            ui.checkbox(&mut eff.bypass, "BP");
            ui.add(egui::DragValue::new(&mut eff.wet_dry).speed(0.01).clamp_range(0.0..=1.0).prefix("W/D: "));
            if ui.small_button("↑").clicked() && i > 0 { move_up = Some(i); }
            if ui.small_button("↓").clicked() && i < n - 1 { move_dn = Some(i); }
            if ui.small_button("✗").clicked() { remove = Some(i); }
            if ui.selectable_label(sel, "Edit").clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
        });
    }

    if let Some(i) = move_up { state.effects.swap(i, i - 1); }
    if let Some(i) = move_dn { state.effects.swap(i, i + 1); }
    if let Some(i) = remove {
        state.effects.remove(i);
        if state.selected == Some(i) { state.selected = None; }
    }

    if let Some(idx) = state.selected {
        if idx < state.effects.len() {
            ui.separator();
            show_effect_params(ui, &mut state.effects[idx]);
        }
    }
}

pub fn draw_signal_flow(ui: &mut egui::Ui, effects: &[ChainedEffect]) {
    let n = effects.len();
    if n == 0 { return; }
    let box_w = 60.0f32;
    let box_h = 28.0f32;
    let gap = 18.0f32;
    let total_w = (box_w + gap) * n as f32 + gap * 2.0 + 40.0;
    let size = Vec2::new(total_w.min(ui.available_width()), box_h + 16.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let start_x = rect.left() + 8.0;
    let cy = rect.center().y;

    // "IN" label
    p.text(Pos2::new(start_x, cy), egui::Align2::LEFT_CENTER, "IN", FontId::monospace(9.0), Color32::GRAY);
    let mut x = start_x + 20.0;

    for eff in effects {
        let col = if eff.bypass { Color32::GRAY } else { eff.effect_type.color() };
        let bx = x;
        // Arrow
        p.line_segment([Pos2::new(bx - gap + 4.0, cy), Pos2::new(bx, cy)], Stroke::new(1.5, Color32::from_rgb(100, 100, 120)));
        p.line_segment([Pos2::new(bx, cy), Pos2::new(bx - 5.0, cy - 4.0)], Stroke::new(1.5, Color32::from_rgb(100, 100, 120)));
        p.line_segment([Pos2::new(bx, cy), Pos2::new(bx - 5.0, cy + 4.0)], Stroke::new(1.5, Color32::from_rgb(100, 100, 120)));
        // Box
        let br = Rect::from_min_size(Pos2::new(bx, cy - box_h / 2.0), Vec2::new(box_w, box_h));
        p.rect_filled(br, 3.0, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 40));
        p.rect_stroke(br, 3.0, Stroke::new(1.0, col));
        let label: String = eff.effect_type.label().chars().take(7).collect();
        p.text(br.center(), egui::Align2::CENTER_CENTER, label, FontId::monospace(8.0), col);
        if eff.bypass {
            p.text(br.center() + Vec2::new(0.0, -1.0), egui::Align2::CENTER_CENTER,
                "BP", FontId::monospace(6.0), Color32::from_rgb(255, 160, 0));
        }
        x += box_w + gap;
    }

    // Arrow to OUT
    p.line_segment([Pos2::new(x, cy), Pos2::new(x + 16.0, cy)], Stroke::new(1.5, Color32::from_rgb(100, 100, 120)));
    p.text(Pos2::new(x + 18.0, cy), egui::Align2::LEFT_CENTER, "OUT", FontId::monospace(9.0), Color32::GRAY);
}

pub fn show_effect_params(ui: &mut egui::Ui, eff: &mut ChainedEffect) {
    ui.label(egui::RichText::new(format!("{} Parameters", eff.name)).strong());
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut eff.name);
    });
    ui.horizontal(|ui| {
        ui.label("Wet/Dry:");
        ui.add(egui::Slider::new(&mut eff.wet_dry, 0.0..=1.0));
        ui.label("In Gain:");
        ui.add(egui::DragValue::new(&mut eff.gain_in_db).speed(0.1).clamp_range(-24.0..=24.0).suffix(" dB"));
        ui.label("Out Gain:");
        ui.add(egui::DragValue::new(&mut eff.gain_out_db).speed(0.1).clamp_range(-24.0..=24.0).suffix(" dB"));
    });

    let keys: Vec<String> = eff.params.keys().cloned().collect();
    egui::Grid::new("eff_params").num_columns(2).spacing([8.0, 3.0]).show(ui, |ui| {
        for k in &keys {
            if let Some(v) = eff.params.get_mut(k) {
                ui.label(k);
                ui.add(egui::DragValue::new(v).speed(0.01));
                ui.end_row();
            }
        }
    });
}

// --- Audio Events ----------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum AudioTriggerType {
    OnCollision,
    OnDestroy,
    OnTimer(f32),
    OnBeat,
    OnThreshold { channel: usize, db: f32 },
    OnGameEvent(String),
    Manual,
}

impl AudioTriggerType {
    pub fn label(&self) -> &str {
        match self {
            AudioTriggerType::OnCollision => "On Collision",
            AudioTriggerType::OnDestroy => "On Destroy",
            AudioTriggerType::OnTimer(_) => "Timer",
            AudioTriggerType::OnBeat => "On Beat",
            AudioTriggerType::OnThreshold { .. } => "Threshold",
            AudioTriggerType::OnGameEvent(_) => "Game Event",
            AudioTriggerType::Manual => "Manual",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum AttenuationModel {
    None,
    Linear,
    InverseSquare,
    Logarithmic,
    Custom,
}

impl AttenuationModel {
    pub fn label(&self) -> &str {
        match self {
            AttenuationModel::None => "None",
            AttenuationModel::Linear => "Linear",
            AttenuationModel::InverseSquare => "Inverse Square",
            AttenuationModel::Logarithmic => "Logarithmic",
            AttenuationModel::Custom => "Custom",
        }
    }

    pub fn compute_gain(&self, dist: f32, min_dist: f32, max_dist: f32) -> f32 {
        if dist <= min_dist { return 1.0; }
        if dist >= max_dist { return 0.0; }
        let t = (dist - min_dist) / (max_dist - min_dist).max(0.001);
        match self {
            AttenuationModel::None => 1.0,
            AttenuationModel::Linear => 1.0 - t,
            AttenuationModel::InverseSquare => 1.0 / (1.0 + t * t * 10.0),
            AttenuationModel::Logarithmic => (1.0 - t).powf(0.5),
            AttenuationModel::Custom => 1.0 - t * t,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SoundConfig {
    pub clip_name: String,
    pub volume_db: f32,
    pub pitch_semitones: f32,
    pub loop_sound: bool,
    pub loop_start: f32,
    pub loop_end: f32,
    pub fade_in_ms: f32,
    pub fade_out_ms: f32,
    pub bus: usize,
    pub priority: u8,
    pub max_instances: u32,
    pub random_pitch_range: f32,
    pub random_volume_range_db: f32,
    pub start_offset_ms: f32,
    pub doppler_enabled: bool,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            clip_name: "unnamed.ogg".into(),
            volume_db: 0.0,
            pitch_semitones: 0.0,
            loop_sound: false,
            loop_start: 0.0,
            loop_end: 1.0,
            fade_in_ms: 0.0,
            fade_out_ms: 0.0,
            bus: 0,
            priority: 128,
            max_instances: 4,
            random_pitch_range: 0.0,
            random_volume_range_db: 0.0,
            start_offset_ms: 0.0,
            doppler_enabled: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioEvent {
    pub id: usize,
    pub name: String,
    pub trigger: AudioTriggerType,
    pub sound: SoundConfig,
    pub enabled: bool,
    pub one_shot: bool,
    pub cooldown_ms: f32,
    pub tags: Vec<String>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioEventSystemState {
    pub events: Vec<AudioEvent>,
    pub selected: Option<usize>,
    pub filter_tag: String,
    pub search: String,
}

pub fn show_audio_event_system(ui: &mut egui::Ui, state: &mut AudioEventSystemState) {
    ui.heading("Audio Event System");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ New Event").clicked() {
            let id = state.events.len();
            state.events.push(AudioEvent {
                id,
                name: format!("Event_{}", id),
                trigger: AudioTriggerType::Manual,
                sound: SoundConfig::default(),
                enabled: true,
                one_shot: false,
                cooldown_ms: 0.0,
                tags: vec![],
            });
        }
        ui.separator();
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search);
        ui.label("Tag:");
        ui.text_edit_singleline(&mut state.filter_tag);
    });
    ui.separator();

    let mut remove = None;
    let search_lower = state.search.to_lowercase();

    for (i, ev) in state.events.iter().enumerate() {
        if !search_lower.is_empty() && !ev.name.to_lowercase().contains(&search_lower) { continue; }
        if !state.filter_tag.is_empty() && !ev.tags.iter().any(|t| t.contains(&state.filter_tag)) { continue; }
        let sel = state.selected == Some(i);
        ui.horizontal(|ui| {
            let col = if ev.enabled { Color32::WHITE } else { Color32::GRAY };
            if ui.selectable_label(sel, egui::RichText::new(format!("{} — {}", ev.name, ev.trigger.label())).color(col)).clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
            if ui.small_button("▶").clicked() {}
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove {
        state.events.remove(ri);
        if state.selected == Some(ri) { state.selected = None; }
    }

    if let Some(idx) = state.selected {
        if idx < state.events.len() {
            ui.separator();
            let ev = &mut state.events[idx];
            ui.label(egui::RichText::new("Event Settings").strong());
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut ev.name); });
            ui.horizontal(|ui| {
                ui.checkbox(&mut ev.enabled, "Enabled");
                ui.checkbox(&mut ev.one_shot, "One Shot");
            });
            ui.horizontal(|ui| {
                ui.label("Cooldown:");
                ui.add(egui::DragValue::new(&mut ev.cooldown_ms).speed(10.0).suffix(" ms").clamp_range(0.0..=60000.0));
            });
            ui.separator();
            ui.label(egui::RichText::new("Sound Config").strong());
            let s = &mut ev.sound;
            ui.horizontal(|ui| { ui.label("Clip:"); ui.text_edit_singleline(&mut s.clip_name); });
            ui.horizontal(|ui| {
                ui.label("Volume:");
                ui.add(egui::Slider::new(&mut s.volume_db, -60.0..=12.0).suffix(" dB"));
            });
            ui.horizontal(|ui| {
                ui.label("Pitch:");
                ui.add(egui::Slider::new(&mut s.pitch_semitones, -24.0..=24.0).suffix(" st"));
                ui.label("±");
                ui.add(egui::DragValue::new(&mut s.random_pitch_range).speed(0.1).clamp_range(0.0..=12.0).suffix(" st"));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.loop_sound, "Loop");
                if s.loop_sound {
                    ui.label("Start:");
                    ui.add(egui::DragValue::new(&mut s.loop_start).speed(0.01).clamp_range(0.0..=1.0));
                    ui.label("End:");
                    ui.add(egui::DragValue::new(&mut s.loop_end).speed(0.01).clamp_range(0.0..=1.0));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Fade In:");
                ui.add(egui::DragValue::new(&mut s.fade_in_ms).speed(1.0).clamp_range(0.0..=5000.0).suffix(" ms"));
                ui.label("Fade Out:");
                ui.add(egui::DragValue::new(&mut s.fade_out_ms).speed(1.0).clamp_range(0.0..=5000.0).suffix(" ms"));
            });
            ui.horizontal(|ui| {
                ui.label("Max Inst:");
                ui.add(egui::DragValue::new(&mut s.max_instances).clamp_range(1..=32));
                ui.label("Priority:");
                ui.add(egui::DragValue::new(&mut s.priority).clamp_range(0..=255));
                ui.checkbox(&mut s.doppler_enabled, "Doppler");
            });
        }
    }
}

// --- 3D Audio Spatialization -----------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioListener3D {
    pub position: [f32; 2],
    pub forward: [f32; 2],
    pub velocity: [f32; 2],
    pub ear_separation: f32,
    pub hrtf_enabled: bool,
    pub reverb_zone: Option<usize>,
}

impl Default for AudioListener3D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            forward: [0.0, -1.0],
            velocity: [0.0, 0.0],
            ear_separation: 0.2,
            hrtf_enabled: false,
            reverb_zone: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioSource3D {
    pub id: usize,
    pub name: String,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub volume_db: f32,
    pub spread_deg: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub attenuation: AttenuationModel,
    pub directional: bool,
    pub directional_angle_deg: f32,
    pub directional_blend: f32,
    pub occluded: bool,
    pub occlusion_db: f32,
    pub sound: SoundConfig,
    pub playing: bool,
    pub spatialize: bool,
}

impl AudioSource3D {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            name: format!("Source_{}", id),
            position: [0.0, 0.0],
            velocity: [0.0, 0.0],
            volume_db: 0.0,
            spread_deg: 360.0,
            min_distance: 1.0,
            max_distance: 20.0,
            attenuation: AttenuationModel::InverseSquare,
            directional: false,
            directional_angle_deg: 60.0,
            directional_blend: 0.5,
            occluded: false,
            occlusion_db: -12.0,
            sound: SoundConfig::default(),
            playing: false,
            spatialize: true,
        }
    }

    pub fn compute_gain_at(&self, listener: &AudioListener3D) -> f32 {
        let dx = self.position[0] - listener.position[0];
        let dy = self.position[1] - listener.position[1];
        let dist = (dx * dx + dy * dy).sqrt();
        let attn = self.attenuation.compute_gain(dist, self.min_distance, self.max_distance);
        let occ_gain = if self.occluded { 10.0f32.powf(self.occlusion_db / 20.0) } else { 1.0 };
        let vol_gain = 10.0f32.powf(self.volume_db / 20.0);
        attn * occ_gain * vol_gain
    }

    pub fn compute_pan(&self, listener: &AudioListener3D) -> f32 {
        let dx = self.position[0] - listener.position[0];
        let dy = self.position[1] - listener.position[1];
        let angle = dy.atan2(dx);
        let lx = listener.forward[1];
        let ly = -listener.forward[0];
        let dot = dx * lx + dy * ly;
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        (dot / len).clamp(-1.0, 1.0)
    }

    pub fn compute_doppler(&self, listener: &AudioListener3D, speed_of_sound: f32) -> f32 {
        let dx = self.position[0] - listener.position[0];
        let dy = self.position[1] - listener.position[1];
        let dist = (dx * dx + dy * dy).sqrt().max(0.001);
        let dir_x = dx / dist;
        let dir_y = dy / dist;
        let vs = self.velocity[0] * dir_x + self.velocity[1] * dir_y;
        let vl = listener.velocity[0] * dir_x + listener.velocity[1] * dir_y;
        (speed_of_sound + vl) / (speed_of_sound + vs).max(0.001)
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Spatialization3DState {
    pub listener: AudioListener3D,
    pub sources: Vec<AudioSource3D>,
    pub selected: Option<usize>,
    pub speed_of_sound: f32,
    pub show_attenuation_curves: bool,
    pub show_2d_view: bool,
}

impl Spatialization3DState {
    pub fn new() -> Self {
        Self { speed_of_sound: 343.0, show_2d_view: true, ..Default::default() }
    }
}

pub fn show_spatialization_editor(ui: &mut egui::Ui, state: &mut Spatialization3DState) {
    ui.heading("3D Audio Spatialization");
    ui.separator();

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_2d_view, "2D View");
        ui.checkbox(&mut state.show_attenuation_curves, "Attenuation");
        ui.label("Speed of Sound:");
        ui.add(egui::DragValue::new(&mut state.speed_of_sound).speed(1.0).clamp_range(100.0..=1000.0).suffix(" m/s"));
        if ui.button("+ Source").clicked() {
            let id = state.sources.len();
            state.sources.push(AudioSource3D::new(id));
        }
    });
    ui.separator();

    if state.show_2d_view {
        draw_spatialization_2d_view(ui, state);
        ui.separator();
    }

    let mut remove = None;
    for (i, src) in state.sources.iter().enumerate() {
        let sel = state.selected == Some(i);
        let gain = src.compute_gain_at(&state.listener);
        let pan = src.compute_pan(&state.listener);
        let doppler = src.compute_doppler(&state.listener, state.speed_of_sound);
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, format!("{} @ ({:.1},{:.1})", src.name, src.position[0], src.position[1])).clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
            ui.label(format!("G:{:.2} Pan:{:.2} D:{:.3}", gain, pan, doppler));
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.sources.remove(ri); if state.selected == Some(ri) { state.selected = None; } }

    if let Some(idx) = state.selected {
        if idx < state.sources.len() {
            ui.separator();
            let src = &mut state.sources[idx];
            ui.label(egui::RichText::new("Source Settings").strong());
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut src.name); });
            ui.horizontal(|ui| {
                ui.label("Pos:");
                ui.add(egui::DragValue::new(&mut src.position[0]).speed(0.1).prefix("X: "));
                ui.add(egui::DragValue::new(&mut src.position[1]).speed(0.1).prefix("Y: "));
            });
            ui.horizontal(|ui| {
                ui.label("Min Dist:");
                ui.add(egui::DragValue::new(&mut src.min_distance).speed(0.1).clamp_range(0.1..=100.0));
                ui.label("Max Dist:");
                ui.add(egui::DragValue::new(&mut src.max_distance).speed(0.5).clamp_range(1.0..=1000.0));
            });
            ui.horizontal(|ui| {
                ui.label("Attenuation:");
                for model in &[AttenuationModel::None, AttenuationModel::Linear,
                                AttenuationModel::InverseSquare, AttenuationModel::Logarithmic] {
                    if ui.selectable_label(src.attenuation == *model, model.label()).clicked() {
                        src.attenuation = model.clone();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut src.directional, "Directional");
                if src.directional {
                    ui.label("Angle:");
                    ui.add(egui::Slider::new(&mut src.directional_angle_deg, 10.0..=360.0).suffix("°"));
                }
                ui.checkbox(&mut src.occluded, "Occluded");
                if src.occluded {
                    ui.add(egui::DragValue::new(&mut src.occlusion_db).speed(0.5).clamp_range(-60.0..=0.0).suffix(" dB"));
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut src.spatialize, "Spatialize");
                ui.checkbox(&mut src.playing, "Playing");
                ui.label("Volume:");
                ui.add(egui::DragValue::new(&mut src.volume_db).speed(0.5).clamp_range(-60.0..=12.0).suffix(" dB"));
            });
            if state.show_attenuation_curves {
                draw_attenuation_curve(ui, src);
            }
        }
    }
}

pub fn draw_spatialization_2d_view(ui: &mut egui::Ui, state: &Spatialization3DState) {
    let size = Vec2::new(ui.available_width().min(340.0), 220.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(40, 45, 60)));

    let scale = 8.0f32;
    let cx = rect.center().x;
    let cy = rect.center().y;

    let world_to_screen = |wx: f32, wy: f32| -> Pos2 {
        Pos2::new(cx + wx * scale, cy - wy * scale)
    };

    // Grid
    let grid_step = 5.0f32;
    let num_lines = 8;
    for i in -num_lines..=num_lines {
        let gx = i as f32 * grid_step;
        let p0 = world_to_screen(gx, -num_lines as f32 * grid_step);
        let p1 = world_to_screen(gx, num_lines as f32 * grid_step);
        p.line_segment([p0, p1], Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 45, 60, 100)));
        let gy = i as f32 * grid_step;
        let p0 = world_to_screen(-num_lines as f32 * grid_step, gy);
        let p1 = world_to_screen(num_lines as f32 * grid_step, gy);
        p.line_segment([p0, p1], Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 45, 60, 100)));
    }

    // Listener
    let lp = world_to_screen(state.listener.position[0], state.listener.position[1]);
    p.circle_filled(lp, 8.0, Color32::from_rgb(80, 200, 255));
    p.text(lp + Vec2::new(0.0, -12.0), egui::Align2::CENTER_CENTER,
        "L", FontId::monospace(8.0), Color32::WHITE);

    // Listener forward arrow
    let fwd = world_to_screen(
        state.listener.position[0] + state.listener.forward[0] * 2.0,
        state.listener.position[1] + state.listener.forward[1] * 2.0,
    );
    p.arrow(lp, fwd - lp, Stroke::new(1.5, Color32::from_rgb(80, 200, 255)));

    // Sources
    for src in &state.sources {
        let sp = world_to_screen(src.position[0], src.position[1]);
        let gain = src.compute_gain_at(&state.listener);
        let col = Color32::from_rgb(
            (255.0 * (1.0 - gain)) as u8,
            (255.0 * gain) as u8,
            80,
        );

        // Attenuation circles
        let r_min = src.min_distance * scale;
        let r_max = src.max_distance * scale;
        p.circle_stroke(sp, r_max, Stroke::new(0.5, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 60)));
        p.circle_stroke(sp, r_min, Stroke::new(1.0, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 120)));

        p.circle_filled(sp, 6.0, col);
        p.text(sp + Vec2::new(0.0, -10.0), egui::Align2::CENTER_CENTER,
            &src.name, FontId::monospace(7.0), Color32::WHITE);

        // Gain line to listener
        let alpha = (gain * 180.0) as u8;
        p.line_segment([sp, lp], Stroke::new(1.0, Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), alpha)));
    }
}

pub fn draw_attenuation_curve(ui: &mut egui::Ui, src: &AudioSource3D) {
    let size = Vec2::new(200.0, 100.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let n = 80;
    let pts: Vec<Pos2> = (0..n).map(|i| {
        let t = i as f32 / (n - 1) as f32;
        let dist = src.min_distance + t * (src.max_distance - src.min_distance + src.max_distance * 0.5);
        let gain = src.attenuation.compute_gain(dist, src.min_distance, src.max_distance);
        Pos2::new(
            rect.left() + t * rect.width(),
            rect.bottom() - gain * rect.height() * 0.9,
        )
    }).collect();

    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], Stroke::new(2.0, Color32::from_rgb(100, 200, 255)));
    }

    p.text(rect.left_bottom() + Vec2::new(2.0, -2.0), egui::Align2::LEFT_BOTTOM,
        "dist", FontId::monospace(8.0), Color32::GRAY);
    p.text(rect.left_top() + Vec2::new(2.0, 2.0), egui::Align2::LEFT_TOP,
        "gain", FontId::monospace(8.0), Color32::GRAY);
    p.text(rect.center_top() + Vec2::new(0.0, 2.0), egui::Align2::CENTER_TOP,
        format!("{}", src.attenuation.label()), FontId::monospace(7.0), Color32::from_rgb(200, 200, 200));
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
