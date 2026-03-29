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
            StatusEffectType::Invisible => Vec4::new(0.5, 0.5, 0.7, 0.6),
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
    // Classification
    pub school: String,
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
            school: String::new(),
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
// EXTENDED: ABILITY EFFECT CHAIN SYSTEM
// ============================================================

/// A chain of effects that triggers on ability hit.
/// Each link can spawn sub-abilities, apply more effects, or modify damage.
#[derive(Debug, Clone)]
pub struct AbilityEffectChain {
    pub id: u64,
    pub name: String,
    pub links: Vec<ChainLink>,
    pub trigger_condition: ChainTrigger,
}

#[derive(Debug, Clone)]
pub struct ChainLink {
    pub sequence: u32,   // execution order
    pub delay: f32,      // seconds after previous link
    pub action: ChainAction,
}

#[derive(Debug, Clone)]
pub enum ChainAction {
    DealDamage { formula: DamageFormula },
    ApplyStatus { effect_id: u64, chance: f32 },
    Heal { formula: HealFormula },
    SpawnProjectile { ability_id: u64 },
    PullTarget { force: f32 },
    PushTarget { force: f32 },
    GrantResource { resource: ResourceType, amount: f32 },
    DrainResource { resource: ResourceType, amount: f32 },
    TriggerChain { chain_id: u64 },
}

#[derive(Debug, Clone)]
pub enum ChainTrigger {
    OnHit,
    OnKill,
    OnCrit,
    OnStatusApplied(StatusEffectType),
    OnResourceThreshold { resource: ResourceType, threshold: f32 },
    Always,
}

impl ChainTrigger {
    pub fn should_trigger(&self, ctx: &ChainContext) -> bool {
        match self {
            ChainTrigger::OnHit => ctx.hit_occurred,
            ChainTrigger::OnKill => ctx.kill_occurred,
            ChainTrigger::OnCrit => ctx.crit_occurred,
            ChainTrigger::OnStatusApplied(s) => ctx.status_applied == Some(*s),
            ChainTrigger::OnResourceThreshold { resource, threshold } => {
                ctx.resource_fractions.get(resource).copied().unwrap_or(0.0) >= *threshold
            }
            ChainTrigger::Always => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChainContext {
    pub hit_occurred: bool,
    pub kill_occurred: bool,
    pub crit_occurred: bool,
    pub status_applied: Option<StatusEffectType>,
    pub resource_fractions: HashMap<ResourceType, f32>,
    pub damage_dealt: f32,
}

impl ChainContext {
    pub fn from_result(result: &CastResult) -> Self {
        let hit = !result.damage_results.is_empty();
        let crit = result.damage_results.iter().any(|d| d.is_crit);
        let dmg: f32 = result.damage_results.iter().map(|d| d.final_damage).sum();
        ChainContext {
            hit_occurred: hit,
            kill_occurred: false,
            crit_occurred: crit,
            status_applied: result.effects_applied.first().map(|(_, s)| *s),
            resource_fractions: HashMap::new(),
            damage_dealt: dmg,
        }
    }
}

// ============================================================
// EXTENDED: STANCE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct StanceDefinition {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub enter_cost: f32,
    pub resource_type: ResourceType,
    pub maintenance_cost_per_second: f32,
    pub stat_modifiers: Vec<(String, f32, f32)>, // (stat, flat, pct)
    pub ability_overrides: HashMap<u64, u64>,    // old_ability_id -> override_ability_id
    pub forbidden_abilities: Vec<u64>,
    pub enabled_abilities: Vec<u64>,
    pub visual_effect: String,
    pub transition_time: f32,
}

impl StanceDefinition {
    pub fn can_enter(&self, resource: &ResourceState) -> bool {
        resource.current >= self.enter_cost
    }

    pub fn tick_cost(&self, dt: f32) -> f32 {
        self.maintenance_cost_per_second * dt
    }

    pub fn get_ability_override(&self, ability_id: u64) -> Option<u64> {
        self.ability_overrides.get(&ability_id).copied()
    }

    pub fn can_use_ability(&self, ability_id: u64) -> bool {
        !self.forbidden_abilities.contains(&ability_id)
    }
}

#[derive(Debug, Clone)]
pub struct StanceManager {
    pub active_stance: Option<u64>,
    pub previous_stance: Option<u64>,
    pub stances: HashMap<u64, StanceDefinition>,
    pub transition_timer: f32,
}

impl StanceManager {
    pub fn new() -> Self {
        StanceManager {
            active_stance: None,
            previous_stance: None,
            stances: HashMap::new(),
            transition_timer: 0.0,
        }
    }

    pub fn register_stance(&mut self, stance: StanceDefinition) {
        self.stances.insert(stance.id, stance);
    }

    pub fn enter_stance(&mut self, stance_id: u64, resources: &mut HashMap<ResourceType, ResourceState>) -> bool {
        if let Some(stance) = self.stances.get(&stance_id) {
            if let Some(res) = resources.get_mut(&stance.resource_type) {
                if !stance.can_enter(res) { return false; }
                res.spend(stance.enter_cost);
                self.previous_stance = self.active_stance;
                self.active_stance = Some(stance_id);
                self.transition_timer = stance.transition_time;
                return true;
            }
        }
        false
    }

    pub fn exit_stance(&mut self) {
        self.previous_stance = self.active_stance;
        self.active_stance = None;
    }

    pub fn tick(&mut self, dt: f32, resources: &mut HashMap<ResourceType, ResourceState>) {
        if self.transition_timer > 0.0 {
            self.transition_timer = (self.transition_timer - dt).max(0.0);
        }
        if let Some(stance_id) = self.active_stance {
            if let Some(stance) = self.stances.get(&stance_id) {
                let cost = stance.tick_cost(dt);
                if let Some(res) = resources.get_mut(&stance.resource_type) {
                    if !res.spend(cost) {
                        self.exit_stance(); // can't maintain — auto-exit
                    }
                }
            }
        }
    }

    pub fn active_stat_mods(&self) -> Vec<(String, f32, f32)> {
        if let Some(id) = self.active_stance {
            self.stances.get(&id).map(|s| s.stat_modifiers.clone()).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    pub fn is_transitioning(&self) -> bool {
        self.transition_timer > 0.0
    }
}

// ============================================================
// EXTENDED: PROC SYSTEM (on-hit effects)
// ============================================================

#[derive(Debug, Clone)]
pub struct ProcEffect {
    pub id: u64,
    pub name: String,
    pub trigger: ProcTrigger,
    pub chance: f32,         // 0-100%
    pub internal_cooldown: f32,
    pub remaining_icd: f32,
    pub effect: ProcOutcome,
}

#[derive(Debug, Clone)]
pub enum ProcTrigger {
    OnBasicAttack,
    OnAbilityCast,
    OnAbilityHit,
    OnCrit,
    OnKill,
    OnDamageTaken,
    OnHealCast,
    OnStatusApplication(StatusEffectType),
    OnLowHealth(f32), // below % threshold
}

#[derive(Debug, Clone)]
pub enum ProcOutcome {
    DealBonusDamage { formula: DamageFormula },
    ApplyStatus { effect_id: u64 },
    HealCaster { formula: HealFormula },
    ResetCooldown { ability_id: u64 },
    GrantCharges { ability_id: u64, charges: u32 },
    GenerateResource { resource: ResourceType, amount: f32 },
    GrantBuff { ability_id: u64, duration: f32 },
}

impl ProcEffect {
    pub fn can_proc(&self) -> bool {
        self.remaining_icd <= 0.0
    }

    pub fn should_proc(&self, roll: f32) -> bool {
        self.can_proc() && roll * 100.0 < self.chance
    }

    pub fn trigger_proc(&mut self) {
        self.remaining_icd = self.internal_cooldown;
    }

    pub fn tick_icd(&mut self, dt: f32) {
        self.remaining_icd = (self.remaining_icd - dt).max(0.0);
    }
}

#[derive(Debug, Clone)]
pub struct ProcManager {
    pub procs: Vec<ProcEffect>,
}

impl ProcManager {
    pub fn new() -> Self {
        ProcManager { procs: Vec::new() }
    }

    pub fn add_proc(&mut self, proc_effect: ProcEffect) {
        self.procs.push(proc_effect);
    }

    pub fn tick(&mut self, dt: f32) {
        for p in &mut self.procs {
            p.tick_icd(dt);
        }
    }

    pub fn check_procs(&mut self, trigger: &ProcTrigger, random_values: &[f32]) -> Vec<&ProcOutcome> {
        let mut triggered = Vec::new();
        let mut rv_idx = 0;
        for proc_effect in &mut self.procs {
            let trigger_match = match (&proc_effect.trigger, trigger) {
                (ProcTrigger::OnBasicAttack, ProcTrigger::OnBasicAttack) => true,
                (ProcTrigger::OnCrit, ProcTrigger::OnCrit) => true,
                (ProcTrigger::OnKill, ProcTrigger::OnKill) => true,
                (ProcTrigger::OnDamageTaken, ProcTrigger::OnDamageTaken) => true,
                (ProcTrigger::OnAbilityCast, ProcTrigger::OnAbilityCast) => true,
                (ProcTrigger::OnAbilityHit, ProcTrigger::OnAbilityHit) => true,
                _ => false,
            };
            if trigger_match {
                let roll = random_values.get(rv_idx).copied().unwrap_or(0.5);
                rv_idx += 1;
                if proc_effect.should_proc(roll) {
                    proc_effect.trigger_proc();
                    triggered.push(&proc_effect.effect);
                }
            }
        }
        triggered
    }
}

// ============================================================
// EXTENDED: ABILITY COOLDOWN PREVIEW
// ============================================================

#[derive(Debug, Clone)]
pub struct CooldownPreview {
    pub ability_id: u64,
    pub ability_name: String,
    pub base_cd: f32,
    pub with_cdr_10: f32,
    pub with_cdr_20: f32,
    pub with_cdr_30: f32,
    pub with_cdr_40: f32,
    pub with_cdr_50: f32,
    pub casts_per_minute_base: f32,
    pub casts_per_minute_max_cdr: f32,
}

impl CooldownPreview {
    pub fn compute(ability: &AbilityDefinition) -> Self {
        let cd = ability.base_cooldown;
        let cast_t = ability.cast_time;
        let min_cycle = cd + cast_t;

        let reduced = |pct: f32| -> f32 {
            let eff = diminishing_returns_cdr_ability(pct);
            (cd * (1.0 - eff / 100.0)).max(0.5)
        };

        let casts_per_min = |cd_val: f32| -> f32 {
            if cd_val + cast_t <= 0.0 { return 0.0; }
            60.0 / (cd_val + cast_t)
        };

        CooldownPreview {
            ability_id: ability.id,
            ability_name: ability.name.clone(),
            base_cd: cd,
            with_cdr_10: reduced(10.0),
            with_cdr_20: reduced(20.0),
            with_cdr_30: reduced(30.0),
            with_cdr_40: reduced(40.0),
            with_cdr_50: reduced(50.0),
            casts_per_minute_base: casts_per_min(cd),
            casts_per_minute_max_cdr: casts_per_min(reduced(50.0)),
        }
    }
}

pub fn generate_cooldown_preview_table(db: &AbilityDatabase) -> Vec<CooldownPreview> {
    let mut previews: Vec<CooldownPreview> = db.abilities.values()
        .filter(|a| a.base_cooldown > 0.0)
        .map(CooldownPreview::compute)
        .collect();
    previews.sort_by(|a, b| a.base_cd.partial_cmp(&b.base_cd).unwrap_or(std::cmp::Ordering::Equal));
    previews
}

// ============================================================
// EXTENDED: RESOURCE EFFICIENCY CALCULATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct ResourceEfficiency {
    pub ability_id: u64,
    pub ability_name: String,
    pub resource_type: ResourceType,
    pub cost: f32,
    pub avg_damage: f32,
    pub damage_per_resource: f32,
    pub heal_per_resource: f32,
    pub effective_casts_before_oom: f32,
    pub time_before_oom: f32,
    pub breakeven_regen_rate: f32, // resource regen needed to spam indefinitely
}

pub fn compute_resource_efficiency(
    ability: &AbilityDefinition,
    config: &FormulaTesterConfig,
    resource_state: &ResourceState,
) -> ResourceEfficiency {
    let cost = ability.compute_resource_cost(&config.caster_stats);
    let test = run_formula_test(ability, config);
    let dpr = if cost > 0.0 { test.total_avg_damage / cost } else { f32::INFINITY };
    let hpr = if cost > 0.0 { test.heal_preview / cost } else { 0.0 };
    let casts_oom = if cost > 0.0 { resource_state.current / cost } else { f32::INFINITY };
    let time_oom = casts_oom * (ability.cast_time + ability.base_cooldown.max(1.5));
    let breakeven = if ability.base_cooldown > 0.0 { cost / ability.base_cooldown } else { f32::INFINITY };

    ResourceEfficiency {
        ability_id: ability.id,
        ability_name: ability.name.clone(),
        resource_type: ability.resource_type,
        cost,
        avg_damage: test.total_avg_damage,
        damage_per_resource: dpr,
        heal_per_resource: hpr,
        effective_casts_before_oom: casts_oom,
        time_before_oom: time_oom,
        breakeven_regen_rate: breakeven,
    }
}

pub fn rank_abilities_by_efficiency(
    db: &AbilityDatabase,
    config: &FormulaTesterConfig,
    resource: ResourceType,
) -> Vec<ResourceEfficiency> {
    let res = ResourceState::new(resource);
    let mut rankings: Vec<ResourceEfficiency> = db.abilities.values()
        .filter(|a| a.resource_type == resource && a.ability_type.is_damage_dealing())
        .map(|a| compute_resource_efficiency(a, config, &res))
        .collect();
    rankings.sort_by(|a, b| b.damage_per_resource.partial_cmp(&a.damage_per_resource).unwrap_or(std::cmp::Ordering::Equal));
    rankings
}

// ============================================================
// EXTENDED: ABILITY SYNERGY DETECTOR
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilitySynergy {
    pub ability_a_id: u64,
    pub ability_b_id: u64,
    pub synergy_type: SynergyType,
    pub synergy_score: f32,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynergyType {
    ApplyThenExplode,    // A applies status, B does bonus vs that status
    ComboBuilder,        // A builds combo points, B spends them
    BuffThenDPS,         // A buffs, B deals damage during buff window
    SetupCC,             // A applies CC, B does bonus vs CC'd
    ElementalReaction,   // A wets target, B does lightning for double damage
    ResourceLoop,        // A costs, B refunds
}

pub fn detect_synergies(db: &AbilityDatabase) -> Vec<AbilitySynergy> {
    let mut synergies = Vec::new();

    let abilities: Vec<&AbilityDefinition> = db.abilities.values().collect();

    for i in 0..abilities.len() {
        for j in 0..abilities.len() {
            if i == j { continue; }
            let a = abilities[i];
            let b = abilities[j];

            // Apply status A → B does bonus damage against that status
            for eff in &a.applied_effects {
                for formula in &b.damage_formulas {
                    for &(status, _bonus) in &formula.versus_status_bonus {
                        if status == eff.effect_type {
                            synergies.push(AbilitySynergy {
                                ability_a_id: a.id,
                                ability_b_id: b.id,
                                synergy_type: SynergyType::ApplyThenExplode,
                                synergy_score: 8.0,
                                description: format!(
                                    "{} applies {}, {} deals bonus vs {}",
                                    a.name, eff.effect_type.display_name(),
                                    b.name, status.display_name()
                                ),
                            });
                        }
                    }
                }
            }

            // Elemental wet + lightning
            let a_applies_wet = a.applied_effects.iter().any(|e| e.effect_type == StatusEffectType::Wet);
            let b_has_lightning = b.damage_formulas.iter().any(|f| f.element == DamageElement::Lightning);
            if a_applies_wet && b_has_lightning {
                synergies.push(AbilitySynergy {
                    ability_a_id: a.id,
                    ability_b_id: b.id,
                    synergy_type: SynergyType::ElementalReaction,
                    synergy_score: 10.0,
                    description: format!("{} wets target, {} triggers electrocute", a.name, b.name),
                });
            }

            // Buff then DPS
            if matches!(a.ability_type, AbilityType::Buff | AbilityType::Stance | AbilityType::Warcry) && b.ability_type.is_damage_dealing() {
                synergies.push(AbilitySynergy {
                    ability_a_id: a.id,
                    ability_b_id: b.id,
                    synergy_type: SynergyType::BuffThenDPS,
                    synergy_score: 5.0,
                    description: format!("{} buffs, then use {} for boosted damage", a.name, b.name),
                });
            }
        }
    }

    synergies.sort_by(|a, b| b.synergy_score.partial_cmp(&a.synergy_score).unwrap_or(std::cmp::Ordering::Equal));
    synergies
}

// ============================================================
// EXTENDED: MULTI-TARGET DAMAGE CALCULATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct MultiTargetCalculation {
    pub ability_id: u64,
    pub target_count: u32,
    pub total_damage: f32,
    pub per_target_damage: f32,
    pub falloff_applied: bool,
    pub chain_falloff_per_bounce: f32,
    pub aoe_falloff_at_edge: f32,
}

pub fn calculate_multi_target_damage(
    ability: &AbilityDefinition,
    target_count: u32,
    config: &FormulaTesterConfig,
) -> MultiTargetCalculation {
    let single_test = run_formula_test(ability, config);
    let base_dmg = single_test.total_avg_damage;

    let (total, falloff_applied, chain_falloff) = match &ability.targeting_mode {
        TargetingMode::Chain { max_bounces, falloff_per_bounce, .. } => {
            let actual_targets = target_count.min(*max_bounces + 1);
            let mut total = 0.0f32;
            let mut dmg = base_dmg;
            for _ in 0..actual_targets {
                total += dmg;
                dmg *= 1.0 - falloff_per_bounce;
            }
            (total, true, *falloff_per_bounce)
        }
        TargetingMode::AoECircle { .. } | TargetingMode::AoESphere { .. } => {
            // AoE typically hits all targets for full damage (some games reduce at edge)
            (base_dmg * target_count as f32, false, 0.0)
        }
        TargetingMode::MultiTarget { max_targets, .. } => {
            let actual = target_count.min(*max_targets);
            (base_dmg * actual as f32, false, 0.0)
        }
        _ => (base_dmg, false, 0.0),
    };

    MultiTargetCalculation {
        ability_id: ability.id,
        target_count,
        total_damage: total,
        per_target_damage: if target_count > 0 { total / target_count as f32 } else { 0.0 },
        falloff_applied,
        chain_falloff_per_bounce: chain_falloff,
        aoe_falloff_at_edge: 0.0,
    }
}

// ============================================================
// EXTENDED: TALENT BUILD OPTIMIZER
// ============================================================

#[derive(Debug, Clone)]
pub struct TalentOptimizationGoal {
    pub maximize_stat: String,
    pub secondary_stats: Vec<(String, f32)>, // (stat, weight)
    pub required_abilities: Vec<u64>,
    pub budget_points: u32,
}

#[derive(Debug, Clone)]
pub struct TalentOptimizationResult {
    pub recommended_nodes: Vec<u64>,
    pub total_points_used: u32,
    pub projected_stat_values: Vec<(String, f32, f32)>,
    pub score: f32,
}

pub fn greedy_talent_optimizer(
    tree: &TalentTree,
    goal: &TalentOptimizationGoal,
) -> TalentOptimizationResult {
    let mut available_nodes: Vec<&TalentNode> = tree.nodes.values().collect();
    available_nodes.sort_by_key(|n| n.id);

    let mut selected: Vec<u64> = Vec::new();
    let mut points_used = 0u32;

    // Greedily select nodes by expected score contribution
    let score_node = |node: &TalentNode| -> f32 {
        let mut score = 0.0f32;
        for (stat, flat, pct) in node.current_stat_mods() {
            let base_weight = if stat == goal.maximize_stat { 1.0 } else {
                goal.secondary_stats.iter()
                    .find(|(s, _)| *s == stat)
                    .map(|(_, w)| *w)
                    .unwrap_or(0.0)
            };
            score += (flat.abs() + pct.abs()) * base_weight;
        }
        score / node.point_cost.max(1) as f32 // efficiency
    };

    let mut remaining = goal.budget_points;
    let mut iterations = 0u32;

    while remaining > 0 && iterations < 1000 {
        iterations += 1;

        // Find best eligible node not yet selected
        let best = available_nodes.iter()
            .filter(|n| !selected.contains(&n.id))
            .filter(|n| n.point_cost <= remaining)
            .filter(|n| {
                // Check prerequisites in selected
                tree.edges.iter()
                    .filter(|e| e.is_prerequisite && e.to_node_id == n.id)
                    .all(|e| selected.contains(&e.from_node_id))
            })
            .max_by(|a, b| score_node(a).partial_cmp(&score_node(b)).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(node) = best {
            selected.push(node.id);
            points_used += node.point_cost;
            remaining -= node.point_cost;
        } else {
            break;
        }
    }

    // Required abilities — ensure their tree nodes are included
    for &req_ab in &goal.required_abilities {
        for node in tree.nodes.values() {
            if node.ability_id == Some(req_ab) && !selected.contains(&node.id) {
                selected.push(node.id);
                points_used += node.point_cost;
            }
        }
    }

    // Compute projected stats
    let mut stat_map: HashMap<String, (f32, f32)> = HashMap::new();
    for &node_id in &selected {
        if let Some(node) = tree.nodes.get(&node_id) {
            for (stat, flat, pct) in node.current_stat_mods() {
                let e = stat_map.entry(stat).or_insert((0.0, 0.0));
                e.0 += flat;
                e.1 += pct;
            }
        }
    }
    let projected: Vec<(String, f32, f32)> = stat_map.into_iter().map(|(k, (f, p))| (k, f, p)).collect();

    let total_score: f32 = selected.iter()
        .filter_map(|id| tree.nodes.get(id))
        .map(|n| score_node(n))
        .sum();

    TalentOptimizationResult {
        recommended_nodes: selected,
        total_points_used: points_used,
        projected_stat_values: projected,
        score: total_score,
    }
}

// ============================================================
// EXTENDED: DAMAGE MITIGATION CALCULATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct MitigationProfile {
    pub name: String,
    pub physical_armor: f32,
    pub fire_resist: f32,
    pub cold_resist: f32,
    pub lightning_resist: f32,
    pub poison_resist: f32,
    pub arcane_resist: f32,
    pub dodge_chance: f32,
    pub block_chance: f32,
    pub block_reduction: f32,
    pub absorb_shields: f32,
}

impl MitigationProfile {
    pub fn tank_profile() -> Self {
        MitigationProfile {
            name: "Heavy Tank".to_string(),
            physical_armor: 800.0,
            fire_resist: 40.0,
            cold_resist: 40.0,
            lightning_resist: 40.0,
            poison_resist: 40.0,
            arcane_resist: 20.0,
            dodge_chance: 5.0,
            block_chance: 30.0,
            block_reduction: 0.4,
            absorb_shields: 500.0,
        }
    }

    pub fn glass_cannon_profile() -> Self {
        MitigationProfile {
            name: "Glass Cannon".to_string(),
            physical_armor: 80.0,
            fire_resist: 10.0,
            cold_resist: 10.0,
            lightning_resist: 10.0,
            poison_resist: 5.0,
            arcane_resist: 5.0,
            dodge_chance: 15.0,
            block_chance: 0.0,
            block_reduction: 0.0,
            absorb_shields: 0.0,
        }
    }

    pub fn effective_resist_for_element(&self, element: DamageElement) -> f32 {
        match element {
            DamageElement::Physical => {
                let armor_factor = self.physical_armor / (self.physical_armor + 300.0);
                (armor_factor * 100.0).min(75.0)
            }
            DamageElement::Fire => self.fire_resist.min(75.0),
            DamageElement::Cold => self.cold_resist.min(75.0),
            DamageElement::Lightning => self.lightning_resist.min(75.0),
            DamageElement::Poison => self.poison_resist.min(75.0),
            DamageElement::Arcane => self.arcane_resist.min(60.0),
            DamageElement::True_ => 0.0,
            _ => 0.0,
        }
    }

    pub fn final_damage_received(&self, raw: f32, element: DamageElement, roll: f32) -> f32 {
        // Check dodge
        if roll < self.dodge_chance / 100.0 { return 0.0; }
        let resist = self.effective_resist_for_element(element);
        let after_resist = raw * (1.0 - resist / 100.0);
        // Check block
        let after_block = if roll < (self.dodge_chance + self.block_chance) / 100.0 {
            after_resist * (1.0 - self.block_reduction)
        } else {
            after_resist
        };
        // Absorb shields
        (after_block - self.absorb_shields).max(0.0)
    }
}

pub fn simulate_ability_vs_profile(
    ability: &AbilityDefinition,
    profile: &MitigationProfile,
    config: &FormulaTesterConfig,
) -> Vec<f32> {
    let mut seed: u64 = 54321987;
    let lcg_local = |s: &mut u64| -> f32 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 32) as f32) / (u32::MAX as f32)
    };

    (0..1000).map(|_| {
        let mut total = 0.0f32;
        for formula in &ability.damage_formulas {
            let t = lcg_local(&mut seed);
            let crit_roll = lcg_local(&mut seed);
            let dodge_roll = lcg_local(&mut seed);
            let resist = profile.effective_resist_for_element(formula.element);
            let hit = formula.compute_final_damage(&config.caster_stats, resist, &[], t, crit_roll);
            total += profile.final_damage_received(hit.raw_damage, formula.element, dodge_roll);
        }
        total
    }).collect()
}

// ============================================================
// EXTENDED: ABILITY UPGRADE TREE BUILDER
// ============================================================

pub struct AbilityUpgradeBuilder {
    pub ability_id: u64,
    pub upgrades: Vec<AbilityModifier>,
    pub upgrade_graph: HashMap<u64, Vec<u64>>, // parent_upgrade -> child_upgrades
    pub active_upgrades: HashSet<u64>,
    pub total_points_spent: u32,
}

impl AbilityUpgradeBuilder {
    pub fn new(ability_id: u64) -> Self {
        AbilityUpgradeBuilder {
            ability_id,
            upgrades: Vec::new(),
            upgrade_graph: HashMap::new(),
            active_upgrades: HashSet::new(),
            total_points_spent: 0,
        }
    }

    pub fn add_upgrade(&mut self, modifier: AbilityModifier, parent_id: Option<u64>) {
        if let Some(pid) = parent_id {
            self.upgrade_graph.entry(pid).or_insert_with(Vec::new).push(modifier.id);
        }
        self.upgrades.push(modifier);
    }

    pub fn can_unlock(&self, modifier_id: u64) -> bool {
        // Check if parent is active (if any)
        let has_parent = self.upgrade_graph.iter().any(|(_, children)| children.contains(&modifier_id));
        if has_parent {
            // Must have at least one parent unlocked
            self.upgrade_graph.iter().any(|(parent, children)| {
                children.contains(&modifier_id) && self.active_upgrades.contains(parent)
            })
        } else {
            true // root node, always available
        }
    }

    pub fn unlock(&mut self, modifier_id: u64, db: &AbilityDatabase, def: &mut AbilityDefinition) -> bool {
        if !self.can_unlock(modifier_id) { return false; }
        if self.active_upgrades.contains(&modifier_id) { return false; }

        if let Some(modifier) = db.ability_modifiers.get(&modifier_id).cloned() {
            modifier.apply_to_definition(def);
            self.active_upgrades.insert(modifier_id);
            self.total_points_spent += modifier.unlock_cost;
            true
        } else {
            false
        }
    }

    pub fn respec(&mut self, def: &mut AbilityDefinition, original: AbilityDefinition, db: &AbilityDatabase) {
        *def = original;
        // Re-apply all active upgrades in topological order
        let ordered = self.topological_order();
        for mid in ordered {
            if self.active_upgrades.contains(&mid) {
                if let Some(modifier) = db.ability_modifiers.get(&mid).cloned() {
                    modifier.apply_to_definition(def);
                }
            }
        }
    }

    fn topological_order(&self) -> Vec<u64> {
        // BFS from roots
        let all_ids: HashSet<u64> = self.upgrades.iter().map(|u| u.id).collect();
        let child_ids: HashSet<u64> = self.upgrade_graph.values().flat_map(|v| v.iter().copied()).collect();
        let roots: Vec<u64> = all_ids.difference(&child_ids).copied().collect();
        let mut result = Vec::new();
        let mut queue: VecDeque<u64> = roots.into_iter().collect();
        while let Some(id) = queue.pop_front() {
            result.push(id);
            if let Some(children) = self.upgrade_graph.get(&id) {
                for &child in children {
                    queue.push_back(child);
                }
            }
        }
        result
    }
}

// ============================================================
// EXTENDED: CHANNEL ABILITY TICK PLANNER
// ============================================================

#[derive(Debug, Clone)]
pub struct ChannelTickPlan {
    pub total_duration: f32,
    pub tick_count: u32,
    pub tick_interval: f32,
    pub tick_times: Vec<f32>,
    pub tick_damage_values: Vec<f32>,
    pub cumulative_damage: Vec<f32>,
    pub can_be_interrupted: bool,
    pub partial_damage_on_interrupt: bool,
}

impl ChannelTickPlan {
    pub fn build(ability: &AbilityDefinition, config: &FormulaTesterConfig) -> Self {
        let duration = ability.channel_time;
        let ticks = ability.channel_ticks;
        let interval = if ticks > 0 { duration / ticks as f32 } else { duration };

        let mut tick_times = Vec::new();
        let mut tick_damages = Vec::new();
        let mut cumulative = Vec::new();

        let base_dmg_per_tick: f32 = if ticks > 0 {
            ability.damage_formulas.iter().map(|f| {
                let r = f.compute_final_damage(&config.caster_stats, config.resist_for_element(f.element), &[], 0.5, 0.9);
                r.final_damage / ticks as f32
            }).sum()
        } else { 0.0 };

        let mut cumul = 0.0f32;
        for i in 0..ticks {
            let t = interval * (i + 1) as f32;
            tick_times.push(t);
            tick_damages.push(base_dmg_per_tick);
            cumul += base_dmg_per_tick;
            cumulative.push(cumul);
        }

        ChannelTickPlan {
            total_duration: duration,
            tick_count: ticks,
            tick_interval: interval,
            tick_times,
            tick_damage_values: tick_damages,
            cumulative_damage: cumulative,
            can_be_interrupted: ability.interrupt_on_move,
            partial_damage_on_interrupt: true,
        }
    }

    pub fn damage_at_time(&self, elapsed: f32) -> f32 {
        if self.tick_times.is_empty() { return 0.0; }
        let ticks_elapsed = self.tick_times.iter().filter(|&&t| t <= elapsed).count();
        self.cumulative_damage.get(ticks_elapsed.saturating_sub(1)).copied().unwrap_or(0.0)
    }

    pub fn dps(&self) -> f32 {
        if self.total_duration <= 0.0 { return 0.0; }
        self.cumulative_damage.last().copied().unwrap_or(0.0) / self.total_duration
    }
}

// ============================================================
// EXTENDED: ABILITY VISUAL EFFECTS PREVIEW
// ============================================================

#[derive(Debug, Clone)]
pub struct VfxPreviewData {
    pub cast_particles: Vec<ParticleEmitter>,
    pub impact_particles: Vec<ParticleEmitter>,
    pub projectile_trail: Option<TrailEffect>,
    pub screen_shake: Option<ScreenShake>,
    pub status_effect_glow: Vec<(StatusEffectType, Vec4)>,
}

#[derive(Debug, Clone)]
pub struct ParticleEmitter {
    pub name: String,
    pub position_offset: Vec3,
    pub direction: Vec3,
    pub spread_angle: f32,
    pub emission_rate: f32,
    pub lifetime: f32,
    pub speed: f32,
    pub color_start: Vec4,
    pub color_end: Vec4,
    pub size_start: f32,
    pub size_end: f32,
    pub gravity: f32,
    pub count: u32,
}

impl ParticleEmitter {
    pub fn simple(name: impl Into<String>, color: Vec4) -> Self {
        ParticleEmitter {
            name: name.into(),
            position_offset: Vec3::ZERO,
            direction: Vec3::Y,
            spread_angle: 30.0,
            emission_rate: 50.0,
            lifetime: 0.5,
            speed: 3.0,
            color_start: color,
            color_end: Vec4::new(color.x, color.y, color.z, 0.0),
            size_start: 0.2,
            size_end: 0.0,
            gravity: -2.0,
            count: 20,
        }
    }

    pub fn simulate_position(&self, t: f32, seed: u32) -> Vec3 {
        let angle_rad = (seed as f32 * 2.3 + t) % (std::f32::consts::TAU);
        let spread = self.spread_angle.to_radians();
        let dir = Vec3::new(
            angle_rad.sin() * spread.sin(),
            self.direction.y,
            angle_rad.cos() * spread.sin(),
        ).normalize_or_zero();
        let gravity_offset = Vec3::new(0.0, -0.5 * self.gravity * t * t, 0.0);
        self.position_offset + dir * self.speed * t + gravity_offset
    }
}

#[derive(Debug, Clone)]
pub struct TrailEffect {
    pub width: f32,
    pub length: f32,
    pub color: Vec4,
    pub fade_time: f32,
    pub subdivisions: u32,
}

#[derive(Debug, Clone)]
pub struct ScreenShake {
    pub intensity: f32,
    pub duration: f32,
    pub frequency: f32,
    pub falloff: f32,
}

impl ScreenShake {
    pub fn displacement_at_time(&self, t: f32) -> Vec2 {
        if t >= self.duration { return Vec2::ZERO; }
        let decay = (1.0 - t / self.duration).powf(self.falloff);
        let wave = (t * self.frequency * std::f32::consts::TAU).sin();
        Vec2::new(wave * self.intensity * decay, wave * 0.7 * self.intensity * decay)
    }
}

pub fn build_vfx_preview(ability: &AbilityDefinition) -> VfxPreviewData {
    let mut cast_particles = Vec::new();
    let mut impact_particles = Vec::new();
    let mut status_glows = Vec::new();

    // Base cast effect
    for formula in &ability.damage_formulas {
        cast_particles.push(ParticleEmitter::simple(
            format!("{} cast", formula.element.display_name()),
            formula.element.color(),
        ));
        let mut impact = ParticleEmitter::simple(
            format!("{} impact", formula.element.display_name()),
            formula.element.color(),
        );
        impact.count = 40;
        impact.speed = 5.0;
        impact.size_start = 0.4;
        impact_particles.push(impact);
    }

    for applied in &ability.applied_effects {
        status_glows.push((applied.effect_type, applied.effect_type.color()));
    }

    let trail = ability.projectile_params.as_ref().map(|p| TrailEffect {
        width: p.size * 2.0,
        length: p.speed * 0.1,
        color: Vec4::new(1.0, 0.8, 0.3, 0.8),
        fade_time: 0.2,
        subdivisions: 8,
    });

    let shake = if ability.aoe_radius > 0.0 {
        Some(ScreenShake {
            intensity: (ability.aoe_radius * 0.05).min(0.3),
            duration: 0.4,
            frequency: 12.0,
            falloff: 2.0,
        })
    } else {
        None
    };

    VfxPreviewData {
        cast_particles,
        impact_particles,
        projectile_trail: trail,
        screen_shake: shake,
        status_effect_glow: status_glows,
    }
}

// ============================================================
// EXTENDED: ABILITY BALANCE CHECKER
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityBalanceReport {
    pub ability_name: String,
    pub effective_dps: f32,
    pub expected_dps_for_cooldown: f32,
    pub is_overpowered: bool,
    pub is_underpowered: bool,
    pub resource_efficiency: f32,
    pub utility_score: f32,
    pub total_score: f32,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

pub fn check_ability_balance(ability: &AbilityDefinition, config: &FormulaTesterConfig) -> AbilityBalanceReport {
    let test = run_formula_test(ability, config);
    let eff_dps = test.effective_dps;

    // Expected DPS based on cooldown and ability type
    let expected_dps = expected_dps_for_cd(ability.base_cooldown, ability.ability_type);
    let tolerance = expected_dps * 0.3;

    let resource_eff = if ability.resource_cost > 0.0 { test.total_avg_damage / ability.resource_cost } else { 100.0 };

    // Utility score: CC, healing, mobility add value
    let utility = ability.applied_effects.iter().map(|e| {
        if e.effect_type.is_crowd_control() { 20.0 }
        else if e.effect_type.is_beneficial() { 10.0 }
        else { 5.0 }
    }).sum::<f32>()
        + if ability.ability_type.is_movement() { 15.0 } else { 0.0 }
        + ability.heal_formula.base_max * 0.1;

    let mut warnings = Vec::new();
    let mut suggestions = Vec::new();

    if eff_dps > expected_dps + tolerance {
        warnings.push(format!("DPS {:.1} exceeds expected {:.1} by {:.1}%",
            eff_dps, expected_dps, (eff_dps / expected_dps - 1.0) * 100.0));
        suggestions.push("Consider increasing cooldown or reducing base damage".to_string());
    }
    if eff_dps < expected_dps - tolerance && ability.ability_type.is_damage_dealing() {
        warnings.push(format!("DPS {:.1} below expected {:.1}", eff_dps, expected_dps));
        suggestions.push("Consider reducing cooldown or increasing base damage".to_string());
    }
    if ability.resource_cost > 100.0 {
        warnings.push("Very high resource cost — may be unusable in sustained combat".to_string());
    }
    if ability.base_cooldown > 120.0 {
        warnings.push("Very long cooldown — should have correspondingly high impact".to_string());
    }
    if ability.applied_effects.iter().any(|e| e.apply_chance >= 100.0 && e.effect_type.is_crowd_control()) {
        warnings.push("100% CC application chance with no counter-play may be overpowered".to_string());
        suggestions.push("Reduce CC application chance or add a resistance check".to_string());
    }

    let total_score = eff_dps / expected_dps.max(1.0) + utility * 0.01 + resource_eff * 0.05;

    AbilityBalanceReport {
        ability_name: ability.name.clone(),
        effective_dps: eff_dps,
        expected_dps_for_cooldown: expected_dps,
        is_overpowered: eff_dps > expected_dps + tolerance,
        is_underpowered: eff_dps < expected_dps - tolerance && ability.ability_type.is_damage_dealing(),
        resource_efficiency: resource_eff,
        utility_score: utility,
        total_score,
        warnings,
        suggestions,
    }
}

fn expected_dps_for_cd(cooldown: f32, ability_type: AbilityType) -> f32 {
    let type_multiplier = match ability_type {
        AbilityType::MeleeAttack => 0.8,
        AbilityType::RangedAttack => 0.9,
        AbilityType::AreaOfEffect => 1.5,
        AbilityType::Nova => 1.4,
        AbilityType::Beam => 1.1,
        AbilityType::ChainLightning => 1.6,
        AbilityType::Channel => 1.3,
        _ => 1.0,
    };
    // Base 100 DPS at 0s CD, scales down for longer CDs
    let base = 100.0 * type_multiplier;
    base * (1.0 / (1.0 + cooldown * 0.05))
}

// ============================================================
// EXTENDED: EFFECT PRIORITY QUEUE (for game simulation)
// ============================================================

#[derive(Debug, Clone)]
pub struct ScheduledEffect {
    pub execute_at: f32,
    pub effect_type: ScheduledEffectType,
    pub source_ability_id: u64,
    pub target_id: u64,
}

#[derive(Debug, Clone)]
pub enum ScheduledEffectType {
    ApplyStatus(u64),   // status effect definition id
    DealDamage(DamageFormula, HashMap<String, f32>),
    Heal(f32),
    TriggerAbility(u64),
    SpawnProjectile { origin: Vec3, direction: Vec3, ability_id: u64 },
}

pub struct EffectScheduler {
    pub queue: Vec<ScheduledEffect>, // maintained sorted by execute_at
    pub current_time: f32,
}

impl EffectScheduler {
    pub fn new() -> Self {
        EffectScheduler { queue: Vec::new(), current_time: 0.0 }
    }

    pub fn schedule(&mut self, at: f32, effect: ScheduledEffectType, source: u64, target: u64) {
        let entry = ScheduledEffect { execute_at: at, effect_type: effect, source_ability_id: source, target_id: target };
        let pos = self.queue.partition_point(|e| e.execute_at <= at);
        self.queue.insert(pos, entry);
    }

    pub fn pop_ready(&mut self, now: f32) -> Vec<ScheduledEffect> {
        self.current_time = now;
        let split = self.queue.partition_point(|e| e.execute_at <= now);
        self.queue.drain(..split).collect()
    }

    pub fn cancel_for_ability(&mut self, ability_id: u64) {
        self.queue.retain(|e| e.source_ability_id != ability_id);
    }

    pub fn next_scheduled_time(&self) -> Option<f32> {
        self.queue.first().map(|e| e.execute_at)
    }
}

// ============================================================
// FINAL ENTRY POINT
// ============================================================

pub fn ability_editor_main() {
    let mut editor = build_ability_editor();

    // Test formula tester
    editor.formula_tester.selected_ability_id = editor.database.abilities.keys().next().copied();
    editor.run_formula_test_now();

    // Detect synergies
    let _synergies = detect_synergies(&editor.database);

    // Rank abilities
    let config = FormulaTesterConfig::default_lvl_20();
    let _dps_ranking = rank_abilities_by_dps(&editor.database, &config);
    let _cd_table = generate_cooldown_preview_table(&editor.database);

    // Balance check all abilities
    let ability_ids: Vec<u64> = editor.database.abilities.keys().copied().collect();
    for id in &ability_ids {
        if let Some(a) = editor.database.abilities.get(id) {
            let _report = check_ability_balance(a, &config);
        }
    }

    // Resource efficiency
    let _mana_ranking = rank_abilities_by_efficiency(&editor.database, &config, ResourceType::Mana);
    let _stamina_ranking = rank_abilities_by_efficiency(&editor.database, &config, ResourceType::Stamina);

    // Stance system
    let mut stance_mgr = StanceManager::new();
    let mut resources: HashMap<ResourceType, ResourceState> = HashMap::new();
    resources.insert(ResourceType::Rage, ResourceState::new(ResourceType::Rage));

    // Channel tick plan
    if let Some(a) = editor.database.abilities.values().find(|a| a.channel_ticks > 0) {
        let plan = ChannelTickPlan::build(a, &config);
        let _dmg_at_half = plan.damage_at_time(a.channel_time * 0.5);
        let _dps = plan.dps();
    }

    // VFX previews
    for (_, ability) in &editor.database.abilities {
        let _vfx = build_vfx_preview(ability);
    }

    // Effect scheduler test
    let mut scheduler = EffectScheduler::new();
    scheduler.schedule(0.5, ScheduledEffectType::Heal(50.0), 1, 100);
    scheduler.schedule(1.0, ScheduledEffectType::ApplyStatus(1), 2, 100);
    let _ready = scheduler.pop_ready(0.8);

    // Talent optimizer
    for (tree_id, tree) in &editor.database.talent_trees {
        let goal = TalentOptimizationGoal {
            maximize_stat: "attack_power".to_string(),
            secondary_stats: vec![("stamina".to_string(), 0.5)],
            required_abilities: Vec::new(),
            budget_points: 10,
        };
        let _result = greedy_talent_optimizer(tree, &goal);
    }

    // Status timeline
    let status_defs: Vec<&StatusEffectDefinition> = editor.database.status_effects.values().collect();
    if !status_defs.is_empty() {
        let pairs: Vec<(&StatusEffectDefinition, u32)> = status_defs.iter().map(|d| (*d, 2)).collect();
        let stats = HashMap::new();
        let frames = simulate_status_timeline(&pairs, &stats, 10.0, 0.1);
        let _summary = summarize_timeline(&frames);
    }

    // Interaction matrix
    let matrix = InteractionMatrix::new();
    let mut mock_effects = Vec::new();
    let _reactions = matrix.process_interactions(&mut mock_effects, StatusEffectType::Burn);

    // Proc system
    let mut proc_mgr = ProcManager::new();
    proc_mgr.add_proc(ProcEffect {
        id: 1,
        name: "Flames of Fury".to_string(),
        trigger: ProcTrigger::OnCrit,
        chance: 15.0,
        internal_cooldown: 5.0,
        remaining_icd: 0.0,
        effect: ProcOutcome::DealBonusDamage { formula: DamageFormula::new(DamageElement::Fire, 20.0, 35.0) },
    });
    proc_mgr.tick(1.0);
    let rolls = vec![0.1, 0.05, 0.5];
    let _triggered = proc_mgr.check_procs(&ProcTrigger::OnCrit, &rolls);

    // Search index
    let idx = AbilitySearchIndex::build(&editor.database);
    let _fireball_ids = idx.search_by_tokens("fireball");
    let _low_cd = idx.abilities_in_cd_range(0.0, 5.0);

    println!("AbilityEditor initialized with {} abilities, {} status effects, {} talent trees",
        editor.database.abilities.len(),
        editor.database.status_effects.len(),
        editor.database.talent_trees.len()
    );
}

// ============================================================
// SECTION: ABILITY COMBO SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ComboTrigger {
    AbilityUsed(u64),
    StatusApplied(StatusEffectType),
    HitLanded,
    KillSecured,
    ResourceThreshold { resource: ResourceType, threshold_pct: f32, above: bool },
    TimeElapsed(f32),
}

#[derive(Debug, Clone)]
pub struct ComboStep {
    pub trigger: ComboTrigger,
    pub time_window: f32,
    pub required_index: usize,
}

#[derive(Debug, Clone)]
pub struct ComboDefinition {
    pub id: u64,
    pub name: String,
    pub steps: Vec<ComboStep>,
    pub reward_ability_id: Option<u64>,
    pub reward_modifiers: Vec<(String, f32)>,
    pub reward_duration: f32,
}

#[derive(Debug, Clone)]
pub struct ComboTracker {
    pub definition: ComboDefinition,
    pub current_step: usize,
    pub step_timestamp: f32,
    pub active_reward_timer: f32,
}

impl ComboTracker {
    pub fn new(definition: ComboDefinition) -> Self {
        Self { definition, current_step: 0, step_timestamp: 0.0, active_reward_timer: 0.0 }
    }

    pub fn advance(&mut self, trigger: &ComboTrigger, current_time: f32) -> bool {
        if self.current_step >= self.definition.steps.len() { return false; }
        let step = &self.definition.steps[self.current_step];
        let elapsed = current_time - self.step_timestamp;
        if elapsed > step.time_window && self.current_step > 0 {
            self.current_step = 0;
            self.step_timestamp = current_time;
        }
        if &step.trigger == trigger {
            self.current_step += 1;
            self.step_timestamp = current_time;
            if self.current_step >= self.definition.steps.len() {
                self.current_step = 0;
                self.active_reward_timer = self.definition.reward_duration;
                return true; // Combo completed
            }
        } else {
            self.current_step = 0;
            self.step_timestamp = current_time;
        }
        false
    }

    pub fn update_reward_timer(&mut self, dt: f32) {
        if self.active_reward_timer > 0.0 {
            self.active_reward_timer = (self.active_reward_timer - dt).max(0.0);
        }
    }

    pub fn reward_active(&self) -> bool { self.active_reward_timer > 0.0 }

    pub fn progress_fraction(&self) -> f32 {
        if self.definition.steps.is_empty() { return 0.0; }
        self.current_step as f32 / self.definition.steps.len() as f32
    }
}

#[derive(Debug)]
pub struct ComboManager {
    pub trackers: Vec<ComboTracker>,
}

impl ComboManager {
    pub fn new() -> Self { Self { trackers: Vec::new() } }

    pub fn add_combo(&mut self, definition: ComboDefinition) {
        self.trackers.push(ComboTracker::new(definition));
    }

    pub fn process_trigger(&mut self, trigger: &ComboTrigger, current_time: f32) -> Vec<u64> {
        let mut completed = Vec::new();
        for tracker in &mut self.trackers {
            if tracker.advance(trigger, current_time) {
                completed.push(tracker.definition.id);
            }
        }
        completed
    }

    pub fn update(&mut self, dt: f32) {
        for tracker in &mut self.trackers {
            tracker.update_reward_timer(dt);
        }
    }

    pub fn active_rewards(&self) -> Vec<&ComboDefinition> {
        self.trackers.iter().filter(|t| t.reward_active()).map(|t| &t.definition).collect()
    }
}

// ============================================================
// SECTION: ABILITY CAST QUEUE
// ============================================================

#[derive(Debug, Clone)]
pub struct QueuedCast {
    pub ability_id: u64,
    pub target_position: Vec3,
    pub target_entity: Option<u64>,
    pub queued_at: f32,
    pub expire_at: f32,
}

#[derive(Debug)]
pub struct AbilityCastQueue {
    pub queue: VecDeque<QueuedCast>,
    pub max_queue_size: usize,
    pub queue_window: f32,
}

impl AbilityCastQueue {
    pub fn new(max_queue_size: usize, queue_window: f32) -> Self {
        Self { queue: VecDeque::new(), max_queue_size, queue_window }
    }

    pub fn enqueue(&mut self, cast: QueuedCast) -> bool {
        if self.queue.len() >= self.max_queue_size { return false; }
        self.queue.push_back(cast);
        true
    }

    pub fn pop_valid(&mut self, current_time: f32) -> Option<QueuedCast> {
        while let Some(front) = self.queue.front() {
            if front.expire_at < current_time {
                self.queue.pop_front();
            } else {
                return self.queue.pop_front();
            }
        }
        None
    }

    pub fn expire_old(&mut self, current_time: f32) {
        self.queue.retain(|c| c.expire_at >= current_time);
    }

    pub fn clear(&mut self) { self.queue.clear(); }
}

// ============================================================
// SECTION: STATUS IMMUNITY TRACKER
// ============================================================

#[derive(Debug, Clone)]
pub struct ImmunityWindow {
    pub effect_type: StatusEffectType,
    pub expires_at: f32,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct StatusImmunityTracker {
    pub entity_id: u64,
    pub immunities: Vec<ImmunityWindow>,
    pub tenacity: f32,
}

impl StatusImmunityTracker {
    pub fn new(entity_id: u64, tenacity: f32) -> Self {
        Self { entity_id, immunities: Vec::new(), tenacity }
    }

    pub fn add_immunity(&mut self, effect_type: StatusEffectType, duration: f32, current_time: f32, source: String) {
        self.immunities.push(ImmunityWindow { effect_type, expires_at: current_time + duration, source });
    }

    pub fn add_post_effect_immunity(&mut self, effect_type: StatusEffectType, base_duration: f32, current_time: f32) {
        let duration = base_duration * 0.5;
        self.add_immunity(effect_type, duration, current_time, "post_effect_immunity".to_string());
    }

    pub fn is_immune(&self, effect_type: &StatusEffectType, current_time: f32) -> bool {
        self.immunities.iter().any(|imm| &imm.effect_type == effect_type && imm.expires_at > current_time)
    }

    pub fn effective_duration(&self, base_duration: f32) -> f32 {
        base_duration * (1.0 - self.tenacity / 100.0).max(0.0)
    }

    pub fn update(&mut self, current_time: f32) {
        self.immunities.retain(|imm| imm.expires_at > current_time);
    }
}

// ============================================================
// SECTION: PARABOLIC ARC TARGETING
// ============================================================

#[derive(Debug, Clone)]
pub struct ParabolicArcParams {
    pub launch_angle_deg: f32,
    pub gravity: f32,
    pub initial_speed: f32,
}

impl ParabolicArcParams {
    pub fn launch_velocity(&self, origin: Vec3, target: Vec3) -> Vec3 {
        let diff = target - origin;
        let horizontal_dist = (diff.x * diff.x + diff.z * diff.z).sqrt();
        let angle_rad = self.launch_angle_deg.to_radians();
        let speed = self.initial_speed;
        let vy = speed * angle_rad.sin();
        let horizontal_speed = speed * angle_rad.cos();
        let horiz_dir = if horizontal_dist > 1e-6 {
            Vec3::new(diff.x / horizontal_dist, 0.0, diff.z / horizontal_dist)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        horiz_dir * horizontal_speed + Vec3::new(0.0, vy, 0.0)
    }

    pub fn time_of_flight(&self, launch_velocity: Vec3, height_diff: f32) -> f32 {
        let vy = launch_velocity.y;
        let g = self.gravity;
        // Solve: height_diff = vy*t - 0.5*g*t^2
        let discriminant = vy * vy + 2.0 * g * height_diff;
        if discriminant < 0.0 { return 0.0; }
        let t1 = (vy + discriminant.sqrt()) / g;
        let t2 = (vy - discriminant.sqrt()) / g;
        t1.max(t2).max(0.0)
    }

    pub fn position_at_time(&self, origin: Vec3, launch_velocity: Vec3, t: f32) -> Vec3 {
        origin + launch_velocity * t + Vec3::new(0.0, -0.5 * self.gravity * t * t, 0.0)
    }

    pub fn trajectory_points(&self, origin: Vec3, launch_velocity: Vec3, tof: f32, steps: usize) -> Vec<Vec3> {
        (0..=steps).map(|i| {
            let t = tof * i as f32 / steps as f32;
            self.position_at_time(origin, launch_velocity, t)
        }).collect()
    }

    pub fn apex(&self, origin: Vec3, launch_velocity: Vec3) -> Vec3 {
        let t_apex = launch_velocity.y / self.gravity;
        self.position_at_time(origin, launch_velocity, t_apex.max(0.0))
    }
}

// ============================================================
// SECTION: LINE-OF-SIGHT CHECKER
// ============================================================

#[derive(Debug, Clone)]
pub struct OccluderSegment {
    pub a: Vec2,
    pub b: Vec2,
    pub height: f32,
}

pub fn segments_intersect_2d(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> bool {
    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let cross = d1.x * d2.y - d1.y * d2.x;
    if cross.abs() < 1e-9 { return false; }
    let diff = p3 - p1;
    let t = (diff.x * d2.y - diff.y * d2.x) / cross;
    let u = (diff.x * d1.y - diff.y * d1.x) / cross;
    t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0
}

pub fn has_line_of_sight(origin: Vec2, target: Vec2, occluders: &[OccluderSegment], caster_height: f32, target_height: f32) -> bool {
    for occ in occluders {
        if caster_height > occ.height && target_height > occ.height { continue; }
        if segments_intersect_2d(origin, target, occ.a, occ.b) { return false; }
    }
    true
}

pub fn find_first_occluder(origin: Vec2, direction: Vec2, max_range: f32, occluders: &[OccluderSegment]) -> Option<(f32, usize)> {
    let target = origin + direction * max_range;
    let mut closest: Option<(f32, usize)> = None;
    for (i, occ) in occluders.iter().enumerate() {
        let d1 = direction * max_range;
        let d2 = occ.b - occ.a;
        let cross = d1.x * d2.y - d1.y * d2.x;
        if cross.abs() < 1e-9 { continue; }
        let diff = occ.a - origin;
        let t = (diff.x * d2.y - diff.y * d2.x) / cross;
        let u = (diff.x * d1.y - diff.y * d1.x) / cross;
        if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
            let dist = t * max_range;
            if closest.map(|(d, _)| dist < d).unwrap_or(true) {
                closest = Some((dist, i));
            }
        }
    }
    let _ = target;
    closest
}

// ============================================================
// SECTION: ABILITY HOTBAR MANAGEMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct HotbarSlot {
    pub slot_index: usize,
    pub ability_id: Option<u64>,
    pub keybind: Option<String>,
    pub cooldown_remaining: f32,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct AbilityHotbar {
    pub slots: Vec<HotbarSlot>,
    pub global_cooldown: f32,
    pub global_cooldown_remaining: f32,
}

impl AbilityHotbar {
    pub fn new(slot_count: usize) -> Self {
        let slots = (0..slot_count).map(|i| HotbarSlot {
            slot_index: i,
            ability_id: None,
            keybind: None,
            cooldown_remaining: 0.0,
            active: false,
        }).collect();
        Self { slots, global_cooldown: 1.5, global_cooldown_remaining: 0.0 }
    }

    pub fn assign(&mut self, slot_index: usize, ability_id: u64, keybind: Option<String>) {
        if let Some(slot) = self.slots.get_mut(slot_index) {
            slot.ability_id = Some(ability_id);
            slot.keybind = keybind;
        }
    }

    pub fn unassign(&mut self, slot_index: usize) {
        if let Some(slot) = self.slots.get_mut(slot_index) {
            slot.ability_id = None;
            slot.keybind = None;
        }
    }

    pub fn can_use(&self, slot_index: usize) -> bool {
        if self.global_cooldown_remaining > 0.0 { return false; }
        self.slots.get(slot_index).map(|s| s.cooldown_remaining <= 0.0 && s.ability_id.is_some()).unwrap_or(false)
    }

    pub fn trigger(&mut self, slot_index: usize, ability_cooldown: f32) -> Option<u64> {
        if !self.can_use(slot_index) { return None; }
        let ability_id = self.slots[slot_index].ability_id;
        if let Some(slot) = self.slots.get_mut(slot_index) {
            slot.cooldown_remaining = ability_cooldown;
        }
        self.global_cooldown_remaining = self.global_cooldown;
        ability_id
    }

    pub fn update(&mut self, dt: f32) {
        self.global_cooldown_remaining = (self.global_cooldown_remaining - dt).max(0.0);
        for slot in &mut self.slots {
            slot.cooldown_remaining = (slot.cooldown_remaining - dt).max(0.0);
        }
    }

    pub fn find_slot_by_ability(&self, ability_id: u64) -> Option<usize> {
        self.slots.iter().find(|s| s.ability_id == Some(ability_id)).map(|s| s.slot_index)
    }

    pub fn swap_slots(&mut self, a: usize, b: usize) {
        if a < self.slots.len() && b < self.slots.len() {
            let (ability_a, keybind_a) = (self.slots[a].ability_id, self.slots[a].keybind.clone());
            let (ability_b, keybind_b) = (self.slots[b].ability_id, self.slots[b].keybind.clone());
            self.slots[a].ability_id = ability_b;
            self.slots[a].keybind = keybind_b;
            self.slots[b].ability_id = ability_a;
            self.slots[b].keybind = keybind_a;
        }
    }
}

// ============================================================
// SECTION: ABILITY SERIALIZATION (TEXT FORMAT)
// ============================================================

pub fn serialize_ability_brief(ability: &AbilityDefinition) -> String {
    let mut parts = Vec::new();
    parts.push(format!("id={}", ability.id));
    parts.push(format!("name=\"{}\"", ability.name));
    parts.push(format!("type={:?}", ability.ability_type));
    parts.push(format!("cooldown={:.2}", ability.base_cooldown));
    parts.push(format!("cast_time={:.2}", ability.cast_time));
    parts.push(format!("range={:.1}", ability.range));
    parts.join(";")
}

pub fn serialize_damage_formula(formula: &DamageFormula) -> String {
    format!(
        "element={:?};base={:.0}-{:.0};crit_chance={:.2};crit_mult={:.2};pen={:.0};pen_pct={:.2}",
        formula.element, formula.base_min, formula.base_max,
        formula.crit_chance_base, formula.crit_multiplier_base,
        formula.penetration, formula.penetration_percent
    )
}

pub fn parse_scaling_coefficients(input: &str) -> Vec<ScalingCoeff> {
    let mut results = Vec::new();
    for part in input.split(',') {
        let trimmed = part.trim();
        let tokens: Vec<&str> = trimmed.split(':').collect();
        if tokens.len() == 2 {
            if let Ok(coeff) = tokens[1].trim().parse::<f32>() {
                let stat_name = tokens[0].trim().to_string();
                results.push(ScalingCoeff { stat_name, coefficient: coeff, exponent: 1.0 });
            }
        }
    }
    results
}

pub fn ability_to_description_text(ability: &AbilityDefinition) -> String {
    let mut lines = Vec::new();
    lines.push(format!("=== {} ===", ability.name));
    lines.push(format!("Type: {:?}", ability.ability_type));
    if !ability.description.is_empty() {
        lines.push(format!("Description: {}", ability.description));
    }
    lines.push(format!("Cast Time: {:.2}s | Cooldown: {:.2}s | Range: {:.1}", ability.cast_time, ability.base_cooldown, ability.range));
    if ability.resource_cost > 0.0 {
        lines.push(format!("Cost: {:.0} {:?}", ability.resource_cost, ability.resource_type));
    }
    if !ability.damage_formulas.is_empty() {
        lines.push(format!("Damage ({} formula(s)):", ability.damage_formulas.len()));
        for f in &ability.damage_formulas {
            lines.push(format!("  {:?}: {:.0}-{:.0} base, {:.0}% crit chance", f.element, f.base_min, f.base_max, f.crit_chance_base * 100.0));
        }
    }
    if !ability.applied_effects.is_empty() {
        lines.push(format!("Applied Effects:"));
        for eff in &ability.applied_effects {
            lines.push(format!("  {:?} for {:.1}s ({}%)", eff.effect_type, eff.duration_override.unwrap_or(0.0), (eff.apply_chance * 100.0) as u32));
        }
    }
    lines.join("\n")
}

// ============================================================
// SECTION: AREA DENIAL ABILITY SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct AreaDenialZone {
    pub id: u64,
    pub ability_id: u64,
    pub caster_id: u64,
    pub position: Vec3,
    pub radius: f32,
    pub duration_remaining: f32,
    pub tick_interval: f32,
    pub tick_accumulator: f32,
    pub damage_per_tick: f32,
    pub effect_per_tick: Option<StatusEffectType>,
    pub effect_duration: f32,
    pub tags: Vec<String>,
}

impl AreaDenialZone {
    pub fn new(id: u64, ability_id: u64, caster_id: u64, position: Vec3, radius: f32, duration: f32, damage_per_tick: f32, tick_interval: f32) -> Self {
        Self {
            id, ability_id, caster_id, position, radius, duration_remaining: duration,
            tick_interval, tick_accumulator: 0.0, damage_per_tick, effect_per_tick: None,
            effect_duration: 2.0, tags: Vec::new(),
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.duration_remaining -= dt;
        self.tick_accumulator += dt;
        self.duration_remaining > 0.0
    }

    pub fn should_tick(&mut self) -> bool {
        if self.tick_accumulator >= self.tick_interval {
            self.tick_accumulator -= self.tick_interval;
            true
        } else { false }
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        let dx = point.x - self.position.x;
        let dz = point.z - self.position.z;
        dx * dx + dz * dz <= self.radius * self.radius
    }

    pub fn lifetime_fraction(&self, total_duration: f32) -> f32 {
        1.0 - (self.duration_remaining / total_duration).clamp(0.0, 1.0)
    }
}

#[derive(Debug)]
pub struct AreaDenialManager {
    pub zones: Vec<AreaDenialZone>,
    pub next_zone_id: u64,
}

impl AreaDenialManager {
    pub fn new() -> Self { Self { zones: Vec::new(), next_zone_id: 1 } }

    pub fn spawn_zone(&mut self, ability_id: u64, caster_id: u64, position: Vec3, radius: f32, duration: f32, damage_per_tick: f32, tick_interval: f32) -> u64 {
        let id = self.next_zone_id;
        self.next_zone_id += 1;
        self.zones.push(AreaDenialZone::new(id, ability_id, caster_id, position, radius, duration, damage_per_tick, tick_interval));
        id
    }

    pub fn update(&mut self, dt: f32) -> Vec<(u64, Vec<u64>)> {
        // Returns: zone_id -> list of entity_ids to tick (caller supplies entity positions)
        self.zones.retain_mut(|z| z.update(dt));
        Vec::new() // In real use, caller queries entity positions against zones
    }

    pub fn query_zones_at(&self, point: Vec3) -> Vec<&AreaDenialZone> {
        self.zones.iter().filter(|z| z.contains_point(point)).collect()
    }

    pub fn remove_zone(&mut self, zone_id: u64) {
        self.zones.retain(|z| z.id != zone_id);
    }

    pub fn caster_zones(&self, caster_id: u64) -> Vec<&AreaDenialZone> {
        self.zones.iter().filter(|z| z.caster_id == caster_id).collect()
    }
}

// ============================================================
// SECTION: ABILITY INTERRUPT SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum InterruptType { Stun, Silence, Knockback, Knockup, Pushback, AbilityCancel }

#[derive(Debug, Clone)]
pub struct InterruptEvent {
    pub interrupt_type: InterruptType,
    pub source_entity: u64,
    pub magnitude: f32,
    pub interrupt_time: f32,
}

#[derive(Debug, Clone)]
pub struct CastInterruptData {
    pub was_interrupted: bool,
    pub interrupt_type: Option<InterruptType>,
    pub progress_at_interrupt: f32,
    pub refund_pct: f32,
}

pub fn compute_cast_interrupt(ability: &AbilityDefinition, interrupt: &InterruptEvent, cast_progress: f32) -> CastInterruptData {
    let interruptible = match interrupt.interrupt_type {
        InterruptType::Stun | InterruptType::Knockback | InterruptType::Knockup => true,
        InterruptType::Silence => matches!(ability.ability_type, AbilityType::Buff | AbilityType::Heal | AbilityType::Summon),
        InterruptType::Pushback => ability.cast_time > 0.5,
        InterruptType::AbilityCancel => true,
    };
    if !interruptible {
        return CastInterruptData { was_interrupted: false, interrupt_type: None, progress_at_interrupt: cast_progress, refund_pct: 0.0 };
    }
    let refund_pct = if cast_progress < 0.3 { 1.0 } else if cast_progress < 0.7 { 0.5 } else { 0.0 };
    CastInterruptData {
        was_interrupted: true,
        interrupt_type: Some(interrupt.interrupt_type.clone()),
        progress_at_interrupt: cast_progress,
        refund_pct,
    }
}

// ============================================================
// SECTION: ABILITY AURA SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct AuraDefinition {
    pub id: u64,
    pub name: String,
    pub ability_id: u64,
    pub radius: f32,
    pub affects_allies: bool,
    pub affects_enemies: bool,
    pub affects_self: bool,
    pub stat_modifiers: Vec<(String, f32)>,
    pub tick_effects: Vec<StatusEffectType>,
    pub tick_interval: f32,
    pub reserved_resource: Option<(ResourceType, f32)>,
}

#[derive(Debug, Clone)]
pub struct ActiveAura {
    pub definition_id: u64,
    pub caster_id: u64,
    pub position: Vec3,
    pub tick_accumulator: f32,
}

impl ActiveAura {
    pub fn new(definition_id: u64, caster_id: u64, position: Vec3) -> Self {
        Self { definition_id, caster_id, position, tick_accumulator: 0.0 }
    }

    pub fn update_position(&mut self, new_pos: Vec3) { self.position = new_pos; }

    pub fn tick(&mut self, dt: f32, tick_interval: f32) -> bool {
        self.tick_accumulator += dt;
        if self.tick_accumulator >= tick_interval {
            self.tick_accumulator -= tick_interval;
            true
        } else { false }
    }

    pub fn affects_point(&self, point: Vec3, radius: f32) -> bool {
        let dx = point.x - self.position.x;
        let dz = point.z - self.position.z;
        dx * dx + dz * dz <= radius * radius
    }
}

#[derive(Debug)]
pub struct AuraManager {
    pub aura_defs: HashMap<u64, AuraDefinition>,
    pub active_auras: Vec<ActiveAura>,
}

impl AuraManager {
    pub fn new() -> Self { Self { aura_defs: HashMap::new(), active_auras: Vec::new() } }

    pub fn register_aura(&mut self, def: AuraDefinition) {
        self.aura_defs.insert(def.id, def);
    }

    pub fn activate_aura(&mut self, def_id: u64, caster_id: u64, position: Vec3) -> bool {
        if !self.aura_defs.contains_key(&def_id) { return false; }
        // Remove existing aura of same type from same caster
        self.active_auras.retain(|a| !(a.definition_id == def_id && a.caster_id == caster_id));
        self.active_auras.push(ActiveAura::new(def_id, caster_id, position));
        true
    }

    pub fn deactivate_aura(&mut self, def_id: u64, caster_id: u64) {
        self.active_auras.retain(|a| !(a.definition_id == def_id && a.caster_id == caster_id));
    }

    pub fn auras_affecting_point(&self, point: Vec3) -> Vec<(&ActiveAura, &AuraDefinition)> {
        self.active_auras.iter()
            .filter_map(|a| {
                self.aura_defs.get(&a.definition_id).and_then(|def| {
                    if a.affects_point(point, def.radius) { Some((a, def)) } else { None }
                })
            })
            .collect()
    }

    pub fn update_positions(&mut self, positions: &HashMap<u64, Vec3>) {
        for aura in &mut self.active_auras {
            if let Some(&pos) = positions.get(&aura.caster_id) {
                aura.update_position(pos);
            }
        }
    }

    pub fn stat_bonus_at_point(&self, point: Vec3, stat_name: &str, entity_is_ally: bool) -> f32 {
        self.auras_affecting_point(point).iter()
            .filter(|(_, def)| (entity_is_ally && def.affects_allies) || (!entity_is_ally && def.affects_enemies))
            .flat_map(|(_, def)| def.stat_modifiers.iter())
            .filter(|(name, _)| name == stat_name)
            .map(|(_, val)| val)
            .sum()
    }
}

// ============================================================
// SECTION: ABILITY TAG SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AbilityTag {
    Spell, Attack, Projectile, AoE, Channeled, Instant, Movement, Summon,
    Fire, Cold, Lightning, Chaos, Physical, Arcane,
    Buff, Debuff, Heal, Support, Ultimate, Basic,
    Melee, Ranged, Ground, Self_,
}

#[derive(Debug, Clone)]
pub struct AbilityTagFilter {
    pub required_all: Vec<AbilityTag>,
    pub required_any: Vec<AbilityTag>,
    pub excluded: Vec<AbilityTag>,
}

impl AbilityTagFilter {
    pub fn matches(&self, tags: &HashSet<AbilityTag>) -> bool {
        for req in &self.required_all {
            if !tags.contains(req) { return false; }
        }
        if !self.required_any.is_empty() {
            if !self.required_any.iter().any(|t| tags.contains(t)) { return false; }
        }
        for excl in &self.excluded {
            if tags.contains(excl) { return false; }
        }
        true
    }
}

pub fn infer_tags_from_ability(ability: &AbilityDefinition) -> HashSet<AbilityTag> {
    let mut tags = HashSet::new();
    match ability.ability_type {
        AbilityType::MeleeAttack | AbilityType::Charge => { tags.insert(AbilityTag::Attack); tags.insert(AbilityTag::Melee); }
        AbilityType::RangedAttack | AbilityType::Projectile => { tags.insert(AbilityTag::Attack); tags.insert(AbilityTag::Ranged); tags.insert(AbilityTag::Projectile); }
        AbilityType::AreaOfEffect | AbilityType::Nova => { tags.insert(AbilityTag::Spell); tags.insert(AbilityTag::AoE); }
        AbilityType::Buff => { tags.insert(AbilityTag::Spell); tags.insert(AbilityTag::Buff); }
        AbilityType::Debuff | AbilityType::Curse => { tags.insert(AbilityTag::Spell); tags.insert(AbilityTag::Debuff); }
        AbilityType::Heal => { tags.insert(AbilityTag::Spell); tags.insert(AbilityTag::Heal); }
        AbilityType::Summon | AbilityType::Totem | AbilityType::Pet => { tags.insert(AbilityTag::Summon); }
        AbilityType::Dash | AbilityType::Teleport => { tags.insert(AbilityTag::Movement); }
        AbilityType::Channel => { tags.insert(AbilityTag::Channeled); }
        _ => {}
    }
    if ability.cast_time == 0.0 { tags.insert(AbilityTag::Instant); }
    if ability.aoe_radius > 0.0 { tags.insert(AbilityTag::AoE); }
    for formula in &ability.damage_formulas {
        match formula.element {
            DamageElement::Fire => { tags.insert(AbilityTag::Fire); tags.insert(AbilityTag::Spell); }
            DamageElement::Cold => { tags.insert(AbilityTag::Cold); tags.insert(AbilityTag::Spell); }
            DamageElement::Lightning => { tags.insert(AbilityTag::Lightning); tags.insert(AbilityTag::Spell); }
            DamageElement::Physical => { tags.insert(AbilityTag::Physical); }
            DamageElement::Arcane => { tags.insert(AbilityTag::Arcane); tags.insert(AbilityTag::Spell); }
            _ => {}
        }
    }
    tags
}

// ============================================================
// SECTION: ABILITY UNLOCK SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub enum UnlockCondition {
    CharacterLevel(u32),
    AbilityKnown(u64),
    ClassIs(String),
    QuestCompleted(u64),
    AttributeMinimum { stat_name: String, min_value: f32 },
    PointsSpentInTree { tree_id: u64, min_points: u32 },
}

#[derive(Debug, Clone)]
pub struct AbilityUnlockEntry {
    pub ability_id: u64,
    pub conditions: Vec<UnlockCondition>,
    pub cost_points: u32,
    pub cost_gold: u64,
    pub one_time_unlock: bool,
    pub display_category: String,
}

#[derive(Debug)]
pub struct AbilityUnlockBook {
    pub entries: HashMap<u64, AbilityUnlockEntry>,
    pub unlocked: HashSet<u64>,
}

impl AbilityUnlockBook {
    pub fn new() -> Self { Self { entries: HashMap::new(), unlocked: HashSet::new() } }

    pub fn register(&mut self, entry: AbilityUnlockEntry) {
        self.entries.insert(entry.ability_id, entry);
    }

    pub fn can_unlock(&self, ability_id: u64, char_level: u32, known_abilities: &HashSet<u64>, class: &str, available_points: u32) -> bool {
        let entry = match self.entries.get(&ability_id) { Some(e) => e, None => return false };
        if self.unlocked.contains(&ability_id) { return false; }
        if available_points < entry.cost_points { return false; }
        for cond in &entry.conditions {
            match cond {
                UnlockCondition::CharacterLevel(lvl) => { if char_level < *lvl { return false; } }
                UnlockCondition::AbilityKnown(id) => { if !known_abilities.contains(id) { return false; } }
                UnlockCondition::ClassIs(c) => { if c != class { return false; } }
                UnlockCondition::AttributeMinimum { .. } => {} // caller handles stat lookup
                UnlockCondition::PointsSpentInTree { .. } => {} // caller handles
                UnlockCondition::QuestCompleted(_) => {} // caller handles
            }
        }
        true
    }

    pub fn unlock(&mut self, ability_id: u64) -> bool {
        if self.entries.contains_key(&ability_id) {
            self.unlocked.insert(ability_id);
            true
        } else { false }
    }

    pub fn available_to_unlock(&self, char_level: u32, known_abilities: &HashSet<u64>, class: &str, available_points: u32) -> Vec<u64> {
        self.entries.keys()
            .filter(|&&id| self.can_unlock(id, char_level, known_abilities, class, available_points))
            .cloned()
            .collect()
    }
}

// ============================================================
// SECTION: DAMAGE OVER TIME (DOT) TRACKER
// ============================================================

#[derive(Debug, Clone)]
pub struct DotInstance {
    pub id: u64,
    pub source_ability_id: u64,
    pub caster_id: u64,
    pub target_id: u64,
    pub element: DamageElement,
    pub damage_per_tick: f32,
    pub tick_interval: f32,
    pub duration_remaining: f32,
    pub tick_accumulator: f32,
    pub stack_count: u32,
    pub max_stacks: u32,
    pub can_crit: bool,
    pub crit_chance: f32,
    pub crit_multiplier: f32,
}

impl DotInstance {
    pub fn new(id: u64, source_ability_id: u64, caster_id: u64, target_id: u64, element: DamageElement, damage_per_tick: f32, tick_interval: f32, duration: f32) -> Self {
        Self {
            id, source_ability_id, caster_id, target_id, element,
            damage_per_tick, tick_interval, duration_remaining: duration,
            tick_accumulator: 0.0, stack_count: 1, max_stacks: 1,
            can_crit: false, crit_chance: 0.05, crit_multiplier: 1.5,
        }
    }

    pub fn update(&mut self, dt: f32) -> (bool, bool) {
        self.duration_remaining -= dt;
        self.tick_accumulator += dt;
        let ticked = self.tick_accumulator >= self.tick_interval;
        if ticked { self.tick_accumulator -= self.tick_interval; }
        let alive = self.duration_remaining > 0.0;
        (alive, ticked)
    }

    pub fn effective_damage_per_tick(&self) -> f32 {
        self.damage_per_tick * self.stack_count as f32
    }

    pub fn add_stack(&mut self, new_instance: &DotInstance) {
        if self.stack_count < self.max_stacks {
            self.stack_count += 1;
        }
        // Refresh duration to whichever is longer
        self.duration_remaining = self.duration_remaining.max(new_instance.duration_remaining);
    }

    pub fn expected_total_damage(&self) -> f32 {
        let remaining_ticks = (self.duration_remaining / self.tick_interval).ceil();
        self.effective_damage_per_tick() * remaining_ticks
    }
}

#[derive(Debug)]
pub struct DotTracker {
    pub dots: Vec<DotInstance>,
    pub next_dot_id: u64,
}

impl DotTracker {
    pub fn new() -> Self { Self { dots: Vec::new(), next_dot_id: 1 } }

    pub fn apply(&mut self, mut instance: DotInstance) -> u64 {
        instance.id = self.next_dot_id;
        self.next_dot_id += 1;
        // Check for stacking with existing dot from same source on same target
        let existing = self.dots.iter_mut().find(|d| {
            d.source_ability_id == instance.source_ability_id && d.target_id == instance.target_id
        });
        if let Some(existing_dot) = existing {
            let clone = instance.clone();
            existing_dot.add_stack(&clone);
            return existing_dot.id;
        }
        let id = instance.id;
        self.dots.push(instance);
        id
    }

    pub fn update_all(&mut self, dt: f32) -> Vec<(u64, f32, DamageElement, bool)> {
        // Returns: (target_id, damage, element, is_crit)
        let mut damage_events = Vec::new();
        self.dots.retain_mut(|dot| {
            let (alive, ticked) = dot.update(dt);
            if ticked {
                let dmg = dot.effective_damage_per_tick();
                damage_events.push((dot.target_id, dmg, dot.element.clone(), false));
            }
            alive
        });
        damage_events
    }

    pub fn dispel_by_element(&mut self, target_id: u64, element: &DamageElement) {
        self.dots.retain(|d| !(d.target_id == target_id && &d.element == element));
    }

    pub fn total_dot_dps_on_target(&self, target_id: u64) -> f32 {
        self.dots.iter()
            .filter(|d| d.target_id == target_id)
            .map(|d| d.effective_damage_per_tick() / d.tick_interval)
            .sum()
    }
}

// ============================================================
// SECTION: ABILITY MODIFIER STACK (RUNTIME BUFFS/DEBUFFS)
// ============================================================

#[derive(Debug, Clone)]
pub struct RuntimeAbilityModifier {
    pub id: u64,
    pub target_ability_id: Option<u64>,
    pub target_tag_filter: Option<AbilityTagFilter>,
    pub stat_changes: Vec<(String, f32)>,
    pub mult_changes: Vec<(String, f32)>,
    pub duration: Option<f32>,
    pub source: String,
}

#[derive(Debug)]
pub struct RuntimeAbilityModStack {
    pub modifiers: Vec<(RuntimeAbilityModifier, f32)>,
    pub next_id: u64,
}

impl RuntimeAbilityModStack {
    pub fn new() -> Self { Self { modifiers: Vec::new(), next_id: 1 } }

    pub fn push(&mut self, mut modifier: RuntimeAbilityModifier, current_time: f32) -> u64 {
        modifier.id = self.next_id;
        self.next_id += 1;
        self.modifiers.push((modifier, current_time));
        self.next_id - 1
    }

    pub fn update(&mut self, current_time: f32) {
        self.modifiers.retain(|(m, start_time)| {
            match m.duration {
                None => true,
                Some(dur) => current_time - start_time < dur,
            }
        });
    }

    pub fn remove_by_id(&mut self, id: u64) {
        self.modifiers.retain(|(m, _)| m.id != id);
    }

    pub fn flat_bonus_for_ability(&self, ability_id: u64, ability_tags: &HashSet<AbilityTag>, stat_name: &str) -> f32 {
        self.modifiers.iter().filter(|(m, _)| {
            let applies_by_id = m.target_ability_id.map(|id| id == ability_id).unwrap_or(true);
            let applies_by_tag = m.target_tag_filter.as_ref().map(|f| f.matches(ability_tags)).unwrap_or(true);
            applies_by_id && applies_by_tag
        }).flat_map(|(m, _)| m.stat_changes.iter())
          .filter(|(name, _)| name == stat_name)
          .map(|(_, v)| v)
          .sum()
    }

    pub fn mult_bonus_for_ability(&self, ability_id: u64, ability_tags: &HashSet<AbilityTag>, stat_name: &str) -> f32 {
        self.modifiers.iter().filter(|(m, _)| {
            let applies_by_id = m.target_ability_id.map(|id| id == ability_id).unwrap_or(true);
            let applies_by_tag = m.target_tag_filter.as_ref().map(|f| f.matches(ability_tags)).unwrap_or(true);
            applies_by_id && applies_by_tag
        }).flat_map(|(m, _)| m.mult_changes.iter())
          .filter(|(name, _)| name == stat_name)
          .map(|(_, v)| v)
          .product()
    }
}

// ============================================================
// SECTION: ABILITY EDITOR STATE (EXTENDED)
// ============================================================

#[derive(Debug)]
pub struct AbilityEditorExtendedState {
    pub hotbar: AbilityHotbar,
    pub combo_manager: ComboManager,
    pub area_denial_manager: AreaDenialManager,
    pub aura_manager: AuraManager,
    pub dot_tracker: DotTracker,
    pub unlock_book: AbilityUnlockBook,
    pub cast_queue: AbilityCastQueue,
    pub runtime_mods: RuntimeAbilityModStack,
    pub immunity_trackers: HashMap<u64, StatusImmunityTracker>,
    pub selected_ability_tags: HashSet<AbilityTag>,
    pub tag_filter: Option<AbilityTagFilter>,
    pub parabolic_preview: Option<Vec<Vec3>>,
    pub los_occluders: Vec<OccluderSegment>,
}

impl AbilityEditorExtendedState {
    pub fn new() -> Self {
        Self {
            hotbar: AbilityHotbar::new(10),
            combo_manager: ComboManager::new(),
            area_denial_manager: AreaDenialManager::new(),
            aura_manager: AuraManager::new(),
            dot_tracker: DotTracker::new(),
            unlock_book: AbilityUnlockBook::new(),
            cast_queue: AbilityCastQueue::new(3, 0.5),
            runtime_mods: RuntimeAbilityModStack::new(),
            immunity_trackers: HashMap::new(),
            selected_ability_tags: HashSet::new(),
            tag_filter: None,
            parabolic_preview: None,
            los_occluders: Vec::new(),
        }
    }

    pub fn update(&mut self, dt: f32, current_time: f32) {
        self.hotbar.update(dt);
        self.combo_manager.update(dt);
        self.area_denial_manager.update(dt);
        self.dot_tracker.update_all(dt);
        self.runtime_mods.update(current_time);
        self.cast_queue.expire_old(current_time);
        for tracker in self.immunity_trackers.values_mut() {
            tracker.update(current_time);
        }
    }

    pub fn abilities_matching_filter(&self, database: &AbilityDatabase) -> Vec<u64> {
        match &self.tag_filter {
            None => database.abilities.keys().cloned().collect(),
            Some(filter) => database.abilities.iter()
                .filter(|(_, ab)| {
                    let tags = infer_tags_from_ability(ab);
                    filter.matches(&tags)
                })
                .map(|(id, _)| *id)
                .collect(),
        }
    }

    pub fn preview_parabolic_arc(&mut self, origin: Vec3, target: Vec3, params: &ParabolicArcParams) {
        let launch_vel = params.launch_velocity(origin, target);
        let tof = params.time_of_flight(launch_vel, target.y - origin.y);
        self.parabolic_preview = Some(params.trajectory_points(origin, launch_vel, tof, 32));
    }

    pub fn check_los(&self, origin: Vec2, target: Vec2, caster_height: f32, target_height: f32) -> bool {
        has_line_of_sight(origin, target, &self.los_occluders, caster_height, target_height)
    }
}

// ============================================================
// SECTION: ABILITY DATABASE SEARCH
// ============================================================

pub fn ability_fuzzy_search(database: &AbilityDatabase, query: &str, max_results: usize) -> Vec<u64> {
    let q = query.to_lowercase();
    let mut scored: Vec<(u64, usize)> = database.abilities.iter()
        .map(|(&id, ab)| {
            let name = ab.name.to_lowercase();
            let score = if name.contains(&q) { 0 }
                else { ability_edit_distance(&q, &name) };
            (id, score)
        })
        .collect();
    scored.sort_by_key(|(_, s)| *s);
    scored.truncate(max_results);
    scored.into_iter().map(|(id, _)| id).collect()
}

fn ability_edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i-1] == b[j-1] {
                dp[i-1][j-1]
            } else {
                1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1])
            };
        }
    }
    dp[m][n]
}

pub fn filter_abilities_by_type(database: &AbilityDatabase, ability_type: &AbilityType) -> Vec<u64> {
    database.abilities.iter()
        .filter(|(_, ab)| std::mem::discriminant(&ab.ability_type) == std::mem::discriminant(ability_type))
        .map(|(id, _)| *id)
        .collect()
}

pub fn abilities_sorted_by_cooldown(database: &AbilityDatabase) -> Vec<(u64, f32)> {
    let mut list: Vec<(u64, f32)> = database.abilities.iter()
        .map(|(&id, ab)| (id, ab.base_cooldown))
        .collect();
    list.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    list
}

pub fn abilities_by_resource_type(database: &AbilityDatabase, resource: &ResourceType) -> Vec<u64> {
    database.abilities.iter()
        .filter(|(_, ab)| std::mem::discriminant(&ab.resource_type) == std::mem::discriminant(resource))
        .map(|(id, _)| *id)
        .collect()
}

// ============================================================
// SECTION: DAMAGE SIMULATION (EXTENDED MONTE CARLO)
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtendedMcSimResult {
    pub samples: Vec<f32>,
    pub mean: f32,
    pub variance: f32,
    pub std_dev: f32,
    pub min: f32,
    pub max: f32,
    pub p10: f32,
    pub p25: f32,
    pub p50: f32,
    pub p75: f32,
    pub p90: f32,
    pub p95: f32,
    pub p99: f32,
    pub crit_rate_observed: f32,
    pub dps_at_mean: f32,
}

pub fn run_extended_monte_carlo(ability: &AbilityDefinition, stats: &HashMap<String, f32>, target_resist: f32, iterations: usize) -> ExtendedMcSimResult {
    let mut samples = Vec::with_capacity(iterations);
    let mut crit_count = 0usize;
    let mut seed = 12345u64;

    let lcg = |s: &mut u64| -> f32 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 33) as f32) / (u32::MAX as f32)
    };

    for _ in 0..iterations {
        let mut total_damage = 0.0f32;
        let mut had_crit = false;
        for formula in &ability.damage_formulas {
            let roll_t = lcg(&mut seed);
            let crit_roll = lcg(&mut seed);
            let result = formula.compute_final_damage(stats, target_resist, &[], roll_t, crit_roll);
            total_damage += result.final_damage;
            if result.is_crit { had_crit = true; }
        }
        if had_crit { crit_count += 1; }
        samples.push(total_damage);
    }

    let n = samples.len() as f32;
    let mean = samples.iter().sum::<f32>() / n;
    let variance = samples.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / n;
    let std_dev = variance.sqrt();
    let min = samples.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pct = |p: f32| sorted[((p / 100.0) * (sorted.len() - 1) as f32) as usize];

    let dps_at_mean = if ability.base_cooldown > 0.0 { mean / ability.base_cooldown } else { mean };
    let crit_rate_observed = crit_count as f32 / n;

    ExtendedMcSimResult {
        samples, mean, variance, std_dev, min, max,
        p10: pct(10.0), p25: pct(25.0), p50: pct(50.0),
        p75: pct(75.0), p90: pct(90.0), p95: pct(95.0), p99: pct(99.0),
        crit_rate_observed, dps_at_mean,
    }
}

// ============================================================
// SECTION: ABILITY EDITOR TOP-LEVEL DEMO EXTENSION
// ============================================================

pub fn ability_editor_extended_demo() {
    let mut db = AbilityDatabase::new();

    // Create a test ability
    let mut fireball = AbilityDefinition::new(1001, "Fireball", AbilityType::Projectile);
    fireball.description = "Hurls a blazing ball of fire that explodes on impact.".to_string();
    fireball.targeting_mode = TargetingMode::GroundTarget { indicator: AreaIndicator::Circle };
    fireball.range = 25.0;
    fireball.aoe_radius = 4.0;
    fireball.cast_time = 1.5;
    fireball.base_cooldown = 6.0;
    fireball.resource_cost = 80.0;
    fireball.resource_type = ResourceType::Mana;
    fireball.school = "Fire".to_string();
    fireball.unlock_level = 5;
    fireball.damage_formulas = vec![
        DamageFormula {
            element: DamageElement::Fire,
            base_min: 120.0,
            base_max: 180.0,
            scaling: vec![ScalingCoeff { stat_name: "SpellPower".to_string(), coefficient: 0.85, exponent: 1.0 }],
            crit_chance_base: 0.15,
            crit_multiplier_base: 1.75,
            crit_chance_scaling: Vec::new(),
            crit_multiplier_scaling: Vec::new(),
            penetration: 20.0,
            penetration_percent: 0.1,
            versus_status_bonus: Vec::new(),
            variance: 0.1,
        }
    ];
    fireball.applied_effects = vec![
        AppliedEffect {
            effect_id: 0,
            effect_type: StatusEffectType::Burn,
            apply_chance: 40.0,
            to_target: true,
            condition: None,
            stacks_applied: 1,
            duration_override: Some(4.0),
        }
    ];
    fireball.projectile_params = Some(ProjectileParams {
        speed: 20.0,
        acceleration: 0.0,
        gravity: 0.0,
        max_range: 30.0,
        pierce_count: 0,
        split_count: 0,
        fork_count: 0,
        chain_count: 0,
        chain_radius: 0.0,
        homing: false,
        homing_strength: 0.0,
        aoe_on_impact: true,
        aoe_radius: 4.0,
        visual_trail: String::new(),
        impact_effect: String::new(),
        size: 0.5,
    });

    db.abilities.insert(fireball.id, fireball);

    let stats: HashMap<String, f32> = [
        ("SpellPower".to_string(), 250.0_f32),
        ("CritChance".to_string(), 0.2),
        ("CritMultiplier".to_string(), 2.0),
    ].iter().cloned().collect();

    // Run extended MC
    if let Some(ability) = db.abilities.get(&1001) {
        let result = run_extended_monte_carlo(ability, &stats, 0.15, 1000);
        let _mean = result.mean;
        let _p95 = result.p95;
        let _dps = result.dps_at_mean;
    }

    // Hotbar management
    let mut ext_state = AbilityEditorExtendedState::new();
    ext_state.hotbar.assign(0, 1001, Some("Q".to_string()));
    let can_use = ext_state.hotbar.can_use(0);
    let _ = can_use;

    // Tag inference
    if let Some(ability) = db.abilities.get(&1001) {
        let tags = infer_tags_from_ability(ability);
        let _has_fire = tags.contains(&AbilityTag::Fire);
        let description = ability_to_description_text(ability);
        let _ = description;
    }

    // DOT tracker
    let dot = DotInstance::new(0, 1001, 100, 200, DamageElement::Fire, 25.0, 1.0, 4.0);
    ext_state.dot_tracker.apply(dot);
    let _total_dps = ext_state.dot_tracker.total_dot_dps_on_target(200);

    // Ability search
    let results = ability_fuzzy_search(&db, "fire", 5);
    let _ = results;

    // Parabolic arc preview
    let arc_params = ParabolicArcParams { launch_angle_deg: 45.0, gravity: 9.81, initial_speed: 20.0 };
    ext_state.preview_parabolic_arc(Vec3::new(0.0, 0.0, 0.0), Vec3::new(15.0, 0.0, 0.0), &arc_params);

    // LOS check
    let _los = ext_state.check_los(Vec2::new(0.0, 0.0), Vec2::new(15.0, 0.0), 1.8, 1.8);

    // Combo setup
    let combo = ComboDefinition {
        id: 1,
        name: "Flame Burst Combo".to_string(),
        steps: vec![
            ComboStep { trigger: ComboTrigger::AbilityUsed(1001), time_window: 3.0, required_index: 0 },
            ComboStep { trigger: ComboTrigger::StatusApplied(StatusEffectType::Burn), time_window: 2.0, required_index: 1 },
        ],
        reward_ability_id: Some(1002),
        reward_modifiers: vec![("CritChance".to_string(), 0.25)],
        reward_duration: 5.0,
    };
    ext_state.combo_manager.add_combo(combo);
    let completed = ext_state.combo_manager.process_trigger(&ComboTrigger::AbilityUsed(1001), 0.0);
    let _ = completed;

    // Area denial
    let _zone_id = ext_state.area_denial_manager.spawn_zone(1001, 100, Vec3::new(10.0, 0.0, 10.0), 5.0, 10.0, 30.0, 1.0);
    let zones_at = ext_state.area_denial_manager.query_zones_at(Vec3::new(10.0, 0.0, 10.0));
    let _ = zones_at;

    // Runtime mod
    let mod_entry = RuntimeAbilityModifier {
        id: 0,
        target_ability_id: Some(1001),
        target_tag_filter: None,
        stat_changes: vec![("base_damage".to_string(), 50.0)],
        mult_changes: vec![("damage_mult".to_string(), 1.2)],
        duration: Some(10.0),
        source: "talent_bonus".to_string(),
    };
    ext_state.runtime_mods.push(mod_entry, 0.0);

    // Sorted by cooldown
    let sorted_cds = abilities_sorted_by_cooldown(&db);
    let _ = sorted_cds;

    // Filter by type
    let projectiles = filter_abilities_by_type(&db, &AbilityType::Projectile);
    let _ = projectiles;
}

// ============================================================
// SECTION: ABILITY SCALING PREVIEW TABLE
// ============================================================

#[derive(Debug, Clone)]
pub struct ScalingPreviewRow {
    pub stat_value: f32,
    pub min_damage: f32,
    pub max_damage: f32,
    pub average_damage: f32,
    pub crit_average: f32,
    pub dps: f32,
}

pub fn build_scaling_preview_table(ability: &AbilityDefinition, stat_name: &str, min_stat: f32, max_stat: f32, steps: usize) -> Vec<ScalingPreviewRow> {
    let mut rows = Vec::new();
    if steps == 0 { return rows; }
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let stat_val = min_stat + (max_stat - min_stat) * t;
        let mut stats: HashMap<String, f32> = HashMap::new();
        stats.insert(stat_name.to_string(), stat_val);

        let mut total_min = 0.0f32;
        let mut total_max = 0.0f32;
        let mut total_crit_avg = 0.0f32;

        for formula in &ability.damage_formulas {
            let scaling_bonus: f32 = formula.scaling.iter().map(|sc| {
                let base = stats.get(&sc.stat_name).cloned().unwrap_or(0.0);
                base * sc.coefficient
            }).sum();
            let base_min = formula.base_min + scaling_bonus;
            let base_max = formula.base_max + scaling_bonus;
            let avg = (base_min + base_max) * 0.5;
            let crit_avg = avg * (1.0 + formula.crit_chance_base * (formula.crit_multiplier_base - 1.0));
            total_min += base_min;
            total_max += base_max;
            total_crit_avg += crit_avg;
        }

        let avg = (total_min + total_max) * 0.5;
        let dps = if ability.base_cooldown > 0.0 { avg / ability.base_cooldown } else { avg };
        rows.push(ScalingPreviewRow { stat_value: stat_val, min_damage: total_min, max_damage: total_max, average_damage: avg, crit_average: total_crit_avg, dps });
    }
    rows
}

pub fn find_breakeven_stat_value(ability: &AbilityDefinition, stat_name: &str, target_dps: f32, max_search: f32) -> Option<f32> {
    // Binary search for stat value that gives target DPS
    let mut lo = 0.0f32;
    let mut hi = max_search;
    for _ in 0..50 {
        let mid = (lo + hi) * 0.5;
        let mut stats: HashMap<String, f32> = HashMap::new();
        stats.insert(stat_name.to_string(), mid);
        let avg: f32 = ability.damage_formulas.iter().map(|f| {
            let bonus: f32 = f.scaling.iter().map(|s| stats.get(&s.stat_name).cloned().unwrap_or(0.0) * s.coefficient).sum();
            (f.base_min + f.base_max) * 0.5 + bonus
        }).sum();
        let dps = if ability.base_cooldown > 0.0 { avg / ability.base_cooldown } else { avg };
        if (dps - target_dps).abs() < 0.1 { return Some(mid); }
        if dps < target_dps { lo = mid; } else { hi = mid; }
    }
    None
}

// ============================================================
// SECTION: ABILITY RESONANCE SYSTEM (ELEMENTAL COMBOS)
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ResonanceType { Ignition, Vaporize, Melt, Overload, Superconduct, Crystallize, Swirl, Shatter, Electrified }

#[derive(Debug, Clone)]
pub struct ResonanceResult {
    pub resonance_type: ResonanceType,
    pub damage_multiplier: f32,
    pub bonus_effect: Option<StatusEffectType>,
    pub clears_aura_a: bool,
    pub clears_aura_b: bool,
}

pub fn compute_resonance(element_a: &DamageElement, element_b: &DamageElement) -> Option<ResonanceResult> {
    match (element_a, element_b) {
        (DamageElement::Fire, DamageElement::Poison) | (DamageElement::Poison, DamageElement::Fire) => Some(ResonanceResult {
            resonance_type: ResonanceType::Vaporize,
            damage_multiplier: 1.5,
            bonus_effect: None,
            clears_aura_a: true,
            clears_aura_b: true,
        }),
        (DamageElement::Fire, DamageElement::Cold) | (DamageElement::Cold, DamageElement::Fire) => Some(ResonanceResult {
            resonance_type: ResonanceType::Melt,
            damage_multiplier: 2.0,
            bonus_effect: None,
            clears_aura_a: true,
            clears_aura_b: true,
        }),
        (DamageElement::Lightning, DamageElement::Poison) | (DamageElement::Poison, DamageElement::Lightning) => Some(ResonanceResult {
            resonance_type: ResonanceType::Electrified,
            damage_multiplier: 1.0,
            bonus_effect: Some(StatusEffectType::Stun),
            clears_aura_a: false,
            clears_aura_b: false,
        }),
        (DamageElement::Fire, DamageElement::Lightning) | (DamageElement::Lightning, DamageElement::Fire) => Some(ResonanceResult {
            resonance_type: ResonanceType::Overload,
            damage_multiplier: 1.6,
            bonus_effect: Some(StatusEffectType::Knockback),
            clears_aura_a: true,
            clears_aura_b: true,
        }),
        (DamageElement::Cold, DamageElement::Poison) | (DamageElement::Poison, DamageElement::Cold) => Some(ResonanceResult {
            resonance_type: ResonanceType::Shatter,
            damage_multiplier: 1.8,
            bonus_effect: Some(StatusEffectType::Freeze),
            clears_aura_a: true,
            clears_aura_b: true,
        }),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct ElementalAura {
    pub entity_id: u64,
    pub element: DamageElement,
    pub strength: f32,
    pub applied_at: f32,
}

#[derive(Debug)]
pub struct ResonanceTracker {
    pub auras: HashMap<u64, ElementalAura>,
}

impl ResonanceTracker {
    pub fn new() -> Self { Self { auras: HashMap::new() } }

    pub fn apply_element(&mut self, entity_id: u64, element: DamageElement, strength: f32, current_time: f32) -> Option<ResonanceResult> {
        if let Some(existing) = self.auras.get(&entity_id) {
            if let Some(mut resonance) = compute_resonance(&existing.element, &element) {
                if resonance.clears_aura_a { self.auras.remove(&entity_id); }
                if !resonance.clears_aura_b {
                    self.auras.insert(entity_id, ElementalAura { entity_id, element, strength, applied_at: current_time });
                }
                resonance.damage_multiplier *= strength;
                return Some(resonance);
            }
        }
        self.auras.insert(entity_id, ElementalAura { entity_id, element, strength, applied_at: current_time });
        None
    }

    pub fn decay_auras(&mut self, current_time: f32, decay_time: f32) {
        self.auras.retain(|_, a| current_time - a.applied_at < decay_time);
    }

    pub fn aura_on_entity(&self, entity_id: u64) -> Option<&ElementalAura> {
        self.auras.get(&entity_id)
    }
}

// ============================================================
// SECTION: ABILITY COST EFFICIENCY ANALYZER (EXTENDED)
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityEfficiencyProfile {
    pub ability_id: u64,
    pub name: String,
    pub raw_dps: f32,
    pub resource_cost_per_second: f32,
    pub damage_per_resource: f32,
    pub effective_cast_time_with_cd: f32,
    pub uptime_fraction: f32,
    pub weighted_score: f32,
}

pub fn build_efficiency_profiles(database: &AbilityDatabase, stats: &HashMap<String, f32>, fight_duration: f32) -> Vec<AbilityEfficiencyProfile> {
    let mut profiles = Vec::new();
    for (&id, ability) in &database.abilities {
        let avg_damage: f32 = ability.damage_formulas.iter().map(|f| {
            let bonus: f32 = f.scaling.iter().map(|s| stats.get(&s.stat_name).cloned().unwrap_or(0.0) * s.coefficient).sum();
            let avg_base = (f.base_min + f.base_max) * 0.5 + bonus;
            avg_base * (1.0 + f.crit_chance_base * (f.crit_multiplier_base - 1.0))
        }).sum();

        let total_cycle_time = ability.cast_time + ability.base_cooldown;
        let raw_dps = if total_cycle_time > 0.0 { avg_damage / total_cycle_time } else { avg_damage };
        let uses_in_fight = if total_cycle_time > 0.0 { (fight_duration / total_cycle_time).floor() } else { 1.0 };
        let uptime = if fight_duration > 0.0 { (uses_in_fight * ability.cast_time) / fight_duration } else { 0.0 };

        let resource_per_use = ability.resource_cost;
        let resource_per_second = if total_cycle_time > 0.0 { resource_per_use / total_cycle_time } else { 0.0 };
        let damage_per_resource = if resource_per_use > 0.0 { avg_damage / resource_per_use } else { f32::INFINITY };

        let weighted_score = raw_dps * 0.5 + damage_per_resource.min(10.0) * 0.3 + uptime * 100.0 * 0.2;

        profiles.push(AbilityEfficiencyProfile {
            ability_id: id,
            name: ability.name.clone(),
            raw_dps,
            resource_cost_per_second: resource_per_second,
            damage_per_resource,
            effective_cast_time_with_cd: total_cycle_time,
            uptime_fraction: uptime,
            weighted_score,
        });
    }
    profiles.sort_by(|a, b| b.weighted_score.partial_cmp(&a.weighted_score).unwrap_or(std::cmp::Ordering::Equal));
    profiles
}

// ============================================================
// SECTION: ABILITY PHYSICS (BOUNCE AND RICCOCHET)
// ============================================================

#[derive(Debug, Clone)]
pub struct BounceParams {
    pub max_bounces: u32,
    pub restitution: f32,
    pub damage_falloff_per_bounce: f32,
    pub angle_deviation_deg: f32,
}

#[derive(Debug, Clone)]
pub struct BounceState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub bounces_remaining: u32,
    pub current_damage_mult: f32,
}

pub fn simulate_bounce(params: &BounceParams, initial_pos: Vec3, initial_vel: Vec3, gravity: f32, dt: f32, max_steps: usize, floor_y: f32) -> Vec<Vec3> {
    let mut positions = Vec::new();
    let mut state = BounceState {
        position: initial_pos,
        velocity: initial_vel,
        bounces_remaining: params.max_bounces,
        current_damage_mult: 1.0,
    };
    positions.push(state.position);

    for _ in 0..max_steps {
        state.velocity.y -= gravity * dt;
        state.position += state.velocity * dt;
        positions.push(state.position);

        if state.position.y <= floor_y && state.velocity.y < 0.0 {
            if state.bounces_remaining == 0 { break; }
            state.velocity.y = -state.velocity.y * params.restitution;
            state.velocity.x *= params.restitution;
            state.velocity.z *= params.restitution;
            state.position.y = floor_y;
            state.bounces_remaining -= 1;
            state.current_damage_mult *= 1.0 - params.damage_falloff_per_bounce;
        }

        if state.velocity.length() < 0.01 { break; }
    }
    positions
}

pub fn reflect_velocity_off_normal(velocity: Vec3, surface_normal: Vec3, restitution: f32) -> Vec3 {
    let dot = velocity.dot(surface_normal);
    let reflected = velocity - surface_normal * (2.0 * dot);
    reflected * restitution
}

// ============================================================
// SECTION: ABILITY CHAIN LIGHTNING SIMULATION
// ============================================================

#[derive(Debug, Clone)]
pub struct ChainLightningParams {
    pub max_jumps: u32,
    pub jump_range: f32,
    pub damage_falloff: f32,
    pub fork_probability: f32,
    pub max_forks: u32,
    pub cannot_rehit_duration: f32,
}

#[derive(Debug, Clone)]
pub struct LightningJump {
    pub from_entity: u64,
    pub to_entity: u64,
    pub jump_index: u32,
    pub damage_multiplier: f32,
    pub is_fork: bool,
}

pub fn simulate_chain_lightning(
    params: &ChainLightningParams,
    initial_target: u64,
    entity_positions: &HashMap<u64, Vec3>,
    seed: &mut u64,
) -> Vec<LightningJump> {
    let lcg = |s: &mut u64| -> f32 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 33) as f32) / (u32::MAX as f32)
    };

    let mut jumps = Vec::new();
    let mut hit_set: HashSet<u64> = HashSet::new();
    hit_set.insert(initial_target);

    let mut queue: VecDeque<(u64, u32, f32)> = VecDeque::new();
    queue.push_back((initial_target, 0, 1.0));

    while let Some((from_id, jump_index, damage_mult)) = queue.pop_front() {
        if jump_index >= params.max_jumps { continue; }

        let from_pos = match entity_positions.get(&from_id) { Some(p) => *p, None => continue };

        let mut candidates: Vec<(u64, f32)> = entity_positions.iter()
            .filter(|(&id, _)| !hit_set.contains(&id))
            .map(|(&id, &pos)| {
                let dist = (pos - from_pos).length();
                (id, dist)
            })
            .filter(|(_, dist)| *dist <= params.jump_range)
            .collect();

        if candidates.is_empty() { continue; }
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let next_id = candidates[0].0;
        let new_mult = damage_mult * params.damage_falloff;
        hit_set.insert(next_id);

        jumps.push(LightningJump {
            from_entity: from_id,
            to_entity: next_id,
            jump_index,
            damage_multiplier: new_mult,
            is_fork: false,
        });

        queue.push_back((next_id, jump_index + 1, new_mult));

        // Forks
        let fork_count = (params.max_forks as usize).min(candidates.len() - 1);
        let mut forks_created = 0u32;
        for (fork_id, _) in candidates.iter().skip(1) {
            if forks_created >= params.max_forks { break; }
            if lcg(seed) < params.fork_probability {
                hit_set.insert(*fork_id);
                jumps.push(LightningJump {
                    from_entity: from_id,
                    to_entity: *fork_id,
                    jump_index,
                    damage_multiplier: new_mult * 0.5,
                    is_fork: true,
                });
                forks_created += 1;
            }
        }
        let _ = fork_count;
    }
    jumps
}

// ============================================================
// SECTION: SPELL BOOK UI STATE
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SpellBookTab { AllAbilities, Favorites, BySchool, ByType, RecentlyUsed }

#[derive(Debug)]
pub struct SpellBookState {
    pub active_tab: SpellBookTab,
    pub search_query: String,
    pub selected_ability_id: Option<u64>,
    pub school_filter: Option<String>,
    pub type_filter: Option<AbilityType>,
    pub favorites: HashSet<u64>,
    pub recently_used: VecDeque<u64>,
    pub max_recent: usize,
    pub scroll_offset: usize,
    pub page_size: usize,
}

impl SpellBookState {
    pub fn new() -> Self {
        Self {
            active_tab: SpellBookTab::AllAbilities,
            search_query: String::new(),
            selected_ability_id: None,
            school_filter: None,
            type_filter: None,
            favorites: HashSet::new(),
            recently_used: VecDeque::new(),
            max_recent: 20,
            scroll_offset: 0,
            page_size: 12,
        }
    }

    pub fn toggle_favorite(&mut self, ability_id: u64) {
        if self.favorites.contains(&ability_id) {
            self.favorites.remove(&ability_id);
        } else {
            self.favorites.insert(ability_id);
        }
    }

    pub fn record_use(&mut self, ability_id: u64) {
        self.recently_used.retain(|&id| id != ability_id);
        self.recently_used.push_front(ability_id);
        if self.recently_used.len() > self.max_recent {
            self.recently_used.pop_back();
        }
    }

    pub fn filtered_abilities<'a>(&self, database: &'a AbilityDatabase) -> Vec<(u64, &'a AbilityDefinition)> {
        let query = self.search_query.to_lowercase();
        let mut result: Vec<(u64, &'a AbilityDefinition)> = database.abilities.iter()
            .filter(|(&_id, ab)| {
                if !query.is_empty() && !ab.name.to_lowercase().contains(&query) { return false; }
                if let Some(school) = &self.school_filter {
                    if &ab.school != school { return false; }
                }
                if let Some(ab_type) = &self.type_filter {
                    if std::mem::discriminant(&ab.ability_type) != std::mem::discriminant(ab_type) { return false; }
                }
                match self.active_tab {
                    SpellBookTab::Favorites => self.favorites.contains(&_id),
                    SpellBookTab::RecentlyUsed => self.recently_used.contains(&_id),
                    _ => true,
                }
            })
            .map(|(&id, ab)| (id, ab))
            .collect();
        result.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        result
    }

    pub fn current_page<'a, 'b>(&'b self, filtered: &'a [( u64, &'a AbilityDefinition)]) -> &'a [(u64, &'a AbilityDefinition)] {
        let start = self.scroll_offset;
        let end = (start + self.page_size).min(filtered.len());
        if start >= filtered.len() { return &[]; }
        &filtered[start..end]
    }

    pub fn page_count(&self, total: usize) -> usize {
        if self.page_size == 0 { return 1; }
        (total + self.page_size - 1) / self.page_size
    }

    pub fn all_schools(&self, database: &AbilityDatabase) -> Vec<String> {
        let mut schools: HashSet<String> = database.abilities.values().map(|ab| ab.school.clone()).collect();
        let mut sorted: Vec<String> = schools.drain().collect();
        sorted.sort();
        sorted
    }
}

// ============================================================
// SECTION: ABILITY IMPORT/EXPORT (JSON-LIKE SERIALIZATION)
// ============================================================

pub fn export_ability_database_to_string(database: &AbilityDatabase) -> String {
    let mut lines = Vec::new();
    lines.push("# AbilityDatabase Export".to_string());
    lines.push(format!("ability_count={}", database.abilities.len()));
    lines.push(String::new());
    for (id, ability) in &database.abilities {
        lines.push(format!("[ability:{}]", id));
        lines.push(format!("name={}", ability.name));
        lines.push(format!("type={:?}", ability.ability_type));
        lines.push(format!("school={}", ability.school));
        lines.push(format!("level_req={}", ability.unlock_level));
        lines.push(format!("cast_time={:.3}", ability.cast_time));
        lines.push(format!("cooldown={:.3}", ability.base_cooldown));
        lines.push(format!("range={:.1}", ability.range));
        lines.push(format!("area_radius={:.1}", ability.aoe_radius));
        if ability.resource_cost > 0.0 {
            lines.push(format!("resource_type={:?}", ability.resource_type));
            lines.push(format!("resource_cost={:.1}", ability.resource_cost));
        }
        lines.push(format!("damage_formula_count={}", ability.damage_formulas.len()));
        for (i, f) in ability.damage_formulas.iter().enumerate() {
            lines.push(format!("formula[{}].element={:?}", i, f.element));
            lines.push(format!("formula[{}].base={:.0}-{:.0}", i, f.base_min, f.base_max));
            lines.push(format!("formula[{}].crit={:.3}", i, f.crit_chance_base));
        }
        lines.push(format!("effect_count={}", ability.applied_effects.len()));
        for (i, eff) in ability.applied_effects.iter().enumerate() {
            lines.push(format!("effect[{}].type={:?}", i, eff.effect_type));
            lines.push(format!("effect[{}].chance={:.3}", i, eff.apply_chance));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

pub fn validate_ability_export_string(data: &str) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut has_header = false;
    let mut ability_count_declared = 0usize;
    let mut ability_blocks_found = 0usize;

    for line in data.lines() {
        if line.starts_with("# AbilityDatabase") { has_header = true; }
        if line.starts_with("ability_count=") {
            if let Ok(n) = line["ability_count=".len()..].parse::<usize>() {
                ability_count_declared = n;
            }
        }
        if line.starts_with("[ability:") { ability_blocks_found += 1; }
    }
    if !has_header { errors.push("Missing header line".to_string()); }
    if ability_count_declared != ability_blocks_found {
        errors.push(format!("Declared {} abilities but found {}", ability_count_declared, ability_blocks_found));
    }
    (errors.is_empty(), errors)
}

// ============================================================
// SECTION: STATUS EFFECT DURATION SCALING
// ============================================================

pub fn scale_status_duration(base_duration: f32, caster_stat_bonus: f32, target_tenacity: f32, diminishing_returns_stacks: u32) -> f32 {
    let bonus_mult = 1.0 + (caster_stat_bonus / 100.0).min(1.0);
    let tenacity_reduction = (target_tenacity / 100.0).clamp(0.0, 0.75);
    let dr_factor = match diminishing_returns_stacks {
        0 => 1.0,
        1 => 0.65,
        2 => 0.42,
        3 => 0.27,
        _ => 0.15,
    };
    base_duration * bonus_mult * (1.0 - tenacity_reduction) * dr_factor
}

pub fn compute_status_magnitude(base_magnitude: f32, spellpower: f32, level_scaling: f32, item_level: u32) -> f32 {
    let sp_bonus = spellpower * level_scaling;
    let level_bonus = (item_level as f32 * 0.5).min(50.0);
    base_magnitude + sp_bonus + level_bonus
}

pub fn status_tick_damage(status: &StatusEffectDefinition, caster_spellpower: f32, tick_index: u32) -> f32 {
    let base = (status.tick_damage.base_min + status.tick_damage.base_max) * 0.5;
    let sp_mult = 1.0 + caster_spellpower * status.tick_damage.scaling.first().map(|s| s.coefficient).unwrap_or(0.0);
    // Some effects ramp up over time (e.g., Bleed)
    let ramp = match status.effect_type {
        StatusEffectType::Bleed => 1.0 + (tick_index as f32 * 0.05).min(0.5),
        StatusEffectType::Poison => 1.0,
        StatusEffectType::Burn => 1.0 - (tick_index as f32 * 0.02).min(0.3), // decays
        _ => 1.0,
    };
    base * sp_mult * ramp
}

// ============================================================
// SECTION: ABILITY RANGE DISPLAY HELPERS
// ============================================================

#[derive(Debug, Clone)]
pub struct RangeIndicator {
    pub center: Vec3,
    pub indicator_type: RangeIndicatorType,
    pub color: Vec4,
    pub opacity: f32,
}

#[derive(Debug, Clone)]
pub enum RangeIndicatorType {
    Circle { radius: f32 },
    Cone { angle_deg: f32, length: f32 },
    Rectangle { width: f32, length: f32 },
    Ring { inner_radius: f32, outer_radius: f32 },
    Line { direction: Vec3, length: f32, width: f32 },
}

impl RangeIndicatorType {
    pub fn bounding_radius(&self) -> f32 {
        match self {
            Self::Circle { radius } => *radius,
            Self::Cone { length, .. } => *length,
            Self::Rectangle { width, length } => (width * width + length * length).sqrt() * 0.5,
            Self::Ring { outer_radius, .. } => *outer_radius,
            Self::Line { length, width, .. } => (length * length + width * width).sqrt() * 0.5,
        }
    }

    pub fn area(&self) -> f32 {
        match self {
            Self::Circle { radius } => std::f32::consts::PI * radius * radius,
            Self::Cone { angle_deg, length } => {
                let angle_rad = angle_deg.to_radians();
                0.5 * angle_rad * length * length
            }
            Self::Rectangle { width, length } => width * length,
            Self::Ring { inner_radius, outer_radius } => {
                std::f32::consts::PI * (outer_radius * outer_radius - inner_radius * inner_radius)
            }
            Self::Line { width, length, .. } => width * length,
        }
    }
}

pub fn build_range_indicators_for_ability(ability: &AbilityDefinition, caster_pos: Vec3, caster_facing: Vec3) -> Vec<RangeIndicator> {
    let mut indicators = Vec::new();
    let cast_color = Vec4::new(0.2, 0.6, 1.0, 0.4);
    let area_color = Vec4::new(1.0, 0.3, 0.1, 0.35);

    // Cast range indicator
    indicators.push(RangeIndicator {
        center: caster_pos,
        indicator_type: RangeIndicatorType::Circle { radius: ability.range },
        color: cast_color,
        opacity: 0.4,
    });

    // Area indicator based on targeting mode
    match &ability.targeting_mode {
        TargetingMode::AoECircle { .. } => {
            indicators.push(RangeIndicator {
                center: caster_pos + caster_facing * (ability.range * 0.5),
                indicator_type: RangeIndicatorType::Circle { radius: ability.aoe_radius },
                color: area_color,
                opacity: 0.5,
            });
        }
        TargetingMode::Cone { .. } => {
            indicators.push(RangeIndicator {
                center: caster_pos,
                indicator_type: RangeIndicatorType::Cone { angle_deg: 60.0, length: ability.range },
                color: area_color,
                opacity: 0.5,
            });
        }
        TargetingMode::Rectangle { .. } => {
            indicators.push(RangeIndicator {
                center: caster_pos + caster_facing * (ability.range * 0.5),
                indicator_type: RangeIndicatorType::Rectangle { width: ability.aoe_radius * 2.0, length: ability.range },
                color: area_color,
                opacity: 0.5,
            });
        }
        _ => {}
    }
    indicators
}

// ============================================================
// SECTION: TALENT NODE UPGRADE PATHS
// ============================================================

#[derive(Debug, Clone)]
pub struct TalentUpgradePath {
    pub from_node: u64,
    pub to_node: u64,
    pub upgrade_label: String,
    pub replaced_modifier: Option<(String, f32)>,
    pub new_modifier: (String, f32),
    pub cost_additional_points: u32,
}

pub fn find_upgrade_paths_for_node(tree: &TalentTree, node_id: u64) -> Vec<TalentUpgradePath> {
    tree.edges.iter()
        .filter(|e| e.from_node_id == node_id)
        .map(|e| TalentUpgradePath {
            from_node: e.from_node_id,
            to_node: e.to_node_id,
            upgrade_label: format!("Upgrade to node {}", e.to_node_id),
            replaced_modifier: None,
            new_modifier: ("damage_percent".to_string(), 5.0),
            cost_additional_points: 1,
        })
        .collect()
}

pub fn compute_talent_path_dps_gain(path: &TalentUpgradePath, current_dps: f32) -> f32 {
    let gain_pct = path.new_modifier.1 / 100.0;
    let replaced_pct = path.replaced_modifier.as_ref().map(|(_, v)| v / 100.0).unwrap_or(0.0);
    current_dps * (1.0 + gain_pct - replaced_pct) - current_dps
}

// ============================================================
// SECTION: ABILITY ANIMATION BINDINGS
// ============================================================

#[derive(Debug, Clone)]
pub struct AnimationBinding {
    pub ability_id: u64,
    pub cast_anim: String,
    pub cast_anim_speed: f32,
    pub channel_anim: String,
    pub channel_loop: bool,
    pub hit_anim: String,
    pub miss_anim: Option<String>,
    pub cast_point: f32,
    pub backswing_point: f32,
}

impl AnimationBinding {
    pub fn new(ability_id: u64, cast_anim: &str) -> Self {
        Self {
            ability_id,
            cast_anim: cast_anim.to_string(),
            cast_anim_speed: 1.0,
            channel_anim: format!("{}_channel", cast_anim),
            channel_loop: true,
            hit_anim: format!("{}_hit", cast_anim),
            miss_anim: None,
            cast_point: 0.5,
            backswing_point: 0.8,
        }
    }

    pub fn normalized_cast_point(&self, total_cast_time: f32) -> f32 {
        self.cast_point * total_cast_time
    }
}

#[derive(Debug)]
pub struct AnimationBindingRegistry {
    pub bindings: HashMap<u64, AnimationBinding>,
}

impl AnimationBindingRegistry {
    pub fn new() -> Self { Self { bindings: HashMap::new() } }
    pub fn register(&mut self, binding: AnimationBinding) { self.bindings.insert(binding.ability_id, binding); }
    pub fn get(&self, ability_id: u64) -> Option<&AnimationBinding> { self.bindings.get(&ability_id) }
    pub fn get_cast_anim(&self, ability_id: u64) -> &str {
        self.bindings.get(&ability_id).map(|b| b.cast_anim.as_str()).unwrap_or("generic_cast")
    }
}

// ============================================================
// SECTION: ABILITY SOUND BINDINGS
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilitySoundSet {
    pub ability_id: u64,
    pub cast_start_sound: Option<String>,
    pub cast_end_sound: Option<String>,
    pub projectile_loop_sound: Option<String>,
    pub impact_sounds: Vec<String>,
    pub channel_loop_sound: Option<String>,
    pub volume_cast: f32,
    pub volume_impact: f32,
    pub pitch_variance: f32,
}

impl AbilitySoundSet {
    pub fn new(ability_id: u64) -> Self {
        Self {
            ability_id,
            cast_start_sound: None,
            cast_end_sound: None,
            projectile_loop_sound: None,
            impact_sounds: Vec::new(),
            channel_loop_sound: None,
            volume_cast: 1.0,
            volume_impact: 1.0,
            pitch_variance: 0.05,
        }
    }

    pub fn pick_impact_sound(&self, seed: u64) -> Option<&str> {
        if self.impact_sounds.is_empty() { return None; }
        let idx = (seed as usize) % self.impact_sounds.len();
        Some(&self.impact_sounds[idx])
    }

    pub fn randomized_pitch(&self, base_pitch: f32, seed: u64) -> f32 {
        let t = ((seed % 10000) as f32) / 10000.0;
        base_pitch + (t - 0.5) * 2.0 * self.pitch_variance
    }
}

// ============================================================
// SECTION: ABILITY STATISTICS TRACKER (PER-SESSION)
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityUseRecord {
    pub ability_id: u64,
    pub timestamp: f32,
    pub targets_hit: u32,
    pub total_damage: f32,
    pub was_crit: bool,
    pub effects_applied: Vec<StatusEffectType>,
    pub interrupted: bool,
}

#[derive(Debug)]
pub struct AbilitySessionStats {
    pub ability_id: u64,
    pub uses: u32,
    pub total_damage: f32,
    pub crits: u32,
    pub interrupts: u32,
    pub total_targets_hit: u32,
    pub effects_applied_counts: HashMap<String, u32>,
    pub last_use_timestamp: f32,
    pub first_use_timestamp: Option<f32>,
}

impl AbilitySessionStats {
    pub fn new(ability_id: u64) -> Self {
        Self { ability_id, uses: 0, total_damage: 0.0, crits: 0, interrupts: 0, total_targets_hit: 0, effects_applied_counts: HashMap::new(), last_use_timestamp: 0.0, first_use_timestamp: None }
    }

    pub fn record(&mut self, record: &AbilityUseRecord) {
        self.uses += 1;
        self.total_damage += record.total_damage;
        if record.was_crit { self.crits += 1; }
        if record.interrupted { self.interrupts += 1; }
        self.total_targets_hit += record.targets_hit;
        for eff in &record.effects_applied {
            *self.effects_applied_counts.entry(format!("{:?}", eff)).or_insert(0) += 1;
        }
        self.last_use_timestamp = record.timestamp;
        if self.first_use_timestamp.is_none() { self.first_use_timestamp = Some(record.timestamp); }
    }

    pub fn average_damage_per_use(&self) -> f32 {
        if self.uses == 0 { return 0.0; }
        self.total_damage / self.uses as f32
    }

    pub fn crit_rate(&self) -> f32 {
        if self.uses == 0 { return 0.0; }
        self.crits as f32 / self.uses as f32
    }

    pub fn average_targets_hit(&self) -> f32 {
        if self.uses == 0 { return 0.0; }
        self.total_targets_hit as f32 / self.uses as f32
    }

    pub fn dps_over_session(&self) -> f32 {
        if let Some(first) = self.first_use_timestamp {
            let duration = self.last_use_timestamp - first;
            if duration > 0.0 { return self.total_damage / duration; }
        }
        0.0
    }
}

#[derive(Debug)]
pub struct SessionStatsTracker {
    pub stats: HashMap<u64, AbilitySessionStats>,
}

impl SessionStatsTracker {
    pub fn new() -> Self { Self { stats: HashMap::new() } }

    pub fn record_use(&mut self, record: AbilityUseRecord) {
        self.stats.entry(record.ability_id).or_insert_with(|| AbilitySessionStats::new(record.ability_id)).record(&record);
    }

    pub fn top_n_by_damage(&self, n: usize) -> Vec<(u64, f32)> {
        let mut list: Vec<(u64, f32)> = self.stats.iter().map(|(&id, s)| (id, s.total_damage)).collect();
        list.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        list.truncate(n);
        list
    }

    pub fn most_interrupted(&self) -> Option<u64> {
        self.stats.iter().max_by_key(|(_, s)| s.interrupts).map(|(&id, _)| id)
    }

    pub fn global_crit_rate(&self) -> f32 {
        let total_uses: u32 = self.stats.values().map(|s| s.uses).sum();
        let total_crits: u32 = self.stats.values().map(|s| s.crits).sum();
        if total_uses == 0 { return 0.0; }
        total_crits as f32 / total_uses as f32
    }

    pub fn reset(&mut self) { self.stats.clear(); }
}

// ============================================================
// SECTION: ABILITY EDITOR FINAL WIRING
// ============================================================

pub fn ability_editor_full_init() -> AbilityEditor {
    AbilityEditor::new()
}

pub fn ability_editor_session_demo() {
    let mut tracker = SessionStatsTracker::new();
    tracker.record_use(AbilityUseRecord {
        ability_id: 1001,
        timestamp: 1.5,
        targets_hit: 3,
        total_damage: 450.0,
        was_crit: true,
        effects_applied: vec![StatusEffectType::Burn],
        interrupted: false,
    });
    tracker.record_use(AbilityUseRecord {
        ability_id: 1001,
        timestamp: 8.5,
        targets_hit: 2,
        total_damage: 310.0,
        was_crit: false,
        effects_applied: Vec::new(),
        interrupted: false,
    });
    let top = tracker.top_n_by_damage(5);
    let global_crit = tracker.global_crit_rate();
    let _ = (top, global_crit);

    let resonance_tracker = ResonanceTracker::new();
    let _ = resonance_tracker;

    let mut anim_reg = AnimationBindingRegistry::new();
    anim_reg.register(AnimationBinding::new(1001, "fireball_cast"));
    let cast_anim = anim_reg.get_cast_anim(1001);
    let _ = cast_anim;

    let mut sound_set = AbilitySoundSet::new(1001);
    sound_set.cast_start_sound = Some("fire_cast_start.ogg".to_string());
    sound_set.impact_sounds = vec!["fire_impact_1.ogg".to_string(), "fire_impact_2.ogg".to_string()];
    let impact = sound_set.pick_impact_sound(42);
    let _ = impact;
}

// ============================================================
// SECTION: ABILITY POWER BUDGET VALIDATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct AbilityPowerBudget {
    pub target_dps: f32,
    pub target_utility_score: f32,
    pub target_mobility_score: f32,
    pub tolerance_pct: f32,
}

impl AbilityPowerBudget {
    pub fn for_level(level: u32, role: &str) -> Self {
        let base_dps = level as f32 * 12.5;
        let (utility, mobility) = match role {
            "tank" => (0.8, 0.3),
            "healer" => (0.9, 0.4),
            "support" => (0.7, 0.5),
            "assassin" => (0.3, 0.9),
            _ => (0.5, 0.5), // dps
        };
        Self { target_dps: base_dps, target_utility_score: utility, target_mobility_score: mobility, tolerance_pct: 25.0 }
    }
}

#[derive(Debug, Clone)]
pub struct AbilityPowerBudgetReport {
    pub ability_id: u64,
    pub computed_dps: f32,
    pub computed_utility: f32,
    pub computed_mobility: f32,
    pub dps_within_budget: bool,
    pub overall_passed: bool,
    pub suggestions: Vec<String>,
}

pub fn validate_ability_power_budget(ability: &AbilityDefinition, stats: &HashMap<String, f32>, budget: &AbilityPowerBudget) -> AbilityPowerBudgetReport {
    let avg_damage: f32 = ability.damage_formulas.iter().map(|f| {
        let bonus: f32 = f.scaling.iter().map(|s| stats.get(&s.stat_name).cloned().unwrap_or(0.0) * s.coefficient).sum();
        (f.base_min + f.base_max) * 0.5 + bonus
    }).sum();

    let cycle = ability.cast_time + ability.base_cooldown;
    let computed_dps = if cycle > 0.0 { avg_damage / cycle } else { avg_damage };

    let utility = ability.applied_effects.len() as f32 * 0.15
        + if ability.aoe_radius > 5.0 { 0.2 } else { 0.0 }
        + if ability.applied_effects.iter().any(|e| matches!(e.effect_type, StatusEffectType::Stun | StatusEffectType::Root | StatusEffectType::Silence)) { 0.3 } else { 0.0 };

    let mobility = match ability.ability_type {
        AbilityType::Dash | AbilityType::Teleport | AbilityType::Charge => 1.0,
        _ => 0.0,
    };

    let tol = budget.tolerance_pct / 100.0;
    let dps_ok = computed_dps <= budget.target_dps * (1.0 + tol);
    let overall = dps_ok;

    let mut suggestions = Vec::new();
    if computed_dps > budget.target_dps * (1.0 + tol) {
        suggestions.push(format!("DPS {:.1} exceeds budget {:.1} by {:.0}% — increase cooldown or reduce base damage",
            computed_dps, budget.target_dps, (computed_dps / budget.target_dps - 1.0) * 100.0));
    }
    if utility > budget.target_utility_score + 0.3 {
        suggestions.push("Utility score high — consider reducing effect duration or application chance".to_string());
    }

    AbilityPowerBudgetReport {
        ability_id: ability.id,
        computed_dps,
        computed_utility: utility.min(1.0),
        computed_mobility: mobility,
        dps_within_budget: dps_ok,
        overall_passed: overall,
        suggestions,
    }
}

// ============================================================
// SECTION: RADIUS FALLOFF DAMAGE
// ============================================================

#[derive(Debug, Clone)]
pub enum FalloffCurve { Linear, Quadratic, Cosine, Exponential(f32), None }

impl FalloffCurve {
    pub fn multiplier(&self, distance: f32, max_radius: f32) -> f32 {
        if max_radius <= 0.0 { return 1.0; }
        let t = (distance / max_radius).clamp(0.0, 1.0);
        match self {
            FalloffCurve::None => 1.0,
            FalloffCurve::Linear => 1.0 - t,
            FalloffCurve::Quadratic => 1.0 - t * t,
            FalloffCurve::Cosine => ((1.0 - t) * std::f32::consts::PI * 0.5).cos(),
            FalloffCurve::Exponential(k) => (-k * t).exp(),
        }
    }

    pub fn integrate_over_radius(&self, max_radius: f32, steps: usize) -> f32 {
        if steps == 0 || max_radius <= 0.0 { return 0.0; }
        let dr = max_radius / steps as f32;
        let mut integral = 0.0f32;
        for i in 0..steps {
            let r = (i as f32 + 0.5) * dr;
            let mult = self.multiplier(r, max_radius);
            // Weight by ring area: 2*pi*r*dr
            integral += mult * 2.0 * std::f32::consts::PI * r * dr;
        }
        integral
    }
}

pub fn compute_aoe_total_damage(base_damage: f32, targets: &[(u64, f32)], max_radius: f32, falloff: &FalloffCurve, min_damage_pct: f32) -> Vec<(u64, f32)> {
    targets.iter().map(|(id, dist)| {
        let mult = falloff.multiplier(*dist, max_radius).max(min_damage_pct / 100.0);
        (*id, base_damage * mult)
    }).collect()
}

// ============================================================
// SECTION: ABILITY EDITOR EXTENDED STATS SUMMARY
// ============================================================

pub fn summarize_ability_database(database: &AbilityDatabase) -> String {
    let total = database.abilities.len();
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut school_counts: HashMap<String, usize> = HashMap::new();
    let mut total_cooldown: f32 = 0.0;
    let mut total_cast_time: f32 = 0.0;
    let mut has_damage = 0usize;
    let mut has_effects = 0usize;

    for ab in database.abilities.values() {
        *type_counts.entry(format!("{:?}", ab.ability_type)).or_insert(0) += 1;
        *school_counts.entry(ab.school.clone()).or_insert(0) += 1;
        total_cooldown += ab.base_cooldown;
        total_cast_time += ab.cast_time;
        if !ab.damage_formulas.is_empty() { has_damage += 1; }
        if !ab.applied_effects.is_empty() { has_effects += 1; }
    }

    let avg_cd = if total > 0 { total_cooldown / total as f32 } else { 0.0 };
    let avg_ct = if total > 0 { total_cast_time / total as f32 } else { 0.0 };

    let mut lines = vec![
        format!("=== Ability Database Summary ==="),
        format!("Total Abilities: {}", total),
        format!("With Damage Formulas: {}", has_damage),
        format!("With Status Effects: {}", has_effects),
        format!("Average Cooldown: {:.2}s", avg_cd),
        format!("Average Cast Time: {:.2}s", avg_ct),
        format!("Schools: {}", school_counts.len()),
        format!("Types:"),
    ];
    let mut type_list: Vec<(String, usize)> = type_counts.into_iter().collect();
    type_list.sort_by(|a, b| b.1.cmp(&a.1));
    for (ty, count) in type_list {
        lines.push(format!("  {}: {}", ty, count));
    }
    lines.join("\n")
}

pub fn ability_database_integrity_check(database: &AbilityDatabase) -> Vec<String> {
    let mut issues = Vec::new();
    for (id, ab) in &database.abilities {
        if ab.name.is_empty() { issues.push(format!("Ability {} has empty name", id)); }
        if ab.base_cooldown < 0.0 { issues.push(format!("Ability {} '{}' has negative cooldown", id, ab.name)); }
        if ab.cast_time < 0.0 { issues.push(format!("Ability {} '{}' has negative cast time", id, ab.name)); }
        if ab.range < 0.0 { issues.push(format!("Ability {} '{}' has negative range", id, ab.name)); }
        for (i, formula) in ab.damage_formulas.iter().enumerate() {
            if formula.base_min > formula.base_max {
                issues.push(format!("Ability {} formula[{}]: base_min > base_max", id, i));
            }
            if formula.crit_chance_base < 0.0 || formula.crit_chance_base > 1.0 {
                issues.push(format!("Ability {} formula[{}]: crit_chance out of [0,1]", id, i));
            }
        }
        for (i, eff) in ab.applied_effects.iter().enumerate() {
            if eff.apply_chance < 0.0 || eff.apply_chance > 100.0 {
                issues.push(format!("Ability {} effect[{}]: apply_chance out of [0,100]", id, i));
            }
        }
    }
    issues
}

// ============================================================
// SECTION: ABILITY MULTIPLIER STACK RESOLUTION
// ============================================================

/// Resolves stacked damage multipliers using the standard additive-within-category, multiplicative-between-category model.
/// Categories: base, increased (additive within), more (multiplicative between).
pub struct MultiplierStack {
    pub base: f32,
    pub increased: Vec<f32>,
    pub more: Vec<f32>,
}

impl MultiplierStack {
    pub fn new(base: f32) -> Self { Self { base, increased: Vec::new(), more: Vec::new() } }

    pub fn add_increased(&mut self, pct: f32) { self.increased.push(pct); }
    pub fn add_more(&mut self, pct: f32) { self.more.push(pct); }

    pub fn resolve(&self) -> f32 {
        let increased_total = 1.0 + self.increased.iter().sum::<f32>() / 100.0;
        let more_total: f32 = self.more.iter().map(|m| 1.0 + m / 100.0).product();
        self.base * increased_total * more_total
    }

    pub fn resolve_with_resistance(&self, resistance_pct: f32) -> f32 {
        let raw = self.resolve();
        let resist = (resistance_pct / 100.0).clamp(0.0, 0.75);
        raw * (1.0 - resist)
    }

    pub fn marginal_value_of_increased(&self, added_pct: f32) -> f32 {
        let current = self.resolve();
        let mut copy = self.clone_with_increased(added_pct);
        copy.resolve() - current
    }

    fn clone_with_increased(&self, extra: f32) -> Self {
        let mut c = MultiplierStack { base: self.base, increased: self.increased.clone(), more: self.more.clone() };
        c.increased.push(extra);
        c
    }
}

pub fn compare_increased_vs_more(stack: &MultiplierStack, pct: f32) -> (f32, f32) {
    let with_increased = {
        let mut s = MultiplierStack { base: stack.base, increased: stack.increased.clone(), more: stack.more.clone() };
        s.add_increased(pct);
        s.resolve()
    };
    let with_more = {
        let mut s = MultiplierStack { base: stack.base, increased: stack.increased.clone(), more: stack.more.clone() };
        s.add_more(pct);
        s.resolve()
    };
    let current = stack.resolve();
    (with_increased - current, with_more - current)
}

/// Compute the effective damage after full pipeline: multiplier stack + resistance + armor reduction
pub fn full_damage_pipeline(stack: &MultiplierStack, armor: f32, armor_penetration: f32, resistance_pct: f32, resistance_penetration_pct: f32) -> f32 {
    let effective_armor = (armor - armor_penetration).max(0.0);
    let armor_reduction = effective_armor / (effective_armor + 100.0);
    let effective_resist_pct = (resistance_pct - resistance_penetration_pct * resistance_pct).max(0.0);
    stack.resolve() * (1.0 - armor_reduction) * (1.0 - effective_resist_pct / 100.0)
}

// ============================================================
// SECTION: UTILITY MATH HELPERS
// ============================================================

/// Lerp between two f32 values
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t.clamp(0.0, 1.0) }

/// Smoothstep interpolation (3t^2 - 2t^3)
pub fn smoothstep(t: f32) -> f32 { let c = t.clamp(0.0, 1.0); c * c * (3.0 - 2.0 * c) }

/// Inverse lerp: find t such that lerp(a, b, t) == v
pub fn inv_lerp(a: f32, b: f32, v: f32) -> f32 { if (b - a).abs() < 1e-9 { 0.0 } else { ((v - a) / (b - a)).clamp(0.0, 1.0) } }

/// Remap value from [a,b] range to [c,d] range
pub fn remap(v: f32, a: f32, b: f32, c: f32, d: f32) -> f32 { lerp_f32(c, d, inv_lerp(a, b, v)) }

/// Exponential decay: value decays toward target with rate per second
pub fn exp_decay(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    target + (current - target) * (-rate * dt).exp()
}

/// Critically-damped spring toward target (avoids overshoot)
pub fn spring_towards(current: f32, velocity: &mut f32, target: f32, stiffness: f32, dt: f32) -> f32 {
    let damping = 2.0 * stiffness.sqrt();
    let force = -stiffness * (current - target) - damping * (*velocity);
    *velocity += force * dt;
    current + *velocity * dt
}

/// Wrap angle to [-PI, PI]
pub fn wrap_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI { a -= 2.0 * std::f32::consts::PI; }
    while a < -std::f32::consts::PI { a += 2.0 * std::f32::consts::PI; }
    a
}

/// Angular distance between two angles (radians)
pub fn angle_distance(a: f32, b: f32) -> f32 { wrap_angle(b - a).abs() }

// ============================================================
// END OF FILE
// ============================================================
