// src/pathfinding/mod.rs
// Navigation and pathfinding subsystem for the Proof Engine game engine.

pub mod navmesh;
pub mod astar;
pub mod steering;

pub use navmesh::{
    NavMesh, NavPoly, NavPolyId, NavPortal, NavPath, NavPoint,
    AreaFlags, AreaCost, ObstacleCutter, NavMeshQuery,
};

pub use astar::{
    AStarGraph, AStarNode, NodeId, AStarResult,
    GridMap, JpsPathfinder,
    HierarchicalPathfinder, Cluster, ClusterId,
    FlowField, FlowFieldGrid,
    PathCache, CachedPath,
};

pub use steering::{
    SteeringAgent, SteeringOutput,
    Seek, Flee, Arrive, Pursuit, Evade, Wander,
    ObstacleAvoidance, WallFollowing,
    Flock, FlockingAgent, FlockingConfig,
    FormationSlot, FormationMovement,
    PathFollower,
    BehaviorWeight, BlendedSteering,
    Vec2,
};
