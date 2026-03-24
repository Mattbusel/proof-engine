//! Shared AI knowledge base — the Blackboard pattern.
//!
//! A `Blackboard` is a typed key-value store where each entry has an optional
//! time-to-live (TTL).  Multiple agents can share a single `SharedBlackboard`
//! (`Arc<RwLock<Blackboard>>`).  `BlackboardCondition` integrates with behavior
//! trees to guard transitions on blackboard values.
//!
//! # Example
//! ```rust
//! use proof_engine::ai::blackboard::{Blackboard, BlackboardValue, BlackboardCondition};
//!
//! let mut bb = Blackboard::new();
//! bb.set("enemy_health", BlackboardValue::Float(45.0));
//! bb.set_with_ttl("target_pos", BlackboardValue::Vec2(glam::Vec2::new(3.0, 4.0)), 5.0);
//!
//! // Check a condition
//! let cond = BlackboardCondition::KeyLessThan("enemy_health".into(), 50.0);
//! assert!(cond.evaluate(&bb));
//!
//! bb.update(6.0); // expire the target_pos entry
//! assert!(!bb.contains("target_pos"));
//! ```

use glam::{Vec2, Vec3};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// BlackboardValue
// ---------------------------------------------------------------------------

/// All value types that can be stored on a blackboard.
#[derive(Debug, Clone, PartialEq)]
pub enum BlackboardValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Vec2(Vec2),
    Vec3(Vec3),
    Str(String),
    Entity(u64),
    List(Vec<BlackboardValue>),
}

impl BlackboardValue {
    /// Attempt to extract a `f64` from Float, Int, or Bool.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            BlackboardValue::Float(v) => Some(*v),
            BlackboardValue::Int(v)   => Some(*v as f64),
            BlackboardValue::Bool(v)  => Some(if *v { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            BlackboardValue::Bool(v)  => Some(*v),
            BlackboardValue::Int(v)   => Some(*v != 0),
            BlackboardValue::Float(v) => Some(*v != 0.0),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            BlackboardValue::Int(v)   => Some(*v),
            BlackboardValue::Float(v) => Some(*v as i64),
            BlackboardValue::Bool(v)  => Some(if *v { 1 } else { 0 }),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            BlackboardValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_vec2(&self) -> Option<Vec2> {
        match self { BlackboardValue::Vec2(v) => Some(*v), _ => None }
    }

    pub fn as_vec3(&self) -> Option<Vec3> {
        match self { BlackboardValue::Vec3(v) => Some(*v), _ => None }
    }

    pub fn as_entity(&self) -> Option<u64> {
        match self { BlackboardValue::Entity(e) => Some(*e), _ => None }
    }

    pub fn as_list(&self) -> Option<&Vec<BlackboardValue>> {
        match self { BlackboardValue::List(l) => Some(l), _ => None }
    }

    /// Human-readable type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            BlackboardValue::Bool(_)   => "bool",
            BlackboardValue::Int(_)    => "int",
            BlackboardValue::Float(_)  => "float",
            BlackboardValue::Vec2(_)   => "vec2",
            BlackboardValue::Vec3(_)   => "vec3",
            BlackboardValue::Str(_)    => "str",
            BlackboardValue::Entity(_) => "entity",
            BlackboardValue::List(_)   => "list",
        }
    }
}

impl From<bool>   for BlackboardValue { fn from(v: bool)   -> Self { BlackboardValue::Bool(v) } }
impl From<i64>    for BlackboardValue { fn from(v: i64)    -> Self { BlackboardValue::Int(v) } }
impl From<i32>    for BlackboardValue { fn from(v: i32)    -> Self { BlackboardValue::Int(v as i64) } }
impl From<f64>    for BlackboardValue { fn from(v: f64)    -> Self { BlackboardValue::Float(v) } }
impl From<f32>    for BlackboardValue { fn from(v: f32)    -> Self { BlackboardValue::Float(v as f64) } }
impl From<Vec2>   for BlackboardValue { fn from(v: Vec2)   -> Self { BlackboardValue::Vec2(v) } }
impl From<Vec3>   for BlackboardValue { fn from(v: Vec3)   -> Self { BlackboardValue::Vec3(v) } }
impl From<String> for BlackboardValue { fn from(v: String) -> Self { BlackboardValue::Str(v) } }
impl From<&str>   for BlackboardValue { fn from(v: &str)   -> Self { BlackboardValue::Str(v.into()) } }
impl From<u64>    for BlackboardValue {
    fn from(v: u64) -> Self { BlackboardValue::Entity(v) }
}

// ---------------------------------------------------------------------------
// BlackboardEntry
// ---------------------------------------------------------------------------

/// A single entry on the blackboard, with optional expiry.
#[derive(Debug, Clone)]
pub struct BlackboardEntry {
    pub value: BlackboardValue,
    /// Simulated time when this entry was written.
    pub timestamp: f64,
    /// Optional time-to-live in seconds.  `None` = permanent.
    pub ttl: Option<f64>,
}

impl BlackboardEntry {
    pub fn new(value: BlackboardValue, timestamp: f64) -> Self {
        BlackboardEntry { value, timestamp, ttl: None }
    }

    pub fn with_ttl(mut self, ttl: f64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Returns `true` if this entry has expired at time `now`.
    pub fn is_expired(&self, now: f64) -> bool {
        match self.ttl {
            Some(ttl) => now > self.timestamp + ttl,
            None => false,
        }
    }

    /// Remaining time-to-live; returns 0.0 if already expired.
    pub fn remaining_ttl(&self, now: f64) -> f64 {
        match self.ttl {
            Some(ttl) => (self.timestamp + ttl - now).max(0.0),
            None => f64::INFINITY,
        }
    }
}

// ---------------------------------------------------------------------------
// Blackboard
// ---------------------------------------------------------------------------

/// A typed, expiring key-value knowledge base for AI agents.
#[derive(Debug, Clone, Default)]
pub struct Blackboard {
    pub entries: HashMap<String, BlackboardEntry>,
    pub current_time: f64,
}

impl Blackboard {
    pub fn new() -> Self { Blackboard::default() }

    /// Store a permanent entry.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<BlackboardValue>) {
        let entry = BlackboardEntry::new(value.into(), self.current_time);
        self.entries.insert(key.into(), entry);
    }

    /// Store an entry that expires after `ttl` seconds.
    pub fn set_with_ttl(
        &mut self,
        key: impl Into<String>,
        value: impl Into<BlackboardValue>,
        ttl: f64,
    ) {
        let entry = BlackboardEntry::new(value.into(), self.current_time).with_ttl(ttl);
        self.entries.insert(key.into(), entry);
    }

    /// Retrieve a value if it exists and has not expired.
    pub fn get(&self, key: &str) -> Option<&BlackboardValue> {
        self.entries.get(key).and_then(|e| {
            if e.is_expired(self.current_time) { None } else { Some(&e.value) }
        })
    }

    /// Retrieve the full entry (value + metadata).
    pub fn get_entry(&self, key: &str) -> Option<&BlackboardEntry> {
        self.entries.get(key).filter(|e| !e.is_expired(self.current_time))
    }

    // Typed accessors -----------------------------------------------------------

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key)?.as_bool()
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_int()
    }

    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_float()
    }

    pub fn get_vec2(&self, key: &str) -> Option<Vec2> {
        self.get(key)?.as_vec2()
    }

    pub fn get_vec3(&self, key: &str) -> Option<Vec3> {
        self.get(key)?.as_vec3()
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    pub fn get_entity(&self, key: &str) -> Option<u64> {
        self.get(key)?.as_entity()
    }

    // Mutation ------------------------------------------------------------------

    /// Remove a key from the blackboard.
    pub fn remove(&mut self, key: &str) -> Option<BlackboardValue> {
        self.entries.remove(key).map(|e| e.value)
    }

    /// Advance simulated time and remove all expired entries.
    pub fn update(&mut self, dt: f64) {
        self.current_time += dt;
        self.entries.retain(|_, e| !e.is_expired(self.current_time));
    }

    /// Set the current time directly (useful for tests).
    pub fn set_time(&mut self, t: f64) {
        self.current_time = t;
        self.entries.retain(|_, e| !e.is_expired(t));
    }

    // Query ---------------------------------------------------------------------

    pub fn contains(&self, key: &str) -> bool { self.get(key).is_some() }

    pub fn len(&self) -> usize {
        self.entries.values().filter(|e| !e.is_expired(self.current_time)).count()
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Iterate over non-expired keys.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter()
            .filter(|(_, e)| !e.is_expired(self.current_time))
            .map(|(k, _)| k)
    }

    /// Clear all entries.
    pub fn clear(&mut self) { self.entries.clear(); }

    /// Merge entries from `other` into this blackboard.
    /// Entries in `other` overwrite entries in `self` if the key already exists.
    pub fn merge(&mut self, other: &Blackboard) {
        for (k, e) in &other.entries {
            if !e.is_expired(other.current_time) {
                self.entries.insert(k.clone(), e.clone());
            }
        }
    }

    /// Increment an integer value, inserting 0 + delta if not present.
    pub fn increment_int(&mut self, key: &str, delta: i64) {
        let current = self.get_int(key).unwrap_or(0);
        self.set(key, BlackboardValue::Int(current + delta));
    }

    /// Increment a float value.
    pub fn increment_float(&mut self, key: &str, delta: f64) {
        let current = self.get_float(key).unwrap_or(0.0);
        self.set(key, BlackboardValue::Float(current + delta));
    }

    /// Append a value to a list entry; creates the list if not present.
    pub fn push_to_list(&mut self, key: &str, value: BlackboardValue) {
        let mut list = match self.get(key) {
            Some(BlackboardValue::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        list.push(value);
        self.set(key, BlackboardValue::List(list));
    }

    /// Returns all non-expired (key, value) pairs.
    pub fn snapshot(&self) -> Vec<(&String, &BlackboardValue)> {
        self.entries.iter()
            .filter(|(_, e)| !e.is_expired(self.current_time))
            .map(|(k, e)| (k, &e.value))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// SharedBlackboard
// ---------------------------------------------------------------------------

/// A thread-safe, reference-counted blackboard for cross-agent sharing.
#[derive(Debug, Clone)]
pub struct SharedBlackboard(pub Arc<RwLock<Blackboard>>);

impl SharedBlackboard {
    pub fn new() -> Self {
        SharedBlackboard(Arc::new(RwLock::new(Blackboard::new())))
    }

    pub fn set(&self, key: impl Into<String>, value: impl Into<BlackboardValue>) {
        if let Ok(mut bb) = self.0.write() {
            bb.set(key, value);
        }
    }

    pub fn set_with_ttl(&self, key: impl Into<String>, value: impl Into<BlackboardValue>, ttl: f64) {
        if let Ok(mut bb) = self.0.write() {
            bb.set_with_ttl(key, value, ttl);
        }
    }

    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.0.read().ok()?.get_float(key)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.0.read().ok()?.get_bool(key)
    }

    pub fn get_vec2(&self, key: &str) -> Option<Vec2> {
        self.0.read().ok()?.get_vec2(key)
    }

    pub fn get_entity(&self, key: &str) -> Option<u64> {
        self.0.read().ok()?.get_entity(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.0.read().map(|bb| bb.contains(key)).unwrap_or(false)
    }

    pub fn update(&self, dt: f64) {
        if let Ok(mut bb) = self.0.write() {
            bb.update(dt);
        }
    }

    pub fn remove(&self, key: &str) {
        if let Ok(mut bb) = self.0.write() {
            bb.remove(key);
        }
    }

    /// Create a snapshot of all non-expired values.
    pub fn read<T, F: FnOnce(&Blackboard) -> T>(&self, f: F) -> Option<T> {
        self.0.read().ok().map(|bb| f(&*bb))
    }

    pub fn write<T, F: FnOnce(&mut Blackboard) -> T>(&self, f: F) -> Option<T> {
        self.0.write().ok().map(|mut bb| f(&mut *bb))
    }
}

impl Default for SharedBlackboard {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// BlackboardCondition
// ---------------------------------------------------------------------------

/// A condition that can be evaluated against a `Blackboard`.
/// Primarily for use in behavior tree guard nodes.
#[derive(Debug, Clone, PartialEq)]
pub enum BlackboardCondition {
    /// True if the key exists (and has not expired).
    HasKey(String),
    /// True if the key does NOT exist (or has expired).
    NotHasKey(String),
    /// True if the value at key equals the given `BlackboardValue`.
    KeyEquals(String, BlackboardValue),
    /// True if the numeric value at key is strictly greater than `threshold`.
    KeyGreaterThan(String, f64),
    /// True if the numeric value at key is strictly less than `threshold`.
    KeyLessThan(String, f64),
    /// True if the numeric value at key is >= `min` and <= `max`.
    KeyInRange(String, f64, f64),
    /// True if the boolean at key is true.
    IsTrue(String),
    /// True if the boolean at key is false (or absent).
    IsFalse(String),
    /// Logical AND of two conditions.
    And(Box<BlackboardCondition>, Box<BlackboardCondition>),
    /// Logical OR of two conditions.
    Or(Box<BlackboardCondition>, Box<BlackboardCondition>),
    /// Logical NOT of a condition.
    Not(Box<BlackboardCondition>),
}

impl BlackboardCondition {
    /// Evaluate this condition against the given blackboard.
    pub fn evaluate(&self, bb: &Blackboard) -> bool {
        match self {
            BlackboardCondition::HasKey(k) => bb.contains(k),
            BlackboardCondition::NotHasKey(k) => !bb.contains(k),
            BlackboardCondition::KeyEquals(k, v) => bb.get(k) == Some(v),
            BlackboardCondition::KeyGreaterThan(k, threshold) => {
                bb.get_float(k).map(|f| f > *threshold).unwrap_or(false)
            }
            BlackboardCondition::KeyLessThan(k, threshold) => {
                bb.get_float(k).map(|f| f < *threshold).unwrap_or(false)
            }
            BlackboardCondition::KeyInRange(k, min, max) => {
                bb.get_float(k).map(|f| f >= *min && f <= *max).unwrap_or(false)
            }
            BlackboardCondition::IsTrue(k) => {
                bb.get_bool(k).unwrap_or(false)
            }
            BlackboardCondition::IsFalse(k) => {
                !bb.get_bool(k).unwrap_or(false)
            }
            BlackboardCondition::And(a, b) => a.evaluate(bb) && b.evaluate(bb),
            BlackboardCondition::Or(a, b)  => a.evaluate(bb) || b.evaluate(bb),
            BlackboardCondition::Not(c)    => !c.evaluate(bb),
        }
    }

    // --- Builder helpers -------------------------------------------------------

    pub fn and(self, other: BlackboardCondition) -> BlackboardCondition {
        BlackboardCondition::And(Box::new(self), Box::new(other))
    }

    pub fn or(self, other: BlackboardCondition) -> BlackboardCondition {
        BlackboardCondition::Or(Box::new(self), Box::new(other))
    }

    pub fn not(self) -> BlackboardCondition {
        BlackboardCondition::Not(Box::new(self))
    }
}

// ---------------------------------------------------------------------------
// BlackboardObserver — tracks writes so systems can react
// ---------------------------------------------------------------------------

/// A simple change-notification system for the blackboard.
#[derive(Debug, Clone, Default)]
pub struct BlackboardObserver {
    /// Keys to watch.
    watched_keys: Vec<String>,
    /// Last seen values per key.
    last_values: HashMap<String, BlackboardValue>,
    /// Keys that changed since last `drain_changes`.
    changed: Vec<String>,
}

impl BlackboardObserver {
    pub fn new() -> Self { BlackboardObserver::default() }

    /// Register a key to watch.
    pub fn watch(&mut self, key: impl Into<String>) {
        self.watched_keys.push(key.into());
    }

    /// Poll the blackboard for changes to watched keys.
    pub fn poll(&mut self, bb: &Blackboard) {
        for key in &self.watched_keys {
            let current = bb.get(key).cloned();
            let previous = self.last_values.get(key).cloned();
            if current != previous {
                self.changed.push(key.clone());
                match current {
                    Some(v) => { self.last_values.insert(key.clone(), v); }
                    None    => { self.last_values.remove(key); }
                }
            }
        }
    }

    /// Returns and clears the list of changed keys since last poll.
    pub fn drain_changes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.changed)
    }

    /// Check whether a key changed in the last poll.
    pub fn has_changed(&self, key: &str) -> bool {
        self.changed.iter().any(|k| k == key)
    }
}

// ---------------------------------------------------------------------------
// BlackboardSerializer — simple text-based debug dump
// ---------------------------------------------------------------------------

impl Blackboard {
    /// Dump all non-expired entries to a human-readable string.
    pub fn debug_dump(&self) -> String {
        let mut entries: Vec<_> = self.entries.iter()
            .filter(|(_, e)| !e.is_expired(self.current_time))
            .collect();
        entries.sort_by_key(|(k, _)| k.as_str());

        let mut out = String::from("Blackboard {\n");
        for (k, e) in &entries {
            let ttl_info = match e.ttl {
                Some(_) => format!(" [TTL: {:.2}s]", e.remaining_ttl(self.current_time)),
                None => String::new(),
            };
            out += &format!("  {} = {:?}{}\n", k, e.value, ttl_info);
        }
        out += "}";
        out
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn test_set_and_get() {
        let mut bb = Blackboard::new();
        bb.set("health", BlackboardValue::Float(100.0));
        assert_eq!(bb.get_float("health"), Some(100.0));
    }

    #[test]
    fn test_set_and_get_bool() {
        let mut bb = Blackboard::new();
        bb.set("alive", true);
        assert_eq!(bb.get_bool("alive"), Some(true));
    }

    #[test]
    fn test_set_and_get_int() {
        let mut bb = Blackboard::new();
        bb.set("kills", BlackboardValue::Int(5));
        assert_eq!(bb.get_int("kills"), Some(5));
    }

    #[test]
    fn test_set_and_get_vec2() {
        let mut bb = Blackboard::new();
        bb.set("pos", Vec2::new(1.0, 2.0));
        assert_eq!(bb.get_vec2("pos"), Some(Vec2::new(1.0, 2.0)));
    }

    #[test]
    fn test_set_and_get_entity() {
        let mut bb = Blackboard::new();
        bb.set("target", BlackboardValue::Entity(42));
        assert_eq!(bb.get_entity("target"), Some(42));
    }

    #[test]
    fn test_set_and_get_str() {
        let mut bb = Blackboard::new();
        bb.set("state", "patrolling");
        assert_eq!(bb.get_str("state"), Some("patrolling"));
    }

    #[test]
    fn test_ttl_expiry() {
        let mut bb = Blackboard::new();
        bb.set_with_ttl("temp", BlackboardValue::Float(1.0), 2.0);
        assert!(bb.contains("temp"));
        bb.update(3.0);
        assert!(!bb.contains("temp"), "entry should have expired");
    }

    #[test]
    fn test_ttl_not_yet_expired() {
        let mut bb = Blackboard::new();
        bb.set_with_ttl("temp", BlackboardValue::Float(1.0), 10.0);
        bb.update(5.0);
        assert!(bb.contains("temp"), "should still be valid");
    }

    #[test]
    fn test_permanent_entry_not_expired() {
        let mut bb = Blackboard::new();
        bb.set("permanent", BlackboardValue::Bool(true));
        bb.update(9999.0);
        assert!(bb.contains("permanent"));
    }

    #[test]
    fn test_remove() {
        let mut bb = Blackboard::new();
        bb.set("x", BlackboardValue::Int(1));
        assert!(bb.contains("x"));
        bb.remove("x");
        assert!(!bb.contains("x"));
    }

    #[test]
    fn test_len() {
        let mut bb = Blackboard::new();
        bb.set("a", BlackboardValue::Bool(true));
        bb.set("b", BlackboardValue::Bool(false));
        assert_eq!(bb.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut bb = Blackboard::new();
        bb.set("a", BlackboardValue::Bool(true));
        bb.clear();
        assert_eq!(bb.len(), 0);
    }

    #[test]
    fn test_merge() {
        let mut bb1 = Blackboard::new();
        bb1.set("a", BlackboardValue::Int(1));
        let mut bb2 = Blackboard::new();
        bb2.set("b", BlackboardValue::Int(2));
        bb2.set("a", BlackboardValue::Int(99)); // should overwrite
        bb1.merge(&bb2);
        assert_eq!(bb1.get_int("a"), Some(99));
        assert_eq!(bb1.get_int("b"), Some(2));
    }

    #[test]
    fn test_condition_has_key() {
        let mut bb = Blackboard::new();
        bb.set("x", BlackboardValue::Bool(true));
        assert!(BlackboardCondition::HasKey("x".into()).evaluate(&bb));
        assert!(!BlackboardCondition::HasKey("y".into()).evaluate(&bb));
    }

    #[test]
    fn test_condition_not_has_key() {
        let bb = Blackboard::new();
        assert!(BlackboardCondition::NotHasKey("missing".into()).evaluate(&bb));
    }

    #[test]
    fn test_condition_key_equals() {
        let mut bb = Blackboard::new();
        bb.set("mode", "attack");
        let cond = BlackboardCondition::KeyEquals("mode".into(), BlackboardValue::Str("attack".into()));
        assert!(cond.evaluate(&bb));
        let cond2 = BlackboardCondition::KeyEquals("mode".into(), BlackboardValue::Str("flee".into()));
        assert!(!cond2.evaluate(&bb));
    }

    #[test]
    fn test_condition_greater_than() {
        let mut bb = Blackboard::new();
        bb.set("health", BlackboardValue::Float(75.0));
        assert!(BlackboardCondition::KeyGreaterThan("health".into(), 50.0).evaluate(&bb));
        assert!(!BlackboardCondition::KeyGreaterThan("health".into(), 80.0).evaluate(&bb));
    }

    #[test]
    fn test_condition_less_than() {
        let mut bb = Blackboard::new();
        bb.set("health", BlackboardValue::Float(25.0));
        assert!(BlackboardCondition::KeyLessThan("health".into(), 50.0).evaluate(&bb));
        assert!(!BlackboardCondition::KeyLessThan("health".into(), 10.0).evaluate(&bb));
    }

    #[test]
    fn test_condition_in_range() {
        let mut bb = Blackboard::new();
        bb.set("ammo", BlackboardValue::Int(5));
        assert!(BlackboardCondition::KeyInRange("ammo".into(), 1.0, 10.0).evaluate(&bb));
        assert!(!BlackboardCondition::KeyInRange("ammo".into(), 6.0, 10.0).evaluate(&bb));
    }

    #[test]
    fn test_condition_is_true_false() {
        let mut bb = Blackboard::new();
        bb.set("armed", true);
        assert!(BlackboardCondition::IsTrue("armed".into()).evaluate(&bb));
        assert!(!BlackboardCondition::IsFalse("armed".into()).evaluate(&bb));
    }

    #[test]
    fn test_condition_and() {
        let mut bb = Blackboard::new();
        bb.set("a", true);
        bb.set("b", true);
        let cond = BlackboardCondition::IsTrue("a".into())
            .and(BlackboardCondition::IsTrue("b".into()));
        assert!(cond.evaluate(&bb));
        bb.set("b", false);
        assert!(!cond.evaluate(&bb));
    }

    #[test]
    fn test_condition_or() {
        let mut bb = Blackboard::new();
        bb.set("a", true);
        bb.set("b", false);
        let cond = BlackboardCondition::IsTrue("a".into())
            .or(BlackboardCondition::IsTrue("b".into()));
        assert!(cond.evaluate(&bb));
    }

    #[test]
    fn test_condition_not() {
        let mut bb = Blackboard::new();
        bb.set("enemy_visible", false);
        let cond = BlackboardCondition::IsTrue("enemy_visible".into()).not();
        assert!(cond.evaluate(&bb));
    }

    #[test]
    fn test_increment_int() {
        let mut bb = Blackboard::new();
        bb.increment_int("score", 10);
        bb.increment_int("score", 5);
        assert_eq!(bb.get_int("score"), Some(15));
    }

    #[test]
    fn test_increment_float() {
        let mut bb = Blackboard::new();
        bb.increment_float("damage", 12.5);
        bb.increment_float("damage", 7.5);
        assert!((bb.get_float("damage").unwrap() - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_push_to_list() {
        let mut bb = Blackboard::new();
        bb.push_to_list("log", BlackboardValue::Str("event1".into()));
        bb.push_to_list("log", BlackboardValue::Str("event2".into()));
        let list = bb.get("log").unwrap().as_list().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_shared_blackboard() {
        let shared = SharedBlackboard::new();
        shared.set("hp", BlackboardValue::Float(80.0));
        assert_eq!(shared.get_float("hp"), Some(80.0));
    }

    #[test]
    fn test_shared_blackboard_ttl_update() {
        let shared = SharedBlackboard::new();
        shared.set_with_ttl("temp", BlackboardValue::Bool(true), 1.0);
        assert!(shared.contains("temp"));
        shared.update(2.0);
        assert!(!shared.contains("temp"));
    }

    #[test]
    fn test_observer_poll() {
        let mut bb = Blackboard::new();
        let mut obs = BlackboardObserver::new();
        obs.watch("health");
        bb.set("health", BlackboardValue::Float(100.0));
        obs.poll(&bb);
        let changes = obs.drain_changes();
        assert!(changes.contains(&"health".to_string()));
    }

    #[test]
    fn test_observer_no_change() {
        let mut bb = Blackboard::new();
        bb.set("health", BlackboardValue::Float(100.0));
        let mut obs = BlackboardObserver::new();
        obs.watch("health");
        obs.poll(&bb); // first poll — sets last value
        obs.drain_changes();
        obs.poll(&bb); // nothing changed
        assert!(obs.drain_changes().is_empty());
    }

    #[test]
    fn test_debug_dump() {
        let mut bb = Blackboard::new();
        bb.set("x", BlackboardValue::Int(42));
        let dump = bb.debug_dump();
        assert!(dump.contains("x"));
    }

    #[test]
    fn test_snapshot() {
        let mut bb = Blackboard::new();
        bb.set("a", BlackboardValue::Bool(true));
        bb.set("b", BlackboardValue::Float(3.14));
        let snap = bb.snapshot();
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn test_value_from_impls() {
        let v: BlackboardValue = true.into();
        assert_eq!(v, BlackboardValue::Bool(true));
        let v: BlackboardValue = 42i64.into();
        assert_eq!(v, BlackboardValue::Int(42));
        let v: BlackboardValue = 3.14f64.into();
        assert_eq!(v, BlackboardValue::Float(3.14));
    }

    #[test]
    fn test_remaining_ttl() {
        let mut bb = Blackboard::new();
        bb.set_with_ttl("item", BlackboardValue::Bool(true), 10.0);
        let entry = bb.get_entry("item").unwrap();
        assert!((entry.remaining_ttl(0.0) - 10.0).abs() < 0.001);
        assert!((entry.remaining_ttl(5.0) - 5.0).abs() < 0.001);
        assert_eq!(entry.remaining_ttl(15.0), 0.0);
    }
}
