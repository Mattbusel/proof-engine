//! Scene event system.

use crate::entity::EntityId;
use crate::glyph::GlyphId;

/// Types of scene events.
#[derive(Debug, Clone)]
pub enum EventKind {
    EntitySpawned(EntityId),
    EntityDespawned(EntityId),
    GlyphSpawned(GlyphId),
    GlyphDespawned(GlyphId),
    FieldAdded(u32),
    FieldRemoved(u32),
    SceneCleared,
    Custom(u32, f32),
}

/// A scene event with a timestamp.
#[derive(Debug, Clone)]
pub struct SceneEvent {
    pub kind: EventKind,
    pub time: f32,
}

/// Queue of scene events accumulated during a tick.
#[derive(Debug, Default)]
pub struct SceneEventQueue {
    events: Vec<SceneEvent>,
}

impl SceneEventQueue {
    pub fn new() -> Self { Self::default() }
    pub fn push(&mut self, e: SceneEvent) { self.events.push(e); }
    pub fn drain(&mut self) -> Vec<SceneEvent> { std::mem::take(&mut self.events) }
    pub fn clear(&mut self) { self.events.clear(); }
    pub fn len(&self) -> usize { self.events.len() }
    pub fn is_empty(&self) -> bool { self.events.is_empty() }
    pub fn iter(&self) -> impl Iterator<Item = &SceneEvent> { self.events.iter() }
}
