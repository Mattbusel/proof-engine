// viewport.rs — Real-time editor viewport: wires EditorCamera, scene entities,
// gizmos, grid/axes overlays, and the GPU Pipeline into a single render loop.

use crate::editor::camera_controller::{EditorCamera, SnapView};
use crate::editor::gizmos::GizmoMode;
use crate::editor::perf_overlay::PerfOverlay;

use glam::{Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ─── Viewport dimensions ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct ViewportRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ViewportRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn aspect(&self) -> f32 {
        if self.height > 0.0 { self.width / self.height } else { 1.0 }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width
            && py >= self.y && py < self.y + self.height
    }

    pub fn to_ndc(&self, px: f32, py: f32) -> Vec2 {
        Vec2::new(
            (px - self.x) / self.width  * 2.0 - 1.0,
            1.0 - (py - self.y) / self.height * 2.0,
        )
    }
}

// ─── Scene entity representation for viewport ───────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    SdfBody,
    ParticleEmitter,
    DirectionalLight,
    PointLight,
    SpotLight,
    AreaLight,
    Camera,
    ForceField,
    BoneRoot,
    StaticMesh,
    Trigger,
    AudioEmitter,
    Decal,
    Probe,
    Marker,
}

impl EntityKind {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::SdfBody         => "[SDF]",
            Self::ParticleEmitter => "[FX]",
            Self::DirectionalLight => "[SUN]",
            Self::PointLight      => "[PT]",
            Self::SpotLight       => "[SPT]",
            Self::AreaLight       => "[AREA]",
            Self::Camera          => "[CAM]",
            Self::ForceField      => "[FIELD]",
            Self::BoneRoot        => "[BONE]",
            Self::StaticMesh      => "[MESH]",
            Self::Trigger         => "[TRIG]",
            Self::AudioEmitter    => "[SND]",
            Self::Decal           => "[DECAL]",
            Self::Probe           => "[PROBE]",
            Self::Marker          => "[MARK]",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ViewportEntity {
    pub id: EntityId,
    pub name: String,
    pub kind: EntityKind,
    pub position: Vec3,
    pub rotation: Vec3,   // Euler angles (degrees)
    pub scale: Vec3,
    pub visible: bool,
    pub locked: bool,
    pub parent: Option<EntityId>,
    pub children: Vec<EntityId>,
    pub sdf_graph_id: Option<u32>,
    pub material_tag: Option<String>,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub static_batching: bool,
    pub layer_mask: u32,
}

impl ViewportEntity {
    pub fn new(id: EntityId, name: String, kind: EntityKind) -> Self {
        Self {
            id,
            name,
            kind,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            visible: true,
            locked: false,
            parent: None,
            children: Vec::new(),
            sdf_graph_id: None,
            material_tag: None,
            cast_shadows: true,
            receive_shadows: true,
            static_batching: false,
            layer_mask: 0xFFFF_FFFF,
        }
    }

    pub fn model_matrix(&self) -> Mat4 {
        let t = Mat4::from_translation(self.position);
        let rx = Mat4::from_rotation_x(self.rotation.x.to_radians());
        let ry = Mat4::from_rotation_y(self.rotation.y.to_radians());
        let rz = Mat4::from_rotation_z(self.rotation.z.to_radians());
        let s = Mat4::from_scale(self.scale);
        t * ry * rx * rz * s
    }

    pub fn world_bounds_radius(&self) -> f32 {
        self.scale.length() * 0.5
    }
}

// ─── Light representation ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LightData {
    Directional {
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        shadow_distance: f32,
        shadow_resolution: u32,
        cascade_count: u8,
        cascade_splits: [f32; 4],
    },
    Point {
        color: Vec3,
        intensity: f32,
        radius: f32,
        falloff_exp: f32,
        cast_shadow: bool,
        shadow_near: f32,
    },
    Spot {
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        radius: f32,
        inner_cone_deg: f32,
        outer_cone_deg: f32,
        cast_shadow: bool,
    },
    Area {
        color: Vec3,
        intensity: f32,
        width: f32,
        height: f32,
        two_sided: bool,
    },
}

impl LightData {
    pub fn color(&self) -> Vec3 {
        match self {
            Self::Directional { color, .. } => *color,
            Self::Point { color, .. } => *color,
            Self::Spot { color, .. } => *color,
            Self::Area { color, .. } => *color,
        }
    }

    pub fn intensity(&self) -> f32 {
        match self {
            Self::Directional { intensity, .. } => *intensity,
            Self::Point { intensity, .. } => *intensity,
            Self::Spot { intensity, .. } => *intensity,
            Self::Area { intensity, .. } => *intensity,
        }
    }
}

// ─── Grid / axes ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GridSettings {
    pub visible: bool,
    pub size: f32,
    pub subdivisions: u32,
    pub color_major: Vec4,
    pub color_minor: Vec4,
    pub fade_distance: f32,
    pub show_axes: bool,
    pub show_origin: bool,
    pub snap_enabled: bool,
    pub snap_translate: f32,
    pub snap_rotate: f32,
    pub snap_scale: f32,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            visible: true,
            size: 100.0,
            subdivisions: 10,
            color_major: Vec4::new(0.5, 0.5, 0.5, 0.8),
            color_minor: Vec4::new(0.3, 0.3, 0.3, 0.4),
            fade_distance: 80.0,
            show_axes: true,
            show_origin: true,
            snap_enabled: false,
            snap_translate: 0.25,
            snap_rotate: 15.0,
            snap_scale: 0.1,
        }
    }
}

// ─── Viewport rendering settings ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingMode {
    Lit,
    Unlit,
    Wireframe,
    Normals,
    Albedo,
    Roughness,
    Metallic,
    AO,
    Emission,
    Depth,
    UV,
    VertexColor,
    SdfDistance,
    SdfNormals,
    Overdraw,
    LightingOnly,
    SpecularOnly,
    DiffuseOnly,
}

impl ShadingMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lit          => "Lit",
            Self::Unlit        => "Unlit",
            Self::Wireframe    => "Wireframe",
            Self::Normals      => "Normals",
            Self::Albedo       => "Albedo",
            Self::Roughness    => "Roughness",
            Self::Metallic     => "Metallic",
            Self::AO           => "AO",
            Self::Emission     => "Emission",
            Self::Depth        => "Depth",
            Self::UV           => "UV",
            Self::VertexColor  => "Vertex Color",
            Self::SdfDistance  => "SDF Distance",
            Self::SdfNormals   => "SDF Normals",
            Self::Overdraw     => "Overdraw",
            Self::LightingOnly => "Lighting Only",
            Self::SpecularOnly => "Specular Only",
            Self::DiffuseOnly  => "Diffuse Only",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Lit, Self::Unlit, Self::Wireframe, Self::Normals,
            Self::Albedo, Self::Roughness, Self::Metallic, Self::AO,
            Self::Emission, Self::Depth, Self::UV, Self::VertexColor,
            Self::SdfDistance, Self::SdfNormals, Self::Overdraw,
            Self::LightingOnly, Self::SpecularOnly, Self::DiffuseOnly,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub shading_mode: ShadingMode,
    pub msaa_samples: u8,
    pub bloom_enabled: bool,
    pub bloom_threshold: f32,
    pub bloom_intensity: f32,
    pub ao_enabled: bool,
    pub ao_radius: f32,
    pub ao_samples: u32,
    pub shadow_enabled: bool,
    pub shadow_bias: f32,
    pub ssr_enabled: bool,
    pub ssr_steps: u32,
    pub dof_enabled: bool,
    pub dof_focus_dist: f32,
    pub dof_aperture: f32,
    pub motion_blur_enabled: bool,
    pub motion_blur_strength: f32,
    pub exposure: f32,
    pub tonemapping: ToneMappingMode,
    pub gamma: f32,
    pub show_bounding_boxes: bool,
    pub show_light_cones: bool,
    pub show_bone_envelopes: bool,
    pub show_force_field_radii: bool,
    pub show_particle_counts: bool,
    pub render_scale: f32,
    pub background_color: Vec4,
    pub use_skybox: bool,
    pub skybox_intensity: f32,
    pub fog_enabled: bool,
    pub fog_start: f32,
    pub fog_end: f32,
    pub fog_density: f32,
    pub fog_color: Vec3,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            shading_mode: ShadingMode::Lit,
            msaa_samples: 4,
            bloom_enabled: true,
            bloom_threshold: 1.0,
            bloom_intensity: 0.3,
            ao_enabled: true,
            ao_radius: 0.5,
            ao_samples: 16,
            shadow_enabled: true,
            shadow_bias: 0.002,
            ssr_enabled: false,
            ssr_steps: 32,
            dof_enabled: false,
            dof_focus_dist: 10.0,
            dof_aperture: 0.05,
            motion_blur_enabled: false,
            motion_blur_strength: 0.5,
            exposure: 1.0,
            tonemapping: ToneMappingMode::Aces,
            gamma: 2.2,
            show_bounding_boxes: false,
            show_light_cones: true,
            show_bone_envelopes: false,
            show_force_field_radii: true,
            show_particle_counts: false,
            render_scale: 1.0,
            background_color: Vec4::new(0.12, 0.12, 0.14, 1.0),
            use_skybox: false,
            skybox_intensity: 1.0,
            fog_enabled: false,
            fog_start: 20.0,
            fog_end: 100.0,
            fog_density: 0.02,
            fog_color: Vec3::new(0.6, 0.7, 0.8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneMappingMode {
    None,
    Reinhard,
    ReinhardExtended,
    Aces,
    AcesApprox,
    Uncharted2,
    Filmic,
    AgX,
    Lottes,
}

impl ToneMappingMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None             => "None (Linear)",
            Self::Reinhard         => "Reinhard",
            Self::ReinhardExtended => "Reinhard Extended",
            Self::Aces             => "ACES",
            Self::AcesApprox       => "ACES (Fast)",
            Self::Uncharted2       => "Uncharted 2",
            Self::Filmic           => "Filmic",
            Self::AgX              => "AgX",
            Self::Lottes           => "Lottes",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::None, Self::Reinhard, Self::ReinhardExtended,
            Self::Aces, Self::AcesApprox, Self::Uncharted2,
            Self::Filmic, Self::AgX, Self::Lottes,
        ]
    }

    pub fn glsl_fn(&self) -> &'static str {
        match self {
            Self::None => "// linear passthrough",
            Self::Reinhard => "color = color / (color + vec3(1.0));",
            Self::ReinhardExtended => "color = color * (1.0 + color/vec3(9.0)) / (1.0 + color);",
            Self::Aces => {
                "const mat3 m1=mat3(0.59719,0.07600,0.02840,0.35458,0.90834,0.13383,0.04823,0.01566,0.83777);\
                 const mat3 m2=mat3(1.60475,-0.10208,-0.00327,-0.53108,1.10813,-0.07276,-0.07367,-0.00605,1.07602);\
                 vec3 v=m1*color;vec3 a=v*(v+0.0245786)-0.000090537;\
                 vec3 b=v*(0.983729*v+0.4329510)+0.238081;color=m2*(a/b);"
            }
            Self::AcesApprox => {
                "color=color*(2.51*color+0.03)/(color*(2.43*color+0.59)+0.14);"
            }
            Self::Uncharted2 => {
                "vec3 _uc2(vec3 x){return((x*(0.15*x+0.10*0.50)+0.20*0.02)/(x*(0.15*x+0.50)+0.20*0.30))-0.02/0.30;}\
                 color=_uc2(color*2.0)/_uc2(vec3(11.2));"
            }
            Self::Filmic => {
                "vec3 x=max(vec3(0.0),color-0.004);\
                 color=(x*(6.2*x+0.5))/(x*(6.2*x+1.7)+0.06);"
            }
            Self::AgX => "// AgX tonemapping (placeholder)",
            Self::Lottes => {
                "const float a=1.6,d=0.977,hdrMax=8.0,midIn=0.18,midOut=0.267;\
                 const float b=((-pow(midIn,a)+pow(hdrMax,a)*midOut)/(pow(pow(hdrMax,a),d)-pow(midIn,a)))/(midOut);\
                 const float c=(pow(hdrMax,a*d)*pow(midIn,a)-pow(hdrMax,a)*pow(midIn,a*d)*midOut)/(pow(pow(hdrMax,a),d)-pow(midIn,a))/midOut;\
                 color=pow(color,vec3(a))/(pow(color,vec3(a*d))*b+c);"
            }
        }
    }
}

// ─── Viewport split / layout ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportLayout {
    Single,
    HorizontalSplit,
    VerticalSplit,
    QuadSplit,
    ThreeLeft,
    ThreeRight,
}

impl ViewportLayout {
    pub fn panel_count(&self) -> usize {
        match self {
            Self::Single         => 1,
            Self::HorizontalSplit | Self::VerticalSplit => 2,
            Self::ThreeLeft | Self::ThreeRight => 3,
            Self::QuadSplit      => 4,
        }
    }

    pub fn rects(&self, w: f32, h: f32) -> Vec<ViewportRect> {
        match self {
            Self::Single => vec![ViewportRect::new(0.0, 0.0, w, h)],
            Self::HorizontalSplit => vec![
                ViewportRect::new(0.0, 0.0, w, h * 0.5),
                ViewportRect::new(0.0, h * 0.5, w, h * 0.5),
            ],
            Self::VerticalSplit => vec![
                ViewportRect::new(0.0, 0.0, w * 0.5, h),
                ViewportRect::new(w * 0.5, 0.0, w * 0.5, h),
            ],
            Self::QuadSplit => vec![
                ViewportRect::new(0.0,     0.0,     w*0.5, h*0.5),
                ViewportRect::new(w*0.5,   0.0,     w*0.5, h*0.5),
                ViewportRect::new(0.0,     h*0.5,   w*0.5, h*0.5),
                ViewportRect::new(w*0.5,   h*0.5,   w*0.5, h*0.5),
            ],
            Self::ThreeLeft => vec![
                ViewportRect::new(0.0,   0.0,   w*0.5, h),
                ViewportRect::new(w*0.5, 0.0,   w*0.5, h*0.5),
                ViewportRect::new(w*0.5, h*0.5, w*0.5, h*0.5),
            ],
            Self::ThreeRight => vec![
                ViewportRect::new(0.0,   0.0,   w*0.5, h*0.5),
                ViewportRect::new(0.0,   h*0.5, w*0.5, h*0.5),
                ViewportRect::new(w*0.5, 0.0,   w*0.5, h),
            ],
        }
    }
}

// ─── Selection outline ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SelectionOutline {
    pub enabled: bool,
    pub color: Vec4,
    pub width: f32,
    pub pulse: bool,
    pub pulse_speed: f32,
}

impl Default for SelectionOutline {
    fn default() -> Self {
        Self {
            enabled: true,
            color: Vec4::new(1.0, 0.6, 0.1, 1.0),
            width: 2.0,
            pulse: true,
            pulse_speed: 2.0,
        }
    }
}

// ─── Hover state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HoverState {
    pub entity: Option<EntityId>,
    pub screen_pos: Vec2,
    pub world_pos: Vec3,
    pub world_normal: Vec3,
    pub distance: f32,
}

impl Default for HoverState {
    fn default() -> Self {
        Self {
            entity: None,
            screen_pos: Vec2::ZERO,
            world_pos: Vec3::ZERO,
            world_normal: Vec3::Y,
            distance: f32::MAX,
        }
    }
}

// ─── Viewport panel ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ViewportPanel {
    pub index: usize,
    pub rect: ViewportRect,
    pub camera: EditorCamera,
    pub snap_view: SnapView,
    pub render_settings: RenderSettings,
    pub is_active: bool,
    pub is_playing: bool,
    pub show_gizmos: bool,
    pub hover: HoverState,
    pub frame_count: u64,
    pub render_time_ms: f32,
    pub hovered_port: Option<String>,
}

impl ViewportPanel {
    pub fn new(index: usize, rect: ViewportRect) -> Self {
        let mut camera = EditorCamera::new();
        // Set default snap views based on panel index
        let snap = match index {
            0 => SnapView::IsoFrontRight,
            1 => SnapView::Top,
            2 => SnapView::Front,
            3 => SnapView::Right,
            _ => SnapView::IsoFrontRight,
        };
        camera.snap_to(snap);
        Self {
            index,
            rect,
            camera,
            snap_view: snap,
            render_settings: RenderSettings::default(),
            is_active: index == 0,
            is_playing: false,
            show_gizmos: true,
            hover: HoverState::default(),
            frame_count: 0,
            render_time_ms: 0.0,
            hovered_port: None,
        }
    }

    pub fn resize(&mut self, rect: ViewportRect) {
        self.rect = rect;
        // EditorCamera stores projection internally; aspect is set via the projection field
    }

    pub fn view_matrix(&self) -> Mat4 {
        self.camera.view_matrix()
    }

    pub fn proj_matrix(&self) -> Mat4 {
        self.camera.projection.matrix(self.rect.aspect())
    }

    pub fn view_proj(&self) -> Mat4 {
        self.proj_matrix() * self.view_matrix()
    }

    pub fn world_to_screen(&self, world: Vec3) -> Vec2 {
        let clip = self.view_proj() * Vec4::new(world.x, world.y, world.z, 1.0);
        if clip.w.abs() < 1e-6 { return Vec2::new(-99999.0, -99999.0); }
        let ndc = Vec2::new(clip.x / clip.w, clip.y / clip.w);
        Vec2::new(
            self.rect.x + (ndc.x * 0.5 + 0.5) * self.rect.width,
            self.rect.y + (1.0 - (ndc.y * 0.5 + 0.5)) * self.rect.height,
        )
    }

    pub fn screen_to_ray(&self, screen_x: f32, screen_y: f32) -> (Vec3, Vec3) {
        let ndc = self.rect.to_ndc(screen_x, screen_y);
        self.camera.screen_to_ray(ndc)
    }

    /// Update hover position given raw cursor input
    pub fn update_hover(&mut self, screen_x: f32, screen_y: f32) {
        if !self.rect.contains(screen_x, screen_y) {
            self.hover.entity = None;
            return;
        }
        self.hover.screen_pos = Vec2::new(screen_x, screen_y);
        let (origin, dir) = self.screen_to_ray(screen_x, screen_y);
        // Plane intersect at y=0 as default
        if dir.y.abs() > 1e-5 {
            let t = -origin.y / dir.y;
            if t > 0.0 {
                self.hover.world_pos = origin + dir * t;
                self.hover.world_normal = Vec3::Y;
                self.hover.distance = t;
            }
        }
    }

    pub fn cycle_shading_mode(&mut self) {
        let all = ShadingMode::all();
        let cur = self.render_settings.shading_mode;
        let idx = all.iter().position(|&m| m == cur).unwrap_or(0);
        self.render_settings.shading_mode = all[(idx + 1) % all.len()];
    }

    pub fn toggle_wireframe(&mut self) {
        if self.render_settings.shading_mode == ShadingMode::Wireframe {
            self.render_settings.shading_mode = ShadingMode::Lit;
        } else {
            self.render_settings.shading_mode = ShadingMode::Wireframe;
        }
    }
}

// ─── Main Viewport struct ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Viewport {
    pub layout: ViewportLayout,
    pub panels: Vec<ViewportPanel>,
    pub active_panel: usize,
    pub total_width: f32,
    pub total_height: f32,
    pub grid: GridSettings,
    pub selection_outline: SelectionOutline,
    pub selected_entities: Vec<EntityId>,
    pub gizmo_mode: GizmoMode,
    pub gizmo_space: GizmoSpace,
    pub entities: HashMap<EntityId, ViewportEntity>,
    pub lights: Vec<LightData>,
    next_entity_id: u32,
    pub show_stats: bool,
    pub show_fps: bool,
    pub show_camera_info: bool,
    pub playback_time: f32,
    pub is_playing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoSpace {
    Local,
    World,
}

impl Viewport {
    pub fn new(width: f32, height: f32) -> Self {
        let layout = ViewportLayout::Single;
        let rects = layout.rects(width, height);
        let panels = rects.into_iter().enumerate()
            .map(|(i, r)| ViewportPanel::new(i, r))
            .collect();

        Self {
            layout,
            panels,
            active_panel: 0,
            total_width: width,
            total_height: height,
            grid: GridSettings::default(),
            selection_outline: SelectionOutline::default(),
            selected_entities: Vec::new(),
            gizmo_mode: GizmoMode::Translate,
            gizmo_space: GizmoSpace::World,
            entities: HashMap::new(),
            lights: Vec::new(),
            next_entity_id: 1,
            show_stats: true,
            show_fps: true,
            show_camera_info: false,
            playback_time: 0.0,
            is_playing: false,
        }
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.total_width = width;
        self.total_height = height;
        let rects = self.layout.rects(width, height);
        for (panel, rect) in self.panels.iter_mut().zip(rects.into_iter()) {
            panel.resize(rect);
        }
    }

    pub fn set_layout(&mut self, layout: ViewportLayout) {
        let rects = layout.rects(self.total_width, self.total_height);
        let old_count = self.panels.len();
        let new_count = layout.panel_count();

        // Add panels if needed
        while self.panels.len() < new_count {
            let idx = self.panels.len();
            let rect = rects.get(idx).copied().unwrap_or(
                ViewportRect::new(0.0, 0.0, self.total_width, self.total_height)
            );
            self.panels.push(ViewportPanel::new(idx, rect));
        }
        // Resize existing
        for (i, rect) in rects.iter().enumerate() {
            if let Some(panel) = self.panels.get_mut(i) {
                panel.resize(*rect);
            }
        }
        // Trim
        self.panels.truncate(new_count);
        let _ = old_count;
        self.layout = layout;
        self.active_panel = self.active_panel.min(new_count.saturating_sub(1));
    }

    pub fn active_panel(&self) -> &ViewportPanel {
        &self.panels[self.active_panel.min(self.panels.len().saturating_sub(1))]
    }

    pub fn active_panel_mut(&mut self) -> &mut ViewportPanel {
        let idx = self.active_panel.min(self.panels.len().saturating_sub(1));
        &mut self.panels[idx]
    }

    pub fn spawn_entity(&mut self, name: String, kind: EntityKind) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        let entity = ViewportEntity::new(id, name, kind);
        self.entities.insert(id, entity);
        id
    }

    pub fn spawn_at(&mut self, name: String, kind: EntityKind, pos: Vec3) -> EntityId {
        let id = self.spawn_entity(name, kind);
        if let Some(e) = self.entities.get_mut(&id) {
            e.position = pos;
        }
        id
    }

    pub fn despawn(&mut self, id: EntityId) {
        if let Some(e) = self.entities.remove(&id) {
            // Unlink children
            for child_id in &e.children {
                if let Some(child) = self.entities.get_mut(child_id) {
                    child.parent = None;
                }
            }
            // Unlink from parent
            if let Some(pid) = e.parent {
                if let Some(parent) = self.entities.get_mut(&pid) {
                    parent.children.retain(|&c| c != id);
                }
            }
        }
        self.selected_entities.retain(|&e| e != id);
    }

    pub fn select(&mut self, id: EntityId, additive: bool) {
        if !additive {
            self.selected_entities.clear();
        }
        if !self.selected_entities.contains(&id) {
            self.selected_entities.push(id);
        }
    }

    pub fn deselect_all(&mut self) {
        self.selected_entities.clear();
    }

    pub fn select_box(
        &mut self,
        min_screen: Vec2,
        max_screen: Vec2,
        additive: bool,
    ) {
        if !additive { self.selected_entities.clear(); }
        let panel = &self.panels[self.active_panel];
        for (&id, entity) in &self.entities {
            if !entity.visible { continue; }
            let sp = panel.world_to_screen(entity.position);
            if sp.x >= min_screen.x && sp.x <= max_screen.x
                && sp.y >= min_screen.y && sp.y <= max_screen.y
            {
                if !self.selected_entities.contains(&id) {
                    self.selected_entities.push(id);
                }
            }
        }
    }

    pub fn frame_selection(&mut self) {
        if self.selected_entities.is_empty() { return; }
        let mut center = Vec3::ZERO;
        let mut count = 0;
        let mut radius = 1.0f32;
        for &id in &self.selected_entities {
            if let Some(e) = self.entities.get(&id) {
                center += e.position;
                count += 1;
                radius = radius.max(e.world_bounds_radius());
            }
        }
        if count > 0 {
            center /= count as f32;
            let panel = &mut self.panels[self.active_panel];
            panel.camera.frame_selection(center, radius);
        }
    }

    pub fn frame_all(&mut self) {
        if self.entities.is_empty() { return; }
        let mut center = Vec3::ZERO;
        for e in self.entities.values() { center += e.position; }
        center /= self.entities.len() as f32;
        let mut radius = 5.0f32;
        for e in self.entities.values() {
            radius = radius.max((e.position - center).length() + e.world_bounds_radius());
        }
        let panel = &mut self.panels[self.active_panel];
        panel.camera.frame_selection(center, radius);
    }

    pub fn set_snap_view(&mut self, view: SnapView) {
        let panel = &mut self.panels[self.active_panel];
        panel.camera.snap_to(view);
        panel.snap_view = view;
    }

    pub fn toggle_gizmo_space(&mut self) {
        self.gizmo_space = match self.gizmo_space {
            GizmoSpace::Local => GizmoSpace::World,
            GizmoSpace::World => GizmoSpace::Local,
        };
    }

    pub fn cycle_gizmo_mode(&mut self) {
        self.gizmo_mode = match self.gizmo_mode {
            GizmoMode::Translate => GizmoMode::Rotate,
            GizmoMode::Rotate    => GizmoMode::Scale,
            _                    => GizmoMode::Translate,
        };
    }

    pub fn update(&mut self, dt: f32, input: &ViewportInput) {
        if self.is_playing {
            self.playback_time += dt;
        }
        for panel in &mut self.panels {
            panel.camera.update(dt, None);
            if input.cursor_moved {
                panel.update_hover(input.cursor_x, input.cursor_y);
            }
        }
        // Determine active panel from cursor
        for (i, panel) in self.panels.iter().enumerate() {
            if panel.rect.contains(input.cursor_x, input.cursor_y) {
                self.active_panel = i;
                break;
            }
        }
    }

    /// Build GPU frame data for the current frame
    pub fn build_frame_data(&self) -> ViewportFrameData {
        let panel = self.active_panel();
        ViewportFrameData {
            view: panel.view_matrix(),
            proj: panel.proj_matrix(),
            view_proj: panel.view_proj(),
            camera_pos: panel.camera.orbit_position(),
            viewport_size: Vec2::new(panel.rect.width, panel.rect.height),
            time: self.playback_time,
            shading_mode: panel.render_settings.shading_mode,
            entity_transforms: self.entities.iter()
                .map(|(&id, e)| (id, e.model_matrix()))
                .collect(),
        }
    }
}

// ─── Frame data ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ViewportFrameData {
    pub view: Mat4,
    pub proj: Mat4,
    pub view_proj: Mat4,
    pub camera_pos: Vec3,
    pub viewport_size: Vec2,
    pub time: f32,
    pub shading_mode: ShadingMode,
    pub entity_transforms: HashMap<EntityId, Mat4>,
}

// ─── Input ───────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct ViewportInput {
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_moved: bool,
    pub left_pressed: bool,
    pub right_pressed: bool,
    pub middle_pressed: bool,
    pub left_released: bool,
    pub right_released: bool,
    pub scroll_delta: f32,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

// ─── Box selection ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BoxSelection {
    pub active: bool,
    pub start: Vec2,
    pub end: Vec2,
}

impl BoxSelection {
    pub fn new() -> Self {
        Self { active: false, start: Vec2::ZERO, end: Vec2::ZERO }
    }

    pub fn begin(&mut self, x: f32, y: f32) {
        self.active = true;
        self.start = Vec2::new(x, y);
        self.end = Vec2::new(x, y);
    }

    pub fn update(&mut self, x: f32, y: f32) {
        self.end = Vec2::new(x, y);
    }

    pub fn finish(&mut self) -> (Vec2, Vec2) {
        self.active = false;
        let min = Vec2::new(self.start.x.min(self.end.x), self.start.y.min(self.end.y));
        let max = Vec2::new(self.start.x.max(self.end.x), self.start.y.max(self.end.y));
        (min, max)
    }
}

impl Default for BoxSelection {
    fn default() -> Self { Self::new() }
}

// ─── Drag-drop target ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropTarget {
    None,
    Viewport { panel: usize },
    HierarchyNode { entity: EntityId },
    AssetSlot { slot_name: String },
}

// ─── Stats overlay ───────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct ViewportStats {
    pub fps: f32,
    pub frame_ms: f32,
    pub entity_count: usize,
    pub light_count: usize,
    pub draw_calls: u32,
    pub triangle_count: u32,
    pub texture_memory_mb: f32,
    pub vertex_memory_mb: f32,
    pub visible_entities: usize,
    pub culled_entities: usize,
    pub shadow_draw_calls: u32,
    pub particle_count: u32,
}

impl ViewportStats {
    pub fn format_compact(&self) -> String {
        format!(
            "FPS:{:.0} | {:.1}ms | {}ent | {}dc | {}K tri",
            self.fps, self.frame_ms, self.entity_count,
            self.draw_calls, self.triangle_count / 1000
        )
    }

    pub fn format_verbose(&self) -> String {
        format!(
            "FPS: {:.1}  Frame: {:.2}ms\n\
             Entities: {} ({} visible, {} culled)\n\
             Draw calls: {} (+{} shadow)\n\
             Triangles: {}K\n\
             Lights: {}\n\
             Particles: {}\n\
             Texture mem: {:.1}MB\n\
             Vertex mem:  {:.1}MB",
            self.fps, self.frame_ms,
            self.entity_count, self.visible_entities, self.culled_entities,
            self.draw_calls, self.shadow_draw_calls,
            self.triangle_count / 1000,
            self.light_count,
            self.particle_count,
            self.texture_memory_mb,
            self.vertex_memory_mb,
        )
    }
}

// ─── Viewport controller (high-level) ────────────────────────────────────────

#[derive(Debug)]
pub struct ViewportController {
    pub viewport: Viewport,
    pub box_select: BoxSelection,
    pub stats: ViewportStats,
    pub perf: PerfOverlay,
    drag_entity_start: Option<(EntityId, Vec3)>,
    pub drop_target: DropTarget,
    pub cursor_shape: CursorShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Default,
    Crosshair,
    Move,
    ResizeH,
    ResizeV,
    ResizeDiag,
    Hand,
    NotAllowed,
    EyeDropper,
}

impl ViewportController {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            viewport: Viewport::new(width, height),
            box_select: BoxSelection::new(),
            stats: ViewportStats::default(),
            perf: PerfOverlay::new(),
            drag_entity_start: None,
            drop_target: DropTarget::None,
            cursor_shape: CursorShape::Default,
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        self.viewport.resize(w, h);
    }

    pub fn handle_mouse_down(&mut self, x: f32, y: f32, button: MouseButton, modifiers: KeyMods) {
        if let Some(panel_idx) = self.panel_at(x, y) {
            self.viewport.active_panel = panel_idx;
            let panel = &self.viewport.panels[panel_idx];

            match button {
                MouseButton::Left => {
                    // Try entity pick first
                    let (origin, dir) = panel.screen_to_ray(x, y);
                    if let Some(picked) = self.pick_entity(origin, dir) {
                        self.viewport.select(picked, modifiers.shift);
                        self.drag_entity_start = Some((picked,
                            self.viewport.entities.get(&picked)
                                .map(|e| e.position).unwrap_or(Vec3::ZERO)
                        ));
                    } else if !modifiers.shift {
                        self.box_select.begin(x, y);
                    }
                }
                MouseButton::Right => {}
                MouseButton::Middle => {}
            }
        }
    }

    pub fn handle_mouse_up(&mut self, x: f32, y: f32, button: MouseButton, modifiers: KeyMods) {
        match button {
            MouseButton::Left => {
                self.drag_entity_start = None;
                if self.box_select.active {
                    let (min, max) = self.box_select.finish();
                    // Only count if the box is big enough (not a click)
                    if (max - min).length() > 4.0 {
                        self.viewport.select_box(min, max, modifiers.shift);
                    }
                }
            }
            _ => {}
        }
        let _ = (x, y);
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32, dx: f32, dy: f32, buttons: MouseButtons) {
        if self.box_select.active {
            self.box_select.update(x, y);
        }
        if buttons.right {
            // Orbit / pan / free-fly handled by EditorCamera
            let panel_idx = self.viewport.active_panel;
            let panel = &mut self.viewport.panels[panel_idx];
            panel.camera.orbit_drag(Vec2::new(dx * 0.3, dy * 0.3));
        }
        if buttons.middle {
            let panel_idx = self.viewport.active_panel;
            let panel = &mut self.viewport.panels[panel_idx];
            panel.camera.pan_drag(Vec2::new(dx, dy));
        }
    }

    pub fn handle_scroll(&mut self, delta: f32) {
        let panel_idx = self.viewport.active_panel;
        let panel = &mut self.viewport.panels[panel_idx];
        panel.camera.scroll_zoom(delta);
    }

    fn panel_at(&self, x: f32, y: f32) -> Option<usize> {
        self.viewport.panels.iter()
            .position(|p| p.rect.contains(x, y))
    }

    fn pick_entity(&self, origin: Vec3, dir: Vec3) -> Option<EntityId> {
        let mut best: Option<(EntityId, f32)> = None;
        for (&id, entity) in &self.viewport.entities {
            if !entity.visible { continue; }
            let radius = entity.world_bounds_radius().max(0.3);
            let oc = origin - entity.position;
            let b = oc.dot(dir);
            let c = oc.dot(oc) - radius * radius;
            let disc = b * b - c;
            if disc >= 0.0 {
                let t = -b - disc.sqrt();
                if t > 0.01 {
                    if best.map(|(_, bt)| t < bt).unwrap_or(true) {
                        best = Some((id, t));
                    }
                }
            }
        }
        best.map(|(id, _)| id)
    }

    pub fn update(&mut self, dt: f32, input: &ViewportInput) {
        self.perf.begin_frame();
        self.viewport.update(dt, input);
        self.perf.end_frame(dt * 1000.0, 0, 0, 0.0, 0.0);

        // Update cursor shape
        self.cursor_shape = if self.box_select.active {
            CursorShape::Crosshair
        } else if self.drag_entity_start.is_some() {
            CursorShape::Move
        } else {
            CursorShape::Default
        };
    }

    pub fn frame_selection(&mut self) { self.viewport.frame_selection(); }
    pub fn frame_all(&mut self)       { self.viewport.frame_all(); }

    pub fn spawn_entity_at_cursor(&mut self, kind: EntityKind) -> EntityId {
        let panel = self.viewport.active_panel();
        let pos = panel.hover.world_pos;
        let name = format!("{:?}_{}", kind, self.viewport.entities.len());
        self.viewport.spawn_at(name, kind, pos)
    }

    pub fn entity_count(&self) -> usize { self.viewport.entities.len() }

    pub fn selected_count(&self) -> usize { self.viewport.selected_entities.len() }

    pub fn render_grid_lines(&self) -> Vec<GridLine> {
        let g = &self.viewport.grid;
        if !g.visible { return Vec::new(); }
        let mut lines = Vec::new();
        let half = g.size * 0.5;
        let step = g.size / g.subdivisions as f32;
        let mut i = 0;
        let mut x = -half;
        while x <= half + 1e-4 {
            let major = (i % g.subdivisions as i32) == 0;
            let color = if major { g.color_major } else { g.color_minor };
            lines.push(GridLine {
                start: Vec3::new(x, 0.0, -half),
                end:   Vec3::new(x, 0.0,  half),
                color,
            });
            lines.push(GridLine {
                start: Vec3::new(-half, 0.0, x),
                end:   Vec3::new( half, 0.0, x),
                color,
            });
            x += step;
            i += 1;
        }
        lines
    }
}

#[derive(Debug, Clone)]
pub struct GridLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: Vec4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton { Left, Right, Middle }

#[derive(Debug, Clone, Copy, Default)]
pub struct MouseButtons {
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyMods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_resize() {
        let mut v = Viewport::new(1920.0, 1080.0);
        v.resize(2560.0, 1440.0);
        assert_eq!(v.total_width, 2560.0);
    }

    #[test]
    fn layout_quad_has_four_panels() {
        let mut v = Viewport::new(1000.0, 1000.0);
        v.set_layout(ViewportLayout::QuadSplit);
        assert_eq!(v.panels.len(), 4);
    }

    #[test]
    fn spawn_and_despawn() {
        let mut v = Viewport::new(800.0, 600.0);
        let id = v.spawn_entity("Test".into(), EntityKind::Marker);
        assert!(v.entities.contains_key(&id));
        v.despawn(id);
        assert!(!v.entities.contains_key(&id));
    }

    #[test]
    fn world_to_screen_roundtrip_origin() {
        let v = Viewport::new(800.0, 600.0);
        let panel = v.active_panel();
        let sp = panel.world_to_screen(Vec3::ZERO);
        // Origin should be somewhere in the viewport (not off screen)
        assert!(sp.x > -1000.0 && sp.x < 2000.0);
    }

    #[test]
    fn gizmo_mode_cycle() {
        let mut v = Viewport::new(800.0, 600.0);
        v.cycle_gizmo_mode();
        assert_eq!(v.gizmo_mode, GizmoMode::Rotate);
        v.cycle_gizmo_mode();
        assert_eq!(v.gizmo_mode, GizmoMode::Scale);
        v.cycle_gizmo_mode();
        assert_eq!(v.gizmo_mode, GizmoMode::Translate);
    }
}
