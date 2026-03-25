//! Markov chains: discrete-time and continuous-time.
//!
//! Provides transition matrix operations, stationary distribution computation
//! via power iteration, ergodicity/irreducibility checks, mean first passage
//! times, and a glyph-based renderer.

use super::brownian::Rng;
use glam::Vec2;

// ---------------------------------------------------------------------------
// MarkovChain (discrete-time)
// ---------------------------------------------------------------------------

/// Discrete-time Markov chain with a finite state space.
pub struct MarkovChain {
    /// Number of states.
    pub states: usize,
    /// Row-stochastic transition matrix: transition[i][j] = P(X_{n+1}=j | X_n=i).
    pub transition: Vec<Vec<f64>>,
}

impl MarkovChain {
    /// Create from a transition matrix. Rows must sum to 1.
    pub fn new(transition: Vec<Vec<f64>>) -> Self {
        let states = transition.len();
        Self { states, transition }
    }

    /// Create a random transition matrix.
    pub fn random(states: usize, rng: &mut Rng) -> Self {
        let mut transition = vec![vec![0.0; states]; states];
        for row in transition.iter_mut() {
            let raw: Vec<f64> = (0..states).map(|_| rng.uniform().max(0.01)).collect();
            let sum: f64 = raw.iter().sum();
            for (j, val) in raw.iter().enumerate() {
                row[j] = val / sum;
            }
        }
        Self { states, transition }
    }

    /// Validate that this is a proper stochastic matrix.
    pub fn is_stochastic(&self) -> bool {
        for row in &self.transition {
            if row.len() != self.states {
                return false;
            }
            let sum: f64 = row.iter().sum();
            if (sum - 1.0).abs() > 1e-6 {
                return false;
            }
            if row.iter().any(|&p| p < -1e-10) {
                return false;
            }
        }
        true
    }

    /// Single step: sample next state from current.
    pub fn step(&self, rng: &mut Rng, current_state: usize) -> usize {
        let u = rng.uniform();
        let row = &self.transition[current_state];
        let mut cumsum = 0.0;
        for (j, &p) in row.iter().enumerate() {
            cumsum += p;
            if u < cumsum {
                return j;
            }
        }
        self.states - 1
    }

    /// Simulate a trajectory of `steps` states starting from `initial`.
    pub fn simulate(&self, rng: &mut Rng, initial: usize, steps: usize) -> Vec<usize> {
        let mut path = Vec::with_capacity(steps + 1);
        path.push(initial);
        let mut current = initial;
        for _ in 0..steps {
            current = self.step(rng, current);
            path.push(current);
        }
        path
    }

    /// Compute stationary distribution via power iteration.
    /// Finds pi such that pi * P = pi.
    pub fn stationary_distribution(&self) -> Vec<f64> {
        let n = self.states;
        let mut pi = vec![1.0 / n as f64; n];
        let max_iter = 10_000;
        let tol = 1e-12;

        for _ in 0..max_iter {
            let mut next = vec![0.0; n];
            for j in 0..n {
                for i in 0..n {
                    next[j] += pi[i] * self.transition[i][j];
                }
            }
            // Normalize
            let sum: f64 = next.iter().sum();
            if sum > 0.0 {
                for v in next.iter_mut() {
                    *v /= sum;
                }
            }

            // Check convergence
            let diff: f64 = pi.iter().zip(next.iter()).map(|(a, b)| (a - b).abs()).sum();
            pi = next;
            if diff < tol {
                break;
            }
        }
        pi
    }

    /// Check if the chain is irreducible (all states reachable from all states).
    pub fn is_irreducible(&self) -> bool {
        let n = self.states;
        // Build adjacency and do BFS from each state
        for start in 0..n {
            let mut visited = vec![false; n];
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(start);
            visited[start] = true;
            while let Some(s) = queue.pop_front() {
                for (j, &p) in self.transition[s].iter().enumerate() {
                    if p > 0.0 && !visited[j] {
                        visited[j] = true;
                        queue.push_back(j);
                    }
                }
            }
            if visited.iter().any(|&v| !v) {
                return false;
            }
        }
        true
    }

    /// Check if the chain is ergodic (irreducible and aperiodic).
    /// Aperiodicity is checked by verifying gcd of return times = 1,
    /// which we approximate by checking if P^n has all positive entries for some n.
    pub fn is_ergodic(&self) -> bool {
        if !self.is_irreducible() {
            return false;
        }
        // Check aperiodicity: if any diagonal entry > 0, then aperiodic
        if self.transition.iter().enumerate().any(|(i, row)| row[i] > 0.0) {
            return true;
        }
        // Otherwise compute P^2 + P^3 and check if all entries > 0
        let p2 = mat_mul(&self.transition, &self.transition);
        let p3 = mat_mul(&p2, &self.transition);
        let combined = mat_add(&p2, &p3);
        combined.iter().all(|row| row.iter().all(|&v| v > 1e-15))
    }

    /// Find absorbing states (states i where P(i,i) = 1).
    pub fn absorbing_states(&self) -> Vec<usize> {
        (0..self.states)
            .filter(|&i| (self.transition[i][i] - 1.0).abs() < 1e-10)
            .collect()
    }

    /// Mean first passage time from state `from` to state `to`.
    /// Computed by solving the system: m_i = 1 + sum_{j != to} P(i,j) * m_j
    /// using iterative method.
    pub fn mean_first_passage(&self, from: usize, to: usize) -> f64 {
        if from == to {
            return 0.0;
        }
        let n = self.states;
        let mut m = vec![0.0; n];
        let max_iter = 50_000;
        let tol = 1e-10;

        for _ in 0..max_iter {
            let mut new_m = vec![0.0; n];
            let mut max_diff = 0.0;
            for i in 0..n {
                if i == to {
                    new_m[i] = 0.0;
                    continue;
                }
                let mut val = 1.0;
                for j in 0..n {
                    if j != to {
                        val += self.transition[i][j] * m[j];
                    }
                }
                new_m[i] = val;
                max_diff = max_diff.max((new_m[i] - m[i]).abs());
            }
            m = new_m;
            if max_diff < tol {
                break;
            }
        }
        m[from]
    }

    /// Compute the n-step transition matrix P^n.
    pub fn power(&self, n: usize) -> Vec<Vec<f64>> {
        let mut result = identity(self.states);
        let mut base = self.transition.clone();
        let mut exp = n;
        while exp > 0 {
            if exp % 2 == 1 {
                result = mat_mul(&result, &base);
            }
            base = mat_mul(&base, &base);
            exp /= 2;
        }
        result
    }

    /// Empirical stationary distribution from a long simulation.
    pub fn empirical_stationary(&self, rng: &mut Rng, steps: usize) -> Vec<f64> {
        let path = self.simulate(rng, 0, steps);
        let mut counts = vec![0usize; self.states];
        for &s in &path {
            counts[s] += 1;
        }
        let total = path.len() as f64;
        counts.iter().map(|&c| c as f64 / total).collect()
    }
}

// ---------------------------------------------------------------------------
// Matrix helpers
// ---------------------------------------------------------------------------

fn identity(n: usize) -> Vec<Vec<f64>> {
    let mut m = vec![vec![0.0; n]; n];
    for i in 0..n {
        m[i][i] = 1.0;
    }
    m
}

fn mat_mul(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let p = b[0].len();
    let k = b.len();
    let mut c = vec![vec![0.0; p]; n];
    for i in 0..n {
        for j in 0..p {
            for l in 0..k {
                c[i][j] += a[i][l] * b[l][j];
            }
        }
    }
    c
}

fn mat_add(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    a.iter()
        .zip(b.iter())
        .map(|(ra, rb)| ra.iter().zip(rb.iter()).map(|(x, y)| x + y).collect())
        .collect()
}

// ---------------------------------------------------------------------------
// ContinuousTimeMarkov
// ---------------------------------------------------------------------------

/// Continuous-time Markov chain defined by a generator matrix Q.
/// Q[i][j] >= 0 for i != j, Q[i][i] = -sum_{j!=i} Q[i][j].
pub struct ContinuousTimeMarkov {
    pub states: usize,
    pub generator: Vec<Vec<f64>>,
}

impl ContinuousTimeMarkov {
    pub fn new(generator: Vec<Vec<f64>>) -> Self {
        let states = generator.len();
        Self { states, generator }
    }

    /// Holding time in state i: Exp(-Q[i][i]).
    pub fn holding_time(&self, state: usize, rng: &mut Rng) -> f64 {
        let rate = -self.generator[state][state];
        if rate <= 0.0 {
            return f64::INFINITY; // absorbing state
        }
        let u = rng.uniform().max(1e-15);
        -u.ln() / rate
    }

    /// Jump probability from state i to state j (given that a jump occurs).
    pub fn jump_prob(&self, from: usize, to: usize) -> f64 {
        if from == to {
            return 0.0;
        }
        let rate = -self.generator[from][from];
        if rate <= 0.0 {
            return 0.0;
        }
        self.generator[from][to] / rate
    }

    /// Simulate the CTMC: returns Vec of (time, state).
    pub fn simulate(&self, rng: &mut Rng, initial: usize, duration: f64) -> Vec<(f64, usize)> {
        let mut path = Vec::new();
        let mut t = 0.0;
        let mut state = initial;
        path.push((t, state));

        loop {
            let hold = self.holding_time(state, rng);
            t += hold;
            if t > duration {
                break;
            }
            // Jump
            let u = rng.uniform();
            let mut cumsum = 0.0;
            let mut next_state = state;
            for j in 0..self.states {
                if j == state {
                    continue;
                }
                cumsum += self.jump_prob(state, j);
                if u < cumsum {
                    next_state = j;
                    break;
                }
            }
            state = next_state;
            path.push((t, state));
        }
        path
    }

    /// Compute the embedded discrete-time chain's transition matrix.
    pub fn embedded_chain(&self) -> MarkovChain {
        let n = self.states;
        let mut p = vec![vec![0.0; n]; n];
        for i in 0..n {
            let rate = -self.generator[i][i];
            if rate <= 0.0 {
                p[i][i] = 1.0; // absorbing
            } else {
                for j in 0..n {
                    if j != i {
                        p[i][j] = self.generator[i][j] / rate;
                    }
                }
            }
        }
        MarkovChain::new(p)
    }

    /// Stationary distribution via solving pi * Q = 0 using power iteration
    /// on the embedded chain weighted by holding times.
    pub fn stationary_distribution(&self) -> Vec<f64> {
        let embedded = self.embedded_chain();
        let pi_embedded = embedded.stationary_distribution();
        let n = self.states;

        // Weight by mean holding time (1 / -Q[i][i])
        let mut weighted = vec![0.0; n];
        for i in 0..n {
            let rate = -self.generator[i][i];
            if rate > 0.0 {
                weighted[i] = pi_embedded[i] / rate;
            }
        }
        let sum: f64 = weighted.iter().sum();
        if sum > 0.0 {
            for w in weighted.iter_mut() {
                *w /= sum;
            }
        }
        weighted
    }
}

// ---------------------------------------------------------------------------
// MarkovChainRenderer
// ---------------------------------------------------------------------------

/// Render Markov chain states as nodes and transitions as weighted edges.
pub struct MarkovChainRenderer {
    pub node_character: char,
    pub edge_character: char,
    pub node_color: [f32; 4],
    pub edge_color: [f32; 4],
    pub radius: f32,
}

impl MarkovChainRenderer {
    pub fn new() -> Self {
        Self {
            node_character: '●',
            edge_character: '→',
            node_color: [1.0, 0.8, 0.2, 1.0],
            edge_color: [0.5, 0.5, 0.8, 0.6],
            radius: 5.0,
        }
    }

    /// Arrange states in a circle and generate glyphs.
    pub fn render(&self, chain: &MarkovChain) -> Vec<(Vec2, char, [f32; 4])> {
        let n = chain.states;
        let mut glyphs = Vec::new();

        // Place nodes in a circle
        let positions: Vec<Vec2> = (0..n)
            .map(|i| {
                let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
                Vec2::new(self.radius * angle.cos(), self.radius * angle.sin())
            })
            .collect();

        // Draw nodes
        for &pos in &positions {
            glyphs.push((pos, self.node_character, self.node_color));
        }

        // Draw edges (sample points along lines for transitions with p > threshold)
        let threshold = 0.05;
        for i in 0..n {
            for j in 0..n {
                let p = chain.transition[i][j];
                if p > threshold && i != j {
                    let from = positions[i];
                    let to = positions[j];
                    let edge_steps = 5;
                    let alpha = (p as f32).min(1.0) * self.edge_color[3];
                    let color = [
                        self.edge_color[0],
                        self.edge_color[1],
                        self.edge_color[2],
                        alpha,
                    ];
                    for k in 1..edge_steps {
                        let t = k as f32 / edge_steps as f32;
                        let pos = from.lerp(to, t);
                        glyphs.push((pos, self.edge_character, color));
                    }
                }
            }
        }

        glyphs
    }

    /// Render a trajectory as a sequence of highlighted states over time.
    pub fn render_trajectory(
        &self,
        chain: &MarkovChain,
        trajectory: &[usize],
    ) -> Vec<(Vec2, char, [f32; 4])> {
        let mut glyphs = Vec::new();
        let n = chain.states;
        let positions: Vec<Vec2> = (0..n)
            .map(|i| {
                let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
                Vec2::new(self.radius * angle.cos(), self.radius * angle.sin())
            })
            .collect();

        for (step, &state) in trajectory.iter().enumerate() {
            let alpha = (step as f32 / trajectory.len() as f32).max(0.1);
            let color = [1.0, 0.3, 0.3, alpha];
            let offset = Vec2::new(step as f32 * 0.02, 0.0);
            glyphs.push((positions[state] + offset, '◆', color));
        }
        glyphs
    }
}

impl Default for MarkovChainRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_chain() -> MarkovChain {
        // Two-state chain
        MarkovChain::new(vec![vec![0.7, 0.3], vec![0.4, 0.6]])
    }

    #[test]
    fn test_is_stochastic() {
        let mc = simple_chain();
        assert!(mc.is_stochastic());
    }

    #[test]
    fn test_simulate_length() {
        let mc = simple_chain();
        let mut rng = Rng::new(42);
        let path = mc.simulate(&mut rng, 0, 100);
        assert_eq!(path.len(), 101);
    }

    #[test]
    fn test_stationary_distribution_sums_to_one() {
        let mc = simple_chain();
        let pi = mc.stationary_distribution();
        let sum: f64 = pi.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "stationary distribution should sum to 1, got {}",
            sum
        );
    }

    #[test]
    fn test_stationary_distribution_values() {
        // For [[0.7, 0.3], [0.4, 0.6]]:
        // pi = [4/7, 3/7] ≈ [0.5714, 0.4286]
        let mc = simple_chain();
        let pi = mc.stationary_distribution();
        assert!(
            (pi[0] - 4.0 / 7.0).abs() < 1e-4,
            "pi[0] should be ~4/7, got {}",
            pi[0]
        );
        assert!(
            (pi[1] - 3.0 / 7.0).abs() < 1e-4,
            "pi[1] should be ~3/7, got {}",
            pi[1]
        );
    }

    #[test]
    fn test_irreducible() {
        let mc = simple_chain();
        assert!(mc.is_irreducible());

        // Reducible: state 1 is absorbing
        let reducible = MarkovChain::new(vec![vec![0.5, 0.5], vec![0.0, 1.0]]);
        assert!(!reducible.is_irreducible());
    }

    #[test]
    fn test_ergodic() {
        let mc = simple_chain();
        assert!(mc.is_ergodic());
    }

    #[test]
    fn test_absorbing_states() {
        let mc = MarkovChain::new(vec![
            vec![0.5, 0.5, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.3, 0.0, 0.7],
        ]);
        let abs = mc.absorbing_states();
        assert_eq!(abs, vec![1]);
    }

    #[test]
    fn test_mean_first_passage() {
        let mc = simple_chain();
        let mfp = mc.mean_first_passage(0, 1);
        // Analytical: m_{0->1} = 1/0.3 = 3.333...
        assert!(
            (mfp - 1.0 / 0.3).abs() < 0.1,
            "mean first passage should be ~3.33, got {}",
            mfp
        );
    }

    #[test]
    fn test_power_matrix() {
        let mc = simple_chain();
        let p1 = mc.power(1);
        assert!((p1[0][0] - 0.7).abs() < 1e-10);

        let p2 = mc.power(2);
        // P^2[0][0] = 0.7*0.7 + 0.3*0.4 = 0.49 + 0.12 = 0.61
        assert!((p2[0][0] - 0.61).abs() < 1e-10);
    }

    #[test]
    fn test_ctmc_simulation() {
        let gen = vec![vec![-2.0, 2.0], vec![3.0, -3.0]];
        let ctmc = ContinuousTimeMarkov::new(gen);
        let mut rng = Rng::new(42);
        let path = ctmc.simulate(&mut rng, 0, 10.0);
        assert!(!path.is_empty());
        assert_eq!(path[0], (0.0, 0));
    }

    #[test]
    fn test_ctmc_stationary() {
        // Q = [[-2, 2], [3, -3]]
        // pi = [3/5, 2/5]
        let gen = vec![vec![-2.0, 2.0], vec![3.0, -3.0]];
        let ctmc = ContinuousTimeMarkov::new(gen);
        let pi = ctmc.stationary_distribution();
        assert!(
            (pi[0] - 0.6).abs() < 0.05,
            "CTMC pi[0] should be ~0.6, got {}",
            pi[0]
        );
        assert!(
            (pi[1] - 0.4).abs() < 0.05,
            "CTMC pi[1] should be ~0.4, got {}",
            pi[1]
        );
    }

    #[test]
    fn test_random_chain_is_stochastic() {
        let mut rng = Rng::new(42);
        let mc = MarkovChain::random(5, &mut rng);
        assert!(mc.is_stochastic());
    }

    #[test]
    fn test_renderer() {
        let mc = simple_chain();
        let renderer = MarkovChainRenderer::new();
        let glyphs = renderer.render(&mc);
        assert!(!glyphs.is_empty());
    }
}
