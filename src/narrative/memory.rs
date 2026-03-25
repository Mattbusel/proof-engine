//! NPC memory system — NPCs remember player actions and reference them.

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Memory {
    pub event: String,
    pub sentiment: f32,
    pub timestamp: f64,
    pub importance: f32,
    pub fading: bool,
}

/// An NPC's memory bank.
#[derive(Debug, Clone)]
pub struct NpcMemory {
    pub memories: VecDeque<Memory>,
    pub max_memories: usize,
    pub forget_threshold: f32,
}

impl NpcMemory {
    pub fn new(capacity: usize) -> Self {
        Self { memories: VecDeque::new(), max_memories: capacity, forget_threshold: 0.1 }
    }

    pub fn remember(&mut self, event: &str, sentiment: f32, importance: f32, time: f64) {
        self.memories.push_back(Memory {
            event: event.to_string(), sentiment, timestamp: time, importance, fading: false,
        });
        while self.memories.len() > self.max_memories {
            self.memories.pop_front();
        }
    }

    /// Decay old memories.
    pub fn tick(&mut self, current_time: f64) {
        for m in &mut self.memories {
            let age = current_time - m.timestamp;
            if age > 100.0 && m.importance < 0.5 {
                m.fading = true;
            }
        }
        self.memories.retain(|m| !m.fading || m.importance > self.forget_threshold);
    }

    /// Recall memories matching a keyword.
    pub fn recall(&self, keyword: &str) -> Vec<&Memory> {
        self.memories.iter().filter(|m| m.event.contains(keyword)).collect()
    }

    /// Most impactful memory.
    pub fn strongest_memory(&self) -> Option<&Memory> {
        self.memories.iter().max_by(|a, b| a.importance.partial_cmp(&b.importance).unwrap())
    }

    /// Overall sentiment toward the player.
    pub fn overall_sentiment(&self) -> f32 {
        if self.memories.is_empty() { return 0.0; }
        let sum: f32 = self.memories.iter().map(|m| m.sentiment * m.importance).sum();
        let weight: f32 = self.memories.iter().map(|m| m.importance).sum();
        if weight > 0.0 { sum / weight } else { 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory() {
        let mut mem = NpcMemory::new(10);
        mem.remember("helped with crops", 0.8, 0.5, 1.0);
        mem.remember("stole an apple", -0.5, 0.3, 2.0);
        assert_eq!(mem.memories.len(), 2);
        assert!(mem.overall_sentiment() > 0.0);
    }

    #[test]
    fn test_recall() {
        let mut mem = NpcMemory::new(10);
        mem.remember("battle at the bridge", 0.3, 0.7, 1.0);
        mem.remember("quiet evening", 0.1, 0.2, 2.0);
        let battles = mem.recall("battle");
        assert_eq!(battles.len(), 1);
    }
}
