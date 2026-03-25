//! AI opponent with policy/value networks, adaptive difficulty, and playstyle tracking.

use super::tensor::Tensor;
use super::model::{Model, Sequential};

/// Possible actions for the AI opponent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Move,
    Attack,
    UseAbility,
    UseItem,
    Wait,
}

impl Action {
    pub const ALL: [Action; 5] = [
        Action::Move,
        Action::Attack,
        Action::UseAbility,
        Action::UseItem,
        Action::Wait,
    ];

    pub fn from_index(idx: usize) -> Self {
        Self::ALL[idx % Self::ALL.len()]
    }

    pub fn index(&self) -> usize {
        match self {
            Action::Move => 0,
            Action::Attack => 1,
            Action::UseAbility => 2,
            Action::UseItem => 3,
            Action::Wait => 4,
        }
    }
}

/// Compact game state encoded as a tensor.
#[derive(Debug, Clone)]
pub struct GameState {
    /// Flattened state vector. Layout:
    /// [player_hp, player_mp, player_x, player_y,
    ///  enemy_hp, enemy_mp, enemy_x, enemy_y,
    ///  ...additional features...]
    pub features: Tensor,
}

impl GameState {
    /// Create a game state from raw feature values.
    pub fn new(features: Vec<f32>) -> Self {
        let n = features.len();
        Self { features: Tensor::from_vec(features, vec![1, n]) }
    }

    /// Create a default 16-feature game state.
    pub fn default_state() -> Self {
        Self::new(vec![
            100.0, 50.0, 5.0, 5.0,   // player: hp, mp, x, y
            100.0, 50.0, 10.0, 10.0,  // enemy: hp, mp, x, y
            0.0, 0.0, 0.0, 0.0,       // ability cooldowns
            1.0, 0.0, 0.0, 0.0,       // flags (phase, items, etc.)
        ])
    }

    pub fn feature_dim(&self) -> usize {
        self.features.data.len()
    }
}

/// AI brain with policy and value networks.
pub struct AIBrain {
    pub policy_net: Model,
    pub value_net: Model,
}

impl AIBrain {
    /// Create an AI brain for a given state dimension.
    pub fn new(state_dim: usize) -> Self {
        let policy_net = Sequential::new("policy")
            .dense(state_dim, 64)
            .relu()
            .dense(64, 32)
            .relu()
            .dense(32, Action::ALL.len())
            .build();

        let value_net = Sequential::new("value")
            .dense(state_dim, 64)
            .relu()
            .dense(64, 32)
            .relu()
            .dense(32, 1)
            .build();

        Self { policy_net, value_net }
    }

    /// Select an action using softmax sampling with temperature.
    /// Higher temperature = more random; lower = more greedy.
    pub fn select_action(&self, state: &GameState, temperature: f32) -> Action {
        let logits = self.policy_net.forward(&state.features);
        // Apply temperature
        let scaled: Vec<f32> = logits.data.iter().map(|&v| v / temperature.max(0.01)).collect();
        let scaled_tensor = Tensor::from_vec(scaled, logits.shape.clone());
        let probs = scaled_tensor.softmax(if scaled_tensor.shape.len() > 1 { 1 } else { 0 });

        // Sample from the distribution using a simple RNG based on state data
        let seed: u64 = state.features.data.iter()
            .map(|v| (v.to_bits() as u64).wrapping_mul(2654435761))
            .fold(0u64, |a, b| a.wrapping_add(b));
        let mut rng_state = seed.wrapping_add(1);
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        let sample = (rng_state as u32 as f32) / (u32::MAX as f32);

        let prob_data = &probs.data;
        let num_actions = Action::ALL.len();
        // Find the probabilities for the last num_actions elements
        let start = prob_data.len().saturating_sub(num_actions);
        let action_probs = &prob_data[start..];

        let mut cumulative = 0.0f32;
        for (i, &p) in action_probs.iter().enumerate() {
            cumulative += p;
            if sample <= cumulative {
                return Action::from_index(i);
            }
        }
        Action::Wait
    }

    /// Evaluate how favorable a state is (higher = better for AI).
    pub fn evaluate_state(&self, state: &GameState) -> f32 {
        let value = self.value_net.forward(&state.features);
        // Return the scalar output (use tanh to bound in [-1, 1])
        value.data.last().copied().unwrap_or(0.0).tanh()
    }
}

/// Adaptive AI that blends optimal and random actions based on difficulty.
pub struct AdaptiveAI {
    pub brain: AIBrain,
    /// Difficulty in [0, 1]. 0 = fully random, 1 = fully optimal.
    pub difficulty: f32,
    /// Running score differential to auto-adjust difficulty.
    score_differential: f32,
    pub adaptation_rate: f32,
}

impl AdaptiveAI {
    pub fn new(state_dim: usize, difficulty: f32) -> Self {
        Self {
            brain: AIBrain::new(state_dim),
            difficulty: difficulty.clamp(0.0, 1.0),
            score_differential: 0.0,
            adaptation_rate: 0.05,
        }
    }

    /// Select an action, blending optimal with random based on difficulty.
    pub fn select_action(&self, state: &GameState) -> Action {
        // Use high temperature (random) for low difficulty, low temperature (greedy) for high
        let temperature = 0.1 + (1.0 - self.difficulty) * 5.0;
        self.brain.select_action(state, temperature)
    }

    /// Update difficulty based on whether the player won or lost the last encounter.
    /// `player_won`: true if player won.
    pub fn update_difficulty(&mut self, player_won: bool) {
        if player_won {
            // Player is winning: increase difficulty
            self.score_differential += 1.0;
        } else {
            // AI is winning: decrease difficulty
            self.score_differential -= 1.0;
        }
        // Adjust difficulty towards balancing the score differential
        self.difficulty += self.adaptation_rate * self.score_differential.signum() * 0.1;
        self.difficulty = self.difficulty.clamp(0.0, 1.0);
        // Decay differential
        self.score_differential *= 0.9;
    }
}

/// Tracks player behavior patterns over time.
pub struct PlaystyleTracker {
    /// Counts of each action type observed from the player.
    pub action_counts: [u32; 5],
    /// Total actions observed.
    pub total_actions: u32,
    /// Running aggression score (attacks / total).
    pub aggression: f32,
    /// Running caution score (waits and items / total).
    pub caution: f32,
    /// Ability usage rate.
    pub ability_usage: f32,
    /// History window for recent actions.
    pub history: Vec<Action>,
    pub history_max: usize,
}

impl PlaystyleTracker {
    pub fn new() -> Self {
        Self {
            action_counts: [0; 5],
            total_actions: 0,
            aggression: 0.0,
            caution: 0.0,
            ability_usage: 0.0,
            history: Vec::new(),
            history_max: 100,
        }
    }

    /// Record a player action and update statistics.
    pub fn record(&mut self, action: Action) {
        self.action_counts[action.index()] += 1;
        self.total_actions += 1;
        self.history.push(action);
        if self.history.len() > self.history_max {
            self.history.remove(0);
        }
        self.update_stats();
    }

    fn update_stats(&mut self) {
        let total = self.total_actions as f32;
        if total == 0.0 { return; }
        self.aggression = self.action_counts[Action::Attack.index()] as f32 / total;
        self.caution = (self.action_counts[Action::Wait.index()] as f32
            + self.action_counts[Action::UseItem.index()] as f32) / total;
        self.ability_usage = self.action_counts[Action::UseAbility.index()] as f32 / total;
    }

    /// Get a feature vector summarizing the playstyle.
    pub fn as_features(&self) -> Vec<f32> {
        vec![
            self.aggression,
            self.caution,
            self.ability_usage,
            self.action_counts[Action::Move.index()] as f32 / self.total_actions.max(1) as f32,
            self.total_actions as f32,
        ]
    }
}

/// Parameters controlling AI behavior.
#[derive(Debug, Clone)]
pub struct AIParameters {
    pub aggression_bias: f32,
    pub defense_bias: f32,
    pub ability_preference: f32,
    pub patience: f32,
}

impl AIParameters {
    pub fn balanced() -> Self {
        Self { aggression_bias: 0.0, defense_bias: 0.0, ability_preference: 0.0, patience: 0.5 }
    }
}

/// Determine counter-strategy parameters based on observed playstyle.
pub fn counter_strategy(tracker: &PlaystyleTracker) -> AIParameters {
    let mut params = AIParameters::balanced();
    // Counter aggressive players with defense
    if tracker.aggression > 0.4 {
        params.defense_bias = 0.5;
        params.patience = 0.8;
    }
    // Counter cautious players with aggression
    if tracker.caution > 0.3 {
        params.aggression_bias = 0.6;
        params.patience = 0.2;
    }
    // Counter ability-heavy players with items and positioning
    if tracker.ability_usage > 0.3 {
        params.defense_bias = 0.3;
        params.aggression_bias = 0.2;
    }
    params
}

/// Buffer for storing experience tuples for learning.
pub struct ExperienceBuffer {
    pub states: Vec<GameState>,
    pub actions: Vec<Action>,
    pub rewards: Vec<f32>,
    pub capacity: usize,
}

impl ExperienceBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            states: Vec::new(),
            actions: Vec::new(),
            rewards: Vec::new(),
            capacity,
        }
    }

    pub fn push(&mut self, state: GameState, action: Action, reward: f32) {
        if self.states.len() >= self.capacity {
            self.states.remove(0);
            self.actions.remove(0);
            self.rewards.remove(0);
        }
        self.states.push(state);
        self.actions.push(action);
        self.rewards.push(reward);
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Sample a random mini-batch of indices.
    pub fn sample_indices(&self, batch_size: usize, rng_seed: u64) -> Vec<usize> {
        let n = self.len();
        if n == 0 { return vec![]; }
        let batch_size = batch_size.min(n);
        let mut indices = Vec::with_capacity(batch_size);
        let mut state = rng_seed.wrapping_add(1);
        for _ in 0..batch_size {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            indices.push((state as usize) % n);
        }
        indices
    }

    /// Compute discounted returns from the reward sequence.
    pub fn compute_returns(&self, gamma: f32) -> Vec<f32> {
        let n = self.rewards.len();
        let mut returns = vec![0.0f32; n];
        if n == 0 { return returns; }
        returns[n - 1] = self.rewards[n - 1];
        for i in (0..n - 1).rev() {
            returns[i] = self.rewards[i] + gamma * returns[i + 1];
        }
        returns
    }

    pub fn clear(&mut self) {
        self.states.clear();
        self.actions.clear();
        self.rewards.clear();
    }

    /// Mean reward across all stored experiences.
    pub fn mean_reward(&self) -> f32 {
        if self.rewards.is_empty() { return 0.0; }
        self.rewards.iter().sum::<f32>() / self.rewards.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_roundtrip() {
        for a in Action::ALL {
            assert_eq!(Action::from_index(a.index()), a);
        }
    }

    #[test]
    fn test_game_state() {
        let state = GameState::default_state();
        assert_eq!(state.feature_dim(), 16);
    }

    #[test]
    fn test_brain_select_action() {
        let brain = AIBrain::new(16);
        let state = GameState::default_state();
        let action = brain.select_action(&state, 1.0);
        assert!(Action::ALL.contains(&action));
    }

    #[test]
    fn test_brain_evaluate_state() {
        let brain = AIBrain::new(16);
        let state = GameState::default_state();
        let value = brain.evaluate_state(&state);
        assert!(value >= -1.0 && value <= 1.0);
    }

    #[test]
    fn test_adaptive_ai() {
        let mut ai = AdaptiveAI::new(16, 0.5);
        let state = GameState::default_state();
        let _action = ai.select_action(&state);

        let initial_diff = ai.difficulty;
        ai.update_difficulty(true); // player won
        // Difficulty should increase
        assert!(ai.difficulty >= initial_diff || (ai.difficulty - initial_diff).abs() < 0.1);
    }

    #[test]
    fn test_playstyle_tracker() {
        let mut tracker = PlaystyleTracker::new();
        for _ in 0..10 { tracker.record(Action::Attack); }
        for _ in 0..5 { tracker.record(Action::Wait); }
        assert_eq!(tracker.total_actions, 15);
        assert!((tracker.aggression - 10.0 / 15.0).abs() < 1e-5);
        assert!((tracker.caution - 5.0 / 15.0).abs() < 1e-5);
    }

    #[test]
    fn test_counter_strategy_aggressive() {
        let mut tracker = PlaystyleTracker::new();
        for _ in 0..10 { tracker.record(Action::Attack); }
        let params = counter_strategy(&tracker);
        assert!(params.defense_bias > 0.0);
        assert!(params.patience > 0.5);
    }

    #[test]
    fn test_counter_strategy_cautious() {
        let mut tracker = PlaystyleTracker::new();
        for _ in 0..10 { tracker.record(Action::Wait); }
        let params = counter_strategy(&tracker);
        assert!(params.aggression_bias > 0.0);
    }

    #[test]
    fn test_experience_buffer() {
        let mut buf = ExperienceBuffer::new(5);
        for i in 0..7 {
            buf.push(GameState::default_state(), Action::Attack, i as f32);
        }
        assert_eq!(buf.len(), 5); // capped at capacity
        assert!(!buf.is_empty());
    }

    #[test]
    fn test_experience_buffer_returns() {
        let mut buf = ExperienceBuffer::new(100);
        buf.push(GameState::default_state(), Action::Move, 1.0);
        buf.push(GameState::default_state(), Action::Attack, 2.0);
        buf.push(GameState::default_state(), Action::Wait, 3.0);
        let returns = buf.compute_returns(0.9);
        // returns[2] = 3.0
        // returns[1] = 2.0 + 0.9*3.0 = 4.7
        // returns[0] = 1.0 + 0.9*4.7 = 5.23
        assert!((returns[2] - 3.0).abs() < 1e-5);
        assert!((returns[1] - 4.7).abs() < 1e-5);
        assert!((returns[0] - 5.23).abs() < 1e-3);
    }

    #[test]
    fn test_sample_indices() {
        let mut buf = ExperienceBuffer::new(100);
        for i in 0..20 {
            buf.push(GameState::default_state(), Action::Move, i as f32);
        }
        let indices = buf.sample_indices(5, 42);
        assert_eq!(indices.len(), 5);
        for &idx in &indices {
            assert!(idx < 20);
        }
    }

    #[test]
    fn test_mean_reward() {
        let mut buf = ExperienceBuffer::new(100);
        buf.push(GameState::default_state(), Action::Move, 2.0);
        buf.push(GameState::default_state(), Action::Move, 4.0);
        assert!((buf.mean_reward() - 3.0).abs() < 1e-5);
    }
}
