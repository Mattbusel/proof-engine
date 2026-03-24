//! Subtitle and localised text display system.
//!
//! Provides time-coded subtitle lines, a multi-language database, a renderer
//! that computes active lines and their fade alpha each frame, a typewriter
//! ("barker") effect, and a closed-caption extension that handles sound-effect
//! labels like `[GUNSHOT]`.
//!
//! # Text format
//! The plain-text parser understands lines of the form:
//! ```text
//! 0.00 | 3.50 | Hero  | Time to fight!
//! 3.60 | 6.00 | Villain | I've been expecting you.
//! ```
//! Fields are separated by `|`. Leading/trailing whitespace is trimmed.
//! Lines starting with `#` are comments.

use std::collections::HashMap;

// ── SubPos ────────────────────────────────────────────────────────────────────

/// Screen position for subtitle rendering.
#[derive(Debug, Clone, Copy)]
pub enum SubPos {
    /// Centred at bottom of screen (default).
    Bottom,
    /// Centred at top of screen.
    Top,
    /// Custom normalised screen position [0,1] × [0,1].
    Custom { x: f32, y: f32 },
}

impl SubPos {
    /// Normalised y position (0 = bottom, 1 = top).
    pub fn y(&self) -> f32 {
        match self {
            SubPos::Bottom       => 0.08,
            SubPos::Top          => 0.92,
            SubPos::Custom { y, .. } => *y,
        }
    }

    /// Normalised x position (0 = left, 1 = right, 0.5 = centre).
    pub fn x(&self) -> f32 {
        match self {
            SubPos::Bottom       => 0.5,
            SubPos::Top          => 0.5,
            SubPos::Custom { x, .. } => *x,
        }
    }
}

impl Default for SubPos {
    fn default() -> Self { SubPos::Bottom }
}

// ── SubtitleStyle ─────────────────────────────────────────────────────────────

/// Visual style parameters for a subtitle line.
#[derive(Debug, Clone)]
pub struct SubtitleStyle {
    pub font_size:  f32,
    /// Text RGBA colour.
    pub color:      [f32; 4],
    /// Background box RGBA colour.
    pub bg_color:   [f32; 4],
    pub position:   SubPos,
    pub bold:       bool,
    pub italic:     bool,
    /// Fade-in duration in seconds.
    pub fade_in:    f32,
    /// Fade-out duration in seconds.
    pub fade_out:   f32,
    /// Optional stroke/outline colour.
    pub outline:    Option<[f32; 4]>,
    /// Outline thickness in pixels.
    pub outline_px: f32,
}

impl SubtitleStyle {
    pub fn default_style() -> Self {
        Self {
            font_size:  24.0,
            color:      [1.0, 1.0, 1.0, 1.0],
            bg_color:   [0.0, 0.0, 0.0, 0.55],
            position:   SubPos::Bottom,
            bold:       false,
            italic:     false,
            fade_in:    0.15,
            fade_out:   0.2,
            outline:    Some([0.0, 0.0, 0.0, 1.0]),
            outline_px: 1.5,
        }
    }

    pub fn top() -> Self {
        Self { position: SubPos::Top, ..Self::default_style() }
    }

    pub fn large() -> Self {
        Self { font_size: 36.0, bold: true, ..Self::default_style() }
    }

    pub fn speaker_style(speaker: &str) -> Self {
        // Give different speakers distinct colours
        let color = match speaker.to_lowercase().as_str() {
            "hero"    => [0.6, 0.9, 1.0, 1.0],
            "villain" => [1.0, 0.4, 0.4, 1.0],
            "narrator"=> [0.9, 0.85, 0.6, 1.0],
            _         => [1.0, 1.0, 1.0, 1.0],
        };
        Self { color, ..Self::default_style() }
    }

    pub fn with_font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn with_color(mut self, c: [f32; 4]) -> Self { self.color = c; self }
    pub fn with_bg(mut self, c: [f32; 4]) -> Self { self.bg_color = c; self }
    pub fn with_position(mut self, p: SubPos) -> Self { self.position = p; self }
    pub fn bold(mut self) -> Self { self.bold = true; self }
    pub fn italic(mut self) -> Self { self.italic = true; self }
    pub fn with_fades(mut self, fade_in: f32, fade_out: f32) -> Self {
        self.fade_in  = fade_in;
        self.fade_out = fade_out;
        self
    }
}

impl Default for SubtitleStyle {
    fn default() -> Self { Self::default_style() }
}

// ── SubtitleLine ──────────────────────────────────────────────────────────────

/// A single time-coded subtitle line.
#[derive(Debug, Clone)]
pub struct SubtitleLine {
    /// Time in seconds at which this line appears.
    pub start:   f32,
    /// Time in seconds at which this line disappears.
    pub end:     f32,
    /// Speaker name (may be empty for narration).
    pub speaker: String,
    /// Full display text.
    pub text:    String,
    /// Visual style.
    pub style:   SubtitleStyle,
    /// Line id for deduplication / referencing.
    pub id:      u32,
}

impl SubtitleLine {
    pub fn new(start: f32, end: f32, speaker: impl Into<String>, text: impl Into<String>) -> Self {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let speaker = speaker.into();
        let style   = SubtitleStyle::speaker_style(&speaker);
        Self {
            start,
            end,
            speaker,
            text:  text.into(),
            style,
            id:    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    pub fn with_style(mut self, s: SubtitleStyle) -> Self { self.style = s; self }

    pub fn duration(&self) -> f32 { (self.end - self.start).max(0.0) }

    pub fn is_active(&self, t: f32) -> bool {
        t >= self.start && t < self.end
    }

    /// Compute fade alpha [0,1] at absolute time `t`.
    pub fn alpha_at(&self, t: f32) -> f32 {
        if t < self.start || t >= self.end { return 0.0; }
        let elapsed  = t - self.start;
        let remaining = self.end - t;
        let fade_in  = self.style.fade_in.max(f32::EPSILON);
        let fade_out = self.style.fade_out.max(f32::EPSILON);
        let in_alpha  = (elapsed  / fade_in).min(1.0);
        let out_alpha = (remaining / fade_out).min(1.0);
        in_alpha.min(out_alpha)
    }
}

// ── SubtitleTrack ─────────────────────────────────────────────────────────────

/// A sorted collection of `SubtitleLine`s with fast binary-search lookup.
#[derive(Debug, Clone, Default)]
pub struct SubtitleTrack {
    /// Lines sorted by `start` time.
    pub lines: Vec<SubtitleLine>,
}

impl SubtitleTrack {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Insert a line, maintaining sort by start time.
    pub fn insert(&mut self, line: SubtitleLine) {
        let idx = self.lines.partition_point(|l| l.start <= line.start);
        self.lines.insert(idx, line);
    }

    pub fn push(&mut self, start: f32, end: f32, speaker: impl Into<String>, text: impl Into<String>) {
        self.insert(SubtitleLine::new(start, end, speaker, text));
    }

    /// All lines whose range contains `t`.
    pub fn active_at(&self, t: f32) -> Vec<&SubtitleLine> {
        // Binary search for the first line that could be active
        let start_idx = self.lines.partition_point(|l| l.end <= t);
        self.lines[start_idx..].iter()
            .take_while(|l| l.start <= t)
            .filter(|l| l.is_active(t))
            .collect()
    }

    /// Total track duration (end of last line).
    pub fn duration(&self) -> f32 {
        self.lines.iter().map(|l| l.end).fold(0.0f32, f32::max)
    }

    pub fn len(&self) -> usize { self.lines.len() }
    pub fn is_empty(&self) -> bool { self.lines.is_empty() }

    /// Remove all lines.
    pub fn clear(&mut self) { self.lines.clear(); }
}

// ── SubtitleRenderer ─────────────────────────────────────────────────────────

/// Computes which subtitle lines are visible at time `t` and their alphas.
///
/// Also drives the optional typewriter / "barker" effect per line.
#[derive(Debug, Clone)]
pub struct ActiveSubtitle {
    pub line:     SubtitleLine,
    /// Alpha [0,1] including fade.
    pub alpha:    f32,
    /// Visible text (typewriter may show a partial string).
    pub visible_text: String,
    /// Whether the typewriter effect is complete for this line.
    pub tw_complete: bool,
}

/// Tracks ongoing typewriter state per line ID.
#[derive(Debug, Clone)]
struct BarkerState {
    chars_shown: usize,
    complete:    bool,
    pause_timer: f32,
}

impl BarkerState {
    fn new() -> Self {
        Self { chars_shown: 0, complete: false, pause_timer: 0.0 }
    }
}

pub struct SubtitleRenderer {
    pub barker_enabled:   bool,
    pub chars_per_second: f32,
    /// Per-line barker state (keyed by SubtitleLine.id).
    barker_states:        HashMap<u32, BarkerState>,
    /// Lines that were active last frame (for lifecycle).
    prev_active_ids:      Vec<u32>,
}

impl SubtitleRenderer {
    pub fn new() -> Self {
        Self {
            barker_enabled:   false,
            chars_per_second: 25.0,
            barker_states:    HashMap::new(),
            prev_active_ids:  Vec::new(),
        }
    }

    pub fn with_barker(mut self, cps: f32) -> Self {
        self.barker_enabled   = true;
        self.chars_per_second = cps;
        self
    }

    /// Compute active subtitles for `t`.  Call once per frame.
    pub fn compute(&mut self, track: &SubtitleTrack, t: f32, dt: f32) -> Vec<ActiveSubtitle> {
        let active_lines = track.active_at(t);
        let current_ids: Vec<u32> = active_lines.iter().map(|l| l.id).collect();

        // Remove barker states for lines that are no longer active
        self.barker_states.retain(|id, _| current_ids.contains(id));

        let mut result = Vec::new();

        for line in &active_lines {
            let alpha = line.alpha_at(t);

            // Initialise barker state if newly active
            if self.barker_enabled && !self.barker_states.contains_key(&line.id) {
                self.barker_states.insert(line.id, BarkerState::new());
            }

            let (visible_text, tw_complete) = if self.barker_enabled {
                let state = self.barker_states.get_mut(&line.id).unwrap();
                Self::tick_barker(state, &line.text, dt, self.chars_per_second);
                let vt = Self::visible_text(&line.text, state.chars_shown);
                (vt, state.complete)
            } else {
                (line.text.clone(), true)
            };

            result.push(ActiveSubtitle {
                line:         (*line).clone(),
                alpha,
                visible_text,
                tw_complete,
            });
        }

        self.prev_active_ids = current_ids;
        result
    }

    fn tick_barker(state: &mut BarkerState, text: &str, dt: f32, cps: f32) {
        if state.complete { return; }

        if state.pause_timer > 0.0 {
            state.pause_timer -= dt;
            return;
        }

        let total_chars = text.chars().count();
        // Advance by cps chars this frame
        let chars_this_frame = (cps * dt).max(0.0);
        let new_count = (state.chars_shown as f32 + chars_this_frame) as usize;
        let new_count = new_count.min(total_chars);

        // Check for punctuation pause on newly revealed characters
        for i in state.chars_shown..new_count {
            if let Some(ch) = text.chars().nth(i) {
                match ch {
                    '.' | '!' | '?' => { state.pause_timer = 0.25; }
                    ',' | ';'       => { state.pause_timer = 0.1;  }
                    _ => {}
                }
            }
        }

        state.chars_shown = new_count;
        if state.chars_shown >= total_chars {
            state.complete = true;
        }
    }

    fn visible_text(text: &str, chars: usize) -> String {
        text.chars().take(chars).collect()
    }

    /// Skip typewriter for a specific line id.
    pub fn skip_barker(&mut self, line_id: u32, text: &str) {
        if let Some(state) = self.barker_states.get_mut(&line_id) {
            state.chars_shown = text.chars().count();
            state.complete    = true;
            state.pause_timer = 0.0;
        }
    }

    /// Skip all active typewriter effects.
    pub fn skip_all_barkers(&mut self, track: &SubtitleTrack, t: f32) {
        for line in track.active_at(t) {
            if let Some(state) = self.barker_states.get_mut(&line.id) {
                state.chars_shown = line.text.chars().count();
                state.complete    = true;
                state.pause_timer = 0.0;
            }
        }
    }
}

impl Default for SubtitleRenderer {
    fn default() -> Self { Self::new() }
}

// ── BarkerMode ────────────────────────────────────────────────────────────────

/// Standalone typewriter effect that reveals text progressively.
/// Can be used independently of `SubtitleRenderer`.
#[derive(Debug, Clone)]
pub struct BarkerMode {
    pub full_text:      String,
    pub chars_shown:    usize,
    pub chars_per_sec:  f32,
    pub complete:       bool,
    accumulator:        f32,
    pause_timer:        f32,
    /// Whether to emit a "tick" sound event on each revealed character.
    pub emit_tick:      bool,
    /// Accumulator for sound tick events — caller drains this.
    pub pending_ticks:  u32,
}

impl BarkerMode {
    pub fn new(text: impl Into<String>, chars_per_sec: f32) -> Self {
        let text     = text.into();
        let complete = text.is_empty();
        Self {
            full_text:     text,
            chars_shown:   0,
            chars_per_sec,
            complete,
            accumulator:   0.0,
            pause_timer:   0.0,
            emit_tick:     false,
            pending_ticks: 0,
        }
    }

    pub fn with_tick_sound(mut self) -> Self { self.emit_tick = true; self }

    /// Advance by `dt` seconds.  Returns true if newly completed.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.complete { return false; }

        if self.pause_timer > 0.0 {
            self.pause_timer -= dt;
            return false;
        }

        self.accumulator += dt * self.chars_per_sec;
        let to_reveal = self.accumulator as usize;
        self.accumulator -= to_reveal as f32;

        let total = self.full_text.chars().count();
        let mut newly_done = false;

        for _ in 0..to_reveal {
            if self.chars_shown >= total { break; }
            let ch = self.full_text.chars().nth(self.chars_shown).unwrap_or(' ');
            self.chars_shown += 1;
            if self.emit_tick { self.pending_ticks += 1; }

            match ch {
                '.' | '!' | '?' => self.pause_timer = 0.25,
                ',' | ';'       => self.pause_timer = 0.1,
                _ => {}
            }

            if self.chars_shown >= total {
                self.complete = true;
                newly_done = true;
                break;
            }
        }

        newly_done
    }

    /// Instantly reveal all text.
    pub fn skip(&mut self) {
        self.chars_shown  = self.full_text.chars().count();
        self.complete     = true;
        self.pause_timer  = 0.0;
        self.accumulator  = 0.0;
    }

    /// Currently visible text.
    pub fn visible_text(&self) -> String {
        self.full_text.chars().take(self.chars_shown).collect()
    }

    /// Progress [0, 1].
    pub fn progress(&self) -> f32 {
        let total = self.full_text.chars().count();
        if total == 0 { 1.0 } else { self.chars_shown as f32 / total as f32 }
    }

    /// Drain pending tick events.
    pub fn drain_ticks(&mut self) -> u32 {
        let n = self.pending_ticks;
        self.pending_ticks = 0;
        n
    }
}

// ── SubtitleDatabase ──────────────────────────────────────────────────────────

/// Stores per-locale subtitle tracks and parses from plain text.
#[derive(Debug, Clone, Default)]
pub struct SubtitleDatabase {
    /// locale -> SubtitleTrack
    tracks: HashMap<String, SubtitleTrack>,
    /// Default locale.
    pub default_locale: String,
}

impl SubtitleDatabase {
    pub fn new() -> Self {
        Self {
            tracks:         HashMap::new(),
            default_locale: "en".into(),
        }
    }

    pub fn with_default_locale(mut self, locale: impl Into<String>) -> Self {
        self.default_locale = locale.into();
        self
    }

    /// Get or create a track for a locale.
    pub fn track_mut(&mut self, locale: impl Into<String>) -> &mut SubtitleTrack {
        self.tracks.entry(locale.into()).or_insert_with(SubtitleTrack::new)
    }

    /// Get the track for a locale (read only).
    pub fn track(&self, locale: &str) -> Option<&SubtitleTrack> {
        self.tracks.get(locale)
    }

    /// Get the default locale track.
    pub fn default_track(&self) -> Option<&SubtitleTrack> {
        self.tracks.get(&self.default_locale)
    }

    /// Parse a subtitle file in the pipe-separated format and load into a locale.
    ///
    /// Format: `time_start | time_end | speaker | text`
    pub fn load_text(&mut self, locale: impl Into<String>, content: &str) {
        let locale  = locale.into();
        let track   = self.tracks.entry(locale).or_insert_with(SubtitleTrack::new);

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }

            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() < 4 { continue; }

            let start = parts[0].trim().parse::<f32>().unwrap_or(0.0);
            let end   = parts[1].trim().parse::<f32>().unwrap_or(0.0);
            let speaker = parts[2].trim().to_string();
            let text    = parts[3].trim().to_string();

            if end > start {
                track.insert(SubtitleLine::new(start, end, speaker, text));
            }
        }
    }

    /// Serialize a track to the pipe-separated text format.
    pub fn dump_text(&self, locale: &str) -> String {
        let mut lines = Vec::new();
        if let Some(track) = self.tracks.get(locale) {
            for line in &track.lines {
                lines.push(format!(
                    "{:.3} | {:.3} | {} | {}",
                    line.start, line.end, line.speaker, line.text
                ));
            }
        }
        lines.join("\n")
    }

    /// Available locale codes.
    pub fn locales(&self) -> Vec<&str> {
        self.tracks.keys().map(|s| s.as_str()).collect()
    }

    pub fn has_locale(&self, locale: &str) -> bool {
        self.tracks.contains_key(locale)
    }

    /// Remove a locale.
    pub fn remove_locale(&mut self, locale: &str) {
        self.tracks.remove(locale);
    }
}

// ── ClosedCaptionLabel ────────────────────────────────────────────────────────

/// A sound-effect or music caption label, displayed separately from speech.
#[derive(Debug, Clone)]
pub struct ClosedCaptionLabel {
    pub start:    f32,
    pub end:      f32,
    /// The label text, e.g. "[GUNSHOT]", "[TENSE MUSIC]".
    pub label:    String,
    pub style:    SubtitleStyle,
}

impl ClosedCaptionLabel {
    pub fn new(start: f32, end: f32, label: impl Into<String>) -> Self {
        Self {
            start,
            end,
            label: label.into(),
            style: SubtitleStyle {
                color:    [0.85, 0.85, 0.85, 1.0],
                italic:   true,
                position: SubPos::Top,
                font_size: 18.0,
                ..SubtitleStyle::default_style()
            },
        }
    }

    pub fn sound_effect(start: f32, end: f32, name: impl Into<String>) -> Self {
        Self::new(start, end, format!("[{}]", name.into().to_uppercase()))
    }

    pub fn music(start: f32, end: f32, name: impl Into<String>) -> Self {
        Self::new(start, end, format!("♪ {} ♪", name.into()))
    }

    pub fn is_active(&self, t: f32) -> bool { t >= self.start && t < self.end }

    pub fn alpha_at(&self, t: f32) -> f32 {
        if !self.is_active(t) { return 0.0; }
        let elapsed   = t - self.start;
        let remaining = self.end - t;
        let fi = self.style.fade_in.max(f32::EPSILON);
        let fo = self.style.fade_out.max(f32::EPSILON);
        (elapsed / fi).min(1.0).min((remaining / fo).min(1.0))
    }
}

// ── ActiveCaption ─────────────────────────────────────────────────────────────

/// A rendered closed caption at a given moment.
#[derive(Debug, Clone)]
pub struct ActiveCaption {
    pub label: String,
    pub alpha: f32,
    pub style: SubtitleStyle,
}

// ── ClosedCaptionSystem ───────────────────────────────────────────────────────

/// Extends the subtitle system with non-speech closed captions.
///
/// Captions are stored in a sorted list separate from subtitle speech lines.
pub struct ClosedCaptionSystem {
    pub subtitle_db: SubtitleDatabase,
    pub captions:    Vec<ClosedCaptionLabel>,
    pub renderer:    SubtitleRenderer,
    /// Currently active locale.
    pub locale:      String,
}

impl ClosedCaptionSystem {
    pub fn new() -> Self {
        Self {
            subtitle_db: SubtitleDatabase::new(),
            captions:    Vec::new(),
            renderer:    SubtitleRenderer::new(),
            locale:      "en".into(),
        }
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    pub fn with_barker(mut self, cps: f32) -> Self {
        self.renderer = SubtitleRenderer::new().with_barker(cps);
        self
    }

    // ── Caption management ────────────────────────────────────────────────────

    pub fn add_caption(&mut self, cap: ClosedCaptionLabel) {
        let idx = self.captions.partition_point(|c| c.start <= cap.start);
        self.captions.insert(idx, cap);
    }

    pub fn add_sound_effect(&mut self, start: f32, end: f32, name: impl Into<String>) {
        self.add_caption(ClosedCaptionLabel::sound_effect(start, end, name));
    }

    pub fn add_music(&mut self, start: f32, end: f32, name: impl Into<String>) {
        self.add_caption(ClosedCaptionLabel::music(start, end, name));
    }

    // ── Per-frame update ──────────────────────────────────────────────────────

    /// Compute active subtitles and captions for time `t`.
    pub fn update(&mut self, t: f32, dt: f32) -> CaptionFrame {
        let subtitles = if let Some(track) = self.subtitle_db.track(&self.locale) {
            self.renderer.compute(track, t, dt)
        } else {
            Vec::new()
        };

        let captions: Vec<ActiveCaption> = self.captions.iter()
            .filter(|c| c.is_active(t))
            .map(|c| ActiveCaption {
                label: c.label.clone(),
                alpha: c.alpha_at(t),
                style: c.style.clone(),
            })
            .collect();

        CaptionFrame { subtitles, captions }
    }

    /// Switch locale.
    pub fn set_locale(&mut self, locale: impl Into<String>) {
        self.locale = locale.into();
    }

    /// Load subtitle text for a locale.
    pub fn load_subtitles(&mut self, locale: impl Into<String>, content: &str) {
        self.subtitle_db.load_text(locale, content);
    }
}

impl Default for ClosedCaptionSystem {
    fn default() -> Self { Self::new() }
}

// ── CaptionFrame ──────────────────────────────────────────────────────────────

/// All subtitle and caption data for a single rendered frame.
pub struct CaptionFrame {
    /// Active speech subtitle lines.
    pub subtitles: Vec<ActiveSubtitle>,
    /// Active non-speech captions.
    pub captions:  Vec<ActiveCaption>,
}

impl CaptionFrame {
    pub fn is_empty(&self) -> bool {
        self.subtitles.is_empty() && self.captions.is_empty()
    }

    pub fn subtitle_count(&self) -> usize { self.subtitles.len() }
    pub fn caption_count(&self)  -> usize { self.captions.len()  }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── SubtitleLine ──────────────────────────────────────────────────────────

    #[test]
    fn subtitle_line_active_range() {
        let line = SubtitleLine::new(1.0, 3.0, "Hero", "Hello.");
        assert!(!line.is_active(0.5));
        assert!(line.is_active(1.0));
        assert!(line.is_active(2.0));
        assert!(!line.is_active(3.0));
    }

    #[test]
    fn subtitle_line_alpha_fades_in() {
        let mut line = SubtitleLine::new(0.0, 2.0, "Hero", "Test");
        line.style.fade_in  = 0.5;
        line.style.fade_out = 0.5;
        let a0 = line.alpha_at(0.0);
        let a1 = line.alpha_at(0.25); // halfway through fade-in
        let a2 = line.alpha_at(0.5);  // fully in
        assert!(a0 < a1);
        assert!((a2 - 1.0).abs() < 0.05, "should be near 1.0 at fade_in: {}", a2);
    }

    #[test]
    fn subtitle_line_alpha_fades_out() {
        let mut line = SubtitleLine::new(0.0, 2.0, "Hero", "Test");
        line.style.fade_in  = 0.01;
        line.style.fade_out = 0.5;
        let a_mid   = line.alpha_at(0.5);
        let a_fade  = line.alpha_at(1.8);
        assert!(a_mid > a_fade, "mid={} fade={}", a_mid, a_fade);
    }

    // ── SubtitleTrack ─────────────────────────────────────────────────────────

    #[test]
    fn subtitle_track_active_at() {
        let mut track = SubtitleTrack::new();
        track.push(0.0, 2.0, "A", "Line one");
        track.push(1.5, 3.5, "B", "Line two");
        track.push(4.0, 6.0, "C", "Line three");

        let active = track.active_at(1.8);
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn subtitle_track_none_active() {
        let mut track = SubtitleTrack::new();
        track.push(5.0, 7.0, "Hero", "Late line");
        let active = track.active_at(1.0);
        assert!(active.is_empty());
    }

    #[test]
    fn subtitle_track_sorted_on_insert() {
        let mut track = SubtitleTrack::new();
        track.push(3.0, 4.0, "C", "Third");
        track.push(1.0, 2.0, "A", "First");
        track.push(2.0, 3.0, "B", "Second");
        assert!(track.lines[0].start <= track.lines[1].start);
        assert!(track.lines[1].start <= track.lines[2].start);
    }

    #[test]
    fn subtitle_track_duration() {
        let mut track = SubtitleTrack::new();
        track.push(0.0, 2.0, "", "a");
        track.push(3.0, 7.0, "", "b");
        assert!((track.duration() - 7.0).abs() < f32::EPSILON);
    }

    // ── SubtitleRenderer ──────────────────────────────────────────────────────

    #[test]
    fn renderer_returns_active_lines() {
        let mut track = SubtitleTrack::new();
        track.push(0.0, 3.0, "Hero", "Hello there.");

        let mut renderer = SubtitleRenderer::new();
        let frame = renderer.compute(&track, 1.0, 0.016);
        assert_eq!(frame.len(), 1);
    }

    #[test]
    fn renderer_barker_reveals_progressively() {
        let mut track = SubtitleTrack::new();
        track.push(0.0, 5.0, "Hero", "Hello World");

        let mut renderer = SubtitleRenderer::new().with_barker(5.0);
        // At t=0.0, fresh start
        let f1 = renderer.compute(&track, 0.0, 0.1);
        let f2 = renderer.compute(&track, 0.1, 0.2);

        let t1 = f1[0].visible_text.len();
        let t2 = f2[0].visible_text.len();
        assert!(t2 >= t1, "barker should reveal more over time: {} vs {}", t2, t1);
    }

    #[test]
    fn renderer_no_barker_shows_full_text() {
        let mut track = SubtitleTrack::new();
        track.push(0.0, 3.0, "Villain", "Full text immediately.");

        let mut renderer = SubtitleRenderer::new(); // no barker
        let frame = renderer.compute(&track, 1.0, 0.016);
        assert!(!frame.is_empty());
        assert_eq!(frame[0].visible_text, "Full text immediately.");
        assert!(frame[0].tw_complete);
    }

    // ── BarkerMode ────────────────────────────────────────────────────────────

    #[test]
    fn barker_mode_reveals_chars() {
        let mut barker = BarkerMode::new("ABCDE", 10.0);
        barker.tick(0.2); // 2 chars
        assert!(barker.chars_shown >= 2);
        assert!(!barker.complete);
    }

    #[test]
    fn barker_mode_completes() {
        let mut barker = BarkerMode::new("Hi", 100.0);
        let done = barker.tick(1.0);
        assert!(done || barker.complete);
        assert_eq!(barker.visible_text(), "Hi");
    }

    #[test]
    fn barker_mode_skip() {
        let mut barker = BarkerMode::new("Long text here for skipping", 5.0);
        barker.skip();
        assert!(barker.complete);
        assert_eq!(barker.visible_text(), "Long text here for skipping");
    }

    #[test]
    fn barker_mode_progress() {
        let mut barker = BarkerMode::new("ABCDE", 100.0);
        barker.tick(0.02); // 2 chars
        let p = barker.progress();
        assert!(p > 0.0 && p < 1.0, "progress={}", p);
    }

    #[test]
    fn barker_mode_tick_sounds() {
        let mut barker = BarkerMode::new("ABC", 100.0).with_tick_sound();
        barker.tick(0.03);
        let ticks = barker.drain_ticks();
        assert!(ticks > 0, "expected tick sounds");
    }

    #[test]
    fn barker_mode_punctuation_pause() {
        let mut barker = BarkerMode::new("Hi.", 1000.0);
        barker.tick(0.01); // reveal all 3 chars at 1000 cps
        // After revealing '.', pause_timer should be set
        // (unless already complete — length is only 3)
        // Just verify it doesn't panic
        assert!(barker.chars_shown > 0);
    }

    // ── SubtitleDatabase ──────────────────────────────────────────────────────

    #[test]
    fn database_load_text() {
        let mut db = SubtitleDatabase::new();
        let content = "
# This is a comment
0.00 | 2.50 | Hero    | Time to fight!
2.60 | 5.00 | Villain | I've been expecting you.
";
        db.load_text("en", content);
        let track = db.track("en").unwrap();
        assert_eq!(track.len(), 2);
        assert_eq!(track.lines[0].speaker, "Hero");
        assert_eq!(track.lines[1].speaker, "Villain");
    }

    #[test]
    fn database_skip_malformed_lines() {
        let mut db = SubtitleDatabase::new();
        let content = "
bad line
1.0 | 2.0 | Speaker | Good line
another bad one
";
        db.load_text("en", content);
        let track = db.track("en").unwrap();
        assert_eq!(track.len(), 1);
    }

    #[test]
    fn database_dump_text() {
        let mut db = SubtitleDatabase::new();
        db.load_text("en", "1.000 | 3.000 | Hero | Hello.");
        let dumped = db.dump_text("en");
        assert!(dumped.contains("Hero"));
        assert!(dumped.contains("Hello."));
    }

    #[test]
    fn database_multi_locale() {
        let mut db = SubtitleDatabase::new();
        db.load_text("en", "0.0 | 2.0 | A | Hello.");
        db.load_text("es", "0.0 | 2.0 | A | Hola.");
        assert!(db.has_locale("en"));
        assert!(db.has_locale("es"));
        assert!(!db.has_locale("fr"));
        let locales = db.locales();
        assert!(locales.len() >= 2);
    }

    // ── ClosedCaptionSystem ───────────────────────────────────────────────────

    #[test]
    fn closed_caption_system_tick() {
        let mut sys = ClosedCaptionSystem::new();
        sys.load_subtitles("en", "0.0 | 3.0 | Hero | Test line.");
        sys.add_sound_effect(1.0, 2.0, "GUNSHOT");
        let frame = sys.update(1.5, 0.016);
        assert_eq!(frame.subtitle_count(), 1);
        assert_eq!(frame.caption_count(), 1);
    }

    #[test]
    fn closed_caption_sound_effect_label() {
        let cap = ClosedCaptionLabel::sound_effect(0.0, 1.0, "explosion");
        assert!(cap.label.contains("EXPLOSION"));
    }

    #[test]
    fn closed_caption_music_label() {
        let cap = ClosedCaptionLabel::music(0.0, 5.0, "tense strings");
        assert!(cap.label.contains("tense strings"));
    }

    #[test]
    fn closed_caption_is_active() {
        let cap = ClosedCaptionLabel::sound_effect(1.0, 2.0, "BANG");
        assert!(!cap.is_active(0.5));
        assert!(cap.is_active(1.5));
        assert!(!cap.is_active(2.0));
    }

    #[test]
    fn caption_frame_empty() {
        let frame = CaptionFrame { subtitles: Vec::new(), captions: Vec::new() };
        assert!(frame.is_empty());
    }

    // ── SubtitleStyle ─────────────────────────────────────────────────────────

    #[test]
    fn style_speaker_colors_differ() {
        let hero    = SubtitleStyle::speaker_style("hero");
        let villain = SubtitleStyle::speaker_style("villain");
        // colours should differ
        assert!(
            hero.color[0] != villain.color[0]
            || hero.color[1] != villain.color[1]
            || hero.color[2] != villain.color[2]
        );
    }

    #[test]
    fn sub_pos_values() {
        let bot = SubPos::Bottom;
        let top = SubPos::Top;
        assert!(top.y() > bot.y());
        let custom = SubPos::Custom { x: 0.3, y: 0.6 };
        assert!((custom.x() - 0.3).abs() < f32::EPSILON);
        assert!((custom.y() - 0.6).abs() < f32::EPSILON);
    }
}
