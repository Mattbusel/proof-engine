//! AI module — Behavior Trees, Utility AI, Goal-Oriented Action Planning,
//! Pathfinding, Navigation Meshes, Flow Fields, Steering Behaviors, and Blackboard.

pub mod behavior_tree;
pub mod utility;
pub mod goap;

// New AI systems
pub mod pathfinding;
pub mod navmesh;
pub mod flowfield;
pub mod steering;
pub mod blackboard;

// Re-exports — most commonly used types surfaced at the ai:: level.

// Pathfinding
pub use pathfinding::{
    PathGrid, PathNode, AStarPathfinder,
    JumpPointSearch, DijkstraMap, HierarchicalPathfinder,
    Path, PathRequest, PathResult, PathfindingStats,
    Heuristic, smooth_path, spline_path,
};

// Navigation mesh
pub use navmesh::{
    NavMesh, NavVertex, NavTriangle, Portal,
    NavMeshAgent, NavMeshBuilder, AabbObstacle,
    BatchPathQuery, NavMeshSpatialHash,
};

// Flow field
pub use flowfield::{
    FlowField, FlowFieldCache, FlowFieldGroup, FlowFieldAgent,
    Flock, Boid, DynamicObstacleField,
};

// Steering
pub use steering::{
    SteeringAgent, SteeringBehavior, WeightedSteering,
    PrioritySteeringCombiner, SteeringSystem,
    ContextMap, KinematicAgent,
    seek, flee, arrive, pursuit, evade, wander,
    obstacle_avoidance, wall_avoidance, path_following,
    separation, alignment, cohesion, leader_following,
    queue, interpose,
};

// Blackboard
pub use blackboard::{
    Blackboard, BlackboardEntry, BlackboardValue,
    SharedBlackboard, BlackboardCondition, BlackboardObserver,
};
