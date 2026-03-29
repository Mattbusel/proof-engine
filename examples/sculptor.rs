//! Density Sculptor — Visual bone editor for GPU density entities.
//!
//! Place, resize, and color bones visually. See 25M particles update in real-time.
//!
//! Controls:
//!   Left-click drag:  Place/move bone endpoint
//!   Right-click:      Select bone
//!   Scroll:           Adjust selected bone radius
//!   Delete:           Remove selected bone
//!   S:                Toggle symmetry mode
//!   Space:            Toggle preview (GPU particles vs wireframe)
//!
//! Run: cargo run --release --example sculptor

use proof_engine::prelude::*;
use proof_engine::particle::gpu_density::*;
use std::f32::consts::{PI, TAU};

const MAX_SCULPT_BONES: usize = 16;
const PREVIEW_PARTICLES: u32 = 5_000_000;

// ── Color palette ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
struct BoneColor {
    r: f32, g: f32, b: f32,
    name: &'static str,
}

const PALETTE: &[BoneColor] = &[
    BoneColor { r: 0.85, g: 0.65, b: 0.50, name: "Skin Light" },
    BoneColor { r: 0.70, g: 0.52, b: 0.40, name: "Skin Dark" },
    BoneColor { r: 0.55, g: 0.40, b: 0.30, name: "Skin Deep" },
    BoneColor { r: 0.15, g: 0.10, b: 0.08, name: "Hair Dark" },
    BoneColor { r: 0.50, g: 0.35, b: 0.15, name: "Hair Brown" },
    BoneColor { r: 0.85, g: 0.75, b: 0.50, name: "Hair Blonde" },
    BoneColor { r: 0.20, g: 0.25, b: 0.50, name: "Cloth Blue" },
    BoneColor { r: 0.50, g: 0.15, b: 0.15, name: "Cloth Red" },
    BoneColor { r: 0.15, g: 0.40, b: 0.15, name: "Cloth Green" },
    BoneColor { r: 0.10, g: 0.10, b: 0.12, name: "Cloth Black" },
    BoneColor { r: 0.80, g: 0.78, b: 0.70, name: "Cloth White" },
    BoneColor { r: 0.20, g: 0.15, b: 0.12, name: "Leather" },
    BoneColor { r: 0.40, g: 0.42, b: 0.45, name: "Metal" },
    BoneColor { r: 0.30, g: 0.50, b: 1.00, name: "Energy Blue" },
    BoneColor { r: 1.00, g: 0.30, b: 0.15, name: "Energy Red" },
    BoneColor { r: 0.20, g: 1.00, b: 0.40, name: "Energy Green" },
];

// ── Sculpt bone ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct SculptBone {
    start: Vec2,
    end: Vec2,
    radius: f32,
    density: f32,
    color_idx: usize,
    name: String,
}

impl SculptBone {
    fn new(start: Vec2, end: Vec2, radius: f32, name: &str) -> Self {
        Self {
            start, end, radius, density: 2.0,
            color_idx: 0, name: name.to_string(),
        }
    }

    fn midpoint(&self) -> Vec2 { (self.start + self.end) * 0.5 }
    fn length(&self) -> f32 { (self.end - self.start).length() }

    fn to_gpu_bone(&self) -> GpuBone {
        let c = &PALETTE[self.color_idx];
        let len = self.length().max(0.1);
        GpuBone {
            start_end: [self.start.x, self.start.y, self.end.x, self.end.y],
            params: [self.radius, self.density, self.density * len * self.radius, 0.0],
            color: [c.r, c.g, c.b, 1.0],
        }
    }
}

// ── Sculptor state ─────────────────────────────────────────────────────────

struct SculptorState {
    bones: Vec<SculptBone>,
    selected: Option<usize>,
    /// Which endpoint is being dragged: 0=start, 1=end, 2=move whole bone
    dragging: Option<(usize, u8)>,
    symmetry: bool,
    show_preview: bool,
    preview_particles: u32,
    entity_scale: f32,
    base_color: Vec4,
    /// Mode: 0=place bones, 1=select/edit, 2=color
    mode: u8,
    new_bone_start: Option<Vec2>,
    density_falloff: f32,
}

impl SculptorState {
    fn new() -> Self {
        Self {
            bones: Vec::new(),
            selected: None,
            dragging: None,
            symmetry: true,
            show_preview: true,
            preview_particles: PREVIEW_PARTICLES,
            entity_scale: 2.0,
            base_color: Vec4::new(0.5, 0.5, 0.5, 1.0),
            mode: 0,
            new_bone_start: None,
            density_falloff: 2.5,
        }
    }

    fn load_humanoid(&mut self) {
        self.bones.clear();
        // Standard humanoid template
        self.bones.push(SculptBone { start: Vec2::new(0.0, -1.10), end: Vec2::new(0.0, -0.90), radius: 0.13, density: 4.0, color_idx: 0, name: "Head".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.0, -1.18), end: Vec2::new(0.0, -1.02), radius: 0.14, density: 1.5, color_idx: 3, name: "Hair".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.0, -0.90), end: Vec2::new(0.0, -0.80), radius: 0.06, density: 1.5, color_idx: 1, name: "Neck".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.0, -0.80), end: Vec2::new(0.0, -0.50), radius: 0.24, density: 3.5, color_idx: 6, name: "Chest".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.0, -0.50), end: Vec2::new(0.0, -0.20), radius: 0.20, density: 2.5, color_idx: 9, name: "Torso".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.0, -0.20), end: Vec2::new(0.0, -0.05), radius: 0.22, density: 2.0, color_idx: 9, name: "Hips".into() });
        self.bones.push(SculptBone { start: Vec2::new(-0.28, -0.78), end: Vec2::new(-0.45, -0.50), radius: 0.07, density: 1.2, color_idx: 0, name: "L Arm".into() });
        self.bones.push(SculptBone { start: Vec2::new(-0.45, -0.50), end: Vec2::new(-0.50, -0.25), radius: 0.055, density: 0.9, color_idx: 1, name: "L Forearm".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.28, -0.78), end: Vec2::new(0.45, -0.50), radius: 0.07, density: 1.2, color_idx: 0, name: "R Arm".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.45, -0.50), end: Vec2::new(0.50, -0.25), radius: 0.055, density: 0.9, color_idx: 1, name: "R Forearm".into() });
        self.bones.push(SculptBone { start: Vec2::new(-0.10, -0.05), end: Vec2::new(-0.12, 0.30), radius: 0.10, density: 1.5, color_idx: 9, name: "L Thigh".into() });
        self.bones.push(SculptBone { start: Vec2::new(-0.12, 0.30), end: Vec2::new(-0.13, 0.65), radius: 0.07, density: 1.0, color_idx: 11, name: "L Shin".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.10, -0.05), end: Vec2::new(0.12, 0.30), radius: 0.10, density: 1.5, color_idx: 9, name: "R Thigh".into() });
        self.bones.push(SculptBone { start: Vec2::new(0.12, 0.30), end: Vec2::new(0.13, 0.65), radius: 0.07, density: 1.0, color_idx: 11, name: "R Shin".into() });
    }

    fn build_gpu_data(&self, pos: Vec3, time: f32) -> GpuDensityEntityData {
        let mut gpu_bones = [GpuBone { start_end: [0.0; 4], params: [0.0; 4], color: [0.0; 4] }; MAX_BONES];
        for (i, bone) in self.bones.iter().take(MAX_BONES).enumerate() {
            gpu_bones[i] = bone.to_gpu_bone();
        }

        GpuDensityEntityData {
            position_scale: [pos.x, pos.y, pos.z, self.entity_scale],
            color: self.base_color.to_array(),
            params: [1.0, time * 1.2, 0.015, self.density_falloff],
            params2: [
                self.bones.len().min(MAX_BONES) as f32,
                self.preview_particles as f32,
                0.05, // jitter
                10.0, // binding
            ],
            bones: gpu_bones,
        }
    }

    fn to_toml(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("# Density Sculptor export\n"));
        s.push_str(&format!("scale = {:.2}\n", self.entity_scale));
        s.push_str(&format!("density_falloff = {:.2}\n", self.density_falloff));
        s.push_str(&format!("base_color = [{:.2}, {:.2}, {:.2}, {:.2}]\n\n",
            self.base_color.x, self.base_color.y, self.base_color.z, self.base_color.w));

        for bone in &self.bones {
            let c = &PALETTE[bone.color_idx];
            s.push_str(&format!("[[bone]]\n"));
            s.push_str(&format!("name = \"{}\"\n", bone.name));
            s.push_str(&format!("start = [{:.3}, {:.3}]\n", bone.start.x, bone.start.y));
            s.push_str(&format!("end = [{:.3}, {:.3}]\n", bone.end.x, bone.end.y));
            s.push_str(&format!("radius = {:.3}\n", bone.radius));
            s.push_str(&format!("density = {:.2}\n", bone.density));
            s.push_str(&format!("color = \"{}\"\n\n", c.name));
        }
        s
    }
}

fn main() {
    env_logger::init();
    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Density Sculptor".to_string(),
        window_width: 1600, window_height: 1000,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true, bloom_intensity: 0.8,
            chromatic_aberration: 0.0, film_grain: 0.0,
            ..Default::default()
        },
        ..Default::default()
    });

    let mut state = SculptorState::new();
    state.load_humanoid(); // Start with humanoid template

    let mut gpu_initialized = false;
    let mut time = 0.0f32;

    engine.run(move |engine, dt| {
        if !gpu_initialized {
            engine.init_gpu_density(PREVIEW_PARTICLES);
            gpu_initialized = true;
        }
        time += dt;

        // ── Input handling ──────────────────────────────────────────────

        let (ww, wh) = engine.window_size();
        let aspect = ww as f32 / wh as f32;
        let cam_z = engine.camera.position.z.position;
        let fov = engine.camera.fov.position.to_radians();
        let half_h = (fov * 0.5).tan() * cam_z.abs();
        let half_w = half_h * aspect;

        // Mouse to world coordinates
        let mx = engine.input.mouse_x;
        let my = engine.input.mouse_y;
        let ndc_x = (mx / ww as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (my / wh as f32) * 2.0; // Y flip
        let world_x = ndc_x * half_w;
        let world_y = ndc_y * half_h;
        let mouse_world = Vec2::new(world_x, world_y);

        // Bone placement mode: click to start bone, drag to end
        if state.mode == 0 {
            if engine.input.mouse_left_just_pressed {
                state.new_bone_start = Some(mouse_world);
            }
            if engine.input.mouse_left_just_released {
                if let Some(start) = state.new_bone_start.take() {
                    let end = mouse_world;
                    if (end - start).length() > 0.02 && state.bones.len() < MAX_SCULPT_BONES {
                        let name = format!("Bone {}", state.bones.len());
                        let bone = SculptBone::new(start, end, 0.08, &name);
                        state.bones.push(bone);

                        // Symmetry: auto-mirror
                        if state.symmetry && state.bones.len() < MAX_SCULPT_BONES {
                            let mirror_name = format!("Bone {} (mirror)", state.bones.len());
                            let mirror = SculptBone::new(
                                Vec2::new(-start.x, start.y),
                                Vec2::new(-end.x, end.y),
                                0.08, &mirror_name,
                            );
                            state.bones.push(mirror);
                        }

                        state.selected = Some(state.bones.len() - 1);
                    }
                }
            }
        }

        // Select mode: right-click nearest bone
        if engine.input.mouse_right_just_pressed {
            let mut best_dist = f32::MAX;
            let mut best_idx = None;
            for (i, bone) in state.bones.iter().enumerate() {
                let mid = bone.midpoint();
                let d = (mid - mouse_world / state.entity_scale).length();
                if d < best_dist {
                    best_dist = d;
                    best_idx = Some(i);
                }
            }
            if best_dist < 0.3 {
                state.selected = best_idx;
            }
        }

        // Scroll to adjust radius of selected bone
        let scroll = engine.input.scroll_delta;
        if scroll.abs() > 0.001 {
            if let Some(idx) = state.selected {
                state.bones[idx].radius = (state.bones[idx].radius + scroll * 0.005).max(0.01).min(0.5);
            }
        }

        // Delete key removes selected bone
        if engine.input.just_pressed(Key::Delete) || engine.input.just_pressed(Key::Backspace) {
            if let Some(idx) = state.selected.take() {
                if idx < state.bones.len() {
                    state.bones.remove(idx);
                }
            }
        }

        // S toggles symmetry
        if engine.input.just_pressed(Key::S) {
            state.symmetry = !state.symmetry;
        }

        // Space toggles preview
        if engine.input.just_pressed(Key::Space) {
            state.show_preview = !state.show_preview;
        }

        // Number keys switch color on selected bone
        if let Some(idx) = state.selected {
            let digit_keys = [
                Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4,
                Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9,
            ];
            for (ci, &key) in digit_keys.iter().enumerate() {
                if engine.input.just_pressed(key) && ci < PALETTE.len() {
                    state.bones[idx].color_idx = ci;
                }
            }
        }

        // ── GPU density preview ─────────────────────────────────────────

        if state.show_preview && !state.bones.is_empty() {
            let gpu_data = state.build_gpu_data(Vec3::new(0.0, 0.3, 0.0), time);
            engine.queue_gpu_density_entity(gpu_data);
        }

        // ── Wireframe bone overlay (always visible) ─────────────────────

        for (i, bone) in state.bones.iter().enumerate() {
            let selected = state.selected == Some(i);
            let c = &PALETTE[bone.color_idx];
            let alpha = if selected { 0.9 } else { 0.4 };
            let scale = state.entity_scale;

            // Draw bone line as dots
            let steps = ((bone.length() / 0.03).ceil() as usize).max(2);
            for s in 0..steps {
                let t = s as f32 / (steps - 1) as f32;
                let p = bone.start + (bone.end - bone.start) * t;
                engine.spawn_glyph(Glyph {
                    character: if selected { '#' } else { '.' },
                    scale: Vec2::splat(if selected { 0.06 } else { 0.04 }),
                    position: Vec3::new(p.x * scale, p.y * scale + 0.3, 0.5),
                    color: Vec4::new(c.r, c.g, c.b, alpha),
                    emission: if selected { 1.5 } else { 0.3 },
                    mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::UI, ..Default::default()
                });
            }

            // Draw radius circles at endpoints
            if selected {
                for endpoint in [bone.start, bone.end] {
                    for a in 0..16 {
                        let angle = (a as f32 / 16.0) * TAU;
                        let rx = endpoint.x + angle.cos() * bone.radius;
                        let ry = endpoint.y + angle.sin() * bone.radius;
                        engine.spawn_glyph(Glyph {
                            character: '.', scale: Vec2::splat(0.03),
                            position: Vec3::new(rx * scale, ry * scale + 0.3, 0.5),
                            color: Vec4::new(1.0, 1.0, 1.0, 0.5),
                            emission: 0.5, mass: 0.0, lifetime: dt * 1.5,
                            layer: RenderLayer::UI, ..Default::default()
                        });
                    }
                }
            }
        }

        // ── Placement preview (dotted line while dragging) ──────────────

        if state.mode == 0 {
            if let Some(start) = state.new_bone_start {
                if engine.input.mouse_left {
                    let end = mouse_world;
                    let steps = 10;
                    for s in 0..=steps {
                        let t = s as f32 / steps as f32;
                        let p = start + (end - start) * t;
                        engine.spawn_glyph(Glyph {
                            character: '+', scale: Vec2::splat(0.05),
                            position: Vec3::new(p.x, p.y, 0.6),
                            color: Vec4::new(1.0, 1.0, 0.5, 0.8),
                            emission: 1.0, mass: 0.0, lifetime: dt * 1.5,
                            layer: RenderLayer::UI, ..Default::default()
                        });
                    }
                }
            }
        }

        // ── HUD text ────────────────────────────────────────────────────

        let mode_name = match state.mode { 0 => "PLACE", 1 => "SELECT", 2 => "COLOR", _ => "?" };
        let sym_str = if state.symmetry { "ON" } else { "OFF" };
        let sel_str = if let Some(idx) = state.selected {
            let b = &state.bones[idx];
            format!("{} (r={:.3} d={:.1} {})", b.name, b.radius, b.density, PALETTE[b.color_idx].name)
        } else {
            "None".to_string()
        };

        let hud_lines = [
            format!("Bones: {}/{}  Mode: {}  Symmetry: {}  Preview: {}",
                state.bones.len(), MAX_SCULPT_BONES, mode_name, sym_str,
                if state.show_preview { "ON" } else { "OFF" }),
            format!("Selected: {}", sel_str),
            "LClick: place bone  RClick: select  Scroll: radius  Del: remove  S: symmetry  Space: preview".to_string(),
            "0-9: set color on selected bone  Export: E".to_string(),
        ];

        for (line_i, text) in hud_lines.iter().enumerate() {
            for (ci, ch) in text.chars().enumerate() {
                if ch == ' ' { continue; }
                let x = -half_w + 0.15 + ci as f32 * 0.09;
                let y = half_h - 0.2 - line_i as f32 * 0.22;
                engine.spawn_glyph(Glyph {
                    character: ch, scale: Vec2::splat(0.08),
                    position: Vec3::new(x, y, 1.0),
                    color: Vec4::new(0.8, 0.8, 0.8, 0.9),
                    emission: 0.3, mass: 0.0, lifetime: dt * 1.5,
                    layer: RenderLayer::UI, ..Default::default()
                });
            }
        }

        // ── Export on E key ─────────────────────────────────────────────

        if engine.input.just_pressed(Key::E) {
            let toml = state.to_toml();
            let path = "density_model.toml";
            if std::fs::write(path, &toml).is_ok() {
                log::info!("Exported to {path}");
            }
        }
    });
}
