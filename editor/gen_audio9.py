path = r"C:\proof-engine\editor\src\audio_mixer.rs"

addon = r"""

// ============================================================
// AUDIO MIXER EXPANSION BLOCK 9
// ============================================================

// --- Pitch Shifter & Harmonizer --------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum PitchShiftAlgorithm {
    PhaseVocoder,
    Wsola,
    PaulStretch,
    PitchSynchronous,
    Formant,
}

impl PitchShiftAlgorithm {
    pub fn label(&self) -> &str {
        match self {
            PitchShiftAlgorithm::PhaseVocoder => "Phase Vocoder",
            PitchShiftAlgorithm::Wsola => "WSOLA",
            PitchShiftAlgorithm::PaulStretch => "PaulStretch",
            PitchShiftAlgorithm::PitchSynchronous => "Pitch Sync",
            PitchShiftAlgorithm::Formant => "Formant Preserve",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HarmonizerVoice {
    pub enabled: bool,
    pub interval_st: i8,
    pub volume_db: f32,
    pub pan: f32,
    pub detune_cents: f32,
    pub delay_ms: f32,
    pub formant_shift: f32,
    pub follow_scale: bool,
}

impl HarmonizerVoice {
    pub fn new(interval: i8) -> Self {
        Self {
            enabled: true,
            interval_st: interval,
            volume_db: -6.0,
            pan: 0.0,
            detune_cents: 0.0,
            delay_ms: 0.0,
            formant_shift: 0.0,
            follow_scale: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PitchShifterState {
    pub algorithm: PitchShiftAlgorithm,
    pub shift_semitones: f32,
    pub shift_cents: f32,
    pub formant_preserve: bool,
    pub formant_shift: f32,
    pub time_stretch: f32,
    pub window_ms: f32,
    pub hop_size: f32,
    pub enabled: bool,
    pub wet_db: f32,
    pub dry_db: f32,
    pub voices: Vec<HarmonizerVoice>,
    pub root_note: u8,
    pub scale: MusicalScale,
}

impl Default for PitchShifterState {
    fn default() -> Self {
        Self {
            algorithm: PitchShiftAlgorithm::PhaseVocoder,
            shift_semitones: 0.0,
            shift_cents: 0.0,
            formant_preserve: true,
            formant_shift: 0.0,
            time_stretch: 1.0,
            window_ms: 23.2,
            hop_size: 0.25,
            enabled: true,
            wet_db: 0.0,
            dry_db: 0.0,
            voices: vec![HarmonizerVoice::new(4), HarmonizerVoice::new(7), HarmonizerVoice::new(12)],
            root_note: 0,
            scale: MusicalScale::Major,
        }
    }
}

pub fn show_pitch_shifter(ui: &mut egui::Ui, state: &mut PitchShifterState) {
    ui.heading("Pitch Shifter & Harmonizer");
    ui.separator();

    ui.horizontal(|ui| {
        for algo in &[PitchShiftAlgorithm::PhaseVocoder, PitchShiftAlgorithm::Wsola,
                       PitchShiftAlgorithm::PaulStretch, PitchShiftAlgorithm::PitchSynchronous,
                       PitchShiftAlgorithm::Formant] {
            if ui.selectable_label(state.algorithm == *algo, algo.label()).clicked() { state.algorithm = algo.clone(); }
        }
        ui.separator();
        ui.checkbox(&mut state.enabled, "Enable");
    });
    ui.separator();

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new("Pitch Shift").strong());
        ui.horizontal(|ui| {
            ui.label("Semitones:");
            ui.add(egui::Slider::new(&mut state.shift_semitones, -24.0..=24.0));
        });
        ui.horizontal(|ui| {
            ui.label("Fine (cents):");
            ui.add(egui::Slider::new(&mut state.shift_cents, -100.0..=100.0));
        });
        ui.checkbox(&mut state.formant_preserve, "Preserve Formants");
        if state.formant_preserve {
            ui.horizontal(|ui| { ui.label("Formant Shift:"); ui.add(egui::Slider::new(&mut state.formant_shift, -6.0..=6.0).suffix(" st")); });
        }
        ui.horizontal(|ui| { ui.label("Time Stretch:"); ui.add(egui::Slider::new(&mut state.time_stretch, 0.25..=4.0).logarithmic(true)); });
        ui.horizontal(|ui| { ui.label("Window:"); ui.add(egui::DragValue::new(&mut state.window_ms).speed(0.5).clamp_range(1.0..=100.0).suffix(" ms")); });
        ui.horizontal(|ui| { ui.label("Hop:"); ui.add(egui::Slider::new(&mut state.hop_size, 0.0625..=0.5)); });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Wet:"); ui.add(egui::Slider::new(&mut state.wet_db, -60.0..=6.0).suffix(" dB"));
        });
        ui.horizontal(|ui| {
            ui.label("Dry:"); ui.add(egui::Slider::new(&mut state.dry_db, -60.0..=6.0).suffix(" dB"));
        });

        let ui = &mut cols[1];
        ui.label(egui::RichText::new("Harmonizer Voices").strong());
        ui.horizontal(|ui| {
            ui.label("Scale Root:");
            for (i, name) in NOTE_NAMES.iter().enumerate() {
                if ui.selectable_label(state.root_note == i as u8, *name).clicked() { state.root_note = i as u8; }
            }
        });
        if ui.button("+ Voice").clicked() {
            state.voices.push(HarmonizerVoice::new(3));
        }
        let mut remove = None;
        for (i, voice) in state.voices.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut voice.enabled, "");
                ui.label("Int:");
                ui.add(egui::DragValue::new(&mut voice.interval_st).clamp_range(-24i8..=24).suffix(" st"));
                ui.label("Vol:");
                ui.add(egui::DragValue::new(&mut voice.volume_db).speed(0.5).clamp_range(-60.0..=6.0).suffix(" dB"));
                ui.label("Pan:");
                ui.add(egui::DragValue::new(&mut voice.pan).speed(0.01).clamp_range(-1.0..=1.0));
                ui.checkbox(&mut voice.follow_scale, "Scale");
                if ui.small_button("✗").clicked() { remove = Some(i); }
            });
        }
        if let Some(ri) = remove { state.voices.remove(ri); }
    });
}

// --- Crossfader/DJ Panel ---------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum CrossfaderCurve {
    Linear,
    EqualPower,
    Scratch,
    Cut,
    Custom,
}

impl CrossfaderCurve {
    pub fn label(&self) -> &str {
        match self {
            CrossfaderCurve::Linear => "Linear",
            CrossfaderCurve::EqualPower => "Equal Power",
            CrossfaderCurve::Scratch => "Scratch",
            CrossfaderCurve::Cut => "Cut",
            CrossfaderCurve::Custom => "Custom",
        }
    }
    pub fn compute(&self, t: f32) -> (f32, f32) {
        let tc = t.clamp(0.0, 1.0);
        match self {
            CrossfaderCurve::Linear => (1.0 - tc, tc),
            CrossfaderCurve::EqualPower => {
                let angle = tc * std::f32::consts::FRAC_PI_2;
                (angle.cos(), angle.sin())
            }
            CrossfaderCurve::Cut => {
                (if tc < 0.5 { 1.0 } else { 0.0 }, if tc > 0.5 { 1.0 } else { 0.0 })
            }
            CrossfaderCurve::Scratch => {
                let a = if tc < 0.5 { 1.0 } else { 2.0 * (1.0 - tc) };
                let b = if tc > 0.5 { 1.0 } else { 2.0 * tc };
                (a, b)
            }
            CrossfaderCurve::Custom => { let t2 = tc * tc; (1.0 - t2, t2) }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DJDeckState {
    pub bpm: f32,
    pub position: f32,
    pub playing: bool,
    pub pitch_st: f32,
    pub tempo_sync: bool,
    pub loop_active: bool,
    pub loop_start: f32,
    pub loop_end: f32,
    pub cue_point: f32,
    pub hot_cues: [f32; 8],
    pub volume_db: f32,
    pub eq_low_db: f32,
    pub eq_mid_db: f32,
    pub eq_high_db: f32,
    pub filter_freq: f32,
    pub filter_resonance: f32,
    pub filter_enabled: bool,
    pub reverb_send: f32,
    pub delay_send: f32,
    pub clip_name: String,
    pub keylock: bool,
    pub reverse: bool,
    pub slip_mode: bool,
}

impl Default for DJDeckState {
    fn default() -> Self {
        Self {
            bpm: 128.0,
            position: 0.0,
            playing: false,
            pitch_st: 0.0,
            tempo_sync: false,
            loop_active: false,
            loop_start: 0.25,
            loop_end: 0.5,
            cue_point: 0.0,
            hot_cues: [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875],
            volume_db: 0.0,
            eq_low_db: 0.0,
            eq_mid_db: 0.0,
            eq_high_db: 0.0,
            filter_freq: 1000.0,
            filter_resonance: 0.7,
            filter_enabled: false,
            reverb_send: 0.0,
            delay_send: 0.0,
            clip_name: "track.mp3".into(),
            keylock: false,
            reverse: false,
            slip_mode: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DJMixerState {
    pub deck_a: DJDeckState,
    pub deck_b: DJDeckState,
    pub crossfader: f32,
    pub crossfader_curve: CrossfaderCurve,
    pub master_volume_db: f32,
    pub headphone_cue_deck: u8,
    pub headphone_mix: f32,
    pub headphone_volume_db: f32,
    pub bpm_sync_enabled: bool,
    pub quantize: bool,
    pub sync_bpm: f32,
}

impl Default for DJMixerState {
    fn default() -> Self {
        Self {
            deck_a: DJDeckState::default(),
            deck_b: DJDeckState::default(),
            crossfader: 0.5,
            crossfader_curve: CrossfaderCurve::EqualPower,
            master_volume_db: 0.0,
            headphone_cue_deck: 0,
            headphone_mix: 0.5,
            headphone_volume_db: -6.0,
            bpm_sync_enabled: false,
            quantize: false,
            sync_bpm: 128.0,
        }
    }
}

pub fn show_dj_mixer(ui: &mut egui::Ui, state: &mut DJMixerState) {
    ui.heading("DJ Mixer");
    ui.separator();

    ui.columns(3, |cols| {
        show_dj_deck(&mut cols[0], &mut state.deck_a, "Deck A");
        let ui = &mut cols[1];
        ui.label(egui::RichText::new("Master").strong().color(Color32::from_rgb(255, 220, 60)));
        ui.separator();
        ui.label("Crossfader:");
        ui.add(egui::Slider::new(&mut state.crossfader, 0.0..=1.0));
        ui.label("Curve:");
        for curve in &[CrossfaderCurve::Linear, CrossfaderCurve::EqualPower, CrossfaderCurve::Scratch, CrossfaderCurve::Cut] {
            if ui.selectable_label(state.crossfader_curve == *curve, curve.label()).clicked() {
                state.crossfader_curve = curve.clone();
            }
        }
        draw_crossfader_curve(ui, &state.crossfader_curve, state.crossfader);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Master:");
            ui.add(egui::Slider::new(&mut state.master_volume_db, -30.0..=12.0).suffix(" dB").vertical());
        });
        ui.separator();
        ui.label("Headphones:");
        ui.add(egui::Slider::new(&mut state.headphone_volume_db, -30.0..=0.0).suffix(" dB"));
        ui.label("Cue Mix:");
        ui.add(egui::Slider::new(&mut state.headphone_mix, 0.0..=1.0));
        ui.separator();
        ui.checkbox(&mut state.bpm_sync_enabled, "BPM Sync");
        ui.checkbox(&mut state.quantize, "Quantize");
        if state.bpm_sync_enabled {
            ui.label(format!("Sync BPM: {:.1}", state.sync_bpm));
        }
        show_dj_deck(&mut cols[2], &mut state.deck_b, "Deck B");
    });
}

pub fn show_dj_deck(ui: &mut egui::Ui, deck: &mut DJDeckState, label: &str) {
    ui.label(egui::RichText::new(label).strong().color(Color32::from_rgb(100, 200, 255)));
    ui.label(egui::RichText::new(&deck.clip_name).small().color(Color32::GRAY));
    ui.horizontal(|ui| {
        if ui.button(if deck.playing { "⏸" } else { "▶" }).clicked() { deck.playing = !deck.playing; }
        if ui.button("CUE").clicked() { deck.cue_point = deck.position; }
        ui.checkbox(&mut deck.keylock, "Key");
        ui.checkbox(&mut deck.reverse, "Rev");
        ui.checkbox(&mut deck.slip_mode, "Slip");
    });
    ui.label(format!("BPM: {:.2}", deck.bpm));
    ui.add(egui::Slider::new(&mut deck.pitch_st, -12.0..=12.0).suffix(" st"));
    ui.add(egui::ProgressBar::new(deck.position).desired_width(160.0).show_percentage(true));

    // Loop
    ui.horizontal(|ui| {
        ui.checkbox(&mut deck.loop_active, "Loop");
        if deck.loop_active {
            ui.add(egui::DragValue::new(&mut deck.loop_start).speed(0.01).clamp_range(0.0..=1.0));
            ui.add(egui::DragValue::new(&mut deck.loop_end).speed(0.01).clamp_range(0.0..=1.0));
        }
    });

    // Hot cues
    ui.horizontal_wrapped(|ui| {
        for (i, cue) in deck.hot_cues.iter_mut().enumerate() {
            if ui.button(format!("H{}", i + 1)).clicked() { *cue = deck.position; }
        }
    });

    // EQ
    ui.label("EQ:");
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut deck.eq_low_db, -24.0..=6.0).vertical().text("Lo"));
        ui.add(egui::Slider::new(&mut deck.eq_mid_db, -24.0..=6.0).vertical().text("Mid"));
        ui.add(egui::Slider::new(&mut deck.eq_high_db, -24.0..=6.0).vertical().text("Hi"));
        ui.add(egui::Slider::new(&mut deck.volume_db, -30.0..=6.0).vertical().text("Vol"));
    });

    // Filter
    ui.horizontal(|ui| {
        ui.checkbox(&mut deck.filter_enabled, "Filter");
        if deck.filter_enabled {
            ui.add(egui::Slider::new(&mut deck.filter_freq, 20.0..=20000.0).logarithmic(true).suffix(" Hz"));
            ui.add(egui::DragValue::new(&mut deck.filter_resonance).speed(0.01).clamp_range(0.1..=20.0).prefix("Q: "));
        }
    });

    // FX sends
    ui.horizontal(|ui| {
        ui.label("Reverb:");
        ui.add(egui::Slider::new(&mut deck.reverb_send, 0.0..=1.0));
    });
    ui.horizontal(|ui| {
        ui.label("Delay:");
        ui.add(egui::Slider::new(&mut deck.delay_send, 0.0..=1.0));
    });
}

pub fn draw_crossfader_curve(ui: &mut egui::Ui, curve: &CrossfaderCurve, current: f32) {
    let size = Vec2::new(140.0, 60.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(10, 12, 18));
    p.rect_stroke(rect, 3.0, Stroke::new(0.5, Color32::from_rgb(40, 50, 65)));

    let n = 60usize;
    let pts_a: Vec<Pos2> = (0..n).map(|i| {
        let t = i as f32 / (n - 1) as f32;
        let (ga, _) = curve.compute(t);
        Pos2::new(rect.left() + t * rect.width(), rect.bottom() - ga * rect.height() * 0.9)
    }).collect();
    let pts_b: Vec<Pos2> = (0..n).map(|i| {
        let t = i as f32 / (n - 1) as f32;
        let (_, gb) = curve.compute(t);
        Pos2::new(rect.left() + t * rect.width(), rect.bottom() - gb * rect.height() * 0.9)
    }).collect();
    for w in pts_a.windows(2) { p.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(80, 140, 255))); }
    for w in pts_b.windows(2) { p.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(255, 100, 80))); }

    let cx = rect.left() + current * rect.width();
    p.line_segment([Pos2::new(cx, rect.top()), Pos2::new(cx, rect.bottom())],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 220, 60, 180)));
}

// --- Noise Generator & Test Tones ------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum TestToneType {
    Sine,
    Square,
    Triangle,
    Sawtooth,
    WhiteNoise,
    PinkNoise,
    BrownNoise,
    Silence,
    SweepUp,
    SweepDown,
    Dirac,
    Comb,
}

impl TestToneType {
    pub fn label(&self) -> &str {
        match self {
            TestToneType::Sine => "Sine",
            TestToneType::Square => "Square",
            TestToneType::Triangle => "Triangle",
            TestToneType::Sawtooth => "Sawtooth",
            TestToneType::WhiteNoise => "White",
            TestToneType::PinkNoise => "Pink",
            TestToneType::BrownNoise => "Brown",
            TestToneType::Silence => "Silence",
            TestToneType::SweepUp => "Sweep Up",
            TestToneType::SweepDown => "Sweep Dn",
            TestToneType::Dirac => "Dirac",
            TestToneType::Comb => "Comb",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TestToneState {
    pub tone_type: TestToneType,
    pub frequency: f32,
    pub frequency_end: f32,
    pub amplitude_db: f32,
    pub duration_s: f32,
    pub sweep_log: bool,
    pub playing: bool,
    pub loop_tone: bool,
    pub output_bus: usize,
    pub comb_delay_ms: f32,
    pub comb_feedback: f32,
}

impl Default for TestToneState {
    fn default() -> Self {
        Self {
            tone_type: TestToneType::Sine,
            frequency: 440.0,
            frequency_end: 20000.0,
            amplitude_db: -12.0,
            duration_s: 1.0,
            sweep_log: true,
            playing: false,
            loop_tone: false,
            output_bus: 0,
            comb_delay_ms: 5.0,
            comb_feedback: 0.7,
        }
    }
}

pub fn show_test_tone_generator(ui: &mut egui::Ui, state: &mut TestToneState) {
    ui.heading("Test Tone Generator");
    ui.separator();

    ui.horizontal_wrapped(|ui| {
        for t in &[TestToneType::Sine, TestToneType::Square, TestToneType::Triangle, TestToneType::Sawtooth,
                    TestToneType::WhiteNoise, TestToneType::PinkNoise, TestToneType::BrownNoise,
                    TestToneType::Silence, TestToneType::SweepUp, TestToneType::SweepDown,
                    TestToneType::Dirac, TestToneType::Comb] {
            if ui.selectable_label(state.tone_type == *t, t.label()).clicked() { state.tone_type = t.clone(); }
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        if !matches!(state.tone_type, TestToneType::WhiteNoise | TestToneType::PinkNoise | TestToneType::BrownNoise | TestToneType::Silence | TestToneType::Dirac) {
            ui.label("Frequency:");
            ui.add(egui::DragValue::new(&mut state.frequency).speed(1.0).clamp_range(1.0..=24000.0).suffix(" Hz"));
        }
        if matches!(state.tone_type, TestToneType::SweepUp | TestToneType::SweepDown) {
            ui.label("End:");
            ui.add(egui::DragValue::new(&mut state.frequency_end).speed(10.0).clamp_range(10.0..=24000.0).suffix(" Hz"));
            ui.checkbox(&mut state.sweep_log, "Log Scale");
        }
        if state.tone_type == TestToneType::Comb {
            ui.label("Delay:");
            ui.add(egui::DragValue::new(&mut state.comb_delay_ms).speed(0.1).clamp_range(0.1..=100.0).suffix(" ms"));
            ui.label("FB:");
            ui.add(egui::DragValue::new(&mut state.comb_feedback).speed(0.01).clamp_range(-0.99..=0.99));
        }
    });
    ui.horizontal(|ui| {
        ui.label("Level:");
        ui.add(egui::Slider::new(&mut state.amplitude_db, -60.0..=0.0).suffix(" dB"));
        ui.label("Duration:");
        ui.add(egui::DragValue::new(&mut state.duration_s).speed(0.1).clamp_range(0.01..=60.0).suffix(" s"));
        ui.checkbox(&mut state.loop_tone, "Loop");
        ui.label("Bus:");
        ui.add(egui::DragValue::new(&mut state.output_bus).clamp_range(0usize..=8));
    });
    ui.horizontal(|ui| {
        if ui.button(if state.playing { "⏸ Stop" } else { "▶ Play" }).clicked() { state.playing = !state.playing; }
        if ui.button("Level Match (-14 LUFS)").clicked() { state.amplitude_db = -20.0; }
        if ui.button("0 dBFS Ceiling Test").clicked() { state.amplitude_db = 0.0; }
    });

    // Quick presets
    ui.separator();
    ui.label(egui::RichText::new("Quick Presets").strong());
    ui.horizontal_wrapped(|ui| {
        let presets: &[(&str, TestToneType, f32)] = &[
            ("A 440", TestToneType::Sine, 440.0),
            ("1kHz", TestToneType::Sine, 1000.0),
            ("10kHz", TestToneType::Sine, 10000.0),
            ("50Hz Thump", TestToneType::Sine, 50.0),
            ("Noise Floor", TestToneType::WhiteNoise, 0.0),
        ];
        for (name, tt, freq) in presets {
            if ui.button(*name).clicked() {
                state.tone_type = tt.clone();
                state.frequency = *freq;
            }
        }
    });
}

// --- Room Acoustic Simulator -----------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct RoomAcousticParams {
    pub width_m: f32,
    pub depth_m: f32,
    pub height_m: f32,
    pub wall_absorption: f32,
    pub floor_absorption: f32,
    pub ceiling_absorption: f32,
    pub air_absorption: bool,
    pub temperature_c: f32,
    pub humidity_percent: f32,
    pub listener_x: f32,
    pub listener_y: f32,
    pub source_x: f32,
    pub source_y: f32,
    pub show_ray_tracing: bool,
    pub num_rays: u32,
    pub max_reflections: u8,
}

impl Default for RoomAcousticParams {
    fn default() -> Self {
        Self {
            width_m: 10.0,
            depth_m: 8.0,
            height_m: 3.0,
            wall_absorption: 0.2,
            floor_absorption: 0.4,
            ceiling_absorption: 0.15,
            air_absorption: true,
            temperature_c: 20.0,
            humidity_percent: 50.0,
            listener_x: 5.0,
            listener_y: 4.0,
            source_x: 2.0,
            source_y: 4.0,
            show_ray_tracing: true,
            num_rays: 32,
            max_reflections: 4,
        }
    }
}

impl RoomAcousticParams {
    pub fn speed_of_sound(&self) -> f32 {
        331.3 * (1.0 + self.temperature_c / 273.15).sqrt()
    }

    pub fn rt60_sabine(&self) -> f32 {
        let v = self.width_m * self.depth_m * self.height_m;
        let s = 2.0 * (self.width_m * self.depth_m + self.width_m * self.height_m + self.depth_m * self.height_m);
        let avg_alpha = (self.wall_absorption * 4.0 + self.floor_absorption + self.ceiling_absorption) / 6.0;
        0.161 * v / (s * avg_alpha).max(0.001)
    }

    pub fn room_modes_hz(&self, n_x: u32, n_y: u32, n_z: u32) -> f32 {
        let c = self.speed_of_sound();
        let fx = n_x as f32 / (2.0 * self.width_m);
        let fy = n_y as f32 / (2.0 * self.depth_m);
        let fz = n_z as f32 / (2.0 * self.height_m);
        c * (fx * fx + fy * fy + fz * fz).sqrt()
    }
}

pub fn show_room_acoustic_simulator(ui: &mut egui::Ui, state: &mut RoomAcousticParams) {
    ui.heading("Room Acoustic Simulator");
    ui.separator();

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new("Room Geometry").strong());
        ui.horizontal(|ui| { ui.label("Width:"); ui.add(egui::DragValue::new(&mut state.width_m).speed(0.1).clamp_range(1.0..=100.0).suffix(" m")); });
        ui.horizontal(|ui| { ui.label("Depth:"); ui.add(egui::DragValue::new(&mut state.depth_m).speed(0.1).clamp_range(1.0..=100.0).suffix(" m")); });
        ui.horizontal(|ui| { ui.label("Height:"); ui.add(egui::DragValue::new(&mut state.height_m).speed(0.05).clamp_range(1.0..=20.0).suffix(" m")); });
        ui.separator();
        ui.label(egui::RichText::new("Absorption Coefficients").strong());
        ui.horizontal(|ui| { ui.label("Walls:"); ui.add(egui::Slider::new(&mut state.wall_absorption, 0.01..=0.99)); });
        ui.horizontal(|ui| { ui.label("Floor:"); ui.add(egui::Slider::new(&mut state.floor_absorption, 0.01..=0.99)); });
        ui.horizontal(|ui| { ui.label("Ceiling:"); ui.add(egui::Slider::new(&mut state.ceiling_absorption, 0.01..=0.99)); });
        ui.separator();
        ui.label(egui::RichText::new("Air Properties").strong());
        ui.checkbox(&mut state.air_absorption, "Air Absorption");
        ui.horizontal(|ui| { ui.label("Temp:"); ui.add(egui::DragValue::new(&mut state.temperature_c).speed(0.5).clamp_range(-20.0..=50.0).suffix(" °C")); });
        ui.horizontal(|ui| { ui.label("Humidity:"); ui.add(egui::Slider::new(&mut state.humidity_percent, 0.0..=100.0).suffix("%")); });
        ui.separator();
        ui.label(egui::RichText::new("Derived Metrics").strong());
        egui::Grid::new("room_metrics").num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
            ui.label("Speed of Sound:"); ui.label(format!("{:.1} m/s", state.speed_of_sound())); ui.end_row();
            ui.label("RT60 (Sabine):"); ui.label(format!("{:.2} s", state.rt60_sabine())); ui.end_row();
            ui.label("Volume:"); ui.label(format!("{:.0} m³", state.width_m * state.depth_m * state.height_m)); ui.end_row();
            ui.label("Surface Area:"); ui.label(format!("{:.0} m²", 2.0 * (state.width_m * state.depth_m + state.width_m * state.height_m + state.depth_m * state.height_m))); ui.end_row();
        });

        ui.separator();
        ui.label(egui::RichText::new("Room Modes").strong());
        ui.label(egui::RichText::new("Axial, tangential, oblique modes:").small().color(Color32::GRAY));
        for nx in 0u32..=3 {
            for ny in 0u32..=2 {
                for nz in 0u32..=1 {
                    if nx + ny + nz == 0 { continue; }
                    let f = state.room_modes_hz(nx, ny, nz);
                    if f > 20.0 && f < 300.0 {
                        ui.label(egui::RichText::new(format!("({},{},{}) = {:.1} Hz", nx, ny, nz, f)).small().monospace().color(Color32::from_rgb(100, 200, 255)));
                    }
                }
            }
        }

        let ui = &mut cols[1];
        ui.checkbox(&mut state.show_ray_tracing, "Ray Tracing");
        ui.horizontal(|ui| {
            ui.label("Rays:");
            ui.add(egui::DragValue::new(&mut state.num_rays).clamp_range(4u32..=128));
            ui.label("Bounces:");
            ui.add(egui::DragValue::new(&mut state.max_reflections).clamp_range(1u8..=8));
        });
        ui.horizontal(|ui| {
            ui.label("Src:");
            ui.add(egui::DragValue::new(&mut state.source_x).speed(0.1).clamp_range(0.0..=state.width_m).prefix("X: "));
            ui.add(egui::DragValue::new(&mut state.source_y).speed(0.1).clamp_range(0.0..=state.depth_m).prefix("Y: "));
        });
        ui.horizontal(|ui| {
            ui.label("Lst:");
            ui.add(egui::DragValue::new(&mut state.listener_x).speed(0.1).clamp_range(0.0..=state.width_m).prefix("X: "));
            ui.add(egui::DragValue::new(&mut state.listener_y).speed(0.1).clamp_range(0.0..=state.depth_m).prefix("Y: "));
        });
        draw_room_top_view(ui, state);
    });
}

pub fn draw_room_top_view(ui: &mut egui::Ui, state: &RoomAcousticParams) {
    let size = Vec2::new(200.0, 200.0 * state.depth_m / state.width_m.max(1.0));
    let size = Vec2::new(size.x.min(250.0), size.y.min(250.0));
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));
    p.rect_stroke(rect, 4.0, Stroke::new(2.0, Color32::from_rgb(80, 100, 130)));

    let to_screen = |wx: f32, wy: f32| -> Pos2 {
        Pos2::new(
            rect.left() + wx / state.width_m * rect.width(),
            rect.bottom() - wy / state.depth_m * rect.height(),
        )
    };

    let src = to_screen(state.source_x, state.source_y);
    let lst = to_screen(state.listener_x, state.listener_y);

    // Direct path
    p.line_segment([src, lst], Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 220, 60, 150)));

    // Ray tracing
    if state.show_ray_tracing {
        let n = state.num_rays.min(64) as usize;
        for ri in 0..n {
            let angle = ri as f32 / n as f32 * std::f32::consts::TAU;
            let mut dx = angle.cos();
            let mut dy = angle.sin();
            let mut x = state.source_x;
            let mut y = state.source_y;
            let mut prev = src;
            let alpha = (120u8 / state.max_reflections.max(1) as u8).max(20);
            let col = Color32::from_rgba_premultiplied(80, 160, 200, alpha);
            for _ in 0..state.max_reflections {
                // Find intersection
                let mut t_min = f32::MAX;
                let mut nx_new = dx;
                let mut ny_new = dy;
                if dx > 0.0 { let t = (state.width_m - x) / dx; if t < t_min { t_min = t; nx_new = -dx; ny_new = dy; } }
                if dx < 0.0 { let t = -x / dx; if t < t_min { t_min = t; nx_new = -dx; ny_new = dy; } }
                if dy > 0.0 { let t = (state.depth_m - y) / dy; if t < t_min { t_min = t; nx_new = dx; ny_new = -dy; } }
                if dy < 0.0 { let t = -y / dy; if t < t_min { t_min = t; nx_new = dx; ny_new = -dy; } }
                if t_min >= 1000.0 { break; }
                let nx = x + dx * t_min;
                let ny = y + dy * t_min;
                let np = to_screen(nx, ny);
                p.line_segment([prev, np], Stroke::new(0.5, col));
                prev = np;
                x = nx; y = ny;
                dx = nx_new; dy = ny_new;
            }
        }
    }

    // Source
    p.circle_filled(src, 6.0, Color32::from_rgb(255, 160, 60));
    p.text(src + Vec2::new(0.0, -10.0), egui::Align2::CENTER_CENTER, "S", FontId::monospace(8.0), Color32::WHITE);

    // Listener
    p.circle_filled(lst, 6.0, Color32::from_rgb(80, 200, 255));
    p.text(lst + Vec2::new(0.0, -10.0), egui::Align2::CENTER_CENTER, "L", FontId::monospace(8.0), Color32::WHITE);
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
