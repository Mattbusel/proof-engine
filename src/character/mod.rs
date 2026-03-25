// src/character/mod.rs
// Character system: stats, inventory, skills, quests, and more.

pub mod stats;
pub mod inventory;
pub mod skills;
pub mod quests;

use std::collections::HashMap;
use stats::{StatSheet, AllResources, LevelData, StatGrowth, ModifierRegistry, ClassArchetype, StatPreset, XpCurve};
use inventory::{EquippedItems, Inventory, Stash};
use skills::{SkillBook, AbilityBar, CooldownTracker, ComboSystem};
use quests::{QuestJournal, AchievementSystem, QuestTracker};

// ---------------------------------------------------------------------------
// CharacterId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CharacterId(pub u64);

impl CharacterId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    pub fn inner(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for CharacterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Char({})", self.0)
    }
}

// ---------------------------------------------------------------------------
// CharacterKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterKind {
    Player,
    NPC,
    Monster,
    Boss,
    Summon,
    Pet,
    Merchant,
    QuestGiver,
}

impl CharacterKind {
    pub fn is_hostile_to_player(&self) -> bool {
        matches!(self, CharacterKind::Monster | CharacterKind::Boss)
    }

    pub fn is_friendly_to_player(&self) -> bool {
        matches!(self, CharacterKind::NPC | CharacterKind::Pet | CharacterKind::Merchant | CharacterKind::QuestGiver)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            CharacterKind::Player => "Player",
            CharacterKind::NPC => "NPC",
            CharacterKind::Monster => "Monster",
            CharacterKind::Boss => "Boss",
            CharacterKind::Summon => "Summon",
            CharacterKind::Pet => "Pet",
            CharacterKind::Merchant => "Merchant",
            CharacterKind::QuestGiver => "Quest Giver",
        }
    }
}

// ---------------------------------------------------------------------------
// CharacterState — the current action/animation state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterState {
    Idle,
    Moving,
    Attacking,
    Casting,
    Stunned,
    Dead,
    Interacting,
    Resting,
    Fleeing,
    Defending,
    Dashing,
    Falling,
}

impl CharacterState {
    pub fn is_alive(&self) -> bool {
        !matches!(self, CharacterState::Dead)
    }

    pub fn can_act(&self) -> bool {
        matches!(self, CharacterState::Idle | CharacterState::Moving | CharacterState::Defending)
    }

    pub fn can_move(&self) -> bool {
        matches!(self, CharacterState::Idle | CharacterState::Moving | CharacterState::Fleeing)
    }

    pub fn is_incapacitated(&self) -> bool {
        matches!(self, CharacterState::Stunned | CharacterState::Dead | CharacterState::Falling)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            CharacterState::Idle => "Idle",
            CharacterState::Moving => "Moving",
            CharacterState::Attacking => "Attacking",
            CharacterState::Casting => "Casting",
            CharacterState::Stunned => "Stunned",
            CharacterState::Dead => "Dead",
            CharacterState::Interacting => "Interacting",
            CharacterState::Resting => "Resting",
            CharacterState::Fleeing => "Fleeing",
            CharacterState::Defending => "Defending",
            CharacterState::Dashing => "Dashing",
            CharacterState::Falling => "Falling",
        }
    }
}

// ---------------------------------------------------------------------------
// CharacterRelationship + FactionSystem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterRelationship {
    Allied,
    Neutral,
    Hostile,
    Feared,
    Worshipped,
    Ignored,
}

impl CharacterRelationship {
    pub fn from_reputation(rep: i32) -> Self {
        match rep {
            i32::MIN..=-500 => CharacterRelationship::Hostile,
            -499..=-100 => CharacterRelationship::Feared,
            -99..=99 => CharacterRelationship::Neutral,
            100..=499 => CharacterRelationship::Allied,
            500..=999 => CharacterRelationship::Allied,
            _ => CharacterRelationship::Worshipped,
        }
    }

    pub fn is_hostile(&self) -> bool {
        matches!(self, CharacterRelationship::Hostile | CharacterRelationship::Feared)
    }

    pub fn is_friendly(&self) -> bool {
        matches!(self, CharacterRelationship::Allied | CharacterRelationship::Worshipped)
    }
}

#[derive(Debug, Clone)]
pub struct Faction {
    pub name: String,
    pub description: String,
    pub default_relationship: CharacterRelationship,
    pub allied_factions: Vec<String>,
    pub hostile_factions: Vec<String>,
    pub icon: char,
}

impl Faction {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            default_relationship: CharacterRelationship::Neutral,
            allied_factions: Vec::new(),
            hostile_factions: Vec::new(),
            icon: '⚑',
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn set_default(mut self, rel: CharacterRelationship) -> Self {
        self.default_relationship = rel;
        self
    }

    pub fn allied_with(mut self, faction: impl Into<String>) -> Self {
        self.allied_factions.push(faction.into());
        self
    }

    pub fn hostile_to(mut self, faction: impl Into<String>) -> Self {
        self.hostile_factions.push(faction.into());
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct FactionSystem {
    pub factions: HashMap<String, Faction>,
    pub player_reputation: HashMap<String, i32>, // faction_name -> reputation
}

impl FactionSystem {
    pub fn new() -> Self {
        let mut sys = Self::default();
        sys.register_defaults();
        sys
    }

    fn register_defaults(&mut self) {
        self.add_faction(Faction::new("Adventurers Guild")
            .with_description("The guild of brave heroes.")
            .set_default(CharacterRelationship::Neutral));
        self.add_faction(Faction::new("Merchants League")
            .with_description("Trade confederation.")
            .set_default(CharacterRelationship::Neutral));
        self.add_faction(Faction::new("Dark Brotherhood")
            .with_description("An assassin cult.")
            .set_default(CharacterRelationship::Hostile));
        self.add_faction(Faction::new("Kingdom Guard")
            .with_description("Defenders of the realm.")
            .set_default(CharacterRelationship::Neutral)
            .allied_with("Adventurers Guild")
            .hostile_to("Dark Brotherhood"));
        self.add_faction(Faction::new("Undead Horde")
            .with_description("Shambling undead.")
            .set_default(CharacterRelationship::Hostile));
    }

    pub fn add_faction(&mut self, faction: Faction) {
        self.factions.insert(faction.name.clone(), faction);
    }

    pub fn get_reputation(&self, faction: &str) -> i32 {
        *self.player_reputation.get(faction).unwrap_or(&0)
    }

    pub fn modify_reputation(&mut self, faction: &str, delta: i32) {
        let rep = self.player_reputation.entry(faction.to_string()).or_insert(0);
        *rep = (*rep + delta).clamp(-1000, 1000);
    }

    pub fn get_relationship(&self, faction: &str) -> CharacterRelationship {
        let rep = self.get_reputation(faction);
        let base = self.factions.get(faction)
            .map(|f| f.default_relationship)
            .unwrap_or(CharacterRelationship::Neutral);
        if rep != 0 {
            CharacterRelationship::from_reputation(rep)
        } else {
            base
        }
    }

    pub fn all_faction_names(&self) -> Vec<&str> {
        self.factions.keys().map(|s| s.as_str()).collect()
    }

    pub fn faction_relationship_between(&self, faction_a: &str, faction_b: &str) -> CharacterRelationship {
        if let Some(f) = self.factions.get(faction_a) {
            if f.allied_factions.iter().any(|x| x == faction_b) {
                return CharacterRelationship::Allied;
            }
            if f.hostile_factions.iter().any(|x| x == faction_b) {
                return CharacterRelationship::Hostile;
            }
        }
        CharacterRelationship::Neutral
    }
}

// ---------------------------------------------------------------------------
// AppearanceData — visual representation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AppearanceData {
    pub glyph_char: char,
    pub color: (f32, f32, f32, f32), // RGBA [0,1]
    pub scale: f32,
    pub formation_preset: String,
    pub glow_color: (f32, f32, f32),
    pub glow_radius: f32,
    pub title: String,
    pub portrait_char: char,
}

impl AppearanceData {
    pub fn new(glyph_char: char) -> Self {
        Self {
            glyph_char,
            color: (1.0, 1.0, 1.0, 1.0),
            scale: 1.0,
            formation_preset: "diamond".to_string(),
            glow_color: (1.0, 1.0, 1.0),
            glow_radius: 0.5,
            title: String::new(),
            portrait_char: '@',
        }
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.color = (r, g, b, 1.0);
        self
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_formation(mut self, preset: impl Into<String>) -> Self {
        self.formation_preset = preset.into();
        self
    }

    pub fn with_glow(mut self, r: f32, g: f32, b: f32, radius: f32) -> Self {
        self.glow_color = (r, g, b);
        self.glow_radius = radius;
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
}

impl Default for AppearanceData {
    fn default() -> Self {
        Self::new('@')
    }
}

// ---------------------------------------------------------------------------
// CharacterController — movement, physics, input
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CharacterController {
    pub position: (f32, f32, f32),
    pub velocity: (f32, f32, f32),
    pub acceleration: (f32, f32, f32),
    pub friction: f32,
    pub max_speed: f32,
    pub jump_force: f32,
    pub is_grounded: bool,
    pub facing_direction: f32, // radians
    pub collision_radius: f32,
    pub collision_height: f32,
    pub gravity: f32,
    pub can_fly: bool,
    pub input_move: (f32, f32), // normalized move input
    pub input_jump: bool,
    pub input_dash: bool,
    pub dash_cooldown: f32,
    pub dash_speed: f32,
    pub dash_duration: f32,
    pub dash_timer: f32,
    pub knockback: (f32, f32, f32),
    pub knockback_decay: f32,
}

impl CharacterController {
    pub fn new(position: (f32, f32, f32)) -> Self {
        Self {
            position,
            velocity: (0.0, 0.0, 0.0),
            acceleration: (0.0, 0.0, 0.0),
            friction: 0.85,
            max_speed: 5.0,
            jump_force: 8.0,
            is_grounded: true,
            facing_direction: 0.0,
            collision_radius: 0.4,
            collision_height: 1.8,
            gravity: -20.0,
            can_fly: false,
            input_move: (0.0, 0.0),
            input_jump: false,
            input_dash: false,
            dash_cooldown: 0.0,
            dash_speed: 15.0,
            dash_duration: 0.15,
            dash_timer: 0.0,
            knockback: (0.0, 0.0, 0.0),
            knockback_decay: 0.8,
        }
    }

    pub fn tick(&mut self, dt: f32, move_speed: f32) {
        // Apply input to velocity
        let speed = move_speed.max(0.0);
        let (ix, iz) = self.input_move;
        let len = (ix * ix + iz * iz).sqrt();
        let (nx, nz) = if len > 0.001 {
            (ix / len, iz / len)
        } else {
            (0.0, 0.0)
        };

        self.velocity.0 = nx * speed;
        self.velocity.2 = nz * speed;

        // Dash
        if self.input_dash && self.dash_cooldown <= 0.0 {
            self.velocity.0 = nx * self.dash_speed;
            self.velocity.2 = nz * self.dash_speed;
            self.dash_timer = self.dash_duration;
            self.dash_cooldown = 0.8; // 0.8s dash cooldown
        }
        if self.dash_timer > 0.0 {
            self.dash_timer -= dt;
        }
        if self.dash_cooldown > 0.0 {
            self.dash_cooldown -= dt;
        }

        // Gravity
        if !self.is_grounded && !self.can_fly {
            self.velocity.1 += self.gravity * dt;
        }

        // Jump
        if self.input_jump && self.is_grounded {
            self.velocity.1 = self.jump_force;
            self.is_grounded = false;
        }

        // Apply knockback
        self.velocity.0 += self.knockback.0;
        self.velocity.1 += self.knockback.1;
        self.velocity.2 += self.knockback.2;
        self.knockback.0 *= self.knockback_decay;
        self.knockback.1 *= self.knockback_decay;
        self.knockback.2 *= self.knockback_decay;

        // Clamp horizontal speed
        let hspd = (self.velocity.0 * self.velocity.0 + self.velocity.2 * self.velocity.2).sqrt();
        if hspd > self.max_speed && self.dash_timer <= 0.0 {
            let scale = self.max_speed / hspd;
            self.velocity.0 *= scale;
            self.velocity.2 *= scale;
        }

        // Integrate position
        self.position.0 += self.velocity.0 * dt;
        self.position.1 += self.velocity.1 * dt;
        self.position.2 += self.velocity.2 * dt;

        // Ground collision (simple flat floor at y=0)
        if self.position.1 < 0.0 {
            self.position.1 = 0.0;
            self.velocity.1 = 0.0;
            self.is_grounded = true;
        }

        // Friction (horizontal)
        if self.is_grounded {
            self.velocity.0 *= self.friction;
            self.velocity.2 *= self.friction;
        }

        // Update facing direction from velocity
        if self.velocity.0.abs() > 0.01 || self.velocity.2.abs() > 0.01 {
            self.facing_direction = self.velocity.2.atan2(self.velocity.0);
        }

        // Reset inputs
        self.input_move = (0.0, 0.0);
        self.input_jump = false;
        self.input_dash = false;
    }

    pub fn apply_knockback(&mut self, direction: (f32, f32, f32), force: f32) {
        let (dx, dy, dz) = direction;
        let len = (dx * dx + dy * dy + dz * dz).sqrt().max(0.001);
        self.knockback.0 += dx / len * force;
        self.knockback.1 += dy / len * force;
        self.knockback.2 += dz / len * force;
    }

    pub fn distance_to(&self, other: &CharacterController) -> f32 {
        let dx = self.position.0 - other.position.0;
        let dy = self.position.1 - other.position.1;
        let dz = self.position.2 - other.position.2;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn is_colliding_with(&self, other: &CharacterController) -> bool {
        let dist = self.distance_to(other);
        dist < self.collision_radius + other.collision_radius
    }

    pub fn resolve_collision(&mut self, other: &mut CharacterController) {
        if !self.is_colliding_with(other) { return; }
        let dx = self.position.0 - other.position.0;
        let dz = self.position.2 - other.position.2;
        let dist = (dx * dx + dz * dz).sqrt().max(0.001);
        let overlap = self.collision_radius + other.collision_radius - dist;
        let nx = dx / dist;
        let nz = dz / dist;
        self.position.0 += nx * overlap * 0.5;
        self.position.2 += nz * overlap * 0.5;
        other.position.0 -= nx * overlap * 0.5;
        other.position.2 -= nz * overlap * 0.5;
    }
}

// ---------------------------------------------------------------------------
// InputBinding — key bindings for player character
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InputBinding {
    pub move_up: u32,
    pub move_down: u32,
    pub move_left: u32,
    pub move_right: u32,
    pub jump: u32,
    pub dash: u32,
    pub interact: u32,
    pub ability_slots: [u32; 12],
    pub inventory: u32,
    pub map: u32,
    pub quest_log: u32,
    pub character_screen: u32,
}

impl Default for InputBinding {
    fn default() -> Self {
        Self {
            move_up: b'w' as u32,
            move_down: b's' as u32,
            move_left: b'a' as u32,
            move_right: b'd' as u32,
            jump: b' ' as u32,
            dash: b'e' as u32,
            interact: b'f' as u32,
            ability_slots: [b'1' as u32, b'2' as u32, b'3' as u32, b'4' as u32,
                b'5' as u32, b'6' as u32, b'7' as u32, b'8' as u32,
                b'9' as u32, b'0' as u32, b'q' as u32, b'r' as u32],
            inventory: b'i' as u32,
            map: b'm' as u32,
            quest_log: b'j' as u32,
            character_screen: b'c' as u32,
        }
    }
}

// ---------------------------------------------------------------------------
// CharacterEvents
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CharacterEvent {
    Spawned(CharacterId),
    Died(CharacterId),
    LevelUp { id: CharacterId, new_level: u32 },
    SkillLearned { id: CharacterId, skill_name: String },
    ItemPickup { id: CharacterId, item_name: String },
    TookDamage { id: CharacterId, amount: f32, source: String },
    Healed { id: CharacterId, amount: f32 },
    StateChanged { id: CharacterId, old: CharacterState, new: CharacterState },
    QuestAccepted { id: CharacterId, quest_name: String },
    QuestCompleted { id: CharacterId, quest_name: String },
    AchievementUnlocked { id: CharacterId, achievement_name: String },
    FactionRepChange { id: CharacterId, faction: String, delta: i32 },
}

// ---------------------------------------------------------------------------
// Character — the master struct combining all subsystems
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Character {
    pub id: CharacterId,
    pub name: String,
    pub kind: CharacterKind,
    pub state: CharacterState,
    pub archetype: ClassArchetype,

    // Core subsystems
    pub stats: StatSheet,
    pub resources: AllResources,
    pub level_data: LevelData,
    pub modifier_registry: ModifierRegistry,
    pub stat_growth: StatGrowth,

    // Equipment and items
    pub equipped: EquippedItems,
    pub inventory: Inventory,
    pub stash: Stash,
    pub gold: u64,

    // Skills
    pub skill_book: SkillBook,
    pub ability_bar: AbilityBar,
    pub cooldowns: CooldownTracker,
    pub combos: ComboSystem,

    // Quests
    pub journal: QuestJournal,
    pub achievements: AchievementSystem,
    pub quest_tracker: QuestTracker,

    // Social
    pub faction: Option<String>,
    pub faction_system: FactionSystem,

    // Movement / physics
    pub controller: CharacterController,
    pub input_binding: InputBinding,

    // Visual
    pub appearance: AppearanceData,

    // Meta
    pub is_player_controlled: bool,
    pub event_queue: Vec<CharacterEvent>,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl Character {
    pub fn new(id: CharacterId, name: impl Into<String>, kind: CharacterKind, archetype: ClassArchetype) -> Self {
        let stats = StatPreset::for_class(archetype, 1);
        let resources = AllResources::from_sheet(&stats, 1);
        let level_data = LevelData::new(XpCurve::default(), 100);
        let stat_growth = StatPreset::growth_for(archetype);
        let appearance = match kind {
            CharacterKind::Player => AppearanceData::new('@').with_color(0.0, 0.8, 1.0),
            CharacterKind::Monster => AppearanceData::new('M').with_color(1.0, 0.2, 0.2),
            CharacterKind::Boss => AppearanceData::new('B').with_color(1.0, 0.5, 0.0).with_scale(1.5),
            CharacterKind::NPC => AppearanceData::new('N').with_color(0.8, 0.8, 0.2),
            CharacterKind::Summon => AppearanceData::new('S').with_color(0.5, 0.5, 1.0),
            CharacterKind::Pet => AppearanceData::new('p').with_color(0.5, 1.0, 0.5),
            CharacterKind::Merchant => AppearanceData::new('$').with_color(0.9, 0.7, 0.2),
            CharacterKind::QuestGiver => AppearanceData::new('Q').with_color(0.2, 1.0, 0.5),
        };
        Self {
            id,
            name: name.into(),
            kind,
            state: CharacterState::Idle,
            archetype,
            stats,
            resources,
            level_data,
            modifier_registry: ModifierRegistry::new(),
            stat_growth,
            equipped: EquippedItems::new(),
            inventory: Inventory::new(40, 200.0),
            stash: Stash::new(),
            gold: 0,
            skill_book: SkillBook::new(),
            ability_bar: AbilityBar::new(),
            cooldowns: CooldownTracker::new(),
            combos: ComboSystem::new(),
            journal: QuestJournal::new(),
            achievements: AchievementSystem::new(),
            quest_tracker: QuestTracker::new(5),
            faction: None,
            faction_system: FactionSystem::new(),
            controller: CharacterController::new((0.0, 0.0, 0.0)),
            input_binding: InputBinding::default(),
            appearance,
            is_player_controlled: matches!(kind, CharacterKind::Player),
            event_queue: Vec::new(),
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Tick all time-dependent subsystems by dt seconds.
    pub fn tick(&mut self, dt: f32) {
        if !self.state.is_alive() { return; }

        // Resources regen
        self.resources.tick(dt);

        // Cooldowns
        self.cooldowns.tick(dt);

        // Combo window
        self.combos.tick(dt);

        // Quest time limits
        let failed_quests = self.journal.tick(dt);
        for qid in failed_quests {
            if let Some(quest) = self.journal.failed.last() {
                let name = quest.name.clone();
                self.event_queue.push(CharacterEvent::QuestCompleted { id: self.id, quest_name: name });
                let _ = qid;
            }
        }

        // Movement
        let move_speed = self.stats.move_speed();
        if self.state.can_move() {
            self.controller.tick(dt, move_speed);
        }

        // State transitions based on velocity
        if self.state == CharacterState::Moving {
            let (vx, _, vz) = self.controller.velocity;
            if vx.abs() < 0.05 && vz.abs() < 0.05 {
                self.set_state(CharacterState::Idle);
            }
        }
    }

    pub fn set_state(&mut self, new_state: CharacterState) {
        if self.state == new_state { return; }
        let old = self.state;
        self.state = new_state;
        self.event_queue.push(CharacterEvent::StateChanged { id: self.id, old, new: new_state });
    }

    /// Deal damage to this character. Returns true if the character died.
    pub fn take_damage(&mut self, amount: f32, source: &str) -> bool {
        if !self.state.is_alive() { return false; }

        // Compute effective defense
        let defense = self.stats.defense(self.equipped.total_defense());
        let effective = (amount - defense * 0.5).max(1.0);

        let actual = self.resources.hp.drain(effective);
        self.event_queue.push(CharacterEvent::TookDamage { id: self.id, amount: actual, source: source.to_string() });

        // Track in achievements
        self.achievements.record_kill(""); // placeholder — actual kills tracked by killer

        if self.resources.hp.empty() {
            self.set_state(CharacterState::Dead);
            self.event_queue.push(CharacterEvent::Died(self.id));
            return true;
        }
        false
    }

    /// Heal this character.
    pub fn heal(&mut self, amount: f32) -> f32 {
        let healed = self.resources.hp.restore(amount);
        self.event_queue.push(CharacterEvent::Healed { id: self.id, amount: healed });
        healed
    }

    /// Gain experience, potentially levelling up.
    pub fn gain_xp(&mut self, amount: u64) {
        let levels = self.level_data.add_xp(amount);
        for _ in 0..levels {
            self.level_up();
        }
    }

    fn level_up(&mut self) {
        self.level_data.level_up(5, 1);
        self.stat_growth.apply_to(&mut self.stats);
        let new_level = self.level_data.level;

        // Recalculate resource maxes
        let max_hp = self.stats.max_hp(new_level);
        let max_mp = self.stats.max_mp(new_level);
        let max_st = self.stats.max_stamina(new_level);
        self.resources.hp.set_max(max_hp, true);
        self.resources.mp.set_max(max_mp, true);
        self.resources.stamina.set_max(max_st, true);
        // Restore on level up
        self.resources.hp.fill();
        self.resources.mp.fill();
        self.resources.stamina.fill();

        let newly_unlocked = self.achievements.check_level(new_level);
        for ach_id in newly_unlocked {
            if let Some(ach) = self.achievements.achievements.iter().find(|a| a.id == ach_id) {
                let name = ach.name.clone();
                self.event_queue.push(CharacterEvent::AchievementUnlocked { id: self.id, achievement_name: name });
            }
        }

        self.event_queue.push(CharacterEvent::LevelUp { id: self.id, new_level });
    }

    /// Modify reputation with a faction.
    pub fn change_reputation(&mut self, faction: &str, delta: i32) {
        self.faction_system.modify_reputation(faction, delta);
        self.event_queue.push(CharacterEvent::FactionRepChange { id: self.id, faction: faction.to_string(), delta });
    }

    /// Get current level.
    pub fn level(&self) -> u32 {
        self.level_data.level
    }

    /// Whether this character is alive.
    pub fn is_alive(&self) -> bool {
        self.state.is_alive()
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<CharacterEvent> {
        std::mem::take(&mut self.event_queue)
    }

    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.push(tag.into());
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// CharacterBundle — everything needed to spawn a character
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CharacterBundle {
    pub id: CharacterId,
    pub name: String,
    pub kind: CharacterKind,
    pub archetype: ClassArchetype,
    pub level: u32,
    pub spawn_position: (f32, f32, f32),
    pub faction: Option<String>,
    pub is_player_controlled: bool,
    pub tags: Vec<String>,
}

impl CharacterBundle {
    pub fn player(name: impl Into<String>, archetype: ClassArchetype, pos: (f32, f32, f32)) -> Self {
        Self {
            id: CharacterId(1),
            name: name.into(),
            kind: CharacterKind::Player,
            archetype,
            level: 1,
            spawn_position: pos,
            faction: Some("Adventurers Guild".to_string()),
            is_player_controlled: true,
            tags: vec!["player".to_string()],
        }
    }

    pub fn monster(name: impl Into<String>, level: u32, pos: (f32, f32, f32)) -> Self {
        Self {
            id: CharacterId(0), // Will be assigned by registry
            name: name.into(),
            kind: CharacterKind::Monster,
            archetype: ClassArchetype::Warrior,
            level,
            spawn_position: pos,
            faction: Some("Undead Horde".to_string()),
            is_player_controlled: false,
            tags: vec!["enemy".to_string()],
        }
    }

    pub fn build(self, id: CharacterId) -> Character {
        let mut ch = Character::new(id, self.name, self.kind, self.archetype);
        ch.controller.position = self.spawn_position;
        ch.faction = self.faction;
        ch.is_player_controlled = self.is_player_controlled;
        for tag in self.tags {
            ch.tags.push(tag);
        }
        // Scale stats to level
        for _ in 1..self.level {
            ch.stat_growth.apply_to(&mut ch.stats);
        }
        let lv = self.level;
        let max_hp = ch.stats.max_hp(lv);
        let max_mp = ch.stats.max_mp(lv);
        let max_st = ch.stats.max_stamina(lv);
        ch.resources.hp.set_max(max_hp, false);
        ch.resources.mp.set_max(max_mp, false);
        ch.resources.stamina.set_max(max_st, false);
        ch.resources.hp.fill();
        ch.resources.mp.fill();
        ch.resources.stamina.fill();
        ch.level_data.level = self.level;
        ch.event_queue.push(CharacterEvent::Spawned(id));
        ch
    }
}

// ---------------------------------------------------------------------------
// CharacterRegistry — global map of all active characters
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CharacterRegistry {
    pub characters: HashMap<CharacterId, Character>,
    next_id: u64,
}

impl CharacterRegistry {
    pub fn new() -> Self {
        Self { characters: HashMap::new(), next_id: 1 }
    }

    pub fn next_id(&mut self) -> CharacterId {
        let id = CharacterId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn spawn(&mut self, bundle: CharacterBundle) -> CharacterId {
        let id = if bundle.id.0 == 0 {
            self.next_id()
        } else {
            // Ensure next_id stays ahead of manually-assigned ids.
            if bundle.id.0 >= self.next_id {
                self.next_id = bundle.id.0 + 1;
            }
            bundle.id
        };
        let character = bundle.build(id);
        self.characters.insert(id, character);
        id
    }

    pub fn despawn(&mut self, id: CharacterId) -> Option<Character> {
        self.characters.remove(&id)
    }

    pub fn get(&self, id: CharacterId) -> Option<&Character> {
        self.characters.get(&id)
    }

    pub fn get_mut(&mut self, id: CharacterId) -> Option<&mut Character> {
        self.characters.get_mut(&id)
    }

    pub fn tick_all(&mut self, dt: f32) {
        for ch in self.characters.values_mut() {
            ch.tick(dt);
        }
    }

    pub fn all_alive(&self) -> impl Iterator<Item = &Character> {
        self.characters.values().filter(|c| c.is_alive())
    }

    pub fn all_dead(&self) -> impl Iterator<Item = &Character> {
        self.characters.values().filter(|c| !c.is_alive())
    }

    pub fn player(&self) -> Option<&Character> {
        self.characters.values().find(|c| c.is_player_controlled)
    }

    pub fn player_mut(&mut self) -> Option<&mut Character> {
        self.characters.values_mut().find(|c| c.is_player_controlled)
    }

    pub fn count(&self) -> usize {
        self.characters.len()
    }

    pub fn by_kind(&self, kind: CharacterKind) -> Vec<&Character> {
        self.characters.values().filter(|c| c.kind == kind).collect()
    }

    pub fn remove_all_dead(&mut self) {
        self.characters.retain(|_, c| c.is_alive());
    }

    pub fn drain_all_events(&mut self) -> Vec<(CharacterId, CharacterEvent)> {
        let mut events = Vec::new();
        for ch in self.characters.values_mut() {
            for ev in ch.drain_events() {
                events.push((ch.id, ev));
            }
        }
        events
    }

    pub fn find_in_radius(&self, center: (f32, f32, f32), radius: f32) -> Vec<CharacterId> {
        self.characters.values()
            .filter(|c| {
                let (cx, cy, cz) = c.controller.position;
                let (ox, oy, oz) = center;
                let dx = cx - ox; let dy = cy - oy; let dz = cz - oz;
                (dx*dx + dy*dy + dz*dz).sqrt() <= radius
            })
            .map(|c| c.id)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_player() -> Character {
        Character::new(CharacterId(1), "Hero", CharacterKind::Player, ClassArchetype::Warrior)
    }

    #[test]
    fn test_character_creation() {
        let ch = make_player();
        assert_eq!(ch.id, CharacterId(1));
        assert_eq!(ch.name, "Hero");
        assert!(ch.is_alive());
    }

    #[test]
    fn test_character_take_damage() {
        let mut ch = make_player();
        ch.take_damage(10.0, "test");
        assert!(ch.resources.hp.current < ch.resources.hp.max);
    }

    #[test]
    fn test_character_death() {
        let mut ch = make_player();
        let died = ch.take_damage(999999.0, "one_shot");
        assert!(died);
        assert_eq!(ch.state, CharacterState::Dead);
    }

    #[test]
    fn test_character_heal() {
        let mut ch = make_player();
        ch.resources.hp.drain(50.0);
        let healed = ch.heal(30.0);
        assert!(healed > 0.0);
    }

    #[test]
    fn test_character_gain_xp() {
        let mut ch = make_player();
        ch.gain_xp(200);
        assert!(ch.level() >= 2);
    }

    #[test]
    fn test_character_state_machine() {
        let mut ch = make_player();
        ch.set_state(CharacterState::Moving);
        assert_eq!(ch.state, CharacterState::Moving);
        ch.set_state(CharacterState::Attacking);
        let events = ch.drain_events();
        assert!(events.iter().any(|e| matches!(e, CharacterEvent::StateChanged { .. })));
    }

    #[test]
    fn test_character_events() {
        let mut ch = make_player();
        ch.take_damage(5.0, "fire");
        ch.heal(3.0);
        let events = ch.drain_events();
        assert!(events.iter().any(|e| matches!(e, CharacterEvent::TookDamage { .. })));
        assert!(events.iter().any(|e| matches!(e, CharacterEvent::Healed { .. })));
    }

    #[test]
    fn test_faction_system_reputation() {
        let mut ch = make_player();
        ch.change_reputation("Adventurers Guild", 100);
        let rep = ch.faction_system.get_reputation("Adventurers Guild");
        assert_eq!(rep, 100);
        let rel = ch.faction_system.get_relationship("Adventurers Guild");
        assert!(rel.is_friendly());
    }

    #[test]
    fn test_character_registry_spawn() {
        let mut registry = CharacterRegistry::new();
        let bundle = CharacterBundle::player("Alice", ClassArchetype::Mage, (0.0, 0.0, 0.0));
        let id = registry.spawn(bundle);
        assert!(registry.get(id).is_some());
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_character_registry_despawn() {
        let mut registry = CharacterRegistry::new();
        let bundle = CharacterBundle::monster("Goblin", 5, (10.0, 0.0, 5.0));
        let id = registry.spawn(bundle);
        let ch = registry.despawn(id);
        assert!(ch.is_some());
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_character_controller_tick() {
        let mut controller = CharacterController::new((0.0, 0.0, 0.0));
        controller.input_move = (1.0, 0.0);
        controller.tick(0.016, 100.0);
        assert!(controller.position.0 > 0.0);
    }

    #[test]
    fn test_character_controller_jump() {
        let mut controller = CharacterController::new((0.0, 0.0, 0.0));
        controller.input_jump = true;
        controller.tick(0.016, 100.0);
        assert!(!controller.is_grounded);
        assert!(controller.velocity.1 > 0.0 || controller.position.1 > 0.0);
    }

    #[test]
    fn test_character_registry_find_in_radius() {
        let mut registry = CharacterRegistry::new();
        let b1 = CharacterBundle::player("Alice", ClassArchetype::Warrior, (0.0, 0.0, 0.0));
        let b2 = CharacterBundle::monster("Goblin", 1, (5.0, 0.0, 0.0));
        let b3 = CharacterBundle::monster("Far Goblin", 1, (100.0, 0.0, 0.0));
        registry.spawn(b1);
        registry.spawn(b2);
        registry.spawn(b3);
        let nearby = registry.find_in_radius((0.0, 0.0, 0.0), 10.0);
        assert_eq!(nearby.len(), 2);
    }

    #[test]
    fn test_faction_relationship_between() {
        let fs = FactionSystem::new();
        let rel = fs.faction_relationship_between("Kingdom Guard", "Dark Brotherhood");
        assert!(rel.is_hostile());
    }
}
