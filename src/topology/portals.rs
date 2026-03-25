// topology/portals.rs — Portals between topological spaces

use glam::Vec2;

// ─── Types ─────────────────────────────────────────────────────────────────

/// The topology type on each side of a portal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TopologyType {
    Euclidean,
    Hyperbolic,
    Spherical,
    Toroidal,
    Klein,
}

/// A portal frame: a line segment in 2D space with a facing direction.
#[derive(Clone, Debug)]
pub struct PortalFrame {
    pub position: Vec2,
    pub normal: Vec2,
    pub width: f32,
}

impl PortalFrame {
    pub fn new(position: Vec2, normal: Vec2, width: f32) -> Self {
        Self {
            position,
            normal: normal.normalize(),
            width,
        }
    }

    /// Get the two endpoints of the portal frame.
    pub fn endpoints(&self) -> (Vec2, Vec2) {
        let tangent = Vec2::new(-self.normal.y, self.normal.x);
        let half = tangent * (self.width / 2.0);
        (self.position - half, self.position + half)
    }

    /// Check if a point is on the front side of the portal.
    pub fn is_front_side(&self, point: Vec2) -> bool {
        (point - self.position).dot(self.normal) > 0.0
    }

    /// Signed distance from a point to the portal plane.
    pub fn signed_distance(&self, point: Vec2) -> f32 {
        (point - self.position).dot(self.normal)
    }
}

/// A portal connecting two frames, possibly with a topology change.
#[derive(Clone, Debug)]
pub struct Portal {
    pub entry: PortalFrame,
    pub exit: PortalFrame,
    pub topology_change: TopologyType,
}

impl Portal {
    pub fn new(entry: PortalFrame, exit: PortalFrame, topology_change: TopologyType) -> Self {
        Self {
            entry,
            exit,
            topology_change,
        }
    }

    /// Check if an entity at `pos` moving with `vel` will cross this portal this frame.
    /// Returns true if the entity crosses from front to back of the entry frame.
    pub fn will_cross(&self, pos: Vec2, vel: Vec2, dt: f32) -> bool {
        let d_before = self.entry.signed_distance(pos);
        let d_after = self.entry.signed_distance(pos + vel * dt);

        // Must go from positive to negative (front to back)
        if d_before <= 0.0 || d_after > 0.0 {
            return false;
        }

        // Check if crossing point is within the portal width
        let t = d_before / (d_before - d_after);
        let cross_point = pos + vel * dt * t;
        let tangent = Vec2::new(-self.entry.normal.y, self.entry.normal.x);
        let along = (cross_point - self.entry.position).dot(tangent);
        along.abs() <= self.entry.width / 2.0
    }
}

/// Transform a position and velocity through a portal.
/// Maps coordinates from the entry frame to the exit frame.
pub fn transform_through_portal(pos: Vec2, vel: Vec2, portal: &Portal) -> (Vec2, Vec2) {
    let entry = &portal.entry;
    let exit = &portal.exit;

    // Compute local coordinates relative to entry frame
    let entry_tangent = Vec2::new(-entry.normal.y, entry.normal.x);
    let local_along = (pos - entry.position).dot(entry_tangent);
    let local_through = (pos - entry.position).dot(entry.normal);

    // Scale by width ratio
    let scale = exit.width / entry.width.max(1e-6);
    let scaled_along = local_along * scale;

    // Map to exit frame (flipped normal — you emerge from the back side of exit)
    let exit_tangent = Vec2::new(-exit.normal.y, exit.normal.x);
    let new_pos = exit.position + exit_tangent * scaled_along - exit.normal * local_through;

    // Transform velocity
    let vel_along = vel.dot(entry_tangent);
    let vel_through = vel.dot(entry.normal);
    let new_vel = exit_tangent * vel_along * scale - exit.normal * vel_through;

    (new_pos, new_vel)
}

// ─── Portal Manager ────────────────────────────────────────────────────────

/// Manages a set of active portals and handles entity transitions.
pub struct PortalManager {
    pub portals: Vec<Portal>,
}

impl PortalManager {
    pub fn new() -> Self {
        Self {
            portals: Vec::new(),
        }
    }

    /// Add a portal.
    pub fn add_portal(&mut self, portal: Portal) {
        self.portals.push(portal);
    }

    /// Remove a portal by index.
    pub fn remove_portal(&mut self, index: usize) {
        if index < self.portals.len() {
            self.portals.remove(index);
        }
    }

    /// Update an entity position/velocity, checking all portals.
    /// Returns (new_pos, new_vel, Option<topology_change>).
    pub fn update_entity(
        &self,
        pos: Vec2,
        vel: Vec2,
        dt: f32,
    ) -> (Vec2, Vec2, Option<TopologyType>) {
        for portal in &self.portals {
            if portal.will_cross(pos, vel, dt) {
                let (new_pos, new_vel) = transform_through_portal(pos, vel, portal);
                return (new_pos, new_vel, Some(portal.topology_change));
            }
        }
        (pos + vel * dt, vel, None)
    }

    /// Find all portals visible from a given position (front side of entry).
    pub fn visible_portals(&self, pos: Vec2) -> Vec<usize> {
        self.portals
            .iter()
            .enumerate()
            .filter(|(_, p)| p.entry.is_front_side(pos))
            .map(|(i, _)| i)
            .collect()
    }
}

/// Compute the visible region through a portal from a viewpoint.
/// Returns a trapezoid (4 corners) representing what can be seen through the portal.
pub fn render_through_portal(viewpoint: Vec2, portal: &Portal, view_depth: f32) -> [Vec2; 4] {
    let (left, right) = portal.entry.endpoints();

    // Direction from viewpoint to each endpoint
    let to_left = (left - viewpoint).normalize();
    let to_right = (right - viewpoint).normalize();

    // Project these rays through the portal into exit space
    let entry = &portal.entry;
    let exit = &portal.exit;
    let exit_tangent = Vec2::new(-exit.normal.y, exit.normal.x);
    let (exit_left, exit_right) = exit.endpoints();

    // The visible region is a trapezoid from exit endpoints extending along the exit normal
    let far_left = exit_left - exit.normal * view_depth;
    let far_right = exit_right - exit.normal * view_depth;

    [exit_left, exit_right, far_right, far_left]
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_portal() -> Portal {
        Portal::new(
            PortalFrame::new(Vec2::new(5.0, 0.0), Vec2::new(-1.0, 0.0), 4.0),
            PortalFrame::new(Vec2::new(20.0, 0.0), Vec2::new(1.0, 0.0), 4.0),
            TopologyType::Euclidean,
        )
    }

    #[test]
    fn test_portal_frame_endpoints() {
        let frame = PortalFrame::new(Vec2::new(5.0, 0.0), Vec2::new(1.0, 0.0), 4.0);
        let (a, b) = frame.endpoints();
        assert!((a.y - (-2.0)).abs() < 1e-4);
        assert!((b.y - 2.0).abs() < 1e-4);
        assert!((a.x - 5.0).abs() < 1e-4);
    }

    #[test]
    fn test_portal_front_side() {
        let frame = PortalFrame::new(Vec2::new(5.0, 0.0), Vec2::new(1.0, 0.0), 4.0);
        assert!(frame.is_front_side(Vec2::new(10.0, 0.0)));
        assert!(!frame.is_front_side(Vec2::new(0.0, 0.0)));
    }

    #[test]
    fn test_will_cross() {
        let portal = make_simple_portal();
        // Moving left (in direction of entry normal = -x) through x=5
        let crosses = portal.will_cross(
            Vec2::new(6.0, 0.0),   // in front of entry
            Vec2::new(-10.0, 0.0), // moving toward entry
            1.0,
        );
        assert!(crosses, "Should cross the portal");
    }

    #[test]
    fn test_will_not_cross_wrong_direction() {
        let portal = make_simple_portal();
        let crosses = portal.will_cross(
            Vec2::new(6.0, 0.0),
            Vec2::new(10.0, 0.0), // moving away
            1.0,
        );
        assert!(!crosses);
    }

    #[test]
    fn test_will_not_cross_outside_width() {
        let portal = make_simple_portal();
        let crosses = portal.will_cross(
            Vec2::new(6.0, 10.0), // far from portal centerline
            Vec2::new(-10.0, 0.0),
            1.0,
        );
        assert!(!crosses);
    }

    #[test]
    fn test_transform_through_portal() {
        let portal = Portal::new(
            PortalFrame::new(Vec2::new(5.0, 0.0), Vec2::new(-1.0, 0.0), 4.0),
            PortalFrame::new(Vec2::new(20.0, 0.0), Vec2::new(1.0, 0.0), 4.0),
            TopologyType::Euclidean,
        );
        // Point right at the entry position
        let (new_pos, new_vel) = transform_through_portal(
            Vec2::new(5.0, 0.0),
            Vec2::new(-5.0, 0.0),
            &portal,
        );
        // Should emerge at exit position
        assert!((new_pos.x - 20.0).abs() < 1e-3, "new_pos.x = {}", new_pos.x);
    }

    #[test]
    fn test_portal_manager_no_portals() {
        let mgr = PortalManager::new();
        let (pos, vel, topo) = mgr.update_entity(
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            1.0,
        );
        assert!((pos.x - 1.0).abs() < 1e-4);
        assert!(topo.is_none());
    }

    #[test]
    fn test_portal_manager_crossing() {
        let mut mgr = PortalManager::new();
        mgr.add_portal(make_simple_portal());

        let (new_pos, _new_vel, topo) = mgr.update_entity(
            Vec2::new(6.0, 0.0),
            Vec2::new(-10.0, 0.0),
            1.0,
        );
        assert!(topo.is_some());
        assert_eq!(topo.unwrap(), TopologyType::Euclidean);
    }

    #[test]
    fn test_visible_portals() {
        let mut mgr = PortalManager::new();
        mgr.add_portal(make_simple_portal());
        let visible = mgr.visible_portals(Vec2::new(4.0, 0.0));
        assert_eq!(visible.len(), 1);

        let visible2 = mgr.visible_portals(Vec2::new(6.0, 0.0));
        assert_eq!(visible2.len(), 0); // behind the entry normal
    }

    #[test]
    fn test_render_through_portal() {
        let portal = make_simple_portal();
        let region = render_through_portal(Vec2::new(0.0, 0.0), &portal, 10.0);
        // Should return 4 points
        assert_eq!(region.len(), 4);
    }

    #[test]
    fn test_portal_manager_remove() {
        let mut mgr = PortalManager::new();
        mgr.add_portal(make_simple_portal());
        assert_eq!(mgr.portals.len(), 1);
        mgr.remove_portal(0);
        assert_eq!(mgr.portals.len(), 0);
    }
}
