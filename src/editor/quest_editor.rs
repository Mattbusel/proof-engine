#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// SECTION 1: QUEST GRAPH NODE TYPES
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum QuestNodeType {
    QuestStart,
    Objective,
    ConditionCheck,
    Reward,
    Branch,
    Fail,
    Completion,
    Timer,
    Trigger,
}

#[derive(Debug, Clone)]
pub struct QuestStartNode {
    pub id: u64,
    pub position: Vec2,
    pub quest_id: String,
    pub quest_name: String,
    pub description: String,
    pub quest_giver_id: String,
    pub prerequisites: Vec<QuestPrerequisite>,
    pub initial_journal_entry: String,
    pub start_map_markers: Vec<MapMarker>,
    pub on_start_events: Vec<String>,
    pub output_port: u64,
    pub tags: Vec<String>,
    pub comment: String,
    pub auto_start: bool,
    pub recommended_level: u32,
    pub difficulty_rating: f32,
}

impl QuestStartNode {
    pub fn new(id: u64) -> Self {
        QuestStartNode {
            id,
            position: Vec2::ZERO,
            quest_id: format!("quest_{}", id),
            quest_name: String::new(),
            description: String::new(),
            quest_giver_id: String::new(),
            prerequisites: Vec::new(),
            initial_journal_entry: String::new(),
            start_map_markers: Vec::new(),
            on_start_events: Vec::new(),
            output_port: 0,
            tags: Vec::new(),
            comment: String::new(),
            auto_start: false,
            recommended_level: 1,
            difficulty_rating: 1.0,
        }
    }

    pub fn check_prerequisites(&self, state: &QuestStateStore) -> bool {
        for prereq in &self.prerequisites {
            if !prereq.is_satisfied(state) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct QuestPrerequisite {
    pub prereq_type: PrerequisiteType,
    pub quest_id: Option<String>,
    pub level_required: Option<u32>,
    pub faction_id: Option<String>,
    pub faction_rep_required: Option<i32>,
    pub flag_name: Option<String>,
    pub negated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrerequisiteType {
    QuestCompleted,
    QuestActive,
    QuestNotStarted,
    PlayerLevel,
    FactionRep,
    Flag,
    ItemOwned,
}

impl QuestPrerequisite {
    pub fn is_satisfied(&self, state: &QuestStateStore) -> bool {
        let result = match &self.prereq_type {
            PrerequisiteType::QuestCompleted => {
                if let Some(qid) = &self.quest_id {
                    state.get_quest_state(qid) == QuestState::Completed
                } else { false }
            }
            PrerequisiteType::QuestActive => {
                if let Some(qid) = &self.quest_id {
                    state.get_quest_state(qid) == QuestState::Active
                } else { false }
            }
            PrerequisiteType::QuestNotStarted => {
                if let Some(qid) = &self.quest_id {
                    state.get_quest_state(qid) == QuestState::NotStarted
                } else { true }
            }
            PrerequisiteType::PlayerLevel => {
                if let Some(level) = self.level_required {
                    state.player_level >= level
                } else { true }
            }
            PrerequisiteType::FactionRep => {
                if let (Some(fid), Some(req)) = (&self.faction_id, &self.faction_rep_required) {
                    state.get_faction_rep(fid) >= *req
                } else { false }
            }
            PrerequisiteType::Flag => {
                if let Some(fname) = &self.flag_name {
                    state.get_flag(fname)
                } else { false }
            }
            PrerequisiteType::ItemOwned => {
                false // Would check inventory
            }
        };
        if self.negated { !result } else { result }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectiveNode {
    pub id: u64,
    pub position: Vec2,
    pub objective_id: String,
    pub objective: QuestObjective,
    pub input_port: u64,
    pub success_port: u64,
    pub failure_port: Option<u64>,
    pub optional: bool,
    pub secret: bool,
    pub journal_entry_on_start: Option<String>,
    pub journal_entry_on_complete: Option<String>,
    pub map_markers: Vec<MapMarker>,
    pub time_limit: Option<f32>,
    pub elapsed_time: f32,
    pub tags: Vec<String>,
    pub comment: String,
    pub completion_xp_bonus: u32,
}

impl ObjectiveNode {
    pub fn new(id: u64, objective: QuestObjective) -> Self {
        ObjectiveNode {
            id,
            position: Vec2::ZERO,
            objective_id: format!("obj_{}", id),
            objective,
            input_port: 0,
            success_port: 1,
            failure_port: None,
            optional: false,
            secret: false,
            journal_entry_on_start: None,
            journal_entry_on_complete: None,
            map_markers: Vec::new(),
            time_limit: None,
            elapsed_time: 0.0,
            tags: Vec::new(),
            comment: String::new(),
            completion_xp_bonus: 0,
        }
    }

    pub fn update(&mut self, delta: f32, events: &[QuestEvent]) -> ObjectiveNodeResult {
        if let Some(limit) = self.time_limit {
            self.elapsed_time += delta;
            if self.elapsed_time >= limit {
                return ObjectiveNodeResult::TimedOut;
            }
        }
        for event in events {
            self.objective.process_event(event);
        }
        match self.objective.get_status() {
            ObjectiveStatus::Completed => ObjectiveNodeResult::Succeeded,
            ObjectiveStatus::Failed => ObjectiveNodeResult::Failed,
            ObjectiveStatus::Active => ObjectiveNodeResult::InProgress(self.objective.get_progress()),
            ObjectiveStatus::NotStarted => ObjectiveNodeResult::NotStarted,
        }
    }

    pub fn time_remaining(&self) -> Option<f32> {
        self.time_limit.map(|limit| (limit - self.elapsed_time).max(0.0))
    }

    pub fn progress_display(&self) -> String {
        self.objective.get_progress_display()
    }
}

#[derive(Debug, Clone)]
pub enum ObjectiveNodeResult {
    NotStarted,
    InProgress(f32),
    Succeeded,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone)]
pub struct ConditionCheckNode {
    pub id: u64,
    pub position: Vec2,
    pub conditions: Vec<QuestCondition>,
    pub logic: ConditionLogic,
    pub true_port: u64,
    pub false_port: u64,
    pub input_port: u64,
    pub check_label: String,
    pub tags: Vec<String>,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConditionLogic {
    All,
    Any,
    None,
    Majority,
}

impl ConditionCheckNode {
    pub fn new(id: u64) -> Self {
        ConditionCheckNode {
            id,
            position: Vec2::ZERO,
            conditions: Vec::new(),
            logic: ConditionLogic::All,
            true_port: 0,
            false_port: 1,
            input_port: 0,
            check_label: String::new(),
            tags: Vec::new(),
            comment: String::new(),
        }
    }

    pub fn evaluate(&self, state: &QuestStateStore) -> bool {
        let results: Vec<bool> = self.conditions.iter()
            .map(|c| c.evaluate(state))
            .collect();
        match self.logic {
            ConditionLogic::All => results.iter().all(|&r| r),
            ConditionLogic::Any => results.iter().any(|&r| r),
            ConditionLogic::None => results.iter().all(|&r| !r),
            ConditionLogic::Majority => {
                let true_count = results.iter().filter(|&&r| r).count();
                true_count * 2 > results.len()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RewardNode {
    pub id: u64,
    pub position: Vec2,
    pub reward_table: RewardTable,
    pub input_port: u64,
    pub output_port: u64,
    pub reward_label: String,
    pub announce_rewards: bool,
    pub delay_secs: f32,
    pub tags: Vec<String>,
    pub comment: String,
}

impl RewardNode {
    pub fn new(id: u64) -> Self {
        RewardNode {
            id,
            position: Vec2::ZERO,
            reward_table: RewardTable::new(),
            input_port: 0,
            output_port: 1,
            reward_label: "Quest Reward".to_string(),
            announce_rewards: true,
            delay_secs: 0.0,
            tags: Vec::new(),
            comment: String::new(),
        }
    }

    pub fn roll_rewards(&self, player_level: u32, rng: &mut SimpleRng) -> Vec<RolledReward> {
        self.reward_table.roll(player_level, rng)
    }
}

#[derive(Debug, Clone)]
pub struct BranchNode {
    pub id: u64,
    pub position: Vec2,
    pub branch_type: BranchType,
    pub conditions: Vec<(QuestCondition, u64)>,
    pub default_port: u64,
    pub input_port: u64,
    pub tags: Vec<String>,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BranchType {
    FirstTrue,
    AllTrue,
    Random,
    PlayerChoice,
    ValueSwitch(String),
}

impl BranchNode {
    pub fn new(id: u64) -> Self {
        BranchNode {
            id,
            position: Vec2::ZERO,
            branch_type: BranchType::FirstTrue,
            conditions: Vec::new(),
            default_port: 0,
            input_port: 0,
            tags: Vec::new(),
            comment: String::new(),
        }
    }

    pub fn evaluate(&self, state: &QuestStateStore, rng: &mut SimpleRng) -> u64 {
        match &self.branch_type {
            BranchType::FirstTrue => {
                for (cond, port) in &self.conditions {
                    if cond.evaluate(state) {
                        return *port;
                    }
                }
                self.default_port
            }
            BranchType::AllTrue => {
                let all_true = self.conditions.iter().all(|(c, _)| c.evaluate(state));
                if all_true && !self.conditions.is_empty() {
                    self.conditions[0].1
                } else {
                    self.default_port
                }
            }
            BranchType::Random => {
                if self.conditions.is_empty() { return self.default_port; }
                let idx = rng.next_usize(self.conditions.len());
                self.conditions[idx].1
            }
            BranchType::PlayerChoice => {
                self.default_port // Player picks - handled externally
            }
            BranchType::ValueSwitch(var_name) => {
                // Would need value from state
                self.default_port
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FailNode {
    pub id: u64,
    pub position: Vec2,
    pub fail_reason: String,
    pub fail_message: String,
    pub on_fail_events: Vec<String>,
    pub penalty_reputation: Vec<(String, i32)>,
    pub penalty_xp: i32,
    pub allow_retry: bool,
    pub retry_cooldown: f32,
    pub input_port: u64,
    pub tags: Vec<String>,
    pub comment: String,
}

impl FailNode {
    pub fn new(id: u64) -> Self {
        FailNode {
            id,
            position: Vec2::ZERO,
            fail_reason: String::new(),
            fail_message: "Quest Failed".to_string(),
            on_fail_events: Vec::new(),
            penalty_reputation: Vec::new(),
            penalty_xp: 0,
            allow_retry: false,
            retry_cooldown: 0.0,
            input_port: 0,
            tags: Vec::new(),
            comment: String::new(),
        }
    }

    pub fn get_penalty_description(&self) -> String {
        let mut parts = Vec::new();
        if self.penalty_xp < 0 {
            parts.push(format!("{} XP", self.penalty_xp));
        }
        for (faction, delta) in &self.penalty_reputation {
            if *delta != 0 {
                parts.push(format!("{}: {:+}", faction, delta));
            }
        }
        parts.join(", ")
    }
}

#[derive(Debug, Clone)]
pub struct CompletionNode {
    pub id: u64,
    pub position: Vec2,
    pub completion_type: CompletionType,
    pub completion_message: String,
    pub journal_entry: String,
    pub on_complete_events: Vec<String>,
    pub unlock_quests: Vec<String>,
    pub set_flags: Vec<String>,
    pub mark_map_markers_complete: bool,
    pub credits_sequence: bool,
    pub input_port: u64,
    pub tags: Vec<String>,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompletionType {
    Success,
    PartialSuccess,
    Bittersweet,
    PyrrhicVictory,
}

impl CompletionNode {
    pub fn new(id: u64) -> Self {
        CompletionNode {
            id,
            position: Vec2::ZERO,
            completion_type: CompletionType::Success,
            completion_message: "Quest Complete!".to_string(),
            journal_entry: String::new(),
            on_complete_events: Vec::new(),
            unlock_quests: Vec::new(),
            set_flags: Vec::new(),
            mark_map_markers_complete: true,
            credits_sequence: false,
            input_port: 0,
            tags: Vec::new(),
            comment: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimerNode {
    pub id: u64,
    pub position: Vec2,
    pub duration_secs: f32,
    pub elapsed: f32,
    pub on_expire_port: u64,
    pub on_complete_port: u64,
    pub show_timer_hud: bool,
    pub timer_label: String,
    pub timer_color: Vec4,
    pub pause_when_inactive: bool,
    pub running: bool,
    pub input_port: u64,
    pub tags: Vec<String>,
    pub comment: String,
}

impl TimerNode {
    pub fn new(id: u64) -> Self {
        TimerNode {
            id,
            position: Vec2::ZERO,
            duration_secs: 60.0,
            elapsed: 0.0,
            on_expire_port: 0,
            on_complete_port: 1,
            show_timer_hud: true,
            timer_label: String::new(),
            timer_color: Vec4::new(1.0, 0.5, 0.0, 1.0),
            pause_when_inactive: false,
            running: false,
            input_port: 0,
            tags: Vec::new(),
            comment: String::new(),
        }
    }

    pub fn update(&mut self, delta: f32) -> Option<bool> {
        if !self.running { return None; }
        self.elapsed += delta;
        if self.elapsed >= self.duration_secs {
            self.running = false;
            Some(true) // expired
        } else {
            None
        }
    }

    pub fn reset(&mut self) { self.elapsed = 0.0; self.running = false; }
    pub fn start(&mut self) { self.running = true; }
    pub fn remaining(&self) -> f32 { (self.duration_secs - self.elapsed).max(0.0) }
    pub fn progress(&self) -> f32 { (self.elapsed / self.duration_secs).clamp(0.0, 1.0) }

    pub fn format_remaining(&self) -> String {
        let rem = self.remaining();
        let mins = (rem / 60.0) as u32;
        let secs = (rem % 60.0) as u32;
        format!("{}:{:02}", mins, secs)
    }
}

#[derive(Debug, Clone)]
pub struct TriggerNode {
    pub id: u64,
    pub position: Vec2,
    pub trigger_type: QuestTriggerType,
    pub trigger_data: HashMap<String, String>,
    pub output_port: u64,
    pub one_shot: bool,
    pub triggered: bool,
    pub cooldown: f32,
    pub last_triggered: f32,
    pub input_port: u64,
    pub tags: Vec<String>,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuestTriggerType {
    EnterArea(Vec3, f32),
    KillTarget(String),
    InteractObject(String),
    ItemPickup(String),
    TimePassed(f32),
    Custom(String),
    PlayerNearNPC(String, f32),
    QuestStateChange(String, QuestState),
}

impl TriggerNode {
    pub fn new(id: u64) -> Self {
        TriggerNode {
            id,
            position: Vec2::ZERO,
            trigger_type: QuestTriggerType::Custom("default".to_string()),
            trigger_data: HashMap::new(),
            output_port: 0,
            one_shot: true,
            triggered: false,
            cooldown: 0.0,
            last_triggered: -f32::MAX,
            input_port: 0,
            tags: Vec::new(),
            comment: String::new(),
        }
    }

    pub fn check_trigger(&self, event: &QuestEvent, current_time: f32) -> bool {
        if self.one_shot && self.triggered { return false; }
        if current_time - self.last_triggered < self.cooldown { return false; }
        match (&self.trigger_type, event) {
            (QuestTriggerType::KillTarget(target_id), QuestEvent::EnemyKilled { enemy_id, .. }) => {
                Some(target_id.as_str()) == enemy_id.as_deref()
            }
            (QuestTriggerType::ItemPickup(item_id), QuestEvent::ItemPickedUp { item_id: picked_id, .. }) => {
                item_id == picked_id
            }
            (QuestTriggerType::InteractObject(obj_id), QuestEvent::ObjectInteracted { object_id }) => {
                obj_id == object_id
            }
            (QuestTriggerType::Custom(name), QuestEvent::CustomEvent { name: event_name, .. }) => {
                name == event_name
            }
            _ => false,
        }
    }
}

// ============================================================
// SECTION 2: OBJECTIVE TYPES (20+)
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectiveStatus {
    NotStarted,
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub enum QuestObjective {
    KillEnemy(KillEnemyObjective),
    CollectItem(CollectItemObjective),
    ReachLocation(ReachLocationObjective),
    TalkToNPC(TalkToNPCObjective),
    ProtectTarget(ProtectTargetObjective),
    EscortTarget(EscortTargetObjective),
    SurviveWaves(SurviveWavesObjective),
    SolvePuzzle(SolvePuzzleObjective),
    CraftItem(CraftItemObjective),
    UseAbility(UseAbilityObjective),
    ExploreArea(ExploreAreaObjective),
    TakePhoto(TakePhotoObjective),
    BuildStructure(BuildStructureObjective),
    DefeatBoss(DefeatBossObjective),
    FindSecret(FindSecretObjective),
    DeliverItem(DeliverItemObjective),
    InvestigateClue(InvestigateClueObjective),
    RepairObject(RepairObjectObjective),
    PlantDevice(PlantDeviceObjective),
    HackTerminal(HackTerminalObjective),
    SneakPast(SneakPastObjective),
    PickpocketTarget(PickpocketTargetObjective),
}

impl QuestObjective {
    pub fn get_status(&self) -> ObjectiveStatus {
        match self {
            QuestObjective::KillEnemy(o) => o.status(),
            QuestObjective::CollectItem(o) => o.status(),
            QuestObjective::ReachLocation(o) => o.status(),
            QuestObjective::TalkToNPC(o) => o.status(),
            QuestObjective::ProtectTarget(o) => o.status(),
            QuestObjective::EscortTarget(o) => o.status(),
            QuestObjective::SurviveWaves(o) => o.status(),
            QuestObjective::SolvePuzzle(o) => o.status(),
            QuestObjective::CraftItem(o) => o.status(),
            QuestObjective::UseAbility(o) => o.status(),
            QuestObjective::ExploreArea(o) => o.status(),
            QuestObjective::TakePhoto(o) => o.status(),
            QuestObjective::BuildStructure(o) => o.status(),
            QuestObjective::DefeatBoss(o) => o.status(),
            QuestObjective::FindSecret(o) => o.status(),
            QuestObjective::DeliverItem(o) => o.status(),
            QuestObjective::InvestigateClue(o) => o.status(),
            QuestObjective::RepairObject(o) => o.status(),
            QuestObjective::PlantDevice(o) => o.status(),
            QuestObjective::HackTerminal(o) => o.status(),
            QuestObjective::SneakPast(o) => o.status(),
            QuestObjective::PickpocketTarget(o) => o.status(),
        }
    }

    pub fn get_progress(&self) -> f32 {
        match self {
            QuestObjective::KillEnemy(o) => o.progress(),
            QuestObjective::CollectItem(o) => o.progress(),
            QuestObjective::ReachLocation(o) => o.progress(),
            QuestObjective::TalkToNPC(o) => o.progress(),
            QuestObjective::ProtectTarget(o) => o.progress(),
            QuestObjective::EscortTarget(o) => o.progress(),
            QuestObjective::SurviveWaves(o) => o.progress(),
            QuestObjective::SolvePuzzle(o) => o.progress(),
            QuestObjective::CraftItem(o) => o.progress(),
            QuestObjective::UseAbility(o) => o.progress(),
            QuestObjective::ExploreArea(o) => o.progress(),
            QuestObjective::TakePhoto(o) => o.progress(),
            QuestObjective::BuildStructure(o) => o.progress(),
            QuestObjective::DefeatBoss(o) => o.progress(),
            QuestObjective::FindSecret(o) => o.progress(),
            QuestObjective::DeliverItem(o) => o.progress(),
            QuestObjective::InvestigateClue(o) => o.progress(),
            QuestObjective::RepairObject(o) => o.progress(),
            QuestObjective::PlantDevice(o) => o.progress(),
            QuestObjective::HackTerminal(o) => o.progress(),
            QuestObjective::SneakPast(o) => o.progress(),
            QuestObjective::PickpocketTarget(o) => o.progress(),
        }
    }

    pub fn get_progress_display(&self) -> String {
        match self {
            QuestObjective::KillEnemy(o) => format!("{}/{}", o.killed_count, o.required_count),
            QuestObjective::CollectItem(o) => format!("{}/{}", o.collected_count, o.required_count),
            QuestObjective::SurviveWaves(o) => format!("Wave {}/{}", o.waves_survived, o.total_waves),
            QuestObjective::ExploreArea(o) => {
                let visited = o.areas.iter().filter(|a| a.visited).count();
                format!("{}/{}", visited, o.areas.len())
            }
            QuestObjective::BuildStructure(o) => format!("{:.0}%", o.build_progress * 100.0),
            QuestObjective::HackTerminal(o) => format!("{:.0}%", o.hack_progress * 100.0),
            _ => String::new(),
        }
    }

    pub fn process_event(&mut self, event: &QuestEvent) {
        match self {
            QuestObjective::KillEnemy(o) => o.process_event(event),
            QuestObjective::CollectItem(o) => o.process_event(event),
            QuestObjective::ReachLocation(o) => o.process_event(event),
            QuestObjective::TalkToNPC(o) => o.process_event(event),
            QuestObjective::ProtectTarget(o) => o.process_event(event),
            QuestObjective::EscortTarget(o) => o.process_event(event),
            QuestObjective::SurviveWaves(o) => o.process_event(event),
            QuestObjective::SolvePuzzle(o) => o.process_event(event),
            QuestObjective::CraftItem(o) => o.process_event(event),
            QuestObjective::UseAbility(o) => o.process_event(event),
            QuestObjective::ExploreArea(o) => o.process_event(event),
            QuestObjective::TakePhoto(o) => o.process_event(event),
            QuestObjective::BuildStructure(o) => o.process_event(event),
            QuestObjective::DefeatBoss(o) => o.process_event(event),
            QuestObjective::FindSecret(o) => o.process_event(event),
            QuestObjective::DeliverItem(o) => o.process_event(event),
            QuestObjective::InvestigateClue(o) => o.process_event(event),
            QuestObjective::RepairObject(o) => o.process_event(event),
            QuestObjective::PlantDevice(o) => o.process_event(event),
            QuestObjective::HackTerminal(o) => o.process_event(event),
            QuestObjective::SneakPast(o) => o.process_event(event),
            QuestObjective::PickpocketTarget(o) => o.process_event(event),
        }
    }

    pub fn description(&self) -> String {
        match self {
            QuestObjective::KillEnemy(o) => format!("Kill {} {}", o.required_count, o.enemy_type),
            QuestObjective::CollectItem(o) => format!("Collect {} {}", o.required_count, o.item_id),
            QuestObjective::ReachLocation(o) => format!("Reach {}", o.location_name),
            QuestObjective::TalkToNPC(o) => format!("Talk to {}", o.npc_name),
            QuestObjective::ProtectTarget(o) => format!("Protect {} for {}s", o.target_id, o.duration_secs),
            QuestObjective::EscortTarget(o) => format!("Escort {} to {}", o.target_id, o.destination_name),
            QuestObjective::SurviveWaves(o) => format!("Survive {} waves", o.total_waves),
            QuestObjective::SolvePuzzle(o) => format!("Solve the {}", o.puzzle_id),
            QuestObjective::CraftItem(o) => format!("Craft {}", o.item_id),
            QuestObjective::UseAbility(o) => format!("Use {} {} times", o.ability_id, o.required_count),
            QuestObjective::ExploreArea(o) => format!("Explore {}", o.area_name),
            QuestObjective::TakePhoto(o) => format!("Take photo of {}", o.subject_id),
            QuestObjective::BuildStructure(o) => format!("Build {}", o.structure_id),
            QuestObjective::DefeatBoss(o) => format!("Defeat {}", o.boss_id),
            QuestObjective::FindSecret(o) => format!("Find the secret"),
            QuestObjective::DeliverItem(o) => format!("Deliver {} to {}", o.item_id, o.recipient_id),
            QuestObjective::InvestigateClue(o) => format!("Investigate {}", o.location_name),
            QuestObjective::RepairObject(o) => format!("Repair {}", o.object_id),
            QuestObjective::PlantDevice(o) => format!("Plant device at {}", o.location_name),
            QuestObjective::HackTerminal(o) => format!("Hack terminal {}", o.terminal_id),
            QuestObjective::SneakPast(o) => format!("Sneak past {} guards", o.guard_count),
            QuestObjective::PickpocketTarget(o) => format!("Pickpocket {}", o.target_id),
        }
    }
}

// --- Kill Enemy ---
#[derive(Debug, Clone)]
pub struct KillEnemyObjective {
    pub enemy_type: String,
    pub enemy_id: Option<String>,
    pub required_count: u32,
    pub killed_count: u32,
    pub in_area: Option<String>,
    pub with_weapon_type: Option<String>,
    pub require_stealth_kill: bool,
    pub allow_assists: bool,
}

impl KillEnemyObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.killed_count >= self.required_count { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 {
        if self.required_count == 0 { return 1.0; }
        (self.killed_count as f32 / self.required_count as f32).min(1.0)
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::EnemyKilled { enemy_type, enemy_id, weapon_type, is_stealth } = event {
            if self.killed_count >= self.required_count { return; }
            let type_match = &self.enemy_type == enemy_type || self.enemy_type == "*";
            let id_match = self.enemy_id.as_ref().map(|id| id == enemy_id.as_deref().unwrap_or("")).unwrap_or(true);
            let weapon_match = self.with_weapon_type.as_ref().map(|w| Some(w) == weapon_type.as_ref()).unwrap_or(true);
            let stealth_match = !self.require_stealth_kill || *is_stealth;
            if type_match && id_match && weapon_match && stealth_match {
                self.killed_count += 1;
            }
        }
    }
}

// --- Collect Item ---
#[derive(Debug, Clone)]
pub struct CollectItemObjective {
    pub item_id: String,
    pub item_name: String,
    pub required_count: u32,
    pub collected_count: u32,
    pub quality_requirement: Option<f32>,
    pub allow_partial: bool,
    pub consume_on_completion: bool,
}

impl CollectItemObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.collected_count >= self.required_count { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 {
        if self.required_count == 0 { return 1.0; }
        (self.collected_count as f32 / self.required_count as f32).min(1.0)
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::ItemPickedUp { item_id, quantity, quality } = event {
            if item_id == &self.item_id {
                let qual_ok = self.quality_requirement.map(|q| *quality >= q).unwrap_or(true);
                if qual_ok {
                    self.collected_count = (self.collected_count + quantity).min(self.required_count);
                }
            }
        }
    }
}

// --- Reach Location ---
#[derive(Debug, Clone)]
pub struct ReachLocationObjective {
    pub location_id: String,
    pub location_name: String,
    pub position: Vec3,
    pub radius: f32,
    pub required_time_secs: Option<f32>,
    pub time_at_location: f32,
    pub reached: bool,
    pub require_no_combat: bool,
}

impl ReachLocationObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.reached { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { if self.reached { 1.0 } else { 0.0 } }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::PlayerMoved { position, is_in_combat } = event {
            if self.require_no_combat && *is_in_combat { return; }
            let dist = (*position - self.position).length();
            if dist <= self.radius {
                if let Some(req_time) = self.required_time_secs {
                    // Need to track delta time - simplified
                    self.time_at_location += 0.016;
                    if self.time_at_location >= req_time { self.reached = true; }
                } else {
                    self.reached = true;
                }
            } else {
                self.time_at_location = 0.0;
            }
        }
    }
}

// --- Talk To NPC ---
#[derive(Debug, Clone)]
pub struct TalkToNPCObjective {
    pub npc_id: String,
    pub npc_name: String,
    pub required_topic: Option<String>,
    pub talked: bool,
    pub dialogue_completed: bool,
    pub require_specific_ending: Option<String>,
}

impl TalkToNPCObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.talked && self.dialogue_completed { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 {
        match (self.talked, self.dialogue_completed) {
            (false, _) => 0.0,
            (true, false) => 0.5,
            (true, true) => 1.0,
        }
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::DialogueStarted { npc_id } => {
                if npc_id == &self.npc_id { self.talked = true; }
            }
            QuestEvent::DialogueEnded { npc_id, ending } => {
                if npc_id == &self.npc_id {
                    let ending_ok = self.require_specific_ending.as_ref()
                        .map(|req| req == ending)
                        .unwrap_or(true);
                    if ending_ok { self.dialogue_completed = true; }
                }
            }
            _ => {}
        }
    }
}

// --- Protect Target ---
#[derive(Debug, Clone)]
pub struct ProtectTargetObjective {
    pub target_id: String,
    pub target_name: String,
    pub duration_secs: f32,
    pub protected_secs: f32,
    pub target_health_min: f32,
    pub failed: bool,
    pub running: bool,
}

impl ProtectTargetObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.failed { return ObjectiveStatus::Failed; }
        if self.protected_secs >= self.duration_secs { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 {
        if self.failed { return 0.0; }
        (self.protected_secs / self.duration_secs).min(1.0)
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::NPCDied { npc_id } => {
                if npc_id == &self.target_id { self.failed = true; }
            }
            QuestEvent::NPCDamaged { npc_id, health_percent } => {
                if npc_id == &self.target_id && *health_percent < self.target_health_min {
                    self.failed = true;
                }
            }
            QuestEvent::TimePassed { delta } => {
                if self.running && !self.failed {
                    self.protected_secs += delta;
                }
            }
            _ => {}
        }
    }
}

// --- Escort Target ---
#[derive(Debug, Clone)]
pub struct EscortTargetObjective {
    pub target_id: String,
    pub destination_name: String,
    pub destination_pos: Vec3,
    pub destination_radius: f32,
    pub reached: bool,
    pub target_died: bool,
    pub must_stay_near: bool,
    pub stay_near_radius: f32,
    pub player_too_far: bool,
}

impl EscortTargetObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.target_died { ObjectiveStatus::Failed }
        else if self.reached { ObjectiveStatus::Completed }
        else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { if self.reached { 1.0 } else if self.target_died { 0.0 } else { 0.5 } }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::NPCDied { npc_id } => {
                if npc_id == &self.target_id { self.target_died = true; }
            }
            QuestEvent::NPCMoved { npc_id, position } => {
                if npc_id == &self.target_id {
                    let dist = (*position - self.destination_pos).length();
                    if dist <= self.destination_radius { self.reached = true; }
                }
            }
            _ => {}
        }
    }
}

// --- Survive Waves ---
#[derive(Debug, Clone)]
pub struct SurviveWavesObjective {
    pub total_waves: u32,
    pub waves_survived: u32,
    pub current_wave_enemies_remaining: u32,
    pub player_died: bool,
    pub between_waves: bool,
    pub wave_countdown: f32,
    pub wave_interval: f32,
    pub enemy_spawn_table: Vec<WaveEnemy>,
}

#[derive(Debug, Clone)]
pub struct WaveEnemy {
    pub enemy_type: String,
    pub count: u32,
    pub wave_start: u32,
}

impl SurviveWavesObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.player_died { ObjectiveStatus::Failed }
        else if self.waves_survived >= self.total_waves { ObjectiveStatus::Completed }
        else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 {
        if self.total_waves == 0 { return 1.0; }
        self.waves_survived as f32 / self.total_waves as f32
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::EnemyKilled { .. } => {
                if self.current_wave_enemies_remaining > 0 {
                    self.current_wave_enemies_remaining -= 1;
                    if self.current_wave_enemies_remaining == 0 {
                        self.waves_survived += 1;
                        self.between_waves = true;
                        self.wave_countdown = self.wave_interval;
                    }
                }
            }
            QuestEvent::PlayerDied => { self.player_died = true; }
            QuestEvent::TimePassed { delta } => {
                if self.between_waves {
                    self.wave_countdown -= delta;
                    if self.wave_countdown <= 0.0 { self.between_waves = false; }
                }
            }
            _ => {}
        }
    }
}

// --- Solve Puzzle ---
#[derive(Debug, Clone)]
pub struct SolvePuzzleObjective {
    pub puzzle_id: String,
    pub puzzle_name: String,
    pub solved: bool,
    pub attempts: u32,
    pub max_attempts: Option<u32>,
    pub failed: bool,
    pub hint_unlocked: bool,
}

impl SolvePuzzleObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.failed { ObjectiveStatus::Failed }
        else if self.solved { ObjectiveStatus::Completed }
        else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { if self.solved { 1.0 } else { 0.0 } }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::PuzzleSolved { puzzle_id } = event {
            if puzzle_id == &self.puzzle_id { self.solved = true; }
        }
        if let QuestEvent::PuzzleAttempted { puzzle_id, success } = event {
            if puzzle_id == &self.puzzle_id {
                self.attempts += 1;
                if *success { self.solved = true; }
                else if let Some(max) = self.max_attempts {
                    if self.attempts >= max { self.failed = true; }
                }
            }
        }
    }
}

// --- Craft Item ---
#[derive(Debug, Clone)]
pub struct CraftItemObjective {
    pub item_id: String,
    pub item_name: String,
    pub required_count: u32,
    pub crafted_count: u32,
    pub quality_min: Option<f32>,
    pub at_station_id: Option<String>,
}

impl CraftItemObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.crafted_count >= self.required_count { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 {
        (self.crafted_count as f32 / self.required_count.max(1) as f32).min(1.0)
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::ItemCrafted { item_id, quality, station_id } = event {
            if item_id == &self.item_id {
                let qual_ok = self.quality_min.map(|q| *quality >= q).unwrap_or(true);
                let station_ok = self.at_station_id.as_ref().map(|s| Some(s) == station_id.as_ref()).unwrap_or(true);
                if qual_ok && station_ok && self.crafted_count < self.required_count {
                    self.crafted_count += 1;
                }
            }
        }
    }
}

// --- Use Ability ---
#[derive(Debug, Clone)]
pub struct UseAbilityObjective {
    pub ability_id: String,
    pub ability_name: String,
    pub required_count: u32,
    pub use_count: u32,
    pub on_enemy_type: Option<String>,
    pub require_hit: bool,
}

impl UseAbilityObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.use_count >= self.required_count { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { (self.use_count as f32 / self.required_count.max(1) as f32).min(1.0) }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::AbilityUsed { ability_id, hit, target_type } = event {
            if ability_id == &self.ability_id {
                let hit_ok = !self.require_hit || *hit;
                let target_ok = self.on_enemy_type.as_ref().map(|t| Some(t) == target_type.as_ref()).unwrap_or(true);
                if hit_ok && target_ok && self.use_count < self.required_count {
                    self.use_count += 1;
                }
            }
        }
    }
}

// --- Explore Area ---
#[derive(Debug, Clone)]
pub struct ExploreAreaObjective {
    pub area_name: String,
    pub areas: Vec<AreaPoint>,
    pub require_all: bool,
    pub discover_radius: f32,
}

#[derive(Debug, Clone)]
pub struct AreaPoint {
    pub id: String,
    pub name: String,
    pub position: Vec3,
    pub visited: bool,
    pub optional: bool,
}

impl ExploreAreaObjective {
    pub fn status(&self) -> ObjectiveStatus {
        let required: Vec<&AreaPoint> = self.areas.iter().filter(|a| !a.optional).collect();
        let visited = required.iter().filter(|a| a.visited).count();
        if self.require_all {
            if visited >= required.len() { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
        } else {
            if visited > 0 { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
        }
    }
    pub fn progress(&self) -> f32 {
        let required: Vec<&AreaPoint> = self.areas.iter().filter(|a| !a.optional).collect();
        if required.is_empty() { return 1.0; }
        let visited = required.iter().filter(|a| a.visited).count();
        visited as f32 / required.len() as f32
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::PlayerMoved { position, .. } = event {
            for area in &mut self.areas {
                if !area.visited {
                    let dist = (*position - area.position).length();
                    if dist <= self.discover_radius { area.visited = true; }
                }
            }
        }
    }
}

// --- Take Photo ---
#[derive(Debug, Clone)]
pub struct TakePhotoObjective {
    pub subject_id: String,
    pub subject_name: String,
    pub required_count: u32,
    pub taken_count: u32,
    pub min_quality: f32,
    pub must_be_undetected: bool,
    pub subject_behavior: Option<String>,
}

impl TakePhotoObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.taken_count >= self.required_count { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { (self.taken_count as f32 / self.required_count.max(1) as f32).min(1.0) }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::PhotoTaken { subject_id, quality, undetected } = event {
            if subject_id == &self.subject_id {
                let qual_ok = *quality >= self.min_quality;
                let detect_ok = !self.must_be_undetected || *undetected;
                if qual_ok && detect_ok && self.taken_count < self.required_count {
                    self.taken_count += 1;
                }
            }
        }
    }
}

// --- Build Structure ---
#[derive(Debug, Clone)]
pub struct BuildStructureObjective {
    pub structure_id: String,
    pub structure_name: String,
    pub build_progress: f32,
    pub required_progress: f32,
    pub at_location: Option<Vec3>,
    pub required_resources: Vec<(String, u32)>,
    pub resources_placed: HashMap<String, u32>,
}

impl BuildStructureObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.build_progress >= self.required_progress { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { (self.build_progress / self.required_progress.max(0.001)).min(1.0) }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::StructureBuilt { structure_id, progress } = event {
            if structure_id == &self.structure_id { self.build_progress = *progress; }
        }
        if let QuestEvent::ResourcePlaced { structure_id, resource_id, amount } = event {
            if structure_id == &self.structure_id {
                *self.resources_placed.entry(resource_id.clone()).or_insert(0) += amount;
            }
        }
    }
}

// --- Defeat Boss ---
#[derive(Debug, Clone)]
pub struct DefeatBossObjective {
    pub boss_id: String,
    pub boss_name: String,
    pub defeated: bool,
    pub phases_completed: u32,
    pub total_phases: u32,
    pub require_no_deaths: bool,
    pub player_died: bool,
    pub time_limit: Option<f32>,
    pub elapsed: f32,
}

impl DefeatBossObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.require_no_deaths && self.player_died { return ObjectiveStatus::Failed; }
        if self.time_limit.map(|t| self.elapsed >= t).unwrap_or(false) { return ObjectiveStatus::Failed; }
        if self.defeated { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 {
        if self.defeated { return 1.0; }
        if self.total_phases == 0 { return 0.0; }
        self.phases_completed as f32 / self.total_phases as f32
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::EnemyKilled { enemy_id, .. } => {
                if enemy_id.as_deref() == Some(&self.boss_id) { self.defeated = true; }
            }
            QuestEvent::BossPhaseCompleted { boss_id, phase } => {
                if boss_id == &self.boss_id { self.phases_completed = *phase; }
            }
            QuestEvent::PlayerDied => { self.player_died = true; }
            QuestEvent::TimePassed { delta } => { self.elapsed += delta; }
            _ => {}
        }
    }
}

// --- Find Secret ---
#[derive(Debug, Clone)]
pub struct FindSecretObjective {
    pub secret_id: String,
    pub location_hint: String,
    pub found: bool,
    pub reveal_on_find: bool,
}

impl FindSecretObjective {
    pub fn status(&self) -> ObjectiveStatus { if self.found { ObjectiveStatus::Completed } else { ObjectiveStatus::Active } }
    pub fn progress(&self) -> f32 { if self.found { 1.0 } else { 0.0 } }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::SecretFound { secret_id } = event {
            if secret_id == &self.secret_id { self.found = true; }
        }
    }
}

// --- Deliver Item ---
#[derive(Debug, Clone)]
pub struct DeliverItemObjective {
    pub item_id: String,
    pub item_name: String,
    pub recipient_id: String,
    pub recipient_name: String,
    pub quantity: u32,
    pub delivered: u32,
    pub item_can_be_damaged: bool,
    pub item_condition: f32,
    pub condition_min: f32,
}

impl DeliverItemObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.delivered >= self.quantity { ObjectiveStatus::Completed }
        else if self.item_can_be_damaged && self.item_condition < self.condition_min { ObjectiveStatus::Failed }
        else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { (self.delivered as f32 / self.quantity.max(1) as f32).min(1.0) }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::ItemDelivered { item_id, recipient_id, quantity } = event {
            if item_id == &self.item_id && recipient_id == &self.recipient_id {
                self.delivered += quantity;
            }
        }
    }
}

// --- Investigate Clue ---
#[derive(Debug, Clone)]
pub struct InvestigateClueObjective {
    pub location_name: String,
    pub clue_ids: Vec<String>,
    pub clues_found: HashSet<String>,
    pub required_clues: u32,
    pub all_clues_required: bool,
}

impl InvestigateClueObjective {
    pub fn status(&self) -> ObjectiveStatus {
        let found = self.clues_found.len() as u32;
        let needed = if self.all_clues_required { self.clue_ids.len() as u32 } else { self.required_clues };
        if found >= needed { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 {
        let needed = if self.all_clues_required { self.clue_ids.len() } else { self.required_clues as usize };
        if needed == 0 { return 1.0; }
        (self.clues_found.len() as f32 / needed as f32).min(1.0)
    }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::ClueFound { clue_id } = event {
            if self.clue_ids.contains(clue_id) { self.clues_found.insert(clue_id.clone()); }
        }
    }
}

// --- Repair Object ---
#[derive(Debug, Clone)]
pub struct RepairObjectObjective {
    pub object_id: String,
    pub object_name: String,
    pub repair_progress: f32,
    pub required_progress: f32,
    pub requires_item: Option<String>,
    pub item_consumed: bool,
}

impl RepairObjectObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.repair_progress >= self.required_progress { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { (self.repair_progress / self.required_progress.max(0.001)).min(1.0) }
    pub fn process_event(&mut self, event: &QuestEvent) {
        if let QuestEvent::ObjectRepaired { object_id, progress } = event {
            if object_id == &self.object_id { self.repair_progress = *progress; }
        }
    }
}

// --- Plant Device ---
#[derive(Debug, Clone)]
pub struct PlantDeviceObjective {
    pub device_id: String,
    pub location_name: String,
    pub target_objects: Vec<String>,
    pub planted_on: Vec<String>,
    pub required_count: u32,
    pub undetected: bool,
    pub was_detected: bool,
}

impl PlantDeviceObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.undetected && self.was_detected { return ObjectiveStatus::Failed; }
        if self.planted_on.len() >= self.required_count as usize { ObjectiveStatus::Completed } else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { (self.planted_on.len() as f32 / self.required_count.max(1) as f32).min(1.0) }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::DevicePlanted { device_id, object_id } => {
                if device_id == &self.device_id && self.target_objects.contains(object_id) {
                    if !self.planted_on.contains(object_id) { self.planted_on.push(object_id.clone()); }
                }
            }
            QuestEvent::PlayerDetected => { self.was_detected = true; }
            _ => {}
        }
    }
}

// --- Hack Terminal ---
#[derive(Debug, Clone)]
pub struct HackTerminalObjective {
    pub terminal_id: String,
    pub terminal_name: String,
    pub hack_progress: f32,
    pub required_progress: f32,
    pub difficulty: f32,
    pub failed_attempts: u32,
    pub max_attempts: Option<u32>,
    pub locked_out: bool,
}

impl HackTerminalObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.locked_out { ObjectiveStatus::Failed }
        else if self.hack_progress >= self.required_progress { ObjectiveStatus::Completed }
        else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { (self.hack_progress / self.required_progress.max(0.001)).min(1.0) }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::TerminalHacked { terminal_id, progress, success } => {
                if terminal_id == &self.terminal_id {
                    if *success {
                        self.hack_progress = *progress;
                    } else {
                        self.failed_attempts += 1;
                        if let Some(max) = self.max_attempts {
                            if self.failed_attempts >= max { self.locked_out = true; }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// --- Sneak Past ---
#[derive(Debug, Clone)]
pub struct SneakPastObjective {
    pub area_id: String,
    pub guard_count: u32,
    pub guards_passed: u32,
    pub was_detected: bool,
    pub destination_reached: bool,
}

impl SneakPastObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.was_detected { ObjectiveStatus::Failed }
        else if self.destination_reached { ObjectiveStatus::Completed }
        else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { if self.destination_reached { 1.0 } else { 0.0 } }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::PlayerDetected => { self.was_detected = true; }
            QuestEvent::PlayerMoved { position, .. } => {}
            _ => {}
        }
    }
}

// --- Pickpocket Target ---
#[derive(Debug, Clone)]
pub struct PickpocketTargetObjective {
    pub target_id: String,
    pub target_name: String,
    pub item_id: String,
    pub stolen: bool,
    pub without_detection: bool,
    pub detected: bool,
}

impl PickpocketTargetObjective {
    pub fn status(&self) -> ObjectiveStatus {
        if self.without_detection && self.detected { ObjectiveStatus::Failed }
        else if self.stolen { ObjectiveStatus::Completed }
        else { ObjectiveStatus::Active }
    }
    pub fn progress(&self) -> f32 { if self.stolen { 1.0 } else { 0.0 } }
    pub fn process_event(&mut self, event: &QuestEvent) {
        match event {
            QuestEvent::Pickpocketed { target_id, item_id, detected } => {
                if target_id == &self.target_id && item_id == &self.item_id {
                    self.stolen = true;
                    if *detected { self.detected = true; }
                }
            }
            _ => {}
        }
    }
}

// ============================================================
// SECTION 3: QUEST EVENTS
// ============================================================

#[derive(Debug, Clone)]
pub enum QuestEvent {
    EnemyKilled { enemy_type: String, enemy_id: Option<String>, weapon_type: Option<String>, is_stealth: bool },
    ItemPickedUp { item_id: String, quantity: u32, quality: f32 },
    PlayerMoved { position: Vec3, is_in_combat: bool },
    DialogueStarted { npc_id: String },
    DialogueEnded { npc_id: String, ending: String },
    NPCDied { npc_id: String },
    NPCDamaged { npc_id: String, health_percent: f32 },
    NPCMoved { npc_id: String, position: Vec3 },
    PuzzleSolved { puzzle_id: String },
    PuzzleAttempted { puzzle_id: String, success: bool },
    ItemCrafted { item_id: String, quality: f32, station_id: Option<String> },
    AbilityUsed { ability_id: String, hit: bool, target_type: Option<String> },
    PhotoTaken { subject_id: String, quality: f32, undetected: bool },
    StructureBuilt { structure_id: String, progress: f32 },
    ResourcePlaced { structure_id: String, resource_id: String, amount: u32 },
    BossPhaseCompleted { boss_id: String, phase: u32 },
    SecretFound { secret_id: String },
    ItemDelivered { item_id: String, recipient_id: String, quantity: u32 },
    ClueFound { clue_id: String },
    ObjectRepaired { object_id: String, progress: f32 },
    DevicePlanted { device_id: String, object_id: String },
    PlayerDetected,
    PlayerDied,
    TerminalHacked { terminal_id: String, progress: f32, success: bool },
    Pickpocketed { target_id: String, item_id: String, detected: bool },
    TimePassed { delta: f32 },
    ObjectInteracted { object_id: String },
    CustomEvent { name: String, data: HashMap<String, String> },
}

// ============================================================
// SECTION 4: COMPLETION CONDITIONS & REWARD SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct QuestCondition {
    pub condition_type: QuestConditionType,
    pub negated: bool,
}

#[derive(Debug, Clone)]
pub enum QuestConditionType {
    QuestState { quest_id: String, state: QuestState },
    PlayerLevel { min: u32, max: Option<u32> },
    FactionRep { faction_id: String, min: i32, max: Option<i32> },
    HasItem { item_id: String, quantity: u32 },
    FlagSet(String),
    TimeSince { event: String, secs: f32 },
    ObjectiveComplete { quest_id: String, objective_id: String },
    PlayerStat { stat: String, min: f32, max: Option<f32> },
}

impl QuestCondition {
    pub fn evaluate(&self, state: &QuestStateStore) -> bool {
        let result = match &self.condition_type {
            QuestConditionType::QuestState { quest_id, state: expected } => {
                state.get_quest_state(quest_id) == *expected
            }
            QuestConditionType::PlayerLevel { min, max } => {
                let level = state.player_level;
                level >= *min && max.map(|m| level <= m).unwrap_or(true)
            }
            QuestConditionType::FactionRep { faction_id, min, max } => {
                let rep = state.get_faction_rep(faction_id);
                rep >= *min && max.map(|m| rep <= m).unwrap_or(true)
            }
            QuestConditionType::HasItem { item_id, quantity } => {
                state.get_item_count(item_id) >= *quantity
            }
            QuestConditionType::FlagSet(flag) => {
                state.get_flag(flag)
            }
            QuestConditionType::TimeSince { event, secs } => {
                state.get_time_since(event) >= *secs
            }
            QuestConditionType::ObjectiveComplete { quest_id, objective_id } => {
                state.is_objective_complete(quest_id, objective_id)
            }
            QuestConditionType::PlayerStat { stat, min, max } => {
                let val = state.get_player_stat(stat);
                val >= *min && max.map(|m| val <= m).unwrap_or(true)
            }
        };
        if self.negated { !result } else { result }
    }
}

#[derive(Debug, Clone)]
pub struct RewardTable {
    pub xp_reward: XpReward,
    pub item_rewards: Vec<ItemRewardEntry>,
    pub currency_rewards: Vec<CurrencyReward>,
    pub reputation_rewards: Vec<ReputationReward>,
    pub unlock_rewards: Vec<UnlockReward>,
    pub skill_rewards: Vec<SkillReward>,
}

impl RewardTable {
    pub fn new() -> Self {
        RewardTable {
            xp_reward: XpReward::default(),
            item_rewards: Vec::new(),
            currency_rewards: Vec::new(),
            reputation_rewards: Vec::new(),
            unlock_rewards: Vec::new(),
            skill_rewards: Vec::new(),
        }
    }

    pub fn roll(&self, player_level: u32, rng: &mut SimpleRng) -> Vec<RolledReward> {
        let mut rewards = Vec::new();
        // XP
        let xp = self.xp_reward.calculate(player_level);
        if xp > 0 {
            rewards.push(RolledReward::Xp(xp));
        }
        // Items
        for item_entry in &self.item_rewards {
            if rng.next_f32() <= item_entry.drop_chance {
                let quantity = item_entry.roll_quantity(rng);
                let quality = item_entry.roll_quality(rng);
                rewards.push(RolledReward::Item {
                    item_id: item_entry.item_id.clone(),
                    quantity,
                    quality,
                    rarity: item_entry.rarity.clone(),
                });
            }
        }
        // Currency
        for currency in &self.currency_rewards {
            let amount = currency.roll_amount(player_level, rng);
            if amount > 0 {
                rewards.push(RolledReward::Currency {
                    currency_id: currency.currency_id.clone(),
                    amount,
                });
            }
        }
        // Reputation
        for rep in &self.reputation_rewards {
            rewards.push(RolledReward::Reputation {
                faction_id: rep.faction_id.clone(),
                delta: rep.calculate_delta(player_level),
            });
        }
        // Unlocks
        for unlock in &self.unlock_rewards {
            rewards.push(RolledReward::Unlock { unlock_id: unlock.unlock_id.clone(), unlock_type: unlock.unlock_type.clone() });
        }
        rewards
    }

    pub fn add_item(&mut self, item_id: &str, quantity_min: u32, quantity_max: u32, drop_chance: f32, rarity: ItemRarity) {
        self.item_rewards.push(ItemRewardEntry {
            item_id: item_id.to_string(),
            quantity_min,
            quantity_max,
            drop_chance,
            rarity,
            quality_min: 0.5,
            quality_max: 1.0,
            quality_weights: Vec::new(),
        });
    }

    pub fn add_xp(&mut self, base: u32, level_scale: f32) {
        self.xp_reward = XpReward { base, level_scale_factor: level_scale, level_cap: 100, diminish_above_level: None };
    }

    pub fn add_currency(&mut self, currency_id: &str, base_min: u32, base_max: u32, level_scale: f32) {
        self.currency_rewards.push(CurrencyReward {
            currency_id: currency_id.to_string(),
            base_amount_min: base_min,
            base_amount_max: base_max,
            level_scale_factor: level_scale,
        });
    }

    pub fn total_estimated_xp(&self, player_level: u32) -> u32 {
        self.xp_reward.calculate(player_level)
    }
}

#[derive(Debug, Clone, Default)]
pub struct XpReward {
    pub base: u32,
    pub level_scale_factor: f32,
    pub level_cap: u32,
    pub diminish_above_level: Option<u32>,
}

impl XpReward {
    pub fn calculate(&self, player_level: u32) -> u32 {
        let level = player_level.min(self.level_cap);
        let scaled = self.base as f32 * (1.0 + level as f32 * self.level_scale_factor);
        if let Some(cap_level) = self.diminish_above_level {
            if player_level > cap_level {
                let excess = (player_level - cap_level) as f32;
                let diminish = 1.0 / (1.0 + excess * 0.1);
                return (scaled * diminish) as u32;
            }
        }
        scaled as u32
    }
}

#[derive(Debug, Clone)]
pub struct ItemRewardEntry {
    pub item_id: String,
    pub quantity_min: u32,
    pub quantity_max: u32,
    pub drop_chance: f32,
    pub rarity: ItemRarity,
    pub quality_min: f32,
    pub quality_max: f32,
    pub quality_weights: Vec<(f32, f32)>,
}

impl ItemRewardEntry {
    pub fn roll_quantity(&self, rng: &mut SimpleRng) -> u32 {
        if self.quantity_min >= self.quantity_max { return self.quantity_min; }
        self.quantity_min + rng.next_u32(self.quantity_max - self.quantity_min + 1)
    }
    pub fn roll_quality(&self, rng: &mut SimpleRng) -> f32 {
        let t = rng.next_f32();
        self.quality_min + t * (self.quality_max - self.quality_min)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

impl ItemRarity {
    pub fn display_color(&self) -> Vec4 {
        match self {
            ItemRarity::Common => Vec4::new(0.8, 0.8, 0.8, 1.0),
            ItemRarity::Uncommon => Vec4::new(0.2, 0.8, 0.2, 1.0),
            ItemRarity::Rare => Vec4::new(0.2, 0.4, 1.0, 1.0),
            ItemRarity::Epic => Vec4::new(0.6, 0.1, 0.9, 1.0),
            ItemRarity::Legendary => Vec4::new(1.0, 0.6, 0.0, 1.0),
            ItemRarity::Mythic => Vec4::new(1.0, 0.0, 0.3, 1.0),
        }
    }

    pub fn drop_chance_modifier(&self) -> f32 {
        match self {
            ItemRarity::Common => 1.0,
            ItemRarity::Uncommon => 0.5,
            ItemRarity::Rare => 0.2,
            ItemRarity::Epic => 0.05,
            ItemRarity::Legendary => 0.01,
            ItemRarity::Mythic => 0.001,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrencyReward {
    pub currency_id: String,
    pub base_amount_min: u32,
    pub base_amount_max: u32,
    pub level_scale_factor: f32,
}

impl CurrencyReward {
    pub fn roll_amount(&self, player_level: u32, rng: &mut SimpleRng) -> u32 {
        let range = self.base_amount_max - self.base_amount_min;
        let base = if range > 0 { self.base_amount_min + rng.next_u32(range + 1) } else { self.base_amount_min };
        let scaled = base as f32 * (1.0 + player_level as f32 * self.level_scale_factor);
        scaled as u32
    }
}

#[derive(Debug, Clone)]
pub struct ReputationReward {
    pub faction_id: String,
    pub base_delta: i32,
    pub level_scale: f32,
    pub related_factions: Vec<(String, f32)>,
}

impl ReputationReward {
    pub fn calculate_delta(&self, player_level: u32) -> i32 {
        let scaled = self.base_delta as f32 * (1.0 + player_level as f32 * self.level_scale);
        scaled as i32
    }
}

#[derive(Debug, Clone)]
pub struct UnlockReward {
    pub unlock_id: String,
    pub unlock_type: UnlockType,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnlockType {
    Ability,
    Recipe,
    Area,
    Item,
    NPC,
    Quest,
}

#[derive(Debug, Clone)]
pub struct SkillReward {
    pub skill_id: String,
    pub skill_name: String,
    pub points: u32,
}

#[derive(Debug, Clone)]
pub enum RolledReward {
    Xp(u32),
    Item { item_id: String, quantity: u32, quality: f32, rarity: ItemRarity },
    Currency { currency_id: String, amount: u32 },
    Reputation { faction_id: String, delta: i32 },
    Unlock { unlock_id: String, unlock_type: UnlockType },
    Skill { skill_id: String, points: u32 },
}

impl RolledReward {
    pub fn display_text(&self) -> String {
        match self {
            RolledReward::Xp(amount) => format!("+{} XP", amount),
            RolledReward::Item { item_id, quantity, rarity, .. } => {
                format!("{} x{} ({:?})", item_id, quantity, rarity)
            }
            RolledReward::Currency { currency_id, amount } => format!("+{} {}", amount, currency_id),
            RolledReward::Reputation { faction_id, delta } => {
                format!("{}: {:+}", faction_id, delta)
            }
            RolledReward::Unlock { unlock_id, .. } => format!("Unlocked: {}", unlock_id),
            RolledReward::Skill { skill_id, points } => format!("{} +{} pts", skill_id, points),
        }
    }
}

// ============================================================
// SECTION 5: QUEST CHAINS & PREREQUISITES
// ============================================================

#[derive(Debug, Clone)]
pub struct QuestChain {
    pub chain_id: String,
    pub chain_name: String,
    pub chain_type: ChainType,
    pub quests: Vec<ChainQuestEntry>,
    pub description: String,
    pub icon: String,
    pub completed: bool,
    pub active_quest_idx: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChainType {
    Sequential,
    Parallel,
    Mutex,
    Branching,
    Repeatable { cooldown: f32 },
}

#[derive(Debug, Clone)]
pub struct ChainQuestEntry {
    pub quest_id: String,
    pub required: bool,
    pub auto_start: bool,
    pub completion_condition: Option<QuestCondition>,
    pub unlock_condition: Option<QuestCondition>,
    pub branch_id: Option<String>,
    pub order: u32,
}

impl QuestChain {
    pub fn new(chain_id: String, name: String, chain_type: ChainType) -> Self {
        QuestChain {
            chain_id,
            chain_name: name,
            chain_type,
            quests: Vec::new(),
            description: String::new(),
            icon: String::new(),
            completed: false,
            active_quest_idx: 0,
        }
    }

    pub fn add_quest(&mut self, quest_id: &str, required: bool) {
        let order = self.quests.len() as u32;
        self.quests.push(ChainQuestEntry {
            quest_id: quest_id.to_string(),
            required,
            auto_start: false,
            completion_condition: None,
            unlock_condition: None,
            branch_id: None,
            order,
        });
    }

    pub fn get_next_quests(&self, state: &QuestStateStore) -> Vec<&str> {
        match &self.chain_type {
            ChainType::Sequential => {
                let mut result = Vec::new();
                for entry in &self.quests {
                    let quest_state = state.get_quest_state(&entry.quest_id);
                    if quest_state == QuestState::NotStarted {
                        // Check if all previous quests are done
                        let prev_done = self.quests.iter()
                            .filter(|e| e.order < entry.order && e.required)
                            .all(|e| state.get_quest_state(&e.quest_id) == QuestState::Completed);
                        if prev_done {
                            result.push(entry.quest_id.as_str());
                        }
                        break;
                    }
                }
                result
            }
            ChainType::Parallel => {
                self.quests.iter()
                    .filter(|e| state.get_quest_state(&e.quest_id) == QuestState::NotStarted)
                    .map(|e| e.quest_id.as_str())
                    .collect()
            }
            ChainType::Mutex => {
                let any_active = self.quests.iter().any(|e| state.get_quest_state(&e.quest_id) == QuestState::Active);
                if any_active { return Vec::new(); }
                self.quests.iter()
                    .filter(|e| state.get_quest_state(&e.quest_id) == QuestState::NotStarted)
                    .map(|e| e.quest_id.as_str())
                    .take(1)
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    pub fn is_chain_complete(&self, state: &QuestStateStore) -> bool {
        match &self.chain_type {
            ChainType::Sequential | ChainType::Parallel => {
                self.quests.iter()
                    .filter(|e| e.required)
                    .all(|e| state.get_quest_state(&e.quest_id) == QuestState::Completed)
            }
            ChainType::Mutex => {
                self.quests.iter()
                    .any(|e| state.get_quest_state(&e.quest_id) == QuestState::Completed)
            }
            _ => false,
        }
    }

    pub fn get_completion_percentage(&self, state: &QuestStateStore) -> f32 {
        if self.quests.is_empty() { return 0.0; }
        let completed = self.quests.iter()
            .filter(|e| state.get_quest_state(&e.quest_id) == QuestState::Completed)
            .count();
        completed as f32 / self.quests.len() as f32
    }

    pub fn get_cooldown_remaining(&self, state: &QuestStateStore, current_time: f32) -> Option<f32> {
        if let ChainType::Repeatable { cooldown } = &self.chain_type {
            let last = state.get_chain_last_completion(&self.chain_id);
            let remaining = cooldown - (current_time - last);
            if remaining > 0.0 { Some(remaining) } else { None }
        } else {
            None
        }
    }
}

// ============================================================
// SECTION 6: FACTION REPUTATION
// ============================================================

#[derive(Debug, Clone)]
pub struct FactionSystem {
    pub factions: HashMap<String, Faction>,
    pub relationships: HashMap<(String, String), FactionRelationship>,
    pub rep_tiers: Vec<RepTier>,
    pub reputation_events: Vec<RepEvent>,
}

#[derive(Debug, Clone)]
pub struct Faction {
    pub id: String,
    pub name: String,
    pub description: String,
    pub reputation: i32,
    pub reputation_min: i32,
    pub reputation_max: i32,
    pub current_tier: ReputationTier,
    pub allied_factions: Vec<String>,
    pub enemy_factions: Vec<String>,
    pub neutral_factions: Vec<String>,
    pub color: Vec4,
    pub icon: String,
    pub is_player_faction: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReputationTier {
    Hated,
    Hostile,
    Unfriendly,
    Neutral,
    Friendly,
    Honored,
    Revered,
    Exalted,
}

impl ReputationTier {
    pub fn from_rep(rep: i32) -> Self {
        match rep {
            i32::MIN..=-3000 => ReputationTier::Hated,
            -2999..=-2000 => ReputationTier::Hostile,
            -1999..=-1000 => ReputationTier::Unfriendly,
            -999..=999 => ReputationTier::Neutral,
            1000..=1999 => ReputationTier::Friendly,
            2000..=2999 => ReputationTier::Honored,
            3000..=3999 => ReputationTier::Revered,
            _ => ReputationTier::Exalted,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            ReputationTier::Hated => "Hated",
            ReputationTier::Hostile => "Hostile",
            ReputationTier::Unfriendly => "Unfriendly",
            ReputationTier::Neutral => "Neutral",
            ReputationTier::Friendly => "Friendly",
            ReputationTier::Honored => "Honored",
            ReputationTier::Revered => "Revered",
            ReputationTier::Exalted => "Exalted",
        }
    }

    pub fn next_tier_threshold(&self) -> Option<i32> {
        match self {
            ReputationTier::Hated => Some(-3000),
            ReputationTier::Hostile => Some(-2000),
            ReputationTier::Unfriendly => Some(-1000),
            ReputationTier::Neutral => Some(1000),
            ReputationTier::Friendly => Some(2000),
            ReputationTier::Honored => Some(3000),
            ReputationTier::Revered => Some(4000),
            ReputationTier::Exalted => None,
        }
    }

    pub fn color(&self) -> Vec4 {
        match self {
            ReputationTier::Hated => Vec4::new(0.8, 0.0, 0.0, 1.0),
            ReputationTier::Hostile => Vec4::new(0.9, 0.2, 0.1, 1.0),
            ReputationTier::Unfriendly => Vec4::new(0.9, 0.5, 0.1, 1.0),
            ReputationTier::Neutral => Vec4::new(0.8, 0.8, 0.8, 1.0),
            ReputationTier::Friendly => Vec4::new(0.4, 0.8, 0.4, 1.0),
            ReputationTier::Honored => Vec4::new(0.2, 0.9, 0.3, 1.0),
            ReputationTier::Revered => Vec4::new(0.2, 0.6, 1.0, 1.0),
            ReputationTier::Exalted => Vec4::new(1.0, 0.9, 0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FactionRelationship {
    pub faction_a: String,
    pub faction_b: String,
    pub relationship_type: RelationshipType,
    pub rep_share_factor: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationshipType {
    Allied,
    Enemy,
    Neutral,
    Rival,
}

#[derive(Debug, Clone)]
pub struct RepTier {
    pub tier: ReputationTier,
    pub min_rep: i32,
    pub unlocks: Vec<String>,
    pub icon: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct RepEvent {
    pub faction_id: String,
    pub delta: i32,
    pub reason: String,
    pub timestamp: f64,
    pub quest_id: Option<String>,
}

impl FactionSystem {
    pub fn new() -> Self {
        let mut sys = FactionSystem {
            factions: HashMap::new(),
            relationships: HashMap::new(),
            rep_tiers: Self::default_tiers(),
            reputation_events: Vec::new(),
        };
        sys
    }

    fn default_tiers() -> Vec<RepTier> {
        vec![
            RepTier { tier: ReputationTier::Hated, min_rep: i32::MIN, unlocks: Vec::new(), icon: "hated".into(), description: "They want you dead".into() },
            RepTier { tier: ReputationTier::Hostile, min_rep: -3000, unlocks: Vec::new(), icon: "hostile".into(), description: "Attack on sight".into() },
            RepTier { tier: ReputationTier::Unfriendly, min_rep: -2000, unlocks: Vec::new(), icon: "unfriendly".into(), description: "Uncomfortable".into() },
            RepTier { tier: ReputationTier::Neutral, min_rep: -1000, unlocks: Vec::new(), icon: "neutral".into(), description: "Neither ally nor enemy".into() },
            RepTier { tier: ReputationTier::Friendly, min_rep: 1000, unlocks: vec!["basic_trade".into()], icon: "friendly".into(), description: "Warmly received".into() },
            RepTier { tier: ReputationTier::Honored, min_rep: 2000, unlocks: vec!["discount_trade".into()], icon: "honored".into(), description: "Deep respect".into() },
            RepTier { tier: ReputationTier::Revered, min_rep: 3000, unlocks: vec!["special_items".into()], icon: "revered".into(), description: "Near legendary status".into() },
            RepTier { tier: ReputationTier::Exalted, min_rep: 4000, unlocks: vec!["exclusive_rewards".into(), "faction_title".into()], icon: "exalted".into(), description: "A true champion".into() },
        ]
    }

    pub fn add_faction(&mut self, faction: Faction) {
        self.factions.insert(faction.id.clone(), faction);
    }

    pub fn apply_rep_delta(&mut self, faction_id: &str, delta: i32, reason: &str, quest_id: Option<&str>) {
        // Apply to main faction
        if let Some(faction) = self.factions.get_mut(faction_id) {
            let new_rep = (faction.reputation + delta).clamp(faction.reputation_min, faction.reputation_max);
            let old_tier = faction.current_tier.clone();
            faction.reputation = new_rep;
            faction.current_tier = ReputationTier::from_rep(new_rep);
            self.reputation_events.push(RepEvent {
                faction_id: faction_id.to_string(),
                delta,
                reason: reason.to_string(),
                timestamp: 0.0,
                quest_id: quest_id.map(|s| s.to_string()),
            });
        }
        // Propagate to related factions
        let related: Vec<(String, f32)> = self.relationships.iter()
            .filter(|((a, b), _)| a == faction_id || b == faction_id)
            .map(|((a, b), rel)| {
                let other = if a == faction_id { b.clone() } else { a.clone() };
                (other, rel.rep_share_factor * if rel.relationship_type == RelationshipType::Enemy { -1.0 } else { 1.0 })
            })
            .collect();
        for (other_id, factor) in related {
            if let Some(other_faction) = self.factions.get_mut(&other_id) {
                let shared_delta = (delta as f32 * factor) as i32;
                let new_rep = (other_faction.reputation + shared_delta).clamp(other_faction.reputation_min, other_faction.reputation_max);
                other_faction.reputation = new_rep;
                other_faction.current_tier = ReputationTier::from_rep(new_rep);
            }
        }
    }

    pub fn get_tier_unlocks(&self, faction_id: &str) -> Vec<String> {
        if let Some(faction) = self.factions.get(faction_id) {
            let tier = &faction.current_tier;
            for rep_tier in &self.rep_tiers {
                if &rep_tier.tier == tier {
                    return rep_tier.unlocks.clone();
                }
            }
        }
        Vec::new()
    }

    pub fn get_relationship(&self, faction_a: &str, faction_b: &str) -> Option<&FactionRelationship> {
        self.relationships.get(&(faction_a.to_string(), faction_b.to_string()))
            .or_else(|| self.relationships.get(&(faction_b.to_string(), faction_a.to_string())))
    }

    pub fn add_relationship(&mut self, faction_a: &str, faction_b: &str, rel_type: RelationshipType, share_factor: f32) {
        self.relationships.insert(
            (faction_a.to_string(), faction_b.to_string()),
            FactionRelationship {
                faction_a: faction_a.to_string(),
                faction_b: faction_b.to_string(),
                relationship_type: rel_type,
                rep_share_factor: share_factor,
            }
        );
    }

    pub fn progress_to_next_tier(&self, faction_id: &str) -> Option<f32> {
        if let Some(faction) = self.factions.get(faction_id) {
            let current_threshold = match &faction.current_tier {
                ReputationTier::Hated => i32::MIN,
                ReputationTier::Hostile => -3000,
                ReputationTier::Unfriendly => -2000,
                ReputationTier::Neutral => -1000,
                ReputationTier::Friendly => 1000,
                ReputationTier::Honored => 2000,
                ReputationTier::Revered => 3000,
                ReputationTier::Exalted => return None,
            };
            if let Some(next) = faction.current_tier.next_tier_threshold() {
                let range = next - current_threshold;
                let progress = faction.reputation - current_threshold;
                return Some((progress as f32 / range as f32).clamp(0.0, 1.0));
            }
        }
        None
    }
}

// ============================================================
// SECTION 7: JOURNAL SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct JournalSystem {
    pub entries: Vec<JournalEntry>,
    pub active_quest_entries: HashMap<String, usize>,
    pub clues: Vec<ClueEntry>,
    pub map_markers: Vec<MapMarker>,
    pub notification_queue: VecDeque<JournalNotification>,
    pub max_entries: usize,
}

#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub entry_id: String,
    pub quest_id: String,
    pub timestamp: f64,
    pub entry_type: JournalEntryType,
    pub title: String,
    pub content: String,
    pub read: bool,
    pub important: bool,
    pub category: JournalCategory,
    pub media: Vec<JournalMedia>,
    pub linked_entries: Vec<String>,
    pub linked_map_markers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JournalEntryType {
    QuestStart,
    QuestUpdate,
    QuestComplete,
    QuestFail,
    ObjectiveComplete,
    ClueDiscovered,
    NPCEncounter,
    LocationDiscovered,
    LoreEntry,
    PlayerNote,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JournalCategory {
    MainQuest,
    SideQuest,
    WorldEvent,
    Character,
    Location,
    Lore,
    Bestiary,
    Crafting,
}

#[derive(Debug, Clone)]
pub struct JournalMedia {
    pub media_type: JournalMediaType,
    pub path: String,
    pub caption: String,
}

#[derive(Debug, Clone)]
pub enum JournalMediaType {
    Image,
    Map,
    Portrait,
    Codex,
}

#[derive(Debug, Clone)]
pub struct ClueEntry {
    pub clue_id: String,
    pub quest_id: String,
    pub title: String,
    pub description: String,
    pub found_at: String,
    pub connects_to: Vec<String>,
    pub is_red_herring: bool,
    pub solved: bool,
    pub location: Option<Vec3>,
}

#[derive(Debug, Clone)]
pub struct MapMarker {
    pub marker_id: String,
    pub quest_id: Option<String>,
    pub marker_type: MapMarkerType,
    pub position: Vec3,
    pub label: String,
    pub description: String,
    pub icon: String,
    pub color: Vec4,
    pub visible: bool,
    pub completed: bool,
    pub tracked: bool,
    pub sub_markers: Vec<MapMarker>,
    pub priority: i32,
}

impl MapMarker {
    pub fn new(id: &str, marker_type: MapMarkerType, position: Vec3, label: &str) -> Self {
        MapMarker {
            marker_id: id.to_string(),
            quest_id: None,
            marker_type,
            position,
            label: label.to_string(),
            description: String::new(),
            icon: String::new(),
            color: Vec4::new(1.0, 0.9, 0.0, 1.0),
            visible: true,
            completed: false,
            tracked: false,
            sub_markers: Vec::new(),
            priority: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MapMarkerType {
    QuestObjective,
    QuestStart,
    QuestEnd,
    NPCLocation,
    ItemLocation,
    DangerZone,
    Discoverable,
    POI,
    FastTravel,
    Custom,
}

#[derive(Debug, Clone)]
pub struct JournalNotification {
    pub notification_type: JournalNotificationType,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub duration: f32,
    pub timestamp: f64,
}

#[derive(Debug, Clone)]
pub enum JournalNotificationType {
    QuestAdded,
    QuestCompleted,
    QuestFailed,
    ObjectiveUpdated,
    ClueFound,
    ReputationChange { faction: String, delta: i32 },
    RewardReceived { rewards: Vec<String> },
}

impl JournalSystem {
    pub fn new() -> Self {
        JournalSystem {
            entries: Vec::new(),
            active_quest_entries: HashMap::new(),
            clues: Vec::new(),
            map_markers: Vec::new(),
            notification_queue: VecDeque::new(),
            max_entries: 1000,
        }
    }

    pub fn add_entry(&mut self, quest_id: &str, entry_type: JournalEntryType, title: &str, content: &str, category: JournalCategory) -> String {
        let entry_id = format!("journal_{}_{}", quest_id, self.entries.len());
        let entry = JournalEntry {
            entry_id: entry_id.clone(),
            quest_id: quest_id.to_string(),
            timestamp: 0.0,
            entry_type: entry_type.clone(),
            title: title.to_string(),
            content: content.to_string(),
            read: false,
            important: false,
            category,
            media: Vec::new(),
            linked_entries: Vec::new(),
            linked_map_markers: Vec::new(),
        };
        let idx = self.entries.len();
        self.active_quest_entries.insert(quest_id.to_string(), idx);
        self.entries.push(entry);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.notification_queue.push_back(JournalNotification {
            notification_type: match entry_type {
                JournalEntryType::QuestStart => JournalNotificationType::QuestAdded,
                JournalEntryType::QuestComplete => JournalNotificationType::QuestCompleted,
                JournalEntryType::QuestFail => JournalNotificationType::QuestFailed,
                _ => JournalNotificationType::ObjectiveUpdated,
            },
            title: title.to_string(),
            description: content.chars().take(80).collect(),
            icon: String::new(),
            duration: 5.0,
            timestamp: 0.0,
        });
        entry_id
    }

    pub fn add_clue(&mut self, clue: ClueEntry) {
        self.clues.push(clue);
    }

    pub fn add_map_marker(&mut self, marker: MapMarker) {
        self.map_markers.push(marker);
    }

    pub fn remove_marker(&mut self, marker_id: &str) {
        self.map_markers.retain(|m| m.marker_id != marker_id);
    }

    pub fn complete_marker(&mut self, marker_id: &str) {
        if let Some(marker) = self.map_markers.iter_mut().find(|m| m.marker_id == marker_id) {
            marker.completed = true;
        }
    }

    pub fn get_entries_for_quest(&self, quest_id: &str) -> Vec<&JournalEntry> {
        self.entries.iter().filter(|e| e.quest_id == quest_id).collect()
    }

    pub fn get_active_markers(&self) -> Vec<&MapMarker> {
        self.map_markers.iter().filter(|m| m.visible && !m.completed).collect()
    }

    pub fn get_clue_web_for_quest(&self, quest_id: &str) -> Vec<(&ClueEntry, Vec<&ClueEntry>)> {
        let quest_clues: Vec<&ClueEntry> = self.clues.iter().filter(|c| c.quest_id == quest_id).collect();
        quest_clues.iter().map(|clue| {
            let connected: Vec<&ClueEntry> = clue.connects_to.iter()
                .filter_map(|id| self.clues.iter().find(|c| &c.clue_id == id))
                .collect();
            (*clue, connected)
        }).collect()
    }

    pub fn mark_as_read(&mut self, entry_id: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.entry_id == entry_id) {
            entry.read = true;
        }
    }

    pub fn unread_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.read).count()
    }

    pub fn get_notification(&mut self) -> Option<JournalNotification> {
        self.notification_queue.pop_front()
    }

    pub fn sort_entries_by_time(&mut self) {
        self.entries.sort_by(|a, b| b.timestamp.partial_cmp(&a.timestamp).unwrap_or(std::cmp::Ordering::Equal));
    }
}

// ============================================================
// SECTION 8: QUEST STATE MACHINE
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QuestState {
    NotStarted,
    Active,
    Paused,
    Completed,
    Failed,
    Abandoned,
}

impl QuestState {
    pub fn can_transition_to(&self, new_state: &QuestState) -> bool {
        match (self, new_state) {
            (QuestState::NotStarted, QuestState::Active) => true,
            (QuestState::Active, QuestState::Paused) => true,
            (QuestState::Active, QuestState::Completed) => true,
            (QuestState::Active, QuestState::Failed) => true,
            (QuestState::Active, QuestState::Abandoned) => true,
            (QuestState::Paused, QuestState::Active) => true,
            (QuestState::Paused, QuestState::Abandoned) => true,
            (QuestState::Failed, QuestState::Active) => true, // retry
            _ => false,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            QuestState::NotStarted => "Not Started",
            QuestState::Active => "Active",
            QuestState::Paused => "Paused",
            QuestState::Completed => "Completed",
            QuestState::Failed => "Failed",
            QuestState::Abandoned => "Abandoned",
        }
    }

    pub fn color(&self) -> Vec4 {
        match self {
            QuestState::NotStarted => Vec4::new(0.5, 0.5, 0.5, 1.0),
            QuestState::Active => Vec4::new(0.2, 0.6, 1.0, 1.0),
            QuestState::Paused => Vec4::new(0.8, 0.7, 0.1, 1.0),
            QuestState::Completed => Vec4::new(0.2, 0.8, 0.2, 1.0),
            QuestState::Failed => Vec4::new(0.9, 0.2, 0.2, 1.0),
            QuestState::Abandoned => Vec4::new(0.4, 0.4, 0.4, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuestInstance {
    pub quest_id: String,
    pub state: QuestState,
    pub started_at: f64,
    pub completed_at: Option<f64>,
    pub failed_at: Option<f64>,
    pub paused_at: Option<f64>,
    pub active_objective_ids: Vec<String>,
    pub completed_objective_ids: Vec<String>,
    pub failed_objective_ids: Vec<String>,
    pub objective_order: VecDeque<String>,
    pub variables: HashMap<String, String>,
    pub retry_count: u32,
    pub partial_score: f32,
    pub tracking: bool,
    pub state_history: Vec<(QuestState, f64)>,
}

impl QuestInstance {
    pub fn new(quest_id: &str) -> Self {
        QuestInstance {
            quest_id: quest_id.to_string(),
            state: QuestState::NotStarted,
            started_at: 0.0,
            completed_at: None,
            failed_at: None,
            paused_at: None,
            active_objective_ids: Vec::new(),
            completed_objective_ids: Vec::new(),
            failed_objective_ids: Vec::new(),
            objective_order: VecDeque::new(),
            variables: HashMap::new(),
            retry_count: 0,
            partial_score: 0.0,
            tracking: false,
            state_history: Vec::new(),
        }
    }

    pub fn transition(&mut self, new_state: QuestState, timestamp: f64) -> Result<(), String> {
        if !self.state.can_transition_to(&new_state) {
            return Err(format!("Cannot transition from {:?} to {:?}", self.state, new_state));
        }
        self.state_history.push((self.state.clone(), timestamp));
        match &new_state {
            QuestState::Active => {
                if self.started_at == 0.0 { self.started_at = timestamp; }
                self.paused_at = None;
            }
            QuestState::Completed => { self.completed_at = Some(timestamp); }
            QuestState::Failed => { self.failed_at = Some(timestamp); }
            QuestState::Paused => { self.paused_at = Some(timestamp); }
            _ => {}
        }
        self.state = new_state;
        Ok(())
    }

    pub fn complete_objective(&mut self, obj_id: &str) {
        self.active_objective_ids.retain(|id| id != obj_id);
        if !self.completed_objective_ids.contains(&obj_id.to_string()) {
            self.completed_objective_ids.push(obj_id.to_string());
        }
    }

    pub fn fail_objective(&mut self, obj_id: &str) {
        self.active_objective_ids.retain(|id| id != obj_id);
        if !self.failed_objective_ids.contains(&obj_id.to_string()) {
            self.failed_objective_ids.push(obj_id.to_string());
        }
    }

    pub fn add_objective(&mut self, obj_id: &str) {
        if !self.active_objective_ids.contains(&obj_id.to_string()) {
            self.active_objective_ids.push(obj_id.to_string());
            self.objective_order.push_back(obj_id.to_string());
        }
    }

    pub fn calculate_completion_score(&self) -> f32 {
        let total = self.completed_objective_ids.len() + self.failed_objective_ids.len();
        if total == 0 { return 0.0; }
        self.completed_objective_ids.len() as f32 / total as f32
    }

    pub fn elapsed_time(&self, current_time: f64) -> f64 {
        if self.started_at == 0.0 { return 0.0; }
        current_time - self.started_at
    }
}

#[derive(Debug, Clone)]
pub struct QuestStateStore {
    pub quest_instances: HashMap<String, QuestInstance>,
    pub faction_reps: HashMap<String, i32>,
    pub player_level: u32,
    pub player_stats: HashMap<String, f32>,
    pub flags: HashSet<String>,
    pub item_inventory: HashMap<String, u32>,
    pub event_times: HashMap<String, f64>,
    pub chain_completions: HashMap<String, f64>,
    pub current_time: f64,
}

impl QuestStateStore {
    pub fn new() -> Self {
        QuestStateStore {
            quest_instances: HashMap::new(),
            faction_reps: HashMap::new(),
            player_level: 1,
            player_stats: HashMap::new(),
            flags: HashSet::new(),
            item_inventory: HashMap::new(),
            event_times: HashMap::new(),
            chain_completions: HashMap::new(),
            current_time: 0.0,
        }
    }

    pub fn get_quest_state(&self, quest_id: &str) -> QuestState {
        self.quest_instances.get(quest_id)
            .map(|q| q.state.clone())
            .unwrap_or(QuestState::NotStarted)
    }

    pub fn start_quest(&mut self, quest_id: &str) -> Result<(), String> {
        let instance = self.quest_instances.entry(quest_id.to_string()).or_insert_with(|| QuestInstance::new(quest_id));
        instance.transition(QuestState::Active, self.current_time)
    }

    pub fn complete_quest(&mut self, quest_id: &str) -> Result<(), String> {
        let time = self.current_time;
        if let Some(instance) = self.quest_instances.get_mut(quest_id) {
            instance.transition(QuestState::Completed, time)
        } else {
            Err(format!("Quest {} not found", quest_id))
        }
    }

    pub fn fail_quest(&mut self, quest_id: &str) -> Result<(), String> {
        let time = self.current_time;
        if let Some(instance) = self.quest_instances.get_mut(quest_id) {
            instance.transition(QuestState::Failed, time)
        } else {
            Err(format!("Quest {} not found", quest_id))
        }
    }

    pub fn get_faction_rep(&self, faction_id: &str) -> i32 {
        self.faction_reps.get(faction_id).copied().unwrap_or(0)
    }

    pub fn modify_faction_rep(&mut self, faction_id: &str, delta: i32) {
        let rep = self.faction_reps.entry(faction_id.to_string()).or_insert(0);
        *rep += delta;
    }

    pub fn get_flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }

    pub fn set_flag(&mut self, name: &str) {
        self.flags.insert(name.to_string());
    }

    pub fn clear_flag(&mut self, name: &str) {
        self.flags.remove(name);
    }

    pub fn get_item_count(&self, item_id: &str) -> u32 {
        self.item_inventory.get(item_id).copied().unwrap_or(0)
    }

    pub fn add_item(&mut self, item_id: &str, quantity: u32) {
        *self.item_inventory.entry(item_id.to_string()).or_insert(0) += quantity;
    }

    pub fn get_player_stat(&self, stat: &str) -> f32 {
        self.player_stats.get(stat).copied().unwrap_or(0.0)
    }

    pub fn get_time_since(&self, event: &str) -> f32 {
        if let Some(&t) = self.event_times.get(event) {
            (self.current_time - t) as f32
        } else {
            f32::MAX
        }
    }

    pub fn record_event(&mut self, event: &str) {
        self.event_times.insert(event.to_string(), self.current_time);
    }

    pub fn is_objective_complete(&self, quest_id: &str, obj_id: &str) -> bool {
        self.quest_instances.get(quest_id)
            .map(|q| q.completed_objective_ids.contains(&obj_id.to_string()))
            .unwrap_or(false)
    }

    pub fn get_chain_last_completion(&self, chain_id: &str) -> f32 {
        self.chain_completions.get(chain_id).copied().unwrap_or(-f64::MAX) as f32
    }

    pub fn get_active_quests(&self) -> Vec<&QuestInstance> {
        self.quest_instances.values().filter(|q| q.state == QuestState::Active).collect()
    }

    pub fn get_completed_quests(&self) -> Vec<&QuestInstance> {
        self.quest_instances.values().filter(|q| q.state == QuestState::Completed).collect()
    }
}

// ============================================================
// SECTION 9: DEBUG TOOLS
// ============================================================

#[derive(Debug, Clone)]
pub struct QuestDebugTools {
    pub log: VecDeque<DebugLogEntry>,
    pub max_log: usize,
    pub breakpoints: HashSet<String>,
    pub watch_objectives: Vec<String>,
    pub force_complete_queue: VecDeque<String>,
    pub force_fail_queue: VecDeque<String>,
    pub override_conditions: HashMap<String, bool>,
    pub reputation_inspector: ReputationInspector,
    pub condition_tester: ConditionTester,
    pub objective_overrides: HashMap<String, ObjectiveOverride>,
    pub slow_motion: bool,
    pub slow_motion_scale: f32,
    pub record_events: bool,
    pub recorded_events: Vec<QuestEvent>,
    pub current_scope_dump: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct DebugLogEntry {
    pub level: LogLevel,
    pub timestamp: f64,
    pub quest_id: Option<String>,
    pub message: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Verbose,
}

#[derive(Debug, Clone)]
pub struct ReputationInspector {
    pub selected_faction: Option<String>,
    pub show_history: bool,
    pub history_count: usize,
    pub filter_quest_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConditionTester {
    pub test_conditions: Vec<(String, QuestCondition)>,
    pub results: Vec<(String, bool)>,
    pub test_state: QuestStateStore,
    pub running: bool,
}

impl ConditionTester {
    pub fn new() -> Self {
        ConditionTester {
            test_conditions: Vec::new(),
            results: Vec::new(),
            test_state: QuestStateStore::new(),
            running: false,
        }
    }

    pub fn run_all(&mut self) {
        self.results.clear();
        for (name, cond) in &self.test_conditions.clone() {
            let result = cond.evaluate(&self.test_state);
            self.results.push((name.clone(), result));
        }
    }

    pub fn add_test(&mut self, name: &str, condition: QuestCondition) {
        self.test_conditions.push((name.to_string(), condition));
    }

    pub fn get_pass_rate(&self) -> f32 {
        if self.results.is_empty() { return 0.0; }
        let passed = self.results.iter().filter(|(_, r)| *r).count();
        passed as f32 / self.results.len() as f32
    }
}

#[derive(Debug, Clone)]
pub struct ObjectiveOverride {
    pub objective_id: String,
    pub force_status: ObjectiveStatus,
    pub force_progress: Option<f32>,
    pub active: bool,
}

impl QuestDebugTools {
    pub fn new() -> Self {
        QuestDebugTools {
            log: VecDeque::new(),
            max_log: 500,
            breakpoints: HashSet::new(),
            watch_objectives: Vec::new(),
            force_complete_queue: VecDeque::new(),
            force_fail_queue: VecDeque::new(),
            override_conditions: HashMap::new(),
            reputation_inspector: ReputationInspector {
                selected_faction: None,
                show_history: false,
                history_count: 20,
                filter_quest_id: None,
            },
            condition_tester: ConditionTester::new(),
            objective_overrides: HashMap::new(),
            slow_motion: false,
            slow_motion_scale: 0.1,
            record_events: false,
            recorded_events: Vec::new(),
            current_scope_dump: Vec::new(),
        }
    }

    pub fn log(&mut self, level: LogLevel, message: &str, quest_id: Option<&str>, source: &str) {
        self.log.push_back(DebugLogEntry {
            level,
            timestamp: 0.0,
            quest_id: quest_id.map(|s| s.to_string()),
            message: message.to_string(),
            source: source.to_string(),
        });
        if self.log.len() > self.max_log {
            self.log.pop_front();
        }
    }

    pub fn force_complete_quest(&mut self, quest_id: &str) {
        self.force_complete_queue.push_back(quest_id.to_string());
        self.log(LogLevel::Warning, &format!("Force completing: {}", quest_id), Some(quest_id), "debug");
    }

    pub fn force_fail_quest(&mut self, quest_id: &str) {
        self.force_fail_queue.push_back(quest_id.to_string());
        self.log(LogLevel::Warning, &format!("Force failing: {}", quest_id), Some(quest_id), "debug");
    }

    pub fn set_condition_override(&mut self, condition_id: &str, value: bool) {
        self.override_conditions.insert(condition_id.to_string(), value);
    }

    pub fn clear_condition_override(&mut self, condition_id: &str) {
        self.override_conditions.remove(condition_id);
    }

    pub fn override_objective(&mut self, obj_id: &str, status: ObjectiveStatus, progress: Option<f32>) {
        self.objective_overrides.insert(obj_id.to_string(), ObjectiveOverride {
            objective_id: obj_id.to_string(),
            force_status: status,
            force_progress: progress,
            active: true,
        });
    }

    pub fn clear_override(&mut self, obj_id: &str) {
        if let Some(ov) = self.objective_overrides.get_mut(obj_id) {
            ov.active = false;
        }
    }

    pub fn dump_state(&mut self, state: &QuestStateStore) {
        self.current_scope_dump.clear();
        self.current_scope_dump.push(("player_level".to_string(), state.player_level.to_string()));
        for (faction, rep) in &state.faction_reps {
            self.current_scope_dump.push((format!("rep_{}", faction), rep.to_string()));
        }
        for (item, qty) in &state.item_inventory {
            self.current_scope_dump.push((format!("item_{}", item), qty.to_string()));
        }
        for flag in &state.flags {
            self.current_scope_dump.push((format!("flag_{}", flag), "true".to_string()));
        }
        self.current_scope_dump.sort_by(|a, b| a.0.cmp(&b.0));
    }

    pub fn get_log_for_quest(&self, quest_id: &str) -> Vec<&DebugLogEntry> {
        self.log.iter().filter(|e| e.quest_id.as_deref() == Some(quest_id)).collect()
    }

    pub fn clear_log(&mut self) { self.log.clear(); }

    pub fn get_error_count(&self) -> usize {
        self.log.iter().filter(|e| e.level == LogLevel::Error).count()
    }

    pub fn get_warning_count(&self) -> usize {
        self.log.iter().filter(|e| e.level == LogLevel::Warning).count()
    }
}

// ============================================================
// SECTION 10: QUEST GRAPH & FULL QUEST EDITOR
// ============================================================

#[derive(Debug, Clone)]
pub enum QuestNodeEnum {
    QuestStart(QuestStartNode),
    Objective(ObjectiveNode),
    ConditionCheck(ConditionCheckNode),
    Reward(RewardNode),
    Branch(BranchNode),
    Fail(FailNode),
    Completion(CompletionNode),
    Timer(TimerNode),
    Trigger(TriggerNode),
}

impl QuestNodeEnum {
    pub fn id(&self) -> u64 {
        match self {
            QuestNodeEnum::QuestStart(n) => n.id,
            QuestNodeEnum::Objective(n) => n.id,
            QuestNodeEnum::ConditionCheck(n) => n.id,
            QuestNodeEnum::Reward(n) => n.id,
            QuestNodeEnum::Branch(n) => n.id,
            QuestNodeEnum::Fail(n) => n.id,
            QuestNodeEnum::Completion(n) => n.id,
            QuestNodeEnum::Timer(n) => n.id,
            QuestNodeEnum::Trigger(n) => n.id,
        }
    }

    pub fn position(&self) -> Vec2 {
        match self {
            QuestNodeEnum::QuestStart(n) => n.position,
            QuestNodeEnum::Objective(n) => n.position,
            QuestNodeEnum::ConditionCheck(n) => n.position,
            QuestNodeEnum::Reward(n) => n.position,
            QuestNodeEnum::Branch(n) => n.position,
            QuestNodeEnum::Fail(n) => n.position,
            QuestNodeEnum::Completion(n) => n.position,
            QuestNodeEnum::Timer(n) => n.position,
            QuestNodeEnum::Trigger(n) => n.position,
        }
    }

    pub fn set_position(&mut self, pos: Vec2) {
        match self {
            QuestNodeEnum::QuestStart(n) => n.position = pos,
            QuestNodeEnum::Objective(n) => n.position = pos,
            QuestNodeEnum::ConditionCheck(n) => n.position = pos,
            QuestNodeEnum::Reward(n) => n.position = pos,
            QuestNodeEnum::Branch(n) => n.position = pos,
            QuestNodeEnum::Fail(n) => n.position = pos,
            QuestNodeEnum::Completion(n) => n.position = pos,
            QuestNodeEnum::Timer(n) => n.position = pos,
            QuestNodeEnum::Trigger(n) => n.position = pos,
        }
    }

    pub fn node_type(&self) -> QuestNodeType {
        match self {
            QuestNodeEnum::QuestStart(_) => QuestNodeType::QuestStart,
            QuestNodeEnum::Objective(_) => QuestNodeType::Objective,
            QuestNodeEnum::ConditionCheck(_) => QuestNodeType::ConditionCheck,
            QuestNodeEnum::Reward(_) => QuestNodeType::Reward,
            QuestNodeEnum::Branch(_) => QuestNodeType::Branch,
            QuestNodeEnum::Fail(_) => QuestNodeType::Fail,
            QuestNodeEnum::Completion(_) => QuestNodeType::Completion,
            QuestNodeEnum::Timer(_) => QuestNodeType::Timer,
            QuestNodeEnum::Trigger(_) => QuestNodeType::Trigger,
        }
    }

    pub fn tags(&self) -> &[String] {
        match self {
            QuestNodeEnum::QuestStart(n) => &n.tags,
            QuestNodeEnum::Objective(n) => &n.tags,
            QuestNodeEnum::ConditionCheck(n) => &n.tags,
            QuestNodeEnum::Reward(n) => &n.tags,
            QuestNodeEnum::Branch(n) => &n.tags,
            QuestNodeEnum::Fail(n) => &n.tags,
            QuestNodeEnum::Completion(n) => &n.tags,
            QuestNodeEnum::Timer(n) => &n.tags,
            QuestNodeEnum::Trigger(n) => &n.tags,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuestNodeConnection {
    pub from_node: u64,
    pub from_port: usize,
    pub to_node: u64,
    pub to_port: usize,
    pub label: Option<String>,
    pub condition: Option<QuestCondition>,
}

#[derive(Debug, Clone)]
pub struct QuestGraph {
    pub id: String,
    pub quest_id: String,
    pub name: String,
    pub nodes: HashMap<u64, QuestNodeEnum>,
    pub connections: Vec<QuestNodeConnection>,
    pub start_node_id: Option<u64>,
    pub metadata: QuestGraphMetadata,
    next_id: u64,
}

#[derive(Debug, Clone, Default)]
pub struct QuestGraphMetadata {
    pub author: String,
    pub version: String,
    pub created: String,
    pub modified: String,
    pub description: String,
    pub tags: Vec<String>,
    pub category: String,
    pub reward_summary: String,
}

impl QuestGraph {
    pub fn new(id: String, quest_id: String, name: String) -> Self {
        QuestGraph {
            id,
            quest_id,
            name,
            nodes: HashMap::new(),
            connections: Vec::new(),
            start_node_id: None,
            metadata: QuestGraphMetadata::default(),
            next_id: 1,
        }
    }

    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_node(&mut self, node: QuestNodeEnum) -> u64 {
        let id = node.id();
        if let QuestNodeEnum::QuestStart(_) = &node {
            self.start_node_id = Some(id);
        }
        self.nodes.insert(id, node);
        id
    }

    pub fn remove_node(&mut self, id: u64) {
        self.nodes.remove(&id);
        self.connections.retain(|c| c.from_node != id && c.to_node != id);
        if self.start_node_id == Some(id) { self.start_node_id = None; }
    }

    pub fn connect(&mut self, from: u64, from_port: usize, to: u64, to_port: usize, label: Option<String>) {
        self.connections.retain(|c| !(c.from_node == from && c.from_port == from_port));
        self.connections.push(QuestNodeConnection {
            from_node: from, from_port,
            to_node: to, to_port,
            label, condition: None,
        });
    }

    pub fn get_outputs(&self, node_id: u64) -> Vec<&QuestNodeConnection> {
        self.connections.iter().filter(|c| c.from_node == node_id).collect()
    }

    pub fn get_inputs(&self, node_id: u64) -> Vec<&QuestNodeConnection> {
        self.connections.iter().filter(|c| c.to_node == node_id).collect()
    }

    pub fn next_node(&self, from: u64, port: usize) -> Option<u64> {
        self.connections.iter()
            .find(|c| c.from_node == from && c.from_port == port)
            .map(|c| c.to_node)
    }

    pub fn validate(&self) -> Vec<QuestValidationError> {
        let mut errors = Vec::new();
        if self.start_node_id.is_none() {
            errors.push(QuestValidationError {
                node_id: None,
                severity: QuestValidationSeverity::Error,
                message: "Quest has no start node".into(),
            });
        }
        for (id, node) in &self.nodes {
            match node {
                QuestNodeEnum::Objective(n) => {
                    if self.get_outputs(*id).is_empty() {
                        errors.push(QuestValidationError {
                            node_id: Some(*id),
                            severity: QuestValidationSeverity::Warning,
                            message: "Objective node has no output".into(),
                        });
                    }
                }
                QuestNodeEnum::ConditionCheck(n) => {
                    let has_true = self.connections.iter().any(|c| c.from_node == *id && c.from_port == 0);
                    let has_false = self.connections.iter().any(|c| c.from_node == *id && c.from_port == 1);
                    if !has_true {
                        errors.push(QuestValidationError {
                            node_id: Some(*id),
                            severity: QuestValidationSeverity::Warning,
                            message: "ConditionCheck missing true branch".into(),
                        });
                    }
                    if !has_false {
                        errors.push(QuestValidationError {
                            node_id: Some(*id),
                            severity: QuestValidationSeverity::Warning,
                            message: "ConditionCheck missing false branch".into(),
                        });
                    }
                }
                QuestNodeEnum::Reward(n) => {
                    if n.reward_table.xp_reward.base == 0 && n.reward_table.item_rewards.is_empty() {
                        errors.push(QuestValidationError {
                            node_id: Some(*id),
                            severity: QuestValidationSeverity::Info,
                            message: "Reward node has no rewards defined".into(),
                        });
                    }
                }
                _ => {}
            }
        }
        // Check unreachable nodes
        if let Some(start) = self.start_node_id {
            let reachable = self.find_reachable(start);
            for id in self.nodes.keys() {
                if !reachable.contains(id) {
                    errors.push(QuestValidationError {
                        node_id: Some(*id),
                        severity: QuestValidationSeverity::Warning,
                        message: "Unreachable node".into(),
                    });
                }
            }
        }
        errors
    }

    pub fn find_reachable(&self, start: u64) -> HashSet<u64> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);
        while let Some(id) = queue.pop_front() {
            if visited.contains(&id) { continue; }
            visited.insert(id);
            for conn in self.get_outputs(id) {
                queue.push_back(conn.to_node);
            }
        }
        visited
    }

    pub fn auto_layout(&mut self) {
        if let Some(start) = self.start_node_id {
            let node_w = 220.0_f32;
            let node_h = 120.0_f32;
            let x_gap = 60.0_f32;
            let y_gap = 30.0_f32;
            let mut queue = VecDeque::new();
            let mut visited = HashSet::new();
            let mut col_row: HashMap<usize, usize> = HashMap::new();
            let mut positions: HashMap<u64, Vec2> = HashMap::new();
            queue.push_back((start, 0usize));
            while let Some((id, col)) = queue.pop_front() {
                if visited.contains(&id) { continue; }
                visited.insert(id);
                let row = *col_row.entry(col).or_insert(0);
                *col_row.entry(col).or_insert(0) += 1;
                positions.insert(id, Vec2::new(
                    col as f32 * (node_w + x_gap),
                    row as f32 * (node_h + y_gap),
                ));
                for conn in self.get_outputs(id) {
                    queue.push_back((conn.to_node, col + 1));
                }
            }
            for (id, pos) in positions {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.set_position(pos);
                }
            }
        }
    }

    pub fn collect_all_objectives(&self) -> Vec<&ObjectiveNode> {
        self.nodes.values().filter_map(|n| {
            if let QuestNodeEnum::Objective(obj) = n { Some(obj) } else { None }
        }).collect()
    }

    pub fn collect_all_rewards(&self) -> Vec<&RewardNode> {
        self.nodes.values().filter_map(|n| {
            if let QuestNodeEnum::Reward(r) = n { Some(r) } else { None }
        }).collect()
    }
}

#[derive(Debug, Clone)]
pub struct QuestValidationError {
    pub node_id: Option<u64>,
    pub severity: QuestValidationSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuestValidationSeverity {
    Info,
    Warning,
    Error,
}

// ============================================================
// FULL QUEST EDITOR STRUCT
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct QuestEditorGraphState {
    pub pan_offset: Vec2,
    pub zoom: f32,
    pub canvas_size: Vec2,
    pub selected_nodes: HashSet<u64>,
    pub hovered_node: Option<u64>,
    pub dragging_node: Option<u64>,
    pub drag_start_pos: Vec2,
    pub drag_node_start: Vec2,
    pub connection_drag_from: Option<(u64, usize)>,
    pub connection_drag_pos: Vec2,
    pub selection_box_start: Option<Vec2>,
    pub selection_box_end: Vec2,
    pub grid_size: f32,
    pub snap_to_grid: bool,
    pub show_grid: bool,
    pub context_menu_pos: Option<Vec2>,
    pub context_menu_node: Option<u64>,
    pub bezier_smoothness: f32,
}

impl QuestEditorGraphState {
    pub fn new() -> Self {
        QuestEditorGraphState {
            zoom: 1.0,
            canvas_size: Vec2::new(1600.0, 900.0),
            grid_size: 20.0,
            snap_to_grid: true,
            show_grid: true,
            bezier_smoothness: 0.5,
            ..Default::default()
        }
    }

    pub fn world_to_screen(&self, world: Vec2) -> Vec2 { (world + self.pan_offset) * self.zoom }
    pub fn screen_to_world(&self, screen: Vec2) -> Vec2 { screen / self.zoom - self.pan_offset }

    pub fn snap(&self, pos: Vec2) -> Vec2 {
        if self.snap_to_grid {
            Vec2::new(
                (pos.x / self.grid_size).round() * self.grid_size,
                (pos.y / self.grid_size).round() * self.grid_size,
            )
        } else { pos }
    }

    pub fn zoom_at(&mut self, point: Vec2, delta: f32) {
        let world = self.screen_to_world(point);
        self.zoom = (self.zoom * (1.0 + delta * 0.1)).clamp(0.05, 5.0);
        let new_world = self.screen_to_world(point);
        self.pan_offset += new_world - world;
    }
}

#[derive(Debug)]
pub struct QuestEditor {
    pub graphs: HashMap<String, QuestGraph>,
    pub active_graph_id: Option<String>,
    pub graph_state: QuestEditorGraphState,
    pub quest_state_store: QuestStateStore,
    pub faction_system: FactionSystem,
    pub journal: JournalSystem,
    pub chains: HashMap<String, QuestChain>,
    pub debug_tools: QuestDebugTools,
    pub undo_stack: VecDeque<QuestEditorAction>,
    pub redo_stack: VecDeque<QuestEditorAction>,
    pub max_undo: usize,
    pub validation_errors: Vec<QuestValidationError>,
    pub search_query: String,
    pub search_results: Vec<QuestSearchResult>,
    pub selected_chain_id: Option<String>,
    pub show_journal: bool,
    pub show_debug: bool,
    pub show_factions: bool,
    pub show_chains: bool,
    pub show_validation: bool,
    pub show_reward_preview: bool,
    pub show_statistics: bool,
    pub node_id_counter: u64,
    pub dirty: bool,
    pub status_message: String,
    pub status_timer: f32,
    pub rng: SimpleRng,
    pub panel_config: QuestEditorPanels,
    pub recent_files: Vec<String>,
    pub last_save_path: Option<String>,
    pub event_log: VecDeque<QuestEvent>,
    pub max_event_log: usize,
    pub preview_player_level: u32,
    pub reward_preview_results: Vec<RolledReward>,
    pub graph_annotations: Vec<QuestAnnotation>,
}

#[derive(Debug, Clone)]
pub struct QuestEditorPanels {
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_height: f32,
    pub show_left: bool,
    pub show_right: bool,
    pub show_bottom: bool,
}

impl Default for QuestEditorPanels {
    fn default() -> Self {
        QuestEditorPanels {
            left_width: 260.0,
            right_width: 320.0,
            bottom_height: 180.0,
            show_left: true,
            show_right: true,
            show_bottom: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuestSearchResult {
    pub graph_id: String,
    pub node_id: u64,
    pub field: String,
    pub excerpt: String,
}

#[derive(Debug, Clone)]
pub struct QuestAnnotation {
    pub id: u64,
    pub position: Vec2,
    pub size: Vec2,
    pub title: String,
    pub text: String,
    pub color: Vec4,
    pub collapsed: bool,
    pub graph_id: String,
}

impl QuestAnnotation {
    pub fn new(id: u64, graph_id: &str, position: Vec2) -> Self {
        QuestAnnotation {
            id,
            position,
            size: Vec2::new(200.0, 100.0),
            title: "Note".to_string(),
            text: String::new(),
            color: Vec4::new(0.8, 0.9, 0.6, 0.5),
            collapsed: false,
            graph_id: graph_id.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum QuestEditorAction {
    AddNode { graph_id: String, node: QuestNodeEnum },
    RemoveNode { graph_id: String, node: QuestNodeEnum, connections: Vec<QuestNodeConnection> },
    MoveNode { graph_id: String, node_id: u64, old_pos: Vec2, new_pos: Vec2 },
    AddConnection { graph_id: String, connection: QuestNodeConnection },
    RemoveConnection { graph_id: String, connection: QuestNodeConnection },
    EditObjective { graph_id: String, node_id: u64, old: QuestObjective, new: QuestObjective },
    AddReward { graph_id: String, node_id: u64, reward: ItemRewardEntry },
    EditStartNode { graph_id: String, node_id: u64, old: QuestStartNode, new: QuestStartNode },
    BatchAction(Vec<QuestEditorAction>),
}

impl QuestEditor {
    pub fn new() -> Self {
        let mut faction_system = FactionSystem::new();
        // Add some sample factions
        faction_system.add_faction(Faction {
            id: "merchants_guild".to_string(),
            name: "Merchants Guild".to_string(),
            description: "A powerful trading consortium".to_string(),
            reputation: 0,
            reputation_min: -5000,
            reputation_max: 5000,
            current_tier: ReputationTier::Neutral,
            allied_factions: vec!["craftsmen".to_string()],
            enemy_factions: vec!["thieves_guild".to_string()],
            neutral_factions: vec!["city_guard".to_string()],
            color: Vec4::new(0.8, 0.6, 0.1, 1.0),
            icon: "merchant_icon".to_string(),
            is_player_faction: false,
        });
        QuestEditor {
            graphs: HashMap::new(),
            active_graph_id: None,
            graph_state: QuestEditorGraphState::new(),
            quest_state_store: QuestStateStore::new(),
            faction_system,
            journal: JournalSystem::new(),
            chains: HashMap::new(),
            debug_tools: QuestDebugTools::new(),
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_undo: 100,
            validation_errors: Vec::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            selected_chain_id: None,
            show_journal: false,
            show_debug: false,
            show_factions: false,
            show_chains: false,
            show_validation: false,
            show_reward_preview: false,
            show_statistics: false,
            node_id_counter: 1000,
            dirty: false,
            status_message: "Ready".to_string(),
            status_timer: 0.0,
            rng: SimpleRng::new(12345),
            panel_config: QuestEditorPanels::default(),
            recent_files: Vec::new(),
            last_save_path: None,
            event_log: VecDeque::new(),
            max_event_log: 500,
            preview_player_level: 10,
            reward_preview_results: Vec::new(),
            graph_annotations: Vec::new(),
        }
    }

    pub fn allocate_id(&mut self) -> u64 {
        self.node_id_counter += 1;
        self.node_id_counter
    }

    pub fn active_graph(&self) -> Option<&QuestGraph> {
        self.active_graph_id.as_ref().and_then(|id| self.graphs.get(id))
    }

    pub fn active_graph_mut(&mut self) -> Option<&mut QuestGraph> {
        self.active_graph_id.as_ref().and_then(|id| self.graphs.get_mut(id))
    }

    pub fn new_quest(&mut self, quest_name: &str, quest_id: &str) -> String {
        let graph_id = format!("graph_{}", self.allocate_id());
        let mut graph = QuestGraph::new(graph_id.clone(), quest_id.to_string(), quest_name.to_string());
        // Create default start node
        let start_id = self.allocate_id();
        let mut start = QuestStartNode::new(start_id);
        start.quest_id = quest_id.to_string();
        start.quest_name = quest_name.to_string();
        start.position = Vec2::new(100.0, 200.0);
        graph.add_node(QuestNodeEnum::QuestStart(start));
        self.graphs.insert(graph_id.clone(), graph);
        self.active_graph_id = Some(graph_id.clone());
        self.dirty = true;
        graph_id
    }

    pub fn open_graph(&mut self, id: &str) {
        if self.graphs.contains_key(id) {
            self.active_graph_id = Some(id.to_string());
            self.validate_active();
        }
    }

    pub fn add_node(&mut self, node_type: QuestNodeType, position: Vec2) -> Option<u64> {
        let id = self.allocate_id();
        let node = self.create_node(node_type, id, position);
        let nid = node.id();
        let graph_id = self.active_graph_id.clone();
        if let Some(gid) = graph_id {
            let action = QuestEditorAction::AddNode { graph_id: gid, node: node.clone() };
            self.push_undo(action);
            if let Some(graph) = self.active_graph_mut() {
                graph.add_node(node);
                self.dirty = true;
            }
            Some(nid)
        } else {
            None
        }
    }

    fn create_node(&self, node_type: QuestNodeType, id: u64, position: Vec2) -> QuestNodeEnum {
        match node_type {
            QuestNodeType::QuestStart => {
                let mut n = QuestStartNode::new(id);
                n.position = position;
                QuestNodeEnum::QuestStart(n)
            }
            QuestNodeType::Objective => {
                let obj = QuestObjective::KillEnemy(KillEnemyObjective {
                    enemy_type: "any".to_string(),
                    enemy_id: None,
                    required_count: 1,
                    killed_count: 0,
                    in_area: None,
                    with_weapon_type: None,
                    require_stealth_kill: false,
                    allow_assists: true,
                });
                let mut n = ObjectiveNode::new(id, obj);
                n.position = position;
                QuestNodeEnum::Objective(n)
            }
            QuestNodeType::ConditionCheck => {
                let mut n = ConditionCheckNode::new(id);
                n.position = position;
                QuestNodeEnum::ConditionCheck(n)
            }
            QuestNodeType::Reward => {
                let mut n = RewardNode::new(id);
                n.position = position;
                QuestNodeEnum::Reward(n)
            }
            QuestNodeType::Branch => {
                let mut n = BranchNode::new(id);
                n.position = position;
                QuestNodeEnum::Branch(n)
            }
            QuestNodeType::Fail => {
                let mut n = FailNode::new(id);
                n.position = position;
                QuestNodeEnum::Fail(n)
            }
            QuestNodeType::Completion => {
                let mut n = CompletionNode::new(id);
                n.position = position;
                QuestNodeEnum::Completion(n)
            }
            QuestNodeType::Timer => {
                let mut n = TimerNode::new(id);
                n.position = position;
                QuestNodeEnum::Timer(n)
            }
            QuestNodeType::Trigger => {
                let mut n = TriggerNode::new(id);
                n.position = position;
                QuestNodeEnum::Trigger(n)
            }
        }
    }

    pub fn remove_selected_nodes(&mut self) {
        let selected: Vec<u64> = self.graph_state.selected_nodes.iter().cloned().collect();
        for id in &selected {
            let action_opt = if let Some(graph) = self.active_graph_mut() {
                if let Some(node) = graph.nodes.get(id).cloned() {
                    let conns: Vec<QuestNodeConnection> = graph.connections.iter()
                        .filter(|c| c.from_node == *id || c.to_node == *id)
                        .cloned()
                        .collect();
                    Some(QuestEditorAction::RemoveNode { graph_id: graph.id.clone(), node, connections: conns })
                } else { None }
            } else { None };
            if let Some(action) = action_opt {
                self.push_undo(action);
                if let Some(graph) = self.active_graph_mut() {
                    graph.remove_node(*id);
                }
            }
        }
        self.graph_state.selected_nodes.clear();
        self.dirty = true;
    }

    pub fn connect_nodes(&mut self, from: u64, from_port: usize, to: u64, to_port: usize) {
        let conn = QuestNodeConnection { from_node: from, from_port, to_node: to, to_port, label: None, condition: None };
        if let Some(gid) = self.active_graph_id.clone() {
            let action = QuestEditorAction::AddConnection { graph_id: gid, connection: conn.clone() };
            self.push_undo(action);
            if let Some(graph) = self.active_graph_mut() {
                graph.connect(from, from_port, to, to_port, None);
                self.dirty = true;
            }
        }
    }

    fn push_undo(&mut self, action: QuestEditorAction) {
        self.redo_stack.clear();
        self.undo_stack.push_back(action);
        if self.undo_stack.len() > self.max_undo {
            self.undo_stack.pop_front();
        }
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop_back() {
            self.redo_stack.push_back(action.clone());
            self.apply_undo(action);
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop_back() {
            self.undo_stack.push_back(action.clone());
            self.apply_redo(action);
        }
    }

    fn apply_undo(&mut self, action: QuestEditorAction) {
        match action {
            QuestEditorAction::AddNode { graph_id, node } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) { g.remove_node(node.id()); }
            }
            QuestEditorAction::RemoveNode { graph_id, node, connections } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) {
                    g.add_node(node);
                    for conn in connections { g.connections.push(conn); }
                }
            }
            QuestEditorAction::MoveNode { graph_id, node_id, old_pos, .. } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) {
                    if let Some(n) = g.nodes.get_mut(&node_id) { n.set_position(old_pos); }
                }
            }
            QuestEditorAction::AddConnection { graph_id, connection } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) {
                    g.connections.retain(|c| !(c.from_node == connection.from_node && c.from_port == connection.from_port));
                }
            }
            QuestEditorAction::RemoveConnection { graph_id, connection } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) { g.connections.push(connection); }
            }
            QuestEditorAction::BatchAction(actions) => {
                for a in actions.into_iter().rev() { self.apply_undo(a); }
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn apply_redo(&mut self, action: QuestEditorAction) {
        match action {
            QuestEditorAction::AddNode { graph_id, node } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) { g.add_node(node); }
            }
            QuestEditorAction::RemoveNode { graph_id, node, .. } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) { g.remove_node(node.id()); }
            }
            QuestEditorAction::MoveNode { graph_id, node_id, new_pos, .. } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) {
                    if let Some(n) = g.nodes.get_mut(&node_id) { n.set_position(new_pos); }
                }
            }
            QuestEditorAction::AddConnection { graph_id, connection } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) {
                    g.connect(connection.from_node, connection.from_port, connection.to_node, connection.to_port, connection.label);
                }
            }
            QuestEditorAction::RemoveConnection { graph_id, connection } => {
                if let Some(g) = self.graphs.get_mut(&graph_id) {
                    g.connections.retain(|c| !(c.from_node == connection.from_node && c.from_port == connection.from_port));
                }
            }
            QuestEditorAction::BatchAction(actions) => {
                for a in actions { self.apply_redo(a); }
            }
            _ => {}
        }
        self.dirty = true;
    }

    pub fn validate_active(&mut self) {
        if let Some(graph) = self.active_graph() {
            self.validation_errors = graph.validate();
        }
    }

    pub fn update(&mut self, delta: f32) {
        self.status_timer -= delta;
        if self.status_timer < 0.0 { self.status_timer = 0.0; }
        // Process debug queues
        while let Some(quest_id) = self.debug_tools.force_complete_queue.pop_front() {
            let _ = self.quest_state_store.complete_quest(&quest_id);
            self.journal.add_entry(&quest_id, JournalEntryType::QuestComplete,
                "Quest Complete", "Force completed by debug tools", JournalCategory::MainQuest);
        }
        while let Some(quest_id) = self.debug_tools.force_fail_queue.pop_front() {
            let _ = self.quest_state_store.fail_quest(&quest_id);
            self.journal.add_entry(&quest_id, JournalEntryType::QuestFail,
                "Quest Failed", "Force failed by debug tools", JournalCategory::MainQuest);
        }
        self.quest_state_store.current_time += delta as f64;
    }

    pub fn process_event(&mut self, event: QuestEvent) {
        if self.debug_tools.record_events {
            self.debug_tools.recorded_events.push(event.clone());
        }
        self.event_log.push_back(event.clone());
        if self.event_log.len() > self.max_event_log {
            self.event_log.pop_front();
        }
        self.debug_tools.log(LogLevel::Verbose, &format!("Event: {:?}", std::mem::discriminant(&event)), None, "event_system");
    }

    pub fn set_status(&mut self, msg: &str, duration: f32) {
        self.status_message = msg.to_string();
        self.status_timer = duration;
    }

    pub fn preview_rewards(&mut self, node_id: u64) {
        let reward_node_clone = self.active_graph()
            .and_then(|g| g.nodes.get(&node_id))
            .and_then(|n| if let QuestNodeEnum::Reward(r) = n { Some(r.clone()) } else { None });
        if let Some(reward_node) = reward_node_clone {
            let results = reward_node.roll_rewards(self.preview_player_level, &mut self.rng);
            self.reward_preview_results = results;
            self.show_reward_preview = true;
        }
    }

    pub fn search(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.search_results.clear();
        let query_lower = query.to_lowercase();
        for (graph_id, graph) in &self.graphs {
            for (node_id, node) in &graph.nodes {
                match node {
                    QuestNodeEnum::QuestStart(n) => {
                        if n.quest_name.to_lowercase().contains(&query_lower) {
                            self.search_results.push(QuestSearchResult {
                                graph_id: graph_id.clone(),
                                node_id: *node_id,
                                field: "quest_name".to_string(),
                                excerpt: n.quest_name.clone(),
                            });
                        }
                        if n.description.to_lowercase().contains(&query_lower) {
                            self.search_results.push(QuestSearchResult {
                                graph_id: graph_id.clone(),
                                node_id: *node_id,
                                field: "description".to_string(),
                                excerpt: n.description.chars().take(60).collect(),
                            });
                        }
                    }
                    QuestNodeEnum::Objective(n) => {
                        let desc = n.objective.description();
                        if desc.to_lowercase().contains(&query_lower) {
                            self.search_results.push(QuestSearchResult {
                                graph_id: graph_id.clone(),
                                node_id: *node_id,
                                field: "objective".to_string(),
                                excerpt: desc,
                            });
                        }
                    }
                    QuestNodeEnum::Fail(n) => {
                        if n.fail_message.to_lowercase().contains(&query_lower) {
                            self.search_results.push(QuestSearchResult {
                                graph_id: graph_id.clone(),
                                node_id: *node_id,
                                field: "fail_message".to_string(),
                                excerpt: n.fail_message.clone(),
                            });
                        }
                    }
                    _ => {}
                }
                for tag in node.tags() {
                    if tag.to_lowercase().contains(&query_lower) {
                        self.search_results.push(QuestSearchResult {
                            graph_id: graph_id.clone(),
                            node_id: *node_id,
                            field: "tag".to_string(),
                            excerpt: tag.clone(),
                        });
                    }
                }
            }
        }
    }

    pub fn select_all(&mut self) {
        if let Some(graph) = self.active_graph() {
            self.graph_state.selected_nodes = graph.nodes.keys().cloned().collect();
        }
    }

    pub fn deselect_all(&mut self) {
        self.graph_state.selected_nodes.clear();
    }

    pub fn auto_layout_active(&mut self) {
        if let Some(graph) = self.active_graph_mut() {
            graph.auto_layout();
        }
    }

    pub fn add_chain(&mut self, chain: QuestChain) {
        self.chains.insert(chain.chain_id.clone(), chain);
        self.dirty = true;
    }

    pub fn get_statistics(&self) -> QuestEditorStats {
        let total_quests = self.graphs.len();
        let total_nodes: usize = self.graphs.values().map(|g| g.nodes.len()).sum();
        let total_connections: usize = self.graphs.values().map(|g| g.connections.len()).sum();
        let active_quests = self.quest_state_store.quest_instances.values()
            .filter(|q| q.state == QuestState::Active).count();
        let completed_quests = self.quest_state_store.quest_instances.values()
            .filter(|q| q.state == QuestState::Completed).count();
        let total_objectives: usize = self.graphs.values()
            .flat_map(|g| g.nodes.values())
            .filter(|n| matches!(n, QuestNodeEnum::Objective(_)))
            .count();
        let total_rewards: usize = self.graphs.values()
            .flat_map(|g| g.nodes.values())
            .filter(|n| matches!(n, QuestNodeEnum::Reward(_)))
            .count();
        QuestEditorStats {
            total_quests,
            total_nodes,
            total_connections,
            active_quests,
            completed_quests,
            total_objectives,
            total_rewards,
            total_chains: self.chains.len(),
            journal_entries: self.journal.entries.len(),
            map_markers: self.journal.map_markers.len(),
            validation_errors: self.validation_errors.iter().filter(|e| e.severity == QuestValidationSeverity::Error).count(),
            validation_warnings: self.validation_errors.iter().filter(|e| e.severity == QuestValidationSeverity::Warning).count(),
        }
    }

    pub fn export_to_json(&self, graph_id: &str) -> Result<String, String> {
        let graph = self.graphs.get(graph_id).ok_or("Graph not found")?;
        let mut json = String::new();
        json.push_str("{\n");
        json.push_str(&format!("  \"id\": \"{}\",\n", graph.id));
        json.push_str(&format!("  \"quest_id\": \"{}\",\n", graph.quest_id));
        json.push_str(&format!("  \"name\": \"{}\",\n", graph.name));
        json.push_str(&format!("  \"node_count\": {},\n", graph.nodes.len()));
        json.push_str(&format!("  \"connection_count\": {}\n", graph.connections.len()));
        json.push('}');
        Ok(json)
    }

    pub fn add_faction_rep_delta(&mut self, faction_id: &str, delta: i32, reason: &str, quest_id: Option<&str>) {
        self.faction_system.apply_rep_delta(faction_id, delta, reason, quest_id);
        self.quest_state_store.modify_faction_rep(faction_id, delta);
        self.journal.notification_queue.push_back(JournalNotification {
            notification_type: JournalNotificationType::ReputationChange {
                faction: faction_id.to_string(),
                delta,
            },
            title: format!("Reputation: {}", faction_id),
            description: format!("{:+} - {}", delta, reason),
            icon: "rep_icon".to_string(),
            duration: 4.0,
            timestamp: self.quest_state_store.current_time,
        });
    }

    pub fn generate_journal_entry_for_quest(&mut self, quest_id: &str, graph_id: &str) {
        if let Some(graph) = self.graphs.get(graph_id) {
            if let Some(start_node_id) = graph.start_node_id {
                if let Some(QuestNodeEnum::QuestStart(start)) = graph.nodes.get(&start_node_id) {
                    self.journal.add_entry(
                        quest_id,
                        JournalEntryType::QuestStart,
                        &start.quest_name,
                        &start.description,
                        JournalCategory::MainQuest,
                    );
                    for marker in &start.start_map_markers.clone() {
                        self.journal.add_map_marker(marker.clone());
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct QuestEditorStats {
    pub total_quests: usize,
    pub total_nodes: usize,
    pub total_connections: usize,
    pub active_quests: usize,
    pub completed_quests: usize,
    pub total_objectives: usize,
    pub total_rewards: usize,
    pub total_chains: usize,
    pub journal_entries: usize,
    pub map_markers: usize,
    pub validation_errors: usize,
    pub validation_warnings: usize,
}

// ============================================================
// EXTRA: SIMPLE RNG (No external deps)
// ============================================================

#[derive(Debug, Clone)]
pub struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    pub fn new(seed: u64) -> Self { SimpleRng { state: seed } }

    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 33) as f32 / (u32::MAX as f32)
    }

    pub fn next_u32(&mut self, max: u32) -> u32 {
        if max == 0 { return 0; }
        (self.next_u64() % max as u64) as u32
    }

    pub fn next_usize(&mut self, max: usize) -> usize {
        if max == 0 { return 0; }
        (self.next_u64() % max as u64) as usize
    }

    pub fn next_range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max { return min; }
        let range = (max - min) as u32;
        min + self.next_u32(range) as i32
    }

    pub fn next_range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    pub fn shuffle<T>(&mut self, slice: &mut Vec<T>) {
        let len = slice.len();
        for i in (1..len).rev() {
            let j = self.next_usize(i + 1);
            slice.swap(i, j);
        }
    }

    pub fn weighted_pick(&mut self, weights: &[f32]) -> Option<usize> {
        if weights.is_empty() { return None; }
        let total: f32 = weights.iter().sum();
        if total <= 0.0 { return None; }
        let mut cursor = self.next_f32() * total;
        for (i, &w) in weights.iter().enumerate() {
            cursor -= w;
            if cursor <= 0.0 { return Some(i); }
        }
        Some(weights.len() - 1)
    }
}

// ============================================================
// EXTRA: LOOT TABLE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct LootTable {
    pub table_id: String,
    pub name: String,
    pub entries: Vec<LootEntry>,
    pub guaranteed_entries: Vec<LootEntry>,
    pub max_rolls: u32,
    pub min_rolls: u32,
    pub quality_modifier: f32,
    pub luck_affects: bool,
}

#[derive(Debug, Clone)]
pub struct LootEntry {
    pub item_id: String,
    pub item_name: String,
    pub weight: f32,
    pub quantity_min: u32,
    pub quantity_max: u32,
    pub quality_min: f32,
    pub quality_max: f32,
    pub rarity: ItemRarity,
    pub level_requirement: Option<u32>,
    pub conditions: Vec<QuestCondition>,
    pub nested_table: Option<String>,
}

impl LootTable {
    pub fn new(table_id: &str, name: &str) -> Self {
        LootTable {
            table_id: table_id.to_string(),
            name: name.to_string(),
            entries: Vec::new(),
            guaranteed_entries: Vec::new(),
            max_rolls: 1,
            min_rolls: 1,
            quality_modifier: 1.0,
            luck_affects: true,
        }
    }

    pub fn add_entry(&mut self, item_id: &str, weight: f32, qty_min: u32, qty_max: u32, rarity: ItemRarity) {
        self.entries.push(LootEntry {
            item_id: item_id.to_string(),
            item_name: item_id.to_string(),
            weight,
            quantity_min: qty_min,
            quantity_max: qty_max,
            quality_min: 0.5,
            quality_max: 1.0,
            rarity,
            level_requirement: None,
            conditions: Vec::new(),
            nested_table: None,
        });
    }

    pub fn roll(&self, rng: &mut SimpleRng, player_level: u32, luck: f32) -> Vec<LootResult> {
        let mut results = Vec::new();
        // Guaranteed drops
        for entry in &self.guaranteed_entries {
            if let Some(req) = entry.level_requirement {
                if player_level < req { continue; }
            }
            let qty = if entry.quantity_min >= entry.quantity_max {
                entry.quantity_min
            } else {
                entry.quantity_min + rng.next_u32(entry.quantity_max - entry.quantity_min + 1)
            };
            let quality = entry.quality_min + rng.next_f32() * (entry.quality_max - entry.quality_min);
            results.push(LootResult {
                item_id: entry.item_id.clone(),
                quantity: qty,
                quality: (quality * self.quality_modifier).min(1.0),
                rarity: entry.rarity.clone(),
                from_table: self.table_id.clone(),
            });
        }
        // Random rolls
        let num_rolls = self.min_rolls + rng.next_u32(self.max_rolls - self.min_rolls + 1);
        let weights: Vec<f32> = self.entries.iter()
            .map(|e| {
                let luck_bonus = if self.luck_affects { luck * e.rarity.drop_chance_modifier() } else { 0.0 };
                (e.weight * (1.0 + luck_bonus)).max(0.0)
            })
            .collect();
        for _ in 0..num_rolls {
            if let Some(idx) = rng.weighted_pick(&weights) {
                let entry = &self.entries[idx];
                if let Some(req) = entry.level_requirement {
                    if player_level < req { continue; }
                }
                let qty = if entry.quantity_min >= entry.quantity_max {
                    entry.quantity_min
                } else {
                    entry.quantity_min + rng.next_u32(entry.quantity_max - entry.quantity_min + 1)
                };
                let quality = entry.quality_min + rng.next_f32() * (entry.quality_max - entry.quality_min);
                results.push(LootResult {
                    item_id: entry.item_id.clone(),
                    quantity: qty,
                    quality: (quality * self.quality_modifier).min(1.0),
                    rarity: entry.rarity.clone(),
                    from_table: self.table_id.clone(),
                });
            }
        }
        results
    }

    pub fn get_total_weight(&self) -> f32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn get_probability(&self, item_id: &str) -> f32 {
        let total = self.get_total_weight();
        if total == 0.0 { return 0.0; }
        let item_weight: f32 = self.entries.iter()
            .filter(|e| e.item_id == item_id)
            .map(|e| e.weight)
            .sum();
        item_weight / total
    }
}

#[derive(Debug, Clone)]
pub struct LootResult {
    pub item_id: String,
    pub quantity: u32,
    pub quality: f32,
    pub rarity: ItemRarity,
    pub from_table: String,
}

impl LootResult {
    pub fn display(&self) -> String {
        format!("{} x{} ({:.0}% quality, {:?})", self.item_id, self.quantity, self.quality * 100.0, self.rarity)
    }
}

// ============================================================
// EXTRA: XP CURVE CALCULATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct XpCurve {
    pub curve_type: XpCurveType,
    pub base_xp: u32,
    pub max_level: u32,
    pub custom_values: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XpCurveType {
    Linear { increment: u32 },
    Exponential { base: f32 },
    Polynomial { exponent: f32 },
    Custom,
    Flat,
}

impl XpCurve {
    pub fn new_exponential(base_xp: u32, max_level: u32, base: f32) -> Self {
        XpCurve {
            curve_type: XpCurveType::Exponential { base },
            base_xp,
            max_level,
            custom_values: Vec::new(),
        }
    }

    pub fn xp_for_level(&self, level: u32) -> u32 {
        if level == 0 { return 0; }
        let level = level.min(self.max_level);
        match &self.curve_type {
            XpCurveType::Linear { increment } => {
                self.base_xp + increment * (level - 1)
            }
            XpCurveType::Exponential { base } => {
                (self.base_xp as f32 * base.powi(level as i32 - 1)) as u32
            }
            XpCurveType::Polynomial { exponent } => {
                (self.base_xp as f32 * (level as f32).powf(*exponent)) as u32
            }
            XpCurveType::Flat => self.base_xp,
            XpCurveType::Custom => {
                self.custom_values.get((level - 1) as usize).copied().unwrap_or(self.base_xp)
            }
        }
    }

    pub fn total_xp_for_level(&self, target_level: u32) -> u32 {
        (1..target_level).map(|l| self.xp_for_level(l)).sum()
    }

    pub fn level_from_total_xp(&self, total_xp: u32) -> u32 {
        let mut accumulated = 0u32;
        for level in 1..=self.max_level {
            let needed = self.xp_for_level(level);
            if accumulated + needed > total_xp { return level.saturating_sub(1); }
            accumulated += needed;
        }
        self.max_level
    }

    pub fn progress_in_level(&self, total_xp: u32) -> f32 {
        let level = self.level_from_total_xp(total_xp);
        let xp_for_this = self.total_xp_for_level(level);
        let xp_needed = self.xp_for_level(level + 1);
        if xp_needed == 0 { return 1.0; }
        let progress_xp = total_xp - xp_for_this;
        (progress_xp as f32 / xp_needed as f32).clamp(0.0, 1.0)
    }

    pub fn generate_table(&self) -> Vec<(u32, u32, u32)> {
        let mut table = Vec::new();
        let mut total = 0u32;
        for level in 1..=self.max_level {
            let xp = self.xp_for_level(level);
            total += xp;
            table.push((level, xp, total));
        }
        table
    }
}

// ============================================================
// EXTRA: QUEST NODE STYLE
// ============================================================

#[derive(Debug, Clone)]
pub struct QuestNodeStyle {
    pub header_color: Vec4,
    pub body_color: Vec4,
    pub border_color: Vec4,
    pub selected_border: Vec4,
    pub text_color: Vec4,
    pub port_color_in: Vec4,
    pub port_color_out: Vec4,
    pub rounding: f32,
    pub border_width: f32,
}

impl QuestNodeStyle {
    pub fn for_type(node_type: &QuestNodeType) -> Self {
        let (header, body) = match node_type {
            QuestNodeType::QuestStart => (Vec4::new(0.15, 0.6, 0.15, 1.0), Vec4::new(0.1, 0.35, 0.1, 1.0)),
            QuestNodeType::Completion => (Vec4::new(0.1, 0.5, 0.8, 1.0), Vec4::new(0.05, 0.3, 0.5, 1.0)),
            QuestNodeType::Fail => (Vec4::new(0.7, 0.1, 0.1, 1.0), Vec4::new(0.4, 0.05, 0.05, 1.0)),
            QuestNodeType::Objective => (Vec4::new(0.2, 0.4, 0.7, 1.0), Vec4::new(0.1, 0.25, 0.45, 1.0)),
            QuestNodeType::ConditionCheck => (Vec4::new(0.6, 0.5, 0.0, 1.0), Vec4::new(0.35, 0.3, 0.0, 1.0)),
            QuestNodeType::Reward => (Vec4::new(0.6, 0.45, 0.0, 1.0), Vec4::new(0.35, 0.25, 0.0, 1.0)),
            QuestNodeType::Branch => (Vec4::new(0.5, 0.2, 0.5, 1.0), Vec4::new(0.3, 0.1, 0.3, 1.0)),
            QuestNodeType::Timer => (Vec4::new(0.7, 0.3, 0.0, 1.0), Vec4::new(0.45, 0.15, 0.0, 1.0)),
            QuestNodeType::Trigger => (Vec4::new(0.4, 0.6, 0.5, 1.0), Vec4::new(0.2, 0.4, 0.3, 1.0)),
        };
        QuestNodeStyle {
            header_color: header,
            body_color: body,
            border_color: Vec4::new(0.35, 0.35, 0.35, 1.0),
            selected_border: Vec4::new(1.0, 0.8, 0.0, 1.0),
            text_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            port_color_in: Vec4::new(0.6, 1.0, 0.6, 1.0),
            port_color_out: Vec4::new(1.0, 0.6, 0.6, 1.0),
            rounding: 6.0,
            border_width: 1.5,
        }
    }
}

// ============================================================
// EXTRA: GRAPH STATISTICS COLLECTOR
// ============================================================

#[derive(Debug, Clone)]
pub struct QuestGraphAnalyzer {
    pub graph_id: String,
    pub node_type_counts: HashMap<String, usize>,
    pub objective_types: Vec<String>,
    pub longest_path_length: usize,
    pub dead_ends: Vec<u64>,
    pub orphan_nodes: Vec<u64>,
    pub total_estimated_xp: u32,
    pub branch_factor_avg: f32,
    pub cycle_count: usize,
}

impl QuestGraphAnalyzer {
    pub fn analyze(graph: &QuestGraph, player_level: u32, rng: &mut SimpleRng) -> Self {
        let mut node_type_counts: HashMap<String, usize> = HashMap::new();
        let mut objective_types = Vec::new();
        let mut dead_ends = Vec::new();
        let mut orphan_nodes = Vec::new();
        let mut total_xp = 0u32;

        let reachable = if let Some(start) = graph.start_node_id {
            graph.find_reachable(start)
        } else { HashSet::new() };

        for (id, node) in &graph.nodes {
            let type_name = format!("{:?}", node.node_type());
            *node_type_counts.entry(type_name).or_insert(0) += 1;
            if !reachable.contains(id) {
                orphan_nodes.push(*id);
                continue;
            }
            match node {
                QuestNodeEnum::Objective(obj) => {
                    objective_types.push(obj.objective.description());
                    if graph.get_outputs(*id).is_empty() { dead_ends.push(*id); }
                }
                QuestNodeEnum::Reward(r) => {
                    total_xp += r.reward_table.total_estimated_xp(player_level);
                }
                _ => {}
            }
        }

        let branch_counts: Vec<usize> = graph.nodes.keys()
            .map(|id| graph.get_outputs(*id).len())
            .collect();
        let branch_factor_avg = if branch_counts.is_empty() { 0.0 } else {
            branch_counts.iter().sum::<usize>() as f32 / branch_counts.len() as f32
        };

        let longest_path = if let Some(start) = graph.start_node_id {
            Self::find_longest_path(graph, start)
        } else { 0 };

        QuestGraphAnalyzer {
            graph_id: graph.id.clone(),
            node_type_counts,
            objective_types,
            longest_path_length: longest_path,
            dead_ends,
            orphan_nodes,
            total_estimated_xp: total_xp,
            branch_factor_avg,
            cycle_count: 0,
        }
    }

    fn find_longest_path(graph: &QuestGraph, start: u64) -> usize {
        let mut best = 0usize;
        let mut stack = vec![(start, 0usize, HashSet::new())];
        while let Some((node, depth, mut visited)) = stack.pop() {
            if visited.contains(&node) { continue; }
            visited.insert(node);
            best = best.max(depth);
            for conn in graph.get_outputs(node) {
                stack.push((conn.to_node, depth + 1, visited.clone()));
            }
        }
        best
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== QUEST GRAPH ANALYSIS ===\n\n");
        report.push_str("Node Types:\n");
        for (t, c) in &self.node_type_counts {
            report.push_str(&format!("  {}: {}\n", t, c));
        }
        report.push_str(&format!("\nLongest path: {} nodes\n", self.longest_path_length));
        report.push_str(&format!("Dead ends: {}\n", self.dead_ends.len()));
        report.push_str(&format!("Orphan nodes: {}\n", self.orphan_nodes.len()));
        report.push_str(&format!("Estimated XP reward: {}\n", self.total_estimated_xp));
        report.push_str(&format!("Average branch factor: {:.2}\n", self.branch_factor_avg));
        report.push_str(&format!("\nObjectives ({}):\n", self.objective_types.len()));
        for obj in &self.objective_types {
            report.push_str(&format!("  - {}\n", obj));
        }
        report
    }
}

// ============================================================
// EXTRA: SAVE/LOAD
// ============================================================

#[derive(Debug, Clone)]
pub struct QuestProjectFile {
    pub version: String,
    pub graphs: Vec<SerializedQuestGraph>,
    pub chains: Vec<QuestChain>,
    pub faction_state: Vec<(String, i32)>,
    pub state_flags: Vec<String>,
    pub metadata: QuestProjectMetadata,
}

#[derive(Debug, Clone)]
pub struct SerializedQuestGraph {
    pub id: String,
    pub quest_id: String,
    pub name: String,
    pub node_count: usize,
    pub data: String,
}

#[derive(Debug, Clone)]
pub struct QuestProjectMetadata {
    pub project_name: String,
    pub game_title: String,
    pub author: String,
    pub created: String,
    pub modified: String,
    pub version: String,
    pub editor_version: String,
}

impl QuestProjectFile {
    pub fn new(project_name: &str) -> Self {
        QuestProjectFile {
            version: "1.0.0".to_string(),
            graphs: Vec::new(),
            chains: Vec::new(),
            faction_state: Vec::new(),
            state_flags: Vec::new(),
            metadata: QuestProjectMetadata {
                project_name: project_name.to_string(),
                game_title: String::new(),
                author: String::new(),
                created: String::new(),
                modified: String::new(),
                version: "1.0.0".to_string(),
                editor_version: "1.0.0".to_string(),
            },
        }
    }

    pub fn from_editor(editor: &QuestEditor) -> Self {
        let mut file = QuestProjectFile::new("project");
        for (id, graph) in &editor.graphs {
            file.graphs.push(SerializedQuestGraph {
                id: graph.id.clone(),
                quest_id: graph.quest_id.clone(),
                name: graph.name.clone(),
                node_count: graph.nodes.len(),
                data: String::new(),
            });
        }
        for (id, chain) in &editor.chains {
            file.chains.push(chain.clone());
        }
        for (faction_id, rep) in &editor.quest_state_store.faction_reps {
            file.faction_state.push((faction_id.clone(), *rep));
        }
        for flag in &editor.quest_state_store.flags {
            file.state_flags.push(flag.clone());
        }
        file
    }
}

// ============================================================
// EXTRA: REPEATABLE QUEST TRACKER
// ============================================================

#[derive(Debug, Clone)]
pub struct RepeatableQuestTracker {
    pub quest_id: String,
    pub completion_count: u32,
    pub last_completion_time: f64,
    pub cooldown_secs: f64,
    pub max_completions: Option<u32>,
    pub rewards_scale_with_completions: bool,
    pub reward_scale_factor: f32,
    pub reward_cap_multiplier: f32,
}

impl RepeatableQuestTracker {
    pub fn new(quest_id: &str, cooldown: f64) -> Self {
        RepeatableQuestTracker {
            quest_id: quest_id.to_string(),
            completion_count: 0,
            last_completion_time: -f64::MAX,
            cooldown_secs: cooldown,
            max_completions: None,
            rewards_scale_with_completions: false,
            reward_scale_factor: 1.0,
            reward_cap_multiplier: 2.0,
        }
    }

    pub fn can_start(&self, current_time: f64) -> bool {
        if let Some(max) = self.max_completions {
            if self.completion_count >= max { return false; }
        }
        current_time - self.last_completion_time >= self.cooldown_secs
    }

    pub fn complete(&mut self, current_time: f64) {
        self.completion_count += 1;
        self.last_completion_time = current_time;
    }

    pub fn get_reward_multiplier(&self) -> f32 {
        if !self.rewards_scale_with_completions { return 1.0; }
        let base = 1.0 + (self.completion_count as f32 * self.reward_scale_factor);
        base.min(self.reward_cap_multiplier)
    }

    pub fn cooldown_remaining(&self, current_time: f64) -> f64 {
        (self.cooldown_secs - (current_time - self.last_completion_time)).max(0.0)
    }

    pub fn format_cooldown(&self, current_time: f64) -> String {
        let rem = self.cooldown_remaining(current_time);
        let hours = (rem / 3600.0) as u64;
        let mins = ((rem % 3600.0) / 60.0) as u64;
        let secs = (rem % 60.0) as u64;
        if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else if mins > 0 {
            format!("{}m {}s", mins, secs)
        } else {
            format!("{}s", secs)
        }
    }
}

// ============================================================
// EXTRA: OBJECTIVE PROGRESS TRACKER (for UI display)
// ============================================================

#[derive(Debug, Clone)]
pub struct ObjectiveProgressDisplay {
    pub objective_id: String,
    pub description: String,
    pub progress_text: String,
    pub progress_value: f32,
    pub status: ObjectiveStatus,
    pub optional: bool,
    pub secret: bool,
    pub time_remaining: Option<f32>,
    pub map_markers: Vec<MapMarker>,
    pub new_this_session: bool,
}

impl ObjectiveProgressDisplay {
    pub fn from_node(node: &ObjectiveNode) -> Self {
        let status = node.objective.get_status();
        ObjectiveProgressDisplay {
            objective_id: node.objective_id.clone(),
            description: node.objective.description(),
            progress_text: node.progress_display(),
            progress_value: node.objective.get_progress(),
            status,
            optional: node.optional,
            secret: node.secret,
            time_remaining: node.time_remaining(),
            map_markers: node.map_markers.clone(),
            new_this_session: false,
        }
    }

    pub fn status_color(&self) -> Vec4 {
        match self.status {
            ObjectiveStatus::NotStarted => Vec4::new(0.5, 0.5, 0.5, 1.0),
            ObjectiveStatus::Active => Vec4::new(0.9, 0.9, 0.9, 1.0),
            ObjectiveStatus::Completed => Vec4::new(0.2, 0.9, 0.2, 1.0),
            ObjectiveStatus::Failed => Vec4::new(0.9, 0.2, 0.2, 1.0),
        }
    }

    pub fn progress_bar_color(&self) -> Vec4 {
        match self.status {
            ObjectiveStatus::Active => Vec4::new(0.2, 0.5, 0.9, 1.0),
            ObjectiveStatus::Completed => Vec4::new(0.2, 0.8, 0.2, 1.0),
            ObjectiveStatus::Failed => Vec4::new(0.8, 0.2, 0.2, 1.0),
            _ => Vec4::new(0.4, 0.4, 0.4, 1.0),
        }
    }
}

// ============================================================
// EXTRA: QUEST HUD MANAGER
// ============================================================

#[derive(Debug, Clone)]
pub struct QuestHudManager {
    pub tracked_quests: Vec<TrackedQuestDisplay>,
    pub max_tracked: usize,
    pub show_completed_briefly: bool,
    pub completed_display_time: f32,
    pub notifications: VecDeque<QuestHudNotification>,
    pub objective_updates: VecDeque<ObjectiveUpdateAnim>,
    pub world_markers: Vec<WorldMapMarker>,
}

#[derive(Debug, Clone)]
pub struct TrackedQuestDisplay {
    pub quest_id: String,
    pub quest_name: String,
    pub objectives: Vec<ObjectiveProgressDisplay>,
    pub is_main_quest: bool,
    pub pin_position: Option<u32>,
    pub recently_updated: bool,
    pub update_flash_time: f32,
}

#[derive(Debug, Clone)]
pub struct QuestHudNotification {
    pub notification_id: String,
    pub text: String,
    pub sub_text: String,
    pub icon: String,
    pub sound: Option<String>,
    pub duration: f32,
    pub elapsed: f32,
    pub style: HudNotificationStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HudNotificationStyle {
    QuestStarted,
    QuestComplete,
    QuestFailed,
    ObjectiveComplete,
    ObjectiveFailed,
    RewardReceived,
    RepChanged,
}

#[derive(Debug, Clone)]
pub struct ObjectiveUpdateAnim {
    pub objective_id: String,
    pub update_type: ObjectiveUpdateType,
    pub elapsed: f32,
    pub duration: f32,
}

#[derive(Debug, Clone)]
pub enum ObjectiveUpdateType {
    Progress(f32),
    Completed,
    Failed,
    Added,
    Removed,
}

#[derive(Debug, Clone)]
pub struct WorldMapMarker {
    pub marker_id: String,
    pub world_pos: Vec3,
    pub label: String,
    pub icon_id: String,
    pub color: Vec4,
    pub is_visible: bool,
    pub distance_to_player: f32,
    pub compass_angle: f32,
    pub show_on_compass: bool,
    pub show_in_world: bool,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl WorldMapMarker {
    pub fn update_from_player(&mut self, player_pos: Vec3, player_forward: Vec3) {
        let to_marker = self.world_pos - player_pos;
        self.distance_to_player = to_marker.length();
        let angle = to_marker.x.atan2(to_marker.z);
        let player_angle = player_forward.x.atan2(player_forward.z);
        self.compass_angle = angle - player_angle;
        self.is_visible = self.distance_to_player >= self.min_distance &&
            (self.max_distance == 0.0 || self.distance_to_player <= self.max_distance);
    }

    pub fn get_screen_edge_pos(&self, screen_size: Vec2) -> Vec2 {
        let half = screen_size * 0.5;
        let angle = self.compass_angle;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let scale_x = half.x / sin_a.abs().max(0.001);
        let scale_y = half.y / cos_a.abs().max(0.001);
        let scale = scale_x.min(scale_y);
        Vec2::new(half.x + sin_a * scale, half.y - cos_a * scale)
    }
}

impl QuestHudManager {
    pub fn new() -> Self {
        QuestHudManager {
            tracked_quests: Vec::new(),
            max_tracked: 5,
            show_completed_briefly: true,
            completed_display_time: 5.0,
            notifications: VecDeque::new(),
            objective_updates: VecDeque::new(),
            world_markers: Vec::new(),
        }
    }

    pub fn update(&mut self, delta: f32) {
        // Update notifications
        self.notifications.retain_mut(|n| {
            n.elapsed += delta;
            n.elapsed < n.duration
        });
        // Update objective anims
        self.objective_updates.retain_mut(|a| {
            a.elapsed += delta;
            a.elapsed < a.duration
        });
        // Update flash timers
        for quest in &mut self.tracked_quests {
            if quest.recently_updated {
                quest.update_flash_time -= delta;
                if quest.update_flash_time <= 0.0 {
                    quest.recently_updated = false;
                }
            }
        }
    }

    pub fn track_quest(&mut self, quest_id: &str, quest_name: &str, is_main: bool) {
        if self.tracked_quests.iter().any(|q| q.quest_id == quest_id) { return; }
        if self.tracked_quests.len() >= self.max_tracked { return; }
        self.tracked_quests.push(TrackedQuestDisplay {
            quest_id: quest_id.to_string(),
            quest_name: quest_name.to_string(),
            objectives: Vec::new(),
            is_main_quest: is_main,
            pin_position: None,
            recently_updated: true,
            update_flash_time: 3.0,
        });
    }

    pub fn untrack_quest(&mut self, quest_id: &str) {
        self.tracked_quests.retain(|q| q.quest_id != quest_id);
    }

    pub fn push_notification(&mut self, text: &str, sub_text: &str, style: HudNotificationStyle, icon: &str) {
        let duration = match style {
            HudNotificationStyle::QuestComplete | HudNotificationStyle::QuestFailed => 6.0,
            HudNotificationStyle::ObjectiveComplete => 4.0,
            _ => 3.0,
        };
        self.notifications.push_back(QuestHudNotification {
            notification_id: format!("notif_{}", self.notifications.len()),
            text: text.to_string(),
            sub_text: sub_text.to_string(),
            icon: icon.to_string(),
            sound: None,
            duration,
            elapsed: 0.0,
            style,
        });
    }

    pub fn get_active_notifications(&self) -> Vec<&QuestHudNotification> {
        self.notifications.iter().collect()
    }
}

// ============================================================
// MODULE-LEVEL UTILITY FUNCTIONS
// ============================================================

pub fn create_sample_quest_graph() -> QuestGraph {
    let mut graph = QuestGraph::new("quest_graph_001".to_string(), "the_lost_artifact".to_string(), "The Lost Artifact".to_string());

    let start_id = 1u64;
    let mut start = QuestStartNode::new(start_id);
    start.quest_name = "The Lost Artifact".to_string();
    start.description = "An ancient artifact has been stolen. Track it down and recover it.".to_string();
    start.quest_giver_id = "npc_scholar_aldred".to_string();
    start.position = Vec2::new(50.0, 200.0);
    graph.add_node(QuestNodeEnum::QuestStart(start));

    let obj1_id = 2u64;
    let obj1 = QuestObjective::TalkToNPC(TalkToNPCObjective {
        npc_id: "npc_witness_sara".to_string(),
        npc_name: "Sara the Witness".to_string(),
        required_topic: Some("artifact_theft".to_string()),
        talked: false,
        dialogue_completed: false,
        require_specific_ending: None,
    });
    let mut obj_node1 = ObjectiveNode::new(obj1_id, obj1);
    obj_node1.position = Vec2::new(300.0, 100.0);
    obj_node1.journal_entry_on_start = Some("Find someone who witnessed the theft.".to_string());
    graph.add_node(QuestNodeEnum::Objective(obj_node1));

    let obj2_id = 3u64;
    let obj2 = QuestObjective::InvestigateClue(InvestigateClueObjective {
        location_name: "The Old Warehouse".to_string(),
        clue_ids: vec!["clue_footprints".to_string(), "clue_torn_cloth".to_string(), "clue_stolen_tools".to_string()],
        clues_found: HashSet::new(),
        required_clues: 2,
        all_clues_required: false,
    });
    let mut obj_node2 = ObjectiveNode::new(obj2_id, obj2);
    obj_node2.position = Vec2::new(600.0, 100.0);
    obj_node2.journal_entry_on_start = Some("Investigate the old warehouse for clues.".to_string());
    graph.add_node(QuestNodeEnum::Objective(obj_node2));

    let obj3_id = 4u64;
    let obj3 = QuestObjective::KillEnemy(KillEnemyObjective {
        enemy_type: "thief".to_string(),
        enemy_id: Some("boss_thief_marcus".to_string()),
        required_count: 1,
        killed_count: 0,
        in_area: Some("thief_hideout".to_string()),
        with_weapon_type: None,
        require_stealth_kill: false,
        allow_assists: false,
    });
    let mut obj_node3 = ObjectiveNode::new(obj3_id, obj3);
    obj_node3.position = Vec2::new(900.0, 100.0);
    obj_node3.journal_entry_on_start = Some("Defeat Marcus and recover the artifact.".to_string());
    graph.add_node(QuestNodeEnum::Objective(obj_node3));

    let reward_id = 5u64;
    let mut reward_node = RewardNode::new(reward_id);
    reward_node.position = Vec2::new(1200.0, 100.0);
    reward_node.reward_table.add_xp(500, 0.1);
    reward_node.reward_table.add_item("ancient_artifact", 1, 1, 1.0, ItemRarity::Rare);
    reward_node.reward_table.add_currency("gold", 100, 200, 0.05);
    graph.add_node(QuestNodeEnum::Reward(reward_node));

    let complete_id = 6u64;
    let mut complete_node = CompletionNode::new(complete_id);
    complete_node.position = Vec2::new(1500.0, 100.0);
    complete_node.completion_message = "The artifact has been recovered!".to_string();
    complete_node.unlock_quests = vec!["quest_ancient_mystery_2".to_string()];
    complete_node.set_flags = vec!["artifact_recovered".to_string()];
    graph.add_node(QuestNodeEnum::Completion(complete_node));

    let fail_id = 7u64;
    let mut fail_node = FailNode::new(fail_id);
    fail_node.position = Vec2::new(900.0, 350.0);
    fail_node.fail_reason = "artifact_destroyed".to_string();
    fail_node.fail_message = "The artifact was destroyed. The quest has failed.".to_string();
    fail_node.allow_retry = false;
    graph.add_node(QuestNodeEnum::Fail(fail_node));

    graph.connect(1, 0, 2, 0, None);
    graph.connect(2, 0, 3, 0, Some("witness_interviewed".to_string()));
    graph.connect(3, 0, 4, 0, Some("clues_found".to_string()));
    graph.connect(4, 0, 5, 0, Some("boss_defeated".to_string()));
    graph.connect(4, 1, 7, 0, Some("artifact_destroyed".to_string()));
    graph.connect(5, 0, 6, 0, None);

    graph
}

pub fn calculate_quest_reward_at_level(reward: &RewardTable, level: u32, rng: &mut SimpleRng) -> Vec<RolledReward> {
    reward.roll(level, rng)
}

pub fn get_faction_tier_display(faction: &Faction) -> String {
    let tier = &faction.current_tier;
    let next_threshold = tier.next_tier_threshold();
    if let Some(next) = next_threshold {
        let current_threshold = match tier {
            ReputationTier::Neutral => -1000,
            ReputationTier::Friendly => 1000,
            ReputationTier::Honored => 2000,
            ReputationTier::Revered => 3000,
            _ => faction.reputation_min,
        };
        let range = next - current_threshold;
        let progress = faction.reputation - current_threshold;
        format!("{}: {}/{}", tier.display_name(), progress, range)
    } else {
        format!("{} (Max)", tier.display_name())
    }
}

pub fn format_xp(xp: u32) -> String {
    if xp >= 1_000_000 {
        format!("{:.1}M XP", xp as f32 / 1_000_000.0)
    } else if xp >= 1_000 {
        format!("{:.1}K XP", xp as f32 / 1_000.0)
    } else {
        format!("{} XP", xp)
    }
}

pub fn format_currency(amount: u32, currency_id: &str) -> String {
    let symbol = match currency_id {
        "gold" => "G",
        "silver" => "S",
        "copper" => "C",
        "credits" => "CR",
        _ => currency_id,
    };
    if amount >= 1_000_000 {
        format!("{:.1}M {}", amount as f32 / 1_000_000.0, symbol)
    } else if amount >= 1_000 {
        format!("{:.1}K {}", amount as f32 / 1_000.0, symbol)
    } else {
        format!("{} {}", amount, symbol)
    }
}

pub fn interpolate_rep_color(rep: i32) -> Vec4 {
    let tier = ReputationTier::from_rep(rep);
    tier.color()
}

pub fn check_all_prerequisites(quest: &QuestStartNode, state: &QuestStateStore) -> (bool, Vec<String>) {
    let mut unmet = Vec::new();
    let mut all_met = true;
    for prereq in &quest.prerequisites {
        if !prereq.is_satisfied(state) {
            all_met = false;
            let reason = match &prereq.prereq_type {
                PrerequisiteType::QuestCompleted => format!("Quest required: {:?}", prereq.quest_id),
                PrerequisiteType::PlayerLevel => format!("Level required: {:?}", prereq.level_required),
                PrerequisiteType::FactionRep => format!("Reputation required: {:?} {:?}", prereq.faction_id, prereq.faction_rep_required),
                PrerequisiteType::Flag => format!("Flag required: {:?}", prereq.flag_name),
                _ => "Unknown requirement".to_string(),
            };
            unmet.push(reason);
        }
    }
    (all_met, unmet)
}
