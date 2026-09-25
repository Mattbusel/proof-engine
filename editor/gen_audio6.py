path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 6
// ============================================================

// --- Granular Synthesizer --------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum GrainWindowType {
    Hann,
    Hamming,
    Gaussian,
    Tukey,
    Rectangular,
    Trapezoidal,
    Triangular,
}

impl GrainWindowType {
    pub fn label(&self) -> &str {
        match self {
            GrainWindowType::Hann => "Hann",
            GrainWindowType::Hamming => "Hamming",
            GrainWindowType::Gaussian => "Gaussian",
            GrainWindowType::Tukey => "Tukey",
            GrainWindowType::Rectangular => "Rectangular",
            GrainWindowType::Trapezoidal => "Trapezoidal",
            GrainWindowType::Triangular => "Triangular",
        }
    }

    pub fn envelope(&self, t: f32) -> f32 {
        let tc = t.clamp(0.0, 1.0);
        match self {
            GrainWindowType::Hann => (std::f32::consts::PI * tc).sin().powi(2),
            GrainWindowType::Hamming => 0.54 - 0.46 * (2.0 * std::f32::consts::PI * tc).cos(),
            GrainWindowType::Gaussian => {
                let sigma = 0.3f32;
                (-(tc - 0.5).powi(2) / (2.0 * sigma * sigma)).exp()
            }
            GrainWindowType::Tukey => {
                let alpha = 0.5f32;
                if tc < alpha / 2.0 { 0.5 * (1.0 + (std::f32::consts::PI * (2.0 * tc / alpha - 1.0)).cos()) }
                else if tc > 1.0 - alpha / 2.0 { 0.5 * (1.0 + (std::f32::consts::PI * (2.0 * tc / alpha - 2.0 / alpha + 1.0)).cos()) }
                else { 1.0 }
            }
            GrainWindowType::Rectangular => 1.0,
            GrainWindowType::Trapezoidal => {
                if tc < 0.1 { tc * 10.0 }
                else if tc > 0.9 { (1.0 - tc) * 10.0 }
                else { 1.0 }
            }
            GrainWindowType::Triangular => 1.0 - (tc - 0.5).abs() * 2.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GranularSynthState {
    pub clip_name: String,
    pub position: f32,
    pub position_randomness: f32,
    pub grain_size_ms: f32,
    pub grain_size_randomness: f32,
    pub density: f32,
    pub pitch_st: f32,
    pub pitch_randomness: f32,
    pub amplitude: f32,
    pub amplitude_randomness: f32,
    pub pan_randomness: f32,
    pub window: GrainWindowType,
    pub overlap: f32,
    pub reverse_chance: f32,
    pub time_stretch: f32,
    pub formant_shift: f32,
    pub spray_ms: f32,
    pub voices: u8,
    pub playing: bool,
    pub freeze: bool,
    pub scan_mode: bool,
    pub scan_speed: f32,
}

impl Default for GranularSynthState {
    fn default() -> Self {
        Self {
            clip_name: "sample.wav".into(),
            position: 0.5,
            position_randomness: 0.05,
            grain_size_ms: 80.0,
            grain_size_randomness: 0.2,
            density: 20.0,
            pitch_st: 0.0,
            pitch_randomness: 0.0,
            amplitude: 1.0,
            amplitude_randomness: 0.1,
            pan_randomness: 0.2,
            window: GrainWindowType::Hann,
            overlap: 0.5,
            reverse_chance: 0.0,
            time_stretch: 1.0,
            formant_shift: 0.0,
            spray_ms: 10.0,
            voices: 1,
            playing: false,
            freeze: false,
            scan_mode: false,
            scan_speed: 0.1,
        }
    }
}

pub fn show_granular_synth(ui: &mut egui::Ui, state: &mut GranularSynthState) {
    ui.heading("Granular Synthesizer");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Source:");
        ui.text_edit_singleline(&mut state.clip_name);
        if ui.button("Browse").clicked() {}
        ui.separator();
        if ui.button(if state.playing { "⏸" } else { "▶" }).clicked() { state.playing = !state.playing; }
        ui.checkbox(&mut state.freeze, "Freeze");
        ui.checkbox(&mut state.scan_mode, "Scan");
        if state.scan_mode {
            ui.add(egui::DragValue::new(&mut state.scan_speed).speed(0.01).clamp_range(0.0..=2.0).prefix("Speed: "));
        }
    });
    ui.separator();

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new("Grain Parameters").strong());
        ui.horizontal(|ui| { ui.label("Position:"); ui.add(egui::Slider::new(&mut state.position, 0.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Pos Random:"); ui.add(egui::Slider::new(&mut state.position_randomness, 0.0..=0.5)); });
        ui.horizontal(|ui| { ui.label("Grain Size:"); ui.add(egui::Slider::new(&mut state.grain_size_ms, 1.0..=500.0).suffix(" ms").logarithmic(true)); });
        ui.horizontal(|ui| { ui.label("Size Random:"); ui.add(egui::Slider::new(&mut state.grain_size_randomness, 0.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Density:"); ui.add(egui::Slider::new(&mut state.density, 1.0..=200.0).suffix(" g/s")); });
        ui.horizontal(|ui| { ui.label("Overlap:"); ui.add(egui::Slider::new(&mut state.overlap, 0.0..=0.99)); });
        ui.horizontal(|ui| { ui.label("Spray:"); ui.add(egui::Slider::new(&mut state.spray_ms, 0.0..=500.0).suffix(" ms")); });
        ui.horizontal(|ui| { ui.label("Voices:"); ui.add(egui::DragValue::new(&mut state.voices).clamp_range(1u8..=8)); });

        ui.separator();
        ui.label(egui::RichText::new("Window").strong());
        ui.horizontal_wrapped(|ui| {
            for w in &[GrainWindowType::Hann, GrainWindowType::Hamming, GrainWindowType::Gaussian,
                       GrainWindowType::Tukey, GrainWindowType::Rectangular, GrainWindowType::Trapezoidal, GrainWindowType::Triangular] {
                if ui.selectable_label(state.window == *w, w.label()).clicked() { state.window = w.clone(); }
            }
        });
        draw_grain_window(ui, &state.window);

        let ui = &mut cols[1];
        ui.label(egui::RichText::new("Pitch & Amplitude").strong());
        ui.horizontal(|ui| { ui.label("Pitch:"); ui.add(egui::Slider::new(&mut state.pitch_st, -24.0..=24.0).suffix(" st")); });
        ui.horizontal(|ui| { ui.label("Pitch Random:"); ui.add(egui::Slider::new(&mut state.pitch_randomness, 0.0..=12.0).suffix(" st")); });
        ui.horizontal(|ui| { ui.label("Amplitude:"); ui.add(egui::Slider::new(&mut state.amplitude, 0.0..=2.0)); });
        ui.horizontal(|ui| { ui.label("Amp Random:"); ui.add(egui::Slider::new(&mut state.amplitude_randomness, 0.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Pan Random:"); ui.add(egui::Slider::new(&mut state.pan_randomness, 0.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Reverse %:"); ui.add(egui::Slider::new(&mut state.reverse_chance, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0))); });

        ui.separator();
        ui.label(egui::RichText::new("Time/Pitch Stretch").strong());
        ui.horizontal(|ui| { ui.label("Time Stretch:"); ui.add(egui::Slider::new(&mut state.time_stretch, 0.25..=4.0).logarithmic(true)); });
        ui.horizontal(|ui| { ui.label("Formant Shift:"); ui.add(egui::Slider::new(&mut state.formant_shift, -12.0..=12.0).suffix(" st")); });

        ui.separator();
        draw_grain_cloud_viz(ui, state);
    });
}

pub fn draw_grain_window(ui: &mut egui::Ui, window: &GrainWindowType) {
    let size = Vec2::new(120.0, 50.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, Color32::from_rgb(10, 12, 18));
    let n = 60usize;
    let pts: Vec<Pos2> = (0..n).map(|i| {
        let t = i as f32 / (n - 1) as f32;
        let env = window.envelope(t);
        Pos2::new(rect.left() + t * rect.width(), rect.bottom() - env * rect.height() * 0.9)
    }).collect();
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(80, 200, 255)));
    }
}

pub fn draw_grain_cloud_viz(ui: &mut egui::Ui, state: &GranularSynthState) {
    let size = Vec2::new(200.0, 80.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));

    // Position marker
    let px = rect.left() + state.position * rect.width();
    let spray_w = state.spray_ms / 500.0 * rect.width();
    p.rect_filled(
        Rect::from_min_max(Pos2::new((px - spray_w).max(rect.left()), rect.top()), Pos2::new((px + spray_w).min(rect.right()), rect.bottom())),
        0.0, Color32::from_rgba_premultiplied(80, 180, 255, 20),
    );
    p.line_segment([Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
        Stroke::new(1.5, Color32::from_rgb(80, 200, 255)));

    // Grain blobs
    let density_n = (state.density * 0.2) as usize;
    for i in 0..density_n.min(30) {
        let phase = i as f32 / density_n as f32;
        let gx = px + (phase * 6.28).sin() * spray_w * 0.6;
        let gy = rect.top() + (phase * 6.28 * 1.3).cos() * rect.height() * 0.3 + rect.height() * 0.5;
        let gw = state.grain_size_ms / 500.0 * rect.width() * 0.15;
        let alpha = (180.0 * state.amplitude) as u8;
        p.circle_filled(Pos2::new(gx, gy), gw.max(1.0),
            Color32::from_rgba_premultiplied(80, 180, 255, alpha / 2));
    }

    p.text(rect.left_top() + Vec2::new(2.0, 2.0), egui::Align2::LEFT_TOP,
        format!("{:.0}g/s  {:.0}ms", state.density, state.grain_size_ms),
        FontId::monospace(8.0), Color32::GRAY);
}

// --- Convolution Reverb IR Browser -----------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct ImpulseResponse {
    pub name: String,
    pub category: String,
    pub duration_ms: f32,
    pub sample_rate: u32,
    pub channels: u8,
    pub size_kb: u32,
    pub reverb_time_s: f32,
    pub pre_delay_ms: f32,
    pub early_reflection_level_db: f32,
    pub description: String,
    pub path: String,
    pub favorite: bool,
    pub waveform: Vec<f32>,
}

impl ImpulseResponse {
    pub fn new(name: &str, cat: &str, dur_ms: f32, reverb_s: f32, desc: &str) -> Self {
        let waveform = (0..64).map(|i| {
            let t = i as f32 / 64.0;
            let decay = (-(t * 6.0)).exp();
            let noise = ((i * 7919 + 12345) % 1000) as f32 / 1000.0 * 2.0 - 1.0;
            noise * decay
        }).collect();
        Self {
            name: name.to_string(),
            category: cat.to_string(),
            duration_ms: dur_ms,
            sample_rate: 44100,
            channels: 2,
            size_kb: (dur_ms * 44100.0 * 4.0 / 1024000.0) as u32,
            reverb_time_s: reverb_s,
            pre_delay_ms: 10.0,
            early_reflection_level_db: -6.0,
            description: desc.to_string(),
            path: format!("ir/{}/{}.wav", cat.to_lowercase().replace(' ', "_"), name.to_lowercase().replace(' ', "_")),
            favorite: false,
            waveform,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct IrBrowserState {
    pub irs: Vec<ImpulseResponse>,
    pub selected: Option<usize>,
    pub filter_category: String,
    pub search: String,
    pub show_waveform: bool,
}

impl IrBrowserState {
    pub fn with_demo_irs() -> Self {
        let mut s = Self { show_waveform: true, ..Default::default() };
        s.irs = vec![
            ImpulseResponse::new("Concert Hall Large", "Concert Hall", 3200.0, 2.8, "Large 2500-seat concert hall"),
            ImpulseResponse::new("Concert Hall Small", "Concert Hall", 2000.0, 1.9, "Intimate 400-seat recital hall"),
            ImpulseResponse::new("Cathedral Notre Dame", "Cathedral", 5500.0, 6.2, "Gothic cathedral with stone walls"),
            ImpulseResponse::new("Cathedral Modern", "Cathedral", 4200.0, 4.8, "Modern concrete cathedral"),
            ImpulseResponse::new("Plate EMT 140", "Plate", 1500.0, 2.1, "Classic EMT 140 gold foil plate"),
            ImpulseResponse::new("Plate Steel", "Plate", 1200.0, 1.6, "Steel plate, brighter character"),
            ImpulseResponse::new("Spring Fender", "Spring", 800.0, 1.1, "Fender tank spring reverb"),
            ImpulseResponse::new("Spring AKG BX20", "Spring", 1100.0, 1.4, "AKG BX20 stereo spring"),
            ImpulseResponse::new("Garage Small", "Room", 400.0, 0.4, "Small garage, early reflections"),
            ImpulseResponse::new("Studio Live Room", "Room", 600.0, 0.6, "Professional studio live room"),
            ImpulseResponse::new("Bathroom Tile", "Room", 700.0, 0.8, "Tiled bathroom, bright decay"),
            ImpulseResponse::new("Stairwell", "Room", 1400.0, 1.5, "9-floor concrete stairwell"),
            ImpulseResponse::new("Underground Cave", "Outdoor", 8000.0, 7.5, "Underground cave system"),
            ImpulseResponse::new("Forest Clearing", "Outdoor", 300.0, 0.3, "Open forest clearing"),
            ImpulseResponse::new("Opera House Vienna", "Concert Hall", 2800.0, 2.4, "Vienna State Opera house"),
            ImpulseResponse::new("Drum Room SSL", "Studio", 500.0, 0.5, "SSL studio drum room"),
            ImpulseResponse::new("Ambient Space", "Ambient", 12000.0, 15.0, "Huge space, infinite tail"),
            ImpulseResponse::new("Parking Garage", "Outdoor", 1800.0, 1.9, "Concrete parking garage"),
            ImpulseResponse::new("Guitar Cab Close", "Cabinet", 50.0, 0.02, "4x12 cabinet close-mic"),
            ImpulseResponse::new("Vintage Chamber", "Chamber", 2200.0, 2.3, "1960s echo chamber"),
            ImpulseResponse::new("Hangar Large", "Outdoor", 6000.0, 5.5, "Aircraft hangar"),
            ImpulseResponse::new("Sewers", "Unusual", 3500.0, 3.0, "Underground sewer system"),
        ];
        s
    }

    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self.irs.iter().map(|ir| ir.category.clone()).collect();
        cats.sort();
        cats.dedup();
        cats
    }
}

pub fn show_ir_browser(ui: &mut egui::Ui, state: &mut IrBrowserState) {
    ui.heading("Impulse Response Browser");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search);
        ui.separator();
        ui.checkbox(&mut state.show_waveform, "Waveform");
        if ui.button("Clear Filter").clicked() { state.filter_category.clear(); }
    });

    ui.horizontal_wrapped(|ui| {
        for cat in state.categories() {
            if ui.selectable_label(state.filter_category == cat, &cat).clicked() {
                state.filter_category = if state.filter_category == cat { String::new() } else { cat };
            }
        }
    });
    ui.separator();

    let search_lower = state.search.to_lowercase();
    egui::ScrollArea::vertical().max_height(250.0).id_source("ir_browser").show(ui, |ui| {
        for (i, ir) in state.irs.iter_mut().enumerate() {
            if !state.filter_category.is_empty() && ir.category != state.filter_category { continue; }
            if !search_lower.is_empty() && !ir.name.to_lowercase().contains(&search_lower) && !ir.category.to_lowercase().contains(&search_lower) { continue; }

            let sel = state.selected == Some(i);
            ui.horizontal(|ui| {
                if ui.selectable_label(sel, egui::RichText::new(&ir.name).color(if sel { Color32::from_rgb(100, 200, 255) } else { Color32::WHITE })).clicked() {
                    state.selected = Some(i);
                }
                ui.label(egui::RichText::new(&ir.category).small().color(Color32::from_rgb(160, 140, 255)));
                ui.label(egui::RichText::new(format!("{:.1}s", ir.reverb_time_s)).small().color(Color32::GRAY));
                ui.label(egui::RichText::new(format!("{} KB", ir.size_kb)).small().color(Color32::GRAY));
                if ui.small_button(if ir.favorite { "★" } else { "☆" }).clicked() { ir.favorite = !ir.favorite; }
                if state.show_waveform {
                    let sw = Vec2::new(60.0, 18.0);
                    let (wr, _) = ui.allocate_exact_size(sw, egui::Sense::hover());
                    let wp = ui.painter_at(wr);
                    wp.rect_filled(wr, 1.0, Color32::from_rgb(10, 12, 18));
                    let n = ir.waveform.len();
                    for (j, &amp) in ir.waveform.iter().enumerate() {
                        let x = wr.left() + j as f32 / n as f32 * wr.width();
                        let h = amp.abs() * wr.height() * 0.45;
                        wp.line_segment([Pos2::new(x, wr.center().y - h), Pos2::new(x, wr.center().y + h)],
                            Stroke::new(1.0, Color32::from_rgb(80, 160, 200)));
                    }
                }
            });
        }
    });

    if let Some(idx) = state.selected {
        if idx < state.irs.len() {
            let ir = &state.irs[idx];
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&ir.name).strong());
                ui.label(format!("T60={:.1}s  Pre={:.0}ms  {:.0}ms  {}Hz  {}ch  {}KB",
                    ir.reverb_time_s, ir.pre_delay_ms, ir.duration_ms, ir.sample_rate, ir.channels, ir.size_kb));
            });
            ui.label(egui::RichText::new(&ir.description).small().color(Color32::GRAY));
            ui.horizontal(|ui| {
                if ui.button("Load into Reverb").clicked() {}
                if ui.button("Preview").clicked() {}
            });
        }
    }
}

// --- Delay Designer --------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum DelayMode {
    Mono,
    Stereo,
    PingPong,
    Haas,
    MultiTap,
}

impl DelayMode {
    pub fn label(&self) -> &str {
        match self {
            DelayMode::Mono => "Mono",
            DelayMode::Stereo => "Stereo",
            DelayMode::PingPong => "Ping Pong",
            DelayMode::Haas => "Haas",
            DelayMode::MultiTap => "Multi-Tap",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DelayTap {
    pub time_ms: f32,
    pub level_db: f32,
    pub pan: f32,
    pub pitch_st: f32,
    pub filter_lp: f32,
    pub filter_hp: f32,
    pub enabled: bool,
}

impl DelayTap {
    pub fn new(time_ms: f32, level_db: f32) -> Self {
        Self { time_ms, level_db, pan: 0.0, pitch_st: 0.0, filter_lp: 20000.0, filter_hp: 20.0, enabled: true }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DelayDesigner {
    pub mode: DelayMode,
    pub time_ms_l: f32,
    pub time_ms_r: f32,
    pub feedback_l: f32,
    pub feedback_r: f32,
    pub cross_feedback: f32,
    pub sync_tempo: bool,
    pub tempo_div_l: u8,
    pub tempo_div_r: u8,
    pub bpm: f32,
    pub filter_lp: f32,
    pub filter_hp: f32,
    pub modulation_rate: f32,
    pub modulation_depth: f32,
    pub diffusion: f32,
    pub ducking_enabled: bool,
    pub ducking_threshold_db: f32,
    pub ducking_release_ms: f32,
    pub wet_db: f32,
    pub dry_db: f32,
    pub enabled: bool,
    pub taps: Vec<DelayTap>,
}

impl Default for DelayDesigner {
    fn default() -> Self {
        Self {
            mode: DelayMode::PingPong,
            time_ms_l: 375.0,
            time_ms_r: 500.0,
            feedback_l: 0.35,
            feedback_r: 0.35,
            cross_feedback: 0.15,
            sync_tempo: true,
            tempo_div_l: 4,
            tempo_div_r: 3,
            bpm: 120.0,
            filter_lp: 8000.0,
            filter_hp: 100.0,
            modulation_rate: 0.3,
            modulation_depth: 0.1,
            diffusion: 0.3,
            ducking_enabled: false,
            ducking_threshold_db: -20.0,
            ducking_release_ms: 200.0,
            wet_db: -12.0,
            dry_db: 0.0,
            enabled: true,
            taps: vec![
                DelayTap::new(250.0, 0.0),
                DelayTap::new(500.0, -6.0),
                DelayTap::new(750.0, -12.0),
                DelayTap::new(1000.0, -18.0),
            ],
        }
    }
}

pub fn show_delay_designer(ui: &mut egui::Ui, state: &mut DelayDesigner) {
    ui.heading("Delay Designer");
    ui.separator();

    ui.horizontal(|ui| {
        for m in &[DelayMode::Mono, DelayMode::Stereo, DelayMode::PingPong, DelayMode::Haas, DelayMode::MultiTap] {
            if ui.selectable_label(state.mode == *m, m.label()).clicked() { state.mode = m.clone(); }
        }
        ui.separator();
        ui.checkbox(&mut state.enabled, "Enable");
        ui.checkbox(&mut state.sync_tempo, "Sync Tempo");
    });
    ui.separator();

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new("Delay Times").strong());
        if state.sync_tempo {
            let divs = ["1/32", "1/16T", "1/16", "1/8T", "1/8", "1/4T", "1/4", "1/2T", "1/2", "1"];
            ui.horizontal(|ui| {
                ui.label("L Div:");
                egui::ComboBox::from_id_source("delay_div_l").selected_text(divs.get(state.tempo_div_l as usize).copied().unwrap_or("?"))
                    .show_ui(ui, |ui| {
                        for (i, d) in divs.iter().enumerate() {
                            ui.selectable_value(&mut state.tempo_div_l, i as u8, *d);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("R Div:");
                egui::ComboBox::from_id_source("delay_div_r").selected_text(divs.get(state.tempo_div_r as usize).copied().unwrap_or("?"))
                    .show_ui(ui, |ui| {
                        for (i, d) in divs.iter().enumerate() {
                            ui.selectable_value(&mut state.tempo_div_r, i as u8, *d);
                        }
                    });
            });
        } else {
            ui.horizontal(|ui| { ui.label("L:"); ui.add(egui::Slider::new(&mut state.time_ms_l, 1.0..=2000.0).suffix(" ms")); });
            ui.horizontal(|ui| { ui.label("R:"); ui.add(egui::Slider::new(&mut state.time_ms_r, 1.0..=2000.0).suffix(" ms")); });
        }

        ui.horizontal(|ui| { ui.label("Feedback L:"); ui.add(egui::Slider::new(&mut state.feedback_l, 0.0..=0.99)); });
        ui.horizontal(|ui| { ui.label("Feedback R:"); ui.add(egui::Slider::new(&mut state.feedback_r, 0.0..=0.99)); });
        ui.horizontal(|ui| { ui.label("Cross FB:"); ui.add(egui::Slider::new(&mut state.cross_feedback, 0.0..=0.99)); });

        ui.separator();
        ui.label(egui::RichText::new("Filter & Mod").strong());
        ui.horizontal(|ui| { ui.label("LP:"); ui.add(egui::Slider::new(&mut state.filter_lp, 200.0..=20000.0).suffix(" Hz").logarithmic(true)); });
        ui.horizontal(|ui| { ui.label("HP:"); ui.add(egui::Slider::new(&mut state.filter_hp, 20.0..=2000.0).suffix(" Hz").logarithmic(true)); });
        ui.horizontal(|ui| { ui.label("Mod Rate:"); ui.add(egui::Slider::new(&mut state.modulation_rate, 0.0..=10.0).suffix(" Hz")); });
        ui.horizontal(|ui| { ui.label("Mod Depth:"); ui.add(egui::Slider::new(&mut state.modulation_depth, 0.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Diffusion:"); ui.add(egui::Slider::new(&mut state.diffusion, 0.0..=1.0)); });

        let ui = &mut cols[1];
        ui.label(egui::RichText::new("Mix").strong());
        ui.horizontal(|ui| { ui.label("Wet:"); ui.add(egui::Slider::new(&mut state.wet_db, -60.0..=6.0).suffix(" dB")); });
        ui.horizontal(|ui| { ui.label("Dry:"); ui.add(egui::Slider::new(&mut state.dry_db, -60.0..=6.0).suffix(" dB")); });

        ui.separator();
        ui.checkbox(&mut state.ducking_enabled, "Ducking");
        if state.ducking_enabled {
            ui.horizontal(|ui| { ui.label("Threshold:"); ui.add(egui::Slider::new(&mut state.ducking_threshold_db, -60.0..=0.0).suffix(" dB")); });
            ui.horizontal(|ui| { ui.label("Release:"); ui.add(egui::Slider::new(&mut state.ducking_release_ms, 10.0..=1000.0).suffix(" ms")); });
        }

        ui.separator();
        draw_delay_tap_visualizer(ui, state);

        if state.mode == DelayMode::MultiTap {
            ui.separator();
            ui.label(egui::RichText::new("Taps").strong());
            if ui.button("+ Tap").clicked() {
                let last = state.taps.last().map(|t| t.time_ms + 250.0).unwrap_or(250.0);
                state.taps.push(DelayTap::new(last, -state.taps.len() as f32 * 6.0));
            }
            let mut remove = None;
            for (i, tap) in state.taps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut tap.time_ms).speed(1.0).clamp_range(1.0..=5000.0).suffix(" ms"));
                    ui.add(egui::DragValue::new(&mut tap.level_db).speed(0.5).clamp_range(-60.0..=6.0).suffix(" dB"));
                    ui.add(egui::DragValue::new(&mut tap.pan).speed(0.01).clamp_range(-1.0..=1.0).prefix("P:"));
                    ui.checkbox(&mut tap.enabled, "");
                    if ui.small_button("✗").clicked() { remove = Some(i); }
                });
            }
            if let Some(ri) = remove { state.taps.remove(ri); }
        }
    });
}

pub fn draw_delay_tap_visualizer(ui: &mut egui::Ui, state: &DelayDesigner) {
    let size = Vec2::new(200.0, 80.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let max_t = state.time_ms_r.max(state.time_ms_l).max(
        state.taps.iter().map(|t| t.time_ms).fold(0.0f32, f32::max)
    ) * 3.0;

    let t_to_x = |t: f32| -> f32 { rect.left() + (t / max_t) * rect.width() };

    // Tap lines
    let draw_tap = |p: &egui::Painter, t: f32, level_db: f32, col: Color32| {
        let x = t_to_x(t);
        let h = ((level_db + 60.0) / 60.0).clamp(0.0, 1.0) * rect.height() * 0.85;
        p.line_segment([Pos2::new(x, rect.bottom()), Pos2::new(x, rect.bottom() - h)], Stroke::new(2.0, col));
        p.circle_filled(Pos2::new(x, rect.bottom() - h), 3.0, col);
    };

    // Dry signal
    draw_tap(&p, 0.0, state.dry_db, Color32::from_rgb(200, 200, 200));

    // L/R channels
    if state.mode != DelayMode::MultiTap {
        let mut t_l = state.time_ms_l;
        let mut t_r = state.time_ms_r;
        let mut fb_l = 1.0f32;
        let mut fb_r = 1.0f32;
        for _ in 0..6 {
            draw_tap(&p, t_l, state.wet_db + linear_to_db(fb_l), Color32::from_rgb(80, 140, 255));
            draw_tap(&p, t_r, state.wet_db + linear_to_db(fb_r), Color32::from_rgb(255, 160, 60));
            fb_l *= state.feedback_l;
            fb_r *= state.feedback_r;
            t_l += state.time_ms_l;
            t_r += state.time_ms_r;
        }
    } else {
        for tap in &state.taps {
            if tap.enabled {
                draw_tap(&p, tap.time_ms, tap.level_db + state.wet_db, Color32::from_rgb(80, 220, 160));
            }
        }
    }
}

#[inline]
fn linear_to_db(linear: f32) -> f32 { 20.0 * linear.max(1e-10).log10() }
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
