use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Shape, RichText};
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

pub type QuestId = usize;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum QuestCategory {
    Main,
    Side,
    Daily,
    Hidden,
    Tutorial,
}

impl QuestCategory {
    pub fn label(&self) -> &'static str {
        match self {
            QuestCategory::Main => "Main",
            QuestCategory::Side => "Side",
            QuestCategory::Daily => "Daily",
            QuestCategory::Hidden => "Hidden",
            QuestCategory::Tutorial => "Tutorial",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            QuestCategory::Main => Color32::from_rgb(255, 200, 50),
            QuestCategory::Side => Color32::from_rgb(100, 200, 255),
            QuestCategory::Daily => Color32::from_rgb(100, 255, 150),
            QuestCategory::Hidden => Color32::from_rgb(180, 100, 255),
            QuestCategory::Tutorial => Color32::from_rgb(200, 200, 200),
        }
    }

    pub fn all() -> &'static [QuestCategory] {
        &[
            QuestCategory::Main,
            QuestCategory::Side,
            QuestCategory::Daily,
            QuestCategory::Hidden,
            QuestCategory::Tutorial,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum QuestState {
    NotStarted,
    Active,
    Completed,
    Failed,
    Abandoned,
}

impl QuestState {
    pub fn label(&self) -> &'static str {
        match self {
            QuestState::NotStarted => "Not Started",
            QuestState::Active => "Active",
            QuestState::Completed => "Completed",
            QuestState::Failed => "Failed",
            QuestState::Abandoned => "Abandoned",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            QuestState::NotStarted => Color32::from_rgb(150, 150, 150),
            QuestState::Active => Color32::from_rgb(100, 200, 255),
            QuestState::Completed => Color32::from_rgb(100, 255, 100),
            QuestState::Failed => Color32::from_rgb(255, 80, 80),
            QuestState::Abandoned => Color32::from_rgb(200, 150, 50),
        }
    }

    pub fn all() -> &'static [QuestState] {
        &[
            QuestState::NotStarted,
            QuestState::Active,
            QuestState::Completed,
            QuestState::Failed,
            QuestState::Abandoned,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ObjectiveType {
    Kill { target: String, enemy_type: String, count: u32, current: u32 },
    Collect { item: String, count: u32, amount: u32, current: u32 },
    Reach { location: String },
    Talk { character: String, npc_id: String, topic: String },
    Craft { item: String, count: u32, recipe_id: String, amount: u32 },
    Survive { duration: f32, elapsed: f32 },
    Escort { npc: String, npc_id: String, destination: String },
    Protect { target: String, duration: f32 },
    Explore { area: String, zone_id: String, percent: u32 },
    Custom { condition: String },
}

impl ObjectiveType {
    pub fn label(&self) -> &'static str {
        match self {
            ObjectiveType::Kill { .. } => "Kill",
            ObjectiveType::Collect { .. } => "Collect",
            ObjectiveType::Reach { .. } => "Reach",
            ObjectiveType::Talk { .. } => "Talk",
            ObjectiveType::Craft { .. } => "Craft",
            ObjectiveType::Survive { .. } => "Survive",
            ObjectiveType::Escort { .. } => "Escort",
            ObjectiveType::Protect { .. } => "Protect",
            ObjectiveType::Explore { .. } => "Explore",
            ObjectiveType::Custom { .. } => "Custom",
        }
    }

    pub fn type_index(&self) -> usize {
        match self {
            ObjectiveType::Kill { .. } => 0,
            ObjectiveType::Collect { .. } => 1,
            ObjectiveType::Reach { .. } => 2,
            ObjectiveType::Talk { .. } => 3,
            ObjectiveType::Craft { .. } => 4,
            ObjectiveType::Survive { .. } => 5,
            ObjectiveType::Escort { .. } => 6,
            ObjectiveType::Protect { .. } => 7,
            ObjectiveType::Explore { .. } => 8,
            ObjectiveType::Custom { .. } => 9,
        }
    }

    pub fn type_labels() -> &'static [&'static str] {
        &[
            "Kill", "Collect", "Reach", "Talk", "Craft",
            "Survive", "Escort", "Protect", "Explore", "Custom",
        ]
    }

    pub fn default_for_index(idx: usize) -> ObjectiveType {
        match idx {
            0 => ObjectiveType::Kill { target: "Enemy".to_string(), enemy_type: "Enemy".to_string(), count: 1, current: 0 },
            1 => ObjectiveType::Collect { item: "Item".to_string(), count: 1, amount: 1, current: 0 },
            2 => ObjectiveType::Reach { location: "Location".to_string() },
            3 => ObjectiveType::Talk { character: "NPC".to_string(), npc_id: String::new(), topic: String::new() },
            4 => ObjectiveType::Craft { item: "Item".to_string(), count: 1, recipe_id: String::new(), amount: 1 },
            5 => ObjectiveType::Survive { duration: 60.0, elapsed: 0.0 },
            6 => ObjectiveType::Escort { npc: "NPC".to_string(), npc_id: String::new(), destination: String::new() },
            7 => ObjectiveType::Protect { target: "Target".to_string(), duration: 120.0 },
            8 => ObjectiveType::Explore { area: "Area".to_string(), zone_id: String::new(), percent: 100 },
            _ => ObjectiveType::Custom { condition: "condition".to_string() },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestObjective {
    pub id: usize,
    pub description: String,
    pub objective_type: ObjectiveType,
    pub required: bool,
    pub current_progress: u32,
    pub required_progress: u32,
    pub optional: bool,
    pub hidden: bool,
    pub order: u32,
    pub completed: bool,
    pub hint: String,
}

impl QuestObjective {
    pub fn new(id: usize) -> Self {
        QuestObjective {
            id,
            description: "New Objective".to_string(),
            objective_type: ObjectiveType::Kill { target: "Enemy".to_string(), enemy_type: "Enemy".to_string(), count: 1, current: 0 },
            required: true,
            current_progress: 0,
            required_progress: 1,
            optional: false,
            hidden: false,
            order: 0,
            completed: false,
            hint: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Reward {
    Experience(u32),
    Gold(u32),
    Item { item_id: String, quantity: u32 },
    Ability(String),
    Reputation { faction: String, amount: i32 },
    Unlock { feature: String },
    Skill { skill_id: String, points: u32 },
}

impl Reward {
    pub fn label(&self) -> String {
        match self {
            Reward::Experience(xp) => format!("{} XP", xp),
            Reward::Gold(g) => format!("{} Gold", g),
            Reward::Item { item_id, quantity } => format!("{}x {}", quantity, item_id),
            Reward::Ability(a) => format!("Ability: {}", a),
            Reward::Reputation { faction, amount } => format!("{} rep: {}", faction, amount),
            Reward::Unlock { feature } => format!("Unlock: {}", feature),
            Reward::Skill { skill_id, points } => format!("Skill {}: +{}", skill_id, points),
        }
    }

    pub fn type_index(&self) -> usize {
        match self {
            Reward::Experience(_) => 0,
            Reward::Gold(_) => 1,
            Reward::Item { .. } => 2,
            Reward::Ability(_) => 3,
            Reward::Reputation { .. } => 4,
            Reward::Unlock { .. } => 5,
            Reward::Skill { .. } => 6,
        }
    }

    pub fn type_labels() -> &'static [&'static str] {
        &["Experience", "Gold", "Item", "Ability", "Reputation", "Unlock", "Skill"]
    }

    pub fn default_for_index(idx: usize) -> Reward {
        match idx {
            0 => Reward::Experience(100),
            1 => Reward::Gold(50),
            2 => Reward::Item { item_id: "item_id".to_string(), quantity: 1 },
            3 => Reward::Ability("new_ability".to_string()),
            4 => Reward::Reputation { faction: "faction".to_string(), amount: 10 },
            5 => Reward::Unlock { feature: "content_id".to_string() },
            _ => Reward::Skill { skill_id: "skill_id".to_string(), points: 1 },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestFlag {
    pub name: String,
    pub value: bool,
    pub set_on_complete: Vec<QuestId>,
    pub set_on_fail: Vec<QuestId>,
    pub checked_by: Vec<QuestId>,
}

impl QuestFlag {
    pub fn new(name: &str) -> Self {
        QuestFlag {
            name: name.to_string(),
            value: false,
            set_on_complete: Vec::new(),
            set_on_fail: Vec::new(),
            checked_by: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Faction {
    pub name: String,
    pub current_rep: i32,
    pub thresholds: (i32, i32, i32, i32),
    pub description: String,
    pub color: [u8; 3],
}

impl Faction {
    pub fn new(name: &str) -> Self {
        Faction {
            name: name.to_string(),
            current_rep: 0,
            thresholds: (-500, -100, 100, 500),
            description: String::new(),
            color: [150, 150, 150],
        }
    }

    pub fn standing_label(&self) -> &'static str {
        if self.current_rep < self.thresholds.0 {
            "Hostile"
        } else if self.current_rep < self.thresholds.1 {
            "Unfriendly"
        } else if self.current_rep < self.thresholds.2 {
            "Neutral"
        } else if self.current_rep < self.thresholds.3 {
            "Friendly"
        } else {
            "Allied"
        }
    }

    pub fn standing_color(&self) -> Color32 {
        if self.current_rep < self.thresholds.0 {
            Color32::from_rgb(220, 50, 50)
        } else if self.current_rep < self.thresholds.1 {
            Color32::from_rgb(230, 130, 50)
        } else if self.current_rep < self.thresholds.2 {
            Color32::from_rgb(180, 180, 180)
        } else if self.current_rep < self.thresholds.3 {
            Color32::from_rgb(80, 200, 80)
        } else {
            Color32::from_rgb(80, 150, 255)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quest {
    pub id: QuestId,
    pub name: String,
    pub description: String,
    pub category: QuestCategory,
    pub state: QuestState,
    pub priority: u32,
    pub prereqs: Vec<QuestId>,
    pub rewards: Vec<Reward>,
    pub objectives: Vec<QuestObjective>,
    pub flags_on_complete: Vec<String>,
    pub flags_on_fail: Vec<String>,
    pub recommended_level: u32,
    pub time_limit: Option<f32>,
    pub repeatable: bool,
    pub objective_next_id: usize,
    #[serde(skip, default)]
    pub graph_pos: Vec2,
    pub prerequisites: Vec<QuestId>,
    pub tags: Vec<String>,
    pub notes: String,
    pub auto_complete: bool,
    pub hidden: bool,
    pub cooldown_hours: u32,
    pub fail_conditions: Vec<String>,
    pub lore_entries: Vec<String>,
    pub level_requirement: u32,
    pub graph_position: [f32; 2],
}

impl Quest {
    pub fn new(id: QuestId) -> Self {
        Quest {
            id,
            name: format!("Quest {}", id),
            description: String::new(),
            category: QuestCategory::Side,
            state: QuestState::NotStarted,
            priority: 0,
            prereqs: Vec::new(),
            rewards: Vec::new(),
            objectives: Vec::new(),
            flags_on_complete: Vec::new(),
            flags_on_fail: Vec::new(),
            recommended_level: 1,
            time_limit: None,
            repeatable: false,
            objective_next_id: 0,
            graph_pos: Vec2::new(
                (id as f32 % 5.0) * 180.0 + 40.0,
                (id as f32 / 5.0).floor() * 130.0 + 40.0,
            ),
            prerequisites: Vec::new(),
            tags: Vec::new(),
            notes: String::new(),
            auto_complete: false,
            hidden: false,
            cooldown_hours: 0,
            fail_conditions: Vec::new(),
            lore_entries: Vec::new(),
            level_requirement: 1,
            graph_position: [(id as f32 % 5.0) * 180.0 + 40.0, (id as f32 / 5.0).floor() * 130.0 + 40.0],
        }
    }

    pub fn progress(&self) -> f32 {
        let required: Vec<&QuestObjective> = self.objectives.iter().filter(|o| o.required).collect();
        if required.is_empty() {
            return 0.0;
        }
        let done = required.iter().filter(|o| o.current_progress >= o.required_progress).count();
        done as f32 / required.len() as f32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum QuestView {
    List,
    Graph,
    Timeline,
}

#[derive(Clone, Debug, Default)]
pub struct GraphState {
    pub dragging_node: Option<usize>,
    pub drag_offset: Vec2,
}

pub struct QuestEditor {
    pub quests: Vec<Quest>,
    pub selected: Option<usize>,
    pub view: QuestView,
    pub filter_state: Option<QuestState>,
    pub filter_category: Option<QuestCategory>,
    pub search: String,
    pub canvas_offset: Vec2,
    pub canvas_zoom: f32,
    pub flags: Vec<QuestFlag>,
    pub factions: Vec<Faction>,
    pub show_flags: bool,
    pub show_factions: bool,
    pub show_rewards: bool,
    pub next_id: QuestId,
    pub graph_state: GraphState,
    pub new_flag_name: String,
    pub new_faction_name: String,
    pub dragging_quest: Option<usize>,
    pub drag_start: Option<egui::Pos2>,
    pub show_detail: bool,
    pub rename_quest: Option<(usize, String)>,
    pub prereq_search: String,
    pub expanded_objectives: HashSet<usize>,
    pub timeline_scroll: f32,
    pub reward_type_sel: usize,
    pub obj_type_sel: usize,
}

impl QuestEditor {
    pub fn new() -> Self {
        let mut editor = QuestEditor {
            quests: Vec::new(),
            selected: None,
            view: QuestView::List,
            filter_state: None,
            filter_category: None,
            search: String::new(),
            canvas_offset: Vec2::ZERO,
            canvas_zoom: 1.0,
            flags: Vec::new(),
            factions: Vec::new(),
            show_flags: false,
            show_factions: false,
            show_rewards: false,
            next_id: 0,
            graph_state: GraphState::default(),
            new_flag_name: String::new(),
            new_faction_name: String::new(),
            dragging_quest: None,
            drag_start: None,
            show_detail: true,
            rename_quest: None,
            prereq_search: String::new(),
            expanded_objectives: HashSet::new(),
            timeline_scroll: 0.0,
            reward_type_sel: 0,
            obj_type_sel: 0,
        };
        editor.populate_demo_data();
        editor
    }

    fn populate_demo_data(&mut self) {
        let mut q0 = Quest::new(self.next_id);
        q0.name = "A Hero's Calling".to_string();
        q0.description = "The ancient evil stirs. The oracle has called for a champion.".to_string();
        q0.category = QuestCategory::Main;
        q0.state = QuestState::Completed;
        q0.priority = 100;
        q0.recommended_level = 1;
        q0.rewards = vec![Reward::Experience(500), Reward::Gold(100)];
        let mut obj = QuestObjective::new(0);
        obj.description = "Speak with the Oracle".to_string();
        obj.objective_type = ObjectiveType::Talk { character: "Oracle Vera".to_string(), npc_id: String::new(), topic: String::new() };
        obj.required_progress = 1;
        obj.current_progress = 1;
        q0.objectives.push(obj);
        q0.objective_next_id = 1;
        q0.graph_pos = Vec2::new(100.0, 100.0);
        self.quests.push(q0);
        self.next_id += 1;

        let mut q1 = Quest::new(self.next_id);
        q1.name = "Into the Dark Wood".to_string();
        q1.description = "Clear the bandits from Ashvale Forest.".to_string();
        q1.category = QuestCategory::Main;
        q1.state = QuestState::Active;
        q1.priority = 90;
        q1.prereqs = vec![0];
        q1.recommended_level = 3;
        q1.rewards = vec![Reward::Experience(800), Reward::Gold(200), Reward::Item { item_id: "iron_sword".to_string(), quantity: 1 }];
        let mut obj = QuestObjective::new(0);
        obj.description = "Kill 10 bandits".to_string();
        obj.objective_type = ObjectiveType::Kill { target: "Bandit".to_string(), enemy_type: "Bandit".to_string(), count: 10, current: 0 };
        obj.required_progress = 10;
        obj.current_progress = 4;
        q1.objectives.push(obj);
        let mut obj2 = QuestObjective::new(1);
        obj2.description = "Find the bandit leader's camp".to_string();
        obj2.objective_type = ObjectiveType::Reach { location: "Bandit Camp".to_string() }; // Reach has no extra fields
        obj2.required_progress = 1;
        obj2.current_progress = 0;
        q1.objectives.push(obj2);
        q1.objective_next_id = 2;
        q1.graph_pos = Vec2::new(320.0, 100.0);
        self.quests.push(q1);
        self.next_id += 1;

        let mut q2 = Quest::new(self.next_id);
        q2.name = "The Lost Merchant".to_string();
        q2.description = "Help Aldric find his missing shipment.".to_string();
        q2.category = QuestCategory::Side;
        q2.state = QuestState::NotStarted;
        q2.priority = 30;
        q2.recommended_level = 2;
        q2.rewards = vec![Reward::Gold(150), Reward::Reputation { faction: "Merchants Guild".to_string(), amount: 25 }];
        let mut obj = QuestObjective::new(0);
        obj.description = "Find the missing crates".to_string();
        obj.objective_type = ObjectiveType::Collect { item: "Merchant Crate".to_string(), count: 3, amount: 3, current: 0 };
        obj.required_progress = 3;
        q2.objectives.push(obj);
        q2.objective_next_id = 1;
        q2.graph_pos = Vec2::new(540.0, 200.0);
        self.quests.push(q2);
        self.next_id += 1;

        let mut q3 = Quest::new(self.next_id);
        q3.name = "Daily: Gather Herbs".to_string();
        q3.description = "Collect healing herbs from the meadow.".to_string();
        q3.category = QuestCategory::Daily;
        q3.state = QuestState::NotStarted;
        q3.priority = 10;
        q3.recommended_level = 1;
        q3.repeatable = true;
        q3.rewards = vec![Reward::Experience(100), Reward::Gold(30)];
        let mut obj = QuestObjective::new(0);
        obj.description = "Gather 5 Silverleaf".to_string();
        obj.objective_type = ObjectiveType::Collect { item: "Silverleaf".to_string(), count: 5, amount: 5, current: 0 };
        obj.required_progress = 5;
        q3.objectives.push(obj);
        q3.objective_next_id = 1;
        q3.graph_pos = Vec2::new(100.0, 300.0);
        self.quests.push(q3);
        self.next_id += 1;

        let mut q4 = Quest::new(self.next_id);
        q4.name = "Tutorial: First Steps".to_string();
        q4.description = "Learn the basics of combat and exploration.".to_string();
        q4.category = QuestCategory::Tutorial;
        q4.state = QuestState::Completed;
        q4.priority = 999;
        q4.recommended_level = 1;
        q4.rewards = vec![Reward::Experience(50)];
        q4.graph_pos = Vec2::new(320.0, 300.0);
        self.quests.push(q4);
        self.next_id += 1;

        self.flags.push(QuestFlag::new("oracle_spoken"));
        self.flags.push(QuestFlag::new("forest_cleared"));
        self.flags.push(QuestFlag::new("merchant_helped"));
        self.flags[0].value = true;

        let mut merchants = Faction::new("Merchants Guild");
        merchants.current_rep = 125;
        merchants.color = [200, 160, 50];
        self.factions.push(merchants);

        let mut rangers = Faction::new("Forest Rangers");
        rangers.current_rep = -50;
        rangers.color = [80, 160, 80];
        self.factions.push(rangers);

        let mut crown = Faction::new("The Crown");
        crown.current_rep = 200;
        crown.color = [180, 150, 255];
        self.factions.push(crown);
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let search_lower = self.search.to_lowercase();
        self.quests.iter().enumerate()
            .filter(|(_, q)| {
                let name_match = search_lower.is_empty() || q.name.to_lowercase().contains(&search_lower);
                let state_match = self.filter_state.as_ref().map_or(true, |s| &q.state == s);
                let cat_match = self.filter_category.as_ref().map_or(true, |c| &q.category == c);
                name_match && state_match && cat_match
            })
            .map(|(i, _)| i)
            .collect()
    }
}

pub fn show(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("Quest & Narrative Editor").size(18.0).color(Color32::from_rgb(220, 200, 100)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ Quest").clicked() {
                let id = editor.next_id;
                editor.quests.push(Quest::new(id));
                editor.next_id += 1;
                editor.selected = Some(editor.quests.len() - 1);
            }
            ui.toggle_value(&mut editor.show_factions, "Factions");
            ui.toggle_value(&mut editor.show_flags, "Flags");
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("View:");
        if ui.selectable_label(editor.view == QuestView::List, "List").clicked() {
            editor.view = QuestView::List;
        }
        if ui.selectable_label(editor.view == QuestView::Graph, "Graph").clicked() {
            editor.view = QuestView::Graph;
        }
        if ui.selectable_label(editor.view == QuestView::Timeline, "Timeline").clicked() {
            editor.view = QuestView::Timeline;
        }
        ui.separator();
        ui.label("Filter:");
        let state_label = editor.filter_state.as_ref().map_or("Any State", |s| s.label());
        egui::ComboBox::from_id_salt("filter_state")
            .selected_text(state_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(editor.filter_state.is_none(), "Any State").clicked() {
                    editor.filter_state = None;
                }
                for state in QuestState::all() {
                    if ui.selectable_label(editor.filter_state.as_ref() == Some(state), state.label()).clicked() {
                        editor.filter_state = Some(state.clone());
                    }
                }
            });
        let cat_label = editor.filter_category.as_ref().map_or("Any Category", |c| c.label());
        egui::ComboBox::from_id_salt("filter_cat")
            .selected_text(cat_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(editor.filter_category.is_none(), "Any Category").clicked() {
                    editor.filter_category = None;
                }
                for cat in QuestCategory::all() {
                    if ui.selectable_label(editor.filter_category.as_ref() == Some(cat), cat.label()).clicked() {
                        editor.filter_category = Some(cat.clone());
                    }
                }
            });
        ui.add(egui::TextEdit::singleline(&mut editor.search).hint_text("Search quests...").desired_width(180.0));
        if !editor.search.is_empty() {
            if ui.small_button("x").clicked() {
                editor.search.clear();
            }
        }
    });
    ui.separator();

    if editor.show_flags {
        show_flag_inspector(ui, editor);
        ui.separator();
    }

    if editor.show_factions {
        show_faction_editor(ui, editor);
        ui.separator();
    }

    egui::SidePanel::right("quest_detail_panel")
        .resizable(true)
        .default_width(380.0)
        .show_inside(ui, |ui| {
            if let Some(sel) = editor.selected {
                if sel < editor.quests.len() {
                    show_quest_detail(ui, editor, sel);
                } else {
                    editor.selected = None;
                    ui.label("No quest selected.");
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Select a quest to edit").color(Color32::GRAY));
                });
            }
        });

    match editor.view.clone() {
        QuestView::List => show_list_view(ui, editor),
        QuestView::Graph => show_graph_view(ui, editor),
        QuestView::Timeline => show_timeline_view(ui, editor),
    }
}

fn show_list_view(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    let indices = editor.filtered_indices();
    egui::ScrollArea::vertical()
        .id_salt("quest_list_scroll")
        .show(ui, |ui| {
            let row_height = 28.0;
            egui::Grid::new("quest_list_grid")
                .num_columns(6)
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.strong("Status");
                    ui.strong("Category");
                    ui.strong("Name");
                    ui.strong("Priority");
                    ui.strong("Progress");
                    ui.strong("Level");
                    ui.end_row();

                    let mut to_select: Option<usize> = None;
                    let mut to_delete: Option<usize> = None;

                    for &idx in &indices {
                        let quest = &editor.quests[idx];
                        let is_selected = editor.selected == Some(idx);

                        let state_color = quest.state.color();
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 6.0, state_color);
                            ui.label(RichText::new(quest.state.label()).small().color(state_color));
                        });

                        let cat_color = quest.category.color();
                        ui.label(RichText::new(quest.category.label()).color(cat_color).small());

                        let name_label = if quest.repeatable {
                            format!("{} [R]", quest.name)
                        } else {
                            quest.name.clone()
                        };
                        let resp = ui.selectable_label(
                            is_selected,
                            RichText::new(&name_label).color(if is_selected { Color32::WHITE } else { Color32::LIGHT_GRAY })
                        );
                        if resp.clicked() {
                            to_select = Some(idx);
                        }
                        resp.context_menu(|ui| {
                            if ui.button("Duplicate").clicked() {
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button(RichText::new("Delete").color(Color32::from_rgb(255, 80, 80))).clicked() {
                                to_delete = Some(idx);
                                ui.close_menu();
                            }
                        });

                        ui.label(format!("{}", quest.priority));

                        let progress = quest.progress();
                        let bar_width = 80.0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, row_height - 6.0), egui::Sense::hover());
                        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(bar_width * progress, rect.height()));
                        let bg_color = Color32::from_rgb(40, 40, 40);
                        let fill_color = if progress >= 1.0 { Color32::from_rgb(80, 200, 80) } else { Color32::from_rgb(80, 150, 220) };
                        ui.painter().rect_filled(rect, 3.0, bg_color);
                        ui.painter().rect_filled(fill_rect, 3.0, fill_color);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{:.0}%", progress * 100.0),
                            FontId::proportional(10.0),
                            Color32::WHITE,
                        );

                        ui.label(format!("Lv.{}", quest.recommended_level));
                        ui.end_row();
                    }

                    if let Some(idx) = to_select {
                        editor.selected = Some(idx);
                    }
                    if let Some(idx) = to_delete {
                        editor.quests.remove(idx);
                        if editor.selected == Some(idx) {
                            editor.selected = None;
                        }
                    }
                });

            if indices.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("No quests match the current filter.").color(Color32::GRAY));
                });
            }
        });
}

fn show_graph_view(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 22));

    // Grid dots
    let zoom = editor.canvas_zoom;
    let offset = editor.canvas_offset;
    let grid_spacing = 40.0 * zoom;
    let start_x = rect.min.x + (offset.x % grid_spacing);
    let start_y = rect.min.y + (offset.y % grid_spacing);
    let mut gx = start_x;
    while gx < rect.max.x {
        let mut gy = start_y;
        while gy < rect.max.y {
            painter.circle_filled(Pos2::new(gx, gy), 1.0, Color32::from_rgb(40, 40, 50));
            gy += grid_spacing;
        }
        gx += grid_spacing;
    }

    // Pan
    if response.dragged_by(egui::PointerButton::Secondary) {
        editor.canvas_offset += response.drag_delta();
    }

    // Zoom
    if let Some(pos) = response.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = 1.0 + scroll * 0.001;
            let new_zoom = (editor.canvas_zoom * factor).clamp(0.2, 4.0);
            let zoom_ratio = new_zoom / editor.canvas_zoom;
            let local = pos - rect.min;
            editor.canvas_offset = local - (local - editor.canvas_offset) * zoom_ratio;
            editor.canvas_zoom = new_zoom;
        }
    }

    let world_to_screen = |world_pos: Vec2| -> Pos2 {
        Pos2::new(
            rect.min.x + world_pos.x * zoom + offset.x,
            rect.min.y + world_pos.y * zoom + offset.y,
        )
    };

    let node_w = 150.0 * zoom;
    let node_h = 60.0 * zoom;

    // Draw edges (prereqs)
    for (idx, quest) in editor.quests.iter().enumerate() {
        let to_pos = world_to_screen(quest.graph_pos) + Vec2::new(node_w / 2.0, node_h / 2.0);
        for &prereq_id in &quest.prereqs {
            if let Some(prereq_idx) = editor.quests.iter().position(|q| q.id == prereq_id) {
                let from_pos = world_to_screen(editor.quests[prereq_idx].graph_pos) + Vec2::new(node_w / 2.0, node_h / 2.0);
                let edge_color = Color32::from_rgb(120, 120, 150);
                painter.line_segment([from_pos, to_pos], Stroke::new(1.5, edge_color));
                // Arrow head
                let dir = (to_pos - from_pos).normalized();
                let perp = Vec2::new(-dir.y, dir.x);
                let arrow_tip = to_pos - dir * (node_w * 0.5);
                let arrow_left = arrow_tip - dir * 12.0 * zoom + perp * 6.0 * zoom;
                let arrow_right = arrow_tip - dir * 12.0 * zoom - perp * 6.0 * zoom;
                painter.add(Shape::convex_polygon(
                    vec![arrow_tip, arrow_left, arrow_right],
                    edge_color,
                    Stroke::NONE,
                ));
            }
        }
    }

    // Draw nodes
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let pointer_pressed = ui.input(|i| i.pointer.primary_pressed());
    let pointer_released = ui.input(|i| i.pointer.primary_released());
    let pointer_down = ui.input(|i| i.pointer.primary_down());

    let mut clicked_node: Option<usize> = None;
    let mut start_drag: Option<(usize, Vec2)> = None;

    for (idx, quest) in editor.quests.iter().enumerate() {
        let screen_pos = world_to_screen(quest.graph_pos);
        let node_rect = Rect::from_min_size(screen_pos, Vec2::new(node_w, node_h));

        if !rect.intersects(node_rect) {
            continue;
        }

        let is_selected = editor.selected == Some(idx);
        let is_hovered = pointer_pos.map_or(false, |p| node_rect.contains(p));

        let base_color = quest.category.color();
        let bg_color = if is_selected {
            Color32::from_rgb(50, 60, 80)
        } else if is_hovered {
            Color32::from_rgb(38, 38, 50)
        } else {
            Color32::from_rgb(28, 28, 38)
        };

        painter.rect_filled(node_rect, 6.0 * zoom, bg_color);
        let border_color = if is_selected { Color32::WHITE } else { base_color };
        painter.rect_stroke(node_rect, 6.0 * zoom, Stroke::new(if is_selected { 2.0 } else { 1.5 }, border_color), egui::StrokeKind::Inside);

        // State dot
        let dot_pos = screen_pos + Vec2::new(10.0 * zoom, 10.0 * zoom);
        painter.circle_filled(dot_pos, 5.0 * zoom, quest.state.color());

        // Category label
        painter.text(
            screen_pos + Vec2::new(22.0 * zoom, 8.0 * zoom),
            egui::Align2::LEFT_TOP,
            quest.category.label(),
            FontId::proportional(9.0 * zoom),
            base_color,
        );

        // Quest name
        painter.text(
            screen_pos + Vec2::new(node_w / 2.0, node_h / 2.0 - 4.0 * zoom),
            egui::Align2::CENTER_CENTER,
            &quest.name,
            FontId::proportional(11.0 * zoom),
            Color32::WHITE,
        );

        // Progress bar
        let progress = quest.progress();
        let bar_rect = Rect::from_min_size(
            screen_pos + Vec2::new(8.0 * zoom, node_h - 14.0 * zoom),
            Vec2::new(node_w - 16.0 * zoom, 6.0 * zoom),
        );
        painter.rect_filled(bar_rect, 2.0, Color32::from_rgb(40, 40, 40));
        if progress > 0.0 {
            let fill = Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width() * progress, bar_rect.height()));
            painter.rect_filled(fill, 2.0, quest.state.color());
        }

        if is_hovered && pointer_pressed {
            clicked_node = Some(idx);
        }
    }

    // Handle node dragging
    if let Some(idx) = clicked_node {
        editor.selected = Some(idx);
        editor.graph_state.dragging_node = Some(idx);
        if let Some(pos) = pointer_pos {
            let screen_pos = world_to_screen(editor.quests[idx].graph_pos);
            editor.graph_state.drag_offset = pos - screen_pos;
        }
    }

    if pointer_down {
        if let Some(drag_idx) = editor.graph_state.dragging_node {
            if let Some(pos) = pointer_pos {
                let new_screen = pos - editor.graph_state.drag_offset;
                let world_x = (new_screen.x - rect.min.x - offset.x) / zoom;
                let world_y = (new_screen.y - rect.min.y - offset.y) / zoom;
                editor.quests[drag_idx].graph_pos = Vec2::new(world_x, world_y);
            }
        }
    }

    if pointer_released {
        editor.graph_state.dragging_node = None;
    }

    // Controls overlay
    let controls_rect = Rect::from_min_size(rect.min + Vec2::new(10.0, 10.0), Vec2::new(200.0, 22.0));
    painter.rect_filled(controls_rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180));
    painter.text(
        controls_rect.center(),
        egui::Align2::CENTER_CENTER,
        "Scroll: Zoom | RMB Drag: Pan | LMB: Select/Move",
        FontId::proportional(9.0),
        Color32::from_rgb(180, 180, 180),
    );

    // Zoom label
    let zoom_label_pos = rect.max - Vec2::new(60.0, 20.0);
    painter.text(
        zoom_label_pos,
        egui::Align2::CENTER_CENTER,
        format!("{:.0}%", zoom * 100.0),
        FontId::proportional(11.0),
        Color32::from_rgb(150, 150, 150),
    );
}

fn show_timeline_view(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    let available = ui.available_size();
    let header_height = 30.0;
    let row_height = 50.0;
    let level_width = 80.0;

    let max_level = editor.quests.iter().map(|q| q.recommended_level).max().unwrap_or(10).max(10);
    let total_width = max_level as f32 * level_width + 200.0;

    egui::ScrollArea::horizontal()
        .id_salt("timeline_scroll_h")
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("timeline_scroll_v")
                .show(ui, |ui| {
                    let num_quests = editor.quests.len();
                    let total_height = header_height + num_quests as f32 * row_height + 20.0;
                    let (response, painter) = ui.allocate_painter(
                        Vec2::new(total_width, total_height.max(available.y)),
                        egui::Sense::click(),
                    );
                    let rect = response.rect;

                    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 22));

                    // Header: level markers
                    for lvl in 1..=max_level {
                        let x = rect.min.x + 120.0 + (lvl as f32 - 1.0) * level_width + level_width / 2.0;
                        let header_y = rect.min.y + header_height / 2.0;
                        let is_major = lvl % 5 == 0;
                        painter.text(
                            Pos2::new(x, header_y),
                            egui::Align2::CENTER_CENTER,
                            format!("Lv.{}", lvl),
                            FontId::proportional(if is_major { 11.0 } else { 9.0 }),
                            if is_major { Color32::WHITE } else { Color32::GRAY },
                        );
                        painter.line_segment(
                            [Pos2::new(x, rect.min.y + header_height), Pos2::new(x, rect.max.y)],
                            Stroke::new(if is_major { 1.0 } else { 0.5 }, Color32::from_rgb(40, 40, 55)),
                        );
                    }

                    // Separator line under header
                    painter.line_segment(
                        [Pos2::new(rect.min.x, rect.min.y + header_height), Pos2::new(rect.max.x, rect.min.y + header_height)],
                        Stroke::new(1.0, Color32::from_rgb(60, 60, 80)),
                    );

                    // Draw quest rows
                    let mut clicked_quest: Option<usize> = None;
                    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

                    let quests_data: Vec<(usize, String, QuestCategory, QuestState, u32, Vec<usize>)> = editor.quests.iter().enumerate()
                        .map(|(i, q)| (i, q.name.clone(), q.category.clone(), q.state.clone(), q.recommended_level, q.prereqs.clone()))
                        .collect();

                    for (idx, name, category, state, level, prereqs) in &quests_data {
                        let row_y = rect.min.y + header_height + *idx as f32 * row_height;
                        let bar_y = row_y + row_height / 2.0 - 12.0;

                        // Category label column
                        painter.text(
                            Pos2::new(rect.min.x + 10.0, row_y + row_height / 2.0),
                            egui::Align2::LEFT_CENTER,
                            category.label(),
                            FontId::proportional(9.0),
                            category.color(),
                        );

                        // Quest bar
                        let bar_x = rect.min.x + 120.0 + (*level as f32 - 1.0) * level_width;
                        let bar_w = level_width * 1.5;
                        let bar_rect = Rect::from_min_size(Pos2::new(bar_x, bar_y), Vec2::new(bar_w, 24.0));

                        let is_selected = editor.selected == Some(*idx);
                        let is_hovered = pointer_pos.map_or(false, |p| bar_rect.contains(p));

                        let bar_fill = if is_selected {
                            Color32::from_rgb(60, 80, 120)
                        } else if is_hovered {
                            Color32::from_rgb(45, 55, 70)
                        } else {
                            Color32::from_rgb(30, 35, 50)
                        };

                        painter.rect_filled(bar_rect, 4.0, bar_fill);
                        painter.rect_stroke(bar_rect, 4.0, Stroke::new(1.5, category.color()), egui::StrokeKind::Inside);

                        // State dot
                        painter.circle_filled(
                            Pos2::new(bar_x + 10.0, bar_y + 12.0),
                            5.0,
                            state.color(),
                        );

                        painter.text(
                            Pos2::new(bar_x + 20.0, bar_y + 12.0),
                            egui::Align2::LEFT_CENTER,
                            name,
                            FontId::proportional(10.0),
                            Color32::WHITE,
                        );

                        if is_hovered && ui.input(|i| i.pointer.primary_pressed()) {
                            clicked_quest = Some(*idx);
                        }

                        // Row separator
                        painter.line_segment(
                            [Pos2::new(rect.min.x, row_y + row_height), Pos2::new(rect.max.x, row_y + row_height)],
                            Stroke::new(0.5, Color32::from_rgb(35, 35, 45)),
                        );
                    }

                    // Draw dependency arcs
                    for (idx, _name, _category, _state, level, prereqs) in &quests_data {
                        let to_x = rect.min.x + 120.0 + (*level as f32 - 1.0) * level_width;
                        let to_y = rect.min.y + header_height + *idx as f32 * row_height + row_height / 2.0;

                        for &prereq_id in prereqs {
                            if let Some((prereq_idx, _, _, _, prereq_level, _)) = quests_data.iter().find(|(i, _, _, _, _, _)| *i == prereq_id) {
                                let from_x = rect.min.x + 120.0 + (*prereq_level as f32 - 1.0) * level_width + level_width * 1.5;
                                let from_y = rect.min.y + header_height + *prereq_idx as f32 * row_height + row_height / 2.0;

                                let mid_x = (from_x + to_x) / 2.0;
                                let ctrl1 = Pos2::new(mid_x, from_y);
                                let ctrl2 = Pos2::new(mid_x, to_y);

                                // Approximate bezier with line segments
                                let steps = 20;
                                let mut prev = Pos2::new(from_x, from_y);
                                for step in 1..=steps {
                                    let t = step as f32 / steps as f32;
                                    let t1 = 1.0 - t;
                                    let bx = t1*t1*t1*from_x + 3.0*t1*t1*t*ctrl1.x + 3.0*t1*t*t*ctrl2.x + t*t*t*to_x;
                                    let by = t1*t1*t1*from_y + 3.0*t1*t1*t*ctrl1.y + 3.0*t1*t*t*ctrl2.y + t*t*t*to_y;
                                    let cur = Pos2::new(bx, by);
                                    painter.line_segment([prev, cur], Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 120, 200, 100)));
                                    prev = cur;
                                }
                            }
                        }
                    }

                    if let Some(idx) = clicked_quest {
                        editor.selected = Some(idx);
                    }
                });
        });
}

fn show_quest_detail(ui: &mut egui::Ui, editor: &mut QuestEditor, sel: usize) {
    ui.heading(RichText::new("Quest Detail").size(14.0).color(Color32::from_rgb(220, 200, 100)));
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("quest_detail_scroll")
        .show(ui, |ui| {
            let quest = &mut editor.quests[sel];
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut quest.name);
            });

            ui.horizontal(|ui| {
                ui.label("Category:");
                let cat_label = quest.category.label().to_string();
                egui::ComboBox::from_id_salt("quest_cat_detail")
                    .selected_text(cat_label)
                    .show_ui(ui, |ui| {
                        for cat in QuestCategory::all() {
                            let is_sel = &quest.category == cat;
                            if ui.selectable_label(is_sel, cat.label()).clicked() {
                                quest.category = cat.clone();
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("State:");
                let state_label = quest.state.label().to_string();
                egui::ComboBox::from_id_salt("quest_state_detail")
                    .selected_text(state_label)
                    .show_ui(ui, |ui| {
                        for state in QuestState::all() {
                            if ui.selectable_label(&quest.state == state, state.label()).clicked() {
                                quest.state = state.clone();
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Priority:");
                ui.add(egui::DragValue::new(&mut quest.priority).range(0..=999).speed(1.0));
                ui.label("Rec. Level:");
                ui.add(egui::DragValue::new(&mut quest.recommended_level).range(1..=100).speed(1.0));
            });

            ui.horizontal(|ui| {
                ui.checkbox(&mut quest.repeatable, "Repeatable");
                let mut has_limit = quest.time_limit.is_some();
                if ui.checkbox(&mut has_limit, "Time Limit").changed() {
                    quest.time_limit = if has_limit { Some(300.0) } else { None };
                }
                if let Some(ref mut limit) = quest.time_limit {
                    ui.add(egui::DragValue::new(limit).suffix("s").speed(1.0).range(0.0..=99999.0));
                }
            });

            ui.separator();
            ui.label("Description:");
            ui.add(egui::TextEdit::multiline(&mut quest.description)
                .desired_rows(3)
                .desired_width(f32::INFINITY));

            // Objectives
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Objectives");
                if ui.small_button("+ Add").clicked() {
                    let oid = quest.objective_next_id;
                    quest.objective_next_id += 1;
                    quest.objectives.push(QuestObjective::new(oid));
                }
            });

            let mut to_remove_obj: Option<usize> = None;
            let mut move_up: Option<usize> = None;
            let mut move_down: Option<usize> = None;

            for (oi, obj) in quest.objectives.iter_mut().enumerate() {
                let header_id = egui::Id::new("obj_header").with(obj.id);
                let is_expanded = editor.expanded_objectives.contains(&obj.id);

                ui.push_id(header_id, |ui| {
                    ui.horizontal(|ui| {
                        let expand_label = if is_expanded { "v" } else { ">" };
                        if ui.small_button(expand_label).clicked() {
                            if is_expanded {
                                editor.expanded_objectives.remove(&obj.id);
                            } else {
                                editor.expanded_objectives.insert(obj.id);
                            }
                        }

                        let type_color = Color32::from_rgb(150, 200, 255);
                        ui.label(RichText::new(format!("[{}]", obj.objective_type.label())).color(type_color).small());
                        ui.text_edit_singleline(&mut obj.description);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("x").clicked() {
                                to_remove_obj = Some(oi);
                            }
                            if oi > 0 && ui.small_button("^").clicked() {
                                move_up = Some(oi);
                            }
                            if ui.small_button("v").clicked() {
                                move_down = Some(oi);
                            }
                            ui.checkbox(&mut obj.hidden, "Hidden");
                            ui.checkbox(&mut obj.optional, "Optional");
                        });
                    });

                    if editor.expanded_objectives.contains(&obj.id) {
                        ui.indent("obj_detail", |ui| {
                            // Type selector
                            let cur_type_idx = obj.objective_type.type_index();
                            let type_labels = ObjectiveType::type_labels();
                            let mut selected_type = cur_type_idx;
                            egui::ComboBox::from_id_salt(egui::Id::new("obj_type_combo").with(obj.id))
                                .selected_text(type_labels[cur_type_idx])
                                .show_ui(ui, |ui| {
                                    for (i, label) in type_labels.iter().enumerate() {
                                        if ui.selectable_label(i == cur_type_idx, *label).clicked() {
                                            selected_type = i;
                                        }
                                    }
                                });
                            if selected_type != cur_type_idx {
                                obj.objective_type = ObjectiveType::default_for_index(selected_type);
                            }

                            match &mut obj.objective_type {
                                ObjectiveType::Kill { target, count, .. } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Target:");
                                        ui.text_edit_singleline(target);
                                        ui.label("Count:");
                                        ui.add(egui::DragValue::new(count).range(1..=9999).speed(1.0));
                                    });
                                }
                                ObjectiveType::Collect { item, count, .. } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Item:");
                                        ui.text_edit_singleline(item);
                                        ui.label("Count:");
                                        ui.add(egui::DragValue::new(count).range(1..=9999).speed(1.0));
                                    });
                                }
                                ObjectiveType::Reach { location } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Location:");
                                        ui.text_edit_singleline(location);
                                    });
                                }
                                ObjectiveType::Talk { character, .. } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Character:");
                                        ui.text_edit_singleline(character);
                                    });
                                }
                                ObjectiveType::Craft { item, count, .. } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Item:");
                                        ui.text_edit_singleline(item);
                                        ui.label("Count:");
                                        ui.add(egui::DragValue::new(count).range(1..=9999).speed(1.0));
                                    });
                                }
                                ObjectiveType::Survive { duration, .. } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Duration (s):");
                                        ui.add(egui::DragValue::new(duration).suffix("s").speed(1.0).range(1.0..=99999.0));
                                    });
                                }
                                ObjectiveType::Escort { npc, .. } => {
                                    ui.horizontal(|ui| {
                                        ui.label("NPC:");
                                        ui.text_edit_singleline(npc);
                                    });
                                }
                                ObjectiveType::Protect { target, duration } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Target:");
                                        ui.text_edit_singleline(target);
                                        ui.label("Duration (s):");
                                        ui.add(egui::DragValue::new(duration).suffix("s").speed(1.0).range(1.0..=99999.0));
                                    });
                                }
                                ObjectiveType::Explore { area, .. } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Area:");
                                        ui.text_edit_singleline(area);
                                    });
                                }
                                ObjectiveType::Custom { condition } => {
                                    ui.horizontal(|ui| {
                                        ui.label("Condition:");
                                        ui.text_edit_singleline(condition);
                                    });
                                }
                            }

                            ui.horizontal(|ui| {
                                ui.label("Progress:");
                                ui.add(egui::DragValue::new(&mut obj.current_progress).range(0..=obj.required_progress).speed(1.0));
                                ui.label("/");
                                ui.add(egui::DragValue::new(&mut obj.required_progress).range(1..=9999).speed(1.0));
                                ui.checkbox(&mut obj.required, "Required");
                            });
                        });
                    }
                });
            }

            if let Some(idx) = to_remove_obj {
                let obj_id = editor.quests[sel].objectives[idx].id;
                editor.quests[sel].objectives.remove(idx);
                editor.expanded_objectives.remove(&obj_id);
            }
            if let Some(idx) = move_up {
                if idx > 0 {
                    editor.quests[sel].objectives.swap(idx, idx - 1);
                }
            }
            if let Some(idx) = move_down {
                let len = editor.quests[sel].objectives.len();
                if idx + 1 < len {
                    editor.quests[sel].objectives.swap(idx, idx + 1);
                }
            }

            // Prerequisites
            ui.separator();
            ui.strong("Prerequisites");
            ui.add(egui::TextEdit::singleline(&mut editor.prereq_search).hint_text("Search quests...").desired_width(180.0));
            let search_lower = editor.prereq_search.to_lowercase();
            let all_quests: Vec<(QuestId, String)> = editor.quests.iter()
                .filter(|q| q.id != sel && (search_lower.is_empty() || q.name.to_lowercase().contains(&search_lower)))
                .map(|q| (q.id, q.name.clone()))
                .collect();
            let current_prereqs = editor.quests[sel].prereqs.clone();
            egui::ScrollArea::vertical()
                .id_salt("prereq_scroll")
                .max_height(100.0)
                .show(ui, |ui| {
                    for (qid, qname) in &all_quests {
                        let is_prereq = current_prereqs.contains(qid);
                        let mut checked = is_prereq;
                        if ui.checkbox(&mut checked, qname.as_str()).changed() {
                            if checked {
                                editor.quests[sel].prereqs.push(*qid);
                            } else {
                                editor.quests[sel].prereqs.retain(|&p| p != *qid);
                            }
                        }
                    }
                });

            // Rewards
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Rewards");
                let reward_labels = Reward::type_labels();
                egui::ComboBox::from_id_salt("new_reward_type")
                    .selected_text(reward_labels[editor.reward_type_sel])
                    .show_ui(ui, |ui| {
                        for (i, label) in reward_labels.iter().enumerate() {
                            if ui.selectable_label(i == editor.reward_type_sel, *label).clicked() {
                                editor.reward_type_sel = i;
                            }
                        }
                    });
                if ui.small_button("+ Add").clicked() {
                    let reward = Reward::default_for_index(editor.reward_type_sel);
                    editor.quests[sel].rewards.push(reward);
                }
            });

            let mut to_remove_reward: Option<usize> = None;
            for (ri, reward) in editor.quests[sel].rewards.iter_mut().enumerate() {
                ui.push_id(ri, |ui| {
                    ui.horizontal(|ui| {
                        let reward_color = Color32::from_rgb(255, 200, 80);
                        ui.label(RichText::new("*").color(reward_color));
                        match reward {
                            Reward::Experience(xp) => {
                                ui.label("XP:");
                                ui.add(egui::DragValue::new(xp).range(0..=999999).speed(10.0));
                            }
                            Reward::Gold(g) => {
                                ui.label("Gold:");
                                ui.add(egui::DragValue::new(g).range(0..=999999).speed(1.0));
                            }
                            Reward::Item { item_id, quantity } => {
                                ui.label("Item ID:");
                                ui.text_edit_singleline(item_id);
                                ui.label("x");
                                ui.add(egui::DragValue::new(quantity).range(1..=9999).speed(1.0));
                            }
                            Reward::Ability(name) => {
                                ui.label("Ability:");
                                ui.text_edit_singleline(name);
                            }
                            Reward::Reputation { faction, amount } => {
                                ui.label("Faction:");
                                ui.text_edit_singleline(faction);
                                ui.label("Rep:");
                                ui.add(egui::DragValue::new(amount).range(-9999..=9999).speed(1.0));
                            }
                            Reward::Unlock { feature } => {
                                ui.label("Unlock ID:");
                                ui.text_edit_singleline(feature);
                            }
                            Reward::Skill { skill_id, points } => {
                                ui.label("Skill:");
                                ui.text_edit_singleline(skill_id);
                                ui.add(egui::DragValue::new(points).range(1..=100).speed(1.0));
                            }
                        }
                        if ui.small_button("x").clicked() {
                            to_remove_reward = Some(ri);
                        }
                    });
                });
            }
            if let Some(idx) = to_remove_reward {
                editor.quests[sel].rewards.remove(idx);
            }

            // Flags on complete/fail
            ui.separator();
            ui.strong("Flag Triggers");
            ui.horizontal(|ui| {
                ui.label("On Complete:");
                let all_flag_names: Vec<String> = editor.flags.iter().map(|f| f.name.clone()).collect();
                for flag_name in &all_flag_names {
                    let quest = &mut editor.quests[sel];
                    let is_set = quest.flags_on_complete.contains(flag_name);
                    let mut checked = is_set;
                    if ui.checkbox(&mut checked, flag_name.as_str()).changed() {
                        if checked {
                            quest.flags_on_complete.push(flag_name.clone());
                        } else {
                            quest.flags_on_complete.retain(|f| f != flag_name);
                        }
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("On Fail:");
                let all_flag_names: Vec<String> = editor.flags.iter().map(|f| f.name.clone()).collect();
                for flag_name in &all_flag_names {
                    let quest = &mut editor.quests[sel];
                    let is_set = quest.flags_on_fail.contains(flag_name);
                    let mut checked = is_set;
                    if ui.checkbox(&mut checked, flag_name.as_str()).changed() {
                        if checked {
                            quest.flags_on_fail.push(flag_name.clone());
                        } else {
                            quest.flags_on_fail.retain(|f| f != flag_name);
                        }
                    }
                }
            });
        });
}

fn show_flag_inspector(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Flag Inspector").color(Color32::from_rgb(200, 180, 100)))
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut editor.new_flag_name)
                    .hint_text("New flag name...")
                    .desired_width(180.0));
                if ui.button("+ Add Flag").clicked() && !editor.new_flag_name.is_empty() {
                    let name = editor.new_flag_name.clone();
                    editor.flags.push(QuestFlag::new(&name));
                    editor.new_flag_name.clear();
                }
            });

            egui::ScrollArea::vertical()
                .id_salt("flag_scroll")
                .max_height(150.0)
                .show(ui, |ui| {
                    egui::Grid::new("flag_grid")
                        .num_columns(4)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Flag Name");
                            ui.strong("Value");
                            ui.strong("Set by Quests");
                            ui.strong("Checked by");
                            ui.end_row();

                            let mut to_remove_flag: Option<usize> = None;
                            for (fi, flag) in editor.flags.iter_mut().enumerate() {
                                ui.text_edit_singleline(&mut flag.name);
                                ui.checkbox(&mut flag.value, "");

                                let setters: Vec<String> = flag.set_on_complete.iter()
                                    .filter_map(|&id| editor.quests.iter().find(|q| q.id == id).map(|q| q.name.clone()))
                                    .collect();
                                ui.label(if setters.is_empty() { "-".to_string() } else { setters.join(", ") });

                                let checkers: Vec<String> = flag.checked_by.iter()
                                    .filter_map(|&id| editor.quests.iter().find(|q| q.id == id).map(|q| q.name.clone()))
                                    .collect();
                                ui.horizontal(|ui| {
                                    ui.label(if checkers.is_empty() { "-".to_string() } else { checkers.join(", ") });
                                    if ui.small_button("x").clicked() {
                                        to_remove_flag = Some(fi);
                                    }
                                });
                                ui.end_row();
                            }

                            if let Some(idx) = to_remove_flag {
                                editor.flags.remove(idx);
                            }
                        });
                });
        });
}

fn show_faction_editor(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Faction Editor").color(Color32::from_rgb(200, 150, 255)))
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut editor.new_faction_name)
                    .hint_text("New faction name...")
                    .desired_width(180.0));
                if ui.button("+ Add Faction").clicked() && !editor.new_faction_name.is_empty() {
                    let name = editor.new_faction_name.clone();
                    editor.factions.push(Faction::new(&name));
                    editor.new_faction_name.clear();
                }
            });

            let mut to_remove: Option<usize> = None;
            egui::ScrollArea::vertical()
                .id_salt("faction_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    for (fi, faction) in editor.factions.iter_mut().enumerate() {
                        ui.push_id(fi, |ui| {
                            egui::CollapsingHeader::new(
                                RichText::new(&faction.name).color(Color32::from_rgb(faction.color[0], faction.color[1], faction.color[2]))
                            )
                            .id_salt(egui::Id::new("faction_header").with(fi))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Name:");
                                    ui.text_edit_singleline(&mut faction.name);
                                    if ui.small_button("Delete").clicked() {
                                        to_remove = Some(fi);
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Current Rep:");
                                    ui.add(egui::DragValue::new(&mut faction.current_rep).range(-9999..=9999).speed(1.0));
                                    let standing = faction.standing_label().to_string();
                                    let sc = faction.standing_color();
                                    ui.label(RichText::new(standing).color(sc).strong());
                                });

                                // Reputation bar
                                let bar_width = 300.0;
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 20.0), egui::Sense::hover());
                                let painter = ui.painter();

                                painter.rect_filled(rect, 4.0, Color32::from_rgb(30, 30, 30));

                                let min_rep = -1000_i32;
                                let max_rep = 1000_i32;
                                let range = (max_rep - min_rep) as f32;

                                let segments = [
                                    (min_rep, faction.thresholds.0, Color32::from_rgb(180, 40, 40)),
                                    (faction.thresholds.0, faction.thresholds.1, Color32::from_rgb(200, 120, 40)),
                                    (faction.thresholds.1, faction.thresholds.2, Color32::from_rgb(120, 120, 120)),
                                    (faction.thresholds.2, faction.thresholds.3, Color32::from_rgb(60, 160, 60)),
                                    (faction.thresholds.3, max_rep, Color32::from_rgb(60, 100, 200)),
                                ];

                                for (seg_min, seg_max, color) in &segments {
                                    let x0 = rect.min.x + (*seg_min - min_rep) as f32 / range * bar_width;
                                    let x1 = rect.min.x + (*seg_max - min_rep) as f32 / range * bar_width;
                                    let seg_rect = Rect::from_min_max(
                                        Pos2::new(x0, rect.min.y + 2.0),
                                        Pos2::new(x1, rect.max.y - 2.0),
                                    );
                                    painter.rect_filled(seg_rect, 0.0, *color);
                                }

                                // Indicator
                                let rep_x = rect.min.x + (faction.current_rep - min_rep) as f32 / range * bar_width;
                                painter.line_segment(
                                    [Pos2::new(rep_x, rect.min.y), Pos2::new(rep_x, rect.max.y)],
                                    Stroke::new(2.0, Color32::WHITE),
                                );

                                ui.label(RichText::new("Description:").small());
                                ui.add(egui::TextEdit::multiline(&mut faction.description)
                                    .desired_rows(2)
                                    .desired_width(f32::INFINITY));

                                ui.horizontal(|ui| {
                                    ui.label("Thresholds: Hostile");
                                    ui.add(egui::DragValue::new(&mut faction.thresholds.0).range(-9999..=9999).speed(1.0));
                                    ui.label("/ Unfriendly");
                                    ui.add(egui::DragValue::new(&mut faction.thresholds.1).range(-9999..=9999).speed(1.0));
                                    ui.label("/ Friendly");
                                    ui.add(egui::DragValue::new(&mut faction.thresholds.2).range(-9999..=9999).speed(1.0));
                                    ui.label("/ Allied");
                                    ui.add(egui::DragValue::new(&mut faction.thresholds.3).range(-9999..=9999).speed(1.0));
                                });
                            });
                        });
                    }
                });

            if let Some(idx) = to_remove {
                editor.factions.remove(idx);
            }
        });
}

pub fn show_panel(ctx: &egui::Context, editor: &mut QuestEditor, open: &mut bool) {
    egui::Window::new("Quest & Narrative System")
        .open(open)
        .resizable(true)
        .default_size([1200.0, 700.0])
        .min_size([800.0, 400.0])
        .show(ctx, |ui| {
            show(ui, editor);
        });
}

// ---- Statistics Panel ----

pub fn show_statistics(ui: &mut egui::Ui, editor: &QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Quest Statistics").color(Color32::from_rgb(200, 200, 100)))
        .default_open(false)
        .show(ui, |ui| {
            let total = editor.quests.len();
            let completed = editor.quests.iter().filter(|q| q.state == QuestState::Completed).count();
            let active = editor.quests.iter().filter(|q| q.state == QuestState::Active).count();
            let failed = editor.quests.iter().filter(|q| q.state == QuestState::Failed).count();
            let not_started = editor.quests.iter().filter(|q| q.state == QuestState::NotStarted).count();
            let abandoned = editor.quests.iter().filter(|q| q.state == QuestState::Abandoned).count();

            egui::Grid::new("quest_stats_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Total Quests:");
                    ui.label(format!("{}", total));
                    ui.end_row();
                    ui.label(RichText::new("Completed:").color(QuestState::Completed.color()));
                    ui.label(format!("{} ({:.0}%)", completed, if total > 0 { completed as f32 / total as f32 * 100.0 } else { 0.0 }));
                    ui.end_row();
                    ui.label(RichText::new("Active:").color(QuestState::Active.color()));
                    ui.label(format!("{}", active));
                    ui.end_row();
                    ui.label(RichText::new("Not Started:").color(QuestState::NotStarted.color()));
                    ui.label(format!("{}", not_started));
                    ui.end_row();
                    ui.label(RichText::new("Failed:").color(QuestState::Failed.color()));
                    ui.label(format!("{}", failed));
                    ui.end_row();
                    ui.label(RichText::new("Abandoned:").color(QuestState::Abandoned.color()));
                    ui.label(format!("{}", abandoned));
                    ui.end_row();
                });

            ui.separator();

            // Category breakdown bar
            ui.label("By Category:");
            let cats = [
                QuestCategory::Main,
                QuestCategory::Side,
                QuestCategory::Daily,
                QuestCategory::Hidden,
                QuestCategory::Tutorial,
            ];
            let bar_width = ui.available_width().min(300.0);
            let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 20.0), egui::Sense::hover());
            let painter = ui.painter();
            let mut x = bar_rect.min.x;
            for cat in &cats {
                let count = editor.quests.iter().filter(|q| &q.category == cat).count();
                let w = if total > 0 { count as f32 / total as f32 * bar_width } else { 0.0 };
                let seg_rect = Rect::from_min_size(Pos2::new(x, bar_rect.min.y), Vec2::new(w, bar_rect.height()));
                painter.rect_filled(seg_rect, 0.0, cat.color());
                x += w;
            }
            painter.rect_stroke(bar_rect, 2.0, Stroke::new(1.0, Color32::from_rgb(80, 80, 80)), egui::StrokeKind::Inside);

            // Legend
            ui.horizontal_wrapped(|ui| {
                for cat in &cats {
                    let count = editor.quests.iter().filter(|q| &q.category == cat).count();
                    let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot_rect.center(), 4.0, cat.color());
                    ui.label(RichText::new(format!("{}: {}", cat.label(), count)).small().color(cat.color()));
                }
            });

            ui.separator();
            ui.label("Total Rewards Summary:");
            let total_xp: u32 = editor.quests.iter()
                .flat_map(|q| q.rewards.iter())
                .filter_map(|r| if let Reward::Experience(xp) = r { Some(*xp) } else { None })
                .sum();
            let total_gold: u32 = editor.quests.iter()
                .flat_map(|q| q.rewards.iter())
                .filter_map(|r| if let Reward::Gold(g) = r { Some(*g) } else { None })
                .sum();
            ui.label(format!("Total XP: {}  |  Total Gold: {}", total_xp, total_gold));
        });
}

// ---- Quest Import/Export helpers ----

pub fn export_quest_names(editor: &QuestEditor) -> String {
    editor.quests.iter()
        .map(|q| format!("[{}] {} ({})", q.id, q.name, q.category.label()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn find_orphaned_quests(editor: &QuestEditor) -> Vec<QuestId> {
    editor.quests.iter()
        .filter(|q| {
            !q.prereqs.is_empty() && q.prereqs.iter().any(|&prereq_id| {
                !editor.quests.iter().any(|oq| oq.id == prereq_id)
            })
        })
        .map(|q| q.id)
        .collect()
}

pub fn find_circular_dependencies(editor: &QuestEditor) -> Vec<(QuestId, QuestId)> {
    let mut cycles = Vec::new();
    for quest in &editor.quests {
        for &prereq_id in &quest.prereqs {
            if let Some(prereq) = editor.quests.iter().find(|q| q.id == prereq_id) {
                if prereq.prereqs.contains(&quest.id) {
                    cycles.push((quest.id, prereq_id));
                }
            }
        }
    }
    cycles
}

pub fn show_validation_panel(ui: &mut egui::Ui, editor: &QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Validation").color(Color32::from_rgb(255, 200, 80)))
        .default_open(false)
        .show(ui, |ui| {
            let orphans = find_orphaned_quests(editor);
            let cycles = find_circular_dependencies(editor);

            if orphans.is_empty() && cycles.is_empty() {
                ui.label(RichText::new("No issues found.").color(Color32::from_rgb(100, 220, 100)));
                return;
            }

            if !orphans.is_empty() {
                ui.label(RichText::new(format!("{} orphaned prerequisite(s):", orphans.len()))
                    .color(Color32::from_rgb(255, 150, 50)));
                for &id in &orphans {
                    if let Some(q) = editor.quests.iter().find(|q| q.id == id) {
                        ui.label(format!("  - {} (ID {}): missing prereqs", q.name, q.id));
                    }
                }
            }

            if !cycles.is_empty() {
                ui.label(RichText::new(format!("{} circular dependency(s):", cycles.len()))
                    .color(Color32::from_rgb(255, 80, 80)));
                for (a, b) in &cycles {
                    let na = editor.quests.iter().find(|q| q.id == *a).map(|q| q.name.as_str()).unwrap_or("?");
                    let nb = editor.quests.iter().find(|q| q.id == *b).map(|q| q.name.as_str()).unwrap_or("?");
                    ui.label(format!("  - {} <-> {}", na, nb));
                }
            }
        });
}

// ---- Reward summary helpers ----

pub fn total_rewards_for_quest(quest: &Quest) -> String {
    let mut parts = Vec::new();
    for r in &quest.rewards {
        parts.push(r.label());
    }
    parts.join(", ")
}

pub fn quest_is_completable(quest: &Quest, flags: &[QuestFlag]) -> bool {
    quest.state == QuestState::Active &&
        quest.objectives.iter()
            .filter(|o| o.required)
            .all(|o| o.current_progress >= o.required_progress)
}

pub fn count_required_objectives(quest: &Quest) -> usize {
    quest.objectives.iter().filter(|o| o.required).count()
}

pub fn count_completed_objectives(quest: &Quest) -> usize {
    quest.objectives.iter()
        .filter(|o| o.required && o.current_progress >= o.required_progress)
        .count()
}

// ---- Graph layout auto-arrange ----

pub fn auto_layout_graph(editor: &mut QuestEditor) {
    let cols = 5_usize;
    let col_w = 200.0_f32;
    let row_h = 140.0_f32;
    let pad_x = 50.0_f32;
    let pad_y = 50.0_f32;

    // Sort by prereq depth
    let mut depth: HashMap<QuestId, usize> = HashMap::new();
    for quest in &editor.quests {
        if quest.prereqs.is_empty() {
            depth.insert(quest.id, 0);
        }
    }
    // Simple BFS-style depth assignment
    for _ in 0..editor.quests.len() {
        for quest in &editor.quests {
            let max_dep = quest.prereqs.iter()
                .filter_map(|&pid| depth.get(&pid).copied())
                .max()
                .unwrap_or(0);
            let entry = depth.entry(quest.id).or_insert(0);
            if quest.prereqs.is_empty() {
                *entry = 0;
            } else {
                *entry = (*entry).max(max_dep + 1);
            }
        }
    }

    let mut col_counters: HashMap<usize, usize> = HashMap::new();
    for quest in editor.quests.iter_mut() {
        let d = depth.get(&quest.id).copied().unwrap_or(0);
        let row = *col_counters.entry(d).or_insert(0);
        quest.graph_pos = Vec2::new(
            pad_x + d as f32 * col_w,
            pad_y + row as f32 * row_h,
        );
        *col_counters.get_mut(&d).unwrap() += 1;
    }
}

// ---- Dialogue/narrative note ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NarrativeNote {
    pub quest_id: QuestId,
    pub note: String,
    pub author: String,
    pub timestamp: u64,
}

impl NarrativeNote {
    pub fn new(quest_id: QuestId, note: &str, author: &str) -> Self {
        NarrativeNote {
            quest_id,
            note: note.to_string(),
            author: author.to_string(),
            timestamp: 0,
        }
    }
}

// ---- Quest lore entry ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoreEntry {
    pub title: String,
    pub body: String,
    pub unlocked_by: Option<QuestId>,
    pub category: String,
}

impl LoreEntry {
    pub fn new(title: &str, body: &str) -> Self {
        LoreEntry {
            title: title.to_string(),
            body: body.to_string(),
            unlocked_by: None,
            category: "General".to_string(),
        }
    }
}

pub struct LoreEditor {
    pub entries: Vec<LoreEntry>,
    pub selected: Option<usize>,
    pub search: String,
    pub new_title: String,
}

impl LoreEditor {
    pub fn new() -> Self {
        let mut le = LoreEditor {
            entries: Vec::new(),
            selected: None,
            search: String::new(),
            new_title: String::new(),
        };
        le.entries.push(LoreEntry::new(
            "The Ancient Evil",
            "Long ago, the dark god Malachar was sealed away by the first heroes. Now the seal weakens...",
        ));
        le.entries.push(LoreEntry {
            title: "The Oracle's Prophecy".to_string(),
            body: "When five stars align, a champion shall rise from the ashes of the old world.".to_string(),
            unlocked_by: Some(0),
            category: "Prophecy".to_string(),
        });
        le
    }
}

pub fn show_lore_editor(ui: &mut egui::Ui, lore: &mut LoreEditor) {
    ui.horizontal(|ui| {
        ui.strong("Lore & Codex");
        ui.add(egui::TextEdit::singleline(&mut lore.new_title).hint_text("Entry title...").desired_width(160.0));
        if ui.button("+ Add Entry").clicked() && !lore.new_title.is_empty() {
            lore.entries.push(LoreEntry::new(&lore.new_title.clone(), ""));
            lore.new_title.clear();
            lore.selected = Some(lore.entries.len() - 1);
        }
    });
    ui.add(egui::TextEdit::singleline(&mut lore.search).hint_text("Search lore...").desired_width(f32::INFINITY));
    ui.separator();

    let search_lower = lore.search.to_lowercase();
    let indices: Vec<usize> = lore.entries.iter().enumerate()
        .filter(|(_, e)| search_lower.is_empty() || e.title.to_lowercase().contains(&search_lower))
        .map(|(i, _)| i)
        .collect();

    egui::SidePanel::left("lore_list_panel")
        .resizable(true)
        .default_width(180.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("lore_list_scroll").show(ui, |ui| {
                for &idx in &indices {
                    let is_sel = lore.selected == Some(idx);
                    let title = lore.entries[idx].title.clone();
                    if ui.selectable_label(is_sel, &title).clicked() {
                        lore.selected = Some(idx);
                    }
                }
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        if let Some(sel) = lore.selected {
            if sel < lore.entries.len() {
                let entry = &mut lore.entries[sel];
                ui.horizontal(|ui| {
                    ui.label("Title:");
                    ui.text_edit_singleline(&mut entry.title);
                });
                ui.horizontal(|ui| {
                    ui.label("Category:");
                    ui.text_edit_singleline(&mut entry.category);
                });
                ui.separator();
                ui.add(egui::TextEdit::multiline(&mut entry.body)
                    .desired_rows(10)
                    .desired_width(f32::INFINITY));
            }
        }
    });
}

// ---- Extended QuestEditor methods ----

impl QuestEditor {
    pub fn select_next(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() { return; }
        let cur = self.selected.unwrap_or(usize::MAX);
        let pos = indices.iter().position(|&i| i == cur).unwrap_or(usize::MAX);
        if pos == usize::MAX || pos + 1 >= indices.len() {
            self.selected = Some(indices[0]);
        } else {
            self.selected = Some(indices[pos + 1]);
        }
    }

    pub fn select_prev(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() { return; }
        let cur = self.selected.unwrap_or(usize::MAX);
        let pos = indices.iter().position(|&i| i == cur).unwrap_or(usize::MAX);
        if pos == usize::MAX || pos == 0 {
            self.selected = Some(*indices.last().unwrap());
        } else {
            self.selected = Some(indices[pos - 1]);
        }
    }

    pub fn duplicate_quest(&mut self, idx: usize) {
        if idx >= self.quests.len() { return; }
        let mut new_q = self.quests[idx].clone();
        new_q.id = self.next_id;
        new_q.name = format!("{} (copy)", new_q.name);
        new_q.graph_pos += Vec2::new(20.0, 20.0);
        self.next_id += 1;
        self.quests.push(new_q);
    }

    pub fn delete_quest(&mut self, idx: usize) {
        if idx >= self.quests.len() { return; }
        let id = self.quests[idx].id;
        self.quests.remove(idx);
        // Remove from all prereq lists
        for q in self.quests.iter_mut() {
            q.prereqs.retain(|&pid| pid != id);
        }
        if self.selected == Some(idx) {
            self.selected = None;
        }
    }

    pub fn get_quest_by_id(&self, id: QuestId) -> Option<&Quest> {
        self.quests.iter().find(|q| q.id == id)
    }

    pub fn get_quest_by_id_mut(&mut self, id: QuestId) -> Option<&mut Quest> {
        self.quests.iter_mut().find(|q| q.id == id)
    }

    pub fn set_state(&mut self, id: QuestId, state: QuestState) {
        if let Some(quest) = self.get_quest_by_id_mut(id) {
            quest.state = state;
        }
    }

    pub fn all_prereqs_met(&self, quest_id: QuestId) -> bool {
        if let Some(quest) = self.get_quest_by_id(quest_id) {
            quest.prereqs.iter().all(|&pid| {
                self.get_quest_by_id(pid)
                    .map(|pq| pq.state == QuestState::Completed)
                    .unwrap_or(false)
            })
        } else {
            false
        }
    }

    pub fn available_quests(&self) -> Vec<QuestId> {
        self.quests.iter()
            .filter(|q| q.state == QuestState::NotStarted && self.all_prereqs_met(q.id))
            .map(|q| q.id)
            .collect()
    }

    pub fn active_quests(&self) -> Vec<QuestId> {
        self.quests.iter()
            .filter(|q| q.state == QuestState::Active)
            .map(|q| q.id)
            .collect()
    }

    pub fn sort_by_priority(&mut self) {
        self.quests.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn sort_by_name(&mut self) {
        self.quests.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn sort_by_level(&mut self) {
        self.quests.sort_by(|a, b| a.recommended_level.cmp(&b.recommended_level));
    }

    pub fn flag_value(&self, name: &str) -> bool {
        self.flags.iter()
            .find(|f| f.name == name)
            .map(|f| f.value)
            .unwrap_or(false)
    }

    pub fn set_flag(&mut self, name: &str, value: bool) {
        if let Some(f) = self.flags.iter_mut().find(|f| f.name == name) {
            f.value = value;
        }
    }

    pub fn faction_standing(&self, faction_name: &str) -> Option<i32> {
        self.factions.iter()
            .find(|f| f.name == faction_name)
            .map(|f| f.current_rep)
    }

    pub fn modify_reputation(&mut self, faction_name: &str, amount: i32) {
        if let Some(f) = self.factions.iter_mut().find(|f| f.name == faction_name) {
            f.current_rep = (f.current_rep + amount).clamp(-9999, 9999);
        }
    }

    pub fn complete_quest(&mut self, id: QuestId) {
        let flags_on_complete: Vec<String> = self.get_quest_by_id(id)
            .map(|q| q.flags_on_complete.clone())
            .unwrap_or_default();
        let rep_rewards: Vec<(String, i32)> = self.get_quest_by_id(id)
            .map(|q| q.rewards.iter()
                .filter_map(|r| if let Reward::Reputation { faction, amount } = r { Some((faction.clone(), *amount)) } else { None })
                .collect())
            .unwrap_or_default();

        self.set_state(id, QuestState::Completed);
        for flag_name in flags_on_complete {
            self.set_flag(&flag_name, true);
        }
        for (faction, amount) in rep_rewards {
            self.modify_reputation(&faction, amount);
        }
    }

    pub fn fail_quest(&mut self, id: QuestId) {
        let flags_on_fail: Vec<String> = self.get_quest_by_id(id)
            .map(|q| q.flags_on_fail.clone())
            .unwrap_or_default();
        self.set_state(id, QuestState::Failed);
        for flag_name in flags_on_fail {
            self.set_flag(&flag_name, true);
        }
    }

    pub fn graph_node_count(&self) -> usize {
        self.quests.len()
    }

    pub fn graph_edge_count(&self) -> usize {
        self.quests.iter().map(|q| q.prereqs.len()).sum()
    }

    pub fn total_objective_count(&self) -> usize {
        self.quests.iter().map(|q| q.objectives.len()).sum()
    }
}

// ---- Full quest editor window with all subpanels ----

pub fn show_full_editor(ctx: &egui::Context, editor: &mut QuestEditor, lore: &mut LoreEditor, open: &mut bool) {
    egui::Window::new("Quest System — Full Editor")
        .open(open)
        .resizable(true)
        .default_size([1400.0, 800.0])
        .min_size([900.0, 500.0])
        .show(ctx, |ui| {
            egui::TopBottomPanel::top("quest_full_toolbar")
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(RichText::new("Quest & Narrative System").size(16.0).color(Color32::from_rgb(220, 200, 100)));
                        ui.separator();
                        if ui.button("Sort: Priority").clicked() { editor.sort_by_priority(); }
                        if ui.button("Sort: Name").clicked() { editor.sort_by_name(); }
                        if ui.button("Sort: Level").clicked() { editor.sort_by_level(); }
                        if ui.button("Auto-Layout").clicked() { auto_layout_graph(editor); }
                        ui.separator();
                        ui.label(RichText::new(format!(
                            "{} quests | {} flags | {} factions | {} edges",
                            editor.graph_node_count(),
                            editor.flags.len(),
                            editor.factions.len(),
                            editor.graph_edge_count(),
                        )).small().color(Color32::GRAY));
                    });
                });

            egui::TopBottomPanel::bottom("quest_full_status")
                .show_inside(ui, |ui| {
                    let avail = editor.available_quests();
                    let active = editor.active_quests();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!(
                            "Available: {} | Active: {} | Objectives: {}",
                            avail.len(),
                            active.len(),
                            editor.total_objective_count(),
                        )).small().color(Color32::from_rgb(150, 180, 150)));

                        ui.separator();
                        let orphans = find_orphaned_quests(editor);
                        if !orphans.is_empty() {
                            ui.label(RichText::new(format!("⚠ {} orphan(s)", orphans.len())).small().color(Color32::from_rgb(255, 150, 50)));
                        }
                        let cycles = find_circular_dependencies(editor);
                        if !cycles.is_empty() {
                            ui.label(RichText::new(format!("⚠ {} cycle(s)", cycles.len())).small().color(Color32::from_rgb(255, 80, 80)));
                        }
                    });
                });

            egui::SidePanel::left("quest_full_left")
                .resizable(true)
                .default_width(220.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("quest_full_left_scroll").show(ui, |ui| {
                        show_statistics(ui, editor);
                        ui.separator();
                        show_validation_panel(ui, editor);
                        ui.separator();
                        show_flag_inspector(ui, editor);
                        ui.separator();
                        show_faction_editor(ui, editor);
                    });
                });

            show(ui, editor);
        });
}

// ---- Serialization helpers ----

pub fn quests_to_json_pretty(editor: &QuestEditor) -> String {
    let data: Vec<serde_json::Value> = editor.quests.iter().map(|q| {
        serde_json::json!({
            "id": q.id,
            "name": q.name,
            "category": q.category.label(),
            "state": q.state.label(),
            "priority": q.priority,
            "recommended_level": q.recommended_level,
            "prereqs": q.prereqs,
            "objective_count": q.objectives.len(),
            "reward_count": q.rewards.len(),
        })
    }).collect();
    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string())
}

// ---- QuestId type aliases and helpers ----

pub fn quest_id_range(editor: &QuestEditor) -> (QuestId, QuestId) {
    let min = editor.quests.iter().map(|q| q.id).min().unwrap_or(0);
    let max = editor.quests.iter().map(|q| q.id).max().unwrap_or(0);
    (min, max)
}

pub fn quests_in_category<'a>(editor: &'a QuestEditor, category: &QuestCategory) -> Vec<&'a Quest> {
    editor.quests.iter().filter(|q| &q.category == category).collect()
}

pub fn quests_by_level_range(editor: &QuestEditor, min: u32, max: u32) -> Vec<&Quest> {
    editor.quests.iter().filter(|q| q.recommended_level >= min && q.recommended_level <= max).collect()
}

// ---- QuestGraph topology helpers ----

pub struct QuestGraph<'a> {
    pub quests: &'a [Quest],
}

impl<'a> QuestGraph<'a> {
    pub fn new(quests: &'a [Quest]) -> Self {
        QuestGraph { quests }
    }

    pub fn roots(&self) -> Vec<QuestId> {
        self.quests.iter()
            .filter(|q| q.prereqs.is_empty())
            .map(|q| q.id)
            .collect()
    }

    pub fn leaves(&self) -> Vec<QuestId> {
        let all_ids: HashSet<QuestId> = self.quests.iter().map(|q| q.id).collect();
        let prereq_ids: HashSet<QuestId> = self.quests.iter()
            .flat_map(|q| q.prereqs.iter().copied())
            .collect();
        all_ids.difference(&prereq_ids).copied().collect()
    }

    pub fn successors(&self, id: QuestId) -> Vec<QuestId> {
        self.quests.iter()
            .filter(|q| q.prereqs.contains(&id))
            .map(|q| q.id)
            .collect()
    }

    pub fn predecessors(&self, id: QuestId) -> Vec<QuestId> {
        self.quests.iter()
            .find(|q| q.id == id)
            .map(|q| q.prereqs.clone())
            .unwrap_or_default()
    }

    pub fn depth_of(&self, id: QuestId) -> usize {
        let preds = self.predecessors(id);
        if preds.is_empty() {
            0
        } else {
            preds.iter().map(|&pid| self.depth_of(pid) + 1).max().unwrap_or(0)
        }
    }

    pub fn total_edges(&self) -> usize {
        self.quests.iter().map(|q| q.prereqs.len()).sum()
    }

    pub fn is_dag(&self) -> bool {
        find_circular_dependencies_graph(self).is_empty()
    }
}

fn find_circular_dependencies_graph(graph: &QuestGraph) -> Vec<(QuestId, QuestId)> {
    let mut cycles = Vec::new();
    for quest in graph.quests {
        for &prereq_id in &quest.prereqs {
            if let Some(prereq) = graph.quests.iter().find(|q| q.id == prereq_id) {
                if prereq.prereqs.contains(&quest.id) {
                    cycles.push((quest.id, prereq_id));
                }
            }
        }
    }
    cycles
}

// ---- Demo: Extra quests for scale ----

pub fn add_bulk_demo_quests(editor: &mut QuestEditor, count: usize) {
    let categories = QuestCategory::all();
    let states = QuestState::all();
    for i in 0..count {
        let mut q = Quest::new(editor.next_id);
        q.name = format!("Generated Quest {}", editor.next_id);
        q.category = categories[i % categories.len()].clone();
        q.state = states[i % states.len()].clone();
        q.priority = (i % 100) as u32;
        q.recommended_level = ((i % 20) + 1) as u32;
        q.graph_pos = Vec2::new(
            (editor.next_id as f32 % 8.0) * 190.0 + 50.0,
            (editor.next_id as f32 / 8.0).floor() * 140.0 + 50.0,
        );
        if editor.next_id > 0 && i % 3 == 0 {
            q.prereqs = vec![editor.next_id - 1];
        }
        let obj_count = (i % 3) + 1;
        for j in 0..obj_count {
            let mut obj = QuestObjective::new(j);
            obj.description = format!("Objective {}", j + 1);
            obj.required_progress = (j as u32 + 1) * 5;
            obj.current_progress = (j as u32) * 2;
            q.objectives.push(obj);
        }
        q.objective_next_id = obj_count;
        q.rewards.push(Reward::Experience((i as u32 + 1) * 100));
        editor.quests.push(q);
        editor.next_id += 1;
    }
}

// ---- Additional UI helpers ----

pub fn draw_state_badge(ui: &mut egui::Ui, state: &QuestState) {
    let color = state.color();
    let label = state.label();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(70.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(color.r() / 3, color.g() / 3, color.b() / 3, 220));
    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, color), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(10.0), color);
}

pub fn draw_category_badge(ui: &mut egui::Ui, category: &QuestCategory) {
    let color = category.color();
    let label = category.label();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(color.r() / 3, color.g() / 3, color.b() / 3, 220));
    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, color), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(10.0), color);
}

pub fn draw_progress_bar_inline(ui: &mut egui::Ui, progress: f32, width: f32, height: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(30, 30, 35));
    let fill_w = rect.width() * progress.clamp(0.0, 1.0);
    let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
    ui.painter().rect_filled(fill_rect, 3.0, color);
    ui.painter().rect_stroke(rect, 3.0, Stroke::new(0.5, Color32::from_rgb(60, 60, 70)), egui::StrokeKind::Inside);
}

pub fn objective_type_icon(obj_type: &ObjectiveType) -> &'static str {
    match obj_type {
        ObjectiveType::Kill { .. } => "⚔",
        ObjectiveType::Collect { .. } => "📦",
        ObjectiveType::Reach { .. } => "📍",
        ObjectiveType::Talk { .. } => "💬",
        ObjectiveType::Craft { .. } => "🔨",
        ObjectiveType::Survive { .. } => "❤",
        ObjectiveType::Escort { .. } => "👣",
        ObjectiveType::Protect { .. } => "🛡",
        ObjectiveType::Explore { .. } => "🗺",
        ObjectiveType::Custom { .. } => "⚙",
    }
}

pub fn reward_icon(reward: &Reward) -> &'static str {
    match reward {
        Reward::Experience(_) => "★",
        Reward::Gold(_) => "◈",
        Reward::Item { .. } => "▣",
        Reward::Ability(_) => "◆",
        Reward::Reputation { .. } => "◉",
        Reward::Unlock { .. } => "▶",
        Reward::Skill { .. } => "⬆",
    }
}

// ---- QuestEditor keyboard navigation ----

pub fn handle_keyboard(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    ui.input(|i| {
        if i.key_pressed(egui::Key::ArrowDown) {
            editor.select_next();
        }
        if i.key_pressed(egui::Key::ArrowUp) {
            editor.select_prev();
        }
        if i.key_pressed(egui::Key::Delete) {
            if let Some(sel) = editor.selected {
                editor.delete_quest(sel);
            }
        }
        if i.key_pressed(egui::Key::D) && i.modifiers.ctrl {
            if let Some(sel) = editor.selected {
                editor.duplicate_quest(sel);
            }
        }
    });
}

// ---- Export as a flat list for game runtime ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeQuest {
    pub id: QuestId,
    pub name: String,
    pub state: String,
    pub category: String,
    pub progress: f32,
    pub completed_objectives: usize,
    pub total_required_objectives: usize,
}

pub fn to_runtime_quests(editor: &QuestEditor) -> Vec<RuntimeQuest> {
    editor.quests.iter().map(|q| RuntimeQuest {
        id: q.id,
        name: q.name.clone(),
        state: q.state.label().to_string(),
        category: q.category.label().to_string(),
        progress: q.progress(),
        completed_objectives: count_completed_objectives(q),
        total_required_objectives: count_required_objectives(q),
    }).collect()
}

// ---- Faction reputation display helpers ----

pub fn draw_faction_row(ui: &mut egui::Ui, faction: &Faction) {
    ui.horizontal(|ui| {
        let fc = Color32::from_rgb(faction.color[0], faction.color[1], faction.color[2]);
        let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(dot_rect.center(), 5.0, fc);
        ui.label(RichText::new(&faction.name).color(fc));
        ui.label(RichText::new(format!("({})", faction.current_rep)).small().color(Color32::GRAY));
        let standing = faction.standing_label();
        let sc = faction.standing_color();
        ui.label(RichText::new(standing).color(sc).strong());
    });
}

pub fn draw_all_factions_mini(ui: &mut egui::Ui, editor: &QuestEditor) {
    egui::CollapsingHeader::new("Faction Standing Summary")
        .default_open(false)
        .show(ui, |ui| {
            for faction in &editor.factions {
                draw_faction_row(ui, faction);
            }
        });
}

// ---- Quest Journal render ----

pub fn show_journal_view(ui: &mut egui::Ui, editor: &QuestEditor) {
    ui.heading(RichText::new("Quest Journal").size(16.0).color(Color32::from_rgb(220, 200, 100)));
    ui.separator();

    let active: Vec<&Quest> = editor.quests.iter().filter(|q| q.state == QuestState::Active).collect();
    let completed: Vec<&Quest> = editor.quests.iter().filter(|q| q.state == QuestState::Completed).collect();
    let available = editor.available_quests();

    egui::CollapsingHeader::new(RichText::new(format!("Active Quests ({})", active.len())).color(Color32::from_rgb(100, 200, 255)))
        .default_open(true)
        .show(ui, |ui| {
            for quest in &active {
                ui.push_id(quest.id, |ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(28, 35, 50))
                        .stroke(Stroke::new(1.0, quest.category.color()))
                        .inner_margin(6.0)
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                draw_category_badge(ui, &quest.category);
                                ui.label(RichText::new(&quest.name).strong().color(Color32::WHITE));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(format!("Lv.{}", quest.recommended_level)).small().color(Color32::GRAY));
                                });
                            });
                            if !quest.description.is_empty() {
                                ui.label(RichText::new(&quest.description).small().color(Color32::GRAY));
                            }
                            ui.separator();
                            let progress = quest.progress();
                            draw_progress_bar_inline(ui, progress, ui.available_width(), 8.0, Color32::from_rgb(80, 150, 220));
                            ui.label(RichText::new(format!(
                                "{}/{} objectives — {:.0}%",
                                count_completed_objectives(quest),
                                count_required_objectives(quest),
                                progress * 100.0,
                            )).small().color(Color32::GRAY));

                            for obj in &quest.objectives {
                                if obj.hidden { continue; }
                                let done = obj.current_progress >= obj.required_progress;
                                let obj_color = if done { Color32::from_rgb(100, 220, 100) } else { Color32::LIGHT_GRAY };
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(if done { "v" } else { "o" }).color(obj_color));
                                    ui.label(RichText::new(&obj.description).color(obj_color).small());
                                    if obj.required_progress > 1 {
                                        ui.label(RichText::new(format!("({}/{})", obj.current_progress, obj.required_progress)).small().color(Color32::GRAY));
                                    }
                                });
                            }

                            if !quest.rewards.is_empty() {
                                ui.separator();
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Rewards:").small().color(Color32::GRAY));
                                    for r in &quest.rewards {
                                        ui.label(RichText::new(format!("{} {}", reward_icon(r), r.label())).small().color(Color32::from_rgb(220, 190, 80)));
                                    }
                                });
                            }
                        });
                    ui.add_space(4.0);
                });
            }
            if active.is_empty() {
                ui.label(RichText::new("No active quests.").color(Color32::GRAY));
            }
        });

    ui.separator();

    egui::CollapsingHeader::new(RichText::new(format!("Available to Start ({})", available.len())).color(Color32::from_rgb(180, 180, 180)))
        .default_open(false)
        .show(ui, |ui| {
            for &qid in &available {
                if let Some(quest) = editor.get_quest_by_id(qid) {
                    ui.horizontal(|ui| {
                        draw_category_badge(ui, &quest.category);
                        ui.label(RichText::new(&quest.name).color(Color32::LIGHT_GRAY));
                        ui.label(RichText::new(format!("Lv.{}", quest.recommended_level)).small().color(Color32::GRAY));
                    });
                }
            }
            if available.is_empty() {
                ui.label(RichText::new("No quests available.").color(Color32::GRAY));
            }
        });

    ui.separator();

    egui::CollapsingHeader::new(RichText::new(format!("Completed ({})", completed.len())).color(Color32::from_rgb(100, 220, 100)))
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("journal_completed_scroll").max_height(200.0).show(ui, |ui| {
                for quest in &completed {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("v").color(Color32::from_rgb(100, 220, 100)));
                        ui.label(RichText::new(&quest.name).color(Color32::from_rgb(150, 200, 150)));
                        ui.label(RichText::new(quest.category.label()).small().color(quest.category.color()));
                    });
                }
            });
        });
}

// ---- Prerequisite chain visualizer ----

pub fn show_prereq_chain(ui: &mut egui::Ui, editor: &QuestEditor, quest_id: QuestId, depth: usize) {
    if depth > 8 { return; }
    let indent = depth as f32 * 20.0;
    if let Some(quest) = editor.get_quest_by_id(quest_id) {
        ui.horizontal(|ui| {
            ui.add_space(indent);
            draw_state_badge(ui, &quest.state);
            ui.label(RichText::new(&quest.name).color(Color32::LIGHT_GRAY));
        });
        let prereqs = quest.prereqs.clone();
        for pid in prereqs {
            show_prereq_chain(ui, editor, pid, depth + 1);
        }
    }
}

// ---- Multi-quest batch editor ----

pub fn show_batch_editor(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Batch Operations").color(Color32::from_rgb(255, 180, 80)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Set all filtered quests state:");
                egui::ComboBox::from_id_salt("batch_state_combo")
                    .selected_text("State...")
                    .show_ui(ui, |ui| {
                        for state in QuestState::all() {
                            if ui.button(state.label()).clicked() {
                                let indices = editor.filtered_indices();
                                for &idx in &indices {
                                    editor.quests[idx].state = state.clone();
                                }
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Set all filtered category:");
                egui::ComboBox::from_id_salt("batch_cat_combo")
                    .selected_text("Category...")
                    .show_ui(ui, |ui| {
                        for cat in QuestCategory::all() {
                            if ui.button(cat.label()).clicked() {
                                let indices = editor.filtered_indices();
                                for &idx in &indices {
                                    editor.quests[idx].category = cat.clone();
                                }
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Reset all objectives progress:");
                if ui.button("Reset").clicked() {
                    for quest in editor.quests.iter_mut() {
                        for obj in quest.objectives.iter_mut() {
                            obj.current_progress = 0;
                        }
                    }
                }
            });

            if ui.button("Add demo bulk quests (10)").clicked() {
                add_bulk_demo_quests(editor, 10);
            }
        });
}

// ---- Reward breakdown panel ----

pub fn show_reward_breakdown(ui: &mut egui::Ui, editor: &QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Global Reward Breakdown").color(Color32::from_rgb(220, 190, 80)))
        .default_open(false)
        .show(ui, |ui| {
            let mut total_xp = 0u32;
            let mut total_gold = 0u32;
            let mut ability_count = 0usize;
            let mut item_count = 0usize;
            let mut unlock_count = 0usize;
            let mut rep_total = 0i32;

            for quest in &editor.quests {
                for r in &quest.rewards {
                    match r {
                        Reward::Experience(xp) => total_xp += xp,
                        Reward::Gold(g) => total_gold += g,
                        Reward::Ability(_) => ability_count += 1,
                        Reward::Item { quantity, .. } => item_count += *quantity as usize,
                        Reward::Unlock { .. } => unlock_count += 1,
                        Reward::Reputation { amount, .. } => rep_total += amount,
                        Reward::Skill { .. } => {}

                    }
                }
            }

            egui::Grid::new("reward_breakdown_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("Total XP:").color(Color32::from_rgb(220, 220, 80)));
                    ui.label(format!("{}", total_xp));
                    ui.end_row();
                    ui.label(RichText::new("Total Gold:").color(Color32::from_rgb(220, 180, 50)));
                    ui.label(format!("{}", total_gold));
                    ui.end_row();
                    ui.label("Abilities:");
                    ui.label(format!("{}", ability_count));
                    ui.end_row();
                    ui.label("Items:");
                    ui.label(format!("{}", item_count));
                    ui.end_row();
                    ui.label("Unlocks:");
                    ui.label(format!("{}", unlock_count));
                    ui.end_row();
                    ui.label("Net Reputation:");
                    let rc = if rep_total > 0 { Color32::from_rgb(100, 220, 100) } else if rep_total < 0 { Color32::from_rgb(220, 80, 80) } else { Color32::GRAY };
                    ui.label(RichText::new(format!("{:+}", rep_total)).color(rc).strong());
                    ui.end_row();
                });
        });
}

// ---- Narrative consistency warnings ----

pub fn check_objectives_completable(editor: &QuestEditor) -> Vec<String> {
    let mut warnings = Vec::new();
    for quest in &editor.quests {
        if quest.objectives.is_empty() && quest.state != QuestState::Completed {
            warnings.push(format!("'{}' has no objectives", quest.name));
        }
        let required_objs: Vec<&QuestObjective> = quest.objectives.iter().filter(|o| o.required).collect();
        if required_objs.is_empty() && !quest.objectives.is_empty() {
            warnings.push(format!("'{}': all objectives are optional", quest.name));
        }
        for obj in &quest.objectives {
            if obj.required_progress == 0 {
                warnings.push(format!("'{}' > '{}': required_progress=0", quest.name, obj.description));
            }
        }
    }
    warnings
}

pub fn show_narrative_warnings(ui: &mut egui::Ui, editor: &QuestEditor) {
    let warnings = check_objectives_completable(editor);
    if warnings.is_empty() { return; }

    egui::CollapsingHeader::new(RichText::new(format!("Warnings ({})", warnings.len())).color(Color32::from_rgb(255, 200, 50)))
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("warnings_scroll").max_height(120.0).show(ui, |ui| {
                for w in &warnings {
                    ui.label(RichText::new(format!("! {}", w)).small().color(Color32::from_rgb(255, 200, 50)));
                }
            });
        });
}

// ---- Faction event log ----

#[derive(Clone, Debug)]
pub struct FactionEvent {
    pub faction_name: String,
    pub change: i32,
    pub reason: String,
    pub quest_id: Option<QuestId>,
}

pub struct FactionLog {
    pub events: Vec<FactionEvent>,
    pub max_events: usize,
}

impl FactionLog {
    pub fn new() -> Self {
        FactionLog { events: Vec::new(), max_events: 100 }
    }

    pub fn record(&mut self, faction: &str, change: i32, reason: &str, quest_id: Option<QuestId>) {
        self.events.push(FactionEvent {
            faction_name: faction.to_string(),
            change,
            reason: reason.to_string(),
            quest_id,
        });
        if self.events.len() > self.max_events {
            self.events.remove(0);
        }
    }

    pub fn net_change_for(&self, faction: &str) -> i32 {
        self.events.iter()
            .filter(|e| e.faction_name == faction)
            .map(|e| e.change)
            .sum()
    }
}

pub fn show_faction_log(ui: &mut egui::Ui, log: &FactionLog) {
    ui.strong("Faction Event Log");
    ui.separator();
    egui::ScrollArea::vertical().id_salt("faction_log_scroll").max_height(150.0).show(ui, |ui| {
        for event in log.events.iter().rev() {
            let change_color = if event.change > 0 { Color32::from_rgb(100, 220, 100) } else { Color32::from_rgb(220, 80, 80) };
            ui.horizontal(|ui| {
                ui.label(RichText::new(&event.faction_name).color(Color32::from_rgb(180, 150, 255)));
                ui.label(RichText::new(format!("{:+}", event.change)).color(change_color).strong());
                ui.label(RichText::new(&event.reason).small().color(Color32::GRAY));
            });
        }
        if log.events.is_empty() {
            ui.label(RichText::new("No events recorded.").color(Color32::GRAY));
        }
    });
}

// ---- Timeline marker types ----

#[derive(Clone, Debug, PartialEq)]
pub enum MarkerType {
    Start,
    Objective,
    Complete,
    Fail,
    Custom,
}

impl MarkerType {
    pub fn color(&self) -> Color32 {
        match self {
            MarkerType::Start => Color32::from_rgb(100, 200, 255),
            MarkerType::Objective => Color32::from_rgb(220, 220, 80),
            MarkerType::Complete => Color32::from_rgb(80, 220, 80),
            MarkerType::Fail => Color32::from_rgb(220, 80, 80),
            MarkerType::Custom => Color32::from_rgb(200, 150, 255),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TimelineMarker {
    pub quest_id: QuestId,
    pub label: String,
    pub time_point: f32,
    pub marker_type: MarkerType,
}

pub fn draw_timeline_marker(painter: &Painter, pos: Pos2, marker: &TimelineMarker) {
    let color = marker.marker_type.color();
    let size = 8.0;
    match marker.marker_type {
        MarkerType::Start => {
            let tri = vec![
                Pos2::new(pos.x, pos.y - size),
                Pos2::new(pos.x + size, pos.y + size),
                Pos2::new(pos.x - size, pos.y + size),
            ];
            painter.add(Shape::convex_polygon(tri, color, Stroke::NONE));
        }
        MarkerType::Complete => {
            painter.circle_filled(pos, size, color);
        }
        MarkerType::Fail => {
            painter.line_segment([Pos2::new(pos.x - size, pos.y - size), Pos2::new(pos.x + size, pos.y + size)], Stroke::new(2.0, color));
            painter.line_segment([Pos2::new(pos.x + size, pos.y - size), Pos2::new(pos.x - size, pos.y + size)], Stroke::new(2.0, color));
        }
        _ => {
            painter.rect_filled(Rect::from_center_size(pos, Vec2::new(size, size)), 2.0, color);
        }
    }
    painter.text(pos + Vec2::new(10.0, 0.0), egui::Align2::LEFT_CENTER, &marker.label, FontId::proportional(9.0), color);
}

// ---- Graph debug overlay ----

pub fn show_graph_debug_overlay(ui: &mut egui::Ui, editor: &QuestEditor) {
    let node_count = editor.quests.len();
    let edge_count: usize = editor.quests.iter().map(|q| q.prereqs.len()).sum();
    let graph = QuestGraph::new(&editor.quests);
    let roots = graph.roots();
    let leaves = graph.leaves();
    let is_dag = graph.is_dag();

    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 200))
        .inner_margin(6.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.label(RichText::new(format!("Nodes: {} | Edges: {}", node_count, edge_count)).small().color(Color32::WHITE));
            ui.label(RichText::new(format!("Roots: {} | Leaves: {}", roots.len(), leaves.len())).small().color(Color32::GRAY));
            ui.label(RichText::new(format!("DAG: {}", if is_dag { "Yes" } else { "No - cycles detected" }))
                .small()
                .color(if is_dag { Color32::from_rgb(100, 220, 100) } else { Color32::from_rgb(255, 80, 80) }));
        });
}

// ---- Objective type description ----

pub fn default_description_for_type(obj_type: &ObjectiveType) -> String {
    match obj_type {
        ObjectiveType::Kill { target, count, .. } => format!("Defeat {} {}{}", count, target, if *count > 1 { "s" } else { "" }),
        ObjectiveType::Collect { item, count, .. } => format!("Collect {} {}{}", count, item, if *count > 1 { "s" } else { "" }),
        ObjectiveType::Reach { location } => format!("Travel to {}", location),
        ObjectiveType::Talk { character, .. } => format!("Speak with {}", character),
        ObjectiveType::Craft { item, count, .. } => format!("Craft {} {}{}", count, item, if *count > 1 { "s" } else { "" }),
        ObjectiveType::Survive { duration, .. } => format!("Survive for {:.0} seconds", duration),
        ObjectiveType::Escort { npc, .. } => format!("Escort {} to safety", npc),
        ObjectiveType::Protect { target, duration } => format!("Protect {} for {:.0}s", target, duration),
        ObjectiveType::Explore { area, .. } => format!("Explore {}", area),
        ObjectiveType::Custom { condition } => condition.clone(),
    }
}

// ---- Filter bar ----

pub fn show_filter_bar(ui: &mut egui::Ui, editor: &mut QuestEditor) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let old_state = editor.filter_state.clone();
        let state_label = editor.filter_state.as_ref().map_or("All States", |s| s.label());
        egui::ComboBox::from_id_salt("filter_state_bar2")
            .selected_text(state_label)
            .width(90.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(editor.filter_state.is_none(), "All States").clicked() {
                    editor.filter_state = None;
                }
                for s in QuestState::all() {
                    if ui.selectable_label(editor.filter_state.as_ref() == Some(s), s.label()).clicked() {
                        editor.filter_state = Some(s.clone());
                    }
                }
            });
        if old_state != editor.filter_state { changed = true; }

        let old_cat = editor.filter_category.clone();
        let cat_label = editor.filter_category.as_ref().map_or("All Categories", |c| c.label());
        egui::ComboBox::from_id_salt("filter_cat_bar2")
            .selected_text(cat_label)
            .width(100.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(editor.filter_category.is_none(), "All Categories").clicked() {
                    editor.filter_category = None;
                }
                for c in QuestCategory::all() {
                    if ui.selectable_label(editor.filter_category.as_ref() == Some(c), c.label()).clicked() {
                        editor.filter_category = Some(c.clone());
                    }
                }
            });
        if old_cat != editor.filter_category { changed = true; }

        let old_search = editor.search.clone();
        ui.add(egui::TextEdit::singleline(&mut editor.search).hint_text("Search...").desired_width(150.0));
        if old_search != editor.search { changed = true; }

        if editor.filter_state.is_some() || editor.filter_category.is_some() || !editor.search.is_empty() {
            if ui.small_button("Clear").clicked() {
                editor.filter_state = None;
                editor.filter_category = None;
                editor.search.clear();
                changed = true;
            }
        }
        let count = editor.filtered_indices().len();
        ui.label(RichText::new(format!("{} matching", count)).small().color(Color32::GRAY));
    });
    changed
}

// ---- Hotkey reference ----

pub fn show_hotkey_reference(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Hotkeys").color(Color32::GRAY).small())
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("hotkey_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for (key, action) in &[
                        ("Up/Down", "Navigate quest list"),
                        ("Del", "Delete selected quest"),
                        ("Ctrl+D", "Duplicate quest"),
                        ("Scroll", "Zoom graph"),
                        ("RMB Drag", "Pan graph"),
                        ("LMB", "Select / move node"),
                    ] {
                        ui.label(RichText::new(*key).monospace().color(Color32::from_rgb(200, 200, 100)));
                        ui.label(RichText::new(*action).small().color(Color32::GRAY));
                        ui.end_row();
                    }
                });
        });
}

// ---- Quest difficulty estimator ----

pub fn estimate_difficulty(quest: &Quest) -> &'static str {
    let objective_count = quest.objectives.len();
    let has_time_limit = quest.time_limit.is_some();
    let hard_types = quest.objectives.iter().filter(|o| {
        matches!(&o.objective_type,
            ObjectiveType::Survive { .. } | ObjectiveType::Protect { .. } | ObjectiveType::Escort { .. })
    }).count();

    let score = objective_count * 10
        + if has_time_limit { 20 } else { 0 }
        + hard_types * 15
        + quest.recommended_level as usize * 2;

    if score < 20 { "Trivial" }
    else if score < 40 { "Easy" }
    else if score < 60 { "Normal" }
    else if score < 80 { "Hard" }
    else { "Extreme" }
}

pub fn difficulty_color(difficulty: &str) -> Color32 {
    match difficulty {
        "Trivial" => Color32::from_rgb(150, 150, 150),
        "Easy" => Color32::from_rgb(80, 200, 80),
        "Normal" => Color32::from_rgb(80, 150, 255),
        "Hard" => Color32::from_rgb(255, 150, 50),
        "Extreme" => Color32::from_rgb(255, 50, 50),
        _ => Color32::GRAY,
    }
}

// ---- Quest search and replace ----

pub fn replace_in_quest_names(editor: &mut QuestEditor, from: &str, to: &str) -> usize {
    let mut count = 0;
    for quest in editor.quests.iter_mut() {
        if quest.name.contains(from) {
            quest.name = quest.name.replace(from, to);
            count += 1;
        }
    }
    count
}

pub fn replace_in_descriptions(editor: &mut QuestEditor, from: &str, to: &str) -> usize {
    let mut count = 0;
    for quest in editor.quests.iter_mut() {
        if quest.description.contains(from) {
            quest.description = quest.description.replace(from, to);
            count += 1;
        }
    }
    count
}

pub fn replace_in_objective_descriptions(editor: &mut QuestEditor, from: &str, to: &str) -> usize {
    let mut count = 0;
    for quest in editor.quests.iter_mut() {
        for obj in quest.objectives.iter_mut() {
            if obj.description.contains(from) {
                obj.description = obj.description.replace(from, to);
                count += 1;
            }
        }
    }
    count
}

// ---- Quest search/replace panel ----

pub struct SearchReplaceState {
    pub from: String,
    pub to: String,
    pub in_names: bool,
    pub in_descriptions: bool,
    pub in_objectives: bool,
    pub last_count: Option<usize>,
}

impl Default for SearchReplaceState {
    fn default() -> Self {
        SearchReplaceState {
            from: String::new(),
            to: String::new(),
            in_names: true,
            in_descriptions: true,
            in_objectives: false,
            last_count: None,
        }
    }
}

pub fn show_search_replace(ui: &mut egui::Ui, editor: &mut QuestEditor, state: &mut SearchReplaceState) {
    egui::CollapsingHeader::new(RichText::new("Search & Replace").color(Color32::from_rgb(200, 180, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Find:");
                ui.add(egui::TextEdit::singleline(&mut state.from).desired_width(150.0));
            });
            ui.horizontal(|ui| {
                ui.label("Replace:");
                ui.add(egui::TextEdit::singleline(&mut state.to).desired_width(150.0));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.in_names, "Names");
                ui.checkbox(&mut state.in_descriptions, "Descriptions");
                ui.checkbox(&mut state.in_objectives, "Objectives");
            });
            ui.horizontal(|ui| {
                if ui.button("Replace All").clicked() && !state.from.is_empty() {
                    let mut total = 0;
                    if state.in_names { total += replace_in_quest_names(editor, &state.from.clone(), &state.to.clone()); }
                    if state.in_descriptions { total += replace_in_descriptions(editor, &state.from.clone(), &state.to.clone()); }
                    if state.in_objectives { total += replace_in_objective_descriptions(editor, &state.from.clone(), &state.to.clone()); }
                    state.last_count = Some(total);
                }
                if let Some(count) = state.last_count {
                    ui.label(RichText::new(format!("{} replacement(s) made", count)).small().color(Color32::from_rgb(100, 220, 100)));
                }
            });
        });
}

// ---- Quest level range checker ----

pub fn quests_above_level(editor: &QuestEditor, level: u32) -> Vec<&Quest> {
    editor.quests.iter().filter(|q| q.recommended_level > level).collect()
}

pub fn quests_below_level(editor: &QuestEditor, level: u32) -> Vec<&Quest> {
    editor.quests.iter().filter(|q| q.recommended_level < level).collect()
}

pub fn show_level_distribution(ui: &mut egui::Ui, editor: &QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Level Distribution").color(Color32::from_rgb(180, 220, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let max_level = editor.quests.iter().map(|q| q.recommended_level).max().unwrap_or(10).max(1);
            let bar_max_w = 200.0_f32;

            for level in 1..=max_level.min(20) {
                let count = editor.quests.iter().filter(|q| q.recommended_level == level).count();
                if count == 0 { continue; }
                let bar_w = (count as f32 / editor.quests.len().max(1) as f32) * bar_max_w;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Lv.{:2}", level)).monospace().small().color(Color32::GRAY));
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_max_w, 14.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(25, 25, 30));
                    let fill = Rect::from_min_size(rect.min, Vec2::new(bar_w, rect.height()));
                    ui.painter().rect_filled(fill, 2.0, Color32::from_rgb(80, 140, 220));
                    ui.label(RichText::new(format!("{}", count)).small().color(Color32::GRAY));
                });
            }
        });
}

// ---- Questline builder (linear chains) ----

#[derive(Clone, Debug)]
pub struct Questline {
    pub name: String,
    pub quest_ids: Vec<QuestId>,
    pub description: String,
}

impl Questline {
    pub fn new(name: &str) -> Self {
        Questline {
            name: name.to_string(),
            quest_ids: Vec::new(),
            description: String::new(),
        }
    }

    pub fn is_complete(&self, editor: &QuestEditor) -> bool {
        self.quest_ids.iter().all(|&id| {
            editor.get_quest_by_id(id)
                .map(|q| q.state == QuestState::Completed)
                .unwrap_or(false)
        })
    }

    pub fn progress(&self, editor: &QuestEditor) -> f32 {
        if self.quest_ids.is_empty() { return 0.0; }
        let completed = self.quest_ids.iter().filter(|&&id| {
            editor.get_quest_by_id(id).map(|q| q.state == QuestState::Completed).unwrap_or(false)
        }).count();
        completed as f32 / self.quest_ids.len() as f32
    }
}

pub struct QuestlineEditor {
    pub questlines: Vec<Questline>,
    pub selected: Option<usize>,
    pub new_name: String,
}

impl QuestlineEditor {
    pub fn new() -> Self {
        QuestlineEditor {
            questlines: Vec::new(),
            selected: None,
            new_name: String::new(),
        }
    }
}

pub fn show_questline_editor(ui: &mut egui::Ui, questline_editor: &mut QuestlineEditor, quest_editor: &QuestEditor) {
    ui.horizontal(|ui| {
        ui.strong("Questlines");
        ui.add(egui::TextEdit::singleline(&mut questline_editor.new_name).hint_text("Questline name...").desired_width(140.0));
        if ui.button("+ Add").clicked() && !questline_editor.new_name.is_empty() {
            questline_editor.questlines.push(Questline::new(&questline_editor.new_name.clone()));
            questline_editor.new_name.clear();
        }
    });
    ui.separator();

    let ql_count = questline_editor.questlines.len();
    egui::SidePanel::left("ql_list_panel")
        .resizable(true)
        .default_width(180.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("ql_list_scroll").show(ui, |ui| {
                let mut to_delete: Option<usize> = None;
                for qi in 0..ql_count {
                    let is_sel = questline_editor.selected == Some(qi);
                    let ql = &questline_editor.questlines[qi];
                    let progress = ql.progress(quest_editor);
                    let name = ql.name.clone();
                    let quest_count = ql.quest_ids.len();

                    ui.push_id(qi, |ui| {
                        egui::Frame::none()
                            .fill(if is_sel { Color32::from_rgb(40, 50, 70) } else { Color32::from_rgb(28, 28, 35) })
                            .stroke(Stroke::new(1.0, if is_sel { Color32::from_rgb(100, 150, 255) } else { Color32::from_rgb(45, 45, 55) }))
                            .inner_margin(4.0)
                            .corner_radius(3.0)
                            .show(ui, |ui| {
                                let resp = ui.selectable_label(is_sel, RichText::new(&name).color(if is_sel { Color32::WHITE } else { Color32::LIGHT_GRAY }));
                                if resp.clicked() { questline_editor.selected = Some(qi); }
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{} quests", quest_count)).small().color(Color32::GRAY));
                                    draw_progress_bar_inline(ui, progress, 60.0, 6.0, Color32::from_rgb(80, 180, 80));
                                    if ui.small_button("x").clicked() { to_delete = Some(qi); }
                                });
                            });
                        ui.add_space(2.0);
                    });
                }
                if let Some(idx) = to_delete {
                    questline_editor.questlines.remove(idx);
                    if questline_editor.selected == Some(idx) { questline_editor.selected = None; }
                }
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        if let Some(qi) = questline_editor.selected {
            if qi < questline_editor.questlines.len() {
                let ql = &mut questline_editor.questlines[qi];
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut ql.name);
                });
                ui.add(egui::TextEdit::multiline(&mut ql.description).desired_rows(2).desired_width(f32::INFINITY).hint_text("Questline description..."));
                ui.separator();
                ui.strong("Quest Order");

                let mut to_remove: Option<usize> = None;
                let mut move_up: Option<usize> = None;
                let mut move_dn: Option<usize> = None;

                for (idx, &qid) in ql.quest_ids.iter().enumerate() {
                    let quest = quest_editor.get_quest_by_id(qid);
                    let quest_name = quest.map(|q| q.name.as_str()).unwrap_or("(deleted)");
                    let state = quest.map(|q| &q.state).cloned();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}.", idx + 1)).color(Color32::GRAY).small());
                        if let Some(s) = &state {
                            let sc = s.color();
                            let (dr, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover());
                            ui.painter().circle_filled(dr.center(), 4.0, sc);
                        }
                        ui.label(RichText::new(quest_name).color(Color32::LIGHT_GRAY));
                        if idx > 0 && ui.small_button("^").clicked() { move_up = Some(idx); }
                        if ui.small_button("v").clicked() { move_dn = Some(idx); }
                        if ui.small_button("x").clicked() { to_remove = Some(idx); }
                    });
                }
                if let Some(i) = to_remove { ql.quest_ids.remove(i); }
                if let Some(i) = move_up { if i > 0 { ql.quest_ids.swap(i, i-1); } }
                if let Some(i) = move_dn { let len = ql.quest_ids.len(); if i+1 < len { ql.quest_ids.swap(i, i+1); } }

                ui.separator();
                ui.label("Add Quest:");
                egui::ScrollArea::vertical().id_salt("ql_add_scroll").max_height(120.0).show(ui, |ui| {
                    for q in &quest_editor.quests {
                        if ql.quest_ids.contains(&q.id) { continue; }
                        if ui.selectable_label(false, RichText::new(&q.name).small()).clicked() {
                            ql.quest_ids.push(q.id);
                        }
                    }
                });
            }
        }
    });
}

// ---- Dynamic event system ----

#[derive(Clone, Debug)]
pub enum QuestEvent {
    QuestStarted(QuestId),
    QuestCompleted(QuestId),
    QuestFailed(QuestId),
    ObjectiveUpdated { quest_id: QuestId, objective_id: usize, progress: u32 },
    FlagChanged { name: String, value: bool },
    ReputationChanged { faction: String, amount: i32 },
}

impl QuestEvent {
    pub fn description(&self, editor: &QuestEditor) -> String {
        match self {
            QuestEvent::QuestStarted(id) => {
                let name = editor.get_quest_by_id(*id).map(|q| q.name.as_str()).unwrap_or("?");
                format!("Quest started: {}", name)
            }
            QuestEvent::QuestCompleted(id) => {
                let name = editor.get_quest_by_id(*id).map(|q| q.name.as_str()).unwrap_or("?");
                format!("Quest completed: {}", name)
            }
            QuestEvent::QuestFailed(id) => {
                let name = editor.get_quest_by_id(*id).map(|q| q.name.as_str()).unwrap_or("?");
                format!("Quest failed: {}", name)
            }
            QuestEvent::ObjectiveUpdated { quest_id, objective_id, progress } => {
                format!("Objective {} in quest {} updated to {}", objective_id, quest_id, progress)
            }
            QuestEvent::FlagChanged { name, value } => {
                format!("Flag '{}' set to {}", name, value)
            }
            QuestEvent::ReputationChanged { faction, amount } => {
                format!("Reputation with {} changed by {:+}", faction, amount)
            }
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            QuestEvent::QuestStarted(_) => Color32::from_rgb(100, 200, 255),
            QuestEvent::QuestCompleted(_) => Color32::from_rgb(100, 220, 100),
            QuestEvent::QuestFailed(_) => Color32::from_rgb(220, 80, 80),
            QuestEvent::ObjectiveUpdated { .. } => Color32::from_rgb(220, 200, 80),
            QuestEvent::FlagChanged { .. } => Color32::from_rgb(200, 150, 255),
            QuestEvent::ReputationChanged { .. } => Color32::from_rgb(150, 200, 255),
        }
    }
}

pub struct QuestEventLog {
    pub events: Vec<QuestEvent>,
    pub max_events: usize,
}

impl QuestEventLog {
    pub fn new() -> Self {
        QuestEventLog { events: Vec::new(), max_events: 200 }
    }

    pub fn push(&mut self, event: QuestEvent) {
        self.events.push(event);
        if self.events.len() > self.max_events {
            self.events.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn events_for_quest(&self, id: QuestId) -> Vec<&QuestEvent> {
        self.events.iter().filter(|e| match e {
            QuestEvent::QuestStarted(qid) | QuestEvent::QuestCompleted(qid) | QuestEvent::QuestFailed(qid) => *qid == id,
            QuestEvent::ObjectiveUpdated { quest_id, .. } => *quest_id == id,
            _ => false,
        }).collect()
    }
}

pub fn show_event_log(ui: &mut egui::Ui, log: &QuestEventLog, editor: &QuestEditor) {
    egui::CollapsingHeader::new(RichText::new(format!("Event Log ({})", log.events.len())).color(Color32::from_rgb(180, 180, 255)))
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("event_log_scroll").max_height(180.0).show(ui, |ui| {
                for event in log.events.iter().rev() {
                    let desc = event.description(editor);
                    ui.label(RichText::new(desc).small().color(event.color()));
                }
                if log.events.is_empty() {
                    ui.label(RichText::new("No events recorded.").color(Color32::GRAY));
                }
            });
        });
}

// ---- Objective progress simulator ----

pub fn simulate_objective_progress(editor: &mut QuestEditor, delta_time: f32) {
    let rng_seed = delta_time.to_bits();
    for quest in editor.quests.iter_mut() {
        if quest.state != QuestState::Active { continue; }
        for obj in quest.objectives.iter_mut() {
            if obj.current_progress >= obj.required_progress { continue; }
            // Fake progression for demo purposes
            let increment = match &obj.objective_type {
                ObjectiveType::Kill { .. } => {
                    if (rng_seed >> (obj.id % 8)) & 1 == 0 { 1 } else { 0 }
                }
                ObjectiveType::Collect { .. } => {
                    if (rng_seed >> (obj.id % 6 + 1)) & 3 == 0 { 1 } else { 0 }
                }
                _ => 0,
            };
            obj.current_progress = (obj.current_progress + increment).min(obj.required_progress);
        }
    }
}

// ---- Quest priority sorter visual ----

pub fn show_priority_grid(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Priority Overview").color(Color32::from_rgb(255, 200, 100)))
        .default_open(false)
        .show(ui, |ui| {
            let mut sorted: Vec<(usize, String, u32, Color32)> = editor.quests.iter().enumerate()
                .map(|(i, q)| (i, q.name.clone(), q.priority, q.category.color()))
                .collect();
            sorted.sort_by(|a, b| b.2.cmp(&a.2));

            for (idx, (qi, name, priority, color)) in sorted.iter().take(15).enumerate() {
                ui.horizontal(|ui| {
                    let bar_w = (*priority as f32 / 999.0 * 100.0).max(2.0);
                    ui.label(RichText::new(format!("{:2}.", idx + 1)).monospace().small().color(Color32::GRAY));
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(100.0, 14.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(25, 25, 30));
                    let fill = Rect::from_min_size(rect.min, Vec2::new(bar_w, rect.height()));
                    ui.painter().rect_filled(fill, 2.0, *color);
                    ui.label(RichText::new(format!("[{}] {}", priority, name)).small().color(Color32::LIGHT_GRAY));
                });
            }
            if sorted.len() > 15 {
                ui.label(RichText::new(format!("...and {} more", sorted.len() - 15)).small().color(Color32::GRAY));
            }
        });
}

// ---- Objective type distribution chart ----

pub fn show_objective_type_chart(ui: &mut egui::Ui, editor: &QuestEditor) {
    let mut type_counts: HashMap<&'static str, usize> = HashMap::new();
    for quest in &editor.quests {
        for obj in &quest.objectives {
            *type_counts.entry(obj.objective_type.label()).or_insert(0) += 1;
        }
    }
    if type_counts.is_empty() { return; }

    egui::CollapsingHeader::new(RichText::new("Objective Types").color(Color32::from_rgb(200, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let total: usize = type_counts.values().sum();
            let mut sorted: Vec<(&&str, &usize)> = type_counts.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));

            let bar_max = 150.0_f32;
            for (type_label, count) in &sorted {
                let count_val = **count;
                let bar_w = (count_val as f32 / total as f32) * bar_max;
                ui.horizontal(|ui| {
                    ui.add_sized(Vec2::new(60.0, 14.0), egui::Label::new(RichText::new(**type_label).small().color(Color32::GRAY)));
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_max, 14.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(25, 25, 30));
                    let fill = Rect::from_min_size(rect.min, Vec2::new(bar_w, rect.height()));
                    ui.painter().rect_filled(fill, 2.0, Color32::from_rgb(100, 150, 220));
                    ui.label(RichText::new(format!("{} ({:.0}%)", count_val, count_val as f32 / total as f32 * 100.0)).small().color(Color32::GRAY));
                });
            }
        });
}

// ---- Category color legend ----

pub fn show_category_legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for cat in QuestCategory::all() {
            let color = cat.color();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, color);
            ui.label(RichText::new(cat.label()).small().color(color));
            ui.add_space(6.0);
        }
    });
}

// ---- State color legend ----

pub fn show_state_legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for state in QuestState::all() {
            let color = state.color();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 5.0, color);
            ui.label(RichText::new(state.label()).small().color(color));
            ui.add_space(6.0);
        }
    });
}

// ---- Quest copy/paste buffer ----

#[derive(Clone, Debug, Default)]
pub struct QuestClipboard {
    pub quests: Vec<Quest>,
}

impl QuestClipboard {
    pub fn copy_quest(quest: &Quest) -> QuestClipboard {
        QuestClipboard { quests: vec![quest.clone()] }
    }

    pub fn paste_into(&self, editor: &mut QuestEditor) {
        for q in &self.quests {
            let new_id = editor.next_id;
            let mut new_q = q.clone();
            new_q.id = new_id;
            new_q.name = format!("{} (copy)", new_q.name);
            new_q.prereqs.clear();
            new_q.graph_pos += Vec2::new(30.0, 30.0);
            editor.quests.push(new_q);
            editor.next_id += 1;
        }
    }
}

// ---- Full quest editor composite window ----

pub fn show_full_quest_window(
    ctx: &egui::Context,
    editor: &mut QuestEditor,
    lore: &mut LoreEditor,
    questlines: &mut QuestlineEditor,
    event_log: &mut QuestEventLog,
    search_replace: &mut SearchReplaceState,
    open: &mut bool,
) {
    egui::Window::new("Quest System — Master Editor")
        .open(open)
        .resizable(true)
        .default_size([1600.0, 900.0])
        .min_size([1000.0, 600.0])
        .show(ctx, |ui| {
            egui::TopBottomPanel::top("quest_master_top").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Quest & Narrative System").size(16.0).color(Color32::from_rgb(220, 200, 100)));
                    ui.separator();
                    if ui.button("Sort Priority").clicked() { editor.sort_by_priority(); }
                    if ui.button("Sort Name").clicked() { editor.sort_by_name(); }
                    if ui.button("Sort Level").clicked() { editor.sort_by_level(); }
                    if ui.button("Auto-Layout").clicked() { auto_layout_graph(editor); }
                    ui.separator();
                    show_category_legend(ui);
                });
            });

            egui::TopBottomPanel::bottom("quest_master_bottom").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let avail = editor.available_quests().len();
                    let active = editor.active_quests().len();
                    let ke = editor.total_objective_count();
                    ui.label(RichText::new(format!(
                        "Quests: {} | Active: {} | Available: {} | Objectives: {} | Flags: {} | Factions: {}",
                        editor.quests.len(), active, avail, ke, editor.flags.len(), editor.factions.len()
                    )).small().color(Color32::GRAY));
                    ui.separator();
                    show_state_legend(ui);
                });
            });

            egui::SidePanel::left("quest_master_left")
                .resizable(true)
                .default_width(240.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("quest_master_left_scroll").show(ui, |ui| {
                        show_statistics(ui, editor);
                        ui.separator();
                        show_validation_panel(ui, editor);
                        ui.separator();
                        show_narrative_warnings(ui, editor);
                        ui.separator();
                        show_reward_breakdown(ui, editor);
                        ui.separator();
                        show_level_distribution(ui, editor);
                        ui.separator();
                        show_objective_type_chart(ui, editor);
                        ui.separator();
                        show_priority_grid(ui, editor);
                        ui.separator();
                        show_batch_editor(ui, editor);
                        ui.separator();
                        show_search_replace(ui, editor, search_replace);
                        ui.separator();
                        show_event_log(ui, event_log, editor);
                    });
                });

            egui::SidePanel::right("quest_master_right")
                .resizable(true)
                .default_width(220.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("quest_master_right_scroll").show(ui, |ui| {
                        show_flag_inspector(ui, editor);
                        ui.separator();
                        show_faction_editor(ui, editor);
                        ui.separator();
                        show_hotkey_reference(ui);
                    });
                });

            show(ui, editor);
        });
}

// ===================== Extended Quest System Components =====================

// ---- Dialogue system integration ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueLine {
    pub speaker: String,
    pub text: String,
    pub conditions: Vec<String>,
    pub next_line: Option<usize>,
    pub choices: Vec<DialogueChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueChoice {
    pub text: String,
    pub condition: Option<String>,
    pub next_line: Option<usize>,
    pub sets_flag: Option<String>,
    pub gives_quest: Option<QuestId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueTree {
    pub id: usize,
    pub name: String,
    pub npc_id: String,
    pub lines: Vec<DialogueLine>,
    pub start_line: usize,
    pub quest_context: Option<QuestId>,
}

impl DialogueTree {
    pub fn new(id: usize, npc_id: &str) -> Self {
        DialogueTree {
            id,
            name: format!("Dialogue {}", id),
            npc_id: npc_id.to_string(),
            lines: vec![
                DialogueLine {
                    speaker: npc_id.to_string(),
                    text: "Hello, adventurer.".to_string(),
                    conditions: Vec::new(),
                    next_line: None,
                    choices: vec![
                        DialogueChoice {
                            text: "Farewell.".to_string(),
                            condition: None,
                            next_line: None,
                            sets_flag: None,
                            gives_quest: None,
                        }
                    ],
                }
            ],
            start_line: 0,
            quest_context: None,
        }
    }
}

pub struct DialogueEditor {
    pub trees: Vec<DialogueTree>,
    pub selected_tree: Option<usize>,
    pub selected_line: Option<usize>,
    pub next_id: usize,
    pub new_npc_name: String,
}

impl DialogueEditor {
    pub fn new() -> Self {
        let mut de = DialogueEditor {
            trees: Vec::new(),
            selected_tree: None,
            selected_line: None,
            next_id: 0,
            new_npc_name: String::new(),
        };
        de.trees.push(DialogueTree::new(0, "Oracle Vera"));
        de.trees.push(DialogueTree::new(1, "Aldric the Merchant"));
        de.next_id = 2;
        de
    }
}

pub fn show_dialogue_editor(ui: &mut egui::Ui, dialogue: &mut DialogueEditor, quest_editor: &QuestEditor) {
    ui.horizontal(|ui| {
        ui.strong("Dialogue Editor");
        ui.add(egui::TextEdit::singleline(&mut dialogue.new_npc_name).hint_text("NPC name...").desired_width(130.0));
        if ui.button("+ Add Tree").clicked() && !dialogue.new_npc_name.is_empty() {
            let id = dialogue.next_id;
            dialogue.trees.push(DialogueTree::new(id, &dialogue.new_npc_name.clone()));
            dialogue.next_id += 1;
            dialogue.new_npc_name.clear();
        }
    });
    ui.separator();

    let tree_count = dialogue.trees.len();
    egui::SidePanel::left("dialogue_tree_list")
        .resizable(true)
        .default_width(160.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("dialogue_tree_scroll").show(ui, |ui| {
                for ti in 0..tree_count {
                    let is_sel = dialogue.selected_tree == Some(ti);
                    let tree_name = dialogue.trees[ti].name.clone();
                    let npc = dialogue.trees[ti].npc_id.clone();
                    if ui.selectable_label(is_sel, RichText::new(format!("{} ({})", tree_name, npc)).small()).clicked() {
                        dialogue.selected_tree = Some(ti);
                        dialogue.selected_line = None;
                    }
                }
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        if let Some(ti) = dialogue.selected_tree {
            if ti < dialogue.trees.len() {
                let tree = &mut dialogue.trees[ti];
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut tree.name);
                    ui.label("NPC:");
                    ui.text_edit_singleline(&mut tree.npc_id);
                });
                ui.horizontal(|ui| {
                    ui.label("Quest context:");
                    let quest_names: Vec<(Option<QuestId>, String)> = std::iter::once((None, "None".to_string()))
                        .chain(quest_editor.quests.iter().map(|q| (Some(q.id), q.name.clone())))
                        .collect();
                    let cur_name = tree.quest_context
                        .and_then(|id| quest_editor.get_quest_by_id(id).map(|q| q.name.clone()))
                        .unwrap_or_else(|| "None".to_string());
                    egui::ComboBox::from_id_salt("dialogue_quest_ctx")
                        .selected_text(cur_name)
                        .show_ui(ui, |ui| {
                            for (qid, qname) in &quest_names {
                                if ui.selectable_label(tree.quest_context == *qid, qname.as_str()).clicked() {
                                    tree.quest_context = *qid;
                                }
                            }
                        });
                });
                ui.separator();
                ui.strong("Dialogue Lines");

                let line_count = tree.lines.len();
                let mut to_remove: Option<usize> = None;
                for li in 0..line_count {
                    let is_sel_line = dialogue.selected_line == Some(li);
                    let line = &mut tree.lines[li];
                    ui.push_id(li, |ui| {
                        egui::Frame::none()
                            .fill(if is_sel_line { Color32::from_rgb(35, 45, 60) } else { Color32::from_rgb(22, 22, 30) })
                            .stroke(Stroke::new(1.0, if is_sel_line { Color32::from_rgb(100, 150, 255) } else { Color32::from_rgb(40, 40, 55) }))
                            .inner_margin(4.0)
                            .corner_radius(3.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("[{}]", li)).small().color(Color32::GRAY));
                                    ui.text_edit_singleline(&mut line.speaker);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("x").clicked() { to_remove = Some(li); }
                                    });
                                });
                                ui.add(egui::TextEdit::multiline(&mut line.text).desired_rows(2).desired_width(f32::INFINITY));
                                let choice_count = line.choices.len();
                                for ci in 0..choice_count {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("  >{}", ci)).small().color(Color32::from_rgb(200, 180, 100)));
                                        ui.text_edit_singleline(&mut line.choices[ci].text);
                                    });
                                }
                                if ui.small_button("+ Choice").clicked() {
                                    line.choices.push(DialogueChoice {
                                        text: "...".to_string(),
                                        condition: None,
                                        next_line: None,
                                        sets_flag: None,
                                        gives_quest: None,
                                    });
                                }
                            });
                        if ui.interact(ui.min_rect(), egui::Id::new("dl_click").with(li), egui::Sense::click()).clicked() {
                            dialogue.selected_line = Some(li);
                        }
                        ui.add_space(2.0);
                    });
                }
                if let Some(li) = to_remove { tree.lines.remove(li); }
                if ui.button("+ Add Line").clicked() {
                    tree.lines.push(DialogueLine {
                        speaker: tree.npc_id.clone(),
                        text: String::new(),
                        conditions: Vec::new(),
                        next_line: None,
                        choices: Vec::new(),
                    });
                }
            }
        }
    });
}

// ---- Quest branching conditions ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BranchCondition {
    QuestState { quest_id: QuestId, state: QuestState },
    FlagSet { flag: String },
    FlagNotSet { flag: String },
    FactionStanding { faction: String, min_rep: i32 },
    PlayerLevel { min_level: u32 },
    HasItem { item_id: String, count: u32 },
    TimeOfDay { hour_min: u8, hour_max: u8 },
    AllOf(Vec<BranchCondition>),
    AnyOf(Vec<BranchCondition>),
    Not(Box<BranchCondition>),
}

impl BranchCondition {
    pub fn describe(&self) -> String {
        match self {
            BranchCondition::QuestState { quest_id, state } => format!("Quest {} is {}", quest_id, state.label()),
            BranchCondition::FlagSet { flag } => format!("Flag '{}' is set", flag),
            BranchCondition::FlagNotSet { flag } => format!("Flag '{}' is NOT set", flag),
            BranchCondition::FactionStanding { faction, min_rep } => format!("{} rep >= {}", faction, min_rep),
            BranchCondition::PlayerLevel { min_level } => format!("Player level >= {}", min_level),
            BranchCondition::HasItem { item_id, count } => format!("Has {}x {}", count, item_id),
            BranchCondition::TimeOfDay { hour_min, hour_max } => format!("Time {}:00 - {}:00", hour_min, hour_max),
            BranchCondition::AllOf(conds) => format!("ALL OF ({} conditions)", conds.len()),
            BranchCondition::AnyOf(conds) => format!("ANY OF ({} conditions)", conds.len()),
            BranchCondition::Not(inner) => format!("NOT ({})", inner.describe()),
        }
    }

    pub fn evaluate(&self, editor: &QuestEditor) -> bool {
        match self {
            BranchCondition::QuestState { quest_id, state } => {
                editor.get_quest_by_id(*quest_id).map(|q| &q.state == state).unwrap_or(false)
            }
            BranchCondition::FlagSet { flag } => editor.flag_value(flag),
            BranchCondition::FlagNotSet { flag } => !editor.flag_value(flag),
            BranchCondition::FactionStanding { faction, min_rep } => {
                editor.faction_standing(faction).map(|r| r >= *min_rep).unwrap_or(false)
            }
            BranchCondition::PlayerLevel { .. } => true,
            BranchCondition::HasItem { .. } => true,
            BranchCondition::TimeOfDay { .. } => true,
            BranchCondition::AllOf(conds) => conds.iter().all(|c| c.evaluate(editor)),
            BranchCondition::AnyOf(conds) => conds.iter().any(|c| c.evaluate(editor)),
            BranchCondition::Not(inner) => !inner.evaluate(editor),
        }
    }
}

pub fn show_condition_editor(ui: &mut egui::Ui, condition: &mut BranchCondition, editor: &QuestEditor) {
    match condition {
        BranchCondition::QuestState { quest_id, state } => {
            ui.horizontal(|ui| {
                ui.label("Quest ID:");
                ui.add(egui::DragValue::new(quest_id).range(0..=9999).speed(1.0));
                ui.label("State:");
                egui::ComboBox::from_id_salt("cond_quest_state")
                    .selected_text(state.label())
                    .show_ui(ui, |ui| {
                        for s in QuestState::all() {
                            if ui.selectable_label(state == s, s.label()).clicked() { *state = s.clone(); }
                        }
                    });
            });
        }
        BranchCondition::FlagSet { flag } | BranchCondition::FlagNotSet { flag } => {
            ui.horizontal(|ui| {
                ui.label("Flag:");
                ui.text_edit_singleline(flag);
                let val = editor.flag_value(flag);
                let color = if val { Color32::from_rgb(100, 220, 100) } else { Color32::from_rgb(180, 80, 80) };
                ui.label(RichText::new(if val { "SET" } else { "NOT SET" }).small().color(color));
            });
        }
        BranchCondition::FactionStanding { faction, min_rep } => {
            ui.horizontal(|ui| {
                ui.label("Faction:");
                ui.text_edit_singleline(faction);
                ui.label("Min Rep:");
                ui.add(egui::DragValue::new(min_rep).range(-9999..=9999).speed(1.0));
            });
        }
        BranchCondition::PlayerLevel { min_level } => {
            ui.horizontal(|ui| {
                ui.label("Min Level:");
                ui.add(egui::DragValue::new(min_level).range(1..=100).speed(1.0));
            });
        }
        BranchCondition::HasItem { item_id, count } => {
            ui.horizontal(|ui| {
                ui.label("Item ID:");
                ui.text_edit_singleline(item_id);
                ui.label("Count:");
                ui.add(egui::DragValue::new(count).range(1..=9999).speed(1.0));
            });
        }
        _ => {
            ui.label(RichText::new(condition.describe()).small().color(Color32::GRAY));
        }
    }

    let met = condition.evaluate(editor);
    let met_color = if met { Color32::from_rgb(100, 220, 100) } else { Color32::from_rgb(220, 80, 80) };
    ui.label(RichText::new(format!("Condition: {}", if met { "MET" } else { "NOT MET" })).small().color(met_color));
}

// ---- Quest timer display ----

pub fn show_quest_timers(ui: &mut egui::Ui, editor: &QuestEditor, current_time: f32) {
    let timed_quests: Vec<&Quest> = editor.quests.iter()
        .filter(|q| q.state == QuestState::Active && q.time_limit.is_some())
        .collect();

    if timed_quests.is_empty() { return; }

    egui::CollapsingHeader::new(RichText::new(format!("Active Timers ({})", timed_quests.len())).color(Color32::from_rgb(255, 150, 50)))
        .default_open(true)
        .show(ui, |ui| {
            for quest in &timed_quests {
                if let Some(limit) = quest.time_limit {
                    let remaining = (limit - current_time % limit).max(0.0);
                    let fraction = remaining / limit;
                    let mins = (remaining / 60.0) as u32;
                    let secs = remaining as u32 % 60;

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&quest.name).small().color(Color32::LIGHT_GRAY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let timer_color = if fraction < 0.25 { Color32::from_rgb(255, 80, 80) }
                                else if fraction < 0.5 { Color32::from_rgb(255, 180, 50) }
                                else { Color32::from_rgb(100, 200, 100) };
                            ui.label(RichText::new(format!("{:02}:{:02}", mins, secs)).monospace().color(timer_color));
                        });
                    });
                    draw_progress_bar_inline(ui, fraction, ui.available_width(), 6.0, if fraction < 0.25 { Color32::from_rgb(220, 60, 60) } else { Color32::from_rgb(100, 180, 255) });
                    ui.add_space(2.0);
                }
            }
        });
}

// ---- Mini quest map (2D world-space preview) ----

pub fn draw_quest_map(ui: &mut egui::Ui, editor: &QuestEditor) {
    let size = Vec2::new(ui.available_width(), 120.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, Color32::from_rgb(20, 25, 30));
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(50, 60, 70)), egui::StrokeKind::Inside);

    // Draw connections between quests with prereqs
    let zoom = 0.4_f32;
    let offset = Vec2::new(rect.width() / 2.0, rect.height() / 2.0);

    for quest in &editor.quests {
        for &prereq_id in &quest.prereqs {
            if let Some(prereq) = editor.get_quest_by_id(prereq_id) {
                let from = Pos2::new(
                    rect.min.x + prereq.graph_pos.x * zoom + offset.x - rect.width() * 0.5,
                    rect.min.y + prereq.graph_pos.y * zoom + offset.y - rect.height() * 0.5,
                );
                let to = Pos2::new(
                    rect.min.x + quest.graph_pos.x * zoom + offset.x - rect.width() * 0.5,
                    rect.min.y + quest.graph_pos.y * zoom + offset.y - rect.height() * 0.5,
                );
                if rect.contains(from) || rect.contains(to) {
                    painter.line_segment([from, to], Stroke::new(0.5, Color32::from_rgba_unmultiplied(100, 100, 150, 100)));
                }
            }
        }
    }

    for quest in &editor.quests {
        let sp = Pos2::new(
            rect.min.x + quest.graph_pos.x * zoom + offset.x - rect.width() * 0.5,
            rect.min.y + quest.graph_pos.y * zoom + offset.y - rect.height() * 0.5,
        );
        if !rect.contains(sp) { continue; }
        let color = quest.state.color();
        let radius = if quest.category == QuestCategory::Main { 5.0 } else { 3.0 };
        painter.circle_filled(sp, radius, color);
    }

    painter.text(rect.min + Vec2::new(6.0, 6.0), egui::Align2::LEFT_TOP, "Quest Map", FontId::proportional(9.0), Color32::from_rgb(120, 120, 140));
}

// ---- Narrative arc editor ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NarrativeArcBeat {
    Inciting,
    RisingAction,
    Climax,
    FallingAction,
    Resolution,
    Custom(String),
}

impl NarrativeArcBeat {
    pub fn label(&self) -> String {
        match self {
            NarrativeArcBeat::Inciting => "Inciting Incident".to_string(),
            NarrativeArcBeat::RisingAction => "Rising Action".to_string(),
            NarrativeArcBeat::Climax => "Climax".to_string(),
            NarrativeArcBeat::FallingAction => "Falling Action".to_string(),
            NarrativeArcBeat::Resolution => "Resolution".to_string(),
            NarrativeArcBeat::Custom(s) => s.clone(),
        }
    }

    pub fn y_position(&self) -> f32 {
        match self {
            NarrativeArcBeat::Inciting => 0.2,
            NarrativeArcBeat::RisingAction => 0.5,
            NarrativeArcBeat::Climax => 0.9,
            NarrativeArcBeat::FallingAction => 0.6,
            NarrativeArcBeat::Resolution => 0.3,
            NarrativeArcBeat::Custom(_) => 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NarrativeArc {
    pub name: String,
    pub beats: Vec<(NarrativeArcBeat, QuestId)>,
    pub description: String,
}

pub fn draw_narrative_arc(ui: &mut egui::Ui, arc: &NarrativeArc, editor: &QuestEditor) {
    let width = ui.available_width().min(400.0);
    let height = 80.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, Color32::from_rgb(15, 15, 20));

    if arc.beats.len() < 2 { return; }

    let step = width / (arc.beats.len() - 1) as f32;
    let mut prev: Option<Pos2> = None;
    for (i, (beat, qid)) in arc.beats.iter().enumerate() {
        let x = rect.min.x + i as f32 * step;
        let y = rect.max.y - beat.y_position() * height;
        let pt = Pos2::new(x, y);

        if let Some(p) = prev {
            painter.line_segment([p, pt], Stroke::new(2.0, Color32::from_rgb(150, 100, 255)));
        }
        prev = Some(pt);

        let quest_name = editor.get_quest_by_id(*qid).map(|q| q.name.as_str()).unwrap_or("?");
        painter.circle_filled(pt, 5.0, Color32::from_rgb(200, 150, 255));
        painter.text(pt + Vec2::new(0.0, -10.0), egui::Align2::CENTER_BOTTOM, beat.label(), FontId::proportional(8.0), Color32::from_rgb(180, 150, 220));
        painter.text(pt + Vec2::new(0.0, 8.0), egui::Align2::CENTER_TOP, quest_name, FontId::proportional(7.0), Color32::GRAY);
    }
}

// ---- Quest world state snapshot ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldStateSnapshot {
    pub label: String,
    pub quest_states: Vec<(QuestId, QuestState)>,
    pub flag_values: Vec<(String, bool)>,
    pub faction_reps: Vec<(String, i32)>,
}

impl WorldStateSnapshot {
    pub fn capture(label: &str, editor: &QuestEditor) -> Self {
        WorldStateSnapshot {
            label: label.to_string(),
            quest_states: editor.quests.iter().map(|q| (q.id, q.state.clone())).collect(),
            flag_values: editor.flags.iter().map(|f| (f.name.clone(), f.value)).collect(),
            faction_reps: editor.factions.iter().map(|f| (f.name.clone(), f.current_rep)).collect(),
        }
    }

    pub fn restore(&self, editor: &mut QuestEditor) {
        for (id, state) in &self.quest_states {
            editor.set_state(*id, state.clone());
        }
        for (flag_name, value) in &self.flag_values {
            editor.set_flag(flag_name, *value);
        }
        for (faction_name, rep) in &self.faction_reps {
            if let Some(f) = editor.factions.iter_mut().find(|f| &f.name == faction_name) {
                f.current_rep = *rep;
            }
        }
    }
}

pub struct WorldStateManager {
    pub snapshots: Vec<WorldStateSnapshot>,
    pub new_label: String,
}

impl WorldStateManager {
    pub fn new() -> Self {
        WorldStateManager {
            snapshots: Vec::new(),
            new_label: "Checkpoint 1".to_string(),
        }
    }
}

pub fn show_world_state_manager(ui: &mut egui::Ui, manager: &mut WorldStateManager, editor: &mut QuestEditor) {
    ui.strong("World State Snapshots");
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut manager.new_label).desired_width(150.0).hint_text("Snapshot label..."));
        if ui.button("Save State").clicked() && !manager.new_label.is_empty() {
            let snap = WorldStateSnapshot::capture(&manager.new_label.clone(), editor);
            manager.snapshots.push(snap);
        }
    });
    ui.separator();

    let mut to_restore: Option<usize> = None;
    let mut to_delete: Option<usize> = None;

    egui::ScrollArea::vertical().id_salt("world_state_scroll").max_height(200.0).show(ui, |ui| {
        for (i, snap) in manager.snapshots.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&snap.label).color(Color32::LIGHT_GRAY));
                ui.label(RichText::new(format!("{} quests, {} flags", snap.quest_states.len(), snap.flag_values.len())).small().color(Color32::GRAY));
                if ui.small_button("Restore").clicked() { to_restore = Some(i); }
                if ui.small_button("x").clicked() { to_delete = Some(i); }
            });
        }
        if manager.snapshots.is_empty() {
            ui.label(RichText::new("No snapshots saved.").color(Color32::GRAY));
        }
    });

    if let Some(i) = to_restore {
        manager.snapshots[i].restore(editor);
    }
    if let Some(i) = to_delete {
        manager.snapshots.remove(i);
    }
}

// ---- Extended QuestEditor keyboard handling ----

pub fn handle_keyboard_extended(ui: &mut egui::Ui, editor: &mut QuestEditor, clipboard: &mut QuestClipboard) {
    ui.input(|i| {
        if i.key_pressed(egui::Key::ArrowDown) { editor.select_next(); }
        if i.key_pressed(egui::Key::ArrowUp) { editor.select_prev(); }
        if i.key_pressed(egui::Key::Delete) {
            if let Some(sel) = editor.selected { editor.delete_quest(sel); }
        }
        if i.key_pressed(egui::Key::D) && i.modifiers.ctrl {
            if let Some(sel) = editor.selected { editor.duplicate_quest(sel); }
        }
        if i.key_pressed(egui::Key::C) && i.modifiers.ctrl {
            if let Some(sel) = editor.selected {
                if sel < editor.quests.len() {
                    *clipboard = QuestClipboard::copy_quest(&editor.quests[sel]);
                }
            }
        }
        if i.key_pressed(egui::Key::V) && i.modifiers.ctrl {
            clipboard.paste_into(editor);
        }
        if i.key_pressed(egui::Key::G) && i.modifiers.ctrl {
            auto_layout_graph(editor);
        }
    });
}

// ============================================================
// QUEST TEMPLATE SYSTEM
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestTemplate {
    pub name: String,
    pub description: String,
    pub category: QuestCategory,
    pub default_objectives: Vec<QuestObjective>,
    pub default_rewards: Vec<Reward>,
    pub suggested_level: u32,
    pub tags: Vec<String>,
}

impl QuestTemplate {
    pub fn builtin_templates() -> Vec<Self> {
        vec![
            Self {
                name: "Fetch Quest".to_string(),
                description: "Retrieve an item and return it to the quest giver.".to_string(),
                category: QuestCategory::Side,
                default_objectives: vec![
                    QuestObjective { id: 0, description: "Collect the item".to_string(), objective_type: ObjectiveType::Collect { item: "item_placeholder".to_string(), count: 1, amount: 1, current: 0 }, required: true, completed: false, optional: false, hint: String::new(), ..QuestObjective::new(0) },
                    QuestObjective { id: 1, description: "Return to quest giver".to_string(), objective_type: ObjectiveType::Talk { character: "Quest Giver".to_string(), npc_id: "npc_quest_giver".to_string(), topic: "delivery".to_string() }, required: true, completed: false, optional: false, hint: String::new(), ..QuestObjective::new(1) },
                ],
                default_rewards: vec![Reward::Gold(100), Reward::Experience(200)],
                suggested_level: 5,
                tags: vec!["fetch".to_string(), "simple".to_string()],
            },
            Self {
                name: "Bounty Hunt".to_string(),
                description: "Kill a target number of enemies of a specific type.".to_string(),
                category: QuestCategory::Side,
                default_objectives: vec![
                    QuestObjective { id: 0, description: "Kill the enemies".to_string(), objective_type: ObjectiveType::Kill { target: "enemy_placeholder".to_string(), enemy_type: "enemy_placeholder".to_string(), count: 10, current: 0 }, required: true, completed: false, optional: false, hint: String::new(), ..QuestObjective::new(0) },
                ],
                default_rewards: vec![Reward::Gold(200), Reward::Experience(500), Reward::Reputation { faction: "guild".to_string(), amount: 10 }],
                suggested_level: 8,
                tags: vec!["combat".to_string(), "bounty".to_string()],
            },
            Self {
                name: "Escort Mission".to_string(),
                description: "Protect an NPC and escort them to a destination.".to_string(),
                category: QuestCategory::Main,
                default_objectives: vec![
                    QuestObjective { id: 0, description: "Meet the escort target".to_string(), objective_type: ObjectiveType::Talk { character: "Escort Target".to_string(), npc_id: "npc_escort".to_string(), topic: "start_escort".to_string() }, required: true, completed: false, optional: false, hint: String::new(), ..QuestObjective::new(0) },
                    QuestObjective { id: 1, description: "Escort safely to destination".to_string(), objective_type: ObjectiveType::Escort { npc: "npc_escort".to_string(), npc_id: "npc_escort".to_string(), destination: "dest_safe_zone".to_string() }, required: true, completed: false, optional: false, hint: String::new(), ..QuestObjective::new(1) },
                ],
                default_rewards: vec![Reward::Gold(300), Reward::Experience(800), Reward::Item { item_id: "reward_item".to_string(), quantity: 1 }],
                suggested_level: 12,
                tags: vec!["escort".to_string(), "main".to_string()],
            },
            Self {
                name: "Exploration Quest".to_string(),
                description: "Discover new locations in the world.".to_string(),
                category: QuestCategory::Side,
                default_objectives: vec![
                    QuestObjective { id: 0, description: "Explore the region".to_string(), objective_type: ObjectiveType::Explore { area: "zone_placeholder".to_string(), zone_id: "zone_placeholder".to_string(), percent: 100 }, required: true, completed: false, optional: false, hint: String::new(), ..QuestObjective::new(0) },
                ],
                default_rewards: vec![Reward::Experience(400), Reward::Unlock { feature: "map_region".to_string() }],
                suggested_level: 3,
                tags: vec!["exploration".to_string(), "discovery".to_string()],
            },
            Self {
                name: "Crafting Quest".to_string(),
                description: "Craft a specific item using gathered materials.".to_string(),
                category: QuestCategory::Tutorial,
                default_objectives: vec![
                    QuestObjective { id: 0, description: "Gather materials".to_string(), objective_type: ObjectiveType::Collect { item: "material_a".to_string(), count: 5, amount: 5, current: 0 }, required: true, completed: false, optional: false, hint: String::new(), ..QuestObjective::new(0) },
                    QuestObjective { id: 1, description: "Craft the item".to_string(), objective_type: ObjectiveType::Craft { item: "crafted_item".to_string(), count: 1, recipe_id: "recipe_placeholder".to_string(), amount: 1 }, required: true, completed: false, optional: false, hint: String::new(), ..QuestObjective::new(1) },
                ],
                default_rewards: vec![Reward::Experience(150), Reward::Item { item_id: "crafted_item".to_string(), quantity: 1 }],
                suggested_level: 1,
                tags: vec!["crafting".to_string(), "tutorial".to_string()],
            },
        ]
    }

    pub fn spawn_quest(&self, id: QuestId) -> Quest {
        let mut q = Quest::new(id);
        q.name = self.name.clone();
        q.description = self.description.clone();
        q.category = self.category.clone();
        q.objectives = self.default_objectives.clone();
        q.rewards = self.default_rewards.clone();
        q.level_requirement = self.suggested_level;
        q.recommended_level = self.suggested_level;
        q.tags = self.tags.clone();
        q.notes = format!("Created from template: {}", self.name);
        q
    }
}

pub fn show_template_browser(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Quest Templates").color(Color32::from_rgb(200, 180, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Click a template to create a new quest from it.").small().color(Color32::GRAY));
            for template in QuestTemplate::builtin_templates() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let cat_color = match template.category {
                            QuestCategory::Main => Color32::from_rgb(255, 200, 50),
                            QuestCategory::Side => Color32::from_rgb(100, 200, 255),
                            QuestCategory::Daily => Color32::from_rgb(100, 220, 150),
                            QuestCategory::Hidden => Color32::from_rgb(180, 100, 180),
                            QuestCategory::Tutorial => Color32::from_rgb(200, 180, 100),
                        };
                        ui.label(RichText::new(&template.name).color(cat_color).strong());
                        ui.label(RichText::new(format!("Lv.{}", template.suggested_level)).small().color(Color32::GRAY));
                        if ui.small_button("Create Quest").clicked() {
                            let id = editor.quests.len();
                            let quest = template.spawn_quest(id);
                            editor.quests.push(quest);
                            editor.selected = Some(id);
                        }
                    });
                    ui.label(RichText::new(&template.description).small().color(Color32::LIGHT_GRAY));
                    ui.horizontal(|ui| {
                        for tag in &template.tags {
                            ui.label(RichText::new(format!("[{}]", tag)).small().color(Color32::from_rgb(140, 140, 200)));
                        }
                    });
                });
            }
        });
}

// ============================================================
// QUEST SEARCH AND FILTER
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct QuestSearchFilter {
    pub search_text: String,
    pub category_filter: Option<QuestCategory>,
    pub state_filter: Option<QuestState>,
    pub min_level: u32,
    pub max_level: u32,
    pub tag_filter: String,
    pub show_hidden: bool,
    pub show_completed: bool,
}

impl QuestSearchFilter {
    pub fn new() -> Self {
        Self { min_level: 0, max_level: 100, show_hidden: false, show_completed: true, ..Default::default() }
    }
    pub fn matches(&self, quest: &Quest) -> bool {
        if !self.search_text.is_empty() {
            let q = self.search_text.to_lowercase();
            if !quest.name.to_lowercase().contains(&q) && !quest.description.to_lowercase().contains(&q) { return false; }
        }
        if let Some(ref cat) = self.category_filter { if &quest.category != cat { return false; } }
        if let Some(ref state) = self.state_filter { if &quest.state != state { return false; } }
        if quest.recommended_level < self.min_level || quest.recommended_level > self.max_level { return false; }
        if !self.tag_filter.is_empty() {
            let tf = self.tag_filter.to_lowercase();
            if !quest.tags.iter().any(|t| t.to_lowercase().contains(&tf)) { return false; }
        }
        if quest.hidden && !self.show_hidden { return false; }
        if quest.state == QuestState::Completed && !self.show_completed { return false; }
        true
    }
}

pub fn show_quest_search_filter(ui: &mut egui::Ui, filter: &mut QuestSearchFilter) {
    egui::CollapsingHeader::new(RichText::new("Search & Filter").color(Color32::from_rgb(150, 200, 255)))
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut filter.search_text);
                if ui.small_button("X").clicked() { filter.search_text.clear(); }
            });
            ui.horizontal(|ui| {
                ui.label("Category:");
                egui::ComboBox::from_id_salt("filter_cat")
                    .selected_text(filter.category_filter.as_ref().map(|c| format!("{:?}", c)).unwrap_or_else(|| "All".to_string()))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filter.category_filter, None, "All");
                        ui.selectable_value(&mut filter.category_filter, Some(QuestCategory::Main), "Main");
                        ui.selectable_value(&mut filter.category_filter, Some(QuestCategory::Side), "Side");
                        ui.selectable_value(&mut filter.category_filter, Some(QuestCategory::Daily), "Daily");
                        ui.selectable_value(&mut filter.category_filter, Some(QuestCategory::Hidden), "Hidden");
                        ui.selectable_value(&mut filter.category_filter, Some(QuestCategory::Tutorial), "Tutorial");
                    });
                ui.label("State:");
                egui::ComboBox::from_id_salt("filter_state")
                    .selected_text(filter.state_filter.as_ref().map(|s| format!("{:?}", s)).unwrap_or_else(|| "All".to_string()))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filter.state_filter, None, "All");
                        ui.selectable_value(&mut filter.state_filter, Some(QuestState::NotStarted), "Not Started");
                        ui.selectable_value(&mut filter.state_filter, Some(QuestState::Active), "Active");
                        ui.selectable_value(&mut filter.state_filter, Some(QuestState::Completed), "Completed");
                        ui.selectable_value(&mut filter.state_filter, Some(QuestState::Failed), "Failed");
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Level:");
                ui.add(egui::DragValue::new(&mut filter.min_level).speed(1).prefix("min:").clamp_range(0u32..=filter.max_level));
                ui.add(egui::DragValue::new(&mut filter.max_level).speed(1).prefix("max:").clamp_range(filter.min_level..=200u32));
                ui.label("Tags:");
                ui.text_edit_singleline(&mut filter.tag_filter);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut filter.show_hidden, "Show hidden");
                ui.checkbox(&mut filter.show_completed, "Show completed");
            });
        });
}

// ============================================================
// QUEST STATISTICS PANEL
// ============================================================

pub fn show_quest_statistics(ui: &mut egui::Ui, quests: &[Quest]) {
    egui::CollapsingHeader::new(RichText::new("Quest Statistics").color(Color32::from_rgb(180, 220, 180)))
        .default_open(false)
        .show(ui, |ui| {
            let total = quests.len();
            let not_started = quests.iter().filter(|q| q.state == QuestState::NotStarted).count();
            let active = quests.iter().filter(|q| q.state == QuestState::Active).count();
            let completed = quests.iter().filter(|q| q.state == QuestState::Completed).count();
            let failed = quests.iter().filter(|q| q.state == QuestState::Failed).count();
            let abandoned = quests.iter().filter(|q| q.state == QuestState::Abandoned).count();
            let total_objectives: usize = quests.iter().map(|q| q.objectives.len()).sum();
            let completed_objectives: usize = quests.iter().map(|q| q.objectives.iter().filter(|o| o.completed).count()).sum();

            egui::Grid::new("quest_stats").num_columns(2).spacing(Vec2::new(10.0, 3.0)).show(ui, |ui| {
                ui.label(RichText::new("Total quests:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", total)).small().monospace()); ui.end_row();
                ui.label(RichText::new("Not started:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", not_started)).small().monospace()); ui.end_row();
                ui.label(RichText::new("Active:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", active)).small().monospace().color(Color32::from_rgb(100, 220, 150))); ui.end_row();
                ui.label(RichText::new("Completed:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", completed)).small().monospace().color(Color32::from_rgb(80, 200, 80))); ui.end_row();
                ui.label(RichText::new("Failed:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", failed)).small().monospace().color(Color32::RED)); ui.end_row();
                ui.label(RichText::new("Abandoned:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", abandoned)).small().monospace().color(Color32::from_rgb(150, 150, 100))); ui.end_row();
                ui.label(RichText::new("Objectives:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}/{}", completed_objectives, total_objectives)).small().monospace()); ui.end_row();
            });

            if total > 0 {
                ui.separator();
                // Completion bar
                let pct = completed as f32 / total as f32;
                let desired = Vec2::new(ui.available_width(), 16.0);
                let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 4.0, Color32::from_rgb(40, 40, 50));
                let fill = Rect::from_min_size(rect.min, Vec2::new(rect.width() * pct, rect.height()));
                painter.rect_filled(fill, 4.0, Color32::from_rgb(80, 200, 80));
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{:.0}% complete", pct * 100.0), FontId::proportional(10.0), Color32::WHITE);

                // Category breakdown
                ui.separator();
                ui.label(RichText::new("By category:").small().strong());
                for cat in &[QuestCategory::Main, QuestCategory::Side, QuestCategory::Daily, QuestCategory::Hidden, QuestCategory::Tutorial] {
                    let cat_total = quests.iter().filter(|q| &q.category == cat).count();
                    let cat_done = quests.iter().filter(|q| &q.category == cat && q.state == QuestState::Completed).count();
                    if cat_total == 0 { continue; }
                    let cat_color = match cat {
                        QuestCategory::Main => Color32::from_rgb(255, 200, 50),
                        QuestCategory::Side => Color32::from_rgb(100, 200, 255),
                        QuestCategory::Daily => Color32::from_rgb(100, 220, 150),
                        QuestCategory::Hidden => Color32::from_rgb(180, 100, 180),
                        QuestCategory::Tutorial => Color32::from_rgb(200, 180, 100),
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:?}", cat)).small().color(cat_color));
                        let bar_w = 100.0 * cat_done as f32 / cat_total as f32;
                        let (r, _) = ui.allocate_exact_size(Vec2::new(100.0, 10.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, 2.0, Color32::from_rgb(40, 40, 50));
                        if cat_done > 0 {
                            let fill = Rect::from_min_size(r.min, Vec2::new(bar_w, r.height()));
                            ui.painter().rect_filled(fill, 2.0, cat_color);
                        }
                        ui.label(RichText::new(format!("{}/{}", cat_done, cat_total)).small().color(Color32::GRAY));
                    });
                }
            }
        });
}

// ============================================================
// EXTENDED QuestEditor METHODS
// ============================================================

impl QuestEditor {
    pub fn show_panel(ctx: &egui::Context, editor: &mut QuestEditor, open: &mut bool) {
        show_panel(ctx, editor, open);
    }

    pub fn quests_by_state(&self, state: QuestState) -> Vec<&Quest> {
        self.quests.iter().filter(|q| q.state == state).collect()
    }

    pub fn quests_by_category(&self, category: &QuestCategory) -> Vec<&Quest> {
        self.quests.iter().filter(|q| &q.category == category).collect()
    }

    pub fn total_objectives(&self) -> usize {
        self.quests.iter().map(|q| q.objectives.len()).sum()
    }

    pub fn completed_objectives_count(&self) -> usize {
        self.quests.iter().map(|q| q.objectives.iter().filter(|o| o.completed).count()).sum()
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: std::collections::HashSet<String> = std::collections::HashSet::new();
        for quest in &self.quests {
            for tag in &quest.tags { tags.insert(tag.clone()); }
        }
        let mut sorted: Vec<String> = tags.into_iter().collect();
        sorted.sort();
        sorted
    }

    pub fn quests_with_tag<'a>(&'a self, tag: &str) -> Vec<&'a Quest> {
        self.quests.iter().filter(|q| q.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn add_quest_from_template(&mut self, template: &QuestTemplate) -> QuestId {
        let id = self.quests.len();
        self.quests.push(template.spawn_quest(id));
        id
    }

    pub fn find_quest_by_name(&self, name: &str) -> Option<&Quest> {
        self.quests.iter().find(|q| q.name == name)
    }

    pub fn quest_summary_text(&self) -> String {
        let total = self.quests.len();
        let active = self.quests.iter().filter(|q| q.state == QuestState::Active).count();
        let done = self.quests.iter().filter(|q| q.state == QuestState::Completed).count();
        format!("{} quests: {} active, {} completed", total, active, done)
    }

    pub fn set_all_states(&mut self, state: QuestState) {
        for quest in &mut self.quests { quest.state = state.clone(); }
    }

    pub fn clear_all_flags(&mut self) {
        for quest in &mut self.quests { quest.flags_on_complete.clear(); quest.flags_on_fail.clear(); }
    }

    pub fn quests_without_objectives(&self) -> Vec<&Quest> {
        self.quests.iter().filter(|q| q.objectives.is_empty()).collect()
    }

    pub fn quests_without_rewards(&self) -> Vec<&Quest> {
        self.quests.iter().filter(|q| q.rewards.is_empty()).collect()
    }

    pub fn validate_all(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        for quest in &self.quests {
            if quest.objectives.is_empty() {
                warnings.push(format!("Quest '{}' has no objectives.", quest.name));
            }
            if quest.name.trim().is_empty() {
                warnings.push(format!("Quest #{} has an empty name.", quest.id));
            }
            for &prereq in &quest.prereqs {
                if prereq >= self.quests.len() {
                    warnings.push(format!("Quest '{}' has invalid prerequisite id {}.", quest.name, prereq));
                }
            }
        }
        warnings
    }
}

// ============================================================
// QUEST VALIDATION PANEL
// ============================================================

pub fn show_quest_validation(ui: &mut egui::Ui, editor: &QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Validation").color(Color32::from_rgb(255, 180, 80)))
        .default_open(false)
        .show(ui, |ui| {
            let warnings = editor.validate_all();
            if warnings.is_empty() {
                ui.label(RichText::new("All quests valid.").color(Color32::from_rgb(80, 200, 80)));
            } else {
                ui.label(RichText::new(format!("{} warnings:", warnings.len())).color(Color32::YELLOW));
                for warn in &warnings {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("!").color(Color32::YELLOW).strong());
                        ui.label(RichText::new(warn).small().color(Color32::from_rgb(240, 200, 100)));
                    });
                }
            }
        });
}

// ============================================================
// QUEST EDITOR TOOLBAR
// ============================================================

pub fn show_quest_editor_toolbar(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    ui.horizontal(|ui| {
        if ui.button("New Quest").clicked() {
            let id = editor.quests.len();
            editor.quests.push(Quest::new(id));
            editor.selected = Some(id);
        }
        if ui.button("Delete").clicked() {
            if let Some(sel) = editor.selected {
                editor.quests.remove(sel);
                editor.selected = None;
            }
        }
        if ui.button("Duplicate").clicked() {
            if let Some(sel) = editor.selected {
                if let Some(quest) = editor.quests.get(sel).cloned() {
                    let new_id = editor.quests.len();
                    let mut new_quest = quest;
                    new_quest.id = new_id;
                    new_quest.name = format!("{} (copy)", new_quest.name);
                    new_quest.graph_pos += Vec2::new(40.0, 40.0);
                    editor.quests.push(new_quest);
                    editor.selected = Some(new_id);
                }
            }
        }
        ui.separator();
        if ui.small_button("List").clicked() { editor.view = crate::quest_system::QuestView::List; }
        if ui.small_button("Graph").clicked() { editor.view = crate::quest_system::QuestView::Graph; }
        if ui.small_button("Timeline").clicked() { editor.view = crate::quest_system::QuestView::Timeline; }
        ui.separator();
        ui.label(RichText::new(editor.quest_summary_text()).small().color(Color32::GRAY));
    });
}

// ============================================================
// QUEST REWARD EDITOR (STANDALONE)
// ============================================================

pub fn show_reward_editor_standalone(ui: &mut egui::Ui, rewards: &mut Vec<Reward>, id_prefix: &str) {
    let mut to_remove = None;
    for (i, reward) in rewards.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            match reward {
                Reward::Gold(amount) => {
                    ui.label(RichText::new("Gold").color(Color32::from_rgb(255, 200, 50)));
                    ui.add(egui::DragValue::new(amount).speed(5.0).clamp_range(0u32..=100000));
                }
                Reward::Experience(amount) => {
                    ui.label(RichText::new("XP").color(Color32::from_rgb(100, 220, 100)));
                    ui.add(egui::DragValue::new(amount).speed(10.0).clamp_range(0u32..=100000));
                }
                Reward::Item { item_id, quantity } => {
                    ui.label(RichText::new("Item").color(Color32::from_rgb(200, 160, 100)));
                    ui.text_edit_singleline(item_id);
                    ui.label("x");
                    ui.add(egui::DragValue::new(quantity).speed(1.0).clamp_range(1u32..=99));
                }
                Reward::Reputation { faction, amount } => {
                    ui.label(RichText::new("Rep").color(Color32::from_rgb(180, 100, 220)));
                    ui.text_edit_singleline(faction);
                    ui.add(egui::DragValue::new(amount).speed(1.0).clamp_range(-100i32..=100));
                }
                Reward::Skill { skill_id, points } => {
                    ui.label(RichText::new("Skill").color(Color32::from_rgb(80, 180, 255)));
                    ui.text_edit_singleline(skill_id);
                    ui.add(egui::DragValue::new(points).speed(1.0).clamp_range(1u32..=100));
                }
                Reward::Unlock { feature } => {
                    ui.label(RichText::new("Unlock").color(Color32::from_rgb(255, 150, 80)));
                    ui.text_edit_singleline(feature);
                }
                Reward::Ability(name) => {
                    ui.label(RichText::new("Ability").color(Color32::from_rgb(200, 100, 255)));
                    ui.text_edit_singleline(name);
                }
            }
            if ui.small_button("X").clicked() { to_remove = Some(i); }
        });
    }
    if let Some(idx) = to_remove { rewards.remove(idx); }
    ui.horizontal(|ui| {
        if ui.small_button("+ Gold").clicked() { rewards.push(Reward::Gold(100)); }
        if ui.small_button("+ XP").clicked() { rewards.push(Reward::Experience(200)); }
        if ui.small_button("+ Item").clicked() { rewards.push(Reward::Item { item_id: "item_id".to_string(), quantity: 1 }); }
        if ui.small_button("+ Rep").clicked() { rewards.push(Reward::Reputation { faction: "faction".to_string(), amount: 10 }); }
        if ui.small_button("+ Unlock").clicked() { rewards.push(Reward::Unlock { feature: "feature_id".to_string() }); }
    });
    let _ = id_prefix;
}

// ============================================================
// QUEST EXPORT SUMMARY
// ============================================================

pub fn show_quest_export_summary(ui: &mut egui::Ui, editor: &QuestEditor) {
    egui::CollapsingHeader::new(RichText::new("Export Summary").color(Color32::from_rgb(150, 220, 180)))
        .default_open(false)
        .show(ui, |ui| {
            let json = serde_json::to_string_pretty(&editor.quests).unwrap_or_else(|_| "{}".to_string());
            let chars = json.len();
            let lines = json.lines().count();
            ui.label(RichText::new(format!("JSON: {} chars / {} lines", chars, lines)).small().color(Color32::GRAY));
            ui.label(RichText::new(format!("Quests: {}", editor.quests.len())).small());
            ui.label(RichText::new(format!("Objectives: {}", editor.total_objectives())).small());
            ui.label(RichText::new(format!("Factions: {}", editor.factions.len())).small());
            ui.label(RichText::new(format!("Quest flags: {}", editor.flags.len())).small());
            ui.separator();
            ui.label(RichText::new("Tags:").small().strong());
            let tags = editor.all_tags();
            ui.horizontal_wrapped(|ui| {
                for tag in &tags {
                    ui.label(RichText::new(format!("[{}]", tag)).small().color(Color32::from_rgb(140, 140, 200)));
                }
            });
            if ui.button("Copy JSON to Clipboard").clicked() {
                ui.ctx().copy_text(json);
            }
        });
}

// ============================================================
// QUEST FULL EDITOR WINDOW
// ============================================================

pub fn show_full_quest_editor_window(ctx: &egui::Context, editor: &mut QuestEditor, open: &mut bool) {
    if !*open { return; }
    let mut still_open = *open;
    egui::Window::new("Full Quest Editor")
        .open(&mut still_open)
        .resizable(true)
        .default_size(Vec2::new(1100.0, 700.0))
        .show(ctx, |ui| {
            show_quest_editor_toolbar(ui, editor);
            ui.separator();
            egui::SidePanel::left("qe_left").min_width(220.0).show_inside(ui, |ui| {
                let mut filter = QuestSearchFilter::new();
                show_quest_search_filter(ui, &mut filter);
                ui.separator();
                show_quest_statistics(ui, &editor.quests);
                ui.separator();
                show_template_browser(ui, editor);
            });
            egui::SidePanel::right("qe_right").min_width(220.0).show_inside(ui, |ui| {
                show_quest_validation(ui, editor);
                ui.separator();
                show_quest_export_summary(ui, editor);
                ui.separator();
                if let Some(sel) = editor.selected {
                    if let Some(quest) = editor.quests.get_mut(sel) {
                        ui.label(RichText::new("Rewards:").strong());
                        show_reward_editor_standalone(ui, &mut quest.rewards, &format!("r_{}", sel));
                    }
                }
            });
            egui::CentralPanel::default().show_inside(ui, |ui| {
                show_full_quest_editor_inner(ui, editor);
            });
        });
    *open = still_open;
}

fn show_full_quest_editor_inner(ui: &mut egui::Ui, editor: &mut QuestEditor) {
    show_quest_editor_toolbar(ui, editor);
    ui.separator();
    show_quest_statistics(ui, &editor.quests.clone());
}

// ============================================================
// QUEST KEYBOARD SHORTCUTS HELP
// ============================================================

pub fn show_quest_keyboard_help(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Keyboard Shortcuts").color(Color32::from_rgb(200, 200, 200)))
        .default_open(false)
        .show(ui, |ui| {
            let shortcuts = [
                ("Ctrl+N", "New quest"),
                ("Ctrl+D", "Duplicate quest"),
                ("Delete", "Delete selected quest"),
                ("Ctrl+C", "Copy quest"),
                ("Ctrl+V", "Paste quest"),
                ("Ctrl+G", "Auto-layout graph"),
                ("1", "Switch to List view"),
                ("2", "Switch to Graph view"),
                ("3", "Switch to Timeline view"),
                ("Ctrl+F", "Focus search"),
                ("Escape", "Deselect"),
                ("Ctrl+S", "Save"),
                ("Ctrl+Z", "Undo"),
                ("Ctrl+Y", "Redo"),
            ];
            egui::Grid::new("quest_shortcuts").num_columns(2).spacing(Vec2::new(12.0, 2.0)).show(ui, |ui| {
                for (key, action) in shortcuts {
                    ui.label(RichText::new(key).monospace().color(Color32::from_rgb(255, 220, 80)));
                    ui.label(RichText::new(action).small().color(Color32::LIGHT_GRAY));
                    ui.end_row();
                }
            });
        });
}

// ============================================================
// QUEST STATUS BAR
// ============================================================

pub fn show_quest_status_bar(ui: &mut egui::Ui, editor: &QuestEditor) {
    ui.horizontal(|ui| {
        let total = editor.quests.len();
        let active = editor.quests.iter().filter(|q| q.state == QuestState::Active).count();
        let done = editor.quests.iter().filter(|q| q.state == QuestState::Completed).count();
        let failed = editor.quests.iter().filter(|q| q.state == QuestState::Failed).count();
        let completion = if total > 0 { done as f32 / total as f32 * 100.0 } else { 0.0 };
        ui.label(RichText::new(format!("{} quests: {} active  {} done  {} failed  ({:.0}% complete)", total, active, done, failed, completion)).small().color(Color32::GRAY));
        ui.separator();
        if let Some(sel) = editor.selected {
            if let Some(q) = editor.quests.get(sel) {
                let state_color = match q.state {
                    QuestState::Active => Color32::from_rgb(100, 220, 150),
                    QuestState::Completed => Color32::from_rgb(80, 200, 80),
                    QuestState::Failed => Color32::RED,
                    QuestState::Abandoned => Color32::from_rgb(150, 150, 100),
                    QuestState::NotStarted => Color32::GRAY,
                };
                ui.label(RichText::new(format!("Selected: {} [{:?}]", q.name, q.state)).small().color(state_color));
            }
        }
    });
}

// ============================================================
// OBJECTIVE PROGRESS BARS
// ============================================================

pub fn draw_objective_progress(ui: &mut egui::Ui, objectives: &[QuestObjective]) {
    for obj in objectives {
        let (current, total) = match &obj.objective_type {
            ObjectiveType::Kill { count, current, .. } => (*current as f32, *count as f32),
            ObjectiveType::Collect { amount, current, .. } => (*current as f32, *amount as f32),
            ObjectiveType::Explore { percent, .. } => (0.0, *percent as f32),
            ObjectiveType::Survive { duration, elapsed } => (*elapsed, *duration),
            _ => (if obj.completed { 1.0 } else { 0.0 }, 1.0),
        };
        let pct = (current / total.max(0.001)).clamp(0.0, 1.0);
        ui.horizontal(|ui| {
            let done_color = if obj.completed { Color32::from_rgb(80, 200, 80) } else if pct > 0.0 { Color32::from_rgb(100, 180, 255) } else { Color32::DARK_GRAY };
            ui.label(RichText::new(if obj.completed { "+" } else { "o" }).color(done_color).strong().small());
            ui.label(RichText::new(&obj.description).small().color(if obj.completed { Color32::GRAY } else { Color32::LIGHT_GRAY }));
            if total > 1.0 {
                let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 8.0), egui::Sense::hover());
                ui.painter().rect_filled(bar_rect, 2.0, Color32::from_rgb(40, 40, 50));
                let fill = Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width() * pct, bar_rect.height()));
                ui.painter().rect_filled(fill, 2.0, done_color);
                ui.label(RichText::new(format!("{:.0}/{:.0}", current, total)).small().color(Color32::GRAY));
            }
        });
    }
}

// ============================================================
// QUICK QUEST OVERVIEW
// ============================================================

pub fn show_quick_overview(ui: &mut egui::Ui, editor: &QuestEditor) {
    if let Some(sel) = editor.selected {
        if let Some(quest) = editor.quests.get(sel) {
            ui.horizontal(|ui| {
                let level_color = Color32::from_rgb(200, 180, 80);
                ui.label(RichText::new(format!("Lv.{}", quest.recommended_level)).color(level_color).strong().small());
                if quest.repeatable { ui.label(RichText::new("[Repeatable]").small().color(Color32::from_rgb(100, 200, 200))); }
                if quest.auto_complete { ui.label(RichText::new("[Auto]").small().color(Color32::from_rgb(200, 200, 100))); }
                if quest.hidden { ui.label(RichText::new("[Hidden]").small().color(Color32::from_rgb(180, 100, 180))); }
                if quest.time_limit.is_some() { ui.label(RichText::new("[Timed]").small().color(Color32::from_rgb(255, 140, 50))); }
            });
            draw_objective_progress(ui, &quest.objectives);
        }
    }
}

// ============================================================
// LORE ENTRY VIEWER
// ============================================================

pub fn show_lore_entry_viewer(ui: &mut egui::Ui, lore: &mut Vec<String>) {
    egui::CollapsingHeader::new(RichText::new("Lore Entries").color(Color32::from_rgb(180, 160, 220)))
        .default_open(false)
        .show(ui, |ui| {
            let mut to_remove = None;
            for (i, entry) in lore.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}.", i + 1)).small().color(Color32::GRAY));
                    ui.add(egui::TextEdit::multiline(entry).desired_rows(2).desired_width(f32::INFINITY));
                    if ui.small_button("X").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(idx) = to_remove { lore.remove(idx); }
            if ui.small_button("+ Add Lore").clicked() { lore.push("New lore entry.".to_string()); }
        });
}
