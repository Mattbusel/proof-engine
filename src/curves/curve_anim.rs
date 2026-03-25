//! Animation state machine for curve entities.

use glam::Vec2;
use super::entity_curves::CurveEntity;

/// Animation states a curve entity can be in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurveAnimState {
    Idle,
    Cast { duration: f32 },
    Hit { direction: Vec2, magnitude: f32 },
    Defend,
    LowHP,
    Channel { duration: f32 },
    Dissolving,
}

/// Manages animation state transitions with blending.
#[derive(Debug, Clone)]
pub struct CurveAnimator {
    pub current: CurveAnimState,
    pub previous: CurveAnimState,
    pub blend: f32,
    pub blend_speed: f32,
    pub state_time: f32,
}

impl CurveAnimator {
    pub fn new() -> Self {
        Self { current: CurveAnimState::Idle, previous: CurveAnimState::Idle, blend: 1.0, blend_speed: 5.0, state_time: 0.0 }
    }

    /// Transition to a new state.
    pub fn transition(&mut self, new_state: CurveAnimState) {
        if self.current != new_state {
            self.previous = self.current;
            self.current = new_state;
            self.blend = 0.0;
            self.state_time = 0.0;
        }
    }

    /// Update the animator and apply to entity.
    pub fn update(&mut self, entity: &mut CurveEntity, dt: f32) {
        self.state_time += dt;
        self.blend = (self.blend + self.blend_speed * dt).min(1.0);

        // Auto-transitions
        match self.current {
            CurveAnimState::Cast { duration } => {
                if self.state_time > duration { self.transition(CurveAnimState::Idle); }
            }
            CurveAnimState::Hit { direction, magnitude } => {
                // Apply recoil at start
                if self.state_time < dt * 2.0 {
                    entity.apply_hit(direction, magnitude);
                }
                if self.state_time > 0.6 { self.transition(CurveAnimState::Idle); }
            }
            CurveAnimState::Channel { duration } => {
                // Add tangential velocity while channeling
                for curve in &mut entity.curves {
                    for (i, vel) in curve.point_velocities.iter_mut().enumerate() {
                        let pt = curve.control_points[i];
                        let tangent = Vec2::new(-pt.y, pt.x).normalize_or_zero();
                        *vel += tangent * 0.5 * dt;
                    }
                }
                if self.state_time > duration { self.transition(CurveAnimState::Idle); }
            }
            CurveAnimState::Defend => {
                // Stiffness is handled externally via brace/unbrace
            }
            CurveAnimState::LowHP => {
                // Entity.tick() already adds noise based on hp_ratio
            }
            CurveAnimState::Dissolving => {
                // Entity.tick() handles dissolution
            }
            CurveAnimState::Idle => {}
        }

        // Breathing intensity varies by state
        entity.breath_amplitude = match self.current {
            CurveAnimState::Idle => 0.03,
            CurveAnimState::Cast { .. } => 0.06,
            CurveAnimState::Hit { .. } => 0.01,
            CurveAnimState::Defend => 0.01,
            CurveAnimState::LowHP => 0.05,
            CurveAnimState::Channel { .. } => 0.04,
            CurveAnimState::Dissolving => 0.0,
        };
    }

    pub fn is_idle(&self) -> bool { self.current == CurveAnimState::Idle }
    pub fn is_blending(&self) -> bool { self.blend < 1.0 }
}
