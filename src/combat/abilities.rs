//! Skill and ability system — cooldowns, mana, resource tracking, ability trees.

use std::collections::HashMap;
use super::Element;

// ── Resource ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Mana,
    Rage,
    Energy,
    Focus,
    Entropy,
    Charges,
}

impl ResourceType {
    pub fn name(self) -> &'static str {
        match self {
            ResourceType::Mana    => "Mana",
            ResourceType::Rage    => "Rage",
            ResourceType::Energy  => "Energy",
            ResourceType::Focus   => "Focus",
            ResourceType::Entropy => "Entropy",
            ResourceType::Charges => "Charges",
        }
    }

    pub fn color(self) -> glam::Vec4 {
        match self {
            ResourceType::Mana    => glam::Vec4::new(0.20, 0.40, 1.00, 1.0),
            ResourceType::Rage    => glam::Vec4::new(1.00, 0.15, 0.10, 1.0),
            ResourceType::Energy  => glam::Vec4::new(1.00, 0.90, 0.10, 1.0),
            ResourceType::Focus   => glam::Vec4::new(0.30, 0.90, 0.60, 1.0),
            ResourceType::Entropy => glam::Vec4::new(0.60, 0.10, 0.80, 1.0),
            ResourceType::Charges => glam::Vec4::new(0.90, 0.90, 0.90, 1.0),
        }
    }

    /// Whether this resource regenerates passively over time.
    pub fn regenerates(self) -> bool {
        matches!(self, ResourceType::Mana | ResourceType::Energy | ResourceType::Focus)
    }

    /// Whether this resource decays when not in combat.
    pub fn decays_out_of_combat(self) -> bool {
        matches!(self, ResourceType::Rage | ResourceType::Entropy)
    }
}

// ── ResourcePool ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ResourcePool {
    pub kind:    ResourceType,
    pub current: f32,
    pub maximum: f32,
    pub regen:   f32,    // per second
    pub decay:   f32,    // per second when decaying
}

impl ResourcePool {
    pub fn new(kind: ResourceType, max: f32) -> Self {
        let regen = match kind {
            ResourceType::Mana   => max * 0.05,
            ResourceType::Energy => max * 0.15,
            ResourceType::Focus  => max * 0.08,
            _                    => 0.0,
        };
        Self { kind, current: max, maximum: max, regen, decay: max * 0.10 }
    }

    pub fn update(&mut self, dt: f32, in_combat: bool) {
        if self.kind.regenerates() && !in_combat {
            self.current = (self.current + self.regen * dt).min(self.maximum);
        }
        if self.kind.decays_out_of_combat() && !in_combat {
            self.current = (self.current - self.decay * dt).max(0.0);
        }
    }

    pub fn spend(&mut self, amount: f32) -> bool {
        if self.current >= amount {
            self.current -= amount;
            true
        } else {
            false
        }
    }

    pub fn restore(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.maximum);
    }

    pub fn fill(&mut self) { self.current = self.maximum; }
    pub fn empty(&mut self) { self.current = 0.0; }

    pub fn percent(&self) -> f32 {
        if self.maximum > 0.0 { self.current / self.maximum } else { 0.0 }
    }

    pub fn is_empty(&self) -> bool { self.current <= 0.0 }
    pub fn is_full(&self) -> bool { self.current >= self.maximum }
}

// ── AbilityTag ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilityTag {
    Attack,
    Spell,
    Movement,
    Channel,
    Toggle,
    Passive,
    Summon,
    Utility,
    Defensive,
    Ultimate,
    AoE,
    SingleTarget,
    Projectile,
    Melee,
    Ranged,
    Instant,
    Delayed,
}

// ── AbilityEffect ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum AbilityEffect {
    /// Deal damage to target
    Damage { base: f32, element: Element, scaling: f32 },
    /// Heal target
    Heal { amount: f32, scaling: f32 },
    /// Apply a status effect
    ApplyStatus { name: String, duration: f32, stacks: u32 },
    /// Teleport caster
    Teleport { range: f32 },
    /// Push target
    Knockback { force: f32, direction_from_caster: bool },
    /// Pull target
    Pull { force: f32 },
    /// Stun target
    Stun { duration: f32 },
    /// Apply damage over time
    DotDamage { dps: f32, element: Element, duration: f32 },
    /// Shield / absorb
    Shield { amount: f32, duration: f32 },
    /// Chain to multiple targets
    Chain { max_jumps: u32, falloff: f32 },
    /// AoE explosion at point
    Explosion { radius: f32, damage: f32, element: Element },
    /// Summon entity
    Summon { entity_id: String, duration: f32 },
    /// Modify resource
    ModifyResource { kind: ResourceType, amount: f32 },
    /// Buff stat temporarily
    StatBuff { stat_name: String, multiplier: f32, duration: f32 },
}

// ── Ability ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Ability {
    pub id:           u32,
    pub name:         String,
    pub description:  String,
    pub tags:         Vec<AbilityTag>,
    pub effects:      Vec<AbilityEffect>,
    pub cooldown:     f32,
    pub cast_time:    f32,
    pub channel_time: f32,
    pub resource_cost: Vec<(ResourceType, f32)>,
    pub range:        f32,
    pub radius:       f32,
    pub level:        u32,
    pub max_level:    u32,
    pub glyph:        char,
    pub rank_bonuses: Vec<RankBonus>,
    pub combo_points_generated: u32,
    pub combo_points_consumed:  Option<u32>,
    pub interrupt_flags: InterruptFlags,
}

#[derive(Debug, Clone)]
pub struct RankBonus {
    pub rank: u32,
    pub description: String,
    pub effect_modifier: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InterruptFlags {
    pub interrupted_by_damage:  bool,
    pub interrupted_by_cc:      bool,
    pub interrupted_by_movement: bool,
}

impl Ability {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            tags: Vec::new(),
            effects: Vec::new(),
            cooldown: 0.0,
            cast_time: 0.0,
            channel_time: 0.0,
            resource_cost: Vec::new(),
            range: 1.0,
            radius: 0.0,
            level: 1,
            max_level: 5,
            glyph: '◆',
            rank_bonuses: Vec::new(),
            combo_points_generated: 0,
            combo_points_consumed: None,
            interrupt_flags: InterruptFlags::default(),
        }
    }

    pub fn with_description(mut self, d: impl Into<String>) -> Self { self.description = d.into(); self }
    pub fn with_cooldown(mut self, cd: f32) -> Self { self.cooldown = cd; self }
    pub fn with_cast_time(mut self, ct: f32) -> Self { self.cast_time = ct; self }
    pub fn with_cost(mut self, resource: ResourceType, amount: f32) -> Self {
        self.resource_cost.push((resource, amount)); self
    }
    pub fn with_range(mut self, r: f32) -> Self { self.range = r; self }
    pub fn with_radius(mut self, r: f32) -> Self { self.radius = r; self }
    pub fn with_tag(mut self, tag: AbilityTag) -> Self { self.tags.push(tag); self }
    pub fn with_effect(mut self, eff: AbilityEffect) -> Self { self.effects.push(eff); self }
    pub fn with_glyph(mut self, g: char) -> Self { self.glyph = g; self }

    pub fn has_tag(&self, tag: AbilityTag) -> bool { self.tags.contains(&tag) }

    pub fn is_instant(&self) -> bool { self.cast_time <= 0.0 && self.channel_time <= 0.0 }

    pub fn scaled_damage(&self, base_attack: f32) -> f32 {
        let level_mult = 1.0 + (self.level as f32 - 1.0) * 0.15;
        self.effects.iter()
            .filter_map(|e| match e {
                AbilityEffect::Damage { base, scaling, .. } => Some(base + scaling * base_attack),
                _ => None,
            })
            .sum::<f32>() * level_mult
    }

    pub fn tooltip(&self) -> Vec<String> {
        let mut lines = vec![
            format!("{} (Rank {})", self.name, self.level),
            self.description.clone(),
            format!("Cooldown: {:.1}s | Range: {:.1}", self.cooldown, self.range),
        ];
        if self.cast_time > 0.0 {
            lines.push(format!("Cast time: {:.1}s", self.cast_time));
        }
        for (res, cost) in &self.resource_cost {
            lines.push(format!("Cost: {:.0} {}", cost, res.name()));
        }
        for tag in &self.tags {
            lines.push(format!("[{:?}]", tag));
        }
        lines
    }

    // ── Preset abilities ──────────────────────────────────────────────────────

    pub fn fireball() -> Self {
        Ability::new(1, "Fireball")
            .with_description("Hurls a sphere of entropic fire at the target.")
            .with_cooldown(3.0)
            .with_cast_time(1.0)
            .with_cost(ResourceType::Mana, 40.0)
            .with_range(12.0)
            .with_radius(3.0)
            .with_tag(AbilityTag::Spell)
            .with_tag(AbilityTag::AoE)
            .with_tag(AbilityTag::Projectile)
            .with_effect(AbilityEffect::Explosion {
                radius: 3.0,
                damage: 80.0,
                element: Element::Fire,
            })
            .with_glyph('♨')
    }

    pub fn blink() -> Self {
        Ability::new(2, "Blink")
            .with_description("Instantly teleport a short distance.")
            .with_cooldown(8.0)
            .with_cost(ResourceType::Mana, 25.0)
            .with_range(8.0)
            .with_tag(AbilityTag::Movement)
            .with_tag(AbilityTag::Instant)
            .with_effect(AbilityEffect::Teleport { range: 8.0 })
            .with_glyph('⟿')
    }

    pub fn void_strike() -> Self {
        Ability::new(3, "Void Strike")
            .with_description("A heavy melee strike that tears through dimensional barriers.")
            .with_cooldown(6.0)
            .with_cast_time(0.3)
            .with_cost(ResourceType::Rage, 30.0)
            .with_range(2.0)
            .with_tag(AbilityTag::Attack)
            .with_tag(AbilityTag::Melee)
            .with_tag(AbilityTag::SingleTarget)
            .with_effect(AbilityEffect::Damage { base: 120.0, element: Element::Void, scaling: 1.8 })
            .with_effect(AbilityEffect::ApplyStatus { name: "Void Shred".to_string(), duration: 4.0, stacks: 1 })
            .with_glyph('◈')
    }

    pub fn temporal_freeze() -> Self {
        Ability::new(4, "Temporal Freeze")
            .with_description("Slows time around the target, stunnning them briefly.")
            .with_cooldown(12.0)
            .with_cast_time(0.5)
            .with_cost(ResourceType::Mana, 60.0)
            .with_range(10.0)
            .with_tag(AbilityTag::Spell)
            .with_tag(AbilityTag::SingleTarget)
            .with_effect(AbilityEffect::Stun { duration: 2.5 })
            .with_effect(AbilityEffect::DotDamage { dps: 20.0, element: Element::Temporal, duration: 3.0 })
            .with_glyph('⧗')
    }

    pub fn entropy_cascade() -> Self {
        Ability::new(5, "Entropy Cascade")
            .with_description("Unleash a wave of pure entropy that chains between enemies.")
            .with_cooldown(20.0)
            .with_cast_time(1.5)
            .with_cost(ResourceType::Entropy, 80.0)
            .with_range(15.0)
            .with_tag(AbilityTag::Spell)
            .with_tag(AbilityTag::AoE)
            .with_tag(AbilityTag::Ultimate)
            .with_effect(AbilityEffect::Damage { base: 200.0, element: Element::Entropy, scaling: 2.5 })
            .with_effect(AbilityEffect::Chain { max_jumps: 5, falloff: 0.15 })
            .with_glyph('∞')
    }

    pub fn iron_skin() -> Self {
        Ability::new(6, "Iron Skin")
            .with_description("Harden your body, gaining a protective shield.")
            .with_cooldown(15.0)
            .with_tag(AbilityTag::Defensive)
            .with_tag(AbilityTag::Instant)
            .with_effect(AbilityEffect::Shield { amount: 150.0, duration: 8.0 })
            .with_effect(AbilityEffect::StatBuff { stat_name: "armor".to_string(), multiplier: 1.5, duration: 8.0 })
            .with_glyph('⚙')
    }
}

// ── AbilityState ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AbilityState {
    pub ability:         Ability,
    pub cooldown_remaining: f32,
    pub is_casting:      bool,
    pub cast_progress:   f32,
    pub is_channeling:   bool,
    pub channel_elapsed: f32,
    pub is_on_gcd:       bool,  // global cooldown
}

impl AbilityState {
    pub fn new(ability: Ability) -> Self {
        Self {
            ability,
            cooldown_remaining: 0.0,
            is_casting: false,
            cast_progress: 0.0,
            is_channeling: false,
            channel_elapsed: 0.0,
            is_on_gcd: false,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.cooldown_remaining > 0.0 {
            self.cooldown_remaining = (self.cooldown_remaining - dt).max(0.0);
        }
        if self.is_casting {
            self.cast_progress += dt;
            if self.cast_progress >= self.ability.cast_time {
                self.is_casting = false;
                self.cast_progress = 0.0;
            }
        }
        if self.is_channeling {
            self.channel_elapsed += dt;
            if self.channel_elapsed >= self.ability.channel_time {
                self.is_channeling = false;
                self.channel_elapsed = 0.0;
            }
        }
    }

    pub fn is_ready(&self) -> bool {
        self.cooldown_remaining <= 0.0 && !self.is_casting && !self.is_channeling && !self.is_on_gcd
    }

    pub fn trigger(&mut self) {
        self.cooldown_remaining = self.ability.cooldown;
        if self.ability.cast_time > 0.0 {
            self.is_casting = true;
            self.cast_progress = 0.0;
        }
    }

    pub fn interrupt(&mut self) {
        if self.ability.interrupt_flags.interrupted_by_damage {
            self.is_casting = false;
            self.cast_progress = 0.0;
            self.is_channeling = false;
            self.channel_elapsed = 0.0;
        }
    }

    pub fn cast_percent(&self) -> f32 {
        if self.ability.cast_time > 0.0 {
            (self.cast_progress / self.ability.cast_time).min(1.0)
        } else { 0.0 }
    }

    pub fn cooldown_percent(&self) -> f32 {
        if self.ability.cooldown > 0.0 {
            1.0 - (self.cooldown_remaining / self.ability.cooldown).min(1.0)
        } else { 1.0 }
    }
}

// ── AbilityBar ────────────────────────────────────────────────────────────────

pub const MAX_ABILITY_SLOTS: usize = 12;

#[derive(Debug, Clone)]
pub struct AbilityBar {
    pub slots: Vec<Option<AbilityState>>,
    pub resources: HashMap<ResourceType, ResourcePool>,
    pub global_cooldown: f32,
    pub gcd_remaining:   f32,
    pub combo_points:    u32,
    pub max_combo_points: u32,
    pub in_combat:       bool,
    pub combat_timer:    f32,
}

impl AbilityBar {
    pub fn new() -> Self {
        Self {
            slots: vec![None; MAX_ABILITY_SLOTS],
            resources: HashMap::new(),
            global_cooldown: 1.5,
            gcd_remaining: 0.0,
            combo_points: 0,
            max_combo_points: 5,
            in_combat: false,
            combat_timer: 0.0,
        }
    }

    pub fn add_resource(&mut self, kind: ResourceType, max: f32) {
        self.resources.insert(kind, ResourcePool::new(kind, max));
    }

    pub fn assign(&mut self, slot: usize, ability: Ability) {
        if slot < MAX_ABILITY_SLOTS {
            self.slots[slot] = Some(AbilityState::new(ability));
        }
    }

    pub fn unassign(&mut self, slot: usize) {
        if slot < MAX_ABILITY_SLOTS {
            self.slots[slot] = None;
        }
    }

    pub fn update(&mut self, dt: f32) {
        // Update GCD
        if self.gcd_remaining > 0.0 {
            self.gcd_remaining = (self.gcd_remaining - dt).max(0.0);
            for slot in self.slots.iter_mut().flatten() {
                slot.is_on_gcd = self.gcd_remaining > 0.0;
            }
        }

        // Update cooldowns and cast states
        for slot in self.slots.iter_mut().flatten() {
            slot.update(dt);
        }

        // Update resources
        for pool in self.resources.values_mut() {
            pool.update(dt, self.in_combat);
        }

        // Combat timer (leave combat after 5s of no attacks)
        if self.in_combat {
            self.combat_timer -= dt;
            if self.combat_timer <= 0.0 {
                self.in_combat = false;
            }
        }
    }

    pub fn can_use(&self, slot: usize) -> Result<(), AbilityCastError> {
        let state = self.slots.get(slot)
            .and_then(|s| s.as_ref())
            .ok_or(AbilityCastError::NoAbilityInSlot)?;

        if !state.is_ready() {
            return Err(if state.cooldown_remaining > 0.0 {
                AbilityCastError::OnCooldown { remaining: state.cooldown_remaining }
            } else {
                AbilityCastError::AlreadyCasting
            });
        }

        for (res_type, cost) in &state.ability.resource_cost {
            let pool = self.resources.get(res_type)
                .ok_or(AbilityCastError::NotEnoughResource(*res_type))?;
            if pool.current < *cost {
                return Err(AbilityCastError::NotEnoughResource(*res_type));
            }
        }

        if let Some(req) = state.ability.combo_points_consumed {
            if self.combo_points < req {
                return Err(AbilityCastError::NotEnoughComboPoints { need: req, have: self.combo_points });
            }
        }

        Ok(())
    }

    pub fn use_ability(&mut self, slot: usize) -> Result<AbilityUseResult, AbilityCastError> {
        self.can_use(slot)?;

        let state = self.slots[slot].as_mut().unwrap();
        let ability_clone = state.ability.clone();
        state.trigger();

        // Spend resources
        for (res_type, cost) in &ability_clone.resource_cost {
            if let Some(pool) = self.resources.get_mut(res_type) {
                pool.spend(*cost);
            }
        }

        // Spend combo points
        if let Some(req) = ability_clone.combo_points_consumed {
            self.combo_points = self.combo_points.saturating_sub(req);
        }

        // Generate combo points
        self.combo_points = (self.combo_points + ability_clone.combo_points_generated)
            .min(self.max_combo_points);

        // Trigger GCD if not instant
        if !ability_clone.is_instant() {
            self.gcd_remaining = self.global_cooldown;
        }

        // Enter combat
        self.in_combat = true;
        self.combat_timer = 5.0;

        Ok(AbilityUseResult {
            ability: ability_clone,
            instant: self.slots[slot].as_ref().unwrap().ability.is_instant(),
        })
    }

    pub fn interrupt_all(&mut self) {
        for slot in self.slots.iter_mut().flatten() {
            slot.interrupt();
        }
    }

    pub fn get_slot(&self, slot: usize) -> Option<&AbilityState> {
        self.slots.get(slot)?.as_ref()
    }

    pub fn resource(&self, kind: ResourceType) -> Option<&ResourcePool> {
        self.resources.get(&kind)
    }

    pub fn all_on_cooldown(&self) -> bool {
        self.slots.iter().flatten().all(|s| !s.is_ready())
    }
}

#[derive(Debug, Clone)]
pub struct AbilityUseResult {
    pub ability: Ability,
    pub instant: bool,
}

#[derive(Debug, Clone)]
pub enum AbilityCastError {
    NoAbilityInSlot,
    OnCooldown { remaining: f32 },
    AlreadyCasting,
    NotEnoughResource(ResourceType),
    NotEnoughComboPoints { need: u32, have: u32 },
    OutOfRange,
    InvalidTarget,
}

impl std::fmt::Display for AbilityCastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAbilityInSlot             => write!(f, "No ability in that slot"),
            Self::OnCooldown { remaining }    => write!(f, "On cooldown ({:.1}s)", remaining),
            Self::AlreadyCasting              => write!(f, "Already casting"),
            Self::NotEnoughResource(r)        => write!(f, "Not enough {}", r.name()),
            Self::NotEnoughComboPoints { need, have } => write!(f, "Need {} combo points, have {}", need, have),
            Self::OutOfRange                  => write!(f, "Target is out of range"),
            Self::InvalidTarget               => write!(f, "Invalid target"),
        }
    }
}

// ── AbilityTree ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AbilityNode {
    pub id:           u32,
    pub ability:      Ability,
    pub required_points: u32,
    pub prerequisites: Vec<u32>,  // node IDs that must be unlocked first
    pub position:     (f32, f32),  // for UI layout
    pub unlocked:     bool,
}

#[derive(Debug, Clone)]
pub struct AbilityTree {
    pub name:  String,
    pub nodes: Vec<AbilityNode>,
    pub spent_points: u32,
}

impl AbilityTree {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), nodes: Vec::new(), spent_points: 0 }
    }

    pub fn add_node(&mut self, ability: Ability, required_points: u32, prereqs: Vec<u32>, pos: (f32, f32)) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(AbilityNode {
            id,
            ability,
            required_points,
            prerequisites: prereqs,
            position: pos,
            unlocked: false,
        });
        id
    }

    pub fn can_unlock(&self, node_id: u32, available_points: u32) -> bool {
        if let Some(node) = self.nodes.iter().find(|n| n.id == node_id) {
            if node.unlocked { return false; }
            if available_points < node.required_points + self.spent_points { return false; }
            node.prerequisites.iter().all(|&prereq_id| {
                self.nodes.iter().find(|n| n.id == prereq_id).map(|n| n.unlocked).unwrap_or(false)
            })
        } else {
            false
        }
    }

    pub fn unlock(&mut self, node_id: u32, available_points: u32) -> bool {
        if !self.can_unlock(node_id, available_points) { return false; }
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            node.unlocked = true;
            self.spent_points += node.required_points;
            true
        } else {
            false
        }
    }

    pub fn unlocked_abilities(&self) -> Vec<&Ability> {
        self.nodes.iter().filter(|n| n.unlocked).map(|n| &n.ability).collect()
    }

    pub fn available_nodes(&self, available_points: u32) -> Vec<u32> {
        self.nodes.iter()
            .filter(|n| !n.unlocked && self.can_unlock(n.id, available_points))
            .map(|n| n.id)
            .collect()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_pool_spend() {
        let mut pool = ResourcePool::new(ResourceType::Mana, 100.0);
        assert!(pool.spend(40.0));
        assert!((pool.current - 60.0).abs() < 0.01);
        assert!(!pool.spend(80.0));  // not enough
    }

    #[test]
    fn test_ability_bar_use() {
        let mut bar = AbilityBar::new();
        bar.add_resource(ResourceType::Mana, 200.0);
        bar.assign(0, Ability::fireball());
        assert!(bar.can_use(0).is_ok());
        let result = bar.use_ability(0);
        assert!(result.is_ok());
        // Ability should now be on cooldown
        assert!(bar.can_use(0).is_err());
    }

    #[test]
    fn test_ability_tree_unlock() {
        let mut tree = AbilityTree::new("Mage");
        let root = tree.add_node(Ability::fireball(), 1, vec![], (0.0, 0.0));
        let branch = tree.add_node(Ability::blink(), 2, vec![root], (1.0, 0.0));

        assert!(tree.unlock(root, 5));
        assert!(!tree.unlock(branch, 2)); // not enough points spent yet
        assert!(tree.unlock(branch, 10));
        assert_eq!(tree.unlocked_abilities().len(), 2);
    }

    #[test]
    fn test_cooldown_tracking() {
        let mut state = AbilityState::new(Ability::fireball());
        assert!(state.is_ready());
        state.trigger();
        assert!(!state.is_ready());
        state.update(10.0);  // fast-forward past cooldown
        assert!(state.is_ready());
    }
}
