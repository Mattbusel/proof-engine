
//! Audio system editor — waveform display, mixer, spatial audio config, DSP chain.

use glam::{Vec2, Vec3};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Primitive types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioFormat {
    Pcm8,
    Pcm16,
    Pcm24,
    Pcm32,
    Float32,
    Float64,
    Adpcm,
    Opus,
    Vorbis,
    Mp3,
    Aac,
}

impl AudioFormat {
    pub fn bits_per_sample(self) -> u32 {
        match self {
            AudioFormat::Pcm8 => 8,
            AudioFormat::Pcm16 => 16,
            AudioFormat::Pcm24 => 24,
            AudioFormat::Pcm32 | AudioFormat::Float32 => 32,
            AudioFormat::Float64 => 64,
            AudioFormat::Adpcm => 4,
            _ => 0, // compressed
        }
    }
    pub fn is_compressed(self) -> bool {
        matches!(self, AudioFormat::Opus | AudioFormat::Vorbis | AudioFormat::Mp3 | AudioFormat::Aac)
    }
    pub fn label(self) -> &'static str {
        match self {
            AudioFormat::Pcm8 => "PCM 8-bit",
            AudioFormat::Pcm16 => "PCM 16-bit",
            AudioFormat::Pcm24 => "PCM 24-bit",
            AudioFormat::Pcm32 => "PCM 32-bit",
            AudioFormat::Float32 => "Float 32",
            AudioFormat::Float64 => "Float 64",
            AudioFormat::Adpcm => "ADPCM",
            AudioFormat::Opus => "Opus",
            AudioFormat::Vorbis => "Vorbis",
            AudioFormat::Mp3 => "MP3",
            AudioFormat::Aac => "AAC",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Quad,
    Surround51,
    Surround71,
    Ambisonics1stOrder,
    Ambisonics2ndOrder,
    Ambisonics3rdOrder,
}

impl ChannelLayout {
    pub fn channel_count(self) -> u32 {
        match self {
            ChannelLayout::Mono => 1,
            ChannelLayout::Stereo => 2,
            ChannelLayout::Quad => 4,
            ChannelLayout::Surround51 => 6,
            ChannelLayout::Surround71 => 8,
            ChannelLayout::Ambisonics1stOrder => 4,
            ChannelLayout::Ambisonics2ndOrder => 9,
            ChannelLayout::Ambisonics3rdOrder => 16,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ChannelLayout::Mono => "Mono",
            ChannelLayout::Stereo => "Stereo",
            ChannelLayout::Quad => "Quad",
            ChannelLayout::Surround51 => "5.1",
            ChannelLayout::Surround71 => "7.1",
            ChannelLayout::Ambisonics1stOrder => "Ambisonics 1st Order (4ch)",
            ChannelLayout::Ambisonics2ndOrder => "Ambisonics 2nd Order (9ch)",
            ChannelLayout::Ambisonics3rdOrder => "Ambisonics 3rd Order (16ch)",
        }
    }
}

// ---------------------------------------------------------------------------
// Waveform data + display
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AudioAsset {
    pub id: u64,
    pub name: String,
    pub format: AudioFormat,
    pub layout: ChannelLayout,
    pub sample_rate: u32,
    pub frame_count: u64,
    pub loop_start: Option<u64>,
    pub loop_end: Option<u64>,
    pub peak_amplitude: f32,
    pub rms_amplitude: f32,
    pub tags: Vec<String>,
    /// Downsampled waveform for display (interleaved by channel)
    pub waveform_preview: Vec<f32>,
    pub preview_frames_per_pixel: u32,
}

impl AudioAsset {
    pub fn new(id: u64, name: impl Into<String>, format: AudioFormat, layout: ChannelLayout, sample_rate: u32, frame_count: u64) -> Self {
        Self {
            id,
            name: name.into(),
            format,
            layout,
            sample_rate,
            frame_count,
            loop_start: None,
            loop_end: None,
            peak_amplitude: 1.0,
            rms_amplitude: 0.707,
            tags: Vec::new(),
            waveform_preview: Vec::new(),
            preview_frames_per_pixel: 64,
        }
    }

    pub fn duration_secs(&self) -> f64 {
        self.frame_count as f64 / self.sample_rate as f64
    }

    pub fn byte_size_uncompressed(&self) -> u64 {
        self.frame_count * self.layout.channel_count() as u64 * (self.format.bits_per_sample() as u64 / 8).max(1)
    }

    pub fn generate_synthetic_waveform(&mut self, pixel_width: usize) {
        self.waveform_preview.clear();
        let channels = self.layout.channel_count() as usize;
        for ch in 0..channels {
            for px in 0..pixel_width {
                let t = px as f32 / pixel_width as f32;
                // Simulate a realistic waveform shape using multiple harmonics
                let v = (t * std::f32::consts::TAU * 3.0).sin() * 0.6
                    + (t * std::f32::consts::TAU * 7.0).sin() * 0.25
                    + (t * std::f32::consts::TAU * 11.0 + ch as f32).sin() * 0.15;
                // Envelope shape
                let envelope = (t * std::f32::consts::PI).sin().powf(0.3);
                self.waveform_preview.push(v * envelope);
            }
        }
        self.preview_frames_per_pixel = (self.frame_count as u32).saturating_div(pixel_width as u32).max(1);
    }

    pub fn set_loop_region(&mut self, start_frame: u64, end_frame: u64) {
        assert!(start_frame < end_frame && end_frame <= self.frame_count);
        self.loop_start = Some(start_frame);
        self.loop_end = Some(end_frame);
    }

    pub fn clear_loop(&mut self) {
        self.loop_start = None;
        self.loop_end = None;
    }

    pub fn frame_at_time(&self, t: f64) -> u64 {
        ((t * self.sample_rate as f64) as u64).min(self.frame_count.saturating_sub(1))
    }
}

// ---------------------------------------------------------------------------
// Waveform editor panel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WaveformView {
    pub asset_id: u64,
    pub view_start_frame: u64,
    pub view_end_frame: u64,
    pub channel_heights: Vec<f32>,
    pub zoom: f32,
    pub cursor_frame: u64,
    pub selection_start: Option<u64>,
    pub selection_end: Option<u64>,
    pub show_loop_markers: bool,
    pub show_spectogram: bool,
    pub amplitude_scale: f32,
}

impl WaveformView {
    pub fn new(asset: &AudioAsset) -> Self {
        Self {
            asset_id: asset.id,
            view_start_frame: 0,
            view_end_frame: asset.frame_count,
            channel_heights: vec![80.0; asset.layout.channel_count() as usize],
            zoom: 1.0,
            cursor_frame: 0,
            selection_start: None,
            selection_end: None,
            show_loop_markers: true,
            show_spectogram: false,
            amplitude_scale: 1.0,
        }
    }

    pub fn view_duration_frames(&self) -> u64 {
        self.view_end_frame.saturating_sub(self.view_start_frame)
    }

    pub fn frame_to_x(&self, frame: u64, width: f32) -> f32 {
        let t = (frame.saturating_sub(self.view_start_frame)) as f32 / self.view_duration_frames().max(1) as f32;
        t * width
    }

    pub fn x_to_frame(&self, x: f32, width: f32) -> u64 {
        let t = (x / width).clamp(0.0, 1.0);
        self.view_start_frame + (t * self.view_duration_frames() as f32) as u64
    }

    pub fn scroll(&mut self, delta_frames: i64, total_frames: u64) {
        let dur = self.view_duration_frames();
        let start = (self.view_start_frame as i64 + delta_frames).max(0) as u64;
        let end = (start + dur).min(total_frames);
        self.view_start_frame = end.saturating_sub(dur);
        self.view_end_frame = end;
    }

    pub fn zoom_around_cursor(&mut self, factor: f32, total_frames: u64) {
        let center = self.cursor_frame;
        let half = (self.view_duration_frames() as f32 / factor / 2.0) as u64;
        self.view_start_frame = center.saturating_sub(half);
        self.view_end_frame = (center + half).min(total_frames);
        self.zoom *= factor;
    }

    pub fn select_region(&mut self, start_x: f32, end_x: f32, width: f32) {
        let a = self.x_to_frame(start_x.min(end_x), width);
        let b = self.x_to_frame(start_x.max(end_x), width);
        self.selection_start = Some(a);
        self.selection_end = Some(b);
    }

    pub fn has_selection(&self) -> bool {
        self.selection_start.is_some() && self.selection_end.is_some()
    }

    pub fn selection_length(&self) -> u64 {
        match (self.selection_start, self.selection_end) {
            (Some(a), Some(b)) => b.saturating_sub(a),
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// DSP effect chain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum DspEffectKind {
    Gain,
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Peaking,
    LowShelf,
    HighShelf,
    Compressor,
    Limiter,
    Gate,
    Reverb,
    Delay,
    Chorus,
    Flanger,
    Phaser,
    Distortion,
    Bitcrusher,
    Eq4Band,
    Eq8Band,
    Eq31Band,
    StereoWidener,
    MidSide,
    Tremolo,
    Vibrato,
    Autopan,
    RingModulator,
    Pitch,
    Transient,
    DeEsser,
    NoiseGate,
}

impl DspEffectKind {
    pub fn label(&self) -> &'static str {
        match self {
            DspEffectKind::Gain => "Gain",
            DspEffectKind::Lowpass => "Low-pass Filter",
            DspEffectKind::Highpass => "High-pass Filter",
            DspEffectKind::Bandpass => "Band-pass Filter",
            DspEffectKind::Notch => "Notch Filter",
            DspEffectKind::Peaking => "Peaking EQ",
            DspEffectKind::LowShelf => "Low Shelf",
            DspEffectKind::HighShelf => "High Shelf",
            DspEffectKind::Compressor => "Compressor",
            DspEffectKind::Limiter => "Limiter",
            DspEffectKind::Gate => "Gate",
            DspEffectKind::Reverb => "Reverb",
            DspEffectKind::Delay => "Delay",
            DspEffectKind::Chorus => "Chorus",
            DspEffectKind::Flanger => "Flanger",
            DspEffectKind::Phaser => "Phaser",
            DspEffectKind::Distortion => "Distortion",
            DspEffectKind::Bitcrusher => "Bitcrusher",
            DspEffectKind::Eq4Band => "4-Band EQ",
            DspEffectKind::Eq8Band => "8-Band EQ",
            DspEffectKind::Eq31Band => "31-Band EQ",
            DspEffectKind::StereoWidener => "Stereo Widener",
            DspEffectKind::MidSide => "Mid/Side",
            DspEffectKind::Tremolo => "Tremolo",
            DspEffectKind::Vibrato => "Vibrato",
            DspEffectKind::Autopan => "Auto-pan",
            DspEffectKind::RingModulator => "Ring Modulator",
            DspEffectKind::Pitch => "Pitch Shift",
            DspEffectKind::Transient => "Transient Shaper",
            DspEffectKind::DeEsser => "De-Esser",
            DspEffectKind::NoiseGate => "Noise Gate",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            DspEffectKind::Gain => "Utility",
            DspEffectKind::Lowpass | DspEffectKind::Highpass | DspEffectKind::Bandpass
            | DspEffectKind::Notch | DspEffectKind::Peaking | DspEffectKind::LowShelf
            | DspEffectKind::HighShelf => "Filter",
            DspEffectKind::Eq4Band | DspEffectKind::Eq8Band | DspEffectKind::Eq31Band => "EQ",
            DspEffectKind::Compressor | DspEffectKind::Limiter | DspEffectKind::Gate
            | DspEffectKind::NoiseGate | DspEffectKind::Transient | DspEffectKind::DeEsser => "Dynamics",
            DspEffectKind::Reverb | DspEffectKind::Delay => "Time-Based",
            DspEffectKind::Chorus | DspEffectKind::Flanger | DspEffectKind::Phaser => "Modulation",
            DspEffectKind::Distortion | DspEffectKind::Bitcrusher => "Distortion",
            DspEffectKind::StereoWidener | DspEffectKind::MidSide => "Stereo",
            DspEffectKind::Tremolo | DspEffectKind::Vibrato | DspEffectKind::Autopan
            | DspEffectKind::RingModulator => "Modulation",
            DspEffectKind::Pitch => "Pitch",
        }
    }

    pub fn default_params(&self) -> Vec<(&'static str, f32, f32, f32)> {
        // (name, default, min, max)
        match self {
            DspEffectKind::Gain => vec![("gain_db", 0.0, -60.0, 24.0)],
            DspEffectKind::Lowpass | DspEffectKind::Highpass => vec![
                ("freq_hz", 1000.0, 20.0, 20000.0),
                ("resonance", 0.707, 0.1, 20.0),
            ],
            DspEffectKind::Bandpass | DspEffectKind::Notch => vec![
                ("freq_hz", 1000.0, 20.0, 20000.0),
                ("bandwidth", 1.0, 0.1, 10.0),
            ],
            DspEffectKind::Peaking | DspEffectKind::LowShelf | DspEffectKind::HighShelf => vec![
                ("freq_hz", 1000.0, 20.0, 20000.0),
                ("gain_db", 0.0, -24.0, 24.0),
                ("q", 1.0, 0.1, 10.0),
            ],
            DspEffectKind::Compressor => vec![
                ("threshold_db", -12.0, -60.0, 0.0),
                ("ratio", 4.0, 1.0, 100.0),
                ("attack_ms", 10.0, 0.1, 200.0),
                ("release_ms", 100.0, 1.0, 3000.0),
                ("makeup_db", 0.0, 0.0, 30.0),
            ],
            DspEffectKind::Reverb => vec![
                ("room_size", 0.5, 0.0, 1.0),
                ("damping", 0.5, 0.0, 1.0),
                ("wet", 0.33, 0.0, 1.0),
                ("width", 1.0, 0.0, 1.0),
            ],
            DspEffectKind::Delay => vec![
                ("delay_ms", 250.0, 1.0, 5000.0),
                ("feedback", 0.4, 0.0, 0.99),
                ("wet", 0.5, 0.0, 1.0),
                ("ping_pong", 0.0, 0.0, 1.0),
            ],
            DspEffectKind::Pitch => vec![
                ("semitones", 0.0, -24.0, 24.0),
                ("formant_shift", 0.0, -2.0, 2.0),
            ],
            _ => vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct DspEffect {
    pub id: u32,
    pub kind: DspEffectKind,
    pub enabled: bool,
    pub params: HashMap<String, f32>,
    pub wet_dry: f32,
}

impl DspEffect {
    pub fn new(id: u32, kind: DspEffectKind) -> Self {
        let params = kind.default_params()
            .into_iter()
            .map(|(name, default, _, _)| (name.to_string(), default))
            .collect();
        Self { id, kind, enabled: true, params, wet_dry: 1.0 }
    }

    pub fn set_param(&mut self, name: &str, value: f32) {
        if let Some(v) = self.params.get_mut(name) {
            *v = value;
        }
    }

    pub fn get_param(&self, name: &str) -> f32 {
        self.params.get(name).copied().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct DspChain {
    pub effects: Vec<DspEffect>,
    pub next_id: u32,
}

impl DspChain {
    pub fn new() -> Self {
        Self { effects: Vec::new(), next_id: 1 }
    }

    pub fn add_effect(&mut self, kind: DspEffectKind) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.effects.push(DspEffect::new(id, kind));
        id
    }

    pub fn remove_effect(&mut self, id: u32) {
        self.effects.retain(|e| e.id != id);
    }

    pub fn move_effect_up(&mut self, id: u32) {
        if let Some(i) = self.effects.iter().position(|e| e.id == id) {
            if i > 0 {
                self.effects.swap(i, i - 1);
            }
        }
    }

    pub fn move_effect_down(&mut self, id: u32) {
        if let Some(i) = self.effects.iter().position(|e| e.id == id) {
            if i + 1 < self.effects.len() {
                self.effects.swap(i, i + 1);
            }
        }
    }

    pub fn active_effects(&self) -> impl Iterator<Item = &DspEffect> {
        self.effects.iter().filter(|e| e.enabled)
    }
}

// ---------------------------------------------------------------------------
// Audio bus / mixer channel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SendType {
    PreFader,
    PostFader,
    PostFaderPostPan,
}

#[derive(Debug, Clone)]
pub struct BusSend {
    pub target_bus: String,
    pub gain: f32,
    pub send_type: SendType,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct MixerChannel {
    pub name: String,
    pub gain_db: f32,
    pub pan: f32,       // -1 .. 1
    pub muted: bool,
    pub soloed: bool,
    pub fader_automation: bool,
    pub sends: Vec<BusSend>,
    pub dsp_chain: DspChain,
    pub color: [u8; 3],
    pub vumeter_l: f32,
    pub vumeter_r: f32,
    pub peak_l: f32,
    pub peak_r: f32,
    pub peak_hold_timer: f32,
}

impl MixerChannel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            gain_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            fader_automation: false,
            sends: Vec::new(),
            dsp_chain: DspChain::new(),
            color: [80, 120, 200],
            vumeter_l: 0.0,
            vumeter_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            peak_hold_timer: 0.0,
        }
    }

    pub fn linear_gain(&self) -> f32 {
        if self.muted { return 0.0; }
        10f32.powf(self.gain_db / 20.0)
    }

    pub fn pan_left(&self) -> f32 {
        ((1.0 - self.pan) * 0.5).sqrt()
    }

    pub fn pan_right(&self) -> f32 {
        ((1.0 + self.pan) * 0.5).sqrt()
    }

    pub fn update_vu(&mut self, l: f32, r: f32, dt: f32) {
        let attack = 0.003_f32;
        let release = 0.3_f32;
        let a_attack = (-2.2 / (attack * 44100.0)).exp();
        let a_release = (-2.2 / (release * 44100.0)).exp();
        self.vumeter_l = if l > self.vumeter_l { a_attack * self.vumeter_l + (1.0 - a_attack) * l }
                         else { a_release * self.vumeter_l + (1.0 - a_release) * l };
        self.vumeter_r = if r > self.vumeter_r { a_attack * self.vumeter_r + (1.0 - a_attack) * r }
                         else { a_release * self.vumeter_r + (1.0 - a_release) * r };
        if l > self.peak_l { self.peak_l = l; self.peak_hold_timer = 2.0; }
        if r > self.peak_r { self.peak_r = r; self.peak_hold_timer = 2.0; }
        self.peak_hold_timer -= dt;
        if self.peak_hold_timer <= 0.0 {
            self.peak_l = (self.peak_l - dt * 0.3).max(0.0);
            self.peak_r = (self.peak_r - dt * 0.3).max(0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Spatial audio source
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttenuationCurve {
    Linear,
    InverseSquare,
    Logarithmic,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanningModel {
    Stereo,
    Hrtf,
    Ambisonics,
    VbapSurround,
}

#[derive(Debug, Clone)]
pub struct SpatialAudioSource {
    pub name: String,
    pub position: Vec3,
    pub velocity: Vec3,
    pub min_distance: f32,
    pub max_distance: f32,
    pub attenuation: AttenuationCurve,
    pub rolloff_factor: f32,
    pub cone_inner_angle: f32,   // degrees
    pub cone_outer_angle: f32,   // degrees
    pub cone_outer_gain: f32,
    pub doppler_factor: f32,
    pub panning_model: PanningModel,
    pub occlude_factor: f32,
    pub reverb_send: f32,
    pub asset_id: Option<u64>,
    pub bus: String,
    pub loop_audio: bool,
    pub auto_play: bool,
    pub priority: i32,
}

impl SpatialAudioSource {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            min_distance: 1.0,
            max_distance: 100.0,
            attenuation: AttenuationCurve::InverseSquare,
            rolloff_factor: 1.0,
            cone_inner_angle: 360.0,
            cone_outer_angle: 360.0,
            cone_outer_gain: 0.0,
            doppler_factor: 1.0,
            panning_model: PanningModel::Hrtf,
            occlude_factor: 0.0,
            reverb_send: 0.0,
            asset_id: None,
            bus: "SFX".to_string(),
            loop_audio: false,
            auto_play: false,
            priority: 128,
        }
    }

    pub fn gain_at_distance(&self, dist: f32) -> f32 {
        let d = dist.max(self.min_distance);
        if d >= self.max_distance { return 0.0; }
        match self.attenuation {
            AttenuationCurve::Linear => {
                1.0 - self.rolloff_factor * (d - self.min_distance) / (self.max_distance - self.min_distance)
            }
            AttenuationCurve::InverseSquare => {
                self.min_distance / (self.min_distance + self.rolloff_factor * (d - self.min_distance))
            }
            AttenuationCurve::Logarithmic => {
                (1.0 - self.rolloff_factor * (d / self.min_distance).ln() / (self.max_distance / self.min_distance).ln()).max(0.0)
            }
            AttenuationCurve::Custom => 1.0,
        }
    }

    pub fn directional_gain(&self, listener_dir: Vec3) -> f32 {
        if self.cone_inner_angle >= 360.0 { return 1.0; }
        let angle_deg = listener_dir.dot(Vec3::NEG_Z).acos().to_degrees();
        if angle_deg <= self.cone_inner_angle * 0.5 {
            1.0
        } else if angle_deg >= self.cone_outer_angle * 0.5 {
            self.cone_outer_gain
        } else {
            let t = (angle_deg - self.cone_inner_angle * 0.5) / ((self.cone_outer_angle - self.cone_inner_angle) * 0.5);
            1.0 + (self.cone_outer_gain - 1.0) * t
        }
    }
}

// ---------------------------------------------------------------------------
// Audio Mixer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AudioMixer {
    pub buses: Vec<MixerChannel>,
    pub master: MixerChannel,
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub latency_ms: f32,
}

impl AudioMixer {
    pub fn new() -> Self {
        let mut mixer = Self {
            buses: Vec::new(),
            master: MixerChannel::new("Master"),
            sample_rate: 48000,
            buffer_size: 512,
            latency_ms: 0.0,
        };
        mixer.latency_ms = mixer.buffer_size as f32 / mixer.sample_rate as f32 * 1000.0;
        // Default buses
        mixer.add_bus("Music");
        mixer.add_bus("SFX");
        mixer.add_bus("Voice");
        mixer.add_bus("Ambient");
        mixer.add_bus("UI");
        mixer.add_bus("Reverb");
        mixer
    }

    pub fn add_bus(&mut self, name: impl Into<String>) {
        self.buses.push(MixerChannel::new(name));
    }

    pub fn remove_bus(&mut self, name: &str) {
        self.buses.retain(|b| b.name != name);
    }

    pub fn bus_mut(&mut self, name: &str) -> Option<&mut MixerChannel> {
        self.buses.iter_mut().find(|b| b.name == name)
    }

    pub fn bus(&self, name: &str) -> Option<&MixerChannel> {
        self.buses.iter().find(|b| b.name == name)
    }

    pub fn set_solo(&mut self, name: &str, solo: bool) {
        for bus in self.buses.iter_mut() {
            bus.soloed = bus.name == name && solo;
        }
    }

    pub fn any_soloed(&self) -> bool {
        self.buses.iter().any(|b| b.soloed)
    }

    pub fn update_meters(&mut self, dt: f32) {
        // Simulate decaying meters
        for bus in self.buses.iter_mut() {
            bus.vumeter_l = (bus.vumeter_l - dt * 8.0).max(0.0);
            bus.vumeter_r = (bus.vumeter_r - dt * 8.0).max(0.0);
            bus.update_vu(bus.vumeter_l, bus.vumeter_r, dt);
        }
    }
}

// ---------------------------------------------------------------------------
// Audio editor state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioEditorPanel {
    Mixer,
    WaveformEditor,
    SpatialSourceList,
    DspChainEditor,
    AssetLibrary,
}

#[derive(Debug, Clone)]
pub struct AudioEditor {
    pub mixer: AudioMixer,
    pub assets: Vec<AudioAsset>,
    pub spatial_sources: Vec<SpatialAudioSource>,
    pub waveform_views: HashMap<u64, WaveformView>,
    pub selected_asset: Option<u64>,
    pub selected_bus: Option<String>,
    pub selected_source: Option<usize>,
    pub active_panel: AudioEditorPanel,
    pub search_query: String,
    pub show_db_scale: bool,
    pub master_volume: f32,
    pub next_asset_id: u64,
}

impl AudioEditor {
    pub fn new() -> Self {
        let mut editor = Self {
            mixer: AudioMixer::new(),
            assets: Vec::new(),
            spatial_sources: Vec::new(),
            waveform_views: HashMap::new(),
            selected_asset: None,
            selected_bus: None,
            selected_source: None,
            active_panel: AudioEditorPanel::Mixer,
            search_query: String::new(),
            show_db_scale: true,
            master_volume: 1.0,
            next_asset_id: 1,
        };
        editor.populate_demo_assets();
        editor
    }

    fn populate_demo_assets(&mut self) {
        let formats = [
            (AudioFormat::Pcm16, ChannelLayout::Stereo, "footstep_concrete.wav", 44100u32, 4410u64),
            (AudioFormat::Opus, ChannelLayout::Stereo, "music_theme.opus", 48000, 2880000),
            (AudioFormat::Float32, ChannelLayout::Surround51, "ambience_forest.wav", 48000, 288000),
            (AudioFormat::Vorbis, ChannelLayout::Mono, "voice_line_01.ogg", 22050, 44100),
            (AudioFormat::Pcm16, ChannelLayout::Mono, "explosion_01.wav", 44100, 22050),
            (AudioFormat::Pcm24, ChannelLayout::Stereo, "ui_click.wav", 44100, 1323),
            (AudioFormat::Pcm16, ChannelLayout::Stereo, "weapon_reload.wav", 44100, 8820),
            (AudioFormat::Opus, ChannelLayout::Ambisonics1stOrder, "room_tone_ambisonic.opus", 48000, 96000),
        ];
        for (fmt, layout, name, sr, frames) in &formats {
            let id = self.next_asset_id;
            self.next_asset_id += 1;
            let mut asset = AudioAsset::new(id, *name, *fmt, *layout, *sr, *frames);
            asset.generate_synthetic_waveform(512);
            asset.rms_amplitude = 0.3 + (id as f32 * 0.07) % 0.5;
            asset.peak_amplitude = asset.rms_amplitude * 1.8;
            self.assets.push(asset);
        }
        // Spatial sources
        for i in 0..5usize {
            let mut src = SpatialAudioSource::new(format!("SpatialSource_{}", i));
            src.position = Vec3::new(i as f32 * 5.0 - 10.0, 0.0, i as f32 * 2.0);
            src.asset_id = Some(i as u64 % self.assets.len() as u64 + 1);
            self.spatial_sources.push(src);
        }
    }

    pub fn open_waveform_editor(&mut self, asset_id: u64) {
        if let Some(asset) = self.assets.iter().find(|a| a.id == asset_id) {
            let view = WaveformView::new(asset);
            self.waveform_views.insert(asset_id, view);
            self.selected_asset = Some(asset_id);
            self.active_panel = AudioEditorPanel::WaveformEditor;
        }
    }

    pub fn search_assets(&self, query: &str) -> Vec<&AudioAsset> {
        let q = query.to_lowercase();
        self.assets.iter().filter(|a| {
            a.name.to_lowercase().contains(&q) ||
            a.tags.iter().any(|t| t.to_lowercase().contains(&q)) ||
            a.format.label().to_lowercase().contains(&q)
        }).collect()
    }

    pub fn total_asset_size_bytes(&self) -> u64 {
        self.assets.iter().map(|a| a.byte_size_uncompressed()).sum()
    }

    pub fn add_dsp_to_selected_bus(&mut self, kind: DspEffectKind) {
        if let Some(bus_name) = &self.selected_bus.clone() {
            if let Some(bus) = self.mixer.bus_mut(bus_name) {
                bus.dsp_chain.add_effect(kind);
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.mixer.update_meters(dt);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_asset_creation() {
        let asset = AudioAsset::new(1, "test.wav", AudioFormat::Pcm16, ChannelLayout::Stereo, 44100, 44100);
        assert!((asset.duration_secs() - 1.0).abs() < 1e-6);
        assert_eq!(asset.byte_size_uncompressed(), 44100 * 2 * 2);
    }

    #[test]
    fn test_dsp_chain() {
        let mut chain = DspChain::new();
        let id = chain.add_effect(DspEffectKind::Compressor);
        assert!(chain.effects.iter().any(|e| e.id == id));
        chain.remove_effect(id);
        assert!(chain.effects.is_empty());
    }

    #[test]
    fn test_spatial_gain() {
        let src = SpatialAudioSource::new("test");
        let g = src.gain_at_distance(1.0);
        assert!((g - 1.0).abs() < 0.01);
        let g2 = src.gain_at_distance(100.0);
        assert!(g2 < 0.01);
    }

    #[test]
    fn test_audio_editor() {
        let mut editor = AudioEditor::new();
        assert!(!editor.assets.is_empty());
        assert!(!editor.spatial_sources.is_empty());
        editor.update(0.016);
    }
}
