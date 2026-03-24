//! Combo system — input buffering, combo chains, and hit-confirm windows.

use std::collections::VecDeque;

// ── ComboInput ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComboInput {
    LightAttack,
    HeavyAttack,
    Special,
    Dodge,
    Block,
    Jump,
    Ability1,
    Ability2,
    Ability3,
    Ability4,
    Direction(ComboDirection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComboDirection {
    Forward,
    Backward,
    Up,
    Down,
    Neutral,
}

impl ComboInput {
    pub fn name(self) -> &'static str {
        match self {
            ComboInput::LightAttack  => "Light",
            ComboInput::HeavyAttack  => "Heavy",
            ComboInput::Special      => "Special",
            ComboInput::Dodge        => "Dodge",
            ComboInput::Block        => "Block",
            ComboInput::Jump         => "Jump",
            ComboInput::Ability1     => "Skill1",
            ComboInput::Ability2     => "Skill2",
            ComboInput::Ability3     => "Skill3",
            ComboInput::Ability4     => "Skill4",
            ComboInput::Direction(_) => "Dir",
        }
    }
}

// ── BufferedInput ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BufferedInput {
    pub input:     ComboInput,
    pub timestamp: f64,
    pub consumed:  bool,
}

/// Ring buffer of recent inputs for combo detection.
#[derive(Debug, Clone)]
pub struct InputBuffer {
    inputs:          VecDeque<BufferedInput>,
    max_size:        usize,
    buffer_window:   f64,  // seconds — inputs older than this are expired
    current_time:    f64,
}

impl InputBuffer {
    pub fn new(max_size: usize, buffer_window: f64) -> Self {
        Self {
            inputs: VecDeque::with_capacity(max_size),
            max_size,
            buffer_window,
            current_time: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.current_time += dt as f64;
        // Expire old inputs
        while let Some(front) = self.inputs.front() {
            if self.current_time - front.timestamp > self.buffer_window {
                self.inputs.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn push(&mut self, input: ComboInput) {
        if self.inputs.len() >= self.max_size {
            self.inputs.pop_front();
        }
        self.inputs.push_back(BufferedInput {
            input,
            timestamp: self.current_time,
            consumed: false,
        });
    }

    pub fn consume_next_unconsumed(&mut self) -> Option<ComboInput> {
        for entry in &mut self.inputs {
            if !entry.consumed {
                entry.consumed = true;
                return Some(entry.input);
            }
        }
        None
    }

    pub fn peek_sequence(&self, len: usize) -> Vec<ComboInput> {
        self.inputs.iter()
            .filter(|e| !e.consumed)
            .map(|e| e.input)
            .take(len)
            .collect()
    }

    pub fn clear(&mut self) {
        self.inputs.clear();
    }

    pub fn len(&self) -> usize {
        self.inputs.iter().filter(|e| !e.consumed).count()
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

// ── ComboLink ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ComboLink {
    pub input:         ComboInput,
    /// Maximum seconds after the previous hit's confirm window opens
    pub time_window:   f32,
    /// Minimum seconds (prevents mashing being too easy)
    pub min_delay:     f32,
}

impl ComboLink {
    pub fn new(input: ComboInput, window: f32) -> Self {
        Self { input, time_window: window, min_delay: 0.0 }
    }

    pub fn with_min_delay(mut self, d: f32) -> Self { self.min_delay = d; self }
}

// ── ComboHit ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ComboHit {
    pub animation_id:     String,
    pub damage_multiplier: f32,
    pub hitstop_duration:  f32,  // freeze frames on hit
    pub launch:            bool,
    pub knockback_force:   f32,
    pub can_cancel_into:   Vec<String>,  // combo IDs you can cancel into
    pub hit_confirm_start: f32,   // when the hitbox is active (relative to hit time)
    pub hit_confirm_end:   f32,
    pub glyph:             char,
    pub element:           Option<super::Element>,
}

impl ComboHit {
    pub fn new(anim: impl Into<String>, dmg_mult: f32) -> Self {
        Self {
            animation_id: anim.into(),
            damage_multiplier: dmg_mult,
            hitstop_duration: 0.1,
            launch: false,
            knockback_force: 0.0,
            can_cancel_into: Vec::new(),
            hit_confirm_start: 0.3,
            hit_confirm_end: 0.5,
            glyph: '✦',
            element: None,
        }
    }

    pub fn heavy(anim: impl Into<String>, dmg_mult: f32) -> Self {
        Self {
            animation_id: anim.into(),
            damage_multiplier: dmg_mult,
            hitstop_duration: 0.2,
            launch: false,
            knockback_force: 5.0,
            can_cancel_into: Vec::new(),
            hit_confirm_start: 0.4,
            hit_confirm_end: 0.7,
            glyph: '⚡',
            element: None,
        }
    }

    pub fn launcher(anim: impl Into<String>) -> Self {
        Self {
            animation_id: anim.into(),
            damage_multiplier: 0.8,
            hitstop_duration: 0.15,
            launch: true,
            knockback_force: 12.0,
            can_cancel_into: Vec::new(),
            hit_confirm_start: 0.35,
            hit_confirm_end: 0.6,
            glyph: '↑',
            element: None,
        }
    }

    pub fn with_element(mut self, el: super::Element) -> Self { self.element = Some(el); self }
    pub fn with_cancel(mut self, combo_id: impl Into<String>) -> Self {
        self.can_cancel_into.push(combo_id.into()); self
    }
}

// ── Combo ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Combo {
    pub id:     String,
    pub name:   String,
    pub links:  Vec<ComboLink>,  // input sequence to execute this combo
    pub hits:   Vec<ComboHit>,   // one per step in the chain
    pub ender:  Option<ComboEnder>,
    pub requires_airborne: bool,
    pub requires_grounded: bool,
    pub priority: u32,  // higher = checked first
}

#[derive(Debug, Clone)]
pub struct ComboEnder {
    pub name: String,
    pub damage_multiplier: f32,
    pub special_effect: Option<String>,
}

impl Combo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            links: Vec::new(),
            hits: Vec::new(),
            ender: None,
            requires_airborne: false,
            requires_grounded: false,
            priority: 0,
        }
    }

    pub fn add_link(mut self, input: ComboInput, window: f32) -> Self {
        self.links.push(ComboLink::new(input, window));
        self
    }

    pub fn add_hit(mut self, hit: ComboHit) -> Self {
        self.hits.push(hit);
        self
    }

    pub fn with_ender(mut self, name: impl Into<String>, dmg_mult: f32) -> Self {
        self.ender = Some(ComboEnder { name: name.into(), damage_multiplier: dmg_mult, special_effect: None });
        self
    }

    pub fn aerial(mut self) -> Self { self.requires_airborne = true; self }
    pub fn grounded(mut self) -> Self { self.requires_grounded = true; self }
    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn total_damage_multiplier(&self) -> f32 {
        let base: f32 = self.hits.iter().map(|h| h.damage_multiplier).sum();
        let ender_mult = self.ender.as_ref().map(|e| e.damage_multiplier).unwrap_or(1.0);
        base * ender_mult
    }

    pub fn input_sequence(&self) -> Vec<ComboInput> {
        self.links.iter().map(|l| l.input).collect()
    }

    /// Check if a given input history matches this combo's sequence.
    pub fn matches_sequence(&self, inputs: &[ComboInput]) -> bool {
        if inputs.len() < self.links.len() { return false; }
        let start = inputs.len() - self.links.len();
        inputs[start..].iter().zip(self.links.iter()).all(|(inp, link)| *inp == link.input)
    }
}

// ── ComboDatabase ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ComboDatabase {
    combos: Vec<Combo>,
}

impl ComboDatabase {
    pub fn new() -> Self { Self { combos: Vec::new() } }

    pub fn register(&mut self, combo: Combo) {
        self.combos.push(combo);
        // Sort by priority descending, then by length (longer = more specific)
        self.combos.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then(b.links.len().cmp(&a.links.len()))
        });
    }

    pub fn find_matching(&self, inputs: &[ComboInput], airborne: bool, grounded: bool) -> Option<&Combo> {
        self.combos.iter().find(|c| {
            c.matches_sequence(inputs)
                && (!c.requires_airborne || airborne)
                && (!c.requires_grounded || grounded)
        })
    }

    pub fn get(&self, id: &str) -> Option<&Combo> {
        self.combos.iter().find(|c| c.id == id)
    }

    pub fn len(&self) -> usize { self.combos.len() }

    /// Prebuilt combo database for a warrior-type character.
    pub fn warrior_presets() -> Self {
        let mut db = ComboDatabase::new();

        db.register(
            Combo::new("light_chain", "Light Chain")
                .add_link(ComboInput::LightAttack, 0.5)
                .add_link(ComboInput::LightAttack, 0.5)
                .add_link(ComboInput::LightAttack, 0.5)
                .add_hit(ComboHit::new("slash_1", 0.8))
                .add_hit(ComboHit::new("slash_2", 0.9))
                .add_hit(ComboHit::new("slash_3", 1.1))
                .with_ender("Final Slash", 1.3)
                .grounded()
                .with_priority(1)
        );

        db.register(
            Combo::new("heavy_opener", "Overhead Smash")
                .add_link(ComboInput::HeavyAttack, 0.8)
                .add_hit(ComboHit::heavy("overhead_smash", 1.8))
                .grounded()
                .with_priority(2)
        );

        db.register(
            Combo::new("light_into_heavy", "Rapid Crush")
                .add_link(ComboInput::LightAttack, 0.5)
                .add_link(ComboInput::LightAttack, 0.5)
                .add_link(ComboInput::HeavyAttack, 0.6)
                .add_hit(ComboHit::new("slash_1", 0.7))
                .add_hit(ComboHit::new("slash_2", 0.8))
                .add_hit(ComboHit::heavy("crush", 2.2))
                .with_ender("Shockwave", 1.5)
                .grounded()
                .with_priority(3)
        );

        db.register(
            Combo::new("launcher", "Rising Strike")
                .add_link(ComboInput::LightAttack, 0.5)
                .add_link(ComboInput::Direction(ComboDirection::Up), 0.3)
                .add_link(ComboInput::HeavyAttack, 0.5)
                .add_hit(ComboHit::new("rising_1", 0.6))
                .add_hit(ComboHit::launcher("launch_hit"))
                .grounded()
                .with_priority(4)
        );

        db.register(
            Combo::new("air_combo", "Aerial Rave")
                .add_link(ComboInput::LightAttack, 0.5)
                .add_link(ComboInput::LightAttack, 0.5)
                .add_link(ComboInput::LightAttack, 0.5)
                .add_hit(ComboHit::new("air_slash_1", 0.7))
                .add_hit(ComboHit::new("air_slash_2", 0.8))
                .add_hit(ComboHit::new("air_slash_3", 1.0))
                .with_ender("Air Slam", 2.0)
                .aerial()
                .with_priority(5)
        );

        db
    }
}

// ── ComboState ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ComboPhase {
    Idle,
    Attacking { hit_index: usize },
    HitConfirmWindow { hit_index: usize, window_remaining: f32 },
    Hitstop { duration_remaining: f32, resume_to_index: usize },
    Ender,
    Recovery { duration_remaining: f32 },
}

#[derive(Debug, Clone)]
pub struct ComboState {
    pub active_combo:   Option<String>,  // combo ID
    pub current_phase:  ComboPhase,
    pub hit_count:      u32,
    pub total_damage:   f32,
    pub combo_timer:    f32,       // time since last hit (for timeout)
    pub combo_timeout:  f32,       // max seconds between hits
    pub input_buffer:   InputBuffer,
    pub is_airborne:    bool,
    pub is_grounded:    bool,
}

impl ComboState {
    pub fn new() -> Self {
        Self {
            active_combo: None,
            current_phase: ComboPhase::Idle,
            hit_count: 0,
            total_damage: 0.0,
            combo_timer: 0.0,
            combo_timeout: 2.0,
            input_buffer: InputBuffer::new(16, 0.3),
            is_airborne: false,
            is_grounded: true,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.input_buffer.update(dt);
        self.combo_timer += dt;

        // Timeout active combo
        if self.active_combo.is_some() && self.combo_timer > self.combo_timeout {
            self.reset();
            return;
        }

        // Advance phase timers
        match &mut self.current_phase {
            ComboPhase::HitConfirmWindow { window_remaining, .. } => {
                *window_remaining -= dt;
                if *window_remaining <= 0.0 {
                    self.current_phase = ComboPhase::Idle;
                }
            }
            ComboPhase::Hitstop { duration_remaining, resume_to_index } => {
                *duration_remaining -= dt;
                if *duration_remaining <= 0.0 {
                    let idx = *resume_to_index;
                    self.current_phase = ComboPhase::HitConfirmWindow {
                        hit_index: idx,
                        window_remaining: 0.4,
                    };
                }
            }
            ComboPhase::Recovery { duration_remaining } => {
                *duration_remaining -= dt;
                if *duration_remaining <= 0.0 {
                    self.current_phase = ComboPhase::Idle;
                }
            }
            _ => {}
        }
    }

    pub fn register_input(&mut self, input: ComboInput) {
        self.input_buffer.push(input);
        self.combo_timer = 0.0;
    }

    pub fn register_hit(&mut self, damage: f32, hit: &ComboHit) {
        self.hit_count += 1;
        self.total_damage += damage;
        let next_idx = match &self.current_phase {
            ComboPhase::Attacking { hit_index } => *hit_index + 1,
            ComboPhase::HitConfirmWindow { hit_index, .. } => *hit_index + 1,
            _ => 0,
        };
        self.current_phase = ComboPhase::Hitstop {
            duration_remaining: hit.hitstop_duration,
            resume_to_index: next_idx,
        };
    }

    pub fn is_in_combo(&self) -> bool { self.active_combo.is_some() }

    pub fn start_combo(&mut self, combo_id: String) {
        self.active_combo = Some(combo_id);
        self.current_phase = ComboPhase::Attacking { hit_index: 0 };
        self.hit_count = 0;
        self.total_damage = 0.0;
        self.combo_timer = 0.0;
    }

    pub fn end_combo(&mut self) {
        self.current_phase = ComboPhase::Recovery { duration_remaining: 0.5 };
        self.active_combo = None;
    }

    pub fn reset(&mut self) {
        self.active_combo = None;
        self.current_phase = ComboPhase::Idle;
        self.hit_count = 0;
        self.total_damage = 0.0;
        self.combo_timer = 0.0;
        self.input_buffer.clear();
    }

    pub fn can_act(&self) -> bool {
        matches!(self.current_phase,
            ComboPhase::Idle |
            ComboPhase::HitConfirmWindow { .. }
        )
    }
}

// ── ComboTracker ──────────────────────────────────────────────────────────────

/// High-level tracker that wraps the combo database and per-entity combo state.
pub struct ComboTracker {
    pub database: ComboDatabase,
}

impl ComboTracker {
    pub fn new(database: ComboDatabase) -> Self {
        Self { database }
    }

    /// Try to start a combo from the current input buffer state.
    pub fn try_start(&self, state: &mut ComboState) -> Option<&Combo> {
        let inputs = state.input_buffer.peek_sequence(8);
        if inputs.is_empty() { return None; }
        let combo = self.database.find_matching(&inputs, state.is_airborne, state.is_grounded)?;
        state.start_combo(combo.id.clone());
        Some(combo)
    }

    /// Get the current active combo (if any).
    pub fn active_combo<'a>(&'a self, state: &ComboState) -> Option<&'a Combo> {
        state.active_combo.as_ref().and_then(|id| self.database.get(id))
    }

    /// Get the current hit in the active combo.
    pub fn current_hit<'a>(&'a self, state: &ComboState) -> Option<&'a ComboHit> {
        let combo = self.active_combo(state)?;
        let hit_idx = match &state.current_phase {
            ComboPhase::Attacking { hit_index }           => *hit_index,
            ComboPhase::HitConfirmWindow { hit_index, .. } => *hit_index,
            ComboPhase::Hitstop { resume_to_index, .. }   => resume_to_index.saturating_sub(1),
            _                                              => return None,
        };
        combo.hits.get(hit_idx)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_buffer() {
        let mut buf = InputBuffer::new(16, 1.0);
        buf.push(ComboInput::LightAttack);
        buf.push(ComboInput::LightAttack);
        buf.push(ComboInput::HeavyAttack);
        assert_eq!(buf.len(), 3);
        let seq = buf.peek_sequence(3);
        assert_eq!(seq[0], ComboInput::LightAttack);
        assert_eq!(seq[2], ComboInput::HeavyAttack);
    }

    #[test]
    fn test_combo_match() {
        let combo = Combo::new("test", "Test")
            .add_link(ComboInput::LightAttack, 0.5)
            .add_link(ComboInput::LightAttack, 0.5)
            .add_link(ComboInput::HeavyAttack, 0.6);

        let inputs = vec![ComboInput::LightAttack, ComboInput::LightAttack, ComboInput::HeavyAttack];
        assert!(combo.matches_sequence(&inputs));

        let wrong = vec![ComboInput::LightAttack, ComboInput::HeavyAttack, ComboInput::HeavyAttack];
        assert!(!combo.matches_sequence(&wrong));
    }

    #[test]
    fn test_combo_database_warrior() {
        let db = ComboDatabase::warrior_presets();
        assert!(db.len() > 0);

        let inputs = vec![
            ComboInput::LightAttack,
            ComboInput::LightAttack,
            ComboInput::LightAttack,
        ];
        let m = db.find_matching(&inputs, false, true);
        assert!(m.is_some());
    }

    #[test]
    fn test_combo_state_flow() {
        let mut state = ComboState::new();
        state.start_combo("test".to_string());
        assert!(state.is_in_combo());
        assert_eq!(state.hit_count, 0);

        let hit = ComboHit::new("slash", 1.0);
        state.register_hit(50.0, &hit);
        assert_eq!(state.hit_count, 1);
        assert!((state.total_damage - 50.0).abs() < 0.01);
    }
}
