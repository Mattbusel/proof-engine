path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 10
// ============================================================

// --- MIDI CC Mapper --------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct MidiCcMapping {
    pub id: usize,
    pub name: String,
    pub cc_number: u8,
    pub midi_channel: u8,
    pub target_param: String,
    pub min_value: f32,
    pub max_value: f32,
    pub curve: f32,
    pub invert: bool,
    pub enabled: bool,
    pub learn_mode: bool,
    pub last_value: u8,
    pub smoothing: f32,
}

impl MidiCcMapping {
    pub fn new(id: usize, cc: u8, target: &str, min: f32, max: f32) -> Self {
        Self {
            id,
            name: format!("CC{} -> {}", cc, target),
            cc_number: cc,
            midi_channel: 0,
            target_param: target.to_string(),
            min_value: min,
            max_value: max,
            curve: 1.0,
            invert: false,
            enabled: true,
            learn_mode: false,
            last_value: 0,
            smoothing: 0.0,
        }
    }

    pub fn map_value(&self, raw: u8) -> f32 {
        let t = if self.invert { 1.0 - raw as f32 / 127.0 } else { raw as f32 / 127.0 };
        let curved = t.powf(self.curve.max(0.01));
        self.min_value + curved * (self.max_value - self.min_value)
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct MidiCcMapperState {
    pub mappings: Vec<MidiCcMapping>,
    pub selected: Option<usize>,
    pub global_learn: bool,
    pub midi_channel_filter: Option<u8>,
    pub search: String,
}

impl MidiCcMapperState {
    pub fn with_demo() -> Self {
        let mut s = Self::default();
        s.mappings = vec![
            MidiCcMapping::new(0, 1, "master.volume_db", -60.0, 0.0),
            MidiCcMapping::new(1, 7, "bus.music.volume_db", -60.0, 0.0),
            MidiCcMapping::new(2, 11, "dynamics.threshold_db", -60.0, 0.0),
            MidiCcMapping::new(3, 74, "eq.band2.gain_db", -12.0, 12.0),
            MidiCcMapping::new(4, 91, "reverb.wet_db", -60.0, 0.0),
            MidiCcMapping::new(5, 93, "delay.feedback_l", 0.0, 0.99),
            MidiCcMapping::new(6, 16, "spatial.listener.x", -50.0, 50.0),
            MidiCcMapping::new(7, 17, "spatial.listener.y", -50.0, 50.0),
        ];
        s
    }
}

pub fn show_midi_cc_mapper(ui: &mut egui::Ui, state: &mut MidiCcMapperState) {
    ui.heading("MIDI CC Mapper");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ Mapping").clicked() {
            let id = state.mappings.len();
            state.mappings.push(MidiCcMapping::new(id, 1, "param", 0.0, 1.0));
        }
        ui.checkbox(&mut state.global_learn, "Global Learn");
        ui.separator();
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search);
        ui.separator();
        ui.label("Chan:");
        let mut all = state.midi_channel_filter.is_none();
        if ui.checkbox(&mut all, "All").changed() && all { state.midi_channel_filter = None; }
        if !all {
            let mut ch = state.midi_channel_filter.unwrap_or(0);
            ui.add(egui::DragValue::new(&mut ch).clamp_range(0u8..=15).prefix("Ch "));
            state.midi_channel_filter = Some(ch);
        }
    });
    ui.separator();

    let search_lower = state.search.to_lowercase();
    let mut remove = None;

    egui::Grid::new("cc_map").num_columns(7).spacing([6.0, 3.0]).striped(true).show(ui, |ui| {
        ui.label(egui::RichText::new("Name").small().strong());
        ui.label(egui::RichText::new("CC").small().strong());
        ui.label(egui::RichText::new("Ch").small().strong());
        ui.label(egui::RichText::new("Target").small().strong());
        ui.label(egui::RichText::new("Range").small().strong());
        ui.label(egui::RichText::new("Value").small().strong());
        ui.label("");
        ui.end_row();

        for (i, m) in state.mappings.iter_mut().enumerate() {
            if !search_lower.is_empty() && !m.name.to_lowercase().contains(&search_lower) && !m.target_param.to_lowercase().contains(&search_lower) { continue; }
            let sel = state.selected == Some(i);
            if ui.selectable_label(sel, egui::RichText::new(&m.name).small()).clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
            ui.label(egui::RichText::new(format!("{}", m.cc_number)).small().monospace());
            ui.label(egui::RichText::new(format!("{}", m.midi_channel)).small());
            ui.label(egui::RichText::new(&m.target_param).small().color(Color32::from_rgb(100, 200, 255)));
            ui.label(egui::RichText::new(format!("{:.1}..{:.1}", m.min_value, m.max_value)).small());
            ui.label(egui::RichText::new(format!("{:.2}", m.map_value(m.last_value))).small().monospace());
            if ui.small_button("✗").clicked() { remove = Some(i); }
            ui.end_row();
        }
    });
    if let Some(ri) = remove { state.mappings.remove(ri); if state.selected == Some(ri) { state.selected = None; } }

    if let Some(idx) = state.selected {
        if idx < state.mappings.len() {
            ui.separator();
            let m = &mut state.mappings[idx];
            ui.label(egui::RichText::new("Mapping Settings").strong());
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut m.name); });
            ui.horizontal(|ui| {
                ui.label("CC#:"); ui.add(egui::DragValue::new(&mut m.cc_number).clamp_range(0u8..=127));
                ui.label("Channel:"); ui.add(egui::DragValue::new(&mut m.midi_channel).clamp_range(0u8..=15));
                ui.checkbox(&mut m.learn_mode, "Learn");
            });
            ui.horizontal(|ui| { ui.label("Target:"); ui.text_edit_singleline(&mut m.target_param); });
            ui.horizontal(|ui| {
                ui.label("Min:"); ui.add(egui::DragValue::new(&mut m.min_value).speed(0.1));
                ui.label("Max:"); ui.add(egui::DragValue::new(&mut m.max_value).speed(0.1));
            });
            ui.horizontal(|ui| {
                ui.label("Curve:"); ui.add(egui::Slider::new(&mut m.curve, 0.1..=4.0).logarithmic(true));
                ui.checkbox(&mut m.invert, "Invert");
                ui.checkbox(&mut m.enabled, "Enabled");
            });
            ui.horizontal(|ui| {
                ui.label("Smoothing:"); ui.add(egui::Slider::new(&mut m.smoothing, 0.0..=0.99));
            });
            draw_cc_curve(ui, m);
        }
    }
}

pub fn draw_cc_curve(ui: &mut egui::Ui, m: &MidiCcMapping) {
    let size = Vec2::new(120.0, 60.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(10, 12, 18));
    let n = 50usize;
    let pts: Vec<Pos2> = (0..n).map(|i| {
        let raw = (i as f32 / (n - 1) as f32 * 127.0) as u8;
        let v = (m.map_value(raw) - m.min_value) / (m.max_value - m.min_value).max(0.001);
        Pos2::new(rect.left() + i as f32 / (n - 1) as f32 * rect.width(), rect.bottom() - v * rect.height() * 0.9)
    }).collect();
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(80, 200, 255)));
    }
}

// --- Ambisonic Panner ------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum AmbisonicOrder {
    First,
    Second,
    Third,
}

impl AmbisonicOrder {
    pub fn label(&self) -> &str {
        match self { AmbisonicOrder::First => "1st Order", AmbisonicOrder::Second => "2nd Order", AmbisonicOrder::Third => "3rd Order" }
    }
    pub fn channel_count(&self) -> usize {
        match self { AmbisonicOrder::First => 4, AmbisonicOrder::Second => 9, AmbisonicOrder::Third => 16 }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AmbisonicSource {
    pub id: usize,
    pub name: String,
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub distance: f32,
    pub volume_db: f32,
    pub enabled: bool,
    pub near_field: bool,
}

impl AmbisonicSource {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            name: format!("Source_{}", id),
            azimuth_deg: 0.0,
            elevation_deg: 0.0,
            distance: 1.0,
            volume_db: 0.0,
            enabled: true,
            near_field: false,
        }
    }

    pub fn acn_weights(&self, order: &AmbisonicOrder) -> Vec<f32> {
        let az = self.azimuth_deg.to_radians();
        let el = self.elevation_deg.to_radians();
        let n = order.channel_count();
        let mut w = vec![0.0f32; n];
        // W (omni)
        w[0] = 1.0;
        if n >= 4 {
            // First order
            w[1] = el.cos() * az.sin(); // Y
            w[2] = el.sin();             // Z
            w[3] = el.cos() * az.cos(); // X
        }
        if n >= 9 {
            // Second order (simplified)
            w[4] = (3.0 * el.sin().powi(2) - 1.0) / 2.0;
            w[5] = (3.0f32).sqrt() * el.cos() * el.sin() * az.cos();
            w[6] = (3.0f32).sqrt() * el.cos() * el.sin() * az.sin();
            w[7] = (3.0f32).sqrt() / 2.0 * el.cos().powi(2) * (2.0 * az).cos();
            w[8] = (3.0f32).sqrt() / 2.0 * el.cos().powi(2) * (2.0 * az).sin();
        }
        w
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AmbisonicPannerState {
    pub order: AmbisonicOrder,
    pub sources: Vec<AmbisonicSource>,
    pub selected: Option<usize>,
    pub decoder_type: String,
    pub binaural_enabled: bool,
    pub hrir_name: String,
}

impl AmbisonicPannerState {
    pub fn new() -> Self {
        Self {
            order: AmbisonicOrder::First,
            decoder_type: "AllRAD".into(),
            hrir_name: "kemar.sofa".into(),
            ..Default::default()
        }
    }
}

pub fn show_ambisonic_panner(ui: &mut egui::Ui, state: &mut AmbisonicPannerState) {
    ui.heading("Ambisonic Panner");
    ui.separator();

    ui.horizontal(|ui| {
        for order in &[AmbisonicOrder::First, AmbisonicOrder::Second, AmbisonicOrder::Third] {
            if ui.selectable_label(state.order == *order, format!("{} ({}ch)", order.label(), order.channel_count())).clicked() {
                state.order = order.clone();
            }
        }
        ui.separator();
        ui.checkbox(&mut state.binaural_enabled, "Binaural Decode");
        if state.binaural_enabled {
            ui.text_edit_singleline(&mut state.hrir_name);
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ Source").clicked() {
            let id = state.sources.len();
            state.sources.push(AmbisonicSource::new(id));
        }
    });

    draw_ambisonic_sphere_view(ui, state);
    ui.separator();

    let mut remove = None;
    for (i, src) in state.sources.iter_mut().enumerate() {
        let sel = state.selected == Some(i);
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, &src.name).clicked() { state.selected = if sel { None } else { Some(i) }; }
            ui.add(egui::DragValue::new(&mut src.azimuth_deg).speed(1.0).clamp_range(-180.0..=180.0).suffix("°").prefix("Az: "));
            ui.add(egui::DragValue::new(&mut src.elevation_deg).speed(1.0).clamp_range(-90.0..=90.0).suffix("°").prefix("El: "));
            ui.add(egui::DragValue::new(&mut src.distance).speed(0.05).clamp_range(0.1..=100.0).suffix("m").prefix("D: "));
            ui.checkbox(&mut src.enabled, "");
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.sources.remove(ri); if state.selected == Some(ri) { state.selected = None; } }
}

pub fn draw_ambisonic_sphere_view(ui: &mut egui::Ui, state: &AmbisonicPannerState) {
    let size = Vec2::new(220.0, 220.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(30, 35, 50)));

    let cx = rect.center().x;
    let cy = rect.center().y;
    let r = rect.width().min(rect.height()) * 0.45;

    // Grid circles
    for ri in 1..=3 {
        p.circle_stroke(rect.center(), r * ri as f32 / 3.0,
            Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 50, 70, 120)));
    }
    // Cross
    p.line_segment([Pos2::new(cx - r, cy), Pos2::new(cx + r, cy)], Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 50, 70, 120)));
    p.line_segment([Pos2::new(cx, cy - r), Pos2::new(cx, cy + r)], Stroke::new(0.5, Color32::from_rgba_premultiplied(40, 50, 70, 120)));

    // Labels
    p.text(Pos2::new(cx, cy - r - 8.0), egui::Align2::CENTER_CENTER, "0°", FontId::monospace(7.0), Color32::GRAY);
    p.text(Pos2::new(cx + r + 4.0, cy), egui::Align2::LEFT_CENTER, "90°", FontId::monospace(7.0), Color32::GRAY);
    p.text(Pos2::new(cx, cy + r + 8.0), egui::Align2::CENTER_CENTER, "180°", FontId::monospace(7.0), Color32::GRAY);
    p.text(Pos2::new(cx - r - 4.0, cy), egui::Align2::RIGHT_CENTER, "270°", FontId::monospace(7.0), Color32::GRAY);

    // Sources
    for src in &state.sources {
        if !src.enabled { continue; }
        let az = src.azimuth_deg.to_radians();
        let el = src.elevation_deg.to_radians();
        let dist_scale = (1.0 - src.elevation_deg.abs() / 90.0) * r * (src.distance / 10.0).min(1.0);
        let sx = cx + az.sin() * dist_scale;
        let sy = cy - az.cos() * dist_scale;
        let alpha = (200.0 * (1.0 - src.distance / 20.0).max(0.1)) as u8;
        let col = Color32::from_rgba_premultiplied(80, 200, 255, alpha);
        p.circle_filled(Pos2::new(sx, sy), 5.0, col);
        let el_r = el.cos() * 4.0;
        p.circle_stroke(Pos2::new(sx, sy), el_r.abs() + 5.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(80, 200, 255, 80)));
        p.text(Pos2::new(sx, sy - 8.0), egui::Align2::CENTER_CENTER,
            &src.name, FontId::monospace(7.0), Color32::WHITE);
    }

    // Listener at center
    p.circle_filled(rect.center(), 4.0, Color32::from_rgb(255, 220, 60));
    p.text(rect.center() + Vec2::new(0.0, -10.0), egui::Align2::CENTER_CENTER, "L", FontId::monospace(7.0), Color32::WHITE);
}

// --- Audio Plugin Rack -----------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginFormat {
    Vst2,
    Vst3,
    Au,
    Clap,
    Ladspa,
    Lv2,
    Builtin,
}

impl PluginFormat {
    pub fn label(&self) -> &str {
        match self {
            PluginFormat::Vst2 => "VST2",
            PluginFormat::Vst3 => "VST3",
            PluginFormat::Au => "AU",
            PluginFormat::Clap => "CLAP",
            PluginFormat::Ladspa => "LADSPA",
            PluginFormat::Lv2 => "LV2",
            PluginFormat::Builtin => "Built-in",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioPlugin {
    pub id: usize,
    pub name: String,
    pub vendor: String,
    pub format: PluginFormat,
    pub category: String,
    pub version: String,
    pub enabled: bool,
    pub bypass: bool,
    pub wet_dry: f32,
    pub preset_name: String,
    pub has_gui: bool,
    pub latency_samples: u32,
    pub param_count: u32,
}

impl AudioPlugin {
    pub fn new(id: usize, name: &str, vendor: &str, format: PluginFormat, cat: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            vendor: vendor.to_string(),
            format,
            category: cat.to_string(),
            version: "1.0.0".into(),
            enabled: true,
            bypass: false,
            wet_dry: 1.0,
            preset_name: "Default".into(),
            has_gui: true,
            latency_samples: 0,
            param_count: 8,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PluginRackState {
    pub plugins: Vec<AudioPlugin>,
    pub selected: Option<usize>,
    pub search: String,
    pub filter_format: Option<PluginFormat>,
    pub filter_category: String,
    pub scan_running: bool,
    pub scan_progress: f32,
    pub total_latency_samples: u32,
}

impl PluginRackState {
    pub fn with_demo() -> Self {
        let mut s = Self::default();
        s.plugins = vec![
            AudioPlugin::new(0, "Compressor Pro", "FabFilter", PluginFormat::Vst3, "Dynamics"),
            AudioPlugin::new(1, "Pro-Q 3", "FabFilter", PluginFormat::Vst3, "EQ"),
            AudioPlugin::new(2, "Reverb Hall", "Valhalla", PluginFormat::Vst3, "Reverb"),
            AudioPlugin::new(3, "Transient Shaper", "Waves", PluginFormat::Vst2, "Dynamics"),
            AudioPlugin::new(4, "Tape Saturation", "Softube", PluginFormat::Vst3, "Saturation"),
            AudioPlugin::new(5, "Width Designer", "iZotope", PluginFormat::Vst3, "Stereo"),
            AudioPlugin::new(6, "Limiter", "Invisible Limiter", PluginFormat::Vst3, "Dynamics"),
        ];
        s.total_latency_samples = s.plugins.iter().filter(|p| p.enabled && !p.bypass).map(|p| p.latency_samples).sum();
        s
    }
}

pub fn show_plugin_rack(ui: &mut egui::Ui, state: &mut PluginRackState) {
    ui.heading("Audio Plugin Rack");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Scan Plugins").clicked() { state.scan_running = true; state.scan_progress = 0.0; }
        if state.scan_running {
            ui.add(egui::ProgressBar::new(state.scan_progress).desired_width(100.0));
        }
        ui.separator();
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search);
        ui.label("Cat:");
        ui.text_edit_singleline(&mut state.filter_category);
        ui.label("Format:");
        if ui.selectable_label(state.filter_format.is_none(), "All").clicked() { state.filter_format = None; }
        for fmt in &[PluginFormat::Vst2, PluginFormat::Vst3, PluginFormat::Au, PluginFormat::Clap, PluginFormat::Builtin] {
            let sel = state.filter_format.as_ref() == Some(fmt);
            if ui.selectable_label(sel, fmt.label()).clicked() {
                state.filter_format = if sel { None } else { Some(fmt.clone()) };
            }
        }
    });

    ui.label(egui::RichText::new(format!("Total Latency: {} samples ({:.1} ms @ 44.1kHz)",
        state.total_latency_samples, state.total_latency_samples as f32 / 44.1)).small().color(Color32::GRAY));
    ui.separator();

    let search_lower = state.search.to_lowercase();
    let mut remove = None;
    let mut move_up = None;
    let mut move_dn = None;
    let n = state.plugins.len();

    for (i, plugin) in state.plugins.iter_mut().enumerate() {
        if !search_lower.is_empty() && !plugin.name.to_lowercase().contains(&search_lower) { continue; }
        if !state.filter_category.is_empty() && !plugin.category.to_lowercase().contains(&state.filter_category.to_lowercase()) { continue; }
        if let Some(ref ff) = state.filter_format { if plugin.format != *ff { continue; } }

        let sel = state.selected == Some(i);
        let col = if plugin.bypass { Color32::GRAY } else if !plugin.enabled { Color32::from_rgb(100, 80, 80) } else { Color32::WHITE };

        ui.horizontal(|ui| {
            if ui.selectable_label(sel, egui::RichText::new(format!("{} — {} [{}]",
                plugin.name, plugin.vendor, plugin.format.label())).color(col)).clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
            ui.label(egui::RichText::new(&plugin.category).small().color(Color32::from_rgb(150, 150, 200)));
            ui.add(egui::DragValue::new(&mut plugin.wet_dry).speed(0.01).clamp_range(0.0..=1.0).prefix("W/D: "));
            ui.checkbox(&mut plugin.enabled, "");
            ui.checkbox(&mut plugin.bypass, "BP");
            if plugin.has_gui {
                if ui.small_button("UI").clicked() {}
            }
            if ui.small_button("↑").clicked() && i > 0 { move_up = Some(i); }
            if ui.small_button("↓").clicked() && i < n - 1 { move_dn = Some(i); }
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }

    if let Some(i) = move_up { state.plugins.swap(i, i - 1); }
    if let Some(i) = move_dn { state.plugins.swap(i, i + 1); }
    if let Some(i) = remove { state.plugins.remove(i); if state.selected == Some(i) { state.selected = None; } }

    if let Some(idx) = state.selected {
        if idx < state.plugins.len() {
            ui.separator();
            let plugin = &mut state.plugins[idx];
            ui.label(egui::RichText::new("Plugin Info").strong());
            egui::Grid::new("plugin_info").num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                ui.label("Name:"); ui.label(&plugin.name); ui.end_row();
                ui.label("Vendor:"); ui.label(&plugin.vendor); ui.end_row();
                ui.label("Format:"); ui.label(plugin.format.label()); ui.end_row();
                ui.label("Category:"); ui.label(&plugin.category); ui.end_row();
                ui.label("Version:"); ui.label(&plugin.version); ui.end_row();
                ui.label("Parameters:"); ui.label(format!("{}", plugin.param_count)); ui.end_row();
                ui.label("Latency:"); ui.label(format!("{} samples", plugin.latency_samples)); ui.end_row();
            });
            ui.horizontal(|ui| { ui.label("Preset:"); ui.text_edit_singleline(&mut plugin.preset_name); if ui.button("Load").clicked() {} if ui.button("Save").clicked() {} });
        }
    }
}

// --- Final large master show function -------------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioMixerFullState {
    pub master: AudioMixerMasterState,
    pub pitch_shifter: PitchShifterState,
    pub dj_mixer: DJMixerState,
    pub test_tones: TestToneState,
    pub room_sim: RoomAcousticParams,
    pub midi_cc: MidiCcMapperState,
    pub ambisonic: AmbisonicPannerState,
    pub plugin_rack: PluginRackState,
    pub chord_scale: ChordScaleAnalyzer,
    pub sampler: SamplerEditorState,
    pub step_seq: StepSequencerState,
    pub chord_seq: ChordSequencerState,
    pub bus_comp: BusCompressorOverview,
    pub granular: GranularSynthState,
    pub ir_browser: IrBrowserState,
    pub delay: DelayDesigner,
    pub selected_panel: u8,
}

impl AudioMixerFullState {
    pub fn new() -> Self {
        Self {
            master: AudioMixerMasterState::new(),
            midi_cc: MidiCcMapperState::with_demo(),
            ambisonic: AmbisonicPannerState::new(),
            plugin_rack: PluginRackState::with_demo(),
            sampler: SamplerEditorState::with_demo(),
            step_seq: StepSequencerState::default_8step(),
            chord_seq: ChordSequencerState::default_progression(),
            bus_comp: BusCompressorOverview::with_demo(),
            ir_browser: IrBrowserState::with_demo_irs(),
            ..Default::default()
        }
    }
}

pub fn show_audio_mixer_full(ui: &mut egui::Ui, state: &mut AudioMixerFullState) {
    ui.heading("Audio Mixer — Full Edition");
    ui.separator();

    let panels = [
        "Master Mix", "Pitch/Harm", "DJ Mixer", "Test Tones", "Room Sim",
        "MIDI CC", "Ambisonic", "Plugins", "Chord/Scale", "Sampler",
        "Step Seq", "Chord Seq", "Bus Comp", "Granular", "IR Browser", "Delay", "Overview",
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
        0 => show_audio_mixer_master(ui, &mut state.master),
        1 => show_pitch_shifter(ui, &mut state.pitch_shifter),
        2 => show_dj_mixer(ui, &mut state.dj_mixer),
        3 => show_test_tone_generator(ui, &mut state.test_tones),
        4 => show_room_acoustic_simulator(ui, &mut state.room_sim),
        5 => show_midi_cc_mapper(ui, &mut state.midi_cc),
        6 => show_ambisonic_panner(ui, &mut state.ambisonic),
        7 => show_plugin_rack(ui, &mut state.plugin_rack),
        8 => show_chord_scale_analyzer(ui, &mut state.chord_scale),
        9 => show_sampler_editor(ui, &mut state.sampler),
        10 => show_step_sequencer(ui, &mut state.step_seq),
        11 => show_chord_sequencer(ui, &mut state.chord_seq),
        12 => show_bus_compressor_overview(ui, &mut state.bus_comp),
        13 => show_granular_synth(ui, &mut state.granular),
        14 => show_ir_browser(ui, &mut state.ir_browser),
        15 => show_delay_designer(ui, &mut state.delay),
        16 => show_audio_system_overview(ui),
        _ => {}
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
