#!/usr/bin/env python
import os

code = r'''
impl TileAnimation {
    pub fn new(tile_id: u32, frames: Vec<u32>, frame_duration_ms: u32) -> Self {
        let n = frames.len();
        Self { tile_id, frames, frame_durations_ms: vec![frame_duration_ms; n], current_frame: 0, elapsed_ms: 0, loop_animation: true }
    }
    pub fn update(&mut self, dt_ms: u32) { self.update_ms(dt_ms); }
    pub fn update_ms(&mut self, dt_ms: u32) {
        self.elapsed_ms += dt_ms;
        if !self.frame_durations_ms.is_empty() {
            while self.elapsed_ms >= self.frame_durations_ms[self.current_frame] {
                self.elapsed_ms -= self.frame_durations_ms[self.current_frame];
                self.current_frame = (self.current_frame + 1) % self.frames.len().max(1);
            }
        }
    }
    pub fn reset(&mut self) { self.current_frame = 0; self.elapsed_ms = 0; }
    pub fn current_tile_id(&self) -> u32 { self.frames.get(self.current_frame).copied().unwrap_or(self.tile_id) }
    pub fn total_duration_ms(&self) -> u32 { self.frame_durations_ms.iter().sum() }
}

impl TileAnimationManager {
    pub fn new() -> Self { Self { animations: HashMap::new() } }
    pub fn standard_water_animation() -> TileAnimation { TileAnimation::new(0, vec![0,1,2,3], 200) }
    pub fn register(&mut self, anim: TileAnimation) { self.animations.insert(anim.tile_id, anim); }
    pub fn update_all(&mut self, dt_ms: u32) { for a in self.animations.values_mut() { a.update_ms(dt_ms); } }
    pub fn resolve_tile(&self, tile_id: u32) -> u32 { self.animations.get(&tile_id).map(|a| a.current_tile_id()).unwrap_or(tile_id) }
}

impl TileObject {
    pub fn new(id: u32, object_type: &str, position: (i32, i32)) -> Self { Self { id, object_type: object_type.to_string(), position, size: (1,1), rotation: 0, flip_x: false, flip_y: false, layer: 0, properties: HashMap::new(), collidable: true, sprite_id: 0 } }
    pub fn set_property(&mut self, k: &str, v: &str) { self.properties.insert(k.to_string(), v.to_string()); }
    pub fn get_property(&self, k: &str) -> Option<&String> { self.properties.get(k) }
}

impl ObjectLayer {
    pub fn new(id: u32, name: &str) -> Self { Self { id, name: name.to_string(), objects: Vec::new(), visible: true, locked: false } }
    pub fn add_object(&mut self, o: TileObject) { self.objects.push(o); }
    pub fn object_at(&self, x: i32, y: i32) -> Option<&TileObject> { self.objects.iter().find(|o| o.position == (x, y)) }
    pub fn find_overlaps(&self) -> Vec<(u32, u32)> { let mut r = Vec::new(); for i in 0..self.objects.len() { for j in i+1..self.objects.len() { if self.objects[i].position == self.objects[j].position { r.push((self.objects[i].id, self.objects[j].id)); } } } r }
}

impl TmxExporter {
    pub fn new() -> Self { Self { version: "1.0".to_string() } }
    pub fn export(&self, _map: &TileMap) -> String { String::new() }
    pub fn export_map(&self, _map: &TileMap, _name: &str) -> String { String::new() }
}

impl CollisionMap {
    pub const SOLID: u8 = 1;
    pub fn new(w: u32, h: u32) -> Self { Self { width: w, height: h, cells: vec![0u8; (w*h) as usize] } }
    pub fn set(&mut self, x: u32, y: u32, v: u8) { if let Some(c) = self.cells.get_mut((y*self.width+x) as usize) { *c = v; } }
    pub fn get_cell(&self, x: u32, y: u32) -> u8 { self.cells.get((y*self.width+x) as usize).copied().unwrap_or(0) }
    pub fn is_solid(&self, x: u32, y: u32) -> bool { self.get_cell(x, y) == Self::SOLID }
    pub fn is_passable(&self, x: u32, y: u32) -> bool { !self.is_solid(x, y) }
    pub fn passable_neighbors(&self, x: u32, y: u32) -> Vec<(u32, u32)> { let mut n = Vec::new(); if x > 0 && self.is_passable(x-1,y) { n.push((x-1,y)); } if y > 0 && self.is_passable(x,y-1) { n.push((x,y-1)); } if x+1 < self.width && self.is_passable(x+1,y) { n.push((x+1,y)); } if y+1 < self.height && self.is_passable(x,y+1) { n.push((x,y+1)); } n }
    pub fn set_flag(&mut self, x: usize, y: usize, solid: bool) { if x < self.width as usize && y < self.height as usize { self.cells[y * self.width as usize + x] = solid as u8; } }
}

impl SpawnPoint {
    pub fn new(id: u32, position: (i32, i32), spawn_type: &str) -> Self { Self { id, position, spawn_type: spawn_type.to_string(), max_concurrent: 1, current_count: 0, enabled: true, respawn_delay_secs: 30.0 } }
}

impl SpawnManager {
    pub fn new() -> Self { Self { spawn_points: Vec::new(), global_spawn_enabled: true } }
    pub fn add(&mut self, sp: SpawnPoint) { self.spawn_points.push(sp); }
    pub fn active_count(&self) -> u32 { self.spawn_points.iter().map(|s| s.current_count).sum() }
    pub fn add_spawn_point(&mut self, sp: SpawnPoint) { self.add(sp); }
    pub fn available_spawns_for_type(&self, t: &str) -> Vec<&SpawnPoint> { self.spawn_points.iter().filter(|s| s.spawn_type == t && s.enabled && s.current_count < s.max_concurrent).collect() }
    pub fn total_capacity(&self) -> u32 { self.spawn_points.iter().map(|s| s.max_concurrent).sum() }
}

impl WarpNetwork {
    pub fn new() -> Self { Self { warps: Vec::new() } }
    pub fn add(&mut self, w: WarpPoint) { self.warps.push(w); }
    pub fn unlocked_count(&self) -> usize { self.warps.iter().filter(|w| w.unlocked).count() }
    pub fn nearest_unlocked(&self, x: i32, y: i32) -> Option<&WarpPoint> { self.warps.iter().filter(|w| w.unlocked).min_by_key(|w| { let dx = w.position.0 - x; let dy = w.position.1 - y; dx*dx + dy*dy }) }
    pub fn unlock_all(&mut self) { for w in &mut self.warps { w.unlocked = true; } }
}

impl SoundZone {
    pub fn new(id: u32, polygon: Vec<(i32, i32)>, track: &str) -> Self { Self { id, polygon, ambient_track: track.to_string(), volume: 1.0, fade_distance: 3.0 } }
    pub fn contains_point(&self, x: i32, y: i32) -> bool { if self.polygon.len() < 2 { return false; } let xs: Vec<i32> = self.polygon.iter().map(|p| p.0).collect(); let ys: Vec<i32> = self.polygon.iter().map(|p| p.1).collect(); let min_x = *xs.iter().min().unwrap_or(&0); let max_x = *xs.iter().max().unwrap_or(&0); let min_y = *ys.iter().min().unwrap_or(&0); let max_y = *ys.iter().max().unwrap_or(&0); x >= min_x && x <= max_x && y >= min_y && y <= max_y }
}

impl SoundZoneManager {
    pub fn new() -> Self { Self { zones: Vec::new(), current_zone: None, player_position: (0, 0) } }
    pub fn add_zone(&mut self, z: SoundZone) { self.zones.push(z); }
    pub fn update_player_position(&mut self, x: i32, y: i32) { self.player_position = (x, y); self.current_zone = self.zones.iter().find(|z| z.contains_point(x, y)).map(|z| z.id); }
    pub fn current_volume(&self) -> f32 { self.zones.iter().find(|z| Some(z.id) == self.current_zone).map(|z| z.volume).unwrap_or(0.0) }
}

impl AoeZone {
    pub fn fire_pillar(id: u32, x: i32, y: i32, radius: f32, damage: f32) -> Self { Self { id, center: (x, y), radius, damage_per_tick: damage, zone_type: String::from("fire"), duration_ticks: 10, elapsed_ticks: 0, active: true } }
    pub fn frost_zone(id: u32, x: i32, y: i32, radius: f32, damage: f32) -> Self { Self { id, center: (x, y), radius, damage_per_tick: damage, zone_type: String::from("frost"), duration_ticks: 10, elapsed_ticks: 0, active: true } }
    pub fn contains(&self, x: i32, y: i32) -> bool { let dx = self.center.0 - x; let dy = self.center.1 - y; ((dx*dx + dy*dy) as f32).sqrt() <= self.radius }
}

impl AoeManager {
    pub fn new() -> Self { Self { zones: Vec::new() } }
    pub fn add_zone(&mut self, z: AoeZone) { self.zones.push(z); }
    pub fn zones_at(&self, x: i32, y: i32) -> Vec<&AoeZone> { self.zones.iter().filter(|z| z.active && z.contains(x, y)).collect() }
    pub fn total_damage_at(&self, x: i32, y: i32) -> f32 { self.zones_at(x, y).iter().map(|z| z.damage_per_tick).sum() }
    pub fn update(&mut self) { for z in &mut self.zones { if z.active { z.elapsed_ticks += 1; if z.elapsed_ticks >= z.duration_ticks { z.active = false; } } } self.zones.retain(|z| z.active); }
}

impl MapEditorStats {
    pub fn new() -> Self { Self { tiles_placed: 0, tiles_erased: 0, session_start: std::time::SystemTime::now(), undo_count: 0, redo_count: 0 } }
    pub fn record_tile_place(&mut self) { self.tiles_placed += 1; }
    pub fn record_tile_erase(&mut self) { self.tiles_erased += 1; }
    pub fn advance_time(&self) {}
    pub fn net_tiles(&self) -> i64 { self.tiles_placed as i64 - self.tiles_erased as i64 }
    pub fn tiles_per_minute(&self) -> f64 { let elapsed = self.session_start.elapsed().unwrap_or_default().as_secs_f64() / 60.0; if elapsed > 0.0 { self.tiles_placed as f64 / elapsed } else { 0.0 } }
    pub fn summary(&self) -> String { format!("placed={} erased={}", self.tiles_placed, self.tiles_erased) }
}

impl MapExportConfig {
    pub fn default_config(filename: &str) -> Self { Self { filename: filename.to_string(), include_metadata: true, compress: false, format: String::from("json"), export_layers: Vec::new() } }
}

impl MapExporter {
    pub fn new(config: MapExportConfig) -> Self { Self { config } }
    pub fn export_header(&self) -> String { format!("# {}", self.config.filename) }
    pub fn export_map_json(&self, _map: &TileMap) -> String { String::from("{}") }
    pub fn export_map_binary(&self, _map: &TileMap) -> Vec<u8> { Vec::new() }
}

impl AutoTilePlugin {
    pub fn new() -> Self { Self { name: String::from("AutoTile"), last_updated: 0 } }
}

impl MapEditorPlugin for AutoTilePlugin {
    fn name(&self) -> &str { &self.name }
    fn on_tile_placed(&mut self, _x: i32, _y: i32, _tile_id: u32) { self.last_updated += 1; }
    fn on_selection_changed(&mut self, _tiles: &[(i32, i32)]) {}
    fn on_layer_changed(&mut self, _layer_name: &str) {}
}

impl CollisionPlugin {
    pub fn new() -> Self { Self { name: String::from("CollisionBuilder"), auto_build: true } }
}

impl MapEditorPlugin for CollisionPlugin {
    fn name(&self) -> &str { &self.name }
    fn on_tile_placed(&mut self, _x: i32, _y: i32, _tile_id: u32) {}
    fn on_selection_changed(&mut self, _tiles: &[(i32, i32)]) {}
    fn on_layer_changed(&mut self, _layer_name: &str) {}
}

impl AutotileLayer {
    pub fn new(w: i32, h: i32) -> Self { Self { width: w, height: h, tiles: vec![0u8; (w*h) as usize], result: Vec::new() } }
    pub fn set(&mut self, x: i32, y: i32, v: u8) { if let Some(c) = self.tiles.get_mut((y*self.width+x) as usize) { *c = v; } }
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32) { for dy in 0..h { for dx in 0..w { self.set(x+dx, y+dy, 1); } } }
    pub fn solid_count(&self) -> usize { self.tiles.iter().filter(|&&v| v != 0).count() }
    pub fn compute_autotile(&mut self) { self.compute_autotiles(); }
    pub fn compute_autotiles(&mut self) {
        self.result.clear();
        let w = self.width; let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if self.tiles[idx] != 0 {
                    let n = if y > 0 { self.tiles[((y-1)*w+x) as usize] != 0 } else { false };
                    let s = if y+1 < h { self.tiles[((y+1)*w+x) as usize] != 0 } else { false };
                    let e = if x+1 < w { self.tiles[(y*w+x+1) as usize] != 0 } else { false };
                    let wo = if x > 0 { self.tiles[(y*w+x-1) as usize] != 0 } else { false };
                    let mask: u8 = (n as u8) | ((e as u8) << 1) | ((s as u8) << 2) | ((wo as u8) << 3);
                    self.result.push((x, y, mask));
                }
            }
        }
    }
}

impl LootTableRegistry {
    pub fn new() -> Self { Self { tables: HashMap::new() } }
    pub fn register(&mut self, id: u32, t: LootTableDef) { self.tables.insert(id, t); }
    pub fn table_count(&self) -> usize { self.tables.len() }
}

impl PathfindingGrid {
    pub fn new(w: usize, h: usize) -> Self { Self { width: w, height: h, passable: vec![true; w * h] } }
    pub fn set_passable(&mut self, x: usize, y: usize, v: bool) { if x < self.width && y < self.height { self.passable[y * self.width + x] = v; } }
    pub fn is_passable(&self, x: usize, y: usize) -> bool { if x < self.width && y < self.height { self.passable[y * self.width + x] } else { false } }
    pub fn find_path_astar(&self, sx: usize, sy: usize, ex: usize, ey: usize) -> Vec<(usize, usize)> {
        use std::collections::BinaryHeap;
        use std::cmp::Reverse;
        if !self.is_passable(ex, ey) { return Vec::new(); }
        let mut dist = vec![usize::MAX; self.width * self.height];
        let mut prev = vec![None::<(usize, usize)>; self.width * self.height];
        let start = sy * self.width + sx;
        dist[start] = 0;
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0usize, sx, sy)));
        while let Some(Reverse((d, x, y))) = heap.pop() {
            if x == ex && y == ey { break; }
            if d > dist[y * self.width + x] { continue; }
            for (nx, ny) in [(x.wrapping_sub(1),y),(x+1,y),(x,y.wrapping_sub(1)),(x,y+1)] {
                if nx < self.width && ny < self.height && self.is_passable(nx, ny) {
                    let nd = d + 1;
                    let ni = ny * self.width + nx;
                    if nd < dist[ni] { dist[ni] = nd; prev[ni] = Some((x, y)); heap.push(Reverse((nd, nx, ny))); }
                }
            }
        }
        let mut path = Vec::new();
        let mut cur = (ex, ey);
        while cur != (sx, sy) {
            path.push(cur);
            if let Some(p) = prev[cur.1 * self.width + cur.0] { cur = p; } else { return Vec::new(); }
        }
        path.reverse();
        path
    }
}

impl PartialEq for TilePos {
    fn eq(&self, o: &Self) -> bool { self.x == o.x && self.y == o.y }
}

impl TilePos {
    pub fn new(x: i32, y: i32) -> Self { Self { x, y } }
    pub fn octile_distance(&self, other: &TilePos) -> f32 {
        let dx = (self.x - other.x).abs() as f32; let dy = (self.y - other.y).abs() as f32;
        let (big, small) = if dx > dy { (dx, dy) } else { (dy, dx) };
        big + (std::f32::consts::SQRT_2 - 1.0) * small
    }
    pub fn manhattan_distance(&self, other: &TilePos) -> i32 { (self.x - other.x).abs() + (self.y - other.y).abs() }
}

impl PatrolWaypoint {
    pub fn new(x: i32, y: i32) -> Self { Self { position: (x, y), wait_time_s: 0.0, animation_hint: String::new() } }
    pub fn with_wait(mut self, t: f32) -> Self { self.wait_time_s = t; self }
}

impl PartialEq for PatrolState {
    fn eq(&self, o: &Self) -> bool { std::mem::discriminant(self) == std::mem::discriminant(o) }
}

impl PatrolAi {
    pub fn new(npc_id: u32, home: (f32, f32), speed: f32) -> Self {
        Self { npc_id, waypoints: Vec::new(), current_waypoint: 0, state: PatrolState::Idle, position: home, move_speed: speed, alert_radius: 5.0, search_timer: 0.0, wait_timer: 0.0, home_position: home }
    }
    pub fn add_waypoint(&mut self, wp: PatrolWaypoint) { self.waypoints.push(wp); }
    pub fn current_target(&self) -> Option<(i32, i32)> { self.waypoints.get(self.current_waypoint).map(|w| w.position) }
    pub fn advance(&mut self) { if !self.waypoints.is_empty() { self.current_waypoint = (self.current_waypoint + 1) % self.waypoints.len(); } }
    pub fn alert(&mut self, x: f32, y: f32) { let dx = x - self.position.0; let dy = y - self.position.1; if (dx*dx+dy*dy).sqrt() < self.alert_radius { self.state = PatrolState::Alert; } }
    pub fn update(&mut self, dt: f32) {
        match self.state {
            PatrolState::Idle => { if !self.waypoints.is_empty() { self.state = PatrolState::Patrolling; } }
            PatrolState::Patrolling => {
                if let Some(target) = self.current_target() {
                    let tx = target.0 as f32; let ty = target.1 as f32;
                    let dx = tx - self.position.0; let dy = ty - self.position.1;
                    let dist = (dx*dx+dy*dy).sqrt();
                    if dist < 0.5 { self.state = PatrolState::Waiting; self.wait_timer = self.waypoints[self.current_waypoint].wait_time_s; }
                    else { let s = self.move_speed * dt / dist; self.position.0 += dx * s; self.position.1 += dy * s; }
                }
            }
            PatrolState::Waiting => { self.wait_timer -= dt; if self.wait_timer <= 0.0 { self.advance(); self.state = PatrolState::Patrolling; } }
            PatrolState::Alert => { self.search_timer += dt; if self.search_timer > 5.0 { self.search_timer = 0.0; self.state = PatrolState::Patrolling; } }
            PatrolState::Returning => { let hx = self.home_position.0; let hy = self.home_position.1; let dx = hx - self.position.0; let dy = hy - self.position.1; let dist = (dx*dx+dy*dy).sqrt(); if dist < 0.5 { self.state = PatrolState::Idle; } else { let s = self.move_speed * dt / dist; self.position.0 += dx * s; self.position.1 += dy * s; } }
        }
    }
}

impl PatrolPath {
    pub fn new(id: u32) -> Self { Self { id, waypoints: Vec::new(), loop_path: true } }
    pub fn add_waypoint(&mut self, wp: PatrolWaypoint) { self.waypoints.push(wp); }
    pub fn current_target(&self, idx: usize) -> Option<(i32, i32)> { self.waypoints.get(idx).map(|w| w.position) }
    pub fn advance(&self, idx: usize) -> usize { if self.waypoints.is_empty() { 0 } else { (idx + 1) % self.waypoints.len() } }
}

impl MapAnnotation {
    pub fn new(id: u32, x: i32, y: i32, text: &str) -> Self { Self { id, tile_pos: (x, y), text: text.to_string(), annotation_type: String::from("info"), color: (255, 255, 0), visible: true } }
    pub fn warning(id: u32, x: i32, y: i32, text: &str) -> Self { let mut a = Self::new(id, x, y, text); a.annotation_type = String::from("warning"); a.color = (255, 128, 0); a }
    pub fn secret(id: u32, x: i32, y: i32, text: &str) -> Self { let mut a = Self::new(id, x, y, text); a.annotation_type = String::from("secret"); a.color = (128, 0, 255); a }
}

impl MapAnnotationLayer {
    pub fn new() -> Self { Self { annotations: Vec::new(), visible: true } }
    pub fn add(&mut self, a: MapAnnotation) { self.annotations.push(a); }
    pub fn remove(&mut self, id: u32) { self.annotations.retain(|a| a.id != id); }
    pub fn visible_count(&self) -> usize { self.annotations.iter().filter(|a| a.visible).count() }
    pub fn at_tile(&self, x: i32, y: i32) -> Vec<&MapAnnotation> { self.annotations.iter().filter(|a| a.tile_pos == (x, y)).collect() }
    pub fn search(&self, q: &str) -> Vec<&MapAnnotation> { self.annotations.iter().filter(|a| a.text.contains(q)).collect() }
    pub fn export_notes(&self) -> Vec<String> { self.annotations.iter().map(|a| format!("({},{}) {}: {}", a.tile_pos.0, a.tile_pos.1, a.annotation_type, a.text)).collect() }
}

impl MapWeatherState {
    pub fn clear() -> Self { Self { weather_type: String::from("clear"), intensity: 0.0, wind_speed: 0.0, wind_dir: 0.0, temperature: 20.0, visibility: 1.0, lightning_chance: 0.0 } }
    pub fn thunderstorm() -> Self { Self { weather_type: String::from("thunderstorm"), intensity: 1.0, wind_speed: 20.0, wind_dir: 270.0, temperature: 15.0, visibility: 0.4, lightning_chance: 0.3 } }
    pub fn rain(intensity: f32) -> Self { Self { weather_type: String::from("rain"), intensity, wind_speed: 5.0, wind_dir: 180.0, temperature: 12.0, visibility: 0.7, lightning_chance: 0.0 } }
    pub fn snow(intensity: f32) -> Self { Self { weather_type: String::from("snow"), intensity, wind_speed: 3.0, wind_dir: 90.0, temperature: -2.0, visibility: 0.6, lightning_chance: 0.0 } }
    pub fn movement_speed_mod(&self) -> f32 { if self.weather_type == "snow" { 0.7 } else if self.weather_type == "rain" { 0.9 } else { 1.0 } }
    pub fn danger_level(&self) -> f32 { self.lightning_chance + self.intensity * 0.5 }
    pub fn ambient_light_mod(&self) -> f32 { 1.0 - self.intensity * 0.3 }
}

impl MapSaveState {
    pub fn new(seed: u64) -> Self { Self { seed, visited_chunks: HashSet::new(), cleared_rooms: HashSet::new(), chest_states: HashMap::new(), npc_states: HashMap::new(), player_position: (0, 0), player_level: 1, play_time_secs: 0 } }
    pub fn mark_visited(&mut self, x: i32, y: i32) { self.visited_chunks.insert((x, y)); }
    pub fn is_visited(&self, x: i32, y: i32) -> bool { self.visited_chunks.contains(&(x, y)) }
    pub fn visited_chunk_count(&self) -> usize { self.visited_chunks.len() }
    pub fn mark_room_cleared(&mut self, room_id: u32) { self.cleared_rooms.insert(room_id); }
    pub fn is_room_cleared(&self, room_id: u32) -> bool { self.cleared_rooms.contains(&room_id) }
    pub fn serialize_header(&self) -> String { format!("seed={} visited={} cleared={}", self.seed, self.visited_chunks.len(), self.cleared_rooms.len()) }
    pub fn save_to_bytes(&self) -> Vec<u8> { Vec::new() }
    pub fn load_from_bytes(_: &[u8]) -> Option<Self> { None }
}

impl DungeonContentPlacer {
    pub fn new(seed: u64) -> Self { Self { rng: MapRng::new(seed), enemy_density: 0.1, chest_density: 0.05, trap_density: 0.03 } }
    pub fn populate_dungeon(&mut self, rooms: &[Room]) -> DungeonContent {
        let mut content = DungeonContent { enemies: Vec::new(), chests: Vec::new(), traps: Vec::new() };
        for room in rooms {
            let cx = (room.x + room.width as i32 / 2) as i32;
            let cy = (room.y + room.height as i32 / 2) as i32;
            if self.rng.next_f32() < self.enemy_density { content.enemies.push((cx, cy, String::from("goblin"))); }
            if self.rng.next_f32() < self.chest_density { content.chests.push((cx+1, cy, 1)); }
            if self.rng.next_f32() < self.trap_density { content.traps.push((cx-1, cy, String::from("spike"))); }
        }
        content
    }
}

impl BspDungeonGenerator {
    pub fn new(seed: u64) -> Self { Self { rng: MapRng::new(seed), room_padding: 2, min_room_size: 5, winding_factor: 0.3 } }
    pub fn generate(&self, _w: usize, _h: usize, _min_rooms: usize) -> (Vec<Room>, Vec<(usize,usize,usize,usize)>) { (Vec::new(), Vec::new()) }
}

impl MinimapGenerator {
    pub fn generate(_map: &TileMap, _db: &TileDatabase, width: usize, height: usize) -> MinimapData { MinimapData { width: width as u32, height: height as u32, pixels: vec![[0u8;4]; width*height], fog_states: vec![FogState::Hidden; width*height], scale_x: 1.0, scale_y: 1.0 } }
}

impl BrushEngine {
    pub fn flood_fill(map: &mut TileMap, layer_idx: usize, x: usize, y: usize, new_tile: u32) {
        if layer_idx >= map.layers.len() { return; }
        let old_tile = map.layers[layer_idx].tile_at(x, y);
        if old_tile == new_tile { return; }
        let w = map.layers[layer_idx].width; let h = map.layers[layer_idx].height;
        let mut stack = vec![(x, y)];
        while let Some((cx, cy)) = stack.pop() {
            if cx >= w || cy >= h { continue; }
            if map.layers[layer_idx].tile_at(cx, cy) != old_tile { continue; }
            map.layers[layer_idx].set(cx, cy, Tile { tile_id: new_tile, ..Tile::default() });
            if cx > 0 { stack.push((cx-1, cy)); }
            if cx+1 < w { stack.push((cx+1, cy)); }
            if cy > 0 { stack.push((cx, cy-1)); }
            if cy+1 < h { stack.push((cx, cy+1)); }
        }
    }
}

impl RoomGraph {
    pub fn add_room(&mut self, room: Room) { self.nodes.insert(room.id, room); }
    pub fn connect(&mut self, a: u32, b: u32) { self.edges.entry(a).or_default().push(b); self.edges.entry(b).or_default().push(a); }
    pub fn is_connected(&self) -> bool {
        if self.nodes.is_empty() { return true; }
        let start = *self.nodes.keys().next().unwrap();
        let mut visited = HashSet::new();
        let mut stack = vec![start];
        while let Some(n) = stack.pop() {
            if visited.insert(n) {
                if let Some(neighbors) = self.edges.get(&n) {
                    for &nb in neighbors { if !visited.contains(&nb) { stack.push(nb); } }
                }
            }
        }
        visited.len() == self.nodes.len()
    }
}
'''

filepath = r'C:\proof-engine\src\editor\map_editor.rs'
with open(filepath, 'a', encoding='utf-8') as f:
    f.write(code)
print("Done")
