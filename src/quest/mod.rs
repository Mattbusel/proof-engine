//! # Quest System
//!
//! Provides quest definitions, player progress tracking, objective management,
//! reward distribution, and automatic event-driven objective advancement for
//! the Proof Engine game loop.
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |---|---|
//! | `journal` | `QuestJournal`, `QuestProgress`, `ObjectiveProgress`, tick-based timers |
//! | `tracker` | `QuestTracker`, `ObjectiveMapper`, game-event → objective wiring |
//!
//! ## Quick-start example
//!
//! ```rust,no_run
//! use proof_engine::quest::{
//!     QuestId, QuestState, QuestDef, QuestCategory, QuestPriority,
//!     ObjectiveDef, ObjectiveType, Reward, QuestDatabase,
//!     journal::QuestJournal,
//!     tracker::QuestTracker,
//! };
//! use std::sync::Arc;
//!
//! let mut db = QuestDatabase::new();
//! // register quest definitions …
//! let db = Arc::new(db);
//! let journal = QuestJournal::new();
//! let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
//! let events = tracker.on_kill("goblin".to_string(), 3);
//! ```

pub mod journal;
pub mod tracker;

// ── Re-exports ────────────────────────────────────────────────────────────────

pub use journal::{
    JournalError, JournalNote, JournalSummary, ObjectiveAdvanceResult, ObjectiveProgress,
    QuestEvent, QuestJournal, QuestProgress,
};

// `PrerequisiteView` is defined below in this file.  It is intentionally
// separate from `journal::QuestProgress` to avoid a naming collision.
pub use tracker::{
    GameEventType, ObjectiveMapper, QuestTracker, RewardDistributor, TrackerSession, TrackerStats,
};

use std::collections::HashMap;

// ── Newtype identifiers ───────────────────────────────────────────────────────

/// Strongly-typed quest identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuestId(pub u32);

impl QuestId {
    pub fn new(id: u32) -> Self { Self(id) }
    pub fn raw(self) -> u32 { self.0 }
}

impl std::fmt::Display for QuestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "QuestId({})", self.0)
    }
}

/// Strongly-typed objective identifier (unique within a quest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectiveId(pub u32);

impl ObjectiveId {
    pub fn new(id: u32) -> Self { Self(id) }
    pub fn raw(self) -> u32 { self.0 }
}

impl std::fmt::Display for ObjectiveId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ObjectiveId({})", self.0)
    }
}

/// Strongly-typed reward identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RewardId(pub u32);

impl RewardId {
    pub fn new(id: u32) -> Self { Self(id) }
    pub fn raw(self) -> u32 { self.0 }
}

// ── QuestState ────────────────────────────────────────────────────────────────

/// Lifecycle state of a quest in the player's journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestState {
    /// Not yet encountered — prerequisites not met or not yet discovered.
    Inactive,
    /// Prerequisites satisfied; player may accept the quest.
    Available,
    /// Accepted and currently in-progress.
    Active,
    /// All required objectives finished; reward granted.
    Completed,
    /// At least one required objective failed or time expired.
    Failed,
    /// Player voluntarily dropped the quest.
    Abandoned,
}

impl QuestState {
    pub fn is_terminal(self) -> bool {
        matches!(self, QuestState::Completed | QuestState::Failed | QuestState::Abandoned)
    }

    pub fn label(self) -> &'static str {
        match self {
            QuestState::Inactive   => "Inactive",
            QuestState::Available  => "Available",
            QuestState::Active     => "Active",
            QuestState::Completed  => "Completed",
            QuestState::Failed     => "Failed",
            QuestState::Abandoned  => "Abandoned",
        }
    }
}

// ── ObjectiveState ────────────────────────────────────────────────────────────

/// Lifecycle state of a single objective within a quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectiveState {
    /// Objective not yet active (may be gated behind prior objectives).
    Inactive,
    /// Objective is being tracked.
    Active,
    /// Objective successfully finished.
    Completed,
    /// Objective failed (only meaningful for non-optional objectives).
    Failed,
    /// Bonus objective — failure does not fail the quest.
    Optional,
}

impl ObjectiveState {
    pub fn is_done(self) -> bool {
        matches!(self, ObjectiveState::Completed | ObjectiveState::Failed)
    }
}

// ── QuestCategory ─────────────────────────────────────────────────────────────

/// High-level categorisation used for journal filtering and UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestCategory {
    /// Critical story quests that drive the main narrative.
    Main,
    /// Optional story or world-building quests.
    Side,
    /// Resets every real-world day.
    Daily,
    /// Resets every real-world week.
    Weekly,
    /// Hunt-style quests with a specific target.
    Bounty,
    /// Discover locations or map areas.
    Exploration,
    /// Create items at a crafting station.
    Crafting,
    /// Defeat enemies or complete combat challenges.
    Combat,
    /// Interact with NPCs; reputation-focused.
    Social,
}

impl QuestCategory {
    pub fn label(self) -> &'static str {
        match self {
            QuestCategory::Main        => "Main",
            QuestCategory::Side        => "Side",
            QuestCategory::Daily       => "Daily",
            QuestCategory::Weekly      => "Weekly",
            QuestCategory::Bounty      => "Bounty",
            QuestCategory::Exploration => "Exploration",
            QuestCategory::Crafting    => "Crafting",
            QuestCategory::Combat      => "Combat",
            QuestCategory::Social      => "Social",
        }
    }

    pub fn is_time_limited(self) -> bool {
        matches!(self, QuestCategory::Daily | QuestCategory::Weekly)
    }
}

// ── QuestPriority ─────────────────────────────────────────────────────────────

/// Sort order hint for quest display and AI attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuestPriority {
    /// Time-sensitive — should be completed immediately.
    Critical = 3,
    /// Important but not urgent.
    High = 2,
    /// Default priority.
    Normal = 1,
    /// Can be done whenever.
    Low = 0,
}

impl QuestPriority {
    pub fn label(self) -> &'static str {
        match self {
            QuestPriority::Critical => "Critical",
            QuestPriority::High     => "High",
            QuestPriority::Normal   => "Normal",
            QuestPriority::Low      => "Low",
        }
    }
}

// ── Reward ────────────────────────────────────────────────────────────────────

/// All rewards granted upon quest completion.
#[derive(Debug, Clone, Default)]
pub struct Reward {
    /// Experience points awarded.
    pub experience: u32,
    /// Gold / currency awarded.
    pub gold: u32,
    /// `(item_id, quantity)` pairs.
    pub items: Vec<(u32, u32)>,
    /// Reputation deltas per faction: `(faction_name, delta)`.
    pub reputation: Vec<(String, i32)>,
    /// Quest IDs that become available after this reward is granted.
    pub unlock_quests: Vec<QuestId>,
}

impl Reward {
    pub fn new() -> Self { Self::default() }

    pub fn with_experience(mut self, xp: u32) -> Self { self.experience = xp; self }
    pub fn with_gold(mut self, gold: u32) -> Self { self.gold = gold; self }
    pub fn with_item(mut self, item_id: u32, qty: u32) -> Self {
        self.items.push((item_id, qty));
        self
    }
    pub fn with_reputation(mut self, faction: impl Into<String>, delta: i32) -> Self {
        self.reputation.push((faction.into(), delta));
        self
    }
    pub fn unlock(mut self, quest_id: QuestId) -> Self {
        self.unlock_quests.push(quest_id);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.experience == 0
            && self.gold == 0
            && self.items.is_empty()
            && self.reputation.is_empty()
            && self.unlock_quests.is_empty()
    }

    /// Merge another reward into this one (additive).
    pub fn merge(&mut self, other: &Reward) {
        self.experience += other.experience;
        self.gold += other.gold;
        self.items.extend(other.items.iter().cloned());
        self.reputation.extend(other.reputation.iter().cloned());
        self.unlock_quests.extend(other.unlock_quests.iter().cloned());
    }
}

// ── Prerequisite ─────────────────────────────────────────────────────────────

/// Conditions that must be satisfied before a quest becomes available.
#[derive(Debug, Clone)]
pub enum Prerequisite {
    /// Another quest must have been completed.
    QuestComplete(QuestId),
    /// Another quest must have been failed (opens an alternative path).
    QuestFailed(QuestId),
    /// Player must be at or above this level.
    MinLevel(u32),
    /// A world/journal flag must be set.
    HasFlag(String),
    /// A world/journal flag must NOT be set.
    NotFlag(String),
    /// All of the listed prerequisites must hold.
    All(Vec<Prerequisite>),
    /// At least one of the listed prerequisites must hold.
    Any(Vec<Prerequisite>),
}

impl Prerequisite {
    /// Evaluate this prerequisite against a `PrerequisiteView`.
    ///
    /// `view` is constructed from the journal's current state and passed in
    /// so we can query completed/failed quests and flags without introducing
    /// a circular module dependency.
    pub fn check(&self, view: &PrerequisiteView) -> bool {
        match self {
            Prerequisite::QuestComplete(id) => view.quest_is_complete(*id),
            Prerequisite::QuestFailed(id)   => view.quest_is_failed(*id),
            Prerequisite::MinLevel(lvl)     => view.player_level >= *lvl,
            Prerequisite::HasFlag(flag)     => view.has_flag(flag),
            Prerequisite::NotFlag(flag)     => !view.has_flag(flag),
            Prerequisite::All(list)         => list.iter().all(|p| p.check(view)),
            Prerequisite::Any(list)         => list.iter().any(|p| p.check(view)),
        }
    }
}

// ── ObjectiveType ─────────────────────────────────────────────────────────────

/// The kind of in-game action that advances an objective.
#[derive(Debug, Clone)]
pub enum ObjectiveType {
    /// Kill enemies matching `enemy_type`.
    Kill { enemy_type: String },
    /// Collect items matching `item_id`.
    Collect { item_id: u32, count: u32 },
    /// Physically reach a named location.
    Reach { location: String },
    /// Initiate or complete dialogue with an NPC.
    Talk { npc_id: u32 },
    /// Successfully craft an item at a station.
    Craft { item_id: u32 },
    /// Survive for `duration` seconds without dying.
    Survive { duration: f32 },
    /// Escort an NPC to a destination without it dying.
    Escort { npc_id: u32, destination: String },
    /// Keep an NPC alive for `duration` seconds.
    Protect { npc_id: u32, duration: f32 },
    /// Freeform event identified by a string key.
    Custom { key: String },
}

impl ObjectiveType {
    pub fn label(&self) -> &str {
        match self {
            ObjectiveType::Kill { .. }    => "Kill",
            ObjectiveType::Collect { .. } => "Collect",
            ObjectiveType::Reach { .. }   => "Reach",
            ObjectiveType::Talk { .. }    => "Talk",
            ObjectiveType::Craft { .. }   => "Craft",
            ObjectiveType::Survive { .. } => "Survive",
            ObjectiveType::Escort { .. }  => "Escort",
            ObjectiveType::Protect { .. } => "Protect",
            ObjectiveType::Custom { .. }  => "Custom",
        }
    }
}

// ── ObjectiveDef ──────────────────────────────────────────────────────────────

/// Static definition of one objective within a quest.
#[derive(Debug, Clone)]
pub struct ObjectiveDef {
    /// Unique identifier within the owning quest.
    pub id: ObjectiveId,
    /// Player-facing description.
    pub description: String,
    /// What game event drives this objective.
    pub obj_type: ObjectiveType,
    /// How many times the event must occur (1 = one-shot).
    pub target_count: u32,
    /// If true, failing this objective does not fail the whole quest.
    pub optional: bool,
    /// If true, the objective is not shown in the journal until active.
    pub hidden: bool,
}

impl ObjectiveDef {
    pub fn new(
        id: ObjectiveId,
        description: impl Into<String>,
        obj_type: ObjectiveType,
        target_count: u32,
    ) -> Self {
        Self {
            id,
            description: description.into(),
            obj_type,
            target_count,
            optional: false,
            hidden: false,
        }
    }

    pub fn optional(mut self) -> Self { self.optional = true; self }
    pub fn hidden(mut self) -> Self { self.hidden = true; self }
}

// ── QuestDef ──────────────────────────────────────────────────────────────────

/// Complete, immutable definition of a quest. Stored in `QuestDatabase`.
#[derive(Debug, Clone)]
pub struct QuestDef {
    /// Unique quest identifier.
    pub id: QuestId,
    /// Short player-facing title.
    pub title: String,
    /// Longer description shown in the journal.
    pub description: String,
    /// Category for filtering / UI bucketing.
    pub category: QuestCategory,
    /// Sort priority for journal display.
    pub priority: QuestPriority,
    /// Conditions that must be true before the quest is offered.
    pub prerequisites: Vec<Prerequisite>,
    /// Ordered list of objectives.
    pub objectives: Vec<ObjectiveDef>,
    /// Reward granted on completion.
    pub reward: Reward,
    /// Optional hard time limit in seconds (wall-clock time while quest is active).
    pub time_limit: Option<f32>,
    /// Whether the quest can be accepted again after completion/failure.
    pub repeatable: bool,
    /// If true, the quest won't appear in available-quest lists until prerequisites are met.
    pub hidden_until_available: bool,
    /// Arbitrary string tags for scripting and filtering.
    pub tags: Vec<String>,
}

impl QuestDef {
    pub fn new(
        id: QuestId,
        title: impl Into<String>,
        description: impl Into<String>,
        category: QuestCategory,
        priority: QuestPriority,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            category,
            priority,
            prerequisites: Vec::new(),
            objectives: Vec::new(),
            reward: Reward::default(),
            time_limit: None,
            repeatable: false,
            hidden_until_available: false,
            tags: Vec::new(),
        }
    }

    pub fn with_prerequisite(mut self, p: Prerequisite) -> Self {
        self.prerequisites.push(p);
        self
    }

    pub fn with_objective(mut self, obj: ObjectiveDef) -> Self {
        self.objectives.push(obj);
        self
    }

    pub fn with_reward(mut self, reward: Reward) -> Self {
        self.reward = reward;
        self
    }

    pub fn with_time_limit(mut self, seconds: f32) -> Self {
        self.time_limit = Some(seconds);
        self
    }

    pub fn repeatable(mut self) -> Self { self.repeatable = true; self }
    pub fn hidden(mut self) -> Self { self.hidden_until_available = true; self }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Number of required (non-optional) objectives.
    pub fn required_objective_count(&self) -> usize {
        self.objectives.iter().filter(|o| !o.optional).count()
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

// ── PrerequisiteView (lightweight view used by Prerequisite::check) ───────────

/// A thin read-only snapshot provided to `Prerequisite::check` so it can
/// query completed/failed quests and flags without a dependency on the full
/// journal type (which would create a circular module dependency).
///
/// Constructed by `QuestJournal::prerequisite_view()`.
#[derive(Debug, Clone)]
pub struct PrerequisiteView {
    pub player_level: u32,
    pub completed_quests: std::collections::HashSet<QuestId>,
    pub failed_quests: std::collections::HashSet<QuestId>,
    pub flags: std::collections::HashSet<String>,
}

impl PrerequisiteView {
    pub fn new(
        player_level: u32,
        completed: std::collections::HashSet<QuestId>,
        failed: std::collections::HashSet<QuestId>,
        flags: std::collections::HashSet<String>,
    ) -> Self {
        Self { player_level, completed_quests: completed, failed_quests: failed, flags }
    }

    pub fn quest_is_complete(&self, id: QuestId) -> bool {
        self.completed_quests.contains(&id)
    }

    pub fn quest_is_failed(&self, id: QuestId) -> bool {
        self.failed_quests.contains(&id)
    }

    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }
}

// ── QuestDatabase ─────────────────────────────────────────────────────────────

/// Registry of all known `QuestDef`s. Typically wrapped in `Arc` and shared
/// between the journal and tracker.
#[derive(Debug, Default)]
pub struct QuestDatabase {
    quests: HashMap<QuestId, QuestDef>,
}

impl QuestDatabase {
    pub fn new() -> Self { Self::default() }

    /// Register a quest definition. Returns `false` and does nothing if
    /// a quest with the same id already exists.
    pub fn register(&mut self, def: QuestDef) -> bool {
        if self.quests.contains_key(&def.id) { return false; }
        self.quests.insert(def.id, def);
        true
    }

    /// Overwrite an existing definition (useful for hot-reload).
    pub fn register_or_replace(&mut self, def: QuestDef) {
        self.quests.insert(def.id, def);
    }

    pub fn get(&self, id: QuestId) -> Option<&QuestDef> {
        self.quests.get(&id)
    }

    /// All quests whose minimum-level prerequisite is at most `level`.
    /// Does not check other prerequisites.
    pub fn available_for_level(&self, level: u32) -> Vec<&QuestDef> {
        self.quests
            .values()
            .filter(|def| {
                def.prerequisites.iter().all(|p| match p {
                    Prerequisite::MinLevel(min) => level >= *min,
                    _ => true,
                })
            })
            .collect()
    }

    pub fn get_by_category(&self, category: QuestCategory) -> Vec<&QuestDef> {
        self.quests.values().filter(|d| d.category == category).collect()
    }

    pub fn get_by_tag(&self, tag: &str) -> Vec<&QuestDef> {
        self.quests.values().filter(|d| d.has_tag(tag)).collect()
    }

    pub fn len(&self) -> usize { self.quests.len() }
    pub fn is_empty(&self) -> bool { self.quests.is_empty() }

    pub fn all(&self) -> impl Iterator<Item = &QuestDef> {
        self.quests.values()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make_db() -> QuestDatabase {
        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            QuestId(1),
            "The First Hunt",
            "Slay 5 goblins.",
            QuestCategory::Combat,
            QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            ObjectiveId(1),
            "Kill goblins",
            ObjectiveType::Kill { enemy_type: "goblin".into() },
            5,
        ))
        .with_reward(Reward::new().with_experience(100).with_gold(50));
        db.register(def);
        db
    }

    #[test]
    fn register_and_retrieve() {
        let db = make_db();
        assert!(db.get(QuestId(1)).is_some());
        assert!(db.get(QuestId(99)).is_none());
    }

    #[test]
    fn duplicate_register_is_noop() {
        let mut db = make_db();
        let def2 = QuestDef::new(
            QuestId(1),
            "Duplicate",
            "Should not overwrite",
            QuestCategory::Side,
            QuestPriority::Low,
        );
        assert!(!db.register(def2));
        assert_eq!(db.get(QuestId(1)).unwrap().title, "The First Hunt");
    }

    #[test]
    fn available_for_level_filter() {
        let mut db = QuestDatabase::new();
        let gated = QuestDef::new(
            QuestId(2),
            "Advanced Quest",
            "Requires level 10",
            QuestCategory::Main,
            QuestPriority::High,
        )
        .with_prerequisite(Prerequisite::MinLevel(10));
        db.register(gated);
        assert!(db.available_for_level(5).is_empty());
        assert_eq!(db.available_for_level(10).len(), 1);
    }

    #[test]
    fn prerequisite_flag_check() {
        let progress = PrerequisiteView::new(
            1,
            HashSet::new(),
            HashSet::new(),
            ["found_cave".to_string()].iter().cloned().collect(),
        );
        assert!(Prerequisite::HasFlag("found_cave".into()).check(&progress));
        assert!(!Prerequisite::HasFlag("missing_flag".into()).check(&progress));
        assert!(Prerequisite::NotFlag("missing_flag".into()).check(&progress));
    }

    #[test]
    fn reward_builder_chain() {
        let r = Reward::new()
            .with_experience(500)
            .with_gold(200)
            .with_item(42, 3)
            .with_reputation("Ironguard", 10)
            .unlock(QuestId(5));
        assert_eq!(r.experience, 500);
        assert_eq!(r.gold, 200);
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.reputation.len(), 1);
        assert_eq!(r.unlock_quests.len(), 1);
    }

    #[test]
    fn reward_merge() {
        let mut a = Reward::new().with_experience(100).with_gold(50);
        let b = Reward::new().with_experience(200).with_gold(75).with_item(1, 2);
        a.merge(&b);
        assert_eq!(a.experience, 300);
        assert_eq!(a.gold, 125);
        assert_eq!(a.items.len(), 1);
    }

    #[test]
    fn quest_category_label() {
        assert_eq!(QuestCategory::Daily.label(), "Daily");
        assert!(QuestCategory::Daily.is_time_limited());
        assert!(!QuestCategory::Combat.is_time_limited());
    }

    #[test]
    fn quest_state_terminal() {
        assert!(QuestState::Completed.is_terminal());
        assert!(QuestState::Failed.is_terminal());
        assert!(QuestState::Abandoned.is_terminal());
        assert!(!QuestState::Active.is_terminal());
    }

    #[test]
    fn prerequisite_all_any() {
        let progress = PrerequisiteView::new(
            5,
            [QuestId(1)].iter().cloned().collect(),
            HashSet::new(),
            HashSet::new(),
        );
        let all_ok = Prerequisite::All(vec![
            Prerequisite::QuestComplete(QuestId(1)),
            Prerequisite::MinLevel(5),
        ]);
        assert!(all_ok.check(&progress));

        let all_fail = Prerequisite::All(vec![
            Prerequisite::QuestComplete(QuestId(1)),
            Prerequisite::MinLevel(10),
        ]);
        assert!(!all_fail.check(&progress));

        let any_ok = Prerequisite::Any(vec![
            Prerequisite::MinLevel(10),
            Prerequisite::QuestComplete(QuestId(1)),
        ]);
        assert!(any_ok.check(&progress));
    }
}
