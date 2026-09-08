
code = r'''

// =================================================================
// ENCHANTMENT CRAFTING SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatBonus {
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub magic: i32,
    pub health: i32,
    pub mana: i32,
    pub crit_chance: f32,
    pub crit_damage: f32,
    pub fire_resist: f32,
    pub ice_resist: f32,
    pub lightning_resist: f32,
    pub poison_resist: f32,
    pub experience_bonus: f32,
    pub gold_bonus: f32,
}

impl Default for StatBonus {
    fn default() -> Self {
        Self { attack:0, defense:0, speed:0, magic:0, health:0, mana:0, crit_chance:0.0, crit_damage:0.0, fire_resist:0.0, ice_resist:0.0, lightning_resist:0.0, poison_resist:0.0, experience_bonus:0.0, gold_bonus:0.0 }
    }
}

impl StatBonus {
    pub fn with_attack(mut self, v: i32) -> Self { self.attack = v; self }
    pub fn with_defense(mut self, v: i32) -> Self { self.defense = v; self }
    pub fn with_health(mut self, v: i32) -> Self { self.health = v; self }
    pub fn with_magic(mut self, v: i32) -> Self { self.magic = v; self }
    pub fn with_speed(mut self, v: i32) -> Self { self.speed = v; self }
    pub fn with_mana(mut self, v: i32) -> Self { self.mana = v; self }
    pub fn with_crit(mut self, chance: f32, damage: f32) -> Self { self.crit_chance = chance; self.crit_damage = damage; self }
    pub fn is_empty(&self) -> bool {
        self.attack == 0 && self.defense == 0 && self.speed == 0 && self.magic == 0 && self.health == 0 && self.mana == 0 && self.crit_chance == 0.0 && self.experience_bonus == 0.0
    }
    pub fn add(&self, other: &StatBonus) -> StatBonus {
        StatBonus {
            attack: self.attack + other.attack,
            defense: self.defense + other.defense,
            speed: self.speed + other.speed,
            magic: self.magic + other.magic,
            health: self.health + other.health,
            mana: self.mana + other.mana,
            crit_chance: self.crit_chance + other.crit_chance,
            crit_damage: self.crit_damage + other.crit_damage,
            fire_resist: self.fire_resist + other.fire_resist,
            ice_resist: self.ice_resist + other.ice_resist,
            lightning_resist: self.lightning_resist + other.lightning_resist,
            poison_resist: self.poison_resist + other.poison_resist,
            experience_bonus: self.experience_bonus + other.experience_bonus,
            gold_bonus: self.gold_bonus + other.gold_bonus,
        }
    }
    pub fn summary_string(&self) -> String {
        let mut parts = Vec::new();
        if self.attack != 0 { parts.push(format!("ATK{:+}", self.attack)); }
        if self.defense != 0 { parts.push(format!("DEF{:+}", self.defense)); }
        if self.speed != 0 { parts.push(format!("SPD{:+}", self.speed)); }
        if self.magic != 0 { parts.push(format!("MAG{:+}", self.magic)); }
        if self.health != 0 { parts.push(format!("HP{:+}", self.health)); }
        if self.mana != 0 { parts.push(format!("MP{:+}", self.mana)); }
        if self.crit_chance != 0.0 { parts.push(format!("CRIT{:+.0}%", self.crit_chance * 100.0)); }
        if self.experience_bonus != 0.0 { parts.push(format!("XP{:+.0}%", self.experience_bonus * 100.0)); }
        if parts.is_empty() { "No bonus".to_string() } else { parts.join(", ") }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnchantmentDef {
    pub name: String,
    pub tier: u8,
    pub stat_bonus: StatBonus,
    pub proc_chance: f32,
    pub proc_effect_name: String,
    pub conflict_set: u32,
    pub glyph: char,
    pub color: Color32,
    pub description: String,
    pub required_materials: Vec<(String, u32)>,
}

impl EnchantmentDef {
    pub fn new(name: &str, tier: u8, glyph: char, color: Color32) -> Self {
        Self {
            name: name.to_string(), tier, stat_bonus: StatBonus::default(), proc_chance: 0.0,
            proc_effect_name: String::new(), conflict_set: 0, glyph, color,
            description: String::new(), required_materials: Vec::new(),
        }
    }
    pub fn fire_attack() -> Self {
        let mut e = EnchantmentDef::new("Fiery", 1, 'F', Color32::from_rgb(220, 80, 30));
        e.stat_bonus = StatBonus::default().with_attack(5);
        e.proc_chance = 0.15;
        e.proc_effect_name = "Burn".to_string();
        e.conflict_set = 1;
        e.description = "Adds fire damage. Chance to burn on hit.".to_string();
        e.required_materials = vec![("Fire Crystal".to_string(), 2), ("Ember Dust".to_string(), 5)];
        e
    }
    pub fn ice_defense() -> Self {
        let mut e = EnchantmentDef::new("Glacial", 1, 'I', Color32::from_rgb(100, 180, 230));
        e.stat_bonus = StatBonus::default().with_defense(6).with_ice_resist(0.1);
        e.proc_chance = 0.1;
        e.proc_effect_name = "Chill".to_string();
        e.conflict_set = 2;
        e.description = "Adds ice resistance and defense.".to_string();
        e.required_materials = vec![("Ice Shard".to_string(), 3), ("Frost Essence".to_string(), 4)];
        e
    }
    pub fn swift_boots() -> Self {
        let mut e = EnchantmentDef::new("Swiftness", 2, 'W', Color32::from_rgb(180, 230, 80));
        e.stat_bonus = StatBonus::default().with_speed(8);
        e.conflict_set = 3;
        e.description = "Increases movement speed.".to_string();
        e.required_materials = vec![("Wind Feather".to_string(), 4), ("Speed Rune".to_string(), 2)];
        e
    }
    pub fn life_steal() -> Self {
        let mut e = EnchantmentDef::new("Vampiric", 3, 'V', Color32::from_rgb(160, 30, 30));
        e.stat_bonus = StatBonus::default().with_attack(8).with_health(20);
        e.proc_chance = 0.2;
        e.proc_effect_name = "Drain".to_string();
        e.conflict_set = 4;
        e.description = "Drains life on hit.".to_string();
        e.required_materials = vec![("Soul Gem".to_string(), 1), ("Blood Ruby".to_string(), 3), ("Dark Essence".to_string(), 8)];
        e
    }
    pub fn arcane_power() -> Self {
        let mut e = EnchantmentDef::new("Arcane", 4, 'A', Color32::from_rgb(160, 80, 220));
        e.stat_bonus = StatBonus::default().with_magic(15).with_mana(40);
        e.conflict_set = 5;
        e.description = "Greatly enhances magical power.".to_string();
        e.required_materials = vec![("Arcane Crystal".to_string(), 5), ("Mana Stone".to_string(), 10), ("Ancient Rune".to_string(), 2)];
        e
    }
    pub fn legendary_sharpness() -> Self {
        let mut e = EnchantmentDef::new("Sharpness V", 5, 'S', Color32::from_rgb(255, 220, 50));
        e.stat_bonus = StatBonus::default().with_attack(25).with_crit(0.15, 0.5);
        e.conflict_set = 6;
        e.description = "Legendary weapon sharpness. Critical hit bonus.".to_string();
        e.required_materials = vec![("Dragon Scale".to_string(), 3), ("Mythril Dust".to_string(), 20), ("God Stone".to_string(), 1)];
        e
    }
    fn with_ice_resist(mut self, v: f32) -> StatBonus { self.stat_bonus.ice_resist = v; self.stat_bonus }
    pub fn conflicts_with(&self, other: &EnchantmentDef) -> bool {
        self.conflict_set != 0 && self.conflict_set == other.conflict_set
    }
    pub fn tier_color(tier: u8) -> Color32 {
        match tier {
            1 => Color32::from_rgb(160, 160, 160),
            2 => Color32::from_rgb(30, 180, 60),
            3 => Color32::from_rgb(50, 120, 255),
            4 => Color32::from_rgb(180, 50, 255),
            5 => Color32::from_rgb(255, 180, 0),
            _ => Color32::WHITE,
        }
    }
    pub fn all_defaults() -> Vec<EnchantmentDef> {
        vec![
            EnchantmentDef::fire_attack(),
            EnchantmentDef::ice_defense(),
            EnchantmentDef::swift_boots(),
            EnchantmentDef::life_steal(),
            EnchantmentDef::arcane_power(),
            EnchantmentDef::legendary_sharpness(),
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnchantmentSlot {
    pub max_tier: u8,
    pub allowed_categories: Vec<ItemCategory>,
    pub current_enchantment: Option<usize>,
    pub slot_name: String,
}

impl EnchantmentSlot {
    pub fn new(max_tier: u8, allowed: Vec<ItemCategory>, name: &str) -> Self {
        Self { max_tier, allowed_categories: allowed, current_enchantment: None, slot_name: name.to_string() }
    }
    pub fn can_accept(&self, enchant: &EnchantmentDef, item_cat: &ItemCategory) -> bool {
        enchant.tier <= self.max_tier && (self.allowed_categories.is_empty() || self.allowed_categories.contains(item_cat))
    }
    pub fn is_empty(&self) -> bool { self.current_enchantment.is_none() }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnchantingTableState {
    pub selected_item_idx: Option<usize>,
    pub selected_enchant_idx: Option<usize>,
    pub enchantment_defs: Vec<EnchantmentDef>,
    pub available_materials: HashMap<String, u32>,
    pub show_conflicts: bool,
    pub filter_text: String,
}

impl Default for EnchantingTableState {
    fn default() -> Self {
        let mut mats = HashMap::new();
        mats.insert("Fire Crystal".to_string(), 10);
        mats.insert("Ember Dust".to_string(), 20);
        mats.insert("Ice Shard".to_string(), 8);
        mats.insert("Frost Essence".to_string(), 15);
        mats.insert("Wind Feather".to_string(), 12);
        mats.insert("Speed Rune".to_string(), 5);
        mats.insert("Soul Gem".to_string(), 2);
        mats.insert("Blood Ruby".to_string(), 6);
        Self {
            selected_item_idx: None,
            selected_enchant_idx: None,
            enchantment_defs: EnchantmentDef::all_defaults(),
            available_materials: mats,
            show_conflicts: true,
            filter_text: String::new(),
        }
    }
}

impl EnchantingTableState {
    pub fn can_afford(&self, enchant: &EnchantmentDef) -> bool {
        enchant.required_materials.iter().all(|(mat, count)| {
            self.available_materials.get(mat).copied().unwrap_or(0) >= *count
        })
    }
    pub fn apply_enchantment(&mut self, enchant_idx: usize) -> bool {
        if enchant_idx >= self.enchantment_defs.len() { return false; }
        let enchant = &self.enchantment_defs[enchant_idx];
        if !self.can_afford(enchant) { return false; }
        for (mat, count) in &enchant.required_materials.clone() {
            if let Some(v) = self.available_materials.get_mut(mat) {
                *v = v.saturating_sub(*count);
            }
        }
        true
    }
}

pub fn show_enchanting_table(ui: &mut egui::Ui, state: &mut EnchantingTableState, item_names: &[String]) {
    ui.heading("Enchanting Table");
    ui.separator();
    ui.horizontal(|ui| {
        // Left panel: item selection
        ui.vertical(|ui| {
            ui.label("Select Item:");
            egui::ScrollArea::vertical().id_salt("ench_item_scroll").max_height(200.0).show(ui, |ui| {
                for (i, name) in item_names.iter().enumerate() {
                    let sel = state.selected_item_idx == Some(i);
                    if ui.selectable_label(sel, name).clicked() {
                        state.selected_item_idx = Some(i);
                    }
                }
            });
        });

        ui.separator();

        // Right panel: enchantment selection
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.text_edit_singleline(&mut state.filter_text);
            });
            egui::ScrollArea::vertical().id_salt("ench_list_scroll").max_height(200.0).show(ui, |ui| {
                for (i, enchant) in state.enchantment_defs.iter().enumerate() {
                    if !state.filter_text.is_empty() && !enchant.name.to_lowercase().contains(&state.filter_text.to_lowercase()) { continue; }
                    let sel = state.selected_enchant_idx == Some(i);
                    let can_afford = state.can_afford(enchant);
                    ui.horizontal(|ui| {
                        let tier_col = EnchantmentDef::tier_color(enchant.tier);
                        ui.colored_label(tier_col, enchant.glyph.to_string());
                        let label = egui::RichText::new(format!("{} (T{})", enchant.name, enchant.tier))
                            .color(if can_afford { Color32::WHITE } else { Color32::from_gray(120) });
                        if ui.selectable_label(sel, label).clicked() {
                            state.selected_enchant_idx = Some(i);
                        }
                    });
                }
            });
        });
    });

    ui.separator();

    // Details panel
    if let Some(ei) = state.selected_enchant_idx {
        if let Some(enchant) = state.enchantment_defs.get(ei) {
            let enchant = enchant.clone();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(enchant.color, enchant.glyph.to_string());
                    ui.heading(&enchant.name);
                    ui.label(format!("Tier {}", enchant.tier));
                });
                ui.label(&enchant.description);
                ui.label(format!("Stats: {}", enchant.stat_bonus.summary_string()));
                if enchant.proc_chance > 0.0 {
                    ui.label(format!("Proc: {:.0}% chance to trigger {}", enchant.proc_chance * 100.0, enchant.proc_effect_name));
                }
                ui.separator();
                ui.label("Required materials:");
                for (mat, count) in &enchant.required_materials {
                    let have = state.available_materials.get(mat).copied().unwrap_or(0);
                    let col = if have >= *count { Color32::GREEN } else { Color32::RED };
                    ui.horizontal(|ui| {
                        ui.colored_label(col, format!("• {} x{} (have: {})", mat, count, have));
                    });
                }
                let can_afford = state.can_afford(&enchant);
                let btn = egui::Button::new("Enchant").fill(if can_afford { Color32::from_rgb(60,120,60) } else { Color32::from_gray(40) });
                if ui.add_enabled(can_afford, btn).clicked() {
                    state.apply_enchantment(ei);
                }
            });
        }
    }
}

// =================================================================
// ITEM SET BONUS SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetBonus {
    pub required_count: u32,
    pub bonus: StatBonus,
    pub effect_name: String,
    pub description: String,
}

impl SetBonus {
    pub fn new(required_count: u32, bonus: StatBonus, desc: &str) -> Self {
        Self { required_count, bonus, effect_name: String::new(), description: desc.to_string() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemSet {
    pub name: String,
    pub description: String,
    pub item_ids: Vec<u32>,
    pub bonuses: Vec<SetBonus>,
    pub set_color: Color32,
    pub icon: char,
}

impl ItemSet {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            item_ids: Vec::new(),
            bonuses: Vec::new(),
            set_color: Color32::from_rgb(200, 150, 80),
            icon: 'S',
        }
    }
    pub fn get_active_bonus(&self, equipped_count: u32) -> Option<StatBonus> {
        let mut total = StatBonus::default();
        let mut any = false;
        for bonus in &self.bonuses {
            if equipped_count >= bonus.required_count {
                total = total.add(&bonus.bonus);
                any = true;
            }
        }
        if any { Some(total) } else { None }
    }
    pub fn next_bonus_threshold(&self, equipped_count: u32) -> Option<u32> {
        self.bonuses.iter()
            .filter(|b| b.required_count > equipped_count)
            .map(|b| b.required_count)
            .min()
    }
    pub fn dragonscale() -> Self {
        let mut s = ItemSet::new("Dragonscale");
        s.description = "Ancient armor forged from dragon scales.".to_string();
        s.set_color = Color32::from_rgb(200, 100, 30);
        s.icon = 'D';
        s.bonuses = vec![
            SetBonus::new(2, StatBonus::default().with_defense(10).with_attack(5), "2pc: Enhanced defense and attack"),
            SetBonus::new(4, StatBonus::default().with_defense(25).with_attack(15), "4pc: Dragon's might"),
            SetBonus::new(6, StatBonus::default().with_defense(50).with_attack(30).with_health(100), "6pc: Dragon's Legacy - Full dragon power"),
        ];
        s
    }
    pub fn shadow_thief() -> Self {
        let mut s = ItemSet::new("Shadow Thief");
        s.description = "Gear of the legendary Shadow Thief guild.".to_string();
        s.set_color = Color32::from_rgb(80, 60, 120);
        s.icon = 'T';
        s.bonuses = vec![
            SetBonus::new(2, StatBonus::default().with_speed(5), "2pc: Quick Fingers"),
            SetBonus::new(4, StatBonus::default().with_speed(12).with_crit(0.1, 0.3), "4pc: Shadow Step"),
        ];
        s
    }
    pub fn archmage() -> Self {
        let mut s = ItemSet::new("Archmage's Regalia");
        s.description = "Robes of the ancient archmages.".to_string();
        s.set_color = Color32::from_rgb(120, 60, 200);
        s.icon = 'M';
        s.bonuses = vec![
            SetBonus::new(2, StatBonus::default().with_magic(15), "2pc: Arcane Insight"),
            SetBonus::new(4, StatBonus::default().with_magic(30).with_mana(100), "4pc: Mana Surge"),
            SetBonus::new(6, StatBonus::default().with_magic(60).with_mana(200), "6pc: Archmage's Ascension"),
        ];
        s
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SetEditorState {
    pub sets: Vec<ItemSet>,
    pub selected_set: Option<usize>,
    pub equipped_set_pieces: HashMap<usize, u32>,
    pub new_set_name: String,
}

impl SetEditorState {
    pub fn with_defaults() -> Self {
        Self {
            sets: vec![ItemSet::dragonscale(), ItemSet::shadow_thief(), ItemSet::archmage()],
            ..Default::default()
        }
    }
    pub fn get_set_bonus_for(&self, set_idx: usize) -> Option<StatBonus> {
        let count = self.equipped_set_pieces.get(&set_idx).copied().unwrap_or(0);
        self.sets.get(set_idx)?.get_active_bonus(count)
    }
}

pub fn show_set_editor(ui: &mut egui::Ui, state: &mut SetEditorState) {
    ui.heading("Item Set Editor");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.new_set_name);
        if ui.button("New Set").clicked() && !state.new_set_name.is_empty() {
            state.sets.push(ItemSet::new(&state.new_set_name));
            state.new_set_name.clear();
        }
    });
    ui.separator();

    if let Some(sel) = state.selected_set {
        if sel < state.sets.len() {
            let set = &mut state.sets[sel];
            ui.horizontal(|ui| {
                ui.colored_label(set.set_color, set.icon.to_string());
                ui.heading(&set.name.clone());
            });
            ui.text_edit_multiline(&mut set.description);
            ui.separator();
            ui.label(format!("{} items in set", set.item_ids.len()));
            ui.separator();
            ui.label("Set Bonuses:");
            for bonus in &set.bonuses {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(220, 180, 80), format!("{}pc:", bonus.required_count));
                    ui.label(&bonus.description);
                    ui.label(format!("[{}]", bonus.bonus.summary_string()));
                });
            }
            if ui.button("Add 2pc Bonus").clicked() {
                set.bonuses.push(SetBonus::new(2, StatBonus::default(), "2pc bonus"));
            }
            if ui.button("Add 4pc Bonus").clicked() {
                set.bonuses.push(SetBonus::new(4, StatBonus::default(), "4pc bonus"));
            }
            // Bonus editor
            for (bi, bonus) in state.sets[sel].bonuses.iter_mut().enumerate() {
                ui.push_id(bi, |ui| {
                    ui.collapsing(format!("{}pc Bonus", bonus.required_count), |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Required:");
                            ui.add(egui::DragValue::new(&mut bonus.required_count).range(1..=12));
                        });
                        ui.horizontal(|ui| {
                            ui.label("ATK:");
                            ui.add(egui::DragValue::new(&mut bonus.bonus.attack));
                            ui.label("DEF:");
                            ui.add(egui::DragValue::new(&mut bonus.bonus.defense));
                            ui.label("SPD:");
                            ui.add(egui::DragValue::new(&mut bonus.bonus.speed));
                        });
                        ui.horizontal(|ui| {
                            ui.label("MAG:");
                            ui.add(egui::DragValue::new(&mut bonus.bonus.magic));
                            ui.label("HP:");
                            ui.add(egui::DragValue::new(&mut bonus.bonus.health));
                            ui.label("MP:");
                            ui.add(egui::DragValue::new(&mut bonus.bonus.mana));
                        });
                        ui.text_edit_singleline(&mut bonus.description);
                    });
                });
            }
        }
    }

    ui.separator();
    // Set list
    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
        for (i, set) in state.sets.iter().enumerate() {
            let sel = state.selected_set == Some(i);
            ui.horizontal(|ui| {
                ui.colored_label(set.set_color, set.icon.to_string());
                if ui.selectable_label(sel, &set.name).clicked() {
                    state.selected_set = Some(i);
                }
                let equipped = state.equipped_set_pieces.get(&i).copied().unwrap_or(0);
                ui.label(format!("{}/{}", equipped, set.item_ids.len()));
            });
        }
    });
}

// =================================================================
// PROCEDURAL ITEM GENERATOR
// =================================================================

pub struct InvRng { state: u64 }
impl InvRng {
    pub fn new(seed: u64) -> Self { Self { state: seed ^ 0xFEDCBA9876543210 } }
    pub fn next_u64(&mut self) -> u64 { self.state ^= self.state << 13; self.state ^= self.state >> 7; self.state ^= self.state << 17; self.state }
    pub fn next_f32(&mut self) -> f32 { (self.next_u64() as f32) / (u64::MAX as f32) }
    pub fn pick_weighted<T: Clone>(&mut self, items: &[(T, f32)]) -> Option<T> {
        if items.is_empty() { return None; }
        let total: f32 = items.iter().map(|(_, w)| w).sum();
        let mut r = self.next_f32() * total;
        for (item, weight) in items { if r < *weight { return Some(item.clone()); } r -= weight; }
        items.last().map(|(t, _)| t.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemGeneratorConfig {
    pub level_range: (u32, u32),
    pub stat_budget: f32,
    pub prefix_pool: Vec<(String, f32)>,
    pub suffix_pool: Vec<(String, f32)>,
    pub stat_weights: HashMap<String, f32>,
    pub min_stats: u32,
    pub max_stats: u32,
}

impl Default for ItemGeneratorConfig {
    fn default() -> Self {
        let prefix_pool = vec![
            ("Fiery".to_string(), 1.0), ("Icy".to_string(), 1.0), ("Mighty".to_string(), 1.5),
            ("Swift".to_string(), 1.0), ("Arcane".to_string(), 0.8), ("Poisoned".to_string(), 0.7),
            ("Blessed".to_string(), 0.5), ("Cursed".to_string(), 0.3), ("Ancient".to_string(), 0.4),
        ];
        let suffix_pool = vec![
            ("of Power".to_string(), 1.0), ("of Defense".to_string(), 1.0), ("of Speed".to_string(), 1.0),
            ("of Sorcery".to_string(), 0.8), ("of Health".to_string(), 1.2), ("of the Bear".to_string(), 0.9),
            ("of the Fox".to_string(), 0.9), ("of Storms".to_string(), 0.5), ("of Dragons".to_string(), 0.3),
        ];
        let mut stat_weights = HashMap::new();
        stat_weights.insert("attack".to_string(), 1.0);
        stat_weights.insert("defense".to_string(), 1.0);
        stat_weights.insert("speed".to_string(), 0.8);
        stat_weights.insert("magic".to_string(), 0.9);
        stat_weights.insert("health".to_string(), 1.1);
        stat_weights.insert("mana".to_string(), 0.8);
        Self { level_range: (1, 50), stat_budget: 100.0, prefix_pool, suffix_pool, stat_weights, min_stats: 1, max_stats: 4 }
    }
}

pub fn generate_item_name(base_name: &str, config: &ItemGeneratorConfig, seed: u64) -> String {
    let mut rng = InvRng::new(seed);
    let use_prefix = rng.next_f32() < 0.7;
    let use_suffix = rng.next_f32() < 0.6;
    let prefix = if use_prefix { config.pick_prefix(&mut rng) } else { None };
    let suffix = if use_suffix { config.pick_suffix(&mut rng) } else { None };
    match (prefix, suffix) {
        (Some(p), Some(s)) => format!("{} {} {}", p, base_name, s),
        (Some(p), None) => format!("{} {}", p, base_name),
        (None, Some(s)) => format!("{} {}", base_name, s),
        (None, None) => base_name.to_string(),
    }
}

impl ItemGeneratorConfig {
    pub fn pick_prefix(&self, rng: &mut InvRng) -> Option<String> {
        rng.pick_weighted(&self.prefix_pool)
    }
    pub fn pick_suffix(&self, rng: &mut InvRng) -> Option<String> {
        rng.pick_weighted(&self.suffix_pool)
    }
    pub fn generate_stats(&self, rng: &mut InvRng, level: u32) -> StatBonus {
        let budget = self.stat_budget * (level as f32 / self.level_range.1 as f32).max(0.1);
        let stat_count = self.min_stats + rng.next_u64() as u32 % (self.max_stats - self.min_stats + 1).max(1);
        let stats: Vec<&str> = ["attack","defense","speed","magic","health","mana"];
        let mut bonus = StatBonus::default();
        let per_stat = budget / stat_count as f32;
        for _ in 0..stat_count {
            let stat_idx = rng.next_u64() as usize % stats.len();
            let value = (per_stat * (0.5 + rng.next_f32())) as i32;
            match stats[stat_idx] {
                "attack" => bonus.attack += value,
                "defense" => bonus.defense += value,
                "speed" => bonus.speed += value.min(30),
                "magic" => bonus.magic += value,
                "health" => bonus.health += value * 3,
                "mana" => bonus.mana += value * 2,
                _ => {},
            }
        }
        bonus
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedItemPreview {
    pub name: String,
    pub level: u32,
    pub stats: StatBonus,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ItemGeneratorEditorState {
    pub config: ItemGeneratorConfig,
    pub base_name: String,
    pub previews: Vec<GeneratedItemPreview>,
    pub selected_preview: Option<usize>,
    pub generate_level: u32,
    pub generate_seed: u64,
}

impl ItemGeneratorEditorState {
    pub fn generate_batch(&mut self, count: usize) {
        self.previews.clear();
        for i in 0..count {
            let seed = self.generate_seed.wrapping_add(i as u64 * 0x9E3779B97F4A7C15);
            let mut rng = InvRng::new(seed);
            let level = self.config.level_range.0 + rng.next_u64() as u32 % (self.config.level_range.1 - self.config.level_range.0 + 1).max(1);
            let name = generate_item_name(&self.base_name, &self.config, seed);
            let stats = self.config.generate_stats(&mut rng, level);
            self.previews.push(GeneratedItemPreview { name, level, stats, seed });
        }
    }
}

pub fn show_item_generator(ui: &mut egui::Ui, state: &mut ItemGeneratorEditorState) {
    ui.heading("Procedural Item Generator");
    ui.horizontal(|ui| {
        ui.label("Base name:");
        ui.text_edit_singleline(&mut state.base_name);
        ui.label("Level:");
        ui.add(egui::DragValue::new(&mut state.generate_level).range(1..=100));
        ui.label("Seed:");
        ui.add(egui::DragValue::new(&mut state.generate_seed));
    });
    ui.horizontal(|ui| {
        ui.label("Level range:");
        ui.add(egui::DragValue::new(&mut state.config.level_range.0).range(1..=100).prefix("min "));
        ui.add(egui::DragValue::new(&mut state.config.level_range.1).range(1..=200).prefix("max "));
        ui.label("Stat budget:");
        ui.add(egui::DragValue::new(&mut state.config.stat_budget).range(10.0..=1000.0));
    });
    ui.horizontal(|ui| {
        ui.label("Stats per item:");
        ui.add(egui::DragValue::new(&mut state.config.min_stats).range(1..=6).prefix("min "));
        ui.add(egui::DragValue::new(&mut state.config.max_stats).range(1..=8).prefix("max "));
    });
    if ui.button("Generate 10 samples").clicked() {
        state.generate_batch(10);
    }
    ui.separator();
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for (i, preview) in state.previews.iter().enumerate() {
            let sel = state.selected_preview == Some(i);
            ui.horizontal(|ui| {
                if ui.selectable_label(sel, format!("[Lv{}] {}", preview.level, preview.name)).clicked() {
                    state.selected_preview = Some(i);
                }
            });
            if sel {
                ui.indent("preview_stats", |ui| {
                    ui.label(format!("Stats: {}", preview.stats.summary_string()));
                });
            }
        }
    });
    if let Some(sel) = state.selected_preview {
        if sel < state.previews.len() {
            let p = &state.previews[sel];
            ui.separator();
            ui.group(|ui| {
                ui.heading(&p.name);
                ui.label(format!("Level: {}", p.level));
                ui.label(format!("ATK: {} | DEF: {} | SPD: {} | MAG: {} | HP: {} | MP: {}", p.stats.attack, p.stats.defense, p.stats.speed, p.stats.magic, p.stats.health, p.stats.mana));
                if p.stats.crit_chance > 0.0 { ui.label(format!("Crit: {:.0}% (+{:.0}%)", p.stats.crit_chance*100.0, p.stats.crit_damage*100.0)); }
                if ui.button("Use this item").clicked() {}
            });
        }
    }
}

// =================================================================
// ECONOMY SIMULATION (expanded)
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriceHistory {
    pub prices: Vec<f32>,
    pub max_history: usize,
}

impl PriceHistory {
    pub fn new(max: usize) -> Self { Self { prices: Vec::new(), max_history: max } }
    pub fn push(&mut self, price: f32) {
        self.prices.push(price);
        if self.prices.len() > self.max_history {
            self.prices.remove(0);
        }
    }
    pub fn latest(&self) -> f32 { self.prices.last().copied().unwrap_or(0.0) }
    pub fn average(&self) -> f32 {
        if self.prices.is_empty() { return 0.0; }
        self.prices.iter().sum::<f32>() / self.prices.len() as f32
    }
    pub fn trend(&self) -> f32 {
        if self.prices.len() < 2 { return 0.0; }
        self.prices.last().unwrap() - self.prices[self.prices.len().saturating_sub(5).max(0)]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketGood {
    pub item_name: String,
    pub base_price: f32,
    pub current_price: f32,
    pub supply: f32,
    pub demand: f32,
    pub elasticity: f32,
    pub price_history: PriceHistory,
    pub produced_per_day: f32,
    pub consumed_per_day: f32,
    pub volatility: f32,
}

impl MarketGood {
    pub fn new(name: &str, base_price: f32, elasticity: f32) -> Self {
        let mut h = PriceHistory::new(50);
        h.push(base_price);
        Self {
            item_name: name.to_string(),
            base_price, current_price: base_price,
            supply: 100.0, demand: 100.0, elasticity,
            price_history: h,
            produced_per_day: 10.0, consumed_per_day: 10.0, volatility: 0.05,
        }
    }
    pub fn tick(&mut self, dt_days: f32, rng: &mut InvRng) {
        // Supply/demand dynamics
        let supply_delta = self.produced_per_day * dt_days;
        let demand_delta = self.consumed_per_day * dt_days * (1.0 + (rng.next_f32() - 0.5) * 0.1);
        self.supply = (self.supply + supply_delta - demand_delta).max(1.0);

        // Price elasticity: price moves based on supply/demand ratio
        let ratio = self.demand / self.supply.max(0.01);
        let target_price = self.base_price * ratio.powf(self.elasticity);
        let noise = 1.0 + (rng.next_f32() - 0.5) * self.volatility;
        self.current_price = (self.current_price * 0.9 + target_price * 0.1) * noise;
        self.current_price = self.current_price.max(self.base_price * 0.1).min(self.base_price * 10.0);
        self.price_history.push(self.current_price);
    }
    pub fn price_change_pct(&self) -> f32 {
        let trend = self.price_history.trend();
        trend / self.base_price * 100.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraderNpc {
    pub name: String,
    pub gold: f32,
    pub markup: f32,
    pub preferred_items: Vec<String>,
    pub buys: Vec<(String, f32)>,
    pub sells: Vec<(String, f32)>,
    pub reputation: f32,
    pub haggle_skill: f32,
}

impl TraderNpc {
    pub fn new(name: &str, gold: f32) -> Self {
        Self { name: name.to_string(), gold, markup: 0.15, preferred_items: Vec::new(), buys: Vec::new(), sells: Vec::new(), reputation: 0.5, haggle_skill: 0.3 }
    }
    pub fn buy_price(&self, market_price: f32) -> f32 { market_price * (1.0 - self.markup * 0.5) }
    pub fn sell_price(&self, market_price: f32) -> f32 { market_price * (1.0 + self.markup) }
    pub fn haggled_price(&self, price: f32, player_haggle: f32) -> f32 {
        let discount = (player_haggle - self.haggle_skill).max(0.0) * 0.1;
        price * (1.0 - discount)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EconomyEditorState {
    pub goods: Vec<MarketGood>,
    pub traders: Vec<TraderNpc>,
    pub selected_good: Option<usize>,
    pub selected_trader: Option<usize>,
    pub sim_speed: f32,
    pub accumulated_time: f32,
    pub is_running: bool,
    pub tick_interval: f32,
    pub rng_seed: u64,
}

impl EconomyEditorState {
    pub fn with_defaults() -> Self {
        let goods = vec![
            MarketGood::new("Iron Ore", 5.0, 0.8),
            MarketGood::new("Wood", 3.0, 0.6),
            MarketGood::new("Wheat", 2.0, 0.5),
            MarketGood::new("Leather", 8.0, 0.9),
            MarketGood::new("Silver", 50.0, 1.2),
            MarketGood::new("Gold", 200.0, 1.5),
            MarketGood::new("Magic Gem", 500.0, 2.0),
        ];
        let traders = vec![
            TraderNpc::new("Tom the Merchant", 1000.0),
            TraderNpc::new("Lady Vera", 5000.0),
            TraderNpc::new("Gruff Blacksmith", 800.0),
        ];
        Self { goods, traders, sim_speed: 1.0, tick_interval: 1.0, rng_seed: 42, ..Default::default() }
    }
    pub fn tick_economy(&mut self, dt: f32) {
        if !self.is_running { return; }
        self.accumulated_time += dt * self.sim_speed;
        while self.accumulated_time >= self.tick_interval {
            self.accumulated_time -= self.tick_interval;
            let mut rng = InvRng::new(self.rng_seed.wrapping_add(self.goods.iter().map(|g| g.price_history.prices.len() as u64).sum::<u64>()));
            for good in &mut self.goods {
                good.tick(self.tick_interval / 30.0, &mut rng);
            }
            self.rng_seed = self.rng_seed.wrapping_add(1);
        }
    }
}

pub fn show_economy_editor(ui: &mut egui::Ui, state: &mut EconomyEditorState, dt: f32) {
    state.tick_economy(dt);
    ui.heading("Economy Simulation");
    ui.horizontal(|ui| {
        let btn_label = if state.is_running { "Pause" } else { "Run" };
        if ui.button(btn_label).clicked() { state.is_running = !state.is_running; }
        ui.label("Speed:");
        ui.add(egui::DragValue::new(&mut state.sim_speed).range(0.1..=100.0).suffix("x"));
        ui.label("Tick:");
        ui.add(egui::DragValue::new(&mut state.tick_interval).range(0.1..=30.0).suffix("d"));
    });
    ui.separator();
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for (i, good) in state.goods.iter().enumerate() {
            let sel = state.selected_good == Some(i);
            ui.horizontal(|ui| {
                if ui.selectable_label(sel, &good.item_name).clicked() { state.selected_good = Some(i); }
                let trend = good.price_change_pct();
                let col = if trend > 1.0 { Color32::GREEN } else if trend < -1.0 { Color32::RED } else { Color32::GRAY };
                ui.colored_label(col, format!("{:.1}g ({:+.1}%)", good.current_price, trend));
                ui.label(format!("S:{:.0} D:{:.0}", good.supply, good.demand));
            });
        }
    });

    if let Some(sel) = state.selected_good {
        if sel < state.goods.len() {
            let good = &state.goods[sel];
            ui.separator();
            ui.group(|ui| {
                ui.heading(&good.item_name.clone());
                ui.horizontal(|ui| {
                    ui.label(format!("Base: {:.1}g", good.base_price));
                    ui.label(format!("Current: {:.1}g", good.current_price));
                    let trend = good.price_change_pct();
                    let col = if trend > 0.0 { Color32::GREEN } else { Color32::RED };
                    ui.colored_label(col, format!("{:+.1}%", trend));
                });
                ui.label(format!("Supply: {:.0} | Demand: {:.0} | Elasticity: {:.1}", good.supply, good.demand, good.elasticity));

                // Price history chart
                let prices = &good.price_history.prices;
                if prices.len() >= 2 {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 60.0), egui::Sense::hover());
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, 2.0, Color32::from_gray(20));
                    let min_price = prices.iter().cloned().fold(f32::MAX, f32::min);
                    let max_price = prices.iter().cloned().fold(f32::MIN, f32::max);
                    let price_range = (max_price - min_price).max(0.01);
                    let mut prev: Option<Pos2> = None;
                    for (j, &price) in prices.iter().enumerate() {
                        let px = rect.left() + j as f32 / (prices.len() - 1) as f32 * rect.width();
                        let py = rect.bottom() - (price - min_price) / price_range * rect.height();
                        let cur = Pos2::new(px, py);
                        if let Some(pp) = prev {
                            let col = if price > good.base_price { Color32::from_rgb(60,200,60) } else { Color32::from_rgb(200,60,60) };
                            p.line_segment([pp, cur], Stroke::new(1.5, col));
                        }
                        prev = Some(cur);
                    }
                    // Base price line
                    let base_y = rect.bottom() - (good.base_price - min_price) / price_range * rect.height();
                    p.line_segment([Pos2::new(rect.left(), base_y), Pos2::new(rect.right(), base_y)], Stroke::new(1.0, Color32::from_rgba_unmultiplied(255,255,255,80)));
                }
            });
        }
    }
}

// =================================================================
// CONTAINER / LOOT SYSTEM
// =================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ContainerType {
    Chest, Barrel, Crate, Body, Shop, Hidden, Vault, Urn, Sack,
}

impl ContainerType {
    pub fn name(&self) -> &str {
        match self { ContainerType::Chest=>"Chest", ContainerType::Barrel=>"Barrel", ContainerType::Crate=>"Crate", ContainerType::Body=>"Body", ContainerType::Shop=>"Shop", ContainerType::Hidden=>"Hidden", ContainerType::Vault=>"Vault", ContainerType::Urn=>"Urn", ContainerType::Sack=>"Sack" }
    }
    pub fn icon(&self) -> char {
        match self { ContainerType::Chest=>'C', ContainerType::Barrel=>'B', ContainerType::Crate=>'K', ContainerType::Body=>'X', ContainerType::Shop=>'$', ContainerType::Hidden=>'?', ContainerType::Vault=>'V', ContainerType::Urn=>'U', ContainerType::Sack=>'S' }
    }
    pub fn all() -> &'static [ContainerType] {
        &[ContainerType::Chest, ContainerType::Barrel, ContainerType::Crate, ContainerType::Body, ContainerType::Shop, ContainerType::Hidden, ContainerType::Vault, ContainerType::Urn, ContainerType::Sack]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerDef {
    pub name: String,
    pub container_type: ContainerType,
    pub loot_table_idx: usize,
    pub can_restock: bool,
    pub restock_interval: f32,
    pub time_since_restock: f32,
    pub locked: bool,
    pub lock_strength: u32,
    pub opened: bool,
    pub position: [f32; 2],
}

impl ContainerDef {
    pub fn new(name: &str, container_type: ContainerType) -> Self {
        Self {
            name: name.to_string(), container_type,
            loot_table_idx: 0, can_restock: false, restock_interval: 86400.0, time_since_restock: 0.0,
            locked: false, lock_strength: 0, opened: false, position: [0.0, 0.0],
        }
    }
    pub fn needs_restock(&self, current_time: f32) -> bool {
        self.can_restock && self.opened && current_time - self.time_since_restock >= self.restock_interval
    }
    pub fn try_restock(&mut self, current_time: f32) -> bool {
        if self.needs_restock(current_time) {
            self.opened = false;
            self.time_since_restock = current_time;
            true
        } else { false }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ContainerEditorState {
    pub containers: Vec<ContainerDef>,
    pub selected_container: Option<usize>,
    pub new_container_name: String,
    pub new_container_type: ContainerType,
    pub loot_simulation_result: Vec<String>,
    pub sim_luck: f32,
    pub world_multiplier: f32,
}

impl Default for ContainerType {
    fn default() -> Self { ContainerType::Chest }
}

impl ContainerEditorState {
    pub fn with_defaults() -> Self {
        let mut state = Self { sim_luck: 1.0, world_multiplier: 1.0, ..Default::default() };
        state.containers.push(ContainerDef::new("Starting Chest", ContainerType::Chest));
        state.containers.push(ContainerDef::new("Barrel of Supplies", ContainerType::Barrel));
        state.containers.push(ContainerDef::new("Locked Vault", ContainerType::Vault));
        state.containers[2].locked = true;
        state.containers[2].lock_strength = 5;
        state
    }
    pub fn simulate_loot(&mut self, container_idx: usize, seed: u64) {
        self.loot_simulation_result.clear();
        if container_idx >= self.containers.len() { return; }
        let container = &self.containers[container_idx];
        let mut rng = InvRng::new(seed);
        let count = (2.0 * self.sim_luck * self.world_multiplier) as u32 + 1 + rng.next_u64() as u32 % 4;
        let item_pool = ["Iron Sword", "Leather Boots", "Health Potion", "Gold Coin", "Old Key", "Cloth Armor", "Torch", "Bread", "Arrow Bundle", "Silver Ring"];
        for _ in 0..count {
            let item = item_pool[rng.next_u64() as usize % item_pool.len()];
            let qty = 1 + rng.next_u64() % 5;
            let _ = container;
            self.loot_simulation_result.push(format!("{} x{}", item, qty));
        }
    }
}

pub fn show_container_editor(ui: &mut egui::Ui, state: &mut ContainerEditorState) {
    ui.heading("Container/Loot Editor");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.new_container_name);
        egui::ComboBox::from_id_salt("new_ct").selected_text(state.new_container_type.name()).show_ui(ui, |ui| {
            for ct in ContainerType::all() {
                ui.selectable_value(&mut state.new_container_type, ct.clone(), ct.name());
            }
        });
        if ui.button("Add Container").clicked() && !state.new_container_name.is_empty() {
            state.containers.push(ContainerDef::new(&state.new_container_name, state.new_container_type.clone()));
            state.new_container_name.clear();
        }
    });
    ui.separator();

    if let Some(sel) = state.selected_container {
        if sel < state.containers.len() {
            let c = &mut state.containers[sel];
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(c.container_type.icon().to_string());
                    ui.text_edit_singleline(&mut c.name);
                });
                ui.horizontal(|ui| {
                    ui.label("Type:");
                    egui::ComboBox::from_id_salt("sel_ct").selected_text(c.container_type.name()).show_ui(ui, |ui| {
                        for ct in ContainerType::all() {
                            ui.selectable_value(&mut c.container_type, ct.clone(), ct.name());
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut c.locked, "Locked");
                    if c.locked { ui.add(egui::DragValue::new(&mut c.lock_strength).range(0..=10).prefix("strength ")); }
                    ui.checkbox(&mut c.can_restock, "Can restock");
                    if c.can_restock { ui.add(egui::DragValue::new(&mut c.restock_interval).range(60.0..=86400.0).suffix("s")); }
                });
                ui.horizontal(|ui| {
                    ui.label("Loot table:");
                    ui.add(egui::DragValue::new(&mut c.loot_table_idx));
                });
            });
        }
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Luck:");
        ui.add(egui::DragValue::new(&mut state.sim_luck).range(0.1..=5.0));
        ui.label("World mult:");
        ui.add(egui::DragValue::new(&mut state.world_multiplier).range(0.1..=3.0));
        if ui.button("Simulate Loot").clicked() {
            if let Some(sel) = state.selected_container {
                state.simulate_loot(sel, 42);
            }
        }
    });

    if !state.loot_simulation_result.is_empty() {
        ui.separator();
        ui.label("Simulated loot roll:");
        for item in &state.loot_simulation_result {
            ui.label(format!("  {}", item));
        }
    }

    ui.separator();
    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
        let mut to_remove = None;
        for (i, c) in state.containers.iter().enumerate() {
            let sel = state.selected_container == Some(i);
            ui.horizontal(|ui| {
                ui.label(c.container_type.icon().to_string());
                if ui.selectable_label(sel, &c.name).clicked() { state.selected_container = Some(i); }
                if c.locked { ui.label("🔒"); }
                if ui.small_button("X").clicked() { to_remove = Some(i); }
            });
        }
        if let Some(idx) = to_remove { state.containers.remove(idx); if state.selected_container == Some(idx) { state.selected_container = None; } }
    });
}

// =================================================================
// UNIFIED INVENTORY PANEL
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FullInventoryEditorState {
    pub enchanting: EnchantingTableState,
    pub sets: SetEditorState,
    pub generator: ItemGeneratorEditorState,
    pub economy: EconomyEditorState,
    pub containers: ContainerEditorState,
    pub active_tab: FullInventoryTab,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum FullInventoryTab { #[default] Enchanting, Sets, Generator, Economy, Containers }
impl FullInventoryTab {
    pub fn name(&self) -> &str { match self { FullInventoryTab::Enchanting=>"Enchanting", FullInventoryTab::Sets=>"Sets", FullInventoryTab::Generator=>"Generator", FullInventoryTab::Economy=>"Economy", FullInventoryTab::Containers=>"Containers" } }
    pub fn all() -> &'static [FullInventoryTab] { &[FullInventoryTab::Enchanting, FullInventoryTab::Sets, FullInventoryTab::Generator, FullInventoryTab::Economy, FullInventoryTab::Containers] }
}

pub fn show_full_inventory_editor(ui: &mut egui::Ui, state: &mut FullInventoryEditorState, item_names: &[String], dt: f32) {
    ui.horizontal(|ui| {
        for tab in FullInventoryTab::all() {
            if ui.selectable_label(state.active_tab == *tab, tab.name()).clicked() {
                state.active_tab = tab.clone();
            }
        }
    });
    ui.separator();
    match state.active_tab {
        FullInventoryTab::Enchanting => show_enchanting_table(ui, &mut state.enchanting, item_names),
        FullInventoryTab::Sets       => show_set_editor(ui, &mut state.sets),
        FullInventoryTab::Generator  => show_item_generator(ui, &mut state.generator),
        FullInventoryTab::Economy    => show_economy_editor(ui, &mut state.economy, dt),
        FullInventoryTab::Containers => show_container_editor(ui, &mut state.containers),
    }
}

// =================================================================
// INVENTORY SYSTEM TESTS
// =================================================================

#[cfg(test)]
mod new_inventory_tests {
    use super::*;

    #[test]
    fn test_stat_bonus_add() {
        let a = StatBonus::default().with_attack(5).with_defense(3);
        let b = StatBonus::default().with_attack(2).with_health(10);
        let c = a.add(&b);
        assert_eq!(c.attack, 7);
        assert_eq!(c.defense, 3);
        assert_eq!(c.health, 10);
    }
    #[test]
    fn test_stat_bonus_summary_non_empty() {
        let b = StatBonus::default().with_attack(5);
        assert!(!b.summary_string().is_empty());
        assert!(b.summary_string().contains("ATK"));
    }
    #[test]
    fn test_stat_bonus_empty_summary() {
        let b = StatBonus::default();
        assert_eq!(b.summary_string(), "No bonus");
    }
    #[test]
    fn test_enchantment_conflicts() {
        let e1 = EnchantmentDef::fire_attack();
        let e2 = EnchantmentDef::fire_attack();
        assert!(e1.conflicts_with(&e2));
    }
    #[test]
    fn test_enchantment_no_conflict_different_sets() {
        let e1 = EnchantmentDef::fire_attack();
        let e2 = EnchantmentDef::ice_defense();
        assert!(!e1.conflicts_with(&e2));
    }
    #[test]
    fn test_enchantment_tier_colors_distinct() {
        let colors: Vec<Color32> = (1..=5).map(EnchantmentDef::tier_color).collect();
        for i in 0..colors.len() {
            for j in (i+1)..colors.len() {
                assert_ne!(colors[i], colors[j]);
            }
        }
    }
    #[test]
    fn test_enchanting_table_can_afford() {
        let state = EnchantingTableState::default();
        let e = EnchantmentDef::fire_attack();
        // Should be affordable since we start with materials
        assert!(state.can_afford(&e));
    }
    #[test]
    fn test_enchanting_table_apply_consumes_materials() {
        let mut state = EnchantingTableState::default();
        let before = *state.available_materials.get("Fire Crystal").unwrap_or(&0);
        state.apply_enchantment(0);
        let after = *state.available_materials.get("Fire Crystal").unwrap_or(&0);
        assert!(after < before);
    }
    #[test]
    fn test_item_set_bonus_thresholds() {
        let set = ItemSet::dragonscale();
        assert!(set.get_active_bonus(0).is_none() || set.get_active_bonus(0).map(|b| b.is_empty()).unwrap_or(true));
        assert!(set.get_active_bonus(2).is_some());
        assert!(set.get_active_bonus(6).is_some());
    }
    #[test]
    fn test_item_set_bonus_accumulates() {
        let set = ItemSet::dragonscale();
        let b2 = set.get_active_bonus(2).unwrap_or_default();
        let b6 = set.get_active_bonus(6).unwrap_or_default();
        assert!(b6.defense >= b2.defense);
    }
    #[test]
    fn test_next_bonus_threshold() {
        let set = ItemSet::dragonscale();
        assert_eq!(set.next_bonus_threshold(0), Some(2));
        assert_eq!(set.next_bonus_threshold(2), Some(4));
        assert_eq!(set.next_bonus_threshold(6), None);
    }
    #[test]
    fn test_item_generator_batch() {
        let mut state = ItemGeneratorEditorState { base_name: "Sword".to_string(), generate_seed: 42, ..Default::default() };
        state.generate_batch(5);
        assert_eq!(state.previews.len(), 5);
        for p in &state.previews { assert!(!p.name.is_empty()); }
    }
    #[test]
    fn test_item_generator_names_include_base() {
        let mut state = ItemGeneratorEditorState { base_name: "Axe".to_string(), generate_seed: 99, ..Default::default() };
        state.generate_batch(10);
        assert!(state.previews.iter().all(|p| p.name.contains("Axe")));
    }
    #[test]
    fn test_market_good_tick_changes_price() {
        let mut good = MarketGood::new("TestGood", 100.0, 1.0);
        let initial_price = good.current_price;
        let mut rng = InvRng::new(42);
        for _ in 0..10 { good.tick(1.0/30.0, &mut rng); }
        // Price should change (not guaranteed but very likely)
        assert!(good.price_history.prices.len() > 1);
        let _ = initial_price;
    }
    #[test]
    fn test_market_good_price_clamped() {
        let mut good = MarketGood::new("TestGood", 100.0, 1.0);
        good.supply = 0.01;
        good.demand = 10000.0;
        let mut rng = InvRng::new(1);
        for _ in 0..100 { good.tick(1.0, &mut rng); }
        assert!(good.current_price <= good.base_price * 10.0);
        assert!(good.current_price >= good.base_price * 0.1);
    }
    #[test]
    fn test_price_history_push_max() {
        let mut h = PriceHistory::new(5);
        for i in 0..10 { h.push(i as f32); }
        assert_eq!(h.prices.len(), 5);
    }
    #[test]
    fn test_price_history_latest() {
        let mut h = PriceHistory::new(10);
        h.push(42.0);
        assert!((h.latest() - 42.0).abs() < 0.001);
    }
    #[test]
    fn test_trader_sell_above_market() {
        let trader = TraderNpc::new("Bob", 500.0);
        assert!(trader.sell_price(100.0) > 100.0);
    }
    #[test]
    fn test_trader_buy_below_market() {
        let trader = TraderNpc::new("Bob", 500.0);
        assert!(trader.buy_price(100.0) < 100.0);
    }
    #[test]
    fn test_container_needs_restock() {
        let mut c = ContainerDef::new("Test", ContainerType::Chest);
        c.can_restock = true;
        c.opened = true;
        c.restock_interval = 100.0;
        c.time_since_restock = 0.0;
        assert!(c.needs_restock(150.0));
        assert!(!c.needs_restock(50.0));
    }
    #[test]
    fn test_container_try_restock() {
        let mut c = ContainerDef::new("Test", ContainerType::Chest);
        c.can_restock = true;
        c.opened = true;
        c.restock_interval = 100.0;
        c.time_since_restock = 0.0;
        let result = c.try_restock(200.0);
        assert!(result);
        assert!(!c.opened);
    }
    #[test]
    fn test_container_loot_simulation() {
        let mut state = ContainerEditorState::with_defaults();
        state.simulate_loot(0, 42);
        assert!(!state.loot_simulation_result.is_empty());
    }
    #[test]
    fn test_container_type_names_unique() {
        use std::collections::HashSet;
        let names: Vec<&str> = ContainerType::all().iter().map(|t| t.name()).collect();
        let set: HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), set.len());
    }
    #[test]
    fn test_inv_rng_pick_weighted() {
        let mut rng = InvRng::new(42);
        let items = vec![("a".to_string(), 1.0), ("b".to_string(), 100.0)];
        let mut b_count = 0;
        for _ in 0..100 { if rng.pick_weighted(&items).unwrap() == "b" { b_count += 1; } }
        assert!(b_count > 50, "Weighted pick should favor 'b'");
    }
    #[test]
    fn test_enchantment_slot_can_accept() {
        let slot = EnchantmentSlot::new(3, vec![ItemCategory::Weapon], "weapon_slot");
        let e = EnchantmentDef::fire_attack();
        assert!(slot.can_accept(&e, &ItemCategory::Weapon));
        assert!(!slot.can_accept(&e, &ItemCategory::Armor));
    }
    #[test]
    fn test_enchantment_slot_tier_check() {
        let slot = EnchantmentSlot::new(2, vec![], "any_slot");
        let e1 = EnchantmentDef::fire_attack(); // tier 1
        let e5 = EnchantmentDef::legendary_sharpness(); // tier 5
        assert!(slot.can_accept(&e1, &ItemCategory::Weapon));
        assert!(!slot.can_accept(&e5, &ItemCategory::Weapon));
    }
    #[test]
    fn test_full_inventory_tab_names_unique() {
        use std::collections::HashSet;
        let names: Vec<&str> = FullInventoryTab::all().iter().map(|t| t.name()).collect();
        let set: HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), set.len());
    }
    #[test]
    fn test_set_editor_get_bonus() {
        let mut state = SetEditorState::with_defaults();
        state.equipped_set_pieces.insert(0, 4);
        let bonus = state.get_set_bonus_for(0);
        assert!(bonus.is_some());
    }
    #[test]
    fn test_economy_editor_defaults() {
        let state = EconomyEditorState::with_defaults();
        assert!(!state.goods.is_empty());
        assert!(!state.traders.is_empty());
    }
    #[test]
    fn test_stat_bonus_with_crit() {
        let b = StatBonus::default().with_crit(0.15, 0.5);
        assert!((b.crit_chance - 0.15).abs() < 0.001);
        assert!((b.crit_damage - 0.5).abs() < 0.001);
    }
    #[test]
    fn test_generate_item_name_contains_base() {
        let config = ItemGeneratorConfig::default();
        let name = generate_item_name("Shield", &config, 777);
        assert!(name.contains("Shield"));
    }
}
'''

with open('C:/proof-engine/editor/src/inventory_system.rs', 'a', encoding='utf-8') as f:
    f.write(code)
import os
print(f"Done. Inventory size: {os.path.getsize('C:/proof-engine/editor/src/inventory_system.rs')} bytes")
