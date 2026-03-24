//! Game Systems Module — top-level game state coordinator
//!
//! Provides GameManager, state machine, score system, event bus, session stats,
//! timers, difficulty configuration, and all top-level game coordination.

pub mod menu;
pub mod localization;
pub mod achievements;

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Load Progress ─────────────────────────────────────────────────────────────

/// Describes progress through a multi-stage loading operation.
#[derive(Debug, Clone)]
pub struct LoadProgress {
    pub stage: String,
    pub current: u32,
    pub total: u32,
    pub sub_progress: f32,
}

impl LoadProgress {
    pub fn new(stage: impl Into<String>, total: u32) -> Self {
        Self {
            stage: stage.into(),
            current: 0,
            total,
            sub_progress: 0.0,
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 1.0;
        }
        (self.current as f32 + self.sub_progress) / self.total as f32
    }

    pub fn advance(&mut self, sub: f32) {
        self.sub_progress = sub.clamp(0.0, 1.0);
    }

    pub fn next_stage(&mut self, stage: impl Into<String>) {
        self.current += 1;
        self.sub_progress = 0.0;
        self.stage = stage.into();
    }

    pub fn is_complete(&self) -> bool {
        self.current >= self.total
    }
}

// ─── Game Over Data ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GameOverData {
    pub score: u64,
    pub cause: String,
    pub survival_time: f64,
    pub kills: u32,
    pub level_reached: u32,
}

impl GameOverData {
    pub fn new(score: u64, cause: impl Into<String>, survival_time: f64, kills: u32, level_reached: u32) -> Self {
        Self {
            score,
            cause: cause.into(),
            survival_time,
            kills,
            level_reached,
        }
    }
}

// ─── Game State ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum GameState {
    MainMenu,
    Loading(LoadProgress),
    Playing,
    Paused,
    GameOver(GameOverData),
    Credits,
    Settings,
}

impl GameState {
    pub fn name(&self) -> &str {
        match self {
            GameState::MainMenu => "MainMenu",
            GameState::Loading(_) => "Loading",
            GameState::Playing => "Playing",
            GameState::Paused => "Paused",
            GameState::GameOver(_) => "GameOver",
            GameState::Credits => "Credits",
            GameState::Settings => "Settings",
        }
    }

    pub fn is_playing(&self) -> bool {
        matches!(self, GameState::Playing | GameState::Paused)
    }

    pub fn can_pause(&self) -> bool {
        matches!(self, GameState::Playing)
    }
}

// ─── Transition Animation ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionKind {
    Fade,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    Dissolve,
    Wipe,
    Crossfade,
    None,
}

#[derive(Debug, Clone)]
pub struct GameTransition {
    pub kind: TransitionKind,
    pub duration: f32,
    pub elapsed: f32,
    pub from: String,
    pub to: String,
    pub complete: bool,
}

impl GameTransition {
    pub fn new(kind: TransitionKind, duration: f32, from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            kind,
            duration,
            elapsed: 0.0,
            from: from.into(),
            to: to.into(),
            complete: false,
        }
    }

    pub fn none() -> Self {
        Self::new(TransitionKind::None, 0.0, "", "")
    }

    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt;
        if self.elapsed >= self.duration {
            self.complete = true;
        }
    }

    pub fn alpha(&self) -> f32 {
        match self.kind {
            TransitionKind::Fade | TransitionKind::Crossfade => {
                let p = self.progress();
                if p < 0.5 { p * 2.0 } else { (1.0 - p) * 2.0 }
            }
            TransitionKind::Dissolve => self.progress(),
            _ => 1.0,
        }
    }

    pub fn offset_x(&self) -> f32 {
        let p = self.progress();
        let ease = 1.0 - (1.0 - p).powi(3); // ease-out cubic
        match self.kind {
            TransitionKind::SlideLeft => -ease,
            TransitionKind::SlideRight => ease,
            _ => 0.0,
        }
    }

    pub fn offset_y(&self) -> f32 {
        let p = self.progress();
        let ease = 1.0 - (1.0 - p).powi(3);
        match self.kind {
            TransitionKind::SlideUp => -ease,
            TransitionKind::SlideDown => ease,
            _ => 0.0,
        }
    }
}

// ─── Difficulty Preset ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DifficultyPreset {
    Story,
    Easy,
    Normal,
    Hard,
    Expert,
    Custom,
}

impl DifficultyPreset {
    pub fn name(&self) -> &str {
        match self {
            DifficultyPreset::Story => "Story",
            DifficultyPreset::Easy => "Easy",
            DifficultyPreset::Normal => "Normal",
            DifficultyPreset::Hard => "Hard",
            DifficultyPreset::Expert => "Expert",
            DifficultyPreset::Custom => "Custom",
        }
    }

    pub fn all() -> &'static [DifficultyPreset] {
        &[
            DifficultyPreset::Story,
            DifficultyPreset::Easy,
            DifficultyPreset::Normal,
            DifficultyPreset::Hard,
            DifficultyPreset::Expert,
        ]
    }

    pub fn default_params(&self) -> DifficultyParams {
        match self {
            DifficultyPreset::Story => DifficultyParams {
                damage_scale: 0.4,
                enemy_health_scale: 0.5,
                enemy_speed_scale: 0.7,
                resource_scale: 2.0,
                xp_scale: 1.5,
            },
            DifficultyPreset::Easy => DifficultyParams {
                damage_scale: 0.7,
                enemy_health_scale: 0.75,
                enemy_speed_scale: 0.85,
                resource_scale: 1.5,
                xp_scale: 1.25,
            },
            DifficultyPreset::Normal => DifficultyParams::default(),
            DifficultyPreset::Hard => DifficultyParams {
                damage_scale: 1.5,
                enemy_health_scale: 1.5,
                enemy_speed_scale: 1.2,
                resource_scale: 0.8,
                xp_scale: 1.5,
            },
            DifficultyPreset::Expert => DifficultyParams {
                damage_scale: 2.0,
                enemy_health_scale: 2.5,
                enemy_speed_scale: 1.4,
                resource_scale: 0.6,
                xp_scale: 2.0,
            },
            DifficultyPreset::Custom => DifficultyParams::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DifficultyParams {
    pub damage_scale: f32,
    pub enemy_health_scale: f32,
    pub enemy_speed_scale: f32,
    pub resource_scale: f32,
    pub xp_scale: f32,
}

impl Default for DifficultyParams {
    fn default() -> Self {
        Self {
            damage_scale: 1.0,
            enemy_health_scale: 1.0,
            enemy_speed_scale: 1.0,
            resource_scale: 1.0,
            xp_scale: 1.0,
        }
    }
}

impl DifficultyParams {
    pub fn scale_damage(&self, base: f32) -> f32 {
        base * self.damage_scale
    }

    pub fn scale_enemy_health(&self, base: f32) -> f32 {
        base * self.enemy_health_scale
    }

    pub fn scale_enemy_speed(&self, base: f32) -> f32 {
        base * self.enemy_speed_scale
    }

    pub fn scale_resource(&self, base: f32) -> f32 {
        base * self.resource_scale
    }

    pub fn scale_xp(&self, base: f32) -> f32 {
        base * self.xp_scale
    }
}

// ─── Game Config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GameConfig {
    pub difficulty_preset: DifficultyPreset,
    pub difficulty_params: DifficultyParams,
    pub target_fps: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub voice_volume: f32,
    pub show_fps: bool,
    pub show_damage_numbers: bool,
    pub screen_shake: bool,
    pub colorblind_mode: bool,
    pub high_contrast: bool,
    pub reduce_motion: bool,
    pub large_text: bool,
    pub subtitles: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            difficulty_preset: DifficultyPreset::Normal,
            difficulty_params: DifficultyParams::default(),
            target_fps: 60,
            fullscreen: false,
            vsync: true,
            master_volume: 1.0,
            music_volume: 0.8,
            sfx_volume: 1.0,
            voice_volume: 1.0,
            show_fps: false,
            show_damage_numbers: true,
            screen_shake: true,
            colorblind_mode: false,
            high_contrast: false,
            reduce_motion: false,
            large_text: false,
            subtitles: false,
        }
    }
}

impl GameConfig {
    pub fn set_difficulty(&mut self, preset: DifficultyPreset) {
        self.difficulty_preset = preset;
        if preset != DifficultyPreset::Custom {
            self.difficulty_params = preset.default_params();
        }
    }

    pub fn effective_volume(&self, channel: VolumeChannel) -> f32 {
        let base = match channel {
            VolumeChannel::Music => self.music_volume,
            VolumeChannel::Sfx => self.sfx_volume,
            VolumeChannel::Voice => self.voice_volume,
        };
        base * self.master_volume
    }
}

#[derive(Debug, Clone, Copy)]
pub enum VolumeChannel {
    Music,
    Sfx,
    Voice,
}

// ─── Game Timer ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GameTimer {
    pub elapsed: f64,
    pub paused_elapsed: f64,
    pub session_count: u32,
    pub first_played_at: u64,
    paused: bool,
}

impl GameTimer {
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            elapsed: 0.0,
            paused_elapsed: 0.0,
            session_count: 0,
            first_played_at: now,
            paused: false,
        }
    }

    pub fn tick(&mut self, dt: f64) {
        if self.paused {
            self.paused_elapsed += dt;
        } else {
            self.elapsed += dt;
        }
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn total_elapsed(&self) -> f64 {
        self.elapsed + self.paused_elapsed
    }

    pub fn start_session(&mut self) {
        self.session_count += 1;
    }

    pub fn format_elapsed(&self) -> String {
        let secs = self.elapsed as u64;
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;
        if hours > 0 {
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{}:{:02}", minutes, seconds)
        }
    }
}

impl Default for GameTimer {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Session Stats ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub enemies_killed: u32,
    pub damage_dealt: f64,
    pub damage_taken: f64,
    pub distance_traveled: f64,
    pub items_collected: u32,
    pub gold_earned: u64,
    pub gold_spent: u64,
    pub quests_completed: u32,
    pub deaths: u32,
    pub highest_combo: u32,
    pub critical_hits: u32,
    pub skills_used: u32,
    pub spells_cast: u32,
    pub chests_opened: u32,
    pub secrets_found: u32,
    pub levels_visited: u32,
    pub items_crafted: u32,
    pub boss_kills: u32,
    pub playtime_secs: f64,
    pub max_level_reached: u32,
    pub highest_score: u64,
}

impl SessionStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_kill(&mut self, is_boss: bool) {
        self.enemies_killed += 1;
        if is_boss {
            self.boss_kills += 1;
        }
    }

    pub fn record_damage_dealt(&mut self, amount: f64, is_crit: bool) {
        self.damage_dealt += amount;
        if is_crit {
            self.critical_hits += 1;
        }
    }

    pub fn record_damage_taken(&mut self, amount: f64) {
        self.damage_taken += amount;
    }

    pub fn record_death(&mut self) {
        self.deaths += 1;
    }

    pub fn record_item_collected(&mut self) {
        self.items_collected += 1;
    }

    pub fn record_gold(&mut self, earned: u64, spent: u64) {
        self.gold_earned += earned;
        self.gold_spent += spent;
    }

    pub fn update_combo(&mut self, combo: u32) {
        if combo > self.highest_combo {
            self.highest_combo = combo;
        }
    }

    pub fn k_d_ratio(&self) -> f32 {
        if self.deaths == 0 {
            return self.enemies_killed as f32;
        }
        self.enemies_killed as f32 / self.deaths as f32
    }

    pub fn accuracy(&self) -> f32 {
        if self.skills_used == 0 {
            return 0.0;
        }
        self.critical_hits as f32 / self.skills_used as f32
    }
}

// ─── Score System ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Score {
    pub base: u64,
    pub combo_bonus: u64,
    pub time_bonus: u64,
    pub style_bonus: u64,
    pub total: u64,
}

impl Score {
    pub fn new(base: u64) -> Self {
        Self { base, total: base, ..Default::default() }
    }

    pub fn calculate_total(&mut self) {
        self.total = self.base + self.combo_bonus + self.time_bonus + self.style_bonus;
    }

    pub fn add_combo_bonus(&mut self, bonus: u64) {
        self.combo_bonus += bonus;
        self.calculate_total();
    }

    pub fn add_time_bonus(&mut self, bonus: u64) {
        self.time_bonus += bonus;
        self.calculate_total();
    }

    pub fn add_style_bonus(&mut self, bonus: u64) {
        self.style_bonus += bonus;
        self.calculate_total();
    }

    pub fn add_base(&mut self, amount: u64) {
        self.base += amount;
        self.calculate_total();
    }

    pub fn grade(&self) -> char {
        match self.total {
            0..=999 => 'F',
            1000..=4999 => 'D',
            5000..=9999 => 'C',
            10000..=24999 => 'B',
            25000..=49999 => 'A',
            50000..=99999 => 'S',
            _ => 'X',
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComboTracker {
    pub count: u32,
    pub multiplier: f32,
    pub decay_timer: f32,
    decay_rate: f32,
    max_multiplier: f32,
    combo_window: f32,
}

impl ComboTracker {
    pub fn new() -> Self {
        Self {
            count: 0,
            multiplier: 1.0,
            decay_timer: 0.0,
            decay_rate: 0.5,
            max_multiplier: 8.0,
            combo_window: 3.0,
        }
    }

    pub fn with_decay_rate(mut self, rate: f32) -> Self {
        self.decay_rate = rate;
        self
    }

    pub fn with_max_multiplier(mut self, max: f32) -> Self {
        self.max_multiplier = max;
        self
    }

    pub fn with_combo_window(mut self, window: f32) -> Self {
        self.combo_window = window;
        self
    }

    pub fn hit(&mut self) -> f32 {
        self.count += 1;
        self.decay_timer = self.combo_window;
        self.multiplier = self.calculate_multiplier();
        self.multiplier
    }

    fn calculate_multiplier(&self) -> f32 {
        let mult = 1.0 + (self.count as f32 / 10.0).ln_1p() * 2.0;
        mult.min(self.max_multiplier)
    }

    pub fn tick(&mut self, dt: f32) {
        if self.decay_timer > 0.0 {
            self.decay_timer -= dt;
            if self.decay_timer <= 0.0 {
                self.reset();
            }
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.multiplier = 1.0;
        self.decay_timer = 0.0;
    }

    pub fn apply_to_score(&self, base_score: u64) -> u64 {
        (base_score as f32 * self.multiplier) as u64
    }

    pub fn is_active(&self) -> bool {
        self.count > 0 && self.decay_timer > 0.0
    }
}

impl Default for ComboTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ScoreEntry {
    pub name: String,
    pub score: u64,
    pub date: u64,
    pub metadata: HashMap<String, String>,
}

impl ScoreEntry {
    pub fn new(name: impl Into<String>, score: u64) -> Self {
        let date = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            name: name.into(),
            score,
            date,
            metadata: HashMap::new(),
        }
    }

    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct HighScoreTable {
    pub entries: Vec<ScoreEntry>,
    pub max_entries: usize,
}

impl HighScoreTable {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ScoreEntry) -> usize {
        let rank = self.entries.iter().position(|e| e.score < entry.score)
            .unwrap_or(self.entries.len());
        self.entries.insert(rank, entry);
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
        rank + 1
    }

    pub fn rank_of(&self, score: u64) -> Option<usize> {
        for (i, entry) in self.entries.iter().enumerate() {
            if score >= entry.score {
                return Some(i + 1);
            }
        }
        if self.entries.len() < self.max_entries {
            Some(self.entries.len() + 1)
        } else {
            None
        }
    }

    pub fn is_high_score(&self, score: u64) -> bool {
        self.rank_of(score).is_some()
    }

    pub fn top_score(&self) -> Option<u64> {
        self.entries.first().map(|e| e.score)
    }

    pub fn format_leaderboard(&self) -> Vec<String> {
        self.entries.iter().enumerate().map(|(i, e)| {
            format!("{:3}. {:20} {:>12}", i + 1, e.name, e.score)
        }).collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ─── Game Events ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum GameEvent {
    PlayerSpawned { player_id: u32 },
    PlayerDied { player_id: u32, cause: String },
    PlayerLevelUp { player_id: u32, new_level: u32 },
    PlayerTookDamage { player_id: u32, amount: f32, source: String },
    PlayerHealed { player_id: u32, amount: f32 },
    PlayerGainedXp { player_id: u32, amount: u32 },
    EnemySpawned { enemy_id: u32, enemy_type: String },
    EnemyDied { enemy_id: u32, enemy_type: String, killer_id: Option<u32> },
    EnemyTookDamage { enemy_id: u32, amount: f32 },
    ItemDropped { item_id: u32, item_type: String, position: (f32, f32) },
    ItemPickedUp { item_id: u32, player_id: u32 },
    ItemEquipped { item_id: u32, player_id: u32, slot: String },
    ItemCrafted { recipe_id: String, player_id: u32 },
    GoldChanged { amount: i64, new_total: u64 },
    QuestStarted { quest_id: String },
    QuestUpdated { quest_id: String, progress: u32, required: u32 },
    QuestCompleted { quest_id: String, rewards: Vec<String> },
    LevelLoaded { level_id: String },
    LevelCompleted { level_id: String, stars: u8 },
    BossEncountered { boss_id: String },
    BossDefeated { boss_id: String, time_taken: f32 },
    SecretFound { secret_id: String },
    ChestOpened { chest_id: u32, loot: Vec<String> },
    SkillUsed { skill_id: String, player_id: u32 },
    SpellCast { spell_id: String, player_id: u32 },
    ComboAchieved { count: u32, multiplier: f32 },
    ScoreChanged { new_score: u64, delta: i64 },
    AchievementUnlocked { achievement_id: String },
    StateChanged { from: String, to: String },
    SessionStarted { session_number: u32 },
    SessionEnded { playtime: f64 },
    SettingsChanged { key: String, value: String },
    CutsceneStarted { id: String },
    CutsceneEnded { id: String },
    DialogueStarted { npc_id: String },
    DialogueEnded { npc_id: String },
    TutorialStep { step_id: String, completed: bool },
}

impl GameEvent {
    pub fn kind_name(&self) -> &str {
        match self {
            GameEvent::PlayerSpawned { .. } => "PlayerSpawned",
            GameEvent::PlayerDied { .. } => "PlayerDied",
            GameEvent::PlayerLevelUp { .. } => "PlayerLevelUp",
            GameEvent::PlayerTookDamage { .. } => "PlayerTookDamage",
            GameEvent::PlayerHealed { .. } => "PlayerHealed",
            GameEvent::PlayerGainedXp { .. } => "PlayerGainedXp",
            GameEvent::EnemySpawned { .. } => "EnemySpawned",
            GameEvent::EnemyDied { .. } => "EnemyDied",
            GameEvent::EnemyTookDamage { .. } => "EnemyTookDamage",
            GameEvent::ItemDropped { .. } => "ItemDropped",
            GameEvent::ItemPickedUp { .. } => "ItemPickedUp",
            GameEvent::ItemEquipped { .. } => "ItemEquipped",
            GameEvent::ItemCrafted { .. } => "ItemCrafted",
            GameEvent::GoldChanged { .. } => "GoldChanged",
            GameEvent::QuestStarted { .. } => "QuestStarted",
            GameEvent::QuestUpdated { .. } => "QuestUpdated",
            GameEvent::QuestCompleted { .. } => "QuestCompleted",
            GameEvent::LevelLoaded { .. } => "LevelLoaded",
            GameEvent::LevelCompleted { .. } => "LevelCompleted",
            GameEvent::BossEncountered { .. } => "BossEncountered",
            GameEvent::BossDefeated { .. } => "BossDefeated",
            GameEvent::SecretFound { .. } => "SecretFound",
            GameEvent::ChestOpened { .. } => "ChestOpened",
            GameEvent::SkillUsed { .. } => "SkillUsed",
            GameEvent::SpellCast { .. } => "SpellCast",
            GameEvent::ComboAchieved { .. } => "ComboAchieved",
            GameEvent::ScoreChanged { .. } => "ScoreChanged",
            GameEvent::AchievementUnlocked { .. } => "AchievementUnlocked",
            GameEvent::StateChanged { .. } => "StateChanged",
            GameEvent::SessionStarted { .. } => "SessionStarted",
            GameEvent::SessionEnded { .. } => "SessionEnded",
            GameEvent::SettingsChanged { .. } => "SettingsChanged",
            GameEvent::CutsceneStarted { .. } => "CutsceneStarted",
            GameEvent::CutsceneEnded { .. } => "CutsceneEnded",
            GameEvent::DialogueStarted { .. } => "DialogueStarted",
            GameEvent::DialogueEnded { .. } => "DialogueEnded",
            GameEvent::TutorialStep { .. } => "TutorialStep",
        }
    }
}

// ─── Game Event Bus ─────────────────────────────────────────────────────────────

type EventHandler = Box<dyn Fn(&GameEvent) + Send + Sync>;

pub struct GameEventBus {
    handlers: HashMap<String, Vec<EventHandler>>,
    queue: VecDeque<GameEvent>,
    history: Vec<GameEvent>,
    history_limit: usize,
}

impl GameEventBus {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            queue: VecDeque::new(),
            history: Vec::new(),
            history_limit: 100,
        }
    }

    pub fn with_history_limit(mut self, limit: usize) -> Self {
        self.history_limit = limit;
        self
    }

    pub fn subscribe(&mut self, kind: impl Into<String>, handler: EventHandler) {
        self.handlers.entry(kind.into()).or_default().push(handler);
    }

    pub fn subscribe_all(&mut self, handler: EventHandler) {
        self.handlers.entry("*".to_string()).or_default().push(handler);
    }

    pub fn publish(&mut self, event: GameEvent) {
        self.queue.push_back(event);
    }

    pub fn publish_immediate(&mut self, event: GameEvent) {
        let kind = event.kind_name().to_string();
        if let Some(handlers) = self.handlers.get(&kind) {
            for handler in handlers {
                handler(&event);
            }
        }
        if let Some(all_handlers) = self.handlers.get("*") {
            for handler in all_handlers {
                handler(&event);
            }
        }
        if self.history.len() >= self.history_limit {
            self.history.remove(0);
        }
        self.history.push(event);
    }

    pub fn flush(&mut self) {
        while let Some(event) = self.queue.pop_front() {
            self.publish_immediate(event);
        }
    }

    pub fn clear_handlers(&mut self, kind: &str) {
        self.handlers.remove(kind);
    }

    pub fn clear_all_handlers(&mut self) {
        self.handlers.clear();
    }

    pub fn history(&self) -> &[GameEvent] {
        &self.history
    }

    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }
}

impl Default for GameEventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Game Manager ───────────────────────────────────────────────────────────────

pub struct GameManager {
    pub state: GameState,
    pub config: GameConfig,
    pub timer: GameTimer,
    pub session_stats: SessionStats,
    pub score: Score,
    pub combo: ComboTracker,
    pub high_scores: HighScoreTable,
    pub event_bus: GameEventBus,
    pub transition: Option<GameTransition>,
    pending_state: Option<GameState>,
}

impl GameManager {
    pub fn new(config: GameConfig) -> Self {
        Self {
            state: GameState::MainMenu,
            config,
            timer: GameTimer::new(),
            session_stats: SessionStats::new(),
            score: Score::new(0),
            combo: ComboTracker::new(),
            high_scores: HighScoreTable::new(10),
            event_bus: GameEventBus::new(),
            transition: None,
            pending_state: None,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.timer.tick(dt as f64);
        self.combo.tick(dt);
        self.event_bus.flush();

        if let Some(ref mut t) = self.transition {
            t.tick(dt);
            if t.complete {
                if let Some(pending) = self.pending_state.take() {
                    let old_name = self.state.name().to_string();
                    let new_name = pending.name().to_string();
                    self.state = pending;
                    self.event_bus.publish(GameEvent::StateChanged {
                        from: old_name,
                        to: new_name,
                    });
                }
                self.transition = None;
            }
        }

        if let GameState::Playing = self.state {
            self.session_stats.playtime_secs += dt as f64;
        }
    }

    pub fn transition_to(&mut self, new_state: GameState, kind: TransitionKind, duration: f32) {
        let from = self.state.name().to_string();
        let to = new_state.name().to_string();
        self.transition = Some(GameTransition::new(kind, duration, from, to));
        self.pending_state = Some(new_state);
    }

    pub fn set_state(&mut self, new_state: GameState) {
        let old_name = self.state.name().to_string();
        let new_name = new_state.name().to_string();
        self.state = new_state;
        self.event_bus.publish(GameEvent::StateChanged {
            from: old_name,
            to: new_name,
        });
    }

    pub fn start_game(&mut self) {
        self.session_stats = SessionStats::new();
        self.score = Score::new(0);
        self.combo = ComboTracker::new();
        self.timer.start_session();
        self.event_bus.publish(GameEvent::SessionStarted {
            session_number: self.timer.session_count,
        });
        self.transition_to(GameState::Playing, TransitionKind::Fade, 0.5);
    }

    pub fn pause(&mut self) {
        if self.state.can_pause() {
            self.timer.pause();
            self.set_state(GameState::Paused);
        }
    }

    pub fn resume(&mut self) {
        if matches!(self.state, GameState::Paused) {
            self.timer.resume();
            self.set_state(GameState::Playing);
        }
    }

    pub fn game_over(&mut self, cause: impl Into<String>) {
        let data = GameOverData::new(
            self.score.total,
            cause,
            self.timer.elapsed,
            self.session_stats.enemies_killed,
            self.session_stats.max_level_reached,
        );
        let playtime = self.timer.elapsed;
        self.event_bus.publish(GameEvent::SessionEnded { playtime });
        self.high_scores.add(ScoreEntry::new("Player", self.score.total));
        self.transition_to(GameState::GameOver(data), TransitionKind::Fade, 1.0);
    }

    pub fn add_score(&mut self, base: u64) {
        let with_combo = self.combo.apply_to_score(base);
        let delta = with_combo as i64;
        self.score.add_base(with_combo);
        self.event_bus.publish(GameEvent::ScoreChanged {
            new_score: self.score.total,
            delta,
        });
    }

    pub fn record_hit(&mut self) -> f32 {
        let mult = self.combo.hit();
        let count = self.combo.count;
        if count > 1 {
            self.event_bus.publish(GameEvent::ComboAchieved {
                count,
                multiplier: mult,
            });
        }
        mult
    }

    pub fn is_in_transition(&self) -> bool {
        self.transition.is_some()
    }

    pub fn transition_progress(&self) -> f32 {
        self.transition.as_ref().map(|t| t.progress()).unwrap_or(1.0)
    }

    pub fn current_state_name(&self) -> &str {
        self.state.name()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_progress_fraction() {
        let mut lp = LoadProgress::new("Textures", 4);
        assert_eq!(lp.fraction(), 0.0);
        lp.current = 2;
        lp.sub_progress = 0.5;
        assert!((lp.fraction() - 0.625).abs() < 1e-5);
        lp.current = 4;
        assert!(lp.is_complete());
    }

    #[test]
    fn test_game_state_names() {
        assert_eq!(GameState::MainMenu.name(), "MainMenu");
        assert_eq!(GameState::Playing.name(), "Playing");
        assert_eq!(GameState::Paused.name(), "Paused");
        assert!(GameState::Playing.is_playing());
        assert!(GameState::Playing.can_pause());
        assert!(!GameState::Paused.can_pause());
    }

    #[test]
    fn test_transition_progress() {
        let mut t = GameTransition::new(TransitionKind::Fade, 1.0, "MainMenu", "Playing");
        assert_eq!(t.progress(), 0.0);
        t.tick(0.5);
        assert!((t.progress() - 0.5).abs() < 1e-5);
        t.tick(0.5);
        assert!(t.complete);
    }

    #[test]
    fn test_difficulty_params() {
        let p = DifficultyPreset::Hard.default_params();
        assert!(p.damage_scale > 1.0);
        assert!(p.enemy_health_scale > 1.0);
        let easy = DifficultyPreset::Easy.default_params();
        assert!(easy.damage_scale < 1.0);
    }

    #[test]
    fn test_game_timer() {
        let mut timer = GameTimer::new();
        timer.tick(1.5);
        assert!((timer.elapsed - 1.5).abs() < 1e-9);
        timer.pause();
        timer.tick(1.0);
        assert!((timer.elapsed - 1.5).abs() < 1e-9);
        assert!((timer.paused_elapsed - 1.0).abs() < 1e-9);
        timer.resume();
        timer.tick(0.5);
        assert!((timer.elapsed - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_combo_tracker() {
        let mut combo = ComboTracker::new();
        let m1 = combo.hit();
        assert!(m1 >= 1.0);
        let m2 = combo.hit();
        assert!(m2 >= m1);
        combo.tick(5.0); // beyond window
        assert!(!combo.is_active());
        assert_eq!(combo.count, 0);
    }

    #[test]
    fn test_high_score_table() {
        let mut table = HighScoreTable::new(3);
        table.add(ScoreEntry::new("Alice", 1000));
        table.add(ScoreEntry::new("Bob", 5000));
        table.add(ScoreEntry::new("Carol", 3000));
        table.add(ScoreEntry::new("Dave", 500)); // should not displace anyone
        assert_eq!(table.entries.len(), 3);
        assert_eq!(table.entries[0].score, 5000);
        assert_eq!(table.entries[1].score, 3000);
    }

    #[test]
    fn test_score_grades() {
        let mut score = Score::new(0);
        assert_eq!(score.grade(), 'F');
        score.add_base(50000);
        assert_eq!(score.grade(), 'S');
        score.add_base(50001);
        assert_eq!(score.grade(), 'X');
    }

    #[test]
    fn test_event_bus_publish_immediate() {
        use std::sync::{Arc, Mutex};
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let mut bus = GameEventBus::new();
        bus.subscribe("ScoreChanged", Box::new(move |e| {
            received_clone.lock().unwrap().push(e.kind_name().to_string());
        }));
        bus.publish_immediate(GameEvent::ScoreChanged { new_score: 100, delta: 100 });
        assert_eq!(received.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_session_stats() {
        let mut stats = SessionStats::new();
        stats.record_kill(false);
        stats.record_kill(true);
        assert_eq!(stats.enemies_killed, 2);
        assert_eq!(stats.boss_kills, 1);
        stats.record_damage_dealt(150.0, true);
        assert_eq!(stats.critical_hits, 1);
        assert!((stats.damage_dealt - 150.0).abs() < 1e-9);
    }

    #[test]
    fn test_game_manager_flow() {
        let config = GameConfig::default();
        let mut manager = GameManager::new(config);
        assert!(matches!(manager.state, GameState::MainMenu));
        // start_game triggers transition, so state is still MainMenu until tick
        manager.start_game();
        manager.tick(1.0); // complete the 0.5s transition
        assert!(matches!(manager.state, GameState::Playing));
        manager.pause();
        assert!(matches!(manager.state, GameState::Paused));
        manager.resume();
        assert!(matches!(manager.state, GameState::Playing));
    }

    #[test]
    fn test_game_config_volume() {
        let mut cfg = GameConfig::default();
        cfg.master_volume = 0.5;
        cfg.music_volume = 0.8;
        let vol = cfg.effective_volume(VolumeChannel::Music);
        assert!((vol - 0.4).abs() < 1e-5);
    }
}
