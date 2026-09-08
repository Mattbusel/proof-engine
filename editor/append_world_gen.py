
code = r'''

# =================================================================
# CITY GENERATION SYSTEM
# =================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CityRoadStyle {
    Grid,
    Organic,
    Radial,
    Medieval,
}

impl CityRoadStyle {
    pub fn name(&self) -> &str {
        match self {
            CityRoadStyle::Grid => "Grid",
            CityRoadStyle::Organic => "Organic",
            CityRoadStyle::Radial => "Radial",
            CityRoadStyle::Medieval => "Medieval",
        }
    }
    pub fn all() -> &'static [CityRoadStyle] {
        &[CityRoadStyle::Grid, CityRoadStyle::Organic, CityRoadStyle::Radial, CityRoadStyle::Medieval]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LandUse {
    Residential,
    Commercial,
    Industrial,
    Park,
    Port,
    Historic,
}

impl LandUse {
    pub fn name(&self) -> &str {
        match self {
            LandUse::Residential => "Residential",
            LandUse::Commercial  => "Commercial",
            LandUse::Industrial  => "Industrial",
            LandUse::Park        => "Park",
            LandUse::Port        => "Port",
            LandUse::Historic    => "Historic",
        }
    }
    pub fn default_color(&self) -> Color32 {
        match self {
            LandUse::Residential => Color32::from_rgb(210, 170, 130),
            LandUse::Commercial  => Color32::from_rgb(120, 180, 240),
            LandUse::Industrial  => Color32::from_rgb(180, 150, 100),
            LandUse::Park        => Color32::from_rgb(80, 180, 80),
            LandUse::Port        => Color32::from_rgb(60, 120, 200),
            LandUse::Historic    => Color32::from_rgb(220, 200, 140),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistrictType {
    pub name: String,
    pub land_use: LandUse,
    pub density: f32,
    pub building_height_range: (f32, f32),
    pub color: Color32,
}

impl DistrictType {
    pub fn residential() -> Self { Self { name: "Residential".to_string(), land_use: LandUse::Residential, density: 0.4, building_height_range: (3.0, 12.0), color: Color32::from_rgb(210, 170, 130) } }
    pub fn commercial()  -> Self { Self { name: "Commercial".to_string(),  land_use: LandUse::Commercial,  density: 0.7, building_height_range: (8.0, 60.0), color: Color32::from_rgb(120, 180, 240) } }
    pub fn industrial()  -> Self { Self { name: "Industrial".to_string(),  land_use: LandUse::Industrial,  density: 0.5, building_height_range: (4.0, 20.0), color: Color32::from_rgb(180, 150, 100) } }
    pub fn park()        -> Self { Self { name: "Park".to_string(),        land_use: LandUse::Park,        density: 0.05, building_height_range: (0.0, 5.0), color: Color32::from_rgb(80, 180, 80)   } }
    pub fn historic()    -> Self { Self { name: "Historic".to_string(),    land_use: LandUse::Historic,    density: 0.6, building_height_range: (5.0, 25.0), color: Color32::from_rgb(220, 200, 140) } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CityConfig {
    pub population_density: f32,
    pub road_style: CityRoadStyle,
    pub block_size: f32,
    pub district_types: Vec<DistrictType>,
    pub landmark_count: u32,
    pub city_radius: f32,
    pub center: [f32; 2],
    pub seed: u64,
}

impl Default for CityConfig {
    fn default() -> Self {
        Self {
            population_density: 0.5,
            road_style: CityRoadStyle::Grid,
            block_size: 60.0,
            district_types: vec![DistrictType::residential(), DistrictType::commercial(), DistrictType::industrial(), DistrictType::park(), DistrictType::historic()],
            landmark_count: 5,
            city_radius: 400.0,
            center: [0.0, 0.0],
            seed: 12345,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CityRoadType { Highway, Main, Secondary, Alley }

impl CityRoadType {
    pub fn name(&self) -> &str {
        match self { CityRoadType::Highway => "Highway", CityRoadType::Main => "Main", CityRoadType::Secondary => "Secondary", CityRoadType::Alley => "Alley" }
    }
    pub fn default_width(&self) -> f32 {
        match self { CityRoadType::Highway => 12.0, CityRoadType::Main => 8.0, CityRoadType::Secondary => 5.0, CityRoadType::Alley => 2.5 }
    }
    pub fn color(&self) -> Color32 {
        match self {
            CityRoadType::Highway   => Color32::from_rgb(60, 60, 60),
            CityRoadType::Main      => Color32::from_rgb(100, 100, 100),
            CityRoadType::Secondary => Color32::from_rgb(140, 140, 140),
            CityRoadType::Alley     => Color32::from_rgb(180, 180, 180),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CityRoad {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub road_type: CityRoadType,
    pub width: f32,
}

impl CityRoad {
    pub fn new(start: [f32; 2], end: [f32; 2], road_type: CityRoadType) -> Self {
        let width = road_type.default_width();
        Self { start, end, road_type, width }
    }
    pub fn length(&self) -> f32 {
        let dx = self.end[0] - self.start[0]; let dy = self.end[1] - self.start[1];
        (dx*dx + dy*dy).sqrt()
    }
    pub fn midpoint(&self) -> [f32; 2] {
        [(self.start[0]+self.end[0])*0.5, (self.start[1]+self.end[1])*0.5]
    }
    pub fn direction(&self) -> [f32; 2] {
        let len = self.length().max(0.001);
        [(self.end[0]-self.start[0])/len, (self.end[1]-self.start[1])/len]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BuildingType {
    House, Apartment, Office, Shop, Warehouse, Church, Castle, Tower,
    Market, Tavern, Blacksmith, Temple, Library, Barracks, Palace,
}

impl BuildingType {
    pub fn name(&self) -> &str {
        match self {
            BuildingType::House => "House", BuildingType::Apartment => "Apartment",
            BuildingType::Office => "Office", BuildingType::Shop => "Shop",
            BuildingType::Warehouse => "Warehouse", BuildingType::Church => "Church",
            BuildingType::Castle => "Castle", BuildingType::Tower => "Tower",
            BuildingType::Market => "Market", BuildingType::Tavern => "Tavern",
            BuildingType::Blacksmith => "Blacksmith", BuildingType::Temple => "Temple",
            BuildingType::Library => "Library", BuildingType::Barracks => "Barracks",
            BuildingType::Palace => "Palace",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            BuildingType::House      => Color32::from_rgb(200, 160, 120),
            BuildingType::Apartment  => Color32::from_rgb(180, 140, 100),
            BuildingType::Office     => Color32::from_rgb(100, 140, 200),
            BuildingType::Shop       => Color32::from_rgb(220, 180, 80),
            BuildingType::Warehouse  => Color32::from_rgb(140, 120, 90),
            BuildingType::Church     => Color32::from_rgb(240, 230, 200),
            BuildingType::Castle     => Color32::from_rgb(150, 130, 110),
            BuildingType::Tower      => Color32::from_rgb(160, 140, 120),
            BuildingType::Market     => Color32::from_rgb(240, 200, 100),
            BuildingType::Tavern     => Color32::from_rgb(200, 140, 80),
            BuildingType::Blacksmith => Color32::from_rgb(100, 90, 80),
            BuildingType::Temple     => Color32::from_rgb(220, 210, 180),
            BuildingType::Library    => Color32::from_rgb(160, 180, 200),
            BuildingType::Barracks   => Color32::from_rgb(120, 140, 100),
            BuildingType::Palace     => Color32::from_rgb(230, 200, 150),
        }
    }
    pub fn for_land_use(land_use: &LandUse, rng: &mut SimpleRng) -> BuildingType {
        match land_use {
            LandUse::Residential => { if rng.next_f32() < 0.6 { BuildingType::House } else { BuildingType::Apartment } }
            LandUse::Commercial => {
                let v = rng.next_f32();
                if v < 0.4 { BuildingType::Shop } else if v < 0.7 { BuildingType::Office } else if v < 0.85 { BuildingType::Market } else { BuildingType::Tavern }
            }
            LandUse::Industrial  => { if rng.next_f32() < 0.5 { BuildingType::Warehouse } else { BuildingType::Blacksmith } }
            LandUse::Park        => BuildingType::Market,
            LandUse::Port        => BuildingType::Warehouse,
            LandUse::Historic => {
                let v = rng.next_f32();
                if v < 0.3 { BuildingType::Church } else if v < 0.5 { BuildingType::Castle } else if v < 0.7 { BuildingType::Temple } else if v < 0.85 { BuildingType::Library } else { BuildingType::Palace }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CityBuilding {
    pub position: [f32; 2],
    pub footprint: Vec<[f32; 2]>,
    pub height: f32,
    pub building_type: BuildingType,
}

impl CityBuilding {
    pub fn new_rect(cx: f32, cy: f32, w: f32, h: f32, height: f32, building_type: BuildingType) -> Self {
        let (hw, hh) = (w*0.5, h*0.5);
        Self { position: [cx,cy], footprint: vec![[cx-hw,cy-hh],[cx+hw,cy-hh],[cx+hw,cy+hh],[cx-hw,cy+hh]], height, building_type }
    }
    pub fn bounds_rect(&self) -> Rect {
        let (mut mnx, mut mny, mut mxx, mut mxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &self.footprint { mnx=mnx.min(p[0]); mny=mny.min(p[1]); mxx=mxx.max(p[0]); mxy=mxy.max(p[1]); }
        Rect::from_min_max(Pos2::new(mnx,mny), Pos2::new(mxx,mxy))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CityBlock {
    pub bounds: Vec<[f32; 2]>,
    pub district: usize,
    pub buildings: Vec<CityBuilding>,
}

impl CityBlock {
    pub fn new(bounds: Vec<[f32; 2]>, district: usize) -> Self { Self { bounds, district, buildings: Vec::new() } }
    pub fn area(&self) -> f32 {
        let n = self.bounds.len(); if n < 3 { return 0.0; }
        let mut a = 0.0_f32;
        for i in 0..n { let j = (i+1)%n; a += self.bounds[i][0]*self.bounds[j][1] - self.bounds[j][0]*self.bounds[i][1]; }
        (a*0.5).abs()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CityLayout {
    pub roads: Vec<CityRoad>,
    pub blocks: Vec<CityBlock>,
    pub landmarks: Vec<([f32;2], BuildingType)>,
    pub center: [f32;2],
    pub bounds: [f32;4],
}

impl CityLayout {
    pub fn new(center: [f32;2]) -> Self {
        Self { roads: Vec::new(), blocks: Vec::new(), landmarks: Vec::new(), center, bounds: [center[0]-500.0, center[1]-500.0, center[0]+500.0, center[1]+500.0] }
    }
}

fn gen_grid_roads(config: &CityConfig, layout: &mut CityLayout, rng: &mut SimpleRng) {
    let (cx, cy, r, block) = (config.center[0], config.center[1], config.city_radius, config.block_size);
    let cols = ((r*2.0)/block) as i32 + 2;
    let rows = ((r*2.0)/block) as i32 + 2;
    let (ox, oy) = (cx - cols as f32*block*0.5, cy - rows as f32*block*0.5);
    for col in 0..=cols {
        let x = ox + col as f32*block;
        if (x-cx).abs() > r+block { continue; }
        let rt = if col%4==0 { CityRoadType::Main } else { CityRoadType::Secondary };
        let w = if config.road_style == CityRoadStyle::Grid { 0.0 } else { (rng.next_f32()-0.5)*block*0.1 };
        layout.roads.push(CityRoad::new([x+w, oy], [x+w, oy+rows as f32*block], rt));
    }
    for row in 0..=rows {
        let y = oy + row as f32*block;
        if (y-cy).abs() > r+block { continue; }
        let rt = if row%4==0 { CityRoadType::Main } else { CityRoadType::Secondary };
        let w = if config.road_style == CityRoadStyle::Grid { 0.0 } else { (rng.next_f32()-0.5)*block*0.1 };
        layout.roads.push(CityRoad::new([ox, y+w], [ox+cols as f32*block, y+w], rt));
    }
    if config.population_density > 0.6 {
        for col in 0..cols { for row in 0..rows {
            let x = ox+col as f32*block+block*0.5;
            let (y0, y1) = (oy+row as f32*block, oy+(row+1) as f32*block);
            if (x-cx).abs() < r && rng.next_f32() < 0.3 {
                layout.roads.push(CityRoad::new([x,y0], [x,y1], CityRoadType::Alley));
            }
        }}
    }
}

fn gen_radial_roads(config: &CityConfig, layout: &mut CityLayout, rng: &mut SimpleRng) {
    let (cx, cy, r) = (config.center[0], config.center[1], config.city_radius);
    let spoke_count = 8 + (config.population_density*6.0) as u32;
    for i in 0..spoke_count {
        let angle = (i as f32 / spoke_count as f32) * std::f32::consts::TAU;
        let rt = if i%2==0 { CityRoadType::Main } else { CityRoadType::Secondary };
        layout.roads.push(CityRoad::new([cx,cy], [cx+angle.cos()*r, cy+angle.sin()*r], rt));
    }
    let ring_count = 3 + (r/100.0) as u32;
    for ring in 1..=ring_count {
        let rr = r*ring as f32/ring_count as f32;
        let segs = (rr*std::f32::consts::TAU/config.block_size) as u32 + 6;
        for i in 0..segs {
            let (a0, a1) = ((i as f32/segs as f32)*std::f32::consts::TAU, ((i+1) as f32/segs as f32)*std::f32::consts::TAU);
            let w = (rng.next_f32()-0.5)*rr*0.05;
            layout.roads.push(CityRoad::new([cx+a0.cos()*(rr+w), cy+a0.sin()*(rr+w)], [cx+a1.cos()*(rr+w), cy+a1.sin()*(rr+w)], CityRoadType::Secondary));
        }
    }
}

fn gen_organic_roads(config: &CityConfig, layout: &mut CityLayout, rng: &mut SimpleRng) {
    let (cx, cy, r) = (config.center[0], config.center[1], config.city_radius);
    let growth_count = (r/config.block_size) as u32 * 4;
    let mut seeds: Vec<([f32;2], f32)> = vec![([cx,cy], 0.0)];
    for _ in 0..growth_count {
        let angle = rng.next_f32() * std::f32::consts::TAU;
        let dist = rng.next_f32() * r;
        seeds.push(([cx+angle.cos()*dist, cy+angle.sin()*dist], angle));
    }
    for i in 0..seeds.len().min(40) {
        let (start, base_angle) = seeds[i];
        let angle = base_angle + (rng.next_f32()-0.5)*0.8;
        let length = config.block_size * (0.8 + rng.next_f32()*1.2);
        let end = [start[0]+angle.cos()*length, start[1]+angle.sin()*length];
        let rt = if i < 4 { CityRoadType::Main } else { CityRoadType::Secondary };
        layout.roads.push(CityRoad::new(start, end, rt));
    }
}

fn gen_medieval_roads(config: &CityConfig, layout: &mut CityLayout, rng: &mut SimpleRng) {
    let (cx, cy, r) = (config.center[0], config.center[1], config.city_radius);
    let mut frontier: Vec<([f32;2], f32, u32)> = vec![([cx,cy], 0.0, 0)];
    let max_depth = 4u32;
    while let Some((pos, angle, depth)) = frontier.pop() {
        if depth >= max_depth { continue; }
        let branch_count = if depth == 0 { 5 } else { 1 + (rng.next_f32()*2.0) as u32 };
        for b in 0..branch_count {
            let spread = std::f32::consts::TAU / branch_count as f32;
            let ang = angle + b as f32*spread + (rng.next_f32()-0.5)*spread*0.5;
            let len = config.block_size * (0.6 + rng.next_f32()*0.8);
            let end = [pos[0]+ang.cos()*len, pos[1]+ang.sin()*len];
            let dist_from_center = ((end[0]-cx).powi(2)+(end[1]-cy).powi(2)).sqrt();
            if dist_from_center > r { continue; }
            let rt = if depth==0 { CityRoadType::Main } else { CityRoadType::Secondary };
            layout.roads.push(CityRoad::new(pos, end, rt));
            if rng.next_f32() < 0.6 { frontier.push((end, ang, depth+1)); }
        }
    }
    let sq = config.block_size * 1.5;
    for side in 0..4 {
        let (a, na) = (side as f32 * std::f32::consts::FRAC_PI_2, (side as f32+1.0) * std::f32::consts::FRAC_PI_2);
        layout.roads.push(CityRoad::new([cx+a.cos()*sq, cy+a.sin()*sq], [cx+na.cos()*sq, cy+na.sin()*sq], CityRoadType::Main));
    }
}

fn subdivide_city_blocks(config: &CityConfig, layout: &mut CityLayout, rng: &mut SimpleRng) {
    let (cx, cy, r, block) = (config.center[0], config.center[1], config.city_radius, config.block_size);
    let (cols, rows) = (((r*2.0)/block) as i32+1, ((r*2.0)/block) as i32+1);
    let (ox, oy) = (cx-cols as f32*block*0.5, cy-rows as f32*block*0.5);
    for row in 0..rows { for col in 0..cols {
        let (bx, by) = (ox+col as f32*block, oy+row as f32*block);
        let (bcx, bcy) = (bx+block*0.5, by+block*0.5);
        let dist = ((bcx-cx).powi(2)+(bcy-cy).powi(2)).sqrt();
        if dist > r { continue; }
        let angle = (bcy-cy).atan2(bcx-cx);
        let nd = dist/r;
        let di = if nd<0.15 { 1 } else if nd<0.3 { 4 } else if nd<0.6 { 0 } else if angle.sin()>0.7 && nd>0.5 { 3 } else if nd>0.7 { 2 } else { 0 };
        let m = block*0.05;
        let bounds = vec![[bx+m,by+m],[bx+block-m,by+m],[bx+block-m,by+block-m],[bx+m,by+block-m]];
        let district_idx = di.min(config.district_types.len().saturating_sub(1));
        let mut cb = CityBlock::new(bounds, district_idx);
        let district = config.district_types[cb.district].clone();
        place_buildings_in_city_block(&mut cb, &district, rng);
        layout.blocks.push(cb);
    }}
}

fn place_buildings_in_city_block(block: &mut CityBlock, district: &DistrictType, rng: &mut SimpleRng) {
    if block.bounds.len() < 4 { return; }
    let (mnx, mny) = (block.bounds.iter().map(|p|p[0]).fold(f32::MAX,f32::min), block.bounds.iter().map(|p|p[1]).fold(f32::MAX,f32::min));
    let (mxx, mxy) = (block.bounds.iter().map(|p|p[0]).fold(f32::MIN,f32::max), block.bounds.iter().map(|p|p[1]).fold(f32::MIN,f32::max));
    let (bw, bh) = (mxx-mnx, mxy-mny);
    let setback = 4.0; let min_bs = 6.0;
    let max_bs = (bw*0.4).min(bh*0.4).max(min_bs);
    let bc = ((district.density*bw*bh/400.0) as u32+1).min(12);
    for _ in 0..bc {
        let (w, h) = (min_bs+rng.next_f32()*(max_bs-min_bs), min_bs+rng.next_f32()*(max_bs-min_bs));
        let bx = mnx+setback+rng.next_f32()*(bw-w-setback*2.0).max(0.1);
        let by = mny+setback+rng.next_f32()*(bh-h-setback*2.0).max(0.1);
        let height = district.building_height_range.0 + rng.next_f32()*(district.building_height_range.1-district.building_height_range.0);
        let btype = BuildingType::for_land_use(&district.land_use, rng);
        block.buildings.push(CityBuilding::new_rect(bx+w*0.5, by+h*0.5, w, h, height, btype));
    }
}

fn place_city_landmarks(config: &CityConfig, layout: &mut CityLayout, rng: &mut SimpleRng) {
    let (cx, cy, r) = (config.center[0], config.center[1], config.city_radius);
    layout.landmarks.push(([cx,cy], BuildingType::Palace));
    for i in 1..config.landmark_count {
        let angle = (i as f32 / config.landmark_count as f32) * std::f32::consts::TAU;
        let dist = r * (0.2 + rng.next_f32()*0.5);
        let pos = [cx+angle.cos()*dist, cy+angle.sin()*dist];
        let bt = match i%5 { 0=>BuildingType::Church, 1=>BuildingType::Castle, 2=>BuildingType::Temple, 3=>BuildingType::Library, _=>BuildingType::Market };
        layout.landmarks.push((pos, bt));
    }
}

pub fn generate_city(config: &CityConfig, _chunk: &GeneratedChunk, seed: u64) -> CityLayout {
    let mut rng = SimpleRng::new(seed ^ config.seed);
    let mut layout = CityLayout::new(config.center);
    match config.road_style {
        CityRoadStyle::Grid     => gen_grid_roads(config, &mut layout, &mut rng),
        CityRoadStyle::Organic  => gen_organic_roads(config, &mut layout, &mut rng),
        CityRoadStyle::Radial   => gen_radial_roads(config, &mut layout, &mut rng),
        CityRoadStyle::Medieval => gen_medieval_roads(config, &mut layout, &mut rng),
    }
    subdivide_city_blocks(config, &mut layout, &mut rng);
    place_city_landmarks(config, &mut layout, &mut rng);
    if !layout.roads.is_empty() {
        let (mut mnx, mut mny, mut mxx, mut mxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for road in &layout.roads { for pt in [road.start, road.end] { mnx=mnx.min(pt[0]); mny=mny.min(pt[1]); mxx=mxx.max(pt[0]); mxy=mxy.max(pt[1]); } }
        layout.bounds = [mnx, mny, mxx, mxy];
    }
    layout
}

pub fn draw_city_preview(painter: &Painter, layout: &CityLayout, editor_to_screen: impl Fn([f32;2])->Pos2, district_types: &[DistrictType], show_buildings: bool, show_districts: bool) {
    if show_districts {
        for block in &layout.blocks {
            if block.bounds.len() < 3 { continue; }
            let color = if block.district < district_types.len() {
                let c = district_types[block.district].color;
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 60)
            } else { Color32::from_rgba_unmultiplied(128, 128, 128, 40) };
            let pts: Vec<Pos2> = block.bounds.iter().map(|p| editor_to_screen(*p)).collect();
            painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
        }
    }
    for road in &layout.roads {
        let col = road.road_type.color();
        let sw = match road.road_type { CityRoadType::Highway=>3.5, CityRoadType::Main=>2.5, CityRoadType::Secondary=>1.5, CityRoadType::Alley=>0.8 };
        painter.line_segment([editor_to_screen(road.start), editor_to_screen(road.end)], Stroke::new(sw, col));
    }
    if show_buildings {
        for block in &layout.blocks { for building in &block.buildings {
            let col = building.building_type.color();
            if building.footprint.len() >= 3 {
                let pts: Vec<Pos2> = building.footprint.iter().map(|p| editor_to_screen(*p)).collect();
                painter.add(Shape::convex_polygon(pts, col, Stroke::new(0.5, Color32::BLACK)));
            }
        }}
    }
    for (pos, btype) in &layout.landmarks {
        let sp = editor_to_screen(*pos);
        painter.circle_filled(sp, 6.0, btype.color());
        painter.circle_stroke(sp, 7.0, Stroke::new(1.5, Color32::WHITE));
    }
}

// =================================================================
// DUNGEON GENERATION SYSTEM
// =================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DungeonAlgorithm { BSP, CellularAutomata, DrunkenWalk, Voronoi, PrefabAssembly }

impl DungeonAlgorithm {
    pub fn name(&self) -> &str {
        match self { DungeonAlgorithm::BSP=>"BSP", DungeonAlgorithm::CellularAutomata=>"Cellular Automata", DungeonAlgorithm::DrunkenWalk=>"Drunken Walk", DungeonAlgorithm::Voronoi=>"Voronoi", DungeonAlgorithm::PrefabAssembly=>"Prefab Assembly" }
    }
    pub fn all() -> &'static [DungeonAlgorithm] {
        &[DungeonAlgorithm::BSP, DungeonAlgorithm::CellularAutomata, DungeonAlgorithm::DrunkenWalk, DungeonAlgorithm::Voronoi, DungeonAlgorithm::PrefabAssembly]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CorridorStyle { Direct, Bent, Winding }
impl CorridorStyle {
    pub fn name(&self) -> &str { match self { CorridorStyle::Direct=>"Direct", CorridorStyle::Bent=>"Bent", CorridorStyle::Winding=>"Winding" } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DungeonTheme { Stone, Crypt, Ice, Lava, Forest, Ruins, Sewer, Crystal }
impl DungeonTheme {
    pub fn name(&self) -> &str {
        match self { DungeonTheme::Stone=>"Stone", DungeonTheme::Crypt=>"Crypt", DungeonTheme::Ice=>"Ice", DungeonTheme::Lava=>"Lava", DungeonTheme::Forest=>"Forest", DungeonTheme::Ruins=>"Ruins", DungeonTheme::Sewer=>"Sewer", DungeonTheme::Crystal=>"Crystal" }
    }
    pub fn wall_color(&self) -> Color32 {
        match self { DungeonTheme::Stone=>Color32::from_rgb(80,80,90), DungeonTheme::Crypt=>Color32::from_rgb(50,40,60), DungeonTheme::Ice=>Color32::from_rgb(140,180,220), DungeonTheme::Lava=>Color32::from_rgb(60,30,20), DungeonTheme::Forest=>Color32::from_rgb(40,70,30), DungeonTheme::Ruins=>Color32::from_rgb(100,90,70), DungeonTheme::Sewer=>Color32::from_rgb(50,70,50), DungeonTheme::Crystal=>Color32::from_rgb(60,80,120) }
    }
    pub fn floor_color(&self) -> Color32 {
        match self { DungeonTheme::Stone=>Color32::from_rgb(120,115,110), DungeonTheme::Crypt=>Color32::from_rgb(80,70,85), DungeonTheme::Ice=>Color32::from_rgb(200,220,240), DungeonTheme::Lava=>Color32::from_rgb(100,60,40), DungeonTheme::Forest=>Color32::from_rgb(80,110,60), DungeonTheme::Ruins=>Color32::from_rgb(140,130,110), DungeonTheme::Sewer=>Color32::from_rgb(80,100,80), DungeonTheme::Crystal=>Color32::from_rgb(120,140,180) }
    }
    pub fn all() -> &'static [DungeonTheme] {
        &[DungeonTheme::Stone, DungeonTheme::Crypt, DungeonTheme::Ice, DungeonTheme::Lava, DungeonTheme::Forest, DungeonTheme::Ruins, DungeonTheme::Sewer, DungeonTheme::Crystal]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonConfig {
    pub width: u32, pub height: u32,
    pub algorithm: DungeonAlgorithm,
    pub room_count: u32, pub min_room_size: u32, pub max_room_size: u32,
    pub corridor_style: CorridorStyle, pub corridor_width: u32,
    pub depth_level: u32, pub theme: DungeonTheme,
}

impl Default for DungeonConfig {
    fn default() -> Self { Self { width:80, height:60, algorithm:DungeonAlgorithm::BSP, room_count:12, min_room_size:4, max_room_size:12, corridor_style:CorridorStyle::Bent, corridor_width:1, depth_level:1, theme:DungeonTheme::Stone } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RoomType { Start, End, Boss, Treasure, Puzzle, Combat, Rest, Shop, Secret }
impl RoomType {
    pub fn name(&self) -> &str {
        match self { RoomType::Start=>"Start", RoomType::End=>"End", RoomType::Boss=>"Boss", RoomType::Treasure=>"Treasure", RoomType::Puzzle=>"Puzzle", RoomType::Combat=>"Combat", RoomType::Rest=>"Rest", RoomType::Shop=>"Shop", RoomType::Secret=>"Secret" }
    }
    pub fn color(&self) -> Color32 {
        match self { RoomType::Start=>Color32::from_rgb(80,200,80), RoomType::End=>Color32::from_rgb(200,80,80), RoomType::Boss=>Color32::from_rgb(200,60,200), RoomType::Treasure=>Color32::from_rgb(220,180,40), RoomType::Puzzle=>Color32::from_rgb(80,160,220), RoomType::Combat=>Color32::from_rgb(200,120,60), RoomType::Rest=>Color32::from_rgb(100,180,140), RoomType::Shop=>Color32::from_rgb(160,140,200), RoomType::Secret=>Color32::from_rgb(160,80,80) }
    }
    pub fn assign_auto(idx: usize, total: usize) -> RoomType {
        if idx==0 { return RoomType::Start; }
        if idx==total.saturating_sub(1) { return RoomType::End; }
        match idx%10 { 1=>RoomType::Boss, 2|3=>RoomType::Combat, 4=>RoomType::Treasure, 5=>RoomType::Rest, 6=>RoomType::Shop, 7=>RoomType::Puzzle, 8=>RoomType::Secret, _=>RoomType::Combat }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonRoom { pub rect: (u32,u32,u32,u32), pub room_type: RoomType, pub connections: Vec<usize> }
impl DungeonRoom {
    pub fn new(x:u32,y:u32,w:u32,h:u32)->Self { Self { rect:(x,y,w,h), room_type:RoomType::Combat, connections:Vec::new() } }
    pub fn center(&self) -> (u32,u32) { (self.rect.0+self.rect.2/2, self.rect.1+self.rect.3/2) }
    pub fn overlaps(&self, other: &DungeonRoom) -> bool {
        let (ax,ay,aw,ah) = self.rect; let (bx,by,bw,bh) = other.rect; let m = 1u32;
        !(ax+aw+m<=bx || bx+bw+m<=ax || ay+ah+m<=by || by+bh+m<=ay)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DungeonTile { Wall, Floor, Door, StairsUp, StairsDown, Chest, Trap, Torch, Pillar, Water, Lava, Bridge, Entrance, Exit, Empty }
impl DungeonTile {
    pub fn char_rep(&self) -> char {
        match self { DungeonTile::Wall=>'#', DungeonTile::Floor=>'.', DungeonTile::Door=>'+', DungeonTile::StairsUp=>'<', DungeonTile::StairsDown=>'>', DungeonTile::Chest=>'C', DungeonTile::Trap=>'^', DungeonTile::Torch=>'T', DungeonTile::Pillar=>'O', DungeonTile::Water=>'~', DungeonTile::Lava=>'!', DungeonTile::Bridge=>'=', DungeonTile::Entrance=>'E', DungeonTile::Exit=>'X', DungeonTile::Empty=>' ' }
    }
    pub fn tile_color(&self, theme: &DungeonTheme) -> Color32 {
        match self { DungeonTile::Wall=>theme.wall_color(), DungeonTile::Floor=>theme.floor_color(), DungeonTile::Door=>Color32::from_rgb(180,120,60), DungeonTile::StairsUp=>Color32::from_rgb(200,200,100), DungeonTile::StairsDown=>Color32::from_rgb(100,150,200), DungeonTile::Chest=>Color32::from_rgb(220,180,40), DungeonTile::Trap=>Color32::from_rgb(180,60,60), DungeonTile::Torch=>Color32::from_rgb(255,180,40), DungeonTile::Pillar=>Color32::from_rgb(130,120,110), DungeonTile::Water=>Color32::from_rgb(60,120,200), DungeonTile::Lava=>Color32::from_rgb(220,80,20), DungeonTile::Bridge=>Color32::from_rgb(160,130,90), DungeonTile::Entrance=>Color32::from_rgb(80,200,80), DungeonTile::Exit=>Color32::from_rgb(200,80,80), DungeonTile::Empty=>Color32::TRANSPARENT }
    }
    pub fn is_passable(&self) -> bool { !matches!(self, DungeonTile::Wall|DungeonTile::Empty|DungeonTile::Lava) }
    pub fn all_variants() -> &'static [DungeonTile] {
        &[DungeonTile::Wall, DungeonTile::Floor, DungeonTile::Door, DungeonTile::StairsUp, DungeonTile::StairsDown, DungeonTile::Chest, DungeonTile::Trap, DungeonTile::Torch, DungeonTile::Pillar, DungeonTile::Water, DungeonTile::Lava, DungeonTile::Bridge, DungeonTile::Entrance, DungeonTile::Exit]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonMap {
    pub width: u32, pub height: u32,
    pub tiles: Vec<DungeonTile>,
    pub rooms: Vec<DungeonRoom>,
    pub theme: DungeonTheme,
}
impl DungeonMap {
    pub fn new(width:u32, height:u32, theme:DungeonTheme) -> Self {
        Self { width, height, tiles:vec![DungeonTile::Wall;(width*height) as usize], rooms:Vec::new(), theme }
    }
    pub fn idx(&self,x:u32,y:u32)->usize { (y*self.width+x) as usize }
    pub fn get(&self,x:u32,y:u32)->&DungeonTile { &self.tiles[self.idx(x,y)] }
    pub fn set(&mut self,x:u32,y:u32,t:DungeonTile) { let i=self.idx(x,y); self.tiles[i]=t; }
    pub fn set_rect_floor(&mut self,x:u32,y:u32,w:u32,h:u32) {
        for ry in y..y.saturating_add(h).min(self.height) { for rx in x..x.saturating_add(w).min(self.width) { self.set(rx,ry,DungeonTile::Floor); } }
    }
    pub fn carve_h(&mut self,x0:u32,x1:u32,y:u32,cw:u32) {
        let (s,e)=if x0<x1{(x0,x1)}else{(x1,x0)};
        for x in s..=e.min(self.width.saturating_sub(1)) { for dy in 0..cw { let wy=y.saturating_add(dy); if wy<self.height{self.set(x,wy,DungeonTile::Floor);} } }
    }
    pub fn carve_v(&mut self,y0:u32,y1:u32,x:u32,cw:u32) {
        let (s,e)=if y0<y1{(y0,y1)}else{(y1,y0)};
        for y in s..=e.min(self.height.saturating_sub(1)) { for dx in 0..cw { let wx=x.saturating_add(dx); if wx<self.width{self.set(wx,y,DungeonTile::Floor);} } }
    }
    pub fn floor_count(&self)->usize { self.tiles.iter().filter(|t|t.is_passable()).count() }
}

struct BspNode2 { x:u32,y:u32,w:u32,h:u32,left:Option<Box<BspNode2>>,right:Option<Box<BspNode2>>,room:Option<(u32,u32,u32,u32)> }
impl BspNode2 {
    fn new(x:u32,y:u32,w:u32,h:u32)->Self { Self{x,y,w,h,left:None,right:None,room:None} }
    fn split(&mut self,ms:u32,rng:&mut SimpleRng) {
        if self.w<ms*2&&self.h<ms*2{return;}
        let sh=if self.w<ms*2{true}else if self.h<ms*2{false}else{rng.next_f32()<0.5};
        if sh&&self.h>=ms*2{
            let sp=ms+rng.next_u64() as u32%(self.h-ms*2+1);
            let (mut l,mut r)=(BspNode2::new(self.x,self.y,self.w,sp),BspNode2::new(self.x,self.y+sp,self.w,self.h-sp));
            l.split(ms,rng);r.split(ms,rng);self.left=Some(Box::new(l));self.right=Some(Box::new(r));
        }else if self.w>=ms*2{
            let sp=ms+rng.next_u64() as u32%(self.w-ms*2+1);
            let (mut l,mut r)=(BspNode2::new(self.x,self.y,sp,self.h),BspNode2::new(self.x+sp,self.y,self.w-sp,self.h));
            l.split(ms,rng);r.split(ms,rng);self.left=Some(Box::new(l));self.right=Some(Box::new(r));
        }
    }
    fn place_rooms(&mut self,min_r:u32,max_r:u32,rng:&mut SimpleRng){
        if self.left.is_some()||self.right.is_some(){
            if let Some(l)=&mut self.left{l.place_rooms(min_r,max_r,rng);}
            if let Some(r)=&mut self.right{r.place_rooms(min_r,max_r,rng);}
        }else{
            let rw=(min_r+rng.next_u64() as u32%(max_r-min_r+1).max(1)).min(self.w.saturating_sub(2));
            let rh=(min_r+rng.next_u64() as u32%(max_r-min_r+1).max(1)).min(self.h.saturating_sub(2));
            if rw<2||rh<2{return;}
            let rx=self.x+1+rng.next_u64() as u32%(self.w-rw-1).max(1);
            let ry=self.y+1+rng.next_u64() as u32%(self.h-rh-1).max(1);
            self.room=Some((rx,ry,rw,rh));
        }
    }
    fn collect_rooms(&self)->Vec<(u32,u32,u32,u32)>{
        let mut v=Vec::new(); if let Some(r)=self.room{v.push(r);}
        if let Some(l)=&self.left{v.extend(l.collect_rooms());}
        if let Some(r)=&self.right{v.extend(r.collect_rooms());}
        v
    }
    fn collect_pairs(&self)->Vec<((u32,u32),(u32,u32))>{
        let mut v=Vec::new();
        if self.left.is_some()&&self.right.is_some(){
            let lr=self.left.as_ref().unwrap().collect_rooms();
            let rr=self.right.as_ref().unwrap().collect_rooms();
            if !lr.is_empty()&&!rr.is_empty(){
                let (lx,ly,lw,lh)=lr[0];let (rx,ry,rw,rh)=rr[0];
                v.push(((lx+lw/2,ly+lh/2),(rx+rw/2,ry+rh/2)));
            }
            v.extend(self.left.as_ref().unwrap().collect_pairs());
            v.extend(self.right.as_ref().unwrap().collect_pairs());
        }
        v
    }
}

pub fn bsp_dungeon(config: &DungeonConfig, seed: u64) -> DungeonMap {
    let mut rng=SimpleRng::new(seed);
    let mut map=DungeonMap::new(config.width,config.height,config.theme.clone());
    let mut root=BspNode2::new(0,0,config.width,config.height);
    root.split(config.min_room_size+2,&mut rng);
    root.place_rooms(config.min_room_size,config.max_room_size,&mut rng);
    let raws=root.collect_rooms();
    for (i,&(rx,ry,rw,rh)) in raws.iter().enumerate(){
        map.set_rect_floor(rx,ry,rw,rh);
        let mut room=DungeonRoom::new(rx,ry,rw,rh);
        room.room_type=RoomType::assign_auto(i,raws.len());
        map.rooms.push(room);
    }
    for ((x0,y0),(x1,y1)) in root.collect_pairs(){
        let cw=config.corridor_width;
        match config.corridor_style{
            CorridorStyle::Direct|CorridorStyle::Bent=>{map.carve_h(x0,x1,y0,cw);map.carve_v(y0,y1,x1,cw);},
            CorridorStyle::Winding=>{let mx=(x0+x1)/2;map.carve_h(x0,mx,y0,cw);map.carve_v(y0,y1,mx,cw);map.carve_h(mx,x1,y1,cw);},
        }
    }
    place_dungeon_specials(&mut map,&mut rng);
    map
}

pub fn cellular_dungeon(config: &DungeonConfig, seed: u64) -> DungeonMap {
    let mut rng=SimpleRng::new(seed);
    let (w,h)=(config.width as usize,config.height as usize);
    let mut map=DungeonMap::new(config.width,config.height,config.theme.clone());
    let mut grid:Vec<bool>=(0..w*h).map(|_|rng.next_f32()<0.45).collect();
    for x in 0..w{grid[x]=true;grid[(h-1)*w+x]=true;}
    for y in 0..h{grid[y*w]=true;grid[y*w+w-1]=true;}
    for _ in 0..5{
        let mut next=grid.clone();
        for y in 1..(h-1){for x in 1..(w-1){
            let mut wc=0u32;
            for dy in 0..3usize{for dx in 0..3usize{if grid[(y+dy-1)*w+(x+dx-1)]{wc+=1;}}}
            next[y*w+x]=wc>=5;
        }}
        grid=next;
    }
    for y in 0..h{for x in 0..w{if !grid[y*w+x]{map.tiles[y*w+x]=DungeonTile::Floor;}}}
    let comp=dungeon_largest_component(&map);
    for i in 0..(w*h){if map.tiles[i]==DungeonTile::Floor&&!comp.contains(&i){map.tiles[i]=DungeonTile::Wall;}}
    let mut placed=0u32;
    for _ in 0..200{
        if placed>=config.room_count{break;}
        let rx=1+rng.next_u64() as u32%(config.width-config.min_room_size-2);
        let ry=1+rng.next_u64() as u32%(config.height-config.min_room_size-2);
        let idx=map.idx(rx+config.min_room_size/2,ry+config.min_room_size/2);
        if map.tiles[idx]==DungeonTile::Floor{map.rooms.push(DungeonRoom::new(rx,ry,config.min_room_size,config.min_room_size));placed+=1;}
    }
    for i in 0..map.rooms.len(){map.rooms[i].room_type=RoomType::assign_auto(i,map.rooms.len());}
    place_dungeon_specials(&mut map,&mut rng);
    map
}

fn dungeon_largest_component(map:&DungeonMap)->HashSet<usize>{
    let (w,h)=(map.width as usize,map.height as usize);
    let mut visited=vec![false;w*h];let mut best=HashSet::new();
    for start in 0..w*h{
        if visited[start]||map.tiles[start]!=DungeonTile::Floor{continue;}
        let mut comp=HashSet::new();let mut stack=vec![start];
        while let Some(idx)=stack.pop(){
            if visited[idx]{continue;}visited[idx]=true;comp.insert(idx);
            let (x,y)=(idx%w,idx/w);
            for (dx,dy) in [(!0usize,0usize),(1,0),(0,!0),(0,1)]{
                let (nx,ny)=(x.wrapping_add(dx),y.wrapping_add(dy));
                if nx<w&&ny<h{let ni=ny*w+nx;if !visited[ni]&&map.tiles[ni]==DungeonTile::Floor{stack.push(ni);}}
            }
        }
        if comp.len()>best.len(){best=comp;}
    }
    best
}

pub fn drunkard_walk_dungeon(config: &DungeonConfig, seed: u64) -> DungeonMap {
    let mut rng=SimpleRng::new(seed);
    let mut map=DungeonMap::new(config.width,config.height,config.theme.clone());
    let target=(config.width*config.height) as usize/3;
    let mut floors=0usize;
    let (mut cx,mut cy)=(config.width/2,config.height/2);
    map.set(cx,cy,DungeonTile::Floor);floors+=1;
    for _ in 0..config.width*config.height*10{
        if floors>=target{break;}
        let (nx,ny)=match rng.next_u64()%4{0=>(cx.saturating_sub(1),cy),1=>((cx+1).min(config.width-1),cy),2=>(cx,cy.saturating_sub(1)),_=>(cx,(cy+1).min(config.height-1))};
        if nx==0||ny==0||nx>=config.width-1||ny>=config.height-1{cx=config.width/2;cy=config.height/2;continue;}
        cx=nx;cy=ny;
        if map.get(cx,cy)==&DungeonTile::Wall{map.set(cx,cy,DungeonTile::Floor);floors+=1;}
    }
    for _ in 0..config.room_count.min(20){
        let rx=2+rng.next_u64() as u32%(config.width.saturating_sub(config.min_room_size+4));
        let ry=2+rng.next_u64() as u32%(config.height.saturating_sub(config.min_room_size+4));
        let rw=config.min_room_size+rng.next_u64() as u32%(config.max_room_size-config.min_room_size+1).max(1);
        let rh=config.min_room_size+rng.next_u64() as u32%(config.max_room_size-config.min_room_size+1).max(1);
        map.set_rect_floor(rx,ry,rw,rh);
        map.rooms.push(DungeonRoom::new(rx,ry,rw,rh));
    }
    for i in 0..map.rooms.len(){map.rooms[i].room_type=RoomType::assign_auto(i,map.rooms.len());}
    place_dungeon_specials(&mut map,&mut rng);
    map
}

pub fn validate_dungeon_connectivity(map:&DungeonMap)->bool{
    let (w,h)=(map.width as usize,map.height as usize);
    let start=match map.tiles.iter().position(|t|t.is_passable()){Some(s)=>s,None=>return false};
    let mut visited=vec![false;w*h];let mut stack=vec![start];
    while let Some(idx)=stack.pop(){
        if visited[idx]{continue;}visited[idx]=true;
        let (x,y)=(idx%w,idx/w);
        for (dx,dy) in [(!0usize,0usize),(1,0),(0,!0),(0,1)]{
            let (nx,ny)=(x.wrapping_add(dx),y.wrapping_add(dy));
            if nx<w&&ny<h{let ni=ny*w+nx;if !visited[ni]&&map.tiles[ni].is_passable(){stack.push(ni);}}
        }
    }
    let total=map.tiles.iter().filter(|t|t.is_passable()).count();
    visited.iter().filter(|&&v|v).count()>=total*9/10
}

fn place_dungeon_specials(map:&mut DungeonMap,rng:&mut SimpleRng){
    let (w,h)=(map.width,map.height);
    let floor_tiles:Vec<(u32,u32)>=(0..h).flat_map(|y|(0..w).filter_map(move|x|if map.get(x,y)==&DungeonTile::Floor{Some((x,y))}else{None})).collect();
    if floor_tiles.len()>2{
        let ui=rng.next_u64() as usize%floor_tiles.len();
        let (ux,uy)=floor_tiles[ui];map.set(ux,uy,DungeonTile::StairsUp);
        let di=(ui+floor_tiles.len()/2)%floor_tiles.len();
        let (dx,dy)=floor_tiles[di];map.set(dx,dy,DungeonTile::StairsDown);
    }
    for room in &map.rooms.clone(){
        if room.room_type==RoomType::Treasure{
            let (cx,cy)=room.center();
            if cx<w&&cy<h&&map.get(cx,cy)==&DungeonTile::Floor{map.set(cx,cy,DungeonTile::Chest);}
        }
    }
    for room in &map.rooms.clone(){
        let (rx,ry,rw,rh)=room.rect;
        let corners=[(rx+1,ry+1),(rx+rw.saturating_sub(2),ry+1),(rx+1,ry+rh.saturating_sub(2)),(rx+rw.saturating_sub(2),ry+rh.saturating_sub(2))];
        for (tx,ty) in corners{if tx<w&&ty<h&&map.get(tx,ty)==&DungeonTile::Floor&&rng.next_f32()<0.4{map.set(tx,ty,DungeonTile::Torch);}}
    }
    let trap_count=map.floor_count()/50+1;let mut placed=0;let mut attempts=0;
    while placed<trap_count&&attempts<1000{attempts+=1;let x=rng.next_u64() as u32%w;let y=rng.next_u64() as u32%h;if map.get(x,y)==&DungeonTile::Floor{map.set(x,y,DungeonTile::Trap);placed+=1;}}
}

pub fn draw_dungeon_preview(painter:&Painter,map:&DungeonMap,origin:Pos2,tile_size:f32){
    for y in 0..map.height{for x in 0..map.width{
        let tile=map.get(x,y);if tile==&DungeonTile::Empty{continue;}
        let (tx,ty)=(origin.x+x as f32*tile_size,origin.y+y as f32*tile_size);
        painter.rect_filled(Rect::from_min_size(Pos2::new(tx,ty),Vec2::splat(tile_size)),0.0,tile.tile_color(&map.theme));
    }}
    for room in &map.rooms{
        let (cx,cy)=room.center();
        let (sx,sy)=(origin.x+cx as f32*tile_size+tile_size*0.5,origin.y+cy as f32*tile_size+tile_size*0.5);
        painter.circle_filled(Pos2::new(sx,sy),tile_size*0.8,room.room_type.color());
    }
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub enum DungeonTool{Paint,RoomPlace,CorridorDraw,Select,Erase}
impl DungeonTool{pub fn name(&self)->&str{match self{DungeonTool::Paint=>"Paint",DungeonTool::RoomPlace=>"Room",DungeonTool::CorridorDraw=>"Corridor",DungeonTool::Select=>"Select",DungeonTool::Erase=>"Erase"}}}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DungeonEditorState{pub config:DungeonConfig,pub map:Option<DungeonMap>,pub active_tool:DungeonTool,pub active_tile:DungeonTile,pub selected_room:Option<usize>,pub view_offset:[f32;2],pub tile_size:f32,pub drag_start:Option<(u32,u32)>,pub show_room_labels:bool,pub connectivity_valid:bool}
impl Default for DungeonEditorState{
    fn default()->Self{Self{config:DungeonConfig::default(),map:None,active_tool:DungeonTool::Paint,active_tile:DungeonTile::Floor,selected_room:None,view_offset:[0.0,0.0],tile_size:8.0,drag_start:None,show_room_labels:true,connectivity_valid:true}}
}
impl DungeonEditorState{
    pub fn generate(&mut self,seed:u64){
        self.map=Some(match self.config.algorithm{
            DungeonAlgorithm::BSP=>bsp_dungeon(&self.config,seed),
            DungeonAlgorithm::CellularAutomata=>cellular_dungeon(&self.config,seed),
            DungeonAlgorithm::DrunkenWalk=>drunkard_walk_dungeon(&self.config,seed),
            DungeonAlgorithm::Voronoi=>bsp_dungeon(&self.config,seed^0xABCD),
            DungeonAlgorithm::PrefabAssembly=>bsp_dungeon(&self.config,seed^0xDEAD),
        });
        if let Some(ref m)=self.map{self.connectivity_valid=validate_dungeon_connectivity(m);}
    }
    pub fn paint_tile(&mut self,tx:u32,ty:u32){if let Some(ref mut map)=self.map{if tx<map.width&&ty<map.height{map.set(tx,ty,self.active_tile.clone());}}}
    pub fn erase_tile(&mut self,tx:u32,ty:u32){if let Some(ref mut map)=self.map{if tx<map.width&&ty<map.height{map.set(tx,ty,DungeonTile::Wall);}}}
    pub fn place_room(&mut self,tx:u32,ty:u32,w:u32,h:u32){if let Some(ref mut map)=self.map{map.set_rect_floor(tx,ty,w,h);map.rooms.push(DungeonRoom::new(tx,ty,w,h));}}
    pub fn screen_to_tile(&self,sp:Pos2,origin:Pos2)->(u32,u32){
        let(rx,ry)=(sp.x-origin.x-self.view_offset[0],sp.y-origin.y-self.view_offset[1]);
        (((rx/self.tile_size).floor() as i32).max(0) as u32,((ry/self.tile_size).floor() as i32).max(0) as u32)
    }
}

pub fn show_dungeon_editor(ui:&mut egui::Ui,state:&mut DungeonEditorState,seed:u64){
    ui.horizontal(|ui|{
        ui.label("Dungeon Editor");
        if ui.button("Generate").clicked(){state.generate(seed);}
        if let Some(ref map)=state.map{
            ui.label(format!("{} rooms",map.rooms.len()));
            let (txt,col)=if state.connectivity_valid{("Connected",Color32::GREEN)}else{("DISCONNECTED!",Color32::RED)};
            ui.colored_label(col,txt);
        }
    });
    ui.separator();
    egui::CollapsingHeader::new("Config").show(ui,|ui|{
        ui.horizontal(|ui|{ui.label("Algorithm:");for alg in DungeonAlgorithm::all(){if ui.selectable_label(state.config.algorithm==*alg,alg.name()).clicked(){state.config.algorithm=alg.clone();}}});
        ui.horizontal(|ui|{ui.label("Size:");ui.add(egui::DragValue::new(&mut state.config.width).range(20..=200).suffix("W"));ui.add(egui::DragValue::new(&mut state.config.height).range(20..=150).suffix("H"));});
        ui.horizontal(|ui|{ui.label("Rooms:");ui.add(egui::DragValue::new(&mut state.config.room_count).range(2..=50));});
        ui.horizontal(|ui|{ui.label("Theme:");for theme in DungeonTheme::all(){if ui.selectable_label(state.config.theme==*theme,theme.name()).clicked(){state.config.theme=theme.clone();}}});
    });
    ui.separator();
    ui.horizontal(|ui|{for tool in &[DungeonTool::Paint,DungeonTool::Erase,DungeonTool::RoomPlace,DungeonTool::Select]{if ui.selectable_label(std::mem::discriminant(&state.active_tool)==std::mem::discriminant(tool),tool.name()).clicked(){state.active_tool=tool.clone();}}});
    ui.horizontal_wrapped(|ui|{
        for tile in DungeonTile::all_variants(){
            let sel=std::mem::discriminant(&state.active_tile)==std::mem::discriminant(tile);
            let btn=egui::Button::new(tile.char_rep().to_string()).fill(if sel{Color32::from_rgb(80,120,180)}else{Color32::from_gray(40)});
            if ui.add(btn).on_hover_text(format!("{:?}",tile)).clicked(){state.active_tile=tile.clone();}
        }
    });
    ui.separator();
    let vp=ui.available_size();
    let (response,painter)=ui.allocate_painter(vp,egui::Sense::click_and_drag());
    let origin=response.rect.min;
    if let Some(ref map)=state.map{
        painter.rect_filled(response.rect,0.0,Color32::from_gray(15));
        let draw_origin=Pos2::new(origin.x+state.view_offset[0],origin.y+state.view_offset[1]);
        draw_dungeon_preview(&painter,map,draw_origin,state.tile_size);
        if state.show_room_labels{for room in &map.rooms{let(cx,cy)=room.center();let(sx,sy)=(draw_origin.x+cx as f32*state.tile_size,draw_origin.y+cy as f32*state.tile_size);painter.text(Pos2::new(sx,sy),egui::Align2::CENTER_CENTER,room.room_type.name(),FontId::proportional(9.0),Color32::WHITE);}}
    }else{painter.text(response.rect.center(),egui::Align2::CENTER_CENTER,"Click Generate to create dungeon",FontId::proportional(14.0),Color32::GRAY);}
    if response.dragged_by(egui::PointerButton::Middle){let d=response.drag_delta();state.view_offset[0]+=d.x;state.view_offset[1]+=d.y;}
    if response.dragged_by(egui::PointerButton::Primary)||response.clicked(){
        if let Some(pos)=response.interact_pointer_pos(){
            let (tx,ty)=state.screen_to_tile(pos,origin);
            match state.active_tool{
                DungeonTool::Paint=>state.paint_tile(tx,ty),
                DungeonTool::Erase=>state.erase_tile(tx,ty),
                DungeonTool::RoomPlace=>{
                    if let Some((sx,sy))=state.drag_start{let(w,h)=(tx.abs_diff(sx).max(1)+1,ty.abs_diff(sy).max(1)+1);state.place_room(sx.min(tx),sy.min(ty),w,h);state.drag_start=None;}
                    else{state.drag_start=Some((tx,ty));}
                },
                _=>{},
            }
        }
    }
}

// =================================================================
// CAVE SYSTEM GENERATION
// =================================================================

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct CaveConfig{pub roughness:f32,pub openness:f32,pub stalactite_density:f32,pub underground_lake_chance:f32,pub crystal_density:f32,pub lava_flow_chance:f32,pub width:u32,pub height:u32,pub depth:u32,pub passes:u32}
impl Default for CaveConfig{fn default()->Self{Self{roughness:0.5,openness:0.45,stalactite_density:0.1,underground_lake_chance:0.2,crystal_density:0.05,lava_flow_chance:0.1,width:100,height:80,depth:1,passes:5}}}

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub enum CaveTile{SolidRock,Air,Stalactite,Stalagmite,Water,Lava,Crystal,Dirt,Gravel,CaveEntrance}
impl CaveTile{
    pub fn is_open(&self)->bool{matches!(self,CaveTile::Air|CaveTile::Water|CaveTile::Lava|CaveTile::Crystal)}
    pub fn cave_color(&self)->Color32{match self{CaveTile::SolidRock=>Color32::from_rgb(60,55,50),CaveTile::Air=>Color32::from_rgb(20,20,25),CaveTile::Stalactite=>Color32::from_rgb(90,80,70),CaveTile::Stalagmite=>Color32::from_rgb(100,88,75),CaveTile::Water=>Color32::from_rgb(40,100,180),CaveTile::Lava=>Color32::from_rgb(220,80,20),CaveTile::Crystal=>Color32::from_rgb(100,180,220),CaveTile::Dirt=>Color32::from_rgb(80,65,50),CaveTile::Gravel=>Color32::from_rgb(100,95,90),CaveTile::CaveEntrance=>Color32::from_rgb(180,160,100)}}
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct CaveChamber{pub tiles:Vec<usize>,pub center:(u32,u32),pub area:u32,pub has_lake:bool,pub has_crystals:bool}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct CaveMap{pub width:u32,pub height:u32,pub tiles:Vec<CaveTile>,pub chambers:Vec<CaveChamber>,pub ceiling_heights:Vec<f32>}
impl CaveMap{
    pub fn new(w:u32,h:u32)->Self{Self{width:w,height:h,tiles:vec![CaveTile::SolidRock;(w*h) as usize],chambers:Vec::new(),ceiling_heights:vec![0.0;(w*h) as usize]}}
    pub fn idx(&self,x:u32,y:u32)->usize{(y*self.width+x) as usize}
    pub fn get(&self,x:u32,y:u32)->&CaveTile{&self.tiles[self.idx(x,y)]}
    pub fn set(&mut self,x:u32,y:u32,t:CaveTile){let i=self.idx(x,y);self.tiles[i]=t;}
}

pub fn generate_cave(config:&CaveConfig,seed:u64)->CaveMap{
    let mut rng=SimpleRng::new(seed);
    let (w,h)=(config.width as usize,config.height as usize);
    let mut map=CaveMap::new(config.width,config.height);
    let mut grid:Vec<bool>=(0..w*h).map(|_|rng.next_f32()>config.openness).collect();
    for x in 0..w{grid[x]=true;grid[(h-1)*w+x]=true;}
    for y in 0..h{grid[y*w]=true;grid[y*w+w-1]=true;}
    let threshold=(4.5+config.roughness*1.5) as u32;
    for _ in 0..config.passes{
        let mut next=grid.clone();
        for y in 1..(h-1){for x in 1..(w-1){
            let mut wc=0u32;
            for dy in 0..3usize{for dx in 0..3usize{if grid[(y+dy-1)*w+(x+dx-1)]{wc+=1;}}}
            next[y*w+x]=wc>=threshold;
        }}
        grid=next;
    }
    for y in 0..h{for x in 0..w{map.tiles[y*w+x]=if grid[y*w+x]{CaveTile::SolidRock}else{CaveTile::Air};}}
    find_cave_chambers_impl(&mut map);
    decorate_cave_impl(&mut map,config,&mut rng);
    for x in 0..config.width{
        let(mut in_air,mut air_start)=(false,0u32);
        for y in 0..config.height{
            let is_open=map.get(x,y).is_open();
            if is_open&&!in_air{in_air=true;air_start=y;}
            else if !is_open&&in_air{in_air=false;let hv=(y-air_start) as f32;for ay in air_start..y{let idx=map.idx(x,ay);map.ceiling_heights[idx]=hv;}}
        }
    }
    map
}

fn find_cave_chambers_impl(map:&mut CaveMap){
    let (w,h)=(map.width as usize,map.height as usize);
    let mut visited=vec![false;w*h];let mut chambers=Vec::new();
    for sy in 0..h{for sx in 0..w{
        let start=sy*w+sx;
        if visited[start]||!map.tiles[start].is_open(){continue;}
        let mut comp=Vec::new();let mut stack=vec![start];
        let(mut mnx,mut mny,mut mxx,mut mxy)=(sx,sy,sx,sy);
        while let Some(idx)=stack.pop(){
            if visited[idx]{continue;}visited[idx]=true;comp.push(idx);
            let(x,y)=(idx%w,idx/w);
            mnx=mnx.min(x);mny=mny.min(y);mxx=mxx.max(x);mxy=mxy.max(y);
            for(dx,dy)in[(!0usize,0usize),(1,0),(0,!0),(0,1)]{
                let(nx,ny)=(x.wrapping_add(dx),y.wrapping_add(dy));
                if nx<w&&ny<h&&!visited[ny*w+nx]&&map.tiles[ny*w+nx].is_open(){stack.push(ny*w+nx);}
            }
        }
        if comp.len()>=10{
            let area=comp.len() as u32;
            chambers.push(CaveChamber{tiles:comp,center:((mnx+mxx)/2 as u32,((mny+mxy)/2) as u32),area,has_lake:false,has_crystals:false});
        }
    }}
    map.chambers=chambers;
}

fn decorate_cave_impl(map:&mut CaveMap,config:&CaveConfig,rng:&mut SimpleRng){
    let(w,h)=(map.width,map.height);
    for ci in 0..map.chambers.len(){
        if rng.next_f32()<config.underground_lake_chance{
            let(cx,cy)=(map.chambers[ci].center.0,map.chambers[ci].center.1);
            let lr=2+(rng.next_f32()*4.0) as u32;
            for dy in 0..lr*2{for dx in 0..lr*2{
                let(lx,ly)=(cx.saturating_sub(lr)+dx,cy.saturating_sub(lr)+dy);
                if lx>=w||ly>=h{continue;}
                let d2=(lx as i32-cx as i32).pow(2)+(ly as i32-cy as i32).pow(2);
                if d2<=(lr*lr) as i32&&map.get(lx,ly).is_open(){map.set(lx,ly,CaveTile::Water);}
            }}
            map.chambers[ci].has_lake=true;
        }
        if rng.next_f32()<config.crystal_density*3.0{
            let cc=3+(rng.next_f32()*8.0) as usize;
            let ct=map.chambers[ci].tiles.clone();
            for _ in 0..cc{if ct.is_empty(){break;}
                let ti=ct[rng.next_u64() as usize%ct.len()];
                let(x,y)=((ti%w as usize) as u32,(ti/w as usize) as u32);
                if map.get(x,y)==&CaveTile::Air{map.set(x,y,CaveTile::Crystal);}
            }
            map.chambers[ci].has_crystals=true;
        }
        if rng.next_f32()<config.lava_flow_chance{
            let ct=map.chambers[ci].tiles.clone();if ct.is_empty(){continue;}
            let si=ct[rng.next_u64() as usize%ct.len()];
            let(mut lx,mut ly)=((si%w as usize) as u32,(si/w as usize) as u32);
            for _ in 0..30{
                if lx>=w||ly>=h{break;}
                if map.get(lx,ly).is_open(){map.set(lx,ly,CaveTile::Lava);}
                let dx=(rng.next_f32()*3.0-1.5) as i32;
                lx=(lx as i32+dx).clamp(0,w as i32-1) as u32;ly=(ly+1).min(h-1);
            }
        }
    }
    for x in 1..(w-1){for y in 1..(h-1){
        if map.get(x,y)==&CaveTile::Air&&map.get(x,y.saturating_sub(1))==&CaveTile::SolidRock&&rng.next_f32()<config.stalactite_density{map.set(x,y,CaveTile::Stalactite);}
        if map.get(x,y)==&CaveTile::Air&&map.get(x,(y+1).min(h-1))==&CaveTile::SolidRock&&rng.next_f32()<config.stalactite_density*0.7{map.set(x,y,CaveTile::Stalagmite);}
    }}
}

pub fn draw_cave_preview(painter:&Painter,map:&CaveMap,origin:Pos2,tile_size:f32,show_chambers:bool){
    for y in 0..map.height{for x in 0..map.width{
        painter.rect_filled(Rect::from_min_size(Pos2::new(origin.x+x as f32*tile_size,origin.y+y as f32*tile_size),Vec2::splat(tile_size)),0.0,map.get(x,y).cave_color());
    }}
    if show_chambers{for ch in &map.chambers{
        let(sx,sy)=(origin.x+ch.center.0 as f32*tile_size,origin.y+ch.center.1 as f32*tile_size);
        let col=if ch.has_lake{Color32::from_rgba_unmultiplied(40,100,200,100)}else if ch.has_crystals{Color32::from_rgba_unmultiplied(100,180,220,100)}else{Color32::from_rgba_unmultiplied(200,200,200,50)};
        painter.circle_filled(Pos2::new(sx,sy),(ch.area as f32).sqrt()*tile_size*0.1,col);
        painter.text(Pos2::new(sx,sy),egui::Align2::CENTER_CENTER,format!("{}t",ch.area),FontId::proportional(7.0),Color32::WHITE);
    }}
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct CaveEditorState{pub config:CaveConfig,pub map:Option<CaveMap>,pub tile_size:f32,pub pan:[f32;2],pub show_chambers:bool,pub seed:u64}
impl Default for CaveEditorState{fn default()->Self{Self{config:CaveConfig::default(),map:None,tile_size:5.0,pan:[0.0,0.0],show_chambers:true,seed:99}}}
pub fn show_cave_editor(ui:&mut egui::Ui,state:&mut CaveEditorState){
    ui.horizontal(|ui|{ui.heading("Cave Generator");if ui.button("Generate").clicked(){state.map=Some(generate_cave(&state.config,state.seed));}ui.add(egui::DragValue::new(&mut state.seed).prefix("Seed: "));});
    ui.separator();
    egui::CollapsingHeader::new("Cave Config").show(ui,|ui|{
        ui.horizontal(|ui|{ui.label("Size:");ui.add(egui::DragValue::new(&mut state.config.width).range(20..=300).suffix("W"));ui.add(egui::DragValue::new(&mut state.config.height).range(20..=200).suffix("H"));});
        ui.horizontal(|ui|{ui.label("Roughness:");ui.add(egui::Slider::new(&mut state.config.roughness,0.0..=1.0));});
        ui.horizontal(|ui|{ui.label("Openness:");ui.add(egui::Slider::new(&mut state.config.openness,0.0..=1.0));});
        ui.horizontal(|ui|{ui.label("Stalactites:");ui.add(egui::Slider::new(&mut state.config.stalactite_density,0.0..=1.0));});
        ui.horizontal(|ui|{ui.label("Lakes:");ui.add(egui::Slider::new(&mut state.config.underground_lake_chance,0.0..=1.0));});
        ui.horizontal(|ui|{ui.label("Crystals:");ui.add(egui::Slider::new(&mut state.config.crystal_density,0.0..=1.0));});
        ui.horizontal(|ui|{ui.label("Lava:");ui.add(egui::Slider::new(&mut state.config.lava_flow_chance,0.0..=1.0));});
        ui.horizontal(|ui|{ui.label("Passes:");ui.add(egui::DragValue::new(&mut state.config.passes).range(1..=10));});
    });
    ui.horizontal(|ui|{ui.checkbox(&mut state.show_chambers,"Chambers");ui.label("Tile size:");ui.add(egui::DragValue::new(&mut state.tile_size).range(1.0..=20.0));});
    if let Some(ref map)=state.map{ui.label(format!("Chambers: {} | Open tiles: {}",map.chambers.len(),map.tiles.iter().filter(|t|t.is_open()).count()));}
    ui.separator();
    let avail=ui.available_size();
    let(response,painter)=ui.allocate_painter(avail,egui::Sense::click_and_drag());
    let origin=Pos2::new(response.rect.left()+state.pan[0],response.rect.top()+state.pan[1]);
    painter.rect_filled(response.rect,0.0,Color32::from_gray(5));
    if let Some(ref map)=state.map{draw_cave_preview(&painter,map,origin,state.tile_size,state.show_chambers);}
    else{painter.text(response.rect.center(),egui::Align2::CENTER_CENTER,"Click Generate to create caves",FontId::proportional(14.0),Color32::GRAY);}
    if response.dragged_by(egui::PointerButton::Primary){let d=response.drag_delta();state.pan[0]+=d.x;state.pan[1]+=d.y;}
}

// =================================================================
// BIOME TRANSITION SYSTEM
// =================================================================

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub enum TransitionCurve{Linear,SmoothStep,EaseIn,EaseOut,Sharp,Sine}
impl TransitionCurve{
    pub fn name(&self)->&str{match self{TransitionCurve::Linear=>"Linear",TransitionCurve::SmoothStep=>"SmoothStep",TransitionCurve::EaseIn=>"Ease In",TransitionCurve::EaseOut=>"Ease Out",TransitionCurve::Sharp=>"Sharp",TransitionCurve::Sine=>"Sine"}}
    pub fn evaluate(&self,t:f32)->f32{let t=t.clamp(0.0,1.0);match self{TransitionCurve::Linear=>t,TransitionCurve::SmoothStep=>t*t*(3.0-2.0*t),TransitionCurve::EaseIn=>t*t,TransitionCurve::EaseOut=>1.0-(1.0-t)*(1.0-t),TransitionCurve::Sharp=>if t<0.5{0.0}else{1.0},TransitionCurve::Sine=>(t*std::f32::consts::FRAC_PI_2).sin()}}
    pub fn all()->&'static [TransitionCurve]{&[TransitionCurve::Linear,TransitionCurve::SmoothStep,TransitionCurve::EaseIn,TransitionCurve::EaseOut,TransitionCurve::Sharp,TransitionCurve::Sine]}
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct BiomeTransition{pub from_biome:usize,pub to_biome:usize,pub transition_width:f32,pub blend_curve:TransitionCurve,pub ecotone_density:f32,pub name:String}
impl BiomeTransition{
    pub fn new(from:usize,to:usize)->Self{Self{from_biome:from,to_biome:to,transition_width:20.0,blend_curve:TransitionCurve::SmoothStep,ecotone_density:0.3,name:format!("Biome {} -> Biome {}",from,to)}}
    pub fn blend_weight(&self,dist:f32)->f32{if dist.abs()>=self.transition_width*0.5{return if dist<0.0{0.0}else{1.0};}self.blend_curve.evaluate((dist+self.transition_width*0.5)/self.transition_width)}
}

pub fn show_biome_transition_editor(ui:&mut egui::Ui,transitions:&mut Vec<BiomeTransition>,biome_names:&[String]){
    ui.heading("Biome Transitions");
    if ui.button("Add Transition").clicked(){transitions.push(BiomeTransition::new(0,1.min(biome_names.len().saturating_sub(1))));}
    let mut to_remove=None;
    for(i,tr) in transitions.iter_mut().enumerate(){
        ui.push_id(i,|ui|{
            egui::CollapsingHeader::new(&tr.name).show(ui,|ui|{
                ui.horizontal(|ui|{
                    ui.label("From:");
                    egui::ComboBox::from_id_salt("from_biome").selected_text(biome_names.get(tr.from_biome).map(|s|s.as_str()).unwrap_or("?")).show_ui(ui,|ui|{for(j,n)in biome_names.iter().enumerate(){ui.selectable_value(&mut tr.from_biome,j,n);}});
                    ui.label("To:");
                    egui::ComboBox::from_id_salt("to_biome").selected_text(biome_names.get(tr.to_biome).map(|s|s.as_str()).unwrap_or("?")).show_ui(ui,|ui|{for(j,n)in biome_names.iter().enumerate(){ui.selectable_value(&mut tr.to_biome,j,n);}});
                });
                ui.horizontal(|ui|{ui.label("Width:");ui.add(egui::DragValue::new(&mut tr.transition_width).range(1.0..=100.0));ui.label("Ecotone:");ui.add(egui::DragValue::new(&mut tr.ecotone_density).range(0.0..=1.0).speed(0.01));});
                ui.horizontal(|ui|{ui.label("Curve:");for curve in TransitionCurve::all(){if ui.selectable_label(tr.blend_curve==*curve,curve.name()).clicked(){tr.blend_curve=curve.clone();}}});
                let(rect,_)=ui.allocate_exact_size(Vec2::new(200.0,40.0),egui::Sense::hover());
                let p=ui.painter_at(rect);p.rect_filled(rect,2.0,Color32::from_gray(20));
                let mut prev:Option<Pos2>=None;
                for s in 0..=100usize{let t=s as f32/100.0;let yv=tr.blend_curve.evaluate(t);let cur=Pos2::new(rect.left()+t*rect.width(),rect.bottom()-yv*rect.height());if let Some(pp)=prev{p.line_segment([pp,cur],Stroke::new(1.5,Color32::from_rgb(100,200,100)));}prev=Some(cur);}
                if ui.button("Remove").clicked(){to_remove=Some(i);}
            });
        });
    }
    if let Some(idx)=to_remove{transitions.remove(idx);}
}

// =================================================================
// OVERWORLD ROAD NETWORK (extended)
// =================================================================

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub enum OverworldRoadClass{Dirt,Stone,Paved,Highway}
impl OverworldRoadClass{
    pub fn name(&self)->&str{match self{OverworldRoadClass::Dirt=>"Dirt",OverworldRoadClass::Stone=>"Stone",OverworldRoadClass::Paved=>"Paved",OverworldRoadClass::Highway=>"Highway"}}
    pub fn default_width(&self)->f32{match self{OverworldRoadClass::Dirt=>2.0,OverworldRoadClass::Stone=>4.0,OverworldRoadClass::Paved=>6.0,OverworldRoadClass::Highway=>12.0}}
    pub fn road_color(&self)->Color32{match self{OverworldRoadClass::Dirt=>Color32::from_rgb(160,130,90),OverworldRoadClass::Stone=>Color32::from_rgb(120,115,100),OverworldRoadClass::Paved=>Color32::from_rgb(80,80,80),OverworldRoadClass::Highway=>Color32::from_rgb(40,40,40)}}
    pub fn movement_cost_multiplier(&self)->f32{match self{OverworldRoadClass::Dirt=>0.8,OverworldRoadClass::Stone=>0.6,OverworldRoadClass::Paved=>0.4,OverworldRoadClass::Highway=>0.2}}
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct OverworldRoad{pub waypoints:Vec<[f32;2]>,pub road_class:OverworldRoadClass,pub width:f32,pub name:String}
impl OverworldRoad{
    pub fn new(rc:OverworldRoadClass)->Self{let w=rc.default_width();Self{waypoints:Vec::new(),road_class:rc,width:w,name:String::new()}}
    pub fn total_length(&self)->f32{if self.waypoints.len()<2{return 0.0;}self.waypoints.windows(2).map(|w|{let dx=w[1][0]-w[0][0];let dy=w[1][1]-w[0][1];(dx*dx+dy*dy).sqrt()}).sum()}
    pub fn draw_road(&self,painter:&Painter,world_to_screen:impl Fn([f32;2])->Pos2,scale:f32,selected:bool){
        if self.waypoints.len()<2{return;}
        let(sw,col)=((self.width*scale).max(1.0),if selected{Color32::YELLOW}else{self.road_class.road_color()});
        for seg in self.waypoints.windows(2){
            let(p0,p1)=(world_to_screen(seg[0]),world_to_screen(seg[1]));
            painter.line_segment([p0,p1],Stroke::new(sw,col));
            let len=((p1.x-p0.x).powi(2)+(p1.y-p0.y).powi(2)).sqrt();
            if len>10.0{let mid=Pos2::new((p0.x+p1.x)*0.5,(p0.y+p1.y)*0.5);painter.line_segment([p0,mid],Stroke::new(1.0,Color32::from_rgba_unmultiplied(255,255,255,80)));}
        }
    }
}

pub fn generate_overworld_road_network(chunk:&GeneratedChunk,settlements:&[[f32;2]],sea_level:f32,seed:u64,road_class:OverworldRoadClass)->Vec<OverworldRoad>{
    generate_road_network(chunk,settlements,sea_level,seed).into_iter().map(|r|{let mut ow=OverworldRoad::new(road_class.clone());ow.waypoints=r.nodes;ow}).collect()
}

// =================================================================
// WORLD EXPORT
// =================================================================

pub fn export_heightmap_json(chunk:&GeneratedChunk)->String{
    let mut rows=Vec::new();
    for y in 0..chunk.height{let mut row=Vec::new();for x in 0..chunk.width{row.push(format!("{:.4}",chunk.height_map[chunk.idx(x,y)]));}rows.push(format!("[{}]",row.join(",")));}
    format!("[{}]",rows.join(",\n"))
}
pub fn export_biome_map_colors(chunk:&GeneratedChunk,biome_colors:&[Color32])->Vec<[u8;4]>{
    (0..(chunk.width*chunk.height) as usize).map(|i|{let c=biome_colors.get(chunk.biome_map[i]).copied().unwrap_or(Color32::MAGENTA);[c.r(),c.g(),c.b(),255]}).collect()
}
pub fn export_entity_positions_csv(chunk:&GeneratedChunk)->String{
    let mut out=String::from("x,y,type\n");
    for(pos,ch,_) in &chunk.entity_positions{out.push_str(&format!("{:.2},{:.2},{}\n",pos[0],pos[1],ch));}
    out
}
pub fn export_dungeon_json(map:&DungeonMap)->String{
    let mut rows=Vec::new();
    for y in 0..map.height{let mut row=Vec::new();for x in 0..map.width{row.push(format!("\"{}\"",map.get(x,y).char_rep()));}rows.push(format!("[{}]",row.join(",")));}
    format!("{{\"width\":{},\"height\":{},\"tiles\":[{}]}}",map.width,map.height,rows.join(",\n"))
}

#[derive(Clone,Debug,Default,Serialize,Deserialize)]
pub struct WorldExportState{pub last_heightmap_json:String,pub last_entity_csv:String,pub last_dungeon_json:String,pub show_heightmap:bool,pub show_entities:bool,pub show_dungeon:bool}

pub fn show_world_export_panel(ui:&mut egui::Ui,chunk:&GeneratedChunk,dungeon:Option<&DungeonMap>,state:&mut WorldExportState){
    ui.heading("World Export");ui.separator();
    ui.horizontal(|ui|{
        if ui.button("Export Heightmap JSON").clicked(){state.last_heightmap_json=export_heightmap_json(chunk);state.show_heightmap=true;}
        if ui.button("Export Entity CSV").clicked(){state.last_entity_csv=export_entity_positions_csv(chunk);state.show_entities=true;}
        if let Some(d)=dungeon{if ui.button("Export Dungeon JSON").clicked(){state.last_dungeon_json=export_dungeon_json(d);state.show_dungeon=true;}}
    });
    ui.separator();
    if state.show_heightmap{ui.collapsing("Heightmap JSON",|ui|{egui::ScrollArea::vertical().max_height(200.0).show(ui,|ui|{ui.code(&state.last_heightmap_json);});});}
    if state.show_entities {ui.collapsing("Entity CSV",|ui|{egui::ScrollArea::vertical().max_height(200.0).show(ui,|ui|{ui.code(&state.last_entity_csv);});});}
    if state.show_dungeon  {ui.collapsing("Dungeon JSON",|ui|{egui::ScrollArea::vertical().max_height(200.0).show(ui,|ui|{ui.code(&state.last_dungeon_json);});});}
}

// City editor state
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct CityEditorState{pub config:CityConfig,pub layout:Option<CityLayout>,pub show_buildings:bool,pub show_districts:bool,pub zoom:f32,pub pan:[f32;2],pub seed:u64}
impl Default for CityEditorState{fn default()->Self{Self{config:CityConfig::default(),layout:None,show_buildings:true,show_districts:true,zoom:1.0,pan:[0.0,0.0],seed:42}}}
impl CityEditorState{
    pub fn generate_city_layout(&mut self,chunk:&GeneratedChunk){self.layout=Some(generate_city(&self.config,chunk,self.seed));}
    pub fn world_to_screen_city(&self,wp:[f32;2],cr:Rect)->Pos2{Pos2::new(cr.center().x+self.pan[0]+wp[0]*self.zoom,cr.center().y+self.pan[1]+wp[1]*self.zoom)}
}
pub fn show_city_editor(ui:&mut egui::Ui,state:&mut CityEditorState,chunk:&GeneratedChunk){
    ui.horizontal(|ui|{ui.heading("City Generator");if ui.button("Generate").clicked(){state.generate_city_layout(chunk);}ui.add(egui::DragValue::new(&mut state.seed).prefix("Seed: "));});
    ui.separator();
    egui::CollapsingHeader::new("City Config").show(ui,|ui|{
        ui.horizontal(|ui|{ui.label("Road style:");for style in CityRoadStyle::all(){if ui.selectable_label(state.config.road_style==*style,style.name()).clicked(){state.config.road_style=style.clone();}}});
        ui.horizontal(|ui|{ui.label("Density:");ui.add(egui::Slider::new(&mut state.config.population_density,0.0..=1.0));});
        ui.horizontal(|ui|{ui.label("Block size:");ui.add(egui::DragValue::new(&mut state.config.block_size).range(20.0..=200.0));});
        ui.horizontal(|ui|{ui.label("Radius:");ui.add(egui::DragValue::new(&mut state.config.city_radius).range(50.0..=2000.0));});
    });
    ui.horizontal(|ui|{ui.checkbox(&mut state.show_buildings,"Buildings");ui.checkbox(&mut state.show_districts,"Districts");});
    ui.separator();
    let avail=ui.available_size();
    let(response,painter)=ui.allocate_painter(avail,egui::Sense::click_and_drag());
    let cr=response.rect;
    painter.rect_filled(cr,0.0,Color32::from_gray(18));
    if let Some(ref layout)=state.layout{
        let sr=&*state;
        let draw_fn=move |wp:[f32;2]|sr.world_to_screen_city(wp,cr);
        draw_city_preview(&painter,layout,draw_fn,&state.config.district_types,state.show_buildings,state.show_districts);
    }else{painter.text(cr.center(),egui::Align2::CENTER_CENTER,"Click Generate to build a city",FontId::proportional(14.0),Color32::GRAY);}
    if response.dragged_by(egui::PointerButton::Primary){let d=response.drag_delta();state.pan[0]+=d.x;state.pan[1]+=d.y;}
    let scroll=ui.ctx().input(|i|i.smooth_scroll_delta.y);
    if scroll!=0.0{state.zoom=(state.zoom*(1.0+scroll*0.001)).clamp(0.05,10.0);}
}

// =================================================================
// ADDITIONAL TESTS
// =================================================================

#[cfg(test)]
mod new_world_tests {
    use super::*;

    #[test]
    fn test_generate_city_grid(){let cfg=CityConfig::default();let chunk=GeneratedChunk::new(64,64);let l=generate_city(&cfg,&chunk,42);assert!(!l.roads.is_empty());assert!(!l.blocks.is_empty());}
    #[test]
    fn test_generate_city_radial(){let mut cfg=CityConfig::default();cfg.road_style=CityRoadStyle::Radial;let chunk=GeneratedChunk::new(64,64);let l=generate_city(&cfg,&chunk,7);assert!(!l.roads.is_empty());}
    #[test]
    fn test_generate_city_organic(){let mut cfg=CityConfig::default();cfg.road_style=CityRoadStyle::Organic;let chunk=GeneratedChunk::new(64,64);let l=generate_city(&cfg,&chunk,99);assert!(!l.roads.is_empty());}
    #[test]
    fn test_generate_city_medieval(){let mut cfg=CityConfig::default();cfg.road_style=CityRoadStyle::Medieval;let chunk=GeneratedChunk::new(64,64);let l=generate_city(&cfg,&chunk,123);assert!(!l.roads.is_empty());}
    #[test]
    fn test_city_road_length(){let r=CityRoad::new([0.0,0.0],[3.0,4.0],CityRoadType::Main);assert!((r.length()-5.0).abs()<0.001);}
    #[test]
    fn test_city_road_direction_unit(){let r=CityRoad::new([0.0,0.0],[3.0,4.0],CityRoadType::Secondary);let d=r.direction();let l=(d[0]*d[0]+d[1]*d[1]).sqrt();assert!((l-1.0).abs()<0.001);}
    #[test]
    fn test_city_block_area(){let b=CityBlock::new(vec![[0.0,0.0],[10.0,0.0],[10.0,10.0],[0.0,10.0]],0);assert!((b.area()-100.0).abs()<0.001);}
    #[test]
    fn test_city_building_bounds(){let b=CityBuilding::new_rect(50.0,50.0,10.0,8.0,5.0,BuildingType::House);let r=b.bounds_rect();assert!((r.width()-10.0).abs()<0.01);assert!((r.height()-8.0).abs()<0.01);}
    #[test]
    fn test_bsp_dungeon_generates(){let cfg=DungeonConfig::default();let map=bsp_dungeon(&cfg,42);assert!(map.floor_count()>0);assert!(!map.rooms.is_empty());}
    #[test]
    fn test_cellular_dungeon(){let cfg=DungeonConfig{width:40,height:30,algorithm:DungeonAlgorithm::CellularAutomata,..Default::default()};let map=cellular_dungeon(&cfg,77);assert!(map.floor_count()>0);}
    #[test]
    fn test_drunkard_walk(){let cfg=DungeonConfig{width:40,height:30,..Default::default()};let map=drunkard_walk_dungeon(&cfg,55);assert!(map.floor_count()>0);}
    #[test]
    fn test_dungeon_rect_floor(){let mut map=DungeonMap::new(20,20,DungeonTheme::Stone);map.set_rect_floor(5,5,4,4);assert_eq!(*map.get(5,5),DungeonTile::Floor);assert_eq!(*map.get(4,4),DungeonTile::Wall);}
    #[test]
    fn test_dungeon_passability(){assert!(DungeonTile::Floor.is_passable());assert!(!DungeonTile::Wall.is_passable());assert!(!DungeonTile::Lava.is_passable());}
    #[test]
    fn test_dungeon_room_overlap(){let r1=DungeonRoom::new(0,0,5,5);let r2=DungeonRoom::new(3,3,5,5);let r3=DungeonRoom::new(10,10,5,5);assert!(r1.overlaps(&r2));assert!(!r1.overlaps(&r3));}
    #[test]
    fn test_room_type_assign(){assert_eq!(RoomType::assign_auto(0,10),RoomType::Start);assert_eq!(RoomType::assign_auto(9,10),RoomType::End);}
    #[test]
    fn test_cave_generation(){let cfg=CaveConfig{width:50,height:40,passes:3,..Default::default()};let map=generate_cave(&cfg,33);assert!(map.tiles.iter().any(|t|t.is_open()));}
    #[test]
    fn test_cave_tile_open(){assert!(CaveTile::Air.is_open());assert!(CaveTile::Water.is_open());assert!(!CaveTile::SolidRock.is_open());}
    #[test]
    fn test_cave_chambers(){let cfg=CaveConfig{width:60,height:50,passes:5,openness:0.45,..Default::default()};let map=generate_cave(&cfg,101);assert!(!map.chambers.is_empty());}
    #[test]
    fn test_transition_curve_endpoints(){for c in TransitionCurve::all(){assert!(c.evaluate(0.0)<=0.01);assert!(c.evaluate(1.0)>=0.99);}}
    #[test]
    fn test_biome_transition_blend(){let t=BiomeTransition::new(0,1);assert!(t.blend_weight(-t.transition_width)<0.01);assert!(t.blend_weight(t.transition_width)>0.99);}
    #[test]
    fn test_overworld_road_length(){let mut r=OverworldRoad::new(OverworldRoadClass::Paved);r.waypoints=vec![[0.0,0.0],[3.0,4.0],[6.0,8.0]];assert!((r.total_length()-10.0).abs()<0.01);}
    #[test]
    fn test_export_heightmap_json(){let cfg=crate::WorldGenConfig{width:4,height:4,..Default::default()};let chunk=crate::generate_chunk(&cfg,1);let json=export_heightmap_json(&chunk);assert!(json.starts_with('['));}
    #[test]
    fn test_export_entity_csv(){let mut chunk=GeneratedChunk::new(8,8);chunk.entity_positions.push(([1.0,2.0],'T',Color32::GREEN));let csv=export_entity_positions_csv(&chunk);assert!(csv.contains("x,y,type"));}
    #[test]
    fn test_export_dungeon_json(){let cfg=DungeonConfig{width:10,height:10,..Default::default()};let map=bsp_dungeon(&cfg,1);let json=export_dungeon_json(&map);assert!(json.contains("width"));}
    #[test]
    fn test_dungeon_editor_generate(){let mut s=DungeonEditorState::default();s.generate(42);assert!(s.map.is_some());}
    #[test]
    fn test_building_type_for_commercial(){let mut rng=SimpleRng::new(55);let bt=BuildingType::for_land_use(&LandUse::Commercial,&mut rng);let _=bt.name();}
    #[test]
    fn test_bsp_rooms_inside_map(){let cfg=DungeonConfig::default();let map=bsp_dungeon(&cfg,999);for room in &map.rooms{let(rx,ry,rw,rh)=room.rect;assert!(rx+rw<=cfg.width);assert!(ry+rh<=cfg.height);}}
    #[test]
    fn test_dungeon_theme_colors_distinct(){for t in DungeonTheme::all(){assert_ne!(t.wall_color(),t.floor_color());}}
    #[test]
    fn test_overworld_road_network_generates(){let chunk=GeneratedChunk::new(64,64);let settlements=[[10.0_f32,10.0],[50.0,50.0]];let roads=generate_overworld_road_network(&chunk,&settlements,0.3,42,OverworldRoadClass::Stone);assert!(!roads.is_empty());}
    #[test]
    fn test_dungeon_connectivity_smoke(){let cfg=DungeonConfig{width:30,height:25,..Default::default()};let map=bsp_dungeon(&cfg,13);let _=validate_dungeon_connectivity(&map);}
    #[test]
    fn test_overworld_road_color_distinct(){let classes=[OverworldRoadClass::Dirt,OverworldRoadClass::Stone,OverworldRoadClass::Paved,OverworldRoadClass::Highway];let colors:Vec<Color32>=classes.iter().map(|c|c.road_color()).collect();for i in 0..colors.len(){for j in(i+1)..colors.len(){assert_ne!(colors[i],colors[j]);}}}
}
'''

with open('C:/proof-engine/editor/src/world_gen.rs', 'a', encoding='utf-8') as f:
    f.write(code)

import os
size = os.path.getsize('C:/proof-engine/editor/src/world_gen.rs')
print(f"Done. File size: {size} bytes")
