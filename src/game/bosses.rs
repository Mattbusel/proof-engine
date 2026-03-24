//! Boss Encounter Orchestration System
//!
//! Ten unique boss encounters for the Chaos RPG, each with multi-phase mechanics,
//! special abilities, and emergent behaviors. Bosses are driven by a phase controller
//! that monitors HP thresholds and triggers transitions with visual animations.
//!
//! # Bosses
//! - **Mirror**: copies player abilities with delay
//! - **Null**: progressively erases game elements
//! - **Committee**: five judges vote on actions
//! - **FibonacciHydra**: splits recursively on death
//! - **Eigenstate**: quantum superposition of forms
//! - **Ouroboros**: reversed healing/damage semantics
//! - **AlgorithmReborn**: learns and predicts player patterns (final boss)
//! - **ChaosWeaver**: manipulates game rules
//! - **VoidSerpent**: consumes the arena
//! - **PrimeFactorial**: arithmetic puzzle boss

use std::collections::HashMap;
use crate::combat::{Element, CombatStats, ResistanceProfile};
use crate::entity::AmorphousEntity;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss Type Registry
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// All boss types in the Chaos RPG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BossType {
    Mirror,
    Null,
    Committee,
    FibonacciHydra,
    Eigenstate,
    Ouroboros,
    AlgorithmReborn,
    ChaosWeaver,
    VoidSerpent,
    PrimeFactorial,
}

impl BossType {
    /// All boss types in order.
    pub fn all() -> &'static [BossType] {
        &[
            BossType::Mirror,
            BossType::Null,
            BossType::Committee,
            BossType::FibonacciHydra,
            BossType::Eigenstate,
            BossType::Ouroboros,
            BossType::AlgorithmReborn,
            BossType::ChaosWeaver,
            BossType::VoidSerpent,
            BossType::PrimeFactorial,
        ]
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            BossType::Mirror => "The Mirror",
            BossType::Null => "The Null",
            BossType::Committee => "The Committee",
            BossType::FibonacciHydra => "Fibonacci Hydra",
            BossType::Eigenstate => "The Eigenstate",
            BossType::Ouroboros => "Ouroboros",
            BossType::AlgorithmReborn => "Algorithm Reborn",
            BossType::ChaosWeaver => "Chaos Weaver",
            BossType::VoidSerpent => "Void Serpent",
            BossType::PrimeFactorial => "Prime Factorial",
        }
    }

    /// Boss subtitle/title.
    pub fn title(self) -> &'static str {
        match self {
            BossType::Mirror => "Reflection of Self",
            BossType::Null => "The Eraser of Meaning",
            BossType::Committee => "Democracy of Violence",
            BossType::FibonacciHydra => "The Golden Recursion",
            BossType::Eigenstate => "Collapsed Possibility",
            BossType::Ouroboros => "The Serpent That Devours",
            BossType::AlgorithmReborn => "Final Proof",
            BossType::ChaosWeaver => "Unraveler of Rules",
            BossType::VoidSerpent => "Consumer of Arenas",
            BossType::PrimeFactorial => "The Indivisible Explosion",
        }
    }

    /// Boss tier (1 = earliest, 5 = final boss).
    pub fn tier(self) -> u32 {
        match self {
            BossType::Mirror => 1,
            BossType::Null => 2,
            BossType::Committee => 2,
            BossType::FibonacciHydra => 3,
            BossType::Eigenstate => 3,
            BossType::Ouroboros => 3,
            BossType::ChaosWeaver => 4,
            BossType::VoidSerpent => 4,
            BossType::PrimeFactorial => 4,
            BossType::AlgorithmReborn => 5,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Music & Arena
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Music style for a boss encounter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicType {
    Ominous,
    Frenetic,
    Orchestral,
    Glitch,
    Silence,
    Reversed,
    Algorithmic,
    Chaotic,
    Crescendo,
    MinimalDrone,
}

/// Modifications applied to the arena during the boss fight.
#[derive(Debug, Clone, PartialEq)]
pub enum ArenaMod {
    /// Shrink arena by removing edge tiles.
    ShrinkEdges { rate_per_turn: u32 },
    /// Add hazard tiles of the given element.
    HazardTiles { element: Element, count: u32 },
    /// Darken vision range.
    DarkenVision { radius_reduction: f32 },
    /// Invert movement controls.
    InvertControls,
    /// Tiles become slippery (momentum).
    SlipperyFloor { friction: f32 },
    /// Random teleport traps.
    TeleportTraps { count: u32 },
    /// No arena modifications.
    None,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss Phase System
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// How a phase transition is visually animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseTransition {
    /// Glyphs rearrange into new formation.
    GlyphReorganize,
    /// Entity dissolves and reforms.
    Dissolve,
    /// Entity splits into multiple parts.
    Split,
    /// Multiple parts merge into one.
    Merge,
    /// Entity teleports to new position.
    Teleport,
    /// Entity charges up with visual energy.
    PowerUp,
}

/// A special ability usable during a boss phase.
#[derive(Debug, Clone, PartialEq)]
pub enum SpecialAbility {
    /// Copy the player's last N abilities.
    MirrorCopy { depth: usize },
    /// Erase a specific game element.
    Erase(EraseTarget),
    /// Summon helper entities.
    Summon { count: u32, hp_each: f32 },
    /// Area-of-effect blast.
    AoeBlast { radius: f32, damage: f32, element: Element },
    /// Self-heal.
    SelfHeal { amount: f32 },
    /// Lock one of the player's abilities.
    LockAbility,
    /// Quantum collapse into a specific form.
    QuantumCollapse,
    /// Reverse damage/heal semantics.
    ReverseSemantic,
    /// Counter the player's most-used action.
    CounterPredict,
    /// Markov-chain based prediction of next player action.
    MarkovPredict,
    /// Randomize game rules.
    RuleRandomize,
    /// Consume arena tiles.
    ConsumeArena { columns: u32 },
    /// Factorial damage sequence.
    FactorialStrike { sequence_index: u32 },
    /// Arithmetic puzzle: player must deal specific damage.
    ArithmeticPuzzle { target_factors: Vec<u32> },
    /// Split into fibonacci sub-entities.
    FibonacciSplit,
    /// Vote-based action selection.
    CommitteeVote,
    /// Entangle with a copy.
    Entangle,
    /// No special ability.
    None,
}

/// What the Null boss can erase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseTarget {
    PlayerBuffs,
    HpBar,
    MiniMap,
    AbilitySlot,
    InventorySlot,
    DamageNumbers,
    BossHpBar,
}

/// Behavior pattern for a boss during a phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorPattern {
    /// Standard attack cycle.
    Standard,
    /// Aggressive: shorter cooldowns, higher damage.
    Aggressive,
    /// Defensive: heals, blocks, retreats.
    Defensive,
    /// Erratic: random action selection.
    Erratic,
    /// Calculated: optimal counter-play.
    Calculated,
    /// Passive: minimal attacks, focus on mechanics.
    Passive,
    /// Berserk: maximum aggression, ignores defense.
    Berserk,
}

/// A single phase of a boss encounter.
#[derive(Debug, Clone, PartialEq)]
pub struct BossPhase {
    /// Phase index (1-based).
    pub phase_number: u32,
    /// HP threshold (as percentage) at which this phase activates.
    /// e.g., 0.75 means phase activates when boss drops below 75% HP.
    pub hp_threshold_pct: f32,
    /// Behavior pattern during this phase.
    pub behavior_pattern: BehaviorPattern,
    /// Speed multiplier for this phase.
    pub speed_mult: f32,
    /// Damage multiplier for this phase.
    pub damage_mult: f32,
    /// Special ability available during this phase.
    pub special_ability: SpecialAbility,
    /// How the transition into this phase is animated.
    pub transition_animation: PhaseTransition,
    /// Dialogue spoken when entering this phase.
    pub dialogue_on_enter: String,
}

impl BossPhase {
    pub fn new(phase_number: u32, hp_threshold_pct: f32) -> Self {
        Self {
            phase_number,
            hp_threshold_pct,
            behavior_pattern: BehaviorPattern::Standard,
            speed_mult: 1.0,
            damage_mult: 1.0,
            special_ability: SpecialAbility::None,
            transition_animation: PhaseTransition::PowerUp,
            dialogue_on_enter: String::new(),
        }
    }

    pub fn with_behavior(mut self, pattern: BehaviorPattern) -> Self {
        self.behavior_pattern = pattern;
        self
    }

    pub fn with_speed(mut self, mult: f32) -> Self {
        self.speed_mult = mult;
        self
    }

    pub fn with_damage(mut self, mult: f32) -> Self {
        self.damage_mult = mult;
        self
    }

    pub fn with_ability(mut self, ability: SpecialAbility) -> Self {
        self.special_ability = ability;
        self
    }

    pub fn with_transition(mut self, anim: PhaseTransition) -> Self {
        self.transition_animation = anim;
        self
    }

    pub fn with_dialogue(mut self, dialogue: impl Into<String>) -> Self {
        self.dialogue_on_enter = dialogue.into();
        self
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss Profile
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Loot drop definition for a boss.
#[derive(Debug, Clone)]
pub struct BossLootEntry {
    pub item_name: String,
    pub drop_chance: f32,
    pub min_quantity: u32,
    pub max_quantity: u32,
}

/// Complete profile describing a boss encounter.
#[derive(Debug, Clone)]
pub struct BossProfile {
    pub boss_type: BossType,
    pub name: String,
    pub title: String,
    pub hp_base: f32,
    pub damage_base: f32,
    pub tier: u32,
    pub phases: Vec<BossPhase>,
    pub special_mechanics: Vec<String>,
    pub loot_table: Vec<BossLootEntry>,
    pub music_type: MusicType,
    pub arena_mods: Vec<ArenaMod>,
    pub resistance: ResistanceProfile,
}

impl BossProfile {
    /// Scale boss stats for a given floor level.
    pub fn scaled_hp(&self, floor: u32) -> f32 {
        self.hp_base * (1.0 + 0.15 * floor as f32)
    }

    pub fn scaled_damage(&self, floor: u32) -> f32 {
        self.damage_base * (1.0 + 0.10 * floor as f32)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss Phase Controller
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Controls phase transitions based on HP thresholds.
#[derive(Debug, Clone)]
pub struct BossPhaseController {
    /// All phases, sorted by hp_threshold_pct descending.
    phases: Vec<BossPhase>,
    /// Index of the currently active phase.
    current_phase_idx: usize,
    /// Whether a transition is currently animating.
    transitioning: bool,
    /// Timer for transition animation.
    transition_timer: f32,
    /// Duration of transition animations.
    transition_duration: f32,
}

impl BossPhaseController {
    pub fn new(mut phases: Vec<BossPhase>) -> Self {
        // Sort phases by HP threshold descending (phase 1 = highest threshold).
        phases.sort_by(|a, b| b.hp_threshold_pct.partial_cmp(&a.hp_threshold_pct).unwrap());
        Self {
            phases,
            current_phase_idx: 0,
            transitioning: false,
            transition_timer: 0.0,
            transition_duration: 1.5,
        }
    }

    /// Current active phase.
    pub fn current_phase(&self) -> Option<&BossPhase> {
        self.phases.get(self.current_phase_idx)
    }

    /// Current phase number (1-based).
    pub fn current_phase_number(&self) -> u32 {
        self.current_phase()
            .map(|p| p.phase_number)
            .unwrap_or(1)
    }

    /// Check HP fraction and potentially trigger a phase transition.
    /// Returns `Some(BossPhase)` if a new phase was entered.
    pub fn check_transition(&mut self, hp_fraction: f32) -> Option<&BossPhase> {
        if self.transitioning {
            return None;
        }

        // Find the deepest phase whose threshold we've crossed.
        let mut target_idx = self.current_phase_idx;
        for (i, phase) in self.phases.iter().enumerate() {
            if i > self.current_phase_idx && hp_fraction <= phase.hp_threshold_pct {
                target_idx = i;
            }
        }

        if target_idx != self.current_phase_idx {
            self.current_phase_idx = target_idx;
            self.transitioning = true;
            self.transition_timer = 0.0;
            return self.phases.get(self.current_phase_idx);
        }

        None
    }

    /// Update transition animation timer. Returns true if transition just completed.
    pub fn update_transition(&mut self, dt: f32) -> bool {
        if !self.transitioning {
            return false;
        }
        self.transition_timer += dt;
        if self.transition_timer >= self.transition_duration {
            self.transitioning = false;
            return true;
        }
        false
    }

    /// Whether a transition is in progress.
    pub fn is_transitioning(&self) -> bool {
        self.transitioning
    }

    /// Transition progress [0, 1].
    pub fn transition_progress(&self) -> f32 {
        if !self.transitioning {
            return 0.0;
        }
        (self.transition_timer / self.transition_duration).clamp(0.0, 1.0)
    }

    /// Total number of phases.
    pub fn phase_count(&self) -> usize {
        self.phases.len()
    }

    /// Get the speed multiplier for the current phase.
    pub fn speed_mult(&self) -> f32 {
        self.current_phase().map(|p| p.speed_mult).unwrap_or(1.0)
    }

    /// Get the damage multiplier for the current phase.
    pub fn damage_mult(&self) -> f32 {
        self.current_phase().map(|p| p.damage_mult).unwrap_or(1.0)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Player Action Tracking (used by several bosses)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A recorded player action for boss mechanics that react to player behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlayerActionType {
    Attack,
    Defend,
    Heal,
    UseAbility(u32),
    UseItem,
    Move,
    Wait,
}

/// Recorded player action with metadata.
#[derive(Debug, Clone)]
pub struct RecordedAction {
    pub action_type: PlayerActionType,
    pub turn: u32,
    pub damage_dealt: f32,
    pub element: Option<Element>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Individual Boss Mechanic States
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// ── Mirror Boss ──────────────────────────────────────────────────────────────────

/// State for the Mirror Boss.
/// Copies player's last 3 abilities with a 1-turn delay.
/// Phase 2: also copies player stats.
/// Phase 3: acts simultaneously with copied action.
#[derive(Debug, Clone)]
pub struct MirrorBossState {
    /// Buffer of player actions to copy (FIFO, max 3).
    pub mirror_buffer: Vec<RecordedAction>,
    /// Maximum buffer depth.
    pub buffer_depth: usize,
    /// Delay in turns before copied action is used.
    pub copy_delay: u32,
    /// Whether stats are also being copied (phase 2+).
    pub copying_stats: bool,
    /// Whether actions are simultaneous (phase 3).
    pub simultaneous: bool,
    /// Copied player stats (if copying_stats is true).
    pub copied_attack: f32,
    pub copied_defense: f32,
}

impl MirrorBossState {
    pub fn new() -> Self {
        Self {
            mirror_buffer: Vec::new(),
            buffer_depth: 3,
            copy_delay: 1,
            copying_stats: false,
            simultaneous: false,
            copied_attack: 0.0,
            copied_defense: 0.0,
        }
    }

    /// Record a player action into the mirror buffer.
    pub fn record_action(&mut self, action: RecordedAction) {
        self.mirror_buffer.push(action);
        if self.mirror_buffer.len() > self.buffer_depth {
            self.mirror_buffer.remove(0);
        }
    }

    /// Get the action to mirror this turn (with delay).
    pub fn get_mirrored_action(&self, current_turn: u32) -> Option<&RecordedAction> {
        self.mirror_buffer
            .iter()
            .find(|a| a.turn + self.copy_delay == current_turn)
    }

    /// Copy player stats (for phase 2+).
    pub fn copy_stats(&mut self, player_stats: &CombatStats) {
        self.copied_attack = player_stats.attack;
        self.copied_defense = player_stats.armor;
        self.copying_stats = true;
    }

    /// Transition to phase 2: enable stat copying.
    pub fn enter_phase2(&mut self) {
        self.copying_stats = true;
    }

    /// Transition to phase 3: enable simultaneous action.
    pub fn enter_phase3(&mut self) {
        self.simultaneous = true;
    }
}

impl Default for MirrorBossState {
    fn default() -> Self { Self::new() }
}

// ── Null Boss ────────────────────────────────────────────────────────────────────

/// State for the Null Boss.
/// Phase 1: erases player buffs.
/// Phase 2: erases UI elements (HP bar, map).
/// Phase 3: erases player abilities (random lock each turn).
/// Death: everything restores.
#[derive(Debug, Clone)]
pub struct NullBossState {
    /// Which UI elements have been erased.
    pub erased_ui: Vec<EraseTarget>,
    /// Which ability slots are currently locked.
    pub locked_abilities: Vec<u32>,
    /// Maximum abilities that can be locked simultaneously.
    pub max_locked: usize,
    /// How many buffs have been erased total.
    pub buffs_erased: u32,
    /// Pseudo-random state for selecting targets.
    pub rng_state: u64,
}

impl NullBossState {
    pub fn new() -> Self {
        Self {
            erased_ui: Vec::new(),
            locked_abilities: Vec::new(),
            max_locked: 3,
            buffs_erased: 0,
            rng_state: 0xDEAD_BEEF_CAFE_1234,
        }
    }

    /// Simple xorshift PRNG.
    fn next_rng(&mut self) -> u64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    /// Erase a random UI element (phase 2).
    pub fn erase_ui_element(&mut self) -> EraseTarget {
        let targets = [
            EraseTarget::HpBar,
            EraseTarget::MiniMap,
            EraseTarget::DamageNumbers,
        ];
        let idx = (self.next_rng() as usize) % targets.len();
        let target = targets[idx];
        if !self.erased_ui.contains(&target) {
            self.erased_ui.push(target);
        }
        target
    }

    /// Lock a random ability slot (phase 3).
    pub fn lock_random_ability(&mut self, max_slots: u32) -> Option<u32> {
        if self.locked_abilities.len() >= self.max_locked || max_slots == 0 {
            return None;
        }
        let slot = (self.next_rng() as u32) % max_slots;
        if !self.locked_abilities.contains(&slot) {
            self.locked_abilities.push(slot);
            Some(slot)
        } else {
            None
        }
    }

    /// Erase player buffs (phase 1). Returns number erased.
    pub fn erase_buffs(&mut self, active_buff_count: u32) -> u32 {
        let to_erase = active_buff_count.min(2); // erase up to 2 per turn
        self.buffs_erased += to_erase;
        to_erase
    }

    /// On death: restore everything.
    pub fn restore_all(&mut self) -> Vec<EraseTarget> {
        let restored = self.erased_ui.clone();
        self.erased_ui.clear();
        self.locked_abilities.clear();
        restored
    }
}

impl Default for NullBossState {
    fn default() -> Self { Self::new() }
}

// ── Committee Boss ───────────────────────────────────────────────────────────────

/// Personality of a committee judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgePersonality {
    Aggressive,
    Defensive,
    Random,
    Strategic,
    Chaotic,
}

/// A single judge in the Committee boss.
#[derive(Debug, Clone)]
pub struct Judge {
    pub id: u32,
    pub personality: JudgePersonality,
    pub hp: f32,
    pub max_hp: f32,
    pub alive: bool,
    /// Dead judges become ghosts that still vote (phase 2+).
    pub ghost: bool,
    /// Whether this judge is currently lit up (voting).
    pub voting: bool,
    /// The action this judge voted for.
    pub current_vote: Option<CommitteeAction>,
}

/// Actions the committee can vote on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitteeAction {
    Attack,
    Defend,
    HeavyStrike,
    Heal,
    Buff,
    Summon,
    SpecialAttack,
}

impl Judge {
    pub fn new(id: u32, personality: JudgePersonality, hp: f32) -> Self {
        Self {
            id,
            personality,
            hp,
            max_hp: hp,
            alive: true,
            ghost: false,
            voting: false,
            current_vote: None,
        }
    }

    /// Cast a vote based on personality and context.
    pub fn cast_vote(&mut self, boss_hp_frac: f32, rng_val: u64) -> CommitteeAction {
        let vote = match self.personality {
            JudgePersonality::Aggressive => {
                if boss_hp_frac < 0.3 {
                    CommitteeAction::HeavyStrike
                } else {
                    CommitteeAction::Attack
                }
            }
            JudgePersonality::Defensive => {
                if boss_hp_frac < 0.5 {
                    CommitteeAction::Heal
                } else {
                    CommitteeAction::Defend
                }
            }
            JudgePersonality::Random => {
                let actions = [
                    CommitteeAction::Attack,
                    CommitteeAction::Defend,
                    CommitteeAction::HeavyStrike,
                    CommitteeAction::Heal,
                    CommitteeAction::Buff,
                    CommitteeAction::Summon,
                    CommitteeAction::SpecialAttack,
                ];
                actions[(rng_val as usize) % actions.len()]
            }
            JudgePersonality::Strategic => {
                if boss_hp_frac < 0.3 {
                    CommitteeAction::Heal
                } else if boss_hp_frac < 0.6 {
                    CommitteeAction::Buff
                } else {
                    CommitteeAction::SpecialAttack
                }
            }
            JudgePersonality::Chaotic => {
                // Chaotic flips between extremes.
                if rng_val % 2 == 0 {
                    CommitteeAction::HeavyStrike
                } else {
                    CommitteeAction::Summon
                }
            }
        };
        self.current_vote = Some(vote);
        self.voting = true;
        vote
    }

    /// Take damage. Returns true if killed.
    pub fn take_damage(&mut self, amount: f32) -> bool {
        if !self.alive {
            return false;
        }
        self.hp = (self.hp - amount).max(0.0);
        if self.hp <= 0.0 {
            self.alive = false;
            true
        } else {
            false
        }
    }

    /// Convert to ghost (phase 2).
    pub fn become_ghost(&mut self) {
        self.ghost = true;
    }
}

/// State for the Committee Boss.
#[derive(Debug, Clone)]
pub struct CommitteeBossState {
    pub judges: Vec<Judge>,
    /// Whether dead judges vote as ghosts.
    pub ghost_voting: bool,
    /// Whether remaining judges have merged (phase 3).
    pub merged: bool,
    /// RNG state for random/chaotic votes.
    pub rng_state: u64,
}

impl CommitteeBossState {
    pub fn new() -> Self {
        let judges = vec![
            Judge::new(0, JudgePersonality::Aggressive, 200.0),
            Judge::new(1, JudgePersonality::Defensive, 200.0),
            Judge::new(2, JudgePersonality::Random, 200.0),
            Judge::new(3, JudgePersonality::Strategic, 200.0),
            Judge::new(4, JudgePersonality::Chaotic, 200.0),
        ];
        Self {
            judges,
            ghost_voting: false,
            merged: false,
            rng_state: 0xC0FF_EE42,
        }
    }

    fn next_rng(&mut self) -> u64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    /// Conduct a vote. All alive judges (and ghosts if enabled) vote.
    /// Returns the winning action by majority.
    pub fn conduct_vote(&mut self, boss_hp_frac: f32) -> CommitteeAction {
        let mut tallies: HashMap<CommitteeAction, u32> = HashMap::new();

        for judge in &mut self.judges {
            let can_vote = judge.alive || (judge.ghost && self.ghost_voting);
            if can_vote {
                let rng_val = self.rng_state;
                self.rng_state ^= self.rng_state << 13;
                self.rng_state ^= self.rng_state >> 7;
                self.rng_state ^= self.rng_state << 17;
                let vote = judge.cast_vote(boss_hp_frac, rng_val);
                *tallies.entry(vote).or_insert(0) += 1;
            }
        }

        // Find majority winner.
        tallies
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(action, _)| action)
            .unwrap_or(CommitteeAction::Attack)
    }

    /// Count alive judges.
    pub fn alive_count(&self) -> usize {
        self.judges.iter().filter(|j| j.alive).count()
    }

    /// Enable ghost voting (phase 2).
    pub fn enable_ghost_voting(&mut self) {
        self.ghost_voting = true;
        for judge in &mut self.judges {
            if !judge.alive {
                judge.become_ghost();
            }
        }
    }

    /// Merge remaining judges (phase 3). Returns combined HP.
    pub fn merge_judges(&mut self) -> f32 {
        let combined_hp: f32 = self.judges.iter().filter(|j| j.alive).map(|j| j.hp).sum();
        self.merged = true;
        combined_hp
    }
}

impl Default for CommitteeBossState {
    fn default() -> Self { Self::new() }
}

// ── Fibonacci Hydra ──────────────────────────────────────────────────────────────

/// A single head of the Fibonacci Hydra.
#[derive(Debug, Clone)]
pub struct HydraHead {
    pub id: u32,
    pub hp: f32,
    pub max_hp: f32,
    pub depth: u32,
    pub alive: bool,
    pub parent_id: Option<u32>,
}

impl HydraHead {
    pub fn new(id: u32, hp: f32, depth: u32, parent_id: Option<u32>) -> Self {
        Self {
            id,
            hp,
            max_hp: hp,
            depth,
            alive: true,
            parent_id,
        }
    }

    pub fn take_damage(&mut self, amount: f32) -> bool {
        self.hp = (self.hp - amount).max(0.0);
        if self.hp <= 0.0 {
            self.alive = false;
            true
        } else {
            false
        }
    }
}

/// State for the Fibonacci Hydra.
/// Starts as 1. On death, splits into 2 at 61.8% original HP each.
/// Max depth 5 (up to 32 instances). All share a damage pool.
#[derive(Debug, Clone)]
pub struct FibonacciHydraState {
    pub heads: Vec<HydraHead>,
    /// Maximum split depth.
    pub max_depth: u32,
    /// Next head ID to assign.
    pub next_id: u32,
    /// Golden ratio factor for child HP.
    pub split_hp_ratio: f32,
    /// Total damage dealt to all heads (shared pool).
    pub total_damage_pool: f32,
}

impl FibonacciHydraState {
    pub fn new(base_hp: f32) -> Self {
        Self {
            heads: vec![HydraHead::new(0, base_hp, 0, None)],
            max_depth: 5,
            next_id: 1,
            split_hp_ratio: 0.618,
            total_damage_pool: 0.0,
        }
    }

    /// Count alive heads.
    pub fn alive_count(&self) -> usize {
        self.heads.iter().filter(|h| h.alive).count()
    }

    /// Split a dead head into two children. Returns new head IDs if split occurred.
    pub fn try_split(&mut self, dead_head_id: u32) -> Option<(u32, u32)> {
        let (depth, parent_hp) = {
            let head = self.heads.iter().find(|h| h.id == dead_head_id)?;
            if head.alive || head.depth >= self.max_depth {
                return None;
            }
            (head.depth, head.max_hp)
        };

        let child_hp = parent_hp * self.split_hp_ratio;
        let id_a = self.next_id;
        let id_b = self.next_id + 1;
        self.next_id += 2;

        self.heads.push(HydraHead::new(id_a, child_hp, depth + 1, Some(dead_head_id)));
        self.heads.push(HydraHead::new(id_b, child_hp, depth + 1, Some(dead_head_id)));

        Some((id_a, id_b))
    }

    /// Apply damage to a specific head. Returns true if it died.
    pub fn damage_head(&mut self, head_id: u32, amount: f32) -> bool {
        self.total_damage_pool += amount;
        if let Some(head) = self.heads.iter_mut().find(|h| h.id == head_id) {
            head.take_damage(amount)
        } else {
            false
        }
    }

    /// Check if the hydra is fully defeated (no alive heads and no more splits possible).
    pub fn is_defeated(&self) -> bool {
        let alive = self.alive_count();
        if alive > 0 {
            return false;
        }
        // Check if any dead head can still split.
        !self.heads.iter().any(|h| !h.alive && h.depth < self.max_depth)
    }

    /// Maximum possible heads at full depth.
    pub fn max_possible_heads(&self) -> u32 {
        1 << self.max_depth // 2^max_depth = 32 at depth 5
    }
}

impl Default for FibonacciHydraState {
    fn default() -> Self { Self::new(1000.0) }
}

// ── Eigenstate Boss ──────────────────────────────────────────────────────────────

/// A quantum form the Eigenstate boss can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantumForm {
    Attack,
    Defense,
    /// Phase 2 adds a third form.
    Evasion,
}

/// State for the Eigenstate Boss.
/// Exists in superposition. Observing (targeting) collapses to one form.
/// Phase 2: 3 forms. Phase 3: entangled with a copy.
#[derive(Debug, Clone)]
pub struct EigenstateBossState {
    /// Available forms in the superposition.
    pub forms: Vec<QuantumForm>,
    /// Currently collapsed form (None = still in superposition).
    pub collapsed_form: Option<QuantumForm>,
    /// Whether the boss is being observed/targeted.
    pub observed: bool,
    /// Whether an entangled copy exists (phase 3).
    pub entangled: bool,
    /// HP of the entangled copy.
    pub entangled_hp: f32,
    /// Turns since last observation.
    pub turns_unobserved: u32,
    /// RNG for collapse.
    pub rng_state: u64,
}

impl EigenstateBossState {
    pub fn new() -> Self {
        Self {
            forms: vec![QuantumForm::Attack, QuantumForm::Defense],
            collapsed_form: None,
            observed: false,
            entangled: false,
            entangled_hp: 0.0,
            turns_unobserved: 0,
            rng_state: 0xABCD_0042,
        }
    }

    /// Observe the boss, collapsing it to a random form.
    pub fn observe(&mut self) -> QuantumForm {
        self.observed = true;
        self.turns_unobserved = 0;
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        let idx = (self.rng_state as usize) % self.forms.len();
        let form = self.forms[idx];
        self.collapsed_form = Some(form);
        form
    }

    /// End observation — return to superposition.
    pub fn unobserve(&mut self) {
        self.observed = false;
        self.collapsed_form = None;
        self.turns_unobserved += 1;
    }

    /// Add evasion form (phase 2).
    pub fn add_evasion_form(&mut self) {
        if !self.forms.contains(&QuantumForm::Evasion) {
            self.forms.push(QuantumForm::Evasion);
        }
    }

    /// Create entangled copy (phase 3). Returns initial copy HP.
    pub fn entangle(&mut self, boss_hp: f32) -> f32 {
        self.entangled = true;
        self.entangled_hp = boss_hp;
        self.entangled_hp
    }

    /// When entangled, damage to one is mirrored to the other.
    pub fn mirror_damage(&mut self, amount: f32) -> f32 {
        if self.entangled {
            self.entangled_hp = (self.entangled_hp - amount).max(0.0);
            amount // damage also applied to main boss
        } else {
            0.0
        }
    }

    /// Is the boss in superposition (not collapsed)?
    pub fn in_superposition(&self) -> bool {
        self.collapsed_form.is_none()
    }

    /// Get damage multiplier based on current form.
    pub fn damage_mult(&self) -> f32 {
        match self.collapsed_form {
            Some(QuantumForm::Attack) => 1.8,
            Some(QuantumForm::Defense) => 0.5,
            Some(QuantumForm::Evasion) => 1.0,
            None => 1.2, // superposition: moderate from both
        }
    }

    /// Get defense multiplier based on current form.
    pub fn defense_mult(&self) -> f32 {
        match self.collapsed_form {
            Some(QuantumForm::Attack) => 0.5,
            Some(QuantumForm::Defense) => 2.0,
            Some(QuantumForm::Evasion) => 0.8,
            None => 1.0,
        }
    }
}

impl Default for EigenstateBossState {
    fn default() -> Self { Self::new() }
}

// ── Ouroboros Boss ───────────────────────────────────────────────────────────────

/// State for the Ouroboros Boss.
/// Heals by dealing damage to player. Player heals by dealing damage to boss.
/// Bleed heals the bleeder. Healing damages the healer.
/// Phase 2: rules randomly swap back to normal for 2 turns.
#[derive(Debug, Clone)]
pub struct OuroborosBossState {
    /// Whether damage/heal semantics are currently reversed.
    pub reversed: bool,
    /// Countdown for temporary normal-rules window.
    pub normal_turns_remaining: u32,
    /// How many turns between rule swaps (phase 2).
    pub swap_interval: u32,
    /// Turn counter since last swap.
    pub turns_since_swap: u32,
    /// Total HP healed from dealing damage.
    pub total_self_heal: f32,
    /// Total HP player healed from dealing damage.
    pub total_player_heal: f32,
    /// Whether phase 2 random swapping is active.
    pub phase2_active: bool,
    /// RNG for swap timing.
    pub rng_state: u64,
}

impl OuroborosBossState {
    pub fn new() -> Self {
        Self {
            reversed: true, // starts reversed
            normal_turns_remaining: 0,
            swap_interval: 5,
            turns_since_swap: 0,
            total_self_heal: 0.0,
            total_player_heal: 0.0,
            phase2_active: false,
            rng_state: 0xB0B0_0007,
        }
    }

    /// Process a damage event under Ouroboros rules.
    /// Returns (actual_damage_to_target, heal_to_source).
    pub fn process_damage(&mut self, raw_damage: f32, is_boss_attacking: bool) -> (f32, f32) {
        if self.reversed {
            // Reversed: dealing damage heals the attacker.
            let heal = raw_damage * 0.5;
            if is_boss_attacking {
                self.total_self_heal += heal;
            } else {
                self.total_player_heal += heal;
            }
            (raw_damage, heal)
        } else {
            // Normal rules.
            (raw_damage, 0.0)
        }
    }

    /// Process a heal event under Ouroboros rules.
    /// Returns actual healing (negative means damage).
    pub fn process_heal(&self, raw_heal: f32) -> f32 {
        if self.reversed {
            -raw_heal // healing becomes damage
        } else {
            raw_heal
        }
    }

    /// Advance a turn. May trigger rule swap in phase 2.
    pub fn advance_turn(&mut self) -> bool {
        if self.normal_turns_remaining > 0 {
            self.normal_turns_remaining -= 1;
            if self.normal_turns_remaining == 0 {
                self.reversed = true;
                return true; // rules swapped back
            }
        }

        if self.phase2_active {
            self.turns_since_swap += 1;
            self.rng_state ^= self.rng_state << 13;
            self.rng_state ^= self.rng_state >> 7;
            self.rng_state ^= self.rng_state << 17;

            // Random chance to swap to normal for 2 turns.
            if self.turns_since_swap >= self.swap_interval
                && (self.rng_state % 3 == 0)
            {
                self.reversed = false;
                self.normal_turns_remaining = 2;
                self.turns_since_swap = 0;
                return true; // rules temporarily normal
            }
        }
        false
    }

    /// Activate phase 2 swapping.
    pub fn enter_phase2(&mut self) {
        self.phase2_active = true;
    }
}

impl Default for OuroborosBossState {
    fn default() -> Self { Self::new() }
}

// ── Algorithm Reborn Boss (Final Boss) ───────────────────────────────────────────

/// State for the Algorithm Reborn Boss.
/// Phase 1: learns player patterns (tracks action frequency).
/// Phase 2: counters player's most-used action type.
/// Phase 3: predicts next action via Markov chain.
/// Unique: no HP bar shown.
#[derive(Debug, Clone)]
pub struct AlgorithmRebornState {
    /// Frequency count of each player action type.
    pub action_frequency: HashMap<PlayerActionType, u32>,
    /// Markov chain: transition probabilities from action A to action B.
    /// Key: (from_action, to_action), Value: count.
    pub markov_transitions: HashMap<(PlayerActionType, PlayerActionType), u32>,
    /// Last player action (for Markov chain).
    pub last_action: Option<PlayerActionType>,
    /// The predicted next player action (phase 3).
    pub predicted_action: Option<PlayerActionType>,
    /// Whether the boss is in counter mode (phase 2+).
    pub counter_mode: bool,
    /// Whether Markov prediction is active (phase 3).
    pub markov_mode: bool,
    /// Visual degradation level [0, 1]. 0 = pristine, 1 = nearly dead.
    pub degradation: f32,
    /// Actual HP fraction (hidden from player).
    pub hidden_hp_frac: f32,
}

impl AlgorithmRebornState {
    pub fn new() -> Self {
        Self {
            action_frequency: HashMap::new(),
            markov_transitions: HashMap::new(),
            last_action: None,
            predicted_action: None,
            counter_mode: false,
            markov_mode: false,
            degradation: 0.0,
            hidden_hp_frac: 1.0,
        }
    }

    /// Record a player action and update the frequency and Markov tables.
    pub fn record_action(&mut self, action: PlayerActionType) {
        *self.action_frequency.entry(action.clone()).or_insert(0) += 1;

        if let Some(ref last) = self.last_action {
            *self
                .markov_transitions
                .entry((last.clone(), action.clone()))
                .or_insert(0) += 1;
        }
        self.last_action = Some(action);
    }

    /// Get the player's most frequently used action.
    pub fn most_used_action(&self) -> Option<PlayerActionType> {
        self.action_frequency
            .iter()
            .max_by_key(|&(_, count)| count)
            .map(|(action, _)| action.clone())
    }

    /// Get the counter-action for a given player action.
    pub fn counter_for(action: &PlayerActionType) -> PlayerActionType {
        match action {
            PlayerActionType::Attack => PlayerActionType::Defend,
            PlayerActionType::Defend => PlayerActionType::UseAbility(0), // piercing
            PlayerActionType::Heal => PlayerActionType::Attack,           // punish
            PlayerActionType::UseAbility(_) => PlayerActionType::Defend,
            PlayerActionType::UseItem => PlayerActionType::Attack,
            PlayerActionType::Move => PlayerActionType::UseAbility(1),    // AoE
            PlayerActionType::Wait => PlayerActionType::Attack,
        }
    }

    /// Predict the next player action using the Markov chain.
    pub fn predict_next(&mut self) -> Option<PlayerActionType> {
        let last = self.last_action.as_ref()?;
        let mut best_action = None;
        let mut best_count = 0u32;

        for ((from, to), &count) in &self.markov_transitions {
            if from == last && count > best_count {
                best_count = count;
                best_action = Some(to.clone());
            }
        }

        self.predicted_action = best_action.clone();
        best_action
    }

    /// Update visual degradation based on actual HP fraction.
    pub fn update_degradation(&mut self, hp_frac: f32) {
        self.hidden_hp_frac = hp_frac;
        self.degradation = 1.0 - hp_frac;
    }

    /// Enter phase 2: enable counter mode.
    pub fn enter_phase2(&mut self) {
        self.counter_mode = true;
    }

    /// Enter phase 3: enable Markov prediction.
    pub fn enter_phase3(&mut self) {
        self.markov_mode = true;
    }
}

impl Default for AlgorithmRebornState {
    fn default() -> Self { Self::new() }
}

// ── Chaos Weaver Boss ────────────────────────────────────────────────────────────

/// State for the Chaos Weaver Boss.
/// Manipulates game rules: element weakness chart, ability slots, damage display.
/// Phase 2: randomizes tile effects.
#[derive(Debug, Clone)]
pub struct ChaosWeaverState {
    /// Scrambled element weakness overrides. Key: element, Value: new weakness.
    pub weakness_overrides: HashMap<Element, Element>,
    /// Mapping of swapped ability slots (original -> new position).
    pub slot_swaps: HashMap<u32, u32>,
    /// Whether damage numbers are visually randomized.
    pub randomize_damage_display: bool,
    /// Whether tile effects are randomized (phase 2).
    pub randomize_tiles: bool,
    /// How many rule changes have occurred.
    pub chaos_count: u32,
    /// RNG state.
    pub rng_state: u64,
}

impl ChaosWeaverState {
    pub fn new() -> Self {
        Self {
            weakness_overrides: HashMap::new(),
            slot_swaps: HashMap::new(),
            randomize_damage_display: false,
            randomize_tiles: false,
            chaos_count: 0,
            rng_state: 0xC4A0_5555,
        }
    }

    fn next_rng(&mut self) -> u64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    /// Scramble the element weakness chart.
    pub fn scramble_weaknesses(&mut self) {
        let elements = [
            Element::Physical, Element::Fire, Element::Ice, Element::Lightning,
            Element::Void, Element::Entropy, Element::Gravity, Element::Radiant,
            Element::Shadow, Element::Temporal,
        ];

        self.weakness_overrides.clear();
        for &elem in &elements {
            let rng = self.next_rng();
            let target_idx = (rng as usize) % elements.len();
            self.weakness_overrides.insert(elem, elements[target_idx]);
        }
        self.chaos_count += 1;
    }

    /// Swap two ability slots.
    pub fn swap_ability_slots(&mut self, max_slots: u32) -> (u32, u32) {
        let a = (self.next_rng() as u32) % max_slots;
        let mut b = (self.next_rng() as u32) % max_slots;
        if b == a {
            b = (a + 1) % max_slots;
        }
        self.slot_swaps.insert(a, b);
        self.slot_swaps.insert(b, a);
        self.chaos_count += 1;
        (a, b)
    }

    /// Get a fake damage number for display (actual damage is correct).
    pub fn fake_damage_number(&mut self, _actual: f32) -> f32 {
        if self.randomize_damage_display {
            let rng = self.next_rng();
            (rng % 9999) as f32 + 1.0
        } else {
            _actual
        }
    }

    /// Enable damage display randomization.
    pub fn enable_damage_randomization(&mut self) {
        self.randomize_damage_display = true;
        self.chaos_count += 1;
    }

    /// Enable tile randomization (phase 2).
    pub fn enable_tile_randomization(&mut self) {
        self.randomize_tiles = true;
        self.chaos_count += 1;
    }

    /// Lookup what element a given element is now weak to (after scramble).
    pub fn effective_weakness(&self, element: Element) -> Element {
        self.weakness_overrides
            .get(&element)
            .copied()
            .unwrap_or(element)
    }
}

impl Default for ChaosWeaverState {
    fn default() -> Self { Self::new() }
}

// ── Void Serpent Boss ────────────────────────────────────────────────────────────

/// State for the Void Serpent Boss.
/// Consumes the arena: each turn, edge tiles become void (instant death).
/// Phase 2: void tiles spit projectiles.
/// Phase 3: serpent emerges for direct attacks.
#[derive(Debug, Clone)]
pub struct VoidSerpentState {
    /// Current arena width (shrinks over time).
    pub arena_width: u32,
    /// Current arena height.
    pub arena_height: u32,
    /// Original arena width.
    pub original_width: u32,
    /// Original arena height.
    pub original_height: u32,
    /// Which edges have been consumed (layers consumed from each side).
    pub consumed_north: u32,
    pub consumed_south: u32,
    pub consumed_east: u32,
    pub consumed_west: u32,
    /// Whether void tiles spit projectiles (phase 2).
    pub void_projectiles: bool,
    /// Number of projectiles per turn.
    pub projectiles_per_turn: u32,
    /// Whether the serpent has emerged (phase 3).
    pub serpent_emerged: bool,
    /// Serpent direct attack damage.
    pub serpent_attack_damage: f32,
    /// Turn counter.
    pub turn_count: u32,
}

impl VoidSerpentState {
    pub fn new(arena_w: u32, arena_h: u32) -> Self {
        Self {
            arena_width: arena_w,
            arena_height: arena_h,
            original_width: arena_w,
            original_height: arena_h,
            consumed_north: 0,
            consumed_south: 0,
            consumed_east: 0,
            consumed_west: 0,
            void_projectiles: false,
            projectiles_per_turn: 2,
            serpent_emerged: false,
            serpent_attack_damage: 50.0,
            turn_count: 0,
        }
    }

    /// Consume one edge row/column. Returns which direction was consumed.
    pub fn consume_edge(&mut self) -> Option<&'static str> {
        self.turn_count += 1;
        // Cycle through directions.
        let direction = self.turn_count % 4;
        match direction {
            0 => {
                if self.consumed_north < self.original_height / 2 {
                    self.consumed_north += 1;
                    self.arena_height = self.arena_height.saturating_sub(1);
                    Some("north")
                } else {
                    None
                }
            }
            1 => {
                if self.consumed_east < self.original_width / 2 {
                    self.consumed_east += 1;
                    self.arena_width = self.arena_width.saturating_sub(1);
                    Some("east")
                } else {
                    None
                }
            }
            2 => {
                if self.consumed_south < self.original_height / 2 {
                    self.consumed_south += 1;
                    self.arena_height = self.arena_height.saturating_sub(1);
                    Some("south")
                } else {
                    None
                }
            }
            3 => {
                if self.consumed_west < self.original_width / 2 {
                    self.consumed_west += 1;
                    self.arena_width = self.arena_width.saturating_sub(1);
                    Some("west")
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Remaining safe arena area.
    pub fn safe_area(&self) -> u32 {
        self.arena_width.saturating_mul(self.arena_height)
    }

    /// Fraction of arena remaining.
    pub fn arena_fraction(&self) -> f32 {
        let original = self.original_width * self.original_height;
        if original == 0 { return 0.0; }
        self.safe_area() as f32 / original as f32
    }

    /// Is a position within the safe zone?
    pub fn is_safe(&self, x: u32, y: u32) -> bool {
        x >= self.consumed_west
            && x < self.original_width - self.consumed_east
            && y >= self.consumed_north
            && y < self.original_height - self.consumed_south
    }

    /// Enable void projectiles (phase 2).
    pub fn enable_projectiles(&mut self) {
        self.void_projectiles = true;
    }

    /// Serpent emerges (phase 3).
    pub fn emerge_serpent(&mut self, damage: f32) {
        self.serpent_emerged = true;
        self.serpent_attack_damage = damage;
    }
}

impl Default for VoidSerpentState {
    fn default() -> Self { Self::new(20, 20) }
}

// ── Prime Factorial Boss ─────────────────────────────────────────────────────────

/// State for the Prime Factorial Boss.
/// HP is a large prime. Deals damage in factorial sequences.
/// Can only be damaged by prime-numbered damage values.
/// Phase 2: arithmetic puzzle mechanic.
#[derive(Debug, Clone)]
pub struct PrimeFactorialState {
    /// Current position in the factorial damage sequence.
    pub factorial_index: u32,
    /// Cached factorial values.
    pub factorial_cache: Vec<f32>,
    /// Whether the arithmetic puzzle mode is active (phase 2).
    pub puzzle_active: bool,
    /// Current puzzle target factors.
    pub puzzle_target_factors: Vec<u32>,
    /// How many puzzles solved.
    pub puzzles_solved: u32,
    /// RNG for puzzle generation.
    pub rng_state: u64,
}

impl PrimeFactorialState {
    pub fn new() -> Self {
        // Pre-compute factorials: 1!, 2!, 3!, 4!, 5!, 6!, 7!
        let factorials = vec![1.0, 2.0, 6.0, 24.0, 120.0, 720.0, 5040.0];
        Self {
            factorial_index: 0,
            factorial_cache: factorials,
            puzzle_active: false,
            puzzle_target_factors: Vec::new(),
            puzzles_solved: 0,
            rng_state: 0xA01E_0013,
        }
    }

    fn next_rng(&mut self) -> u64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    /// Get the next damage value in the factorial sequence.
    pub fn next_factorial_damage(&mut self) -> f32 {
        let idx = self.factorial_index as usize;
        let damage = if idx < self.factorial_cache.len() {
            self.factorial_cache[idx]
        } else {
            // Cap at last cached value.
            *self.factorial_cache.last().unwrap_or(&1.0)
        };
        self.factorial_index += 1;
        // Cycle back after reaching the end.
        if self.factorial_index as usize >= self.factorial_cache.len() {
            self.factorial_index = 0;
        }
        damage
    }

    /// Check if a damage value is prime.
    pub fn is_prime(n: u32) -> bool {
        if n < 2 {
            return false;
        }
        if n == 2 || n == 3 {
            return true;
        }
        if n % 2 == 0 || n % 3 == 0 {
            return false;
        }
        let mut i = 5u32;
        while i.saturating_mul(i) <= n {
            if n % i == 0 || n % (i + 2) == 0 {
                return false;
            }
            i += 6;
        }
        true
    }

    /// Filter incoming damage: only prime values deal damage.
    pub fn filter_damage(&self, raw_damage: f32) -> f32 {
        let rounded = raw_damage.round() as u32;
        if Self::is_prime(rounded) {
            raw_damage
        } else {
            0.0 // non-prime damage is nullified
        }
    }

    /// Generate an arithmetic puzzle (phase 2).
    /// Player must deal damage that factors to these specific numbers.
    pub fn generate_puzzle(&mut self) -> Vec<u32> {
        let small_primes = [2, 3, 5, 7, 11, 13];
        let count = 2 + (self.puzzles_solved.min(3) as usize); // 2-5 factors
        let mut factors = Vec::new();
        for _ in 0..count {
            let idx = (self.next_rng() as usize) % small_primes.len();
            factors.push(small_primes[idx]);
        }
        self.puzzle_target_factors = factors.clone();
        self.puzzle_active = true;
        factors
    }

    /// Check if player's damage solves the current puzzle.
    pub fn check_puzzle_solution(&mut self, damage: u32) -> bool {
        if !self.puzzle_active || self.puzzle_target_factors.is_empty() {
            return false;
        }

        // Check if damage equals the product of target factors.
        let target: u32 = self.puzzle_target_factors.iter().product();
        if damage == target {
            self.puzzles_solved += 1;
            self.puzzle_active = false;
            self.puzzle_target_factors.clear();
            true
        } else {
            false
        }
    }

    /// Get a large prime for boss HP based on tier.
    pub fn prime_hp(tier: u32) -> f32 {
        match tier {
            1 => 997.0,
            2 => 4999.0,
            3 => 10_007.0,
            4 => 49_999.0,  // not actually prime, but close
            _ => 99_991.0,
        }
    }
}

impl Default for PrimeFactorialState {
    fn default() -> Self { Self::new() }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss Mechanic State (union of all boss-specific states)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Union of all boss-specific mechanic states.
#[derive(Debug, Clone)]
pub enum BossMechanicState {
    Mirror(MirrorBossState),
    Null(NullBossState),
    Committee(CommitteeBossState),
    FibonacciHydra(FibonacciHydraState),
    Eigenstate(EigenstateBossState),
    Ouroboros(OuroborosBossState),
    AlgorithmReborn(AlgorithmRebornState),
    ChaosWeaver(ChaosWeaverState),
    VoidSerpent(VoidSerpentState),
    PrimeFactorial(PrimeFactorialState),
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss Events
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Events emitted during a boss encounter.
#[derive(Debug, Clone)]
pub enum BossEvent {
    /// Boss entered a new phase.
    PhaseChange {
        new_phase: u32,
        transition: PhaseTransition,
        dialogue: String,
    },
    /// Boss used a special ability.
    SpecialAbility {
        ability: SpecialAbility,
        description: String,
    },
    /// Boss speaks dialogue.
    Dialogue(String),
    /// Music should change.
    MusicChange(MusicType),
    /// Arena was modified.
    ArenaModification(ArenaMod),
    /// Boss is defeated — victory rewards.
    VictoryReward {
        boss_type: BossType,
        loot: Vec<BossLootEntry>,
        xp_reward: u64,
    },
    /// UI element erased (Null boss).
    UiErased(EraseTarget),
    /// UI elements restored (Null boss death).
    UiRestored(Vec<EraseTarget>),
    /// Hydra split into new heads.
    HydraSplit { parent_id: u32, child_ids: (u32, u32) },
    /// Quantum form collapsed.
    QuantumCollapse(QuantumForm),
    /// Rules changed (Ouroboros/ChaosWeaver).
    RulesChanged(String),
    /// Ability locked (Null boss).
    AbilityLocked(u32),
    /// Committee vote result.
    CommitteeVoteResult(CommitteeAction),
    /// Arena shrunk (Void Serpent).
    ArenaShrunk { direction: String, remaining_fraction: f32 },
    /// Puzzle generated (PrimeFactorial).
    PuzzleGenerated(Vec<u32>),
    /// Puzzle solved.
    PuzzleSolved,
    /// Boss defeated.
    BossDefeated(BossType),
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss Encounter
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A live boss encounter.
#[derive(Clone)]
pub struct BossEncounter {
    /// The boss entity.
    pub entity: AmorphousEntity,
    /// Boss profile data.
    pub profile: BossProfile,
    /// Phase controller.
    pub phase_controller: BossPhaseController,
    /// Boss-specific mechanic state.
    pub mechanic_state: BossMechanicState,
    /// Turn counter.
    pub turn_count: u32,
    /// Cumulative damage log.
    pub damage_log: Vec<f32>,
    /// Floor this encounter is on (for scaling).
    pub floor: u32,
    /// Whether the encounter is finished.
    pub finished: bool,
    /// Combat stats for the boss.
    pub boss_stats: CombatStats,
}

impl BossEncounter {
    /// Drive the encounter forward. Processes AI, checks phases, applies mechanics.
    pub fn update(&mut self, dt: f32, player_actions: &[RecordedAction]) -> Vec<BossEvent> {
        let mut events = Vec::new();

        if self.finished {
            return events;
        }

        // Record player actions for bosses that care.
        for action in player_actions {
            self.record_player_action(action.clone());
        }

        // Update phase transition animation.
        if self.phase_controller.is_transitioning() {
            self.phase_controller.update_transition(dt);
            return events; // skip AI during transitions
        }

        // Check for phase transition.
        let hp_frac = self.entity.hp_frac();
        if let Some(phase) = self.phase_controller.check_transition(hp_frac) {
            let phase_num = phase.phase_number;
            let transition = phase.transition_animation;
            let dialogue = phase.dialogue_on_enter.clone();

            events.push(BossEvent::PhaseChange {
                new_phase: phase_num,
                transition,
                dialogue: dialogue.clone(),
            });

            if !dialogue.is_empty() {
                events.push(BossEvent::Dialogue(dialogue));
            }

            // Trigger phase-specific state changes.
            self.on_phase_enter(phase_num, &mut events);
        }

        // Run boss-specific mechanic logic.
        self.tick_mechanic(dt, &mut events);

        // Check for death.
        if self.entity.is_dead() {
            self.on_death(&mut events);
        }

        // Update entity visuals.
        self.entity.tick(dt, self.entity.age);

        self.turn_count += 1;
        events
    }

    /// Record a player action for boss mechanics.
    fn record_player_action(&mut self, action: RecordedAction) {
        match &mut self.mechanic_state {
            BossMechanicState::Mirror(state) => {
                state.record_action(action);
            }
            BossMechanicState::AlgorithmReborn(state) => {
                state.record_action(action.action_type);
            }
            _ => {}
        }
    }

    /// Handle phase-specific state changes when entering a new phase.
    fn on_phase_enter(&mut self, phase_num: u32, events: &mut Vec<BossEvent>) {
        match &mut self.mechanic_state {
            BossMechanicState::Mirror(state) => {
                if phase_num == 2 {
                    state.enter_phase2();
                } else if phase_num == 3 {
                    state.enter_phase3();
                }
            }
            BossMechanicState::Null(_state) => {
                // Null boss phases are handled in tick_mechanic.
            }
            BossMechanicState::Committee(state) => {
                if phase_num == 2 {
                    state.enable_ghost_voting();
                } else if phase_num == 3 {
                    let combined_hp = state.merge_judges();
                    events.push(BossEvent::Dialogue(
                        format!("The judges merge! Combined HP: {:.0}", combined_hp),
                    ));
                }
            }
            BossMechanicState::Eigenstate(state) => {
                if phase_num == 2 {
                    state.add_evasion_form();
                } else if phase_num == 3 {
                    let copy_hp = state.entangle(self.entity.hp);
                    events.push(BossEvent::SpecialAbility {
                        ability: SpecialAbility::Entangle,
                        description: format!("Entangled copy spawned with {:.0} HP", copy_hp),
                    });
                }
            }
            BossMechanicState::Ouroboros(state) => {
                if phase_num == 2 {
                    state.enter_phase2();
                    events.push(BossEvent::RulesChanged(
                        "The rules of damage and healing flicker...".into(),
                    ));
                }
            }
            BossMechanicState::AlgorithmReborn(state) => {
                if phase_num == 2 {
                    state.enter_phase2();
                    events.push(BossEvent::Dialogue(
                        "I have studied your every move.".into(),
                    ));
                } else if phase_num == 3 {
                    state.enter_phase3();
                    events.push(BossEvent::Dialogue(
                        "I know what you will do before you do it.".into(),
                    ));
                }
            }
            BossMechanicState::ChaosWeaver(state) => {
                if phase_num == 2 {
                    state.enable_tile_randomization();
                    events.push(BossEvent::ArenaModification(
                        ArenaMod::HazardTiles { element: Element::Entropy, count: 10 },
                    ));
                }
            }
            BossMechanicState::VoidSerpent(state) => {
                if phase_num == 2 {
                    state.enable_projectiles();
                } else if phase_num == 3 {
                    state.emerge_serpent(80.0);
                    events.push(BossEvent::Dialogue(
                        "The Void Serpent emerges from the darkness!".into(),
                    ));
                }
            }
            BossMechanicState::PrimeFactorial(state) => {
                if phase_num == 2 {
                    let factors = state.generate_puzzle();
                    events.push(BossEvent::PuzzleGenerated(factors));
                }
            }
            _ => {}
        }
    }

    /// Tick boss-specific mechanic logic each turn.
    fn tick_mechanic(&mut self, _dt: f32, events: &mut Vec<BossEvent>) {
        let phase_num = self.phase_controller.current_phase_number();

        match &mut self.mechanic_state {
            BossMechanicState::Mirror(state) => {
                if let Some(action) = state.get_mirrored_action(self.turn_count) {
                    events.push(BossEvent::SpecialAbility {
                        ability: SpecialAbility::MirrorCopy { depth: state.buffer_depth },
                        description: format!("Mirror copies: {:?}", action.action_type),
                    });
                }
            }
            BossMechanicState::Null(state) => {
                match phase_num {
                    1 => {
                        let erased = state.erase_buffs(3); // assume 3 active buffs
                        if erased > 0 {
                            events.push(BossEvent::SpecialAbility {
                                ability: SpecialAbility::Erase(EraseTarget::PlayerBuffs),
                                description: format!("Erased {} buff(s)", erased),
                            });
                        }
                    }
                    2 => {
                        let target = state.erase_ui_element();
                        events.push(BossEvent::UiErased(target));
                    }
                    3 => {
                        if let Some(slot) = state.lock_random_ability(6) {
                            events.push(BossEvent::AbilityLocked(slot));
                        }
                    }
                    _ => {}
                }
            }
            BossMechanicState::Committee(state) => {
                if !state.merged {
                    let hp_frac = self.entity.hp_frac();
                    let action = state.conduct_vote(hp_frac);
                    events.push(BossEvent::CommitteeVoteResult(action));
                }
            }
            BossMechanicState::FibonacciHydra(state) => {
                // Check for dead heads that can split.
                let dead_heads: Vec<u32> = state
                    .heads
                    .iter()
                    .filter(|h| !h.alive && h.depth < state.max_depth)
                    .map(|h| h.id)
                    .collect();

                for head_id in dead_heads {
                    if let Some((a, b)) = state.try_split(head_id) {
                        events.push(BossEvent::HydraSplit {
                            parent_id: head_id,
                            child_ids: (a, b),
                        });
                    }
                }

                if state.is_defeated() {
                    self.entity.hp = 0.0; // ensure entity death triggers
                }
            }
            BossMechanicState::Eigenstate(state) => {
                // Auto-unobserve after a turn.
                if state.observed {
                    state.unobserve();
                }
            }
            BossMechanicState::Ouroboros(state) => {
                let swapped = state.advance_turn();
                if swapped {
                    let msg = if state.reversed {
                        "The rules twist back... damage heals, healing harms."
                    } else {
                        "For a brief moment, the rules return to normal..."
                    };
                    events.push(BossEvent::RulesChanged(msg.into()));
                }
            }
            BossMechanicState::AlgorithmReborn(state) => {
                let hp_frac = self.entity.hp_frac();
                state.update_degradation(hp_frac);

                if state.markov_mode {
                    if let Some(predicted) = state.predict_next() {
                        let counter = AlgorithmRebornState::counter_for(&predicted);
                        events.push(BossEvent::SpecialAbility {
                            ability: SpecialAbility::MarkovPredict,
                            description: format!(
                                "Algorithm predicts {:?}, counters with {:?}",
                                predicted, counter
                            ),
                        });
                    }
                } else if state.counter_mode {
                    if let Some(most_used) = state.most_used_action() {
                        let counter = AlgorithmRebornState::counter_for(&most_used);
                        events.push(BossEvent::SpecialAbility {
                            ability: SpecialAbility::CounterPredict,
                            description: format!(
                                "Algorithm counters your favorite: {:?} with {:?}",
                                most_used, counter
                            ),
                        });
                    }
                }
            }
            BossMechanicState::ChaosWeaver(state) => {
                // Scramble weaknesses every 3 turns.
                if self.turn_count % 3 == 0 {
                    state.scramble_weaknesses();
                    events.push(BossEvent::RulesChanged(
                        "Element weaknesses have been scrambled!".into(),
                    ));
                }
                // Swap ability slots every 5 turns.
                if self.turn_count % 5 == 0 {
                    let (a, b) = state.swap_ability_slots(6);
                    events.push(BossEvent::RulesChanged(
                        format!("Ability slots {} and {} swapped!", a, b),
                    ));
                }
            }
            BossMechanicState::VoidSerpent(state) => {
                if let Some(direction) = state.consume_edge() {
                    let frac = state.arena_fraction();
                    events.push(BossEvent::ArenaShrunk {
                        direction: direction.to_string(),
                        remaining_fraction: frac,
                    });
                }
            }
            BossMechanicState::PrimeFactorial(state) => {
                let damage = state.next_factorial_damage();
                events.push(BossEvent::SpecialAbility {
                    ability: SpecialAbility::FactorialStrike {
                        sequence_index: state.factorial_index,
                    },
                    description: format!("Factorial strike: {:.0} damage!", damage),
                });
            }
        }
    }

    /// Handle boss death.
    fn on_death(&mut self, events: &mut Vec<BossEvent>) {
        self.finished = true;

        // Boss-specific death effects.
        if let BossMechanicState::Null(state) = &mut self.mechanic_state {
            let restored = state.restore_all();
            events.push(BossEvent::UiRestored(restored));
        }

        events.push(BossEvent::BossDefeated(self.profile.boss_type));

        // Loot reward.
        events.push(BossEvent::VictoryReward {
            boss_type: self.profile.boss_type,
            loot: self.profile.loot_table.clone(),
            xp_reward: (self.profile.tier as u64) * 500 + (self.floor as u64) * 100,
        });
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Boss Encounter Manager
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Factory and manager for boss encounters.
pub struct BossEncounterManager;

impl BossEncounterManager {
    /// Create a new boss encounter.
    pub fn start_encounter(
        boss_type: BossType,
        floor: u32,
        player_stats: &CombatStats,
    ) -> BossEncounter {
        let profile = Self::build_profile(boss_type);
        let scaled_hp = profile.scaled_hp(floor);
        let scaled_damage = profile.scaled_damage(floor);

        let mut entity = AmorphousEntity::new(profile.name.clone(), glam::Vec3::ZERO);
        entity.hp = scaled_hp;
        entity.max_hp = scaled_hp;

        let mut boss_stats = CombatStats {
            attack: scaled_damage,
            max_hp: scaled_hp,
            hp: scaled_hp,
            level: floor,
            crit_chance: 0.1,
            crit_mult: 2.5,
            ..CombatStats::default()
        };

        // Scale boss slightly based on player stats.
        boss_stats.armor = player_stats.attack * 0.3;

        let phase_controller = BossPhaseController::new(profile.phases.clone());
        let mechanic_state = Self::create_mechanic_state(boss_type, scaled_hp);

        BossEncounter {
            entity,
            profile,
            phase_controller,
            mechanic_state,
            turn_count: 0,
            damage_log: Vec::new(),
            floor,
            finished: false,
            boss_stats,
        }
    }

    /// Build the full profile for a boss type.
    fn build_profile(boss_type: BossType) -> BossProfile {
        match boss_type {
            BossType::Mirror => Self::mirror_profile(),
            BossType::Null => Self::null_profile(),
            BossType::Committee => Self::committee_profile(),
            BossType::FibonacciHydra => Self::fibonacci_hydra_profile(),
            BossType::Eigenstate => Self::eigenstate_profile(),
            BossType::Ouroboros => Self::ouroboros_profile(),
            BossType::AlgorithmReborn => Self::algorithm_reborn_profile(),
            BossType::ChaosWeaver => Self::chaos_weaver_profile(),
            BossType::VoidSerpent => Self::void_serpent_profile(),
            BossType::PrimeFactorial => Self::prime_factorial_profile(),
        }
    }

    /// Create the appropriate mechanic state for a boss type.
    fn create_mechanic_state(boss_type: BossType, base_hp: f32) -> BossMechanicState {
        match boss_type {
            BossType::Mirror => BossMechanicState::Mirror(MirrorBossState::new()),
            BossType::Null => BossMechanicState::Null(NullBossState::new()),
            BossType::Committee => BossMechanicState::Committee(CommitteeBossState::new()),
            BossType::FibonacciHydra => {
                BossMechanicState::FibonacciHydra(FibonacciHydraState::new(base_hp))
            }
            BossType::Eigenstate => BossMechanicState::Eigenstate(EigenstateBossState::new()),
            BossType::Ouroboros => BossMechanicState::Ouroboros(OuroborosBossState::new()),
            BossType::AlgorithmReborn => {
                BossMechanicState::AlgorithmReborn(AlgorithmRebornState::new())
            }
            BossType::ChaosWeaver => BossMechanicState::ChaosWeaver(ChaosWeaverState::new()),
            BossType::VoidSerpent => {
                BossMechanicState::VoidSerpent(VoidSerpentState::new(20, 20))
            }
            BossType::PrimeFactorial => {
                BossMechanicState::PrimeFactorial(PrimeFactorialState::new())
            }
        }
    }

    // ── Individual boss profiles ──────────────────────────────────────────────

    fn mirror_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::Mirror,
            name: "The Mirror".into(),
            title: "Reflection of Self".into(),
            hp_base: 800.0,
            damage_base: 15.0,
            tier: 1,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Standard)
                    .with_ability(SpecialAbility::MirrorCopy { depth: 3 })
                    .with_dialogue("I am you, delayed."),
                BossPhase::new(2, 0.5)
                    .with_behavior(BehaviorPattern::Calculated)
                    .with_damage(1.3)
                    .with_ability(SpecialAbility::MirrorCopy { depth: 3 })
                    .with_transition(PhaseTransition::GlyphReorganize)
                    .with_dialogue("Now I wear your strength as well."),
                BossPhase::new(3, 0.25)
                    .with_behavior(BehaviorPattern::Aggressive)
                    .with_speed(1.5)
                    .with_damage(1.5)
                    .with_ability(SpecialAbility::MirrorCopy { depth: 3 })
                    .with_transition(PhaseTransition::PowerUp)
                    .with_dialogue("We act as one. There is no delay."),
            ],
            special_mechanics: vec![
                "Copies player abilities with 1-turn delay".into(),
                "Phase 2: copies player stats".into(),
                "Phase 3: simultaneous mirrored actions".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Shard of Reflection".into(),
                drop_chance: 1.0,
                min_quantity: 1,
                max_quantity: 1,
            }],
            music_type: MusicType::Ominous,
            arena_mods: vec![ArenaMod::None],
            resistance: ResistanceProfile::boss_resist(),
        }
    }

    fn null_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::Null,
            name: "The Null".into(),
            title: "The Eraser of Meaning".into(),
            hp_base: 1200.0,
            damage_base: 12.0,
            tier: 2,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Passive)
                    .with_ability(SpecialAbility::Erase(EraseTarget::PlayerBuffs))
                    .with_dialogue("Let us subtract."),
                BossPhase::new(2, 0.6)
                    .with_behavior(BehaviorPattern::Standard)
                    .with_damage(1.2)
                    .with_ability(SpecialAbility::Erase(EraseTarget::HpBar))
                    .with_transition(PhaseTransition::Dissolve)
                    .with_dialogue("Your interface is a luxury I revoke."),
                BossPhase::new(3, 0.3)
                    .with_behavior(BehaviorPattern::Aggressive)
                    .with_speed(1.3)
                    .with_damage(1.4)
                    .with_ability(SpecialAbility::LockAbility)
                    .with_transition(PhaseTransition::Dissolve)
                    .with_dialogue("Even your skills are expendable."),
            ],
            special_mechanics: vec![
                "Phase 1: erases player buffs".into(),
                "Phase 2: erases UI elements".into(),
                "Phase 3: locks random abilities each turn".into(),
                "On death: all erased elements restored".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Void Fragment".into(),
                drop_chance: 1.0,
                min_quantity: 1,
                max_quantity: 2,
            }],
            music_type: MusicType::Silence,
            arena_mods: vec![ArenaMod::DarkenVision { radius_reduction: 3.0 }],
            resistance: ResistanceProfile::void_entity(),
        }
    }

    fn committee_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::Committee,
            name: "The Committee".into(),
            title: "Democracy of Violence".into(),
            hp_base: 1500.0,
            damage_base: 18.0,
            tier: 2,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Standard)
                    .with_ability(SpecialAbility::CommitteeVote)
                    .with_dialogue("The vote is called. All in favor?"),
                BossPhase::new(2, 0.5)
                    .with_behavior(BehaviorPattern::Calculated)
                    .with_damage(1.2)
                    .with_ability(SpecialAbility::CommitteeVote)
                    .with_transition(PhaseTransition::GlyphReorganize)
                    .with_dialogue("The dead still have a voice here."),
                BossPhase::new(3, 0.2)
                    .with_behavior(BehaviorPattern::Berserk)
                    .with_speed(1.4)
                    .with_damage(1.8)
                    .with_ability(SpecialAbility::CommitteeVote)
                    .with_transition(PhaseTransition::Merge)
                    .with_dialogue("We are ONE. The motion carries unanimously."),
            ],
            special_mechanics: vec![
                "5 judges vote on each action".into(),
                "Kill judges to change vote balance".into(),
                "Phase 2: dead judges vote as ghosts".into(),
                "Phase 3: remaining judges merge".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Gavel of Authority".into(),
                drop_chance: 0.8,
                min_quantity: 1,
                max_quantity: 1,
            }],
            music_type: MusicType::Orchestral,
            arena_mods: vec![ArenaMod::None],
            resistance: ResistanceProfile::neutral(),
        }
    }

    fn fibonacci_hydra_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::FibonacciHydra,
            name: "Fibonacci Hydra".into(),
            title: "The Golden Recursion".into(),
            hp_base: 1000.0,
            damage_base: 14.0,
            tier: 3,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Standard)
                    .with_ability(SpecialAbility::FibonacciSplit)
                    .with_dialogue("Cut one, and two shall grow."),
                BossPhase::new(2, 0.618)
                    .with_behavior(BehaviorPattern::Aggressive)
                    .with_speed(1.2)
                    .with_damage(1.3)
                    .with_ability(SpecialAbility::FibonacciSplit)
                    .with_transition(PhaseTransition::Split)
                    .with_dialogue("The golden ratio demands expansion!"),
                BossPhase::new(3, 0.3)
                    .with_behavior(BehaviorPattern::Berserk)
                    .with_speed(1.5)
                    .with_damage(1.5)
                    .with_ability(SpecialAbility::FibonacciSplit)
                    .with_transition(PhaseTransition::Split)
                    .with_dialogue("We are legion! 1, 1, 2, 3, 5, 8, 13..."),
            ],
            special_mechanics: vec![
                "Splits into 2 on death at 61.8% HP each".into(),
                "Max depth 5 (up to 32 heads)".into(),
                "All heads share a damage pool".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Golden Spiral Shell".into(),
                drop_chance: 1.0,
                min_quantity: 1,
                max_quantity: 1,
            }],
            music_type: MusicType::Frenetic,
            arena_mods: vec![ArenaMod::None],
            resistance: ResistanceProfile::neutral(),
        }
    }

    fn eigenstate_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::Eigenstate,
            name: "The Eigenstate".into(),
            title: "Collapsed Possibility".into(),
            hp_base: 1100.0,
            damage_base: 20.0,
            tier: 3,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Erratic)
                    .with_ability(SpecialAbility::QuantumCollapse)
                    .with_dialogue("Observe me and I become certain. Look away and I am everything."),
                BossPhase::new(2, 0.5)
                    .with_behavior(BehaviorPattern::Calculated)
                    .with_speed(1.3)
                    .with_damage(1.4)
                    .with_ability(SpecialAbility::QuantumCollapse)
                    .with_transition(PhaseTransition::Dissolve)
                    .with_dialogue("A third possibility emerges."),
                BossPhase::new(3, 0.25)
                    .with_behavior(BehaviorPattern::Aggressive)
                    .with_speed(1.5)
                    .with_damage(1.6)
                    .with_ability(SpecialAbility::Entangle)
                    .with_transition(PhaseTransition::Split)
                    .with_dialogue("We are entangled now. Harm one, harm both."),
            ],
            special_mechanics: vec![
                "Exists in superposition of Attack/Defense".into(),
                "Targeting collapses to one form".into(),
                "Phase 2: 3 forms (adds Evasion)".into(),
                "Phase 3: entangled copy mirrors all damage".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Quantum Shard".into(),
                drop_chance: 1.0,
                min_quantity: 1,
                max_quantity: 1,
            }],
            music_type: MusicType::Glitch,
            arena_mods: vec![ArenaMod::None],
            resistance: ResistanceProfile::neutral(),
        }
    }

    fn ouroboros_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::Ouroboros,
            name: "Ouroboros".into(),
            title: "The Serpent That Devours".into(),
            hp_base: 1300.0,
            damage_base: 16.0,
            tier: 3,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Standard)
                    .with_ability(SpecialAbility::ReverseSemantic)
                    .with_dialogue("What heals you, harms me. What harms you, heals me. Or is it the other way?"),
                BossPhase::new(2, 0.5)
                    .with_behavior(BehaviorPattern::Erratic)
                    .with_speed(1.2)
                    .with_damage(1.3)
                    .with_ability(SpecialAbility::ReverseSemantic)
                    .with_transition(PhaseTransition::GlyphReorganize)
                    .with_dialogue("The rules flicker between truth and lies."),
            ],
            special_mechanics: vec![
                "Damage/heal semantics reversed".into(),
                "Boss heals by dealing damage".into(),
                "Player heals by dealing damage to boss".into(),
                "Phase 2: rules randomly swap to normal for 2 turns".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Ouroboros Ring".into(),
                drop_chance: 1.0,
                min_quantity: 1,
                max_quantity: 1,
            }],
            music_type: MusicType::Reversed,
            arena_mods: vec![ArenaMod::None],
            resistance: ResistanceProfile::boss_resist(),
        }
    }

    fn algorithm_reborn_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::AlgorithmReborn,
            name: "Algorithm Reborn".into(),
            title: "Final Proof".into(),
            hp_base: 3000.0,
            damage_base: 25.0,
            tier: 5,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Standard)
                    .with_ability(SpecialAbility::None)
                    .with_dialogue("Begin the final computation."),
                BossPhase::new(2, 0.65)
                    .with_behavior(BehaviorPattern::Calculated)
                    .with_speed(1.2)
                    .with_damage(1.4)
                    .with_ability(SpecialAbility::CounterPredict)
                    .with_transition(PhaseTransition::PowerUp)
                    .with_dialogue("I have studied your every move."),
                BossPhase::new(3, 0.3)
                    .with_behavior(BehaviorPattern::Calculated)
                    .with_speed(1.5)
                    .with_damage(1.8)
                    .with_ability(SpecialAbility::MarkovPredict)
                    .with_transition(PhaseTransition::GlyphReorganize)
                    .with_dialogue("I know what you will do before you do it."),
            ],
            special_mechanics: vec![
                "Tracks player action frequency".into(),
                "Phase 2: counters most-used action".into(),
                "Phase 3: Markov-chain prediction of next action".into(),
                "No HP bar: judge by visual degradation".into(),
            ],
            loot_table: vec![
                BossLootEntry {
                    item_name: "Core of the Algorithm".into(),
                    drop_chance: 1.0,
                    min_quantity: 1,
                    max_quantity: 1,
                },
                BossLootEntry {
                    item_name: "Proof of Completion".into(),
                    drop_chance: 1.0,
                    min_quantity: 1,
                    max_quantity: 1,
                },
            ],
            music_type: MusicType::Algorithmic,
            arena_mods: vec![
                ArenaMod::HazardTiles { element: Element::Entropy, count: 5 },
            ],
            resistance: ResistanceProfile::boss_resist(),
        }
    }

    fn chaos_weaver_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::ChaosWeaver,
            name: "Chaos Weaver".into(),
            title: "Unraveler of Rules".into(),
            hp_base: 1400.0,
            damage_base: 17.0,
            tier: 4,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Erratic)
                    .with_ability(SpecialAbility::RuleRandomize)
                    .with_dialogue("The rules are merely suggestions."),
                BossPhase::new(2, 0.45)
                    .with_behavior(BehaviorPattern::Erratic)
                    .with_speed(1.3)
                    .with_damage(1.5)
                    .with_ability(SpecialAbility::RuleRandomize)
                    .with_transition(PhaseTransition::Teleport)
                    .with_dialogue("Even the ground beneath you obeys me now."),
            ],
            special_mechanics: vec![
                "Scrambles element weakness chart".into(),
                "Swaps player ability slots".into(),
                "Randomizes damage number display".into(),
                "Phase 2: randomizes tile effects".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Thread of Chaos".into(),
                drop_chance: 1.0,
                min_quantity: 1,
                max_quantity: 3,
            }],
            music_type: MusicType::Chaotic,
            arena_mods: vec![ArenaMod::None],
            resistance: ResistanceProfile::chaos_rift(),
        }
    }

    fn void_serpent_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::VoidSerpent,
            name: "Void Serpent".into(),
            title: "Consumer of Arenas".into(),
            hp_base: 1600.0,
            damage_base: 20.0,
            tier: 4,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Passive)
                    .with_ability(SpecialAbility::ConsumeArena { columns: 1 })
                    .with_dialogue("The void hungers."),
                BossPhase::new(2, 0.55)
                    .with_behavior(BehaviorPattern::Standard)
                    .with_speed(1.2)
                    .with_damage(1.3)
                    .with_ability(SpecialAbility::ConsumeArena { columns: 2 })
                    .with_transition(PhaseTransition::Dissolve)
                    .with_dialogue("The void spits back what it cannot digest."),
                BossPhase::new(3, 0.25)
                    .with_behavior(BehaviorPattern::Berserk)
                    .with_speed(1.5)
                    .with_damage(1.8)
                    .with_ability(SpecialAbility::ConsumeArena { columns: 3 })
                    .with_transition(PhaseTransition::Teleport)
                    .with_dialogue("I emerge from the nothing!"),
            ],
            special_mechanics: vec![
                "Each turn, edge tiles become void".into(),
                "Phase 2: void tiles spit projectiles".into(),
                "Phase 3: serpent emerges for direct attacks".into(),
                "Arena shrinks continuously".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Void Scale".into(),
                drop_chance: 1.0,
                min_quantity: 1,
                max_quantity: 2,
            }],
            music_type: MusicType::MinimalDrone,
            arena_mods: vec![
                ArenaMod::ShrinkEdges { rate_per_turn: 1 },
                ArenaMod::DarkenVision { radius_reduction: 2.0 },
            ],
            resistance: ResistanceProfile::void_entity(),
        }
    }

    fn prime_factorial_profile() -> BossProfile {
        BossProfile {
            boss_type: BossType::PrimeFactorial,
            name: "Prime Factorial".into(),
            title: "The Indivisible Explosion".into(),
            hp_base: PrimeFactorialState::prime_hp(4),
            damage_base: 22.0,
            tier: 4,
            phases: vec![
                BossPhase::new(1, 1.0)
                    .with_behavior(BehaviorPattern::Standard)
                    .with_ability(SpecialAbility::FactorialStrike { sequence_index: 0 })
                    .with_dialogue("Only primes can wound me."),
                BossPhase::new(2, 0.5)
                    .with_behavior(BehaviorPattern::Calculated)
                    .with_speed(1.2)
                    .with_damage(1.5)
                    .with_ability(SpecialAbility::ArithmeticPuzzle {
                        target_factors: vec![2, 3, 5],
                    })
                    .with_transition(PhaseTransition::PowerUp)
                    .with_dialogue("Solve the equation or perish in the factorial!"),
            ],
            special_mechanics: vec![
                "HP is a large prime number".into(),
                "Damage in factorial sequences: 1, 2, 6, 24, 120...".into(),
                "Only prime-numbered damage hurts this boss".into(),
                "Phase 2: arithmetic puzzle mechanic".into(),
            ],
            loot_table: vec![BossLootEntry {
                item_name: "Prime Gemstone".into(),
                drop_chance: 1.0,
                min_quantity: 1,
                max_quantity: 1,
            }],
            music_type: MusicType::Crescendo,
            arena_mods: vec![ArenaMod::None],
            resistance: ResistanceProfile::neutral(),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    // ── Phase Controller ──

    #[test]
    fn phase_controller_starts_at_first_phase() {
        let phases = vec![
            BossPhase::new(1, 1.0),
            BossPhase::new(2, 0.5),
            BossPhase::new(3, 0.25),
        ];
        let ctrl = BossPhaseController::new(phases);
        assert_eq!(ctrl.current_phase_number(), 1);
    }

    #[test]
    fn phase_controller_transitions_on_hp_drop() {
        let phases = vec![
            BossPhase::new(1, 1.0),
            BossPhase::new(2, 0.5),
            BossPhase::new(3, 0.25),
        ];
        let mut ctrl = BossPhaseController::new(phases);

        // At full HP, no transition.
        assert!(ctrl.check_transition(0.8).is_none());

        // Drop below 50%: transition to phase 2.
        let result = ctrl.check_transition(0.4);
        assert!(result.is_some());
        assert_eq!(ctrl.current_phase_number(), 2);
    }

    #[test]
    fn phase_controller_skips_to_deepest_crossed_phase() {
        let phases = vec![
            BossPhase::new(1, 1.0),
            BossPhase::new(2, 0.5),
            BossPhase::new(3, 0.25),
        ];
        let mut ctrl = BossPhaseController::new(phases);

        // Drop below 25% in one hit: should skip to phase 3.
        let result = ctrl.check_transition(0.1);
        assert!(result.is_some());
        assert_eq!(ctrl.current_phase_number(), 3);
    }

    #[test]
    fn phase_controller_transition_animation() {
        let phases = vec![
            BossPhase::new(1, 1.0),
            BossPhase::new(2, 0.5),
        ];
        let mut ctrl = BossPhaseController::new(phases);
        ctrl.check_transition(0.4);

        assert!(ctrl.is_transitioning());
        assert!(!ctrl.update_transition(0.5)); // not done
        assert!(ctrl.is_transitioning());
        assert!(ctrl.update_transition(1.5)); // done (total > 1.5)
        assert!(!ctrl.is_transitioning());
    }

    // ── Mirror Boss ──

    #[test]
    fn mirror_records_and_retrieves_actions() {
        let mut state = MirrorBossState::new();
        state.record_action(RecordedAction {
            action_type: PlayerActionType::Attack,
            turn: 5,
            damage_dealt: 20.0,
            element: Some(Element::Fire),
        });

        // With delay of 1, action from turn 5 is available at turn 6.
        let mirrored = state.get_mirrored_action(6);
        assert!(mirrored.is_some());
        assert_eq!(mirrored.unwrap().action_type, PlayerActionType::Attack);
    }

    #[test]
    fn mirror_buffer_limited_to_depth() {
        let mut state = MirrorBossState::new();
        for i in 0..10 {
            state.record_action(RecordedAction {
                action_type: PlayerActionType::Attack,
                turn: i,
                damage_dealt: 10.0,
                element: None,
            });
        }
        assert_eq!(state.mirror_buffer.len(), 3); // max depth
    }

    #[test]
    fn mirror_phase2_copies_stats() {
        let mut state = MirrorBossState::new();
        let stats = CombatStats { attack: 50.0, armor: 30.0, ..CombatStats::default() };
        state.enter_phase2();
        state.copy_stats(&stats);
        assert!(state.copying_stats);
        assert!((state.copied_attack - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mirror_phase3_simultaneous() {
        let mut state = MirrorBossState::new();
        assert!(!state.simultaneous);
        state.enter_phase3();
        assert!(state.simultaneous);
    }

    // ── Null Boss ──

    #[test]
    fn null_erase_buffs() {
        let mut state = NullBossState::new();
        let erased = state.erase_buffs(5);
        assert_eq!(erased, 2); // max 2 per turn
        assert_eq!(state.buffs_erased, 2);
    }

    #[test]
    fn null_erase_ui_element() {
        let mut state = NullBossState::new();
        let target = state.erase_ui_element();
        assert!(!state.erased_ui.is_empty());
        assert!(state.erased_ui.contains(&target));
    }

    #[test]
    fn null_lock_ability() {
        let mut state = NullBossState::new();
        let slot = state.lock_random_ability(6);
        assert!(slot.is_some());
        assert_eq!(state.locked_abilities.len(), 1);
    }

    #[test]
    fn null_restore_all() {
        let mut state = NullBossState::new();
        state.erase_ui_element();
        state.lock_random_ability(6);
        let restored = state.restore_all();
        assert!(!restored.is_empty());
        assert!(state.erased_ui.is_empty());
        assert!(state.locked_abilities.is_empty());
    }

    // ── Committee Boss ──

    #[test]
    fn committee_has_five_judges() {
        let state = CommitteeBossState::new();
        assert_eq!(state.judges.len(), 5);
        assert_eq!(state.alive_count(), 5);
    }

    #[test]
    fn committee_vote_returns_action() {
        let mut state = CommitteeBossState::new();
        let action = state.conduct_vote(0.8);
        // Just verify it returns a valid action.
        let _ = action;
    }

    #[test]
    fn committee_killing_judges() {
        let mut state = CommitteeBossState::new();
        let killed = state.judges[0].take_damage(500.0);
        assert!(killed);
        assert_eq!(state.alive_count(), 4);
    }

    #[test]
    fn committee_ghost_voting() {
        let mut state = CommitteeBossState::new();
        state.judges[0].take_damage(500.0);
        state.enable_ghost_voting();
        assert!(state.judges[0].ghost);
        // Ghost should still participate in vote.
        let _action = state.conduct_vote(0.5);
    }

    #[test]
    fn committee_merge() {
        let mut state = CommitteeBossState::new();
        let combined = state.merge_judges();
        assert!(combined > 0.0);
        assert!(state.merged);
    }

    // ── Fibonacci Hydra ──

    #[test]
    fn fibonacci_hydra_starts_with_one_head() {
        let state = FibonacciHydraState::new(1000.0);
        assert_eq!(state.alive_count(), 1);
    }

    #[test]
    fn fibonacci_hydra_split_on_death() {
        let mut state = FibonacciHydraState::new(1000.0);
        // Kill head 0.
        state.damage_head(0, 1000.0);
        assert_eq!(state.alive_count(), 0);

        // Split.
        let result = state.try_split(0);
        assert!(result.is_some());
        let (a, b) = result.unwrap();

        // Two new heads at 61.8% HP.
        assert_eq!(state.alive_count(), 2);
        let head_a = state.heads.iter().find(|h| h.id == a).unwrap();
        assert!((head_a.max_hp - 618.0).abs() < 1.0);
        let head_b = state.heads.iter().find(|h| h.id == b).unwrap();
        assert!((head_b.max_hp - 618.0).abs() < 1.0);
    }

    #[test]
    fn fibonacci_hydra_max_depth() {
        let mut state = FibonacciHydraState::new(1000.0);
        state.max_depth = 2; // limit for test speed

        // Kill and split head 0.
        state.damage_head(0, 1000.0);
        let (a, b) = state.try_split(0).unwrap();

        // Kill and split children.
        state.damage_head(a, 1000.0);
        let (c, d) = state.try_split(a).unwrap();

        state.damage_head(b, 1000.0);
        let (e, f) = state.try_split(b).unwrap();

        // Kill depth-2 heads: they should NOT split further.
        state.damage_head(c, 1000.0);
        assert!(state.try_split(c).is_none());
        state.damage_head(d, 1000.0);
        state.damage_head(e, 1000.0);
        state.damage_head(f, 1000.0);

        assert!(state.is_defeated());
    }

    #[test]
    fn fibonacci_hydra_max_possible_heads() {
        let state = FibonacciHydraState::new(1000.0);
        assert_eq!(state.max_possible_heads(), 32); // 2^5
    }

    // ── Eigenstate Boss ──

    #[test]
    fn eigenstate_superposition_by_default() {
        let state = EigenstateBossState::new();
        assert!(state.in_superposition());
        assert!(state.collapsed_form.is_none());
    }

    #[test]
    fn eigenstate_observation_collapses() {
        let mut state = EigenstateBossState::new();
        let form = state.observe();
        assert!(!state.in_superposition());
        assert_eq!(state.collapsed_form, Some(form));
    }

    #[test]
    fn eigenstate_unobserve_returns_to_superposition() {
        let mut state = EigenstateBossState::new();
        state.observe();
        state.unobserve();
        assert!(state.in_superposition());
    }

    #[test]
    fn eigenstate_phase2_adds_evasion() {
        let mut state = EigenstateBossState::new();
        assert_eq!(state.forms.len(), 2);
        state.add_evasion_form();
        assert_eq!(state.forms.len(), 3);
    }

    #[test]
    fn eigenstate_entangle() {
        let mut state = EigenstateBossState::new();
        let copy_hp = state.entangle(500.0);
        assert!(state.entangled);
        assert!((copy_hp - 500.0).abs() < f32::EPSILON);
    }

    #[test]
    fn eigenstate_mirror_damage() {
        let mut state = EigenstateBossState::new();
        state.entangle(500.0);
        let mirrored = state.mirror_damage(100.0);
        assert!((mirrored - 100.0).abs() < f32::EPSILON);
        assert!((state.entangled_hp - 400.0).abs() < f32::EPSILON);
    }

    // ── Ouroboros Boss ──

    #[test]
    fn ouroboros_starts_reversed() {
        let state = OuroborosBossState::new();
        assert!(state.reversed);
    }

    #[test]
    fn ouroboros_reversed_damage_heals_attacker() {
        let mut state = OuroborosBossState::new();
        let (damage, heal) = state.process_damage(100.0, true);
        assert!((damage - 100.0).abs() < f32::EPSILON);
        assert!((heal - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn ouroboros_reversed_heal_damages() {
        let state = OuroborosBossState::new();
        let result = state.process_heal(50.0);
        assert!((result - (-50.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn ouroboros_normal_rules_no_heal() {
        let mut state = OuroborosBossState::new();
        state.reversed = false;
        let (damage, heal) = state.process_damage(100.0, true);
        assert!((damage - 100.0).abs() < f32::EPSILON);
        assert!((heal - 0.0).abs() < f32::EPSILON);
    }

    // ── Algorithm Reborn ──

    #[test]
    fn algorithm_records_frequency() {
        let mut state = AlgorithmRebornState::new();
        state.record_action(PlayerActionType::Attack);
        state.record_action(PlayerActionType::Attack);
        state.record_action(PlayerActionType::Heal);

        assert_eq!(state.action_frequency[&PlayerActionType::Attack], 2);
        assert_eq!(state.action_frequency[&PlayerActionType::Heal], 1);
    }

    #[test]
    fn algorithm_most_used() {
        let mut state = AlgorithmRebornState::new();
        state.record_action(PlayerActionType::Attack);
        state.record_action(PlayerActionType::Attack);
        state.record_action(PlayerActionType::Heal);

        assert_eq!(state.most_used_action(), Some(PlayerActionType::Attack));
    }

    #[test]
    fn algorithm_markov_prediction() {
        let mut state = AlgorithmRebornState::new();
        // Build pattern: Attack -> Heal -> Attack -> Heal.
        state.record_action(PlayerActionType::Attack);
        state.record_action(PlayerActionType::Heal);
        state.record_action(PlayerActionType::Attack);
        state.record_action(PlayerActionType::Heal);

        // Last action is Heal. After Heal, Attack appeared twice.
        let prediction = state.predict_next();
        assert_eq!(prediction, Some(PlayerActionType::Attack));
    }

    #[test]
    fn algorithm_counter_for() {
        assert_eq!(
            AlgorithmRebornState::counter_for(&PlayerActionType::Attack),
            PlayerActionType::Defend
        );
        assert_eq!(
            AlgorithmRebornState::counter_for(&PlayerActionType::Heal),
            PlayerActionType::Attack
        );
    }

    #[test]
    fn algorithm_degradation_tracks_hp() {
        let mut state = AlgorithmRebornState::new();
        state.update_degradation(0.3);
        assert!((state.degradation - 0.7).abs() < f32::EPSILON);
        assert!((state.hidden_hp_frac - 0.3).abs() < f32::EPSILON);
    }

    // ── Chaos Weaver ──

    #[test]
    fn chaos_weaver_scramble_weaknesses() {
        let mut state = ChaosWeaverState::new();
        state.scramble_weaknesses();
        assert!(!state.weakness_overrides.is_empty());
        assert_eq!(state.chaos_count, 1);
    }

    #[test]
    fn chaos_weaver_swap_slots() {
        let mut state = ChaosWeaverState::new();
        let (a, b) = state.swap_ability_slots(6);
        assert_ne!(a, b);
        assert_eq!(state.slot_swaps[&a], b);
        assert_eq!(state.slot_swaps[&b], a);
    }

    #[test]
    fn chaos_weaver_fake_damage() {
        let mut state = ChaosWeaverState::new();
        state.enable_damage_randomization();
        let fake = state.fake_damage_number(100.0);
        // Fake number should be different from actual (with high probability).
        // Just verify it's positive.
        assert!(fake > 0.0);
    }

    // ── Void Serpent ──

    #[test]
    fn void_serpent_starts_full() {
        let state = VoidSerpentState::new(20, 20);
        assert!((state.arena_fraction() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn void_serpent_consume_edge_shrinks_arena() {
        let mut state = VoidSerpentState::new(20, 20);
        let dir = state.consume_edge();
        assert!(dir.is_some());
        assert!(state.arena_fraction() < 1.0);
    }

    #[test]
    fn void_serpent_safe_zone() {
        let mut state = VoidSerpentState::new(20, 20);
        assert!(state.is_safe(10, 10));
        // Consume north edge.
        state.consumed_north = 5;
        state.arena_height = 15;
        assert!(!state.is_safe(10, 2)); // in consumed zone
        assert!(state.is_safe(10, 10)); // still safe
    }

    #[test]
    fn void_serpent_emerge() {
        let mut state = VoidSerpentState::new(20, 20);
        state.emerge_serpent(100.0);
        assert!(state.serpent_emerged);
        assert!((state.serpent_attack_damage - 100.0).abs() < f32::EPSILON);
    }

    // ── Prime Factorial ──

    #[test]
    fn prime_check() {
        assert!(!PrimeFactorialState::is_prime(0));
        assert!(!PrimeFactorialState::is_prime(1));
        assert!(PrimeFactorialState::is_prime(2));
        assert!(PrimeFactorialState::is_prime(3));
        assert!(!PrimeFactorialState::is_prime(4));
        assert!(PrimeFactorialState::is_prime(5));
        assert!(PrimeFactorialState::is_prime(7));
        assert!(!PrimeFactorialState::is_prime(9));
        assert!(PrimeFactorialState::is_prime(97));
        assert!(PrimeFactorialState::is_prime(997));
    }

    #[test]
    fn prime_filter_damage() {
        let state = PrimeFactorialState::new();
        assert!((state.filter_damage(7.0) - 7.0).abs() < f32::EPSILON); // prime
        assert!((state.filter_damage(8.0) - 0.0).abs() < f32::EPSILON); // not prime
    }

    #[test]
    fn factorial_sequence() {
        let mut state = PrimeFactorialState::new();
        assert!((state.next_factorial_damage() - 1.0).abs() < f32::EPSILON);
        assert!((state.next_factorial_damage() - 2.0).abs() < f32::EPSILON);
        assert!((state.next_factorial_damage() - 6.0).abs() < f32::EPSILON);
        assert!((state.next_factorial_damage() - 24.0).abs() < f32::EPSILON);
        assert!((state.next_factorial_damage() - 120.0).abs() < f32::EPSILON);
        assert!((state.next_factorial_damage() - 720.0).abs() < f32::EPSILON);
        assert!((state.next_factorial_damage() - 5040.0).abs() < f32::EPSILON);
        // Wraps around.
        assert!((state.next_factorial_damage() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn puzzle_generation() {
        let mut state = PrimeFactorialState::new();
        let factors = state.generate_puzzle();
        assert!(factors.len() >= 2);
        assert!(state.puzzle_active);
    }

    #[test]
    fn puzzle_solution() {
        let mut state = PrimeFactorialState::new();
        state.puzzle_target_factors = vec![2, 3, 5];
        state.puzzle_active = true;

        assert!(!state.check_puzzle_solution(29)); // wrong
        assert!(state.check_puzzle_solution(30));   // 2*3*5 = 30
        assert_eq!(state.puzzles_solved, 1);
        assert!(!state.puzzle_active);
    }

    // ── Boss Encounter Manager ──

    #[test]
    fn encounter_creates_all_boss_types() {
        let player_stats = CombatStats::default();
        for &boss_type in BossType::all() {
            let encounter = BossEncounterManager::start_encounter(boss_type, 1, &player_stats);
            assert!(!encounter.finished);
            assert!(encounter.entity.hp > 0.0);
            assert!(!encounter.profile.name.is_empty());
        }
    }

    #[test]
    fn encounter_scales_with_floor() {
        let player_stats = CombatStats::default();
        let e1 = BossEncounterManager::start_encounter(BossType::Mirror, 1, &player_stats);
        let e10 = BossEncounterManager::start_encounter(BossType::Mirror, 10, &player_stats);
        assert!(e10.entity.max_hp > e1.entity.max_hp);
    }

    #[test]
    fn encounter_update_emits_events() {
        let player_stats = CombatStats::default();
        let mut encounter = BossEncounterManager::start_encounter(
            BossType::ChaosWeaver, 1, &player_stats,
        );
        // Turn 0 is divisible by 3 (ChaosWeaver scrambles on turn % 3 == 0).
        let events = encounter.update(0.016, &[]);
        assert!(!events.is_empty());
    }

    #[test]
    fn encounter_boss_death_emits_events() {
        let player_stats = CombatStats::default();
        let mut encounter = BossEncounterManager::start_encounter(
            BossType::Mirror, 1, &player_stats,
        );
        encounter.entity.hp = 0.0;
        let events = encounter.update(0.016, &[]);

        let has_defeat = events.iter().any(|e| matches!(e, BossEvent::BossDefeated(_)));
        assert!(has_defeat);
        assert!(encounter.finished);
    }

    #[test]
    fn encounter_null_death_restores_ui() {
        let player_stats = CombatStats::default();
        let mut encounter = BossEncounterManager::start_encounter(
            BossType::Null, 1, &player_stats,
        );

        // Manually erase some UI elements.
        if let BossMechanicState::Null(ref mut state) = encounter.mechanic_state {
            state.erase_ui_element();
        }

        encounter.entity.hp = 0.0;
        let events = encounter.update(0.016, &[]);

        let has_restore = events.iter().any(|e| matches!(e, BossEvent::UiRestored(_)));
        assert!(has_restore);
    }
}
