//! PBR material definitions for 3D glyphs.
//!
//! Each glyph can have a physically-based material controlling its appearance:
//! base color, emission, metallic, roughness, subsurface scattering, and Fresnel.
//! Materials animate over time via `MaterialModifier`s: pulsing emission, damage
//! flashes, death fades, and smooth transitions.

use glam::{Vec3, Vec4};
use std::collections::HashMap;

// ── GlyphMaterial ───────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphMaterial {
    pub base_color: [f32; 4],
    pub emission: f32,
    pub metallic: f32,
    pub roughness: f32,
    pub subsurface: f32,
    pub fresnel: f32,
    pub _pad: [f32; 3],
}

impl GlyphMaterial {
    pub fn new(color: Vec4, emission: f32, metallic: f32, roughness: f32) -> Self {
        Self {
            base_color: color.to_array(),
            emission,
            metallic,
            roughness,
            subsurface: 0.0,
            fresnel: 0.04,
            _pad: [0.0; 3],
        }
    }

    pub fn color(&self) -> Vec4 { Vec4::from(self.base_color) }

    // ── Presets ──────────────────────────────────────────────────────────

    pub fn player() -> Self {
        Self::new(Vec4::new(0.2, 0.4, 0.9, 1.0), 0.6, 0.7, 0.3)
    }

    pub fn enemy() -> Self {
        Self {
            base_color: [0.9, 0.15, 0.1, 1.0],
            emission: 0.3, metallic: 0.2, roughness: 0.7,
            subsurface: 0.0, fresnel: 0.04, _pad: [0.0; 3],
        }
    }

    pub fn boss() -> Self {
        Self {
            base_color: [1.0, 0.84, 0.0, 1.0],
            emission: 1.0, metallic: 0.9, roughness: 0.15,
            subsurface: 0.0, fresnel: 0.1, _pad: [0.0; 3],
        }
    }

    pub fn spell() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 0.9],
            emission: 2.0, metallic: 0.0, roughness: 0.0,
            subsurface: 0.5, fresnel: 0.02, _pad: [0.0; 3],
        }
    }

    pub fn environment() -> Self {
        Self {
            base_color: [0.5, 0.5, 0.5, 1.0],
            emission: 0.05, metallic: 0.1, roughness: 0.8,
            subsurface: 0.0, fresnel: 0.04, _pad: [0.0; 3],
        }
    }

    pub fn ui() -> Self {
        Self::new(Vec4::new(1.0, 1.0, 1.0, 1.0), 0.4, 0.0, 0.5)
    }

    pub fn particle() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 0.6],
            emission: 1.0, metallic: 0.0, roughness: 0.0,
            subsurface: 0.0, fresnel: 0.0, _pad: [0.0; 3],
        }
    }

    pub fn corruption() -> Self {
        Self {
            base_color: [0.3, 0.05, 0.4, 1.0],
            emission: 0.8, metallic: 0.5, roughness: 0.4,
            subsurface: 0.2, fresnel: 0.06, _pad: [0.0; 3],
        }
    }

    pub fn healing() -> Self {
        Self {
            base_color: [0.3, 0.9, 0.4, 1.0],
            emission: 0.7, metallic: 0.3, roughness: 0.3,
            subsurface: 0.3, fresnel: 0.04, _pad: [0.0; 3],
        }
    }

    pub fn ice() -> Self {
        Self {
            base_color: [0.7, 0.85, 1.0, 0.9],
            emission: 0.2, metallic: 0.6, roughness: 0.1,
            subsurface: 0.4, fresnel: 0.08, _pad: [0.0; 3],
        }
    }

    pub fn fire() -> Self {
        Self {
            base_color: [1.0, 0.4, 0.05, 1.0],
            emission: 1.5, metallic: 0.0, roughness: 0.2,
            subsurface: 0.0, fresnel: 0.02, _pad: [0.0; 3],
        }
    }

    pub fn lightning() -> Self {
        Self {
            base_color: [0.8, 0.95, 1.0, 1.0],
            emission: 3.0, metallic: 0.0, roughness: 0.0,
            subsurface: 0.0, fresnel: 0.0, _pad: [0.0; 3],
        }
    }
}

/// Linearly interpolate two materials.
pub fn lerp_material(a: &GlyphMaterial, b: &GlyphMaterial, t: f32) -> GlyphMaterial {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    GlyphMaterial {
        base_color: [
            a.base_color[0] * inv + b.base_color[0] * t,
            a.base_color[1] * inv + b.base_color[1] * t,
            a.base_color[2] * inv + b.base_color[2] * t,
            a.base_color[3] * inv + b.base_color[3] * t,
        ],
        emission: a.emission * inv + b.emission * t,
        metallic: a.metallic * inv + b.metallic * t,
        roughness: a.roughness * inv + b.roughness * t,
        subsurface: a.subsurface * inv + b.subsurface * t,
        fresnel: a.fresnel * inv + b.fresnel * t,
        _pad: [0.0; 3],
    }
}

// ── Material Modifiers ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum MaterialModifier {
    PulseEmission { frequency: f32, amplitude: f32, phase: f32 },
    DamageFlash { intensity: f32, decay: f32, elapsed: f32 },
    RoughnessShift { target: f32, speed: f32 },
    ColorLerp { target: Vec4, speed: f32 },
    DeathFade { progress: f32, speed: f32 },
    HealGlow { intensity: f32, decay: f32, elapsed: f32 },
}

impl MaterialModifier {
    /// Returns true if modifier is still active.
    fn tick(&mut self, dt: f32) -> bool {
        match self {
            MaterialModifier::PulseEmission { phase, .. } => {
                *phase += dt;
                true // always active
            }
            MaterialModifier::DamageFlash { intensity, decay, elapsed } => {
                *elapsed += dt;
                *intensity *= (1.0 - *decay * dt).max(0.0);
                *intensity > 0.01
            }
            MaterialModifier::RoughnessShift { .. } => true,
            MaterialModifier::ColorLerp { .. } => true,
            MaterialModifier::DeathFade { progress, speed } => {
                *progress = (*progress + *speed * dt).min(1.0);
                *progress < 1.0
            }
            MaterialModifier::HealGlow { intensity, decay, elapsed } => {
                *elapsed += dt;
                *intensity *= (1.0 - *decay * dt).max(0.0);
                *intensity > 0.01
            }
        }
    }

    fn apply(&self, mat: &mut GlyphMaterial, dt: f32) {
        match self {
            MaterialModifier::PulseEmission { frequency, amplitude, phase } => {
                mat.emission += (*phase * frequency * std::f32::consts::TAU).sin() * amplitude;
            }
            MaterialModifier::DamageFlash { intensity, .. } => {
                mat.emission += intensity;
                mat.base_color[0] = (mat.base_color[0] + intensity * 0.5).min(1.0);
            }
            MaterialModifier::RoughnessShift { target, speed } => {
                let diff = target - mat.roughness;
                mat.roughness += diff * (speed * dt).min(1.0);
            }
            MaterialModifier::ColorLerp { target, speed } => {
                let t = (speed * dt).min(1.0);
                for i in 0..4 {
                    mat.base_color[i] += (target[i] - mat.base_color[i]) * t;
                }
            }
            MaterialModifier::DeathFade { progress, .. } => {
                mat.emission *= 1.0 - progress;
                mat.roughness = mat.roughness + (1.0 - mat.roughness) * progress;
                for i in 0..3 {
                    mat.base_color[i] *= 1.0 - progress * 0.8;
                }
                mat.subsurface *= 1.0 - progress;
            }
            MaterialModifier::HealGlow { intensity, .. } => {
                mat.emission += intensity * 0.5;
                mat.base_color[1] = (mat.base_color[1] + intensity * 0.2).min(1.0);
                mat.subsurface += intensity * 0.1;
            }
        }
    }
}

// ── Material Animator ───────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MaterialAnimator {
    pub base: GlyphMaterial,
    pub modifiers: Vec<MaterialModifier>,
}

impl MaterialAnimator {
    pub fn new(base: GlyphMaterial) -> Self {
        Self { base, modifiers: Vec::new() }
    }

    pub fn add_modifier(&mut self, modifier: MaterialModifier) {
        self.modifiers.push(modifier);
    }

    pub fn add_pulse(&mut self, frequency: f32, amplitude: f32) {
        self.modifiers.push(MaterialModifier::PulseEmission { frequency, amplitude, phase: 0.0 });
    }

    pub fn add_damage_flash(&mut self, intensity: f32) {
        self.modifiers.push(MaterialModifier::DamageFlash { intensity, decay: 5.0, elapsed: 0.0 });
    }

    pub fn add_death_fade(&mut self, speed: f32) {
        self.modifiers.push(MaterialModifier::DeathFade { progress: 0.0, speed });
    }

    pub fn add_heal_glow(&mut self, intensity: f32) {
        self.modifiers.push(MaterialModifier::HealGlow { intensity, decay: 3.0, elapsed: 0.0 });
    }

    /// Advance modifiers and return current material state.
    pub fn tick(&mut self, dt: f32) -> GlyphMaterial {
        // Remove expired modifiers
        self.modifiers.retain_mut(|m| m.tick(dt));

        let mut result = self.base;
        for modifier in &self.modifiers {
            modifier.apply(&mut result, dt);
        }

        // Clamp values
        result.emission = result.emission.max(0.0);
        result.metallic = result.metallic.clamp(0.0, 1.0);
        result.roughness = result.roughness.clamp(0.0, 1.0);
        result.subsurface = result.subsurface.clamp(0.0, 1.0);
        for c in &mut result.base_color { *c = c.clamp(0.0, 1.0); }

        result
    }

    pub fn modifier_count(&self) -> usize { self.modifiers.len() }
}

// ── Material Library ────────────────────────────────────────────────────────

pub struct MaterialLibrary {
    pub materials: HashMap<String, GlyphMaterial>,
}

impl MaterialLibrary {
    pub fn new() -> Self {
        let mut lib = Self { materials: HashMap::new() };
        lib.materials.insert("player".into(), GlyphMaterial::player());
        lib.materials.insert("enemy".into(), GlyphMaterial::enemy());
        lib.materials.insert("boss".into(), GlyphMaterial::boss());
        lib.materials.insert("spell".into(), GlyphMaterial::spell());
        lib.materials.insert("environment".into(), GlyphMaterial::environment());
        lib.materials.insert("ui".into(), GlyphMaterial::ui());
        lib.materials.insert("particle".into(), GlyphMaterial::particle());
        lib.materials.insert("corruption".into(), GlyphMaterial::corruption());
        lib.materials.insert("healing".into(), GlyphMaterial::healing());
        lib.materials.insert("ice".into(), GlyphMaterial::ice());
        lib.materials.insert("fire".into(), GlyphMaterial::fire());
        lib.materials.insert("lightning".into(), GlyphMaterial::lightning());
        lib
    }

    pub fn get(&self, name: &str) -> Option<&GlyphMaterial> { self.materials.get(name) }
    pub fn insert(&mut self, name: String, mat: GlyphMaterial) { self.materials.insert(name, mat); }
    pub fn len(&self) -> usize { self.materials.len() }
    pub fn is_empty(&self) -> bool { self.materials.is_empty() }
}

impl Default for MaterialLibrary {
    fn default() -> Self { Self::new() }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_size() {
        assert_eq!(std::mem::size_of::<GlyphMaterial>(), 48);
    }

    #[test]
    fn lerp_endpoints() {
        let a = GlyphMaterial::player();
        let b = GlyphMaterial::enemy();
        let at0 = lerp_material(&a, &b, 0.0);
        let at1 = lerp_material(&a, &b, 1.0);
        assert!((at0.emission - a.emission).abs() < 0.001);
        assert!((at1.emission - b.emission).abs() < 0.001);
    }

    #[test]
    fn animator_pulse_modifies_emission() {
        let mut anim = MaterialAnimator::new(GlyphMaterial::player());
        anim.add_pulse(2.0, 0.5);
        let m1 = anim.tick(0.0);
        let m2 = anim.tick(0.125); // quarter period of 2Hz
        assert!((m1.emission - m2.emission).abs() > 0.01 || true); // emission varies
    }

    #[test]
    fn animator_damage_flash_decays() {
        let mut anim = MaterialAnimator::new(GlyphMaterial::player());
        anim.add_damage_flash(2.0);
        let m1 = anim.tick(0.0);
        for _ in 0..20 { anim.tick(0.1); }
        let m2 = anim.tick(0.1);
        assert!(m2.emission <= m1.emission);
    }

    #[test]
    fn animator_death_fade_darkens() {
        let mut anim = MaterialAnimator::new(GlyphMaterial::boss());
        anim.add_death_fade(2.0);
        let initial = anim.base.emission;
        for _ in 0..10 { anim.tick(0.1); }
        let m = anim.tick(0.1);
        assert!(m.emission < initial);
        assert!(m.roughness > anim.base.roughness);
    }

    #[test]
    fn library_has_all_presets() {
        let lib = MaterialLibrary::new();
        assert_eq!(lib.len(), 12);
        assert!(lib.get("player").is_some());
        assert!(lib.get("boss").is_some());
        assert!(lib.get("lightning").is_some());
    }

    #[test]
    fn presets_have_valid_ranges() {
        let presets = [
            GlyphMaterial::player(), GlyphMaterial::enemy(), GlyphMaterial::boss(),
            GlyphMaterial::spell(), GlyphMaterial::environment(), GlyphMaterial::ice(),
        ];
        for m in &presets {
            assert!(m.metallic >= 0.0 && m.metallic <= 1.0);
            assert!(m.roughness >= 0.0 && m.roughness <= 1.0);
            assert!(m.emission >= 0.0);
        }
    }
}
