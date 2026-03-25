//! Constructive Solid Geometry — union, intersection, difference of mathematical volumes.

use glam::Vec3;
use super::implicit::ScalarField;

/// CSG operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsgOp {
    Union,
    Intersection,
    Difference,
    SmoothUnion { k: u32 },        // k/100 = smoothing radius
    SmoothIntersection { k: u32 },
    SmoothDifference { k: u32 },
}

/// A node in a CSG tree — either a leaf (scalar field) or a binary operation.
pub enum CsgNode {
    Leaf(Box<dyn ScalarField>),
    Binary {
        op: CsgOp,
        left: Box<CsgNode>,
        right: Box<CsgNode>,
    },
}

impl CsgNode {
    pub fn leaf(field: impl ScalarField + 'static) -> Self {
        Self::Leaf(Box::new(field))
    }

    pub fn union(a: CsgNode, b: CsgNode) -> Self {
        Self::Binary { op: CsgOp::Union, left: Box::new(a), right: Box::new(b) }
    }

    pub fn intersection(a: CsgNode, b: CsgNode) -> Self {
        Self::Binary { op: CsgOp::Intersection, left: Box::new(a), right: Box::new(b) }
    }

    pub fn difference(a: CsgNode, b: CsgNode) -> Self {
        Self::Binary { op: CsgOp::Difference, left: Box::new(a), right: Box::new(b) }
    }

    pub fn smooth_union(a: CsgNode, b: CsgNode, k: f32) -> Self {
        Self::Binary { op: CsgOp::SmoothUnion { k: (k * 100.0) as u32 }, left: Box::new(a), right: Box::new(b) }
    }

    /// Evaluate the CSG tree at a point.
    pub fn evaluate(&self, p: Vec3) -> f32 {
        match self {
            Self::Leaf(f) => f.evaluate(p),
            Self::Binary { op, left, right } => {
                let a = left.evaluate(p);
                let b = right.evaluate(p);
                match op {
                    CsgOp::Union => a.min(b),
                    CsgOp::Intersection => a.max(b),
                    CsgOp::Difference => a.max(-b),
                    CsgOp::SmoothUnion { k } => {
                        let k = *k as f32 / 100.0;
                        smooth_min(a, b, k)
                    }
                    CsgOp::SmoothIntersection { k } => {
                        let k = *k as f32 / 100.0;
                        -smooth_min(-a, -b, k)
                    }
                    CsgOp::SmoothDifference { k } => {
                        let k = *k as f32 / 100.0;
                        -smooth_min(-a, b, k)
                    }
                }
            }
        }
    }
}

impl ScalarField for CsgNode {
    fn evaluate(&self, p: Vec3) -> f32 { self.evaluate(p) }
}

/// A complete CSG tree that can be evaluated as a scalar field.
pub struct CsgTree {
    pub root: CsgNode,
}

impl CsgTree {
    pub fn new(root: CsgNode) -> Self { Self { root } }
    pub fn evaluate(&self, p: Vec3) -> f32 { self.root.evaluate(p) }
}

impl ScalarField for CsgTree {
    fn evaluate(&self, p: Vec3) -> f32 { self.root.evaluate(p) }
}

/// Smooth minimum (polynomial).
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if k < 1e-6 { return a.min(b); }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::implicit::{SdfSphere, SdfBox};

    #[test]
    fn union_takes_minimum() {
        let a = CsgNode::leaf(SdfSphere { center: Vec3::ZERO, radius: 1.0 });
        let b = CsgNode::leaf(SdfSphere { center: Vec3::new(1.0, 0.0, 0.0), radius: 1.0 });
        let u = CsgNode::union(a, b);
        // Point at origin: inside sphere A (neg), should be negative
        assert!(u.evaluate(Vec3::ZERO) < 0.0);
    }

    #[test]
    fn difference_subtracts() {
        let a = CsgNode::leaf(SdfSphere { center: Vec3::ZERO, radius: 2.0 });
        let b = CsgNode::leaf(SdfSphere { center: Vec3::ZERO, radius: 1.0 });
        let d = CsgNode::difference(a, b);
        // Point at origin is inside B, so difference should be positive (carved out)
        assert!(d.evaluate(Vec3::ZERO) > 0.0);
    }
}
