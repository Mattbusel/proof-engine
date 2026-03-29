
//! Visual particle system editor — emitter modules, curves, preview, gradient editor.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Value sampling — constant, curve, or random
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum MinMaxCurve {
    Constant(f32),
    Random(f32, f32),          // min, max
    Curve(Vec<Vec2>),          // control points (time, value)
    TwoCurves(Vec<Vec2>, Vec<Vec2>),
}

impl MinMaxCurve {
    pub fn evaluate(&self, t: f32, rand01: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            MinMaxCurve::Constant(v) => *v,
            MinMaxCurve::Random(lo, hi) => lo + (hi - lo) * rand01,
            MinMaxCurve::Curve(pts) => {
                if pts.is_empty() { return 0.0; }
                if pts.len() == 1 { return pts[0].y; }
                let i = pts.partition_point(|p| p.x <= t);
                if i == 0 { return pts[0].y; }
                if i >= pts.len() { return pts[pts.len()-1].y; }
                let a = pts[i-1];
                let b = pts[i];
                let u = (t - a.x) / (b.x - a.x).max(1e-6);
                a.y + (b.y - a.y) * u
            }
            MinMaxCurve::TwoCurves(lo_pts, hi_pts) => {
                let lo = MinMaxCurve::Curve(lo_pts.clone()).evaluate(t, 0.0);
                let hi = MinMaxCurve::Curve(hi_pts.clone()).evaluate(t, 0.0);
                lo + (hi - lo) * rand01
            }
        }
    }

    pub fn constant(v: f32) -> Self { MinMaxCurve::Constant(v) }
    pub fn random_range(lo: f32, hi: f32) -> Self { MinMaxCurve::Random(lo, hi) }
}

#[derive(Debug, Clone)]
pub struct MinMaxGradient {
    pub stops: Vec<(f32, Vec4)>, // (time 0..1, rgba)
}

impl MinMaxGradient {
    pub fn solid(color: Vec4) -> Self {
        Self { stops: vec![(0.0, color), (1.0, color)] }
    }

    pub fn two_color(from: Vec4, to: Vec4) -> Self {
        Self { stops: vec![(0.0, from), (1.0, to)] }
    }

    pub fn evaluate(&self, t: f32) -> Vec4 {
        let t = t.clamp(0.0, 1.0);
        if self.stops.is_empty() { return Vec4::ONE; }
        if self.stops.len() == 1 { return self.stops[0].1; }
        let i = self.stops.partition_point(|s| s.0 <= t);
        if i == 0 { return self.stops[0].1; }
        if i >= self.stops.len() { return self.stops[self.stops.len()-1].1; }
        let (ta, ca) = self.stops[i-1];
        let (tb, cb) = self.stops[i];
        let u = (t - ta) / (tb - ta).max(1e-6);
        ca.lerp(cb, u)
    }

    pub fn add_stop(&mut self, t: f32, color: Vec4) {
        let i = self.stops.partition_point(|s| s.0 <= t);
        self.stops.insert(i, (t, color));
    }
}

// ---------------------------------------------------------------------------
// Emitter shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum EmitterShape {
    Point,
    Sphere { radius: f32, thickness: f32 },
    Hemisphere { radius: f32, thickness: f32 },
    Cone { angle: f32, radius: f32, length: f32, emit_from_volume: bool },
    Box { half_extents: Vec3, emit_from_surface: bool },
    Circle { radius: f32, arc: f32, thickness: f32 },
    Edge { length: f32 },
    Mesh { mesh_id: u64 },
    Trail,
    Donut { radius: f32, donut_radius: f32 },
    Rectangle { half_extents: Vec2, emit_from_edge: bool },
    Line { length: f32, spread: f32 },
}

impl EmitterShape {
    pub fn label(&self) -> &'static str {
        match self {
            EmitterShape::Point => "Point",
            EmitterShape::Sphere { .. } => "Sphere",
            EmitterShape::Hemisphere { .. } => "Hemisphere",
            EmitterShape::Cone { .. } => "Cone",
            EmitterShape::Box { .. } => "Box",
            EmitterShape::Circle { .. } => "Circle",
            EmitterShape::Edge { .. } => "Edge",
            EmitterShape::Mesh { .. } => "Mesh",
            EmitterShape::Trail => "Trail",
            EmitterShape::Donut { .. } => "Donut",
            EmitterShape::Rectangle { .. } => "Rectangle",
            EmitterShape::Line { .. } => "Line",
        }
    }

    /// Generate a position and direction for a newly emitted particle (pseudo-random).
    pub fn sample_position(&self, rand: [f32; 4]) -> (Vec3, Vec3) {
        match self {
            EmitterShape::Point => (Vec3::ZERO, Vec3::Y),
            EmitterShape::Sphere { radius, thickness } => {
                let theta = rand[0] * std::f32::consts::TAU;
                let phi = (rand[1] * 2.0 - 1.0).acos();
                let r = radius * (1.0 - thickness * rand[2]);
                let pos = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()) * r;
                (pos, pos.normalize())
            }
            EmitterShape::Cone { angle, radius, length, .. } => {
                let t = rand[0] * length;
                let local_radius = (angle * std::f32::consts::PI / 180.0).tan() * t + radius;
                let theta = rand[1] * std::f32::consts::TAU;
                let r = (rand[2]).sqrt() * local_radius;
                let pos = Vec3::new(r * theta.cos(), t, r * theta.sin());
                let dir = Vec3::new(0.0, 1.0, 0.0).normalize();
                (pos, dir)
            }
            EmitterShape::Box { half_extents, emit_from_surface } => {
                if *emit_from_surface {
                    let pos = Vec3::new(
                        (rand[0] * 2.0 - 1.0) * half_extents.x,
                        half_extents.y,
                        (rand[1] * 2.0 - 1.0) * half_extents.z,
                    );
                    (pos, Vec3::Y)
                } else {
                    let pos = Vec3::new(
                        (rand[0] * 2.0 - 1.0) * half_extents.x,
                        (rand[1] * 2.0 - 1.0) * half_extents.y,
                        (rand[2] * 2.0 - 1.0) * half_extents.z,
                    );
                    (pos, Vec3::Y)
                }
            }
            EmitterShape::Circle { radius, arc, thickness } => {
                let theta = rand[0] * arc * std::f32::consts::PI / 180.0;
                let r = radius * (1.0 - thickness * rand[1]);
                (Vec3::new(r * theta.cos(), 0.0, r * theta.sin()), Vec3::Y)
            }
            _ => (Vec3::ZERO, Vec3::Y),
        }
    }
}

// ---------------------------------------------------------------------------
// Module system (Unity-style)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MainModule {
    pub duration: f32,
    pub looping: bool,
    pub prewarm: bool,
    pub start_delay: MinMaxCurve,
    pub start_lifetime: MinMaxCurve,
    pub start_speed: MinMaxCurve,
    pub start_size_3d: bool,
    pub start_size: MinMaxCurve,
    pub start_size_x: MinMaxCurve,
    pub start_size_y: MinMaxCurve,
    pub start_size_z: MinMaxCurve,
    pub start_rotation_3d: bool,
    pub start_rotation: MinMaxCurve,
    pub flip_rotation: f32,
    pub start_color: MinMaxGradient,
    pub gravity_modifier: MinMaxCurve,
    pub simulation_space: SimulationSpace,
    pub simulation_speed: f32,
    pub stop_action: StopAction,
    pub culling_mode: CullingMode,
    pub max_particles: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimulationSpace { Local, World, Custom }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StopAction { None, Disable, Destroy, Callback }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CullingMode { Automatic, AlwaysSimulate, PauseAndCatchup, Pause }

impl Default for MainModule {
    fn default() -> Self {
        Self {
            duration: 5.0,
            looping: true,
            prewarm: false,
            start_delay: MinMaxCurve::constant(0.0),
            start_lifetime: MinMaxCurve::random_range(1.0, 3.0),
            start_speed: MinMaxCurve::random_range(1.0, 5.0),
            start_size_3d: false,
            start_size: MinMaxCurve::random_range(0.05, 0.2),
            start_size_x: MinMaxCurve::constant(1.0),
            start_size_y: MinMaxCurve::constant(1.0),
            start_size_z: MinMaxCurve::constant(1.0),
            start_rotation_3d: false,
            start_rotation: MinMaxCurve::random_range(0.0, 360.0),
            flip_rotation: 0.0,
            start_color: MinMaxGradient::solid(Vec4::ONE),
            gravity_modifier: MinMaxCurve::constant(0.0),
            simulation_space: SimulationSpace::World,
            simulation_speed: 1.0,
            stop_action: StopAction::None,
            culling_mode: CullingMode::Automatic,
            max_particles: 1000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmissionModule {
    pub enabled: bool,
    pub rate_over_time: MinMaxCurve,
    pub rate_over_distance: MinMaxCurve,
    pub bursts: Vec<EmissionBurst>,
}

#[derive(Debug, Clone)]
pub struct EmissionBurst {
    pub time: f32,
    pub count: MinMaxCurve,
    pub cycles: u32,
    pub interval: f32,
    pub probability: f32,
}

impl Default for EmissionModule {
    fn default() -> Self {
        Self {
            enabled: true,
            rate_over_time: MinMaxCurve::constant(10.0),
            rate_over_distance: MinMaxCurve::constant(0.0),
            bursts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VelocityOverLifetimeModule {
    pub enabled: bool,
    pub x: MinMaxCurve,
    pub y: MinMaxCurve,
    pub z: MinMaxCurve,
    pub space: SimulationSpace,
    pub orbital_x: MinMaxCurve,
    pub orbital_y: MinMaxCurve,
    pub orbital_z: MinMaxCurve,
    pub orbital_offset_x: MinMaxCurve,
    pub orbital_offset_y: MinMaxCurve,
    pub orbital_offset_z: MinMaxCurve,
    pub radial: MinMaxCurve,
    pub speed_modifier: MinMaxCurve,
}

impl Default for VelocityOverLifetimeModule {
    fn default() -> Self {
        Self {
            enabled: false,
            x: MinMaxCurve::constant(0.0),
            y: MinMaxCurve::constant(0.0),
            z: MinMaxCurve::constant(0.0),
            space: SimulationSpace::Local,
            orbital_x: MinMaxCurve::constant(0.0),
            orbital_y: MinMaxCurve::constant(0.0),
            orbital_z: MinMaxCurve::constant(0.0),
            orbital_offset_x: MinMaxCurve::constant(0.0),
            orbital_offset_y: MinMaxCurve::constant(0.0),
            orbital_offset_z: MinMaxCurve::constant(0.0),
            radial: MinMaxCurve::constant(0.0),
            speed_modifier: MinMaxCurve::constant(1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColorOverLifetimeModule {
    pub enabled: bool,
    pub color: MinMaxGradient,
}

impl Default for ColorOverLifetimeModule {
    fn default() -> Self {
        Self {
            enabled: true,
            color: MinMaxGradient::two_color(Vec4::ONE, Vec4::new(1.0, 1.0, 1.0, 0.0)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SizeOverLifetimeModule {
    pub enabled: bool,
    pub size: MinMaxCurve,
    pub size_3d: bool,
    pub x: MinMaxCurve,
    pub y: MinMaxCurve,
    pub z: MinMaxCurve,
}

impl Default for SizeOverLifetimeModule {
    fn default() -> Self {
        Self {
            enabled: true,
            size: MinMaxCurve::Curve(vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(0.2, 1.0),
                Vec2::new(0.8, 1.0),
                Vec2::new(1.0, 0.0),
            ]),
            size_3d: false,
            x: MinMaxCurve::constant(1.0),
            y: MinMaxCurve::constant(1.0),
            z: MinMaxCurve::constant(1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RotationOverLifetimeModule {
    pub enabled: bool,
    pub angular_velocity: MinMaxCurve,
    pub rotation_3d: bool,
    pub x: MinMaxCurve,
    pub y: MinMaxCurve,
    pub z: MinMaxCurve,
}

impl Default for RotationOverLifetimeModule {
    fn default() -> Self {
        Self {
            enabled: false,
            angular_velocity: MinMaxCurve::random_range(-90.0, 90.0),
            rotation_3d: false,
            x: MinMaxCurve::constant(0.0),
            y: MinMaxCurve::constant(0.0),
            z: MinMaxCurve::constant(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForceMode { Force, Acceleration, Impulse, VelocityChange }

#[derive(Debug, Clone)]
pub struct ForceOverLifetimeModule {
    pub enabled: bool,
    pub x: MinMaxCurve,
    pub y: MinMaxCurve,
    pub z: MinMaxCurve,
    pub space: SimulationSpace,
    pub randomize_per_frame: bool,
}

impl Default for ForceOverLifetimeModule {
    fn default() -> Self {
        Self {
            enabled: false,
            x: MinMaxCurve::constant(0.0),
            y: MinMaxCurve::constant(-9.8),
            z: MinMaxCurve::constant(0.0),
            space: SimulationSpace::World,
            randomize_per_frame: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CollisionType { Planes, World }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CollisionMode { Collision2D, Collision3D }

#[derive(Debug, Clone)]
pub struct CollisionModule {
    pub enabled: bool,
    pub collision_type: CollisionType,
    pub mode: CollisionMode,
    pub dampen: MinMaxCurve,
    pub bounce: MinMaxCurve,
    pub lifetime_loss: MinMaxCurve,
    pub min_kill_speed: f32,
    pub max_kill_speed: f32,
    pub radius_scale: f32,
    pub send_collision_messages: bool,
}

impl Default for CollisionModule {
    fn default() -> Self {
        Self {
            enabled: false,
            collision_type: CollisionType::World,
            mode: CollisionMode::Collision3D,
            dampen: MinMaxCurve::constant(0.1),
            bounce: MinMaxCurve::constant(0.6),
            lifetime_loss: MinMaxCurve::constant(0.0),
            min_kill_speed: 0.0,
            max_kill_speed: 10000.0,
            radius_scale: 1.0,
            send_collision_messages: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrailModule {
    pub enabled: bool,
    pub mode: TrailMode,
    pub ratio: f32,
    pub lifetime: MinMaxCurve,
    pub min_vertex_distance: f32,
    pub world_space: bool,
    pub die_with_particles: bool,
    pub texture_mode: TrailTextureMode,
    pub size_affects_width: bool,
    pub size_affects_lifetime: bool,
    pub inherit_particle_color: bool,
    pub color_over_lifetime: MinMaxGradient,
    pub width_over_trail: MinMaxCurve,
    pub color_over_trail: MinMaxGradient,
    pub generate_lighting_data: bool,
    pub shadow_bias: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrailMode { PerParticle, Ribbon }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrailTextureMode { Stretch, Tile, DistributedPerSegment, RepeatPerSegment }

impl Default for TrailModule {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: TrailMode::PerParticle,
            ratio: 1.0,
            lifetime: MinMaxCurve::constant(1.0),
            min_vertex_distance: 0.1,
            world_space: false,
            die_with_particles: true,
            texture_mode: TrailTextureMode::Stretch,
            size_affects_width: true,
            size_affects_lifetime: false,
            inherit_particle_color: true,
            color_over_lifetime: MinMaxGradient::solid(Vec4::ONE),
            width_over_trail: MinMaxCurve::constant(1.0),
            color_over_trail: MinMaxGradient::solid(Vec4::ONE),
            generate_lighting_data: false,
            shadow_bias: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Renderer module
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode { Billboard, StretchedBillboard, HorizontalBillboard, VerticalBillboard, Mesh, None }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BillboardAlignment { View, World, Local, Facing, Velocity }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortMode { None, ByDistance, OldestInFront, YoungestInFront }

#[derive(Debug, Clone)]
pub struct RendererModule {
    pub render_mode: RenderMode,
    pub material_id: Option<u64>,
    pub mesh_id: Option<u64>,
    pub sort_mode: SortMode,
    pub sorting_fudge: f32,
    pub min_particle_size: f32,
    pub max_particle_size: f32,
    pub billboard_alignment: BillboardAlignment,
    pub flip_u: f32,
    pub flip_v: f32,
    pub enable_gpu_instancing: bool,
    pub shadow_casting: bool,
    pub receive_shadows: bool,
    pub motion_vectors: bool,
    pub allow_roll: bool,
    pub pivot: Vec3,
    pub masking_layer: u32,
    pub apply_active_color_space: bool,
}

impl Default for RendererModule {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::Billboard,
            material_id: None,
            mesh_id: None,
            sort_mode: SortMode::None,
            sorting_fudge: 0.0,
            min_particle_size: 0.0,
            max_particle_size: 0.5,
            billboard_alignment: BillboardAlignment::View,
            flip_u: 0.0,
            flip_v: 0.0,
            enable_gpu_instancing: true,
            shadow_casting: false,
            receive_shadows: false,
            motion_vectors: false,
            allow_roll: true,
            pivot: Vec3::ZERO,
            masking_layer: 0,
            apply_active_color_space: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Particle system definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ParticleSystem {
    pub id: u64,
    pub name: String,
    pub main: MainModule,
    pub emission: EmissionModule,
    pub shape: EmitterShape,
    pub velocity_over_lifetime: VelocityOverLifetimeModule,
    pub force_over_lifetime: ForceOverLifetimeModule,
    pub color_over_lifetime: ColorOverLifetimeModule,
    pub size_over_lifetime: SizeOverLifetimeModule,
    pub rotation_over_lifetime: RotationOverLifetimeModule,
    pub collision: CollisionModule,
    pub trails: TrailModule,
    pub renderer: RendererModule,
    pub position: Vec3,
    pub enabled: bool,
    pub sub_systems: Vec<u64>,
    pub tags: Vec<String>,
}

impl ParticleSystem {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            main: MainModule::default(),
            emission: EmissionModule::default(),
            shape: EmitterShape::Cone { angle: 25.0, radius: 0.1, length: 0.0, emit_from_volume: false },
            velocity_over_lifetime: VelocityOverLifetimeModule::default(),
            force_over_lifetime: ForceOverLifetimeModule::default(),
            color_over_lifetime: ColorOverLifetimeModule::default(),
            size_over_lifetime: SizeOverLifetimeModule::default(),
            rotation_over_lifetime: RotationOverLifetimeModule::default(),
            collision: CollisionModule::default(),
            trails: TrailModule::default(),
            renderer: RendererModule::default(),
            position: Vec3::ZERO,
            enabled: true,
            sub_systems: Vec::new(),
            tags: Vec::new(),
        }
    }

    pub fn fire() -> Self {
        let mut ps = Self::new(1, "Fire");
        ps.main.start_lifetime = MinMaxCurve::random_range(0.5, 1.5);
        ps.main.start_speed = MinMaxCurve::random_range(0.5, 2.0);
        ps.main.start_size = MinMaxCurve::random_range(0.1, 0.5);
        ps.main.start_color = MinMaxGradient::two_color(
            Vec4::new(1.0, 0.5, 0.0, 1.0),
            Vec4::new(1.0, 0.8, 0.0, 1.0),
        );
        ps.main.gravity_modifier = MinMaxCurve::constant(-0.2);
        ps.emission.rate_over_time = MinMaxCurve::constant(30.0);
        ps.shape = EmitterShape::Cone { angle: 15.0, radius: 0.2, length: 0.0, emit_from_volume: false };
        ps.color_over_lifetime.color = MinMaxGradient::two_color(
            Vec4::new(1.0, 0.5, 0.0, 1.0),
            Vec4::new(0.2, 0.2, 0.2, 0.0),
        );
        ps
    }

    pub fn smoke() -> Self {
        let mut ps = Self::new(2, "Smoke");
        ps.main.start_lifetime = MinMaxCurve::random_range(2.0, 5.0);
        ps.main.start_speed = MinMaxCurve::random_range(0.2, 0.8);
        ps.main.start_size = MinMaxCurve::random_range(0.3, 1.0);
        ps.main.start_color = MinMaxGradient::solid(Vec4::new(0.4, 0.4, 0.4, 0.5));
        ps.emission.rate_over_time = MinMaxCurve::constant(5.0);
        ps.size_over_lifetime.size = MinMaxCurve::Curve(vec![
            Vec2::new(0.0, 0.3), Vec2::new(0.5, 0.8), Vec2::new(1.0, 2.0),
        ]);
        ps.rotation_over_lifetime.enabled = true;
        ps
    }

    pub fn sparks() -> Self {
        let mut ps = Self::new(3, "Sparks");
        ps.main.start_lifetime = MinMaxCurve::random_range(0.5, 1.5);
        ps.main.start_speed = MinMaxCurve::random_range(3.0, 8.0);
        ps.main.start_size = MinMaxCurve::random_range(0.02, 0.06);
        ps.main.start_color = MinMaxGradient::two_color(
            Vec4::new(1.0, 1.0, 0.5, 1.0),
            Vec4::new(1.0, 0.3, 0.0, 1.0),
        );
        ps.main.gravity_modifier = MinMaxCurve::constant(1.0);
        ps.emission.rate_over_time = MinMaxCurve::constant(0.0);
        ps.emission.bursts.push(EmissionBurst {
            time: 0.0,
            count: MinMaxCurve::random_range(50.0, 100.0),
            cycles: 1,
            interval: 0.0,
            probability: 1.0,
        });
        ps.shape = EmitterShape::Sphere { radius: 0.1, thickness: 0.0 };
        ps.collision.enabled = true;
        ps
    }

    pub fn rain() -> Self {
        let mut ps = Self::new(4, "Rain");
        ps.main.start_lifetime = MinMaxCurve::constant(2.0);
        ps.main.start_speed = MinMaxCurve::random_range(5.0, 10.0);
        ps.main.start_size = MinMaxCurve::random_range(0.01, 0.03);
        ps.main.start_color = MinMaxGradient::solid(Vec4::new(0.6, 0.7, 1.0, 0.6));
        ps.main.gravity_modifier = MinMaxCurve::constant(1.0);
        ps.main.max_particles = 5000;
        ps.emission.rate_over_time = MinMaxCurve::constant(200.0);
        ps.shape = EmitterShape::Box { half_extents: Vec3::new(20.0, 0.1, 20.0), emit_from_surface: true };
        ps.renderer.render_mode = RenderMode::StretchedBillboard;
        ps.collision.enabled = true;
        ps
    }

    pub fn estimated_particle_count(&self) -> f32 {
        let rate = self.emission.rate_over_time.evaluate(0.0, 0.5);
        let lifetime_avg = (self.main.start_lifetime.evaluate(0.0, 0.0) +
                            self.main.start_lifetime.evaluate(0.0, 1.0)) * 0.5;
        (rate * lifetime_avg).min(self.main.max_particles as f32)
    }
}

// ---------------------------------------------------------------------------
// Particle editor state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParticleEditorTab {
    Emitter,
    Modules,
    Preview,
    Timeline,
    Curves,
    Gradient,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParticlePreviewMode {
    Solid,
    Wireframe,
    Velocity,
    Age,
    Count,
}

#[derive(Debug, Clone)]
pub struct ParticleEditor {
    pub systems: Vec<ParticleSystem>,
    pub selected_system: Option<usize>,
    pub active_tab: ParticleEditorTab,
    pub preview_mode: ParticlePreviewMode,
    pub preview_time: f32,
    pub preview_playing: bool,
    pub preview_loop: bool,
    pub preview_speed: f32,
    pub show_bounds: bool,
    pub show_emitter_shape: bool,
    pub camera_distance: f32,
    pub search_query: String,
    pub next_id: u64,
}

impl ParticleEditor {
    pub fn new() -> Self {
        let mut ed = Self {
            systems: Vec::new(),
            selected_system: None,
            active_tab: ParticleEditorTab::Emitter,
            preview_mode: ParticlePreviewMode::Solid,
            preview_time: 0.0,
            preview_playing: true,
            preview_loop: true,
            preview_speed: 1.0,
            show_bounds: false,
            show_emitter_shape: true,
            camera_distance: 10.0,
            search_query: String::new(),
            next_id: 5,
        };
        ed.systems.push(ParticleSystem::fire());
        ed.systems.push(ParticleSystem::smoke());
        ed.systems.push(ParticleSystem::sparks());
        ed.systems.push(ParticleSystem::rain());
        ed.selected_system = Some(0);
        ed
    }

    pub fn add_system(&mut self, name: impl Into<String>) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let ps = ParticleSystem::new(id, name);
        let idx = self.systems.len();
        self.systems.push(ps);
        idx
    }

    pub fn remove_system(&mut self, idx: usize) {
        if idx < self.systems.len() {
            self.systems.remove(idx);
            if self.selected_system == Some(idx) {
                self.selected_system = if self.systems.is_empty() { None } else { Some(0) };
            }
        }
    }

    pub fn duplicate_system(&mut self, idx: usize) -> Option<usize> {
        if idx >= self.systems.len() { return None; }
        let mut copy = self.systems[idx].clone();
        copy.id = self.next_id;
        self.next_id += 1;
        copy.name = format!("{}_copy", copy.name);
        let new_idx = self.systems.len();
        self.systems.push(copy);
        Some(new_idx)
    }

    pub fn update(&mut self, dt: f32) {
        if self.preview_playing {
            self.preview_time += dt * self.preview_speed;
            if let Some(idx) = self.selected_system {
                let duration = self.systems[idx].main.duration;
                if self.preview_time >= duration {
                    if self.preview_loop {
                        self.preview_time = self.preview_time % duration;
                    } else {
                        self.preview_time = duration;
                        self.preview_playing = false;
                    }
                }
            }
        }
    }

    pub fn selected_system(&self) -> Option<&ParticleSystem> {
        self.selected_system.and_then(|i| self.systems.get(i))
    }

    pub fn selected_system_mut(&mut self) -> Option<&mut ParticleSystem> {
        self.selected_system.and_then(|i| self.systems.get_mut(i))
    }

    pub fn search_systems(&self, query: &str) -> Vec<usize> {
        let q = query.to_lowercase();
        self.systems.iter().enumerate()
            .filter(|(_, s)| s.name.to_lowercase().contains(&q) || s.tags.iter().any(|t| t.to_lowercase().contains(&q)))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn total_estimated_particles(&self) -> f32 {
        self.systems.iter().filter(|s| s.enabled).map(|s| s.estimated_particle_count()).sum()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_max_curve() {
        let c = MinMaxCurve::random_range(1.0, 3.0);
        let v = c.evaluate(0.5, 0.5);
        assert!((v - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_gradient() {
        let g = MinMaxGradient::two_color(Vec4::ZERO, Vec4::ONE);
        let mid = g.evaluate(0.5);
        assert!((mid.x - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_emitter_shape_sample() {
        let shape = EmitterShape::Sphere { radius: 1.0, thickness: 0.0 };
        let (pos, dir) = shape.sample_position([0.3, 0.6, 0.9, 0.1]);
        assert!((pos.length() - 1.0).abs() < 0.01);
        assert!((dir.length() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_particle_system_presets() {
        let fire = ParticleSystem::fire();
        assert!(fire.estimated_particle_count() > 0.0);
        let sparks = ParticleSystem::sparks();
        assert!(!sparks.emission.bursts.is_empty());
    }

    #[test]
    fn test_editor() {
        let mut ed = ParticleEditor::new();
        assert_eq!(ed.systems.len(), 4);
        ed.update(0.016);
        let idx = ed.add_system("TestPS");
        assert!(ed.systems[idx].name == "TestPS");
    }
}
