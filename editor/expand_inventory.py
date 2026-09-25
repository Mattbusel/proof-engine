code = r'''

// =================================================================
// INVENTORY EXPANSION: CRAFTING SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CraftingStation {
    Workbench,
    Forge,
    Anvil,
    AlchemyTable,
    EnchantingTable,
    Cauldron,
    LoomTable,
    JewelryBench,
    CarpenterBench,
    StoneMasonBench,
}

impl CraftingStation {
    pub fn name(&self) -> &'static str {
        match self {
            CraftingStation::Workbench => "Workbench",
            CraftingStation::Forge => "Forge",
            CraftingStation::Anvil => "Anvil",
            CraftingStation::AlchemyTable => "Alchemy Table",
            CraftingStation::EnchantingTable => "Enchanting Table",
            CraftingStation::Cauldron => "Cauldron",
            CraftingStation::LoomTable => "Loom Table",
            CraftingStation::JewelryBench => "Jewelry Bench",
            CraftingStation::CarpenterBench => "Carpenter Bench",
            CraftingStation::StoneMasonBench => "Stone Mason Bench",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            CraftingStation::Workbench => 'W',
            CraftingStation::Forge | CraftingStation::Anvil => 'F',
            CraftingStation::AlchemyTable | CraftingStation::Cauldron => 'A',
            CraftingStation::EnchantingTable => 'E',
            CraftingStation::LoomTable => 'L',
            CraftingStation::JewelryBench => 'J',
            CraftingStation::CarpenterBench => 'C',
            CraftingStation::StoneMasonBench => 'S',
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftingIngredient {
    pub item_name: String,
    pub quantity: u32,
    pub consumed: bool,
    pub is_tool: bool,
    pub quality_requirement: Option<u32>,
}

impl CraftingIngredient {
    pub fn new(name: &str, qty: u32) -> Self {
        Self { item_name: name.to_string(), quantity: qty, consumed: true, is_tool: false, quality_requirement: None }
    }

    pub fn tool(name: &str) -> Self {
        Self { item_name: name.to_string(), quantity: 1, consumed: false, is_tool: true, quality_requirement: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftingRecipe {
    pub id: u32,
    pub name: String,
    pub category: String,
    pub station: CraftingStation,
    pub ingredients: Vec<CraftingIngredient>,
    pub outputs: Vec<(String, u32)>,
    pub experience_reward: u32,
    pub level_requirement: u32,
    pub time_seconds: f32,
    pub success_chance: f32,
    pub quality_bonus: f32,
    pub description: String,
    pub unlocked: bool,
    pub tags: Vec<String>,
}

impl CraftingRecipe {
    pub fn new(id: u32, name: &str, station: CraftingStation) -> Self {
        Self {
            id, name: name.to_string(), category: String::new(), station,
            ingredients: Vec::new(), outputs: Vec::new(), experience_reward: 10,
            level_requirement: 1, time_seconds: 5.0, success_chance: 1.0,
            quality_bonus: 0.0, description: String::new(), unlocked: true, tags: Vec::new(),
        }
    }

    pub fn add_ingredient(&mut self, name: &str, qty: u32) -> &mut Self {
        self.ingredients.push(CraftingIngredient::new(name, qty)); self
    }

    pub fn add_output(&mut self, name: &str, qty: u32) -> &mut Self {
        self.outputs.push((name.to_string(), qty)); self
    }

    pub fn iron_sword() -> Self {
        let mut r = CraftingRecipe::new(1, "Iron Sword", CraftingStation::Anvil);
        r.add_ingredient("Iron Ingot", 3).add_ingredient("Wood Plank", 1);
        r.add_output("Iron Sword", 1);
        r.experience_reward = 25; r.level_requirement = 5; r.description = "A sturdy iron sword".to_string();
        r
    }

    pub fn health_potion() -> Self {
        let mut r = CraftingRecipe::new(2, "Health Potion", CraftingStation::AlchemyTable);
        r.add_ingredient("Red Herb", 3).add_ingredient("Water Vial", 1).add_ingredient("Honey", 1);
        r.add_output("Health Potion", 2);
        r.experience_reward = 15; r.time_seconds = 8.0; r.description = "Restores 50 HP".to_string();
        r
    }

    pub fn steel_armor() -> Self {
        let mut r = CraftingRecipe::new(3, "Steel Chestplate", CraftingStation::Forge);
        r.add_ingredient("Steel Ingot", 8).add_ingredient("Leather Strip", 4);
        r.add_output("Steel Chestplate", 1);
        r.experience_reward = 50; r.level_requirement = 15; r.time_seconds = 30.0;
        r
    }

    pub fn mana_potion() -> Self {
        let mut r = CraftingRecipe::new(4, "Mana Potion", CraftingStation::AlchemyTable);
        r.add_ingredient("Blue Herb", 2).add_ingredient("Water Vial", 1).add_ingredient("Magic Dust", 1);
        r.add_output("Mana Potion", 1);
        r.experience_reward = 20; r.time_seconds = 10.0;
        r
    }

    pub fn enchanted_ring() -> Self {
        let mut r = CraftingRecipe::new(5, "Ring of Power", CraftingStation::JewelryBench);
        r.add_ingredient("Gold Ingot", 2).add_ingredient("Ruby", 1).add_ingredient("Magic Dust", 3);
        r.add_output("Ring of Power", 1);
        r.experience_reward = 75; r.level_requirement = 20; r.success_chance = 0.8;
        r
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftingQueueEntry {
    pub recipe_id: u32,
    pub quantity: u32,
    pub progress: f32,
    pub started: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CraftingEditorState {
    pub recipes: Vec<CraftingRecipe>,
    pub queue: Vec<CraftingQueueEntry>,
    pub selected_recipe: Option<usize>,
    pub selected_station: Option<CraftingStation>,
    pub search_query: String,
    pub show_locked: bool,
    pub filter_category: String,
    pub crafting_level: u32,
    pub total_experience: u32,
}

impl CraftingEditorState {
    pub fn with_default_recipes() -> Self {
        let mut s = Self::default();
        s.recipes = vec![
            CraftingRecipe::iron_sword(),
            CraftingRecipe::health_potion(),
            CraftingRecipe::steel_armor(),
            CraftingRecipe::mana_potion(),
            CraftingRecipe::enchanted_ring(),
        ];
        s.crafting_level = 1;
        s
    }

    pub fn craftable_recipes(&self) -> Vec<&CraftingRecipe> {
        self.recipes.iter().filter(|r| r.unlocked && r.level_requirement <= self.crafting_level).collect()
    }

    pub fn add_to_queue(&mut self, recipe_id: u32, qty: u32) {
        if let Some(existing) = self.queue.iter_mut().find(|e| e.recipe_id == recipe_id) {
            existing.quantity += qty;
        } else {
            self.queue.push(CraftingQueueEntry { recipe_id, quantity: qty, progress: 0.0, started: false });
        }
    }

    pub fn level(&self) -> u32 {
        let xp = self.total_experience;
        if xp < 100 { 1 }
        else if xp < 300 { 2 }
        else if xp < 600 { 3 }
        else if xp < 1000 { 4 }
        else { 5 + (xp - 1000) / 500 }
    }
}

pub fn show_crafting_editor(ui: &mut egui::Ui, state: &mut CraftingEditorState) {
    ui.horizontal(|ui| {
        ui.label(format!("Crafting Level: {} (XP: {})", state.level(), state.total_experience));
        ui.separator();
        ui.text_edit_singleline(&mut state.search_query);
        ui.checkbox(&mut state.show_locked, "Show Locked");
    });

    let query = state.search_query.to_lowercase();
    let craftable_ids: Vec<usize> = state.recipes.iter().enumerate().filter(|(_, r)| {
        (state.show_locked || r.unlocked) &&
        (query.is_empty() || r.name.to_lowercase().contains(&query) || r.category.to_lowercase().contains(&query))
    }).map(|(i, _)| i).collect();

    ui.columns(2, |cols| {
        egui::ScrollArea::vertical().max_height(300.0).show(&mut cols[0], |ui| {
            ui.heading("Recipes");
            for &i in &craftable_ids {
                let r = &state.recipes[i];
                let can_craft = r.level_requirement <= state.crafting_level;
                let sel = state.selected_recipe == Some(i);
                let label = egui::RichText::new(format!("{} [{}]", r.name, r.station.name()))
                    .color(if can_craft { egui::Color32::WHITE } else { egui::Color32::GRAY });
                if ui.selectable_label(sel, label).clicked() {
                    state.selected_recipe = Some(i);
                }
            }
        });

        if let Some(idx) = state.selected_recipe {
            if let Some(recipe) = state.recipes.get(idx) {
                let recipe = recipe.clone();
                egui::ScrollArea::vertical().max_height(300.0).show(&mut cols[1], |ui| {
                    ui.heading(&recipe.name);
                    ui.label(&recipe.description);
                    ui.label(format!("Station: {}", recipe.station.name()));
                    ui.label(format!("Level Req: {} | XP: {} | Time: {:.0}s", recipe.level_requirement, recipe.experience_reward, recipe.time_seconds));
                    if recipe.success_chance < 1.0 { ui.label(format!("Success Chance: {:.0}%", recipe.success_chance * 100.0)); }
                    ui.separator();
                    ui.label("Ingredients:");
                    for ing in &recipe.ingredients {
                        ui.label(format!("  {} x{}", ing.item_name, ing.quantity));
                    }
                    ui.separator();
                    ui.label("Outputs:");
                    for (name, qty) in &recipe.outputs {
                        ui.label(format!("  {} x{}", name, qty));
                    }
                    ui.separator();
                    if recipe.level_requirement <= state.crafting_level {
                        if ui.button("Craft x1").clicked() {
                            state.add_to_queue(recipe.id, 1);
                        }
                        if ui.button("Craft x5").clicked() {
                            state.add_to_queue(recipe.id, 5);
                        }
                    } else {
                        ui.label(egui::RichText::new(format!("Requires Level {}", recipe.level_requirement)).color(egui::Color32::RED));
                    }
                });
            }
        }
    });

    ui.separator();
    ui.heading("Crafting Queue");
    if state.queue.is_empty() {
        ui.label("No active crafting jobs.");
    }
    let mut to_remove = None;
    for (i, entry) in state.queue.iter_mut().enumerate() {
        if let Some(recipe) = state.recipes.iter().find(|r| r.id == entry.recipe_id) {
            ui.horizontal(|ui| {
                ui.label(format!("{} x{}", recipe.name, entry.quantity));
                ui.add(egui::ProgressBar::new(entry.progress).text(format!("{:.0}%", entry.progress * 100.0)));
                if ui.button("Cancel").clicked() { to_remove = Some(i); }
            });
        }
    }
    if let Some(i) = to_remove { state.queue.remove(i); }
}

// =================================================================
// INVENTORY EXPANSION: EQUIPMENT DURABILITY & REPAIR
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DurabilityInfo {
    pub current: f32,
    pub maximum: f32,
    pub degradation_rate: f32,
    pub repair_cost_per_point: f32,
    pub material: RepairMaterial,
    pub can_repair: bool,
    pub broken: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum RepairMaterial {
    Iron,
    Steel,
    Leather,
    Wood,
    Cloth,
    Mithril,
    Dragonbone,
    Magic,
}

impl RepairMaterial {
    pub fn name(&self) -> &'static str {
        match self {
            RepairMaterial::Iron => "Iron",
            RepairMaterial::Steel => "Steel",
            RepairMaterial::Leather => "Leather",
            RepairMaterial::Wood => "Wood",
            RepairMaterial::Cloth => "Cloth",
            RepairMaterial::Mithril => "Mithril",
            RepairMaterial::Dragonbone => "Dragonbone",
            RepairMaterial::Magic => "Magic Dust",
        }
    }

    pub fn base_cost(&self) -> f32 {
        match self {
            RepairMaterial::Iron => 1.0,
            RepairMaterial::Steel => 2.5,
            RepairMaterial::Leather => 1.5,
            RepairMaterial::Wood => 0.5,
            RepairMaterial::Cloth => 0.75,
            RepairMaterial::Mithril => 10.0,
            RepairMaterial::Dragonbone => 25.0,
            RepairMaterial::Magic => 5.0,
        }
    }
}

impl DurabilityInfo {
    pub fn new(max: f32, material: RepairMaterial) -> Self {
        let cost = material.base_cost();
        Self { current: max, maximum: max, degradation_rate: 1.0, repair_cost_per_point: cost, material, can_repair: true, broken: false }
    }

    pub fn percentage(&self) -> f32 { self.current / self.maximum.max(0.001) }

    pub fn condition_text(&self) -> &'static str {
        match (self.percentage() * 100.0) as u32 {
            90..=100 => "Perfect",
            70..=89 => "Good",
            50..=69 => "Worn",
            25..=49 => "Damaged",
            1..=24 => "Badly Damaged",
            _ => "Broken",
        }
    }

    pub fn color(&self) -> egui::Color32 {
        let p = self.percentage();
        if p > 0.8 { egui::Color32::from_rgb(50, 200, 50) }
        else if p > 0.5 { egui::Color32::from_rgb(200, 200, 50) }
        else if p > 0.25 { egui::Color32::from_rgb(200, 100, 50) }
        else { egui::Color32::from_rgb(200, 50, 50) }
    }

    pub fn apply_use_damage(&mut self, amount: f32) {
        self.current = (self.current - amount * self.degradation_rate).max(0.0);
        if self.current == 0.0 { self.broken = true; }
    }

    pub fn repair(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.maximum);
        if self.current > 0.0 { self.broken = false; }
    }

    pub fn full_repair_cost(&self) -> f32 {
        (self.maximum - self.current) * self.repair_cost_per_point
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DurabilityEditorState {
    pub items_with_durability: Vec<(String, DurabilityInfo)>,
    pub selected: Option<usize>,
    pub repair_amount: f32,
    pub auto_repair_threshold: f32,
    pub show_all_conditions: bool,
}

impl DurabilityEditorState {
    pub fn new() -> Self {
        let mut s = Self { repair_amount: 50.0, auto_repair_threshold: 0.3, ..Default::default() };
        s.items_with_durability = vec![
            ("Iron Sword".to_string(), DurabilityInfo::new(100.0, RepairMaterial::Iron)),
            ("Steel Chestplate".to_string(), DurabilityInfo::new(200.0, RepairMaterial::Steel)),
            ("Leather Boots".to_string(), DurabilityInfo::new(80.0, RepairMaterial::Leather)),
        ];
        s
    }
}

pub fn show_durability_editor(ui: &mut egui::Ui, state: &mut DurabilityEditorState) {
    ui.heading("Equipment Durability");
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_all_conditions, "Show All");
        ui.add(egui::Slider::new(&mut state.auto_repair_threshold, 0.0..=1.0).text("Auto-repair below"));
    });

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        for (i, (name, dur)) in state.items_with_durability.iter().enumerate() {
            let sel = state.selected == Some(i);
            ui.horizontal(|ui| {
                if ui.selectable_label(sel, name).clicked() { state.selected = Some(i); }
                let prog = egui::ProgressBar::new(dur.percentage()).desired_width(80.0).text(dur.condition_text());
                ui.add(prog);
                ui.colored_label(dur.color(), format!("{:.0}/{:.0}", dur.current, dur.maximum));
            });
        }
    });

    if let Some(idx) = state.selected {
        if let Some((name, dur)) = state.items_with_durability.get_mut(idx) {
            ui.separator();
            ui.label(format!("Selected: {}", name));
            ui.label(format!("Material: {} | Condition: {}", dur.material.name(), dur.condition_text()));
            ui.label(format!("Repair Cost (full): {:.1} {}", dur.full_repair_cost(), dur.material.name()));
            ui.add(egui::Slider::new(&mut dur.degradation_rate, 0.1..=5.0).text("Degradation Rate"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut state.repair_amount).clamp_range(0.0..=1000.0f32).prefix("Amount: "));
                if ui.button("Repair").clicked() { dur.repair(state.repair_amount); }
                if ui.button("Repair Full").clicked() { dur.repair(dur.maximum); }
                if ui.button("Apply Damage").clicked() { dur.apply_use_damage(state.repair_amount); }
            });
        }
    }
}

// =================================================================
// INVENTORY EXPANSION: ITEM SOCKETING SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SocketType {
    Weapon,
    Armor,
    Ring,
    Universal,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GemType {
    Ruby,
    Sapphire,
    Emerald,
    Diamond,
    Amethyst,
    Topaz,
    Onyx,
    Opal,
}

impl GemType {
    pub fn name(&self) -> &'static str {
        match self {
            GemType::Ruby => "Ruby",
            GemType::Sapphire => "Sapphire",
            GemType::Emerald => "Emerald",
            GemType::Diamond => "Diamond",
            GemType::Amethyst => "Amethyst",
            GemType::Topaz => "Topaz",
            GemType::Onyx => "Onyx",
            GemType::Opal => "Opal",
        }
    }

    pub fn color(&self) -> egui::Color32 {
        match self {
            GemType::Ruby => egui::Color32::from_rgb(220, 30, 30),
            GemType::Sapphire => egui::Color32::from_rgb(30, 80, 220),
            GemType::Emerald => egui::Color32::from_rgb(30, 180, 60),
            GemType::Diamond => egui::Color32::from_rgb(220, 240, 255),
            GemType::Amethyst => egui::Color32::from_rgb(160, 60, 220),
            GemType::Topaz => egui::Color32::from_rgb(220, 180, 30),
            GemType::Onyx => egui::Color32::from_rgb(40, 40, 50),
            GemType::Opal => egui::Color32::from_rgb(200, 200, 240),
        }
    }

    pub fn stat_bonus(&self, quality: u32) -> StatBonus {
        let q = quality as f32;
        match self {
            GemType::Ruby => StatBonus::default().with_attack(5 + (q * 3.0) as u32),
            GemType::Sapphire => StatBonus::default().with_mana(30 + (q * 15.0) as u32),
            GemType::Emerald => StatBonus::default().with_health(20 + (q * 10.0) as u32),
            GemType::Diamond => StatBonus::default().with_attack(3 + (q * 2.0) as u32).with_defense(3 + (q * 2.0) as u32),
            GemType::Amethyst => StatBonus::default().with_magic(8 + (q * 4.0) as u32),
            GemType::Topaz => StatBonus::default().with_speed(3 + (q * 2.0) as u32),
            GemType::Onyx => StatBonus::default().with_defense(8 + (q * 5.0) as u32),
            GemType::Opal => {
                let mut b = StatBonus::default();
                b.crit_chance += 0.02 + q as f32 * 0.01;
                b
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gem {
    pub gem_type: GemType,
    pub quality: u32,
    pub name: String,
    pub enhanced: bool,
}

impl Gem {
    pub fn new(gem_type: GemType, quality: u32) -> Self {
        let name = format!("{} Gem (Q{})", gem_type.name(), quality);
        Self { gem_type, quality, name, enhanced: false }
    }

    pub fn get_bonus(&self) -> StatBonus {
        let mut bonus = self.gem_type.stat_bonus(self.quality);
        if self.enhanced {
            bonus.attack = (bonus.attack as f32 * 1.5) as u32;
            bonus.defense = (bonus.defense as f32 * 1.5) as u32;
            bonus.health = (bonus.health as f32 * 1.5) as u32;
            bonus.mana = (bonus.mana as f32 * 1.5) as u32;
            bonus.magic = (bonus.magic as f32 * 1.5) as u32;
        }
        bonus
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemSocket {
    pub socket_type: SocketType,
    pub inserted_gem: Option<Gem>,
    pub locked: bool,
}

impl ItemSocket {
    pub fn empty(socket_type: SocketType) -> Self {
        Self { socket_type, inserted_gem: None, locked: false }
    }

    pub fn can_accept(&self, gem: &Gem) -> bool {
        !self.locked && self.inserted_gem.is_none()
    }

    pub fn insert(&mut self, gem: Gem) -> Result<(), &'static str> {
        if self.locked { return Err("Socket is locked"); }
        if self.inserted_gem.is_some() { return Err("Socket already filled"); }
        self.inserted_gem = Some(gem);
        Ok(())
    }

    pub fn remove(&mut self) -> Option<Gem> {
        self.inserted_gem.take()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SocketingEditorState {
    pub items_with_sockets: Vec<(String, Vec<ItemSocket>)>,
    pub gem_inventory: Vec<Gem>,
    pub selected_item: Option<usize>,
    pub selected_socket: Option<usize>,
    pub selected_gem: Option<usize>,
    pub show_gem_details: bool,
}

impl SocketingEditorState {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.items_with_sockets = vec![
            ("Dragonbone Sword".to_string(), vec![
                ItemSocket::empty(SocketType::Weapon),
                ItemSocket::empty(SocketType::Weapon),
                ItemSocket::empty(SocketType::Universal),
            ]),
            ("Mithril Armor".to_string(), vec![
                ItemSocket::empty(SocketType::Armor),
                ItemSocket::empty(SocketType::Universal),
            ]),
        ];
        s.gem_inventory = vec![
            Gem::new(GemType::Ruby, 3),
            Gem::new(GemType::Sapphire, 2),
            Gem::new(GemType::Diamond, 4),
            Gem::new(GemType::Emerald, 1),
        ];
        s
    }
}

pub fn show_socketing_editor(ui: &mut egui::Ui, state: &mut SocketingEditorState) {
    ui.heading("Item Socketing");

    ui.columns(2, |cols| {
        egui::ScrollArea::vertical().max_height(250.0).show(&mut cols[0], |ui| {
            ui.heading("Items");
            for (i, (name, sockets)) in state.items_with_sockets.iter().enumerate() {
                let sel = state.selected_item == Some(i);
                if ui.selectable_label(sel, format!("{} ({} sockets)", name, sockets.len())).clicked() {
                    state.selected_item = Some(i);
                    state.selected_socket = None;
                }
            }
        });

        egui::ScrollArea::vertical().max_height(250.0).show(&mut cols[1], |ui| {
            if let Some(item_idx) = state.selected_item {
                if let Some((name, sockets)) = state.items_with_sockets.get(item_idx) {
                    ui.heading(name.clone());
                    for (si, socket) in sockets.iter().enumerate() {
                        let sel = state.selected_socket == Some(si);
                        let label = if let Some(gem) = &socket.inserted_gem {
                            format!("[{:?}] {} Q{}", socket.socket_type, gem.gem_type.name(), gem.quality)
                        } else {
                            format!("[{:?}] Empty", socket.socket_type)
                        };
                        if ui.selectable_label(sel, label).clicked() { state.selected_socket = Some(si); }
                    }
                }
            }
        });
    });

    ui.separator();
    ui.heading("Gem Inventory");
    let mut gem_to_insert = None;
    for (gi, gem) in state.gem_inventory.iter().enumerate() {
        let sel = state.selected_gem == Some(gi);
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, &gem.name).clicked() { state.selected_gem = Some(gi); }
            let bonus = gem.get_bonus();
            ui.colored_label(gem.gem_type.color(), format!("ATK:{} DEF:{} HP:{} MP:{}", bonus.attack, bonus.defense, bonus.health, bonus.mana));
            if ui.button("Insert").clicked() {
                if let (Some(item_i), Some(socket_i)) = (state.selected_item, state.selected_socket) {
                    gem_to_insert = Some((item_i, socket_i, gi));
                }
            }
        });
    }

    if let Some((item_i, socket_i, gem_i)) = gem_to_insert {
        if gem_i < state.gem_inventory.len() {
            let gem = state.gem_inventory.remove(gem_i);
            if item_i < state.items_with_sockets.len() {
                if socket_i < state.items_with_sockets[item_i].1.len() {
                    let _ = state.items_with_sockets[item_i].1[socket_i].insert(gem);
                }
            }
        }
    }
}

// =================================================================
// INVENTORY EXPANSION: ITEM TRADING / AUCTION HOUSE
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuctionListing {
    pub id: u32,
    pub seller: String,
    pub item_name: String,
    pub item_stack: u32,
    pub buyout_price: u64,
    pub bid_price: u64,
    pub current_bid: u64,
    pub bid_count: u32,
    pub time_remaining_hours: f32,
    pub category: String,
    pub quality: u32,
    pub level_requirement: u32,
}

impl AuctionListing {
    pub fn new(id: u32, seller: &str, item: &str, buyout: u64) -> Self {
        Self {
            id, seller: seller.to_string(), item_name: item.to_string(),
            item_stack: 1, buyout_price: buyout, bid_price: buyout / 2,
            current_bid: 0, bid_count: 0, time_remaining_hours: 48.0,
            category: String::new(), quality: 1, level_requirement: 1,
        }
    }

    pub fn is_expired(&self) -> bool { self.time_remaining_hours <= 0.0 }

    pub fn tick(&mut self, dt_hours: f32) {
        self.time_remaining_hours = (self.time_remaining_hours - dt_hours).max(0.0);
    }

    pub fn place_bid(&mut self, amount: u64, bidder: &str) -> bool {
        if amount > self.current_bid && amount >= self.bid_price {
            self.current_bid = amount;
            self.bid_count += 1;
            true
        } else { false }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AuctionHouseState {
    pub listings: Vec<AuctionListing>,
    pub search_query: String,
    pub filter_category: String,
    pub sort_by: AuctionSortBy,
    pub sort_ascending: bool,
    pub selected_listing: Option<usize>,
    pub bid_amount: u64,
    pub my_gold: u64,
    pub next_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum AuctionSortBy {
    #[default]
    TimeRemaining,
    Price,
    Name,
    Quality,
    BidCount,
}

impl AuctionHouseState {
    pub fn new() -> Self {
        let mut s = Self { my_gold: 10000, next_id: 10, ..Default::default() };
        s.listings = vec![
            AuctionListing::new(1, "Merchant_A", "Iron Sword", 250),
            AuctionListing::new(2, "Blacksmith_99", "Steel Armor", 1500),
            AuctionListing::new(3, "Wizard_Grix", "Fire Staff", 3200),
            AuctionListing::new(4, "Hunter_Tom", "Elven Bow", 800),
            AuctionListing::new(5, "Alchemist", "Health Potion x10", 120),
        ];
        for (i, listing) in s.listings.iter_mut().enumerate() {
            listing.category = ["Weapons", "Armor", "Weapons", "Weapons", "Consumables"][i].to_string();
            listing.quality = [2, 3, 4, 3, 1][i];
            listing.time_remaining_hours = [12.0, 24.0, 48.0, 6.0, 2.0][i];
        }
        s
    }

    pub fn sorted_visible_listings(&self) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        let mut indices: Vec<usize> = self.listings.iter().enumerate()
            .filter(|(_, l)| !l.is_expired() &&
                (q.is_empty() || l.item_name.to_lowercase().contains(&q)) &&
                (self.filter_category.is_empty() || l.category == self.filter_category)
            )
            .map(|(i, _)| i).collect();
        indices.sort_by(|&a, &b| {
            let la = &self.listings[a];
            let lb = &self.listings[b];
            let cmp = match self.sort_by {
                AuctionSortBy::TimeRemaining => la.time_remaining_hours.partial_cmp(&lb.time_remaining_hours).unwrap(),
                AuctionSortBy::Price => la.buyout_price.cmp(&lb.buyout_price),
                AuctionSortBy::Name => la.item_name.cmp(&lb.item_name),
                AuctionSortBy::Quality => la.quality.cmp(&lb.quality),
                AuctionSortBy::BidCount => la.bid_count.cmp(&lb.bid_count),
            };
            if self.sort_ascending { cmp } else { cmp.reverse() }
        });
        indices
    }
}

pub fn show_auction_house(ui: &mut egui::Ui, state: &mut AuctionHouseState) {
    ui.heading("Auction House");
    ui.label(format!("Gold: {} g", state.my_gold));
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search_query);
        ui.label("Category:");
        egui::ComboBox::from_id_source("ah_category")
            .selected_text(if state.filter_category.is_empty() { "All" } else { &state.filter_category })
            .show_ui(ui, |ui| {
                for cat in ["", "Weapons", "Armor", "Consumables", "Misc"] {
                    if ui.selectable_label(state.filter_category == cat, if cat.is_empty() { "All" } else { cat }).clicked() {
                        state.filter_category = cat.to_string();
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Sort:");
        for (sort, label) in [
            (AuctionSortBy::TimeRemaining, "Time"),
            (AuctionSortBy::Price, "Price"),
            (AuctionSortBy::Name, "Name"),
            (AuctionSortBy::Quality, "Quality"),
        ] {
            if ui.selectable_label(state.sort_by == sort, label).clicked() {
                if state.sort_by == sort { state.sort_ascending = !state.sort_ascending; }
                else { state.sort_by = sort; state.sort_ascending = true; }
            }
        }
    });

    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        let visible = state.sorted_visible_listings();
        for i in visible {
            let listing = &state.listings[i];
            let sel = state.selected_listing == Some(i);
            ui.horizontal(|ui| {
                if ui.selectable_label(sel, &listing.item_name).clicked() {
                    state.selected_listing = Some(i);
                    state.bid_amount = listing.current_bid + 1;
                }
                ui.label(format!("Q{}", listing.quality));
                ui.colored_label(egui::Color32::YELLOW, format!("{} g", listing.buyout_price));
                ui.label(format!("Bids: {} | {:.0}h left", listing.bid_count, listing.time_remaining_hours));
            });
        }
    });

    if let Some(idx) = state.selected_listing {
        if let Some(listing) = state.listings.get_mut(idx) {
            ui.separator();
            ui.label(format!("Selected: {}", listing.item_name));
            ui.label(format!("Seller: {} | Current Bid: {} g", listing.seller, listing.current_bid));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut state.bid_amount).prefix("Bid: ").suffix(" g"));
                if ui.button("Place Bid").clicked() {
                    if state.my_gold >= state.bid_amount {
                        let bid = state.bid_amount;
                        if listing.place_bid(bid, "Player") {
                            state.my_gold -= bid;
                        }
                    }
                }
                if ui.button(format!("Buy Now: {} g", listing.buyout_price)).clicked() {
                    if state.my_gold >= listing.buyout_price {
                        state.my_gold -= listing.buyout_price;
                        listing.time_remaining_hours = 0.0;
                    }
                }
            });
        }
    }
}

// =================================================================
// INVENTORY EXPANSION: LOOT TABLE DESIGNER
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LootEntry {
    pub item_name: String,
    pub min_quantity: u32,
    pub max_quantity: u32,
    pub weight: f32,
    pub guaranteed: bool,
    pub level_scale: bool,
    pub min_level: u32,
    pub max_level: u32,
    pub rarity_override: Option<String>,
}

impl LootEntry {
    pub fn new(name: &str, weight: f32) -> Self {
        Self { item_name: name.to_string(), min_quantity: 1, max_quantity: 1, weight, guaranteed: false, level_scale: false, min_level: 1, max_level: 100, rarity_override: None }
    }

    pub fn guaranteed(name: &str, qty: u32) -> Self {
        let mut e = Self::new(name, 1.0);
        e.min_quantity = qty; e.max_quantity = qty; e.guaranteed = true; e
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LootTable {
    pub id: u32,
    pub name: String,
    pub entries: Vec<LootEntry>,
    pub rolls_min: u32,
    pub rolls_max: u32,
    pub luck_modifier: f32,
    pub category: String,
    pub description: String,
}

impl LootTable {
    pub fn new(id: u32, name: &str) -> Self {
        Self { id, name: name.to_string(), entries: Vec::new(), rolls_min: 1, rolls_max: 3, luck_modifier: 0.0, category: String::new(), description: String::new() }
    }

    pub fn total_weight(&self) -> f32 {
        self.entries.iter().filter(|e| !e.guaranteed).map(|e| e.weight).sum()
    }

    pub fn simulate_roll(&self, rng: &mut InvRng, level: u32) -> Vec<(String, u32)> {
        let mut results = Vec::new();
        // Always include guaranteed entries
        for entry in self.entries.iter().filter(|e| e.guaranteed) {
            results.push((entry.item_name.clone(), entry.min_quantity));
        }
        // Random rolls
        let roll_count = self.rolls_min + (rng.next_u64() % (self.rolls_max - self.rolls_min + 1) as u64) as u32;
        let optional: Vec<&LootEntry> = self.entries.iter().filter(|e| !e.guaranteed && level >= e.min_level && level <= e.max_level).collect();
        let total_weight: f32 = optional.iter().map(|e| e.weight).sum();
        if total_weight <= 0.0 { return results; }

        for _ in 0..roll_count {
            let roll = rng.next_f32() * total_weight;
            let mut accum = 0.0f32;
            for entry in &optional {
                accum += entry.weight;
                if roll <= accum {
                    let qty = entry.min_quantity + (rng.next_u64() % ((entry.max_quantity - entry.min_quantity + 1) as u64)) as u32;
                    results.push((entry.item_name.clone(), qty));
                    break;
                }
            }
        }
        results
    }

    pub fn goblin_loot() -> Self {
        let mut t = LootTable::new(1, "Goblin Loot");
        t.entries = vec![
            LootEntry::guaranteed("Gold Coin", 3),
            LootEntry::new("Rusty Dagger", 20.0),
            LootEntry::new("Leather Scrap", 30.0),
            LootEntry::new("Healing Herb", 25.0),
            LootEntry::new("Goblin Ear", 15.0),
            LootEntry::new("Small Shield", 8.0),
            LootEntry::new("Crude Bow", 5.0),
            { let mut e = LootEntry::new("Steel Sword", 2.0); e.level_scale = true; e.min_level = 10; e },
        ];
        t.rolls_min = 1; t.rolls_max = 4;
        t
    }

    pub fn dragon_hoard() -> Self {
        let mut t = LootTable::new(2, "Dragon Hoard");
        t.entries = vec![
            LootEntry::guaranteed("Dragon Scale", 5),
            LootEntry::guaranteed("Gold Coin", 200),
            LootEntry::new("Dragon Sword", 15.0),
            LootEntry::new("Fire Tome", 10.0),
            LootEntry::new("Dragon Armor", 8.0),
            LootEntry::new("Ancient Relic", 5.0),
            LootEntry::new("Dragon Eye Gem", 3.0),
            { let mut e = LootEntry::new("Legendary Dragon Blade", 1.0); e.min_level = 50; e },
        ];
        t.rolls_min = 3; t.rolls_max = 7;
        t
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LootTableEditorState {
    pub tables: Vec<LootTable>,
    pub selected_table: Option<usize>,
    pub selected_entry: Option<usize>,
    pub preview_level: u32,
    pub preview_results: Vec<(String, u32)>,
    pub preview_count: u32,
    pub preview_seed: u64,
}

impl LootTableEditorState {
    pub fn new() -> Self {
        let mut s = Self { preview_level: 10, preview_count: 10, preview_seed: 42, ..Default::default() };
        s.tables = vec![LootTable::goblin_loot(), LootTable::dragon_hoard()];
        s
    }
}

pub fn show_loot_table_editor(ui: &mut egui::Ui, state: &mut LootTableEditorState) {
    ui.heading("Loot Table Designer");
    ui.horizontal(|ui| {
        if ui.button("+ New Table").clicked() {
            let id = state.tables.len() as u32 + 1;
            state.tables.push(LootTable::new(id, &format!("Table {}", id)));
        }
        if ui.button("Add Goblin Loot").clicked() { state.tables.push(LootTable::goblin_loot()); }
        if ui.button("Add Dragon Hoard").clicked() { state.tables.push(LootTable::dragon_hoard()); }
    });

    ui.columns(2, |cols| {
        egui::ScrollArea::vertical().max_height(200.0).show(&mut cols[0], |ui| {
            for (i, table) in state.tables.iter().enumerate() {
                let sel = state.selected_table == Some(i);
                if ui.selectable_label(sel, format!("{} ({} entries)", table.name, table.entries.len())).clicked() {
                    state.selected_table = Some(i);
                    state.selected_entry = None;
                }
            }
        });

        if let Some(ti) = state.selected_table {
            if let Some(table) = state.tables.get_mut(ti) {
                egui::ScrollArea::vertical().max_height(200.0).show(&mut cols[1], |ui| {
                    ui.text_edit_singleline(&mut table.name);
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut table.rolls_min).clamp_range(0..=20u32).prefix("Rolls min: "));
                        ui.add(egui::DragValue::new(&mut table.rolls_max).clamp_range(1..=20u32).prefix("max: "));
                    });
                    if ui.button("+ Add Entry").clicked() {
                        table.entries.push(LootEntry::new("New Item", 10.0));
                    }
                    let mut to_remove = None;
                    for (i, entry) in table.entries.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut entry.item_name);
                            ui.add(egui::DragValue::new(&mut entry.weight).clamp_range(0.0..=100.0f32).speed(0.1).prefix("W:"));
                            ui.add(egui::DragValue::new(&mut entry.min_quantity).clamp_range(1..=999u32).prefix("Q:"));
                            ui.checkbox(&mut entry.guaranteed, "G");
                            if ui.button("🗑").clicked() { to_remove = Some(i); }
                        });
                    }
                    if let Some(i) = to_remove { table.entries.remove(i); }
                });
            }
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut state.preview_level, 1..=100).text("Level"));
        ui.add(egui::DragValue::new(&mut state.preview_count).clamp_range(1..=1000u32).prefix("Rolls: "));
        ui.add(egui::DragValue::new(&mut state.preview_seed).prefix("Seed: "));
        if ui.button("Simulate Loot").clicked() {
            if let Some(ti) = state.selected_table {
                if let Some(table) = state.tables.get(ti) {
                    let mut rng = InvRng::new(state.preview_seed);
                    let mut combined: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
                    for _ in 0..state.preview_count {
                        let roll = table.simulate_roll(&mut rng, state.preview_level);
                        for (name, qty) in roll {
                            *combined.entry(name).or_default() += qty;
                        }
                    }
                    state.preview_results = combined.into_iter().collect();
                    state.preview_results.sort_by(|a, b| b.1.cmp(&a.1));
                }
            }
        }
    });

    if !state.preview_results.is_empty() {
        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            for (name, qty) in &state.preview_results {
                ui.label(format!("{}: {}", name, qty));
            }
        });
    }
}

// =================================================================
// INVENTORY EXPANSION: FULL INVENTORY EDITOR INTEGRATION
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MasterInventoryEditorState {
    pub crafting: CraftingEditorState,
    pub durability: DurabilityEditorState,
    pub socketing: SocketingEditorState,
    pub auction: AuctionHouseState,
    pub loot_tables: LootTableEditorState,
    pub active_tab: MasterInventoryTab,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum MasterInventoryTab {
    #[default]
    Crafting,
    Durability,
    Socketing,
    AuctionHouse,
    LootTables,
    Enchanting,
    SetBonuses,
    ItemGenerator,
    Economy,
    Containers,
}

impl MasterInventoryEditorState {
    pub fn new() -> Self {
        Self {
            crafting: CraftingEditorState::with_default_recipes(),
            durability: DurabilityEditorState::new(),
            socketing: SocketingEditorState::new(),
            auction: AuctionHouseState::new(),
            loot_tables: LootTableEditorState::new(),
            active_tab: MasterInventoryTab::Crafting,
        }
    }
}

pub fn show_master_inventory_editor(ui: &mut egui::Ui, state: &mut MasterInventoryEditorState) {
    ui.horizontal(|ui| {
        for (tab, label) in [
            (MasterInventoryTab::Crafting, "Crafting"),
            (MasterInventoryTab::Durability, "Durability"),
            (MasterInventoryTab::Socketing, "Sockets"),
            (MasterInventoryTab::AuctionHouse, "Auction"),
            (MasterInventoryTab::LootTables, "Loot"),
            (MasterInventoryTab::Enchanting, "Enchant"),
            (MasterInventoryTab::SetBonuses, "Sets"),
            (MasterInventoryTab::ItemGenerator, "Generator"),
            (MasterInventoryTab::Economy, "Economy"),
            (MasterInventoryTab::Containers, "Containers"),
        ] {
            if ui.selectable_label(state.active_tab == tab, label).clicked() {
                state.active_tab = tab;
            }
        }
    });
    ui.separator();
    match state.active_tab {
        MasterInventoryTab::Crafting => show_crafting_editor(ui, &mut state.crafting),
        MasterInventoryTab::Durability => show_durability_editor(ui, &mut state.durability),
        MasterInventoryTab::Socketing => show_socketing_editor(ui, &mut state.socketing),
        MasterInventoryTab::AuctionHouse => show_auction_house(ui, &mut state.auction),
        MasterInventoryTab::LootTables => show_loot_table_editor(ui, &mut state.loot_tables),
        _ => { ui.label("Select a tab above to view the editor."); }
    }
}

// =================================================================
// INVENTORY EXPANSION: TESTS
// =================================================================

#[cfg(test)]
mod inventory_expansion_tests {
    use super::*;

    #[test]
    fn test_crafting_recipe_iron_sword() {
        let recipe = CraftingRecipe::iron_sword();
        assert_eq!(recipe.name, "Iron Sword");
        assert!(!recipe.ingredients.is_empty());
        assert!(!recipe.outputs.is_empty());
    }

    #[test]
    fn test_crafting_editor_state() {
        let state = CraftingEditorState::with_default_recipes();
        assert!(!state.recipes.is_empty());
        assert_eq!(state.level(), 1);
    }

    #[test]
    fn test_durability_lifecycle() {
        let mut dur = DurabilityInfo::new(100.0, RepairMaterial::Iron);
        assert_eq!(dur.percentage(), 1.0);
        dur.apply_use_damage(50.0);
        assert!((dur.percentage() - 0.5).abs() < 0.001);
        dur.repair(30.0);
        assert!((dur.current - 80.0).abs() < 0.001);
    }

    #[test]
    fn test_durability_broken_state() {
        let mut dur = DurabilityInfo::new(10.0, RepairMaterial::Iron);
        dur.apply_use_damage(100.0);
        assert!(dur.broken);
        assert_eq!(dur.current, 0.0);
    }

    #[test]
    fn test_gem_stat_bonus() {
        let gem = Gem::new(GemType::Ruby, 1);
        let bonus = gem.get_bonus();
        assert!(bonus.attack > 0);
    }

    #[test]
    fn test_gem_socket_insert() {
        let mut socket = ItemSocket::empty(SocketType::Weapon);
        let gem = Gem::new(GemType::Ruby, 2);
        assert!(socket.insert(gem).is_ok());
        assert!(socket.inserted_gem.is_some());
    }

    #[test]
    fn test_gem_socket_double_insert() {
        let mut socket = ItemSocket::empty(SocketType::Weapon);
        let gem1 = Gem::new(GemType::Ruby, 1);
        let gem2 = Gem::new(GemType::Sapphire, 1);
        assert!(socket.insert(gem1).is_ok());
        assert!(socket.insert(gem2).is_err());
    }

    #[test]
    fn test_loot_table_simulation() {
        let table = LootTable::goblin_loot();
        let mut rng = InvRng::new(42);
        let results = table.simulate_roll(&mut rng, 5);
        assert!(!results.is_empty());
        // Gold coins are guaranteed
        assert!(results.iter().any(|(name, _)| name.contains("Gold")));
    }

    #[test]
    fn test_loot_table_dragon() {
        let table = LootTable::dragon_hoard();
        let mut rng = InvRng::new(99);
        let results = table.simulate_roll(&mut rng, 50);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_auction_house_bid() {
        let mut listing = AuctionListing::new(1, "Seller", "Item", 100);
        assert!(listing.place_bid(60, "Bidder"));
        assert_eq!(listing.bid_count, 1);
        assert!(!listing.place_bid(30, "Bidder2")); // Lower than current bid
    }

    #[test]
    fn test_auction_house_expiry() {
        let mut listing = AuctionListing::new(1, "Seller", "Item", 100);
        listing.time_remaining_hours = 1.0;
        listing.tick(2.0);
        assert!(listing.is_expired());
    }

    #[test]
    fn test_crafting_queue() {
        let mut state = CraftingEditorState::with_default_recipes();
        state.add_to_queue(1, 3);
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.queue[0].quantity, 3);
        state.add_to_queue(1, 2);
        assert_eq!(state.queue[0].quantity, 5); // Stacked
    }

    #[test]
    fn test_repair_material_costs() {
        assert!(RepairMaterial::Mithril.base_cost() > RepairMaterial::Iron.base_cost());
        assert!(RepairMaterial::Dragonbone.base_cost() > RepairMaterial::Mithril.base_cost());
    }

    #[test]
    fn test_stat_bonus_builder() {
        let b = StatBonus::default().with_attack(10).with_defense(5).with_health(50);
        assert_eq!(b.attack, 10);
        assert_eq!(b.defense, 5);
        assert_eq!(b.health, 50);
    }

    #[test]
    fn test_item_set_active_bonus() {
        let set = ItemSet::dragonscale();
        assert!(set.get_active_bonus(1).is_none());
        assert!(set.get_active_bonus(2).is_some());
        assert!(set.get_active_bonus(6).is_some());
    }
}
'''

with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'a', encoding='utf-8') as f:
    f.write(code)

print(f"Done. inventory_system size: {__import__('os').path.getsize(r'C:\proof-engine\editor\src\inventory_system.rs')} bytes")
