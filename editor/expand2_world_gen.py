code = r'''

// =================================================================
// WORLD GEN EXPANSION 2: FACTION & CULTURE SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CultureType {
    Nordic,
    Mediterranean,
    Eastern,
    Desert,
    Forest,
    Coastal,
    Mountain,
    Steppe,
    Jungle,
    Arctic,
}

impl CultureType {
    pub fn name(&self) -> &'static str {
        match self {
            CultureType::Nordic => "Nordic",
            CultureType::Mediterranean => "Mediterranean",
            CultureType::Eastern => "Eastern",
            CultureType::Desert => "Desert",
            CultureType::Forest => "Forest Dweller",
            CultureType::Coastal => "Coastal",
            CultureType::Mountain => "Mountain Folk",
            CultureType::Steppe => "Steppe Nomad",
            CultureType::Jungle => "Jungle Tribe",
            CultureType::Arctic => "Arctic Clan",
        }
    }

    pub fn primary_resource(&self) -> &'static str {
        match self {
            CultureType::Nordic => "Timber",
            CultureType::Mediterranean => "Grain",
            CultureType::Eastern => "Silk",
            CultureType::Desert => "Gold",
            CultureType::Forest => "Herbs",
            CultureType::Coastal => "Fish",
            CultureType::Mountain => "Iron Ore",
            CultureType::Steppe => "Horses",
            CultureType::Jungle => "Spices",
            CultureType::Arctic => "Furs",
        }
    }

    pub fn architecture_style(&self) -> &'static str {
        match self {
            CultureType::Nordic => "Longhouse",
            CultureType::Mediterranean => "Marble Colonnades",
            CultureType::Eastern => "Pagoda",
            CultureType::Desert => "Sandstone Citadels",
            CultureType::Forest => "Treehouse",
            CultureType::Coastal => "Stilt Houses",
            CultureType::Mountain => "Stone Keeps",
            CultureType::Steppe => "Yurt Camps",
            CultureType::Jungle => "Pyramid Temples",
            CultureType::Arctic => "Ice Halls",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Faction {
    pub id: u32,
    pub name: String,
    pub culture: CultureType,
    pub color: Color32,
    pub population: u64,
    pub wealth: f64,
    pub military_power: f32,
    pub technology_level: u32,
    pub reputation: i32,
    pub controlling_region: Option<usize>,
    pub leaders: Vec<FactionLeader>,
    pub traits: Vec<FactionTrait>,
    pub allies: Vec<u32>,
    pub enemies: Vec<u32>,
    pub description: String,
    pub founding_year: i32,
    pub capital_name: String,
}

impl Faction {
    pub fn new(id: u32, name: &str, culture: CultureType) -> Self {
        Self {
            id, name: name.to_string(), culture, color: Color32::from_rgb(150, 150, 200),
            population: 10000, wealth: 1000.0, military_power: 0.5, technology_level: 1,
            reputation: 0, controlling_region: None, leaders: Vec::new(), traits: Vec::new(),
            allies: Vec::new(), enemies: Vec::new(), description: String::new(),
            founding_year: -100, capital_name: String::new(),
        }
    }

    pub fn is_allied_with(&self, other_id: u32) -> bool { self.allies.contains(&other_id) }
    pub fn is_enemy_of(&self, other_id: u32) -> bool { self.enemies.contains(&other_id) }

    pub fn add_ally(&mut self, other_id: u32) {
        if !self.allies.contains(&other_id) { self.allies.push(other_id); }
        self.enemies.retain(|&e| e != other_id);
    }

    pub fn declare_war(&mut self, other_id: u32) {
        if !self.enemies.contains(&other_id) { self.enemies.push(other_id); }
        self.allies.retain(|&a| a != other_id);
    }

    pub fn military_strength(&self) -> f64 {
        self.population as f64 * self.military_power as f64 * self.technology_level as f64
    }

    pub fn economic_output(&self) -> f64 {
        self.wealth * 0.1 + self.population as f64 * 0.001
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FactionLeader {
    pub name: String,
    pub title: String,
    pub age: u32,
    pub charisma: u32,
    pub intelligence: u32,
    pub warfare: u32,
    pub diplomacy: u32,
    pub stewardship: u32,
    pub traits: Vec<String>,
}

impl FactionLeader {
    pub fn new(name: &str, title: &str) -> Self {
        Self { name: name.to_string(), title: title.to_string(), age: 35, charisma: 10, intelligence: 10, warfare: 10, diplomacy: 10, stewardship: 10, traits: Vec::new() }
    }

    pub fn effective_power(&self) -> u32 {
        self.charisma + self.intelligence + self.warfare
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum FactionTrait {
    Aggressive,
    Diplomatic,
    Isolationist,
    Expansionist,
    Mercantile,
    Religious,
    Scholarly,
    Militaristic,
    Pacifist,
    Nomadic,
}

impl FactionTrait {
    pub fn name(&self) -> &'static str {
        match self {
            FactionTrait::Aggressive => "Aggressive",
            FactionTrait::Diplomatic => "Diplomatic",
            FactionTrait::Isolationist => "Isolationist",
            FactionTrait::Expansionist => "Expansionist",
            FactionTrait::Mercantile => "Mercantile",
            FactionTrait::Religious => "Religious",
            FactionTrait::Scholarly => "Scholarly",
            FactionTrait::Militaristic => "Militaristic",
            FactionTrait::Pacifist => "Pacifist",
            FactionTrait::Nomadic => "Nomadic",
        }
    }

    pub fn military_modifier(&self) -> f32 {
        match self {
            FactionTrait::Aggressive | FactionTrait::Militaristic | FactionTrait::Expansionist => 1.3,
            FactionTrait::Pacifist | FactionTrait::Scholarly => 0.7,
            _ => 1.0,
        }
    }

    pub fn economy_modifier(&self) -> f32 {
        match self {
            FactionTrait::Mercantile | FactionTrait::Scholarly => 1.3,
            FactionTrait::Nomadic | FactionTrait::Isolationist => 0.8,
            _ => 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FactionEditorState {
    pub factions: Vec<Faction>,
    pub selected: Option<usize>,
    pub show_relationships: bool,
    pub show_territory: bool,
    pub filter_culture: Option<CultureType>,
}

pub fn generate_factions(count: u32, rng: &mut SimpleRng) -> Vec<Faction> {
    let faction_names = [
        "Ironclad Brotherhood", "Silver Wolves", "Azure Covenant", "Red Talon Clan",
        "Obsidian Order", "Golden Path", "Storm Hammers", "Night Foxes",
        "Earthborn Pact", "Celestial Court", "Verdant Circle", "Ember Fang",
    ];
    let cultures = [
        CultureType::Nordic, CultureType::Mediterranean, CultureType::Eastern,
        CultureType::Desert, CultureType::Forest, CultureType::Coastal,
        CultureType::Mountain, CultureType::Steppe, CultureType::Jungle, CultureType::Arctic,
    ];
    let colors = [
        Color32::from_rgb(200, 80, 80), Color32::from_rgb(80, 150, 200),
        Color32::from_rgb(80, 200, 100), Color32::from_rgb(200, 180, 50),
        Color32::from_rgb(150, 80, 200), Color32::from_rgb(200, 120, 50),
        Color32::from_rgb(80, 200, 200), Color32::from_rgb(200, 50, 150),
    ];

    let all_traits = [
        FactionTrait::Aggressive, FactionTrait::Diplomatic, FactionTrait::Isolationist,
        FactionTrait::Expansionist, FactionTrait::Mercantile, FactionTrait::Religious,
        FactionTrait::Scholarly, FactionTrait::Militaristic, FactionTrait::Pacifist,
        FactionTrait::Nomadic,
    ];

    let mut factions: Vec<Faction> = (0..count as usize).map(|i| {
        let name = faction_names[i % faction_names.len()];
        let culture = cultures[rng.next_u64() as usize % cultures.len()].clone();
        let mut f = Faction::new(i as u32, name, culture);
        f.color = colors[i % colors.len()];
        f.population = 5000 + (rng.next_u64() % 100000);
        f.wealth = 500.0 + rng.next_f32() as f64 * 10000.0;
        f.military_power = rng.next_f32_range(0.2, 1.0);
        f.technology_level = 1 + (rng.next_u64() % 5) as u32;
        f.reputation = rng.next_f32_range(-100.0, 100.0) as i32;

        // Assign 1-3 traits
        let trait_count = 1 + rng.next_u64() as usize % 3;
        for _ in 0..trait_count {
            let t = all_traits[rng.next_u64() as usize % all_traits.len()].clone();
            if !f.traits.contains(&t) { f.traits.push(t); }
        }

        // Generate a leader
        let leader_names = ["Aldric", "Brenna", "Corvus", "Dwyn", "Elara", "Finn", "Gael", "Hilda"];
        let titles = ["King", "Queen", "Warlord", "Chancellor", "High Priest", "Commander", "Elder"];
        let leader_name = leader_names[rng.next_u64() as usize % leader_names.len()];
        let title = titles[rng.next_u64() as usize % titles.len()];
        let mut leader = FactionLeader::new(leader_name, title);
        leader.age = 25 + (rng.next_u64() % 50) as u32;
        leader.charisma = 5 + (rng.next_u64() % 15) as u32;
        leader.warfare = 5 + (rng.next_u64() % 15) as u32;
        leader.intelligence = 5 + (rng.next_u64() % 15) as u32;
        f.leaders.push(leader);
        f
    }).collect();

    // Set up random alliances and enemies
    let total = factions.len();
    for i in 0..total {
        let num_allies = rng.next_u64() as usize % 3;
        let num_enemies = rng.next_u64() as usize % 2;
        for _ in 0..num_allies {
            let j = rng.next_u64() as usize % total;
            if j != i { factions[i].add_ally(j as u32); }
        }
        for _ in 0..num_enemies {
            let j = rng.next_u64() as usize % total;
            if j != i { factions[i].declare_war(j as u32); }
        }
    }
    factions
}

pub fn show_faction_editor(ui: &mut egui::Ui, state: &mut FactionEditorState) {
    ui.horizontal(|ui| {
        ui.label("Faction Manager");
        if ui.button("Generate Factions").clicked() {
            let mut rng = SimpleRng::new(12345);
            state.factions = generate_factions(8, &mut rng);
        }
        ui.checkbox(&mut state.show_relationships, "Relationships");
        ui.checkbox(&mut state.show_territory, "Territory");
    });

    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for (i, faction) in state.factions.iter().enumerate() {
            let sel = state.selected == Some(i);
            let label = egui::RichText::new(format!("{} ({})", faction.name, faction.culture.name()))
                .color(faction.color);
            if ui.selectable_label(sel, label).clicked() { state.selected = Some(i); }
        }
    });

    if let Some(idx) = state.selected {
        if let Some(f) = state.factions.get_mut(idx) {
            ui.separator();
            ui.text_edit_singleline(&mut f.name);
            ui.label(format!("Culture: {} | Pop: {} | Wealth: {:.0}", f.culture.name(), f.population, f.wealth));
            ui.add(egui::Slider::new(&mut f.military_power, 0.0..=1.0).text("Military Power"));
            ui.add(egui::DragValue::new(&mut f.technology_level).clamp_range(1..=10u32).prefix("Tech Level: "));
            ui.label(format!("Allies: {:?}", f.allies));
            ui.label(format!("Enemies: {:?}", f.enemies));
            ui.label("Traits:");
            for t in &f.traits { ui.label(format!("  - {}", t.name())); }
            if let Some(leader) = f.leaders.first() {
                ui.separator();
                ui.label(format!("Leader: {} {} (age {})", leader.title, leader.name, leader.age));
                ui.label(format!("  CHR:{} INT:{} WAR:{}", leader.charisma, leader.intelligence, leader.warfare));
            }
        }
    }
}

// =================================================================
// WORLD GEN EXPANSION 2: HISTORICAL EVENT SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum HistoricalEventType {
    War,
    Alliance,
    Discovery,
    Plague,
    NaturalDisaster,
    Coronation,
    Revolution,
    Migration,
    TechBreakthrough,
    ReligiousSchism,
    FamousBattle,
    CityFounded,
    TradeRoute,
    Assassination,
}

impl HistoricalEventType {
    pub fn name(&self) -> &'static str {
        match self {
            HistoricalEventType::War => "War",
            HistoricalEventType::Alliance => "Alliance",
            HistoricalEventType::Discovery => "Discovery",
            HistoricalEventType::Plague => "Plague",
            HistoricalEventType::NaturalDisaster => "Natural Disaster",
            HistoricalEventType::Coronation => "Coronation",
            HistoricalEventType::Revolution => "Revolution",
            HistoricalEventType::Migration => "Migration",
            HistoricalEventType::TechBreakthrough => "Tech Breakthrough",
            HistoricalEventType::ReligiousSchism => "Religious Schism",
            HistoricalEventType::FamousBattle => "Famous Battle",
            HistoricalEventType::CityFounded => "City Founded",
            HistoricalEventType::TradeRoute => "Trade Route Established",
            HistoricalEventType::Assassination => "Assassination",
        }
    }

    pub fn significance(&self) -> u32 {
        match self {
            HistoricalEventType::War | HistoricalEventType::Revolution => 5,
            HistoricalEventType::Plague | HistoricalEventType::NaturalDisaster => 4,
            HistoricalEventType::FamousBattle | HistoricalEventType::Assassination => 4,
            HistoricalEventType::Alliance | HistoricalEventType::TechBreakthrough => 3,
            HistoricalEventType::CityFounded | HistoricalEventType::Migration => 2,
            HistoricalEventType::Discovery | HistoricalEventType::TradeRoute => 2,
            _ => 1,
        }
    }

    pub fn icon(&self) -> char {
        match self {
            HistoricalEventType::War | HistoricalEventType::FamousBattle => '⚔',
            HistoricalEventType::Alliance => '🤝',
            HistoricalEventType::Plague => '☠',
            HistoricalEventType::NaturalDisaster => '🌊',
            HistoricalEventType::Coronation => '👑',
            HistoricalEventType::Discovery => '🗺',
            _ => '*',
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoricalEvent {
    pub id: u32,
    pub year: i32,
    pub event_type: HistoricalEventType,
    pub title: String,
    pub description: String,
    pub involved_factions: Vec<u32>,
    pub location: Option<[f32; 2]>,
    pub outcome: String,
    pub population_impact: f64,
    pub wealth_impact: f64,
    pub duration_years: u32,
    pub landmark_created: Option<String>,
}

impl HistoricalEvent {
    pub fn new(id: u32, year: i32, event_type: HistoricalEventType, title: &str) -> Self {
        Self {
            id, year, event_type, title: title.to_string(),
            description: String::new(), involved_factions: Vec::new(),
            location: None, outcome: String::new(),
            population_impact: 0.0, wealth_impact: 0.0,
            duration_years: 1, landmark_created: None,
        }
    }

    pub fn is_positive(&self) -> bool {
        self.population_impact >= 0.0 && self.wealth_impact >= 0.0
    }

    pub fn color(&self) -> Color32 {
        match self.event_type {
            HistoricalEventType::War | HistoricalEventType::Plague | HistoricalEventType::Assassination => Color32::from_rgb(220, 80, 80),
            HistoricalEventType::Alliance | HistoricalEventType::TechBreakthrough => Color32::from_rgb(80, 200, 80),
            HistoricalEventType::Discovery | HistoricalEventType::TradeRoute => Color32::from_rgb(80, 180, 220),
            HistoricalEventType::NaturalDisaster => Color32::from_rgb(180, 140, 50),
            _ => Color32::from_rgb(200, 200, 200),
        }
    }
}

pub fn generate_world_history(factions: &[Faction], years: u32, rng: &mut SimpleRng) -> Vec<HistoricalEvent> {
    let mut events: Vec<HistoricalEvent> = Vec::new();
    let start_year = -1000i32;
    let end_year = start_year + years as i32;

    let event_titles_war = ["The Great War", "The Iron Conflict", "The Shadow War", "The Red Siege", "The Eternal Struggle"];
    let event_titles_plague = ["The Black Death", "The Silver Plague", "The Wasting Sickness", "The Great Mortality"];
    let event_titles_discovery = ["Discovery of the Northern Passage", "The Ancient Ruins Found", "The Map of Ages", "Lost Library Uncovered"];

    let event_types = [
        HistoricalEventType::War, HistoricalEventType::Alliance, HistoricalEventType::Discovery,
        HistoricalEventType::Plague, HistoricalEventType::NaturalDisaster, HistoricalEventType::Coronation,
        HistoricalEventType::Revolution, HistoricalEventType::Migration, HistoricalEventType::TechBreakthrough,
        HistoricalEventType::FamousBattle, HistoricalEventType::CityFounded, HistoricalEventType::TradeRoute,
    ];

    let mut event_id = 1u32;
    let mut year = start_year;
    while year < end_year {
        // Random gap between events (1-50 years)
        year += 1 + (rng.next_u64() % 50) as i32;
        if year >= end_year { break; }

        let et = event_types[rng.next_u64() as usize % event_types.len()].clone();
        let title = match et {
            HistoricalEventType::War => event_titles_war[rng.next_u64() as usize % event_titles_war.len()],
            HistoricalEventType::Plague => event_titles_plague[rng.next_u64() as usize % event_titles_plague.len()],
            HistoricalEventType::Discovery => event_titles_discovery[rng.next_u64() as usize % event_titles_discovery.len()],
            _ => "Historical Event",
        };

        let mut evt = HistoricalEvent::new(event_id, year, et.clone(), title);
        evt.duration_years = 1 + (rng.next_u64() % 30) as u32;

        // Assign involved factions
        if !factions.is_empty() {
            let n_involved = 1 + rng.next_u64() as usize % factions.len().min(3);
            for _ in 0..n_involved {
                let fi = rng.next_u64() as usize % factions.len();
                if !evt.involved_factions.contains(&(fi as u32)) {
                    evt.involved_factions.push(fi as u32);
                }
            }
        }

        // Compute impact
        match et {
            HistoricalEventType::War | HistoricalEventType::Plague => {
                evt.population_impact = -(rng.next_f32() as f64 * 0.3);
                evt.wealth_impact = -(rng.next_f32() as f64 * 0.2);
            }
            HistoricalEventType::TechBreakthrough | HistoricalEventType::TradeRoute => {
                evt.wealth_impact = rng.next_f32() as f64 * 0.2;
                evt.population_impact = rng.next_f32() as f64 * 0.05;
            }
            _ => {}
        }

        evt.location = Some([rng.next_f32(), rng.next_f32()]);
        events.push(evt);
        event_id += 1;
    }

    events
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct WorldHistoryEditorState {
    pub events: Vec<HistoricalEvent>,
    pub selected_event: Option<usize>,
    pub start_year: i32,
    pub end_year: i32,
    pub filter_type: Option<HistoricalEventType>,
    pub show_timeline: bool,
    pub generation_years: u32,
}

pub fn show_world_history_editor(ui: &mut egui::Ui, state: &mut WorldHistoryEditorState, factions: &[Faction]) {
    ui.horizontal(|ui| {
        ui.label("World History");
        ui.add(egui::DragValue::new(&mut state.generation_years).clamp_range(100..=5000u32).prefix("Years: "));
        if ui.button("Generate History").clicked() {
            let mut rng = SimpleRng::new(99999);
            state.events = generate_world_history(factions, state.generation_years, &mut rng);
        }
        ui.checkbox(&mut state.show_timeline, "Timeline View");
    });

    ui.label(format!("Events: {}", state.events.len()));

    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for (i, event) in state.events.iter().enumerate() {
            let sel = state.selected_event == Some(i);
            ui.horizontal(|ui| {
                let label = egui::RichText::new(format!("[{}] {} - {}", event.year, event.event_type.name(), event.title))
                    .color(event.color());
                if ui.selectable_label(sel, label).clicked() { state.selected_event = Some(i); }
            });
        }
    });

    if let Some(idx) = state.selected_event {
        if let Some(evt) = state.events.get_mut(idx) {
            ui.separator();
            ui.heading(&evt.title.clone());
            ui.label(format!("Year {} | Duration: {} years | Significance: {}", evt.year, evt.duration_years, evt.event_type.significance()));
            ui.text_edit_multiline(&mut evt.description);
            ui.text_edit_singleline(&mut evt.outcome);
            ui.label(format!("Population impact: {:.1}%", evt.population_impact * 100.0));
            ui.label(format!("Wealth impact: {:.1}%", evt.wealth_impact * 100.0));
            ui.label(format!("Involved factions: {:?}", evt.involved_factions));
        }
    }
}

// =================================================================
// WORLD GEN EXPANSION 2: TRADE ROUTE NETWORK
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TradeRouteType {
    LandRoute,
    SeaRoute,
    RiverRoute,
    AirRoute,
    UndergroundRoute,
}

impl TradeRouteType {
    pub fn name(&self) -> &'static str {
        match self {
            TradeRouteType::LandRoute => "Land Route",
            TradeRouteType::SeaRoute => "Sea Route",
            TradeRouteType::RiverRoute => "River Route",
            TradeRouteType::AirRoute => "Air Route",
            TradeRouteType::UndergroundRoute => "Underground Route",
        }
    }

    pub fn speed(&self) -> f32 {
        match self {
            TradeRouteType::LandRoute => 1.0,
            TradeRouteType::SeaRoute => 2.0,
            TradeRouteType::RiverRoute => 1.5,
            TradeRouteType::AirRoute => 5.0,
            TradeRouteType::UndergroundRoute => 0.8,
        }
    }

    pub fn capacity(&self) -> f32 {
        match self {
            TradeRouteType::LandRoute => 1.0,
            TradeRouteType::SeaRoute => 5.0,
            TradeRouteType::RiverRoute => 2.0,
            TradeRouteType::AirRoute => 0.5,
            TradeRouteType::UndergroundRoute => 1.5,
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            TradeRouteType::LandRoute => Color32::from_rgb(200, 180, 100),
            TradeRouteType::SeaRoute => Color32::from_rgb(80, 140, 220),
            TradeRouteType::RiverRoute => Color32::from_rgb(100, 180, 220),
            TradeRouteType::AirRoute => Color32::from_rgb(180, 220, 255),
            TradeRouteType::UndergroundRoute => Color32::from_rgb(140, 100, 80),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeRouteWaypoint {
    pub position: [f32; 2],
    pub settlement_index: Option<usize>,
    pub waypoint_name: String,
    pub tariff_rate: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldTradeRoute {
    pub id: u32,
    pub name: String,
    pub route_type: TradeRouteType,
    pub waypoints: Vec<TradeRouteWaypoint>,
    pub goods: Vec<String>,
    pub controlling_faction: Option<u32>,
    pub annual_value: f64,
    pub traffic_level: f32,
    pub established_year: i32,
    pub is_active: bool,
    pub danger_level: u32,
}

impl WorldTradeRoute {
    pub fn new(id: u32, name: &str, route_type: TradeRouteType) -> Self {
        Self {
            id, name: name.to_string(), route_type,
            waypoints: Vec::new(), goods: Vec::new(),
            controlling_faction: None, annual_value: 1000.0,
            traffic_level: 0.5, established_year: -500,
            is_active: true, danger_level: 1,
        }
    }

    pub fn total_length(&self) -> f32 {
        let mut len = 0.0f32;
        for i in 1..self.waypoints.len() {
            let dx = self.waypoints[i].position[0] - self.waypoints[i-1].position[0];
            let dy = self.waypoints[i].position[1] - self.waypoints[i-1].position[1];
            len += (dx*dx + dy*dy).sqrt();
        }
        len
    }

    pub fn effective_value(&self) -> f64 {
        self.annual_value * self.traffic_level as f64 / (1.0 + self.danger_level as f64 * 0.1)
    }

    pub fn add_waypoint(&mut self, pos: [f32;2], name: &str) {
        self.waypoints.push(TradeRouteWaypoint { position: pos, settlement_index: None, waypoint_name: name.to_string(), tariff_rate: 0.05 });
    }
}

pub fn generate_trade_network(settlements: &[NewSettlement], factions: &[Faction], rng: &mut SimpleRng) -> Vec<WorldTradeRoute> {
    let mut routes: Vec<WorldTradeRoute> = Vec::new();
    let route_count = 5.min(settlements.len().max(1));
    let goods_pool = ["Grain", "Iron", "Silk", "Timber", "Spices", "Gold", "Fish", "Horses", "Salt", "Cloth"];
    let route_names = ["Amber Road", "Silk Path", "Iron Way", "Sea of Gold Route", "Spice Trail", "Northern Road", "Southern Coast Route"];

    for i in 0..route_count {
        let rt = if rng.next_f32() < 0.3 { TradeRouteType::SeaRoute } else { TradeRouteType::LandRoute };
        let name = route_names[i % route_names.len()];
        let mut route = WorldTradeRoute::new(i as u32, name, rt);
        route.annual_value = 1000.0 + rng.next_f32() as f64 * 50000.0;
        route.traffic_level = rng.next_f32_range(0.2, 1.0);
        route.danger_level = (rng.next_u64() % 5 + 1) as u32;

        let n_goods = 2 + rng.next_u64() as usize % 4;
        for _ in 0..n_goods {
            let g = goods_pool[rng.next_u64() as usize % goods_pool.len()];
            if !route.goods.contains(&g.to_string()) { route.goods.push(g.to_string()); }
        }

        // Generate 2-5 waypoints
        let n_wp = 2 + rng.next_u64() as usize % 4;
        for wi in 0..n_wp {
            let px = rng.next_f32();
            let py = rng.next_f32();
            let wp_name = format!("Stop {}", wi + 1);
            route.add_waypoint([px, py], &wp_name);
        }

        if !factions.is_empty() {
            route.controlling_faction = Some((rng.next_u64() % factions.len() as u64) as u32);
        }

        routes.push(route);
    }
    routes
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TradeNetworkEditorState {
    pub routes: Vec<WorldTradeRoute>,
    pub selected_route: Option<usize>,
    pub selected_waypoint: Option<usize>,
    pub show_route_labels: bool,
    pub show_goods: bool,
    pub filter_active_only: bool,
}

pub fn show_trade_network_editor(ui: &mut egui::Ui, state: &mut TradeNetworkEditorState) {
    ui.horizontal(|ui| {
        ui.label("Trade Network Editor");
        if ui.button("+ New Route").clicked() {
            let id = state.routes.len() as u32;
            state.routes.push(WorldTradeRoute::new(id, &format!("Route {}", id), TradeRouteType::LandRoute));
        }
        ui.checkbox(&mut state.show_route_labels, "Labels");
        ui.checkbox(&mut state.show_goods, "Goods");
        ui.checkbox(&mut state.filter_active_only, "Active Only");
    });

    egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
        for (i, route) in state.routes.iter().enumerate() {
            if state.filter_active_only && !route.is_active { continue; }
            let sel = state.selected_route == Some(i);
            let label = egui::RichText::new(format!("{} ({}) Val:{:.0}", route.name, route.route_type.name(), route.effective_value()))
                .color(route.route_type.color());
            if ui.selectable_label(sel, label).clicked() { state.selected_route = Some(i); }
        }
    });

    if let Some(ri) = state.selected_route {
        if let Some(route) = state.routes.get_mut(ri) {
            ui.separator();
            ui.text_edit_singleline(&mut route.name);
            ui.checkbox(&mut route.is_active, "Active");
            ui.add(egui::Slider::new(&mut route.traffic_level, 0.0..=1.0).text("Traffic"));
            ui.add(egui::DragValue::new(&mut route.danger_level).clamp_range(1..=10u32).prefix("Danger: "));
            ui.label(format!("Goods: {}", route.goods.join(", ")));
            ui.label(format!("Length: {:.2} | Effective Value: {:.0}", route.total_length(), route.effective_value()));
            ui.label("Waypoints:");
            for (wi, wp) in route.waypoints.iter().enumerate() {
                ui.label(format!("  {} ({:.2},{:.2})", wp.waypoint_name, wp.position[0], wp.position[1]));
            }
        }
    }
}

// =================================================================
// WORLD GEN EXPANSION 2: MAGIC SITES & LEYLINES
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MagicSiteType {
    AncientNexus,
    Wellspring,
    FeyPortal,
    DragonLair,
    ArcaneObservatory,
    SacredGrove,
    AbyssalRift,
    CelestialAltar,
    ElementalNode,
    ShadowLibrary,
}

impl MagicSiteType {
    pub fn name(&self) -> &'static str {
        match self {
            MagicSiteType::AncientNexus => "Ancient Nexus",
            MagicSiteType::Wellspring => "Magical Wellspring",
            MagicSiteType::FeyPortal => "Fey Portal",
            MagicSiteType::DragonLair => "Dragon Lair",
            MagicSiteType::ArcaneObservatory => "Arcane Observatory",
            MagicSiteType::SacredGrove => "Sacred Grove",
            MagicSiteType::AbyssalRift => "Abyssal Rift",
            MagicSiteType::CelestialAltar => "Celestial Altar",
            MagicSiteType::ElementalNode => "Elemental Node",
            MagicSiteType::ShadowLibrary => "Shadow Library",
        }
    }

    pub fn power_level(&self) -> u32 {
        match self {
            MagicSiteType::AncientNexus | MagicSiteType::AbyssalRift => 5,
            MagicSiteType::DragonLair | MagicSiteType::FeyPortal => 4,
            MagicSiteType::CelestialAltar | MagicSiteType::ShadowLibrary => 4,
            MagicSiteType::ArcaneObservatory | MagicSiteType::ElementalNode => 3,
            MagicSiteType::Wellspring | MagicSiteType::SacredGrove => 2,
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            MagicSiteType::AncientNexus => Color32::from_rgb(255, 180, 50),
            MagicSiteType::Wellspring => Color32::from_rgb(100, 200, 255),
            MagicSiteType::FeyPortal => Color32::from_rgb(200, 100, 255),
            MagicSiteType::DragonLair => Color32::from_rgb(255, 80, 30),
            MagicSiteType::ArcaneObservatory => Color32::from_rgb(150, 200, 255),
            MagicSiteType::SacredGrove => Color32::from_rgb(80, 200, 80),
            MagicSiteType::AbyssalRift => Color32::from_rgb(80, 20, 120),
            MagicSiteType::CelestialAltar => Color32::from_rgb(255, 240, 180),
            MagicSiteType::ElementalNode => Color32::from_rgb(200, 160, 80),
            MagicSiteType::ShadowLibrary => Color32::from_rgb(120, 100, 160),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MagicSite {
    pub id: u32,
    pub name: String,
    pub site_type: MagicSiteType,
    pub position: [f32; 2],
    pub power_level: u32,
    pub active: bool,
    pub guardians: Vec<String>,
    pub connected_leylines: Vec<usize>,
    pub discovered: bool,
    pub lore: String,
    pub effects_radius: f32,
}

impl MagicSite {
    pub fn new(id: u32, name: &str, site_type: MagicSiteType, pos: [f32;2]) -> Self {
        let power = site_type.power_level();
        Self {
            id, name: name.to_string(), site_type, position: pos,
            power_level: power, active: true, guardians: Vec::new(),
            connected_leylines: Vec::new(), discovered: false, lore: String::new(),
            effects_radius: power as f32 * 0.05,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Leyline {
    pub id: usize,
    pub start_site: usize,
    pub end_site: usize,
    pub strength: f32,
    pub element: LeylineElement,
    pub waypoints: Vec<[f32;2]>,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LeylineElement {
    Fire, Water, Earth, Air, Shadow, Light, Chaos, Order,
}

impl LeylineElement {
    pub fn color(&self) -> Color32 {
        match self {
            LeylineElement::Fire => Color32::from_rgb(255, 100, 30),
            LeylineElement::Water => Color32::from_rgb(50, 150, 255),
            LeylineElement::Earth => Color32::from_rgb(120, 100, 50),
            LeylineElement::Air => Color32::from_rgb(200, 230, 255),
            LeylineElement::Shadow => Color32::from_rgb(80, 50, 120),
            LeylineElement::Light => Color32::from_rgb(255, 250, 200),
            LeylineElement::Chaos => Color32::from_rgb(200, 50, 200),
            LeylineElement::Order => Color32::from_rgb(180, 180, 255),
        }
    }
}

pub fn generate_magic_sites(count: u32, rng: &mut SimpleRng) -> Vec<MagicSite> {
    let site_types = [
        MagicSiteType::Wellspring, MagicSiteType::SacredGrove, MagicSiteType::ElementalNode,
        MagicSiteType::AncientNexus, MagicSiteType::FeyPortal, MagicSiteType::DragonLair,
        MagicSiteType::ArcaneObservatory, MagicSiteType::AbyssalRift, MagicSiteType::CelestialAltar,
        MagicSiteType::ShadowLibrary,
    ];
    let site_name_prefixes = ["Elder", "Ancient", "Hidden", "Lost", "Sacred", "Forbidden", "Eternal", "Cursed", "Blessed", "Dark"];
    let site_name_suffixes = ["Nexus", "Pool", "Grove", "Spire", "Gate", "Lair", "Shrine", "Vault", "Circle", "Hollow"];

    (0..count).map(|i| {
        let st = site_types[rng.next_u64() as usize % site_types.len()].clone();
        let prefix = site_name_prefixes[rng.next_u64() as usize % site_name_prefixes.len()];
        let suffix = site_name_suffixes[rng.next_u64() as usize % site_name_suffixes.len()];
        let name = format!("{} {}", prefix, suffix);
        let pos = [rng.next_f32(), rng.next_f32()];
        let mut site = MagicSite::new(i, &name, st, pos);
        site.discovered = rng.next_f32() < 0.6;
        site.active = rng.next_f32() < 0.8;
        site
    }).collect()
}

pub fn generate_leylines(sites: &[MagicSite], rng: &mut SimpleRng) -> Vec<Leyline> {
    let elements = [
        LeylineElement::Fire, LeylineElement::Water, LeylineElement::Earth, LeylineElement::Air,
        LeylineElement::Shadow, LeylineElement::Light,
    ];
    let n = sites.len();
    if n < 2 { return Vec::new(); }
    let count = (n * 2 / 3).max(1);
    (0..count).map(|i| {
        let a = rng.next_u64() as usize % n;
        let mut b = rng.next_u64() as usize % n;
        while b == a { b = rng.next_u64() as usize % n; }
        Leyline {
            id: i,
            start_site: a,
            end_site: b,
            strength: rng.next_f32_range(0.3, 1.0),
            element: elements[rng.next_u64() as usize % elements.len()].clone(),
            waypoints: Vec::new(),
            active: rng.next_f32() < 0.85,
        }
    }).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MagicMapEditorState {
    pub sites: Vec<MagicSite>,
    pub leylines: Vec<Leyline>,
    pub selected_site: Option<usize>,
    pub selected_leyline: Option<usize>,
    pub show_sites: bool,
    pub show_leylines: bool,
    pub show_undiscovered: bool,
    pub site_count: u32,
}

impl MagicMapEditorState {
    pub fn new() -> Self {
        Self { show_sites: true, show_leylines: true, show_undiscovered: false, site_count: 15, ..Default::default() }
    }
}

pub fn show_magic_map_editor(ui: &mut egui::Ui, state: &mut MagicMapEditorState) {
    ui.horizontal(|ui| {
        ui.label("Magic Sites & Leylines");
        ui.add(egui::DragValue::new(&mut state.site_count).clamp_range(5..=50u32).prefix("Sites: "));
        if ui.button("Generate").clicked() {
            let mut rng = SimpleRng::new(77777);
            state.sites = generate_magic_sites(state.site_count, &mut rng);
            state.leylines = generate_leylines(&state.sites, &mut rng);
        }
        ui.checkbox(&mut state.show_sites, "Sites");
        ui.checkbox(&mut state.show_leylines, "Leylines");
        ui.checkbox(&mut state.show_undiscovered, "Undiscovered");
    });

    egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
        ui.collapsing("Magic Sites", |ui| {
            for (i, site) in state.sites.iter().enumerate() {
                if !state.show_undiscovered && !site.discovered { continue; }
                let sel = state.selected_site == Some(i);
                let label = egui::RichText::new(format!("{} (Power:{})", site.name, site.power_level))
                    .color(if site.active { site.site_type.color() } else { Color32::GRAY });
                if ui.selectable_label(sel, label).clicked() { state.selected_site = Some(i); }
            }
        });
        ui.collapsing("Leylines", |ui| {
            for (i, ll) in state.leylines.iter().enumerate() {
                let sel = state.selected_leyline == Some(i);
                let label = egui::RichText::new(format!("LL{}: {}->{} ({:?})", i, ll.start_site, ll.end_site, ll.element))
                    .color(if ll.active { ll.element.color() } else { Color32::GRAY });
                if ui.selectable_label(sel, label).clicked() { state.selected_leyline = Some(i); }
            }
        });
    });

    if let Some(si) = state.selected_site {
        if let Some(site) = state.sites.get_mut(si) {
            ui.separator();
            ui.text_edit_singleline(&mut site.name);
            ui.checkbox(&mut site.active, "Active");
            ui.checkbox(&mut site.discovered, "Discovered");
            ui.add(egui::Slider::new(&mut site.power_level, 1..=5).text("Power Level"));
            ui.add(egui::Slider::new(&mut site.effects_radius, 0.01..=0.3).text("Influence Radius"));
            ui.text_edit_multiline(&mut site.lore);
        }
    }
}

// =================================================================
// WORLD GEN EXPANSION 2: FULL WORLD STATE
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MasterWorldEditorState {
    pub full_world: FullWorldEditorState,
    pub faction_state: FactionEditorState,
    pub history_state: WorldHistoryEditorState,
    pub trade_state: TradeNetworkEditorState,
    pub magic_state: MagicMapEditorState,
    pub active_tab: MasterWorldTab,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum MasterWorldTab {
    #[default]
    Generation,
    Factions,
    History,
    Trade,
    Magic,
    Export,
}

impl MasterWorldEditorState {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            full_world: FullWorldEditorState::new(w, h),
            magic_state: MagicMapEditorState::new(),
            history_state: WorldHistoryEditorState { generation_years: 2000, ..Default::default() },
            ..Default::default()
        }
    }
}

pub fn show_master_world_editor(ui: &mut egui::Ui, state: &mut MasterWorldEditorState) {
    ui.horizontal(|ui| {
        for (tab, label) in [
            (MasterWorldTab::Generation, "World Gen"),
            (MasterWorldTab::Factions, "Factions"),
            (MasterWorldTab::History, "History"),
            (MasterWorldTab::Trade, "Trade"),
            (MasterWorldTab::Magic, "Magic"),
            (MasterWorldTab::Export, "Export"),
        ] {
            if ui.selectable_label(state.active_tab == tab, label).clicked() { state.active_tab = tab; }
        }
    });
    ui.separator();
    match state.active_tab {
        MasterWorldTab::Generation => show_full_world_editor(ui, &mut state.full_world),
        MasterWorldTab::Factions => show_faction_editor(ui, &mut state.faction_state),
        MasterWorldTab::History => show_world_history_editor(ui, &mut state.history_state, &state.faction_state.factions),
        MasterWorldTab::Trade => show_trade_network_editor(ui, &mut state.trade_state),
        MasterWorldTab::Magic => show_magic_map_editor(ui, &mut state.magic_state),
        MasterWorldTab::Export => show_world_export_panel(ui, &mut WorldExportState::default()),
    }
}

// =================================================================
// WORLD GEN EXPANSION 2: TESTS
// =================================================================

#[cfg(test)]
mod world_gen_expansion2_tests {
    use super::*;

    #[test]
    fn test_faction_generation() {
        let mut rng = SimpleRng::new(42);
        let factions = generate_factions(5, &mut rng);
        assert_eq!(factions.len(), 5);
        for f in &factions { assert!(!f.leaders.is_empty()); }
    }

    #[test]
    fn test_faction_alliance_enemy() {
        let mut f = Faction::new(1, "Test", CultureType::Nordic);
        f.add_ally(2);
        assert!(f.is_allied_with(2));
        f.declare_war(2);
        assert!(f.is_enemy_of(2));
        assert!(!f.is_allied_with(2));
    }

    #[test]
    fn test_faction_trait_modifiers() {
        assert!(FactionTrait::Militaristic.military_modifier() > 1.0);
        assert!(FactionTrait::Pacifist.military_modifier() < 1.0);
        assert!(FactionTrait::Mercantile.economy_modifier() > 1.0);
    }

    #[test]
    fn test_historical_event_generation() {
        let factions = vec![Faction::new(0, "A", CultureType::Nordic), Faction::new(1, "B", CultureType::Eastern)];
        let mut rng = SimpleRng::new(42);
        let events = generate_world_history(&factions, 1000, &mut rng);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_trade_route_length() {
        let mut route = WorldTradeRoute::new(1, "Test Route", TradeRouteType::LandRoute);
        route.add_waypoint([0.0, 0.0], "Start");
        route.add_waypoint([3.0, 4.0], "End");
        assert!((route.total_length() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_trade_route_effective_value() {
        let mut route = WorldTradeRoute::new(1, "Test", TradeRouteType::SeaRoute);
        route.annual_value = 1000.0;
        route.traffic_level = 1.0;
        route.danger_level = 1;
        let val = route.effective_value();
        assert!(val > 0.0);
    }

    #[test]
    fn test_magic_site_generation() {
        let mut rng = SimpleRng::new(42);
        let sites = generate_magic_sites(10, &mut rng);
        assert_eq!(sites.len(), 10);
        assert!(sites.iter().any(|s| s.power_level > 0));
    }

    #[test]
    fn test_leyline_generation() {
        let mut rng = SimpleRng::new(42);
        let sites = generate_magic_sites(8, &mut rng);
        let leylines = generate_leylines(&sites, &mut rng);
        assert!(!leylines.is_empty());
        for ll in &leylines {
            assert!(ll.start_site < sites.len());
            assert!(ll.end_site < sites.len());
        }
    }

    #[test]
    fn test_culture_type_names() {
        assert_eq!(CultureType::Nordic.name(), "Nordic");
        assert!(!CultureType::Eastern.primary_resource().is_empty());
    }

    #[test]
    fn test_faction_leader_power() {
        let mut leader = FactionLeader::new("Test", "King");
        leader.charisma = 15; leader.intelligence = 12; leader.warfare = 10;
        assert_eq!(leader.effective_power(), 37);
    }
}
'''

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'a', encoding='utf-8') as f:
    f.write(code)
print(f"Done. world_gen size: {__import__('os').path.getsize(r'C:\proof-engine\editor\src\world_gen.rs')} bytes")
