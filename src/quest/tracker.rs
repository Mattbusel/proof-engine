//! # Quest Tracker
//!
//! Bridges raw game events to the `QuestJournal` by mapping every incoming
//! `GameEventType` to all matching active objectives across every active quest.
//!
//! Typical usage in a game loop:
//!
//! ```text
//! // each frame / event:
//! let events = tracker.on_kill("goblin".to_string(), 1);
//! for event in events { ui.handle_quest_event(event); }
//!
//! // every frame:
//! let tick_events = tracker.tick(delta);
//! ```

use std::collections::VecDeque;
use std::sync::Arc;

use super::{
    journal::{JournalError, ObjectiveAdvanceResult, QuestEvent, QuestJournal},
    ObjectiveId, ObjectiveType, QuestDatabase, QuestDef, QuestId, Reward,
};

// ── GameEventType ─────────────────────────────────────────────────────────────

/// A discrete in-game occurrence that may advance one or more quest objectives.
#[derive(Debug, Clone)]
pub enum GameEventType {
    /// An entity of `entity_type` was killed (`count` times).
    EntityKilled { entity_type: String, count: u32 },
    /// An item was picked up (`item_id`, `count` times).
    ItemPickedUp { item_id: u32, count: u32 },
    /// The player entered a named location.
    LocationReached(String),
    /// The player completed dialogue with an NPC.
    NpcTalkedTo(u32),
    /// The player crafted an item.
    ItemCrafted { item_id: u32, count: u32 },
    /// Simulation time advanced by this amount (for Survive/Protect timers).
    TimePassed(f32),
    /// An escort NPC reached its destination.
    EscortReached { npc_id: u32, location: String },
    /// An entity that was being protected survived a threat.
    EntityProtected { npc_id: u32 },
    /// A freeform event identified by a string key and numeric value.
    CustomEvent { key: String, value: u32 },
}

impl GameEventType {
    pub fn label(&self) -> &str {
        match self {
            GameEventType::EntityKilled { .. }   => "EntityKilled",
            GameEventType::ItemPickedUp { .. }   => "ItemPickedUp",
            GameEventType::LocationReached(_)    => "LocationReached",
            GameEventType::NpcTalkedTo(_)        => "NpcTalkedTo",
            GameEventType::ItemCrafted { .. }    => "ItemCrafted",
            GameEventType::TimePassed(_)         => "TimePassed",
            GameEventType::EscortReached { .. }  => "EscortReached",
            GameEventType::EntityProtected { .. }=> "EntityProtected",
            GameEventType::CustomEvent { .. }    => "CustomEvent",
        }
    }
}

// ── ObjectiveMapper ───────────────────────────────────────────────────────────

/// Stateless helper that maps a `GameEventType` to all `(quest_id, obj_id,
/// amount)` tuples that should be advanced in the journal.
pub struct ObjectiveMapper;

impl ObjectiveMapper {
    /// Return all `(quest_id, objective_id, amount)` matches for `event`
    /// across every active quest in `journal`.
    pub fn find_matching_objectives(
        event: &GameEventType,
        journal: &QuestJournal,
        db: &QuestDatabase,
    ) -> Vec<(QuestId, ObjectiveId, u32)> {
        let mut matches = Vec::new();

        for progress in journal.active_quests() {
            let qid = progress.def_id;
            let def = match db.get(qid) {
                Some(d) => d,
                None => continue,
            };

            for obj_def in &def.objectives {
                // Skip already-complete objectives
                if let Some(op) = progress.objectives.get(&obj_def.id) {
                    if op.is_done() { continue; }
                }

                if let Some(amount) = Self::event_matches_objective(event, obj_def, def) {
                    if amount > 0 {
                        matches.push((qid, obj_def.id, amount));
                    }
                }
            }
        }

        matches
    }

    /// Returns `Some(amount)` if `event` advances `obj_def`, or `None`.
    fn event_matches_objective(
        event: &GameEventType,
        obj_def: &super::ObjectiveDef,
        _quest_def: &QuestDef,
    ) -> Option<u32> {
        match (&obj_def.obj_type, event) {
            // ── Kill ──────────────────────────────────────────────────────
            (
                ObjectiveType::Kill { enemy_type },
                GameEventType::EntityKilled { entity_type, count },
            ) => {
                if Self::type_matches(enemy_type, entity_type) {
                    Some(*count)
                } else {
                    None
                }
            }

            // ── Collect ───────────────────────────────────────────────────
            (
                ObjectiveType::Collect { item_id, .. },
                GameEventType::ItemPickedUp { item_id: picked_id, count },
            ) => {
                if item_id == picked_id { Some(*count) } else { None }
            }

            // ── Reach ─────────────────────────────────────────────────────
            (
                ObjectiveType::Reach { location },
                GameEventType::LocationReached(reached),
            ) => {
                if Self::location_matches(location, reached) { Some(1) } else { None }
            }

            // ── Talk ──────────────────────────────────────────────────────
            (
                ObjectiveType::Talk { npc_id },
                GameEventType::NpcTalkedTo(talked_id),
            ) => {
                if npc_id == talked_id { Some(1) } else { None }
            }

            // ── Craft ─────────────────────────────────────────────────────
            (
                ObjectiveType::Craft { item_id },
                GameEventType::ItemCrafted { item_id: crafted_id, count },
            ) => {
                if item_id == crafted_id { Some(*count) } else { None }
            }

            // ── Survive ───────────────────────────────────────────────────
            // TimePassed advances survive objectives; the target_count in the
            // def is interpreted as whole seconds (u32).  We advance by 1 per
            // full second elapsed (floored).
            (
                ObjectiveType::Survive { duration: _ },
                GameEventType::TimePassed(dt),
            ) => {
                let whole_secs = dt.floor() as u32;
                if whole_secs > 0 { Some(whole_secs) } else { None }
            }

            // ── Escort ────────────────────────────────────────────────────
            (
                ObjectiveType::Escort { npc_id, destination },
                GameEventType::EscortReached { npc_id: enpc, location },
            ) => {
                if npc_id == enpc && Self::location_matches(destination, location) {
                    Some(1)
                } else {
                    None
                }
            }

            // ── Protect ───────────────────────────────────────────────────
            (
                ObjectiveType::Protect { npc_id, duration: _ },
                GameEventType::EntityProtected { npc_id: pnpc },
            ) => {
                if npc_id == pnpc { Some(1) } else { None }
            }

            // ── Custom ────────────────────────────────────────────────────
            (
                ObjectiveType::Custom { key },
                GameEventType::CustomEvent { key: event_key, value },
            ) => {
                if key == event_key { Some(*value) } else { None }
            }

            _ => None,
        }
    }

    /// Case-insensitive enemy type matching; supports wildcards `"*"` and `"any"`.
    fn type_matches(pattern: &str, value: &str) -> bool {
        if pattern == "*" || pattern.eq_ignore_ascii_case("any") {
            return true;
        }
        pattern.eq_ignore_ascii_case(value)
    }

    /// Location matching; supports wildcards and prefix matching with `"*"` suffix.
    fn location_matches(pattern: &str, value: &str) -> bool {
        if pattern == "*" { return true; }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return value.starts_with(prefix);
        }
        pattern.eq_ignore_ascii_case(value)
    }
}

// ── TrackerStats ──────────────────────────────────────────────────────────────

/// Cumulative statistics for the current play session.
#[derive(Debug, Clone, Default)]
pub struct TrackerStats {
    /// Total number of `GameEventType` instances processed.
    pub events_processed: u64,
    /// Total number of individual objective increments applied.
    pub objectives_advanced: u64,
    /// Number of quests completed in this session.
    pub quests_completed: u64,
    /// Number of quests that timed out this session.
    pub quests_timed_out: u64,
    /// Number of kills routed to objectives.
    pub kills_tracked: u64,
    /// Number of item pickups routed to objectives.
    pub items_tracked: u64,
    /// Number of location events routed to objectives.
    pub locations_tracked: u64,
}

impl TrackerStats {
    pub fn new() -> Self { Self::default() }

    fn record_event(&mut self, event: &GameEventType) {
        self.events_processed += 1;
        match event {
            GameEventType::EntityKilled { count, .. } => self.kills_tracked += *count as u64,
            GameEventType::ItemPickedUp { count, .. } => self.items_tracked += *count as u64,
            GameEventType::LocationReached(_)          => self.locations_tracked += 1,
            _ => {}
        }
    }

    fn record_advance(&mut self, result: &ObjectiveAdvanceResult) {
        match result {
            ObjectiveAdvanceResult::Progressed { .. } => self.objectives_advanced += 1,
            ObjectiveAdvanceResult::QuestComplete => {
                self.objectives_advanced += 1;
                self.quests_completed += 1;
            }
            _ => {}
        }
    }
}

// ── RewardDistributor ─────────────────────────────────────────────────────────

/// Accumulates rewards granted by quest completions and makes them available
/// for batch application to the player character.
#[derive(Debug, Default)]
pub struct RewardDistributor {
    pending_rewards: Vec<(QuestId, Reward)>,
}

impl RewardDistributor {
    pub fn new() -> Self { Self::default() }

    /// Queue a reward for distribution.
    pub fn queue(&mut self, quest_id: QuestId, reward: Reward) {
        self.pending_rewards.push((quest_id, reward));
    }

    /// Drain and return all pending rewards.
    pub fn drain(&mut self) -> Vec<(QuestId, Reward)> {
        self.pending_rewards.drain(..).collect()
    }

    /// Peek at pending rewards without consuming them.
    pub fn pending(&self) -> &[(QuestId, Reward)] {
        &self.pending_rewards
    }

    pub fn has_pending(&self) -> bool { !self.pending_rewards.is_empty() }

    pub fn pending_count(&self) -> usize { self.pending_rewards.len() }
}

// ── QuestTracker ──────────────────────────────────────────────────────────────

/// Top-level entry point: owns the journal, database reference, event queue,
/// and routes `GameEventType` instances to objective advancement.
pub struct QuestTracker {
    /// Mutable quest journal.
    pub journal: QuestJournal,
    /// Shared read-only quest database.
    pub db: Arc<QuestDatabase>,
    /// Events waiting to be consumed by the caller.
    pending_events: VecDeque<QuestEvent>,
    /// Quest ids flagged for automatic acceptance when prerequisites are met.
    auto_accept: Vec<QuestId>,
    /// Session statistics.
    pub stats: TrackerStats,
    /// Reward accumulator.
    pub rewards: RewardDistributor,
}

impl QuestTracker {
    // ── Construction ─────────────────────────────────────────────────────

    pub fn new(db: Arc<QuestDatabase>, journal: QuestJournal) -> Self {
        Self {
            journal,
            db,
            pending_events: VecDeque::new(),
            auto_accept: Vec::new(),
            stats: TrackerStats::new(),
            rewards: RewardDistributor::new(),
        }
    }

    // ── Auto-accept ───────────────────────────────────────────────────────

    /// Mark `quest_id` for automatic acceptance when its prerequisites become
    /// satisfied and the player's level is sufficient.
    pub fn mark_auto_accept(&mut self, quest_id: QuestId) {
        if !self.auto_accept.contains(&quest_id) {
            self.auto_accept.push(quest_id);
        }
    }

    /// Check all auto-accept quests and accept any that are now available.
    /// Returns the ids of quests that were accepted.
    pub fn try_auto_accept(&mut self, player_level: u32) -> Vec<QuestId> {
        self.journal.player_level = player_level;

        let candidates: Vec<QuestId> = self.auto_accept.clone();
        let mut accepted = Vec::new();

        for id in candidates {
            if self.journal.is_quest_active(id) { continue; }
            if self.journal.is_quest_complete(id) { continue; }

            if let Some(def) = self.db.get(id) {
                let def_clone = def.clone();
                if self.journal.accept_quest(&def_clone, self.journal.game_time()).is_ok() {
                    accepted.push(id);
                }
            }
        }

        // Drain any started events into our pending queue
        for ev in self.journal.drain_events() {
            self.pending_events.push_back(ev);
        }

        accepted
    }

    // ── Core event processing ─────────────────────────────────────────────

    /// Route a `GameEventType` to all matching active objectives and return
    /// the resulting `QuestEvent`s.
    pub fn process_game_event(&mut self, event: GameEventType) -> Vec<QuestEvent> {
        self.stats.record_event(&event);

        let matches = ObjectiveMapper::find_matching_objectives(&event, &self.journal, &self.db);

        for (quest_id, obj_id, amount) in matches {
            let result = self.journal.advance_objective(quest_id, obj_id, amount, &self.db);
            self.stats.record_advance(&result);
        }

        // Collect any reward events and route them to the distributor
        let journal_events = self.journal.drain_events();
        for ev in &journal_events {
            if let QuestEvent::RewardGranted(qid, reward) = ev {
                self.rewards.queue(*qid, reward.clone());
            }
        }

        for ev in journal_events {
            self.pending_events.push_back(ev);
        }

        self.drain_events()
    }

    // ── Convenience event methods ─────────────────────────────────────────

    /// Notify the tracker that `count` entities of `entity_type` were killed.
    pub fn on_kill(&mut self, entity_type: String, count: u32) -> Vec<QuestEvent> {
        self.process_game_event(GameEventType::EntityKilled { entity_type, count })
    }

    /// Notify the tracker that `count` copies of `item_id` were picked up.
    pub fn on_item_pickup(&mut self, item_id: u32, count: u32) -> Vec<QuestEvent> {
        self.process_game_event(GameEventType::ItemPickedUp { item_id, count })
    }

    /// Notify the tracker that the player reached a named location.
    pub fn on_location_reached(&mut self, location: String) -> Vec<QuestEvent> {
        self.process_game_event(GameEventType::LocationReached(location))
    }

    /// Notify the tracker that the player talked to an NPC.
    pub fn on_npc_talked(&mut self, npc_id: u32) -> Vec<QuestEvent> {
        self.process_game_event(GameEventType::NpcTalkedTo(npc_id))
    }

    /// Notify the tracker that the player crafted an item.
    pub fn on_item_crafted(&mut self, item_id: u32, count: u32) -> Vec<QuestEvent> {
        self.process_game_event(GameEventType::ItemCrafted { item_id, count })
    }

    // ── Tick ──────────────────────────────────────────────────────────────

    /// Advance all active quest timers by `delta` seconds.
    /// Also fires `TimePassed` events for Survive/Protect objectives.
    pub fn tick(&mut self, delta: f32) -> Vec<QuestEvent> {
        // First route a TimePassed event through the mapper
        let time_event = GameEventType::TimePassed(delta);
        self.stats.record_event(&time_event);

        let matches =
            ObjectiveMapper::find_matching_objectives(&time_event, &self.journal, &self.db);

        for (quest_id, obj_id, amount) in matches {
            let result = self.journal.advance_objective(quest_id, obj_id, amount, &self.db);
            self.stats.record_advance(&result);
        }

        // Now advance all timers via the journal tick
        let tick_events = self.journal.tick(delta, &self.db);
        for ev in &tick_events {
            if let QuestEvent::QuestTimedOut(_) = ev {
                self.stats.quests_timed_out += 1;
            }
            if let QuestEvent::RewardGranted(qid, reward) = ev {
                self.rewards.queue(*qid, reward.clone());
            }
        }

        for ev in tick_events {
            self.pending_events.push_back(ev);
        }

        // Also drain any events from the mapper's advance calls
        let more = self.journal.drain_events();
        for ev in &more {
            if let QuestEvent::RewardGranted(qid, reward) = ev {
                self.rewards.queue(*qid, reward.clone());
            }
        }
        for ev in more { self.pending_events.push_back(ev); }

        self.drain_events()
    }

    // ── Event drain ───────────────────────────────────────────────────────

    /// Drain and return all pending quest events.
    pub fn drain_events(&mut self) -> Vec<QuestEvent> {
        self.pending_events.drain(..).collect()
    }

    // ── Journal delegation ────────────────────────────────────────────────

    pub fn set_flag(&mut self, flag: impl Into<String>) { self.journal.set_flag(flag); }
    pub fn has_flag(&self, flag: &str) -> bool { self.journal.has_flag(flag) }
    pub fn clear_flag(&mut self, flag: &str) { self.journal.clear_flag(flag); }

    pub fn accept_quest(&mut self, quest_id: QuestId) -> Result<(), JournalError> {
        let def = self.db.get(quest_id).ok_or(JournalError::QuestNotFound)?.clone();
        let time = self.journal.game_time();
        let result = self.journal.accept_quest(&def, time);
        // Drain events into pending queue
        for ev in self.journal.drain_events() {
            self.pending_events.push_back(ev);
        }
        result
    }

    pub fn is_quest_active(&self, id: QuestId) -> bool { self.journal.is_quest_active(id) }
    pub fn is_quest_complete(&self, id: QuestId) -> bool { self.journal.is_quest_complete(id) }
    pub fn player_level(&self) -> u32 { self.journal.player_level }
    pub fn set_player_level(&mut self, level: u32) { self.journal.player_level = level; }
}

// ── TrackerSession ────────────────────────────────────────────────────────────

/// A higher-level wrapper around `QuestTracker` that provides batch event
/// processing and session-level bookkeeping.
pub struct TrackerSession {
    pub tracker: QuestTracker,
    /// Events accumulated since last flush.
    event_log: Vec<QuestEvent>,
    /// Whether batch mode is active (events are buffered, not returned live).
    batch_mode: bool,
    session_id: u64,
}

impl TrackerSession {
    pub fn new(db: Arc<QuestDatabase>, journal: QuestJournal) -> Self {
        Self {
            tracker: QuestTracker::new(db, journal),
            event_log: Vec::new(),
            batch_mode: false,
            session_id: 0,
        }
    }

    /// Enable batch mode: events accumulate in `event_log` instead of being
    /// returned immediately.
    pub fn begin_batch(&mut self) {
        self.batch_mode = true;
        self.session_id += 1;
    }

    /// Disable batch mode and return all accumulated events.
    pub fn end_batch(&mut self) -> Vec<QuestEvent> {
        self.batch_mode = false;
        self.event_log.drain(..).collect()
    }

    pub fn session_id(&self) -> u64 { self.session_id }

    /// Process a list of events in order.
    pub fn process_batch(&mut self, events: Vec<GameEventType>) -> Vec<QuestEvent> {
        let mut all_results = Vec::new();
        for event in events {
            let results = self.tracker.process_game_event(event);
            if self.batch_mode {
                self.event_log.extend(results);
            } else {
                all_results.extend(results);
            }
        }
        all_results
    }

    /// Route a single event.
    pub fn process(&mut self, event: GameEventType) -> Vec<QuestEvent> {
        let results = self.tracker.process_game_event(event);
        if self.batch_mode {
            self.event_log.extend(results);
            Vec::new()
        } else {
            results
        }
    }

    pub fn tick(&mut self, delta: f32) -> Vec<QuestEvent> {
        let results = self.tracker.tick(delta);
        if self.batch_mode {
            self.event_log.extend(results);
            Vec::new()
        } else {
            results
        }
    }

    /// How many events are buffered in the current batch.
    pub fn buffered_event_count(&self) -> usize { self.event_log.len() }

    /// Peek at buffered events without consuming them.
    pub fn buffered_events(&self) -> &[QuestEvent] { &self.event_log }

    pub fn stats(&self) -> &TrackerStats { &self.tracker.stats }

    pub fn drain_rewards(&mut self) -> Vec<(QuestId, Reward)> {
        self.tracker.rewards.drain()
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Build a minimal tracker for tests: register one kill quest and auto-accept it.
#[cfg(test)]
fn make_tracker(
    quest_id: u32,
    obj_id: u32,
    enemy: &str,
    kill_count: u32,
) -> (QuestTracker, QuestId, ObjectiveId) {
    use super::{
        ObjectiveDef, ObjectiveType, QuestCategory, QuestDef, QuestPriority, Reward,
    };

    let qid = QuestId(quest_id);
    let oid = ObjectiveId(obj_id);

    let mut db = QuestDatabase::new();
    let def = QuestDef::new(
        qid,
        "Test Kill Quest",
        "Kill enemies.",
        QuestCategory::Combat,
        QuestPriority::Normal,
    )
    .with_objective(ObjectiveDef::new(
        oid,
        "Kill enemies",
        ObjectiveType::Kill { enemy_type: enemy.to_string() },
        kill_count,
    ))
    .with_reward(Reward::new().with_experience(200).with_gold(100));
    db.register(def.clone());

    let db = Arc::new(db);
    let mut journal = QuestJournal::new();
    journal.accept_quest(&def, 0.0).unwrap();
    let _ = journal.drain_events();

    let tracker = QuestTracker::new(Arc::clone(&db), journal);
    (tracker, qid, oid)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{
        journal::QuestJournal,
        ObjectiveDef, ObjectiveId, ObjectiveState, ObjectiveType, QuestCategory, QuestDef,
        QuestId, QuestPriority, QuestState, Reward,
    };

    // ── helpers ──────────────────────────────────────────────────────────

    fn kill_quest_tracker(enemy: &str, needed: u32) -> (QuestTracker, QuestId, ObjectiveId) {
        make_tracker(1, 1, enemy, needed)
    }

    fn make_collect_tracker(item_id: u32, needed: u32) -> (QuestTracker, QuestId, ObjectiveId) {
        let qid = QuestId(2);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid,
            "Collect Quest",
            "Collect items.",
            QuestCategory::Side,
            QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            oid,
            "Collect items",
            ObjectiveType::Collect { item_id, count: needed },
            needed,
        ))
        .with_reward(Reward::new().with_experience(100));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let tracker = QuestTracker::new(Arc::clone(&db), journal);
        (tracker, qid, oid)
    }

    // ── kill event routing ────────────────────────────────────────────────

    #[test]
    fn kill_event_advances_objective() {
        let (mut tracker, qid, oid) = kill_quest_tracker("goblin", 5);

        let events = tracker.on_kill("goblin".to_string(), 2);
        assert!(events.iter().any(|e| matches!(e,
            QuestEvent::ObjectiveUpdated { quest, obj, progress: 2 }
            if *quest == qid && *obj == oid
        )));

        let progress = tracker.journal.get_progress(qid).unwrap();
        assert_eq!(progress.objectives[&oid].current, 2);
    }

    #[test]
    fn kill_event_wrong_type_no_effect() {
        let (mut tracker, qid, _) = kill_quest_tracker("goblin", 5);
        let events = tracker.on_kill("orc".to_string(), 3);
        // No objective update events
        assert!(!events.iter().any(|e| matches!(e, QuestEvent::ObjectiveUpdated { .. })));
        // Objective still at 0
        let p = tracker.journal.get_progress(qid).unwrap();
        assert_eq!(p.objectives[&ObjectiveId(1)].current, 0);
    }

    #[test]
    fn kill_event_case_insensitive() {
        let (mut tracker, qid, oid) = kill_quest_tracker("Goblin", 3);
        let events = tracker.on_kill("goblin".to_string(), 1);
        assert!(events.iter().any(|e| matches!(e, QuestEvent::ObjectiveUpdated { .. })));
    }

    #[test]
    fn kill_wildcard_matches_any_enemy() {
        let qid = QuestId(99);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid,
            "Any Kill Quest",
            "Kill anything.",
            QuestCategory::Combat,
            QuestPriority::Low,
        )
        .with_objective(ObjectiveDef::new(
            oid,
            "Kill any enemy",
            ObjectiveType::Kill { enemy_type: "*".to_string() },
            3,
        ))
        .with_reward(Reward::new().with_experience(50));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        tracker.on_kill("dragon".to_string(), 1);
        tracker.on_kill("rat".to_string(), 2);

        let p = tracker.journal.get_progress(qid).unwrap();
        assert_eq!(p.objectives[&oid].current, 3);
        assert_eq!(p.objectives[&oid].state, ObjectiveState::Completed);
    }

    // ── multi-quest kill routing ──────────────────────────────────────────

    #[test]
    fn kill_advances_multiple_active_quests() {
        let mut db = QuestDatabase::new();

        let q1 = QuestId(1);
        let q2 = QuestId(2);
        let o1 = ObjectiveId(1);

        for (qid, enemy) in &[(q1, "wolf"), (q2, "wolf")] {
            let def = QuestDef::new(
                *qid,
                format!("Wolf Quest {}", qid.raw()),
                "Kill wolves.",
                QuestCategory::Bounty,
                QuestPriority::Normal,
            )
            .with_objective(ObjectiveDef::new(
                o1,
                "Kill wolves",
                ObjectiveType::Kill { enemy_type: enemy.to_string() },
                5,
            ))
            .with_reward(Reward::new().with_experience(100));
            db.register(def);
        }

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();

        for qid in &[q1, q2] {
            let def = db.get(*qid).unwrap().clone();
            journal.accept_quest(&def, 0.0).unwrap();
        }
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        let events = tracker.on_kill("wolf".to_string(), 2);

        // Both quests should receive updates
        let updated_quests: HashSet<QuestId> = events
            .iter()
            .filter_map(|e| {
                if let QuestEvent::ObjectiveUpdated { quest, .. } = e { Some(*quest) } else { None }
            })
            .collect();
        assert!(updated_quests.contains(&q1));
        assert!(updated_quests.contains(&q2));

        // Both should be at 2
        assert_eq!(tracker.journal.get_progress(q1).unwrap().objectives[&o1].current, 2);
        assert_eq!(tracker.journal.get_progress(q2).unwrap().objectives[&o1].current, 2);
    }

    // ── multi-objective quest completion ──────────────────────────────────

    #[test]
    fn multi_objective_quest_requires_all() {
        let qid = QuestId(5);
        let o_kill = ObjectiveId(1);
        let o_collect = ObjectiveId(2);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid,
            "Hunt and Gather",
            "Kill 2 boars and collect 3 tusks.",
            QuestCategory::Side,
            QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            o_kill,
            "Kill boars",
            ObjectiveType::Kill { enemy_type: "boar".into() },
            2,
        ))
        .with_objective(ObjectiveDef::new(
            o_collect,
            "Collect tusks",
            ObjectiveType::Collect { item_id: 77, count: 3 },
            3,
        ))
        .with_reward(Reward::new().with_experience(300).with_gold(150));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);

        // Complete kills — quest should still be active
        let events = tracker.on_kill("boar".to_string(), 2);
        assert!(!events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(_))));
        assert!(tracker.is_quest_active(qid));

        // Complete collection — quest should complete
        let events = tracker.on_item_pickup(77, 3);
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
        assert!(!tracker.is_quest_active(qid));
        assert!(tracker.is_quest_complete(qid));
    }

    // ── collect event ─────────────────────────────────────────────────────

    #[test]
    fn collect_event_advances_objective() {
        let (mut tracker, qid, oid) = make_collect_tracker(42, 5);

        tracker.on_item_pickup(42, 3);
        let p = tracker.journal.get_progress(qid).unwrap();
        assert_eq!(p.objectives[&oid].current, 3);
    }

    #[test]
    fn collect_wrong_item_no_effect() {
        let (mut tracker, qid, oid) = make_collect_tracker(42, 5);
        tracker.on_item_pickup(99, 3);
        let p = tracker.journal.get_progress(qid).unwrap();
        assert_eq!(p.objectives[&oid].current, 0);
    }

    // ── location event ────────────────────────────────────────────────────

    #[test]
    fn location_reached_advances_objective() {
        let qid = QuestId(10);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid,
            "Explore the Keep",
            "Reach the ancient keep.",
            QuestCategory::Exploration,
            QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            oid,
            "Reach Ancient Keep",
            ObjectiveType::Reach { location: "ancient_keep".into() },
            1,
        ))
        .with_reward(Reward::new().with_experience(50));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        let events = tracker.on_location_reached("ancient_keep".to_string());

        assert!(events.iter().any(|e| matches!(e, QuestEvent::ObjectiveComplete(id, _) if *id == qid)));
        assert!(tracker.is_quest_complete(qid));
    }

    #[test]
    fn location_wrong_destination_no_effect() {
        let qid = QuestId(10);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Find the Tower", "Reach tower.",
            QuestCategory::Exploration, QuestPriority::Low,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Go to tower",
            ObjectiveType::Reach { location: "tower".into() }, 1,
        ))
        .with_reward(Reward::new());
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        let events = tracker.on_location_reached("dungeon".to_string());
        assert!(!events.iter().any(|e| matches!(e, QuestEvent::ObjectiveComplete(_, _))));
    }

    // ── NPC talk ──────────────────────────────────────────────────────────

    #[test]
    fn npc_talk_advances_objective() {
        let qid = QuestId(11);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Speak to the Elder", "Find and talk to the elder.",
            QuestCategory::Social, QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Talk to Elder",
            ObjectiveType::Talk { npc_id: 42 }, 1,
        ))
        .with_reward(Reward::new().with_reputation("Villagers", 5));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        let events = tracker.on_npc_talked(42);
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
    }

    // ── crafting ──────────────────────────────────────────────────────────

    #[test]
    fn craft_event_advances_objective() {
        let qid = QuestId(12);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Master Crafter", "Craft 3 potions.",
            QuestCategory::Crafting, QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Craft potions",
            ObjectiveType::Craft { item_id: 55 }, 3,
        ))
        .with_reward(Reward::new().with_experience(120));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        tracker.on_item_crafted(55, 2);
        let p = tracker.journal.get_progress(qid).unwrap();
        assert_eq!(p.objectives[&oid].current, 2);

        let events = tracker.on_item_crafted(55, 1);
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
    }

    // ── escort ────────────────────────────────────────────────────────────

    #[test]
    fn escort_event_completes_objective() {
        let qid = QuestId(13);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Safe Passage", "Escort the merchant to town.",
            QuestCategory::Side, QuestPriority::High,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Escort merchant to town",
            ObjectiveType::Escort { npc_id: 7, destination: "town".into() }, 1,
        ))
        .with_reward(Reward::new().with_gold(200));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        let events = tracker.process_game_event(
            GameEventType::EscortReached { npc_id: 7, location: "town".into() }
        );
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
    }

    // ── custom event ──────────────────────────────────────────────────────

    #[test]
    fn custom_event_advances_objective() {
        let qid = QuestId(14);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Special Challenge", "Complete the ritual 3 times.",
            QuestCategory::Side, QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Complete ritual",
            ObjectiveType::Custom { key: "ritual_complete".into() }, 3,
        ))
        .with_reward(Reward::new().with_experience(500));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        tracker.process_game_event(GameEventType::CustomEvent { key: "ritual_complete".into(), value: 2 });
        let p = tracker.journal.get_progress(qid).unwrap();
        assert_eq!(p.objectives[&oid].current, 2);

        let events = tracker.process_game_event(
            GameEventType::CustomEvent { key: "ritual_complete".into(), value: 1 }
        );
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
    }

    // ── tick / time-based ─────────────────────────────────────────────────

    #[test]
    fn tick_advances_survive_objective() {
        let qid = QuestId(20);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Survive 10s", "Survive in the arena for 10 seconds.",
            QuestCategory::Combat, QuestPriority::High,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Survive",
            ObjectiveType::Survive { duration: 10.0 }, 10,
        ))
        .with_reward(Reward::new().with_experience(400));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);

        // 5 seconds should advance objective to 5
        tracker.tick(5.0);
        let p = tracker.journal.get_progress(qid).unwrap();
        assert_eq!(p.objectives[&oid].current, 5);

        // 5 more should complete it
        let events = tracker.tick(5.0);
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
    }

    #[test]
    fn tick_triggers_timeout() {
        let qid = QuestId(21);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Timed Delivery", "Deliver the package in 5 seconds.",
            QuestCategory::Side, QuestPriority::Critical,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Deliver package",
            ObjectiveType::Reach { location: "warehouse".into() }, 1,
        ))
        .with_reward(Reward::new().with_gold(50))
        .with_time_limit(5.0);
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);

        let events = tracker.tick(6.0);
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestTimedOut(id) if *id == qid)));
        assert!(!tracker.is_quest_active(qid));
        assert_eq!(tracker.stats.quests_timed_out, 1);
    }

    // ── auto-accept ───────────────────────────────────────────────────────

    #[test]
    fn auto_accept_triggers_when_level_sufficient() {
        let qid = QuestId(30);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Level 5 Quest", "Requires level 5.",
            QuestCategory::Main, QuestPriority::High,
        )
        .with_prerequisite(super::super::Prerequisite::MinLevel(5))
        .with_objective(ObjectiveDef::new(
            oid, "Kill troll",
            ObjectiveType::Kill { enemy_type: "troll".into() }, 1,
        ))
        .with_reward(Reward::new().with_experience(300));
        db.register(def.clone());

        let db = Arc::new(db);
        let journal = QuestJournal::new();
        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);

        tracker.mark_auto_accept(qid);

        // Level 3 — not yet
        let accepted = tracker.try_auto_accept(3);
        assert!(accepted.is_empty());
        assert!(!tracker.is_quest_active(qid));

        // Level 5 — should trigger
        let accepted = tracker.try_auto_accept(5);
        assert_eq!(accepted, vec![qid]);
        assert!(tracker.is_quest_active(qid));
    }

    // ── reward distributor ────────────────────────────────────────────────

    #[test]
    fn reward_distributor_fills_on_quest_complete() {
        let (mut tracker, qid, _) = kill_quest_tracker("rat", 1);

        let events = tracker.on_kill("rat".to_string(), 1);
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(_))));
        assert!(tracker.rewards.has_pending());

        let rewards = tracker.rewards.drain();
        assert_eq!(rewards.len(), 1);
        assert_eq!(rewards[0].0, qid);
        assert_eq!(rewards[0].1.experience, 200);
        assert_eq!(rewards[0].1.gold, 100);
    }

    // ── tracker session batch mode ────────────────────────────────────────

    #[test]
    fn tracker_session_batch_buffers_events() {
        let qid = QuestId(40);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Batch Test Quest", "Kill 3 spiders.",
            QuestCategory::Combat, QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Kill spiders",
            ObjectiveType::Kill { enemy_type: "spider".into() }, 3,
        ))
        .with_reward(Reward::new().with_experience(100));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut session = TrackerSession::new(Arc::clone(&db), journal);
        session.begin_batch();

        // Events should be buffered, not returned
        let immediate = session.process(GameEventType::EntityKilled {
            entity_type: "spider".into(),
            count: 1,
        });
        assert!(immediate.is_empty());
        assert!(session.buffered_event_count() > 0);

        // End batch — should flush all
        let flushed = session.end_batch();
        assert!(!flushed.is_empty());
        assert_eq!(session.buffered_event_count(), 0);
    }

    #[test]
    fn tracker_session_batch_process_multiple() {
        let qid = QuestId(41);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Batch Kill 5", "Kill 5 bats.",
            QuestCategory::Combat, QuestPriority::Low,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Kill bats",
            ObjectiveType::Kill { enemy_type: "bat".into() }, 5,
        ))
        .with_reward(Reward::new().with_experience(80));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut session = TrackerSession::new(Arc::clone(&db), journal);

        let events = session.process_batch(vec![
            GameEventType::EntityKilled { entity_type: "bat".into(), count: 2 },
            GameEventType::EntityKilled { entity_type: "bat".into(), count: 3 },
        ]);

        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
        assert_eq!(session.stats().quests_completed, 1);
    }

    // ── stats tracking ────────────────────────────────────────────────────

    #[test]
    fn stats_count_events_and_advances() {
        let (mut tracker, _, _) = kill_quest_tracker("skeleton", 10);

        tracker.on_kill("skeleton".to_string(), 3);
        tracker.on_kill("skeleton".to_string(), 2);
        tracker.on_item_pickup(1, 5); // irrelevant event

        assert_eq!(tracker.stats.events_processed, 3);
        assert_eq!(tracker.stats.kills_tracked, 5);
        assert_eq!(tracker.stats.objectives_advanced, 2);
    }

    // ── objective mapper directly ─────────────────────────────────────────

    #[test]
    fn objective_mapper_returns_no_matches_for_inactive_quest() {
        let qid = QuestId(50);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Inactive Quest", "Not started.",
            QuestCategory::Side, QuestPriority::Low,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Kill bears",
            ObjectiveType::Kill { enemy_type: "bear".into() }, 2,
        ))
        .with_reward(Reward::new());
        db.register(def.clone());

        // Deliberately do NOT accept the quest
        let journal = QuestJournal::new();
        let event = GameEventType::EntityKilled { entity_type: "bear".into(), count: 1 };
        let matches = ObjectiveMapper::find_matching_objectives(&event, &journal, &db);
        assert!(matches.is_empty());
    }

    // ── protect event ─────────────────────────────────────────────────────

    #[test]
    fn protect_event_advances_objective() {
        let qid = QuestId(60);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Guard Duty", "Protect the wounded soldier.",
            QuestCategory::Combat, QuestPriority::High,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Keep soldier alive",
            ObjectiveType::Protect { npc_id: 9, duration: 1.0 }, 1,
        ))
        .with_reward(Reward::new().with_experience(250).with_reputation("Kingdom", 10));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        let events = tracker.process_game_event(
            GameEventType::EntityProtected { npc_id: 9 }
        );
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
    }

    // ── drain events is empty after drain ─────────────────────────────────

    #[test]
    fn drain_events_empties_queue() {
        let (mut tracker, _, _) = kill_quest_tracker("imp", 10);
        tracker.on_kill("imp".to_string(), 1);
        let first = tracker.drain_events();
        assert!(!first.is_empty()); // should have gotten events from on_kill already, but re-verify drain
        let second = tracker.drain_events();
        assert!(second.is_empty());
    }

    // ── location wildcard ─────────────────────────────────────────────────

    #[test]
    fn location_prefix_wildcard_matches() {
        let qid = QuestId(70);
        let oid = ObjectiveId(1);

        let mut db = QuestDatabase::new();
        let def = QuestDef::new(
            qid, "Forest Explorer", "Reach any forest location.",
            QuestCategory::Exploration, QuestPriority::Normal,
        )
        .with_objective(ObjectiveDef::new(
            oid, "Reach forest area",
            ObjectiveType::Reach { location: "forest_*".into() }, 1,
        ))
        .with_reward(Reward::new().with_experience(75));
        db.register(def.clone());

        let db = Arc::new(db);
        let mut journal = QuestJournal::new();
        journal.accept_quest(&def, 0.0).unwrap();
        let _ = journal.drain_events();

        let mut tracker = QuestTracker::new(Arc::clone(&db), journal);
        let events = tracker.on_location_reached("forest_clearing".to_string());
        assert!(events.iter().any(|e| matches!(e, QuestEvent::QuestComplete(id) if *id == qid)));
    }
}
