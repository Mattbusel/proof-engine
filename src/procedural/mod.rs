//! Procedural content generation — dungeons, rooms, spawn tables, loot, names.
//!
//! This module drives the chaos-RPG world generation system. All content is
//! generated from seed-based deterministic random functions so worlds are
//! reproducible.
//!
//! ## Subsystems
//! - `dungeon`   — BSP dungeon floor layout
//! - `spawn`     — creature/item spawn tables with weighted probability
//! - `names`     — procedural name generation (markov chains)
//! - `loot`      — loot tables with rarity tiers
//! - `encounter` — encounter difficulty scaling

pub mod dungeon;
pub mod spawn;
pub mod names;
pub mod loot;
pub mod world;
pub mod items;

pub use dungeon::{DungeonFloor, Room, Corridor, DungeonTheme};
pub use spawn::{SpawnTable, SpawnEntry, SpawnResult};
pub use names::{NameGenerator, NameStyle};
pub use loot::{LootTable, LootTier, LootDrop};

// ── Seeded RNG ─────────────────────────────────────────────────────────────────

/// Lightweight seeded pseudo-random number generator (xoshiro256**).
/// Used throughout procedural generation for reproducibility.
#[derive(Clone, Debug)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Splitmix64 initialization to spread seed entropy
        let mut s = seed;
        let mut next = || {
            s = s.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^ (z >> 31)
        };
        Self { state: [next(), next(), next(), next()] }
    }

    fn rol64(x: u64, k: u32) -> u64 {
        (x << k) | (x >> (64 - k))
    }

    /// Next u64 (xoshiro256**).
    pub fn next_u64(&mut self) -> u64 {
        let result = Self::rol64(self.state[1].wrapping_mul(5), 7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = Self::rol64(self.state[3], 45);
        result
    }

    /// Next f32 in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32
    }

    /// Next f32 in `[min, max)`.
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    /// Next usize in `[0, n)`.
    pub fn range_usize(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Next i32 in `[min, max]`.
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        min + (self.next_u64() % ((max - min + 1) as u64)) as i32
    }

    /// Bernoulli: true with probability `p ∈ [0, 1]`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    /// Shuffle a slice in-place (Fisher-Yates).
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.range_usize(i + 1);
            slice.swap(i, j);
        }
    }

    /// Pick a random element from a slice.
    pub fn pick<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() { return None; }
        Some(&slice[self.range_usize(slice.len())])
    }

    /// Pick a random element from a weighted list. Weights can be any positive f32.
    pub fn pick_weighted<'a, T>(&mut self, items: &'a [(T, f32)]) -> Option<&'a T> {
        let total: f32 = items.iter().map(|(_, w)| *w).sum();
        if total <= 0.0 { return None; }
        let mut r = self.next_f32() * total;
        for (item, weight) in items {
            r -= weight;
            if r <= 0.0 { return Some(item); }
        }
        items.last().map(|(t, _)| t)
    }

    /// Gaussian sample with given mean and stddev (Box-Muller).
    pub fn gaussian(&mut self, mean: f32, stddev: f32) -> f32 {
        let u1 = self.next_f32().max(1e-10);
        let u2 = self.next_f32();
        let z  = (-2.0 * u1.ln()).sqrt() * (u2 * std::f32::consts::TAU).cos();
        mean + z * stddev
    }

    /// Poisson-distributed random integer with given lambda.
    pub fn poisson(&mut self, lambda: f32) -> u32 {
        let l = (-lambda).exp();
        let mut k = 0u32;
        let mut p = 1.0_f32;
        loop {
            k += 1;
            p *= self.next_f32();
            if p <= l { break; }
        }
        k - 1
    }

    /// Fork: create a child RNG seeded from this one (deterministic).
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u64())
    }
}
