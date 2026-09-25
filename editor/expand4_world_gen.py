
code = r'''

// ============================================================
// EXPANSION 4: Procedural Dungeon Generator
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DungeonRoomType {
    Entrance,
    Exit,
    BossRoom,
    TreasureRoom,
    MonsterDen,
    Corridor,
    Shrine,
    Library,
    Armory,
    Barracks,
    Kitchen,
    Throne,
    Crypt,
    Laboratory,
    Prison,
    Garden,
    Forge,
    Vault,
    Trap,
    Empty,
}

impl DungeonRoomType {
    pub fn name(&self) -> &'static str {
        match self {
            DungeonRoomType::Entrance => "Entrance",
            DungeonRoomType::Exit => "Exit",
            DungeonRoomType::BossRoom => "Boss Chamber",
            DungeonRoomType::TreasureRoom => "Treasure Room",
            DungeonRoomType::MonsterDen => "Monster Den",
            DungeonRoomType::Corridor => "Corridor",
            DungeonRoomType::Shrine => "Shrine",
            DungeonRoomType::Library => "Library",
            DungeonRoomType::Armory => "Armory",
            DungeonRoomType::Barracks => "Barracks",
            DungeonRoomType::Kitchen => "Kitchen",
            DungeonRoomType::Throne => "Throne Room",
            DungeonRoomType::Crypt => "Crypt",
            DungeonRoomType::Laboratory => "Laboratory",
            DungeonRoomType::Prison => "Prison",
            DungeonRoomType::Garden => "Garden",
            DungeonRoomType::Forge => "Forge",
            DungeonRoomType::Vault => "Vault",
            DungeonRoomType::Trap => "Trap Room",
            DungeonRoomType::Empty => "Empty Room",
        }
    }
    pub fn danger_level(&self) -> u8 {
        match self {
            DungeonRoomType::BossRoom => 10,
            DungeonRoomType::MonsterDen => 7,
            DungeonRoomType::Trap => 5,
            DungeonRoomType::TreasureRoom => 4,
            DungeonRoomType::Crypt => 6,
            DungeonRoomType::Laboratory => 5,
            DungeonRoomType::Prison => 3,
            DungeonRoomType::Shrine => 2,
            DungeonRoomType::Empty => 0,
            _ => 2,
        }
    }
    pub fn loot_value(&self) -> u32 {
        match self {
            DungeonRoomType::TreasureRoom => 1000,
            DungeonRoomType::Vault => 2000,
            DungeonRoomType::BossRoom => 1500,
            DungeonRoomType::Armory => 600,
            DungeonRoomType::Library => 400,
            DungeonRoomType::Laboratory => 500,
            DungeonRoomType::Forge => 350,
            DungeonRoomType::Shrine => 200,
            _ => 50,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonRoom {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub room_type: DungeonRoomType,
    pub connections: Vec<u32>,
    pub depth: u32,
    pub visited: bool,
    pub cleared: bool,
    pub special_feature: Option<String>,
    pub monster_count: u32,
    pub treasure_amount: u32,
    pub trap_count: u32,
    pub is_locked: bool,
    pub lock_key_room: Option<u32>,
    pub lighting: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub notes: String,
}

impl DungeonRoom {
    pub fn new(id: u32, x: i32, y: i32, w: u32, h: u32, rt: DungeonRoomType) -> Self {
        let danger = rt.danger_level() as u32;
        let loot = rt.loot_value();
        DungeonRoom {
            id,
            x, y,
            width: w,
            height: h,
            room_type: rt,
            connections: vec![],
            depth: 0,
            visited: false,
            cleared: false,
            special_feature: None,
            monster_count: danger.saturating_sub(1),
            treasure_amount: loot / 100,
            trap_count: 0,
            is_locked: false,
            lock_key_room: None,
            lighting: 0.3,
            temperature: 15.0,
            humidity: 0.5,
            notes: String::new(),
        }
    }

    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width as i32 / 2, self.y + self.height as i32 / 2)
    }

    pub fn overlaps(&self, other: &DungeonRoom) -> bool {
        self.x < other.x + other.width as i32 + 1
            && self.x + self.width as i32 + 1 > other.x
            && self.y < other.y + other.height as i32 + 1
            && self.y + self.height as i32 + 1 > other.y
    }

    pub fn area(&self) -> u32 { self.width * self.height }

    pub fn total_threat(&self) -> u32 {
        self.room_type.danger_level() as u32 * (self.monster_count + 1) + self.trap_count * 2
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonCorridor {
    pub from_room: u32,
    pub to_room: u32,
    pub points: Vec<(i32, i32)>,
    pub width: u32,
    pub has_door: bool,
    pub door_locked: bool,
    pub secret: bool,
    pub trap_chance: f32,
}

impl DungeonCorridor {
    pub fn new(from: u32, to: u32, pts: Vec<(i32, i32)>) -> Self {
        DungeonCorridor {
            from_room: from,
            to_room: to,
            points: pts,
            width: 2,
            has_door: false,
            door_locked: false,
            secret: false,
            trap_chance: 0.0,
        }
    }
    pub fn length(&self) -> f32 {
        if self.points.len() < 2 { return 0.0; }
        self.points.windows(2).map(|w| {
            let dx = (w[1].0 - w[0].0) as f32;
            let dy = (w[1].1 - w[0].1) as f32;
            (dx*dx + dy*dy).sqrt()
        }).sum()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonLevel {
    pub level_number: u32,
    pub width: u32,
    pub height: u32,
    pub rooms: Vec<DungeonRoom>,
    pub corridors: Vec<DungeonCorridor>,
    pub entrance_room: u32,
    pub exit_room: u32,
    pub boss_room: Option<u32>,
    pub theme: DungeonTheme,
    pub ambient_light: f32,
    pub ambient_sound: String,
    pub tile_grid: Vec<Vec<u8>>,
    pub total_depth: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DungeonTheme {
    Classic,
    Crypt,
    Cavern,
    IceCave,
    LavaCave,
    AncientTemple,
    ForestRuins,
    SewerSystem,
    Fortress,
    Void,
}

impl DungeonTheme {
    pub fn name(&self) -> &'static str {
        match self {
            DungeonTheme::Classic => "Classic Dungeon",
            DungeonTheme::Crypt => "Undead Crypt",
            DungeonTheme::Cavern => "Natural Cavern",
            DungeonTheme::IceCave => "Ice Cave",
            DungeonTheme::LavaCave => "Lava Cave",
            DungeonTheme::AncientTemple => "Ancient Temple",
            DungeonTheme::ForestRuins => "Forest Ruins",
            DungeonTheme::SewerSystem => "Sewer System",
            DungeonTheme::Fortress => "Dark Fortress",
            DungeonTheme::Void => "Void Dimension",
        }
    }
    pub fn ambient_temperature(&self) -> f32 {
        match self {
            DungeonTheme::IceCave => -10.0,
            DungeonTheme::LavaCave => 60.0,
            DungeonTheme::Crypt => 5.0,
            DungeonTheme::SewerSystem => 18.0,
            _ => 12.0,
        }
    }
}

impl DungeonLevel {
    pub fn new(level_number: u32, width: u32, height: u32, theme: DungeonTheme) -> Self {
        DungeonLevel {
            level_number,
            width,
            height,
            rooms: vec![],
            corridors: vec![],
            entrance_room: 0,
            exit_room: 0,
            boss_room: None,
            theme,
            ambient_light: 0.1,
            ambient_sound: "drip".to_string(),
            tile_grid: vec![vec![0u8; width as usize]; height as usize],
            total_depth: level_number,
        }
    }

    pub fn total_rooms(&self) -> usize { self.rooms.len() }
    pub fn cleared_rooms(&self) -> usize { self.rooms.iter().filter(|r| r.cleared).count() }
    pub fn completion_pct(&self) -> f32 {
        if self.rooms.is_empty() { return 0.0; }
        self.cleared_rooms() as f32 / self.total_rooms() as f32 * 100.0
    }

    pub fn room_by_id(&self, id: u32) -> Option<&DungeonRoom> {
        self.rooms.iter().find(|r| r.id == id)
    }

    pub fn add_room(&mut self, room: DungeonRoom) {
        self.rooms.push(room);
    }

    pub fn connect_rooms(&mut self, from_id: u32, to_id: u32) {
        if let Some(r) = self.rooms.iter_mut().find(|r| r.id == from_id) {
            if !r.connections.contains(&to_id) { r.connections.push(to_id); }
        }
        if let Some(r) = self.rooms.iter_mut().find(|r| r.id == to_id) {
            if !r.connections.contains(&from_id) { r.connections.push(from_id); }
        }
    }

    pub fn total_treasure(&self) -> u32 {
        self.rooms.iter().map(|r| r.treasure_amount).sum()
    }
    pub fn total_monsters(&self) -> u32 {
        self.rooms.iter().map(|r| r.monster_count).sum()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonGenConfig {
    pub seed: u64,
    pub levels: u32,
    pub min_rooms_per_level: u32,
    pub max_rooms_per_level: u32,
    pub min_room_size: u32,
    pub max_room_size: u32,
    pub corridor_width: u32,
    pub theme: DungeonTheme,
    pub boss_every_n_levels: u32,
    pub treasure_density: f32,
    pub monster_density: f32,
    pub trap_density: f32,
    pub secret_room_chance: f32,
    pub locked_door_chance: f32,
    pub allow_loops: bool,
    pub use_bsp: bool,
}

impl Default for DungeonGenConfig {
    fn default() -> Self {
        DungeonGenConfig {
            seed: 42,
            levels: 5,
            min_rooms_per_level: 8,
            max_rooms_per_level: 20,
            min_room_size: 4,
            max_room_size: 12,
            corridor_width: 2,
            theme: DungeonTheme::Classic,
            boss_every_n_levels: 5,
            treasure_density: 0.3,
            monster_density: 0.5,
            trap_density: 0.2,
            secret_room_chance: 0.1,
            locked_door_chance: 0.2,
            allow_loops: true,
            use_bsp: true,
        }
    }
}

pub fn generate_dungeon(cfg: &DungeonGenConfig) -> Vec<DungeonLevel> {
    let mut rng = SimpleRng::new(cfg.seed);
    let mut levels = vec![];

    for level_idx in 0..cfg.levels {
        let mut level = DungeonLevel::new(level_idx + 1, 80, 60, cfg.theme.clone());
        let num_rooms = cfg.min_rooms_per_level
            + rng.next_u64() as u32 % (cfg.max_rooms_per_level - cfg.min_rooms_per_level + 1);

        let mut next_id = 0u32;
        for _attempt in 0..num_rooms * 10 {
            if level.rooms.len() >= num_rooms as usize { break; }
            let w = cfg.min_room_size + rng.next_u64() as u32 % (cfg.max_room_size - cfg.min_room_size + 1);
            let h = cfg.min_room_size + rng.next_u64() as u32 % (cfg.max_room_size - cfg.min_room_size + 1);
            let x = 1 + (rng.next_u64() as i32).abs() % (level.width as i32 - w as i32 - 2);
            let y = 1 + (rng.next_u64() as i32).abs() % (level.height as i32 - h as i32 - 2);

            let rt = match rng.next_u64() % 20 {
                0 if level.rooms.is_empty() => DungeonRoomType::Entrance,
                1 => DungeonRoomType::TreasureRoom,
                2 => DungeonRoomType::MonsterDen,
                3 => DungeonRoomType::Shrine,
                4 => DungeonRoomType::Library,
                5 => DungeonRoomType::Crypt,
                6 => DungeonRoomType::Trap,
                _ => DungeonRoomType::Empty,
            };

            let candidate = DungeonRoom::new(next_id, x, y, w, h, rt);
            let overlaps = level.rooms.iter().any(|r| r.overlaps(&candidate));
            if !overlaps {
                level.rooms.push(candidate);
                next_id += 1;
            }
        }

        // Add exit room
        if !level.rooms.is_empty() {
            let exit_x = (level.width as i32 - 6).max(1);
            let exit_y = (level.height as i32 - 6).max(1);
            let exit = DungeonRoom::new(next_id, exit_x, exit_y, 4, 4, DungeonRoomType::Exit);
            level.rooms.push(exit);
            level.exit_room = next_id;
            next_id += 1;
        }

        // Add boss room on boss levels
        if (level_idx + 1) % cfg.boss_every_n_levels == 0 {
            let boss = DungeonRoom::new(next_id, 35, 25, 10, 10, DungeonRoomType::BossRoom);
            level.rooms.push(boss);
            level.boss_room = Some(next_id);
        }

        // Connect rooms by nearest-neighbor spanning tree
        let n = level.rooms.len();
        let mut connected = vec![false; n];
        if n > 0 {
            connected[0] = true;
            level.entrance_room = level.rooms[0].id;
        }
        for _ in 0..n.saturating_sub(1) {
            let mut best_dist = f32::MAX;
            let mut best_from = 0usize;
            let mut best_to = 0usize;
            for i in 0..n {
                if !connected[i] { continue; }
                for j in 0..n {
                    if connected[j] { continue; }
                    let (cx0, cy0) = level.rooms[i].center();
                    let (cx1, cy1) = level.rooms[j].center();
                    let dx = (cx1 - cx0) as f32;
                    let dy = (cy1 - cy0) as f32;
                    let d = (dx*dx + dy*dy).sqrt();
                    if d < best_dist {
                        best_dist = d;
                        best_from = i;
                        best_to = j;
                    }
                }
            }
            if best_to < n {
                let fid = level.rooms[best_from].id;
                let tid = level.rooms[best_to].id;
                level.connect_rooms(fid, tid);
                let (cx0, cy0) = level.rooms[best_from].center();
                let (cx1, cy1) = level.rooms[best_to].center();
                let corridor = DungeonCorridor::new(fid, tid, vec![
                    (cx0, cy0), (cx1, cy0), (cx1, cy1)
                ]);
                level.corridors.push(corridor);
                connected[best_to] = true;
            }
        }

        levels.push(level);
    }
    levels
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonEditorState {
    pub config: DungeonGenConfig,
    pub levels: Vec<DungeonLevel>,
    pub selected_level: usize,
    pub selected_room: Option<u32>,
    pub show_grid: bool,
    pub show_room_ids: bool,
    pub show_connections: bool,
    pub show_stats: bool,
    pub zoom: f32,
    pub pan: (f32, f32),
    pub generated: bool,
}

impl Default for DungeonEditorState {
    fn default() -> Self {
        DungeonEditorState {
            config: DungeonGenConfig::default(),
            levels: vec![],
            selected_level: 0,
            selected_room: None,
            show_grid: true,
            show_room_ids: true,
            show_connections: true,
            show_stats: false,
            zoom: 8.0,
            pan: (0.0, 0.0),
            generated: false,
        }
    }
}

pub fn show_dungeon_editor(ui: &mut egui::Ui, state: &mut DungeonEditorState) {
    ui.horizontal(|ui| {
        ui.heading("Dungeon Generator");
        if ui.button("Generate").clicked() {
            state.levels = generate_dungeon(&state.config);
            state.generated = true;
            if !state.levels.is_empty() { state.selected_level = 0; }
        }
    });

    egui::SidePanel::left("dungeon_cfg").show_inside(ui, |ui| {
        ui.label("Seed:");
        ui.add(egui::DragValue::new(&mut state.config.seed));
        ui.label("Levels:");
        ui.add(egui::Slider::new(&mut state.config.levels, 1..=20));
        ui.label("Rooms per level:");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut state.config.min_rooms_per_level).prefix("min:"));
            ui.add(egui::DragValue::new(&mut state.config.max_rooms_per_level).prefix("max:"));
        });
        ui.label("Room size:");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut state.config.min_room_size).prefix("min:"));
            ui.add(egui::DragValue::new(&mut state.config.max_room_size).prefix("max:"));
        });
        ui.label("Densities:");
        ui.add(egui::Slider::new(&mut state.config.monster_density, 0.0..=1.0).text("Monster"));
        ui.add(egui::Slider::new(&mut state.config.treasure_density, 0.0..=1.0).text("Treasure"));
        ui.add(egui::Slider::new(&mut state.config.trap_density, 0.0..=1.0).text("Traps"));
        ui.checkbox(&mut state.config.allow_loops, "Allow Loops");
        ui.checkbox(&mut state.config.use_bsp, "Use BSP");

        ui.separator();
        ui.checkbox(&mut state.show_grid, "Show Grid");
        ui.checkbox(&mut state.show_room_ids, "Show IDs");
        ui.checkbox(&mut state.show_connections, "Show Connections");
        ui.add(egui::Slider::new(&mut state.zoom, 2.0..=20.0).text("Zoom"));
    });

    if !state.levels.is_empty() {
        ui.horizontal(|ui| {
            for i in 0..state.levels.len() {
                if ui.selectable_label(state.selected_level == i, format!("L{}", i+1)).clicked() {
                    state.selected_level = i;
                }
            }
        });

        let level = &state.levels[state.selected_level];
        ui.label(format!("Rooms: {} | Cleared: {}/{} ({:.0}%)",
            level.total_rooms(), level.cleared_rooms(), level.total_rooms(), level.completion_pct()));
        ui.label(format!("Monsters: {} | Treasure: {} gold",
            level.total_monsters(), level.total_treasure()));

        let painter_rect = ui.available_rect_before_wrap();
        let painter = ui.painter_at(painter_rect);

        let room_color_fn = |rt: &DungeonRoomType| -> egui::Color32 {
            match rt {
                DungeonRoomType::BossRoom => egui::Color32::from_rgb(180, 20, 20),
                DungeonRoomType::TreasureRoom => egui::Color32::from_rgb(200, 180, 20),
                DungeonRoomType::Entrance => egui::Color32::from_rgb(20, 180, 20),
                DungeonRoomType::Exit => egui::Color32::from_rgb(20, 100, 200),
                DungeonRoomType::Shrine => egui::Color32::from_rgb(180, 100, 200),
                DungeonRoomType::MonsterDen => egui::Color32::from_rgb(200, 80, 20),
                DungeonRoomType::Crypt => egui::Color32::from_rgb(80, 60, 100),
                DungeonRoomType::Library => egui::Color32::from_rgb(60, 100, 80),
                DungeonRoomType::Trap => egui::Color32::from_rgb(200, 100, 20),
                _ => egui::Color32::from_rgb(80, 80, 80),
            }
        };

        let ox = painter_rect.min.x + state.pan.0 + 10.0;
        let oy = painter_rect.min.y + state.pan.1 + 10.0;
        let z = state.zoom;

        if state.show_connections {
            for corridor in &level.corridors {
                if corridor.points.len() >= 2 {
                    for w in corridor.points.windows(2) {
                        let p0 = egui::pos2(ox + w[0].0 as f32 * z, oy + w[0].1 as f32 * z);
                        let p1 = egui::pos2(ox + w[1].0 as f32 * z, oy + w[1].1 as f32 * z);
                        painter.line_segment([p0, p1], egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 80, 60)));
                    }
                }
            }
        }

        for room in &level.rooms {
            let rx = ox + room.x as f32 * z;
            let ry = oy + room.y as f32 * z;
            let rw = room.width as f32 * z;
            let rh = room.height as f32 * z;
            let rect = egui::Rect::from_min_size(egui::pos2(rx, ry), egui::vec2(rw, rh));
            let color = room_color_fn(&room.room_type);
            let is_selected = state.selected_room == Some(room.id);
            painter.rect_filled(rect, 2.0, color.linear_multiply(if is_selected { 1.3 } else { 0.8 }));
            painter.rect_stroke(rect, 2.0, egui::Stroke::new(if is_selected { 2.0 } else { 1.0 },
                if is_selected { egui::Color32::WHITE } else { egui::Color32::from_gray(160) }));
            if state.show_room_ids {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER,
                    room.id.to_string(), egui::FontId::proportional(9.0), egui::Color32::WHITE);
            }
        }

        if let Some(sel_id) = state.selected_room {
            if let Some(room) = level.room_by_id(sel_id) {
                egui::Window::new(format!("Room {} - {}", room.id, room.room_type.name()))
                    .show(ui.ctx(), |ui| {
                        ui.label(format!("Position: ({}, {})", room.x, room.y));
                        ui.label(format!("Size: {}x{}", room.width, room.height));
                        ui.label(format!("Monsters: {}", room.monster_count));
                        ui.label(format!("Treasure: {} gp", room.treasure_amount));
                        ui.label(format!("Traps: {}", room.trap_count));
                        ui.label(format!("Danger: {}", room.room_type.danger_level()));
                        ui.label(format!("Connections: {:?}", room.connections));
                    });
            }
        }
    }
}

// ============================================================
// EXPANSION 4: Civilization Simulator
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CivStage {
    Nomadic,
    Tribal,
    Chiefdom,
    CityState,
    Kingdom,
    Empire,
    Collapsed,
}

impl CivStage {
    pub fn name(&self) -> &'static str {
        match self {
            CivStage::Nomadic => "Nomadic",
            CivStage::Tribal => "Tribal",
            CivStage::Chiefdom => "Chiefdom",
            CivStage::CityState => "City-State",
            CivStage::Kingdom => "Kingdom",
            CivStage::Empire => "Empire",
            CivStage::Collapsed => "Collapsed",
        }
    }
    pub fn population_threshold(&self) -> u64 {
        match self {
            CivStage::Nomadic => 100,
            CivStage::Tribal => 500,
            CivStage::Chiefdom => 2000,
            CivStage::CityState => 10000,
            CivStage::Kingdom => 100000,
            CivStage::Empire => 1000000,
            CivStage::Collapsed => 0,
        }
    }
    pub fn next(&self) -> Option<CivStage> {
        match self {
            CivStage::Nomadic => Some(CivStage::Tribal),
            CivStage::Tribal => Some(CivStage::Chiefdom),
            CivStage::Chiefdom => Some(CivStage::CityState),
            CivStage::CityState => Some(CivStage::Kingdom),
            CivStage::Kingdom => Some(CivStage::Empire),
            CivStage::Empire => None,
            CivStage::Collapsed => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TechTree {
    pub agriculture: u8,
    pub metalworking: u8,
    pub writing: u8,
    pub mathematics: u8,
    pub astronomy: u8,
    pub medicine: u8,
    pub architecture: u8,
    pub military: u8,
    pub navigation: u8,
    pub magic: u8,
    pub engineering: u8,
    pub philosophy: u8,
}

impl Default for TechTree {
    fn default() -> Self {
        TechTree {
            agriculture: 1, metalworking: 0, writing: 0, mathematics: 0,
            astronomy: 0, medicine: 0, architecture: 0, military: 1,
            navigation: 0, magic: 0, engineering: 0, philosophy: 0,
        }
    }
}

impl TechTree {
    pub fn total_tech_level(&self) -> u32 {
        self.agriculture as u32 + self.metalworking as u32 + self.writing as u32
            + self.mathematics as u32 + self.astronomy as u32 + self.medicine as u32
            + self.architecture as u32 + self.military as u32 + self.navigation as u32
            + self.magic as u32 + self.engineering as u32 + self.philosophy as u32
    }

    pub fn advance(&mut self, field: &str, rng: &mut SimpleRng) {
        let success = rng.next_u64() % 100 < 60;
        if !success { return; }
        match field {
            "agriculture" => { if self.agriculture < 10 { self.agriculture += 1; } },
            "metalworking" => { if self.metalworking < 10 && self.agriculture >= 2 { self.metalworking += 1; } },
            "writing" => { if self.writing < 10 && self.agriculture >= 3 { self.writing += 1; } },
            "mathematics" => { if self.mathematics < 10 && self.writing >= 1 { self.mathematics += 1; } },
            "astronomy" => { if self.astronomy < 10 && self.mathematics >= 1 { self.astronomy += 1; } },
            "medicine" => { if self.medicine < 10 && self.writing >= 2 { self.medicine += 1; } },
            "architecture" => { if self.architecture < 10 && self.metalworking >= 2 { self.architecture += 1; } },
            "military" => { if self.military < 10 && self.metalworking >= 1 { self.military += 1; } },
            "navigation" => { if self.navigation < 10 && self.astronomy >= 2 { self.navigation += 1; } },
            "magic" => { if self.magic < 10 && self.astronomy >= 3 { self.magic += 1; } },
            "engineering" => { if self.engineering < 10 && self.mathematics >= 3 { self.engineering += 1; } },
            "philosophy" => { if self.philosophy < 10 && self.writing >= 3 { self.philosophy += 1; } },
            _ => {},
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Civilization {
    pub id: u32,
    pub name: String,
    pub stage: CivStage,
    pub population: u64,
    pub territory_cells: Vec<(u32, u32)>,
    pub capital: (u32, u32),
    pub tech: TechTree,
    pub stability: f32,
    pub wealth: f64,
    pub military_strength: u32,
    pub culture_score: u32,
    pub age_years: u32,
    pub is_player: bool,
    pub color: [u8; 3],
    pub relations: Vec<(u32, i32)>,
    pub vassal_of: Option<u32>,
    pub vassals: Vec<u32>,
    pub at_war_with: Vec<u32>,
}

impl Civilization {
    pub fn new(id: u32, name: String, capital: (u32, u32), color: [u8; 3]) -> Self {
        Civilization {
            id, name, stage: CivStage::Nomadic, population: 100,
            territory_cells: vec![capital],
            capital,
            tech: TechTree::default(),
            stability: 0.8,
            wealth: 100.0,
            military_strength: 10,
            culture_score: 0,
            age_years: 0,
            is_player: false,
            color,
            relations: vec![],
            vassal_of: None,
            vassals: vec![],
            at_war_with: vec![],
        }
    }

    pub fn simulate_year(&mut self, rng: &mut SimpleRng) {
        self.age_years += 1;

        // Population growth
        let growth_rate = 0.02 * self.stability * (1.0 + self.tech.agriculture as f32 * 0.05);
        let growth = (self.population as f64 * growth_rate as f64) as u64;
        self.population += growth.max(1);

        // Check stage advancement
        if let Some(next) = self.stage.next() {
            if self.population >= self.stage.population_threshold() {
                self.stage = next;
            }
        }

        // Wealth generation
        let income = self.territory_cells.len() as f64
            * (1.0 + self.tech.agriculture as f64 * 0.1)
            * (1.0 + self.tech.metalworking as f64 * 0.05)
            * self.stability as f64;
        self.wealth += income;

        // Tech advancement
        let fields = ["agriculture", "metalworking", "writing", "mathematics",
                       "astronomy", "medicine", "architecture", "military",
                       "navigation", "magic", "engineering", "philosophy"];
        let field = fields[rng.next_u64() as usize % fields.len()];
        self.tech.advance(field, rng);

        // Military
        self.military_strength = (self.population / 10) as u32
            * (1 + self.tech.military as u32)
            / 100;

        // Culture
        self.culture_score += self.tech.philosophy as u32
            + self.tech.writing as u32;

        // Stability drift
        let delta = (rng.next_u64() as f32 / u64::MAX as f32) * 0.1 - 0.05;
        self.stability = (self.stability + delta).clamp(0.0, 1.0);
    }

    pub fn relation_with(&self, other_id: u32) -> i32 {
        self.relations.iter().find(|(id, _)| *id == other_id).map(|(_, v)| *v).unwrap_or(0)
    }

    pub fn set_relation(&mut self, other_id: u32, value: i32) {
        if let Some(r) = self.relations.iter_mut().find(|(id, _)| *id == other_id) {
            r.1 = value;
        } else {
            self.relations.push((other_id, value));
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CivSimState {
    pub civilizations: Vec<Civilization>,
    pub year: u32,
    pub map_width: u32,
    pub map_height: u32,
    pub selected_civ: Option<u32>,
    pub running: bool,
    pub speed: u32,
    pub show_territory: bool,
    pub show_capitals: bool,
    pub show_relations: bool,
    pub history_log: Vec<String>,
    pub max_history: usize,
    pub rng: SimpleRng,
}

impl Default for CivSimState {
    fn default() -> Self {
        CivSimState {
            civilizations: vec![],
            year: 0,
            map_width: 100,
            map_height: 100,
            selected_civ: None,
            running: false,
            speed: 1,
            show_territory: true,
            show_capitals: true,
            show_relations: false,
            history_log: vec![],
            max_history: 200,
            rng: SimpleRng::new(12345),
        }
    }
}

impl CivSimState {
    pub fn add_civilization(&mut self, name: String, capital: (u32, u32)) {
        let id = self.civilizations.len() as u32;
        let r = (self.rng.next_u64() % 200 + 55) as u8;
        let g = (self.rng.next_u64() % 200 + 55) as u8;
        let b = (self.rng.next_u64() % 200 + 55) as u8;
        self.civilizations.push(Civilization::new(id, name.clone(), capital, [r, g, b]));
        self.log(format!("Year {}: {} founded at ({},{})", self.year, name, capital.0, capital.1));
    }

    pub fn step(&mut self, years: u32) {
        for _ in 0..years {
            self.year += 1;
            let mut rng = SimpleRng::new(self.rng.next_u64());
            for civ in self.civilizations.iter_mut() {
                if civ.stage == CivStage::Collapsed { continue; }
                civ.simulate_year(&mut rng);
            }
            // Check for wars
            let n = self.civilizations.len();
            for i in 0..n {
                for j in (i+1)..n {
                    if self.civilizations[i].stage == CivStage::Collapsed { continue; }
                    if self.civilizations[j].stage == CivStage::Collapsed { continue; }
                    let rel_ij = self.civilizations[i].relation_with(self.civilizations[j].id);
                    if rel_ij < -50 && rng.next_u64() % 100 < 20 {
                        let a_id = self.civilizations[i].id;
                        let b_id = self.civilizations[j].id;
                        if !self.civilizations[i].at_war_with.contains(&b_id) {
                            self.civilizations[i].at_war_with.push(b_id);
                            self.civilizations[j].at_war_with.push(a_id);
                            self.log(format!("Year {}: {} declares war on {}!", self.year,
                                self.civilizations[i].name.clone(),
                                self.civilizations[j].name.clone()));
                        }
                    }
                }
            }
        }
    }

    pub fn log(&mut self, msg: String) {
        self.history_log.push(msg);
        if self.history_log.len() > self.max_history {
            self.history_log.remove(0);
        }
    }

    pub fn civ_by_id(&self, id: u32) -> Option<&Civilization> {
        self.civilizations.iter().find(|c| c.id == id)
    }
}

pub fn show_civ_simulator(ui: &mut egui::Ui, state: &mut CivSimState) {
    ui.horizontal(|ui| {
        ui.heading("Civilization Simulator");
        ui.label(format!("Year: {}", state.year));
        if ui.button("Step 1").clicked() { state.step(1); }
        if ui.button("Step 10").clicked() { state.step(10); }
        if ui.button("Step 100").clicked() { state.step(100); }
        ui.checkbox(&mut state.show_territory, "Territory");
        ui.checkbox(&mut state.show_capitals, "Capitals");
    });

    egui::SidePanel::left("civ_list").show_inside(ui, |ui| {
        ui.heading("Civilizations");
        for civ in &state.civilizations {
            let label = format!("{} ({}) Pop:{}", civ.name, civ.stage.name(),
                if civ.population > 1_000_000 { format!("{:.1}M", civ.population as f64 / 1e6) }
                else if civ.population > 1000 { format!("{:.0}K", civ.population as f64 / 1000.0) }
                else { civ.population.to_string() });
            if ui.selectable_label(state.selected_civ == Some(civ.id), label).clicked() {
                state.selected_civ = Some(civ.id);
            }
        }
        ui.separator();
        if ui.button("Add Random Civ").clicked() {
            let n = state.civilizations.len() + 1;
            let cx = (state.rng.next_u64() % state.map_width as u64) as u32;
            let cy = (state.rng.next_u64() % state.map_height as u64) as u32;
            state.add_civilization(format!("Civ{}", n), (cx, cy));
        }
    });

    if let Some(sel_id) = state.selected_civ {
        if let Some(civ) = state.civilizations.iter().find(|c| c.id == sel_id) {
            egui::SidePanel::right("civ_detail").show_inside(ui, |ui| {
                ui.heading(&civ.name.clone());
                ui.label(format!("Stage: {}", civ.stage.name()));
                ui.label(format!("Age: {} years", civ.age_years));
                ui.label(format!("Population: {}", civ.population));
                ui.label(format!("Wealth: {:.0}", civ.wealth));
                ui.label(format!("Military: {}", civ.military_strength));
                ui.label(format!("Stability: {:.0}%", civ.stability * 100.0));
                ui.label(format!("Territory: {} cells", civ.territory_cells.len()));
                ui.separator();
                ui.label("Technology:");
                ui.label(format!("  Agriculture: {}", civ.tech.agriculture));
                ui.label(format!("  Metalworking: {}", civ.tech.metalworking));
                ui.label(format!("  Writing: {}", civ.tech.writing));
                ui.label(format!("  Military: {}", civ.tech.military));
                ui.label(format!("  Magic: {}", civ.tech.magic));
                ui.label(format!("  Total Tech: {}", civ.tech.total_tech_level()));
            });
        }
    }

    ui.separator();
    ui.heading("History Log");
    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
        for entry in state.history_log.iter().rev().take(50) {
            ui.label(entry);
        }
    });
}

// ============================================================
// EXPANSION 4: Heightmap Sculpting Tools
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SculptBrushShape {
    Circle,
    Square,
    Diamond,
    Feather,
    Rocky,
    Plateau,
    Ridge,
    Valley,
}

impl SculptBrushShape {
    pub fn name(&self) -> &'static str {
        match self {
            SculptBrushShape::Circle => "Circle",
            SculptBrushShape::Square => "Square",
            SculptBrushShape::Diamond => "Diamond",
            SculptBrushShape::Feather => "Feather",
            SculptBrushShape::Rocky => "Rocky",
            SculptBrushShape::Plateau => "Plateau",
            SculptBrushShape::Ridge => "Ridge",
            SculptBrushShape::Valley => "Valley",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SculptMode {
    Raise,
    Lower,
    Flatten,
    Smooth,
    Noise,
    Stamp,
    Erode,
    Paint,
}

impl SculptMode {
    pub fn name(&self) -> &'static str {
        match self {
            SculptMode::Raise => "Raise",
            SculptMode::Lower => "Lower",
            SculptMode::Flatten => "Flatten",
            SculptMode::Smooth => "Smooth",
            SculptMode::Noise => "Noise",
            SculptMode::Stamp => "Stamp",
            SculptMode::Erode => "Erode",
            SculptMode::Paint => "Paint",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SculptBrush {
    pub radius: f32,
    pub strength: f32,
    pub falloff: f32,
    pub shape: SculptBrushShape,
    pub mode: SculptMode,
    pub target_height: f32,
    pub noise_scale: f32,
    pub noise_seed: u64,
    pub stamp_pattern: Vec<Vec<f32>>,
}

impl Default for SculptBrush {
    fn default() -> Self {
        SculptBrush {
            radius: 10.0,
            strength: 0.05,
            falloff: 0.5,
            shape: SculptBrushShape::Circle,
            mode: SculptMode::Raise,
            target_height: 0.5,
            noise_scale: 0.1,
            noise_seed: 42,
            stamp_pattern: vec![],
        }
    }
}

impl SculptBrush {
    pub fn weight_at(&self, dx: f32, dy: f32) -> f32 {
        let dist = match self.shape {
            SculptBrushShape::Square => dx.abs().max(dy.abs()),
            SculptBrushShape::Diamond => dx.abs() + dy.abs(),
            _ => (dx * dx + dy * dy).sqrt(),
        };
        if dist >= self.radius { return 0.0; }
        let t = 1.0 - dist / self.radius;
        match self.shape {
            SculptBrushShape::Feather => t * t,
            SculptBrushShape::Plateau => if t > 0.3 { 1.0 } else { t / 0.3 },
            _ => {
                let e = (1.0 / (self.falloff + 0.001)).max(0.1).min(10.0);
                t.powf(e)
            }
        }
    }

    pub fn apply_to_heightmap(&self, heightmap: &mut Vec<Vec<f32>>,
                               cx: usize, cy: usize, rng: &mut SimpleRng) {
        let h = heightmap.len();
        if h == 0 { return; }
        let w = heightmap[0].len();
        let r = self.radius.ceil() as i32;

        let sum_neighborhood = if self.mode == SculptMode::Smooth || self.mode == SculptMode::Flatten {
            let mut sum = 0.0f32;
            let mut count = 0u32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                        sum += heightmap[ny as usize][nx as usize];
                        count += 1;
                    }
                }
            }
            if count > 0 { sum / count as f32 } else { 0.5 }
        } else { self.target_height };

        for dy in -r..=r {
            for dx in -r..=r {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 { continue; }
                let weight = self.weight_at(dx as f32, dy as f32);
                if weight <= 0.0 { continue; }
                let cell = &mut heightmap[ny as usize][nx as usize];
                match self.mode {
                    SculptMode::Raise => *cell = (*cell + self.strength * weight).min(1.0),
                    SculptMode::Lower => *cell = (*cell - self.strength * weight).max(0.0),
                    SculptMode::Flatten | SculptMode::Smooth => {
                        *cell = *cell + (sum_neighborhood - *cell) * self.strength * weight;
                    },
                    SculptMode::Noise => {
                        let noise = (rng.next_u64() as f32 / u64::MAX as f32) * 2.0 - 1.0;
                        *cell = (*cell + noise * self.strength * weight).clamp(0.0, 1.0);
                    },
                    SculptMode::Stamp => {
                        if !self.stamp_pattern.is_empty() {
                            let pi = (dx + r) as usize * self.stamp_pattern.len() / (2 * r as usize + 1);
                            let pj = (dy + r) as usize * self.stamp_pattern[0].len() / (2 * r as usize + 1);
                            if pi < self.stamp_pattern.len() && pj < self.stamp_pattern[0].len() {
                                let sv = self.stamp_pattern[pi][pj];
                                *cell = (*cell + sv * self.strength * weight).clamp(0.0, 1.0);
                            }
                        }
                    },
                    SculptMode::Erode => {
                        // Simple erosion: lower and redistribute to neighbors
                        let erode_amt = self.strength * weight * 0.5;
                        *cell = (*cell - erode_amt).max(0.0);
                    },
                    SculptMode::Paint => {
                        // No-op here (used for texture painting)
                    },
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeightmapSculptState {
    pub heightmap: Vec<Vec<f32>>,
    pub width: usize,
    pub height: usize,
    pub brush: SculptBrush,
    pub undo_stack: Vec<Vec<Vec<f32>>>,
    pub redo_stack: Vec<Vec<Vec<f32>>>,
    pub max_undo: usize,
    pub cursor_pos: Option<(usize, usize)>,
    pub zoom: f32,
    pub pan: (f32, f32),
    pub show_wireframe: bool,
    pub show_contours: bool,
    pub contour_interval: f32,
    pub colormap: HeightColormap,
    pub rng: SimpleRng,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum HeightColormap {
    Greyscale,
    Terrain,
    Ocean,
    Hot,
    Cool,
    Rainbow,
}

impl HeightColormap {
    pub fn sample(&self, t: f32) -> egui::Color32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            HeightColormap::Greyscale => {
                let v = (t * 255.0) as u8;
                egui::Color32::from_rgb(v, v, v)
            },
            HeightColormap::Terrain => {
                if t < 0.3 { // deep water
                    let u = t / 0.3;
                    egui::Color32::from_rgb((20.0 + u * 40.0) as u8, (80.0 + u * 60.0) as u8, (180.0 + u * 40.0) as u8)
                } else if t < 0.35 { // shore
                    egui::Color32::from_rgb(220, 210, 160)
                } else if t < 0.6 { // grass
                    let u = (t - 0.35) / 0.25;
                    egui::Color32::from_rgb((60.0 + u * 30.0) as u8, (140.0 - u * 20.0) as u8, 60)
                } else if t < 0.8 { // mountain
                    let u = (t - 0.6) / 0.2;
                    egui::Color32::from_rgb((100.0 + u * 60.0) as u8, (100.0 + u * 60.0) as u8, (80.0 + u * 60.0) as u8)
                } else { // snow
                    let u = (t - 0.8) / 0.2;
                    let v = (220.0 + u * 35.0) as u8;
                    egui::Color32::from_rgb(v, v, v)
                }
            },
            HeightColormap::Hot => {
                egui::Color32::from_rgb(
                    (t * 2.0 * 255.0).min(255.0) as u8,
                    ((t * 2.0 - 1.0).max(0.0) * 255.0) as u8,
                    0,
                )
            },
            HeightColormap::Cool => {
                egui::Color32::from_rgb(
                    (t * 255.0) as u8,
                    ((1.0 - t) * 200.0) as u8,
                    255,
                )
            },
            HeightColormap::Ocean => {
                egui::Color32::from_rgb(
                    0,
                    (50.0 + t * 100.0) as u8,
                    (150.0 + t * 100.0) as u8,
                )
            },
            HeightColormap::Rainbow => {
                let h = t * 300.0;
                let (r, g, b) = hsv_to_rgb_world(h, 1.0, 1.0);
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            },
        }
    }
}

pub fn hsv_to_rgb_world(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s == 0.0 { return (v, v, v); }
    let h = h % 360.0;
    let sector = (h / 60.0) as u32;
    let f = h / 60.0 - sector as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t2 = v * (1.0 - (1.0 - f) * s);
    match sector {
        0 => (v, t2, p),
        1 => (q, v, p),
        2 => (p, v, t2),
        3 => (p, q, v),
        4 => (t2, p, v),
        _ => (v, p, q),
    }
}

impl Default for HeightmapSculptState {
    fn default() -> Self {
        let w = 128usize;
        let h = 128usize;
        HeightmapSculptState {
            heightmap: vec![vec![0.5f32; w]; h],
            width: w,
            height: h,
            brush: SculptBrush::default(),
            undo_stack: vec![],
            redo_stack: vec![],
            max_undo: 20,
            cursor_pos: None,
            zoom: 4.0,
            pan: (0.0, 0.0),
            show_wireframe: false,
            show_contours: true,
            contour_interval: 0.1,
            colormap: HeightColormap::Terrain,
            rng: SimpleRng::new(77777),
        }
    }
}

impl HeightmapSculptState {
    pub fn push_undo(&mut self) {
        self.undo_stack.push(self.heightmap.clone());
        if self.undo_stack.len() > self.max_undo {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.heightmap.clone());
            self.heightmap = prev;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.heightmap.clone());
            self.heightmap = next;
        }
    }

    pub fn sculpt_at(&mut self, cx: usize, cy: usize) {
        let mut rng = SimpleRng::new(self.rng.next_u64());
        let brush = self.brush.clone();
        brush.apply_to_heightmap(&mut self.heightmap, cx, cy, &mut rng);
    }

    pub fn normalize(&mut self) {
        let min_h = self.heightmap.iter().flat_map(|r| r.iter()).cloned().fold(f32::MAX, f32::min);
        let max_h = self.heightmap.iter().flat_map(|r| r.iter()).cloned().fold(f32::MIN, f32::max);
        let range = max_h - min_h;
        if range > 0.0001 {
            for row in self.heightmap.iter_mut() {
                for cell in row.iter_mut() {
                    *cell = (*cell - min_h) / range;
                }
            }
        }
    }

    pub fn fill_flat(&mut self, height: f32) {
        for row in self.heightmap.iter_mut() {
            for cell in row.iter_mut() {
                *cell = height;
            }
        }
    }

    pub fn apply_fbm_noise(&mut self, scale: f32, octaves: u32, amplitude: f32) {
        let mut rng = SimpleRng::new(self.rng.next_u64());
        for y in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 * scale;
                let ny = y as f32 * scale;
                let mut val = 0.0f32;
                let mut amp = amplitude;
                let mut freq = 1.0f32;
                for _ in 0..octaves {
                    let noise = ((rng.next_u64() as f32 / u64::MAX as f32) * 2.0 - 1.0)
                        * (nx * freq).sin() * (ny * freq).cos();
                    val += noise * amp;
                    amp *= 0.5;
                    freq *= 2.0;
                }
                self.heightmap[y][x] = (self.heightmap[y][x] + val).clamp(0.0, 1.0);
            }
        }
    }

    pub fn resize(&mut self, new_w: usize, new_h: usize) {
        let mut new_map = vec![vec![0.5f32; new_w]; new_h];
        for y in 0..new_h {
            for x in 0..new_w {
                let src_x = (x as f32 / new_w as f32 * self.width as f32) as usize;
                let src_y = (y as f32 / new_h as f32 * self.height as f32) as usize;
                let sx = src_x.min(self.width - 1);
                let sy = src_y.min(self.height - 1);
                new_map[y][x] = self.heightmap[sy][sx];
            }
        }
        self.heightmap = new_map;
        self.width = new_w;
        self.height = new_h;
    }
}

pub fn show_heightmap_sculptor(ui: &mut egui::Ui, state: &mut HeightmapSculptState) {
    ui.horizontal(|ui| {
        ui.heading("Heightmap Sculptor");
        if ui.button("Undo").clicked() { state.undo(); }
        if ui.button("Redo").clicked() { state.redo(); }
        if ui.button("Normalize").clicked() { state.normalize(); }
        if ui.button("Fill Flat").clicked() { state.fill_flat(0.5); }
        if ui.button("Add fBm").clicked() {
            state.push_undo();
            state.apply_fbm_noise(0.05, 5, 0.2);
        }
    });

    egui::SidePanel::left("sculpt_tools").show_inside(ui, |ui| {
        ui.label("Brush Mode:");
        for mode in [SculptMode::Raise, SculptMode::Lower, SculptMode::Flatten,
                     SculptMode::Smooth, SculptMode::Noise, SculptMode::Erode] {
            if ui.selectable_label(state.brush.mode == mode, mode.name()).clicked() {
                state.brush.mode = mode;
            }
        }
        ui.separator();
        ui.label("Brush Shape:");
        for shape in [SculptBrushShape::Circle, SculptBrushShape::Square,
                      SculptBrushShape::Diamond, SculptBrushShape::Feather,
                      SculptBrushShape::Plateau, SculptBrushShape::Ridge] {
            if ui.selectable_label(state.brush.shape == shape, shape.name()).clicked() {
                state.brush.shape = shape;
            }
        }
        ui.separator();
        ui.add(egui::Slider::new(&mut state.brush.radius, 1.0..=50.0).text("Radius"));
        ui.add(egui::Slider::new(&mut state.brush.strength, 0.001..=0.5).text("Strength"));
        ui.add(egui::Slider::new(&mut state.brush.falloff, 0.01..=2.0).text("Falloff"));
        if state.brush.mode == SculptMode::Flatten {
            ui.add(egui::Slider::new(&mut state.brush.target_height, 0.0..=1.0).text("Target"));
        }
        ui.separator();
        ui.label("Colormap:");
        for cm in [HeightColormap::Terrain, HeightColormap::Greyscale,
                   HeightColormap::Hot, HeightColormap::Cool, HeightColormap::Rainbow] {
            if ui.selectable_label(state.colormap == cm, format!("{:?}", cm)).clicked() {
                state.colormap = cm;
            }
        }
        ui.checkbox(&mut state.show_contours, "Show Contours");
        ui.add(egui::Slider::new(&mut state.zoom, 1.0..=16.0).text("Zoom"));
    });

    let available = ui.available_rect_before_wrap();
    let painter = ui.painter_at(available);
    let cell_size = state.zoom;
    let ox = available.min.x + state.pan.0;
    let oy = available.min.y + state.pan.1;

    let vis_w = ((available.width() / cell_size) as usize).min(state.width);
    let vis_h = ((available.height() / cell_size) as usize).min(state.height);

    for py in 0..vis_h {
        for px in 0..vis_w {
            let h = state.heightmap[py][px];
            let color = state.colormap.sample(h);
            let rect = egui::Rect::from_min_size(
                egui::pos2(ox + px as f32 * cell_size, oy + py as f32 * cell_size),
                egui::vec2(cell_size, cell_size),
            );
            painter.rect_filled(rect, 0.0, color);

            if state.show_contours {
                let contour = (h / state.contour_interval).floor() * state.contour_interval;
                let next_contour = contour + state.contour_interval;
                if h > contour + state.contour_interval * 0.9 {
                    painter.rect_stroke(rect, 0.0, egui::Stroke::new(0.5, egui::Color32::from_rgba_premultiplied(0, 0, 0, 80)));
                }
            }
        }
    }

    if let Some(cursor) = state.cursor_pos {
        let cx = ox + cursor.0 as f32 * cell_size;
        let cy = oy + cursor.1 as f32 * cell_size;
        painter.circle_stroke(egui::pos2(cx, cy), state.brush.radius * cell_size,
            egui::Stroke::new(1.5, egui::Color32::WHITE));
    }
}

// ============================================================
// EXPANSION 4: World Export System
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldExportOptions {
    pub export_heightmap: bool,
    pub export_biomes: bool,
    pub export_rivers: bool,
    pub export_settlements: bool,
    pub export_political: bool,
    pub export_trade: bool,
    pub export_factions: bool,
    pub export_history: bool,
    pub export_quests: bool,
    pub format_json: bool,
    pub format_csv: bool,
    pub format_png: bool,
    pub format_svg: bool,
    pub resolution_scale: u32,
    pub compress_output: bool,
    pub output_path: String,
}

impl Default for WorldExportOptions {
    fn default() -> Self {
        WorldExportOptions {
            export_heightmap: true,
            export_biomes: true,
            export_rivers: false,
            export_settlements: true,
            export_political: false,
            export_trade: false,
            export_factions: false,
            export_history: false,
            export_quests: false,
            format_json: true,
            format_csv: false,
            format_png: true,
            format_svg: false,
            resolution_scale: 1,
            compress_output: false,
            output_path: "world_export".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldExportState {
    pub options: WorldExportOptions,
    pub log: Vec<String>,
    pub export_progress: f32,
    pub is_exporting: bool,
    pub last_export_path: Option<String>,
    pub estimated_size_mb: f32,
}

impl Default for WorldExportState {
    fn default() -> Self {
        WorldExportState {
            options: WorldExportOptions::default(),
            log: vec![],
            export_progress: 0.0,
            is_exporting: false,
            last_export_path: None,
            estimated_size_mb: 0.0,
        }
    }
}

impl WorldExportState {
    pub fn estimate_size(&mut self) {
        let mut mb = 0.0f32;
        if self.options.export_heightmap { mb += 0.5 * self.options.resolution_scale as f32; }
        if self.options.export_biomes { mb += 0.3; }
        if self.options.export_settlements { mb += 0.1; }
        if self.options.format_png { mb *= 0.2; }
        if self.options.compress_output { mb *= 0.3; }
        self.estimated_size_mb = mb;
    }

    pub fn add_log(&mut self, msg: &str) {
        self.log.push(msg.to_string());
        if self.log.len() > 100 { self.log.remove(0); }
    }

    pub fn simulate_export(&mut self) {
        self.is_exporting = true;
        self.export_progress = 0.0;
        self.add_log(&format!("Starting export to: {}", self.options.output_path));
        if self.options.export_heightmap { self.add_log("  Exporting heightmap..."); }
        if self.options.export_biomes { self.add_log("  Exporting biome map..."); }
        if self.options.export_settlements { self.add_log("  Exporting settlement data..."); }
        if self.options.format_json { self.add_log("  Writing JSON data..."); }
        if self.options.format_png { self.add_log("  Rendering PNG maps..."); }
        self.export_progress = 1.0;
        self.is_exporting = false;
        let path = self.options.output_path.clone();
        self.last_export_path = Some(path.clone());
        self.add_log(&format!("Export complete: {}", path));
    }
}

pub fn show_world_export(ui: &mut egui::Ui, state: &mut WorldExportState) {
    ui.heading("World Export");

    egui::CollapsingHeader::new("Export Options").default_open(true).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("Data Layers:");
                ui.checkbox(&mut state.options.export_heightmap, "Heightmap");
                ui.checkbox(&mut state.options.export_biomes, "Biomes");
                ui.checkbox(&mut state.options.export_rivers, "Rivers");
                ui.checkbox(&mut state.options.export_settlements, "Settlements");
                ui.checkbox(&mut state.options.export_political, "Political Map");
                ui.checkbox(&mut state.options.export_trade, "Trade Routes");
                ui.checkbox(&mut state.options.export_factions, "Factions");
                ui.checkbox(&mut state.options.export_history, "History");
                ui.checkbox(&mut state.options.export_quests, "Quests");
            });
            ui.vertical(|ui| {
                ui.label("Formats:");
                ui.checkbox(&mut state.options.format_json, "JSON");
                ui.checkbox(&mut state.options.format_csv, "CSV");
                ui.checkbox(&mut state.options.format_png, "PNG");
                ui.checkbox(&mut state.options.format_svg, "SVG");
                ui.separator();
                ui.label("Resolution:");
                ui.add(egui::Slider::new(&mut state.options.resolution_scale, 1..=4).text("Scale"));
                ui.checkbox(&mut state.options.compress_output, "Compress");
            });
        });
        ui.label("Output Path:");
        ui.text_edit_singleline(&mut state.options.output_path);
    });

    state.estimate_size();
    ui.label(format!("Estimated size: {:.1} MB", state.estimated_size_mb));

    if ui.button("Export World").clicked() {
        state.simulate_export();
    }

    if state.export_progress > 0.0 && state.export_progress < 1.0 {
        ui.add(egui::ProgressBar::new(state.export_progress).text("Exporting..."));
    }

    if let Some(path) = &state.last_export_path.clone() {
        ui.label(format!("Last export: {}", path));
    }

    ui.separator();
    ui.heading("Export Log");
    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
        for entry in &state.log {
            ui.label(entry);
        }
    });
}

// ============================================================
// EXPANSION 4: Minimap & Overview Systems
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinimapConfig {
    pub width: u32,
    pub height: u32,
    pub show_settlements: bool,
    pub show_rivers: bool,
    pub show_roads: bool,
    pub show_borders: bool,
    pub show_grid: bool,
    pub show_viewport: bool,
    pub opacity: f32,
    pub zoom_factor: f32,
    pub colormap: MinimapColorMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MinimapColorMode {
    Heightmap,
    Political,
    Biome,
    Temperature,
    Moisture,
    Faction,
}

impl Default for MinimapConfig {
    fn default() -> Self {
        MinimapConfig {
            width: 200,
            height: 200,
            show_settlements: true,
            show_rivers: true,
            show_roads: false,
            show_borders: true,
            show_grid: false,
            show_viewport: true,
            opacity: 1.0,
            zoom_factor: 1.0,
            colormap: MinimapColorMode::Heightmap,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct WorldOverviewState {
    pub minimap_cfg: MinimapConfig,
    pub current_viewport: (f32, f32, f32, f32),
    pub selected_region: Option<u32>,
    pub show_stats_overlay: bool,
    pub global_temperature: f32,
    pub global_moisture: f32,
    pub world_age_years: u64,
    pub total_settlements: u32,
    pub total_population: u64,
    pub num_factions: u32,
    pub num_active_wars: u32,
    pub num_trade_routes: u32,
}

impl WorldOverviewState {
    pub fn update_stats(&mut self,
        settlements: u32, population: u64, factions: u32,
        wars: u32, trade: u32) {
        self.total_settlements = settlements;
        self.total_population = population;
        self.num_factions = factions;
        self.num_active_wars = wars;
        self.num_trade_routes = trade;
    }
}

pub fn show_world_overview(ui: &mut egui::Ui, state: &mut WorldOverviewState) {
    ui.heading("World Overview");
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(format!("World Age: {} years", state.world_age_years));
            ui.label(format!("Settlements: {}", state.total_settlements));
            ui.label(format!("Population: {:.0}M", state.total_population as f64 / 1e6));
            ui.label(format!("Factions: {}", state.num_factions));
            ui.label(format!("Active Wars: {}", state.num_active_wars));
            ui.label(format!("Trade Routes: {}", state.num_trade_routes));
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label("Minimap:");
            ui.add(egui::Slider::new(&mut state.minimap_cfg.opacity, 0.0..=1.0).text("Opacity"));
            ui.checkbox(&mut state.minimap_cfg.show_settlements, "Settlements");
            ui.checkbox(&mut state.minimap_cfg.show_rivers, "Rivers");
            ui.checkbox(&mut state.minimap_cfg.show_borders, "Borders");
            ui.checkbox(&mut state.show_stats_overlay, "Stats Overlay");
        });
    });
}

// ============================================================
// EXPANSION 4: Volcanic & Geological Systems
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum VolcanoType {
    Shield,
    Stratovolcano,
    Caldera,
    CinderCone,
    Submarine,
    Supervolcano,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volcano {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub volcano_type: VolcanoType,
    pub elevation: f32,
    pub caldero_radius: f32,
    pub is_active: bool,
    pub last_eruption_year: Option<i64>,
    pub eruption_interval_years: u32,
    pub lava_viscosity: f32,
    pub ash_cloud_radius: f32,
    pub name: String,
    pub magma_chamber_depth: f32,
    pub temperature: f32,
}

impl Volcano {
    pub fn new_stratovolcano(id: u32, x: u32, y: u32, name: String) -> Self {
        Volcano {
            id, x, y,
            volcano_type: VolcanoType::Stratovolcano,
            elevation: 0.85,
            caldero_radius: 5.0,
            is_active: true,
            last_eruption_year: Some(-500),
            eruption_interval_years: 200,
            lava_viscosity: 0.8,
            ash_cloud_radius: 50.0,
            name,
            magma_chamber_depth: 5000.0,
            temperature: 1200.0,
        }
    }

    pub fn erupt(&self, heightmap: &mut Vec<Vec<f32>>, rng: &mut SimpleRng) {
        let hw = heightmap.len();
        if hw == 0 { return; }
        let ww = heightmap[0].len();
        let r = self.caldero_radius as i32 * 3;
        for dy in -r..=r {
            for dx in -r..=r {
                let nx = self.x as i32 + dx;
                let ny = self.y as i32 + dy;
                if nx < 0 || nx >= ww as i32 || ny < 0 || ny >= hw as i32 { continue; }
                let dist = ((dx*dx + dy*dy) as f32).sqrt();
                if dist <= self.caldero_radius {
                    // Caldera collapse
                    heightmap[ny as usize][nx as usize] *= 0.9;
                } else if dist <= self.caldero_radius * 3.0 {
                    // Lava flow deposit
                    let lava = (1.0 - dist / (self.caldero_radius * 3.0)) * 0.1;
                    let noise = (rng.next_u64() as f32 / u64::MAX as f32) * 0.05;
                    heightmap[ny as usize][nx as usize] =
                        (heightmap[ny as usize][nx as usize] + lava + noise).min(1.0);
                }
            }
        }
    }

    pub fn years_until_next_eruption(&self, current_year: i64) -> Option<i64> {
        self.last_eruption_year.map(|last| {
            let next = last + self.eruption_interval_years as i64;
            next - current_year
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeologicalFeature {
    pub id: u32,
    pub name: String,
    pub feature_type: GeoFeatureType,
    pub x: u32,
    pub y: u32,
    pub radius: f32,
    pub age_years: u64,
    pub notes: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GeoFeatureType {
    Volcano(VolcanoType),
    HotSpring,
    Geyser,
    CrystalCave,
    MineralDeposit,
    OilField,
    GeothermalVent,
    Fault,
    Glacier,
    Sinkhole,
    NaturalArch,
    SeaCave,
}

impl GeoFeatureType {
    pub fn name(&self) -> &str {
        match self {
            GeoFeatureType::Volcano(_) => "Volcano",
            GeoFeatureType::HotSpring => "Hot Spring",
            GeoFeatureType::Geyser => "Geyser",
            GeoFeatureType::CrystalCave => "Crystal Cave",
            GeoFeatureType::MineralDeposit => "Mineral Deposit",
            GeoFeatureType::OilField => "Oil Field",
            GeoFeatureType::GeothermalVent => "Geothermal Vent",
            GeoFeatureType::Fault => "Fault Line",
            GeoFeatureType::Glacier => "Glacier",
            GeoFeatureType::Sinkhole => "Sinkhole",
            GeoFeatureType::NaturalArch => "Natural Arch",
            GeoFeatureType::SeaCave => "Sea Cave",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct VolcanicSystemState {
    pub volcanoes: Vec<Volcano>,
    pub geo_features: Vec<GeologicalFeature>,
    pub current_year: i64,
    pub selected_volcano: Option<u32>,
    pub show_eruption_zones: bool,
    pub show_ash_clouds: bool,
    pub heightmap_width: u32,
    pub heightmap_height: u32,
}

impl VolcanicSystemState {
    pub fn add_volcano(&mut self, v: Volcano) { self.volcanoes.push(v); }

    pub fn simulate_year(&mut self, heightmap: &mut Vec<Vec<f32>>, rng: &mut SimpleRng) {
        self.current_year += 1;
        for v in self.volcanoes.iter() {
            if !v.is_active { continue; }
            if let Some(last) = v.last_eruption_year {
                let years_since = self.current_year - last;
                if years_since >= v.eruption_interval_years as i64 {
                    if rng.next_u64() % 100 < 30 {
                        v.erupt(heightmap, rng);
                    }
                }
            }
        }
    }
}

pub fn show_volcanic_system(ui: &mut egui::Ui, state: &mut VolcanicSystemState) {
    ui.heading("Volcanic & Geological Systems");

    ui.horizontal(|ui| {
        ui.label(format!("Year: {}", state.current_year));
        ui.label(format!("Volcanoes: {}", state.volcanoes.len()));
        ui.label(format!("Geo Features: {}", state.geo_features.len()));
        ui.checkbox(&mut state.show_eruption_zones, "Eruption Zones");
        ui.checkbox(&mut state.show_ash_clouds, "Ash Clouds");
    });

    egui::ScrollArea::vertical().show(ui, |ui| {
        for v in &state.volcanoes {
            let active_str = if v.is_active { "ACTIVE" } else { "dormant" };
            egui::CollapsingHeader::new(format!("{} ({}) [{}]", v.name,
                match &v.volcano_type {
                    VolcanoType::Stratovolcano => "Stratovolcano",
                    VolcanoType::Shield => "Shield",
                    VolcanoType::Caldera => "Caldera",
                    VolcanoType::CinderCone => "Cinder Cone",
                    VolcanoType::Submarine => "Submarine",
                    VolcanoType::Supervolcano => "Supervolcano",
                }, active_str))
            .show(ui, |ui| {
                ui.label(format!("Position: ({}, {})", v.x, v.y));
                ui.label(format!("Elevation: {:.2}", v.elevation));
                ui.label(format!("Caldera Radius: {:.1}", v.caldero_radius));
                ui.label(format!("Ash Cloud Radius: {:.1}", v.ash_cloud_radius));
                ui.label(format!("Eruption Interval: {} years", v.eruption_interval_years));
                if let Some(last) = v.last_eruption_year {
                    ui.label(format!("Last Eruption: year {}", last));
                    let until = v.years_until_next_eruption(state.current_year).unwrap_or(0);
                    if until <= 0 {
                        ui.colored_label(egui::Color32::RED, "OVERDUE FOR ERUPTION!");
                    } else {
                        ui.label(format!("Next Eruption: in ~{} years", until));
                    }
                }
            });
        }
    });
}

// ============================================================
// EXPANSION 4: Ocean & Water Bodies
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum WaterBodyType {
    Ocean,
    Sea,
    Bay,
    Gulf,
    Lake,
    River,
    Pond,
    Swamp,
    Estuary,
    Lagoon,
    Fjord,
    Strait,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaterBody {
    pub id: u32,
    pub name: String,
    pub body_type: WaterBodyType,
    pub area_sq_km: f64,
    pub max_depth_m: f32,
    pub avg_depth_m: f32,
    pub salinity: f32,
    pub temperature_c: f32,
    pub has_currents: bool,
    pub current_direction_deg: f32,
    pub current_speed_ms: f32,
    pub tidal_range_m: f32,
    pub is_navigable: bool,
    pub connected_to: Vec<u32>,
    pub fish_density: f32,
    pub pollution_level: f32,
}

impl WaterBody {
    pub fn new_ocean(id: u32, name: String) -> Self {
        WaterBody {
            id, name,
            body_type: WaterBodyType::Ocean,
            area_sq_km: 100_000_000.0,
            max_depth_m: 11000.0,
            avg_depth_m: 3600.0,
            salinity: 35.0,
            temperature_c: 4.0,
            has_currents: true,
            current_direction_deg: 45.0,
            current_speed_ms: 0.5,
            tidal_range_m: 1.5,
            is_navigable: true,
            connected_to: vec![],
            fish_density: 0.6,
            pollution_level: 0.1,
        }
    }

    pub fn new_lake(id: u32, name: String, area: f64) -> Self {
        WaterBody {
            id, name,
            body_type: WaterBodyType::Lake,
            area_sq_km: area,
            max_depth_m: 50.0,
            avg_depth_m: 20.0,
            salinity: 0.1,
            temperature_c: 15.0,
            has_currents: false,
            current_direction_deg: 0.0,
            current_speed_ms: 0.0,
            tidal_range_m: 0.01,
            is_navigable: area > 1.0,
            connected_to: vec![],
            fish_density: 0.8,
            pollution_level: 0.0,
        }
    }

    pub fn volume_cubic_km(&self) -> f64 {
        self.area_sq_km * self.avg_depth_m as f64 / 1_000_000.0
    }

    pub fn is_saltwater(&self) -> bool { self.salinity > 5.0 }

    pub fn ecosystem_health(&self) -> f32 {
        (self.fish_density * (1.0 - self.pollution_level)).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OceanSystemState {
    pub water_bodies: Vec<WaterBody>,
    pub selected_body: Option<u32>,
    pub global_sea_level: f32,
    pub sea_level_change_rate: f32,
    pub show_currents: bool,
    pub show_temperature: bool,
    pub show_salinity: bool,
}

impl OceanSystemState {
    pub fn total_water_area(&self) -> f64 {
        self.water_bodies.iter().map(|w| w.area_sq_km).sum()
    }
    pub fn total_volume(&self) -> f64 {
        self.water_bodies.iter().map(|w| w.volume_cubic_km()).sum()
    }
}

pub fn show_ocean_system(ui: &mut egui::Ui, state: &mut OceanSystemState) {
    ui.heading("Ocean & Water Bodies");
    ui.label(format!("Sea Level: {:.1}m | Change: {:.3}m/yr",
        state.global_sea_level, state.sea_level_change_rate));
    ui.label(format!("Total Water Area: {:.0} sq km", state.total_water_area()));
    ui.label(format!("Total Volume: {:.0} cubic km", state.total_volume()));

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_currents, "Currents");
        ui.checkbox(&mut state.show_temperature, "Temperature");
        ui.checkbox(&mut state.show_salinity, "Salinity");
    });

    egui::ScrollArea::vertical().show(ui, |ui| {
        for wb in &state.water_bodies {
            egui::CollapsingHeader::new(format!("{} ({:?})", wb.name, wb.body_type))
                .show(ui, |ui| {
                    ui.label(format!("Area: {:.0} sq km", wb.area_sq_km));
                    ui.label(format!("Depth: {:.0}m avg / {:.0}m max", wb.avg_depth_m, wb.max_depth_m));
                    ui.label(format!("Salinity: {:.1}‰", wb.salinity));
                    ui.label(format!("Temperature: {:.1}°C", wb.temperature_c));
                    ui.label(format!("Ecosystem Health: {:.0}%", wb.ecosystem_health() * 100.0));
                    if wb.is_navigable { ui.label("Navigable"); }
                });
        }
    });
}

// ============================================================
// EXPANSION 4: World Gen Tests
// ============================================================

#[cfg(test)]
mod world_gen_expansion4_tests {
    use super::*;

    #[test]
    fn test_dungeon_room_overlap() {
        let r1 = DungeonRoom::new(0, 0, 0, 10, 10, DungeonRoomType::Empty);
        let r2 = DungeonRoom::new(1, 5, 5, 10, 10, DungeonRoomType::Empty);
        let r3 = DungeonRoom::new(2, 20, 20, 5, 5, DungeonRoomType::Empty);
        assert!(r1.overlaps(&r2));
        assert!(!r1.overlaps(&r3));
    }

    #[test]
    fn test_dungeon_generation() {
        let cfg = DungeonGenConfig { seed: 42, levels: 2, ..DungeonGenConfig::default() };
        let levels = generate_dungeon(&cfg);
        assert_eq!(levels.len(), 2);
        assert!(!levels[0].rooms.is_empty());
    }

    #[test]
    fn test_civ_stage_progression() {
        let mut civ = Civilization::new(0, "Test".to_string(), (50, 50), [255, 0, 0]);
        let mut rng = SimpleRng::new(1);
        for _ in 0..1000 {
            civ.simulate_year(&mut rng);
        }
        assert!(civ.age_years == 1000);
        assert!(civ.population > 100);
    }

    #[test]
    fn test_tech_tree_advance() {
        let mut tech = TechTree::default();
        let mut rng = SimpleRng::new(999);
        for _ in 0..100 {
            tech.advance("agriculture", &mut rng);
        }
        assert!(tech.agriculture > 1 && tech.agriculture <= 10);
    }

    #[test]
    fn test_sculpt_brush_weight() {
        let brush = SculptBrush { radius: 10.0, ..SculptBrush::default() };
        assert_eq!(brush.weight_at(0.0, 0.0), 1.0);
        assert_eq!(brush.weight_at(10.0, 0.0), 0.0);
        assert!(brush.weight_at(5.0, 0.0) > 0.0);
    }

    #[test]
    fn test_heightmap_sculpt() {
        let mut state = HeightmapSculptState::default();
        state.fill_flat(0.5);
        state.push_undo();
        state.sculpt_at(64, 64);
        state.undo();
        assert!((state.heightmap[64][64] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_water_body_volume() {
        let wb = WaterBody::new_lake(0, "Test Lake".to_string(), 100.0);
        assert!(wb.volume_cubic_km() > 0.0);
        assert!(!wb.is_saltwater());
    }

    #[test]
    fn test_volcano_eruption() {
        let v = Volcano::new_stratovolcano(0, 50, 50, "Mount Test".to_string());
        let mut heightmap = vec![vec![0.5f32; 100]; 100];
        let mut rng = SimpleRng::new(42);
        v.erupt(&mut heightmap, &mut rng);
        // Caldera area should be reduced
        assert!(heightmap[50][50] < 0.5);
    }

    #[test]
    fn test_heightmap_colormap() {
        let cm = HeightColormap::Terrain;
        let deep = cm.sample(0.1);
        let land = cm.sample(0.5);
        let snow = cm.sample(0.9);
        // Deep water should be bluish
        assert!(deep.b() > deep.r());
        // Snow should be bright
        assert!(snow.r() > 180);
    }
}
'''

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'a', encoding='utf-8') as f:
    f.write(code)

import os
size = os.path.getsize(r'C:\proof-engine\editor\src\world_gen.rs')
print(f"world_gen.rs size: {size} bytes")
