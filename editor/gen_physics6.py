import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 6
// ============================================================

// ─── Particle System Physics ──────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicsParticle {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub age: f32,
    pub lifetime: f32,
    pub mass: f32,
    pub radius: f32,
    pub restitution: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ParticleEmitterPhysics {
    pub name: String,
    pub position: [f32; 2],
    pub emission_rate: f32,
    pub initial_velocity: [f32; 2],
    pub velocity_spread: f32,
    pub particle_lifetime: f32,
    pub particle_mass: f32,
    pub particle_radius: f32,
    pub gravity_scale: f32,
    pub drag: f32,
    pub bounce: bool,
    pub restitution: f32,
    pub max_particles: usize,
    pub active: bool,
    pub burst_count: u32,
    pub burst_on_spawn: bool,
    pub emit_cone_angle: f32,
    pub emit_direction: [f32; 2],
}

impl Default for ParticleEmitterPhysics {
    fn default() -> Self {
        Self { name: "Emitter_0".into(), position: [0.0, 0.0], emission_rate: 20.0, initial_velocity: [0.0, 5.0], velocity_spread: 1.5, particle_lifetime: 3.0, particle_mass: 0.01, particle_radius: 0.1, gravity_scale: 1.0, drag: 0.02, bounce: true, restitution: 0.4, max_particles: 200, active: true, burst_count: 0, burst_on_spawn: false, emit_cone_angle: 30.0, emit_direction: [0.0, 1.0] }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsParticleSystemState {
    pub emitters: Vec<ParticleEmitterPhysics>,
    pub particles: Vec<PhysicsParticle>,
    pub selected_emitter: Option<usize>,
    pub simulating: bool,
    pub sim_time: f32,
    pub gravity: [f32; 2],
    pub world_bounds: Option<[f32; 4]>,
}

pub fn show_physics_particle_system(ui: &mut egui::Ui, state: &mut PhysicsParticleSystemState) {
    ui.heading("Physics Particle System");
    ui.separator();

    if state.emitters.is_empty() {
        state.emitters.push(ParticleEmitterPhysics::default());
        state.gravity = [0.0, -9.81];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Emitter").clicked() {
            let mut e = ParticleEmitterPhysics::default();
            e.name = format!("Emitter_{}", state.emitters.len());
            state.emitters.push(e);
        }
        let sim_lbl = if state.simulating { "⏸ Pause" } else { "▶ Simulate" };
        if ui.button(sim_lbl).clicked() { state.simulating = !state.simulating; }
        if ui.button("Clear Particles").clicked() { state.particles.clear(); }
        ui.label(format!("Particles: {}", state.particles.len()));
    });

    ui.horizontal(|ui| {
        ui.label("Gravity:");
        ui.add(egui::DragValue::new(&mut state.gravity[0]).prefix("x:").speed(0.05));
        ui.add(egui::DragValue::new(&mut state.gravity[1]).prefix("y:").speed(0.05));
    });

    for (i, e) in state.emitters.iter().enumerate() {
        let sel = state.selected_emitter == Some(i);
        let acol = if e.active { Color32::GREEN } else { Color32::DARK_GRAY };
        ui.horizontal(|ui| {
            ui.colored_label(acol, "◉");
            if ui.selectable_label(sel, &e.name).clicked() { state.selected_emitter = Some(i); }
            ui.label(format!("rate:{:.0}/s bounce:{}", e.emission_rate, e.bounce));
        });
    }

    if let Some(idx) = state.selected_emitter {
        if idx < state.emitters.len() {
            let e = &mut state.emitters[idx];
            ui.separator();
            ui.group(|ui| {
                ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut e.name); ui.checkbox(&mut e.active, "Active"); });
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    ui.add(egui::DragValue::new(&mut e.position[0]).prefix("x:").speed(0.05));
                    ui.add(egui::DragValue::new(&mut e.position[1]).prefix("y:").speed(0.05));
                });
                ui.horizontal(|ui| {
                    ui.label("Emit Rate:");
                    ui.add(egui::DragValue::new(&mut e.emission_rate).speed(0.5).clamp_range(0.0..=1000.0).suffix("/s"));
                    ui.label("Cone:");
                    ui.add(egui::Slider::new(&mut e.emit_cone_angle, 0.0..=180.0).suffix("°"));
                });
                ui.horizontal(|ui| {
                    ui.label("Init Vel:");
                    ui.add(egui::DragValue::new(&mut e.initial_velocity[0]).prefix("vx:").speed(0.1));
                    ui.add(egui::DragValue::new(&mut e.initial_velocity[1]).prefix("vy:").speed(0.1));
                    ui.label("Spread:");
                    ui.add(egui::DragValue::new(&mut e.velocity_spread).speed(0.05));
                });
                ui.horizontal(|ui| {
                    ui.label("Lifetime:");
                    ui.add(egui::DragValue::new(&mut e.particle_lifetime).speed(0.05).clamp_range(0.1..=60.0).suffix("s"));
                    ui.label("Mass:");
                    ui.add(egui::DragValue::new(&mut e.particle_mass).speed(0.001).clamp_range(0.0001..=100.0));
                    ui.label("Radius:");
                    ui.add(egui::DragValue::new(&mut e.particle_radius).speed(0.005).clamp_range(0.01..=5.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Gravity Scale:");
                    ui.add(egui::Slider::new(&mut e.gravity_scale, -2.0..=5.0));
                    ui.label("Drag:");
                    ui.add(egui::Slider::new(&mut e.drag, 0.0..=1.0));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut e.bounce, "Bounce");
                    if e.bounce { ui.label("Restitution:"); ui.add(egui::Slider::new(&mut e.restitution, 0.0..=1.0)); }
                    ui.label("Max:");
                    ui.add(egui::DragValue::new(&mut e.max_particles).clamp_range(1..=10000));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut e.burst_on_spawn, "Burst on Spawn");
                    ui.label("Burst Count:");
                    ui.add(egui::DragValue::new(&mut e.burst_count).clamp_range(0..=1000));
                });
            });
        }
    }

    draw_particle_system_preview(ui, state);
}

fn draw_particle_system_preview(ui: &mut egui::Ui, state: &PhysicsParticleSystemState) {
    let desired = Vec2::new(ui.available_width().min(500.0), 220.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let world_to_screen = |wx: f32, wy: f32| -> Pos2 {
        Pos2::new(rect.center().x + wx * 22.0, rect.center().y - wy * 22.0)
    };

    // Ground line
    p.line_segment([world_to_screen(-10.0, -4.0), world_to_screen(10.0, -4.0)], Stroke::new(1.5, Color32::from_rgb(60, 80, 60)));

    // Draw emitters
    for (i, e) in state.emitters.iter().enumerate() {
        if !e.active { continue; }
        let es = world_to_screen(e.position[0], e.position[1]);
        let ec = if state.selected_emitter == Some(i) { Color32::WHITE } else { Color32::GRAY };
        p.circle_filled(es, 4.0, ec);

        // Emit direction cone
        let base_angle = e.emit_direction[0].atan2(e.emit_direction[1]);
        let half_cone = e.emit_cone_angle.to_radians() * 0.5;
        let cone_len = 20.0;
        for ci in &[-1.0f32, 1.0] {
            let a = base_angle + half_cone * ci;
            p.line_segment([es, es + Vec2::new(a.sin(), -a.cos()) * cone_len], Stroke::new(0.8, Color32::from_rgba_unmultiplied(200, 200, 100, 100)));
        }
    }

    // Draw particles
    for particle in state.particles.iter().take(500) {
        let age_frac = (particle.age / particle.lifetime).clamp(0.0, 1.0);
        let ps = world_to_screen(particle.pos[0], particle.pos[1]);
        if ps.x < rect.left() || ps.x > rect.right() || ps.y < rect.top() || ps.y > rect.bottom() { continue; }
        let alpha = ((1.0 - age_frac) * 220.0) as u8;
        let r = (particle.color[0] * 255.0) as u8;
        let g = (particle.color[1] * 255.0) as u8;
        let b = (particle.color[2] * 255.0) as u8;
        let pr = (particle.radius * 22.0).clamp(1.0, 8.0);
        p.circle_filled(ps, pr, Color32::from_rgba_unmultiplied(r, g, b, alpha));
    }

    p.text(Pos2::new(rect.left()+4.0, rect.bottom()-12.0), egui::Align2::LEFT_BOTTOM,
        format!("{} emitters | {} particles | {}",
            state.emitters.len(), state.particles.len(),
            if state.simulating { "SIMULATING" } else { "PAUSED" }),
        FontId::monospace(8.0), Color32::DARK_GRAY);
}

// ─── Wind System ──────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct WindZone {
    pub name: String,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub wind_direction: [f32; 2],
    pub wind_speed: f32,
    pub turbulence_frequency: f32,
    pub turbulence_strength: f32,
    pub affects_particles: bool,
    pub affects_cloth: bool,
    pub affects_soft_bodies: bool,
    pub affects_rigid_bodies: bool,
    pub active: bool,
    pub pulse_enabled: bool,
    pub pulse_frequency: f32,
    pub pulse_amplitude: f32,
    pub gust_enabled: bool,
    pub gust_interval: f32,
    pub gust_speed: f32,
}

impl Default for WindZone {
    fn default() -> Self {
        Self { name: "Wind_0".into(), position: [0.0, 3.0], size: [20.0, 10.0], wind_direction: [1.0, 0.0], wind_speed: 5.0, turbulence_frequency: 0.5, turbulence_strength: 0.3, affects_particles: true, affects_cloth: true, affects_soft_bodies: true, affects_rigid_bodies: false, active: true, pulse_enabled: false, pulse_frequency: 1.0, pulse_amplitude: 0.5, gust_enabled: false, gust_interval: 8.0, gust_speed: 25.0 }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct WindSystemState {
    pub zones: Vec<WindZone>,
    pub selected: Option<usize>,
    pub global_wind_speed: f32,
    pub global_wind_direction: f32,
    pub enable_global_wind: bool,
}

pub fn show_wind_system_editor(ui: &mut egui::Ui, state: &mut WindSystemState) {
    ui.heading("Wind System");
    ui.separator();

    if state.zones.is_empty() {
        state.zones.push(WindZone::default());
    }

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.enable_global_wind, "Global Wind");
        if state.enable_global_wind {
            ui.label("Speed:");
            ui.add(egui::DragValue::new(&mut state.global_wind_speed).speed(0.1).clamp_range(0.0..=100.0));
            ui.label("Direction:");
            ui.add(egui::Slider::new(&mut state.global_wind_direction, -180.0..=180.0).suffix("°"));
        }
    });

    ui.horizontal(|ui| {
        if ui.button("+ Wind Zone").clicked() {
            let mut w = WindZone::default();
            w.name = format!("Wind_{}", state.zones.len());
            state.zones.push(w);
        }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.zones.remove(s); state.selected = None; } }
    });

    for (i, z) in state.zones.iter().enumerate() {
        let sel = state.selected == Some(i);
        let acol = if z.active { Color32::from_rgb(100, 200, 255) } else { Color32::DARK_GRAY };
        ui.horizontal(|ui| {
            ui.colored_label(acol, "≋");
            if ui.selectable_label(sel, format!("{} ({:.1} m/s)", z.name, z.wind_speed)).clicked() { state.selected = Some(i); }
        });
    }

    if let Some(idx) = state.selected {
        if idx < state.zones.len() {
            let z = &mut state.zones[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut z.name); ui.checkbox(&mut z.active, "Active"); });
            ui.horizontal(|ui| {
                ui.label("Position:");
                ui.add(egui::DragValue::new(&mut z.position[0]).prefix("x:").speed(0.1));
                ui.add(egui::DragValue::new(&mut z.position[1]).prefix("y:").speed(0.1));
                ui.label("Size:");
                ui.add(egui::DragValue::new(&mut z.size[0]).prefix("w:").speed(0.1).clamp_range(0.1..=200.0));
                ui.add(egui::DragValue::new(&mut z.size[1]).prefix("h:").speed(0.1).clamp_range(0.1..=200.0));
            });
            ui.horizontal(|ui| {
                ui.label("Direction:");
                ui.add(egui::DragValue::new(&mut z.wind_direction[0]).prefix("dx:").speed(0.01));
                ui.add(egui::DragValue::new(&mut z.wind_direction[1]).prefix("dy:").speed(0.01));
                ui.label("Speed:");
                ui.add(egui::DragValue::new(&mut z.wind_speed).speed(0.1).clamp_range(0.0..=200.0).suffix(" m/s"));
            });
            ui.horizontal(|ui| {
                ui.label("Turb. Freq:");
                ui.add(egui::Slider::new(&mut z.turbulence_frequency, 0.0..=10.0));
                ui.label("Strength:");
                ui.add(egui::Slider::new(&mut z.turbulence_strength, 0.0..=2.0));
            });
            ui.label("Affects:");
            ui.horizontal(|ui| {
                ui.checkbox(&mut z.affects_particles, "Particles");
                ui.checkbox(&mut z.affects_cloth, "Cloth");
                ui.checkbox(&mut z.affects_soft_bodies, "Soft Bodies");
                ui.checkbox(&mut z.affects_rigid_bodies, "Rigid Bodies");
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut z.pulse_enabled, "Pulse");
                if z.pulse_enabled {
                    ui.label("Freq:");
                    ui.add(egui::DragValue::new(&mut z.pulse_frequency).speed(0.05).clamp_range(0.01..=20.0).suffix("Hz"));
                    ui.label("Amp:");
                    ui.add(egui::Slider::new(&mut z.pulse_amplitude, 0.0..=1.0));
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut z.gust_enabled, "Gusts");
                if z.gust_enabled {
                    ui.label("Interval:");
                    ui.add(egui::DragValue::new(&mut z.gust_interval).speed(0.5).clamp_range(0.5..=60.0).suffix("s"));
                    ui.label("Gust Speed:");
                    ui.add(egui::DragValue::new(&mut z.gust_speed).speed(1.0).clamp_range(0.0..=200.0));
                }
            });
        }
    }
    draw_wind_zone_visualization(ui, state);
}

fn draw_wind_zone_visualization(ui: &mut egui::Ui, state: &WindSystemState) {
    let desired = Vec2::new(ui.available_width().min(480.0), 180.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 14, 20));

    let world_to_screen = |wx: f32, wy: f32| -> Pos2 {
        Pos2::new(rect.center().x + wx * 8.0, rect.center().y - wy * 8.0)
    };

    for (zi, z) in state.zones.iter().enumerate() {
        if !z.active { continue; }
        let top_left = world_to_screen(z.position[0] - z.size[0]*0.5, z.position[1] + z.size[1]*0.5);
        let bottom_right = world_to_screen(z.position[0] + z.size[0]*0.5, z.position[1] - z.size[1]*0.5);
        let zone_rect = Rect::from_min_max(top_left, bottom_right).intersect(rect);
        if zone_rect.area() > 0.0 {
            p.rect_filled(zone_rect, 2.0, Color32::from_rgba_unmultiplied(40, 100, 200, 30));
            let border_col = if state.selected == Some(zi) { Color32::WHITE } else { Color32::from_rgb(80, 160, 255) };
            p.rect_stroke(zone_rect, 2.0, Stroke::new(1.0, border_col));
            // Wind arrows
            let rows = 3i32; let cols = 5i32;
            for row in 0..rows {
                for col in 0..cols {
                    let ax = zone_rect.left() + (col as f32 + 0.5) / cols as f32 * zone_rect.width();
                    let ay = zone_rect.top() + (row as f32 + 0.5) / rows as f32 * zone_rect.height();
                    let wind_len = (z.wind_speed / 30.0).clamp(0.1, 1.0) * 20.0;
                    let adx = z.wind_direction[0] * wind_len;
                    let ady = -z.wind_direction[1] * wind_len;
                    p.line_segment([Pos2::new(ax, ay), Pos2::new(ax+adx, ay+ady)], Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 180, 255, 160)));
                }
            }
            p.text(top_left + Vec2::new(4.0, 4.0), egui::Align2::LEFT_TOP, format!("{} {:.0}m/s", z.name, z.wind_speed), FontId::monospace(7.5), Color32::WHITE);
        }
    }
    p.text(Pos2::new(rect.left()+4.0, rect.bottom()-12.0), egui::Align2::LEFT_BOTTOM, "Wind Zone Map", FontId::monospace(7.0), Color32::DARK_GRAY);
}

// ─── Inverse Kinematics Solver ────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct IKBone {
    pub length: f32,
    pub angle: f32,
    pub min_angle: f32,
    pub max_angle: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IKChain {
    pub name: String,
    pub bones: Vec<IKBone>,
    pub root: [f32; 2],
    pub target: [f32; 2],
    pub solver: IKSolverType,
    pub iterations: u32,
    pub tolerance: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum IKSolverType {
    #[default]
    FABRIK,
    CCD,
    Jacobian,
    TwoBone,
}

impl IKSolverType {
    pub fn label(&self) -> &str {
        match self { IKSolverType::FABRIK => "FABRIK", IKSolverType::CCD => "CCD", IKSolverType::Jacobian => "Jacobian", IKSolverType::TwoBone => "Two Bone" }
    }
}

impl IKChain {
    pub fn default_arm() -> Self {
        Self {
            name: "Arm".into(),
            bones: vec![
                IKBone { length: 1.5, angle: 0.0, min_angle: -90.0, max_angle: 90.0 },
                IKBone { length: 1.2, angle: 0.0, min_angle: -150.0, max_angle: 0.0 },
                IKBone { length: 0.8, angle: 0.0, min_angle: -60.0, max_angle: 60.0 },
            ],
            root: [0.0, 2.0],
            target: [2.0, 1.0],
            solver: IKSolverType::FABRIK,
            iterations: 10,
            tolerance: 0.001,
        }
    }
    pub fn default_leg() -> Self {
        Self {
            name: "Leg".into(),
            bones: vec![
                IKBone { length: 1.8, angle: 0.0, min_angle: -60.0, max_angle: 30.0 },
                IKBone { length: 1.6, angle: 0.0, min_angle: -120.0, max_angle: 0.0 },
                IKBone { length: 0.5, angle: 0.0, min_angle: -60.0, max_angle: 60.0 },
            ],
            root: [0.5, 0.0],
            target: [0.5, -3.0],
            solver: IKSolverType::FABRIK,
            iterations: 10,
            tolerance: 0.001,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct IKEditorState {
    pub chains: Vec<IKChain>,
    pub selected: Option<usize>,
    pub selected_bone: Option<usize>,
    pub show_limits: bool,
    pub show_reach_circle: bool,
}

pub fn show_ik_editor(ui: &mut egui::Ui, state: &mut IKEditorState) {
    ui.heading("Inverse Kinematics Editor");
    ui.separator();

    if state.chains.is_empty() {
        state.chains = vec![IKChain::default_arm(), IKChain::default_leg()];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Arm Chain").clicked() { state.chains.push(IKChain::default_arm()); }
        if ui.button("+ Leg Chain").clicked() { state.chains.push(IKChain::default_leg()); }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.chains.remove(s); state.selected = None; } }
        ui.checkbox(&mut state.show_limits, "Show Limits");
        ui.checkbox(&mut state.show_reach_circle, "Reach Circle");
    });

    for (i, chain) in state.chains.iter().enumerate() {
        let sel = state.selected == Some(i);
        if ui.selectable_label(sel, format!("{} ({} bones, {})", chain.name, chain.bones.len(), chain.solver.label())).clicked() { state.selected = Some(i); }
    }

    if let Some(idx) = state.selected {
        if idx < state.chains.len() {
            let chain = &mut state.chains[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut chain.name); });
            ui.horizontal(|ui| {
                ui.label("Solver:");
                for s in &[IKSolverType::FABRIK, IKSolverType::CCD, IKSolverType::TwoBone] {
                    if ui.selectable_label(chain.solver == *s, s.label()).clicked() { chain.solver = s.clone(); }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Iterations:");
                ui.add(egui::DragValue::new(&mut chain.iterations).clamp_range(1..=100));
                ui.label("Tolerance:");
                ui.add(egui::DragValue::new(&mut chain.tolerance).speed(0.0001).clamp_range(0.00001..=0.1));
            });
            ui.horizontal(|ui| {
                ui.label("Root:");
                ui.add(egui::DragValue::new(&mut chain.root[0]).prefix("x:").speed(0.05));
                ui.add(egui::DragValue::new(&mut chain.root[1]).prefix("y:").speed(0.05));
                ui.label("Target:");
                ui.add(egui::DragValue::new(&mut chain.target[0]).prefix("x:").speed(0.05));
                ui.add(egui::DragValue::new(&mut chain.target[1]).prefix("y:").speed(0.05));
            });

            ui.label("Bones:");
            let nb = chain.bones.len();
            for (bi, bone) in chain.bones.iter_mut().enumerate() {
                let bsel = state.selected_bone == Some(bi);
                ui.horizontal(|ui| {
                    if ui.selectable_label(bsel, format!("Bone #{}", bi)).clicked() { state.selected_bone = Some(bi); }
                    ui.add(egui::DragValue::new(&mut bone.length).prefix("L:").speed(0.02).clamp_range(0.05..=10.0));
                    ui.add(egui::Slider::new(&mut bone.min_angle, -180.0..=0.0).prefix("min:").suffix("°"));
                    ui.add(egui::Slider::new(&mut bone.max_angle, 0.0..=180.0).prefix("max:").suffix("°"));
                });
            }
            if ui.button("+ Bone").clicked() { chain.bones.push(IKBone { length: 1.0, angle: 0.0, min_angle: -90.0, max_angle: 90.0 }); }
        }
    }

    draw_ik_chain_preview(ui, state);
}

fn draw_ik_chain_preview(ui: &mut egui::Ui, state: &IKEditorState) {
    let desired = Vec2::new(300.0, 300.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let scale = 40.0;
    let origin = Pos2::new(rect.center().x - 20.0, rect.center().y + 40.0);
    let world_to_screen = |wx: f32, wy: f32| -> Pos2 {
        Pos2::new(origin.x + wx * scale, origin.y - wy * scale)
    };

    if let Some(idx) = state.selected {
        if idx < state.chains.len() {
            let chain = &state.chains[idx];

            // Reach circle
            if state.show_reach_circle {
                let total_length: f32 = chain.bones.iter().map(|b| b.length).sum();
                let root_s = world_to_screen(chain.root[0], chain.root[1]);
                p.circle_stroke(root_s, total_length * scale, Stroke::new(1.0, Color32::from_rgba_unmultiplied(200, 200, 80, 80)));
            }

            // Draw chain (simplified — straight bones from root following angles)
            let mut cur = world_to_screen(chain.root[0], chain.root[1]);
            let mut cumulative_angle = 0.0f32;
            for (bi, bone) in chain.bones.iter().enumerate() {
                cumulative_angle += bone.angle;
                let end = cur + Vec2::new(cumulative_angle.to_radians().cos(), -cumulative_angle.to_radians().sin()) * bone.length * scale;
                let bsel = state.selected_bone == Some(bi);
                let col = if bsel { Color32::WHITE } else { Color32::from_rgb(100, 180, 255) };
                p.line_segment([cur, end], Stroke::new(if bsel { 3.0 } else { 2.0 }, col));
                p.circle_filled(cur, 4.0, col);

                if state.show_limits {
                    let lo = (cumulative_angle + bone.min_angle).to_radians();
                    let hi = (cumulative_angle + bone.max_angle).to_radians();
                    let len = bone.length * scale * 0.5;
                    p.line_segment([cur, cur + Vec2::new(lo.cos(), -lo.sin()) * len], Stroke::new(0.8, Color32::from_rgba_unmultiplied(200, 80, 80, 120)));
                    p.line_segment([cur, cur + Vec2::new(hi.cos(), -hi.sin()) * len], Stroke::new(0.8, Color32::from_rgba_unmultiplied(80, 200, 80, 120)));
                }
                cur = end;
            }
            p.circle_filled(cur, 5.0, Color32::from_rgb(100, 180, 255));

            // Target
            let target_s = world_to_screen(chain.target[0], chain.target[1]);
            p.circle_filled(target_s, 6.0, Color32::from_rgb(255, 120, 60));
            p.circle_stroke(target_s, 8.0, Stroke::new(1.0, Color32::from_rgb(255, 120, 60)));
            p.text(target_s + Vec2::new(10.0, 0.0), egui::Align2::LEFT_CENTER, "Target", FontId::monospace(7.0), Color32::from_rgb(255, 120, 60));

            // Root
            let root_s = world_to_screen(chain.root[0], chain.root[1]);
            p.circle_filled(root_s, 5.0, Color32::GRAY);
            p.text(root_s + Vec2::new(8.0, 0.0), egui::Align2::LEFT_CENTER, "Root", FontId::monospace(7.0), Color32::GRAY);

            p.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP, format!("{} | {}", chain.name, chain.solver.label()), FontId::monospace(8.0), Color32::GRAY);
        }
    }
}

// ─── Spring Network Analyzer ──────────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SpringNetworkState {
    pub node_count: usize,
    pub spring_count: usize,
    pub avg_spring_length: f32,
    pub min_spring_length: f32,
    pub max_spring_length: f32,
    pub total_spring_energy: f32,
    pub stiffest_spring_idx: usize,
    pub weakest_spring_idx: usize,
    pub overextended_count: usize,
    pub compressed_count: usize,
    pub neutral_count: usize,
    pub pinned_node_count: usize,
    pub free_node_count: usize,
    pub analyze_pressed: bool,
}

pub fn show_spring_network_analyzer(ui: &mut egui::Ui, state: &mut SpringNetworkState, soft_state: &SoftBodyEditorState) {
    ui.heading("Spring Network Analyzer");
    ui.separator();

    if ui.button("Analyze Selected Soft Body").clicked() {
        state.analyze_pressed = true;
        if let Some(idx) = soft_state.selected {
            if idx < soft_state.bodies.len() {
                let body = &soft_state.bodies[idx];
                state.node_count = body.nodes.len();
                state.spring_count = body.springs.len();
                state.pinned_node_count = body.nodes.iter().filter(|n| n.pinned).count();
                state.free_node_count = state.node_count - state.pinned_node_count;
                if !body.springs.is_empty() {
                    let lengths: Vec<f32> = body.springs.iter().map(|s| s.rest_length).collect();
                    state.avg_spring_length = lengths.iter().sum::<f32>() / lengths.len() as f32;
                    state.min_spring_length = lengths.iter().cloned().fold(f32::INFINITY, f32::min);
                    state.max_spring_length = lengths.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    state.total_spring_energy = body.springs.iter().enumerate().map(|(i, s)| {
                        let dx = if i < body.springs.len()-1 { body.springs[i+1].rest_length - s.rest_length } else { 0.0 };
                        0.5 * s.stiffness * dx * dx
                    }).sum();
                }
            }
        }
    }

    if state.analyze_pressed {
        ui.separator();
        egui::Grid::new("spring_analysis").num_columns(2).spacing([16.0, 3.0]).show(ui, |ui| {
            ui.label("Nodes:"); ui.label(format!("{}", state.node_count)); ui.end_row();
            ui.label("Springs:"); ui.label(format!("{}", state.spring_count)); ui.end_row();
            ui.label("Pinned Nodes:"); ui.label(format!("{}", state.pinned_node_count)); ui.end_row();
            ui.label("Free Nodes:"); ui.label(format!("{}", state.free_node_count)); ui.end_row();
            ui.label("Avg Spring Length:"); ui.label(format!("{:.4}", state.avg_spring_length)); ui.end_row();
            ui.label("Min Spring Length:"); ui.label(format!("{:.4}", state.min_spring_length)); ui.end_row();
            ui.label("Max Spring Length:"); ui.label(format!("{:.4}", state.max_spring_length)); ui.end_row();
            ui.label("Total Spring Energy:"); ui.label(format!("{:.6} J", state.total_spring_energy)); ui.end_row();
            ui.label("Connectivity:"); ui.label(format!("{:.2} springs/node", if state.node_count > 0 { state.spring_count as f32 / state.node_count as f32 } else { 0.0 })); ui.end_row();
        });
    }
}

// ─── Physics Scene Statistics Extended ───────────────────────────────────────

pub fn show_extended_scene_statistics(ui: &mut egui::Ui, body_list: &RigidBodyListState) {
    ui.heading("Extended Scene Statistics");
    ui.separator();

    let total = body_list.bodies.len();
    let dynamic_count = body_list.bodies.iter().filter(|b| b.body_type == RigidBodyType::Dynamic).count();
    let static_count = body_list.bodies.iter().filter(|b| b.body_type == RigidBodyType::Static).count();
    let kinematic_count = body_list.bodies.iter().filter(|b| b.body_type == RigidBodyType::Kinematic).count();
    let total_mass: f32 = body_list.bodies.iter().map(|b| b.mass).sum();
    let avg_mass = if dynamic_count > 0 { total_mass / dynamic_count as f32 } else { 0.0 };
    let max_mass = body_list.bodies.iter().map(|b| b.mass).fold(0.0f32, f32::max);
    let min_mass = body_list.bodies.iter().filter(|b| b.mass > 0.0).map(|b| b.mass).fold(f32::INFINITY, f32::min);
    let bullet_count = body_list.bodies.iter().filter(|b| b.is_bullet).count();
    let fixed_rot_count = body_list.bodies.iter().filter(|b| b.fixed_rotation).count();

    ui.columns(2, |cols| {
        egui::Grid::new("scene_stats_1").num_columns(2).spacing([12.0, 2.0]).show(&mut cols[0], |ui| {
            ui.label("Total Bodies:"); ui.label(format!("{}", total)); ui.end_row();
            ui.label("Dynamic:"); ui.colored_label(RigidBodyType::Dynamic.color(), format!("{}", dynamic_count)); ui.end_row();
            ui.label("Static:"); ui.colored_label(RigidBodyType::Static.color(), format!("{}", static_count)); ui.end_row();
            ui.label("Kinematic:"); ui.colored_label(RigidBodyType::Kinematic.color(), format!("{}", kinematic_count)); ui.end_row();
            ui.label("Bullet Bodies:"); ui.label(format!("{}", bullet_count)); ui.end_row();
            ui.label("Fixed Rotation:"); ui.label(format!("{}", fixed_rot_count)); ui.end_row();
        });
        egui::Grid::new("scene_stats_2").num_columns(2).spacing([12.0, 2.0]).show(&mut cols[1], |ui| {
            ui.label("Total Mass:"); ui.label(format!("{:.2} kg", total_mass)); ui.end_row();
            ui.label("Avg Dynamic Mass:"); ui.label(format!("{:.2} kg", avg_mass)); ui.end_row();
            ui.label("Max Mass:"); ui.label(format!("{:.2} kg", max_mass)); ui.end_row();
            ui.label("Min Mass:"); ui.label(format!("{:.2} kg", if min_mass.is_infinite() { 0.0 } else { min_mass })); ui.end_row();
        });
    });

    // Body type pie chart representation
    let desired = Vec2::new(ui.available_width().min(300.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(12, 14, 20));

    let bar_h = rect.height() - 20.0;
    let bar_top = rect.top() + 10.0;
    let max_count = dynamic_count.max(static_count).max(kinematic_count).max(1);
    let bar_data = [(dynamic_count, RigidBodyType::Dynamic.color(), "Dyn"), (static_count, RigidBodyType::Static.color(), "Sta"), (kinematic_count, RigidBodyType::Kinematic.color(), "Kin")];
    let bar_width = rect.width() / (bar_data.len() as f32 * 2.0 + 1.0);

    for (i, (count, col, lbl)) in bar_data.iter().enumerate() {
        let bx = rect.left() + (i as f32 * 2.0 + 1.0) * bar_width;
        let bh = *count as f32 / max_count as f32 * bar_h;
        p.rect_filled(Rect::from_min_size(Pos2::new(bx, bar_top + bar_h - bh), Vec2::new(bar_width, bh)), 2.0, *col);
        p.text(Pos2::new(bx + bar_width * 0.5, rect.bottom() - 2.0), egui::Align2::CENTER_BOTTOM, *lbl, FontId::monospace(7.0), Color32::GRAY);
        p.text(Pos2::new(bx + bar_width * 0.5, bar_top + bar_h - bh - 2.0), egui::Align2::CENTER_BOTTOM, format!("{}", count), FontId::monospace(7.0), *col);
    }
}

// ─── Misc Physics Utilities ───────────────────────────────────────────────────

pub fn compute_relative_velocity(vel_a: [f32; 2], vel_b: [f32; 2], omega_a: f32, omega_b: f32, contact_point_a: [f32; 2], contact_point_b: [f32; 2]) -> [f32; 2] {
    let rv_x = vel_b[0] - vel_a[0] - omega_b * contact_point_b[1] + omega_a * contact_point_a[1];
    let rv_y = vel_b[1] - vel_a[1] + omega_b * contact_point_b[0] - omega_a * contact_point_a[0];
    [rv_x, rv_y]
}

pub fn compute_impulse_magnitude(relative_velocity: [f32; 2], normal: [f32; 2], mass_a: f32, mass_b: f32, inv_inertia_a: f32, inv_inertia_b: f32, ra: [f32; 2], rb: [f32; 2], restitution: f32) -> f32 {
    let vn = relative_velocity[0] * normal[0] + relative_velocity[1] * normal[1];
    if vn > 0.0 { return 0.0; }
    let ra_cross_n = ra[0] * normal[1] - ra[1] * normal[0];
    let rb_cross_n = rb[0] * normal[1] - rb[1] * normal[0];
    let denom = 1.0/mass_a + 1.0/mass_b + ra_cross_n*ra_cross_n*inv_inertia_a + rb_cross_n*rb_cross_n*inv_inertia_b;
    if denom.abs() < 1e-10 { return 0.0; }
    -(1.0 + restitution) * vn / denom
}

pub fn aabb_overlap(ax_min: f32, ax_max: f32, ay_min: f32, ay_max: f32, bx_min: f32, bx_max: f32, by_min: f32, by_max: f32) -> bool {
    ax_min <= bx_max && ax_max >= bx_min && ay_min <= by_max && ay_max >= by_min
}

pub fn gjk_support_circle(center: [f32; 2], radius: f32, direction: [f32; 2]) -> [f32; 2] {
    let len = (direction[0]*direction[0]+direction[1]*direction[1]).sqrt().max(1e-10);
    [center[0] + direction[0]/len * radius, center[1] + direction[1]/len * radius]
}

pub fn gjk_support_box(center: [f32; 2], half_w: f32, half_h: f32, angle: f32, direction: [f32; 2]) -> [f32; 2] {
    let cos_a = angle.cos(); let sin_a = angle.sin();
    let local_dx = direction[0]*cos_a + direction[1]*sin_a;
    let local_dy = -direction[0]*sin_a + direction[1]*cos_a;
    let lx = if local_dx > 0.0 { half_w } else { -half_w };
    let ly = if local_dy > 0.0 { half_h } else { -half_h };
    let world_x = lx*cos_a - ly*sin_a + center[0];
    let world_y = lx*sin_a + ly*cos_a + center[1];
    [world_x, world_y]
}

pub fn cross2d(a: [f32; 2], b: [f32; 2]) -> f32 { a[0]*b[1] - a[1]*b[0] }
pub fn dot2d(a: [f32; 2], b: [f32; 2]) -> f32 { a[0]*b[0] + a[1]*b[1] }
pub fn normalize2d(v: [f32; 2]) -> [f32; 2] { let l = (v[0]*v[0]+v[1]*v[1]).sqrt().max(1e-10); [v[0]/l, v[1]/l] }
pub fn length2d(v: [f32; 2]) -> f32 { (v[0]*v[0]+v[1]*v[1]).sqrt() }
pub fn lerp2d(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] { [a[0] + (b[0]-a[0])*t, a[1] + (b[1]-a[1])*t] }

pub fn verlet_integrate(pos: &mut [f32; 2], old_pos: &mut [f32; 2], acc: [f32; 2], dt: f32) {
    let new_x = 2.0*pos[0] - old_pos[0] + acc[0]*dt*dt;
    let new_y = 2.0*pos[1] - old_pos[1] + acc[1]*dt*dt;
    old_pos[0] = pos[0]; old_pos[1] = pos[1];
    pos[0] = new_x; pos[1] = new_y;
}

pub fn euler_integrate(pos: &mut [f32; 2], vel: &mut [f32; 2], acc: [f32; 2], dt: f32) {
    vel[0] += acc[0]*dt; vel[1] += acc[1]*dt;
    pos[0] += vel[0]*dt; pos[1] += vel[1]*dt;
}

pub fn rk4_integrate(pos: &mut [f32; 2], vel: &mut [f32; 2], acc: [f32; 2], dt: f32) {
    let k1v = [acc[0]*dt, acc[1]*dt];
    let k1p = [vel[0]*dt, vel[1]*dt];
    let k2v = [(acc[0]+k1v[0]*0.5)*dt, (acc[1]+k1v[1]*0.5)*dt];
    let k2p = [(vel[0]+k1p[0]*0.5)*dt, (vel[1]+k1p[1]*0.5)*dt];
    let k3v = [(acc[0]+k2v[0]*0.5)*dt, (acc[1]+k2v[1]*0.5)*dt];
    let k3p = [(vel[0]+k2p[0]*0.5)*dt, (vel[1]+k2p[1]*0.5)*dt];
    let k4v = [(acc[0]+k3v[0])*dt, (acc[1]+k3v[1])*dt];
    let k4p = [(vel[0]+k3p[0])*dt, (vel[1]+k3p[1])*dt];
    vel[0] += (k1v[0]+2.0*k2v[0]+2.0*k3v[0]+k4v[0])/6.0;
    vel[1] += (k1v[1]+2.0*k2v[1]+2.0*k3v[1]+k4v[1])/6.0;
    pos[0] += (k1p[0]+2.0*k2p[0]+2.0*k3p[0]+k4p[0])/6.0;
    pos[1] += (k1p[1]+2.0*k2p[1]+2.0*k3p[1]+k4p[1])/6.0;
}

pub fn show_integration_method_selector(ui: &mut egui::Ui, method: &mut IntegrationMethod) {
    ui.heading("Integration Method");
    ui.separator();

    for m in &[IntegrationMethod::Euler, IntegrationMethod::SemiImplicitEuler, IntegrationMethod::Verlet, IntegrationMethod::RK4, IntegrationMethod::Leapfrog] {
        let sel = *method == *m;
        if ui.selectable_label(sel, m.label()).clicked() { *method = m.clone(); }
    }
    ui.separator();
    ui.label(egui::RichText::new(method.description()).small().color(Color32::GRAY));
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum IntegrationMethod {
    Euler,
    #[default]
    SemiImplicitEuler,
    Verlet,
    RK4,
    Leapfrog,
}

impl IntegrationMethod {
    pub fn label(&self) -> &str {
        match self {
            IntegrationMethod::Euler => "Euler (Explicit)",
            IntegrationMethod::SemiImplicitEuler => "Semi-Implicit Euler",
            IntegrationMethod::Verlet => "Verlet",
            IntegrationMethod::RK4 => "Runge-Kutta 4",
            IntegrationMethod::Leapfrog => "Leapfrog",
        }
    }
    pub fn description(&self) -> &str {
        match self {
            IntegrationMethod::Euler => "Simple, fast. Adds energy to simulation over time. Not recommended for long runs.",
            IntegrationMethod::SemiImplicitEuler => "Default. Symplectic, conserves energy better than explicit. Fast.",
            IntegrationMethod::Verlet => "No velocity storage. Very stable for constraint-based systems. Good for cloth/soft body.",
            IntegrationMethod::RK4 => "4th order accurate. Best energy conservation. 4x more expensive.",
            IntegrationMethod::Leapfrog => "Time-reversible. Excellent for orbital/planetary simulations.",
        }
    }
}

// ─── Physics Heatmap Tools ────────────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsHeatmapState {
    pub heatmap_type: HeatmapType,
    pub grid_resolution: usize,
    pub data: Vec<f32>,
    pub min_val: f32,
    pub max_val: f32,
    pub color_scheme: HeatmapColorScheme,
    pub show_contours: bool,
    pub contour_levels: u32,
    pub sample_radius: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum HeatmapType {
    #[default]
    KineticEnergy,
    ContactDensity,
    ForceField,
    Velocity,
    AngularVelocity,
    Pressure,
    Temperature,
}

impl HeatmapType {
    pub fn label(&self) -> &str {
        match self {
            HeatmapType::KineticEnergy => "Kinetic Energy",
            HeatmapType::ContactDensity => "Contact Density",
            HeatmapType::ForceField => "Force Field",
            HeatmapType::Velocity => "Velocity",
            HeatmapType::AngularVelocity => "Angular Velocity",
            HeatmapType::Pressure => "Pressure",
            HeatmapType::Temperature => "Temperature",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum HeatmapColorScheme {
    #[default]
    BlueRed,
    Viridis,
    Plasma,
    Greyscale,
    YellowGreen,
}

impl HeatmapColorScheme {
    pub fn label(&self) -> &str {
        match self {
            HeatmapColorScheme::BlueRed => "Blue→Red",
            HeatmapColorScheme::Viridis => "Viridis",
            HeatmapColorScheme::Plasma => "Plasma",
            HeatmapColorScheme::Greyscale => "Greyscale",
            HeatmapColorScheme::YellowGreen => "Yellow→Green",
        }
    }
    pub fn evaluate(&self, t: f32) -> Color32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            HeatmapColorScheme::BlueRed => {
                let r = (t * 2.0).min(1.0);
                let b = ((1.0 - t) * 2.0).min(1.0);
                Color32::from_rgb((r*255.0) as u8, 0, (b*255.0) as u8)
            }
            HeatmapColorScheme::Viridis => {
                let r = (0.3 + t * 0.5) * 255.0;
                let g = (t * 0.8) * 255.0;
                let b = (0.4 + (1.0-t) * 0.6) * 255.0;
                Color32::from_rgb(r as u8, g as u8, b as u8)
            }
            HeatmapColorScheme::Plasma => {
                let r = (0.5 + t * 0.5) * 255.0;
                let g = (t * 0.4) * 255.0;
                let b = (0.8 - t * 0.7) * 255.0;
                Color32::from_rgb(r as u8, g as u8, b as u8)
            }
            HeatmapColorScheme::Greyscale => {
                let v = (t * 255.0) as u8;
                Color32::from_rgb(v, v, v)
            }
            HeatmapColorScheme::YellowGreen => {
                let r = ((1.0-t) * 220.0) as u8;
                let g = (180.0 + t * 75.0) as u8;
                Color32::from_rgb(r, g, 30)
            }
        }
    }
}

pub fn show_physics_heatmap_tool(ui: &mut egui::Ui, state: &mut PhysicsHeatmapState) {
    ui.heading("Physics Heatmap");
    ui.separator();

    if state.grid_resolution == 0 { state.grid_resolution = 32; state.contour_levels = 5; state.sample_radius = 2.0; }

    ui.horizontal_wrapped(|ui| {
        for ht in &[HeatmapType::KineticEnergy, HeatmapType::ContactDensity, HeatmapType::ForceField, HeatmapType::Velocity, HeatmapType::AngularVelocity, HeatmapType::Pressure] {
            if ui.selectable_label(state.heatmap_type == *ht, ht.label()).clicked() { state.heatmap_type = ht.clone(); }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Color:");
        for cs in &[HeatmapColorScheme::BlueRed, HeatmapColorScheme::Viridis, HeatmapColorScheme::Plasma, HeatmapColorScheme::Greyscale, HeatmapColorScheme::YellowGreen] {
            if ui.selectable_label(state.color_scheme == *cs, cs.label()).clicked() { state.color_scheme = cs.clone(); }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Resolution:");
        ui.add(egui::DragValue::new(&mut state.grid_resolution).clamp_range(8..=128));
        ui.checkbox(&mut state.show_contours, "Contours");
        if state.show_contours {
            ui.add(egui::DragValue::new(&mut state.contour_levels).clamp_range(2..=20));
        }
        if ui.button("Generate Sample").clicked() {
            let n = state.grid_resolution;
            state.data = (0..n*n).map(|i| {
                let x = (i % n) as f32 / n as f32;
                let y = (i / n) as f32 / n as f32;
                ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt() * 2.0
            }).collect();
            state.min_val = 0.0;
            state.max_val = 1.0;
        }
    });

    draw_physics_heatmap(ui, state);
}

fn draw_physics_heatmap(ui: &mut egui::Ui, state: &PhysicsHeatmapState) {
    let desired = Vec2::new(ui.available_width().min(500.0), 240.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 14));

    if state.data.is_empty() || state.grid_resolution == 0 {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, "Click 'Generate Sample' or connect to simulation", FontId::monospace(9.0), Color32::DARK_GRAY);
        return;
    }

    let n = state.grid_resolution;
    let cell_w = rect.width() / n as f32;
    let cell_h = rect.height() / n as f32;
    let range = (state.max_val - state.min_val).max(0.001);

    for row in 0..n {
        for col in 0..n {
            let idx = row * n + col;
            if idx >= state.data.len() { break; }
            let val = state.data[idx];
            let t = ((val - state.min_val) / range).clamp(0.0, 1.0);
            let col32 = state.color_scheme.evaluate(t);
            let cr = Rect::from_min_size(
                Pos2::new(rect.left() + col as f32 * cell_w, rect.top() + row as f32 * cell_h),
                Vec2::new(cell_w + 0.5, cell_h + 0.5)
            );
            p.rect_filled(cr, 0.0, col32);
        }
    }

    // Contour lines (simplified — horizontal passes at each level)
    if state.show_contours {
        let levels = state.contour_levels as usize;
        for li in 1..levels {
            let threshold = state.min_val + range * li as f32 / levels as f32;
            for row in 0..n-1 {
                for col in 0..n-1 {
                    let idx = row * n + col;
                    if idx+1 >= state.data.len() || idx+n >= state.data.len() { break; }
                    let v00 = state.data[idx];
                    let v10 = state.data[idx+1];
                    if (v00 < threshold) != (v10 < threshold) {
                        let cx = rect.left() + (col as f32 + 0.5) * cell_w;
                        let cy = rect.top() + (row as f32 + 0.5) * cell_h;
                        p.circle_filled(Pos2::new(cx, cy), 0.8, Color32::from_rgba_unmultiplied(255, 255, 255, 120));
                    }
                }
            }
        }
    }

    // Color legend bar
    let legend_rect = Rect::from_min_size(Pos2::new(rect.right()-30.0, rect.top()+10.0), Vec2::new(16.0, rect.height()-20.0));
    let grad_steps = 30;
    let step_h = legend_rect.height() / grad_steps as f32;
    for gi in 0..grad_steps {
        let t = 1.0 - gi as f32 / grad_steps as f32;
        let col32 = state.color_scheme.evaluate(t);
        let gr = Rect::from_min_size(Pos2::new(legend_rect.left(), legend_rect.top() + gi as f32 * step_h), Vec2::new(legend_rect.width(), step_h+0.5));
        p.rect_filled(gr, 0.0, col32);
    }
    p.rect_stroke(legend_rect, 0.0, Stroke::new(0.5, Color32::from_rgb(60,70,80)));
    p.text(Pos2::new(legend_rect.left()-2.0, legend_rect.top()), egui::Align2::RIGHT_TOP, format!("{:.2}", state.max_val), FontId::monospace(6.5), Color32::GRAY);
    p.text(Pos2::new(legend_rect.left()-2.0, legend_rect.bottom()), egui::Align2::RIGHT_BOTTOM, format!("{:.2}", state.min_val), FontId::monospace(6.5), Color32::GRAY);
    p.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP, state.heatmap_type.label(), FontId::monospace(8.0), Color32::from_rgba_unmultiplied(255, 255, 255, 180));
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
