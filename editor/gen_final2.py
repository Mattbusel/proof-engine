path = r"C:\proof-engine\editor\src\physics_editor.rs"

addon = """

// ─── Physics Simulation State Snapshot ───────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct PhysicsEditorMasterState {
    pub soft_body: SoftBodyEditorState,
    pub cloth: ClothEditorState,
    pub rope: RopeEditorState,
    pub fluid: FluidEditorState,
    pub ragdoll: RagdollEditorState,
    pub constraint_graph: ConstraintGraphState,
    pub collision_editor: CollisionEditorState,
    pub material_library: MaterialLibraryState,
    pub gravity_field: GravityFieldEditorState,
    pub layer_manager: LayerManager,
    pub simulation_running: bool,
    pub simulation_time: f32,
    pub gravity: [f32; 2],
    pub time_scale: f32,
    pub paused: bool,
    pub show_debug_overlay: bool,
    pub selected_panel: u8,
}

impl PhysicsEditorMasterState {
    pub fn new() -> Self {
        Self {
            gravity: [0.0, -9.81],
            time_scale: 1.0,
            ..Default::default()
        }
    }

    pub fn reset_simulation(&mut self) {
        self.simulation_time = 0.0;
        self.simulation_running = false;
    }

    pub fn step_simulation(&mut self, dt: f32) {
        if self.simulation_running && !self.paused {
            let scaled_dt = dt * self.time_scale;
            self.simulation_time += scaled_dt;
            simulate_soft_body(&mut self.soft_body, scaled_dt, self.gravity);
            simulate_cloth(&mut self.cloth, scaled_dt, self.gravity, self.cloth.wind_dir);
            if let Some(ref mut rope) = self.rope.rope {
                simulate_rope(rope, scaled_dt, self.gravity);
            }
            simulate_fluid(&mut self.fluid, scaled_dt, self.gravity);
        }
    }

    pub fn toggle_play(&mut self) {
        self.simulation_running = !self.simulation_running;
    }

    pub fn is_running(&self) -> bool {
        self.simulation_running && !self.paused
    }
}

pub fn show_master_physics_editor(ui: &mut egui::Ui, state: &mut PhysicsEditorMasterState) {
    ui.horizontal(|ui| {
        let play_lbl = if state.simulation_running { "Pause (Space)" } else { "Play (Space)" };
        if ui.button(play_lbl).clicked() { state.toggle_play(); }
        if ui.button("Reset").clicked() { state.reset_simulation(); }
        if ui.button("Step").clicked() { state.step_simulation(0.016); }
        ui.label("Time Scale:");
        ui.add(egui::Slider::new(&mut state.time_scale, 0.0..=4.0));
        ui.label(format!("t={:.2}s", state.simulation_time));
    });
    ui.separator();

    let panels = ["Soft Body", "Cloth", "Rope", "Fluid", "Ragdoll", "Constraints", "Collision", "Materials", "Gravity", "Layers"];
    ui.horizontal_wrapped(|ui| {
        for (i, panel) in panels.iter().enumerate() {
            if ui.selectable_label(state.selected_panel == i as u8, *panel).clicked() {
                state.selected_panel = i as u8;
            }
        }
    });
    ui.separator();

    match state.selected_panel {
        0 => show_soft_body_editor(ui, &mut state.soft_body),
        1 => show_cloth_editor(ui, &mut state.cloth),
        2 => show_rope_editor(ui, &mut state.rope),
        3 => show_fluid_editor(ui, &mut state.fluid),
        4 => show_ragdoll_editor(ui, &mut state.ragdoll),
        5 => show_constraint_graph(ui, &mut state.constraint_graph),
        6 => show_collision_editor(ui, &mut state.collision_editor),
        7 => show_material_library(ui, &mut state.material_library),
        8 => show_gravity_field_editor(ui, &mut state.gravity_field),
        9 => show_layer_manager(ui, &mut state.layer_manager),
        _ => {}
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
