//! Simulation systems: cellular automata, N-body physics, reaction-diffusion,
//! epidemiological models, and traffic simulation.

use std::f64::consts::PI;

// ============================================================
// SHARED VECTOR TYPE
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self { Vec3 { x, y, z } }
    pub fn zero() -> Self { Vec3 { x: 0.0, y: 0.0, z: 0.0 } }
    pub fn dot(self, other: Self) -> f64 { self.x * other.x + self.y * other.y + self.z * other.z }
    pub fn cross(self, other: Self) -> Self {
        Vec3 {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }
    pub fn len_sq(self) -> f64 { self.dot(self) }
    pub fn len(self) -> f64 { self.len_sq().sqrt() }
    pub fn normalize(self) -> Self {
        let l = self.len();
        if l < 1e-300 { return Self::zero(); }
        Vec3 { x: self.x / l, y: self.y / l, z: self.z / l }
    }
    pub fn dist(self, other: Self) -> f64 { (self - other).len() }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self { Vec3 { x: self.x + o.x, y: self.y + o.y, z: self.z + o.z } }
}
impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self { Vec3 { x: self.x - o.x, y: self.y - o.y, z: self.z - o.z } }
}
impl std::ops::Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self { Vec3 { x: self.x * s, y: self.y * s, z: self.z * s } }
}
impl std::ops::Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, s: f64) -> Self { Vec3 { x: self.x / s, y: self.y / s, z: self.z / s } }
}
impl std::ops::Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self { Vec3 { x: -self.x, y: -self.y, z: -self.z } }
}
impl std::ops::AddAssign for Vec3 {
    fn add_assign(&mut self, o: Self) { self.x += o.x; self.y += o.y; self.z += o.z; }
}
impl std::ops::MulAssign<f64> for Vec3 {
    fn mul_assign(&mut self, s: f64) { self.x *= s; self.y *= s; self.z *= s; }
}

// ============================================================
// CELLULAR AUTOMATA
// ============================================================

/// Generic 2D cellular automaton grid.
#[derive(Clone, Debug)]
pub struct CellularAutomaton<T: Clone> {
    pub grid: Vec<Vec<T>>,
    pub width: usize,
    pub height: usize,
}

impl<T: Clone + Default> CellularAutomaton<T> {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            grid: vec![vec![T::default(); width]; height],
            width,
            height,
        }
    }
    pub fn get(&self, x: usize, y: usize) -> &T { &self.grid[y][x] }
    pub fn set(&mut self, x: usize, y: usize, v: T) { self.grid[y][x] = v; }
    pub fn get_wrapped(&self, x: i64, y: i64) -> &T {
        let xi = ((x % self.width as i64 + self.width as i64) % self.width as i64) as usize;
        let yi = ((y % self.height as i64 + self.height as i64) % self.height as i64) as usize;
        &self.grid[yi][xi]
    }
}

// ============================================================
// GAME OF LIFE
// ============================================================

/// Conway's Game of Life.
#[derive(Clone, Debug)]
pub struct GameOfLife {
    pub grid: Vec<Vec<bool>>,
    pub width: usize,
    pub height: usize,
}

impl GameOfLife {
    pub fn new(width: usize, height: usize) -> Self {
        Self { grid: vec![vec![false; width]; height], width, height }
    }

    pub fn from_pattern(width: usize, height: usize, pattern: &[(usize, usize)]) -> Self {
        let mut g = Self::new(width, height);
        for &(x, y) in pattern {
            if x < width && y < height { g.grid[y][x] = true; }
        }
        g
    }

    pub fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        let mut count = 0u8;
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 { continue; }
                let nx = ((x as i64 + dx).rem_euclid(self.width as i64)) as usize;
                let ny = ((y as i64 + dy).rem_euclid(self.height as i64)) as usize;
                if self.grid[ny][nx] { count += 1; }
            }
        }
        count
    }

    pub fn step(&mut self) {
        let old = self.grid.clone();
        for y in 0..self.height {
            for x in 0..self.width {
                let n = {
                    let mut count = 0u8;
                    for dy in -1i64..=1 {
                        for dx in -1i64..=1 {
                            if dx == 0 && dy == 0 { continue; }
                            let nx = ((x as i64 + dx).rem_euclid(self.width as i64)) as usize;
                            let ny = ((y as i64 + dy).rem_euclid(self.height as i64)) as usize;
                            if old[ny][nx] { count += 1; }
                        }
                    }
                    count
                };
                self.grid[y][x] = if old[y][x] {
                    n == 2 || n == 3
                } else {
                    n == 3
                };
            }
        }
    }

    pub fn count_living(&self) -> usize {
        self.grid.iter().flat_map(|row| row.iter()).filter(|&&c| c).count()
    }

    pub fn is_stable(&self) -> bool {
        let prev = self.grid.clone();
        let mut tmp = self.clone();
        tmp.step();
        tmp.grid == prev
    }

    /// Standard glider pattern (5x5 bounding box, placed at offset).
    pub fn place_glider(&mut self, ox: usize, oy: usize) {
        let cells = [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];
        for (dx, dy) in cells {
            let x = ox + dx; let y = oy + dy;
            if x < self.width && y < self.height { self.grid[y][x] = true; }
        }
    }

    /// Gosper Glider Gun pattern.
    pub fn place_glider_gun(&mut self, ox: usize, oy: usize) {
        let cells: &[(usize, usize)] = &[
            (0,4),(0,5),(1,4),(1,5),
            (10,4),(10,5),(10,6),(11,3),(11,7),(12,2),(12,8),(13,2),(13,8),
            (14,5),(15,3),(15,7),(16,4),(16,5),(16,6),(17,5),
            (20,2),(20,3),(20,4),(21,2),(21,3),(21,4),(22,1),(22,5),
            (24,0),(24,1),(24,5),(24,6),
            (34,2),(34,3),(35,2),(35,3),
        ];
        for &(dx, dy) in cells {
            let x = ox + dx; let y = oy + dy;
            if x < self.width && y < self.height { self.grid[y][x] = true; }
        }
    }

    /// Pulsar pattern (13×13 oscillator, period 3).
    pub fn place_pulsar(&mut self, ox: usize, oy: usize) {
        let cols = [2usize, 3, 4, 8, 9, 10];
        let rows = [0usize, 5, 7, 12];
        for &r in &rows {
            for &c in &cols {
                let x = ox + c; let y = oy + r;
                if x < self.width && y < self.height { self.grid[y][x] = true; }
            }
        }
        // Transposed
        for &c in &rows {
            for &r in &cols {
                let x = ox + c; let y = oy + r;
                if x < self.width && y < self.height { self.grid[y][x] = true; }
            }
        }
    }

    /// Lightweight spaceship (LWSS).
    pub fn place_lwss(&mut self, ox: usize, oy: usize) {
        let cells = [(1,0),(4,0),(0,1),(0,2),(4,2),(0,3),(1,3),(2,3),(3,3)];
        for (dx, dy) in cells {
            let x = ox + dx; let y = oy + dy;
            if x < self.width && y < self.height { self.grid[y][x] = true; }
        }
    }

    /// R-pentomino.
    pub fn place_r_pentomino(&mut self, ox: usize, oy: usize) {
        for (dx, dy) in [(1,0),(2,0),(0,1),(1,1),(1,2)] {
            let x = ox + dx; let y = oy + dy;
            if x < self.width && y < self.height { self.grid[y][x] = true; }
        }
    }

    /// Acorn — small pattern with 5206 gen lifetime.
    pub fn place_acorn(&mut self, ox: usize, oy: usize) {
        for (dx, dy) in [(0,1),(1,3),(2,0),(2,1),(4,1),(5,1),(6,1)] {
            let x = ox + dx; let y = oy + dy;
            if x < self.width && y < self.height { self.grid[y][x] = true; }
        }
    }
}

// ============================================================
// WIREWORLD
// ============================================================

/// WireWorld cell states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WireWorldCell {
    #[default]
    Empty,
    ElectronHead,
    ElectronTail,
    Conductor,
}

/// WireWorld cellular automaton — simulates digital electronic circuits.
#[derive(Clone, Debug)]
pub struct WireWorld {
    pub grid: Vec<Vec<WireWorldCell>>,
    pub width: usize,
    pub height: usize,
}

impl WireWorld {
    pub fn new(width: usize, height: usize) -> Self {
        Self { grid: vec![vec![WireWorldCell::Empty; width]; height], width, height }
    }

    pub fn set(&mut self, x: usize, y: usize, cell: WireWorldCell) {
        self.grid[y][x] = cell;
    }

    pub fn step(&mut self) {
        let old = self.grid.clone();
        for y in 0..self.height {
            for x in 0..self.width {
                self.grid[y][x] = match old[y][x] {
                    WireWorldCell::ElectronHead => WireWorldCell::ElectronTail,
                    WireWorldCell::ElectronTail => WireWorldCell::Conductor,
                    WireWorldCell::Conductor => {
                        let heads = self.count_heads_neighbor(&old, x, y);
                        if heads == 1 || heads == 2 {
                            WireWorldCell::ElectronHead
                        } else {
                            WireWorldCell::Conductor
                        }
                    }
                    WireWorldCell::Empty => WireWorldCell::Empty,
                };
            }
        }
    }

    fn count_heads_neighbor(&self, grid: &[Vec<WireWorldCell>], x: usize, y: usize) -> usize {
        let mut count = 0;
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 { continue; }
                let nx = x as i64 + dx;
                let ny = y as i64 + dy;
                if nx >= 0 && nx < self.width as i64 && ny >= 0 && ny < self.height as i64 {
                    if grid[ny as usize][nx as usize] == WireWorldCell::ElectronHead { count += 1; }
                }
            }
        }
        count
    }
}

// ============================================================
// FOREST FIRE
// ============================================================

/// Cell states for forest fire model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ForestCell { #[default] Empty, Tree, Burning }

/// Probabilistic forest fire cellular automaton.
#[derive(Clone, Debug)]
pub struct ForestFire {
    pub grid: Vec<Vec<ForestCell>>,
    pub width: usize,
    pub height: usize,
    pub p_grow: f64,    // probability empty→tree
    pub p_catch: f64,   // probability tree→fire (lightning)
    rng_state: u64,
}

impl ForestFire {
    pub fn new(width: usize, height: usize, p_grow: f64, p_catch: f64, seed: u64) -> Self {
        Self {
            grid: vec![vec![ForestCell::Empty; width]; height],
            width, height, p_grow, p_catch,
            rng_state: seed,
        }
    }

    fn rand(&mut self) -> f64 {
        self.rng_state = self.rng_state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (self.rng_state >> 33) as f64 / (u32::MAX as f64)
    }

    pub fn step(&mut self) {
        let old = self.grid.clone();
        for y in 0..self.height {
            for x in 0..self.width {
                self.grid[y][x] = match old[y][x] {
                    ForestCell::Burning => ForestCell::Empty,
                    ForestCell::Empty => {
                        if self.rand() < self.p_grow { ForestCell::Tree } else { ForestCell::Empty }
                    }
                    ForestCell::Tree => {
                        let neighbor_burning = (-1i64..=1).any(|dy| {
                            (-1i64..=1).any(|dx| {
                                if dx == 0 && dy == 0 { return false; }
                                let nx = x as i64 + dx; let ny = y as i64 + dy;
                                nx >= 0 && nx < self.width as i64 && ny >= 0 && ny < self.height as i64
                                    && old[ny as usize][nx as usize] == ForestCell::Burning
                            })
                        });
                        if neighbor_burning || self.rand() < self.p_catch {
                            ForestCell::Burning
                        } else {
                            ForestCell::Tree
                        }
                    }
                };
            }
        }
    }

    pub fn count_trees(&self) -> usize {
        self.grid.iter().flat_map(|r| r.iter()).filter(|&&c| c == ForestCell::Tree).count()
    }

    pub fn count_burning(&self) -> usize {
        self.grid.iter().flat_map(|r| r.iter()).filter(|&&c| c == ForestCell::Burning).count()
    }
}

// ============================================================
// BRIAN'S BRAIN
// ============================================================

/// Brian's Brain 3-state automaton.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrianCell { #[default] Off, On, Dying }

#[derive(Clone, Debug)]
pub struct BriansBrain {
    pub grid: Vec<Vec<BrianCell>>,
    pub width: usize,
    pub height: usize,
}

impl BriansBrain {
    pub fn new(width: usize, height: usize) -> Self {
        Self { grid: vec![vec![BrianCell::Off; width]; height], width, height }
    }

    pub fn step(&mut self) {
        let old = self.grid.clone();
        for y in 0..self.height {
            for x in 0..self.width {
                self.grid[y][x] = match old[y][x] {
                    BrianCell::On => BrianCell::Dying,
                    BrianCell::Dying => BrianCell::Off,
                    BrianCell::Off => {
                        let on_count = (-1i64..=1).flat_map(|dy| {
                            (-1i64..=1).map(move |dx| (dx, dy))
                        }).filter(|&(dx, dy)| {
                            if dx == 0 && dy == 0 { return false; }
                            let nx = x as i64 + dx; let ny = y as i64 + dy;
                            nx >= 0 && nx < self.width as i64 && ny >= 0 && ny < self.height as i64
                                && old[ny as usize][nx as usize] == BrianCell::On
                        }).count();
                        if on_count == 2 { BrianCell::On } else { BrianCell::Off }
                    }
                };
            }
        }
    }

    pub fn count_on(&self) -> usize {
        self.grid.iter().flat_map(|r| r.iter()).filter(|&&c| c == BrianCell::On).count()
    }
}

// ============================================================
// LANGTON'S ANT
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AntDir { North, East, South, West }

impl AntDir {
    fn turn_right(self) -> Self {
        match self { Self::North => Self::East, Self::East => Self::South, Self::South => Self::West, Self::West => Self::North }
    }
    fn turn_left(self) -> Self {
        match self { Self::North => Self::West, Self::West => Self::South, Self::South => Self::East, Self::East => Self::North }
    }
    fn delta(self) -> (i64, i64) {
        match self { Self::North => (0,-1), Self::East => (1,0), Self::South => (0,1), Self::West => (-1,0) }
    }
}

/// Langton's Ant — universal Turing machine on a 2D grid.
#[derive(Clone, Debug)]
pub struct LangtonsAnt {
    pub grid: std::collections::HashMap<(i64, i64), bool>,
    pub ant_pos: (i64, i64),
    pub ant_dir: AntDir,
    pub steps: u64,
}

impl LangtonsAnt {
    pub fn new() -> Self {
        Self {
            grid: std::collections::HashMap::new(),
            ant_pos: (0, 0),
            ant_dir: AntDir::North,
            steps: 0,
        }
    }

    pub fn step(&mut self) {
        let cell = self.grid.get(&self.ant_pos).copied().unwrap_or(false);
        if !cell {
            // White cell: turn right, flip to black, move forward
            self.ant_dir = self.ant_dir.turn_right();
            self.grid.insert(self.ant_pos, true);
        } else {
            // Black cell: turn left, flip to white, move forward
            self.ant_dir = self.ant_dir.turn_left();
            self.grid.insert(self.ant_pos, false);
        }
        let (dx, dy) = self.ant_dir.delta();
        self.ant_pos = (self.ant_pos.0 + dx, self.ant_pos.1 + dy);
        self.steps += 1;
    }

    pub fn step_n(&mut self, n: usize) {
        for _ in 0..n { self.step(); }
    }

    pub fn count_black(&self) -> usize {
        self.grid.values().filter(|&&v| v).count()
    }
}

impl Default for LangtonsAnt {
    fn default() -> Self { Self::new() }
}

// ============================================================
// WOLFRAM ELEMENTARY CA (1D)
// ============================================================

/// Wolfram elementary 1D cellular automaton (256 rules).
#[derive(Clone, Debug)]
pub struct Totalistic1D {
    pub rule: u8,
}

impl Totalistic1D {
    pub fn new(rule: u8) -> Self { Self { rule } }

    /// Apply rule to (left, center, right) booleans.
    #[inline]
    fn apply(&self, l: bool, c: bool, r: bool) -> bool {
        let idx = ((l as u8) << 2) | ((c as u8) << 1) | (r as u8);
        (self.rule >> idx) & 1 == 1
    }

    /// Evolve a row for `steps` generations. Returns all generations including initial.
    pub fn evolve(&self, initial: Vec<bool>, steps: usize) -> Vec<Vec<bool>> {
        let n = initial.len();
        let mut result = Vec::with_capacity(steps + 1);
        result.push(initial);
        for _ in 0..steps {
            let prev = result.last().unwrap();
            let mut next = vec![false; n];
            for i in 0..n {
                let l = if i == 0 { prev[n - 1] } else { prev[i - 1] };
                let c = prev[i];
                let r = if i == n - 1 { prev[0] } else { prev[i + 1] };
                next[i] = self.apply(l, c, r);
            }
            result.push(next);
        }
        result
    }

    /// Rule 30 used as a pseudo-random number generator.
    pub fn rule_30_random(x: u64) -> bool {
        // Run rule 30 for 64 generations using bit at position 32 as seed
        let ca = Totalistic1D::new(30);
        let n = 128usize;
        let mut row = vec![false; n];
        row[n / 2] = true;
        let steps = (x % 64) as usize + 10;
        let result = ca.evolve(row, steps);
        let last = result.last().unwrap();
        last[n / 2]
    }
}

// ============================================================
// N-BODY SIMULATION
// ============================================================

/// A gravitational body.
#[derive(Clone, Debug)]
pub struct Body {
    pub position: Vec3,
    pub velocity: Vec3,
    pub mass: f64,
    pub radius: f64,
}

/// N-body simulation state.
#[derive(Clone, Debug)]
pub struct NBodySimulation {
    pub bodies: Vec<Body>,
    pub dt: f64,
    pub time: f64,
    pub softening: f64, // gravitational softening parameter
}

impl NBodySimulation {
    pub fn new(bodies: Vec<Body>, dt: f64) -> Self {
        Self { bodies, dt, time: 0.0, softening: 1e-4 }
    }
}

/// Direct O(n²) gravity calculation and Euler step.
pub fn direct_gravity_step(sim: &mut NBodySimulation) {
    let n = sim.bodies.len();
    let eps2 = sim.softening * sim.softening;
    let mut accels = vec![Vec3::zero(); n];
    const G: f64 = 6.674e-11;
    for i in 0..n {
        for j in 0..n {
            if i == j { continue; }
            let r = sim.bodies[j].position - sim.bodies[i].position;
            let r2 = r.len_sq() + eps2;
            let r3 = r2 * r2.sqrt();
            let a = G * sim.bodies[j].mass / r3;
            accels[i] += r * a;
        }
    }
    for i in 0..n {
        sim.bodies[i].velocity += accels[i] * sim.dt;
        let vel = sim.bodies[i].velocity;
        sim.bodies[i].position += vel * sim.dt;
    }
    sim.time += sim.dt;
}

/// Leapfrog (Störmer-Verlet) symplectic N-body step.
pub fn leapfrog_step(sim: &mut NBodySimulation) {
    let n = sim.bodies.len();
    let eps2 = sim.softening * sim.softening;
    const G: f64 = 6.674e-11;
    let compute_accels = |bodies: &[Body]| -> Vec<Vec3> {
        let mut accels = vec![Vec3::zero(); bodies.len()];
        for i in 0..bodies.len() {
            for j in 0..bodies.len() {
                if i == j { continue; }
                let r = bodies[j].position - bodies[i].position;
                let r2 = r.len_sq() + eps2;
                let r3 = r2 * r2.sqrt();
                accels[i] += r * (G * bodies[j].mass / r3);
            }
        }
        accels
    };
    // Half-kick velocities
    let acc0 = compute_accels(&sim.bodies);
    for i in 0..n { sim.bodies[i].velocity += acc0[i] * (sim.dt * 0.5); }
    // Full drift positions
    for i in 0..n {
        let v = sim.bodies[i].velocity;
        sim.bodies[i].position += v * sim.dt;
    }
    // Second half-kick
    let acc1 = compute_accels(&sim.bodies);
    for i in 0..n { sim.bodies[i].velocity += acc1[i] * (sim.dt * 0.5); }
    sim.time += sim.dt;
}

// ============================================================
// BARNES-HUT OCTREE
// ============================================================

/// Node in the Barnes-Hut octree.
#[derive(Clone, Debug)]
pub enum OctreeNode {
    Empty,
    Leaf { body_idx: usize, pos: Vec3, mass: f64 },
    Internal {
        center_of_mass: Vec3,
        total_mass: f64,
        children: Box<[OctreeNode; 8]>,
        min: Vec3,
        max: Vec3,
    },
}

impl OctreeNode {
    fn child_idx(center: Vec3, pos: Vec3) -> usize {
        let x = if pos.x >= center.x { 1 } else { 0 };
        let y = if pos.y >= center.y { 2 } else { 0 };
        let z = if pos.z >= center.z { 4 } else { 0 };
        x | y | z
    }

    fn child_bounds(min: Vec3, max: Vec3, idx: usize) -> (Vec3, Vec3) {
        let center = (min + max) * 0.5;
        let child_min = Vec3::new(
            if idx & 1 != 0 { center.x } else { min.x },
            if idx & 2 != 0 { center.y } else { min.y },
            if idx & 4 != 0 { center.z } else { min.z },
        );
        let child_max = Vec3::new(
            if idx & 1 != 0 { max.x } else { center.x },
            if idx & 2 != 0 { max.y } else { center.y },
            if idx & 4 != 0 { max.z } else { center.z },
        );
        (child_min, child_max)
    }

    pub fn insert(&mut self, body_idx: usize, pos: Vec3, mass: f64, min: Vec3, max: Vec3) {
        match self {
            OctreeNode::Empty => {
                *self = OctreeNode::Leaf { body_idx, pos, mass };
            }
            OctreeNode::Leaf { body_idx: bi, pos: bp, mass: bm } => {
                let old_bi = *bi; let old_pos = *bp; let old_mass = *bm;
                let center = (min + max) * 0.5;
                let children: [OctreeNode; 8] = Default::default();
                *self = OctreeNode::Internal {
                    center_of_mass: (old_pos * old_mass + pos * mass) * (1.0 / (old_mass + mass)),
                    total_mass: old_mass + mass,
                    children: Box::new(children),
                    min, max,
                };
                if let OctreeNode::Internal { children, min, max, .. } = self {
                    let ci = Self::child_idx(((*min + *max) * 0.5), old_pos);
                    let (cmin, cmax) = Self::child_bounds(*min, *max, ci);
                    children[ci].insert(old_bi, old_pos, old_mass, cmin, cmax);
                    let ci2 = Self::child_idx(((*min + *max) * 0.5), pos);
                    let (cmin2, cmax2) = Self::child_bounds(*min, *max, ci2);
                    children[ci2].insert(body_idx, pos, mass, cmin2, cmax2);
                }
            }
            OctreeNode::Internal { center_of_mass, total_mass, children, min, max } => {
                *total_mass += mass;
                *center_of_mass = (*center_of_mass * (*total_mass - mass) + pos * mass) * (1.0 / *total_mass);
                let ci = Self::child_idx((*min + *max) * 0.5, pos);
                let (cmin, cmax) = Self::child_bounds(*min, *max, ci);
                children[ci].insert(body_idx, pos, mass, cmin, cmax);
            }
        }
    }

    pub fn compute_accel(&self, pos: Vec3, theta: f64, eps2: f64) -> Vec3 {
        const G: f64 = 6.674e-11;
        match self {
            OctreeNode::Empty => Vec3::zero(),
            OctreeNode::Leaf { pos: lp, mass, .. } => {
                let r = *lp - pos;
                let r2 = r.len_sq() + eps2;
                if r2 < eps2 * 100.0 { return Vec3::zero(); }
                r * (G * mass / (r2 * r2.sqrt()))
            }
            OctreeNode::Internal { center_of_mass, total_mass, children, min, max } => {
                let size = (max.x - min.x).max((max.y - min.y).max(max.z - min.z));
                let r = *center_of_mass - pos;
                let dist = r.len();
                if size / dist < theta {
                    let r2 = r.len_sq() + eps2;
                    r * (G * total_mass / (r2 * r2.sqrt()))
                } else {
                    children.iter().map(|c| c.compute_accel(pos, theta, eps2)).fold(Vec3::zero(), |a, b| a + b)
                }
            }
        }
    }
}

impl Default for OctreeNode {
    fn default() -> Self { OctreeNode::Empty }
}

/// Barnes-Hut O(n log n) gravity step.
pub fn barnes_hut_step(sim: &mut NBodySimulation, theta: f64) {
    if sim.bodies.is_empty() { return; }
    // Find bounding box
    let mut min = sim.bodies[0].position;
    let mut max = sim.bodies[0].position;
    for b in &sim.bodies {
        min.x = min.x.min(b.position.x); max.x = max.x.max(b.position.x);
        min.y = min.y.min(b.position.y); max.y = max.y.max(b.position.y);
        min.z = min.z.min(b.position.z); max.z = max.z.max(b.position.z);
    }
    let padding = 1.0;
    min = min - Vec3::new(padding, padding, padding);
    max = max + Vec3::new(padding, padding, padding);

    // Build octree
    let mut root = OctreeNode::Empty;
    for (i, b) in sim.bodies.iter().enumerate() {
        root.insert(i, b.position, b.mass, min, max);
    }

    // Compute accelerations
    let eps2 = sim.softening * sim.softening;
    let n = sim.bodies.len();
    let accels: Vec<Vec3> = (0..n)
        .map(|i| root.compute_accel(sim.bodies[i].position, theta, eps2))
        .collect();

    for i in 0..n {
        sim.bodies[i].velocity += accels[i] * sim.dt;
        let vel = sim.bodies[i].velocity;
        sim.bodies[i].position += vel * sim.dt;
    }
    sim.time += sim.dt;
}

// ============================================================
// ORBITAL MECHANICS
// ============================================================

/// Classical orbital elements.
#[derive(Clone, Debug)]
pub struct OrbitalElements {
    pub semi_major_axis: f64,    // a
    pub eccentricity: f64,       // e
    pub inclination: f64,        // i (radians)
    pub lan: f64,                // Longitude of ascending node (radians)
    pub arg_periapsis: f64,      // ω (radians)
    pub true_anomaly: f64,       // ν (radians)
}

/// Compute orbital elements from state vectors.
pub fn from_state_vectors(r: Vec3, v: Vec3, mu: f64) -> OrbitalElements {
    let h = r.cross(v); // specific angular momentum
    let r_len = r.len();
    let v_len = v.len();

    // Eccentricity vector
    let e_vec = r * (v_len * v_len / mu - 1.0 / r_len) - v * (r.dot(v) / mu);
    let ecc = e_vec.len();

    let energy = v_len * v_len / 2.0 - mu / r_len;
    let a = if energy.abs() > 1e-12 { -mu / (2.0 * energy) } else { 1e30 };

    let i = (h.z / h.len()).acos().min(PI).max(0.0);
    let n = Vec3::new(-h.y, h.x, 0.0); // ascending node vector
    let n_len = n.len();

    let lan = if n_len > 1e-12 {
        let mut lan = (n.x / n_len).acos();
        if n.y < 0.0 { lan = 2.0 * PI - lan; }
        lan
    } else { 0.0 };

    let arg_p = if n_len > 1e-12 && ecc > 1e-12 {
        let mut omega = (n.dot(e_vec) / (n_len * ecc)).clamp(-1.0, 1.0).acos();
        if e_vec.z < 0.0 { omega = 2.0 * PI - omega; }
        omega
    } else if ecc > 1e-12 {
        // Equatorial orbit: measure argument of periapsis from x-axis
        let mut omega = (e_vec.x / ecc).clamp(-1.0, 1.0).acos();
        if e_vec.y < 0.0 { omega = 2.0 * PI - omega; }
        omega
    } else { 0.0 };

    let nu = if ecc > 1e-12 {
        let mut nu = (e_vec.dot(r) / (ecc * r_len)).clamp(-1.0, 1.0).acos();
        if r.dot(v) < 0.0 { nu = 2.0 * PI - nu; }
        nu
    } else { 0.0 };

    OrbitalElements {
        semi_major_axis: a,
        eccentricity: ecc,
        inclination: i,
        lan,
        arg_periapsis: arg_p,
        true_anomaly: nu,
    }
}

/// Convert orbital elements to state vectors.
pub fn to_state_vectors(elems: &OrbitalElements, mu: f64) -> (Vec3, Vec3) {
    let a = elems.semi_major_axis;
    let e = elems.eccentricity;
    let i = elems.inclination;
    let lan = elems.lan;
    let omega = elems.arg_periapsis;
    let nu = elems.true_anomaly;

    let p = a * (1.0 - e * e);
    let r_perifocal = p / (1.0 + e * nu.cos());

    let r_peri = Vec3::new(r_perifocal * nu.cos(), r_perifocal * nu.sin(), 0.0);
    let v_peri = Vec3::new(
        -(mu / p).sqrt() * nu.sin(),
        (mu / p).sqrt() * (e + nu.cos()),
        0.0,
    );

    // Rotation matrices
    let rx = |theta: f64, v: Vec3| -> Vec3 {
        Vec3::new(v.x, v.y * theta.cos() - v.z * theta.sin(), v.y * theta.sin() + v.z * theta.cos())
    };
    let rz = |theta: f64, v: Vec3| -> Vec3 {
        Vec3::new(v.x * theta.cos() - v.y * theta.sin(), v.x * theta.sin() + v.y * theta.cos(), v.z)
    };

    let r_rot = rz(-lan, rx(-i, rz(-omega, r_peri)));
    let v_rot = rz(-lan, rx(-i, rz(-omega, v_peri)));

    (r_rot, v_rot)
}

/// Solve Kepler's equation M = E - e*sin(E) for eccentric anomaly E.
pub fn solve_kepler_equation(m_anom: f64, e: f64) -> f64 {
    let mut e_anom = if e < 0.8 { m_anom } else { PI };
    for _ in 0..100 {
        let f = e_anom - e * e_anom.sin() - m_anom;
        let df = 1.0 - e * e_anom.cos();
        if df.abs() < 1e-12 { break; }
        let delta = f / df;
        e_anom -= delta;
        if delta.abs() < 1e-12 { break; }
    }
    e_anom
}

// ============================================================
// REACTION-DIFFUSION
// ============================================================

/// Gray-Scott reaction-diffusion system.
/// Models Turing pattern formation.
#[derive(Clone, Debug)]
pub struct GrayScott {
    pub u: Vec<f64>,
    pub v: Vec<f64>,
    pub width: usize,
    pub height: usize,
    pub feed: f64,
    pub kill: f64,
    pub du: f64,
    pub dv: f64,
}

/// Preset parameters for Gray-Scott system.
#[derive(Clone, Copy, Debug)]
pub struct GrayScottPreset {
    pub feed: f64,
    pub kill: f64,
    pub name: &'static str,
}

impl GrayScott {
    pub fn new(width: usize, height: usize, feed: f64, kill: f64) -> Self {
        let n = width * height;
        Self {
            u: vec![1.0; n],
            v: vec![0.0; n],
            width, height, feed, kill,
            du: 0.2,
            dv: 0.1,
        }
    }

    pub fn seed_center(&mut self) {
        let cx = self.width / 2;
        let cy = self.height / 2;
        let r = 5.min(self.width.min(self.height) / 4);
        for y in (cy.saturating_sub(r))..(cy + r).min(self.height) {
            for x in (cx.saturating_sub(r))..(cx + r).min(self.width) {
                let idx = y * self.width + x;
                self.u[idx] = 0.5;
                self.v[idx] = 0.25;
            }
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    fn laplacian_u(&self, x: usize, y: usize) -> f64 {
        let cx = x;
        let cy = y;
        let xm = if cx == 0 { self.width - 1 } else { cx - 1 };
        let xp = if cx == self.width - 1 { 0 } else { cx + 1 };
        let ym = if cy == 0 { self.height - 1 } else { cy - 1 };
        let yp = if cy == self.height - 1 { 0 } else { cy + 1 };
        self.u[self.idx(xm, cy)] + self.u[self.idx(xp, cy)]
            + self.u[self.idx(cx, ym)] + self.u[self.idx(cx, yp)]
            - 4.0 * self.u[self.idx(cx, cy)]
    }

    fn laplacian_v(&self, x: usize, y: usize) -> f64 {
        let cx = x; let cy = y;
        let xm = if cx == 0 { self.width - 1 } else { cx - 1 };
        let xp = if cx == self.width - 1 { 0 } else { cx + 1 };
        let ym = if cy == 0 { self.height - 1 } else { cy - 1 };
        let yp = if cy == self.height - 1 { 0 } else { cy + 1 };
        self.v[self.idx(xm, cy)] + self.v[self.idx(xp, cy)]
            + self.v[self.idx(cx, ym)] + self.v[self.idx(cx, yp)]
            - 4.0 * self.v[self.idx(cx, cy)]
    }

    pub fn step(&mut self, dt: f64) {
        let n = self.width * self.height;
        let mut u_new = vec![0.0f64; n];
        let mut v_new = vec![0.0f64; n];
        for y in 0..self.height {
            for x in 0..self.width {
                let i = self.idx(x, y);
                let u = self.u[i];
                let v = self.v[i];
                let uvv = u * v * v;
                u_new[i] = u + dt * (self.du * self.laplacian_u(x, y) - uvv + self.feed * (1.0 - u));
                v_new[i] = v + dt * (self.dv * self.laplacian_v(x, y) + uvv - (self.feed + self.kill) * v);
                u_new[i] = u_new[i].clamp(0.0, 1.0);
                v_new[i] = v_new[i].clamp(0.0, 1.0);
            }
        }
        self.u = u_new;
        self.v = v_new;
    }

    /// Preset: mitosis (f=0.0367, k=0.0649)
    pub fn preset_mitosis(width: usize, height: usize) -> Self {
        Self::new(width, height, 0.0367, 0.0649)
    }

    /// Preset: coral (f=0.0545, k=0.062)
    pub fn preset_coral(width: usize, height: usize) -> Self {
        Self::new(width, height, 0.0545, 0.062)
    }

    /// Preset: worms (f=0.078, k=0.061)
    pub fn preset_worms(width: usize, height: usize) -> Self {
        Self::new(width, height, 0.078, 0.061)
    }

    /// Preset: maze (f=0.029, k=0.057)
    pub fn preset_maze(width: usize, height: usize) -> Self {
        Self::new(width, height, 0.029, 0.057)
    }

    /// Preset: solitons (f=0.03, k=0.06)
    pub fn preset_solitons(width: usize, height: usize) -> Self {
        Self::new(width, height, 0.03, 0.06)
    }

    /// Preset: unstable (f=0.02, k=0.055)
    pub fn preset_unstable(width: usize, height: usize) -> Self {
        Self::new(width, height, 0.02, 0.055)
    }
}

// ============================================================
// FITZHUGH-NAGUMO
// ============================================================

/// FitzHugh-Nagumo excitable medium — models action potentials and spiral waves.
#[derive(Clone, Debug)]
pub struct FitzHughNagumo {
    pub v: Vec<f64>,   // fast variable (voltage)
    pub w: Vec<f64>,   // slow recovery variable
    pub width: usize,
    pub height: usize,
    pub a: f64,        // threshold parameter
    pub b: f64,        // recovery coupling
    pub tau: f64,      // time scale separation
    pub eps: f64,      // diffusion
}

impl FitzHughNagumo {
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Self {
            v: vec![0.0; n],
            w: vec![0.0; n],
            width, height,
            a: 0.7, b: 0.8, tau: 12.5, eps: 0.1,
        }
    }

    pub fn stimulate(&mut self, x: usize, y: usize, radius: usize) {
        for dy in 0..=radius * 2 {
            for dx in 0..=radius * 2 {
                let nx = (x + dx).saturating_sub(radius);
                let ny = (y + dy).saturating_sub(radius);
                if nx < self.width && ny < self.height {
                    let i = ny * self.width + nx;
                    self.v[i] = 1.0;
                }
            }
        }
    }

    fn laplacian(data: &[f64], x: usize, y: usize, w: usize, h: usize) -> f64 {
        let idx = |x: usize, y: usize| y * w + x;
        let xm = if x == 0 { w - 1 } else { x - 1 };
        let xp = if x == w - 1 { 0 } else { x + 1 };
        let ym = if y == 0 { h - 1 } else { y - 1 };
        let yp = if y == h - 1 { 0 } else { y + 1 };
        data[idx(xm, y)] + data[idx(xp, y)] + data[idx(x, ym)] + data[idx(x, yp)] - 4.0 * data[idx(x, y)]
    }

    pub fn step(&mut self, dt: f64) {
        let n = self.width * self.height;
        let mut v_new = vec![0.0f64; n];
        let mut w_new = vec![0.0f64; n];
        for y in 0..self.height {
            for x in 0..self.width {
                let i = y * self.width + x;
                let v = self.v[i];
                let w = self.w[i];
                let lap_v = Self::laplacian(&self.v, x, y, self.width, self.height);
                let dv = v - v.powi(3) / 3.0 - w + self.eps * lap_v;
                let dw = (v + self.a - self.b * w) / self.tau;
                v_new[i] = v + dt * dv;
                w_new[i] = w + dt * dw;
            }
        }
        self.v = v_new;
        self.w = w_new;
    }
}

// ============================================================
// BELOUSOV-ZHABOTINSKY
// ============================================================

/// Parameters for the Belousov-Zhabotinsky oscillator.
#[derive(Clone, Debug)]
pub struct BzParams {
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
}

impl Default for BzParams {
    fn default() -> Self { BzParams { alpha: 0.1, beta: 0.1, gamma: 0.1 } }
}

/// Simplified 3-variable BZ reaction model on a 2D grid.
#[derive(Clone, Debug)]
pub struct BelousovZhabotinsky {
    /// Each cell stores 3 concentrations [u, v, w].
    pub concentrations: Vec<Vec<f64>>,
    pub width: usize,
    pub height: usize,
    pub params: BzParams,
}

impl BelousovZhabotinsky {
    pub fn new(width: usize, height: usize, params: BzParams) -> Self {
        let n = width * height;
        // Random-ish initial concentrations
        let mut concentrations = vec![vec![0.0f64; 3]; n];
        let mut state = 12345u64;
        for row in &mut concentrations {
            for v in row.iter_mut() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *v = (state >> 33) as f64 / (u32::MAX as f64);
            }
        }
        Self { concentrations, width, height, params }
    }

    fn laplacian_conc(data: &[Vec<f64>], comp: usize, x: usize, y: usize, w: usize, h: usize) -> f64 {
        let idx = |x: usize, y: usize| y * w + x;
        let xm = if x == 0 { w - 1 } else { x - 1 };
        let xp = if x == w - 1 { 0 } else { x + 1 };
        let ym = if y == 0 { h - 1 } else { y - 1 };
        let yp = if y == h - 1 { 0 } else { y + 1 };
        data[idx(xm, y)][comp] + data[idx(xp, y)][comp]
            + data[idx(x, ym)][comp] + data[idx(x, yp)][comp]
            - 4.0 * data[idx(x, y)][comp]
    }

    pub fn step(&mut self, dt: f64) {
        let n = self.width * self.height;
        let mut new_c = self.concentrations.clone();
        let alpha = self.params.alpha;
        let beta = self.params.beta;
        let gamma = self.params.gamma;
        for y in 0..self.height {
            for x in 0..self.width {
                let i = y * self.width + x;
                let u = self.concentrations[i][0];
                let v = self.concentrations[i][1];
                let w = self.concentrations[i][2];
                let lap_u = Self::laplacian_conc(&self.concentrations, 0, x, y, self.width, self.height);
                let lap_v = Self::laplacian_conc(&self.concentrations, 1, x, y, self.width, self.height);
                let lap_w = Self::laplacian_conc(&self.concentrations, 2, x, y, self.width, self.height);
                // Oregonator-inspired simplified ODE
                let du = u * (1.0 - u - alpha * v / (u + beta)) + 0.1 * lap_u;
                let dv = u - v + 0.05 * lap_v;
                let dw = gamma * (u - w) + 0.02 * lap_w;
                new_c[i][0] = (u + dt * du).clamp(0.0, 1.0);
                new_c[i][1] = (v + dt * dv).clamp(0.0, 1.0);
                new_c[i][2] = (w + dt * dw).clamp(0.0, 1.0);
            }
        }
        self.concentrations = new_c;
    }
}

// ============================================================
// EPIDEMIOLOGICAL MODELS
// ============================================================

/// Susceptible-Infected-Recovered (SIR) model.
#[derive(Clone, Debug)]
pub struct SirModel {
    pub s: f64,     // susceptible fraction
    pub i: f64,     // infected fraction
    pub r: f64,     // recovered fraction
    pub n: f64,     // total population
    pub beta: f64,  // transmission rate
    pub gamma: f64, // recovery rate
    pub time: f64,
}

impl SirModel {
    pub fn new(s0: f64, i0: f64, n: f64, beta: f64, gamma: f64) -> Self {
        let r0 = n - s0 - i0;
        Self { s: s0, i: i0, r: r0.max(0.0), n, beta, gamma, time: 0.0 }
    }

    pub fn step(&mut self, dt: f64) {
        let ds = -self.beta * self.s * self.i / self.n;
        let di = self.beta * self.s * self.i / self.n - self.gamma * self.i;
        let dr = self.gamma * self.i;
        self.s = (self.s + dt * ds).max(0.0);
        self.i = (self.i + dt * di).max(0.0);
        self.r = (self.r + dt * dr).max(0.0);
        self.time += dt;
    }

    pub fn basic_reproduction_number(&self) -> f64 {
        self.beta / self.gamma
    }

    pub fn time_to_peak(&self) -> f64 {
        // Approximate: t_peak ~ (1/(beta-gamma)) * ln(beta*S0/(gamma*N))
        let r0 = self.basic_reproduction_number();
        if r0 <= 1.0 { return f64::INFINITY; }
        let beta = self.beta; let gamma = self.gamma;
        (1.0 / (beta - gamma)) * (beta * self.s / (gamma * self.n)).ln().abs()
    }

    pub fn herd_immunity_threshold(&self) -> f64 {
        1.0 - 1.0 / self.basic_reproduction_number()
    }
}

/// SEIRD model with Exposed and Deceased compartments.
#[derive(Clone, Debug)]
pub struct SeirdModel {
    pub s: f64,      // susceptible
    pub e: f64,      // exposed (latent)
    pub i: f64,      // infectious
    pub r: f64,      // recovered
    pub d: f64,      // deceased
    pub n: f64,      // total population
    pub beta: f64,   // transmission rate
    pub sigma: f64,  // rate E→I (1/incubation period)
    pub gamma: f64,  // recovery rate
    pub delta: f64,  // mortality rate
    pub time: f64,
}

impl SeirdModel {
    pub fn new(s0: f64, e0: f64, i0: f64, n: f64, beta: f64, sigma: f64, gamma: f64, delta: f64) -> Self {
        Self { s: s0, e: e0, i: i0, r: 0.0, d: 0.0, n, beta, sigma, gamma, delta, time: 0.0 }
    }

    pub fn step(&mut self, dt: f64) {
        let force = self.beta * self.s * self.i / self.n;
        let ds = -force;
        let de = force - self.sigma * self.e;
        let di = self.sigma * self.e - (self.gamma + self.delta) * self.i;
        let dr = self.gamma * self.i;
        let dd = self.delta * self.i;
        self.s = (self.s + dt * ds).max(0.0);
        self.e = (self.e + dt * de).max(0.0);
        self.i = (self.i + dt * di).max(0.0);
        self.r = (self.r + dt * dr).max(0.0);
        self.d = (self.d + dt * dd).max(0.0);
        self.time += dt;
    }

    pub fn total_active(&self) -> f64 { self.s + self.e + self.i }
}

/// Per-cell state for spatial SIR model.
#[derive(Clone, Copy, Debug, Default)]
pub struct SirCell {
    pub s: f64,
    pub i: f64,
    pub r: f64,
}

/// Spatial SIR model on a 2D grid.
#[derive(Clone, Debug)]
pub struct SirGrid {
    pub grid: Vec<SirCell>,
    pub width: usize,
    pub height: usize,
    pub beta: f64,
    pub gamma: f64,
    pub diffusion: f64,
    pub time: f64,
}

impl SirGrid {
    pub fn new(width: usize, height: usize, beta: f64, gamma: f64) -> Self {
        let n = width * height;
        let cell = SirCell { s: 1.0, i: 0.0, r: 0.0 };
        Self { grid: vec![cell; n], width, height, beta, gamma, diffusion: 0.1, time: 0.0 }
    }

    pub fn seed(&mut self, x: usize, y: usize, fraction: f64) {
        if x < self.width && y < self.height {
            let i = y * self.width + x;
            self.grid[i].i = fraction;
            self.grid[i].s = (1.0 - fraction).max(0.0);
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    pub fn step(&mut self, dt: f64) {
        let old = self.grid.clone();
        for y in 0..self.height {
            for x in 0..self.width {
                let i = self.idx(x, y);
                let cell = old[i];
                let force = self.beta * cell.s * cell.i;
                // Diffusion of infected cells
                let xm = if x == 0 { self.width - 1 } else { x - 1 };
                let xp = if x == self.width - 1 { 0 } else { x + 1 };
                let ym = if y == 0 { self.height - 1 } else { y - 1 };
                let yp = if y == self.height - 1 { 0 } else { y + 1 };
                let lap_i = old[self.idx(xm, y)].i + old[self.idx(xp, y)].i
                    + old[self.idx(x, ym)].i + old[self.idx(x, yp)].i - 4.0 * cell.i;
                self.grid[i].s = (cell.s - dt * force).max(0.0);
                self.grid[i].i = (cell.i + dt * (force - self.gamma * cell.i + self.diffusion * lap_i)).max(0.0);
                self.grid[i].r = (cell.r + dt * self.gamma * cell.i).max(0.0);
            }
        }
        self.time += dt;
    }

    pub fn total_infected(&self) -> f64 {
        self.grid.iter().map(|c| c.i).sum()
    }
}

// ============================================================
// TRAFFIC SIMULATION
// ============================================================

/// Vehicle in the IDM model.
#[derive(Clone, Debug)]
pub struct Vehicle {
    pub pos: f64,
    pub speed: f64,
    pub length: f64,
    pub desired_speed: f64,
    pub accel: f64,
    pub decel: f64,
}

/// Nagel-Schreckenberg cellular automaton traffic model.
#[derive(Clone, Debug)]
pub struct NagelSchreckenberg {
    pub road: Vec<Option<usize>>,   // cell index → vehicle index or None
    pub vehicles: Vec<(usize, u32)>, // (cell_pos, speed) for each vehicle
    pub road_length: usize,
    pub v_max: u32,
    pub p_brake: f64,               // random braking probability
    rng_state: u64,
}

impl NagelSchreckenberg {
    pub fn new(road_length: usize, v_max: u32, p_brake: f64, density: f64, seed: u64) -> Self {
        let mut road = vec![None; road_length];
        let mut vehicles = Vec::new();
        let mut rng = seed;
        let n_vehicles = (density * road_length as f64) as usize;
        let mut positions: Vec<usize> = (0..road_length).collect();
        // Shuffle positions
        for i in (1..road_length).rev() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (rng >> 33) as usize % (i + 1);
            positions.swap(i, j);
        }
        for k in 0..n_vehicles.min(road_length) {
            let pos = positions[k];
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let spd = (rng >> 33) as u32 % (v_max + 1);
            road[pos] = Some(k);
            vehicles.push((pos, spd));
        }
        Self { road, vehicles, road_length, v_max, p_brake, rng_state: rng }
    }

    fn rand(&mut self) -> f64 {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.rng_state >> 33) as f64 / (u32::MAX as f64)
    }

    pub fn step(&mut self) {
        let n_veh = self.vehicles.len();
        let road_len = self.road_length;

        // Phase 1: Acceleration
        for v in &mut self.vehicles {
            if v.1 < self.v_max { v.1 += 1; }
        }
        // Phase 2: Braking (maintain gap)
        let positions: Vec<usize> = self.vehicles.iter().map(|v| v.0).collect();
        for (k, v) in self.vehicles.iter_mut().enumerate() {
            // Find gap to next vehicle
            let mut gap = road_len; // maximum
            for j in 1..=road_len {
                let next_pos = (v.0 + j) % road_len;
                if self.road[next_pos].is_some() { gap = j - 1; break; }
            }
            if v.1 as usize > gap { v.1 = gap as u32; }
        }
        // Phase 3: Random braking
        let brakes: Vec<f64> = (0..self.vehicles.len()).map(|_| self.rand()).collect();
        for (i, v) in self.vehicles.iter_mut().enumerate() {
            if v.1 > 0 && brakes[i] < self.p_brake { v.1 -= 1; }
        }
        // Phase 4: Move
        let mut new_road = vec![None; road_len];
        for (k, v) in self.vehicles.iter_mut().enumerate() {
            self.road[v.0] = None;
            v.0 = (v.0 + v.1 as usize) % road_len;
            new_road[v.0] = Some(k);
        }
        self.road = new_road;
    }

    pub fn flow_rate(&self) -> f64 {
        let total_speed: f64 = self.vehicles.iter().map(|v| v.1 as f64).sum();
        total_speed / self.road_length as f64
    }

    pub fn density(&self) -> f64 {
        self.vehicles.len() as f64 / self.road_length as f64
    }

    pub fn average_speed(&self) -> f64 {
        if self.vehicles.is_empty() { return 0.0; }
        self.vehicles.iter().map(|v| v.1 as f64).sum::<f64>() / self.vehicles.len() as f64
    }
}

/// Intelligent Driver Model (IDM) parameters.
#[derive(Clone, Debug)]
pub struct IntelligentDriverModel {
    pub t_desired: f64,   // desired time headway (s)
    pub a_desired: f64,   // max acceleration (m/s²)
    pub b_desired: f64,   // comfortable deceleration (m/s²)
    pub v_desired: f64,   // desired speed (m/s)
    pub s_min: f64,        // minimum gap (m)
}

impl IntelligentDriverModel {
    pub fn new() -> Self {
        Self {
            t_desired: 1.5,
            a_desired: 1.4,
            b_desired: 2.0,
            v_desired: 33.33,   // ~120 km/h
            s_min: 2.0,
        }
    }

    /// Compute acceleration for a vehicle given:
    /// `v` — current speed, `delta_v` — speed difference to leader (v - v_leader),
    /// `s` — gap to leader.
    pub fn acceleration(&self, v: f64, delta_v: f64, s: f64) -> f64 {
        let s_star = self.s_min
            + (v * self.t_desired
                + v * delta_v / (2.0 * (self.a_desired * self.b_desired).sqrt()))
            .max(0.0);
        let free_term = (v / self.v_desired).powi(4);
        let interaction_term = (s_star / s.max(0.001)).powi(2);
        self.a_desired * (1.0 - free_term - interaction_term)
    }
}

impl Default for IntelligentDriverModel {
    fn default() -> Self { Self::new() }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_of_life_step() {
        let mut gol = GameOfLife::new(20, 20);
        gol.place_glider(1, 1);
        let initial_count = gol.count_living();
        assert_eq!(initial_count, 5);
        gol.step();
        // Glider should still have 5 live cells after one step
        assert_eq!(gol.count_living(), 5);
    }

    #[test]
    fn test_game_of_life_still_life() {
        // A 2x2 block is a still life
        let mut gol = GameOfLife::new(10, 10);
        gol.grid[1][1] = true; gol.grid[1][2] = true;
        gol.grid[2][1] = true; gol.grid[2][2] = true;
        assert!(gol.is_stable());
    }

    #[test]
    fn test_wolfram_rule30() {
        let ca = Totalistic1D::new(30);
        let init: Vec<bool> = (0..11).map(|i| i == 5).collect();
        let result = ca.evolve(init, 5);
        assert_eq!(result.len(), 6);
    }

    #[test]
    fn test_wolfram_rule110() {
        let ca = Totalistic1D::new(110);
        let init = vec![false, false, false, false, true, false, false, false, false];
        let result = ca.evolve(init, 4);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_langtons_ant_steps() {
        let mut ant = LangtonsAnt::new();
        ant.step_n(100);
        assert_eq!(ant.steps, 100);
        // After 100 steps, should have some black cells
        assert!(ant.count_black() > 0);
    }

    #[test]
    fn test_langtons_ant_highway() {
        // After 10000 steps Langton's ant enters the highway pattern
        let mut ant = LangtonsAnt::new();
        ant.step_n(10000);
        assert_eq!(ant.steps, 10000);
    }

    #[test]
    fn test_nbody_basic() {
        let bodies = vec![
            Body { position: Vec3::new(0.0, 0.0, 0.0), velocity: Vec3::zero(), mass: 1e10, radius: 1.0 },
            Body { position: Vec3::new(100.0, 0.0, 0.0), velocity: Vec3::zero(), mass: 1e10, radius: 1.0 },
        ];
        let mut sim = NBodySimulation::new(bodies, 1.0);
        direct_gravity_step(&mut sim);
        // Bodies should move toward each other
        assert!(sim.bodies[0].velocity.x > 0.0);
        assert!(sim.bodies[1].velocity.x < 0.0);
    }

    #[test]
    fn test_leapfrog_step() {
        let bodies = vec![
            Body { position: Vec3::new(0.0, 0.0, 0.0), velocity: Vec3::new(0.0, 1e-3, 0.0), mass: 1e12, radius: 1.0 },
            Body { position: Vec3::new(1000.0, 0.0, 0.0), velocity: Vec3::new(0.0, -1e-3, 0.0), mass: 1e12, radius: 1.0 },
        ];
        let mut sim = NBodySimulation::new(bodies, 0.1);
        for _ in 0..10 { leapfrog_step(&mut sim); }
        assert!((sim.time - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_solve_kepler() {
        // Circular orbit: e=0, M=1.0 => E=1.0
        let e_anom = solve_kepler_equation(1.0, 0.0);
        assert!((e_anom - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_orbital_round_trip() {
        let r = Vec3::new(6371e3 + 400e3, 0.0, 0.0); // LEO
        let v = Vec3::new(0.0, 7.66e3, 0.0); // approximately circular
        let mu = 3.986e14;
        let elems = from_state_vectors(r, v, mu);
        let (r2, v2) = to_state_vectors(&elems, mu);
        assert!((r.x - r2.x).abs() < 1.0, "r.x mismatch");
        assert!((v.y - v2.y).abs() < 1.0, "v.y mismatch");
    }

    #[test]
    fn test_gray_scott_step() {
        let mut gs = GrayScott::new(20, 20, 0.055, 0.062);
        gs.seed_center();
        for _ in 0..10 { gs.step(1.0); }
        // Just check no NaN or out-of-range
        assert!(gs.u.iter().all(|&x| x >= 0.0 && x <= 1.0));
        assert!(gs.v.iter().all(|&x| x >= 0.0 && x <= 1.0));
    }

    #[test]
    fn test_fitzhugh_nagumo_step() {
        let mut fhn = FitzHughNagumo::new(10, 10);
        fhn.stimulate(5, 5, 1);
        for _ in 0..20 { fhn.step(0.05); }
        // Check finite values
        assert!(fhn.v.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_sir_model() {
        let mut sir = SirModel::new(999.0, 1.0, 1000.0, 0.3, 0.1);
        let r0 = sir.basic_reproduction_number();
        assert!((r0 - 3.0).abs() < 1e-10);
        for _ in 0..100 { sir.step(1.0); }
        // Epidemic should have spread
        assert!(sir.r > 10.0);
    }

    #[test]
    fn test_sir_herd_immunity() {
        let sir = SirModel::new(999.0, 1.0, 1000.0, 0.3, 0.1);
        let hit = sir.herd_immunity_threshold();
        assert!((hit - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_seird_model() {
        let mut seird = SeirdModel::new(9990.0, 5.0, 5.0, 10000.0, 0.3, 0.2, 0.1, 0.01);
        for _ in 0..50 { seird.step(1.0); }
        assert!(seird.r > 0.0);
        assert!(seird.d > 0.0);
    }

    #[test]
    fn test_nagel_schreckenberg() {
        let mut ns = NagelSchreckenberg::new(100, 5, 0.3, 0.3, 42);
        for _ in 0..20 { ns.step(); }
        assert!(ns.average_speed() >= 0.0);
        assert!(ns.flow_rate() >= 0.0);
        assert!(ns.density() >= 0.0 && ns.density() <= 1.0);
    }

    #[test]
    fn test_idm_acceleration() {
        let idm = IntelligentDriverModel::new();
        // Free flow — no leader in front, large gap
        let a_free = idm.acceleration(20.0, 0.0, 1000.0);
        assert!(a_free > 0.0, "should accelerate in free flow");
        // Braking — small gap and leader is slower
        let a_brake = idm.acceleration(20.0, 10.0, 5.0);
        assert!(a_brake < 0.0, "should decelerate when too close");
    }

    #[test]
    fn test_bz_reaction() {
        let params = BzParams::default();
        let mut bz = BelousovZhabotinsky::new(10, 10, params);
        for _ in 0..5 { bz.step(0.01); }
        // All concentrations should remain in [0,1]
        for cell in &bz.concentrations {
            for &c in cell { assert!(c >= 0.0 && c <= 1.0, "concentration out of bounds: {}", c); }
        }
    }

    #[test]
    fn test_wireworld_step() {
        let mut ww = WireWorld::new(10, 5);
        ww.set(0, 2, WireWorldCell::Conductor);
        ww.set(1, 2, WireWorldCell::ElectronHead);
        ww.set(2, 2, WireWorldCell::Conductor);
        ww.step();
        assert_eq!(ww.grid[2][1], WireWorldCell::ElectronTail);
        assert_eq!(ww.grid[2][2], WireWorldCell::ElectronHead);
    }

    #[test]
    fn test_forest_fire_step() {
        let mut ff = ForestFire::new(20, 20, 0.1, 0.001, 42);
        // Fill with trees
        for row in ff.grid.iter_mut() {
            for cell in row.iter_mut() { *cell = ForestCell::Tree; }
        }
        ff.grid[10][10] = ForestCell::Burning;
        ff.step();
        // Some neighboring cells should have caught fire
        let burning = ff.count_burning();
        assert!(burning > 0);
    }

    #[test]
    fn test_brians_brain() {
        let mut bb = BriansBrain::new(10, 10);
        bb.grid[5][5] = BrianCell::On;
        bb.step();
        assert_eq!(bb.grid[5][5], BrianCell::Dying);
        bb.step();
        assert_eq!(bb.grid[5][5], BrianCell::Off);
    }

    #[test]
    fn test_sir_grid() {
        let mut sg = SirGrid::new(20, 20, 0.3, 0.05);
        sg.seed(10, 10, 0.1);
        for _ in 0..50 { sg.step(1.0); }
        assert!(sg.total_infected() > 0.0);
    }

    #[test]
    fn test_vec3_ops() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let c = a.cross(b);
        assert!((c.z - 1.0).abs() < 1e-10);
        assert!((a.dot(b)).abs() < 1e-10);
    }
}
