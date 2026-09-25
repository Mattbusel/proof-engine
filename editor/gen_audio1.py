path = r"C:\proof-engine\editor\src\audio_mixer.rs"

EXPANSION = r"""
// ============================================================
// AUDIO MIXER — EXPANSION BLOCK 1
// MIDI Sequencer, Automation Lanes, Spectral Analyzer
// ============================================================

use std::collections::HashMap;

// ─── MIDI Sequencer ───────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct MidiNote {
    pub pitch: u8,        // 0-127, 60=C4
    pub velocity: u8,     // 0-127
    pub start_beat: f32,
    pub duration_beats: f32,
    pub channel: u8,
    pub selected: bool,
}

impl MidiNote {
    pub fn note_name(pitch: u8) -> &'static str {
        const NAMES: &[&str] = &["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"];
        NAMES[(pitch % 12) as usize]
    }
    pub fn octave(pitch: u8) -> i32 { (pitch as i32 / 12) - 1 }
    pub fn label(pitch: u8) -> String {
        format!("{}{}", Self::note_name(pitch), Self::octave(pitch))
    }
    pub fn frequency(pitch: u8) -> f32 {
        440.0 * 2.0f32.powf((pitch as f32 - 69.0) / 12.0)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MidiTrack {
    pub name: String,
    pub notes: Vec<MidiNote>,
    pub instrument: String,
    pub channel: u8,
    pub muted: bool,
    pub solo: bool,
    pub volume: f32,
    pub pan: f32,
    pub color: [f32; 3],
    pub visible: bool,
}

impl MidiTrack {
    pub fn new(name: &str, channel: u8) -> Self {
        Self { name: name.into(), notes: vec![], instrument: "Piano".into(), channel, muted: false, solo: false, volume: 1.0, pan: 0.0, color: [0.3, 0.6, 1.0], visible: true }
    }
    pub fn add_sample_notes(&mut self) {
        let scale = [60u8, 62, 64, 65, 67, 69, 71, 72];
        for (i, &pitch) in scale.iter().enumerate() {
            self.notes.push(MidiNote { pitch, velocity: 80 + (i as u8 * 5), start_beat: i as f32, duration_beats: 0.8, channel: self.channel, selected: false });
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MidiSequencer {
    pub tracks: Vec<MidiTrack>,
    pub bpm: f32,
    pub time_signature_num: u8,
    pub time_signature_den: u8,
    pub total_bars: u32,
    pub current_beat: f32,
    pub playing: bool,
    pub loop_enabled: bool,
    pub loop_start_beat: f32,
    pub loop_end_beat: f32,
    pub quantize: MidiQuantize,
    pub selected_track: Option<usize>,
    pub zoom_x: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub show_piano_keys: bool,
    pub snap_to_grid: bool,
    pub metronome: bool,
    pub transpose_semitones: i32,
    pub velocity_mode: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum MidiQuantize {
    Quarter,
    Eighth,
    #[default]
    Sixteenth,
    ThirtySecond,
    Triplet,
    None,
}

impl MidiQuantize {
    pub fn label(&self) -> &str {
        match self {
            MidiQuantize::Quarter => "1/4",
            MidiQuantize::Eighth => "1/8",
            MidiQuantize::Sixteenth => "1/16",
            MidiQuantize::ThirtySecond => "1/32",
            MidiQuantize::Triplet => "1/8T",
            MidiQuantize::None => "Free",
        }
    }
    pub fn beats_per_unit(&self) -> f32 {
        match self {
            MidiQuantize::Quarter => 1.0,
            MidiQuantize::Eighth => 0.5,
            MidiQuantize::Sixteenth => 0.25,
            MidiQuantize::ThirtySecond => 0.125,
            MidiQuantize::Triplet => 1.0/3.0,
            MidiQuantize::None => 0.0,
        }
    }
}

impl MidiSequencer {
    pub fn new() -> Self {
        let mut tracks = vec![
            MidiTrack::new("Piano", 0),
            MidiTrack::new("Bass", 1),
            MidiTrack::new("Drums", 9),
        ];
        tracks[0].add_sample_notes();
        tracks[1].color = [0.8, 0.4, 0.2];
        tracks[2].color = [0.8, 0.8, 0.2];
        Self { tracks, bpm: 120.0, time_signature_num: 4, time_signature_den: 4, total_bars: 8, current_beat: 0.0, playing: false, loop_enabled: true, loop_start_beat: 0.0, loop_end_beat: 16.0, quantize: MidiQuantize::Sixteenth, selected_track: Some(0), zoom_x: 1.0, scroll_x: 0.0, scroll_y: 0.0, show_piano_keys: true, snap_to_grid: true, metronome: false, transpose_semitones: 0, velocity_mode: false }
    }
    pub fn total_beats(&self) -> f32 { self.total_bars as f32 * self.time_signature_num as f32 }
    pub fn beat_duration_secs(&self) -> f32 { 60.0 / self.bpm }
}

pub fn show_midi_sequencer(ui: &mut egui::Ui, seq: &mut MidiSequencer) {
    ui.heading("MIDI Sequencer");
    ui.separator();

    // Transport controls
    ui.horizontal(|ui| {
        let play_lbl = if seq.playing { "Pause (Space)" } else { "Play (Space)" };
        if ui.button(play_lbl).clicked() { seq.playing = !seq.playing; }
        if ui.button("|<< Rewind").clicked() { seq.current_beat = 0.0; }
        if ui.button("Stop").clicked() { seq.playing = false; seq.current_beat = 0.0; }
        ui.checkbox(&mut seq.loop_enabled, "Loop");
        ui.checkbox(&mut seq.metronome, "Metro");
        ui.separator();
        ui.label("BPM:");
        ui.add(egui::DragValue::new(&mut seq.bpm).speed(0.5).clamp_range(20.0..=300.0));
        ui.label(format!("{}/ {}", seq.time_signature_num, seq.time_signature_den));
        ui.label(format!("Beat: {:.2}", seq.current_beat));
    });

    ui.horizontal(|ui| {
        ui.label("Quantize:");
        for q in &[MidiQuantize::Quarter, MidiQuantize::Eighth, MidiQuantize::Sixteenth, MidiQuantize::ThirtySecond, MidiQuantize::Triplet, MidiQuantize::None] {
            if ui.selectable_label(seq.quantize == *q, q.label()).clicked() { seq.quantize = q.clone(); }
        }
        ui.separator();
        ui.checkbox(&mut seq.snap_to_grid, "Snap");
        ui.checkbox(&mut seq.show_piano_keys, "Keys");
        ui.checkbox(&mut seq.velocity_mode, "Velocity");
        ui.label("Zoom:");
        ui.add(egui::Slider::new(&mut seq.zoom_x, 0.5..=4.0));
        ui.label("Transpose:");
        ui.add(egui::DragValue::new(&mut seq.transpose_semitones).clamp_range(-24..=24));
    });

    // Track list
    ui.horizontal(|ui| {
        if ui.button("+ Track").clicked() {
            let n = seq.tracks.len() as u8;
            seq.tracks.push(MidiTrack::new(&format!("Track_{}", n), n));
        }
        if let Some(s) = seq.selected_track {
            if ui.button("Delete Track").clicked() { seq.tracks.remove(s); seq.selected_track = None; }
            if ui.button("Duplicate").clicked() {
                if s < seq.tracks.len() {
                    let mut t = seq.tracks[s].clone();
                    t.name = format!("{}_copy", t.name);
                    seq.tracks.push(t);
                }
            }
        }
        ui.label(format!("{} tracks | {} total notes", seq.tracks.len(), seq.tracks.iter().map(|t| t.notes.len()).sum::<usize>()));
    });

    let track_count = seq.tracks.len();
    for i in 0..track_count {
        let t = &mut seq.tracks[i];
        let sel = seq.selected_track == Some(i);
        let tc = Color32::from_rgb((t.color[0]*255.0) as u8, (t.color[1]*255.0) as u8, (t.color[2]*255.0) as u8);
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, egui::RichText::new(format!("Ch{} {}", t.channel, t.name)).color(if t.muted { Color32::DARK_GRAY } else { tc })).clicked() { seq.selected_track = Some(i); }
            if ui.small_button(if t.muted { "M+" } else { "M" }).clicked() { t.muted = !t.muted; }
            if ui.small_button(if t.solo { "S+" } else { "S" }).clicked() { t.solo = !t.solo; }
            ui.add(egui::Slider::new(&mut t.volume, 0.0..=1.5).desired_width(60.0).text("vol"));
            ui.add(egui::Slider::new(&mut t.pan, -1.0..=1.0).desired_width(60.0).text("pan"));
            ui.label(format!("{} notes", t.notes.len()));
        });
    }

    draw_piano_roll(ui, seq);
}

fn draw_piano_roll(ui: &mut egui::Ui, seq: &mut MidiSequencer) {
    let desired = Vec2::new(ui.available_width().min(800.0), 280.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(14, 16, 22));

    let piano_width = if seq.show_piano_keys { 44.0 } else { 0.0 };
    let roll_rect = Rect::from_min_max(Pos2::new(rect.left() + piano_width, rect.top()), rect.max);

    let total_beats = seq.total_beats();
    let visible_beats = total_beats / seq.zoom_x;
    let beat_px = roll_rect.width() / visible_beats;
    let note_height = 5.0;
    let min_pitch = 36u8;
    let max_pitch = 96u8;
    let pitch_range = (max_pitch - min_pitch) as f32;
    let visible_pitch_range = (roll_rect.height() / note_height).min(pitch_range) as u8;

    // Background grid
    let start_pitch = min_pitch + (seq.scroll_y as u8).min(pitch_range as u8 - visible_pitch_range);
    for pitch in start_pitch..=start_pitch + visible_pitch_range {
        let row = (pitch - start_pitch) as f32;
        let y = roll_rect.top() + (pitch_range - row - 1.0) * note_height - seq.scroll_y;
        let is_black = matches!(pitch % 12, 1|3|6|8|10);
        let row_col = if is_black { Color32::from_rgb(22, 25, 32) } else { Color32::from_rgb(28, 32, 40) };
        let row_rect = Rect::from_min_size(Pos2::new(roll_rect.left(), y), Vec2::new(roll_rect.width(), note_height));
        p.rect_filled(row_rect, 0.0, row_col);

        // C note markers
        if pitch % 12 == 0 {
            p.text(Pos2::new(roll_rect.left()+2.0, y+1.0), egui::Align2::LEFT_TOP, MidiNote::label(pitch), FontId::monospace(6.0), Color32::from_rgb(80,80,80));
        }
    }

    // Beat lines
    let start_beat = seq.scroll_x;
    let end_beat = start_beat + visible_beats;
    let mut beat = (start_beat / seq.quantize.beats_per_unit().max(0.01)).floor() * seq.quantize.beats_per_unit().max(0.01);
    while beat <= end_beat {
        let x = roll_rect.left() + (beat - start_beat) * beat_px;
        let is_bar = (beat / seq.time_signature_num as f32).fract() < 0.01;
        let beat_col = if is_bar { Color32::from_rgb(70,80,90) } else { Color32::from_rgb(35,40,48) };
        p.line_segment([Pos2::new(x, roll_rect.top()), Pos2::new(x, roll_rect.bottom())], Stroke::new(if is_bar { 1.5 } else { 0.5 }, beat_col));
        if is_bar {
            let bar_num = (beat / seq.time_signature_num as f32) as i32 + 1;
            p.text(Pos2::new(x+2.0, roll_rect.top()+2.0), egui::Align2::LEFT_TOP, format!("{}", bar_num), FontId::monospace(8.0), Color32::from_rgb(100,110,120));
        }
        beat += seq.quantize.beats_per_unit().max(0.25);
    }

    // Draw notes for selected track
    if let Some(track_idx) = seq.selected_track {
        if track_idx < seq.tracks.len() {
            let track = &seq.tracks[track_idx];
            if !track.muted {
                let tc = Color32::from_rgb((track.color[0]*220.0) as u8, (track.color[1]*220.0) as u8, (track.color[2]*220.0) as u8);
                for note in &track.notes {
                    let adjusted_pitch = (note.pitch as i32 + seq.transpose_semitones).clamp(0, 127) as u8;
                    if adjusted_pitch < start_pitch || adjusted_pitch > start_pitch + visible_pitch_range { continue; }
                    let nx = roll_rect.left() + (note.start_beat - start_beat) * beat_px;
                    let nw = note.duration_beats * beat_px;
                    let row = (adjusted_pitch - start_pitch) as f32;
                    let ny = roll_rect.top() + (pitch_range - row - 1.0) * note_height - seq.scroll_y;
                    if nx + nw < roll_rect.left() || nx > roll_rect.right() { continue; }
                    let note_rect = Rect::from_min_size(Pos2::new(nx.max(roll_rect.left()), ny), Vec2::new(nw.min(roll_rect.right()-nx), note_height - 1.0));
                    let vel_alpha = (note.velocity as f32 / 127.0 * 200.0 + 55.0) as u8;
                    p.rect_filled(note_rect, 1.0, Color32::from_rgba_unmultiplied(tc.r(), tc.g(), tc.b(), vel_alpha));
                    if note.selected { p.rect_stroke(note_rect, 1.0, Stroke::new(1.5, Color32::WHITE)); }
                    if nw > 20.0 {
                        p.text(note_rect.min + Vec2::new(2.0, 0.0), egui::Align2::LEFT_TOP, MidiNote::label(adjusted_pitch), FontId::monospace(5.5), Color32::from_rgba_unmultiplied(255,255,255,180));
                    }
                }
            }
        }
    }

    // Playhead
    let ph_x = roll_rect.left() + (seq.current_beat - start_beat) * beat_px;
    if ph_x >= roll_rect.left() && ph_x <= roll_rect.right() {
        p.line_segment([Pos2::new(ph_x, roll_rect.top()), Pos2::new(ph_x, roll_rect.bottom())], Stroke::new(2.0, Color32::from_rgb(255, 80, 80)));
    }

    // Loop region
    if seq.loop_enabled {
        let lx0 = roll_rect.left() + (seq.loop_start_beat - start_beat) * beat_px;
        let lx1 = roll_rect.left() + (seq.loop_end_beat - start_beat) * beat_px;
        let loop_rect = Rect::from_min_max(Pos2::new(lx0.max(roll_rect.left()), roll_rect.top()), Pos2::new(lx1.min(roll_rect.right()), roll_rect.top()+12.0));
        p.rect_filled(loop_rect, 0.0, Color32::from_rgba_unmultiplied(60, 200, 100, 60));
        p.rect_stroke(loop_rect, 0.0, Stroke::new(1.0, Color32::from_rgb(60,200,100)));
    }

    // Piano keys
    if seq.show_piano_keys {
        let keys_rect = Rect::from_min_max(rect.min, Pos2::new(rect.left() + piano_width, rect.max.y));
        p.rect_filled(keys_rect, 0.0, Color32::from_rgb(20, 22, 28));
        for pitch in start_pitch..=start_pitch+visible_pitch_range {
            let row = (pitch - start_pitch) as f32;
            let y = rect.top() + (pitch_range - row - 1.0) * note_height - seq.scroll_y;
            let is_black = matches!(pitch % 12, 1|3|6|8|10);
            let key_col = if is_black { Color32::from_rgb(30,30,40) } else { Color32::from_rgb(200,200,210) };
            let kw = if is_black { piano_width * 0.65 } else { piano_width };
            p.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(kw, note_height-1.0)), 0.0, key_col);
        }
    }

    // Handle scroll via drag
    if response.dragged_by(egui::PointerButton::Middle) {
        let drag = response.drag_delta();
        seq.scroll_x = (seq.scroll_x - drag.x / beat_px).clamp(0.0, total_beats);
        seq.scroll_y = (seq.scroll_y + drag.y).clamp(0.0, pitch_range * note_height - roll_rect.height());
    }
}

pub fn show_midi_note_editor(ui: &mut egui::Ui, seq: &mut MidiSequencer) {
    ui.heading("MIDI Note Properties");
    ui.separator();

    if let Some(track_idx) = seq.selected_track {
        if track_idx < seq.tracks.len() {
            let track = &mut seq.tracks[track_idx];
            let selected_notes: Vec<usize> = track.notes.iter().enumerate().filter(|(_, n)| n.selected).map(|(i, _)| i).collect();
            ui.label(format!("Track: {} | {} notes selected", track.name, selected_notes.len()));

            if !selected_notes.is_empty() {
                let first_idx = selected_notes[0];
                if first_idx < track.notes.len() {
                    let note = &mut track.notes[first_idx];
                    ui.horizontal(|ui| {
                        ui.label("Pitch:");
                        ui.add(egui::DragValue::new(&mut note.pitch).clamp_range(0..=127));
                        ui.label(MidiNote::label(note.pitch));
                        ui.label(format!("({:.1} Hz)", MidiNote::frequency(note.pitch)));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Velocity:");
                        ui.add(egui::Slider::new(&mut note.velocity, 1..=127));
                        let vel_category = match note.velocity {
                            0..=31 => "ppp",
                            32..=63 => "pp/p",
                            64..=95 => "mp/mf",
                            96..=111 => "f",
                            112..=127 => "ff/fff",
                            _ => "?"
                        };
                        ui.label(vel_category);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Start Beat:");
                        ui.add(egui::DragValue::new(&mut note.start_beat).speed(0.0625).clamp_range(0.0..=1000.0));
                        ui.label("Duration:");
                        ui.add(egui::DragValue::new(&mut note.duration_beats).speed(0.0625).clamp_range(0.0625..=32.0));
                    });
                    ui.label(format!("Channel: {}", note.channel));
                }
            } else {
                ui.label("No notes selected in piano roll.");
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Select All").clicked() { for n in &mut track.notes { n.selected = true; } }
                if ui.button("Deselect All").clicked() { for n in &mut track.notes { n.selected = false; } }
                if ui.button("Delete Selected").clicked() { track.notes.retain(|n| !n.selected); }
                if ui.button("Quantize Selected").clicked() {
                    let unit = seq.quantize.beats_per_unit().max(0.0625);
                    for n in track.notes.iter_mut().filter(|n| n.selected) {
                        n.start_beat = (n.start_beat / unit).round() * unit;
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Transpose +1").clicked() { for n in track.notes.iter_mut().filter(|n| n.selected && n.pitch < 127) { n.pitch += 1; } }
                if ui.button("Transpose -1").clicked() { for n in track.notes.iter_mut().filter(|n| n.selected && n.pitch > 0) { n.pitch -= 1; } }
                if ui.button("Transpose +12").clicked() { for n in track.notes.iter_mut().filter(|n| n.selected && n.pitch <= 115) { n.pitch += 12; } }
                if ui.button("Transpose -12").clicked() { for n in track.notes.iter_mut().filter(|n| n.selected && n.pitch >= 12) { n.pitch -= 12; } }
                if ui.button("Vel +10").clicked() { for n in track.notes.iter_mut().filter(|n| n.selected) { n.velocity = (n.velocity as i32 + 10).clamp(1,127) as u8; } }
                if ui.button("Vel -10").clicked() { for n in track.notes.iter_mut().filter(|n| n.selected) { n.velocity = (n.velocity as i32 - 10).clamp(1,127) as u8; } }
            });
        }
    }
}

pub fn show_midi_track_settings(ui: &mut egui::Ui, seq: &mut MidiSequencer) {
    ui.heading("Track Settings");
    ui.separator();

    if let Some(idx) = seq.selected_track {
        if idx < seq.tracks.len() {
            let t = &mut seq.tracks[idx];
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut t.name); });
            ui.horizontal(|ui| { ui.label("Instrument:"); ui.text_edit_singleline(&mut t.instrument); });
            ui.horizontal(|ui| {
                ui.label("Channel:");
                ui.add(egui::DragValue::new(&mut t.channel).clamp_range(0..=15));
                ui.label("Volume:");
                ui.add(egui::Slider::new(&mut t.volume, 0.0..=1.5));
            });
            ui.horizontal(|ui| {
                ui.label("Pan:");
                ui.add(egui::Slider::new(&mut t.pan, -1.0..=1.0));
                ui.checkbox(&mut t.muted, "Muted");
                ui.checkbox(&mut t.solo, "Solo");
            });
        }
    }
}

// ─── Automation Lanes ─────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct AutoPoint {
    pub beat: f32,
    pub value: f32,
    pub curve_type: CurveType,
    pub tension: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum CurveType {
    Linear,
    CubicHermite,
    Step,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl CurveType {
    pub fn label(&self) -> &str {
        match self {
            CurveType::Linear => "Linear",
            CurveType::CubicHermite => "Cubic",
            CurveType::Step => "Step",
            CurveType::EaseIn => "Ease In",
            CurveType::EaseOut => "Ease Out",
            CurveType::EaseInOut => "Ease In/Out",
        }
    }
}

impl Default for CurveType { fn default() -> Self { CurveType::Linear } }

#[derive(Clone, Serialize, Deserialize)]
pub struct AutomationLane {
    pub name: String,
    pub parameter: String,
    pub points: Vec<AutoPoint>,
    pub min_value: f32,
    pub max_value: f32,
    pub default_value: f32,
    pub visible: bool,
    pub active: bool,
    pub color: [f32; 3],
    pub track_index: Option<usize>,
    pub bus_index: Option<usize>,
}

impl AutomationLane {
    pub fn new(name: &str, parameter: &str, min: f32, max: f32, default: f32) -> Self {
        Self { name: name.into(), parameter: parameter.into(), points: vec![], min_value: min, max_value: max, default_value: default, visible: true, active: true, color: [0.9, 0.6, 0.2], track_index: None, bus_index: None }
    }

    pub fn new_volume() -> Self {
        let mut l = Self::new("Volume", "volume", 0.0, 1.5, 1.0);
        l.color = [0.4, 0.8, 0.4];
        l.points = vec![
            AutoPoint { beat: 0.0, value: 0.0, curve_type: CurveType::Linear, tension: 0.0 },
            AutoPoint { beat: 1.0, value: 1.0, curve_type: CurveType::EaseIn, tension: 0.0 },
            AutoPoint { beat: 8.0, value: 1.0, curve_type: CurveType::Linear, tension: 0.0 },
            AutoPoint { beat: 9.0, value: 0.0, curve_type: CurveType::EaseOut, tension: 0.0 },
        ];
        l
    }

    pub fn new_pan() -> Self {
        let mut l = Self::new("Pan", "pan", -1.0, 1.0, 0.0);
        l.color = [0.4, 0.6, 1.0];
        l
    }

    pub fn new_eq_low() -> Self {
        let mut l = Self::new("EQ Low", "eq_low_gain", -24.0, 24.0, 0.0);
        l.color = [1.0, 0.4, 0.4];
        l
    }

    pub fn evaluate_automation(&self, beat: f32) -> f32 {
        if self.points.is_empty() { return self.default_value; }
        if beat <= self.points[0].beat { return self.points[0].value; }
        if beat >= self.points.last().unwrap().beat { return self.points.last().unwrap().value; }

        let idx = self.points.partition_point(|p| p.beat <= beat).saturating_sub(1);
        if idx + 1 >= self.points.len() { return self.points.last().unwrap().value; }

        let p0 = &self.points[idx];
        let p1 = &self.points[idx + 1];
        let t = (beat - p0.beat) / (p1.beat - p0.beat).max(0.001);

        match p0.curve_type {
            CurveType::Linear => p0.value + (p1.value - p0.value) * t,
            CurveType::Step => p0.value,
            CurveType::EaseIn => { let t2 = t * t; p0.value + (p1.value - p0.value) * t2 }
            CurveType::EaseOut => { let t2 = 1.0 - (1.0 - t) * (1.0 - t); p0.value + (p1.value - p0.value) * t2 }
            CurveType::EaseInOut => { let t2 = t * t * (3.0 - 2.0 * t); p0.value + (p1.value - p0.value) * t2 }
            CurveType::CubicHermite => {
                let t2 = t * t; let t3 = t2 * t;
                let h00 = 2.0*t3 - 3.0*t2 + 1.0;
                let h10 = t3 - 2.0*t2 + t;
                let h01 = -2.0*t3 + 3.0*t2;
                let h11 = t3 - t2;
                let span = p1.beat - p0.beat;
                let m0 = p0.tension * span;
                let m1 = p0.tension * span;
                h00 * p0.value + h10 * m0 + h01 * p1.value + h11 * m1
            }
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AutomationEditorState {
    pub lanes: Vec<AutomationLane>,
    pub selected_lane: Option<usize>,
    pub selected_point: Option<usize>,
    pub current_beat: f32,
    pub zoom_x: f32,
    pub scroll_x: f32,
    pub total_beats: f32,
    pub show_grid: bool,
    pub show_values: bool,
    pub mode: AutomationEditMode,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum AutomationEditMode {
    #[default]
    Draw,
    Erase,
    Select,
    Smooth,
}

pub fn show_automation_editor(ui: &mut egui::Ui, state: &mut AutomationEditorState) {
    ui.heading("Automation Lanes");
    ui.separator();

    if state.zoom_x == 0.0 { state.zoom_x = 1.0; state.total_beats = 32.0; state.show_grid = true; state.show_values = true; }
    if state.lanes.is_empty() {
        state.lanes = vec![
            AutomationLane::new_volume(),
            AutomationLane::new_pan(),
            AutomationLane::new_eq_low(),
        ];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Volume").clicked() { state.lanes.push(AutomationLane::new_volume()); }
        if ui.button("+ Pan").clicked() { state.lanes.push(AutomationLane::new_pan()); }
        if ui.button("+ EQ Low").clicked() { state.lanes.push(AutomationLane::new_eq_low()); }
        if let Some(s) = state.selected_lane { if ui.button("Delete Lane").clicked() { state.lanes.remove(s); state.selected_lane = None; } }
    });

    ui.horizontal(|ui| {
        for mode in &[AutomationEditMode::Draw, AutomationEditMode::Erase, AutomationEditMode::Select, AutomationEditMode::Smooth] {
            let lbl = match mode { AutomationEditMode::Draw => "Draw", AutomationEditMode::Erase => "Erase", AutomationEditMode::Select => "Select", AutomationEditMode::Smooth => "Smooth" };
            if ui.selectable_label(state.mode == *mode, lbl).clicked() { state.mode = mode.clone(); }
        }
        ui.checkbox(&mut state.show_grid, "Grid");
        ui.checkbox(&mut state.show_values, "Values");
        ui.label("Zoom:");
        ui.add(egui::Slider::new(&mut state.zoom_x, 0.5..=4.0));
    });

    for (i, lane) in state.lanes.iter().enumerate() {
        let sel = state.selected_lane == Some(i);
        let lc = Color32::from_rgb((lane.color[0]*255.0) as u8, (lane.color[1]*255.0) as u8, (lane.color[2]*255.0) as u8);
        ui.horizontal(|ui| {
            ui.colored_label(lc, "▶");
            if ui.selectable_label(sel, format!("{} [{}]", lane.name, lane.parameter)).clicked() { state.selected_lane = Some(i); }
            let cur_val = lane.evaluate_automation(state.current_beat);
            ui.label(egui::RichText::new(format!("{:.3}", cur_val)).color(lc));
            ui.label(format!("{} pts", lane.points.len()));
        });
    }

    if let Some(sel_idx) = state.selected_lane {
        if sel_idx < state.lanes.len() {
            draw_automation_lane_editor(ui, &mut state.lanes[sel_idx], state.current_beat, state.zoom_x, state.scroll_x, state.total_beats, state.show_grid, state.show_values);
        }
    }

    if let Some(lane_idx) = state.selected_lane {
        if let Some(pt_idx) = state.selected_point {
            if lane_idx < state.lanes.len() && pt_idx < state.lanes[lane_idx].points.len() {
                let lane = &mut state.lanes[lane_idx];
                let pt = &mut lane.points[pt_idx];
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("Point #{}", pt_idx));
                    ui.label("Beat:");
                    ui.add(egui::DragValue::new(&mut pt.beat).speed(0.0625));
                    ui.label("Value:");
                    ui.add(egui::DragValue::new(&mut pt.value).speed(0.01).clamp_range(lane.min_value..=lane.max_value));
                });
                ui.horizontal(|ui| {
                    ui.label("Curve:");
                    for ct in &[CurveType::Linear, CurveType::CubicHermite, CurveType::Step, CurveType::EaseIn, CurveType::EaseOut, CurveType::EaseInOut] {
                        if ui.selectable_label(pt.curve_type == *ct, ct.label()).clicked() { pt.curve_type = ct.clone(); }
                    }
                });
            }
        }
    }
}

fn draw_automation_lane_editor(ui: &mut egui::Ui, lane: &mut AutomationLane, current_beat: f32, zoom_x: f32, scroll_x: f32, total_beats: f32, show_grid: bool, show_values: bool) {
    let desired = Vec2::new(ui.available_width().min(700.0), 100.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(14, 16, 22));

    let visible_beats = total_beats / zoom_x;
    let beat_px = rect.width() / visible_beats;
    let val_range = (lane.max_value - lane.min_value).max(0.001);
    let val_to_y = |v: f32| -> f32 { rect.bottom() - (v - lane.min_value) / val_range * rect.height() * 0.9 - rect.height() * 0.05 };
    let beat_to_x = |b: f32| -> f32 { rect.left() + (b - scroll_x) * beat_px };

    // Zero/default line
    let zero_y = val_to_y(lane.default_value);
    p.line_segment([Pos2::new(rect.left(), zero_y), Pos2::new(rect.right(), zero_y)], Stroke::new(0.5, Color32::from_rgb(50,55,65)));

    // Grid
    if show_grid {
        let steps = 8i32;
        for i in 0..=steps {
            let b = scroll_x + i as f32 * visible_beats / steps as f32;
            let x = beat_to_x(b);
            p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(0.4, Color32::from_rgb(35,40,50)));
        }
        for j in 0..=4 {
            let v = lane.min_value + j as f32 * val_range / 4.0;
            let y = val_to_y(v);
            p.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(0.4, Color32::from_rgb(35,40,50)));
        }
    }

    let lc = Color32::from_rgb((lane.color[0]*255.0) as u8, (lane.color[1]*255.0) as u8, (lane.color[2]*255.0) as u8);

    // Draw interpolated curve
    let steps = (rect.width() as usize).min(500);
    let mut last_pos: Option<Pos2> = None;
    for si in 0..=steps {
        let b = scroll_x + si as f32 / steps as f32 * visible_beats;
        let v = lane.evaluate_automation(b);
        let x = beat_to_x(b);
        let y = val_to_y(v);
        if let Some(lp) = last_pos {
            p.line_segment([lp, Pos2::new(x, y)], Stroke::new(1.5, lc));
        }
        last_pos = Some(Pos2::new(x, y));
    }

    // Draw control points
    for (i, pt) in lane.points.iter().enumerate() {
        let px = beat_to_x(pt.beat);
        let py = val_to_y(pt.value);
        if px < rect.left() - 6.0 || px > rect.right() + 6.0 { continue; }
        p.circle_filled(Pos2::new(px, py), 4.0, lc);
        p.circle_stroke(Pos2::new(px, py), 4.0, Stroke::new(1.5, Color32::WHITE));
        if show_values {
            p.text(Pos2::new(px, py - 8.0), egui::Align2::CENTER_BOTTOM, format!("{:.2}", pt.value), FontId::monospace(7.0), lc);
        }
    }

    // Playhead
    let ph_x = beat_to_x(current_beat);
    if ph_x >= rect.left() && ph_x <= rect.right() {
        p.line_segment([Pos2::new(ph_x, rect.top()), Pos2::new(ph_x, rect.bottom())], Stroke::new(1.5, Color32::from_rgb(255,80,80)));
    }

    // Labels
    p.text(Pos2::new(rect.left()+2.0, rect.top()+2.0), egui::Align2::LEFT_TOP, format!("{} [{:.2}, {:.2}]", lane.parameter, lane.min_value, lane.max_value), FontId::monospace(8.0), lc);

    // Click to add/remove points
    if let Some(click_pos) = response.interact_pointer_pos() {
        if response.clicked() {
            let b = scroll_x + (click_pos.x - rect.left()) / beat_px;
            let v = lane.min_value + (rect.bottom() - click_pos.y) / (rect.height() * 0.9) * val_range;
            let v_clamped = v.clamp(lane.min_value, lane.max_value);
            lane.points.push(AutoPoint { beat: b.max(0.0), value: v_clamped, curve_type: CurveType::Linear, tension: 0.0 });
            lane.points.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap_or(std::cmp::Ordering::Equal));
        }
    }
}

// ─── Spectral Analyzer ────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectrumAnalyzer {
    pub bins: Vec<f32>,
    pub peaks: Vec<f32>,
    pub peak_decay: f32,
    pub smoothing: f32,
    pub bin_count: usize,
    pub sample_rate: f32,
    pub show_peaks: bool,
    pub show_waterfall: bool,
    pub waterfall_history: Vec<Vec<f32>>,
    pub waterfall_rows: usize,
    pub display_mode: SpectrumDisplayMode,
    pub scale_mode: SpectrumScaleMode,
    pub floor_db: f32,
    pub band_markers: Vec<(String, f32)>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SpectrumDisplayMode {
    #[default]
    Bars,
    Line,
    Fill,
    Dots,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SpectrumScaleMode {
    #[default]
    Log,
    Linear,
    Mel,
}

impl SpectrumAnalyzer {
    pub fn new(bin_count: usize, sample_rate: f32) -> Self {
        let bins = vec![0.0f32; bin_count];
        let peaks = vec![0.0f32; bin_count];
        let waterfall_rows = 32;
        let band_markers = vec![
            ("Sub".into(), 60.0), ("Bass".into(), 120.0), ("Low-Mid".into(), 500.0),
            ("Mid".into(), 2000.0), ("High-Mid".into(), 6000.0), ("Hi".into(), 12000.0),
            ("Air".into(), 18000.0),
        ];
        Self { bins, peaks, peak_decay: 0.98, smoothing: 0.85, bin_count, sample_rate, show_peaks: true, show_waterfall: false, waterfall_history: vec![vec![0.0; bin_count]; waterfall_rows], waterfall_rows, display_mode: SpectrumDisplayMode::Bars, scale_mode: SpectrumScaleMode::Log, floor_db: -80.0, band_markers }
    }

    pub fn bin_to_freq(&self, bin: usize) -> f32 {
        bin as f32 * self.sample_rate / (self.bin_count * 2) as f32
    }

    pub fn generate_test_spectrum(&mut self) {
        for i in 0..self.bin_count {
            let freq = self.bin_to_freq(i);
            let val = {
                let bass = if freq < 200.0 { (1.0 - freq / 200.0) * 0.8 } else { 0.0 };
                let mid = if freq > 500.0 && freq < 5000.0 { ((freq - 500.0) / 4500.0 * std::f32::consts::PI).sin() * 0.6 } else { 0.0 };
                let high = if freq > 8000.0 { (1.0 - (freq - 8000.0) / 14000.0).max(0.0) * 0.3 } else { 0.0 };
                (bass + mid + high).clamp(0.0, 1.0)
            };
            let noisy = val + (i as f32 * 0.37).sin() * 0.1 + (i as f32 * 1.1).cos() * 0.05;
            self.bins[i] = self.bins[i] * self.smoothing + noisy.clamp(0.0, 1.0) * (1.0 - self.smoothing);
            if self.bins[i] > self.peaks[i] { self.peaks[i] = self.bins[i]; } else { self.peaks[i] *= self.peak_decay; }
        }
    }
}

pub fn show_spectrum_analyzer(ui: &mut egui::Ui, analyzer: &mut SpectrumAnalyzer) {
    ui.heading("Spectrum Analyzer");
    ui.separator();

    ui.horizontal(|ui| {
        for dm in &[SpectrumDisplayMode::Bars, SpectrumDisplayMode::Line, SpectrumDisplayMode::Fill, SpectrumDisplayMode::Dots] {
            let lbl = match dm { SpectrumDisplayMode::Bars => "Bars", SpectrumDisplayMode::Line => "Line", SpectrumDisplayMode::Fill => "Fill", SpectrumDisplayMode::Dots => "Dots" };
            if ui.selectable_label(analyzer.display_mode == *dm, lbl).clicked() { analyzer.display_mode = dm.clone(); }
        }
        ui.separator();
        for sm in &[SpectrumScaleMode::Log, SpectrumScaleMode::Linear, SpectrumScaleMode::Mel] {
            let lbl = match sm { SpectrumScaleMode::Log => "Log", SpectrumScaleMode::Linear => "Linear", SpectrumScaleMode::Mel => "Mel" };
            if ui.selectable_label(analyzer.scale_mode == *sm, lbl).clicked() { analyzer.scale_mode = sm.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut analyzer.show_peaks, "Peak Hold");
        ui.checkbox(&mut analyzer.show_waterfall, "Waterfall");
        ui.label("Smoothing:");
        ui.add(egui::Slider::new(&mut analyzer.smoothing, 0.0..=0.99));
        ui.label("Peak Decay:");
        ui.add(egui::Slider::new(&mut analyzer.peak_decay, 0.9..=0.999));
        ui.label("Floor dB:");
        ui.add(egui::DragValue::new(&mut analyzer.floor_db).speed(1.0).clamp_range(-120.0..=-20.0));
        if ui.button("Test Signal").clicked() { analyzer.generate_test_spectrum(); }
    });

    draw_spectrum_display(ui, analyzer);
    if analyzer.show_waterfall { draw_waterfall_display(ui, analyzer); }
    draw_frequency_band_markers(ui, analyzer);
}

fn draw_spectrum_display(ui: &mut egui::Ui, analyzer: &SpectrumAnalyzer) {
    let desired = Vec2::new(ui.available_width().min(800.0), 180.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(8, 10, 14));

    let n = analyzer.bin_count;
    if n == 0 { return; }

    let bin_to_x = |bin: usize| -> f32 {
        match analyzer.scale_mode {
            SpectrumScaleMode::Linear => rect.left() + bin as f32 / n as f32 * rect.width(),
            SpectrumScaleMode::Log => {
                let freq = analyzer.bin_to_freq(bin.max(1));
                let log_pos = (freq.log2() - 4.0) / (analyzer.sample_rate.log2() / 2.0 - 4.0);
                rect.left() + log_pos.clamp(0.0, 1.0) * rect.width()
            }
            SpectrumScaleMode::Mel => {
                let freq = analyzer.bin_to_freq(bin.max(1));
                let mel = 1127.0 * (1.0 + freq / 700.0).ln();
                let mel_max = 1127.0 * (1.0 + analyzer.sample_rate / 2.0 / 700.0).ln();
                rect.left() + (mel / mel_max).clamp(0.0, 1.0) * rect.width()
            }
        }
    };

    // dB grid lines
    for db_step in &[-60.0f32, -40.0, -20.0, -10.0, -6.0, -3.0, 0.0] {
        let norm = (db_step - analyzer.floor_db) / (-analyzer.floor_db);
        let y = rect.bottom() - norm.clamp(0.0, 1.0) * rect.height();
        p.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(0.5, Color32::from_rgb(30,36,44)));
        p.text(Pos2::new(rect.left()+2.0, y+1.0), egui::Align2::LEFT_TOP, format!("{}dB", db_step), FontId::monospace(6.5), Color32::from_rgb(50,60,70));
    }

    match analyzer.display_mode {
        SpectrumDisplayMode::Bars => {
            for i in 0..n.saturating_sub(1) {
                let x0 = bin_to_x(i);
                let x1 = bin_to_x(i + 1);
                let bw = (x1 - x0).max(1.0);
                let val = analyzer.bins[i].clamp(0.0, 1.0);
                let bh = val * rect.height() * 0.95;
                let freq = analyzer.bin_to_freq(i);
                let hue = (freq.log2() - 4.0) / 10.0;
                let col = hsl_to_color32(hue, 0.8, 0.4 + val * 0.3);
                p.rect_filled(Rect::from_min_size(Pos2::new(x0, rect.bottom()-bh), Vec2::new(bw.max(1.0)-0.5, bh)), 0.0, col);
                if analyzer.show_peaks && analyzer.peaks[i] > 0.01 {
                    let ph = analyzer.peaks[i].clamp(0.0,1.0) * rect.height() * 0.95;
                    let py = rect.bottom() - ph;
                    p.line_segment([Pos2::new(x0, py), Pos2::new(x0+bw, py)], Stroke::new(1.0, Color32::from_rgb(255, 200, 60)));
                }
            }
        }
        SpectrumDisplayMode::Line | SpectrumDisplayMode::Fill => {
            let mut last: Option<Pos2> = None;
            let mut pts: Vec<Pos2> = Vec::new();
            for i in 0..n {
                let x = bin_to_x(i);
                let val = analyzer.bins[i].clamp(0.0, 1.0);
                let y = rect.bottom() - val * rect.height() * 0.95;
                let pos = Pos2::new(x, y);
                if let Some(lp) = last {
                    p.line_segment([lp, pos], Stroke::new(1.5, Color32::from_rgb(100, 200, 255)));
                }
                last = Some(pos);
                pts.push(pos);
            }
            if analyzer.display_mode == SpectrumDisplayMode::Fill && pts.len() > 1 {
                let mut fill_pts = pts.clone();
                fill_pts.push(Pos2::new(rect.right(), rect.bottom()));
                fill_pts.push(Pos2::new(rect.left(), rect.bottom()));
                p.add(egui::Shape::convex_polygon(fill_pts, Color32::from_rgba_unmultiplied(40, 100, 200, 60), Stroke::NONE));
            }
        }
        SpectrumDisplayMode::Dots => {
            for i in 0..n {
                let x = bin_to_x(i);
                let val = analyzer.bins[i].clamp(0.0, 1.0);
                let y = rect.bottom() - val * rect.height() * 0.95;
                p.circle_filled(Pos2::new(x, y), 1.5, Color32::from_rgb(100, 220, 255));
            }
        }
    }

    // Band markers
    for (name, freq) in &analyzer.band_markers {
        let bin = (*freq * (n * 2) as f32 / analyzer.sample_rate) as usize;
        if bin < n {
            let x = bin_to_x(bin);
            p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(0.8, Color32::from_rgba_unmultiplied(200, 200, 200, 50)));
            p.text(Pos2::new(x+2.0, rect.top()+2.0), egui::Align2::LEFT_TOP, name, FontId::monospace(6.5), Color32::from_rgba_unmultiplied(180, 180, 180, 150));
        }
    }
}

fn hsl_to_color32(h: f32, s: f32, l: f32) -> Color32 {
    let h = h.fract() * 6.0;
    let c = (1.0 - (2.0*l - 1.0).abs()) * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = if h < 1.0 { (c, x, 0.0) } else if h < 2.0 { (x, c, 0.0) } else if h < 3.0 { (0.0, c, x) } else if h < 4.0 { (0.0, x, c) } else if h < 5.0 { (x, 0.0, c) } else { (c, 0.0, x) };
    let m = l - c * 0.5;
    Color32::from_rgb(((r+m)*255.0) as u8, ((g+m)*255.0) as u8, ((b+m)*255.0) as u8)
}

fn draw_waterfall_display(ui: &mut egui::Ui, analyzer: &SpectrumAnalyzer) {
    let desired = Vec2::new(ui.available_width().min(800.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(6, 8, 12));

    let rows = analyzer.waterfall_history.len().min(analyzer.waterfall_rows);
    let row_h = rect.height() / rows as f32;
    let n = analyzer.bin_count;

    for (ri, row_data) in analyzer.waterfall_history.iter().enumerate().take(rows) {
        let y = rect.top() + ri as f32 * row_h;
        let col_w = rect.width() / n as f32;
        for (bi, &val) in row_data.iter().enumerate() {
            let x = rect.left() + bi as f32 * col_w;
            let t = val.clamp(0.0, 1.0);
            let col = egui::lerp(egui::Rgba::from(Color32::from_rgb(0,0,40))..=egui::Rgba::from(Color32::from_rgb(100,200,255)), t);
            p.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(col_w+0.5, row_h+0.5)), 0.0, Color32::from(col));
        }
    }
    p.text(Pos2::new(rect.left()+4.0, rect.top()+2.0), egui::Align2::LEFT_TOP, "Waterfall", FontId::monospace(7.0), Color32::GRAY);
}

fn draw_frequency_band_markers(ui: &mut egui::Ui, analyzer: &SpectrumAnalyzer) {
    let desired = Vec2::new(ui.available_width().min(800.0), 22.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, Color32::from_rgb(10, 12, 18));

    let n = analyzer.bin_count;
    for (i, (name, freq)) in analyzer.band_markers.iter().enumerate() {
        let bin = (*freq * (n * 2) as f32 / analyzer.sample_rate) as usize;
        if bin < n {
            let x = rect.left() + bin as f32 / n as f32 * rect.width();
            let colors = [Color32::from_rgb(80,120,220), Color32::from_rgb(80,180,120), Color32::from_rgb(220,160,60), Color32::from_rgb(220,80,80), Color32::from_rgb(160,80,220), Color32::from_rgb(60,200,200), Color32::from_rgb(200,200,60)];
            let col = colors[i % colors.len()];
            p.rect_filled(Rect::from_min_size(Pos2::new(x, rect.top()+2.0), Vec2::new(40.0, rect.height()-4.0)), 2.0, Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 60));
            p.text(Pos2::new(x+4.0, rect.center().y), egui::Align2::LEFT_CENTER, format!("{} {:.0}Hz", name, freq), FontId::monospace(7.0), col);
        }
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
