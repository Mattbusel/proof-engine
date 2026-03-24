// src/character/skills.rs
// Skills, abilities, skill trees, cooldowns, and combos.

use std::collections::HashMap;
use crate::character::stats::{StatKind, StatModifier};

// ---------------------------------------------------------------------------
// SkillId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillId(pub u64);

// ---------------------------------------------------------------------------
// SkillType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillType {
    Active,
    Passive,
    Toggle,
    Aura,
    Reaction,
    Ultimate,
}

// ---------------------------------------------------------------------------
// Element & HealTarget & BuffTarget
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Physical,
    Fire,
    Ice,
    Lightning,
    Holy,
    Dark,
    Arcane,
    Poison,
    Nature,
    Wind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealTarget {
    Self_,
    SingleAlly,
    AllAllies,
    AreaAllies { radius: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffTarget {
    Self_,
    SingleAlly,
    SingleEnemy,
    AllAllies,
    AllEnemies,
    Area { radius: u32 },
}

// ---------------------------------------------------------------------------
// SkillEffect — what a skill does when cast
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum SkillEffect {
    Damage {
        base_damage: f32,
        ratio: f32,
        element: Element,
        aoe_radius: f32,
        pierces: bool,
    },
    Heal {
        base_heal: f32,
        ratio: f32,
        target: HealTarget,
    },
    Buff {
        modifiers: Vec<StatModifier>,
        duration_secs: f32,
        target: BuffTarget,
    },
    Debuff {
        modifiers: Vec<StatModifier>,
        duration_secs: f32,
        target: BuffTarget,
    },
    Summon {
        entity_type: String,
        count: u32,
        duration_secs: f32,
    },
    Teleport {
        range: f32,
        blink: bool, // true = instant, false = cast-time warp
    },
    Zone {
        radius: f32,
        duration_secs: f32,
        tick_interval: f32,
        tick_effect: Box<SkillEffect>,
    },
    Projectile {
        speed: f32,
        pierce_count: u32,
        split_count: u32,
        element: Element,
        damage: f32,
    },
    Chain {
        max_targets: u32,
        jump_range: f32,
        damage_reduction: f32,
        element: Element,
        base_damage: f32,
    },
    Shield {
        absorb_amount: f32,
        duration_secs: f32,
    },
    Drain {
        stat: DrainTarget,
        amount: f32,
        return_fraction: f32,
    },
    Composite(Vec<SkillEffect>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainTarget {
    Hp,
    Mp,
    Stamina,
}

impl SkillEffect {
    pub fn is_damaging(&self) -> bool {
        matches!(self, SkillEffect::Damage { .. } | SkillEffect::Projectile { .. } | SkillEffect::Chain { .. })
    }

    pub fn is_healing(&self) -> bool {
        matches!(self, SkillEffect::Heal { .. })
    }
}

// ---------------------------------------------------------------------------
// SkillCost
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SkillCost {
    pub mana: f32,
    pub stamina: f32,
    pub hp: f32,
    pub cooldown_secs: f32,
    pub cast_time_secs: f32,
    pub channel_time_secs: f32,
    pub skill_point_cost: u32,
}

impl SkillCost {
    pub fn free() -> Self {
        Self::default()
    }

    pub fn mana_cost(mana: f32, cooldown: f32) -> Self {
        Self { mana, cooldown_secs: cooldown, ..Default::default() }
    }

    pub fn stamina_cost(stamina: f32, cooldown: f32) -> Self {
        Self { stamina, cooldown_secs: cooldown, ..Default::default() }
    }

    pub fn with_cast_time(mut self, cast_time: f32) -> Self {
        self.cast_time_secs = cast_time;
        self
    }
}

// ---------------------------------------------------------------------------
// SkillRequirement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum SkillRequirement {
    Level(u32),
    SkillRank { skill_id: SkillId, min_rank: u32 },
    Stat { kind: StatKind, min_value: f32 },
    ClassArchetype(String),
}

impl SkillRequirement {
    pub fn check_level(&self, level: u32) -> bool {
        match self {
            SkillRequirement::Level(required) => level >= *required,
            _ => true, // Other requirements checked elsewhere
        }
    }
}

// ---------------------------------------------------------------------------
// Skill
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Skill {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub icon_char: char,
    pub skill_type: SkillType,
    pub max_rank: u32,
    pub requirements: Vec<SkillRequirement>,
    pub effects_per_rank: Vec<SkillEffect>,
    pub cost_per_rank: Vec<SkillCost>,
    pub passive_modifiers: Vec<StatModifier>,
    pub tags: Vec<String>,
}

impl Skill {
    pub fn new(id: SkillId, name: impl Into<String>, skill_type: SkillType) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            icon_char: '*',
            skill_type,
            max_rank: 5,
            requirements: Vec::new(),
            effects_per_rank: Vec::new(),
            cost_per_rank: Vec::new(),
            passive_modifiers: Vec::new(),
            tags: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_icon(mut self, c: char) -> Self {
        self.icon_char = c;
        self
    }

    pub fn with_max_rank(mut self, rank: u32) -> Self {
        self.max_rank = rank;
        self
    }

    pub fn add_requirement(mut self, req: SkillRequirement) -> Self {
        self.requirements.push(req);
        self
    }

    pub fn add_rank_effect(mut self, effect: SkillEffect) -> Self {
        self.effects_per_rank.push(effect);
        self
    }

    pub fn add_rank_cost(mut self, cost: SkillCost) -> Self {
        self.cost_per_rank.push(cost);
        self
    }

    pub fn add_passive(mut self, modifier: StatModifier) -> Self {
        self.passive_modifiers.push(modifier);
        self
    }

    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn effect_at_rank(&self, rank: u32) -> Option<&SkillEffect> {
        let idx = (rank as usize).saturating_sub(1).min(self.effects_per_rank.len().saturating_sub(1));
        self.effects_per_rank.get(idx)
    }

    pub fn cost_at_rank(&self, rank: u32) -> Option<&SkillCost> {
        let idx = (rank as usize).saturating_sub(1).min(self.cost_per_rank.len().saturating_sub(1));
        self.cost_per_rank.get(idx)
    }

    pub fn cooldown_at_rank(&self, rank: u32) -> f32 {
        self.cost_at_rank(rank).map(|c| c.cooldown_secs).unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// SkillNode — a node in a skill tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SkillNode {
    pub skill: Skill,
    pub position: (u8, u8),
    pub unlocked: bool,
    pub rank: u32,
    pub prereqs: Vec<usize>, // indices into the tree's skill list
}

impl SkillNode {
    pub fn new(skill: Skill, position: (u8, u8)) -> Self {
        Self { skill, position, unlocked: false, rank: 0, prereqs: Vec::new() }
    }

    pub fn with_prereqs(mut self, prereqs: Vec<usize>) -> Self {
        self.prereqs = prereqs;
        self
    }

    pub fn is_available(&self, tree: &SkillTree) -> bool {
        if self.prereqs.is_empty() { return true; }
        self.prereqs.iter().all(|&idx| {
            tree.nodes.get(idx).map(|n| n.rank > 0).unwrap_or(false)
        })
    }
}

// ---------------------------------------------------------------------------
// SkillTree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SkillTree {
    pub name: String,
    pub nodes: Vec<SkillNode>,
    pub connections: Vec<(usize, usize)>,
}

impl SkillTree {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), nodes: Vec::new(), connections: Vec::new() }
    }

    pub fn add_node(mut self, node: SkillNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn add_connection(mut self, from: usize, to: usize) -> Self {
        self.connections.push((from, to));
        self
    }

    pub fn total_points_spent(&self) -> u32 {
        self.nodes.iter().map(|n| n.rank).sum()
    }

    pub fn find_by_id(&self, id: SkillId) -> Option<(usize, &SkillNode)> {
        self.nodes.iter().enumerate().find(|(_, n)| n.skill.id == id)
    }

    pub fn find_by_id_mut(&mut self, id: SkillId) -> Option<(usize, &mut SkillNode)> {
        self.nodes.iter_mut().enumerate().find(|(_, n)| n.skill.id == id)
    }

    pub fn available_nodes(&self) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|&i| self.nodes[i].is_available(self))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// SkillBook — a character's known skills
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SkillBook {
    pub known: HashMap<SkillId, (Skill, u32)>, // skill_id -> (skill, rank)
}

impl SkillBook {
    pub fn new() -> Self {
        Self { known: HashMap::new() }
    }

    pub fn learn(&mut self, skill: Skill) -> bool {
        if self.known.contains_key(&skill.id) { return false; }
        self.known.insert(skill.id, (skill, 1));
        true
    }

    pub fn upgrade(&mut self, skill_id: SkillId) -> bool {
        if let Some((skill, rank)) = self.known.get_mut(&skill_id) {
            if *rank < skill.max_rank {
                *rank += 1;
                return true;
            }
        }
        false
    }

    pub fn forget(&mut self, skill_id: SkillId) -> Option<Skill> {
        self.known.remove(&skill_id).map(|(s, _)| s)
    }

    pub fn rank_of(&self, skill_id: SkillId) -> u32 {
        self.known.get(&skill_id).map(|(_, r)| *r).unwrap_or(0)
    }

    pub fn knows(&self, skill_id: SkillId) -> bool {
        self.known.contains_key(&skill_id)
    }

    pub fn all_skills(&self) -> impl Iterator<Item = (&Skill, u32)> {
        self.known.values().map(|(s, r)| (s, *r))
    }

    pub fn passive_skills(&self) -> impl Iterator<Item = (&Skill, u32)> {
        self.known.values()
            .filter(|(s, _)| s.skill_type == SkillType::Passive)
            .map(|(s, r)| (s, *r))
    }

    pub fn active_skills(&self) -> impl Iterator<Item = (&Skill, u32)> {
        self.known.values()
            .filter(|(s, _)| s.skill_type == SkillType::Active || s.skill_type == SkillType::Ultimate)
            .map(|(s, r)| (s, *r))
    }

    pub fn can_afford_upgrade(&self, skill_id: SkillId, skill_points: u32) -> bool {
        if let Some((skill, rank)) = self.known.get(&skill_id) {
            if *rank >= skill.max_rank { return false; }
            let cost = skill.cost_at_rank(*rank + 1)
                .map(|c| c.skill_point_cost)
                .unwrap_or(1);
            skill_points >= cost
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Ability — an active skill bound to a hotkey
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Ability {
    pub skill_id: SkillId,
    pub hotkey: u8,
    pub override_icon: Option<char>,
    pub override_name: Option<String>,
}

impl Ability {
    pub fn new(skill_id: SkillId, hotkey: u8) -> Self {
        Self { skill_id, hotkey, override_icon: None, override_name: None }
    }
}

// ---------------------------------------------------------------------------
// AbilityBar — 12-slot action bar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AbilityBar {
    pub slots: [Option<Ability>; 12],
}

impl AbilityBar {
    pub fn new() -> Self {
        Self { slots: [const { None }; 12] }
    }

    pub fn bind(&mut self, slot: usize, ability: Ability) -> Option<Ability> {
        if slot >= 12 { return None; }
        let old = self.slots[slot].take();
        self.slots[slot] = Some(ability);
        old
    }

    pub fn unbind(&mut self, slot: usize) -> Option<Ability> {
        if slot >= 12 { return None; }
        self.slots[slot].take()
    }

    pub fn get(&self, slot: usize) -> Option<&Ability> {
        self.slots.get(slot).and_then(|s| s.as_ref())
    }

    pub fn find_by_skill(&self, skill_id: SkillId) -> Option<usize> {
        self.slots.iter().position(|s| s.as_ref().map(|a| a.skill_id) == Some(skill_id))
    }

    pub fn occupied_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}

impl Default for AbilityBar {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CooldownTracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CooldownTracker {
    pub timers: HashMap<SkillId, f32>,
}

impl CooldownTracker {
    pub fn new() -> Self {
        Self { timers: HashMap::new() }
    }

    pub fn start(&mut self, skill_id: SkillId, duration: f32) {
        self.timers.insert(skill_id, duration);
    }

    pub fn remaining(&self, skill_id: SkillId) -> f32 {
        *self.timers.get(&skill_id).unwrap_or(&0.0)
    }

    pub fn is_ready(&self, skill_id: SkillId) -> bool {
        self.remaining(skill_id) <= 0.0
    }

    pub fn tick(&mut self, dt: f32) {
        for timer in self.timers.values_mut() {
            *timer = (*timer - dt).max(0.0);
        }
    }

    pub fn reduce(&mut self, skill_id: SkillId, amount: f32) {
        if let Some(t) = self.timers.get_mut(&skill_id) {
            *t = (*t - amount).max(0.0);
        }
    }

    pub fn reset(&mut self, skill_id: SkillId) {
        self.timers.remove(&skill_id);
    }

    pub fn reset_all(&mut self) {
        self.timers.clear();
    }

    pub fn apply_cdr(&mut self, cdr_percent: f32) {
        // Cooldown Reduction: remaining time = remaining * (1 - cdr)
        let mult = (1.0 - cdr_percent / 100.0).max(0.0);
        for timer in self.timers.values_mut() {
            *timer *= mult;
        }
    }
}

// ---------------------------------------------------------------------------
// Combo System — chained skill sequences
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Combo {
    pub name: String,
    pub trigger_sequence: Vec<SkillId>,
    pub bonus_effect: SkillEffect,
    pub window_ms: f32,
    pub reset_on_damage: bool,
}

impl Combo {
    pub fn new(name: impl Into<String>, sequence: Vec<SkillId>, bonus: SkillEffect, window_ms: f32) -> Self {
        Self {
            name: name.into(),
            trigger_sequence: sequence,
            bonus_effect: bonus,
            window_ms,
            reset_on_damage: false,
        }
    }

    pub fn matches(&self, recent: &[SkillId]) -> bool {
        if recent.len() < self.trigger_sequence.len() { return false; }
        let start = recent.len() - self.trigger_sequence.len();
        &recent[start..] == self.trigger_sequence.as_slice()
    }
}

#[derive(Debug, Clone)]
pub struct ComboSystem {
    pub combos: Vec<Combo>,
    pub recent_skills: Vec<SkillId>,
    pub last_skill_time: f32,
    pub current_time: f32,
    pub max_history: usize,
}

impl ComboSystem {
    pub fn new() -> Self {
        Self {
            combos: Vec::new(),
            recent_skills: Vec::new(),
            last_skill_time: 0.0,
            current_time: 0.0,
            max_history: 8,
        }
    }

    pub fn add_combo(&mut self, combo: Combo) {
        self.combos.push(combo);
    }

    pub fn tick(&mut self, dt: f32) {
        self.current_time += dt;
    }

    pub fn register_skill_use(&mut self, skill_id: SkillId) {
        // Reset if window expired
        if let Some(last) = self.recent_skills.last() {
            let _ = last;
            let elapsed_ms = (self.current_time - self.last_skill_time) * 1000.0;
            let max_window = self.combos.iter().map(|c| c.window_ms).fold(0.0f32, f32::max);
            if elapsed_ms > max_window && max_window > 0.0 {
                self.recent_skills.clear();
            }
        }
        self.recent_skills.push(skill_id);
        self.last_skill_time = self.current_time;
        if self.recent_skills.len() > self.max_history {
            self.recent_skills.remove(0);
        }
    }

    pub fn check_combos(&self) -> Vec<&Combo> {
        let elapsed_ms = (self.current_time - self.last_skill_time) * 1000.0;
        self.combos.iter()
            .filter(|c| {
                c.matches(&self.recent_skills) && elapsed_ms <= c.window_ms
            })
            .collect()
    }

    pub fn reset(&mut self) {
        self.recent_skills.clear();
    }
}

impl Default for ComboSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Skill Presets — class archetypes with pre-built skill trees
// ---------------------------------------------------------------------------

pub struct SkillPresets;

impl SkillPresets {
    pub fn warrior_tree() -> SkillTree {
        let slash = Skill::new(SkillId(1001), "Power Slash", SkillType::Active)
            .with_description("A powerful slash dealing heavy physical damage.")
            .with_icon('/')
            .with_max_rank(5)
            .add_rank_effect(SkillEffect::Damage { base_damage: 30.0, ratio: 1.5, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(20.0, 4.0))
            .add_rank_effect(SkillEffect::Damage { base_damage: 45.0, ratio: 1.7, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(20.0, 3.8))
            .add_rank_effect(SkillEffect::Damage { base_damage: 60.0, ratio: 1.9, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(22.0, 3.5))
            .add_rank_effect(SkillEffect::Damage { base_damage: 80.0, ratio: 2.1, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(25.0, 3.2))
            .add_rank_effect(SkillEffect::Damage { base_damage: 100.0, ratio: 2.5, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(30.0, 3.0))
            .add_tag("melee");

        let whirlwind = Skill::new(SkillId(1002), "Whirlwind", SkillType::Active)
            .with_description("Spin and deal AoE damage to all nearby enemies.")
            .with_icon('✦')
            .with_max_rank(5)
            .add_requirement(SkillRequirement::SkillRank { skill_id: SkillId(1001), min_rank: 2 })
            .add_rank_effect(SkillEffect::Damage { base_damage: 20.0, ratio: 1.0, element: Element::Physical, aoe_radius: 3.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(35.0, 8.0))
            .add_rank_effect(SkillEffect::Damage { base_damage: 30.0, ratio: 1.2, element: Element::Physical, aoe_radius: 3.5, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(35.0, 7.5))
            .add_rank_effect(SkillEffect::Damage { base_damage: 45.0, ratio: 1.4, element: Element::Physical, aoe_radius: 4.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(38.0, 7.0))
            .add_rank_effect(SkillEffect::Damage { base_damage: 60.0, ratio: 1.6, element: Element::Physical, aoe_radius: 4.5, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(40.0, 6.5))
            .add_rank_effect(SkillEffect::Damage { base_damage: 80.0, ratio: 2.0, element: Element::Physical, aoe_radius: 5.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(45.0, 6.0))
            .add_tag("melee").add_tag("aoe");

        let battle_cry = Skill::new(SkillId(1003), "Battle Cry", SkillType::Active)
            .with_description("Rally allies, granting bonus attack for 30 seconds.")
            .with_icon('!')
            .with_max_rank(3)
            .add_rank_effect(SkillEffect::Buff {
                modifiers: vec![StatModifier::percent("battle_cry", StatKind::PhysicalAttack, 0.1)],
                duration_secs: 30.0,
                target: BuffTarget::AllAllies,
            })
            .add_rank_cost(SkillCost::stamina_cost(30.0, 60.0))
            .add_tag("support");

        let iron_skin = Skill::new(SkillId(1004), "Iron Skin", SkillType::Passive)
            .with_description("Passive increase to Defense.")
            .with_icon('Ω')
            .with_max_rank(5)
            .add_passive(StatModifier::flat("iron_skin", StatKind::Defense, 5.0));

        let berserker_rage = Skill::new(SkillId(1005), "Berserker Rage", SkillType::Toggle)
            .with_description("Enter a rage state: more damage, less defense.")
            .with_icon('Ψ')
            .with_max_rank(1)
            .add_requirement(SkillRequirement::Level(10))
            .add_rank_effect(SkillEffect::Composite(vec![
                SkillEffect::Buff {
                    modifiers: vec![StatModifier::percent("berserk", StatKind::PhysicalAttack, 0.3)],
                    duration_secs: f32::MAX,
                    target: BuffTarget::Self_,
                },
                SkillEffect::Debuff {
                    modifiers: vec![StatModifier::percent("berserk_def", StatKind::Defense, -0.2)],
                    duration_secs: f32::MAX,
                    target: BuffTarget::Self_,
                },
            ]))
            .add_rank_cost(SkillCost::free());

        SkillTree::new("Warrior")
            .add_node(SkillNode::new(slash, (2, 0)))
            .add_node(SkillNode::new(whirlwind, (2, 1)).with_prereqs(vec![0]))
            .add_node(SkillNode::new(battle_cry, (1, 1)))
            .add_node(SkillNode::new(iron_skin, (3, 0)))
            .add_node(SkillNode::new(berserker_rage, (2, 2)).with_prereqs(vec![1]))
            .add_connection(0, 1)
            .add_connection(1, 4)
    }

    pub fn mage_tree() -> SkillTree {
        let fireball = Skill::new(SkillId(2001), "Fireball", SkillType::Active)
            .with_description("Hurl a flaming orb at your enemies.")
            .with_icon('o')
            .with_max_rank(5)
            .add_rank_effect(SkillEffect::Projectile { speed: 15.0, pierce_count: 0, split_count: 0, element: Element::Fire, damage: 40.0 })
            .add_rank_cost(SkillCost::mana_cost(25.0, 3.0).with_cast_time(0.8))
            .add_rank_effect(SkillEffect::Projectile { speed: 15.0, pierce_count: 0, split_count: 0, element: Element::Fire, damage: 60.0 })
            .add_rank_cost(SkillCost::mana_cost(25.0, 2.8).with_cast_time(0.75))
            .add_rank_effect(SkillEffect::Projectile { speed: 17.0, pierce_count: 0, split_count: 1, element: Element::Fire, damage: 80.0 })
            .add_rank_cost(SkillCost::mana_cost(28.0, 2.5).with_cast_time(0.7))
            .add_rank_effect(SkillEffect::Projectile { speed: 17.0, pierce_count: 0, split_count: 1, element: Element::Fire, damage: 100.0 })
            .add_rank_cost(SkillCost::mana_cost(30.0, 2.3).with_cast_time(0.65))
            .add_rank_effect(SkillEffect::Projectile { speed: 20.0, pierce_count: 0, split_count: 2, element: Element::Fire, damage: 130.0 })
            .add_rank_cost(SkillCost::mana_cost(35.0, 2.0).with_cast_time(0.6))
            .add_tag("fire").add_tag("ranged");

        let ice_shard = Skill::new(SkillId(2002), "Ice Shard", SkillType::Active)
            .with_description("Launch a shard of ice that pierces through enemies.")
            .with_icon('*')
            .with_max_rank(5)
            .add_rank_effect(SkillEffect::Projectile { speed: 20.0, pierce_count: 2, split_count: 0, element: Element::Ice, damage: 30.0 })
            .add_rank_cost(SkillCost::mana_cost(20.0, 2.5))
            .add_rank_effect(SkillEffect::Projectile { speed: 20.0, pierce_count: 3, split_count: 0, element: Element::Ice, damage: 45.0 })
            .add_rank_cost(SkillCost::mana_cost(22.0, 2.3))
            .add_rank_effect(SkillEffect::Projectile { speed: 22.0, pierce_count: 3, split_count: 0, element: Element::Ice, damage: 60.0 })
            .add_rank_cost(SkillCost::mana_cost(24.0, 2.1))
            .add_rank_effect(SkillEffect::Projectile { speed: 22.0, pierce_count: 4, split_count: 0, element: Element::Ice, damage: 80.0 })
            .add_rank_cost(SkillCost::mana_cost(26.0, 1.9))
            .add_rank_effect(SkillEffect::Projectile { speed: 25.0, pierce_count: 5, split_count: 0, element: Element::Ice, damage: 100.0 })
            .add_rank_cost(SkillCost::mana_cost(30.0, 1.7))
            .add_tag("ice").add_tag("ranged");

        let arcane_shield = Skill::new(SkillId(2003), "Arcane Shield", SkillType::Active)
            .with_description("Create a barrier that absorbs incoming damage.")
            .with_icon('Ω')
            .with_max_rank(3)
            .add_rank_effect(SkillEffect::Shield { absorb_amount: 100.0, duration_secs: 10.0 })
            .add_rank_cost(SkillCost::mana_cost(40.0, 30.0).with_cast_time(0.5))
            .add_rank_effect(SkillEffect::Shield { absorb_amount: 175.0, duration_secs: 12.0 })
            .add_rank_cost(SkillCost::mana_cost(40.0, 28.0).with_cast_time(0.4))
            .add_rank_effect(SkillEffect::Shield { absorb_amount: 280.0, duration_secs: 15.0 })
            .add_rank_cost(SkillCost::mana_cost(45.0, 25.0).with_cast_time(0.3));

        let mana_mastery = Skill::new(SkillId(2004), "Mana Mastery", SkillType::Passive)
            .with_description("Reduces mana cost of all spells.")
            .with_icon('M')
            .with_max_rank(5)
            .add_passive(StatModifier::percent("mana_mastery", StatKind::MaxMp, 0.05));

        let blink = Skill::new(SkillId(2005), "Blink", SkillType::Active)
            .with_description("Instantly teleport a short distance.")
            .with_icon('→')
            .with_max_rank(3)
            .add_requirement(SkillRequirement::Level(8))
            .add_rank_effect(SkillEffect::Teleport { range: 8.0, blink: true })
            .add_rank_cost(SkillCost::mana_cost(30.0, 15.0))
            .add_rank_effect(SkillEffect::Teleport { range: 12.0, blink: true })
            .add_rank_cost(SkillCost::mana_cost(28.0, 12.0))
            .add_rank_effect(SkillEffect::Teleport { range: 16.0, blink: true })
            .add_rank_cost(SkillCost::mana_cost(25.0, 10.0));

        let chain_lightning = Skill::new(SkillId(2006), "Chain Lightning", SkillType::Active)
            .with_description("Lightning that jumps between enemies.")
            .with_icon('~')
            .with_max_rank(5)
            .add_requirement(SkillRequirement::Level(15))
            .add_rank_effect(SkillEffect::Chain { max_targets: 3, jump_range: 5.0, damage_reduction: 0.2, element: Element::Lightning, base_damage: 50.0 })
            .add_rank_cost(SkillCost::mana_cost(45.0, 8.0).with_cast_time(1.0))
            .add_rank_effect(SkillEffect::Chain { max_targets: 4, jump_range: 5.5, damage_reduction: 0.18, element: Element::Lightning, base_damage: 70.0 })
            .add_rank_cost(SkillCost::mana_cost(47.0, 7.5).with_cast_time(0.9))
            .add_rank_effect(SkillEffect::Chain { max_targets: 5, jump_range: 6.0, damage_reduction: 0.15, element: Element::Lightning, base_damage: 90.0 })
            .add_rank_cost(SkillCost::mana_cost(50.0, 7.0).with_cast_time(0.8))
            .add_rank_effect(SkillEffect::Chain { max_targets: 6, jump_range: 6.5, damage_reduction: 0.12, element: Element::Lightning, base_damage: 115.0 })
            .add_rank_cost(SkillCost::mana_cost(53.0, 6.5).with_cast_time(0.75))
            .add_rank_effect(SkillEffect::Chain { max_targets: 8, jump_range: 7.0, damage_reduction: 0.10, element: Element::Lightning, base_damage: 145.0 })
            .add_rank_cost(SkillCost::mana_cost(58.0, 6.0).with_cast_time(0.7))
            .add_tag("lightning").add_tag("aoe");

        SkillTree::new("Mage")
            .add_node(SkillNode::new(fireball, (1, 0)))
            .add_node(SkillNode::new(ice_shard, (3, 0)))
            .add_node(SkillNode::new(arcane_shield, (2, 0)))
            .add_node(SkillNode::new(mana_mastery, (2, 1)))
            .add_node(SkillNode::new(blink, (2, 2)))
            .add_node(SkillNode::new(chain_lightning, (1, 2)).with_prereqs(vec![0]))
            .add_connection(0, 5)
            .add_connection(2, 3)
            .add_connection(3, 4)
    }

    pub fn rogue_tree() -> SkillTree {
        let backstab = Skill::new(SkillId(3001), "Backstab", SkillType::Active)
            .with_description("Deal massive damage from stealth or behind the target.")
            .with_icon('↑')
            .with_max_rank(5)
            .add_rank_effect(SkillEffect::Damage { base_damage: 50.0, ratio: 2.0, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(25.0, 6.0))
            .add_rank_effect(SkillEffect::Damage { base_damage: 70.0, ratio: 2.3, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(25.0, 5.5))
            .add_rank_effect(SkillEffect::Damage { base_damage: 95.0, ratio: 2.6, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(27.0, 5.0))
            .add_rank_effect(SkillEffect::Damage { base_damage: 125.0, ratio: 3.0, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(28.0, 4.5))
            .add_rank_effect(SkillEffect::Damage { base_damage: 160.0, ratio: 3.5, element: Element::Physical, aoe_radius: 0.0, pierces: false })
            .add_rank_cost(SkillCost::stamina_cost(30.0, 4.0))
            .add_tag("melee").add_tag("stealth");

        let poison_blade = Skill::new(SkillId(3002), "Poison Blade", SkillType::Active)
            .with_description("Coat your blade in poison, applying DoT.")
            .with_icon('¥')
            .with_max_rank(5)
            .add_rank_effect(SkillEffect::Zone {
                radius: 0.0,
                duration_secs: 10.0,
                tick_interval: 1.0,
                tick_effect: Box::new(SkillEffect::Damage { base_damage: 8.0, ratio: 0.3, element: Element::Poison, aoe_radius: 0.0, pierces: false }),
            })
            .add_rank_cost(SkillCost::stamina_cost(15.0, 12.0))
            .add_tag("poison");

        let evasion_skill = Skill::new(SkillId(3003), "Evasion", SkillType::Passive)
            .with_description("Permanently increases evasion.")
            .with_icon('E')
            .with_max_rank(5)
            .add_passive(StatModifier::flat("evasion_skill", StatKind::Evasion, 3.0));

        let shadowstep = Skill::new(SkillId(3004), "Shadowstep", SkillType::Active)
            .with_description("Teleport behind a target enemy.")
            .with_icon('↓')
            .with_max_rank(3)
            .add_requirement(SkillRequirement::Level(12))
            .add_rank_effect(SkillEffect::Teleport { range: 6.0, blink: true })
            .add_rank_cost(SkillCost::stamina_cost(35.0, 20.0))
            .add_tag("movement");

        SkillTree::new("Rogue")
            .add_node(SkillNode::new(backstab, (2, 0)))
            .add_node(SkillNode::new(poison_blade, (1, 0)))
            .add_node(SkillNode::new(evasion_skill, (3, 0)))
            .add_node(SkillNode::new(shadowstep, (2, 1)).with_prereqs(vec![0]))
            .add_connection(0, 3)
    }

    pub fn healer_tree() -> SkillTree {
        let holy_light = Skill::new(SkillId(4001), "Holy Light", SkillType::Active)
            .with_description("Call down a beam of healing light on a target.")
            .with_icon('+')
            .with_max_rank(5)
            .add_rank_effect(SkillEffect::Heal { base_heal: 60.0, ratio: 1.5, target: HealTarget::SingleAlly })
            .add_rank_cost(SkillCost::mana_cost(30.0, 4.0).with_cast_time(1.0))
            .add_rank_effect(SkillEffect::Heal { base_heal: 90.0, ratio: 1.7, target: HealTarget::SingleAlly })
            .add_rank_cost(SkillCost::mana_cost(30.0, 3.8).with_cast_time(0.9))
            .add_rank_effect(SkillEffect::Heal { base_heal: 120.0, ratio: 2.0, target: HealTarget::SingleAlly })
            .add_rank_cost(SkillCost::mana_cost(32.0, 3.5).with_cast_time(0.8))
            .add_rank_effect(SkillEffect::Heal { base_heal: 160.0, ratio: 2.3, target: HealTarget::SingleAlly })
            .add_rank_cost(SkillCost::mana_cost(35.0, 3.3).with_cast_time(0.7))
            .add_rank_effect(SkillEffect::Heal { base_heal: 200.0, ratio: 2.8, target: HealTarget::SingleAlly })
            .add_rank_cost(SkillCost::mana_cost(38.0, 3.0).with_cast_time(0.6));

        let renew = Skill::new(SkillId(4002), "Renew", SkillType::Active)
            .with_description("Apply a regeneration aura to an ally.")
            .with_icon('R')
            .with_max_rank(5)
            .add_rank_effect(SkillEffect::Zone {
                radius: 0.0,
                duration_secs: 15.0,
                tick_interval: 1.0,
                tick_effect: Box::new(SkillEffect::Heal { base_heal: 10.0, ratio: 0.2, target: HealTarget::SingleAlly }),
            })
            .add_rank_cost(SkillCost::mana_cost(20.0, 15.0));

        let mass_heal = Skill::new(SkillId(4003), "Mass Heal", SkillType::Active)
            .with_description("Heal all allies in range.")
            .with_icon('H')
            .with_max_rank(3)
            .add_requirement(SkillRequirement::Level(15))
            .add_rank_effect(SkillEffect::Heal { base_heal: 80.0, ratio: 1.2, target: HealTarget::AllAllies })
            .add_rank_cost(SkillCost::mana_cost(70.0, 30.0).with_cast_time(2.0))
            .add_rank_effect(SkillEffect::Heal { base_heal: 120.0, ratio: 1.5, target: HealTarget::AllAllies })
            .add_rank_cost(SkillCost::mana_cost(70.0, 28.0).with_cast_time(1.8))
            .add_rank_effect(SkillEffect::Heal { base_heal: 180.0, ratio: 2.0, target: HealTarget::AllAllies })
            .add_rank_cost(SkillCost::mana_cost(75.0, 25.0).with_cast_time(1.5));

        let divine_favor = Skill::new(SkillId(4004), "Divine Favor", SkillType::Passive)
            .with_description("Increases healing output.")
            .with_icon('†')
            .with_max_rank(5)
            .add_passive(StatModifier::flat("divine_favor", StatKind::Wisdom, 2.0));

        SkillTree::new("Healer")
            .add_node(SkillNode::new(holy_light, (2, 0)))
            .add_node(SkillNode::new(renew, (1, 0)))
            .add_node(SkillNode::new(mass_heal, (2, 1)).with_prereqs(vec![0]))
            .add_node(SkillNode::new(divine_favor, (3, 0)))
            .add_connection(0, 2)
    }

    pub fn summoner_tree() -> SkillTree {
        let summon_wolf = Skill::new(SkillId(5001), "Summon Wolf", SkillType::Active)
            .with_description("Summon a wolf companion to fight for you.")
            .with_icon('W')
            .with_max_rank(5)
            .add_rank_effect(SkillEffect::Summon { entity_type: "wolf".to_string(), count: 1, duration_secs: 60.0 })
            .add_rank_cost(SkillCost::mana_cost(50.0, 30.0).with_cast_time(2.0))
            .add_rank_effect(SkillEffect::Summon { entity_type: "wolf".to_string(), count: 1, duration_secs: 90.0 })
            .add_rank_cost(SkillCost::mana_cost(50.0, 28.0).with_cast_time(1.8))
            .add_rank_effect(SkillEffect::Summon { entity_type: "dire_wolf".to_string(), count: 1, duration_secs: 120.0 })
            .add_rank_cost(SkillCost::mana_cost(55.0, 25.0).with_cast_time(1.5))
            .add_rank_effect(SkillEffect::Summon { entity_type: "dire_wolf".to_string(), count: 2, duration_secs: 150.0 })
            .add_rank_cost(SkillCost::mana_cost(60.0, 23.0).with_cast_time(1.3))
            .add_rank_effect(SkillEffect::Summon { entity_type: "shadow_wolf".to_string(), count: 2, duration_secs: 180.0 })
            .add_rank_cost(SkillCost::mana_cost(70.0, 20.0).with_cast_time(1.0));

        let summon_golem = Skill::new(SkillId(5002), "Summon Stone Golem", SkillType::Active)
            .with_description("Summon a powerful stone golem tank.")
            .with_icon('G')
            .with_max_rank(3)
            .add_requirement(SkillRequirement::Level(10))
            .add_rank_effect(SkillEffect::Summon { entity_type: "stone_golem".to_string(), count: 1, duration_secs: 120.0 })
            .add_rank_cost(SkillCost::mana_cost(80.0, 60.0).with_cast_time(3.0))
            .add_rank_effect(SkillEffect::Summon { entity_type: "iron_golem".to_string(), count: 1, duration_secs: 150.0 })
            .add_rank_cost(SkillCost::mana_cost(85.0, 55.0).with_cast_time(2.5))
            .add_rank_effect(SkillEffect::Summon { entity_type: "crystal_golem".to_string(), count: 1, duration_secs: 200.0 })
            .add_rank_cost(SkillCost::mana_cost(90.0, 50.0).with_cast_time(2.0));

        let bond = Skill::new(SkillId(5003), "Empathic Bond", SkillType::Passive)
            .with_description("Your summons gain more of your stats.")
            .with_icon('∞')
            .with_max_rank(5)
            .add_passive(StatModifier::flat("bond", StatKind::Charisma, 3.0));

        SkillTree::new("Summoner")
            .add_node(SkillNode::new(summon_wolf, (1, 0)))
            .add_node(SkillNode::new(summon_golem, (3, 0)))
            .add_node(SkillNode::new(bond, (2, 0)))
            .add_connection(0, 1)
    }

    /// Build default combos for a warrior
    pub fn warrior_combos() -> Vec<Combo> {
        vec![
            Combo::new(
                "Unstoppable Force",
                vec![SkillId(1001), SkillId(1001), SkillId(1002)],
                SkillEffect::Damage { base_damage: 200.0, ratio: 3.0, element: Element::Physical, aoe_radius: 5.0, pierces: true },
                2000.0,
            ),
            Combo::new(
                "Warcry Slash",
                vec![SkillId(1003), SkillId(1001)],
                SkillEffect::Damage { base_damage: 50.0, ratio: 1.5, element: Element::Physical, aoe_radius: 0.0, pierces: false },
                1500.0,
            ),
        ]
    }

    pub fn mage_combos() -> Vec<Combo> {
        vec![
            Combo::new(
                "Frozen Inferno",
                vec![SkillId(2002), SkillId(2001)],
                SkillEffect::Damage { base_damage: 120.0, ratio: 2.5, element: Element::Arcane, aoe_radius: 3.0, pierces: false },
                1500.0,
            ),
        ]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_id_equality() {
        assert_eq!(SkillId(1), SkillId(1));
        assert_ne!(SkillId(1), SkillId(2));
    }

    #[test]
    fn test_skill_effect_at_rank() {
        let tree = SkillPresets::warrior_tree();
        let node = &tree.nodes[0]; // Power Slash
        let eff = node.skill.effect_at_rank(1);
        assert!(eff.is_some());
        assert!(eff.unwrap().is_damaging());
    }

    #[test]
    fn test_skill_book_learn() {
        let mut book = SkillBook::new();
        let skill = Skill::new(SkillId(1), "Test", SkillType::Active);
        assert!(book.learn(skill.clone()));
        assert!(!book.learn(skill)); // Can't learn twice
    }

    #[test]
    fn test_skill_book_upgrade() {
        let mut book = SkillBook::new();
        let skill = Skill::new(SkillId(1), "Test", SkillType::Active).with_max_rank(3);
        book.learn(skill);
        assert!(book.upgrade(SkillId(1)));
        assert_eq!(book.rank_of(SkillId(1)), 2);
    }

    #[test]
    fn test_skill_book_max_rank() {
        let mut book = SkillBook::new();
        let skill = Skill::new(SkillId(1), "Test", SkillType::Active).with_max_rank(1);
        book.learn(skill);
        assert!(!book.upgrade(SkillId(1))); // Already at max rank 1
    }

    #[test]
    fn test_ability_bar_bind_unbind() {
        let mut bar = AbilityBar::new();
        bar.bind(0, Ability::new(SkillId(1), 0));
        assert!(bar.get(0).is_some());
        bar.unbind(0);
        assert!(bar.get(0).is_none());
    }

    #[test]
    fn test_cooldown_tracker() {
        let mut tracker = CooldownTracker::new();
        tracker.start(SkillId(1), 5.0);
        assert!(!tracker.is_ready(SkillId(1)));
        tracker.tick(3.0);
        assert!((tracker.remaining(SkillId(1)) - 2.0).abs() < 0.001);
        tracker.tick(2.0);
        assert!(tracker.is_ready(SkillId(1)));
    }

    #[test]
    fn test_cooldown_tracker_reduce() {
        let mut tracker = CooldownTracker::new();
        tracker.start(SkillId(1), 10.0);
        tracker.reduce(SkillId(1), 5.0);
        assert!((tracker.remaining(SkillId(1)) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_combo_matches() {
        let combo = Combo::new(
            "Test",
            vec![SkillId(1), SkillId(2), SkillId(3)],
            SkillEffect::Damage { base_damage: 100.0, ratio: 1.0, element: Element::Physical, aoe_radius: 0.0, pierces: false },
            2000.0,
        );
        let seq = vec![SkillId(5), SkillId(1), SkillId(2), SkillId(3)];
        assert!(combo.matches(&seq));
        let bad_seq = vec![SkillId(1), SkillId(2)];
        assert!(!combo.matches(&bad_seq));
    }

    #[test]
    fn test_combo_system_detects_combo() {
        let mut sys = ComboSystem::new();
        sys.add_combo(Combo::new(
            "Test",
            vec![SkillId(1001), SkillId(1002)],
            SkillEffect::Damage { base_damage: 50.0, ratio: 1.0, element: Element::Physical, aoe_radius: 0.0, pierces: false },
            2000.0,
        ));
        sys.register_skill_use(SkillId(1001));
        sys.register_skill_use(SkillId(1002));
        let found = sys.check_combos();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_skill_tree_warrior_available() {
        let tree = SkillPresets::warrior_tree();
        let available = tree.available_nodes();
        assert!(!available.is_empty());
    }

    #[test]
    fn test_skill_tree_total_points() {
        let tree = SkillPresets::mage_tree();
        assert_eq!(tree.total_points_spent(), 0);
    }
}
