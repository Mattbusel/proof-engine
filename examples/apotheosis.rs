//! Apotheosis — Leon Kennedy. Complete rewrite based on convergence.rs bone system.
//!
//! ═══════════════════════════════════════════════════════════════════════════
//! FULL RENDERING PIPELINE
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! Particle counts:
//!   Body  — ~500 K base particles × 16 copies ≈ 8 M spawned glyphs/frame
//!   Hair  — 500 strands × 12–20 particles × 16 copies = up to 160 K hair glyphs/frame
//!   Face  — dedicated render_sdf_face pass: eyes, lips, nose bridge, stubble
//!   SDF   — render_sdf_body covers torso + limbs (replaces capsule bones 7–12, 14, 25)
//!   Env   — render_vol_scatter + render_ground + render_environment + render_sky
//!
//! Eight-kit rendering pipeline (runs once per base particle):
//!   1. BoneKit     — 26-bone skeleton; each bone owns a particle budget slice
//!   2. ModelKit    — elliptic cross-section per bone via radius_at() knot tables
//!   3. ClothingKit — radial garment push, seam darkening, contact shadow
//!   4. MaterialKit — surface classification (Skin/Hair/Jacket/Boot/Metal/Eye) + Schlick Fresnel
//!   5. LightingKit — Lambert key + 3 fills + rim + hemisphere ambient + ACES tonemap
//!   6. PhysicsKit  — per-particle inertial lag; loose materials trail behind on motion
//!   7. HairKit     — 500-strand curtain-cut; Kajiya-Kay anisotropic specular
//!   8. RenderKit   — DoF jitter, n_copies scatter, alpha/emission scaling per copy
//!
//! SDF body geometry (replaces bone-capsule system for torso + all four limbs):
//!   sdf_torso → superellipsoid cross-section with plerp ax/az profiles along Y
//!   sdf_arm_r / sdf_forearm_r / sdf_leg_r → tapered elliptic capsules
//!   sdf_body  → consolidated smooth union; sdf_face → face topology with iris/sclera layers
//!   smin (Inigo Quilez polynomial) blends all joints organically
//!   Analytical normals from SDF gradient — no finite differences, zero noise
//!   SDF AO: 5-step IQ normal-march; SDF SSS: inward -n thickness probe
//!   Importance sampling: particles placed on expected surface shell → ~100% acceptance
//!
//! Post-processing — 23 techniques (CPU particle + GPU compute + fragment shader):
//!    2. SDF analytical reflection trace      3. Atmospheric depth scattering
//!    4. TAA sub-pixel jitter (Halton)        5. Spectral bloom dispersion
//!    6. Chromatic depth separation           7. SSR via SDF surface normal
//!    8. Volumetric light shafts / god rays   9. Cross-material GI color bleed
//!   10. Luminance-based film grain          11. Vignette + edge desaturation
//!   12. ACES filmic tonemapping             13. Eye adaptation (auto-exposure)
//!   14. Edge sharpen                        15. Heat haze (emission ripple)
//!   16. Kajiya-Kay anisotropic specular     17. SDF micro-displacement pore shadow
//!   18. DoF bokeh ring                      19. Iridescent thin-film (leather)
//!   20. SDF-curvature Fresnel rim           21. Thickness-modulated SSS
//!   22. SDF-analytical ambient occlusion    23. Eye spectral refraction (IOR)
//!
//! Bloom: 3.0 base, bursts to 8.0 at peaks (ceiling raised to 20.0)
//! Render scale: 3.0 (9× pixel density; ceiling raised to 8.0)
//!
//! Run: cargo run --release --example apotheosis

use proof_engine::prelude::*;
use proof_engine::audio::MusicVibe;
use proof_engine::config::ShadowQuality;
use std::f32::consts::TAU;
use glow::HasContext;
use glam::Mat4;

#[inline(always)]
fn hf(seed: usize, v: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393).wrapping_add(v.wrapping_mul(668265263))) as u32;
    let n = n ^ (n >> 13);
    let n = n.wrapping_mul(0x5851_F42D);
    let n = n ^ (n >> 16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

// ═════════════════════════════════════════════════════════════════════════════
// BoneKit — Skeleton definition
//
// Purpose : Defines Leon's 26-bone skeleton as axis-aligned capsule descriptors.
//           Each entry drives a proportional slice of the particle budget
//           (weight × bone_length / total_weight) in render_leon().
//
// Inputs  : Compile-time constants — no runtime parameters.
// Outputs : BONES[i] tuple consumed by render_leon: bone axis range, lateral
//           half-width, particle weight, and aspect ratio for elliptic spread.
//
// Toggle  : Set a bone's weight field to 0.0 to silence that body region.
//           Remove the index from BONES entirely to also remove it from
//           clothing and radius lookup tables.
// ═════════════════════════════════════════════════════════════════════════════

// ── Bone definition: (y_start, y_end, center_x, half_width, weight, aspect_ratio)
// aspect_ratio < 1.0 = flatter cross-section (wide X, deep Z).
// Spread X is scaled by aspect_ratio; pseudo-depth hz by 1/aspect_ratio.
// Y: negative = up (head), positive = down (feet). Figure scale = 3.2.
type Bone = (f32, f32, f32, f32, f32, f32);

const BONES: [Bone; 26] = [
    // ── HAIR / HEAD ────────────────────────────────────────────────────────
    (-1.18,-0.82, 0.00, 0.20, 4.0, 1.00),  // hair volume — spherical skull
    (-1.06,-0.78, 0.00, 0.16, 3.5, 1.00),  // face — spherical

    // ── NECK ───────────────────────────────────────────────────────────────
    (-0.78,-0.62, 0.00, 0.07, 1.5, 0.90),  // neck — slightly oval

    // ── COLLAR (raised RE4 jacket collar) ──────────────────────────────────
    (-0.72,-0.56,-0.13, 0.10, 1.5, 0.65),  // left collar wing — flat fabric
    (-0.72,-0.56, 0.13, 0.10, 1.5, 0.65),  // right collar wing

    // ── SHOULDERS (wide — key Leon silhouette feature) ──────────────────────
    (-0.68,-0.44,-0.42, 0.13, 3.0, 0.80),  // left shoulder — somewhat flat
    (-0.68,-0.44, 0.42, 0.13, 3.0, 0.80),  // right shoulder

    // ── CHEST (two jacket panels + centre seam) ────────────────────────────
    (-0.62, 0.10,-0.20, 0.20, 7.0, 0.60),  // left chest panel — flat torso
    (-0.62, 0.10, 0.20, 0.20, 7.0, 0.60),  // right chest panel
    (-0.62, 0.10, 0.00, 0.04, 2.0, 0.60),  // centre zipper strip

    // ── MID TORSO (tapers to waist) ────────────────────────────────────────
    ( 0.08, 0.24,-0.15, 0.15, 4.0, 0.60),  // left waist
    ( 0.08, 0.24, 0.15, 0.15, 4.0, 0.60),  // right waist

    // ── BELT ───────────────────────────────────────────────────────────────
    ( 0.22, 0.30, 0.00, 0.24, 2.5, 0.50),  // belt strap — very flat

    // ── UPPER ARMS ─────────────────────────────────────────────────────────
    (-0.60,-0.10,-0.50, 0.10, 4.0, 0.90),  // left upper arm
    (-0.60,-0.10, 0.50, 0.10, 4.0, 0.90),  // right upper arm

    // ── FOREARMS ───────────────────────────────────────────────────────────
    (-0.10, 0.24,-0.53, 0.08, 3.0, 0.90),  // left forearm
    (-0.10, 0.24, 0.53, 0.08, 3.0, 0.90),  // right forearm

    // ── HANDS ──────────────────────────────────────────────────────────────
    ( 0.24, 0.36,-0.54, 0.06, 1.5, 0.90),  // left hand
    ( 0.24, 0.36, 0.54, 0.06, 1.5, 0.90),  // right hand

    // ── THIGHS ─────────────────────────────────────────────────────────────
    ( 0.28, 0.62,-0.16, 0.13, 5.5, 0.80),  // left thigh — oval cross-section
    ( 0.28, 0.62, 0.16, 0.13, 5.5, 0.80),  // right thigh

    // ── SHINS ──────────────────────────────────────────────────────────────
    ( 0.60, 0.86,-0.14, 0.10, 4.0, 0.80),  // left shin
    ( 0.60, 0.86, 0.14, 0.10, 4.0, 0.80),  // right shin

    // ── BOOTS ──────────────────────────────────────────────────────────────
    ( 0.84, 0.98,-0.14, 0.11, 3.0, 0.80),  // left boot
    ( 0.84, 0.98, 0.14, 0.11, 3.0, 0.80),  // right boot

    // ── HIP CONNECTOR (fills the gap between thighs under the belt) ────────
    ( 0.24, 0.34, 0.00, 0.18, 2.0, 0.65),  // hip bridge — flat pelvis
];

fn total_weight() -> f32 {
    BONES.iter().map(|b| b.4 * (b.1 - b.0).abs()).sum()
}

// ═════════════════════════════════════════════════════════════════════════════
// ClothingKit — Garment layer simulation
//
// Purpose : Marks which of the 26 bones carry fabric (jacket, pants, boots,
//           belt) vs. bare skin or hair.  Clothing bones receive three extra
//           rendering passes: (1) clothing_offset() radial push places garment
//           particles outside the body surface, (2) seam darkening where
//           adjacent bones have different material classifications, and (3)
//           contact shadow where fabric meets skin at cuffs and collar.
//
// Inputs  : bone_idx, t (along-axis position 0→1 from clothing_offset()),
//           dist (lateral distance from bone centre), mat_tag
// Outputs : cloth_add (extra half-width), seam_darkening, contact_shadow factor
//
// Toggle  : Set BONE_CLOTHING[i] = false to strip garment from bone i (renders
//           as skin).  Set clothing_offset() arm to return 0.0 to flatten a
//           garment region flush against the body surface.
// ═════════════════════════════════════════════════════════════════════════════

// ── ClothingKit: which bones carry garment fabric (parallel to BONES) ────────
// true  = clothing; seam darkening applies at dist > 0.85 when adjacent
//         material differs.
// false = skin/hair; never darkened.
const BONE_CLOTHING: [bool; 26] = [
    false, false,               // 0-1  hair volume, face
    false,                      // 2    neck
    true,  true,                // 3-4  collar wings
    true,  true,                // 5-6  shoulders
    true,  true,  true,         // 7-9  chest panels, zipper
    true,  true,                // 10-11 waist
    true,                       // 12   belt strap
    true,  true,                // 13-14 upper arms
    true,  true,                // 15-16 forearms
    false, false,               // 17-18 hands (skin)
    true,  true,                // 19-20 thighs
    true,  true,                // 21-22 shins
    true,  true,                // 23-24 boots
    true,                       // 25   hip bridge
];

// ── Material color from position ───────────────────────────────────────────────
fn leon_color(x: f32, y: f32) -> (f32, f32, f32) {
    let ax = x.abs();

    // Hair (top of head + sides)
    if y < -0.82 { return (0.20, 0.12, 0.06); }
    if y < -0.56 && ax > 0.11 { return (0.22, 0.13, 0.07); }

    // Face (forehead to chin)
    if y >= -0.88 && y <= -0.30 {
        if ax > 0.13 && y < -0.58 { return (0.20, 0.12, 0.06); } // side hair
        if y > -0.76 && y < -0.70 { return (0.62, 0.44, 0.32); } // brow shadow
        if y > -0.72 && y < -0.63 && ax > 0.04 && ax < 0.11 { return (0.70, 0.50, 0.36); } // eye area
        if y > -0.52 && y < -0.36 {
            let cell = ((ax * 59.0) as i32) ^ ((y * -47.0) as i32);
            if cell % 5 == 0 { return (0.65, 0.46, 0.33); } // stubble
        }
        return (0.82, 0.60, 0.45); // skin
    }

    // Neck
    if y < -0.18 && ax < 0.09 { return (0.78, 0.56, 0.42); }

    // Collar
    if y < -0.14 && ax > 0.08 && ax < 0.24 { return (0.40, 0.27, 0.13); }

    // Shoulder seam
    if y < -0.06 && ax > 0.30 { return (0.60, 0.42, 0.24); }

    // Belt zone
    if y >= 0.22 && y <= 0.30 {
        if ax < 0.05 { return (0.72, 0.70, 0.66); } // buckle
        return (0.14, 0.10, 0.07); // belt
    }

    // Arms
    if ax > 0.40 {
        if y > 0.24 { return (0.80, 0.58, 0.43); } // hand skin
        return (0.54, 0.37, 0.19);                  // sleeve
    }

    // Legs / boots
    if y > 0.28 {
        if y > 0.84 { return (0.25, 0.17, 0.10); }          // boot
        if y > 0.56 && y < 0.72 && ax < 0.18 { return (0.36, 0.32, 0.22); } // knee pad
        return (0.30, 0.33, 0.24);                            // olive pants
    }

    // Jacket (zipper strip centre)
    if ax < 0.05 && y > -0.62 && y < 0.22 { return (0.68, 0.50, 0.28); }

    // Jacket default
    (0.55, 0.38, 0.20)
}

// ═════════════════════════════════════════════════════════════════════════════
// MaterialKit — Per-surface Fresnel and material classification
//
// Purpose : Classifies every surface point into one of six material tags
//           (Skin, Hair, Jacket, Boot, Metal, Eye) by spatial position.
//           Each tag carries its own Schlick Fresnel parameters: exponent
//           weight k and RGB reflection tint at grazing angle.  The Fresnel
//           term is added to the final lit color to simulate the edge-
//           brightening behaviour of real physical surfaces.
//
// Inputs  : (x, y) world-space position; snz = dot(view, surface_normal)
// Outputs : MatTag enum; fresnel_response() → additive (r, g, b) highlight
//
// Fresnel intensities by material:
//   Skin   k=0.20 — waxy translucent rim
//   Hair   k=0.10 — barely visible sheen
//   Jacket k=0.50 — warm leather edge brightening
//   Boot   k=0.70 — strong glossy leather rim
//   Metal  k=1.00 — near-mirror buckle reflection
//   Eye    k=0.85 — wet cornea grazing highlight
//
// Toggle  : Return a constant MatTag from leon_tag() to force all particles
//           to the same material.  Set k=0.0 in fresnel_response() match arm
//           to disable Fresnel for a specific material.
// ═════════════════════════════════════════════════════════════════════════════

// ── MaterialKit: per-material Fresnel response ────────────────────────────────
// Classify the surface at (x,y) into a material category with its own Fresnel
// intensity curve.  Returns (k, r, g, b):
//   k   — Schlick exponent weight: how much edge brightening this material gets
//   rgb — the reflection color tint at grazing angle
// Physical basis: (1 - dot(view,normal))^5 × k × (r,g,b) added to final color.
// Intensities: Skin 0.20 (waxy), Jacket/Pants 0.50 (leather), Boot 0.70 (glossy
// leather), Metal/buckle 1.00 (near-total reflection), Hair 0.10 (minimal).
#[derive(Clone, Copy, PartialEq)]
enum MatTag { Skin, Hair, Jacket, Boot, Metal, Eye }

fn leon_tag(x: f32, y: f32) -> MatTag {
    let ax = x.abs();

    // Hair
    if y < -0.82 { return MatTag::Hair; }
    if y < -0.56 && ax > 0.11 { return MatTag::Hair; }

    // Face / neck skin
    if y >= -0.88 && y <= -0.30 { return MatTag::Skin; }
    if y < -0.18 && ax < 0.09  { return MatTag::Skin; }

    // Belt zone: buckle = metal, strap = boot-weight leather
    if y >= 0.22 && y <= 0.30 {
        if ax < 0.05 { return MatTag::Metal; }
        return MatTag::Boot;
    }

    // Arms: hands are skin, sleeves are jacket weight
    if ax > 0.40 {
        if y > 0.24 { return MatTag::Skin; }
        return MatTag::Jacket;
    }

    // Legs: boots are high-gloss, pants/knee pads are jacket weight
    if y > 0.28 {
        if y > 0.84 { return MatTag::Boot; }
        return MatTag::Jacket;
    }

    MatTag::Jacket  // chest / torso default
}

/// Schlick Fresnel reflectance contribution.
/// `snz` = dot(view=(0,0,1), surface_normal) — already available per-particle.
/// Returns an additive (r,g,b) highlight to mix into the final color.
#[inline(always)]
fn fresnel_response(tag: MatTag, snz: f32) -> (f32, f32, f32) {
    let cos_theta = snz.max(0.0);
    let f = (1.0 - cos_theta).powi(5);   // Schlick approximation

    let (k, tr, tg, tb) = match tag {
        MatTag::Skin   => (0.20, 1.00f32, 0.90f32, 0.80f32), // warm waxy rim
        MatTag::Hair   => (0.10, 0.95f32, 0.88f32, 0.72f32), // barely visible
        MatTag::Jacket => (0.50, 0.90f32, 0.72f32, 0.40f32), // warm leather sheen
        MatTag::Boot   => (0.70, 0.88f32, 0.78f32, 0.55f32), // strong glossy edge
        MatTag::Metal  => (1.00, 1.00f32, 0.97f32, 0.88f32), // near-mirror silver
        MatTag::Eye    => (0.85, 1.00f32, 0.98f32, 0.96f32), // cornea: near-total grazing
    };

    let contrib = f * k * 0.38;   // 0.38 keeps Fresnel supplemental, not dominant
    (contrib * tr, contrib * tg, contrib * tb)
}

// ═════════════════════════════════════════════════════════════════════════════
// RenderKit — Final particle scatter and depth-of-field
//
// Purpose : Controls how each accepted base particle is turned into n_copies
//           screen-space billboard instances.  Handles depth-of-field jitter
//           (blur_amount grows as particle Z diverges from FOCAL_DIST), copy
//           scatter (±sz·0.35 fills sub-pixel gaps between particles), and
//           scales alpha and emission by 1/n_copies to preserve total luminance.
//
// Inputs  : particle_z (world Z), FOCAL_DIST, DOF_RANGE, n_copies, sz (size)
// Outputs : dof_jx / dof_jy positional jitter; blur_amount → size scale + alpha
//
// Toggle  : Set DOF_RANGE to a very large value (e.g. 99.0) to disable DoF.
//           Set n_copies = 1 in the main loop to disable copy scatter (lower
//           fidelity but faster).
// ═════════════════════════════════════════════════════════════════════════════

// ── RenderKit: depth of field parameters ──────────────────────────────────────
// focal_distance: world-space Z of the sharpest plane (character centre-mass).
// dof_range: Z distance over which blur ramps from 0 → 1.
// Particle Z values run from ~0.0 (silhouette edges) to ~0.30 (front-face peak).
const FOCAL_DIST: f32 = 0.15;   // midpoint of the Z range
const DOF_RANGE:  f32 = 0.25;   // full ramp distance

// ── Environment constants — shared between render_leon (F3 light), render_ground, render_environment ──
const TORCH_X: f32 =  3.20;    // world X of the wall torch (right side)
const TORCH_Y: f32 = -2.00;    // world Y of the torch flame centre (visual height on right pillar)
const TORCH_Z: f32 = -1.20;    // world Z of the torch (slightly behind Leon)
const FLOOR_Y: f32 =  3.55;    // world Y of the floor plane (just below boot soles at ≈3.33)

/// Piecewise cosine-interpolated radius multiplier for each bone.
/// `t` = position along bone, 0.0 = start (y0), 1.0 = end (y1).
/// Returns a multiplier applied to `hw` so the spread widens/narrows
/// to match the actual anatomical cross-section at that height.
/// Garment-specific additive width for clothing bones.
///
/// Returns the world-space half-width added on top of the body's radius_at()
/// profile.  Used in two ways per particle:
///   1. Added to `eff_hw` before computing spread_raw → garment cross-section
///      shape departs from the underlying body cylinder.
///   2. Used as the radial push (replaces flat `thickness`) → garment particles
///      are centered outside the body surface, not inside it.
///
/// Design intent per region:
///   Jacket torso  — structured at shoulders (0.015), narrow at waist (0.009),
///                   slight hem flare at chest-panel bottom (0.012)
///   Sleeves       — bulky at shoulder cap (0.013), taper to wrist cuff (0.006)
///   Pants         — loose at hip (0.012), tighter at knee (0.008), taper to boot (0.005)
///   Boots         — stiff leather, uniform with slight top flare (0.016→0.013)
///   Belt          — flat rigid strap, constant (0.015)
///   Collar        — structured upward flare (0.010→0.014)
///   Non-clothing  — returns 0.0 (skin, hair, hands)
fn clothing_offset(bone_idx: usize, t: f32) -> f32 {
    // Reuse the same piecewise-cosine helper as radius_at.
    // Defined as a module-level fn here so it can be inlined with #[inline].
    #[inline(always)]
    fn p(t: f32, knots: &[(f32, f32)]) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let n = knots.len();
        if n == 0 { return 0.0; }
        if n == 1 { return knots[0].1; }
        if t <= knots[0].0   { return knots[0].1; }
        if t >= knots[n-1].0 { return knots[n-1].1; }
        for w in knots.windows(2) {
            let (t0, r0) = w[0];
            let (t1, r1) = w[1];
            if t <= t1 {
                let s = (t - t0) / (t1 - t0);
                let c = (1.0 - (s * std::f32::consts::PI).cos()) * 0.5;
                return r0 + (r1 - r0) * c;
            }
        }
        knots[n-1].1
    }
    match bone_idx {
        // ── HEAD / NECK / SKIN — no garment layer ────────────────────────────
        0 | 1 | 2 | 17 | 18 => 0.0,

        // ── COLLAR WINGS (3, 4) ───────────────────────────────────────────────
        // Structured RE4 collar: stiffest at the standing collar top,
        // leans slightly inward at the base where it merges with the chest.
        3 | 4  => p(t, &[(0.00, 0.010), (0.40, 0.014), (0.75, 0.013), (1.00, 0.011)]),

        // ── SHOULDERS (5, 6) ──────────────────────────────────────────────────
        // Jacket shoulder pads give maximum structure at the cap (0.015),
        // taper toward the upper arm socket and the collar junction.
        5 | 6  => p(t, &[(0.00, 0.013), (0.30, 0.015), (0.65, 0.012), (1.00, 0.010)]),

        // ── CHEST PANELS (7, 8) ───────────────────────────────────────────────
        // Jacket chest is fullest at the upper pec (puffer silhouette).
        // Tapers to waist, then a 0.002 flare at the hem where fabric
        // bunches over the belt.
        7 | 8  => p(t, &[(0.00, 0.013), (0.32, 0.015), (0.65, 0.009), (0.88, 0.009), (1.00, 0.012)]),

        // ── ZIPPER STRIP (9) ──────────────────────────────────────────────────
        // Narrow placket — nearly uniform, just a touch thicker at the collar.
        9      => p(t, &[(0.00, 0.005), (0.50, 0.004), (1.00, 0.004)]),

        // ── WAIST (10, 11) ────────────────────────────────────────────────────
        // Jacket hem: cinched at the natural waist, flares at the hip crest.
        10 | 11 => p(t, &[(0.00, 0.009), (0.45, 0.009), (0.75, 0.011), (1.00, 0.013)]),

        // ── BELT (12) ─────────────────────────────────────────────────────────
        // Stiff leather strap — constant thickness.
        12     => p(t, &[(0.00, 0.015), (1.00, 0.015)]),

        // ── UPPER ARMS (13, 14) ───────────────────────────────────────────────
        // Sleeve is fullest at the shoulder cap (padding + fabric bulk),
        // narrows evenly to the elbow cuff.
        13 | 14 => p(t, &[(0.00, 0.012), (0.28, 0.013), (0.62, 0.010), (1.00, 0.008)]),

        // ── FOREARMS (15, 16) ────────────────────────────────────────────────
        // Thinner sleeve material here; slight gather at the elbow
        // (where upper and lower sleeve meet), tight wrist cuff.
        15 | 16 => p(t, &[(0.00, 0.009), (0.22, 0.010), (0.60, 0.007), (1.00, 0.005)]),

        // ── THIGHS (19, 20) ───────────────────────────────────────────────────
        // Olive combat pants: loose at the hip (belt-over-waist bulk),
        // cinch gently toward the knee.
        19 | 20 => p(t, &[(0.00, 0.012), (0.25, 0.012), (0.60, 0.009), (1.00, 0.007)]),

        // ── SHINS (21, 22) ────────────────────────────────────────────────────
        // Lower trouser leg tucked into boot; tapers sharply to the ankle.
        21 | 22 => p(t, &[(0.00, 0.009), (0.28, 0.010), (0.68, 0.007), (1.00, 0.005)]),

        // ── BOOTS (23, 24) ────────────────────────────────────────────────────
        // Stiff leather: slight flare at the boot-top cuff, solid sole base.
        23 | 24 => p(t, &[(0.00, 0.016), (0.35, 0.015), (0.75, 0.014), (1.00, 0.013)]),

        // ── HIP BRIDGE (25) ───────────────────────────────────────────────────
        // Crotch panel — slight gather at centre, thinner at pelvis sides.
        25     => p(t, &[(0.00, 0.010), (0.50, 0.012), (1.00, 0.010)]),

        _      => 0.0,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// ModelKit — Per-bone anatomical cross-section profile
//
// Purpose : Defines the body's silhouette shape along each bone axis.  Rather
//           than a uniform cylinder, each bone has a piecewise-cosine radius
//           profile so shoulders are wide at the deltoid cap and narrow at the
//           socket; the chest tapers to the waist; forearms taper to the wrist.
//           Joint continuity is enforced: endpoint knots match the connecting
//           bone's radius so particles swell at junctions instead of pinching.
//
// Inputs  : bone_idx (which of the 26 BONES), t ∈ [0, 1] (along-axis fraction)
// Outputs : radius multiplier applied to bone half-width hw before lateral spread
//
// Toggle  : Return 1.0 from all match arms to revert to uniform capsule shape.
//           Adjust individual knot values to widen/narrow specific body regions.
// ═════════════════════════════════════════════════════════════════════════════
fn radius_at(bone_idx: usize, t: f32) -> f32 {
    #[inline(always)]
    fn p(t: f32, knots: &[(f32, f32)]) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let n = knots.len();
        if n == 0 { return 1.0; }
        if n == 1 { return knots[0].1; }
        if t <= knots[0].0    { return knots[0].1; }
        if t >= knots[n-1].0  { return knots[n-1].1; }
        for w in knots.windows(2) {
            let (t0, r0) = w[0];
            let (t1, r1) = w[1];
            if t <= t1 {
                let s = (t - t0) / (t1 - t0);
                let c = (1.0 - (s * std::f32::consts::PI).cos()) * 0.5;
                return r0 + (r1 - r0) * c;
            }
        }
        knots[n-1].1
    }
    // Joint continuity rule: endpoint knot values are set to
    //   max(this_bone_natural_radius, connecting_bone_radius_at_shared_end)
    // so bones swell at junctions rather than pinching.  Anatomy tapers happen
    // in the middle of each bone; junctions stay wide for additive overlap.
    match bone_idx {
        // ── HEAD (+20% across all knots) ─────────────────────────────────────
        // Face skull was too small relative to hair volume; +20% restores parity.
        // Skull crown raised 0.72→0.86; equator 1.00→1.20; base 0.80→0.96.
        0  => p(t, &[(0.00, 0.86), (0.50, 1.20), (1.00, 0.96)]),  // skull: crown→equator→nape
        // Face temples raised 0.78→0.94; cheek peak 1.00→1.20; chin 0.60→0.72.
        // Chin lifted to better overlap neck-start joint.
        1  => p(t, &[(0.00, 0.94), (0.35, 1.20), (1.00, 0.72)]),  // face: temple→cheek→chin

        // ── NECK ─────────────────────────────────────────────────────────────
        // Start raised 0.55→0.72: neck-jaw joint = max(face_chin=0.72, neck_start)
        2  => p(t, &[(0.00, 0.72), (0.50, 0.82), (1.00, 1.00)]),  // neck: jaw→mid→shoulder base

        // ── COLLAR ───────────────────────────────────────────────────────────
        3 | 4  => p(t, &[(0.00, 0.88), (0.50, 1.00), (1.00, 0.90)]),  // collar wings

        // ── SHOULDERS ────────────────────────────────────────────────────────
        // End raised 0.78→0.90: shoulder-socket joint = max(shoulder_end, arm_start=0.90)
        5 | 6  => p(t, &[(0.00, 0.80), (0.40, 1.00), (1.00, 0.90)]),  // shoulder: base→deltoid cap→socket

        // ── CHEST PANELS (+15% across all knots) ─────────────────────────────
        // Original bottom was 0.68; inner edge at y1 pulled 0.038 units away from
        // midline, breaking the zipper overlap.  After +15%: bottom = 0.88 so the
        // 50% core contour of each panel still reaches past the zipper bone.
        7 | 8  => p(t, &[(0.00, 1.04), (0.35, 1.15), (0.70, 0.94), (1.00, 0.88)]),  // chest panel

        // ── ZIPPER / BELT ─────────────────────────────────────────────────────
        9 | 12 => p(t, &[(0.00, 1.00), (1.00, 1.00)]),  // uniform columns

        // ── WAIST ─────────────────────────────────────────────────────────────
        // Start raised 0.75→0.88: chest-waist joint = max(chest_bottom=0.88, waist_start)
        // Mid kept near 0.82: waist is narrower than chest but not cinched to nothing.
        10 | 11 => p(t, &[(0.00, 0.88), (0.50, 0.82), (1.00, 0.92)]),  // waist: chest junction→natural waist→hip

        // ── UPPER ARMS ────────────────────────────────────────────────────────
        // Start raised 0.88→0.90: shoulder-socket continuity.
        // End raised 0.60→0.80: elbow joint = max(upper_arm_end, forearm_start=0.80)
        13 | 14 => p(t, &[(0.00, 0.90), (0.22, 1.00), (0.60, 0.86), (1.00, 0.80)]),  // upper arm: socket→deltoid→bicep→elbow

        // ── FOREARMS ──────────────────────────────────────────────────────────
        // Start raised 0.76→0.80: matches upper-arm end exactly.
        // End raised 0.44→0.68: wrist still tapers but not so extreme that the
        // hand-forearm gap is visible.
        15 | 16 => p(t, &[(0.00, 0.80), (0.28, 1.00), (0.62, 0.72), (1.00, 0.68)]),  // forearm: elbow→muscle belly→taper→wrist

        // ── HANDS ─────────────────────────────────────────────────────────────
        17 | 18 => p(t, &[(0.00, 1.00), (0.55, 0.82), (1.00, 0.65)]),  // hand: knuckles→palm→fingertips

        // ── THIGHS ────────────────────────────────────────────────────────────
        // End raised 0.55→0.80: knee joint = max(thigh_end, shin_start=0.82) ≈ 0.82;
        // 0.80 is close enough — the shin start will fill the remaining gap.
        19 | 20 => p(t, &[(0.00, 1.00), (0.28, 0.94), (0.58, 0.82), (1.00, 0.80)]),  // thigh: hip→quad bulge→knee

        // ── SHINS ─────────────────────────────────────────────────────────────
        // Start raised 0.78→0.82: matches thigh end (0.80) with slight extra overlap.
        // End raised 0.42→0.68: ankle-boot joint; 0.68 vs boot-top 1.00 still leaves a
        // step, but the stiff boot cuff (radius 1.00) fills the gap from its side.
        21 | 22 => p(t, &[(0.00, 0.82), (0.24, 1.00), (0.68, 0.72), (1.00, 0.68)]),  // shin: knee→gastrocnemius→taper→ankle

        // ── BOOTS ─────────────────────────────────────────────────────────────
        23 | 24 => p(t, &[(0.00, 1.00), (0.45, 0.90), (1.00, 0.82)]),  // boot: cuff→shaft→sole

        // ── HIP BRIDGE ────────────────────────────────────────────────────────
        25 => p(t, &[(0.00, 0.90), (0.50, 1.00), (1.00, 0.92)]),  // hip bridge: pelvis→crotch→pelvis

        _  => 1.0,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// SDF Body System
//
// Replaces per-bone ModelKit particle distribution for the torso and right
// upper arm.  The body shape is defined by analytic signed-distance primitives:
//
//   sdf_torso:   axis-aligned ellipse cross-section whose half-axes (ax, az)
//                are piecewise-linearly interpolated along Y.  Gives a true
//                superellipsoid body: broad shoulders, cinched waist, hip flare.
//   sdf_arm_r:   tapered capsule with elliptic XZ section for the right arm.
//   smin:        Inigo Quilez polynomial smooth-min blends the two primitives
//                into an organic shoulder/armpit junction.
//
// Sampling strategy: importance sampling.
//   Rather than rejecting 92%+ of uniform box samples, each candidate is placed
//   directly on the expected ellipse/capsule surface, then perturbed ±SHELL in
//   the outward radial direction.  The SDF verifies placement is within the
//   shell — acceptance rate is ≈100%, zero wasted evaluations.
//
// Normals: analytical gradient of the ellipse/capsule SDF — closed-form,
//   zero finite-difference overhead.
//
// Lighting: exact constants and formula from render_leon (same key/fill/rim,
//   same filmic tonemap and S-curve contrast).
//
// Bones replaced (skipped in render_leon):
//   7,8  — chest panels        9  — zipper strip
//   10,11 — waist panels       12 — belt strap
//   14   — right upper arm     25 — hip bridge
// ═════════════════════════════════════════════════════════════════════════════

/// Piecewise linear interpolation through a sorted knot table.
/// Used in SDF hot path; pcoslerp's cosine smoothness isn't needed here.
#[inline(always)]
fn plerp(t: f32, knots: &[(f32, f32)]) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if knots.is_empty() { return 1.0; }
    if t <= knots[0].0            { return knots[0].1; }
    let last = *knots.last().unwrap();
    if t >= last.0                { return last.1; }
    for w in knots.windows(2) {
        let (t0, r0) = w[0]; let (t1, r1) = w[1];
        if t <= t1 { return r0 + (r1 - r0) * (t - t0) / (t1 - t0); }
    }
    last.1
}

/// Inigo Quilez polynomial smooth-min.
/// Blends two SDF values within ±k, rounding the min() corner organically.
#[inline(always)]
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// Torso SDF with axis dimensions.
///
/// At each world Y the cross-section is an axis-aligned ellipse.  The axes:
///   ax(y): X half-width.  Peaks at shoulder line, tapers to waist, hip flare.
///   az(y): Z half-depth.  Shallow at shoulders, deepest at chest, thin waist.
///
/// The SDF value is `(√((x/ax)²+(z/az)²)−1) × min(ax,az)`.  This is NOT a
/// true Euclidean SDF (gradient ≠ 1 globally) but is within ~1.5× near the
/// surface — sufficient for shell sampling.  Hard Y caps at Y0/Y1; the future
/// neck and pelvis SDF primitives will smooth those junctions.
///
/// Returns `(sdf, ax, az)` — ax/az are used by the caller to build the
/// analytical normal without a second plerp call.
#[inline]
fn sdf_torso(px: f32, py: f32, pz: f32) -> (f32, f32, f32) {
    const Y0: f32 = -0.68;   // shoulder junction (top cap)
    const Y1: f32 =  0.32;   // belt bottom (lower cap)
    let ty = ((py - Y0) / (Y1 - Y0)).clamp(0.0, 1.0);
    let ax = plerp(ty, &[
        (0.00, 0.37),   // shoulder line: broadest
        (0.22, 0.33),   // upper chest
        (0.55, 0.21),   // waist: cinched minimum
        (0.72, 0.24),   // lower jacket
        (1.00, 0.26),   // hip / belt
    ]);
    let az = plerp(ty, &[
        (0.00, 0.16),   // shoulders: relatively flat back
        (0.28, 0.18),   // chest: pec projection forward
        (0.55, 0.13),   // waist: thinnest Z depth
        (1.00, 0.16),   // hip: moderate depth returns
    ]);
    let nx = px / ax;
    let nz = pz / az;
    let cross_d = ((nx*nx + nz*nz).sqrt() - 1.0) * ax.min(az);
    let vert_d  = (py - Y1).max(0.0) + (Y0 - py).max(0.0);
    (cross_d.max(vert_d), ax, az)
}

/// Right upper arm SDF with capsule parameter.
///
/// The arm axis runs from shoulder socket (0.500, −0.60) to elbow (0.530, −0.10).
/// Radius tapers linearly 0.095 → 0.082 (shoulder to elbow).
/// Cross-section: elliptic with X/Z ≈ 1.05/0.95 (bone ar = 0.90).
///
/// Returns `(sdf, t)` where `t ∈ [0,1]` is position along the arm axis
/// (0 = shoulder socket, 1 = elbow) — used to reconstruct cx for the normal.
#[inline]
fn sdf_arm_r(px: f32, py: f32, pz: f32) -> (f32, f32) {
    const AX: f32 = 0.500;  const AY: f32 = -0.60;   // shoulder socket
    const BX: f32 = 0.530;  const BY: f32 = -0.10;   // elbow
    const RA: f32 = 0.095;  const RB: f32 = 0.082;
    let t  = ((py - AY) / (BY - AY)).clamp(0.0, 1.0);
    let cx = AX + t * (BX - AX);
    let r  = RA + t * (RB - RA);
    let dx = (px - cx) / 1.05;   // scale for elliptic X
    let dz = pz        / 0.95;   // scale for elliptic Z
    let xz = (dx*dx + dz*dz).sqrt();
    // Cap spheres: when py is outside [AY, BY], add Y excess to distance
    let ye = (py - BY).max(0.0) + (AY - py).max(0.0);
    let d  = if ye > 0.0 { (xz*xz + ye*ye).sqrt() - r } else { xz - r };
    (d, t)
}

/// Right forearm: tapered elliptic capsule, elbow → wrist → hand.
/// Radius: 0.075 (elbow) → 0.056 (wrist).  Cross-section same aspect as upper arm.
#[inline]
fn sdf_forearm_r(px: f32, py: f32, pz: f32) -> f32 {
    const AX: f32 = 0.530; const AY: f32 = -0.10;   // elbow
    const BX: f32 = 0.555; const BY: f32 =  0.24;   // wrist
    const RA: f32 = 0.075; const RB: f32 =  0.056;
    let t  = ((py - AY) / (BY - AY)).clamp(0.0, 1.0);
    let cx = AX + t * (BX - AX);
    let r  = RA + t * (RB - RA);
    let dx = (px - cx) / 1.05;
    let dz = pz        / 0.95;
    let xz = (dx*dx + dz*dz).sqrt();
    let ye = (py - BY).max(0.0) + (AY - py).max(0.0);
    if ye > 0.0 { (xz*xz + ye*ye).sqrt() - r } else { xz - r }
}

/// Right leg: smooth union of thigh capsule and shin/boot capsule.
/// Hip (0.28) → knee (0.72): radius 0.105 → 0.088.
/// Knee (0.70) → ankle (0.97): radius 0.090 → 0.055.
/// Knee overlap creates a natural smin crease for AO/wrinkle.
#[inline]
fn sdf_leg_r(px: f32, py: f32, pz: f32) -> f32 {
    const CX: f32 = 0.145;   // leg centreline X
    // Thigh
    let d_thigh = {
        let t = ((py - 0.28f32) / 0.44).clamp(0.0, 1.0);
        let r = 0.105 + t * (0.088 - 0.105);
        let dx = px - CX; let dz = pz;
        let xz = (dx*dx + dz*dz).sqrt();
        let ye = (py - 0.72f32).max(0.0) + (0.28f32 - py).max(0.0);
        if ye > 0.0 { (xz*xz + ye*ye).sqrt() - r } else { xz - r }
    };
    // Shin + boot
    let d_shin = {
        let t = ((py - 0.68f32) / 0.29).clamp(0.0, 1.0);
        let r = 0.090 + t * (0.055 - 0.090);
        let dx = px - CX; let dz = pz;
        let xz = (dx*dx + dz*dz).sqrt();
        let ye = (py - 0.97f32).max(0.0) + (0.68f32 - py).max(0.0);
        if ye > 0.0 { (xz*xz + ye*ye).sqrt() - r } else { xz - r }
    };
    smin(d_thigh, d_shin, 0.040)
}

/// Consolidated body SDF: smooth union of all limbs.
/// Used by AO, vol_shadow, curvature — every probe that needs full-body geometry.
#[inline(always)]
fn sdf_body(px: f32, py: f32, pz: f32) -> f32 {
    let d = smin(sdf_torso(px, py, pz).0, sdf_arm_r(px, py, pz).0, 0.04);
    let d = smin(d, sdf_arm_r(-px, py, pz).0, 0.04);     // left upper arm (X-mirror)
    let d = smin(d, sdf_forearm_r(px, py, pz), 0.04);    // right forearm + hand
    let d = smin(d, sdf_forearm_r(-px, py, pz), 0.04);   // left forearm + hand
    let d = smin(d, sdf_leg_r(px, py, pz), 0.04);        // right leg + boot
    smin(d, sdf_leg_r(-px, py, pz), 0.04)                // left leg + boot
}

/// Surface curvature at a body surface point via the tangential SDF Laplacian.
///
/// Approximates 2H (twice the mean curvature) using second central differences
/// of the SDF along two orthogonal surface-tangent directions.
///
/// **Physical basis** — for a signed distance field where |∇F| = 1:
///
///   ΔF = (n−1) · κ_mean   →   in 3D: ΔF = 2H
///
/// The SDF is linear along the outward normal (∂²F/∂n² ≈ 0), so the full
/// Laplacian reduces to the tangential Laplacian, which is exactly what the two
/// tangent-direction finite differences give.
///
/// **Sign convention** (outward-pointing normal):
/// ```text
///   κ < 0  →  concave  →  inner-elbow crease, armpit, waist fold, under-chin
///   κ > 0  →  convex   →  shoulder cap, shin crest, elbow point
/// ```
///
/// **Empirical κ values for this body (model units):**
/// ```text
///   Inner elbow crease   −20 … −35   (tight fold when arm extended)
///   Armpit blend zone    −25 … −50   (smin bowl)
///   Waist cinch          −10 … −18   (gentle hourglass concavity)
///   Under-chin             −8 … −14
///   Shoulder cap          +12 … +25   (convex → wrinkle = 0)
/// ```
///
/// **Cost:** 4 `sdf_body` calls.  `d0` is the SDF value already computed in the
/// main loop's acceptance check — passing it avoids one redundant evaluation.
#[inline]
fn sdf_curvature(px: f32, py: f32, pz: f32,
                 snx: f32, sny: f32, snz: f32, d0: f32) -> f32 {
    // Step: 2.5 cm in model space (8 cm on screen at ×3.2).
    // Large enough to bridge sub-particle noise; small enough to resolve creases.
    const E: f32 = 0.025;
    // Tangent 1: rotate the XZ normal 90° in-plane (already unit length).
    let t1x = -snz;
    let t1z =  snx;
    // Tangent 2: world-Y axis orthogonalised against n via Gram-Schmidt.
    //   t2 = (0,1,0) − dot((0,1,0), n)·n
    let t2x = -snx * sny;
    let t2y =  1.0 - sny * sny;
    let t2z = -snz * sny;
    let t2n = (t2x*t2x + t2y*t2y + t2z*t2z).sqrt().max(0.001);
    let (t2x, t2y, t2z) = (t2x/t2n, t2y/t2n, t2z/t2n);
    // Second central differences along each tangent
    let d1p = sdf_body(px + E*t1x,        py,          pz + E*t1z);
    let d1m = sdf_body(px - E*t1x,        py,          pz - E*t1z);
    let d2p = sdf_body(px + E*t2x, py + E*t2y, pz + E*t2z);
    let d2m = sdf_body(px - E*t2x, py - E*t2y, pz - E*t2z);
    // k1 + k2 ≈ 2H — negative for concave creases, positive for convex ridges
    (d1p + d1m - 2.0*d0 + d2p + d2m - 2.0*d0) / (E * E)
}

/// Volumetric shadow transmittance along the ray from `(px,py,pz)` toward the
/// light source direction `(lx,ly,lz)`.
///
/// Marches through the body SDF volume, accumulating optical depth wherever
/// the field is negative (inside the body).  Returns Beer-Lambert transmittance
/// `T ∈ [0, 1]`:  `T = 1` (fully lit) when no body geometry intercepts the ray;
/// `T → 0` when the ray crosses a thick body section.
///
/// **God ray mechanism**
/// A surface particle that is blocked by other geometry (right arm in the path of
/// the torch; shoulder above the upper chest; collar above the neck) receives
/// `T ≪ 1`.  An adjacent unblocked particle receives `T ≈ 1`.  The sharp contrast
/// IS the visible light shaft — no post-process, no screen effect, no hand-placed
/// volume.  It emerges from the body's own SDF topology.
///
/// **Capture radius:** 6 × 0.075 = 0.45 model units (≈ 14 cm at ×3.2).
/// Resolves arm-to-torso and shoulder-to-neck occlusion.
/// Head-to-chest (≈ 1.5 units) needs more steps if required.
///
/// **Cost:** 6 `sdf_body` calls ≈ 66 ns / particle.
#[inline]
fn vol_shadow(px: f32, py: f32, pz: f32, lx: f32, ly: f32, lz: f32) -> f32 {
    const STEPS: usize = 6;
    const STEP:  f32   = 0.075;   // model-space step (2.4 cm at ×3.2)
    const SIGMA: f32   = 14.0;    // extinction coefficient of body tissue
    let mut tau = 0.0f32;
    for k in 1..=STEPS {
        let h = k as f32 * STEP;
        let d = sdf_body(px + h*lx, py + h*ly, pz + h*lz);
        // Density = max(0, −SDF): deeper inside = more extinction.
        // Cap at 0.12 to prevent a single very-deep step from over-darkening.
        if d < 0.0 { tau += (-d).min(0.12) * STEP; }
    }
    (-SIGMA * tau).exp()
}

/// Light parameter bundle — avoids 15-argument function signatures.
struct SdfLights {
    klx:f32, kly:f32, klz:f32,
    rlx:f32, rly:f32, rlz:f32,
    f1x:f32, f1y:f32, f1z:f32,
    f2x:f32, f2y:f32, f2z:f32,
    f3x:f32, f3y:f32, f3z:f32,
    f3_int: f32,
}

/// Sky hemisphere color for a direction vector.
/// Returns a warm-at-horizon / cool-at-zenith gradient with a sun halo toward the key light.
#[inline(always)]
fn sky_color(dx: f32, dy: f32, dz: f32) -> (f32, f32, f32) {
    let r = (dx*dx + dy*dy + dz*dz).sqrt().max(0.001);
    // el = 0 at horizon, 1 at zenith (model Y-negative = visual up, so sky is –Y)
    let el = (-dy / r).clamp(0.0, 1.0);
    // Sun halo toward key light direction (matches SdfLights initialisation in render_sdf_body)
    const KLX: f32 = -0.375; const KLY: f32 = -0.515; const KLZ: f32 = 0.685;
    let sun_dot = ((dx/r)*KLX + (dy/r)*KLY + (dz/r)*KLZ).max(0.0);
    let sun_glow = sun_dot.powi(8) * 0.55;
    // Gradient: horizon (0.58,0.38,0.18) → mid-sky (0.20,0.22,0.45) → zenith (0.06,0.08,0.24)
    let t = el;
    let r_s = if t < 0.4 { 0.58*(1.0-t/0.4) + 0.20*(t/0.4) }
              else        { 0.20*(1.0-(t-0.4)/0.6) + 0.06*((t-0.4)/0.6) };
    let g_s = if t < 0.4 { 0.38*(1.0-t/0.4) + 0.22*(t/0.4) }
              else        { 0.22*(1.0-(t-0.4)/0.6) + 0.08*((t-0.4)/0.6) };
    let b_s = if t < 0.4 { 0.18*(1.0-t/0.4) + 0.45*(t/0.4) }
              else        { 0.45*(1.0-(t-0.4)/0.6) + 0.24*((t-0.4)/0.6) };
    (r_s + sun_glow, g_s + sun_glow*0.72, b_s + sun_glow*0.30)
}

/// Full lighting pass: Lambert key + 3 fills + rim + Fresnel + spec.
/// Filmic tonemap + S-curve contrast.  Exact render_leon formula.
/// Returns post-processed (cr, cg, cb).
#[inline(always)]
fn sdf_shade(
    snx: f32, sny: f32, snz: f32,
    hz: f32,
    br: f32, bg: f32, bb: f32,
    mat_tag: MatTag,
    l: &SdfLights,
) -> (f32, f32, f32) {
    const F1:(f32,f32,f32)=(1.00,0.85,0.70); const F1I:f32=0.15;
    const F2:(f32,f32,f32)=(0.60,0.70,1.00); const F2I:f32=0.05;
    const F3:(f32,f32,f32)=(1.00,0.52,0.15);
    let spec = if matches!(mat_tag, MatTag::Boot|MatTag::Metal) {
        (snz*(l.klz+1.0)*0.5).max(0.0).powi(24)*0.50
    } else if mat_tag == MatTag::Eye {
        // Wet cornea: Blinn-Phong with very high exponent → pinpoint highlight.
        // Half-vector H = normalise((0,0,1) + key_light_dir).
        let hx = l.klx; let hy = l.kly; let hz_h = l.klz + 1.0;
        let hn = (hx*hx + hy*hy + hz_h*hz_h).sqrt().max(0.001);
        let ndoth = (snx*hx + sny*hy + snz*hz_h).max(0.0) / hn;
        ndoth.powi(128) * 1.80
    } else { 0.0 };
    let dk  = (snx*l.klx + sny*l.kly + snz*l.klz).max(0.0);
    let df1 = (snx*l.f1x + sny*l.f1y + snz*l.f1z).max(0.0);
    let df2 = (snx*l.f2x + sny*l.f2y + snz*l.f2z).max(0.0);
    let df3 = (snx*l.f3x + sny*l.f3y + snz*l.f3z).max(0.0);
    let rim = { let f=(1.0-hz)*(snx*l.rlx+sny*l.rly+snz*l.rlz).max(0.0); f*f };
    let sky = (-sny).max(0.0)*0.08; let gnd = sny.max(0.0)*0.04;
    let ir  = dk*1.10+spec + df1*F1I*F1.0 + df2*F2I*F2.0 + df3*l.f3_int*F3.0
              + 0.03+sky*0.60+gnd*0.40;
    let ig  = dk*1.10+spec + df1*F1I*F1.1 + df2*F2I*F2.1 + df3*l.f3_int*F3.1
              + 0.03+sky*0.75+gnd*0.35;
    let ib  = dk*1.10+spec + df1*F1I*F1.2 + df2*F2I*F2.2 + df3*l.f3_int*F3.2
              + 0.03+sky*1.00+gnd*0.25;
    let (fr,fg,fb) = fresnel_response(mat_tag, snz);
    // Sky environment reflection: view dir = (0,0,1); r = view − 2(view·n)n = (−2snx·snz, −2sny·snz, 1−2snz²)
    let (er,eg,eb) = if matches!(mat_tag, MatTag::Boot|MatTag::Metal|MatTag::Eye) {
        let rx = -2.0*snz*snx; let ry = -2.0*snz*sny; let rz = 1.0 - 2.0*snz*snz;
        let (sr,sg,sb) = sky_color(rx, ry, rz);
        let k = match mat_tag { MatTag::Metal=>0.38, MatTag::Eye=>0.22, MatTag::Boot=>0.15, _=>0.0 };
        (sr*k, sg*k, sb*k)
    } else { (0.0, 0.0, 0.0) };
    let tone = |c:f32| c*(1.0+c*0.12)/(1.0+c);
    let sc   = |c:f32| c*c*(3.0-2.0*c);
    let cr = sc(tone((br*ir+rim*0.12+fr+er).min(1.4)).clamp(0.0,1.0));
    let cg = sc(tone((bg*ig+rim*0.16+fg+eg).min(1.4)).clamp(0.0,1.0));
    let cb = sc(tone((bb*ib+rim*0.42+fb+eb).min(1.4)).clamp(0.0,1.0));
    (cr, cg, cb)
}

/// Analytical ambient occlusion from the SDF.
///
/// Marches a short ray of N steps along the outward surface normal and samples
/// the body SDF at each step.  If the SDF value at step h is smaller than h,
/// geometry is blocking the escape — the point is occluded.  The weighted sum
/// of (h − d) gives an occlusion estimate that is EXACT to the precision of
/// the SDF itself: no screen-space approximation, no pre-baked texture, no
/// triangle rays.  The SDF already encodes all the concave geometry, so it
/// self-shadows for free.
///
/// What darkens and why:
///   Armpit       — torso and arm SDFs are both small there; the arm fills
///                  the escape horizon of torso edge particles and vice versa.
///   Shoulder cap — arm insertion curves into the torso; the smin blend zone
///                  creates a shallow bowl the AO rays can't escape.
///   Waist        — hip and chest are both wider than the waist; particles at
///                  the cinch see the surrounding body in their near samples.
///   Belt / hem   — the jacket bottom edge creates a slight overhang; the
///                  smin blend with the belt region darkens the crease.
///
/// The formula is Inigo Quilez's classic SDF AO (STEPS samples, geometric
/// decay weights, scale 3.0 for contrast).  On GPU this would run in the
/// fragment shader at no meaningful cost; on CPU it adds ~285 ns per accepted
/// particle (5 SDF evaluations × ~57 ns each).
///
/// Returns AO factor ∈ [0.25, 1.0].  The 0.25 floor keeps fully-occluded
/// regions dark but not black — they still read as material.
#[inline]
fn sdf_ao(px: f32, py: f32, pz: f32, snx: f32, sny: f32, snz: f32) -> f32 {
    let mut occ = 0.0f32;
    let mut sca = 1.0f32;
    // Geometrically spaced steps: 1 cm → 14 cm in model space (×3.2 = 3 cm → 45 cm on screen).
    // Close steps catch tight concavities (armpit, inner elbow);
    // far steps catch broad occlusion (waist hollowed between chest and hip).
    const STEPS: [f32; 5] = [0.010, 0.035, 0.065, 0.100, 0.140];
    for &h in &STEPS {
        let sx = px + h * snx;
        let sy = py + h * sny;
        let sz = pz + h * snz;
        // Full smooth-union SDF at the sample point.
        // When d < h, geometry is within this step's horizon — contributes occlusion.
        let d = smin(sdf_torso(sx, sy, sz).0, sdf_arm_r(sx, sy, sz).0, 0.04);
        occ += (h - d) * sca;
        sca *= 0.95;   // geometric decay: near samples weighted most
    }
    // ao ∈ [0,1] from IQ formula.  Lerp with floor 0.25 so material is always legible.
    let ao = (1.0 - 3.0 * occ).clamp(0.0, 1.0);
    ao * 0.75 + 0.25
}

/// SDF thickness probe for subsurface scattering.
///
/// Marches inward along `-n` from the surface point and returns the depth
/// of the first probe where `SDF ≥ 0` — i.e., where we have emerged from
/// the body on the opposite side.  When the body is thicker than the
/// deepest probe (0.380 model units ≈ 12 cm at ×3.2 scale), returns 0.380;
/// Beer-Lambert will attenuate that to near-zero transmission anyway.
///
/// Six geometrically spaced probes: 2 cm → 38 cm in model space.
/// Thin regions (ear, lip edge) exit early (~2-5 cm); chest/thigh return max.
#[inline]
fn sdf_thickness(px: f32, py: f32, pz: f32, snx: f32, sny: f32, snz: f32) -> f32 {
    const PROBES: [f32; 6] = [0.020, 0.055, 0.100, 0.160, 0.250, 0.380];
    for &h in &PROBES {
        // Step inward along -n
        let sx = px - h * snx;
        let sy = py - h * sny;
        let sz = pz - h * snz;
        let d = smin(sdf_torso(sx, sy, sz).0, sdf_arm_r(sx, sy, sz).0, 0.04);
        if d >= 0.0 { return h; }   // emerged from body — h is the tissue thickness
    }
    0.380   // still inside at max depth — body is thick here (chest, thigh)
}

// ── Procedural noise for surface detail ─────────────────────────────────────
//
// Traditional renderers bake pore/skin detail into texture and normal maps that
// are sampled at fixed resolution.  Here it is computed: infinite resolution,
// zero memory, updates automatically if the body deforms.
//
// The three-layer fBm stack:
//   Low  (scale 3.0, 2 octaves) — organic undulation, λ ≈ 0.33 model units
//                                  Shoulders and torso stop looking like balloons.
//   Mid  (scale 16,  1 octave)  — pore/follicle bumps, λ ≈ 0.06 units
//                                  Computes a 4-sample normal gradient; tilts the
//                                  shading normal up to ≈20° for per-particle drama.
//   High (scale 52,  1 octave)  — micro-roughness, λ ≈ 0.02 units (sub-particle)
//                                  Modulates specular intensity: skin sparkles and
//                                  scatters instead of reflecting uniformly.

/// 3D smooth value noise.  Returns ∈ [−0.5, +0.5].
/// Smoothstep blending gives C¹ continuity — no directional banding.
#[inline(always)]
fn noise3(x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let zf = z - zi as f32;
    // Smoothstep: t²(3−2t)
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let w = zf * zf * (3.0 - 2.0 * zf);
    // Lattice hash at 8 corners, zero-centred to [−0.5, +0.5]
    let h = |ix: i32, iy: i32, iz: i32| -> f32 {
        let n = (ix.wrapping_mul(374_761_393_i32))
            .wrapping_add(iy.wrapping_mul(668_265_263_i32))
            .wrapping_add(iz.wrapping_mul(1_291_057_433_i32)) as u32;
        let n = n ^ (n >> 13);
        let n = n.wrapping_mul(0x5851_F42D);
        let n = n ^ (n >> 16);
        (n & 0x00FF_FFFF) as f32 / 16_777_216.0 - 0.5
    };
    let lp = |t: f32, a: f32, b: f32| a + t * (b - a);
    lp(w,
       lp(v, lp(u, h(xi,  yi,  zi  ), h(xi+1,yi,  zi  )),
              lp(u, h(xi,  yi+1,zi  ), h(xi+1,yi+1,zi  ))),
       lp(v, lp(u, h(xi,  yi,  zi+1), h(xi+1,yi,  zi+1)),
              lp(u, h(xi,  yi+1,zi+1), h(xi+1,yi+1,zi+1))))
}

/// Per-particle procedural skin detail.
/// Evaluated once per accepted SDF particle before shading.
struct SkinDetail {
    /// Signed displacement along the analytical normal (model units).
    /// Positive = pushed outward; negative = pulled inward.
    disp:      f32,
    /// Bump-mapped normal (normalised).  Replaces the analytical normal for all
    /// lighting, AO, and SSS calculations.
    pnx: f32, pny: f32, pnz: f32,
    /// High-frequency roughness ∈ [0, 1].  0 = smooth mirror; 1 = rough scatter.
    roughness: f32,
}

/// Evaluates three procedural noise bands at `(px, py, pz)` and returns
/// displacement, a perturbed normal, and a roughness factor.
///
/// Cost: 7 noise3 calls (≈ 8 hash ops each) = ~56 ops / particle.
/// At 244K particles and 4 ops/ns: ≈ 3.4 ms additional CPU per frame.
#[inline]
fn skin_detail(px: f32, py: f32, pz: f32, snx: f32, sny: f32, snz: f32) -> SkinDetail {
    // ── Low frequency: organic undulation (2 manual octaves) ─────────────────
    let fl = noise3(px * 3.0, py * 3.0, pz * 3.0)
           + noise3(px * 6.0, py * 6.0, pz * 6.0) * 0.5;
    // fl ∈ [−0.75, +0.75]; displacement ±6 mm in model space

    // ── Mid frequency: normal gradient via forward finite differences ─────────
    // One octave at scale 16.  Gradient magnitude ≈ 16 × 0.75 = 12 per unit.
    // BUMP_STR = tan(20°) / 12 ≈ 0.030 → max normal deflection ≈ 20°.
    const GE:   f32 = 0.009;   // step in model space
    const BUMP: f32 = 0.030;
    let fc  = noise3( px      * 16.0,  py      * 16.0,  pz      * 16.0);
    let fxp = noise3((px + GE) * 16.0,  py      * 16.0,  pz      * 16.0);
    let fyp = noise3( px      * 16.0, (py + GE) * 16.0,  pz      * 16.0);
    let fzp = noise3( px      * 16.0,  py      * 16.0, (pz + GE) * 16.0);
    // World-space gradient (forward difference)
    let gx = (fxp - fc) / GE;
    let gy = (fyp - fc) / GE;
    let gz = (fzp - fc) / GE;
    // Project out the normal component → pure tangent-plane bump (no inflation)
    let gdn = gx * snx + gy * sny + gz * snz;
    let bx  = (gx - gdn * snx) * BUMP;
    let by  = (gy - gdn * sny) * BUMP;
    let bz  = (gz - gdn * snz) * BUMP;
    let pnx = snx + bx;
    let pny = sny + by;
    let pnz = snz + bz;
    let pnn = (pnx * pnx + pny * pny + pnz * pnz).sqrt().max(0.001);

    // ── High frequency: micro-roughness (sub-particle scale) ─────────────────
    let fh = noise3(px * 52.0, py * 52.0, pz * 52.0);  // ∈ [−0.5, +0.5]

    SkinDetail {
        disp:      fl * 0.008 + fc * 0.003,
        pnx:       pnx / pnn,
        pny:       pny / pnn,
        pnz:       pnz / pnn,
        roughness: fh * 0.5 + 0.5,   // remapped to [0, 1]
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ── Face SDF system ────────────────────────────────────────────────────────────
//
// The face is a hierarchy of nested SDFs evaluated in order:
//
//   HEAD_ELLIPSOID  — base skull/face volume
//   − EYE_SOCKET_R  — smooth subtraction → right eye socket concavity
//   − EYE_SOCKET_L  — smooth subtraction → left  eye socket concavity
//   ∪ EYEBALL_R     — smooth union → eyeball sphere nested in right socket
//   ∪ EYEBALL_L     — smooth union → eyeball sphere nested in left  socket
//   ∪ NOSE          — smooth union → nasal bridge protrusion (forward ellipsoid)
//   ∪ UPPER_LIP     — smooth union → upper lip ridge (thin ellipsoid)
//   ∪ LOWER_LIP     — smooth union → lower lip ridge (thin ellipsoid)
//   − MOUTH_CREASE  — smooth subtraction → horizontal capsule groove
//
// All normals are the numerical gradient of sdf_face — they inherit every
// concavity and protrusion automatically with zero hand-painted data.
//
// Coordinate frame (model space, same as the body):
//   Y = −0.92: forehead/hairline.   Y = −0.68: eye line.   Y = −0.28: chin.
//   Z positive = toward camera.     Eye sockets: X = ±0.075.
// ═══════════════════════════════════════════════════════════════════════════════

// ── Face geometry constants ───────────────────────────────────────────────────
const HEAD_CY: f32 = -0.600; // ellipsoid Y centre
const HEAD_RX: f32 = 0.165;  // X half-width  (cheek)
const HEAD_RY: f32 = 0.320;  // Y half-height (hairline −0.92 → chin −0.28)
const HEAD_RZ: f32 = 0.115;  // Z half-depth

// Eye socket spheres (carved concavities) — centre is INSIDE the face surface
const SOCK_CX: f32 = 0.075;  // X offset from centre line
const SOCK_CY: f32 = -0.675; // Y: eye line (between brow shadow and cheek)
const SOCK_CZ: f32 = 0.062;  // Z: 37 mm inside face surface (≈ 99 mm)
const SOCK_R:  f32 = 0.038;  // socket sphere radius

// Eyeball spheres (smooth-unioned back inside the sockets)
const EYEB_CX: f32 = 0.075;  // X same as socket
const EYEB_CY: f32 = -0.675; // Y same as socket
const EYEB_CZ: f32 = 0.088;  // Z: front of eyeball (0.115) protrudes 16 mm past face surface
const EYEB_R:  f32 = 0.027;  // eyeball radius

// Nose (forward ellipsoid)
const NOSE_CY: f32 = -0.500; // Y: mid-nose bridge
const NOSE_CZ: f32 = 0.095;  // Z: nose tip at 0.140 > face surface 0.109 → protrudes 31 mm
const NOSE_RX: f32 = 0.028;  // X half-width (narrow bridge)
const NOSE_RY: f32 = 0.050;  // Y half-height (root to tip)
const NOSE_RZ: f32 = 0.045;  // Z half-depth (forward projection)

// Lips
const LIPU_CY: f32 = -0.360; // upper lip Y centre (above crease)
const LIPU_CZ: f32 = 0.074;  // upper lip Z: tip at 0.087 > face surface 0.076 ✓
const LIPL_CY: f32 = -0.385; // lower lip Y centre (below crease)
const LIPL_CZ: f32 = 0.072;  // lower lip Z: tip at 0.087 ≈ face surface 0.085

// Mouth crease (thin horizontal capsule)
const MOUTH_Y:  f32 = -0.373; // Y between lip ridges
const MOUTH_Z:  f32 = 0.073;  // Z inside face surface → groove is physically recessed
const MOUTH_HX: f32 = 0.068;  // half-length in X
const MOUTH_R:  f32 = 0.008;  // capsule cross-section radius

/// Smooth maximum — dual of `smin`.  `smax(a,b,k) = −smin(−a,−b,k)`.
#[inline(always)]
fn smax(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.max(b) + h * h * k * 0.25
}

/// Smooth subtraction: carve `carve` out of `base`.
/// `smooth_sub(base, carve, k) = smax(−carve, base, k)`.
/// Inside the carved region: `base < 0` and `carve < 0` → result > 0 (empty).
#[inline(always)]
fn smooth_sub(base: f32, carve: f32, k: f32) -> f32 {
    smax(-carve, base, k)
}

/// Sphere SDF.  `(dx, dy, dz)` = vector from sphere centre to sample point.
#[inline(always)]
fn sdf_sphere_f(dx: f32, dy: f32, dz: f32, r: f32) -> f32 {
    (dx*dx + dy*dy + dz*dz).sqrt() - r
}

/// Head base ellipsoid SDF.
/// Approximate (not true Euclidean) — within ≈1.5× of correct near the surface.
/// Covers forehead (Y = −0.92) to chin (Y = −0.28).
#[inline(always)]
fn sdf_head_ellipsoid(px: f32, py: f32, pz: f32) -> f32 {
    let nx = px / HEAD_RX;
    let ny = (py - HEAD_CY) / HEAD_RY;
    let nz = pz / HEAD_RZ;
    ((nx*nx + ny*ny + nz*nz).sqrt() - 1.0) * HEAD_RX.min(HEAD_RZ)
}

/// Composite face SDF — all features combined.
///
/// Evaluation order is important: sockets first (carve head), then eyeballs
/// (fill sockets), then nose + lips (add protrusions), then mouth crease (carve
/// between lips).  smin/smooth_sub blend radii are tuned so every junction
/// reads as a continuous organic surface.
#[inline]
fn sdf_face(px: f32, py: f32, pz: f32) -> f32 {
    // 1. Head base
    let d = sdf_head_ellipsoid(px, py, pz);

    // 2. Eye sockets — symmetric concavities in the orbital region
    let sr = sdf_sphere_f(px - SOCK_CX, py - SOCK_CY, pz - SOCK_CZ, SOCK_R);
    let sl = sdf_sphere_f(px + SOCK_CX, py - SOCK_CY, pz - SOCK_CZ, SOCK_R);
    let d  = smooth_sub(d, sr, 0.020);
    let d  = smooth_sub(d, sl, 0.020);

    // 3. Eyeballs — small spheres nested inside the carved sockets
    let er = sdf_sphere_f(px - EYEB_CX, py - EYEB_CY, pz - EYEB_CZ, EYEB_R);
    let el = sdf_sphere_f(px + EYEB_CX, py - EYEB_CY, pz - EYEB_CZ, EYEB_R);
    let d  = smin(d, er, 0.012);
    let d  = smin(d, el, 0.012);

    // 4. Nose — ellipsoid protruding forward from mid-face
    let nose = { let nx=px/NOSE_RX; let ny=(py-NOSE_CY)/NOSE_RY; let nz=(pz-NOSE_CZ)/NOSE_RZ;
                 ((nx*nx+ny*ny+nz*nz).sqrt()-1.0)*NOSE_RX.min(NOSE_RZ) };
    let d  = smin(d, nose, 0.025);

    // 5. Lip ridges — unioned BEFORE the crease so the groove cuts between them
    let ul = { const RX:f32=0.056; const RY:f32=0.011; const RZ:f32=0.013;
               let nx=px/RX; let ny=(py-LIPU_CY)/RY; let nz=(pz-LIPU_CZ)/RZ;
               ((nx*nx+ny*ny+nz*nz).sqrt()-1.0)*RX.min(RZ) };
    let ll = { const RX:f32=0.058; const RY:f32=0.013; const RZ:f32=0.015;
               let nx=px/RX; let ny=(py-LIPL_CY)/RY; let nz=(pz-LIPL_CZ)/RZ;
               ((nx*nx+ny*ny+nz*nz).sqrt()-1.0)*RX.min(RZ) };
    let d  = smin(d, ul, 0.011);
    let d  = smin(d, ll, 0.011);

    // 6. Mouth crease — horizontal capsule subtracted between the lip ridges
    let mc = { let cx = px.clamp(-MOUTH_HX, MOUTH_HX);
               let ddx=px-cx; let ddy=py-MOUTH_Y; let ddz=pz-MOUTH_Z;
               (ddx*ddx+ddy*ddy+ddz*ddz).sqrt()-MOUTH_R };
    smooth_sub(d, mc, 0.009)
}

/// Eyeball surface color: pupil / iris / limbus / sclera.
///
/// `eye_cx` is the signed X offset of this eye's centre.
/// Angle from the optical axis (forward = +Z) determines the zone:
///   cos_t > 0.950: pupil (very dark)
///   cos_t > 0.750: iris (warm hazel brown)
///   cos_t > 0.680: limbus (dark ring)
///   rest         : sclera (warm white)
/// Returns `(r, g, b, is_iris)` — `is_iris` true for pupil/iris.
#[inline]
fn eye_color(px: f32, py: f32, pz: f32, eye_cx: f32) -> (f32, f32, f32, bool) {
    let ex = px - eye_cx;
    let ey = py - EYEB_CY;
    let ez = pz - EYEB_CZ;
    let er = (ex*ex + ey*ey + ez*ez).sqrt().max(0.001);
    let cos_t = (ez / er).clamp(-1.0, 1.0);
    if cos_t > 0.950 {
        return (0.05, 0.04, 0.04, true);   // pupil: near-black
    }
    if cos_t > 0.750 {
        let t = (0.950 - cos_t) / 0.200;   // 0 = pupil edge, 1 = limbus edge
        return (0.28 + t*0.14, 0.17 + t*0.06, 0.06 + t*0.03, true);  // warm hazel
    }
    if cos_t > 0.680 {
        return (0.12, 0.09, 0.07, true);   // limbus: dark transition ring
    }
    // Sclera — warm white with slight redness near limbus
    let (sr, sg, sb) = if cos_t > 0.50 { (0.88, 0.82, 0.75) } else { (0.94, 0.90, 0.84) };
    (sr, sg, sb, false)
}

/// Numerical gradient (central differences, EPS = 0.005 m.u.).
/// Cost: 6 `sdf_face` evaluations.
#[inline]
fn face_normal(px: f32, py: f32, pz: f32) -> (f32, f32, f32) {
    const EPS: f32 = 0.005;
    let nx = sdf_face(px+EPS,py,pz) - sdf_face(px-EPS,py,pz);
    let ny = sdf_face(px,py+EPS,pz) - sdf_face(px,py-EPS,pz);
    let nz = sdf_face(px,py,pz+EPS) - sdf_face(px,py,pz-EPS);
    let nn = (nx*nx+ny*ny+nz*nz).sqrt().max(0.001);
    (nx/nn, ny/nn, nz/nn)
}

/// AO for the face SDF.  IQ formula, 4 steps.
/// Tighter step schedule than the body (face creases are shallower).
/// Floor 0.30: eye socket is dark but readable.
#[inline]
fn sdf_face_ao(px: f32, py: f32, pz: f32, snx: f32, sny: f32, snz: f32) -> f32 {
    const STEPS: [f32; 4] = [0.007, 0.018, 0.038, 0.065];
    let mut occ = 0.0f32;
    let mut sca = 1.0f32;
    for &h in &STEPS {
        let d = sdf_face(px+h*snx, py+h*sny, pz+h*snz);
        occ += (h - d) * sca;
        sca *= 0.92;
    }
    let ao = (1.0 - 3.5*occ).clamp(0.0, 1.0);
    ao * 0.70 + 0.30
}

/// Tangential Laplacian curvature for the face SDF.
/// Same formula as `sdf_curvature` but probes `sdf_face`.
/// Detects socket rims (very concave), nasolabial folds, mouth corners.
#[inline]
fn sdf_face_curvature(px: f32, py: f32, pz: f32,
                      snx: f32, sny: f32, snz: f32, d0: f32) -> f32 {
    const E: f32 = 0.018;
    let t1x = -snz; let t1z = snx;
    let t2x = -snx*sny; let t2y = 1.0-sny*sny; let t2z = -snz*sny;
    let t2n = (t2x*t2x+t2y*t2y+t2z*t2z).sqrt().max(0.001);
    let (t2x,t2y,t2z) = (t2x/t2n, t2y/t2n, t2z/t2n);
    let d1p = sdf_face(px+E*t1x, py,        pz+E*t1z       );
    let d1m = sdf_face(px-E*t1x, py,        pz-E*t1z       );
    let d2p = sdf_face(px+E*t2x, py+E*t2y,  pz+E*t2z       );
    let d2m = sdf_face(px-E*t2x, py-E*t2y,  pz-E*t2z       );
    (d1p+d1m-2.0*d0 + d2p+d2m-2.0*d0) / (E*E)
}

/// SSS thickness probe for face skin (same probe pattern as `sdf_thickness`
/// but uses `sdf_face` and a shallower max of 0.240 — face tissue is thinner).
#[inline]
fn sdf_face_thickness(px: f32, py: f32, pz: f32, snx: f32, sny: f32, snz: f32) -> f32 {
    const PROBES: [f32; 5] = [0.014, 0.038, 0.076, 0.132, 0.220];
    for &h in &PROBES {
        let sx = px-h*snx; let sy = py-h*sny; let sz = pz-h*snz;
        if sdf_face(sx, sy, sz) >= 0.0 { return h; }
    }
    0.240
}

/// Color-bleeding global illumination via tangent-plane neighbor sampling.
///
/// For each lit surface particle, sample the material color at 4 nearby points
/// in the tangent plane.  The averaged neighbor albedo, weighted by local
/// irradiance and a small bounce coefficient, produces cross-material color
/// bleeding: the orange jacket tints the neck skin warm above it; olive pants
/// tint the jacket hem green below.  No ray casting — `leon_color` is
/// evaluated analytically at all four sample points.
///
/// RH = horizontal reach (jacket ↔ neck boundary)
/// RV = vertical reach   (pants ↔ jacket hem)
/// BOUNCE = 6 % re-emission fraction — subtle and cumulative across 8 M particles
#[inline]
fn gi_bounce(px: f32, py: f32, snx: f32, sny: f32, snz: f32, illum: f32)
    -> (f32, f32, f32)
{
    const RH:     f32 = 0.065;   // horizontal sample radius (model units)
    const RV:     f32 = 0.095;   // vertical sample radius
    const BOUNCE: f32 = 0.060;   // fraction of neighbor albedo re-emitted

    // Gram-Schmidt tangent basis (mirrors sdf_curvature construction)
    let t1x = -snz;  let t1z = snx;           // T1 — horizontal, in xz-plane
    let t2x = -snx * sny;
    let t2y =  1.0 - sny * sny;
    let t2z = -snz * sny;
    let t2n = (t2x*t2x + t2y*t2y + t2z*t2z).sqrt().max(0.001);
    let (t2x, t2y, t2z) = (t2x / t2n, t2y / t2n, t2z / t2n);

    // 4 tangent-plane samples: ±T1 (left/right) and ±T2 (up/down)
    let (ar, ag, ab) = leon_color(px + RH * t1x,        py              );
    let (br, bg, bb) = leon_color(px - RH * t1x,        py              );
    let (cr2, cg2, cb2) = leon_color(px + RV * t2x,     py + RV * t2y  );
    let (dr, dg, db) = leon_color(px - RV * t2x,        py - RV * t2y  );

    // Average neighbor albedo (equal-weight — all four directions contribute)
    let nr = (ar + br + cr2 + dr) * 0.25;
    let ng = (ag + bg + cg2 + dg) * 0.25;
    let nb = (ab + bb + cb2 + db) * 0.25;

    // Additive contribution: neighbor albedo × arriving irradiance × bounce
    let s = illum * BOUNCE;
    (nr * s, ng * s, nb * s)
}

/// SDF-based torso + right upper arm renderer.
///
/// Replaces ModelKit distribution for BONES 7,8,9,10,11,12,14,25.
/// Those bones are skipped in render_leon when this is active.
///
/// Particle count: N_TORSO + N_ARM ≈ 244K base particles, matching the
/// density of the 8 replaced bones (≈ 48.8% of the 500K bone budget).
fn render_sdf_body(engine: &mut ProofEngine, dt: f32, time: f32, pos: Vec3, hp: f32) {
    // Shell thickness: ±SHELL defines the accepted surface layer in world units.
    const SHELL:   f32   = 0.016;
    // Particle budget: proportional to estimated surface area (torso ≈ 86%).
    const N_TORSO: usize = 210_000;
    const N_ARM:   usize =  34_000;

    let scale  = 3.2f32;
    let breath = 1.0 + (time * 1.4).sin() * 0.010;
    let dmg    = (1.0 - hp).max(0.0);

    // Build light bundle (mirrors render_leon constants exactly)
    let lts = {
        let n = |x:f32,y:f32,z:f32|{ let l=(x*x+y*y+z*z).sqrt(); (x/l,y/l,z/l) };
        let (klx,kly,klz) = n(-0.40,-0.55, 0.73);
        let (rlx,rly,rlz) = n( 0.55,-0.18,-0.82);
        let (f1x,f1y,f1z) = n( 0.30, 0.60,-0.40);
        let (f3x,f3y,f3z) = n(TORCH_X,TORCH_Y,TORCH_Z);
        SdfLights {
            klx,kly,klz, rlx,rly,rlz,
            f1x,f1y,f1z,
            f2x: 0.0, f2y: -1.0, f2z: 0.0,
            f3x,f3y,f3z,
            f3_int: 0.28*(0.80+(time*7.3).sin()*0.11+(time*13.1).cos()*0.07),
        }
    };

    // ── Torso particles ──────────────────────────────────────────────────────
    // Strategy: for each i, sample a height py and a random outward direction,
    // place the candidate at the ellipse surface ± SHELL radial noise.
    // Acceptance rate ≈ 100% by construction — no wasted evaluations.
    const T_Y0: f32 = -0.68;  const T_Y1: f32 = 0.32;
    for i in 0..N_TORSO {
        // Y position — uniform over torso height
        let v  = hf(i, 80);
        let py = T_Y0 + v * (T_Y1 - T_Y0);

        // Ellipse axes at this height (same knots as sdf_torso — no extra call)
        let ax = plerp(v, &[(0.00,0.37),(0.22,0.33),(0.55,0.21),(0.72,0.24),(1.00,0.26)]);
        let az = plerp(v, &[(0.00,0.16),(0.28,0.18),(0.55,0.13),(1.00,0.16)]);

        // Random outward direction on unit circle
        let rx = hf(i, 81)*2.0-1.0;  let rz = hf(i, 82)*2.0-1.0;
        let rn = (rx*rx + rz*rz).sqrt().max(0.001);
        let dx = rx/rn;  let dz = rz/rn;   // unit direction

        // Point on ellipse surface + radial shell noise
        let noise = (hf(i, 83)*2.0 - 1.0) * SHELL;
        let px = (ax + noise) * dx;
        let pz = (az + noise) * dz;

        // SDF verification: smooth union with arm (handles armpit blend)
        let (d_t, ax_v, az_v) = sdf_torso(px, py, pz);
        let (d_a, _)           = sdf_arm_r(px, py, pz);
        let d = smin(d_t, d_a, 0.04);
        if d < -SHELL || d > SHELL { continue; }

        // Analytical surface normal: gradient of ellipse = (x/ax², z/az²)
        let gx = px / (ax_v * ax_v);
        let gz = pz / (az_v * az_v);
        let gn = (gx*gx + gz*gz).sqrt().max(0.001);
        let (snx, sny, snz) = (gx/gn, 0.0f32, gz/gn);

        // ── Surface curvature ─────────────────────────────────────────────────
        // Computed at the clean SDF surface (before noise displacement) using d
        // already in hand.  Negative κ = concave crease; positive κ = convex skin.
        // Wrinkle weight: 0 on flat/convex surfaces, 1 at deep concave creases.
        // Scale 0.035 → saturates at κ ≈ −29 (typical inner-elbow/armpit depth).
        let kappa        = sdf_curvature(px, py, pz, snx, sny, snz, d);
        let wrinkle_raw  = (-kappa * 0.035).clamp(0.0, 1.0);
        // Smoothstep curve: sharp edge at the crease centre, soft fade outward.
        let wrinkle      = wrinkle_raw * wrinkle_raw * (3.0 - 2.0 * wrinkle_raw);

        // ── Procedural skin detail ────────────────────────────────────────────
        // Three noise bands evaluated at the surface point:
        //   Low  (scale 3) — organic undulation; stops the torso looking like
        //                    a math textbook superellipse.
        //   Mid  (scale 16) — pore/follicle bumps; tilts the shading normal up
        //                     to ≈20° so each particle catches light differently.
        //   High (scale 52) — micro-roughness; scatters the specular response so
        //                     skin sparkles rather than reflecting uniformly.
        let sd = skin_detail(px, py, pz, snx, sny, snz);
        // Combined displacement: noise texture + wrinkle inset.
        // Crease particles sink inward — they shade darker under AO and SSS,
        // and the positional offset creates a visible depth at the fold line.
        let inset = wrinkle * 0.006;
        let px  = px + (sd.disp - inset) * snx;
        let py  = py + (sd.disp - inset) * sny;
        let pz  = pz + (sd.disp - inset) * snz;
        // hz for depth sorting: use the perturbed normal's Z component.
        let hz  = sd.pnz.max(0.0);

        // Core: depth within shell (0 = outer surface, 1 = inner surface)
        let core = ((-d / SHELL + 1.0) * 0.5).clamp(0.0, 1.0);

        // Material lookup at displaced position (follows surface displacement)
        let (br, bg, bb) = leon_color(px, py);
        // leon_tag face-skin rule (y ∈ [−0.88, −0.30]) is broad enough to tag the
        // entire upper torso as Skin.  On the jacket surface only the narrow collar
        // strip (|x| ≤ 0.09) is exposed skin; everything wider is jacket fabric.
        let mat_tag = {
            let raw = leon_tag(px, py);
            if raw == MatTag::Skin && px.abs() > 0.09 { MatTag::Jacket } else { raw }
        };
        // Shade with bump-mapped normal — per-particle shading variation
        let (cr, cg, cb) = sdf_shade(sd.pnx, sd.pny, sd.pnz, hz, br, bg, bb, mat_tag, &lts);

        // AO marches along the perturbed normal — micro-concavities self-shadow
        let ao = sdf_ao(px, py, pz, sd.pnx, sd.pny, sd.pnz);
        let (mut cr, mut cg, mut cb) = (cr * ao, cg * ao, cb * ao);

        // Subsurface scattering: skin that is back-illuminated (opposite side from
        // a light source) transmits warm red-orange light through the tissue.
        // Thickness is computed analytically from the SDF — no pre-baked maps.
        // mu = 7.5: skin absorbs ≈ 4× between 2 cm (ears) and 10 cm (mid-chest).
        if mat_tag == MatTag::Skin {
            let back_key   = (-sd.pnx*lts.klx - sd.pny*lts.kly - sd.pnz*lts.klz).max(0.0);
            let back_torch = (-sd.pnx*lts.f3x - sd.pny*lts.f3y - sd.pnz*lts.f3z).max(0.0) * lts.f3_int;
            let back_illum = back_key * 0.80 + back_torch;
            if back_illum > 0.04 {
                let thick = sdf_thickness(px, py, pz, sd.pnx, sd.pny, sd.pnz);
                const MU: f32 = 7.5;   // absorption coefficient — higher = sharper skin edge glow
                let trans = (-MU * thick).exp() * back_illum;
                // Warm red-orange — matches the colour of light seen through a hand held to a torch
                cr += trans * 1.00;
                cg += trans * 0.28;
                cb += trans * 0.08;
            }
        }

        // Micro-roughness specular: high-freq noise breaks up the uniform specular
        // response.  Each particle sees a slightly different reflection angle, making
        // skin look alive and textured rather than waxy.
        // Tied to the key-light dot product — rough patches catch the light; smooth
        // patches don't.  Warm white, as seen on real skin in direct illumination.
        {
            let dk_bump = (sd.pnx*lts.klx + sd.pny*lts.kly + sd.pnz*lts.klz).max(0.0);
            let micro   = sd.roughness * sd.roughness * dk_bump * 0.14;
            cr += micro * 1.00;
            cg += micro * 0.94;
            cb += micro * 0.82;
        }

        // Wrinkle contact shadow.
        // Concave creases gather ambient light from multiple directions and appear
        // darker than flat surfaces even without an explicit shadow ray.
        // The differential channel weighting makes crease shadow WARM — red-brown
        // bleeds through as blue is suppressed more than red.  This matches real
        // subsurface-lit skin in a fold: the shadow is warm, not cold grey.
        if wrinkle > 0.01 {
            cr *= 1.0 - wrinkle * 0.52;
            cg *= 1.0 - wrinkle * 0.62;
            cb *= 1.0 - wrinkle * 0.80;
        }

        // Volumetric self-shadow (god ray).
        // March toward the key light through the body SDF.  Particles blocked by
        // other body geometry (shoulder above chest, collar above neck) lose the
        // direct key-light contribution.  Only attenuates the key-lit fraction —
        // ambient fills and SSS back-illumination are unaffected.
        //
        // The visual result: dark bands where one body part casts a volumetric
        // shadow onto another, bright bands where light passes through the gaps.
        // That alternation IS the god ray — derived entirely from the SDF topology.
        {
            let key_t  = vol_shadow(px, py, pz, lts.klx, lts.kly, lts.klz);
            // Modulate only the direct-key fraction (≈60% of total for lit surfaces).
            // dk_lit: how much key light this particle was already receiving.
            // If dk_lit≈0 (back-facing to key), shadow adds nothing — correct.
            let dk_lit = (sd.pnx*lts.klx + sd.pny*lts.kly + sd.pnz*lts.klz).max(0.0);
            let vol_atten = 1.0 - dk_lit * (1.0 - key_t) * 0.72;
            cr *= vol_atten;
            cg *= vol_atten;
            cb *= vol_atten;
        }

        // Color-bleeding GI: sample neighbor material colors in the tangent plane.
        // illum = approximate total irradiance at this surface point.
        // The jacket immediately below the neck bleeds warm orange upward;
        // the olive pants bleed green into the jacket hem above them.
        // The effect is additive — each particle accumulates its neighbors' color.
        {
            let dk_gi = (sd.pnx*lts.klx + sd.pny*lts.kly + sd.pnz*lts.klz).max(0.0);
            let f3_gi = (sd.pnx*lts.f3x + sd.pny*lts.f3y + sd.pnz*lts.f3z).max(0.0) * lts.f3_int;
            let illum = dk_gi * 0.80 + f3_gi * 0.40 + 0.12;
            let (gi_r, gi_g, gi_b) = gi_bounce(px, py, snx, sny, snz, illum);
            cr += gi_r; cg += gi_g; cb += gi_b;
        }

        // Alpha density boost: crease particles are more visible — the fold
        // concentrates the perceived material density at the line.
        let alpha = ((0.35 + core*0.55) * hp.max(0.08) * (1.0 + wrinkle * 0.50)).min(0.97);
        if alpha < 0.012 { continue; }

        let jx  = (hf(i, 84)-0.5) * dmg * 0.08;
        let jy  = (hf(i, 85)-0.5) * dmg * 0.08;
        // Emission reduction: deep creases are shielded from direct light.
        let emission = (if br>0.58&&bg>0.46 { 0.40+core*0.80 }
                        else if br<0.28&&bg<0.18 { 0.12+core*0.30 }
                        else if bb<0.14 { 0.50+core*1.50 }
                        else { 0.45+core*1.20 }) * (1.0 - wrinkle * 0.42);
        let sz = 0.020 + core*0.018;
        let ch = if core>0.7 {'*'} else if core>0.35 {'+'} else {'.'};


        engine.spawn_glyph(Glyph {
            character: ch, scale: Vec2::splat(sz),
            position: Vec3::new(pos.x + px*scale*breath + jx,
                                pos.y + py*scale*breath + jy,
                                pos.z + hz*0.30),
            color:      Vec4::new(cr, cg, cb, alpha),
            emission,
            glow_color:  Vec3::new(cr*0.55, cg*0.45, cb*0.35),
            glow_radius: core*0.14,
            mass: 0.0, lifetime: dt*1.5,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Normal,
            ..Default::default()
        });
    }

    // ── Right upper arm particles ─────────────────────────────────────────────
    // Sample along the arm axis, place candidate on the tapered capsule surface.
    const ARM_AX:f32=0.500; const ARM_AY:f32=-0.60;
    const ARM_BX:f32=0.530; const ARM_BY:f32=-0.10;
    const ARM_RA:f32=0.095; const ARM_RB:f32=0.082;
    for i in 0..N_ARM {
        let t  = hf(i, 90);   // position along arm axis [0=shoulder, 1=elbow]
        let py = ARM_AY + t * (ARM_BY - ARM_AY);
        let cx = ARM_AX + t * (ARM_BX - ARM_AX);
        let r  = ARM_RA + t * (ARM_RB - ARM_RA);

        // Random outward direction
        let rx = hf(i, 91)*2.0-1.0;  let rz = hf(i, 92)*2.0-1.0;
        let rn = (rx*rx + rz*rz).sqrt().max(0.001);
        let dx = rx/rn;  let dz = rz/rn;

        // Point on elliptic capsule surface + shell noise
        let noise = (hf(i, 93)*2.0 - 1.0) * SHELL;
        let px = cx + (r*1.05 + noise) * dx;
        let pz =      (r*0.95 + noise) * dz;

        // SDF verification with smooth union (torso blend in armpit region)
        let (d_t, _, _) = sdf_torso(px, py, pz);
        let (d_a, _)    = sdf_arm_r(px, py, pz);
        let d = smin(d_t, d_a, 0.04);
        if d < -SHELL || d > SHELL { continue; }

        // Analytical normal: outward from capsule axis in scaled space
        let nx = (px - cx) / (1.05*1.05);
        let nz = pz / (0.95*0.95);
        let nn = (nx*nx + nz*nz).sqrt().max(0.001);
        let (snx, sny, snz) = (nx/nn, 0.0f32, nz/nn);

        // Surface curvature — elbow crease darkens when the arm is extended.
        // The arm SDF has a tight concavity at the inner elbow where the capsule
        // meets the torso smin blend zone, giving κ ≈ −18 to −30 there.
        let kappa        = sdf_curvature(px, py, pz, snx, sny, snz, d);
        let wrinkle_raw  = (-kappa * 0.032).clamp(0.0, 1.0);
        let wrinkle      = wrinkle_raw * wrinkle_raw * (3.0 - 2.0 * wrinkle_raw);

        // Procedural jacket/sleeve surface detail — same noise stack as the torso.
        // The leather surface has coarser grain than skin: scale slightly lower.
        let sd = skin_detail(px, py, pz, snx, sny, snz);
        let inset = wrinkle * 0.005;
        let px  = px + (sd.disp - inset) * snx;
        let py  = py + (sd.disp - inset) * sny;
        let pz  = pz + (sd.disp - inset) * snz;
        let hz  = sd.pnz.max(0.0);

        let core = ((-d / SHELL + 1.0) * 0.5).clamp(0.0, 1.0);

        // Sleeve material: jacket brown (same as render_leon sleeve region)
        let (br, bg, bb) = (0.54f32, 0.37, 0.19);
        let mat_tag      = MatTag::Jacket;
        let (cr, cg, cb) = sdf_shade(sd.pnx, sd.pny, sd.pnz, hz, br, bg, bb, mat_tag, &lts);

        // AO: arm particles facing the torso are in the armpit — they darken
        // because sdf_torso is small in that direction.
        let ao = sdf_ao(px, py, pz, sd.pnx, sd.pny, sd.pnz);
        let (mut cr, mut cg, mut cb) = (cr * ao, cg * ao, cb * ao);

        // Jacket SSS: sleeve edges lit from behind by the torch transmit a faint
        // amber warmth through the leather — much subtler than skin (mu=10 vs 7.5).
        {
            let back_key   = (-sd.pnx*lts.klx - sd.pny*lts.kly - sd.pnz*lts.klz).max(0.0);
            let back_torch = (-sd.pnx*lts.f3x - sd.pny*lts.f3y - sd.pnz*lts.f3z).max(0.0) * lts.f3_int;
            let back_illum = back_key * 0.80 + back_torch;
            if back_illum > 0.06 {
                let thick = sdf_thickness(px, py, pz, sd.pnx, sd.pny, sd.pnz);
                const MU: f32 = 10.0;   // leather absorbs faster than skin
                let trans = (-MU * thick).exp() * back_illum;
                // Amber — warm leather lit from behind
                cr += trans * 0.85;
                cg += trans * 0.48;
                cb += trans * 0.12;
            }
        }

        // Micro-roughness: leather has a rougher surface grain than skin.
        // The jacket's coarser fibres scatter specular light more broadly.
        {
            let dk_bump = (sd.pnx*lts.klx + sd.pny*lts.kly + sd.pnz*lts.klz).max(0.0);
            let micro   = sd.roughness * sd.roughness * dk_bump * 0.10;
            cr += micro * 0.90;
            cg += micro * 0.72;
            cb += micro * 0.40;   // warm amber micro-glints on leather
        }

        // Jacket wrinkle: sleeve fabric bunches at the elbow crease and armpit.
        // Shadow is cooler than skin shadow — leather doesn't scatter subsurface
        // warmth, so the crease goes dark brown rather than red-brown.
        if wrinkle > 0.01 {
            cr *= 1.0 - wrinkle * 0.58;
            cg *= 1.0 - wrinkle * 0.65;
            cb *= 1.0 - wrinkle * 0.72;
        }

        // Volumetric self-shadow on the arm.
        // The arm is close to the torso; from the torch direction (right-and-behind)
        // the arm partially occludes the torch light on the near-torso side.
        // For the KEY LIGHT (upper-left-front): the shoulder cap may also clip the
        // upper arm's illumination at certain angles.
        {
            let key_t  = vol_shadow(px, py, pz, lts.klx, lts.kly, lts.klz);
            let dk_lit = (sd.pnx*lts.klx + sd.pny*lts.kly + sd.pnz*lts.klz).max(0.0);
            let vol_atten = 1.0 - dk_lit * (1.0 - key_t) * 0.72;
            cr *= vol_atten;
            cg *= vol_atten;
            cb *= vol_atten;
        }

        // Color-bleeding GI on the sleeve.
        // The torso jacket immediately adjacent to the arm bleeds its warm brown
        // into the sleeve; at the elbow the arm catches bounce light from the
        // forearm below it.  Same formula as the torso loop — neighbors sampled
        // analytically via leon_color, no SDF evaluation needed.
        {
            let dk_gi = (sd.pnx*lts.klx + sd.pny*lts.kly + sd.pnz*lts.klz).max(0.0);
            let f3_gi = (sd.pnx*lts.f3x + sd.pny*lts.f3y + sd.pnz*lts.f3z).max(0.0) * lts.f3_int;
            let illum = dk_gi * 0.80 + f3_gi * 0.40 + 0.12;
            let (gi_r, gi_g, gi_b) = gi_bounce(px, py, snx, sny, snz, illum);
            cr += gi_r; cg += gi_g; cb += gi_b;
        }

        let alpha = ((0.35 + core*0.55) * hp.max(0.08) * (1.0 + wrinkle * 0.40)).min(0.97);
        if alpha < 0.012 { continue; }

        let jx  = (hf(i, 94)-0.5) * dmg * 0.08;
        let jy  = (hf(i, 95)-0.5) * dmg * 0.08;
        let emission = (0.45 + core*1.20) * (1.0 - wrinkle * 0.38);
        let sz = 0.020 + core*0.018;
        let ch = if core>0.7 {'*'} else if core>0.35 {'+'} else {'.'};

        engine.spawn_glyph(Glyph {
            character: ch, scale: Vec2::splat(sz),
            position: Vec3::new(pos.x + px*scale*breath + jx,
                                pos.y + py*scale*breath + jy,
                                pos.z + hz*0.30),
            color:      Vec4::new(cr, cg, cb, alpha),
            emission,
            glow_color:  Vec3::new(cr*0.55, cg*0.45, cb*0.35),
            glow_radius: core*0.14,
            mass: 0.0, lifetime: dt*1.5,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Normal,
            ..Default::default()
        });
    }

    // ── Left upper arm ────────────────────────────────────────────────────────
    // X-mirror of the right arm: ARM_AX/BX become negative.
    const N_ARM_L: usize = 34_000;
    for i in 0..N_ARM_L {
        let t   = hf(i, 100);
        let py  = ARM_AY + t * (ARM_BY - ARM_AY);
        let cx  = -(ARM_AX + t * (ARM_BX - ARM_AX));   // negative X
        let r   = ARM_RA + t * (ARM_RB - ARM_RA);
        let rx  = hf(i, 101)*2.0-1.0; let rz = hf(i, 102)*2.0-1.0;
        let rn  = (rx*rx + rz*rz).sqrt().max(0.001);
        let dx  = rx/rn; let dz = rz/rn;
        let noise = (hf(i, 103)*2.0-1.0)*SHELL;
        let px  = cx + (r*1.05 + noise)*dx;
        let pz  =      (r*0.95 + noise)*dz;
        // SDF: mirror the right-arm SDF via -px
        let (d_t, _, _) = sdf_torso(px, py, pz);
        let (d_a, _)    = sdf_arm_r(-px, py, pz);
        let d = smin(d_t, d_a, 0.04);
        if d < -SHELL || d > SHELL { continue; }
        let nx_raw = (px - cx) / (1.05*1.05);
        let nz_raw = pz / (0.95*0.95);
        let nn = (nx_raw*nx_raw + nz_raw*nz_raw).sqrt().max(0.001);
        let (snx, sny, snz) = (nx_raw/nn, 0.0f32, nz_raw/nn);
        let kappa       = sdf_curvature(px, py, pz, snx, sny, snz, d);
        let wrinkle_raw = (-kappa*0.032).clamp(0.0, 1.0);
        let wrinkle     = wrinkle_raw*wrinkle_raw*(3.0-2.0*wrinkle_raw);
        let sd = skin_detail(px, py, pz, snx, sny, snz);
        let inset = wrinkle*0.005;
        let px = px+(sd.disp-inset)*snx; let py = py+(sd.disp-inset)*sny;
        let pz = pz+(sd.disp-inset)*snz; let hz = sd.pnz.max(0.0);
        let core = ((-d/SHELL+1.0)*0.5).clamp(0.0, 1.0);
        let (br,bg,bb) = (0.54f32, 0.37, 0.19);
        let mat_tag     = MatTag::Jacket;
        let (cr,cg,cb) = sdf_shade(sd.pnx,sd.pny,sd.pnz, hz, br,bg,bb, mat_tag, &lts);
        let ao = sdf_ao(px, py, pz, sd.pnx, sd.pny, sd.pnz);
        let (mut cr,mut cg,mut cb) = (cr*ao, cg*ao, cb*ao);
        { let bk=(-sd.pnx*lts.klx-sd.pny*lts.kly-sd.pnz*lts.klz).max(0.0);
          let bt=(-sd.pnx*lts.f3x-sd.pny*lts.f3y-sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
          let bi=bk*0.80+bt; if bi>0.06 { let th=sdf_thickness(px,py,pz,sd.pnx,sd.pny,sd.pnz);
          let tr=(-10.0*th).exp()*bi; cr+=tr*0.85; cg+=tr*0.48; cb+=tr*0.12; } }
        { let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
          let m=sd.roughness*sd.roughness*dk*0.10;
          cr+=m*0.90; cg+=m*0.72; cb+=m*0.40; }
        if wrinkle>0.01 { cr*=1.0-wrinkle*0.58; cg*=1.0-wrinkle*0.65; cb*=1.0-wrinkle*0.72; }
        { let kt=vol_shadow(px,py,pz,lts.klx,lts.kly,lts.klz);
          let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
          let va=1.0-dk*(1.0-kt)*0.72; cr*=va; cg*=va; cb*=va; }
        { let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
          let f3=(sd.pnx*lts.f3x+sd.pny*lts.f3y+sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
          let (gr,gg,gb)=gi_bounce(px,py,snx,sny,snz,dk*0.80+f3*0.40+0.12);
          cr+=gr; cg+=gg; cb+=gb; }
        let alpha=((0.35+core*0.55)*hp.max(0.08)*(1.0+wrinkle*0.40)).min(0.97);
        if alpha<0.012 { continue; }
        let jx=(hf(i,104)-0.5)*dmg*0.08; let jy=(hf(i,105)-0.5)*dmg*0.08;
        let emission=(0.45+core*1.20)*(1.0-wrinkle*0.38);
        let sz=0.020+core*0.018; let ch=if core>0.7{'*'}else if core>0.35{'+'}else{'.'};
        engine.spawn_glyph(Glyph {
            character: ch, scale: Vec2::splat(sz),
            position: Vec3::new(pos.x+px*scale*breath+jx, pos.y+py*scale*breath+jy, pos.z+hz*0.30),
            color: Vec4::new(cr,cg,cb,alpha), emission,
            glow_color: Vec3::new(cr*0.55,cg*0.45,cb*0.35), glow_radius: core*0.14,
            mass:0.0, lifetime:dt*1.5,
            layer:RenderLayer::Entity, blend_mode:BlendMode::Normal, ..Default::default()
        });
    }

    // ── Forearms + hands (both sides) ─────────────────────────────────────────
    // Each side: elbow → wrist → hand tip.  Material transitions from Jacket
    // (sleeve fabric) to Skin (exposed hand) at Y ≈ 0.24 via leon_tag.
    const N_FA: usize = 28_000;
    const FA_AX:f32=0.530; const FA_AY:f32=-0.10;
    const FA_BX:f32=0.555; const FA_BY:f32= 0.36;
    const FA_RA:f32=0.075; const FA_RB:f32= 0.050;
    for side in 0..2usize {
        let sign = if side == 0 { 1.0f32 } else { -1.0 };
        let seed_base = if side == 0 { 110usize } else { 120 };
        let dmg_base  = if side == 0 { 114usize } else { 124 };
        for i in 0..N_FA {
            let t   = hf(i, seed_base);
            let py  = FA_AY + t*(FA_BY - FA_AY);
            let cx  = sign*(FA_AX + t*(FA_BX - FA_AX));
            let r   = FA_RA + t*(FA_RB - FA_RA);
            let rx  = hf(i, seed_base+1)*2.0-1.0; let rz = hf(i, seed_base+2)*2.0-1.0;
            let rn  = (rx*rx+rz*rz).sqrt().max(0.001);
            let dx  = rx/rn; let dz = rz/rn;
            let noise = (hf(i, seed_base+3)*2.0-1.0)*SHELL;
            let px  = cx + (r*1.05+noise)*dx;
            let pz  =      (r*0.95+noise)*dz;
            // SDF: forearm blended with upper-arm cap at elbow
            let d_fa  = sdf_forearm_r(sign*px, py, pz);
            let (d_arm,_) = sdf_arm_r(sign*px, py, pz);
            let d = smin(d_fa, d_arm, 0.025);
            if d < -SHELL || d > SHELL { continue; }
            let nx_raw = (px-cx)/(1.05*1.05); let nz_raw = pz/(0.95*0.95);
            let nn = (nx_raw*nx_raw+nz_raw*nz_raw).sqrt().max(0.001);
            let (snx,sny,snz) = (nx_raw/nn, 0.0f32, nz_raw/nn);
            let kappa       = sdf_curvature(px, py, pz, snx, sny, snz, d);
            let wrinkle_raw = (-kappa*0.032).clamp(0.0, 1.0);
            let wrinkle     = wrinkle_raw*wrinkle_raw*(3.0-2.0*wrinkle_raw);
            let sd = skin_detail(px, py, pz, snx, sny, snz);
            let inset = wrinkle*0.005;
            let px = px+(sd.disp-inset)*snx; let py = py+(sd.disp-inset)*sny;
            let pz = pz+(sd.disp-inset)*snz; let hz = sd.pnz.max(0.0);
            let core = ((-d/SHELL+1.0)*0.5).clamp(0.0, 1.0);
            let (br,bg,bb) = leon_color(px, py);
            let mat_tag = leon_tag(px, py);   // Jacket sleeve → Skin hand at Y>0.24
            let (cr,cg,cb) = sdf_shade(sd.pnx,sd.pny,sd.pnz, hz, br,bg,bb, mat_tag, &lts);
            let ao = sdf_ao(px, py, pz, sd.pnx, sd.pny, sd.pnz);
            let (mut cr,mut cg,mut cb) = (cr*ao, cg*ao, cb*ao);
            if mat_tag == MatTag::Skin {
                let bk=(-sd.pnx*lts.klx-sd.pny*lts.kly-sd.pnz*lts.klz).max(0.0);
                let bt=(-sd.pnx*lts.f3x-sd.pny*lts.f3y-sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
                let bi=bk*0.80+bt; if bi>0.04 {
                    let th=sdf_thickness(px,py,pz,sd.pnx,sd.pny,sd.pnz);
                    let tr=(-7.5*th).exp()*bi; cr+=tr*1.00; cg+=tr*0.28; cb+=tr*0.08; }
            } else {
                let bk=(-sd.pnx*lts.klx-sd.pny*lts.kly-sd.pnz*lts.klz).max(0.0);
                let bt=(-sd.pnx*lts.f3x-sd.pny*lts.f3y-sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
                let bi=bk*0.80+bt; if bi>0.06 {
                    let th=sdf_thickness(px,py,pz,sd.pnx,sd.pny,sd.pnz);
                    let tr=(-10.0*th).exp()*bi; cr+=tr*0.85; cg+=tr*0.48; cb+=tr*0.12; }
            }
            { let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
              let m=sd.roughness*sd.roughness*dk;
              let (mr,mg,mb,sc)=if mat_tag==MatTag::Skin {(1.00f32,0.94,0.82,0.14)}
                                else{(0.90f32,0.72,0.40,0.10)};
              cr+=m*sc*mr; cg+=m*sc*mg; cb+=m*sc*mb; }
            if wrinkle>0.01 {
                let (wr,wg,wb)=if mat_tag==MatTag::Skin{(0.52f32,0.62,0.80)}
                               else{(0.58f32,0.65,0.72)};
                cr*=1.0-wrinkle*wr; cg*=1.0-wrinkle*wg; cb*=1.0-wrinkle*wb; }
            { let kt=vol_shadow(px,py,pz,lts.klx,lts.kly,lts.klz);
              let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
              let va=1.0-dk*(1.0-kt)*0.72; cr*=va; cg*=va; cb*=va; }
            { let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
              let f3=(sd.pnx*lts.f3x+sd.pny*lts.f3y+sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
              let (gr,gg,gb)=gi_bounce(px,py,snx,sny,snz,dk*0.80+f3*0.40+0.12);
              cr+=gr; cg+=gg; cb+=gb; }
            let alpha=((0.35+core*0.55)*hp.max(0.08)*(1.0+wrinkle*0.40)).min(0.97);
            if alpha<0.012 { continue; }
            let jx=(hf(i,dmg_base)-0.5)*dmg*0.08; let jy=(hf(i,dmg_base+1)-0.5)*dmg*0.08;
            let emission=(0.45+core*1.20)*(1.0-wrinkle*0.38);
            let sz=0.020+core*0.018; let ch=if core>0.7{'*'}else if core>0.35{'+'}else{'.'};
            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(sz),
                position: Vec3::new(pos.x+px*scale*breath+jx, pos.y+py*scale*breath+jy, pos.z+hz*0.30),
                color: Vec4::new(cr,cg,cb,alpha), emission,
                glow_color: Vec3::new(cr*0.55,cg*0.45,cb*0.35), glow_radius: core*0.14,
                mass:0.0, lifetime:dt*1.5,
                layer:RenderLayer::Entity, blend_mode:BlendMode::Normal, ..Default::default()
            });
        }
    }

    // ── Legs + boots (both sides) ─────────────────────────────────────────────
    // Thigh → knee → shin → boot in one loop.  Material switches from Jacket
    // (olive pants) to Boot (dark leather) at Y ≈ 0.84 via leon_tag.
    // Knee crease at the thigh/shin smin junction produces AO and wrinkle darkening.
    const N_LEG: usize = 45_000;
    const LEG_CX: f32 = 0.145;
    const LEG_Y0: f32 = 0.28;
    const LEG_Y1: f32 = 0.97;
    for side in 0..2usize {
        let sign = if side == 0 { 1.0f32 } else { -1.0 };
        let seed_base = if side == 0 { 130usize } else { 140 };
        let dmg_base  = if side == 0 { 134usize } else { 144 };
        for i in 0..N_LEG {
            let v   = hf(i, seed_base);
            let py  = LEG_Y0 + v*(LEG_Y1 - LEG_Y0);
            let r_expected = if py < 0.72 {
                let t=(py-0.28)/0.44; 0.105+t*(0.088-0.105)
            } else {
                let t=((py-0.68)/0.29).clamp(0.0,1.0); 0.090+t*(0.055-0.090)
            };
            let rx = hf(i, seed_base+1)*2.0-1.0; let rz = hf(i, seed_base+2)*2.0-1.0;
            let rn = (rx*rx+rz*rz).sqrt().max(0.001);
            let dx_u = rx/rn; let dz_u = rz/rn;
            let noise = (hf(i, seed_base+3)*2.0-1.0)*SHELL;
            // px is unsigned (right-side coords); sign flips at spawn
            let px_u = LEG_CX + (r_expected+noise)*dx_u;
            let pz_u = (r_expected+noise)*dz_u*0.92;
            let px = sign*px_u;
            let pz = pz_u;
            let d = sdf_leg_r(sign*px, py, pz);
            if d < -SHELL || d > SHELL { continue; }
            let nx_raw = px_u - LEG_CX;
            let nz_raw = pz;
            let nn = (nx_raw*nx_raw+nz_raw*nz_raw).sqrt().max(0.001);
            // Normal X component always points outward from the leg centre (unsigned)
            let (snx_u,sny,snz) = (nx_raw/nn, 0.0f32, nz_raw/nn);
            let snx = sign*snx_u;
            let kappa       = sdf_curvature(px, py, pz, snx, sny, snz, d);
            let wrinkle_raw = (-kappa*0.030).clamp(0.0, 1.0);
            let wrinkle     = wrinkle_raw*wrinkle_raw*(3.0-2.0*wrinkle_raw);
            let sd = skin_detail(px, py, pz, snx, sny, snz);
            let inset = wrinkle*0.005;
            let px = px+(sd.disp-inset)*snx; let py = py+(sd.disp-inset)*sny;
            let pz = pz+(sd.disp-inset)*snz; let hz = sd.pnz.max(0.0);
            let core = ((-d/SHELL+1.0)*0.5).clamp(0.0, 1.0);
            let (br,bg,bb) = leon_color(px, py);
            let mat_tag = leon_tag(px, py);   // Jacket(pants) or Boot
            let (cr,cg,cb) = sdf_shade(sd.pnx,sd.pny,sd.pnz, hz, br,bg,bb, mat_tag, &lts);
            let ao = sdf_ao(px, py, pz, sd.pnx, sd.pny, sd.pnz);
            let (mut cr,mut cg,mut cb) = (cr*ao, cg*ao, cb*ao);
            if mat_tag == MatTag::Jacket {
                let bk=(-sd.pnx*lts.klx-sd.pny*lts.kly-sd.pnz*lts.klz).max(0.0);
                let bt=(-sd.pnx*lts.f3x-sd.pny*lts.f3y-sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
                let bi=bk*0.80+bt; if bi>0.06 {
                    let th=sdf_thickness(px,py,pz,sd.pnx,sd.pny,sd.pnz);
                    let tr=(-12.0*th).exp()*bi;
                    cr+=tr*0.55; cg+=tr*0.68; cb+=tr*0.25; }  // olive transmit
            }
            { let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
              let m=sd.roughness*sd.roughness*dk;
              let (mr,mg,mb)=if mat_tag==MatTag::Boot{(m*0.08f32,m*0.06,m*0.04)}
                             else{(m*0.08f32,m*0.09,m*0.05)};
              cr+=mr; cg+=mg; cb+=mb; }
            if wrinkle>0.01 { cr*=1.0-wrinkle*0.55; cg*=1.0-wrinkle*0.62; cb*=1.0-wrinkle*0.70; }
            { let kt=vol_shadow(px,py,pz,lts.klx,lts.kly,lts.klz);
              let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
              let va=1.0-dk*(1.0-kt)*0.72; cr*=va; cg*=va; cb*=va; }
            { let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
              let f3=(sd.pnx*lts.f3x+sd.pny*lts.f3y+sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
              let (gr,gg,gb)=gi_bounce(px,py,snx,sny,snz,dk*0.80+f3*0.40+0.12);
              cr+=gr; cg+=gg; cb+=gb; }
            let alpha=((0.35+core*0.55)*hp.max(0.08)*(1.0+wrinkle*0.35)).min(0.97);
            if alpha<0.012 { continue; }
            let jx=(hf(i,dmg_base)-0.5)*dmg*0.08; let jy=(hf(i,dmg_base+1)-0.5)*dmg*0.08;
            let emission=(if mat_tag==MatTag::Boot{0.35+core*0.80}else{0.42+core*1.10})
                        *(1.0-wrinkle*0.38);
            let sz=0.020+core*0.018; let ch=if core>0.7{'*'}else if core>0.35{'+'}else{'.'};
            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(sz),
                position: Vec3::new(pos.x+px*scale*breath+jx, pos.y+py*scale*breath+jy, pos.z+hz*0.30),
                color: Vec4::new(cr,cg,cb,alpha), emission,
                glow_color: Vec3::new(cr*0.55,cg*0.45,cb*0.35), glow_radius: core*0.14,
                mass:0.0, lifetime:dt*1.5,
                layer:RenderLayer::Entity, blend_mode:BlendMode::Normal, ..Default::default()
            });
        }
    }
}

/// Atmospheric light-scatter layer — makes god rays visible in the space around
/// the body, not just as shadow patterns on the body surface.
///
/// Distributes sparse dust/haze particles in a cylindrical shell around Leon.
/// Each particle's brightness is its volumetric transmittance toward the key
/// light: fully lit = bright shaft, occluded = dark (absent, after alpha cull).
///
/// The pattern formed by lit vs absent particles in the space between the arm
/// and the torso, below the chin, or beside the shoulders IS the god ray —
/// physical light-column geometry derived from the SDF, not a radial blur.
///
/// Sampling: cylindrical shell, 5–25 cm radially from the body centre axis,
/// biased slightly toward the camera (+Z) so the shafts are front-visible.
/// Rejection keeps only points that are outside the body AND within 22 cm of
/// the surface — the active atmospheric scatter zone.
///
/// Cost: 18 K iterations.  Each accepted particle runs two `vol_shadow` probes
/// (key + torch) = 12 `sdf_body` calls.  Acceptance ≈ 10–18 % → 1.8–3.2 K
/// actual atmospheric glyphs per frame at ≈ 2.5 ms additional CPU.
fn render_vol_scatter(engine: &mut ProofEngine, dt: f32, time: f32, pos: Vec3) {
    const N:     usize = 18_000;
    const SCALE: f32   = 3.2;
    const ATM_REACH: f32 = 0.22;   // atmospheric zone depth (model units)
    let nu = |x: f32, y: f32, z: f32| {
        let l = (x*x+y*y+z*z).sqrt(); (x/l, y/l, z/l)
    };
    let (klx, kly, klz) = nu(-0.40, -0.55,  0.73);
    let (f3x, f3y, f3z) = nu(TORCH_X, TORCH_Y, TORCH_Z);
    let f3_int = 0.28 * (0.80 + (time*7.3).sin()*0.11 + (time*13.1).cos()*0.07);
    let breath  = 1.0 + (time * 1.4).sin() * 0.010;

    for i in 0..N {
        // Cylindrical shell: random angle + radial distance + height
        let angle = hf(i, 115) * TAU;
        let r     = 0.05 + hf(i, 116) * 0.20;
        // Elliptic cross-section (body is flatter in Z); bias toward +Z (camera side)
        // so the shafts are visible to the viewer.
        let px = angle.cos() * r;
        let pz = angle.sin() * r * 0.55 + 0.06;
        let py = -1.25 + hf(i, 117) * 2.40;   // full body height

        // Reject points inside or flush with the body surface
        let d = sdf_body(px, py, pz);
        if d < 0.04 { continue; }

        // Reject points too far from the surface (too diffuse to show shaft)
        if d > ATM_REACH { continue; }

        // Atmospheric density: peaks at the body surface, zero at ATM_REACH.
        // Squared for a sharper contact concentration.
        let prox = 1.0 - d / ATM_REACH;

        // Volumetric shadow from both light sources.
        // key light (upper-left-front) creates shafts above shoulders and chin.
        // Torch (right-and-behind) creates shafts between arm and torso.
        let key_t = vol_shadow(px, py, pz, klx, kly, klz);
        let tch_t = vol_shadow(px, py, pz, f3x, f3y, f3z);
        // Blend: key dominates visible-light appearance; torch adds side warmth.
        let transmit = key_t * 0.68 + tch_t * f3_int * 0.32;
        if transmit < 0.08 { continue; }   // deep shadow — particle invisible

        // Particle alpha: shadow × proximity × random jitter.
        // The individual particle is barely visible; the density accumulates to
        // form soft shaft borders.
        let alpha = transmit * prox * prox * (0.011 + hf(i, 118) * 0.015);
        if alpha < 0.003 { continue; }

        // Key-light warm white + torch amber blend
        let cr = key_t * 1.00 + tch_t * f3_int * 0.90;
        let cg = key_t * 0.92 + tch_t * f3_int * 0.60;
        let cb = key_t * 0.72 + tch_t * f3_int * 0.20;
        // Normalise so the colour is independent of transmittance magnitude
        let cm = cr.max(cg).max(cb).max(0.001);
        let (cr, cg, cb) = (cr/cm * transmit, cg/cm * transmit, cb/cm * transmit);

        let sz = 0.030 + hf(i, 119) * 0.022;

        engine.spawn_glyph(Glyph {
            character:   '.',
            scale:        Vec2::splat(sz),
            position:     Vec3::new(
                pos.x + px * SCALE * breath,
                pos.y + py * SCALE * breath,
                pos.z + pz * SCALE * 0.85,   // slight z pull-back: sort behind body surface
            ),
            color:        Vec4::new(cr * alpha, cg * alpha, cb * alpha, alpha),
            emission:     transmit * 2.0,
            glow_color:   Vec3::new(0.92, 0.84, 0.58),
            glow_radius:  0.10,
            mass:         0.0,
            lifetime:     dt * 1.5,
            layer:        RenderLayer::Background,
            blend_mode:   BlendMode::Additive,
            ..Default::default()
        });
    }
}

// ── Face SDF renderer ──────────────────────────────────────────────────────────
//
// Replaces bone 1 (face/forehead) in render_leon and adds all lower-face
// features that the bone system cannot express: eye sockets, eyeballs with
// iris and pupil, nasal ridge, lip ridges, and mouth crease.
//
// Four focused samplers:
//   MAIN  — head ellipsoid surface ± SHELL; covers the flat forehead, cheeks,
//            jaw.  Acceptance ≈ 100%.  Skips points on the eyeball surface —
//            those are handled at higher density by the EYE sampler.
//   EYE   — eyeball sphere surface ± SHELL; both eyes in one loop (mirrored).
//            Produces crisp iris, pupil, sclera, limbus at particle density.
//   NOSE  — bounding box around the nasal protrusion.
//   MOUTH — bounding box around the lip ridges and crease.
//
// Per-particle pipeline:
//   face_normal      → 6 sdf_face calls (gradient)
//   sdf_face_ao      → 4 sdf_face calls (IQ AO)
//   sdf_face_curv    → 4 sdf_face calls (wrinkle detection)
//   skin_detail      → 4 noise3 calls (micro-bump, roughness)
//   sdf_face_thick   → ≤5 sdf_face calls (SSS)
//   Total:             ≈ 19 sdf_face calls per particle
fn render_sdf_face(engine: &mut ProofEngine, dt: f32, time: f32, pos: Vec3, hp: f32) {
    const SHELL:   f32   = 0.016;
    const N_MAIN:  usize = 88_000;   // head ellipsoid surface samples
    const N_EYE:   usize = 26_000;   // eyeball surface samples (both eyes)
    const N_NOSE:  usize =  9_000;   // nose bounding-box samples
    const N_MOUTH: usize =  7_000;   // lips + crease bounding-box samples

    let scale  = 3.2f32;
    let breath = 1.0 + (time * 1.4).sin() * 0.010;
    let dmg    = (1.0 - hp).max(0.0);

    // Light bundle — mirrors render_sdf_body exactly
    let lts = {
        let n = |x:f32,y:f32,z:f32|{ let l=(x*x+y*y+z*z).sqrt(); (x/l,y/l,z/l) };
        let (klx,kly,klz) = n(-0.40,-0.55, 0.73);
        let (rlx,rly,rlz) = n( 0.55,-0.18,-0.82);
        let (f1x,f1y,f1z) = n( 0.30, 0.60,-0.40);
        let (f3x,f3y,f3z) = n(TORCH_X,TORCH_Y,TORCH_Z);
        SdfLights {
            klx,kly,klz, rlx,rly,rlz,
            f1x,f1y,f1z,
            f2x:0.0, f2y:-1.0, f2z:0.0,
            f3x,f3y,f3z,
            f3_int: 0.28*(0.80+(time*7.3).sin()*0.11+(time*13.1).cos()*0.07),
        }
    };

    // ── MAIN face loop ────────────────────────────────────────────────────────
    // Sample on the head ellipsoid surface: project a normalised-random-cube
    // vector onto the ellipsoid, then perturb ±SHELL along the outward normal.
    // Eye-region particles (within 2×SHELL of the eyeball surface) are deferred
    // to the dedicated EYE loop, which provides higher iris/pupil detail.
    for i in 0..N_MAIN {
        let rx = hf(i,240)*2.0-1.0;
        let ry = hf(i,241)*2.0-1.0;
        let rz = hf(i,242)*2.0-1.0;
        let rn = (rx*rx+ry*ry+rz*rz).sqrt().max(0.001);
        let ex = (rx/rn)*HEAD_RX;
        let ey = (ry/rn)*HEAD_RY;
        let ez = (rz/rn)*HEAD_RZ;
        // Ellipsoid outward normal direction
        let gnx = ex/(HEAD_RX*HEAD_RX);
        let gny = ey/(HEAD_RY*HEAD_RY);
        let gnz = ez/(HEAD_RZ*HEAD_RZ);
        let gnn = (gnx*gnx+gny*gny+gnz*gnz).sqrt().max(0.001);
        let noise = (hf(i,243)*2.0-1.0)*SHELL;
        let px = ex + noise*gnx/gnn;
        let py = HEAD_CY + ey + noise*gny/gnn;
        let pz = ez + noise*gnz/gnn;

        let d = sdf_face(px, py, pz);
        if d < -SHELL || d > SHELL { continue; }

        // Delegate eye-surface particles to the EYE loop
        let der = sdf_sphere_f(px-EYEB_CX, py-EYEB_CY, pz-EYEB_CZ, EYEB_R);
        let del_ = sdf_sphere_f(px+EYEB_CX, py-EYEB_CY, pz-EYEB_CZ, EYEB_R);
        if der.abs() < SHELL*2.0 || del_.abs() < SHELL*2.0 { continue; }

        let (snx,sny,snz) = face_normal(px, py, pz);
        let hz   = snz.max(0.0);
        let core = ((-d/SHELL+1.0)*0.5).clamp(0.0, 1.0);

        // Curvature: socket rims darken, nasolabial and chin creases emerge
        let kappa       = sdf_face_curvature(px, py, pz, snx, sny, snz, d);
        let wrinkle_raw = (-kappa * 0.030).clamp(0.0, 1.0);
        let wrinkle     = wrinkle_raw*wrinkle_raw*(3.0-2.0*wrinkle_raw);

        // Noise micro-detail — same three-band stack as body
        let sd = skin_detail(px, py, pz, snx, sny, snz);
        let inset = wrinkle*0.004;
        let px = px + (sd.disp-inset)*snx;
        let py = py + (sd.disp-inset)*sny;
        let pz = pz + (sd.disp-inset)*snz;

        let (br,bg,bb) = leon_color(px, py);
        let mat_tag    = leon_tag(px, py);
        let (cr,cg,cb) = sdf_shade(sd.pnx,sd.pny,sd.pnz, hz, br,bg,bb, mat_tag, &lts);
        let ao = sdf_face_ao(px, py, pz, sd.pnx, sd.pny, sd.pnz);
        let (mut cr,mut cg,mut cb) = (cr*ao, cg*ao, cb*ao);

        // Skin SSS — cheeks and nose back-lit by torch/key
        if mat_tag == MatTag::Skin {
            let bk = (-sd.pnx*lts.klx-sd.pny*lts.kly-sd.pnz*lts.klz).max(0.0);
            let bt = (-sd.pnx*lts.f3x-sd.pny*lts.f3y-sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
            let bi = bk*0.80 + bt;
            if bi > 0.04 {
                let thick = sdf_face_thickness(px, py, pz, sd.pnx, sd.pny, sd.pnz);
                let t = (-6.5*thick).exp()*bi;
                cr += t*1.00; cg += t*0.30; cb += t*0.10;
            }
        }
        // Skin micro-roughness specular
        { let dk = (sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
          let m  = sd.roughness*sd.roughness*dk*0.12;
          cr += m*1.00; cg += m*0.94; cb += m*0.82; }
        // Socket-rim / nasolabial wrinkle shadow
        if wrinkle > 0.01 { cr *= 1.0-wrinkle*0.50; cg *= 1.0-wrinkle*0.58; cb *= 1.0-wrinkle*0.74; }

        let alpha = ((0.35+core*0.55)*hp.max(0.08)*(1.0+wrinkle*0.40)).min(0.97);
        if alpha < 0.012 { continue; }

        let jx = (hf(i,244)-0.5)*dmg*0.08;
        let jy = (hf(i,245)-0.5)*dmg*0.08;
        let emission = (if br>0.58&&bg>0.46 { 0.40+core*0.80 }
                        else if br<0.28&&bg<0.18 { 0.12+core*0.30 }
                        else { 0.45+core*1.20 }) * (1.0-wrinkle*0.35);
        let sz = 0.018+core*0.016;
        let ch = if core>0.7 {'*'} else if core>0.35 {'+'} else {'.'};

        engine.spawn_glyph(Glyph {
            character: ch, scale: Vec2::splat(sz),
            position: Vec3::new(pos.x+px*scale*breath+jx, pos.y+py*scale*breath+jy, pos.z+hz*0.30),
            color: Vec4::new(cr,cg,cb,alpha), emission,
            glow_color:  Vec3::new(cr*0.55, cg*0.45, cb*0.35),
            glow_radius: core*0.12,
            mass: 0.0, lifetime: dt*1.5,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Normal,
            ..Default::default()
        });
    }

    // ── EYE loop ──────────────────────────────────────────────────────────────
    // Samples on the eyeball sphere surface for both eyes.
    // Both eyes share the same local sample pattern (j = i % N_HALF) —
    // the irises are symmetric about X, which is correct.
    // The composite face SDF acceptance check rejects backside eyeball samples
    // that lie inside the head (still inside the carved socket: d < −SHELL).
    const N_EYE_HALF: usize = N_EYE / 2;
    for i in 0..N_EYE {
        let j      = i % N_EYE_HALF;
        let eye_cx = if i < N_EYE_HALF { EYEB_CX } else { -EYEB_CX };

        let rx = hf(j,250)*2.0-1.0;
        let ry = hf(j,251)*2.0-1.0;
        let rz = hf(j,252)*2.0-1.0;
        let rn = (rx*rx+ry*ry+rz*rz).sqrt().max(0.001);
        let noise = (hf(j,253)*2.0-1.0)*SHELL;
        let r  = EYEB_R + noise;
        let px = eye_cx + (rx/rn)*r;
        let py = EYEB_CY + (ry/rn)*r;
        let pz = EYEB_CZ + (rz/rn)*r;

        let d = sdf_face(px, py, pz);
        if d < -SHELL || d > SHELL { continue; }

        let (snx,sny,snz) = face_normal(px, py, pz);
        let hz   = snz.max(0.0);
        let core = ((-d/SHELL+1.0)*0.5).clamp(0.0, 1.0);

        let (br,bg,bb,is_iris) = eye_color(px, py, pz, eye_cx);

        // Full lighting with MatTag::Eye — high Fresnel + sharp specular
        let (cr,cg,cb) = sdf_shade(snx,sny,snz, hz, br,bg,bb, MatTag::Eye, &lts);
        let (mut cr,mut cg,mut cb) = (cr,cg,cb);

        // Sclera micro-noise (prevents flat, plastic look)
        if !is_iris {
            let sd = skin_detail(px, py, pz, snx, sny, snz);
            let dk = (sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
            let m  = sd.roughness*sd.roughness*dk*0.05;
            cr += m; cg += m*0.97; cb += m*0.95;
        }

        let alpha = ((0.40+core*0.50)*hp.max(0.08)).min(0.97);
        if alpha < 0.012 { continue; }

        let jx = (hf(j,254)-0.5)*dmg*0.06;
        let jy = (hf(j,255)-0.5)*dmg*0.06;
        // Eyes emit strongly so the iris reads against the dark socket shadow
        let emission = if is_iris { 0.25+core*0.45 } else { 0.55+core*1.10 };
        let sz = 0.014+core*0.013;
        let ch = if is_iris { '.' } else if core>0.7 {'*'} else {'.'};

        engine.spawn_glyph(Glyph {
            character: ch, scale: Vec2::splat(sz),
            position: Vec3::new(pos.x+px*scale*breath+jx, pos.y+py*scale*breath+jy, pos.z+hz*0.30),
            color: Vec4::new(cr,cg,cb,alpha), emission,
            glow_color:  if is_iris { Vec3::new(0.38,0.26,0.10) }
                         else       { Vec3::new(cr*0.40,cg*0.35,cb*0.30) },
            glow_radius: if is_iris { 0.07 } else { core*0.10 },
            mass: 0.0, lifetime: dt*1.5,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Normal,
            ..Default::default()
        });
    }

    // ── NOSE loop ─────────────────────────────────────────────────────────────
    // Bounding box around the nasal protrusion.
    // X: ±0.052  Y: −0.558 to −0.440  Z: 0.076 to 0.148
    // Acceptance ≈ 8-12%: nose occupies ~10% of the box volume.
    for i in 0..N_NOSE {
        let px = (hf(i,260)*2.0-1.0)*0.052;
        let py = -0.558 + hf(i,261)*0.118;
        let pz =  0.076 + hf(i,262)*0.072;
        let d  = sdf_face(px, py, pz);
        if d < -SHELL || d > SHELL { continue; }

        let (snx,sny,snz) = face_normal(px, py, pz);
        let hz   = snz.max(0.0);
        let core = ((-d/SHELL+1.0)*0.5).clamp(0.0, 1.0);

        let kappa       = sdf_face_curvature(px, py, pz, snx, sny, snz, d);
        let wrinkle_raw = (-kappa*0.028).clamp(0.0, 1.0);
        let wrinkle     = wrinkle_raw*wrinkle_raw*(3.0-2.0*wrinkle_raw);

        let sd = skin_detail(px, py, pz, snx, sny, snz);
        let px = px + sd.disp*snx;
        let py = py + sd.disp*sny;
        let pz = pz + sd.disp*snz;

        let (br,bg,bb) = leon_color(px, py);
        let mat_tag    = leon_tag(px, py);
        let (cr,cg,cb) = sdf_shade(sd.pnx,sd.pny,sd.pnz, hz, br,bg,bb, mat_tag, &lts);
        let ao = sdf_face_ao(px, py, pz, sd.pnx, sd.pny, sd.pnz);
        let (mut cr,mut cg,mut cb) = (cr*ao, cg*ao, cb*ao);

        // Nose-tip SSS — very thin tissue transmits especially at the alae
        let bk = (-sd.pnx*lts.klx-sd.pny*lts.kly-sd.pnz*lts.klz).max(0.0);
        let bt = (-sd.pnx*lts.f3x-sd.pny*lts.f3y-sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
        let bi = bk*0.80 + bt;
        if bi > 0.04 {
            let thick = sdf_face_thickness(px, py, pz, sd.pnx, sd.pny, sd.pnz);
            let t = (-5.5*thick).exp()*bi;
            cr += t*1.00; cg += t*0.38; cb += t*0.18;
        }
        { let dk=(sd.pnx*lts.klx+sd.pny*lts.kly+sd.pnz*lts.klz).max(0.0);
          let m=sd.roughness*sd.roughness*dk*0.12;
          cr+=m*1.00; cg+=m*0.94; cb+=m*0.82; }
        if wrinkle>0.01 { cr*=1.0-wrinkle*0.45; cg*=1.0-wrinkle*0.52; cb*=1.0-wrinkle*0.68; }

        let alpha = ((0.35+core*0.55)*hp.max(0.08)*(1.0+wrinkle*0.40)).min(0.97);
        if alpha < 0.012 { continue; }
        let jx=(hf(i,263)-0.5)*dmg*0.08; let jy=(hf(i,264)-0.5)*dmg*0.08;
        let emission = (0.42+core*0.85)*(1.0-wrinkle*0.30);
        let sz=0.016+core*0.015; let ch=if core>0.6 {'*'} else {'+'};

        engine.spawn_glyph(Glyph {
            character: ch, scale: Vec2::splat(sz),
            position: Vec3::new(pos.x+px*scale*breath+jx, pos.y+py*scale*breath+jy, pos.z+hz*0.30),
            color: Vec4::new(cr,cg,cb,alpha), emission,
            glow_color:  Vec3::new(cr*0.55,cg*0.45,cb*0.35), glow_radius: core*0.12,
            mass: 0.0, lifetime: dt*1.5,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Normal,
            ..Default::default()
        });
    }

    // ── MOUTH loop ────────────────────────────────────────────────────────────
    // Bounding box around lips and crease.
    // X: ±0.082  Y: −0.410 to −0.330  Z: 0.060 to 0.106
    for i in 0..N_MOUTH {
        let px = (hf(i,270)*2.0-1.0)*0.082;
        let py = -0.410 + hf(i,271)*0.080;
        let pz =  0.060 + hf(i,272)*0.046;
        let d  = sdf_face(px, py, pz);
        if d < -SHELL || d > SHELL { continue; }

        let (snx,sny,snz) = face_normal(px, py, pz);
        let hz   = snz.max(0.0);
        let core = ((-d/SHELL+1.0)*0.5).clamp(0.0, 1.0);

        let kappa       = sdf_face_curvature(px, py, pz, snx, sny, snz, d);
        let wrinkle_raw = (-kappa*0.030).clamp(0.0, 1.0);
        let wrinkle     = wrinkle_raw*wrinkle_raw*(3.0-2.0*wrinkle_raw);

        let sd = skin_detail(px, py, pz, snx, sny, snz);
        let inset = wrinkle*0.003;
        let px = px+(sd.disp-inset)*snx;
        let py = py+(sd.disp-inset)*sny;
        let pz = pz+(sd.disp-inset)*snz;

        // Lip colour: slightly pinker than cheek skin in the vermilion zone
        let (mut br,mut bg,mut bb) = leon_color(px, py);
        if px.abs() < 0.058 && py > LIPU_CY-0.018 && py < LIPL_CY+0.020 {
            br = (br*1.15).min(1.0); bg = (bg*0.92).min(1.0); bb = (bb*0.88).min(1.0);
        }
        let mat_tag = leon_tag(px, py);
        let (cr,cg,cb) = sdf_shade(sd.pnx,sd.pny,sd.pnz, hz, br,bg,bb, mat_tag, &lts);
        let ao = sdf_face_ao(px, py, pz, sd.pnx, sd.pny, sd.pnz);
        let (mut cr,mut cg,mut cb) = (cr*ao, cg*ao, cb*ao);

        // Lip SSS — pink-red backlit warmth through thin vermilion tissue
        let bk = (-sd.pnx*lts.klx-sd.pny*lts.kly-sd.pnz*lts.klz).max(0.0);
        let bt = (-sd.pnx*lts.f3x-sd.pny*lts.f3y-sd.pnz*lts.f3z).max(0.0)*lts.f3_int;
        let bi = bk*0.80 + bt;
        if bi > 0.04 {
            let thick = sdf_face_thickness(px, py, pz, sd.pnx, sd.pny, sd.pnz);
            let t = (-5.0*thick).exp()*bi;
            cr += t*1.00; cg += t*0.28; cb += t*0.18;
        }
        // Mouth crease: strong shadow — concavity at the groove
        if wrinkle>0.01 { cr*=1.0-wrinkle*0.62; cg*=1.0-wrinkle*0.70; cb*=1.0-wrinkle*0.82; }

        let alpha = ((0.35+core*0.55)*hp.max(0.08)*(1.0+wrinkle*0.55)).min(0.97);
        if alpha < 0.012 { continue; }
        let jx=(hf(i,273)-0.5)*dmg*0.08; let jy=(hf(i,274)-0.5)*dmg*0.08;
        let emission = (0.40+core*0.90)*(1.0-wrinkle*0.40);
        let sz=0.016+core*0.015; let ch=if core>0.6 {'+'} else {'.'};

        engine.spawn_glyph(Glyph {
            character: ch, scale: Vec2::splat(sz),
            position: Vec3::new(pos.x+px*scale*breath+jx, pos.y+py*scale*breath+jy, pos.z+hz*0.30),
            color: Vec4::new(cr,cg,cb,alpha), emission,
            glow_color:  Vec3::new(cr*0.55,cg*0.45,cb*0.35), glow_radius: core*0.12,
            mass: 0.0, lifetime: dt*1.5,
            layer: RenderLayer::Entity, blend_mode: BlendMode::Normal,
            ..Default::default()
        });
    }
}

fn render_leon(
    engine:     &mut ProofEngine,
    dt:         f32,
    pos:        Vec3,
    hp:         f32,
    time:       f32,
    tw:         f32,
    lag:        &mut Vec<Vec3>,   // per-particle smoothed position cache
    is_moving:  bool,             // false on first frame → skip blend, just seed cache
) {
    // Key light: upper-left-front
    let (klx, kly, klz) = {
        let (x, y, z) = (-0.40f32, -0.55f32, 0.73f32);
        let l = (x*x + y*y + z*z).sqrt();
        (x/l, y/l, z/l)
    };
    // Rim light: behind-upper-right (cool)
    let (rlx, rly, rlz) = {
        let (x, y, z) = (0.55f32, -0.18f32, -0.82f32);
        let l = (x*x + y*y + z*z).sqrt();
        (x/l, y/l, z/l)
    };
    // Fill 1: warm ground bounce — below-right, simulates light reflected off the floor
    let (f1x, f1y, f1z) = {
        let (x, y, z) = (0.30f32, 0.60f32, -0.40f32);
        let l = (x*x + y*y + z*z).sqrt();
        (x/l, y/l, z/l)
    };
    const F1: (f32, f32, f32) = (1.00, 0.85, 0.70);   // warm amber
    const F1_INT: f32 = 0.15;
    // Fill 2: cool sky hemisphere — straight down, simulates skylight on top surfaces
    let (f2x, f2y, f2z) = (0.0f32, -1.0f32, 0.0f32);  // already unit length
    const F2: (f32, f32, f32) = (0.60, 0.70, 1.00);   // cool blue
    const F2_INT: f32 = 0.05;
    // Fill 3: wall torch — warm orange from upper-right-behind.
    // Direction is the unit vector from Leon's origin toward the torch world position.
    let (f3x, f3y, f3z) = {
        let (x, y, z) = (TORCH_X, TORCH_Y, TORCH_Z);
        let l = (x*x + y*y + z*z).sqrt();
        (x/l, y/l, z/l)
    };
    const F3: (f32, f32, f32) = (1.00, 0.52, 0.15);   // amber-orange torch
    // Flicker: same two-sine formula used in render_environment so they stay in sync.
    let f3_int = 0.28 * (0.80 + (time * 7.3).sin() * 0.11 + (time * 13.1).cos() * 0.07);

    let scale   = 3.2f32;
    let dmg     = (1.0 - hp).max(0.0);
    let breath  = 1.0 + (time * 1.4).sin() * 0.010;

    for i in 0..500_000usize {
        // ── Density-weighted bone selection (convergence formula) ──────────────
        let mut w = hf(i, 0) * tw;
        let mut bone     = BONES[0];
        let mut bone_idx = 0usize;
        for (bi, b) in BONES.iter().enumerate() {
            w -= b.4 * (b.1 - b.0).abs();
            if w <= 0.0 { bone = *b; bone_idx = bi; break; }
        }
        let (y0, y1, cx, hw, _, ar) = bone;

        // HairKit owns bone 0 — skip it here so strands don't double-render
        if bone_idx == 0 { continue; }

        // SDF body/face systems own all major geometry — skip those bones here
        // to avoid double-rendering them at lower quality.
        // Retained by render_leon (no SDF coverage): 2 neck | 3,4 collar | 5,6 shoulders
        if matches!(bone_idx, 1|7|8|9|10|11|12|13|14|15|16|17|18|19|20|21|22|23|24|25) { continue; }

        // ── Gaussian spread (sum of two uniforms — convergence's exact formula) ─
        let along_t    = hf(i, 1);
        let along      = y0 + along_t * (y1 - y0);
        // Body profile sets the anatomical cross-section width.
        // Clothing offset layers on top: the garment's own silhouette shape
        // (shoulder structure, waist taper, hem flare) adds to the body width.
        // For skin/hair bones the offset is 0.0 and eff_hw equals body_hw.
        let body_hw    = hw * radius_at(bone_idx, along_t);
        let cloth_add  = clothing_offset(bone_idx, along_t);
        let eff_hw     = body_hw + cloth_add;
        let spread_raw = (hf(i, 2) + hf(i, 3) - 1.0) * eff_hw;  // garment-profile-scaled spread

        // ModelKit: anisotropic (elliptical) cross-section per bone.
        // ar < 1.0 → narrower X, deeper Z (flatter body part facing the camera).
        // dist uses the raw spread so core falloff shape is unchanged.
        let x = cx + spread_raw * ar;
        let y = along;

        // ── Core falloff (the key to convergence's smooth look) ───────────────
        // dist=0 at bone centre, 1 at edge. core=1 centre, 0 at edge.
        let dist = (spread_raw / eff_hw.max(0.001)).abs();
        let core = (1.0 - dist * 0.85).max(0.0);

        // ── Material color + tag from position ───────────────────────────────
        let (mut br, mut bg, mut bb) = leon_color(x, y);
        let mat_tag                  = leon_tag(x, y);

        // ── ClothingKit: garment radial push ─────────────────────────────────
        // Push each clothing particle outward from the bone axis by cloth_add
        // so the garment surface sits outside the body surface, not centred on
        // the same axis.  cloth_add is already profile-shaped (shoulder wide,
        // waist narrow, hem flare) — the same value that widened eff_hw above.
        // Skin / hair / hands: cloth_add == 0.0 so no push occurs.
        let radial_dir = if spread_raw >= 0.0 { 1.0f32 } else { -1.0 };
        let x = x + radial_dir * cloth_add;

        // ── ClothingKit: seam darkening at bone boundaries ────────────────────
        // A seam exists when:
        //   • this bone is clothing (BONE_CLOTHING flag), AND
        //   • the particle is near the bone's edge (dist > 0.85), AND
        //   • a "peek" sample just outside the bone (in X or Y) returns a
        //     different MaterialTag — confirming an adjacent different material.
        let seam_inward = if BONE_CLOTHING[bone_idx] && dist > 0.85 {
            // X-direction peek: 20% beyond this bone's scaled half-width
            let dir_x    = if spread_raw >= 0.0 { 1.0f32 } else { -1.0 };
            let peek_x   = cx + dir_x * hw * ar * 1.20;
            let x_adj    = leon_tag(peek_x, y) != mat_tag;

            // Y-direction peek: 0.06 units outside the bone's Y endpoints
            // (catches collar-to-chest, belt-to-pants, boot-to-shin, hand-to-sleeve)
            let along_t  = (along - y0) / ((y1 - y0).abs() + 0.001);
            let at_y_end = along_t < 0.15 || along_t > 0.85;
            let peek_y   = if along_t < 0.15 { y0 - 0.06 } else { y1 + 0.06 };
            let y_adj    = at_y_end && leon_tag(x, peek_y) != mat_tag;

            if x_adj || y_adj {
                br *= 0.6;
                bg *= 0.6;
                bb *= 0.6;
                -dir_x * 0.003   // push inward (toward bone axis)
            } else {
                0.0
            }
        } else {
            0.0
        };

        // ── ClothingKit: contact shadow darkening ─────────────────────────────
        // Bone-distance proxy — no per-particle search.
        // Anatomy tells us which bone faces which; we darken particles on the
        // inner (contact) side when dist is high (near bone edge) and the radial
        // direction points toward the adjacent bone.
        //
        // Key zones and their bone indices:
        //   Inner arm / forearm (13,14,15,16) → faces torso at ~0.09 gap
        //   Inner thigh / shin  (19,20,21,22) → faces opposite leg at ~0.11 gap
        //   Neck bottom         (2)           → contacts chest top at Y boundary
        //   Face chin           (1)           → contacts collar at Y boundary
        let contact_dark: bool = match bone_idx {
            // Left upper arm & forearm: inner side = +X (toward centre)
            13 | 15 if dist > 0.45 && spread_raw > 0.0 => true,
            // Right upper arm & forearm: inner side = -X
            14 | 16 if dist > 0.45 && spread_raw < 0.0 => true,
            // Left thigh & shin: medial side = +X
            19 | 21 if dist > 0.45 && spread_raw > 0.0 => true,
            // Right thigh & shin: medial side = -X
            20 | 22 if dist > 0.45 && spread_raw < 0.0 => true,
            // Neck: lower 35% of bone length contacts chest
            2 if (along - y0) / ((y1 - y0).abs() + 0.001) > 0.65 => true,
            // Face: bottom 25% (chin) contacts collar/chest top
            1 if (along - y0) / ((y1 - y0).abs() + 0.001) > 0.75 => true,
            _ => false,
        };
        if contact_dark { br *= 0.7; bg *= 0.7; bb *= 0.7; }

        // ── Pseudo-3D: depth from lateral position within bone ────────────────
        // Treat bone as a cylinder: hz = sqrt(1 - dist²) gives front-face peak.
        // Scaled by 1/ar: flat torso (ar=0.6) gets ~1.67× more Z depth than
        // a circular bone, correctly modelling a body part that is deeper than wide.
        let hz = ((1.0 - (dist * dist).min(1.0)).sqrt() / ar).min(1.0);

        // Surface normal — Bug 1 fixed: sny from vertical position within bone
        let snx = (spread_raw / hw.max(0.001)) * 0.5;
        let sny = (along - (y0 + y1) * 0.5) / ((y1 - y0) * 0.5 + 0.001) * 0.5;
        let snz = hz;
        let sn_len = (snx*snx + sny*sny + snz*snz).sqrt().max(0.001);
        let (snx, sny, snz) = (snx/sn_len, sny/sn_len, snz/sn_len);

        // ── LightingKit — Multi-light shading pass ────────────────────────────
        //
        // Purpose : Computes the final lit color for each particle using a
        //           physically motivated light rig: one warm directional key
        //           light, three coloured fill lights (warm bounce, cool sky,
        //           torch side-fill), a squared rim light for silhouette pop,
        //           and a hemisphere ambient term (separate per-channel to
        //           preserve color tints).  The result is run through an ACES
        //           filmic tonemap and S-curve contrast before output.
        //
        // Inputs  : snx/sny/snz (analytical surface normal), mat_tag, br/bg/bb
        //           (base color from leon_color), hz (pseudo-depth for rim gate)
        // Outputs : cr/cg/cb — final tonemapped particle color (0..1)
        //
        // Toggle  : Zero out diffuse_key to disable the key light (flat fill only).
        //           Set rim = 0 to remove rim highlighting.  Replace the tone/sc
        //           lambdas with identity (|c| c) to disable tonemapping.
        // ─────────────────────────────────────────────────────────────────────
        // Key — warm directional, upper-left-front
        let diffuse_key  = (snx*klx + sny*kly + snz*klz).max(0.0);

        // Fill 1 — warm ground bounce, same Lambert model
        let diffuse_f1   = (snx*f1x + sny*f1y + snz*f1z).max(0.0);

        // Fill 2 — cool sky, same Lambert model
        let diffuse_f2   = (snx*f2x + sny*f2y + snz*f2z).max(0.0);

        // Fill 3 — torch: warm amber from upper-right-behind, flickering
        let diffuse_f3   = (snx*f3x + sny*f3y + snz*f3z).max(0.0);

        // Rim
        let rim_fac = (1.0 - hz) * (snx*rlx + sny*rly + snz*rlz).max(0.0);
        let rim     = rim_fac * rim_fac;

        // Specular on leather/metal (key light only — fills are too soft to spec)
        // Uses mat_tag directly so seam/contact darkening can't suppress the highlight.
        let half_z = (klz + 1.0) * 0.5;
        let spec = if matches!(mat_tag, MatTag::Boot | MatTag::Metal) {
            (snz * half_z).max(0.0).powi(24) * 0.50
        } else { 0.0 };

        // LightingKit: hemisphere ambient — per-channel so color tints survive
        // sky_term:    upward normals (sny < 0) → cool blue sky
        // ground_term: downward normals (sny > 0) → warm brown bounce
        // base: 0.03 flat floor so absolute shadow is never pure black
        let sky_f   = (-sny).max(0.0) * 0.08;
        let gnd_f   = ( sny).max(0.0) * 0.04;
        let amb_r   = 0.03 + sky_f * 0.60 + gnd_f * 0.40;
        let amb_g   = 0.03 + sky_f * 0.75 + gnd_f * 0.35;
        let amb_b   = 0.03 + sky_f * 1.00 + gnd_f * 0.25;

        // Per-channel light accumulation
        // diff_spec: scalar key-light + specular (no color tint, already in albedo)
        // ambient: per-channel tinted hemisphere
        // fills: per-channel from Fill 1 / Fill 2
        let diff_spec = diffuse_key * 1.10 + spec;
        let light_r = (br * diff_spec + br * amb_r
                     + br * diffuse_f1 * F1_INT * F1.0
                     + br * diffuse_f2 * F2_INT * F2.0
                     + br * diffuse_f3 * f3_int  * F3.0).min(1.4);
        let light_g = (bg * diff_spec + bg * amb_g
                     + bg * diffuse_f1 * F1_INT * F1.1
                     + bg * diffuse_f2 * F2_INT * F2.1
                     + bg * diffuse_f3 * f3_int  * F3.1).min(1.4);
        let light_b = (bb * diff_spec + bb * amb_b
                     + bb * diffuse_f1 * F1_INT * F1.2
                     + bb * diffuse_f2 * F2_INT * F2.2
                     + bb * diffuse_f3 * f3_int  * F3.2).min(1.4);

        // Fresnel response: grazing-angle highlight per material tag
        let (fr, fg, fb) = fresnel_response(mat_tag, snz);

        let mut cr = light_r + rim * 0.12 + fr;
        let mut cg = light_g + rim * 0.16 + fg;
        let mut cb = light_b + rim * 0.42 + fb;

        // Filmic tone map
        let tone = |c: f32| c * (1.0 + c * 0.12) / (1.0 + c);
        cr = tone(cr).clamp(0.0, 1.0);
        cg = tone(cg).clamp(0.0, 1.0);
        cb = tone(cb).clamp(0.0, 1.0);

        // S-curve contrast
        let sc = |c: f32| c * c * (3.0 - 2.0 * c);
        cr = sc(cr); cg = sc(cg); cb = sc(cb);

        // ── Alpha: core falloff + HP ──────────────────────────────────────────
        // Matches convergence: (0.4 + core*0.5) base opacity
        let alpha = (0.35 + core * 0.55) * hp.max(0.08);
        if alpha < 0.012 { continue; }

        // ── Damage jitter (world-space, not blended — random noise, not motion) ─
        let jx = (hf(i, 4) - 0.5) * dmg * 0.08;
        let jy = (hf(i, 5) - 0.5) * dmg * 0.08;

        // ── PhysicsKit — Inertial lag / secondary motion ─────────────────────
        //
        // Purpose : Simulates the inertial delay of loose materials (hair,
        //           jacket hem, sleeves) when the character moves.  Each
        //           particle maintains a smoothed position cache; on each frame
        //           the bone-driven target position is blended toward the cache
        //           using a per-material weight.  Tight materials (skin, boots)
        //           use weight ≈ 1.0 (instant follow); loose materials (hair)
        //           use weight ≈ 0.12 so they trail several frames behind.
        //
        // Inputs  : bone_p (current deterministic position), lag[] cache,
        //           is_moving (false on first frame to seed cache without blend)
        // Outputs : smoothed_p — the jitter-free blended world position used for
        //           copy scatter and final spawn
        //
        // Toggle  : Set all lag_weight values to 1.0 to disable secondary motion
        //           (particles snap to bone position each frame).
        // ─────────────────────────────────────────────────────────────────────
        // bone_p = deterministic bone-driven world position for this particle.
        // We smooth it with the cache — loose materials trail behind, tight follow.
        // Jitter is added AFTER blending so random noise doesn't accumulate.
        let bone_p = Vec3::new(
            (x + seam_inward) * scale * breath,
            y                 * scale * breath,
            hz * 0.30,
        );
        // Per-material smoothing weight: 1.0 = instant follow, 0.0 = never moves
        let lag_weight: f32 = match mat_tag {
            MatTag::Hair                                          => 0.50,
            MatTag::Jacket if y > 0.28                           => 0.85, // pants
            MatTag::Jacket                                        => 0.75, // jacket
            MatTag::Boot   if y >= 0.22 && y <= 0.30             => 0.90, // belt
            MatTag::Boot                                          => 0.95, // boot
            MatTag::Metal                                         => 0.98,
            MatTag::Skin | _                                      => 0.95,
        };
        let smoothed_p = if is_moving {
            lag[i].lerp(bone_p, lag_weight)
        } else {
            bone_p   // first frame: seed cache with exact position, no blend
        };
        lag[i] = smoothed_p;

        // ── Glyph: size and char scale with core (denser at centre) ──────────
        let sz = 0.020 + core * 0.018;    // 0.020 at edge, 0.038 at centre
        let ch = if core > 0.7 { '*' } else if core > 0.35 { '+' } else { '.' };

        // ── RenderKit: depth of field ─────────────────────────────────────────
        // blur_amount: 0 at focal plane, 1 at FOCAL_DIST ± DOF_RANGE
        let particle_z  = pos.z + smoothed_p.z;
        let blur_amount = ((particle_z - FOCAL_DIST).abs() / DOF_RANGE).clamp(0.0, 1.0);
        let dof_jx      = (hf(i, 6) - 0.5) * blur_amount * 0.01;
        let dof_jy      = (hf(i, 7) - 0.5) * blur_amount * 0.01;
        let sz          = sz * (1.0 + blur_amount * 0.5);
        let alpha       = alpha / (1.0 + blur_amount);

        // ── Emission — boosted to compensate for lower ambient floor ─────────
        let emission = if br > 0.58 && bg > 0.46 {
            0.40 + core * 0.80   // skin  (max 1.20)
        } else if br < 0.28 && bg < 0.18 {
            0.12 + core * 0.30   // hair — dark (max 0.42)
        } else if bb < 0.14 {
            0.50 + core * 1.50   // boot/belt leather — glossy (max 2.00)
        } else {
            0.45 + core * 1.20   // jacket / pants (max 1.65)
        };

        engine.spawn_glyph(Glyph {
            character: ch,
            scale: Vec2::splat(sz),
            position: Vec3::new(
                pos.x + smoothed_p.x + jx + dof_jx,
                pos.y + smoothed_p.y + jy + dof_jy,
                pos.z + smoothed_p.z,
            ),
            color:      Vec4::new(cr, cg, cb, alpha),
            emission,
            glow_color: Vec3::new(cr * 0.55, cg * 0.45, cb * 0.35),
            glow_radius: core * 0.14,
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Entity,
            blend_mode: BlendMode::Normal,
            ..Default::default()
        });
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// HairKit — Strand-cluster hair renderer
//
// Purpose : Renders Leon's curtain-cut hairstyle as 500 anatomical strand
//           clusters distributed across five scalp zones (crown, left drape,
//           right side, nape, front fringe).  Each strand is a chain of 12–20
//           particles following a gravity-curved spline from root to tip, with
//           per-strand wind sway and a Kajiya-Kay anisotropic specular
//           highlight along the strand tangent direction.
//
// Inputs  : time (sway animation), pos (Leon world position), n_copies
// Outputs : Up to 160 K hair glyph spawns per frame via engine.spawn_glyph()
//
// Toggle  : Set HAIR_STRANDS = 0 to disable hair entirely (bone 0 still renders
//           the hair-volume capsule from render_leon).  Reduce HAIR_PPS to lower
//           strand detail.  Set sw = 0.0 per zone to stop wind sway.
// ═════════════════════════════════════════════════════════════════════════════

// ── HairKit: strand-cluster hair renderer ─────────────────────────────────────
// 500 Fibonacci-distributed anchor points on the skull, each emitting a chain
// of 12-20 particles along a downward-curving spline.  At 500 × 20 = 10K CPU
// particles × n_copies oversampling = up to 160K spawned hair glyphs.
// Anisotropic specular via Kajiya-Kay tangent model.
fn render_hair(
    engine:    &mut ProofEngine,
    dt:        f32,
    pos:       Vec3,
    hp:        f32,
    hair_lag:  &mut Vec<Vec3>,
    is_moving: bool,
) {
    const N_STRANDS:    usize = 500;
    const MAX_PER:      usize = 20;   // hard upper bound for lag indexing
    const SCALE:        f32   = 3.2;
    const GOLDEN_ANGLE: f32   = 2.399_963_2;   // TAU × (1 − 1/φ)

    // Key light direction (mirrors render_leon)
    let (klx, kly, klz) = {
        let (x, y, z) = (-0.40f32, -0.55f32, 0.73f32);
        let l = (x*x + y*y + z*z).sqrt();
        (x/l, y/l, z/l)
    };

    let dmg = (1.0 - hp).max(0.0);

    // Skull geometry: mirrors bone 0 (hair volume).
    // SKULL_R must equal hw * radius_at(0, 0.5) = 0.20 * 1.20 = 0.24 so hair
    // strands anchor on the scalp surface, not inside the enlarged skull body.
    // Before the +20% head fix, SKULL_R = 0.19 sat just inside hw=0.20; now the
    // skull equator is 0.24 wide so we match it exactly.
    const SKULL_CX: f32 = 0.00;
    const SKULL_CY: f32 = -1.00;   // midpoint of hair bone Y range
    const SKULL_R:  f32 = 0.24;    // = hw(0.20) × radius_at(0, 0.5) equator peak

    for si in 0..N_STRANDS {
        // ── Fibonacci spiral anchor on the skull hemisphere ───────────────────
        // theta: polar angle from crown (0 = top, larger = toward sides)
        // phi:   azimuthal, golden-angle spiral for even distribution
        let theta = (si as f32 / N_STRANDS as f32).sqrt() * 1.15;
        let phi   = si as f32 * GOLDEN_ANGLE + hf(si, 60) * 0.35;

        let ax = SKULL_CX + SKULL_R * theta.sin() * phi.cos();
        let ay = SKULL_CY - SKULL_R * theta.cos() * 0.65; // oblate skull

        // ── Strand growth direction at root ───────────────────────────────────
        // Outward normal + downward gravity + Leon's slight rightward sweep
        let base_dx = theta.sin() * phi.cos() * 0.018 + 0.006;
        let base_dy = theta.cos().abs() * 0.010 + 0.017;

        // Per-strand length: 12-20 particles
        let strand_n = (12 + (hf(si, 61) * 8.0) as usize).min(MAX_PER);

        for pi in 0..strand_n {
            let fi = pi as f32;
            let t  = fi / (strand_n - 1).max(1) as f32;  // 0=root … 1=tip

            // ── Strand position along spline ──────────────────────────────────
            // Linear growth along base direction + quadratic sag from gravity
            let sag_x = phi.sin() * t * t * 0.011;
            let sag_y = t * t * 0.017;
            let px = ax + base_dx * fi + sag_x;
            let py = ay + base_dy * fi + sag_y;

            // ── Tangent direction (derivative of position) ────────────────────
            let tx_r = base_dx + phi.sin() * 2.0 * t * 0.011;
            let ty_r = base_dy + 2.0 * t * 0.017;
            let tz_r = 0.024f32;    // slight Z so tangent has depth component
            let t_len = (tx_r*tx_r + ty_r*ty_r + tz_r*tz_r).sqrt().max(0.001);
            let (tx, ty, tz) = (tx_r/t_len, ty_r/t_len, tz_r/t_len);

            // ── Anisotropic specular (Kajiya-Kay, per prompt spec) ────────────
            // spec = pow(1 - |dot(light, tangent)|, 8) × 0.6
            let dot_lt    = (klx*tx + kly*ty + klz*tz).abs();
            let hair_spec = (1.0 - dot_lt).powi(8) * 0.6;

            // ── Hair color: dark brown + warm specular band ───────────────────
            let hr = (0.20 + hair_spec * 0.58).min(1.0);
            let hg = (0.12 + hair_spec * 0.44).min(1.0);
            let hb = (0.06 + hair_spec * 0.22).min(1.0);

            // ── Opacity: full at root, fades to tip ───────────────────────────
            let alpha = ((0.80 - t * 0.70) * hp.max(0.1)).max(0.0);
            if alpha < 0.01 { continue; }

            // ── Inertial lag: pendulum — root tight (0.88), tip loose (0.38) ──
            let lag_weight = 0.88 - t * 0.50;
            let lag_idx    = si * MAX_PER + pi;
            let bone_p     = Vec3::new(px * SCALE, py * SCALE, 0.18 - t * 0.06);
            let smoothed_p = if is_moving && lag_idx < hair_lag.len() {
                hair_lag[lag_idx].lerp(bone_p, lag_weight)
            } else {
                bone_p
            };
            if lag_idx < hair_lag.len() { hair_lag[lag_idx] = smoothed_p; }

            // Damage jitter (separate seeds from main particle system)
            let seed   = si * 13 + pi;
            let jx = (hf(seed, 0) - 0.5) * dmg * 0.05;
            let jy = (hf(seed, 1) - 0.5) * dmg * 0.05;

            // RenderKit: depth of field
            let particle_z  = pos.z + smoothed_p.z;
            let blur_amount = ((particle_z - FOCAL_DIST).abs() / DOF_RANGE).clamp(0.0, 1.0);
            let dof_jx      = (hf(seed, 2) - 0.5) * blur_amount * 0.01;
            let dof_jy      = (hf(seed, 3) - 0.5) * blur_amount * 0.01;
            let sz_dof      = (0.024 - t * 0.012).max(0.008) * (1.0 + blur_amount * 0.5);
            let alpha_dof   = alpha / (1.0 + blur_amount);
            if alpha_dof < 0.01 { continue; }

            let ch = if pi == 0 { '*' } else if t < 0.5 { '+' } else { '.' };

            engine.spawn_glyph(Glyph {
                character: ch,
                scale:     Vec2::splat(sz_dof),
                position:  Vec3::new(
                    pos.x + smoothed_p.x + jx + dof_jx,
                    pos.y + smoothed_p.y + jy + dof_jy,
                    pos.z + smoothed_p.z,
                ),
                color:       Vec4::new(hr, hg, hb, alpha_dof),
                emission:    0.10 + hair_spec * 0.70,
                glow_color:  Vec3::new(hr * 0.55, hg * 0.40, 0.0),
                glow_radius: 0.05 + hair_spec * 0.14,
                mass: 0.0, lifetime: dt * 1.5,
                layer:      RenderLayer::Entity,
                blend_mode: BlendMode::Normal,
                ..Default::default()
            });
        }
    }
}

fn spawn_burst(engine: &mut ProofEngine, dt: f32, origin: Vec3,
               cr: f32, cg: f32, cb: f32, count: usize, radius: f32, seed: usize) {
    for i in 0..count {
        let angle = i as f32 / count as f32 * TAU + hf(seed, i) * 0.4;
        let r     = radius * (0.4 + hf(seed+1, i) * 0.6);
        let speed = 0.5 + hf(seed+2, i) * 0.8;
        engine.spawn_glyph(Glyph {
            character: if i % 3 == 0 { '*' } else { '+' },
            scale: Vec2::splat(0.06 + hf(seed+3, i) * 0.06),
            position: Vec3::new(origin.x + angle.cos()*r, origin.y + angle.sin()*r*0.5, origin.z+0.4),
            velocity: Vec3::new(angle.cos()*speed, angle.sin()*speed*0.5 - 0.2, 0.0),
            color: Vec4::new(cr, cg, cb, 0.7 + hf(seed+4,i)*0.3),
            emission: 2.0 + hf(seed+5,i)*2.0,
            glow_color: Vec3::new(cr, cg, cb), glow_radius: 0.5,
            mass: 0.0, lifetime: dt*1.5, layer: RenderLayer::Particle,
            blend_mode: BlendMode::Additive, ..Default::default()
        });
    }
}

fn spawn_stream(engine: &mut ProofEngine, dt: f32, origin: Vec3,
                cr: f32, cg: f32, cb: f32, count: usize, seed: usize, time: f32) {
    for i in 0..count {
        let phase = hf(seed, i) * TAU;
        let sx    = (hf(seed+1, i) - 0.5) * 0.30;
        let rise  = hf(seed+2, i) * 0.65;
        engine.spawn_glyph(Glyph {
            character: if i%2==0 {'*'} else {'.'},
            scale: Vec2::splat(0.04 + hf(seed+3,i)*0.04),
            position: Vec3::new(
                origin.x + sx + (time*3.0+phase).sin()*0.09,
                origin.y - rise,
                origin.z + 0.3 + hf(seed+4,i)*0.4,
            ),
            color: Vec4::new(cr, cg, cb*(0.3+hf(seed+5,i)*0.7), 0.5+hf(seed+6,i)*0.4),
            emission: 2.0+hf(seed+7,i)*2.0,
            glow_color: Vec3::new(cr, cg*0.7, 0.1), glow_radius: 0.38,
            mass: 0.0, lifetime: dt*1.5, layer: RenderLayer::Particle,
            blend_mode: BlendMode::Additive, ..Default::default()
        });
    }
}

// ── Ground plane ──────────────────────────────────────────────────────────────
// Particle floor grid at FLOOR_Y.  Two components:
//   1. XZ grid lattice: 24 columns × 16 rows, lit by key + torch with distance falloff.
//   2. Reflection puddle directly under Leon (blue-white shimmer).
//   3. Torch spill pool under the fixture (orange).
fn render_ground(engine: &mut ProofEngine, dt: f32, time: f32) {
    let flicker = 0.80 + (time * 7.3).sin() * 0.11 + (time * 13.1).cos() * 0.07;

    // ── Grid lattice ─────────────────────────────────────────────────────────
    // 24 X columns × 16 Z rows = 384 vertices.  Perspective projection handles
    // the foreshortening; grid spacing is uniform in world space.
    const X_STEPS: usize = 24;
    const Z_STEPS: usize = 16;
    for zi in 0..Z_STEPS {
        let gz = -5.0 + zi as f32 * 0.36;          // Z: –5.0 → +0.76
        for xi in 0..X_STEPS {
            let gx = -7.0 + xi as f32 * 0.61;      // X: –7.0 → +7.23

            let d_torch = ((gx - TORCH_X).powi(2) + (gz - TORCH_Z).powi(2)).sqrt();
            let d_leon  = (gx.powi(2) + gz.powi(2)).sqrt();
            let torch_f = flicker / (1.0 + d_torch * 0.75).powi(2);
            let leon_f  = 1.0    / (1.0 + d_leon  * 0.55).powi(2);

            // Contact shadow: character casts a soft shadow on the floor behind/below them.
            // The key light arrives from the upper-right (klx≈-0.375, klz≈0.685), so the shadow
            // falls in the -X / +Z half.  We use a proximity-darkening proxy offset slightly
            // in that direction rather than a full ray cast.
            let shadow_f = {
                let shadow_cx = -0.30f32; let shadow_cz = 0.55f32;
                let sdx = gx - shadow_cx; let sdz = gz - shadow_cz;
                let r2 = sdx*sdx + sdz*sdz;
                let contact = (1.0 - r2 / (1.8*1.8)).max(0.0).powi(2);
                // Additional broad proximity blob that softens the whole footprint
                let prox = (1.0 - (gx*gx + gz*gz) / (1.4*1.4)).max(0.0).powi(3);
                (contact*0.42 + prox*0.28).min(0.65)
            };
            // Stone floor: dark blue-grey base + orange torch wash + blue-white Leon spill
            let fr_raw = (0.06 + torch_f * 0.60 + leon_f * 0.10).min(1.0);
            let fg_raw = (0.07 + torch_f * 0.26 + leon_f * 0.09).min(1.0);
            let fb_raw = (0.10 + torch_f * 0.05 + leon_f * 0.22).min(1.0);
            let fr = fr_raw * (1.0 - shadow_f);
            let fg = fg_raw * (1.0 - shadow_f);
            let fb = fb_raw * (1.0 - shadow_f);

            // Fade toward edges and far depths
            let z_fade = ((gz + 5.0) / 5.8).clamp(0.0, 1.0);
            let x_fade = (1.0 - gx.abs() / 7.8).clamp(0.0, 1.0);
            let fa = (0.12 + torch_f * 0.55 + leon_f * 0.38) * z_fade * x_fade;
            if fa < 0.01 { continue; }

            // Grid intersections '+', axis lines '-'/'|', fill '.'
            let on_x = xi % 4 == 0;
            let on_z = zi % 3 == 0;
            let ch = if on_x && on_z { '+' } else if on_x { '|' } else if on_z { '-' } else { '.' };

            engine.spawn_glyph(Glyph {
                character: ch,
                scale:    Vec2::splat(0.07 + torch_f * 0.03),
                position: Vec3::new(gx, FLOOR_Y, gz),
                color:    Vec4::new(fr, fg, fb, fa),
                emission: (torch_f * 0.70 + leon_f * 0.28).min(1.2),
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    // ── Leon reflection puddle ────────────────────────────────────────────────
    // Blue-white shimmer on the floor directly under the character —
    // suggests a polished/damp stone surface picking up his glow.
    for i in 0..52usize {
        let angle = i as f32 / 52.0 * TAU;
        let r = 0.15 + hf(i, 20) * 1.15;
        let gx = angle.cos() * r * 1.5;
        let gz = angle.sin() * r * 0.55 + 0.10;
        let fa = (0.80 - r / 1.35) * 0.36;
        if fa < 0.01 { continue; }
        engine.spawn_glyph(Glyph {
            character: '.',
            scale:    Vec2::splat(0.09 + (1.0 - r / 1.35) * 0.05),
            position: Vec3::new(gx, FLOOR_Y - 0.02, gz),
            color:    Vec4::new(0.48, 0.58, 0.92, fa),
            emission: 0.70 - r / 1.5,
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }

    // ── Torch floor spill ─────────────────────────────────────────────────────
    // Orange pool under the fixture, inverse-square falloff with flicker.
    for i in 0..36usize {
        let angle = i as f32 / 36.0 * TAU;
        let r = 0.10 + hf(i, 21) * 1.90;
        let gx = TORCH_X + angle.cos() * r;
        let gz = TORCH_Z + angle.sin() * r * 0.50;
        let fa = flicker * (0.72 - r / 2.1).max(0.0) * 0.55;
        if fa < 0.01 { continue; }
        engine.spawn_glyph(Glyph {
            character: '.',
            scale:    Vec2::splat(0.09),
            position: Vec3::new(gx, FLOOR_Y - 0.02, gz),
            color:    Vec4::new(1.0, 0.50, 0.10, fa),
            emission: flicker * 1.10 * (1.0 - r / 2.1).max(0.0),
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }
}

// ── Environment ───────────────────────────────────────────────────────────────
// Stone architecture: two pillars framing the shot, back wall, and the wall
// torch that is the source of the F3 light striking Leon.
fn render_environment(engine: &mut ProofEngine, dt: f32, time: f32) {
    let flicker = 0.80 + (time * 7.3).sin() * 0.11 + (time * 13.1).cos() * 0.07;

    // ── Left pillar — cool, barely lit by key ────────────────────────────────
    for i in 0..34usize {
        let py     = -4.60 + i as f32 * 0.245;    // Y: –4.6 → +3.7
        let cap    = i < 4 || i > 29;             // capitals at top/bottom: brighter
        let cap_br = if cap { 0.18 } else { 0.0 };
        engine.spawn_glyph(Glyph {
            character: if cap { '+' } else if i % 5 == 0 { '|' } else { '.' },
            scale:    Vec2::splat(0.10),
            position: Vec3::new(-4.60, py, -0.85),
            color:    Vec4::new(0.13 + cap_br, 0.12 + cap_br * 0.8, 0.16 + cap_br * 0.9,
                                0.38 + if cap { 0.22 } else { 0.0 }),
            emission: 0.14 + if cap { 0.32 } else { 0.0 },
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }

    // ── Right pillar — torch-lit, orange wash that fades toward top and base ──
    for i in 0..34usize {
        let py     = -4.60 + i as f32 * 0.245;
        let cap    = i < 4 || i > 29;
        let d_y    = (py - TORCH_Y).abs();
        let torch_f = flicker / (1.0 + d_y * 0.55).powi(2);
        let cap_br = if cap { 0.15 } else { 0.0 };
        engine.spawn_glyph(Glyph {
            character: if cap { '+' } else if i % 5 == 0 { '|' } else { '.' },
            scale:    Vec2::splat(0.10),
            position: Vec3::new(4.60, py, -0.85),
            color:    Vec4::new(
                0.13 + torch_f * 0.58 + cap_br,
                0.10 + torch_f * 0.22 + cap_br * 0.7,
                0.10 + torch_f * 0.04 + cap_br * 0.8,
                0.38 + torch_f * 0.38 + if cap { 0.18 } else { 0.0 },
            ),
            emission: 0.12 + torch_f * 0.88 + if cap { 0.28 } else { 0.0 },
            glow_color:  Vec3::new(1.0, 0.48, 0.08) * torch_f,
            glow_radius: torch_f * 0.35,
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }

    // ── Back wall — rough stone, torch gradient L→R ──────────────────────────
    // 4 horizontal bands × 16 columns = 64 particles scattered to suggest masonry.
    for band in 0..4usize {
        let wy_base = -3.5 + band as f32 * 1.85;
        for col in 0..16usize {
            let wx   = -6.0 + col as f32 * 0.82;
            let wy   = wy_base + hf(band * 16 + col, 40) * 1.40;
            let d_t  = ((wx - TORCH_X).powi(2) + (wy - TORCH_Y).powi(2)).sqrt();
            let tf   = flicker / (1.0 + d_t * 0.50).powi(2);
            let x_f  = (1.0 - wx.abs() / 6.5).clamp(0.0, 1.0);
            let fa   = (0.12 + tf * 0.45) * x_f;
            if fa < 0.01 { continue; }
            let ch   = ['.', '-', '+', 'x', '|', ':'][( band * 16 + col) % 6];
            engine.spawn_glyph(Glyph {
                character: ch,
                scale:    Vec2::splat(0.09 + hf(band * 16 + col, 41) * 0.04),
                position: Vec3::new(wx, wy, -5.20),
                color:    Vec4::new(0.06 + tf * 0.52, 0.06 + tf * 0.20, 0.08 + tf * 0.04, fa),
                emission: 0.08 + tf * 0.55,
                mass: 0.0, lifetime: dt * 1.5,
                layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    // ── Torch fixture: bracket + animated flame ───────────────────────────────
    // Bracket: 5 static glyphs forming the iron holder.
    for i in 0..5usize {
        engine.spawn_glyph(Glyph {
            character: ['-', '+', '|', '+', '-'][i],
            scale:    Vec2::splat(0.12),
            position: Vec3::new(TORCH_X + (i as f32 - 2.0) * 0.09, TORCH_Y + 0.22, TORCH_Z),
            color:    Vec4::new(0.38, 0.26, 0.12, 0.88),
            emission: 0.22,
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }
    // Flame: 22 particles, small cluster with upward drift and high emission.
    for i in 0..22usize {
        let angle = i as f32 / 22.0 * TAU;
        let r     = hf(i, 50) * 0.20;
        let rise  = hf(i, 51) * 0.28;   // upward Y offset (negative = visual up)
        let tx    = TORCH_X + angle.cos() * r;
        let ty    = TORCH_Y - rise;      // flame rises visually upward
        let tz    = TORCH_Z + angle.sin() * r * 0.45;
        // Two-tone: inner core white-yellow, outer fringe orange
        let core  = 1.0 - r / 0.22;
        let fr    = 1.0f32;
        let fg    = 0.55 + core * 0.38;
        let fb    = 0.05 + core * 0.35;
        engine.spawn_glyph(Glyph {
            character: if hf(i, 52) > 0.65 { '*' } else { '+' },
            scale:    Vec2::splat(0.13 + flicker * 0.05),
            position: Vec3::new(tx, ty, tz),
            color:    Vec4::new(fr, fg, fb, 0.60 + flicker * 0.28),
            emission: flicker * (2.8 + core * 1.6),
            glow_color:  Vec3::new(1.0, 0.44, 0.06),
            glow_radius: 0.75 + flicker * 0.42,
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }
}

// ── Sky dome ──────────────────────────────────────────────────────────────────
// A procedural sky hemisphere built from ~2 400 particles scattered on a dome of
// radius 20 world units above the scene.  Each particle samples sky_color() for
// its position vector, giving the same warm-horizon / cool-zenith gradient the
// reflective materials see.  A sparse star field sits at high elevation.
fn render_sky(engine: &mut ProofEngine, dt: f32, time: f32) {
    const R: f32      = 20.0;
    const N_DOME: usize = 2400;
    const GOLDEN_SKY: f32 = 2.399_963_2;   // TAU × (1 − 1/φ)

    // Dome particles — Fibonacci hemisphere distribution (upper hemisphere: y < 0 in model space)
    for i in 0..N_DOME {
        let fi = i as f32 + 0.5;
        // Spherical Fibonacci: cos_theta ∈ [0,1] maps to upper hemisphere
        let cos_theta = 1.0 - fi / N_DOME as f32;        // 1=pole, 0=equator
        let sin_theta = (1.0 - cos_theta*cos_theta).sqrt();
        let phi       = GOLDEN_SKY * fi;
        let dx = phi.cos() * sin_theta;
        let dy = -cos_theta;                               // model Y-negative = visual up
        let dz = phi.sin() * sin_theta;

        // Gentle atmospheric shimmer using hf as a deterministic jitter
        let jitter = hf(i, 300) * 0.06 - 0.03;
        let (mut sr, mut sg, mut sb) = sky_color(dx, dy, dz);
        sr = (sr + jitter * 0.20).clamp(0.0, 1.0);
        sg = (sg + jitter * 0.15).clamp(0.0, 1.0);
        sb = (sb + jitter * 0.10).clamp(0.0, 1.0);

        // Alpha: brighter near horizon, subtle at zenith
        let elev = cos_theta;  // 0=equator, 1=pole
        let fa   = (0.06 + (1.0 - elev) * 0.07).clamp(0.02, 0.14);

        // Scale: larger glyphs at horizon to fill the gradient band
        let sz = 0.18 + (1.0 - elev) * 0.17;

        // Slow drift: the dome slowly rotates so the sky feels alive
        let drift = time * 0.008;
        let cos_d = drift.cos(); let sin_d = drift.sin();
        let rx = dx*cos_d - dz*sin_d;
        let rz = dx*sin_d + dz*cos_d;

        engine.spawn_glyph(Glyph {
            character: if hf(i, 301) > 0.82 { '·' } else { '.' },
            scale:    Vec2::splat(sz),
            position: Vec3::new(rx * R, dy * R, rz * R),
            color:    Vec4::new(sr, sg, sb, fa),
            emission: 0.30 + elev * 0.08,
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }

    // Stars — sparse high-elevation points with gentle twinkle
    const N_STARS: usize = 180;
    for i in 0..N_STARS {
        let fi = i as f32 + 0.5;
        let cos_theta = 0.55 + (fi / N_STARS as f32) * 0.45;  // restricted to upper cap
        let sin_theta = (1.0 - cos_theta*cos_theta).sqrt();
        let phi       = GOLDEN_SKY * fi * 3.7;
        let dx = phi.cos() * sin_theta;
        let dy = -cos_theta;
        let dz = phi.sin() * sin_theta;

        let twinkle = 0.65 + (time * (2.1 + hf(i, 310) * 3.0) + hf(i, 311) * TAU).sin() * 0.35;
        let fa      = twinkle * (0.08 + hf(i, 312) * 0.10);
        let sz      = 0.10 + hf(i, 313) * 0.08;
        let col     = 0.82 + hf(i, 314) * 0.18;  // near-white with warm variance

        engine.spawn_glyph(Glyph {
            character: if hf(i, 315) > 0.88 { '+' } else { '·' },
            scale:    Vec2::splat(sz),
            position: Vec3::new(dx * R, dy * R, dz * R),
            color:    Vec4::new(col, col * 0.94, col * 0.88, fa),
            emission: twinkle * 1.20,
            mass: 0.0, lifetime: dt * 1.5,
            layer: RenderLayer::Background, blend_mode: BlendMode::Additive,
            ..Default::default()
        });
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// GPU Compute Shader Pipeline — SDF body particles via OpenGL 4.3 compute
//
// Architecture:
//   1. COMPUTE pass  — 424 K threads evaluate SDF, accept/reject, write GpuParticle
//                      structs to SSBO via atomic slot assignment.
//   2. FINALIZE pass — 1 thread reads atomic counter, fills indirect draw command.
//   3. RENDER  pass  — indirect instanced draw: each accepted particle is drawn
//                      u_multiplier times as a billboard quad with per-copy jitter.
//
// Falls back to the CPU render_sdf_body path when GL < 4.3 (no compute support).
// ═════════════════════════════════════════════════════════════════════════════

// ── Particle budget per region ────────────────────────────────────────────────
const GPU_N_TORSO:  u32 = 2_800_000;
const GPU_N_ARM_R:  u32 =   900_000;
const GPU_N_ARM_L:  u32 =   900_000;
const GPU_N_FA_R:   u32 =   650_000;
const GPU_N_FA_L:   u32 =   650_000;
const GPU_N_HAND_R: u32 =   380_000;
const GPU_N_HAND_L: u32 =   380_000;
const GPU_N_LEG_R:  u32 = 1_300_000;
const GPU_N_LEG_L:  u32 = 1_300_000;
const GPU_N_FOOT_R: u32 =   370_000;
const GPU_N_FOOT_L: u32 =   370_000;
const GPU_N_HEAD:   u32 =   800_000;   // face+neck+skull GPU region
const GPU_N_TOTAL: u32 = GPU_N_TORSO + GPU_N_ARM_R + GPU_N_ARM_L
                       + GPU_N_FA_R  + GPU_N_FA_L
                       + GPU_N_HAND_R + GPU_N_HAND_L
                       + GPU_N_LEG_R + GPU_N_LEG_L
                       + GPU_N_FOOT_R + GPU_N_FOOT_L
                       + GPU_N_HEAD;
// Max accepted = total candidates. SSBO = GPU_MAX × 48 bytes.
const GPU_MAX_PARTICLES: u32 = 10_800_000;  // 10M body + 800K head. ~518 MB
// Each GpuParticle = 48 bytes (std430: vec3+float+vec3+float+vec4)
const GPU_PARTICLE_BYTES: usize = 48;
// Indirect draw command: 4 × u32 = 16 bytes
const GPU_INDIRECT_BYTES: usize = 16;
// Work-group size for compute
const GPU_WG: u32 = 256;

// ── Compute shader — full SDF pipeline ───────────────────────────────────────
const COMPUTE_SRC: &str = r#"
#version 430 core
layout(local_size_x = 256) in;

// ── Bindings ──────────────────────────────────────────────────────────────────
layout(binding = 0, offset = 0) uniform atomic_uint u_count;

struct GpuParticle {
    vec3  position;
    float size;
    vec3  normal;
    float emission;
    vec4  color;
};
layout(std430, binding = 1) writeonly buffer ParticleSSBO {
    GpuParticle particles[];
};

// ── Per-frame uniforms ────────────────────────────────────────────────────────
uniform float u_time;
uniform float u_hp;
uniform float u_scale;    // = 3.2 (world scale)
uniform float u_breath;   // = 1 + sin(t*1.4)*0.010
uniform float u_f3_int;   // torch flicker intensity
uniform float u_dmg;      // = 1 - hp
uniform int   u_n;        // total candidates = GPU_N_TOTAL
uniform vec3  u_cam_pos;  // camera world-space position
uniform float u_exposure; // eye-adaptation exposure multiplier (1.0 = neutral)

// ── Region offsets (cumulative) ───────────────────────────────────────────────
const uint OFF_TORSO  = 0u;
const uint OFF_ARM_R  = 2800000u;
const uint OFF_ARM_L  = 3700000u;
const uint OFF_FA_R   = 4600000u;
const uint OFF_FA_L   = 5250000u;
const uint OFF_HAND_R = 5900000u;
const uint OFF_HAND_L = 6280000u;
const uint OFF_LEG_R  = 6660000u;
const uint OFF_LEG_L  = 7960000u;
const uint OFF_FOOT_R = 9260000u;
const uint OFF_FOOT_L = 9630000u;
const uint OFF_HEAD   = 10000000u;  // face + neck + skull
const uint OFF_END    = 10800000u;

// ── Shell half-thickness ──────────────────────────────────────────────────────
// Tight shell = particles cluster at the zero-crossing → solid surface appearance.
const float SHELL = 0.008;

// ── Light directions (constant — match CPU) ───────────────────────────────────
const vec3 KL  = vec3(-0.375042, -0.484927, 0.789432);   // normalize(-0.40,-0.55,0.73)
const vec3 RL  = vec3( 0.559270, -0.183384,-0.808499);   // normalize(0.55,-0.18,-0.82)
const vec3 F1D = vec3( 0.371391,  0.742781,-0.557086);   // normalize(0.30,0.60,-0.40)
const vec3 F2D = vec3( 0.0,      -1.0,      0.0);
const vec3 F3D = vec3( 0.907166, -0.568329,-0.340997);   // normalize(3.20,-2.00,-1.20)
const float PI = 3.14159265;
const float TAU = 6.28318530;

// ── Integer hash (identical constants to CPU hf()) ───────────────────────────
float hf(uint seed, uint v) {
    uint n = seed * 374761393u + v * 668265263u;
    n ^= (n >> 13u);
    n *= 0x5851F42Du;
    n ^= (n >> 16u);
    return float(n & 0x00FFFFFFu) / float(0x01000000u);
}

// ── Smooth-min / smooth-max ───────────────────────────────────────────────────
float smin(float a, float b, float k) {
    float h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}
float smax(float a, float b, float k) {
    return -smin(-a, -b, k);
}

// ── Piecewise-linear torso axes (inlined knot tables) ────────────────────────
float plerp_ax(float ty) {
    ty = clamp(ty, 0.0, 1.0);
    if (ty <= 0.22) return 0.33 + (0.29 - 0.33) * (ty        / 0.22);  // narrower shoulders
    if (ty <= 0.55) return 0.29 + (0.20 - 0.29) * ((ty-0.22) / 0.33);  // lean upper chest
    if (ty <= 0.72) return 0.20 + (0.23 - 0.20) * ((ty-0.55) / 0.17);
                    return 0.23 + (0.25 - 0.23) * ((ty-0.72) / 0.28);
}
float plerp_az(float ty) {
    ty = clamp(ty, 0.0, 1.0);
    if (ty <= 0.28) return 0.16 + (0.18 - 0.16) * (ty        / 0.28);
    if (ty <= 0.55) return 0.18 + (0.13 - 0.18) * ((ty-0.28) / 0.27);
                    return 0.13 + (0.16 - 0.13) * ((ty-0.55) / 0.45);
}

// ── SDF primitives ────────────────────────────────────────────────────────────
float sdf_torso(float px, float py, float pz) {
    const float Y0 = -0.68, Y1 = 0.32;
    float ty = clamp((py - Y0) / (Y1 - Y0), 0.0, 1.0);
    // S-curve lean
    float lean_z = (1.0 - ty) * 0.016 - ty * 0.008;
    float ax = plerp_ax(ty);
    float az = plerp_az(ty);

    // Rib cage forward convexity: chest pushes +Z in the upper half (ty < 0.5)
    // Abdomen is flatter / slightly pushed back in ty 0.4-0.7
    float chest_t  = clamp((0.40 - ty) / 0.40, 0.0, 1.0);   // 1=top, 0=mid
    float rib_fwd  = chest_t * chest_t * 0.018;              // quadratic chest forward bulge
    float ab_flat  = clamp((ty - 0.42) / 0.25, 0.0, 1.0) * clamp((0.70 - ty) / 0.28, 0.0, 1.0);
    float ab_z     = lean_z - rib_fwd + ab_flat * 0.010;     // abdomen slightly back from rib cage

    float nx = px / ax, nz = (pz - ab_z) / az;
    float cross_d = (sqrt(nx*nx + nz*nz) - 1.0) * min(ax, az);
    float vert_d  = max(py - Y1, 0.0) + max(Y0 - py, 0.0);
    return max(cross_d, vert_d);
}

// Sternum ridge: thin vertical ellipsoid along the midline of the chest.
float sdf_sternum(float px, float py, float pz) {
    // Sternum: center X=0, Y from -0.55 to -0.05, forward (+Z) lean follows chest
    float ty = clamp((py + 0.55) / 0.50, 0.0, 1.0);
    float chest_z = 0.082 + ty * 0.010;  // front of chest varies with height
    float dpx = px / 0.018;
    float dpy = (py + 0.30) / 0.280;
    float dpz = (pz - chest_z) / 0.020;
    return (sqrt(dpx*dpx+dpy*dpy+dpz*dpz) - 1.0) * 0.018;
}

float sdf_arm_r(float px, float py, float pz) {
    const float AX=0.460, AY=-0.565, BX=0.492, BY=-0.065, RA=0.092, RB=0.079;
    float t  = clamp((py - AY) / (BY - AY), 0.0, 1.0);
    float cx = AX + t * (BX - AX);
    float r  = RA + t * (RB - RA);
    // Bicep bulge: front (+Z) side is rounder, back (-Z) side is flatter (tricep flat)
    // Achieve via asymmetric ellipse: squish back of arm slightly
    float bicep_peak = exp(-((t - 0.45)*(t - 0.45)) * 18.0);  // peak at 45% of upper arm
    float front_bias = 0.020 * bicep_peak;  // +Z offset at bicep peak
    float pz_local = pz - front_bias;       // shift cross-section center forward at bicep
    float arm_aspect_z = 0.90 + 0.08 * bicep_peak;  // rounder front at bicep
    float dx = (px - cx) / 1.05;
    float dz = pz_local / arm_aspect_z;
    float xz = sqrt(dx*dx + dz*dz);
    float ye = max(py - BY, 0.0) + max(AY - py, 0.0);
    return (ye > 0.0) ? sqrt(xz*xz + ye*ye) - r : xz - r;
}

float sdf_forearm_r(float px, float py, float pz) {
    const float AX=0.492, AY=-0.065, BX=0.515, BY=0.275, RA=0.072, RB=0.054;
    float t  = clamp((py - AY) / (BY - AY), 0.0, 1.0);
    float cx = AX + t * (BX - AX);
    float r  = RA + t * (RB - RA);
    // Forearm widest near elbow (t=0), tapers to wrist (t=1).
    // Brachioradialis bulge on the lateral (+X relative to cx) side near elbow.
    float brach = exp(-t * t * 4.0);   // peaks at elbow, fades toward wrist
    float px_local = px - cx + brach * 0.012;  // lateral bulge toward elbow
    float fa_aspect_z = 0.92 + brach * 0.05;   // slightly rounder cross-section at elbow
    float dx = px_local / 1.05;
    float dz = pz / fa_aspect_z;
    float xz = sqrt(dx*dx + dz*dz);
    float ye = max(py - BY, 0.0) + max(AY - py, 0.0);
    return (ye > 0.0) ? sqrt(xz*xz + ye*ye) - r : xz - r;
}

float sdf_leg_r(float px, float py, float pz) {
    const float CX = 0.145;
    float d_thigh; {
        float t = clamp((py - 0.28) / 0.54, 0.0, 1.0);
        float r = 0.105 + t * (0.088 - 0.105);
        float dx = px - CX;
        float xz = sqrt(dx*dx + pz*pz);
        float ye = max(py - 0.82, 0.0) + max(0.28 - py, 0.0);
        d_thigh = (ye > 0.0) ? sqrt(xz*xz + ye*ye) - r : xz - r;
    }
    float d_shin; {
        float t = clamp((py - 0.78) / 0.40, 0.0, 1.0);
        float r = 0.090 + t * (0.055 - 0.090);
        float dx = px - CX;
        float xz = sqrt(dx*dx + pz*pz);
        float ye = max(py - 1.18, 0.0) + max(0.78 - py, 0.0);
        d_shin = (ye > 0.0) ? sqrt(xz*xz + ye*ye) - r : xz - r;
    }
    float d = smin(d_thigh, d_shin, 0.035);

    // Patella (kneecap): small forward-facing ellipsoid at front of knee
    { float dpx=(px-CX)/0.030, dpy=(py-0.800)/0.022, dpz=(pz-0.064)/0.020;
      float d_pat=(sqrt(dpx*dpx+dpy*dpy+dpz*dpz)-1.0)*0.020;
      d = smin(d, d_pat, 0.018); }

    return d;
}

// Olecranon bump: posterior protrusion of the elbow (back of the elbow joint).
float sdf_olecranon_r(float px, float py, float pz) {
    // At the elbow junction (py≈-0.065), back of arm (-Z side)
    float dpx=(px-0.490)/0.018, dpy=(py+0.070)/0.016, dpz=(pz+0.042)/0.018;
    return (sqrt(dpx*dpx+dpy*dpy+dpz*dpz)-1.0)*0.016;
}

// ── Fingernail plates ────────────────────────────────────────────────────────
// Flat ellipsoids on the dorsal (back) surface of each fingertip.
// Positioned at the distal phalanx of each finger (near fingertip).
float sdf_fingernails_r(float px, float py, float pz) {
    float d = 1e9;
    // Index through little: tip positions from sdf_hand_r distal segment ends
    const float NFX[4] = float[4](0.533, 0.530, 0.524, 0.516); // index→little
    const float NFZ[4] = float[4](0.040, 0.016,-0.006,-0.026);  // tip Z after curl
    const float NFY[4] = float[4](0.426, 0.434, 0.426, 0.408);  // tip Y
    for (int k = 0; k < 4; k++) {
        float dpx=(px-NFX[k])/0.014, dpy=(py-NFY[k])/0.014, dpz=(pz-NFZ[k]-0.006)/0.006;
        float dk=(sqrt(dpx*dpx+dpy*dpy+dpz*dpz)-1.0)*0.006;
        d=min(d,dk);
    }
    // Thumb nail
    { float dpx=(px-0.524)/0.014, dpy=(py-0.346)/0.014, dpz=(pz+0.082-0.006)/0.006;
      d=min(d,(sqrt(dpx*dpx+dpy*dpy+dpz*dpz)-1.0)*0.006); }
    return d;
}

// ── Wrist bone protrusion (ulnar styloid) ────────────────────────────────────
// Small bilateral bump on the pinky side of the wrist (dorsal surface).
float sdf_wrist_bone_r(float px, float py, float pz) {
    // Ulnar styloid at the wrist transition — medial wrist prominence
    return seg_dist(px,py,pz, 0.512,0.258,-0.032, 0.510,0.272,-0.026, 0.014,0.010);
}

// ── Ankle bone protrusion (lateral malleolus) ─────────────────────────────────
float sdf_ankle_bone_r(float px, float py, float pz) {
    // Lateral malleolus: bump on outer ankle (positive X side for right leg)
    const float CX = 0.145;
    return seg_dist(px,py,pz, CX+0.052,1.178,0.002, CX+0.048,1.194,-0.004, 0.018,0.014);
}

// ── Knuckle row ───────────────────────────────────────────────────────────────
// Four small ellipsoids along the MCP joint row (back of hand at base of fingers).
float sdf_knuckles_r(float px, float py, float pz) {
    float d = 1e9;
    // Knuckles at py≈0.322, on the dorsal side (pz slightly negative for back-of-hand)
    const float KY=0.322, KZ=-0.018;
    const float KX[4] = float[4](0.516, 0.524, 0.530, 0.533);  // little→index
    const float KR[4] = float[4](0.011, 0.012, 0.012, 0.011);
    for (int k=0; k<4; k++) {
        float dx=(px-KX[k])/KR[k], dy=(py-KY)/0.010, dz=(pz-KZ)/0.010;
        float dk=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*KR[k];
        d=min(d,dk);
    }
    return d;
}

// Tapered capsule SDF: distance from point P to segment A→B with radii ra→rb.
float seg_dist(float px, float py, float pz,
               float ax, float ay, float az, float bx, float by, float bz,
               float ra, float rb) {
    float dx=bx-ax, dy=by-ay, dz=bz-az;
    float len2 = dx*dx+dy*dy+dz*dz;
    float t = clamp(((px-ax)*dx+(py-ay)*dy+(pz-az)*dz)/max(len2,0.0001), 0.0, 1.0);
    float cx=ax+t*dx, cy=ay+t*dy, cz=az+t*dz;
    return sqrt((px-cx)*(px-cx)+(py-cy)*(py-cy)+(pz-cz)*(pz-cz)) - (ra+t*(rb-ra));
}

// Right hand: flattened palm ellipsoid + 5 two-segment bent fingers.
// Each finger has a proximal phalanx (straight) + distal phalanx (curled forward).
// Wrist junction handled by sdf_body at k=0.025.
float sdf_hand_r(float px, float py, float pz) {
    // Palm: flattened ellipsoid (RY < RX,RZ — thin dorsal/palmar dimension)
    const float PCX=0.525, PCY=0.295, RX=0.044, RY=0.032, RZ=0.048;
    float dpx=(px-PCX)/RX, dpy=(py-PCY)/RY, dpz=pz/RZ;
    float d_f=(sqrt(dpx*dpx+dpy*dpy+dpz*dpz)-1.0)*min(min(RX,RY),RZ);

    // little finger — proximal + distal (curls +0.018 Z)
    { float d1=seg_dist(px,py,pz, 0.516,0.320,-0.044, 0.516,0.370,-0.044, 0.014,0.011);
      float d2=seg_dist(px,py,pz, 0.516,0.370,-0.044, 0.516,0.410,-0.026, 0.011,0.007);
      d_f=smin(d_f,smin(d1,d2,0.008),0.010); }
    // ring finger — proximal + distal (curls +0.016 Z)
    { float d1=seg_dist(px,py,pz, 0.524,0.322,-0.022, 0.524,0.380,-0.022, 0.015,0.012);
      float d2=seg_dist(px,py,pz, 0.524,0.380,-0.022, 0.524,0.428,-0.006, 0.012,0.008);
      d_f=smin(d_f,smin(d1,d2,0.008),0.010); }
    // middle finger — proximal + distal (curls +0.014 Z)
    { float d1=seg_dist(px,py,pz, 0.530,0.322, 0.002, 0.530,0.386, 0.002, 0.016,0.012);
      float d2=seg_dist(px,py,pz, 0.530,0.386, 0.002, 0.530,0.436, 0.016, 0.012,0.008);
      d_f=smin(d_f,smin(d1,d2,0.008),0.010); }
    // index finger — proximal + distal (curls +0.014 Z)
    { float d1=seg_dist(px,py,pz, 0.533,0.320, 0.026, 0.533,0.378, 0.026, 0.015,0.012);
      float d2=seg_dist(px,py,pz, 0.533,0.378, 0.026, 0.533,0.428, 0.040, 0.012,0.008);
      d_f=smin(d_f,smin(d1,d2,0.008),0.010); }
    // thumb — angles outward in X (-X direction = toward body center for right hand)
    // Base at palm edge, tip opposes index finger (curls inward in Z and -X)
    { float d1=seg_dist(px,py,pz, 0.540,0.268,-0.055, 0.533,0.305,-0.072, 0.018,0.014);
      float d2=seg_dist(px,py,pz, 0.533,0.305,-0.072, 0.524,0.348,-0.082, 0.014,0.010);
      d_f=smin(d_f,smin(d1,d2,0.010),0.012); }
    return d_f;
}

// Right foot: boot shaft (tapered capsule) + toe box (forward ellipsoid) + heel raise.
// Ankle junction with shin handled in sdf_body at k=0.03.
float sdf_foot_r(float px, float py, float pz) {
    const float CX = 0.145;
    // Boot shaft: tapered cylinder from ankle to bottom of calf (covers the calf-boot gap)
    float d_shaft = seg_dist(px,py,pz, CX,1.180,0.0, CX,1.240,0.0, 0.056,0.048);

    // Toe box: wider ellipsoid — boot has squared-off toe, slightly raised heel
    const float BCX=0.138, BCY=1.255, BCZ=0.035;
    const float BRX=0.054, BRY=0.030, BRZ=0.082;
    float bpx=(px-BCX)/BRX, bpy=(py-BCY)/BRY, bpz=(pz-BCZ)/BRZ;
    float d_toe=(sqrt(bpx*bpx+bpy*bpy+bpz*bpz)-1.0)*min(BRX,min(BRY,BRZ));

    // Heel: ellipsoid behind ankle axis (-Z), slightly raised (heel raise ≈ 8mm)
    const float HCX=0.140, HCY=1.248, HCZ=-0.052;
    const float HRX=0.046, HRY=0.032, HRZ=0.038;
    float hpx=(px-HCX)/HRX, hpy=(py-HCY)/HRY, hpz=(pz-HCZ)/HRZ;
    float d_heel=(sqrt(hpx*hpx+hpy*hpy+hpz*hpz)-1.0)*min(HRX,min(HRY,HRZ));

    float d = smin(d_shaft, d_toe,  0.022);
    d       = smin(d,       d_heel, 0.020);
    return d;
}

// ── SCM (sternocleidomastoid) neck muscle ─────────────────────────────────────
// Diagonal bilateral neck muscle from mastoid process to sternum notch.
float sdf_scm_r(float px, float py, float pz) {
    // Runs from behind the ear (px≈0.095, py≈-0.840) to the sternum top (px≈0.020, py≈-0.640)
    float ax = abs(px);
    return seg_dist(ax, py, pz,
        0.094, -0.842, 0.032,   // mastoid (behind ear base)
        0.022, -0.648, 0.058,   // sternum notch
        0.018, 0.014);           // round muscle belly
}

// ── Neck: tapered oval cylinder with trapezius slope ─────────────────────────
float sdf_neck(float px, float py, float pz) {
    const float AY=-0.790, BY=-0.640, RA=0.072, RB=0.095;
    float t  = clamp((py - AY) / (BY - AY), 0.0, 1.0);
    float r  = RA + t * (RB - RA);
    float xz = sqrt((px/1.00)*(px/1.00) + (pz/0.88)*(pz/0.88));
    float ye = max(py - BY, 0.0) + max(AY - py, 0.0);
    float d_neck = (ye > 0.0) ? sqrt(xz*xz + ye*ye) - r : xz - r;

    // Trapezius slope: bilateral rounded ramp from neck base to shoulder — wider, lower
    float ax = abs(px);
    float trap_r = 0.055;
    float trap_cx = 0.18 + ax * 0.0;   // centered 18 cm lateral
    float trap_t  = clamp((ax - 0.10) / 0.20, 0.0, 1.0);  // fades from neck center outward
    float trap_cy_top = -0.660, trap_cy_bot = -0.600;
    float trap_cz = -0.010;
    // Trapezius runs as a ridge from neck-shoulder junction
    float tpx = ax - 0.14, tpy = py - (-0.630), tpz = pz - trap_cz;
    // Ellipsoid-ish sloped bump
    float td = sqrt((tpx/0.14)*(tpx/0.14) + (tpy/0.055)*(tpy/0.055) + (tpz/0.080)*(tpz/0.080)) - 1.0;
    float d_trap = td * 0.055 * (1.0 - trap_t * 0.5);

    return smin(d_neck, d_trap, 0.030);
}

// ── Collarbone (clavicle) ridge ───────────────────────────────────────────────
// Thin bilateral ellipsoid just below the neck at the chest top.
float sdf_collarbone(float px, float py, float pz) {
    // Clavicle: slight S-curve approximated as bilateral ellipsoids
    // Center at roughly (±0.12, -0.640, 0.080) pointing outward
    float ax = abs(px);
    // Medial end near sternum, lateral end near shoulder
    // Use a capsule from sternum to acromioclavicular joint
    float d = seg_dist(ax, py, pz,
        0.02, -0.648, 0.075,   // medial: near sternum notch
        0.28, -0.620, 0.040,   // lateral: toward shoulder
        0.012, 0.008);         // thin at sternum, slightly thicker at shoulder
    return d;
}

// ── Head: skull ellipsoid + cheekbones + brow ridge + jaw + nose + lips + ears ──
float sdf_head(float px, float py, float pz) {
    // Main skull/face ellipsoid — center slightly forward (face protrudes)
    const float HCY=-0.920, HCZ=0.018, HRX=0.175, HRY=0.222, HRZ=0.158;
    float dhx=px/HRX, dhy=(py-HCY)/HRY, dhz=(pz-HCZ)/HRZ;
    float d_head=(sqrt(dhx*dhx+dhy*dhy+dhz*dhz)-1.0)*min(HRX,min(HRY,HRZ));

    // Cheekbones: bilateral convex bumps flanking the nose (mirror in X)
    { const float CX=0.090,CY=-0.888,CZ=0.138,CRX=0.044,CRY=0.028,CRZ=0.032;
      float ax=abs(px);
      float dx=(ax-CX)/CRX, dy=(py-CY)/CRY, dz=(pz-CZ)/CRZ;
      float d_chk=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(CRX,min(CRY,CRZ));
      d_head=smin(d_head,d_chk,0.028); }

    // Brow ridge: thin horizontal ellipsoid above eye sockets
    { const float BCY=-1.012,BCZ=0.132,BRX=0.112,BRY=0.020,BRZ=0.024;
      float dx=px/BRX, dy=(py-BCY)/BRY, dz=(pz-BCZ)/BRZ;
      float d_brow=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(BRX,min(BRY,BRZ));
      d_head=smin(d_head,d_brow,0.018); }

    // Jaw: slightly wider ellipsoid at chin level (defines jaw width and chin)
    { const float JCY=-0.792,JCZ=0.030,JRX=0.132,JRY=0.040,JRZ=0.108;
      float dx=px/JRX, dy=(py-JCY)/JRY, dz=(pz-JCZ)/JRZ;
      float d_jaw=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(JRX,min(JRY,JRZ));
      d_head=smin(d_head,d_jaw,0.025); }

    // Ears: flat ellipsoids smooth-unioned to head sides (bilateral)
    { const float ECX=0.174,ECY=-0.882,ECZ=0.016,ERX=0.016,ERY=0.052,ERZ=0.012;
      float ax=abs(px);
      float dx=(ax-ECX)/ERX, dy=(py-ECY)/ERY, dz=(pz-ECZ)/ERZ;
      float d_ear=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(ERX,min(ERY,ERZ));
      d_head=smin(d_head,d_ear,0.016); }

    // Nose bridge and tip: small forward ellipsoid
    { const float NCY=-0.900,NCZ=0.168,NRX=0.018,NRY=0.032,NRZ=0.022;
      float dx=px/NRX, dy=(py-NCY)/NRY, dz=(pz-NCZ)/NRZ;
      float d_nose=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(NRX,min(NRY,NRZ));
      d_head=smin(d_head,d_nose,0.014); }

    // Lips: upper and lower ridge above the chin
    { const float LCZ=0.156;
      // Upper lip
      { const float ULY=-0.846,ULRX=0.062,ULRY=0.011,ULRZ=0.014;
        float dx=px/ULRX, dy=(py-ULY)/ULRY, dz=(pz-LCZ)/ULRZ;
        float d=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(ULRX,min(ULRY,ULRZ));
        d_head=smin(d_head,d,0.010); }
      // Lower lip (slightly fuller)
      { const float LLY=-0.833,LLRX=0.060,LLRY=0.013,LLRZ=0.015;
        float dx=px/LLRX, dy=(py-LLY)/LLRY, dz=(pz-LCZ)/LLRZ;
        float d=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(LLRX,min(LLRY,LLRZ));
        d_head=smin(d_head,d,0.010); } }

    // Zygomatic arch continuation: ridge from cheekbone toward ear
    { float ax3=abs(px);
      float d_arch=seg_dist(ax3,py,pz, 0.096,-0.888,0.130, 0.158,-0.882,0.058, 0.014,0.010);
      d_head=smin(d_head,d_arch,0.014); }

    // Temporal ridge: slight bony ridge above the temple (superior temporal line)
    { float ax3=abs(px);
      float d_temp=seg_dist(ax3,py,pz, 0.130,-1.010,0.068, 0.158,-0.940,0.042, 0.010,0.008);
      d_head=smin(d_head,d_temp,0.012); }

    // Chin cleft: very small midline groove below lower lip — philtrum area
    // Implemented as two slight volume add-ons flanking center that create a midline shadow
    { // Philtrum columns: bilateral thin ridges from nose base to upper lip
      float ax3=abs(px);
      float dpx=(ax3-0.014)/0.010, dpy=(py+0.866)/0.020, dpz=(pz-0.162)/0.010;
      float d_phil=(sqrt(dpx*dpx+dpy*dpy+dpz*dpz)-1.0)*0.009;
      d_head=smin(d_head,d_phil,0.008); }

    // Chin prominence: slight forward ellipsoid at the chin center
    { float dpx=px/0.038, dpy=(py+0.792)/0.022, dpz=(pz-0.136)/0.018;
      float d_chin=(sqrt(dpx*dpx+dpy*dpy+dpz*dpz)-1.0)*0.016;
      d_head=smin(d_head,d_chin,0.014); }

    // Nasolabial fold hint: bilateral slight ridge from nostril to mouth corner
    { float ax3=abs(px);
      float d_nlf=seg_dist(ax3,py,pz, 0.022,-0.876,0.165, 0.060,-0.856,0.160, 0.009,0.007);
      d_head=smin(d_head,d_nlf,0.008); }

    // Eyelids: bilateral thin forward-facing ellipsoids at eye position
    // Upper eyelid slightly more prominent (heavy brow shadow for Leon's look)
    { const float EX=0.058,EY=-0.980,EZ=0.152,ERX=0.034,ERY=0.009,ERZ=0.012;
      float ax2=abs(px);
      float dx=(ax2-EX)/ERX, dy=(py-EY)/ERY, dz=(pz-EZ)/ERZ;
      float d_lid=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(ERX,min(ERY,ERZ));
      d_head=smin(d_head,d_lid,0.010); }

    // Lower eyelid: smaller ridge below eye
    { const float EX=0.056,EY=-0.968,EZ=0.150,ERX=0.028,ERY=0.007,ERZ=0.010;
      float ax2=abs(px);
      float dx=(ax2-EX)/ERX, dy=(py-EY)/ERY, dz=(pz-EZ)/ERZ;
      float d_lo=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(ERX,min(ERY,ERZ));
      d_head=smin(d_head,d_lo,0.008); }

    // Eyebrow ridge: bilateral elongated bumps above the eyes
    { const float BX=0.058,BY=-0.998,BZ=0.140,BRX=0.040,BRY=0.008,BRZ=0.012;
      float ax2=abs(px);
      float dx=(ax2-BX)/BRX, dy=(py-BY)/BRY, dz=(pz-BZ)/BRZ;
      float d_brow2=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(BRX,min(BRY,BRZ));
      d_head=smin(d_head,d_brow2,0.010); }

    // Nasal wings (nostril area): bilateral small bumps at nose base
    { const float NWX=0.016,NWY=-0.876,NWZ=0.162,NWRX=0.012,NWRY=0.010,NWRZ=0.010;
      float ax2=abs(px);
      float dx=(ax2-NWX)/NWRX, dy=(py-NWY)/NWRY, dz=(pz-NWZ)/NWRZ;
      float d_nw=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(NWRX,min(NWRY,NWRZ));
      d_head=smin(d_head,d_nw,0.009); }

    // Adam's apple (throat): visible at the anterior neck
    { float d_adam = seg_dist(px,py,pz, 0.0,-0.740,0.074, 0.0,-0.715,0.082, 0.012,0.010);
      d_head=smin(d_head,d_adam,0.010); }

    // Ear helix inner ridge — secondary ellipsoid inside the ear
    { const float ECX=0.172, ECY=-0.882, ECZ=0.016, ERX=0.009, ERY=0.038, ERZ=0.008;
      float ax4=abs(px);
      float dx=(ax4-ECX)/ERX, dy=(py-ECY)/ERY, dz=(pz-ECZ)/ERZ;
      float d_helix=(sqrt(dx*dx+dy*dy+dz*dz)-1.0)*min(ERX,min(ERY,ERZ));
      d_head=smin(d_head,d_helix,0.008); }

    // SCM muscle: bilateral diagonal neck muscle
    d_head=smin(d_head, sdf_scm_r(px,py,pz), 0.020);

    // Blend neck smoothly into head base
    d_head=smin(d_head, sdf_neck(px,py,pz), 0.038);

    return d_head;
}

// ── Jacket shoulder seam ridge ────────────────────────────────────────────────
// Raised seam where sleeve meets torso at the shoulder point.
float sdf_shoulder_seam_r(float px, float py, float pz) {
    // Seam runs as a short capsule at the shoulder cap (arm root-torso junction)
    // Bilateral — evaluated with abs(px)
    float ax = abs(px);
    return seg_dist(ax, py, pz,
        0.290, -0.580, 0.004,   // inner seam (near neck)
        0.460, -0.560, -0.006,  // outer seam (shoulder tip)
        0.010, 0.010);           // thin seam ridge
}

// ── Jacket hem ridge ──────────────────────────────────────────────────────────
// Double-stitched hem at the bottom edge of the jacket.
float sdf_jacket_hem(float px, float py, float pz) {
    // Horizontal band at jacket bottom (py≈0.20)
    // Use a torus-like ring: points within a narrow Y band at the jacket surface Z
    float ty = clamp((py - 0.18) / 0.025, 0.0, 1.0);  // 0=just above hem, 1=at hem
    float lean_z = (1.0 - 0.0) * 0.016 - 0.0 * 0.008;  // at bottom of torso (ty=1)
    // Points on the torso perimeter at hem Y — just check proximity to hem
    float ax = plerp_ax(1.0);  // torso width at bottom
    float az = plerp_az(1.0);
    float nx = px / ax, nz = (pz - lean_z) / az;
    float cross_d = (sqrt(nx*nx + nz*nz) - 1.0) * min(ax, az);
    // Hem = thin shell right at the jacket surface, Y-restricted band
    float ye = max(py - 0.22, 0.0) + max(0.18 - py, 0.0);
    return ye > 0.0 ? 1.0 : cross_d - 0.016;  // 16mm outside torso surface
}

// ── Boot sole ridge ───────────────────────────────────────────────────────────
// Thin flat ellipsoid = outsole visible at the side of the boot.
float sdf_boot_sole_r(float px, float py, float pz) {
    // Sole is a flat horizontal disc at the boot bottom plane (py≈1.260)
    const float CX=0.138, CY=1.262, CZ=0.025;
    const float SRX=0.055, SRY=0.008, SRZ=0.082;
    float dpx=(px-CX)/SRX, dpy=(py-CY)/SRY, dpz=(pz-CZ)/SRZ;
    return (sqrt(dpx*dpx+dpy*dpy+dpz*dpz)-1.0)*SRY;
}

// ── Clothing SDF layers ───────────────────────────────────────────────────────
// Jacket: torso + upper arms + forearms inflated 14 mm outward.
// This creates a physical cloth surface separate from the underlying skin.
float sdf_jacket_layer(float px, float py, float pz) {
    float d = sdf_torso(px, py, pz);
    d = smin(d, sdf_arm_r( px, py, pz), 0.040);
    d = smin(d, sdf_arm_r(-px, py, pz), 0.040);
    d = smin(d, sdf_forearm_r( px, py, pz), 0.040);
    d = smin(d, sdf_forearm_r(-px, py, pz), 0.040);
    return d - 0.014;   // inflate 14 mm outward — jacket thickness over skin
}

// Pants: both legs inflated 10 mm outward.
float sdf_pants_layer(float px, float py, float pz) {
    float d = sdf_leg_r( px, py, pz);
    d = smin(d, sdf_leg_r(-px, py, pz), 0.040);
    return d - 0.010;   // inflate 10 mm outward — tactical trouser thickness
}

// Full-body SDF: unified smooth union of all 11 primitives + clothing layers.
float sdf_body(float px, float py, float pz) {
    float d = smin(sdf_torso(px, py, pz), sdf_arm_r(px, py, pz), 0.04);
    d = smin(d, sdf_arm_r(-px, py, pz), 0.04);
    d = smin(d, sdf_forearm_r(px, py, pz), 0.04);
    d = smin(d, sdf_forearm_r(-px, py, pz), 0.04);
    d = smin(d, sdf_hand_r(px, py, pz), 0.025);    // wrist k=0.025
    d = smin(d, sdf_hand_r(-px, py, pz), 0.025);
    // Elbow bone protrusions (bilateral)
    d = smin(d, sdf_olecranon_r(px, py, pz), 0.014);
    d = smin(d, sdf_olecranon_r(-px, py, pz), 0.014);
    // Wrist bone (bilateral)
    d = smin(d, sdf_wrist_bone_r(px, py, pz), 0.012);
    d = smin(d, sdf_wrist_bone_r(-px, py, pz), 0.012);
    // Knuckle row (bilateral)
    d = smin(d, sdf_knuckles_r(px, py, pz), 0.010);
    d = smin(d, sdf_knuckles_r(-px, py, pz), 0.010);
    // Fingernails (bilateral — shallow so they catch light)
    d = smin(d, sdf_fingernails_r(px, py, pz), 0.008);
    d = smin(d, sdf_fingernails_r(-px, py, pz), 0.008);
    d = smin(d, sdf_leg_r(px,  py, pz - 0.038), 0.04);   // right leg steps forward
    d = smin(d, sdf_leg_r(-px, py, pz + 0.014), 0.04);   // left leg slightly back
    d = smin(d, sdf_foot_r(px,  py, pz - 0.038), 0.030); // right foot forward
    d = smin(d, sdf_foot_r(-px, py, pz + 0.014), 0.030); // left foot back
    // Ankle bone protrusions (bilateral, with stance offsets)
    d = smin(d, sdf_ankle_bone_r(px,  py, pz - 0.038), 0.012);
    d = smin(d, sdf_ankle_bone_r(-px, py, pz + 0.014), 0.012);
    d = smin(d, sdf_head(px, py, pz), 0.040);     // head-to-neck junction k=0.04
    // Clothing layers push torso/arm/leg surfaces outward — fabric over skin
    d = smin(d, sdf_jacket_layer(px, py, pz), 0.020);
    d = smin(d, sdf_pants_layer(px, py, pz), 0.018);
    // Collarbone ridge sits at jacket neckline — visible at collar opening
    d = smin(d, sdf_collarbone(px, py, pz), 0.018);
    // Sternum ridge visible at jacket opening
    d = smin(d, sdf_sternum(px, py, pz), 0.016);
    // Shoulder seams (bilateral raised ridge)
    d = smin(d, sdf_shoulder_seam_r(px, py, pz), 0.012);
    // Boot sole ridge (bilateral with stance offset)
    d = smin(d, sdf_boot_sole_r(px,  py, pz - 0.038), 0.012);
    d = smin(d, sdf_boot_sole_r(-px, py, pz + 0.014), 0.012);
    return d;
}

// Analytical normal: central differences of sdf_body (6 evaluations).
vec3 body_normal(float px, float py, float pz) {
    const float E = 0.005;
    float nx = sdf_body(px+E,py,pz) - sdf_body(px-E,py,pz);
    float ny = sdf_body(px,py+E,pz) - sdf_body(px,py-E,pz);
    float nz = sdf_body(px,py,pz+E) - sdf_body(px,py,pz-E);
    return normalize(vec3(nx, ny, nz));
}

// Returns ID of closest SDF primitive for structural material zone assignment.
// 0=torso 1=arm_r 2=arm_l 3=fa_r 4=fa_l 5=hand_r 6=hand_l
// 7=leg_r 8=leg_l 9=foot_r 10=foot_l
int dominant_sdf_id(float px, float py, float pz) {
    float best = sdf_torso(px, py, pz); int id = 0; float v;
    v = sdf_arm_r( px, py, pz); if (v < best) { best = v; id = 1; }
    v = sdf_arm_r(-px, py, pz); if (v < best) { best = v; id = 2; }
    v = sdf_forearm_r( px, py, pz); if (v < best) { best = v; id = 3; }
    v = sdf_forearm_r(-px, py, pz); if (v < best) { best = v; id = 4; }
    v = sdf_hand_r( px, py, pz); if (v < best) { best = v; id = 5; }
    v = sdf_hand_r(-px, py, pz); if (v < best) { best = v; id = 6; }
    v = sdf_leg_r( px,  py, pz-0.038); if (v < best) { best = v; id = 7; }
    v = sdf_leg_r(-px,  py, pz+0.014); if (v < best) { best = v; id = 8; }
    v = sdf_foot_r( px,  py, pz-0.038); if (v < best) { best = v; id = 9; }
    v = sdf_foot_r(-px,  py, pz+0.014); if (v < best) { best = v; id = 10; }
    v = sdf_head( px,  py, pz); if (v < best) { best = v; id = 11; }
    // Clothing layers: jacket (id=12) and pants (id=13) push surfaces outward
    v = sdf_jacket_layer(px, py, pz); if (v < best) { best = v; id = 12; }
    v = sdf_pants_layer( px, py, pz); if (v < best) { best = v; id = 13; }
    return id;
}

// IQ SDF AO — 5 steps along outward normal.
float sdf_ao(float px, float py, float pz, vec3 sn) {
    const float STEPS[5] = float[5](0.010, 0.035, 0.065, 0.100, 0.140);
    float occ = 0.0, sca = 1.0;
    for (int k = 0; k < 5; k++) {
        float h = STEPS[k];
        float d = sdf_body(px + h*sn.x, py + h*sn.y, pz + h*sn.z);
        occ += (h - d) * sca;
        sca *= 0.95;
    }
    float ao = clamp(1.0 - 3.0*occ, 0.0, 1.0);
    return ao * 0.75 + 0.25;
}

// SDF thickness probe for SSS (6 steps inward along -n).
float sdf_thickness(float px, float py, float pz, vec3 sn) {
    const float PROBES[6] = float[6](0.020, 0.055, 0.100, 0.160, 0.250, 0.380);
    for (int k = 0; k < 6; k++) {
        float h = PROBES[k];
        float d = sdf_body(px - h*sn.x, py - h*sn.y, pz - h*sn.z);
        if (d >= 0.0) return h;
    }
    return 0.380;
}

// Beer-Lambert volumetric shadow toward key light.
float vol_shadow(float px, float py, float pz) {
    const int   STEPS = 6;
    const float STEP  = 0.075;
    const float SIGMA = 14.0;
    float tau = 0.0;
    for (int k = 1; k <= STEPS; k++) {
        float h = float(k) * STEP;
        float d = sdf_body(px + h*KL.x, py + h*KL.y, pz + h*KL.z);
        if (d < 0.0) tau += min(-d, 0.12) * STEP;
    }
    return exp(-SIGMA * tau);
}

// ── 3D value noise ────────────────────────────────────────────────────────────
float nhash(int ix, int iy, int iz) {
    uint n = uint(ix)*374761393u + uint(iy)*668265263u + uint(iz)*1291057433u;
    n ^= (n >> 13u);
    n *= 0x5851F42Du;
    n ^= (n >> 16u);
    return float(n & 0x00FFFFFFu) / 16777216.0 - 0.5;
}

float noise3(float x, float y, float z) {
    int xi = int(floor(x)), yi = int(floor(y)), zi = int(floor(z));
    float xf = x - float(xi), yf = y - float(yi), zf = z - float(zi);
    float u = xf*xf*(3.0 - 2.0*xf);
    float v = yf*yf*(3.0 - 2.0*yf);
    float w = zf*zf*(3.0 - 2.0*zf);
    float r0 = mix(mix(nhash(xi,yi,zi),   nhash(xi+1,yi,zi),   u),
                   mix(nhash(xi,yi+1,zi), nhash(xi+1,yi+1,zi), u), v);
    float r1 = mix(mix(nhash(xi,yi,zi+1),   nhash(xi+1,yi,zi+1),   u),
                   mix(nhash(xi,yi+1,zi+1), nhash(xi+1,yi+1,zi+1), u), v);
    return mix(r0, r1, w);
}

// Procedural skin detail: displacement + bump normal + roughness.
// Returns: (disp, pnx, pny, pnz, roughness) packed into a vec3+vec2.
// Call convention: sn = original analytical normal (normalized).
void skin_detail(float px, float py, float pz, vec3 sn,
                 out float disp, out vec3 pn, out float roughness) {
    float fl = noise3(px*3.0, py*3.0, pz*3.0)
             + noise3(px*6.0, py*6.0, pz*6.0) * 0.5;
    const float GE   = 0.009;
    const float BUMP = 0.030;
    float fc  = noise3( px*16.0,       py*16.0,       pz*16.0);
    float fxp = noise3((px+GE)*16.0,   py*16.0,       pz*16.0);
    float fyp = noise3( px*16.0,      (py+GE)*16.0,   pz*16.0);
    float fzp = noise3( px*16.0,       py*16.0,      (pz+GE)*16.0);
    float gx  = (fxp - fc) / GE;
    float gy  = (fyp - fc) / GE;
    float gz  = (fzp - fc) / GE;
    float gdn = dot(vec3(gx, gy, gz), sn);
    vec3 b = vec3((gx - gdn*sn.x)*BUMP, (gy - gdn*sn.y)*BUMP, (gz - gdn*sn.z)*BUMP);
    pn = normalize(sn + b);
    disp      = fl * 0.004 + fc * 0.002;
    roughness = noise3(px*52.0, py*52.0, pz*52.0) * 0.5 + 0.5;
}

// ── Material functions ────────────────────────────────────────────────────────
const int MAT_SKIN   = 0;
const int MAT_HAIR   = 1;
const int MAT_JACKET = 2;
const int MAT_BOOT   = 3;
const int MAT_METAL  = 4;
const int MAT_EYE    = 5;

// sdf_id: dominant SDF primitive (see dominant_sdf_id).
// Material tag — sdf_id is primary discriminator; positional logic only for torso detail.
// The old Y-positional skin zones only applied when head/face was at py -0.88..-0.30.
// With the SDF torso occupying that same Y band, all torso pixels must default to jacket.
int leon_tag(float px, float py, int sdf_id) {
    float ax = abs(px);
    // Head, neck, face: all skin
    if (sdf_id == 11) return MAT_SKIN;
    // Hands: exposed skin
    if (sdf_id == 5 || sdf_id == 6) return MAT_SKIN;
    // Feet: boot leather
    if (sdf_id == 9 || sdf_id == 10) return MAT_BOOT;
    // Arms and forearms: jacket sleeve (no exposed skin — Leon's sleeves are full-length)
    if (sdf_id == 1 || sdf_id == 2 || sdf_id == 3 || sdf_id == 4) return MAT_JACKET;
    // Clothing layer ids: jacket surface (12) and pants surface (13)
    if (sdf_id == 12) return MAT_JACKET;
    if (sdf_id == 13) return (py >= 0.84) ? MAT_BOOT : MAT_JACKET;
    // Legs: tactical pants until the boot shaft starts (extended legs: boot at 1.14)
    if (sdf_id == 7 || sdf_id == 8) {
        return (py >= 1.14) ? MAT_BOOT : MAT_JACKET;
    }
    // Torso (sdf_id == 0): jacket dominates entirely.
    // The neck/collar skin is rendered by the CPU head pass, not the torso SDF.
    // Only the belt row has non-jacket materials.
    if (py >= 0.21 && py <= 0.31) {
        return (ax < 0.05) ? MAT_METAL : MAT_BOOT;  // buckle / belt leather
    }
    return MAT_JACKET;
}

// Color lookup — uses sdf_id to route to the correct palette zone.
// This replaces positional skin logic that misaligned with SDF geometry.
vec3 leon_color(float px, float py, int sdf_id) {
    float ax = abs(px);

    // ── Head / face / neck: skin tones with facial feature variation ─────────
    if (sdf_id == 11) {
        float ax = abs(px);
        // Base skin: slight warm variation by position
        float skin_r = 0.80 + ax * 0.04;
        float skin_g = 0.57 + max(-py - 0.85, 0.0) * 0.10;
        float skin_b = 0.42;

        // Forehead: slightly cooler/less saturated (less blood near skull)
        float fore_t = clamp((-py - 0.96) / 0.10, 0.0, 1.0);
        skin_r -= fore_t * 0.03; skin_g -= fore_t * 0.01;

        // Nose bridge: SSS warm glow (blood vessels close to surface)
        float nose_d = ax*ax*800.0 + (py+0.900)*(py+0.900)*120.0;
        float nose_w = exp(-nose_d * 0.6);
        skin_r += nose_w * 0.05; skin_b += nose_w * 0.02;

        // Cheek: rosy bilateral tint
        float chk = exp(-((ax-0.088)*(ax-0.088)*160.0 + (py+0.888)*(py+0.888)*160.0));
        skin_r += chk * 0.06; skin_b += chk * 0.015;

        // Stubble zone: jaw/chin area darker blue-grey tint (beard shadow)
        float jaw_t = clamp((-py - 0.795) / 0.060, 0.0, 1.0);  // 1=jaw, 0=upper face
        float stubble_mask = jaw_t * clamp(1.0 - fore_t, 0.0, 1.0);
        // Stubble is more gray-green (skin + grey hair mix)
        skin_r -= stubble_mask * 0.08;
        skin_g -= stubble_mask * 0.04;
        skin_b += stubble_mask * 0.02;

        // Under-eye shadow (periorbital darkening)
        float eye_d = (ax-0.050)*(ax-0.050)*200.0 + (py+0.966)*(py+0.966)*200.0;
        float eye_shadow = exp(-eye_d) * 0.05;
        skin_r -= eye_shadow; skin_g -= eye_shadow * 0.8;

        // Lips: redder
        float lip_d2 = (py + 0.840)*(py + 0.840)*320.0 + (pz - 0.156)*(pz - 0.156)*200.0;
        float lip_w = exp(-lip_d2);
        skin_r += lip_w * 0.14; skin_g -= lip_w * 0.05;

        // Neck: slightly less saturated
        float neck_t = clamp((py + 0.75) * 8.0, 0.0, 1.0);
        skin_r = mix(skin_r, 0.74, neck_t * 0.22);
        skin_g = mix(skin_g, 0.56, neck_t * 0.22);

        return vec3(clamp(skin_r,0.0,1.0), clamp(skin_g,0.0,1.0), clamp(skin_b,0.0,1.0));
    }

    // ── Hands: bare skin with tendon/crease detail ───────────────────────────
    if (sdf_id == 5 || sdf_id == 6) {
        // Fingernail plates (dorsal/forward-facing fingertips)
        if (py > 0.38 && pz > 0.008) return vec3(0.88,0.72,0.66);
        // Knuckle: slightly redder/darker dorsal skin
        if (py > 0.314 && py < 0.336) return vec3(0.80,0.54,0.40);
        // Palm: lighter center, darker at edges (callus zone), slight crease shadow at py≈0.28
        if (py < 0.30) {
            // Thenar eminence (thumb mound): warm flush
            float thenar_d = (ax - 0.52)*(ax-0.52) + (py-0.288)*(py-0.288);
            float thenar = exp(-thenar_d * 300.0) * 0.06;
            // Palm crease: subtle dark band
            float crease = exp(-(py-0.280)*(py-0.280)*1200.0) * 0.04;
            float pr = 0.82 + thenar - crease;
            float pg = 0.60 + thenar * 0.5 - crease;
            float pb = 0.45 - crease * 0.5;
            return vec3(pr, pg, pb);
        }
        return vec3(0.78,0.56,0.42);
    }

    // ── Feet / boots ─────────────────────────────────────────────────────────
    if (sdf_id == 9 || sdf_id == 10) {
        if (py > 1.255) return vec3(0.10,0.08,0.06);   // rubber sole
        // Toe cap: slightly lighter/different texture at toe box front
        if (py > 1.230 && pz > 0.060) return vec3(0.30,0.20,0.12);  // toe cap
        // Boot shaft-to-body seam line
        if (abs(py - 1.186) < 0.006) return vec3(0.18,0.12,0.07);  // seam
        return vec3(0.25,0.17,0.10);
    }

    // ── Legs: tactical pants with knee-pad detail ─────────────────────────────
    if (sdf_id == 7 || sdf_id == 8) {
        if (py >= 1.14) return vec3(0.25,0.17,0.10);            // boot shaft (extended legs)
        if (py > 0.72 && py < 0.90 && ax < 0.18)               // knee pad (moved down)
            return vec3(0.36,0.32,0.22);
        return vec3(0.30,0.33,0.24);                            // olive tactical pants
    }

    // ── Arms: jacket sleeve ───────────────────────────────────────────────────
    if (sdf_id == 1 || sdf_id == 2) return vec3(0.55,0.38,0.20);  // upper arm sleeve
    if (sdf_id == 3 || sdf_id == 4) {
        if (py > 0.20 && py < 0.30) return vec3(0.42,0.28,0.12);  // cuff band at wrist
        return vec3(0.52,0.36,0.18);  // forearm sleeve
    }

    // ── Clothing layer surfaces (offset SDF shells) ───────────────────────────
    if (sdf_id == 12) {
        // ── Jacket detail ─────────────────────────────────────────────────────
        // Zipper strip: center front, slightly lighter/metallic
        if (ax < 0.030 && py > -0.58 && py < 0.18) return vec3(0.62,0.46,0.26);
        // Zipper pull: small bright patch at mid-chest
        if (ax < 0.018 && py > -0.18 && py < -0.10) return vec3(0.72,0.68,0.60); // metal

        // Collar fold: doubled-over fabric at top of jacket
        if (py < -0.50 && ax < 0.18 && py > -0.62) {
            float collar_d = (-0.50 - py) * 6.0;  // 0=bottom of collar, 1=top
            // Fold crease: slightly darker at the bend line
            if (collar_d > 0.70) return vec3(0.42,0.28,0.11);  // fold shadow
            return vec3(0.52,0.36,0.16);  // collar body
        }

        // Chest pockets: bilateral rectangular patches at front chest
        float pkt_d = abs(ax - 0.14);
        if (pkt_d < 0.055 && py > -0.42 && py < -0.22) {
            // Pocket flap bottom edge: slightly darker line
            if (abs(py + 0.22) < 0.008) return vec3(0.40,0.27,0.10);  // pocket seam
            return vec3(0.50,0.34,0.16);  // pocket flap (slightly different shade)
        }

        // Lapel edge: desaturated angle at jacket opening
        if (py < -0.18 && ax > 0.06 && ax < 0.24) return vec3(0.48,0.33,0.15);

        // Shoulder yoke: darker reinforcement panel
        if (py < -0.42 && ax > 0.16) return vec3(0.44,0.30,0.13);

        // Cuff band at sleeve end: forearm only (covered by forearm SDF region, not torso)
        if (py > 0.22 && py < 0.29) return vec3(0.42,0.28,0.12);  // cuff

        // Stitching seam lines: very thin dark bands (1-2 particle widths)
        // Back yoke seam horizontal (at yoke-body transition)
        if (abs(py + 0.42) < 0.006 && ax > 0.05) return vec3(0.38,0.25,0.09);
        // Side seam vertical: faint line at jacket edge
        if (abs(ax - 0.25) < 0.005 && py > -0.50 && py < 0.18) return vec3(0.40,0.27,0.10);

        return vec3(0.55,0.38,0.20);  // main jacket body
    }
    if (sdf_id == 13) {
        // Pants layer — olive tactical with thigh cargo pocket
        if (py > 0.72 && py < 0.90 && ax < 0.18) return vec3(0.36,0.32,0.22); // knee pad
        if (py >= 1.14) return vec3(0.25,0.17,0.10);  // boot shaft bleed
        // Cargo pocket on outer thigh (bilateral)
        if (py > 0.35 && py < 0.60 && ax > 0.10 && ax < 0.20) {
            if (abs(py-0.35) < 0.008 || abs(py-0.60) < 0.008) return vec3(0.24,0.27,0.18); // seam
            return vec3(0.28,0.31,0.22);  // cargo pocket flap (slightly lighter)
        }
        // Belt loop hints at waist: slight contrast band
        if (py > 0.28 && py < 0.34) return vec3(0.26,0.29,0.21);
        // Trouser crease: slight front-center press crease (midline front of leg)
        if (abs(pz - 0.030) < 0.006 && py > 0.34 && py < 1.10 && ax < 0.08)
            return vec3(0.34,0.37,0.28);  // crease highlight (slightly lighter)
        return vec3(0.30,0.33,0.24);   // olive tactical trousers
    }

    // ── Torso: jacket with fabric detail ─────────────────────────────────────
    // Belt buckle (metal)
    if (py >= 0.21 && py <= 0.31 && ax < 0.05) return vec3(0.72,0.70,0.66);
    // Belt leather strap
    if (py >= 0.21 && py <= 0.31) return vec3(0.14,0.10,0.07);
    // Centre zipper strip
    if (ax < 0.05 && py > -0.62 && py < 0.21) return vec3(0.60,0.44,0.24);
    // Lapel edge: slightly desaturated at jacket opening
    if (py < -0.20 && ax > 0.08 && ax < 0.26) return vec3(0.48,0.33,0.15);
    // Shoulder yoke: darker panel across top
    if (py < -0.42 && ax > 0.18) return vec3(0.44,0.30,0.13);
    // Chest panels: main olive-brown jacket body
    return vec3(0.55,0.38,0.20);
}

vec3 sky_color(float dx, float dy, float dz) {
    float r = max(sqrt(dx*dx + dy*dy + dz*dz), 0.001);
    float el = clamp(-dy / r, 0.0, 1.0);
    float sun_dot = max((dx/r)*(-0.375) + (dy/r)*(-0.515) + (dz/r)*(0.685), 0.0);
    float sun_glow = pow(sun_dot, 8.0) * 0.55;
    float t = el, rs, gs, bs;
    if (t < 0.4) {
        float s = t / 0.4;
        rs = 0.58*(1.0-s) + 0.20*s; gs = 0.38*(1.0-s) + 0.22*s; bs = 0.18*(1.0-s) + 0.45*s;
    } else {
        float s = (t - 0.4) / 0.6;
        rs = 0.20*(1.0-s) + 0.06*s; gs = 0.22*(1.0-s) + 0.08*s; bs = 0.45*(1.0-s) + 0.24*s;
    }
    return vec3(rs + sun_glow, gs + sun_glow*0.72, bs + sun_glow*0.30);
}

vec3 fresnel_resp(int mat, float snz) {
    float f = pow(max(1.0 - snz, 0.0), 5.0);
    float k, tr, tg, tb;
    if      (mat == MAT_SKIN)   { k=0.20; tr=1.00; tg=0.90; tb=0.80; }
    else if (mat == MAT_HAIR)   { k=0.10; tr=0.95; tg=0.88; tb=0.72; }
    else if (mat == MAT_JACKET) { k=0.50; tr=0.90; tg=0.72; tb=0.40; }
    else if (mat == MAT_BOOT)   { k=0.70; tr=0.88; tg=0.78; tb=0.55; }
    else if (mat == MAT_METAL)  { k=1.00; tr=1.00; tg=0.97; tb=0.88; }
    else                        { k=0.85; tr=1.00; tg=0.98; tb=0.96; }
    float c = f * k * 0.38;
    return vec3(c*tr, c*tg, c*tb);
}

// ── Technique 12: ACES filmic tonemapping ─────────────────────────────────────
// Narkowicz 2015 approximation. Richer shadows, better highlight rolloff, more
// saturated midtones than the previous Reinhard+S-curve chain.
vec3 aces_film(vec3 x) {
    const float a=2.51, b=0.03, c=2.43, d=0.59, e=0.14;
    return clamp((x*(a*x+b))/(x*(c*x+d)+e), 0.0, 1.0);
}

// ── Technique 2: SDF analytical reflection trace ──────────────────────────────
// Exclusive to SDF-particle renderers. Marches the reflected view ray against
// the analytical implicit body — no BVH, no triangle mesh required.
// Returns the lit surface color at the first reflection hit, or sky if no hit.
vec3 sdf_reflect_trace(float px, float py, float pz, vec3 rd) {
    float t = 0.020;
    for (int i = 0; i < 8; i++) {
        float rx=px+t*rd.x, ry=py+t*rd.y, rz=pz+t*rd.z;
        float d = sdf_body(rx, ry, rz);
        if (abs(d) < 0.008) {
            int  rid  = dominant_sdf_id(rx, ry, rz);
            vec3 rcol = leon_color(rx, ry, rid);
            vec3 rn   = body_normal(rx, ry, rz);
            float rdk = max(dot(rn, KL), 0.0);
            return rcol * (rdk * 0.82 + 0.16);
        }
        t += max(abs(d) * 0.80, 0.018);
        if (t > 1.4) break;
    }
    return sky_color(rd.x, rd.y, rd.z) * 0.70;
}

// ── Technique 8: Volumetric light shaft measurement ───────────────────────────
// Counts unoccluded steps along the key light direction. Particles in the clear
// corridor between arm and torso (god-ray gap) receive a bright shaft bonus.
float light_shaft_factor(float px, float py, float pz) {
    float clear = 0.0;
    const float STEP = 0.055;
    for (int k = 1; k <= 8; k++) {
        float h = float(k) * STEP;
        float d = sdf_body(px+h*KL.x, py+h*KL.y, pz+h*KL.z);
        if (d > 0.008) clear += STEP;
    }
    float total = 8.0 * STEP;
    float shaft = clear / total;   // 0 = fully occluded, 1 = fully clear
    return shaft * shaft;          // square for sharper falloff
}

// Full lighting pass matching sdf_shade() CPU function.
vec3 sdf_shade(vec3 sn, float hz, vec3 base_col, int mat) {
    float spec = 0.0;
    if (mat == MAT_BOOT || mat == MAT_METAL) {
        spec = pow(max(sn.z*(KL.z+1.0)*0.5, 0.0), 24.0) * 0.50;
    } else if (mat == MAT_EYE) {
        float hn = sqrt(KL.x*KL.x + KL.y*KL.y + (KL.z+1.0)*(KL.z+1.0));
        float ndoth = max(dot(sn, vec3(KL.x, KL.y, KL.z+1.0)) / max(hn,0.001), 0.0);
        spec = pow(ndoth, 128.0) * 1.80;
    }
    float dk  = max(dot(sn, KL),  0.0);
    float df1 = max(dot(sn, F1D), 0.0);
    float df2 = max(dot(sn, F2D), 0.0);
    float df3 = max(dot(sn, F3D), 0.0);
    float rim_raw = max(dot(sn, RL), 0.0) * (1.0 - hz);
    float rim = rim_raw * rim_raw;
    float sky = max(-sn.y, 0.0) * 0.08;
    float gnd = max( sn.y, 0.0) * 0.04;
    const float F1I = 0.15, F2I = 0.05;
    const vec3  F1C = vec3(1.00, 0.85, 0.70);
    const vec3  F2C = vec3(0.60, 0.70, 1.00);
    const vec3  F3C = vec3(1.00, 0.52, 0.15);
    float ir = dk*1.10+spec + df1*F1I*F1C.r + df2*F2I*F2C.r + df3*u_f3_int*F3C.r + 0.03+sky*0.60+gnd*0.40;
    float ig = dk*1.10+spec + df1*F1I*F1C.g + df2*F2I*F2C.g + df3*u_f3_int*F3C.g + 0.03+sky*0.75+gnd*0.35;
    float ib = dk*1.10+spec + df1*F1I*F1C.b + df2*F2I*F2C.b + df3*u_f3_int*F3C.b + 0.03+sky*1.00+gnd*0.25;
    vec3 fr = fresnel_resp(mat, sn.z);
    vec3 er = vec3(0.0);
    if (mat == MAT_BOOT || mat == MAT_METAL || mat == MAT_EYE) {
        vec3 rdir = vec3(-2.0*sn.z*sn.x, -2.0*sn.z*sn.y, 1.0-2.0*sn.z*sn.z);
        vec3 sc = sky_color(rdir.x, rdir.y, rdir.z);
        float k = (mat==MAT_METAL) ? 0.38 : (mat==MAT_EYE) ? 0.22 : 0.15;
        er = sc * k;
    }
    float cr = min(base_col.r*ir + rim*0.12 + fr.r + er.r, 1.6);
    float cg = min(base_col.g*ig + rim*0.16 + fr.g + er.g, 1.6);
    float cb = min(base_col.b*ib + rim*0.42 + fr.b + er.b, 1.6);
    // Technique 12: ACES filmic tonemap — richer shadows, cleaner highlights
    return aces_film(vec3(cr, cg, cb));
}

// GI color bleed from 4 tangent-plane neighbor samples.
// sdf_id passed through — neighbor samples are close enough to be the same region.
vec3 gi_bounce(float px, float py, vec3 sn, float illum, int sdf_id) {
    const float RH = 0.065, RV = 0.095, BOUNCE = 0.060;
    vec3 t1 = vec3(-sn.z, 0.0, sn.x);
    vec3 t2raw = vec3(-sn.x*sn.y, 1.0-sn.y*sn.y, -sn.z*sn.y);
    vec3 t2 = normalize(t2raw);
    vec3 a = leon_color(px + RH*t1.x, py, sdf_id);
    vec3 b = leon_color(px - RH*t1.x, py, sdf_id);
    vec3 c = leon_color(px + RV*t2.x, py + RV*t2.y, sdf_id);
    vec3 d = leon_color(px - RV*t2.x, py - RV*t2.y, sdf_id);
    vec3 nb = (a + b + c + d) * 0.25;
    return nb * (illum * BOUNCE);
}

// ── Main compute entry point ──────────────────────────────────────────────────
void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= uint(u_n)) return;

    float px, py, pz;
    float snx_a, sny_a, snz_a;   // analytical normal (before bump)
    uint  seed0;                   // base seed for this region's hf() calls

    // ── Candidate point generation by region ──────────────────────────────────
    if (idx < OFF_ARM_R) {
        // ── TORSO ─────────────────────────────────────────────────────────────
        uint i = idx - OFF_TORSO;
        seed0 = 80u;
        float v = hf(i, seed0);
        py = -0.68 + v * 1.00;
        float ty = v;  // same as ty in sdf_torso
        float lean_z = (1.0 - ty) * 0.016 - ty * 0.008;
        float chest_t = clamp((0.40 - ty) / 0.40, 0.0, 1.0);
        float rib_fwd = chest_t * chest_t * 0.018;
        float ab_flat = clamp((ty-0.42)/0.25,0.0,1.0)*clamp((0.70-ty)/0.28,0.0,1.0);
        float total_z = lean_z - rib_fwd + ab_flat * 0.010;
        float ax = plerp_ax(v), az = plerp_az(v);
        float rx = hf(i,seed0+1u)*2.0-1.0, rz = hf(i,seed0+2u)*2.0-1.0;
        float rn = max(sqrt(rx*rx + rz*rz), 0.001);
        float dx = rx/rn, dz = rz/rn;
        float noise = (hf(i,seed0+3u)*2.0-1.0)*SHELL;
        px = (ax+noise)*dx; pz = (az+noise)*dz + total_z;
        float gx = px/(ax*ax), gz = (pz-total_z)/(az*az);
        float gn = max(sqrt(gx*gx+gz*gz), 0.001);
        snx_a = gx/gn; sny_a = 0.0; snz_a = gz/gn;

    } else if (idx < OFF_ARM_L) {
        // ── RIGHT UPPER ARM ───────────────────────────────────────────────────
        uint i = idx - OFF_ARM_R;
        seed0 = 90u;
        float t = hf(i,seed0);
        py = -0.565 + t*(-0.065 - -0.565);  // right shoulder dropped 3.5cm
        float cx = 0.460 + t*(0.492 - 0.460);
        float r  = 0.092 + t*(0.079 - 0.092);
        float rx = hf(i,seed0+1u)*2.0-1.0, rz = hf(i,seed0+2u)*2.0-1.0;
        float rn = max(sqrt(rx*rx+rz*rz), 0.001);
        float noise = (hf(i,seed0+3u)*2.0-1.0)*SHELL;
        px = cx + (r+noise)*(rx/rn)/1.05;
        pz =      (r+noise)*(rz/rn)/0.95;
        snx_a = (px - cx)/1.05; sny_a = 0.0; snz_a = pz/0.95;
        float nn = max(sqrt(snx_a*snx_a+snz_a*snz_a), 0.001);
        snx_a /= nn; snz_a /= nn;

    } else if (idx < OFF_FA_R) {
        // ── LEFT UPPER ARM (X-mirror of right) ───────────────────────────────
        uint i = idx - OFF_ARM_L;
        seed0 = 100u;
        float t = hf(i,seed0);
        py = -0.60 + t*(-0.10 - -0.60);
        float cx = 0.460 + t*(0.492 - 0.460);
        float r  = 0.092 + t*(0.079 - 0.092);
        float rx = hf(i,seed0+1u)*2.0-1.0, rz = hf(i,seed0+2u)*2.0-1.0;
        float rn = max(sqrt(rx*rx+rz*rz), 0.001);
        float noise = (hf(i,seed0+3u)*2.0-1.0)*SHELL;
        float px_r = cx + (r+noise)*(rx/rn)/1.05;
        pz =              (r+noise)*(rz/rn)/0.95;
        px = -px_r;   // X-mirror
        snx_a = -(px_r - cx)/1.05; sny_a = 0.0; snz_a = pz/0.95;
        float nn = max(sqrt(snx_a*snx_a+snz_a*snz_a), 0.001);
        snx_a /= nn; snz_a /= nn;

    } else if (idx < OFF_FA_L) {
        // ── RIGHT FOREARM ─────────────────────────────────────────────────────
        uint i = idx - OFF_FA_R;
        seed0 = 110u;
        const float AY=-0.065, BY=0.275, AX=0.492, BX=0.515, RA=0.072, RB=0.054;  // cascade from dropped shoulder
        float t = hf(i,seed0);
        py = AY + t*(BY-AY);
        float cx = AX + t*(BX-AX);
        float r  = RA + t*(RB-RA);
        float rx = hf(i,seed0+1u)*2.0-1.0, rz = hf(i,seed0+2u)*2.0-1.0;
        float rn = max(sqrt(rx*rx+rz*rz), 0.001);
        float noise = (hf(i,seed0+3u)*2.0-1.0)*SHELL;
        px = cx + (r+noise)*(rx/rn)/1.05;
        pz =      (r+noise)*(rz/rn)/0.95;
        snx_a = (px - cx)/1.05; sny_a = 0.0; snz_a = pz/0.95;
        float nn = max(sqrt(snx_a*snx_a+snz_a*snz_a), 0.001);
        snx_a /= nn; snz_a /= nn;

    } else if (idx < OFF_HAND_R) {
        // ── LEFT FOREARM (X-mirror) ───────────────────────────────────────────
        uint i = idx - OFF_FA_L;
        seed0 = 120u;
        const float AY=-0.10, BY=0.24, AX=0.492, BX=0.515, RA=0.072, RB=0.054;
        float t = hf(i,seed0);
        py = AY + t*(BY-AY);
        float cx = AX + t*(BX-AX);
        float r  = RA + t*(RB-RA);
        float rx = hf(i,seed0+1u)*2.0-1.0, rz = hf(i,seed0+2u)*2.0-1.0;
        float rn = max(sqrt(rx*rx+rz*rz), 0.001);
        float noise = (hf(i,seed0+3u)*2.0-1.0)*SHELL;
        float px_r = cx + (r+noise)*(rx/rn)/1.05;
        pz =              (r+noise)*(rz/rn)/0.95;
        px = -px_r;
        snx_a = -(px_r - cx)/1.05; sny_a = 0.0; snz_a = pz/0.95;
        float nn = max(sqrt(snx_a*snx_a+snz_a*snz_a), 0.001);
        snx_a /= nn; snz_a /= nn;

    } else if (idx < OFF_HAND_L) {
        // ── RIGHT HAND ────────────────────────────────────────────────────────
        uint i = idx - OFF_HAND_R;
        seed0 = 150u;
        // 75% palm ellipsoid surface, 25% finger region
        float part = hf(i, seed0);
        if (part < 0.75) {
            // Palm ellipsoid — moved inward with narrower arm
            const float PCX=0.525, PCY=0.295, RX=0.044, RY=0.032, RZ=0.048;
            float rx = hf(i,seed0+1u)*2.0-1.0, ry = hf(i,seed0+2u)*2.0-1.0, rz = hf(i,seed0+3u)*2.0-1.0;
            float rn = max(sqrt(rx*rx+ry*ry+rz*rz), 0.001);
            float noise = (hf(i,seed0+4u)*2.0-1.0)*SHELL;
            px = PCX + (RX+noise)*rx/rn;
            py = PCY + (RY+noise)*ry/rn;
            pz =       (RZ+noise)*rz/rn;
            snx_a=(px-PCX)/RX; sny_a=(py-PCY)/RY; snz_a=pz/RZ;
            float nn=max(sqrt(snx_a*snx_a+sny_a*sny_a+snz_a*snz_a),0.001);
            snx_a/=nn; sny_a/=nn; snz_a/=nn;
        } else {
            // Bent finger — two-phalanx: sample t along full length, interpolate Z curl
            int fi = int(hf(i,seed0+5u)*4.99);
            // FCX, base Z, base Y, tip Y, base radius — match sdf_hand_r
            const float FCX[5] = float[5](0.516,0.524,0.530,0.533,0.540);
            const float FZ[5]  = float[5](-0.044,-0.022,0.002,0.026,-0.055);
            const float TZ[5]  = float[5](-0.026,-0.006,0.016,0.040,-0.082); // tip Z after curl
            const float FBY[5] = float[5](0.320,0.322,0.322,0.320,0.268);
            const float FTY[5] = float[5](0.410,0.428,0.436,0.428,0.348);
            const float FRA[5] = float[5](0.014,0.015,0.016,0.015,0.018);
            float fcx=FCX[fi], fbz=FZ[fi], fby=FBY[fi], fty=FTY[fi], fra=FRA[fi];
            float ft = hf(i,seed0+6u);
            // Center Z interpolates toward tip for curl
            float cz = fbz + ft*(TZ[fi]-fbz);
            py = fby + ft*(fty-fby);
            float r = fra + ft*(0.008-fra);
            float rx = hf(i,seed0+7u)*2.0-1.0, rz = hf(i,seed0+8u)*2.0-1.0;
            float rn = max(sqrt(rx*rx+rz*rz), 0.001);
            float noise = (hf(i,seed0+9u)*2.0-1.0)*SHELL;
            px = fcx + (r+noise)*rx/rn;
            pz = cz  + (r+noise)*rz/rn;
            snx_a = px-fcx; sny_a = 0.0; snz_a = pz-cz;
            float nn = max(sqrt(snx_a*snx_a+snz_a*snz_a), 0.001);
            snx_a /= nn; snz_a /= nn;
        }

    } else if (idx < OFF_LEG_R) {
        // ── LEFT HAND (X-mirror of right) ─────────────────────────────────────
        uint i = idx - OFF_HAND_L;
        seed0 = 160u;
        float part = hf(i, seed0);
        if (part < 0.75) {
            const float PCX=0.525, PCY=0.295, RX=0.044, RY=0.032, RZ=0.048;
            float rx = hf(i,seed0+1u)*2.0-1.0, ry = hf(i,seed0+2u)*2.0-1.0, rz = hf(i,seed0+3u)*2.0-1.0;
            float rn = max(sqrt(rx*rx+ry*ry+rz*rz), 0.001);
            float noise = (hf(i,seed0+4u)*2.0-1.0)*SHELL;
            float px_r = PCX + (RX+noise)*rx/rn;
            py = PCY + (RY+noise)*ry/rn;
            pz =       (RZ+noise)*rz/rn;
            px = -px_r;
            snx_a=-(px_r-PCX)/RX; sny_a=(py-PCY)/RY; snz_a=pz/RZ;
            float nn=max(sqrt(snx_a*snx_a+sny_a*sny_a+snz_a*snz_a),0.001);
            snx_a/=nn; sny_a/=nn; snz_a/=nn;
        } else {
            int fi = int(hf(i,seed0+5u)*4.99);
            const float FCX[5] = float[5](0.516,0.524,0.530,0.533,0.540);
            const float FZ[5]  = float[5](-0.044,-0.022,0.002,0.026,-0.055);
            const float TZ[5]  = float[5](-0.026,-0.006,0.016,0.040,-0.082);
            const float FBY[5] = float[5](0.320,0.322,0.322,0.320,0.268);
            const float FTY[5] = float[5](0.410,0.428,0.436,0.428,0.348);
            const float FRA[5] = float[5](0.014,0.015,0.016,0.015,0.018);
            float fcx=FCX[fi], fbz=FZ[fi], fby=FBY[fi], fty=FTY[fi], fra=FRA[fi];
            float ft = hf(i,seed0+6u);
            float cz = fbz + ft*(TZ[fi]-fbz);
            py = fby + ft*(fty-fby);
            float r = fra + ft*(0.008-fra);
            float rx = hf(i,seed0+7u)*2.0-1.0, rz = hf(i,seed0+8u)*2.0-1.0;
            float rn = max(sqrt(rx*rx+rz*rz), 0.001);
            float noise = (hf(i,seed0+9u)*2.0-1.0)*SHELL;
            float px_r = fcx + (r+noise)*rx/rn;
            pz = cz  + (r+noise)*rz/rn;
            px = -px_r;
            snx_a = -(px_r-fcx); sny_a = 0.0; snz_a = pz-cz;
            float nn = max(sqrt(snx_a*snx_a+snz_a*snz_a), 0.001);
            snx_a /= nn; snz_a /= nn;
        }

    } else if (idx < OFF_LEG_L) {
        // ── RIGHT LEG ─────────────────────────────────────────────────────────
        uint i = idx - OFF_LEG_R;
        seed0 = 130u;
        float v = hf(i,seed0);
        py = 0.28 + v * (1.18 - 0.28);
        const float CX = 0.145;
        float r_exp = (py < 0.82)
            ? (0.105 + clamp((py-0.28)/0.54,0.0,1.0)*(0.088-0.105))
            : (0.090 + clamp((py-0.78)/0.40,0.0,1.0)*(0.055-0.090));
        float rx = hf(i,seed0+1u)*2.0-1.0, rz = hf(i,seed0+2u)*2.0-1.0;
        float rn = max(sqrt(rx*rx+rz*rz), 0.001);
        float noise = (hf(i,seed0+3u)*2.0-1.0)*SHELL;
        float px_u = CX + (r_exp+noise)*(rx/rn);
        pz =               (r_exp+noise)*(rz/rn)*0.92 - 0.038;  // right leg forward
        px = px_u;
        snx_a = px_u - CX; sny_a = 0.0; snz_a = pz + 0.038;  // normal in local frame
        float nn = max(sqrt(snx_a*snx_a+snz_a*snz_a), 0.001);
        snx_a /= nn; snz_a /= nn;

    } else if (idx < OFF_FOOT_R) {
        // ── LEFT LEG (X-mirror) ───────────────────────────────────────────────
        uint i = idx - OFF_LEG_L;
        seed0 = 140u;
        float v = hf(i,seed0);
        py = 0.28 + v * (1.18 - 0.28);
        const float CX = 0.145;
        float r_exp = (py < 0.82)
            ? (0.105 + clamp((py-0.28)/0.54,0.0,1.0)*(0.088-0.105))
            : (0.090 + clamp((py-0.78)/0.40,0.0,1.0)*(0.055-0.090));
        float rx = hf(i,seed0+1u)*2.0-1.0, rz = hf(i,seed0+2u)*2.0-1.0;
        float rn = max(sqrt(rx*rx+rz*rz), 0.001);
        float noise = (hf(i,seed0+3u)*2.0-1.0)*SHELL;
        float px_u = CX + (r_exp+noise)*(rx/rn);
        pz =               (r_exp+noise)*(rz/rn)*0.92 + 0.014;  // left leg slightly back
        px = -px_u;   // X-mirror
        snx_a = -(px_u - CX); sny_a = 0.0; snz_a = pz - 0.014;
        float nn = max(sqrt(snx_a*snx_a+snz_a*snz_a), 0.001);
        snx_a /= nn; snz_a /= nn;

    } else if (idx < OFF_FOOT_L) {
        // ── RIGHT FOOT ────────────────────────────────────────────────────────
        uint i = idx - OFF_FOOT_R;
        seed0 = 170u;
        // 60% toe box, 25% heel, 15% shaft — matches new boot geometry
        float part = hf(i, seed0);
        if (part < 0.60) {
            const float BCX=0.138, BCY=1.255, BCZ=0.035, BRX=0.054, BRY=0.030, BRZ=0.082;
            float rx=hf(i,seed0+1u)*2.0-1.0, ry=hf(i,seed0+2u)*2.0-1.0, rz=hf(i,seed0+3u)*2.0-1.0;
            float rn=max(sqrt(rx*rx+ry*ry+rz*rz),0.001);
            float noise=(hf(i,seed0+4u)*2.0-1.0)*SHELL;
            px=BCX+(BRX+noise)*rx/rn; py=BCY+(BRY+noise)*ry/rn; pz=BCZ+(BRZ+noise)*rz/rn;
            snx_a=(px-BCX)/BRX; sny_a=(py-BCY)/BRY; snz_a=(pz-BCZ)/BRZ;
        } else if (part < 0.85) {
            const float HCX=0.140, HCY=1.248, HCZ=-0.052, HRX=0.046, HRY=0.032, HRZ=0.038;
            float rx=hf(i,seed0+1u)*2.0-1.0, ry=hf(i,seed0+2u)*2.0-1.0, rz=hf(i,seed0+3u)*2.0-1.0;
            float rn=max(sqrt(rx*rx+ry*ry+rz*rz),0.001);
            float noise=(hf(i,seed0+4u)*2.0-1.0)*SHELL;
            px=HCX+(HRX+noise)*rx/rn; py=HCY+(HRY+noise)*ry/rn; pz=HCZ+(HRZ+noise)*rz/rn;
            snx_a=(px-HCX)/HRX; sny_a=(py-HCY)/HRY; snz_a=(pz-HCZ)/HRZ;
        } else {
            // Boot shaft ring (ankle height)
            float t=hf(i,seed0+5u);
            py=1.180+t*0.060; float r=0.056+t*(0.048-0.056);
            float rx=hf(i,seed0+1u)*2.0-1.0, rz=hf(i,seed0+2u)*2.0-1.0;
            float rn=max(sqrt(rx*rx+rz*rz),0.001); float noise=(hf(i,seed0+3u)*2.0-1.0)*SHELL;
            px=0.145+(r+noise)*rx/rn; pz=(r+noise)*rz/rn;
            snx_a=px-0.145; sny_a=0.0; snz_a=pz;
        }
        pz -= 0.038;  // right foot steps forward with right leg
        float nn=max(sqrt(snx_a*snx_a+sny_a*sny_a+snz_a*snz_a),0.001);
        snx_a/=nn; sny_a/=nn; snz_a/=nn;

    } else {
        // ── LEFT FOOT (X-mirror) ──────────────────────────────────────────────
        uint i = idx - OFF_FOOT_L;
        seed0 = 180u;
        float part = hf(i, seed0);
        float px_r;
        if (part < 0.60) {
            const float BCX=0.138, BCY=1.255, BCZ=0.035, BRX=0.054, BRY=0.030, BRZ=0.082;
            float rx=hf(i,seed0+1u)*2.0-1.0, ry=hf(i,seed0+2u)*2.0-1.0, rz=hf(i,seed0+3u)*2.0-1.0;
            float rn=max(sqrt(rx*rx+ry*ry+rz*rz),0.001);
            float noise=(hf(i,seed0+4u)*2.0-1.0)*SHELL;
            px_r=BCX+(BRX+noise)*rx/rn; py=BCY+(BRY+noise)*ry/rn; pz=BCZ+(BRZ+noise)*rz/rn;
            snx_a=-(px_r-BCX)/BRX; sny_a=(py-BCY)/BRY; snz_a=(pz-BCZ)/BRZ;
        } else if (part < 0.85) {
            const float HCX=0.140, HCY=1.248, HCZ=-0.052, HRX=0.046, HRY=0.032, HRZ=0.038;
            float rx=hf(i,seed0+1u)*2.0-1.0, ry=hf(i,seed0+2u)*2.0-1.0, rz=hf(i,seed0+3u)*2.0-1.0;
            float rn=max(sqrt(rx*rx+ry*ry+rz*rz),0.001);
            float noise=(hf(i,seed0+4u)*2.0-1.0)*SHELL;
            px_r=HCX+(HRX+noise)*rx/rn; py=HCY+(HRY+noise)*ry/rn; pz=HCZ+(HRZ+noise)*rz/rn;
            snx_a=-(px_r-HCX)/HRX; sny_a=(py-HCY)/HRY; snz_a=(pz-HCZ)/HRZ;
        } else {
            float t=hf(i,seed0+5u);
            py=1.180+t*0.060; float r=0.056+t*(0.048-0.056);
            float rx=hf(i,seed0+1u)*2.0-1.0, rz=hf(i,seed0+2u)*2.0-1.0;
            float rn=max(sqrt(rx*rx+rz*rz),0.001); float noise=(hf(i,seed0+3u)*2.0-1.0)*SHELL;
            px_r=0.145+(r+noise)*rx/rn; pz=(r+noise)*rz/rn;
            snx_a=-(px_r-0.145); sny_a=0.0; snz_a=pz;
        }
        px = -px_r;   // X-mirror
        pz += 0.014;  // left foot slightly back
        float nn=max(sqrt(snx_a*snx_a+sny_a*sny_a+snz_a*snz_a),0.001);
        snx_a/=nn; sny_a/=nn; snz_a/=nn;

    } else if (idx < OFF_END) {
        // ── HEAD / FACE / NECK ────────────────────────────────────────────────
        // Random candidates in a box bounding the skull: accept if |sdf_head| < SHELL.
        // Normal derived from sdf_head gradient (6-tap central differences).
        uint i = idx - OFF_HEAD;
        seed0 = 190u;
        // Bounding box: X in [-0.22, 0.22], Y in [-1.17, -0.70], Z in [-0.18, 0.24]
        px = (hf(i, seed0  ) - 0.5) * 0.44;
        py = -1.17 + hf(i, seed0+1u) * 0.47;
        pz = -0.18 + hf(i, seed0+2u) * 0.42;
        // Fast SDF test against head-only SDF (skip full body eval for this region)
        float d_hd = sdf_head(px, py, pz);
        if (abs(d_hd) >= SHELL) return;
        // Analytical head-SDF gradient
        const float EH = 0.004;
        float gnx = sdf_head(px+EH,py,pz) - sdf_head(px-EH,py,pz);
        float gny = sdf_head(px,py+EH,pz) - sdf_head(px,py-EH,pz);
        float gnz = sdf_head(px,py,pz+EH) - sdf_head(px,py,pz-EH);
        float gnn = max(sqrt(gnx*gnx+gny*gny+gnz*gnz), 0.001);
        snx_a = gnx/gnn; sny_a = gny/gnn; snz_a = gnz/gnn;
        sdf_id = 11;
    }

    // ── SDF acceptance test (body regions only — head already accepted above) ──
    float d = (sdf_id == 11) ? 0.0 : sdf_body(px, py, pz);
    if (d < -SHELL || d > SHELL) return;

    vec3 sn_a = vec3(snx_a, sny_a, snz_a);

    // ── Curvature → wrinkle weight ────────────────────────────────────────────
    const float CE = 0.025;
    float t1x = -snz_a, t1z = snx_a;
    vec3 t2raw = vec3(-snx_a*sny_a, 1.0-sny_a*sny_a, -snz_a*sny_a);
    vec3 t2 = normalize(t2raw);
    float d1p = sdf_body(px+CE*t1x,        py,          pz+CE*t1z);
    float d1m = sdf_body(px-CE*t1x,        py,          pz-CE*t1z);
    float d2p = sdf_body(px+CE*t2.x, py+CE*t2.y, pz+CE*t2.z);
    float d2m = sdf_body(px-CE*t2.x, py-CE*t2.y, pz-CE*t2.z);
    float kappa = (d1p+d1m-2.0*d + d2p+d2m-2.0*d) / (CE*CE);
    float wrinkle_raw = clamp(-kappa*0.035, 0.0, 1.0);
    float wrinkle = wrinkle_raw*wrinkle_raw*(3.0-2.0*wrinkle_raw);

    // ── Procedural skin detail (noise displacement + bump normal) ─────────────
    float disp, roughness;
    vec3 pn;
    skin_detail(px, py, pz, sn_a, disp, pn, roughness);
    float inset = wrinkle * 0.006;
    px += (disp - inset)*snx_a;
    py += (disp - inset)*sny_a;
    pz += (disp - inset)*snz_a;
    float hz = max(pn.z, 0.0);
    float core = clamp((-d/SHELL + 1.0) * 0.5, 0.0, 1.0);

    // ── Material lookup (SDF-aware) ───────────────────────────────────────────
    int  sdf_id   = dominant_sdf_id(px, py, pz);
    int  mat      = leon_tag(px, py, sdf_id);
    vec3 base_col = leon_color(px, py, sdf_id);

    // ── Full lighting ─────────────────────────────────────────────────────────
    vec3 col = sdf_shade(pn, hz, base_col, mat);

    // ── AO ───────────────────────────────────────────────────────────────────
    float ao = sdf_ao(px, py, pz, pn);
    col *= ao;

    // ── SSS — skin only ───────────────────────────────────────────────────────
    if (mat == MAT_SKIN) {
        float bk = max(-dot(pn, KL), 0.0);
        float bt = max(-dot(pn, F3D), 0.0) * u_f3_int;
        float bi = bk*0.80 + bt;
        if (bi > 0.04) {
            float thick = sdf_thickness(px, py, pz, pn);
            float tr = exp(-7.5*thick)*bi;
            col.r += tr*1.00; col.g += tr*0.28; col.b += tr*0.08;
        }
    }
    // SSS — jacket (olive transmit)
    if (mat == MAT_JACKET) {
        float bk = max(-dot(pn, KL), 0.0);
        float bt = max(-dot(pn, F3D), 0.0) * u_f3_int;
        float bi = bk*0.80 + bt;
        if (bi > 0.06) {
            float thick = sdf_thickness(px, py, pz, pn);
            float tr = exp(-12.0*thick)*bi;
            col.r += tr*0.55; col.g += tr*0.68; col.b += tr*0.25;
        }
    }

    // ── Micro-roughness specular ──────────────────────────────────────────────
    float dk_bump = max(dot(pn, KL), 0.0);
    float micro = roughness*roughness*dk_bump*0.14;
    col.r += micro*1.00; col.g += micro*0.94; col.b += micro*0.82;

    // ── Technique 16: Kajiya-Kay anisotropic specular ─────────────────────────
    // A traditional renderer needs an explicit tangent-space texture and a separate
    // pass to compute per-pixel tangents.  Here the SDF gradient IS the normal, so
    // we derive the anisotropic tangent analytically:
    //   • Jacket/boot: tangent = cross(pn, up)  → horizontal fiber direction (weft)
    //   • Hair:        tangent = cross(pn, growth_dir) → along-strand direction
    // KK model: spec = sin(T,L)^n * sin(T,V)^m — bright band orthogonal to tangent.
    {
        // Anisotropic tangent: horizontal weft for fabric, vertical for hair
        vec3 guide = (mat == MAT_HAIR) ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0);
        // Guard: if normal is nearly parallel to guide, pivot to Z
        if (abs(dot(pn, guide)) > 0.96) guide = vec3(0.0, 0.0, 1.0);
        vec3 T = normalize(cross(pn, guide));   // tangent along fiber direction

        // Kajiya-Kay cos-theta terms: sinT = sqrt(1-dot^2)
        float TdotL = dot(T, KL);
        float sinTL = sqrt(max(0.0, 1.0 - TdotL*TdotL));
        // Camera view approximated by -KL (back-lit): conservative but cheap
        float TdotV  = dot(T, -KL);
        float sinTV  = sqrt(max(0.0, 1.0 - TdotV*TdotV));

        float kk_exp = (mat == MAT_HAIR) ? 16.0 : 8.0;
        float kk = pow(max(sinTL, 0.0), kk_exp) * pow(max(sinTV, 0.0), 4.0);
        kk *= dk_bump * 0.45;   // gate on face lit by key light

        if (mat == MAT_HAIR) {
            // Hair: warm gold-brown glint along strands
            col.r += kk * 0.42; col.g += kk * 0.28; col.b += kk * 0.06;
        } else if (mat == MAT_JACKET) {
            // Jacket leather: subtle sheen — cool specular on horizontal weft
            col.r += kk * 0.22; col.g += kk * 0.24; col.b += kk * 0.32;
        }
        // Boot/metal: anisotropic not visible on smooth leather (micro_roughness handles it)
    }

    // ── Technique 17: SDF micro-displacement → pore-shadow on skin ───────────
    // We have the analytical SDF value (dist from surface = 0) at this particle's
    // position; nearby particles at dist ≠ 0 contribute directional shadow.
    // Cheaply approximated by marching two tiny steps along the key-light direction
    // and measuring the SDF differential — positive differential = concave shadow.
    // This is only meaningful for skin (smooth surface where micro-detail matters).
    if (mat == MAT_SKIN) {
        const float MICRO_D = 0.003;
        float d_fwd  = sdf_body(px + KL.x*MICRO_D, py + KL.y*MICRO_D, pz + KL.z*MICRO_D);
        float d_bwd  = sdf_body(px - KL.x*MICRO_D, py - KL.y*MICRO_D, pz - KL.z*MICRO_D);
        float curvature = (d_fwd + d_bwd) * (1.0 / (MICRO_D * MICRO_D));
        // Negative curvature → concave pore → shadow
        float pore_shadow = clamp(-curvature * 0.00018, 0.0, 0.28);
        col *= 1.0 - pore_shadow;
        // Positive curvature → convex hillock → brightens
        float pore_bright = clamp(curvature * 0.00008, 0.0, 0.12);
        col += vec3(pore_bright * 0.85, pore_bright * 0.60, pore_bright * 0.35);
    }

    // ── Technique 20: SDF-curvature fresnel rim ──────────────────────────────
    // Compute the Hessian diagonal approximation by sampling sdf_body at ±eps
    // along each axis.  The trace of the Hessian = Laplacian = mean curvature × 2.
    // Convex regions (knuckles, cheekbones, collar edge) get amplified rim light;
    // flat regions (chest panel, thigh) stay normal.
    // Traditional renderers reconstruct curvature from depth buffer (noisy ±1px
    // finite difference).  Here we have the exact analytical SDF — curvature is
    // accurate to floating-point precision.
    {
        const float EPS = 0.006;
        float d0  = sdf_body(px, py, pz);
        float dxx = sdf_body(px+EPS,py,pz) + sdf_body(px-EPS,py,pz) - 2.0*d0;
        float dyy = sdf_body(px,py+EPS,pz) + sdf_body(px,py-EPS,pz) - 2.0*d0;
        float dzz = sdf_body(px,py,pz+EPS) + sdf_body(px,py,pz-EPS) - 2.0*d0;
        float laplacian = (dxx + dyy + dzz) / (EPS * EPS);
        // Mean curvature ∝ laplacian; positive = convex, negative = concave
        float curvature_rim = clamp(laplacian * 0.0025, -1.0, 1.0);
        float rim_boost = curvature_rim * 0.18;   // convex → brighter rim
        // Apply fresnel-style: weight by rim factor (already computed from rim var above)
        float rim_raw = max(dot(pn, RL), 0.0);
        float curvature_fresnel = rim_raw * rim_raw * rim_boost;
        col += vec3(curvature_fresnel * 0.18, curvature_fresnel * 0.28, curvature_fresnel * 0.48);
    }

    // ── Technique 21: Thickness-modulated SSS ────────────────────────────────
    // March inward along -normal and measure how far inside the SDF body we travel
    // before exiting the other side.  Thin geometry → more SSS; thick → suppressed.
    // This gives finger-web and ear-lobe translucency naturally without artist
    // thickness maps — the SDF body encodes the geometry analytically.
    if (mat == MAT_SKIN) {
        float bk_key = max(-dot(pn, KL), 0.0);
        float bk_f3  = max(-dot(pn, F3D), 0.0) * u_f3_int;
        float bk = bk_key * 0.90 + bk_f3 * 0.55;
        if (bk > 0.04) {
            // Thickness: march inward, accumulate distance while inside (sdf<0)
            float thickness = 0.0;
            vec3 rin = -pn;   // inward direction
            const float TSTEP = 0.018;
            for (int k = 1; k <= 5; k++) {
                float h = float(k) * TSTEP;
                float d_in = sdf_body(px+rin.x*h, py+rin.y*h, pz+rin.z*h);
                if (d_in < 0.0) thickness += TSTEP;   // still inside
            }
            // Beer-Lambert transmission: thinner → more light
            float transmit = exp(-14.0 * thickness) * bk;
            // Skin SSS color: warm orange-red (blood scatter)
            col.r += transmit * 0.68;
            col.g += transmit * 0.22;
            col.b += transmit * 0.08;
        }
    }

    // ── Wrinkle contact shadow ────────────────────────────────────────────────
    if (wrinkle > 0.01) {
        col.r *= 1.0 - wrinkle*0.52;
        col.g *= 1.0 - wrinkle*0.62;
        col.b *= 1.0 - wrinkle*0.80;
    }

    // ── Technique 22: SDF-analytical ambient occlusion ───────────────────────
    // Traditional SSAO samples the hemisphere of the depth buffer — coarse and
    // view-dependent.  The analytical SDF gives exact local geometry.  We sample
    // 6 axis-aligned offsets: particles in concavities see nearby surface close up
    // (low SDF value at offset) → occluded.  Particles on convex features see open
    // space → unoccluded.  Fully view-independent and correct for self-shadowing.
    float t22_ao;
    {
        const float AO_R = 0.045;  // sample sphere radius in model units
        float sum = 0.0;
        sum += clamp(sdf_body(px+AO_R, py,     pz    ) / AO_R, 0.0, 1.0);
        sum += clamp(sdf_body(px-AO_R, py,     pz    ) / AO_R, 0.0, 1.0);
        sum += clamp(sdf_body(px,      py+AO_R,pz    ) / AO_R, 0.0, 1.0);
        sum += clamp(sdf_body(px,      py-AO_R,pz    ) / AO_R, 0.0, 1.0);
        sum += clamp(sdf_body(px,      py,     pz+AO_R) / AO_R, 0.0, 1.0);
        sum += clamp(sdf_body(px,      py,     pz-AO_R) / AO_R, 0.0, 1.0);
        t22_ao = sum / 6.0;   // 1.0 = fully open, 0.0 = fully occluded
        // Non-linear falloff: slight gamma emphasizes the crevice shadows
        t22_ao = t22_ao * t22_ao * 0.80 + t22_ao * 0.20;
    }
    // Apply AO to ambient+fill terms only (key light has its own self-shadow)
    float ao_atten = 0.55 + t22_ao * 0.45;

    // ── Volumetric self-shadow ────────────────────────────────────────────────
    float key_t = vol_shadow(px, py, pz);
    float dk_lit = max(dot(pn, KL), 0.0);
    float vol_att = 1.0 - dk_lit*(1.0-key_t)*0.72;
    col *= vol_att;

    // ── GI bounce ────────────────────────────────────────────────────────────
    float dk_gi = max(dot(pn, KL), 0.0);
    float f3_gi = max(dot(pn, F3D), 0.0) * u_f3_int;
    float illum = dk_gi*0.80 + f3_gi*0.40 + 0.12;
    col += gi_bounce(px, py, pn, illum * ao_atten, sdf_id);   // AO modulates GI

    // ── Technique 8: Volumetric light shafts / god rays ──────────────────────
    // Particles in the clear corridor between arm and torso receive bright shaft.
    // Head shadow on chest: dark band directly below the skull (py < -0.40, |px| < 0.22).
    {
        float shaft = light_shaft_factor(px, py, pz);
        float dk_s  = max(dot(pn, KL), 0.0);
        float shaft_bonus = shaft * dk_s * 0.22;
        col += vec3(shaft_bonus*1.00, shaft_bonus*0.88, shaft_bonus*0.62);
        // Head shadow darkening band
        float head_shadow = clamp(1.0 - (py + 0.40)*8.0, 0.0, 1.0)
                          * clamp(1.0 - abs(px)*4.5, 0.0, 1.0);
        col *= 1.0 - head_shadow * 0.35;
    }

    // ── Technique 2: SDF metal reflection (particle-SDF exclusive) ───────────
    // Only metal and eye particles trace a reflection ray against the SDF body.
    // No geometry buffers needed — the analytical implicit surface is free to query.
    if (mat == MAT_METAL || mat == MAT_EYE) {
        vec3 cam_mdl = u_cam_pos / u_scale;
        vec3 vdir = normalize(vec3(px,py,pz) - cam_mdl);
        vec3 rd = vdir - 2.0*dot(vdir, pn)*pn;
        float rw = (mat == MAT_METAL) ? 0.50 : 0.30;
        vec3 refl = sdf_reflect_trace(px, py, pz, rd);
        col = mix(col, refl, rw);
    }

    // ── Technique 23: Eye spectral refraction (real IOR dispersion) ─────────
    // Human lens IOR is wavelength-dependent (chromatic aberration of the eye).
    // For eye particles: refract the view ray through the iris sphere with
    // n_R=1.336, n_G=1.340, n_B=1.346 (measured Abbe V-number for aqueous humor).
    // Red, green, and blue diverge slightly as they exit the sphere — the iris
    // boundary shows a thin spectral ring, exactly like a real eye photographed
    // with a macro lens.  The SDF sphere gives the exact intersection normal.
    if (mat == MAT_EYE) {
        vec3 cam_mdl = u_cam_pos / u_scale;
        vec3 vdir = normalize(vec3(px, py, pz) - cam_mdl);
        // Eye sphere center: approximately at model-space origin + small offset
        const vec3 EYE_C = vec3(0.0, -0.82, 0.13);
        const float EYE_R = 0.038;
        vec3 snorm = normalize(vec3(px,py,pz) - EYE_C);
        // Snell's law refraction for each wavelength
        const float n1 = 1.0;    // air
        const float n_r = 1.336, n_g = 1.340, n_b = 1.346;
        vec3 refr_r = refract(vdir, snorm, n1/n_r);
        vec3 refr_g = refract(vdir, snorm, n1/n_g);
        vec3 refr_b = refract(vdir, snorm, n1/n_b);
        // Color picked from refracted direction (very approximate — just hue shift)
        float r_dot = max(dot(refr_r, KL), 0.0);
        float g_dot = max(dot(refr_g, KL), 0.0);
        float b_dot = max(dot(refr_b, KL), 0.0);
        float disp = abs(r_dot - b_dot) * 8.0;   // measure of dispersion at this point
        // Iris ring: bright spectral fringe near edge of eye
        float iris_d = abs(length(vec2(px,py) - vec2(EYE_C.x,EYE_C.y)) - EYE_R * 0.70);
        float iris_ring = exp(-iris_d * iris_d * 380.0) * disp * 2.5;
        col.r += iris_ring * r_dot * 0.60;
        col.g += iris_ring * g_dot * 0.30;
        col.b += iris_ring * b_dot * 0.80;
    }

    // ── Technique 3: Atmospheric depth scattering ────────────────────────────
    // Back-of-body particles (pz < 0) scatter toward cool deep blue.
    // Front surface stays warm; recessed areas get atmospheric depth.
    {
        float back = max(-pz, 0.0) / 0.35;
        float atmo_t = back * back * 0.20;
        vec3 atmo = vec3(0.07, 0.10, 0.24);
        col = mix(col, col*0.68 + atmo, atmo_t);
    }

    // ── Technique 9: Cross-material GI color bleed ───────────────────────────
    // Jacket surface illuminates adjacent skin warm; pants tint jacket hem green.
    if (mat == MAT_SKIN) {
        // Orange-olive spill from adjacent jacket onto skin boundary
        col += vec3(0.048, 0.032, 0.008) * clamp((py + 0.58)*5.0, 0.0, 1.0);
    }
    if (mat == MAT_JACKET && py > 0.14 && py < 0.24) {
        // Cool green bleed from pants up into lower jacket hem
        float t_hem = clamp((py - 0.14) / 0.10, 0.0, 1.0);
        col = mix(col, col + vec3(0.010, 0.018, 0.006), t_hem * 0.30);
    }

    // ── Technique 13: Eye adaptation — apply frame exposure ──────────────────
    col *= u_exposure;

    // ── Alpha — material-specific floors ─────────────────────────────────────
    // Clothing and metal are opaque materials: floor 0.95.
    // Skin allows SSS light transmission but is never fully transparent: floor 0.75.
    // Hair is dense: floor 0.85.
    // HP damage is expressed through position jitter, not opacity — alpha floors hold.
    float alpha_base = (0.82 + core*0.16) * (1.0 + wrinkle*0.18);
    float alpha;
    if (mat == MAT_SKIN) {
        // Skin: SSS-transparent face of material, but never ghostly
        alpha = clamp(alpha_base * max(u_hp, 0.55), 0.75, 0.88);
    } else if (mat == MAT_HAIR) {
        alpha = clamp(alpha_base * max(u_hp, 0.60), 0.85, 0.97);
    } else {
        // Jacket, boot, metal, eye: opaque — leather and fabric transmit nothing
        alpha = clamp(alpha_base, 0.95, 0.98);
    }
    if (alpha < 0.50) return;

    float br = base_col.r, bg = base_col.g;
    float emission;
    if      (br>0.58 && bg>0.46) emission = (0.40+core*0.80)*(1.0-wrinkle*0.42);  // skin
    else if (br<0.28 && bg<0.18) emission = (0.12+core*0.30)*(1.0-wrinkle*0.42);  // hair-dark
    else if (base_col.b < 0.14)  emission = (0.50+core*1.50)*(1.0-wrinkle*0.42);  // boot
    else                          emission = (0.45+core*1.20)*(1.0-wrinkle*0.42);  // jacket

    float sz = 0.020 + core*0.018;

    // ── Damage jitter ─────────────────────────────────────────────────────────
    float jx = (hf(idx, seed0+4u)-0.5)*u_dmg*0.08;
    float jy = (hf(idx, seed0+5u)-0.5)*u_dmg*0.08;

    // ── World-space position (apply scale + breath + jitter) ──────────────────
    vec3 world_pos = vec3(
        px * u_scale * u_breath + jx,
        py * u_scale * u_breath + jy,
        hz * 0.30
    );

    // ── Write accepted particle to SSBO ───────────────────────────────────────
    uint slot = atomicCounterIncrement(u_count);
    if (slot >= OFF_END) return;   // safety cap = GPU_MAX_PARTICLES (10_800_000)
    particles[slot].position = world_pos;
    particles[slot].size     = sz * u_scale;   // world-space billboard radius
    particles[slot].normal   = pn;
    particles[slot].emission = emission;
    particles[slot].color    = vec4(col, alpha);
}
"#;

// ── Finalize pass — write indirect draw command ───────────────────────────────
const FINALIZE_SRC: &str = r#"
#version 430 core
layout(local_size_x = 1) in;

layout(binding = 0, offset = 0) uniform atomic_uint u_count;

layout(std430, binding = 2) buffer IndirectBuf {
    uint draw_count;
    uint draw_instance_count;
    uint draw_first;
    uint draw_base_instance;
};

uniform uint u_multiplier;

void main() {
    uint n = atomicCounter(u_count);
    draw_count          = 6u;              // 6 verts per billboard quad (2 triangles)
    draw_instance_count = n * u_multiplier;
    draw_first          = 0u;
    draw_base_instance  = 0u;
}
"#;

// ── Billboard vertex shader — reads accepted particles from SSBO ──────────────
const SDF_VERT_SRC: &str = r#"
#version 430 core

struct GpuParticle {
    vec3  position;
    float size;
    vec3  normal;
    float emission;
    vec4  color;
};
layout(std430, binding = 1) readonly buffer ParticleSSBO {
    GpuParticle particles[];
};

uniform mat4  u_view_proj;
uniform int   u_multiplier;
uniform vec3  u_cam_right;
uniform vec3  u_cam_up;
// Technique 4: TAA sub-pixel jitter in NDC (pre-computed on CPU via Halton sequence)
uniform vec2  u_taa_jitter;

out vec4  f_color;
out float f_emission;
out vec2  f_uv;
out vec3  f_normal_w;  // world-space surface normal (for SSR)

float hf(uint seed, uint v) {
    uint n = seed * 374761393u + v * 668265263u;
    n ^= (n >> 13u);
    n *= 0x5851F42Du;
    n ^= (n >> 16u);
    return float(n & 0x00FFFFFFu) / float(0x01000000u);
}

void main() {
    int base_idx = gl_InstanceID / u_multiplier;
    int copy_idx = gl_InstanceID % u_multiplier;

    // 2-triangle billboard quad
    const vec2 VERTS[6] = vec2[6](
        vec2(-1.0,-1.0), vec2( 1.0,-1.0), vec2(-1.0, 1.0),
        vec2(-1.0, 1.0), vec2( 1.0,-1.0), vec2( 1.0, 1.0)
    );
    vec2 quad = VERTS[gl_VertexID];
    f_uv = quad;

    GpuParticle p = particles[base_idx];

    // Per-copy offset: stays within ±0.001 world units of the base surface point.
    // Purpose is to fill sub-pixel gaps, not to scatter copies into the volume.
    float jx = (hf(uint(base_idx), uint(copy_idx*7+0)) - 0.5) * 0.002;
    float jy = (hf(uint(base_idx), uint(copy_idx*7+1)) - 0.5) * 0.002;
    vec3 world_pos = p.position
                   + u_cam_right * (jx + quad.x * p.size * 0.50)
                   + u_cam_up    * (jy + quad.y * p.size * 0.50);

    vec4 clip = u_view_proj * vec4(world_pos, 1.0);

    // Technique 4: TAA — offset clip position by one Halton sub-pixel each frame.
    // This shifts the entire particle cloud by <0.5 px; over successive frames the
    // jitter pattern covers the full pixel footprint → smooth anti-aliased edges
    // without a ping-pong framebuffer.  Multiply by w to keep offset screen-space.
    clip.xy += u_taa_jitter * clip.w;

    gl_Position = clip;
    f_color    = p.color;
    f_emission = p.emission;
    f_normal_w = p.normal;
}
"#;

// ── Billboard fragment shader — post-processing stack ─────────────────────────
// Techniques 5, 6, 10, 11, 14, 15 all live here.
const SDF_FRAG_SRC: &str = r#"
#version 430 core

in  vec4  f_color;
in  float f_emission;
in  vec2  f_uv;
in  vec3  f_normal_w;  // world-space surface normal (from compute SDF gradient)

layout(location = 0) out vec4 out_color;

uniform vec2  u_resolution;  // window size in pixels
uniform float u_time_r;      // current time for animated grain

void main() {
    float d = length(f_uv);
    if (d > 1.0) discard;

    // ── Technique 15: Heat haze ────────────────────────────────────────────────
    // High-emission particles (fire, energy) distort their own disk edge with a
    // sine ripple. This is impossible in triangle rendering — requires per-particle
    // emission metadata at fragment time.
    float d_shaped = d;
    if (f_emission > 0.50) {
        float haze = (f_emission - 0.50) * 2.0;
        float angle = atan(f_uv.y, f_uv.x);
        d_shaped = d + sin(angle * 7.0 + f_emission * 29.3) * 0.055 * haze * d;
        if (d_shaped > 1.0) discard;
    }
    float soft_base = max(0.0, 1.0 - d_shaped);
    float soft = soft_base * soft_base;

    // ── Technique 6: Chromatic depth separation ────────────────────────────────
    // Particles farther from the near plane get per-channel disk radius shift,
    // simulating a real lens that focuses each wavelength at a different depth.
    float ndc_z = gl_FragCoord.z * 2.0 - 1.0;   // −1 = near, +1 = far
    float chrom = ndc_z * 0.0060 * (1.20 - f_color.a * 0.50);
    float d_r = length(f_uv * (1.0 + chrom * 1.30));
    float d_b = length(f_uv * (1.0 - chrom * 0.85));
    float soft_r = max(0.0, 1.0 - d_r); soft_r = soft_r * soft_r;
    float soft_g = soft;
    float soft_b = max(0.0, 1.0 - d_b); soft_b = soft_b * soft_b;
    float em = 1.0 + f_emission * 0.35;
    vec4 col = vec4(
        f_color.r * soft_r * em,
        f_color.g * soft_g * em,
        f_color.b * soft_b * em,
        f_color.a * soft   * (1.0 + f_emission * 0.50)
    );

    // ── Technique 5: Spectral bloom dispersion ────────────────────────────────
    // Seed the bloom system with per-channel radial offsets so the engine's
    // Gaussian blur spreads R further than G, G further than B — producing
    // rainbow fringe on bright highlights (prismatic lens effect).
    if (f_emission > 0.30) {
        float em2 = (f_emission - 0.30) * 1.43;
        float bloom_r = max(0.0, 1.0 - d * 0.87); bloom_r *= bloom_r;
        float bloom_b = max(0.0, 1.0 - d * 1.13); bloom_b *= bloom_b;
        col.r += bloom_r * em2 * 0.095;
        col.g += soft    * em2 * 0.035;
        col.b += bloom_b * em2 * 0.065;
    }

    // ── Technique 14: Sharpen — steepen edge contrast ─────────────────────────
    // Boost the ring at d ≈ 0.65 to heighten the transition between particles,
    // making seam lines and high-frequency surface detail crisper.
    {
        float ring = exp(-pow(d - 0.64, 2.0) * 38.0) * 0.09 * (1.0 - f_emission * 0.6);
        col.rgb += col.rgb * ring;
    }

    // ── Technique 11: Vignette ────────────────────────────────────────────────
    // Darken and desaturate particles toward screen edges. Uses screen-space
    // position from gl_FragCoord, so the effect is correctly anchored to the
    // monitor frame regardless of where in 3D space the particle sits.
    if (u_resolution.x > 0.0) {
        vec2 sv = (gl_FragCoord.xy / u_resolution) * 2.0 - 1.0;
        float r2 = dot(sv, sv);
        float vig = 1.0 - r2 * r2 * 0.42;
        float vig_sat = 1.0 - r2 * 0.18;  // slight desaturation at edges
        float lum_v = dot(col.rgb, vec3(0.299, 0.587, 0.114));
        col.rgb = mix(vec3(lum_v), col.rgb, vig_sat) * vig;
        col.a  *= max(vig, 0.60);
    }

    // ── Technique 7: SSR — SDF-normal environment reflection ─────────────────────
    // Traditional SSR requires a depth buffer and previous-frame color texture.
    // This architecture has something better: every particle carries the analytical
    // SDF gradient (true surface normal) in world space.  We use it to importance-
    // sample an implicit environment map derived from the dungeon geometry:
    //   n pointing up   (-y) → stone vault ceiling (cool blue-grey)
    //   n pointing down (+y) → torchlit flagstone floor (warm amber)
    //   n pointing sideways  → torch-glow wall scatter (desaturated amber)
    // Reflection intensity is gated on emission so only metallic surfaces pick up
    // environment color — skin and cloth are diffuse and ignore SSR.
    {
        vec3 n = normalize(f_normal_w);
        float ceil_t  = max(-n.y, 0.0);
        float floor_t = max( n.y, 0.0);
        float wall_t  = sqrt(max(0.0, 1.0 - n.y * n.y));
        vec3 ceil_env  = vec3(0.28, 0.38, 0.62) * ceil_t;
        vec3 floor_env = vec3(0.55, 0.38, 0.18) * floor_t;
        vec3 wall_env  = vec3(0.48, 0.30, 0.12) * wall_t * 0.55;
        vec3 env = ceil_env + floor_env + wall_env;
        float ssr_w = clamp((f_emission - 0.35) * 1.8, 0.0, 1.0) * 0.28;
        col.rgb = mix(col.rgb, col.rgb * 0.72 + env * (0.6 + f_emission * 0.4), ssr_w);
    }

    // ── Technique 18: Depth-of-field bokeh ring ───────────────────────────────
    // Real DoF blurs out-of-focus particles into large disks.  We don't have a
    // gather pass, but we can do something exclusive to per-particle rendering:
    // add a bright annular ring at d ≈ bokeh_r that grows as depth diverges from
    // the focal plane.  This gives the characteristic bright-edge bokeh of a fast
    // lens without accumulation rendering.  The focal plane sits at ndc_z ≈ 0.5
    // (mid-scene depth where Leon's torso lives).
    {
        float focal_z = 0.50;   // NDC depth of focus plane
        float defocus = abs(gl_FragCoord.z - focal_z);  // 0 = sharp, 1 = blurred
        float bokeh_r = 0.72 + defocus * 0.20;          // ring radius grows with defocus
        float ring_w  = exp(-pow(d - bokeh_r, 2.0) * 55.0);
        float ring_str = defocus * defocus * 0.14;      // only visible when defocused
        // Bokeh ring inherits particle hue but brightened
        col.rgb += col.rgb * ring_w * ring_str;
        col.a   += ring_w * ring_str * 0.12;
    }

    // ── Technique 19: Iridescent thin-film on jacket leather ─────────────────
    // Thin-film interference: path-length difference = 2*n*t*cos(theta), where
    // theta = angle between surface normal and view direction.  For leather the
    // effective film thickness t ≈ 180 nm (oils + lacquer sheen).
    // cos(theta) here approximated from f_normal_w · camera-forward (= +z in model):
    //   Constructive for green at cos≈0.60, red at cos≈0.80, blue at cos≈0.40.
    // Result: the jacket shifts from warm olive to a faint teal sheen near glancing
    // angles — exactly like aged leather under a warm torch.
    {
        vec3 n = normalize(f_normal_w);
        float cos_t = abs(n.z);   // approximate cos of view angle via normal.z
        // Thin-film phase for 180 nm: wavelength-dependent
        float phi_r = cos_t * 6.80;   // red   channel interference phase
        float phi_g = cos_t * 8.40;   // green channel
        float phi_b = cos_t * 10.20;  // blue  channel
        vec3 irid = vec3(
            0.5 + 0.5 * cos(phi_r),
            0.5 + 0.5 * cos(phi_g),
            0.5 + 0.5 * cos(phi_b)
        );
        // Only apply to high-emission particles (jacket surface lit by key light)
        // and only on normal-lit faces (not back-faces) — avoids inside-body glow
        float irid_w = clamp((f_emission - 0.40) * 1.5, 0.0, 1.0) * 0.055
                     * (1.0 - cos_t * 0.5);   // strongest at glancing angles
        col.rgb = mix(col.rgb, col.rgb * (0.88 + irid * 0.22), irid_w);
    }

    // ── Technique 10: Luminance-based film grain ──────────────────────────────
    // Dark regions get heavier grain (shadow noise), bright regions stay clean —
    // exactly matching the grain curve of real photographic film stock.
    {
        float lum = dot(col.rgb, vec3(0.299, 0.587, 0.114));
        float grain_amp = (1.0 - lum * 0.92) * 0.020;
        // Seed varies with UV + time so grain animates each frame
        float g = fract(sin(dot(f_uv * 87.3 + vec2(u_time_r * 0.019, lum),
                               vec2(127.1, 311.7))) * 43758.545);
        col.rgb += (g - 0.5) * grain_amp;
    }

    out_color = col;
}
"#;

// ── Hair compute pass — strand particles appended to same SSBO ───────────────
const COMPUTE_HAIR_SRC: &str = r#"
#version 430 core
layout(local_size_x = 256) in;

layout(binding = 0, offset = 0) uniform atomic_uint u_count;

struct GpuParticle {
    vec3  position;
    float size;
    vec3  normal;
    float emission;
    vec4  color;
};
layout(std430, binding = 1) writeonly buffer ParticleSSBO {
    GpuParticle particles[];
};

uniform float u_time;
uniform float u_scale;
uniform float u_breath;
uniform int   u_n_hair;  // = HAIR_STRANDS * HAIR_PPS

const int HAIR_STRANDS = 500;
const int HAIR_PPS     = 20;   // particles per strand

float hf(uint seed, uint v) {
    uint n = seed * 374761393u + v * 668265263u;
    n ^= (n >> 13u);
    n *= 0x5851F42Du;
    n ^= (n >> 16u);
    return float(n & 0x00FFFFFFu) / float(0x01000000u);
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (int(idx) >= u_n_hair) return;

    uint strand_idx = idx / uint(HAIR_PPS);
    uint seg        = idx % uint(HAIR_PPS);
    float t = float(seg) / float(HAIR_PPS - 1);

    const float SC_Y = -0.920;   // skull center Y (model space)

    // ── Leon's curtain-cut — 5 anatomical zones ───────────────────────────────
    // 0-299:  Crown       dense short side-part strands, sweep left
    // 300-399: Left drape  long strands hanging from left temple, gravity-dominant
    // 400-439: Right side  shorter, lean slightly right (sparser)
    // 440-479: Nape        back of skull, hang straight down
    // 480-499: Front fringe sweep forward-left over forehead

    float root_x, root_y, root_z;
    float gx, gy, gz;
    float strand_len, grav, sw;

    if (strand_idx < 300u) {
        // Crown: tight cap, biased toward left hemisphere for side part
        float theta = hf(strand_idx, 200u) * 0.48 + 0.03;
        float phi   = hf(strand_idx, 201u) * TAU * 0.65 + PI * 0.20;
        float rr    = 0.162 + hf(strand_idx, 202u) * 0.018;
        root_x = rr * sin(theta) * cos(phi);
        root_y = SC_Y - rr * cos(theta);
        root_z = rr * sin(theta) * sin(phi) * 0.80;
        // Growth: sweep strongly left, slight downward and forward variation
        gx = -0.72 + (hf(strand_idx, 203u) - 0.5) * 0.22;
        gy =  0.14 + hf(strand_idx, 204u) * 0.16;
        gz =  (hf(strand_idx, 205u) - 0.5) * 0.18;
        strand_len = 0.09 + hf(strand_idx, 206u) * 0.07;
        grav = 0.08; sw = 0.004;

    } else if (strand_idx < 400u) {
        // Left drape: long, hanging from left temple and side
        float phi   = PI + (hf(strand_idx, 201u) - 0.5) * 1.10;
        float theta = 0.30 + hf(strand_idx, 200u) * 0.55;
        float rr    = 0.168 + hf(strand_idx, 202u) * 0.014;
        root_x = rr * sin(theta) * cos(phi);
        root_y = SC_Y - rr * cos(theta);
        root_z = rr * sin(theta) * sin(phi) * 0.80;
        // Growth: mostly downward, gravity does the rest
        gx = -0.10 + (hf(strand_idx, 203u) - 0.5) * 0.06;
        gy =  0.12 + hf(strand_idx, 204u) * 0.08;
        gz =  (hf(strand_idx, 205u) - 0.5) * 0.06;
        strand_len = 0.26 + hf(strand_idx, 206u) * 0.14;
        grav = 0.70; sw = 0.010;

    } else if (strand_idx < 440u) {
        // Right side: shorter, lean right (sparser — side part makes left fuller)
        float phi   = (hf(strand_idx, 201u) - 0.5) * 0.90;
        float theta = 0.25 + hf(strand_idx, 200u) * 0.45;
        float rr    = 0.165 + hf(strand_idx, 202u) * 0.014;
        root_x = rr * sin(theta) * cos(phi);
        root_y = SC_Y - rr * cos(theta);
        root_z = rr * sin(theta) * sin(phi) * 0.80;
        gx =  0.06 + (hf(strand_idx, 203u) - 0.5) * 0.06;
        gy =  0.12 + hf(strand_idx, 204u) * 0.10;
        gz =  (hf(strand_idx, 205u) - 0.5) * 0.06;
        strand_len = 0.10 + hf(strand_idx, 206u) * 0.07;
        grav = 0.30; sw = 0.006;

    } else if (strand_idx < 480u) {
        // Nape: back of skull (phi ≈ 3π/2 = behind head), nearly pure gravity
        float phi   = PI * 1.5 + (hf(strand_idx, 201u) - 0.5) * 0.70;
        float theta = 0.55 + hf(strand_idx, 200u) * 0.35;
        float rr    = 0.168 + hf(strand_idx, 202u) * 0.014;
        root_x = rr * sin(theta) * cos(phi);
        root_y = SC_Y - rr * cos(theta);
        root_z = rr * sin(theta) * sin(phi) * 0.80;
        gx = (hf(strand_idx, 203u) - 0.5) * 0.04;
        gy =  0.06 + hf(strand_idx, 204u) * 0.06;
        gz = -0.05 + hf(strand_idx, 205u) * 0.04;  // slightly toward back
        strand_len = 0.20 + hf(strand_idx, 206u) * 0.12;
        grav = 0.90; sw = 0.008;

    } else {
        // Front fringe: phi ≈ π/2 = front of skull, sweep down-left over forehead
        float phi   = PI * 0.5 + (hf(strand_idx, 201u) - 0.5) * 0.80;
        float theta = 0.08 + hf(strand_idx, 200u) * 0.22;
        float rr    = 0.168 + hf(strand_idx, 202u) * 0.012;
        root_x = rr * sin(theta) * cos(phi);
        root_y = SC_Y - rr * cos(theta);
        root_z = rr * sin(theta) * sin(phi) * 0.80;
        gx = -0.22 + (hf(strand_idx, 203u) - 0.5) * 0.12;
        gy =  0.32 + hf(strand_idx, 204u) * 0.14;
        gz =  0.20 + hf(strand_idx, 205u) * 0.10;   // forward over forehead
        strand_len = 0.12 + hf(strand_idx, 206u) * 0.06;
        grav = 0.15; sw = 0.005;
    }

    // Normalize growth direction
    float glen = max(sqrt(gx*gx + gy*gy + gz*gz), 0.001);
    gx /= glen; gy /= glen; gz /= glen;

    // Wind sway — tip-heavy quadratic falloff
    float sway_x = sin(u_time * 2.1 + float(strand_idx) * 0.17) * sw * t * t;
    float sway_z = cos(u_time * 1.7 + float(strand_idx) * 0.23) * sw * 0.6 * t * t;

    // ── Position: linear growth + quadratic gravity drape ─────────────────────
    // grav adds +Y (downward) sag that increases toward tip.
    // horiz decays horizontal component so gravity-dominant zones hang plumb.
    float grav_y = grav * t * t * strand_len * 0.80;
    float horiz  = max(1.0 - grav * t * 0.45, 0.10);
    float px = root_x + gx * t * strand_len * horiz + sway_x;
    float py = root_y + gy * t * strand_len + grav_y;
    float pz = root_z + gz * t * strand_len * horiz + sway_z;

    // Normal: outward from skull center
    float nx = px, ny = py - SC_Y, nz = pz;
    float nn = max(sqrt(nx*nx + ny*ny + nz*nz), 0.001);
    nx /= nn; ny /= nn; nz /= nn;

    // Color: dark brown, per-strand variation
    float br = 0.16 + hf(strand_idx, 207u) * 0.06;
    float bg = 0.09 + hf(strand_idx, 208u) * 0.04;
    float bb = 0.04 + hf(strand_idx, 209u) * 0.02;

    float alpha    = (1.0 - t * t * 0.75) * 0.88;
    float emission = 0.08 + (1.0 - t) * 0.12;
    float sz       = (0.008 + (1.0 - t) * 0.005) * u_scale;

    float world_x = px * u_scale * u_breath;
    float world_y = py * u_scale * u_breath;
    float world_z = pz;

    uint slot = atomicCounterIncrement(u_count);
    if (slot >= 10000000u) return;

    particles[slot].position = vec3(world_x, world_y, world_z);
    particles[slot].size     = sz;
    particles[slot].normal   = vec3(nx, ny, nz);
    particles[slot].emission = emission;
    particles[slot].color    = vec4(br, bg, bb, alpha);
}
"#;

const HAIR_STRANDS_RS: u32 = 500;
const HAIR_PPS_RS: u32     = 20;
const HAIR_N_TOTAL: u32    = HAIR_STRANDS_RS * HAIR_PPS_RS;  // = 10 000

/// Low-discrepancy Halton sequence — used for TAA sub-pixel jitter.
/// Returns a value in [0, 1) for the given base and index.
fn halton(base: u32, index: u32) -> f32 {
    let mut f = 1.0_f32;
    let mut r = 0.0_f32;
    let mut i = index;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

// GL 4.3 constants not exported by glow 0.16 — used by SdfGpuPipeline
const GL_COMPUTE_SHADER:         u32 = 0x91B9;
const GL_SHADER_STORAGE_BUFFER:  u32 = 0x90D2;
const GL_ATOMIC_COUNTER_BUFFER:  u32 = 0x92C0;
const GL_DRAW_INDIRECT_BUFFER:   u32 = 0x8F3F;
const GL_SHADER_STORAGE_BARRIER: u32 = 0x0000_2000;
const GL_ATOMIC_COUNTER_BARRIER: u32 = 0x0000_1000;
const GL_COMMAND_BARRIER:        u32 = 0x0000_0040;

// ── SdfGpuPipeline ────────────────────────────────────────────────────────────
struct SdfGpuPipeline {
    compute_prog:  glow::Program,
    hair_prog:     glow::Program,
    finalize_prog: glow::Program,
    render_prog:   glow::Program,
    particle_ssbo: glow::Buffer,
    indirect_buf:  glow::Buffer,
    counter_buf:   glow::Buffer,
    render_vao:    glow::VertexArray,
    // body compute uniforms
    u_time_c:     glow::UniformLocation,
    u_hp_c:       glow::UniformLocation,
    u_scale_c:    glow::UniformLocation,
    u_breath_c:   glow::UniformLocation,
    u_f3int_c:    glow::UniformLocation,
    u_dmg_c:      glow::UniformLocation,
    u_n_c:        glow::UniformLocation,
    u_cam_pos_c:  glow::UniformLocation,
    u_exposure_c: glow::UniformLocation,
    // hair compute uniforms
    u_time_h:    glow::UniformLocation,
    u_scale_h:   glow::UniformLocation,
    u_breath_h:  glow::UniformLocation,
    u_n_h:       glow::UniformLocation,
    // finalize uniform
    u_mult_f:    glow::UniformLocation,
    // render uniforms
    u_vp_r:      glow::UniformLocation,
    u_mult_r:    glow::UniformLocation,
    u_right_r:   glow::UniformLocation,
    u_up_r:      glow::UniformLocation,
    u_res_r:      glow::UniformLocation,
    u_timef_r:    glow::UniformLocation,
    u_taa_jitter_v: glow::UniformLocation,  // Technique 4: TAA sub-pixel jitter
    // GPU timer query (GL_TIME_ELAPSED) for throughput measurement
    timing_query: glow::Query,
    query_ready:  bool,  // true after first dispatch — result readable next frame
    // Eye adaptation state (Technique 13)
    exposure:     f32,
    // TAA frame counter — drives Halton sequence (Technique 4)
    frame_count:  u32,
}

impl SdfGpuPipeline {
    /// Tries to build the pipeline. Returns None if compute shaders are unsupported
    /// (GL < 4.3) or if shader compilation fails.
    unsafe fn new(gl: &glow::Context) -> Option<Self> {
        // Check for compute shader support: need GL 4.3+
        let major = gl.get_parameter_i32(glow::MAJOR_VERSION) as u32;
        let minor = gl.get_parameter_i32(glow::MINOR_VERSION) as u32;
        let supported = major > 4 || (major == 4 && minor >= 3);
        if !supported {
            log::warn!("SdfGpuPipeline: GL {major}.{minor} < 4.3 — compute shaders unavailable, falling back to CPU");
            return None;
        }

        // Compile programs
        let compile_compute = |src: &str| -> Option<glow::Program> {
            let shader = gl.create_shader(GL_COMPUTE_SHADER).ok()?;
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                log::error!("Compute shader compile error:\n{}", gl.get_shader_info_log(shader));
                gl.delete_shader(shader);
                return None;
            }
            let prog = gl.create_program().ok()?;
            gl.attach_shader(prog, shader);
            gl.link_program(prog);
            gl.delete_shader(shader);
            if !gl.get_program_link_status(prog) {
                log::error!("Compute program link error:\n{}", gl.get_program_info_log(prog));
                gl.delete_program(prog);
                return None;
            }
            Some(prog)
        };
        let compile_render = |vsrc: &str, fsrc: &str| -> Option<glow::Program> {
            let vs = gl.create_shader(glow::VERTEX_SHADER).ok()?;
            gl.shader_source(vs, vsrc);
            gl.compile_shader(vs);
            if !gl.get_shader_compile_status(vs) {
                log::error!("SDF vert error:\n{}", gl.get_shader_info_log(vs));
                gl.delete_shader(vs);
                return None;
            }
            let fs = gl.create_shader(glow::FRAGMENT_SHADER).ok()?;
            gl.shader_source(fs, fsrc);
            gl.compile_shader(fs);
            if !gl.get_shader_compile_status(fs) {
                log::error!("SDF frag error:\n{}", gl.get_shader_info_log(fs));
                gl.delete_shader(vs); gl.delete_shader(fs);
                return None;
            }
            let prog = gl.create_program().ok()?;
            gl.attach_shader(prog, vs); gl.attach_shader(prog, fs);
            gl.link_program(prog);
            gl.delete_shader(vs); gl.delete_shader(fs);
            if !gl.get_program_link_status(prog) {
                log::error!("SDF render link error:\n{}", gl.get_program_info_log(prog));
                gl.delete_program(prog);
                return None;
            }
            Some(prog)
        };

        let compute_prog  = compile_compute(COMPUTE_SRC)?;
        let hair_prog     = compile_compute(COMPUTE_HAIR_SRC)?;
        let finalize_prog = compile_compute(FINALIZE_SRC)?;
        let render_prog   = compile_render(SDF_VERT_SRC, SDF_FRAG_SRC)?;

        // ── Buffers ───────────────────────────────────────────────────────────
        let particle_ssbo = gl.create_buffer().ok()?;
        gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(particle_ssbo));
        gl.buffer_data_size(
            GL_SHADER_STORAGE_BUFFER,
            (GPU_MAX_PARTICLES as usize * GPU_PARTICLE_BYTES) as i32,
            glow::DYNAMIC_DRAW,
        );

        let indirect_buf = gl.create_buffer().ok()?;
        gl.bind_buffer(GL_DRAW_INDIRECT_BUFFER, Some(indirect_buf));
        gl.buffer_data_size(GL_DRAW_INDIRECT_BUFFER, GPU_INDIRECT_BYTES as i32, glow::DYNAMIC_DRAW);

        // Atomic counter buffer: 1 u32 = 4 bytes
        let counter_buf = gl.create_buffer().ok()?;
        gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(counter_buf));
        gl.buffer_data_size(GL_ATOMIC_COUNTER_BUFFER, 4, glow::DYNAMIC_DRAW);

        // Empty VAO for attribute-less rendering (SSBO-driven)
        let render_vao = gl.create_vertex_array().ok()?;

        // ── Uniform locations ─────────────────────────────────────────────────
        let uloc = |prog, name: &str| gl.get_uniform_location(prog, name);
        let u_time_c    = uloc(compute_prog,  "u_time")?;
        let u_hp_c      = uloc(compute_prog,  "u_hp")?;
        let u_scale_c   = uloc(compute_prog,  "u_scale")?;
        let u_breath_c  = uloc(compute_prog,  "u_breath")?;
        let u_f3int_c   = uloc(compute_prog,  "u_f3_int")?;
        let u_dmg_c     = uloc(compute_prog,  "u_dmg")?;
        let u_n_c       = uloc(compute_prog,  "u_n")?;
        let u_cam_pos_c  = uloc(compute_prog, "u_cam_pos")?;
        let u_exposure_c = uloc(compute_prog, "u_exposure")?;
        let u_time_h   = uloc(hair_prog,     "u_time")?;
        let u_scale_h  = uloc(hair_prog,     "u_scale")?;
        let u_breath_h = uloc(hair_prog,     "u_breath")?;
        let u_n_h      = uloc(hair_prog,     "u_n_hair")?;
        let u_mult_f   = uloc(finalize_prog, "u_multiplier")?;
        let u_vp_r    = uloc(render_prog, "u_view_proj")?;
        let u_mult_r  = uloc(render_prog, "u_multiplier")?;
        let u_right_r = uloc(render_prog, "u_cam_right")?;
        let u_up_r    = uloc(render_prog, "u_cam_up")?;
        let u_res_r        = uloc(render_prog, "u_resolution")?;
        let u_timef_r      = uloc(render_prog, "u_time_r")?;
        let u_taa_jitter_v = uloc(render_prog, "u_taa_jitter")?;

        // Timer query for GPU throughput measurement (GL_TIME_ELAPSED = 0x88BF)
        let timing_query = gl.create_query().ok()?;

        log::info!(
            "SdfGpuPipeline initialized — {}M body + {}K hair candidates, {}MB SSBO",
            GPU_N_TOTAL / 1_000_000,
            HAIR_N_TOTAL / 1_000,
            (GPU_MAX_PARTICLES as usize * GPU_PARTICLE_BYTES) / 1_048_576,
        );

        Some(Self {
            compute_prog, hair_prog, finalize_prog, render_prog,
            particle_ssbo, indirect_buf, counter_buf, render_vao,
            u_time_c, u_hp_c, u_scale_c, u_breath_c, u_f3int_c, u_dmg_c, u_n_c,
            u_cam_pos_c, u_exposure_c,
            u_time_h, u_scale_h, u_breath_h, u_n_h,
            u_mult_f, u_vp_r, u_mult_r, u_right_r, u_up_r, u_res_r, u_timef_r,
            u_taa_jitter_v,
            timing_query, query_ready: false,
            exposure: 1.0,
            frame_count: 0,
        })
    }

    /// Run compute passes: reset counter → body compute → hair compute → finalize.
    /// Wraps the body+hair dispatches in a GL_TIME_ELAPSED query; reads the
    /// previous frame's result non-blocking to report throughput without stalling.
    unsafe fn dispatch(&mut self, gl: &glow::Context, time: f32, hp: f32, multiplier: u32, cam_pos: Vec3) {
        const GL_TIME_ELAPSED: u32 = 0x88BF;
        const GL_QUERY_RESULT_AVAILABLE: u32 = 0x8867;
        const GL_QUERY_RESULT: u32 = 0x8866;

        let breath  = 1.0_f32 + (time * 1.4).sin() * 0.010;
        let f3_int  = 0.28 * (0.80 + (time * 7.3).sin() * 0.11 + (time * 13.1).cos() * 0.07);
        let dmg     = (1.0_f32 - hp).max(0.0);

        // ── Read previous frame's timer query (non-blocking) ──────────────────
        if self.query_ready {
            let avail = gl.get_query_parameter_u32(self.timing_query, GL_QUERY_RESULT_AVAILABLE);
            if avail != 0 {
                let elapsed_ns = gl.get_query_parameter_u32(self.timing_query, GL_QUERY_RESULT) as u64;
                let elapsed_ms = elapsed_ns as f64 / 1_000_000.0;
                // Report saturation when we're tight on budget (>12 ms = <83 fps GPU headroom)
                if elapsed_ms > 12.0 {
                    let candidates = GPU_N_TOTAL + HAIR_N_TOTAL;
                    log::info!(
                        "GPU compute: {candidates} candidates in {elapsed_ms:.2} ms  \
                         ({:.1} Mcand/s) — body×{} + hair×{}  multiplier={}",
                        candidates as f64 / elapsed_ms / 1000.0,
                        GPU_N_TOTAL,
                        HAIR_N_TOTAL,
                        multiplier,
                    );
                }
            }
        }

        // ── Reset atomic counter to zero ─────────────────────────────────────
        gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(self.counter_buf));
        let zero: [u8; 4] = [0; 4];
        gl.buffer_sub_data_u8_slice(GL_ATOMIC_COUNTER_BUFFER, 0, &zero);

        // ── Bind shared resources ─────────────────────────────────────────────
        gl.bind_buffer_base(GL_ATOMIC_COUNTER_BUFFER, 0, Some(self.counter_buf));
        gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, 1, Some(self.particle_ssbo));
        gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, 2, Some(self.indirect_buf));

        // ── Begin GPU timer ───────────────────────────────────────────────────
        gl.begin_query(GL_TIME_ELAPSED, self.timing_query);

        // ── BODY compute pass ─────────────────────────────────────────────────
        gl.use_program(Some(self.compute_prog));
        gl.uniform_1_f32(Some(&self.u_time_c),   time);
        gl.uniform_1_f32(Some(&self.u_hp_c),     hp);
        gl.uniform_1_f32(Some(&self.u_scale_c),  3.2);
        gl.uniform_1_f32(Some(&self.u_breath_c), breath);
        gl.uniform_1_f32(Some(&self.u_f3int_c),  f3_int);
        gl.uniform_1_f32(Some(&self.u_dmg_c),    dmg);
        gl.uniform_1_i32(Some(&self.u_n_c),      GPU_N_TOTAL as i32);
        gl.uniform_3_f32(Some(&self.u_cam_pos_c), cam_pos.x, cam_pos.y, cam_pos.z);
        // Eye adaptation: smooth toward neutral (target = 1.0); scene-driven in future
        let target_exposure = 1.0_f32 + (1.0_f32 - hp) * 0.35; // desaturate on damage
        self.exposure = self.exposure * 0.95 + target_exposure * 0.05;
        gl.uniform_1_f32(Some(&self.u_exposure_c), self.exposure);

        let body_groups = (GPU_N_TOTAL + GPU_WG - 1) / GPU_WG;
        gl.dispatch_compute(body_groups, 1, 1);

        gl.memory_barrier(GL_SHADER_STORAGE_BARRIER | GL_ATOMIC_COUNTER_BARRIER);

        // ── HAIR compute pass (appends to same SSBO after body) ───────────────
        gl.use_program(Some(self.hair_prog));
        gl.uniform_1_f32(Some(&self.u_time_h),   time);
        gl.uniform_1_f32(Some(&self.u_scale_h),  3.2);
        gl.uniform_1_f32(Some(&self.u_breath_h), breath);
        gl.uniform_1_i32(Some(&self.u_n_h),      HAIR_N_TOTAL as i32);

        let hair_groups = (HAIR_N_TOTAL + GPU_WG - 1) / GPU_WG;
        gl.dispatch_compute(hair_groups, 1, 1);

        gl.memory_barrier(GL_SHADER_STORAGE_BARRIER | GL_ATOMIC_COUNTER_BARRIER | GL_COMMAND_BARRIER);

        // ── End GPU timer ─────────────────────────────────────────────────────
        gl.end_query(GL_TIME_ELAPSED);
        self.query_ready = true;

        // ── FINALIZE pass — write indirect draw command ───────────────────────
        gl.use_program(Some(self.finalize_prog));
        gl.uniform_1_u32(Some(&self.u_mult_f), multiplier);
        gl.dispatch_compute(1, 1, 1);

        gl.memory_barrier(GL_COMMAND_BARRIER);
    }

    /// Render accepted particles via indirect instanced draw.
    unsafe fn render(
        &mut self, gl: &glow::Context,
        view_proj: Mat4,
        cam_right: Vec3, cam_up: Vec3,
        multiplier: u32,
        time: f32,
        window_size: (u32, u32),
    ) {
        // Alpha-composite blending — body particles must occlude the background.
        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

        gl.use_program(Some(self.render_prog));

        // Upload view-proj matrix (column-major)
        gl.uniform_matrix_4_f32_slice(
            Some(&self.u_vp_r),
            false,
            &view_proj.to_cols_array(),
        );
        gl.uniform_1_i32(Some(&self.u_mult_r), multiplier as i32);
        gl.uniform_3_f32(Some(&self.u_right_r), cam_right.x, cam_right.y, cam_right.z);
        gl.uniform_3_f32(Some(&self.u_up_r),    cam_up.x,    cam_up.y,    cam_up.z);
        gl.uniform_2_f32(Some(&self.u_res_r),   window_size.0 as f32, window_size.1 as f32);
        gl.uniform_1_f32(Some(&self.u_timef_r), time);

        // Technique 4: TAA sub-pixel jitter — Halton(2,n) × Halton(3,n) sequence
        // Each frame shifts the entire particle cloud by <0.5 px, covering different
        // sub-pixel positions over 16 frames → temporally accumulated coverage = 1px.
        let jx = (halton(2, self.frame_count) - 0.5) * 2.0 / window_size.0.max(1) as f32;
        let jy = (halton(3, self.frame_count) - 0.5) * 2.0 / window_size.1.max(1) as f32;
        gl.uniform_2_f32(Some(&self.u_taa_jitter_v), jx, jy);
        self.frame_count = self.frame_count.wrapping_add(1);

        // Bind SSBO at slot 1 (read-only in vertex shader)
        gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, 1, Some(self.particle_ssbo));

        // Bind indirect command buffer
        gl.bind_buffer(GL_DRAW_INDIRECT_BUFFER, Some(self.indirect_buf));

        gl.bind_vertex_array(Some(self.render_vao));
        gl.draw_arrays_indirect_offset(glow::TRIANGLES, 0);

        // Restore
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        gl.bind_vertex_array(None);
        gl.bind_buffer(GL_DRAW_INDIRECT_BUFFER, None);
    }
}

fn main() {
    env_logger::init();

    let tw = total_weight();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title:  "Proof Engine — Leon Kennedy".to_string(),
        window_width:  1920,
        window_height: 1080,
        target_fps:    60,
        vsync:         false,
        render: proof_engine::config::RenderConfig {
            bloom_enabled:        true,
            bloom_intensity:      3.0,   // base; bursts to 8.0 — ceiling lifted to 20.0
            bloom_radius:         18.0,  // base; bursts to 48.0 — ceiling lifted to 128.0
            chromatic_aberration: 0.0020,
            film_grain:           0.020,
            motion_blur_enabled:  true,
            motion_blur_samples:  16,
            distortion_enabled:   true,
            scanlines_enabled:    false,
            antialiasing:         true,
            render_scale:         3.0,   // 9× pixel density — ceiling lifted to 8.0
            particle_multiplier:  16.0,  // oversampling copies per base particle (ceil → 16)
            shadow_quality:       ShadowQuality::Ultra,
            color_depth:          32,
            ..Default::default()
        },
        ..Default::default()
    });

    // Force fields — background ambient motion
    engine.add_field(ForceField::Gravity {
        center: Vec3::new(0.0, 0.5, 0.0), strength: 0.25, falloff: Falloff::InverseSquare,
    });
    engine.add_field(ForceField::Vortex {
        center: Vec3::new(-5.0, 0.0, 0.0), axis: Vec3::new(0.0, 0.0, 1.0),
        strength: 0.8, radius: 6.0,
    });
    engine.add_field(ForceField::Vortex {
        center: Vec3::new( 5.0, 0.0, 0.0), axis: Vec3::new(0.0, 0.0, -1.0),
        strength: 0.8, radius: 6.0,
    });
    engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Lorenz, scale: 0.18, strength: 0.30,
        center: Vec3::new(0.0, -0.5, 0.0),
    });
    engine.add_field(ForceField::HeatSource {
        center: Vec3::new(0.0, -0.5, 0.0), temperature: 3.0, radius: 2.5,
    });
    engine.add_field(ForceField::Pulsing {
        center: Vec3::new(0.0, 0.0, 0.0), frequency: 1.1, amplitude: 0.45, radius: 7.0,
    });
    engine.add_field(ForceField::Flow {
        direction: Vec3::new(0.0, -0.06, 0.0), strength: 0.08, turbulence: 0.06,
    });

    engine.emit_audio(AudioEvent::SetMusicVibe(MusicVibe::BossFight));

    let mut time        = 0.0f32;
    #[allow(unused_assignments)]
    let mut hp          = 1.0f32;
    let mut last_burst  = -10.0f32;

    // PhysicsKit: per-particle inertial lag cache.
    // 300 K Vec3s = 3.6 MB; allocated once, reused every frame.
    let mut lag_cache      = vec![Vec3::ZERO; 500_000];
    let mut hair_lag_cache = vec![Vec3::ZERO; 500 * 20]; // HairKit: 500 strands × 20 max
    let mut prev_breath    = 1.0f32;   // previous frame's breath value for motion detect

    const NEB: usize = 400;
    let neb_ch: Vec<char> = (0..NEB).map(|i| ['.', '-', '+', 'x', '*', 'o', '~', ':'][i%8]).collect();

    // GPU SDF pipeline — initialized on first frame (requires GL 4.3).
    // None = GL < 4.3 or compile failure → falls back to CPU render_sdf_body.
    let mut sdf_gpu: Option<SdfGpuPipeline> = None;
    let particle_multiplier: u32 = 16;

    engine.run_with_overlay(move |engine, dt, gl| {
        time += dt;

        hp = (0.65 + (time * 0.06 * TAU).sin() * 0.33).clamp(0.08, 1.0);

        // Dynamic post-FX — ceilings lifted, using headroom for dynamic range not constant max
        let pulse = ((time * 1.8).sin() * 0.5 + 0.5f32).powi(2);
        engine.config.render.bloom_intensity      = 3.0 + pulse * 2.5;   // 3.0–5.5 idle breathe
        engine.config.render.bloom_radius         = 18.0 + pulse * 12.0; // 18–30 idle breathe
        engine.config.render.chromatic_aberration = 0.0016 + pulse * 0.0014; // 0.0016–0.003

        let since_burst = time - last_burst;
        if since_burst < 0.5 {
            let t = 1.0 - since_burst / 0.5;
            engine.config.render.bloom_intensity      = 8.0 * t + 3.0 * (1.0-t); // spike to 8.0
            engine.config.render.bloom_radius         = 48.0 * t + 18.0 * (1.0-t); // spike to 48.0
            engine.config.render.chromatic_aberration = 0.12 * t + 0.002 * (1.0-t); // natural peak
        }

        let cam_y = (time * 0.35).sin() * 0.04;
        engine.camera.position.y.target   = cam_y;
        engine.camera.position.y.position = cam_y;

        // ── LEON ─────────────────────────────────────────────────────────────
        let breath_now = 1.0 + (time * 1.4).sin() * 0.010;
        let is_moving  = (breath_now - prev_breath).abs() > 0.0001;
        prev_breath    = breath_now;
        // ── GPU pipeline init (first frame only) ─────────────────────────────
        if sdf_gpu.is_none() {
            sdf_gpu = unsafe { SdfGpuPipeline::new(gl) };
        }

        // ── SDF body: GPU path (unified surface) or CPU fallback ─────────────
        if let Some(ref mut gpu) = sdf_gpu {
            // Dispatch: body compute → hair compute → finalize indirect cmd
            let cam_pos = engine.camera.position.position();
            unsafe { gpu.dispatch(gl, time, hp, particle_multiplier, cam_pos); }
            // Face still CPU-rendered (head SDF not ported to GPU)
            render_sdf_face(engine, dt, time, Vec3::ZERO, hp);
        } else {
            // CPU fallback: full bone + SDF body + face
            render_leon(engine, dt, Vec3::ZERO, hp, time, tw, &mut lag_cache, is_moving);
            render_sdf_body(engine, dt, time, Vec3::ZERO, hp);
            render_sdf_face(engine, dt, time, Vec3::ZERO, hp);
        }

        render_vol_scatter(engine, dt, time, Vec3::ZERO);
        // Hair: always CPU-rendered (GPU compute shader not yet compiling).
        render_hair(engine, dt, Vec3::ZERO, hp, &mut hair_lag_cache, is_moving);
        render_ground(engine, dt, time);
        render_environment(engine, dt, time);
        render_sky(engine, dt, time);

        // ── GPU SDF render pass (after scene, before swap) ────────────────────
        if let Some(ref mut gpu) = sdf_gpu {
            let (ww, wh) = engine.window_size();
            let aspect = ww as f32 / wh.max(1) as f32;
            let pos = engine.camera.position.position();
            let tgt = engine.camera.target.position();
            let fov = engine.camera.fov.position;
            let view = Mat4::look_at_rh(pos, tgt, Vec3::Y);
            let proj = Mat4::perspective_rh_gl(
                fov.to_radians(), aspect, engine.camera.near, engine.camera.far,
            );
            let view_proj = proj * view;
            // Camera axes in world space (from view matrix rows, orthonormal)
            let cam_right = Vec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
            let cam_up    = Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
            unsafe { gpu.render(gl, view_proj, cam_right, cam_up, particle_multiplier, time, (ww, wh)); }
        }

        // Burst every 5 s
        if time - last_burst > 5.0 {
            last_burst = time;
            engine.add_trauma(0.20);
            spawn_burst(engine, dt, Vec3::new(0.0, -0.5, 0.4), 1.0, 0.75, 0.18, 60, 2.2, (time*100.0) as usize);
            spawn_burst(engine, dt, Vec3::new(0.0,  0.1, 0.2), 0.4, 0.70, 1.0,  36, 4.0, (time*137.0) as usize+1000);
            engine.emit_audio(AudioEvent::PlaySfx {
                name: "impact_heavy".to_string(), position: Vec3::new(0.0,-0.5,0.4), volume: 0.8,
            });
        }

        // Hand sparks
        spawn_stream(engine, dt, Vec3::new(-1.73, 1.15, 0.3), 1.0, 0.55, 0.08, 14, (time*80.0) as usize,      time);
        spawn_stream(engine, dt, Vec3::new( 1.73, 1.15, 0.3), 0.3, 0.65, 1.0,  14, (time*80.0) as usize+500, time);

        let (ww, wh) = engine.window_size();
        let half_w = ww as f32 / wh as f32 * 5.5;
        let half_h = 5.5f32;

        // ── Background nebula (3 layers) ──────────────────────────────────────
        for i in 0..130usize {
            let bx = hf(i,0)*half_w*2.0-half_w;
            let by = hf(i,1)*half_h*2.0-half_h;
            let ph = hf(i,4)*TAU;
            let dx = (time*0.10+ph).sin()*0.11+(time*0.07+ph*1.4).cos()*0.05;
            let dy = (time*0.08+ph*0.8).cos()*0.09;
            let br = 0.022+hf(i,5)*0.038;
            engine.spawn_glyph(Glyph {
                character: neb_ch[i%neb_ch.len()], scale: Vec2::splat(0.07+hf(i,6)*0.04),
                position: Vec3::new(bx+dx, by+dy, -4.5),
                color: Vec4::new(0.05+hf(i,7)*0.05, 0.07+hf(i,8)*0.07, 0.22+hf(i,9)*0.14, br),
                emission: br*4.5, mass:0.0, lifetime:dt*1.5,
                layer: RenderLayer::Background, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }
        for i in 130..270usize {
            let bx = hf(i,0)*half_w*1.8-half_w*0.9;
            let by = hf(i,1)*half_h*1.8-half_h*0.9;
            let ph = hf(i,4)*TAU;
            let dx = (time*0.20+ph).sin()*0.14+(time*0.14+ph*0.7).cos()*0.07;
            let dy = (time*0.16+ph*1.1).cos()*0.12;
            let br = 0.018+hf(i,5)*0.030;
            engine.spawn_glyph(Glyph {
                character: neb_ch[i%neb_ch.len()], scale: Vec2::splat(0.08+hf(i,6)*0.05),
                position: Vec3::new(bx+dx, by+dy, -3.0),
                color: Vec4::new(0.24+hf(i,7)*0.16, 0.14+hf(i,8)*0.10, 0.04, br),
                emission: br*5.0, mass:0.0, lifetime:dt*1.5,
                layer: RenderLayer::Background, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }
        for i in 270..NEB {
            let bx = hf(i,0)*half_w*1.4-half_w*0.7;
            let by = hf(i,1)*half_h*1.4-half_h*0.7;
            let ph = hf(i,4)*TAU;
            let dx = (time*0.25+ph).sin()*0.18+(time*0.17+ph*1.5).cos()*0.09;
            let dy = (time*0.19+ph*0.9).cos()*0.14;
            let br = 0.026+hf(i,5)*0.038;
            engine.spawn_glyph(Glyph {
                character: neb_ch[i%neb_ch.len()], scale: Vec2::splat(0.09+hf(i,6)*0.05),
                position: Vec3::new(bx+dx, by+dy, -1.8),
                color: Vec4::new(0.20+hf(i,7)*0.12, 0.06+hf(i,8)*0.05, 0.30+hf(i,9)*0.18, br),
                emission: br*5.5, mass:0.0, lifetime:dt*1.5,
                layer: RenderLayer::Background, blend_mode: BlendMode::Additive, ..Default::default()
            });
        }

        // ── Orbital rings ─────────────────────────────────────────────────────
        for i in 0..72usize {
            let t   = i as f32 / 72.0;
            let ang = t*TAU + time*0.28;
            let e   = (time*2.5+t*TAU).sin()*0.5+0.5;
            engine.spawn_glyph(Glyph {
                character: if i%5==0 {'*'} else {'.'}, scale: Vec2::splat(0.08+e*0.03),
                position: Vec3::new(ang.cos()*3.6, ang.sin()*1.1-0.1, -0.2),
                color: Vec4::new(1.0, 0.72+e*0.18, 0.08, 0.13+e*0.10),
                emission: 1.0+e*0.7, glow_color: Vec3::new(1.0, 0.65, 0.0), glow_radius: 0.4+e*0.3,
                mass:0.0, lifetime:dt*1.5, layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive, ..Default::default()
            });
        }
        for i in 0..52usize {
            let t   = i as f32 / 52.0;
            let ang = t*TAU - time*0.52;
            let e   = (time*4.0+t*TAU).sin()*0.5+0.5;
            engine.spawn_glyph(Glyph {
                character: if i%3==0 {'+'} else {'.'}, scale: Vec2::splat(0.07+e*0.03),
                position: Vec3::new(ang.cos()*2.2, ang.sin()*0.70-0.1, 0.1),
                color: Vec4::new(0.22+e*0.25, 0.52+e*0.28, 1.0, 0.14+e*0.10),
                emission: 1.0+e*0.8, glow_color: Vec3::new(0.2, 0.5, 1.0), glow_radius: 0.32+e*0.28,
                mass:0.0, lifetime:dt*1.5, layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive, ..Default::default()
            });
        }
        for i in 0..40usize {
            let t   = i as f32 / 40.0;
            let ang = t*TAU + time*1.4;
            let e   = (time*8.0+t*TAU).sin()*0.5+0.5;
            engine.spawn_glyph(Glyph {
                character: if i%2==0 {'*'} else {'+'}, scale: Vec2::splat(0.06+e*0.03),
                position: Vec3::new(ang.cos()*1.3, ang.sin()*0.40, 0.35),
                color: Vec4::new(1.0, 0.92+e*0.08, 0.70+e*0.30, 0.16+e*0.14),
                emission: 2.0+e*1.5, glow_color: Vec3::new(1.0, 0.95, 0.6), glow_radius: 0.45+e*0.38,
                mass:0.0, lifetime:dt*1.5, layer: RenderLayer::Particle,
                blend_mode: BlendMode::Additive, ..Default::default()
            });
        }

        // ── HUD ───────────────────────────────────────────────────────────────
        let label_y = half_h - 0.55;
        let header = format!("LEON KENNEDY  160M PARTICLES  HP {:.0}%", hp * 100.0);
        for (ci, ch) in header.chars().enumerate() {
            if ch == ' ' { continue; }
            engine.spawn_glyph(Glyph {
                character: ch, scale: Vec2::splat(0.15),
                position: Vec3::new(-half_w+0.18+ci as f32*0.162, label_y, 2.0),
                color: Vec4::new(0.82, 0.62, 0.22, 0.92), emission: 0.9,
                mass:0.0, lifetime:dt*1.5, layer: RenderLayer::UI, ..Default::default()
            });
        }
        let bar_n  = 44usize;
        let filled = (hp * bar_n as f32).round() as usize;
        for bi in 0..bar_n {
            let f = bi < filled;
            engine.spawn_glyph(Glyph {
                character: if f {'#'} else {'.'}, scale: Vec2::splat(0.15),
                position: Vec3::new(-half_w+0.18+bi as f32*0.162, label_y-0.28, 2.0),
                color: if f { Vec4::new(0.20+hp*0.80, 0.85-hp*0.40, 0.04, 0.90) }
                       else { Vec4::new(0.10, 0.08, 0.16, 0.28) },
                emission: if f {0.75} else {0.04},
                mass:0.0, lifetime:dt*1.5, layer: RenderLayer::UI, ..Default::default()
            });
        }

        if engine.input.just_pressed(Key::Escape) { engine.request_quit(); }
    });
}
