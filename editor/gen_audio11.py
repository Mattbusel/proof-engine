path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 11
// ============================================================

// --- Voice Leading Analyzer ------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct VoiceLeadingEntry {
    pub from_chord: Vec<u8>,
    pub to_chord: Vec<u8>,
    pub from_label: String,
    pub to_label: String,
    pub voice_movements: Vec<i8>,
    pub parallel_fifths: bool,
    pub parallel_octaves: bool,
    pub contrary_motion: u32,
    pub similar_motion: u32,
    pub oblique_motion: u32,
    pub dissonance_score: f32,
}

impl VoiceLeadingEntry {
    pub fn analyze(from: Vec<u8>, to: Vec<u8>, from_label: &str, to_label: &str) -> Self {
        let movements: Vec<i8> = from.iter().zip(to.iter())
            .map(|(&f, &t)| t as i8 - f as i8).collect();
        let mut parallel_fifths = false;
        let mut parallel_octaves = false;
        let mut contrary = 0u32;
        let mut similar = 0u32;
        let mut oblique = 0u32;

        let n = movements.len();
        for i in 0..n {
            for j in (i+1)..n {
                let from_int = (from[j] as i32 - from[i] as i32).abs() % 12;
                let to_int = (to[j] as i32 - to[i] as i32).abs() % 12;
                if to_int == 7 && from_int == 7 { parallel_fifths = true; }
                if to_int == 0 && from_int == 0 { parallel_octaves = true; }
                let mi = movements.get(i).copied().unwrap_or(0);
                let mj = movements.get(j).copied().unwrap_or(0);
                if mi == 0 || mj == 0 { oblique += 1; }
                else if (mi > 0) != (mj > 0) { contrary += 1; }
                else { similar += 1; }
            }
        }
        let dissonance = movements.iter().map(|&m| m.unsigned_abs() as f32).sum::<f32>() / n as f32;
        Self {
            from_chord: from, to_chord: to,
            from_label: from_label.to_string(),
            to_label: to_label.to_string(),
            voice_movements: movements,
            parallel_fifths, parallel_octaves, contrary_motion: contrary,
            similar_motion: similar, oblique_motion: oblique,
            dissonance_score: dissonance,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VoiceLeadingState {
    pub history: Vec<VoiceLeadingEntry>,
    pub current_chord: Vec<u8>,
    pub voice_count: u8,
    pub show_staff: bool,
}

pub fn show_voice_leading_analyzer(ui: &mut egui::Ui, state: &mut VoiceLeadingState) {
    ui.heading("Voice Leading Analyzer");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Voices:");
        ui.add(egui::DragValue::new(&mut state.voice_count).clamp_range(2u8..=8));
        ui.checkbox(&mut state.show_staff, "Staff View");
    });

    ui.label("Current Chord Notes (MIDI):");
    ui.horizontal_wrapped(|ui| {
        let n = state.voice_count as usize;
        state.current_chord.resize(n, 60);
        for i in 0..n {
            ui.add(egui::DragValue::new(&mut state.current_chord[i]).clamp_range(21u8..=108));
            ui.label(format!("({})", NOTE_NAMES[state.current_chord[i] as usize % 12]));
        }
    });

    if ui.button("Analyze to Previous").clicked() && !state.history.is_empty() {
        let prev = state.history.last().unwrap().to_chord.clone();
        let entry = VoiceLeadingEntry::analyze(prev, state.current_chord.clone(), "Prev", "Current");
        state.history.push(entry);
    }
    if ui.button("Add to History").clicked() {
        let from = state.history.last().map(|e| e.to_chord.clone()).unwrap_or_else(|| state.current_chord.clone());
        let entry = VoiceLeadingEntry::analyze(from, state.current_chord.clone(), "Chord", "Next");
        state.history.push(entry);
    }
    ui.separator();

    egui::ScrollArea::vertical().max_height(200.0).id_source("vl_hist").show(ui, |ui| {
        for entry in &state.history {
            ui.horizontal(|ui| {
                let notes_from: String = entry.from_chord.iter().map(|&n| format!("{}", NOTE_NAMES[n as usize % 12])).collect::<Vec<_>>().join("-");
                let notes_to: String = entry.to_chord.iter().map(|&n| format!("{}", NOTE_NAMES[n as usize % 12])).collect::<Vec<_>>().join("-");
                ui.label(egui::RichText::new(format!("[{}] -> [{}]", notes_from, notes_to)).monospace().small());
                let mvt: String = entry.voice_movements.iter().map(|&m| format!("{:+}", m)).collect::<Vec<_>>().join(" ");
                ui.label(egui::RichText::new(mvt).small().color(Color32::from_rgb(100, 200, 255)));
                if entry.parallel_fifths { ui.colored_label(Color32::RED, "||5"); }
                if entry.parallel_octaves { ui.colored_label(Color32::RED, "||8"); }
                ui.label(format!("dis={:.1}", entry.dissonance_score));
            });
        }
    });
}

// --- Audio History / Undo --------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioUndoAction {
    pub description: String,
    pub timestamp: f32,
    pub panel: String,
    pub action_type: String,
    pub reversible: bool,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioUndoState {
    pub history: Vec<AudioUndoAction>,
    pub current: usize,
    pub max_history: usize,
}

impl AudioUndoState {
    pub fn new() -> Self { Self { max_history: 100, ..Default::default() } }

    pub fn push(&mut self, desc: &str, panel: &str, action: &str) {
        self.history.truncate(self.current);
        self.history.push(AudioUndoAction {
            description: desc.to_string(),
            timestamp: 0.0,
            panel: panel.to_string(),
            action_type: action.to_string(),
            reversible: true,
        });
        if self.history.len() > self.max_history { self.history.remove(0); }
        self.current = self.history.len();
    }

    pub fn can_undo(&self) -> bool { self.current > 0 }
    pub fn can_redo(&self) -> bool { self.current < self.history.len() }
    pub fn undo(&mut self) { if self.can_undo() { self.current -= 1; } }
    pub fn redo(&mut self) { if self.can_redo() { self.current += 1; } }
}

pub fn show_audio_undo_panel(ui: &mut egui::Ui, state: &mut AudioUndoState) {
    ui.heading("Audio History");
    ui.separator();
    ui.horizontal(|ui| {
        if ui.add_enabled(state.can_undo(), egui::Button::new("⬅ Undo")).clicked() { state.undo(); }
        if ui.add_enabled(state.can_redo(), egui::Button::new("Redo ➡")).clicked() { state.redo(); }
        if ui.button("Clear History").clicked() { state.history.clear(); state.current = 0; }
        ui.label(format!("{}/{}", state.current, state.history.len()));
    });
    ui.separator();
    egui::ScrollArea::vertical().max_height(200.0).id_source("undo_hist").show(ui, |ui| {
        for (i, action) in state.history.iter().enumerate().rev() {
            let is_current = i + 1 == state.current;
            let col = if i >= state.current { Color32::GRAY }
                      else if is_current { Color32::from_rgb(80, 220, 100) }
                      else { Color32::WHITE };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{:3}.", i + 1)).small().monospace().color(Color32::GRAY));
                ui.label(egui::RichText::new(&action.description).small().color(col));
                ui.label(egui::RichText::new(format!("[{}]", action.panel)).small().color(Color32::from_rgb(100, 140, 200)));
            });
        }
    });
}

// --- Sidechain Visualizer --------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct SidechainVisualizerState {
    pub source_bus: usize,
    pub target_bus: usize,
    pub filter_hp: f32,
    pub filter_lp: f32,
    pub filter_enabled: bool,
    pub send_level_db: f32,
    pub invert: bool,
    pub source_level: f32,
    pub target_gain_reduction: f32,
    pub source_name: String,
    pub target_name: String,
    pub history_src: Vec<f32>,
    pub history_gr: Vec<f32>,
}

impl Default for SidechainVisualizerState {
    fn default() -> Self {
        Self {
            source_bus: 1,
            target_bus: 2,
            filter_hp: 60.0,
            filter_lp: 20000.0,
            filter_enabled: true,
            send_level_db: 0.0,
            invert: false,
            source_level: -20.0,
            target_gain_reduction: 0.0,
            source_name: "Kick".into(),
            target_name: "Bass".into(),
            history_src: vec![-40.0; 60],
            history_gr: vec![0.0; 60],
        }
    }
}

pub fn show_sidechain_visualizer(ui: &mut egui::Ui, state: &mut SidechainVisualizerState) {
    ui.heading("Sidechain Visualizer");
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Source:");
        ui.text_edit_singleline(&mut state.source_name);
        ui.label("→");
        ui.label("Target:");
        ui.text_edit_singleline(&mut state.target_name);
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.filter_enabled, "Filter");
        if state.filter_enabled {
            ui.label("HP:");
            ui.add(egui::DragValue::new(&mut state.filter_hp).speed(5.0).clamp_range(20.0..=2000.0).suffix(" Hz"));
            ui.label("LP:");
            ui.add(egui::DragValue::new(&mut state.filter_lp).speed(100.0).clamp_range(200.0..=20000.0).suffix(" Hz"));
        }
        ui.label("Send:");
        ui.add(egui::DragValue::new(&mut state.send_level_db).speed(0.5).clamp_range(-60.0..=12.0).suffix(" dB"));
        ui.checkbox(&mut state.invert, "Invert");
    });
    ui.separator();

    let size = Vec2::new(ui.available_width().min(400.0), 100.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let h2 = rect.height() * 0.5;
    let top_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), h2 - 1.0));
    let bot_rect = Rect::from_min_size(Pos2::new(rect.left(), rect.top() + h2 + 1.0), Vec2::new(rect.width(), h2 - 1.0));

    p.text(Pos2::new(top_rect.left() + 2.0, top_rect.top() + 2.0), egui::Align2::LEFT_TOP,
        format!("SC: {}", state.source_name), FontId::monospace(8.0), Color32::GRAY);
    p.text(Pos2::new(bot_rect.left() + 2.0, bot_rect.top() + 2.0), egui::Align2::LEFT_TOP,
        format!("GR: {}", state.target_name), FontId::monospace(8.0), Color32::GRAY);

    let n = state.history_src.len();
    // Source envelope
    let src_pts: Vec<Pos2> = state.history_src.iter().enumerate().map(|(i, &db)| {
        let t = i as f32 / n as f32;
        let v = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        Pos2::new(top_rect.left() + t * top_rect.width(), top_rect.bottom() - v * top_rect.height() * 0.9)
    }).collect();
    for w in src_pts.windows(2) { p.line_segment([w[0], w[1]], Stroke::new(1.0, Color32::from_rgb(255, 160, 60))); }

    // GR
    let gr_pts: Vec<Pos2> = state.history_gr.iter().enumerate().map(|(i, &gr)| {
        let t = i as f32 / n as f32;
        let v = (gr.abs() / 30.0).clamp(0.0, 1.0);
        Pos2::new(bot_rect.left() + t * bot_rect.width(), bot_rect.bottom() - v * bot_rect.height() * 0.9)
    }).collect();
    for w in gr_pts.windows(2) { p.line_segment([w[0], w[1]], Stroke::new(1.0, Color32::from_rgb(80, 200, 120))); }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("Source: {:.1} dB", state.source_level));
        ui.label(format!("GR: {:.1} dB", state.target_gain_reduction));
    });
}

// --- Audio Session Manager -------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioSession {
    pub id: usize,
    pub name: String,
    pub created: f32,
    pub last_modified: f32,
    pub bpm: f32,
    pub sample_rate: u32,
    pub bus_count: u8,
    pub track_count: u8,
    pub duration_s: f32,
    pub notes: String,
    pub tags: Vec<String>,
    pub file_path: String,
    pub modified: bool,
}

impl AudioSession {
    pub fn new(id: usize, name: &str) -> Self {
        Self {
            id, name: name.to_string(), created: 0.0, last_modified: 0.0,
            bpm: 120.0, sample_rate: 44100, bus_count: 8, track_count: 4,
            duration_s: 180.0, notes: String::new(), tags: vec![],
            file_path: format!("sessions/{}.amx", name.to_lowercase().replace(' ', "_")),
            modified: false,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AudioSessionManager {
    pub sessions: Vec<AudioSession>,
    pub active: Option<usize>,
    pub search: String,
}

impl AudioSessionManager {
    pub fn with_demo() -> Self {
        let mut s = Self::default();
        s.sessions = vec![
            AudioSession::new(0, "Main Theme"),
            AudioSession::new(1, "Battle Music"),
            AudioSession::new(2, "Ambient Forest"),
            AudioSession::new(3, "Victory Fanfare"),
            AudioSession::new(4, "SFX Library"),
        ];
        s.active = Some(0);
        s
    }
}

pub fn show_audio_session_manager(ui: &mut egui::Ui, state: &mut AudioSessionManager) {
    ui.heading("Session Manager");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("New Session").clicked() {
            let id = state.sessions.len();
            state.sessions.push(AudioSession::new(id, "Untitled"));
            state.active = Some(id);
        }
        if ui.button("Save All").clicked() {}
        ui.separator();
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search);
    });
    ui.separator();

    let search_lower = state.search.to_lowercase();
    let mut remove = None;
    for (i, sess) in state.sessions.iter().enumerate() {
        if !search_lower.is_empty() && !sess.name.to_lowercase().contains(&search_lower) { continue; }
        let is_active = state.active == Some(i);
        let col = if is_active { Color32::from_rgb(80, 220, 100) } else { Color32::WHITE };
        ui.horizontal(|ui| {
            if ui.selectable_label(is_active, egui::RichText::new(&sess.name).color(col)).clicked() {
                state.active = Some(i);
            }
            ui.label(egui::RichText::new(format!("{:.0}bpm  {}tr  {:.0}s", sess.bpm, sess.track_count, sess.duration_s)).small().color(Color32::GRAY));
            if sess.modified { ui.colored_label(Color32::YELLOW, "*"); }
            if ui.small_button("💾").clicked() {}
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.sessions.remove(ri); if state.active == Some(ri) { state.active = None; } }

    if let Some(idx) = state.active {
        if idx < state.sessions.len() {
            let sess = &mut state.sessions[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut sess.name); });
            ui.horizontal(|ui| {
                ui.label("BPM:"); ui.add(egui::DragValue::new(&mut sess.bpm).speed(1.0).clamp_range(20.0..=300.0));
                ui.label("SR:"); ui.add(egui::DragValue::new(&mut sess.sample_rate).clamp_range(8000u32..=192000).suffix(" Hz"));
            });
            ui.horizontal(|ui| { ui.label("Notes:"); ui.text_edit_multiline(&mut sess.notes); });
            ui.label(egui::RichText::new(format!("Path: {}", sess.file_path)).small().color(Color32::GRAY));
        }
    }
}

// --- Audio Preferences -----------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioPreferences {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub bit_depth: u8,
    pub driver_api: String,
    pub output_device: String,
    pub input_device: String,
    pub enable_asio: bool,
    pub exclusive_mode: bool,
    pub low_latency: bool,
    pub auto_connect: bool,
    pub normalize_on_import: bool,
    pub default_sample_rate: u32,
    pub default_bit_depth: u8,
    pub import_folder: String,
    pub export_folder: String,
    pub plugin_scan_paths: Vec<String>,
    pub max_polyphony: u32,
    pub metering_hold_ms: u32,
    pub spectral_fft_size: u32,
    pub spectral_window: String,
    pub theme: String,
    pub show_tips: bool,
    pub auto_save_interval_s: u32,
    pub undo_levels: u32,
    pub log_midi: bool,
    pub log_audio: bool,
}

impl Default for AudioPreferences {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            buffer_size: 512,
            bit_depth: 32,
            driver_api: "WASAPI".into(),
            output_device: "Default Output".into(),
            input_device: "Default Input".into(),
            enable_asio: false,
            exclusive_mode: false,
            low_latency: false,
            auto_connect: true,
            normalize_on_import: false,
            default_sample_rate: 44100,
            default_bit_depth: 24,
            import_folder: "./imports".into(),
            export_folder: "./exports".into(),
            plugin_scan_paths: vec!["C:/Program Files/VSTPlugins".into(), "C:/Program Files/Common Files/VST3".into()],
            max_polyphony: 256,
            metering_hold_ms: 2000,
            spectral_fft_size: 2048,
            spectral_window: "Hann".into(),
            theme: "Dark".into(),
            show_tips: true,
            auto_save_interval_s: 300,
            undo_levels: 100,
            log_midi: false,
            log_audio: false,
        }
    }
}

pub fn show_audio_preferences(ui: &mut egui::Ui, state: &mut AudioPreferences) {
    ui.heading("Audio Preferences");
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.label(egui::RichText::new("Audio Device").strong());
        ui.horizontal(|ui| {
            ui.label("Driver:");
            for api in &["WASAPI", "ASIO", "DirectSound", "CoreAudio", "ALSA", "JACK"] {
                if ui.selectable_label(state.driver_api == *api, *api).clicked() { state.driver_api = api.to_string(); }
            }
        });
        ui.horizontal(|ui| { ui.label("Output:"); ui.text_edit_singleline(&mut state.output_device); });
        ui.horizontal(|ui| { ui.label("Input:"); ui.text_edit_singleline(&mut state.input_device); });
        ui.horizontal(|ui| {
            ui.label("Sample Rate:");
            for sr in &[22050u32, 44100, 48000, 88200, 96000, 176400, 192000] {
                if ui.selectable_label(state.sample_rate == *sr, format!("{}k", sr / 1000)).clicked() { state.sample_rate = *sr; }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Buffer Size:");
            for bs in &[64u32, 128, 256, 512, 1024, 2048, 4096] {
                if ui.selectable_label(state.buffer_size == *bs, format!("{}", bs)).clicked() { state.buffer_size = *bs; }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Bit Depth:");
            for bd in &[16u8, 24, 32] {
                if ui.selectable_label(state.bit_depth == *bd, format!("{}", bd)).clicked() { state.bit_depth = *bd; }
            }
        });
        ui.horizontal(|ui| {
            let latency_ms = state.buffer_size as f32 / state.sample_rate as f32 * 1000.0;
            ui.label(format!("Latency: {:.1} ms", latency_ms));
        });
        ui.checkbox(&mut state.enable_asio, "Enable ASIO");
        ui.checkbox(&mut state.exclusive_mode, "Exclusive Mode");
        ui.checkbox(&mut state.low_latency, "Low Latency Priority");
        ui.separator();

        ui.label(egui::RichText::new("Defaults").strong());
        ui.horizontal(|ui| {
            ui.label("Default SR:");
            ui.add(egui::DragValue::new(&mut state.default_sample_rate).clamp_range(8000u32..=192000).suffix(" Hz"));
            ui.label("Default Bit:");
            ui.add(egui::DragValue::new(&mut state.default_bit_depth).clamp_range(16u8..=32));
        });
        ui.checkbox(&mut state.normalize_on_import, "Normalize on Import");
        ui.checkbox(&mut state.auto_connect, "Auto-connect Devices");
        ui.horizontal(|ui| { ui.label("Import:"); ui.text_edit_singleline(&mut state.import_folder); });
        ui.horizontal(|ui| { ui.label("Export:"); ui.text_edit_singleline(&mut state.export_folder); });
        ui.separator();

        ui.label(egui::RichText::new("Plugin Paths").strong());
        let mut remove_path = None;
        for (i, path) in state.plugin_scan_paths.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(path);
                if ui.small_button("✗").clicked() { remove_path = Some(i); }
            });
        }
        if let Some(ri) = remove_path { state.plugin_scan_paths.remove(ri); }
        if ui.button("+ Add Path").clicked() { state.plugin_scan_paths.push(String::new()); }
        ui.separator();

        ui.label(egui::RichText::new("Performance").strong());
        ui.horizontal(|ui| { ui.label("Max Polyphony:"); ui.add(egui::DragValue::new(&mut state.max_polyphony).clamp_range(16u32..=1024)); });
        ui.horizontal(|ui| { ui.label("FFT Size:"); for sz in &[512u32, 1024, 2048, 4096, 8192] {
            if ui.selectable_label(state.spectral_fft_size == *sz, format!("{}", sz)).clicked() { state.spectral_fft_size = *sz; }
        }});
        ui.horizontal(|ui| { ui.label("Meter Hold:"); ui.add(egui::DragValue::new(&mut state.metering_hold_ms).clamp_range(0u32..=10000).suffix(" ms")); });
        ui.separator();

        ui.label(egui::RichText::new("UI & Workflow").strong());
        ui.horizontal(|ui| {
            ui.label("Theme:");
            for theme in &["Dark", "Light", "High Contrast", "Solarized"] {
                if ui.selectable_label(state.theme == *theme, *theme).clicked() { state.theme = theme.to_string(); }
            }
        });
        ui.checkbox(&mut state.show_tips, "Show Tips");
        ui.horizontal(|ui| {
            ui.label("Auto-save:");
            ui.add(egui::DragValue::new(&mut state.auto_save_interval_s).clamp_range(0u32..=3600).suffix(" s"));
        });
        ui.horizontal(|ui| { ui.label("Undo Levels:"); ui.add(egui::DragValue::new(&mut state.undo_levels).clamp_range(10u32..=500)); });
        ui.checkbox(&mut state.log_midi, "Log MIDI");
        ui.checkbox(&mut state.log_audio, "Log Audio");
    });
}

// --- Audio Debug Overlay ---------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioDebugOverlay {
    pub enabled: bool,
    pub show_voice_count: bool,
    pub show_cpu: bool,
    pub show_latency: bool,
    pub show_xruns: bool,
    pub show_bus_levels: bool,
    pub show_midi_activity: bool,
    pub xrun_count: u32,
    pub cpu_usage: f32,
    pub current_latency_ms: f32,
    pub voice_count: u32,
    pub bus_levels: Vec<f32>,
    pub midi_notes_active: Vec<u8>,
}

impl Default for AudioDebugOverlay {
    fn default() -> Self {
        Self {
            enabled: true,
            show_voice_count: true,
            show_cpu: true,
            show_latency: true,
            show_xruns: true,
            show_bus_levels: true,
            show_midi_activity: true,
            xrun_count: 0,
            cpu_usage: 12.3,
            current_latency_ms: 11.6,
            voice_count: 14,
            bus_levels: vec![-20.0, -12.0, -18.0, -25.0, -30.0, -40.0],
            midi_notes_active: vec![60, 64, 67],
        }
    }
}

pub fn show_audio_debug_overlay(ui: &mut egui::Ui, state: &mut AudioDebugOverlay) {
    ui.heading("Audio Debug Overlay");
    ui.separator();
    ui.checkbox(&mut state.enabled, "Enable");
    if !state.enabled { return; }
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_voice_count, "Voices");
        ui.checkbox(&mut state.show_cpu, "CPU");
        ui.checkbox(&mut state.show_latency, "Latency");
        ui.checkbox(&mut state.show_xruns, "Xruns");
        ui.checkbox(&mut state.show_bus_levels, "Bus Levels");
        ui.checkbox(&mut state.show_midi_activity, "MIDI");
    });
    ui.separator();

    let size = Vec2::new(ui.available_width().min(360.0), 100.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgba_premultiplied(0, 0, 0, 200));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 200, 100, 200)));

    let mut y = rect.top() + 5.0;
    let line_h = 14.0f32;
    let lx = rect.left() + 5.0;

    if state.show_voice_count {
        p.text(Pos2::new(lx, y), egui::Align2::LEFT_TOP,
            format!("Voices: {}/{}", state.voice_count, AUDIO_DEFAULT_POLYPHONY), FontId::monospace(10.0), Color32::from_rgb(80, 220, 100));
        y += line_h;
    }
    if state.show_cpu {
        let col = if state.cpu_usage > 80.0 { Color32::RED } else if state.cpu_usage > 50.0 { Color32::YELLOW } else { Color32::from_rgb(80, 220, 100) };
        p.text(Pos2::new(lx, y), egui::Align2::LEFT_TOP, format!("CPU: {:.1}%", state.cpu_usage), FontId::monospace(10.0), col);
        y += line_h;
    }
    if state.show_latency {
        p.text(Pos2::new(lx, y), egui::Align2::LEFT_TOP, format!("Latency: {:.1} ms", state.current_latency_ms), FontId::monospace(10.0), Color32::from_rgb(100, 180, 255));
        y += line_h;
    }
    if state.show_xruns {
        let col = if state.xrun_count > 0 { Color32::RED } else { Color32::from_rgb(80, 220, 100) };
        p.text(Pos2::new(lx, y), egui::Align2::LEFT_TOP, format!("Xruns: {}", state.xrun_count), FontId::monospace(10.0), col);
        y += line_h;
    }
    if state.show_midi_activity && !state.midi_notes_active.is_empty() {
        let notes: String = state.midi_notes_active.iter().map(|&n| format!("{}", NOTE_NAMES[n as usize % 12])).collect::<Vec<_>>().join(" ");
        p.text(Pos2::new(lx, y), egui::Align2::LEFT_TOP, format!("MIDI: {}", notes), FontId::monospace(10.0), Color32::from_rgb(255, 160, 60));
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
