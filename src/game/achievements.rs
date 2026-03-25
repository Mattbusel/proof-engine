//! Achievement, progression, daily challenge, and mastery systems.
//!
//! Complete implementation of achievement tracking, skill progression trees,
//! daily/weekly challenges, and mastery level bonuses.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use super::SessionStats;

// ─── Achievement Category ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AchievementCategory {
    Combat,
    Exploration,
    Progression,
    Collection,
    Challenge,
    Social,
    Hidden,
}

impl AchievementCategory {
    pub fn name(&self) -> &str {
        match self {
            AchievementCategory::Combat => "Combat",
            AchievementCategory::Exploration => "Exploration",
            AchievementCategory::Progression => "Progression",
            AchievementCategory::Collection => "Collection",
            AchievementCategory::Challenge => "Challenge",
            AchievementCategory::Social => "Social",
            AchievementCategory::Hidden => "Hidden",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            AchievementCategory::Combat => '⚔',
            AchievementCategory::Exploration => '🗺',
            AchievementCategory::Progression => '⬆',
            AchievementCategory::Collection => '📦',
            AchievementCategory::Challenge => '⚡',
            AchievementCategory::Social => '👥',
            AchievementCategory::Hidden => '?',
        }
    }

    pub fn all() -> &'static [AchievementCategory] {
        &[
            AchievementCategory::Combat,
            AchievementCategory::Exploration,
            AchievementCategory::Progression,
            AchievementCategory::Collection,
            AchievementCategory::Challenge,
            AchievementCategory::Social,
            AchievementCategory::Hidden,
        ]
    }
}

// ─── Achievement Condition ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum AchievementCondition {
    KillCount { enemy_type: String, count: u32 },
    TotalKills(u32),
    ReachLevel(u32),
    CompleteQuests(u32),
    CompleteAllQuests,
    CollectItems(u32),
    CollectRareItem,
    MaxInventory,
    DealDamage(f64),
    TakeDamage(f64),
    DealCritDamage(u32),
    SpendGold(u64),
    EarnGold(u64),
    HaveGold(u64),
    PlayTime(f64),
    SurviveMinutes(f32),
    Die(u32),
    VisitLocations(u32),
    DiscoverSecrets(u32),
    OpenChests(u32),
    UsedSkills(u32),
    CastSpells(u32),
    CraftItems(u32),
    ScoreThreshold(u64),
    ComboCount(u32),
    PerfectClear,
    WinStreak(u32),
    WinWithoutDamage,
    WinAtMinHealth(f32),
    BossKills(u32),
    KillBossUnderTime { boss_id: String, seconds: f32 },
    ReachComboMultiplier(f32),
    CollectAllSecretsInLevel,
    CompleteWithClass { class_name: String },
    Custom(String),
}

impl AchievementCondition {
    pub fn description(&self) -> String {
        match self {
            AchievementCondition::KillCount { enemy_type, count } =>
                format!("Kill {} {} enemies", count, enemy_type),
            AchievementCondition::TotalKills(n) =>
                format!("Kill {} enemies total", n),
            AchievementCondition::ReachLevel(n) =>
                format!("Reach level {}", n),
            AchievementCondition::CompleteQuests(n) =>
                format!("Complete {} quests", n),
            AchievementCondition::CompleteAllQuests =>
                "Complete all quests".to_string(),
            AchievementCondition::CollectItems(n) =>
                format!("Collect {} items", n),
            AchievementCondition::CollectRareItem =>
                "Find a rare or better item".to_string(),
            AchievementCondition::MaxInventory =>
                "Fill your inventory completely".to_string(),
            AchievementCondition::DealDamage(n) =>
                format!("Deal {:.0} total damage", n),
            AchievementCondition::TakeDamage(n) =>
                format!("Take {:.0} total damage", n),
            AchievementCondition::DealCritDamage(n) =>
                format!("Land {} critical hits", n),
            AchievementCondition::SpendGold(n) =>
                format!("Spend {} gold", n),
            AchievementCondition::EarnGold(n) =>
                format!("Earn {} gold", n),
            AchievementCondition::HaveGold(n) =>
                format!("Have {} gold at once", n),
            AchievementCondition::PlayTime(secs) =>
                format!("Play for {:.0} minutes", secs / 60.0),
            AchievementCondition::SurviveMinutes(mins) =>
                format!("Survive for {} minutes", mins),
            AchievementCondition::Die(n) =>
                format!("Die {} times", n),
            AchievementCondition::VisitLocations(n) =>
                format!("Visit {} locations", n),
            AchievementCondition::DiscoverSecrets(n) =>
                format!("Discover {} secrets", n),
            AchievementCondition::OpenChests(n) =>
                format!("Open {} chests", n),
            AchievementCondition::UsedSkills(n) =>
                format!("Use skills {} times", n),
            AchievementCondition::CastSpells(n) =>
                format!("Cast {} spells", n),
            AchievementCondition::CraftItems(n) =>
                format!("Craft {} items", n),
            AchievementCondition::ScoreThreshold(n) =>
                format!("Reach a score of {}", n),
            AchievementCondition::ComboCount(n) =>
                format!("Achieve a {} hit combo", n),
            AchievementCondition::PerfectClear =>
                "Clear a level without taking damage".to_string(),
            AchievementCondition::WinStreak(n) =>
                format!("Win {} games in a row", n),
            AchievementCondition::WinWithoutDamage =>
                "Win a game without taking any damage".to_string(),
            AchievementCondition::WinAtMinHealth(pct) =>
                format!("Win with less than {:.0}% health remaining", pct * 100.0),
            AchievementCondition::BossKills(n) =>
                format!("Defeat {} bosses", n),
            AchievementCondition::KillBossUnderTime { boss_id, seconds } =>
                format!("Defeat {} in under {:.0}s", boss_id, seconds),
            AchievementCondition::ReachComboMultiplier(m) =>
                format!("Reach a {:.1}x combo multiplier", m),
            AchievementCondition::CollectAllSecretsInLevel =>
                "Find all secrets in a single level".to_string(),
            AchievementCondition::CompleteWithClass { class_name } =>
                format!("Complete the game as a {}", class_name),
            AchievementCondition::Custom(s) => s.clone(),
        }
    }

    pub fn check(&self, stats: &SessionStats) -> bool {
        match self {
            AchievementCondition::TotalKills(n) => stats.enemies_killed >= *n,
            AchievementCondition::DealDamage(n) => stats.damage_dealt >= *n,
            AchievementCondition::TakeDamage(n) => stats.damage_taken >= *n,
            AchievementCondition::DealCritDamage(n) => stats.critical_hits >= *n,
            AchievementCondition::EarnGold(n) => stats.gold_earned >= *n,
            AchievementCondition::SpendGold(n) => stats.gold_spent >= *n,
            AchievementCondition::PlayTime(secs) => stats.playtime_secs >= *secs,
            AchievementCondition::Die(n) => stats.deaths >= *n,
            AchievementCondition::DiscoverSecrets(n) => stats.secrets_found >= *n,
            AchievementCondition::OpenChests(n) => stats.chests_opened >= *n,
            AchievementCondition::UsedSkills(n) => stats.skills_used >= *n,
            AchievementCondition::CastSpells(n) => stats.spells_cast >= *n,
            AchievementCondition::CraftItems(n) => stats.items_crafted >= *n,
            AchievementCondition::CompleteQuests(n) => stats.quests_completed >= *n,
            AchievementCondition::ScoreThreshold(n) => stats.highest_score >= *n,
            AchievementCondition::ComboCount(n) => stats.highest_combo >= *n,
            AchievementCondition::ReachLevel(n) => stats.max_level_reached >= *n,
            AchievementCondition::CollectItems(n) => stats.items_collected >= *n,
            AchievementCondition::BossKills(n) => stats.boss_kills >= *n,
            _ => false, // Conditions requiring external context return false here
        }
    }
}

// ─── Achievement ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub points: u32,
    pub icon_char: char,
    pub secret: bool,
    pub category: AchievementCategory,
    pub condition: AchievementCondition,
    pub unlocked: bool,
    pub unlock_date: Option<u64>,
    pub progress: i64,
    pub progress_max: i64,
}

impl Achievement {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        points: u32,
        icon_char: char,
        category: AchievementCategory,
        condition: AchievementCondition,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            points,
            icon_char,
            secret: false,
            category,
            condition,
            unlocked: false,
            unlock_date: None,
            progress: 0,
            progress_max: 1,
        }
    }

    pub fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    pub fn with_progress_max(mut self, max: i64) -> Self {
        self.progress_max = max;
        self
    }

    pub fn display_name(&self) -> &str {
        if self.secret && !self.unlocked {
            "???"
        } else {
            &self.name
        }
    }

    pub fn display_description(&self) -> &str {
        if self.secret && !self.unlocked {
            "This achievement is secret."
        } else {
            &self.description
        }
    }

    pub fn progress_fraction(&self) -> f32 {
        if self.progress_max <= 0 {
            return if self.unlocked { 1.0 } else { 0.0 };
        }
        (self.progress as f32 / self.progress_max as f32).clamp(0.0, 1.0)
    }

    pub fn unlock_now(&mut self) {
        if !self.unlocked {
            self.unlocked = true;
            self.progress = self.progress_max;
            self.unlock_date = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            );
        }
    }
}

// ─── Built-in Achievement List ───────────────────────────────────────────────────

pub fn build_default_achievements() -> Vec<Achievement> {
    vec![
        // Combat
        Achievement::new("first_blood", "First Blood", "Kill your first enemy.", 10, '⚔', AchievementCategory::Combat,
            AchievementCondition::TotalKills(1)).with_progress_max(1),
        Achievement::new("warrior", "Warrior", "Kill 100 enemies.", 25, '⚔', AchievementCategory::Combat,
            AchievementCondition::TotalKills(100)).with_progress_max(100),
        Achievement::new("slayer", "Slayer", "Kill 500 enemies.", 50, '⚔', AchievementCategory::Combat,
            AchievementCondition::TotalKills(500)).with_progress_max(500),
        Achievement::new("legend", "Legend", "Kill 2000 enemies.", 100, '⚔', AchievementCategory::Combat,
            AchievementCondition::TotalKills(2000)).with_progress_max(2000),
        Achievement::new("crit_expert", "Critical Expert", "Land 100 critical hits.", 30, '✦', AchievementCategory::Combat,
            AchievementCondition::DealCritDamage(100)).with_progress_max(100),
        Achievement::new("damage_dealer", "Damage Dealer", "Deal 10,000 total damage.", 40, '💥', AchievementCategory::Combat,
            AchievementCondition::DealDamage(10000.0)).with_progress_max(10000),
        Achievement::new("boss_slayer", "Boss Slayer", "Defeat 10 bosses.", 60, '👑', AchievementCategory::Combat,
            AchievementCondition::BossKills(10)).with_progress_max(10),
        Achievement::new("untouchable", "Untouchable", "Win a game without taking damage.", 100, '🛡', AchievementCategory::Challenge,
            AchievementCondition::WinWithoutDamage).secret(),
        Achievement::new("combo_beginner", "Combo Beginner", "Achieve a 10-hit combo.", 15, '🔥', AchievementCategory::Combat,
            AchievementCondition::ComboCount(10)).with_progress_max(10),
        Achievement::new("combo_master", "Combo Master", "Achieve a 50-hit combo.", 50, '🔥', AchievementCategory::Combat,
            AchievementCondition::ComboCount(50)).with_progress_max(50),

        // Exploration
        Achievement::new("explorer", "Explorer", "Visit 10 locations.", 20, '🗺', AchievementCategory::Exploration,
            AchievementCondition::VisitLocations(10)).with_progress_max(10),
        Achievement::new("cartographer", "Cartographer", "Visit 50 locations.", 50, '🗺', AchievementCategory::Exploration,
            AchievementCondition::VisitLocations(50)).with_progress_max(50),
        Achievement::new("secret_finder", "Secret Finder", "Discover 5 secrets.", 30, '🔍', AchievementCategory::Exploration,
            AchievementCondition::DiscoverSecrets(5)).with_progress_max(5),
        Achievement::new("treasure_hunter", "Treasure Hunter", "Open 20 chests.", 25, '📦', AchievementCategory::Exploration,
            AchievementCondition::OpenChests(20)).with_progress_max(20),

        // Progression
        Achievement::new("level_10", "Rising Star", "Reach level 10.", 20, '⬆', AchievementCategory::Progression,
            AchievementCondition::ReachLevel(10)).with_progress_max(10),
        Achievement::new("level_25", "Veteran", "Reach level 25.", 40, '⬆', AchievementCategory::Progression,
            AchievementCondition::ReachLevel(25)).with_progress_max(25),
        Achievement::new("level_50", "Master", "Reach level 50.", 80, '⬆', AchievementCategory::Progression,
            AchievementCondition::ReachLevel(50)).with_progress_max(50),
        Achievement::new("quester", "Quester", "Complete 10 quests.", 25, '📜', AchievementCategory::Progression,
            AchievementCondition::CompleteQuests(10)).with_progress_max(10),
        Achievement::new("craftsman", "Craftsman", "Craft 20 items.", 30, '🔨', AchievementCategory::Progression,
            AchievementCondition::CraftItems(20)).with_progress_max(20),
        Achievement::new("skilled", "Skilled", "Use skills 100 times.", 20, '✨', AchievementCategory::Progression,
            AchievementCondition::UsedSkills(100)).with_progress_max(100),

        // Collection
        Achievement::new("hoarder", "Hoarder", "Collect 50 items.", 20, '🎒', AchievementCategory::Collection,
            AchievementCondition::CollectItems(50)).with_progress_max(50),
        Achievement::new("wealthy", "Wealthy", "Earn 10,000 gold.", 30, '💰', AchievementCategory::Collection,
            AchievementCondition::EarnGold(10000)).with_progress_max(10000),
        Achievement::new("big_spender", "Big Spender", "Spend 5,000 gold.", 25, '💸', AchievementCategory::Collection,
            AchievementCondition::SpendGold(5000)).with_progress_max(5000),

        // Challenge
        Achievement::new("score_10k", "High Scorer", "Reach a score of 10,000.", 30, '🏆', AchievementCategory::Challenge,
            AchievementCondition::ScoreThreshold(10000)).with_progress_max(10000),
        Achievement::new("score_100k", "Champion", "Reach a score of 100,000.", 75, '🏆', AchievementCategory::Challenge,
            AchievementCondition::ScoreThreshold(100000)).with_progress_max(100000),
        Achievement::new("perfectionist", "Perfectionist", "Clear a level without taking damage.", 80, '⭐', AchievementCategory::Challenge,
            AchievementCondition::PerfectClear).secret(),
        Achievement::new("win_streak_5", "On a Roll", "Win 5 games in a row.", 50, '🔥', AchievementCategory::Challenge,
            AchievementCondition::WinStreak(5)).with_progress_max(5),
        Achievement::new("survivor", "Survivor", "Take 1,000 damage without dying.", 35, '❤', AchievementCategory::Challenge,
            AchievementCondition::TakeDamage(1000.0)).with_progress_max(1000),

        // Time
        Achievement::new("dedicated", "Dedicated", "Play for 1 hour total.", 30, '⏰', AchievementCategory::Progression,
            AchievementCondition::PlayTime(3600.0)).with_progress_max(3600),
        Achievement::new("addicted", "Addicted", "Play for 10 hours total.", 60, '⏰', AchievementCategory::Progression,
            AchievementCondition::PlayTime(36000.0)).with_progress_max(36000),

        // Hidden/Special
        Achievement::new("die_100", "Persistent", "Die 100 times. Keep trying!", 50, '💀', AchievementCategory::Hidden,
            AchievementCondition::Die(100)).secret().with_progress_max(100),
    ]
}

// ─── Achievement Notification ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AchievementNotification {
    pub achievement: Achievement,
    pub state: NotificationState,
    pub timer: f32,
    pub slide_x: f32,
    pub target_x: f32,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotificationState {
    SlidingIn,
    Holding,
    SlidingOut,
    Done,
}

impl AchievementNotification {
    pub fn new(achievement: Achievement) -> Self {
        Self {
            achievement,
            state: NotificationState::SlidingIn,
            timer: 0.0,
            slide_x: -40.0,
            target_x: 5.0,
            alpha: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        match self.state {
            NotificationState::SlidingIn => {
                self.timer += dt;
                let t = (self.timer / 0.4).min(1.0);
                let ease = 1.0 - (1.0 - t).powi(3);
                self.slide_x = self.slide_x + (self.target_x - self.slide_x) * ease;
                self.alpha = ease;
                if self.timer >= 0.4 {
                    self.state = NotificationState::Holding;
                    self.timer = 0.0;
                    self.slide_x = self.target_x;
                    self.alpha = 1.0;
                }
            }
            NotificationState::Holding => {
                self.timer += dt;
                if self.timer >= 4.0 {
                    self.state = NotificationState::SlidingOut;
                    self.timer = 0.0;
                }
            }
            NotificationState::SlidingOut => {
                self.timer += dt;
                let t = (self.timer / 0.4).min(1.0);
                let ease = t.powi(3);
                self.slide_x = self.target_x + ease * (-self.target_x - 45.0);
                self.alpha = 1.0 - ease;
                if self.timer >= 0.4 {
                    self.state = NotificationState::Done;
                }
            }
            NotificationState::Done => {}
        }
    }

    pub fn is_done(&self) -> bool {
        self.state == NotificationState::Done
    }
}

// ─── Achievement Manager ─────────────────────────────────────────────────────────

pub struct AchievementManager {
    pub achievements: Vec<Achievement>,
    pub notify_queue: VecDeque<Achievement>,
    pub active_notifications: Vec<AchievementNotification>,
    win_streak: u32,
    custom_progress: HashMap<String, i64>,
    enemy_kill_counts: HashMap<String, u32>,
    have_gold: u64,
}

impl AchievementManager {
    pub fn new() -> Self {
        Self {
            achievements: build_default_achievements(),
            notify_queue: VecDeque::new(),
            active_notifications: Vec::new(),
            win_streak: 0,
            custom_progress: HashMap::new(),
            enemy_kill_counts: HashMap::new(),
            have_gold: 0,
        }
    }

    pub fn with_achievements(achievements: Vec<Achievement>) -> Self {
        let mut m = Self::new();
        m.achievements = achievements;
        m
    }

    pub fn check_all(&mut self, stats: &SessionStats) {
        let ids: Vec<String> = self.achievements.iter()
            .filter(|a| !a.unlocked)
            .map(|a| a.id.clone())
            .collect();
        for id in ids {
            if let Some(ach) = self.achievements.iter().find(|a| a.id == id) {
                if ach.condition.check(stats) {
                    let ach = self.achievements.iter_mut().find(|a| a.id == id).unwrap();
                    ach.unlock_now();
                    let unlocked = ach.clone();
                    self.notify_queue.push_back(unlocked);
                }
            }
        }
    }

    pub fn unlock(&mut self, id: &str) {
        if let Some(ach) = self.achievements.iter_mut().find(|a| a.id == id) {
            if !ach.unlocked {
                ach.unlock_now();
                let unlocked = ach.clone();
                self.notify_queue.push_back(unlocked);
            }
        }
    }

    pub fn progress(&mut self, id: &str, delta: i64) {
        if let Some(ach) = self.achievements.iter_mut().find(|a| a.id == id) {
            if !ach.unlocked {
                ach.progress = (ach.progress + delta).min(ach.progress_max);
                if ach.progress >= ach.progress_max {
                    ach.unlock_now();
                    let unlocked = ach.clone();
                    self.notify_queue.push_back(unlocked);
                }
            }
        }
    }

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.achievements.iter().find(|a| a.id == id).map(|a| a.unlocked).unwrap_or(false)
    }

    pub fn completion_percent(&self) -> f32 {
        let total = self.achievements.len();
        if total == 0 { return 100.0; }
        let unlocked = self.achievements.iter().filter(|a| a.unlocked).count();
        unlocked as f32 / total as f32 * 100.0
    }

    pub fn points(&self) -> u32 {
        self.achievements.iter().filter(|a| a.unlocked).map(|a| a.points).sum()
    }

    pub fn total_possible_points(&self) -> u32 {
        self.achievements.iter().map(|a| a.points).sum()
    }

    pub fn update(&mut self, dt: f32) {
        // Drain notify queue into active notifications (max 3 at once)
        while self.active_notifications.len() < 3 {
            if let Some(ach) = self.notify_queue.pop_front() {
                self.active_notifications.push(AchievementNotification::new(ach));
            } else {
                break;
            }
        }
        // Update active notifications
        for n in &mut self.active_notifications {
            n.update(dt);
        }
        self.active_notifications.retain(|n| !n.is_done());
    }

    pub fn by_category(&self, category: AchievementCategory) -> Vec<&Achievement> {
        self.achievements.iter().filter(|a| a.category == category).collect()
    }

    pub fn unlocked_achievements(&self) -> Vec<&Achievement> {
        self.achievements.iter().filter(|a| a.unlocked).collect()
    }

    pub fn locked_achievements(&self) -> Vec<&Achievement> {
        self.achievements.iter().filter(|a| !a.unlocked && !a.secret).collect()
    }

    pub fn record_win(&mut self) {
        self.win_streak += 1;
        self.progress("win_streak_5", 1);
    }

    pub fn record_loss(&mut self) {
        self.win_streak = 0;
    }

    pub fn record_enemy_kill(&mut self, enemy_type: &str) {
        *self.enemy_kill_counts.entry(enemy_type.to_string()).or_insert(0) += 1;
        let count = self.enemy_kill_counts[enemy_type];
        // Check kill count achievements
        let ids: Vec<String> = self.achievements.iter()
            .filter(|a| !a.unlocked)
            .filter_map(|a| {
                if let AchievementCondition::KillCount { enemy_type: et, count: needed } = &a.condition {
                    if et == enemy_type && count >= *needed {
                        Some(a.id.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        for id in ids {
            self.unlock(&id);
        }
    }

    pub fn set_gold(&mut self, amount: u64) {
        self.have_gold = amount;
        let ids: Vec<String> = self.achievements.iter()
            .filter(|a| !a.unlocked)
            .filter_map(|a| {
                if let AchievementCondition::HaveGold(needed) = &a.condition {
                    if amount >= *needed { Some(a.id.clone()) } else { None }
                } else { None }
            })
            .collect();
        for id in ids {
            self.unlock(&id);
        }
    }

    pub fn achievement_by_id(&self, id: &str) -> Option<&Achievement> {
        self.achievements.iter().find(|a| a.id == id)
    }
}

impl Default for AchievementManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Progression Node ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProgressionNode {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cost: u32,
    pub unlocks: Vec<String>,
    pub requires: Vec<String>,
    pub icon: char,
    pub tier: u32,
}

impl ProgressionNode {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        cost: u32,
        tier: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            cost,
            unlocks: Vec::new(),
            requires: Vec::new(),
            icon: '◆',
            tier,
        }
    }

    pub fn with_requires(mut self, reqs: Vec<impl Into<String>>) -> Self {
        self.requires = reqs.into_iter().map(|r| r.into()).collect();
        self
    }

    pub fn with_unlocks(mut self, unlocks: Vec<impl Into<String>>) -> Self {
        self.unlocks = unlocks.into_iter().map(|u| u.into()).collect();
        self
    }

    pub fn with_icon(mut self, icon: char) -> Self {
        self.icon = icon;
        self
    }
}

// ─── Progression Tree ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProgressionTree {
    pub nodes: Vec<ProgressionNode>,
    pub name: String,
}

impl ProgressionTree {
    pub fn new(name: impl Into<String>) -> Self {
        Self { nodes: Vec::new(), name: name.into() }
    }

    pub fn add_node(mut self, node: ProgressionNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn node_by_id(&self, id: &str) -> Option<&ProgressionNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Topological sort for display ordering — returns node IDs in dependency order.
    pub fn topological_order(&self) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut order = Vec::new();
        for node in &self.nodes {
            self.visit_node(&node.id, &mut visited, &mut order);
        }
        order
    }

    fn visit_node(&self, id: &str, visited: &mut HashSet<String>, order: &mut Vec<String>) {
        if visited.contains(id) { return; }
        visited.insert(id.to_string());
        if let Some(node) = self.node_by_id(id) {
            for req in &node.requires {
                self.visit_node(req, visited, order);
            }
        }
        order.push(id.to_string());
    }

    pub fn tiers(&self) -> Vec<Vec<&ProgressionNode>> {
        let max_tier = self.nodes.iter().map(|n| n.tier).max().unwrap_or(0);
        (0..=max_tier).map(|t| {
            self.nodes.iter().filter(|n| n.tier == t).collect()
        }).collect()
    }
}

// ─── Progression State ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ProgressionState {
    pub unlocked: HashSet<String>,
    pub currency: u32,
    pub total_spent: u32,
}

impl ProgressionState {
    pub fn new(starting_currency: u32) -> Self {
        Self {
            unlocked: HashSet::new(),
            currency: starting_currency,
            total_spent: 0,
        }
    }

    pub fn can_unlock(&self, tree: &ProgressionTree, node_id: &str) -> bool {
        if self.unlocked.contains(node_id) { return false; }
        if let Some(node) = tree.node_by_id(node_id) {
            for req in &node.requires {
                if !self.unlocked.contains(req) { return false; }
            }
            true
        } else {
            false
        }
    }

    pub fn unlock(&mut self, tree: &ProgressionTree, node_id: &str) -> bool {
        if !self.can_unlock(tree, node_id) { return false; }
        if let Some(node) = tree.node_by_id(node_id) {
            self.currency -= node.cost;
            self.total_spent += node.cost;
            self.unlocked.insert(node_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn add_currency(&mut self, amount: u32) {
        self.currency += amount;
    }

    pub fn is_unlocked(&self, node_id: &str) -> bool {
        self.unlocked.contains(node_id)
    }

    pub fn available_to_unlock<'a>(&self, tree: &'a ProgressionTree) -> Vec<&'a ProgressionNode> {
        tree.nodes.iter().filter(|n| self.can_unlock(tree, &n.id)).collect()
    }
}

// ─── Progression Presets ─────────────────────────────────────────────────────────

pub struct ProgressionPreset;

impl ProgressionPreset {
    pub fn warrior_tree() -> ProgressionTree {
        ProgressionTree::new("Warrior")
            .add_node(ProgressionNode::new("power_strike", "Power Strike", "Increase basic attack damage by 15%.", 50, 0)
                .with_icon('⚔'))
            .add_node(ProgressionNode::new("iron_skin", "Iron Skin", "Increase armor by 20%.", 50, 0)
                .with_icon('🛡'))
            .add_node(ProgressionNode::new("battle_cry", "Battle Cry", "AOE taunt nearby enemies.", 100, 1)
                .with_requires(vec!["power_strike"]).with_icon('📢'))
            .add_node(ProgressionNode::new("shield_wall", "Shield Wall", "Reduce incoming damage by 25% for 5s.", 100, 1)
                .with_requires(vec!["iron_skin"]).with_icon('🛡'))
            .add_node(ProgressionNode::new("berserker", "Berserker", "Below 30% HP, gain +50% attack speed.", 200, 2)
                .with_requires(vec!["battle_cry", "power_strike"]).with_icon('😡'))
            .add_node(ProgressionNode::new("juggernaut", "Juggernaut", "Become unstoppable for 3 seconds.", 300, 3)
                .with_requires(vec!["shield_wall", "berserker"]).with_icon('💪'))
    }

    pub fn mage_tree() -> ProgressionTree {
        ProgressionTree::new("Mage")
            .add_node(ProgressionNode::new("arcane_bolt", "Arcane Bolt", "Unlock arcane projectile.", 50, 0)
                .with_icon('✦'))
            .add_node(ProgressionNode::new("mana_shield", "Mana Shield", "Convert 10% mana damage to health.", 50, 0)
                .with_icon('🔵'))
            .add_node(ProgressionNode::new("fireball", "Fireball", "Unlock explosive fire spell.", 100, 1)
                .with_requires(vec!["arcane_bolt"]).with_icon('🔥'))
            .add_node(ProgressionNode::new("ice_lance", "Ice Lance", "Freezing projectile.", 100, 1)
                .with_requires(vec!["arcane_bolt"]).with_icon('❄'))
            .add_node(ProgressionNode::new("meteor", "Meteor", "Call down a devastating meteor.", 200, 2)
                .with_requires(vec!["fireball"]).with_icon('☄'))
            .add_node(ProgressionNode::new("blizzard", "Blizzard", "Persistent ice storm.", 200, 2)
                .with_requires(vec!["ice_lance"]).with_icon('🌨'))
            .add_node(ProgressionNode::new("archmage", "Archmage", "Reduce all spell cooldowns by 30%.", 300, 3)
                .with_requires(vec!["meteor", "blizzard"]).with_icon('👑'))
    }

    pub fn rogue_tree() -> ProgressionTree {
        ProgressionTree::new("Rogue")
            .add_node(ProgressionNode::new("backstab", "Backstab", "+50% damage when attacking from behind.", 50, 0)
                .with_icon('🗡'))
            .add_node(ProgressionNode::new("evasion", "Evasion", "+15% dodge chance.", 50, 0)
                .with_icon('💨'))
            .add_node(ProgressionNode::new("shadow_step", "Shadow Step", "Teleport behind target.", 100, 1)
                .with_requires(vec!["evasion"]).with_icon('🌑'))
            .add_node(ProgressionNode::new("poison_blade", "Poison Blade", "Attacks apply poison.", 100, 1)
                .with_requires(vec!["backstab"]).with_icon('☠'))
            .add_node(ProgressionNode::new("vanish", "Vanish", "Become invisible for 5 seconds.", 200, 2)
                .with_requires(vec!["shadow_step"]).with_icon('👻'))
            .add_node(ProgressionNode::new("death_mark", "Death Mark", "Mark a target for triple damage.", 300, 3)
                .with_requires(vec!["vanish", "poison_blade"]).with_icon('💀'))
    }
}

// ─── Challenge Objective Type ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ObjectiveType {
    KillEnemies { enemy_type: Option<String>, count: u32 },
    DealDamage(f64),
    CollectGold(u64),
    CompleteLevels(u32),
    SurviveTime(f32),
    AchieveCombo(u32),
    CraftItems(u32),
    OpenChests(u32),
    ScorePoints(u64),
    WinWithoutDying,
    Custom(String),
}

impl ObjectiveType {
    pub fn description(&self) -> String {
        match self {
            ObjectiveType::KillEnemies { enemy_type, count } => {
                if let Some(et) = enemy_type {
                    format!("Kill {} {} enemies", count, et)
                } else {
                    format!("Kill {} enemies", count)
                }
            }
            ObjectiveType::DealDamage(n) => format!("Deal {:.0} damage", n),
            ObjectiveType::CollectGold(n) => format!("Collect {} gold", n),
            ObjectiveType::CompleteLevels(n) => format!("Complete {} levels", n),
            ObjectiveType::SurviveTime(secs) => format!("Survive for {:.0} seconds", secs),
            ObjectiveType::AchieveCombo(n) => format!("Achieve a {}-hit combo", n),
            ObjectiveType::CraftItems(n) => format!("Craft {} items", n),
            ObjectiveType::OpenChests(n) => format!("Open {} chests", n),
            ObjectiveType::ScorePoints(n) => format!("Score {} points", n),
            ObjectiveType::WinWithoutDying => "Win without dying".to_string(),
            ObjectiveType::Custom(s) => s.clone(),
        }
    }
}

// ─── Challenge Objective ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ChallengeObjective {
    pub description: String,
    pub progress: u32,
    pub required: u32,
    pub objective_type: ObjectiveType,
    pub completed: bool,
}

impl ChallengeObjective {
    pub fn new(objective_type: ObjectiveType, required: u32) -> Self {
        let description = objective_type.description();
        Self {
            description,
            progress: 0,
            required,
            objective_type,
            completed: false,
        }
    }

    pub fn advance(&mut self, amount: u32) {
        if !self.completed {
            self.progress = (self.progress + amount).min(self.required);
            if self.progress >= self.required {
                self.completed = true;
            }
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.required == 0 { return 1.0; }
        self.progress as f32 / self.required as f32
    }
}

// ─── Challenge Reward ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ChallengeReward {
    pub gold: u32,
    pub xp: u32,
    pub item_name: Option<String>,
    pub progression_currency: u32,
}

impl ChallengeReward {
    pub fn gold_xp(gold: u32, xp: u32) -> Self {
        Self { gold, xp, item_name: None, progression_currency: 0 }
    }

    pub fn description(&self) -> String {
        let mut parts = Vec::new();
        if self.gold > 0 { parts.push(format!("{} gold", self.gold)); }
        if self.xp > 0 { parts.push(format!("{} XP", self.xp)); }
        if let Some(ref item) = self.item_name { parts.push(item.clone()); }
        if self.progression_currency > 0 { parts.push(format!("{} skill points", self.progression_currency)); }
        if parts.is_empty() { "No reward".to_string() } else { parts.join(", ") }
    }
}

// ─── Challenge ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Challenge {
    pub id: String,
    pub name: String,
    pub description: String,
    pub expiry_secs: u64,
    pub objectives: Vec<ChallengeObjective>,
    pub reward: ChallengeReward,
    pub is_weekly: bool,
    pub completed: bool,
}

impl Challenge {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        expiry_secs: u64,
        objectives: Vec<ChallengeObjective>,
        reward: ChallengeReward,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            expiry_secs,
            objectives,
            reward,
            is_weekly: false,
            completed: false,
        }
    }

    pub fn weekly(mut self) -> Self {
        self.is_weekly = true;
        self
    }

    pub fn is_expired(&self, now_secs: u64) -> bool {
        now_secs >= self.expiry_secs
    }

    pub fn check_completion(&mut self) {
        if !self.completed && self.objectives.iter().all(|o| o.completed) {
            self.completed = true;
        }
    }

    pub fn progress_summary(&self) -> String {
        let done = self.objectives.iter().filter(|o| o.completed).count();
        format!("{}/{} objectives", done, self.objectives.len())
    }
}

// ─── Challenge Generator ─────────────────────────────────────────────────────────

pub struct ChallengeGenerator;

impl ChallengeGenerator {
    /// Generate daily challenges deterministically from day number + seed.
    pub fn generate_daily(day_number: u64, seed: u64) -> Vec<Challenge> {
        let mut challenges = Vec::new();
        let rng_base = Self::hash(day_number, seed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expiry = Self::next_midnight_utc(now);

        // Generate 3 daily challenges
        for i in 0..3u64 {
            let rng = Self::hash(rng_base, i);
            let challenge = Self::pick_challenge(rng, expiry, false, day_number, i);
            challenges.push(challenge);
        }
        challenges
    }

    /// Generate weekly challenges from week number + seed.
    pub fn generate_weekly(week_number: u64, seed: u64) -> Vec<Challenge> {
        let mut challenges = Vec::new();
        let rng_base = Self::hash(week_number, seed.wrapping_add(99999));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expiry = now + 7 * 86400;

        for i in 0..2u64 {
            let rng = Self::hash(rng_base, i);
            let challenge = Self::pick_challenge(rng, expiry, true, week_number, i);
            challenges.push(challenge);
        }
        challenges
    }

    fn pick_challenge(rng: u64, expiry: u64, weekly: bool, period: u64, idx: u64) -> Challenge {
        let challenge_types = ["kill", "score", "survive", "combo", "craft", "collect", "explore"];
        let ctype = challenge_types[(rng % challenge_types.len() as u64) as usize];
        let scale = if weekly { 5u32 } else { 1u32 };

        match ctype {
            "kill" => {
                let count = (20 + (rng >> 8) % 80) as u32 * scale;
                Challenge::new(
                    format!("daily_kill_{}_{}", period, idx),
                    "Elimination",
                    format!("Kill {} enemies today", count),
                    expiry,
                    vec![ChallengeObjective::new(ObjectiveType::KillEnemies { enemy_type: None, count }, count)],
                    ChallengeReward::gold_xp(100 * scale, 200 * scale),
                )
            }
            "score" => {
                let target = (1000 + (rng >> 4) % 9000) as u64 * scale as u64;
                Challenge::new(
                    format!("daily_score_{}_{}", period, idx),
                    "High Score Run",
                    format!("Score {} points in a single run", target),
                    expiry,
                    vec![ChallengeObjective::new(ObjectiveType::ScorePoints(target), target as u32)],
                    ChallengeReward::gold_xp(150 * scale, 300 * scale),
                )
            }
            "survive" => {
                let secs = (120 + (rng >> 6) % 180) as f32 * scale as f32;
                Challenge::new(
                    format!("daily_survive_{}_{}", period, idx),
                    "Endurance",
                    format!("Survive for {:.0} seconds", secs),
                    expiry,
                    vec![ChallengeObjective::new(ObjectiveType::SurviveTime(secs), secs as u32)],
                    ChallengeReward::gold_xp(120 * scale, 250 * scale),
                )
            }
            "combo" => {
                let combo = (10 + (rng >> 3) % 40) as u32 * scale;
                Challenge::new(
                    format!("daily_combo_{}_{}", period, idx),
                    "Combo Artist",
                    format!("Achieve a {}-hit combo", combo),
                    expiry,
                    vec![ChallengeObjective::new(ObjectiveType::AchieveCombo(combo), combo)],
                    ChallengeReward::gold_xp(80 * scale, 180 * scale),
                )
            }
            "craft" => {
                let count = (3 + (rng >> 2) % 7) as u32 * scale;
                Challenge::new(
                    format!("daily_craft_{}_{}", period, idx),
                    "Craftsman",
                    format!("Craft {} items today", count),
                    expiry,
                    vec![ChallengeObjective::new(ObjectiveType::CraftItems(count), count)],
                    ChallengeReward::gold_xp(90 * scale, 150 * scale),
                )
            }
            "collect" => {
                let gold = (200 + (rng >> 7) % 800) as u64 * scale as u64;
                Challenge::new(
                    format!("daily_collect_{}_{}", period, idx),
                    "Gold Rush",
                    format!("Collect {} gold today", gold),
                    expiry,
                    vec![ChallengeObjective::new(ObjectiveType::CollectGold(gold), gold as u32)],
                    ChallengeReward::gold_xp(200 * scale, 100 * scale),
                )
            }
            _ => {
                let levels = (1 + (rng >> 5) % 5) as u32 * scale;
                Challenge::new(
                    format!("daily_explore_{}_{}", period, idx),
                    "Level Clearer",
                    format!("Complete {} levels today", levels),
                    expiry,
                    vec![ChallengeObjective::new(ObjectiveType::CompleteLevels(levels), levels)],
                    ChallengeReward::gold_xp(130 * scale, 220 * scale),
                )
            }
        }
    }

    fn hash(a: u64, b: u64) -> u64 {
        let mut h = a.wrapping_add(b.wrapping_mul(6364136223846793005));
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }

    fn next_midnight_utc(now: u64) -> u64 {
        let secs_since_midnight = now % 86400;
        now - secs_since_midnight + 86400
    }

    pub fn day_number(epoch_secs: u64) -> u64 {
        epoch_secs / 86400
    }

    pub fn week_number(epoch_secs: u64) -> u64 {
        epoch_secs / (86400 * 7)
    }
}

// ─── Challenge Tracker ───────────────────────────────────────────────────────────

pub struct ChallengeTracker {
    pub active: Vec<Challenge>,
    pub completed: Vec<String>,
    pub reroll_tokens: u32,
    seed: u64,
}

impl ChallengeTracker {
    pub fn new(seed: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let day = ChallengeGenerator::day_number(now);
        let week = ChallengeGenerator::week_number(now);
        let mut active = ChallengeGenerator::generate_daily(day, seed);
        active.extend(ChallengeGenerator::generate_weekly(week, seed));
        Self { active, completed: Vec::new(), reroll_tokens: 3, seed }
    }

    pub fn refresh_if_expired(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.active.retain(|c| !c.is_expired(now));
        let day = ChallengeGenerator::day_number(now);
        let week = ChallengeGenerator::week_number(now);
        let daily_count = self.active.iter().filter(|c| !c.is_weekly).count();
        let weekly_count = self.active.iter().filter(|c| c.is_weekly).count();
        if daily_count < 3 {
            let new_daily = ChallengeGenerator::generate_daily(day, self.seed);
            for c in new_daily {
                if self.active.len() < 5 {
                    self.active.push(c);
                }
            }
        }
        if weekly_count < 2 {
            let new_weekly = ChallengeGenerator::generate_weekly(week, self.seed);
            for c in new_weekly {
                if self.active.len() < 7 {
                    self.active.push(c);
                }
            }
        }
    }

    pub fn reroll(&mut self, challenge_id: &str) -> bool {
        if self.reroll_tokens == 0 { return false; }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let day = ChallengeGenerator::day_number(now);
        if let Some(pos) = self.active.iter().position(|c| c.id == challenge_id) {
            let was_weekly = self.active[pos].is_weekly;
            self.active.remove(pos);
            self.reroll_tokens -= 1;
            let reroll_seed = self.seed.wrapping_add(now);
            if was_weekly {
                let week = ChallengeGenerator::week_number(now);
                if let Some(c) = ChallengeGenerator::generate_weekly(week, reroll_seed).into_iter().next() {
                    self.active.push(c);
                }
            } else {
                if let Some(c) = ChallengeGenerator::generate_daily(day, reroll_seed).into_iter().next() {
                    self.active.push(c);
                }
            }
            true
        } else {
            false
        }
    }

    pub fn advance_objective(&mut self, objective_kind: &str, amount: u32) {
        for challenge in &mut self.active {
            if challenge.completed { continue; }
            let kind_matches: Vec<usize> = challenge.objectives.iter().enumerate()
                .filter(|(_, o)| o.objective_type.description().to_lowercase().contains(objective_kind))
                .map(|(i, _)| i)
                .collect();
            for idx in kind_matches {
                challenge.objectives[idx].advance(amount);
            }
            challenge.check_completion();
        }
    }

    pub fn complete_challenge(&mut self, id: &str) -> Option<ChallengeReward> {
        if let Some(pos) = self.active.iter().position(|c| c.id == id && c.completed) {
            let challenge = self.active.remove(pos);
            self.completed.push(challenge.id.clone());
            Some(challenge.reward)
        } else {
            None
        }
    }

    pub fn active_daily(&self) -> Vec<&Challenge> {
        self.active.iter().filter(|c| !c.is_weekly).collect()
    }

    pub fn active_weekly(&self) -> Vec<&Challenge> {
        self.active.iter().filter(|c| c.is_weekly).collect()
    }
}

// ─── Mastery Bonus ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum MasteryBonus {
    DamageBonus(f32),
    CooldownReduction(f32),
    ResourceGain(f32),
    CritChance(f32),
    CritMultiplier(f32),
    SpeedBonus(f32),
    DefenseBonus(f32),
    HealingBonus(f32),
    XpBonus(f32),
    GoldBonus(f32),
    ComboWindow(f32),
    DamageReduction(f32),
    SkillPowerBonus(f32),
}

impl MasteryBonus {
    pub fn description(&self) -> String {
        match self {
            MasteryBonus::DamageBonus(v) => format!("+{:.0}% damage", v * 100.0),
            MasteryBonus::CooldownReduction(v) => format!("-{:.0}% cooldowns", v * 100.0),
            MasteryBonus::ResourceGain(v) => format!("+{:.0}% resource gain", v * 100.0),
            MasteryBonus::CritChance(v) => format!("+{:.0}% crit chance", v * 100.0),
            MasteryBonus::CritMultiplier(v) => format!("+{:.0}% crit damage", v * 100.0),
            MasteryBonus::SpeedBonus(v) => format!("+{:.0}% speed", v * 100.0),
            MasteryBonus::DefenseBonus(v) => format!("+{:.0}% defense", v * 100.0),
            MasteryBonus::HealingBonus(v) => format!("+{:.0}% healing", v * 100.0),
            MasteryBonus::XpBonus(v) => format!("+{:.0}% XP gain", v * 100.0),
            MasteryBonus::GoldBonus(v) => format!("+{:.0}% gold gain", v * 100.0),
            MasteryBonus::ComboWindow(v) => format!("+{:.1}s combo window", v),
            MasteryBonus::DamageReduction(v) => format!("-{:.0}% damage taken", v * 100.0),
            MasteryBonus::SkillPowerBonus(v) => format!("+{:.0}% skill power", v * 100.0),
        }
    }

    pub fn value(&self) -> f32 {
        match self {
            MasteryBonus::DamageBonus(v) | MasteryBonus::CooldownReduction(v) |
            MasteryBonus::ResourceGain(v) | MasteryBonus::CritChance(v) |
            MasteryBonus::CritMultiplier(v) | MasteryBonus::SpeedBonus(v) |
            MasteryBonus::DefenseBonus(v) | MasteryBonus::HealingBonus(v) |
            MasteryBonus::XpBonus(v) | MasteryBonus::GoldBonus(v) |
            MasteryBonus::ComboWindow(v) | MasteryBonus::DamageReduction(v) |
            MasteryBonus::SkillPowerBonus(v) => *v,
        }
    }
}

// ─── Mastery Level ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MasteryLevel {
    pub level: u32,
    pub xp: u64,
    pub xp_per_level: u64,
    pub bonuses: Vec<MasteryBonus>,
}

impl MasteryLevel {
    pub fn new(xp_per_level: u64) -> Self {
        Self { level: 0, xp: 0, xp_per_level, bonuses: Vec::new() }
    }

    pub fn add_xp(&mut self, amount: u64) -> u32 {
        self.xp += amount;
        let mut levels_gained = 0u32;
        while self.xp >= self.xp_required_for_next() {
            self.xp -= self.xp_required_for_next();
            self.level += 1;
            levels_gained += 1;
            self.apply_level_up_bonus();
        }
        levels_gained
    }

    pub fn xp_required_for_next(&self) -> u64 {
        self.xp_per_level + self.level as u64 * (self.xp_per_level / 5)
    }

    pub fn progress_fraction(&self) -> f32 {
        let needed = self.xp_required_for_next();
        if needed == 0 { return 1.0; }
        self.xp as f32 / needed as f32
    }

    fn apply_level_up_bonus(&mut self) {
        let bonus = match self.level % 5 {
            1 => MasteryBonus::DamageBonus(0.02),
            2 => MasteryBonus::CritChance(0.01),
            3 => MasteryBonus::CooldownReduction(0.02),
            4 => MasteryBonus::ResourceGain(0.03),
            0 => MasteryBonus::SkillPowerBonus(0.05),
            _ => MasteryBonus::DamageBonus(0.01),
        };
        self.bonuses.push(bonus);
    }

    pub fn total_damage_bonus(&self) -> f32 {
        self.bonuses.iter().filter_map(|b| {
            if let MasteryBonus::DamageBonus(v) = b { Some(*v) } else { None }
        }).sum()
    }

    pub fn total_cdr(&self) -> f32 {
        self.bonuses.iter().filter_map(|b| {
            if let MasteryBonus::CooldownReduction(v) = b { Some(*v) } else { None }
        }).sum()
    }
}

// ─── Mastery Book ────────────────────────────────────────────────────────────────

pub struct MasteryBook {
    masteries: HashMap<String, MasteryLevel>,
    default_xp_per_level: u64,
}

impl MasteryBook {
    pub fn new(default_xp_per_level: u64) -> Self {
        Self {
            masteries: HashMap::new(),
            default_xp_per_level,
        }
    }

    pub fn get_or_create(&mut self, entity_type: &str) -> &mut MasteryLevel {
        let xp = self.default_xp_per_level;
        self.masteries.entry(entity_type.to_string())
            .or_insert_with(|| MasteryLevel::new(xp))
    }

    pub fn add_xp(&mut self, entity_type: &str, amount: u64) -> u32 {
        let xp = self.default_xp_per_level;
        let mastery = self.masteries.entry(entity_type.to_string())
            .or_insert_with(|| MasteryLevel::new(xp));
        mastery.add_xp(amount)
    }

    pub fn level_of(&self, entity_type: &str) -> u32 {
        self.masteries.get(entity_type).map(|m| m.level).unwrap_or(0)
    }

    pub fn get(&self, entity_type: &str) -> Option<&MasteryLevel> {
        self.masteries.get(entity_type)
    }

    pub fn all_masteries(&self) -> &HashMap<String, MasteryLevel> {
        &self.masteries
    }

    pub fn highest_mastery(&self) -> Option<(&String, &MasteryLevel)> {
        self.masteries.iter().max_by_key(|(_, m)| m.level)
    }

    pub fn total_mastery_levels(&self) -> u32 {
        self.masteries.values().map(|m| m.level).sum()
    }

    pub fn global_damage_bonus(&self) -> f32 {
        self.masteries.values().map(|m| m.total_damage_bonus()).sum::<f32>().min(2.0)
    }

    pub fn global_cdr(&self) -> f32 {
        self.masteries.values().map(|m| m.total_cdr()).sum::<f32>().min(0.5)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_achievement_condition_check() {
        let mut stats = SessionStats::new();
        stats.enemies_killed = 100;
        stats.boss_kills = 5;
        stats.critical_hits = 50;
        stats.damage_dealt = 15000.0;

        assert!(AchievementCondition::TotalKills(100).check(&stats));
        assert!(!AchievementCondition::TotalKills(101).check(&stats));
        assert!(AchievementCondition::DealCritDamage(50).check(&stats));
        assert!(AchievementCondition::DealDamage(10000.0).check(&stats));
        assert!(!AchievementCondition::WinWithoutDamage.check(&stats));
    }

    #[test]
    fn test_achievement_manager_unlock() {
        let mut mgr = AchievementManager::new();
        assert!(!mgr.is_unlocked("first_blood"));
        mgr.unlock("first_blood");
        assert!(mgr.is_unlocked("first_blood"));
        assert!(mgr.points() > 0);
    }

    #[test]
    fn test_achievement_manager_progress() {
        let mut mgr = AchievementManager::new();
        // warrior achievement requires 100 kills, progress_max = 100
        mgr.progress("warrior", 50);
        assert!(!mgr.is_unlocked("warrior"));
        mgr.progress("warrior", 50);
        assert!(mgr.is_unlocked("warrior"));
    }

    #[test]
    fn test_achievement_completion_percent() {
        let mut mgr = AchievementManager::new();
        let total = mgr.achievements.len() as f32;
        assert!((mgr.completion_percent() - 0.0).abs() < 1e-5);
        mgr.unlock("first_blood");
        let expected = 1.0 / total * 100.0;
        assert!((mgr.completion_percent() - expected).abs() < 0.5);
    }

    #[test]
    fn test_achievement_notification_lifecycle() {
        let mut mgr = AchievementManager::new();
        mgr.unlock("first_blood");
        assert!(!mgr.notify_queue.is_empty());
        mgr.update(0.0);
        assert!(!mgr.active_notifications.is_empty());
        // Simulate time until done
        for _ in 0..300 {
            mgr.update(0.016);
        }
        assert!(mgr.active_notifications.is_empty());
    }

    #[test]
    fn test_progression_tree_can_unlock() {
        let tree = ProgressionPreset::warrior_tree();
        let mut state = ProgressionState::new(100);

        assert!(state.can_unlock(&tree, "power_strike"));
        assert!(state.can_unlock(&tree, "iron_skin"));
        // battle_cry requires power_strike
        assert!(!state.can_unlock(&tree, "battle_cry"));

        state.unlock(&tree, "power_strike");
        assert!(state.is_unlocked("power_strike"));
        assert_eq!(state.currency, 50);

        assert!(state.can_unlock(&tree, "battle_cry"));
    }

    #[test]
    fn test_progression_topological_order() {
        let tree = ProgressionPreset::mage_tree();
        let order = tree.topological_order();
        // arcane_bolt should come before fireball
        let arcane_pos = order.iter().position(|n| n == "arcane_bolt").unwrap();
        let fireball_pos = order.iter().position(|n| n == "fireball").unwrap();
        assert!(arcane_pos < fireball_pos);
    }

    #[test]
    fn test_challenge_generator_deterministic() {
        let challenges1 = ChallengeGenerator::generate_daily(1000, 42);
        let challenges2 = ChallengeGenerator::generate_daily(1000, 42);
        assert_eq!(challenges1.len(), challenges2.len());
        for (c1, c2) in challenges1.iter().zip(challenges2.iter()) {
            assert_eq!(c1.id, c2.id);
            assert_eq!(c1.name, c2.name);
        }
    }

    #[test]
    fn test_challenge_objective_advance() {
        let mut obj = ChallengeObjective::new(
            ObjectiveType::KillEnemies { enemy_type: None, count: 20 },
            20,
        );
        assert!(!obj.completed);
        obj.advance(10);
        assert!(!obj.completed);
        obj.advance(10);
        assert!(obj.completed);
        assert_eq!(obj.progress, 20);
    }

    #[test]
    fn test_mastery_level_xp() {
        let mut mastery = MasteryLevel::new(100);
        assert_eq!(mastery.level, 0);
        let levels = mastery.add_xp(100);
        assert_eq!(levels, 1);
        assert_eq!(mastery.level, 1);
        assert!(!mastery.bonuses.is_empty());
    }

    #[test]
    fn test_mastery_book() {
        let mut book = MasteryBook::new(100);
        let levels = book.add_xp("goblin", 300);
        assert!(levels > 0);
        assert!(book.level_of("goblin") > 0);
        assert!(book.total_mastery_levels() > 0);
    }

    #[test]
    fn test_mastery_book_global_bonuses() {
        let mut book = MasteryBook::new(50);
        for _ in 0..20 {
            book.add_xp("goblin", 100);
        }
        for _ in 0..20 {
            book.add_xp("orc", 100);
        }
        let damage = book.global_damage_bonus();
        assert!(damage > 0.0);
        assert!(damage <= 2.0); // capped at 200%
    }

    #[test]
    fn test_achievement_notification_state() {
        let achievements = build_default_achievements();
        let ach = achievements.into_iter().next().unwrap();
        let mut notif = AchievementNotification::new(ach);
        assert_eq!(notif.state, NotificationState::SlidingIn);
        for _ in 0..30 {
            notif.update(0.02);
        }
        assert_eq!(notif.state, NotificationState::Holding);
        for _ in 0..200 {
            notif.update(0.02);
        }
        assert_eq!(notif.state, NotificationState::SlidingOut);
        for _ in 0..30 {
            notif.update(0.02);
        }
        assert_eq!(notif.state, NotificationState::Done);
        assert!(notif.is_done());
    }

    #[test]
    fn test_category_all() {
        let cats = AchievementCategory::all();
        assert!(cats.len() >= 7);
        assert!(cats.contains(&AchievementCategory::Hidden));
    }

    #[test]
    fn test_challenge_tracker_reroll() {
        let mut tracker = ChallengeTracker::new(42);
        let initial_count = tracker.active.len();
        assert!(initial_count > 0);
        let initial_tokens = tracker.reroll_tokens;
        if let Some(first_id) = tracker.active.first().map(|c| c.id.clone()) {
            let result = tracker.reroll(&first_id);
            assert!(result);
            assert_eq!(tracker.reroll_tokens, initial_tokens - 1);
        }
    }
}
