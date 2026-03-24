//! AI Behavior subsystem — Behavior Trees, built-in nodes, and GOAP planner.
//!
//! # Modules
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`tree`]    | Core BT engine: `BehaviorNode`, `NodeStatus`, `Blackboard`, `BehaviorTree`, `TreeBuilder`, `SubtreeRegistry` |
//! | [`nodes`]   | Ready-to-use leaf/decorator constructors: Wait, MoveTo, LookAt, PlayAnimation, CheckDistance, CheckHealth, CheckLineOfSight, SetBlackboard, RandomSelector, WeightedSelector, and more |
//! | [`planner`] | GOAP planner: `WorldState`, `Action`, `GoalStack`, `GoapPlanner`, `PlanExecutor`, `GoapAgent`, `ActionLibrary` |
//!
//! # Quick start
//!
//! ```ignore
//! use proof_engine::behavior::prelude::*;
//!
//! // Build a simple patrol-and-attack behavior tree.
//! let root = TreeBuilder::selector("root")
//!     .sequence_child("attack_sequence")
//!         .node(check_in_range("in_range", "agent_pos", "enemy_pos", 5.0))
//!         .node(fire_at_target("fire", "can_fire", "ammo", "fire_request"))
//!     .end()
//!     .node(patrol_set_target(
//!         "patrol",
//!         vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
//!         "wp_idx", "agent_pos", "patrol_target", 0.5,
//!     ))
//!     .build();
//!
//! let mut tree = BehaviorTree::new("enemy_ai", root);
//! tree.blackboard_mut().set("agent_pos", Vec3::ZERO);
//!
//! loop {
//!     let status = tree.tick(0.016);
//!     // status is Running / Success / Failure
//!     # break;
//! }
//! ```

pub mod tree;
pub mod nodes;
pub mod planner;

// ── Re-exports ────────────────────────────────────────────────────────────────

// Core tree types
pub use tree::{
    BehaviorNode,
    BehaviorTree,
    Blackboard,
    BlackboardValue,
    DecoratorKind,
    DecoratorState,
    NodeStatus,
    ParallelPolicy,
    SubtreeRegistry,
    TreeBuilder,
    // Convenience constructors
    cooldown,
    invert,
    leaf,
    parallel,
    repeat,
    selector,
    sequence,
    timeout,
};

// Built-in node constructors
pub use nodes::{
    CompareOp,
    // Actions
    check_distance,
    check_health,
    check_health_low,
    check_health_ok,
    check_in_range,
    check_out_of_range,
    check_line_of_sight,
    check_blackboard_bool,
    check_blackboard_float,
    check_blackboard_exists,
    clear_blackboard,
    copy_blackboard,
    cooldown_node,
    debug_log,
    debug_log_blackboard,
    face_direction,
    fail_always,
    fire_at_target,
    flee,
    idle,
    invert_node,
    look_at,
    melee_attack,
    move_to,
    move_to_2d,
    patrol_set_target,
    play_animation,
    random_selector,
    repeat_forever,
    repeat_node,
    set_blackboard,
    succeed_always,
    timeout_node,
    wait,
    weighted_selector,
    blackboard_guard,
};

// GOAP planner types
pub use planner::{
    Action,
    ActionEffects,
    ActionLibrary,
    ExecutorState,
    GoapAgent,
    GoapPlanner,
    Goal,
    GoalStack,
    PlanError,
    PlanExecutor,
    PlanStep,
    PlanStepStatus,
    Preconditions,
    WorldState,
};

// ── Prelude ───────────────────────────────────────────────────────────────────

/// Convenience glob import: `use proof_engine::behavior::prelude::*;`
pub mod prelude {
    pub use super::{
        // Tree types
        BehaviorNode, BehaviorTree, Blackboard, BlackboardValue,
        DecoratorKind, NodeStatus, ParallelPolicy, SubtreeRegistry, TreeBuilder,
        // Tree constructors
        cooldown, invert, leaf, parallel, repeat, selector, sequence, timeout,
        // Node constructors
        CompareOp,
        check_distance, check_health, check_health_low, check_health_ok,
        check_in_range, check_out_of_range, check_line_of_sight,
        check_blackboard_bool, check_blackboard_float, check_blackboard_exists,
        clear_blackboard, copy_blackboard, cooldown_node,
        debug_log, debug_log_blackboard,
        face_direction, fail_always, fire_at_target, flee, idle,
        invert_node, look_at, melee_attack, move_to, move_to_2d,
        patrol_set_target, play_animation,
        random_selector, repeat_forever, repeat_node,
        set_blackboard, succeed_always, timeout_node, wait, weighted_selector,
        blackboard_guard,
        // GOAP
        Action, ActionEffects, ActionLibrary,
        ExecutorState, GoapAgent, GoapPlanner, Goal, GoalStack,
        PlanError, PlanExecutor, PlanStep, PlanStepStatus,
        Preconditions, WorldState,
    };
    pub use glam::{Vec2, Vec3};
}
