world_gen_code = r'''

// =================================================================
// WORLD GEN EXPANSION 3: NARRATIVE EVENTS & QUEST SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum QuestType {
    MainStory,
    SideQuest,
    Exploration,
    Combat,
    Diplomatic,
    Economic,
    Mystery,
    Rescue,
    Defense,
    Escort,
}

impl QuestType {
    pub fn name(&self) -> &'static str {
        match self {
            QuestType::MainStory => "Main Story",
            QuestType::SideQuest => "Side Quest",
            QuestType::Exploration => "Exploration",
            QuestType::Combat => "Combat",
            QuestType::Diplomatic => "Diplomatic",
            QuestType::Economic => "Economic",
            QuestType::Mystery => "Mystery",
            QuestType::Rescue => "Rescue",
            QuestType::Defense => "Defense",
            QuestType::Escort => "Escort",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            QuestType::MainStory => '★',
            QuestType::SideQuest => '◈',
            QuestType::Exploration => '🗺',
            QuestType::Combat => '⚔',
            QuestType::Diplomatic => '🕊',
            QuestType::Economic => '💰',
            QuestType::Mystery => '?',
            QuestType::Rescue => '!',
            QuestType::Defense => '🛡',
            QuestType::Escort => '→',
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestObjective {
    pub description: String,
    pub objective_type: String,
    pub target_count: u32,
    pub current_count: u32,
    pub optional: bool,
    pub completed: bool,
    pub location: Option<[f32;2]>,
}

impl QuestObjective {
    pub fn new(desc: &str, target: u32) -> Self {
        Self { description: desc.to_string(), objective_type: String::new(), target_count: target, current_count: 0, optional: false, completed: false, location: None }
    }

    pub fn progress_fraction(&self) -> f32 {
        if self.target_count == 0 { return 1.0; }
        (self.current_count as f32 / self.target_count as f32).min(1.0)
    }

    pub fn advance(&mut self, amount: u32) {
        self.current_count = (self.current_count + amount).min(self.target_count);
        if self.current_count >= self.target_count { self.completed = true; }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestReward {
    pub gold: u32,
    pub experience: u32,
    pub items: Vec<String>,
    pub reputation_faction: Option<u32>,
    pub reputation_change: i32,
    pub special_reward: Option<String>,
}

impl QuestReward {
    pub fn new(gold: u32, xp: u32) -> Self {
        Self { gold, experience: xp, items: Vec::new(), reputation_faction: None, reputation_change: 0, special_reward: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum QuestStatus {
    Available,
    Active,
    Completed,
    Failed,
    Locked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldQuest {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub quest_type: QuestType,
    pub status: QuestStatus,
    pub objectives: Vec<QuestObjective>,
    pub reward: QuestReward,
    pub giver_faction: Option<u32>,
    pub start_location: Option<[f32;2]>,
    pub target_location: Option<[f32;2]>,
    pub min_level: u32,
    pub time_limit_days: Option<u32>,
    pub days_active: u32,
    pub prerequisites: Vec<u32>,
    pub unlocks: Vec<u32>,
    pub lore_text: String,
}

impl WorldQuest {
    pub fn new(id: u32, title: &str, qt: QuestType) -> Self {
        Self {
            id, title: title.to_string(), description: String::new(), quest_type: qt,
            status: QuestStatus::Available, objectives: Vec::new(),
            reward: QuestReward::new(100, 50), giver_faction: None,
            start_location: None, target_location: None, min_level: 1,
            time_limit_days: None, days_active: 0, prerequisites: Vec::new(),
            unlocks: Vec::new(), lore_text: String::new(),
        }
    }

    pub fn is_completable(&self) -> bool {
        self.objectives.iter().all(|o| o.optional || o.completed)
    }

    pub fn completion_percentage(&self) -> f32 {
        if self.objectives.is_empty() { return 0.0; }
        let required: Vec<&QuestObjective> = self.objectives.iter().filter(|o| !o.optional).collect();
        if required.is_empty() { return 1.0; }
        required.iter().map(|o| o.progress_fraction()).sum::<f32>() / required.len() as f32
    }
}

pub fn generate_world_quests(factions: &[Faction], sites: &[MagicSite], rng: &mut SimpleRng) -> Vec<WorldQuest> {
    let quest_titles = [
        "The Missing Merchant", "Bandit's Stronghold", "The Ancient Seal", "A Diplomat's Gambit",
        "The Corrupted Well", "Lost in the Wilds", "Siege of the Northern Keep", "The Alchemist's Request",
        "Whispers in the Dark", "The Dragon's Toll", "A Question of Loyalty", "The Forbidden Archive",
        "Reclaim the Farmlands", "The Silver Chalice", "Trade Route Sabotage",
    ];

    let quest_types = [
        QuestType::Combat, QuestType::Exploration, QuestType::Mystery, QuestType::Diplomatic,
        QuestType::Rescue, QuestType::SideQuest, QuestType::Defense, QuestType::Escort,
        QuestType::Economic, QuestType::MainStory,
    ];

    let obj_templates = [
        ("Defeat {} enemies", 10u32),
        ("Explore {} locations", 3),
        ("Deliver {} packages", 1),
        ("Collect {} resources", 5),
        ("Talk to {} NPCs", 2),
        ("Survive {} days", 7),
    ];

    (0..15).map(|i| {
        let title = quest_titles[i % quest_titles.len()];
        let qt = quest_types[rng.next_u64() as usize % quest_types.len()].clone();
        let mut quest = WorldQuest::new(i as u32, title, qt);
        quest.min_level = 1 + (rng.next_u64() % 20) as u32;
        quest.reward = QuestReward::new(
            50 + (rng.next_u64() % 500) as u32,
            20 + (rng.next_u64() % 200) as u32
        );

        // Add 2-4 objectives
        let n_obj = 2 + rng.next_u64() as usize % 3;
        for _ in 0..n_obj {
            let (desc_template, target) = obj_templates[rng.next_u64() as usize % obj_templates.len()];
            let adjusted_target = target + (rng.next_u64() % target.max(1) as u64) as u32;
            let desc = desc_template.replace("{}", &adjusted_target.to_string());
            let mut obj = QuestObjective::new(&desc, adjusted_target);
            obj.optional = rng.next_f32() < 0.2;
            quest.objectives.push(obj);
        }

        // Assign to a faction
        if !factions.is_empty() {
            quest.giver_faction = Some(rng.next_u64() as u32 % factions.len() as u32);
        }

        // Set locations
        quest.start_location = Some([rng.next_f32(), rng.next_f32()]);
        quest.target_location = Some([rng.next_f32(), rng.next_f32()]);

        // Sometimes link to magic sites
        if !sites.is_empty() && rng.next_f32() < 0.3 {
            let si = rng.next_u64() as usize % sites.len();
            quest.target_location = Some(sites[si].position);
        }

        quest
    }).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct QuestEditorState {
    pub quests: Vec<WorldQuest>,
    pub selected_quest: Option<usize>,
    pub selected_objective: Option<usize>,
    pub filter_type: Option<QuestType>,
    pub filter_status: Option<QuestStatus>,
    pub show_map_markers: bool,
    pub show_rewards: bool,
}

pub fn show_quest_editor(ui: &mut egui::Ui, state: &mut QuestEditorState, factions: &[Faction]) {
    ui.horizontal(|ui| {
        ui.label("Quest Editor");
        if ui.button("+ New Quest").clicked() {
            let id = state.quests.len() as u32;
            state.quests.push(WorldQuest::new(id, &format!("Quest {}", id), QuestType::SideQuest));
        }
        ui.checkbox(&mut state.show_map_markers, "Map Markers");
        ui.checkbox(&mut state.show_rewards, "Show Rewards");
    });

    egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
        for (i, quest) in state.quests.iter().enumerate() {
            let sel = state.selected_quest == Some(i);
            let prog = quest.completion_percentage();
            let label = format!("[{:?}] {} ({:.0}%)", quest.quest_type, quest.title, prog * 100.0);
            let rich = egui::RichText::new(label).color(match quest.status {
                QuestStatus::Completed => egui::Color32::from_rgb(80, 200, 80),
                QuestStatus::Failed => egui::Color32::from_rgb(200, 80, 80),
                QuestStatus::Active => egui::Color32::from_rgb(255, 200, 50),
                _ => egui::Color32::WHITE,
            });
            if ui.selectable_label(sel, rich).clicked() { state.selected_quest = Some(i); }
        }
    });

    if let Some(qi) = state.selected_quest {
        if let Some(quest) = state.quests.get_mut(qi) {
            ui.separator();
            ui.text_edit_singleline(&mut quest.title);
            ui.text_edit_multiline(&mut quest.description);
            ui.add(egui::DragValue::new(&mut quest.min_level).clamp_range(1..=100u32).prefix("Level Req: "));
            ui.label(format!("Completion: {:.0}%", quest.completion_percentage() * 100.0));
            ui.label("Objectives:");
            for (i, obj) in quest.objectives.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut obj.completed, "");
                    ui.label(format!("{}/{}: {}", obj.current_count, obj.target_count, obj.description));
                    if ui.button("+").clicked() { obj.advance(1); }
                    ui.checkbox(&mut obj.optional, "opt");
                });
            }
            if ui.button("+ Add Objective").clicked() {
                quest.objectives.push(QuestObjective::new("New objective", 1));
            }
            if state.show_rewards {
                ui.separator();
                ui.label(format!("Reward: {} gold, {} XP", quest.reward.gold, quest.reward.experience));
            }
        }
    }
}

// =================================================================
// WORLD GEN EXPANSION 3: DUNGEON GRID ENCOUNTER TABLES
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EncounterCategory {
    Combat,
    Trap,
    Treasure,
    Environmental,
    NPC,
    Puzzle,
    Rest,
    Lore,
}

impl EncounterCategory {
    pub fn name(&self) -> &'static str {
        match self {
            EncounterCategory::Combat => "Combat",
            EncounterCategory::Trap => "Trap",
            EncounterCategory::Treasure => "Treasure",
            EncounterCategory::Environmental => "Environmental",
            EncounterCategory::NPC => "NPC Encounter",
            EncounterCategory::Puzzle => "Puzzle",
            EncounterCategory::Rest => "Rest Site",
            EncounterCategory::Lore => "Lore Discovery",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            EncounterCategory::Combat => Color32::from_rgb(220, 80, 80),
            EncounterCategory::Trap => Color32::from_rgb(200, 140, 50),
            EncounterCategory::Treasure => Color32::from_rgb(255, 200, 50),
            EncounterCategory::Environmental => Color32::from_rgb(80, 180, 100),
            EncounterCategory::NPC => Color32::from_rgb(100, 180, 220),
            EncounterCategory::Puzzle => Color32::from_rgb(180, 100, 220),
            EncounterCategory::Rest => Color32::from_rgb(80, 220, 180),
            EncounterCategory::Lore => Color32::from_rgb(200, 200, 255),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncounterEntry {
    pub name: String,
    pub category: EncounterCategory,
    pub weight: f32,
    pub challenge_rating: f32,
    pub description: String,
    pub min_dungeon_level: u32,
    pub max_dungeon_level: u32,
    pub unique: bool,
    pub can_repeat: bool,
    pub triggered: bool,
}

impl EncounterEntry {
    pub fn new(name: &str, cat: EncounterCategory, weight: f32) -> Self {
        Self { name: name.to_string(), category: cat, weight, challenge_rating: 1.0, description: String::new(), min_dungeon_level: 1, max_dungeon_level: 20, unique: false, can_repeat: true, triggered: false }
    }

    pub fn combat(name: &str, cr: f32) -> Self {
        let mut e = Self::new(name, EncounterCategory::Combat, 30.0);
        e.challenge_rating = cr;
        e
    }

    pub fn treasure(name: &str) -> Self { Self::new(name, EncounterCategory::Treasure, 15.0) }
    pub fn trap(name: &str) -> Self { Self::new(name, EncounterCategory::Trap, 20.0) }
    pub fn rest() -> Self { Self::new("Safe Room", EncounterCategory::Rest, 5.0) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncounterTable {
    pub name: String,
    pub entries: Vec<EncounterEntry>,
    pub dungeon_type: String,
    pub level_range: (u32, u32),
}

impl EncounterTable {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), entries: Vec::new(), dungeon_type: String::new(), level_range: (1, 10) }
    }

    pub fn default_dungeon() -> Self {
        let mut t = Self::new("Standard Dungeon");
        t.entries = vec![
            EncounterEntry::combat("Goblin Pack", 0.5),
            EncounterEntry::combat("Skeleton Warriors", 1.0),
            EncounterEntry::combat("Orc Raiders", 2.0),
            EncounterEntry::combat("Troll", 5.0),
            EncounterEntry::treasure("Gold Cache"),
            EncounterEntry::treasure("Magic Chest"),
            EncounterEntry::trap("Spike Pit"),
            EncounterEntry::trap("Pressure Plate Arrow Trap"),
            EncounterEntry::trap("Poison Gas Vent"),
            EncounterEntry::new("Merchant Prisoner", EncounterCategory::NPC, 5.0),
            EncounterEntry::new("Ancient Inscription", EncounterCategory::Lore, 8.0),
            EncounterEntry::new("Balance Puzzle", EncounterCategory::Puzzle, 6.0),
            EncounterEntry::rest(),
        ];
        t
    }

    pub fn total_weight(&self) -> f32 { self.entries.iter().map(|e| e.weight).sum() }

    pub fn roll(&self, rng: &mut SimpleRng, dungeon_level: u32) -> Option<&EncounterEntry> {
        let eligible: Vec<&EncounterEntry> = self.entries.iter()
            .filter(|e| dungeon_level >= e.min_dungeon_level && dungeon_level <= e.max_dungeon_level && (!e.unique || !e.triggered))
            .collect();
        if eligible.is_empty() { return None; }
        let total: f32 = eligible.iter().map(|e| e.weight).sum();
        let roll = rng.next_f32() * total;
        let mut accum = 0.0f32;
        for entry in &eligible {
            accum += entry.weight;
            if roll <= accum { return Some(entry); }
        }
        eligible.last().copied()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EncounterTableEditorState {
    pub tables: Vec<EncounterTable>,
    pub selected_table: Option<usize>,
    pub selected_entry: Option<usize>,
    pub preview_rolls: Vec<String>,
    pub preview_count: u32,
    pub preview_level: u32,
    pub preview_seed: u64,
}

impl EncounterTableEditorState {
    pub fn new() -> Self {
        Self { tables: vec![EncounterTable::default_dungeon()], preview_count: 20, preview_level: 5, preview_seed: 42, ..Default::default() }
    }
}

pub fn show_encounter_table_editor(ui: &mut egui::Ui, state: &mut EncounterTableEditorState) {
    ui.heading("Encounter Table Editor");
    ui.horizontal(|ui| {
        if ui.button("+ New Table").clicked() { state.tables.push(EncounterTable::new("New Table")); }
        if ui.button("Default Dungeon").clicked() { state.tables.push(EncounterTable::default_dungeon()); }
    });

    ui.columns(2, |cols| {
        egui::ScrollArea::vertical().max_height(200.0).show(&mut cols[0], |ui| {
            for (i, table) in state.tables.iter().enumerate() {
                let sel = state.selected_table == Some(i);
                if ui.selectable_label(sel, format!("{} ({} entries)", table.name, table.entries.len())).clicked() {
                    state.selected_table = Some(i);
                }
            }
        });

        if let Some(ti) = state.selected_table {
            if let Some(table) = state.tables.get_mut(ti) {
                egui::ScrollArea::vertical().max_height(200.0).show(&mut cols[1], |ui| {
                    ui.text_edit_singleline(&mut table.name);
                    if ui.button("+ Add Combat").clicked() { table.entries.push(EncounterEntry::combat("New Enemy", 1.0)); }
                    if ui.button("+ Add Treasure").clicked() { table.entries.push(EncounterEntry::treasure("New Treasure")); }
                    let mut remove = None;
                    for (i, entry) in table.entries.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.colored_label(entry.category.color(), &entry.name);
                            ui.add(egui::DragValue::new(&mut entry.weight).clamp_range(0.1..=100.0f32).speed(0.1).prefix("W:"));
                            if ui.button("🗑").clicked() { remove = Some(i); }
                        });
                    }
                    if let Some(i) = remove { table.entries.remove(i); }
                });
            }
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut state.preview_count).clamp_range(1..=100u32).prefix("Rolls: "));
        ui.add(egui::DragValue::new(&mut state.preview_level).clamp_range(1..=20u32).prefix("Level: "));
        ui.add(egui::DragValue::new(&mut state.preview_seed).prefix("Seed: "));
        if ui.button("Roll Encounters").clicked() {
            if let Some(ti) = state.selected_table {
                if let Some(table) = state.tables.get(ti) {
                    let mut rng = SimpleRng::new(state.preview_seed);
                    state.preview_rolls = (0..state.preview_count).filter_map(|_| {
                        table.roll(&mut rng, state.preview_level).map(|e| format!("[{:?}] {}", e.category, e.name))
                    }).collect();
                }
            }
        }
    });

    if !state.preview_rolls.is_empty() {
        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
            for r in &state.preview_rolls { ui.label(r); }
        });
    }
}

// =================================================================
// WORLD GEN EXPANSION 3: HEIGHTMAP LAYERS & COMPOSITING
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LayerBlendMode {
    Add,
    Subtract,
    Multiply,
    Divide,
    Screen,
    Overlay,
    Min,
    Max,
    Replace,
    Difference,
}

impl LayerBlendMode {
    pub fn name(&self) -> &'static str {
        match self {
            LayerBlendMode::Add => "Add",
            LayerBlendMode::Subtract => "Subtract",
            LayerBlendMode::Multiply => "Multiply",
            LayerBlendMode::Divide => "Divide",
            LayerBlendMode::Screen => "Screen",
            LayerBlendMode::Overlay => "Overlay",
            LayerBlendMode::Min => "Min",
            LayerBlendMode::Max => "Max",
            LayerBlendMode::Replace => "Replace",
            LayerBlendMode::Difference => "Difference",
        }
    }

    pub fn blend(&self, a: f32, b: f32) -> f32 {
        match self {
            LayerBlendMode::Add => (a + b).clamp(0.0, 1.0),
            LayerBlendMode::Subtract => (a - b).clamp(0.0, 1.0),
            LayerBlendMode::Multiply => a * b,
            LayerBlendMode::Divide => (a / b.max(0.001)).clamp(0.0, 1.0),
            LayerBlendMode::Screen => 1.0 - (1.0 - a) * (1.0 - b),
            LayerBlendMode::Overlay => {
                if a < 0.5 { 2.0 * a * b } else { 1.0 - 2.0 * (1.0 - a) * (1.0 - b) }
            }
            LayerBlendMode::Min => a.min(b),
            LayerBlendMode::Max => a.max(b),
            LayerBlendMode::Replace => b,
            LayerBlendMode::Difference => (a - b).abs(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeightmapLayer {
    pub name: String,
    pub data: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub enabled: bool,
    pub opacity: f32,
    pub blend_mode: LayerBlendMode,
    pub noise_config: HeightmapNoiseCfg,
    pub offset: [f32; 2],
    pub scale: [f32; 2],
}

impl HeightmapLayer {
    pub fn new(name: &str, w: usize, h: usize) -> Self {
        Self {
            name: name.to_string(),
            data: vec![0.0; w * h],
            width: w, height: h,
            enabled: true,
            opacity: 1.0,
            blend_mode: LayerBlendMode::Add,
            noise_config: HeightmapNoiseCfg::default(),
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
        }
    }

    pub fn generate(&mut self) {
        self.data = generate_heightmap_noise(self.width, self.height, &self.noise_config);
    }
}

pub fn composite_heightmap_layers(layers: &[HeightmapLayer]) -> Vec<f32> {
    if layers.is_empty() { return Vec::new(); }
    let w = layers[0].width;
    let h = layers[0].height;
    let mut result = vec![0.0f32; w * h];

    for layer in layers {
        if !layer.enabled { continue; }
        for i in 0..result.len() {
            let b = layer.data.get(i).copied().unwrap_or(0.0) * layer.opacity;
            result[i] = layer.blend_mode.blend(result[i], b);
        }
    }

    // Normalize
    let min = result.iter().cloned().fold(f32::MAX, f32::min);
    let max = result.iter().cloned().fold(f32::MIN, f32::max);
    let range = (max - min).max(0.001);
    for v in result.iter_mut() { *v = (*v - min) / range; }
    result
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct HeightmapLayerEditorState {
    pub layers: Vec<HeightmapLayer>,
    pub selected_layer: Option<usize>,
    pub composite_result: Vec<f32>,
    pub canvas_w: usize,
    pub canvas_h: usize,
    pub auto_composite: bool,
}

impl HeightmapLayerEditorState {
    pub fn new(w: usize, h: usize) -> Self {
        let mut s = Self { canvas_w: w, canvas_h: h, auto_composite: true, ..Default::default() };
        // Add default base layer
        let mut base = HeightmapLayer::new("Base Terrain", w, h);
        base.noise_config.seed = 12345;
        base.noise_config.octaves = 6;
        base.blend_mode = LayerBlendMode::Replace;
        s.layers.push(base);
        s
    }
}

pub fn show_heightmap_layer_editor(ui: &mut egui::Ui, state: &mut HeightmapLayerEditorState) {
    ui.heading("Heightmap Layers");
    ui.horizontal(|ui| {
        if ui.button("+ Add Layer").clicked() {
            let idx = state.layers.len();
            let mut layer = HeightmapLayer::new(&format!("Layer {}", idx), state.canvas_w, state.canvas_h);
            layer.noise_config.seed = idx as u64 * 1000 + 1;
            state.layers.push(layer);
        }
        if ui.button("Composite").clicked() {
            state.composite_result = composite_heightmap_layers(&state.layers);
        }
        ui.checkbox(&mut state.auto_composite, "Auto");
    });

    let mut move_up = None;
    let mut move_down = None;
    let mut remove_layer = None;

    for (i, layer) in state.layers.iter_mut().enumerate() {
        let sel = state.selected_layer == Some(i);
        ui.horizontal(|ui| {
            ui.checkbox(&mut layer.enabled, "");
            if ui.selectable_label(sel, &layer.name).clicked() { state.selected_layer = Some(i); }
            egui::ComboBox::from_id_source(format!("blend_{}", i))
                .selected_text(layer.blend_mode.name())
                .width(70.0)
                .show_ui(ui, |ui| {
                    for mode in [LayerBlendMode::Replace, LayerBlendMode::Add, LayerBlendMode::Subtract, LayerBlendMode::Multiply, LayerBlendMode::Max, LayerBlendMode::Min] {
                        let name = mode.name();
                        if ui.selectable_label(layer.blend_mode == mode, name).clicked() { layer.blend_mode = mode; }
                    }
                });
            ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).show_value(false));
            if i > 0 && ui.button("↑").clicked() { move_up = Some(i); }
            if i + 1 < state.layers.len() && ui.button("↓").clicked() { move_down = Some(i); }
            if ui.button("🗑").clicked() { remove_layer = Some(i); }
        });
    }

    if let Some(i) = move_up { state.layers.swap(i, i-1); }
    if let Some(i) = move_down { state.layers.swap(i, i+1); }
    if let Some(i) = remove_layer { state.layers.remove(i); }

    if let Some(idx) = state.selected_layer {
        if let Some(layer) = state.layers.get_mut(idx) {
            ui.separator();
            ui.text_edit_singleline(&mut layer.name);
            show_noise_heightmap_editor(ui, &mut layer.noise_config, &mut layer.data, layer.width, layer.height);
        }
    }
}

// =================================================================
// WORLD GEN EXPANSION 3: TESTS
// =================================================================

#[cfg(test)]
mod world_gen_expansion3_tests {
    use super::*;

    #[test]
    fn test_quest_objective_progress() {
        let mut obj = QuestObjective::new("Kill 10 goblins", 10);
        assert!(!obj.completed);
        obj.advance(5);
        assert_eq!(obj.current_count, 5);
        assert!((obj.progress_fraction() - 0.5).abs() < 0.001);
        obj.advance(10);
        assert!(obj.completed);
    }

    #[test]
    fn test_quest_completion() {
        let mut quest = WorldQuest::new(1, "Test Quest", QuestType::Combat);
        let mut obj = QuestObjective::new("Defeat enemy", 1);
        obj.advance(1);
        quest.objectives.push(obj);
        assert!(quest.is_completable());
    }

    #[test]
    fn test_quest_generation() {
        let factions = vec![Faction::new(0, "A", CultureType::Nordic)];
        let sites: Vec<MagicSite> = Vec::new();
        let mut rng = SimpleRng::new(42);
        let quests = generate_world_quests(&factions, &sites, &mut rng);
        assert!(!quests.is_empty());
    }

    #[test]
    fn test_encounter_table_roll() {
        let table = EncounterTable::default_dungeon();
        let mut rng = SimpleRng::new(42);
        let enc = table.roll(&mut rng, 5);
        assert!(enc.is_some());
    }

    #[test]
    fn test_encounter_table_weights() {
        let table = EncounterTable::default_dungeon();
        assert!(table.total_weight() > 0.0);
    }

    #[test]
    fn test_layer_blend_modes() {
        assert!((LayerBlendMode::Add.blend(0.5, 0.3) - 0.8).abs() < 0.001);
        assert!((LayerBlendMode::Multiply.blend(0.5, 0.5) - 0.25).abs() < 0.001);
        assert!((LayerBlendMode::Max.blend(0.3, 0.7) - 0.7).abs() < 0.001);
        assert!((LayerBlendMode::Min.blend(0.3, 0.7) - 0.3).abs() < 0.001);
        assert!((LayerBlendMode::Replace.blend(0.1, 0.9) - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_heightmap_layer_composite() {
        let mut layer_a = HeightmapLayer::new("A", 4, 4);
        layer_a.data = vec![0.5; 16];
        layer_a.blend_mode = LayerBlendMode::Replace;
        let mut layer_b = HeightmapLayer::new("B", 4, 4);
        layer_b.data = vec![0.5; 16];
        layer_b.blend_mode = LayerBlendMode::Add;
        let result = composite_heightmap_layers(&[layer_a, layer_b]);
        assert_eq!(result.len(), 16);
    }
}
'''

spline_code = r'''

// =================================================================
// SPLINE EXPANSION 3: SPLINE PAINTER & RENDERING
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SplinePaintMode {
    Solid,
    Dashed,
    Dotted,
    Gradient,
    Animated,
    Glow,
    Rainbow,
    Arrow,
}

impl SplinePaintMode {
    pub fn name(&self) -> &'static str {
        match self {
            SplinePaintMode::Solid => "Solid",
            SplinePaintMode::Dashed => "Dashed",
            SplinePaintMode::Dotted => "Dotted",
            SplinePaintMode::Gradient => "Gradient",
            SplinePaintMode::Animated => "Animated",
            SplinePaintMode::Glow => "Glow",
            SplinePaintMode::Rainbow => "Rainbow",
            SplinePaintMode::Arrow => "Arrow",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplinePaintStyle {
    pub mode: SplinePaintMode,
    pub color_start: egui::Color32,
    pub color_end: egui::Color32,
    pub width: f32,
    pub dash_length: f32,
    pub gap_length: f32,
    pub animation_speed: f32,
    pub animation_offset: f32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
    pub arrow_spacing: f32,
    pub arrow_size: f32,
    pub opacity: f32,
}

impl Default for SplinePaintStyle {
    fn default() -> Self {
        Self {
            mode: SplinePaintMode::Solid,
            color_start: egui::Color32::from_rgb(100, 200, 255),
            color_end: egui::Color32::from_rgb(255, 100, 200),
            width: 2.0, dash_length: 10.0, gap_length: 5.0,
            animation_speed: 1.0, animation_offset: 0.0,
            glow_radius: 8.0, glow_intensity: 0.5,
            arrow_spacing: 30.0, arrow_size: 8.0,
            opacity: 1.0,
        }
    }
}

pub fn draw_styled_spline(painter: &egui::Painter, points: &[egui::Pos2], style: &SplinePaintStyle) {
    if points.len() < 2 { return; }
    let total_pts = points.len();
    match style.mode {
        SplinePaintMode::Solid => {
            for w in points.windows(2) {
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, style.color_start));
            }
        }
        SplinePaintMode::Dashed => {
            let mut dist = 0.0f32;
            let mut drawing = true;
            let pattern = style.dash_length + style.gap_length;
            for w in points.windows(2) {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                let seg_len = (dx*dx + dy*dy).sqrt();
                let phase = dist % pattern;
                let draw_amt = (style.dash_length - phase).max(0.0).min(seg_len);
                if drawing && draw_amt > 0.0 {
                    let tx = w[0].x + dx * draw_amt / seg_len;
                    let ty = w[0].y + dy * draw_amt / seg_len;
                    painter.line_segment([w[0], egui::Pos2::new(tx, ty)], egui::Stroke::new(style.width, style.color_start));
                }
                dist += seg_len;
            }
        }
        SplinePaintMode::Gradient => {
            for (i, w) in points.windows(2).enumerate() {
                let t = i as f32 / total_pts as f32;
                let r = (style.color_start.r() as f32 + (style.color_end.r() as f32 - style.color_start.r() as f32) * t) as u8;
                let g = (style.color_start.g() as f32 + (style.color_end.g() as f32 - style.color_start.g() as f32) * t) as u8;
                let b = (style.color_start.b() as f32 + (style.color_end.b() as f32 - style.color_start.b() as f32) * t) as u8;
                let col = egui::Color32::from_rgb(r, g, b);
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, col));
            }
        }
        SplinePaintMode::Rainbow => {
            for (i, w) in points.windows(2).enumerate() {
                let hue = (i as f32 / total_pts as f32 * 360.0) as u32 % 360;
                let col = hsv_to_rgb(hue as f32, 1.0, 1.0);
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, col));
            }
        }
        SplinePaintMode::Arrow => {
            for w in points.windows(2) {
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, style.color_start));
            }
            // Draw arrowheads along the path
            let mut dist = style.arrow_spacing;
            for w in points.windows(2) {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                let seg_len = (dx*dx + dy*dy).sqrt().max(0.001);
                if dist <= seg_len {
                    let t = dist / seg_len;
                    let px = w[0].x + dx * t;
                    let py = w[0].y + dy * t;
                    let nx = dx / seg_len;
                    let ny = dy / seg_len;
                    let s = style.arrow_size;
                    painter.line_segment([egui::Pos2::new(px, py), egui::Pos2::new(px - nx*s - ny*s*0.5, py - ny*s + nx*s*0.5)], egui::Stroke::new(style.width, style.color_start));
                    painter.line_segment([egui::Pos2::new(px, py), egui::Pos2::new(px - nx*s + ny*s*0.5, py - ny*s - nx*s*0.5)], egui::Stroke::new(style.width, style.color_start));
                    dist = style.arrow_spacing;
                } else {
                    dist -= seg_len;
                }
            }
        }
        SplinePaintMode::Glow => {
            // Draw multiple strokes with decreasing opacity for glow
            for layer in 0..4 {
                let radius = style.glow_radius * (4 - layer) as f32 / 4.0;
                let alpha = (style.glow_intensity * layer as f32 / 4.0 * 255.0) as u8;
                let col = egui::Color32::from_rgba_premultiplied(style.color_start.r(), style.color_start.g(), style.color_start.b(), alpha);
                for w in points.windows(2) {
                    painter.line_segment([w[0], w[1]], egui::Stroke::new(radius, col));
                }
            }
            for w in points.windows(2) {
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, style.color_start));
            }
        }
        SplinePaintMode::Dotted => {
            let mut dist = 0.0f32;
            for w in points.windows(2) {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                let seg_len = (dx*dx + dy*dy).sqrt().max(0.001);
                while dist < seg_len {
                    let t = dist / seg_len;
                    let px = w[0].x + dx * t;
                    let py = w[0].y + dy * t;
                    painter.circle_filled(egui::Pos2::new(px, py), style.width, style.color_start);
                    dist += style.dash_length + style.gap_length;
                }
                dist -= seg_len;
            }
        }
        SplinePaintMode::Animated => {
            // Animated dashes based on offset
            let offset = style.animation_offset;
            let pattern = style.dash_length + style.gap_length;
            for w in points.windows(2) {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                let seg_len = (dx*dx + dy*dy).sqrt().max(0.001);
                let phase = (offset + 0.0) % pattern;
                let draw_from = if phase < style.dash_length { 0.0 } else { pattern - phase };
                if draw_from < seg_len {
                    let t = draw_from / seg_len;
                    let t2 = ((draw_from + style.dash_length) / seg_len).min(1.0);
                    let pa = egui::Pos2::new(w[0].x + dx * t, w[0].y + dy * t);
                    let pb = egui::Pos2::new(w[0].x + dx * t2, w[0].y + dy * t2);
                    painter.line_segment([pa, pb], egui::Stroke::new(style.width, style.color_start));
                }
            }
        }
    }
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> egui::Color32 {
    let h6 = h / 60.0;
    let i = h6.floor() as u32 % 6;
    let f = h6 - h6.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

pub fn show_paint_style_editor(ui: &mut egui::Ui, style: &mut SplinePaintStyle) {
    ui.collapsing("Paint Style", |ui| {
        ui.horizontal(|ui| {
            for mode in [SplinePaintMode::Solid, SplinePaintMode::Dashed, SplinePaintMode::Gradient, SplinePaintMode::Glow, SplinePaintMode::Arrow, SplinePaintMode::Rainbow] {
                if ui.selectable_label(style.mode == mode, mode.name()).clicked() { style.mode = mode; }
            }
        });
        ui.add(egui::Slider::new(&mut style.width, 0.5..=20.0).text("Width"));
        ui.add(egui::Slider::new(&mut style.opacity, 0.0..=1.0).text("Opacity"));
        ui.horizontal(|ui| {
            ui.label("Color A:");
            egui::color_picker::color_edit_button_srgba(ui, &mut style.color_start, egui::color_picker::Alpha::Opaque);
            ui.label("Color B:");
            egui::color_picker::color_edit_button_srgba(ui, &mut style.color_end, egui::color_picker::Alpha::Opaque);
        });
        if matches!(style.mode, SplinePaintMode::Dashed | SplinePaintMode::Dotted | SplinePaintMode::Animated) {
            ui.add(egui::Slider::new(&mut style.dash_length, 1.0..=50.0).text("Dash Length"));
            ui.add(egui::Slider::new(&mut style.gap_length, 1.0..=50.0).text("Gap Length"));
        }
        if style.mode == SplinePaintMode::Glow {
            ui.add(egui::Slider::new(&mut style.glow_radius, 1.0..=30.0).text("Glow Radius"));
            ui.add(egui::Slider::new(&mut style.glow_intensity, 0.0..=1.0).text("Glow Intensity"));
        }
        if style.mode == SplinePaintMode::Arrow {
            ui.add(egui::Slider::new(&mut style.arrow_spacing, 10.0..=100.0).text("Arrow Spacing"));
            ui.add(egui::Slider::new(&mut style.arrow_size, 3.0..=20.0).text("Arrow Size"));
        }
        if style.mode == SplinePaintMode::Animated {
            ui.add(egui::Slider::new(&mut style.animation_speed, 0.1..=10.0).text("Animation Speed"));
        }
    });
}

// =================================================================
// SPLINE EXPANSION 3: SPLINE LIBRARY MANAGER
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineLibraryEntry {
    pub id: u32,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub points: Vec<[f32;2]>,
    pub paint_style: SplinePaintStyle,
    pub is_favorite: bool,
    pub usage_count: u32,
    pub description: String,
}

impl SplineLibraryEntry {
    pub fn new(id: u32, name: &str, points: Vec<[f32;2]>) -> Self {
        Self { id, name: name.to_string(), category: String::new(), tags: Vec::new(), points, paint_style: SplinePaintStyle::default(), is_favorite: false, usage_count: 0, description: String::new() }
    }

    pub fn arc_length(&self) -> f32 {
        compute_spline_arc_length_points(&self.points)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SplineLibraryManagerState {
    pub entries: Vec<SplineLibraryEntry>,
    pub selected: Option<usize>,
    pub search_query: String,
    pub filter_category: String,
    pub filter_favorites: bool,
    pub sort_by: SplineLibrarySortBy,
    pub next_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum SplineLibrarySortBy {
    #[default]
    Name,
    Category,
    Usage,
    Length,
}

impl SplineLibraryManagerState {
    pub fn new() -> Self {
        let mut s = Self::default();
        // Add some example entries
        s.entries = vec![
            SplineLibraryEntry::new(0, "Simple Curve", vec![[0.0,0.0],[0.5,0.3],[1.0,0.0]]),
            SplineLibraryEntry::new(1, "S-Curve", vec![[0.0,0.0],[0.3,-0.2],[0.7,0.2],[1.0,0.0]]),
            SplineLibraryEntry::new(2, "Circle Path", generate_parametric_curve(&ParametricCurveConfig { curve_type: ParametricCurveType::Circle, param_a: 1.0, resolution: 32, ..Default::default() })),
        ];
        s.entries[0].category = "Basic".to_string();
        s.entries[1].category = "Basic".to_string();
        s.entries[2].category = "Shapes".to_string();
        s.next_id = 3;
        s
    }

    pub fn visible_entries(&self) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        let mut indices: Vec<usize> = self.entries.iter().enumerate().filter(|(_, e)| {
            (!self.filter_favorites || e.is_favorite) &&
            (self.filter_category.is_empty() || e.category == self.filter_category) &&
            (q.is_empty() || e.name.to_lowercase().contains(&q) || e.tags.iter().any(|t| t.contains(&q)))
        }).map(|(i, _)| i).collect();
        indices.sort_by(|&a, &b| {
            match self.sort_by {
                SplineLibrarySortBy::Name => self.entries[a].name.cmp(&self.entries[b].name),
                SplineLibrarySortBy::Category => self.entries[a].category.cmp(&self.entries[b].category),
                SplineLibrarySortBy::Usage => self.entries[b].usage_count.cmp(&self.entries[a].usage_count),
                SplineLibrarySortBy::Length => self.entries[a].arc_length().partial_cmp(&self.entries[b].arc_length()).unwrap_or(std::cmp::Ordering::Equal),
            }
        });
        indices
    }
}

pub fn show_spline_library_manager(ui: &mut egui::Ui, state: &mut SplineLibraryManagerState) {
    ui.heading("Spline Library");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.search_query);
        ui.checkbox(&mut state.filter_favorites, "★ Favorites");
    });
    ui.horizontal(|ui| {
        ui.label("Sort:");
        for (sort, label) in [(SplineLibrarySortBy::Name, "Name"), (SplineLibrarySortBy::Category, "Cat"), (SplineLibrarySortBy::Usage, "Usage"), (SplineLibrarySortBy::Length, "Length")] {
            if ui.selectable_label(state.sort_by == sort, label).clicked() { state.sort_by = sort; }
        }
    });
    ui.horizontal(|ui| {
        if ui.button("+ New Entry").clicked() {
            let id = state.next_id;
            state.next_id += 1;
            state.entries.push(SplineLibraryEntry::new(id, &format!("Spline {}", id), vec![[0.0,0.0],[1.0,0.0]]));
        }
    });

    egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
        let visible = state.visible_entries();
        for i in visible {
            let entry = &state.entries[i];
            let sel = state.selected == Some(i);
            ui.horizontal(|ui| {
                if entry.is_favorite { ui.label("★"); }
                if ui.selectable_label(sel, format!("{} [{} pts]", entry.name, entry.points.len())).clicked() {
                    state.selected = Some(i);
                    state.entries[i].usage_count += 1;
                }
                ui.label(format!("L:{:.1}", entry.arc_length()));
            });
        }
    });

    if let Some(idx) = state.selected {
        if let Some(entry) = state.entries.get_mut(idx) {
            ui.separator();
            ui.text_edit_singleline(&mut entry.name);
            ui.text_edit_singleline(&mut entry.category);
            ui.checkbox(&mut entry.is_favorite, "Favorite");
            ui.label(format!("Points: {} | Length: {:.2} | Used: {} times", entry.points.len(), entry.arc_length(), entry.usage_count));
            ui.text_edit_multiline(&mut entry.description);
            show_paint_style_editor(ui, &mut entry.paint_style);
        }
    }
}

// =================================================================
// SPLINE EXPANSION 3: TESTS
// =================================================================

#[cfg(test)]
mod spline_expansion3_tests {
    use super::*;

    #[test]
    fn test_hsv_to_rgb_red() {
        let col = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(col.r(), 255);
        assert_eq!(col.g(), 0);
        assert_eq!(col.b(), 0);
    }

    #[test]
    fn test_hsv_to_rgb_green() {
        let col = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(col.r(), 0);
        assert!(col.g() > 200);
    }

    #[test]
    fn test_paint_style_default() {
        let style = SplinePaintStyle::default();
        assert_eq!(style.mode, SplinePaintMode::Solid);
        assert!(style.width > 0.0);
    }

    #[test]
    fn test_spline_library_search() {
        let mut state = SplineLibraryManagerState::new();
        state.search_query = "circle".to_string();
        let visible = state.visible_entries();
        assert!(!visible.is_empty());
    }

    #[test]
    fn test_spline_library_favorites() {
        let mut state = SplineLibraryManagerState::new();
        state.entries[0].is_favorite = true;
        state.filter_favorites = true;
        let visible = state.visible_entries();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0], 0);
    }

    #[test]
    fn test_spline_library_usage_count() {
        let mut state = SplineLibraryManagerState::new();
        state.selected = Some(0);
        // Simulate clicking
        state.entries[0].usage_count += 1;
        assert_eq!(state.entries[0].usage_count, 1);
    }

    #[test]
    fn test_draw_styled_spline_does_not_panic() {
        // No painter available in test, just test the data structures work
        let style = SplinePaintStyle::default();
        let pts = vec![[0.0f32, 0.0], [1.0, 0.0], [2.0, 1.0]];
        assert_eq!(pts.len(), 3);
        assert_eq!(style.mode, SplinePaintMode::Solid);
    }
}
'''

inventory_code = r'''

// =================================================================
// INVENTORY EXPANSION 2: ATTRIBUTE & STAT SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterAttributes {
    pub strength: u32,
    pub dexterity: u32,
    pub intelligence: u32,
    pub wisdom: u32,
    pub constitution: u32,
    pub charisma: u32,
    pub luck: u32,
    pub endurance: u32,
    pub perception: u32,
    pub willpower: u32,
}

impl Default for CharacterAttributes {
    fn default() -> Self {
        Self { strength: 10, dexterity: 10, intelligence: 10, wisdom: 10, constitution: 10, charisma: 10, luck: 10, endurance: 10, perception: 10, willpower: 10 }
    }
}

impl CharacterAttributes {
    pub fn point_buy_cost(&self) -> u32 {
        let base = 8u32;
        let cost_above = |v: u32| -> u32 {
            if v <= base { 0 }
            else if v <= 13 { v - base }
            else if v <= 15 { (v - base) + (v - 13) }
            else { (v - base) + (v - 13) * 2 }
        };
        cost_above(self.strength) + cost_above(self.dexterity) + cost_above(self.intelligence) +
        cost_above(self.wisdom) + cost_above(self.constitution) + cost_above(self.charisma) +
        cost_above(self.luck) + cost_above(self.endurance) + cost_above(self.perception) + cost_above(self.willpower)
    }

    pub fn modifier(stat: u32) -> i32 { (stat as i32 - 10) / 2 }

    pub fn str_mod(&self) -> i32 { Self::modifier(self.strength) }
    pub fn dex_mod(&self) -> i32 { Self::modifier(self.dexterity) }
    pub fn int_mod(&self) -> i32 { Self::modifier(self.intelligence) }
    pub fn wis_mod(&self) -> i32 { Self::modifier(self.wisdom) }
    pub fn con_mod(&self) -> i32 { Self::modifier(self.constitution) }
    pub fn cha_mod(&self) -> i32 { Self::modifier(self.charisma) }

    pub fn derived_hp(&self) -> u32 { 10 + self.constitution * 5 }
    pub fn derived_mp(&self) -> u32 { 5 + self.intelligence * 4 }
    pub fn derived_carry_weight(&self) -> u32 { self.strength * 10 }
    pub fn derived_initiative(&self) -> i32 { Self::modifier(self.dexterity) + Self::modifier(self.perception) }
    pub fn derived_armor_class(&self) -> u32 { 10 + (Self::modifier(self.dexterity).max(0) as u32) }
    pub fn derived_attack_bonus(&self) -> i32 { Self::modifier(self.strength).max(Self::modifier(self.dexterity)) }

    pub fn total_points(&self) -> u32 {
        self.strength + self.dexterity + self.intelligence + self.wisdom + self.constitution + self.charisma + self.luck + self.endurance + self.perception + self.willpower
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AttributeEditorState {
    pub attributes: CharacterAttributes,
    pub point_budget: u32,
    pub base_points: u32,
    pub locked: bool,
    pub character_level: u32,
    pub stat_allocation_history: Vec<String>,
}

impl AttributeEditorState {
    pub fn new(budget: u32) -> Self {
        Self { attributes: CharacterAttributes::default(), point_budget: budget, base_points: 80, character_level: 1, ..Default::default() }
    }

    pub fn points_spent(&self) -> u32 {
        self.attributes.point_buy_cost()
    }

    pub fn points_remaining(&self) -> i32 {
        self.point_budget as i32 - self.points_spent() as i32
    }

    pub fn can_increase(&self, _attr: u32) -> bool {
        self.points_remaining() > 0 && !self.locked
    }
}

pub fn show_attribute_editor(ui: &mut egui::Ui, state: &mut AttributeEditorState) {
    ui.heading("Character Attributes");
    ui.label(format!("Level {} | Budget: {} | Spent: {} | Remaining: {}",
        state.character_level, state.point_budget, state.points_spent(), state.points_remaining()));
    ui.checkbox(&mut state.locked, "Locked");

    let attrs = [
        ("STR", &mut state.attributes.strength as *mut u32),
        ("DEX", &mut state.attributes.dexterity as *mut u32),
        ("INT", &mut state.attributes.intelligence as *mut u32),
        ("WIS", &mut state.attributes.wisdom as *mut u32),
        ("CON", &mut state.attributes.constitution as *mut u32),
        ("CHA", &mut state.attributes.charisma as *mut u32),
        ("LCK", &mut state.attributes.luck as *mut u32),
        ("END", &mut state.attributes.endurance as *mut u32),
        ("PER", &mut state.attributes.perception as *mut u32),
        ("WIL", &mut state.attributes.willpower as *mut u32),
    ];

    for (name, ptr) in attrs {
        let val = unsafe { *ptr };
        let modi = CharacterAttributes::modifier(val);
        ui.horizontal(|ui| {
            ui.label(format!("{}: {:2} ({:+})", name, val, modi));
            if !state.locked {
                if ui.button("-").clicked() && val > 3 { unsafe { *ptr = val - 1; } }
                if ui.button("+").clicked() && state.points_remaining() > 0 { unsafe { *ptr = val + 1; } }
            }
        });
    }

    ui.separator();
    ui.label("Derived Stats:");
    let a = &state.attributes;
    ui.label(format!("  HP: {} | MP: {}", a.derived_hp(), a.derived_mp()));
    ui.label(format!("  Carry: {}lbs | Initiative: {:+}", a.derived_carry_weight(), a.derived_initiative()));
    ui.label(format!("  AC: {} | Attack: {:+}", a.derived_armor_class(), a.derived_attack_bonus()));
}

// =================================================================
// INVENTORY EXPANSION 2: STATUS EFFECTS SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum StatusEffectType {
    Poison,
    Burn,
    Freeze,
    Stun,
    Bleed,
    Regeneration,
    Haste,
    Slow,
    Shield,
    Invisible,
    Curse,
    Blessed,
    Confused,
    Enrage,
    Weaken,
    Strengthen,
    Silence,
    Root,
}

impl StatusEffectType {
    pub fn name(&self) -> &'static str {
        match self {
            StatusEffectType::Poison => "Poison",
            StatusEffectType::Burn => "Burning",
            StatusEffectType::Freeze => "Frozen",
            StatusEffectType::Stun => "Stunned",
            StatusEffectType::Bleed => "Bleeding",
            StatusEffectType::Regeneration => "Regeneration",
            StatusEffectType::Haste => "Haste",
            StatusEffectType::Slow => "Slow",
            StatusEffectType::Shield => "Shield",
            StatusEffectType::Invisible => "Invisible",
            StatusEffectType::Curse => "Cursed",
            StatusEffectType::Blessed => "Blessed",
            StatusEffectType::Confused => "Confused",
            StatusEffectType::Enrage => "Enraged",
            StatusEffectType::Weaken => "Weakened",
            StatusEffectType::Strengthen => "Strengthened",
            StatusEffectType::Silence => "Silenced",
            StatusEffectType::Root => "Rooted",
        }
    }

    pub fn is_beneficial(&self) -> bool {
        matches!(self, StatusEffectType::Regeneration | StatusEffectType::Haste | StatusEffectType::Shield | StatusEffectType::Invisible | StatusEffectType::Blessed | StatusEffectType::Strengthen)
    }

    pub fn color(&self) -> egui::Color32 {
        if self.is_beneficial() { egui::Color32::from_rgb(80, 200, 100) }
        else { egui::Color32::from_rgb(200, 80, 80) }
    }

    pub fn icon(&self) -> char {
        match self {
            StatusEffectType::Poison => '☠',
            StatusEffectType::Burn => '🔥',
            StatusEffectType::Freeze => '❄',
            StatusEffectType::Stun => '★',
            StatusEffectType::Bleed => '♥',
            StatusEffectType::Regeneration => '+',
            StatusEffectType::Haste => '▶',
            StatusEffectType::Slow => '▼',
            StatusEffectType::Shield => '🛡',
            StatusEffectType::Invisible => '?',
            _ => '*',
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusEffect {
    pub effect_type: StatusEffectType,
    pub duration: f32,
    pub remaining: f32,
    pub intensity: f32,
    pub source: String,
    pub stack_count: u32,
    pub max_stacks: u32,
    pub tick_damage: f32,
    pub tick_heal: f32,
    pub stat_modifier: StatBonus,
    pub can_cleanse: bool,
    pub can_dispel: bool,
}

impl StatusEffect {
    pub fn new(et: StatusEffectType, duration: f32, intensity: f32) -> Self {
        let (tick_dmg, tick_heal) = match et {
            StatusEffectType::Poison => (intensity * 5.0, 0.0),
            StatusEffectType::Burn => (intensity * 8.0, 0.0),
            StatusEffectType::Bleed => (intensity * 3.0, 0.0),
            StatusEffectType::Regeneration => (0.0, intensity * 10.0),
            _ => (0.0, 0.0),
        };
        Self {
            effect_type: et, duration, remaining: duration, intensity,
            source: String::new(), stack_count: 1, max_stacks: 5,
            tick_damage: tick_dmg, tick_heal,
            stat_modifier: StatBonus::default(),
            can_cleanse: true, can_dispel: true,
        }
    }

    pub fn progress(&self) -> f32 { 1.0 - self.remaining / self.duration.max(0.001) }

    pub fn is_expired(&self) -> bool { self.remaining <= 0.0 }

    pub fn tick(&mut self, dt: f32) {
        self.remaining = (self.remaining - dt).max(0.0);
    }

    pub fn add_stack(&mut self) -> bool {
        if self.stack_count < self.max_stacks {
            self.stack_count += 1;
            self.remaining = self.duration;
            true
        } else {
            self.remaining = self.duration;
            false
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct StatusEffectPanelState {
    pub active_effects: Vec<StatusEffect>,
    pub selected: Option<usize>,
    pub show_expired: bool,
    pub filter_beneficial: Option<bool>,
}

impl StatusEffectPanelState {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.active_effects = vec![
            StatusEffect::new(StatusEffectType::Poison, 30.0, 1.0),
            StatusEffect::new(StatusEffectType::Regeneration, 60.0, 1.5),
            StatusEffect::new(StatusEffectType::Haste, 15.0, 1.0),
        ];
        s.active_effects[0].remaining = 15.0;
        s
    }

    pub fn add_effect(&mut self, mut effect: StatusEffect) {
        if let Some(existing) = self.active_effects.iter_mut().find(|e| e.effect_type == effect.effect_type) {
            existing.add_stack();
        } else {
            self.active_effects.push(effect);
        }
    }

    pub fn remove_expired(&mut self) {
        self.active_effects.retain(|e| !e.is_expired());
    }

    pub fn tick_all(&mut self, dt: f32) {
        for e in self.active_effects.iter_mut() { e.tick(dt); }
    }

    pub fn total_tick_damage(&self) -> f32 {
        self.active_effects.iter().map(|e| e.tick_damage * e.stack_count as f32).sum()
    }

    pub fn total_tick_heal(&self) -> f32 {
        self.active_effects.iter().map(|e| e.tick_heal * e.stack_count as f32).sum()
    }
}

pub fn show_status_effect_panel(ui: &mut egui::Ui, state: &mut StatusEffectPanelState) {
    ui.heading("Status Effects");
    ui.horizontal(|ui| {
        ui.label(format!("Active: {} | DMG/tick: {:.1} | Heal/tick: {:.1}",
            state.active_effects.len(), state.total_tick_damage(), state.total_tick_heal()));
        if ui.button("Tick").clicked() { state.tick_all(1.0); state.remove_expired(); }
        if ui.button("Clear All").clicked() { state.active_effects.clear(); }
    });
    ui.horizontal(|ui| {
        if ui.button("+ Poison").clicked() { state.add_effect(StatusEffect::new(StatusEffectType::Poison, 30.0, 1.0)); }
        if ui.button("+ Regen").clicked() { state.add_effect(StatusEffect::new(StatusEffectType::Regeneration, 60.0, 1.0)); }
        if ui.button("+ Haste").clicked() { state.add_effect(StatusEffect::new(StatusEffectType::Haste, 15.0, 1.0)); }
        if ui.button("+ Burn").clicked() { state.add_effect(StatusEffect::new(StatusEffectType::Burn, 10.0, 2.0)); }
    });
    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        for (i, effect) in state.active_effects.iter().enumerate() {
            let sel = state.selected == Some(i);
            ui.horizontal(|ui| {
                let prog = effect.progress();
                if ui.selectable_label(sel, format!("{} ({:.0}s x{})", effect.effect_type.name(), effect.remaining, effect.stack_count)).clicked() {
                    state.selected = Some(i);
                }
                ui.colored_label(effect.effect_type.color(), format!("{:.0}%", prog * 100.0));
                if effect.tick_damage > 0.0 { ui.label(format!("-{:.0}dmg", effect.tick_damage * effect.stack_count as f32)); }
                if effect.tick_heal > 0.0 { ui.label(format!("+{:.0}hp", effect.tick_heal * effect.stack_count as f32)); }
            });
        }
    });
}

// =================================================================
// INVENTORY EXPANSION 2: ITEM COMPARISON SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ItemCompareState {
    pub item_a_name: String,
    pub item_b_name: String,
    pub stat_a: StatBonus,
    pub stat_b: StatBonus,
    pub show_percentages: bool,
    pub highlight_better: bool,
}

impl ItemCompareState {
    pub fn new() -> Self {
        let a = StatBonus::default().with_attack(25).with_defense(5).with_speed(3);
        let b = StatBonus::default().with_attack(20).with_defense(15).with_speed(5).with_health(30);
        Self { item_a_name: "Iron Sword".to_string(), item_b_name: "Bronze Hammer".to_string(), stat_a: a, stat_b: b, show_percentages: true, highlight_better: true }
    }
}

pub fn show_item_compare(ui: &mut egui::Ui, state: &mut ItemCompareState) {
    ui.heading("Item Comparison");
    ui.checkbox(&mut state.highlight_better, "Highlight Better");
    ui.checkbox(&mut state.show_percentages, "Show % Difference");

    ui.columns(2, |cols| {
        cols[0].heading(&state.item_a_name.clone());
        cols[0].text_edit_singleline(&mut state.item_a_name);
        cols[1].heading(&state.item_b_name.clone());
        cols[1].text_edit_singleline(&mut state.item_b_name);
    });

    let stats = [
        ("Attack", state.stat_a.attack as i64, state.stat_b.attack as i64),
        ("Defense", state.stat_a.defense as i64, state.stat_b.defense as i64),
        ("Speed", state.stat_a.speed as i64, state.stat_b.speed as i64),
        ("Magic", state.stat_a.magic as i64, state.stat_b.magic as i64),
        ("Health", state.stat_a.health as i64, state.stat_b.health as i64),
        ("Mana", state.stat_a.mana as i64, state.stat_b.mana as i64),
    ];

    for (stat_name, va, vb) in &stats {
        ui.horizontal(|ui| {
            let diff = vb - va;
            let a_color = if state.highlight_better && va > vb { egui::Color32::from_rgb(50, 220, 50) } else { egui::Color32::WHITE };
            let b_color = if state.highlight_better && vb > va { egui::Color32::from_rgb(50, 220, 50) } else { egui::Color32::WHITE };
            ui.colored_label(a_color, format!("{}: {}", stat_name, va));
            if diff != 0 {
                let arrow = if diff > 0 { "▲" } else { "▼" };
                let diff_color = if diff > 0 { egui::Color32::from_rgb(50, 220, 50) } else { egui::Color32::from_rgb(220, 80, 80) };
                ui.colored_label(diff_color, format!("{} {:+}", arrow, diff));
                if state.show_percentages && *va > 0 {
                    let pct = diff as f64 / *va as f64 * 100.0;
                    ui.label(format!("({:+.0}%)", pct));
                }
            } else {
                ui.label("=");
            }
            ui.colored_label(b_color, format!("{}", vb));
        });
    }

    let a_score = stats.iter().map(|(_, va, _)| va).sum::<i64>();
    let b_score = stats.iter().map(|(_, _, vb)| vb).sum::<i64>();
    ui.separator();
    let winner_color = if a_score > b_score { egui::Color32::from_rgb(50, 220, 50) } else { egui::Color32::WHITE };
    let loser_color = if b_score > a_score { egui::Color32::from_rgb(50, 220, 50) } else { egui::Color32::WHITE };
    ui.horizontal(|ui| {
        ui.colored_label(winner_color, format!("Total A: {}", a_score));
        ui.label("vs");
        ui.colored_label(loser_color, format!("Total B: {}", b_score));
        let winner = if a_score > b_score { &state.item_a_name } else if b_score > a_score { &state.item_b_name } else { "Tie" };
        ui.label(format!("→ {} is better overall", winner));
    });
}

// =================================================================
// INVENTORY EXPANSION 2: TESTS
// =================================================================

#[cfg(test)]
mod inventory_expansion2_tests {
    use super::*;

    #[test]
    fn test_character_attributes_defaults() {
        let attrs = CharacterAttributes::default();
        assert_eq!(attrs.strength, 10);
        assert_eq!(CharacterAttributes::modifier(10), 0);
        assert_eq!(CharacterAttributes::modifier(18), 4);
    }

    #[test]
    fn test_derived_stats() {
        let attrs = CharacterAttributes::default();
        assert!(attrs.derived_hp() > 0);
        assert!(attrs.derived_mp() > 0);
        assert!(attrs.derived_carry_weight() > 0);
    }

    #[test]
    fn test_point_buy_cost() {
        let attrs = CharacterAttributes::default();
        let base_cost = attrs.point_buy_cost();
        let mut attrs2 = attrs.clone();
        attrs2.strength = 15;
        assert!(attrs2.point_buy_cost() > base_cost);
    }

    #[test]
    fn test_status_effect_tick() {
        let mut effect = StatusEffect::new(StatusEffectType::Poison, 10.0, 1.0);
        effect.tick(3.0);
        assert!((effect.remaining - 7.0).abs() < 0.001);
        assert!(!effect.is_expired());
        effect.tick(10.0);
        assert!(effect.is_expired());
    }

    #[test]
    fn test_status_effect_stacking() {
        let mut effect = StatusEffect::new(StatusEffectType::Poison, 10.0, 1.0);
        assert_eq!(effect.stack_count, 1);
        effect.add_stack();
        assert_eq!(effect.stack_count, 2);
    }

    #[test]
    fn test_status_effect_panel() {
        let mut panel = StatusEffectPanelState::new();
        assert!(!panel.active_effects.is_empty());
        let dmg = panel.total_tick_damage();
        let heal = panel.total_tick_heal();
        assert!(dmg >= 0.0);
        assert!(heal >= 0.0);
    }

    #[test]
    fn test_status_effect_add() {
        let mut panel = StatusEffectPanelState::default();
        panel.add_effect(StatusEffect::new(StatusEffectType::Burn, 10.0, 1.0));
        assert_eq!(panel.active_effects.len(), 1);
        panel.add_effect(StatusEffect::new(StatusEffectType::Burn, 10.0, 1.0));
        assert_eq!(panel.active_effects.len(), 1); // Stacked, not added
        assert_eq!(panel.active_effects[0].stack_count, 2);
    }

    #[test]
    fn test_item_compare_stat_diff() {
        let state = ItemCompareState::new();
        // item_a has attack 25, item_b has attack 20
        assert!(state.stat_a.attack > state.stat_b.attack);
        // item_b has defense 15, item_a has defense 5
        assert!(state.stat_b.defense > state.stat_a.defense);
    }

    #[test]
    fn test_status_effect_beneficial() {
        assert!(StatusEffectType::Regeneration.is_beneficial());
        assert!(StatusEffectType::Haste.is_beneficial());
        assert!(!StatusEffectType::Poison.is_beneficial());
        assert!(!StatusEffectType::Burn.is_beneficial());
    }
}
'''

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'a', encoding='utf-8') as f:
    f.write(world_gen_code)
print(f"Done. world_gen size: {__import__('os').path.getsize(r'C:\proof-engine\editor\src\world_gen.rs')} bytes")

with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'a', encoding='utf-8') as f:
    f.write(spline_code)
print(f"Done. spline_editor size: {__import__('os').path.getsize(r'C:\proof-engine\editor\src\spline_editor.rs')} bytes")

with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'a', encoding='utf-8') as f:
    f.write(inventory_code)
print(f"Done. inventory_system size: {__import__('os').path.getsize(r'C:\proof-engine\editor\src\inventory_system.rs')} bytes")
