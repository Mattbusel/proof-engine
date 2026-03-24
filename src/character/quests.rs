// src/character/quests.rs
// Quest system, journal, achievements, procedural quests.

use std::collections::{HashMap, HashSet};
use crate::character::inventory::Item;
use crate::character::skills::SkillId;

// ---------------------------------------------------------------------------
// QuestId / ItemId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuestId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AchievementId(pub u64);

// ---------------------------------------------------------------------------
// QuestState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestState {
    Available,
    Active,
    Completed,
    Failed,
    Abandoned,
}

// ---------------------------------------------------------------------------
// ObjectiveKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ObjectiveKind {
    Kill { enemy_type: String, count: u32 },
    Collect { item_id: ItemId, count: u32 },
    Talk { npc_id: u64 },
    Reach { location_name: String, x: f32, y: f32, z: f32, radius: f32 },
    Survive { duration_secs: f32 },
    Escort { npc_id: u64 },
    Craft { item_id: ItemId, count: u32 },
    UseSkill { skill_id: SkillId, count: u32 },
    Explore { zone_name: String },
    Protect { target_id: u64, duration_secs: f32 },
    Deliver { item_id: ItemId, npc_id: u64 },
    Defeat { boss_id: u64 },
    Custom { description: String, required: u32 },
}

impl ObjectiveKind {
    pub fn required(&self) -> u32 {
        match self {
            ObjectiveKind::Kill { count, .. } => *count,
            ObjectiveKind::Collect { count, .. } => *count,
            ObjectiveKind::Talk { .. } => 1,
            ObjectiveKind::Reach { .. } => 1,
            ObjectiveKind::Survive { duration_secs } => *duration_secs as u32,
            ObjectiveKind::Escort { .. } => 1,
            ObjectiveKind::Craft { count, .. } => *count,
            ObjectiveKind::UseSkill { count, .. } => *count,
            ObjectiveKind::Explore { .. } => 1,
            ObjectiveKind::Protect { duration_secs, .. } => *duration_secs as u32,
            ObjectiveKind::Deliver { .. } => 1,
            ObjectiveKind::Defeat { .. } => 1,
            ObjectiveKind::Custom { required, .. } => *required,
        }
    }
}

// ---------------------------------------------------------------------------
// QuestObjective
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuestObjective {
    pub description: String,
    pub kind: ObjectiveKind,
    pub progress: u32,
    pub required: u32,
    pub optional: bool,
    pub hidden: bool, // revealed only when triggered
}

impl QuestObjective {
    pub fn new(description: impl Into<String>, kind: ObjectiveKind) -> Self {
        let required = kind.required();
        Self {
            description: description.into(),
            required,
            kind,
            progress: 0,
            optional: false,
            hidden: false,
        }
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    pub fn is_complete(&self) -> bool {
        self.progress >= self.required
    }

    pub fn advance(&mut self, amount: u32) -> bool {
        if self.is_complete() { return false; }
        self.progress = (self.progress + amount).min(self.required);
        self.is_complete()
    }

    pub fn fraction(&self) -> f32 {
        if self.required == 0 { return 1.0; }
        self.progress as f32 / self.required as f32
    }
}

// ---------------------------------------------------------------------------
// QuestReward
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuestReward {
    pub xp: u64,
    pub gold: u64,
    pub items: Vec<(Item, u32)>,
    pub skills: Vec<SkillId>,
    pub reputation: Vec<(String, i32)>,
    pub title: Option<String>,
    pub stat_points: u32,
    pub skill_points: u32,
}

impl QuestReward {
    pub fn new(xp: u64, gold: u64) -> Self {
        Self {
            xp,
            gold,
            items: Vec::new(),
            skills: Vec::new(),
            reputation: Vec::new(),
            title: None,
            stat_points: 0,
            skill_points: 0,
        }
    }

    pub fn add_item(mut self, item: Item, count: u32) -> Self {
        self.items.push((item, count));
        self
    }

    pub fn add_skill(mut self, skill_id: SkillId) -> Self {
        self.skills.push(skill_id);
        self
    }

    pub fn add_rep(mut self, faction: impl Into<String>, amount: i32) -> Self {
        self.reputation.push((faction.into(), amount));
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

impl Default for QuestReward {
    fn default() -> Self {
        Self::new(100, 50)
    }
}

// ---------------------------------------------------------------------------
// Quest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Quest {
    pub id: QuestId,
    pub name: String,
    pub description: String,
    pub giver_id: Option<u64>,
    pub state: QuestState,
    pub objectives: Vec<QuestObjective>,
    pub reward: QuestReward,
    pub level_requirement: u32,
    pub chain_id: Option<u64>,
    pub chain_position: u32,
    pub time_limit_secs: Option<f32>,
    pub time_elapsed: f32,
    pub repeatable: bool,
    pub times_completed: u32,
    pub category: QuestCategory,
    pub priority: QuestPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestCategory {
    MainStory,
    SideQuest,
    Daily,
    Weekly,
    Guild,
    Bounty,
    Exploration,
    Crafting,
    Escort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuestPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Quest {
    pub fn new(id: QuestId, name: impl Into<String>, reward: QuestReward) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            giver_id: None,
            state: QuestState::Available,
            objectives: Vec::new(),
            reward,
            level_requirement: 1,
            chain_id: None,
            chain_position: 0,
            time_limit_secs: None,
            time_elapsed: 0.0,
            repeatable: false,
            times_completed: 0,
            category: QuestCategory::SideQuest,
            priority: QuestPriority::Normal,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_giver(mut self, npc_id: u64) -> Self {
        self.giver_id = Some(npc_id);
        self
    }

    pub fn add_objective(mut self, obj: QuestObjective) -> Self {
        self.objectives.push(obj);
        self
    }

    pub fn with_level_req(mut self, level: u32) -> Self {
        self.level_requirement = level;
        self
    }

    pub fn with_time_limit(mut self, secs: f32) -> Self {
        self.time_limit_secs = Some(secs);
        self
    }

    pub fn repeatable(mut self) -> Self {
        self.repeatable = true;
        self
    }

    pub fn with_category(mut self, cat: QuestCategory) -> Self {
        self.category = cat;
        self
    }

    pub fn with_priority(mut self, p: QuestPriority) -> Self {
        self.priority = p;
        self
    }

    pub fn activate(&mut self) {
        self.state = QuestState::Active;
        self.time_elapsed = 0.0;
    }

    pub fn all_objectives_complete(&self) -> bool {
        self.objectives.iter()
            .filter(|o| !o.optional)
            .all(|o| o.is_complete())
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        if self.state != QuestState::Active { return false; }
        self.time_elapsed += dt;
        if let Some(limit) = self.time_limit_secs {
            if self.time_elapsed >= limit {
                self.state = QuestState::Failed;
                return true; // Signal: quest expired
            }
        }
        false
    }

    pub fn time_remaining(&self) -> Option<f32> {
        self.time_limit_secs.map(|l| (l - self.time_elapsed).max(0.0))
    }

    pub fn update_objective(&mut self, obj_idx: usize, delta: u32) -> bool {
        if let Some(obj) = self.objectives.get_mut(obj_idx) {
            let completed = obj.advance(delta);
            if self.all_objectives_complete() {
                self.state = QuestState::Completed;
                self.times_completed += 1;
                return true; // Quest completed!
            }
            return completed;
        }
        false
    }

    pub fn is_active(&self) -> bool {
        self.state == QuestState::Active
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, QuestState::Completed | QuestState::Failed | QuestState::Abandoned)
    }
}

// ---------------------------------------------------------------------------
// QuestJournal — the player's quest log (max 25 active)
// ---------------------------------------------------------------------------

pub const MAX_ACTIVE_QUESTS: usize = 25;

#[derive(Debug, Clone, Default)]
pub struct QuestJournal {
    pub active: HashMap<QuestId, Quest>,
    pub completed: Vec<Quest>,
    pub failed: Vec<Quest>,
}

impl QuestJournal {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            completed: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub fn can_accept(&self) -> bool {
        self.active.len() < MAX_ACTIVE_QUESTS
    }

    pub fn add_quest(&mut self, mut quest: Quest) -> bool {
        if self.active.len() >= MAX_ACTIVE_QUESTS { return false; }
        if self.active.contains_key(&quest.id) { return false; }
        quest.activate();
        self.active.insert(quest.id, quest);
        true
    }

    pub fn complete_quest(&mut self, id: QuestId) -> Option<Quest> {
        let mut quest = self.active.remove(&id)?;
        quest.state = QuestState::Completed;
        quest.times_completed += 1;
        self.completed.push(quest.clone());
        Some(quest)
    }

    pub fn fail_quest(&mut self, id: QuestId) -> Option<Quest> {
        let mut quest = self.active.remove(&id)?;
        quest.state = QuestState::Failed;
        self.failed.push(quest.clone());
        Some(quest)
    }

    pub fn abandon_quest(&mut self, id: QuestId) -> Option<Quest> {
        let mut quest = self.active.remove(&id)?;
        quest.state = QuestState::Abandoned;
        Some(quest)
    }

    pub fn update_objective(&mut self, quest_id: QuestId, obj_idx: usize, delta: u32) -> Option<bool> {
        let quest = self.active.get_mut(&quest_id)?;
        let newly_done = quest.update_objective(obj_idx, delta);
        // If quest got auto-completed, move it
        let completed = quest.state == QuestState::Completed;
        Some(newly_done || completed)
    }

    pub fn check_completion(&mut self, quest_id: QuestId) -> bool {
        let quest = match self.active.get(&quest_id) {
            Some(q) => q,
            None => return false,
        };
        if quest.all_objectives_complete() {
            let id = quest.id;
            self.complete_quest(id);
            return true;
        }
        false
    }

    pub fn tick(&mut self, dt: f32) -> Vec<QuestId> {
        let mut failed = Vec::new();
        for quest in self.active.values_mut() {
            if quest.tick(dt) {
                failed.push(quest.id);
            }
        }
        for id in &failed {
            self.fail_quest(*id);
        }
        failed
    }

    pub fn update_kill_objectives(&mut self, enemy_type: &str) -> Vec<(QuestId, usize)> {
        let mut updates = Vec::new();
        for quest in self.active.values_mut() {
            for (obj_idx, obj) in quest.objectives.iter_mut().enumerate() {
                if let ObjectiveKind::Kill { enemy_type: et, .. } = &obj.kind {
                    if et == enemy_type && !obj.is_complete() {
                        obj.advance(1);
                        updates.push((quest.id, obj_idx));
                    }
                }
            }
        }
        updates
    }

    pub fn update_collect_objectives(&mut self, item_id: ItemId, count: u32) -> Vec<(QuestId, usize)> {
        let mut updates = Vec::new();
        for quest in self.active.values_mut() {
            for (obj_idx, obj) in quest.objectives.iter_mut().enumerate() {
                if let ObjectiveKind::Collect { item_id: iid, .. } = &obj.kind {
                    if *iid == item_id && !obj.is_complete() {
                        obj.advance(count);
                        updates.push((quest.id, obj_idx));
                    }
                }
            }
        }
        updates
    }

    pub fn has_completed(&self, id: QuestId) -> bool {
        self.completed.iter().any(|q| q.id == id)
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn get_active(&self, id: QuestId) -> Option<&Quest> {
        self.active.get(&id)
    }

    pub fn all_active_sorted(&self) -> Vec<&Quest> {
        let mut quests: Vec<&Quest> = self.active.values().collect();
        quests.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.name.cmp(&b.name)));
        quests
    }
}

// ---------------------------------------------------------------------------
// QuestChain — sequential quest series
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuestChain {
    pub id: u64,
    pub name: String,
    pub quests: Vec<QuestId>,
    pub auto_advance: bool,
    pub current_index: usize,
}

impl QuestChain {
    pub fn new(id: u64, name: impl Into<String>, quests: Vec<QuestId>, auto_advance: bool) -> Self {
        Self { id, name: name.into(), quests, auto_advance, current_index: 0 }
    }

    pub fn current_quest(&self) -> Option<QuestId> {
        self.quests.get(self.current_index).copied()
    }

    pub fn advance(&mut self) -> Option<QuestId> {
        if self.current_index + 1 < self.quests.len() {
            self.current_index += 1;
            self.current_quest()
        } else {
            None
        }
    }

    pub fn is_complete(&self) -> bool {
        self.current_index >= self.quests.len()
    }

    pub fn progress_fraction(&self) -> f32 {
        if self.quests.is_empty() { return 1.0; }
        self.current_index as f32 / self.quests.len() as f32
    }
}

// ---------------------------------------------------------------------------
// QuestTrigger — conditions that make quests available
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum QuestTrigger {
    LevelReached(u32),
    QuestCompleted(QuestId),
    ItemOwned(ItemId),
    FactionRep { faction: String, min_rep: i32 },
    TimeElapsed(f64),
    TalkToNpc(u64),
    EnterZone(String),
    AchievementUnlocked(AchievementId),
    Always,
}

impl QuestTrigger {
    pub fn check_level(&self, player_level: u32) -> bool {
        match self {
            QuestTrigger::LevelReached(req) => player_level >= *req,
            QuestTrigger::Always => true,
            _ => false,
        }
    }

    pub fn check_quest_complete(&self, journal: &QuestJournal) -> bool {
        match self {
            QuestTrigger::QuestCompleted(id) => journal.has_completed(*id),
            QuestTrigger::Always => true,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// QuestBoard — dynamic board of available quests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuestBoardEntry {
    pub quest: Quest,
    pub trigger: QuestTrigger,
    pub expires_at: Option<f64>,
    pub posted: bool,
}

impl QuestBoardEntry {
    pub fn new(quest: Quest, trigger: QuestTrigger) -> Self {
        Self { quest, trigger, expires_at: None, posted: true }
    }

    pub fn with_expiry(mut self, time: f64) -> Self {
        self.expires_at = Some(time);
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct QuestBoard {
    pub entries: Vec<QuestBoardEntry>,
    pub current_time: f64,
}

impl QuestBoard {
    pub fn new() -> Self {
        Self { entries: Vec::new(), current_time: 0.0 }
    }

    pub fn post(&mut self, entry: QuestBoardEntry) {
        self.entries.push(entry);
    }

    pub fn tick(&mut self, dt: f64) {
        self.current_time += dt;
        self.entries.retain(|e| {
            e.expires_at.map(|exp| self.current_time < exp).unwrap_or(true)
        });
    }

    pub fn available_for_level(&self, level: u32) -> Vec<&Quest> {
        self.entries.iter()
            .filter(|e| e.posted && e.quest.level_requirement <= level)
            .map(|e| &e.quest)
            .collect()
    }

    pub fn remove_quest(&mut self, id: QuestId) -> Option<Quest> {
        if let Some(pos) = self.entries.iter().position(|e| e.quest.id == id) {
            Some(self.entries.remove(pos).quest)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// QuestGenerator — procedural quest generation
// ---------------------------------------------------------------------------

static ENEMY_TYPES: &[&str] = &["goblin", "skeleton", "wolf", "bandit", "orc", "vampire", "zombie", "drake", "giant_spider", "troll"];
static LOCATIONS: &[&str] = &["Dark Forest", "Abandoned Mine", "Cursed Ruins", "Flooded Caves", "Mountain Peak", "Shadow Swamp", "Haunted Tower"];
static NPC_NAMES: &[&str] = &["Aldric", "Theron", "Lyra", "Sable", "Mordecai", "Veran", "Kessa", "Torvin", "Aelys", "Bramwell"];
static QUEST_TEMPLATES_KILL: &[&str] = &[
    "Thin the Herd", "Extermination", "Clear the Path", "Bounty: {enemy}",
    "Defend the Village", "Purge the {enemy}s",
];
static QUEST_TEMPLATES_COLLECT: &[&str] = &[
    "Resource Gathering", "Supply Run", "The Missing Shipment", "Reagent Collection",
];

pub struct QuestGenerator {
    next_id: u64,
    seed: u64,
}

impl QuestGenerator {
    pub fn new(seed: u64) -> Self {
        Self { next_id: 10000, seed }
    }

    fn next_rand(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    fn rand_range(&mut self, min: u64, max: u64) -> u64 {
        if max <= min { return min; }
        min + self.next_rand() % (max - min)
    }

    fn next_id(&mut self) -> QuestId {
        let id = QuestId(self.next_id);
        self.next_id += 1;
        id
    }

    fn pick<T>(&mut self, slice: &[T]) -> usize {
        self.next_rand() as usize % slice.len()
    }

    fn make_name(&mut self, template: &str, enemy: &str) -> String {
        template.replace("{enemy}", enemy)
    }

    pub fn generate_kill_quest(&mut self, player_level: u32) -> Quest {
        let enemy_idx = self.pick(ENEMY_TYPES);
        let enemy = ENEMY_TYPES[enemy_idx];
        let count = self.rand_range(3, 15 + player_level as u64) as u32;
        let tmpl_idx = self.pick(QUEST_TEMPLATES_KILL);
        let name = self.make_name(QUEST_TEMPLATES_KILL[tmpl_idx], enemy);
        let xp = (count as u64 * 20 + player_level as u64 * 50).max(100);
        let gold = (count as u64 * 5 + player_level as u64 * 10).max(20);
        let id = self.next_id();
        let desc = format!(
            "Kill {} {}{}. They have been terrorizing the region.",
            count,
            enemy,
            if count > 1 { "s" } else { "" }
        );
        Quest::new(id, name, QuestReward::new(xp, gold))
            .with_description(desc)
            .add_objective(QuestObjective::new(
                format!("Kill {count} {enemy}s"),
                ObjectiveKind::Kill { enemy_type: enemy.to_string(), count },
            ))
            .with_level_req(player_level.saturating_sub(2))
            .with_category(QuestCategory::Bounty)
    }

    pub fn generate_collect_quest(&mut self, player_level: u32) -> Quest {
        let count = self.rand_range(3, 10 + player_level as u64 / 2) as u32;
        let item_id = ItemId(self.rand_range(1000, 2000));
        let tmpl_idx = self.pick(QUEST_TEMPLATES_COLLECT);
        let name = QUEST_TEMPLATES_COLLECT[tmpl_idx].to_string();
        let xp = (count as u64 * 15 + player_level as u64 * 30).max(80);
        let gold = (count as u64 * 8 + player_level as u64 * 8).max(15);
        let id = self.next_id();
        Quest::new(id, name, QuestReward::new(xp, gold))
            .with_description(format!("Collect {} rare materials for the crafters guild.", count))
            .add_objective(QuestObjective::new(
                format!("Collect {count} materials"),
                ObjectiveKind::Collect { item_id, count },
            ))
            .with_level_req(player_level.saturating_sub(2))
            .with_category(QuestCategory::Crafting)
    }

    pub fn generate_escort_quest(&mut self, player_level: u32) -> Quest {
        let npc_idx = self.pick(NPC_NAMES);
        let npc_name = NPC_NAMES[npc_idx];
        let npc_id = self.rand_range(100, 500);
        let loc_idx = self.pick(LOCATIONS);
        let loc = LOCATIONS[loc_idx];
        let xp = (player_level as u64 * 80 + 200).max(300);
        let gold = (player_level as u64 * 20 + 100).max(100);
        let id = self.next_id();
        Quest::new(id, format!("Escort {npc_name} to Safety"), QuestReward::new(xp, gold))
            .with_description(format!("Escort {} safely to {}.", npc_name, loc))
            .add_objective(QuestObjective::new(
                format!("Escort {npc_name}"),
                ObjectiveKind::Escort { npc_id },
            ))
            .add_objective(QuestObjective::new(
                format!("Reach {loc}"),
                ObjectiveKind::Reach { location_name: loc.to_string(), x: 0.0, y: 0.0, z: 0.0, radius: 5.0 },
            ))
            .with_level_req(player_level.saturating_sub(3))
            .with_category(QuestCategory::Escort)
    }

    pub fn generate_explore_quest(&mut self, player_level: u32) -> Quest {
        let loc_idx = self.pick(LOCATIONS);
        let loc = LOCATIONS[loc_idx];
        let xp = (player_level as u64 * 60 + 150).max(200);
        let gold = (player_level as u64 * 15 + 50).max(50);
        let id = self.next_id();
        Quest::new(id, format!("Explore: {loc}"), QuestReward::new(xp, gold))
            .with_description(format!("Survey the {} area and report back.", loc))
            .add_objective(QuestObjective::new(
                format!("Explore {loc}"),
                ObjectiveKind::Explore { zone_name: loc.to_string() },
            ))
            .with_level_req(player_level.saturating_sub(1))
            .with_category(QuestCategory::Exploration)
    }

    pub fn generate_daily_quests(&mut self, player_level: u32, count: usize) -> Vec<Quest> {
        let mut quests = Vec::new();
        for i in 0..count {
            let quest = match i % 4 {
                0 => self.generate_kill_quest(player_level),
                1 => self.generate_collect_quest(player_level),
                2 => self.generate_escort_quest(player_level),
                _ => self.generate_explore_quest(player_level),
            };
            quests.push(quest);
        }
        quests
    }
}

// ---------------------------------------------------------------------------
// DialogueQuestIntegration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DialogueChoice {
    pub text: String,
    pub gives_quest: Option<QuestId>,
    pub requires_quest_completed: Option<QuestId>,
    pub requires_item: Option<ItemId>,
    pub requires_level: u32,
    pub leads_to_node: Option<usize>,
}

impl DialogueChoice {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            gives_quest: None,
            requires_quest_completed: None,
            requires_item: None,
            requires_level: 0,
            leads_to_node: None,
        }
    }

    pub fn gives_quest(mut self, id: QuestId) -> Self {
        self.gives_quest = Some(id);
        self
    }

    pub fn requires_level(mut self, level: u32) -> Self {
        self.requires_level = level;
        self
    }

    pub fn is_available(&self, player_level: u32, journal: &QuestJournal) -> bool {
        if player_level < self.requires_level { return false; }
        if let Some(id) = self.requires_quest_completed {
            if !journal.has_completed(id) { return false; }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct DialogueNode {
    pub npc_text: String,
    pub choices: Vec<DialogueChoice>,
}

impl DialogueNode {
    pub fn new(npc_text: impl Into<String>) -> Self {
        Self { npc_text: npc_text.into(), choices: Vec::new() }
    }

    pub fn add_choice(mut self, choice: DialogueChoice) -> Self {
        self.choices.push(choice);
        self
    }
}

#[derive(Debug, Clone)]
pub struct DialogueTree {
    pub npc_id: u64,
    pub npc_name: String,
    pub nodes: Vec<DialogueNode>,
    pub root_node: usize,
}

impl DialogueTree {
    pub fn new(npc_id: u64, npc_name: impl Into<String>) -> Self {
        Self { npc_id, npc_name: npc_name.into(), nodes: Vec::new(), root_node: 0 }
    }

    pub fn add_node(mut self, node: DialogueNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn get_root(&self) -> Option<&DialogueNode> {
        self.nodes.get(self.root_node)
    }

    pub fn get_node(&self, idx: usize) -> Option<&DialogueNode> {
        self.nodes.get(idx)
    }

    pub fn available_choices(&self, node_idx: usize, level: u32, journal: &QuestJournal) -> Vec<(usize, &DialogueChoice)> {
        self.nodes.get(node_idx)
            .map(|n| {
                n.choices.iter().enumerate()
                    .filter(|(_, c)| c.is_available(level, journal))
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// QuestTracker — minimal HUD data for objectives
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TrackerObjective {
    pub quest_name: String,
    pub description: String,
    pub progress: u32,
    pub required: u32,
}

impl TrackerObjective {
    pub fn fraction(&self) -> f32 {
        if self.required == 0 { return 1.0; }
        self.progress as f32 / self.required as f32
    }
}

#[derive(Debug, Clone, Default)]
pub struct QuestTracker {
    pub tracked: Vec<(QuestId, usize)>, // (quest_id, obj_idx)
    pub max_tracked: usize,
}

impl QuestTracker {
    pub fn new(max: usize) -> Self {
        Self { tracked: Vec::new(), max_tracked: max }
    }

    pub fn track(&mut self, quest_id: QuestId, obj_idx: usize) -> bool {
        if self.tracked.len() >= self.max_tracked { return false; }
        if self.tracked.contains(&(quest_id, obj_idx)) { return false; }
        self.tracked.push((quest_id, obj_idx));
        true
    }

    pub fn untrack(&mut self, quest_id: QuestId, obj_idx: usize) {
        self.tracked.retain(|&(qid, oi)| !(qid == quest_id && oi == obj_idx));
    }

    pub fn get_display(&self, journal: &QuestJournal) -> Vec<TrackerObjective> {
        self.tracked.iter().filter_map(|&(qid, oi)| {
            let quest = journal.get_active(qid)?;
            let obj = quest.objectives.get(oi)?;
            Some(TrackerObjective {
                quest_name: quest.name.clone(),
                description: obj.description.clone(),
                progress: obj.progress,
                required: obj.required,
            })
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// AchievementSystem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AchievementCategory {
    Combat,
    Exploration,
    Crafting,
    Social,
    Collection,
    Progression,
    Secret,
    Event,
}

#[derive(Debug, Clone)]
pub struct Achievement {
    pub id: AchievementId,
    pub name: String,
    pub description: String,
    pub icon: char,
    pub points: u32,
    pub secret: bool,
    pub category: AchievementCategory,
    pub trigger: AchievementTrigger,
    pub reward: Option<AchievementReward>,
}

impl Achievement {
    pub fn new(id: AchievementId, name: impl Into<String>, category: AchievementCategory, trigger: AchievementTrigger) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            icon: '★',
            points: 10,
            secret: false,
            category,
            trigger,
            reward: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_points(mut self, pts: u32) -> Self {
        self.points = pts;
        self
    }

    pub fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    pub fn with_reward(mut self, reward: AchievementReward) -> Self {
        self.reward = Some(reward);
        self
    }
}

#[derive(Debug, Clone)]
pub enum AchievementTrigger {
    LevelReached(u32),
    QuestCompleted(QuestId),
    KillCount { enemy_type: String, count: u64 },
    TotalKills(u64),
    ItemCollected { item_id: ItemId },
    GoldAccumulated(u64),
    SkillRankMaxed(SkillId),
    QuestsCompleted(u32),
    AchievementsUnlocked(u32),
    DeathCount(u32),
    Manual, // triggered from code
}

impl AchievementTrigger {
    pub fn check_level(&self, level: u32) -> bool {
        matches!(self, AchievementTrigger::LevelReached(req) if level >= *req)
    }

    pub fn check_kill_count(&self, enemy_type: &str, count: u64) -> bool {
        match self {
            AchievementTrigger::KillCount { enemy_type: et, count: req } => {
                et == enemy_type && count >= *req
            }
            AchievementTrigger::TotalKills(req) => count >= *req,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AchievementReward {
    pub xp: u64,
    pub title: Option<String>,
    pub cosmetic: Option<String>,
}

impl AchievementReward {
    pub fn new(xp: u64) -> Self {
        Self { xp, title: None, cosmetic: None }
    }
    pub fn with_title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct AchievementProgress {
    pub achievement_id: AchievementId,
    pub current: u64,
    pub required: u64,
}

impl AchievementProgress {
    pub fn fraction(&self) -> f32 {
        if self.required == 0 { return 1.0; }
        (self.current as f32 / self.required as f32).min(1.0)
    }

    pub fn is_complete(&self) -> bool {
        self.current >= self.required
    }
}

#[derive(Debug, Clone, Default)]
pub struct AchievementSystem {
    pub achievements: Vec<Achievement>,
    pub unlocked: HashSet<AchievementId>,
    pub progress: HashMap<AchievementId, AchievementProgress>,
    pub total_points: u32,
    pub kill_counts: HashMap<String, u64>,
    pub total_kills: u64,
    pub quests_completed: u32,
    pub gold_accumulated: u64,
}

impl AchievementSystem {
    pub fn new() -> Self {
        let mut sys = Self::default();
        sys.register_defaults();
        sys
    }

    fn register_defaults(&mut self) {
        let defaults = vec![
            Achievement::new(
                AchievementId(1), "First Blood", AchievementCategory::Combat,
                AchievementTrigger::TotalKills(1),
            ).with_description("Get your first kill.").with_points(5),

            Achievement::new(
                AchievementId(2), "Slayer", AchievementCategory::Combat,
                AchievementTrigger::TotalKills(100),
            ).with_description("Kill 100 enemies.").with_points(20),

            Achievement::new(
                AchievementId(3), "Centurion", AchievementCategory::Combat,
                AchievementTrigger::TotalKills(1000),
            ).with_description("Kill 1000 enemies.").with_points(50),

            Achievement::new(
                AchievementId(4), "Goblin Slayer", AchievementCategory::Combat,
                AchievementTrigger::KillCount { enemy_type: "goblin".to_string(), count: 50 },
            ).with_description("Kill 50 goblins.").with_points(15),

            Achievement::new(
                AchievementId(5), "Quest Beginner", AchievementCategory::Progression,
                AchievementTrigger::QuestsCompleted(1),
            ).with_description("Complete your first quest.").with_points(10),

            Achievement::new(
                AchievementId(6), "Adventurer", AchievementCategory::Progression,
                AchievementTrigger::QuestsCompleted(25),
            ).with_description("Complete 25 quests.").with_points(25),

            Achievement::new(
                AchievementId(7), "Veteran", AchievementCategory::Progression,
                AchievementTrigger::QuestsCompleted(100),
            ).with_description("Complete 100 quests.").with_points(75),

            Achievement::new(
                AchievementId(8), "Level 10", AchievementCategory::Progression,
                AchievementTrigger::LevelReached(10),
            ).with_description("Reach level 10.").with_points(10),

            Achievement::new(
                AchievementId(9), "Level 50", AchievementCategory::Progression,
                AchievementTrigger::LevelReached(50),
            ).with_description("Reach level 50.").with_points(50),

            Achievement::new(
                AchievementId(10), "Max Level", AchievementCategory::Progression,
                AchievementTrigger::LevelReached(100),
            ).with_description("Reach the maximum level.").with_points(100)
                .with_reward(AchievementReward::new(10000).with_title("The Ascended")),

            Achievement::new(
                AchievementId(11), "Wealthy", AchievementCategory::Collection,
                AchievementTrigger::GoldAccumulated(10000),
            ).with_description("Accumulate 10,000 gold.").with_points(20),

            Achievement::new(
                AchievementId(12), "Secret: The Unkillable", AchievementCategory::Secret,
                AchievementTrigger::DeathCount(0),
            ).with_description("Never die. Ever.").with_points(500).secret(),
        ];
        for ach in defaults {
            self.register(ach);
        }
    }

    pub fn register(&mut self, achievement: Achievement) {
        self.achievements.push(achievement);
    }

    pub fn is_unlocked(&self, id: AchievementId) -> bool {
        self.unlocked.contains(&id)
    }

    pub fn unlock(&mut self, id: AchievementId) -> bool {
        if self.unlocked.contains(&id) { return false; }
        if let Some(ach) = self.achievements.iter().find(|a| a.id == id) {
            self.total_points += ach.points;
            self.unlocked.insert(id);
            return true;
        }
        false
    }

    pub fn record_kill(&mut self, enemy_type: &str) -> Vec<AchievementId> {
        *self.kill_counts.entry(enemy_type.to_string()).or_insert(0) += 1;
        self.total_kills += 1;
        self.check_all()
    }

    pub fn record_quest_complete(&mut self) -> Vec<AchievementId> {
        self.quests_completed += 1;
        self.check_all()
    }

    pub fn record_gold(&mut self, amount: u64) -> Vec<AchievementId> {
        self.gold_accumulated += amount;
        self.check_all()
    }

    pub fn check_level(&mut self, level: u32) -> Vec<AchievementId> {
        let ids: Vec<AchievementId> = self.achievements.iter()
            .filter(|a| !self.unlocked.contains(&a.id) && a.trigger.check_level(level))
            .map(|a| a.id)
            .collect();
        let mut newly_unlocked = Vec::new();
        for id in ids {
            if self.unlock(id) { newly_unlocked.push(id); }
        }
        newly_unlocked
    }

    pub fn manual_unlock(&mut self, id: AchievementId) -> bool {
        self.unlock(id)
    }

    fn check_all(&mut self) -> Vec<AchievementId> {
        let total_kills = self.total_kills;
        let kill_counts = self.kill_counts.clone();
        let quests = self.quests_completed;
        let gold = self.gold_accumulated;

        let ids: Vec<AchievementId> = self.achievements.iter()
            .filter(|a| !self.unlocked.contains(&a.id))
            .filter(|a| match &a.trigger {
                AchievementTrigger::TotalKills(req) => total_kills >= *req,
                AchievementTrigger::KillCount { enemy_type, count } => {
                    kill_counts.get(enemy_type.as_str()).copied().unwrap_or(0) >= *count
                }
                AchievementTrigger::QuestsCompleted(req) => quests >= *req,
                AchievementTrigger::GoldAccumulated(req) => gold >= *req,
                _ => false,
            })
            .map(|a| a.id)
            .collect();

        let mut newly_unlocked = Vec::new();
        for id in ids {
            if self.unlock(id) { newly_unlocked.push(id); }
        }
        newly_unlocked
    }

    pub fn unlocked_count(&self) -> usize {
        self.unlocked.len()
    }

    pub fn total_achievement_count(&self) -> usize {
        self.achievements.len()
    }

    pub fn completion_fraction(&self) -> f32 {
        if self.achievements.is_empty() { return 0.0; }
        self.unlocked.len() as f32 / self.achievements.len() as f32
    }

    pub fn by_category(&self, cat: AchievementCategory) -> Vec<&Achievement> {
        self.achievements.iter()
            .filter(|a| a.category == cat)
            .collect()
    }

    pub fn recently_unlocked(&self, count: usize) -> Vec<&Achievement> {
        // Returns last N unlocked (order of unlock is not tracked precisely here;
        // we return achievements whose ids are in unlocked, by order in the list)
        self.achievements.iter()
            .filter(|a| self.unlocked.contains(&a.id))
            .rev()
            .take(count)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_quest(id: u64, enemy: &str, count: u32) -> Quest {
        Quest::new(QuestId(id), format!("Kill {enemy}"), QuestReward::new(100, 50))
            .add_objective(QuestObjective::new(
                format!("Kill {count} {enemy}s"),
                ObjectiveKind::Kill { enemy_type: enemy.to_string(), count },
            ))
    }

    #[test]
    fn test_quest_objective_advance() {
        let mut obj = QuestObjective::new("Kill 5 goblins", ObjectiveKind::Kill { enemy_type: "goblin".to_string(), count: 5 });
        assert!(!obj.is_complete());
        obj.advance(3);
        assert!(!obj.is_complete());
        obj.advance(2);
        assert!(obj.is_complete());
    }

    #[test]
    fn test_quest_auto_complete() {
        let mut quest = simple_quest(1, "goblin", 3);
        quest.activate();
        quest.update_objective(0, 3);
        assert_eq!(quest.state, QuestState::Completed);
    }

    #[test]
    fn test_quest_journal_add_and_complete() {
        let mut journal = QuestJournal::new();
        let q = simple_quest(1, "wolf", 2);
        assert!(journal.add_quest(q));
        assert_eq!(journal.active_count(), 1);
        let done = journal.complete_quest(QuestId(1));
        assert!(done.is_some());
        assert_eq!(journal.active_count(), 0);
        assert!(journal.has_completed(QuestId(1)));
    }

    #[test]
    fn test_quest_journal_fail() {
        let mut journal = QuestJournal::new();
        let q = simple_quest(2, "orc", 5);
        journal.add_quest(q);
        let failed = journal.fail_quest(QuestId(2));
        assert!(failed.is_some());
    }

    #[test]
    fn test_quest_journal_max_active() {
        let mut journal = QuestJournal::new();
        for i in 0..MAX_ACTIVE_QUESTS {
            let q = simple_quest(i as u64, "goblin", 1);
            journal.add_quest(q);
        }
        let overflow = simple_quest(999, "goblin", 1);
        assert!(!journal.add_quest(overflow));
    }

    #[test]
    fn test_quest_kill_objective_tracking() {
        let mut journal = QuestJournal::new();
        let q = simple_quest(1, "goblin", 5);
        journal.add_quest(q);
        let updates = journal.update_kill_objectives("goblin");
        assert!(!updates.is_empty());
    }

    #[test]
    fn test_quest_time_limit_expiry() {
        let mut quest = Quest::new(QuestId(1), "Timed", QuestReward::default())
            .with_time_limit(5.0);
        quest.activate();
        let expired = quest.tick(6.0);
        assert!(expired);
        assert_eq!(quest.state, QuestState::Failed);
    }

    #[test]
    fn test_quest_generator_kill() {
        let mut gen = QuestGenerator::new(42);
        let q = gen.generate_kill_quest(10);
        assert!(!q.objectives.is_empty());
        assert!(matches!(q.objectives[0].kind, ObjectiveKind::Kill { .. }));
    }

    #[test]
    fn test_quest_generator_daily() {
        let mut gen = QuestGenerator::new(99);
        let quests = gen.generate_daily_quests(15, 8);
        assert_eq!(quests.len(), 8);
    }

    #[test]
    fn test_achievement_system_unlock() {
        let mut sys = AchievementSystem::new();
        // Record 100 kills
        for _ in 0..100 {
            sys.record_kill("anything");
        }
        assert!(sys.is_unlocked(AchievementId(2))); // Slayer: 100 kills
    }

    #[test]
    fn test_achievement_kill_count() {
        let mut sys = AchievementSystem::new();
        for _ in 0..50 {
            sys.record_kill("goblin");
        }
        assert!(sys.is_unlocked(AchievementId(4))); // Goblin Slayer
    }

    #[test]
    fn test_achievement_quest_completion() {
        let mut sys = AchievementSystem::new();
        sys.record_quest_complete();
        assert!(sys.is_unlocked(AchievementId(5))); // Quest Beginner
    }

    #[test]
    fn test_quest_chain_advance() {
        let mut chain = QuestChain::new(1, "Main Story",
            vec![QuestId(1), QuestId(2), QuestId(3)], true);
        assert_eq!(chain.current_quest(), Some(QuestId(1)));
        chain.advance();
        assert_eq!(chain.current_quest(), Some(QuestId(2)));
        chain.advance();
        chain.advance();
        assert!(chain.is_complete());
    }

    #[test]
    fn test_quest_board_expiry() {
        let mut board = QuestBoard::new();
        let q = simple_quest(1, "troll", 1);
        board.post(QuestBoardEntry::new(q, QuestTrigger::Always).with_expiry(5.0));
        board.tick(6.0);
        assert!(board.entries.is_empty());
    }
}
