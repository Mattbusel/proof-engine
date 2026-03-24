//! Cohesion dynamics — how strongly glyphs cling to their formation positions.

use glam::Vec3;

/// Calculate how far a glyph at `actual` should move toward `target`
/// given cohesion strength [0, 1] and elapsed time dt.
pub fn cohesion_pull(actual: Vec3, target: Vec3, cohesion: f32, dt: f32) -> Vec3 {
    let delta = target - actual;
    // Spring pull toward target: stronger cohesion = snappier return
    let stiffness = cohesion * 8.0;
    delta * stiffness * dt
}

/// Emit a formation dissolution effect when cohesion reaches 0.
/// Returns a list of velocity vectors for each glyph (outward burst).
pub fn dissolution_burst(positions: &[Vec3], center: Vec3) -> Vec<Vec3> {
    positions.iter().map(|pos| {
        let dir = (*pos - center).normalize_or_zero();
        dir * (2.0 + rand_f32(*pos))
    }).collect()
}

fn rand_f32(v: Vec3) -> f32 {
    let h = (v.x * 127.1 + v.y * 311.7 + v.z * 74.3) as u64;
    let h = h.wrapping_mul(0x9e3779b97f4a7c15);
    ((h >> 32) as f32 / u32::MAX as f32) * 2.0
}
