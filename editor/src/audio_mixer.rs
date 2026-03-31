use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Shape, RichText};
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AudioEffect {
    Eq {
        low_gain: f32,
        low_freq: f32,
        mid_gain: f32,
        mid_freq: f32,
        mid_q: f32,
        high_gain: f32,
        high_freq: f32,
    },
    Reverb {
        room_size: f32,
        damping: f32,
        wet: f32,
        dry: f32,
        pre_delay: f32,
    },
    Delay {
        time_l: f32,
        time_r: f32,
        feedback: f32,
        wet: f32,
        sync_to_bpm: bool,
    },
    Compressor {
        threshold: f32,
        ratio: f32,
        attack: f32,
        release: f32,
        makeup_gain: f32,
        makeup: f32,
        knee: f32,
    },
    Limiter {
        threshold: f32,
        release: f32,
        ceiling: f32,
    },
    Chorus {
        rate: f32,
        depth: f32,
        delay: f32,
        wet: f32,
        voices: u32,
    },
    Distortion {
        drive: f32,
        tone: f32,
        wet: f32,
    },
    Lowpass {
        cutoff: f32,
        resonance: f32,
    },
    Highpass {
        cutoff: f32,
        resonance: f32,
    },
    Bitcrusher {
        bit_depth: u8,
        sample_rate: f32,
        bits: u8,
        downsample: u32,
    },
}

impl AudioEffect {
    pub fn label(&self) -> &'static str {
        match self {
            AudioEffect::Eq { .. } => "EQ",
            AudioEffect::Reverb { .. } => "Reverb",
            AudioEffect::Delay { .. } => "Delay",
            AudioEffect::Compressor { .. } => "Comp",
            AudioEffect::Limiter { .. } => "Limiter",
            AudioEffect::Chorus { .. } => "Chorus",
            AudioEffect::Distortion { .. } => "Dist",
            AudioEffect::Lowpass { .. } => "LP",
            AudioEffect::Highpass { .. } => "HP",
            AudioEffect::Bitcrusher { .. } => "Bits",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            AudioEffect::Eq { .. } => Color32::from_rgb(100, 180, 255),
            AudioEffect::Reverb { .. } => Color32::from_rgb(180, 100, 255),
            AudioEffect::Delay { .. } => Color32::from_rgb(100, 255, 200),
            AudioEffect::Compressor { .. } => Color32::from_rgb(255, 180, 80),
            AudioEffect::Limiter { .. } => Color32::from_rgb(255, 80, 80),
            AudioEffect::Chorus { .. } => Color32::from_rgb(180, 255, 100),
            AudioEffect::Distortion { .. } => Color32::from_rgb(255, 100, 100),
            AudioEffect::Lowpass { .. } => Color32::from_rgb(120, 200, 220),
            AudioEffect::Highpass { .. } => Color32::from_rgb(220, 200, 120),
            AudioEffect::Bitcrusher { .. } => Color32::from_rgb(255, 150, 50),
        }
    }

    pub fn default_eq() -> AudioEffect {
        AudioEffect::Eq { low_gain: 0.0, low_freq: 80.0, mid_gain: 0.0, mid_freq: 1000.0, mid_q: 1.0, high_gain: 0.0, high_freq: 8000.0 }
    }

    pub fn default_reverb() -> AudioEffect {
        AudioEffect::Reverb { room_size: 0.5, damping: 0.5, wet: 0.3, dry: 0.7, pre_delay: 0.02 }
    }

    pub fn default_delay() -> AudioEffect {
        AudioEffect::Delay { time_l: 0.25, time_r: 0.375, feedback: 0.3, wet: 0.3, sync_to_bpm: false }
    }

    pub fn default_compressor() -> AudioEffect {
        AudioEffect::Compressor { threshold: -18.0, ratio: 4.0, attack: 0.01, release: 0.1, makeup_gain: 0.0, makeup: 0.0, knee: 2.0 }
    }

    pub fn default_limiter() -> AudioEffect {
        AudioEffect::Limiter { threshold: -1.0, release: 0.05, ceiling: -0.1 }
    }

    pub fn default_chorus() -> AudioEffect {
        AudioEffect::Chorus { rate: 1.0, depth: 0.5, delay: 0.02, wet: 0.3, voices: 3 }
    }

    pub fn default_distortion() -> AudioEffect {
        AudioEffect::Distortion { drive: 0.5, tone: 0.5, wet: 0.5 }
    }

    pub fn default_lowpass() -> AudioEffect {
        AudioEffect::Lowpass { cutoff: 1000.0, resonance: 0.5 }
    }

    pub fn default_highpass() -> AudioEffect {
        AudioEffect::Highpass { cutoff: 200.0, resonance: 0.5 }
    }

    pub fn default_bitcrusher() -> AudioEffect {
        AudioEffect::Bitcrusher { bit_depth: 8, sample_rate: 22050.0, bits: 8, downsample: 1 }
    }

    pub fn type_labels() -> &'static [&'static str] {
        &["EQ", "Reverb", "Delay", "Compressor", "Limiter", "Chorus", "Distortion", "Lowpass", "Highpass", "Bitcrusher"]
    }

    pub fn default_for_index(idx: usize) -> AudioEffect {
        match idx {
            0 => AudioEffect::default_eq(),
            1 => AudioEffect::default_reverb(),
            2 => AudioEffect::default_delay(),
            3 => AudioEffect::default_compressor(),
            4 => AudioEffect::default_limiter(),
            5 => AudioEffect::default_chorus(),
            6 => AudioEffect::default_distortion(),
            7 => AudioEffect::default_lowpass(),
            8 => AudioEffect::default_highpass(),
            _ => AudioEffect::default_bitcrusher(),
        }
    }

    pub fn type_index(&self) -> usize {
        match self {
            AudioEffect::Eq { .. } => 0,
            AudioEffect::Reverb { .. } => 1,
            AudioEffect::Delay { .. } => 2,
            AudioEffect::Compressor { .. } => 3,
            AudioEffect::Limiter { .. } => 4,
            AudioEffect::Chorus { .. } => 5,
            AudioEffect::Distortion { .. } => 6,
            AudioEffect::Lowpass { .. } => 7,
            AudioEffect::Highpass { .. } => 8,
            AudioEffect::Bitcrusher { .. } => 9,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BusSend {
    pub target_bus: usize,
    pub amount: f32,
    pub level: f32,
    pub pre_fader: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioBus {
    pub name: String,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    pub parent: Option<usize>,
    pub sends: Vec<BusSend>,
    pub effects: Vec<AudioEffect>,
    pub color: Color32,
    pub vu_l: f32,
    pub vu_r: f32,
    pub vu_peak_l: f32,
    pub vu_peak_r: f32,
    pub notes: String,
}

impl AudioBus {
    pub fn new(name: &str) -> Self {
        AudioBus {
            name: name.to_string(),
            volume: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            parent: None,
            sends: Vec::new(),
            effects: Vec::new(),
            color: Color32::from_rgb(80, 120, 180),
            vu_l: 0.0,
            vu_r: 0.0,
            vu_peak_l: 0.0,
            vu_peak_r: 0.0,
            notes: String::new(),
        }
    }

    pub fn volume_db(&self) -> f32 {
        if self.volume <= 0.0 {
            -f32::INFINITY
        } else {
            20.0 * self.volume.log10()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioClip {
    pub path: String,
    pub name: String,
    pub duration: f32,
    pub sample_rate: u32,
    pub channels: u8,
    pub loop_start: Option<f32>,
    pub loop_end: Option<f32>,
    pub assigned_bus: Option<usize>,
    pub id: usize,
    pub bus_id: Option<usize>,
    pub start_time: f32,
    pub gain: f32,
    pub looping: bool,
    pub file_path: String,
}

impl AudioClip {
    pub fn new(name: &str, path: &str, duration: f32, sample_rate: u32, channels: u8) -> Self {
        AudioClip {
            path: path.to_string(),
            name: name.to_string(),
            duration,
            sample_rate,
            channels,
            loop_start: None,
            loop_end: None,
            assigned_bus: None,
            id: 0,
            bus_id: None,
            start_time: 0.0,
            gain: 1.0,
            looping: false,
            file_path: path.to_string(),
        }
    }

    pub fn duration_str(&self) -> String {
        let minutes = (self.duration / 60.0) as u32;
        let seconds = self.duration as u32 % 60;
        let millis = ((self.duration % 1.0) * 100.0) as u32;
        format!("{:02}:{:02}.{:02}", minutes, seconds, millis)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MixerSnapshot {
    pub name: String,
    pub bus_volumes: HashMap<usize, f32>,
    pub bus_pans: HashMap<usize, f32>,
    pub bus_mutes: HashMap<usize, bool>,
    pub volumes: Vec<f32>,
    pub pans: Vec<f32>,
    pub muted: Vec<bool>,
}

impl MixerSnapshot {
    pub fn new(name: &str) -> Self {
        MixerSnapshot {
            name: name.to_string(),
            bus_volumes: HashMap::new(),
            bus_pans: HashMap::new(),
            bus_mutes: HashMap::new(),
            volumes: Vec::new(),
            pans: Vec::new(),
            muted: Vec::new(),
        }
    }

    pub fn capture(name: &str, buses: &[AudioBus]) -> Self {
        let mut snap = MixerSnapshot::new(name);
        for (i, bus) in buses.iter().enumerate() {
            snap.bus_volumes.insert(i, bus.volume);
            snap.bus_pans.insert(i, bus.pan);
            snap.bus_mutes.insert(i, bus.muted);
            snap.volumes.push(bus.volume);
            snap.pans.push(bus.pan);
            snap.muted.push(bus.muted);
        }
        snap
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AutomationParameter {
    Volume,
    Pan,
    EffectParam(usize, String),
}

impl AutomationParameter {
    pub fn label(&self) -> String {
        match self {
            AutomationParameter::Volume => "Volume".to_string(),
            AutomationParameter::Pan => "Pan".to_string(),
            AutomationParameter::EffectParam(idx, name) => format!("FX{} {}", idx, name),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Automation {
    pub bus_idx: usize,
    pub bus_id: usize,
    pub parameter: AutomationParameter,
    pub curve: Vec<(f32, f32)>,
    pub points: Vec<(f32, f32)>,
    pub name: String,
    pub enabled: bool,
}

impl Automation {
    pub fn new(bus_idx: usize, param: AutomationParameter) -> Self {
        Automation {
            bus_idx,
            bus_id: bus_idx,
            parameter: param.clone(),
            curve: vec![(0.0, 1.0), (1.0, 1.0)],
            points: vec![(0.0, 1.0), (1.0, 1.0)],
            name: format!("Bus {} {}", bus_idx, param.label()),
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BpmClock {
    pub bpm: f32,
    pub numerator: u8,
    pub denominator: u8,
    pub enabled: bool,
}

impl Default for BpmClock {
    fn default() -> Self {
        BpmClock { bpm: 120.0, numerator: 4, denominator: 4, enabled: false }
    }
}

pub struct AudioMixerEditor {
    pub buses: Vec<AudioBus>,
    pub clips: Vec<AudioClip>,
    pub snapshots: Vec<MixerSnapshot>,
    pub active_snapshot: Option<usize>,
    pub bpm_clock: BpmClock,
    pub selected_bus: Option<usize>,
    pub selected_effect: Option<(usize, usize)>,
    pub show_clips: bool,
    pub show_snapshots: bool,
    pub show_automation: bool,
    pub vu_meters: HashMap<usize, (f32, f32)>,
    pub meter_decay: f32,
    pub automation: Vec<Automation>,
    pub automations: Vec<Automation>,
    pub time: f32,
    pub new_snapshot_name: String,
    pub new_effect_type: usize,
    pub selected_automation: Option<usize>,
    pub automation_time_range: (f32, f32),
    pub snap_interpolate_target: Option<usize>,
    pub snap_interpolate_t: f32,
    pub show_bus_tree: bool,
    pub selected_clip: Option<usize>,
    pub meter_phases: Vec<f32>,
}

impl AudioMixerEditor {
    pub fn new() -> Self {
        let mut editor = AudioMixerEditor {
            buses: Vec::new(),
            clips: Vec::new(),
            snapshots: Vec::new(),
            active_snapshot: None,
            bpm_clock: BpmClock::default(),
            selected_bus: None,
            selected_effect: None,
            show_clips: true,
            show_snapshots: true,
            show_automation: false,
            vu_meters: HashMap::new(),
            meter_decay: 0.95,
            automation: Vec::new(),
            automations: Vec::new(),
            time: 0.0,
            new_snapshot_name: "Snapshot".to_string(),
            new_effect_type: 0,
            selected_automation: None,
            automation_time_range: (0.0, 8.0),
            snap_interpolate_target: None,
            snap_interpolate_t: 0.0,
            show_bus_tree: true,
            selected_clip: None,
            meter_phases: Vec::new(),
        };
        editor.populate_demo_data();
        editor
    }

    fn populate_demo_data(&mut self) {
        let mut master = AudioBus::new("Master");
        master.color = Color32::from_rgb(220, 180, 80);
        master.effects.push(AudioEffect::Limiter { threshold: -1.0, release: 0.05, ceiling: -0.1 });
        self.buses.push(master);

        let mut music = AudioBus::new("Music");
        music.parent = Some(0);
        music.color = Color32::from_rgb(100, 180, 255);
        music.effects.push(AudioEffect::default_eq());
        music.effects.push(AudioEffect::default_compressor());
        self.buses.push(music);

        let mut sfx = AudioBus::new("SFX");
        sfx.parent = Some(0);
        sfx.color = Color32::from_rgb(100, 220, 150);
        sfx.effects.push(AudioEffect::default_eq());
        self.buses.push(sfx);

        let mut voice = AudioBus::new("Voice");
        voice.parent = Some(0);
        voice.color = Color32::from_rgb(220, 150, 100);
        voice.effects.push(AudioEffect::Compressor { threshold: -12.0, ratio: 3.0, attack: 0.005, release: 0.08, makeup_gain: 3.0, makeup: 3.0, knee: 2.0 });
        voice.effects.push(AudioEffect::Reverb { room_size: 0.2, damping: 0.6, wet: 0.1, dry: 0.9, pre_delay: 0.005 });
        self.buses.push(voice);

        let mut ambient = AudioBus::new("Ambient");
        ambient.parent = Some(1);
        ambient.color = Color32::from_rgb(150, 100, 220);
        ambient.volume = 0.7;
        ambient.effects.push(AudioEffect::default_reverb());
        self.buses.push(ambient);

        let mut combat_sfx = AudioBus::new("Combat SFX");
        combat_sfx.parent = Some(2);
        combat_sfx.color = Color32::from_rgb(220, 100, 100);
        self.buses.push(combat_sfx);

        self.meter_phases = vec![0.0; self.buses.len()];

        self.clips.push(AudioClip::new("theme_main", "audio/music/theme_main.ogg", 180.0, 44100, 2));
        self.clips.push(AudioClip::new("battle_music", "audio/music/battle.ogg", 120.0, 44100, 2));
        self.clips.push(AudioClip::new("sword_hit", "audio/sfx/sword_hit.wav", 0.3, 44100, 1));
        self.clips.push(AudioClip::new("footstep_grass", "audio/sfx/footstep_grass.wav", 0.25, 44100, 1));
        self.clips.push(AudioClip::new("npc_greeting", "audio/voice/npc_greeting.ogg", 2.5, 44100, 1));
        let mut looped_clip = AudioClip::new("forest_ambience", "audio/ambient/forest.ogg", 60.0, 44100, 2);
        looped_clip.loop_start = Some(0.0);
        looped_clip.loop_end = Some(60.0);
        looped_clip.assigned_bus = Some(4);
        self.clips.push(looped_clip);

        self.clips[0].assigned_bus = Some(1);
        self.clips[1].assigned_bus = Some(1);
        self.clips[2].assigned_bus = Some(5);
        self.clips[3].assigned_bus = Some(2);
        self.clips[4].assigned_bus = Some(3);

        let snap = MixerSnapshot::capture("Default Mix", &self.buses);
        self.snapshots.push(snap);

        let mut combat_snap = MixerSnapshot::capture("Combat Mix", &self.buses);
        combat_snap.bus_volumes.insert(1, 0.4);
        combat_snap.bus_volumes.insert(2, 1.2f32.min(1.0));
        self.snapshots.push(combat_snap);
    }

    pub fn update_vu_meters(&mut self, dt: f32) {
        self.time += dt;

        while self.meter_phases.len() < self.buses.len() {
            self.meter_phases.push(0.0);
        }

        for (i, bus) in self.buses.iter().enumerate() {
            if bus.muted {
                let entry = self.vu_meters.entry(i).or_insert((0.0, 0.0));
                entry.0 *= self.meter_decay.powf(dt * 60.0);
                entry.1 *= self.meter_decay.powf(dt * 60.0);
                continue;
            }

            let phase = self.meter_phases.get(i).copied().unwrap_or(0.0);
            let freq = 0.8 + (i as f32) * 0.3;
            let raw_l = 0.3 + 0.5 * (self.time * freq * std::f32::consts::TAU + phase).sin().abs();
            let raw_r = 0.3 + 0.5 * (self.time * freq * std::f32::consts::TAU + phase + 0.7).sin().abs();
            let peak_l = raw_l * bus.volume * (1.0 - bus.pan.max(0.0));
            let peak_r = raw_r * bus.volume * (1.0 + bus.pan.min(0.0));

            let entry = self.vu_meters.entry(i).or_insert((0.0, 0.0));
            if peak_l > entry.0 {
                entry.0 = peak_l;
            } else {
                entry.0 *= self.meter_decay.powf(dt * 60.0);
            }
            if peak_r > entry.1 {
                entry.1 = peak_r;
            } else {
                entry.1 *= self.meter_decay.powf(dt * 60.0);
            }
        }
    }
}

pub fn show(ui: &mut egui::Ui, editor: &mut AudioMixerEditor, dt: f32) {
    editor.update_vu_meters(dt);

    ui.horizontal(|ui| {
        ui.heading(RichText::new("Audio Mixer").size(18.0).color(Color32::from_rgb(100, 200, 255)));
        ui.separator();
        ui.label("BPM:");
        ui.add(egui::DragValue::new(&mut editor.bpm_clock.bpm).range(20.0..=300.0).speed(0.5));
        ui.label(format!("{}/{}", editor.bpm_clock.numerator, editor.bpm_clock.denominator));
        ui.checkbox(&mut editor.bpm_clock.enabled, "Sync");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.toggle_value(&mut editor.show_automation, "Automation");
            ui.toggle_value(&mut editor.show_snapshots, "Snapshots");
            ui.toggle_value(&mut editor.show_clips, "Clips");
            ui.toggle_value(&mut editor.show_bus_tree, "Bus Tree");
        });
    });
    ui.separator();

    // Bottom panels first
    if let Some((bus_idx, eff_idx)) = editor.selected_effect {
        if bus_idx < editor.buses.len() && eff_idx < editor.buses[bus_idx].effects.len() {
            egui::TopBottomPanel::bottom("effect_editor_panel")
                .resizable(true)
                .default_height(180.0)
                .show_inside(ui, |ui| {
                    let bus_name = editor.buses[bus_idx].name.clone();
                    let eff_label = editor.buses[bus_idx].effects[eff_idx].label();
                    ui.horizontal(|ui| {
                        ui.strong(format!("Effect Editor — {} > {}", bus_name, eff_label));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                editor.selected_effect = None;
                            }
                        });
                    });
                    ui.separator();
                    show_effect_editor(ui, &mut editor.buses[bus_idx].effects[eff_idx], &editor.bpm_clock);
                });
        }
    }

    if editor.show_automation {
        egui::TopBottomPanel::bottom("automation_panel")
            .resizable(true)
            .default_height(200.0)
            .show_inside(ui, |ui| {
                show_automation_panel(ui, editor);
            });
    }

    if editor.show_bus_tree {
        egui::SidePanel::left("bus_tree_panel")
            .resizable(true)
            .default_width(160.0)
            .show_inside(ui, |ui| {
                show_bus_tree(ui, editor);
            });
    }

    if editor.show_clips {
        egui::SidePanel::right("clip_browser_panel")
            .resizable(true)
            .default_width(220.0)
            .show_inside(ui, |ui| {
                show_clip_browser(ui, editor);
            });
    }

    if editor.show_snapshots {
        egui::SidePanel::right("snapshot_panel")
            .resizable(true)
            .default_width(180.0)
            .show_inside(ui, |ui| {
                show_snapshot_manager(ui, editor);
            });
    }

    // Main channel strip view
    egui::CentralPanel::default().show_inside(ui, |ui| {
        show_channel_strips(ui, editor);
    });
}

fn show_bus_tree(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    ui.strong("Bus Hierarchy");
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("bus_tree_scroll")
        .show(ui, |ui| {
            let bus_count = editor.buses.len();
            // Find roots (no parent)
            let roots: Vec<usize> = (0..bus_count).filter(|&i| editor.buses[i].parent.is_none()).collect();

            fn draw_bus_node(
                ui: &mut egui::Ui,
                editor: &mut AudioMixerEditor,
                bus_idx: usize,
                depth: usize,
                bus_count: usize,
            ) {
                let indent = depth as f32 * 16.0;
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    let bus = &editor.buses[bus_idx];
                    let is_sel = editor.selected_bus == Some(bus_idx);
                    let dot_color = bus.color;

                    let label_text = RichText::new(format!("  {}", bus.name))
                        .color(if is_sel { Color32::WHITE } else { Color32::LIGHT_GRAY });

                    let resp = ui.selectable_label(is_sel, label_text);
                    // Draw colored dot before
                    let dot_center = Pos2::new(resp.rect.min.x + 4.0, resp.rect.center().y);
                    ui.painter().circle_filled(dot_center, 4.0, dot_color);

                    if resp.clicked() {
                        editor.selected_bus = Some(bus_idx);
                    }

                    // Mute indicator
                    if bus.muted {
                        ui.label(RichText::new("M").color(Color32::from_rgb(255, 180, 50)).small());
                    }
                    if bus.soloed {
                        ui.label(RichText::new("S").color(Color32::from_rgb(255, 220, 50)).small());
                    }
                });

                // Children
                let children: Vec<usize> = (0..bus_count)
                    .filter(|&i| editor.buses[i].parent == Some(bus_idx))
                    .collect();
                for child in children {
                    draw_bus_node(ui, editor, child, depth + 1, bus_count);
                }
            }

            let roots_clone = roots.clone();
            for root in roots_clone {
                draw_bus_node(ui, editor, root, 0, bus_count);
            }
        });

    ui.separator();
    if ui.button("+ Add Bus").clicked() {
        let mut new_bus = AudioBus::new(&format!("Bus {}", editor.buses.len()));
        new_bus.parent = Some(0);
        let colors = [
            Color32::from_rgb(180, 100, 255),
            Color32::from_rgb(100, 255, 180),
            Color32::from_rgb(255, 180, 100),
            Color32::from_rgb(100, 180, 255),
        ];
        new_bus.color = colors[editor.buses.len() % colors.len()];
        editor.buses.push(new_bus);
        editor.meter_phases.push(0.0);
    }
}

fn show_channel_strips(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    let strip_width = 90.0;
    let strip_height = ui.available_height();

    egui::ScrollArea::horizontal()
        .id_salt("channel_strips_scroll")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let bus_count = editor.buses.len();
                for bus_idx in 0..bus_count {
                    draw_channel_strip(ui, editor, bus_idx, strip_width, strip_height);
                    ui.separator();
                }
            });
        });
}

fn draw_channel_strip(ui: &mut egui::Ui, editor: &mut AudioMixerEditor, bus_idx: usize, width: f32, height: f32) {
    let is_selected = editor.selected_bus == Some(bus_idx);
    let bus_color = editor.buses[bus_idx].color;
    let bus_muted = editor.buses[bus_idx].muted;
    let bus_soloed = editor.buses[bus_idx].soloed;
    let bus_volume = editor.buses[bus_idx].volume;
    let bus_pan = editor.buses[bus_idx].pan;
    let bus_name = editor.buses[bus_idx].name.clone();
    let effects_count = editor.buses[bus_idx].effects.len();

    let frame_color = if is_selected {
        Color32::from_rgb(50, 65, 90)
    } else {
        Color32::from_rgb(28, 28, 35)
    };

    egui::Frame::none()
        .fill(frame_color)
        .stroke(Stroke::new(if is_selected { 2.0 } else { 1.0 }, if is_selected { Color32::WHITE } else { Color32::from_rgb(55, 55, 70) }))
        .inner_margin(4.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);

            // Bus name (colored bar at top)
            let (name_rect, name_resp) = ui.allocate_exact_size(Vec2::new(width - 8.0, 20.0), egui::Sense::click());
            ui.painter().rect_filled(name_rect, 3.0, bus_color);
            ui.painter().text(
                name_rect.center(),
                egui::Align2::CENTER_CENTER,
                &bus_name,
                FontId::proportional(10.0),
                Color32::BLACK,
            );
            if name_resp.clicked() {
                editor.selected_bus = Some(bus_idx);
            }

            // Effect chain
            let eff_count = editor.buses[bus_idx].effects.len();
            for eff_idx in 0..eff_count {
                let eff_label = editor.buses[bus_idx].effects[eff_idx].label().to_string();
                let eff_color = editor.buses[bus_idx].effects[eff_idx].color();
                let is_eff_sel = editor.selected_effect == Some((bus_idx, eff_idx));
                let (eff_rect, eff_resp) = ui.allocate_exact_size(Vec2::new(width - 8.0, 16.0), egui::Sense::click());
                let eff_bg = if is_eff_sel { Color32::from_rgb(50, 60, 80) } else { Color32::from_rgb(35, 35, 45) };
                ui.painter().rect_filled(eff_rect, 2.0, eff_bg);
                ui.painter().rect_stroke(eff_rect, 2.0, Stroke::new(1.0, eff_color), egui::StrokeKind::Inside);
                ui.painter().text(
                    eff_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &eff_label,
                    FontId::proportional(9.0),
                    eff_color,
                );
                if eff_resp.clicked() {
                    if editor.selected_effect == Some((bus_idx, eff_idx)) {
                        editor.selected_effect = None;
                    } else {
                        editor.selected_effect = Some((bus_idx, eff_idx));
                    }
                }
                eff_resp.context_menu(|ui| {
                    if ui.button("Remove Effect").clicked() {
                        editor.buses[bus_idx].effects.remove(eff_idx);
                        if editor.selected_effect == Some((bus_idx, eff_idx)) {
                            editor.selected_effect = None;
                        }
                        ui.close_menu();
                    }
                });
            }

            // Add effect button
            ui.horizontal(|ui| {
                let eff_labels = AudioEffect::type_labels();
                egui::ComboBox::from_id_salt(egui::Id::new("add_eff").with(bus_idx))
                    .selected_text(eff_labels[editor.new_effect_type])
                    .width(50.0)
                    .show_ui(ui, |ui| {
                        for (i, label) in eff_labels.iter().enumerate() {
                            if ui.selectable_label(i == editor.new_effect_type, *label).clicked() {
                                editor.new_effect_type = i;
                            }
                        }
                    });
                if ui.small_button("+").clicked() {
                    let eff = AudioEffect::default_for_index(editor.new_effect_type);
                    editor.buses[bus_idx].effects.push(eff);
                }
            });

            ui.add_space(4.0);

            // VU Meters
            let vu_l = editor.vu_meters.get(&bus_idx).map(|v| v.0).unwrap_or(0.0);
            let vu_r = editor.vu_meters.get(&bus_idx).map(|v| v.1).unwrap_or(0.0);

            let meter_width = (width - 12.0) / 2.0 - 2.0;
            let meter_height = 100.0;

            ui.horizontal(|ui| {
                draw_vu_meter(ui, vu_l, meter_width, meter_height);
                ui.add_space(2.0);
                draw_vu_meter(ui, vu_r, meter_width, meter_height);
            });

            ui.add_space(4.0);

            // Pan knob
            ui.horizontal(|ui| {
                ui.label(RichText::new("Pan").small().color(Color32::GRAY));
                let pan_size = 30.0;
                let (pan_rect, pan_resp) = ui.allocate_exact_size(Vec2::new(pan_size, pan_size), egui::Sense::drag());
                draw_pan_knob(ui.painter(), pan_rect, editor.buses[bus_idx].pan, bus_color);
                if pan_resp.dragged() {
                    editor.buses[bus_idx].pan = (editor.buses[bus_idx].pan + pan_resp.drag_delta().x * 0.01).clamp(-1.0, 1.0);
                }
                if pan_resp.double_clicked() {
                    editor.buses[bus_idx].pan = 0.0;
                }
                ui.label(RichText::new(format!("{:.0}", editor.buses[bus_idx].pan * 100.0)).small().color(Color32::GRAY));
            });

            // Volume fader (vertical)
            let fader_height = 120.0;
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                let vol = &mut editor.buses[bus_idx].volume;
                let fader_resp = ui.add(
                    egui::Slider::new(vol, 0.0..=1.5)
                        .vertical()
                        .show_value(false)
                        .step_by(0.001)
                );
            });

            // Volume dB label
            let db = if editor.buses[bus_idx].volume <= 0.0 { "-inf".to_string() } else {
                format!("{:.1} dB", 20.0 * editor.buses[bus_idx].volume.log10())
            };
            ui.label(RichText::new(db).small().color(Color32::from_rgb(180, 220, 180)));

            // Mute / Solo buttons
            ui.horizontal(|ui| {
                let mute_color = if editor.buses[bus_idx].muted { Color32::from_rgb(255, 180, 50) } else { Color32::from_rgb(60, 60, 70) };
                if ui.add(egui::Button::new(RichText::new("M").small().strong()).fill(mute_color)).clicked() {
                    editor.buses[bus_idx].muted = !editor.buses[bus_idx].muted;
                }
                let solo_color = if editor.buses[bus_idx].soloed { Color32::from_rgb(255, 220, 50) } else { Color32::from_rgb(60, 60, 70) };
                if ui.add(egui::Button::new(RichText::new("S").small().strong()).fill(solo_color)).clicked() {
                    editor.buses[bus_idx].soloed = !editor.buses[bus_idx].soloed;
                }
            });

            // Sends button
            if !editor.buses[bus_idx].sends.is_empty() {
                ui.label(RichText::new(format!("{} sends", editor.buses[bus_idx].sends.len())).small().color(Color32::GRAY));
            }
        });
}

fn draw_vu_meter(ui: &mut egui::Ui, level: f32, width: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 25));

    let level_clamped = level.clamp(0.0, 1.0);
    let segments = [
        (0.0, 0.7, Color32::from_rgb(50, 200, 80)),
        (0.7, 0.85, Color32::from_rgb(200, 200, 50)),
        (0.85, 1.0, Color32::from_rgb(220, 60, 60)),
    ];

    for (seg_lo, seg_hi, color) in &segments {
        if level_clamped > *seg_lo {
            let fill_lo = (*seg_lo).max(0.0);
            let fill_hi = level_clamped.min(*seg_hi);
            if fill_hi <= fill_lo { continue; }

            let y_hi = rect.max.y - fill_lo * height;
            let y_lo = rect.max.y - fill_hi * height;
            let seg_rect = Rect::from_min_max(
                Pos2::new(rect.min.x + 1.0, y_lo),
                Pos2::new(rect.max.x - 1.0, y_hi),
            );
            painter.rect_filled(seg_rect, 0.0, *color);
        }
    }

    // Segment lines
    for seg_line in [0.7, 0.85] {
        let y = rect.max.y - seg_line * height;
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            Stroke::new(0.5, Color32::from_rgb(40, 40, 50)),
        );
    }

    // Scale marks
    for db_mark in [-18_i32, -12, -6, -3, 0] {
        let linear = if db_mark == -60 { 0.0 } else { 10_f32.powf(db_mark as f32 / 20.0) };
        let norm = (linear / 1.5).clamp(0.0, 1.0);
        let y = rect.max.y - norm * height;
        painter.line_segment(
            [Pos2::new(rect.max.x - 3.0, y), Pos2::new(rect.max.x, y)],
            Stroke::new(0.5, Color32::GRAY),
        );
    }
}

fn draw_pan_knob(painter: &Painter, rect: Rect, pan: f32, color: Color32) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 2.0;

    painter.circle_filled(center, radius, Color32::from_rgb(35, 35, 45));
    painter.circle_stroke(center, radius, Stroke::new(1.5, Color32::from_rgb(80, 80, 100)));

    // Center line (gray)
    painter.line_segment(
        [Pos2::new(center.x, center.y), Pos2::new(center.x, center.y - radius * 0.6)],
        Stroke::new(1.0, Color32::from_rgb(60, 60, 70)),
    );

    // Pan indicator
    let angle = pan * std::f32::consts::FRAC_PI_2;
    let indicator_end = Pos2::new(
        center.x + radius * 0.7 * angle.sin(),
        center.y - radius * 0.7 * angle.cos(),
    );
    painter.line_segment([center, indicator_end], Stroke::new(2.0, color));
    painter.circle_filled(indicator_end, 2.5, color);
}

fn show_effect_editor(ui: &mut egui::Ui, effect: &mut AudioEffect, bpm_clock: &BpmClock) {
    match effect {
        AudioEffect::Eq { low_gain, low_freq, mid_gain, mid_freq, mid_q, high_gain, high_freq } => {
            ui.columns(3, |cols| {
                cols[0].label(RichText::new("Low").color(Color32::from_rgb(100, 150, 255)));
                cols[0].add(egui::Slider::new(low_gain, -18.0..=18.0).text("Gain dB").suffix(" dB"));
                cols[0].add(egui::Slider::new(low_freq, 20.0..=500.0).text("Freq").suffix(" Hz").logarithmic(true));

                cols[1].label(RichText::new("Mid").color(Color32::from_rgb(100, 220, 100)));
                cols[1].add(egui::Slider::new(mid_gain, -18.0..=18.0).text("Gain dB").suffix(" dB"));
                cols[1].add(egui::Slider::new(mid_freq, 200.0..=8000.0).text("Freq").suffix(" Hz").logarithmic(true));
                cols[1].add(egui::Slider::new(mid_q, 0.1..=10.0).text("Q").logarithmic(true));

                cols[2].label(RichText::new("High").color(Color32::from_rgb(255, 150, 100)));
                cols[2].add(egui::Slider::new(high_gain, -18.0..=18.0).text("Gain dB").suffix(" dB"));
                cols[2].add(egui::Slider::new(high_freq, 1000.0..=20000.0).text("Freq").suffix(" Hz").logarithmic(true));
            });
        }

        AudioEffect::Reverb { room_size, damping, wet, dry, pre_delay } => {
            ui.add(egui::Slider::new(room_size, 0.0..=1.0).text("Room Size"));
            ui.add(egui::Slider::new(damping, 0.0..=1.0).text("Damping"));
            ui.add(egui::Slider::new(wet, 0.0..=1.0).text("Wet"));
            ui.add(egui::Slider::new(dry, 0.0..=1.0).text("Dry"));
            ui.add(egui::Slider::new(pre_delay, 0.0..=0.5).text("Pre-Delay").suffix(" s"));
        }

        AudioEffect::Delay { time_l, time_r, feedback, wet, sync_to_bpm } => {
            ui.checkbox(sync_to_bpm, "Sync to BPM");
            if *sync_to_bpm && bpm_clock.enabled {
                let beat_dur = 60.0 / bpm_clock.bpm;
                ui.label(format!("Beat: {:.3}s | Half: {:.3}s | Quarter: {:.3}s", beat_dur, beat_dur * 0.5, beat_dur * 0.25));
                egui::ComboBox::from_label("Left Note")
                    .selected_text(format!("{:.3}s", time_l))
                    .show_ui(ui, |ui| {
                        for (label, factor) in [("1/1", 1.0), ("1/2", 0.5), ("1/4", 0.25), ("1/8", 0.125), ("3/8", 0.375)] {
                            if ui.selectable_label(false, label).clicked() {
                                *time_l = beat_dur * factor;
                            }
                        }
                    });
            } else {
                ui.add(egui::Slider::new(time_l, 0.01..=2.0).text("Time L").suffix(" s"));
                ui.add(egui::Slider::new(time_r, 0.01..=2.0).text("Time R").suffix(" s"));
            }
            ui.add(egui::Slider::new(feedback, 0.0..=0.99).text("Feedback"));
            ui.add(egui::Slider::new(wet, 0.0..=1.0).text("Wet"));
        }

        AudioEffect::Compressor { threshold, ratio, attack, release, makeup_gain, .. } => {
            ui.add(egui::Slider::new(threshold, -60.0..=0.0).text("Threshold").suffix(" dB"));
            ui.add(egui::Slider::new(ratio, 1.0..=20.0).text("Ratio").suffix(":1"));
            ui.add(egui::Slider::new(attack, 0.0..=0.5).text("Attack").suffix(" s").logarithmic(true));
            ui.add(egui::Slider::new(release, 0.01..=2.0).text("Release").suffix(" s").logarithmic(true));
            ui.add(egui::Slider::new(makeup_gain, -12.0..=24.0).text("Makeup Gain").suffix(" dB"));

            // Compression curve visualization
            let (curve_rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 120.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(curve_rect, 2.0, Color32::from_rgb(20, 20, 25));

            let thresh_x = curve_rect.min.x + (*threshold + 60.0) / 60.0 * curve_rect.width();
            painter.line_segment(
                [Pos2::new(thresh_x, curve_rect.min.y), Pos2::new(thresh_x, curve_rect.max.y)],
                Stroke::new(0.5, Color32::from_rgb(80, 80, 80)),
            );

            // 1:1 reference
            painter.line_segment(
                [curve_rect.min, curve_rect.max],
                Stroke::new(0.5, Color32::from_rgb(60, 60, 60)),
            );

            // Compression curve
            let steps = 60;
            let mut prev: Option<Pos2> = None;
            for step in 0..=steps {
                let in_db = -60.0 + step as f32 * 60.0 / steps as f32;
                let out_db = if in_db < *threshold {
                    in_db
                } else {
                    *threshold + (in_db - *threshold) / *ratio
                };
                let x = curve_rect.min.x + (in_db + 60.0) / 60.0 * curve_rect.width();
                let y = curve_rect.max.y - (out_db + 60.0) / 60.0 * curve_rect.height();
                let pt = Pos2::new(x, y);
                if let Some(p) = prev {
                    painter.line_segment([p, pt], Stroke::new(1.5, Color32::from_rgb(100, 200, 255)));
                }
                prev = Some(pt);
            }
        }

        AudioEffect::Limiter { threshold, release, .. } => {
            ui.add(egui::Slider::new(threshold, -20.0..=0.0).text("Threshold").suffix(" dB"));
            ui.add(egui::Slider::new(release, 0.01..=1.0).text("Release").suffix(" s").logarithmic(true));
        }

        AudioEffect::Chorus { rate, depth, delay, wet, .. } => {
            ui.add(egui::Slider::new(rate, 0.01..=10.0).text("Rate").suffix(" Hz").logarithmic(true));
            ui.add(egui::Slider::new(depth, 0.0..=1.0).text("Depth"));
            ui.add(egui::Slider::new(delay, 0.001..=0.05).text("Delay").suffix(" s"));
            ui.add(egui::Slider::new(wet, 0.0..=1.0).text("Wet"));
        }

        AudioEffect::Distortion { drive, tone, wet } => {
            ui.add(egui::Slider::new(drive, 0.0..=1.0).text("Drive"));
            ui.add(egui::Slider::new(tone, 0.0..=1.0).text("Tone"));
            ui.add(egui::Slider::new(wet, 0.0..=1.0).text("Wet/Dry"));

            // Waveshaping visualization
            let (wave_rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 60.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(wave_rect, 2.0, Color32::from_rgb(20, 20, 25));
            let steps = 60;
            let mut prev: Option<Pos2> = None;
            for step in 0..=steps {
                let x_norm = step as f32 / steps as f32;
                let input = x_norm * 2.0 - 1.0;
                let driven = input * (1.0 + *drive * 10.0);
                let output = driven.tanh();
                let x = wave_rect.min.x + x_norm * wave_rect.width();
                let y = wave_rect.center().y - output * wave_rect.height() * 0.45;
                let pt = Pos2::new(x, y.clamp(wave_rect.min.y, wave_rect.max.y));
                if let Some(p) = prev {
                    painter.line_segment([p, pt], Stroke::new(1.5, Color32::from_rgb(255, 120, 80)));
                }
                prev = Some(pt);
            }
        }

        AudioEffect::Lowpass { cutoff, resonance } => {
            ui.add(egui::Slider::new(cutoff, 20.0..=20000.0).text("Cutoff").suffix(" Hz").logarithmic(true));
            ui.add(egui::Slider::new(resonance, 0.0..=1.0).text("Resonance"));
        }

        AudioEffect::Highpass { cutoff, resonance } => {
            ui.add(egui::Slider::new(cutoff, 20.0..=20000.0).text("Cutoff").suffix(" Hz").logarithmic(true));
            ui.add(egui::Slider::new(resonance, 0.0..=1.0).text("Resonance"));
        }

        AudioEffect::Bitcrusher { bit_depth, sample_rate, .. } => {
            let mut bd = *bit_depth as i32;
            ui.add(egui::Slider::new(&mut bd, 1..=32).text("Bit Depth"));
            *bit_depth = bd as u8;
            ui.add(egui::Slider::new(sample_rate, 1000.0..=96000.0).text("Sample Rate").suffix(" Hz").logarithmic(true));
        }
    }
}

fn show_clip_browser(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    ui.strong("Clip Browser");
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("clip_browser_scroll")
        .show(ui, |ui| {
            let clip_count = editor.clips.len();
            for ci in 0..clip_count {
                let is_sel = editor.selected_clip == Some(ci);
                let clip = &editor.clips[ci];
                let clip_name = clip.name.clone();
                let duration_str = clip.duration_str();
                let channels = clip.channels;
                let has_loop = clip.loop_start.is_some();
                let assigned_bus = clip.assigned_bus;

                ui.push_id(ci, |ui| {
                    egui::Frame::none()
                        .fill(if is_sel { Color32::from_rgb(40, 50, 70) } else { Color32::from_rgb(28, 28, 35) })
                        .stroke(Stroke::new(1.0, if is_sel { Color32::from_rgb(100, 150, 255) } else { Color32::from_rgb(45, 45, 55) }))
                        .inner_margin(4.0)
                        .corner_radius(3.0)
                        .show(ui, |ui| {
                            let resp = ui.horizontal(|ui| {
                                let icon = if channels > 1 { "ST" } else { "MN" };
                                ui.label(RichText::new(icon).small().color(Color32::from_rgb(150, 150, 180)));
                                ui.label(RichText::new(&clip_name).color(Color32::WHITE));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if has_loop {
                                        ui.label(RichText::new("↻").color(Color32::from_rgb(100, 200, 150)));
                                    }
                                });
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(duration_str).small().color(Color32::GRAY));
                                ui.label(RichText::new(format!("{}ch", channels)).small().color(Color32::GRAY));
                                if let Some(bus_idx) = assigned_bus {
                                    if bus_idx < editor.buses.len() {
                                        let bus_color = editor.buses[bus_idx].color;
                                        let bus_name = editor.buses[bus_idx].name.clone();
                                        ui.label(RichText::new(format!("→{}", bus_name)).small().color(bus_color));
                                    }
                                }
                            });

                            if ui.interact(ui.min_rect(), egui::Id::new("clip_click").with(ci), egui::Sense::click()).clicked() {
                                editor.selected_clip = Some(ci);
                            }
                        });

                    ui.add_space(2.0);
                });
            }
        });

    ui.separator();
    if let Some(ci) = editor.selected_clip {
        if ci < editor.clips.len() {
            ui.strong("Clip Properties");
            let clip = &mut editor.clips[ci];
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut clip.name);
            });
            ui.label(format!("Path: {}", clip.path));
            ui.label(format!("Sample Rate: {} Hz", clip.sample_rate));
            ui.label(format!("Channels: {}", clip.channels));
            ui.label(format!("Duration: {}s", clip.duration));

            let mut has_loop = clip.loop_start.is_some();
            if ui.checkbox(&mut has_loop, "Loop").changed() {
                if has_loop {
                    clip.loop_start = Some(0.0);
                    clip.loop_end = Some(clip.duration);
                } else {
                    clip.loop_start = None;
                    clip.loop_end = None;
                }
            }
            if let Some(ref mut ls) = clip.loop_start {
                ui.add(egui::Slider::new(ls, 0.0..=clip.duration).text("Loop Start").suffix("s"));
            }
            if let Some(ref mut le) = clip.loop_end {
                let dur = clip.duration;
                ui.add(egui::Slider::new(le, 0.0..=dur).text("Loop End").suffix("s"));
            }

            ui.horizontal(|ui| {
                ui.label("Assign to Bus:");
                let bus_names: Vec<String> = editor.buses.iter().map(|b| b.name.clone()).collect();
                let cur_bus_name = clip.assigned_bus.and_then(|idx| bus_names.get(idx).cloned()).unwrap_or_else(|| "None".to_string());
                egui::ComboBox::from_id_salt("clip_bus_assign")
                    .selected_text(cur_bus_name)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(clip.assigned_bus.is_none(), "None").clicked() {
                            clip.assigned_bus = None;
                        }
                        for (i, name) in bus_names.iter().enumerate() {
                            if ui.selectable_label(clip.assigned_bus == Some(i), name.as_str()).clicked() {
                                clip.assigned_bus = Some(i);
                            }
                        }
                    });
            });
        }
    }
}

fn show_snapshot_manager(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    ui.strong("Mixer Snapshots");
    ui.separator();

    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut editor.new_snapshot_name);
        if ui.button("Save").clicked() && !editor.new_snapshot_name.is_empty() {
            let snap = MixerSnapshot::capture(&editor.new_snapshot_name.clone(), &editor.buses);
            editor.snapshots.push(snap);
        }
    });

    ui.separator();

    let mut to_delete: Option<usize> = None;
    let mut to_recall: Option<usize> = None;
    let mut to_interpolate: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("snapshot_scroll")
        .show(ui, |ui| {
            let snap_count = editor.snapshots.len();
            for si in 0..snap_count {
                let is_active = editor.active_snapshot == Some(si);
                let snap_name = editor.snapshots[si].name.clone();

                ui.push_id(si, |ui| {
                    egui::Frame::none()
                        .fill(if is_active { Color32::from_rgb(40, 60, 40) } else { Color32::from_rgb(28, 28, 35) })
                        .stroke(Stroke::new(1.0, if is_active { Color32::from_rgb(80, 200, 80) } else { Color32::from_rgb(45, 45, 55) }))
                        .inner_margin(4.0)
                        .corner_radius(3.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if is_active {
                                    ui.label(RichText::new("●").color(Color32::from_rgb(80, 220, 80)));
                                }
                                ui.label(RichText::new(&snap_name).color(Color32::WHITE));
                            });
                            ui.horizontal(|ui| {
                                if ui.small_button("Recall").clicked() {
                                    to_recall = Some(si);
                                }
                                if ui.small_button("Interp.").clicked() {
                                    to_interpolate = Some(si);
                                }
                                if ui.small_button("x").clicked() {
                                    to_delete = Some(si);
                                }
                            });
                        });
                    ui.add_space(2.0);
                });
            }
        });

    if let Some(si) = to_recall {
        let snap = editor.snapshots[si].clone();
        for (&bus_idx, &vol) in &snap.bus_volumes {
            if bus_idx < editor.buses.len() {
                editor.buses[bus_idx].volume = vol;
            }
        }
        for (&bus_idx, &pan) in &snap.bus_pans {
            if bus_idx < editor.buses.len() {
                editor.buses[bus_idx].pan = pan;
            }
        }
        for (&bus_idx, &muted) in &snap.bus_mutes {
            if bus_idx < editor.buses.len() {
                editor.buses[bus_idx].muted = muted;
            }
        }
        editor.active_snapshot = Some(si);
    }

    if let Some(si) = to_delete {
        editor.snapshots.remove(si);
        if editor.active_snapshot == Some(si) {
            editor.active_snapshot = None;
        }
    }

    if let Some(si) = to_interpolate {
        editor.snap_interpolate_target = Some(si);
        editor.snap_interpolate_t = 0.0;
    }

    // Interpolation progress
    if let Some(target_si) = editor.snap_interpolate_target {
        if target_si < editor.snapshots.len() {
            editor.snap_interpolate_t += 0.016;
            let t = editor.snap_interpolate_t.min(1.0);
            let snap = editor.snapshots[target_si].clone();
            for (&bus_idx, &target_vol) in &snap.bus_volumes {
                if bus_idx < editor.buses.len() {
                    let cur = editor.buses[bus_idx].volume;
                    editor.buses[bus_idx].volume = cur + (target_vol - cur) * t * 0.1;
                }
            }
            if t >= 1.0 {
                editor.snap_interpolate_target = None;
                editor.active_snapshot = Some(target_si);
            }

            ui.separator();
            ui.label(format!("Interpolating: {:.0}%", t * 100.0));
            let (prog_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), egui::Sense::hover());
            ui.painter().rect_filled(prog_rect, 3.0, Color32::from_rgb(30, 30, 35));
            let fill = Rect::from_min_size(prog_rect.min, Vec2::new(prog_rect.width() * t, prog_rect.height()));
            ui.painter().rect_filled(fill, 3.0, Color32::from_rgb(100, 220, 100));
        }
    }
}

fn show_automation_panel(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    ui.horizontal(|ui| {
        ui.strong("Automation");
        if ui.button("+ Add Lane").clicked() {
            if let Some(bus_idx) = editor.selected_bus {
                let auto = Automation::new(bus_idx, AutomationParameter::Volume);
                editor.automation.push(auto);
            }
        }
    });
    ui.separator();

    let auto_count = editor.automation.len();
    let time_range = editor.automation_time_range;
    let time_span = time_range.1 - time_range.0;
    let available_width = ui.available_width() - 150.0;

    egui::ScrollArea::vertical()
        .id_salt("automation_scroll")
        .show(ui, |ui| {
            for ai in 0..auto_count {
                let is_sel = editor.selected_automation == Some(ai);
                let auto_name = editor.automation[ai].name.clone();
                let bus_idx = editor.automation[ai].bus_idx;
                let param_label = editor.automation[ai].parameter.label();
                let bus_name = editor.buses.get(bus_idx).map(|b| b.name.clone()).unwrap_or_default();

                ui.push_id(ai, |ui| {
                    ui.horizontal(|ui| {
                        // Lane header
                        let lane_label = format!("{} > {}", bus_name, param_label);
                        if ui.selectable_label(is_sel, &lane_label).clicked() {
                            editor.selected_automation = Some(ai);
                        }

                        // Curve display
                        let (curve_rect, curve_resp) = ui.allocate_exact_size(
                            Vec2::new(available_width.max(100.0), 40.0),
                            egui::Sense::click_and_drag(),
                        );
                        let painter = ui.painter();
                        painter.rect_filled(curve_rect, 2.0, Color32::from_rgb(22, 22, 28));
                        painter.rect_stroke(curve_rect, 2.0, Stroke::new(1.0, Color32::from_rgb(50, 50, 65)), egui::StrokeKind::Inside);

                        // Grid lines
                        for beat in 0..=8 {
                            let x = curve_rect.min.x + beat as f32 / 8.0 * curve_rect.width();
                            painter.line_segment(
                                [Pos2::new(x, curve_rect.min.y), Pos2::new(x, curve_rect.max.y)],
                                Stroke::new(0.5, Color32::from_rgb(40, 40, 50)),
                            );
                        }

                        let curve = &editor.automation[ai].curve;
                        let curve_color = Color32::from_rgb(100, 200, 255);

                        if curve.len() >= 2 {
                            for pair in curve.windows(2) {
                                let (t0, v0) = pair[0];
                                let (t1, v1) = pair[1];
                                let x0 = curve_rect.min.x + (t0 - time_range.0) / time_span * curve_rect.width();
                                let x1 = curve_rect.min.x + (t1 - time_range.0) / time_span * curve_rect.width();
                                let y0 = curve_rect.max.y - v0 * curve_rect.height();
                                let y1 = curve_rect.max.y - v1 * curve_rect.height();
                                painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y1)], Stroke::new(1.5, curve_color));
                            }
                        }

                        // Draw control points
                        for &(t, v) in curve {
                            let x = curve_rect.min.x + (t - time_range.0) / time_span * curve_rect.width();
                            let y = curve_rect.max.y - v * curve_rect.height();
                            painter.circle_filled(Pos2::new(x, y), 3.5, curve_color);
                        }

                        // Add point on click
                        if curve_resp.clicked() {
                            if let Some(pos) = curve_resp.interact_pointer_pos() {
                                let t = time_range.0 + (pos.x - curve_rect.min.x) / curve_rect.width() * time_span;
                                let v = 1.0 - (pos.y - curve_rect.min.y) / curve_rect.height();
                                editor.automation[ai].curve.push((t, v.clamp(0.0, 1.0)));
                                editor.automation[ai].curve.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                            }
                        }

                        ui.vertical(|ui| {
                            if ui.small_button("x").clicked() {
                                // mark for deletion - handled outside loop
                            }
                        });
                    });
                });
            }
        });
}

pub fn show_panel(ctx: &egui::Context, editor: &mut AudioMixerEditor, dt: f32, open: &mut bool) {
    egui::Window::new("Audio Mixer")
        .open(open)
        .resizable(true)
        .default_size([1400.0, 750.0])
        .min_size([900.0, 500.0])
        .show(ctx, |ui| {
            show(ui, editor, dt);
        });
}

// ---- Bus routing helpers ----

pub fn get_children(buses: &[AudioBus], parent: usize) -> Vec<usize> {
    buses.iter().enumerate()
        .filter(|(_, b)| b.parent == Some(parent))
        .map(|(i, _)| i)
        .collect()
}

pub fn get_ancestors(buses: &[AudioBus], idx: usize) -> Vec<usize> {
    let mut ancestors = Vec::new();
    let mut cur = idx;
    let mut visited = HashSet::new();
    loop {
        if visited.contains(&cur) { break; }
        visited.insert(cur);
        if let Some(parent) = buses[cur].parent {
            ancestors.push(parent);
            cur = parent;
        } else {
            break;
        }
    }
    ancestors
}

pub fn get_descendants(buses: &[AudioBus], idx: usize) -> Vec<usize> {
    let mut desc = Vec::new();
    let mut stack = vec![idx];
    while let Some(cur) = stack.pop() {
        for child in get_children(buses, cur) {
            desc.push(child);
            stack.push(child);
        }
    }
    desc
}

pub fn effective_volume(buses: &[AudioBus], idx: usize) -> f32 {
    let mut vol = buses[idx].volume;
    let ancestors = get_ancestors(buses, idx);
    for anc in ancestors {
        vol *= buses[anc].volume;
    }
    vol
}

pub fn is_effectively_muted(buses: &[AudioBus], idx: usize) -> bool {
    if buses[idx].muted { return true; }
    get_ancestors(buses, idx).iter().any(|&a| buses[a].muted)
}

// ---- VU meter helpers ----

pub fn db_to_linear(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}

pub fn linear_to_db(lin: f32) -> f32 {
    if lin <= 0.0 { return -f32::INFINITY; }
    20.0 * lin.log10()
}

pub fn vu_color_for_level(level: f32) -> Color32 {
    if level > 0.85 {
        Color32::from_rgb(220, 60, 60)
    } else if level > 0.7 {
        Color32::from_rgb(220, 200, 50)
    } else {
        Color32::from_rgb(50, 200, 80)
    }
}

pub fn format_db(level: f32) -> String {
    let db = linear_to_db(level);
    if db.is_infinite() {
        "-inf dB".to_string()
    } else {
        format!("{:.1} dB", db)
    }
}

// ---- Effect parameter ranges ----

pub fn effect_param_range(effect: &AudioEffect, param_name: &str) -> (f32, f32) {
    match effect {
        AudioEffect::Eq { .. } => match param_name {
            "gain" => (-18.0, 18.0),
            "freq" => (20.0, 20000.0),
            "q" => (0.1, 10.0),
            _ => (0.0, 1.0),
        },
        AudioEffect::Compressor { .. } => match param_name {
            "threshold" => (-60.0, 0.0),
            "ratio" => (1.0, 20.0),
            "attack" => (0.0, 0.5),
            "release" => (0.01, 2.0),
            "makeup_gain" => (-12.0, 24.0),
            _ => (0.0, 1.0),
        },
        AudioEffect::Reverb { .. } => (0.0, 1.0),
        _ => (0.0, 1.0),
    }
}

// ---- Snapshot interpolation ----

pub fn interpolate_snapshots(a: &MixerSnapshot, b: &MixerSnapshot, t: f32) -> MixerSnapshot {
    let mut result = MixerSnapshot::new(&format!("Interp {:.2}", t));
    let all_keys: HashSet<usize> = a.bus_volumes.keys().chain(b.bus_volumes.keys()).copied().collect();
    for key in &all_keys {
        let va = a.bus_volumes.get(key).copied().unwrap_or(1.0);
        let vb = b.bus_volumes.get(key).copied().unwrap_or(1.0);
        result.bus_volumes.insert(*key, va + (vb - va) * t);
    }
    let pan_keys: HashSet<usize> = a.bus_pans.keys().chain(b.bus_pans.keys()).copied().collect();
    for key in &pan_keys {
        let pa = a.bus_pans.get(key).copied().unwrap_or(0.0);
        let pb = b.bus_pans.get(key).copied().unwrap_or(0.0);
        result.bus_pans.insert(*key, pa + (pb - pa) * t);
    }
    result
}

// ---- Automation curve helpers ----

pub fn evaluate_curve(curve: &[(f32, f32)], t: f32) -> f32 {
    if curve.is_empty() { return 1.0; }
    if curve.len() == 1 { return curve[0].1; }
    if t <= curve[0].0 { return curve[0].1; }
    if t >= curve[curve.len() - 1].0 { return curve[curve.len() - 1].1; }

    for i in 0..curve.len() - 1 {
        let (t0, v0) = curve[i];
        let (t1, v1) = curve[i + 1];
        if t >= t0 && t <= t1 {
            let frac = (t - t0) / (t1 - t0);
            return v0 + (v1 - v0) * frac;
        }
    }
    1.0
}

pub fn simplify_curve(curve: &mut Vec<(f32, f32)>, tolerance: f32) {
    if curve.len() <= 2 { return; }
    let mut keep = vec![true; curve.len()];
    for i in 1..curve.len() - 1 {
        let (t0, v0) = curve[i - 1];
        let (t2, v2) = curve[i + 1];
        let (t1, v1) = curve[i];
        let t_frac = if t2 - t0 > 0.0 { (t1 - t0) / (t2 - t0) } else { 0.5 };
        let expected = v0 + (v2 - v0) * t_frac;
        if (expected - v1).abs() < tolerance {
            keep[i] = false;
        }
    }
    let mut simplified = Vec::new();
    for (i, &k) in keep.iter().enumerate() {
        if k { simplified.push(curve[i]); }
    }
    *curve = simplified;
}

pub fn normalize_curve(curve: &mut Vec<(f32, f32)>) {
    if curve.is_empty() { return; }
    let min_v = curve.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min);
    let max_v = curve.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);
    let range = max_v - min_v;
    if range < 0.0001 { return; }
    for (_, v) in curve.iter_mut() {
        *v = (*v - min_v) / range;
    }
}

// ---- Clip analysis display ----

pub fn show_clip_waveform_placeholder(ui: &mut egui::Ui, clip: &AudioClip) {
    let width = ui.available_width().min(200.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 25));

    let seed = clip.name.len() as f32;
    let steps = 60_usize;
    let step_w = width / steps as f32;
    for s in 0..steps {
        let x = rect.min.x + s as f32 * step_w;
        let amp = ((s as f32 * 0.3 + seed).sin() * 0.5 + 0.5) * rect.height() * 0.4;
        let cy = rect.center().y;
        painter.line_segment(
            [Pos2::new(x, cy - amp), Pos2::new(x, cy + amp)],
            Stroke::new(step_w * 0.6, Color32::from_rgb(80, 160, 220)),
        );
    }

    if let (Some(ls), Some(le)) = (clip.loop_start, clip.loop_end) {
        let ls_x = rect.min.x + (ls / clip.duration.max(0.001)) * width;
        let le_x = rect.min.x + (le / clip.duration.max(0.001)) * width;
        painter.line_segment(
            [Pos2::new(ls_x, rect.min.y), Pos2::new(ls_x, rect.max.y)],
            Stroke::new(1.5, Color32::from_rgb(100, 220, 100)),
        );
        painter.line_segment(
            [Pos2::new(le_x, rect.min.y), Pos2::new(le_x, rect.max.y)],
            Stroke::new(1.5, Color32::from_rgb(220, 100, 100)),
        );
        let loop_rect = Rect::from_min_max(
            Pos2::new(ls_x, rect.min.y),
            Pos2::new(le_x, rect.max.y),
        );
        painter.rect_filled(loop_rect, 0.0, Color32::from_rgba_unmultiplied(100, 200, 100, 25));
    }
}

// ---- BPM sync helpers ----

pub fn beats_to_seconds(beats: f32, bpm: f32) -> f32 {
    beats * 60.0 / bpm
}

pub fn seconds_to_beats(seconds: f32, bpm: f32) -> f32 {
    seconds * bpm / 60.0
}

pub fn nearest_beat_value(seconds: f32, bpm: f32, subdivisions: &[f32]) -> f32 {
    let beat = 60.0 / bpm;
    let best = subdivisions.iter()
        .map(|&sub| beat * sub)
        .min_by(|a, b| {
            let da = (a - seconds).abs();
            let db = (b - seconds).abs();
            da.partial_cmp(&db).unwrap()
        });
    best.unwrap_or(seconds)
}

// ---- AudioMixerEditor extended methods ----

impl AudioMixerEditor {
    pub fn master_volume(&self) -> f32 {
        self.buses.first().map(|b| b.volume).unwrap_or(1.0)
    }

    pub fn set_master_volume(&mut self, vol: f32) {
        if let Some(master) = self.buses.first_mut() {
            master.volume = vol.clamp(0.0, 2.0);
        }
    }

    pub fn mute_all(&mut self) {
        for bus in self.buses.iter_mut() {
            bus.muted = true;
        }
    }

    pub fn unmute_all(&mut self) {
        for bus in self.buses.iter_mut() {
            bus.muted = false;
        }
    }

    pub fn solo_bus(&mut self, idx: usize) {
        for (i, bus) in self.buses.iter_mut().enumerate() {
            bus.soloed = i == idx;
            bus.muted = i != idx;
        }
    }

    pub fn clear_solo(&mut self) {
        for bus in self.buses.iter_mut() {
            bus.soloed = false;
            bus.muted = false;
        }
    }

    pub fn add_effect_to_bus(&mut self, bus_idx: usize, effect: AudioEffect) {
        if bus_idx < self.buses.len() {
            self.buses[bus_idx].effects.push(effect);
        }
    }

    pub fn remove_effect_from_bus(&mut self, bus_idx: usize, effect_idx: usize) {
        if bus_idx < self.buses.len() && effect_idx < self.buses[bus_idx].effects.len() {
            self.buses[bus_idx].effects.remove(effect_idx);
        }
    }

    pub fn move_effect_up(&mut self, bus_idx: usize, effect_idx: usize) {
        if bus_idx < self.buses.len() && effect_idx > 0 && effect_idx < self.buses[bus_idx].effects.len() {
            self.buses[bus_idx].effects.swap(effect_idx - 1, effect_idx);
        }
    }

    pub fn move_effect_down(&mut self, bus_idx: usize, effect_idx: usize) {
        let len = self.buses.get(bus_idx).map(|b| b.effects.len()).unwrap_or(0);
        if bus_idx < self.buses.len() && effect_idx + 1 < len {
            self.buses[bus_idx].effects.swap(effect_idx, effect_idx + 1);
        }
    }

    pub fn add_send(&mut self, from_bus: usize, to_bus: usize, amount: f32) {
        if from_bus < self.buses.len() && to_bus < self.buses.len() {
            self.buses[from_bus].sends.push(BusSend { target_bus: to_bus, amount, level: amount, pre_fader: false });
        }
    }

    pub fn remove_send(&mut self, from_bus: usize, send_idx: usize) {
        if from_bus < self.buses.len() && send_idx < self.buses[from_bus].sends.len() {
            self.buses[from_bus].sends.remove(send_idx);
        }
    }

    pub fn clips_for_bus(&self, bus_idx: usize) -> Vec<usize> {
        self.clips.iter().enumerate()
            .filter(|(_, c)| c.assigned_bus == Some(bus_idx))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn automation_indices_for_bus(&self, bus_idx: usize) -> Vec<usize> {
        self.automation.iter().enumerate()
            .filter(|(_, a)| a.bus_idx == bus_idx)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn total_effect_count(&self) -> usize {
        self.buses.iter().map(|b| b.effects.len()).sum()
    }

    pub fn bus_depth(&self, idx: usize) -> usize {
        let ancestors = get_ancestors(&self.buses, idx);
        ancestors.len()
    }

    pub fn apply_automation_at_time(&mut self, time: f32) {
        let auto_clone = self.automation.clone();
        for auto in &auto_clone {
            let value = evaluate_curve(&auto.curve, time);
            if auto.bus_idx < self.buses.len() {
                match &auto.parameter {
                    AutomationParameter::Volume => {
                        self.buses[auto.bus_idx].volume = value.clamp(0.0, 2.0);
                    }
                    AutomationParameter::Pan => {
                        self.buses[auto.bus_idx].pan = (value * 2.0 - 1.0).clamp(-1.0, 1.0);
                    }
                    AutomationParameter::EffectParam(_, _) => {
                        // Effect param automation would be applied here in a full implementation
                    }
                }
            }
        }
    }

    pub fn reset_all_to_default(&mut self) {
        for bus in self.buses.iter_mut() {
            bus.volume = 1.0;
            bus.pan = 0.0;
            bus.muted = false;
            bus.soloed = false;
        }
        self.active_snapshot = None;
    }

    pub fn get_peak_db(&self, bus_idx: usize) -> (f32, f32) {
        let (l, r) = self.vu_meters.get(&bus_idx).copied().unwrap_or((0.0, 0.0));
        (linear_to_db(l), linear_to_db(r))
    }

    pub fn is_clipping(&self, bus_idx: usize) -> bool {
        self.vu_meters.get(&bus_idx)
            .map(|(l, r)| *l > 1.0 || *r > 1.0)
            .unwrap_or(false)
    }

    pub fn bus_index_by_name(&self, name: &str) -> Option<usize> {
        self.buses.iter().position(|b| b.name == name)
    }

    pub fn clip_index_by_name(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|c| c.name == name)
    }
}

// ---- Send editor panel ----

pub fn show_send_editor(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    if let Some(bus_idx) = editor.selected_bus {
        if bus_idx >= editor.buses.len() { return; }

        let bus_name = editor.buses[bus_idx].name.clone();
        ui.strong(format!("Sends from: {}", bus_name));
        ui.separator();

        let sends_count = editor.buses[bus_idx].sends.len();
        let mut to_remove: Option<usize> = None;

        for si in 0..sends_count {
            let target_bus_idx = editor.buses[bus_idx].sends[si].target_bus;
            let target_name = editor.buses.get(target_bus_idx).map(|b| b.name.clone()).unwrap_or_else(|| format!("Bus {}", target_bus_idx));
            let send = &mut editor.buses[bus_idx].sends[si];
            ui.push_id(si, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("→ {}", target_name));
                    ui.add(egui::Slider::new(&mut send.amount, 0.0..=1.0).show_value(true));
                    if ui.small_button("x").clicked() { to_remove = Some(si); }
                });
            });
        }

        if let Some(idx) = to_remove {
            editor.remove_send(bus_idx, idx);
        }

        ui.separator();
        ui.horizontal(|ui| {
            let bus_names: Vec<String> = editor.buses.iter().map(|b| b.name.clone()).collect();
            let mut target = 0_usize;
            egui::ComboBox::from_id_salt("new_send_target")
                .selected_text(bus_names.get(target).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (i, name) in bus_names.iter().enumerate() {
                        if i != bus_idx {
                            if ui.selectable_label(false, name.as_str()).clicked() {
                                target = i;
                            }
                        }
                    }
                });
            if ui.button("Add Send").clicked() {
                editor.add_send(bus_idx, target, 0.5);
            }
        });
    }
}

// ---- Master bus quick controls ----

pub fn show_master_controls(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    ui.horizontal(|ui| {
        ui.strong("Master:");
        let master_vol = &mut editor.buses[0].volume;
        ui.add(egui::Slider::new(master_vol, 0.0..=1.5).text("Volume").show_value(false));
        let db_str = if *master_vol <= 0.0 { "-inf".to_string() } else {
            format!("{:.1}dB", linear_to_db(*master_vol))
        };
        ui.label(RichText::new(db_str).color(Color32::from_rgb(180, 220, 180)).monospace());

        let master_muted = editor.buses[0].muted;
        if ui.add(egui::Button::new(RichText::new("M").small())
            .fill(if master_muted { Color32::from_rgb(255, 150, 50) } else { Color32::from_rgb(50, 50, 60) })
        ).clicked() {
            editor.buses[0].muted = !master_muted;
        }

        let is_clipping = editor.is_clipping(0);
        if is_clipping {
            ui.label(RichText::new("CLIP!").color(Color32::from_rgb(255, 50, 50)).strong().monospace());
        }
    });
}

// ---- Spectrum display placeholder ----

pub fn draw_spectrum_display(ui: &mut egui::Ui, time: f32, bus_idx: usize, volume: f32) {
    let width = ui.available_width().min(220.0);
    let height = 60.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_rgb(15, 15, 20));

    let bands = 24_usize;
    let band_w = width / bands as f32;
    let freq_start = 20.0_f32;
    let freq_end = 20000.0_f32;

    for b in 0..bands {
        let freq = freq_start * (freq_end / freq_start).powf(b as f32 / bands as f32);
        let x = rect.min.x + b as f32 * band_w;
        let amp = (time * freq.log2() * 0.1 + bus_idx as f32 * 0.5).sin().abs() * volume;
        let bar_h = amp * height * 0.85;
        let bar_rect = Rect::from_min_size(
            Pos2::new(x + 1.0, rect.max.y - bar_h),
            Vec2::new(band_w - 2.0, bar_h),
        );
        let band_color = if amp > 0.85 {
            Color32::from_rgb(220, 60, 60)
        } else if amp > 0.7 {
            Color32::from_rgb(220, 200, 50)
        } else {
            Color32::from_rgb(60, 180, 100)
        };
        painter.rect_filled(bar_rect, 1.0, band_color);
    }
}

// ---- Effect preset system ----

pub struct EffectPreset {
    pub name: String,
    pub effect: AudioEffect,
}

impl EffectPreset {
    pub fn builtin_presets() -> Vec<EffectPreset> {
        vec![
            EffectPreset { name: "Bright EQ".to_string(), effect: AudioEffect::Eq { low_gain: -2.0, low_freq: 80.0, mid_gain: 0.0, mid_freq: 1000.0, mid_q: 1.0, high_gain: 4.0, high_freq: 8000.0 } },
            EffectPreset { name: "Warm EQ".to_string(), effect: AudioEffect::Eq { low_gain: 3.0, low_freq: 120.0, mid_gain: -1.0, mid_freq: 2500.0, mid_q: 0.8, high_gain: -2.0, high_freq: 6000.0 } },
            EffectPreset { name: "Cave Reverb".to_string(), effect: AudioEffect::Reverb { room_size: 0.9, damping: 0.2, wet: 0.5, dry: 0.5, pre_delay: 0.05 } },
            EffectPreset { name: "Small Room".to_string(), effect: AudioEffect::Reverb { room_size: 0.2, damping: 0.7, wet: 0.15, dry: 0.85, pre_delay: 0.002 } },
            EffectPreset { name: "Quarter Note Delay".to_string(), effect: AudioEffect::Delay { time_l: 0.25, time_r: 0.375, feedback: 0.3, wet: 0.25, sync_to_bpm: true } },
            EffectPreset { name: "Heavy Compressor".to_string(), effect: AudioEffect::Compressor { threshold: -24.0, ratio: 8.0, attack: 0.003, release: 0.08, makeup_gain: 6.0, makeup: 6.0, knee: 2.0 } },
            EffectPreset { name: "Gentle Chorus".to_string(), effect: AudioEffect::Chorus { rate: 0.5, depth: 0.3, delay: 0.015, wet: 0.2, voices: 3 } },
            EffectPreset { name: "Hard Clip".to_string(), effect: AudioEffect::Distortion { drive: 0.9, tone: 0.6, wet: 0.7 } },
            EffectPreset { name: "Lo-Fi".to_string(), effect: AudioEffect::Bitcrusher { bit_depth: 8, sample_rate: 11025.0, bits: 8, downsample: 2 } },
            EffectPreset { name: "LP Sweep".to_string(), effect: AudioEffect::Lowpass { cutoff: 800.0, resonance: 0.4 } },
        ]
    }
}

pub fn show_effect_presets(ui: &mut egui::Ui, bus_idx: usize, buses: &mut Vec<AudioBus>) {
    egui::CollapsingHeader::new("Effect Presets")
        .default_open(false)
        .show(ui, |ui| {
            for preset in EffectPreset::builtin_presets() {
                if ui.button(RichText::new(&preset.name).small()).clicked() {
                    if bus_idx < buses.len() {
                        buses[bus_idx].effects.push(preset.effect);
                    }
                }
            }
        });
}

// ---- Routing matrix display ----

pub fn show_routing_matrix(ui: &mut egui::Ui, buses: &[AudioBus]) {
    let n = buses.len().min(8);
    if n == 0 { return; }

    ui.strong("Routing Matrix");
    ui.separator();

    let cell_size = 22.0;
    let label_w = 80.0;

    // Headers
    ui.horizontal(|ui| {
        ui.add_space(label_w);
        for i in 0..n {
            let (cell_rect, _) = ui.allocate_exact_size(Vec2::new(cell_size, cell_size), egui::Sense::hover());
            let name = buses[i].name.chars().take(4).collect::<String>();
            ui.painter().text(
                cell_rect.center(),
                egui::Align2::CENTER_CENTER,
                name,
                FontId::proportional(7.5),
                Color32::GRAY,
            );
        }
    });

    for row in 0..n {
        ui.horizontal(|ui| {
            let row_name = buses[row].name.chars().take(10).collect::<String>();
            ui.add_sized(Vec2::new(label_w, cell_size), egui::Label::new(RichText::new(row_name).small()));
            for col in 0..n {
                let (cell_rect, _) = ui.allocate_exact_size(Vec2::new(cell_size, cell_size), egui::Sense::hover());
                let has_send = buses[row].sends.iter().any(|s| s.target_bus == col);
                let is_parent = buses[row].parent == Some(col);
                let fill = if row == col {
                    Color32::from_rgb(50, 50, 60)
                } else if is_parent {
                    Color32::from_rgb(50, 100, 50)
                } else if has_send {
                    Color32::from_rgb(50, 80, 120)
                } else {
                    Color32::from_rgb(25, 25, 30)
                };
                ui.painter().rect_filled(cell_rect.shrink(1.0), 2.0, fill);
                if is_parent || has_send {
                    ui.painter().text(
                        cell_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        if is_parent { "P" } else { "S" },
                        FontId::proportional(8.0),
                        if is_parent { Color32::from_rgb(100, 200, 100) } else { Color32::from_rgb(100, 150, 220) },
                    );
                }
            }
        });
    }
}

// ---- Clip timeline mini view ----

pub fn show_clip_timeline_mini(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    let total_dur = editor.clips.iter().map(|c| c.duration).fold(0.0_f32, f32::max).max(10.0);
    let width = ui.available_width().min(400.0);
    let row_h = 18.0;
    let label_w = 80.0;
    let timeline_w = width - label_w;

    ui.strong("Clip Timeline");
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("clip_timeline_scroll")
        .max_height(200.0)
        .show(ui, |ui| {
            for (ci, clip) in editor.clips.iter().enumerate() {
                ui.horizontal(|ui| {
                    let name_label = clip.name.chars().take(10).collect::<String>();
                    ui.add_sized(Vec2::new(label_w, row_h), egui::Label::new(RichText::new(name_label).small().color(Color32::GRAY)));

                    let (row_rect, _) = ui.allocate_exact_size(Vec2::new(timeline_w, row_h), egui::Sense::hover());
                    ui.painter().rect_filled(row_rect, 0.0, Color32::from_rgb(18, 18, 22));

                    let clip_x0 = row_rect.min.x;
                    let clip_x1 = row_rect.min.x + (clip.duration / total_dur) * timeline_w;
                    let clip_rect = Rect::from_min_max(
                        Pos2::new(clip_x0, row_rect.min.y + 2.0),
                        Pos2::new(clip_x1, row_rect.max.y - 2.0),
                    );
                    let bus_color = clip.assigned_bus
                        .and_then(|bi| editor.buses.get(bi))
                        .map(|b| b.color)
                        .unwrap_or(Color32::from_rgb(80, 80, 100));
                    ui.painter().rect_filled(clip_rect, 2.0, Color32::from_rgba_unmultiplied(bus_color.r() / 2, bus_color.g() / 2, bus_color.b() / 2, 200));
                    ui.painter().rect_stroke(clip_rect, 2.0, Stroke::new(1.0, bus_color), egui::StrokeKind::Inside);

                    if let (Some(ls), Some(le)) = (clip.loop_start, clip.loop_end) {
                        let lx0 = row_rect.min.x + (ls / total_dur) * timeline_w;
                        let lx1 = row_rect.min.x + (le / total_dur) * timeline_w;
                        let loop_rect = Rect::from_min_max(
                            Pos2::new(lx0, row_rect.min.y + 2.0),
                            Pos2::new(lx1, row_rect.max.y - 2.0),
                        );
                        ui.painter().rect_filled(loop_rect, 0.0, Color32::from_rgba_unmultiplied(100, 220, 100, 40));
                    }
                });
            }
        });
}

// ---- Additional AudioMixerEditor display helpers ----

pub fn show_bus_stats(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    let total_buses = editor.buses.len();
    let muted_count = editor.buses.iter().filter(|b| b.muted).count();
    let soloed_count = editor.buses.iter().filter(|b| b.soloed).count();
    let fx_count: usize = editor.buses.iter().map(|b| b.effects.len()).sum();
    let send_count: usize = editor.buses.iter().map(|b| b.sends.len()).sum();

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Buses: {}", total_buses)).small().color(Color32::GRAY));
        ui.separator();
        if muted_count > 0 {
            ui.label(RichText::new(format!("Muted: {}", muted_count)).small().color(Color32::from_rgb(255, 180, 50)));
        }
        if soloed_count > 0 {
            ui.label(RichText::new(format!("Solo: {}", soloed_count)).small().color(Color32::from_rgb(255, 220, 50)));
        }
        ui.label(RichText::new(format!("FX: {} | Sends: {}", fx_count, send_count)).small().color(Color32::GRAY));
        ui.label(RichText::new(format!("Clips: {} | Snapshots: {}", editor.clips.len(), editor.snapshots.len())).small().color(Color32::GRAY));
    });
}

// ---- EQ Frequency response curve ----

pub fn draw_eq_frequency_response(ui: &mut egui::Ui, effect: &AudioEffect) {
    if let AudioEffect::Eq { low_gain, low_freq, mid_gain, mid_freq, mid_q, high_gain, high_freq } = effect {
        let width = ui.available_width().min(300.0);
        let height = 80.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, Color32::from_rgb(15, 15, 20));

        let freq_min_log = 20.0_f32.log10();
        let freq_max_log = 20000.0_f32.log10();

        let freq_to_x = |freq: f32| -> f32 {
            let log_f = freq.max(20.0).log10();
            rect.min.x + (log_f - freq_min_log) / (freq_max_log - freq_min_log) * width
        };

        // Grid lines
        for &f in &[100.0, 1000.0, 10000.0] {
            let x = freq_to_x(f);
            painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], Stroke::new(0.5, Color32::from_rgb(35, 35, 45)));
        }
        let zero_y = rect.center().y;
        painter.line_segment([Pos2::new(rect.min.x, zero_y), Pos2::new(rect.max.x, zero_y)], Stroke::new(0.5, Color32::from_rgb(50, 50, 60)));

        // Frequency response
        let steps = 200_usize;
        let mut prev: Option<Pos2> = None;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let log_f = freq_min_log + t * (freq_max_log - freq_min_log);
            let freq = 10.0_f32.powf(log_f);
            let x = rect.min.x + t * width;

            // Simple shelving + bell approximation
            let low_shelf = low_gain * (1.0 / (1.0 + (freq / low_freq).powi(2))).sqrt();
            let mid_bell = mid_gain * (1.0 / (1.0 + ((freq / mid_freq - mid_freq / freq) * mid_q).powi(2)));
            let high_shelf = high_gain * (1.0 - 1.0 / (1.0 + (freq / high_freq).powi(2))).sqrt();
            let total_db = low_shelf + mid_bell + high_shelf;

            let y = zero_y - (total_db / 18.0) * (height / 2.0 - 4.0);
            let y = y.clamp(rect.min.y + 2.0, rect.max.y - 2.0);
            let pt = Pos2::new(x, y);
            if let Some(p) = prev {
                painter.line_segment([p, pt], Stroke::new(1.5, Color32::from_rgb(100, 180, 255)));
            }
            prev = Some(pt);
        }

        // Frequency labels
        for &(f, label) in &[(100.0, "100"), (1000.0, "1k"), (10000.0, "10k")] {
            let x = freq_to_x(f);
            painter.text(Pos2::new(x, rect.max.y - 8.0), egui::Align2::CENTER_BOTTOM, label, FontId::proportional(8.0), Color32::GRAY);
        }
    }
}

// ---- Compressor gain reduction meter ----

pub fn draw_gain_reduction_meter(ui: &mut egui::Ui, input_level: f32, threshold: f32, ratio: f32) {
    let width = ui.available_width().min(200.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 16.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 25));

    let input_db = linear_to_db(input_level);
    let threshold_db = threshold;
    let gain_reduction = if input_db > threshold_db {
        (input_db - threshold_db) * (1.0 - 1.0 / ratio)
    } else {
        0.0
    };

    let gr_norm = (gain_reduction / 20.0).clamp(0.0, 1.0);
    if gr_norm > 0.001 {
        let fill = Rect::from_min_size(
            Pos2::new(rect.max.x - gr_norm * rect.width(), rect.min.y),
            Vec2::new(gr_norm * rect.width(), rect.height()),
        );
        painter.rect_filled(fill, 2.0, Color32::from_rgb(200, 120, 50));
    }

    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("-{:.1} dB GR", gain_reduction),
        FontId::proportional(9.0),
        Color32::WHITE,
    );
}

// ---- Delay time display ----

pub fn draw_delay_visualization(ui: &mut egui::Ui, time_l: f32, time_r: f32, feedback: f32) {
    let width = ui.available_width().min(200.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_rgb(15, 15, 20));

    let max_time = time_l.max(time_r).max(0.001);
    let max_echoes = 5;
    let cy = rect.center().y;

    for echo in 0..max_echoes {
        let amp = feedback.powi(echo as i32);
        if amp < 0.01 { break; }

        // Left channel
        let lx = rect.min.x + (time_l * (echo as f32 + 1.0) / (max_time * max_echoes as f32 + 0.001)).min(1.0) * width;
        let lh = amp * (rect.height() / 2.0 - 4.0);
        painter.line_segment(
            [Pos2::new(lx, cy - lh), Pos2::new(lx, cy + lh)],
            Stroke::new(2.0, Color32::from_rgba_unmultiplied(100, 200, 255, (amp * 220.0) as u8)),
        );

        // Right channel
        let rx = rect.min.x + (time_r * (echo as f32 + 1.0) / (max_time * max_echoes as f32 + 0.001)).min(1.0) * width;
        painter.line_segment(
            [Pos2::new(rx, cy - lh * 0.8), Pos2::new(rx, cy + lh * 0.8)],
            Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 180, 100, (amp * 180.0) as u8)),
        );
    }
    painter.line_segment([Pos2::new(rect.min.x, cy), Pos2::new(rect.max.x, cy)], Stroke::new(0.5, Color32::from_rgb(40, 40, 50)));
}

// ---- Reverb tail visualization ----

pub fn draw_reverb_tail(ui: &mut egui::Ui, room_size: f32, damping: f32, wet: f32) {
    let width = ui.available_width().min(200.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_rgb(15, 15, 20));

    let tail_length = room_size * 3.0 + 0.5;
    let steps = 100_usize;
    let mut prev: Option<Pos2> = None;
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let time = t * 3.0;
        let decay = (-time / (tail_length + 0.001)).exp();
        let damp_factor = 1.0 - damping * t;
        let env = decay * damp_factor.max(0.0) * wet;
        let x = rect.min.x + t * width;
        let y = rect.center().y - env * (rect.height() / 2.0 - 4.0);
        let pt = Pos2::new(x, y.clamp(rect.min.y + 2.0, rect.max.y - 2.0));
        if let Some(p) = prev {
            painter.line_segment([p, pt], Stroke::new(1.5, Color32::from_rgb(180, 100, 255)));
        }
        prev = Some(pt);
    }
}

// ---- Bitcrusher waveform display ----

pub fn draw_bitcrusher_effect(ui: &mut egui::Ui, bit_depth: u8, sample_rate_ratio: f32) {
    let width = ui.available_width().min(160.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_rgb(15, 15, 20));

    let levels = 2_u32.pow(bit_depth as u32) as f32;
    let steps = 80_usize;
    let sr_steps = (sample_rate_ratio * steps as f32).max(1.0) as usize;

    let mut prev: Option<Pos2> = None;
    let mut last_quantized = 0.0_f32;
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let x = rect.min.x + t * width;

        // Input sine wave
        let input = (t * 4.0 * std::f32::consts::PI).sin();

        // Sample-and-hold
        let quantized = if s % (steps / sr_steps.max(1) + 1) == 0 {
            (input * levels / 2.0).round() / (levels / 2.0)
        } else {
            last_quantized
        };
        last_quantized = quantized;

        // Draw original faint
        let y_orig = rect.center().y - input * (rect.height() / 2.0 - 4.0);
        let y_crush = rect.center().y - quantized * (rect.height() / 2.0 - 4.0);

        if let Some(p) = prev {
            painter.line_segment([p, Pos2::new(x, y_crush)], Stroke::new(1.5, Color32::from_rgb(255, 150, 50)));
        }
        prev = Some(Pos2::new(x, y_crush));
    }
}

// ---- AudioBus detailed info ----

pub fn show_bus_detail_overlay(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    if let Some(bus_idx) = editor.selected_bus {
        if bus_idx >= editor.buses.len() { return; }
        let bus = &editor.buses[bus_idx];
        let eff_vol = effective_volume(&editor.buses, bus_idx);
        let is_muted = is_effectively_muted(&editor.buses, bus_idx);

        egui::Frame::none()
            .fill(Color32::from_rgb(22, 28, 40))
            .stroke(Stroke::new(1.0, bus.color))
            .inner_margin(8.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot_rect.center(), 6.0, bus.color);
                    ui.heading(RichText::new(&bus.name).size(14.0).color(Color32::WHITE));
                });
                ui.separator();
                egui::Grid::new("bus_detail_grid").num_columns(2).striped(true).show(ui, |ui| {
                    ui.label("Volume:");
                    let db_str = format_db(bus.volume);
                    ui.label(RichText::new(db_str).color(Color32::from_rgb(180, 220, 180)).monospace());
                    ui.end_row();
                    ui.label("Effective Volume:");
                    ui.label(RichText::new(format_db(eff_vol)).color(Color32::from_rgb(150, 180, 150)).monospace());
                    ui.end_row();
                    ui.label("Pan:");
                    let pan_label = if bus.pan < -0.01 { format!("L {:.0}%", bus.pan.abs() * 100.0) }
                        else if bus.pan > 0.01 { format!("R {:.0}%", bus.pan * 100.0) }
                        else { "Center".to_string() };
                    ui.label(pan_label);
                    ui.end_row();
                    ui.label("Status:");
                    let status = if is_muted { "Muted" } else if bus.soloed { "Soloed" } else { "Active" };
                    let sc = if is_muted { Color32::from_rgb(255, 180, 50) } else if bus.soloed { Color32::from_rgb(255, 220, 50) } else { Color32::from_rgb(100, 220, 100) };
                    ui.label(RichText::new(status).color(sc));
                    ui.end_row();
                    ui.label("Effects:");
                    ui.label(format!("{}", bus.effects.len()));
                    ui.end_row();
                    ui.label("Sends:");
                    ui.label(format!("{}", bus.sends.len()));
                    ui.end_row();
                    if let Some(parent) = bus.parent {
                        ui.label("Parent:");
                        ui.label(editor.buses.get(parent).map(|b| b.name.as_str()).unwrap_or("?"));
                        ui.end_row();
                    }
                    let children = get_children(&editor.buses, bus_idx);
                    if !children.is_empty() {
                        ui.label("Children:");
                        let child_names: Vec<_> = children.iter().filter_map(|&i| editor.buses.get(i).map(|b| b.name.as_str())).collect();
                        ui.label(child_names.join(", "));
                        ui.end_row();
                    }
                    let (peak_l_db, peak_r_db) = editor.get_peak_db(bus_idx);
                    ui.label("Peak L:");
                    ui.label(RichText::new(if peak_l_db.is_infinite() { "-inf dB".to_string() } else { format!("{:.1} dB", peak_l_db) }).monospace());
                    ui.end_row();
                    ui.label("Peak R:");
                    ui.label(RichText::new(if peak_r_db.is_infinite() { "-inf dB".to_string() } else { format!("{:.1} dB", peak_r_db) }).monospace());
                    ui.end_row();
                });

                // Spectrum display
                draw_spectrum_display(ui, editor.time, bus_idx, bus.volume * if is_muted { 0.0 } else { 1.0 });

                // Effect chain summary
                if !bus.effects.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("Effect Chain:").small().color(Color32::GRAY));
                    ui.horizontal_wrapped(|ui| {
                        for eff in &bus.effects {
                            let (eff_rect, _) = ui.allocate_exact_size(Vec2::new(36.0, 18.0), egui::Sense::hover());
                            ui.painter().rect_filled(eff_rect, 2.0, Color32::from_rgb(30, 35, 45));
                            ui.painter().rect_stroke(eff_rect, 2.0, Stroke::new(1.0, eff.color()), egui::StrokeKind::Inside);
                            ui.painter().text(eff_rect.center(), egui::Align2::CENTER_CENTER, eff.label(), FontId::proportional(9.0), eff.color());
                        }
                    });
                }

                // Waveform / clip info
                let clips = editor.clips_for_bus(bus_idx);
                if !clips.is_empty() {
                    ui.separator();
                    ui.label(RichText::new(format!("{} assigned clip(s):", clips.len())).small().color(Color32::GRAY));
                    for ci in clips.iter().take(3) {
                        let clip = &editor.clips[*ci];
                        ui.label(RichText::new(format!("  {} ({})", clip.name, clip.duration_str())).small().color(Color32::from_rgb(150, 180, 200)));
                    }
                    if clips.len() > 3 {
                        ui.label(RichText::new(format!("  ...and {} more", clips.len() - 3)).small().color(Color32::GRAY));
                    }
                }
            });
    }
}

// ---- Audio session summary ----

pub fn show_session_summary(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    egui::CollapsingHeader::new(RichText::new("Session Summary").color(Color32::from_rgb(150, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let total_clips = editor.clips.len();
            let total_duration: f32 = editor.clips.iter().map(|c| c.duration).sum();
            let stereo_clips = editor.clips.iter().filter(|c| c.channels > 1).count();
            let looped_clips = editor.clips.iter().filter(|c| c.loop_start.is_some()).count();
            let total_autos = editor.automation.len();
            let total_fx = editor.total_effect_count();

            egui::Grid::new("session_grid").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Total Clips:");
                ui.label(format!("{}", total_clips));
                ui.end_row();
                ui.label("Total Duration:");
                let mins = (total_duration / 60.0) as u32;
                let secs = total_duration as u32 % 60;
                ui.label(format!("{}:{:02}", mins, secs));
                ui.end_row();
                ui.label("Stereo Clips:");
                ui.label(format!("{}", stereo_clips));
                ui.end_row();
                ui.label("Looped Clips:");
                ui.label(format!("{}", looped_clips));
                ui.end_row();
                ui.label("Automation Lanes:");
                ui.label(format!("{}", total_autos));
                ui.end_row();
                ui.label("Total FX Inserts:");
                ui.label(format!("{}", total_fx));
                ui.end_row();
                ui.label("Snapshots:");
                ui.label(format!("{}", editor.snapshots.len()));
                ui.end_row();
                ui.label("BPM:");
                if editor.bpm_clock.enabled {
                    ui.label(RichText::new(format!("{:.1} ({}/{})", editor.bpm_clock.bpm, editor.bpm_clock.numerator, editor.bpm_clock.denominator)).color(Color32::from_rgb(100, 200, 255)));
                } else {
                    ui.label(RichText::new("Disabled").color(Color32::GRAY));
                }
                ui.end_row();
            });
        });
}

// ---- Mixer channel color picker ----

pub fn show_bus_color_picker(ui: &mut egui::Ui, bus: &mut AudioBus) {
    ui.horizontal(|ui| {
        ui.label("Channel Color:");
        let mut rgb = [bus.color.r() as f32 / 255.0, bus.color.g() as f32 / 255.0, bus.color.b() as f32 / 255.0];
        if ui.color_edit_button_rgb(&mut rgb).changed() {
            bus.color = Color32::from_rgb((rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8);
        }

        // Quick preset colors
        let presets = [
            Color32::from_rgb(100, 180, 255),
            Color32::from_rgb(100, 220, 150),
            Color32::from_rgb(220, 150, 100),
            Color32::from_rgb(180, 100, 255),
            Color32::from_rgb(220, 200, 80),
            Color32::from_rgb(200, 100, 100),
        ];
        for preset in &presets {
            let (dot_rect, resp) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::click());
            ui.painter().circle_filled(dot_rect.center(), 7.0, *preset);
            if resp.clicked() { bus.color = *preset; }
        }
    });
}

// ---- Full mixer window ----

pub fn show_full_mixer(ctx: &egui::Context, editor: &mut AudioMixerEditor, dt: f32, open: &mut bool) {
    egui::Window::new("Audio Mixer — Full")
        .open(open)
        .resizable(true)
        .default_size([1400.0, 800.0])
        .min_size([900.0, 500.0])
        .show(ctx, |ui| {
            egui::TopBottomPanel::top("mixer_full_top").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Audio Mixer").size(16.0).color(Color32::from_rgb(100, 200, 255)));
                    ui.separator();
                    show_master_controls(ui, editor);
                    ui.separator();
                    show_bus_stats(ui, editor);
                });
            });

            egui::TopBottomPanel::bottom("mixer_full_bottom").show_inside(ui, |ui| {
                if editor.bpm_clock.enabled {
                    let beat_dur = 60.0 / editor.bpm_clock.bpm;
                    let beat_num = (editor.time / beat_dur) as u32;
                    let beat_frac = (editor.time / beat_dur).fract();
                    let bar = beat_num / editor.bpm_clock.numerator as u32;
                    let beat_in_bar = beat_num % editor.bpm_clock.numerator as u32;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Bar {} | Beat {} | {:.0}%",
                            bar + 1, beat_in_bar + 1, beat_frac * 100.0
                        )).monospace().color(Color32::from_rgb(150, 200, 255)));
                        // Beat flash
                        let flash = (1.0 - beat_frac).powi(4);
                        let (flash_rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                        let flash_color = Color32::from_rgba_unmultiplied(
                            (flash * 255.0) as u8, (flash * 200.0) as u8, 50, 255
                        );
                        ui.painter().circle_filled(flash_rect.center(), 6.0, flash_color);
                    });
                }
            });

            egui::SidePanel::left("mixer_full_bus_detail")
                .resizable(true)
                .default_width(220.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("mixer_full_detail_scroll").show(ui, |ui| {
                        show_bus_detail_overlay(ui, editor);
                        ui.separator();
                        show_session_summary(ui, editor);
                        ui.separator();
                        show_routing_matrix(ui, &editor.buses);
                        ui.separator();
                        show_clip_timeline_mini(ui, editor);
                    });
                });

            show(ui, editor, dt);
        });
}

// ---- Effect type description texts ----

pub fn effect_description(effect: &AudioEffect) -> &'static str {
    match effect {
        AudioEffect::Eq { .. } => "3-band parametric equalizer. Shape the tonal character by boosting or cutting specific frequency ranges.",
        AudioEffect::Reverb { .. } => "Algorithmic reverb simulating acoustic spaces. Control room size, damping, and wet/dry mix.",
        AudioEffect::Delay { .. } => "Stereo delay with independent left/right times. Can sync to project BPM for rhythmic effects.",
        AudioEffect::Compressor { .. } => "Dynamic range compressor. Reduces loud peaks and brings up quiet passages for consistent levels.",
        AudioEffect::Limiter { .. } => "Brick-wall limiter preventing signal from exceeding the threshold. Essential for mastering.",
        AudioEffect::Chorus { .. } => "Modulation effect creating a thickened, ensemble sound by duplicating and slightly detuning the signal.",
        AudioEffect::Distortion { .. } => "Harmonic saturation and hard clipping. Adds grit and character, especially to guitar and bass.",
        AudioEffect::Lowpass { .. } => "Removes frequencies above the cutoff. Useful for filtering out harshness or creating sweeping effects.",
        AudioEffect::Highpass { .. } => "Removes frequencies below the cutoff. Eliminates rumble, mud, and low-frequency noise.",
        AudioEffect::Bitcrusher { .. } => "Reduces bit depth and sample rate for lo-fi digital degradation effects.",
    }
}

// ---- Bus routing validator ----

pub fn validate_routing(buses: &[AudioBus]) -> Vec<String> {
    let mut warnings = Vec::new();
    for (i, bus) in buses.iter().enumerate() {
        if let Some(parent) = bus.parent {
            if parent >= buses.len() {
                warnings.push(format!("Bus '{}' has invalid parent index {}", bus.name, parent));
            } else if parent == i {
                warnings.push(format!("Bus '{}' has itself as parent (loop!)", bus.name));
            }
        }
        for send in &bus.sends {
            if send.target_bus >= buses.len() {
                warnings.push(format!("Bus '{}' has invalid send target {}", bus.name, send.target_bus));
            }
            if send.target_bus == i {
                warnings.push(format!("Bus '{}' sends to itself", bus.name));
            }
        }
        if bus.volume > 2.0 {
            warnings.push(format!("Bus '{}' has volume > +6dB (potential clipping)", bus.name));
        }
    }
    warnings
}

pub fn show_routing_validator(ui: &mut egui::Ui, buses: &[AudioBus]) {
    let warnings = validate_routing(buses);
    if warnings.is_empty() { return; }

    egui::CollapsingHeader::new(RichText::new(format!("Routing Warnings ({})", warnings.len())).color(Color32::from_rgb(255, 200, 50)))
        .default_open(false)
        .show(ui, |ui| {
            for w in &warnings {
                ui.label(RichText::new(format!("! {}", w)).small().color(Color32::from_rgb(255, 200, 50)));
            }
        });
}

// ---- Volume automation visualizer ----

pub fn show_automation_overview(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    egui::CollapsingHeader::new(RichText::new("Automation Overview").color(Color32::from_rgb(180, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            if editor.automation.is_empty() {
                ui.label(RichText::new("No automation lanes.").color(Color32::GRAY));
                return;
            }

            egui::Grid::new("auto_overview_grid").num_columns(4).striped(true).show(ui, |ui| {
                ui.strong("Bus");
                ui.strong("Param");
                ui.strong("Points");
                ui.strong("Range");
                ui.end_row();

                for auto in &editor.automation {
                    let bus_name = editor.buses.get(auto.bus_idx).map(|b| b.name.as_str()).unwrap_or("?");
                    ui.label(RichText::new(bus_name).small());
                    ui.label(RichText::new(auto.parameter.label()).small().color(Color32::from_rgb(150, 200, 255)));
                    ui.label(format!("{}", auto.curve.len()));
                    if !auto.curve.is_empty() {
                        let min_v = auto.curve.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min);
                        let max_v = auto.curve.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);
                        ui.label(RichText::new(format!("{:.2}–{:.2}", min_v, max_v)).small().color(Color32::GRAY));
                    } else {
                        ui.label("-");
                    }
                    ui.end_row();
                }
            });
        });
}

// ---- Clip duplicate and trim ----

impl AudioMixerEditor {
    pub fn duplicate_clip(&mut self, idx: usize) {
        if idx >= self.clips.len() { return; }
        let mut new_clip = self.clips[idx].clone();
        new_clip.name = format!("{} (copy)", new_clip.name);
        self.clips.push(new_clip);
    }

    pub fn trim_clip(&mut self, idx: usize, start: f32, end: f32) {
        if idx >= self.clips.len() { return; }
        let clip = &mut self.clips[idx];
        let start = start.clamp(0.0, clip.duration);
        let end = end.clamp(start, clip.duration);
        clip.duration = end - start;
        if let Some(ls) = clip.loop_start.as_mut() { *ls = (*ls - start).max(0.0); }
        if let Some(le) = clip.loop_end.as_mut() { *le = (*le - start).clamp(0.0, clip.duration); }
    }

    pub fn normalize_clip_volume(&mut self, bus_idx: usize) {
        if bus_idx < self.buses.len() {
            let cur = self.buses[bus_idx].volume;
            self.buses[bus_idx].volume = cur / cur.max(0.001);
        }
    }

    pub fn find_clipping_buses(&self) -> Vec<usize> {
        (0..self.buses.len()).filter(|&i| self.is_clipping(i)).collect()
    }

    pub fn total_bus_sends(&self) -> usize {
        self.buses.iter().map(|b| b.sends.len()).sum()
    }

    pub fn export_bus_hierarchy(&self) -> String {
        let mut lines = Vec::new();
        fn print_bus(buses: &[AudioBus], idx: usize, depth: usize, lines: &mut Vec<String>) {
            let indent = "  ".repeat(depth);
            let bus = &buses[idx];
            let mute = if bus.muted { " [M]" } else { "" };
            let solo = if bus.soloed { " [S]" } else { "" };
            let db = if bus.volume <= 0.0 { "-inf".to_string() } else { format!("{:.1}dB", 20.0 * bus.volume.log10()) };
            lines.push(format!("{}{} ({}{}{})", indent, bus.name, db, mute, solo));
            for (i, b) in buses.iter().enumerate() {
                if b.parent == Some(idx) {
                    print_bus(buses, i, depth + 1, lines);
                }
            }
        }
        for (i, bus) in self.buses.iter().enumerate() {
            if bus.parent.is_none() {
                print_bus(&self.buses, i, 0, &mut lines);
            }
        }
        lines.join("\n")
    }
}

// ---- Mixer snapshot diff ----

pub fn snapshot_diff(a: &MixerSnapshot, b: &MixerSnapshot) -> Vec<String> {
    let mut diffs = Vec::new();
    let all_keys: HashSet<usize> = a.bus_volumes.keys().chain(b.bus_volumes.keys()).copied().collect();
    for key in &all_keys {
        let va = a.bus_volumes.get(key).copied().unwrap_or(1.0);
        let vb = b.bus_volumes.get(key).copied().unwrap_or(1.0);
        if (va - vb).abs() > 0.001 {
            diffs.push(format!("Bus {}: {:.2} -> {:.2}", key, va, vb));
        }
    }
    diffs
}

pub fn show_snapshot_diff(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    if editor.snapshots.len() < 2 { return; }
    egui::CollapsingHeader::new(RichText::new("Snapshot Diff").color(Color32::from_rgb(200, 180, 100)))
        .default_open(false)
        .show(ui, |ui| {
            let diff = snapshot_diff(&editor.snapshots[0], &editor.snapshots[editor.snapshots.len()-1]);
            if diff.is_empty() {
                ui.label(RichText::new("No differences between first and last snapshot.").color(Color32::GRAY));
            } else {
                for d in &diff {
                    ui.label(RichText::new(d).small().color(Color32::from_rgb(180, 200, 255)));
                }
            }
        });
}

// ---- Large-scale clip analysis ----

pub fn clips_by_bus(editor: &AudioMixerEditor) -> HashMap<usize, Vec<usize>> {
    let mut map: HashMap<usize, Vec<usize>> = HashMap::new();
    for (ci, clip) in editor.clips.iter().enumerate() {
        if let Some(bus_idx) = clip.assigned_bus {
            map.entry(bus_idx).or_default().push(ci);
        }
    }
    map
}

pub fn show_clip_bus_distribution(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    egui::CollapsingHeader::new("Clips per Bus")
        .default_open(false)
        .show(ui, |ui| {
            let map = clips_by_bus(editor);
            if map.is_empty() {
                ui.label(RichText::new("No clips assigned.").color(Color32::GRAY));
                return;
            }
            let max_count = map.values().map(|v| v.len()).max().unwrap_or(1) as f32;
            let bar_max = 120.0_f32;
            for (bus_idx, clip_indices) in &map {
                let bus_name = editor.buses.get(*bus_idx).map(|b| b.name.clone()).unwrap_or_else(|| format!("Bus {}", bus_idx));
                let bus_color = editor.buses.get(*bus_idx).map(|b| b.color).unwrap_or(Color32::GRAY);
                let count = clip_indices.len();
                ui.horizontal(|ui| {
                    ui.add_sized(Vec2::new(70.0, 14.0), egui::Label::new(RichText::new(&bus_name).small().color(bus_color)));
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_max, 14.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(25, 25, 30));
                    let fill_w = (count as f32 / max_count) * bar_max;
                    let fill = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
                    ui.painter().rect_filled(fill, 2.0, bus_color);
                    ui.label(RichText::new(format!("{}", count)).small().color(Color32::GRAY));
                });
            }
        });
}

// ---- Stereo field display ----

pub fn draw_stereo_field(ui: &mut egui::Ui, pan: f32, level: f32) {
    let size = 80.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), egui::Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(rect.center(), size / 2.0, Color32::from_rgb(15, 15, 20));
    painter.circle_stroke(rect.center(), size / 2.0, Stroke::new(1.0, Color32::from_rgb(40, 40, 50)));

    // Center cross
    painter.line_segment([Pos2::new(rect.center().x, rect.min.y + 4.0), Pos2::new(rect.center().x, rect.max.y - 4.0)], Stroke::new(0.5, Color32::from_rgb(40, 40, 50)));
    painter.line_segment([Pos2::new(rect.min.x + 4.0, rect.center().y), Pos2::new(rect.max.x - 4.0, rect.center().y)], Stroke::new(0.5, Color32::from_rgb(40, 40, 50)));

    // Ball position
    let ball_x = rect.center().x + pan * (size / 2.0 - 8.0);
    let ball_y = rect.center().y - level * (size / 2.0 - 8.0) * 0.5;
    let ball_pos = Pos2::new(ball_x, ball_y.clamp(rect.min.y + 6.0, rect.max.y - 6.0));
    painter.circle_filled(ball_pos, 6.0, Color32::from_rgb(100, 200, 255));

    // Labels
    painter.text(rect.min + Vec2::new(4.0, 4.0), egui::Align2::LEFT_TOP, "L", FontId::proportional(8.0), Color32::GRAY);
    painter.text(rect.max - Vec2::new(4.0, 4.0), egui::Align2::RIGHT_BOTTOM, "R", FontId::proportional(8.0), Color32::GRAY);
}

// ---- Effect bypass toggle visual ----

pub fn draw_effect_bypass_button(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    let size = Vec2::new(40.0, 18.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let bg = Color32::from_rgb(30, 35, 45);
    ui.painter().rect_filled(rect, 3.0, bg);
    ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, color), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(9.0), color);
    resp.clicked()
}

// ---- Plugin-style effect header ----

pub fn draw_effect_header(ui: &mut egui::Ui, effect: &AudioEffect) {
    let color = effect.color();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3.0, Color32::from_rgba_unmultiplied(color.r() / 3, color.g() / 3, color.b() / 3, 255));
    ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.5, color), egui::StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("[ {} ]", effect.label()),
        FontId::proportional(12.0),
        color,
    );
    ui.painter().text(
        rect.min + Vec2::new(8.0, 12.0),
        egui::Align2::LEFT_CENTER,
        effect_description(effect).chars().take(50).collect::<String>(),
        FontId::proportional(8.0),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 180),
    );
}

// ---- Mix console compact view ----

pub fn show_compact_mix_console(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    egui::CollapsingHeader::new(RichText::new("Compact Console").color(Color32::from_rgb(180, 180, 255)))
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("compact_console_scroll").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for bi in 0..editor.buses.len() {
                        let bus_color = editor.buses[bi].color;
                        let bus_name = editor.buses[bi].name.clone();
                        let is_muted = editor.buses[bi].muted;
                        let is_soloed = editor.buses[bi].soloed;
                        let vol = editor.buses[bi].volume;
                        let pan = editor.buses[bi].pan;

                        let strip_w = 55.0;
                        let (strip_rect, strip_resp) = ui.allocate_exact_size(Vec2::new(strip_w, 120.0), egui::Sense::click());
                        let painter = ui.painter();

                        let bg = if editor.selected_bus == Some(bi) { Color32::from_rgb(35, 45, 60) } else { Color32::from_rgb(22, 22, 30) };
                        painter.rect_filled(strip_rect, 3.0, bg);
                        painter.rect_stroke(strip_rect, 3.0, Stroke::new(if editor.selected_bus == Some(bi) { 1.5 } else { 0.5 }, bus_color), egui::StrokeKind::Inside);

                        // Name
                        let name_short = bus_name.chars().take(6).collect::<String>();
                        painter.text(
                            Pos2::new(strip_rect.center().x, strip_rect.min.y + 10.0),
                            egui::Align2::CENTER_CENTER,
                            name_short,
                            FontId::proportional(9.0),
                            bus_color,
                        );

                        // VU mini meter
                        let vu_l = editor.vu_meters.get(&bi).map(|v| v.0).unwrap_or(0.0);
                        let vu_h = vu_l * 60.0;
                        let vu_rect = Rect::from_min_size(
                            Pos2::new(strip_rect.min.x + 8.0, strip_rect.max.y - 15.0 - vu_h),
                            Vec2::new(strip_w - 16.0, vu_h),
                        );
                        let vu_color = vu_color_for_level(vu_l);
                        painter.rect_filled(vu_rect, 1.0, vu_color);

                        // Volume text
                        let db_s = if vol <= 0.0 { "-inf".to_string() } else { format!("{:.0}", linear_to_db(vol)) };
                        painter.text(
                            Pos2::new(strip_rect.center().x, strip_rect.max.y - 5.0),
                            egui::Align2::CENTER_BOTTOM,
                            db_s,
                            FontId::proportional(8.0),
                            Color32::from_rgb(150, 200, 150),
                        );

                        // Mute indicator
                        if is_muted {
                            painter.text(
                                Pos2::new(strip_rect.min.x + 8.0, strip_rect.min.y + 22.0),
                                egui::Align2::LEFT_TOP,
                                "M",
                                FontId::proportional(8.0),
                                Color32::from_rgb(255, 180, 50),
                            );
                        }
                        if is_soloed {
                            painter.text(
                                Pos2::new(strip_rect.max.x - 8.0, strip_rect.min.y + 22.0),
                                egui::Align2::RIGHT_TOP,
                                "S",
                                FontId::proportional(8.0),
                                Color32::from_rgb(255, 220, 50),
                            );
                        }

                        if strip_resp.clicked() {
                            editor.selected_bus = Some(bi);
                        }

                        ui.add_space(2.0);
                    }
                });
            });
        });
}

// ---- BPM tap tempo ----

pub struct TapTempoState {
    pub tap_times: Vec<f64>,
    pub max_taps: usize,
}

impl Default for TapTempoState {
    fn default() -> Self {
        TapTempoState { tap_times: Vec::new(), max_taps: 8 }
    }
}

impl TapTempoState {
    pub fn tap(&mut self, current_time: f64) {
        self.tap_times.push(current_time);
        if self.tap_times.len() > self.max_taps {
            self.tap_times.remove(0);
        }
    }

    pub fn calculated_bpm(&self) -> Option<f32> {
        if self.tap_times.len() < 2 { return None; }
        let intervals: Vec<f64> = self.tap_times.windows(2)
            .map(|w| w[1] - w[0])
            .filter(|&d| d > 0.1 && d < 3.0)
            .collect();
        if intervals.is_empty() { return None; }
        let avg = intervals.iter().sum::<f64>() / intervals.len() as f64;
        Some((60.0 / avg) as f32)
    }

    pub fn reset(&mut self) {
        self.tap_times.clear();
    }
}

pub fn show_tap_tempo(ui: &mut egui::Ui, tap_state: &mut TapTempoState, bpm_clock: &mut BpmClock) {
    ui.horizontal(|ui| {
        if ui.add_sized(Vec2::new(80.0, 30.0), egui::Button::new(RichText::new("TAP").strong())).clicked() {
            let t = ui.input(|i| i.time);
            tap_state.tap(t);
            if let Some(bpm) = tap_state.calculated_bpm() {
                bpm_clock.bpm = bpm.clamp(20.0, 300.0);
            }
        }
        if let Some(bpm) = tap_state.calculated_bpm() {
            ui.label(RichText::new(format!("{:.1} BPM", bpm)).color(Color32::from_rgb(100, 200, 255)));
        } else {
            ui.label(RichText::new("Tap to set BPM").color(Color32::GRAY).small());
        }
        if ui.small_button("Reset").clicked() { tap_state.reset(); }
    });
}

// ============================================================
// MIDI LEARN SYSTEM
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MidiTarget {
    Volume,
    Pan,
    Send(usize),
    EffectParam { effect_index: usize, param_name: String },
    Mute,
    Solo,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MidiCurve {
    Linear,
    Exponential,
    Logarithmic,
    SCurve,
}

impl MidiCurve {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            MidiCurve::Linear => t,
            MidiCurve::Exponential => t * t,
            MidiCurve::Logarithmic => t.sqrt(),
            MidiCurve::SCurve => t * t * (3.0 - 2.0 * t),
        }
    }
    pub fn label(&self) -> &str {
        match self {
            MidiCurve::Linear => "Linear",
            MidiCurve::Exponential => "Exp",
            MidiCurve::Logarithmic => "Log",
            MidiCurve::SCurve => "S-Curve",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidiMapping {
    pub channel: u8,
    pub cc: u8,
    pub target_bus: usize,
    pub target_param: MidiTarget,
    pub min_val: f32,
    pub max_val: f32,
    pub curve: MidiCurve,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MidiLearnState {
    pub mappings: Vec<MidiMapping>,
    pub learning: Option<(usize, MidiTarget)>,
    pub show_window: bool,
    pub last_cc: Option<(u8, u8, u8)>,
}

pub fn show_midi_learn_window(ctx: &egui::Context, state: &mut MidiLearnState, buses: &mut Vec<AudioBus>) {
    if !state.show_window { return; }
    egui::Window::new("MIDI Learn")
        .resizable(true)
        .default_size(Vec2::new(500.0, 400.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("MIDI Mappings");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() { state.show_window = false; }
                    if ui.button("Clear All").clicked() { state.mappings.clear(); }
                });
            });
            ui.separator();
            if let Some((bus_idx, tgt)) = state.learning.clone() {
                let bus_name = buses.get(bus_idx).map(|b| b.name.clone()).unwrap_or_else(|| format!("Bus {}", bus_idx));
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(format!("Learning: {} / {:?}", bus_name, tgt))
                        .color(Color32::from_rgb(255, 180, 50)));
                    if ui.button("Cancel").clicked() { state.learning = None; }
                });
            }
            if let Some((ch, cc, val)) = state.last_cc {
                ui.label(RichText::new(format!("Last MIDI: ch={} cc={} val={}", ch, cc, val))
                    .small().color(Color32::GRAY));
            }
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut to_remove: Option<usize> = None;
                for (i, mapping) in state.mappings.iter_mut().enumerate() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Ch{} CC{}", mapping.channel, mapping.cc))
                                .color(Color32::from_rgb(100, 200, 255)).monospace());
                            ui.separator();
                            let bus_name = buses.get(mapping.target_bus).map(|b| b.name.clone()).unwrap_or_default();
                            ui.label(&bus_name);
                            ui.separator();
                            ui.label(format!("{:?}", mapping.target_param));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("X").clicked() { to_remove = Some(i); }
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.label("Min:");
                            ui.add(egui::DragValue::new(&mut mapping.min_val).speed(0.01).fixed_decimals(2));
                            ui.label("Max:");
                            ui.add(egui::DragValue::new(&mut mapping.max_val).speed(0.01).fixed_decimals(2));
                            ui.label("Curve:");
                            egui::ComboBox::from_id_salt(format!("midi_curve_{}", i))
                                .selected_text(mapping.curve.label())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut mapping.curve, MidiCurve::Linear, "Linear");
                                    ui.selectable_value(&mut mapping.curve, MidiCurve::Exponential, "Exp");
                                    ui.selectable_value(&mut mapping.curve, MidiCurve::Logarithmic, "Log");
                                    ui.selectable_value(&mut mapping.curve, MidiCurve::SCurve, "S-Curve");
                                });
                        });
                    });
                }
                if let Some(idx) = to_remove { state.mappings.remove(idx); }
            });
        });
}

// ============================================================
// LOUDNESS METER (LUFS / True Peak)
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LoudnessMeter {
    pub integrated_lufs: f32,
    pub short_term_lufs: f32,
    pub momentary_lufs: f32,
    pub true_peak_l: f32,
    pub true_peak_r: f32,
    pub correlation: f32,
    pub lra: f32,
    pub history_short: Vec<f32>,
    pub target_lufs: f32,
    pub ceiling_dbtp: f32,
}

impl LoudnessMeter {
    pub fn new() -> Self {
        Self { target_lufs: -14.0, ceiling_dbtp: -1.0, ..Default::default() }
    }
    pub fn simulate_tick(&mut self, time: f64, bus_volume: f32) {
        let base = -23.0 + 20.0 * bus_volume.max(0.0001).log10().max(-60.0);
        let moment = base + 2.0 * (time * 3.7).sin() as f32 + 0.5 * (time * 11.3).sin() as f32;
        self.momentary_lufs = moment;
        self.short_term_lufs = self.short_term_lufs * 0.97 + moment * 0.03;
        self.integrated_lufs = self.integrated_lufs * 0.999 + moment * 0.001;
        self.true_peak_l = (moment + 3.0 + (time * 17.1).sin() as f32 * 2.0).min(0.0);
        self.true_peak_r = (moment + 3.0 + (time * 13.7).sin() as f32 * 2.0).min(0.0);
        self.correlation = 0.8 + 0.2 * (time * 0.5).sin() as f32;
        self.lra = (self.short_term_lufs - self.integrated_lufs).abs().clamp(0.0, 20.0);
        self.history_short.push(self.short_term_lufs);
        if self.history_short.len() > 200 { self.history_short.remove(0); }
    }
    pub fn is_integrated_ok(&self) -> bool { (self.integrated_lufs - self.target_lufs).abs() < 2.0 }
    pub fn is_peak_ok(&self) -> bool { self.true_peak_l < self.ceiling_dbtp && self.true_peak_r < self.ceiling_dbtp }
}

pub fn draw_loudness_meter(ui: &mut egui::Ui, meter: &LoudnessMeter) {
    let desired = Vec2::new(ui.available_width(), 140.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 25));
    let label_w = 90.0;
    let bar_x = rect.left() + label_w;
    let bar_w = rect.width() - label_w - 10.0;
    let range_min = -40.0f32;
    let range_max = 0.0f32;
    let range = range_max - range_min;
    let rows: &[(&str, f32, Color32)] = &[
        ("Momentary", meter.momentary_lufs, Color32::from_rgb(100, 220, 100)),
        ("Short-term", meter.short_term_lufs, Color32::from_rgb(80, 180, 255)),
        ("Integrated", meter.integrated_lufs, Color32::from_rgb(200, 160, 60)),
    ];
    for (i, (label, val, color)) in rows.iter().enumerate() {
        let row_y = rect.top() + 8.0 + i as f32 * 28.0;
        painter.text(Pos2::new(rect.left() + 4.0, row_y + 9.0), egui::Align2::LEFT_CENTER, label, FontId::proportional(11.0), Color32::GRAY);
        let pct = ((val - range_min) / range).clamp(0.0, 1.0);
        let bar_rect = Rect::from_min_size(Pos2::new(bar_x, row_y + 2.0), Vec2::new(bar_w * pct, 18.0));
        painter.rect_filled(bar_rect, 2.0, *color);
        let target_x = bar_x + bar_w * ((meter.target_lufs - range_min) / range).clamp(0.0, 1.0);
        painter.line_segment([Pos2::new(target_x, row_y), Pos2::new(target_x, row_y + 22.0)], Stroke::new(1.5, Color32::from_rgb(255, 200, 50)));
        painter.text(Pos2::new(bar_x + bar_w + 4.0, row_y + 9.0), egui::Align2::LEFT_CENTER, format!("{:.1}", val), FontId::monospace(10.0), *color);
    }
    let row_y = rect.top() + 8.0 + 3.0 * 28.0;
    painter.text(Pos2::new(rect.left() + 4.0, row_y + 9.0), egui::Align2::LEFT_CENTER, "Correlation", FontId::proportional(11.0), Color32::GRAY);
    let corr_x = bar_x + bar_w * 0.5 + bar_w * 0.5 * meter.correlation;
    let corr_bg = Rect::from_min_size(Pos2::new(bar_x, row_y + 2.0), Vec2::new(bar_w, 18.0));
    painter.rect_filled(corr_bg, 2.0, Color32::from_rgb(40, 40, 45));
    let half_x = bar_x + bar_w * 0.5;
    painter.line_segment([Pos2::new(half_x, row_y + 2.0), Pos2::new(half_x, row_y + 20.0)], Stroke::new(1.0, Color32::from_rgb(80, 80, 90)));
    let c_color = if meter.correlation > 0.5 { Color32::from_rgb(80, 200, 80) } else if meter.correlation > 0.0 { Color32::YELLOW } else { Color32::RED };
    painter.line_segment([Pos2::new(half_x, row_y + 10.0), Pos2::new(corr_x, row_y + 10.0)], Stroke::new(6.0, c_color));
    painter.text(Pos2::new(bar_x + bar_w + 4.0, row_y + 9.0), egui::Align2::LEFT_CENTER, format!("{:.2}", meter.correlation), FontId::monospace(10.0), c_color);
    let hist_y = row_y + 26.0;
    let hist_h = rect.bottom() - hist_y - 4.0;
    if hist_h > 10.0 && !meter.history_short.is_empty() {
        let n = meter.history_short.len();
        for (i, val) in meter.history_short.iter().enumerate() {
            let x = bar_x + bar_w * i as f32 / n as f32;
            let pct = ((val - range_min) / range).clamp(0.0, 1.0);
            let y = hist_y + hist_h * (1.0 - pct);
            if i > 0 {
                let prev_val = meter.history_short[i - 1];
                let px = bar_x + bar_w * (i - 1) as f32 / n as f32;
                let py = hist_y + hist_h * (1.0 - ((prev_val - range_min) / range).clamp(0.0, 1.0));
                painter.line_segment([Pos2::new(px, py), Pos2::new(x, y)], Stroke::new(1.0, Color32::from_rgb(80, 180, 255)));
            }
        }
    }
}

pub fn show_loudness_panel(ui: &mut egui::Ui, meter: &mut LoudnessMeter) {
    egui::CollapsingHeader::new(RichText::new("Loudness Meter (LUFS)").color(Color32::from_rgb(80, 180, 255)))
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Target:");
                ui.add(egui::DragValue::new(&mut meter.target_lufs).speed(0.1).suffix(" LUFS").clamp_range(-40.0f32..=-6.0));
                ui.label("Ceiling:");
                ui.add(egui::DragValue::new(&mut meter.ceiling_dbtp).speed(0.1).suffix(" dBTP").clamp_range(-6.0f32..=0.0));
            });
            draw_loudness_meter(ui, meter);
            ui.horizontal(|ui| {
                let ok_lufs = meter.is_integrated_ok();
                let ok_peak = meter.is_peak_ok();
                let (lufs_color, lufs_label) = if ok_lufs { (Color32::from_rgb(80, 200, 80), "LUFS OK") } else { (Color32::RED, "LUFS OUT") };
                let (peak_color, peak_label) = if ok_peak { (Color32::from_rgb(80, 200, 80), "Peak OK") } else { (Color32::RED, "Peak CLIP") };
                ui.label(RichText::new(lufs_label).color(lufs_color).strong());
                ui.label(RichText::new(peak_label).color(peak_color).strong());
                ui.label(RichText::new(format!("LRA: {:.1} LU", meter.lra)).color(Color32::GRAY).small());
            });
        });
}

// ============================================================
// CROSSFADER
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CrossfadeCurve { Linear, ConstantPower, Notch }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Crossfader {
    pub position: f32,
    pub bus_a: usize,
    pub bus_b: usize,
    pub curve: CrossfadeCurve,
    pub label_a: String,
    pub label_b: String,
}

impl Crossfader {
    pub fn new(bus_a: usize, bus_b: usize) -> Self {
        Self { position: 0.5, bus_a, bus_b, curve: CrossfadeCurve::ConstantPower, label_a: "A".to_string(), label_b: "B".to_string() }
    }
    pub fn gain_a(&self) -> f32 {
        let t = 1.0 - self.position;
        match self.curve {
            CrossfadeCurve::Linear => t,
            CrossfadeCurve::ConstantPower => (t * std::f32::consts::FRAC_PI_2).sin(),
            CrossfadeCurve::Notch => if t > 0.5 { 1.0 } else { t * 2.0 },
        }
    }
    pub fn gain_b(&self) -> f32 {
        let t = self.position;
        match self.curve {
            CrossfadeCurve::Linear => t,
            CrossfadeCurve::ConstantPower => (t * std::f32::consts::FRAC_PI_2).sin(),
            CrossfadeCurve::Notch => if t > 0.5 { 1.0 } else { t * 2.0 },
        }
    }
}

pub fn draw_crossfader(ui: &mut egui::Ui, xf: &mut Crossfader) {
    ui.group(|ui| {
        ui.label(RichText::new("Crossfader").strong());
        ui.horizontal(|ui| {
            ui.label(RichText::new(&xf.label_a).color(Color32::from_rgb(100, 180, 255)));
            ui.add(egui::Slider::new(&mut xf.position, 0.0f32..=1.0).show_value(false).text(""));
            ui.label(RichText::new(&xf.label_b).color(Color32::from_rgb(255, 140, 80)));
        });
        ui.horizontal(|ui| {
            ui.label("Curve:");
            ui.radio_value(&mut xf.curve, CrossfadeCurve::Linear, "Linear");
            ui.radio_value(&mut xf.curve, CrossfadeCurve::ConstantPower, "Const Power");
            ui.radio_value(&mut xf.curve, CrossfadeCurve::Notch, "Notch");
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("A: {:.0}%", xf.gain_a() * 100.0)).small().color(Color32::from_rgb(100, 180, 255)));
            ui.label(RichText::new(format!("B: {:.0}%", xf.gain_b() * 100.0)).small().color(Color32::from_rgb(255, 140, 80)));
        });
        let desired = Vec2::new(ui.available_width().min(200.0), 40.0);
        let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_rgb(25, 25, 30));
        let n = 60usize;
        let mut pts_a = Vec::new();
        let mut pts_b = Vec::new();
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let mut tmp = xf.clone();
            tmp.position = t;
            let ga = tmp.gain_a();
            let gb = tmp.gain_b();
            let x = rect.left() + rect.width() * t;
            pts_a.push(Pos2::new(x, rect.bottom() - rect.height() * ga));
            pts_b.push(Pos2::new(x, rect.bottom() - rect.height() * gb));
        }
        for w in pts_a.windows(2) { painter.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(100, 180, 255))); }
        for w in pts_b.windows(2) { painter.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(255, 140, 80))); }
        let pos_x = rect.left() + rect.width() * xf.position;
        painter.line_segment([Pos2::new(pos_x, rect.top()), Pos2::new(pos_x, rect.bottom())], Stroke::new(1.0, Color32::WHITE));
    });
}

// ============================================================
// STEM EXPORT
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ExportFormat { Wav, Flac, Ogg, Mp3 }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum StemExportStatus { Idle, Running, Done, Error(String) }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StemExportConfig {
    pub bus_ids: Vec<usize>,
    pub bus_names: Vec<String>,
    pub export_format: ExportFormat,
    pub sample_rate: u32,
    pub bit_depth: u32,
    pub normalize: bool,
    pub include_master_fx: bool,
    pub export_path: String,
    pub status: StemExportStatus,
    pub progress: f32,
}

impl Default for StemExportConfig {
    fn default() -> Self {
        Self { bus_ids: Vec::new(), bus_names: Vec::new(), export_format: ExportFormat::Wav, sample_rate: 48000, bit_depth: 24, normalize: false, include_master_fx: true, export_path: "./stems".to_string(), status: StemExportStatus::Idle, progress: 0.0 }
    }
}

pub fn show_stem_export(ui: &mut egui::Ui, config: &mut StemExportConfig, buses: &[AudioBus]) {
    egui::CollapsingHeader::new(RichText::new("Stem Export").color(Color32::from_rgb(180, 220, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label("Select buses to export:");
            for (i, bus) in buses.iter().enumerate() {
                let mut checked = config.bus_ids.contains(&i);
                if ui.checkbox(&mut checked, &bus.name).changed() {
                    if checked { config.bus_ids.push(i); config.bus_names.push(bus.name.clone()); }
                    else { config.bus_ids.retain(|&x| x != i); }
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Format:");
                ui.radio_value(&mut config.export_format, ExportFormat::Wav, "WAV");
                ui.radio_value(&mut config.export_format, ExportFormat::Flac, "FLAC");
                ui.radio_value(&mut config.export_format, ExportFormat::Ogg, "OGG");
                ui.radio_value(&mut config.export_format, ExportFormat::Mp3, "MP3");
            });
            ui.horizontal(|ui| {
                ui.label("Sample rate:");
                egui::ComboBox::from_id_salt("stem_sr").selected_text(format!("{} Hz", config.sample_rate))
                    .show_ui(ui, |ui| {
                        for &sr in &[44100u32, 48000, 88200, 96000] {
                            ui.selectable_value(&mut config.sample_rate, sr, format!("{} Hz", sr));
                        }
                    });
                ui.label("Bit depth:");
                egui::ComboBox::from_id_salt("stem_bd").selected_text(format!("{}-bit", config.bit_depth))
                    .show_ui(ui, |ui| {
                        for &bd in &[16u32, 24, 32] {
                            ui.selectable_value(&mut config.bit_depth, bd, format!("{}-bit", bd));
                        }
                    });
            });
            ui.checkbox(&mut config.normalize, "Normalize each stem");
            ui.checkbox(&mut config.include_master_fx, "Include master FX");
            ui.horizontal(|ui| {
                ui.label("Export path:");
                ui.text_edit_singleline(&mut config.export_path);
            });
            match &config.status {
                StemExportStatus::Idle => {
                    if ui.button(RichText::new("Export Stems").strong()).clicked() {
                        config.status = StemExportStatus::Running;
                        config.progress = 0.0;
                    }
                }
                StemExportStatus::Running => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.add(egui::ProgressBar::new(config.progress).text(format!("{:.0}%", config.progress * 100.0)));
                    });
                    config.progress = (config.progress + 0.002).min(1.0);
                    if config.progress >= 1.0 { config.status = StemExportStatus::Done; }
                    ui.ctx().request_repaint();
                }
                StemExportStatus::Done => {
                    ui.label(RichText::new("Export complete!").color(Color32::from_rgb(80, 200, 80)));
                    if ui.small_button("Reset").clicked() { config.status = StemExportStatus::Idle; }
                }
                StemExportStatus::Error(e) => {
                    let msg = format!("Error: {}", e);
                    ui.label(RichText::new(msg).color(Color32::RED));
                    if ui.small_button("Reset").clicked() { config.status = StemExportStatus::Idle; }
                }
            }
        });
}

// ============================================================
// GAIN STAGING ADVISOR
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GainSeverity { Ok, Warning, Critical }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GainEntry {
    pub bus_name: String,
    pub peak_db: f32,
    pub rms_db: f32,
    pub headroom_db: f32,
    pub advice: String,
    pub severity: GainSeverity,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GainStagingReport {
    pub entries: Vec<GainEntry>,
}

impl GainStagingReport {
    pub fn analyze(buses: &[AudioBus]) -> Self {
        let entries = buses.iter().map(|bus| {
            let peak_db = 20.0 * bus.volume.max(0.0001).log10();
            let rms_db = peak_db - 3.5;
            let headroom_db = 0.0 - peak_db;
            let (advice, severity) = if peak_db > -3.0 {
                ("Too hot — reduce gain or add limiting".to_string(), GainSeverity::Critical)
            } else if peak_db < -24.0 {
                ("Too quiet — raise gain for better SNR".to_string(), GainSeverity::Warning)
            } else if headroom_db < 6.0 {
                ("Low headroom — consider gain reduction".to_string(), GainSeverity::Warning)
            } else {
                ("Gain staging OK".to_string(), GainSeverity::Ok)
            };
            GainEntry { bus_name: bus.name.clone(), peak_db, rms_db, headroom_db, advice, severity }
        }).collect();
        Self { entries }
    }
}

pub fn show_gain_staging(ui: &mut egui::Ui, report: &GainStagingReport) {
    egui::CollapsingHeader::new(RichText::new("Gain Staging").color(Color32::from_rgb(200, 180, 80)))
        .default_open(false)
        .show(ui, |ui| {
            for entry in &report.entries {
                let (badge_color, badge) = match entry.severity {
                    GainSeverity::Ok => (Color32::from_rgb(80, 200, 80), "OK"),
                    GainSeverity::Warning => (Color32::YELLOW, "WARN"),
                    GainSeverity::Critical => (Color32::RED, "CRIT"),
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(badge).color(badge_color).strong().small().monospace());
                    ui.label(RichText::new(&entry.bus_name).strong());
                    ui.label(RichText::new(format!("Peak:{:.1}dB RMS:{:.1}dB Hdr:{:.1}dB", entry.peak_db, entry.rms_db, entry.headroom_db)).small().color(Color32::GRAY));
                });
                if entry.severity != GainSeverity::Ok {
                    ui.label(RichText::new(format!("  -> {}", entry.advice)).small().color(badge_color));
                }
            }
        });
}

// ============================================================
// TRACK GROUPS
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TrackGroup {
    pub name: String,
    pub color: [u8; 3],
    pub bus_ids: Vec<usize>,
    pub collapsed: bool,
}

impl TrackGroup {
    pub fn color32(&self) -> Color32 {
        Color32::from_rgb(self.color[0], self.color[1], self.color[2])
    }
}

pub fn show_track_groups(ui: &mut egui::Ui, groups: &mut Vec<TrackGroup>, buses: &[AudioBus]) {
    egui::CollapsingHeader::new(RichText::new("Track Groups").color(Color32::from_rgb(180, 140, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let mut to_remove = None;
            for (gi, group) in groups.iter_mut().enumerate() {
                let gc = group.color32();
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 2.0, gc);
                    ui.text_edit_singleline(&mut group.name);
                    if ui.small_button("X").clicked() { to_remove = Some(gi); }
                    ui.checkbox(&mut group.collapsed, "Collapse");
                });
                let mut color_arr = [group.color[0] as f32 / 255.0, group.color[1] as f32 / 255.0, group.color[2] as f32 / 255.0];
                if ui.color_edit_button_rgb(&mut color_arr).changed() {
                    group.color = [(color_arr[0] * 255.0) as u8, (color_arr[1] * 255.0) as u8, (color_arr[2] * 255.0) as u8];
                }
                ui.label("Members:");
                for (bi, bus) in buses.iter().enumerate() {
                    let mut member = group.bus_ids.contains(&bi);
                    if ui.checkbox(&mut member, &bus.name).changed() {
                        if member { group.bus_ids.push(bi); } else { group.bus_ids.retain(|&x| x != bi); }
                    }
                }
                ui.separator();
            }
            if let Some(idx) = to_remove { groups.remove(idx); }
            if ui.button("+ Add Group").clicked() {
                groups.push(TrackGroup { name: format!("Group {}", groups.len() + 1), color: [180, 140, 80], bus_ids: vec![], collapsed: false });
            }
        });
}

// ============================================================
// PLUGIN SLOT SYSTEM
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PluginType { Instrument, Effect, MidiFx, Analyzer }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginParam {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: String,
    pub automatable: bool,
}

impl PluginParam {
    pub fn new(name: &str, value: f32, min: f32, max: f32, unit: &str) -> Self {
        Self { name: name.to_string(), value, min, max, default: value, unit: unit.to_string(), automatable: true }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginSlot {
    pub name: String,
    pub vendor: String,
    pub plugin_type: PluginType,
    pub enabled: bool,
    pub params: Vec<PluginParam>,
    pub preset_name: String,
    pub latency_samples: u32,
    pub input_channels: u32,
    pub output_channels: u32,
}

pub fn show_plugin_slot(ui: &mut egui::Ui, slot: &mut PluginSlot, index: usize) {
    let header_color = match slot.plugin_type {
        PluginType::Instrument => Color32::from_rgb(100, 200, 255),
        PluginType::Effect => Color32::from_rgb(200, 160, 100),
        PluginType::MidiFx => Color32::from_rgb(180, 100, 220),
        PluginType::Analyzer => Color32::from_rgb(100, 220, 150),
    };
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut slot.enabled, "");
            ui.label(RichText::new(&slot.name).color(header_color).strong());
            ui.label(RichText::new(&slot.vendor).color(Color32::GRAY).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{}ch", slot.output_channels)).small().color(Color32::GRAY));
                ui.label(RichText::new(format!("{}smp", slot.latency_samples)).small().color(Color32::GRAY));
            });
        });
        if !slot.enabled {
            ui.label(RichText::new("(Bypassed)").small().color(Color32::DARK_GRAY));
            return;
        }
        ui.collapsing(format!("Parameters ({})", slot.params.len()), |ui| {
            egui::Grid::new(format!("plugin_params_{}", index))
                .num_columns(3)
                .spacing(Vec2::new(6.0, 2.0))
                .show(ui, |ui| {
                    for param in slot.params.iter_mut() {
                        ui.label(RichText::new(&param.name).small());
                        ui.add(egui::Slider::new(&mut param.value, param.min..=param.max).suffix(&param.unit));
                        if ui.small_button("~").on_hover_text("Reset to default").clicked() { param.value = param.default; }
                        ui.end_row();
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Preset:");
            ui.text_edit_singleline(&mut slot.preset_name);
            if ui.small_button("Save").clicked() {}
            if ui.small_button("Load").clicked() {}
        });
    });
}

pub fn show_plugin_rack(ui: &mut egui::Ui, slots: &mut Vec<PluginSlot>) {
    egui::CollapsingHeader::new(RichText::new("Plugin Rack").color(Color32::from_rgb(200, 180, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let mut to_remove = None;
            let slots_len = slots.len();
            for (i, slot) in slots.iter_mut().enumerate() {
                show_plugin_slot(ui, slot, i);
                ui.horizontal(|ui| {
                    if ui.small_button("^").clicked() && i > 0 {}
                    if ui.small_button("v").clicked() && i < slots_len - 1 {}
                    if ui.small_button("Remove").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(idx) = to_remove { slots.remove(idx); }
            if ui.button("+ Add Plugin").clicked() {
                slots.push(PluginSlot {
                    name: "New Plugin".to_string(), vendor: "Unknown".to_string(), plugin_type: PluginType::Effect,
                    enabled: true, params: vec![PluginParam::new("Gain", 0.0, -24.0, 24.0, "dB"), PluginParam::new("Mix", 1.0, 0.0, 1.0, "")],
                    preset_name: "Default".to_string(), latency_samples: 0, input_channels: 2, output_channels: 2,
                });
            }
        });
}

// ============================================================
// SPECTRUM ANALYZER EXTENDED
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SpectrumAnalyzerState {
    pub bins: Vec<f32>,
    pub peak_hold: Vec<f32>,
    pub peak_hold_time: Vec<f32>,
    pub hold_time: f32,
    pub decay_rate: f32,
    pub log_scale: bool,
    pub fill: bool,
    pub show_peak_hold: bool,
    pub n_bins: usize,
}

impl SpectrumAnalyzerState {
    pub fn new(n_bins: usize) -> Self {
        Self { bins: vec![0.0; n_bins], peak_hold: vec![0.0; n_bins], peak_hold_time: vec![0.0; n_bins], hold_time: 2.0, decay_rate: 12.0, log_scale: true, fill: true, show_peak_hold: true, n_bins, }
    }
    pub fn simulate_tick(&mut self, time: f64, dt: f32) {
        let n = self.bins.len();
        for i in 0..n {
            let freq_norm = i as f64 / n as f64;
            let base = -60.0 + 40.0 * (1.0 - freq_norm) as f32;
            let noise = 5.0 * (time * (17.3 + i as f64 * 0.7)).sin() as f32;
            let target = (base + noise).clamp(-80.0, 0.0);
            if target > self.bins[i] {
                self.bins[i] = target;
            } else {
                self.bins[i] -= self.decay_rate * dt;
                self.bins[i] = self.bins[i].max(-80.0);
            }
            if self.bins[i] > self.peak_hold[i] {
                self.peak_hold[i] = self.bins[i]; self.peak_hold_time[i] = 0.0;
            } else {
                self.peak_hold_time[i] += dt;
                if self.peak_hold_time[i] > self.hold_time { self.peak_hold[i] -= self.decay_rate * 0.5 * dt; }
            }
        }
    }
}

pub fn draw_spectrum_analyzer(ui: &mut egui::Ui, state: &SpectrumAnalyzerState) {
    let desired = Vec2::new(ui.available_width(), 140.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(15, 15, 20));
    let db_min = -80.0f32; let db_max = 0.0f32; let db_range = db_max - db_min;
    for db in [-60.0f32, -48.0, -36.0, -24.0, -12.0, 0.0] {
        let y = rect.bottom() - rect.height() * ((db - db_min) / db_range).clamp(0.0, 1.0);
        painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(0.5, Color32::from_rgb(50, 50, 60)));
        painter.text(Pos2::new(rect.left() + 2.0, y - 1.0), egui::Align2::LEFT_BOTTOM, format!("{}", db as i32), FontId::monospace(8.0), Color32::from_rgb(80, 80, 90));
    }
    let n = state.bins.len();
    if n == 0 { return; }
    for i in 1..n {
        let x1 = if state.log_scale { rect.left() + rect.width() * (i as f32).log2() / (n as f32).log2() } else { rect.left() + rect.width() * (i - 1) as f32 / (n - 1) as f32 };
        let x2 = if state.log_scale { rect.left() + rect.width() * ((i + 1) as f32).log2() / (n as f32).log2() } else { rect.left() + rect.width() * i as f32 / (n - 1) as f32 };
        let y1 = rect.bottom() - rect.height() * ((state.bins[i - 1] - db_min) / db_range).clamp(0.0, 1.0);
        let y2 = rect.bottom() - rect.height() * ((state.bins[i] - db_min) / db_range).clamp(0.0, 1.0);
        painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], Stroke::new(1.5, Color32::from_rgb(80, 180, 255)));
        if state.show_peak_hold {
            let py1 = rect.bottom() - rect.height() * ((state.peak_hold[i - 1] - db_min) / db_range).clamp(0.0, 1.0);
            let py2 = rect.bottom() - rect.height() * ((state.peak_hold[i] - db_min) / db_range).clamp(0.0, 1.0);
            painter.line_segment([Pos2::new(x1, py1), Pos2::new(x2, py2)], Stroke::new(1.0, Color32::from_rgb(255, 160, 60)));
        }
    }
}

pub fn show_spectrum_controls(ui: &mut egui::Ui, state: &mut SpectrumAnalyzerState) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.log_scale, "Log");
        ui.checkbox(&mut state.fill, "Fill");
        ui.checkbox(&mut state.show_peak_hold, "Peak Hold");
        ui.label("Hold:");
        ui.add(egui::DragValue::new(&mut state.hold_time).speed(0.1).suffix("s").clamp_range(0.0f32..=10.0));
        ui.label("Decay:");
        ui.add(egui::DragValue::new(&mut state.decay_rate).speed(0.5).suffix("dB/s").clamp_range(1.0f32..=60.0));
    });
}

// ============================================================
// OSCILLOSCOPE
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OscilloscopeState {
    pub samples_l: Vec<f32>,
    pub samples_r: Vec<f32>,
    pub trigger_level: f32,
    pub time_div: f32,
    pub volt_div: f32,
    pub show_both: bool,
}

impl OscilloscopeState {
    pub fn new() -> Self { Self { trigger_level: 0.0, time_div: 1.0, volt_div: 1.0, show_both: true, ..Default::default() } }
    pub fn simulate_tick(&mut self, time: f64, volume: f32) {
        let n = 256;
        self.samples_l = (0..n).map(|i| {
            let t = time + i as f64 * 0.001;
            volume * (0.4 * (t * 440.0 * std::f64::consts::TAU).sin() as f32 + 0.2 * (t * 880.0 * std::f64::consts::TAU).sin() as f32)
        }).collect();
        self.samples_r = (0..n).map(|i| {
            let t = time + i as f64 * 0.001 + 0.002;
            volume * (0.35 * (t * 440.0 * std::f64::consts::TAU).sin() as f32 + 0.25 * (t * 880.0 * std::f64::consts::TAU).sin() as f32)
        }).collect();
    }
}

pub fn draw_oscilloscope(ui: &mut egui::Ui, state: &OscilloscopeState) {
    let desired = Vec2::new(ui.available_width(), 100.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0, 20, 5));
    for i in 0..=4 {
        let x = rect.left() + rect.width() * i as f32 / 4.0;
        painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(0.5, Color32::from_rgb(0, 60, 0)));
        let y = rect.top() + rect.height() * i as f32 / 4.0;
        painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(0.5, Color32::from_rgb(0, 60, 0)));
    }
    let zero_y = rect.center().y;
    painter.line_segment([Pos2::new(rect.left(), zero_y), Pos2::new(rect.right(), zero_y)], Stroke::new(1.0, Color32::from_rgb(0, 100, 0)));
    let trig_y = zero_y - rect.height() * 0.5 * state.trigger_level;
    painter.line_segment([Pos2::new(rect.left(), trig_y), Pos2::new(rect.left() + 8.0, trig_y)], Stroke::new(1.5, Color32::YELLOW));
    if !state.samples_l.is_empty() {
        let n = state.samples_l.len();
        for channel in 0..if state.show_both { 2 } else { 1 } {
            let samples = if channel == 0 { &state.samples_l } else { &state.samples_r };
            let color = if channel == 0 { Color32::from_rgb(80, 220, 80) } else { Color32::from_rgb(80, 180, 255) };
            for i in 1..n.min(rect.width() as usize) {
                let x1 = rect.left() + (i - 1) as f32 / n as f32 * rect.width();
                let x2 = rect.left() + i as f32 / n as f32 * rect.width();
                let y1 = zero_y - rect.height() * 0.45 * samples[i - 1] / state.volt_div;
                let y2 = zero_y - rect.height() * 0.45 * samples[i] / state.volt_div;
                painter.line_segment([Pos2::new(x1, y1.clamp(rect.top(), rect.bottom())), Pos2::new(x2, y2.clamp(rect.top(), rect.bottom()))], Stroke::new(1.2, color));
            }
        }
    }
    painter.text(Pos2::new(rect.right() - 2.0, rect.top() + 2.0), egui::Align2::RIGHT_TOP, "SCOPE", FontId::monospace(8.0), Color32::from_rgb(0, 120, 0));
}

// ============================================================
// DELAY + REVERB VISUALIZERS
// ============================================================

pub fn draw_delay_network(ui: &mut egui::Ui, delay_l: f32, delay_r: f32, feedback: f32, bpm: f32) {
    let desired = Vec2::new(ui.available_width().min(340.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, Color32::from_rgb(20, 20, 28));
    let in_x = rect.left() + 20.0;
    let out_x = rect.right() - 20.0;
    let mid_y = rect.center().y;
    painter.circle_filled(Pos2::new(in_x, mid_y), 6.0, Color32::from_rgb(100, 200, 255));
    painter.text(Pos2::new(in_x, mid_y - 12.0), egui::Align2::CENTER_CENTER, "IN", FontId::monospace(8.0), Color32::GRAY);
    let l_x = in_x + (out_x - in_x) * 0.35;
    let l_y = mid_y - 20.0;
    painter.circle_filled(Pos2::new(l_x, l_y), 5.0, Color32::from_rgb(180, 100, 255));
    painter.text(Pos2::new(l_x, l_y - 9.0), egui::Align2::CENTER_CENTER, format!("L {:.0}ms", delay_l * 1000.0), FontId::monospace(8.0), Color32::from_rgb(180, 100, 255));
    let r_x = in_x + (out_x - in_x) * 0.35;
    let r_y = mid_y + 20.0;
    painter.circle_filled(Pos2::new(r_x, r_y), 5.0, Color32::from_rgb(100, 255, 160));
    painter.text(Pos2::new(r_x, r_y + 10.0), egui::Align2::CENTER_CENTER, format!("R {:.0}ms", delay_r * 1000.0), FontId::monospace(8.0), Color32::from_rgb(100, 255, 160));
    painter.line_segment([Pos2::new(in_x, mid_y), Pos2::new(l_x, l_y)], Stroke::new(1.5, Color32::from_rgb(180, 100, 255)));
    painter.line_segment([Pos2::new(in_x, mid_y), Pos2::new(r_x, r_y)], Stroke::new(1.5, Color32::from_rgb(100, 255, 160)));
    painter.line_segment([Pos2::new(l_x, l_y), Pos2::new(out_x, mid_y)], Stroke::new(1.5, Color32::from_rgb(180, 100, 255)));
    painter.line_segment([Pos2::new(r_x, r_y), Pos2::new(out_x, mid_y)], Stroke::new(1.5, Color32::from_rgb(100, 255, 160)));
    let fb_color = Color32::from_rgba_premultiplied(255, 180, 50, (feedback * 200.0) as u8);
    let fb_pts: Vec<Pos2> = (0..=20).map(|i| {
        let t = i as f32 / 20.0;
        let x = l_x + (out_x - l_x) * t;
        let y = mid_y - 35.0 - 10.0 * (t * std::f32::consts::PI).sin();
        Pos2::new(x, y)
    }).collect();
    for w in fb_pts.windows(2) { painter.line_segment([w[0], w[1]], Stroke::new(1.5, fb_color)); }
    painter.circle_filled(Pos2::new(out_x, mid_y), 6.0, Color32::from_rgb(255, 160, 80));
    painter.text(Pos2::new(out_x, mid_y - 12.0), egui::Align2::CENTER_CENTER, "OUT", FontId::monospace(8.0), Color32::GRAY);
    let beat_sec = 60.0 / bpm;
    for i in 1..=4u32 {
        let t = (beat_sec * i as f32 / 2.0).min(2.0) / 2.0;
        let x = in_x + (out_x - in_x) * t;
        painter.line_segment([Pos2::new(x, rect.bottom() - 5.0), Pos2::new(x, rect.bottom())], Stroke::new(1.0, Color32::from_rgb(100, 100, 120)));
    }
    painter.text(Pos2::new(in_x + 2.0, rect.bottom() - 3.0), egui::Align2::LEFT_BOTTOM, format!("{:.1} BPM", bpm), FontId::monospace(7.0), Color32::GRAY);
}

pub fn draw_reverb_room(ui: &mut egui::Ui, room_size: f32, damping: f32, pre_delay: f32) {
    let desired = Vec2::new(ui.available_width().min(200.0), 120.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 25));
    let room_w = rect.width() * 0.7 * room_size;
    let room_h = rect.height() * 0.6 * room_size;
    let room_rect = Rect::from_center_size(Pos2::new(rect.center().x, rect.center().y + 5.0), Vec2::new(room_w.max(20.0), room_h.max(15.0)));
    painter.rect_filled(room_rect, 3.0, Color32::from_rgba_premultiplied(60, 100, 180, 80));
    painter.rect_stroke(room_rect, 3.0, Stroke::new(1.5, Color32::from_rgb(80, 130, 220)), egui::StrokeKind::Inside);
    for i in 1..=4 {
        let scale = 1.0 + i as f32 * 0.12 * room_size;
        let r = Rect::from_center_size(room_rect.center(), room_rect.size() * scale);
        let alpha = ((80 - i * 15).max(0) as f32 * (1.0 - damping * 0.8)) as u8;
        painter.rect_stroke(r, 3.0, Stroke::new(0.5, Color32::from_rgba_premultiplied(100, 150, 255, alpha)), egui::StrokeKind::Inside);
    }
    let src = Pos2::new(room_rect.left() + 15.0, room_rect.center().y);
    painter.circle_filled(src, 4.0, Color32::from_rgb(255, 200, 80));
    painter.text(src + Vec2::new(0.0, -10.0), egui::Align2::CENTER_CENTER, "SRC", FontId::monospace(7.0), Color32::GRAY);
    painter.text(Pos2::new(rect.left() + 3.0, rect.top() + 3.0), egui::Align2::LEFT_TOP, format!("PD: {:.0}ms", pre_delay * 1000.0), FontId::monospace(8.0), Color32::from_rgb(150, 200, 255));
    let damp_rect = Rect::from_min_size(Pos2::new(rect.right() - 18.0, rect.top() + 5.0), Vec2::new(12.0, rect.height() - 10.0));
    painter.rect_filled(damp_rect, 2.0, Color32::from_rgb(30, 30, 40));
    let fill_h = damp_rect.height() * damping;
    let fill = Rect::from_min_size(Pos2::new(damp_rect.left(), damp_rect.bottom() - fill_h), Vec2::new(12.0, fill_h));
    painter.rect_filled(fill, 2.0, Color32::from_rgb(80, 160, 200));
    painter.text(Pos2::new(damp_rect.center().x, damp_rect.top() - 2.0), egui::Align2::CENTER_BOTTOM, "D", FontId::monospace(7.0), Color32::GRAY);
}

pub fn draw_compressor_viz(ui: &mut egui::Ui, threshold: f32, ratio: f32, attack: f32, release: f32, time: f64) {
    let desired = Vec2::new(ui.available_width().min(300.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 28));
    let db_min = -60.0f32; let db_max = 0.0f32; let range = db_max - db_min;
    let n = rect.width() as usize;
    let mut prev_in = Pos2::default(); let mut prev_out = Pos2::default();
    for i in 0..n {
        let t = i as f64 / n as f64;
        let in_db = -20.0 + 18.0 * (time * 2.1 + t * 8.0).sin() as f32 + 5.0 * (time * 5.7 + t * 3.0).sin() as f32;
        let gain_reduction = if in_db > threshold { (in_db - threshold) * (1.0 - 1.0 / ratio.max(1.0)) } else { 0.0 };
        let out_db = in_db - gain_reduction;
        let x = rect.left() + i as f32;
        let in_y = rect.bottom() - rect.height() * ((in_db - db_min) / range).clamp(0.0, 1.0);
        let out_y = rect.bottom() - rect.height() * ((out_db - db_min) / range).clamp(0.0, 1.0);
        if i > 0 {
            painter.line_segment([prev_in, Pos2::new(x, in_y)], Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 180, 255, 120)));
            painter.line_segment([prev_out, Pos2::new(x, out_y)], Stroke::new(1.5, Color32::from_rgb(255, 160, 80)));
        }
        prev_in = Pos2::new(x, in_y); prev_out = Pos2::new(x, out_y);
    }
    let thresh_y = rect.bottom() - rect.height() * ((threshold - db_min) / range).clamp(0.0, 1.0);
    painter.line_segment([Pos2::new(rect.left(), thresh_y), Pos2::new(rect.right(), thresh_y)], Stroke::new(1.0, Color32::from_rgb(255, 100, 100)));
    painter.text(Pos2::new(rect.left() + 2.0, thresh_y - 2.0), egui::Align2::LEFT_BOTTOM, format!("Thr {:.0}dB", threshold), FontId::monospace(8.0), Color32::from_rgb(255, 100, 100));
    painter.text(Pos2::new(rect.right() - 2.0, rect.top() + 2.0), egui::Align2::RIGHT_TOP, format!("{:.1}:1 A:{:.0}ms R:{:.0}ms", ratio, attack * 1000.0, release * 1000.0), FontId::monospace(8.0), Color32::GRAY);
}

// ============================================================
// EXTENDED AudioMixerEditor METHODS
// ============================================================

impl AudioMixerEditor {
    pub fn show_panel(ctx: &egui::Context, editor: &mut AudioMixerEditor, dt: f32, open: &mut bool) {
        show_panel(ctx, editor, dt, open);
    }

    pub fn bus_signal_path(&self, bus_id: usize) -> Vec<String> {
        let mut path = Vec::new();
        if let Some(bus) = self.buses.get(bus_id) {
            path.push(bus.name.clone());
            if let Some(parent) = bus.parent { path.extend(self.bus_signal_path(parent)); }
        }
        path
    }

    pub fn reset_all_peak_hold(&mut self) {
        for bus in &mut self.buses { bus.vu_peak_l = 0.0; bus.vu_peak_r = 0.0; }
    }

    pub fn apply_gain_staging_recommendations(&mut self, report: &GainStagingReport) {
        for entry in &report.entries {
            if let Some(bus) = self.buses.iter_mut().find(|b| b.name == entry.bus_name) {
                match entry.severity {
                    GainSeverity::Critical => { bus.volume *= 0.5; }
                    GainSeverity::Warning if entry.peak_db < -24.0 => { bus.volume *= 2.0; }
                    _ => {}
                }
                bus.volume = bus.volume.clamp(0.0, 2.0);
            }
        }
    }

    pub fn count_clipping_buses(&self) -> usize {
        self.buses.iter().filter(|b| b.vu_peak_l > 0.99 || b.vu_peak_r > 0.99).count()
    }

    pub fn buses_with_effects(&self) -> Vec<usize> {
        self.buses.iter().enumerate().filter(|(_, b)| !b.effects.is_empty()).map(|(i, _)| i).collect()
    }

    pub fn copy_effects_to_bus(&mut self, from: usize, to: usize) {
        if from < self.buses.len() && to < self.buses.len() {
            let effects = self.buses[from].effects.clone();
            self.buses[to].effects = effects;
        }
    }

    pub fn mute_all_except(&mut self, keep: usize) {
        for (i, bus) in self.buses.iter_mut().enumerate() { bus.muted = i != keep; }
    }

    pub fn set_all_volumes(&mut self, vol: f32) {
        for bus in &mut self.buses { bus.volume = vol.clamp(0.0, 2.0); }
    }

    pub fn get_master_bus(&self) -> Option<&AudioBus> {
        self.buses.iter().find(|b| b.parent.is_none() && b.name.to_lowercase().contains("master"))
    }

    pub fn effect_count_total(&self) -> usize {
        self.buses.iter().map(|b| b.effects.len()).sum()
    }

    pub fn bus_names(&self) -> Vec<String> {
        self.buses.iter().map(|b| b.name.clone()).collect()
    }

    pub fn automation_parameter_names(&self) -> Vec<String> {
        self.automations.iter().map(|a| format!("Bus {} / {:?}", a.bus_id, a.parameter)).collect()
    }

    pub fn remove_all_effects_from_bus(&mut self, bus_id: usize) {
        if let Some(bus) = self.buses.get_mut(bus_id) { bus.effects.clear(); }
    }

    pub fn toggle_bypass_all_effects(&mut self, bus_id: usize) {
        if let Some(bus) = self.buses.get_mut(bus_id) {
            for eff in &mut bus.effects { *eff = eff.clone(); }
        }
    }

    pub fn find_bus_by_name(&self, name: &str) -> Option<usize> {
        self.buses.iter().position(|b| b.name == name)
    }

    pub fn rename_bus(&mut self, bus_id: usize, new_name: &str) {
        if let Some(bus) = self.buses.get_mut(bus_id) { bus.name = new_name.to_string(); }
    }

    pub fn all_leaf_buses(&self) -> Vec<usize> {
        let parent_ids: std::collections::HashSet<usize> = self.buses.iter().filter_map(|b| b.parent).collect();
        (0..self.buses.len()).filter(|i| !parent_ids.contains(i)).collect()
    }

    pub fn total_sends(&self) -> usize {
        self.buses.iter().map(|b| b.sends.len()).sum()
    }

    pub fn snapshot_names(&self) -> Vec<String> {
        self.snapshots.iter().map(|s| s.name.clone()).collect()
    }

    pub fn clear_snapshots(&mut self) { self.snapshots.clear(); }

    pub fn automation_for_bus(&self, bus_id: usize) -> Vec<&Automation> {
        self.automations.iter().filter(|a| a.bus_id == bus_id).collect()
    }

    pub fn remove_automations_for_bus(&mut self, bus_id: usize) {
        self.automations.retain(|a| a.bus_id != bus_id);
    }
}

// ============================================================
// WINDOWS: ANALYSIS, EFFECTS BROWSER, PREFERENCES, SNAPSHOT COMPARE
// ============================================================

pub fn show_analysis_window(ctx: &egui::Context, editor: &mut AudioMixerEditor, open: &mut bool) {
    egui::Window::new("Audio Analysis")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(500.0, 600.0))
        .show(ctx, |ui| {
            let time = ui.input(|i| i.time);
            ui.heading("Audio Analysis");
            ui.separator();
            if let Some(bus) = editor.buses.first() {
                let vol = bus.volume;
                let mut meter = LoudnessMeter::new();
                meter.simulate_tick(time, vol);
                show_loudness_panel(ui, &mut meter);
            }
            ui.separator();
            let mut spec = SpectrumAnalyzerState::new(64);
            spec.simulate_tick(time, 0.016);
            show_spectrum_controls(ui, &mut spec);
            draw_spectrum_analyzer(ui, &spec);
            ui.separator();
            let mut scope = OscilloscopeState::new();
            if let Some(bus) = editor.buses.first() { scope.simulate_tick(time, bus.volume); }
            ui.horizontal(|ui| {
                ui.label("Oscilloscope");
                ui.checkbox(&mut scope.show_both, "Both channels");
                ui.label("Trig:");
                ui.add(egui::DragValue::new(&mut scope.trigger_level).speed(0.01).clamp_range(-1.0f32..=1.0));
            });
            draw_oscilloscope(ui, &scope);
        });
}

pub fn show_effects_browser(ctx: &egui::Context, open: &mut bool, selected_bus: &mut Option<usize>, buses: &mut Vec<AudioBus>) {
    egui::Window::new("Effects Browser")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(360.0, 450.0))
        .show(ctx, |ui| {
            ui.heading("Effects Browser");
            ui.separator();
            if buses.is_empty() { ui.label("No buses."); return; }
            ui.horizontal(|ui| {
                ui.label("Target bus:");
                egui::ComboBox::from_id_salt("fx_browser_bus")
                    .selected_text(selected_bus.map(|i| buses.get(i).map(|b| b.name.as_str()).unwrap_or("?")).unwrap_or("None"))
                    .show_ui(ui, |ui| {
                        for (i, bus) in buses.iter().enumerate() {
                            ui.selectable_value(selected_bus, Some(i), &bus.name);
                        }
                    });
            });
            ui.separator();
            let effect_list: Vec<(&str, AudioEffect)> = vec![
                ("EQ (3-Band)", AudioEffect::Eq { low_gain: 0.0, low_freq: 100.0, mid_gain: 0.0, mid_freq: 1000.0, mid_q: 1.0, high_gain: 0.0, high_freq: 8000.0 }),
                ("Reverb", AudioEffect::Reverb { room_size: 0.5, damping: 0.5, wet: 0.3, dry: 0.7, pre_delay: 0.01 }),
                ("Delay", AudioEffect::Delay { time_l: 0.25, time_r: 0.375, feedback: 0.4, wet: 0.3, sync_to_bpm: false }),
                ("Compressor", AudioEffect::Compressor { threshold: -18.0, ratio: 4.0, attack: 0.01, release: 0.1, makeup_gain: 0.0, makeup: 0.0, knee: 2.0 }),
                ("Limiter", AudioEffect::Limiter { threshold: -1.0, release: 0.05, ceiling: -0.1 }),
                ("Chorus", AudioEffect::Chorus { rate: 1.2, depth: 0.5, delay: 0.02, wet: 0.4, voices: 3 }),
                ("Distortion", AudioEffect::Distortion { drive: 0.5, tone: 0.5, wet: 0.5 }),
                ("Lowpass", AudioEffect::Lowpass { cutoff: 8000.0, resonance: 0.5 }),
                ("Highpass", AudioEffect::Highpass { cutoff: 80.0, resonance: 0.3 }),
                ("Bitcrusher", AudioEffect::Bitcrusher { bit_depth: 8, sample_rate: 22050.0, bits: 8, downsample: 2 }),
            ];
            for (name, effect) in effect_list {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).color(Color32::from_rgb(200, 180, 100)));
                    if let Some(bus_idx) = *selected_bus {
                        if ui.small_button("Add").clicked() {
                            if let Some(bus) = buses.get_mut(bus_idx) { bus.effects.push(effect); }
                        }
                    }
                });
            }
        });
}

pub fn show_mixer_preferences(ctx: &egui::Context, open: &mut bool) {
    egui::Window::new("Mixer Preferences")
        .open(open)
        .resizable(false)
        .default_size(Vec2::new(340.0, 300.0))
        .show(ctx, |ui| {
            ui.heading("Preferences");
            ui.separator();
            egui::Grid::new("mixer_prefs")
                .num_columns(2)
                .spacing(Vec2::new(12.0, 6.0))
                .show(ui, |ui| {
                    ui.label("VU meter response:");
                    let mut r = 0.03f32;
                    ui.add(egui::Slider::new(&mut r, 0.005f32..=0.1).logarithmic(true));
                    ui.end_row();
                    ui.label("Peak hold time:");
                    let mut t = 2.0f32;
                    ui.add(egui::Slider::new(&mut t, 0.5f32..=10.0).suffix("s"));
                    ui.end_row();
                    ui.label("Channel strip width:");
                    let mut w = 90.0f32;
                    ui.add(egui::Slider::new(&mut w, 60.0f32..=160.0).suffix("px"));
                    ui.end_row();
                    ui.label("Default fader gain:");
                    let mut d = 1.0f32;
                    ui.add(egui::Slider::new(&mut d, 0.1f32..=2.0));
                    ui.end_row();
                    ui.label("BPM default:");
                    let mut bpm = 120.0f32;
                    ui.add(egui::DragValue::new(&mut bpm).speed(0.5).suffix(" BPM").clamp_range(20.0f32..=300.0));
                    ui.end_row();
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {}
                if ui.button("Reset Defaults").clicked() {}
            });
        });
}

pub fn show_snapshot_compare_window(ctx: &egui::Context, open: &mut bool, snapshots: &[MixerSnapshot]) {
    egui::Window::new("Snapshot Compare")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(500.0, 350.0))
        .show(ctx, |ui| {
            ui.heading("Compare Snapshots");
            if snapshots.len() < 2 { ui.label("Need at least 2 snapshots."); return; }
            let mut a_idx = 0usize; let mut b_idx = 1usize;
            ui.horizontal(|ui| {
                ui.label("A:");
                egui::ComboBox::from_id_salt("snap_a").selected_text(&snapshots[a_idx].name)
                    .show_ui(ui, |ui| { for (i, s) in snapshots.iter().enumerate() { ui.selectable_value(&mut a_idx, i, &s.name); } });
                ui.label("B:");
                egui::ComboBox::from_id_salt("snap_b").selected_text(&snapshots[b_idx].name)
                    .show_ui(ui, |ui| { for (i, s) in snapshots.iter().enumerate() { ui.selectable_value(&mut b_idx, i, &s.name); } });
            });
            ui.separator();
            let snap_a = &snapshots[a_idx]; let snap_b = &snapshots[b_idx];
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("snap_compare").num_columns(4).striped(true).spacing(Vec2::new(8.0, 3.0)).show(ui, |ui| {
                    ui.label(RichText::new("Bus").strong()); ui.label(RichText::new("Vol A").strong()); ui.label(RichText::new("Vol B").strong()); ui.label(RichText::new("Delta").strong()); ui.end_row();
                    let n = snap_a.volumes.len().max(snap_b.volumes.len());
                    for i in 0..n {
                        let va = snap_a.volumes.get(i).copied().unwrap_or(0.0);
                        let vb = snap_b.volumes.get(i).copied().unwrap_or(0.0);
                        let delta = vb - va;
                        let dc = if delta.abs() < 0.01 { Color32::GRAY } else if delta > 0.0 { Color32::from_rgb(100, 200, 100) } else { Color32::from_rgb(200, 100, 100) };
                        ui.label(format!("Bus {}", i)); ui.label(format!("{:.3}", va)); ui.label(format!("{:.3}", vb));
                        ui.label(RichText::new(format!("{:+.3}", delta)).color(dc));
                        ui.end_row();
                    }
                });
            });
        });
}

// ============================================================
// AUTOMATION LANE EDITOR
// ============================================================

pub fn show_automation_lane_editor(ctx: &egui::Context, open: &mut bool, automations: &mut Vec<Automation>, buses: &[AudioBus]) {
    egui::Window::new("Automation Editor")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(700.0, 400.0))
        .show(ctx, |ui| {
            ui.heading("Automation Lanes");
            ui.separator();
            if ui.button("+ Add Automation").clicked() {
                automations.push(Automation {
                    bus_id: 0,
                    bus_idx: 0,
                    parameter: AutomationParameter::Volume,
                    points: vec![(0.0, 1.0), (1.0, 1.0), (2.0, 0.5), (4.0, 1.0)],
                    curve: vec![(0.0, 1.0), (1.0, 1.0)],
                    name: "New Automation".to_string(),
                    enabled: true,
                });
            }
            ui.separator();
            let mut to_remove = None;
            for (ai, auto) in automations.iter_mut().enumerate() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut auto.enabled, "");
                        let bus_name = buses.get(auto.bus_id).map(|b| b.name.as_str()).unwrap_or("?");
                        egui::ComboBox::from_id_salt(format!("auto_bus_{}", ai))
                            .selected_text(bus_name)
                            .show_ui(ui, |ui| {
                                for (bi, bus) in buses.iter().enumerate() {
                                    ui.selectable_value(&mut auto.bus_id, bi, &bus.name);
                                }
                            });
                        egui::ComboBox::from_id_salt(format!("auto_param_{}", ai))
                            .selected_text(format!("{:?}", auto.parameter))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut auto.parameter, AutomationParameter::Volume, "Volume");
                                ui.selectable_value(&mut auto.parameter, AutomationParameter::Pan, "Pan");
                                ui.selectable_value(&mut auto.parameter, AutomationParameter::EffectParam(0, "Send".to_string()), "Send 0");
                            });
                        if ui.small_button("Remove").clicked() { to_remove = Some(ai); }
                    });
                    // Draw automation curve
                    let desired = Vec2::new(ui.available_width(), 60.0);
                    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(22, 22, 30));

                    if !auto.points.is_empty() {
                        let max_t = auto.points.last().map(|p| p.0).unwrap_or(1.0).max(1.0);
                        let min_v = auto.points.iter().map(|p| p.1).fold(f32::MAX, f32::min).min(0.0);
                        let max_v = auto.points.iter().map(|p| p.1).fold(f32::MIN, f32::max).max(1.0);
                        let v_range = (max_v - min_v).max(0.001);

                        for i in 1..auto.points.len() {
                            let (t1, v1) = auto.points[i - 1];
                            let (t2, v2) = auto.points[i];
                            let x1 = rect.left() + rect.width() * t1 / max_t;
                            let x2 = rect.left() + rect.width() * t2 / max_t;
                            let y1 = rect.bottom() - rect.height() * (v1 - min_v) / v_range;
                            let y2 = rect.bottom() - rect.height() * (v2 - min_v) / v_range;
                            painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], Stroke::new(1.5, Color32::from_rgb(100, 200, 255)));
                        }
                        for &(t, v) in &auto.points {
                            let x = rect.left() + rect.width() * t / max_t;
                            let y = rect.bottom() - rect.height() * (v - min_v) / v_range;
                            painter.circle_filled(Pos2::new(x, y), 3.5, Color32::from_rgb(255, 200, 80));
                        }
                        // Zero line
                        let zy = rect.bottom() - rect.height() * (0.0 - min_v) / v_range;
                        painter.line_segment([Pos2::new(rect.left(), zy.clamp(rect.top(), rect.bottom())), Pos2::new(rect.right(), zy.clamp(rect.top(), rect.bottom()))], Stroke::new(0.5, Color32::from_rgb(60, 60, 70)));
                    }

                    if response.clicked() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let max_t = auto.points.last().map(|p| p.0).unwrap_or(4.0).max(1.0);
                            let t = (pos.x - rect.left()) / rect.width() * max_t;
                            let v = 1.0 - (pos.y - rect.top()) / rect.height();
                            auto.points.push((t, v.clamp(0.0, 2.0)));
                            auto.points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                        }
                    }

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} points", auto.points.len())).small().color(Color32::GRAY));
                        if ui.small_button("Clear").clicked() { auto.points.clear(); }
                        if ui.small_button("Reset").clicked() { auto.points = vec![(0.0, 1.0), (4.0, 1.0)]; }
                    });
                });
            }
            if let Some(idx) = to_remove { automations.remove(idx); }
        });
}

// ============================================================
// REVERB IMPULSE RESPONSE LIBRARY
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImpulseResponse {
    pub name: String,
    pub category: IrCategory,
    pub length_ms: f32,
    pub sample_rate: u32,
    pub file_path: String,
    pub preview_shape: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum IrCategory { Hall, Room, Chamber, Plate, Spring, OutdoorSpace, Special }

impl IrCategory {
    pub fn label(&self) -> &str {
        match self {
            Self::Hall => "Hall", Self::Room => "Room", Self::Chamber => "Chamber",
            Self::Plate => "Plate", Self::Spring => "Spring", Self::OutdoorSpace => "Outdoor", Self::Special => "Special",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            Self::Hall => Color32::from_rgb(100, 160, 255),
            Self::Room => Color32::from_rgb(100, 220, 150),
            Self::Chamber => Color32::from_rgb(200, 180, 80),
            Self::Plate => Color32::from_rgb(220, 120, 80),
            Self::Spring => Color32::from_rgb(180, 100, 220),
            Self::OutdoorSpace => Color32::from_rgb(100, 200, 200),
            Self::Special => Color32::from_rgb(255, 100, 180),
        }
    }
}

pub fn builtin_impulse_responses() -> Vec<ImpulseResponse> {
    vec![
        ImpulseResponse { name: "Concert Hall".to_string(), category: IrCategory::Hall, length_ms: 2800.0, sample_rate: 48000, file_path: "ir/hall_concert.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.08).exp()).collect() },
        ImpulseResponse { name: "Cathedral".to_string(), category: IrCategory::Hall, length_ms: 6000.0, sample_rate: 48000, file_path: "ir/hall_cathedral.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.04).exp()).collect() },
        ImpulseResponse { name: "Small Room".to_string(), category: IrCategory::Room, length_ms: 400.0, sample_rate: 48000, file_path: "ir/room_small.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.25).exp()).collect() },
        ImpulseResponse { name: "Bathroom".to_string(), category: IrCategory::Room, length_ms: 600.0, sample_rate: 48000, file_path: "ir/room_bathroom.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.2).exp()).collect() },
        ImpulseResponse { name: "Stone Chamber".to_string(), category: IrCategory::Chamber, length_ms: 1800.0, sample_rate: 48000, file_path: "ir/chamber_stone.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.1).exp()).collect() },
        ImpulseResponse { name: "EMT 140 Plate".to_string(), category: IrCategory::Plate, length_ms: 3200.0, sample_rate: 48000, file_path: "ir/plate_emt140.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.07).exp()).collect() },
        ImpulseResponse { name: "Garage".to_string(), category: IrCategory::Room, length_ms: 900.0, sample_rate: 48000, file_path: "ir/garage.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.15).exp()).collect() },
        ImpulseResponse { name: "Forest".to_string(), category: IrCategory::OutdoorSpace, length_ms: 1200.0, sample_rate: 48000, file_path: "ir/outdoor_forest.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.12).exp()).collect() },
        ImpulseResponse { name: "Cave".to_string(), category: IrCategory::Special, length_ms: 4500.0, sample_rate: 48000, file_path: "ir/cave.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.05).exp() * (i as f32 * 0.3).sin().abs()).collect() },
        ImpulseResponse { name: "Spring Tank".to_string(), category: IrCategory::Spring, length_ms: 1500.0, sample_rate: 48000, file_path: "ir/spring_tank.wav".to_string(), preview_shape: (0..32).map(|i| (-i as f32 * 0.09).exp() * (i as f32 * 0.8).cos().abs()).collect() },
    ]
}

pub fn draw_ir_shape(painter: &Painter, rect: Rect, shape: &[f32]) {
    if shape.is_empty() { return; }
    let n = shape.len();
    let max_v = shape.iter().cloned().fold(0.0f32, f32::max).max(0.001);
    for i in 1..n {
        let x1 = rect.left() + rect.width() * (i - 1) as f32 / n as f32;
        let x2 = rect.left() + rect.width() * i as f32 / n as f32;
        let y1 = rect.bottom() - rect.height() * shape[i - 1] / max_v;
        let y2 = rect.bottom() - rect.height() * shape[i] / max_v;
        painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], Stroke::new(1.0, Color32::from_rgb(100, 200, 255)));
    }
}

pub fn show_ir_browser(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Impulse Response Library").color(Color32::from_rgb(100, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let irs = builtin_impulse_responses();
            let mut filter_cat: Option<IrCategory> = None;
            ui.horizontal(|ui| {
                ui.label("Filter:");
                for cat in &[IrCategory::Hall, IrCategory::Room, IrCategory::Chamber, IrCategory::Plate, IrCategory::Spring, IrCategory::OutdoorSpace, IrCategory::Special] {
                    if ui.small_button(RichText::new(cat.label()).color(cat.color())).clicked() {
                        filter_cat = Some(cat.clone());
                    }
                }
                if ui.small_button("All").clicked() { filter_cat = None; }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for ir in &irs {
                    if let Some(ref cat) = filter_cat {
                        if &ir.category != cat { continue; }
                    }
                    ui.horizontal(|ui| {
                        let (shape_rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 20.0), egui::Sense::hover());
                        let painter = ui.painter_at(shape_rect);
                        painter.rect_filled(shape_rect, 1.0, Color32::from_rgb(25, 25, 30));
                        draw_ir_shape(&painter, shape_rect, &ir.preview_shape);
                        ui.label(RichText::new(&ir.name).color(ir.category.color()));
                        ui.label(RichText::new(ir.category.label()).small().color(Color32::GRAY));
                        ui.label(RichText::new(format!("{:.0}ms", ir.length_ms)).small().color(Color32::GRAY));
                        if ui.small_button("Load").clicked() {}
                    });
                }
            });
        });
}

// ============================================================
// AUDIO BUS NOTES / COMMENTS
// ============================================================

pub fn show_bus_notes(ui: &mut egui::Ui, buses: &mut Vec<AudioBus>) {
    egui::CollapsingHeader::new(RichText::new("Bus Notes").color(Color32::from_rgb(200, 220, 150)))
        .default_open(false)
        .show(ui, |ui| {
            for bus in buses.iter_mut() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&bus.name).strong().small());
                });
                ui.add(egui::TextEdit::multiline(&mut bus.notes).desired_rows(2).hint_text("Notes...").desired_width(f32::INFINITY));
                ui.separator();
            }
        });
}

// ============================================================
// SEND MATRIX
// ============================================================

pub fn show_send_matrix_window(ctx: &egui::Context, open: &mut bool, buses: &mut Vec<AudioBus>) {
    egui::Window::new("Send Matrix")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(600.0, 400.0))
        .show(ctx, |ui| {
            ui.heading("Bus Send Matrix");
            ui.label(RichText::new("Rows = source, Columns = destination. Click cell to add/remove send.").small().color(Color32::GRAY));
            ui.separator();
            if buses.is_empty() { ui.label("No buses."); return; }
            let n = buses.len().min(12);
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("send_matrix_grid").num_columns(n + 1).spacing(Vec2::new(2.0, 2.0)).show(ui, |ui| {
                    ui.label("");
                    for j in 0..n {
                        let name = buses[j].name.chars().take(6).collect::<String>();
                        ui.label(RichText::new(name).small().color(Color32::GRAY));
                    }
                    ui.end_row();
                    for i in 0..n {
                        let src_name = buses[i].name.chars().take(8).collect::<String>();
                        ui.label(RichText::new(src_name).small().strong());
                        for j in 0..n {
                            let has_send = buses[i].sends.iter().any(|s| s.target_bus == j);
                            let cell_color = if i == j { Color32::from_rgb(40, 40, 50) }
                                else if has_send { Color32::from_rgb(80, 160, 80) }
                                else { Color32::from_rgb(35, 35, 42) };
                            let (cell_rect, cell_resp) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::click());
                            ui.painter().rect_filled(cell_rect, 2.0, cell_color);
                            if has_send {
                                let send = buses[i].sends.iter().find(|s| s.target_bus == j).unwrap();
                                ui.painter().text(cell_rect.center(), egui::Align2::CENTER_CENTER, format!("{:.0}", send.level * 100.0), FontId::monospace(7.0), Color32::from_rgb(200, 255, 200));
                            }
                            if cell_resp.clicked() && i != j {
                                if has_send {
                                    buses[i].sends.retain(|s| s.target_bus != j);
                                } else {
                                    buses[i].sends.push(BusSend { target_bus: j, amount: 1.0, level: 1.0, pre_fader: false });
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
            });
        });
}

// ============================================================
// CLIP WAVEFORM EDITOR
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveformRegion {
    pub start: f32,
    pub end: f32,
    pub gain: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub color: [u8; 3],
    pub label: String,
    pub muted: bool,
}

impl WaveformRegion {
    pub fn new(start: f32, end: f32, label: &str) -> Self {
        Self { start, end, gain: 1.0, fade_in: 0.0, fade_out: 0.0, color: [80, 150, 220], label: label.to_string(), muted: false }
    }
    pub fn duration(&self) -> f32 { self.end - self.start }
}

pub fn draw_waveform_editor(ui: &mut egui::Ui, clip: &AudioClip, regions: &mut Vec<WaveformRegion>, view_start: f32, view_end: f32) {
    let desired = Vec2::new(ui.available_width(), 80.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 26));

    let view_dur = (view_end - view_start).max(0.001);

    // Fake waveform
    let n = rect.width() as usize;
    let mut prev = Pos2::default();
    for i in 0..n {
        let t = view_start + view_dur * i as f32 / n as f32;
        let sample = (t * 220.0 * std::f32::consts::TAU).sin() * 0.6
            + (t * 440.0 * std::f32::consts::TAU).sin() * 0.25
            + (t * 880.0 * std::f32::consts::TAU).sin() * 0.1;
        let x = rect.left() + i as f32;
        let y = rect.center().y - rect.height() * 0.45 * sample;
        if i > 0 {
            painter.line_segment([prev, Pos2::new(x, y.clamp(rect.top(), rect.bottom()))], Stroke::new(0.8, Color32::from_rgb(80, 160, 220)));
        }
        prev = Pos2::new(x, y.clamp(rect.top(), rect.bottom()));
    }

    // Draw regions
    for region in regions.iter() {
        if region.end < view_start || region.start > view_end { continue; }
        let rx1 = rect.left() + rect.width() * (region.start - view_start) / view_dur;
        let rx2 = rect.left() + rect.width() * (region.end - view_start) / view_dur;
        let rx1 = rx1.clamp(rect.left(), rect.right());
        let rx2 = rx2.clamp(rect.left(), rect.right());
        let rr = Rect::from_min_max(Pos2::new(rx1, rect.top()), Pos2::new(rx2, rect.bottom()));
        let alpha = if region.muted { 40u8 } else { 60 };
        painter.rect_filled(rr, 0.0, Color32::from_rgba_premultiplied(region.color[0], region.color[1], region.color[2], alpha));
        painter.rect_stroke(rr, 0.0, Stroke::new(1.0, Color32::from_rgb(region.color[0], region.color[1], region.color[2])), egui::StrokeKind::Inside);
        painter.text(Pos2::new(rx1 + 2.0, rect.top() + 3.0), egui::Align2::LEFT_TOP, &region.label, FontId::proportional(9.0), Color32::WHITE);
        // Fade in gradient indicator
        if region.fade_in > 0.0 {
            let fi_w = rect.width() * region.fade_in / view_dur;
            let fi_rect = Rect::from_min_max(Pos2::new(rx1, rect.top()), Pos2::new((rx1 + fi_w).min(rx2), rect.bottom()));
            painter.rect_filled(fi_rect, 0.0, Color32::from_rgba_premultiplied(255, 255, 100, 30));
        }
        // Fade out
        if region.fade_out > 0.0 {
            let fo_w = rect.width() * region.fade_out / view_dur;
            let fo_rect = Rect::from_min_max(Pos2::new((rx2 - fo_w).max(rx1), rect.top()), Pos2::new(rx2, rect.bottom()));
            painter.rect_filled(fo_rect, 0.0, Color32::from_rgba_premultiplied(255, 100, 100, 30));
        }
    }

    // Playhead
    let ph_x = rect.left() + rect.width() * 0.3;
    painter.line_segment([Pos2::new(ph_x, rect.top()), Pos2::new(ph_x, rect.bottom())], Stroke::new(1.5, Color32::from_rgb(255, 220, 80)));

    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = view_start + view_dur * (pos.x - rect.left()) / rect.width();
            regions.push(WaveformRegion::new(t, t + 0.5, "New"));
        }
    }

    ui.label(RichText::new(format!("Clip: {}  ({:.2}s)", clip.name, clip.duration)).small().color(Color32::GRAY));
}

pub fn show_waveform_region_list(ui: &mut egui::Ui, regions: &mut Vec<WaveformRegion>) {
    egui::CollapsingHeader::new(RichText::new("Regions").color(Color32::from_rgb(180, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let mut to_remove = None;
            for (i, region) in regions.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut region.muted, "");
                    let mut c = [region.color[0] as f32 / 255.0, region.color[1] as f32 / 255.0, region.color[2] as f32 / 255.0];
                    if ui.color_edit_button_rgb(&mut c).changed() {
                        region.color = [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8];
                    }
                    ui.text_edit_singleline(&mut region.label);
                    ui.add(egui::DragValue::new(&mut region.start).speed(0.01).prefix("s:").clamp_range(0.0f32..=region.end - 0.01));
                    ui.add(egui::DragValue::new(&mut region.end).speed(0.01).prefix("e:").clamp_range(region.start + 0.01..=1000.0));
                    ui.add(egui::DragValue::new(&mut region.gain).speed(0.01).prefix("g:").clamp_range(0.0f32..=2.0));
                    if ui.small_button("X").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(idx) = to_remove { regions.remove(idx); }
            if ui.button("+ Add Region").clicked() {
                let start = regions.last().map(|r| r.end + 0.1).unwrap_or(0.0);
                regions.push(WaveformRegion::new(start, start + 1.0, "Region"));
            }
        });
}

// ============================================================
// MASTER CHAIN EDITOR
// ============================================================

pub fn show_master_chain_editor(ui: &mut egui::Ui, master_effects: &mut Vec<AudioEffect>) {
    egui::CollapsingHeader::new(RichText::new("Master Chain").color(Color32::from_rgb(255, 220, 80)).strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.label(RichText::new("Master bus processing chain — applied to final mix.").small().color(Color32::GRAY));
            if master_effects.is_empty() { ui.label(RichText::new("No effects on master.").small().color(Color32::DARK_GRAY)); }
            let mut to_remove = None;
            let mut to_move_up = None;
            let mut to_move_down = None;
            for (i, eff) in master_effects.iter().enumerate() {
                let eff_name = match eff {
                    AudioEffect::Eq { .. } => "EQ", AudioEffect::Reverb { .. } => "Reverb", AudioEffect::Delay { .. } => "Delay",
                    AudioEffect::Compressor { .. } => "Compressor", AudioEffect::Limiter { .. } => "Limiter",
                    AudioEffect::Chorus { .. } => "Chorus", AudioEffect::Distortion { .. } => "Distortion",
                    AudioEffect::Lowpass { .. } => "Lowpass", AudioEffect::Highpass { .. } => "Highpass",
                    AudioEffect::Bitcrusher { .. } => "Bitcrusher",
                };
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let slot_color = match eff {
                            AudioEffect::Compressor { .. } | AudioEffect::Limiter { .. } => Color32::from_rgb(255, 140, 80),
                            AudioEffect::Eq { .. } => Color32::from_rgb(80, 200, 255),
                            AudioEffect::Reverb { .. } | AudioEffect::Delay { .. } => Color32::from_rgb(180, 100, 255),
                            _ => Color32::from_rgb(180, 200, 100),
                        };
                        ui.label(RichText::new(format!("{}. {}", i + 1, eff_name)).color(slot_color).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").clicked() { to_remove = Some(i); }
                            if ui.small_button("v").clicked() && i < master_effects.len() - 1 { to_move_down = Some(i); }
                            if ui.small_button("^").clicked() && i > 0 { to_move_up = Some(i); }
                        });
                    });
                });
            }
            if let Some(idx) = to_remove { master_effects.remove(idx); }
            if let Some(idx) = to_move_up { master_effects.swap(idx, idx - 1); }
            if let Some(idx) = to_move_down { master_effects.swap(idx, idx + 1); }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.small_button("+ EQ").clicked() { master_effects.push(AudioEffect::Eq { low_gain: 0.0, low_freq: 100.0, mid_gain: 0.0, mid_freq: 1000.0, mid_q: 1.0, high_gain: 0.0, high_freq: 8000.0 }); }
                if ui.small_button("+ Comp").clicked() { master_effects.push(AudioEffect::Compressor { threshold: -12.0, ratio: 2.0, attack: 0.005, release: 0.1, makeup_gain: 0.0, makeup: 0.0, knee: 2.0 }); }
                if ui.small_button("+ Limit").clicked() { master_effects.push(AudioEffect::Limiter { threshold: -0.3, release: 0.05, ceiling: -0.3 }); }
                if ui.small_button("+ Reverb").clicked() { master_effects.push(AudioEffect::Reverb { room_size: 0.3, damping: 0.7, wet: 0.1, dry: 0.9, pre_delay: 0.01 }); }
            });
        });
}

// ============================================================
// AUDIO CLIP PITCH / TIME STRETCH
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PitchTimeStretch {
    pub pitch_semitones: f32,
    pub time_ratio: f32,
    pub algorithm: StretchAlgorithm,
    pub formant_shift: f32,
    pub transient_preserve: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum StretchAlgorithm { Elastique, PhaseVocoder, Granular, Sinusoidal }

impl Default for PitchTimeStretch {
    fn default() -> Self {
        Self { pitch_semitones: 0.0, time_ratio: 1.0, algorithm: StretchAlgorithm::Elastique, formant_shift: 0.0, transient_preserve: true }
    }
}

pub fn show_pitch_time_stretch(ui: &mut egui::Ui, pts: &mut PitchTimeStretch, clip_name: &str) {
    egui::CollapsingHeader::new(RichText::new(format!("Pitch/Time: {}", clip_name)).color(Color32::from_rgb(200, 150, 255)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Algorithm:");
                egui::ComboBox::from_id_salt("pts_algo").selected_text(format!("{:?}", pts.algorithm))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut pts.algorithm, StretchAlgorithm::Elastique, "Elastique");
                        ui.selectable_value(&mut pts.algorithm, StretchAlgorithm::PhaseVocoder, "Phase Vocoder");
                        ui.selectable_value(&mut pts.algorithm, StretchAlgorithm::Granular, "Granular");
                        ui.selectable_value(&mut pts.algorithm, StretchAlgorithm::Sinusoidal, "Sinusoidal");
                    });
            });
            ui.add(egui::Slider::new(&mut pts.pitch_semitones, -24.0f32..=24.0).text("Pitch (semitones)").show_value(true));
            ui.add(egui::Slider::new(&mut pts.time_ratio, 0.25f32..=4.0).text("Time ratio").logarithmic(true));
            ui.add(egui::Slider::new(&mut pts.formant_shift, -12.0f32..=12.0).text("Formant shift"));
            ui.checkbox(&mut pts.transient_preserve, "Preserve transients");
            ui.horizontal(|ui| {
                let cents = (pts.pitch_semitones * 100.0) as i32;
                ui.label(RichText::new(format!("{:+} cents  x{:.3} time", cents, pts.time_ratio)).small().color(Color32::from_rgb(200, 200, 255)));
            });
        });
}

// ============================================================
// EXTENDED PANEL UTILITIES
// ============================================================

pub fn show_audio_mixer_toolbar(ui: &mut egui::Ui, editor: &mut AudioMixerEditor) {
    ui.horizontal(|ui| {
        if ui.button("New Bus").clicked() {
            let n = editor.buses.len();
            editor.buses.push(AudioBus::new(&format!("Bus {}", n + 1)));
        }
        if ui.button("Reset All").clicked() { editor.set_all_volumes(1.0); editor.unmute_all(); }
        if ui.button("Snapshot").clicked() {
            let snap = MixerSnapshot::capture(&format!("Snap {}", editor.snapshots.len() + 1), &editor.buses);
            editor.snapshots.push(snap);
        }
        ui.separator();
        ui.label(RichText::new(format!("{} buses  {} effects  {} clips",
            editor.buses.len(), editor.effect_count_total(), editor.clips.len())).small().color(Color32::GRAY));
    });
}

pub fn draw_bus_hierarchy_tree(ui: &mut egui::Ui, editor: &AudioMixerEditor, parent: Option<usize>, depth: usize) {
    let indent = depth as f32 * 14.0;
    for (i, bus) in editor.buses.iter().enumerate() {
        if bus.parent != parent { continue; }
        let bc = Color32::from_rgb(bus.color[0], bus.color[1], bus.color[2]);
        ui.horizontal(|ui| {
            ui.add_space(indent);
            let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
            ui.painter().circle_filled(dot_r.center(), 4.0, bc);
            let vol_db = 20.0 * bus.volume.max(0.0001).log10();
            ui.label(RichText::new(&bus.name).color(bc).strong().small());
            ui.label(RichText::new(format!("{:.1}dB", vol_db)).small().color(Color32::GRAY));
            if bus.muted { ui.label(RichText::new("M").color(Color32::RED).small().strong()); }
            if bus.soloed { ui.label(RichText::new("S").color(Color32::YELLOW).small().strong()); }
            if !bus.effects.is_empty() { ui.label(RichText::new(format!("[{} fx]", bus.effects.len())).small().color(Color32::from_rgb(180, 160, 80))); }
        });
        draw_bus_hierarchy_tree(ui, editor, Some(i), depth + 1);
    }
}

pub fn show_bus_hierarchy_panel(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    egui::CollapsingHeader::new(RichText::new("Bus Hierarchy").color(Color32::from_rgb(180, 220, 255)))
        .default_open(true)
        .show(ui, |ui| {
            if editor.buses.is_empty() { ui.label("No buses."); return; }
            draw_bus_hierarchy_tree(ui, editor, None, 0);
        });
}

// ============================================================
// AUDIO MIXER FULL SETTINGS WINDOW
// ============================================================

pub fn show_mixer_full_settings(ctx: &egui::Context, open: &mut bool, editor: &mut AudioMixerEditor) {
    egui::Window::new("Mixer Settings")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(420.0, 500.0))
        .show(ctx, |ui| {
            ui.heading("Full Mixer Settings");
            ui.separator();
            egui::Grid::new("mixer_full_settings").num_columns(2).spacing(Vec2::new(12.0, 5.0)).show(ui, |ui| {
                ui.label("BPM:"); ui.add(egui::DragValue::new(&mut editor.bpm_clock.bpm).speed(0.5).clamp_range(20.0f32..=300.0).suffix(" BPM")); ui.end_row();
                ui.label("Time sig:"); ui.label(format!("{}/{}", editor.bpm_clock.numerator, editor.bpm_clock.denominator)); ui.end_row();
                ui.label("Buses:"); ui.label(format!("{}", editor.buses.len())); ui.end_row();
                ui.label("Automations:"); ui.label(format!("{}", editor.automations.len())); ui.end_row();
                ui.label("Clips:"); ui.label(format!("{}", editor.clips.len())); ui.end_row();
                ui.label("Snapshots:"); ui.label(format!("{}", editor.snapshots.len())); ui.end_row();
                ui.label("Effects total:"); ui.label(format!("{}", editor.effect_count_total())); ui.end_row();
            });
            ui.separator();
            ui.label(RichText::new("Bus Signal Paths").strong());
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for (i, _) in editor.buses.iter().enumerate() {
                    let path = editor.bus_signal_path(i);
                    ui.label(RichText::new(path.join(" -> ")).small().color(Color32::from_rgb(180, 200, 180)));
                }
            });
            ui.separator();
            show_gain_staging(ui, &GainStagingReport::analyze(&editor.buses));
        });
}

// ============================================================
// MIXER CLIP LIBRARY
// ============================================================

pub fn show_clip_library(ui: &mut egui::Ui, clips: &mut Vec<AudioClip>) {
    egui::CollapsingHeader::new(RichText::new("Clip Library").color(Color32::from_rgb(200, 220, 150)))
        .default_open(false)
        .show(ui, |ui| {
            if clips.is_empty() { ui.label(RichText::new("No clips.").small().color(Color32::GRAY)); }
            let mut to_remove = None;
            for (i, clip) in clips.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&clip.name).small().strong());
                    ui.label(RichText::new(format!("{:.2}s", clip.duration)).small().color(Color32::GRAY));
                    ui.label(RichText::new(format!("{}Hz", clip.sample_rate)).small().color(Color32::GRAY));
                    ui.add(egui::DragValue::new(&mut clip.gain).speed(0.01).prefix("g:").clamp_range(0.0f32..=2.0));
                    ui.checkbox(&mut clip.looping, "Loop");
                    if ui.small_button("X").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(idx) = to_remove { clips.remove(idx); }
            if ui.button("+ Add Clip").clicked() {
                clips.push(AudioClip {
                    id: clips.len(),
                    name: format!("Clip {}", clips.len() + 1),
                    bus_id: None,
                    start_time: 0.0,
                    duration: 4.0,
                    gain: 1.0,
                    looping: false,
                    file_path: String::new(),
                    path: String::new(),
                    sample_rate: 48000,
                    channels: 2,
                    loop_start: None,
                    loop_end: None,
                    assigned_bus: None,
                });
            }
        });
}

// ============================================================
// AUTOMATION PLAYBACK
// ============================================================

pub fn show_automation_playback(ui: &mut egui::Ui, editor: &mut AudioMixerEditor, playhead: f32) {
    egui::CollapsingHeader::new(RichText::new("Automation Playback").color(Color32::from_rgb(100, 220, 200)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Playhead:");
                let ph = playhead;
                ui.label(RichText::new(format!("{:.2}s", ph)).monospace().color(Color32::from_rgb(255, 220, 80)));
                if ui.small_button("Apply Now").clicked() {
                    editor.apply_automation_at_time(playhead);
                }
            });
            for auto in &editor.automations {
                if !auto.enabled { continue; }
                if let Some(bus) = editor.buses.get(auto.bus_id) {
                    let v = evaluate_curve(&auto.points, playhead);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&bus.name).small().strong());
                        ui.label(RichText::new(format!("{:?}", auto.parameter)).small().color(Color32::GRAY));
                        ui.label(RichText::new(format!("-> {:.3}", v)).small().color(Color32::from_rgb(100, 220, 150)));
                    });
                }
            }
        });
}

// ============================================================
// FULL MIXER PANEL WITH EVERYTHING
// ============================================================

pub fn show_full_mixer_panel(ctx: &egui::Context, editor: &mut AudioMixerEditor, dt: f32, open: &mut bool) {
    egui::Window::new("Full Mixer")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(900.0, 650.0))
        .show(ctx, |ui| {
            show_audio_mixer_toolbar(ui, editor);
            ui.separator();
            egui::SidePanel::left("mixer_sidebar").min_width(200.0).show_inside(ui, |ui| {
                let time = ui.input(|i| i.time);
                editor.update_vu_meters(dt);
                show_bus_hierarchy_panel(ui, editor);
                ui.separator();
                show_clip_library(ui, &mut editor.clips);
            });
            egui::CentralPanel::default().show_inside(ui, |ui| {
                show(ui, editor, dt);
            });
        });
}

// ============================================================
// CHANNEL EQ VISUALIZER (4-BAND PARAMETRIC)
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParametricBand {
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub band_type: EqBandType,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EqBandType { LowShelf, HighShelf, Peak, LowCut, HighCut, Notch }

impl ParametricBand {
    pub fn label(&self) -> &str {
        match self.band_type {
            EqBandType::LowShelf => "LS", EqBandType::HighShelf => "HS",
            EqBandType::Peak => "PK", EqBandType::LowCut => "LC",
            EqBandType::HighCut => "HC", EqBandType::Notch => "NT",
        }
    }
    pub fn color(&self) -> Color32 {
        match self.band_type {
            EqBandType::LowShelf | EqBandType::LowCut => Color32::from_rgb(80, 160, 255),
            EqBandType::HighShelf | EqBandType::HighCut => Color32::from_rgb(255, 140, 80),
            EqBandType::Peak => Color32::from_rgb(100, 220, 150),
            EqBandType::Notch => Color32::from_rgb(255, 100, 180),
        }
    }
    pub fn response_at(&self, freq: f32) -> f32 {
        if !self.enabled { return 0.0; }
        let dist = (freq.log2() - self.freq.log2()).abs();
        let bw = 1.0 / self.q.max(0.01);
        let env = (-dist * dist / (2.0 * bw * bw)).exp();
        self.gain * env
    }
}

pub fn draw_parametric_eq(ui: &mut egui::Ui, bands: &mut Vec<ParametricBand>) {
    let desired = Vec2::new(ui.available_width(), 120.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(16, 16, 22));

    let freq_min = 20.0f32; let freq_max = 20000.0f32;
    let db_min = -18.0f32; let db_max = 18.0f32; let db_range = db_max - db_min;

    // Grid
    for db in [-12.0f32, -6.0, 0.0, 6.0, 12.0] {
        let y = rect.top() + rect.height() * (1.0 - (db - db_min) / db_range);
        let c = if db == 0.0 { Color32::from_rgb(70, 70, 80) } else { Color32::from_rgb(40, 40, 50) };
        painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(0.5, c));
        painter.text(Pos2::new(rect.left() + 2.0, y - 1.0), egui::Align2::LEFT_BOTTOM, format!("{:+}", db as i32), FontId::monospace(7.0), Color32::from_rgb(70, 70, 90));
    }

    // Combined response
    let n = rect.width() as usize;
    let mut pts: Vec<Pos2> = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let freq = freq_min * (freq_max / freq_min).powf(t);
        let total_db: f32 = bands.iter().map(|b| b.response_at(freq)).sum();
        let x = rect.left() + t * rect.width();
        let y = rect.top() + rect.height() * (1.0 - (total_db - db_min) / db_range);
        pts.push(Pos2::new(x, y.clamp(rect.top(), rect.bottom())));
    }
    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(2.0, Color32::from_rgb(200, 220, 255)));
    }

    // Individual bands
    for band in bands.iter() {
        if !band.enabled { continue; }
        let bc = band.color();
        let freq_t = (band.freq.log2() - freq_min.log2()) / (freq_max.log2() - freq_min.log2());
        let freq_x = rect.left() + rect.width() * freq_t.clamp(0.0, 1.0);
        let gain_y = rect.top() + rect.height() * (1.0 - (band.gain - db_min) / db_range);
        painter.circle_filled(Pos2::new(freq_x, gain_y.clamp(rect.top(), rect.bottom())), 4.0, bc);
        painter.text(Pos2::new(freq_x, gain_y - 8.0), egui::Align2::CENTER_CENTER, band.label(), FontId::monospace(8.0), bc);
    }
}

pub fn show_parametric_eq_editor(ui: &mut egui::Ui, bands: &mut Vec<ParametricBand>) {
    egui::CollapsingHeader::new(RichText::new("Parametric EQ").color(Color32::from_rgb(200, 220, 255)))
        .default_open(false)
        .show(ui, |ui| {
            draw_parametric_eq(ui, bands);
            let mut to_remove = None;
            for (i, band) in bands.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut band.enabled, "");
                    ui.label(RichText::new(band.label()).color(band.color()).small().monospace());
                    ui.add(egui::DragValue::new(&mut band.freq).speed(10.0).clamp_range(20.0f32..=20000.0).suffix("Hz"));
                    ui.add(egui::DragValue::new(&mut band.gain).speed(0.1).clamp_range(-18.0f32..=18.0).suffix("dB"));
                    ui.add(egui::DragValue::new(&mut band.q).speed(0.01).clamp_range(0.1f32..=20.0).prefix("Q:"));
                    egui::ComboBox::from_id_salt(format!("eq_type_{}", i)).selected_text(band.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut band.band_type, EqBandType::LowShelf, "Low Shelf");
                            ui.selectable_value(&mut band.band_type, EqBandType::HighShelf, "High Shelf");
                            ui.selectable_value(&mut band.band_type, EqBandType::Peak, "Peak");
                            ui.selectable_value(&mut band.band_type, EqBandType::LowCut, "Low Cut");
                            ui.selectable_value(&mut band.band_type, EqBandType::HighCut, "High Cut");
                            ui.selectable_value(&mut band.band_type, EqBandType::Notch, "Notch");
                        });
                    if ui.small_button("X").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(idx) = to_remove { bands.remove(idx); }
            if ui.small_button("+ Band").clicked() {
                let freq = [80.0f32, 250.0, 1000.0, 5000.0, 12000.0][bands.len() % 5];
                bands.push(ParametricBand { freq, gain: 0.0, q: 1.0, band_type: EqBandType::Peak, enabled: true });
            }
        });
}

// ============================================================
// AUDIO SESSION METADATA
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AudioSessionMetadata {
    pub project_name: String,
    pub author: String,
    pub created: String,
    pub modified: String,
    pub sample_rate: u32,
    pub bit_depth: u32,
    pub tempo: f32,
    pub key: String,
    pub time_sig_num: u32,
    pub time_sig_den: u32,
    pub notes: String,
    pub version: String,
}

pub fn show_session_metadata(ui: &mut egui::Ui, meta: &mut AudioSessionMetadata) {
    egui::CollapsingHeader::new(RichText::new("Session Metadata").color(Color32::from_rgb(180, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("session_meta").num_columns(2).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
                ui.label("Project:"); ui.text_edit_singleline(&mut meta.project_name); ui.end_row();
                ui.label("Author:"); ui.text_edit_singleline(&mut meta.author); ui.end_row();
                ui.label("Sample rate:");
                egui::ComboBox::from_id_salt("meta_sr").selected_text(format!("{}", meta.sample_rate))
                    .show_ui(ui, |ui| {
                        for &sr in &[44100u32, 48000, 88200, 96000] { ui.selectable_value(&mut meta.sample_rate, sr, format!("{}", sr)); }
                    });
                ui.end_row();
                ui.label("Bit depth:");
                egui::ComboBox::from_id_salt("meta_bd").selected_text(format!("{}", meta.bit_depth))
                    .show_ui(ui, |ui| {
                        for &bd in &[16u32, 24, 32] { ui.selectable_value(&mut meta.bit_depth, bd, format!("{}", bd)); }
                    });
                ui.end_row();
                ui.label("Tempo:"); ui.add(egui::DragValue::new(&mut meta.tempo).speed(0.5).suffix(" BPM").clamp_range(20.0f32..=300.0)); ui.end_row();
                ui.label("Key:"); ui.text_edit_singleline(&mut meta.key); ui.end_row();
                ui.label("Time sig:");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut meta.time_sig_num).speed(1.0).clamp_range(1u32..=16));
                    ui.label("/");
                    egui::ComboBox::from_id_salt("meta_ts_den").selected_text(format!("{}", meta.time_sig_den))
                        .show_ui(ui, |ui| {
                            for &d in &[2u32, 4, 8, 16] { ui.selectable_value(&mut meta.time_sig_den, d, format!("{}", d)); }
                        });
                });
                ui.end_row();
                ui.label("Notes:"); ui.add(egui::TextEdit::multiline(&mut meta.notes).desired_rows(3)); ui.end_row();
            });
        });
}

// ============================================================
// MIXER KEYBOARD SHORTCUTS HELP
// ============================================================

pub fn show_mixer_keyboard_help(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Keyboard Shortcuts").color(Color32::from_rgb(200, 200, 200)))
        .default_open(false)
        .show(ui, |ui| {
            let shortcuts = [
                ("M", "Mute selected bus"),
                ("S", "Solo selected bus"),
                ("Ctrl+Z", "Undo"),
                ("Ctrl+Y", "Redo"),
                ("Ctrl+D", "Duplicate bus"),
                ("Ctrl+S", "Save snapshot"),
                ("Ctrl+E", "Export stems"),
                ("Space", "Play/Pause"),
                ("0..9", "Recall snapshot 0-9"),
                ("F", "Toggle effects panel"),
                ("R", "Toggle routing view"),
                ("+/-", "Zoom in/out timeline"),
                ("Ctrl+A", "Select all clips"),
                ("Delete", "Delete selected"),
            ];
            egui::Grid::new("mixer_shortcuts").num_columns(2).spacing(Vec2::new(12.0, 2.0)).show(ui, |ui| {
                for (key, action) in shortcuts {
                    ui.label(RichText::new(key).monospace().color(Color32::from_rgb(255, 220, 80)));
                    ui.label(RichText::new(action).small().color(Color32::LIGHT_GRAY));
                    ui.end_row();
                }
            });
        });
}

// ============================================================
// QUICK MIXER STATUS BAR
// ============================================================

pub fn show_mixer_status_bar(ui: &mut egui::Ui, editor: &AudioMixerEditor) {
    ui.horizontal(|ui| {
        let clipping = editor.count_clipping_buses();
        if clipping > 0 {
            ui.label(RichText::new(format!("! {} clipping", clipping)).color(Color32::RED).strong().small());
        } else {
            ui.label(RichText::new("OK").color(Color32::from_rgb(80, 200, 80)).strong().small());
        }
        ui.separator();
        ui.label(RichText::new(format!("{} buses | {} sends | {} fx | BPM {:.0}", editor.buses.len(), editor.total_sends(), editor.effect_count_total(), editor.bpm_clock.bpm)).small().color(Color32::GRAY));
        ui.separator();
        let muted_count = editor.buses.iter().filter(|b| b.muted).count();
        let soloed_count = editor.buses.iter().filter(|b| b.soloed).count();
        if muted_count > 0 { ui.label(RichText::new(format!("M:{}", muted_count)).small().color(Color32::from_rgb(200, 80, 80))); }
        if soloed_count > 0 { ui.label(RichText::new(format!("S:{}", soloed_count)).small().color(Color32::YELLOW)); }
    });
}

// ============================================================
// NOISE GATE EFFECT HELPER
// ============================================================

pub fn draw_noise_gate_diagram(ui: &mut egui::Ui, threshold: f32, attack: f32, hold: f32, release: f32) {
    let desired = Vec2::new(ui.available_width().min(280.0), 60.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 26));
    // Simulated envelope
    let n = rect.width() as usize;
    let mut prev = Pos2::new(rect.left(), rect.bottom());
    let total = attack + hold + release + 0.1;
    for i in 0..n {
        let t = i as f32 / n as f32 * total;
        let env = if t < attack { t / attack.max(0.001) }
            else if t < attack + hold { 1.0 }
            else if t < attack + hold + release { 1.0 - (t - attack - hold) / release.max(0.001) }
            else { 0.0 };
        let x = rect.left() + i as f32;
        let y = rect.bottom() - rect.height() * 0.8 * env;
        if i > 0 { painter.line_segment([prev, Pos2::new(x, y)], Stroke::new(1.5, Color32::from_rgb(100, 220, 150))); }
        prev = Pos2::new(x, y);
    }
    let thresh_y = rect.bottom() - rect.height() * 0.5 * (1.0 - threshold / -80.0);
    painter.line_segment([Pos2::new(rect.left(), thresh_y), Pos2::new(rect.right(), thresh_y)], Stroke::new(1.0, Color32::from_rgb(255, 100, 100)));
    painter.text(Pos2::new(rect.left() + 2.0, rect.top() + 2.0), egui::Align2::LEFT_TOP, format!("Noise Gate  A:{:.0}ms H:{:.0}ms R:{:.0}ms", attack*1000.0, hold*1000.0, release*1000.0), FontId::monospace(7.0), Color32::GRAY);
}
