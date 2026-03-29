
// ============================================================
// SECTION 96: ITEM AFFINITY SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct AffinityBonus {
    pub stat: &'static str,
    pub bonus_pct: f32,
}

#[derive(Clone, Debug)]
pub struct ItemAffinity {
    pub affinity_id: u32,
    pub name: &'static str,
    pub required_items: Vec<u32>,
    pub bonuses: Vec<AffinityBonus>,
}

pub struct AffinityRegistry {
    pub affinities: Vec<ItemAffinity>,
}

impl AffinityRegistry {
    pub fn new() -> Self { Self { affinities: Vec::new() } }

    pub fn register(&mut self, affinity: ItemAffinity) {
        self.affinities.push(affinity);
    }

    pub fn check_active(&self, equipped: &[u32]) -> Vec<&ItemAffinity> {
        let equipped_set: std::collections::HashSet<u32> = equipped.iter().copied().collect();
        self.affinities.iter().filter(|a| {
            a.required_items.iter().all(|id| equipped_set.contains(id))
        }).collect()
    }

    pub fn total_bonus(&self, equipped: &[u32], stat: &str) -> f32 {
        self.check_active(equipped)
            .iter()
            .flat_map(|a| a.bonuses.iter())
            .filter(|b| b.stat == stat)
            .map(|b| b.bonus_pct)
            .sum()
    }
}

// ============================================================
// SECTION 97: ITEM SOCKET SYSTEM
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum SocketColor { Red, Blue, Green, White }

#[derive(Clone, Debug)]
pub struct Socket {
    pub color: SocketColor,
    pub gem_id: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct GemDefinition {
    pub gem_id: u32,
    pub name: &'static str,
    pub color: SocketColor,
    pub stat_bonus: f32,
    pub stat_type: &'static str,
}

pub struct SocketSystem {
    pub gems: Vec<GemDefinition>,
    pub rng: LootRng,
}

impl SocketSystem {
    pub fn new(seed: u64) -> Self {
        let mut gems = Vec::new();
        gems.push(GemDefinition { gem_id: 1, name: "Ruby", color: SocketColor::Red, stat_bonus: 15.0, stat_type: "strength" });
        gems.push(GemDefinition { gem_id: 2, name: "Sapphire", color: SocketColor::Blue, stat_bonus: 15.0, stat_type: "intelligence" });
        gems.push(GemDefinition { gem_id: 3, name: "Emerald", color: SocketColor::Green, stat_bonus: 15.0, stat_type: "agility" });
        gems.push(GemDefinition { gem_id: 4, name: "Diamond", color: SocketColor::White, stat_bonus: 10.0, stat_type: "all_stats" });
        gems.push(GemDefinition { gem_id: 5, name: "Onyx", color: SocketColor::Red, stat_bonus: 25.0, stat_type: "critical_strike" });
        gems.push(GemDefinition { gem_id: 6, name: "Topaz", color: SocketColor::Yellow, stat_bonus: 20.0, stat_type: "haste" });
        Self { gems, rng: LootRng::new(seed) }
    }

    pub fn generate_sockets(&mut self, item_level: u32) -> Vec<Socket> {
        let num_sockets = if item_level >= 80 { 3 } else if item_level >= 50 { 2 } else { 1 };
        let colors = [SocketColor::Red, SocketColor::Blue, SocketColor::Green, SocketColor::White];
        (0..num_sockets).map(|_| {
            let idx = self.rng.next_u32() as usize % colors.len();
            Socket { color: colors[idx].clone(), gem_id: None }
        }).collect()
    }

    pub fn insert_gem(&self, socket: &mut Socket, gem_id: u32) -> bool {
        if let Some(gem) = self.gems.iter().find(|g| g.gem_id == gem_id) {
            if gem.color == socket.color || socket.color == SocketColor::White {
                socket.gem_id = Some(gem_id);
                return true;
            }
        }
        false
    }

    pub fn calculate_socket_bonus(&self, sockets: &[Socket], stat: &str) -> f32 {
        sockets.iter()
            .filter_map(|s| s.gem_id.and_then(|gid| self.gems.iter().find(|g| g.gem_id == gid)))
            .filter(|g| g.stat_type == stat || g.stat_type == "all_stats")
            .map(|g| g.stat_bonus)
            .sum()
    }
}

// ============================================================
// SECTION 98: LOOT ACHIEVEMENT TRACKER
// ============================================================

#[derive(Clone, Debug)]
pub struct LootAchievement {
    pub id: u32,
    pub name: &'static str,
    pub description: &'static str,
    pub target_count: u32,
    pub current_count: u32,
    pub completed: bool,
    pub reward_item_id: Option<u32>,
}

impl LootAchievement {
    pub fn new(id: u32, name: &'static str, description: &'static str, target: u32, reward: Option<u32>) -> Self {
        Self { id, name, description, target_count: target, current_count: 0, completed: false, reward_item_id: reward }
    }

    pub fn increment(&mut self) -> bool {
        if self.completed { return false; }
        self.current_count += 1;
        if self.current_count >= self.target_count {
            self.completed = true;
            return true;
        }
        false
    }

    pub fn progress_pct(&self) -> f32 {
        (self.current_count as f32 / self.target_count as f32).min(1.0)
    }
}

pub struct AchievementTracker {
    pub achievements: Vec<LootAchievement>,
    pub completed_ids: Vec<u32>,
}

impl AchievementTracker {
    pub fn new() -> Self {
        let mut achievements = Vec::new();
        achievements.push(LootAchievement::new(1, "First Blood", "Get your first drop", 1, Some(9001)));
        achievements.push(LootAchievement::new(2, "Collector", "Collect 100 items", 100, Some(9002)));
        achievements.push(LootAchievement::new(3, "Legendary Hunter", "Obtain 10 legendary items", 10, Some(9003)));
        achievements.push(LootAchievement::new(4, "Crafter", "Craft 50 items", 50, Some(9004)));
        achievements.push(LootAchievement::new(5, "Hoarder", "Collect 500 items", 500, Some(9005)));
        achievements.push(LootAchievement::new(6, "Boss Slayer", "Kill 100 bosses", 100, Some(9006)));
        achievements.push(LootAchievement::new(7, "Enchanter", "Apply 25 enchantments", 25, Some(9007)));
        achievements.push(LootAchievement::new(8, "Mythic Collector", "Obtain 3 mythic items", 3, Some(9008)));
        Self { achievements, completed_ids: Vec::new() }
    }

    pub fn record_event(&mut self, event: &str, amount: u32) -> Vec<u32> {
        let mut newly_completed = Vec::new();
        for ach in &mut self.achievements {
            if ach.name.to_lowercase().contains(event) {
                for _ in 0..amount {
                    if ach.increment() {
                        newly_completed.push(ach.id);
                    }
                }
            }
        }
        self.completed_ids.extend_from_slice(&newly_completed);
        newly_completed
    }

    pub fn completion_rate(&self) -> f32 {
        let done = self.achievements.iter().filter(|a| a.completed).count();
        done as f32 / self.achievements.len() as f32
    }
}

// ============================================================
// SECTION 99: GLOBAL LOOT EDITOR STATE
// ============================================================

pub struct GlobalLootEditorState {
    pub socket_system: SocketSystem,
    pub affinity_registry: AffinityRegistry,
    pub achievement_tracker: AchievementTracker,
    pub version: u32,
    pub session_id: u64,
}

impl GlobalLootEditorState {
    pub fn new(seed: u64) -> Self {
        Self {
            socket_system: SocketSystem::new(seed),
            affinity_registry: AffinityRegistry::new(),
            achievement_tracker: AchievementTracker::new(),
            version: 1,
            session_id: seed,
        }
    }

    pub fn process_item_looted(&mut self, item_id: u32, item_level: u32) -> Vec<Socket> {
        self.achievement_tracker.record_event("collector", 1);
        self.socket_system.generate_sockets(item_level)
    }

    pub fn summary(&self) -> String {
        format!(
            "Session {} | Achievements: {:.0}% complete | Version: {}",
            self.session_id,
            self.achievement_tracker.completion_rate() * 100.0,
            self.version
        )
    }
}

// ============================================================
// SECTION 100: LOOT EDITOR ENTRY POINT
// ============================================================

pub fn create_full_loot_editor(seed: u64) -> GlobalLootEditorState {
    GlobalLootEditorState::new(seed)
}

pub fn loot_editor_full_version() -> &'static str {
    "LootEditor v2.1 - Full Feature Set - 100 Sections"
}

#[test]
fn test_socket_system() {
    let mut sys = SocketSystem::new(42);
    let sockets = sys.generate_sockets(80);
    assert_eq!(sockets.len(), 3);
}

#[test]
fn test_achievement_tracker() {
    let mut tracker = AchievementTracker::new();
    let completed = tracker.record_event("first blood", 1);
    assert!(completed.contains(&1));
}

#[test]
fn test_affinity_registry() {
    let mut reg = AffinityRegistry::new();
    reg.register(ItemAffinity {
        affinity_id: 1,
        name: "Fire Set",
        required_items: vec![100, 101],
        bonuses: vec![AffinityBonus { stat: "fire_damage", bonus_pct: 0.15 }],
    });
    let bonus = reg.total_bonus(&[100, 101], "fire_damage");
    assert!((bonus - 0.15).abs() < 1e-6);
}
