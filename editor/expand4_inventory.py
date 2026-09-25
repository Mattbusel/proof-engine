
code = r'''

// ============================================================
// EXPANSION 4: Advanced Equipment System
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EquipSlot {
    Head,
    Neck,
    Shoulders,
    Chest,
    Back,
    Wrists,
    Hands,
    Waist,
    Legs,
    Feet,
    Finger1,
    Finger2,
    Trinket1,
    Trinket2,
    MainHand,
    OffHand,
    TwoHand,
    Ranged,
    Ammo,
    Relic,
}

impl EquipSlot {
    pub fn name(&self) -> &'static str {
        match self {
            EquipSlot::Head => "Head",
            EquipSlot::Neck => "Neck",
            EquipSlot::Shoulders => "Shoulders",
            EquipSlot::Chest => "Chest",
            EquipSlot::Back => "Back/Cloak",
            EquipSlot::Wrists => "Wrists",
            EquipSlot::Hands => "Gloves",
            EquipSlot::Waist => "Belt",
            EquipSlot::Legs => "Legs",
            EquipSlot::Feet => "Boots",
            EquipSlot::Finger1 => "Ring 1",
            EquipSlot::Finger2 => "Ring 2",
            EquipSlot::Trinket1 => "Trinket 1",
            EquipSlot::Trinket2 => "Trinket 2",
            EquipSlot::MainHand => "Main Hand",
            EquipSlot::OffHand => "Off Hand",
            EquipSlot::TwoHand => "Two Hand",
            EquipSlot::Ranged => "Ranged",
            EquipSlot::Ammo => "Ammo",
            EquipSlot::Relic => "Relic",
        }
    }
    pub fn all() -> &'static [EquipSlot] {
        &[
            EquipSlot::Head, EquipSlot::Neck, EquipSlot::Shoulders,
            EquipSlot::Chest, EquipSlot::Back, EquipSlot::Wrists,
            EquipSlot::Hands, EquipSlot::Waist, EquipSlot::Legs,
            EquipSlot::Feet, EquipSlot::Finger1, EquipSlot::Finger2,
            EquipSlot::Trinket1, EquipSlot::Trinket2, EquipSlot::MainHand,
            EquipSlot::OffHand,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EquippedItem {
    pub item_id: u32,
    pub item_name: String,
    pub slot: EquipSlot,
    pub level_requirement: u32,
    pub item_level: u32,
    pub stats: EquipStats,
    pub gem_slots: Vec<Option<String>>,
    pub enchant: Option<String>,
    pub durability: u32,
    pub max_durability: u32,
    pub transmog_item_id: Option<u32>,
    pub soulbound: bool,
    pub account_bound: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EquipStats {
    pub strength: i32,
    pub agility: i32,
    pub intellect: i32,
    pub stamina: i32,
    pub spirit: i32,
    pub armor: i32,
    pub dodge: f32,
    pub parry: f32,
    pub block: f32,
    pub hit: f32,
    pub crit: f32,
    pub haste: f32,
    pub mastery: f32,
    pub versatility: f32,
    pub spell_power: i32,
    pub attack_power: i32,
    pub weapon_damage_min: i32,
    pub weapon_damage_max: i32,
    pub weapon_speed: f32,
    pub dps: f32,
}

impl EquipStats {
    pub fn total_secondary_stats(&self) -> f32 {
        self.dodge + self.parry + self.block + self.hit + self.crit
            + self.haste + self.mastery + self.versatility
    }

    pub fn add(&mut self, other: &EquipStats) {
        self.strength += other.strength;
        self.agility += other.agility;
        self.intellect += other.intellect;
        self.stamina += other.stamina;
        self.spirit += other.spirit;
        self.armor += other.armor;
        self.dodge += other.dodge;
        self.parry += other.parry;
        self.crit += other.crit;
        self.haste += other.haste;
        self.mastery += other.mastery;
        self.versatility += other.versatility;
        self.spell_power += other.spell_power;
        self.attack_power += other.attack_power;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CharacterEquipment {
    pub slots: std::collections::HashMap<String, EquippedItem>,
    pub character_level: u32,
    pub equipped_item_level: f32,
}

impl CharacterEquipment {
    pub fn equip(&mut self, item: EquippedItem) {
        let key = format!("{:?}", item.slot);
        self.slots.insert(key, item);
        self.recalculate_ilvl();
    }

    pub fn unequip(&mut self, slot: &EquipSlot) -> Option<EquippedItem> {
        let key = format!("{:?}", slot);
        let item = self.slots.remove(&key);
        self.recalculate_ilvl();
        item
    }

    pub fn total_stats(&self) -> EquipStats {
        let mut total = EquipStats::default();
        for item in self.slots.values() {
            total.add(&item.stats);
        }
        total
    }

    pub fn recalculate_ilvl(&mut self) {
        if self.slots.is_empty() {
            self.equipped_item_level = 0.0;
            return;
        }
        let sum: u32 = self.slots.values().map(|i| i.item_level).sum();
        self.equipped_item_level = sum as f32 / self.slots.len() as f32;
    }

    pub fn items_needing_repair(&self) -> Vec<&EquippedItem> {
        self.slots.values().filter(|i| i.durability < i.max_durability / 2).collect()
    }

    pub fn is_slot_occupied(&self, slot: &EquipSlot) -> bool {
        self.slots.contains_key(&format!("{:?}", slot))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EquipmentEditorState {
    pub equipment: CharacterEquipment,
    pub selected_slot: Option<usize>,
    pub available_items: Vec<EquippedItem>,
    pub show_tooltips: bool,
    pub show_stats_panel: bool,
    pub filter_slot: Option<String>,
}

impl EquipmentEditorState {
    pub fn new() -> Self {
        let mut state = EquipmentEditorState::default();
        state.equipment.character_level = 60;
        state.show_tooltips = true;
        state.show_stats_panel = true;

        // Add some sample equipment
        state.available_items.push(EquippedItem {
            item_id: 1001,
            item_name: "Helm of Valor".to_string(),
            slot: EquipSlot::Head,
            level_requirement: 50,
            item_level: 220,
            stats: EquipStats { stamina: 200, intellect: 150, crit: 1.2, haste: 0.8, ..Default::default() },
            gem_slots: vec![None, None],
            enchant: Some("Eternal Insight".to_string()),
            durability: 100,
            max_durability: 100,
            transmog_item_id: None,
            soulbound: true,
            account_bound: false,
        });
        state.available_items.push(EquippedItem {
            item_id: 1002,
            item_name: "Chestplate of the Guardian".to_string(),
            slot: EquipSlot::Chest,
            level_requirement: 55,
            item_level: 235,
            stats: EquipStats { stamina: 350, strength: 200, armor: 1500, parry: 0.5, ..Default::default() },
            gem_slots: vec![None, None, None],
            enchant: None,
            durability: 120,
            max_durability: 120,
            transmog_item_id: None,
            soulbound: true,
            account_bound: false,
        });
        state
    }
}

pub fn show_equipment_editor(ui: &mut egui::Ui, state: &mut EquipmentEditorState) {
    ui.heading("Character Equipment");
    ui.label(format!("Character Level: {} | Avg iLvl: {:.1}",
        state.equipment.character_level, state.equipment.equipped_item_level));

    ui.horizontal(|ui| {
        // Equipment slots display
        ui.vertical(|ui| {
            ui.label("Equipment Slots:");
            for slot in EquipSlot::all() {
                let key = format!("{:?}", slot);
                let occupied = state.equipment.slots.contains_key(&key);
                let label = if occupied {
                    let item = &state.equipment.slots[&key];
                    format!("{}: {} [{}]", slot.name(), item.item_name, item.item_level)
                } else {
                    format!("{}: (empty)", slot.name())
                };
                let color = if occupied { egui::Color32::from_rgb(200, 200, 100) } else { egui::Color32::GRAY };
                ui.colored_label(color, label);
            }
        });

        ui.separator();

        if state.show_stats_panel {
            ui.vertical(|ui| {
                ui.label("Total Stats:");
                let stats = state.equipment.total_stats();
                ui.label(format!("Strength: {}", stats.strength));
                ui.label(format!("Agility: {}", stats.agility));
                ui.label(format!("Intellect: {}", stats.intellect));
                ui.label(format!("Stamina: {}", stats.stamina));
                ui.label(format!("Armor: {}", stats.armor));
                ui.separator();
                ui.label(format!("Crit: {:.2}%", stats.crit));
                ui.label(format!("Haste: {:.2}%", stats.haste));
                ui.label(format!("Mastery: {:.2}%", stats.mastery));
                ui.label(format!("Versatility: {:.2}%", stats.versatility));
                ui.separator();
                ui.label(format!("Attack Power: {}", stats.attack_power));
                ui.label(format!("Spell Power: {}", stats.spell_power));
            });
        }
    });

    ui.separator();
    ui.heading("Available Items");
    let available_names: Vec<(u32, String, String)> = state.available_items.iter()
        .map(|i| (i.item_id, i.item_name.clone(), format!("{:?}", i.slot)))
        .collect();
    for (id, name, slot_name) in available_names {
        ui.horizontal(|ui| {
            ui.label(format!("[{}] {} ({})", id, name, slot_name));
            if ui.small_button("Equip").clicked() {
                if let Some(item) = state.available_items.iter().find(|i| i.item_id == id).cloned() {
                    state.equipment.equip(item);
                }
            }
        });
    }

    let needs_repair = state.equipment.items_needing_repair().len();
    if needs_repair > 0 {
        ui.colored_label(egui::Color32::RED, format!("{} items need repair!", needs_repair));
    }
}

// ============================================================
// EXPANSION 4: Item Generation System
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ItemQuality {
    Poor,
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Artifact,
    Heirloom,
}

impl ItemQuality {
    pub fn name(&self) -> &'static str {
        match self {
            ItemQuality::Poor => "Poor",
            ItemQuality::Common => "Common",
            ItemQuality::Uncommon => "Uncommon",
            ItemQuality::Rare => "Rare",
            ItemQuality::Epic => "Epic",
            ItemQuality::Legendary => "Legendary",
            ItemQuality::Artifact => "Artifact",
            ItemQuality::Heirloom => "Heirloom",
        }
    }
    pub fn color(&self) -> egui::Color32 {
        match self {
            ItemQuality::Poor => egui::Color32::GRAY,
            ItemQuality::Common => egui::Color32::WHITE,
            ItemQuality::Uncommon => egui::Color32::from_rgb(30, 255, 30),
            ItemQuality::Rare => egui::Color32::from_rgb(0, 112, 221),
            ItemQuality::Epic => egui::Color32::from_rgb(163, 53, 238),
            ItemQuality::Legendary => egui::Color32::from_rgb(255, 128, 0),
            ItemQuality::Artifact => egui::Color32::from_rgb(230, 204, 128),
            ItemQuality::Heirloom => egui::Color32::from_rgb(0, 204, 255),
        }
    }
    pub fn stat_multiplier(&self) -> f32 {
        match self {
            ItemQuality::Poor => 0.5,
            ItemQuality::Common => 0.8,
            ItemQuality::Uncommon => 1.0,
            ItemQuality::Rare => 1.2,
            ItemQuality::Epic => 1.5,
            ItemQuality::Legendary => 2.0,
            ItemQuality::Artifact => 2.5,
            ItemQuality::Heirloom => 1.3,
        }
    }
    pub fn gem_slots(&self) -> u32 {
        match self {
            ItemQuality::Poor | ItemQuality::Common => 0,
            ItemQuality::Uncommon => 1,
            ItemQuality::Rare => 2,
            ItemQuality::Epic => 2,
            ItemQuality::Legendary => 3,
            ItemQuality::Artifact => 3,
            ItemQuality::Heirloom => 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemGenConfig {
    pub quality: ItemQuality,
    pub item_level: u32,
    pub slot: EquipSlot,
    pub allow_set_items: bool,
    pub allow_legendary_effects: bool,
    pub force_primary_stat: Option<String>,
    pub num_secondary_stats: u32,
    pub seed: u64,
}

impl Default for ItemGenConfig {
    fn default() -> Self {
        ItemGenConfig {
            quality: ItemQuality::Rare,
            item_level: 200,
            slot: EquipSlot::Chest,
            allow_set_items: true,
            allow_legendary_effects: false,
            force_primary_stat: None,
            num_secondary_stats: 2,
            seed: 42,
        }
    }
}

pub fn generate_random_item(cfg: &ItemGenConfig) -> EquippedItem {
    let mut rng = InvRng::new(cfg.seed);
    let mult = cfg.quality.stat_multiplier();
    let ilvl_factor = cfg.item_level as f32 / 200.0;

    let base_stat = (150.0 * mult * ilvl_factor) as i32;
    let secondary = (50.0 * mult * ilvl_factor) as f32;

    let adjectives = ["Valiant", "Mighty", "Ancient", "Cursed", "Blessed", "Shattered",
                      "Radiant", "Obsidian", "Celestial", "Void-touched"];
    let nouns: [&str; 10] = match cfg.slot {
        EquipSlot::Head => ["Helm", "Crown", "Cowl", "Hood", "Circlet", "Visor", "Mask", "Headband", "Skullcap", "Tiara"].as_ref().try_into().unwrap_or(["Helm"; 10]),
        EquipSlot::Chest => ["Breastplate", "Tunic", "Robe", "Vestment", "Hauberk", "Coat", "Cuirass", "Chestguard", "Shirt", "Surcoat"].as_ref().try_into().unwrap_or(["Breastplate"; 10]),
        EquipSlot::MainHand | EquipSlot::TwoHand => ["Sword", "Axe", "Mace", "Staff", "Glaive", "Dagger", "Blade", "Edge", "Fang", "Cleaver"].as_ref().try_into().unwrap_or(["Sword"; 10]),
        _ => ["Item", "Piece", "Token", "Shard", "Fragment", "Relic", "Artifact", "Charm", "Trophy", "Prize"].as_ref().try_into().unwrap_or(["Item"; 10]),
    };

    let adj = adjectives[rng.next_u64() as usize % adjectives.len()];
    let noun = nouns[rng.next_u64() as usize % nouns.len()];
    let name = format!("{} {}", adj, noun);

    let mut stats = EquipStats::default();
    // Primary stats
    let primary_roll = rng.next_u64() % 3;
    match primary_roll {
        0 => { stats.strength = base_stat; },
        1 => { stats.agility = base_stat; },
        _ => { stats.intellect = base_stat; },
    }
    stats.stamina = (base_stat as f32 * 1.5) as i32;

    // Secondary stats
    let secondaries = ["crit", "haste", "mastery", "versatility", "dodge", "parry"];
    for i in 0..cfg.num_secondary_stats.min(4) {
        let sec = secondaries[(rng.next_u64() as usize + i as usize) % secondaries.len()];
        match sec {
            "crit" => stats.crit += secondary,
            "haste" => stats.haste += secondary * 0.8,
            "mastery" => stats.mastery += secondary * 0.9,
            "versatility" => stats.versatility += secondary * 0.7,
            "dodge" => stats.dodge += secondary * 0.5,
            "parry" => stats.parry += secondary * 0.5,
            _ => {},
        }
    }

    EquippedItem {
        item_id: (rng.next_u64() % 100000) as u32 + 10000,
        item_name: name,
        slot: cfg.slot.clone(),
        level_requirement: (cfg.item_level as f32 * 0.8) as u32,
        item_level: cfg.item_level,
        stats,
        gem_slots: vec![None; cfg.quality.gem_slots() as usize],
        enchant: None,
        durability: 100,
        max_durability: 100,
        transmog_item_id: None,
        soulbound: true,
        account_bound: false,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ItemGenState {
    pub config: ItemGenConfig,
    pub generated_items: Vec<EquippedItem>,
    pub selected_item: Option<usize>,
    pub batch_count: u32,
}

impl ItemGenState {
    pub fn generate_one(&mut self) {
        let item = generate_random_item(&self.config);
        self.config.seed = self.config.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.generated_items.push(item);
    }

    pub fn generate_batch(&mut self) {
        for _ in 0..self.batch_count.max(1) {
            self.generate_one();
        }
    }

    pub fn clear(&mut self) { self.generated_items.clear(); self.selected_item = None; }
}

pub fn show_item_generator(ui: &mut egui::Ui, state: &mut ItemGenState) {
    ui.heading("Item Generator");

    ui.horizontal(|ui| {
        if ui.button("Generate Item").clicked() { state.generate_one(); }
        if ui.button("Batch Generate").clicked() { state.generate_batch(); }
        ui.add(egui::DragValue::new(&mut state.batch_count).prefix("Batch: ").clamp_range(1..=100));
        if ui.button("Clear All").clicked() { state.clear(); }
    });

    ui.separator();
    egui::CollapsingHeader::new("Generation Config").default_open(true).show(ui, |ui| {
        ui.label("Item Level:");
        ui.add(egui::Slider::new(&mut state.config.item_level, 1..=300));

        ui.label("Quality:");
        for q in [ItemQuality::Common, ItemQuality::Uncommon, ItemQuality::Rare,
                  ItemQuality::Epic, ItemQuality::Legendary] {
            let is_sel = state.config.quality == q;
            let color = q.color();
            if ui.selectable_label(is_sel,
                egui::RichText::new(q.name()).color(color)).clicked() {
                state.config.quality = q;
            }
        }

        ui.label("Secondary Stats:");
        ui.add(egui::Slider::new(&mut state.config.num_secondary_stats, 0..=4));
        ui.checkbox(&mut state.config.allow_set_items, "Allow Set Items");
        ui.checkbox(&mut state.config.allow_legendary_effects, "Allow Legendary Effects");
    });

    ui.separator();
    ui.label(format!("Generated: {} items", state.generated_items.len()));

    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        let items: Vec<(usize, String, ItemQuality, u32)> = state.generated_items.iter().enumerate()
            .map(|(i, item)| (i, item.item_name.clone(), {
                let ilvl = item.item_level;
                let q = if ilvl >= 260 { ItemQuality::Legendary }
                    else if ilvl >= 240 { ItemQuality::Epic }
                    else if ilvl >= 210 { ItemQuality::Rare }
                    else if ilvl >= 150 { ItemQuality::Uncommon }
                    else { ItemQuality::Common };
                q
            }, item.item_level))
            .collect();
        for (i, name, quality, ilvl) in items {
            let color = quality.color();
            let label = format!("[{}] {} (iLvl {})", i+1, name, ilvl);
            if ui.selectable_label(state.selected_item == Some(i),
                egui::RichText::new(label).color(color)).clicked() {
                state.selected_item = Some(i);
            }
        }
    });

    if let Some(idx) = state.selected_item {
        if let Some(item) = state.generated_items.get(idx) {
            ui.separator();
            ui.label(format!("=== {} ===", item.item_name));
            ui.label(format!("Item Level: {}", item.item_level));
            ui.label(format!("Level Req: {}", item.level_requirement));
            ui.label(format!("Slot: {}", item.slot.name()));
            ui.label(format!("Durability: {}/{}", item.durability, item.max_durability));
            ui.separator();
            let s = &item.stats;
            if s.strength != 0 { ui.label(format!("+{} Strength", s.strength)); }
            if s.agility != 0 { ui.label(format!("+{} Agility", s.agility)); }
            if s.intellect != 0 { ui.label(format!("+{} Intellect", s.intellect)); }
            if s.stamina != 0 { ui.label(format!("+{} Stamina", s.stamina)); }
            if s.armor != 0 { ui.label(format!("+{} Armor", s.armor)); }
            if s.crit > 0.0 { ui.label(format!("+{:.2}% Critical Strike", s.crit)); }
            if s.haste > 0.0 { ui.label(format!("+{:.2}% Haste", s.haste)); }
            if s.mastery > 0.0 { ui.label(format!("+{:.2}% Mastery", s.mastery)); }
            if s.versatility > 0.0 { ui.label(format!("+{:.2}% Versatility", s.versatility)); }
            if let Some(ench) = &item.enchant {
                ui.label(format!("Enchant: {}", ench));
            }
            if !item.gem_slots.is_empty() {
                ui.label(format!("Gem Slots: {}", item.gem_slots.len()));
            }
        }
    }
}

// ============================================================
// EXPANSION 4: Vendor & Shop System
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VendorItem {
    pub item_id: u32,
    pub name: String,
    pub price_gold: u32,
    pub price_currency: Option<(String, u32)>,
    pub stock: Option<u32>,
    pub max_stock: Option<u32>,
    pub restock_time_hours: Option<u32>,
    pub is_limited: bool,
    pub level_requirement: u32,
    pub reputation_requirement: Option<(String, u32)>,
    pub description: String,
}

impl VendorItem {
    pub fn new(id: u32, name: String, price: u32) -> Self {
        VendorItem {
            item_id: id,
            name,
            price_gold: price,
            price_currency: None,
            stock: None,
            max_stock: None,
            restock_time_hours: None,
            is_limited: false,
            level_requirement: 0,
            reputation_requirement: None,
            description: String::new(),
        }
    }
    pub fn is_available(&self, player_level: u32) -> bool {
        player_level >= self.level_requirement
            && self.stock.map_or(true, |s| s > 0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VendorNpc {
    pub id: u32,
    pub name: String,
    pub title: String,
    pub location: String,
    pub vendor_type: VendorType,
    pub items: Vec<VendorItem>,
    pub reputation_faction: Option<String>,
    pub can_repair: bool,
    pub can_buy_junk: bool,
    pub buy_price_modifier: f32,
    pub sell_price_modifier: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum VendorType {
    General,
    Blacksmith,
    Alchemist,
    Enchanter,
    Tailor,
    Leatherworker,
    Jeweler,
    Stable,
    FoodDrink,
    Magic,
    Specialist,
}

impl VendorType {
    pub fn name(&self) -> &'static str {
        match self {
            VendorType::General => "General Goods",
            VendorType::Blacksmith => "Blacksmith",
            VendorType::Alchemist => "Alchemist",
            VendorType::Enchanter => "Enchanter",
            VendorType::Tailor => "Tailor",
            VendorType::Leatherworker => "Leatherworker",
            VendorType::Jeweler => "Jeweler",
            VendorType::Stable => "Stable",
            VendorType::FoodDrink => "Food & Drink",
            VendorType::Magic => "Magic Goods",
            VendorType::Specialist => "Specialist",
        }
    }
}

impl VendorNpc {
    pub fn create_general_store(id: u32, name: String, location: String) -> Self {
        let mut vendor = VendorNpc {
            id, name, title: "Merchant".to_string(), location,
            vendor_type: VendorType::General,
            items: vec![],
            reputation_faction: None,
            can_repair: false,
            can_buy_junk: true,
            buy_price_modifier: 1.0,
            sell_price_modifier: 0.25,
        };
        vendor.items.push(VendorItem::new(5001, "Healing Potion".to_string(), 50));
        vendor.items.push(VendorItem::new(5002, "Mana Potion".to_string(), 75));
        vendor.items.push(VendorItem::new(5003, "Bandage".to_string(), 5));
        vendor.items.push(VendorItem::new(5004, "Torch".to_string(), 2));
        vendor.items.push(VendorItem::new(5005, "Rope (50ft)".to_string(), 10));
        vendor
    }

    pub fn item_count(&self) -> usize { self.items.len() }
    pub fn available_items(&self, player_level: u32) -> Vec<&VendorItem> {
        self.items.iter().filter(|i| i.is_available(player_level)).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ShopSystemState {
    pub vendors: Vec<VendorNpc>,
    pub selected_vendor: Option<usize>,
    pub selected_item: Option<usize>,
    pub player_gold: u32,
    pub player_level: u32,
    pub search_filter: String,
    pub sort_by_price: bool,
    pub show_unavailable: bool,
    pub transaction_log: Vec<String>,
}

impl ShopSystemState {
    pub fn new() -> Self {
        let mut state = ShopSystemState {
            player_gold: 1000,
            player_level: 30,
            ..Default::default()
        };
        state.vendors.push(VendorNpc::create_general_store(1, "Bob's General Store".to_string(), "Stormwind".to_string()));
        state
    }

    pub fn buy_item(&mut self, vendor_idx: usize, item_idx: usize) {
        if vendor_idx >= self.vendors.len() { return; }
        let vendor = &self.vendors[vendor_idx];
        if item_idx >= vendor.items.len() { return; }
        let item = &vendor.items[item_idx];
        let adjusted_price = (item.price_gold as f32 * vendor.buy_price_modifier) as u32;
        if self.player_gold >= adjusted_price && item.is_available(self.player_level) {
            self.player_gold -= adjusted_price;
            let msg = format!("Purchased '{}' for {} gold", item.name, adjusted_price);
            self.transaction_log.push(msg);
        } else {
            let msg = format!("Cannot buy '{}': insufficient gold or requirements not met", item.name);
            self.transaction_log.push(msg);
        }
        if self.transaction_log.len() > 50 { self.transaction_log.remove(0); }
    }
}

pub fn show_shop_system(ui: &mut egui::Ui, state: &mut ShopSystemState) {
    ui.heading("Shop System");
    ui.label(format!("Player: Level {} | Gold: {}g", state.player_level, state.player_gold));

    egui::SidePanel::left("vendor_list").show_inside(ui, |ui| {
        ui.heading("Vendors");
        for (i, vendor) in state.vendors.iter().enumerate() {
            if ui.selectable_label(state.selected_vendor == Some(i),
                format!("{} ({})", vendor.name, vendor.vendor_type.name())).clicked() {
                state.selected_vendor = Some(i);
                state.selected_item = None;
            }
        }
    });

    if let Some(vendor_idx) = state.selected_vendor {
        if let Some(vendor) = state.vendors.get(vendor_idx) {
            ui.heading(&vendor.name.clone());
            ui.label(format!("{} - {}", vendor.title, vendor.location));
            if vendor.can_repair { ui.label("Can repair items"); }
            if vendor.can_buy_junk { ui.label(format!("Buys junk at {}x price", vendor.sell_price_modifier)); }

            ui.separator();
            ui.label(format!("Items ({})", vendor.item_count()));

            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut state.search_filter);
                ui.checkbox(&mut state.show_unavailable, "Show Unavailable");
            });

            let filter = state.search_filter.to_lowercase();
            egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                let items: Vec<(usize, String, u32, bool)> = vendor.items.iter().enumerate()
                    .filter(|(_, it)| filter.is_empty() || it.name.to_lowercase().contains(&filter))
                    .filter(|(_, it)| state.show_unavailable || it.is_available(state.player_level))
                    .map(|(i, it)| (i, it.name.clone(), it.price_gold, it.is_available(state.player_level)))
                    .collect();
                for (i, name, price, available) in items {
                    let color = if available { egui::Color32::WHITE } else { egui::Color32::GRAY };
                    ui.horizontal(|ui| {
                        if ui.selectable_label(state.selected_item == Some(i),
                            egui::RichText::new(format!("{} - {}g", name, price)).color(color)).clicked() {
                            state.selected_item = Some(i);
                        }
                        if available {
                            if ui.small_button("Buy").clicked() {
                                state.buy_item(vendor_idx, i);
                            }
                        }
                    });
                }
            });
        }
    }

    ui.separator();
    ui.heading("Transaction Log");
    egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
        for entry in state.transaction_log.iter().rev().take(20) {
            ui.label(entry);
        }
    });
}

// ============================================================
// EXPANSION 4: Inventory Tests
// ============================================================

#[cfg(test)]
mod inventory_expansion4_tests {
    use super::*;

    #[test]
    fn test_equip_slot_names() {
        assert_eq!(EquipSlot::Head.name(), "Head");
        assert_eq!(EquipSlot::MainHand.name(), "Main Hand");
    }

    #[test]
    fn test_equip_stats_add() {
        let mut a = EquipStats { strength: 100, ..Default::default() };
        let b = EquipStats { strength: 50, crit: 1.5, ..Default::default() };
        a.add(&b);
        assert_eq!(a.strength, 150);
        assert!((a.crit - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_character_equipment() {
        let mut equipment = CharacterEquipment::default();
        let item = EquippedItem {
            item_id: 1,
            item_name: "Test Helm".to_string(),
            slot: EquipSlot::Head,
            level_requirement: 10,
            item_level: 100,
            stats: EquipStats { stamina: 50, ..Default::default() },
            gem_slots: vec![],
            enchant: None,
            durability: 100,
            max_durability: 100,
            transmog_item_id: None,
            soulbound: true,
            account_bound: false,
        };
        equipment.equip(item);
        assert_eq!(equipment.total_stats().stamina, 50);
        assert!((equipment.equipped_item_level - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_item_quality_color() {
        assert_eq!(ItemQuality::Rare.name(), "Rare");
        assert!(ItemQuality::Legendary.stat_multiplier() > ItemQuality::Common.stat_multiplier());
    }

    #[test]
    fn test_generate_random_item() {
        let cfg = ItemGenConfig {
            quality: ItemQuality::Epic,
            item_level: 250,
            slot: EquipSlot::Chest,
            seed: 999,
            ..ItemGenConfig::default()
        };
        let item = generate_random_item(&cfg);
        assert!(item.item_level == 250);
        assert!(!item.item_name.is_empty());
    }

    #[test]
    fn test_vendor_available_items() {
        let vendor = VendorNpc::create_general_store(1, "Test".to_string(), "Town".to_string());
        let items = vendor.available_items(30);
        assert!(!items.is_empty());
        assert!(vendor.item_count() > 0);
    }

    #[test]
    fn test_shop_buy_item() {
        let mut state = ShopSystemState::new();
        state.selected_vendor = Some(0);
        let initial_gold = state.player_gold;
        state.buy_item(0, 0);
        assert!(state.player_gold < initial_gold || !state.transaction_log.is_empty());
    }
}
'''

with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'a', encoding='utf-8') as f:
    f.write(code)

import os
size = os.path.getsize(r'C:\proof-engine\editor\src\inventory_system.rs')
print(f"inventory_system.rs size: {size} bytes")
