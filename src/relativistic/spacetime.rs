//! Spacetime diagrams: Minkowski, Penrose, worldlines, causal structure.

use glam::{Vec2, Vec3, Vec4};
use super::lorentz;

/// An event in spacetime.
#[derive(Debug, Clone)]
pub struct SpacetimeEvent {
    pub t: f64,
    pub x: f64,
    pub label: String,
}

impl SpacetimeEvent {
    pub fn new(t: f64, x: f64, label: &str) -> Self {
        Self { t, x, label: label.to_string() }
    }

    /// Spacetime interval squared between two events: ds^2 = c^2 dt^2 - dx^2.
    pub fn interval_sq(&self, other: &SpacetimeEvent, c: f64) -> f64 {
        let dt = self.t - other.t;
        let dx = self.x - other.x;
        c * c * dt * dt - dx * dx
    }

    /// As a 2D point (x, t) for plotting.
    pub fn as_point(&self) -> Vec2 {
        Vec2::new(self.x as f32, self.t as f32)
    }
}

/// A worldline: a sequence of spacetime events.
#[derive(Debug, Clone)]
pub struct Worldline {
    pub events: Vec<(f64, f64)>, // (t, x)
    pub color: Vec4,
    pub label: String,
}

impl Worldline {
    pub fn new(label: &str, color: Vec4) -> Self {
        Self {
            events: Vec::new(),
            color,
            label: label.to_string(),
        }
    }

    pub fn add_event(&mut self, t: f64, x: f64) {
        self.events.push((t, x));
    }

    /// Create a worldline for an object at rest at position x.
    pub fn at_rest(x: f64, t_range: (f64, f64), steps: usize, label: &str) -> Self {
        let mut wl = Self::new(label, Vec4::new(1.0, 1.0, 1.0, 1.0));
        let dt = (t_range.1 - t_range.0) / steps.max(1) as f64;
        for i in 0..=steps {
            wl.add_event(t_range.0 + dt * i as f64, x);
        }
        wl
    }

    /// Create a worldline for constant velocity motion.
    pub fn constant_velocity(x0: f64, v: f64, t_range: (f64, f64), steps: usize, label: &str) -> Self {
        let mut wl = Self::new(label, Vec4::new(0.5, 1.0, 0.5, 1.0));
        let dt = (t_range.1 - t_range.0) / steps.max(1) as f64;
        for i in 0..=steps {
            let t = t_range.0 + dt * i as f64;
            wl.add_event(t, x0 + v * t);
        }
        wl
    }

    /// Get the worldline as plotting points.
    pub fn as_points(&self) -> Vec<Vec2> {
        self.events.iter().map(|(t, x)| Vec2::new(*x as f32, *t as f32)).collect()
    }

    /// Velocity at a given segment index.
    pub fn velocity_at(&self, index: usize) -> f64 {
        if index + 1 >= self.events.len() {
            return 0.0;
        }
        let (t1, x1) = self.events[index];
        let (t2, x2) = self.events[index + 1];
        let dt = t2 - t1;
        if dt.abs() < 1e-15 { return 0.0; }
        (x2 - x1) / dt
    }
}

/// Minkowski diagram containing events and worldlines.
#[derive(Debug, Clone)]
pub struct MinkowskiDiagram {
    pub events: Vec<SpacetimeEvent>,
    pub worldlines: Vec<Worldline>,
}

impl MinkowskiDiagram {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            worldlines: Vec::new(),
        }
    }

    pub fn add_event(&mut self, event: SpacetimeEvent) {
        self.events.push(event);
    }

    pub fn add_worldline(&mut self, wl: Worldline) {
        self.worldlines.push(wl);
    }

    /// Get all event positions as plotting points.
    pub fn event_points(&self) -> Vec<Vec2> {
        self.events.iter().map(|e| e.as_point()).collect()
    }
}

/// Compute the light cone from an event.
/// Returns (future_cone, past_cone) as line segments in (x, t) space.
pub fn light_cone(
    event: &SpacetimeEvent,
    c: f64,
) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let extent = 10.0; // how far to draw the cone
    let steps = 50;
    let dt = extent / steps as f64;

    let mut future = Vec::with_capacity(steps * 2);
    let mut past = Vec::with_capacity(steps * 2);

    for i in 0..=steps {
        let t_off = dt * i as f64;
        // Future: rightward and leftward at speed c
        future.push((event.x + c * t_off, event.t + t_off));
        future.push((event.x - c * t_off, event.t + t_off));
        // Past:
        past.push((event.x + c * t_off, event.t - t_off));
        past.push((event.x - c * t_off, event.t - t_off));
    }

    (future, past)
}

/// Check if two events are timelike separated (ds^2 > 0).
pub fn is_timelike(event_a: &SpacetimeEvent, event_b: &SpacetimeEvent, c: f64) -> bool {
    event_a.interval_sq(event_b, c) > 0.0
}

/// Check if two events are spacelike separated (ds^2 < 0).
pub fn is_spacelike(event_a: &SpacetimeEvent, event_b: &SpacetimeEvent, c: f64) -> bool {
    event_a.interval_sq(event_b, c) < 0.0
}

/// Check if two events are lightlike (null) separated (ds^2 ~ 0).
pub fn is_lightlike(event_a: &SpacetimeEvent, event_b: &SpacetimeEvent, c: f64) -> bool {
    event_a.interval_sq(event_b, c).abs() < 1e-10
}

/// Compute proper time along a worldline.
/// tau = integral of sqrt(c^2 dt^2 - dx^2) / c.
pub fn proper_time_along_worldline(worldline: &Worldline, c: f64) -> f64 {
    let mut tau = 0.0;
    for i in 1..worldline.events.len() {
        let (t1, x1) = worldline.events[i - 1];
        let (t2, x2) = worldline.events[i];
        let dt = t2 - t1;
        let dx = x2 - x1;
        let ds2 = c * c * dt * dt - dx * dx;
        if ds2 > 0.0 {
            tau += ds2.sqrt() / c;
        }
    }
    tau
}

/// Lorentz-transform all events in a diagram.
pub fn boost_diagram(diagram: &MinkowskiDiagram, velocity: f64, c: f64) -> MinkowskiDiagram {
    let gamma = lorentz::lorentz_factor(velocity, c);
    let beta = velocity / c;

    let transform = |t: f64, x: f64| -> (f64, f64) {
        let t_new = gamma * (t - beta * x / c);
        let x_new = gamma * (x - velocity * t);
        (t_new, x_new)
    };

    let mut new_diagram = MinkowskiDiagram::new();

    for event in &diagram.events {
        let (t_new, x_new) = transform(event.t, event.x);
        new_diagram.add_event(SpacetimeEvent::new(t_new, x_new, &event.label));
    }

    for wl in &diagram.worldlines {
        let mut new_wl = Worldline::new(&wl.label, wl.color);
        for &(t, x) in &wl.events {
            let (t_new, x_new) = transform(t, x);
            new_wl.add_event(t_new, x_new);
        }
        new_diagram.add_worldline(new_wl);
    }

    new_diagram
}

/// Penrose diagram: conformal compactification of spacetime.
#[derive(Debug, Clone)]
pub struct PenroseDiagram {
    pub events: Vec<(f64, f64)>, // (T, X) in Penrose coordinates
    pub worldlines: Vec<Vec<(f64, f64)>>,
}

impl PenroseDiagram {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            worldlines: Vec::new(),
        }
    }

    /// Add an event in Minkowski coordinates; automatically transforms to Penrose.
    pub fn add_event(&mut self, t: f64, r: f64) {
        let (pt, px) = penrose_transform(t, r);
        self.events.push((pt, px));
    }

    /// Add a worldline in Minkowski coordinates.
    pub fn add_worldline(&mut self, points: &[(f64, f64)]) {
        let transformed: Vec<(f64, f64)> = points.iter()
            .map(|&(t, r)| penrose_transform(t, r))
            .collect();
        self.worldlines.push(transformed);
    }

    /// Get the boundary of the Penrose diagram (diamond shape).
    pub fn boundary(&self, n_points: usize) -> Vec<(f64, f64)> {
        let pi = std::f64::consts::PI;
        let mut boundary = Vec::new();
        let dp = pi / n_points as f64;

        // Right boundary: future null infinity to i+
        for i in 0..=n_points {
            let T = -pi / 2.0 + dp * i as f64;
            let X = pi / 2.0 - T.abs();
            boundary.push((T, X));
        }
        // Left boundary
        for i in (0..=n_points).rev() {
            let T = -pi / 2.0 + dp * i as f64;
            let X = -(pi / 2.0 - T.abs());
            boundary.push((T, X));
        }

        boundary
    }
}

/// Transform Minkowski coordinates (t, r) to Penrose coordinates.
/// Uses the conformal compactification:
///   T = arctan(t + r) + arctan(t - r)
///   X = arctan(t + r) - arctan(t - r)
pub fn penrose_transform(t: f64, r: f64) -> (f64, f64) {
    let u = (t + r).atan();
    let v = (t - r).atan();
    let big_t = u + v;
    let big_x = u - v;
    (big_t, big_x)
}

/// Spacetime renderer for Minkowski diagrams.
#[derive(Debug, Clone)]
pub struct SpacetimeRenderer {
    pub c: f64,
    pub show_light_cones: bool,
    pub show_simultaneity: bool,
    pub x_range: (f32, f32),
    pub t_range: (f32, f32),
    pub grid_color: Vec4,
    pub light_cone_color: Vec4,
}

impl SpacetimeRenderer {
    pub fn new(c: f64) -> Self {
        Self {
            c,
            show_light_cones: true,
            show_simultaneity: true,
            x_range: (-10.0, 10.0),
            t_range: (-10.0, 10.0),
            grid_color: Vec4::new(0.2, 0.2, 0.3, 1.0),
            light_cone_color: Vec4::new(1.0, 1.0, 0.0, 0.5),
        }
    }

    /// Render the diagram: returns lines and points.
    pub fn render_diagram(
        &self,
        diagram: &MinkowskiDiagram,
    ) -> (Vec<(Vec2, Vec2, Vec4)>, Vec<(Vec2, Vec4)>) {
        let mut lines = Vec::new();
        let mut points = Vec::new();

        // Event points
        for event in &diagram.events {
            points.push((event.as_point(), Vec4::new(1.0, 0.3, 0.3, 1.0)));

            // Light cones
            if self.show_light_cones {
                let p = event.as_point();
                let extent = 5.0;
                let c = self.c as f32;
                // Future
                lines.push((p, p + Vec2::new(extent, extent / c), self.light_cone_color));
                lines.push((p, p + Vec2::new(-extent, extent / c), self.light_cone_color));
                // Past
                lines.push((p, p + Vec2::new(extent, -extent / c), self.light_cone_color));
                lines.push((p, p + Vec2::new(-extent, -extent / c), self.light_cone_color));
            }
        }

        // Worldlines
        for wl in &diagram.worldlines {
            let pts = wl.as_points();
            for i in 1..pts.len() {
                lines.push((pts[i - 1], pts[i], wl.color));
            }
        }

        (lines, points)
    }

    /// Render simultaneity surfaces for a boosted observer.
    pub fn simultaneity_lines(
        &self,
        velocity: f64,
        n_lines: usize,
    ) -> Vec<(Vec2, Vec2)> {
        let beta = velocity / self.c;
        let mut lines = Vec::new();
        let dt = (self.t_range.1 - self.t_range.0) / n_lines as f32;

        for i in 0..n_lines {
            let t0 = self.t_range.0 + dt * i as f32;
            // Simultaneity surface has slope beta (in x-t diagram)
            let x1 = self.x_range.0;
            let x2 = self.x_range.1;
            let t1 = t0 + beta as f32 * x1;
            let t2 = t0 + beta as f32 * x2;
            lines.push((Vec2::new(x1, t1), Vec2::new(x2, t2)));
        }
        lines
    }
}

/// Compute the causal structure matrix.
/// Returns a matrix where entry [i][j] is true if event i can causally influence event j.
pub fn causal_structure(events: &[SpacetimeEvent], c: f64) -> Vec<Vec<bool>> {
    let n = events.len();
    let mut matrix = vec![vec![false; n]; n];

    for i in 0..n {
        for j in 0..n {
            if i == j {
                matrix[i][j] = true;
                continue;
            }
            let dt = events[j].t - events[i].t;
            if dt < 0.0 {
                continue; // j is in the past of i, so i cannot influence j
            }
            let dx = (events[j].x - events[i].x).abs();
            // Causal if the separation is timelike or lightlike (and j is in i's future)
            if c * dt >= dx {
                matrix[i][j] = true;
            }
        }
    }

    matrix
}

/// Compute the invariant interval between two events.
pub fn invariant_interval(a: &SpacetimeEvent, b: &SpacetimeEvent, c: f64) -> f64 {
    a.interval_sq(b, c)
}

/// Generate a grid of light cones for visualization.
pub fn light_cone_grid(
    x_range: (f64, f64),
    t_range: (f64, f64),
    c: f64,
    nx: usize,
    nt: usize,
) -> Vec<(SpacetimeEvent, Vec<(f64, f64)>, Vec<(f64, f64)>)> {
    let dx = (x_range.1 - x_range.0) / nx.max(1) as f64;
    let dt = (t_range.1 - t_range.0) / nt.max(1) as f64;
    let mut grid = Vec::new();

    for it in 0..=nt {
        for ix in 0..=nx {
            let x = x_range.0 + dx * ix as f64;
            let t = t_range.0 + dt * it as f64;
            let event = SpacetimeEvent::new(t, x, "");
            let (future, past) = light_cone(&event, c);
            grid.push((event, future, past));
        }
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 1.0; // use natural units for simplicity

    #[test]
    fn test_timelike_separation() {
        let a = SpacetimeEvent::new(0.0, 0.0, "A");
        let b = SpacetimeEvent::new(2.0, 1.0, "B");
        // ds^2 = 4 - 1 = 3 > 0
        assert!(is_timelike(&a, &b, C));
        assert!(!is_spacelike(&a, &b, C));
    }

    #[test]
    fn test_spacelike_separation() {
        let a = SpacetimeEvent::new(0.0, 0.0, "A");
        let b = SpacetimeEvent::new(1.0, 3.0, "B");
        // ds^2 = 1 - 9 = -8 < 0
        assert!(is_spacelike(&a, &b, C));
        assert!(!is_timelike(&a, &b, C));
    }

    #[test]
    fn test_lightlike_separation() {
        let a = SpacetimeEvent::new(0.0, 0.0, "A");
        let b = SpacetimeEvent::new(5.0, 5.0, "B");
        // ds^2 = 25 - 25 = 0
        assert!(is_lightlike(&a, &b, C));
    }

    #[test]
    fn test_light_cone_structure() {
        let event = SpacetimeEvent::new(0.0, 0.0, "O");
        let (future, past) = light_cone(&event, C);
        assert!(!future.is_empty());
        assert!(!past.is_empty());
        // First future point should be at origin
        assert!((future[0].0).abs() < 1e-10);
        assert!((future[0].1).abs() < 1e-10);
    }

    #[test]
    fn test_proper_time_at_rest() {
        let wl = Worldline::at_rest(0.0, (0.0, 10.0), 100, "rest");
        let tau = proper_time_along_worldline(&wl, C);
        // At rest, proper time = coordinate time
        assert!((tau - 10.0).abs() < 0.1, "Proper time at rest: {}", tau);
    }

    #[test]
    fn test_proper_time_moving() {
        let v = 0.866; // gamma ~ 2
        let wl = Worldline::constant_velocity(0.0, v, (0.0, 10.0), 1000, "moving");
        let tau = proper_time_along_worldline(&wl, C);
        // tau = t / gamma ~ 10 / 2 = 5
        assert!((tau - 5.0).abs() < 0.1, "Moving proper time: {}", tau);
    }

    #[test]
    fn test_boost_diagram_preserves_interval() {
        let mut diagram = MinkowskiDiagram::new();
        let a = SpacetimeEvent::new(0.0, 0.0, "A");
        let b = SpacetimeEvent::new(3.0, 1.0, "B");
        let interval_before = a.interval_sq(&b, C);
        diagram.add_event(a);
        diagram.add_event(b);

        let boosted = boost_diagram(&diagram, 0.5, C);
        let interval_after = boosted.events[0].interval_sq(&boosted.events[1], C);

        assert!(
            (interval_before - interval_after).abs() < 1e-6,
            "Interval not preserved: {} vs {}",
            interval_before, interval_after
        );
    }

    #[test]
    fn test_causal_structure() {
        let events = vec![
            SpacetimeEvent::new(0.0, 0.0, "A"),
            SpacetimeEvent::new(1.0, 0.5, "B"), // timelike future of A
            SpacetimeEvent::new(0.5, 3.0, "C"), // spacelike from A
        ];
        let matrix = causal_structure(&events, C);

        // A can influence B (timelike, B in future)
        assert!(matrix[0][1], "A should causally influence B");
        // A cannot influence C (spacelike)
        assert!(!matrix[0][2], "A should not influence C (spacelike)");
        // Self-influence
        assert!(matrix[0][0]);
        assert!(matrix[1][1]);
    }

    #[test]
    fn test_causal_structure_no_backward() {
        let events = vec![
            SpacetimeEvent::new(5.0, 0.0, "A"),
            SpacetimeEvent::new(0.0, 0.0, "B"), // B is in the past of A
        ];
        let matrix = causal_structure(&events, C);
        // A cannot influence B (B is in the past)
        assert!(!matrix[0][1]);
        // B can influence A
        assert!(matrix[1][0]);
    }

    #[test]
    fn test_penrose_transform_origin() {
        let (t, x) = penrose_transform(0.0, 0.0);
        assert!(t.abs() < 1e-10);
        assert!(x.abs() < 1e-10);
    }

    #[test]
    fn test_penrose_transform_finite() {
        // Even large coordinates map to finite values
        let (t, x) = penrose_transform(1e10, 0.0);
        assert!(t.is_finite());
        assert!(x.is_finite());
        assert!(t.abs() < std::f64::consts::PI);
    }

    #[test]
    fn test_invariant_interval() {
        let a = SpacetimeEvent::new(0.0, 0.0, "A");
        let b = SpacetimeEvent::new(3.0, 4.0, "B");
        let ds2 = invariant_interval(&a, &b, C);
        // c=1: ds^2 = 9 - 16 = -7
        assert!((ds2 - (-7.0)).abs() < 1e-10);
    }

    #[test]
    fn test_spacetime_renderer() {
        let renderer = SpacetimeRenderer::new(C);
        let mut diagram = MinkowskiDiagram::new();
        diagram.add_event(SpacetimeEvent::new(0.0, 0.0, "O"));
        diagram.add_worldline(Worldline::at_rest(0.0, (-5.0, 5.0), 10, "static"));

        let (lines, points) = renderer.render_diagram(&diagram);
        assert!(!points.is_empty());
        assert!(!lines.is_empty()); // light cone lines
    }

    #[test]
    fn test_penrose_diagram() {
        let mut pd = PenroseDiagram::new();
        pd.add_event(0.0, 0.0);
        pd.add_event(5.0, 3.0);
        assert_eq!(pd.events.len(), 2);

        let boundary = pd.boundary(10);
        assert!(!boundary.is_empty());
    }

    #[test]
    fn test_worldline_velocity() {
        let wl = Worldline::constant_velocity(0.0, 0.5, (0.0, 10.0), 100, "v=0.5");
        let v = wl.velocity_at(50);
        assert!((v - 0.5).abs() < 0.01, "Velocity: {}", v);
    }
}
