with open('C:/proof-engine/editor/src/particle_editor.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Keep lines 1-7145 (0-indexed: 0-7144)
kept = lines[:7145]

unique = (
    "\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "// COLOR / MATH UTILITIES (unique extras)\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "\n"
    "pub fn remap_val(value: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {\n"
    "    let t = if (in_max - in_min).abs() < 1e-9 { 0.0 } else { (value - in_min) / (in_max - in_min) };\n"
    "    lerp(out_min, out_max, t.clamp(0.0, 1.0))\n"
    "}\n"
    "\n"
    "pub fn curve_value_max(curve: &Curve, steps: u32) -> f32 {\n"
    "    (0..=steps).map(|i| curve.evaluate(i as f32 / steps as f32)).fold(f32::NEG_INFINITY, f32::max)\n"
    "}\n"
    "\n"
    "pub fn curve_value_min(curve: &Curve, steps: u32) -> f32 {\n"
    "    (0..=steps).map(|i| curve.evaluate(i as f32 / steps as f32)).fold(f32::INFINITY, f32::min)\n"
    "}\n"
    "\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "// BATCH EDITOR\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "\n"
    "pub struct BatchEditOp { pub field: BatchField, pub value: f32, pub enabled: bool }\n"
    "\n"
    "#[derive(Clone, Debug, PartialEq)]\n"
    "pub enum BatchField {\n"
    "    EmissionRate, MaxParticles, Duration, GravityModifier, StartSize, StartSpeed, StartLifetime,\n"
    "}\n"
    "\n"
    "impl BatchField {\n"
    "    pub fn label(&self) -> &str {\n"
    "        match self {\n"
    '            BatchField::EmissionRate => "Emission Rate",\n'
    '            BatchField::MaxParticles => "Max Particles",\n'
    '            BatchField::Duration => "Duration",\n'
    '            BatchField::GravityModifier => "Gravity",\n'
    '            BatchField::StartSize => "Start Size",\n'
    '            BatchField::StartSpeed => "Start Speed",\n'
    '            BatchField::StartLifetime => "Start Lifetime",\n'
    "        }\n"
    "    }\n"
    "}\n"
    "\n"
    "pub fn apply_batch_edit(systems: &mut [ParticleSystem], selected: &[usize], op: &BatchEditOp) {\n"
    "    if !op.enabled { return; }\n"
    "    for &idx in selected {\n"
    "        if let Some(s) = systems.get_mut(idx) {\n"
    "            match op.field {\n"
    "                BatchField::EmissionRate => s.emitter.emission_rate = op.value,\n"
    "                BatchField::MaxParticles => s.max_particles = op.value as u32,\n"
    "                BatchField::Duration => s.duration = op.value,\n"
    "                BatchField::GravityModifier => s.emitter.gravity_modifier = op.value,\n"
    "                BatchField::StartSize => s.emitter.start_size = RangeOrCurve::Constant(op.value),\n"
    "                BatchField::StartSpeed => s.emitter.start_speed = RangeOrCurve::Constant(op.value),\n"
    "                BatchField::StartLifetime => s.emitter.start_lifetime = RangeOrCurve::Constant(op.value),\n"
    "            }\n"
    "        }\n"
    "    }\n"
    "}\n"
    "\n"
    "pub struct BatchEditor { pub selected_systems: Vec<bool>, pub ops: Vec<BatchEditOp> }\n"
    "\n"
    "impl BatchEditor {\n"
    "    pub fn new(n: usize) -> Self {\n"
    "        BatchEditor {\n"
    "            selected_systems: vec![false; n],\n"
    "            ops: vec![\n"
    "                BatchEditOp { field: BatchField::EmissionRate, value: 10.0, enabled: false },\n"
    "                BatchEditOp { field: BatchField::MaxParticles, value: 1000.0, enabled: false },\n"
    "                BatchEditOp { field: BatchField::Duration, value: 5.0, enabled: false },\n"
    "                BatchEditOp { field: BatchField::GravityModifier, value: 0.0, enabled: false },\n"
    "            ],\n"
    "        }\n"
    "    }\n"
    "\n"
    "    pub fn show(&mut self, ui: &mut egui::Ui, systems: &mut Vec<ParticleSystem>) {\n"
    '        egui::CollapsingHeader::new("Batch Edit").default_open(false).show(ui, |ui| {\n'
    "            self.selected_systems.resize(systems.len(), false);\n"
    '            ui.label("Select systems:");\n'
    "            for (i, s) in systems.iter().enumerate() {\n"
    "                ui.checkbox(&mut self.selected_systems[i], &s.name);\n"
    "            }\n"
    "            ui.separator();\n"
    '            ui.label("Operations:");\n'
    "            for op in &mut self.ops {\n"
    "                ui.horizontal(|ui| {\n"
    "                    ui.checkbox(&mut op.enabled, op.field.label());\n"
    "                    if op.enabled {\n"
    "                        ui.add(egui::DragValue::new(&mut op.value).speed(0.1));\n"
    "                    }\n"
    "                });\n"
    "            }\n"
    "            let sel: Vec<usize> = self.selected_systems.iter().enumerate()\n"
    "                .filter_map(|(i, &s)| if s { Some(i) } else { None }).collect();\n"
    '            if !sel.is_empty() && ui.button(format!("Apply to {} systems", sel.len())).clicked() {\n'
    "                for op in &self.ops { apply_batch_edit(systems, &sel, op); }\n"
    "            }\n"
    "        });\n"
    "    }\n"
    "}\n"
    "\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "// SYSTEM INSPECTOR\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "\n"
    "pub fn show_system_inspector(ui: &mut egui::Ui, sys: &ParticleSystem) {\n"
    '    egui::CollapsingHeader::new(format!("Inspector: {}", sys.name)).default_open(false).show(ui, |ui| {\n'
    '        egui::Grid::new("inspector_final").num_columns(2).striped(true).show(ui, |ui| {\n'
    '            ui.label("Max Particles"); ui.label(sys.max_particles.to_string()); ui.end_row();\n'
    '            ui.label("Duration"); ui.label(format!("{:.2}s", sys.duration)); ui.end_row();\n'
    '            ui.label("Looping"); ui.label(sys.looping.to_string()); ui.end_row();\n'
    '            ui.label("Emission Rate"); ui.label(format!("{:.1}/s", sys.emitter.emission_rate)); ui.end_row();\n'
    '            ui.label("Shape"); ui.label(sys.emitter.shape.label()); ui.end_row();\n'
    '            ui.label("Gravity"); ui.label(format!("{:.3}", sys.emitter.gravity_modifier)); ui.end_row();\n'
    '            ui.label("Modules"); ui.label(format!("{} ({} active)", sys.modules.len(), sys.modules.iter().filter(|m| m.is_enabled()).count())); ui.end_row();\n'
    "        });\n"
    "        ui.separator();\n"
    "        for m in &sys.modules {\n"
    "            let col = if m.is_enabled() { Color32::from_rgb(180, 220, 180) } else { Color32::from_rgb(100, 100, 100) };\n"
    '            ui.label(RichText::new(format!("  {} {}", if m.is_enabled() { "+" } else { "-" }, m.name())).color(col).size(10.0));\n'
    "        }\n"
    "    });\n"
    "}\n"
    "\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "// CURVE LIBRARY\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "\n"
    "pub struct CurveLibrary { pub entries: Vec<(String, Curve)> }\n"
    "\n"
    "impl CurveLibrary {\n"
    "    pub fn with_defaults() -> Self {\n"
    "        CurveLibrary {\n"
    "            entries: vec![\n"
    '                ("0 to 1".to_string(), Curve::linear_zero_to_one()),\n'
    '                ("1 to 0".to_string(), Curve::linear_one_to_zero()),\n'
    '                ("Bell".to_string(), Curve::bell()),\n'
    '                ("Bounce".to_string(), Curve::bounce()),\n'
    '                ("Pulse".to_string(), Curve::pulse()),\n'
    '                ("Ease In/Out".to_string(), Curve::ease_in_out()),\n'
    '                ("Constant 1".to_string(), Curve::constant(1.0)),\n'
    '                ("Constant 0".to_string(), Curve::constant(0.0)),\n'
    "            ],\n"
    "        }\n"
    "    }\n"
    "\n"
    "    pub fn show(&self, ui: &mut egui::Ui) -> Option<Curve> {\n"
    "        let mut result = None;\n"
    '        egui::CollapsingHeader::new("Curve Library").default_open(false).show(ui, |ui| {\n'
    "            for (name, curve) in &self.entries {\n"
    "                ui.horizontal(|ui| {\n"
    "                    let (r, resp) = ui.allocate_exact_size(Vec2::new(60.0, 18.0), egui::Sense::click());\n"
    "                    draw_curve_mini(ui.painter(), r, curve);\n"
    "                    if resp.hovered() { ui.painter().rect_stroke(r, 1.0, Stroke::new(1.5, Color32::WHITE)); }\n"
    "                    ui.label(RichText::new(name).size(10.0));\n"
    "                    if resp.clicked() { result = Some(curve.clone()); }\n"
    "                });\n"
    "            }\n"
    "        });\n"
    "        result\n"
    "    }\n"
    "\n"
    "    pub fn add(&mut self, name: String, curve: Curve) { self.entries.push((name, curve)); }\n"
    "    pub fn remove(&mut self, idx: usize) { if idx < self.entries.len() { self.entries.remove(idx); } }\n"
    "}\n"
    "\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "// ASSET BROWSER\n"
    "// ─────────────────────────────────────────────────────────────────────────────\n"
    "\n"
    "pub struct ParticleAssetRecord { pub path: String, pub name: String, pub tags: Vec<String>, pub preset_hint: String }\n"
    "pub struct ParticleAssetBrowser { pub records: Vec<ParticleAssetRecord>, pub filter: String, pub selected: Option<usize> }\n"
    "\n"
    "impl ParticleAssetBrowser {\n"
    "    pub fn new() -> Self {\n"
    "        ParticleAssetBrowser {\n"
    "            selected: None,\n"
    "            filter: String::new(),\n"
    "            records: vec![\n"
    '                ParticleAssetRecord { path: "vfx/fire.json".into(), name: "Fire".into(), tags: vec!["fire".into(), "vfx".into()], preset_hint: "Fire".into() },\n'
    '                ParticleAssetRecord { path: "vfx/smoke.json".into(), name: "Smoke".into(), tags: vec!["smoke".into(), "vfx".into()], preset_hint: "Smoke".into() },\n'
    '                ParticleAssetRecord { path: "vfx/sparks.json".into(), name: "Sparks".into(), tags: vec!["sparks".into()], preset_hint: "Sparks".into() },\n'
    '                ParticleAssetRecord { path: "vfx/explosion.json".into(), name: "Explosion".into(), tags: vec!["combat".into(), "vfx".into()], preset_hint: "Explosion".into() },\n'
    '                ParticleAssetRecord { path: "vfx/heal.json".into(), name: "Heal".into(), tags: vec!["ui".into(), "healing".into()], preset_hint: "Heal".into() },\n'
    '                ParticleAssetRecord { path: "vfx/stars.json".into(), name: "Stars".into(), tags: vec!["ambient".into()], preset_hint: "Stars".into() },\n'
    '                ParticleAssetRecord { path: "vfx/confetti.json".into(), name: "Confetti".into(), tags: vec!["celebration".into(), "ui".into()], preset_hint: "Confetti".into() },\n'
    '                ParticleAssetRecord { path: "vfx/rain.json".into(), name: "Rain".into(), tags: vec!["weather".into()], preset_hint: "Rain".into() },\n'
    '                ParticleAssetRecord { path: "vfx/snow.json".into(), name: "Snow".into(), tags: vec!["weather".into()], preset_hint: "Snow".into() },\n'
    '                ParticleAssetRecord { path: "vfx/portal.json".into(), name: "Portal".into(), tags: vec!["magic".into()], preset_hint: "Portal".into() },\n'
    '                ParticleAssetRecord { path: "vfx/poison.json".into(), name: "Poison".into(), tags: vec!["combat".into(), "magic".into()], preset_hint: "Poison".into() },\n'
    '                ParticleAssetRecord { path: "vfx/magic_dust.json".into(), name: "Magic Dust".into(), tags: vec!["magic".into()], preset_hint: "Magic Dust".into() },\n'
    "            ],\n"
    "        }\n"
    "    }\n"
    "\n"
    "    pub fn show(&mut self, ui: &mut egui::Ui, editor: &mut ParticleEditor) {\n"
    '        ui.heading("Particle Assets");\n'
    "        ui.horizontal(|ui| {\n"
    '            ui.label("Filter:");\n'
    "            ui.text_edit_singleline(&mut self.filter);\n"
    '            if ui.small_button("Clear").clicked() { self.filter.clear(); }\n'
    "        });\n"
    "        ui.separator();\n"
    "\n"
    "        let filter_lower = self.filter.to_lowercase();\n"
    "\n"
    '        egui::ScrollArea::vertical().id_source("pab_final").show(ui, |ui| {\n'
    "            for (i, rec) in self.records.iter().enumerate() {\n"
    "                let matches = filter_lower.is_empty()\n"
    "                    || rec.name.to_lowercase().contains(&filter_lower)\n"
    "                    || rec.tags.iter().any(|t| t.to_lowercase().contains(&filter_lower));\n"
    "                if !matches { continue; }\n"
    "\n"
    "                let is_sel = self.selected == Some(i);\n"
    "                ui.horizontal(|ui| {\n"
    "                    if ui.selectable_label(is_sel, RichText::new(&rec.name).strong()).clicked() {\n"
    "                        self.selected = if is_sel { None } else { Some(i) };\n"
    "                    }\n"
    '                    ui.label(RichText::new(rec.tags.join(", ")).size(9.0).color(Color32::from_rgb(100, 100, 130)));\n'
    "                });\n"
    "\n"
    "                if is_sel {\n"
    '                    ui.indent("asset_actions", |ui| {\n'
    "                        ui.label(RichText::new(&rec.path).size(9.0).color(Color32::from_rgb(120, 120, 150)));\n"
    "                        ui.horizontal(|ui| {\n"
    '                            if ui.button("Load Preset").clicked() {\n'
    "                                apply_preset(editor, &rec.preset_hint);\n"
    "                            }\n"
    '                            if ui.button("Load + Duplicate").clicked() {\n'
    "                                apply_preset(editor, &rec.preset_hint);\n"
    "                                ParticleEditor::duplicate_active_system(editor);\n"
    "                            }\n"
    "                        });\n"
    "                    });\n"
    "                }\n"
    "            }\n"
    "        });\n"
    "    }\n"
    "}\n"
)

kept.append(unique)

with open('C:/proof-engine/editor/src/particle_editor.rs', 'w', encoding='utf-8') as f:
    f.writelines(kept)

print('Done. Lines kept before unique block:', len(kept) - 1)
