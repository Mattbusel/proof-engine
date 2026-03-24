//! Script DSL for building timelines in a fluent, readable way.
//!
//! `CutsceneScript` provides a builder that accumulates actions with an
//! implicit cursor time — no need to manually specify timestamps for
//! every action when sequencing a dialogue scene or cutscene.

use glam::Vec3;
use std::collections::HashMap;

use super::{Timeline, TimelineAction};

// ── ScriptCursor ──────────────────────────────────────────────────────────────

/// Tracks the "pen position" — the time at which the next action will be placed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScriptCursor {
    pub time: f32,
}

impl ScriptCursor {
    pub fn advance(&mut self, dt: f32) -> f32 {
        self.time += dt;
        self.time
    }

    pub fn current(&self) -> f32 { self.time }
}

// ── CutsceneScript ────────────────────────────────────────────────────────────

/// Fluent builder for Timeline.  Maintains an implicit cursor time.
///
/// # Usage
/// ```text
/// let tl = CutsceneScript::new("my_scene")
///     .fade_in(1.0)
///     .wait(0.5)
///     .say("Hero", "Time to fight!")
///     .camera_shake(0.3, 0.6, 20.0)
///     .wait(1.0)
///     .fade_out(1.0)
///     .build();
/// ```
pub struct CutsceneScript {
    name:    String,
    cursor:  ScriptCursor,
    entries: Vec<(f32, TimelineAction)>,
    looping: bool,
    speed:   f32,
}

impl CutsceneScript {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name:    name.into(),
            cursor:  ScriptCursor::default(),
            entries: Vec::new(),
            looping: false,
            speed:   1.0,
        }
    }

    // ── Cursor control ───────────────────────────────────────────────────────

    /// Place cursor at an absolute time.
    pub fn at(mut self, time: f32) -> Self {
        self.cursor.time = time;
        self
    }

    /// Advance cursor by dt seconds (explicit wait).
    pub fn wait(mut self, dt: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::Wait { duration: dt }));
        self.cursor.advance(dt);
        self
    }

    /// Add a label at the current cursor position without advancing.
    pub fn label(mut self, name: impl Into<String>) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::GotoLabel { label: format!("__label_{}", name.into()) }));
        self
    }

    // ── Visual ───────────────────────────────────────────────────────────────

    pub fn fade_in(mut self, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::FadeIn { duration }));
        self.cursor.advance(duration);
        self
    }

    pub fn fade_out(mut self, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::FadeOut {
            duration,
            color: [0.0, 0.0, 0.0, 1.0],
        }));
        self.cursor.advance(duration);
        self
    }

    pub fn fade_to(mut self, color: [f32; 4], duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::FadeOut { color, duration }));
        self.cursor.advance(duration);
        self
    }

    pub fn flash(mut self, color: [f32; 4], duration: f32, intensity: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::Flash { color, duration, intensity }));
        // Flash is instantaneous from script perspective — no cursor advance
        self
    }

    pub fn bloom(mut self, enabled: bool, intensity: f32, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SetBloom { enabled, intensity, duration }));
        self
    }

    pub fn chromatic_aberration(mut self, amount: f32, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SetChromaticAberration { amount, duration }));
        self
    }

    pub fn film_grain(mut self, amount: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SetFilmGrain { amount }));
        self
    }

    pub fn vignette(mut self, radius: f32, softness: f32, intensity: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SetVignette { radius, softness, intensity }));
        self
    }

    // ── Camera ───────────────────────────────────────────────────────────────

    pub fn camera_move(mut self, target: Vec3, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::CameraMoveTo { target, duration }));
        self.cursor.advance(duration);
        self
    }

    pub fn camera_look_at(mut self, target: Vec3, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::CameraLookAt { target, duration }));
        self
    }

    pub fn camera_shake(mut self, intensity: f32, duration: f32, frequency: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::CameraShake { intensity, duration, frequency }));
        self
    }

    pub fn camera_zoom(mut self, zoom: f32, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::CameraZoom { zoom, duration }));
        self
    }

    // ── Entities ─────────────────────────────────────────────────────────────

    pub fn spawn(mut self, blueprint: impl Into<String>, position: Vec3) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SpawnEntity {
            blueprint: blueprint.into(),
            position,
            tag: None,
        }));
        self
    }

    pub fn spawn_tagged(mut self, blueprint: impl Into<String>, position: Vec3, tag: impl Into<String>) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SpawnEntity {
            blueprint: blueprint.into(),
            position,
            tag: Some(tag.into()),
        }));
        self
    }

    pub fn despawn_tag(mut self, tag: impl Into<String>) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::DespawnTag { tag: tag.into() }));
        self
    }

    // ── Audio ─────────────────────────────────────────────────────────────────

    pub fn play_sfx(mut self, name: impl Into<String>, volume: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::PlaySfx {
            name: name.into(),
            volume,
            position: None,
        }));
        self
    }

    pub fn play_sfx_at(mut self, name: impl Into<String>, volume: f32, position: Vec3) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::PlaySfx {
            name: name.into(),
            volume,
            position: Some(position),
        }));
        self
    }

    pub fn music_vibe(mut self, vibe: impl Into<String>) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SetMusicVibe { vibe: vibe.into() }));
        self
    }

    pub fn master_volume(mut self, volume: f32, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SetMasterVolume { volume, duration }));
        self
    }

    // ── UI / Dialogue ─────────────────────────────────────────────────────────

    /// Say a line of dialogue and advance cursor by `duration`.
    pub fn say(mut self, speaker: impl Into<String>, text: impl Into<String>, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::Dialogue {
            speaker:  speaker.into(),
            text:     text.into(),
            duration: Some(duration),
        }));
        self.cursor.advance(duration);
        self
    }

    /// Say a line without advancing cursor (fire-and-forget).
    pub fn say_async(mut self, speaker: impl Into<String>, text: impl Into<String>) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::Dialogue {
            speaker:  speaker.into(),
            text:     text.into(),
            duration: None,
        }));
        self
    }

    pub fn title_card(mut self, text: impl Into<String>, subtitle: impl Into<String>, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::TitleCard {
            text:     text.into(),
            subtitle: subtitle.into(),
            duration,
        }));
        self.cursor.advance(duration);
        self
    }

    pub fn notify(mut self, text: impl Into<String>, duration: f32) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::Notify { text: text.into(), duration }));
        self
    }

    pub fn hide_dialogue(mut self) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::HideDialogue));
        self
    }

    // ── Control ───────────────────────────────────────────────────────────────

    pub fn set_flag(mut self, name: impl Into<String>, value: bool) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::SetFlag { name: name.into(), value }));
        self
    }

    pub fn callback(mut self, name: impl Into<String>) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::Callback {
            name: name.into(),
            args: HashMap::new(),
        }));
        self
    }

    pub fn callback_with_args(mut self, name: impl Into<String>, args: HashMap<String, String>) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::Callback { name: name.into(), args }));
        self
    }

    /// Run multiple actions at the same cursor time.
    pub fn parallel(mut self, actions: Vec<TimelineAction>) -> Self {
        let t = self.cursor.time;
        self.entries.push((t, TimelineAction::Parallel { actions }));
        self
    }

    pub fn set_looping(mut self) -> Self { self.looping = true; self }
    pub fn set_speed(mut self, s: f32) -> Self { self.speed = s; self }

    /// Finalize and build the Timeline.
    pub fn build(mut self) -> Timeline {
        // Append End marker
        let end_t = self.cursor.time;
        self.entries.push((end_t, TimelineAction::End));

        let mut tl = Timeline::new()
            .named(self.name)
            .with_speed(self.speed);
        if self.looping { tl = tl.looping(); }

        for (time, action) in self.entries {
            tl.at(time, action);
        }
        tl
    }
}

// ── DialogueSequence — multiple lines in order ────────────────────────────────

/// Builds a dialogue-only sequence with automatic timing.
pub struct DialogueSequence {
    script:    CutsceneScript,
    chars_per_second: f32,
    pause_after: f32,  // seconds between lines
}

impl DialogueSequence {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            script:    CutsceneScript::new(name),
            chars_per_second: 25.0,
            pause_after: 0.4,
        }
    }

    pub fn with_speed(mut self, chars_per_second: f32) -> Self {
        self.chars_per_second = chars_per_second;
        self
    }

    pub fn with_pause(mut self, pause: f32) -> Self {
        self.pause_after = pause;
        self
    }

    /// Add a line.  Duration is computed from character count.
    pub fn line(mut self, speaker: impl Into<String>, text: impl Into<String>) -> Self {
        let t      = text.into();
        let dur    = (t.chars().count() as f32 / self.chars_per_second).max(1.0);
        let pause  = self.pause_after;
        self.script = self.script.say(speaker, t, dur).wait(pause);
        self
    }

    pub fn build(self) -> Timeline {
        self.script.hide_dialogue().build()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_builds_timeline() {
        let tl = CutsceneScript::new("test")
            .fade_in(1.0)
            .wait(0.5)
            .fade_out(0.5)
            .build();
        assert!(!tl.cues.is_empty());
        assert!(tl.duration() > 0.0);
    }

    #[test]
    fn script_cursor_advances() {
        let tl = CutsceneScript::new("test")
            .fade_in(1.0)    // cursor → 1.0
            .wait(2.0)       // cursor → 3.0
            .fade_out(0.5)   // cursor → 3.5
            .build();
        // fade_out should be at t=3.0 and End at t=3.5
        let fade_out_time = tl.cues.iter().find(|c| {
            matches!(&c.action, TimelineAction::FadeOut { .. })
        }).map(|c| c.time).unwrap();
        assert!((fade_out_time - 3.0).abs() < 0.01);
    }

    #[test]
    fn dialogue_sequence() {
        let tl = DialogueSequence::new("intro_dialogue")
            .with_speed(30.0)
            .line("Hero", "Hello there.")
            .line("Villain", "I've been expecting you.")
            .build();
        // Should have dialogue + hide
        let has_dialogue = tl.cues.iter().any(|c| matches!(&c.action, TimelineAction::Dialogue { .. }));
        assert!(has_dialogue);
    }

    #[test]
    fn script_at_positions_cursor() {
        let tl = CutsceneScript::new("test")
            .at(5.0)
            .flash([1.0,0.0,0.0,1.0], 0.2, 1.0)
            .build();
        let flash_time = tl.cues.iter().find(|c| {
            matches!(&c.action, TimelineAction::Flash { .. })
        }).map(|c| c.time).unwrap();
        assert!((flash_time - 5.0).abs() < 0.01);
    }
}
