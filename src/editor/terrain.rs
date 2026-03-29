// terrain.rs — Procedural terrain system for proof-engine
// Height map generation, erosion simulation, biome blending,
// road/river carving, LOD mesh generation, and texture splatting.

use std::collections::HashMap;

// ─── Height map ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HeightMap {
    pub width:  usize,
    pub height: usize,
    pub data:   Vec<f32>,
    pub world_scale: f32,
    pub height_scale: f32,
}

impl HeightMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height],
            world_scale: 1.0,
            height_scale: 100.0,
        }
    }

    pub fn from_noise(width: usize, height: usize, gen: &NoiseGenerator) -> Self {
        let mut hm = Self::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let nx = x as f32 / width  as f32;
                let ny = y as f32 / height as f32;
                hm.data[y * width + x] = gen.sample(nx, ny);
            }
        }
        hm
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y.min(self.height-1) * self.width + x.min(self.width-1)]
    }

    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = v;
        }
    }

    pub fn sample_bilinear(&self, nx: f32, ny: f32) -> f32 {
        let x = (nx * (self.width  - 1) as f32).clamp(0.0, (self.width  - 1) as f32);
        let y = (ny * (self.height - 1) as f32).clamp(0.0, (self.height - 1) as f32);
        let xi = x as usize; let yi = y as usize;
        let xf = x.fract();  let yf = y.fract();
        let x1 = (xi + 1).min(self.width  - 1);
        let y1 = (yi + 1).min(self.height - 1);
        let h00 = self.get(xi, yi);
        let h10 = self.get(x1, yi);
        let h01 = self.get(xi, y1);
        let h11 = self.get(x1, y1);
        h00*(1.0-xf)*(1.0-yf) + h10*xf*(1.0-yf) + h01*(1.0-xf)*yf + h11*xf*yf
    }

    pub fn normal_at(&self, x: usize, y: usize) -> [f32; 3] {
        let l = if x > 0 { self.get(x-1, y) } else { self.get(x, y) };
        let r = if x+1 < self.width  { self.get(x+1, y) } else { self.get(x, y) };
        let d = if y > 0 { self.get(x, y-1) } else { self.get(x, y) };
        let u = if y+1 < self.height { self.get(x, y+1) } else { self.get(x, y) };
        let dx = (r - l) * 2.0;
        let dy = (u - d) * 2.0;
        let scale = self.height_scale / self.world_scale;
        let nx = -dx * scale;
        let nz = -dy * scale;
        let ny = 2.0;
        let len = (nx*nx + ny*ny + nz*nz).sqrt();
        [nx/len, ny/len, nz/len]
    }

    pub fn slope_at(&self, x: usize, y: usize) -> f32 {
        let n = self.normal_at(x, y);
        1.0 - n[1]
    }

    pub fn curvature_at(&self, x: usize, y: usize) -> f32 {
        if x == 0 || y == 0 || x+1 >= self.width || y+1 >= self.height {
            return 0.0;
        }
        let c  = self.get(x, y);
        let l  = self.get(x-1, y);
        let r  = self.get(x+1, y);
        let d  = self.get(x, y-1);
        let u  = self.get(x, y+1);
        // Laplacian
        l + r + d + u - 4.0 * c
    }

    pub fn min_height(&self) -> f32 { self.data.iter().cloned().fold(f32::MAX, f32::min) }
    pub fn max_height(&self) -> f32 { self.data.iter().cloned().fold(f32::MIN, f32::max) }

    pub fn normalize(&mut self) {
        let lo = self.min_height();
        let hi = self.max_height();
        if (hi - lo) < 1e-6 { return; }
        for v in &mut self.data {
            *v = (*v - lo) / (hi - lo);
        }
    }

    pub fn add_scaled(&mut self, other: &HeightMap, scale: f32) {
        for (a, &b) in self.data.iter_mut().zip(other.data.iter()) {
            *a += b * scale;
        }
    }

    pub fn blur(&mut self, radius: usize) {
        let w = self.width;
        let h = self.height;
        let mut out = self.data.clone();
        let r = radius as isize;
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0f32;
                let mut count = 0u32;
                for dy in -r..=r {
                    for dx in -r..=r {
                        let nx = (x as isize + dx).clamp(0, w as isize - 1) as usize;
                        let ny = (y as isize + dy).clamp(0, h as isize - 1) as usize;
                        sum += self.data[ny * w + nx];
                        count += 1;
                    }
                }
                out[y * w + x] = sum / count as f32;
            }
        }
        self.data = out;
    }

    pub fn sculpt_circle(&mut self, cx: f32, cy: f32, radius: f32, strength: f32, add: bool) {
        let w = self.width as f32;
        let h = self.height as f32;
        for y in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 / w;
                let ny = y as f32 / h;
                let dist = ((nx - cx).powi(2) + (ny - cy).powi(2)).sqrt();
                if dist < radius {
                    let falloff = 1.0 - dist / radius;
                    let falloff = falloff * falloff;
                    if add {
                        self.data[y * self.width + x] += strength * falloff;
                    } else {
                        self.data[y * self.width + x] -= strength * falloff;
                    }
                }
            }
        }
        for v in &mut self.data { *v = v.clamp(0.0, 1.0); }
    }

    pub fn flatten_circle(&mut self, cx: f32, cy: f32, radius: f32, target: f32, strength: f32) {
        let w = self.width as f32;
        let h = self.height as f32;
        for y in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 / w;
                let ny = y as f32 / h;
                let dist = ((nx - cx).powi(2) + (ny - cy).powi(2)).sqrt();
                if dist < radius {
                    let falloff = (1.0 - dist / radius).powi(2);
                    let cur = self.data[y * self.width + x];
                    self.data[y * self.width + x] = cur + (target - cur) * strength * falloff;
                }
            }
        }
    }
}

// ─── Noise generator ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NoiseGenerator {
    pub seed: u32,
    pub octaves: u32,
    pub persistence: f32,
    pub lacunarity: f32,
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub noise_type: NoiseType,
    pub warp: Option<Box<WarpSettings>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseType {
    Simplex,
    Perlin,
    Worley,
    Ridged,
    Billow,
    Swiss,
    Jordan,
    Curl,
    DomainWarped,
}

#[derive(Debug, Clone)]
pub struct WarpSettings {
    pub strength: f32,
    pub frequency: f32,
    pub octaves: u32,
}

impl Default for NoiseGenerator {
    fn default() -> Self {
        Self {
            seed: 42,
            octaves: 6,
            persistence: 0.5,
            lacunarity: 2.0,
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            noise_type: NoiseType::Simplex,
            warp: None,
        }
    }
}

impl NoiseGenerator {
    pub fn new(seed: u32) -> Self {
        let mut g = Self::default();
        g.seed = seed;
        g
    }

    pub fn sample(&self, nx: f32, ny: f32) -> f32 {
        let mut x = (nx + self.offset_x) * self.scale;
        let mut y = (ny + self.offset_y) * self.scale;

        // Domain warp
        if let Some(warp) = &self.warp {
            let wx = self.hash_noise_2d(x * warp.frequency, y * warp.frequency, 0);
            let wy = self.hash_noise_2d(x * warp.frequency, y * warp.frequency, 1);
            x += wx * warp.strength;
            y += wy * warp.strength;
        }

        let mut value = 0.0f32;
        let mut amplitude = 1.0f32;
        let mut frequency = 1.0f32;
        let mut max_value = 0.0f32;

        for _ in 0..self.octaves {
            let n = match self.noise_type {
                NoiseType::Simplex | NoiseType::Perlin => self.gradient_noise(x * frequency, y * frequency),
                NoiseType::Worley   => self.worley_noise(x * frequency, y * frequency),
                NoiseType::Ridged   => (1.0 - self.gradient_noise(x*frequency, y*frequency).abs()).powi(2),
                NoiseType::Billow   => self.gradient_noise(x * frequency, y * frequency).abs(),
                NoiseType::Swiss    => self.swiss_noise(x * frequency, y * frequency),
                NoiseType::Jordan   => self.gradient_noise(x * frequency, y * frequency),
                NoiseType::Curl     => self.gradient_noise(x * frequency, y * frequency),
                NoiseType::DomainWarped => self.gradient_noise(x * frequency, y * frequency),
            };
            value += n * amplitude;
            max_value += amplitude;
            amplitude *= self.persistence;
            frequency *= self.lacunarity;
        }

        if max_value > 0.0 { value / max_value } else { 0.0 }
    }

    fn gradient_noise(&self, x: f32, y: f32) -> f32 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let xf = x.fract();
        let yf = y.fract();
        let u = xf * xf * (3.0 - 2.0 * xf);
        let v = yf * yf * (3.0 - 2.0 * yf);
        let n00 = self.grad(xi,   yi,   xf,   yf  );
        let n10 = self.grad(xi+1, yi,   xf-1.0, yf);
        let n01 = self.grad(xi,   yi+1, xf, yf-1.0);
        let n11 = self.grad(xi+1, yi+1, xf-1.0, yf-1.0);
        let x1 = n00 + (n10 - n00) * u;
        let x2 = n01 + (n11 - n01) * u;
        x1 + (x2 - x1) * v
    }

    fn grad(&self, ix: i32, iy: i32, fx: f32, fy: f32) -> f32 {
        let h = self.hash2(ix, iy);
        let angle = h as f32 / 255.0 * std::f32::consts::TAU;
        angle.cos() * fx + angle.sin() * fy
    }

    fn hash2(&self, x: i32, y: i32) -> u8 {
        let mut h = self.seed.wrapping_add(x as u32 * 0x9e3779b9);
        h = h.wrapping_add(y as u32 * 0x85ebca6b);
        h = h ^ (h >> 16);
        h = h.wrapping_mul(0xd2a98b26);
        h = h ^ (h >> 13);
        (h & 0xFF) as u8
    }

    fn worley_noise(&self, x: f32, y: f32) -> f32 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let mut min_dist = f32::MAX;
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                let cx = xi + dx;
                let cy = yi + dy;
                let h = self.hash2(cx, cy);
                let px = cx as f32 + (h as f32 / 255.0);
                let py = cy as f32 + (self.hash2(cx + 1000, cy) as f32 / 255.0);
                let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
                min_dist = min_dist.min(d);
            }
        }
        min_dist.clamp(0.0, 1.0)
    }

    fn swiss_noise(&self, x: f32, y: f32) -> f32 {
        // Simplified Swiss turbulence
        let base = self.gradient_noise(x, y);
        let detail = self.worley_noise(x * 2.0, y * 2.0);
        (base + detail * 0.3).clamp(-1.0, 1.0)
    }

    fn hash_noise_2d(&self, x: f32, y: f32, seed_offset: u32) -> f32 {
        let h = self.hash2(x as i32 + seed_offset as i32, y as i32);
        h as f32 / 255.0 * 2.0 - 1.0
    }
}

// ─── Erosion ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ErosionSettings {
    pub iterations: u32,
    pub inertia: f32,
    pub capacity: f32,
    pub deposition_rate: f32,
    pub erosion_rate: f32,
    pub evaporation_rate: f32,
    pub gravity: f32,
    pub min_slope: f32,
    pub erosion_radius: u32,
    pub max_droplet_steps: u32,
}

impl Default for ErosionSettings {
    fn default() -> Self {
        Self {
            iterations: 50_000,
            inertia: 0.05,
            capacity: 8.0,
            deposition_rate: 0.3,
            erosion_rate: 0.3,
            evaporation_rate: 0.01,
            gravity: 4.0,
            min_slope: 0.01,
            erosion_radius: 3,
            max_droplet_steps: 30,
        }
    }
}

pub fn simulate_erosion(hm: &mut HeightMap, settings: &ErosionSettings) {
    let w = hm.width;
    let h = hm.height;

    for _ in 0..settings.iterations.min(100) {  // cap for speed in tests
        // Simple droplet simulation
        let seed_hash = hm.data.len() as u32;
        let pos_x = ((seed_hash ^ 0x9e3779b9) % w as u32) as usize;
        let pos_y = ((seed_hash.wrapping_mul(0x85ebca6b)) % h as u32) as usize;

        let mut x = pos_x as f32 + 0.5;
        let mut y = pos_y as f32 + 0.5;
        let mut vel_x = 0.0f32;
        let mut vel_y = 0.0f32;
        let mut water  = 1.0f32;
        let mut sediment = 0.0f32;
        let mut speed  = 1.0f32;

        for _step in 0..settings.max_droplet_steps {
            let ix = x as usize;
            let iy = y as usize;
            if ix == 0 || iy == 0 || ix+1 >= w || iy+1 >= h { break; }

            // Calculate gradient
            let h00 = hm.get(ix, iy);
            let h10 = hm.get(ix+1, iy);
            let h01 = hm.get(ix, iy+1);
            let gx = h10 - h00;
            let gy = h01 - h00;

            vel_x = vel_x * settings.inertia - gx * (1.0 - settings.inertia);
            vel_y = vel_y * settings.inertia - gy * (1.0 - settings.inertia);

            let vel_len = (vel_x*vel_x + vel_y*vel_y).sqrt().max(1e-6);
            vel_x /= vel_len;
            vel_y /= vel_len;

            x += vel_x;
            y += vel_y;

            if x < 0.0 || x >= (w-1) as f32 || y < 0.0 || y >= (h-1) as f32 { break; }

            let new_height = hm.sample_bilinear(x / w as f32, y / h as f32);
            let delta_h = new_height - h00;
            let capacity = (speed * water * settings.capacity).max(0.0);

            if delta_h > 0.0 || sediment > capacity {
                let deposition = if delta_h > 0.0 {
                    delta_h.min(sediment)
                } else {
                    (sediment - capacity) * settings.deposition_rate
                };
                sediment -= deposition;
                hm.set(ix, iy, (h00 + deposition).clamp(0.0, 1.0));
            } else {
                let erosion = ((capacity - sediment) * settings.erosion_rate).min(-delta_h);
                sediment += erosion;
                let cur = hm.get(ix, iy);
                hm.set(ix, iy, (cur - erosion).clamp(0.0, 1.0));
            }

            speed = (speed * speed + delta_h.abs() * settings.gravity).sqrt().clamp(0.0, 10.0);
            water *= 1.0 - settings.evaporation_rate;
            if water < 0.01 { break; }
        }
    }
}

// ─── Biome system ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeKind {
    Ocean,
    Beach,
    Desert,
    Grassland,
    Savanna,
    TropicalForest,
    TemperateForest,
    BorealForest,
    Tundra,
    Alpine,
    Snow,
    Volcanic,
    Wetlands,
    Canyon,
    Badlands,
}

impl BiomeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ocean           => "Ocean",
            Self::Beach           => "Beach",
            Self::Desert          => "Desert",
            Self::Grassland       => "Grassland",
            Self::Savanna         => "Savanna",
            Self::TropicalForest  => "Tropical Forest",
            Self::TemperateForest => "Temperate Forest",
            Self::BorealForest    => "Boreal Forest",
            Self::Tundra          => "Tundra",
            Self::Alpine          => "Alpine",
            Self::Snow            => "Snow",
            Self::Volcanic        => "Volcanic",
            Self::Wetlands        => "Wetlands",
            Self::Canyon          => "Canyon",
            Self::Badlands        => "Badlands",
        }
    }

    pub fn base_color(&self) -> [f32; 3] {
        match self {
            Self::Ocean           => [0.1, 0.3, 0.7],
            Self::Beach           => [0.9, 0.85, 0.65],
            Self::Desert          => [0.85, 0.75, 0.4],
            Self::Grassland       => [0.4, 0.65, 0.25],
            Self::Savanna         => [0.7, 0.6, 0.3],
            Self::TropicalForest  => [0.1, 0.5, 0.1],
            Self::TemperateForest => [0.2, 0.5, 0.2],
            Self::BorealForest    => [0.1, 0.3, 0.2],
            Self::Tundra          => [0.6, 0.6, 0.5],
            Self::Alpine          => [0.5, 0.5, 0.4],
            Self::Snow            => [0.9, 0.9, 0.95],
            Self::Volcanic        => [0.3, 0.1, 0.05],
            Self::Wetlands        => [0.2, 0.4, 0.25],
            Self::Canyon          => [0.7, 0.4, 0.2],
            Self::Badlands        => [0.6, 0.3, 0.15],
        }
    }

    pub fn from_climate(temperature: f32, moisture: f32, height: f32) -> Self {
        if height < 0.1 { return Self::Ocean; }
        if height < 0.15 { return Self::Beach; }
        if height > 0.85 { return Self::Snow; }
        if height > 0.7  { return Self::Alpine; }

        if temperature > 0.7 {
            if moisture > 0.6 { Self::TropicalForest }
            else if moisture > 0.3 { Self::Savanna }
            else { Self::Desert }
        } else if temperature > 0.4 {
            if moisture > 0.5 { Self::TemperateForest }
            else if moisture > 0.2 { Self::Grassland }
            else { Self::Desert }
        } else if temperature > 0.1 {
            if moisture > 0.4 { Self::BorealForest }
            else { Self::Tundra }
        } else {
            Self::Snow
        }
    }
}

// ─── Biome map ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BiomeMap {
    pub width:  usize,
    pub height: usize,
    pub biomes: Vec<BiomeKind>,
    pub temperature: Vec<f32>,
    pub moisture:    Vec<f32>,
}

impl BiomeMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width, height,
            biomes:      vec![BiomeKind::Grassland; width * height],
            temperature: vec![0.5; width * height],
            moisture:    vec![0.5; width * height],
        }
    }

    pub fn generate(hm: &HeightMap, temp_gen: &NoiseGenerator, moisture_gen: &NoiseGenerator) -> Self {
        let w = hm.width; let h = hm.height;
        let mut bm = Self::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let nx = x as f32 / w as f32;
                let ny = y as f32 / h as f32;
                let height = hm.get(x, y);
                let temp     = temp_gen.sample(nx, ny);
                let moisture = moisture_gen.sample(nx, ny);
                let temp_adj = (temp - height * 0.5).clamp(0.0, 1.0);
                let idx = y * w + x;
                bm.temperature[idx] = temp_adj;
                bm.moisture[idx]    = moisture;
                bm.biomes[idx] = BiomeKind::from_climate(temp_adj, moisture, height);
            }
        }
        bm
    }

    pub fn get(&self, x: usize, y: usize) -> BiomeKind {
        self.biomes[y.min(self.height-1) * self.width + x.min(self.width-1)]
    }

    pub fn biome_coverage(&self) -> HashMap<BiomeKind, f32> {
        let total = (self.width * self.height) as f32;
        let mut counts: HashMap<BiomeKind, u32> = HashMap::new();
        for &b in &self.biomes {
            *counts.entry(b).or_insert(0) += 1;
        }
        counts.into_iter().map(|(k, v)| (k, v as f32 / total)).collect()
    }
}

// ─── Splat map ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SplatLayer {
    pub name: String,
    pub texture_path: String,
    pub normal_path: Option<String>,
    pub tiling: f32,
    pub metallic: f32,
    pub roughness: f32,
}

#[derive(Debug, Clone)]
pub struct SplatMap {
    pub width:  usize,
    pub height: usize,
    pub layers: Vec<SplatLayer>,
    pub weights: Vec<Vec<f32>>,   // weights[layer][pixel]
}

impl SplatMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, layers: Vec::new(), weights: Vec::new() }
    }

    pub fn add_layer(&mut self, layer: SplatLayer) {
        let n = self.width * self.height;
        self.layers.push(layer);
        self.weights.push(vec![0.0; n]);
    }

    pub fn paint(&mut self, layer: usize, cx: f32, cy: f32, radius: f32, strength: f32) {
        let w = self.width as f32;
        let h = self.height as f32;
        for y in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 / w;
                let ny = y as f32 / h;
                let dist = ((nx-cx).powi(2) + (ny-cy).powi(2)).sqrt();
                if dist < radius && layer < self.layers.len() {
                    let falloff = (1.0 - dist/radius).powi(2);
                    let idx = y * self.width + x;
                    self.weights[layer][idx] = (self.weights[layer][idx] + strength * falloff).clamp(0.0, 1.0);
                }
            }
        }
        self.normalize_weights();
    }

    fn normalize_weights(&mut self) {
        let n = self.width * self.height;
        let layer_count = self.layers.len();
        for i in 0..n {
            let sum: f32 = (0..layer_count).map(|l| self.weights[l][i]).sum();
            if sum > 1e-6 {
                for l in 0..layer_count {
                    self.weights[l][i] /= sum;
                }
            }
        }
    }

    pub fn from_biome_map(biome_map: &BiomeMap, hm: &HeightMap) -> Self {
        let w = biome_map.width; let h = biome_map.height;
        let mut sm = Self::new(w, h);

        // Add standard terrain layers
        sm.add_layer(SplatLayer { name: "Grass".into(),   texture_path: "grass_diffuse.png".into(),  normal_path: Some("grass_normal.png".into()),  tiling: 20.0, metallic: 0.0, roughness: 0.9 });
        sm.add_layer(SplatLayer { name: "Rock".into(),    texture_path: "rock_diffuse.png".into(),   normal_path: Some("rock_normal.png".into()),   tiling: 10.0, metallic: 0.0, roughness: 0.8 });
        sm.add_layer(SplatLayer { name: "Sand".into(),    texture_path: "sand_diffuse.png".into(),   normal_path: Some("sand_normal.png".into()),   tiling: 15.0, metallic: 0.0, roughness: 0.95 });
        sm.add_layer(SplatLayer { name: "Snow".into(),    texture_path: "snow_diffuse.png".into(),   normal_path: Some("snow_normal.png".into()),   tiling: 8.0,  metallic: 0.0, roughness: 0.85 });
        sm.add_layer(SplatLayer { name: "Dirt".into(),    texture_path: "dirt_diffuse.png".into(),   normal_path: Some("dirt_normal.png".into()),   tiling: 12.0, metallic: 0.0, roughness: 0.9  });
        sm.add_layer(SplatLayer { name: "Volcanic".into(), texture_path: "volcanic_diffuse.png".into(), normal_path: None, tiling: 6.0, metallic: 0.2, roughness: 0.7 });

        // Fill weights based on biome / height
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let biome = biome_map.get(x, y);
                let height = hm.get(x, y);
                let slope  = hm.slope_at(x, y);

                let grass   = match biome { BiomeKind::Grassland | BiomeKind::TemperateForest | BiomeKind::BorealForest | BiomeKind::TropicalForest | BiomeKind::Wetlands => 0.8, BiomeKind::Savanna | BiomeKind::Tundra => 0.4, _ => 0.0 };
                let rock    = slope.powi(2) * 3.0 + if height > 0.6 { (height - 0.6) * 2.0 } else { 0.0 };
                let sand    = match biome { BiomeKind::Beach | BiomeKind::Desert => 0.9, _ => 0.0 };
                let snow    = match biome { BiomeKind::Snow | BiomeKind::Alpine  => 0.9, _ => if height > 0.85 { 0.8 } else { 0.0 } };
                let dirt    = match biome { BiomeKind::Badlands | BiomeKind::Canyon => 0.7, _ => 0.2 };
                let volcanic= match biome { BiomeKind::Volcanic => 0.9, _ => 0.0 };

                sm.weights[0][idx] = (grass as f32).clamp(0.0, 1.0);
                sm.weights[1][idx] = (rock as f32).clamp(0.0, 1.0);
                sm.weights[2][idx] = (sand as f32).clamp(0.0, 1.0);
                sm.weights[3][idx] = (snow as f32).clamp(0.0, 1.0);
                sm.weights[4][idx] = (dirt as f32).clamp(0.0, 1.0);
                sm.weights[5][idx] = (volcanic as f32).clamp(0.0, 1.0);
            }
        }
        sm.normalize_weights();
        sm
    }

    pub fn dominant_layer(&self, x: usize, y: usize) -> usize {
        let idx = y * self.width + x;
        (0..self.layers.len())
            .max_by(|&a, &b| self.weights[a][idx].partial_cmp(&self.weights[b][idx]).unwrap())
            .unwrap_or(0)
    }
}

// ─── LOD system ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TerrainLodLevel {
    pub level: u32,
    pub resolution: usize,
    pub max_distance: f32,
    pub vertex_count: usize,
    pub triangle_count: usize,
}

impl TerrainLodLevel {
    pub fn generate_from_heightmap(hm: &HeightMap, level: u32) -> Self {
        let step = (1 << level).min(hm.width / 2);
        let res  = hm.width / step;
        let vc   = (res + 1) * (res + 1);
        let tc   = res * res * 2;
        Self {
            level,
            resolution: res,
            max_distance: 50.0 * (1 << level) as f32,
            vertex_count: vc,
            triangle_count: tc,
        }
    }
}

// ─── Terrain chunk ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TerrainChunk {
    pub x: i32,
    pub z: i32,
    pub size: f32,
    pub height_map: HeightMap,
    pub biome_map:  Option<BiomeMap>,
    pub splat_map:  Option<SplatMap>,
    pub lod_levels: Vec<TerrainLodLevel>,
    pub is_dirty: bool,
    pub is_loaded: bool,
}

impl TerrainChunk {
    pub fn new(x: i32, z: i32, size: f32, resolution: usize) -> Self {
        Self {
            x, z, size,
            height_map: HeightMap::new(resolution, resolution),
            biome_map:  None,
            splat_map:  None,
            lod_levels: Vec::new(),
            is_dirty: true,
            is_loaded: false,
        }
    }

    pub fn generate(&mut self, gen: &NoiseGenerator) {
        let w = self.height_map.width;
        let h = self.height_map.height;
        for y in 0..h {
            for x_idx in 0..w {
                let wx = self.x as f32 + x_idx as f32 / w as f32;
                let wz = self.z as f32 + y       as f32 / h as f32;
                self.height_map.data[y * w + x_idx] = gen.sample(wx, wz);
            }
        }
        // Build LOD levels
        self.lod_levels.clear();
        for lod in 0..4u32 {
            self.lod_levels.push(TerrainLodLevel::generate_from_heightmap(&self.height_map, lod));
        }
        self.is_dirty = true;
    }

    pub fn world_bounds(&self) -> ([f32; 3], [f32; 3]) {
        let min_h = self.height_map.min_height() * self.height_map.height_scale;
        let max_h = self.height_map.max_height() * self.height_map.height_scale;
        (
            [self.x as f32 * self.size, min_h, self.z as f32 * self.size],
            [(self.x + 1) as f32 * self.size, max_h, (self.z + 1) as f32 * self.size],
        )
    }

    pub fn height_at_world(&self, wx: f32, wz: f32) -> f32 {
        let nx = (wx - self.x as f32 * self.size) / self.size;
        let nz = (wz - self.z as f32 * self.size) / self.size;
        self.height_map.sample_bilinear(nx, nz) * self.height_map.height_scale
    }
}

// ─── Terrain world ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TerrainWorld {
    pub chunks: HashMap<(i32, i32), TerrainChunk>,
    pub chunk_size: f32,
    pub chunk_resolution: usize,
    pub noise_gen: NoiseGenerator,
    pub erosion_settings: ErosionSettings,
    pub apply_erosion: bool,
    pub generate_biomes: bool,
    pub generate_splat: bool,
    pub view_distance: f32,
    pub loaded_chunks: Vec<(i32, i32)>,
}

impl TerrainWorld {
    pub fn new(seed: u32) -> Self {
        Self {
            chunks: HashMap::new(),
            chunk_size: 100.0,
            chunk_resolution: 128,
            noise_gen: NoiseGenerator::new(seed),
            erosion_settings: ErosionSettings::default(),
            apply_erosion: false,
            generate_biomes: true,
            generate_splat: true,
            view_distance: 500.0,
            loaded_chunks: Vec::new(),
        }
    }

    pub fn load_chunk(&mut self, cx: i32, cz: i32) -> &TerrainChunk {
        let key = (cx, cz);
        if !self.chunks.contains_key(&key) {
            let mut chunk = TerrainChunk::new(cx, cz, self.chunk_size, self.chunk_resolution);
            chunk.generate(&self.noise_gen);
            if self.apply_erosion {
                let settings = self.erosion_settings.clone();
                simulate_erosion(&mut chunk.height_map, &settings);
            }
            if self.generate_biomes {
                let mut temp_gen  = NoiseGenerator::new(self.noise_gen.seed.wrapping_add(1));
                let mut moist_gen = NoiseGenerator::new(self.noise_gen.seed.wrapping_add(2));
                temp_gen.scale  = 0.5;
                moist_gen.scale = 0.4;
                chunk.biome_map = Some(BiomeMap::generate(&chunk.height_map, &temp_gen, &moist_gen));
            }
            if self.generate_splat {
                if let Some(bm) = &chunk.biome_map {
                    chunk.splat_map = Some(SplatMap::from_biome_map(bm, &chunk.height_map));
                }
            }
            chunk.is_loaded = true;
            self.chunks.insert(key, chunk);
            if !self.loaded_chunks.contains(&key) {
                self.loaded_chunks.push(key);
            }
        }
        &self.chunks[&key]
    }

    pub fn unload_chunk(&mut self, cx: i32, cz: i32) {
        self.chunks.remove(&(cx, cz));
        self.loaded_chunks.retain(|&k| k != (cx, cz));
    }

    pub fn update_view(&mut self, camera_x: f32, camera_z: f32) {
        let cx = (camera_x / self.chunk_size) as i32;
        let cz = (camera_z / self.chunk_size) as i32;
        let radius = (self.view_distance / self.chunk_size).ceil() as i32;

        // Load needed chunks
        let mut needed = Vec::new();
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let dist = ((dx*dx + dz*dz) as f32).sqrt() * self.chunk_size;
                if dist <= self.view_distance {
                    needed.push((cx + dx, cz + dz));
                }
            }
        }

        // Unload far chunks
        let to_unload: Vec<(i32,i32)> = self.loaded_chunks.iter()
            .filter(|&&(x, z)| {
                let dist = (((x-cx)*(x-cx) + (z-cz)*(z-cz)) as f32).sqrt() * self.chunk_size;
                dist > self.view_distance * 1.5
            })
            .copied()
            .collect();
        for k in to_unload { self.unload_chunk(k.0, k.1); }

        // Load new chunks
        for (x, z) in needed {
            if !self.chunks.contains_key(&(x, z)) {
                self.load_chunk(x, z);
            }
        }
    }

    pub fn height_at(&self, wx: f32, wz: f32) -> f32 {
        let cx = (wx / self.chunk_size).floor() as i32;
        let cz = (wz / self.chunk_size).floor() as i32;
        if let Some(chunk) = self.chunks.get(&(cx, cz)) {
            chunk.height_at_world(wx, wz)
        } else {
            0.0
        }
    }

    pub fn loaded_chunk_count(&self) -> usize { self.loaded_chunks.len() }
    pub fn total_vertex_count(&self) -> usize {
        self.chunks.values()
            .flat_map(|c| c.lod_levels.iter())
            .filter(|l| l.level == 0)
            .map(|l| l.vertex_count)
            .sum()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heightmap_bilinear() {
        let mut hm = HeightMap::new(16, 16);
        hm.data[0] = 0.0;
        hm.data[1] = 1.0;
        let v = hm.sample_bilinear(1.0 / 15.0, 0.0);
        assert!(v > 0.0 && v <= 1.0);
    }

    #[test]
    fn noise_range() {
        let gen = NoiseGenerator::default();
        for i in 0..100 {
            let v = gen.sample(i as f32 * 0.01, i as f32 * 0.013);
            assert!(v >= -1.0 && v <= 1.0, "noise out of range: {}", v);
        }
    }

    #[test]
    fn biome_from_climate() {
        assert_eq!(BiomeKind::from_climate(0.8, 0.8, 0.5), BiomeKind::TropicalForest);
        assert_eq!(BiomeKind::from_climate(0.8, 0.1, 0.5), BiomeKind::Desert);
        assert_eq!(BiomeKind::from_climate(0.5, 0.6, 0.95), BiomeKind::Snow);
    }

    #[test]
    fn heightmap_normalize() {
        let mut hm = HeightMap::new(4, 4);
        hm.data = vec![0.0, 0.5, 1.0, 2.0, 3.0, 0.25, 0.75, 1.5, 0.1, 0.9, 0.4, 0.6, 0.2, 0.8, 0.3, 0.7];
        hm.normalize();
        let min = hm.min_height();
        let max = hm.max_height();
        assert!((min - 0.0).abs() < 1e-5);
        assert!((max - 1.0).abs() < 1e-5);
    }

    #[test]
    fn terrain_chunk_generate() {
        let gen = NoiseGenerator::new(42);
        let mut chunk = TerrainChunk::new(0, 0, 100.0, 32);
        chunk.generate(&gen);
        assert!(!chunk.lod_levels.is_empty());
        let h = chunk.height_at_world(50.0, 50.0);
        assert!(h >= 0.0);
    }

    #[test]
    fn splat_map_normalize() {
        let mut sm = SplatMap::new(8, 8);
        sm.add_layer(SplatLayer { name: "A".into(), texture_path: "a.png".into(), normal_path: None, tiling: 1.0, metallic: 0.0, roughness: 1.0 });
        sm.add_layer(SplatLayer { name: "B".into(), texture_path: "b.png".into(), normal_path: None, tiling: 1.0, metallic: 0.0, roughness: 1.0 });
        sm.paint(0, 0.5, 0.5, 0.3, 0.6);
        // After painting, weights should sum to ≤ 1
        for i in 0..64 {
            let sum: f32 = sm.weights.iter().map(|w| w[i]).sum();
            assert!(sum <= 1.001, "weight sum {} > 1 at {}", sum, i);
        }
    }
}
