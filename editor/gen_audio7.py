path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 7
// ============================================================

// --- Chord & Scale Analysis ------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum MusicalScale {
    Major,
    Minor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    WholeTone,
    Diminished,
    Pentatonic,
    Blues,
    Chromatic,
    Custom(Vec<u8>),
}

impl MusicalScale {
    pub fn label(&self) -> &str {
        match self {
            MusicalScale::Major => "Major",
            MusicalScale::Minor => "Minor",
            MusicalScale::HarmonicMinor => "Harmonic Minor",
            MusicalScale::MelodicMinor => "Melodic Minor",
            MusicalScale::Dorian => "Dorian",
            MusicalScale::Phrygian => "Phrygian",
            MusicalScale::Lydian => "Lydian",
            MusicalScale::Mixolydian => "Mixolydian",
            MusicalScale::Locrian => "Locrian",
            MusicalScale::WholeTone => "Whole Tone",
            MusicalScale::Diminished => "Diminished",
            MusicalScale::Pentatonic => "Pentatonic",
            MusicalScale::Blues => "Blues",
            MusicalScale::Chromatic => "Chromatic",
            MusicalScale::Custom(_) => "Custom",
        }
    }

    pub fn intervals(&self) -> Vec<u8> {
        match self {
            MusicalScale::Major => vec![0, 2, 4, 5, 7, 9, 11],
            MusicalScale::Minor => vec![0, 2, 3, 5, 7, 8, 10],
            MusicalScale::HarmonicMinor => vec![0, 2, 3, 5, 7, 8, 11],
            MusicalScale::MelodicMinor => vec![0, 2, 3, 5, 7, 9, 11],
            MusicalScale::Dorian => vec![0, 2, 3, 5, 7, 9, 10],
            MusicalScale::Phrygian => vec![0, 1, 3, 5, 7, 8, 10],
            MusicalScale::Lydian => vec![0, 2, 4, 6, 7, 9, 11],
            MusicalScale::Mixolydian => vec![0, 2, 4, 5, 7, 9, 10],
            MusicalScale::Locrian => vec![0, 1, 3, 5, 6, 8, 10],
            MusicalScale::WholeTone => vec![0, 2, 4, 6, 8, 10],
            MusicalScale::Diminished => vec![0, 2, 3, 5, 6, 8, 9, 11],
            MusicalScale::Pentatonic => vec![0, 2, 4, 7, 9],
            MusicalScale::Blues => vec![0, 3, 5, 6, 7, 10],
            MusicalScale::Chromatic => (0..12).collect(),
            MusicalScale::Custom(v) => v.clone(),
        }
    }

    pub fn contains_note(&self, root: u8, note_class: u8) -> bool {
        let relative = (note_class + 12 - root % 12) % 12;
        self.intervals().contains(&relative)
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ChordType {
    Major,
    Minor,
    Dominant7,
    Major7,
    Minor7,
    Diminished,
    HalfDim7,
    Augmented,
    Sus2,
    Sus4,
    Add9,
    Major9,
    Minor9,
}

impl ChordType {
    pub fn label(&self) -> &str {
        match self {
            ChordType::Major => "maj",
            ChordType::Minor => "m",
            ChordType::Dominant7 => "7",
            ChordType::Major7 => "maj7",
            ChordType::Minor7 => "m7",
            ChordType::Diminished => "dim",
            ChordType::HalfDim7 => "m7b5",
            ChordType::Augmented => "aug",
            ChordType::Sus2 => "sus2",
            ChordType::Sus4 => "sus4",
            ChordType::Add9 => "add9",
            ChordType::Major9 => "maj9",
            ChordType::Minor9 => "m9",
        }
    }
    pub fn intervals(&self) -> Vec<u8> {
        match self {
            ChordType::Major => vec![0, 4, 7],
            ChordType::Minor => vec![0, 3, 7],
            ChordType::Dominant7 => vec![0, 4, 7, 10],
            ChordType::Major7 => vec![0, 4, 7, 11],
            ChordType::Minor7 => vec![0, 3, 7, 10],
            ChordType::Diminished => vec![0, 3, 6],
            ChordType::HalfDim7 => vec![0, 3, 6, 10],
            ChordType::Augmented => vec![0, 4, 8],
            ChordType::Sus2 => vec![0, 2, 7],
            ChordType::Sus4 => vec![0, 5, 7],
            ChordType::Add9 => vec![0, 4, 7, 14],
            ChordType::Major9 => vec![0, 4, 7, 11, 14],
            ChordType::Minor9 => vec![0, 3, 7, 10, 14],
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChordScaleAnalyzer {
    pub root_note: u8,
    pub scale: MusicalScale,
    pub show_chord_chart: bool,
    pub show_interval_labels: bool,
    pub transpose_semitones: i8,
    pub highlight_chord: Option<(u8, ChordType)>,
}

impl Default for ChordScaleAnalyzer {
    fn default() -> Self {
        Self {
            root_note: 0,
            scale: MusicalScale::Major,
            show_chord_chart: true,
            show_interval_labels: true,
            transpose_semitones: 0,
            highlight_chord: None,
        }
    }
}

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

pub fn show_chord_scale_analyzer(ui: &mut egui::Ui, state: &mut ChordScaleAnalyzer) {
    ui.heading("Chord & Scale Analyzer");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Root:");
        for (i, name) in NOTE_NAMES.iter().enumerate() {
            if ui.selectable_label(state.root_note == i as u8, *name).clicked() { state.root_note = i as u8; }
        }
    });

    ui.horizontal_wrapped(|ui| {
        for scale in &[MusicalScale::Major, MusicalScale::Minor, MusicalScale::Dorian, MusicalScale::Phrygian,
                        MusicalScale::Lydian, MusicalScale::Mixolydian, MusicalScale::HarmonicMinor,
                        MusicalScale::Pentatonic, MusicalScale::Blues, MusicalScale::WholeTone, MusicalScale::Diminished] {
            if ui.selectable_label(state.scale == *scale, scale.label()).clicked() { state.scale = scale.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_chord_chart, "Chord Chart");
        ui.checkbox(&mut state.show_interval_labels, "Intervals");
        ui.label("Transpose:");
        ui.add(egui::DragValue::new(&mut state.transpose_semitones).clamp_range(-12i8..=12).suffix(" st"));
    });
    ui.separator();

    draw_piano_keyboard_scale(ui, state);

    if state.show_chord_chart {
        ui.separator();
        draw_chord_chart(ui, state);
    }
}

pub fn draw_piano_keyboard_scale(ui: &mut egui::Ui, state: &ChordScaleAnalyzer) {
    let octaves = 2usize;
    let white_w = 16.0f32;
    let white_h = 60.0f32;
    let black_w = 10.0f32;
    let black_h = 38.0f32;
    let total_white = octaves * 7;
    let size = Vec2::new(total_white as f32 * white_w + 2.0, white_h + 20.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));

    let white_order = [0u8, 2, 4, 5, 7, 9, 11]; // C D E F G A B
    let black_positions = [0usize, 1, 3, 4, 5]; // after which white key: C#, D#, F#, G#, A#
    let black_notes = [1u8, 3, 6, 8, 10]; // C# D# F# G# A#

    let root = state.root_note;
    let intervals = state.scale.intervals();

    // Draw white keys
    for oct in 0..octaves {
        for (wi, &note_class) in white_order.iter().enumerate() {
            let x = rect.left() + 1.0 + (oct * 7 + wi) as f32 * white_w;
            let wr = Rect::from_min_size(Pos2::new(x, rect.top() + 16.0), Vec2::new(white_w - 1.0, white_h));
            let midi = oct as u8 * 12 + note_class + 48;
            let nc = midi % 12;
            let in_scale = intervals.contains(&((nc + 12 - root % 12) % 12));
            let is_root = nc == root % 12;
            let col = if is_root { Color32::from_rgb(80, 200, 120) }
                      else if in_scale { Color32::from_rgb(180, 220, 255) }
                      else { Color32::from_rgb(230, 230, 230) };
            p.rect_filled(wr, 1.0, col);
            p.rect_stroke(wr, 1.0, Stroke::new(0.5, Color32::from_rgb(60, 70, 90)));
            if state.show_interval_labels && in_scale {
                let interval = (nc + 12 - root % 12) % 12;
                let label = match interval { 0 => "R", 2 => "2", 3 => "b3", 4 => "3", 5 => "4", 6 => "b5", 7 => "5", 8 => "b6", 9 => "6", 10 => "b7", 11 => "7", _ => "" };
                p.text(Pos2::new(wr.center().x, wr.bottom() - 6.0), egui::Align2::CENTER_CENTER,
                    label, FontId::monospace(7.0), Color32::from_rgb(40, 40, 60));
            }
        }
    }

    // Draw black keys
    for oct in 0..octaves {
        for (bi, (&wi, &note_class)) in black_positions.iter().zip(black_notes.iter()).enumerate() {
            let x = rect.left() + 1.0 + (oct * 7 + wi) as f32 * white_w + white_w * 0.6;
            let br = Rect::from_min_size(Pos2::new(x, rect.top() + 16.0), Vec2::new(black_w, black_h));
            let midi = oct as u8 * 12 + note_class + 48;
            let nc = midi % 12;
            let in_scale = intervals.contains(&((nc + 12 - root % 12) % 12));
            let is_root = nc == root % 12;
            let col = if is_root { Color32::from_rgb(20, 140, 60) }
                      else if in_scale { Color32::from_rgb(30, 100, 180) }
                      else { Color32::from_rgb(20, 22, 28) };
            p.rect_filled(br, 2.0, col);
        }
    }

    // Note name labels
    for (i, name) in NOTE_NAMES.iter().enumerate() {
        let in_scale = intervals.contains(&((i as u8 + 12 - root % 12) % 12));
        let is_root = i as u8 == root % 12;
        if in_scale || is_root {
            let col = if is_root { Color32::from_rgb(80, 220, 120) } else { Color32::from_rgb(180, 200, 220) };
            p.text(Pos2::new(rect.left() + 4.0 + i as f32 * (rect.width() - 8.0) / 12.0, rect.top() + 8.0),
                egui::Align2::CENTER_CENTER, *name, FontId::monospace(8.0), col);
        }
    }
}

pub fn draw_chord_chart(ui: &mut egui::Ui, state: &ChordScaleAnalyzer) {
    ui.label(egui::RichText::new(format!("{} {} Scale — Diatonic Chords", NOTE_NAMES[state.root_note as usize % 12], state.scale.label())).strong());
    let intervals = state.scale.intervals();
    let n = intervals.len();
    ui.horizontal_wrapped(|ui| {
        for (deg, &ivl) in intervals.iter().enumerate() {
            let root_nc = (state.root_note + ivl) % 12;
            let third = intervals.get((deg + 2) % n).copied().unwrap_or(0);
            let fifth = intervals.get((deg + 4) % n).copied().unwrap_or(0);
            let third_rel = (third + 12 - ivl) % 12;
            let fifth_rel = (fifth + 12 - ivl) % 12;
            let chord_type = if third_rel == 4 && fifth_rel == 7 { ChordType::Major }
                else if third_rel == 3 && fifth_rel == 7 { ChordType::Minor }
                else if third_rel == 3 && fifth_rel == 6 { ChordType::Diminished }
                else if third_rel == 4 && fifth_rel == 8 { ChordType::Augmented }
                else { ChordType::Major };
            let roman = ["I","II","III","IV","V","VI","VII","VIII"][deg.min(7)];
            let roman_case = match &chord_type {
                ChordType::Minor | ChordType::Diminished => roman.to_lowercase(),
                _ => roman.to_string(),
            };
            let col = match &chord_type {
                ChordType::Major => Color32::from_rgb(80, 200, 120),
                ChordType::Minor => Color32::from_rgb(100, 150, 220),
                ChordType::Diminished => Color32::from_rgb(200, 80, 80),
                ChordType::Augmented => Color32::from_rgb(255, 180, 60),
                _ => Color32::GRAY,
            };
            ui.group(|ui| {
                ui.label(egui::RichText::new(format!("{}", roman_case)).strong().color(col));
                ui.label(egui::RichText::new(format!("{}{}", NOTE_NAMES[root_nc as usize], chord_type.label())).small().color(col));
            });
        }
    });
}

// --- Sampler Editor --------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum SamplerMode {
    OneShot,
    Loop,
    PingPong,
    Sustain,
    Slice,
}

impl SamplerMode {
    pub fn label(&self) -> &str {
        match self {
            SamplerMode::OneShot => "One Shot",
            SamplerMode::Loop => "Loop",
            SamplerMode::PingPong => "Ping Pong",
            SamplerMode::Sustain => "Sustain",
            SamplerMode::Slice => "Slice",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SamplerZone {
    pub id: usize,
    pub name: String,
    pub clip_name: String,
    pub root_note: u8,
    pub low_note: u8,
    pub high_note: u8,
    pub low_velocity: u8,
    pub high_velocity: u8,
    pub volume_db: f32,
    pub pan: f32,
    pub pitch_tune_cents: f32,
    pub sample_start: f32,
    pub sample_end: f32,
    pub loop_start: f32,
    pub loop_end: f32,
    pub loop_crossfade: f32,
    pub mode: SamplerMode,
    pub reverse: bool,
    pub filter_lp: f32,
    pub filter_hp: f32,
    pub envelope_attack_ms: f32,
    pub envelope_decay_ms: f32,
    pub envelope_sustain: f32,
    pub envelope_release_ms: f32,
}

impl SamplerZone {
    pub fn new(id: usize, root: u8) -> Self {
        Self {
            id,
            name: format!("Zone_{}", id),
            clip_name: "sample.wav".into(),
            root_note: root,
            low_note: root.saturating_sub(6),
            high_note: (root + 6).min(127),
            low_velocity: 0,
            high_velocity: 127,
            volume_db: 0.0,
            pan: 0.0,
            pitch_tune_cents: 0.0,
            sample_start: 0.0,
            sample_end: 1.0,
            loop_start: 0.0,
            loop_end: 1.0,
            loop_crossfade: 0.0,
            mode: SamplerMode::OneShot,
            reverse: false,
            filter_lp: 20000.0,
            filter_hp: 20.0,
            envelope_attack_ms: 5.0,
            envelope_decay_ms: 200.0,
            envelope_sustain: 1.0,
            envelope_release_ms: 200.0,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SamplerEditorState {
    pub zones: Vec<SamplerZone>,
    pub selected: Option<usize>,
    pub preview_note: Option<u8>,
    pub voices: u8,
    pub polyphony: u8,
    pub interpolation: u8,
    pub show_keyboard: bool,
    pub show_velocity_grid: bool,
}

impl SamplerEditorState {
    pub fn with_demo() -> Self {
        let zones = (0..8).map(|i| SamplerZone::new(i, 36 + i as u8 * 12)).collect();
        Self { zones, voices: 16, polyphony: 16, interpolation: 2, show_keyboard: true, show_velocity_grid: false, ..Default::default() }
    }
}

pub fn show_sampler_editor(ui: &mut egui::Ui, state: &mut SamplerEditorState) {
    ui.heading("Sampler Editor");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ Zone").clicked() {
            let root = 60u8;
            let id = state.zones.len();
            state.zones.push(SamplerZone::new(id, root));
        }
        ui.label("Voices:");
        ui.add(egui::DragValue::new(&mut state.voices).clamp_range(1u8..=64));
        ui.label("Poly:");
        ui.add(egui::DragValue::new(&mut state.polyphony).clamp_range(1u8..=64));
        ui.label("Interp:");
        ui.add(egui::DragValue::new(&mut state.interpolation).clamp_range(0u8..=3));
        ui.separator();
        ui.checkbox(&mut state.show_keyboard, "Keyboard");
        ui.checkbox(&mut state.show_velocity_grid, "Vel Grid");
    });
    ui.separator();

    if state.show_keyboard {
        draw_sampler_keyboard_map(ui, state);
        ui.separator();
    }

    let mut remove = None;
    for (i, zone) in state.zones.iter().enumerate() {
        let sel = state.selected == Some(i);
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, format!("{} [{}]  N{}-N{}  V{}-V{}",
                zone.name, zone.clip_name, zone.low_note, zone.high_note,
                zone.low_velocity, zone.high_velocity)).clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.zones.remove(ri); if state.selected == Some(ri) { state.selected = None; } }

    if let Some(idx) = state.selected {
        if idx < state.zones.len() {
            ui.separator();
            let zone = &mut state.zones[idx];
            show_sampler_zone_settings(ui, zone);
        }
    }
}

pub fn draw_sampler_keyboard_map(ui: &mut egui::Ui, state: &SamplerEditorState) {
    let key_w = 6.0f32;
    let total_notes = 88usize;
    let h = 40.0f32;
    let size = Vec2::new(key_w * total_notes as f32 + 2.0, h + 16.0);
    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    // Note 21 = A0, last = C8 = 108
    let n_min = 21u8;
    for ni in 0..total_notes {
        let note = n_min + ni as u8;
        let x = rect.left() + 1.0 + ni as f32 * key_w;
        let is_black = [1, 3, 6, 8, 10].contains(&(note % 12));
        let bg = if is_black { Color32::from_rgb(20, 22, 28) } else { Color32::from_rgb(40, 45, 55) };
        let nr = Rect::from_min_size(Pos2::new(x, rect.top() + 14.0), Vec2::new(key_w - 0.5, h));
        p.rect_filled(nr, 0.0, bg);

        // Zone overlays
        for (zi, zone) in state.zones.iter().enumerate() {
            if note >= zone.low_note && note <= zone.high_note {
                let colors: [[u8; 3]; 8] = [[80,160,255],[255,160,60],[80,220,100],[200,80,255],[255,80,120],[80,220,220],[255,220,60],[160,200,80]];
                let col = colors[zi % 8];
                let sel = state.selected == Some(zi);
                let alpha = if sel { 200u8 } else { 120u8 };
                p.rect_filled(Rect::from_min_size(Pos2::new(x, rect.top() + 14.0), Vec2::new(key_w - 0.5, h * 0.7)),
                    0.0, Color32::from_rgba_premultiplied(col[0], col[1], col[2], alpha));
                if note == zone.root_note {
                    p.circle_filled(Pos2::new(x + key_w * 0.5, rect.top() + 14.0 + h * 0.85),
                        2.5, Color32::from_rgb(col[0], col[1], col[2]));
                }
            }
        }
    }

    // Octave labels
    for oct in 0..9u8 {
        let note = oct * 12 + 24;
        if note >= n_min {
            let ni = (note - n_min) as f32;
            let x = rect.left() + 1.0 + ni * key_w;
            p.text(Pos2::new(x, rect.top() + 8.0), egui::Align2::LEFT_CENTER,
                format!("C{}", oct + 1), FontId::monospace(7.0), Color32::GRAY);
        }
    }
}

pub fn show_sampler_zone_settings(ui: &mut egui::Ui, zone: &mut SamplerZone) {
    ui.label(egui::RichText::new("Zone Settings").strong());
    ui.horizontal(|ui| {
        ui.label("Name:"); ui.text_edit_singleline(&mut zone.name);
        ui.label("File:"); ui.text_edit_singleline(&mut zone.clip_name);
        if ui.button("Browse").clicked() {}
    });
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label("Note Range:");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut zone.low_note).clamp_range(0u8..=127).prefix("Lo: "));
            ui.label(format!("({})", NOTE_NAMES[zone.low_note as usize % 12]));
            ui.add(egui::DragValue::new(&mut zone.high_note).clamp_range(0u8..=127).prefix("Hi: "));
            ui.label(format!("({})", NOTE_NAMES[zone.high_note as usize % 12]));
        });
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut zone.root_note).clamp_range(0u8..=127).prefix("Root: "));
            ui.label(format!("({})", NOTE_NAMES[zone.root_note as usize % 12]));
        });
        ui.label("Velocity Range:");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut zone.low_velocity).clamp_range(0u8..=127).prefix("Lo: "));
            ui.add(egui::DragValue::new(&mut zone.high_velocity).clamp_range(0u8..=127).prefix("Hi: "));
        });
        ui.separator();
        ui.horizontal(|ui| { ui.label("Volume:"); ui.add(egui::DragValue::new(&mut zone.volume_db).speed(0.5).clamp_range(-60.0..=12.0).suffix(" dB")); });
        ui.horizontal(|ui| { ui.label("Pan:"); ui.add(egui::Slider::new(&mut zone.pan, -1.0..=1.0)); });
        ui.horizontal(|ui| { ui.label("Tune:"); ui.add(egui::Slider::new(&mut zone.pitch_tune_cents, -100.0..=100.0).suffix(" cents")); });

        let ui = &mut cols[1];
        ui.label("Loop Mode:");
        for m in &[SamplerMode::OneShot, SamplerMode::Loop, SamplerMode::PingPong, SamplerMode::Sustain, SamplerMode::Slice] {
            if ui.selectable_label(zone.mode == *m, m.label()).clicked() { zone.mode = m.clone(); }
        }
        ui.checkbox(&mut zone.reverse, "Reverse");
        ui.horizontal(|ui| { ui.label("LP:"); ui.add(egui::DragValue::new(&mut zone.filter_lp).speed(100.0).clamp_range(200.0..=20000.0).suffix(" Hz")); });
        ui.horizontal(|ui| { ui.label("HP:"); ui.add(egui::DragValue::new(&mut zone.filter_hp).speed(10.0).clamp_range(20.0..=2000.0).suffix(" Hz")); });
        ui.separator();
        ui.label("ADSR:");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut zone.envelope_attack_ms).speed(1.0).clamp_range(0.0..=5000.0).suffix(" ms").prefix("A: "));
            ui.add(egui::DragValue::new(&mut zone.envelope_decay_ms).speed(1.0).clamp_range(0.0..=5000.0).suffix(" ms").prefix("D: "));
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut zone.envelope_sustain, 0.0..=1.0).prefix("S: "));
            ui.add(egui::DragValue::new(&mut zone.envelope_release_ms).speed(1.0).clamp_range(0.0..=10000.0).suffix(" ms").prefix("R: "));
        });
    });
}

// --- Mix Bus Compressor Overview -------------------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct BusCompressorOverview {
    pub buses: Vec<(String, DynamicsProcessor, bool)>,
    pub selected: Option<usize>,
}

impl BusCompressorOverview {
    pub fn with_demo() -> Self {
        let mut s = Self::default();
        let bus_names = ["Master", "Drums", "Bass", "Keys", "Guitars", "Vocals", "FX"];
        for name in &bus_names {
            let mut proc = DynamicsProcessor::default();
            proc.threshold_db = -18.0 - (name.len() as f32 * 0.5);
            s.buses.push((name.to_string(), proc, true));
        }
        s
    }
}

pub fn show_bus_compressor_overview(ui: &mut egui::Ui, state: &mut BusCompressorOverview) {
    ui.heading("Bus Compressor Overview");
    ui.separator();

    if ui.button("+ Add Bus Comp").clicked() {
        state.buses.push(("Bus".into(), DynamicsProcessor::default(), true));
    }
    ui.separator();

    let mut remove = None;
    for (i, (name, proc, enabled)) in state.buses.iter_mut().enumerate() {
        let sel = state.selected == Some(i);
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, name.as_str()).clicked() {
                state.selected = if sel { None } else { Some(i) };
            }
            ui.checkbox(enabled, "");
            let gr = proc.compute_gain_reduction(proc.input_level_db);
            let gr_abs = gr.abs().min(30.0);
            let t = gr_abs / 30.0;
            let col = if gr_abs > 12.0 { Color32::RED } else if gr_abs > 6.0 { Color32::YELLOW } else { Color32::from_rgb(80, 220, 100) };
            let mw = 80.0f32;
            let (mr, _) = ui.allocate_exact_size(Vec2::new(mw, 14.0), egui::Sense::hover());
            let mp = ui.painter_at(mr);
            mp.rect_filled(mr, 2.0, Color32::from_rgb(20, 22, 28));
            mp.rect_filled(Rect::from_min_size(mr.min, Vec2::new(t * mw, 14.0)), 2.0, col);
            ui.label(format!("{:.1} dB GR", gr));
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.buses.remove(ri); if state.selected == Some(ri) { state.selected = None; } }

    if let Some(idx) = state.selected {
        if idx < state.buses.len() {
            ui.separator();
            let (name, proc, _) = &mut state.buses[idx];
            ui.horizontal(|ui| { ui.label("Bus:"); ui.text_edit_singleline(name); });
            show_dynamics_editor(ui, &mut DynamicsEditorState { processor: proc.clone(), show_transfer: true, show_meter: true, history_gr: vec![], history_in: vec![], history_out: vec![] });
        }
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
