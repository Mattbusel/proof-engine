#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// ABILITY TYPES (20+)
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilityType {
    MeleeAttack,
    RangedAttack,
    AreaOfEffect,
    Cone,
    Line,
    Nova,
    Projectile,
    Beam,
    Teleport,
    Summon,
    Buff,
    Debuff,
    Heal,
    Shield,
    Terrain,
    Transform,
    Stealth,
    Channel,
    Charge,
    Dash,
    Trap,
    Mine,
    Totem,
    Warcry,
    Curse,
    Brand,
    Stance,
    Pet,
    Vortex,
    ChainLightning,
}

impl AbilityType {
    pub fn display_name(&self) -> &'static str {
        match self {
            AbilityType::MeleeAttack => "Melee Attack",
            AbilityType::RangedAttack => "Ranged Attack",
            AbilityType::AreaOfEffect => "Area of Effect",
            AbilityType::Cone => "Cone",
            AbilityType::Line => "Line / Ray",
            AbilityType::Nova => "Nova",
            AbilityType::Projectile => "Projectile",
            AbilityType::Beam => "Beam",
            AbilityType::Teleport => "Teleport",
            AbilityType::Summon => "Summon",
            AbilityType::Buff => "Buff",
            AbilityType::Debuff => "Debuff",
            AbilityType::Heal => "Heal",
            AbilityType::Shield => "Shield",
            AbilityType::Terrain => "Terrain Alteration",
            AbilityType::Transform => "Transform",
            AbilityType::Stealth => "Stealth",
            AbilityType::Channel => "Channel",
            AbilityType::Charge => "Charge",
            AbilityType::Dash => "Dash",
            AbilityType::Trap => "Trap",
            AbilityType::Mine => "Mine",
            AbilityType::Totem => "Totem",
            AbilityType::Warcry => "Warcry",
            AbilityType::Curse => "Curse",
            AbilityType::Brand => "Brand",
            AbilityType::Stance => "Stance",
            AbilityType::Pet => "Pet",
            AbilityType::Vortex => "Vortex",
            AbilityType::ChainLightning => "Chain Lightning",
        }
    }

    pub fn is_movement(&self) -> bool {
        matches!(self, AbilityType::Teleport | AbilityType::Dash | AbilityType::Charge)
    }

    pub fn is_damage_dealing(&self) -> bool {
        matches!(self,
            AbilityType::MeleeAttack | AbilityType::RangedAttack | AbilityType::AreaOfEffect |
            AbilityType::Cone | AbilityType::Line | AbilityType::Nova | AbilityType::Projectile |
            AbilityType::Beam | AbilityType::Charge | AbilityType::ChainLightning | AbilityType::Vortex
        )
    }

    pub fn can_have_projectile(&self) -> bool {
        matches!(self, AbilityType::RangedAttack | AbilityType::Projectile | AbilityType::ChainLightning)
    }

    pub fn default_targeting(&self) -> TargetingMode {
        match self {
            AbilityType::MeleeAttack => TargetingMode::SingleTarget,
            AbilityType::RangedAttack => TargetingMode::SingleTarget,
            AbilityType::AreaOfEffect => TargetingMode::GroundTarget { indicator: AreaIndicator::Circle },
            AbilityType::Cone => TargetingMode::Cone { half_angle_deg: 30.0, distance: 8.0 },
            AbilityType::Line => TargetingMode::Rectangle { width: 2.0, length: 15.0 },
            AbilityType::Nova => TargetingMode::Self_,
            AbilityType::Projectile => TargetingMode::SingleTarget,
            AbilityType::Beam => TargetingMode::SingleTarget,
            AbilityType::Teleport => TargetingMode::GroundTarget { indicator: AreaIndicator::Point },
            AbilityType::Summon => TargetingMode::GroundTarget { indicator: AreaIndicator::Point },
            AbilityType::Buff | AbilityType::Stance | AbilityType::Transform | AbilityType::Stealth => TargetingMode::Self_,
            AbilityType::Heal => TargetingMode::SingleTarget,
            AbilityType::Shield => TargetingMode::Self_,
            AbilityType::Terrain => TargetingMode::GroundTarget { indicator: AreaIndicator::Circle },
            AbilityType::Channel => TargetingMode::SingleTarget,
            AbilityType::Charge | AbilityType::Dash => TargetingMode::SingleTarget,
            AbilityType::Trap | AbilityType::Mine => TargetingMode::GroundTarget { indicator: AreaIndicator::Point },
            AbilityType::Totem => TargetingMode::GroundTarget { indicator: AreaIndicator::Point },
            AbilityType::Warcry | AbilityType::Debuff | AbilityType::Curse => TargetingMode::Self_,
            AbilityType::Brand => TargetingMode::SingleTarget,
            AbilityType::Pet => TargetingMode::Self_,
            AbilityType::Vortex => TargetingMode::GroundTarget { indicator: AreaIndicator::Circle },
            AbilityType::ChainLightning => TargetingMode::SingleTarget,
        }
    }
}

// ============================================================
// TARGETING SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TargetingMode {
    Self_,
    SingleTarget,
    MultiTarget { max_targets: u32, radius: f32 },
    AoECircle { radius: f32 },
    AoESphere { radius: f32 },
    Cone { half_angle_deg: f32, distance: f32 },
    Rectangle { width: f32, length: f32 },
    Chain { max_bounces: u32, bounce_radius: f32, falloff_per_bounce: f32 },
    SmartCast { priority: TargetPriority },
    GroundTarget { indicator: AreaIndicator },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPriority {
    Nearest,
    LowestHealth,
    HighestHealth,
    LowestArmor,
    MostDangerous,
    Player,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaIndicator {
    Circle,
    Rectangle,
    Cone,
    Point,
    Line,
    Ring,
}

impl TargetingMode {
    pub fn display_name(&self) -> String {
        match self {
            TargetingMode::Self_ => "Self".to_string(),
            TargetingMode::SingleTarget => "Single Target".to_string(),
            TargetingMode::MultiTarget { max_targets, .. } => format!("Multi-Target ({})", max_targets),
            TargetingMode::AoECircle { radius } => format!("AoE Circle (r={})", radius),
            TargetingMode::AoESphere { radius } => format!("AoE Sphere (r={})", radius),
            TargetingMode::Cone { half_angle_deg, distance } => format!("Cone ({}°, {}u)", half_angle_deg * 2.0, distance),
            TargetingMode::Rectangle { width, length } => format!("Rectangle ({}×{})", width, length),
            TargetingMode::Chain { max_bounces, .. } => format!("Chain ({} bounces)", max_bounces),
            TargetingMode::SmartCast { .. } => "Smart Cast".to_string(),
            TargetingMode::GroundTarget { .. } => "Ground Target".to_string(),
        }
    }

    /// Returns true if a given world-space point is within this targeting shape,
    /// assuming caster is at `origin`, facing in `forward` direction.
    pub fn contains_point_2d(&self, origin: Vec2, forward: Vec2, target: Vec2) -> bool {
        let delta = target - origin;
        let dist = delta.length();
        match self {
            TargetingMode::AoECircle { radius } => dist <= *radius,
            TargetingMode::AoESphere { radius } => dist <= *radius,
            TargetingMode::Cone { half_angle_deg, distance } => {
                if dist > *distance { return false; }
                let fwd = if forward.length_squared() < 1e-9 { Vec2::new(0.0, 1.0) } else { forward.normalize() };
                let to_target = if dist < 1e-9 { return true; } else { delta / dist };
                let dot = fwd.dot(to_target);
                let angle = dot.acos().to_degrees();
                angle <= *half_angle_deg
            }
            TargetingMode::Rectangle { width, length } => {
                // Rectangle along forward axis
                let fwd = if forward.length_squared() < 1e-9 { Vec2::new(0.0, 1.0) } else { forward.normalize() };
                let right = Vec2::new(-fwd.y, fwd.x);
                let along = delta.dot(fwd);
                let across = delta.dot(right).abs();
                along >= 0.0 && along <= *length && across <= *width * 0.5
            }
            TargetingMode::SingleTarget => dist < 0.5,
            _ => false,
        }
    }

    /// Collect all targets from a list that fall within this targeting shape.
    pub fn gather_targets<'a>(
        &self,
        origin: Vec2,
        forward: Vec2,
        candidates: &'a [(u64, Vec2)],
        ground_target: Option<Vec2>,
    ) -> Vec<(u64, Vec2)> {
        let effective_origin = match self {
            TargetingMode::AoECircle { .. } | TargetingMode::AoESphere { .. } |
            TargetingMode::GroundTarget { .. } => ground_target.unwrap_or(origin),
            _ => origin,
        };

        candidates.iter().filter(|(_, pos)| {
            self.contains_point_2d(effective_origin, forward, *pos)
        }).cloned().collect()
    }

    /// Chain targeting: start from a primary target and bounce to nearest within bounce_radius.
    pub fn chain_targets<'a>(
        &self,
        primary_target: (u64, Vec2),
        all_candidates: &'a [(u64, Vec2)],
    ) -> Vec<(u64, Vec2)> {
        if let TargetingMode::Chain { max_bounces, bounce_radius, .. } = self {
            let mut result = vec![primary_target];
            let mut hit: HashSet<u64> = HashSet::new();
            hit.insert(primary_target.0);
            let mut last_pos = primary_target.1;

            for _ in 0..*max_bounces {
                let next = all_candidates.iter()
                    .filter(|(id, _)| !hit.contains(id))
                    .filter(|(_, pos)| (*pos - last_pos).length() <= *bounce_radius)
                    .min_by(|(_, pa), (_, pb)| {
                        let da = (*pa - last_pos).length();
                        let db = (*pb - last_pos).length();
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    });
                if let Some(&(id, pos)) = next {
                    result.push((id, pos));
                    hit.insert(id);
                    last_pos = pos;
                } else {
                    break;
                }
            }
            result
        } else {
            vec![]
        }
    }
}

// ============================================================
// DAMAGE TYPES AND FORMULAS
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageElement {
    Physical,
    Fire,
    Cold,
    Lightning,
    Poison,
    Arcane,
    Holy,
    Shadow,
    Chaos,
    True_,
}

impl DamageElement {
    pub fn display_name(&self) -> &'static str {
        match self {
            DamageElement::Physical => "Physical",
            DamageElement::Fire => "Fire",
            DamageElement::Cold => "Cold",
            DamageElement::Lightning => "Lightning",
            DamageElement::Poison => "Poison",
            DamageElement::Arcane => "Arcane",
            DamageElement::Holy => "Holy",
            DamageElement::Shadow => "Shadow",
            DamageElement::Chaos => "Chaos",
            DamageElement::True_ => "True",
        }
    }

    pub fn color(&self) -> Vec4 {
        match self {
            DamageElement::Physical => Vec4::new(0.8, 0.7, 0.6, 1.0),
            DamageElement::Fire => Vec4::new(1.0, 0.3, 0.0, 1.0),
            DamageElement::Cold => Vec4::new(0.3, 0.7, 1.0, 1.0),
            DamageElement::Lightning => Vec4::new(1.0, 1.0, 0.2, 1.0),
            DamageElement::Poison => Vec4::new(0.4, 0.9, 0.1, 1.0),
            DamageElement::Arcane => Vec4::new(0.8, 0.2, 1.0, 1.0),
            DamageElement::Holy => Vec4::new(1.0, 0.95, 0.7, 1.0),
            DamageElement::Shadow => Vec4::new(0.3, 0.0, 0.5, 1.0),
            DamageElement::Chaos => Vec4::new(0.7, 0.0, 0.2, 1.0),
            DamageElement::True_ => Vec4::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

/// Stat-based scaling coefficient
#[derive(Debug, Clone)]
pub struct ScalingCoeff {
    pub stat_name: String,
    pub coefficient: f32,
    pub exponent: f32, // power scaling: stat^exponent * coefficient
}

impl ScalingCoeff {
    pub fn linear(stat: impl Into<String>, coeff: f32) -> Self {
        ScalingCoeff { stat_name: stat.into(), coefficient: coeff, exponent: 1.0 }
    }

    pub fn power(stat: impl Into<String>, coeff: f32, exp: f32) -> Self {
        ScalingCoeff { stat_name: stat.into(), coefficient: coeff, exponent: exp }
    }

    pub fn compute(&self, stat_value: f32) -> f32 {
        self.coefficient * stat_value.powf(self.exponent)
    }
}

#[derive(Debug, Clone)]
pub struct DamageFormula {
    pub element: DamageElement,
    pub base_min: f32,
    pub base_max: f32,
    pub scaling: Vec<ScalingCoeff>,
    pub crit_chance_base: f32,         // percent
    pub crit_multiplier_base: f32,     // e.g. 1.5 = 150% damage
    pub crit_chance_scaling: Vec<ScalingCoeff>,
    pub crit_multiplier_scaling: Vec<ScalingCoeff>,
    pub penetration: f32,              // flat resistance penetration
    pub penetration_percent: f32,      // percent of resistance ignored
    pub versus_status_bonus: Vec<(StatusEffectType, f32)>, // bonus damage if target has status
    pub variance: f32,                 // ± fraction of computed damage
}

impl DamageFormula {
    pub fn new(element: DamageElement, min: f32, max: f32) -> Self {
        DamageFormula {
            element,
            base_min: min,
            base_max: max,
            scaling: Vec::new(),
            crit_chance_base: 5.0,
            crit_multiplier_base: 1.5,
            crit_chance_scaling: Vec::new(),
            crit_multiplier_scaling: Vec::new(),
            penetration: 0.0,
            penetration_percent: 0.0,
            versus_status_bonus: Vec::new(),
            variance: 0.1,
        }
    }

    pub fn compute_raw(&self, stats: &HashMap<String, f32>, roll_t: f32) -> f32 {
        // Base damage with variance
        let base = self.base_min + (self.base_max - self.base_min) * roll_t;
        let scaling_bonus: f32 = self.scaling.iter().map(|s| {
            let val = stats.get(&s.stat_name).copied().unwrap_or(0.0);
            s.compute(val)
        }).sum();
        let raw = base + scaling_bonus;
        let var = 1.0 + (roll_t * 2.0 - 1.0) * self.variance;
        raw * var
    }

    pub fn compute_crit_chance(&self, stats: &HashMap<String, f32>) -> f32 {
        let bonus: f32 = self.crit_chance_scaling.iter().map(|s| {
            let val = stats.get(&s.stat_name).copied().unwrap_or(0.0);
            s.compute(val)
        }).sum();
        (self.crit_chance_base + bonus).clamp(0.0, 100.0)
    }

    pub fn compute_crit_multiplier(&self, stats: &HashMap<String, f32>) -> f32 {
        let bonus: f32 = self.crit_multiplier_scaling.iter().map(|s| {
            let val = stats.get(&s.stat_name).copied().unwrap_or(0.0);
            s.compute(val)
        }).sum();
        self.crit_multiplier_base + bonus
    }

    pub fn compute_final_damage(
        &self,
        stats: &HashMap<String, f32>,
        target_resist: f32,
        active_effects: &[StatusEffectType],
        roll_t: f32,
        crit_roll: f32,
    ) -> DamageResult {
        let raw = self.compute_raw(stats, roll_t);
        let crit_chance = self.compute_crit_chance(stats);
        let crit_mul = self.compute_crit_multiplier(stats);

        let is_crit = crit_roll < crit_chance / 100.0;
        let crit_factor = if is_crit { crit_mul } else { 1.0 };

        // Status bonus
        let mut status_mul = 1.0f32;
        for (effect_type, bonus) in &self.versus_status_bonus {
            if active_effects.contains(effect_type) {
                status_mul += bonus;
            }
        }

        // Resistance after penetration
        let effective_resist = if self.element == DamageElement::True_ {
            0.0
        } else {
            let after_flat = (target_resist - self.penetration).max(0.0);
            let after_pct = after_flat * (1.0 - self.penetration_percent / 100.0);
            after_pct.min(75.0) // resist cap
        };

        let after_resist = raw * (1.0 - effective_resist / 100.0);
        let final_dmg = after_resist * crit_factor * status_mul;

        DamageResult {
            raw_damage: raw,
            final_damage: final_dmg,
            is_crit,
            element: self.element,
            effective_resist,
            status_bonus_applied: status_mul > 1.0,
        }
    }

    pub fn average_dps_preview(&self, stats: &HashMap<String, f32>, target_resist: f32) -> f32 {
        let mut total = 0.0f32;
        let samples = 20;
        for i in 0..samples {
            let t = i as f32 / (samples - 1) as f32;
            let result = self.compute_final_damage(stats, target_resist, &[], t, t);
            total += result.final_damage;
        }
        total / samples as f32
    }
}

#[derive(Debug, Clone)]
pub struct DamageResult {
    pub raw_damage: f32,
    pub final_damage: f32,
    pub is_crit: bool,
    pub element: DamageElement,
    pub effective_resist: f32,
    pub status_bonus_applied: bool,
}

// ============================================================
// STATUS EFFECTS (30+)
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusEffectType {
    Burn,
    Freeze,
    Chill,
    Shock,
    Stun,
    Slow,
    Poison,
    Bleed,
    Blind,
    Silence,
    Root,
    Knockback,
    Knockup,
    Fear,
    Charm,
    Haste,
    Shield,
    Regen,
    ManaRegen,
    Taunt,
    Confused,
    Petrified,
    Invulnerable,
    Invisible,
    Cursed,
    Marked,
    Wet,
    Oiled,
    Shocked,
    Brittle,
    Empowered,
    Weakened,
    Exposed,
    Enraged,
    Exhausted,
}

impl StatusEffectType {
    pub fn display_name(&self) -> &'static str {
        match self {
            StatusEffectType::Burn => "Burn",
            StatusEffectType::Freeze => "Freeze",
            StatusEffectType::Chill => "Chill",
            StatusEffectType::Shock => "Shock",
            StatusEffectType::Stun => "Stun",
            StatusEffectType::Slow => "Slow",
            StatusEffectType::Poison => "Poison",
            StatusEffectType::Bleed => "Bleed",
            StatusEffectType::Blind => "Blind",
            StatusEffectType::Silence => "Silence",
            StatusEffectType::Root => "Root",
            StatusEffectType::Knockback => "Knockback",
            StatusEffectType::Knockup => "Knockup",
            StatusEffectType::Fear => "Fear",
            StatusEffectType::Charm => "Charm",
            StatusEffectType::Haste => "Haste",
            StatusEffectType::Shield => "Shield",
            StatusEffectType::Regen => "Regen",
            StatusEffectType::ManaRegen => "Mana Regen",
            StatusEffectType::Taunt => "Taunt",
            StatusEffectType::Confused => "Confused",
            StatusEffectType::Petrified => "Petrified",
            StatusEffectType::Invulnerable => "Invulnerable",
            StatusEffectType::Invisible => "Invisible",
            StatusEffectType::Cursed => "Cursed",
            StatusEffectType::Marked => "Marked",
            StatusEffectType::Wet => "Wet",
            StatusEffectType::Oiled => "Oiled",
            StatusEffectType::Shocked => "Shocked",
            StatusEffectType::Brittle => "Brittle",
            StatusEffectType::Empowered => "Empowered",
            StatusEffectType::Weakened => "Weakened",
            StatusEffectType::Exposed => "Exposed",
            StatusEffectType::Enraged => "Enraged",
            StatusEffectType::Exhausted => "Exhausted",
        }
    }

    pub fn color(&self) -> Vec4 {
        match self {
            StatusEffectType::Burn => Vec4::new(1.0, 0.3, 0.0, 1.0),
            StatusEffectType::Freeze | StatusEffectType::Chill => Vec4::new(0.4, 0.8, 1.0, 1.0),
            StatusEffectType::Shock | StatusEffectType::Shocked => Vec4::new(1.0, 1.0, 0.0, 1.0),
            StatusEffectType::Stun | StatusEffectType::Petrified => Vec4::new(0.7, 0.5, 0.2, 1.0),
            StatusEffectType::Slow | StatusEffectType::Root => Vec4::new(0.5, 0.5, 0.2, 1.0),
            StatusEffectType::Poison | StatusEffectType::Bleed => Vec4::new(0.3, 0.9, 0.1, 1.0),
            StatusEffectType::Blind | StatusEffectType::Silence => Vec4::new(0.3, 0.3, 0.3, 1.0),
            StatusEffectType::Knockback | StatusEffectType::Knockup => Vec4::new(1.0, 0.5, 0.8, 1.0),
            StatusEffectType::Fear | StatusEffectType::Confused => Vec4::new(0.8, 0.2, 0.8, 1.0),
            StatusEffectType::Charm => Vec4::new(1.0, 0.4, 0.6, 1.0),
            StatusEffectType::Haste | StatusEffectType::Empowered => Vec4::new(0.2, 1.0, 0.2, 1.0),
            StatusEffectType::Shield | StatusEffectType::Invulnerable => Vec4::new(0.6, 0.8, 1.0, 1.0),
            StatusEffectType::Regen | StatusEffectType::ManaRegen => Vec4::new(0.0, 1.0, 0.5, 1.0),
            StatusEffectType::Invisible | StatusEffectType::Stealth => Vec4::new(0.5, 0.5, 0.7, 0.6),
            StatusEffectType::Cursed | StatusEffectType::Weakened | StatusEffectType::Exposed => Vec4::new(0.8, 0.0, 0.5, 1.0),
            StatusEffectType::Marked => Vec4::new(1.0, 0.0, 0.0, 1.0),
            StatusEffectType::Wet => Vec4::new(0.3, 0.5, 1.0, 1.0),
            StatusEffectType::Oiled => Vec4::new(0.3, 0.2, 0.0, 1.0),
            StatusEffectType::Brittle => Vec4::new(0.7, 0.7, 0.7, 1.0),
            StatusEffectType::Enraged => Vec4::new(1.0, 0.1, 0.0, 1.0),
            StatusEffectType::Exhausted => Vec4::new(0.5, 0.4, 0.3, 1.0),
            StatusEffectType::Taunt => Vec4::new(0.9, 0.4, 0.0, 1.0),
        }
    }

    pub fn is_crowd_control(&self) -> bool {
        matches!(self,
            StatusEffectType::Freeze | StatusEffectType::Stun | StatusEffectType::Root |
            StatusEffectType::Knockback | StatusEffectType::Knockup | StatusEffectType::Fear |
            StatusEffectType::Charm | StatusEffectType::Petrified | StatusEffectType::Confused
        )
    }

    pub fn is_beneficial(&self) -> bool {
        matches!(self,
            StatusEffectType::Haste | StatusEffectType::Shield | StatusEffectType::Regen |
            StatusEffectType::ManaRegen | StatusEffectType::Invulnerable | StatusEffectType::Invisible |
            StatusEffectType::Empowered | StatusEffectType::Enraged
        )
    }

    pub fn is_dot(&self) -> bool {
        matches!(self, StatusEffectType::Burn | StatusEffectType::Poison | StatusEffectType::Bleed)
    }
}

// Handle Stealth as a special display case for color matching
trait StatusEffectTypeExt {
    fn stealth_color() -> Vec4;
}

#[derive(Debug, Clone)]
pub struct StackRule {
    pub max_stacks: u32,
    pub stack_behavior: StackBehavior,
    pub application_behavior: ApplicationBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackBehavior {
    Replace,             // New application overwrites old
    Refresh,             // Reset duration on new application
    AddStack,            // Add a stack, each with its own timer
    AddStackRefreshAll,  // Add stack and refresh all timers
    Ignore,              // Ignore if already present
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationBehavior {
    Always,
    OnlyIfAbsent,
    OnlyIfPresent,
    IncrementOnly,
}

#[derive(Debug, Clone)]
pub struct StatusEffectDefinition {
    pub id: u64,
    pub effect_type: StatusEffectType,
    pub name: String,
    pub description: String,
    pub base_duration: f32,       // seconds; 0 = permanent until removed
    pub tick_interval: f32,       // seconds between tick applications (0 = no ticks)
    pub tick_damage: DamageFormula,
    pub tick_heal: f32,
    pub stat_modifiers: Vec<StatusStatMod>,
    pub stack_rule: StackRule,
    pub immunity_window: f32,     // seconds of immunity after expiry
    pub can_be_cleansed: bool,
    pub can_be_dispelled: bool,
    pub is_debuff: bool,
    pub visual_particle: String,
    pub sound_on_apply: String,
    pub sound_on_tick: String,
    pub sound_on_expire: String,
    pub cc_break_on_damage: bool,
    pub cc_damage_threshold: f32, // damage needed to break CC (0 = never breaks)
    pub interactions: Vec<StatusInteraction>,
}

#[derive(Debug, Clone)]
pub struct StatusStatMod {
    pub stat: String,
    pub flat_delta: f32,
    pub percent_delta: f32,
    pub per_stack: bool, // if true, multiply by stack count
}

#[derive(Debug, Clone)]
pub struct StatusInteraction {
    pub when_hit_by: StatusEffectType,
    pub reaction: StatusReaction,
}

#[derive(Debug, Clone)]
pub enum StatusReaction {
    Explode { damage_formula: DamageFormula },
    Transform { into: StatusEffectType },
    Remove,
    Amplify { factor: f32 },
}

impl StatusEffectDefinition {
    pub fn tick(&self, stack_count: u32, stats: &HashMap<String, f32>, t: f32) -> TickResult {
        let damage = if self.tick_interval > 0.0 {
            let d = self.tick_damage.compute_raw(stats, t);
            d * stack_count as f32
        } else {
            0.0
        };

        let heal = self.tick_heal * stack_count as f32;

        TickResult { damage, heal, stack_count }
    }

    pub fn stat_bonuses(&self, stack_count: u32) -> Vec<(String, f32, f32)> {
        self.stat_modifiers.iter().map(|m| {
            let mul = if m.per_stack { stack_count as f32 } else { 1.0 };
            (m.stat.clone(), m.flat_delta * mul, m.percent_delta * mul)
        }).collect()
    }
}

#[derive(Debug, Clone)]
pub struct TickResult {
    pub damage: f32,
    pub heal: f32,
    pub stack_count: u32,
}

// Active instance of a status effect on a target
#[derive(Debug, Clone)]
pub struct ActiveStatusEffect {
    pub definition_id: u64,
    pub effect_type: StatusEffectType,
    pub stack_count: u32,
    pub remaining_duration: f32,
    pub tick_timer: f32,
    pub source_id: u64,        // ability ID that applied this
    pub source_caster_id: u64,
    pub immunity_remaining: f32,
}

impl ActiveStatusEffect {
    pub fn new(def: &StatusEffectDefinition, source_id: u64, caster_id: u64) -> Self {
        ActiveStatusEffect {
            definition_id: def.id,
            effect_type: def.effect_type,
            stack_count: 1,
            remaining_duration: def.base_duration,
            tick_timer: def.tick_interval,
            source_id,
            source_caster_id: caster_id,
            immunity_remaining: 0.0,
        }
    }

    pub fn tick_update(&mut self, dt: f32, def: &StatusEffectDefinition, stats: &HashMap<String, f32>) -> Option<TickResult> {
        if def.base_duration > 0.0 {
            self.remaining_duration -= dt;
        }

        if def.tick_interval > 0.0 {
            self.tick_timer -= dt;
            if self.tick_timer <= 0.0 {
                self.tick_timer += def.tick_interval;
                return Some(def.tick(self.stack_count, stats, 0.5));
            }
        }

        None
    }

    pub fn is_expired(&self, def: &StatusEffectDefinition) -> bool {
        def.base_duration > 0.0 && self.remaining_duration <= 0.0
    }

    pub fn apply_stack(&mut self, def: &StatusEffectDefinition) {
        match def.stack_rule.stack_behavior {
            StackBehavior::Replace => {
                self.remaining_duration = def.base_duration;
                self.stack_count = 1;
            }
            StackBehavior::Refresh => {
                self.remaining_duration = def.base_duration;
            }
            StackBehavior::AddStack => {
                if self.stack_count < def.stack_rule.max_stacks {
                    self.stack_count += 1;
                }
            }
            StackBehavior::AddStackRefreshAll => {
                self.remaining_duration = def.base_duration;
                if self.stack_count < def.stack_rule.max_stacks {
                    self.stack_count += 1;
                }
            }
            StackBehavior::Ignore => {}
        }
    }
}

// ============================================================
// COOLDOWN SYSTEM
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CooldownCategory {
    Global,
    Melee,
    Ranged,
    Magic,
    Movement,
    Utility,
    Custom(u32),
}

#[derive(Debug, Clone)]
pub struct CooldownEntry {
    pub base_cooldown: f32,
    pub remaining: f32,
    pub category: CooldownCategory,
    pub charges: u32,
    pub max_charges: u32,
    pub charge_recharge_time: f32,
    pub charge_timer: f32,
    pub in_global_cooldown: bool,
}

impl CooldownEntry {
    pub fn new(base_cd: f32, category: CooldownCategory, max_charges: u32) -> Self {
        CooldownEntry {
            base_cooldown: base_cd,
            remaining: 0.0,
            category,
            charges: max_charges,
            max_charges,
            charge_recharge_time: base_cd,
            charge_timer: 0.0,
            in_global_cooldown: false,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.remaining <= 0.0 && self.charges > 0 && !self.in_global_cooldown
    }

    pub fn use_charge(&mut self) {
        if self.charges > 0 {
            self.charges -= 1;
            if self.charges < self.max_charges && self.charge_timer <= 0.0 {
                self.charge_timer = self.charge_recharge_time;
            }
        } else {
            self.remaining = self.base_cooldown;
        }
    }

    pub fn tick(&mut self, dt: f32, gcd_remaining: f32) {
        self.in_global_cooldown = gcd_remaining > 0.0;

        if self.remaining > 0.0 {
            self.remaining = (self.remaining - dt).max(0.0);
        }

        if self.charges < self.max_charges && self.charge_timer > 0.0 {
            self.charge_timer -= dt;
            if self.charge_timer <= 0.0 {
                self.charges += 1;
                if self.charges < self.max_charges {
                    self.charge_timer = self.charge_recharge_time;
                }
            }
        }
    }

    pub fn reduce_with_cdr(&self, cdr_pct: f32) -> f32 {
        // Diminishing returns CDR formula
        let effective_cdr = diminishing_returns_cdr_ability(cdr_pct);
        self.base_cooldown * (1.0 - effective_cdr / 100.0)
    }
}

fn diminishing_returns_cdr_ability(cdr_pct: f32) -> f32 {
    // Soft cap at 40% CDR with hard cap at 50%
    // Formula: effective = cdr / (1 + cdr/100) * 100
    let raw = cdr_pct / 100.0;
    let effective = raw / (1.0 + raw);
    (effective * 100.0).min(50.0)
}

#[derive(Debug, Clone)]
pub struct CooldownManager {
    pub cooldowns: HashMap<u64, CooldownEntry>, // ability_id -> entry
    pub global_cooldown_remaining: f32,
    pub global_cooldown_duration: f32,
}

impl CooldownManager {
    pub fn new() -> Self {
        CooldownManager {
            cooldowns: HashMap::new(),
            global_cooldown_remaining: 0.0,
            global_cooldown_duration: 1.5,
        }
    }

    pub fn register_ability(&mut self, ability_id: u64, base_cd: f32, category: CooldownCategory, max_charges: u32) {
        self.cooldowns.insert(ability_id, CooldownEntry::new(base_cd, category, max_charges));
    }

    pub fn can_use(&self, ability_id: u64) -> bool {
        self.cooldowns.get(&ability_id).map(|e| e.is_ready()).unwrap_or(false)
    }

    pub fn trigger(&mut self, ability_id: u64, triggers_gcd: bool) {
        if let Some(entry) = self.cooldowns.get_mut(&ability_id) {
            entry.use_charge();
        }
        if triggers_gcd {
            self.global_cooldown_remaining = self.global_cooldown_duration;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let gcd = self.global_cooldown_remaining;
        if self.global_cooldown_remaining > 0.0 {
            self.global_cooldown_remaining = (self.global_cooldown_remaining - dt).max(0.0);
        }
        for entry in self.cooldowns.values_mut() {
            entry.tick(dt, gcd);
        }
    }

    pub fn apply_cdr_to_all(&mut self, cdr_pct: f32) {
        for entry in self.cooldowns.values_mut() {
            entry.base_cooldown = entry.reduce_with_cdr(cdr_pct);
            entry.charge_recharge_time = entry.base_cooldown;
        }
    }

    pub fn reset_ability(&mut self, ability_id: u64) {
        if let Some(entry) = self.cooldowns.get_mut(&ability_id) {
            entry.remaining = 0.0;
            entry.charges = entry.max_charges;
        }
    }

    pub fn reset_all(&mut self) {
        for entry in self.cooldowns.values_mut() {
            entry.remaining = 0.0;
            entry.charges = entry.max_charges;
        }
    }
}

// ============================================================
// RESOURCE SYSTEM
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Mana,
    Stamina,
    Energy,
    Rage,
    ComboPoints,
    Heat,
    Charges,
    Focus,
    Fury,
    Essence,
}

impl ResourceType {
    pub fn display_name(&self) -> &'static str {
        match self {
            ResourceType::Mana => "Mana",
            ResourceType::Stamina => "Stamina",
            ResourceType::Energy => "Energy",
            ResourceType::Rage => "Rage",
            ResourceType::ComboPoints => "Combo Points",
            ResourceType::Heat => "Heat",
            ResourceType::Charges => "Charges",
            ResourceType::Focus => "Focus",
            ResourceType::Fury => "Fury",
            ResourceType::Essence => "Essence",
        }
    }

    pub fn color(&self) -> Vec4 {
        match self {
            ResourceType::Mana => Vec4::new(0.2, 0.4, 1.0, 1.0),
            ResourceType::Stamina => Vec4::new(0.1, 0.8, 0.1, 1.0),
            ResourceType::Energy => Vec4::new(1.0, 1.0, 0.0, 1.0),
            ResourceType::Rage => Vec4::new(1.0, 0.0, 0.0, 1.0),
            ResourceType::ComboPoints => Vec4::new(1.0, 0.7, 0.0, 1.0),
            ResourceType::Heat => Vec4::new(1.0, 0.4, 0.0, 1.0),
            ResourceType::Charges => Vec4::new(0.5, 0.5, 1.0, 1.0),
            ResourceType::Focus => Vec4::new(0.3, 1.0, 0.9, 1.0),
            ResourceType::Fury => Vec4::new(0.8, 0.1, 0.1, 1.0),
            ResourceType::Essence => Vec4::new(0.7, 0.2, 0.9, 1.0),
        }
    }

    pub fn base_max(&self) -> f32 {
        match self {
            ResourceType::Mana => 200.0,
            ResourceType::Stamina => 150.0,
            ResourceType::Energy => 100.0,
            ResourceType::Rage => 100.0,
            ResourceType::ComboPoints => 5.0,
            ResourceType::Heat => 100.0,
            ResourceType::Charges => 3.0,
            ResourceType::Focus => 100.0,
            ResourceType::Fury => 100.0,
            ResourceType::Essence => 50.0,
        }
    }

    pub fn decays_over_time(&self) -> bool {
        matches!(self, ResourceType::Rage | ResourceType::Fury | ResourceType::ComboPoints)
    }

    pub fn regens_over_time(&self) -> bool {
        matches!(self, ResourceType::Mana | ResourceType::Stamina | ResourceType::Energy | ResourceType::Focus)
    }

    pub fn generated_by_damage(&self) -> bool {
        matches!(self, ResourceType::Rage | ResourceType::Fury | ResourceType::Heat)
    }
}

#[derive(Debug, Clone)]
pub struct ResourceState {
    pub resource_type: ResourceType,
    pub current: f32,
    pub maximum: f32,
    pub regen_per_second: f32,
    pub decay_per_second: f32,
    pub overflow_behavior: OverflowBehavior,
    pub generation_on_hit: f32,
    pub generation_on_kill: f32,
    pub drain_on_ability: f32,     // resource drained per ability use (e.g. Stamina drain per attack)
    pub minimum: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowBehavior {
    Clamp,
    Wraparound,
    Explode, // full bar triggers an effect (e.g. Heat Overload)
}

impl ResourceState {
    pub fn new(rt: ResourceType) -> Self {
        let max = rt.base_max();
        ResourceState {
            resource_type: rt,
            current: max,
            maximum: max,
            regen_per_second: if rt.regens_over_time() { max * 0.05 } else { 0.0 },
            decay_per_second: if rt.decays_over_time() { max * 0.1 } else { 0.0 },
            overflow_behavior: OverflowBehavior::Clamp,
            generation_on_hit: if rt.generated_by_damage() { max * 0.05 } else { 0.0 },
            generation_on_kill: if rt.generated_by_damage() { max * 0.1 } else { 0.0 },
            drain_on_ability: 0.0,
            minimum: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) -> Option<ResourceOverflowEvent> {
        if self.resource_type.regens_over_time() && self.current < self.maximum {
            self.current += self.regen_per_second * dt;
        }
        if self.resource_type.decays_over_time() && self.current > self.minimum {
            self.current -= self.decay_per_second * dt;
        }

        let old = self.current;
        self.current = self.current.clamp(self.minimum, self.maximum);

        if old > self.maximum && self.overflow_behavior == OverflowBehavior::Explode {
            self.current = self.minimum;
            return Some(ResourceOverflowEvent { resource_type: self.resource_type });
        }

        None
    }

    pub fn spend(&mut self, amount: f32) -> bool {
        if self.current >= amount {
            self.current -= amount;
            true
        } else {
            false
        }
    }

    pub fn gain(&mut self, amount: f32) {
        match self.overflow_behavior {
            OverflowBehavior::Clamp => {
                self.current = (self.current + amount).min(self.maximum);
            }
            OverflowBehavior::Wraparound => {
                self.current = (self.current + amount) % (self.maximum + 1.0);
            }
            OverflowBehavior::Explode => {
                self.current += amount;
                // Overflow handled in tick()
            }
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.maximum <= 0.0 { return 0.0; }
        (self.current / self.maximum).clamp(0.0, 1.0)
    }

    pub fn regen_formula_with_stats(&self, spirit: f32, regen_bonus_pct: f32) -> f32 {
        // spirit increases regen logarithmically to prevent stat inflation
        let spirit_regen = if spirit > 0.0 { (spirit * 0.5).ln().max(0.0) } else { 0.0 };
        self.regen_per_second * (1.0 + regen_bonus_pct / 100.0) + spirit_regen
    }
}

#[derive(Debug, Clone)]
pub struct ResourceOverflowEvent {
    pub resource_type: ResourceType,
}

// ============================================================
// PROJECTILE PARAMETERS
// ============================================================

#[derive(Debug, Clone)]
pub struct ProjectileParams {
    pub speed: f32,
    pub acceleration: f32,     // positive = accelerating, negative = decelerating
    pub gravity: f32,          // downward force
    pub max_range: f32,
    pub pierce_count: u32,     // how many enemies it can pass through
    pub split_count: u32,      // splits into N projectiles on first hit
    pub fork_count: u32,       // forks into N on expiry
    pub chain_count: u32,      // chains to N additional targets
    pub chain_radius: f32,
    pub homing: bool,
    pub homing_strength: f32,  // radians/second angular tracking
    pub aoe_on_impact: bool,
    pub aoe_radius: f32,
    pub visual_trail: String,
    pub impact_effect: String,
    pub size: f32,
}

impl ProjectileParams {
    pub fn default_arrow() -> Self {
        ProjectileParams {
            speed: 25.0,
            acceleration: -2.0,
            gravity: 0.0,
            max_range: 30.0,
            pierce_count: 0,
            split_count: 0,
            fork_count: 0,
            chain_count: 0,
            chain_radius: 0.0,
            homing: false,
            homing_strength: 0.0,
            aoe_on_impact: false,
            aoe_radius: 0.0,
            visual_trail: "arrow_trail".to_string(),
            impact_effect: "arrow_impact".to_string(),
            size: 0.1,
        }
    }

    pub fn default_fireball() -> Self {
        ProjectileParams {
            speed: 18.0,
            acceleration: 0.0,
            gravity: -0.5,
            max_range: 25.0,
            pierce_count: 0,
            split_count: 0,
            fork_count: 0,
            chain_count: 0,
            chain_radius: 0.0,
            homing: false,
            homing_strength: 0.0,
            aoe_on_impact: true,
            aoe_radius: 3.5,
            visual_trail: "fire_trail".to_string(),
            impact_effect: "explosion".to_string(),
            size: 0.4,
        }
    }

    pub fn simulate_trajectory(&self, origin: Vec3, direction: Vec3, dt: f32, steps: u32) -> Vec<Vec3> {
        let mut positions = Vec::new();
        let dir = direction.normalize();
        let mut pos = origin;
        let mut vel = dir * self.speed;
        let gravity_vec = Vec3::new(0.0, -self.gravity, 0.0);

        for _ in 0..steps {
            positions.push(pos);
            let speed = vel.length();
            if speed > 0.0 {
                let drag_dir = vel / speed;
                vel += drag_dir * self.acceleration * dt + gravity_vec * dt;
            }
            pos += vel * dt;

            if (pos - origin).length() > self.max_range { break; }
        }

        positions
    }

    pub fn homing_update(&self, vel: Vec3, to_target: Vec3, dt: f32) -> Vec3 {
        if !self.homing || self.homing_strength <= 0.0 { return vel; }
        let speed = vel.length();
        if speed < 1e-6 { return vel; }
        let current_dir = vel / speed;
        let target_dir = if to_target.length() > 1e-6 { to_target.normalize() } else { current_dir };
        // Rotate current_dir towards target_dir by homing_strength * dt radians
        let angle = current_dir.dot(target_dir).acos().min(self.homing_strength * dt);
        // Compute rotation axis
        let axis = current_dir.cross(target_dir);
        let axis_len = axis.length();
        if axis_len < 1e-6 { return vel; }
        let rotation = Quat::from_axis_angle(axis / axis_len, angle);
        rotation * vel
    }
}

// ============================================================
// ABILITY MODIFIERS (TALENTS / UPGRADES)
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityModifier {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub changes: Vec<AbilityChange>,
    pub unlock_cost: u32,
}

#[derive(Debug, Clone)]
pub enum AbilityChange {
    MultiplyDamage(f32),
    AddDamageFlat(f32),
    MultiplyRange(f32),
    AddRange(f32),
    MultiplyCooldown(f32),
    MultiplyResourceCost(f32),
    AddStatusEffect { effect_id: u64, chance: f32 },
    RemoveStatusEffect { effect_id: u64 },
    AddProjectile(u32),
    AddPierce(u32),
    EnableHoming,
    MultiplyAoeRadius(f32),
    SetTargetingMode(TargetingMode),
    MultiplyDuration(f32),
    MultiplyHeal(f32),
    AddConditionalDamage { condition: String, bonus: f32 },
    TransformToElement(DamageElement),
    AddCharge,
    EnableSplit(u32),
}

impl AbilityModifier {
    pub fn apply_to_definition(&self, def: &mut AbilityDefinition) {
        for change in &self.changes {
            match change {
                AbilityChange::MultiplyDamage(f) => {
                    for formula in &mut def.damage_formulas {
                        formula.base_min *= f;
                        formula.base_max *= f;
                    }
                }
                AbilityChange::AddDamageFlat(v) => {
                    for formula in &mut def.damage_formulas {
                        formula.base_min += v;
                        formula.base_max += v;
                    }
                }
                AbilityChange::MultiplyRange(f) => {
                    def.range *= f;
                }
                AbilityChange::AddRange(v) => {
                    def.range += v;
                }
                AbilityChange::MultiplyCooldown(f) => {
                    def.base_cooldown *= f;
                }
                AbilityChange::MultiplyResourceCost(f) => {
                    def.resource_cost *= f;
                }
                AbilityChange::MultiplyAoeRadius(f) => {
                    def.aoe_radius *= f;
                }
                AbilityChange::MultiplyDuration(f) => {
                    def.duration *= f;
                }
                AbilityChange::MultiplyHeal(f) => {
                    def.heal_formula.base_min *= f;
                    def.heal_formula.base_max *= f;
                }
                AbilityChange::AddCharge => {
                    def.max_charges += 1;
                }
                AbilityChange::EnableSplit(n) => {
                    if let Some(ref mut proj) = def.projectile_params {
                        proj.split_count = *n;
                    }
                }
                AbilityChange::EnableHoming => {
                    if let Some(ref mut proj) = def.projectile_params {
                        proj.homing = true;
                        proj.homing_strength = 2.0;
                    }
                }
                AbilityChange::AddPierce(n) => {
                    if let Some(ref mut proj) = def.projectile_params {
                        proj.pierce_count += n;
                    }
                }
                AbilityChange::AddProjectile(n) => {
                    def.projectile_count += n;
                }
                _ => {}
            }
        }
    }
}

// ============================================================
// CONDITIONAL MODIFIERS / AURA EFFECTS
// ============================================================

#[derive(Debug, Clone)]
pub struct ConditionalModifier {
    pub id: u64,
    pub name: String,
    pub condition: AbilityCondition,
    pub stat_modifier: String,
    pub flat_bonus: f32,
    pub percent_bonus: f32,
    pub duration_limit: Option<f32>, // None = permanent while condition is true
}

#[derive(Debug, Clone)]
pub enum AbilityCondition {
    TargetHasStatus(StatusEffectType),
    TargetBelowHealthPct(f32),
    TargetAboveHealthPct(f32),
    CasterBelowHealthPct(f32),
    CasterHasStatus(StatusEffectType),
    ComboPointsAbove(u32),
    InCombat,
    OutOfCombat,
    AlwaysTrue,
    CasterRageAbove(f32),
}

impl AbilityCondition {
    pub fn evaluate(&self, ctx: &CastContext) -> bool {
        match self {
            AbilityCondition::TargetHasStatus(s) => ctx.target_status_effects.contains(s),
            AbilityCondition::TargetBelowHealthPct(pct) => ctx.target_health_pct < *pct,
            AbilityCondition::TargetAboveHealthPct(pct) => ctx.target_health_pct > *pct,
            AbilityCondition::CasterBelowHealthPct(pct) => ctx.caster_health_pct < *pct,
            AbilityCondition::CasterHasStatus(s) => ctx.caster_status_effects.contains(s),
            AbilityCondition::ComboPointsAbove(n) => ctx.combo_points >= *n,
            AbilityCondition::InCombat => ctx.in_combat,
            AbilityCondition::OutOfCombat => !ctx.in_combat,
            AbilityCondition::AlwaysTrue => true,
            AbilityCondition::CasterRageAbove(r) => ctx.rage >= *r,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CastContext {
    pub caster_id: u64,
    pub target_id: Option<u64>,
    pub caster_stats: HashMap<String, f32>,
    pub target_status_effects: HashSet<StatusEffectType>,
    pub caster_status_effects: HashSet<StatusEffectType>,
    pub target_health_pct: f32,
    pub caster_health_pct: f32,
    pub combo_points: u32,
    pub rage: f32,
    pub in_combat: bool,
    pub cast_position: Vec3,
    pub target_position: Option<Vec3>,
}

impl CastContext {
    pub fn default() -> Self {
        CastContext {
            caster_id: 0,
            target_id: None,
            caster_stats: HashMap::new(),
            target_status_effects: HashSet::new(),
            caster_status_effects: HashSet::new(),
            target_health_pct: 1.0,
            caster_health_pct: 1.0,
            combo_points: 0,
            rage: 0.0,
            in_combat: false,
            cast_position: Vec3::ZERO,
            target_position: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuraEffect {
    pub id: u64,
    pub name: String,
    pub radius: f32,
    pub affects_allies: bool,
    pub affects_enemies: bool,
    pub stat_mods: Vec<ConditionalModifier>,
    pub pulses: bool,
    pub pulse_interval: f32,
    pub pulse_damage: Option<DamageFormula>,
    pub visual_effect: String,
}

impl AuraEffect {
    pub fn targets_in_range<'a>(&self, caster_pos: Vec3, candidates: &'a [(u64, Vec3, bool)]) -> Vec<(u64, Vec3)> {
        // candidates = (id, position, is_ally)
        candidates.iter().filter(|(_, pos, is_ally)| {
            let in_range = (*pos - caster_pos).length() <= self.radius;
            let affected = (self.affects_allies && *is_ally) || (self.affects_enemies && !*is_ally);
            in_range && affected
        }).map(|(id, pos, _)| (*id, *pos)).collect()
    }
}

// ============================================================
// ABILITY DEFINITION
// ============================================================

#[derive(Debug, Clone)]
pub struct HealFormula {
    pub base_min: f32,
    pub base_max: f32,
    pub scaling: Vec<ScalingCoeff>,
    pub overheal_shield: f32, // fraction of excess heal converted to shield
    pub critical_heal_multiplier: f32,
    pub critical_heal_chance: f32,
}

impl HealFormula {
    pub fn zero() -> Self {
        HealFormula {
            base_min: 0.0,
            base_max: 0.0,
            scaling: Vec::new(),
            overheal_shield: 0.0,
            critical_heal_multiplier: 1.5,
            critical_heal_chance: 0.0,
        }
    }

    pub fn compute(&self, stats: &HashMap<String, f32>, roll_t: f32) -> f32 {
        let base = self.base_min + (self.base_max - self.base_min) * roll_t;
        let scaling: f32 = self.scaling.iter().map(|s| {
            s.compute(stats.get(&s.stat_name).copied().unwrap_or(0.0))
        }).sum();
        base + scaling
    }
}

#[derive(Debug, Clone)]
pub struct AbilityDefinition {
    // Identity
    pub id: u64,
    pub name: String,
    pub description: String,
    pub flavor_text: String,
    pub icon_path: String,
    pub ability_type: AbilityType,
    // Targeting
    pub targeting_mode: TargetingMode,
    pub range: f32,
    pub aoe_radius: f32,
    // Timing
    pub cast_time: f32,       // 0 = instant
    pub channel_time: f32,    // 0 = not channeled
    pub channel_ticks: u32,   // how many damage ticks during channel
    pub base_cooldown: f32,
    pub max_charges: u32,
    pub gcd_category: CooldownCategory,
    pub triggers_gcd: bool,
    // Resources
    pub resource_type: ResourceType,
    pub resource_cost: f32,
    pub resource_cost_scaling: Vec<ScalingCoeff>,
    pub resource_generation: f32, // generates this much of the resource on use
    // Damage
    pub damage_formulas: Vec<DamageFormula>,
    pub projectile_params: Option<ProjectileParams>,
    pub projectile_count: u32,
    pub spread_angle: f32,     // angle between multi-projectiles
    // Healing
    pub heal_formula: HealFormula,
    // Status effects
    pub applied_effects: Vec<AppliedEffect>,
    pub required_target_status: Vec<StatusEffectType>, // must have these to be usable
    pub consumed_target_status: Vec<StatusEffectType>, // these are removed on cast
    // Ability flags
    pub duration: f32,         // for buffs/summons/terrain
    pub is_passive: bool,
    pub interrupt_on_move: bool,
    pub usable_while_moving: bool,
    pub usable_while_stunned: bool,
    pub usable_while_silenced: bool,
    pub knockback_force: f32,
    pub knockback_direction: KnockbackDirection,
    // Modifiers
    pub applied_modifiers: Vec<u64>, // AbilityModifier IDs applied
    pub conditional_mods: Vec<ConditionalModifier>,
    pub unlock_level: u32,
    pub talent_tree_id: Option<u64>,
    pub talent_node_id: Option<u64>,
    // Visual / Audio
    pub cast_vfx: String,
    pub hit_vfx: String,
    pub projectile_vfx: String,
    pub cast_sfx: String,
    pub hit_sfx: String,
    pub interrupt_sfx: String,
}

#[derive(Debug, Clone)]
pub struct AppliedEffect {
    pub effect_id: u64,
    pub effect_type: StatusEffectType,
    pub apply_chance: f32,    // 0-100%
    pub to_target: bool,      // true = applied to target, false = applied to caster
    pub condition: Option<AbilityCondition>,
    pub stacks_applied: u32,
    pub duration_override: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnockbackDirection {
    AwayFromCaster,
    TowardCaster,
    Up,
    Fixed(i32), // degrees, encoded as integer for Copy
}

impl AbilityDefinition {
    pub fn new(id: u64, name: impl Into<String>, ability_type: AbilityType) -> Self {
        let targeting = ability_type.default_targeting();
        AbilityDefinition {
            id,
            name: name.into(),
            description: String::new(),
            flavor_text: String::new(),
            icon_path: String::new(),
            ability_type,
            targeting_mode: targeting,
            range: 5.0,
            aoe_radius: 0.0,
            cast_time: 0.0,
            channel_time: 0.0,
            channel_ticks: 0,
            base_cooldown: 1.0,
            max_charges: 1,
            gcd_category: CooldownCategory::Global,
            triggers_gcd: true,
            resource_type: ResourceType::Mana,
            resource_cost: 10.0,
            resource_cost_scaling: Vec::new(),
            resource_generation: 0.0,
            damage_formulas: Vec::new(),
            projectile_params: None,
            projectile_count: 1,
            spread_angle: 0.0,
            heal_formula: HealFormula::zero(),
            applied_effects: Vec::new(),
            required_target_status: Vec::new(),
            consumed_target_status: Vec::new(),
            duration: 0.0,
            is_passive: false,
            interrupt_on_move: false,
            usable_while_moving: true,
            usable_while_stunned: false,
            usable_while_silenced: false,
            knockback_force: 0.0,
            knockback_direction: KnockbackDirection::AwayFromCaster,
            applied_modifiers: Vec::new(),
            conditional_mods: Vec::new(),
            unlock_level: 1,
            talent_tree_id: None,
            talent_node_id: None,
            cast_vfx: String::new(),
            hit_vfx: String::new(),
            projectile_vfx: String::new(),
            cast_sfx: String::new(),
            hit_sfx: String::new(),
            interrupt_sfx: String::new(),
        }
    }

    pub fn compute_resource_cost(&self, stats: &HashMap<String, f32>) -> f32 {
        let scale_bonus: f32 = self.resource_cost_scaling.iter().map(|s| {
            s.compute(stats.get(&s.stat_name).copied().unwrap_or(0.0))
        }).sum();
        (self.resource_cost + scale_bonus).max(0.0)
    }

    pub fn can_cast(&self, ctx: &CastContext, resources: &HashMap<ResourceType, ResourceState>, cooldown_mgr: &CooldownManager) -> CastabilityResult {
        // Check cooldown
        if !cooldown_mgr.can_use(self.id) {
            let cd_entry = cooldown_mgr.cooldowns.get(&self.id);
            let remaining = cd_entry.map(|e| e.remaining).unwrap_or(0.0);
            return CastabilityResult::OnCooldown { remaining_seconds: remaining };
        }

        // Check resources
        let cost = self.compute_resource_cost(&ctx.caster_stats);
        if let Some(res) = resources.get(&self.resource_type) {
            if res.current < cost {
                return CastabilityResult::InsufficientResources {
                    needed: cost,
                    available: res.current,
                    resource: self.resource_type,
                };
            }
        }

        // Check CC
        if self.usable_while_stunned == false && ctx.caster_status_effects.contains(&StatusEffectType::Stun) {
            return CastabilityResult::CastBlocked { reason: "Stunned".to_string() };
        }
        if self.usable_while_silenced == false && ctx.caster_status_effects.contains(&StatusEffectType::Silence) {
            return CastabilityResult::CastBlocked { reason: "Silenced".to_string() };
        }

        // Check required target status
        for req in &self.required_target_status {
            if !ctx.target_status_effects.contains(req) {
                return CastabilityResult::RequirementNotMet {
                    description: format!("Target must have {}", req.display_name()),
                };
            }
        }

        CastabilityResult::Ready
    }

    pub fn compute_damage_preview(&self, ctx: &CastContext, target_resist: f32) -> Vec<DamagePreview> {
        self.damage_formulas.iter().map(|formula| {
            let avg = formula.average_dps_preview(&ctx.caster_stats, target_resist);
            let crit_chance = formula.compute_crit_chance(&ctx.caster_stats);
            let crit_mul = formula.compute_crit_multiplier(&ctx.caster_stats);
            let min_hit = formula.compute_final_damage(&ctx.caster_stats, target_resist, &[], 0.0, 1.0);
            let max_hit = formula.compute_final_damage(&ctx.caster_stats, target_resist, &[], 1.0, 1.0);
            let crit_hit = formula.compute_final_damage(&ctx.caster_stats, target_resist, &[], 0.5, 0.0);
            DamagePreview {
                element: formula.element,
                min_hit: min_hit.final_damage,
                max_hit: max_hit.final_damage,
                avg_hit: avg,
                crit_hit: crit_hit.final_damage,
                crit_chance,
                crit_multiplier: crit_mul,
                effective_resist: min_hit.effective_resist,
            }
        }).collect()
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("Name is empty".to_string()); }
        if self.base_cooldown < 0.0 { errors.push("Cooldown cannot be negative".to_string()); }
        if self.resource_cost < 0.0 { errors.push("Resource cost cannot be negative".to_string()); }
        if self.range < 0.0 { errors.push("Range cannot be negative".to_string()); }
        if self.max_charges == 0 { errors.push("Max charges must be >= 1".to_string()); }
        for formula in &self.damage_formulas {
            if formula.base_min > formula.base_max {
                errors.push(format!("{:?} damage: min > max", formula.element));
            }
        }
        if self.cast_time < 0.0 { errors.push("Cast time cannot be negative".to_string()); }
        errors
    }
}

#[derive(Debug, Clone)]
pub struct DamagePreview {
    pub element: DamageElement,
    pub min_hit: f32,
    pub max_hit: f32,
    pub avg_hit: f32,
    pub crit_hit: f32,
    pub crit_chance: f32,
    pub crit_multiplier: f32,
    pub effective_resist: f32,
}

#[derive(Debug, Clone)]
pub enum CastabilityResult {
    Ready,
    OnCooldown { remaining_seconds: f32 },
    InsufficientResources { needed: f32, available: f32, resource: ResourceType },
    CastBlocked { reason: String },
    RequirementNotMet { description: String },
    OutOfRange { current_distance: f32, required_range: f32 },
}

impl CastabilityResult {
    pub fn is_ready(&self) -> bool {
        matches!(self, CastabilityResult::Ready)
    }

    pub fn display_reason(&self) -> String {
        match self {
            CastabilityResult::Ready => "Ready".to_string(),
            CastabilityResult::OnCooldown { remaining_seconds } => format!("Cooldown: {:.1}s", remaining_seconds),
            CastabilityResult::InsufficientResources { needed, available, resource } =>
                format!("Need {:.0} {} (have {:.0})", needed, resource.display_name(), available),
            CastabilityResult::CastBlocked { reason } => reason.clone(),
            CastabilityResult::RequirementNotMet { description } => description.clone(),
            CastabilityResult::OutOfRange { current_distance, required_range } =>
                format!("Out of range: {:.1}m > {:.1}m", current_distance, required_range),
        }
    }
}

// ============================================================
// TALENT TREE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct TalentNode {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub ability_id: Option<u64>,    // ability unlocked
    pub modifier_id: Option<u64>,   // modifier applied to an ability
    pub passive_stat_mods: Vec<(String, f32, f32)>, // (stat_name, flat, percent)
    pub point_cost: u32,
    pub max_ranks: u32,
    pub current_ranks: u32,
    pub position: Vec2,             // position in the tree UI
    pub icon_path: String,
    pub is_keystone: bool,          // major powerful node
    pub is_notable: bool,           // medium power node
    pub unlock_bonus_at: Option<u32>, // special bonus when fully ranked
}

impl TalentNode {
    pub fn is_fully_ranked(&self) -> bool {
        self.current_ranks >= self.max_ranks
    }

    pub fn can_invest(&self, available_points: u32) -> bool {
        !self.is_fully_ranked() && available_points >= self.point_cost
    }

    pub fn current_stat_mods(&self) -> Vec<(String, f32, f32)> {
        let rank_mul = self.current_ranks as f32;
        self.passive_stat_mods.iter().map(|(name, flat, pct)| {
            (name.clone(), flat * rank_mul, pct * rank_mul)
        }).collect()
    }
}

#[derive(Debug, Clone)]
pub struct TalentEdge {
    pub from_node_id: u64,
    pub to_node_id: u64,
    pub is_prerequisite: bool,       // true = must have from before unlocking to
    pub is_mutual_exclusion: bool,   // true = cannot have both nodes
}

#[derive(Debug, Clone)]
pub struct TalentTreeTier {
    pub tier_index: u32,
    pub node_ids: Vec<u64>,
    pub points_required_to_unlock: u32,
    pub tier_mastery_ability: Option<u64>, // unlocked when all tier nodes maxed
    pub tier_mastery_bonus: Vec<(String, f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct TalentTree {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub class_restriction: Option<String>,
    pub nodes: HashMap<u64, TalentNode>,
    pub edges: Vec<TalentEdge>,
    pub tiers: Vec<TalentTreeTier>,
    pub total_points_invested: u32,
    pub mastery_bonus_thresholds: Vec<(u32, Vec<(String, f32, f32)>)>, // (points, bonuses)
}

impl TalentTree {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        TalentTree {
            id,
            name: name.into(),
            description: String::new(),
            class_restriction: None,
            nodes: HashMap::new(),
            edges: Vec::new(),
            tiers: Vec::new(),
            total_points_invested: 0,
            mastery_bonus_thresholds: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: TalentNode) {
        self.nodes.insert(node.id, node);
    }

    pub fn connect(&mut self, from: u64, to: u64, prerequisite: bool) {
        self.edges.push(TalentEdge { from_node_id: from, to_node_id: to, is_prerequisite: prerequisite, is_mutual_exclusion: false });
    }

    pub fn mutually_exclude(&mut self, a: u64, b: u64) {
        self.edges.push(TalentEdge { from_node_id: a, to_node_id: b, is_prerequisite: false, is_mutual_exclusion: true });
    }

    pub fn prerequisites_met(&self, node_id: u64) -> bool {
        let prereq_edges: Vec<&TalentEdge> = self.edges.iter()
            .filter(|e| e.to_node_id == node_id && e.is_prerequisite)
            .collect();
        prereq_edges.iter().all(|e| {
            self.nodes.get(&e.from_node_id).map(|n| n.current_ranks > 0).unwrap_or(false)
        })
    }

    pub fn mutual_exclusions_clear(&self, node_id: u64) -> bool {
        let excl_edges: Vec<&TalentEdge> = self.edges.iter()
            .filter(|e| e.is_mutual_exclusion && (e.from_node_id == node_id || e.to_node_id == node_id))
            .collect();
        excl_edges.iter().all(|e| {
            let other_id = if e.from_node_id == node_id { e.to_node_id } else { e.from_node_id };
            self.nodes.get(&other_id).map(|n| n.current_ranks == 0).unwrap_or(true)
        })
    }

    pub fn can_invest_in(&self, node_id: u64, available_points: u32) -> bool {
        if let Some(node) = self.nodes.get(&node_id) {
            node.can_invest(available_points) &&
            self.prerequisites_met(node_id) &&
            self.mutual_exclusions_clear(node_id)
        } else {
            false
        }
    }

    pub fn invest_point(&mut self, node_id: u64) -> bool {
        if self.can_invest_in(node_id, 1) {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.current_ranks += 1;
                self.total_points_invested += node.point_cost;
                return true;
            }
        }
        false
    }

    pub fn refund_node(&mut self, node_id: u64) -> bool {
        // Check no dependent nodes are invested
        let has_dependents = self.edges.iter().any(|e| {
            e.is_prerequisite && e.from_node_id == node_id &&
            self.nodes.get(&e.to_node_id).map(|n| n.current_ranks > 0).unwrap_or(false)
        });
        if has_dependents { return false; }

        if let Some(node) = self.nodes.get_mut(&node_id) {
            let cost = node.point_cost * node.current_ranks;
            self.total_points_invested -= cost;
            node.current_ranks = 0;
            return true;
        }
        false
    }

    pub fn active_mastery_bonuses(&self) -> Vec<&(u32, Vec<(String, f32, f32)>)> {
        self.mastery_bonus_thresholds.iter()
            .filter(|(threshold, _)| self.total_points_invested >= *threshold)
            .collect()
    }

    pub fn compute_all_stat_mods(&self) -> Vec<(String, f32, f32)> {
        let mut totals: HashMap<String, (f32, f32)> = HashMap::new();

        for node in self.nodes.values() {
            for (name, flat, pct) in node.current_stat_mods() {
                let entry = totals.entry(name).or_insert((0.0, 0.0));
                entry.0 += flat;
                entry.1 += pct;
            }
        }

        for (_, bonuses) in self.active_mastery_bonuses() {
            for (name, flat, pct) in bonuses {
                let entry = totals.entry(name.clone()).or_insert((0.0, 0.0));
                entry.0 += flat;
                entry.1 += pct;
            }
        }

        totals.into_iter().map(|(k, (f, p))| (k, f, p)).collect()
    }

    pub fn tier_completion_pct(&self, tier_idx: u32) -> f32 {
        if let Some(tier) = self.tiers.iter().find(|t| t.tier_index == tier_idx) {
            let total: u32 = tier.node_ids.iter().map(|id| {
                self.nodes.get(id).map(|n| n.max_ranks).unwrap_or(0)
            }).sum();
            let current: u32 = tier.node_ids.iter().map(|id| {
                self.nodes.get(id).map(|n| n.current_ranks).unwrap_or(0)
            }).sum();
            if total == 0 { return 0.0; }
            current as f32 / total as f32
        } else {
            0.0
        }
    }

    pub fn build_dependency_graph(&self) -> HashMap<u64, Vec<u64>> {
        let mut graph: HashMap<u64, Vec<u64>> = HashMap::new();
        for node_id in self.nodes.keys() {
            graph.entry(*node_id).or_insert_with(Vec::new);
        }
        for edge in &self.edges {
            if edge.is_prerequisite {
                graph.entry(edge.from_node_id).or_insert_with(Vec::new).push(edge.to_node_id);
            }
        }
        graph
    }

    pub fn topological_sort(&self) -> Vec<u64> {
        let graph = self.build_dependency_graph();
        let mut in_degree: HashMap<u64, u32> = HashMap::new();
        for id in self.nodes.keys() { in_degree.insert(*id, 0); }
        for edge in &self.edges {
            if edge.is_prerequisite {
                *in_degree.entry(edge.to_node_id).or_insert(0) += 1;
            }
        }
        let mut queue: VecDeque<u64> = in_degree.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut result = Vec::new();
        while let Some(node_id) = queue.pop_front() {
            result.push(node_id);
            if let Some(deps) = graph.get(&node_id) {
                for &dep in deps {
                    let deg = in_degree.entry(dep).or_insert(1);
                    *deg -= 1;
                    if *deg == 0 { queue.push_back(dep); }
                }
            }
        }
        result
    }
}

// ============================================================
// ABILITY DATABASE
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityDatabase {
    pub abilities: HashMap<u64, AbilityDefinition>,
    pub status_effects: HashMap<u64, StatusEffectDefinition>,
    pub talent_trees: HashMap<u64, TalentTree>,
    pub ability_modifiers: HashMap<u64, AbilityModifier>,
    pub aura_effects: HashMap<u64, AuraEffect>,
    pub next_id: u64,
}

impl AbilityDatabase {
    pub fn new() -> Self {
        AbilityDatabase {
            abilities: HashMap::new(),
            status_effects: HashMap::new(),
            talent_trees: HashMap::new(),
            ability_modifiers: HashMap::new(),
            aura_effects: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_ability(&mut self, ability: AbilityDefinition) {
        self.abilities.insert(ability.id, ability);
    }

    pub fn add_status_effect(&mut self, effect: StatusEffectDefinition) {
        self.status_effects.insert(effect.id, effect);
    }

    pub fn add_talent_tree(&mut self, tree: TalentTree) {
        self.talent_trees.insert(tree.id, tree);
    }

    pub fn abilities_by_type(&self, ability_type: AbilityType) -> Vec<&AbilityDefinition> {
        self.abilities.values().filter(|a| a.ability_type == ability_type).collect()
    }

    pub fn search_abilities(&self, query: &str) -> Vec<&AbilityDefinition> {
        let q = query.to_lowercase();
        let mut results: Vec<&AbilityDefinition> = self.abilities.values().filter(|a| {
            a.name.to_lowercase().contains(&q) ||
            a.description.to_lowercase().contains(&q) ||
            a.ability_type.display_name().to_lowercase().contains(&q)
        }).collect();
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    pub fn abilities_applying_status(&self, effect_type: StatusEffectType) -> Vec<&AbilityDefinition> {
        self.abilities.values().filter(|a| {
            a.applied_effects.iter().any(|e| e.effect_type == effect_type)
        }).collect()
    }

    pub fn stats_summary(&self) -> AbilityDbStats {
        let mut by_type: HashMap<String, u32> = HashMap::new();
        let mut by_resource: HashMap<String, u32> = HashMap::new();
        for a in self.abilities.values() {
            *by_type.entry(a.ability_type.display_name().to_string()).or_insert(0) += 1;
            *by_resource.entry(a.resource_type.display_name().to_string()).or_insert(0) += 1;
        }
        AbilityDbStats {
            total_abilities: self.abilities.len(),
            total_status_effects: self.status_effects.len(),
            total_talent_trees: self.talent_trees.len(),
            by_type,
            by_resource,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AbilityDbStats {
    pub total_abilities: usize,
    pub total_status_effects: usize,
    pub total_talent_trees: usize,
    pub by_type: HashMap<String, u32>,
    pub by_resource: HashMap<String, u32>,
}

// ============================================================
// FORMULA TESTER
// ============================================================

#[derive(Debug, Clone)]
pub struct FormulaTesterConfig {
    pub caster_stats: HashMap<String, f32>,
    pub target_armor: f32,
    pub target_fire_resist: f32,
    pub target_cold_resist: f32,
    pub target_lightning_resist: f32,
    pub target_poison_resist: f32,
    pub target_arcane_resist: f32,
    pub target_health_pct: f32,
    pub target_status_effects: HashSet<StatusEffectType>,
    pub caster_status_effects: HashSet<StatusEffectType>,
    pub combo_points: u32,
    pub sample_count: u32,
}

impl FormulaTesterConfig {
    pub fn default_lvl_20() -> Self {
        let mut stats = HashMap::new();
        stats.insert("strength".to_string(), 50.0);
        stats.insert("dexterity".to_string(), 40.0);
        stats.insert("intelligence".to_string(), 35.0);
        stats.insert("vitality".to_string(), 45.0);
        stats.insert("attack_power".to_string(), 120.0);
        stats.insert("spell_power".to_string(), 80.0);
        stats.insert("crit_chance".to_string(), 15.0);
        stats.insert("crit_multiplier".to_string(), 1.8);
        FormulaTesterConfig {
            caster_stats: stats,
            target_armor: 200.0,
            target_fire_resist: 20.0,
            target_cold_resist: 15.0,
            target_lightning_resist: 10.0,
            target_poison_resist: 5.0,
            target_arcane_resist: 0.0,
            target_health_pct: 1.0,
            target_status_effects: HashSet::new(),
            caster_status_effects: HashSet::new(),
            combo_points: 0,
            sample_count: 1000,
        }
    }

    pub fn resist_for_element(&self, element: DamageElement) -> f32 {
        match element {
            DamageElement::Physical => self.physical_resist_from_armor(),
            DamageElement::Fire => self.target_fire_resist,
            DamageElement::Cold => self.target_cold_resist,
            DamageElement::Lightning => self.target_lightning_resist,
            DamageElement::Poison => self.target_poison_resist,
            DamageElement::Arcane => self.target_arcane_resist,
            DamageElement::Holy | DamageElement::Shadow | DamageElement::Chaos => 0.0,
            DamageElement::True_ => 0.0,
        }
    }

    fn physical_resist_from_armor(&self) -> f32 {
        let pct = self.target_armor / (self.target_armor + 300.0);
        (pct * 100.0).min(75.0)
    }
}

#[derive(Debug, Clone)]
pub struct FormulaTesterResult {
    pub ability_name: String,
    pub damage_previews: Vec<DamagePreview>,
    pub total_avg_damage: f32,
    pub total_min_damage: f32,
    pub total_max_damage: f32,
    pub total_crit_damage: f32,
    pub heal_preview: f32,
    pub resource_cost_preview: f32,
    pub effective_dps: f32,         // total_avg_damage / cooldown
    pub samples: Vec<f32>,          // Monte Carlo samples
    pub percentile_25: f32,
    pub percentile_50: f32,
    pub percentile_75: f32,
    pub percentile_95: f32,
}

impl FormulaTesterResult {
    pub fn compute_percentiles(samples: &mut Vec<f32>) -> (f32, f32, f32, f32) {
        if samples.is_empty() { return (0.0, 0.0, 0.0, 0.0); }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = samples.len();
        let p25 = samples[(n as f32 * 0.25) as usize];
        let p50 = samples[(n as f32 * 0.50) as usize];
        let p75 = samples[(n as f32 * 0.75) as usize];
        let p95 = samples[((n as f32 * 0.95) as usize).min(n - 1)];
        (p25, p50, p75, p95)
    }
}

pub fn run_formula_test(ability: &AbilityDefinition, config: &FormulaTesterConfig) -> FormulaTesterResult {
    let mut samples: Vec<f32> = Vec::new();
    let mut seed: u64 = 99887766;

    let lcg_next_local = |seed: &mut u64| -> f32 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*seed >> 32) as f32) / (u32::MAX as f32)
    };

    let active_statuses: Vec<StatusEffectType> = config.target_status_effects.iter().cloned().collect();

    for _ in 0..config.sample_count {
        let mut total = 0.0f32;
        for formula in &ability.damage_formulas {
            let t = lcg_next_local(&mut seed);
            let crit_roll = lcg_next_local(&mut seed);
            let resist = config.resist_for_element(formula.element);
            let result = formula.compute_final_damage(&config.caster_stats, resist, &active_statuses, t, crit_roll);
            total += result.final_damage;
        }
        samples.push(total);
    }

    let total_sum: f32 = samples.iter().sum();
    let avg = total_sum / samples.len() as f32;
    let min = samples.iter().copied().fold(f32::INFINITY, f32::min);
    let max = samples.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    // Crit samples (force is_crit = true)
    let total_crit: f32 = ability.damage_formulas.iter().map(|formula| {
        let resist = config.resist_for_element(formula.element);
        formula.compute_final_damage(&config.caster_stats, resist, &active_statuses, 0.5, 0.0).final_damage
    }).sum();

    let heal = ability.heal_formula.compute(&config.caster_stats, 0.5);
    let resource_cost = ability.compute_resource_cost(&config.caster_stats);
    let effective_dps = if ability.base_cooldown > 0.0 { avg / ability.base_cooldown } else { avg };

    let damage_previews: Vec<DamagePreview> = {
        let ctx = CastContext::default();
        ability.damage_formulas.iter().map(|f| {
            let resist = config.resist_for_element(f.element);
            let avg_p = f.average_dps_preview(&config.caster_stats, resist);
            let crit_c = f.compute_crit_chance(&config.caster_stats);
            let crit_m = f.compute_crit_multiplier(&config.caster_stats);
            let min_r = f.compute_final_damage(&config.caster_stats, resist, &active_statuses, 0.0, 1.0);
            let max_r = f.compute_final_damage(&config.caster_stats, resist, &active_statuses, 1.0, 1.0);
            let crit_r = f.compute_final_damage(&config.caster_stats, resist, &active_statuses, 0.5, 0.0);
            DamagePreview {
                element: f.element,
                min_hit: min_r.final_damage,
                max_hit: max_r.final_damage,
                avg_hit: avg_p,
                crit_hit: crit_r.final_damage,
                crit_chance: crit_c,
                crit_multiplier: crit_m,
                effective_resist: min_r.effective_resist,
            }
        }).collect()
    };

    let mut sorted_samples = samples.clone();
    let (p25, p50, p75, p95) = FormulaTesterResult::compute_percentiles(&mut sorted_samples);

    FormulaTesterResult {
        ability_name: ability.name.clone(),
        damage_previews,
        total_avg_damage: avg,
        total_min_damage: min,
        total_max_damage: max,
        total_crit_damage: total_crit,
        heal_preview: heal,
        resource_cost_preview: resource_cost,
        effective_dps,
        samples,
        percentile_25: p25,
        percentile_50: p50,
        percentile_75: p75,
        percentile_95: p95,
    }
}

// ============================================================
// STATUS EFFECT TIMELINE PREVIEW
// ============================================================

#[derive(Debug, Clone)]
pub struct TimelineFrame {
    pub time: f32,
    pub active_effects: Vec<(StatusEffectType, u32)>, // (type, stacks)
    pub damage_ticks: Vec<(StatusEffectType, f32)>,
    pub heal_ticks: Vec<(StatusEffectType, f32)>,
    pub resource_events: Vec<(ResourceType, f32)>,
}

pub fn simulate_status_timeline(
    applied_effects: &[(&StatusEffectDefinition, u32)], // (def, initial_stacks)
    caster_stats: &HashMap<String, f32>,
    duration_seconds: f32,
    tick_rate: f32,
) -> Vec<TimelineFrame> {
    let mut frames = Vec::new();
    let dt = tick_rate;
    let steps = (duration_seconds / dt).ceil() as u32;

    let mut active: Vec<ActiveStatusEffect> = applied_effects.iter().map(|(def, stacks)| {
        let mut inst = ActiveStatusEffect::new(def, 0, 0);
        inst.stack_count = *stacks;
        inst
    }).collect();

    let defs: HashMap<u64, &StatusEffectDefinition> = applied_effects.iter().map(|(def, _)| (def.id, *def)).collect();

    for step in 0..steps {
        let time = step as f32 * dt;
        let mut frame = TimelineFrame {
            time,
            active_effects: Vec::new(),
            damage_ticks: Vec::new(),
            heal_ticks: Vec::new(),
            resource_events: Vec::new(),
        };

        let mut to_remove: Vec<usize> = Vec::new();
        for (i, inst) in active.iter_mut().enumerate() {
            if let Some(def) = defs.get(&inst.definition_id) {
                if inst.is_expired(def) {
                    to_remove.push(i);
                    continue;
                }
                if let Some(tick) = inst.tick_update(dt, def, caster_stats) {
                    if tick.damage > 0.0 {
                        frame.damage_ticks.push((inst.effect_type, tick.damage));
                    }
                    if tick.heal > 0.0 {
                        frame.heal_ticks.push((inst.effect_type, tick.heal));
                    }
                }
                frame.active_effects.push((inst.effect_type, inst.stack_count));
            }
        }

        // Remove expired in reverse order
        for &i in to_remove.iter().rev() {
            active.swap_remove(i);
        }

        frames.push(frame);
    }

    frames
}

#[derive(Debug, Clone)]
pub struct TimelineSummary {
    pub total_dot_damage: f32,
    pub total_heal: f32,
    pub duration_cc: f32,
    pub unique_effects: HashSet<StatusEffectType>,
    pub peak_stacks: HashMap<StatusEffectType, u32>,
}

pub fn summarize_timeline(frames: &[TimelineFrame]) -> TimelineSummary {
    let mut total_dot = 0.0f32;
    let mut total_heal = 0.0f32;
    let mut cc_frames = 0u32;
    let mut unique: HashSet<StatusEffectType> = HashSet::new();
    let mut peak: HashMap<StatusEffectType, u32> = HashMap::new();

    for frame in frames {
        for (et, dmg) in &frame.damage_ticks { total_dot += dmg; }
        for (et, heal) in &frame.heal_ticks { total_heal += heal; }
        for (et, stacks) in &frame.active_effects {
            unique.insert(*et);
            let entry = peak.entry(*et).or_insert(0);
            if *stacks > *entry { *entry = *stacks; }
            if et.is_crowd_control() { cc_frames += 1; }
        }
    }

    let dt = if frames.len() > 1 { frames[1].time - frames[0].time } else { 0.1 };
    TimelineSummary {
        total_dot_damage: total_dot,
        total_heal,
        duration_cc: cc_frames as f32 * dt,
        unique_effects: unique,
        peak_stacks: peak,
    }
}

// ============================================================
// ABILITY EDITOR STATE
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityEditorTab {
    AbilityBrowser,
    AbilityEditor,
    StatusEffectEditor,
    TalentTreeEditor,
    FormulaTester,
    StatusTimeline,
    Database,
}

#[derive(Debug, Clone)]
pub struct AbilityEditorState {
    pub current_ability: Option<AbilityDefinition>,
    pub is_dirty: bool,
    pub validation_errors: Vec<String>,
    pub undo_stack: VecDeque<AbilityDefinition>,
    pub redo_stack: VecDeque<AbilityDefinition>,
}

impl AbilityEditorState {
    pub fn new() -> Self {
        AbilityEditorState {
            current_ability: None,
            is_dirty: false,
            validation_errors: Vec::new(),
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    pub fn open_ability(&mut self, ability: AbilityDefinition) {
        self.current_ability = Some(ability);
        self.is_dirty = false;
        self.validation_errors.clear();
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
        if let Some(ref a) = self.current_ability {
            if self.undo_stack.len() >= 50 { self.undo_stack.pop_front(); }
            self.undo_stack.push_back(a.clone());
            self.redo_stack.clear();
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop_back() {
            if let Some(ref cur) = self.current_ability {
                if self.redo_stack.len() >= 50 { self.redo_stack.pop_front(); }
                self.redo_stack.push_back(cur.clone());
            }
            self.current_ability = Some(prev);
            self.is_dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop_back() {
            if let Some(ref cur) = self.current_ability {
                if self.undo_stack.len() >= 50 { self.undo_stack.pop_front(); }
                self.undo_stack.push_back(cur.clone());
            }
            self.current_ability = Some(next);
            self.is_dirty = true;
        }
    }

    pub fn validate(&mut self) {
        if let Some(ref a) = self.current_ability {
            self.validation_errors = a.validate();
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatusEffectEditorState {
    pub current_effect: Option<StatusEffectDefinition>,
    pub is_dirty: bool,
    pub preview_duration: f32,
    pub preview_stats: HashMap<String, f32>,
    pub timeline_preview: Vec<TimelineFrame>,
}

impl StatusEffectEditorState {
    pub fn new() -> Self {
        StatusEffectEditorState {
            current_effect: None,
            is_dirty: false,
            preview_duration: 5.0,
            preview_stats: HashMap::new(),
            timeline_preview: Vec::new(),
        }
    }

    pub fn update_timeline_preview(&mut self) {
        if let Some(ref def) = self.current_effect {
            self.timeline_preview = simulate_status_timeline(
                &[(def, 3)],
                &self.preview_stats,
                self.preview_duration,
                0.1,
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct TalentTreeEditorState {
    pub current_tree_id: Option<u64>,
    pub selected_node_id: Option<u64>,
    pub total_points_available: u32,
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub drag_node: Option<u64>,
    pub pending_connection: Option<(u64, bool)>, // (from_node, is_prerequisite)
}

impl TalentTreeEditorState {
    pub fn new() -> Self {
        TalentTreeEditorState {
            current_tree_id: None,
            selected_node_id: None,
            total_points_available: 0,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            drag_node: None,
            pending_connection: None,
        }
    }

    pub fn screen_to_tree_space(&self, screen_pos: Vec2) -> Vec2 {
        (screen_pos - self.pan_offset) / self.zoom
    }

    pub fn tree_to_screen_space(&self, tree_pos: Vec2) -> Vec2 {
        tree_pos * self.zoom + self.pan_offset
    }

    pub fn node_screen_rect(&self, tree_pos: Vec2, node_size: Vec2) -> Vec4 {
        let screen = self.tree_to_screen_space(tree_pos);
        let sz = node_size * self.zoom;
        Vec4::new(screen.x, screen.y, sz.x, sz.y)
    }

    pub fn hit_test_node(&self, screen_pos: Vec2, nodes: &HashMap<u64, TalentNode>, node_size: Vec2) -> Option<u64> {
        let tree_pos = self.screen_to_tree_space(screen_pos);
        for (id, node) in nodes {
            let rect = node.position;
            let half = node_size * 0.5;
            if (tree_pos - rect).abs().cmple(half).all() {
                return Some(*id);
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct FormulaTesterState {
    pub config: FormulaTesterConfig,
    pub selected_ability_id: Option<u64>,
    pub last_result: Option<FormulaTesterResult>,
    pub show_samples_histogram: bool,
    pub histogram_bins: u32,
    pub compare_ability_id: Option<u64>,
    pub compare_result: Option<FormulaTesterResult>,
}

impl FormulaTesterState {
    pub fn new() -> Self {
        FormulaTesterState {
            config: FormulaTesterConfig::default_lvl_20(),
            selected_ability_id: None,
            last_result: None,
            show_samples_histogram: false,
            histogram_bins: 20,
            compare_ability_id: None,
            compare_result: None,
        }
    }

    pub fn run_test(&mut self, db: &AbilityDatabase) {
        if let Some(id) = self.selected_ability_id {
            if let Some(ability) = db.abilities.get(&id) {
                self.last_result = Some(run_formula_test(ability, &self.config));
            }
        }
        if let Some(id) = self.compare_ability_id {
            if let Some(ability) = db.abilities.get(&id) {
                self.compare_result = Some(run_formula_test(ability, &self.config));
            }
        }
    }

    pub fn build_histogram(&self) -> Vec<(f32, f32, u32)> {
        // Returns Vec<(bin_min, bin_max, count)>
        if let Some(ref result) = self.last_result {
            if result.samples.is_empty() { return Vec::new(); }
            let min = result.samples.iter().copied().fold(f32::INFINITY, f32::min);
            let max = result.samples.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            if (max - min).abs() < 1e-6 { return vec![(min, max, result.samples.len() as u32)]; }
            let bin_width = (max - min) / self.histogram_bins as f32;
            let mut counts = vec![0u32; self.histogram_bins as usize];
            for &s in &result.samples {
                let bin = ((s - min) / bin_width).floor() as usize;
                let bin = bin.min(self.histogram_bins as usize - 1);
                counts[bin] += 1;
            }
            counts.iter().enumerate().map(|(i, &c)| {
                let lo = min + i as f32 * bin_width;
                let hi = lo + bin_width;
                (lo, hi, c)
            }).collect()
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Clone)]
pub struct AbilityBrowserState {
    pub filter_query: String,
    pub filter_types: HashSet<AbilityType>,
    pub filter_resources: HashSet<ResourceType>,
    pub filtered_ids: Vec<u64>,
    pub selected_ids: HashSet<u64>,
    pub sort_column: AbilitySortColumn,
    pub sort_ascending: bool,
    pub scroll_offset: f32,
    pub row_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilitySortColumn {
    Name,
    Type,
    Cooldown,
    ResourceCost,
    Damage,
    Range,
}

impl AbilityBrowserState {
    pub fn new() -> Self {
        AbilityBrowserState {
            filter_query: String::new(),
            filter_types: HashSet::new(),
            filter_resources: HashSet::new(),
            filtered_ids: Vec::new(),
            selected_ids: HashSet::new(),
            sort_column: AbilitySortColumn::Name,
            sort_ascending: true,
            scroll_offset: 0.0,
            row_height: 22.0,
        }
    }

    pub fn refresh(&mut self, db: &AbilityDatabase) {
        let q = self.filter_query.to_lowercase();
        let mut results: Vec<&AbilityDefinition> = db.abilities.values().filter(|a| {
            let name_match = q.is_empty() || a.name.to_lowercase().contains(&q);
            let type_match = self.filter_types.is_empty() || self.filter_types.contains(&a.ability_type);
            let res_match = self.filter_resources.is_empty() || self.filter_resources.contains(&a.resource_type);
            name_match && type_match && res_match
        }).collect();

        let col = self.sort_column;
        let asc = self.sort_ascending;
        results.sort_by(|a, b| {
            let cmp = match col {
                AbilitySortColumn::Name => a.name.cmp(&b.name),
                AbilitySortColumn::Type => a.ability_type.display_name().cmp(b.ability_type.display_name()),
                AbilitySortColumn::Cooldown => a.base_cooldown.partial_cmp(&b.base_cooldown).unwrap_or(std::cmp::Ordering::Equal),
                AbilitySortColumn::ResourceCost => a.resource_cost.partial_cmp(&b.resource_cost).unwrap_or(std::cmp::Ordering::Equal),
                AbilitySortColumn::Damage => {
                    let da: f32 = a.damage_formulas.iter().map(|f| (f.base_min + f.base_max) / 2.0).sum();
                    let db: f32 = b.damage_formulas.iter().map(|f| (f.base_min + f.base_max) / 2.0).sum();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                }
                AbilitySortColumn::Range => a.range.partial_cmp(&b.range).unwrap_or(std::cmp::Ordering::Equal),
            };
            if asc { cmp } else { cmp.reverse() }
        });

        self.filtered_ids = results.iter().map(|a| a.id).collect();
    }

    pub fn visible_rows(&self, viewport_height: f32) -> usize {
        (viewport_height / self.row_height) as usize
    }

    pub fn scroll_to_selected(&mut self) {
        if let Some(&id) = self.selected_ids.iter().next() {
            if let Some(pos) = self.filtered_ids.iter().position(|&i| i == id) {
                self.scroll_offset = pos as f32 * self.row_height;
            }
        }
    }
}

// ============================================================
// MAIN ABILITY EDITOR
// ============================================================

#[derive(Debug)]
pub struct AbilityEditor {
    pub database: AbilityDatabase,
    pub active_tab: AbilityEditorTab,
    pub ability_editor: AbilityEditorState,
    pub status_editor: StatusEffectEditorState,
    pub talent_tree_editor: TalentTreeEditorState,
    pub formula_tester: FormulaTesterState,
    pub browser: AbilityBrowserState,
    pub window_size: Vec2,
    pub panel_split: f32,
    pub status_message: String,
    pub status_timer: f32,
    pub clipboard_ability: Option<AbilityDefinition>,
    pub search_history: VecDeque<String>,
    pub cooldown_manager: CooldownManager,
    pub resource_states: HashMap<ResourceType, ResourceState>,
    pub recently_viewed: VecDeque<u64>,
    pub timeline_target_effects: Vec<u64>, // status effect IDs to preview
}

impl AbilityEditor {
    pub fn new() -> Self {
        let mut resource_states = HashMap::new();
        for rt in [ResourceType::Mana, ResourceType::Stamina, ResourceType::Energy,
                   ResourceType::Rage, ResourceType::ComboPoints, ResourceType::Heat,
                   ResourceType::Focus, ResourceType::Fury, ResourceType::Essence] {
            resource_states.insert(rt, ResourceState::new(rt));
        }
        AbilityEditor {
            database: AbilityDatabase::new(),
            active_tab: AbilityEditorTab::AbilityBrowser,
            ability_editor: AbilityEditorState::new(),
            status_editor: StatusEffectEditorState::new(),
            talent_tree_editor: TalentTreeEditorState::new(),
            formula_tester: FormulaTesterState::new(),
            browser: AbilityBrowserState::new(),
            window_size: Vec2::new(1280.0, 720.0),
            panel_split: 0.35,
            status_message: String::new(),
            status_timer: 0.0,
            clipboard_ability: None,
            search_history: VecDeque::new(),
            cooldown_manager: CooldownManager::new(),
            resource_states,
            recently_viewed: VecDeque::new(),
            timeline_target_effects: Vec::new(),
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_timer = 3.0;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.status_timer > 0.0 {
            self.status_timer = (self.status_timer - dt).max(0.0);
        }
        self.cooldown_manager.tick(dt);
        for res in self.resource_states.values_mut() {
            res.tick(dt);
        }
    }

    pub fn create_new_ability(&mut self, ability_type: AbilityType) {
        let id = self.database.alloc_id();
        let ability = AbilityDefinition::new(id, "New Ability", ability_type);
        self.ability_editor.open_ability(ability);
        self.active_tab = AbilityEditorTab::AbilityEditor;
    }

    pub fn open_ability_by_id(&mut self, id: u64) {
        if let Some(ability) = self.database.abilities.get(&id).cloned() {
            self.ability_editor.open_ability(ability);
            self.active_tab = AbilityEditorTab::AbilityEditor;
            self.recently_viewed.push_front(id);
            if self.recently_viewed.len() > 20 { self.recently_viewed.pop_back(); }
        }
    }

    pub fn save_current_ability(&mut self) -> bool {
        if let Some(ability) = self.ability_editor.current_ability.clone() {
            self.ability_editor.validate();
            if !self.ability_editor.validation_errors.is_empty() {
                self.set_status(format!("Cannot save: {} error(s)", self.ability_editor.validation_errors.len()));
                return false;
            }
            let id = ability.id;
            self.cooldown_manager.register_ability(id, ability.base_cooldown, ability.gcd_category, ability.max_charges);
            self.database.abilities.insert(id, ability);
            self.ability_editor.is_dirty = false;
            self.browser.refresh(&self.database);
            self.set_status("Ability saved.");
            true
        } else {
            false
        }
    }

    pub fn delete_selected_abilities(&mut self) {
        let to_delete: Vec<u64> = self.browser.selected_ids.iter().cloned().collect();
        for id in &to_delete {
            self.database.abilities.remove(id);
            self.cooldown_manager.cooldowns.remove(id);
        }
        self.browser.selected_ids.clear();
        self.browser.refresh(&self.database);
        self.set_status(format!("Deleted {} ability(s).", to_delete.len()));
    }

    pub fn duplicate_ability(&mut self, id: u64) {
        if let Some(a) = self.database.abilities.get(&id).cloned() {
            let new_id = self.database.alloc_id();
            let mut new_a = a;
            new_a.id = new_id;
            new_a.name = format!("{} (Copy)", new_a.name);
            self.database.abilities.insert(new_id, new_a);
            self.browser.refresh(&self.database);
            self.set_status("Ability duplicated.");
        }
    }

    pub fn copy_ability(&mut self) {
        if let Some(ref a) = self.ability_editor.current_ability {
            self.clipboard_ability = Some(a.clone());
            self.set_status("Ability copied to clipboard.");
        }
    }

    pub fn paste_ability(&mut self) {
        if let Some(a) = self.clipboard_ability.clone() {
            let new_id = self.database.alloc_id();
            let mut new_a = a;
            new_a.id = new_id;
            new_a.name = format!("{} (Paste)", new_a.name);
            self.database.abilities.insert(new_id, new_a.clone());
            self.ability_editor.open_ability(new_a);
            self.browser.refresh(&self.database);
            self.set_status("Ability pasted.");
        }
    }

    pub fn apply_modifier_to_current(&mut self, modifier_id: u64) {
        if let Some(ref mut ability) = self.ability_editor.current_ability {
            if let Some(modifier) = self.database.ability_modifiers.get(&modifier_id).cloned() {
                if !ability.applied_modifiers.contains(&modifier_id) {
                    modifier.apply_to_definition(ability);
                    ability.applied_modifiers.push(modifier_id);
                    self.ability_editor.mark_dirty();
                    self.set_status(format!("Applied modifier: {}", modifier.name));
                }
            }
        }
    }

    pub fn run_formula_test_now(&mut self) {
        self.formula_tester.run_test(&self.database);
    }

    pub fn update_status_timeline(&mut self) {
        let mut defs_to_preview: Vec<StatusEffectDefinition> = self.timeline_target_effects.iter()
            .filter_map(|id| self.database.status_effects.get(id).cloned())
            .collect();

        if !defs_to_preview.is_empty() {
            let pairs: Vec<(&StatusEffectDefinition, u32)> = defs_to_preview.iter().map(|d| (d, 1)).collect();
            let stats = HashMap::new();
            let frames = simulate_status_timeline(&pairs, &stats, 10.0, 0.1);
            // Store in status editor or formula tester as needed
        }
    }

    pub fn layout(&self) -> AbilityEditorLayout {
        let left_w = self.window_size.x * self.panel_split;
        let right_w = self.window_size.x - left_w - 2.0;
        let header_h = 40.0;
        let footer_h = 24.0;
        let content_h = self.window_size.y - header_h - footer_h;
        AbilityEditorLayout {
            header: Vec4::new(0.0, 0.0, self.window_size.x, header_h),
            left_panel: Vec4::new(0.0, header_h, left_w, content_h),
            right_panel: Vec4::new(left_w + 2.0, header_h, right_w, content_h),
            footer: Vec4::new(0.0, self.window_size.y - footer_h, self.window_size.x, footer_h),
        }
    }

    pub fn get_ability_tooltip(&self, id: u64) -> Option<AbilityTooltip> {
        let a = self.database.abilities.get(&id)?;
        let mut lines = Vec::new();
        lines.push((a.name.clone(), Vec4::new(1.0, 0.9, 0.5, 1.0)));
        lines.push((a.ability_type.display_name().to_string(), Vec4::new(0.7, 0.7, 0.7, 1.0)));
        lines.push((format!("Range: {:.1}m | CD: {:.1}s", a.range, a.base_cooldown), Vec4::new(0.8, 0.8, 0.8, 1.0)));
        lines.push((format!("Cost: {:.0} {}", a.resource_cost, a.resource_type.display_name()), a.resource_type.color()));
        if !a.damage_formulas.is_empty() {
            for f in &a.damage_formulas {
                lines.push((format!("{}: {:.0}-{:.0}", f.element.display_name(), f.base_min, f.base_max), f.element.color()));
            }
        }
        if a.heal_formula.base_max > 0.0 {
            lines.push((format!("Heals: {:.0}-{:.0}", a.heal_formula.base_min, a.heal_formula.base_max), Vec4::new(0.3, 1.0, 0.5, 1.0)));
        }
        for eff in &a.applied_effects {
            let pct = eff.apply_chance;
            let target = if eff.to_target { "target" } else { "self" };
            lines.push((format!("{:.0}% {}: {} (+{})", pct, target, eff.effect_type.display_name(), eff.stacks_applied), eff.effect_type.color()));
        }
        if !a.description.is_empty() {
            lines.push((a.description.clone(), Vec4::new(0.6, 0.6, 0.9, 1.0)));
        }
        Some(AbilityTooltip { lines })
    }

    pub fn search_with_history(&mut self, query: String) {
        if !query.is_empty() {
            self.search_history.push_front(query.clone());
            if self.search_history.len() > 20 { self.search_history.pop_back(); }
        }
        self.browser.filter_query = query;
        self.browser.refresh(&self.database);
    }

    pub fn export_abilities_json(&self) -> String {
        let mut lines = Vec::new();
        lines.push("[".to_string());
        let mut sorted: Vec<&AbilityDefinition> = self.database.abilities.values().collect();
        sorted.sort_by_key(|a| a.id);
        for (i, a) in sorted.iter().enumerate() {
            let comma = if i + 1 < sorted.len() { "," } else { "" };
            let dmg: String = a.damage_formulas.iter().map(|f| {
                format!("{{\"element\":\"{}\",\"min\":{:.1},\"max\":{:.1}}}", f.element.display_name(), f.base_min, f.base_max)
            }).collect::<Vec<_>>().join(",");
            lines.push(format!(
                "  {{\"id\":{},\"name\":\"{}\",\"type\":\"{}\",\"cd\":{:.2},\"cost\":{:.1},\"resource\":\"{}\",\"range\":{:.1},\"damage\":[{}]}}{}",
                a.id, a.name, a.ability_type.display_name(), a.base_cooldown,
                a.resource_cost, a.resource_type.display_name(), a.range, dmg, comma
            ));
        }
        lines.push("]".to_string());
        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct AbilityTooltip {
    pub lines: Vec<(String, Vec4)>,
}

#[derive(Debug, Clone)]
pub struct AbilityEditorLayout {
    pub header: Vec4,
    pub left_panel: Vec4,
    pub right_panel: Vec4,
    pub footer: Vec4,
}

// ============================================================
// DEFAULT ABILITY DATABASE (starter set)
// ============================================================

pub fn create_starter_ability_database() -> AbilityDatabase {
    let mut db = AbilityDatabase::new();

    // ---- Status Effects ----

    // Burn
    let burn_id = db.alloc_id();
    let burn_formula = DamageFormula::new(DamageElement::Fire, 5.0, 12.0);
    let burn = StatusEffectDefinition {
        id: burn_id,
        effect_type: StatusEffectType::Burn,
        name: "Burn".to_string(),
        description: "Dealing fire damage over time.".to_string(),
        base_duration: 4.0,
        tick_interval: 1.0,
        tick_damage: burn_formula,
        tick_heal: 0.0,
        stat_modifiers: vec![],
        stack_rule: StackRule { max_stacks: 3, stack_behavior: StackBehavior::AddStackRefreshAll, application_behavior: ApplicationBehavior::Always },
        immunity_window: 0.5,
        can_be_cleansed: true,
        can_be_dispelled: false,
        is_debuff: true,
        visual_particle: "fire_burn".to_string(),
        sound_on_apply: "burn_start".to_string(),
        sound_on_tick: "burn_tick".to_string(),
        sound_on_expire: "burn_end".to_string(),
        cc_break_on_damage: false,
        cc_damage_threshold: 0.0,
        interactions: vec![
            StatusInteraction {
                when_hit_by: StatusEffectType::Wet,
                reaction: StatusReaction::Remove,
            }
        ],
    };
    db.add_status_effect(burn);

    // Freeze
    let freeze_id = db.alloc_id();
    let freeze_formula = DamageFormula::new(DamageElement::Cold, 0.0, 0.0);
    let freeze = StatusEffectDefinition {
        id: freeze_id,
        effect_type: StatusEffectType::Freeze,
        name: "Freeze".to_string(),
        description: "Target cannot move or act.".to_string(),
        base_duration: 2.0,
        tick_interval: 0.0,
        tick_damage: freeze_formula,
        tick_heal: 0.0,
        stat_modifiers: vec![
            StatusStatMod { stat: "move_speed".to_string(), flat_delta: 0.0, percent_delta: -100.0, per_stack: false },
        ],
        stack_rule: StackRule { max_stacks: 1, stack_behavior: StackBehavior::Refresh, application_behavior: ApplicationBehavior::Always },
        immunity_window: 3.0,
        can_be_cleansed: true,
        can_be_dispelled: true,
        is_debuff: true,
        visual_particle: "freeze_ice".to_string(),
        sound_on_apply: "freeze_crack".to_string(),
        sound_on_tick: String::new(),
        sound_on_expire: "freeze_shatter".to_string(),
        cc_break_on_damage: true,
        cc_damage_threshold: 50.0,
        interactions: vec![
            StatusInteraction {
                when_hit_by: StatusEffectType::Burn,
                reaction: StatusReaction::Explode {
                    damage_formula: DamageFormula::new(DamageElement::Physical, 20.0, 35.0),
                },
            }
        ],
    };
    db.add_status_effect(freeze);

    // Haste
    let haste_id = db.alloc_id();
    let haste_formula = DamageFormula::new(DamageElement::True_, 0.0, 0.0);
    let haste = StatusEffectDefinition {
        id: haste_id,
        effect_type: StatusEffectType::Haste,
        name: "Haste".to_string(),
        description: "Increased movement and attack speed.".to_string(),
        base_duration: 6.0,
        tick_interval: 0.0,
        tick_damage: haste_formula,
        tick_heal: 0.0,
        stat_modifiers: vec![
            StatusStatMod { stat: "move_speed".to_string(), flat_delta: 0.0, percent_delta: 30.0, per_stack: true },
            StatusStatMod { stat: "attack_speed".to_string(), flat_delta: 0.0, percent_delta: 20.0, per_stack: true },
        ],
        stack_rule: StackRule { max_stacks: 2, stack_behavior: StackBehavior::AddStack, application_behavior: ApplicationBehavior::Always },
        immunity_window: 0.0,
        can_be_cleansed: false,
        can_be_dispelled: true,
        is_debuff: false,
        visual_particle: "haste_glow".to_string(),
        sound_on_apply: "haste_whoosh".to_string(),
        sound_on_tick: String::new(),
        sound_on_expire: "haste_fade".to_string(),
        cc_break_on_damage: false,
        cc_damage_threshold: 0.0,
        interactions: vec![],
    };
    db.add_status_effect(haste);

    // Poison
    let poison_id = db.alloc_id();
    let poison_formula = DamageFormula::new(DamageElement::Poison, 3.0, 8.0);
    let poison = StatusEffectDefinition {
        id: poison_id,
        effect_type: StatusEffectType::Poison,
        name: "Poison".to_string(),
        description: "Deals poison damage over time. Stacks amplify damage.".to_string(),
        base_duration: 8.0,
        tick_interval: 0.5,
        tick_damage: poison_formula,
        tick_heal: 0.0,
        stat_modifiers: vec![
            StatusStatMod { stat: "healing_received".to_string(), flat_delta: 0.0, percent_delta: -20.0, per_stack: false },
        ],
        stack_rule: StackRule { max_stacks: 5, stack_behavior: StackBehavior::AddStackRefreshAll, application_behavior: ApplicationBehavior::Always },
        immunity_window: 0.0,
        can_be_cleansed: true,
        can_be_dispelled: false,
        is_debuff: true,
        visual_particle: "poison_bubbles".to_string(),
        sound_on_apply: "poison_hiss".to_string(),
        sound_on_tick: String::new(),
        sound_on_expire: "poison_end".to_string(),
        cc_break_on_damage: false,
        cc_damage_threshold: 0.0,
        interactions: vec![],
    };
    db.add_status_effect(poison);

    // Stun
    let stun_id = db.alloc_id();
    let stun_formula = DamageFormula::new(DamageElement::True_, 0.0, 0.0);
    let stun = StatusEffectDefinition {
        id: stun_id,
        effect_type: StatusEffectType::Stun,
        name: "Stun".to_string(),
        description: "Target is completely incapacitated.".to_string(),
        base_duration: 1.5,
        tick_interval: 0.0,
        tick_damage: stun_formula,
        tick_heal: 0.0,
        stat_modifiers: vec![],
        stack_rule: StackRule { max_stacks: 1, stack_behavior: StackBehavior::Refresh, application_behavior: ApplicationBehavior::Always },
        immunity_window: 4.0,
        can_be_cleansed: true,
        can_be_dispelled: true,
        is_debuff: true,
        visual_particle: "stun_stars".to_string(),
        sound_on_apply: "stun_hit".to_string(),
        sound_on_tick: String::new(),
        sound_on_expire: "stun_clear".to_string(),
        cc_break_on_damage: false,
        cc_damage_threshold: 0.0,
        interactions: vec![],
    };
    db.add_status_effect(stun);

    // Bleed
    let bleed_id = db.alloc_id();
    let mut bleed_formula = DamageFormula::new(DamageElement::Physical, 4.0, 10.0);
    bleed_formula.scaling.push(ScalingCoeff::linear("strength", 0.15));
    let bleed = StatusEffectDefinition {
        id: bleed_id,
        effect_type: StatusEffectType::Bleed,
        name: "Bleed".to_string(),
        description: "Dealing physical damage over time, scaling with attacker strength.".to_string(),
        base_duration: 5.0,
        tick_interval: 0.5,
        tick_damage: bleed_formula,
        tick_heal: 0.0,
        stat_modifiers: vec![],
        stack_rule: StackRule { max_stacks: 8, stack_behavior: StackBehavior::AddStack, application_behavior: ApplicationBehavior::Always },
        immunity_window: 0.0,
        can_be_cleansed: true,
        can_be_dispelled: false,
        is_debuff: true,
        visual_particle: "bleed_drips".to_string(),
        sound_on_apply: "bleed_slash".to_string(),
        sound_on_tick: "bleed_drip".to_string(),
        sound_on_expire: "bleed_close".to_string(),
        cc_break_on_damage: false,
        cc_damage_threshold: 0.0,
        interactions: vec![],
    };
    db.add_status_effect(bleed);

    // ---- Abilities ----

    // Basic Attack (melee)
    let basic_atk_id = db.alloc_id();
    let mut basic_atk = AbilityDefinition::new(basic_atk_id, "Basic Attack", AbilityType::MeleeAttack);
    basic_atk.description = "A standard melee strike.".to_string();
    basic_atk.range = 2.5;
    basic_atk.base_cooldown = 0.0;
    basic_atk.resource_cost = 0.0;
    basic_atk.triggers_gcd = true;
    let mut atk_formula = DamageFormula::new(DamageElement::Physical, 10.0, 18.0);
    atk_formula.scaling.push(ScalingCoeff::linear("attack_power", 1.0));
    atk_formula.crit_chance_base = 5.0;
    basic_atk.damage_formulas.push(atk_formula);
    basic_atk.applied_effects.push(AppliedEffect {
        effect_id: bleed_id,
        effect_type: StatusEffectType::Bleed,
        apply_chance: 10.0,
        to_target: true,
        condition: None,
        stacks_applied: 1,
        duration_override: None,
    });
    db.add_ability(basic_atk);

    // Fireball
    let fireball_id = db.alloc_id();
    let mut fireball = AbilityDefinition::new(fireball_id, "Fireball", AbilityType::Projectile);
    fireball.description = "Hurls a ball of fire that explodes on impact.".to_string();
    fireball.range = 25.0;
    fireball.aoe_radius = 3.5;
    fireball.base_cooldown = 3.0;
    fireball.cast_time = 0.8;
    fireball.resource_type = ResourceType::Mana;
    fireball.resource_cost = 30.0;
    fireball.projectile_params = Some(ProjectileParams::default_fireball());
    let mut fire_formula = DamageFormula::new(DamageElement::Fire, 40.0, 65.0);
    fire_formula.scaling.push(ScalingCoeff::linear("spell_power", 1.2));
    fire_formula.crit_chance_base = 8.0;
    fire_formula.crit_multiplier_base = 1.8;
    fire_formula.versus_status_bonus.push((StatusEffectType::Burn, 0.3));
    fireball.damage_formulas.push(fire_formula);
    fireball.applied_effects.push(AppliedEffect {
        effect_id: burn_id,
        effect_type: StatusEffectType::Burn,
        apply_chance: 50.0,
        to_target: true,
        condition: None,
        stacks_applied: 1,
        duration_override: None,
    });
    fireball.targeting_mode = TargetingMode::GroundTarget { indicator: AreaIndicator::Circle };
    db.add_ability(fireball);

    // Frost Nova
    let frost_nova_id = db.alloc_id();
    let mut frost_nova = AbilityDefinition::new(frost_nova_id, "Frost Nova", AbilityType::Nova);
    frost_nova.description = "Releases a burst of frost in all directions, freezing nearby enemies.".to_string();
    frost_nova.range = 0.0;
    frost_nova.aoe_radius = 6.0;
    frost_nova.base_cooldown = 12.0;
    frost_nova.resource_cost = 25.0;
    frost_nova.targeting_mode = TargetingMode::Self_;
    let mut nova_formula = DamageFormula::new(DamageElement::Cold, 15.0, 25.0);
    nova_formula.scaling.push(ScalingCoeff::linear("spell_power", 0.6));
    frost_nova.damage_formulas.push(nova_formula);
    frost_nova.applied_effects.push(AppliedEffect {
        effect_id: freeze_id,
        effect_type: StatusEffectType::Freeze,
        apply_chance: 80.0,
        to_target: true,
        condition: None,
        stacks_applied: 1,
        duration_override: None,
    });
    db.add_ability(frost_nova);

    // Haste Buff
    let haste_ability_id = db.alloc_id();
    let mut haste_ability = AbilityDefinition::new(haste_ability_id, "Swiftness Aura", AbilityType::Buff);
    haste_ability.description = "Grants haste to the caster.".to_string();
    haste_ability.base_cooldown = 20.0;
    haste_ability.resource_cost = 15.0;
    haste_ability.resource_type = ResourceType::Mana;
    haste_ability.duration = 8.0;
    haste_ability.targeting_mode = TargetingMode::Self_;
    haste_ability.applied_effects.push(AppliedEffect {
        effect_id: haste_id,
        effect_type: StatusEffectType::Haste,
        apply_chance: 100.0,
        to_target: false,
        condition: None,
        stacks_applied: 1,
        duration_override: None,
    });
    db.add_ability(haste_ability);

    // Whirlwind (AoE melee)
    let whirlwind_id = db.alloc_id();
    let mut whirlwind = AbilityDefinition::new(whirlwind_id, "Whirlwind", AbilityType::AreaOfEffect);
    whirlwind.description = "Spin and deal damage to all nearby enemies.".to_string();
    whirlwind.range = 0.0;
    whirlwind.aoe_radius = 5.0;
    whirlwind.base_cooldown = 8.0;
    whirlwind.resource_type = ResourceType::Stamina;
    whirlwind.resource_cost = 35.0;
    whirlwind.cast_time = 0.3;
    whirlwind.duration = 1.5;
    whirlwind.targeting_mode = TargetingMode::Self_;
    let mut ww_formula = DamageFormula::new(DamageElement::Physical, 25.0, 40.0);
    ww_formula.scaling.push(ScalingCoeff::linear("attack_power", 0.9));
    ww_formula.scaling.push(ScalingCoeff::linear("strength", 0.3));
    whirlwind.damage_formulas.push(ww_formula);
    whirlwind.applied_effects.push(AppliedEffect {
        effect_id: bleed_id,
        effect_type: StatusEffectType::Bleed,
        apply_chance: 30.0,
        to_target: true,
        condition: None,
        stacks_applied: 2,
        duration_override: None,
    });
    db.add_ability(whirlwind);

    // Shadow Step (Dash / Teleport)
    let shadow_step_id = db.alloc_id();
    let mut shadow_step = AbilityDefinition::new(shadow_step_id, "Shadow Step", AbilityType::Dash);
    shadow_step.description = "Teleport behind target and deal damage.".to_string();
    shadow_step.range = 15.0;
    shadow_step.base_cooldown = 10.0;
    shadow_step.resource_type = ResourceType::Energy;
    shadow_step.resource_cost = 20.0;
    shadow_step.targeting_mode = TargetingMode::SingleTarget;
    let mut step_formula = DamageFormula::new(DamageElement::Shadow, 30.0, 50.0);
    step_formula.scaling.push(ScalingCoeff::linear("dexterity", 0.5));
    step_formula.crit_chance_base = 25.0; // backstab bonus
    step_formula.crit_multiplier_base = 2.5;
    shadow_step.damage_formulas.push(step_formula);
    shadow_step.applied_effects.push(AppliedEffect {
        effect_id: stun_id,
        effect_type: StatusEffectType::Stun,
        apply_chance: 20.0,
        to_target: true,
        condition: None,
        stacks_applied: 1,
        duration_override: Some(0.75),
    });
    db.add_ability(shadow_step);

    // Rain of Arrows (Channel)
    let rain_arrows_id = db.alloc_id();
    let mut rain_arrows = AbilityDefinition::new(rain_arrows_id, "Rain of Arrows", AbilityType::Channel);
    rain_arrows.description = "Channels a barrage of arrows onto a target area.".to_string();
    rain_arrows.range = 30.0;
    rain_arrows.aoe_radius = 5.0;
    rain_arrows.base_cooldown = 15.0;
    rain_arrows.cast_time = 0.0;
    rain_arrows.channel_time = 3.0;
    rain_arrows.channel_ticks = 6;
    rain_arrows.resource_type = ResourceType::Mana;
    rain_arrows.resource_cost = 50.0;
    rain_arrows.targeting_mode = TargetingMode::GroundTarget { indicator: AreaIndicator::Circle };
    let mut arrow_formula = DamageFormula::new(DamageElement::Physical, 12.0, 20.0);
    arrow_formula.scaling.push(ScalingCoeff::linear("dexterity", 0.6));
    rain_arrows.damage_formulas.push(arrow_formula);
    db.add_ability(rain_arrows);

    // Summon Wolf
    let summon_wolf_id = db.alloc_id();
    let mut summon_wolf = AbilityDefinition::new(summon_wolf_id, "Summon Wolf", AbilityType::Summon);
    summon_wolf.description = "Summons a wolf companion to fight for you.".to_string();
    summon_wolf.range = 5.0;
    summon_wolf.base_cooldown = 30.0;
    summon_wolf.resource_cost = 60.0;
    summon_wolf.duration = 60.0;
    summon_wolf.resource_type = ResourceType::Mana;
    db.add_ability(summon_wolf);

    // Poison Arrow
    let poison_arrow_id = db.alloc_id();
    let mut poison_arrow = AbilityDefinition::new(poison_arrow_id, "Poison Arrow", AbilityType::RangedAttack);
    poison_arrow.description = "Fire a poisoned arrow that applies stacking poison.".to_string();
    poison_arrow.range = 25.0;
    poison_arrow.base_cooldown = 4.0;
    poison_arrow.resource_type = ResourceType::Mana;
    poison_arrow.resource_cost = 15.0;
    poison_arrow.projectile_params = Some(ProjectileParams::default_arrow());
    let pa_formula = DamageFormula::new(DamageElement::Poison, 8.0, 14.0);
    poison_arrow.damage_formulas.push(pa_formula);
    poison_arrow.applied_effects.push(AppliedEffect {
        effect_id: poison_id,
        effect_type: StatusEffectType::Poison,
        apply_chance: 100.0,
        to_target: true,
        condition: None,
        stacks_applied: 2,
        duration_override: None,
    });
    db.add_ability(poison_arrow);

    // Lightning Strike (Chain)
    let lightning_id = db.alloc_id();
    let mut lightning = AbilityDefinition::new(lightning_id, "Chain Lightning", AbilityType::ChainLightning);
    lightning.description = "Lightning that bounces between enemies.".to_string();
    lightning.range = 20.0;
    lightning.base_cooldown = 5.0;
    lightning.resource_cost = 35.0;
    lightning.targeting_mode = TargetingMode::Chain { max_bounces: 4, bounce_radius: 8.0, falloff_per_bounce: 0.3 };
    let mut chain_formula = DamageFormula::new(DamageElement::Lightning, 30.0, 55.0);
    chain_formula.scaling.push(ScalingCoeff::linear("spell_power", 1.0));
    chain_formula.crit_chance_base = 10.0;
    chain_formula.versus_status_bonus.push((StatusEffectType::Wet, 0.5));
    lightning.damage_formulas.push(chain_formula);
    db.add_ability(lightning);

    // Heal
    let heal_id = db.alloc_id();
    let mut heal_ability = AbilityDefinition::new(heal_id, "Healing Touch", AbilityType::Heal);
    heal_ability.description = "Restores health to the target.".to_string();
    heal_ability.range = 30.0;
    heal_ability.base_cooldown = 0.0;
    heal_ability.cast_time = 1.5;
    heal_ability.resource_type = ResourceType::Mana;
    heal_ability.resource_cost = 40.0;
    heal_ability.targeting_mode = TargetingMode::SingleTarget;
    let mut h_formula = HealFormula {
        base_min: 50.0,
        base_max: 80.0,
        scaling: vec![ScalingCoeff::linear("spell_power", 1.5)],
        overheal_shield: 0.2,
        critical_heal_multiplier: 1.5,
        critical_heal_chance: 10.0,
    };
    heal_ability.heal_formula = h_formula;
    db.add_ability(heal_ability);

    // ---- Talent Tree ----

    let warrior_tree_id = db.alloc_id();
    let mut warrior_tree = TalentTree::new(warrior_tree_id, "Warrior Mastery");
    warrior_tree.description = "Enhances melee combat and physical abilities.".to_string();

    let node1_id = db.alloc_id();
    warrior_tree.add_node(TalentNode {
        id: node1_id,
        name: "Iron Skin".to_string(),
        description: "+5 armor per rank".to_string(),
        ability_id: None,
        modifier_id: None,
        passive_stat_mods: vec![("physical_armor".to_string(), 5.0, 0.0)],
        point_cost: 1,
        max_ranks: 5,
        current_ranks: 0,
        position: Vec2::new(0.0, 0.0),
        icon_path: "node_ironskin".to_string(),
        is_keystone: false,
        is_notable: false,
        unlock_bonus_at: Some(5),
    });

    let node2_id = db.alloc_id();
    warrior_tree.add_node(TalentNode {
        id: node2_id,
        name: "Berserker Rage".to_string(),
        description: "+3% attack speed per rank".to_string(),
        ability_id: None,
        modifier_id: None,
        passive_stat_mods: vec![("attack_speed".to_string(), 0.0, 3.0)],
        point_cost: 1,
        max_ranks: 5,
        current_ranks: 0,
        position: Vec2::new(100.0, 0.0),
        icon_path: "node_berserker".to_string(),
        is_keystone: false,
        is_notable: true,
        unlock_bonus_at: None,
    });

    let node3_id = db.alloc_id();
    warrior_tree.add_node(TalentNode {
        id: node3_id,
        name: "Mighty Blow".to_string(),
        description: "Whirlwind now stuns enemies for 0.5s (keystone)".to_string(),
        ability_id: Some(whirlwind_id),
        modifier_id: None,
        passive_stat_mods: vec![],
        point_cost: 2,
        max_ranks: 1,
        current_ranks: 0,
        position: Vec2::new(50.0, 100.0),
        icon_path: "node_mightyblow".to_string(),
        is_keystone: true,
        is_notable: true,
        unlock_bonus_at: None,
    });

    warrior_tree.connect(node1_id, node3_id, true);
    warrior_tree.connect(node2_id, node3_id, true);

    warrior_tree.mastery_bonus_thresholds.push((
        10,
        vec![("physical_damage".to_string(), 10.0, 0.0)],
    ));
    warrior_tree.mastery_bonus_thresholds.push((
        20,
        vec![("critical_chance".to_string(), 5.0, 0.0), ("critical_multiplier".to_string(), 0.0, 25.0)],
    ));

    warrior_tree.tiers.push(TalentTreeTier {
        tier_index: 0,
        node_ids: vec![node1_id, node2_id],
        points_required_to_unlock: 0,
        tier_mastery_ability: None,
        tier_mastery_bonus: vec![("stamina".to_string(), 10.0, 0.0)],
    });
    warrior_tree.tiers.push(TalentTreeTier {
        tier_index: 1,
        node_ids: vec![node3_id],
        points_required_to_unlock: 5,
        tier_mastery_ability: Some(whirlwind_id),
        tier_mastery_bonus: vec![("attack_power".to_string(), 20.0, 0.0)],
    });

    db.add_talent_tree(warrior_tree);

    // ---- Ability Modifiers ----

    let burning_fireball_mod = AbilityModifier {
        id: db.alloc_id(),
        name: "Burning Barrage".to_string(),
        description: "Fireball deals 20% more damage and always applies Burn.".to_string(),
        changes: vec![
            AbilityChange::MultiplyDamage(1.2),
        ],
        unlock_cost: 2,
    };
    db.ability_modifiers.insert(burning_fireball_mod.id, burning_fireball_mod);

    let extra_pierce_mod = AbilityModifier {
        id: db.alloc_id(),
        name: "Piercing Shot".to_string(),
        description: "Arrow pierces through 2 additional enemies.".to_string(),
        changes: vec![AbilityChange::AddPierce(2)],
        unlock_cost: 1,
    };
    db.ability_modifiers.insert(extra_pierce_mod.id, extra_pierce_mod);

    let homing_mod = AbilityModifier {
        id: db.alloc_id(),
        name: "Seeking Flame".to_string(),
        description: "Fireball now homes toward targets.".to_string(),
        changes: vec![AbilityChange::EnableHoming],
        unlock_cost: 3,
    };
    db.ability_modifiers.insert(homing_mod.id, homing_mod);

    // Aura
    let aura = AuraEffect {
        id: db.alloc_id(),
        name: "Warlord's Presence".to_string(),
        radius: 12.0,
        affects_allies: true,
        affects_enemies: false,
        stat_mods: vec![
            ConditionalModifier {
                id: 0,
                name: "Warlord Bonus".to_string(),
                condition: AbilityCondition::AlwaysTrue,
                stat_modifier: "attack_power".to_string(),
                flat_bonus: 15.0,
                percent_bonus: 0.0,
                duration_limit: None,
            }
        ],
        pulses: false,
        pulse_interval: 0.0,
        pulse_damage: None,
        visual_effect: "warlord_aura".to_string(),
    };
    db.aura_effects.insert(aura.id, aura);

    db
}

// ============================================================
// ABILITY COMPARISON
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityComparison {
    pub a: AbilityDefinition,
    pub b: AbilityDefinition,
    pub damage_delta: f32,  // positive = B deals more
    pub heal_delta: f32,
    pub cooldown_delta: f32,
    pub range_delta: f32,
    pub cost_delta: f32,
    pub effective_dps_a: f32,
    pub effective_dps_b: f32,
    pub notes: Vec<String>,
}

pub fn compare_abilities(a: &AbilityDefinition, b: &AbilityDefinition, config: &FormulaTesterConfig) -> AbilityComparison {
    let test_a = run_formula_test(a, config);
    let test_b = run_formula_test(b, config);

    let mut notes = Vec::new();
    if a.resource_type != b.resource_type {
        notes.push(format!("Resource types differ: {} vs {}", a.resource_type.display_name(), b.resource_type.display_name()));
    }
    if a.targeting_mode != b.targeting_mode {
        notes.push(format!("Targeting modes differ: {} vs {}", a.targeting_mode.display_name(), b.targeting_mode.display_name()));
    }
    if !a.applied_effects.is_empty() || !b.applied_effects.is_empty() {
        notes.push(format!("A applies {} effects, B applies {} effects", a.applied_effects.len(), b.applied_effects.len()));
    }

    AbilityComparison {
        a: a.clone(),
        b: b.clone(),
        damage_delta: test_b.total_avg_damage - test_a.total_avg_damage,
        heal_delta: test_b.heal_preview - test_a.heal_preview,
        cooldown_delta: b.base_cooldown - a.base_cooldown,
        range_delta: b.range - a.range,
        cost_delta: b.resource_cost - a.resource_cost,
        effective_dps_a: test_a.effective_dps,
        effective_dps_b: test_b.effective_dps,
        notes,
    }
}

// ============================================================
// CAST QUEUE SIMULATION
// ============================================================

#[derive(Debug, Clone)]
pub struct CastQueueEntry {
    pub ability_id: u64,
    pub target_id: Option<u64>,
    pub target_pos: Option<Vec3>,
    pub queued_at: f32,
    pub execute_at: f32,
}

#[derive(Debug, Clone)]
pub struct CastResult {
    pub ability_id: u64,
    pub success: bool,
    pub damage_results: Vec<DamageResult>,
    pub heal_amount: f32,
    pub effects_applied: Vec<(u64, StatusEffectType)>, // (target_id, effect_type)
    pub resource_consumed: f32,
    pub cast_time: f32,
}

pub struct CastSimulator {
    pub ability_db: AbilityDatabase,
    pub status_db: HashMap<u64, StatusEffectDefinition>,
    pub cooldown_mgr: CooldownManager,
    pub resources: HashMap<ResourceType, ResourceState>,
    pub current_time: f32,
    pub cast_queue: VecDeque<CastQueueEntry>,
    pub cast_log: VecDeque<CastResult>,
}

impl CastSimulator {
    pub fn new(ability_db: AbilityDatabase) -> Self {
        let mut resources = HashMap::new();
        for rt in [ResourceType::Mana, ResourceType::Stamina, ResourceType::Energy] {
            resources.insert(rt, ResourceState::new(rt));
        }
        let mut mgr = CooldownManager::new();
        for (id, a) in &ability_db.abilities {
            mgr.register_ability(*id, a.base_cooldown, a.gcd_category, a.max_charges);
        }
        CastSimulator {
            ability_db,
            status_db: HashMap::new(),
            cooldown_mgr: mgr,
            resources,
            current_time: 0.0,
            cast_queue: VecDeque::new(),
            cast_log: VecDeque::new(),
        }
    }

    pub fn queue_ability(&mut self, ability_id: u64, target_id: Option<u64>, target_pos: Option<Vec3>) {
        if let Some(ability) = self.ability_db.abilities.get(&ability_id) {
            let execute_at = self.current_time + ability.cast_time;
            self.cast_queue.push_back(CastQueueEntry {
                ability_id,
                target_id,
                target_pos,
                queued_at: self.current_time,
                execute_at,
            });
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.current_time += dt;
        self.cooldown_mgr.tick(dt);
        for r in self.resources.values_mut() { r.tick(dt); }

        let mut to_execute = Vec::new();
        while let Some(entry) = self.cast_queue.front() {
            if entry.execute_at <= self.current_time {
                to_execute.push(self.cast_queue.pop_front().unwrap());
            } else {
                break;
            }
        }

        let default_stats: HashMap<String, f32> = {
            let mut m = HashMap::new();
            m.insert("attack_power".to_string(), 100.0);
            m.insert("spell_power".to_string(), 80.0);
            m
        };

        for entry in to_execute {
            let result = self.execute_ability(entry.ability_id, &default_stats, 0.0, 0.5);
            if let Some(r) = result {
                if self.cast_log.len() >= 200 { self.cast_log.pop_front(); }
                self.cast_log.push_back(r);
            }
        }
    }

    fn execute_ability(&mut self, ability_id: u64, stats: &HashMap<String, f32>, target_resist: f32, roll: f32) -> Option<CastResult> {
        let ability = self.ability_db.abilities.get(&ability_id)?.clone();

        // Check resources
        let cost = ability.compute_resource_cost(stats);
        let res = self.resources.get_mut(&ability.resource_type)?;
        if !res.spend(cost) { return None; }

        self.cooldown_mgr.trigger(ability_id, ability.triggers_gcd);

        let mut damage_results = Vec::new();
        for formula in &ability.damage_formulas {
            let resist = target_resist;
            let result = formula.compute_final_damage(stats, resist, &[], roll, roll * 0.7);
            damage_results.push(result);
        }

        let heal = ability.heal_formula.compute(stats, roll);

        let mut effects_applied = Vec::new();
        for eff in &ability.applied_effects {
            // roll would normally be random; use roll for determinism in sim
            if roll * 100.0 < eff.apply_chance {
                effects_applied.push((0u64, eff.effect_type));
            }
        }

        Some(CastResult {
            ability_id,
            success: true,
            damage_results,
            heal_amount: heal,
            effects_applied,
            resource_consumed: cost,
            cast_time: ability.cast_time,
        })
    }

    pub fn total_simulated_damage(&self) -> f32 {
        self.cast_log.iter().map(|r| {
            r.damage_results.iter().map(|d| d.final_damage).sum::<f32>()
        }).sum()
    }

    pub fn total_simulated_healing(&self) -> f32 {
        self.cast_log.iter().map(|r| r.heal_amount).sum()
    }
}

// ============================================================
// UI LAYOUT HELPERS (targeting zone visualization)
// ============================================================

#[derive(Debug, Clone)]
pub struct TargetingVisualizer {
    pub origin: Vec3,
    pub forward: Vec3,
    pub mode: TargetingMode,
    pub resolution: u32, // arc/circle segment count
}

impl TargetingVisualizer {
    pub fn new(origin: Vec3, forward: Vec3, mode: TargetingMode) -> Self {
        TargetingVisualizer { origin, forward, mode, resolution: 32 }
    }

    pub fn generate_outline_points_2d(&self) -> Vec<Vec2> {
        let origin_2d = Vec2::new(self.origin.x, self.origin.z);
        let forward_2d = Vec2::new(self.forward.x, self.forward.z).normalize_or_zero();
        let angle_fwd = forward_2d.y.atan2(forward_2d.x);

        match &self.mode {
            TargetingMode::AoECircle { radius } | TargetingMode::AoESphere { radius } => {
                (0..=self.resolution).map(|i| {
                    let angle = i as f32 / self.resolution as f32 * std::f32::consts::TAU;
                    origin_2d + Vec2::new(angle.cos(), angle.sin()) * *radius
                }).collect()
            }
            TargetingMode::Cone { half_angle_deg, distance } => {
                let half_rad = half_angle_deg.to_radians();
                let mut pts = vec![origin_2d];
                let steps = self.resolution;
                for i in 0..=steps {
                    let frac = i as f32 / steps as f32;
                    let angle = angle_fwd - half_rad + frac * 2.0 * half_rad;
                    pts.push(origin_2d + Vec2::new(angle.cos(), angle.sin()) * *distance);
                }
                pts.push(origin_2d);
                pts
            }
            TargetingMode::Rectangle { width, length } => {
                let fwd = forward_2d;
                let right = Vec2::new(-fwd.y, fwd.x);
                let half_w = *width * 0.5;
                vec![
                    origin_2d + right * half_w,
                    origin_2d + right * half_w + fwd * *length,
                    origin_2d - right * half_w + fwd * *length,
                    origin_2d - right * half_w,
                    origin_2d + right * half_w,
                ]
            }
            _ => vec![origin_2d],
        }
    }

    pub fn aabb_2d(&self) -> (Vec2, Vec2) {
        let pts = self.generate_outline_points_2d();
        if pts.is_empty() {
            let p = Vec2::new(self.origin.x, self.origin.z);
            return (p, p);
        }
        let min_x = pts.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let min_y = pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_x = pts.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let max_y = pts.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        (Vec2::new(min_x, min_y), Vec2::new(max_x, max_y))
    }
}

// ============================================================
// STATUS EFFECT INTERACTION MATRIX
// ============================================================

#[derive(Debug, Clone)]
pub struct InteractionMatrix {
    pub reactions: HashMap<(StatusEffectType, StatusEffectType), Vec<StatusReaction>>,
}

impl InteractionMatrix {
    pub fn new() -> Self {
        let mut m = InteractionMatrix { reactions: HashMap::new() };
        // Fire + Wet = extinguish fire
        m.add_reaction(StatusEffectType::Burn, StatusEffectType::Wet, StatusReaction::Remove);
        // Cold + Wet = super-freeze
        m.add_reaction(StatusEffectType::Freeze, StatusEffectType::Wet, StatusReaction::Amplify { factor: 1.5 });
        // Freeze + Burn = shatter explosion
        m.add_reaction(StatusEffectType::Freeze, StatusEffectType::Burn, StatusReaction::Explode {
            damage_formula: DamageFormula::new(DamageElement::Physical, 30.0, 60.0),
        });
        // Oiled + Fire = extra burn stacks
        m.add_reaction(StatusEffectType::Burn, StatusEffectType::Oiled, StatusReaction::Amplify { factor: 2.0 });
        // Shock + Wet = doubled shock damage
        m.add_reaction(StatusEffectType::Shock, StatusEffectType::Wet, StatusReaction::Amplify { factor: 2.0 });
        m
    }

    pub fn add_reaction(&mut self, existing: StatusEffectType, incoming: StatusEffectType, reaction: StatusReaction) {
        self.reactions.entry((existing, incoming)).or_insert_with(Vec::new).push(reaction);
    }

    pub fn get_reactions(&self, existing: StatusEffectType, incoming: StatusEffectType) -> Option<&Vec<StatusReaction>> {
        self.reactions.get(&(existing, incoming))
    }

    pub fn process_interactions(
        &self,
        target_effects: &mut Vec<ActiveStatusEffect>,
        incoming_type: StatusEffectType,
    ) -> Vec<StatusReaction> {
        let mut triggered = Vec::new();
        for active in target_effects.iter() {
            if let Some(reactions) = self.get_reactions(active.effect_type, incoming_type) {
                triggered.extend(reactions.iter().cloned());
            }
        }
        triggered
    }
}

// ============================================================
// ABILITY SEARCH INDEX (for large databases)
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilitySearchIndex {
    pub by_type: HashMap<AbilityType, Vec<u64>>,
    pub by_status_applied: HashMap<StatusEffectType, Vec<u64>>,
    pub by_resource: HashMap<ResourceType, Vec<u64>>,
    pub by_cooldown_range: BTreeMap<u64, Vec<u64>>, // (cd * 10) -> ids
    pub name_tokens: HashMap<String, Vec<u64>>,
}

impl AbilitySearchIndex {
    pub fn build(db: &AbilityDatabase) -> Self {
        let mut idx = AbilitySearchIndex {
            by_type: HashMap::new(),
            by_status_applied: HashMap::new(),
            by_resource: HashMap::new(),
            by_cooldown_range: BTreeMap::new(),
            name_tokens: HashMap::new(),
        };

        for (id, a) in &db.abilities {
            idx.by_type.entry(a.ability_type).or_insert_with(Vec::new).push(*id);
            idx.by_resource.entry(a.resource_type).or_insert_with(Vec::new).push(*id);
            let cd_key = (a.base_cooldown * 10.0) as u64;
            idx.by_cooldown_range.entry(cd_key).or_insert_with(Vec::new).push(*id);
            for eff in &a.applied_effects {
                idx.by_status_applied.entry(eff.effect_type).or_insert_with(Vec::new).push(*id);
            }
            for token in a.name.split_whitespace() {
                idx.name_tokens.entry(token.to_lowercase()).or_insert_with(Vec::new).push(*id);
            }
        }

        idx
    }

    pub fn search_by_tokens(&self, query: &str) -> HashSet<u64> {
        let mut results: Option<HashSet<u64>> = None;
        for token in query.split_whitespace() {
            let tok = token.to_lowercase();
            let ids: HashSet<u64> = self.name_tokens.get(&tok).map(|v| v.iter().copied().collect()).unwrap_or_default();
            results = Some(match results {
                None => ids,
                Some(prev) => prev.intersection(&ids).copied().collect(),
            });
        }
        results.unwrap_or_default()
    }

    pub fn abilities_by_type(&self, t: AbilityType) -> &[u64] {
        self.by_type.get(&t).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn abilities_in_cd_range(&self, min_cd: f32, max_cd: f32) -> Vec<u64> {
        let min_key = (min_cd * 10.0) as u64;
        let max_key = (max_cd * 10.0) as u64;
        self.by_cooldown_range.range(min_key..=max_key).flat_map(|(_, v)| v.iter().copied()).collect()
    }
}

// ============================================================
// ABILITY DPS RANKING
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityRank {
    pub ability_id: u64,
    pub ability_name: String,
    pub effective_dps: f32,
    pub burst_damage: f32,    // max single-cast damage
    pub total_damage_60s: f32, // total if spammed for 60 seconds
    pub rank: u32,
}

pub fn rank_abilities_by_dps(db: &AbilityDatabase, config: &FormulaTesterConfig) -> Vec<AbilityRank> {
    let mut ranks: Vec<AbilityRank> = db.abilities.values()
        .filter(|a| a.ability_type.is_damage_dealing())
        .map(|a| {
            let result = run_formula_test(a, config);
            let burst = result.total_max_damage;
            let casts_per_60 = if a.base_cooldown > 0.0 {
                60.0 / (a.base_cooldown + a.cast_time)
            } else {
                60.0 / (1.5 + a.cast_time) // assume 1.5s GCD
            };
            let total_60 = result.total_avg_damage * casts_per_60;
            AbilityRank {
                ability_id: a.id,
                ability_name: a.name.clone(),
                effective_dps: result.effective_dps,
                burst_damage: burst,
                total_damage_60s: total_60,
                rank: 0,
            }
        }).collect();

    ranks.sort_by(|a, b| b.effective_dps.partial_cmp(&a.effective_dps).unwrap_or(std::cmp::Ordering::Equal));
    for (i, rank) in ranks.iter_mut().enumerate() {
        rank.rank = (i + 1) as u32;
    }
    ranks
}

// ============================================================
// BUILD EDITOR (combining talent trees + ability loadout)
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityLoadout {
    pub id: u64,
    pub name: String,
    pub class_name: String,
    pub level: u32,
    pub slotted_ability_ids: Vec<Option<u64>>, // 10 ability slots
    pub active_talent_trees: Vec<u64>,
    pub available_points: u32,
    pub invested_points: HashMap<u64, u32>, // tree_id -> points_spent
}

impl AbilityLoadout {
    pub fn new(id: u64, name: impl Into<String>, class: impl Into<String>, level: u32) -> Self {
        AbilityLoadout {
            id,
            name: name.into(),
            class_name: class.into(),
            level,
            slotted_ability_ids: vec![None; 10],
            active_talent_trees: Vec::new(),
            available_points: level,
            invested_points: HashMap::new(),
        }
    }

    pub fn slot_ability(&mut self, slot_idx: usize, ability_id: u64) -> bool {
        if slot_idx >= self.slotted_ability_ids.len() { return false; }
        self.slotted_ability_ids[slot_idx] = Some(ability_id);
        true
    }

    pub fn unslot_ability(&mut self, slot_idx: usize) {
        if slot_idx < self.slotted_ability_ids.len() {
            self.slotted_ability_ids[slot_idx] = None;
        }
    }

    pub fn total_effective_dps(&self, db: &AbilityDatabase, config: &FormulaTesterConfig) -> f32 {
        self.slotted_ability_ids.iter().filter_map(|id| *id).map(|id| {
            db.abilities.get(&id).map(|a| {
                run_formula_test(a, config).effective_dps
            }).unwrap_or(0.0)
        }).sum()
    }

    pub fn all_talent_stat_mods(&self, db: &AbilityDatabase) -> Vec<(String, f32, f32)> {
        let mut totals: HashMap<String, (f32, f32)> = HashMap::new();
        for &tree_id in &self.active_talent_trees {
            if let Some(tree) = db.talent_trees.get(&tree_id) {
                for (name, flat, pct) in tree.compute_all_stat_mods() {
                    let entry = totals.entry(name).or_insert((0.0, 0.0));
                    entry.0 += flat;
                    entry.1 += pct;
                }
            }
        }
        totals.into_iter().map(|(k, (f, p))| (k, f, p)).collect()
    }
}

// ============================================================
// ABILITY EDITOR ENTRY POINTS
// ============================================================

pub fn build_ability_editor() -> AbilityEditor {
    let mut editor = AbilityEditor::new();
    editor.database = create_starter_ability_database();
    editor.browser.refresh(&editor.database);
    // Register all abilities in cooldown manager
    for (id, a) in &editor.database.abilities {
        editor.cooldown_manager.register_ability(*id, a.base_cooldown, a.gcd_category, a.max_charges);
    }
    editor
}

pub fn run_ability_editor_frame(editor: &mut AbilityEditor, dt: f32, input: &AbilityEditorInput) {
    editor.tick(dt);

    match input.action {
        Some(AbilityEditorAction::Save) => { editor.save_current_ability(); }
        Some(AbilityEditorAction::Undo) => { editor.ability_editor.undo(); }
        Some(AbilityEditorAction::Redo) => { editor.ability_editor.redo(); }
        Some(AbilityEditorAction::Copy) => { editor.copy_ability(); }
        Some(AbilityEditorAction::Paste) => { editor.paste_ability(); }
        Some(AbilityEditorAction::RunFormulaTest) => { editor.run_formula_test_now(); }
        Some(AbilityEditorAction::NewAbility(t)) => { editor.create_new_ability(t); }
        Some(AbilityEditorAction::Delete) => { editor.delete_selected_abilities(); }
        None => {}
    }

    if let Some(ref q) = input.search_query {
        editor.search_with_history(q.clone());
    }

    if let Some(id) = input.open_ability_id {
        editor.open_ability_by_id(id);
    }
}

#[derive(Debug, Clone)]
pub struct AbilityEditorInput {
    pub action: Option<AbilityEditorAction>,
    pub search_query: Option<String>,
    pub open_ability_id: Option<u64>,
    pub mouse_pos: Vec2,
    pub delta_time: f32,
}

#[derive(Debug, Clone)]
pub enum AbilityEditorAction {
    Save,
    Undo,
    Redo,
    Copy,
    Paste,
    Delete,
    RunFormulaTest,
    NewAbility(AbilityType),
}

impl AbilityEditorInput {
    pub fn idle() -> Self {
        AbilityEditorInput {
            action: None,
            search_query: None,
            open_ability_id: None,
            mouse_pos: Vec2::ZERO,
            delta_time: 0.016,
        }
    }
}

// ============================================================
// MATH UTILITY FUNCTIONS
// ============================================================

pub fn angle_between_2d(a: Vec2, b: Vec2) -> f32 {
    let dot = a.normalize_or_zero().dot(b.normalize_or_zero());
    dot.clamp(-1.0, 1.0).acos()
}

pub fn rotate_vec2(v: Vec2, angle_rad: f32) -> Vec2 {
    let cos = angle_rad.cos();
    let sin = angle_rad.sin();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

pub fn reflect_vec2(v: Vec2, normal: Vec2) -> Vec2 {
    v - 2.0 * v.dot(normal) * normal
}

pub fn closest_point_on_segment_2d(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let ap = p - a;
    let t = (ap.dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
    a + ab * t
}

pub fn circle_intersects_rect(circle_center: Vec2, radius: f32, rect_min: Vec2, rect_max: Vec2) -> bool {
    let closest = Vec2::new(
        circle_center.x.clamp(rect_min.x, rect_max.x),
        circle_center.y.clamp(rect_min.y, rect_max.y),
    );
    (circle_center - closest).length_squared() <= radius * radius
}

pub fn cone_intersects_circle(apex: Vec2, direction: Vec2, half_angle: f32, length: f32, circle_center: Vec2, radius: f32) -> bool {
    let to_circle = circle_center - apex;
    let dist = to_circle.length();
    if dist > length + radius { return false; }
    if dist < radius { return true; }
    let angle = angle_between_2d(direction, to_circle);
    angle <= half_angle + (radius / dist).asin()
}

pub fn ring_contains_point(center: Vec2, inner_r: f32, outer_r: f32, point: Vec2) -> bool {
    let d = (point - center).length();
    d >= inner_r && d <= outer_r
}

pub fn parabola_peak_position(origin: Vec3, velocity: Vec3, gravity: f32) -> Vec3 {
    // Time to reach peak: t = vy / g
    if gravity.abs() < 1e-6 { return origin + velocity * 100.0; }
    let t_peak = velocity.y / gravity;
    origin + velocity * t_peak - Vec3::new(0.0, 0.5 * gravity * t_peak * t_peak, 0.0)
}

pub fn knockback_velocity(direction: Vec2, force: f32, target_mass: f32) -> Vec2 {
    let actual_force = force / target_mass.max(0.1);
    direction.normalize_or_zero() * actual_force
}

pub fn ability_power_score(a: &AbilityDefinition) -> f32 {
    let dmg_score: f32 = a.damage_formulas.iter().map(|f| (f.base_min + f.base_max) / 2.0).sum::<f32>();
    let heal_score = (a.heal_formula.base_min + a.heal_formula.base_max) / 2.0;
    let cd_penalty = (a.base_cooldown * 0.5).max(1.0);
    let cost_penalty = a.resource_cost * 0.1;
    let range_bonus = (a.range * 0.5).min(15.0);
    let aoe_bonus = a.aoe_radius * 3.0;
    let effects_bonus = a.applied_effects.len() as f32 * 10.0;
    (dmg_score + heal_score + range_bonus + aoe_bonus + effects_bonus - cost_penalty) / cd_penalty
}

// ============================================================
// END OF FILE
// ============================================================
