//! Built-in behavior-tree node library.
//!
//! Every function in this module returns a [`BehaviorNode::Leaf`] (or a small
//! composite that acts like a logical leaf) that encapsulates one well-defined
//! game-AI action or condition.  They are designed to be composed with
//! [`crate::behavior::tree::TreeBuilder`].
//!
//! # Node catalogue
//!
//! **Actions** (mutate world state / run over time)
//! - [`wait`]             — pause for a fixed duration
//! - [`move_to`]          — move agent toward a target position
//! - [`look_at`]          — rotate agent to face a target
//! - [`play_animation`]   — trigger and wait for an animation clip
//! - [`set_blackboard`]   — write a value onto the blackboard
//!
//! **Conditions** (instant pass/fail checks)
//! - [`check_distance`]   — is agent within/outside a range?
//! - [`check_health`]     — compare health against a threshold
//! - [`check_line_of_sight`] — can the agent see a target?
//! - [`check_blackboard_bool`] — read a bool flag
//! - [`check_blackboard_float`] — compare a float value
//!
//! **Decorator wrappers** (higher-order node constructors)
//! - [`invert_node`]      — flip Success↔Failure
//! - [`repeat_node`]      — run child N times
//! - [`timeout_node`]     — fail child if it takes too long
//!
//! **Composite helpers**
//! - [`random_selector`]  — shuffle children then select
//! - [`weighted_selector`] — pick child by probability weight

use std::collections::HashMap;
use glam::{Vec2, Vec3};

use super::tree::{
    BehaviorNode, BehaviorTree, Blackboard, BlackboardValue,
    DecoratorKind, DecoratorState, NodeStatus, ParallelPolicy,
    SubtreeRegistry,
};

// ── Internal Utilities ────────────────────────────────────────────────────────

/// Simple deterministic pseudo-random number generator (xorshift64).
/// Seeded from the blackboard `"__rng_seed"` key, or 12345 if absent.
struct Rng { state: u64 }

impl Rng {
    fn new(seed: u64) -> Self { Self { state: seed.max(1) } }
    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
    fn next_f32(&mut self) -> f32 { (self.next_u64() as f32) / (u64::MAX as f32) }
    fn range_f32(&mut self, lo: f32, hi: f32) -> f32 { lo + self.next_f32() * (hi - lo) }
    fn range_usize(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next_u64() as usize % (hi - lo))
    }
    /// Fisher-Yates shuffle.
    fn shuffle<T>(&mut self, data: &mut [T]) {
        let n = data.len();
        for i in (1..n).rev() {
            let j = self.range_usize(0, i + 1);
            data.swap(i, j);
        }
    }
}

fn rng_from_bb(bb: &Blackboard) -> Rng {
    let seed = bb.get_int("__rng_seed").unwrap_or(12345) as u64;
    Rng::new(seed)
}

// ── Wait ──────────────────────────────────────────────────────────────────────

/// Pause execution for `duration_secs` seconds, then succeed.
///
/// The node stores its accumulated elapsed time on the blackboard under
/// `__wait_{name}_elapsed` to survive tree resets gracefully.
pub fn wait(name: &str, duration_secs: f32) -> BehaviorNode {
    let elapsed_key = format!("__wait_{name}_elapsed");
    let enter_key   = elapsed_key.clone();
    let tick_key    = elapsed_key.clone();

    BehaviorNode::Leaf {
        name:    name.to_string(),
        on_enter: Some(Box::new(move |bb: &mut Blackboard| {
            bb.set(enter_key.as_str(), 0.0f64);
        })),
        on_tick: Box::new(move |bb: &mut Blackboard, dt: f32| {
            let elapsed = bb.get_float(tick_key.as_str()).unwrap_or(0.0) + dt as f64;
            bb.set(tick_key.as_str(), elapsed);
            if elapsed >= duration_secs as f64 {
                NodeStatus::Success
            } else {
                NodeStatus::Running
            }
        }),
        on_exit: None,
        entered: false,
    }
}

// ── MoveTo ────────────────────────────────────────────────────────────────────

/// Move an agent toward a 3-D target stored on the blackboard.
///
/// Reads:
/// - `"{agent_pos_key}"` — current agent position (`Vec3`)
/// - `"{target_key}"`    — target position (`Vec3`)
///
/// Writes:
/// - `"{agent_pos_key}"` — updated position after movement this tick
///
/// Succeeds when the agent is within `arrival_radius` of the target.
/// Returns `Failure` if either position key is absent.
pub fn move_to(
    name:          &str,
    agent_pos_key: &str,
    target_key:    &str,
    speed:         f32,
    arrival_radius: f32,
) -> BehaviorNode {
    let pos_key    = agent_pos_key.to_string();
    let tgt_key    = target_key.to_string();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, dt: f32| {
            let pos = match bb.get_vec3(&pos_key) {
                Some(p) => p,
                None    => return NodeStatus::Failure,
            };
            let target = match bb.get_vec3(&tgt_key) {
                Some(t) => t,
                None    => return NodeStatus::Failure,
            };

            let delta = target - pos;
            let dist  = delta.length();

            if dist <= arrival_radius {
                return NodeStatus::Success;
            }

            let dir   = delta / dist;
            let step  = (speed * dt).min(dist - arrival_radius);
            let new_pos = pos + dir * step;
            bb.set(pos_key.as_str(), new_pos);

            if (new_pos - target).length() <= arrival_radius {
                NodeStatus::Success
            } else {
                NodeStatus::Running
            }
        }),
        on_exit: None,
        entered: false,
    }
}

/// Variant that reads the target from a `Vec2` position and moves in 2-D.
pub fn move_to_2d(
    name:           &str,
    agent_pos_key:  &str,
    target_key:     &str,
    speed:          f32,
    arrival_radius: f32,
) -> BehaviorNode {
    let pos_key = agent_pos_key.to_string();
    let tgt_key = target_key.to_string();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, dt: f32| {
            let pos = match bb.get_vec2(&pos_key) {
                Some(p) => p,
                None    => return NodeStatus::Failure,
            };
            let target = match bb.get_vec2(&tgt_key) {
                Some(t) => t,
                None    => return NodeStatus::Failure,
            };

            let delta = target - pos;
            let dist  = delta.length();
            if dist <= arrival_radius {
                return NodeStatus::Success;
            }
            let dir    = delta / dist;
            let step   = (speed * dt).min(dist - arrival_radius);
            let new_pos = pos + dir * step;
            bb.set(pos_key.as_str(), new_pos);

            if (new_pos - target).length() <= arrival_radius {
                NodeStatus::Success
            } else {
                NodeStatus::Running
            }
        }),
        on_exit: None,
        entered: false,
    }
}

// ── LookAt ────────────────────────────────────────────────────────────────────

/// Smoothly rotate the agent's yaw toward a target.
///
/// Reads:
/// - `"{yaw_key}"`    — current yaw angle in radians (`f64`)
/// - `"{target_key}"` — target position (`Vec3`) *or* target yaw directly
///                      (`f64`, taken as absolute yaw)
/// - `"{pos_key}"`    — agent position (`Vec3`, only needed if target is Vec3)
///
/// Writes:
/// - `"{yaw_key}"` — updated yaw
///
/// Succeeds when angular delta ≤ `tolerance_rad`.
pub fn look_at(
    name:          &str,
    yaw_key:       &str,
    pos_key:       &str,
    target_key:    &str,
    turn_speed:    f32,
    tolerance_rad: f32,
) -> BehaviorNode {
    let yaw_k = yaw_key.to_string();
    let pos_k = pos_key.to_string();
    let tgt_k = target_key.to_string();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, dt: f32| {
            let current_yaw = bb.get_float(&yaw_k).unwrap_or(0.0) as f32;

            // Determine desired yaw from target key (Vec3 or f64).
            let desired_yaw = if let Some(target_pos) = bb.get_vec3(&tgt_k) {
                let agent_pos = bb.get_vec3(&pos_k).unwrap_or(Vec3::ZERO);
                let d = target_pos - agent_pos;
                d.z.atan2(d.x)
            } else if let Some(yaw) = bb.get_float(&tgt_k) {
                yaw as f32
            } else {
                return NodeStatus::Failure;
            };

            // Shortest-arc rotation.
            let mut delta = desired_yaw - current_yaw;
            while delta >  std::f32::consts::PI { delta -= std::f32::consts::TAU; }
            while delta < -std::f32::consts::PI { delta += std::f32::consts::TAU; }

            if delta.abs() <= tolerance_rad {
                return NodeStatus::Success;
            }

            let step = (turn_speed * dt).min(delta.abs()) * delta.signum();
            let new_yaw = current_yaw + step;
            bb.set(yaw_k.as_str(), new_yaw as f64);

            if (new_yaw - desired_yaw).abs() <= tolerance_rad {
                NodeStatus::Success
            } else {
                NodeStatus::Running
            }
        }),
        on_exit: None,
        entered: false,
    }
}

// ── PlayAnimation ─────────────────────────────────────────────────────────────

/// Trigger a named animation clip and block until it finishes.
///
/// Writes `"{anim_request_key}"` on enter with the clip name.
/// Reads `"{anim_done_key}"` each tick; returns Success when it becomes true.
///
/// If the done key is not set within `timeout_secs`, returns Failure.
pub fn play_animation(
    name:             &str,
    clip_name:        &str,
    anim_request_key: &str,
    anim_done_key:    &str,
    timeout_secs:     f32,
) -> BehaviorNode {
    let req_key    = anim_request_key.to_string();
    let done_key   = anim_done_key.to_string();
    let clip       = clip_name.to_string();
    let elapsed_key = format!("__anim_{name}_elapsed");

    let enter_req  = req_key.clone();
    let enter_clip = clip.clone();
    let enter_ek   = elapsed_key.clone();
    let tick_dk    = done_key.clone();
    let tick_ek    = elapsed_key.clone();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: Some(Box::new(move |bb: &mut Blackboard| {
            bb.set(enter_req.as_str(), enter_clip.as_str());
            bb.set(enter_ek.as_str(), 0.0f64);
        })),
        on_tick: Box::new(move |bb: &mut Blackboard, dt: f32| {
            let elapsed = bb.get_float(&tick_ek).unwrap_or(0.0) + dt as f64;
            bb.set(tick_ek.as_str(), elapsed);

            if elapsed >= timeout_secs as f64 {
                return NodeStatus::Failure;
            }
            if bb.get_bool(&tick_dk).unwrap_or(false) {
                NodeStatus::Success
            } else {
                NodeStatus::Running
            }
        }),
        on_exit: Some(Box::new(move |bb: &mut Blackboard, _status: NodeStatus| {
            bb.remove(done_key.as_str());
        })),
        entered: false,
    }
}

// ── SetBlackboard ─────────────────────────────────────────────────────────────

/// Instantly write a value onto the blackboard and succeed.
pub fn set_blackboard<V>(name: &str, key: &str, value: V) -> BehaviorNode
where
    V: Into<BlackboardValue> + Clone + Send + 'static,
{
    let k = key.to_string();
    let v = value.into();
    BehaviorNode::Leaf {
        name:     name.to_string(),
        on_enter: None,
        on_tick:  Box::new(move |bb: &mut Blackboard, _dt: f32| {
            bb.set(k.as_str(), v.clone());
            NodeStatus::Success
        }),
        on_exit:  None,
        entered:  false,
    }
}

/// Remove a key from the blackboard and succeed.
pub fn clear_blackboard(name: &str, key: &str) -> BehaviorNode {
    let k = key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            bb.remove(k.as_str());
            NodeStatus::Success
        }),
        on_exit: None,
        entered: false,
    }
}

/// Copy a blackboard value from one key to another and succeed.
pub fn copy_blackboard(name: &str, src_key: &str, dst_key: &str) -> BehaviorNode {
    let src = src_key.to_string();
    let dst = dst_key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            match bb.get(&src).cloned() {
                Some(val) => { bb.set(dst.as_str(), val); NodeStatus::Success }
                None      => NodeStatus::Failure,
            }
        }),
        on_exit: None,
        entered: false,
    }
}

// ── CheckDistance ─────────────────────────────────────────────────────────────

/// Condition: is the distance between two Vec3 positions within `[min, max]`?
///
/// Reads `"{pos_a_key}"` and `"{pos_b_key}"` from the blackboard.
/// Returns Success if `min <= dist <= max`, Failure otherwise.
pub fn check_distance(
    name:      &str,
    pos_a_key: &str,
    pos_b_key: &str,
    min_dist:  f32,
    max_dist:  f32,
) -> BehaviorNode {
    let a = pos_a_key.to_string();
    let b = pos_b_key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            let pa = match bb.get_vec3(&a) { Some(v) => v, None => return NodeStatus::Failure };
            let pb = match bb.get_vec3(&b) { Some(v) => v, None => return NodeStatus::Failure };
            let d = (pa - pb).length();
            if d >= min_dist && d <= max_dist { NodeStatus::Success } else { NodeStatus::Failure }
        }),
        on_exit: None,
        entered: false,
    }
}

/// Condition: is the distance within `range` (i.e. `0 <= dist <= range`)?
pub fn check_in_range(
    name:     &str,
    pos_a_key: &str,
    pos_b_key: &str,
    range:    f32,
) -> BehaviorNode {
    check_distance(name, pos_a_key, pos_b_key, 0.0, range)
}

/// Condition: is the distance strictly outside `range`?
pub fn check_out_of_range(
    name:     &str,
    pos_a_key: &str,
    pos_b_key: &str,
    range:    f32,
) -> BehaviorNode {
    let a = pos_a_key.to_string();
    let b = pos_b_key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            let pa = match bb.get_vec3(&a) { Some(v) => v, None => return NodeStatus::Failure };
            let pb = match bb.get_vec3(&b) { Some(v) => v, None => return NodeStatus::Failure };
            let d = (pa - pb).length();
            if d > range { NodeStatus::Success } else { NodeStatus::Failure }
        }),
        on_exit: None,
        entered: false,
    }
}

// ── CheckHealth ───────────────────────────────────────────────────────────────

/// Condition: compare a health value on the blackboard against a threshold.
///
/// Reads `"{health_key}"` (float).  Applies `operator` comparison.
pub enum CompareOp { Lt, Lte, Gt, Gte, Eq }

/// Condition: `health_key OP threshold` → Success / Failure.
pub fn check_health(
    name:       &str,
    health_key: &str,
    op:         CompareOp,
    threshold:  f64,
) -> BehaviorNode {
    let k = health_key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            let h = match bb.get_float(&k) { Some(v) => v, None => return NodeStatus::Failure };
            let ok = match op {
                CompareOp::Lt  => h <  threshold,
                CompareOp::Lte => h <= threshold,
                CompareOp::Gt  => h >  threshold,
                CompareOp::Gte => h >= threshold,
                CompareOp::Eq  => (h - threshold).abs() < 1e-6,
            };
            if ok { NodeStatus::Success } else { NodeStatus::Failure }
        }),
        on_exit: None,
        entered: false,
    }
}

/// Shorthand: succeed if health < `threshold` (agent is wounded).
pub fn check_health_low(name: &str, health_key: &str, threshold: f64) -> BehaviorNode {
    check_health(name, health_key, CompareOp::Lt, threshold)
}

/// Shorthand: succeed if health >= `threshold` (agent is healthy).
pub fn check_health_ok(name: &str, health_key: &str, threshold: f64) -> BehaviorNode {
    check_health(name, health_key, CompareOp::Gte, threshold)
}

// ── CheckLineOfSight ──────────────────────────────────────────────────────────

/// Condition: can agent see target?
///
/// A simplified LOS check: reads the agent position and target position from
/// the blackboard, then tests a list of obstacle positions stored under
/// `"{obstacles_key}"` (a `BlackboardValue::List` of `Vec3` entries).
///
/// For each obstacle sphere (radius `obstacle_radius`), the check performs a
/// ray-sphere intersection.  If any obstacle blocks the ray, returns Failure.
///
/// For games with a custom LOS system, replace this function with one that
/// calls into your spatial query API — the signature is identical.
pub fn check_line_of_sight(
    name:           &str,
    agent_pos_key:  &str,
    target_pos_key: &str,
    obstacles_key:  &str,
    obstacle_radius: f32,
    max_range:      f32,
) -> BehaviorNode {
    let ap = agent_pos_key.to_string();
    let tp = target_pos_key.to_string();
    let ok = obstacles_key.to_string();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            let a = match bb.get_vec3(&ap) { Some(v) => v, None => return NodeStatus::Failure };
            let t = match bb.get_vec3(&tp) { Some(v) => v, None => return NodeStatus::Failure };

            let diff = t - a;
            let dist = diff.length();
            if dist > max_range { return NodeStatus::Failure; }
            if dist < 1e-5      { return NodeStatus::Success;  } // same position

            let dir = diff / dist;

            // Gather obstacle positions.
            let blocked = match bb.get(&ok) {
                Some(BlackboardValue::List(list)) => {
                    list.iter().any(|entry| {
                        if let BlackboardValue::Vec3(obs) = entry {
                            ray_sphere_intersect(a, dir, *obs, obstacle_radius, dist)
                        } else {
                            false
                        }
                    })
                }
                _ => false,
            };

            if blocked { NodeStatus::Failure } else { NodeStatus::Success }
        }),
        on_exit: None,
        entered: false,
    }
}

/// Ray-sphere intersection test.
/// Returns true if a sphere at `center` with `radius` blocks a ray from
/// `origin` in direction `dir` within `max_t` distance.
fn ray_sphere_intersect(origin: Vec3, dir: Vec3, center: Vec3, radius: f32, max_t: f32) -> bool {
    let oc = origin - center;
    let b  = 2.0 * oc.dot(dir);
    let c  = oc.dot(oc) - radius * radius;
    let disc = b * b - 4.0 * c;
    if disc < 0.0 { return false; }
    let sqrt_d = disc.sqrt();
    let t0 = (-b - sqrt_d) * 0.5;
    let t1 = (-b + sqrt_d) * 0.5;
    (t0 > 0.0 && t0 < max_t) || (t1 > 0.0 && t1 < max_t)
}

// ── CheckBlackboard ───────────────────────────────────────────────────────────

/// Condition: return Success if `key` holds a truthy bool.
pub fn check_blackboard_bool(name: &str, key: &str, expected: bool) -> BehaviorNode {
    let k = key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            let ok = bb.get_bool(&k).unwrap_or(!expected) == expected;
            if ok { NodeStatus::Success } else { NodeStatus::Failure }
        }),
        on_exit: None,
        entered: false,
    }
}

/// Condition: compare a float blackboard value.
pub fn check_blackboard_float(
    name:      &str,
    key:       &str,
    op:        CompareOp,
    threshold: f64,
) -> BehaviorNode {
    let k = key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            let v = match bb.get_float(&k) { Some(v) => v, None => return NodeStatus::Failure };
            let ok = match op {
                CompareOp::Lt  => v <  threshold,
                CompareOp::Lte => v <= threshold,
                CompareOp::Gt  => v >  threshold,
                CompareOp::Gte => v >= threshold,
                CompareOp::Eq  => (v - threshold).abs() < 1e-9,
            };
            if ok { NodeStatus::Success } else { NodeStatus::Failure }
        }),
        on_exit: None,
        entered: false,
    }
}

/// Condition: succeed if key is present in the blackboard.
pub fn check_blackboard_exists(name: &str, key: &str) -> BehaviorNode {
    let k = key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            if bb.contains(&k) { NodeStatus::Success } else { NodeStatus::Failure }
        }),
        on_exit: None,
        entered: false,
    }
}

// ── InvertDecorator ───────────────────────────────────────────────────────────

/// Wrap `child` in an Invert decorator: Success↔Failure, Running unchanged.
pub fn invert_node(name: &str, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::Invert,
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

// ── RepeatDecorator ───────────────────────────────────────────────────────────

/// Repeat `child` exactly `count` times; succeed only if all iterations succeed.
pub fn repeat_node(name: &str, count: u32, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::Repeat { count },
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

/// Repeat `child` forever (always returns Running).
pub fn repeat_forever(name: &str, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::RepeatForever,
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

// ── TimeoutDecorator ──────────────────────────────────────────────────────────

/// Return Failure if `child` is still Running after `timeout_secs` seconds.
pub fn timeout_node(name: &str, timeout_secs: f32, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::Timeout { timeout_secs },
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

/// Only tick `child` if at least `cooldown_secs` have elapsed since last tick.
pub fn cooldown_node(name: &str, cooldown_secs: f32, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::Cooldown { cooldown_secs },
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

/// Only tick child when a blackboard bool key equals `expected`.
pub fn blackboard_guard(name: &str, key: &str, expected: bool, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::BlackboardGuard { key: key.to_string(), expected },
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

// ── RandomSelector ────────────────────────────────────────────────────────────

/// A Selector that shuffles its children randomly on each activation, then
/// tries them in the shuffled order.  Because the shuffle is computed
/// per-activation (inside the tick closure), the node re-shuffles every time
/// the selector restarts.
///
/// The children are pre-built `BehaviorNode` instances passed in.  Internally
/// this wraps them in a custom Leaf that drives them manually, maintaining its
/// own cursor and shuffled order.
pub fn random_selector(name: &str, mut children: Vec<BehaviorNode>) -> BehaviorNode {
    // We need mutable state (cursor, shuffled order) that lives across ticks.
    // We box it all into the closure's captured environment.
    let order_key   = format!("__rselector_{name}_order");
    let cursor_key  = format!("__rselector_{name}_cursor");
    let entered_key = format!("__rselector_{name}_active");
    let n = children.len();

    // children is moved into the closure.
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: Some(Box::new({
            let ok  = order_key.clone();
            let ck  = cursor_key.clone();
            let ek  = entered_key.clone();
            move |bb: &mut Blackboard| {
                // Build initial [0,1,...,n-1] order list.
                let mut rng = rng_from_bb(bb);
                let mut indices: Vec<i64> = (0..n as i64).collect();
                rng.shuffle(&mut indices);
                let list: Vec<BlackboardValue> = indices.iter()
                    .map(|&i| BlackboardValue::Int(i))
                    .collect();
                bb.set(ok.as_str(), BlackboardValue::List(list));
                bb.set(ck.as_str(), 0i64);
                bb.set(ek.as_str(), true);
            }
        })),
        on_tick: Box::new({
            let order_key_tick = order_key.clone();
            let cursor_key_tick = cursor_key.clone();
            move |bb: &mut Blackboard, dt: f32| {
                let reg = SubtreeRegistry::new();
                loop {
                    let cursor = bb.get_int(&cursor_key_tick).unwrap_or(0) as usize;
                    if cursor >= n {
                        return NodeStatus::Failure;
                    }
                    let child_idx = match bb.get(&order_key_tick) {
                        Some(BlackboardValue::List(list)) => {
                            list.get(cursor)
                                .and_then(|v| v.as_int())
                                .unwrap_or(cursor as i64) as usize
                        }
                        _ => cursor,
                    };

                    if child_idx >= n { return NodeStatus::Failure; }

                    let status = children[child_idx].tick(dt, bb, &reg);
                    match status {
                        NodeStatus::Success => return NodeStatus::Success,
                        NodeStatus::Failure => {
                            bb.set(cursor_key_tick.as_str(), (cursor + 1) as i64);
                        }
                        NodeStatus::Running => return NodeStatus::Running,
                    }
                }
            }
        }),
        on_exit: Some(Box::new({
            let ok = order_key;
            let ck = cursor_key;
            let ek = entered_key;
            move |bb: &mut Blackboard, _: NodeStatus| {
                bb.remove(ok.as_str());
                bb.remove(ck.as_str());
                bb.remove(ek.as_str());
            }
        })),
        entered: false,
    }
}

// ── WeightedSelector ─────────────────────────────────────────────────────────

/// A Selector that picks exactly one child based on probability weights and
/// ticks only that child.  Each activation samples a fresh child.
///
/// `weights` must be the same length as `children`.  Weights need not sum to
/// 1.0 — they are normalized internally.
pub fn weighted_selector(
    name:     &str,
    mut children: Vec<BehaviorNode>,
    weights:  Vec<f32>,
) -> BehaviorNode {
    assert_eq!(children.len(), weights.len(),
               "weighted_selector: children and weights must be the same length");

    let n          = children.len();
    let chosen_key = format!("__wsel_{name}_chosen");

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: Some(Box::new({
            let ck      = chosen_key.clone();
            let ws      = weights.clone();
            move |bb: &mut Blackboard| {
                let mut rng  = rng_from_bb(bb);
                let total: f32 = ws.iter().sum();
                let sample = rng.next_f32() * total;
                let mut acc = 0.0f32;
                let mut chosen = 0usize;
                for (i, &w) in ws.iter().enumerate() {
                    acc += w;
                    if sample <= acc { chosen = i; break; }
                }
                bb.set(ck.as_str(), chosen as i64);
            }
        })),
        on_tick: Box::new(move |bb: &mut Blackboard, dt: f32| {
            let idx = bb.get_int(&chosen_key).unwrap_or(0) as usize;
            if idx >= n { return NodeStatus::Failure; }
            let reg = SubtreeRegistry::new();
            children[idx].tick(dt, bb, &reg)
        }),
        on_exit: None,
        entered: false,
    }
}

// ── DebugLog ──────────────────────────────────────────────────────────────────

/// A leaf that logs a message and always succeeds.  Useful for tracing tree
/// execution during development.
pub fn debug_log(name: &str, message: &str) -> BehaviorNode {
    let msg = message.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |_bb: &mut Blackboard, _dt: f32| {
            log::debug!("[BT] {msg}");
            NodeStatus::Success
        }),
        on_exit: None,
        entered: false,
    }
}

/// A leaf that logs the current value of a blackboard key and succeeds.
pub fn debug_log_blackboard(name: &str, key: &str) -> BehaviorNode {
    let k = key.to_string();
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            match bb.get(&k) {
                Some(v) => log::debug!("[BT] {k} = {v:?}"),
                None    => log::debug!("[BT] {k} = <absent>"),
            }
            NodeStatus::Success
        }),
        on_exit: None,
        entered: false,
    }
}

// ── Flee ─────────────────────────────────────────────────────────────────────

/// Move the agent away from a threat position stored on the blackboard.
///
/// Reads:
/// - `"{agent_pos_key}"` — current agent position (`Vec3`)
/// - `"{threat_key}"`    — threat position (`Vec3`)
///
/// Writes:
/// - `"{agent_pos_key}"` — updated position
///
/// Succeeds when the agent is more than `safe_distance` away.
pub fn flee(
    name:          &str,
    agent_pos_key: &str,
    threat_key:    &str,
    speed:         f32,
    safe_distance: f32,
) -> BehaviorNode {
    let pk = agent_pos_key.to_string();
    let tk = threat_key.to_string();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, dt: f32| {
            let pos    = match bb.get_vec3(&pk) { Some(v) => v, None => return NodeStatus::Failure };
            let threat = match bb.get_vec3(&tk) { Some(v) => v, None => return NodeStatus::Failure };

            let delta = pos - threat;
            let dist  = delta.length();
            if dist >= safe_distance { return NodeStatus::Success; }

            let dir = if dist > 1e-5 { delta / dist } else { Vec3::X };
            let new_pos = pos + dir * speed * dt;
            bb.set(pk.as_str(), new_pos);

            if (new_pos - threat).length() >= safe_distance {
                NodeStatus::Success
            } else {
                NodeStatus::Running
            }
        }),
        on_exit: None,
        entered: false,
    }
}

// ── Patrol ────────────────────────────────────────────────────────────────────

/// Move through a fixed list of waypoints in a loop.
///
/// Stores current waypoint index in `"{waypoint_idx_key}"`.
/// Writes the next target to `"{target_pos_key}"` and then delegates movement
/// to the blackboard consumer (i.e. pair this with `move_to`).
///
/// On each tick:
/// 1. Load current waypoint index from `"{waypoint_idx_key}"` (default 0).
/// 2. Write that waypoint's Vec3 into `"{target_pos_key}"`.
/// 3. If agent is within `arrival_radius`, advance the index and return Running.
/// 4. Otherwise return Running (movement handled externally).
pub fn patrol_set_target(
    name:            &str,
    waypoints:       Vec<Vec3>,
    waypoint_idx_key: &str,
    agent_pos_key:   &str,
    target_pos_key:  &str,
    arrival_radius:  f32,
) -> BehaviorNode {
    let wik = waypoint_idx_key.to_string();
    let apk = agent_pos_key.to_string();
    let tpk = target_pos_key.to_string();
    let n   = waypoints.len();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            if n == 0 { return NodeStatus::Failure; }

            let idx = (bb.get_int(&wik).unwrap_or(0) as usize) % n;
            let target = waypoints[idx];
            bb.set(tpk.as_str(), target);

            // Check if arrived at this waypoint.
            if let Some(pos) = bb.get_vec3(&apk) {
                if (pos - target).length() <= arrival_radius {
                    bb.set(wik.as_str(), ((idx + 1) % n) as i64);
                }
            }

            NodeStatus::Running // Patrol never "finishes"
        }),
        on_exit: None,
        entered: false,
    }
}

// ── FaceDirection ─────────────────────────────────────────────────────────────

/// Instantly snap the agent's yaw to face a target position.
///
/// Reads:
/// - `"{agent_pos_key}"` — Vec3
/// - `"{target_pos_key}"` — Vec3
///
/// Writes:
/// - `"{yaw_key}"` — f64 yaw angle in radians
pub fn face_direction(
    name:           &str,
    agent_pos_key:  &str,
    target_pos_key: &str,
    yaw_key:        &str,
) -> BehaviorNode {
    let ap = agent_pos_key.to_string();
    let tp = target_pos_key.to_string();
    let yk = yaw_key.to_string();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            let a = match bb.get_vec3(&ap) { Some(v) => v, None => return NodeStatus::Failure };
            let t = match bb.get_vec3(&tp) { Some(v) => v, None => return NodeStatus::Failure };
            let d = t - a;
            let yaw = d.z.atan2(d.x) as f64;
            bb.set(yk.as_str(), yaw);
            NodeStatus::Success
        }),
        on_exit: None,
        entered: false,
    }
}

// ── FireAtTarget ──────────────────────────────────────────────────────────────

/// Action: attempt to fire a projectile at a target.
///
/// Reads:
/// - `"{can_fire_key}"` — bool: is the weapon ready?
/// - `"{ammo_key}"`     — int: remaining ammo
///
/// Writes:
/// - `"{fire_request_key}"` — bool true (game system consumes this)
/// - `"{ammo_key}"`         — decremented by 1
///
/// Succeeds once the fire request is written.  Fails if can_fire is false or
/// ammo is zero.
pub fn fire_at_target(
    name:            &str,
    can_fire_key:    &str,
    ammo_key:        &str,
    fire_request_key: &str,
) -> BehaviorNode {
    let cfk = can_fire_key.to_string();
    let amk = ammo_key.to_string();
    let frk = fire_request_key.to_string();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            let can_fire = bb.get_bool(&cfk).unwrap_or(false);
            let ammo     = bb.get_int(&amk).unwrap_or(0);
            if !can_fire || ammo <= 0 { return NodeStatus::Failure; }
            bb.set(frk.as_str(), true);
            bb.set(amk.as_str(), ammo - 1);
            NodeStatus::Success
        }),
        on_exit: None,
        entered: false,
    }
}

// ── Melee Attack ──────────────────────────────────────────────────────────────

/// Action: perform a melee attack if the target is within `melee_range`.
///
/// Reads:
/// - `"{agent_pos_key}"`  — Vec3
/// - `"{target_pos_key}"` — Vec3
/// - `"{can_attack_key}"` — bool
///
/// Writes:
/// - `"{attack_request_key}"` — bool true
pub fn melee_attack(
    name:              &str,
    agent_pos_key:     &str,
    target_pos_key:    &str,
    can_attack_key:    &str,
    attack_request_key: &str,
    melee_range:       f32,
) -> BehaviorNode {
    let ap  = agent_pos_key.to_string();
    let tp  = target_pos_key.to_string();
    let cak = can_attack_key.to_string();
    let ark = attack_request_key.to_string();

    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(move |bb: &mut Blackboard, _dt: f32| {
            if !bb.get_bool(&cak).unwrap_or(false) { return NodeStatus::Failure; }
            let a = match bb.get_vec3(&ap) { Some(v) => v, None => return NodeStatus::Failure };
            let t = match bb.get_vec3(&tp) { Some(v) => v, None => return NodeStatus::Failure };
            if (a - t).length() > melee_range { return NodeStatus::Failure; }
            bb.set(ark.as_str(), true);
            NodeStatus::Success
        }),
        on_exit: None,
        entered: false,
    }
}

// ── Idle ─────────────────────────────────────────────────────────────────────

/// Always-running idle node.  Returns Running indefinitely.
pub fn idle(name: &str) -> BehaviorNode {
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(|_bb: &mut Blackboard, _dt: f32| NodeStatus::Running),
        on_exit: None,
        entered: false,
    }
}

/// A leaf that always succeeds immediately.
pub fn succeed_always(name: &str) -> BehaviorNode {
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(|_bb: &mut Blackboard, _dt: f32| NodeStatus::Success),
        on_exit: None,
        entered: false,
    }
}

/// A leaf that always fails immediately.
pub fn fail_always(name: &str) -> BehaviorNode {
    BehaviorNode::Leaf {
        name: name.to_string(),
        on_enter: None,
        on_tick: Box::new(|_bb: &mut Blackboard, _dt: f32| NodeStatus::Failure),
        on_exit: None,
        entered: false,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::tree::{Blackboard, SubtreeRegistry};

    fn tick(node: &mut BehaviorNode, bb: &mut Blackboard) -> NodeStatus {
        let reg = SubtreeRegistry::new();
        node.tick(0.016, bb, &reg)
    }

    #[test]
    fn wait_ticks_until_done() {
        let mut bb  = Blackboard::new();
        let mut node = wait("w", 0.1);
        // Need more than 6 ticks of 0.016s to exceed 0.1s.
        for _ in 0..6 {
            assert_eq!(tick(&mut node, &mut bb), NodeStatus::Running);
        }
        // At tick 7 (0.016*7 = 0.112s > 0.1) should succeed.
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Success);
    }

    #[test]
    fn move_to_reaches_target() {
        let mut bb = Blackboard::new();
        bb.set("pos", Vec3::ZERO);
        bb.set("target", Vec3::new(1.0, 0.0, 0.0));
        let mut node = move_to("m", "pos", "target", 10.0, 0.1);
        // With speed 10 and dt 0.016, should cover 1m in < 10 ticks.
        let mut reached = false;
        for _ in 0..20 {
            let s = tick(&mut node, &mut bb);
            if s == NodeStatus::Success { reached = true; break; }
        }
        assert!(reached);
    }

    #[test]
    fn check_distance_pass() {
        let mut bb = Blackboard::new();
        bb.set("a", Vec3::ZERO);
        bb.set("b", Vec3::new(3.0, 0.0, 0.0));
        let mut node = check_distance("cd", "a", "b", 0.0, 5.0);
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Success);
    }

    #[test]
    fn check_distance_fail() {
        let mut bb = Blackboard::new();
        bb.set("a", Vec3::ZERO);
        bb.set("b", Vec3::new(10.0, 0.0, 0.0));
        let mut node = check_distance("cd", "a", "b", 0.0, 5.0);
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Failure);
    }

    #[test]
    fn check_health_low_pass() {
        let mut bb = Blackboard::new();
        bb.set("hp", 20.0f64);
        let mut node = check_health_low("h", "hp", 50.0);
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Success);
    }

    #[test]
    fn check_health_low_fail() {
        let mut bb = Blackboard::new();
        bb.set("hp", 80.0f64);
        let mut node = check_health_low("h", "hp", 50.0);
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Failure);
    }

    #[test]
    fn set_blackboard_writes_value() {
        let mut bb = Blackboard::new();
        let mut node = set_blackboard("s", "flag", true);
        tick(&mut node, &mut bb);
        assert_eq!(bb.get_bool("flag"), Some(true));
    }

    #[test]
    fn invert_node_works() {
        let mut bb = Blackboard::new();
        let mut node = invert_node("inv", succeed_always("ok"));
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Failure);
    }

    #[test]
    fn check_bb_bool_true() {
        let mut bb = Blackboard::new();
        bb.set("ready", true);
        let mut node = check_blackboard_bool("c", "ready", true);
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Success);
    }

    #[test]
    fn los_unobstructed() {
        let mut bb = Blackboard::new();
        bb.set("agent", Vec3::ZERO);
        bb.set("target", Vec3::new(5.0, 0.0, 0.0));
        bb.set("obstacles", BlackboardValue::List(vec![]));
        let mut node = check_line_of_sight("los", "agent", "target", "obstacles", 0.5, 20.0);
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Success);
    }

    #[test]
    fn los_obstructed() {
        let mut bb = Blackboard::new();
        bb.set("agent", Vec3::ZERO);
        bb.set("target", Vec3::new(5.0, 0.0, 0.0));
        let obs = vec![BlackboardValue::Vec3(Vec3::new(2.5, 0.0, 0.0))];
        bb.set("obstacles", BlackboardValue::List(obs));
        let mut node = check_line_of_sight("los", "agent", "target", "obstacles", 0.5, 20.0);
        assert_eq!(tick(&mut node, &mut bb), NodeStatus::Failure);
    }
}
