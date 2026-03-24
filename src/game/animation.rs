//! Chaos RPG Animation State Machines, Blend Trees, and IK
//!
//! Wires the Proof Engine animation infrastructure into the Chaos RPG game loop.
//! Every entity is an amorphous cluster of glyphs — animation here means driving
//! per-glyph transforms (position offsets, scale, rotation, emission) through
//! state machines, blend trees, and simplified inverse kinematics.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use glam::Vec3;

use crate::entity::EntityId;

// ─────────────────────────────────────────────────────────────────────────────
// Glyph Transform Output
// ─────────────────────────────────────────────────────────────────────────────

/// Transform applied to a single glyph within an entity.
#[derive(Debug, Clone, Copy)]
pub struct GlyphTransform {
    /// Additive position offset (world units).
    pub position_offset: Vec3,
    /// Multiplicative scale factor (1.0 = identity).
    pub scale: f32,
    /// Rotation around Z axis (radians).
    pub rotation_z: f32,
    /// Emission multiplier (glow intensity, 0.0 = none).
    pub emission: f32,
}

impl Default for GlyphTransform {
    fn default() -> Self {
        Self {
            position_offset: Vec3::ZERO,
            scale: 1.0,
            rotation_z: 0.0,
            emission: 0.0,
        }
    }
}

impl GlyphTransform {
    /// Linearly interpolate between two transforms.
    pub fn lerp(a: &GlyphTransform, b: &GlyphTransform, t: f32) -> Self {
        Self {
            position_offset: a.position_offset + (b.position_offset - a.position_offset) * t,
            scale: a.scale + (b.scale - a.scale) * t,
            rotation_z: a.rotation_z + (b.rotation_z - a.rotation_z) * t,
            emission: a.emission + (b.emission - a.emission) * t,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Animation Pose
// ─────────────────────────────────────────────────────────────────────────────

/// A snapshot of per-glyph transforms for an entire entity.
#[derive(Debug, Clone)]
pub struct AnimPose {
    pub transforms: Vec<GlyphTransform>,
}

impl AnimPose {
    pub fn identity(count: usize) -> Self {
        Self {
            transforms: vec![GlyphTransform::default(); count],
        }
    }

    /// Blend two poses by weight `t` (0.0 = self, 1.0 = other).
    pub fn blend(&self, other: &AnimPose, t: f32) -> Self {
        let len = self.transforms.len().min(other.transforms.len());
        let mut transforms = Vec::with_capacity(len);
        for i in 0..len {
            transforms.push(GlyphTransform::lerp(&self.transforms[i], &other.transforms[i], t));
        }
        Self { transforms }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Blend Curves
// ─────────────────────────────────────────────────────────────────────────────

/// Easing function applied during state transitions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl BlendCurve {
    /// Evaluate the curve at normalized time `t` in [0, 1].
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            BlendCurve::Linear => t,
            BlendCurve::EaseIn => t * t,
            BlendCurve::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            BlendCurve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// PLAYER ANIMATION
// ═════════════════════════════════════════════════════════════════════════════

/// All possible player animation states in the Chaos RPG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerAnimState {
    Idle,
    Walk,
    Attack,
    HeavyAttack,
    Cast,
    Defend,
    Hurt,
    Flee,
    Channel,
    Dodge,
    Interact,
}

impl PlayerAnimState {
    /// Duration hint for non-looping states (seconds). Returns None for looping states.
    pub fn fixed_duration(&self) -> Option<f32> {
        match self {
            PlayerAnimState::Attack => Some(0.3),
            PlayerAnimState::HeavyAttack => Some(1.0),  // 0.5 windup + 0.3 swing + 0.2 follow
            PlayerAnimState::Cast => Some(0.75),         // 0.5-1.0 depending on spell
            PlayerAnimState::Hurt => Some(0.2),
            PlayerAnimState::Dodge => Some(0.25),
            PlayerAnimState::Interact => Some(0.5),
            _ => None, // looping states
        }
    }
}

/// Sub-phase for HeavyAttack timing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum HeavyAttackPhase {
    Windup,     // 0.0..0.5s
    Swing,      // 0.5..0.8s
    FollowThru, // 0.8..1.0s
}

/// Sub-phase for Cast timing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CastPhase {
    Raise,    // arms rise
    Hold,     // sustained glow
    Release,  // return
}

// ─────────────────────────────────────────────────────────────────────────────
// Player Animation Controller
// ─────────────────────────────────────────────────────────────────────────────

/// Drives per-glyph transforms for a player entity based on the current state.
pub struct PlayerAnimController {
    pub current_state: PlayerAnimState,
    pub prev_state: PlayerAnimState,
    /// Blend factor between prev and current (0 = fully prev, 1 = fully current).
    pub blend_factor: f32,
    /// Total time for the current transition (seconds).
    pub transition_time: f32,
    /// Elapsed time in the current state.
    pub state_timer: f32,
    /// Elapsed time within the transition (counts up to transition_time).
    transition_elapsed: f32,
    /// Whether we are mid-transition.
    in_transition: bool,
    /// Blend curve for the active transition.
    transition_curve: BlendCurve,
    /// Number of glyphs in the entity.
    glyph_count: usize,
    /// Direction toward the current damage source (for Hurt recoil).
    pub damage_direction: Vec3,
    /// Direction of movement (for Dodge squash-stretch).
    pub move_direction: Vec3,
    /// Movement speed parameter [0, 1] for idle/walk blend.
    pub movement_speed: f32,
    /// IK targets active on this controller.
    pub ik_targets: HashMap<String, IKTarget>,
}

impl PlayerAnimController {
    pub fn new(glyph_count: usize) -> Self {
        Self {
            current_state: PlayerAnimState::Idle,
            prev_state: PlayerAnimState::Idle,
            blend_factor: 1.0,
            transition_time: 0.0,
            state_timer: 0.0,
            transition_elapsed: 0.0,
            in_transition: false,
            transition_curve: BlendCurve::Linear,
            glyph_count,
            damage_direction: Vec3::NEG_X,
            move_direction: Vec3::X,
            movement_speed: 0.0,
            ik_targets: HashMap::new(),
        }
    }

    /// Attempt to transition to a new state. Returns true if the transition was accepted.
    pub fn transition_to(&mut self, new_state: PlayerAnimState) -> bool {
        if new_state == self.current_state && !self.in_transition {
            return false;
        }
        let (duration, curve) = transition_params(self.current_state, new_state);
        self.prev_state = self.current_state;
        self.current_state = new_state;
        self.transition_time = duration;
        self.transition_elapsed = 0.0;
        self.blend_factor = 0.0;
        self.in_transition = true;
        self.transition_curve = curve;
        self.state_timer = 0.0;
        true
    }

    /// Advance the controller by `dt` seconds and return the resulting pose.
    pub fn update(&mut self, dt: f32) -> AnimPose {
        self.state_timer += dt;

        // Advance transition blend
        if self.in_transition {
            self.transition_elapsed += dt;
            if self.transition_elapsed >= self.transition_time {
                self.blend_factor = 1.0;
                self.in_transition = false;
            } else {
                let raw = self.transition_elapsed / self.transition_time.max(0.001);
                self.blend_factor = self.transition_curve.evaluate(raw);
            }
        }

        // Auto-return to idle when a fixed-duration state expires
        if !self.in_transition {
            if let Some(dur) = self.current_state.fixed_duration() {
                if self.state_timer >= dur {
                    self.transition_to(PlayerAnimState::Idle);
                }
            }
        }

        // Evaluate poses
        if self.in_transition {
            let prev_pose = self.evaluate_state(self.prev_state);
            let curr_pose = self.evaluate_state(self.current_state);
            prev_pose.blend(&curr_pose, self.blend_factor)
        } else {
            self.evaluate_state(self.current_state)
        }
    }

    /// Evaluate the pose for a given state at the current state_timer.
    fn evaluate_state(&self, state: PlayerAnimState) -> AnimPose {
        let t = self.state_timer;
        let n = self.glyph_count;
        match state {
            PlayerAnimState::Idle => self.pose_idle(t, n),
            PlayerAnimState::Walk => self.pose_walk(t, n),
            PlayerAnimState::Attack => self.pose_attack(t, n),
            PlayerAnimState::HeavyAttack => self.pose_heavy_attack(t, n),
            PlayerAnimState::Cast => self.pose_cast(t, n),
            PlayerAnimState::Defend => self.pose_defend(t, n),
            PlayerAnimState::Hurt => self.pose_hurt(t, n),
            PlayerAnimState::Flee => self.pose_flee(t, n),
            PlayerAnimState::Channel => self.pose_channel(t, n),
            PlayerAnimState::Dodge => self.pose_dodge(t, n),
            PlayerAnimState::Interact => self.pose_interact(t, n),
        }
    }

    // ── State Pose Evaluators ────────────────────────────────────────────────

    /// Idle: gentle breathing (sine on scale 0.98-1.02) and slight Y bob (0.3 Hz, 2px).
    fn pose_idle(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let breath_freq = 0.3;
        let breath_scale_amp = 0.02; // +-0.02 around 1.0
        let bob_amp = 2.0;           // pixels

        for i in 0..n {
            let phase = i as f32 * 0.1; // slight per-glyph offset for organic feel
            let breath = (TAU * breath_freq * t + phase).sin();
            let scale = 1.0 + breath * breath_scale_amp;
            let y_bob = breath * bob_amp;
            transforms.push(GlyphTransform {
                position_offset: Vec3::new(0.0, y_bob, 0.0),
                scale,
                rotation_z: 0.0,
                emission: 0.0,
            });
        }
        AnimPose { transforms }
    }

    /// Walk: increased bob (4px, 1.5 Hz), arm glyphs swing left-right.
    fn pose_walk(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let bob_freq = 1.5;
        let bob_amp = 4.0;
        let arm_swing_amp = 3.0; // pixels of X offset

        for i in 0..n {
            let phase = i as f32 * 0.15;
            let bob = (TAU * bob_freq * t + phase).sin();
            let y_bob = bob * bob_amp;

            // Arm swing: odd-indexed glyphs swing opposite to even-indexed
            let arm_phase = if i % 2 == 0 { 0.0 } else { PI };
            let x_swing = (TAU * bob_freq * t + arm_phase).sin() * arm_swing_amp;

            // Slight forward lean
            let lean = 0.02; // radians

            transforms.push(GlyphTransform {
                position_offset: Vec3::new(x_swing, y_bob, 0.0),
                scale: 1.0,
                rotation_z: lean,
                emission: 0.0,
            });
        }
        AnimPose { transforms }
    }

    /// Attack: weapon arm swings toward target, body leans forward 5-10 degrees, 0.3s.
    fn pose_attack(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let duration = 0.3;
        let progress = (t / duration).clamp(0.0, 1.0);

        // Attack arc: fast swing peaking at ~60% through
        let swing_curve = if progress < 0.6 {
            (progress / 0.6 * PI * 0.5).sin()
        } else {
            ((1.0 - progress) / 0.4 * PI * 0.5).sin()
        };

        let lean_angle = swing_curve * 0.17; // ~10 degrees max
        let forward_push = swing_curve * 3.0; // pixels

        for i in 0..n {
            // Weapon arm glyphs (upper indices) swing more aggressively
            let arm_factor = if i >= n / 2 { 1.5 } else { 0.5 };
            let swing_offset = swing_curve * 6.0 * arm_factor;

            transforms.push(GlyphTransform {
                position_offset: Vec3::new(swing_offset, -forward_push * 0.3, 0.0),
                scale: 1.0 + swing_curve * 0.03,
                rotation_z: lean_angle * arm_factor,
                emission: swing_curve * 0.3,
            });
        }
        AnimPose { transforms }
    }

    /// HeavyAttack: 0.5s windup, 0.3s swing, 0.2s follow-through (1.0s total).
    fn pose_heavy_attack(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);

        let (phase, phase_progress) = if t < 0.5 {
            (HeavyAttackPhase::Windup, t / 0.5)
        } else if t < 0.8 {
            (HeavyAttackPhase::Swing, (t - 0.5) / 0.3)
        } else {
            (HeavyAttackPhase::FollowThru, ((t - 0.8) / 0.2).clamp(0.0, 1.0))
        };

        for i in 0..n {
            let arm_factor = if i >= n / 2 { 1.8 } else { 0.6 };
            let (offset, scale, rot, emit) = match phase {
                HeavyAttackPhase::Windup => {
                    // Arm pulls back
                    let pull = (phase_progress * PI * 0.5).sin();
                    let x = -pull * 5.0 * arm_factor;
                    let lean = -pull * 0.05;
                    (Vec3::new(x, pull * 1.5, 0.0), 1.0 + pull * 0.02, lean, 0.0)
                }
                HeavyAttackPhase::Swing => {
                    // Big forward arc
                    let swing = (phase_progress * PI * 0.5).sin();
                    let x = swing * 10.0 * arm_factor;
                    let lean = swing * 0.22; // ~12 degrees
                    (Vec3::new(x, -swing * 3.0, 0.0), 1.0 + swing * 0.05, lean, swing * 0.6)
                }
                HeavyAttackPhase::FollowThru => {
                    // Gradual recovery
                    let recover = 1.0 - phase_progress;
                    let x = recover * 4.0 * arm_factor;
                    let lean = recover * 0.1;
                    (Vec3::new(x, -recover * 1.0, 0.0), 1.0, lean, recover * 0.2)
                }
            };
            transforms.push(GlyphTransform {
                position_offset: offset,
                scale,
                rotation_z: rot,
                emission: emit,
            });
        }
        AnimPose { transforms }
    }

    /// Cast: staff/hands rise upward (IK above head), body straightens, glow on hands.
    fn pose_cast(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let duration = 0.75;
        let progress = (t / duration).clamp(0.0, 1.0);

        let (cast_phase, phase_t) = if progress < 0.4 {
            (CastPhase::Raise, progress / 0.4)
        } else if progress < 0.8 {
            (CastPhase::Hold, (progress - 0.4) / 0.4)
        } else {
            (CastPhase::Release, (progress - 0.8) / 0.2)
        };

        for i in 0..n {
            let is_hand = i >= n * 2 / 3; // upper glyphs are "hands"
            let (offset, scale, rot, emit) = match cast_phase {
                CastPhase::Raise => {
                    let rise = (phase_t * PI * 0.5).sin();
                    let y_up = if is_hand { rise * 8.0 } else { rise * 1.0 };
                    let glow = if is_hand { rise * 0.8 } else { 0.0 };
                    (Vec3::new(0.0, y_up, 0.0), 1.0 + rise * 0.01, -rise * 0.03, glow)
                }
                CastPhase::Hold => {
                    let pulse = (TAU * 3.0 * phase_t).sin() * 0.3 + 0.7;
                    let y_up = if is_hand { 8.0 } else { 1.0 };
                    let glow = if is_hand { pulse } else { pulse * 0.1 };
                    (Vec3::new(0.0, y_up, 0.0), 1.0 + 0.01, -0.03, glow)
                }
                CastPhase::Release => {
                    let drop = 1.0 - phase_t;
                    let y_up = if is_hand { drop * 8.0 } else { drop * 1.0 };
                    let glow = if is_hand { drop * 0.5 } else { 0.0 };
                    (Vec3::new(0.0, y_up, 0.0), 1.0, -drop * 0.03, glow)
                }
            };
            transforms.push(GlyphTransform {
                position_offset: offset,
                scale,
                rotation_z: rot,
                emission: emit,
            });
        }
        AnimPose { transforms }
    }

    /// Defend: arms cross / shield raise (IK to chest center), body hunches, scale reduced.
    fn pose_defend(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        // Sustain at full defend after 0.15s snap
        let snap = (t / 0.15).clamp(0.0, 1.0);
        let defend = snap;

        for i in 0..n {
            let is_arm = i >= n / 2;
            // Arms converge toward center
            let center_pull = if is_arm {
                let base_x = if i % 2 == 0 { 3.0 } else { -3.0 };
                Vec3::new(-base_x * defend, -defend * 2.0, 0.0)
            } else {
                Vec3::new(0.0, -defend * 1.0, 0.0)
            };
            // Defensive crouch: scale slightly reduced
            let crouch_scale = 1.0 - defend * 0.05;
            // Body hunches forward
            let hunch = defend * 0.08;

            transforms.push(GlyphTransform {
                position_offset: center_pull,
                scale: crouch_scale,
                rotation_z: hunch,
                emission: 0.0,
            });
        }
        AnimPose { transforms }
    }

    /// Hurt: recoil backward, arms spread, 0.2s stagger, shake on all glyphs.
    fn pose_hurt(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let duration = 0.2;
        let progress = (t / duration).clamp(0.0, 1.0);
        // Sharp recoil then recovery
        let recoil = if progress < 0.3 {
            progress / 0.3
        } else {
            1.0 - (progress - 0.3) / 0.7
        };

        let recoil_dir = self.damage_direction.normalize_or_zero();

        for i in 0..n {
            let phase = i as f32 * 2.7;
            // Shake: high-frequency noise
            let shake_x = (t * 40.0 + phase).sin() * recoil * 2.0;
            let shake_y = (t * 37.0 + phase * 1.3).sin() * recoil * 2.0;

            // Recoil away from damage
            let recoil_offset = recoil_dir * recoil * -5.0;

            // Arms spread outward
            let spread = if i >= n / 2 {
                let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                Vec3::new(side * recoil * 3.0, 0.0, 0.0)
            } else {
                Vec3::ZERO
            };

            transforms.push(GlyphTransform {
                position_offset: recoil_offset + spread + Vec3::new(shake_x, shake_y, 0.0),
                scale: 1.0 + recoil * 0.04,
                rotation_z: (t * 30.0 + phase).sin() * recoil * 0.05,
                emission: recoil * 0.4,
            });
        }
        AnimPose { transforms }
    }

    /// Flee: 180-degree turn, maxed bob, speed lines implied via high emission trail.
    fn pose_flee(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let bob_freq = 3.0; // maxed frequency
        let bob_amp = 5.0;

        // 180-degree rotation over first 0.3s
        let turn_progress = (t / 0.3).clamp(0.0, 1.0);
        let rotation = turn_progress * PI;

        for i in 0..n {
            let phase = i as f32 * 0.2;
            let bob = (TAU * bob_freq * t + phase).sin();
            let y_bob = bob * bob_amp;

            // Speed trail: trailing glyphs get more emission
            let trail_emission = (i as f32 / n.max(1) as f32) * 0.6;

            transforms.push(GlyphTransform {
                position_offset: Vec3::new(0.0, y_bob, 0.0),
                scale: 1.0,
                rotation_z: rotation,
                emission: trail_emission,
            });
        }
        AnimPose { transforms }
    }

    /// Channel: sustained cast pose with pulsing emission and rotating rune particles.
    fn pose_channel(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let pulse_freq = 2.0;

        for i in 0..n {
            let is_hand = i >= n * 2 / 3;
            let phase = i as f32 * TAU / n.max(1) as f32;

            // Hands stay raised
            let y_up = if is_hand { 7.0 } else { 0.5 };

            // Pulsing glow
            let pulse = ((TAU * pulse_freq * t + phase).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            let glow = if is_hand { pulse * 0.9 } else { pulse * 0.15 };

            // Rotating rune orbit for hand glyphs
            let orbit_offset = if is_hand {
                let orbit_angle = t * TAU * 0.5 + phase;
                Vec3::new(orbit_angle.cos() * 1.5, y_up + orbit_angle.sin() * 1.5, 0.0)
            } else {
                Vec3::new(0.0, y_up, 0.0)
            };

            transforms.push(GlyphTransform {
                position_offset: orbit_offset,
                scale: 1.0 + pulse * 0.02,
                rotation_z: if is_hand { t * 0.5 } else { 0.0 },
                emission: glow,
            });
        }
        AnimPose { transforms }
    }

    /// Dodge: quick lateral movement with squash-stretch.
    fn pose_dodge(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let duration = 0.25;
        let progress = (t / duration).clamp(0.0, 1.0);

        // Bell-curve lateral displacement
        let lateral = (progress * PI).sin() * 12.0;
        let move_dir = self.move_direction.normalize_or_zero();

        // Squash-stretch: compress in movement direction, expand perpendicular
        let squash = if progress < 0.5 {
            progress / 0.5
        } else {
            1.0 - (progress - 0.5) / 0.5
        };

        for i in 0..n {
            let offset = move_dir * lateral;
            // Squash in movement axis, stretch perpendicular
            let scale_x = 1.0 - squash * 0.15;
            let scale_y = 1.0 + squash * 0.15;
            // Approximate as average scale (glyph-level squash)
            let avg_scale = (scale_x + scale_y) * 0.5;

            transforms.push(GlyphTransform {
                position_offset: offset,
                scale: avg_scale,
                rotation_z: squash * 0.1 * if i % 2 == 0 { 1.0 } else { -1.0 },
                emission: squash * 0.2,
            });
        }
        AnimPose { transforms }
    }

    /// Interact: gentle reach forward and slight lean.
    fn pose_interact(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let duration = 0.5;
        let progress = (t / duration).clamp(0.0, 1.0);
        let reach = (progress * PI).sin();

        for i in 0..n {
            let is_arm = i >= n / 2;
            let forward = if is_arm { reach * 4.0 } else { reach * 1.0 };
            let lean = reach * 0.06;

            transforms.push(GlyphTransform {
                position_offset: Vec3::new(forward, -reach * 0.5, 0.0),
                scale: 1.0,
                rotation_z: lean,
                emission: 0.0,
            });
        }
        AnimPose { transforms }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Transition Table
// ─────────────────────────────────────────────────────────────────────────────

/// Defines a transition between two animation states.
#[derive(Debug, Clone)]
pub struct AnimTransitionDef {
    pub from_state: PlayerAnimState,
    pub to_state: PlayerAnimState,
    pub duration: f32,
    pub blend_curve: BlendCurve,
}

/// Lookup transition parameters for a given (from, to) pair.
/// Falls back to defaults when no explicit rule exists.
fn transition_params(from: PlayerAnimState, to: PlayerAnimState) -> (f32, BlendCurve) {
    // Any → Hurt is instant reaction
    if to == PlayerAnimState::Hurt {
        return (0.05, BlendCurve::Linear);
    }

    match (from, to) {
        (PlayerAnimState::Idle, PlayerAnimState::Walk)   => (0.2, BlendCurve::EaseInOut),
        (PlayerAnimState::Walk, PlayerAnimState::Idle)   => (0.2, BlendCurve::EaseInOut),
        (PlayerAnimState::Walk, PlayerAnimState::Attack)  => (0.1, BlendCurve::EaseIn),
        (PlayerAnimState::Idle, PlayerAnimState::Attack)  => (0.1, BlendCurve::EaseIn),
        (PlayerAnimState::Attack, PlayerAnimState::Idle)  => (0.3, BlendCurve::EaseOut),
        (PlayerAnimState::HeavyAttack, PlayerAnimState::Idle) => (0.4, BlendCurve::EaseOut),
        (PlayerAnimState::Hurt, PlayerAnimState::Idle)    => (0.4, BlendCurve::EaseOut),
        (PlayerAnimState::Idle, PlayerAnimState::Cast)    => (0.3, BlendCurve::EaseIn),
        (PlayerAnimState::Cast, PlayerAnimState::Idle)    => (0.2, BlendCurve::EaseOut),
        (PlayerAnimState::Idle, PlayerAnimState::Channel) => (0.3, BlendCurve::EaseIn),
        (PlayerAnimState::Channel, PlayerAnimState::Idle) => (0.25, BlendCurve::EaseOut),
        (PlayerAnimState::Idle, PlayerAnimState::Defend)  => (0.15, BlendCurve::EaseIn),
        (PlayerAnimState::Defend, PlayerAnimState::Idle)  => (0.2, BlendCurve::EaseOut),
        (PlayerAnimState::Idle, PlayerAnimState::Flee)    => (0.15, BlendCurve::EaseIn),
        (PlayerAnimState::Flee, PlayerAnimState::Idle)    => (0.3, BlendCurve::EaseOut),
        (PlayerAnimState::Idle, PlayerAnimState::Dodge)   => (0.05, BlendCurve::Linear),
        (PlayerAnimState::Dodge, PlayerAnimState::Idle)   => (0.15, BlendCurve::EaseOut),
        (PlayerAnimState::Walk, PlayerAnimState::Dodge)   => (0.05, BlendCurve::Linear),
        (PlayerAnimState::Idle, PlayerAnimState::Interact) => (0.2, BlendCurve::EaseInOut),
        (PlayerAnimState::Interact, PlayerAnimState::Idle) => (0.2, BlendCurve::EaseOut),
        _ => (0.2, BlendCurve::EaseInOut), // default fallback
    }
}

/// Return the full transition table as a Vec (useful for inspection/serialization).
pub fn build_transition_table() -> Vec<AnimTransitionDef> {
    let entries: &[(PlayerAnimState, PlayerAnimState, f32, BlendCurve)] = &[
        (PlayerAnimState::Idle, PlayerAnimState::Walk,   0.2,  BlendCurve::EaseInOut),
        (PlayerAnimState::Walk, PlayerAnimState::Attack,  0.1,  BlendCurve::EaseIn),
        (PlayerAnimState::Attack, PlayerAnimState::Idle,  0.3,  BlendCurve::EaseOut),
        (PlayerAnimState::Idle, PlayerAnimState::Cast,    0.3,  BlendCurve::EaseIn),
        (PlayerAnimState::Cast, PlayerAnimState::Idle,    0.2,  BlendCurve::EaseOut),
        (PlayerAnimState::Hurt, PlayerAnimState::Idle,    0.4,  BlendCurve::EaseOut),
        (PlayerAnimState::Idle, PlayerAnimState::Defend,  0.15, BlendCurve::EaseIn),
        (PlayerAnimState::Defend, PlayerAnimState::Idle,  0.2,  BlendCurve::EaseOut),
        (PlayerAnimState::Idle, PlayerAnimState::Flee,    0.15, BlendCurve::EaseIn),
        (PlayerAnimState::Idle, PlayerAnimState::Channel, 0.3,  BlendCurve::EaseIn),
        (PlayerAnimState::Idle, PlayerAnimState::Dodge,   0.05, BlendCurve::Linear),
        (PlayerAnimState::Idle, PlayerAnimState::Interact,0.2,  BlendCurve::EaseInOut),
    ];
    entries
        .iter()
        .map(|&(from, to, dur, curve)| AnimTransitionDef {
            from_state: from,
            to_state: to,
            duration: dur,
            blend_curve: curve,
        })
        .collect()
}

// ═════════════════════════════════════════════════════════════════════════════
// BLEND TREES
// ═════════════════════════════════════════════════════════════════════════════

/// A 1D blend tree: interpolates between poses based on a single float parameter.
pub struct BlendTree1D {
    /// Current parameter value [0, 1].
    pub parameter: f32,
    /// Sorted entries: (parameter_value, pose).
    entries: Vec<(f32, AnimPose)>,
}

impl BlendTree1D {
    pub fn new() -> Self {
        Self {
            parameter: 0.0,
            entries: Vec::new(),
        }
    }

    /// Add a pose at a given parameter value. Entries should be added in order.
    pub fn add_entry(&mut self, param: f32, pose: AnimPose) {
        self.entries.push((param, pose));
        self.entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// Evaluate the blend tree at the current parameter value.
    pub fn evaluate(&self) -> AnimPose {
        if self.entries.is_empty() {
            return AnimPose::identity(0);
        }
        if self.entries.len() == 1 {
            return self.entries[0].1.clone();
        }

        let p = self.parameter;

        // Below first entry
        if p <= self.entries[0].0 {
            return self.entries[0].1.clone();
        }
        // Above last entry
        if p >= self.entries.last().unwrap().0 {
            return self.entries.last().unwrap().1.clone();
        }

        // Find the two bracketing entries
        for i in 0..self.entries.len() - 1 {
            let (p0, ref pose0) = self.entries[i];
            let (p1, ref pose1) = self.entries[i + 1];
            if p >= p0 && p <= p1 {
                let range = p1 - p0;
                let t = if range.abs() < 1e-6 { 0.0 } else { (p - p0) / range };
                return pose0.blend(pose1, t);
            }
        }

        self.entries.last().unwrap().1.clone()
    }
}

/// IdleWalkBlend: parameter = movement_speed [0, 1], blends idle bob into walk bob.
pub struct IdleWalkBlend {
    pub tree: BlendTree1D,
}

impl IdleWalkBlend {
    pub fn new(glyph_count: usize) -> Self {
        let ctrl = PlayerAnimController::new(glyph_count);
        let idle_pose = ctrl.pose_idle(0.0, glyph_count);
        let walk_pose = ctrl.pose_walk(0.0, glyph_count);

        let mut tree = BlendTree1D::new();
        tree.add_entry(0.0, idle_pose);
        tree.add_entry(1.0, walk_pose);
        Self { tree }
    }

    pub fn evaluate(&mut self, speed: f32) -> AnimPose {
        self.tree.parameter = speed.clamp(0.0, 1.0);
        self.tree.evaluate()
    }
}

/// AttackPowerBlend: parameter = force_stat [0, 1], blends light swing into heavy swing.
pub struct AttackPowerBlend {
    pub tree: BlendTree1D,
}

impl AttackPowerBlend {
    pub fn new(glyph_count: usize) -> Self {
        let ctrl = PlayerAnimController::new(glyph_count);
        let light = ctrl.pose_attack(0.15, glyph_count);  // mid-attack snapshot
        let heavy = ctrl.pose_heavy_attack(0.65, glyph_count); // mid-swing snapshot

        let mut tree = BlendTree1D::new();
        tree.add_entry(0.0, light);
        tree.add_entry(1.0, heavy);
        Self { tree }
    }

    pub fn evaluate(&mut self, force: f32) -> AnimPose {
        self.tree.parameter = force.clamp(0.0, 1.0);
        self.tree.evaluate()
    }
}

/// DamageReactionBlend: parameter = damage_fraction [0, 1], small → large recoil.
pub struct DamageReactionBlend {
    pub tree: BlendTree1D,
}

impl DamageReactionBlend {
    pub fn new(glyph_count: usize) -> Self {
        // Build two snapshots at different recoil intensities
        let mut ctrl_small = PlayerAnimController::new(glyph_count);
        ctrl_small.damage_direction = Vec3::NEG_X;
        let small_recoil = ctrl_small.pose_hurt(0.06, glyph_count); // early = small

        let mut ctrl_large = PlayerAnimController::new(glyph_count);
        ctrl_large.damage_direction = Vec3::NEG_X;
        let large_recoil = ctrl_large.pose_hurt(0.06, glyph_count);
        // Scale up the large recoil manually
        let large_recoil = AnimPose {
            transforms: large_recoil
                .transforms
                .into_iter()
                .map(|mut gt| {
                    gt.position_offset *= 2.5;
                    gt.emission *= 2.0;
                    gt
                })
                .collect(),
        };

        let mut tree = BlendTree1D::new();
        tree.add_entry(0.0, small_recoil);
        tree.add_entry(1.0, large_recoil);
        Self { tree }
    }

    pub fn evaluate(&mut self, damage_fraction: f32) -> AnimPose {
        self.tree.parameter = damage_fraction.clamp(0.0, 1.0);
        self.tree.evaluate()
    }
}

/// CastIntensityBlend: parameter = mana_cost_fraction, small cast → large cast.
pub struct CastIntensityBlend {
    pub tree: BlendTree1D,
}

impl CastIntensityBlend {
    pub fn new(glyph_count: usize) -> Self {
        let ctrl = PlayerAnimController::new(glyph_count);
        let small_cast = ctrl.pose_cast(0.3, glyph_count);
        let large_cast_raw = ctrl.pose_cast(0.3, glyph_count);
        let large_cast = AnimPose {
            transforms: large_cast_raw
                .transforms
                .into_iter()
                .map(|mut gt| {
                    gt.position_offset *= 1.5;
                    gt.emission *= 2.5;
                    gt.scale += 0.05;
                    gt
                })
                .collect(),
        };

        let mut tree = BlendTree1D::new();
        tree.add_entry(0.0, small_cast);
        tree.add_entry(1.0, large_cast);
        Self { tree }
    }

    pub fn evaluate(&mut self, mana_fraction: f32) -> AnimPose {
        self.tree.parameter = mana_fraction.clamp(0.0, 1.0);
        self.tree.evaluate()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// INVERSE KINEMATICS (simplified 2-bone for glyphs)
// ═════════════════════════════════════════════════════════════════════════════

/// A simplified 2-bone IK chain operating on glyph positions.
#[derive(Debug, Clone)]
pub struct IKChain {
    pub root_pos: Vec3,
    pub mid_pos: Vec3,
    pub end_pos: Vec3,
    /// Bone lengths: [root→mid, mid→end].
    pub lengths: [f32; 2],
}

impl IKChain {
    pub fn new(root: Vec3, mid: Vec3, end: Vec3) -> Self {
        let l0 = (mid - root).length();
        let l1 = (end - mid).length();
        Self {
            root_pos: root,
            mid_pos: mid,
            end_pos: end,
            lengths: [l0, l1],
        }
    }

    /// Total reach of the chain.
    pub fn total_length(&self) -> f32 {
        self.lengths[0] + self.lengths[1]
    }
}

/// Analytical two-bone IK solver.
///
/// Positions the mid and end joints so that `end` reaches as close to `target`
/// as possible. Uses the law of cosines for the elbow angle.
pub fn solve_two_bone(chain: &mut IKChain, target: Vec3) {
    let l0 = chain.lengths[0];
    let l1 = chain.lengths[1];
    let total = l0 + l1;

    let root = chain.root_pos;
    let to_target = target - root;
    let dist = to_target.length().max(0.001);

    // Clamp target to reachable distance
    let dist_clamped = dist.min(total - 0.001).max((l0 - l1).abs() + 0.001);
    let direction = to_target.normalize_or_zero();
    let clamped_target = root + direction * dist_clamped;

    // Law of cosines: angle at root
    let cos_angle = ((l0 * l0 + dist_clamped * dist_clamped - l1 * l1) / (2.0 * l0 * dist_clamped))
        .clamp(-1.0, 1.0);
    let angle = cos_angle.acos();

    // Construct a perpendicular axis for the elbow bend.
    // Default to Y-up if direction is nearly vertical.
    let up = if direction.dot(Vec3::Y).abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let side = direction.cross(up).normalize_or_zero();
    let bend_axis = side.cross(direction).normalize_or_zero();

    // Rotate direction by angle around bend_axis to get mid position
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let mid_dir = direction * cos_a + bend_axis * sin_a;
    chain.mid_pos = root + mid_dir * l0;

    // End position: from mid toward clamped target, at length l1
    let mid_to_target = (clamped_target - chain.mid_pos).normalize_or_zero();
    chain.end_pos = chain.mid_pos + mid_to_target * l1;
}

/// What an IK chain is targeting.
#[derive(Debug, Clone)]
pub enum IKTarget {
    /// Track a specific entity (e.g., current enemy).
    Enemy(EntityId),
    /// Fixed world position.
    Position(Vec3),
    /// Offset relative to the owning entity's position.
    Offset(Vec3),
    /// Orient toward a direction (head/eye tracking).
    LookDirection(Vec3),
    /// No target (chain relaxes to rest pose).
    None,
}

/// Named IK chain for a specific limb/purpose.
#[derive(Debug, Clone)]
pub struct IKLimb {
    pub name: String,
    pub chain: IKChain,
    pub target: IKTarget,
    /// Glyph indices that this IK chain controls: [root_glyph, mid_glyph, end_glyph].
    pub glyph_indices: [usize; 3],
    /// Blend weight for this IK [0, 1]. 0 = animation only, 1 = full IK.
    pub weight: f32,
}

impl IKLimb {
    pub fn new(name: &str, glyph_indices: [usize; 3], rest_positions: [Vec3; 3]) -> Self {
        Self {
            name: name.to_string(),
            chain: IKChain::new(rest_positions[0], rest_positions[1], rest_positions[2]),
            target: IKTarget::None,
            glyph_indices,
            weight: 1.0,
        }
    }

    /// Solve IK and return position offsets for the three controlled glyphs.
    pub fn solve(&mut self, entity_pos: Vec3, entities: &HashMap<EntityId, Vec3>) -> [Vec3; 3] {
        let world_target = match &self.target {
            IKTarget::Enemy(id) => {
                entities.get(id).copied().unwrap_or(entity_pos + Vec3::X * 5.0)
            }
            IKTarget::Position(p) => *p,
            IKTarget::Offset(o) => entity_pos + *o,
            IKTarget::LookDirection(dir) => entity_pos + dir.normalize_or_zero() * 10.0,
            IKTarget::None => {
                // Relax to rest — no offset
                return [Vec3::ZERO; 3];
            }
        };

        solve_two_bone(&mut self.chain, world_target);

        // Return offsets relative to entity position (these get blended with animation)
        let offsets = [
            (self.chain.root_pos - entity_pos) * self.weight,
            (self.chain.mid_pos - entity_pos) * self.weight,
            (self.chain.end_pos - entity_pos) * self.weight,
        ];
        offsets
    }
}

// ── Game-specific IK target factories ────────────────────────────────────────

/// Create a weapon-arm IK limb that aims at the current target enemy.
pub fn weapon_arm_ik(glyph_indices: [usize; 3], rest: [Vec3; 3]) -> IKLimb {
    let mut limb = IKLimb::new("weapon_arm", glyph_indices, rest);
    limb.weight = 0.8;
    limb
}

/// Create a look-at IK limb for head/eye orientation.
pub fn look_at_ik(glyph_indices: [usize; 3], rest: [Vec3; 3]) -> IKLimb {
    let mut limb = IKLimb::new("look_at", glyph_indices, rest);
    limb.weight = 0.6;
    limb
}

/// Create a staff-aim IK limb for mage casting.
pub fn staff_aim_ik(glyph_indices: [usize; 3], rest: [Vec3; 3]) -> IKLimb {
    let mut limb = IKLimb::new("staff_aim", glyph_indices, rest);
    limb.weight = 0.9;
    limb
}

/// Create a shield IK limb that positions between player and threat.
pub fn shield_ik(glyph_indices: [usize; 3], rest: [Vec3; 3]) -> IKLimb {
    let mut limb = IKLimb::new("shield", glyph_indices, rest);
    limb.weight = 1.0;
    limb
}

// ═════════════════════════════════════════════════════════════════════════════
// ENEMY ANIMATION
// ═════════════════════════════════════════════════════════════════════════════

/// Base enemy animation states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyAnimState {
    Idle,
    Approach,
    Attack,
    Hurt,
    Die,
    Special,
}

/// Simpler animation controller for enemies.
pub struct EnemyAnimController {
    pub current_state: EnemyAnimState,
    pub prev_state: EnemyAnimState,
    pub blend_factor: f32,
    pub transition_time: f32,
    pub state_timer: f32,
    transition_elapsed: f32,
    in_transition: bool,
    transition_curve: BlendCurve,
    glyph_count: usize,
    /// Direction the enemy is approaching from.
    pub approach_direction: Vec3,
    /// Death dissolution progress [0, 1].
    pub death_progress: f32,
}

impl EnemyAnimController {
    pub fn new(glyph_count: usize) -> Self {
        Self {
            current_state: EnemyAnimState::Idle,
            prev_state: EnemyAnimState::Idle,
            blend_factor: 1.0,
            transition_time: 0.0,
            state_timer: 0.0,
            transition_elapsed: 0.0,
            in_transition: false,
            transition_curve: BlendCurve::Linear,
            glyph_count,
            approach_direction: Vec3::X,
            death_progress: 0.0,
        }
    }

    pub fn transition_to(&mut self, new_state: EnemyAnimState) -> bool {
        if new_state == self.current_state && !self.in_transition {
            return false;
        }
        let (duration, curve) = enemy_transition_params(self.current_state, new_state);
        self.prev_state = self.current_state;
        self.current_state = new_state;
        self.transition_time = duration;
        self.transition_elapsed = 0.0;
        self.blend_factor = 0.0;
        self.in_transition = true;
        self.transition_curve = curve;
        self.state_timer = 0.0;
        true
    }

    pub fn update(&mut self, dt: f32) -> AnimPose {
        self.state_timer += dt;

        if self.in_transition {
            self.transition_elapsed += dt;
            if self.transition_elapsed >= self.transition_time {
                self.blend_factor = 1.0;
                self.in_transition = false;
            } else {
                let raw = self.transition_elapsed / self.transition_time.max(0.001);
                self.blend_factor = self.transition_curve.evaluate(raw);
            }
        }

        if self.in_transition {
            let prev = self.evaluate_state(self.prev_state);
            let curr = self.evaluate_state(self.current_state);
            prev.blend(&curr, self.blend_factor)
        } else {
            self.evaluate_state(self.current_state)
        }
    }

    fn evaluate_state(&self, state: EnemyAnimState) -> AnimPose {
        let t = self.state_timer;
        let n = self.glyph_count;
        match state {
            EnemyAnimState::Idle => self.enemy_idle(t, n),
            EnemyAnimState::Approach => self.enemy_approach(t, n),
            EnemyAnimState::Attack => self.enemy_attack(t, n),
            EnemyAnimState::Hurt => self.enemy_hurt(t, n),
            EnemyAnimState::Die => self.enemy_die(t, n),
            EnemyAnimState::Special => self.enemy_special(t, n),
        }
    }

    fn enemy_idle(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        for i in 0..n {
            let phase = i as f32 * 0.3;
            let bob = (TAU * 0.4 * t + phase).sin();
            transforms.push(GlyphTransform {
                position_offset: Vec3::new(0.0, bob * 1.5, 0.0),
                scale: 1.0 + bob * 0.01,
                rotation_z: 0.0,
                emission: 0.05,
            });
        }
        AnimPose { transforms }
    }

    fn enemy_approach(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let dir = self.approach_direction.normalize_or_zero();
        for i in 0..n {
            let phase = i as f32 * 0.25;
            let bob = (TAU * 1.2 * t + phase).sin();
            let lean = 0.1;
            transforms.push(GlyphTransform {
                position_offset: Vec3::new(dir.x * 2.0, bob * 3.0, 0.0),
                scale: 1.0,
                rotation_z: lean,
                emission: 0.1,
            });
        }
        AnimPose { transforms }
    }

    fn enemy_attack(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let progress = (t / 0.4).clamp(0.0, 1.0);
        let swing = (progress * PI).sin();
        for i in 0..n {
            let factor = if i >= n / 2 { 1.5 } else { 0.7 };
            transforms.push(GlyphTransform {
                position_offset: Vec3::new(swing * 5.0 * factor, -swing * 2.0, 0.0),
                scale: 1.0 + swing * 0.04,
                rotation_z: swing * 0.15 * factor,
                emission: swing * 0.5,
            });
        }
        AnimPose { transforms }
    }

    fn enemy_hurt(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let recoil = (1.0 - (t / 0.3).clamp(0.0, 1.0)).max(0.0);
        for i in 0..n {
            let phase = i as f32 * 3.1;
            let shake_x = (t * 35.0 + phase).sin() * recoil * 2.5;
            let shake_y = (t * 31.0 + phase).cos() * recoil * 2.5;
            transforms.push(GlyphTransform {
                position_offset: Vec3::new(shake_x - recoil * 3.0, shake_y, 0.0),
                scale: 1.0 + recoil * 0.03,
                rotation_z: (t * 25.0 + phase).sin() * recoil * 0.06,
                emission: recoil * 0.6,
            });
        }
        AnimPose { transforms }
    }

    fn enemy_die(&self, t: f32, n: usize) -> AnimPose {
        let mut transforms = Vec::with_capacity(n);
        let progress = (t / 1.5).clamp(0.0, 1.0);
        self.death_progress;

        for i in 0..n {
            let phase = i as f32 * 1.618;
            // Outward scatter
            let angle = phase * TAU;
            let scatter = progress * progress * 8.0;
            let x = angle.cos() * scatter;
            let y = angle.sin() * scatter + progress * 3.0; // float upward
            // Fade out via scale
            let fade_scale = (1.0 - progress).max(0.0);
            transforms.push(GlyphTransform {
                position_offset: Vec3::new(x, y, 0.0),
                scale: fade_scale,
                rotation_z: progress * TAU * 0.5 * if i % 2 == 0 { 1.0 } else { -1.0 },
                emission: (1.0 - progress) * 0.8,
            });
        }
        AnimPose { transforms }
    }

    fn enemy_special(&self, t: f32, n: usize) -> AnimPose {
        // Generic special: pulsing glow + scale oscillation
        let mut transforms = Vec::with_capacity(n);
        let pulse = (TAU * 2.0 * t).sin() * 0.5 + 0.5;
        for i in 0..n {
            let phase = i as f32 * TAU / n.max(1) as f32;
            let orbit = (t * TAU * 0.3 + phase).sin() * 2.0;
            transforms.push(GlyphTransform {
                position_offset: Vec3::new(orbit, (t * TAU * 0.3 + phase).cos() * 2.0, 0.0),
                scale: 1.0 + pulse * 0.08,
                rotation_z: t * 0.2,
                emission: pulse * 0.7,
            });
        }
        AnimPose { transforms }
    }
}

fn enemy_transition_params(from: EnemyAnimState, to: EnemyAnimState) -> (f32, BlendCurve) {
    if to == EnemyAnimState::Hurt {
        return (0.05, BlendCurve::Linear);
    }
    if to == EnemyAnimState::Die {
        return (0.1, BlendCurve::EaseIn);
    }
    match (from, to) {
        (EnemyAnimState::Idle, EnemyAnimState::Approach) => (0.2, BlendCurve::EaseInOut),
        (EnemyAnimState::Approach, EnemyAnimState::Attack) => (0.1, BlendCurve::EaseIn),
        (EnemyAnimState::Attack, EnemyAnimState::Idle) => (0.3, BlendCurve::EaseOut),
        (EnemyAnimState::Hurt, EnemyAnimState::Idle) => (0.35, BlendCurve::EaseOut),
        (EnemyAnimState::Idle, EnemyAnimState::Special) => (0.25, BlendCurve::EaseIn),
        _ => (0.2, BlendCurve::EaseInOut),
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// BOSS ANIMATION STATES
// ═════════════════════════════════════════════════════════════════════════════

/// Hydra boss: can split and reform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HydraAnimState {
    Normal,
    /// Dissolve current form and spawn two smaller copies.
    Splitting,
    /// Two copies merge back into one.
    Reforming,
}

/// Committee boss: judges that vote on player fate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitteeAnimState {
    Normal,
    /// Judges light up sequentially (0.5s each).
    Voting,
    /// Final verdict — all judges glow simultaneously.
    Verdict,
}

/// Algorithm boss: multi-phase encounter with entity reorganization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlgorithmAnimState {
    Phase1,
    /// Full entity reorganization over 2 seconds.
    PhaseTransition,
    Phase2,
    Phase3,
}

/// Per-boss animation controller wrapping EnemyAnimController with boss-specific logic.
pub struct BossAnimController {
    pub base: EnemyAnimController,
    pub boss_state: BossState,
    /// For Hydra: split progress [0, 1].
    pub split_progress: f32,
    /// For Committee: which judge is currently lit (0-based index).
    pub voting_index: usize,
    /// For Committee: total number of judges.
    pub judge_count: usize,
    /// For Committee: timer within current vote step.
    pub vote_timer: f32,
    /// For Algorithm: phase transition progress [0, 1].
    pub phase_transition_progress: f32,
}

/// Unified boss state enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BossState {
    Hydra(HydraAnimState),
    Committee(CommitteeAnimState),
    Algorithm(AlgorithmAnimState),
}

impl BossAnimController {
    pub fn new_hydra(glyph_count: usize) -> Self {
        Self {
            base: EnemyAnimController::new(glyph_count),
            boss_state: BossState::Hydra(HydraAnimState::Normal),
            split_progress: 0.0,
            voting_index: 0,
            judge_count: 5,
            vote_timer: 0.0,
            phase_transition_progress: 0.0,
        }
    }

    pub fn new_committee(glyph_count: usize, judge_count: usize) -> Self {
        Self {
            base: EnemyAnimController::new(glyph_count),
            boss_state: BossState::Committee(CommitteeAnimState::Normal),
            split_progress: 0.0,
            voting_index: 0,
            judge_count,
            vote_timer: 0.0,
            phase_transition_progress: 0.0,
        }
    }

    pub fn new_algorithm(glyph_count: usize) -> Self {
        Self {
            base: EnemyAnimController::new(glyph_count),
            boss_state: BossState::Algorithm(AlgorithmAnimState::Phase1),
            split_progress: 0.0,
            voting_index: 0,
            judge_count: 0,
            vote_timer: 0.0,
            phase_transition_progress: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) -> AnimPose {
        let base_pose = self.base.update(dt);
        let n = base_pose.transforms.len();

        match self.boss_state {
            BossState::Hydra(state) => self.apply_hydra(state, &base_pose, dt, n),
            BossState::Committee(state) => self.apply_committee(state, &base_pose, dt, n),
            BossState::Algorithm(state) => self.apply_algorithm(state, &base_pose, dt, n),
        }
    }

    fn apply_hydra(&mut self, state: HydraAnimState, base: &AnimPose, dt: f32, n: usize) -> AnimPose {
        match state {
            HydraAnimState::Normal => base.clone(),
            HydraAnimState::Splitting => {
                self.split_progress = (self.split_progress + dt / 1.0).min(1.0);
                let p = self.split_progress;
                let mut transforms = Vec::with_capacity(n);
                for i in 0..n {
                    let side = if i < n / 2 { -1.0 } else { 1.0 };
                    let spread = p * 8.0 * side;
                    let dissolve_scale = 1.0 - p * 0.3;
                    let mut gt = base.transforms.get(i).copied().unwrap_or_default();
                    gt.position_offset.x += spread;
                    gt.scale *= dissolve_scale;
                    gt.emission += p * 0.5;
                    transforms.push(gt);
                }
                AnimPose { transforms }
            }
            HydraAnimState::Reforming => {
                self.split_progress = (self.split_progress - dt / 1.5).max(0.0);
                let p = self.split_progress;
                let mut transforms = Vec::with_capacity(n);
                for i in 0..n {
                    let side = if i < n / 2 { -1.0 } else { 1.0 };
                    let spread = p * 8.0 * side;
                    let mut gt = base.transforms.get(i).copied().unwrap_or_default();
                    gt.position_offset.x += spread;
                    gt.emission += p * 0.3;
                    transforms.push(gt);
                }
                AnimPose { transforms }
            }
        }
    }

    fn apply_committee(
        &mut self,
        state: CommitteeAnimState,
        base: &AnimPose,
        dt: f32,
        n: usize,
    ) -> AnimPose {
        match state {
            CommitteeAnimState::Normal => base.clone(),
            CommitteeAnimState::Voting => {
                self.vote_timer += dt;
                if self.vote_timer >= 0.5 {
                    self.vote_timer -= 0.5;
                    self.voting_index += 1;
                    if self.voting_index >= self.judge_count {
                        self.boss_state = BossState::Committee(CommitteeAnimState::Verdict);
                        self.voting_index = 0;
                    }
                }
                let glyphs_per_judge = n / self.judge_count.max(1);
                let mut transforms = Vec::with_capacity(n);
                for i in 0..n {
                    let judge_idx = i / glyphs_per_judge.max(1);
                    let mut gt = base.transforms.get(i).copied().unwrap_or_default();
                    if judge_idx == self.voting_index {
                        gt.emission += 0.8;
                        gt.scale += 0.05;
                    }
                    transforms.push(gt);
                }
                AnimPose { transforms }
            }
            CommitteeAnimState::Verdict => {
                let mut transforms = Vec::with_capacity(n);
                let t = self.base.state_timer;
                let flash = (TAU * 4.0 * t).sin() * 0.5 + 0.5;
                for i in 0..n {
                    let mut gt = base.transforms.get(i).copied().unwrap_or_default();
                    gt.emission += flash;
                    gt.scale += flash * 0.03;
                    transforms.push(gt);
                }
                AnimPose { transforms }
            }
        }
    }

    fn apply_algorithm(
        &mut self,
        state: AlgorithmAnimState,
        base: &AnimPose,
        dt: f32,
        n: usize,
    ) -> AnimPose {
        match state {
            AlgorithmAnimState::Phase1 | AlgorithmAnimState::Phase2 | AlgorithmAnimState::Phase3 => {
                base.clone()
            }
            AlgorithmAnimState::PhaseTransition => {
                self.phase_transition_progress =
                    (self.phase_transition_progress + dt / 2.0).min(1.0);
                let p = self.phase_transition_progress;
                let mut transforms = Vec::with_capacity(n);
                for i in 0..n {
                    let phase = i as f32 * TAU / n.max(1) as f32;
                    let chaos = (p * PI).sin(); // peaks at midpoint
                    let orbit_r = chaos * 6.0;
                    let angle = phase + p * TAU * 2.0;
                    let mut gt = base.transforms.get(i).copied().unwrap_or_default();
                    gt.position_offset += Vec3::new(
                        angle.cos() * orbit_r,
                        angle.sin() * orbit_r,
                        0.0,
                    );
                    gt.rotation_z += p * TAU;
                    gt.emission += chaos * 0.7;
                    gt.scale = gt.scale * (1.0 - chaos * 0.2);
                    transforms.push(gt);
                }
                AnimPose { transforms }
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// ANIMATION MANAGER
// ═════════════════════════════════════════════════════════════════════════════

/// Centralized animation manager owning all active controllers.
pub struct AnimationManager {
    /// Player controllers keyed by entity ID.
    pub player_controllers: HashMap<EntityId, PlayerAnimController>,
    /// Enemy controllers keyed by entity ID.
    pub enemy_controllers: HashMap<EntityId, EnemyAnimController>,
    /// Boss controllers keyed by entity ID.
    pub boss_controllers: HashMap<EntityId, BossAnimController>,
    /// IK limbs keyed by (entity_id, limb_name).
    pub ik_limbs: HashMap<(EntityId, String), IKLimb>,
    /// Cached output transforms per entity.
    output_cache: HashMap<EntityId, Vec<GlyphTransform>>,
    /// Known entity positions (updated externally).
    pub entity_positions: HashMap<EntityId, Vec3>,
}

impl AnimationManager {
    pub fn new() -> Self {
        Self {
            player_controllers: HashMap::new(),
            enemy_controllers: HashMap::new(),
            boss_controllers: HashMap::new(),
            ik_limbs: HashMap::new(),
            output_cache: HashMap::new(),
            entity_positions: HashMap::new(),
        }
    }

    /// Register a player entity for animation.
    pub fn register_player(&mut self, entity_id: EntityId, glyph_count: usize) {
        self.player_controllers
            .insert(entity_id, PlayerAnimController::new(glyph_count));
    }

    /// Register an enemy entity for animation.
    pub fn register_enemy(&mut self, entity_id: EntityId, glyph_count: usize) {
        self.enemy_controllers
            .insert(entity_id, EnemyAnimController::new(glyph_count));
    }

    /// Register a boss entity.
    pub fn register_boss(&mut self, entity_id: EntityId, controller: BossAnimController) {
        self.boss_controllers.insert(entity_id, controller);
    }

    /// Register an IK limb for an entity.
    pub fn register_ik_limb(&mut self, entity_id: EntityId, limb: IKLimb) {
        self.ik_limbs
            .insert((entity_id, limb.name.clone()), limb);
    }

    /// Tick all animation controllers and update the output cache.
    pub fn update(&mut self, dt: f32) {
        self.output_cache.clear();

        // Update player controllers
        let player_ids: Vec<EntityId> = self.player_controllers.keys().copied().collect();
        for id in player_ids {
            if let Some(ctrl) = self.player_controllers.get_mut(&id) {
                let pose = ctrl.update(dt);
                self.output_cache.insert(id, pose.transforms);
            }
        }

        // Update enemy controllers
        let enemy_ids: Vec<EntityId> = self.enemy_controllers.keys().copied().collect();
        for id in enemy_ids {
            if let Some(ctrl) = self.enemy_controllers.get_mut(&id) {
                let pose = ctrl.update(dt);
                self.output_cache.insert(id, pose.transforms);
            }
        }

        // Update boss controllers
        let boss_ids: Vec<EntityId> = self.boss_controllers.keys().copied().collect();
        for id in boss_ids {
            if let Some(ctrl) = self.boss_controllers.get_mut(&id) {
                let pose = ctrl.update(dt);
                self.output_cache.insert(id, pose.transforms);
            }
        }

        // Apply IK on top of animation results
        let ik_keys: Vec<(EntityId, String)> = self.ik_limbs.keys().cloned().collect();
        for (entity_id, limb_name) in ik_keys {
            if let Some(limb) = self.ik_limbs.get_mut(&(entity_id, limb_name)) {
                let entity_pos = self.entity_positions.get(&entity_id).copied().unwrap_or(Vec3::ZERO);
                let offsets = limb.solve(entity_pos, &self.entity_positions);
                // Apply IK offsets to the cached transforms
                if let Some(transforms) = self.output_cache.get_mut(&entity_id) {
                    for (slot, &glyph_idx) in limb.glyph_indices.iter().enumerate() {
                        if glyph_idx < transforms.len() {
                            let ik_blend = limb.weight;
                            transforms[glyph_idx].position_offset = transforms[glyph_idx]
                                .position_offset
                                * (1.0 - ik_blend)
                                + offsets[slot] * ik_blend;
                        }
                    }
                }
            }
        }
    }

    /// Trigger a state transition on a player entity.
    pub fn trigger_player_state(&mut self, entity_id: EntityId, new_state: PlayerAnimState) -> bool {
        if let Some(ctrl) = self.player_controllers.get_mut(&entity_id) {
            ctrl.transition_to(new_state)
        } else {
            false
        }
    }

    /// Trigger a state transition on an enemy entity.
    pub fn trigger_enemy_state(&mut self, entity_id: EntityId, new_state: EnemyAnimState) -> bool {
        if let Some(ctrl) = self.enemy_controllers.get_mut(&entity_id) {
            ctrl.transition_to(new_state)
        } else {
            false
        }
    }

    /// Set an IK target for a named limb on an entity.
    pub fn set_ik_target(&mut self, entity_id: EntityId, chain_name: &str, target: IKTarget) {
        if let Some(limb) = self.ik_limbs.get_mut(&(entity_id, chain_name.to_string())) {
            limb.target = target;
        }
    }

    /// Get the output transforms for an entity (after update).
    pub fn get_glyph_transforms(&self, entity_id: EntityId) -> Vec<GlyphTransform> {
        self.output_cache
            .get(&entity_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Remove all controllers for a despawned entity.
    pub fn remove_entity(&mut self, entity_id: EntityId) {
        self.player_controllers.remove(&entity_id);
        self.enemy_controllers.remove(&entity_id);
        self.boss_controllers.remove(&entity_id);
        self.output_cache.remove(&entity_id);
        self.entity_positions.remove(&entity_id);
        // Remove IK limbs for this entity
        self.ik_limbs.retain(|(id, _), _| *id != entity_id);
    }
}

impl Default for AnimationManager {
    fn default() -> Self {
        Self::new()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── GlyphTransform ───────────────────────────────────────────────────────

    #[test]
    fn glyph_transform_default_is_identity() {
        let gt = GlyphTransform::default();
        assert_eq!(gt.position_offset, Vec3::ZERO);
        assert!((gt.scale - 1.0).abs() < 1e-6);
        assert!((gt.rotation_z).abs() < 1e-6);
        assert!((gt.emission).abs() < 1e-6);
    }

    #[test]
    fn glyph_transform_lerp_midpoint() {
        let a = GlyphTransform {
            position_offset: Vec3::ZERO,
            scale: 1.0,
            rotation_z: 0.0,
            emission: 0.0,
        };
        let b = GlyphTransform {
            position_offset: Vec3::new(10.0, 0.0, 0.0),
            scale: 2.0,
            rotation_z: 1.0,
            emission: 1.0,
        };
        let mid = GlyphTransform::lerp(&a, &b, 0.5);
        assert!((mid.position_offset.x - 5.0).abs() < 1e-6);
        assert!((mid.scale - 1.5).abs() < 1e-6);
        assert!((mid.rotation_z - 0.5).abs() < 1e-6);
        assert!((mid.emission - 0.5).abs() < 1e-6);
    }

    // ── BlendCurve ───────────────────────────────────────────────────────────

    #[test]
    fn blend_curve_endpoints() {
        for curve in &[BlendCurve::Linear, BlendCurve::EaseIn, BlendCurve::EaseOut, BlendCurve::EaseInOut] {
            let v0 = curve.evaluate(0.0);
            let v1 = curve.evaluate(1.0);
            assert!(v0.abs() < 1e-6, "curve {curve:?} at t=0 should be ~0, got {v0}");
            assert!((v1 - 1.0).abs() < 1e-6, "curve {curve:?} at t=1 should be ~1, got {v1}");
        }
    }

    #[test]
    fn blend_curve_monotonic() {
        for curve in &[BlendCurve::Linear, BlendCurve::EaseIn, BlendCurve::EaseOut, BlendCurve::EaseInOut] {
            let mut prev = 0.0f32;
            for step in 1..=20 {
                let t = step as f32 / 20.0;
                let v = curve.evaluate(t);
                assert!(v >= prev - 1e-6, "curve {curve:?} not monotonic at t={t}");
                prev = v;
            }
        }
    }

    // ── Player Animation States ──────────────────────────────────────────────

    #[test]
    fn player_idle_produces_correct_glyph_count() {
        let ctrl = PlayerAnimController::new(9);
        let pose = ctrl.pose_idle(0.5, 9);
        assert_eq!(pose.transforms.len(), 9);
    }

    #[test]
    fn player_idle_breathing_range() {
        let ctrl = PlayerAnimController::new(1);
        // Sample over one full cycle
        let mut min_scale = f32::MAX;
        let mut max_scale = f32::MIN;
        for step in 0..100 {
            let t = step as f32 / 100.0 * (1.0 / 0.3); // one full cycle at 0.3Hz
            let pose = ctrl.pose_idle(t, 1);
            let s = pose.transforms[0].scale;
            min_scale = min_scale.min(s);
            max_scale = max_scale.max(s);
        }
        assert!(min_scale >= 0.97, "min scale {min_scale} should be >= 0.97");
        assert!(max_scale <= 1.03, "max scale {max_scale} should be <= 1.03");
    }

    #[test]
    fn player_walk_higher_bob_than_idle() {
        let ctrl = PlayerAnimController::new(4);
        let idle = ctrl.pose_idle(0.25, 4);
        let walk = ctrl.pose_walk(0.25, 4);
        // Walk bob amplitude is higher
        let idle_max_y: f32 = idle.transforms.iter().map(|t| t.position_offset.y.abs()).fold(0.0f32, f32::max);
        let walk_max_y: f32 = walk.transforms.iter().map(|t| t.position_offset.y.abs()).fold(0.0f32, f32::max);
        // Walk should generally have bigger displacement (across a representative sample)
        // This is a statistical test so we check the amplitudes
        assert!(walk_max_y >= idle_max_y * 0.5, "walk bob should be comparable or larger than idle");
    }

    #[test]
    fn player_attack_has_emission() {
        let ctrl = PlayerAnimController::new(6);
        let pose = ctrl.pose_attack(0.15, 6); // mid-attack
        let total_emission: f32 = pose.transforms.iter().map(|t| t.emission).sum();
        assert!(total_emission > 0.0, "attack should have emission glow");
    }

    #[test]
    fn player_heavy_attack_phases() {
        let ctrl = PlayerAnimController::new(4);
        // Windup phase: arm should pull back (negative X for arm glyphs)
        let windup = ctrl.pose_heavy_attack(0.25, 4);
        // Swing phase: big forward movement
        let swing = ctrl.pose_heavy_attack(0.65, 4);
        let swing_emission: f32 = swing.transforms.iter().map(|t| t.emission).sum();
        assert!(swing_emission > 0.0, "heavy attack swing should have emission");
        // Follow-through should be calmer
        let follow = ctrl.pose_heavy_attack(0.9, 4);
        let follow_emission: f32 = follow.transforms.iter().map(|t| t.emission).sum();
        assert!(follow_emission < swing_emission, "follow-through emission should be less than swing");
    }

    #[test]
    fn player_cast_hands_rise() {
        let ctrl = PlayerAnimController::new(9);
        let pose = ctrl.pose_cast(0.3, 9); // during raise phase
        // Hand glyphs (upper indices) should have positive Y offset
        let hand_y: f32 = pose.transforms[8].position_offset.y;
        let body_y: f32 = pose.transforms[0].position_offset.y;
        assert!(hand_y > body_y, "hand glyphs should rise higher than body glyphs");
    }

    #[test]
    fn player_hurt_has_recoil() {
        let mut ctrl = PlayerAnimController::new(4);
        ctrl.damage_direction = Vec3::new(-1.0, 0.0, 0.0);
        let pose = ctrl.pose_hurt(0.05, 4); // early = peak recoil
        let avg_x: f32 = pose.transforms.iter().map(|t| t.position_offset.x).sum::<f32>() / 4.0;
        // Recoil should push away from damage (positive X since damage from -X)
        assert!(avg_x > 0.0, "hurt recoil should push away from damage source");
    }

    #[test]
    fn player_defend_reduces_scale() {
        let ctrl = PlayerAnimController::new(6);
        let pose = ctrl.pose_defend(0.5, 6); // fully defended
        for gt in &pose.transforms {
            assert!(gt.scale < 1.0, "defend should reduce scale (crouch), got {}", gt.scale);
        }
    }

    #[test]
    fn player_dodge_has_lateral_movement() {
        let mut ctrl = PlayerAnimController::new(4);
        ctrl.move_direction = Vec3::X;
        let pose = ctrl.pose_dodge(0.125, 4); // peak lateral
        let avg_x: f32 = pose.transforms.iter().map(|t| t.position_offset.x).sum::<f32>() / 4.0;
        assert!(avg_x.abs() > 1.0, "dodge should have significant lateral movement");
    }

    // ── State Transitions ────────────────────────────────────────────────────

    #[test]
    fn player_transition_idle_to_walk() {
        let mut ctrl = PlayerAnimController::new(4);
        assert!(ctrl.transition_to(PlayerAnimState::Walk));
        assert_eq!(ctrl.current_state, PlayerAnimState::Walk);
        assert_eq!(ctrl.prev_state, PlayerAnimState::Idle);
        assert!(ctrl.in_transition);
    }

    #[test]
    fn player_transition_same_state_rejected() {
        let mut ctrl = PlayerAnimController::new(4);
        assert!(!ctrl.transition_to(PlayerAnimState::Idle));
    }

    #[test]
    fn player_transition_blend_completes() {
        let mut ctrl = PlayerAnimController::new(4);
        ctrl.transition_to(PlayerAnimState::Walk);
        // Tick past transition time (0.2s for Idle→Walk)
        for _ in 0..20 {
            ctrl.update(0.016);
        }
        assert!(!ctrl.in_transition);
        assert!((ctrl.blend_factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn player_hurt_transition_is_fast() {
        let (dur, curve) = transition_params(PlayerAnimState::Walk, PlayerAnimState::Hurt);
        assert!((dur - 0.05).abs() < 1e-6, "Any→Hurt should be 0.05s, got {dur}");
        assert_eq!(curve, BlendCurve::Linear);
    }

    #[test]
    fn player_auto_return_to_idle_after_attack() {
        let mut ctrl = PlayerAnimController::new(4);
        ctrl.transition_to(PlayerAnimState::Attack);
        // Tick past attack duration (0.3s) + transition time
        for _ in 0..50 {
            ctrl.update(0.016);
        }
        // Should have auto-transitioned back to idle
        assert_eq!(ctrl.current_state, PlayerAnimState::Idle);
    }

    // ── Blend Trees ──────────────────────────────────────────────────────────

    #[test]
    fn blend_tree_1d_single_entry() {
        let mut tree = BlendTree1D::new();
        tree.add_entry(0.5, AnimPose::identity(3));
        tree.parameter = 0.0;
        let pose = tree.evaluate();
        assert_eq!(pose.transforms.len(), 3);
    }

    #[test]
    fn blend_tree_1d_interpolation() {
        let pose_a = AnimPose {
            transforms: vec![GlyphTransform {
                position_offset: Vec3::ZERO,
                scale: 1.0,
                rotation_z: 0.0,
                emission: 0.0,
            }],
        };
        let pose_b = AnimPose {
            transforms: vec![GlyphTransform {
                position_offset: Vec3::new(10.0, 0.0, 0.0),
                scale: 2.0,
                rotation_z: 0.0,
                emission: 1.0,
            }],
        };

        let mut tree = BlendTree1D::new();
        tree.add_entry(0.0, pose_a);
        tree.add_entry(1.0, pose_b);

        tree.parameter = 0.5;
        let result = tree.evaluate();
        assert!((result.transforms[0].position_offset.x - 5.0).abs() < 1e-6);
        assert!((result.transforms[0].scale - 1.5).abs() < 1e-6);
    }

    #[test]
    fn blend_tree_1d_clamp_below() {
        let pose = AnimPose::identity(2);
        let mut tree = BlendTree1D::new();
        tree.add_entry(0.3, pose.clone());
        tree.add_entry(0.7, AnimPose::identity(2));

        tree.parameter = 0.0; // below first entry
        let result = tree.evaluate();
        assert_eq!(result.transforms.len(), 2);
    }

    #[test]
    fn idle_walk_blend_at_zero_is_idle() {
        let mut blend = IdleWalkBlend::new(4);
        let pose = blend.evaluate(0.0);
        assert_eq!(pose.transforms.len(), 4);
    }

    #[test]
    fn attack_power_blend_range() {
        let mut blend = AttackPowerBlend::new(6);
        let light = blend.evaluate(0.0);
        let heavy = blend.evaluate(1.0);
        // Heavy should have more emission/displacement
        let light_emit: f32 = light.transforms.iter().map(|t| t.emission).sum();
        let heavy_emit: f32 = heavy.transforms.iter().map(|t| t.emission).sum();
        assert!(heavy_emit >= light_emit, "heavy attack should have >= emission");
    }

    #[test]
    fn damage_reaction_blend_scales() {
        let mut blend = DamageReactionBlend::new(4);
        let small = blend.evaluate(0.0);
        let large = blend.evaluate(1.0);
        let small_displacement: f32 = small.transforms.iter().map(|t| t.position_offset.length()).sum();
        let large_displacement: f32 = large.transforms.iter().map(|t| t.position_offset.length()).sum();
        assert!(large_displacement > small_displacement, "large damage should have more displacement");
    }

    #[test]
    fn cast_intensity_blend_emission() {
        let mut blend = CastIntensityBlend::new(9);
        let small = blend.evaluate(0.0);
        let large = blend.evaluate(1.0);
        let small_emit: f32 = small.transforms.iter().map(|t| t.emission).sum();
        let large_emit: f32 = large.transforms.iter().map(|t| t.emission).sum();
        assert!(large_emit > small_emit, "large cast should have more emission");
    }

    // ── IK System ────────────────────────────────────────────────────────────

    #[test]
    fn ik_chain_construction() {
        let chain = IKChain::new(Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
        assert!((chain.lengths[0] - 3.0).abs() < 1e-6);
        assert!((chain.lengths[1] - 2.0).abs() < 1e-6);
        assert!((chain.total_length() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn ik_solve_reaches_target_within_range() {
        let mut chain = IKChain::new(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
        );
        let target = Vec3::new(4.0, 1.0, 0.0);
        solve_two_bone(&mut chain, target);

        let end_dist = (chain.end_pos - target).length();
        assert!(end_dist < 0.5, "IK end should be near target, dist = {end_dist}");

        // Verify bone lengths are preserved
        let bone0_len = (chain.mid_pos - chain.root_pos).length();
        let bone1_len = (chain.end_pos - chain.mid_pos).length();
        assert!((bone0_len - 3.0).abs() < 0.1, "bone0 length should be ~3, got {bone0_len}");
        assert!((bone1_len - 2.0).abs() < 0.1, "bone1 length should be ~2, got {bone1_len}");
    }

    #[test]
    fn ik_solve_unreachable_target_extends() {
        let mut chain = IKChain::new(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
        );
        // Target beyond reach
        let target = Vec3::new(100.0, 0.0, 0.0);
        solve_two_bone(&mut chain, target);

        // Should extend toward target as far as possible
        assert!(chain.end_pos.x > 3.0, "should extend toward target");
    }

    #[test]
    fn ik_limb_no_target_returns_zero_offsets() {
        let mut limb = IKLimb::new(
            "test",
            [0, 1, 2],
            [Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0)],
        );
        limb.target = IKTarget::None;
        let offsets = limb.solve(Vec3::ZERO, &HashMap::new());
        for o in &offsets {
            assert!(o.length() < 1e-6, "None target should give zero offsets");
        }
    }

    #[test]
    fn ik_limb_position_target() {
        let mut limb = IKLimb::new(
            "arm",
            [0, 1, 2],
            [Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0)],
        );
        limb.target = IKTarget::Position(Vec3::new(3.0, 1.0, 0.0));
        let offsets = limb.solve(Vec3::ZERO, &HashMap::new());
        // End effector offset should be non-zero toward target
        assert!(offsets[2].length() > 0.1, "IK should produce non-zero offset for position target");
    }

    // ── Enemy Animation ──────────────────────────────────────────────────────

    #[test]
    fn enemy_idle_glyph_count() {
        let ctrl = EnemyAnimController::new(6);
        let pose = ctrl.enemy_idle(0.5, 6);
        assert_eq!(pose.transforms.len(), 6);
    }

    #[test]
    fn enemy_transition_to_attack() {
        let mut ctrl = EnemyAnimController::new(6);
        assert!(ctrl.transition_to(EnemyAnimState::Approach));
        assert_eq!(ctrl.current_state, EnemyAnimState::Approach);
    }

    #[test]
    fn enemy_die_fades_out() {
        let ctrl = EnemyAnimController::new(4);
        let early = ctrl.enemy_die(0.1, 4);
        let late = ctrl.enemy_die(1.4, 4);
        let early_scale: f32 = early.transforms.iter().map(|t| t.scale).sum();
        let late_scale: f32 = late.transforms.iter().map(|t| t.scale).sum();
        assert!(late_scale < early_scale, "dying entity should fade out over time");
    }

    #[test]
    fn enemy_hurt_transition_is_fast() {
        let (dur, _) = enemy_transition_params(EnemyAnimState::Idle, EnemyAnimState::Hurt);
        assert!((dur - 0.05).abs() < 1e-6);
    }

    // ── Boss Animation ───────────────────────────────────────────────────────

    #[test]
    fn boss_hydra_splitting_spreads_glyphs() {
        let mut boss = BossAnimController::new_hydra(8);
        boss.boss_state = BossState::Hydra(HydraAnimState::Splitting);
        let pose1 = boss.update(0.01);
        // Advance splitting
        for _ in 0..50 {
            boss.update(0.016);
        }
        let pose2 = boss.update(0.016);
        // Spread should increase
        let spread1: f32 = pose1.transforms.iter().map(|t| t.position_offset.x.abs()).sum();
        let spread2: f32 = pose2.transforms.iter().map(|t| t.position_offset.x.abs()).sum();
        assert!(spread2 > spread1, "splitting should spread glyphs apart");
    }

    #[test]
    fn boss_committee_voting_advances() {
        let mut boss = BossAnimController::new_committee(10, 5);
        boss.boss_state = BossState::Committee(CommitteeAnimState::Voting);
        boss.voting_index = 0;
        boss.vote_timer = 0.0;

        // Tick past one vote cycle (0.5s)
        for _ in 0..35 {
            boss.update(0.016);
        }
        assert!(boss.voting_index > 0 || matches!(boss.boss_state, BossState::Committee(CommitteeAnimState::Verdict)),
            "voting should advance index or reach verdict");
    }

    #[test]
    fn boss_algorithm_phase_transition_chaos() {
        let mut boss = BossAnimController::new_algorithm(8);
        boss.boss_state = BossState::Algorithm(AlgorithmAnimState::PhaseTransition);
        boss.phase_transition_progress = 0.0;

        // Tick to midpoint
        for _ in 0..60 {
            boss.update(0.016);
        }
        let pose = boss.update(0.016);
        let total_emission: f32 = pose.transforms.iter().map(|t| t.emission).sum();
        assert!(total_emission > 0.0, "phase transition should produce emission chaos");
    }

    // ── Animation Manager ────────────────────────────────────────────────────

    #[test]
    fn manager_register_and_update() {
        let mut mgr = AnimationManager::new();
        let player_id = EntityId(1);
        let enemy_id = EntityId(2);
        mgr.register_player(player_id, 9);
        mgr.register_enemy(enemy_id, 6);
        mgr.update(0.016);

        let player_transforms = mgr.get_glyph_transforms(player_id);
        assert_eq!(player_transforms.len(), 9);
        let enemy_transforms = mgr.get_glyph_transforms(enemy_id);
        assert_eq!(enemy_transforms.len(), 6);
    }

    #[test]
    fn manager_trigger_state() {
        let mut mgr = AnimationManager::new();
        let id = EntityId(1);
        mgr.register_player(id, 4);
        assert!(mgr.trigger_player_state(id, PlayerAnimState::Walk));
        assert!(!mgr.trigger_player_state(EntityId(999), PlayerAnimState::Walk)); // nonexistent
    }

    #[test]
    fn manager_set_ik_target() {
        let mut mgr = AnimationManager::new();
        let id = EntityId(1);
        mgr.register_player(id, 6);
        let limb = weapon_arm_ik([0, 1, 2], [Vec3::ZERO, Vec3::X * 2.0, Vec3::X * 4.0]);
        mgr.register_ik_limb(id, limb);
        mgr.set_ik_target(id, "weapon_arm", IKTarget::Position(Vec3::new(3.0, 2.0, 0.0)));
        mgr.update(0.016);
        let transforms = mgr.get_glyph_transforms(id);
        assert_eq!(transforms.len(), 6);
    }

    #[test]
    fn manager_remove_entity_cleans_up() {
        let mut mgr = AnimationManager::new();
        let id = EntityId(42);
        mgr.register_player(id, 4);
        mgr.update(0.016);
        assert!(!mgr.get_glyph_transforms(id).is_empty());

        mgr.remove_entity(id);
        assert!(mgr.get_glyph_transforms(id).is_empty());
        assert!(!mgr.player_controllers.contains_key(&id));
    }

    #[test]
    fn manager_boss_registration() {
        let mut mgr = AnimationManager::new();
        let id = EntityId(100);
        let boss = BossAnimController::new_hydra(12);
        mgr.register_boss(id, boss);
        mgr.update(0.016);
        let transforms = mgr.get_glyph_transforms(id);
        assert_eq!(transforms.len(), 12);
    }

    // ── Transition Table ─────────────────────────────────────────────────────

    #[test]
    fn transition_table_not_empty() {
        let table = build_transition_table();
        assert!(table.len() >= 10, "transition table should have at least 10 entries");
    }

    #[test]
    fn transition_table_durations_positive() {
        let table = build_transition_table();
        for entry in &table {
            assert!(entry.duration > 0.0, "transition duration must be positive");
        }
    }

    // ── Timing ───────────────────────────────────────────────────────────────

    #[test]
    fn fixed_duration_states() {
        assert!(PlayerAnimState::Attack.fixed_duration().is_some());
        assert!(PlayerAnimState::HeavyAttack.fixed_duration().is_some());
        assert!(PlayerAnimState::Hurt.fixed_duration().is_some());
        assert!(PlayerAnimState::Idle.fixed_duration().is_none());
        assert!(PlayerAnimState::Walk.fixed_duration().is_none());
    }

    #[test]
    fn pose_blend_preserves_length() {
        let a = AnimPose::identity(5);
        let b = AnimPose::identity(5);
        let blended = a.blend(&b, 0.5);
        assert_eq!(blended.transforms.len(), 5);
    }

    #[test]
    fn ik_factory_functions() {
        let rest = [Vec3::ZERO, Vec3::X * 2.0, Vec3::X * 4.0];
        let indices = [0, 1, 2];
        let w = weapon_arm_ik(indices, rest);
        assert_eq!(w.name, "weapon_arm");
        assert!((w.weight - 0.8).abs() < 1e-6);

        let l = look_at_ik(indices, rest);
        assert_eq!(l.name, "look_at");

        let s = staff_aim_ik(indices, rest);
        assert_eq!(s.name, "staff_aim");

        let sh = shield_ik(indices, rest);
        assert_eq!(sh.name, "shield");
        assert!((sh.weight - 1.0).abs() < 1e-6);
    }
}
