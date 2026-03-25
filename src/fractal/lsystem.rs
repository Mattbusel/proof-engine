//! L-systems with parametric rules and 3D turtle graphics.

use glam::Vec3;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LSystemRule { pub from: char, pub to: String }

#[derive(Debug, Clone)]
pub struct LSystem {
    pub axiom: String,
    pub rules: Vec<LSystemRule>,
    pub angle: f32,
    pub step_length: f32,
}

#[derive(Debug, Clone)]
pub struct TurtleState { pub position: Vec3, pub heading: Vec3, pub up: Vec3, pub right: Vec3 }

impl Default for TurtleState {
    fn default() -> Self {
        Self { position: Vec3::ZERO, heading: Vec3::Y, up: Vec3::Z, right: Vec3::X }
    }
}

impl LSystem {
    pub fn new(axiom: &str, rules: Vec<(&str, &str)>, angle: f32, step: f32) -> Self {
        Self {
            axiom: axiom.to_string(),
            rules: rules.into_iter().map(|(f, t)| LSystemRule { from: f.chars().next().unwrap(), to: t.to_string() }).collect(),
            angle, step_length: step,
        }
    }

    /// Generate the string after n iterations.
    pub fn generate(&self, iterations: u32) -> String {
        let mut current = self.axiom.clone();
        let rule_map: HashMap<char, &str> = self.rules.iter().map(|r| (r.from, r.to.as_str())).collect();
        for _ in 0..iterations {
            let mut next = String::with_capacity(current.len() * 2);
            for ch in current.chars() {
                if let Some(&replacement) = rule_map.get(&ch) { next.push_str(replacement); }
                else { next.push(ch); }
            }
            current = next;
        }
        current
    }

    /// Interpret the generated string as turtle graphics. Returns line segments.
    pub fn interpret(&self, generated: &str) -> Vec<(Vec3, Vec3)> {
        let mut segments = Vec::new();
        let mut turtle = TurtleState::default();
        let mut stack: Vec<TurtleState> = Vec::new();
        let angle_rad = self.angle.to_radians();

        for ch in generated.chars() {
            match ch {
                'F' | 'G' => {
                    let start = turtle.position;
                    turtle.position += turtle.heading * self.step_length;
                    segments.push((start, turtle.position));
                }
                'f' | 'g' => { turtle.position += turtle.heading * self.step_length; }
                '+' => rotate_yaw(&mut turtle, angle_rad),
                '-' => rotate_yaw(&mut turtle, -angle_rad),
                '&' => rotate_pitch(&mut turtle, angle_rad),
                '^' => rotate_pitch(&mut turtle, -angle_rad),
                '\\' => rotate_roll(&mut turtle, angle_rad),
                '/' => rotate_roll(&mut turtle, -angle_rad),
                '|' => rotate_yaw(&mut turtle, std::f32::consts::PI),
                '[' => stack.push(turtle.clone()),
                ']' => { if let Some(s) = stack.pop() { turtle = s; } }
                _ => {}
            }
        }
        segments
    }

    // ── Presets ──────────────────────────────────────────────────────────

    pub fn koch_curve() -> Self { Self::new("F", vec![("F", "F+F-F-F+F")], 90.0, 1.0) }
    pub fn sierpinski_triangle() -> Self { Self::new("F-G-G", vec![("F", "F-G+F+G-F"), ("G", "GG")], 120.0, 1.0) }
    pub fn dragon_curve() -> Self { Self::new("FX", vec![("X", "X+YF+"), ("Y", "-FX-Y")], 90.0, 1.0) }
    pub fn plant() -> Self { Self::new("X", vec![("X", "F+[[X]-X]-F[-FX]+X"), ("F", "FF")], 25.0, 1.0) }
    pub fn hilbert_3d() -> Self { Self::new("A", vec![("A", "B-F+CFC+F-D&F^D-F+&&CFC+F+B//"), ("B", "A&F^CFB^F^D^^-F-D^|F^B|FC^F^A//"), ("C", "|D^|F^B-F+C^F^A&&FA&F^C+F+B^F^D//"), ("D", "|CFB-F+B|FA&F^A&&FB-F+B|FC//")], 90.0, 1.0) }
}

fn rotate_yaw(t: &mut TurtleState, angle: f32) {
    let (s, c) = angle.sin_cos();
    let new_h = t.heading * c + t.right * s;
    let new_r = -t.heading * s + t.right * c;
    t.heading = new_h.normalize_or_zero();
    t.right = new_r.normalize_or_zero();
}

fn rotate_pitch(t: &mut TurtleState, angle: f32) {
    let (s, c) = angle.sin_cos();
    let new_h = t.heading * c + t.up * s;
    let new_u = -t.heading * s + t.up * c;
    t.heading = new_h.normalize_or_zero();
    t.up = new_u.normalize_or_zero();
}

fn rotate_roll(t: &mut TurtleState, angle: f32) {
    let (s, c) = angle.sin_cos();
    let new_r = t.right * c + t.up * s;
    let new_u = -t.right * s + t.up * c;
    t.right = new_r.normalize_or_zero();
    t.up = new_u.normalize_or_zero();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn koch_generates() { let s = LSystem::koch_curve().generate(2); assert!(s.len() > 10); }
    #[test]
    fn plant_produces_segments() {
        let ls = LSystem::plant();
        let gen = ls.generate(3);
        let segs = ls.interpret(&gen);
        assert!(!segs.is_empty());
    }
    #[test]
    fn dragon_curve_grows() {
        let ls = LSystem::dragon_curve();
        let s1 = ls.generate(1); let s2 = ls.generate(2);
        assert!(s2.len() > s1.len());
    }
}
