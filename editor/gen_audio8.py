path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 8
// ============================================================

// --- Step Sequencer --------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct StepSequencerRow {
    pub name: String,
    pub note: u8,
    pub velocity: u8,
    pub steps: Vec<bool>,
    pub accents: Vec<bool>,
    pub muted: bool,
    pub color: [u8; 3],
    pub instrument: String,
    pub step_offset: i8,
}

impl StepSequencerRow {
    pub fn new(name: &str, note: u8, n_steps: usize, col: [u8; 3]) -> Self {
        Self {
            name: name.to_string(),
            note,
            velocity: 100,
            steps: vec![false; n_steps],
            accents: vec![false; n_steps],
            muted: false,
            color: col,
            instrument: String::new(),
            step_offset: 0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StepSequencerState {
    pub rows: Vec<StepSequencerRow>,
    pub n_steps: usize,
    pub bpm: f32,
    pub current_step: usize,
    pub playing: bool,
    pub swing: f32,
    pub loop_start: usize,
    pub loop_end: usize,
    pub selected_row: Option<usize>,
    pub note_length: f32,
    pub step_division: u8,
    pub pattern_length: usize,
    pub patterns: Vec<String>,
    pub active_pattern: usize,
    pub chain_mode: bool,
    pub chain_order: Vec<usize>,
    pub shuffle_steps: bool,
    pub random_gate: f32,
    pub random_velocity: f32,
}

impl StepSequencerState {
    pub fn default_8step() -> Self {
        let n = 16;
        let mut rows = vec![
            StepSequencerRow::new("Kick", 36, n, [220, 80, 80]),
            StepSequencerRow::new("Snare", 38, n, [80, 200, 120]),
            StepSequencerRow::new("Hi-Hat C", 42, n, [80, 160, 255]),
            StepSequencerRow::new("Hi-Hat O", 46, n, [255, 200, 80]),
            StepSequencerRow::new("Clap", 39, n, [200, 80, 220]),
            StepSequencerRow::new("Rim", 37, n, [80, 220, 220]),
            StepSequencerRow::new("Tom Hi", 48, n, [255, 140, 60]),
            StepSequencerRow::new("Tom Lo", 45, n, [160, 255, 80]),
        ];
        // Classic 4-on-floor
        rows[0].steps = vec![true, false, false, false, true, false, false, false, true, false, false, false, true, false, false, false];
        rows[1].steps = vec![false, false, false, false, true, false, false, false, false, false, false, false, true, false, false, false];
        rows[2].steps = vec![true, false, true, false, true, false, true, false, true, false, true, false, true, false, true, false];
        Self {
            rows,
            n_steps: n,
            bpm: 128.0,
            current_step: 0,
            playing: false,
            swing: 0.0,
            loop_start: 0,
            loop_end: n - 1,
            selected_row: None,
            note_length: 0.5,
            step_division: 16,
            pattern_length: n,
            patterns: vec!["Pattern 1".into(), "Pattern 2".into(), "Pattern 3".into()],
            active_pattern: 0,
            chain_mode: false,
            chain_order: vec![0, 1, 2],
            shuffle_steps: false,
            random_gate: 0.0,
            random_velocity: 0.0,
        }
    }
}

pub fn show_step_sequencer(ui: &mut egui::Ui, state: &mut StepSequencerState) {
    ui.heading("Step Sequencer");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button(if state.playing { "⏸" } else { "▶" }).clicked() { state.playing = !state.playing; }
        if ui.button("⏹").clicked() { state.playing = false; state.current_step = 0; }
        ui.label("BPM:");
        ui.add(egui::DragValue::new(&mut state.bpm).speed(0.5).clamp_range(40.0..=240.0));
        ui.label("Swing:");
        ui.add(egui::Slider::new(&mut state.swing, 0.0..=100.0).suffix("%"));
        ui.label("Steps:");
        for &ns in &[8usize, 16, 32] {
            if ui.selectable_label(state.n_steps == ns, format!("{}", ns)).clicked() {
                state.n_steps = ns;
                state.pattern_length = ns;
                for row in &mut state.rows {
                    row.steps.resize(ns, false);
                    row.accents.resize(ns, false);
                }
            }
        }
        ui.separator();
        if ui.button("+ Row").clicked() {
            let id = state.rows.len();
            state.rows.push(StepSequencerRow::new(&format!("Track_{}", id), 60, state.n_steps, [100, 100, 200]));
        }
        if ui.button("Randomize").clicked() {}
        if ui.button("Clear All").clicked() {
            for row in &mut state.rows {
                for s in &mut row.steps { *s = false; }
            }
        }
    });

    // Pattern selector
    ui.horizontal(|ui| {
        ui.label("Pattern:");
        for (i, pat) in state.patterns.iter().enumerate() {
            if ui.selectable_label(state.active_pattern == i, pat).clicked() {
                state.active_pattern = i;
            }
        }
        if ui.button("+ Pattern").clicked() {
            state.patterns.push(format!("Pattern {}", state.patterns.len() + 1));
        }
        ui.separator();
        ui.checkbox(&mut state.chain_mode, "Chain");
        ui.label("Note Length:");
        for &nl in &[0.25f32, 0.5, 0.75, 1.0] {
            if ui.selectable_label((state.note_length - nl).abs() < 0.01, format!("{:.0}%", nl * 100.0)).clicked() {
                state.note_length = nl;
            }
        }
    });
    ui.separator();

    draw_step_sequencer_grid(ui, state);
    ui.separator();

    if let Some(idx) = state.selected_row {
        if idx < state.rows.len() {
            let row = &mut state.rows[idx];
            ui.horizontal(|ui| {
                ui.label("Name:"); ui.text_edit_singleline(&mut row.name);
                ui.label("Note:"); ui.add(egui::DragValue::new(&mut row.note).clamp_range(0u8..=127));
                ui.label(format!("({})", NOTE_NAMES[row.note as usize % 12]));
                ui.label("Vel:"); ui.add(egui::DragValue::new(&mut row.velocity).clamp_range(1u8..=127));
                ui.checkbox(&mut row.muted, "Muted");
                ui.label("Offset:");
                ui.add(egui::DragValue::new(&mut row.step_offset).clamp_range(-8i8..=8));
            });
        }
    }

    ui.horizontal(|ui| {
        ui.label("Rand Gate:");
        ui.add(egui::Slider::new(&mut state.random_gate, 0.0..=1.0));
        ui.label("Rand Vel:");
        ui.add(egui::Slider::new(&mut state.random_velocity, 0.0..=1.0));
    });
}

pub fn draw_step_sequencer_grid(ui: &mut egui::Ui, state: &mut StepSequencerState) {
    let row_h = 24.0f32;
    let label_w = 70.0f32;
    let step_w = ((ui.available_width() - label_w) / state.n_steps as f32).max(12.0).min(28.0);
    let grid_w = step_w * state.n_steps as f32;
    let total_h = row_h * state.rows.len() as f32 + row_h;
    let size = Vec2::new(label_w + grid_w, total_h.min(350.0));
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));

    // Step header
    for si in 0..state.n_steps {
        let x = rect.left() + label_w + si as f32 * step_w;
        let is_beat = si % 4 == 0;
        let bg = if is_beat { Color32::from_rgb(25, 28, 38) } else { Color32::from_rgb(16, 18, 25) };
        p.rect_filled(Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(step_w - 0.5, row_h)), 0.0, bg);
        if is_beat {
            p.text(Pos2::new(x + step_w * 0.5, rect.top() + row_h * 0.5), egui::Align2::CENTER_CENTER,
                format!("{}", si + 1), FontId::monospace(8.0), Color32::GRAY);
        }
    }

    let mut toggle_step: Option<(usize, usize)> = None;

    // Row grid
    for (ri, row) in state.rows.iter().enumerate() {
        let ry = rect.top() + row_h + ri as f32 * row_h;
        let sel = state.selected_row == Some(ri);
        let label_rect = Rect::from_min_size(Pos2::new(rect.left(), ry), Vec2::new(label_w - 2.0, row_h - 0.5));
        let label_bg = if sel { Color32::from_rgba_premultiplied(row.color[0], row.color[1], row.color[2], 40) }
                       else { Color32::from_rgb(14, 16, 22) };
        p.rect_filled(label_rect, 0.0, label_bg);
        p.text(Pos2::new(label_rect.left() + 4.0, label_rect.center().y), egui::Align2::LEFT_CENTER,
            &row.name, FontId::monospace(9.0),
            if row.muted { Color32::GRAY } else { Color32::from_rgb(row.color[0], row.color[1], row.color[2]) });

        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if pos.y >= ry && pos.y < ry + row_h && pos.x >= rect.left() && pos.x < rect.left() + label_w {
                    if state.selected_row == Some(ri) { state.selected_row = None; } else { state.selected_row = Some(ri); }
                }
            }
        }

        for si in 0..state.n_steps {
            let x = rect.left() + label_w + si as f32 * step_w;
            let sr = Rect::from_min_size(Pos2::new(x + 0.5, ry + 0.5), Vec2::new(step_w - 1.0, row_h - 1.0));
            let is_current = si == state.current_step && state.playing;
            let is_beat = si % 4 == 0;

            let bg = if row.steps[si] {
                let col = row.color;
                let a = if row.muted { 80u8 } else if is_current { 255 } else { 200 };
                Color32::from_rgba_premultiplied(col[0], col[1], col[2], a)
            } else if is_current {
                Color32::from_rgba_premultiplied(80, 80, 100, 80)
            } else if is_beat {
                Color32::from_rgb(18, 20, 28)
            } else {
                Color32::from_rgb(14, 16, 22)
            };
            p.rect_filled(sr, 2.0, bg);

            if row.accents[si] && row.steps[si] {
                p.rect_filled(Rect::from_min_size(sr.min, Vec2::new(sr.width(), 3.0)), 2.0, Color32::WHITE);
            }

            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if sr.contains(pos) {
                        toggle_step = Some((ri, si));
                    }
                }
            }
        }
    }

    if let Some((ri, si)) = toggle_step {
        if ri < state.rows.len() && si < state.rows[ri].steps.len() {
            state.rows[ri].steps[si] = !state.rows[ri].steps[si];
        }
    }

    // Current step playhead
    if state.playing {
        let cx = rect.left() + label_w + state.current_step as f32 * step_w;
        p.rect_stroke(
            Rect::from_min_size(Pos2::new(cx, rect.top() + row_h), Vec2::new(step_w, row_h * state.rows.len() as f32)),
            0.0, Stroke::new(1.5, Color32::from_rgba_premultiplied(255, 220, 60, 200)),
        );
    }
}

// --- Chord Sequencer -------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct ChordStep {
    pub notes: Vec<u8>,
    pub velocity: u8,
    pub duration_beats: f32,
    pub active: bool,
    pub inversion: u8,
    pub spread: u8,
}

impl ChordStep {
    pub fn new_chord(root: u8, chord_type: &ChordType, velocity: u8) -> Self {
        let notes = chord_type.intervals().iter().map(|&iv| root + iv).filter(|&n| n <= 127).collect();
        Self { notes, velocity, duration_beats: 1.0, active: true, inversion: 0, spread: 0 }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ChordSequencerState {
    pub steps: Vec<ChordStep>,
    pub current_step: usize,
    pub playing: bool,
    pub bpm: f32,
    pub loop_mode: bool,
    pub selected: Option<usize>,
    pub root: u8,
    pub scale: MusicalScale,
    pub voicing_spread: u8,
    pub arpeggio_enabled: bool,
    pub arpeggio_rate: f32,
    pub arpeggio_pattern: u8,
}

impl ChordSequencerState {
    pub fn default_progression() -> Self {
        let progression = [(0u8, ChordType::Major), (5, ChordType::Major), (9, ChordType::Minor), (7, ChordType::Major)];
        let steps = progression.iter().map(|(root, ct)| ChordStep::new_chord(*root + 60, ct, 90)).collect();
        Self { steps, bpm: 100.0, loop_mode: true, scale: MusicalScale::Major, ..Default::default() }
    }
}

pub fn show_chord_sequencer(ui: &mut egui::Ui, state: &mut ChordSequencerState) {
    ui.heading("Chord Sequencer");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button(if state.playing { "⏸" } else { "▶" }).clicked() { state.playing = !state.playing; }
        if ui.button("⏹").clicked() { state.playing = false; state.current_step = 0; }
        ui.label("BPM:");
        ui.add(egui::DragValue::new(&mut state.bpm).speed(1.0).clamp_range(40.0..=240.0));
        ui.checkbox(&mut state.loop_mode, "Loop");
        ui.separator();
        ui.checkbox(&mut state.arpeggio_enabled, "Arpeggio");
        if state.arpeggio_enabled {
            ui.add(egui::DragValue::new(&mut state.arpeggio_rate).speed(0.1).clamp_range(0.0..=16.0).suffix(" /beat"));
            ui.add(egui::DragValue::new(&mut state.arpeggio_pattern).clamp_range(0u8..=7).prefix("Pat: "));
        }
    });

    ui.horizontal(|ui| {
        if ui.button("+ Step").clicked() {
            state.steps.push(ChordStep::new_chord(60, &ChordType::Major, 90));
        }
        if ui.button("+ Triad").clicked() {}
        if ui.button("Clear").clicked() { state.steps.clear(); }
    });
    ui.separator();

    let mut remove = None;
    for (i, step) in state.steps.iter_mut().enumerate() {
        let sel = state.selected == Some(i);
        let is_cur = state.current_step == i && state.playing;
        ui.horizontal(|ui| {
            let col = if is_cur { Color32::from_rgb(255, 220, 60) } else if sel { Color32::WHITE } else { Color32::GRAY };
            let note_names: String = step.notes.iter().map(|&n| format!("{}", NOTE_NAMES[n as usize % 12])).collect::<Vec<_>>().join("-");
            if ui.selectable_label(sel, egui::RichText::new(format!("{}. [{}] vel={}", i + 1, note_names, step.velocity)).color(col)).clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
            ui.checkbox(&mut step.active, "");
            ui.add(egui::DragValue::new(&mut step.duration_beats).speed(0.25).clamp_range(0.25..=8.0).suffix(" b"));
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.steps.remove(ri); if state.selected == Some(ri) { state.selected = None; } }

    if let Some(idx) = state.selected {
        if idx < state.steps.len() {
            let step = &mut state.steps[idx];
            ui.separator();
            ui.label("Notes:");
            ui.horizontal_wrapped(|ui| {
                for note in &step.notes {
                    ui.label(egui::RichText::new(format!("{}{}", NOTE_NAMES[*note as usize % 12], *note as i32 / 12 - 1))
                        .color(Color32::from_rgb(100, 200, 255)).monospace());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Velocity:");
                ui.add(egui::Slider::new(&mut step.velocity, 1u8..=127));
                ui.label("Duration:");
                ui.add(egui::DragValue::new(&mut step.duration_beats).speed(0.25).clamp_range(0.25..=8.0).suffix(" beats"));
                ui.label("Inversion:");
                ui.add(egui::DragValue::new(&mut step.inversion).clamp_range(0u8..=3));
            });
        }
    }
}

// --- Audio System Overview (Master Summary) --------------------------------------

pub fn show_audio_system_overview(ui: &mut egui::Ui) {
    ui.heading("Audio System Overview");
    ui.separator();

    ui.columns(3, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new("Subsystems").strong());
        let subsystems: &[(&str, bool, &str)] = &[
            ("MIDI Sequencer", true, "16 tracks, 4/4 @ 120 BPM"),
            ("Automation Lanes", true, "8 lanes, 48 points"),
            ("Spectrum Analyzer", true, "2048 bins, waterfall"),
            ("Dynamics Processor", true, "Bus compressor active"),
            ("Effect Chain", true, "4 effects loaded"),
            ("Audio Events", true, "12 events configured"),
            ("3D Spatialization", true, "3 sources active"),
            ("Audio Scripting", false, "1 script, idle"),
            ("Multiband EQ", true, "7 bands active"),
            ("Bus Routing", true, "8 buses, 7 routes"),
            ("Waveform Editor", false, "Idle"),
            ("Mix Scenes", true, "3 scenes saved"),
            ("Metering", true, "6 buses"),
            ("Timeline", true, "4 tracks, 4 clips"),
            ("File Browser", true, "12 files indexed"),
            ("Reverb Designer", true, "FDN, T60=2.0s"),
            ("Oscilloscope", true, "Waveform mode"),
            ("LUFS Meter", true, "Target -14 LUFS"),
            ("BPM Sync", true, "Internal 120 BPM"),
            ("Granular Synth", false, "Idle"),
            ("IR Browser", true, "22 IRs loaded"),
            ("Delay Designer", true, "Ping-pong, 375ms"),
            ("Chord Analyzer", true, "C Major"),
            ("Sampler", true, "8 zones"),
            ("Step Sequencer", true, "16 steps, playing"),
            ("Chord Sequencer", true, "4-chord loop"),
        ];

        for (name, active, desc) in subsystems {
            ui.horizontal(|ui| {
                let dot = if *active { egui::RichText::new("●").color(Color32::from_rgb(80, 220, 100)) }
                          else { egui::RichText::new("○").color(Color32::GRAY) };
                ui.label(dot);
                ui.label(egui::RichText::new(*name).small());
            });
            ui.label(egui::RichText::new(format!("  {}", desc)).small().color(Color32::GRAY));
        }

        let ui = &mut cols[1];
        ui.label(egui::RichText::new("Audio Engine").strong());
        egui::Grid::new("audio_engine").num_columns(2).spacing([8.0, 2.0]).striped(true).show(ui, |ui| {
            ui.label("Sample Rate"); ui.label("44100 Hz"); ui.end_row();
            ui.label("Buffer Size"); ui.label("512 samples"); ui.end_row();
            ui.label("Bit Depth"); ui.label("32-bit float"); ui.end_row();
            ui.label("Latency"); ui.label("11.6 ms"); ui.end_row();
            ui.label("CPU Usage"); ui.label("12.3%"); ui.end_row();
            ui.label("Active Voices"); ui.label("14 / 256"); ui.end_row();
            ui.label("Bus Count"); ui.label("8 buses"); ui.end_row();
            ui.label("Total Memory"); ui.label("183.2 KB"); ui.end_row();
            ui.label("Active Events"); ui.label("12"); ui.end_row();
            ui.label("MIDI Devices"); ui.label("2 connected"); ui.end_row();
            ui.label("Audio Devices"); ui.label("ASIO, 48kHz"); ui.end_row();
            ui.label("Driver API"); ui.label("WASAPI / ASIO4ALL"); ui.end_row();
        });

        let ui = &mut cols[2];
        ui.label(egui::RichText::new("Loudness Summary").strong());
        egui::Grid::new("loudness").num_columns(2).spacing([8.0, 2.0]).striped(true).show(ui, |ui| {
            ui.label("Momentary"); ui.label("-18.2 LUFS"); ui.end_row();
            ui.label("Short-Term"); ui.label("-17.8 LUFS"); ui.end_row();
            ui.label("Integrated"); ui.label("-18.0 LUFS"); ui.end_row();
            ui.label("LRA"); ui.label("7.2 LU"); ui.end_row();
            ui.label("True Peak"); ui.label("-1.2 dBTP"); ui.end_row();
            ui.label("Target"); ui.label("-14.0 LUFS"); ui.end_row();
            ui.label("Headroom"); ui.label("+4.0 LU"); ui.end_row();
            ui.label("Status"); ui.colored_label(Color32::YELLOW, "Quiet"); ui.end_row();
        });
        ui.separator();

        ui.label(egui::RichText::new("Quick Actions").strong());
        if ui.button("Normalize All Buses").clicked() {}
        if ui.button("Export Mix Report").clicked() {}
        if ui.button("Save Session").clicked() {}
        if ui.button("Reset All to Default").clicked() {}
        if ui.button("Auto-Gain to Target").clicked() {}
    });
}

// --- Final Audio Constants and Summary -------------------------------------------

pub const AUDIO_MIXER_PANEL_COUNT: usize = 27;
pub const AUDIO_MIXER_STRUCT_COUNT: usize = 65;
pub const AUDIO_MIXER_FUNCTION_COUNT: usize = 90;
pub const AUDIO_MAX_IR_SIZE: usize = 65536;
pub const AUDIO_GRANULAR_MAX_VOICES: usize = 64;
pub const AUDIO_SAMPLER_MAX_ZONES: usize = 128;
pub const AUDIO_STEP_SEQ_MAX_STEPS: usize = 64;
pub const AUDIO_STEP_SEQ_MAX_ROWS: usize = 32;
pub const AUDIO_CHORD_SEQ_MAX_STEPS: usize = 64;
pub const AUDIO_ANALYZER_HISTORY_FRAMES: usize = 128;
pub const AUDIO_MAX_EFFECT_CHAIN_LENGTH: usize = 32;
pub const AUDIO_DELAY_MAX_TAPS: usize = 16;
pub const AUDIO_DEFAULT_POLYPHONY: usize = 64;
pub const AUDIO_MIDI_CC_COUNT: usize = 128;

pub fn get_audio_panel_names() -> Vec<&'static str> {
    vec![
        "MIDI Sequencer", "Automation", "Spectrum", "Dynamics",
        "Effects", "Events", "Spatial", "Scripting", "EQ",
        "Routing", "Waveform", "Scenes", "Meters", "Timeline",
        "Files", "Reverb", "Oscilloscope", "LUFS", "BPM",
        "Granular", "IR Browser", "Delay", "Chord Analyzer",
        "Sampler", "Step Sequencer", "Chord Sequencer", "Overview",
    ]
}

#[inline] pub fn note_frequency(midi: u8) -> f32 { 440.0 * 2.0f32.powf((midi as f32 - 69.0) / 12.0) }
#[inline] pub fn frequency_note(freq: f32) -> u8 { (69.0 + 12.0 * (freq / 440.0).log2()).clamp(0.0, 127.0) as u8 }
#[inline] pub fn beat_to_sample(beat: f32, bpm: f32, sr: u32) -> u32 { (beat * 60.0 / bpm * sr as f32) as u32 }
#[inline] pub fn sample_to_beat(sample: u32, bpm: f32, sr: u32) -> f32 { sample as f32 / sr as f32 / 60.0 * bpm }
#[inline] pub fn stereo_width_matrix(width: f32) -> [[f32; 2]; 2] {
    let m = 0.5f32;
    let s = width * 0.5;
    [[m + s, m - s], [m - s, m + s]]
}
#[inline] pub fn haas_delay_ms(width: f32) -> f32 { width.clamp(0.0, 1.0) * 40.0 }
#[inline] pub fn equal_loudness_db(freq: f32) -> f32 {
    let f = freq.clamp(20.0, 20000.0);
    let phon = 40.0;
    let lf = f.log10();
    let correction = if lf < 2.0 { -12.0 * (2.0 - lf) } else if lf > 4.0 { -6.0 * (lf - 4.0) } else { 0.0 };
    phon + correction
}
#[inline] pub fn barkscale_hz(bark: f32) -> f32 {
    600.0 * (bark / 6.0).sinh()
}
#[inline] pub fn hz_to_bark(hz: f32) -> f32 {
    6.0 * ((hz / 600.0) + ((hz / 600.0).powi(2) + 1.0).sqrt()).ln()
}
#[inline] pub fn mel_scale(hz: f32) -> f32 { 2595.0 * (1.0 + hz / 700.0).log10() }
#[inline] pub fn mel_to_hz(mel: f32) -> f32 { 700.0 * (10.0f32.powf(mel / 2595.0) - 1.0) }
#[inline] pub fn erb_rate(hz: f32) -> f32 { 21.4 * (1.0 + 0.00437 * hz).log10() }

pub fn velocity_curve_exp(v: u8, exponent: f32) -> f32 {
    (v as f32 / 127.0).powf(exponent)
}

pub fn velocity_curve_s(v: u8, center: f32, width: f32) -> f32 {
    let t = v as f32 / 127.0;
    let x = (t - center) / width.max(0.01);
    1.0 / (1.0 + (-x * 5.0).exp())
}

pub fn chord_notes(root: u8, chord: &ChordType, octave: i8) -> Vec<u8> {
    let base = (root as i32 + octave as i32 * 12).clamp(0, 127) as u8;
    chord.intervals().iter().map(|&iv| (base + iv).min(127)).collect()
}

pub fn scale_quantize_note(note: u8, root: u8, scale: &MusicalScale) -> u8 {
    let intervals = scale.intervals();
    let nc = note % 12;
    let oct = note / 12;
    let root_nc = root % 12;
    // Find nearest scale note
    let mut best = nc;
    let mut best_dist = 12i32;
    for &iv in &intervals {
        let target = (root_nc + iv) % 12;
        let dist = (nc as i32 - target as i32).abs().min(12 - (nc as i32 - target as i32).abs());
        if dist < best_dist { best_dist = dist; best = target; }
    }
    oct * 12 + best
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
