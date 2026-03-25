//! Layered entity rendering — four visual layers peeling away as HP drops.
//!
//! Layer 4 (outermost): Particle density aura
//! Layer 3: Metaball/isosurface primary form
//! Layer 2: Mathematical curve skeleton
//! Layer 1 (innermost): SDF glyph identity markers
//!
//! At full HP, only the outer layers are visible (solid form with aura).
//! As damage accumulates, outer layers thin and inner layers emerge.
//! On death, layers dissolve in sequence from outside in.

use glam::{Vec2, Vec3, Vec4};
use crate::glyph::{Glyph, GlyphId, RenderLayer, BlendMode};
use crate::particle::density_entity::{DensityEntity, DensityParticle, ShapeField, ShapeBone};
use crate::curves::entity_curves::{CurveEntity, EntityCurve, CurveType};
use crate::curves::tessellate::tessellate_curve;
use crate::curves::curve_renderer::render_curve_entity;
use crate::math::{MathFunction, ForceField};
use std::f32::consts::TAU;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Layer visibility
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Controls opacity of each rendering layer based on HP.
#[derive(Debug, Clone, Copy)]
pub struct LayerVisibility {
    /// Particle density cloud opacity (outermost).
    pub density_opacity: f32,
    /// Metaball/isosurface opacity.
    pub metaball_opacity: f32,
    /// Mathematical curve skeleton opacity.
    pub curve_opacity: f32,
    /// SDF glyph identity markers opacity (innermost).
    pub glyph_opacity: f32,
}

impl LayerVisibility {
    /// Compute layer visibility from HP ratio (0.0-1.0).
    pub fn from_hp(hp_ratio: f32) -> Self {
        let hp = hp_ratio.clamp(0.0, 1.0);

        if hp > 0.75 {
            // Full HP: solid form, no inner layers visible
            let t = (hp - 0.75) / 0.25; // 0 at 75%, 1 at 100%
            Self {
                density_opacity: 0.8 + t * 0.2,
                metaball_opacity: 0.8 + t * 0.2,
                curve_opacity: 0.0,
                glyph_opacity: 0.0,
            }
        } else if hp > 0.5 {
            // Moderate damage: surface thinning, curves emerging
            let t = (hp - 0.5) / 0.25; // 0 at 50%, 1 at 75%
            Self {
                density_opacity: 0.5 + t * 0.3,
                metaball_opacity: 0.5 + t * 0.3,
                curve_opacity: (1.0 - t) * 0.3,
                glyph_opacity: 0.0,
            }
        } else if hp > 0.25 {
            // Heavy damage: holes in surface, curves visible, glyphs starting
            let t = (hp - 0.25) / 0.25; // 0 at 25%, 1 at 50%
            Self {
                density_opacity: 0.2 + t * 0.3,
                metaball_opacity: 0.2 + t * 0.3,
                curve_opacity: 0.4 + (1.0 - t) * 0.3,
                glyph_opacity: (1.0 - t) * 0.3,
            }
        } else {
            // Critical: surface barely exists, curves fraying, glyphs exposed
            let t = hp / 0.25; // 0 at 0%, 1 at 25%
            Self {
                density_opacity: t * 0.2,
                metaball_opacity: t * 0.2,
                curve_opacity: 0.5 + (1.0 - t) * 0.5,
                glyph_opacity: 0.5 + (1.0 - t) * 0.3,
            }
        }
    }

    /// All layers at zero (fully dissolved).
    pub fn zero() -> Self {
        Self { density_opacity: 0.0, metaball_opacity: 0.0, curve_opacity: 0.0, glyph_opacity: 0.0 }
    }

    /// All layers at full (for debug/editor).
    pub fn full() -> Self {
        Self { density_opacity: 1.0, metaball_opacity: 1.0, curve_opacity: 1.0, glyph_opacity: 1.0 }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Glyph layer (innermost identity markers)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A 3D-positioned glyph instance for the identity marker layer.
#[derive(Debug, Clone)]
pub struct Glyph3DInstance {
    pub character: char,
    pub offset: Vec2,
    pub base_offset: Vec2,
    pub color: Vec4,
    pub emission: f32,
    pub scale: f32,
    pub velocity: Vec2,
    pub rotation: f32,
    pub angular_velocity: f32,
    pub alive: bool,
}

impl Glyph3DInstance {
    pub fn new(character: char, offset: Vec2, color: Vec4) -> Self {
        Self {
            character, offset, base_offset: offset,
            color, emission: 1.5, scale: 0.4,
            velocity: Vec2::ZERO, rotation: 0.0, angular_velocity: 0.0,
            alive: true,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Dissolution state
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Tracks the sequenced death dissolution across all layers.
#[derive(Debug, Clone)]
pub struct DissolutionState {
    pub active: bool,
    pub frame: u32,
    pub elapsed: f32,
    /// Which layers have started dissolving.
    pub density_started: bool,
    pub metaball_started: bool,
    pub curves_started: bool,
    pub glyphs_started: bool,
    /// Which layers have finished dissolving.
    pub density_done: bool,
    pub metaball_done: bool,
    pub curves_done: bool,
    pub glyphs_done: bool,
}

impl DissolutionState {
    pub fn inactive() -> Self {
        Self {
            active: false, frame: 0, elapsed: 0.0,
            density_started: false, metaball_started: false,
            curves_started: false, glyphs_started: false,
            density_done: false, metaball_done: false,
            curves_done: false, glyphs_done: false,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.density_done && self.metaball_done && self.curves_done && self.glyphs_done
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Layered Entity
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A multi-layered entity with four visual systems that peel away as HP drops.
pub struct LayeredEntity {
    /// Layer 4 (outermost): particle density aura.
    pub density_layer: Option<DensityEntity>,
    /// Layer 3: metaball/isosurface primary form.
    /// (Stored as opacity + emission parameters; actual metaball rendering
    /// would use the metaball module when the GPU pipeline supports it.)
    pub metaball_opacity: f32,
    pub metaball_emission: f32,
    /// Layer 2: mathematical curve skeleton.
    pub curve_layer: Option<CurveEntity>,
    /// Layer 1 (innermost): identity glyph markers.
    pub glyph_layer: Vec<Glyph3DInstance>,

    /// World position.
    pub position: Vec3,
    /// Current HP.
    pub hp: f32,
    /// Maximum HP.
    pub max_hp: f32,
    /// Computed layer visibility.
    pub layer_visibility: LayerVisibility,
    /// Death dissolution state.
    pub dissolution: DissolutionState,
    /// Whether the entity is alive.
    pub alive: bool,
    /// Entity name.
    pub name: String,
    /// Unique ID.
    pub id: u32,
    /// Accumulated time.
    pub time: f32,
    /// Base color (propagated to all layers).
    pub base_color: Vec4,
}

impl LayeredEntity {
    // ════════════════════════════════════════════════════════════════════════
    // Construction
    // ════════════════════════════════════════════════════════════════════════

    pub fn new(name: &str, position: Vec3, max_hp: f32) -> Self {
        Self {
            density_layer: None,
            metaball_opacity: 1.0,
            metaball_emission: 1.0,
            curve_layer: None,
            glyph_layer: Vec::new(),
            position,
            hp: max_hp,
            max_hp,
            layer_visibility: LayerVisibility::from_hp(1.0),
            dissolution: DissolutionState::inactive(),
            alive: true,
            name: name.to_string(),
            id: 0,
            time: 0.0,
            base_color: Vec4::new(0.5, 0.7, 1.0, 1.0),
        }
    }

    /// Set the density (particle cloud) layer.
    pub fn with_density(mut self, density: DensityEntity) -> Self {
        self.density_layer = Some(density);
        self
    }

    /// Set the curve (mathematical skeleton) layer.
    pub fn with_curves(mut self, curves: CurveEntity) -> Self {
        self.curve_layer = Some(curves);
        self
    }

    /// Add a glyph identity marker.
    pub fn with_glyph(mut self, glyph: Glyph3DInstance) -> Self {
        self.glyph_layer.push(glyph);
        self
    }

    /// Add multiple glyph markers at once.
    pub fn with_glyphs(mut self, glyphs: Vec<Glyph3DInstance>) -> Self {
        self.glyph_layer = glyphs;
        self
    }

    pub fn with_color(mut self, color: Vec4) -> Self {
        self.base_color = color;
        self
    }

    // ════════════════════════════════════════════════════════════════════════
    // Update
    // ════════════════════════════════════════════════════════════════════════

    /// Main update: advance all layers, compute visibility, handle dissolution.
    pub fn update(&mut self, dt: f32) {
        self.time += dt;

        if self.dissolution.active {
            self.dissolve_update(dt);
            return;
        }

        // Compute layer visibility from HP
        let hp_ratio = (self.hp / self.max_hp).clamp(0.0, 1.0);
        self.layer_visibility = LayerVisibility::from_hp(hp_ratio);

        // Update density layer
        if let Some(ref mut density) = self.density_layer {
            density.hp_ratio = hp_ratio;
            density.tick(dt);
            // Modulate density particle emission by visibility
            let density_em = self.layer_visibility.density_opacity;
            for p in &mut density.particles {
                p.emission = p.emission * density_em;
            }
        }

        // Update curve layer
        if let Some(ref mut curves) = self.curve_layer {
            curves.hp_ratio = hp_ratio;
            curves.emission_mult = self.layer_visibility.curve_opacity;
            curves.tick(dt);
        }

        // Update metaball parameters
        self.metaball_opacity = self.layer_visibility.metaball_opacity;
        self.metaball_emission = self.layer_visibility.metaball_opacity * 1.5;

        // Update glyph layer (spring toward base positions with jitter from HP)
        let glyph_vis = self.layer_visibility.glyph_opacity;
        let jitter = (1.0 - hp_ratio) * 0.1;
        for glyph in &mut self.glyph_layer {
            if !glyph.alive { continue; }
            // Spring toward base
            let to_base = glyph.base_offset - glyph.offset;
            glyph.velocity += to_base * 5.0 * dt;
            glyph.velocity *= 0.9; // damping
            glyph.offset += glyph.velocity * dt;
            // Jitter
            glyph.offset.x += hash_noise(self.time + glyph.base_offset.x * 10.0) * jitter;
            glyph.offset.y += hash_noise(self.time + glyph.base_offset.y * 10.0 + 50.0) * jitter;
            // Rotation from angular velocity
            glyph.rotation += glyph.angular_velocity * dt;
        }

        // Check for death
        if self.hp <= 0.0 && self.alive {
            self.begin_dissolution();
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Damage
    // ════════════════════════════════════════════════════════════════════════

    /// Take damage at an impact point.
    pub fn take_damage(&mut self, amount: f32, impact_point: Vec2, impact_direction: Vec2) {
        self.hp = (self.hp - amount).max(0.0);

        // Distribute damage to layers
        // Density: particles near impact scatter
        if let Some(ref mut density) = self.density_layer {
            density.apply_hit(impact_point, amount, 2.0);
        }

        // Curves: recoil from impact
        if let Some(ref mut curves) = self.curve_layer {
            for curve in &mut curves.curves {
                curve.apply_hit_recoil(impact_direction, amount * 0.3);
            }
        }

        // Glyphs: impulse away from impact
        for glyph in &mut self.glyph_layer {
            let to_glyph = glyph.offset - impact_point;
            let dist = to_glyph.length();
            if dist < 2.0 {
                let impulse = to_glyph.normalize_or_zero() * amount * 0.02 / (dist + 0.1);
                glyph.velocity += impulse;
                glyph.angular_velocity += (hash_noise(glyph.offset.x * 7.0) - 0.5) * amount * 0.05;
            }
        }

        if self.hp <= 0.0 && self.alive {
            self.begin_dissolution();
        }
    }

    /// Apply a critical hit: permanent particle loss + curve break.
    pub fn take_crit(&mut self, amount: f32, impact_point: Vec2, impact_direction: Vec2) {
        self.take_damage(amount, impact_point, impact_direction);

        // Density: permanently kill particles
        if let Some(ref mut density) = self.density_layer {
            density.apply_crit(impact_point, amount);
        }

        // Curves: break a random curve
        if let Some(ref mut curves) = self.curve_layer {
            curves.break_random_curve(self.time as u32);
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Force field response
    // ════════════════════════════════════════════════════════════════════════

    /// Apply an external force to all layers.
    pub fn apply_force(&mut self, force: Vec2) {
        if let Some(ref mut density) = self.density_layer {
            for p in &mut density.particles {
                if p.alive { p.velocity += force * 0.1; }
            }
        }
        if let Some(ref mut curves) = self.curve_layer {
            for curve in &mut curves.curves {
                curve.apply_force(force);
            }
        }
        for glyph in &mut self.glyph_layer {
            glyph.velocity += force * 0.05;
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Death dissolution (sequenced across layers)
    // ════════════════════════════════════════════════════════════════════════

    fn begin_dissolution(&mut self) {
        self.alive = false;
        self.dissolution.active = true;
        self.dissolution.frame = 0;
        self.dissolution.elapsed = 0.0;
    }

    fn dissolve_update(&mut self, dt: f32) {
        self.dissolution.elapsed += dt;
        self.dissolution.frame += 1;
        let frame = self.dissolution.frame;

        // Frame 0-30: density explodes outward
        if frame <= 30 && !self.dissolution.density_started {
            self.dissolution.density_started = true;
            if let Some(ref mut density) = self.density_layer {
                density.die();
            }
        }
        if frame > 30 {
            if let Some(ref mut density) = self.density_layer {
                density.tick(dt);
                if density.is_dissolved() { self.dissolution.density_done = true; }
            } else {
                self.dissolution.density_done = true;
            }
        }

        // Frame 15-60: metaball sources decay
        if frame >= 15 && !self.dissolution.metaball_started {
            self.dissolution.metaball_started = true;
        }
        if frame >= 15 {
            let t = ((frame - 15) as f32 / 45.0).min(1.0);
            self.metaball_opacity = (1.0 - t).max(0.0);
            self.metaball_emission = (1.0 - t) * 2.0; // flash then fade
            if frame >= 60 { self.dissolution.metaball_done = true; }
        }

        // Frame 30-90: curves lose stiffness, scatter
        if frame >= 30 && !self.dissolution.curves_started {
            self.dissolution.curves_started = true;
            if let Some(ref mut curves) = self.curve_layer {
                curves.die();
            }
        }
        if frame >= 30 {
            if let Some(ref mut curves) = self.curve_layer {
                curves.tick(dt);
                if curves.is_dissolved() { self.dissolution.curves_done = true; }
            } else {
                self.dissolution.curves_done = true;
            }
        }

        // Frame 45-120: glyphs get physics, tumble and scatter
        if frame >= 45 && !self.dissolution.glyphs_started {
            self.dissolution.glyphs_started = true;
            for glyph in &mut self.glyph_layer {
                let angle = hash_noise(glyph.offset.x * 3.0 + glyph.offset.y * 7.0) * TAU;
                glyph.velocity += Vec2::new(angle.cos(), angle.sin()) * 2.0;
                glyph.angular_velocity = (hash_noise(glyph.offset.x * 5.0) - 0.5) * 10.0;
            }
        }
        if frame >= 45 {
            let glyph_t = ((frame - 45) as f32 / 75.0).min(1.0);
            for glyph in &mut self.glyph_layer {
                glyph.velocity *= 0.98;
                glyph.offset += glyph.velocity * dt;
                glyph.rotation += glyph.angular_velocity * dt;
                // Fade
                glyph.color.w = (1.0 - glyph_t).max(0.0);
                glyph.emission = (1.0 - glyph_t) * 2.0;
            }
            if frame >= 120 { self.dissolution.glyphs_done = true; }
        }

        // All done after frame 180
        if frame >= 180 {
            self.dissolution.density_done = true;
            self.dissolution.metaball_done = true;
            self.dissolution.curves_done = true;
            self.dissolution.glyphs_done = true;
        }

        // Update layer visibility during dissolution
        self.layer_visibility = LayerVisibility {
            density_opacity: if self.dissolution.density_done { 0.0 } else { (1.0 - self.dissolution.elapsed / 1.0).max(0.0) },
            metaball_opacity: self.metaball_opacity,
            curve_opacity: if self.dissolution.curves_done { 0.0 } else { (1.0 - (self.dissolution.elapsed - 0.5).max(0.0) / 1.5).max(0.0) },
            glyph_opacity: if self.dissolution.glyphs_done { 0.0 } else { (1.0 - (self.dissolution.elapsed - 0.75).max(0.0) / 1.5).max(0.0) },
        };
    }

    // ════════════════════════════════════════════════════════════════════════
    // Rendering
    // ════════════════════════════════════════════════════════════════════════

    /// Render all visible layers by spawning glyphs. Returns glyph count.
    pub fn render(&self, spawn_fn: &mut dyn FnMut(Glyph) -> GlyphId, dt: f32) -> usize {
        let mut count = 0;
        let vis = &self.layer_visibility;

        // Layer 4 (back): Density cloud
        if vis.density_opacity > 0.01 {
            if let Some(ref density) = self.density_layer {
                for p in &density.particles {
                    if !p.alive { continue; }
                    let alpha = p.color.w * vis.density_opacity;
                    if alpha < 0.005 { continue; }
                    spawn_fn(Glyph {
                        character: '.', scale: Vec2::splat(p.size * 5.0),
                        position: Vec3::new(self.position.x + p.position.x, self.position.y + p.position.y, self.position.z - 0.2),
                        color: Vec4::new(p.color.x, p.color.y, p.color.z, alpha),
                        emission: p.emission * vis.density_opacity,
                        glow_color: Vec3::new(p.color.x, p.color.y, p.color.z),
                        glow_radius: p.emission * 0.3 * vis.density_opacity,
                        mass: 0.0, lifetime: dt * 1.5,
                        layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                        ..Default::default()
                    });
                    count += 1;
                }
            }
        }

        // Layer 3: Metaball (rendered as a filled shape approximation using dense glyphs)
        if vis.metaball_opacity > 0.01 {
            // Approximate metaball as a solid core of overlapping '#' glyphs
            let mb_count = 20;
            for i in 0..mb_count {
                let angle = (i as f32 / mb_count as f32) * TAU;
                let r = 0.3 * vis.metaball_opacity;
                let x = r * angle.cos();
                let y = r * angle.sin() + 0.3;
                spawn_fn(Glyph {
                    character: '#', scale: Vec2::splat(0.35 * vis.metaball_opacity),
                    position: Vec3::new(self.position.x + x, self.position.y + y, self.position.z - 0.1),
                    color: Vec4::new(self.base_color.x, self.base_color.y, self.base_color.z, vis.metaball_opacity * 0.4),
                    emission: self.metaball_emission * 0.5,
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
                count += 1;
            }
        }

        // Layer 2: Curves
        if vis.curve_opacity > 0.01 {
            if let Some(ref curves) = self.curve_layer {
                for curve in &curves.curves {
                    if !curve.alive && curve.kinetic_energy() < 0.001 { continue; }
                    let polyline = tessellate_curve(curve);
                    for (i, pt) in polyline.iter().enumerate() {
                        let t = i as f32 / polyline.len().max(1) as f32;
                        let alpha = curve.color.w * vis.curve_opacity;
                        if alpha < 0.005 { continue; }
                        let ch = if curve.thickness > 0.03 { '*' } else { '.' };
                        spawn_fn(Glyph {
                            character: ch, scale: Vec2::splat(curve.thickness * 4.0),
                            position: Vec3::new(
                                self.position.x + curves.position.x + pt.x,
                                self.position.y + curves.position.y + pt.y,
                                self.position.z,
                            ),
                            color: Vec4::new(curve.color.x, curve.color.y, curve.color.z, alpha),
                            emission: curve.emission * vis.curve_opacity * curves.emission_mult,
                            mass: 0.0, lifetime: dt * 1.5,
                            layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                        count += 1;
                    }
                }
            }
        }

        // Layer 1 (front): Glyphs
        if vis.glyph_opacity > 0.01 {
            for glyph in &self.glyph_layer {
                if !glyph.alive { continue; }
                let alpha = glyph.color.w * vis.glyph_opacity;
                if alpha < 0.005 { continue; }
                spawn_fn(Glyph {
                    character: glyph.character, scale: Vec2::splat(glyph.scale),
                    position: Vec3::new(
                        self.position.x + glyph.offset.x,
                        self.position.y + glyph.offset.y,
                        self.position.z + 0.1,
                    ),
                    rotation: glyph.rotation,
                    color: Vec4::new(glyph.color.x, glyph.color.y, glyph.color.z, alpha),
                    emission: glyph.emission * vis.glyph_opacity,
                    glow_color: Vec3::new(glyph.color.x, glyph.color.y, glyph.color.z),
                    glow_radius: glyph.emission * 0.5 * vis.glyph_opacity,
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
                count += 1;
            }
        }

        count
    }

    // ════════════════════════════════════════════════════════════════════════
    // Queries
    // ════════════════════════════════════════════════════════════════════════

    pub fn hp_ratio(&self) -> f32 { (self.hp / self.max_hp).clamp(0.0, 1.0) }

    pub fn is_dissolved(&self) -> bool {
        !self.alive && self.dissolution.is_complete()
    }

    /// Combined bounding box from all layers.
    pub fn get_bounds(&self) -> (Vec2, Vec2) {
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);

        if let Some(ref density) = self.density_layer {
            let (dmin, dmax) = density.bounds();
            min = min.min(dmin); max = max.max(dmax);
        }
        if let Some(ref curves) = self.curve_layer {
            let (cmin, cmax) = curves.bounding_box();
            min = min.min(cmin); max = max.max(cmax);
        }
        for glyph in &self.glyph_layer {
            min = min.min(glyph.offset - Vec2::splat(glyph.scale));
            max = max.max(glyph.offset + Vec2::splat(glyph.scale));
        }

        (min, max)
    }

    /// Total alive particle count across all layers.
    pub fn total_particle_count(&self) -> u32 {
        let density_count = self.density_layer.as_ref().map(|d| d.alive_count()).unwrap_or(0);
        let curve_count = self.curve_layer.as_ref().map(|c| c.alive_curve_count() as u32).unwrap_or(0);
        let glyph_count = self.glyph_layer.iter().filter(|g| g.alive).count() as u32;
        density_count + curve_count + glyph_count
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Builder for common entity configurations
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Convenience builders for layered entities.
pub struct LayeredEntityBuilder;

impl LayeredEntityBuilder {
    /// Create a fully layered mage entity.
    pub fn mage(position: Vec3) -> LayeredEntity {
        use crate::particle::density_entity::*;
        use crate::particle::shape_templates::DensityTemplates;
        use crate::curves::templates::CurveTemplates;

        let density = DensityTemplates::mage(position);
        let curves = CurveTemplates::mage(position);
        let glyphs = vec![
            Glyph3DInstance::new('@', Vec2::new(0.0, 0.9), Vec4::new(0.5, 0.7, 1.0, 0.9)),
            Glyph3DInstance::new('#', Vec2::new(0.0, 0.4), Vec4::new(0.4, 0.6, 0.9, 0.8)),
            Glyph3DInstance::new('*', Vec2::new(0.7, 1.0), Vec4::new(0.8, 0.8, 1.0, 0.95)),
            Glyph3DInstance::new('<', Vec2::new(-0.6, 0.4), Vec4::new(0.35, 0.55, 1.0, 0.7)),
            Glyph3DInstance::new('>', Vec2::new(0.6, 0.4), Vec4::new(0.35, 0.55, 1.0, 0.7)),
        ];

        LayeredEntity::new("Mage", position, 100.0)
            .with_density(density)
            .with_curves(curves)
            .with_glyphs(glyphs)
            .with_color(Vec4::new(0.3, 0.5, 1.0, 1.0))
    }

    /// Create a fully layered boss entity.
    pub fn boss(position: Vec3) -> LayeredEntity {
        use crate::particle::density_entity::*;
        use crate::particle::shape_templates::DensityTemplates;
        use crate::curves::templates::CurveTemplates;

        let density = DensityTemplates::boss("Chaos Lord", position, Vec4::new(0.9, 0.15, 0.3, 0.9), 3000);
        let curves = CurveTemplates::boss(position);
        let glyphs = vec![
            Glyph3DInstance::new('X', Vec2::new(-0.25, 0.9), Vec4::new(1.0, 0.9, 0.1, 1.0)),
            Glyph3DInstance::new('X', Vec2::new(0.25, 0.9), Vec4::new(1.0, 0.9, 0.1, 1.0)),
            Glyph3DInstance::new('H', Vec2::new(0.0, 0.0), Vec4::new(0.8, 0.1, 0.2, 0.9)),
            Glyph3DInstance::new('^', Vec2::new(-0.4, 1.3), Vec4::new(0.9, 0.3, 0.6, 0.8)),
            Glyph3DInstance::new('^', Vec2::new(0.4, 1.3), Vec4::new(0.9, 0.3, 0.6, 0.8)),
            Glyph3DInstance::new('v', Vec2::new(0.0, -0.7), Vec4::new(0.7, 0.15, 0.2, 0.7)),
        ];

        LayeredEntity::new("Chaos Lord", position, 500.0)
            .with_density(density)
            .with_curves(curves)
            .with_glyphs(glyphs)
            .with_color(Vec4::new(0.9, 0.15, 0.3, 1.0))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Noise helper
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn hash_noise(x: f32) -> f32 {
    let n = (x * 374761.393) as i32;
    let n = (n as u32) ^ ((n as u32) >> 13);
    let n = n.wrapping_mul(0x5851F42D);
    (n & 0x00FF_FFFF) as f32 / 0x0080_0000 as f32 - 1.0
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_visibility_full_hp() {
        let vis = LayerVisibility::from_hp(1.0);
        assert!(vis.density_opacity > 0.9);
        assert!(vis.metaball_opacity > 0.9);
        assert!(vis.curve_opacity < 0.01);
        assert!(vis.glyph_opacity < 0.01);
    }

    #[test]
    fn test_layer_visibility_half_hp() {
        let vis = LayerVisibility::from_hp(0.5);
        assert!(vis.density_opacity > 0.3);
        assert!(vis.curve_opacity > 0.1);
    }

    #[test]
    fn test_layer_visibility_critical() {
        let vis = LayerVisibility::from_hp(0.1);
        assert!(vis.density_opacity < 0.15);
        assert!(vis.curve_opacity > 0.7);
        assert!(vis.glyph_opacity > 0.5);
    }

    #[test]
    fn test_take_damage() {
        let mut ent = LayeredEntity::new("test", Vec3::ZERO, 100.0);
        ent.take_damage(30.0, Vec2::ZERO, Vec2::X);
        assert!((ent.hp - 70.0).abs() < 0.01);
    }

    #[test]
    fn test_death_triggers_dissolution() {
        let mut ent = LayeredEntity::new("test", Vec3::ZERO, 100.0);
        ent.take_damage(100.0, Vec2::ZERO, Vec2::X);
        assert!(!ent.alive);
        assert!(ent.dissolution.active);
    }

    #[test]
    fn test_dissolution_completes() {
        let mut ent = LayeredEntity::new("test", Vec3::ZERO, 100.0);
        ent.take_damage(100.0, Vec2::ZERO, Vec2::X);
        for _ in 0..300 { ent.update(1.0 / 60.0); }
        assert!(ent.is_dissolved());
    }

    #[test]
    fn test_builder_mage() {
        let mage = LayeredEntityBuilder::mage(Vec3::ZERO);
        assert!(mage.density_layer.is_some());
        assert!(mage.curve_layer.is_some());
        assert!(!mage.glyph_layer.is_empty());
        assert_eq!(mage.hp, 100.0);
    }

    #[test]
    fn test_builder_boss() {
        let boss = LayeredEntityBuilder::boss(Vec3::ZERO);
        assert!(boss.density_layer.is_some());
        assert!(boss.curve_layer.is_some());
        assert!(boss.glyph_layer.len() >= 5);
        assert_eq!(boss.hp, 500.0);
    }
}
