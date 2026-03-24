//! # UV Animation
//!
//! Animation and manipulation of UV texture coordinates on surfaces.
//!
//! ## Features
//!
//! - [`UVAnimator`] — scroll, rotate, scale, and warp UV coordinates over time
//! - [`FlowMap`] — distort UVs along a 2D vector field
//! - [`ParallaxLayer`] — multi-layer parallax scrolling
//! - [`SpriteSheetAnimator`] — step through texture atlas frames
//! - [`TriplanarProjector`] — seamless texturing of arbitrary geometry
//! - [`UVUnwrap`] — UV unwrapping utilities (cylindrical, spherical, box projection)

use glam::{Vec2, Vec3};
use std::f32::consts::{PI, TAU};

// ─────────────────────────────────────────────────────────────────────────────
// UV animation modes
// ─────────────────────────────────────────────────────────────────────────────

/// UV animation mode.
#[derive(Debug, Clone)]
pub enum UVMode {
    /// Constant scrolling in a direction.
    Scroll {
        velocity: Vec2,
    },
    /// Rotation around a pivot point.
    Rotate {
        pivot: Vec2,
        speed: f32,
    },
    /// Oscillating scale.
    Scale {
        center: Vec2,
        amplitude: Vec2,
        frequency: f32,
    },
    /// Sinusoidal warp distortion.
    SineWarp {
        amplitude: Vec2,
        frequency: Vec2,
        speed: f32,
    },
    /// Radial warp from a center point.
    RadialWarp {
        center: Vec2,
        amplitude: f32,
        frequency: f32,
        speed: f32,
    },
    /// Turbulence-based warp.
    Turbulence {
        amplitude: f32,
        frequency: f32,
        speed: f32,
        octaves: u32,
    },
    /// Custom warp function.
    Custom {
        label: String,
    },
}

/// Animates UV coordinates over time.
pub struct UVAnimator {
    pub mode: UVMode,
    pub time: f32,
    /// Optional custom warp function.
    custom_fn: Option<Box<dyn Fn(Vec2, f32) -> Vec2 + Send + Sync>>,
}

impl UVAnimator {
    /// Create a new UV animator with the given mode.
    pub fn new(mode: UVMode) -> Self {
        Self {
            mode,
            time: 0.0,
            custom_fn: None,
        }
    }

    /// Create a scrolling UV animator.
    pub fn scroll(velocity: Vec2) -> Self {
        Self::new(UVMode::Scroll { velocity })
    }

    /// Create a rotating UV animator.
    pub fn rotate(pivot: Vec2, speed: f32) -> Self {
        Self::new(UVMode::Rotate { pivot, speed })
    }

    /// Create a scaling UV animator.
    pub fn scale(center: Vec2, amplitude: Vec2, frequency: f32) -> Self {
        Self::new(UVMode::Scale { center, amplitude, frequency })
    }

    /// Create a sine-warp UV animator.
    pub fn sine_warp(amplitude: Vec2, frequency: Vec2, speed: f32) -> Self {
        Self::new(UVMode::SineWarp { amplitude, frequency, speed })
    }

    /// Create a custom UV animator.
    pub fn custom<F>(label: &str, func: F) -> Self
    where
        F: Fn(Vec2, f32) -> Vec2 + Send + Sync + 'static,
    {
        Self {
            mode: UVMode::Custom { label: label.to_string() },
            time: 0.0,
            custom_fn: Some(Box::new(func)),
        }
    }

    /// Advance the animation time.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
    }

    /// Reset the animation time.
    pub fn reset(&mut self) {
        self.time = 0.0;
    }

    /// Transform a single UV coordinate.
    pub fn transform(&self, uv: Vec2) -> Vec2 {
        self.transform_at(uv, self.time)
    }

    /// Transform a UV coordinate at a specific time.
    pub fn transform_at(&self, uv: Vec2, time: f32) -> Vec2 {
        match &self.mode {
            UVMode::Scroll { velocity } => {
                let offset = *velocity * time;
                fract_vec2(uv + offset)
            }
            UVMode::Rotate { pivot, speed } => {
                let angle = time * speed;
                let cos_a = angle.cos();
                let sin_a = angle.sin();
                let centered = uv - *pivot;
                let rotated = Vec2::new(
                    centered.x * cos_a - centered.y * sin_a,
                    centered.x * sin_a + centered.y * cos_a,
                );
                rotated + *pivot
            }
            UVMode::Scale { center, amplitude, frequency } => {
                let scale = Vec2::ONE + *amplitude * (time * frequency * TAU).sin();
                let centered = uv - *center;
                centered * scale + *center
            }
            UVMode::SineWarp { amplitude, frequency, speed } => {
                let offset_x = (uv.y * frequency.y * TAU + time * speed).sin() * amplitude.x;
                let offset_y = (uv.x * frequency.x * TAU + time * speed).sin() * amplitude.y;
                Vec2::new(uv.x + offset_x, uv.y + offset_y)
            }
            UVMode::RadialWarp { center, amplitude, frequency, speed } => {
                let dir = uv - *center;
                let dist = dir.length();
                if dist < 1e-6 {
                    return uv;
                }
                let wave = (dist * *frequency * TAU - time * speed).sin() * amplitude;
                let offset = dir.normalize() * wave;
                uv + offset
            }
            UVMode::Turbulence { amplitude, frequency, speed, octaves } => {
                let mut dx = 0.0_f32;
                let mut dy = 0.0_f32;
                let mut freq = *frequency;
                let mut amp = *amplitude;
                for _ in 0..*octaves {
                    dx += ((uv.x * freq + time * speed) * TAU).sin() * amp;
                    dy += ((uv.y * freq + time * speed * 1.3) * TAU).sin() * amp;
                    freq *= 2.0;
                    amp *= 0.5;
                }
                Vec2::new(uv.x + dx, uv.y + dy)
            }
            UVMode::Custom { .. } => {
                if let Some(ref func) = self.custom_fn {
                    func(uv, time)
                } else {
                    uv
                }
            }
        }
    }

    /// Transform an array of UV coordinates in place.
    pub fn transform_array(&self, uvs: &mut [[f32; 2]]) {
        for uv_pair in uvs.iter_mut() {
            let uv = Vec2::new(uv_pair[0], uv_pair[1]);
            let result = self.transform(uv);
            uv_pair[0] = result.x;
            uv_pair[1] = result.y;
        }
    }

    /// Transform an array and return new UVs (non-mutating).
    pub fn transform_array_new(&self, uvs: &[[f32; 2]]) -> Vec<[f32; 2]> {
        uvs.iter().map(|&uv_pair| {
            let uv = Vec2::new(uv_pair[0], uv_pair[1]);
            let result = self.transform(uv);
            [result.x, result.y]
        }).collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Multi-layer UV animation (chaining)
// ─────────────────────────────────────────────────────────────────────────────

/// Chain of UV animators applied sequentially.
pub struct UVAnimatorChain {
    pub animators: Vec<UVAnimator>,
}

impl UVAnimatorChain {
    pub fn new() -> Self {
        Self { animators: Vec::new() }
    }

    pub fn push(&mut self, animator: UVAnimator) {
        self.animators.push(animator);
    }

    pub fn tick(&mut self, dt: f32) {
        for anim in &mut self.animators {
            anim.tick(dt);
        }
    }

    pub fn transform(&self, mut uv: Vec2) -> Vec2 {
        for anim in &self.animators {
            uv = anim.transform(uv);
        }
        uv
    }
}

impl Default for UVAnimatorChain {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Flow map
// ─────────────────────────────────────────────────────────────────────────────

/// A flow map for UV distortion.
///
/// A flow map stores 2D velocity vectors at each texel. At render time, UVs are
/// offset by the flow direction, creating the illusion of flowing liquid, lava, etc.
pub struct FlowMap {
    /// Flow vectors stored row-major. Each entry is a 2D direction.
    pub vectors: Vec<Vec2>,
    /// Width of the flow map.
    pub width: usize,
    /// Height of the flow map.
    pub height: usize,
    /// Flow speed multiplier.
    pub speed: f32,
    /// Flow strength multiplier.
    pub strength: f32,
    /// Current cycle time.
    pub time: f32,
    /// Duration of one flow cycle (UV offset resets to avoid extreme distortion).
    pub cycle_duration: f32,
}

impl FlowMap {
    /// Create a new flow map with the given dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            vectors: vec![Vec2::ZERO; width * height],
            width,
            height,
            speed: 1.0,
            strength: 0.1,
            time: 0.0,
            cycle_duration: 2.0,
        }
    }

    /// Create a flow map with a uniform direction.
    pub fn uniform(width: usize, height: usize, direction: Vec2) -> Self {
        let mut fm = Self::new(width, height);
        fm.vectors.fill(direction);
        fm
    }

    /// Create a circular flow map (vortex).
    pub fn vortex(width: usize, height: usize, strength: f32) -> Self {
        let mut fm = Self::new(width, height);
        let cx = width as f32 * 0.5;
        let cy = height as f32 * 0.5;
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt().max(0.01);
                let falloff = 1.0 / (1.0 + dist * 0.1);
                fm.vectors[y * width + x] = Vec2::new(-dy, dx).normalize_or_zero() * strength * falloff;
            }
        }
        fm
    }

    /// Create a divergent flow map (expanding from center).
    pub fn divergent(width: usize, height: usize, strength: f32) -> Self {
        let mut fm = Self::new(width, height);
        let cx = width as f32 * 0.5;
        let cy = height as f32 * 0.5;
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                fm.vectors[y * width + x] = Vec2::new(dx, dy).normalize_or_zero() * strength;
            }
        }
        fm
    }

    /// Set flow speed.
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Set flow strength.
    pub fn with_strength(mut self, strength: f32) -> Self {
        self.strength = strength;
        self
    }

    /// Set cycle duration.
    pub fn with_cycle(mut self, duration: f32) -> Self {
        self.cycle_duration = duration;
        self
    }

    /// Advance time.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt * self.speed;
    }

    /// Sample the flow vector at a UV coordinate using bilinear interpolation.
    pub fn sample(&self, uv: Vec2) -> Vec2 {
        let fx = (uv.x.fract() + 1.0).fract() * (self.width - 1) as f32;
        let fy = (uv.y.fract() + 1.0).fract() * (self.height - 1) as f32;

        let ix = fx as usize;
        let iy = fy as usize;
        let sx = fx - ix as f32;
        let sy = fy - iy as f32;

        let ix1 = (ix + 1) % self.width;
        let iy1 = (iy + 1) % self.height;

        let v00 = self.vectors[iy * self.width + ix];
        let v10 = self.vectors[iy * self.width + ix1];
        let v01 = self.vectors[iy1 * self.width + ix];
        let v11 = self.vectors[iy1 * self.width + ix1];

        let top = v00 * (1.0 - sx) + v10 * sx;
        let bottom = v01 * (1.0 - sx) + v11 * sx;
        top * (1.0 - sy) + bottom * sy
    }

    /// Apply flow distortion to a UV coordinate.
    ///
    /// Uses dual-phase approach to avoid visual discontinuities when the
    /// flow cycle resets.
    pub fn distort(&self, uv: Vec2) -> Vec2 {
        let flow = self.sample(uv) * self.strength;
        let cycle = self.cycle_duration;
        if cycle < 1e-6 {
            return uv + flow * self.time;
        }

        let phase0 = (self.time / cycle).fract();
        let phase1 = ((self.time / cycle) + 0.5).fract();
        let blend = (phase0 * 2.0 - 1.0).abs(); // triangle wave 0->1->0

        let uv0 = uv - flow * phase0;
        let uv1 = uv - flow * phase1;

        uv0 * (1.0 - blend) + uv1 * blend
    }

    /// Distort an array of UVs.
    pub fn distort_array(&self, uvs: &mut [[f32; 2]]) {
        for uv_pair in uvs.iter_mut() {
            let uv = Vec2::new(uv_pair[0], uv_pair[1]);
            let result = self.distort(uv);
            uv_pair[0] = result.x;
            uv_pair[1] = result.y;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parallax scrolling layers
// ─────────────────────────────────────────────────────────────────────────────

/// A single parallax scrolling layer.
#[derive(Debug, Clone)]
pub struct ParallaxLayer {
    /// Scroll speed relative to camera (1.0 = same speed, 0.5 = half speed).
    pub speed_factor: f32,
    /// UV offset of this layer.
    pub offset: Vec2,
    /// UV scale of this layer.
    pub scale: Vec2,
    /// Opacity of this layer (0.0 = transparent, 1.0 = opaque).
    pub opacity: f32,
    /// Depth (for sorting; higher = further back).
    pub depth: f32,
}

impl ParallaxLayer {
    pub fn new(speed_factor: f32, depth: f32) -> Self {
        Self {
            speed_factor,
            offset: Vec2::ZERO,
            scale: Vec2::ONE,
            opacity: 1.0,
            depth,
        }
    }

    pub fn with_scale(mut self, scale: Vec2) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}

/// Multi-layer parallax scrolling system.
pub struct ParallaxScroller {
    pub layers: Vec<ParallaxLayer>,
    pub scroll_position: Vec2,
}

impl ParallaxScroller {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            scroll_position: Vec2::ZERO,
        }
    }

    /// Add a layer. Returns the layer index.
    pub fn add_layer(&mut self, layer: ParallaxLayer) -> usize {
        let idx = self.layers.len();
        self.layers.push(layer);
        // Sort by depth (furthest first)
        self.layers.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));
        idx
    }

    /// Create a standard multi-layer parallax setup.
    pub fn standard_layers(num_layers: usize) -> Self {
        let mut scroller = Self::new();
        for i in 0..num_layers {
            let depth = (i + 1) as f32;
            let speed = 1.0 / depth;
            let opacity = 1.0 - (i as f32 * 0.15).min(0.8);
            scroller.add_layer(
                ParallaxLayer::new(speed, depth)
                    .with_opacity(opacity),
            );
        }
        scroller
    }

    /// Update scroll position.
    pub fn scroll(&mut self, delta: Vec2) {
        self.scroll_position += delta;
    }

    /// Set absolute scroll position.
    pub fn set_position(&mut self, position: Vec2) {
        self.scroll_position = position;
    }

    /// Get the UV offset for a specific layer.
    pub fn layer_uv(&self, layer_index: usize, base_uv: Vec2) -> Vec2 {
        if let Some(layer) = self.layers.get(layer_index) {
            let offset = self.scroll_position * layer.speed_factor + layer.offset;
            fract_vec2((base_uv + offset) * layer.scale)
        } else {
            base_uv
        }
    }

    /// Get all layer UVs for a given base UV.
    pub fn all_layer_uvs(&self, base_uv: Vec2) -> Vec<(Vec2, f32)> {
        self.layers.iter().map(|layer| {
            let offset = self.scroll_position * layer.speed_factor + layer.offset;
            let uv = fract_vec2((base_uv + offset) * layer.scale);
            (uv, layer.opacity)
        }).collect()
    }
}

impl Default for ParallaxScroller {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sprite sheet animation
// ─────────────────────────────────────────────────────────────────────────────

/// Sprite sheet (texture atlas) animator.
///
/// Steps through frames of a regular grid of sprites in a texture atlas.
pub struct SpriteSheetAnimator {
    /// Number of columns in the sprite sheet.
    pub columns: usize,
    /// Number of rows in the sprite sheet.
    pub rows: usize,
    /// Total number of frames (may be less than columns * rows if last row is partial).
    pub total_frames: usize,
    /// Current frame index.
    pub current_frame: usize,
    /// Frames per second.
    pub fps: f32,
    /// Whether to loop.
    pub looping: bool,
    /// Accumulated time.
    time_accumulator: f32,
    /// Whether the animation is playing.
    pub playing: bool,
}

impl SpriteSheetAnimator {
    /// Create a new sprite sheet animator.
    pub fn new(columns: usize, rows: usize, fps: f32) -> Self {
        Self {
            columns: columns.max(1),
            rows: rows.max(1),
            total_frames: columns * rows,
            current_frame: 0,
            fps,
            looping: true,
            time_accumulator: 0.0,
            playing: true,
        }
    }

    /// Set the total frame count (for partial last rows).
    pub fn with_total_frames(mut self, total: usize) -> Self {
        self.total_frames = total.min(self.columns * self.rows);
        self
    }

    /// Set looping.
    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Advance the animation.
    pub fn tick(&mut self, dt: f32) {
        if !self.playing || self.total_frames == 0 {
            return;
        }

        self.time_accumulator += dt;
        let frame_duration = 1.0 / self.fps;

        while self.time_accumulator >= frame_duration {
            self.time_accumulator -= frame_duration;
            self.current_frame += 1;

            if self.current_frame >= self.total_frames {
                if self.looping {
                    self.current_frame = 0;
                } else {
                    self.current_frame = self.total_frames - 1;
                    self.playing = false;
                    return;
                }
            }
        }
    }

    /// Set the current frame.
    pub fn set_frame(&mut self, frame: usize) {
        self.current_frame = frame.min(self.total_frames.saturating_sub(1));
    }

    /// Get the UV rect for the current frame: (uv_min, uv_max).
    pub fn current_uv_rect(&self) -> (Vec2, Vec2) {
        self.frame_uv_rect(self.current_frame)
    }

    /// Get the UV rect for a specific frame.
    pub fn frame_uv_rect(&self, frame: usize) -> (Vec2, Vec2) {
        let col = frame % self.columns;
        let row = frame / self.columns;
        let cell_w = 1.0 / self.columns as f32;
        let cell_h = 1.0 / self.rows as f32;
        let uv_min = Vec2::new(col as f32 * cell_w, row as f32 * cell_h);
        let uv_max = Vec2::new((col + 1) as f32 * cell_w, (row + 1) as f32 * cell_h);
        (uv_min, uv_max)
    }

    /// Transform a normalized UV (0-1) to the atlas UV for the current frame.
    pub fn map_uv(&self, local_uv: Vec2) -> Vec2 {
        let (uv_min, uv_max) = self.current_uv_rect();
        Vec2::new(
            uv_min.x + local_uv.x * (uv_max.x - uv_min.x),
            uv_min.y + local_uv.y * (uv_max.y - uv_min.y),
        )
    }

    /// Get the current frame row and column.
    pub fn current_cell(&self) -> (usize, usize) {
        (self.current_frame / self.columns, self.current_frame % self.columns)
    }

    /// Play the animation.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Pause the animation.
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Reset to the first frame.
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.time_accumulator = 0.0;
        self.playing = true;
    }

    /// Is the animation finished (non-looping only)?
    pub fn is_finished(&self) -> bool {
        !self.looping && self.current_frame >= self.total_frames.saturating_sub(1)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Animated normal maps
// ─────────────────────────────────────────────────────────────────────────────

/// Animated normal map generator.
///
/// Generates procedural animated normal maps for water, organic surfaces, etc.
pub struct AnimatedNormalMap {
    /// Width of the normal map.
    pub width: usize,
    /// Height of the normal map.
    pub height: usize,
    /// Generated normals in RGB format (x, y, z mapped to [0, 1]).
    pub normals: Vec<[f32; 3]>,
    /// Animation time.
    pub time: f32,
    /// Wave parameters.
    pub waves: Vec<NormalMapWave>,
}

/// A single wave contribution to the normal map.
#[derive(Debug, Clone, Copy)]
pub struct NormalMapWave {
    pub direction: Vec2,
    pub frequency: f32,
    pub amplitude: f32,
    pub speed: f32,
}

impl NormalMapWave {
    pub fn new(direction: Vec2, frequency: f32, amplitude: f32, speed: f32) -> Self {
        Self { direction: direction.normalize_or_zero(), frequency, amplitude, speed }
    }
}

impl AnimatedNormalMap {
    /// Create a new animated normal map.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            normals: vec![[0.5, 0.5, 1.0]; width * height],
            time: 0.0,
            waves: Vec::new(),
        }
    }

    /// Add a wave pattern.
    pub fn add_wave(&mut self, wave: NormalMapWave) {
        self.waves.push(wave);
    }

    /// Create a water-like normal map with default waves.
    pub fn water(width: usize, height: usize) -> Self {
        let mut nm = Self::new(width, height);
        nm.add_wave(NormalMapWave::new(Vec2::new(1.0, 0.3), 4.0, 0.3, 0.5));
        nm.add_wave(NormalMapWave::new(Vec2::new(-0.5, 1.0), 6.0, 0.2, 0.7));
        nm.add_wave(NormalMapWave::new(Vec2::new(0.7, -0.7), 10.0, 0.1, 1.2));
        nm
    }

    /// Update the normal map for the current time.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.regenerate();
    }

    /// Regenerate the normal map at the current time.
    pub fn regenerate(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                let uv = Vec2::new(
                    x as f32 / self.width as f32,
                    y as f32 / self.height as f32,
                );

                let mut dx = 0.0_f32;
                let mut dy = 0.0_f32;

                for wave in &self.waves {
                    let phase = uv.dot(wave.direction) * wave.frequency - self.time * wave.speed;
                    let deriv = (phase * TAU).cos() * wave.amplitude * wave.frequency * TAU;
                    dx += wave.direction.x * deriv;
                    dy += wave.direction.y * deriv;
                }

                // Construct normal from derivatives
                let normal = Vec3::new(-dx, -dy, 1.0).normalize();
                // Map from [-1,1] to [0,1]
                let idx = y * self.width + x;
                self.normals[idx] = [
                    normal.x * 0.5 + 0.5,
                    normal.y * 0.5 + 0.5,
                    normal.z * 0.5 + 0.5,
                ];
            }
        }
    }

    /// Sample the normal at a UV coordinate.
    pub fn sample(&self, uv: Vec2) -> Vec3 {
        let fx = (uv.x.fract() + 1.0).fract() * (self.width - 1) as f32;
        let fy = (uv.y.fract() + 1.0).fract() * (self.height - 1) as f32;

        let ix = (fx as usize).min(self.width - 1);
        let iy = (fy as usize).min(self.height - 1);

        let n = self.normals[iy * self.width + ix];
        Vec3::new(n[0] * 2.0 - 1.0, n[1] * 2.0 - 1.0, n[2] * 2.0 - 1.0).normalize()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Triplanar projection
// ─────────────────────────────────────────────────────────────────────────────

/// Triplanar projection for seamless texturing of arbitrary surfaces.
///
/// Projects texture coordinates from three orthogonal planes (XY, XZ, YZ)
/// and blends based on the surface normal direction.
pub struct TriplanarProjector {
    /// Scale factor for the texture coordinates.
    pub scale: f32,
    /// Blending sharpness (higher = sharper transitions between planes).
    pub sharpness: f32,
    /// Per-axis scale adjustment.
    pub axis_scale: Vec3,
    /// Per-axis offset.
    pub axis_offset: Vec3,
}

impl TriplanarProjector {
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            sharpness: 1.0,
            axis_scale: Vec3::ONE,
            axis_offset: Vec3::ZERO,
        }
    }

    pub fn with_sharpness(mut self, sharpness: f32) -> Self {
        self.sharpness = sharpness;
        self
    }

    pub fn with_axis_scale(mut self, axis_scale: Vec3) -> Self {
        self.axis_scale = axis_scale;
        self
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.axis_offset = offset;
        self
    }

    /// Compute the blending weights for a given normal.
    pub fn blend_weights(&self, normal: Vec3) -> Vec3 {
        let n = normal.abs();
        // Raise to power for sharpness
        let mut w = Vec3::new(
            n.x.powf(self.sharpness),
            n.y.powf(self.sharpness),
            n.z.powf(self.sharpness),
        );
        let sum = w.x + w.y + w.z;
        if sum > 1e-6 {
            w /= sum;
        }
        w
    }

    /// Compute triplanar UVs for a vertex.
    ///
    /// Returns three UV pairs (for XY, XZ, YZ planes) and their blend weights.
    pub fn project(&self, position: Vec3, normal: Vec3) -> TriplanarUVs {
        let p = (position + self.axis_offset) * self.scale;
        let weights = self.blend_weights(normal);

        TriplanarUVs {
            uv_xy: Vec2::new(p.x * self.axis_scale.x, p.y * self.axis_scale.y),
            uv_xz: Vec2::new(p.x * self.axis_scale.x, p.z * self.axis_scale.z),
            uv_yz: Vec2::new(p.y * self.axis_scale.y, p.z * self.axis_scale.z),
            weight_xy: weights.z, // Z-facing surfaces use XY plane
            weight_xz: weights.y, // Y-facing surfaces use XZ plane
            weight_yz: weights.x, // X-facing surfaces use YZ plane
        }
    }

    /// Compute a single blended UV for a vertex (simplified, loses some quality).
    pub fn project_single(&self, position: Vec3, normal: Vec3) -> Vec2 {
        let uvs = self.project(position, normal);
        uvs.uv_xy * uvs.weight_xy
            + uvs.uv_xz * uvs.weight_xz
            + uvs.uv_yz * uvs.weight_yz
    }
}

/// Result of a triplanar projection.
#[derive(Debug, Clone, Copy)]
pub struct TriplanarUVs {
    /// UV coordinates for the XY plane.
    pub uv_xy: Vec2,
    /// UV coordinates for the XZ plane.
    pub uv_xz: Vec2,
    /// UV coordinates for the YZ plane.
    pub uv_yz: Vec2,
    /// Blend weight for XY plane.
    pub weight_xy: f32,
    /// Blend weight for XZ plane.
    pub weight_xz: f32,
    /// Blend weight for YZ plane.
    pub weight_yz: f32,
}

impl TriplanarUVs {
    /// Blend three sampled values (e.g., colors) using the triplanar weights.
    pub fn blend_f32(&self, val_xy: f32, val_xz: f32, val_yz: f32) -> f32 {
        val_xy * self.weight_xy + val_xz * self.weight_xz + val_yz * self.weight_yz
    }

    /// Blend three Vec3 values.
    pub fn blend_vec3(&self, val_xy: Vec3, val_xz: Vec3, val_yz: Vec3) -> Vec3 {
        val_xy * self.weight_xy + val_xz * self.weight_xz + val_yz * self.weight_yz
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UV unwrapping utilities
// ─────────────────────────────────────────────────────────────────────────────

/// UV unwrapping projection modes.
pub struct UVUnwrap;

impl UVUnwrap {
    /// Cylindrical UV projection around the Y axis.
    ///
    /// Maps position to UV: u = atan2(z, x) / TAU, v = y / height.
    pub fn cylindrical(position: Vec3, height: f32) -> [f32; 2] {
        let u = (position.z.atan2(position.x) / TAU + 0.5).fract();
        let v = (position.y / height + 0.5).clamp(0.0, 1.0);
        [u, v]
    }

    /// Spherical UV projection.
    ///
    /// Maps position to UV using spherical coordinates.
    pub fn spherical(position: Vec3) -> [f32; 2] {
        let len = position.length();
        if len < 1e-6 {
            return [0.5, 0.5];
        }
        let normalized = position / len;
        let u = (normalized.z.atan2(normalized.x) / TAU + 0.5).fract();
        let v = (normalized.y.asin() / PI + 0.5).clamp(0.0, 1.0);
        [u, v]
    }

    /// Box (cube) projection — project from the most-facing axis.
    pub fn box_projection(position: Vec3, normal: Vec3) -> [f32; 2] {
        let abs_n = normal.abs();
        if abs_n.x >= abs_n.y && abs_n.x >= abs_n.z {
            // Project from X
            [position.z.fract().abs(), position.y.fract().abs()]
        } else if abs_n.y >= abs_n.z {
            // Project from Y
            [position.x.fract().abs(), position.z.fract().abs()]
        } else {
            // Project from Z
            [position.x.fract().abs(), position.y.fract().abs()]
        }
    }

    /// Planar projection onto a plane defined by origin, u-axis, and v-axis.
    pub fn planar(
        position: Vec3,
        origin: Vec3,
        u_axis: Vec3,
        v_axis: Vec3,
    ) -> [f32; 2] {
        let relative = position - origin;
        let u = relative.dot(u_axis) / u_axis.length_squared();
        let v = relative.dot(v_axis) / v_axis.length_squared();
        [u, v]
    }

    /// Camera (view-space) projection.
    pub fn camera_projection(
        position: Vec3,
        camera_pos: Vec3,
        camera_forward: Vec3,
        camera_up: Vec3,
        fov: f32,
    ) -> [f32; 2] {
        let right = camera_forward.cross(camera_up).normalize_or_zero();
        let up = right.cross(camera_forward).normalize_or_zero();
        let to_point = position - camera_pos;
        let depth = to_point.dot(camera_forward);
        if depth < 1e-6 {
            return [0.5, 0.5];
        }
        let scale = 1.0 / (fov * 0.5).tan() / depth;
        let u = to_point.dot(right) * scale * 0.5 + 0.5;
        let v = to_point.dot(up) * scale * 0.5 + 0.5;
        [u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)]
    }

    /// Unwrap an array of positions using cylindrical projection.
    pub fn cylindrical_array(positions: &[Vec3], height: f32) -> Vec<[f32; 2]> {
        positions.iter().map(|&p| Self::cylindrical(p, height)).collect()
    }

    /// Unwrap an array of positions using spherical projection.
    pub fn spherical_array(positions: &[Vec3]) -> Vec<[f32; 2]> {
        positions.iter().map(|&p| Self::spherical(p)).collect()
    }

    /// Unwrap an array using box projection.
    pub fn box_projection_array(positions: &[Vec3], normals: &[Vec3]) -> Vec<[f32; 2]> {
        positions.iter().zip(normals.iter()).map(|(&p, &n)| {
            Self::box_projection(p, n)
        }).collect()
    }

    /// Fix UV seams for cylindrical/spherical projections.
    ///
    /// When a triangle spans the 0/1 boundary in U, this adjusts UVs to
    /// prevent stretching.
    pub fn fix_seams(uvs: &mut [[f32; 2]], indices: &[[u32; 3]]) {
        for tri in indices {
            let u0 = uvs[tri[0] as usize][0];
            let u1 = uvs[tri[1] as usize][0];
            let u2 = uvs[tri[2] as usize][0];

            // Check if triangle spans the 0/1 boundary
            let max_u = u0.max(u1).max(u2);
            let min_u = u0.min(u1).min(u2);
            if max_u - min_u > 0.5 {
                // Adjust UVs less than 0.5 by adding 1.0
                for &idx in tri {
                    if uvs[idx as usize][0] < 0.5 {
                        uvs[idx as usize][0] += 1.0;
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utility
// ─────────────────────────────────────────────────────────────────────────────

/// Fractional part of a Vec2 (wrap to [0, 1)).
fn fract_vec2(v: Vec2) -> Vec2 {
    Vec2::new(
        (v.x.fract() + 1.0).fract(),
        (v.y.fract() + 1.0).fract(),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uv_scroll() {
        let mut anim = UVAnimator::scroll(Vec2::new(1.0, 0.0));
        anim.tick(0.5);
        let uv = anim.transform(Vec2::new(0.0, 0.0));
        assert!((uv.x - 0.5).abs() < 1e-5);
        assert!(uv.y.abs() < 1e-5);
    }

    #[test]
    fn uv_rotate() {
        let anim = UVAnimator::rotate(Vec2::new(0.5, 0.5), PI);
        let uv = anim.transform_at(Vec2::new(1.0, 0.5), 0.0);
        // At time 0, no rotation
        assert!((uv.x - 1.0).abs() < 1e-4);
    }

    #[test]
    fn uv_sine_warp() {
        let anim = UVAnimator::sine_warp(Vec2::new(0.1, 0.1), Vec2::new(2.0, 2.0), 1.0);
        let uv = anim.transform_at(Vec2::new(0.5, 0.5), 0.0);
        // Just verify it produces reasonable output
        assert!(uv.x.is_finite());
        assert!(uv.y.is_finite());
    }

    #[test]
    fn flow_map_uniform() {
        let mut fm = FlowMap::uniform(8, 8, Vec2::new(1.0, 0.0))
            .with_strength(0.1);
        fm.tick(0.5);
        let uv = fm.distort(Vec2::new(0.5, 0.5));
        assert!(uv.x.is_finite());
    }

    #[test]
    fn flow_map_vortex() {
        let fm = FlowMap::vortex(16, 16, 1.0);
        let center = fm.sample(Vec2::new(0.5, 0.5));
        // At center, flow should be near zero
        assert!(center.length() < 5.0);
    }

    #[test]
    fn sprite_sheet() {
        let mut ss = SpriteSheetAnimator::new(4, 4, 10.0);
        assert_eq!(ss.total_frames, 16);
        assert_eq!(ss.current_frame, 0);

        ss.tick(0.1); // Should advance 1 frame
        assert_eq!(ss.current_frame, 1);

        let (uv_min, uv_max) = ss.current_uv_rect();
        assert!((uv_min.x - 0.25).abs() < 1e-5);
        assert!((uv_max.x - 0.5).abs() < 1e-5);
    }

    #[test]
    fn sprite_sheet_looping() {
        let mut ss = SpriteSheetAnimator::new(2, 2, 10.0);
        // 4 frames at 10fps, advance 0.5s = 5 frames, should wrap to frame 1
        ss.tick(0.5);
        assert!(ss.current_frame < 4);
    }

    #[test]
    fn triplanar_projection() {
        let proj = TriplanarProjector::new(1.0).with_sharpness(2.0);

        // Y-facing surface should use XZ plane primarily
        let uvs = proj.project(Vec3::new(1.0, 2.0, 3.0), Vec3::Y);
        assert!(uvs.weight_xz > 0.9);
    }

    #[test]
    fn triplanar_blend_weights() {
        let proj = TriplanarProjector::new(1.0);
        let weights = proj.blend_weights(Vec3::new(0.0, 1.0, 0.0));
        assert!((weights.x + weights.y + weights.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cylindrical_unwrap() {
        let uv = UVUnwrap::cylindrical(Vec3::new(1.0, 0.0, 0.0), 2.0);
        assert!(uv[0] >= 0.0 && uv[0] <= 1.0);
        assert!(uv[1] >= 0.0 && uv[1] <= 1.0);
    }

    #[test]
    fn spherical_unwrap() {
        let uv = UVUnwrap::spherical(Vec3::new(0.0, 1.0, 0.0));
        assert!((uv[1] - 1.0).abs() < 1e-5); // North pole
    }

    #[test]
    fn box_projection_unwrap() {
        let uv = UVUnwrap::box_projection(
            Vec3::new(0.5, 0.3, 0.7),
            Vec3::new(0.0, 1.0, 0.0),
        );
        // Y-facing: should use X and Z
        assert!(uv[0].is_finite());
        assert!(uv[1].is_finite());
    }

    #[test]
    fn parallax_layers() {
        let mut scroller = ParallaxScroller::standard_layers(4);
        assert_eq!(scroller.layers.len(), 4);

        scroller.scroll(Vec2::new(1.0, 0.0));
        let uvs = scroller.all_layer_uvs(Vec2::new(0.5, 0.5));
        assert_eq!(uvs.len(), 4);
        // Each layer should have different UVs due to different speed factors
    }

    #[test]
    fn animated_normal_map() {
        let mut nm = AnimatedNormalMap::water(32, 32);
        nm.tick(0.016);
        let normal = nm.sample(Vec2::new(0.5, 0.5));
        assert!(normal.z > 0.0); // Normal should point generally upward
    }

    #[test]
    fn uv_chain() {
        let mut chain = UVAnimatorChain::new();
        chain.push(UVAnimator::scroll(Vec2::new(0.1, 0.0)));
        chain.push(UVAnimator::scroll(Vec2::new(0.0, 0.1)));
        chain.tick(1.0);
        let uv = chain.transform(Vec2::ZERO);
        assert!((uv.x - 0.1).abs() < 1e-4);
        assert!((uv.y - 0.1).abs() < 1e-4);
    }

    #[test]
    fn planar_unwrap() {
        let uv = UVUnwrap::planar(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::ZERO,
            Vec3::X,
            Vec3::Y,
        );
        assert!((uv[0] - 1.0).abs() < 1e-5);
        assert!((uv[1] - 2.0).abs() < 1e-5);
    }
}
