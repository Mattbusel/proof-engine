//! Material Painter — click-to-assign material tags on SDF surfaces.
//!
//! # Overview
//!
//! The material painter works in SDF space.  The user moves a spherical brush
//! over the rendered surface; at each frame the painter samples the SDF
//! gradient to find the surface normal at the brush centre, then stamps a
//! `MaterialPatch` into the patch list.  Patches are stored in paint order;
//! the last patch that covers a given surface point wins.
//!
//! # Material tags
//!
//! Tags mirror the eight MaterialTag variants used in apotheosis.rs.  Each tag
//! carries full PBR parameters: base colour, metallic, roughness, Fresnel
//! strength (F0), SSS intensity, emission colour and emission strength.
//!
//! # Real-time preview
//!
//! `MaterialPainter::classify` returns the best-matching `MaterialTag` for any
//! world-space point.  The rendering pipeline calls this once per particle to
//! tint its output colour.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// MaterialTag
// ─────────────────────────────────────────────────────────────────────────────

/// Matches the eight material classifications used in the SDF renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MaterialTag {
    Skin,
    Hair,
    Jacket,
    Boot,
    Metal,
    Eye,
    Cloth,
    Emissive,
    Glass,
    Ceramic,
    Rubber,
    Wood,
    Stone,
    Custom(u16),
}

impl MaterialTag {
    pub fn label(self) -> &'static str {
        match self {
            MaterialTag::Skin     => "Skin",
            MaterialTag::Hair     => "Hair",
            MaterialTag::Jacket   => "Jacket",
            MaterialTag::Boot     => "Boot",
            MaterialTag::Metal    => "Metal",
            MaterialTag::Eye      => "Eye",
            MaterialTag::Cloth    => "Cloth",
            MaterialTag::Emissive => "Emissive",
            MaterialTag::Glass    => "Glass",
            MaterialTag::Ceramic  => "Ceramic",
            MaterialTag::Rubber   => "Rubber",
            MaterialTag::Wood     => "Wood",
            MaterialTag::Stone    => "Stone",
            MaterialTag::Custom(_)=> "Custom",
        }
    }

    pub fn all() -> &'static [MaterialTag] {
        &[
            MaterialTag::Skin, MaterialTag::Hair, MaterialTag::Jacket,
            MaterialTag::Boot, MaterialTag::Metal, MaterialTag::Eye,
            MaterialTag::Cloth, MaterialTag::Emissive, MaterialTag::Glass,
            MaterialTag::Ceramic, MaterialTag::Rubber, MaterialTag::Wood,
            MaterialTag::Stone,
        ]
    }

    /// Default PBR parameters for this tag.
    pub fn default_params(self) -> MaterialParams {
        match self {
            MaterialTag::Skin => MaterialParams {
                base_color:       Vec4::new(0.85, 0.72, 0.60, 1.0),
                metallic:         0.0,
                roughness:        0.65,
                fresnel_f0:       0.028,
                sss_intensity:    0.6,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.42,
                anisotropy:       0.0,
                subsurface_color: Vec3::new(0.9, 0.4, 0.3),
            },
            MaterialTag::Hair => MaterialParams {
                base_color:       Vec4::new(0.15, 0.10, 0.07, 1.0),
                metallic:         0.0,
                roughness:        0.5,
                fresnel_f0:       0.046,
                sss_intensity:    0.1,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.55,
                anisotropy:       0.85,
                subsurface_color: Vec3::new(0.3, 0.2, 0.1),
            },
            MaterialTag::Jacket => MaterialParams {
                base_color:       Vec4::new(0.25, 0.25, 0.30, 1.0),
                metallic:         0.0,
                roughness:        0.8,
                fresnel_f0:       0.04,
                sss_intensity:    0.0,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.5,
                anisotropy:       0.0,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Boot => MaterialParams {
                base_color:       Vec4::new(0.10, 0.08, 0.07, 1.0),
                metallic:         0.0,
                roughness:        0.7,
                fresnel_f0:       0.05,
                sss_intensity:    0.0,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.5,
                anisotropy:       0.1,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Metal => MaterialParams {
                base_color:       Vec4::new(0.82, 0.82, 0.82, 1.0),
                metallic:         1.0,
                roughness:        0.2,
                fresnel_f0:       0.9,
                sss_intensity:    0.0,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              2.93,
                anisotropy:       0.0,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Eye => MaterialParams {
                base_color:       Vec4::new(0.3, 0.6, 0.9, 1.0),
                metallic:         0.0,
                roughness:        0.02,
                fresnel_f0:       0.04,
                sss_intensity:    0.15,
                emission_color:   Vec3::new(0.2, 0.4, 0.8),
                emission_strength:0.3,
                ior:              1.336,
                anisotropy:       0.0,
                subsurface_color: Vec3::new(0.2, 0.4, 0.8),
            },
            MaterialTag::Cloth => MaterialParams {
                base_color:       Vec4::new(0.5, 0.5, 0.5, 1.0),
                metallic:         0.0,
                roughness:        0.9,
                fresnel_f0:       0.04,
                sss_intensity:    0.0,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.5,
                anisotropy:       0.3,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Emissive => MaterialParams {
                base_color:       Vec4::new(0.2, 0.5, 1.0, 1.0),
                metallic:         0.0,
                roughness:        0.3,
                fresnel_f0:       0.04,
                sss_intensity:    0.0,
                emission_color:   Vec3::new(0.2, 0.5, 1.0),
                emission_strength:3.0,
                ior:              1.5,
                anisotropy:       0.0,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Glass => MaterialParams {
                base_color:       Vec4::new(0.9, 0.95, 1.0, 0.1),
                metallic:         0.0,
                roughness:        0.01,
                fresnel_f0:       0.04,
                sss_intensity:    0.0,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.52,
                anisotropy:       0.0,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Ceramic => MaterialParams {
                base_color:       Vec4::new(0.95, 0.93, 0.88, 1.0),
                metallic:         0.0,
                roughness:        0.15,
                fresnel_f0:       0.04,
                sss_intensity:    0.0,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.7,
                anisotropy:       0.0,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Rubber => MaterialParams {
                base_color:       Vec4::new(0.15, 0.15, 0.15, 1.0),
                metallic:         0.0,
                roughness:        0.95,
                fresnel_f0:       0.04,
                sss_intensity:    0.0,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.519,
                anisotropy:       0.0,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Wood => MaterialParams {
                base_color:       Vec4::new(0.55, 0.35, 0.18, 1.0),
                metallic:         0.0,
                roughness:        0.8,
                fresnel_f0:       0.04,
                sss_intensity:    0.05,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.5,
                anisotropy:       0.5,
                subsurface_color: Vec3::new(0.6, 0.4, 0.2),
            },
            MaterialTag::Stone => MaterialParams {
                base_color:       Vec4::new(0.5, 0.48, 0.45, 1.0),
                metallic:         0.0,
                roughness:        0.9,
                fresnel_f0:       0.04,
                sss_intensity:    0.0,
                emission_color:   Vec3::ZERO,
                emission_strength:0.0,
                ior:              1.6,
                anisotropy:       0.0,
                subsurface_color: Vec3::ZERO,
            },
            MaterialTag::Custom(_) => MaterialParams::default(),
        }
    }
}

impl Default for MaterialTag {
    fn default() -> Self { MaterialTag::Skin }
}

// ─────────────────────────────────────────────────────────────────────────────
// MaterialParams
// ─────────────────────────────────────────────────────────────────────────────

/// Full PBR parameter set for a material.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialParams {
    /// RGBA base colour (alpha = opacity for glass/transparent surfaces).
    pub base_color:       Vec4,
    /// 0 = dielectric, 1 = metal.
    pub metallic:         f32,
    /// Perceptual roughness [0, 1].
    pub roughness:        f32,
    /// Schlick F0 reflectance at normal incidence.
    pub fresnel_f0:       f32,
    /// Sub-surface scattering weight [0, 1].
    pub sss_intensity:    f32,
    /// Emission RGB (pre-tonemapped).
    pub emission_color:   Vec3,
    /// Emission multiplier (values > 1 bloom-clip).
    pub emission_strength:f32,
    /// Index of refraction (for glass, water, cornea).
    pub ior:              f32,
    /// GGX anisotropy [0, 1].
    pub anisotropy:       f32,
    /// Subsurface tint colour.
    pub subsurface_color: Vec3,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            base_color:        Vec4::new(0.8, 0.8, 0.8, 1.0),
            metallic:          0.0,
            roughness:         0.5,
            fresnel_f0:        0.04,
            sss_intensity:     0.0,
            emission_color:    Vec3::ZERO,
            emission_strength: 0.0,
            ior:               1.5,
            anisotropy:        0.0,
            subsurface_color:  Vec3::ZERO,
        }
    }
}

impl MaterialParams {
    /// Lerp between two parameter sets by weight t ∈ [0,1].
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let s = 1.0 - t;
        Self {
            base_color:        self.base_color * s + other.base_color * t,
            metallic:          self.metallic * s + other.metallic * t,
            roughness:         self.roughness * s + other.roughness * t,
            fresnel_f0:        self.fresnel_f0 * s + other.fresnel_f0 * t,
            sss_intensity:     self.sss_intensity * s + other.sss_intensity * t,
            emission_color:    self.emission_color * s + other.emission_color * t,
            emission_strength: self.emission_strength * s + other.emission_strength * t,
            ior:               self.ior * s + other.ior * t,
            anisotropy:        self.anisotropy * s + other.anisotropy * t,
            subsurface_color:  self.subsurface_color * s + other.subsurface_color * t,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MaterialPreset
// ─────────────────────────────────────────────────────────────────────────────

/// A named material preset stored in the asset library.
#[derive(Debug, Clone)]
pub struct MaterialPreset {
    pub name:   String,
    pub tag:    MaterialTag,
    pub params: MaterialParams,
}

impl MaterialPreset {
    pub fn new(name: impl Into<String>, tag: MaterialTag) -> Self {
        Self {
            name: name.into(),
            params: tag.default_params(),
            tag,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MaterialPatch
// ─────────────────────────────────────────────────────────────────────────────

/// A single paint stroke on the SDF surface.
///
/// A patch is a sphere in SDF object space.  Any surface point within
/// `radius` of `center` receives `tag` and `params`.
#[derive(Debug, Clone)]
pub struct MaterialPatch {
    pub id:       u32,
    pub center:   Vec3,
    pub radius:   f32,
    pub tag:      MaterialTag,
    pub params:   MaterialParams,
    pub opacity:  f32,
    pub falloff:  PatchFalloff,
}

/// How patch influence fades toward the edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatchFalloff {
    /// Hard edge: full influence up to `radius`, zero outside.
    Hard,
    /// Linear: full at centre, zero at `radius`.
    Linear,
    /// Smooth cubic fade (default).
    Smooth,
    /// Gaussian with sigma = radius / 3.
    Gaussian,
}

impl Default for PatchFalloff {
    fn default() -> Self { PatchFalloff::Smooth }
}

impl MaterialPatch {
    /// Returns the influence weight [0, 1] for a point at distance `d` from center.
    pub fn weight_at(&self, d: f32) -> f32 {
        if d >= self.radius { return 0.0; }
        let t = d / self.radius; // 0 at center, 1 at edge
        let w = match self.falloff {
            PatchFalloff::Hard     => 1.0,
            PatchFalloff::Linear   => 1.0 - t,
            PatchFalloff::Smooth   => { let t2 = t * t; 1.0 - t2 * (3.0 - 2.0 * t) },
            PatchFalloff::Gaussian => {
                let sigma = self.radius / 3.0;
                (-0.5 * (d / sigma).powi(2)).exp()
            }
        };
        w * self.opacity
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Brush
// ─────────────────────────────────────────────────────────────────────────────

/// The active brush state while painting.
#[derive(Debug, Clone)]
pub struct Brush {
    pub tag:      MaterialTag,
    pub params:   MaterialParams,
    pub radius:   f32,
    pub opacity:  f32,
    pub falloff:  PatchFalloff,
    pub painting: bool,
    /// World-space cursor position on the SDF surface.
    pub cursor:   Vec3,
    /// Surface normal at cursor (for orientation).
    pub normal:   Vec3,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            tag:      MaterialTag::Skin,
            params:   MaterialTag::Skin.default_params(),
            radius:   0.08,
            opacity:  1.0,
            falloff:  PatchFalloff::Smooth,
            painting: false,
            cursor:   Vec3::ZERO,
            normal:   Vec3::Y,
        }
    }
}

impl Brush {
    pub fn set_tag(&mut self, tag: MaterialTag) {
        self.tag = tag;
        self.params = tag.default_params();
    }

    pub fn radius_step_up(&mut self) { self.radius = (self.radius * 1.2).min(2.0); }
    pub fn radius_step_down(&mut self) { self.radius = (self.radius / 1.2).max(0.005); }
    pub fn opacity_step_up(&mut self) { self.opacity = (self.opacity + 0.05).min(1.0); }
    pub fn opacity_step_down(&mut self) { self.opacity = (self.opacity - 0.05).max(0.01); }
}

// ─────────────────────────────────────────────────────────────────────────────
// PaintLayer
// ─────────────────────────────────────────────────────────────────────────────

/// A named layer of paint patches — like Photoshop layers.
#[derive(Debug, Clone)]
pub struct PaintLayer {
    pub name:    String,
    pub visible: bool,
    pub locked:  bool,
    pub opacity: f32,
    patches:     Vec<MaterialPatch>,
    next_patch:  u32,
}

impl PaintLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name:       name.into(),
            visible:    true,
            locked:     false,
            opacity:    1.0,
            patches:    Vec::new(),
            next_patch: 1,
        }
    }

    pub fn add_patch(&mut self, center: Vec3, radius: f32, tag: MaterialTag,
                     params: MaterialParams, opacity: f32, falloff: PatchFalloff) -> u32 {
        let id = self.next_patch;
        self.next_patch += 1;
        self.patches.push(MaterialPatch { id, center, radius, tag, params, opacity, falloff });
        id
    }

    pub fn remove_patch(&mut self, id: u32) {
        self.patches.retain(|p| p.id != id);
    }

    pub fn patches(&self) -> &[MaterialPatch] { &self.patches }
    pub fn patch_count(&self) -> usize { self.patches.len() }

    /// Classify a point, returning the top-most (last) patch that covers it.
    pub fn classify(&self, p: Vec3) -> Option<(&MaterialPatch, f32)> {
        if !self.visible { return None; }
        let mut best: Option<(&MaterialPatch, f32)> = None;
        for patch in self.patches.iter().rev() {
            let d = (p - patch.center).length();
            let w = patch.weight_at(d) * self.opacity;
            if w > 0.0 {
                best = Some((patch, w));
                break; // last patch wins
            }
        }
        best
    }

    /// Erase patches whose center is within `radius` of `p`.
    pub fn erase_at(&mut self, p: Vec3, radius: f32) {
        self.patches.retain(|patch| (patch.center - p).length() > radius);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MaterialPainter
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level material painter state.
#[derive(Debug)]
pub struct MaterialPainter {
    pub layers:       Vec<PaintLayer>,
    pub active_layer: usize,
    pub brush:        Brush,
    /// Preset library — available materials for quick selection.
    pub presets:      Vec<MaterialPreset>,
    /// Whether to show the layer panel in the UI.
    pub show_layers:  bool,
    /// Whether to show the material param panel in the UI.
    pub show_params:  bool,
    undo_stack:       Vec<PaintUndoEntry>,
    redo_stack:       Vec<PaintUndoEntry>,
    /// Custom colour palette (recent colours).
    pub recent_colors: Vec<Vec4>,
}

#[derive(Debug, Clone)]
enum PaintUndoEntry {
    AddPatch  { layer: usize, patch_id: u32 },
    Erase     { layer: usize, erased: Vec<MaterialPatch> },
    BatchPaint{ layer: usize, patch_ids: Vec<u32> },
}

impl MaterialPainter {
    pub fn new() -> Self {
        let mut presets = Vec::new();
        for &tag in MaterialTag::all() {
            presets.push(MaterialPreset::new(tag.label(), tag));
        }
        let mut layers = vec![PaintLayer::new("Base")];
        layers.push(PaintLayer::new("Details"));
        layers.push(PaintLayer::new("Emissive"));

        Self {
            layers,
            active_layer: 0,
            brush: Brush::default(),
            presets,
            show_layers: true,
            show_params: true,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            recent_colors: Vec::new(),
        }
    }

    // ── Layer management ──────────────────────────────────────────────────

    pub fn add_layer(&mut self, name: impl Into<String>) -> usize {
        self.layers.push(PaintLayer::new(name));
        self.layers.len() - 1
    }

    pub fn remove_layer(&mut self, idx: usize) {
        if self.layers.len() > 1 && idx < self.layers.len() {
            self.layers.remove(idx);
            if self.active_layer >= self.layers.len() {
                self.active_layer = self.layers.len() - 1;
            }
        }
    }

    pub fn move_layer_up(&mut self, idx: usize) {
        if idx > 0 && idx < self.layers.len() {
            self.layers.swap(idx, idx - 1);
        }
    }

    pub fn move_layer_down(&mut self, idx: usize) {
        if idx + 1 < self.layers.len() {
            self.layers.swap(idx, idx + 1);
        }
    }

    pub fn active_layer(&self) -> &PaintLayer { &self.layers[self.active_layer] }
    pub fn active_layer_mut(&mut self) -> &mut PaintLayer { &mut self.layers[self.active_layer] }

    // ── Painting ──────────────────────────────────────────────────────────

    /// Begin a paint stroke at `surface_p` (world space on the SDF surface).
    pub fn begin_stroke(&mut self, surface_p: Vec3) {
        self.brush.painting = true;
        self.brush.cursor = surface_p;
        self.paint_at(surface_p);
    }

    /// Continue a stroke as the cursor moves.
    pub fn continue_stroke(&mut self, surface_p: Vec3) {
        if !self.brush.painting { return; }
        // Only stamp if cursor moved more than 40% of brush radius
        let delta = (surface_p - self.brush.cursor).length();
        if delta > self.brush.radius * 0.4 {
            self.brush.cursor = surface_p;
            self.paint_at(surface_p);
        }
    }

    /// End the current stroke.
    pub fn end_stroke(&mut self) {
        self.brush.painting = false;
    }

    fn paint_at(&mut self, p: Vec3) {
        if self.layers[self.active_layer].locked { return; }
        let id = self.layers[self.active_layer].add_patch(
            p,
            self.brush.radius,
            self.brush.tag,
            self.brush.params.clone(),
            self.brush.opacity,
            self.brush.falloff,
        );
        self.undo_stack.push(PaintUndoEntry::AddPatch { layer: self.active_layer, patch_id: id });
        self.redo_stack.clear();
        // Track recent colour
        let c = self.brush.params.base_color;
        self.recent_colors.retain(|&rc| rc != c);
        self.recent_colors.insert(0, c);
        if self.recent_colors.len() > 16 { self.recent_colors.truncate(16); }
    }

    /// Erase all patches on the active layer within `radius` of `p`.
    pub fn erase_at(&mut self, p: Vec3, radius: f32) {
        if self.layers[self.active_layer].locked { return; }
        let erased: Vec<_> = self.layers[self.active_layer]
            .patches().iter()
            .filter(|patch| (patch.center - p).length() <= radius)
            .cloned()
            .collect();
        if !erased.is_empty() {
            self.undo_stack.push(PaintUndoEntry::Erase { layer: self.active_layer, erased });
            self.redo_stack.clear();
            self.layers[self.active_layer].erase_at(p, radius);
        }
    }

    // ── Classification ────────────────────────────────────────────────────

    /// Return the best material tag and params for a world-space point.
    /// Layers are composited in order (last visible layer covering the point wins).
    pub fn classify(&self, p: Vec3) -> Option<(MaterialTag, &MaterialParams)> {
        for layer in self.layers.iter().rev() {
            if let Some((patch, _w)) = layer.classify(p) {
                return Some((patch.tag, &patch.params));
            }
        }
        None
    }

    /// Return the default material if no patch covers `p`.
    pub fn classify_or_default(&self, p: Vec3) -> (MaterialTag, MaterialParams) {
        self.classify(p)
            .map(|(tag, params)| (tag, params.clone()))
            .unwrap_or_else(|| {
                let tag = MaterialTag::Skin;
                (tag, tag.default_params())
            })
    }

    // ── Undo / redo ───────────────────────────────────────────────────────

    pub fn undo(&mut self) {
        let Some(entry) = self.undo_stack.pop() else { return; };
        match &entry {
            PaintUndoEntry::AddPatch { layer, patch_id } => {
                if let Some(l) = self.layers.get_mut(*layer) {
                    l.remove_patch(*patch_id);
                }
            }
            PaintUndoEntry::Erase { layer, erased } => {
                if let Some(l) = self.layers.get_mut(*layer) {
                    for patch in erased {
                        l.patches.push(patch.clone());
                    }
                }
            }
            PaintUndoEntry::BatchPaint { layer, patch_ids } => {
                if let Some(l) = self.layers.get_mut(*layer) {
                    for &id in patch_ids {
                        l.remove_patch(id);
                    }
                }
            }
        }
        self.redo_stack.push(entry);
    }

    pub fn redo(&mut self) {
        // Simplified: just mark dirty.
        if let Some(e) = self.redo_stack.pop() {
            self.undo_stack.push(e);
        }
    }

    // ── Preset management ─────────────────────────────────────────────────

    pub fn save_preset(&mut self, name: impl Into<String>) {
        let preset = MaterialPreset {
            name: name.into(),
            tag: self.brush.tag,
            params: self.brush.params.clone(),
        };
        self.presets.push(preset);
    }

    pub fn apply_preset(&mut self, idx: usize) {
        if let Some(preset) = self.presets.get(idx) {
            self.brush.tag = preset.tag;
            self.brush.params = preset.params.clone();
        }
    }

    // ── Stats ─────────────────────────────────────────────────────────────

    pub fn total_patches(&self) -> usize {
        self.layers.iter().map(|l| l.patch_count()).sum()
    }

    pub fn status_line(&self) -> String {
        let al = &self.layers[self.active_layer];
        format!(
            "Material Painter — layer '{}' ({} patches) | brush {} r={:.3} op={:.2} | {} total patches",
            al.name, al.patch_count(),
            self.brush.tag.label(), self.brush.radius, self.brush.opacity,
            self.total_patches()
        )
    }
}

impl Default for MaterialPainter {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// MaterialLibrary
// ─────────────────────────────────────────────────────────────────────────────

/// A collection of named materials that can be referenced from node graphs.
#[derive(Debug, Default)]
pub struct MaterialLibrary {
    pub materials: HashMap<String, MaterialParams>,
    pub tags:      HashMap<String, MaterialTag>,
}

impl MaterialLibrary {
    pub fn new() -> Self {
        let mut lib = Self::default();
        for &tag in MaterialTag::all() {
            lib.materials.insert(tag.label().to_string(), tag.default_params());
            lib.tags.insert(tag.label().to_string(), tag);
        }
        lib
    }

    pub fn insert(&mut self, name: impl Into<String>, tag: MaterialTag, params: MaterialParams) {
        let n = name.into();
        self.materials.insert(n.clone(), params);
        self.tags.insert(n, tag);
    }

    pub fn get(&self, name: &str) -> Option<(&MaterialTag, &MaterialParams)> {
        let tag = self.tags.get(name)?;
        let params = self.materials.get(name)?;
        Some((tag, params))
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.materials.keys().map(String::as_str).collect();
        names.sort();
        names
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_weight_hard() {
        let p = MaterialPatch {
            id: 1, center: Vec3::ZERO, radius: 1.0,
            tag: MaterialTag::Skin, params: MaterialParams::default(),
            opacity: 1.0, falloff: PatchFalloff::Hard,
        };
        assert!((p.weight_at(0.5) - 1.0).abs() < 1e-5);
        assert!((p.weight_at(1.5)).abs() < 1e-5);
    }

    #[test]
    fn patch_weight_smooth() {
        let p = MaterialPatch {
            id: 1, center: Vec3::ZERO, radius: 1.0,
            tag: MaterialTag::Skin, params: MaterialParams::default(),
            opacity: 1.0, falloff: PatchFalloff::Smooth,
        };
        assert!((p.weight_at(0.0) - 1.0).abs() < 1e-5);
        assert!((p.weight_at(1.0)).abs() < 1e-5);
        assert!(p.weight_at(0.5) > 0.0 && p.weight_at(0.5) < 1.0);
    }

    #[test]
    fn classify_last_patch_wins() {
        let mut layer = PaintLayer::new("test");
        layer.add_patch(Vec3::ZERO, 1.0, MaterialTag::Skin, MaterialParams::default(), 1.0, PatchFalloff::Hard);
        layer.add_patch(Vec3::ZERO, 1.0, MaterialTag::Metal, MaterialParams::default(), 1.0, PatchFalloff::Hard);
        let result = layer.classify(Vec3::ZERO);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0.tag, MaterialTag::Metal);
    }

    #[test]
    fn painter_undo() {
        let mut painter = MaterialPainter::new();
        painter.begin_stroke(Vec3::ZERO);
        painter.end_stroke();
        assert_eq!(painter.active_layer().patch_count(), 1);
        painter.undo();
        assert_eq!(painter.active_layer().patch_count(), 0);
    }

    #[test]
    fn material_lerp() {
        let a = MaterialTag::Metal.default_params();
        let b = MaterialTag::Skin.default_params();
        let mid = a.lerp(&b, 0.5);
        let expected_rough = (a.roughness + b.roughness) * 0.5;
        assert!((mid.roughness - expected_rough).abs() < 1e-5);
    }

    #[test]
    fn library_all_tags() {
        let lib = MaterialLibrary::new();
        for &tag in MaterialTag::all() {
            assert!(lib.get(tag.label()).is_some());
        }
    }
}
