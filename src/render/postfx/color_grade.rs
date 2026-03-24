//! Color grading pass — full CPU/GPU color pipeline.
//!
//! Implements a professional-grade color grading system including:
//! - Per-channel curves (lift/gamma/gain)
//! - Saturation, contrast, brightness, hue rotation
//! - Split toning (shadows/highlights)
//! - Color look-up tables (3D LUT, 17³ = 4913 entries)
//! - Cinematic film looks (ACES, Kodak, Fuji, Noir, etc.)
//! - Vignette with user-controlled shape
//! - Animated grade transitions via keyframes
//! - Color grading presets for common game states

use glam::{Vec3, Vec4};

// ── ColorGradeParams ──────────────────────────────────────────────────────────

/// Full color grading parameters for one frame.
#[derive(Clone, Debug)]
pub struct ColorGradeParams {
    pub enabled: bool,

    // ── Global adjustments ────────────────────────────────────────────────────
    /// Overall tint multiplied onto the final image (RGB, 1.0 = neutral).
    pub tint: Vec3,
    /// Saturation multiplier (1.0 = normal, 0.0 = greyscale, >1 = oversaturated).
    pub saturation: f32,
    /// Contrast multiplier (1.0 = normal).
    pub contrast: f32,
    /// Brightness offset (0.0 = normal, range -1..1).
    pub brightness: f32,
    /// Hue rotation in degrees (0.0 = none).
    pub hue_shift: f32,

    // ── Lift/Gamma/Gain ───────────────────────────────────────────────────────
    /// Shadow color offset (RGB, 0.0 = neutral). Applied in dark regions.
    pub lift: Vec3,
    /// Midtone gamma correction (RGB, 1.0 = neutral). Applied via power function.
    pub gamma: Vec3,
    /// Highlight multiplier (RGB, 1.0 = neutral).
    pub gain: Vec3,

    // ── Split toning ──────────────────────────────────────────────────────────
    /// Shadow tint color (applied to dark areas).
    pub shadow_tint: Vec3,
    /// Highlight tint color (applied to bright areas).
    pub highlight_tint: Vec3,
    /// How much shadow tint to apply (0.0 = none, 1.0 = full).
    pub shadow_tint_strength: f32,
    /// How much highlight tint to apply.
    pub highlight_tint_strength: f32,
    /// Luminance threshold separating shadows from highlights.
    pub split_midpoint: f32,

    // ── Curves ────────────────────────────────────────────────────────────────
    /// Per-channel luminance S-curve strength (0.0 = linear, 1.0 = full S).
    pub curve_strength: f32,
    /// Independent S-curve strengths for R, G, B channels.
    pub channel_curves: Vec3,

    // ── Vignette ──────────────────────────────────────────────────────────────
    /// Vignette strength (0.0 = none, 1.0 = full black edges).
    pub vignette: f32,
    /// Vignette feather (0.0 = hard, 1.0 = smooth).
    pub vignette_feather: f32,
    /// Vignette roundness (1.0 = circle, 0.0 = rectangle).
    pub vignette_roundness: f32,
    /// Vignette color (default black).
    pub vignette_color: Vec3,

    // ── LUT ───────────────────────────────────────────────────────────────────
    /// Optional 3D LUT. If present, applied after all other grading.
    pub lut: Option<ColorLut>,
    /// How much to blend the LUT result with the non-LUT result (0.0-1.0).
    pub lut_strength: f32,

    // ── Film look ─────────────────────────────────────────────────────────────
    pub film_look: FilmLook,
    /// Strength of the selected film look (0.0 = none, 1.0 = full).
    pub film_look_strength: f32,
}

impl Default for ColorGradeParams {
    fn default() -> Self {
        Self {
            enabled:                 true,
            tint:                    Vec3::ONE,
            saturation:              1.0,
            contrast:                1.0,
            brightness:              0.0,
            hue_shift:               0.0,
            lift:                    Vec3::ZERO,
            gamma:                   Vec3::ONE,
            gain:                    Vec3::ONE,
            shadow_tint:             Vec3::ZERO,
            highlight_tint:          Vec3::ZERO,
            shadow_tint_strength:    0.0,
            highlight_tint_strength: 0.0,
            split_midpoint:          0.5,
            curve_strength:          0.0,
            channel_curves:          Vec3::ZERO,
            vignette:                0.15,
            vignette_feather:        0.5,
            vignette_roundness:      0.8,
            vignette_color:          Vec3::ZERO,
            lut:                     None,
            lut_strength:            1.0,
            film_look:               FilmLook::None,
            film_look_strength:      1.0,
        }
    }
}

impl ColorGradeParams {
    // ── Presets ────────────────────────────────────────────────────────────────

    /// Neutral — no grading applied.
    pub fn neutral() -> Self { Self::default() }

    /// Red-tinted grade for hit flash effects.
    pub fn hit_flash(intensity: f32) -> Self {
        Self {
            tint:       Vec3::new(1.0 + intensity * 0.5, 0.8 - intensity * 0.2, 0.8 - intensity * 0.2),
            saturation: 1.3,
            contrast:   1.1,
            vignette:   0.15 + intensity * 0.4,
            shadow_tint: Vec3::new(0.4, 0.0, 0.0),
            shadow_tint_strength: intensity * 0.3,
            ..Default::default()
        }
    }

    /// Desaturated grade for death/game over sequence.
    pub fn death(progress: f32) -> Self {
        Self {
            saturation: 1.0 - progress * 0.85,
            brightness: -progress * 0.3,
            contrast:   1.0 + progress * 0.2,
            vignette:   0.15 + progress * 0.7,
            tint:       Vec3::new(0.8, 0.7, 0.7),
            lift:       Vec3::splat(-progress * 0.05),
            film_look:  FilmLook::Noir,
            film_look_strength: progress * 0.6,
            ..Default::default()
        }
    }

    /// Warm golden grade for victory/level completion.
    pub fn victory() -> Self {
        Self {
            tint:                    Vec3::new(1.15, 1.05, 0.85),
            saturation:              1.3,
            brightness:              0.08,
            highlight_tint:          Vec3::new(1.0, 0.9, 0.5),
            highlight_tint_strength: 0.4,
            film_look:               FilmLook::Golden,
            film_look_strength:      0.5,
            ..Default::default()
        }
    }

    /// Cold blue for low-health / danger state.
    pub fn danger(severity: f32) -> Self {
        Self {
            tint:          Vec3::new(0.9 - severity * 0.2, 0.9, 1.1 + severity * 0.1),
            saturation:    1.0 - severity * 0.3,
            contrast:      1.0 + severity * 0.15,
            vignette:      0.2 + severity * 0.5,
            shadow_tint:   Vec3::new(0.0, 0.05, 0.2),
            shadow_tint_strength: severity * 0.4,
            ..Default::default()
        }
    }

    /// Retro CRT / scanline aesthetic.
    pub fn retro_crt() -> Self {
        Self {
            saturation:   1.4,
            contrast:     1.2,
            brightness:   -0.05,
            film_look:    FilmLook::RetroTv,
            film_look_strength: 0.8,
            vignette:     0.3,
            vignette_feather: 0.3,
            tint:         Vec3::new(0.95, 1.05, 0.9),
            ..Default::default()
        }
    }

    /// Dreamy soft look (healing, sanctuary).
    pub fn dream() -> Self {
        Self {
            saturation:              1.2,
            contrast:                0.85,
            brightness:              0.1,
            highlight_tint:          Vec3::new(1.0, 0.95, 1.1),
            highlight_tint_strength: 0.3,
            shadow_tint:             Vec3::new(0.1, 0.1, 0.3),
            shadow_tint_strength:    0.2,
            curve_strength:          0.3,
            film_look:               FilmLook::Soft,
            film_look_strength:      0.6,
            ..Default::default()
        }
    }

    /// Poison/acid aesthetic.
    pub fn poison(intensity: f32) -> Self {
        Self {
            tint:          Vec3::new(0.8, 1.1 + intensity * 0.3, 0.7),
            saturation:    1.5,
            contrast:      1.1,
            shadow_tint:   Vec3::new(0.0, 0.3, 0.0),
            shadow_tint_strength: intensity * 0.4,
            vignette:      0.2 + intensity * 0.2,
            vignette_color: Vec3::new(0.0, 0.3, 0.0),
            ..Default::default()
        }
    }

    /// Lerp between two color grades.
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            enabled:                 a.enabled || b.enabled,
            tint:                    Vec3::lerp(a.tint, b.tint, t),
            saturation:              lerp_f32(a.saturation, b.saturation, t),
            contrast:                lerp_f32(a.contrast, b.contrast, t),
            brightness:              lerp_f32(a.brightness, b.brightness, t),
            hue_shift:               lerp_f32(a.hue_shift, b.hue_shift, t),
            lift:                    Vec3::lerp(a.lift, b.lift, t),
            gamma:                   Vec3::lerp(a.gamma, b.gamma, t),
            gain:                    Vec3::lerp(a.gain, b.gain, t),
            shadow_tint:             Vec3::lerp(a.shadow_tint, b.shadow_tint, t),
            highlight_tint:          Vec3::lerp(a.highlight_tint, b.highlight_tint, t),
            shadow_tint_strength:    lerp_f32(a.shadow_tint_strength, b.shadow_tint_strength, t),
            highlight_tint_strength: lerp_f32(a.highlight_tint_strength, b.highlight_tint_strength, t),
            split_midpoint:          lerp_f32(a.split_midpoint, b.split_midpoint, t),
            curve_strength:          lerp_f32(a.curve_strength, b.curve_strength, t),
            channel_curves:          Vec3::lerp(a.channel_curves, b.channel_curves, t),
            vignette:                lerp_f32(a.vignette, b.vignette, t),
            vignette_feather:        lerp_f32(a.vignette_feather, b.vignette_feather, t),
            vignette_roundness:      lerp_f32(a.vignette_roundness, b.vignette_roundness, t),
            vignette_color:          Vec3::lerp(a.vignette_color, b.vignette_color, t),
            lut:                     if t < 0.5 { a.lut.clone() } else { b.lut.clone() },
            lut_strength:            lerp_f32(a.lut_strength, b.lut_strength, t),
            film_look:               if t < 0.5 { a.film_look } else { b.film_look },
            film_look_strength:      lerp_f32(a.film_look_strength, b.film_look_strength, t),
        }
    }

    // ── CPU pixel processing ───────────────────────────────────────────────────

    /// Apply the full color grade to a single pixel (linear float RGB).
    /// Returns a graded linear RGB value.
    pub fn apply_to_pixel(&self, pixel: Vec3) -> Vec3 {
        if !self.enabled { return pixel; }

        let mut c = pixel;

        // 1. Brightness
        c += Vec3::splat(self.brightness);

        // 2. Contrast (around 0.5)
        c = (c - 0.5) * self.contrast + 0.5;

        // 3. Lift / Gamma / Gain
        c = c + self.lift;
        c = Vec3::new(
            pow_f32(c.x.max(0.0), 1.0 / self.gamma.x.max(0.001)),
            pow_f32(c.y.max(0.0), 1.0 / self.gamma.y.max(0.001)),
            pow_f32(c.z.max(0.0), 1.0 / self.gamma.z.max(0.001)),
        );
        c *= self.gain;

        // 4. Tint
        c *= self.tint;

        // 5. Saturation (via luminance)
        let luma = luminance(c);
        c = Vec3::splat(luma) + (c - Vec3::splat(luma)) * self.saturation;

        // 6. Hue rotation
        if self.hue_shift.abs() > 0.001 {
            c = rotate_hue(c, self.hue_shift);
        }

        // 7. S-curve
        if self.curve_strength > 0.001 {
            c = s_curve_v3(c, self.curve_strength);
        }

        // 8. Per-channel curves
        if self.channel_curves.length_squared() > 0.001 {
            c.x = apply_channel_curve(c.x, self.channel_curves.x);
            c.y = apply_channel_curve(c.y, self.channel_curves.y);
            c.z = apply_channel_curve(c.z, self.channel_curves.z);
        }

        // 9. Split toning
        if self.shadow_tint_strength > 0.001 || self.highlight_tint_strength > 0.001 {
            let luma2 = luminance(c);
            let shadow_w    = (1.0 - luma2 / self.split_midpoint.max(0.001)).clamp(0.0, 1.0);
            let highlight_w = (luma2 - self.split_midpoint).max(0.0)
                            / (1.0 - self.split_midpoint).max(0.001);
            let highlight_w = highlight_w.clamp(0.0, 1.0);
            c = Vec3::lerp(c, c * (Vec3::ONE + self.shadow_tint),
                           shadow_w * self.shadow_tint_strength);
            c = Vec3::lerp(c, c * (Vec3::ONE + self.highlight_tint),
                           highlight_w * self.highlight_tint_strength);
        }

        // 10. LUT
        if let Some(ref lut) = self.lut {
            let lut_out = lut.sample(c);
            c = Vec3::lerp(c, lut_out, self.lut_strength);
        }

        // 11. Film look
        if self.film_look_strength > 0.001 {
            let film_out = self.film_look.apply(c);
            c = Vec3::lerp(c, film_out, self.film_look_strength);
        }

        c.max(Vec3::ZERO)
    }

    /// Apply vignette at normalized UV position `(u, v) ∈ [0, 1]²`.
    /// Returns a multiplier in `[0, 1]` to apply to the pixel.
    pub fn vignette_at(&self, u: f32, v: f32) -> f32 {
        if self.vignette < 0.001 { return 1.0; }
        let dx = (u - 0.5) * 2.0;
        let dy = (v - 0.5) * 2.0;
        // Roundness blends between L∞ (rect) and L2 (circle)
        let r = lerp_f32(
            dx.abs().max(dy.abs()),
            (dx * dx + dy * dy).sqrt(),
            self.vignette_roundness,
        );
        let feather = self.vignette_feather.max(0.001);
        let edge_dist = ((r - (1.0 - self.vignette)) / feather).clamp(0.0, 1.0);
        1.0 - edge_dist * edge_dist * (3.0 - 2.0 * edge_dist) // smoothstep
    }

    /// Full pixel apply including vignette at UV.
    pub fn apply_full(&self, pixel: Vec3, u: f32, v: f32) -> Vec3 {
        let graded  = self.apply_to_pixel(pixel);
        let vignette_m = self.vignette_at(u, v);
        let dark    = Vec3::lerp(graded, self.vignette_color, 1.0 - vignette_m);
        dark
    }

    /// Apply grading to an entire image buffer (width × height × 3 linear f32).
    pub fn apply_to_image(&self, pixels: &mut [f32], width: usize, height: usize) {
        let inv_w = 1.0 / width.max(1) as f32;
        let inv_h = 1.0 / height.max(1) as f32;
        for y in 0..height {
            for x in 0..width {
                let base = (y * width + x) * 3;
                if base + 2 >= pixels.len() { break; }
                let c = Vec3::new(pixels[base], pixels[base + 1], pixels[base + 2]);
                let u = (x as f32 + 0.5) * inv_w;
                let v = (y as f32 + 0.5) * inv_h;
                let out = self.apply_full(c, u, v);
                pixels[base    ] = out.x;
                pixels[base + 1] = out.y;
                pixels[base + 2] = out.z;
            }
        }
    }
}

// ── FilmLook ──────────────────────────────────────────────────────────────────

/// Cinematic film simulation presets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilmLook {
    None,
    /// ACES (Academy Color Encoding System) filmic tone curve.
    Aces,
    /// Kodak-inspired warm emulsion look.
    Kodak,
    /// Fuji-inspired cooler, slightly desaturated look.
    Fuji,
    /// High contrast black-and-white noir.
    Noir,
    /// Warm golden summer look.
    Golden,
    /// Soft pastel dreamlike look.
    Soft,
    /// Retro CRT / VHS look with slight color bleeding.
    RetroTv,
    /// Faded film (lifted blacks, reduced contrast).
    FadedFilm,
    /// Teal-and-orange (Hollywood blockbuster style).
    TealOrange,
}

impl FilmLook {
    pub fn apply(self, c: Vec3) -> Vec3 {
        match self {
            FilmLook::None     => c,
            FilmLook::Aces     => aces_filmic(c),
            FilmLook::Kodak    => kodak_look(c),
            FilmLook::Fuji     => fuji_look(c),
            FilmLook::Noir     => noir_look(c),
            FilmLook::Golden   => golden_look(c),
            FilmLook::Soft     => soft_look(c),
            FilmLook::RetroTv  => retro_tv_look(c),
            FilmLook::FadedFilm => faded_film_look(c),
            FilmLook::TealOrange => teal_orange_look(c),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            FilmLook::None      => "None",
            FilmLook::Aces      => "ACES",
            FilmLook::Kodak     => "Kodak",
            FilmLook::Fuji      => "Fuji",
            FilmLook::Noir      => "Noir",
            FilmLook::Golden    => "Golden",
            FilmLook::Soft      => "Soft",
            FilmLook::RetroTv   => "Retro TV",
            FilmLook::FadedFilm => "Faded Film",
            FilmLook::TealOrange => "Teal & Orange",
        }
    }
}

// ACES filmic tone mapping approximation (Hill 2016)
fn aces_filmic(c: Vec3) -> Vec3 {
    let a = 2.51_f32;
    let b = 0.03_f32;
    let cc = 2.43_f32;
    let d = 0.59_f32;
    let e = 0.14_f32;
    let x = c;
    ((x * (a * x + b)) / (x * (cc * x + d) + e)).clamp(Vec3::ZERO, Vec3::ONE)
}

fn kodak_look(c: Vec3) -> Vec3 {
    // Warm, slightly lifted shadows, slightly rolled-off highlights
    let lifted = c * 0.93 + Vec3::new(0.02, 0.015, 0.01);
    let warm = lifted * Vec3::new(1.08, 1.02, 0.95);
    soft_knee_compress(warm, 0.85, 0.1)
}

fn fuji_look(c: Vec3) -> Vec3 {
    let cooler = c * Vec3::new(0.97, 1.0, 1.06);
    let slight_desat = {
        let luma = luminance(cooler);
        Vec3::lerp(Vec3::splat(luma), cooler, 0.88)
    };
    soft_knee_compress(slight_desat, 0.9, 0.08)
}

fn noir_look(c: Vec3) -> Vec3 {
    let luma = luminance(c);
    // Full desaturation + S-curve for high contrast
    let grey = Vec3::splat(luma);
    let contrasted = s_curve_v3(grey, 0.7);
    contrasted * Vec3::new(0.95, 0.92, 0.9) // slight warm tint in greys
}

fn golden_look(c: Vec3) -> Vec3 {
    let warm = c * Vec3::new(1.15, 1.05, 0.82);
    // Compress highlights
    soft_knee_compress(warm, 0.88, 0.1)
}

fn soft_look(c: Vec3) -> Vec3 {
    // Reduce contrast, add slight glow in highlights
    let soft_c = (c - 0.5) * 0.8 + 0.5;
    let bloom  = (c - Vec3::splat(0.7)).max(Vec3::ZERO) * 0.3;
    soft_c + bloom
}

fn retro_tv_look(c: Vec3) -> Vec3 {
    // Slight color bleeding: G channel bleeds into R, B fades
    let r = c.x * 0.9 + c.y * 0.1;
    let g = c.y;
    let b = c.z * 0.85;
    // Higher saturation
    let bleed = Vec3::new(r, g, b);
    let luma  = luminance(bleed);
    Vec3::lerp(Vec3::splat(luma), bleed, 1.4)
}

fn faded_film_look(c: Vec3) -> Vec3 {
    // Lift blacks, reduce whites
    c * 0.85 + Vec3::splat(0.06)
}

fn teal_orange_look(c: Vec3) -> Vec3 {
    let luma = luminance(c);
    let shadow_w    = (1.0 - luma).clamp(0.0, 1.0).powi(2);
    let highlight_w = luma.clamp(0.0, 1.0).powi(2);
    // Shadows → teal
    let teal_shadows = Vec3::lerp(c, Vec3::new(0.3, 0.7, 0.8), shadow_w * 0.35);
    // Highlights → orange
    Vec3::lerp(teal_shadows, Vec3::new(1.1, 0.7, 0.4), highlight_w * 0.35)
}

// ── 3D Color LUT ──────────────────────────────────────────────────────────────

/// 3D color look-up table (17³ entries = 4913 triplets).
#[derive(Clone, Debug)]
pub struct ColorLut {
    pub size: usize,
    /// Flat array of RGB values: `[r, g, b, r, g, b, ...]` with `size³` entries.
    pub entries: Vec<Vec3>,
}

impl ColorLut {
    /// Create a neutral (identity) LUT of given size (typically 17 or 33).
    pub fn identity(size: usize) -> Self {
        let n = size * size * size;
        let mut entries = Vec::with_capacity(n);
        let inv = (size - 1) as f32;
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    entries.push(Vec3::new(r as f32 / inv, g as f32 / inv, b as f32 / inv));
                }
            }
        }
        Self { size, entries }
    }

    /// Build a LUT by applying a color grade to the identity LUT.
    pub fn from_grade(grade: &ColorGradeParams, size: usize) -> Self {
        let mut lut = Self::identity(size);
        for e in &mut lut.entries {
            *e = grade.apply_to_pixel(*e);
        }
        lut
    }

    /// Trilinear interpolation sample from the LUT.
    pub fn sample(&self, color: Vec3) -> Vec3 {
        let n = self.size;
        let s = (n - 1) as f32;
        let cr = (color.x * s).clamp(0.0, s);
        let cg = (color.y * s).clamp(0.0, s);
        let cb = (color.z * s).clamp(0.0, s);
        let r0 = cr.floor() as usize;
        let g0 = cg.floor() as usize;
        let b0 = cb.floor() as usize;
        let r1 = (r0 + 1).min(n - 1);
        let g1 = (g0 + 1).min(n - 1);
        let b1 = (b0 + 1).min(n - 1);
        let tr = cr.fract();
        let tg = cg.fract();
        let tb = cb.fract();

        let idx = |r: usize, g: usize, b: usize| b * n * n + g * n + r;

        let c000 = self.entries[idx(r0, g0, b0)];
        let c100 = self.entries[idx(r1, g0, b0)];
        let c010 = self.entries[idx(r0, g1, b0)];
        let c110 = self.entries[idx(r1, g1, b0)];
        let c001 = self.entries[idx(r0, g0, b1)];
        let c101 = self.entries[idx(r1, g0, b1)];
        let c011 = self.entries[idx(r0, g1, b1)];
        let c111 = self.entries[idx(r1, g1, b1)];

        let c00 = Vec3::lerp(c000, c100, tr);
        let c01 = Vec3::lerp(c001, c101, tr);
        let c10 = Vec3::lerp(c010, c110, tr);
        let c11 = Vec3::lerp(c011, c111, tr);
        let c0  = Vec3::lerp(c00, c10, tg);
        let c1  = Vec3::lerp(c01, c11, tg);
        Vec3::lerp(c0, c1, tb)
    }

    /// Serialize LUT to .cube format string for external tools.
    pub fn to_cube_string(&self, title: &str) -> String {
        let mut out = format!("TITLE \"{}\"\nLUT_3D_SIZE {}\n", title, self.size);
        for e in &self.entries {
            out.push_str(&format!("{:.6} {:.6} {:.6}\n", e.x, e.y, e.z));
        }
        out
    }
}

// ── ColorGradeKeyframe ────────────────────────────────────────────────────────

/// An animated keyframe for a color grade transition.
#[derive(Clone, Debug)]
pub struct ColorGradeKeyframe {
    pub time:   f32,
    pub grade:  ColorGradeParams,
    /// Easing: 0 = linear, 1 = smooth, 2 = ease-in, 3 = ease-out.
    pub easing: u8,
}

/// Animated color grader — holds a sequence of keyframes and evaluates smoothly.
#[derive(Clone, Debug)]
pub struct AnimatedColorGrade {
    pub keyframes: Vec<ColorGradeKeyframe>,
}

impl AnimatedColorGrade {
    pub fn new() -> Self { Self { keyframes: Vec::new() } }

    pub fn add(mut self, time: f32, grade: ColorGradeParams) -> Self {
        self.keyframes.push(ColorGradeKeyframe { time, grade, easing: 1 });
        self.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        self
    }

    pub fn add_eased(mut self, time: f32, grade: ColorGradeParams, easing: u8) -> Self {
        self.keyframes.push(ColorGradeKeyframe { time, grade, easing });
        self.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        self
    }

    /// Evaluate the grade at a given time.
    pub fn evaluate(&self, t: f32) -> ColorGradeParams {
        if self.keyframes.is_empty() { return ColorGradeParams::default(); }
        if self.keyframes.len() == 1 { return self.keyframes[0].grade.clone(); }
        if t <= self.keyframes[0].time { return self.keyframes[0].grade.clone(); }
        let last = self.keyframes.last().unwrap();
        if t >= last.time { return last.grade.clone(); }

        // Binary search for surrounding keyframes
        let i = self.keyframes.partition_point(|k| k.time <= t) - 1;
        let k0 = &self.keyframes[i];
        let k1 = &self.keyframes[i + 1];
        let span = k1.time - k0.time;
        let raw_t = if span < 1e-6 { 0.0 } else { (t - k0.time) / span };
        let et = ease(raw_t, k0.easing);
        ColorGradeParams::lerp(&k0.grade, &k1.grade, et)
    }
}

fn ease(t: f32, mode: u8) -> f32 {
    match mode {
        0 => t,
        1 => t * t * (3.0 - 2.0 * t), // smoothstep
        2 => t * t,                      // ease-in
        3 => t * (2.0 - t),              // ease-out
        _ => t,
    }
}

// ── Utility functions ─────────────────────────────────────────────────────────

#[inline]
fn luminance(c: Vec3) -> f32 {
    c.x * 0.2126 + c.y * 0.7152 + c.z * 0.0722
}

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

#[inline]
fn pow_f32(base: f32, exp: f32) -> f32 {
    if base <= 0.0 { 0.0 } else { base.powf(exp) }
}

fn rotate_hue(c: Vec3, degrees: f32) -> Vec3 {
    let angle = degrees.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    // Rotate in RGB space using a hue rotation matrix
    let sqrt3 = 3.0_f32.sqrt();
    let r = c.x * (cos_a + (1.0 - cos_a) / 3.0) + c.y * ((1.0 - cos_a) / 3.0 - sin_a / sqrt3)
          + c.z * ((1.0 - cos_a) / 3.0 + sin_a / sqrt3);
    let g = c.x * ((1.0 - cos_a) / 3.0 + sin_a / sqrt3) + c.y * (cos_a + (1.0 - cos_a) / 3.0)
          + c.z * ((1.0 - cos_a) / 3.0 - sin_a / sqrt3);
    let b = c.x * ((1.0 - cos_a) / 3.0 - sin_a / sqrt3) + c.y * ((1.0 - cos_a) / 3.0 + sin_a / sqrt3)
          + c.z * (cos_a + (1.0 - cos_a) / 3.0);
    Vec3::new(r, g, b).max(Vec3::ZERO)
}

fn s_curve_v3(c: Vec3, strength: f32) -> Vec3 {
    Vec3::new(
        s_curve(c.x, strength),
        s_curve(c.y, strength),
        s_curve(c.z, strength),
    )
}

fn s_curve(x: f32, strength: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let curved = x * x * (3.0 - 2.0 * x); // smoothstep
    lerp_f32(x, curved, strength)
}

fn apply_channel_curve(x: f32, strength: f32) -> f32 {
    // Soft S-curve with adjustable strength per channel
    s_curve(x, strength)
}

fn soft_knee_compress(c: Vec3, knee: f32, width: f32) -> Vec3 {
    Vec3::new(
        soft_knee_channel(c.x, knee, width),
        soft_knee_channel(c.y, knee, width),
        soft_knee_channel(c.z, knee, width),
    )
}

fn soft_knee_channel(x: f32, knee: f32, width: f32) -> f32 {
    if x <= knee - width {
        x
    } else if x >= knee + width {
        knee + (x - knee) * 0.1 // strong compression above knee
    } else {
        let t = (x - (knee - width)) / (2.0 * width);
        let blend = t * t * (3.0 - 2.0 * t);
        let compressed = knee + (x - knee) * 0.1;
        lerp_f32(x, compressed, blend)
    }
}

// ── GLSL shader source ────────────────────────────────────────────────────────

/// GLSL fragment shader implementing the color grade pass.
pub const COLOR_GRADE_FRAG: &str = r#"
#version 330 core
in  vec2 vUv;
out vec4 fColor;

uniform sampler2D uScene;
uniform float uSaturation;
uniform float uContrast;
uniform float uBrightness;
uniform float uHueShift;
uniform vec3  uTint;
uniform vec3  uLift;
uniform vec3  uGamma;
uniform vec3  uGain;
uniform float uVignette;
uniform float uVignetteFeather;
uniform float uVignetteRoundness;

float luminance(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }

vec3 saturation(vec3 c, float s) {
    float luma = luminance(c);
    return mix(vec3(luma), c, s);
}

vec3 adjustContrast(vec3 c, float contrast) {
    return (c - 0.5) * contrast + 0.5;
}

void main() {
    vec3 c = texture(uScene, vUv).rgb;

    // Brightness / Contrast
    c += uBrightness;
    c = adjustContrast(c, uContrast);

    // Lift / Gamma / Gain
    c = c + uLift;
    c = pow(max(c, 0.0), 1.0 / max(uGamma, vec3(0.001)));
    c *= uGain;

    // Tint + Saturation
    c *= uTint;
    c = saturation(c, uSaturation);

    // Vignette
    vec2 uv = vUv * 2.0 - 1.0;
    float r = mix(max(abs(uv.x), abs(uv.y)), length(uv), uVignetteRoundness);
    float vig = 1.0 - smoothstep(1.0 - uVignette, 1.0 - uVignette + uVignetteFeather, r);
    c *= vig;

    fColor = vec4(clamp(c, 0.0, 1.0), 1.0);
}
"#;

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_grade() {
        let grade = ColorGradeParams::neutral();
        let pixel = Vec3::new(0.5, 0.3, 0.7);
        let out   = grade.apply_to_pixel(pixel);
        // Should be approximately unchanged
        assert!((out.x - pixel.x).abs() < 0.05);
        assert!((out.y - pixel.y).abs() < 0.05);
        assert!((out.z - pixel.z).abs() < 0.05);
    }

    #[test]
    fn test_greyscale() {
        let mut grade = ColorGradeParams::neutral();
        grade.saturation = 0.0;
        let pixel = Vec3::new(1.0, 0.0, 0.0);
        let out = grade.apply_to_pixel(pixel);
        // All channels should be equal (greyscale)
        assert!((out.x - out.y).abs() < 0.01);
        assert!((out.y - out.z).abs() < 0.01);
    }

    #[test]
    fn test_vignette_center() {
        let grade = ColorGradeParams::default();
        let v = grade.vignette_at(0.5, 0.5);
        assert!(v > 0.99); // center should be fully lit
    }

    #[test]
    fn test_vignette_corner() {
        let grade = ColorGradeParams::default();
        let v = grade.vignette_at(0.0, 0.0);
        assert!(v < 0.9); // corner should be dimmed
    }

    #[test]
    fn test_lerp() {
        let a = ColorGradeParams::neutral();
        let b = ColorGradeParams::death(1.0);
        let mid = ColorGradeParams::lerp(&a, &b, 0.5);
        assert!((mid.saturation - 0.5 * (1.0 + (1.0 - 0.85))).abs() < 0.1);
    }

    #[test]
    fn test_lut_identity() {
        let lut = ColorLut::identity(17);
        let c = Vec3::new(0.4, 0.6, 0.8);
        let out = lut.sample(c);
        assert!((out.x - c.x).abs() < 0.01);
        assert!((out.y - c.y).abs() < 0.01);
        assert!((out.z - c.z).abs() < 0.01);
    }

    #[test]
    fn test_lut_from_grade() {
        let grade = ColorGradeParams::death(0.5);
        let lut = ColorLut::from_grade(&grade, 9);
        assert_eq!(lut.entries.len(), 9 * 9 * 9);
    }

    #[test]
    fn test_film_looks_dont_panic() {
        let pixel = Vec3::new(0.3, 0.5, 0.7);
        for look in [FilmLook::Aces, FilmLook::Kodak, FilmLook::Fuji, FilmLook::Noir,
                     FilmLook::Golden, FilmLook::Soft, FilmLook::RetroTv,
                     FilmLook::FadedFilm, FilmLook::TealOrange] {
            let out = look.apply(pixel);
            assert!(out.x.is_finite());
            assert!(out.y.is_finite());
            assert!(out.z.is_finite());
        }
    }

    #[test]
    fn test_animated_grade() {
        let anim = AnimatedColorGrade::new()
            .add(0.0, ColorGradeParams::neutral())
            .add(1.0, ColorGradeParams::death(1.0));
        let mid = anim.evaluate(0.5);
        assert!((mid.saturation - 0.5 * (1.0 + 0.15)).abs() < 0.3);
        assert_eq!(anim.evaluate(0.0).brightness, 0.0);
    }

    #[test]
    fn test_image_processing() {
        let mut pixels = vec![0.5_f32, 0.5, 0.5, 0.3, 0.6, 0.9];
        let grade = ColorGradeParams::neutral();
        grade.apply_to_image(&mut pixels, 2, 1);
        // Should not panic or produce NaN
        for p in &pixels { assert!(p.is_finite()); }
    }

    #[test]
    fn test_cube_export() {
        let lut = ColorLut::identity(3);
        let cube = lut.to_cube_string("Test LUT");
        assert!(cube.contains("LUT_3D_SIZE 3"));
        assert!(cube.contains("TITLE"));
    }
}
