code = r'''

// =================================================================
// WORLD GEN EXPANSION: EROSION, RIVER NETWORKS, TECTONIC SIMULATION
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErosionConfig {
    pub iterations: u32,
    pub rain_amount: f32,
    pub evaporation: f32,
    pub sediment_capacity: f32,
    pub deposition_rate: f32,
    pub erosion_rate: f32,
    pub min_slope: f32,
    pub gravity: f32,
    pub inertia: f32,
    pub radius: f32,
    pub max_steps: u32,
}

impl Default for ErosionConfig {
    fn default() -> Self {
        Self {
            iterations: 50000,
            rain_amount: 1.0,
            evaporation: 0.01,
            sediment_capacity: 4.0,
            deposition_rate: 0.3,
            erosion_rate: 0.3,
            min_slope: 0.01,
            gravity: 4.0,
            inertia: 0.05,
            radius: 3.0,
            max_steps: 30,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiverNode {
    pub x: f32,
    pub y: f32,
    pub flow: f32,
    pub elevation: f32,
    pub width: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiverSegment {
    pub nodes: Vec<RiverNode>,
    pub source_elevation: f32,
    pub mouth_elevation: f32,
    pub total_flow: f32,
    pub length: f32,
    pub tributaries: Vec<usize>,
    pub is_navigable: bool,
}

impl RiverSegment {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            source_elevation: 0.0,
            mouth_elevation: 0.0,
            total_flow: 0.0,
            length: 0.0,
            tributaries: Vec::new(),
            is_navigable: false,
        }
    }

    pub fn compute_length(&mut self) {
        self.length = 0.0;
        for i in 1..self.nodes.len() {
            let dx = self.nodes[i].x - self.nodes[i-1].x;
            let dy = self.nodes[i].y - self.nodes[i-1].y;
            self.length += (dx*dx + dy*dy).sqrt();
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiverNetwork {
    pub rivers: Vec<RiverSegment>,
    pub grid_w: usize,
    pub grid_h: usize,
    pub flow_map: Vec<f32>,
    pub drainage_map: Vec<f32>,
}

impl RiverNetwork {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            rivers: Vec::new(),
            grid_w: w,
            grid_h: h,
            flow_map: vec![0.0; w * h],
            drainage_map: vec![0.0; w * h],
        }
    }

    pub fn generate_from_heightmap(&mut self, heightmap: &[f32], rng: &mut SimpleRng) {
        let w = self.grid_w;
        let h = self.grid_h;
        // Compute flow directions using D8 algorithm
        let mut flow_dir = vec![0u8; w * h];
        let neighbors8: [(i32,i32); 8] = [(-1,-1),(-1,0),(-1,1),(0,-1),(0,1),(1,-1),(1,0),(1,1)];
        let dist8 = [1.414f32, 1.0, 1.414, 1.0, 1.0, 1.414, 1.0, 1.414];

        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx = y * w + x;
                let elev = heightmap[idx];
                let mut max_drop = 0.0f32;
                let mut best_dir = 0u8;
                for (d, &(dy, dx)) in neighbors8.iter().enumerate() {
                    let nx = (x as i32 + dx) as usize;
                    let ny = (y as i32 + dy) as usize;
                    let drop = (elev - heightmap[ny * w + nx]) / dist8[d];
                    if drop > max_drop { max_drop = drop; best_dir = d as u8; }
                }
                flow_dir[idx] = best_dir;
            }
        }

        // Accumulate flow
        let mut flow = vec![1.0f32; w * h];
        for _ in 0..8 {
            let flow_copy = flow.clone();
            for y in 1..h-1 {
                for x in 1..w-1 {
                    let idx = y * w + x;
                    let d = flow_dir[idx] as usize;
                    let (dy, dx) = neighbors8[d];
                    let nx = (x as i32 + dx) as usize;
                    let ny = (y as i32 + dy) as usize;
                    let nidx = ny * w + nx;
                    if nidx < w * h { flow[nidx] += flow_copy[idx] * 0.9; }
                }
            }
        }
        self.flow_map = flow.clone();

        // Trace rivers from high-flow cells
        let threshold = 50.0;
        let mut river_starts: Vec<(usize, usize)> = Vec::new();
        for y in 2..h-2 {
            for x in 2..w-2 {
                if flow[y * w + x] > threshold && heightmap[y * w + x] > 0.3 {
                    // Check it's a local flow maximum
                    let mut is_local_max = true;
                    for &(dy, dx) in &neighbors8 {
                        let nx2 = (x as i32 + dx) as usize;
                        let ny2 = (y as i32 + dy) as usize;
                        if flow[ny2 * w + nx2] > flow[y * w + x] * 1.5 { is_local_max = false; break; }
                    }
                    if is_local_max && rng.next_f32() < 0.1 { river_starts.push((x, y)); }
                }
            }
        }

        // Trace each river downstream
        for (sx, sy) in river_starts.iter().take(20) {
            let mut seg = RiverSegment::new();
            seg.source_elevation = heightmap[sy * w + sx];
            let mut cx = *sx;
            let mut cy = *sy;
            let mut steps = 0;
            loop {
                let idx = cy * w + cx;
                seg.nodes.push(RiverNode {
                    x: cx as f32 / w as f32,
                    y: cy as f32 / h as f32,
                    flow: flow[idx],
                    elevation: heightmap[idx],
                    width: (flow[idx] / threshold).sqrt() * 2.0,
                });
                // Follow steepest descent
                let d = flow_dir[idx] as usize;
                let (dy, dx) = neighbors8[d];
                let nx = (cx as i32 + dx) as usize;
                let ny = (cy as i32 + dy) as usize;
                if nx == 0 || ny == 0 || nx >= w-1 || ny >= h-1 { break; }
                if heightmap[ny * w + nx] >= heightmap[idx] { break; }
                cx = nx; cy = ny;
                steps += 1;
                if steps > 500 { break; }
            }
            seg.mouth_elevation = heightmap[cy * w + cx];
            seg.total_flow = flow[cy * w + cx];
            seg.is_navigable = seg.total_flow > threshold * 3.0;
            seg.compute_length();
            if seg.nodes.len() > 5 { self.rivers.push(seg); }
        }
    }

    pub fn river_count(&self) -> usize { self.rivers.len() }
    pub fn navigable_rivers(&self) -> Vec<&RiverSegment> {
        self.rivers.iter().filter(|r| r.is_navigable).collect()
    }
}

// Hydraulic erosion simulation
pub fn hydraulic_erosion(heightmap: &mut Vec<f32>, w: usize, h: usize, cfg: &ErosionConfig, rng: &mut SimpleRng) {
    for _iter in 0..cfg.iterations {
        // Spawn droplet at random position
        let mut px = rng.next_f32() * (w as f32 - 2.0) + 1.0;
        let mut py = rng.next_f32() * (h as f32 - 2.0) + 1.0;
        let mut vx = 0.0f32;
        let mut vy = 0.0f32;
        let mut water = 1.0f32;
        let mut sediment = 0.0f32;
        let mut speed = 1.0f32;

        for _step in 0..cfg.max_steps {
            let ix = px as usize;
            let iy = py as usize;
            if ix >= w-1 || iy >= h-1 { break; }
            let fx = px - ix as f32;
            let fy = py - iy as f32;

            // Bilinear height
            let h00 = heightmap[iy * w + ix];
            let h10 = heightmap[iy * w + ix + 1];
            let h01 = heightmap[(iy+1) * w + ix];
            let h11 = heightmap[(iy+1) * w + ix + 1];
            let height = h00*(1.0-fx)*(1.0-fy) + h10*fx*(1.0-fy) + h01*(1.0-fx)*fy + h11*fx*fy;

            // Gradient
            let gx = (h10 - h00) * (1.0 - fy) + (h11 - h01) * fy;
            let gy = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;

            // Update velocity
            vx = vx * cfg.inertia - gx * (1.0 - cfg.inertia);
            vy = vy * cfg.inertia - gy * (1.0 - cfg.inertia);
            let vel_len = (vx*vx + vy*vy).sqrt().max(0.001);
            vx /= vel_len; vy /= vel_len;

            let new_px = px + vx;
            let new_py = py + vy;
            if new_px < 1.0 || new_py < 1.0 || new_px >= w as f32 - 1.0 || new_py >= h as f32 - 1.0 { break; }

            let ix2 = new_px as usize;
            let iy2 = new_py as usize;
            let new_h = heightmap[iy2 * w + ix2];
            let delta_h = new_h - height;

            let capacity = (cfg.sediment_capacity * speed * water * (-delta_h).max(cfg.min_slope)).max(0.0);

            if sediment > capacity || delta_h > 0.0 {
                let deposit = if delta_h > 0.0 { sediment.min(delta_h) } else { (sediment - capacity) * cfg.deposition_rate };
                sediment -= deposit;
                heightmap[iy * w + ix] += deposit * (1.0 - fx) * (1.0 - fy);
                if ix + 1 < w { heightmap[iy * w + ix + 1] += deposit * fx * (1.0 - fy); }
                if iy + 1 < h { heightmap[(iy+1) * w + ix] += deposit * (1.0 - fx) * fy; }
                if ix + 1 < w && iy + 1 < h { heightmap[(iy+1) * w + ix + 1] += deposit * fx * fy; }
            } else {
                let erode = ((capacity - sediment) * cfg.erosion_rate).min(-delta_h);
                let r = cfg.radius as usize;
                let mut total_weight = 0.0f32;
                let mut weights = vec![0.0f32; (2*r+1)*(2*r+1)];
                for ey in 0..=2*r {
                    for ex in 0..=2*r {
                        let dist = (((ex as i32 - r as i32).pow(2) + (ey as i32 - r as i32).pow(2)) as f32).sqrt();
                        if dist < cfg.radius {
                            let w2 = cfg.radius - dist;
                            weights[ey*(2*r+1)+ex] = w2;
                            total_weight += w2;
                        }
                    }
                }
                if total_weight > 0.0 {
                    for ey in 0..=2*r {
                        for ex in 0..=2*r {
                            let wx2 = ix as i32 + ex as i32 - r as i32;
                            let wy2 = iy as i32 + ey as i32 - r as i32;
                            if wx2 >= 0 && wy2 >= 0 && wx2 < w as i32 && wy2 < h as i32 {
                                heightmap[wy2 as usize * w + wx2 as usize] -= erode * weights[ey*(2*r+1)+ex] / total_weight;
                            }
                        }
                    }
                    sediment += erode;
                }
            }

            speed = (speed * speed + delta_h.abs() * cfg.gravity).sqrt();
            water *= 1.0 - cfg.evaporation;
            if water < 0.01 { break; }
            px = new_px; py = new_py;
        }
    }
}

// Thermal erosion (talus formation)
pub fn thermal_erosion(heightmap: &mut Vec<f32>, w: usize, h: usize, iterations: u32, talus_angle: f32) {
    let neighbors4: [(i32,i32); 4] = [(0,-1),(0,1),(-1,0),(1,0)];
    for _ in 0..iterations {
        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx = y * w + x;
                let elev = heightmap[idx];
                let mut total_diff = 0.0f32;
                let mut diffs = [0.0f32; 4];
                for (i, &(dy, dx)) in neighbors4.iter().enumerate() {
                    let nx = (x as i32 + dx) as usize;
                    let ny = (y as i32 + dy) as usize;
                    let diff = elev - heightmap[ny * w + nx];
                    if diff > talus_angle { diffs[i] = diff; total_diff += diff; }
                }
                if total_diff > 0.0 {
                    for (i, &(dy, dx)) in neighbors4.iter().enumerate() {
                        if diffs[i] > 0.0 {
                            let nx = (x as i32 + dx) as usize;
                            let ny = (y as i32 + dy) as usize;
                            let transfer = 0.5 * diffs[i] / total_diff * (elev - talus_angle);
                            heightmap[idx] -= transfer;
                            heightmap[ny * w + nx] += transfer;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TectonicPlate {
    pub id: u32,
    pub velocity: [f32; 2],
    pub density: f32,
    pub is_oceanic: bool,
    pub cells: Vec<usize>,
    pub center: [f32; 2],
    pub elevation_bias: f32,
}

impl TectonicPlate {
    pub fn new(id: u32, cx: f32, cy: f32, oceanic: bool) -> Self {
        Self {
            id,
            velocity: [0.0, 0.0],
            density: if oceanic { 2.9 } else { 2.7 },
            is_oceanic: oceanic,
            cells: Vec::new(),
            center: [cx, cy],
            elevation_bias: if oceanic { -0.3 } else { 0.1 },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TectonicSimConfig {
    pub plate_count: u32,
    pub oceanic_ratio: f32,
    pub simulation_steps: u32,
    pub collision_strength: f32,
    pub rift_strength: f32,
    pub grid_w: usize,
    pub grid_h: usize,
}

impl Default for TectonicSimConfig {
    fn default() -> Self {
        Self {
            plate_count: 8,
            oceanic_ratio: 0.6,
            simulation_steps: 10,
            collision_strength: 0.8,
            rift_strength: 0.4,
            grid_w: 256,
            grid_h: 256,
        }
    }
}

pub fn simulate_tectonics(cfg: &TectonicSimConfig, rng: &mut SimpleRng) -> Vec<f32> {
    let w = cfg.grid_w;
    let h = cfg.grid_h;
    let n = w * h;
    let mut heightmap = vec![0.0f32; n];
    let mut plate_map = vec![0u32; n];

    // Generate plate centers (Voronoi seeds)
    let mut plates: Vec<TectonicPlate> = Vec::new();
    for i in 0..cfg.plate_count {
        let cx = rng.next_f32();
        let cy = rng.next_f32();
        let oceanic = rng.next_f32() < cfg.oceanic_ratio;
        let mut p = TectonicPlate::new(i, cx, cy, oceanic);
        p.velocity = [rng.next_f32_range(-0.02, 0.02), rng.next_f32_range(-0.02, 0.02)];
        plates.push(p);
    }

    // Assign cells to nearest plate
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let mut best_dist = f32::MAX;
            let mut best_plate = 0u32;
            for (i, p) in plates.iter().enumerate() {
                let dx = fx - p.center[0];
                let dy = fy - p.center[1];
                let d = dx*dx + dy*dy;
                if d < best_dist { best_dist = d; best_plate = i as u32; }
            }
            plate_map[y * w + x] = best_plate;
            plates[best_plate as usize].cells.push(y * w + x);
        }
    }

    // Apply base elevations
    for y in 0..h {
        for x in 0..w {
            let pid = plate_map[y * w + x] as usize;
            heightmap[y * w + x] = plates[pid].elevation_bias;
        }
    }

    // Detect boundaries and simulate collisions
    let neighbors4: [(i32,i32); 4] = [(0,-1),(0,1),(-1,0),(1,0)];
    for _step in 0..cfg.simulation_steps {
        let mut boundary_stress = vec![0.0f32; n];
        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx = y * w + x;
                let pid_a = plate_map[idx] as usize;
                for &(dy, dx) in &neighbors4 {
                    let nx = (x as i32 + dx) as usize;
                    let ny = (y as i32 + dy) as usize;
                    let pid_b = plate_map[ny * w + nx] as usize;
                    if pid_a != pid_b {
                        let va = plates[pid_a].velocity;
                        let vb = plates[pid_b].velocity;
                        let rel_vx = va[0] - vb[0];
                        let rel_vy = va[1] - vb[1];
                        let convergence = rel_vx * dx as f32 + rel_vy * dy as f32;
                        if convergence > 0.0 {
                            // Collision — uplift
                            boundary_stress[idx] += convergence * cfg.collision_strength;
                        } else {
                            // Divergence — rift/depression
                            boundary_stress[idx] += convergence * cfg.rift_strength;
                        }
                    }
                }
            }
        }
        for i in 0..n { heightmap[i] += boundary_stress[i]; }
    }

    // Add noise and normalize
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            heightmap[i] += rng.next_f32_range(-0.05, 0.05);
        }
    }
    let min_h = heightmap.iter().cloned().fold(f32::MAX, f32::min);
    let max_h = heightmap.iter().cloned().fold(f32::MIN, f32::max);
    let range = (max_h - min_h).max(0.001);
    for v in heightmap.iter_mut() { *v = (*v - min_h) / range; }
    heightmap
}

// =================================================================
// WORLD GEN EXPANSION: CLIMATE SIMULATION
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ClimateZone {
    Tropical,
    Subtropical,
    Mediterranean,
    Temperate,
    Continental,
    Subarctic,
    Arctic,
    Arid,
    SemiArid,
    Oceanic,
}

impl ClimateZone {
    pub fn name(&self) -> &'static str {
        match self {
            ClimateZone::Tropical => "Tropical",
            ClimateZone::Subtropical => "Subtropical",
            ClimateZone::Mediterranean => "Mediterranean",
            ClimateZone::Temperate => "Temperate",
            ClimateZone::Continental => "Continental",
            ClimateZone::Subarctic => "Subarctic",
            ClimateZone::Arctic => "Arctic",
            ClimateZone::Arid => "Arid",
            ClimateZone::SemiArid => "Semi-Arid",
            ClimateZone::Oceanic => "Oceanic",
        }
    }

    pub fn avg_temperature(&self) -> f32 {
        match self {
            ClimateZone::Tropical => 28.0,
            ClimateZone::Subtropical => 22.0,
            ClimateZone::Mediterranean => 17.0,
            ClimateZone::Temperate => 12.0,
            ClimateZone::Continental => 6.0,
            ClimateZone::Subarctic => -5.0,
            ClimateZone::Arctic => -20.0,
            ClimateZone::Arid => 25.0,
            ClimateZone::SemiArid => 20.0,
            ClimateZone::Oceanic => 11.0,
        }
    }

    pub fn avg_precipitation(&self) -> f32 {
        match self {
            ClimateZone::Tropical => 2000.0,
            ClimateZone::Subtropical => 900.0,
            ClimateZone::Mediterranean => 600.0,
            ClimateZone::Temperate => 800.0,
            ClimateZone::Continental => 500.0,
            ClimateZone::Subarctic => 300.0,
            ClimateZone::Arctic => 150.0,
            ClimateZone::Arid => 150.0,
            ClimateZone::SemiArid => 300.0,
            ClimateZone::Oceanic => 1000.0,
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            ClimateZone::Tropical => Color32::from_rgb(0, 180, 60),
            ClimateZone::Subtropical => Color32::from_rgb(80, 200, 80),
            ClimateZone::Mediterranean => Color32::from_rgb(200, 200, 80),
            ClimateZone::Temperate => Color32::from_rgb(100, 180, 80),
            ClimateZone::Continental => Color32::from_rgb(120, 140, 80),
            ClimateZone::Subarctic => Color32::from_rgb(180, 200, 200),
            ClimateZone::Arctic => Color32::from_rgb(220, 240, 255),
            ClimateZone::Arid => Color32::from_rgb(230, 190, 100),
            ClimateZone::SemiArid => Color32::from_rgb(200, 180, 110),
            ClimateZone::Oceanic => Color32::from_rgb(100, 160, 200),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClimateCell {
    pub temperature: f32,
    pub precipitation: f32,
    pub humidity: f32,
    pub wind_x: f32,
    pub wind_y: f32,
    pub zone: ClimateZone,
    pub latitude: f32,
}

impl ClimateCell {
    pub fn new(lat: f32) -> Self {
        Self {
            temperature: 15.0 - lat.abs() * 0.5,
            precipitation: 800.0 - lat.abs() * 10.0,
            humidity: 0.5,
            wind_x: 0.0,
            wind_y: 0.0,
            zone: ClimateZone::Temperate,
            latitude: lat,
        }
    }

    pub fn classify(&mut self) {
        self.zone = classify_climate(self.temperature, self.precipitation);
    }
}

pub fn classify_climate(temp: f32, precip: f32) -> ClimateZone {
    if temp > 25.0 && precip > 1500.0 { return ClimateZone::Tropical; }
    if temp > 20.0 && precip > 700.0 { return ClimateZone::Subtropical; }
    if temp > 15.0 && precip < 600.0 && precip > 300.0 { return ClimateZone::Mediterranean; }
    if temp < -10.0 { return ClimateZone::Arctic; }
    if temp < 0.0 { return ClimateZone::Subarctic; }
    if precip < 250.0 { return ClimateZone::Arid; }
    if precip < 400.0 { return ClimateZone::SemiArid; }
    if temp > 10.0 && precip > 700.0 { return ClimateZone::Oceanic; }
    if temp > 8.0 { return ClimateZone::Temperate; }
    ClimateZone::Continental
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClimateSimConfig {
    pub grid_w: usize,
    pub grid_h: usize,
    pub simulation_steps: u32,
    pub wind_strength: f32,
    pub orographic_effect: f32,
    pub ocean_influence: f32,
    pub polar_temp: f32,
    pub equator_temp: f32,
}

impl Default for ClimateSimConfig {
    fn default() -> Self {
        Self {
            grid_w: 64,
            grid_h: 64,
            simulation_steps: 20,
            wind_strength: 1.0,
            orographic_effect: 2.0,
            ocean_influence: 0.5,
            polar_temp: -30.0,
            equator_temp: 30.0,
        }
    }
}

pub fn simulate_climate(cfg: &ClimateSimConfig, heightmap: &[f32], ocean_map: &[bool]) -> Vec<ClimateCell> {
    let w = cfg.grid_w;
    let h = cfg.grid_h;
    let mut cells: Vec<ClimateCell> = (0..h).flat_map(|y| {
        let lat = (y as f32 / h as f32 - 0.5) * 180.0;
        (0..w).map(move |_| ClimateCell::new(lat))
    }).collect();

    // Apply initial temperature gradient
    for y in 0..h {
        let lat_ratio = y as f32 / h as f32;
        let base_temp = cfg.polar_temp + (cfg.equator_temp - cfg.polar_temp) * (1.0 - (lat_ratio * 2.0 - 1.0).abs());
        for x in 0..w {
            let idx = y * w + x;
            let elev_penalty = heightmap[idx].max(0.0) * 20.0;
            cells[idx].temperature = base_temp - elev_penalty;
            if ocean_map[idx] { cells[idx].temperature += cfg.ocean_influence * 5.0; }
        }
    }

    // Simulate wind patterns (Hadley cells approximation)
    for y in 0..h {
        for x in 0..w {
            let lat = cells[y * w + x].latitude;
            let wind_x = (lat * 3.14159 / 30.0).cos() * cfg.wind_strength;
            let wind_y = (lat * 3.14159 / 60.0).sin() * cfg.wind_strength * 0.3;
            cells[y * w + x].wind_x = wind_x;
            cells[y * w + x].wind_y = wind_y;
        }
    }

    // Simulate precipitation (orographic + moisture transport)
    let mut moisture = vec![0.5f32; w * h];
    for y in 0..h {
        for x in 0..w {
            if ocean_map[y * w + x] { moisture[y * w + x] = 0.9; }
        }
    }

    for _step in 0..cfg.simulation_steps {
        let moisture_copy = moisture.clone();
        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx = y * w + x;
                let wx = cells[idx].wind_x;
                let wy = cells[idx].wind_y;
                let src_x = ((x as f32 - wx).round() as i32).clamp(0, w as i32 - 1) as usize;
                let src_y = ((y as f32 - wy).round() as i32).clamp(0, h as i32 - 1) as usize;
                let transported = moisture_copy[src_y * w + src_x] * 0.3;
                moisture[idx] = moisture[idx] * 0.7 + transported;
                // Orographic lift: rain on windward side
                if !ocean_map[idx] {
                    let h_curr = heightmap[idx];
                    let src_h = heightmap[src_y * w + src_x];
                    if h_curr > src_h {
                        let rain_factor = ((h_curr - src_h) * cfg.orographic_effect).min(1.0);
                        cells[idx].precipitation += moisture[idx] * rain_factor * 500.0;
                        moisture[idx] *= 1.0 - rain_factor * 0.5;
                    }
                }
            }
        }
    }

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            cells[idx].humidity = moisture[idx];
            cells[idx].precipitation = cells[idx].precipitation.max(50.0).min(3000.0);
            cells[idx].classify();
        }
    }
    cells
}

// =================================================================
// WORLD GEN EXPANSION: SETTLEMENT / CIVILIZATION PLACEMENT
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SettlementType {
    Hamlet,
    Village,
    Town,
    City,
    Capital,
    Fort,
    Port,
    Mine,
    FarmingVillage,
    TradingPost,
}

impl SettlementType {
    pub fn name(&self) -> &'static str {
        match self {
            SettlementType::Hamlet => "Hamlet",
            SettlementType::Village => "Village",
            SettlementType::Town => "Town",
            SettlementType::City => "City",
            SettlementType::Capital => "Capital",
            SettlementType::Fort => "Fort",
            SettlementType::Port => "Port",
            SettlementType::Mine => "Mine",
            SettlementType::FarmingVillage => "Farming Village",
            SettlementType::TradingPost => "Trading Post",
        }
    }

    pub fn min_suitability(&self) -> f32 {
        match self {
            SettlementType::Hamlet | SettlementType::Mine => 0.2,
            SettlementType::Village | SettlementType::FarmingVillage | SettlementType::Fort | SettlementType::TradingPost => 0.35,
            SettlementType::Town | SettlementType::Port => 0.5,
            SettlementType::City => 0.65,
            SettlementType::Capital => 0.75,
        }
    }

    pub fn population_range(&self) -> (u32, u32) {
        match self {
            SettlementType::Hamlet => (10, 100),
            SettlementType::Village | SettlementType::FarmingVillage => (100, 1000),
            SettlementType::TradingPost | SettlementType::Mine => (50, 500),
            SettlementType::Fort => (100, 800),
            SettlementType::Town | SettlementType::Port => (1000, 10000),
            SettlementType::City => (10000, 100000),
            SettlementType::Capital => (50000, 500000),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settlement {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub settlement_type: SettlementType,
    pub population: u32,
    pub elevation: f32,
    pub suitability: f32,
    pub trade_connections: Vec<usize>,
    pub founded_year: i32,
    pub prosperity: f32,
    pub defense_rating: f32,
    pub description: String,
}

impl Settlement {
    pub fn new(name: &str, x: f32, y: f32, st: SettlementType) -> Self {
        Self {
            name: name.to_string(),
            x, y,
            settlement_type: st,
            population: 500,
            elevation: 0.0,
            suitability: 0.5,
            trade_connections: Vec::new(),
            founded_year: -500,
            prosperity: 0.5,
            defense_rating: 0.5,
            description: String::new(),
        }
    }

    pub fn icon(&self) -> char {
        match self.settlement_type {
            SettlementType::Hamlet => 'h',
            SettlementType::Village | SettlementType::FarmingVillage => 'v',
            SettlementType::Town => 't',
            SettlementType::City => 'c',
            SettlementType::Capital => 'C',
            SettlementType::Fort => 'f',
            SettlementType::Port => 'p',
            SettlementType::Mine => 'm',
            SettlementType::TradingPost => 'T',
        }
    }

    pub fn color(&self) -> Color32 {
        match self.settlement_type {
            SettlementType::Hamlet | SettlementType::Village | SettlementType::FarmingVillage => Color32::from_rgb(200, 200, 150),
            SettlementType::TradingPost | SettlementType::Mine => Color32::from_rgb(200, 170, 100),
            SettlementType::Town => Color32::from_rgb(220, 220, 180),
            SettlementType::Fort => Color32::from_rgb(180, 140, 120),
            SettlementType::Port => Color32::from_rgb(100, 180, 220),
            SettlementType::City => Color32::from_rgb(255, 220, 180),
            SettlementType::Capital => Color32::from_rgb(255, 200, 50),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementPlacementConfig {
    pub suitability_threshold: f32,
    pub min_distance_between: f32,
    pub max_settlements: u32,
    pub river_bonus: f32,
    pub coast_bonus: f32,
    pub flat_land_bonus: f32,
    pub forest_penalty: f32,
    pub high_elevation_penalty: f32,
    pub seed: u64,
}

impl Default for SettlementPlacementConfig {
    fn default() -> Self {
        Self {
            suitability_threshold: 0.4,
            min_distance_between: 0.05,
            max_settlements: 50,
            river_bonus: 0.3,
            coast_bonus: 0.2,
            flat_land_bonus: 0.2,
            forest_penalty: 0.1,
            high_elevation_penalty: 0.2,
            seed: 12345,
        }
    }
}

pub fn compute_settlement_suitability(
    heightmap: &[f32],
    ocean_map: &[bool],
    climate: &[ClimateCell],
    river_flow: &[f32],
    w: usize, h: usize,
    cfg: &SettlementPlacementConfig,
) -> Vec<f32> {
    let mut suit = vec![0.0f32; w * h];
    for y in 1..h-1 {
        for x in 1..w-1 {
            let idx = y * w + x;
            if ocean_map[idx] { continue; }
            let elev = heightmap[idx];
            if elev > 0.85 { suit[idx] = 0.0; continue; } // Too mountainous
            let mut s = 0.5;
            // Flat land is better
            let grad = {
                let dx = heightmap[y * w + x + 1] - heightmap[y * w + x - 1];
                let dy = heightmap[(y+1) * w + x] - heightmap[(y-1) * w + x];
                (dx*dx + dy*dy).sqrt()
            };
            s -= grad * cfg.flat_land_bonus;
            // Elevation penalty for high terrain
            if elev > 0.6 { s -= (elev - 0.6) * cfg.high_elevation_penalty * 3.0; }
            // River bonus
            if river_flow[idx] > 10.0 { s += cfg.river_bonus * (river_flow[idx] / 100.0).min(1.0); }
            // Coast bonus
            let near_ocean = [-1i32,0,1].iter().any(|&dy| {
                [-1i32,0,1].iter().any(|&dx| {
                    let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                    let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                    ocean_map[ny * w + nx]
                })
            });
            if near_ocean { s += cfg.coast_bonus; }
            // Climate factor
            let cl = &climate[idx];
            if cl.temperature > 5.0 && cl.temperature < 30.0 { s += 0.1; }
            if cl.precipitation > 300.0 && cl.precipitation < 1500.0 { s += 0.1; }
            suit[idx] = s.max(0.0).min(1.0);
        }
    }
    suit
}

static SETTLEMENT_NAME_PREFIXES: &[&str] = &["Oak", "Stone", "Iron", "River", "Green", "White", "Black", "Storm", "Gold", "Silver", "Shadow", "Frost", "Sun", "Moon", "Wind", "Ash", "Elder", "New", "Old", "East", "West", "North", "South", "High", "Low", "Lake", "Hill", "Vale", "Glen", "Broad", "Swift", "Deep", "Bright", "Dark", "Long", "Short", "Great", "Little"];
static SETTLEMENT_NAME_SUFFIXES: &[&str] = &["ford", "wick", "burg", "heim", "haven", "bridge", "gate", "watch", "hold", "keep", "port", "town", "ville", "shire", "dale", "moor", "field", "wood", "cliff", "falls", "crossing", "harbor", "bay", "point", "reach", "crest", "peak", "pass", "hollow", "glen", "ridge", "marsh", "heath", "mead", "stead", "thorpe", "by", "ham", "ton", "chester"];

pub fn generate_settlement_name(rng: &mut SimpleRng) -> String {
    let prefix = SETTLEMENT_NAME_PREFIXES[rng.next_u64() as usize % SETTLEMENT_NAME_PREFIXES.len()];
    let suffix = SETTLEMENT_NAME_SUFFIXES[rng.next_u64() as usize % SETTLEMENT_NAME_SUFFIXES.len()];
    format!("{}{}", prefix, suffix)
}

pub fn place_settlements(
    suitability: &[f32],
    w: usize, h: usize,
    cfg: &SettlementPlacementConfig,
    rng: &mut SimpleRng,
) -> Vec<Settlement> {
    let mut settlements: Vec<Settlement> = Vec::new();
    let min_dist_sq = cfg.min_distance_between * cfg.min_distance_between;

    // Collect candidate positions sorted by suitability
    let mut candidates: Vec<(usize, f32)> = suitability.iter().enumerate()
        .filter(|(_, &s)| s >= cfg.suitability_threshold)
        .map(|(i, &s)| (i, s + rng.next_f32() * 0.1))
        .collect();
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    for (idx, suit) in candidates.iter() {
        if settlements.len() >= cfg.max_settlements as usize { break; }
        let x = *idx % w;
        let y = *idx / w;
        let fx = x as f32 / w as f32;
        let fy = y as f32 / h as f32;

        // Check min distance
        let too_close = settlements.iter().any(|s| {
            let dx = s.x - fx;
            let dy = s.y - fy;
            dx*dx + dy*dy < min_dist_sq
        });
        if too_close { continue; }

        let st = if *suit > 0.8 && settlements.is_empty() { SettlementType::Capital }
            else if *suit > 0.75 { SettlementType::City }
            else if *suit > 0.65 { SettlementType::Town }
            else if *suit > 0.55 { SettlementType::Village }
            else if rng.next_f32() < 0.3 { SettlementType::Fort }
            else { SettlementType::Hamlet };

        let name = generate_settlement_name(rng);
        let pop_range = st.population_range();
        let pop = pop_range.0 + (rng.next_u64() % (pop_range.1 - pop_range.0) as u64) as u32;
        let mut s = Settlement::new(&name, fx, fy, st);
        s.population = pop;
        s.suitability = *suit;
        s.prosperity = suit * 0.8 + rng.next_f32() * 0.2;
        settlements.push(s);
    }

    // Connect settlements by trade routes
    let count = settlements.len();
    for i in 0..count {
        let mut nearest: Vec<(usize, f32)> = (0..count).filter(|&j| j != i).map(|j| {
            let dx = settlements[i].x - settlements[j].x;
            let dy = settlements[i].y - settlements[j].y;
            (j, dx*dx + dy*dy)
        }).collect();
        nearest.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let connections: Vec<usize> = nearest.iter().take(3).map(|&(j, _)| j).collect();
        settlements[i].trade_connections = connections;
    }
    settlements
}

// =================================================================
// WORLD GEN EXPANSION: WORLD MAP LEGEND & ANNOTATION TOOLS
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapLabel {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: Color32,
    pub label_type: MapLabelType,
    pub visible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MapLabelType {
    Region,
    Settlement,
    Mountain,
    River,
    Sea,
    Forest,
    Desert,
    Custom,
}

impl MapLabel {
    pub fn new(text: &str, x: f32, y: f32) -> Self {
        Self {
            text: text.to_string(),
            x, y,
            font_size: 14.0,
            color: Color32::WHITE,
            label_type: MapLabelType::Custom,
            visible: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapAnnotation {
    pub id: u32,
    pub points: Vec<[f32; 2]>,
    pub color: Color32,
    pub thickness: f32,
    pub closed: bool,
    pub filled: bool,
    pub label: Option<String>,
    pub annotation_type: MapAnnotationType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MapAnnotationType {
    Border,
    Road,
    River,
    Region,
    Highlight,
    Arrow,
    Freeform,
}

impl MapAnnotation {
    pub fn new_border(id: u32, color: Color32) -> Self {
        Self { id, points: Vec::new(), color, thickness: 2.0, closed: true, filled: false, label: None, annotation_type: MapAnnotationType::Border }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldMapEditorState {
    pub labels: Vec<MapLabel>,
    pub annotations: Vec<MapAnnotation>,
    pub show_labels: bool,
    pub show_annotations: bool,
    pub show_grid: bool,
    pub show_coordinates: bool,
    pub selected_label: Option<usize>,
    pub selected_annotation: Option<usize>,
    pub active_tool: WorldMapTool,
    pub edit_text: String,
    pub annotation_color: Color32,
    pub annotation_thickness: f32,
    pub label_font_size: f32,
    pub zoom: f32,
    pub pan: [f32; 2],
    pub next_annotation_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum WorldMapTool {
    Select,
    AddLabel,
    DrawBorder,
    DrawRoad,
    DrawRiver,
    EraseLabel,
    EraseAnnotation,
}

impl Default for WorldMapEditorState {
    fn default() -> Self {
        Self {
            labels: Vec::new(),
            annotations: Vec::new(),
            show_labels: true,
            show_annotations: true,
            show_grid: false,
            show_coordinates: true,
            selected_label: None,
            selected_annotation: None,
            active_tool: WorldMapTool::Select,
            edit_text: String::new(),
            annotation_color: Color32::from_rgb(200, 100, 50),
            annotation_thickness: 2.0,
            label_font_size: 14.0,
            zoom: 1.0,
            pan: [0.0, 0.0],
            next_annotation_id: 1,
        }
    }
}

pub fn show_world_map_editor(ui: &mut egui::Ui, state: &mut WorldMapEditorState) {
    ui.horizontal(|ui| {
        ui.label("World Map Annotations");
        ui.separator();
        ui.checkbox(&mut state.show_labels, "Labels");
        ui.checkbox(&mut state.show_annotations, "Annotations");
        ui.checkbox(&mut state.show_grid, "Grid");
    });

    ui.horizontal(|ui| {
        ui.label("Tool:");
        for (tool, label) in [
            (WorldMapTool::Select, "Select"),
            (WorldMapTool::AddLabel, "Label"),
            (WorldMapTool::DrawBorder, "Border"),
            (WorldMapTool::DrawRoad, "Road"),
            (WorldMapTool::DrawRiver, "River"),
        ] {
            if ui.selectable_label(state.active_tool == tool, label).clicked() {
                state.active_tool = tool;
            }
        }
    });

    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        ui.collapsing("Labels", |ui| {
            if ui.button("+ Add Label").clicked() && !state.edit_text.is_empty() {
                state.labels.push(MapLabel::new(&state.edit_text.clone(), 0.5, 0.5));
                state.edit_text.clear();
            }
            ui.text_edit_singleline(&mut state.edit_text);
            ui.add(egui::Slider::new(&mut state.label_font_size, 8.0..=48.0).text("Font Size"));
            let mut to_remove = None;
            for (i, label) in state.labels.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut label.visible, "");
                    ui.text_edit_singleline(&mut label.text);
                    ui.add(egui::DragValue::new(&mut label.x).speed(0.001).prefix("X:"));
                    ui.add(egui::DragValue::new(&mut label.y).speed(0.001).prefix("Y:"));
                    if ui.button("🗑").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(i) = to_remove { state.labels.remove(i); }
        });

        ui.collapsing("Annotations", |ui| {
            ui.horizontal(|ui| {
                ui.label("Color:");
                egui::color_picker::color_edit_button_srgba(ui, &mut state.annotation_color, egui::color_picker::Alpha::Opaque);
                ui.add(egui::Slider::new(&mut state.annotation_thickness, 1.0..=10.0).text("Thickness"));
            });
            let mut to_remove = None;
            for (i, ann) in state.annotations.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}: {} pts", i, ann.points.len()));
                    if ui.button("Select").clicked() { state.selected_annotation = Some(i); }
                    if ui.button("🗑").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(i) = to_remove { state.annotations.remove(i); }
        });
    });
}

// =================================================================
// WORLD GEN EXPANSION: REGION EDITOR & POLITICAL BOUNDARIES
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoliticalRegion {
    pub name: String,
    pub color: Color32,
    pub territory_cells: Vec<usize>,
    pub capital_settlement: Option<usize>,
    pub population: u32,
    pub government_type: GovernmentType,
    pub military_strength: f32,
    pub economic_strength: f32,
    pub diplomacy: Vec<DiplomaticRelation>,
    pub flag_colors: [Color32; 3],
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GovernmentType {
    Monarchy,
    Republic,
    Empire,
    Theocracy,
    Oligarchy,
    Tribal,
    Confederation,
    Dictatorship,
}

impl GovernmentType {
    pub fn name(&self) -> &'static str {
        match self {
            GovernmentType::Monarchy => "Monarchy",
            GovernmentType::Republic => "Republic",
            GovernmentType::Empire => "Empire",
            GovernmentType::Theocracy => "Theocracy",
            GovernmentType::Oligarchy => "Oligarchy",
            GovernmentType::Tribal => "Tribal",
            GovernmentType::Confederation => "Confederation",
            GovernmentType::Dictatorship => "Dictatorship",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiplomaticRelation {
    pub other_region_id: usize,
    pub relation_type: DiplomacyType,
    pub strength: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DiplomacyType {
    Allied,
    Friendly,
    Neutral,
    Tense,
    Hostile,
    AtWar,
    Vassal,
    Overlord,
}

impl DiplomacyType {
    pub fn color(&self) -> Color32 {
        match self {
            DiplomacyType::Allied => Color32::from_rgb(50, 200, 50),
            DiplomacyType::Friendly => Color32::from_rgb(150, 220, 150),
            DiplomacyType::Neutral => Color32::from_rgb(200, 200, 200),
            DiplomacyType::Tense => Color32::from_rgb(220, 180, 50),
            DiplomacyType::Hostile => Color32::from_rgb(220, 100, 50),
            DiplomacyType::AtWar => Color32::from_rgb(220, 30, 30),
            DiplomacyType::Vassal => Color32::from_rgb(180, 140, 200),
            DiplomacyType::Overlord => Color32::from_rgb(140, 100, 200),
        }
    }
}

impl PoliticalRegion {
    pub fn new(name: &str, color: Color32) -> Self {
        Self {
            name: name.to_string(),
            color,
            territory_cells: Vec::new(),
            capital_settlement: None,
            population: 0,
            government_type: GovernmentType::Monarchy,
            military_strength: 0.5,
            economic_strength: 0.5,
            diplomacy: Vec::new(),
            flag_colors: [color, Color32::WHITE, Color32::BLACK],
            description: String::new(),
        }
    }

    pub fn territory_size(&self) -> usize { self.territory_cells.len() }

    pub fn tax_income(&self) -> f32 {
        self.population as f32 * self.economic_strength * 0.01
    }

    pub fn military_units(&self) -> u32 {
        (self.population as f32 * self.military_strength * 0.001) as u32
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoliticalMapConfig {
    pub region_count: u32,
    pub seed: u64,
    pub expansion_iterations: u32,
    pub allow_enclaves: bool,
}

impl Default for PoliticalMapConfig {
    fn default() -> Self {
        Self { region_count: 8, seed: 42, expansion_iterations: 500, allow_enclaves: false }
    }
}

pub fn generate_political_map(cfg: &PoliticalMapConfig, settlements: &[Settlement], w: usize, h: usize, rng: &mut SimpleRng) -> Vec<PoliticalRegion> {
    let mut regions: Vec<PoliticalRegion> = Vec::new();
    let colors = [
        Color32::from_rgb(200, 100, 100), Color32::from_rgb(100, 200, 100),
        Color32::from_rgb(100, 100, 200), Color32::from_rgb(200, 200, 100),
        Color32::from_rgb(200, 100, 200), Color32::from_rgb(100, 200, 200),
        Color32::from_rgb(255, 150, 50),  Color32::from_rgb(150, 255, 100),
    ];
    let govts = [GovernmentType::Monarchy, GovernmentType::Republic, GovernmentType::Empire, GovernmentType::Theocracy, GovernmentType::Oligarchy, GovernmentType::Tribal, GovernmentType::Confederation, GovernmentType::Dictatorship];
    let region_names = ["Aldoria", "Brethmark", "Calyvion", "Duskweald", "Eranthos", "Fenvale", "Greyspire", "Hallenmoor", "Ironreach", "Jadeshire"];

    for i in 0..cfg.region_count as usize {
        let name = if i < region_names.len() { region_names[i] } else { "Region" };
        let color = colors[i % colors.len()];
        let mut r = PoliticalRegion::new(name, color);
        r.government_type = govts[i % govts.len()].clone();
        r.military_strength = rng.next_f32_range(0.2, 1.0);
        r.economic_strength = rng.next_f32_range(0.2, 1.0);
        // Assign capital from settlements
        if let Some(cap_idx) = settlements.iter().enumerate()
            .filter(|(_, s)| matches!(s.settlement_type, SettlementType::Capital | SettlementType::City))
            .nth(i) {
            r.capital_settlement = Some(cap_idx.0);
        }
        regions.push(r);
    }

    // Voronoi expansion from capital positions
    let mut territory_map = vec![usize::MAX; w * h];
    let mut seeds: Vec<(usize, usize)> = Vec::new(); // (x, y) for each region seed
    for (ri, region) in regions.iter().enumerate() {
        if let Some(cap_idx) = region.capital_settlement {
            if cap_idx < settlements.len() {
                let s = &settlements[cap_idx];
                let sx = (s.x * w as f32) as usize;
                let sy = (s.y * h as f32) as usize;
                seeds.push((sx.min(w-1), sy.min(h-1)));
                territory_map[sy.min(h-1) * w + sx.min(w-1)] = ri;
                continue;
            }
        }
        let sx = rng.next_u64() as usize % w;
        let sy = rng.next_u64() as usize % h;
        seeds.push((sx, sy));
        territory_map[sy * w + sx] = ri;
    }

    // BFS expansion
    let mut queue: std::collections::VecDeque<(usize, usize, usize)> = seeds.iter().enumerate()
        .map(|(ri, &(sx, sy))| (sx, sy, ri)).collect();
    let neighbors4: [(i32,i32); 4] = [(0,-1),(0,1),(-1,0),(1,0)];
    for _ in 0..cfg.expansion_iterations {
        if queue.is_empty() { break; }
        let (x, y, ri) = queue.pop_front().unwrap();
        for &(dy, dx) in &neighbors4 {
            let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
            let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
            let nidx = ny * w + nx;
            if territory_map[nidx] == usize::MAX {
                territory_map[nidx] = ri;
                queue.push_back((nx, ny, ri));
            }
        }
    }

    // Collect cells for each region
    for (idx, &ri) in territory_map.iter().enumerate() {
        if ri < regions.len() { regions[ri].territory_cells.push(idx); }
    }

    // Generate diplomatic relations
    let region_count = regions.len();
    for i in 0..region_count {
        let relations: Vec<DiplomaticRelation> = (0..region_count).filter(|&j| j != i).map(|j| {
            let roll = rng.next_f32();
            let rel_type = if roll < 0.1 { DiplomacyType::Allied }
                else if roll < 0.25 { DiplomacyType::Friendly }
                else if roll < 0.6 { DiplomacyType::Neutral }
                else if roll < 0.8 { DiplomacyType::Tense }
                else if roll < 0.95 { DiplomacyType::Hostile }
                else { DiplomacyType::AtWar };
            DiplomaticRelation { other_region_id: j, relation_type: rel_type, strength: rng.next_f32() }
        }).collect();
        regions[i].diplomacy = relations;
    }

    regions
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PoliticalEditorState {
    pub regions: Vec<PoliticalRegion>,
    pub config: PoliticalMapConfig,
    pub selected_region: Option<usize>,
    pub show_borders: bool,
    pub show_capitals: bool,
    pub show_diplomacy: bool,
    pub view_mode: PoliticalViewMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum PoliticalViewMode {
    #[default]
    Territory,
    Military,
    Economic,
    Diplomacy,
}

pub fn show_political_editor(ui: &mut egui::Ui, state: &mut PoliticalEditorState) {
    ui.horizontal(|ui| {
        ui.label("Political Map Editor");
        ui.separator();
        ui.checkbox(&mut state.show_borders, "Borders");
        ui.checkbox(&mut state.show_capitals, "Capitals");
        ui.checkbox(&mut state.show_diplomacy, "Diplomacy");
    });

    ui.horizontal(|ui| {
        ui.label("View:");
        for (mode, label) in [
            (PoliticalViewMode::Territory, "Territory"),
            (PoliticalViewMode::Military, "Military"),
            (PoliticalViewMode::Economic, "Economic"),
            (PoliticalViewMode::Diplomacy, "Diplomacy"),
        ] {
            if ui.selectable_label(state.view_mode == mode, label).clicked() {
                state.view_mode = mode;
            }
        }
    });

    ui.add(egui::DragValue::new(&mut state.config.region_count).clamp_range(2..=16u32).prefix("Regions: "));
    ui.add(egui::DragValue::new(&mut state.config.seed).prefix("Seed: "));

    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for (i, region) in state.regions.iter_mut().enumerate() {
            let selected = state.selected_region == Some(i);
            let resp = ui.selectable_label(selected, format!("{} ({}) - Pop: {}", region.name, region.government_type.name(), region.population));
            if resp.clicked() { state.selected_region = Some(i); }
        }
    });

    if let Some(idx) = state.selected_region {
        if let Some(region) = state.regions.get_mut(idx) {
            ui.separator();
            ui.heading(&region.name.clone());
            ui.text_edit_singleline(&mut region.name);
            ui.label(format!("Territory: {} cells", region.territory_size()));
            ui.label(format!("Tax income: {:.0}", region.tax_income()));
            ui.label(format!("Military units: {}", region.military_units()));
            ui.add(egui::Slider::new(&mut region.military_strength, 0.0..=1.0).text("Military"));
            ui.add(egui::Slider::new(&mut region.economic_strength, 0.0..=1.0).text("Economy"));
            ui.text_edit_multiline(&mut region.description);
        }
    }
}

// =================================================================
// WORLD GEN EXPANSION: WEATHER EVENTS & SEASONAL SIMULATION
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Season {
    Spring, Summer, Autumn, Winter,
}

impl Season {
    pub fn next(&self) -> Season {
        match self {
            Season::Spring => Season::Summer,
            Season::Summer => Season::Autumn,
            Season::Autumn => Season::Winter,
            Season::Winter => Season::Spring,
        }
    }

    pub fn temp_modifier(&self) -> f32 {
        match self {
            Season::Spring => 0.0,
            Season::Summer => 8.0,
            Season::Autumn => -3.0,
            Season::Winter => -15.0,
        }
    }

    pub fn precip_modifier(&self) -> f32 {
        match self {
            Season::Spring => 1.3,
            Season::Summer => 0.8,
            Season::Autumn => 1.1,
            Season::Winter => 0.9,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum WeatherEventType {
    Drought,
    Flood,
    Blizzard,
    Heatwave,
    Storm,
    Fog,
    Hurricane,
    Tornado,
    Hail,
    ClearSkies,
}

impl WeatherEventType {
    pub fn name(&self) -> &'static str {
        match self {
            WeatherEventType::Drought => "Drought",
            WeatherEventType::Flood => "Flood",
            WeatherEventType::Blizzard => "Blizzard",
            WeatherEventType::Heatwave => "Heatwave",
            WeatherEventType::Storm => "Thunderstorm",
            WeatherEventType::Fog => "Dense Fog",
            WeatherEventType::Hurricane => "Hurricane",
            WeatherEventType::Tornado => "Tornado",
            WeatherEventType::Hail => "Hailstorm",
            WeatherEventType::ClearSkies => "Clear Skies",
        }
    }

    pub fn severity(&self) -> f32 {
        match self {
            WeatherEventType::ClearSkies | WeatherEventType::Fog => 0.1,
            WeatherEventType::Hail | WeatherEventType::Storm => 0.5,
            WeatherEventType::Blizzard | WeatherEventType::Heatwave | WeatherEventType::Drought => 0.7,
            WeatherEventType::Flood | WeatherEventType::Tornado => 0.85,
            WeatherEventType::Hurricane => 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeatherEvent {
    pub event_type: WeatherEventType,
    pub center: [f32; 2],
    pub radius: f32,
    pub duration_days: u32,
    pub days_active: u32,
    pub intensity: f32,
    pub movement: [f32; 2],
}

impl WeatherEvent {
    pub fn new(et: WeatherEventType, cx: f32, cy: f32) -> Self {
        Self {
            intensity: et.severity(),
            event_type: et,
            center: [cx, cy],
            radius: 0.1,
            duration_days: 7,
            days_active: 0,
            movement: [0.01, 0.005],
        }
    }

    pub fn tick(&mut self) {
        self.days_active += 1;
        self.center[0] += self.movement[0];
        self.center[1] += self.movement[1];
        // Wrap around world
        if self.center[0] > 1.0 { self.center[0] -= 1.0; }
        if self.center[1] > 1.0 { self.center[1] -= 1.0; }
        if self.center[0] < 0.0 { self.center[0] += 1.0; }
        if self.center[1] < 0.0 { self.center[1] += 1.0; }
    }

    pub fn is_expired(&self) -> bool { self.days_active >= self.duration_days }
    pub fn affects_point(&self, x: f32, y: f32) -> bool {
        let dx = x - self.center[0];
        let dy = y - self.center[1];
        dx*dx + dy*dy < self.radius * self.radius
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct WeatherSimState {
    pub events: Vec<WeatherEvent>,
    pub current_season: Season,
    pub day: u32,
    pub year: i32,
    pub auto_generate: bool,
    pub event_frequency: f32,
    pub seed: u64,
}

impl Default for Season { fn default() -> Self { Season::Spring } }

impl WeatherSimState {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            current_season: Season::Spring,
            day: 0,
            year: 0,
            auto_generate: false,
            event_frequency: 0.1,
            seed: 42,
        }
    }

    pub fn advance_day(&mut self, rng: &mut SimpleRng) {
        self.day += 1;
        if self.day >= 90 {
            self.day = 0;
            self.current_season = self.current_season.next();
            if self.current_season == Season::Spring { self.year += 1; }
        }

        // Tick existing events
        self.events.iter_mut().for_each(|e| e.tick());
        self.events.retain(|e| !e.is_expired());

        // Generate new events randomly
        if self.auto_generate && rng.next_f32() < self.event_frequency {
            let event_types = [
                WeatherEventType::Storm, WeatherEventType::Fog, WeatherEventType::Hail,
                WeatherEventType::Drought, WeatherEventType::Flood, WeatherEventType::ClearSkies,
            ];
            let et = event_types[rng.next_u64() as usize % event_types.len()].clone();
            let cx = rng.next_f32();
            let cy = rng.next_f32();
            self.events.push(WeatherEvent::new(et, cx, cy));
        }
    }
}

pub fn show_weather_editor(ui: &mut egui::Ui, state: &mut WeatherSimState) {
    ui.label(format!("Year {} | Season: {:?} | Day {}", state.year, state.current_season, state.day));
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.auto_generate, "Auto-generate events");
        ui.add(egui::Slider::new(&mut state.event_frequency, 0.0..=0.5).text("Frequency"));
    });

    if ui.button("Add Storm").clicked() { state.events.push(WeatherEvent::new(WeatherEventType::Storm, 0.5, 0.5)); }
    if ui.button("Add Drought").clicked() { state.events.push(WeatherEvent::new(WeatherEventType::Drought, 0.5, 0.5)); }
    if ui.button("Advance Day").clicked() {
        let mut rng = SimpleRng::new(state.seed + state.day as u64);
        state.advance_day(&mut rng);
    }

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        for (i, event) in state.events.iter().enumerate() {
            ui.label(format!("[{}] {} - Day {}/{} (Intensity: {:.2})", i, event.event_type.name(), event.days_active, event.duration_days, event.intensity));
        }
        if state.events.is_empty() { ui.label("No active weather events"); }
    });
}

// =================================================================
// WORLD GEN EXPANSION: UNDERGROUND WORLD / CAVERN SYSTEMS
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum UndergroundFeatureType {
    CrystalCavern,
    LavaPool,
    UndergroundLake,
    AncientRuins,
    MushoomForest,
    GeothermalVent,
    CrystalFormation,
    FossilBed,
    MineralDeposit,
    HiddenShrine,
}

impl UndergroundFeatureType {
    pub fn name(&self) -> &'static str {
        match self {
            UndergroundFeatureType::CrystalCavern => "Crystal Cavern",
            UndergroundFeatureType::LavaPool => "Lava Pool",
            UndergroundFeatureType::UndergroundLake => "Underground Lake",
            UndergroundFeatureType::AncientRuins => "Ancient Ruins",
            UndergroundFeatureType::MushoomForest => "Mushroom Forest",
            UndergroundFeatureType::GeothermalVent => "Geothermal Vent",
            UndergroundFeatureType::CrystalFormation => "Crystal Formation",
            UndergroundFeatureType::FossilBed => "Fossil Bed",
            UndergroundFeatureType::MineralDeposit => "Mineral Deposit",
            UndergroundFeatureType::HiddenShrine => "Hidden Shrine",
        }
    }

    pub fn rarity(&self) -> f32 {
        match self {
            UndergroundFeatureType::MineralDeposit | UndergroundFeatureType::FossilBed => 0.3,
            UndergroundFeatureType::UndergroundLake | UndergroundFeatureType::MushoomForest | UndergroundFeatureType::GeothermalVent => 0.15,
            UndergroundFeatureType::CrystalFormation | UndergroundFeatureType::CrystalCavern => 0.08,
            UndergroundFeatureType::LavaPool | UndergroundFeatureType::AncientRuins => 0.05,
            UndergroundFeatureType::HiddenShrine => 0.02,
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            UndergroundFeatureType::CrystalCavern | UndergroundFeatureType::CrystalFormation => Color32::from_rgb(180, 100, 255),
            UndergroundFeatureType::LavaPool => Color32::from_rgb(255, 80, 20),
            UndergroundFeatureType::UndergroundLake => Color32::from_rgb(50, 120, 200),
            UndergroundFeatureType::AncientRuins | UndergroundFeatureType::HiddenShrine => Color32::from_rgb(220, 180, 100),
            UndergroundFeatureType::MushoomForest => Color32::from_rgb(180, 50, 200),
            UndergroundFeatureType::GeothermalVent => Color32::from_rgb(255, 160, 50),
            UndergroundFeatureType::FossilBed => Color32::from_rgb(160, 140, 100),
            UndergroundFeatureType::MineralDeposit => Color32::from_rgb(100, 200, 180),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UndergroundLayer {
    pub depth: u32,
    pub name: String,
    pub cave_map: CaveMap,
    pub features: Vec<(usize, usize, UndergroundFeatureType)>,
    pub connections_to_above: Vec<(usize, usize)>,
    pub connections_to_below: Vec<(usize, usize)>,
    pub ambient_light: f32,
    pub temperature: f32,
    pub danger_level: u32,
}

impl UndergroundLayer {
    pub fn new(depth: u32, w: usize, h: usize) -> Self {
        let layer_names = ["Upper Caverns", "Mid Depths", "Deep Delve", "Abyssal Reaches", "Primordial Depths"];
        let name = layer_names.get(depth as usize).unwrap_or(&"Unknown Depths").to_string();
        Self {
            depth,
            name,
            cave_map: CaveMap::new(w, h),
            features: Vec::new(),
            connections_to_above: Vec::new(),
            connections_to_below: Vec::new(),
            ambient_light: (1.0 - depth as f32 * 0.2).max(0.0),
            temperature: 15.0 + depth as f32 * 5.0,
            danger_level: depth + 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UndergroundWorldConfig {
    pub layer_count: u32,
    pub layer_width: usize,
    pub layer_height: usize,
    pub feature_density: f32,
    pub connection_count: u32,
    pub seed: u64,
}

impl Default for UndergroundWorldConfig {
    fn default() -> Self {
        Self { layer_count: 3, layer_width: 64, layer_height: 64, feature_density: 0.05, connection_count: 5, seed: 99999 }
    }
}

pub fn generate_underground_world(cfg: &UndergroundWorldConfig) -> Vec<UndergroundLayer> {
    let mut rng = SimpleRng::new(cfg.seed);
    let mut layers: Vec<UndergroundLayer> = Vec::new();

    let cave_cfg = CaveConfig {
        width: cfg.layer_width, height: cfg.layer_height,
        initial_wall_chance: 0.45 + cfg.feature_density * 0.5,
        smoothing_iterations: 5,
        birth_limit: 4, death_limit: 3,
        min_chamber_size: 20,
        seed: cfg.seed,
    };

    for depth in 0..cfg.layer_count {
        let mut layer = UndergroundLayer::new(depth, cfg.layer_width, cfg.layer_height);
        let mut layer_cave_cfg = cave_cfg.clone();
        layer_cave_cfg.seed = cfg.seed + depth as u64 * 1000;
        layer_cave_cfg.initial_wall_chance = 0.4 + depth as f32 * 0.03;
        layer.cave_map = generate_cave(&layer_cave_cfg, &mut SimpleRng::new(layer_cave_cfg.seed));

        // Place features
        let feature_types = [
            UndergroundFeatureType::MineralDeposit, UndergroundFeatureType::FossilBed,
            UndergroundFeatureType::UndergroundLake, UndergroundFeatureType::CrystalFormation,
            UndergroundFeatureType::AncientRuins, UndergroundFeatureType::HiddenShrine,
        ];
        let feature_count = (cfg.layer_width * cfg.layer_height) as f32 * cfg.feature_density;
        for _ in 0..feature_count as u32 {
            let fx = rng.next_u64() as usize % cfg.layer_width;
            let fy = rng.next_u64() as usize % cfg.layer_height;
            if layer.cave_map.tiles[fy * cfg.layer_width + fx] == CaveTile::Floor {
                let ft = feature_types[rng.next_u64() as usize % feature_types.len()].clone();
                layer.features.push((fx, fy, ft));
            }
        }

        // Add connections to above layer
        if depth > 0 {
            for _ in 0..cfg.connection_count {
                let cx = rng.next_u64() as usize % cfg.layer_width;
                let cy = rng.next_u64() as usize % cfg.layer_height;
                layer.connections_to_above.push((cx, cy));
            }
        }
        if depth < cfg.layer_count - 1 {
            for _ in 0..cfg.connection_count {
                let cx = rng.next_u64() as usize % cfg.layer_width;
                let cy = rng.next_u64() as usize % cfg.layer_height;
                layer.connections_to_below.push((cx, cy));
            }
        }
        layers.push(layer);
    }
    layers
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct UndergroundEditorState {
    pub layers: Vec<UndergroundLayer>,
    pub config: UndergroundWorldConfig,
    pub active_layer: usize,
    pub selected_feature: Option<usize>,
    pub show_features: bool,
    pub show_connections: bool,
    pub zoom: f32,
    pub pan: [f32; 2],
}

pub fn show_underground_editor(ui: &mut egui::Ui, state: &mut UndergroundEditorState) {
    ui.horizontal(|ui| {
        ui.label("Underground World Editor");
        if ui.button("Generate").clicked() {
            state.layers = generate_underground_world(&state.config);
        }
    });

    ui.add(egui::Slider::new(&mut state.config.layer_count, 1..=5).text("Layers"));
    ui.add(egui::Slider::new(&mut state.config.feature_density, 0.01..=0.2).text("Feature Density"));
    ui.add(egui::DragValue::new(&mut state.config.seed).prefix("Seed: "));

    ui.separator();
    ui.label("Layer:");
    ui.horizontal(|ui| {
        for i in 0..state.layers.len() {
            let name = state.layers[i].name.clone();
            if ui.selectable_label(state.active_layer == i, &name).clicked() {
                state.active_layer = i;
            }
        }
    });

    if let Some(layer) = state.layers.get(state.active_layer) {
        ui.label(format!("Depth: {} | Danger: {} | Temp: {:.0}°C", layer.depth, layer.danger_level, layer.temperature));
        ui.label(format!("Features: {} | Light: {:.0}%", layer.features.len(), layer.ambient_light * 100.0));

        ui.checkbox(&mut state.show_features, "Show Features");
        ui.checkbox(&mut state.show_connections, "Show Connections");

        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for (i, (fx, fy, ft)) in layer.features.iter().enumerate() {
                ui.label(format!("  {} at ({}, {})", ft.name(), fx, fy));
            }
        });
    }
}

// =================================================================
// WORLD GEN EXPANSION: ECOLOGY / FLORA & FAUNA PLACEMENT
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum FloraType {
    TropicalRainforest,
    TemperateForest,
    BorealForest,
    Savanna,
    Grassland,
    Desert,
    Tundra,
    WetlandMarsh,
    Alpine,
    Mangrove,
    Coral,
    Kelp,
}

impl FloraType {
    pub fn name(&self) -> &'static str {
        match self {
            FloraType::TropicalRainforest => "Tropical Rainforest",
            FloraType::TemperateForest => "Temperate Forest",
            FloraType::BorealForest => "Boreal Forest",
            FloraType::Savanna => "Savanna",
            FloraType::Grassland => "Grassland",
            FloraType::Desert => "Desert",
            FloraType::Tundra => "Tundra",
            FloraType::WetlandMarsh => "Wetland/Marsh",
            FloraType::Alpine => "Alpine Meadow",
            FloraType::Mangrove => "Mangrove",
            FloraType::Coral => "Coral Reef",
            FloraType::Kelp => "Kelp Forest",
        }
    }

    pub fn from_climate(temp: f32, precip: f32, elev: f32, is_ocean: bool) -> Self {
        if is_ocean { return if temp > 25.0 { FloraType::Coral } else { FloraType::Kelp }; }
        if elev > 0.8 { return FloraType::Alpine; }
        if temp < -5.0 { return FloraType::Tundra; }
        if temp < 5.0 { return FloraType::BorealForest; }
        if precip < 200.0 { return FloraType::Desert; }
        if precip < 500.0 { return if temp > 20.0 { FloraType::Savanna } else { FloraType::Grassland }; }
        if precip > 1500.0 && temp > 20.0 { return FloraType::TropicalRainforest; }
        if precip > 600.0 { return FloraType::TemperateForest; }
        FloraType::Grassland
    }

    pub fn color(&self) -> Color32 {
        match self {
            FloraType::TropicalRainforest => Color32::from_rgb(0, 120, 20),
            FloraType::TemperateForest => Color32::from_rgb(30, 140, 50),
            FloraType::BorealForest => Color32::from_rgb(40, 100, 60),
            FloraType::Savanna => Color32::from_rgb(200, 170, 80),
            FloraType::Grassland => Color32::from_rgb(140, 200, 80),
            FloraType::Desert => Color32::from_rgb(230, 200, 130),
            FloraType::Tundra => Color32::from_rgb(180, 200, 180),
            FloraType::WetlandMarsh => Color32::from_rgb(80, 150, 100),
            FloraType::Alpine => Color32::from_rgb(160, 200, 150),
            FloraType::Mangrove => Color32::from_rgb(50, 130, 80),
            FloraType::Coral => Color32::from_rgb(255, 150, 150),
            FloraType::Kelp => Color32::from_rgb(80, 120, 60),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaunaEntry {
    pub name: String,
    pub is_predator: bool,
    pub population_density: f32,
    pub preferred_flora: Vec<FloraType>,
    pub danger_level: u32,
    pub icon: char,
}

impl FaunaEntry {
    pub fn wolf() -> Self { Self { name: "Wolf".into(), is_predator: true, population_density: 0.3, preferred_flora: vec![FloraType::TemperateForest, FloraType::BorealForest], danger_level: 3, icon: 'w' } }
    pub fn bear() -> Self { Self { name: "Bear".into(), is_predator: true, population_density: 0.2, preferred_flora: vec![FloraType::TemperateForest, FloraType::BorealForest], danger_level: 4, icon: 'B' } }
    pub fn deer() -> Self { Self { name: "Deer".into(), is_predator: false, population_density: 0.8, preferred_flora: vec![FloraType::TemperateForest, FloraType::Grassland], danger_level: 0, icon: 'd' } }
    pub fn camel() -> Self { Self { name: "Camel".into(), is_predator: false, population_density: 0.4, preferred_flora: vec![FloraType::Desert, FloraType::Savanna], danger_level: 1, icon: 'c' } }
    pub fn lion() -> Self { Self { name: "Lion".into(), is_predator: true, population_density: 0.3, preferred_flora: vec![FloraType::Savanna, FloraType::Grassland], danger_level: 5, icon: 'L' } }
    pub fn penguin() -> Self { Self { name: "Penguin".into(), is_predator: false, population_density: 0.6, preferred_flora: vec![FloraType::Tundra], danger_level: 0, icon: 'P' } }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EcologyMap {
    pub flora: Vec<FloraType>,
    pub fauna_density: Vec<f32>,
    pub w: usize,
    pub h: usize,
}

impl EcologyMap {
    pub fn generate(climate: &[ClimateCell], heightmap: &[f32], ocean_map: &[bool], w: usize, h: usize) -> Self {
        let mut flora = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let cl = &climate[idx];
                let ft = FloraType::from_climate(cl.temperature, cl.precipitation, heightmap[idx], ocean_map[idx]);
                flora.push(ft);
            }
        }
        let fauna_density = vec![0.5f32; w * h];
        Self { flora, fauna_density, w, h }
    }

    pub fn flora_type_at(&self, x: usize, y: usize) -> Option<&FloraType> {
        self.flora.get(y * self.w + x)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EcologyEditorState {
    pub ecology: EcologyMap,
    pub known_fauna: Vec<FaunaEntry>,
    pub show_flora: bool,
    pub show_fauna: bool,
    pub selected_fauna: Option<usize>,
}

impl EcologyEditorState {
    pub fn new() -> Self {
        Self {
            ecology: EcologyMap::default(),
            known_fauna: vec![
                FaunaEntry::wolf(), FaunaEntry::bear(), FaunaEntry::deer(),
                FaunaEntry::camel(), FaunaEntry::lion(), FaunaEntry::penguin(),
            ],
            show_flora: true,
            show_fauna: true,
            selected_fauna: None,
        }
    }
}

pub fn show_ecology_editor(ui: &mut egui::Ui, state: &mut EcologyEditorState) {
    ui.horizontal(|ui| {
        ui.label("Ecology Editor");
        ui.checkbox(&mut state.show_flora, "Flora");
        ui.checkbox(&mut state.show_fauna, "Fauna");
    });

    ui.collapsing("Fauna Registry", |ui| {
        for (i, fauna) in state.known_fauna.iter().enumerate() {
            let selected = state.selected_fauna == Some(i);
            ui.selectable_label(selected, format!("{} ({}) Danger: {}", fauna.name, fauna.icon, fauna.danger_level));
        }
    });

    ui.collapsing("Flora Summary", |ui| {
        let flora_counts = [
            FloraType::TropicalRainforest, FloraType::TemperateForest, FloraType::BorealForest,
            FloraType::Savanna, FloraType::Grassland, FloraType::Desert, FloraType::Tundra,
        ];
        for ft in &flora_counts {
            let count = state.ecology.flora.iter().filter(|f| *f == ft).count();
            if count > 0 {
                ui.label(format!("{}: {} cells", ft.name(), count));
            }
        }
    });
}

// =================================================================
// WORLD GEN EXPANSION: COMPREHENSIVE EDITOR INTEGRATION STATE
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FullWorldEditorState {
    pub erosion_config: ErosionConfig,
    pub river_network: RiverNetwork,
    pub tectonic_config: TectonicSimConfig,
    pub climate_config: ClimateSimConfig,
    pub climate_cells: Vec<ClimateCell>,
    pub settlements: Vec<Settlement>,
    pub settlement_config: SettlementPlacementConfig,
    pub political_state: PoliticalEditorState,
    pub weather_state: WeatherSimState,
    pub underground_state: UndergroundEditorState,
    pub ecology_state: EcologyEditorState,
    pub world_map_state: WorldMapEditorState,
    pub active_tab: FullWorldTab,
    pub ocean_map: Vec<bool>,
    pub heightmap_cache: Vec<f32>,
    pub world_w: usize,
    pub world_h: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum FullWorldTab {
    #[default]
    Heightmap,
    Tectonics,
    Climate,
    Rivers,
    Erosion,
    Settlements,
    Political,
    Weather,
    Underground,
    Ecology,
    Annotations,
    Export,
}

impl FullWorldEditorState {
    pub fn new(w: usize, h: usize) -> Self {
        let mut s = Self::default();
        s.world_w = w;
        s.world_h = h;
        s.river_network = RiverNetwork::new(w, h);
        s.ocean_map = vec![false; w * h];
        s.heightmap_cache = vec![0.5; w * h];
        s
    }
}

pub fn show_full_world_editor(ui: &mut egui::Ui, state: &mut FullWorldEditorState) {
    ui.horizontal(|ui| {
        for (tab, label) in [
            (FullWorldTab::Heightmap, "Height"),
            (FullWorldTab::Tectonics, "Tectonics"),
            (FullWorldTab::Climate, "Climate"),
            (FullWorldTab::Rivers, "Rivers"),
            (FullWorldTab::Erosion, "Erosion"),
            (FullWorldTab::Settlements, "Settlements"),
            (FullWorldTab::Political, "Political"),
            (FullWorldTab::Weather, "Weather"),
            (FullWorldTab::Underground, "Underground"),
            (FullWorldTab::Ecology, "Ecology"),
            (FullWorldTab::Annotations, "Notes"),
            (FullWorldTab::Export, "Export"),
        ] {
            if ui.selectable_label(state.active_tab == tab, label).clicked() {
                state.active_tab = tab;
            }
        }
    });
    ui.separator();
    match state.active_tab {
        FullWorldTab::Tectonics => {
            ui.add(egui::Slider::new(&mut state.tectonic_config.plate_count, 2..=16).text("Plates"));
            ui.add(egui::Slider::new(&mut state.tectonic_config.simulation_steps, 1..=30).text("Steps"));
            if ui.button("Simulate Tectonics").clicked() {
                let mut rng = SimpleRng::new(state.tectonic_config.seed + 1);
                state.heightmap_cache = simulate_tectonics(&state.tectonic_config, &mut rng);
                state.ocean_map = state.heightmap_cache.iter().map(|&h| h < 0.4).collect();
            }
        }
        FullWorldTab::Climate => {
            ui.add(egui::Slider::new(&mut state.climate_config.simulation_steps, 1..=50).text("Steps"));
            if ui.button("Simulate Climate").clicked() {
                state.climate_cells = simulate_climate(&state.climate_config, &state.heightmap_cache, &state.ocean_map);
            }
            ui.label(format!("Climate cells: {}", state.climate_cells.len()));
        }
        FullWorldTab::Rivers => {
            if ui.button("Generate River Network").clicked() {
                let mut rng = SimpleRng::new(state.settlement_config.seed);
                state.river_network = RiverNetwork::new(state.world_w, state.world_h);
                state.river_network.generate_from_heightmap(&state.heightmap_cache, &mut rng);
            }
            ui.label(format!("Rivers: {} | Navigable: {}", state.river_network.river_count(), state.river_network.navigable_rivers().len()));
        }
        FullWorldTab::Erosion => {
            ui.add(egui::Slider::new(&mut state.erosion_config.iterations, 1000..=200000).text("Iterations"));
            ui.add(egui::Slider::new(&mut state.erosion_config.erosion_rate, 0.01..=1.0).text("Erosion Rate"));
            ui.add(egui::Slider::new(&mut state.erosion_config.deposition_rate, 0.01..=1.0).text("Deposition Rate"));
            if ui.button("Run Hydraulic Erosion").clicked() {
                let w = state.world_w; let h = state.world_h;
                let mut rng = SimpleRng::new(54321);
                let cfg = state.erosion_config.clone();
                hydraulic_erosion(&mut state.heightmap_cache, w, h, &cfg, &mut rng);
            }
            if ui.button("Run Thermal Erosion").clicked() {
                let w = state.world_w; let h = state.world_h;
                thermal_erosion(&mut state.heightmap_cache, w, h, 10, 0.01);
            }
        }
        FullWorldTab::Settlements => {
            ui.add(egui::Slider::new(&mut state.settlement_config.max_settlements, 5..=200).text("Max Settlements"));
            ui.add(egui::Slider::new(&mut state.settlement_config.suitability_threshold, 0.1..=0.9).text("Min Suitability"));
            if ui.button("Place Settlements").clicked() {
                let w = state.world_w; let h = state.world_h;
                let flow = state.river_network.flow_map.clone();
                let suitability = compute_settlement_suitability(
                    &state.heightmap_cache, &state.ocean_map,
                    &state.climate_cells.clone().into_iter().chain(std::iter::repeat(ClimateCell::new(0.0))).take(w*h).collect::<Vec<_>>(),
                    &flow, w, h, &state.settlement_config
                );
                let mut rng = SimpleRng::new(state.settlement_config.seed);
                state.settlements = place_settlements(&suitability, w, h, &state.settlement_config, &mut rng);
            }
            ui.label(format!("Settlements: {}", state.settlements.len()));
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for s in &state.settlements {
                    ui.label(format!("[{}] {} ({}) Pop: {}", s.icon(), s.name, s.settlement_type.name(), s.population));
                }
            });
        }
        FullWorldTab::Political => {
            show_political_editor(ui, &mut state.political_state);
            if state.political_state.regions.is_empty() && ui.button("Generate Regions").clicked() {
                let mut rng = SimpleRng::new(state.political_state.config.seed);
                state.political_state.regions = generate_political_map(&state.political_state.config, &state.settlements, state.world_w, state.world_h, &mut rng);
            }
        }
        FullWorldTab::Weather => { show_weather_editor(ui, &mut state.weather_state); }
        FullWorldTab::Underground => { show_underground_editor(ui, &mut state.underground_state); }
        FullWorldTab::Ecology => { show_ecology_editor(ui, &mut state.ecology_state); }
        FullWorldTab::Annotations => { show_world_map_editor(ui, &mut state.world_map_state); }
        _ => { ui.label("Select a tab above"); }
    }
}

// =================================================================
// WORLD GEN EXPANSION: NOISE GENERATORS
// =================================================================

pub fn voronoi_noise(x: f32, y: f32, seed: u64, point_count: usize) -> f32 {
    let mut rng = SimpleRng::new(seed);
    let mut points: Vec<[f32; 2]> = (0..point_count).map(|_| [rng.next_f32(), rng.next_f32()]).collect();
    let mut min_dist = f32::MAX;
    let mut second_dist = f32::MAX;
    for p in &points {
        let dx = x - p[0];
        let dy = y - p[1];
        let d = dx*dx + dy*dy;
        if d < min_dist { second_dist = min_dist; min_dist = d; }
        else if d < second_dist { second_dist = d; }
    }
    (second_dist - min_dist).sqrt().min(1.0)
}

pub fn domain_warp_fbm(x: f32, y: f32, seed: u64, octaves: u32, warp_strength: f32) -> f32 {
    let wx = fbm_noise(x + warp_strength * fbm_noise(x, y, seed, octaves/2), y + warp_strength * fbm_noise(x + 5.2, y + 1.3, seed, octaves/2), seed + 1, octaves);
    wx.clamp(0.0, 1.0)
}

pub fn fbm_noise(x: f32, y: f32, seed: u64, octaves: u32) -> f32 {
    let mut val = 0.0f32;
    let mut amp = 1.0f32;
    let mut freq = 1.0f32;
    let mut max_val = 0.0f32;
    let mut rng = SimpleRng::new(seed);
    for _ in 0..octaves {
        let offset_x = rng.next_f32() * 1000.0;
        let offset_y = rng.next_f32() * 1000.0;
        val += value_noise_2d(x * freq + offset_x, y * freq + offset_y, seed) * amp;
        max_val += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (val / max_val).clamp(0.0, 1.0)
}

pub fn value_noise_2d(x: f32, y: f32, seed: u64) -> f32 {
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let fx = fx * fx * (3.0 - 2.0 * fx);
    let fy = fy * fy * (3.0 - 2.0 * fy);
    let h00 = hash_2d(ix, iy, seed);
    let h10 = hash_2d(ix+1, iy, seed);
    let h01 = hash_2d(ix, iy+1, seed);
    let h11 = hash_2d(ix+1, iy+1, seed);
    let lo = h00 + (h10 - h00) * fx;
    let hi = h01 + (h11 - h01) * fx;
    lo + (hi - lo) * fy
}

pub fn hash_2d(x: i64, y: i64, seed: u64) -> f32 {
    let mut h = seed.wrapping_add(x as u64 * 1234567891).wrapping_add(y as u64 * 9876543211);
    h ^= h >> 17; h ^= h << 31; h ^= h >> 8;
    (h as f32) / (u64::MAX as f32)
}

pub fn ridge_noise(x: f32, y: f32, seed: u64, octaves: u32) -> f32 {
    let mut val = 0.0f32;
    let mut amp = 1.0f32;
    let mut freq = 1.0f32;
    let mut max_val = 0.0f32;
    let mut prev = 1.0f32;
    let mut rng = SimpleRng::new(seed);
    for _ in 0..octaves {
        let offset_x = rng.next_f32() * 1000.0;
        let offset_y = rng.next_f32() * 1000.0;
        let n = value_noise_2d(x * freq + offset_x, y * freq + offset_y, seed);
        let ridge = (1.0 - n.abs() * 2.0).abs();
        val += ridge * ridge * prev * amp;
        prev = ridge;
        max_val += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (val / max_val).clamp(0.0, 1.0)
}

pub fn billow_noise(x: f32, y: f32, seed: u64, octaves: u32) -> f32 {
    let mut val = 0.0f32;
    let mut amp = 1.0f32;
    let mut freq = 1.0f32;
    let mut max_val = 0.0f32;
    let mut rng = SimpleRng::new(seed);
    for _ in 0..octaves {
        let offset_x = rng.next_f32() * 1000.0;
        let offset_y = rng.next_f32() * 1000.0;
        let n = value_noise_2d(x * freq + offset_x, y * freq + offset_y, seed);
        val += (n * 2.0 - 1.0).abs() * amp;
        max_val += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (val / max_val).clamp(0.0, 1.0)
}

pub fn warp_noise(x: f32, y: f32, seed: u64) -> f32 {
    let qx = fbm_noise(x, y, seed, 4);
    let qy = fbm_noise(x + 5.2, y + 1.3, seed, 4);
    let rx = fbm_noise(x + 4.0*qx + 1.7, y + 4.0*qy + 9.2, seed+1, 4);
    let ry = fbm_noise(x + 4.0*qx + 8.3, y + 4.0*qy + 2.8, seed+2, 4);
    fbm_noise(x + 4.0*rx, y + 4.0*ry, seed+3, 4)
}

// Generate heightmap using layered noise
pub fn generate_heightmap_noise(w: usize, h: usize, config: &NoiseHeightmapConfig) -> Vec<f32> {
    let mut heightmap = vec![0.0f32; w * h];
    let scale = config.scale;
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32 * scale;
            let fy = y as f32 / h as f32 * scale;
            let v = match config.noise_type {
                NoiseType::FBM => fbm_noise(fx, fy, config.seed, config.octaves),
                NoiseType::Ridge => ridge_noise(fx, fy, config.seed, config.octaves),
                NoiseType::Billow => billow_noise(fx, fy, config.seed, config.octaves),
                NoiseType::Voronoi => voronoi_noise(fx, fy, config.seed, 32),
                NoiseType::DomainWarp => domain_warp_fbm(fx, fy, config.seed, config.octaves, config.warp_strength),
                NoiseType::Warp => warp_noise(fx, fy, config.seed),
                NoiseType::Hybrid => {
                    let a = fbm_noise(fx, fy, config.seed, config.octaves);
                    let b = ridge_noise(fx, fy, config.seed+1, config.octaves/2+1);
                    a * 0.6 + b * 0.4
                }
            };
            heightmap[y * w + x] = (v * config.height_scale + config.height_offset).clamp(0.0, 1.0);
        }
    }

    // Apply island mask if enabled
    if config.apply_island_mask {
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let max_dist = (cx * cx + cy * cy).sqrt();
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx*dx + dy*dy).sqrt() / max_dist;
                let mask = (1.0 - dist * dist).max(0.0);
                heightmap[y * w + x] *= mask.powf(config.island_falloff);
            }
        }
    }
    heightmap
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum NoiseType {
    FBM,
    Ridge,
    Billow,
    Voronoi,
    DomainWarp,
    Warp,
    Hybrid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoiseHeightmapConfig {
    pub noise_type: NoiseType,
    pub seed: u64,
    pub octaves: u32,
    pub scale: f32,
    pub height_scale: f32,
    pub height_offset: f32,
    pub warp_strength: f32,
    pub apply_island_mask: bool,
    pub island_falloff: f32,
}

impl Default for NoiseHeightmapConfig {
    fn default() -> Self {
        Self {
            noise_type: NoiseType::FBM,
            seed: 12345,
            octaves: 6,
            scale: 4.0,
            height_scale: 1.0,
            height_offset: 0.0,
            warp_strength: 0.3,
            apply_island_mask: false,
            island_falloff: 1.5,
        }
    }
}

pub fn show_noise_heightmap_editor(ui: &mut egui::Ui, cfg: &mut NoiseHeightmapConfig, heightmap: &mut Vec<f32>, w: usize, h: usize) {
    ui.heading("Noise Heightmap Generator");
    ui.horizontal(|ui| {
        ui.label("Noise Type:");
        for (nt, label) in [
            (NoiseType::FBM, "fBm"),
            (NoiseType::Ridge, "Ridge"),
            (NoiseType::Billow, "Billow"),
            (NoiseType::Voronoi, "Voronoi"),
            (NoiseType::DomainWarp, "Domain Warp"),
            (NoiseType::Warp, "Warp"),
            (NoiseType::Hybrid, "Hybrid"),
        ] {
            if ui.selectable_label(cfg.noise_type == nt, label).clicked() { cfg.noise_type = nt; }
        }
    });
    ui.add(egui::DragValue::new(&mut cfg.seed).prefix("Seed: "));
    ui.add(egui::Slider::new(&mut cfg.octaves, 1..=10).text("Octaves"));
    ui.add(egui::Slider::new(&mut cfg.scale, 0.5..=16.0).text("Scale"));
    ui.add(egui::Slider::new(&mut cfg.height_scale, 0.1..=2.0).text("Height Scale"));
    ui.add(egui::Slider::new(&mut cfg.height_offset, -0.5..=0.5).text("Height Offset"));
    if cfg.noise_type == NoiseType::DomainWarp {
        ui.add(egui::Slider::new(&mut cfg.warp_strength, 0.0..=2.0).text("Warp Strength"));
    }
    ui.checkbox(&mut cfg.apply_island_mask, "Island Mask");
    if cfg.apply_island_mask {
        ui.add(egui::Slider::new(&mut cfg.island_falloff, 0.5..=4.0).text("Falloff"));
    }
    if ui.button("Generate Heightmap").clicked() {
        *heightmap = generate_heightmap_noise(w, h, cfg);
    }
}

// =================================================================
// WORLD GEN TESTS
// =================================================================

#[cfg(test)]
mod world_gen_expansion_tests {
    use super::*;

    #[test]
    fn test_erosion_config_default() {
        let cfg = ErosionConfig::default();
        assert!(cfg.erosion_rate > 0.0);
        assert!(cfg.iterations > 0);
    }

    #[test]
    fn test_river_network_generation() {
        let mut rng = SimpleRng::new(42);
        let w = 32; let h = 32;
        let heightmap: Vec<f32> = (0..w*h).map(|i| fbm_noise(i as f32 / w as f32, (i / w) as f32 / h as f32, 0, 4)).collect();
        let mut net = RiverNetwork::new(w, h);
        net.generate_from_heightmap(&heightmap, &mut rng);
        assert_eq!(net.flow_map.len(), w * h);
    }

    #[test]
    fn test_tectonic_simulation() {
        let mut rng = SimpleRng::new(42);
        let cfg = TectonicSimConfig { grid_w: 16, grid_h: 16, plate_count: 4, ..Default::default() };
        let hmap = simulate_tectonics(&cfg, &mut rng);
        assert_eq!(hmap.len(), 256);
        assert!(hmap.iter().all(|&v| v >= 0.0 && v <= 1.0));
    }

    #[test]
    fn test_climate_simulation() {
        let cfg = ClimateSimConfig { grid_w: 16, grid_h: 16, ..Default::default() };
        let hmap = vec![0.5f32; 16 * 16];
        let ocean = vec![false; 16 * 16];
        let cells = simulate_climate(&cfg, &hmap, &ocean);
        assert_eq!(cells.len(), 256);
    }

    #[test]
    fn test_settlement_name_generation() {
        let mut rng = SimpleRng::new(99);
        let name = generate_settlement_name(&mut rng);
        assert!(!name.is_empty());
    }

    #[test]
    fn test_settlement_placement() {
        let mut rng = SimpleRng::new(42);
        let w = 32; let h = 32;
        let suit = vec![0.7f32; w * h];
        let cfg = SettlementPlacementConfig { max_settlements: 10, ..Default::default() };
        let settlements = place_settlements(&suit, w, h, &cfg, &mut rng);
        assert!(!settlements.is_empty());
    }

    #[test]
    fn test_climate_classification() {
        assert_eq!(classify_climate(28.0, 2000.0), ClimateZone::Tropical);
        assert_eq!(classify_climate(-15.0, 100.0), ClimateZone::Arctic);
        assert_eq!(classify_climate(15.0, 150.0), ClimateZone::Arid);
    }

    #[test]
    fn test_noise_functions() {
        let v = fbm_noise(0.5, 0.5, 42, 4);
        assert!(v >= 0.0 && v <= 1.0);
        let r = ridge_noise(0.5, 0.5, 42, 4);
        assert!(r >= 0.0 && r <= 1.0);
        let b = billow_noise(0.5, 0.5, 42, 4);
        assert!(b >= 0.0 && b <= 1.0);
    }

    #[test]
    fn test_underground_world_generation() {
        let cfg = UndergroundWorldConfig { layer_count: 2, layer_width: 32, layer_height: 32, ..Default::default() };
        let layers = generate_underground_world(&cfg);
        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn test_political_map_generation() {
        let cfg = PoliticalMapConfig { region_count: 4, seed: 42, ..Default::default() };
        let settlements: Vec<Settlement> = Vec::new();
        let mut rng = SimpleRng::new(42);
        let regions = generate_political_map(&cfg, &settlements, 32, 32, &mut rng);
        assert_eq!(regions.len(), 4);
    }

    #[test]
    fn test_weather_event_lifecycle() {
        let mut event = WeatherEvent::new(WeatherEventType::Storm, 0.5, 0.5);
        assert!(!event.is_expired());
        for _ in 0..100 { event.tick(); }
        assert!(event.is_expired());
    }

    #[test]
    fn test_flora_classification() {
        assert_eq!(FloraType::from_climate(28.0, 2000.0, 0.2, false), FloraType::TropicalRainforest);
        assert_eq!(FloraType::from_climate(-10.0, 100.0, 0.2, false), FloraType::Tundra);
        assert_eq!(FloraType::from_climate(25.0, 100.0, 0.2, false), FloraType::Desert);
    }
}
'''

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'a', encoding='utf-8') as f:
    f.write(code)

print(f"Done. world_gen size: {__import__('os').path.getsize(r'C:\proof-engine\editor\src\world_gen.rs')} bytes")
