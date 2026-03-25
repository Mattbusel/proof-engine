//! Drama management — pacing system using narrative arc templates.

/// Narrative tension state.
#[derive(Debug, Clone)]
pub struct DramaState {
    pub tension: f32,
    pub target_tension: f32,
    pub arc_position: f32,
    pub beats_since_action: u32,
    pub beats_since_quiet: u32,
}

impl DramaState {
    pub fn new() -> Self {
        Self { tension: 0.2, target_tension: 0.2, arc_position: 0.0, beats_since_action: 0, beats_since_quiet: 0 }
    }

    /// Update tension toward target, following a narrative arc.
    pub fn tick(&mut self, dt: f32) {
        self.arc_position = (self.arc_position + dt * 0.01).min(1.0);
        self.target_tension = narrative_arc(self.arc_position);
        let rate = if self.tension < self.target_tension { 0.3 } else { 0.15 };
        self.tension += (self.target_tension - self.tension) * rate * dt;
        self.tension = self.tension.clamp(0.0, 1.0);
    }

    /// Should we inject an action beat?
    pub fn needs_action(&self) -> bool {
        self.beats_since_action > 5 && self.tension < self.target_tension
    }

    /// Should we inject a quiet moment?
    pub fn needs_quiet(&self) -> bool {
        self.beats_since_quiet > 3 && self.tension > self.target_tension + 0.2
    }

    pub fn on_action(&mut self) { self.beats_since_action = 0; self.beats_since_quiet += 1; }
    pub fn on_quiet(&mut self) { self.beats_since_quiet = 0; self.beats_since_action += 1; }
}

/// Standard three-act narrative arc: setup (low) → rising → climax → falling → resolution.
fn narrative_arc(t: f32) -> f32 {
    if t < 0.15 { t * 2.0 }
    else if t < 0.6 { 0.3 + (t - 0.15) * 1.2 }
    else if t < 0.75 { 0.84 + (t - 0.6) * 1.0 }
    else { 1.0 - (t - 0.75) * 3.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_shape() {
        assert!(narrative_arc(0.0) < narrative_arc(0.5));
        assert!(narrative_arc(0.7) > narrative_arc(0.0));
        assert!(narrative_arc(1.0) < narrative_arc(0.7));
    }

    #[test]
    fn test_drama_tick() {
        let mut d = DramaState::new();
        for _ in 0..100 { d.tick(1.0); }
        assert!(d.tension > 0.0);
    }
}
