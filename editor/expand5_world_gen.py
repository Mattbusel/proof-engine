
code = r'''

// ============================================================
// EXPANSION 5: Biome Transition System
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum BiomeTransitionMode {
    Hard,
    Gradual,
    Noise,
    Elevation,
    Moisture,
    Temperature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomeTransitionRule {
    pub from_biome: u32,
    pub to_biome: u32,
    pub mode: BiomeTransitionMode,
    pub blend_width: f32,
    pub blend_noise_scale: f32,
    pub elevation_threshold: f32,
    pub moisture_threshold: f32,
    pub temperature_threshold: f32,
}

impl BiomeTransitionRule {
    pub fn hard_border(from: u32, to: u32) -> Self {
        BiomeTransitionRule {
            from_biome: from, to_biome: to,
            mode: BiomeTransitionMode::Hard,
            blend_width: 0.0,
            blend_noise_scale: 0.05,
            elevation_threshold: 0.5,
            moisture_threshold: 0.5,
            temperature_threshold: 0.5,
        }
    }
    pub fn gradual_blend(from: u32, to: u32, width: f32) -> Self {
        BiomeTransitionRule {
            from_biome: from, to_biome: to,
            mode: BiomeTransitionMode::Gradual,
            blend_width: width,
            blend_noise_scale: 0.05,
            elevation_threshold: 0.5,
            moisture_threshold: 0.5,
            temperature_threshold: 0.5,
        }
    }
    pub fn transition_factor(&self, distance: f32, rng_val: f32) -> f32 {
        match self.mode {
            BiomeTransitionMode::Hard => if distance <= 0.0 { 0.0 } else { 1.0 },
            BiomeTransitionMode::Gradual => {
                (distance / self.blend_width.max(0.001)).clamp(0.0, 1.0)
            },
            BiomeTransitionMode::Noise => {
                let noisy = distance + (rng_val * 2.0 - 1.0) * self.blend_noise_scale;
                (noisy / self.blend_width.max(0.001)).clamp(0.0, 1.0)
            },
            _ => (distance / self.blend_width.max(0.001)).clamp(0.0, 1.0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomePalette {
    pub id: u32,
    pub name: String,
    pub base_color: [u8; 3],
    pub ground_texture: String,
    pub tree_density: f32,
    pub grass_density: f32,
    pub rock_density: f32,
    pub water_bodies: bool,
    pub fog_density: f32,
    pub ambient_sound: String,
    pub particle_effect: Option<String>,
    pub elevation_range: (f32, f32),
    pub moisture_range: (f32, f32),
    pub temperature_range: (f32, f32),
}

impl BiomePalette {
    pub fn forest() -> Self {
        BiomePalette {
            id: 0, name: "Temperate Forest".to_string(),
            base_color: [60, 100, 40],
            ground_texture: "grass_dirt".to_string(),
            tree_density: 0.7,
            grass_density: 0.8,
            rock_density: 0.1,
            water_bodies: true,
            fog_density: 0.2,
            ambient_sound: "forest_birds".to_string(),
            particle_effect: Some("leaves".to_string()),
            elevation_range: (0.3, 0.65),
            moisture_range: (0.5, 1.0),
            temperature_range: (0.3, 0.7),
        }
    }
    pub fn desert() -> Self {
        BiomePalette {
            id: 1, name: "Arid Desert".to_string(),
            base_color: [210, 180, 100],
            ground_texture: "sand".to_string(),
            tree_density: 0.02,
            grass_density: 0.05,
            rock_density: 0.3,
            water_bodies: false,
            fog_density: 0.05,
            ambient_sound: "wind".to_string(),
            particle_effect: Some("sand_particles".to_string()),
            elevation_range: (0.3, 0.55),
            moisture_range: (0.0, 0.2),
            temperature_range: (0.7, 1.0),
        }
    }
    pub fn tundra() -> Self {
        BiomePalette {
            id: 2, name: "Arctic Tundra".to_string(),
            base_color: [200, 220, 230],
            ground_texture: "snow_rock".to_string(),
            tree_density: 0.05,
            grass_density: 0.15,
            rock_density: 0.4,
            water_bodies: true,
            fog_density: 0.4,
            ambient_sound: "arctic_wind".to_string(),
            particle_effect: Some("snow".to_string()),
            elevation_range: (0.2, 0.9),
            moisture_range: (0.0, 0.5),
            temperature_range: (0.0, 0.2),
        }
    }
    pub fn tropical() -> Self {
        BiomePalette {
            id: 3, name: "Tropical Rainforest".to_string(),
            base_color: [20, 120, 30],
            ground_texture: "jungle_floor".to_string(),
            tree_density: 0.95,
            grass_density: 0.9,
            rock_density: 0.05,
            water_bodies: true,
            fog_density: 0.5,
            ambient_sound: "jungle_ambient".to_string(),
            particle_effect: Some("rain_drops".to_string()),
            elevation_range: (0.25, 0.5),
            moisture_range: (0.8, 1.0),
            temperature_range: (0.7, 1.0),
        }
    }
    pub fn is_suitable(&self, elevation: f32, moisture: f32, temperature: f32) -> bool {
        elevation >= self.elevation_range.0 && elevation <= self.elevation_range.1
            && moisture >= self.moisture_range.0 && moisture <= self.moisture_range.1
            && temperature >= self.temperature_range.0 && temperature <= self.temperature_range.1
    }
    pub fn suitability_score(&self, elevation: f32, moisture: f32, temperature: f32) -> f32 {
        let e = if elevation >= self.elevation_range.0 && elevation <= self.elevation_range.1 {
            1.0 - (elevation - (self.elevation_range.0 + self.elevation_range.1) * 0.5).abs()
                / ((self.elevation_range.1 - self.elevation_range.0) * 0.5 + 0.001)
        } else { 0.0 };
        let m = if moisture >= self.moisture_range.0 && moisture <= self.moisture_range.1 {
            1.0 - (moisture - (self.moisture_range.0 + self.moisture_range.1) * 0.5).abs()
                / ((self.moisture_range.1 - self.moisture_range.0) * 0.5 + 0.001)
        } else { 0.0 };
        let t = if temperature >= self.temperature_range.0 && temperature <= self.temperature_range.1 {
            1.0 - (temperature - (self.temperature_range.0 + self.temperature_range.1) * 0.5).abs()
                / ((self.temperature_range.1 - self.temperature_range.0) * 0.5 + 0.001)
        } else { 0.0 };
        (e + m + t) / 3.0
    }
}

pub fn assign_biomes(
    heightmap: &[Vec<f32>],
    moisture_map: &[Vec<f32>],
    temp_map: &[Vec<f32>],
    palettes: &[BiomePalette],
) -> Vec<Vec<u32>> {
    let h = heightmap.len();
    if h == 0 { return vec![]; }
    let w = heightmap[0].len();
    let mut biome_map = vec![vec![0u32; w]; h];

    for y in 0..h {
        for x in 0..w {
            let elev = heightmap[y][x];
            let moist = if y < moisture_map.len() && x < moisture_map[y].len() {
                moisture_map[y][x]
            } else { 0.5 };
            let temp = if y < temp_map.len() && x < temp_map[y].len() {
                temp_map[y][x]
            } else { 0.5 };

            let best = palettes.iter().enumerate()
                .max_by(|(_, a), (_, b)| {
                    a.suitability_score(elev, moist, temp)
                        .partial_cmp(&b.suitability_score(elev, moist, temp))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

            biome_map[y][x] = best.map(|(i, p)| p.id).unwrap_or(0);
        }
    }
    biome_map
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct BiomeSystemState {
    pub palettes: Vec<BiomePalette>,
    pub transitions: Vec<BiomeTransitionRule>,
    pub biome_map: Vec<Vec<u32>>,
    pub selected_palette: Option<usize>,
    pub selected_transition: Option<usize>,
    pub show_transition_zones: bool,
    pub show_biome_legend: bool,
}

impl BiomeSystemState {
    pub fn with_defaults() -> Self {
        BiomeSystemState {
            palettes: vec![
                BiomePalette::forest(),
                BiomePalette::desert(),
                BiomePalette::tundra(),
                BiomePalette::tropical(),
            ],
            transitions: vec![
                BiomeTransitionRule::gradual_blend(0, 1, 5.0),
                BiomeTransitionRule::gradual_blend(0, 2, 3.0),
            ],
            ..Default::default()
        }
    }
}

pub fn show_biome_system(ui: &mut egui::Ui, state: &mut BiomeSystemState) {
    ui.heading("Biome System");
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_transition_zones, "Transition Zones");
        ui.checkbox(&mut state.show_biome_legend, "Legend");
    });

    egui::CollapsingHeader::new("Biome Palettes").show(ui, |ui| {
        for (i, pal) in state.palettes.iter().enumerate() {
            let c = pal.base_color;
            let sel = state.selected_palette == Some(i);
            let label = egui::RichText::new(&pal.name)
                .color(egui::Color32::from_rgb(c[0], c[1], c[2]));
            if ui.selectable_label(sel, label).clicked() {
                state.selected_palette = Some(i);
            }
        }
        if ui.button("+ Add Forest").clicked() {
            let id = state.palettes.len() as u32;
            let mut p = BiomePalette::forest();
            p.id = id;
            p.name = format!("Forest{}", id);
            state.palettes.push(p);
        }
    });

    if let Some(idx) = state.selected_palette {
        if let Some(pal) = state.palettes.get_mut(idx) {
            egui::CollapsingHeader::new("Selected Palette").default_open(true).show(ui, |ui| {
                ui.text_edit_singleline(&mut pal.name);
                ui.add(egui::Slider::new(&mut pal.tree_density, 0.0..=1.0).text("Trees"));
                ui.add(egui::Slider::new(&mut pal.grass_density, 0.0..=1.0).text("Grass"));
                ui.add(egui::Slider::new(&mut pal.rock_density, 0.0..=1.0).text("Rocks"));
                ui.add(egui::Slider::new(&mut pal.fog_density, 0.0..=1.0).text("Fog"));
                ui.horizontal(|ui| {
                    ui.label("Elev:");
                    ui.add(egui::DragValue::new(&mut pal.elevation_range.0).speed(0.01));
                    ui.label("-");
                    ui.add(egui::DragValue::new(&mut pal.elevation_range.1).speed(0.01));
                });
                ui.horizontal(|ui| {
                    ui.label("Moisture:");
                    ui.add(egui::DragValue::new(&mut pal.moisture_range.0).speed(0.01));
                    ui.label("-");
                    ui.add(egui::DragValue::new(&mut pal.moisture_range.1).speed(0.01));
                });
                ui.horizontal(|ui| {
                    ui.label("Temp:");
                    ui.add(egui::DragValue::new(&mut pal.temperature_range.0).speed(0.01));
                    ui.label("-");
                    ui.add(egui::DragValue::new(&mut pal.temperature_range.1).speed(0.01));
                });
            });
        }
    }

    if state.show_biome_legend {
        ui.separator();
        ui.label("Legend:");
        for pal in &state.palettes {
            let c = pal.base_color;
            ui.horizontal(|ui| {
                let (resp, painter) = ui.allocate_painter(egui::vec2(16.0, 16.0), egui::Sense::hover());
                painter.rect_filled(resp.rect, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                ui.label(&pal.name);
            });
        }
    }
}

// ============================================================
// EXPANSION 5: Prophecy & Oracle System
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldProphecy {
    pub id: u32,
    pub text: String,
    pub given_year: i64,
    pub fulfillment_condition: ProphecyCondition,
    pub fulfilled: bool,
    pub fulfillment_year: Option<i64>,
    pub given_by: String,
    pub is_dark: bool,
    pub probability: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProphecyCondition {
    PopulationReaches(u64),
    FactionDestroyed(u32),
    VolcanoErupts,
    AncientArtefactFound,
    WarBetweenFactions(u32, u32),
    MagicSiteActivated,
    DragonAwakens,
    EmpireFalls,
    HeroEmerges,
    WorldPlagueStarts,
    Custom(String),
}

impl ProphecyCondition {
    pub fn description(&self) -> String {
        match self {
            ProphecyCondition::PopulationReaches(n) => format!("Population reaches {}", n),
            ProphecyCondition::FactionDestroyed(id) => format!("Faction {} is destroyed", id),
            ProphecyCondition::VolcanoErupts => "A great volcano erupts".to_string(),
            ProphecyCondition::AncientArtefactFound => "An ancient artefact is uncovered".to_string(),
            ProphecyCondition::WarBetweenFactions(a, b) => format!("War between factions {} and {}", a, b),
            ProphecyCondition::MagicSiteActivated => "A dormant magic site awakens".to_string(),
            ProphecyCondition::DragonAwakens => "The ancient dragon awakens from slumber".to_string(),
            ProphecyCondition::EmpireFalls => "The great empire crumbles".to_string(),
            ProphecyCondition::HeroEmerges => "A hero of destiny is born".to_string(),
            ProphecyCondition::WorldPlagueStarts => "A terrible plague spreads".to_string(),
            ProphecyCondition::Custom(s) => s.clone(),
        }
    }
}

impl WorldProphecy {
    pub fn new(id: u32, text: String, year: i64, condition: ProphecyCondition, giver: String) -> Self {
        WorldProphecy {
            id, text, given_year: year,
            fulfillment_condition: condition,
            fulfilled: false,
            fulfillment_year: None,
            given_by: giver,
            is_dark: false,
            probability: 0.7,
        }
    }
}

pub fn generate_prophecies(rng: &mut SimpleRng, year: i64) -> Vec<WorldProphecy> {
    let prophecy_texts = [
        "When the twin moons align, a great darkness shall consume the eastern lands.",
        "The child born under the crimson star shall either save or doom all.",
        "When rivers run dry and mountains crumble, the ancient ones shall return.",
        "Three kings will fall before a new age of peace can begin.",
        "The sleeping giant beneath the mountain shall wake when wars cease.",
        "An empire built on lies shall crumble in a single season.",
        "The chosen one carries the mark of the serpent and the dove.",
        "When the great tree in the heart of the world withers, so too shall all magic.",
    ];
    let givers = ["The Oracle of Valindra", "The Last Prophet", "The Dreaming Sage",
                  "The Elder of the Deep", "The Voice in the Mist"];
    let conditions = [
        ProphecyCondition::EmpireFalls,
        ProphecyCondition::HeroEmerges,
        ProphecyCondition::DragonAwakens,
        ProphecyCondition::VolcanoErupts,
        ProphecyCondition::WorldPlagueStarts,
        ProphecyCondition::MagicSiteActivated,
    ];

    let count = 2 + rng.next_u64() as usize % 4;
    let mut prophecies = vec![];
    for i in 0..count {
        let text = prophecy_texts[rng.next_u64() as usize % prophecy_texts.len()];
        let giver = givers[rng.next_u64() as usize % givers.len()];
        let cond = conditions[rng.next_u64() as usize % conditions.len()].clone();
        let dark = rng.next_u64() % 2 == 0;
        let mut p = WorldProphecy::new(i as u32, text.to_string(), year, cond, giver.to_string());
        p.is_dark = dark;
        p.probability = 0.4 + (rng.next_u64() as f32 / u64::MAX as f32) * 0.5;
        prophecies.push(p);
    }
    prophecies
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProphecyState {
    pub prophecies: Vec<WorldProphecy>,
    pub selected: Option<usize>,
    pub show_fulfilled: bool,
    pub show_dark_only: bool,
    pub filter_text: String,
}

pub fn show_prophecy_system(ui: &mut egui::Ui, state: &mut ProphecyState) {
    ui.heading("Prophecies & Oracles");
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_fulfilled, "Show Fulfilled");
        ui.checkbox(&mut state.show_dark_only, "Dark Only");
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.filter_text);
    });
    ui.label(format!("Total: {} | Fulfilled: {}",
        state.prophecies.len(),
        state.prophecies.iter().filter(|p| p.fulfilled).count()));

    let filter = state.filter_text.to_lowercase();
    egui::ScrollArea::vertical().show(ui, |ui| {
        let indices: Vec<usize> = state.prophecies.iter().enumerate()
            .filter(|(_, p)| (state.show_fulfilled || !p.fulfilled))
            .filter(|(_, p)| (!state.show_dark_only || p.is_dark))
            .filter(|(_, p)| filter.is_empty() || p.text.to_lowercase().contains(&filter))
            .map(|(i, _)| i)
            .collect();
        for idx in indices {
            let p = &state.prophecies[idx];
            let color = if p.fulfilled { egui::Color32::GRAY }
                else if p.is_dark { egui::Color32::from_rgb(200, 80, 80) }
                else { egui::Color32::from_rgb(200, 180, 120) };
            let sel = state.selected == Some(idx);
            if ui.selectable_label(sel,
                egui::RichText::new(format!("[{}] {}", p.given_year, &p.text[..p.text.len().min(60)]))
                    .color(color)).clicked() {
                state.selected = Some(idx);
            }
        }
    });

    if let Some(idx) = state.selected {
        if let Some(p) = state.prophecies.get(idx) {
            ui.separator();
            ui.label(format!("Given in year {} by {}", p.given_year, p.given_by));
            ui.label(p.text.clone());
            ui.label(format!("Condition: {}", p.fulfillment_condition.description()));
            ui.label(format!("Probability: {:.0}%", p.probability * 100.0));
            if p.fulfilled {
                ui.colored_label(egui::Color32::GREEN,
                    format!("FULFILLED in year {}", p.fulfillment_year.unwrap_or(0)));
            }
        }
    }
}

// ============================================================
// EXPANSION 5: Myth & Legend Generator
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldMyth {
    pub id: u32,
    pub title: String,
    pub myth_type: MythType,
    pub content: String,
    pub protagonist: String,
    pub antagonist: Option<String>,
    pub setting: String,
    pub moral: Option<String>,
    pub age_years: u32,
    pub believed_by_factions: Vec<u32>,
    pub is_historical: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MythType {
    CreationMyth,
    HeroJourney,
    ForbiddenLove,
    MonsterSlaying,
    BetrayalOfGods,
    WarOfAges,
    LostKingdom,
    ProphecyFulfilled,
    SacrificeForGreaterGood,
    CurseAndRedemption,
}

impl MythType {
    pub fn name(&self) -> &'static str {
        match self {
            MythType::CreationMyth => "Creation Myth",
            MythType::HeroJourney => "Hero's Journey",
            MythType::ForbiddenLove => "Forbidden Love",
            MythType::MonsterSlaying => "Monster Slaying",
            MythType::BetrayalOfGods => "Betrayal of Gods",
            MythType::WarOfAges => "War of Ages",
            MythType::LostKingdom => "Lost Kingdom",
            MythType::ProphecyFulfilled => "Prophecy Fulfilled",
            MythType::SacrificeForGreaterGood => "Noble Sacrifice",
            MythType::CurseAndRedemption => "Curse & Redemption",
        }
    }
}

pub fn generate_myths(rng: &mut SimpleRng, world_age: u32) -> Vec<WorldMyth> {
    let heroes = ["Aldric the Bold", "Seraphina Moonwhisper", "Theron of the Iron Keep",
                  "Lyra the Wanderer", "Galdur Stoneheart", "Mira the Enchantress"];
    let antagonists = ["The Shadow Dragon Malachar", "The Lich King Vordrath",
                       "The Betrayer Cassian", "The Demon Lord Xarath"];
    let settings = ["the Sunken City of Aral", "the Eternal Mountain of Velmor",
                    "the Cursed Forest of Grimhallow", "the Desert of Endless Glass"];
    let morals = [
        "True courage is not the absence of fear, but acting despite it.",
        "Power without wisdom leads only to destruction.",
        "Every sacrifice plants the seeds of a better tomorrow.",
        "Even the darkest night gives way to dawn.",
    ];

    let mut myths = vec![];
    let count = 3 + rng.next_u64() as usize % 5;
    let types = [
        MythType::CreationMyth, MythType::HeroJourney, MythType::MonsterSlaying,
        MythType::LostKingdom, MythType::CurseAndRedemption, MythType::WarOfAges,
    ];
    for i in 0..count {
        let hero = heroes[rng.next_u64() as usize % heroes.len()];
        let antagIdx = rng.next_u64() as usize % (antagonists.len() + 1);
        let antag = if antagIdx < antagonists.len() { Some(antagonists[antagIdx].to_string()) } else { None };
        let setting = settings[rng.next_u64() as usize % settings.len()];
        let moral = if rng.next_u64() % 2 == 0 {
            Some(morals[rng.next_u64() as usize % morals.len()].to_string())
        } else { None };
        let mt = types[rng.next_u64() as usize % types.len()].clone();
        let age = (world_age / 10 + (rng.next_u64() as u32 % (world_age / 5 + 1))).min(world_age);

        let content = match &mt {
            MythType::HeroJourney => format!(
                "In the age before memory, {} set out from their homeland to journey to {}. \
                 Armed with nothing but wit and courage, they faced trials that would break lesser souls. \
                 After many years of hardship, they returned forever changed, bearing gifts of wisdom.",
                hero, setting),
            MythType::MonsterSlaying => format!(
                "Long before the current age, {} slew the great beast that terrorized {}. \
                 Three weapons broke against its hide before the final blow was struck, \
                 and the hero's name was forever etched in stone.",
                hero, setting),
            MythType::LostKingdom => format!(
                "There once existed a magnificent kingdom at {}, built over ten generations. \
                 At its height, none could surpass its glory. Then came a single fateful choice \
                 that doomed it to be swallowed by the earth.",
                setting),
            _ => format!(
                "The ancient tale of {} at {} is told differently by each generation, \
                 but all agree that it changed the world forever.",
                hero, setting),
        };

        myths.push(WorldMyth {
            id: i as u32,
            title: format!("The Legend of {}", hero),
            myth_type: mt,
            content,
            protagonist: hero.to_string(),
            antagonist: antag,
            setting: setting.to_string(),
            moral,
            age_years: age,
            believed_by_factions: vec![],
            is_historical: rng.next_u64() % 3 == 0,
        });
    }
    myths
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MythLoreState {
    pub myths: Vec<WorldMyth>,
    pub selected: Option<usize>,
    pub filter_type: Option<String>,
    pub show_historical_only: bool,
}

pub fn show_myth_lore(ui: &mut egui::Ui, state: &mut MythLoreState) {
    ui.heading("Myths & Legends");
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_historical_only, "Historical Only");
        ui.label(format!("Total: {}", state.myths.len()));
    });

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        let indices: Vec<usize> = state.myths.iter().enumerate()
            .filter(|(_, m)| !state.show_historical_only || m.is_historical)
            .map(|(i, _)| i)
            .collect();
        for idx in indices {
            let m = &state.myths[idx];
            let sel = state.selected == Some(idx);
            if ui.selectable_label(sel,
                format!("[{}] {} - {}", m.age_years, m.title, m.myth_type.name())).clicked() {
                state.selected = Some(idx);
            }
        }
    });

    if let Some(idx) = state.selected {
        if let Some(m) = state.myths.get(idx) {
            ui.separator();
            ui.heading(&m.title.clone());
            ui.label(format!("Type: {} | Age: {} years ago", m.myth_type.name(), m.age_years));
            if m.is_historical { ui.colored_label(egui::Color32::YELLOW, "Historical Record"); }
            ui.label(&m.content.clone());
            if let Some(moral) = &m.moral.clone() {
                ui.separator();
                ui.label(format!("Moral: {}", moral));
            }
        }
    }
}

// ============================================================
// EXPANSION 5: World Timeline Viewer
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub year: i64,
    pub title: String,
    pub description: String,
    pub event_type: TimelineEventType,
    pub magnitude: f32,
    pub faction_id: Option<u32>,
    pub location: Option<(f32, f32)>,
    pub is_player_caused: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TimelineEventType {
    War,
    Peace,
    FoundedCity,
    DestroyedCity,
    NaturalDisaster,
    MagicEvent,
    PoliticalShift,
    Discovery,
    Migration,
    Plague,
    FactionFormed,
    FactionDissolved,
    ProphecyFulfilled,
    AncientAwakening,
    WorldChange,
}

impl TimelineEventType {
    pub fn icon(&self) -> &'static str {
        match self {
            TimelineEventType::War => "⚔",
            TimelineEventType::Peace => "☮",
            TimelineEventType::FoundedCity => "🏛",
            TimelineEventType::DestroyedCity => "💥",
            TimelineEventType::NaturalDisaster => "🌋",
            TimelineEventType::MagicEvent => "✨",
            TimelineEventType::PoliticalShift => "👑",
            TimelineEventType::Discovery => "🔍",
            TimelineEventType::Migration => "→",
            TimelineEventType::Plague => "☠",
            TimelineEventType::FactionFormed => "+",
            TimelineEventType::FactionDissolved => "×",
            TimelineEventType::ProphecyFulfilled => "📜",
            TimelineEventType::AncientAwakening => "☆",
            TimelineEventType::WorldChange => "🌍",
        }
    }
    pub fn color(&self) -> egui::Color32 {
        match self {
            TimelineEventType::War | TimelineEventType::DestroyedCity => egui::Color32::from_rgb(200, 50, 50),
            TimelineEventType::Peace | TimelineEventType::FoundedCity => egui::Color32::from_rgb(50, 200, 50),
            TimelineEventType::NaturalDisaster => egui::Color32::from_rgb(200, 100, 20),
            TimelineEventType::MagicEvent | TimelineEventType::AncientAwakening => egui::Color32::from_rgb(150, 50, 220),
            TimelineEventType::Plague => egui::Color32::from_rgb(80, 180, 80),
            TimelineEventType::PoliticalShift => egui::Color32::from_rgb(200, 200, 50),
            TimelineEventType::Discovery => egui::Color32::from_rgb(50, 150, 220),
            _ => egui::Color32::from_gray(180),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TimelineState {
    pub events: Vec<TimelineEvent>,
    pub view_start_year: i64,
    pub view_end_year: i64,
    pub selected_event: Option<usize>,
    pub filter_types: Vec<TimelineEventType>,
    pub show_magnitude_scale: bool,
    pub timeline_zoom: f32,
    pub show_faction_filter: Option<u32>,
}

impl TimelineState {
    pub fn new() -> Self {
        TimelineState {
            view_start_year: -1000,
            view_end_year: 1000,
            timeline_zoom: 1.0,
            ..Default::default()
        }
    }

    pub fn add_event(&mut self, e: TimelineEvent) {
        let pos = self.events.iter().position(|ev| ev.year > e.year).unwrap_or(self.events.len());
        self.events.insert(pos, e);
    }

    pub fn events_in_range(&self, start: i64, end: i64) -> Vec<&TimelineEvent> {
        self.events.iter().filter(|e| e.year >= start && e.year <= end).collect()
    }

    pub fn years_span(&self) -> i64 { self.view_end_year - self.view_start_year }
}

pub fn show_timeline_viewer(ui: &mut egui::Ui, state: &mut TimelineState) {
    ui.heading("World Timeline");
    ui.horizontal(|ui| {
        ui.label("View:");
        ui.add(egui::DragValue::new(&mut state.view_start_year).prefix("from year "));
        ui.label("to");
        ui.add(egui::DragValue::new(&mut state.view_end_year).prefix("year "));
        ui.add(egui::Slider::new(&mut state.timeline_zoom, 0.1..=10.0).text("Zoom"));
        ui.checkbox(&mut state.show_magnitude_scale, "Scale by Magnitude");
    });

    let in_range = state.events_in_range(state.view_start_year, state.view_end_year);
    ui.label(format!("Events in view: {}/{}", in_range.len(), state.events.len()));

    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter_at(rect);
    let span = (state.view_end_year - state.view_start_year).max(1) as f32;

    // Draw timeline axis
    painter.line_segment(
        [egui::pos2(rect.min.x, rect.center().y), egui::pos2(rect.max.x, rect.center().y)],
        egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
    );

    // Draw year markers
    let tick_interval = (span / 10.0) as i64;
    if tick_interval > 0 {
        let mut y = state.view_start_year - (state.view_start_year % tick_interval);
        while y <= state.view_end_year {
            let t = (y - state.view_start_year) as f32 / span;
            let x = rect.min.x + rect.width() * t;
            painter.line_segment(
                [egui::pos2(x, rect.center().y - 5.0), egui::pos2(x, rect.center().y + 5.0)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
            );
            painter.text(egui::pos2(x, rect.center().y + 10.0),
                egui::Align2::CENTER_TOP, y.to_string(),
                egui::FontId::proportional(9.0), egui::Color32::from_gray(150));
            y += tick_interval;
        }
    }

    // Draw events
    let events_in_range: Vec<(usize, i64, TimelineEventType, f32)> = state.events.iter().enumerate()
        .filter(|(_, e)| e.year >= state.view_start_year && e.year <= state.view_end_year)
        .map(|(i, e)| (i, e.year, e.event_type.clone(), e.magnitude))
        .collect();

    for (i, year, event_type, magnitude) in events_in_range {
        let t = (year - state.view_start_year) as f32 / span;
        let x = rect.min.x + rect.width() * t;
        let radius = if state.show_magnitude_scale { 3.0 + magnitude * 8.0 } else { 5.0 };
        let color = event_type.color();
        let y = rect.center().y;
        painter.circle_filled(egui::pos2(x, y), radius, color);
        let is_sel = state.selected_event == Some(i);
        if is_sel {
            painter.circle_stroke(egui::pos2(x, y), radius + 2.0,
                egui::Stroke::new(1.5, egui::Color32::WHITE));
        }
    }

    ui.allocate_rect(rect, egui::Sense::click());

    if let Some(idx) = state.selected_event {
        if let Some(e) = state.events.get(idx) {
            ui.separator();
            ui.label(format!("Year {}: {}", e.year, e.title));
            ui.label(&e.description.clone());
            ui.label(format!("Type: {:?}", e.event_type));
            ui.label(format!("Magnitude: {:.1}", e.magnitude));
        }
    }
}

// ============================================================
// EXPANSION 5: Tests
// ============================================================

#[cfg(test)]
mod world_gen_expansion5_tests {
    use super::*;

    #[test]
    fn test_biome_suitability() {
        let forest = BiomePalette::forest();
        let score = forest.suitability_score(0.5, 0.8, 0.5);
        assert!(score > 0.5);
        let bad_score = forest.suitability_score(0.0, 0.0, 1.0);
        assert!(bad_score == 0.0);
    }

    #[test]
    fn test_biome_assignment() {
        let hmap = vec![vec![0.5f32; 4]; 4];
        let mmap = vec![vec![0.7f32; 4]; 4];
        let tmap = vec![vec![0.5f32; 4]; 4];
        let palettes = vec![BiomePalette::forest(), BiomePalette::desert()];
        let biomes = assign_biomes(&hmap, &mmap, &tmap, &palettes);
        assert_eq!(biomes.len(), 4);
        assert_eq!(biomes[0].len(), 4);
    }

    #[test]
    fn test_prophecy_generation() {
        let mut rng = SimpleRng::new(42);
        let prophecies = generate_prophecies(&mut rng, 500);
        assert!(!prophecies.is_empty());
        for p in &prophecies {
            assert!(!p.text.is_empty());
            assert!(!p.given_by.is_empty());
        }
    }

    #[test]
    fn test_myth_generation() {
        let mut rng = SimpleRng::new(123);
        let myths = generate_myths(&mut rng, 5000);
        assert!(!myths.is_empty());
        for m in &myths {
            assert!(!m.title.is_empty());
            assert!(!m.content.is_empty());
        }
    }

    #[test]
    fn test_timeline_event_ordering() {
        let mut state = TimelineState::new();
        state.add_event(TimelineEvent {
            year: 100, title: "Second".to_string(), description: "".to_string(),
            event_type: TimelineEventType::War, magnitude: 0.5,
            faction_id: None, location: None, is_player_caused: false,
        });
        state.add_event(TimelineEvent {
            year: 50, title: "First".to_string(), description: "".to_string(),
            event_type: TimelineEventType::Peace, magnitude: 0.3,
            faction_id: None, location: None, is_player_caused: false,
        });
        assert_eq!(state.events[0].year, 50);
        assert_eq!(state.events[1].year, 100);
    }

    #[test]
    fn test_transition_rule_factor() {
        let rule = BiomeTransitionRule::gradual_blend(0, 1, 10.0);
        assert!((rule.transition_factor(0.0, 0.5) - 0.0).abs() < 0.001);
        assert!((rule.transition_factor(10.0, 0.5) - 1.0).abs() < 0.001);
        let mid = rule.transition_factor(5.0, 0.5);
        assert!((mid - 0.5).abs() < 0.001);
    }
}
'''

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'a', encoding='utf-8') as f:
    f.write(code)

import os
size = os.path.getsize(r'C:\proof-engine\editor\src\world_gen.rs')
print(f"world_gen.rs: {size} bytes")
