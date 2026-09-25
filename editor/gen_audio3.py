path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 3
// ============================================================

// --- Audio Scripting Editor ------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum AudioScriptLanguage {
    Lua,
    Wren,
    AudioScript,
}

impl AudioScriptLanguage {
    pub fn label(&self) -> &str {
        match self {
            AudioScriptLanguage::Lua => "Lua",
            AudioScriptLanguage::Wren => "Wren",
            AudioScriptLanguage::AudioScript => "AudioScript",
        }
    }
}

impl Default for AudioScriptLanguage {
    fn default() -> Self { AudioScriptLanguage::Lua }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioScript {
    pub id: usize,
    pub name: String,
    pub language: AudioScriptLanguage,
    pub code: String,
    pub enabled: bool,
    pub auto_run: bool,
    pub trigger: AudioTriggerType,
    pub last_error: String,
    pub last_run_ms: f32,
}

impl AudioScript {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            name: format!("script_{}", id),
            language: AudioScriptLanguage::Lua,
            code: DEFAULT_AUDIO_SCRIPT.to_string(),
            enabled: true,
            auto_run: false,
            trigger: AudioTriggerType::Manual,
            last_error: String::new(),
            last_run_ms: 0.0,
        }
    }
}

const DEFAULT_AUDIO_SCRIPT: &str = r#"-- Audio Script Example
-- Available API:
--   audio.play(clip, bus, volume_db, pitch_st)
--   audio.stop(handle)
--   audio.set_volume(bus_idx, db)
--   audio.get_level(bus_idx) -> db
--   audio.set_effect_param(bus, effect, param, value)
--   audio.fade(handle, target_db, duration_ms)
--   mixer.get_bpm() -> float
--   mixer.get_beat() -> float

function on_beat(beat)
    if beat % 4 == 0 then
        audio.play("kick.wav", 0, 0.0, 0.0)
    end
    if beat % 2 == 0 then
        audio.play("hihat.wav", 0, -6.0, 0.0)
    end
end

function on_event(name, data)
    if name == "footstep" then
        local pitch = math.random(-2, 2)
        audio.play("footstep_" .. data.surface .. ".ogg", 1, -3.0, pitch)
    end
end
"#;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioScriptingState {
    pub scripts: Vec<AudioScript>,
    pub selected: Option<usize>,
    pub console_output: Vec<String>,
    pub max_console_lines: usize,
    pub show_api_reference: bool,
}

pub fn show_audio_scripting_editor(ui: &mut egui::Ui, state: &mut AudioScriptingState) {
    ui.heading("Audio Scripting Editor");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ New Script").clicked() {
            let id = state.scripts.len();
            state.scripts.push(AudioScript::new(id));
            state.selected = Some(id);
        }
        ui.checkbox(&mut state.show_api_reference, "API Reference");
    });
    ui.separator();

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new("Scripts").strong());

        let mut remove = None;
        for (i, script) in state.scripts.iter().enumerate() {
            let sel = state.selected == Some(i);
            let col = if script.enabled { Color32::WHITE } else { Color32::GRAY };
            ui.horizontal(|ui| {
                if ui.selectable_label(sel, egui::RichText::new(&script.name).color(col)).clicked() {
                    state.selected = Some(i);
                }
                ui.label(egui::RichText::new(script.language.label()).small().color(Color32::from_rgb(100, 180, 255)));
                if !script.last_error.is_empty() {
                    ui.colored_label(Color32::RED, "!");
                }
                if ui.small_button("▶").clicked() {}
                if ui.small_button("✗").clicked() { remove = Some(i); }
            });
        }
        if let Some(ri) = remove {
            state.scripts.remove(ri);
            if state.selected == Some(ri) { state.selected = None; }
        }

        if state.show_api_reference {
            ui.separator();
            show_audio_api_reference(ui);
        }

        let ui = &mut cols[1];
        if let Some(idx) = state.selected {
            if idx < state.scripts.len() {
                let script = &mut state.scripts[idx];
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut script.name);
                });
                ui.horizontal(|ui| {
                    for lang in &[AudioScriptLanguage::Lua, AudioScriptLanguage::Wren, AudioScriptLanguage::AudioScript] {
                        if ui.selectable_label(script.language == *lang, lang.label()).clicked() {
                            script.language = lang.clone();
                        }
                    }
                    ui.separator();
                    ui.checkbox(&mut script.enabled, "Enabled");
                    ui.checkbox(&mut script.auto_run, "Auto-run");
                });
                if !script.last_error.is_empty() {
                    ui.colored_label(Color32::RED, format!("Error: {}", script.last_error));
                }
                egui::ScrollArea::vertical()
                    .id_source("script_code")
                    .max_height(250.0)
                    .show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(&mut script.code)
                            .font(FontId::monospace(10.0))
                            .desired_rows(16)
                            .desired_width(f32::INFINITY)
                            .code_editor());
                    });
            }
        } else {
            ui.label(egui::RichText::new("No script selected").color(Color32::GRAY));
        }
    });

    ui.separator();
    ui.label(egui::RichText::new("Console").strong());
    if ui.small_button("Clear").clicked() { state.console_output.clear(); }
    egui::ScrollArea::vertical()
        .id_source("console")
        .max_height(80.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for line in &state.console_output {
                let col = if line.starts_with("ERROR") { Color32::RED }
                          else if line.starts_with("WARN") { Color32::YELLOW }
                          else { Color32::from_rgb(160, 220, 160) };
                ui.colored_label(col, egui::RichText::new(line).font(FontId::monospace(9.0)));
            }
        });
}

pub fn show_audio_api_reference(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("API Reference").strong());
    egui::ScrollArea::vertical().id_source("api_ref").max_height(200.0).show(ui, |ui| {
        let entries: &[(&str, &str)] = &[
            ("audio.play(clip, bus, vol, pitch)", "Play sound, returns handle"),
            ("audio.stop(handle)", "Stop a playing sound"),
            ("audio.fade(handle, db, ms)", "Fade volume over time"),
            ("audio.set_volume(bus, db)", "Set bus volume in dB"),
            ("audio.get_level(bus) -> db", "Get current RMS level"),
            ("audio.set_effect_param(bus, fx, p, v)", "Modify effect parameter"),
            ("audio.get_beat() -> float", "Current beat position"),
            ("audio.get_bpm() -> float", "Current BPM"),
            ("audio.schedule(clip, beat)", "Schedule sound at exact beat"),
            ("on_beat(beat)", "Called every beat (define to use)"),
            ("on_event(name, data)", "Called on game events"),
            ("on_level(bus, db)", "Called on threshold cross"),
        ];
        egui::Grid::new("api_grid").num_columns(2).spacing([4.0, 2.0]).striped(true).show(ui, |ui| {
            for (sig, desc) in entries {
                ui.label(egui::RichText::new(*sig).font(FontId::monospace(8.5)).color(Color32::from_rgb(100, 200, 255)));
                ui.label(egui::RichText::new(*desc).small().color(Color32::GRAY));
                ui.end_row();
            }
        });
    });
}

// --- Multi-Band EQ ---------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum EqBandType {
    LowCut,
    LowShelf,
    Bell,
    Notch,
    HighShelf,
    HighCut,
    AllPass,
}

impl EqBandType {
    pub fn label(&self) -> &str {
        match self {
            EqBandType::LowCut => "Low Cut",
            EqBandType::LowShelf => "Low Shelf",
            EqBandType::Bell => "Bell",
            EqBandType::Notch => "Notch",
            EqBandType::HighShelf => "Hi Shelf",
            EqBandType::HighCut => "Hi Cut",
            EqBandType::AllPass => "All Pass",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            EqBandType::LowCut | EqBandType::LowShelf => Color32::from_rgb(80, 140, 255),
            EqBandType::Bell => Color32::from_rgb(80, 220, 120),
            EqBandType::Notch => Color32::from_rgb(255, 80, 80),
            EqBandType::HighShelf | EqBandType::HighCut => Color32::from_rgb(255, 180, 60),
            EqBandType::AllPass => Color32::GRAY,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EqBand {
    pub band_type: EqBandType,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
    pub slope: u8,
}

impl EqBand {
    pub fn new_bell(freq: f32) -> Self {
        Self { band_type: EqBandType::Bell, freq, gain_db: 0.0, q: 1.0, enabled: true, slope: 1 }
    }
    pub fn new_low_cut(freq: f32) -> Self {
        Self { band_type: EqBandType::LowCut, freq, gain_db: 0.0, q: 0.707, enabled: true, slope: 2 }
    }
    pub fn new_high_cut(freq: f32) -> Self {
        Self { band_type: EqBandType::HighCut, freq, gain_db: 0.0, q: 0.707, enabled: true, slope: 2 }
    }

    pub fn magnitude_at(&self, f: f32) -> f32 {
        if !self.enabled { return 0.0; }
        let ratio = f / self.freq.max(1.0);
        match self.band_type {
            EqBandType::Bell => {
                let bw = self.freq / self.q.max(0.01);
                let df = f - self.freq;
                self.gain_db * (1.0 / (1.0 + (df / bw * 2.0).powi(2)))
            }
            EqBandType::LowShelf => {
                if f < self.freq { self.gain_db } else { self.gain_db * (1.0 - (ratio - 1.0).min(1.0)) }
            }
            EqBandType::HighShelf => {
                if f > self.freq { self.gain_db } else { self.gain_db * (1.0 - (1.0 / ratio - 1.0).min(1.0)) }
            }
            EqBandType::Notch => {
                let bw = self.freq / self.q.max(0.01);
                let df = f - self.freq;
                -12.0 / (1.0 + (df / bw * 2.0).powi(2))
            }
            EqBandType::LowCut => {
                if f > self.freq { 0.0 } else {
                    let order = self.slope as f32 * 6.0;
                    -order * (ratio).log2().min(0.0)
                }
            }
            EqBandType::HighCut => {
                if f < self.freq { 0.0 } else {
                    let order = self.slope as f32 * 6.0;
                    -order * (1.0/ratio).log2().min(0.0)
                }
            }
            EqBandType::AllPass => 0.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MultibandEq {
    pub bands: Vec<EqBand>,
    pub enabled: bool,
    pub selected_band: Option<usize>,
    pub analyzer_enabled: bool,
    pub high_quality: bool,
}

impl Default for MultibandEq {
    fn default() -> Self {
        Self {
            bands: vec![
                EqBand::new_low_cut(20.0),
                EqBand { band_type: EqBandType::LowShelf, freq: 80.0, gain_db: 0.0, q: 0.707, enabled: true, slope: 1 },
                EqBand::new_bell(250.0),
                EqBand::new_bell(1000.0),
                EqBand::new_bell(4000.0),
                EqBand { band_type: EqBandType::HighShelf, freq: 10000.0, gain_db: 0.0, q: 0.707, enabled: true, slope: 1 },
                EqBand::new_high_cut(20000.0),
            ],
            enabled: true,
            selected_band: Some(2),
            analyzer_enabled: true,
            high_quality: true,
        }
    }
}

impl MultibandEq {
    pub fn total_magnitude_at(&self, f: f32) -> f32 {
        self.bands.iter().map(|b| b.magnitude_at(f)).sum()
    }
}

pub fn show_multiband_eq(ui: &mut egui::Ui, eq: &mut MultibandEq) {
    ui.heading("Multiband EQ");
    ui.horizontal(|ui| {
        ui.checkbox(&mut eq.enabled, "Enable");
        ui.checkbox(&mut eq.analyzer_enabled, "Analyzer");
        ui.checkbox(&mut eq.high_quality, "High Quality");
        if ui.button("Reset All").clicked() {
            for b in &mut eq.bands { b.gain_db = 0.0; }
        }
        if ui.button("+ Band").clicked() {
            eq.bands.push(EqBand::new_bell(1000.0));
        }
    });
    ui.separator();

    draw_eq_display(ui, eq);
    ui.separator();

    let mut remove = None;
    for (i, band) in eq.bands.iter_mut().enumerate() {
        let sel = eq.selected_band == Some(i);
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, egui::RichText::new(band.band_type.label()).color(band.band_type.color()).small()).clicked() {
                eq.selected_band = Some(i);
            }
            ui.add(egui::DragValue::new(&mut band.freq).speed(5.0).clamp_range(20.0..=20000.0).suffix(" Hz"));
            if !matches!(band.band_type, EqBandType::LowCut | EqBandType::HighCut | EqBandType::AllPass) {
                ui.add(egui::DragValue::new(&mut band.gain_db).speed(0.1).clamp_range(-24.0..=24.0).suffix(" dB"));
            }
            ui.add(egui::DragValue::new(&mut band.q).speed(0.01).clamp_range(0.1..=20.0).prefix("Q:"));
            ui.checkbox(&mut band.enabled, "");
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove {
        eq.bands.remove(ri);
        if eq.selected_band == Some(ri) { eq.selected_band = None; }
    }

    if let Some(idx) = eq.selected_band {
        if idx < eq.bands.len() {
            ui.separator();
            let band = &mut eq.bands[idx];
            ui.horizontal(|ui| {
                ui.label("Type:");
                for bt in &[EqBandType::LowCut, EqBandType::LowShelf, EqBandType::Bell,
                             EqBandType::Notch, EqBandType::HighShelf, EqBandType::HighCut, EqBandType::AllPass] {
                    if ui.selectable_label(band.band_type == *bt, egui::RichText::new(bt.label()).small().color(bt.color())).clicked() {
                        band.band_type = bt.clone();
                    }
                }
            });
        }
    }
}

pub fn draw_eq_display(ui: &mut egui::Ui, eq: &MultibandEq) {
    let size = Vec2::new(ui.available_width().min(520.0), 160.0);
    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(40, 45, 60)));

    let freq_to_x = |f: f32| -> f32 {
        let t = (f.log10() - (20.0f32).log10()) / ((20000.0f32).log10() - (20.0f32).log10());
        rect.left() + t * rect.width()
    };
    let db_to_y = |db: f32| -> f32 {
        rect.center().y - (db / 24.0) * rect.height() * 0.45
    };

    // dB grid
    for db in [-24.0f32, -12.0, -6.0, 0.0, 6.0, 12.0, 24.0] {
        let y = db_to_y(db);
        let col = if db == 0.0 {
            Color32::from_rgba_premultiplied(100, 100, 120, 160)
        } else {
            Color32::from_rgba_premultiplied(40, 45, 60, 100)
        };
        p.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(if db == 0.0 { 1.0 } else { 0.5 }, col));
        p.text(Pos2::new(rect.left() + 2.0, y), egui::Align2::LEFT_CENTER,
            format!("{:+.0}", db), FontId::monospace(7.0), Color32::from_rgb(80, 80, 100));
    }

    // Frequency grid
    for &f in &[50.0f32, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0] {
        let x = freq_to_x(f);
        p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 45, 60, 80)));
        let lbl = if f >= 1000.0 { format!("{}k", (f / 1000.0) as u32) } else { format!("{}", f as u32) };
        p.text(Pos2::new(x, rect.bottom() - 8.0), egui::Align2::CENTER_CENTER,
            lbl, FontId::monospace(7.0), Color32::from_rgb(60, 70, 90));
    }

    // Individual band curves
    let n_pts = 200usize;
    for (bi, band) in eq.bands.iter().enumerate() {
        if !band.enabled { continue; }
        let pts: Vec<Pos2> = (0..n_pts).map(|i| {
            let t = i as f32 / (n_pts - 1) as f32;
            let f = 20.0f32 * (1000.0f32).powf(t);
            let db = band.magnitude_at(f);
            Pos2::new(freq_to_x(f), db_to_y(db))
        }).collect();
        let col_base = band.band_type.color();
        let alpha = if eq.selected_band == Some(bi) { 200u8 } else { 80u8 };
        let col = Color32::from_rgba_premultiplied(col_base.r(), col_base.g(), col_base.b(), alpha);
        for w in pts.windows(2) {
            p.line_segment([w[0], w[1]], Stroke::new(1.0, col));
        }
        // Band handle
        let hx = freq_to_x(band.freq);
        let hy = db_to_y(band.gain_db);
        let hcol = if eq.selected_band == Some(bi) { col_base } else {
            Color32::from_rgba_premultiplied(col_base.r(), col_base.g(), col_base.b(), 140)
        };
        p.circle_stroke(Pos2::new(hx, hy), 5.0, Stroke::new(1.5, hcol));
    }

    // Combined response curve
    let total_pts: Vec<Pos2> = (0..n_pts).map(|i| {
        let t = i as f32 / (n_pts - 1) as f32;
        let f = 20.0f32 * (1000.0f32).powf(t);
        let db = eq.total_magnitude_at(f);
        Pos2::new(freq_to_x(f), db_to_y(db))
    }).collect();
    for w in total_pts.windows(2) {
        p.line_segment([w[0], w[1]], Stroke::new(2.0, Color32::from_rgb(220, 220, 255)));
    }
}

// --- Bus Routing -----------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioBusRoute {
    pub from_bus: usize,
    pub to_bus: usize,
    pub gain_db: f32,
    pub enabled: bool,
    pub pre_fader: bool,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct BusRoutingState {
    pub buses: Vec<String>,
    pub routes: Vec<AudioBusRoute>,
    pub selected: Option<usize>,
}

impl BusRoutingState {
    pub fn default_buses() -> Self {
        Self {
            buses: vec![
                "Master".into(), "Music".into(), "SFX".into(),
                "Voice".into(), "Ambient".into(), "UI".into(),
                "Reverb Send".into(), "Delay Send".into(),
            ],
            routes: vec![
                AudioBusRoute { from_bus: 1, to_bus: 0, gain_db: 0.0, enabled: true, pre_fader: false },
                AudioBusRoute { from_bus: 2, to_bus: 0, gain_db: 0.0, enabled: true, pre_fader: false },
                AudioBusRoute { from_bus: 3, to_bus: 0, gain_db: 0.0, enabled: true, pre_fader: false },
                AudioBusRoute { from_bus: 4, to_bus: 0, gain_db: -3.0, enabled: true, pre_fader: false },
                AudioBusRoute { from_bus: 5, to_bus: 0, gain_db: 0.0, enabled: true, pre_fader: false },
                AudioBusRoute { from_bus: 2, to_bus: 6, gain_db: -12.0, enabled: true, pre_fader: false },
                AudioBusRoute { from_bus: 3, to_bus: 7, gain_db: -18.0, enabled: false, pre_fader: false },
            ],
            selected: None,
        }
    }
}

pub fn show_bus_routing(ui: &mut egui::Ui, state: &mut BusRoutingState) {
    ui.heading("Bus Routing Matrix");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ Route").clicked() {
            state.routes.push(AudioBusRoute { from_bus: 0, to_bus: 0, gain_db: 0.0, enabled: true, pre_fader: false });
        }
        if ui.button("+ Bus").clicked() {
            state.buses.push(format!("Bus_{}", state.buses.len()));
        }
    });
    ui.separator();

    draw_bus_routing_matrix(ui, state);
    ui.separator();

    ui.label(egui::RichText::new("Route List").strong());
    let mut remove = None;
    for (i, route) in state.routes.iter_mut().enumerate() {
        let from = state.buses.get(route.from_bus).map(|s| s.as_str()).unwrap_or("?");
        let to = state.buses.get(route.to_bus).map(|s| s.as_str()).unwrap_or("?");
        ui.horizontal(|ui| {
            ui.colored_label(Color32::from_rgb(100, 200, 255), from);
            ui.label("->");
            ui.colored_label(Color32::from_rgb(255, 160, 80), to);
            ui.add(egui::DragValue::new(&mut route.gain_db).speed(0.1).clamp_range(-60.0..=12.0).suffix(" dB"));
            ui.checkbox(&mut route.pre_fader, "Pre");
            ui.checkbox(&mut route.enabled, "");
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.routes.remove(ri); }
}

pub fn draw_bus_routing_matrix(ui: &mut egui::Ui, state: &BusRoutingState) {
    let n = state.buses.len().min(8);
    if n == 0 { return; }
    let cell_size = 28.0f32;
    let label_w = 80.0f32;
    let total_w = label_w + cell_size * n as f32;
    let total_h = label_w + cell_size * n as f32;
    let size = Vec2::new(total_w.min(ui.available_width()), total_h.min(300.0));
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    // Column headers
    for (j, bus) in state.buses.iter().take(n).enumerate() {
        let x = rect.left() + label_w + j as f32 * cell_size + cell_size * 0.5;
        let short: String = bus.chars().take(5).collect();
        p.text(Pos2::new(x, rect.top() + 8.0), egui::Align2::CENTER_CENTER,
            short, FontId::monospace(7.0), Color32::from_rgb(180, 180, 200));
    }

    // Row headers + cells
    for (i, bus_from) in state.buses.iter().take(n).enumerate() {
        let y = rect.top() + label_w + i as f32 * cell_size;
        let short: String = bus_from.chars().take(8).collect();
        p.text(Pos2::new(rect.left() + label_w - 2.0, y + cell_size * 0.5),
            egui::Align2::RIGHT_CENTER, short, FontId::monospace(7.0), Color32::from_rgb(180, 180, 200));

        for j in 0..n {
            let cx = rect.left() + label_w + j as f32 * cell_size;
            let cell = Rect::from_min_size(Pos2::new(cx, y), Vec2::splat(cell_size - 1.0));
            p.rect_stroke(cell, 1.0, Stroke::new(0.5, Color32::from_rgb(30, 35, 50)));

            if i == j {
                p.rect_filled(cell, 1.0, Color32::from_rgb(20, 25, 35));
                continue;
            }

            let has_route = state.routes.iter().any(|r| r.from_bus == i && r.to_bus == j && r.enabled);
            if has_route {
                p.rect_filled(cell, 1.0, Color32::from_rgba_premultiplied(80, 180, 100, 120));
                p.text(cell.center(), egui::Align2::CENTER_CENTER, "→",
                    FontId::monospace(10.0), Color32::from_rgb(80, 220, 100));
            }
        }
    }
}

// --- Waveform Editor -------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum WaveformEditTool {
    Select,
    Pencil,
    Erase,
    Silence,
    Gain,
    Normalize,
    FadeIn,
    FadeOut,
    Reverse,
}

impl WaveformEditTool {
    pub fn label(&self) -> &str {
        match self {
            WaveformEditTool::Select => "Select",
            WaveformEditTool::Pencil => "Pencil",
            WaveformEditTool::Erase => "Erase",
            WaveformEditTool::Silence => "Silence",
            WaveformEditTool::Gain => "Gain",
            WaveformEditTool::Normalize => "Normalize",
            WaveformEditTool::FadeIn => "Fade In",
            WaveformEditTool::FadeOut => "Fade Out",
            WaveformEditTool::Reverse => "Reverse",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WaveformEditor {
    pub clip_name: String,
    pub sample_rate: u32,
    pub channels: u8,
    pub duration_samples: u32,
    pub zoom_start: f32,
    pub zoom_end: f32,
    pub selection_start: Option<u32>,
    pub selection_end: Option<u32>,
    pub tool: WaveformEditTool,
    pub gain_db: f32,
    pub snap_zero_crossing: bool,
    pub show_spectrogram: bool,
    pub cursor_sample: u32,
    pub playing: bool,
    pub loop_region: bool,
}

impl Default for WaveformEditor {
    fn default() -> Self {
        Self {
            clip_name: "untitled.wav".into(),
            sample_rate: 44100,
            channels: 2,
            duration_samples: 44100 * 4,
            zoom_start: 0.0,
            zoom_end: 1.0,
            selection_start: None,
            selection_end: None,
            tool: WaveformEditTool::Select,
            gain_db: 0.0,
            snap_zero_crossing: true,
            show_spectrogram: false,
            cursor_sample: 0,
            playing: false,
            loop_region: false,
        }
    }
}

pub fn show_waveform_editor(ui: &mut egui::Ui, state: &mut WaveformEditor) {
    ui.heading("Waveform Editor");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label(&state.clip_name);
        ui.label(format!("{}Hz {}ch {:.2}s",
            state.sample_rate, state.channels,
            state.duration_samples as f32 / state.sample_rate as f32));
    });

    ui.horizontal(|ui| {
        for tool in &[WaveformEditTool::Select, WaveformEditTool::Pencil, WaveformEditTool::Erase,
                       WaveformEditTool::Silence, WaveformEditTool::Gain, WaveformEditTool::Normalize,
                       WaveformEditTool::FadeIn, WaveformEditTool::FadeOut, WaveformEditTool::Reverse] {
            if ui.selectable_label(state.tool == *tool, tool.label()).clicked() {
                state.tool = tool.clone();
            }
        }
    });

    ui.horizontal(|ui| {
        if ui.button(if state.playing { "⏸" } else { "▶" }).clicked() { state.playing = !state.playing; }
        if ui.button("⏹").clicked() { state.playing = false; state.cursor_sample = 0; }
        if ui.button("|◀").clicked() { state.cursor_sample = 0; }
        if ui.button("▶|").clicked() { state.cursor_sample = state.duration_samples; }
        ui.separator();
        ui.checkbox(&mut state.loop_region, "Loop Region");
        ui.checkbox(&mut state.snap_zero_crossing, "Snap ZC");
        ui.checkbox(&mut state.show_spectrogram, "Spectrogram");
        ui.separator();
        if state.tool == WaveformEditTool::Gain {
            ui.label("Gain:");
            ui.add(egui::Slider::new(&mut state.gain_db, -24.0..=24.0).suffix(" dB"));
        }
    });

    ui.horizontal(|ui| {
        ui.label("Zoom:");
        ui.add(egui::Slider::new(&mut state.zoom_start, 0.0..=state.zoom_end - 0.01));
        ui.label("-");
        ui.add(egui::Slider::new(&mut state.zoom_end, state.zoom_start + 0.01..=1.0));
        if ui.button("Zoom Fit").clicked() { state.zoom_start = 0.0; state.zoom_end = 1.0; }
        if let (Some(s), Some(e)) = (state.selection_start, state.selection_end) {
            if ui.button("Zoom Selection").clicked() {
                let dur = state.duration_samples as f32;
                state.zoom_start = s as f32 / dur;
                state.zoom_end = e as f32 / dur;
            }
        }
    });

    draw_waveform_display(ui, state);
    ui.separator();

    if let (Some(s), Some(e)) = (state.selection_start, state.selection_end) {
        let dur_s = (e.saturating_sub(s)) as f32 / state.sample_rate as f32;
        let s_time = s as f32 / state.sample_rate as f32;
        let e_time = e as f32 / state.sample_rate as f32;
        ui.label(format!("Selection: {:.3}s - {:.3}s  ({:.3}s, {} samples)",
            s_time, e_time, dur_s, e.saturating_sub(s)));

        ui.horizontal(|ui| {
            if ui.button("Cut").clicked() {}
            if ui.button("Copy").clicked() {}
            if ui.button("Paste").clicked() {}
            if ui.button("Delete").clicked() {}
            if ui.button("Silence").clicked() {}
            if ui.button("Normalize").clicked() {}
            if ui.button("Fade In").clicked() {}
            if ui.button("Fade Out").clicked() {}
            if ui.button("Reverse").clicked() {}
        });
    }
}

pub fn draw_waveform_display(ui: &mut egui::Ui, state: &WaveformEditor) {
    let height = if state.channels > 1 { 120.0f32 } else { 80.0f32 };
    let size = Vec2::new(ui.available_width(), height);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let n_pts = (rect.width() as usize).min(800);
    let dur = state.duration_samples as f32;
    let vis_start = (state.zoom_start * dur) as u32;
    let vis_end = (state.zoom_end * dur) as u32;
    let vis_len = vis_end.saturating_sub(vis_start).max(1);

    // Selection highlight
    if let (Some(sel_s), Some(sel_e)) = (state.selection_start, state.selection_end) {
        let s_frac = (sel_s.saturating_sub(vis_start)) as f32 / vis_len as f32;
        let e_frac = (sel_e.saturating_sub(vis_start)) as f32 / vis_len as f32;
        let sx = rect.left() + s_frac.clamp(0.0, 1.0) * rect.width();
        let ex = rect.left() + e_frac.clamp(0.0, 1.0) * rect.width();
        p.rect_filled(
            Rect::from_min_max(Pos2::new(sx, rect.top()), Pos2::new(ex, rect.bottom())),
            0.0, Color32::from_rgba_premultiplied(100, 160, 255, 40),
        );
    }

    let ch_h = rect.height() / state.channels as f32;
    for ch in 0..state.channels as usize {
        let ch_rect = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + ch as f32 * ch_h),
            Vec2::new(rect.width(), ch_h),
        );
        let mid_y = ch_rect.center().y;

        p.line_segment(
            [Pos2::new(ch_rect.left(), mid_y), Pos2::new(ch_rect.right(), mid_y)],
            Stroke::new(0.5, Color32::from_rgb(30, 35, 50)),
        );

        // Synthetic waveform
        for i in 0..n_pts {
            let t = i as f32 / n_pts as f32;
            let sample_pos = vis_start as f32 + t * vis_len as f32;
            let phase = sample_pos * std::f32::consts::TAU / 512.0;
            let amp = (phase.sin() * 0.6 + (phase * 2.1).sin() * 0.3 + (phase * 3.7).sin() * 0.1)
                * 0.8 * (1.0 + (sample_pos * 0.0001).sin() * 0.2);

            let x = ch_rect.left() + t * ch_rect.width();
            let half_h = ch_h * 0.45;
            let y_top = mid_y - amp.abs() * half_h;
            let y_bot = mid_y + amp.abs() * half_h;

            p.line_segment(
                [Pos2::new(x, y_top), Pos2::new(x, y_bot)],
                Stroke::new(1.0, Color32::from_rgb(80, 160, 220)),
            );
        }

        if ch < state.channels as usize - 1 {
            p.line_segment(
                [Pos2::new(ch_rect.left(), ch_rect.bottom()), Pos2::new(ch_rect.right(), ch_rect.bottom())],
                Stroke::new(0.5, Color32::from_rgb(30, 40, 55)),
            );
        }
    }

    // Playhead
    let cursor_frac = (state.cursor_sample.saturating_sub(vis_start)) as f32 / vis_len as f32;
    if (0.0..=1.0).contains(&cursor_frac) {
        let cx = rect.left() + cursor_frac * rect.width();
        p.line_segment([Pos2::new(cx, rect.top()), Pos2::new(cx, rect.bottom())],
            Stroke::new(1.5, Color32::from_rgb(255, 220, 80)));
    }

    // Time ruler
    p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), 14.0)),
        0.0, Color32::from_rgba_premultiplied(10, 12, 18, 200));
    let vis_secs = vis_len as f32 / state.sample_rate as f32;
    let tick_step = if vis_secs < 0.5 { 0.05 } else if vis_secs < 2.0 { 0.1 } else if vis_secs < 10.0 { 0.5 } else { 1.0 };
    let start_sec = vis_start as f32 / state.sample_rate as f32;
    let mut t_tick = (start_sec / tick_step).ceil() * tick_step;
    while t_tick < start_sec + vis_secs {
        let frac = (t_tick - start_sec) / vis_secs;
        let x = rect.left() + frac * rect.width();
        p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.top() + 14.0)],
            Stroke::new(0.5, Color32::from_rgb(60, 70, 90)));
        p.text(Pos2::new(x + 2.0, rect.top() + 2.0), egui::Align2::LEFT_TOP,
            format!("{:.2}s", t_tick), FontId::monospace(7.0), Color32::from_rgb(100, 110, 130));
        t_tick += tick_step;
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
