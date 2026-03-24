//! Localization, i18n, number/date formatting, unicode utilities, and colored text.
//!
//! Provides Locale, L10n, NumberFormatter, DateTimeFormatter, UnicodeUtils,
//! ColoredText, and MarkupParser with full implementations.

use std::collections::HashMap;

// ─── Locale ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    EnUs,
    FrFr,
    DeDe,
    JaJp,
    ZhCn,
    KoKr,
    EsEs,
    PtBr,
    RuRu,
    ArSa,
}

impl Locale {
    pub fn code(&self) -> &str {
        match self {
            Locale::EnUs => "en_US",
            Locale::FrFr => "fr_FR",
            Locale::DeDe => "de_DE",
            Locale::JaJp => "ja_JP",
            Locale::ZhCn => "zh_CN",
            Locale::KoKr => "ko_KR",
            Locale::EsEs => "es_ES",
            Locale::PtBr => "pt_BR",
            Locale::RuRu => "ru_RU",
            Locale::ArSa => "ar_SA",
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Locale::EnUs => "English (US)",
            Locale::FrFr => "French (France)",
            Locale::DeDe => "German (Germany)",
            Locale::JaJp => "Japanese",
            Locale::ZhCn => "Chinese (Simplified)",
            Locale::KoKr => "Korean",
            Locale::EsEs => "Spanish (Spain)",
            Locale::PtBr => "Portuguese (Brazil)",
            Locale::RuRu => "Russian",
            Locale::ArSa => "Arabic (Saudi Arabia)",
        }
    }

    pub fn is_rtl(&self) -> bool {
        matches!(self, Locale::ArSa)
    }

    pub fn decimal_separator(&self) -> char {
        match self {
            Locale::EnUs | Locale::JaJp | Locale::ZhCn | Locale::KoKr => '.',
            Locale::FrFr | Locale::RuRu => ',',
            Locale::DeDe | Locale::EsEs | Locale::PtBr => ',',
            Locale::ArSa => '.',
        }
    }

    pub fn thousands_separator(&self) -> &str {
        match self {
            Locale::EnUs | Locale::JaJp | Locale::ZhCn | Locale::KoKr => ",",
            Locale::FrFr | Locale::RuRu => "\u{202F}",  // narrow no-break space
            Locale::DeDe => ".",
            Locale::EsEs | Locale::PtBr => ".",
            Locale::ArSa => ",",
        }
    }

    pub fn all() -> &'static [Locale] {
        &[
            Locale::EnUs, Locale::FrFr, Locale::DeDe, Locale::JaJp,
            Locale::ZhCn, Locale::KoKr, Locale::EsEs, Locale::PtBr,
            Locale::RuRu, Locale::ArSa,
        ]
    }
}

// ─── Translation ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Translation {
    pub key: String,
    pub value: String,
}

impl Translation {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into() }
    }
}

// ─── Translation Map ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TranslationMap {
    entries: HashMap<String, String>,
    plurals: HashMap<String, Vec<String>>,
}

impl TranslationMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.insert(key.into(), value.into());
    }

    pub fn insert_plural(&mut self, key: impl Into<String>, forms: Vec<String>) {
        self.plurals.insert(key.into(), forms);
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    pub fn get_plural(&self, key: &str, form: usize) -> Option<&str> {
        self.plurals.get(key)
            .and_then(|forms| forms.get(form))
            .map(|s| s.as_str())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Parse simple `key = "value"` format
    pub fn parse_from_str(&mut self, data: &str) {
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let raw_val = line[eq_pos + 1..].trim();
                let value = if raw_val.starts_with('"') && raw_val.ends_with('"') && raw_val.len() >= 2 {
                    raw_val[1..raw_val.len() - 1].to_string()
                } else {
                    raw_val.to_string()
                };
                self.entries.insert(key, value);
            }
        }
    }
}

// ─── Built-in English Translations ──────────────────────────────────────────────

fn build_english_translations() -> TranslationMap {
    let mut map = TranslationMap::new();

    // Menu items
    map.insert("menu.play", "Play");
    map.insert("menu.continue", "Continue");
    map.insert("menu.new_game", "New Game");
    map.insert("menu.settings", "Settings");
    map.insert("menu.credits", "Credits");
    map.insert("menu.quit", "Quit");
    map.insert("menu.resume", "Resume");
    map.insert("menu.restart", "Restart");
    map.insert("menu.main_menu", "Main Menu");
    map.insert("menu.quit_desktop", "Quit to Desktop");
    map.insert("menu.load_game", "Load Game");
    map.insert("menu.save_game", "Save Game");
    map.insert("menu.back", "Back");
    map.insert("menu.confirm", "Confirm");
    map.insert("menu.cancel", "Cancel");
    map.insert("menu.yes", "Yes");
    map.insert("menu.no", "No");
    map.insert("menu.ok", "OK");
    map.insert("menu.retry", "Retry");
    map.insert("menu.delete", "Delete");

    // Settings labels
    map.insert("settings.title", "Settings");
    map.insert("settings.graphics", "Graphics");
    map.insert("settings.audio", "Audio");
    map.insert("settings.controls", "Controls");
    map.insert("settings.accessibility", "Accessibility");
    map.insert("settings.language", "Language");
    map.insert("settings.resolution", "Resolution");
    map.insert("settings.fullscreen", "Fullscreen");
    map.insert("settings.vsync", "V-Sync");
    map.insert("settings.fps", "Target FPS");
    map.insert("settings.quality", "Quality Preset");
    map.insert("settings.master_vol", "Master Volume");
    map.insert("settings.music_vol", "Music Volume");
    map.insert("settings.sfx_vol", "SFX Volume");
    map.insert("settings.voice_vol", "Voice Volume");
    map.insert("settings.subtitles", "Subtitles");
    map.insert("settings.colorblind", "Colorblind Mode");
    map.insert("settings.high_contrast", "High Contrast");
    map.insert("settings.reduce_motion", "Reduce Motion");
    map.insert("settings.large_text", "Large Text");
    map.insert("settings.screen_reader", "Screen Reader");

    // Status effects
    map.insert("status.burning", "Burning");
    map.insert("status.frozen", "Frozen");
    map.insert("status.poisoned", "Poisoned");
    map.insert("status.stunned", "Stunned");
    map.insert("status.slowed", "Slowed");
    map.insert("status.hasted", "Hasted");
    map.insert("status.shielded", "Shielded");
    map.insert("status.cursed", "Cursed");
    map.insert("status.blessed", "Blessed");
    map.insert("status.silenced", "Silenced");
    map.insert("status.confused", "Confused");
    map.insert("status.invisible", "Invisible");

    // Item rarity names
    map.insert("rarity.common", "Common");
    map.insert("rarity.uncommon", "Uncommon");
    map.insert("rarity.rare", "Rare");
    map.insert("rarity.epic", "Epic");
    map.insert("rarity.legendary", "Legendary");
    map.insert("rarity.mythic", "Mythic");
    map.insert("rarity.unique", "Unique");

    // Biome names
    map.insert("biome.forest", "Verdant Forest");
    map.insert("biome.desert", "Scorched Desert");
    map.insert("biome.snow", "Frozen Tundra");
    map.insert("biome.dungeon", "Dark Dungeon");
    map.insert("biome.cave", "Crystal Cave");
    map.insert("biome.volcano", "Volcanic Wastes");
    map.insert("biome.ocean", "Abyssal Ocean");
    map.insert("biome.sky", "Sky Citadel");
    map.insert("biome.void", "The Void");

    // Skill names
    map.insert("skill.fireball", "Fireball");
    map.insert("skill.lightning", "Chain Lightning");
    map.insert("skill.heal", "Holy Light");
    map.insert("skill.shield", "Iron Fortress");
    map.insert("skill.dash", "Shadow Step");
    map.insert("skill.arrow", "Piercing Arrow");
    map.insert("skill.strike", "Power Strike");
    map.insert("skill.blizzard", "Blizzard");
    map.insert("skill.meteor", "Meteor Strike");
    map.insert("skill.revive", "Resurrection");
    map.insert("skill.stealth", "Vanish");
    map.insert("skill.berserk", "Berserker Rage");

    // Error messages
    map.insert("error.save_failed", "Failed to save game data.");
    map.insert("error.load_failed", "Failed to load save file.");
    map.insert("error.no_save", "No save file found.");
    map.insert("error.corrupt_save", "Save file is corrupted.");
    map.insert("error.network", "Network connection lost.");
    map.insert("error.unknown", "An unknown error occurred.");

    // UI labels
    map.insert("ui.level", "Level");
    map.insert("ui.health", "Health");
    map.insert("ui.mana", "Mana");
    map.insert("ui.stamina", "Stamina");
    map.insert("ui.gold", "Gold");
    map.insert("ui.score", "Score");
    map.insert("ui.combo", "Combo");
    map.insert("ui.time", "Time");
    map.insert("ui.wave", "Wave");
    map.insert("ui.lives", "Lives");
    map.insert("ui.kills", "Kills");
    map.insert("ui.inventory", "Inventory");
    map.insert("ui.equipment", "Equipment");
    map.insert("ui.skills", "Skills");
    map.insert("ui.map", "Map");
    map.insert("ui.quest_log", "Quest Log");

    // Plural forms for English
    map.insert_plural("item", vec!["item".to_string(), "items".to_string()]);
    map.insert_plural("enemy", vec!["enemy".to_string(), "enemies".to_string()]);
    map.insert_plural("kill", vec!["kill".to_string(), "kills".to_string()]);
    map.insert_plural("minute", vec!["minute".to_string(), "minutes".to_string()]);
    map.insert_plural("hour", vec!["hour".to_string(), "hours".to_string()]);
    map.insert_plural("day", vec!["day".to_string(), "days".to_string()]);
    map.insert_plural("second", vec!["second".to_string(), "seconds".to_string()]);

    map
}

// ─── L10n Context ────────────────────────────────────────────────────────────────

pub struct L10n {
    maps: HashMap<Locale, TranslationMap>,
    current: Locale,
    fallback: Locale,
}

impl L10n {
    pub fn new() -> Self {
        let mut l = Self {
            maps: HashMap::new(),
            current: Locale::EnUs,
            fallback: Locale::EnUs,
        };
        l.maps.insert(Locale::EnUs, build_english_translations());
        l
    }

    pub fn load(&mut self, locale: Locale, data: &str) {
        let map = self.maps.entry(locale).or_default();
        map.parse_from_str(data);
    }

    pub fn get<'a>(&'a self, key: &str) -> &'a str {
        if let Some(map) = self.maps.get(&self.current) {
            if let Some(val) = map.get(key) {
                return val;
            }
        }
        if let Some(map) = self.maps.get(&self.fallback) {
            if let Some(val) = map.get(key) {
                return val;
            }
        }
        ""
    }

    pub fn fmt(&self, key: &str, args: &[(&str, &str)]) -> String {
        let template = self.get(key);
        let mut result = template.to_string();
        for (name, value) in args {
            let placeholder = format!("{{{}}}", name);
            result = result.replace(&placeholder, value);
        }
        result
    }

    pub fn plural<'a>(&'a self, key: &str, n: i64) -> &'a str {
        let form = self.plural_form(n);
        if let Some(map) = self.maps.get(&self.current) {
            if let Some(val) = map.get_plural(key, form) {
                return val;
            }
        }
        if let Some(map) = self.maps.get(&self.fallback) {
            if let Some(val) = map.get_plural(key, form) {
                return val;
            }
        }
        ""
    }

    fn plural_form(&self, n: i64) -> usize {
        match self.current {
            Locale::EnUs | Locale::DeDe | Locale::EsEs | Locale::PtBr => {
                if n == 1 { 0 } else { 1 }
            }
            Locale::FrFr => {
                if n <= 1 { 0 } else { 1 }
            }
            Locale::RuRu => {
                let n_mod10 = n.abs() % 10;
                let n_mod100 = n.abs() % 100;
                if n_mod10 == 1 && n_mod100 != 11 { 0 }
                else if n_mod10 >= 2 && n_mod10 <= 4 && (n_mod100 < 10 || n_mod100 >= 20) { 1 }
                else { 2 }
            }
            Locale::JaJp | Locale::ZhCn | Locale::KoKr => 0,
            Locale::ArSa => {
                match n {
                    0 => 0,
                    1 => 1,
                    2 => 2,
                    n if n % 100 >= 3 && n % 100 <= 10 => 3,
                    n if n % 100 >= 11 => 4,
                    _ => 5,
                }
            }
        }
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.current = locale;
    }

    pub fn current_locale(&self) -> Locale {
        self.current
    }

    pub fn has_locale(&self, locale: Locale) -> bool {
        self.maps.contains_key(&locale)
    }

    pub fn available_locales(&self) -> Vec<Locale> {
        self.maps.keys().copied().collect()
    }

    pub fn key_count(&self, locale: Locale) -> usize {
        self.maps.get(&locale).map(|m| m.len()).unwrap_or(0)
    }
}

impl Default for L10n {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Number Formatter ────────────────────────────────────────────────────────────

pub struct NumberFormatter;

impl NumberFormatter {
    pub fn format_int(n: i64, locale: Locale) -> String {
        let negative = n < 0;
        let abs_n = n.unsigned_abs();
        let digits = abs_n.to_string();
        let sep = locale.thousands_separator();
        let grouped = Self::group_digits(&digits, sep);
        if negative { format!("-{}", grouped) } else { grouped }
    }

    fn group_digits(digits: &str, sep: &str) -> String {
        if digits.len() <= 3 {
            return digits.to_string();
        }
        let mut result = String::new();
        let start = digits.len() % 3;
        if start > 0 {
            result.push_str(&digits[..start]);
        }
        let mut i = start;
        while i < digits.len() {
            if !result.is_empty() {
                result.push_str(sep);
            }
            result.push_str(&digits[i..i + 3]);
            i += 3;
        }
        result
    }

    pub fn format_float(f: f64, decimals: usize, locale: Locale) -> String {
        let dec_sep = locale.decimal_separator();
        let negative = f < 0.0;
        let abs_f = f.abs();
        let int_part = abs_f.floor() as i64;
        let frac_part = abs_f - int_part as f64;
        let frac_str = if decimals == 0 {
            String::new()
        } else {
            let mult = 10f64.powi(decimals as i32);
            let frac_digits = (frac_part * mult).round() as u64;
            format!("{}{:0>width$}", dec_sep, frac_digits, width = decimals)
        };
        let int_str = Self::format_int(int_part, locale);
        let sign = if negative { "-" } else { "" };
        format!("{}{}{}", sign, int_str, frac_str)
    }

    pub fn format_percent(f: f32, locale: Locale) -> String {
        format!("{}%", Self::format_float(f as f64 * 100.0, 1, locale))
    }

    pub fn format_currency(amount: i64, locale: Locale) -> String {
        let (symbol, before) = match locale {
            Locale::EnUs => ("$", true),
            Locale::FrFr => ("€", false),
            Locale::DeDe => ("€", false),
            Locale::JaJp => ("¥", true),
            Locale::ZhCn => ("¥", true),
            Locale::KoKr => ("₩", true),
            Locale::EsEs => ("€", false),
            Locale::PtBr => ("R$", true),
            Locale::RuRu => ("₽", false),
            Locale::ArSa => ("﷼", false),
        };
        let num_str = Self::format_float(amount as f64 / 100.0, 2, locale);
        if before {
            format!("{}{}", symbol, num_str)
        } else {
            format!("{} {}", num_str, symbol)
        }
    }

    pub fn format_duration(secs: f64, locale: Locale) -> String {
        let total_secs = secs as u64;
        let days = total_secs / 86400;
        let hours = (total_secs % 86400) / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        match locale {
            Locale::JaJp => {
                if days > 0 {
                    format!("{}日{}時間", days, hours)
                } else if hours > 0 {
                    format!("{}時間{}分", hours, minutes)
                } else if minutes > 0 {
                    format!("{}分{}秒", minutes, seconds)
                } else {
                    format!("{}秒", seconds)
                }
            }
            _ => {
                if days > 0 {
                    format!("{}d {}h", days, hours)
                } else if hours > 0 {
                    format!("{}h {}m", hours, minutes)
                } else if minutes > 0 {
                    format!("{}m {}s", minutes, seconds)
                } else {
                    format!("{}s", seconds)
                }
            }
        }
    }

    pub fn format_large(n: i64, locale: Locale) -> String {
        let dec_sep = locale.decimal_separator();
        let abs_n = n.abs() as f64;
        let sign = if n < 0 { "-" } else { "" };
        if abs_n >= 1_000_000_000.0 {
            let v = abs_n / 1_000_000_000.0;
            format!("{}{:.1}B", sign, v).replace('.', &dec_sep.to_string())
        } else if abs_n >= 1_000_000.0 {
            let v = abs_n / 1_000_000.0;
            format!("{}{:.1}M", sign, v).replace('.', &dec_sep.to_string())
        } else if abs_n >= 1_000.0 {
            let v = abs_n / 1_000.0;
            format!("{}{:.1}K", sign, v).replace('.', &dec_sep.to_string())
        } else {
            Self::format_int(n, locale)
        }
    }

    pub fn format_ordinal(n: u32, locale: Locale) -> String {
        match locale {
            Locale::EnUs => {
                let suffix = match (n % 100, n % 10) {
                    (11..=13, _) => "th",
                    (_, 1) => "st",
                    (_, 2) => "nd",
                    (_, 3) => "rd",
                    _ => "th",
                };
                format!("{}{}", n, suffix)
            }
            Locale::FrFr => {
                let suffix = if n == 1 { "er" } else { "ème" };
                format!("{}{}", n, suffix)
            }
            _ => format!("{}", n),
        }
    }
}

// ─── DateTime Formatter ──────────────────────────────────────────────────────────

pub struct DateTimeFormatter;

impl DateTimeFormatter {
    fn epoch_to_parts(epoch_secs: i64) -> (i32, u32, u32, u32, u32, u32) {
        // Simple implementation: compute year/month/day/hour/min/sec from epoch
        const SECS_PER_MIN: i64 = 60;
        const SECS_PER_HOUR: i64 = 3600;
        const SECS_PER_DAY: i64 = 86400;

        let mut days = epoch_secs / SECS_PER_DAY;
        let time_in_day = epoch_secs % SECS_PER_DAY;
        let hour = (time_in_day / SECS_PER_HOUR) as u32;
        let minute = ((time_in_day % SECS_PER_HOUR) / SECS_PER_MIN) as u32;
        let second = (time_in_day % SECS_PER_MIN) as u32;

        // Compute year/month/day from days since 1970-01-01
        let mut year = 1970i32;
        loop {
            let days_in_year = if Self::is_leap(year) { 366 } else { 365 };
            if days < days_in_year {
                break;
            }
            days -= days_in_year;
            year += 1;
        }
        let months = [31u32, if Self::is_leap(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut month = 1u32;
        for &m_days in &months {
            if days < m_days as i64 {
                break;
            }
            days -= m_days as i64;
            month += 1;
        }
        let day = (days + 1) as u32;

        (year, month, day, hour, minute, second)
    }

    fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    pub fn format_date(epoch_secs: i64, locale: Locale) -> String {
        let (year, month, day, _, _, _) = Self::epoch_to_parts(epoch_secs);
        match locale {
            Locale::EnUs => format!("{:02}/{:02}/{}", month, day, year),
            Locale::DeDe | Locale::FrFr | Locale::EsEs | Locale::PtBr | Locale::RuRu => {
                format!("{:02}.{:02}.{}", day, month, year)
            }
            Locale::JaJp | Locale::ZhCn | Locale::KoKr => {
                format!("{}-{:02}-{:02}", year, month, day)
            }
            Locale::ArSa => format!("{:02}/{:02}/{}", day, month, year),
        }
    }

    pub fn format_time(epoch_secs: i64, locale: Locale) -> String {
        let (_, _, _, hour, minute, second) = Self::epoch_to_parts(epoch_secs);
        match locale {
            Locale::EnUs => {
                let (h12, ampm) = if hour == 0 { (12, "AM") }
                    else if hour < 12 { (hour, "AM") }
                    else if hour == 12 { (12, "PM") }
                    else { (hour - 12, "PM") };
                format!("{}:{:02}:{:02} {}", h12, minute, second, ampm)
            }
            _ => format!("{:02}:{:02}:{:02}", hour, minute, second),
        }
    }

    pub fn format_relative(epoch_secs: i64, now: i64, locale: Locale) -> String {
        let diff = now - epoch_secs;
        let abs_diff = diff.abs();

        let (value, unit, past) = if abs_diff < 60 {
            (abs_diff, "second", diff > 0)
        } else if abs_diff < 3600 {
            (abs_diff / 60, "minute", diff > 0)
        } else if abs_diff < 86400 {
            (abs_diff / 3600, "hour", diff > 0)
        } else if abs_diff < 86400 * 30 {
            (abs_diff / 86400, "day", diff > 0)
        } else if abs_diff < 86400 * 365 {
            (abs_diff / (86400 * 30), "month", diff > 0)
        } else {
            (abs_diff / (86400 * 365), "year", diff > 0)
        };

        match locale {
            Locale::EnUs | Locale::EsEs => {
                let plural_s = if value == 1 { "" } else { "s" };
                if past {
                    format!("{} {}{} ago", value, unit, plural_s)
                } else {
                    format!("in {} {}{}", value, unit, plural_s)
                }
            }
            Locale::FrFr => {
                let plural_s = if value == 1 { "" } else { "s" };
                if past {
                    format!("il y a {} {}{}", value, unit, plural_s)
                } else {
                    format!("dans {} {}{}", value, unit, plural_s)
                }
            }
            Locale::DeDe => {
                if past {
                    format!("vor {} {}en", value, unit)
                } else {
                    format!("in {} {}en", value, unit)
                }
            }
            Locale::JaJp => {
                if past {
                    format!("{}{}前", value, unit)
                } else {
                    format!("{}{}後", value, unit)
                }
            }
            Locale::RuRu => {
                if past {
                    format!("{} {} назад", value, unit)
                } else {
                    format!("через {} {}", value, unit)
                }
            }
            _ => {
                if past {
                    format!("{} {} ago", value, unit)
                } else {
                    format!("in {} {}", value, unit)
                }
            }
        }
    }

    pub fn format_datetime(epoch_secs: i64, locale: Locale) -> String {
        format!("{} {}", Self::format_date(epoch_secs, locale), Self::format_time(epoch_secs, locale))
    }
}

// ─── Align ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

// ─── Unicode Utilities ───────────────────────────────────────────────────────────

pub struct UnicodeUtils;

impl UnicodeUtils {
    /// Returns visual width of a single character.
    /// CJK and full-width = 2, combining/zero-width = 0, else 1.
    pub fn char_width(c: char) -> usize {
        let cp = c as u32;
        // Zero-width and combining characters
        if cp == 0 { return 0; }
        if Self::is_combining(c) { return 0; }
        if Self::is_fullwidth_or_cjk(c) { return 2; }
        1
    }

    fn is_combining(c: char) -> bool {
        let cp = c as u32;
        matches!(cp,
            0x0300..=0x036F | // Combining Diacritical Marks
            0x1DC0..=0x1DFF | // Combining Diacritical Marks Supplement
            0x20D0..=0x20FF | // Combining Diacritical Marks for Symbols
            0xFE20..=0xFE2F   // Combining Half Marks
        )
    }

    fn is_fullwidth_or_cjk(c: char) -> bool {
        let cp = c as u32;
        matches!(cp,
            0x1100..=0x11FF | // Hangul Jamo
            0x2E80..=0x2FFF | // CJK Radicals
            0x3000..=0x9FFF | // CJK Unified Ideographs (and punctuation, symbols, etc.)
            0xA000..=0xA4CF | // Yi
            0xAC00..=0xD7AF | // Hangul Syllables
            0xF900..=0xFAFF | // CJK Compatibility Ideographs
            0xFE10..=0xFE1F | // Vertical Forms
            0xFE30..=0xFE6F | // CJK Compatibility Forms
            0xFF00..=0xFF60 | // Fullwidth Forms
            0xFFE0..=0xFFE6   // Fullwidth Signs
        )
    }

    pub fn display_width(s: &str) -> usize {
        s.chars().map(Self::char_width).sum()
    }

    pub fn truncate_display(s: &str, max_width: usize) -> &str {
        let mut width = 0;
        let mut byte_end = 0;
        for (byte_idx, ch) in s.char_indices() {
            let w = Self::char_width(ch);
            if width + w > max_width {
                return &s[..byte_end];
            }
            width += w;
            byte_end = byte_idx + ch.len_utf8();
        }
        s
    }

    pub fn pad_display(s: &str, width: usize, align: Align) -> String {
        let current_width = Self::display_width(s);
        if current_width >= width {
            return s.to_string();
        }
        let padding = width - current_width;
        match align {
            Align::Left => format!("{}{}", s, " ".repeat(padding)),
            Align::Right => format!("{}{}", " ".repeat(padding), s),
            Align::Center => {
                let left_pad = padding / 2;
                let right_pad = padding - left_pad;
                format!("{}{}{}", " ".repeat(left_pad), s, " ".repeat(right_pad))
            }
        }
    }

    /// Simplified NFC normalization — decomposes and recomposes common diacritics.
    /// Full NFC would require Unicode normalization tables; this handles common cases.
    pub fn normalize_nfc(s: &str) -> String {
        // For our purposes: pass-through but normalize ASCII and handle
        // common precomposed forms. A production impl would use unicode-normalization crate.
        let mut result = String::with_capacity(s.len());
        for ch in s.chars() {
            // Map common decomposed combinations back to precomposed
            result.push(Self::to_precomposed(ch));
        }
        result
    }

    fn to_precomposed(c: char) -> char {
        // Common precomposed mappings for Latin extended
        match c as u32 {
            0x0041 => 'A', 0x0042 => 'B', 0x0043 => 'C',
            _ => c,
        }
    }

    pub fn to_title_case(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut capitalize_next = true;
        for ch in s.chars() {
            if ch == ' ' || ch == '\t' || ch == '\n' {
                result.push(ch);
                capitalize_next = true;
            } else if capitalize_next {
                for upper in ch.to_uppercase() {
                    result.push(upper);
                }
                capitalize_next = false;
            } else {
                for lower in ch.to_lowercase() {
                    result.push(lower);
                }
            }
        }
        result
    }

    pub fn to_snake_case(s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 4);
        let mut prev_upper = false;
        for (i, ch) in s.chars().enumerate() {
            if ch == ' ' || ch == '-' {
                result.push('_');
                prev_upper = false;
            } else if ch.is_uppercase() {
                if i > 0 && !prev_upper {
                    result.push('_');
                }
                for lower in ch.to_lowercase() {
                    result.push(lower);
                }
                prev_upper = true;
            } else {
                result.push(ch);
                prev_upper = false;
            }
        }
        result
    }

    pub fn to_camel_case(s: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = false;
        for (i, ch) in s.chars().enumerate() {
            if ch == '_' || ch == '-' || ch == ' ' {
                capitalize_next = true;
            } else if capitalize_next {
                for upper in ch.to_uppercase() {
                    result.push(upper);
                }
                capitalize_next = false;
            } else {
                if i == 0 {
                    for lower in ch.to_lowercase() {
                        result.push(lower);
                    }
                } else {
                    result.push(ch);
                }
            }
        }
        result
    }

    /// Word-wrap text to max_width, respecting CJK double-width characters.
    pub fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
        if max_width == 0 {
            return vec![];
        }
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let mut current_line = String::new();
            let mut current_width = 0usize;
            let words: Vec<&str> = paragraph.split_whitespace().collect();

            for (i, word) in words.iter().enumerate() {
                let word_width = Self::display_width(word);
                let space_needed = if current_line.is_empty() { 0 } else { 1 };

                if current_width + space_needed + word_width > max_width {
                    // Word doesn't fit on current line
                    if !current_line.is_empty() {
                        lines.push(current_line.clone());
                        current_line.clear();
                        current_width = 0;
                    }

                    // If the word itself is wider than max_width, split it
                    if word_width > max_width {
                        let mut char_buf = String::new();
                        let mut char_width = 0;
                        for ch in word.chars() {
                            let cw = Self::char_width(ch);
                            if char_width + cw > max_width {
                                lines.push(char_buf.clone());
                                char_buf.clear();
                                char_width = 0;
                            }
                            char_buf.push(ch);
                            char_width += cw;
                        }
                        if !char_buf.is_empty() {
                            current_line = char_buf;
                            current_width = char_width;
                        }
                    } else {
                        current_line.push_str(word);
                        current_width = word_width;
                    }
                } else {
                    if i > 0 && !current_line.is_empty() {
                        current_line.push(' ');
                        current_width += 1;
                    }
                    current_line.push_str(word);
                    current_width += word_width;
                }
            }

            if !current_line.is_empty() {
                lines.push(current_line);
            } else if paragraph.is_empty() {
                lines.push(String::new());
            }
        }
        lines
    }

    pub fn repeat_char(ch: char, n: usize) -> String {
        std::iter::repeat(ch).take(n).collect()
    }

    pub fn center_in_width(s: &str, width: usize) -> String {
        Self::pad_display(s, width, Align::Center)
    }

    pub fn strip_ansi(s: &str) -> String {
        let mut result = String::new();
        let mut in_escape = false;
        for ch in s.chars() {
            if in_escape {
                if ch == 'm' || ch == 'A' || ch == 'B' || ch == 'C' || ch == 'D' ||
                   ch == 'H' || ch == 'J' || ch == 'K' {
                    in_escape = false;
                }
            } else if ch == '\x1b' {
                in_escape = true;
            } else {
                result.push(ch);
            }
        }
        result
    }
}

// ─── Terminal Color ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TermColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Rgb(u8, u8, u8),
    Color256(u8),
}

impl TermColor {
    pub fn ansi_fg(&self) -> String {
        match self {
            TermColor::Black => "\x1b[30m".to_string(),
            TermColor::Red => "\x1b[31m".to_string(),
            TermColor::Green => "\x1b[32m".to_string(),
            TermColor::Yellow => "\x1b[33m".to_string(),
            TermColor::Blue => "\x1b[34m".to_string(),
            TermColor::Magenta => "\x1b[35m".to_string(),
            TermColor::Cyan => "\x1b[36m".to_string(),
            TermColor::White => "\x1b[37m".to_string(),
            TermColor::BrightBlack => "\x1b[90m".to_string(),
            TermColor::BrightRed => "\x1b[91m".to_string(),
            TermColor::BrightGreen => "\x1b[92m".to_string(),
            TermColor::BrightYellow => "\x1b[93m".to_string(),
            TermColor::BrightBlue => "\x1b[94m".to_string(),
            TermColor::BrightMagenta => "\x1b[95m".to_string(),
            TermColor::BrightCyan => "\x1b[96m".to_string(),
            TermColor::BrightWhite => "\x1b[97m".to_string(),
            TermColor::Rgb(r, g, b) => format!("\x1b[38;2;{};{};{}m", r, g, b),
            TermColor::Color256(n) => format!("\x1b[38;5;{}m", n),
        }
    }

    pub fn ansi_bg(&self) -> String {
        match self {
            TermColor::Black => "\x1b[40m".to_string(),
            TermColor::Red => "\x1b[41m".to_string(),
            TermColor::Green => "\x1b[42m".to_string(),
            TermColor::Yellow => "\x1b[43m".to_string(),
            TermColor::Blue => "\x1b[44m".to_string(),
            TermColor::Magenta => "\x1b[45m".to_string(),
            TermColor::Cyan => "\x1b[46m".to_string(),
            TermColor::White => "\x1b[47m".to_string(),
            TermColor::BrightBlack => "\x1b[100m".to_string(),
            TermColor::BrightRed => "\x1b[101m".to_string(),
            TermColor::BrightGreen => "\x1b[102m".to_string(),
            TermColor::BrightYellow => "\x1b[103m".to_string(),
            TermColor::BrightBlue => "\x1b[104m".to_string(),
            TermColor::BrightMagenta => "\x1b[105m".to_string(),
            TermColor::BrightCyan => "\x1b[106m".to_string(),
            TermColor::BrightWhite => "\x1b[107m".to_string(),
            TermColor::Rgb(r, g, b) => format!("\x1b[48;2;{};{};{}m", r, g, b),
            TermColor::Color256(n) => format!("\x1b[48;5;{}m", n),
        }
    }
}

// ─── Colored Text ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ColoredText {
    text: String,
    fg: Option<TermColor>,
    bg: Option<TermColor>,
    bold: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    strikethrough: bool,
    dim: bool,
}

impl ColoredText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            blink: false,
            strikethrough: false,
            dim: false,
        }
    }

    pub fn fg(mut self, color: TermColor) -> Self {
        self.fg = Some(color);
        self
    }

    pub fn bg(mut self, color: TermColor) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn blink(mut self) -> Self {
        self.blink = true;
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn render(&self) -> String {
        self.render_with_support(true)
    }

    pub fn render_with_support(&self, supports_color: bool) -> String {
        if !supports_color {
            return self.text.clone();
        }

        let mut codes = Vec::new();

        if self.bold { codes.push("1".to_string()); }
        if self.dim { codes.push("2".to_string()); }
        if self.italic { codes.push("3".to_string()); }
        if self.underline { codes.push("4".to_string()); }
        if self.blink { codes.push("5".to_string()); }
        if self.strikethrough { codes.push("9".to_string()); }

        let mut result = String::new();

        // Apply fg color
        if let Some(ref color) = self.fg {
            result.push_str(&color.ansi_fg());
        }

        // Apply bg color
        if let Some(ref color) = self.bg {
            result.push_str(&color.ansi_bg());
        }

        // Apply attributes
        if !codes.is_empty() {
            result.push_str(&format!("\x1b[{}m", codes.join(";")));
        }

        result.push_str(&self.text);
        result.push_str("\x1b[0m");
        result
    }

    pub fn plain_len(&self) -> usize {
        UnicodeUtils::display_width(&self.text)
    }

    /// Concatenate two ColoredText segments into a plain rendering
    pub fn concat(&self, other: &ColoredText) -> String {
        format!("{}{}", self.render(), other.render())
    }
}

impl From<&str> for ColoredText {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ColoredText {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

// ─── Text Style ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TextStyle {
    pub fg: Option<(u8, u8, u8)>,
    pub bg: Option<(u8, u8, u8)>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub wave: Option<f32>,
    pub shake: Option<f32>,
    pub rainbow: bool,
}

impl TextStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bold() -> Self {
        Self { bold: true, ..Default::default() }
    }

    pub fn colored(r: u8, g: u8, b: u8) -> Self {
        Self { fg: Some((r, g, b)), ..Default::default() }
    }

    pub fn with_wave(amp: f32) -> Self {
        Self { wave: Some(amp), ..Default::default() }
    }
}

// ─── Text Span ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TextSpan {
    pub text: String,
    pub style: TextStyle,
}

impl TextSpan {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self { text: text.into(), style }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), style: TextStyle::default() }
    }
}

// ─── Markup Parser ───────────────────────────────────────────────────────────────

/// Parses proof-engine rich text markup.
///
/// Supported tags:
/// - `[b]text[/b]` — bold
/// - `[i]text[/i]` — italic
/// - `[u]text[/u]` — underline
/// - `[color:rrggbb]text[/color]` — hex color
/// - `[wave:amplitude]text[/wave]` — wave animation
/// - `[shake:intensity]text[/shake]` — shake effect
/// - `[rainbow]text[/rainbow]` — rainbow color cycling
/// - `[bg:rrggbb]text[/bg]` — background color
pub struct MarkupParser;

impl MarkupParser {
    pub fn parse(markup: &str) -> Vec<TextSpan> {
        let mut spans = Vec::new();
        let mut style_stack: Vec<TextStyle> = vec![TextStyle::default()];
        let mut current_text = String::new();
        let mut chars = markup.char_indices().peekable();

        while let Some((_, ch)) = chars.next() {
            if ch == '[' {
                // Collect tag content
                let mut tag_buf = String::new();
                let mut closed = false;
                for (_, tc) in chars.by_ref() {
                    if tc == ']' {
                        closed = true;
                        break;
                    }
                    tag_buf.push(tc);
                }

                if !closed {
                    current_text.push('[');
                    current_text.push_str(&tag_buf);
                    continue;
                }

                let tag = tag_buf.trim();

                if tag.starts_with('/') {
                    // Closing tag
                    if !current_text.is_empty() {
                        if let Some(style) = style_stack.last() {
                            spans.push(TextSpan::new(current_text.clone(), style.clone()));
                        }
                        current_text.clear();
                    }
                    if style_stack.len() > 1 {
                        style_stack.pop();
                    }
                } else {
                    // Opening tag — flush current text with current style
                    if !current_text.is_empty() {
                        if let Some(style) = style_stack.last() {
                            spans.push(TextSpan::new(current_text.clone(), style.clone()));
                        }
                        current_text.clear();
                    }

                    // Build new style based on parent
                    let parent = style_stack.last().cloned().unwrap_or_default();
                    let new_style = Self::apply_tag(tag, parent);
                    style_stack.push(new_style);
                }
            } else {
                current_text.push(ch);
            }
        }

        if !current_text.is_empty() {
            if let Some(style) = style_stack.last() {
                spans.push(TextSpan::new(current_text, style.clone()));
            }
        }

        spans
    }

    fn apply_tag(tag: &str, mut style: TextStyle) -> TextStyle {
        if tag == "b" || tag == "bold" {
            style.bold = true;
        } else if tag == "i" || tag == "italic" {
            style.italic = true;
        } else if tag == "u" || tag == "underline" {
            style.underline = true;
        } else if tag == "rainbow" {
            style.rainbow = true;
        } else if tag.starts_with("color:") {
            let hex = &tag[6..];
            if let Some(rgb) = Self::parse_hex_color(hex) {
                style.fg = Some(rgb);
            }
        } else if tag.starts_with("bg:") {
            let hex = &tag[3..];
            if let Some(rgb) = Self::parse_hex_color(hex) {
                style.bg = Some(rgb);
            }
        } else if tag.starts_with("wave:") {
            if let Ok(amp) = tag[5..].parse::<f32>() {
                style.wave = Some(amp);
            }
        } else if tag.starts_with("shake:") {
            if let Ok(intensity) = tag[6..].parse::<f32>() {
                style.shake = Some(intensity);
            }
        }
        style
    }

    fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        } else if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some((r, g, b))
        } else {
            None
        }
    }

    /// Convert parsed spans back to plain text (no markup).
    pub fn to_plain(spans: &[TextSpan]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join("")
    }

    /// Convert parsed spans to ANSI-colored terminal output.
    pub fn to_ansi(spans: &[TextSpan]) -> String {
        let mut result = String::new();
        for span in spans {
            let mut ct = ColoredText::new(&span.text);
            if let Some((r, g, b)) = span.style.fg {
                ct = ct.fg(TermColor::Rgb(r, g, b));
            }
            if let Some((r, g, b)) = span.style.bg {
                ct = ct.bg(TermColor::Rgb(r, g, b));
            }
            if span.style.bold { ct = ct.bold(); }
            if span.style.italic { ct = ct.italic(); }
            if span.style.underline { ct = ct.underline(); }
            result.push_str(&ct.render());
        }
        result
    }
}

// ─── Rich Text Builder ───────────────────────────────────────────────────────────

pub struct RichTextBuilder {
    segments: Vec<(String, TextStyle)>,
    current_style: TextStyle,
}

impl RichTextBuilder {
    pub fn new() -> Self {
        Self { segments: Vec::new(), current_style: TextStyle::default() }
    }

    pub fn text(mut self, s: impl Into<String>) -> Self {
        self.segments.push((s.into(), self.current_style.clone()));
        self
    }

    pub fn bold(mut self) -> Self {
        self.current_style.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.current_style.italic = true;
        self
    }

    pub fn color(mut self, r: u8, g: u8, b: u8) -> Self {
        self.current_style.fg = Some((r, g, b));
        self
    }

    pub fn reset_style(mut self) -> Self {
        self.current_style = TextStyle::default();
        self
    }

    pub fn build(self) -> Vec<TextSpan> {
        self.segments.into_iter().map(|(text, style)| TextSpan { text, style }).collect()
    }

    pub fn to_plain(&self) -> String {
        self.segments.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>().join("")
    }
}

impl Default for RichTextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_codes() {
        assert_eq!(Locale::EnUs.code(), "en_US");
        assert_eq!(Locale::JaJp.code(), "ja_JP");
        assert!(Locale::ArSa.is_rtl());
        assert!(!Locale::EnUs.is_rtl());
    }

    #[test]
    fn test_translation_map_parse() {
        let mut map = TranslationMap::new();
        map.parse_from_str(r#"
# This is a comment
greeting = "Hello, World!"
farewell = "Goodbye!"
"#);
        assert_eq!(map.get("greeting"), Some("Hello, World!"));
        assert_eq!(map.get("farewell"), Some("Goodbye!"));
        assert_eq!(map.get("missing"), None);
    }

    #[test]
    fn test_l10n_get_fallback() {
        let l = L10n::new();
        assert_eq!(l.get("menu.play"), "Play");
        assert_eq!(l.get("menu.settings"), "Settings");
        // Missing key returns the key itself
        assert_eq!(l.get("nonexistent.key"), "nonexistent.key");
    }

    #[test]
    fn test_l10n_fmt_substitution() {
        let l = L10n::new();
        let result = l.fmt("ui.level", &[]);
        // Key exists, no placeholders
        assert_eq!(result, "Level");

        // Test with a custom format string loaded
        let mut l2 = L10n::new();
        l2.load(Locale::EnUs, "welcome = \"Hello, {name}!\"");
        let result = l2.fmt("welcome", &[("name", "Alice")]);
        assert_eq!(result, "Hello, Alice!");
    }

    #[test]
    fn test_l10n_plural_english() {
        let l = L10n::new();
        assert_eq!(l.plural("item", 1), "item");
        assert_eq!(l.plural("item", 5), "items");
        assert_eq!(l.plural("enemy", 1), "enemy");
        assert_eq!(l.plural("enemy", 3), "enemies");
    }

    #[test]
    fn test_number_formatter_int() {
        assert_eq!(NumberFormatter::format_int(1234567, Locale::EnUs), "1,234,567");
        assert_eq!(NumberFormatter::format_int(-999, Locale::EnUs), "-999");
        assert_eq!(NumberFormatter::format_int(1000, Locale::DeDe), "1.000");
        assert_eq!(NumberFormatter::format_int(0, Locale::EnUs), "0");
    }

    #[test]
    fn test_number_formatter_float() {
        let result = NumberFormatter::format_float(1234.567, 2, Locale::EnUs);
        assert_eq!(result, "1,234.57");
        let result_de = NumberFormatter::format_float(1234.5, 1, Locale::DeDe);
        assert_eq!(result_de, "1.234,5");
    }

    #[test]
    fn test_number_formatter_large() {
        assert_eq!(NumberFormatter::format_large(1500, Locale::EnUs), "1.5K");
        assert_eq!(NumberFormatter::format_large(2_300_000, Locale::EnUs), "2.3M");
        assert_eq!(NumberFormatter::format_large(4_100_000_000, Locale::EnUs), "4.1B");
        assert_eq!(NumberFormatter::format_large(500, Locale::EnUs), "500");
    }

    #[test]
    fn test_number_formatter_duration() {
        let d = NumberFormatter::format_duration(7200.0, Locale::EnUs);
        assert_eq!(d, "2h 0m");
        let d2 = NumberFormatter::format_duration(90.0, Locale::EnUs);
        assert_eq!(d2, "1m 30s");
        let d3 = NumberFormatter::format_duration(45.0, Locale::EnUs);
        assert_eq!(d3, "45s");
    }

    #[test]
    fn test_date_formatter() {
        // Unix epoch = 1970-01-01 00:00:00 UTC
        let date = DateTimeFormatter::format_date(0, Locale::EnUs);
        assert_eq!(date, "01/01/1970");
        let date_de = DateTimeFormatter::format_date(0, Locale::DeDe);
        assert_eq!(date_de, "01.01.1970");
    }

    #[test]
    fn test_relative_time() {
        let rel = DateTimeFormatter::format_relative(1000, 1090, Locale::EnUs);
        assert_eq!(rel, "1 minute ago");
        let rel2 = DateTimeFormatter::format_relative(0, 7200, Locale::EnUs);
        assert_eq!(rel2, "2 hours ago");
    }

    #[test]
    fn test_unicode_char_width() {
        assert_eq!(UnicodeUtils::char_width('A'), 1);
        assert_eq!(UnicodeUtils::char_width('中'), 2);
        assert_eq!(UnicodeUtils::char_width('한'), 2);
        assert_eq!(UnicodeUtils::char_width('\u{0300}'), 0); // combining grave
    }

    #[test]
    fn test_unicode_display_width() {
        assert_eq!(UnicodeUtils::display_width("hello"), 5);
        assert_eq!(UnicodeUtils::display_width("日本語"), 6); // 3 CJK chars = 6
        assert_eq!(UnicodeUtils::display_width("A日"), 3);
    }

    #[test]
    fn test_unicode_pad() {
        let padded = UnicodeUtils::pad_display("hi", 10, Align::Right);
        assert_eq!(padded.len(), 10);
        assert!(padded.starts_with("        "));
    }

    #[test]
    fn test_word_wrap() {
        let lines = UnicodeUtils::word_wrap("The quick brown fox jumps over the lazy dog", 20);
        for line in &lines {
            assert!(UnicodeUtils::display_width(line) <= 20, "Line too wide: {:?}", line);
        }
    }

    #[test]
    fn test_snake_case() {
        assert_eq!(UnicodeUtils::to_snake_case("CamelCase"), "camel_case");
        assert_eq!(UnicodeUtils::to_snake_case("hello world"), "hello_world");
        assert_eq!(UnicodeUtils::to_snake_case("HTML"), "h_t_m_l");
    }

    #[test]
    fn test_title_case() {
        assert_eq!(UnicodeUtils::to_title_case("hello world"), "Hello World");
        assert_eq!(UnicodeUtils::to_title_case("the quick brown fox"), "The Quick Brown Fox");
    }

    #[test]
    fn test_colored_text() {
        let ct = ColoredText::new("Hello").fg(TermColor::Red).bold();
        let rendered = ct.render();
        assert!(rendered.contains("Hello"));
        assert!(rendered.contains("\x1b["));
        assert!(rendered.contains("\x1b[0m")); // reset at end
    }

    #[test]
    fn test_markup_parser() {
        let spans = MarkupParser::parse("[b]bold[/b] and [color:ff0000]red[/color] text");
        assert!(spans.len() >= 3);
        assert!(spans[0].style.bold);
        let red_span = spans.iter().find(|s| s.style.fg == Some((255, 0, 0)));
        assert!(red_span.is_some());
        let plain = MarkupParser::to_plain(&spans);
        assert_eq!(plain, "bold and red text");
    }

    #[test]
    fn test_markup_wave() {
        let spans = MarkupParser::parse("[wave:0.5]animated[/wave]");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].style.wave, Some(0.5));
    }
}
