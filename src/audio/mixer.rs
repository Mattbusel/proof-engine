//! Spatial audio mixer — mixes MathAudioSources based on 3D position.

/// Mix weight for a source given listener position and source position.
pub fn spatial_weight(listener: glam::Vec3, source: glam::Vec3, max_distance: f32) -> f32 {
    let dist = (source - listener).length();
    if dist >= max_distance { return 0.0; }
    1.0 - dist / max_distance
}

/// Compute stereo pan from a 3D position relative to listener.
/// Returns (left, right) gain [0, 1].
pub fn stereo_pan(listener: glam::Vec3, source: glam::Vec3) -> (f32, f32) {
    let delta = source - listener;
    let pan = (delta.x / (delta.length().max(0.001))).clamp(-1.0, 1.0);
    let left  = (1.0 - pan).sqrt() * 0.5f32.sqrt();
    let right = (1.0 + pan).sqrt() * 0.5f32.sqrt();
    (left, right)
}
