//! Utility AI — scoring-based decision making.
//!
//! Each action has a set of considerations (response curves) that evaluate
//! world state and produce a [0, 1] score. The action with the highest
//! combined score is selected each tick.

use std::collections::HashMap;

// ── ResponseCurve ─────────────────────────────────────────────────────────────

/// Maps a raw input [0, 1] to a utility score [0, 1].
#[derive(Debug, Clone)]
pub enum ResponseCurve {
    /// Linear pass-through.
    Linear { slope: f32, intercept: f32 },
    /// Quadratic: ax² + b.
    Quadratic { a: f32, b: f32 },
    /// Logistic sigmoid.
    Logistic { k: f32, x0: f32 },
    /// Step threshold.
    Step { threshold: f32 },
    /// Exponential curve.
    Exponential { base: f32, k: f32 },
    /// Sine wave mapped to [0, 1].
    Sine { amplitude: f32, phase: f32 },
    /// Custom lookup table (piecewise linear).
    Table(Vec<(f32, f32)>),
}

impl ResponseCurve {
    pub fn evaluate(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            ResponseCurve::Linear { slope, intercept } => (slope * x + intercept).clamp(0.0, 1.0),
            ResponseCurve::Quadratic { a, b } => (a * x * x + b).clamp(0.0, 1.0),
            ResponseCurve::Logistic { k, x0 } => {
                1.0 / (1.0 + (-k * (x - x0)).exp())
            }
            ResponseCurve::Step { threshold } => if x >= *threshold { 1.0 } else { 0.0 },
            ResponseCurve::Exponential { base, k } => {
                (base.powf(k * x) - 1.0) / (base.powf(*k) - 1.0).max(1e-10)
            }
            ResponseCurve::Sine { amplitude, phase } => {
                let v = (x * std::f32::consts::PI + phase).sin() * amplitude;
                (v * 0.5 + 0.5).clamp(0.0, 1.0)
            }
            ResponseCurve::Table(pts) => {
                if pts.is_empty() { return 0.0; }
                let idx = pts.partition_point(|(px, _)| *px <= x);
                if idx == 0 { return pts[0].1; }
                if idx >= pts.len() { return pts.last().unwrap().1; }
                let (x0, y0) = pts[idx - 1];
                let (x1, y1) = pts[idx];
                if (x1 - x0).abs() < 1e-6 { return y1; }
                let t = (x - x0) / (x1 - x0);
                y0 + t * (y1 - y0)
            }
        }
    }
}

// ── Consideration ─────────────────────────────────────────────────────────────

/// A single input consideration for an action.
#[derive(Debug, Clone)]
pub struct Consideration {
    pub name:   String,
    /// How to evaluate this consideration's raw input.
    pub curve:  ResponseCurve,
    /// Weight in the final multiplication.
    pub weight: f32,
}

impl Consideration {
    pub fn new(name: &str, curve: ResponseCurve) -> Self {
        Self { name: name.to_string(), curve, weight: 1.0 }
    }

    pub fn evaluate(&self, raw_input: f32) -> f32 {
        self.curve.evaluate(raw_input) * self.weight
    }
}

// ── UtilityAction ─────────────────────────────────────────────────────────────

/// An action with a set of considerations.
pub struct UtilityAction<W> {
    pub name:           String,
    pub considerations: Vec<Consideration>,
    /// Minimum score threshold to select this action.
    pub min_threshold:  f32,
    /// Bonus score added when this action was selected last tick (momentum).
    pub momentum:       f32,
    /// Score normalisation: geometric mean vs product.
    pub use_geo_mean:   bool,
    /// The action to execute when selected.
    pub execute:        Box<dyn Fn(&mut W, f32) -> bool + Send + Sync>,
    /// Input provider: maps W → [0, 1] per consideration name.
    pub input_provider: Box<dyn Fn(&W) -> HashMap<String, f32> + Send + Sync>,
}

impl<W> UtilityAction<W> {
    pub fn new(
        name: &str,
        execute: impl Fn(&mut W, f32) -> bool + Send + Sync + 'static,
        input_provider: impl Fn(&W) -> HashMap<String, f32> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.to_string(),
            considerations: Vec::new(),
            min_threshold: 0.0,
            momentum: 0.0,
            use_geo_mean: true,
            execute: Box::new(execute),
            input_provider: Box::new(input_provider),
        }
    }

    pub fn with_consideration(mut self, c: Consideration) -> Self {
        self.considerations.push(c);
        self
    }

    /// Evaluate this action's combined utility score.
    pub fn score(&self, world: &W, is_current: bool) -> f32 {
        if self.considerations.is_empty() { return 0.5; }
        let inputs = (self.input_provider)(world);
        let n = self.considerations.len() as f32;

        let product: f32 = self.considerations.iter().map(|c| {
            let raw = inputs.get(&c.name).copied().unwrap_or(0.0);
            c.evaluate(raw)
        }).product();

        let score = if self.use_geo_mean && n > 1.0 {
            product.powf(1.0 / n)
        } else { product };

        let momentum_bonus = if is_current { self.momentum } else { 0.0 };
        (score + momentum_bonus).clamp(0.0, 1.0)
    }
}

// ── UtilitySelector ───────────────────────────────────────────────────────────

/// Selects the highest-scoring action each tick.
pub struct UtilitySelector<W> {
    pub name:        String,
    actions:         Vec<UtilityAction<W>>,
    current_action:  Option<usize>,
    pub last_scores: Vec<f32>,
}

impl<W> UtilitySelector<W> {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), actions: Vec::new(), current_action: None, last_scores: Vec::new() }
    }

    pub fn add_action(mut self, action: UtilityAction<W>) -> Self {
        self.actions.push(action);
        self
    }

    /// Evaluate all actions and execute the best.
    /// Returns true if an action was executed.
    pub fn tick(&mut self, world: &mut W, dt: f32) -> bool {
        if self.actions.is_empty() { return false; }

        // Score all actions
        self.last_scores = self.actions.iter().enumerate().map(|(i, a)| {
            a.score(world, self.current_action == Some(i))
        }).collect();

        // Find best above threshold
        let best = self.last_scores.iter().enumerate()
            .filter(|(i, &s)| s >= self.actions[*i].min_threshold)
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());

        if let Some((idx, _score)) = best {
            self.current_action = Some(idx);
            (self.actions[idx].execute)(world, dt)
        } else {
            self.current_action = None;
            false
        }
    }

    pub fn current_action_name(&self) -> Option<&str> {
        self.current_action.map(|i| self.actions[i].name.as_str())
    }

    pub fn action_count(&self) -> usize { self.actions.len() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_curve() {
        let c = ResponseCurve::Linear { slope: 1.0, intercept: 0.0 };
        assert!((c.evaluate(0.5) - 0.5).abs() < 0.001);
        assert!((c.evaluate(1.5) - 1.0).abs() < 0.001); // clamped
    }

    #[test]
    fn test_logistic_curve() {
        let c = ResponseCurve::Logistic { k: 10.0, x0: 0.5 };
        assert!(c.evaluate(0.5) > 0.45 && c.evaluate(0.5) < 0.55);
        assert!(c.evaluate(0.9) > 0.9);
        assert!(c.evaluate(0.1) < 0.1);
    }

    #[test]
    fn test_step_curve() {
        let c = ResponseCurve::Step { threshold: 0.5 };
        assert_eq!(c.evaluate(0.3), 0.0);
        assert_eq!(c.evaluate(0.7), 1.0);
    }

    #[test]
    fn test_table_curve() {
        let c = ResponseCurve::Table(vec![(0.0, 0.0), (0.5, 0.8), (1.0, 1.0)]);
        assert!((c.evaluate(0.25) - 0.4).abs() < 0.001); // midpoint between (0,0) and (0.5,0.8)
    }

    #[test]
    fn test_utility_selector_picks_highest() {
        struct World { health: f32, threat: f32, last_action: String }

        let flee = UtilityAction::new(
            "flee",
            |w: &mut World, _| { w.last_action = "flee".to_string(); true },
            |w: &World| { let mut m = HashMap::new(); m.insert("health".to_string(), 1.0 - w.health); m },
        ).with_consideration(Consideration::new("health", ResponseCurve::Linear { slope: 1.0, intercept: 0.0 }));

        let attack = UtilityAction::new(
            "attack",
            |w: &mut World, _| { w.last_action = "attack".to_string(); true },
            |w: &World| { let mut m = HashMap::new(); m.insert("threat".to_string(), w.threat); m },
        ).with_consideration(Consideration::new("threat", ResponseCurve::Linear { slope: 1.0, intercept: 0.0 }));

        let mut selector = UtilitySelector::new("combat")
            .add_action(flee)
            .add_action(attack);

        let mut world = World { health: 0.1, threat: 0.5, last_action: String::new() };
        selector.tick(&mut world, 0.016);
        assert_eq!(world.last_action, "flee", "low health should prefer fleeing");

        world.health = 0.9;
        world.threat = 0.9;
        selector.tick(&mut world, 0.016);
        assert_eq!(world.last_action, "attack", "high health + high threat → attack");
    }
}
