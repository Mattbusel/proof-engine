use glam::Vec2;
use std::collections::HashMap;
use super::graph_core::{NodeId, EdgeId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationKind {
    AddNode,
    RemoveNode,
    AddEdge,
    RemoveEdge,
}

#[derive(Debug, Clone)]
pub struct NodeAnimation {
    pub kind: AnimationKind,
    pub node_id: NodeId,
    pub start_pos: Vec2,
    pub target_pos: Vec2,
    pub progress: f32,
    pub duration: f32,
    pub scale: f32,
    pub alpha: f32,
}

#[derive(Debug, Clone)]
pub struct EdgeAnimation {
    pub kind: AnimationKind,
    pub edge_id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub progress: f32,
    pub duration: f32,
    pub draw_progress: f32, // 0..1 how much of the edge is drawn
    pub alpha: f32,
}

#[derive(Debug, Clone)]
pub struct AnimationState {
    pub node_positions: HashMap<NodeId, Vec2>,
    pub node_scales: HashMap<NodeId, f32>,
    pub node_alphas: HashMap<NodeId, f32>,
    pub edge_draw_progress: HashMap<EdgeId, f32>,
    pub edge_alphas: HashMap<EdgeId, f32>,
}

impl AnimationState {
    pub fn new() -> Self {
        Self {
            node_positions: HashMap::new(),
            node_scales: HashMap::new(),
            node_alphas: HashMap::new(),
            edge_draw_progress: HashMap::new(),
            edge_alphas: HashMap::new(),
        }
    }
}

pub struct GraphAnimator {
    node_anims: Vec<NodeAnimation>,
    edge_anims: Vec<EdgeAnimation>,
    default_duration: f32,
    completed_removes: Vec<(AnimationKind, u32)>, // (kind, id)
}

impl GraphAnimator {
    pub fn new() -> Self {
        Self {
            node_anims: Vec::new(),
            edge_anims: Vec::new(),
            default_duration: 0.5,
            completed_removes: Vec::new(),
        }
    }

    pub fn with_duration(mut self, duration: f32) -> Self {
        self.default_duration = duration;
        self
    }

    /// Animate a node appearing: starts at center (0,0), scales from 0 to 1, tweens to target.
    pub fn animate_add_node(&mut self, id: NodeId, target_pos: Vec2) {
        self.node_anims.push(NodeAnimation {
            kind: AnimationKind::AddNode,
            node_id: id,
            start_pos: Vec2::ZERO,
            target_pos,
            progress: 0.0,
            duration: self.default_duration,
            scale: 0.0,
            alpha: 0.0,
        });
    }

    /// Animate a node disappearing: fades out and scales to 0.
    pub fn animate_remove_node(&mut self, id: NodeId, current_pos: Vec2) {
        self.node_anims.push(NodeAnimation {
            kind: AnimationKind::RemoveNode,
            node_id: id,
            start_pos: current_pos,
            target_pos: current_pos,
            progress: 0.0,
            duration: self.default_duration,
            scale: 1.0,
            alpha: 1.0,
        });
    }

    /// Animate an edge being drawn from one end to the other.
    pub fn animate_add_edge(&mut self, id: EdgeId, from: NodeId, to: NodeId) {
        self.edge_anims.push(EdgeAnimation {
            kind: AnimationKind::AddEdge,
            edge_id: id,
            from,
            to,
            progress: 0.0,
            duration: self.default_duration,
            draw_progress: 0.0,
            alpha: 1.0,
        });
    }

    /// Animate an edge fading out.
    pub fn animate_remove_edge(&mut self, id: EdgeId, from: NodeId, to: NodeId) {
        self.edge_anims.push(EdgeAnimation {
            kind: AnimationKind::RemoveEdge,
            edge_id: id,
            from,
            to,
            progress: 0.0,
            duration: self.default_duration,
            draw_progress: 1.0,
            alpha: 1.0,
        });
    }

    /// Advance all animations by dt seconds.
    pub fn tick(&mut self, dt: f32) -> AnimationState {
        let mut state = AnimationState::new();
        self.completed_removes.clear();

        // Update node animations
        for anim in &mut self.node_anims {
            anim.progress = (anim.progress + dt / anim.duration).min(1.0);
            let t = ease_out_cubic(anim.progress);

            match anim.kind {
                AnimationKind::AddNode => {
                    let pos = anim.start_pos.lerp(anim.target_pos, t);
                    anim.scale = t;
                    anim.alpha = t;
                    state.node_positions.insert(anim.node_id, pos);
                    state.node_scales.insert(anim.node_id, anim.scale);
                    state.node_alphas.insert(anim.node_id, anim.alpha);
                }
                AnimationKind::RemoveNode => {
                    anim.scale = 1.0 - t;
                    anim.alpha = 1.0 - t;
                    state.node_positions.insert(anim.node_id, anim.start_pos);
                    state.node_scales.insert(anim.node_id, anim.scale);
                    state.node_alphas.insert(anim.node_id, anim.alpha);
                }
                _ => {}
            }
        }

        // Update edge animations
        for anim in &mut self.edge_anims {
            anim.progress = (anim.progress + dt / anim.duration).min(1.0);
            let t = ease_out_cubic(anim.progress);

            match anim.kind {
                AnimationKind::AddEdge => {
                    anim.draw_progress = t;
                    anim.alpha = 1.0;
                    state.edge_draw_progress.insert(anim.edge_id, anim.draw_progress);
                    state.edge_alphas.insert(anim.edge_id, anim.alpha);
                }
                AnimationKind::RemoveEdge => {
                    anim.alpha = 1.0 - t;
                    state.edge_draw_progress.insert(anim.edge_id, 1.0);
                    state.edge_alphas.insert(anim.edge_id, anim.alpha);
                }
                _ => {}
            }
        }

        // Remove completed animations
        self.node_anims.retain(|a| a.progress < 1.0);
        self.edge_anims.retain(|a| a.progress < 1.0);

        state
    }

    /// Returns true if any animations are still running.
    pub fn is_animating(&self) -> bool {
        !self.node_anims.is_empty() || !self.edge_anims.is_empty()
    }

    /// Number of active animations.
    pub fn active_count(&self) -> usize {
        self.node_anims.len() + self.edge_anims.len()
    }
}

/// Cubic ease-out: decelerating to zero velocity.
fn ease_out_cubic(t: f32) -> f32 {
    let t1 = t - 1.0;
    t1 * t1 * t1 + 1.0
}

/// Cubic ease-in-out.
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let t1 = -2.0 * t + 2.0;
        1.0 - t1 * t1 * t1 / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_node_animation() {
        let mut animator = GraphAnimator::new().with_duration(1.0);
        let nid = NodeId(0);
        animator.animate_add_node(nid, Vec2::new(100.0, 200.0));

        assert!(animator.is_animating());

        // Tick halfway
        let state = animator.tick(0.5);
        let pos = state.node_positions[&nid];
        let scale = state.node_scales[&nid];
        assert!(pos.x > 0.0 && pos.x < 100.0);
        assert!(scale > 0.0 && scale < 1.0);

        // Tick to completion
        let state = animator.tick(0.6);
        assert!(!animator.is_animating());
    }

    #[test]
    fn test_remove_node_animation() {
        let mut animator = GraphAnimator::new().with_duration(1.0);
        let nid = NodeId(0);
        animator.animate_remove_node(nid, Vec2::new(50.0, 50.0));

        let state = animator.tick(0.5);
        let alpha = state.node_alphas[&nid];
        assert!(alpha > 0.0 && alpha < 1.0);

        let state = animator.tick(0.6);
        assert!(!animator.is_animating());
    }

    #[test]
    fn test_add_edge_animation() {
        let mut animator = GraphAnimator::new().with_duration(1.0);
        let eid = EdgeId(0);
        animator.animate_add_edge(eid, NodeId(0), NodeId(1));

        let state = animator.tick(0.5);
        let draw = state.edge_draw_progress[&eid];
        assert!(draw > 0.0 && draw < 1.0);
    }

    #[test]
    fn test_remove_edge_animation() {
        let mut animator = GraphAnimator::new().with_duration(1.0);
        let eid = EdgeId(0);
        animator.animate_remove_edge(eid, NodeId(0), NodeId(1));

        let state = animator.tick(0.5);
        let alpha = state.edge_alphas[&eid];
        assert!(alpha > 0.0 && alpha < 1.0);
    }

    #[test]
    fn test_multiple_animations() {
        let mut animator = GraphAnimator::new().with_duration(0.5);
        animator.animate_add_node(NodeId(0), Vec2::new(10.0, 10.0));
        animator.animate_add_node(NodeId(1), Vec2::new(20.0, 20.0));
        animator.animate_add_edge(EdgeId(0), NodeId(0), NodeId(1));

        assert_eq!(animator.active_count(), 3);
        let state = animator.tick(0.25);
        assert_eq!(state.node_positions.len(), 2);

        let state = animator.tick(0.3);
        assert!(!animator.is_animating());
    }

    #[test]
    fn test_ease_out_cubic() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-6);
        // Monotonically increasing
        assert!(ease_out_cubic(0.5) > ease_out_cubic(0.25));
    }

    #[test]
    fn test_no_animations() {
        let mut animator = GraphAnimator::new();
        assert!(!animator.is_animating());
        let state = animator.tick(0.1);
        assert!(state.node_positions.is_empty());
    }
}
