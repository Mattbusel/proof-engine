//! Event system for the ECS.
//!
//! Events use a **double-buffered** design: events sent in frame N are readable
//! in frame N and N+1. When [`Events::update`] is called (once per frame), the
//! old buffer is cleared and the current buffer becomes the old buffer.
//!
//! # Usage
//! ```rust,ignore
//! // In a system:
//! let mut events: &mut Events<CollisionEvent> = world.resource_mut();
//! events.send(CollisionEvent { a, b });
//!
//! // In another system (same or next frame):
//! let events: &Events<CollisionEvent> = world.resource();
//! let mut reader = ManualEventReader::default();
//! for ev in reader.read(events) {
//!     // handle event
//! }
//! ```

use std::marker::PhantomData;
use std::any::Any;

// ---------------------------------------------------------------------------
// EventId — unique event identifier
// ---------------------------------------------------------------------------

/// A monotonically increasing identifier assigned to each event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub u64);

impl EventId {
    pub const ZERO: EventId = EventId(0);
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventId({})", self.0)
    }
}

// ---------------------------------------------------------------------------
// EventInstance — event + metadata
// ---------------------------------------------------------------------------

/// An event value paired with its unique id.
#[derive(Debug, Clone)]
pub struct EventInstance<E> {
    pub id: EventId,
    pub event: E,
}

// ---------------------------------------------------------------------------
// Events<E> — the double-buffered event queue
// ---------------------------------------------------------------------------

/// Double-buffered event storage for event type `E`.
///
/// Register as a resource: `world.insert_resource(Events::<MyEvent>::default())`.
///
/// Call `Events::update()` once per frame in a `PreUpdate` system to rotate buffers.
#[derive(Debug)]
pub struct Events<E: 'static + Send + Sync> {
    /// Events sent in the current frame.
    current: Vec<EventInstance<E>>,
    /// Events sent in the previous frame (still readable).
    old: Vec<EventInstance<E>>,
    /// Total number of events ever sent (used as cursor by readers).
    event_count: u64,
    /// Event id counter.
    next_id: u64,
}

impl<E: 'static + Send + Sync> Events<E> {
    /// Create an empty event queue.
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            old: Vec::new(),
            event_count: 0,
            next_id: 0,
        }
    }

    /// Send one event.
    pub fn send(&mut self, event: E) {
        let id = EventId(self.next_id);
        self.next_id += 1;
        self.event_count += 1;
        self.current.push(EventInstance { id, event });
    }

    /// Send multiple events from an iterator.
    pub fn send_batch(&mut self, events: impl IntoIterator<Item = E>) {
        for event in events {
            self.send(event);
        }
    }

    /// Send an event and return its [`EventId`].
    pub fn send_with_id(&mut self, event: E) -> EventId {
        let id = EventId(self.next_id);
        self.send(event);
        id
    }

    /// Rotate buffers. Call once per frame at the start of `PreUpdate`.
    ///
    /// After this call:
    /// - Events from the previous `current` become `old`.
    /// - A fresh empty `current` is started.
    /// - Events from two frames ago are dropped.
    pub fn update(&mut self) {
        // Move current → old, clear current.
        std::mem::swap(&mut self.current, &mut self.old);
        self.current.clear();
    }

    /// Read all events visible since `last_event_count`.
    ///
    /// Returns events from both buffers that have id >= `last_event_count`.
    pub fn read_since(&self, last_event_count: u64) -> impl Iterator<Item = &EventInstance<E>> {
        self.old
            .iter()
            .chain(self.current.iter())
            .filter(move |ev| ev.id.0 >= last_event_count)
    }

    /// Drain all events from the current buffer (destructive read).
    pub fn drain(&mut self) -> impl Iterator<Item = E> + '_ {
        self.current.drain(..).map(|ev| ev.event)
    }

    /// Clear both buffers immediately.
    pub fn clear(&mut self) {
        self.current.clear();
        self.old.clear();
    }

    /// Number of events in the current buffer.
    pub fn current_len(&self) -> usize {
        self.current.len()
    }

    /// Number of events in the old buffer.
    pub fn old_len(&self) -> usize {
        self.old.len()
    }

    /// Total events ever sent.
    pub fn total_count(&self) -> u64 {
        self.event_count
    }

    /// Returns true if both buffers are empty.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty() && self.old.is_empty()
    }

    /// Get the next id that will be assigned.
    pub fn next_event_id(&self) -> EventId {
        EventId(self.next_id)
    }

    /// Get a specific event by id. O(n).
    pub fn get_by_id(&self, id: EventId) -> Option<&E> {
        self.old
            .iter()
            .chain(self.current.iter())
            .find(|ev| ev.id == id)
            .map(|ev| &ev.event)
    }

    /// Iterate over all currently buffered events (old + current).
    pub fn iter_all(&self) -> impl Iterator<Item = &EventInstance<E>> {
        self.old.iter().chain(self.current.iter())
    }
}

impl<E: 'static + Send + Sync> Default for Events<E> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EventWriter<'w, E>
// ---------------------------------------------------------------------------

/// Borrows `Events<E>` mutably and provides a write-only API.
/// In a system, you'd obtain this as a parameter.
pub struct EventWriter<'w, E: 'static + Send + Sync> {
    events: &'w mut Events<E>,
}

impl<'w, E: 'static + Send + Sync> EventWriter<'w, E> {
    pub fn new(events: &'w mut Events<E>) -> Self {
        Self { events }
    }

    /// Send a single event.
    pub fn send(&mut self, event: E) {
        self.events.send(event);
    }

    /// Send multiple events.
    pub fn send_batch(&mut self, events: impl IntoIterator<Item = E>) {
        self.events.send_batch(events);
    }

    /// Send and return the event id.
    pub fn send_with_id(&mut self, event: E) -> EventId {
        self.events.send_with_id(event)
    }

    /// Number of events sent this frame so far.
    pub fn pending_count(&self) -> usize {
        self.events.current_len()
    }
}

// ---------------------------------------------------------------------------
// EventReader<'w, E>
// ---------------------------------------------------------------------------

/// Borrows `Events<E>` immutably and tracks a read cursor.
///
/// The cursor is stored on the `EventReader` itself — if you want the cursor
/// to persist across frames, use [`ManualEventReader`] stored as a resource.
pub struct EventReader<'w, E: 'static + Send + Sync> {
    events: &'w Events<E>,
    last_event_id: u64,
}

impl<'w, E: 'static + Send + Sync> EventReader<'w, E> {
    /// Create a reader that will see all currently buffered events.
    pub fn new(events: &'w Events<E>) -> Self {
        Self {
            events,
            last_event_id: 0,
        }
    }

    /// Create a reader that starts at the current watermark (misses already-sent events).
    pub fn at_current(events: &'w Events<E>) -> Self {
        Self {
            events,
            last_event_id: events.next_id,
        }
    }

    /// Read all new events since the last read.
    pub fn read(&mut self) -> impl Iterator<Item = &E> {
        let last = self.last_event_id;
        if let Some(max_id) = self.events.iter_all().map(|ev| ev.id.0).max() {
            self.last_event_id = max_id + 1;
        }
        self.events
            .read_since(last)
            .map(|ev| &ev.event)
    }

    /// Returns true if there are no new events.
    pub fn is_empty(&self) -> bool {
        self.events.read_since(self.last_event_id).next().is_none()
    }

    /// Number of unread events.
    pub fn len(&self) -> usize {
        self.events.read_since(self.last_event_id).count()
    }

    /// Reset the reader to see all currently buffered events again.
    pub fn reset(&mut self) {
        self.last_event_id = 0;
    }
}

// ---------------------------------------------------------------------------
// ManualEventReader<E> — persistent reader
// ---------------------------------------------------------------------------

/// A persistent event reader that can be stored as a resource and survives
/// frame boundaries.
///
/// Unlike [`EventReader`] (which borrows Events), this stores the cursor
/// separately and must be passed the events on each read.
#[derive(Debug, Clone, Default)]
pub struct ManualEventReader<E: 'static + Send + Sync> {
    last_event_id: u64,
    _marker: PhantomData<E>,
}

impl<E: 'static + Send + Sync> ManualEventReader<E> {
    pub fn new() -> Self {
        Self {
            last_event_id: 0,
            _marker: PhantomData,
        }
    }

    /// Read new events from `events`, advancing the cursor.
    pub fn read<'a>(&mut self, events: &'a Events<E>) -> impl Iterator<Item = &'a E> {
        let last = self.last_event_id;
        // Advance cursor past all current events.
        if let Some(max_id) = events.iter_all().map(|ev| ev.id.0).max() {
            self.last_event_id = max_id + 1;
        }
        events.read_since(last).map(|ev| &ev.event)
    }

    /// Read without advancing the cursor (peek).
    pub fn peek<'a>(&self, events: &'a Events<E>) -> impl Iterator<Item = &'a E> {
        events.read_since(self.last_event_id).map(|ev| &ev.event)
    }

    /// Returns true if there are unread events.
    pub fn has_unread(&self, events: &Events<E>) -> bool {
        events.read_since(self.last_event_id).next().is_some()
    }

    /// Number of unread events.
    pub fn unread_count(&self, events: &Events<E>) -> usize {
        events.read_since(self.last_event_id).count()
    }

    /// Reset cursor to zero — will re-read all buffered events on next read.
    pub fn reset(&mut self) {
        self.last_event_id = 0;
    }

    /// Fast-forward to current time — will only see future events.
    pub fn catch_up(&mut self, events: &Events<E>) {
        self.last_event_id = events.next_id;
    }
}

// ---------------------------------------------------------------------------
// AnyEvents — type-erased event queue
// ---------------------------------------------------------------------------

/// Type-erased interface to an event queue. Used to store event queues in the
/// world's resource map without knowing the event type at compile time.
pub trait AnyEvents: Any + Send + Sync {
    /// Rotate the double-buffer. Call once per frame.
    fn update(&mut self);
    /// Clear all events.
    fn clear(&mut self);
    /// Total events ever sent.
    fn total_count(&self) -> u64;
    /// Currently buffered event count.
    fn buffered_count(&self) -> usize;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<E: 'static + Send + Sync> AnyEvents for Events<E> {
    fn update(&mut self) {
        self.update();
    }
    fn clear(&mut self) {
        self.clear();
    }
    fn total_count(&self) -> u64 {
        self.event_count
    }
    fn buffered_count(&self) -> usize {
        self.current_len() + self.old_len()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// EventQueue — registry of all event types (for World integration)
// ---------------------------------------------------------------------------

/// Holds all event queues, keyed by type id.
/// Stored as a resource in the World.
#[derive(Default)]
pub struct EventQueues {
    queues: std::collections::HashMap<std::any::TypeId, Box<dyn AnyEvents>>,
}

impl EventQueues {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an event type.
    pub fn register<E: 'static + Send + Sync>(&mut self) {
        self.queues
            .entry(std::any::TypeId::of::<E>())
            .or_insert_with(|| Box::new(Events::<E>::new()));
    }

    /// Get the event queue for `E`.
    pub fn get<E: 'static + Send + Sync>(&self) -> Option<&Events<E>> {
        self.queues
            .get(&std::any::TypeId::of::<E>())?
            .as_any()
            .downcast_ref::<Events<E>>()
    }

    /// Get the mutable event queue for `E`.
    pub fn get_mut<E: 'static + Send + Sync>(&mut self) -> Option<&mut Events<E>> {
        self.queues
            .get_mut(&std::any::TypeId::of::<E>())?
            .as_any_mut()
            .downcast_mut::<Events<E>>()
    }

    /// Get or create the event queue for `E`.
    pub fn get_or_create<E: 'static + Send + Sync>(&mut self) -> &mut Events<E> {
        self.queues
            .entry(std::any::TypeId::of::<E>())
            .or_insert_with(|| Box::new(Events::<E>::new()))
            .as_any_mut()
            .downcast_mut::<Events<E>>()
            .expect("EventQueues: type mismatch")
    }

    /// Send an event, registering the queue if needed.
    pub fn send<E: 'static + Send + Sync>(&mut self, event: E) {
        self.get_or_create::<E>().send(event);
    }

    /// Rotate all event queues. Call once per frame.
    pub fn update_all(&mut self) {
        for queue in self.queues.values_mut() {
            queue.update();
        }
    }

    /// Clear all queues.
    pub fn clear_all(&mut self) {
        for queue in self.queues.values_mut() {
            queue.clear();
        }
    }

    /// Number of registered event types.
    pub fn type_count(&self) -> usize {
        self.queues.len()
    }
}

impl std::fmt::Debug for EventQueues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventQueues(types={})", self.queues.len())
    }
}

// ---------------------------------------------------------------------------
// EventBus — convenience wrapper
// ---------------------------------------------------------------------------

/// A simple publish/subscribe event bus backed by a `Vec` of callbacks.
/// Useful when you want reactive callbacks rather than polling.
pub struct EventBus<E: Clone + 'static> {
    listeners: Vec<Box<dyn Fn(&E) + Send + Sync>>,
    history: Vec<E>,
    history_limit: usize,
}

impl<E: Clone + 'static> EventBus<E> {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
            history: Vec::new(),
            history_limit: 64,
        }
    }

    pub fn with_history_limit(mut self, limit: usize) -> Self {
        self.history_limit = limit;
        self
    }

    /// Subscribe a listener callback.
    pub fn subscribe(&mut self, listener: impl Fn(&E) + Send + Sync + 'static) {
        self.listeners.push(Box::new(listener));
    }

    /// Publish an event, calling all listeners.
    pub fn publish(&mut self, event: E) {
        for listener in &self.listeners {
            listener(&event);
        }
        if self.history.len() >= self.history_limit {
            self.history.remove(0);
        }
        self.history.push(event);
    }

    /// Recent event history.
    pub fn history(&self) -> &[E] {
        &self.history
    }

    /// Number of subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.listeners.len()
    }

    /// Clear all subscribers.
    pub fn clear_listeners(&mut self) {
        self.listeners.clear();
    }

    /// Clear history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

impl<E: Clone + 'static> Default for EventBus<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Clone + std::fmt::Debug + 'static> std::fmt::Debug for EventBus<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("listeners", &self.listeners.len())
            .field("history", &self.history.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Damage { amount: i32, source: u32 }

    #[derive(Debug, Clone, PartialEq)]
    struct SpawnEvent(String);

    #[test]
    fn test_send_and_read() {
        let mut events: Events<Damage> = Events::new();
        events.send(Damage { amount: 10, source: 1 });
        events.send(Damage { amount: 20, source: 2 });

        let mut reader = ManualEventReader::new();
        let collected: Vec<_> = reader.read(&events).cloned().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0], Damage { amount: 10, source: 1 });
        assert_eq!(collected[1], Damage { amount: 20, source: 2 });
    }

    #[test]
    fn test_double_buffer_update() {
        let mut events: Events<Damage> = Events::new();
        events.send(Damage { amount: 5, source: 0 });
        events.update(); // current → old

        // Still readable in next frame.
        let mut reader = ManualEventReader::new();
        let collected: Vec<_> = reader.read(&events).cloned().collect();
        assert_eq!(collected.len(), 1);

        events.update(); // old is now cleared

        let mut reader2 = ManualEventReader::new();
        let collected2: Vec<_> = reader2.read(&events).collect();
        assert_eq!(collected2.len(), 0);
    }

    #[test]
    fn test_reader_cursor_advances() {
        let mut events: Events<SpawnEvent> = Events::new();
        let mut reader = ManualEventReader::new();

        events.send(SpawnEvent("A".to_string()));
        let batch1: Vec<_> = reader.read(&events).cloned().collect();
        assert_eq!(batch1.len(), 1);

        events.send(SpawnEvent("B".to_string()));
        events.send(SpawnEvent("C".to_string()));
        let batch2: Vec<_> = reader.read(&events).cloned().collect();
        assert_eq!(batch2.len(), 2);

        // No new events.
        let batch3: Vec<_> = reader.read(&events).collect();
        assert_eq!(batch3.len(), 0);
    }

    #[test]
    fn test_send_batch() {
        let mut events: Events<i32> = Events::new();
        events.send_batch(vec![1, 2, 3, 4, 5]);
        assert_eq!(events.current_len(), 5);
        assert_eq!(events.total_count(), 5);
    }

    #[test]
    fn test_event_writer() {
        let mut events: Events<i32> = Events::new();
        {
            let mut writer = EventWriter::new(&mut events);
            writer.send(10);
            writer.send(20);
        }
        assert_eq!(events.current_len(), 2);
    }

    #[test]
    fn test_event_reader_is_empty() {
        let mut events: Events<i32> = Events::new();
        let mut reader = ManualEventReader::new();

        assert!(!reader.has_unread(&events)); // nothing yet
        events.send(42);
        assert!(reader.has_unread(&events));
        let _ = reader.read(&events).count(); // consume
        assert!(!reader.has_unread(&events));
    }

    #[test]
    fn test_event_id_monotonic() {
        let mut events: Events<i32> = Events::new();
        let id1 = events.send_with_id(1);
        let id2 = events.send_with_id(2);
        assert!(id2 > id1);
    }

    #[test]
    fn test_get_by_id() {
        let mut events: Events<String> = Events::new();
        let id = events.send_with_id("hello".to_string());
        assert_eq!(events.get_by_id(id), Some(&"hello".to_string()));
    }

    #[test]
    fn test_clear() {
        let mut events: Events<i32> = Events::new();
        events.send_batch(1..=10);
        events.clear();
        assert!(events.is_empty());
        assert_eq!(events.total_count(), 10); // count is not reset
    }

    #[test]
    fn test_event_queues() {
        let mut queues = EventQueues::new();
        queues.register::<i32>();
        queues.send::<i32>(42);
        queues.send::<i32>(99);

        let q = queues.get::<i32>().unwrap();
        assert_eq!(q.current_len(), 2);

        queues.update_all();
        let q2 = queues.get::<i32>().unwrap();
        assert_eq!(q2.current_len(), 0);
        assert_eq!(q2.old_len(), 2);
    }

    #[test]
    fn test_event_bus() {
        let mut bus: EventBus<i32> = EventBus::new();
        let collected = std::sync::Arc::new(std::sync::Mutex::new(Vec::<i32>::new()));
        let c = collected.clone();
        bus.subscribe(move |&v| { c.lock().unwrap().push(v); });

        bus.publish(1);
        bus.publish(2);
        bus.publish(3);

        let result = collected.lock().unwrap().clone();
        assert_eq!(result, vec![1, 2, 3]);
        assert_eq!(bus.history(), &[1, 2, 3]);
    }

    #[test]
    fn test_catch_up() {
        let mut events: Events<i32> = Events::new();
        events.send(1);
        events.send(2);

        let mut reader = ManualEventReader::new();
        reader.catch_up(&events); // skip existing events

        events.send(3);
        let seen: Vec<_> = reader.read(&events).copied().collect();
        assert_eq!(seen, vec![3]);
    }

    #[test]
    fn test_reader_reset() {
        let mut events: Events<i32> = Events::new();
        events.send(10);
        events.send(20);

        let mut reader = ManualEventReader::new();
        let _ = reader.read(&events).count();
        reader.reset();

        // After reset, sees all buffered events again.
        let seen: Vec<_> = reader.read(&events).copied().collect();
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn test_peek_does_not_advance() {
        let mut events: Events<i32> = Events::new();
        events.send(5);

        let reader = ManualEventReader::new();
        let peek1: Vec<_> = reader.peek(&events).copied().collect();
        let peek2: Vec<_> = reader.peek(&events).copied().collect();
        assert_eq!(peek1, vec![5]);
        assert_eq!(peek2, vec![5]); // cursor didn't advance
    }
}
