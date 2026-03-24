//! User profiles, play statistics, local leaderboards, unlockables, and
//! the prestige system.

use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
//  UserProfile
// ─────────────────────────────────────────────────────────────────────────────

/// A single user profile stored locally.
#[derive(Debug, Clone, PartialEq)]
pub struct UserProfile {
    pub id: u64,
    pub display_name: String,
    /// Seed used to procedurally generate an avatar image.
    pub avatar_seed: u64,
    pub created_at: u64,
    pub last_login: u64,
    pub play_time_seconds: u64,
    pub preferences: HashMap<String, String>,
}

impl UserProfile {
    pub fn new(id: u64, display_name: impl Into<String>, created_at: u64) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            avatar_seed: id.wrapping_mul(0x9E37_79B9_7F4A_7C15),
            created_at,
            last_login: created_at,
            play_time_seconds: 0,
            preferences: HashMap::new(),
        }
    }

    pub fn set_preference(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.preferences.insert(key.into(), value.into());
    }

    pub fn get_preference(&self, key: &str) -> Option<&str> {
        self.preferences.get(key).map(|s| s.as_str())
    }

    /// Serialise to a simple binary blob (little-endian).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.id.to_le_bytes());
        let name_bytes = self.display_name.as_bytes();
        out.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(&self.avatar_seed.to_le_bytes());
        out.extend_from_slice(&self.created_at.to_le_bytes());
        out.extend_from_slice(&self.last_login.to_le_bytes());
        out.extend_from_slice(&self.play_time_seconds.to_le_bytes());
        out.extend_from_slice(&(self.preferences.len() as u32).to_le_bytes());
        for (k, v) in &self.preferences {
            let kb = k.as_bytes();
            let vb = v.as_bytes();
            out.extend_from_slice(&(kb.len() as u32).to_le_bytes());
            out.extend_from_slice(kb);
            out.extend_from_slice(&(vb.len() as u32).to_le_bytes());
            out.extend_from_slice(vb);
        }
        out
    }

    /// Deserialise from bytes produced by `to_bytes`.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut pos = 0usize;

        macro_rules! read_u32 {
            () => {{
                if pos + 4 > data.len() { return Err("truncated u32".into()); }
                let v = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
                pos += 4;
                v
            }};
        }
        macro_rules! read_u64 {
            () => {{
                if pos + 8 > data.len() { return Err("truncated u64".into()); }
                let v = u64::from_le_bytes([
                    data[pos], data[pos+1], data[pos+2], data[pos+3],
                    data[pos+4], data[pos+5], data[pos+6], data[pos+7],
                ]);
                pos += 8;
                v
            }};
        }
        macro_rules! read_string {
            () => {{
                let len = read_u32!() as usize;
                if pos + len > data.len() { return Err("truncated string".into()); }
                let s = std::str::from_utf8(&data[pos..pos+len])
                    .map_err(|e| e.to_string())?.to_owned();
                pos += len;
                s
            }};
        }

        let id              = read_u64!();
        let display_name    = read_string!();
        let avatar_seed     = read_u64!();
        let created_at      = read_u64!();
        let last_login      = read_u64!();
        let play_time       = read_u64!();
        let pref_count      = read_u32!() as usize;

        let mut preferences = HashMap::new();
        for _ in 0..pref_count {
            let k = read_string!();
            let v = read_string!();
            preferences.insert(k, v);
        }

        Ok(Self {
            id,
            display_name,
            avatar_seed,
            created_at,
            last_login,
            play_time_seconds: play_time,
            preferences,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  ProfileManager
// ─────────────────────────────────────────────────────────────────────────────

/// Manages up to 8 local user profiles.
pub struct ProfileManager {
    profiles: Vec<UserProfile>,
    current_index: Option<usize>,
    next_id: u64,
}

impl ProfileManager {
    pub const MAX_PROFILES: usize = 8;

    pub fn new() -> Self {
        Self { profiles: Vec::new(), current_index: None, next_id: 1 }
    }

    /// Create a new profile with the given display name.
    pub fn create(&mut self, display_name: impl Into<String>, timestamp: u64) -> Result<u64, String> {
        if self.profiles.len() >= Self::MAX_PROFILES {
            return Err(format!("maximum {} profiles reached", Self::MAX_PROFILES));
        }
        let id = self.next_id;
        self.next_id += 1;
        let profile = UserProfile::new(id, display_name, timestamp);
        self.profiles.push(profile);
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
        Ok(id)
    }

    /// Delete a profile by ID.
    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        let idx = self.profiles.iter().position(|p| p.id == id)
            .ok_or_else(|| format!("profile {id} not found"))?;
        self.profiles.remove(idx);
        // Fix current index
        match self.current_index {
            Some(cur) if cur == idx => {
                self.current_index = if self.profiles.is_empty() { None } else { Some(0) };
            }
            Some(cur) if cur > idx => {
                self.current_index = Some(cur - 1);
            }
            _ => {}
        }
        Ok(())
    }

    /// Switch to a profile by ID.
    pub fn switch(&mut self, id: u64) -> Result<(), String> {
        let idx = self.profiles.iter().position(|p| p.id == id)
            .ok_or_else(|| format!("profile {id} not found"))?;
        self.current_index = Some(idx);
        Ok(())
    }

    /// Get the currently active profile.
    pub fn current(&self) -> Option<&UserProfile> {
        self.current_index.and_then(|i| self.profiles.get(i))
    }

    /// Get a mutable reference to the currently active profile.
    pub fn current_mut(&mut self) -> Option<&mut UserProfile> {
        self.current_index.and_then(|i| self.profiles.get_mut(i))
    }

    pub fn list(&self) -> Vec<&UserProfile> {
        self.profiles.iter().collect()
    }

    pub fn get_by_id(&self, id: u64) -> Option<&UserProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn get_by_id_mut(&mut self, id: u64) -> Option<&mut UserProfile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    pub fn count(&self) -> usize {
        self.profiles.len()
    }

    /// Serialise all profiles to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.profiles.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.next_id.to_le_bytes());
        let cur = self.current_index.map(|i| i as u64).unwrap_or(u64::MAX);
        out.extend_from_slice(&cur.to_le_bytes());
        for p in &self.profiles {
            let pb = p.to_bytes();
            out.extend_from_slice(&(pb.len() as u32).to_le_bytes());
            out.extend_from_slice(&pb);
        }
        out
    }

    /// Deserialise from bytes produced by `to_bytes`.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 20 {
            return Err("ProfileManager bytes too short".into());
        }
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let next_id = u64::from_le_bytes([
            data[4], data[5], data[6], data[7],
            data[8], data[9], data[10], data[11],
        ]);
        let cur_raw = u64::from_le_bytes([
            data[12], data[13], data[14], data[15],
            data[16], data[17], data[18], data[19],
        ]);
        let current_index = if cur_raw == u64::MAX { None } else { Some(cur_raw as usize) };

        let mut pos = 20usize;
        let mut profiles = Vec::with_capacity(count);
        for _ in 0..count {
            if pos + 4 > data.len() { return Err("truncated profile length".into()); }
            let len = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
            pos += 4;
            if pos + len > data.len() { return Err("truncated profile data".into()); }
            let p = UserProfile::from_bytes(&data[pos..pos+len])?;
            profiles.push(p);
            pos += len;
        }

        Ok(Self { profiles, current_index, next_id })
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  StatisticsRecord
// ─────────────────────────────────────────────────────────────────────────────

/// Lifetime play statistics for a profile.
#[derive(Debug, Clone, Default)]
pub struct StatisticsRecord {
    pub enemies_killed: HashMap<String, u32>,
    pub damage_dealt: u64,
    pub damage_received: u64,
    pub distance_traveled: f64,
    pub items_collected: u64,
    pub deaths: u32,
    pub highest_combo: u32,
    pub max_damage_hit: u64,
    pub areas_visited: Vec<String>,
}

impl StatisticsRecord {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_kill(&mut self, enemy_type: impl Into<String>) {
        *self.enemies_killed.entry(enemy_type.into()).or_insert(0) += 1;
    }

    pub fn record_damage_dealt(&mut self, amount: u64) {
        self.damage_dealt += amount;
        if amount > self.max_damage_hit {
            self.max_damage_hit = amount;
        }
    }

    pub fn record_damage_received(&mut self, amount: u64) {
        self.damage_received += amount;
    }

    pub fn record_travel(&mut self, distance: f64) {
        self.distance_traveled += distance;
    }

    pub fn collect_item(&mut self) {
        self.items_collected += 1;
    }

    pub fn record_death(&mut self) {
        self.deaths += 1;
    }

    pub fn record_combo(&mut self, combo: u32) {
        if combo > self.highest_combo {
            self.highest_combo = combo;
        }
    }

    pub fn visit_area(&mut self, area: impl Into<String>) {
        let area = area.into();
        if !self.areas_visited.contains(&area) {
            self.areas_visited.push(area);
        }
    }

    pub fn total_kills(&self) -> u32 {
        self.enemies_killed.values().sum()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Leaderboard
// ─────────────────────────────────────────────────────────────────────────────

/// A single leaderboard entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LeaderboardEntry {
    pub score: u64,
    pub player_name: String,
    pub metadata: HashMap<String, String>,
}

/// Local top-N leaderboard for a level/mode.
pub struct Leaderboard {
    pub name: String,
    entries: Vec<LeaderboardEntry>,
    max_entries: usize,
}

impl Leaderboard {
    pub fn new(name: impl Into<String>, max_entries: usize) -> Self {
        Self { name: name.into(), entries: Vec::new(), max_entries }
    }

    /// Submit a score.  The leaderboard stays sorted (highest first) and
    /// truncated to `max_entries`.
    pub fn submit(&mut self, score: u64, player_name: impl Into<String>, metadata: HashMap<String, String>) {
        self.entries.push(LeaderboardEntry { score, player_name: player_name.into(), metadata });
        self.entries.sort_by(|a, b| b.score.cmp(&a.score));
        self.entries.truncate(self.max_entries);
    }

    pub fn get_top(&self, n: usize) -> &[LeaderboardEntry] {
        let end = n.min(self.entries.len());
        &self.entries[..end]
    }

    /// 1-based rank of the given score; returns `entries.len() + 1` if not on board.
    pub fn rank_of(&self, score: u64) -> usize {
        self.entries.iter().position(|e| e.score == score)
            .map(|i| i + 1)
            .unwrap_or(self.entries.len() + 1)
    }

    /// Fraction (0.0–1.0) of entries the given score beats.
    pub fn percentile_of(&self, score: u64) -> f32 {
        if self.entries.is_empty() { return 1.0; }
        let beaten = self.entries.iter().filter(|e| e.score < score).count();
        beaten as f32 / self.entries.len() as f32
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  UnlockCondition / Unlockable / UnlockRegistry
// ─────────────────────────────────────────────────────────────────────────────

/// Condition that must be met to unlock something.
#[derive(Debug, Clone, PartialEq)]
pub enum UnlockCondition {
    StatThreshold { stat: String, threshold: u64 },
    AchievementCompleted(String),
    ItemCollected(String),
    LevelReached(u32),
    Manual,
}

/// A single unlockable item, achievement, or cosmetic.
#[derive(Debug, Clone)]
pub struct Unlockable {
    pub id: String,
    pub name: String,
    pub unlock_condition: UnlockCondition,
    pub unlocked: bool,
    /// If true, the item is not shown until unlocked.
    pub hidden_until_unlocked: bool,
}

impl Unlockable {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        condition: UnlockCondition,
        hidden: bool,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            unlock_condition: condition,
            unlocked: false,
            hidden_until_unlocked: hidden,
        }
    }

    pub fn unlock(&mut self) {
        self.unlocked = true;
    }

    pub fn is_visible(&self) -> bool {
        self.unlocked || !self.hidden_until_unlocked
    }
}

/// Registry of all unlockables.
pub struct UnlockRegistry {
    items: HashMap<String, Unlockable>,
}

impl UnlockRegistry {
    pub fn new() -> Self {
        Self { items: HashMap::new() }
    }

    pub fn register(&mut self, item: Unlockable) {
        self.items.insert(item.id.clone(), item);
    }

    pub fn unlock(&mut self, id: &str) -> bool {
        if let Some(item) = self.items.get_mut(id) {
            item.unlock();
            true
        } else {
            false
        }
    }

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.items.get(id).map_or(false, |i| i.unlocked)
    }

    /// Check conditions against current stats and auto-unlock anything that qualifies.
    pub fn check_and_unlock(&mut self, stats: &StatisticsRecord, player_level: u32) -> Vec<String> {
        let mut newly_unlocked = Vec::new();
        for item in self.items.values_mut() {
            if item.unlocked { continue; }
            let should_unlock = match &item.unlock_condition {
                UnlockCondition::StatThreshold { stat, threshold } => {
                    match stat.as_str() {
                        "kills"            => stats.total_kills() as u64 >= *threshold,
                        "deaths"           => stats.deaths as u64 >= *threshold,
                        "damage_dealt"     => stats.damage_dealt >= *threshold,
                        "items_collected"  => stats.items_collected >= *threshold,
                        _                  => false,
                    }
                }
                UnlockCondition::LevelReached(lvl) => player_level >= *lvl,
                UnlockCondition::Manual => false,
                UnlockCondition::AchievementCompleted(_) => false,
                UnlockCondition::ItemCollected(_) => false,
            };
            if should_unlock {
                item.unlocked = true;
                newly_unlocked.push(item.id.clone());
            }
        }
        newly_unlocked
    }

    pub fn visible_items(&self) -> Vec<&Unlockable> {
        self.items.values().filter(|i| i.is_visible()).collect()
    }

    pub fn all_items(&self) -> Vec<&Unlockable> {
        self.items.values().collect()
    }
}

impl Default for UnlockRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  PrestigeSystem
// ─────────────────────────────────────────────────────────────────────────────

const MIN_LEVEL_FOR_PRESTIGE: u32 = 50;
const PRESTIGE_CURRENCY_PER_LEVEL: u64 = 10;

/// Tracks prestige level, cumulative prestige currency, and bonus multipliers.
pub struct PrestigeSystem {
    pub prestige_level: u32,
    pub prestige_currency: u64,
    /// Cumulative currency earned across all prestiges.
    pub lifetime_currency: u64,
    pub current_player_level: u32,
}

impl PrestigeSystem {
    pub fn new() -> Self {
        Self {
            prestige_level: 0,
            prestige_currency: 0,
            lifetime_currency: 0,
            current_player_level: 1,
        }
    }

    /// Whether the player is eligible to prestige.
    pub fn can_prestige(&self) -> bool {
        self.current_player_level >= MIN_LEVEL_FOR_PRESTIGE
    }

    /// Perform a prestige reset.
    ///
    /// Returns the amount of prestige currency earned.
    pub fn prestige(&mut self) -> Result<u64, String> {
        if !self.can_prestige() {
            return Err(format!(
                "must reach level {} to prestige (current: {})",
                MIN_LEVEL_FOR_PRESTIGE, self.current_player_level
            ));
        }
        let earned = self.current_player_level as u64 * PRESTIGE_CURRENCY_PER_LEVEL;
        self.prestige_level += 1;
        self.prestige_currency += earned;
        self.lifetime_currency += earned;
        self.current_player_level = 1; // reset
        Ok(earned)
    }

    /// Bonus multiplier applied to experience/rewards based on prestige level.
    pub fn get_bonus_multiplier(&self) -> f32 {
        1.0 + (self.prestige_level as f32) * 0.1
    }

    /// Spend prestige currency; returns error if insufficient.
    pub fn spend_currency(&mut self, amount: u64) -> Result<(), String> {
        if self.prestige_currency < amount {
            return Err(format!(
                "insufficient prestige currency: have {}, need {}",
                self.prestige_currency, amount
            ));
        }
        self.prestige_currency -= amount;
        Ok(())
    }

    pub fn set_player_level(&mut self, level: u32) {
        self.current_player_level = level;
    }
}

impl Default for PrestigeSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── UserProfile ──────────────────────────────────────────────────────────

    #[test]
    fn test_profile_serialization_roundtrip() {
        let mut p = UserProfile::new(1, "Hero", 1000);
        p.set_preference("theme", "dark");
        let bytes = p.to_bytes();
        let restored = UserProfile::from_bytes(&bytes).unwrap();
        assert_eq!(restored.id, p.id);
        assert_eq!(restored.display_name, p.display_name);
        assert_eq!(restored.get_preference("theme"), Some("dark"));
    }

    #[test]
    fn test_profile_manager_create_and_list() {
        let mut mgr = ProfileManager::new();
        mgr.create("Alice", 100).unwrap();
        mgr.create("Bob", 200).unwrap();
        assert_eq!(mgr.count(), 2);
        let names: Vec<&str> = mgr.list().iter().map(|p| p.display_name.as_str()).collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Bob"));
    }

    #[test]
    fn test_profile_manager_max_8() {
        let mut mgr = ProfileManager::new();
        for i in 0..8 {
            mgr.create(format!("P{i}"), i as u64).unwrap();
        }
        let result = mgr.create("Extra", 999);
        assert!(result.is_err());
    }

    #[test]
    fn test_profile_manager_switch_and_delete() {
        let mut mgr = ProfileManager::new();
        let id1 = mgr.create("Alice", 1).unwrap();
        let id2 = mgr.create("Bob", 2).unwrap();
        mgr.switch(id2).unwrap();
        assert_eq!(mgr.current().unwrap().display_name, "Bob");
        mgr.delete(id2).unwrap();
        assert_eq!(mgr.count(), 1);
        let _ = mgr.switch(id1);
        assert_eq!(mgr.current().unwrap().display_name, "Alice");
    }

    #[test]
    fn test_profile_manager_serialization() {
        let mut mgr = ProfileManager::new();
        mgr.create("Alice", 1).unwrap();
        mgr.create("Bob", 2).unwrap();
        let bytes = mgr.to_bytes();
        let restored = ProfileManager::from_bytes(&bytes).unwrap();
        assert_eq!(restored.count(), 2);
    }

    // ── StatisticsRecord ─────────────────────────────────────────────────────

    #[test]
    fn test_statistics_record() {
        let mut stats = StatisticsRecord::new();
        stats.record_kill("goblin");
        stats.record_kill("goblin");
        stats.record_kill("orc");
        assert_eq!(stats.total_kills(), 3);
        stats.record_damage_dealt(500);
        stats.record_damage_dealt(1000);
        assert_eq!(stats.max_damage_hit, 1000);
        stats.record_travel(100.0);
        stats.record_travel(50.0);
        assert!((stats.distance_traveled - 150.0).abs() < 1e-6);
        stats.visit_area("Forest");
        stats.visit_area("Forest"); // dedup
        assert_eq!(stats.areas_visited.len(), 1);
    }

    // ── Leaderboard ──────────────────────────────────────────────────────────

    #[test]
    fn test_leaderboard_submit_and_rank() {
        let mut lb = Leaderboard::new("level_1", 5);
        lb.submit(1000, "Alice", HashMap::new());
        lb.submit(2000, "Bob", HashMap::new());
        lb.submit(500, "Carol", HashMap::new());
        assert_eq!(lb.get_top(3)[0].score, 2000);
        assert_eq!(lb.rank_of(2000), 1);
        assert_eq!(lb.rank_of(1000), 2);
    }

    #[test]
    fn test_leaderboard_max_entries() {
        let mut lb = Leaderboard::new("test", 3);
        for s in [100u64, 200, 300, 400, 500] {
            lb.submit(s, "p", HashMap::new());
        }
        assert_eq!(lb.len(), 3);
        assert_eq!(lb.get_top(1)[0].score, 500);
    }

    #[test]
    fn test_leaderboard_percentile() {
        let mut lb = Leaderboard::new("test", 10);
        for s in [100u64, 200, 300, 400] {
            lb.submit(s, "p", HashMap::new());
        }
        // Score of 300 beats 100 and 200 → 2/4 = 0.5
        let pct = lb.percentile_of(300);
        assert!((pct - 0.5).abs() < 1e-5, "got {pct}");
    }

    // ── UnlockRegistry ───────────────────────────────────────────────────────

    #[test]
    fn test_unlock_registry_manual() {
        let mut reg = UnlockRegistry::new();
        reg.register(Unlockable::new("item_a", "Item A", UnlockCondition::Manual, false));
        assert!(!reg.is_unlocked("item_a"));
        reg.unlock("item_a");
        assert!(reg.is_unlocked("item_a"));
    }

    #[test]
    fn test_unlock_auto_by_kills() {
        let mut reg = UnlockRegistry::new();
        reg.register(Unlockable::new(
            "killer",
            "Killer Badge",
            UnlockCondition::StatThreshold { stat: "kills".into(), threshold: 5 },
            false,
        ));
        let mut stats = StatisticsRecord::new();
        for _ in 0..5 { stats.record_kill("goblin"); }
        let newly = reg.check_and_unlock(&stats, 1);
        assert!(newly.contains(&"killer".to_string()));
        assert!(reg.is_unlocked("killer"));
    }

    // ── PrestigeSystem ───────────────────────────────────────────────────────

    #[test]
    fn test_prestige_system_cannot_prestige_early() {
        let mut ps = PrestigeSystem::new();
        ps.set_player_level(10);
        assert!(!ps.can_prestige());
        assert!(ps.prestige().is_err());
    }

    #[test]
    fn test_prestige_system_prestige() {
        let mut ps = PrestigeSystem::new();
        ps.set_player_level(50);
        assert!(ps.can_prestige());
        let earned = ps.prestige().unwrap();
        assert_eq!(earned, 50 * 10);
        assert_eq!(ps.prestige_level, 1);
        assert_eq!(ps.current_player_level, 1);
    }

    #[test]
    fn test_prestige_bonus_multiplier() {
        let mut ps = PrestigeSystem::new();
        ps.set_player_level(50);
        assert!((ps.get_bonus_multiplier() - 1.0).abs() < 1e-6);
        ps.prestige().unwrap();
        assert!((ps.get_bonus_multiplier() - 1.1).abs() < 1e-5);
    }

    #[test]
    fn test_prestige_spend_currency() {
        let mut ps = PrestigeSystem::new();
        ps.set_player_level(50);
        ps.prestige().unwrap();
        let available = ps.prestige_currency;
        assert!(ps.spend_currency(available / 2).is_ok());
        assert!(ps.spend_currency(available).is_err());
    }
}
