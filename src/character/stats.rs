// src/character/stats.rs
// Character stats, leveling, resource pools, and modifiers.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// StatKind — 30+ distinct statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatKind {
    // Primary
    Strength,
    Dexterity,
    Intelligence,
    Vitality,
    Wisdom,
    Charisma,
    Luck,
    Constitution,
    Agility,
    Endurance,
    Perception,
    Willpower,
    // Combat derived (also addressable directly for bonuses)
    MaxHp,
    MaxMp,
    MaxStamina,
    PhysicalAttack,
    MagicalAttack,
    Defense,
    MagicResist,
    Speed,
    CritChance,
    CritMultiplier,
    Evasion,
    Accuracy,
    BlockChance,
    ArmorPenetration,
    MagicPenetration,
    // Utility
    MoveSpeed,
    AttackSpeed,
    CastSpeed,
    LifeSteal,
    ManaSteal,
    Tenacity,
    CooldownReduction,
    GoldFind,
    MagicFind,
    ExpBonus,
    Thorns,
    Regeneration,
    ManaRegen,
}

impl StatKind {
    pub fn all_primary() -> &'static [StatKind] {
        &[
            StatKind::Strength,
            StatKind::Dexterity,
            StatKind::Intelligence,
            StatKind::Vitality,
            StatKind::Wisdom,
            StatKind::Charisma,
            StatKind::Luck,
            StatKind::Constitution,
            StatKind::Agility,
            StatKind::Endurance,
            StatKind::Perception,
            StatKind::Willpower,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            StatKind::Strength => "Strength",
            StatKind::Dexterity => "Dexterity",
            StatKind::Intelligence => "Intelligence",
            StatKind::Vitality => "Vitality",
            StatKind::Wisdom => "Wisdom",
            StatKind::Charisma => "Charisma",
            StatKind::Luck => "Luck",
            StatKind::Constitution => "Constitution",
            StatKind::Agility => "Agility",
            StatKind::Endurance => "Endurance",
            StatKind::Perception => "Perception",
            StatKind::Willpower => "Willpower",
            StatKind::MaxHp => "Max HP",
            StatKind::MaxMp => "Max MP",
            StatKind::MaxStamina => "Max Stamina",
            StatKind::PhysicalAttack => "Physical Attack",
            StatKind::MagicalAttack => "Magical Attack",
            StatKind::Defense => "Defense",
            StatKind::MagicResist => "Magic Resist",
            StatKind::Speed => "Speed",
            StatKind::CritChance => "Crit Chance",
            StatKind::CritMultiplier => "Crit Multiplier",
            StatKind::Evasion => "Evasion",
            StatKind::Accuracy => "Accuracy",
            StatKind::BlockChance => "Block Chance",
            StatKind::ArmorPenetration => "Armor Penetration",
            StatKind::MagicPenetration => "Magic Penetration",
            StatKind::MoveSpeed => "Move Speed",
            StatKind::AttackSpeed => "Attack Speed",
            StatKind::CastSpeed => "Cast Speed",
            StatKind::LifeSteal => "Life Steal",
            StatKind::ManaSteal => "Mana Steal",
            StatKind::Tenacity => "Tenacity",
            StatKind::CooldownReduction => "Cooldown Reduction",
            StatKind::GoldFind => "Gold Find",
            StatKind::MagicFind => "Magic Find",
            StatKind::ExpBonus => "EXP Bonus",
            StatKind::Thorns => "Thorns",
            StatKind::Regeneration => "HP Regeneration",
            StatKind::ManaRegen => "MP Regeneration",
        }
    }
}

// ---------------------------------------------------------------------------
// StatValue — a single stat with layered bonuses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct StatValue {
    pub base: f32,
    pub flat_bonus: f32,
    pub percent_bonus: f32,
    pub multiplier: f32,
}

impl StatValue {
    pub fn new(base: f32) -> Self {
        Self {
            base,
            flat_bonus: 0.0,
            percent_bonus: 0.0,
            multiplier: 1.0,
        }
    }

    /// Final = (base + flat_bonus) * (1 + percent_bonus) * multiplier
    pub fn final_value(&self) -> f32 {
        (self.base + self.flat_bonus) * (1.0 + self.percent_bonus) * self.multiplier
    }

    pub fn reset_bonuses(&mut self) {
        self.flat_bonus = 0.0;
        self.percent_bonus = 0.0;
        self.multiplier = 1.0;
    }
}

impl Default for StatValue {
    fn default() -> Self {
        Self::new(0.0)
    }
}

// ---------------------------------------------------------------------------
// ModifierKind + StatModifier
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ModifierKind {
    FlatAdd,
    PercentAdd,
    FlatMult,
    Override,
}

#[derive(Debug, Clone)]
pub struct StatModifier {
    pub source: String,
    pub stat: StatKind,
    pub value: f32,
    pub kind: ModifierKind,
}

impl StatModifier {
    pub fn flat(source: impl Into<String>, stat: StatKind, value: f32) -> Self {
        Self { source: source.into(), stat, value, kind: ModifierKind::FlatAdd }
    }

    pub fn percent(source: impl Into<String>, stat: StatKind, value: f32) -> Self {
        Self { source: source.into(), stat, value, kind: ModifierKind::PercentAdd }
    }

    pub fn mult(source: impl Into<String>, stat: StatKind, value: f32) -> Self {
        Self { source: source.into(), stat, value, kind: ModifierKind::FlatMult }
    }

    pub fn override_val(source: impl Into<String>, stat: StatKind, value: f32) -> Self {
        Self { source: source.into(), stat, value, kind: ModifierKind::Override }
    }
}

// ---------------------------------------------------------------------------
// ModifierRegistry — tracks all active modifiers and recomputes stats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ModifierRegistry {
    modifiers: Vec<StatModifier>,
}

impl ModifierRegistry {
    pub fn new() -> Self {
        Self { modifiers: Vec::new() }
    }

    pub fn add(&mut self, modifier: StatModifier) {
        self.modifiers.push(modifier);
    }

    pub fn remove_by_source(&mut self, source: &str) {
        self.modifiers.retain(|m| m.source != source);
    }

    pub fn remove_by_source_and_stat(&mut self, source: &str, stat: StatKind) {
        self.modifiers.retain(|m| !(m.source == source && m.stat == stat));
    }

    pub fn clear(&mut self) {
        self.modifiers.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &StatModifier> {
        self.modifiers.iter()
    }

    pub fn count(&self) -> usize {
        self.modifiers.len()
    }

    /// Apply all modifiers for a given stat to a StatValue.
    pub fn apply_to(&self, stat: StatKind, sv: &mut StatValue) {
        sv.reset_bonuses();
        let mut override_val: Option<f32> = None;
        for m in &self.modifiers {
            if m.stat != stat { continue; }
            match m.kind {
                ModifierKind::FlatAdd => sv.flat_bonus += m.value,
                ModifierKind::PercentAdd => sv.percent_bonus += m.value,
                ModifierKind::FlatMult => sv.multiplier *= m.value,
                ModifierKind::Override => override_val = Some(m.value),
            }
        }
        if let Some(ov) = override_val {
            sv.base = ov;
            sv.flat_bonus = 0.0;
            sv.percent_bonus = 0.0;
            sv.multiplier = 1.0;
        }
    }
}

// ---------------------------------------------------------------------------
// StatSheet — the full set of stats for one character
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StatSheet {
    pub stats: HashMap<StatKind, StatValue>,
}

impl StatSheet {
    pub fn new() -> Self {
        let mut stats = HashMap::new();
        // Initialise every primary stat to 10
        for &kind in StatKind::all_primary() {
            stats.insert(kind, StatValue::new(10.0));
        }
        Self { stats }
    }

    pub fn with_base(mut self, kind: StatKind, base: f32) -> Self {
        self.stats.insert(kind, StatValue::new(base));
        self
    }

    pub fn get(&self, kind: StatKind) -> f32 {
        self.stats.get(&kind).map(|sv| sv.final_value()).unwrap_or(0.0)
    }

    pub fn get_mut(&mut self, kind: StatKind) -> &mut StatValue {
        self.stats.entry(kind).or_insert_with(|| StatValue::new(0.0))
    }

    pub fn set_base(&mut self, kind: StatKind, base: f32) {
        self.stats.entry(kind).or_insert_with(|| StatValue::new(0.0)).base = base;
    }

    pub fn add_base(&mut self, kind: StatKind, delta: f32) {
        let sv = self.stats.entry(kind).or_insert_with(|| StatValue::new(0.0));
        sv.base += delta;
    }

    /// Reapply all modifiers from the registry.
    pub fn apply_modifiers(&mut self, registry: &ModifierRegistry) {
        // Collect keys first to avoid borrow conflicts
        let keys: Vec<StatKind> = self.stats.keys().copied().collect();
        for key in keys {
            if let Some(sv) = self.stats.get_mut(&key) {
                registry.apply_to(key, sv);
            }
        }
    }

    /// Compute derived stat: MaxHP
    pub fn max_hp(&self, level: u32) -> f32 {
        self.get(StatKind::Vitality) * 10.0
            + self.get(StatKind::Constitution) * 5.0
            + level as f32 * 20.0
    }

    /// Compute derived stat: MaxMP
    pub fn max_mp(&self, level: u32) -> f32 {
        self.get(StatKind::Intelligence) * 8.0
            + self.get(StatKind::Wisdom) * 4.0
            + level as f32 * 10.0
    }

    /// Compute derived stat: MaxStamina
    pub fn max_stamina(&self, level: u32) -> f32 {
        self.get(StatKind::Endurance) * 6.0
            + self.get(StatKind::Constitution) * 3.0
            + level as f32 * 5.0
    }

    /// Physical Attack (weapon_damage is passed in from equipment)
    pub fn physical_attack(&self, weapon_damage: f32) -> f32 {
        self.get(StatKind::Strength) * 2.0 + weapon_damage
    }

    /// Magical Attack (spell_power from equipment/skills)
    pub fn magical_attack(&self, spell_power: f32) -> f32 {
        self.get(StatKind::Intelligence) * 2.0 + spell_power
    }

    /// Defense
    pub fn defense(&self, armor_rating: f32) -> f32 {
        self.get(StatKind::Constitution) + armor_rating
    }

    /// Magic Resist
    pub fn magic_resist(&self, magic_armor: f32) -> f32 {
        self.get(StatKind::Willpower) * 0.5 + magic_armor
    }

    /// Speed
    pub fn speed(&self) -> f32 {
        self.get(StatKind::Dexterity) * 0.5 + self.get(StatKind::Agility) * 0.5
    }

    /// Crit chance (capped at 75%)
    pub fn crit_chance(&self) -> f32 {
        let raw = self.get(StatKind::Luck) * 0.1 + self.get(StatKind::Dexterity) * 0.05;
        raw.min(75.0)
    }

    /// Crit multiplier
    pub fn crit_multiplier(&self) -> f32 {
        1.5 + self.get(StatKind::Strength) * 0.01
    }

    /// Evasion
    pub fn evasion(&self) -> f32 {
        self.get(StatKind::Dexterity) * 0.3 + self.get(StatKind::Agility) * 0.2
    }

    /// Accuracy
    pub fn accuracy(&self) -> f32 {
        self.get(StatKind::Perception) * 0.5 + self.get(StatKind::Dexterity) * 0.2
    }

    /// Block chance (capped at 50%)
    pub fn block_chance(&self) -> f32 {
        let raw = self.get(StatKind::Constitution) * 0.1 + self.get(StatKind::Strength) * 0.05;
        raw.min(50.0)
    }

    /// HP regen per second
    pub fn hp_regen(&self) -> f32 {
        self.get(StatKind::Vitality) * 0.02 + self.get(StatKind::Regeneration)
    }

    /// MP regen per second
    pub fn mp_regen(&self) -> f32 {
        self.get(StatKind::Wisdom) * 0.05 + self.get(StatKind::ManaRegen)
    }

    /// Move speed (base 100 units/s)
    pub fn move_speed(&self) -> f32 {
        100.0 + self.get(StatKind::Agility) * 2.0 + self.get(StatKind::MoveSpeed)
    }

    /// Attack speed (1.0 = base, higher is faster)
    pub fn attack_speed(&self) -> f32 {
        1.0 + self.get(StatKind::Dexterity) * 0.01 + self.get(StatKind::AttackSpeed)
    }
}

impl Default for StatSheet {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ResourcePool — HP / MP / Stamina
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResourcePool {
    pub current: f32,
    pub max: f32,
    pub regen_rate: f32,
    /// Seconds after taking damage before regen resumes
    pub regen_delay: f32,
    regen_timer: f32,
}

impl ResourcePool {
    pub fn new(max: f32, regen_rate: f32, regen_delay: f32) -> Self {
        Self {
            current: max,
            max,
            regen_rate,
            regen_delay,
            regen_timer: 0.0,
        }
    }

    pub fn full(&self) -> bool {
        self.current >= self.max
    }

    pub fn empty(&self) -> bool {
        self.current <= 0.0
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 { 0.0 } else { (self.current / self.max).clamp(0.0, 1.0) }
    }

    /// Drain amount, returns actual amount drained (clamped to available).
    pub fn drain(&mut self, amount: f32) -> f32 {
        let drained = amount.min(self.current).max(0.0);
        self.current -= drained;
        self.regen_timer = self.regen_delay;
        drained
    }

    /// Restore amount, returns actual amount restored (clamped to max).
    pub fn restore(&mut self, amount: f32) -> f32 {
        let before = self.current;
        self.current = (self.current + amount).min(self.max);
        self.current - before
    }

    /// Set max and optionally scale current proportionally.
    pub fn set_max(&mut self, new_max: f32, scale_current: bool) {
        if scale_current && self.max > 0.0 {
            let ratio = self.current / self.max;
            self.max = new_max.max(1.0);
            self.current = (self.max * ratio).min(self.max);
        } else {
            self.max = new_max.max(1.0);
            self.current = self.current.min(self.max);
        }
    }

    /// Tick regeneration by dt seconds.
    pub fn tick(&mut self, dt: f32) {
        // Clamp active timer to current regen_delay so that lowering
        // regen_delay at runtime takes effect immediately.
        self.regen_timer = self.regen_timer.min(self.regen_delay);

        if self.regen_timer > 0.0 {
            self.regen_timer -= dt;
            if self.regen_timer >= 0.0 {
                return;
            }
            // Timer expired mid-tick — regen for the leftover time
            let leftover = -self.regen_timer;
            self.regen_timer = 0.0;
            if !self.full() {
                self.current = (self.current + self.regen_rate * leftover).min(self.max);
            }
            return;
        }
        if !self.full() {
            self.current = (self.current + self.regen_rate * dt).min(self.max);
        }
    }

    /// Force set current (clamps to [0, max]).
    pub fn set_current(&mut self, val: f32) {
        self.current = val.clamp(0.0, self.max);
    }

    /// Instant fill to max.
    pub fn fill(&mut self) {
        self.current = self.max;
    }
}

impl Default for ResourcePool {
    fn default() -> Self {
        Self::new(100.0, 1.0, 5.0)
    }
}

// ---------------------------------------------------------------------------
// XpCurve — experience requirements per level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum XpCurve {
    Linear { base: u64, increment: u64 },
    Quadratic { base: u64, factor: f64 },
    Exponential { base: u64, exponent: f64 },
    Custom(Vec<u64>),
}

impl XpCurve {
    pub fn xp_for_level(&self, level: u32) -> u64 {
        let lvl = level as u64;
        match self {
            XpCurve::Linear { base, increment } => base + increment * (lvl.saturating_sub(1)),
            XpCurve::Quadratic { base, factor } => {
                (*base as f64 * (*factor).powf(lvl as f64 - 1.0)) as u64
            }
            XpCurve::Exponential { base, exponent } => {
                (*base as f64 * (lvl as f64).powf(*exponent)) as u64
            }
            XpCurve::Custom(table) => {
                let idx = (level as usize).saturating_sub(1);
                table.get(idx).copied().unwrap_or(u64::MAX)
            }
        }
    }

    pub fn total_xp_to_level(&self, target_level: u32) -> u64 {
        (1..target_level).map(|l| self.xp_for_level(l)).sum()
    }
}

impl Default for XpCurve {
    fn default() -> Self {
        XpCurve::Quadratic { base: 100, factor: 1.5 }
    }
}

// ---------------------------------------------------------------------------
// LevelData — tracks XP and level progression
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LevelData {
    pub level: u32,
    pub xp: u64,
    pub xp_to_next: u64,
    pub stat_points: u32,
    pub skill_points: u32,
    pub curve: XpCurve,
    pub max_level: u32,
}

impl LevelData {
    pub fn new(curve: XpCurve, max_level: u32) -> Self {
        let xp_to_next = curve.xp_for_level(1);
        Self {
            level: 1,
            xp: 0,
            xp_to_next,
            stat_points: 0,
            skill_points: 0,
            curve,
            max_level,
        }
    }

    /// Add XP and return number of levels gained.
    pub fn add_xp(&mut self, amount: u64) -> u32 {
        if self.level >= self.max_level { return 0; }
        self.xp += amount;
        let mut levels_gained = 0u32;
        while self.level < self.max_level && self.xp >= self.xp_to_next {
            self.xp -= self.xp_to_next;
            self.level += 1;
            levels_gained += 1;
            self.xp_to_next = self.curve.xp_for_level(self.level);
        }
        if self.level >= self.max_level {
            self.xp = 0;
            self.xp_to_next = 0;
        }
        levels_gained
    }

    pub fn level_up(&mut self, stat_points_per_level: u32, skill_points_per_level: u32) {
        self.stat_points += stat_points_per_level;
        self.skill_points += skill_points_per_level;
    }

    pub fn spend_stat_point(&mut self) -> bool {
        if self.stat_points > 0 {
            self.stat_points -= 1;
            true
        } else {
            false
        }
    }

    pub fn spend_skill_point(&mut self) -> bool {
        if self.skill_points > 0 {
            self.skill_points -= 1;
            true
        } else {
            false
        }
    }

    pub fn xp_progress_fraction(&self) -> f32 {
        if self.xp_to_next == 0 { return 1.0; }
        (self.xp as f64 / self.xp_to_next as f64) as f32
    }
}

impl Default for LevelData {
    fn default() -> Self {
        Self::new(XpCurve::default(), 100)
    }
}

// ---------------------------------------------------------------------------
// StatGrowth — per-level stat increases for a class archetype
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StatGrowth {
    pub growths: HashMap<StatKind, f32>,
}

impl StatGrowth {
    pub fn new() -> Self {
        Self { growths: HashMap::new() }
    }

    pub fn set(mut self, kind: StatKind, per_level: f32) -> Self {
        self.growths.insert(kind, per_level);
        self
    }

    pub fn apply_to(&self, sheet: &mut StatSheet) {
        for (&kind, &amount) in &self.growths {
            sheet.add_base(kind, amount);
        }
    }

    /// Warrior growth template
    pub fn warrior() -> Self {
        Self::new()
            .set(StatKind::Strength, 3.0)
            .set(StatKind::Constitution, 2.0)
            .set(StatKind::Vitality, 2.0)
            .set(StatKind::Endurance, 1.5)
            .set(StatKind::Agility, 0.5)
            .set(StatKind::Dexterity, 1.0)
    }

    pub fn mage() -> Self {
        Self::new()
            .set(StatKind::Intelligence, 4.0)
            .set(StatKind::Wisdom, 2.5)
            .set(StatKind::Willpower, 2.0)
            .set(StatKind::Vitality, 1.0)
            .set(StatKind::Charisma, 0.5)
    }

    pub fn rogue() -> Self {
        Self::new()
            .set(StatKind::Dexterity, 3.5)
            .set(StatKind::Agility, 3.0)
            .set(StatKind::Perception, 2.0)
            .set(StatKind::Luck, 1.5)
            .set(StatKind::Strength, 1.0)
    }

    pub fn healer() -> Self {
        Self::new()
            .set(StatKind::Wisdom, 3.5)
            .set(StatKind::Intelligence, 2.0)
            .set(StatKind::Charisma, 2.5)
            .set(StatKind::Vitality, 2.0)
            .set(StatKind::Willpower, 1.5)
    }

    pub fn ranger() -> Self {
        Self::new()
            .set(StatKind::Dexterity, 3.0)
            .set(StatKind::Perception, 3.0)
            .set(StatKind::Agility, 2.0)
            .set(StatKind::Strength, 1.5)
            .set(StatKind::Endurance, 1.0)
    }
}

impl Default for StatGrowth {
    fn default() -> Self {
        Self::new()
            .set(StatKind::Vitality, 1.0)
            .set(StatKind::Strength, 1.0)
            .set(StatKind::Dexterity, 1.0)
    }
}

// ---------------------------------------------------------------------------
// StatPreset — predefined stat spreads for common archetypes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassArchetype {
    Warrior,
    Mage,
    Rogue,
    Healer,
    Ranger,
    Summoner,
    Paladin,
    Necromancer,
    Berserker,
    Elementalist,
}

pub struct StatPreset;

impl StatPreset {
    pub fn for_class(class: ClassArchetype, level: u32) -> StatSheet {
        let mut sheet = StatSheet::new();
        let growth = Self::growth_for(class);
        for _ in 1..level {
            growth.apply_to(&mut sheet);
        }
        // Set base spread
        match class {
            ClassArchetype::Warrior => {
                sheet.set_base(StatKind::Strength, 16.0);
                sheet.set_base(StatKind::Constitution, 14.0);
                sheet.set_base(StatKind::Vitality, 14.0);
                sheet.set_base(StatKind::Endurance, 13.0);
                sheet.set_base(StatKind::Intelligence, 8.0);
                sheet.set_base(StatKind::Wisdom, 8.0);
            }
            ClassArchetype::Mage => {
                sheet.set_base(StatKind::Intelligence, 18.0);
                sheet.set_base(StatKind::Wisdom, 14.0);
                sheet.set_base(StatKind::Willpower, 13.0);
                sheet.set_base(StatKind::Strength, 6.0);
                sheet.set_base(StatKind::Constitution, 8.0);
            }
            ClassArchetype::Rogue => {
                sheet.set_base(StatKind::Dexterity, 17.0);
                sheet.set_base(StatKind::Agility, 16.0);
                sheet.set_base(StatKind::Perception, 14.0);
                sheet.set_base(StatKind::Luck, 13.0);
                sheet.set_base(StatKind::Strength, 10.0);
            }
            ClassArchetype::Healer => {
                sheet.set_base(StatKind::Wisdom, 18.0);
                sheet.set_base(StatKind::Charisma, 15.0);
                sheet.set_base(StatKind::Intelligence, 13.0);
                sheet.set_base(StatKind::Vitality, 12.0);
                sheet.set_base(StatKind::Willpower, 12.0);
            }
            ClassArchetype::Ranger => {
                sheet.set_base(StatKind::Dexterity, 16.0);
                sheet.set_base(StatKind::Perception, 15.0);
                sheet.set_base(StatKind::Agility, 14.0);
                sheet.set_base(StatKind::Strength, 12.0);
                sheet.set_base(StatKind::Endurance, 12.0);
            }
            ClassArchetype::Summoner => {
                sheet.set_base(StatKind::Intelligence, 16.0);
                sheet.set_base(StatKind::Charisma, 17.0);
                sheet.set_base(StatKind::Wisdom, 13.0);
                sheet.set_base(StatKind::Willpower, 12.0);
            }
            ClassArchetype::Paladin => {
                sheet.set_base(StatKind::Strength, 14.0);
                sheet.set_base(StatKind::Constitution, 15.0);
                sheet.set_base(StatKind::Charisma, 13.0);
                sheet.set_base(StatKind::Wisdom, 12.0);
                sheet.set_base(StatKind::Vitality, 13.0);
            }
            ClassArchetype::Necromancer => {
                sheet.set_base(StatKind::Intelligence, 16.0);
                sheet.set_base(StatKind::Willpower, 15.0);
                sheet.set_base(StatKind::Wisdom, 12.0);
                sheet.set_base(StatKind::Charisma, 8.0);
                sheet.set_base(StatKind::Endurance, 11.0);
            }
            ClassArchetype::Berserker => {
                sheet.set_base(StatKind::Strength, 18.0);
                sheet.set_base(StatKind::Endurance, 16.0);
                sheet.set_base(StatKind::Vitality, 14.0);
                sheet.set_base(StatKind::Agility, 12.0);
                sheet.set_base(StatKind::Constitution, 10.0);
            }
            ClassArchetype::Elementalist => {
                sheet.set_base(StatKind::Intelligence, 17.0);
                sheet.set_base(StatKind::Wisdom, 15.0);
                sheet.set_base(StatKind::Agility, 12.0);
                sheet.set_base(StatKind::Perception, 11.0);
                sheet.set_base(StatKind::Willpower, 13.0);
            }
        }
        sheet
    }

    pub fn growth_for(class: ClassArchetype) -> StatGrowth {
        match class {
            ClassArchetype::Warrior => StatGrowth::warrior(),
            ClassArchetype::Mage => StatGrowth::mage(),
            ClassArchetype::Rogue => StatGrowth::rogue(),
            ClassArchetype::Healer => StatGrowth::healer(),
            ClassArchetype::Ranger => StatGrowth::ranger(),
            ClassArchetype::Summoner => StatGrowth::new()
                .set(StatKind::Intelligence, 2.5)
                .set(StatKind::Charisma, 3.0)
                .set(StatKind::Wisdom, 2.0),
            ClassArchetype::Paladin => StatGrowth::new()
                .set(StatKind::Strength, 2.0)
                .set(StatKind::Constitution, 2.5)
                .set(StatKind::Wisdom, 1.5)
                .set(StatKind::Vitality, 2.0),
            ClassArchetype::Necromancer => StatGrowth::new()
                .set(StatKind::Intelligence, 3.0)
                .set(StatKind::Willpower, 2.5)
                .set(StatKind::Wisdom, 1.5),
            ClassArchetype::Berserker => StatGrowth::new()
                .set(StatKind::Strength, 4.0)
                .set(StatKind::Endurance, 2.5)
                .set(StatKind::Vitality, 2.0)
                .set(StatKind::Agility, 1.0),
            ClassArchetype::Elementalist => StatGrowth::new()
                .set(StatKind::Intelligence, 3.5)
                .set(StatKind::Wisdom, 2.0)
                .set(StatKind::Agility, 1.5),
        }
    }
}

// ---------------------------------------------------------------------------
// AllResources — convenience wrapper for HP / MP / Stamina
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AllResources {
    pub hp: ResourcePool,
    pub mp: ResourcePool,
    pub stamina: ResourcePool,
}

impl AllResources {
    pub fn from_sheet(sheet: &StatSheet, level: u32) -> Self {
        let max_hp = sheet.max_hp(level);
        let max_mp = sheet.max_mp(level);
        let max_st = sheet.max_stamina(level);
        Self {
            hp: ResourcePool::new(max_hp, sheet.hp_regen(), 5.0),
            mp: ResourcePool::new(max_mp, sheet.mp_regen(), 3.0),
            stamina: ResourcePool::new(max_st, 10.0, 1.0),
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.hp.tick(dt);
        self.mp.tick(dt);
        self.stamina.tick(dt);
    }

    pub fn is_alive(&self) -> bool {
        self.hp.current > 0.0
    }
}

impl Default for AllResources {
    fn default() -> Self {
        Self {
            hp: ResourcePool::new(100.0, 1.0, 5.0),
            mp: ResourcePool::new(50.0, 2.0, 3.0),
            stamina: ResourcePool::new(100.0, 10.0, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stat_value_final() {
        let mut sv = StatValue::new(10.0);
        sv.flat_bonus = 5.0;
        sv.percent_bonus = 0.5; // +50%
        sv.multiplier = 2.0;
        // (10 + 5) * (1 + 0.5) * 2.0 = 15 * 1.5 * 2 = 45
        assert!((sv.final_value() - 45.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_modifier_registry_flat_add() {
        let mut reg = ModifierRegistry::new();
        reg.add(StatModifier::flat("sword", StatKind::Strength, 10.0));
        let mut sv = StatValue::new(20.0);
        reg.apply_to(StatKind::Strength, &mut sv);
        assert!((sv.final_value() - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_modifier_registry_remove_source() {
        let mut reg = ModifierRegistry::new();
        reg.add(StatModifier::flat("enchant", StatKind::Dexterity, 5.0));
        reg.remove_by_source("enchant");
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_modifier_override() {
        let mut reg = ModifierRegistry::new();
        reg.add(StatModifier::override_val("cap", StatKind::CritChance, 75.0));
        let mut sv = StatValue::new(99.0);
        reg.apply_to(StatKind::CritChance, &mut sv);
        assert!((sv.final_value() - 75.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_stat_sheet_derived_hp() {
        let sheet = StatPreset::for_class(ClassArchetype::Warrior, 1);
        let hp = sheet.max_hp(10);
        assert!(hp > 0.0);
    }

    #[test]
    fn test_crit_chance_cap() {
        let mut sheet = StatSheet::new();
        sheet.set_base(StatKind::Luck, 1000.0);
        assert!(sheet.crit_chance() <= 75.0);
    }

    #[test]
    fn test_resource_pool_drain_restore() {
        let mut pool = ResourcePool::new(100.0, 5.0, 0.0);
        let drained = pool.drain(30.0);
        assert!((drained - 30.0).abs() < f32::EPSILON);
        assert!((pool.current - 70.0).abs() < f32::EPSILON);
        let restored = pool.restore(20.0);
        assert!((restored - 20.0).abs() < f32::EPSILON);
        assert!((pool.current - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resource_pool_overflow() {
        let mut pool = ResourcePool::new(100.0, 5.0, 0.0);
        pool.restore(999.0);
        assert!((pool.current - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resource_pool_underflow() {
        let mut pool = ResourcePool::new(100.0, 5.0, 0.0);
        let drained = pool.drain(999.0);
        assert!((drained - 100.0).abs() < f32::EPSILON);
        assert!((pool.current).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resource_pool_regen() {
        let mut pool = ResourcePool::new(100.0, 10.0, 0.0);
        pool.drain(50.0);
        pool.tick(1.0);
        assert!((pool.current - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resource_pool_regen_delay() {
        let mut pool = ResourcePool::new(100.0, 10.0, 5.0);
        pool.drain(50.0);
        pool.tick(3.0); // still in delay
        assert!((pool.current - 50.0).abs() < f32::EPSILON);
        pool.tick(3.0); // delay expires, regen kicks in
        assert!(pool.current > 50.0);
    }

    #[test]
    fn test_xp_curve_quadratic() {
        let curve = XpCurve::Quadratic { base: 100, factor: 1.5 };
        let l1 = curve.xp_for_level(1);
        let l2 = curve.xp_for_level(2);
        assert!(l2 > l1);
    }

    #[test]
    fn test_level_data_add_xp() {
        let mut ld = LevelData::new(XpCurve::Linear { base: 100, increment: 50 }, 100);
        let levs = ld.add_xp(100);
        assert_eq!(levs, 1);
        assert_eq!(ld.level, 2);
    }

    #[test]
    fn test_level_data_multi_level() {
        let mut ld = LevelData::new(XpCurve::Linear { base: 10, increment: 0 }, 100);
        let levs = ld.add_xp(100);
        assert!(levs >= 10);
    }

    #[test]
    fn test_stat_preset_warrior() {
        let sheet = StatPreset::for_class(ClassArchetype::Warrior, 10);
        assert!(sheet.get(StatKind::Strength) > 10.0);
    }

    #[test]
    fn test_stat_sheet_apply_modifiers() {
        let mut sheet = StatSheet::new();
        let mut reg = ModifierRegistry::new();
        reg.add(StatModifier::flat("test", StatKind::Strength, 100.0));
        sheet.apply_modifiers(&reg);
        assert!(sheet.get(StatKind::Strength) > 100.0);
    }

    #[test]
    fn test_all_resources_tick() {
        let sheet = StatSheet::new();
        let mut res = AllResources::from_sheet(&sheet, 1);
        res.hp.drain(10.0);
        res.hp.regen_delay = 0.0;
        res.tick(1.0);
        // Should have regenerated some HP
        assert!(res.hp.current > res.hp.max - 10.0);
    }
}
