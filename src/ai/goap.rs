//! Goal-Oriented Action Planning (GOAP).
//!
//! Each agent has a world state (map of bool conditions), a goal state, and
//! a set of actions with preconditions and effects. The planner uses A* to
//! find the cheapest sequence of actions that transforms current state into
//! goal state.

use std::collections::{HashMap, BinaryHeap, HashSet};
use std::cmp::Ordering;

// ── WorldState ────────────────────────────────────────────────────────────────

/// A set of named boolean conditions describing world state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorldState(HashMap<String, bool>);

impl WorldState {
    pub fn new() -> Self { Self::default() }

    pub fn set(&mut self, key: &str, value: bool) {
        self.0.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> bool {
        *self.0.get(key).unwrap_or(&false)
    }

    /// Check if this state satisfies all conditions in `goal`.
    pub fn satisfies(&self, goal: &WorldState) -> bool {
        goal.0.iter().all(|(k, &v)| self.get(k) == v)
    }

    /// Apply an action's effects, returning a new state.
    pub fn apply(&self, effects: &WorldState) -> WorldState {
        let mut next = self.clone();
        for (k, &v) in &effects.0 {
            next.0.insert(k.clone(), v);
        }
        next
    }

    /// Distance heuristic: number of conditions in goal not satisfied.
    pub fn distance_to(&self, goal: &WorldState) -> usize {
        goal.0.iter().filter(|(k, &v)| self.get(k) != v).count()
    }
}

// ── GoapAction ────────────────────────────────────────────────────────────────

/// An action that can be taken by a GOAP agent.
#[derive(Debug, Clone)]
pub struct GoapAction {
    pub name:          String,
    pub cost:          f32,
    pub preconditions: WorldState,
    pub effects:       WorldState,
    /// Optional: position/range requirements evaluated at plan time.
    pub requires_in_range: Option<String>,
}

impl GoapAction {
    pub fn new(name: &str, cost: f32) -> Self {
        Self {
            name: name.to_string(),
            cost,
            preconditions: WorldState::new(),
            effects: WorldState::new(),
            requires_in_range: None,
        }
    }

    pub fn with_precondition(mut self, key: &str, value: bool) -> Self {
        self.preconditions.set(key, value);
        self
    }

    pub fn with_effect(mut self, key: &str, value: bool) -> Self {
        self.effects.set(key, value);
        self
    }

    pub fn requires_range(mut self, target: &str) -> Self {
        self.requires_in_range = Some(target.to_string());
        self
    }

    pub fn is_applicable(&self, state: &WorldState) -> bool {
        state.satisfies(&self.preconditions)
    }
}

// ── A* search node ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct SearchNode {
    state:    WorldState,
    path:     Vec<String>, // action names taken
    cost:     f32,
    heuristic: usize,
}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool { self.f() == other.f() }
}
impl Eq for SearchNode {}

impl PartialOrd for SearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for SearchNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_ord().cmp(&self.f_ord()) // min-heap
    }
}

impl SearchNode {
    fn f(&self) -> f32 { self.cost + self.heuristic as f32 }
    fn f_ord(&self) -> u64 { (self.f() * 1000.0) as u64 }
}

// ── GoapPlanner ───────────────────────────────────────────────────────────────

/// Plans a sequence of actions to reach goal state from current state.
pub struct GoapPlanner;

impl GoapPlanner {
    /// Returns the cheapest action plan, or None if no plan exists.
    pub fn plan(
        start: &WorldState,
        goal:  &WorldState,
        actions: &[GoapAction],
        max_depth: usize,
    ) -> Option<Vec<String>> {
        let mut open: BinaryHeap<SearchNode> = BinaryHeap::new();
        let mut closed: Vec<WorldState>  = Vec::new();

        open.push(SearchNode {
            state: start.clone(),
            path: Vec::new(),
            cost: 0.0,
            heuristic: start.distance_to(goal),
        });

        while let Some(node) = open.pop() {
            if node.state.satisfies(goal) {
                return Some(node.path);
            }
            if node.path.len() >= max_depth { continue; }
            if closed.contains(&node.state) { continue; }
            closed.push(node.state.clone());

            for action in actions {
                if !action.is_applicable(&node.state) { continue; }
                let next_state = node.state.apply(&action.effects);
                if closed.iter().any(|s| s == &next_state) { continue; }
                let mut path = node.path.clone();
                path.push(action.name.clone());
                open.push(SearchNode {
                    heuristic: next_state.distance_to(goal),
                    state: next_state,
                    path,
                    cost: node.cost + action.cost,
                });
            }
        }
        None
    }
}

// ── GoapAgent ─────────────────────────────────────────────────────────────────

/// A runtime GOAP agent that re-plans when its goal or state changes.
pub struct GoapAgent<W> {
    pub name:       String,
    pub world_state: WorldState,
    pub goal:       WorldState,
    pub actions:    Vec<GoapAction>,
    current_plan:   Vec<String>,
    plan_step:      usize,
    pub max_depth:  usize,
    /// Callbacks: action_name → execute fn.
    executors:      HashMap<String, Box<dyn Fn(&mut W, &mut WorldState) -> ActionResult + Send + Sync>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionResult {
    /// Still executing.
    InProgress,
    /// Action completed; advance plan.
    Done,
    /// Action failed; re-plan.
    Failed,
}

impl<W> GoapAgent<W> {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            world_state: WorldState::new(),
            goal: WorldState::new(),
            actions: Vec::new(),
            current_plan: Vec::new(),
            plan_step: 0,
            max_depth: 10,
            executors: HashMap::new(),
        }
    }

    pub fn add_action(&mut self, action: GoapAction) {
        self.actions.push(action);
    }

    pub fn add_executor(
        &mut self,
        name: &str,
        func: impl Fn(&mut W, &mut WorldState) -> ActionResult + Send + Sync + 'static,
    ) {
        self.executors.insert(name.to_string(), Box::new(func));
    }

    pub fn set_state(&mut self, key: &str, value: bool) {
        self.world_state.set(key, value);
    }

    pub fn set_goal(&mut self, key: &str, value: bool) {
        self.goal.set(key, value);
        self.current_plan.clear();
        self.plan_step = 0;
    }

    /// Call each frame. Returns None if no plan/goal, Some(action_name) for current action.
    pub fn tick(&mut self, world: &mut W) -> Option<String> {
        // Re-plan if needed
        if self.plan_step >= self.current_plan.len() {
            if self.world_state.satisfies(&self.goal) {
                return None; // Already at goal
            }
            match GoapPlanner::plan(&self.world_state, &self.goal, &self.actions, self.max_depth) {
                Some(plan) => { self.current_plan = plan; self.plan_step = 0; }
                None       => return None,
            }
        }

        let action_name = self.current_plan[self.plan_step].clone();

        if let Some(executor) = self.executors.get(&action_name) {
            let result = executor(world, &mut self.world_state);
            match result {
                ActionResult::Done     => { self.plan_step += 1; }
                ActionResult::Failed   => {
                    self.current_plan.clear();
                    self.plan_step = 0;
                }
                ActionResult::InProgress => {}
            }
        } else {
            // No executor registered — auto-advance
            self.plan_step += 1;
        }

        Some(action_name)
    }

    pub fn current_plan(&self) -> &[String] { &self.current_plan }
    pub fn has_goal(&self) -> bool { !self.goal.0.is_empty() }
    pub fn plan_length(&self) -> usize { self.current_plan.len() }
}

// ── Pre-built action sets ─────────────────────────────────────────────────────

/// Standard combat actions for a melee fighter.
pub fn melee_combat_actions() -> Vec<GoapAction> {
    vec![
        GoapAction::new("move_to_target", 1.0)
            .with_precondition("has_target", true)
            .with_effect("in_range", true),
        GoapAction::new("attack", 1.0)
            .with_precondition("has_target", true)
            .with_precondition("in_range", true)
            .with_precondition("weapon_ready", true)
            .with_effect("target_dead", true),
        GoapAction::new("equip_weapon", 2.0)
            .with_precondition("has_weapon", true)
            .with_effect("weapon_ready", true),
        GoapAction::new("pick_up_weapon", 1.5)
            .with_precondition("weapon_nearby", true)
            .with_effect("has_weapon", true),
        GoapAction::new("flee", 1.0)
            .with_precondition("low_health", true)
            .with_effect("safe", true),
        GoapAction::new("heal", 2.0)
            .with_precondition("has_potion", true)
            .with_effect("low_health", false),
    ]
}

/// Patrol and investigation actions.
pub fn guard_actions() -> Vec<GoapAction> {
    vec![
        GoapAction::new("patrol", 1.0)
            .with_effect("patrolling", true),
        GoapAction::new("investigate_noise", 1.5)
            .with_precondition("heard_noise", true)
            .with_effect("area_clear", true)
            .with_effect("heard_noise", false),
        GoapAction::new("sound_alarm", 2.0)
            .with_precondition("sees_intruder", true)
            .with_effect("alarm_raised", true),
        GoapAction::new("chase_intruder", 1.0)
            .with_precondition("sees_intruder", true)
            .with_effect("in_range", true),
        GoapAction::new("return_to_post", 0.5)
            .with_effect("at_post", true),
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_state_satisfies() {
        let mut state = WorldState::new();
        state.set("alive", true);
        state.set("armed", false);

        let mut goal = WorldState::new();
        goal.set("alive", true);
        assert!(state.satisfies(&goal));

        goal.set("armed", true);
        assert!(!state.satisfies(&goal));
    }

    #[test]
    fn test_world_state_apply() {
        let mut state = WorldState::new();
        state.set("alive", true);
        let mut effects = WorldState::new();
        effects.set("armed", true);
        let next = state.apply(&effects);
        assert!(next.get("alive"));
        assert!(next.get("armed"));
    }

    #[test]
    fn test_planner_finds_plan() {
        let actions = melee_combat_actions();

        let mut start = WorldState::new();
        start.set("has_target", true);
        start.set("in_range", false);
        start.set("weapon_ready", true);

        let mut goal = WorldState::new();
        goal.set("target_dead", true);

        let plan = GoapPlanner::plan(&start, &goal, &actions, 5);
        assert!(plan.is_some(), "should find a plan");
        let plan = plan.unwrap();
        assert!(plan.contains(&"move_to_target".to_string()));
        assert!(plan.contains(&"attack".to_string()));
    }

    #[test]
    fn test_planner_longer_chain() {
        let actions = melee_combat_actions();

        let mut start = WorldState::new();
        start.set("has_target", true);
        start.set("weapon_nearby", true);

        let mut goal = WorldState::new();
        goal.set("target_dead", true);

        let plan = GoapPlanner::plan(&start, &goal, &actions, 8);
        assert!(plan.is_some(), "should plan pick_up → equip → move → attack chain");
    }

    #[test]
    fn test_planner_no_possible_plan() {
        let mut start = WorldState::new();
        start.set("has_target", false);

        let mut goal = WorldState::new();
        goal.set("target_dead", true);

        // No action to acquire target
        let actions = vec![
            GoapAction::new("attack", 1.0)
                .with_precondition("has_target", true)
                .with_effect("target_dead", true),
        ];

        let plan = GoapPlanner::plan(&start, &goal, &actions, 5);
        assert!(plan.is_none());
    }

    #[test]
    fn test_agent_executes_plan() {
        let mut agent: GoapAgent<Vec<String>> = GoapAgent::new("test_agent");

        agent.add_action(
            GoapAction::new("do_thing", 1.0)
                .with_effect("thing_done", true),
        );

        agent.add_executor("do_thing", |world, state| {
            world.push("did_thing".to_string());
            state.set("thing_done", true);
            ActionResult::Done
        });

        agent.set_goal("thing_done", true);

        let mut world: Vec<String> = Vec::new();
        let action = agent.tick(&mut world);
        assert_eq!(action, Some("do_thing".to_string()));
        assert!(world.contains(&"did_thing".to_string()));
    }

    #[test]
    fn test_agent_no_replan_at_goal() {
        let mut agent: GoapAgent<()> = GoapAgent::new("agent");
        let mut state = agent.world_state.clone();
        state.set("done", true);
        agent.world_state = state;
        agent.set_goal("done", true);
        let mut world = ();
        let action = agent.tick(&mut world);
        assert!(action.is_none(), "already at goal — no action needed");
    }

    #[test]
    fn test_guard_actions_plan() {
        let actions = guard_actions();
        let mut start = WorldState::new();
        start.set("heard_noise", true);
        let mut goal = WorldState::new();
        goal.set("area_clear", true);
        let plan = GoapPlanner::plan(&start, &goal, &actions, 3);
        assert!(plan.is_some());
        assert!(plan.unwrap().contains(&"investigate_noise".to_string()));
    }
}
