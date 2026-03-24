//! BSP dungeon floor generation.
//!
//! Generates a dungeon floor using Binary Space Partitioning:
//! 1. Recursively split a bounding rectangle into sub-regions
//! 2. Place rooms in leaf nodes
//! 3. Connect sibling rooms with L-shaped corridors
//! 4. Choose a spawn point (start) and exit point
//!
//! All coordinates are integer tile coordinates.

use super::Rng;

// ── Theme ─────────────────────────────────────────────────────────────────────

/// Visual theme for a dungeon floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DungeonTheme {
    Cave,
    Cathedral,
    Laboratory,
    Temple,
    Ruins,
    Void,
}

impl DungeonTheme {
    /// Glyph characters used for this theme's floor tiles.
    pub fn floor_glyphs(self) -> &'static [char] {
        match self {
            DungeonTheme::Cave       => &['.', ',', '\''],
            DungeonTheme::Cathedral  => &['+', '.', '·'],
            DungeonTheme::Laboratory => &['.', ':', '·'],
            DungeonTheme::Temple     => &['╬', '╪', '╫', '.'],
            DungeonTheme::Ruins      => &['.', ',', '~', '"'],
            DungeonTheme::Void       => &['.', '·', '∘', '°'],
        }
    }

    /// Glyph characters for wall tiles.
    pub fn wall_glyphs(self) -> &'static [char] {
        match self {
            DungeonTheme::Cave       => &['#', '█', '▓'],
            DungeonTheme::Cathedral  => &['█', '▓', '│', '─'],
            DungeonTheme::Laboratory => &['█', '▓', '╔', '╗'],
            DungeonTheme::Temple     => &['█', '▓', '╠', '╣'],
            DungeonTheme::Ruins      => &['#', '▓', '%'],
            DungeonTheme::Void       => &['▓', '░', '▒'],
        }
    }

    /// The ambient light colour for this theme.
    pub fn ambient_color(self) -> glam::Vec4 {
        match self {
            DungeonTheme::Cave       => glam::Vec4::new(0.4, 0.3, 0.2, 1.0),
            DungeonTheme::Cathedral  => glam::Vec4::new(0.6, 0.5, 0.8, 1.0),
            DungeonTheme::Laboratory => glam::Vec4::new(0.3, 0.8, 0.4, 1.0),
            DungeonTheme::Temple     => glam::Vec4::new(0.8, 0.6, 0.2, 1.0),
            DungeonTheme::Ruins      => glam::Vec4::new(0.5, 0.5, 0.4, 1.0),
            DungeonTheme::Void       => glam::Vec4::new(0.1, 0.0, 0.2, 1.0),
        }
    }
}

// ── Rect / Tile ───────────────────────────────────────────────────────────────

/// An axis-aligned integer rectangle in tile space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x:  i32,
    pub y:  i32,
    pub w:  i32,
    pub h:  i32,
}

impl IRect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self { Self { x, y, w, h } }

    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    pub fn contains(&self, tx: i32, ty: i32) -> bool {
        tx >= self.x && tx < self.x + self.w && ty >= self.y && ty < self.y + self.h
    }

    pub fn overlaps(&self, other: &IRect) -> bool {
        self.x < other.x + other.w && self.x + self.w > other.x
            && self.y < other.y + other.h && self.y + self.h > other.y
    }

    pub fn area(&self) -> i32 { self.w * self.h }

    /// Shrink by `margin` on all sides.
    pub fn shrink(&self, margin: i32) -> Option<IRect> {
        let nw = self.w - margin * 2;
        let nh = self.h - margin * 2;
        if nw < 1 || nh < 1 { return None; }
        Some(IRect::new(self.x + margin, self.y + margin, nw, nh))
    }
}

/// Tile type in the dungeon grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Wall,
    Floor,
    Door,
    Corridor,
    Stairs,
    Void,
}

impl Tile {
    pub fn is_walkable(self) -> bool {
        matches!(self, Tile::Floor | Tile::Door | Tile::Corridor | Tile::Stairs)
    }

    pub fn is_opaque(self) -> bool {
        matches!(self, Tile::Wall | Tile::Void)
    }
}

// ── Room ──────────────────────────────────────────────────────────────────────

/// A room in the dungeon.
#[derive(Debug, Clone)]
pub struct Room {
    pub bounds:  IRect,
    pub id:      usize,
    pub theme:   RoomType,
    /// Whether this room has been visited by the player.
    pub visited: bool,
    /// Spawn points inside this room.
    pub spawns:  Vec<(i32, i32)>,
}

/// Type of room — affects spawns and decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomType {
    Normal,
    Start,
    Boss,
    Treasure,
    Shop,
    Shrine,
    Trap,
}

impl Room {
    pub fn new(id: usize, bounds: IRect) -> Self {
        Self { bounds, id, theme: RoomType::Normal, visited: false, spawns: Vec::new() }
    }

    pub fn center(&self) -> (i32, i32) {
        self.bounds.center()
    }

    /// Distribute spawn points evenly inside the room.
    pub fn generate_spawns(&mut self, rng: &mut Rng, count: usize) {
        let IRect { x, y, w, h } = self.bounds;
        self.spawns.clear();
        for _ in 0..count {
            let sx = rng.range_i32(x + 1, x + w - 2);
            let sy = rng.range_i32(y + 1, y + h - 2);
            self.spawns.push((sx, sy));
        }
    }
}

// ── Corridor ──────────────────────────────────────────────────────────────────

/// An L-shaped corridor connecting two rooms.
#[derive(Debug, Clone)]
pub struct Corridor {
    pub from:   (i32, i32),
    pub to:     (i32, i32),
    pub bend:   (i32, i32),
    pub width:  u8,
    pub has_door: bool,
}

impl Corridor {
    pub fn new(from: (i32, i32), to: (i32, i32), bend: (i32, i32)) -> Self {
        Self { from, to, bend, width: 1, has_door: false }
    }

    /// All tiles covered by this corridor.
    pub fn tiles(&self) -> Vec<(i32, i32)> {
        let mut tiles = Vec::new();
        // Horizontal segment from → bend
        let (x0, y0) = self.from;
        let (xb, yb) = self.bend;
        let (x1, y1) = self.to;

        let hx_range = if x0 <= xb { x0..=xb } else { xb..=x0 };
        for x in hx_range { tiles.push((x, y0)); }

        let vy_range = if yb <= y1 { yb..=y1 } else { y1..=yb };
        for y in vy_range { tiles.push((xb, y)); }
        tiles
    }
}

// ── BSP tree ──────────────────────────────────────────────────────────────────

struct BspNode {
    bounds:  IRect,
    left:    Option<Box<BspNode>>,
    right:   Option<Box<BspNode>>,
    room:    Option<IRect>,
}

impl BspNode {
    fn new(bounds: IRect) -> Self {
        Self { bounds, left: None, right: None, room: None }
    }

    /// Recursively split down to `min_size`.
    fn split(&mut self, rng: &mut Rng, min_size: i32, depth: u32) {
        if depth == 0 || self.bounds.w < min_size * 2 && self.bounds.h < min_size * 2 {
            return;
        }

        let split_h = if self.bounds.w > self.bounds.h {
            false // split vertically
        } else if self.bounds.h > self.bounds.w {
            true  // split horizontally
        } else {
            rng.chance(0.5)
        };

        if split_h {
            if self.bounds.h < min_size * 2 { return; }
            let split_y = rng.range_i32(
                self.bounds.y + min_size,
                self.bounds.y + self.bounds.h - min_size,
            );
            let top = IRect::new(self.bounds.x, self.bounds.y,
                                 self.bounds.w, split_y - self.bounds.y);
            let bot = IRect::new(self.bounds.x, split_y,
                                 self.bounds.w, self.bounds.h - (split_y - self.bounds.y));
            self.left  = Some(Box::new(BspNode::new(top)));
            self.right = Some(Box::new(BspNode::new(bot)));
        } else {
            if self.bounds.w < min_size * 2 { return; }
            let split_x = rng.range_i32(
                self.bounds.x + min_size,
                self.bounds.x + self.bounds.w - min_size,
            );
            let left  = IRect::new(self.bounds.x, self.bounds.y,
                                   split_x - self.bounds.x, self.bounds.h);
            let right = IRect::new(split_x, self.bounds.y,
                                   self.bounds.w - (split_x - self.bounds.x), self.bounds.h);
            self.left  = Some(Box::new(BspNode::new(left)));
            self.right = Some(Box::new(BspNode::new(right)));
        }

        if let Some(ref mut l) = self.left  { l.split(rng, min_size, depth - 1); }
        if let Some(ref mut r) = self.right { r.split(rng, min_size, depth - 1); }
    }

    /// Place rooms in leaf nodes.
    fn place_rooms(&mut self, rng: &mut Rng, min_room: i32, margin: i32) {
        match (&mut self.left, &mut self.right) {
            (Some(l), Some(r)) => {
                l.place_rooms(rng, min_room, margin);
                r.place_rooms(rng, min_room, margin);
            }
            _ => {
                if let Some(shrunk) = self.bounds.shrink(margin) {
                    if shrunk.w >= min_room && shrunk.h >= min_room {
                        let rw = rng.range_i32(min_room, shrunk.w);
                        let rh = rng.range_i32(min_room, shrunk.h);
                        let rx = rng.range_i32(shrunk.x, shrunk.x + shrunk.w - rw);
                        let ry = rng.range_i32(shrunk.y, shrunk.y + shrunk.h - rh);
                        self.room = Some(IRect::new(rx, ry, rw, rh));
                    }
                }
            }
        }
    }

    /// Collect all leaf rooms.
    fn collect_rooms(&self, out: &mut Vec<IRect>) {
        if let Some(room) = self.room {
            out.push(room);
        }
        if let Some(ref l) = self.left  { l.collect_rooms(out); }
        if let Some(ref r) = self.right { r.collect_rooms(out); }
    }

    /// Get the "representative center" of this node's sub-tree (for corridor routing).
    fn representative_room(&self) -> Option<IRect> {
        if let Some(r) = self.room { return Some(r); }
        let lc = self.left.as_ref().and_then(|n| n.representative_room());
        let rc = self.right.as_ref().and_then(|n| n.representative_room());
        lc.or(rc)
    }

    /// Generate corridors connecting sibling subtrees.
    fn generate_corridors(&self, rng: &mut Rng, out: &mut Vec<Corridor>) {
        if let (Some(ref l), Some(ref r)) = (&self.left, &self.right) {
            if let (Some(lr), Some(rr)) = (l.representative_room(), r.representative_room()) {
                let (x0, y0) = lr.center();
                let (x1, y1) = rr.center();
                // Random bend point
                let bend = if rng.chance(0.5) {
                    (x1, y0) // horizontal then vertical
                } else {
                    (x0, y1) // vertical then horizontal
                };
                let mut corridor = Corridor::new((x0, y0), (x1, y1), bend);
                corridor.has_door = rng.chance(0.4);
                out.push(corridor);
            }
            l.generate_corridors(rng, out);
            r.generate_corridors(rng, out);
        }
    }
}

// ── DungeonFloor ──────────────────────────────────────────────────────────────

/// A complete generated dungeon floor.
#[derive(Debug, Clone)]
pub struct DungeonFloor {
    pub width:      usize,
    pub height:     usize,
    pub theme:      DungeonTheme,
    pub rooms:      Vec<Room>,
    pub corridors:  Vec<Corridor>,
    pub tiles:      Vec<Tile>,
    pub depth:      u32,     // floor number (affects difficulty)
    /// Spawn point (player start).
    pub start:      (i32, i32),
    /// Staircase exit.
    pub exit:       (i32, i32),
    /// Boss room index (if any).
    pub boss_room:  Option<usize>,
}

impl DungeonFloor {
    /// Generate a complete floor.
    ///
    /// - `seed`:  reproducibility seed
    /// - `depth`: floor number (1+), affects size and complexity
    /// - `theme`: visual theme
    pub fn generate(seed: u64, depth: u32, theme: DungeonTheme) -> Self {
        let mut rng = Rng::new(seed ^ (depth as u64 * 0xdeadbeef));

        // Scale with depth
        let w = (60 + depth as usize * 5).min(200);
        let h = (40 + depth as usize * 3).min(120);
        let min_room = 5i32;
        let bsp_depth = 5u32 + (depth / 2);

        // BSP split
        let bounds = IRect::new(0, 0, w as i32, h as i32);
        let mut root = BspNode::new(bounds);
        root.split(&mut rng, min_room + 3, bsp_depth);
        root.place_rooms(&mut rng, min_room, 2);

        let mut room_rects = Vec::new();
        root.collect_rooms(&mut room_rects);

        let mut corridors = Vec::new();
        root.generate_corridors(&mut rng, &mut corridors);

        // Build rooms
        let mut rooms: Vec<Room> = room_rects.iter().enumerate()
            .map(|(i, &r)| Room::new(i, r))
            .collect();

        // Assign room types
        if !rooms.is_empty() {
            rooms[0].theme = RoomType::Start;
        }
        let last = rooms.len() - 1;
        if rooms.len() > 1 {
            rooms[last].theme = RoomType::Boss;
        }

        // Boss rooms on deeper floors more likely
        let boss_room = if depth >= 3 { Some(last) } else { None };

        // Scatter treasure rooms
        let n_treasure = (rooms.len() / 6).max(1);
        let mut indices: Vec<usize> = (1..rooms.len() - 1).collect();
        rng.shuffle(&mut indices);
        for &i in indices.iter().take(n_treasure) {
            if rooms[i].theme == RoomType::Normal {
                rooms[i].theme = RoomType::Treasure;
            }
        }

        // Scatter shrine rooms
        for &i in indices.iter().skip(n_treasure).take(n_treasure) {
            if rooms[i].theme == RoomType::Normal {
                rooms[i].theme = RoomType::Shrine;
            }
        }

        // Generate spawn points
        for room in &mut rooms {
            let count = match room.theme {
                RoomType::Normal   => rng.range_usize(2) + 1,
                RoomType::Boss     => 1,
                _                  => 0,
            };
            room.generate_spawns(&mut rng, count);
        }

        // Determine start and exit positions
        let start = rooms.first().map(|r| r.center()).unwrap_or((1, 1));
        let exit  = rooms.last() .map(|r| r.center()).unwrap_or((w as i32 - 2, h as i32 - 2));

        // Build tile grid
        let mut tiles = vec![Tile::Wall; w * h];

        // Paint rooms
        for room in &rooms {
            let IRect { x, y, w: rw, h: rh } = room.bounds;
            for ty in y..(y + rh) {
                for tx in x..(x + rw) {
                    if tx >= 0 && ty >= 0 && (tx as usize) < w && (ty as usize) < h {
                        tiles[ty as usize * w + tx as usize] = Tile::Floor;
                    }
                }
            }
        }

        // Paint corridors
        for corridor in &corridors {
            for (tx, ty) in corridor.tiles() {
                if tx >= 0 && ty >= 0 && (tx as usize) < w && (ty as usize) < h {
                    let idx = ty as usize * w + tx as usize;
                    if tiles[idx] == Tile::Wall {
                        tiles[idx] = Tile::Corridor;
                    }
                }
            }
            // Place doors
            if corridor.has_door {
                let (bx, by) = corridor.bend;
                if bx >= 0 && by >= 0 && (bx as usize) < w && (by as usize) < h {
                    tiles[by as usize * w + bx as usize] = Tile::Door;
                }
            }
        }

        // Place stairs at exit
        let (ex, ey) = exit;
        if ex >= 0 && ey >= 0 && (ex as usize) < w && (ey as usize) < h {
            tiles[ey as usize * w + ex as usize] = Tile::Stairs;
        }

        Self { width: w, height: h, theme, rooms, corridors, tiles, depth, start, exit, boss_room }
    }

    /// Get the tile at `(x, y)`.
    pub fn get(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return Tile::Void;
        }
        self.tiles[y as usize * self.width + x as usize]
    }

    /// Flood-fill reachability from `(sx, sy)`. Returns set of reachable tiles.
    pub fn reachable_tiles(&self, sx: i32, sy: i32) -> Vec<(i32, i32)> {
        let mut visited = vec![false; self.width * self.height];
        let mut queue   = std::collections::VecDeque::new();
        let mut result  = Vec::new();

        if self.get(sx, sy).is_walkable() {
            queue.push_back((sx, sy));
        }

        while let Some((x, y)) = queue.pop_front() {
            if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height { continue; }
            let idx = y as usize * self.width + x as usize;
            if visited[idx] { continue; }
            visited[idx] = true;

            if self.tiles[idx].is_walkable() {
                result.push((x, y));
                for (dx, dy) in &[(0,1),(0,-1),(1,0),(-1,0)] {
                    queue.push_back((x + dx, y + dy));
                }
            }
        }
        result
    }

    /// Count of walkable tiles.
    pub fn walkable_count(&self) -> usize {
        self.tiles.iter().filter(|t| t.is_walkable()).count()
    }

    /// Find the room containing a tile position.
    pub fn room_at(&self, x: i32, y: i32) -> Option<usize> {
        self.rooms.iter().position(|r| r.bounds.contains(x, y))
    }

    /// Return all door positions.
    pub fn doors(&self) -> impl Iterator<Item=(i32,i32)> + '_ {
        (0..self.height).flat_map(move |y| {
            (0..self.width).filter_map(move |x| {
                if self.tiles[y * self.width + x] == Tile::Door {
                    Some((x as i32, y as i32))
                } else {
                    None
                }
            })
        })
    }

    /// Grid dimensions as (width, height).
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

// ── Dungeon metrics ───────────────────────────────────────────────────────────

/// Statistics about a generated floor (for balancing).
#[derive(Debug, Clone)]
pub struct FloorMetrics {
    pub room_count:      usize,
    pub corridor_count:  usize,
    pub walkable_tiles:  usize,
    pub total_tiles:     usize,
    pub fill_ratio:      f32,
    pub has_boss:        bool,
    pub treasure_rooms:  usize,
    pub avg_room_area:   f32,
}

impl FloorMetrics {
    pub fn compute(floor: &DungeonFloor) -> Self {
        let walkable  = floor.walkable_count();
        let total     = floor.width * floor.height;
        let treasure  = floor.rooms.iter().filter(|r| r.theme == RoomType::Treasure).count();
        let avg_area  = if floor.rooms.is_empty() { 0.0 } else {
            floor.rooms.iter().map(|r| r.bounds.area()).sum::<i32>() as f32 / floor.rooms.len() as f32
        };
        Self {
            room_count:     floor.rooms.len(),
            corridor_count: floor.corridors.len(),
            walkable_tiles: walkable,
            total_tiles:    total,
            fill_ratio:     walkable as f32 / total as f32,
            has_boss:       floor.boss_room.is_some(),
            treasure_rooms: treasure,
            avg_room_area:  avg_area,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_generates_rooms() {
        let floor = DungeonFloor::generate(42, 1, DungeonTheme::Cave);
        assert!(!floor.rooms.is_empty(), "expected at least one room");
    }

    #[test]
    fn start_tile_is_walkable() {
        let floor = DungeonFloor::generate(99, 1, DungeonTheme::Cave);
        let (sx, sy) = floor.start;
        assert!(floor.get(sx, sy).is_walkable(), "start tile should be walkable");
    }

    #[test]
    fn walkable_tiles_from_start_is_nonempty() {
        let floor = DungeonFloor::generate(1234, 1, DungeonTheme::Cave);
        let reachable = floor.reachable_tiles(floor.start.0, floor.start.1);
        assert!(reachable.len() > 10, "too few reachable tiles: {}", reachable.len());
    }

    #[test]
    fn fill_ratio_reasonable() {
        let floor = DungeonFloor::generate(7, 1, DungeonTheme::Cave);
        let m = FloorMetrics::compute(&floor);
        assert!(m.fill_ratio > 0.05 && m.fill_ratio < 0.9,
                "fill ratio out of range: {}", m.fill_ratio);
    }
}
