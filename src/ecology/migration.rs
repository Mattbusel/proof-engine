//! Migration patterns — seasonal flow of entity populations.

/// A migration route.
#[derive(Debug, Clone)]
pub struct MigrationRoute {
    pub species_id: u32,
    pub waypoints: Vec<(f32, f32)>,
    pub season_start: f32,   // 0.0-1.0 (fraction of year)
    pub season_end: f32,
    pub fraction: f64,       // fraction of population that migrates
}

/// Compute current position along a migration route given time.
pub fn route_position(route: &MigrationRoute, time_of_year: f32) -> Option<(f32, f32)> {
    if route.waypoints.len() < 2 { return None; }
    let t = time_of_year;
    if t < route.season_start || t > route.season_end { return None; }

    let progress = (t - route.season_start) / (route.season_end - route.season_start);
    let segment_f = progress * (route.waypoints.len() - 1) as f32;
    let seg = (segment_f as usize).min(route.waypoints.len() - 2);
    let local_t = segment_f - seg as f32;

    let (x0, y0) = route.waypoints[seg];
    let (x1, y1) = route.waypoints[seg + 1];
    Some((x0 + (x1 - x0) * local_t, y0 + (y1 - y0) * local_t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_position() {
        let route = MigrationRoute {
            species_id: 0,
            waypoints: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
            season_start: 0.2,
            season_end: 0.8,
            fraction: 0.5,
        };
        let pos = route_position(&route, 0.5).unwrap();
        assert!(pos.0 > 0.0, "should have moved");
    }
}
