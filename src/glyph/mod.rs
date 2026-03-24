//! Glyph — the fundamental rendering primitive.
//!
//! Everything in Proof Engine is a Glyph: a single character rendered as a
//! textured quad in 3D space with position, mass, charge, temperature, and entropy.

pub mod batch;
pub mod atlas;

use glam::{Vec2, Vec3, Vec4};
use crate::math::MathFunction;

/// Opaque handle to a Glyph in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphId(pub u32);

/// Which render pass this glyph belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderLayer {
    Background,  // chaos field — rendered first, no depth write
    World,       // environment elements
    Entity,      // characters, enemies, bosses
    Particle,    // particles (additive blend usually)
    UI,          // UI elements (rendered without perspective)
    Overlay,     // post-processing overlays (HUD flash, vignette)
}

/// How this glyph blends with the framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,    // standard alpha blend
    Additive,  // add color to destination (for glow/particles)
    Multiply,  // multiply with destination (for shadows/tints)
    Screen,    // 1 - (1-src)(1-dst) (for bloom-like effects)
}

/// A single character rendered as a textured quad in 3D space.
#[derive(Clone, Debug)]
pub struct Glyph {
    // ── Identity ─────────────────────────────────────────────────────────────
    pub character: char,

    // ── Spatial ──────────────────────────────────────────────────────────────
    pub position: Vec3,
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub rotation: f32,       // radians, around the facing axis
    pub scale: Vec2,

    // ── Visual ───────────────────────────────────────────────────────────────
    pub color: Vec4,         // RGBA
    pub emission: f32,       // how much light this glyph emits (0 = none, 1+ = glows)
    pub glow_color: Vec3,    // color of emitted light
    pub glow_radius: f32,    // how far the glow reaches in world units

    // ── Mathematical properties (physically simulated) ───────────────────────
    pub mass: f32,           // affects gravitational pull
    pub charge: f32,         // EM interaction: + attracts -, like repels like
    pub temperature: f32,    // affects color shift and jitter amount
    pub entropy: f32,        // 0 = predictable, 1 = chaotic behavior

    // ── Animation ────────────────────────────────────────────────────────────
    pub life_function: Option<MathFunction>,  // drives this glyph's position/color
    pub age: f32,                             // seconds since creation
    pub lifetime: f32,                        // total lifespan (-1 = infinite)

    // ── Rendering ────────────────────────────────────────────────────────────
    pub layer: RenderLayer,
    pub blend_mode: BlendMode,
    pub visible: bool,
}

impl Default for Glyph {
    fn default() -> Self {
        Self {
            character: ' ',
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            rotation: 0.0,
            scale: Vec2::ONE,
            color: Vec4::ONE,
            emission: 0.0,
            glow_color: Vec3::ONE,
            glow_radius: 0.0,
            mass: 1.0,
            charge: 0.0,
            temperature: 0.5,
            entropy: 0.0,
            life_function: None,
            age: 0.0,
            lifetime: -1.0,
            layer: RenderLayer::World,
            blend_mode: BlendMode::Normal,
            visible: true,
        }
    }
}

impl Glyph {
    /// Returns true if this glyph has expired and should be removed.
    pub fn is_expired(&self) -> bool {
        self.lifetime >= 0.0 && self.age >= self.lifetime
    }
}

/// Pre-allocated pool of Glyphs for efficient batch rendering.
#[allow(dead_code)]
pub struct GlyphPool {
    glyphs: Vec<Option<Glyph>>,
    free_slots: Vec<u32>,
    next_id: u32,
}

impl GlyphPool {
    pub fn new(capacity: usize) -> Self {
        Self {
            glyphs: vec![None; capacity],
            free_slots: (0..capacity as u32).rev().collect(),
            next_id: 0,
        }
    }

    pub fn spawn(&mut self, glyph: Glyph) -> GlyphId {
        if let Some(slot) = self.free_slots.pop() {
            self.glyphs[slot as usize] = Some(glyph);
            GlyphId(slot)
        } else {
            // Expand if full
            let id = self.glyphs.len() as u32;
            self.glyphs.push(Some(glyph));
            GlyphId(id)
        }
    }

    pub fn remove(&mut self, id: GlyphId) {
        if let Some(slot) = self.glyphs.get_mut(id.0 as usize) {
            *slot = None;
            self.free_slots.push(id.0);
        }
    }

    /// Alias for `remove` — despawn a glyph by ID.
    pub fn despawn(&mut self, id: GlyphId) {
        self.remove(id);
    }

    pub fn get(&self, id: GlyphId) -> Option<&Glyph> {
        self.glyphs.get(id.0 as usize)?.as_ref()
    }

    pub fn get_mut(&mut self, id: GlyphId) -> Option<&mut Glyph> {
        self.glyphs.get_mut(id.0 as usize)?.as_mut()
    }

    pub fn iter(&self) -> impl Iterator<Item = (GlyphId, &Glyph)> {
        self.glyphs.iter().enumerate().filter_map(|(i, g)| {
            g.as_ref().map(|g| (GlyphId(i as u32), g))
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (GlyphId, &mut Glyph)> {
        self.glyphs.iter_mut().enumerate().filter_map(|(i, g)| {
            g.as_mut().map(|g| (GlyphId(i as u32), g))
        })
    }

    /// Advance all glyphs by dt seconds. Remove expired ones.
    pub fn tick(&mut self, dt: f32) {
        for slot in self.glyphs.iter_mut() {
            if let Some(ref mut g) = slot {
                g.age += dt;
                // Apply velocity
                g.position += g.velocity * dt;
                g.velocity += g.acceleration * dt;
                // Expire check handled externally via is_expired()
            }
        }
        // Remove expired
        let mut to_remove = Vec::new();
        for (i, slot) in self.glyphs.iter().enumerate() {
            if let Some(ref g) = slot {
                if g.is_expired() {
                    to_remove.push(i as u32);
                }
            }
        }
        for id in to_remove {
            self.glyphs[id as usize] = None;
            self.free_slots.push(id);
        }
    }
}
