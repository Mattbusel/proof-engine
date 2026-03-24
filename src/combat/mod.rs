//! Combat system — damage, status effects, hit detection, and DPS calculation.
//!
//! The combat system is purely mathematical: no sprites, no hitboxes.
//! Damage is a function of attacker stats, defender stats, and environmental
//! entropy. Status effects are time-varying functions applied each tick.
//!
//! # Architecture
//!
//! - `DamageEvent`     — a single damage application with element and source
//! - `StatusEffect`    — a timed, stackable debuff/buff with per-tick function
//! - `CombatStats`     — attacker/defender stat block
//! - `HitResult`       — full damage resolution output
//! - `DpsTracker`      — rolling DPS measurement
//! - `CombatFormulas`  — all damage formulas in one place

use glam::Vec3;
use std::collections::HashMap;

// ── Element ────────────────────────────────────────────────────────────────────

/// Elemental type. Determines resistances, weakness multipliers, and visual effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Physical,
    Fire,
    Ice,
    Lightning,
    Void,
    Entropy,
    Gravity,
    Radiant,
    Shadow,
    Temporal,
}

impl Element {
    /// Default color for this element.
    pub fn color(self) -> glam::Vec4 {
        match self {
            Element::Physical   => glam::Vec4::new(0.85, 0.80, 0.75, 1.0),
            Element::Fire       => glam::Vec4::new(1.00, 0.40, 0.10, 1.0),
            Element::Ice        => glam::Vec4::new(0.50, 0.85, 1.00, 1.0),
            Element::Lightning  => glam::Vec4::new(1.00, 0.95, 0.20, 1.0),
            Element::Void       => glam::Vec4::new(0.20, 0.00, 0.40, 1.0),
            Element::Entropy    => glam::Vec4::new(0.60, 0.10, 0.80, 1.0),
            Element::Gravity    => glam::Vec4::new(0.30, 0.30, 0.60, 1.0),
            Element::Radiant    => glam::Vec4::new(1.00, 1.00, 0.70, 1.0),
            Element::Shadow     => glam::Vec4::new(0.10, 0.05, 0.20, 1.0),
            Element::Temporal   => glam::Vec4::new(0.40, 0.90, 0.70, 1.0),
        }
    }

    /// Glyph character used to represent this element in damage numbers.
    pub fn glyph(self) -> char {
        match self {
            Element::Physical   => '✦',
            Element::Fire       => '♨',
            Element::Ice        => '❄',
            Element::Lightning  => '⚡',
            Element::Void       => '◈',
            Element::Entropy    => '∞',
            Element::Gravity    => '⊕',
            Element::Radiant    => '☀',
            Element::Shadow     => '◆',
            Element::Temporal   => '⧗',
        }
    }
}

// ── ResistanceProfile ──────────────────────────────────────────────────────────

/// Per-element resistance multipliers for a combatant.
///
/// `1.0` = normal damage, `0.5` = 50% resistance, `2.0` = 200% (weakness),
/// `0.0` = immune, `-0.5` = healed by that element.
#[derive(Debug, Clone)]
pub struct ResistanceProfile {
    pub resistances: HashMap<Element, f32>,
}

impl ResistanceProfile {
    pub fn neutral() -> Self {
        let mut r = HashMap::new();
        for &el in &[
            Element::Physical, Element::Fire, Element::Ice, Element::Lightning,
            Element::Void, Element::Entropy, Element::Gravity, Element::Radiant,
            Element::Shadow, Element::Temporal,
        ] {
            r.insert(el, 1.0);
        }
        Self { resistances: r }
    }

    pub fn get(&self, el: Element) -> f32 {
        *self.resistances.get(&el).unwrap_or(&1.0)
    }

    pub fn set(&mut self, el: Element, value: f32) {
        self.resistances.insert(el, value);
    }

    /// Common preset: fire elemental — immune to fire, weak to ice.
    pub fn fire_elemental() -> Self {
        let mut p = Self::neutral();
        p.set(Element::Fire, 0.0);
        p.set(Element::Ice, 2.0);
        p.set(Element::Shadow, 1.3);
        p
    }

    /// Common preset: void entity — immune to void, weak to radiant.
    pub fn void_entity() -> Self {
        let mut p = Self::neutral();
        p.set(Element::Void, 0.0);
        p.set(Element::Radiant, 2.5);
        p.set(Element::Shadow, 0.3);
        p.set(Element::Physical, 0.5);
        p
    }

    /// Common preset: chaos rift — amplified by entropy, normal otherwise.
    pub fn chaos_rift() -> Self {
        let mut p = Self::neutral();
        p.set(Element::Entropy, 0.0);
        p.set(Element::Temporal, 0.0);
        p.set(Element::Physical, 0.3);
        p.set(Element::Gravity, 2.0);
        p
    }

    /// Boss resist profile: everything at half, but entropy at 1.5x.
    pub fn boss_resist() -> Self {
        let mut p = Self::neutral();
        for (_, v) in p.resistances.iter_mut() {
            *v *= 0.5;
        }
        p.set(Element::Entropy, 1.5);
        p
    }
}

// ── CombatStats ───────────────────────────────────────────────────────────────

/// Stat block for an attacker or defender.
#[derive(Debug, Clone)]
pub struct CombatStats {
    // Offensive
    pub attack:       f32,   // Base attack power
    pub crit_chance:  f32,   // [0, 1] probability of a critical hit
    pub crit_mult:    f32,   // Multiplier when critting (e.g., 2.0 = double damage)
    pub penetration:  f32,   // Armor penetration percentage [0, 1]
    pub entropy_amp:  f32,   // Amplifier from entropy field (1.0 = normal)

    // Defensive
    pub armor:        f32,   // Flat damage reduction
    pub dodge_chance: f32,   // [0, 1] probability of full miss
    pub block_chance: f32,   // [0, 1] probability of reducing damage by block_amount
    pub block_amount: f32,   // How much damage is absorbed on a successful block
    pub max_hp:       f32,
    pub hp:           f32,

    // Meta
    pub level:        u32,
    pub entropy:      f32,   // Current chaos level [0, 1] — affects entropy damage
}

impl Default for CombatStats {
    fn default() -> Self {
        Self {
            attack: 10.0, crit_chance: 0.05, crit_mult: 2.0, penetration: 0.0,
            entropy_amp: 1.0, armor: 5.0, dodge_chance: 0.05, block_chance: 0.0,
            block_amount: 0.0, max_hp: 100.0, hp: 100.0, level: 1, entropy: 0.0,
        }
    }
}

impl CombatStats {
    pub fn hp_fraction(&self) -> f32 {
        (self.hp / self.max_hp.max(1.0)).clamp(0.0, 1.0)
    }

    pub fn is_alive(&self) -> bool { self.hp > 0.0 }

    pub fn take_damage(&mut self, amount: f32) {
        self.hp = (self.hp - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.hp = (self.hp + amount).min(self.max_hp);
    }

    /// Effective armor after penetration applied.
    pub fn effective_armor(&self, penetration: f32) -> f32 {
        self.armor * (1.0 - penetration.clamp(0.0, 1.0))
    }
}

// ── DamageEvent ───────────────────────────────────────────────────────────────

/// A single damage application.
#[derive(Debug, Clone)]
pub struct DamageEvent {
    pub base_damage:    f32,
    pub element:        Element,
    pub attacker_pos:   Vec3,
    pub defender_pos:   Vec3,
    /// Applies a RNG seed for deterministic crit resolution.
    pub roll:           f32,  // [0, 1] — pre-rolled random value
}

// ── HitResult ─────────────────────────────────────────────────────────────────

/// Full result of damage resolution.
#[derive(Debug, Clone)]
pub struct HitResult {
    pub final_damage:   f32,
    pub is_crit:        bool,
    pub is_dodge:       bool,
    pub is_block:       bool,
    pub is_kill:        bool,
    pub element:        Element,
    pub pre_resist:     f32,   // damage before element resistance
    pub post_resist:    f32,   // damage after element resistance
    pub post_armor:     f32,   // damage after armor subtraction
    pub overkill:       f32,   // damage beyond remaining HP (0 if no kill)
}

impl HitResult {
    pub fn miss(element: Element) -> Self {
        Self {
            final_damage: 0.0, is_crit: false, is_dodge: true, is_block: false,
            is_kill: false, element, pre_resist: 0.0, post_resist: 0.0,
            post_armor: 0.0, overkill: 0.0,
        }
    }
}

// ── CombatFormulas ────────────────────────────────────────────────────────────

/// Stateless damage resolution functions.
pub struct CombatFormulas;

impl CombatFormulas {
    /// Resolve a single damage event, returning a `HitResult`.
    ///
    /// Uses the provided `roll` value (from attacker's entropy or RNG) instead of
    /// an actual RNG call, making the system fully deterministic and seedable.
    pub fn resolve(
        event: &DamageEvent,
        attacker: &CombatStats,
        defender: &CombatStats,
        resistances: &ResistanceProfile,
    ) -> HitResult {
        // ── Dodge check ───────────────────────────────────────────────────────
        if event.roll < defender.dodge_chance {
            return HitResult::miss(event.element);
        }

        // ── Crit check ────────────────────────────────────────────────────────
        let crit_roll = (event.roll * 1.61803) % 1.0; // different sample of roll
        let is_crit = crit_roll < attacker.crit_chance;
        let crit_factor = if is_crit { attacker.crit_mult } else { 1.0 };

        // ── Base damage ───────────────────────────────────────────────────────
        let base = event.base_damage * attacker.attack * crit_factor * attacker.entropy_amp;

        // ── Level scaling — higher level enemies get a natural armor bonus ───
        let level_armor = (defender.level as f32 - attacker.level as f32).max(0.0) * 2.0;
        let effective_armor = defender.effective_armor(attacker.penetration) + level_armor;

        // ── Element resistance ────────────────────────────────────────────────
        let resist = resistances.get(event.element);
        let post_resist = base * resist;

        // ── Block check ───────────────────────────────────────────────────────
        let block_roll = (event.roll * 2.71828) % 1.0;
        let is_block = block_roll < defender.block_chance;
        let post_block = if is_block {
            (post_resist - defender.block_amount).max(post_resist * 0.1)
        } else {
            post_resist
        };

        // ── Armor subtraction (multiplicative formula to prevent negatives) ──
        // Damage after armor = damage * armor_reduction_factor
        // armor_reduction_factor = 100 / (100 + armor)
        let armor_factor = 100.0 / (100.0 + effective_armor.max(0.0));
        let post_armor = (post_block * armor_factor).max(1.0); // minimum 1 damage

        // ── Kill check ────────────────────────────────────────────────────────
        let final_damage = post_armor;
        let is_kill = final_damage >= defender.hp;
        let overkill = if is_kill { final_damage - defender.hp } else { 0.0 };

        HitResult {
            final_damage,
            is_crit,
            is_dodge: false,
            is_block,
            is_kill,
            element: event.element,
            pre_resist: base,
            post_resist,
            post_armor,
            overkill,
        }
    }

    /// Splash damage — apply a full hit result at reduced strength to multiple targets.
    pub fn splash_damage(base_result: &HitResult, splash_radius: f32, distance: f32) -> f32 {
        let falloff = (1.0 - (distance / splash_radius.max(0.001))).max(0.0);
        base_result.final_damage * falloff * falloff
    }

    /// Entropy damage — additional chaos damage based on defender's current entropy.
    ///
    /// High-entropy targets take bonus damage; low-entropy targets are unaffected.
    pub fn entropy_damage(base_damage: f32, defender_entropy: f32, attacker_entropy_amp: f32) -> f32 {
        base_damage * defender_entropy * attacker_entropy_amp * 0.5
    }

    /// Gravity damage — scales with relative height difference between attacker and defender.
    pub fn gravity_damage(base_damage: f32, attacker_pos: Vec3, defender_pos: Vec3) -> f32 {
        let height_diff = (attacker_pos.y - defender_pos.y).max(0.0);
        base_damage * (1.0 + height_diff * 0.1)
    }

    /// Temporal damage — slows time for the defender based on damage dealt.
    /// Returns a slow factor in [0, 1] where 0 = full stop, 1 = normal.
    pub fn temporal_slow_factor(damage: f32, defender_max_hp: f32) -> f32 {
        let ratio = (damage / defender_max_hp.max(1.0)).min(1.0);
        (1.0 - ratio * 0.8).max(0.1)
    }

    /// Damage-per-second — useful for comparing damage sources.
    pub fn dps(damage_per_hit: f32, hits_per_second: f32, crit_chance: f32, crit_mult: f32) -> f32 {
        let avg_mult = 1.0 + crit_chance * (crit_mult - 1.0);
        damage_per_hit * hits_per_second * avg_mult
    }

    /// Effective HP — actual HP accounting for armor reduction.
    pub fn effective_hp(hp: f32, armor: f32) -> f32 {
        hp * (1.0 + armor / 100.0)
    }
}

// ── StatusEffect ──────────────────────────────────────────────────────────────

/// Type of status effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusKind {
    /// Deals fire damage each second.
    Burning,
    /// Slows movement and attack speed.
    Frozen,
    /// Applies random small damages each tick.
    Poisoned,
    /// Stuns the target — no actions possible.
    Stunned,
    /// Drains HP and transfers to attacker (vampiric).
    Cursed,
    /// Reduces armor.
    Corroded,
    /// Increases damage taken.
    Vulnerable,
    /// Reflects a portion of damage back to attacker.
    Thorned,
    /// Regenerates HP each second.
    Regenerating,
    /// Increases all stats temporarily.
    Enraged,
    /// Disables skill use.
    Silenced,
    /// Amplifies entropy field on target.
    Entropied,
    /// Slows time around the target.
    TemporalSnare,
    /// Gravity well — pulls nearby particles toward the target.
    GravityWell,
}

impl StatusKind {
    /// Is this effect a debuff (negative)?
    pub fn is_debuff(self) -> bool {
        !matches!(self, StatusKind::Regenerating | StatusKind::Enraged | StatusKind::Thorned)
    }

    /// Element associated with this status.
    pub fn element(self) -> Element {
        match self {
            StatusKind::Burning     => Element::Fire,
            StatusKind::Frozen      => Element::Ice,
            StatusKind::Poisoned    => Element::Physical,
            StatusKind::Stunned     => Element::Physical,
            StatusKind::Cursed      => Element::Shadow,
            StatusKind::Corroded    => Element::Physical,
            StatusKind::Vulnerable  => Element::Physical,
            StatusKind::Thorned     => Element::Physical,
            StatusKind::Regenerating => Element::Radiant,
            StatusKind::Enraged     => Element::Fire,
            StatusKind::Silenced    => Element::Void,
            StatusKind::Entropied   => Element::Entropy,
            StatusKind::TemporalSnare => Element::Temporal,
            StatusKind::GravityWell => Element::Gravity,
        }
    }

    /// Glyph shown on the target while this status is active.
    pub fn indicator_glyph(self) -> char {
        match self {
            StatusKind::Burning      => '🔥',
            StatusKind::Frozen       => '❄',
            StatusKind::Poisoned     => '☠',
            StatusKind::Stunned      => '★',
            StatusKind::Cursed       => '⊗',
            StatusKind::Corroded     => '⊙',
            StatusKind::Vulnerable   => '↓',
            StatusKind::Thorned      => '✦',
            StatusKind::Regenerating => '✚',
            StatusKind::Enraged      => '↑',
            StatusKind::Silenced     => '∅',
            StatusKind::Entropied    => '∞',
            StatusKind::TemporalSnare => '⧗',
            StatusKind::GravityWell  => '⊕',
        }
    }
}

/// A timed status effect applied to a combatant.
#[derive(Debug, Clone)]
pub struct StatusEffect {
    pub kind:       StatusKind,
    /// Total duration in seconds.
    pub duration:   f32,
    /// Elapsed time since application.
    pub age:        f32,
    /// Strength of the effect (damage per second, slow factor, etc.).
    pub strength:   f32,
    /// How many times this effect has been applied (stacking).
    pub stacks:     u32,
    /// Maximum stacks allowed.
    pub max_stacks: u32,
    /// Source entity that applied this effect.
    pub source_id:  Option<u32>,
}

impl StatusEffect {
    pub fn new(kind: StatusKind, duration: f32, strength: f32) -> Self {
        Self { kind, duration, age: 0.0, strength, stacks: 1, max_stacks: 5, source_id: None }
    }

    /// Burning: x damage/sec for 4 seconds.
    pub fn burning(dps: f32) -> Self { Self::new(StatusKind::Burning, 4.0, dps) }

    /// Frozen: slow factor 0.3, lasts 2 seconds.
    pub fn frozen() -> Self { Self::new(StatusKind::Frozen, 2.0, 0.3) }

    /// Poisoned: x damage/sec for 6 seconds, stacks up to 8.
    pub fn poisoned(dps: f32) -> Self {
        let mut s = Self::new(StatusKind::Poisoned, 6.0, dps);
        s.max_stacks = 8;
        s
    }

    /// Stunned: full stop for duration seconds.
    pub fn stunned(duration: f32) -> Self { Self::new(StatusKind::Stunned, duration, 1.0) }

    /// Regenerating: x hp/sec for duration seconds.
    pub fn regen(hp_per_sec: f32, duration: f32) -> Self {
        Self::new(StatusKind::Regenerating, duration, hp_per_sec)
    }

    /// Enraged: attack +50%, speed +30%, lasts 8 seconds.
    pub fn enraged() -> Self { Self::new(StatusKind::Enraged, 8.0, 1.5) }

    /// Entropied: increases entropy by strength, lasts duration.
    pub fn entropied(entropy: f32, duration: f32) -> Self {
        Self::new(StatusKind::Entropied, duration, entropy)
    }

    pub fn is_expired(&self) -> bool { self.age >= self.duration }
    pub fn remaining(&self) -> f32 { (self.duration - self.age).max(0.0) }
    pub fn progress(&self) -> f32 { (self.age / self.duration).clamp(0.0, 1.0) }

    /// Effective strength, accounting for stacking.
    pub fn effective_strength(&self) -> f32 {
        self.strength * self.stacks as f32
    }

    /// Advance the effect by `dt` seconds.
    /// Returns the damage dealt this tick (for DoT effects).
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.age += dt;
        match self.kind {
            StatusKind::Burning | StatusKind::Poisoned => {
                self.effective_strength() * dt
            }
            StatusKind::Regenerating => {
                // healing is positive but returned as negative damage
                -self.effective_strength() * dt
            }
            _ => 0.0,
        }
    }

    /// Try to add a stack. Returns true if stack was added.
    pub fn add_stack(&mut self) -> bool {
        if self.stacks < self.max_stacks {
            self.stacks += 1;
            self.age = 0.0; // refresh duration on stack
            true
        } else {
            false
        }
    }

    /// Slow factor this effect applies to movement (1.0 = normal, 0.0 = stopped).
    pub fn movement_slow(&self) -> f32 {
        match self.kind {
            StatusKind::Frozen        => 1.0 - self.strength.clamp(0.0, 0.9),
            StatusKind::Stunned       => 0.0,
            StatusKind::TemporalSnare => 1.0 - self.strength.clamp(0.0, 0.8),
            StatusKind::Poisoned      => 1.0 - self.stacks as f32 * 0.03,
            _                         => 1.0,
        }
    }

    /// Attack speed multiplier this effect applies (1.0 = normal).
    pub fn attack_speed_mult(&self) -> f32 {
        match self.kind {
            StatusKind::Frozen    => 0.3,
            StatusKind::Stunned   => 0.0,
            StatusKind::Enraged   => self.strength,
            StatusKind::Silenced  => 0.0,
            _                     => 1.0,
        }
    }
}

// ── StatusTracker ─────────────────────────────────────────────────────────────

/// Manages all status effects on a single entity.
///
/// Handles stacking, expiry, and per-tick resolution.
#[derive(Debug, Clone, Default)]
pub struct StatusTracker {
    pub effects: Vec<StatusEffect>,
}

impl StatusTracker {
    pub fn new() -> Self { Self { effects: Vec::new() } }

    /// Apply a status effect. If the same kind already exists, attempts to stack.
    pub fn apply(&mut self, mut effect: StatusEffect) {
        for existing in &mut self.effects {
            if existing.kind == effect.kind {
                if !existing.add_stack() {
                    // Max stacks reached — refresh duration only
                    existing.age = 0.0;
                }
                return;
            }
        }
        effect.stacks = 1;
        self.effects.push(effect);
    }

    /// Advance all effects by `dt`. Returns total damage dealt this tick (DoT).
    pub fn tick(&mut self, dt: f32) -> f32 {
        let mut total_damage = 0.0;
        for effect in &mut self.effects {
            total_damage += effect.tick(dt);
        }
        self.effects.retain(|e| !e.is_expired());
        total_damage
    }

    /// Remove all effects of a given kind.
    pub fn remove(&mut self, kind: StatusKind) {
        self.effects.retain(|e| e.kind != kind);
    }

    /// Remove all effects.
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    pub fn has(&self, kind: StatusKind) -> bool {
        self.effects.iter().any(|e| e.kind == kind)
    }

    pub fn is_stunned(&self) -> bool { self.has(StatusKind::Stunned) }
    pub fn is_frozen(&self)  -> bool { self.has(StatusKind::Frozen) }
    pub fn is_silenced(&self) -> bool { self.has(StatusKind::Silenced) }

    /// Combined movement slow from all active effects.
    pub fn movement_factor(&self) -> f32 {
        self.effects.iter().map(|e| e.movement_slow())
            .fold(1.0_f32, f32::min)
    }

    /// Combined attack speed factor from all active effects.
    pub fn attack_speed_factor(&self) -> f32 {
        self.effects.iter().map(|e| e.attack_speed_mult())
            .fold(1.0_f32, f32::min)
    }

    /// Current entropy amplification from Entropied stacks.
    pub fn entropy_amp(&self) -> f32 {
        self.effects.iter()
            .filter(|e| e.kind == StatusKind::Entropied)
            .map(|e| e.effective_strength())
            .sum::<f32>()
            .clamp(0.0, 3.0)
    }

    /// Damage multiplier from Vulnerable status.
    pub fn vulnerable_mult(&self) -> f32 {
        if self.has(StatusKind::Vulnerable) { 1.25 } else { 1.0 }
    }

    /// Thorns damage: fraction of incoming damage reflected.
    pub fn thorns_reflection(&self) -> f32 {
        self.effects.iter()
            .filter(|e| e.kind == StatusKind::Thorned)
            .map(|e| e.effective_strength() * 0.1)
            .sum::<f32>()
            .min(0.5)
    }
}

// ── DpsTracker ────────────────────────────────────────────────────────────────

/// Tracks damage-per-second in a rolling window.
#[derive(Debug, Clone)]
pub struct DpsTracker {
    /// Rolling window in seconds.
    pub window:  f32,
    samples:     std::collections::VecDeque<(f32, f32)>, // (timestamp, damage)
    pub time:    f32,
}

impl DpsTracker {
    pub fn new(window_seconds: f32) -> Self {
        Self { window: window_seconds, samples: std::collections::VecDeque::new(), time: 0.0 }
    }

    pub fn record(&mut self, damage: f32) {
        self.samples.push_back((self.time, damage));
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        let cutoff = self.time - self.window;
        while self.samples.front().map_or(false, |&(t, _)| t < cutoff) {
            self.samples.pop_front();
        }
    }

    /// Current DPS over the rolling window.
    pub fn dps(&self) -> f32 {
        let total: f32 = self.samples.iter().map(|(_, d)| d).sum();
        total / self.window.max(0.001)
    }

    /// Total damage recorded.
    pub fn total_damage(&self) -> f32 {
        self.samples.iter().map(|(_, d)| d).sum()
    }

    pub fn hit_count(&self) -> usize { self.samples.len() }

    pub fn reset(&mut self) {
        self.samples.clear();
        self.time = 0.0;
    }
}

// ── HitDetection ──────────────────────────────────────────────────────────────

/// Geometric hit detection utilities.
///
/// All hit detection is point-vs-shape — no physics engine is required.
pub struct HitDetection;

impl HitDetection {
    /// Point vs sphere. Returns true if `point` is within `radius` of `center`.
    pub fn point_in_sphere(point: Vec3, center: Vec3, radius: f32) -> bool {
        (point - center).length_squared() <= radius * radius
    }

    /// Point vs AABB. Returns true if `point` is inside the box.
    pub fn point_in_aabb(point: Vec3, min: Vec3, max: Vec3) -> bool {
        point.x >= min.x && point.x <= max.x
            && point.y >= min.y && point.y <= max.y
            && point.z >= min.z && point.z <= max.z
    }

    /// Point vs cylinder (along Y axis). Returns true if point is inside.
    pub fn point_in_cylinder(point: Vec3, center: Vec3, radius: f32, half_height: f32) -> bool {
        let dx = point.x - center.x;
        let dz = point.z - center.z;
        let dy = (point.y - center.y).abs();
        dx * dx + dz * dz <= radius * radius && dy <= half_height
    }

    /// Sphere vs sphere intersection. Returns overlap depth (negative = no overlap).
    pub fn sphere_overlap(ca: Vec3, ra: f32, cb: Vec3, rb: f32) -> f32 {
        let dist = (ca - cb).length();
        ra + rb - dist
    }

    /// Cone hit test — used for directional attacks (breath, slam, sweep).
    ///
    /// Returns true if `target` is inside a cone from `origin` pointing `direction`,
    /// with half-angle `half_angle_rad` and max range `range`.
    pub fn point_in_cone(
        target: Vec3, origin: Vec3, direction: Vec3, half_angle_rad: f32, range: f32,
    ) -> bool {
        let to_target = target - origin;
        let dist = to_target.length();
        if dist > range || dist < 1e-6 { return false; }
        let cos_angle = to_target.dot(direction.normalize_or_zero()) / dist;
        cos_angle >= half_angle_rad.cos()
    }

    /// Raycast against a sphere. Returns distance to hit (None if miss).
    pub fn ray_vs_sphere(
        ray_origin: Vec3, ray_dir: Vec3, sphere_center: Vec3, sphere_radius: f32,
    ) -> Option<f32> {
        let oc = ray_origin - sphere_center;
        let b = oc.dot(ray_dir);
        let c = oc.dot(oc) - sphere_radius * sphere_radius;
        let discriminant = b * b - c;
        if discriminant < 0.0 { return None; }
        let sqrt_d = discriminant.sqrt();
        let t0 = -b - sqrt_d;
        let t1 = -b + sqrt_d;
        if t0 >= 0.0 { Some(t0) } else if t1 >= 0.0 { Some(t1) } else { None }
    }

    /// Find all targets within range of `origin`, sorted by distance.
    pub fn targets_in_range<'a>(
        origin: Vec3,
        targets: &'a [Vec3],
        range: f32,
    ) -> Vec<(usize, f32)> {
        let mut hits: Vec<(usize, f32)> = targets.iter().enumerate()
            .filter_map(|(i, &pos)| {
                let dist = (pos - origin).length();
                if dist <= range { Some((i, dist)) } else { None }
            })
            .collect();
        hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        hits
    }

    /// Compute knockback vector from attacker to defender, scaled by `strength`.
    pub fn knockback(attacker_pos: Vec3, defender_pos: Vec3, strength: f32) -> Vec3 {
        let dir = (defender_pos - attacker_pos).normalize_or_zero();
        dir * strength
    }
}

// ── CombatEvent log ───────────────────────────────────────────────────────────

/// An entry in the combat event log.
#[derive(Debug, Clone)]
pub struct CombatLogEntry {
    pub timestamp:    f32,
    pub attacker_id:  u32,
    pub defender_id:  u32,
    pub result:       HitResult,
    pub status_applied: Option<StatusKind>,
}

/// Rolling combat event log.
#[derive(Debug, Clone)]
pub struct CombatLog {
    pub entries:     Vec<CombatLogEntry>,
    pub max_entries: usize,
}

impl CombatLog {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), max_entries }
    }

    pub fn push(&mut self, entry: CombatLogEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn kills(&self) -> usize {
        self.entries.iter().filter(|e| e.result.is_kill).count()
    }

    pub fn crits(&self) -> usize {
        self.entries.iter().filter(|e| e.result.is_crit).count()
    }

    pub fn total_damage(&self) -> f32 {
        self.entries.iter().map(|e| e.result.final_damage).sum()
    }

    pub fn crit_rate(&self) -> f32 {
        if self.entries.is_empty() { return 0.0; }
        self.crits() as f32 / self.entries.len() as f32
    }

    pub fn avg_damage(&self) -> f32 {
        if self.entries.is_empty() { return 0.0; }
        self.total_damage() / self.entries.len() as f32
    }

    pub fn clear(&mut self) { self.entries.clear(); }
}

// ── Threat system ─────────────────────────────────────────────────────────────

/// Tracks threat levels for AI targeting.
///
/// Entities with higher threat are prioritized for attacks.
#[derive(Debug, Clone, Default)]
pub struct ThreatTable {
    pub threat: HashMap<u32, f32>,
}

impl ThreatTable {
    pub fn new() -> Self { Self { threat: HashMap::new() } }

    pub fn add_threat(&mut self, id: u32, amount: f32) {
        *self.threat.entry(id).or_insert(0.0) += amount;
    }

    pub fn reduce_threat(&mut self, id: u32, amount: f32) {
        if let Some(t) = self.threat.get_mut(&id) {
            *t = (*t - amount).max(0.0);
        }
    }

    /// Decay all threat by `factor` per second.
    pub fn decay(&mut self, dt: f32, factor: f32) {
        for t in self.threat.values_mut() {
            *t *= (1.0 - factor * dt).max(0.0);
        }
        self.threat.retain(|_, &mut t| t > 0.001);
    }

    /// Highest threat entity ID.
    pub fn top_target(&self) -> Option<u32> {
        self.threat.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, _)| id)
    }

    /// All threats sorted by amount (descending).
    pub fn sorted_targets(&self) -> Vec<(u32, f32)> {
        let mut v: Vec<(u32, f32)> = self.threat.iter().map(|(&id, &t)| (id, t)).collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v
    }

    pub fn remove(&mut self, id: u32) { self.threat.remove(&id); }
    pub fn clear(&mut self) { self.threat.clear(); }
    pub fn get(&self, id: u32) -> f32 { *self.threat.get(&id).unwrap_or(&0.0) }
    pub fn target_count(&self) -> usize { self.threat.len() }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_attacker() -> CombatStats {
        CombatStats { attack: 20.0, crit_chance: 0.0, crit_mult: 2.0, ..Default::default() }
    }

    fn make_defender() -> CombatStats {
        CombatStats { hp: 100.0, max_hp: 100.0, armor: 0.0, dodge_chance: 0.0, ..Default::default() }
    }

    #[test]
    fn dodge_on_low_roll() {
        let att = make_attacker();
        let mut def = make_defender();
        def.dodge_chance = 1.0; // always dodge
        let event = DamageEvent {
            base_damage: 1.0, element: Element::Physical,
            attacker_pos: Vec3::ZERO, defender_pos: Vec3::ONE,
            roll: 0.5,
        };
        let result = CombatFormulas::resolve(&event, &att, &def, &ResistanceProfile::neutral());
        assert!(result.is_dodge, "should dodge with dodge_chance=1.0");
        assert_eq!(result.final_damage, 0.0);
    }

    #[test]
    fn crit_doubles_damage() {
        let att = CombatStats { attack: 10.0, crit_chance: 1.0, crit_mult: 2.0, ..Default::default() };
        let def = make_defender();
        let event = DamageEvent {
            base_damage: 1.0, element: Element::Physical,
            attacker_pos: Vec3::ZERO, defender_pos: Vec3::ONE,
            roll: 0.5,
        };
        let result = CombatFormulas::resolve(&event, &att, &def, &ResistanceProfile::neutral());
        assert!(result.is_crit, "should be crit with crit_chance=1.0");
        assert!(result.pre_resist > 10.0, "crit should amplify damage");
    }

    #[test]
    fn fire_resistance_halves_fire_damage() {
        let att = make_attacker();
        let def = make_defender();
        let mut resist = ResistanceProfile::neutral();
        resist.set(Element::Fire, 0.5);
        let event = DamageEvent {
            base_damage: 1.0, element: Element::Fire,
            attacker_pos: Vec3::ZERO, defender_pos: Vec3::ONE,
            roll: 0.5,
        };
        let result = CombatFormulas::resolve(&event, &att, &def, &resist);
        // post_resist should be ~half of pre_resist
        assert!((result.post_resist - result.pre_resist * 0.5).abs() < 0.01,
                "fire resist 0.5 should halve damage");
    }

    #[test]
    fn status_tracker_stacks() {
        let mut tracker = StatusTracker::new();
        tracker.apply(StatusEffect::poisoned(5.0));
        tracker.apply(StatusEffect::poisoned(5.0));
        let poison = tracker.effects.iter().find(|e| e.kind == StatusKind::Poisoned).unwrap();
        assert_eq!(poison.stacks, 2);
    }

    #[test]
    fn status_tracker_dots_damage() {
        let mut tracker = StatusTracker::new();
        tracker.apply(StatusEffect::burning(10.0));
        let dmg = tracker.tick(1.0); // 1 second
        assert!((dmg - 10.0).abs() < 0.01, "burning 10 dps for 1 sec = 10 damage, got {}", dmg);
    }

    #[test]
    fn dps_tracker_rolling() {
        let mut tracker = DpsTracker::new(3.0);
        tracker.record(30.0);
        tracker.tick(1.0);
        assert!((tracker.dps() - 10.0).abs() < 0.01, "30 damage over 3s window = 10 dps");
    }

    #[test]
    fn hit_detection_sphere() {
        assert!(HitDetection::point_in_sphere(Vec3::new(0.5, 0.0, 0.0), Vec3::ZERO, 1.0));
        assert!(!HitDetection::point_in_sphere(Vec3::new(2.0, 0.0, 0.0), Vec3::ZERO, 1.0));
    }

    #[test]
    fn hit_detection_cone() {
        let in_cone = HitDetection::point_in_cone(
            Vec3::new(0.0, 0.0, 1.0), Vec3::ZERO, Vec3::Z, 0.5, 5.0
        );
        assert!(in_cone, "point directly in front should be in cone");
        let behind = HitDetection::point_in_cone(
            Vec3::new(0.0, 0.0, -1.0), Vec3::ZERO, Vec3::Z, 0.5, 5.0
        );
        assert!(!behind, "point behind should not be in cone");
    }

    #[test]
    fn threat_table_top_target() {
        let mut tt = ThreatTable::new();
        tt.add_threat(1, 50.0);
        tt.add_threat(2, 200.0);
        tt.add_threat(3, 10.0);
        assert_eq!(tt.top_target(), Some(2));
    }

    #[test]
    fn combat_log_stats() {
        let mut log = CombatLog::new(100);
        log.push(CombatLogEntry {
            timestamp: 0.0, attacker_id: 1, defender_id: 2,
            result: HitResult {
                final_damage: 50.0, is_crit: true, is_dodge: false,
                is_block: false, is_kill: false, element: Element::Fire,
                pre_resist: 60.0, post_resist: 50.0, post_armor: 50.0, overkill: 0.0,
            },
            status_applied: None,
        });
        assert!((log.total_damage() - 50.0).abs() < 0.01);
        assert_eq!(log.crits(), 1);
    }
}
