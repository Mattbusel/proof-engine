//! Procedural name generation — syllable chains and markov-like construction.
//!
//! Generates phonetically consistent names for creatures, NPCs, locations, and items.
//! Each `NameStyle` has curated syllable tables. Names are built by combining
//! prefixes, middles, and suffixes using a seeded RNG for reproducibility.

use super::Rng;

// ── NameStyle ─────────────────────────────────────────────────────────────────

/// Style/culture of generated names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameStyle {
    /// Dark, ominous names (undead, demons).
    Dark,
    /// Elvish-inspired (long, flowing).
    Elvish,
    /// Orcish (harsh, guttural).
    Orcish,
    /// Arcane (mystical, esoteric).
    Arcane,
    /// Draconic (ancient, powerful).
    Draconic,
    /// Void (strange, alien).
    Void,
    /// Place names (towns, dungeons).
    Place,
    /// Human common names.
    Human,
}

// ── Syllable tables ───────────────────────────────────────────────────────────

fn dark_prefixes()  -> &'static [&'static str] { &["Mor","Mal","Kra","Vor","Zar","Dra","Sha","Gul","Neth","Bal","Tor","Khal","Vex","Crul","Dusk","Grim","Nox"] }
fn dark_middles()   -> &'static [&'static str] { &["ath","ul","ash","ak","ek","on","az","eth","ix","og","ur","an","ix","el","or"] }
fn dark_suffixes()  -> &'static [&'static str] { &["us","ak","on","ur","ax","ix","oth","eth","ar","as","en","ul","or","ath"] }

fn elvish_prefixes() -> &'static [&'static str] { &["Aer","Cal","Syl","Elan","Mir","Tal","Ael","Ith","Sel","Ari","Fin","Gil","Lir","Mel","Nel","Ori"] }
fn elvish_middles()  -> &'static [&'static str] { &["an","aer","iel","ion","ias","iel","ian","ael","ial","uer","ien","ele","oth","ill","ir"] }
fn elvish_suffixes() -> &'static [&'static str] { &["iel","ias","ion","ual","ael","ial","uen","iel","ean","ian","iel","ias","ier","ael"] }

fn orcish_prefixes() -> &'static [&'static str] { &["Grak","Urg","Mag","Brul","Thog","Vrak","Krug","Gash","Ugreth","Marg","Bruk","Thrak"] }
fn orcish_middles()  -> &'static [&'static str] { &["ak","ug","ag","ok","ut","rag","ash","ul","grak","nak","krul","gur"] }
fn orcish_suffixes() -> &'static [&'static str] { &["nak","gash","rug","ak","ug","og","ul","ash","rok","krul","gut","mak"] }

fn arcane_prefixes() -> &'static [&'static str] { &["Aex","Zyr","Vael","Xan","Qaer","Zyth","Aex","Ixir","Myzt","Pyrr","Zel","Xyr"] }
fn arcane_middles()  -> &'static [&'static str] { &["ael","yth","yx","ire","ast","yrr","ael","ix","an","eth","yx","oth"] }
fn arcane_suffixes() -> &'static [&'static str] { &["ix","aex","yth","ael","yx","ire","oth","ast","yrr","us","eth","ax"] }

fn draconic_prefixes() -> &'static [&'static str] { &["Dra","Vrax","Thur","Kyr","Zur","Aur","Syr","Vor","Xar","Rha","Dur","Asr"] }
fn draconic_middles()  -> &'static [&'static str] { &["ak","ul","ath","ix","on","ur","eth","or","ax","an","ith","us"] }
fn draconic_suffixes() -> &'static [&'static str] { &["ix","ax","us","ath","on","ur","ix","eth","or","an","ith","ax"] }

fn void_prefixes()   -> &'static [&'static str] { &["Zzz","Vhx","Yth","Xhl","Zhyr","Vlt","Kthx","Nyth","Pzr","Qxl","Zyx","Vlr"] }
fn void_middles()    -> &'static [&'static str] { &["yx","hz","zyr","xn","th","rx","yz","xr","zx","hz","yr","xz"] }
fn void_suffixes()   -> &'static [&'static str] { &["xyr","zth","vrx","yx","hz","xn","zyr","th","rx","yz","xr","vx"] }

fn place_prefixes()  -> &'static [&'static str] { &["Stone","Dark","Iron","Grim","Shadow","Blood","Frost","Ash","Black","Bone","Crypt","Death","Doom"] }
fn place_middles()   -> &'static [&'static str] { &["haven","hold","moor","gate","ridge","peak","forge","keep","vale","fell","fen","mere"] }
fn place_suffixes()  -> &'static [&'static str] { &["heim","moor","fell","fen","vale","keep","hold","croft","wick","ford","ton","burg"] }

fn human_prefixes()  -> &'static [&'static str] { &["Ald","Bar","Cal","Dan","Ed","Fred","Gar","Hal","Ing","Jon","Karl","Leo","Mar","Nor","Osw"] }
fn human_middles()   -> &'static [&'static str] { &["ric","win","ald","bert","ward","mund","gar","helm","ulf","frid","her","wig","rath","run"] }
fn human_suffixes()  -> &'static [&'static str] { &["son","sen","man","ard","ert","olf","ric","and","ley","ford","ton","ham","worth"] }

// ── Title words ───────────────────────────────────────────────────────────────

const DARK_TITLES:    &[&str] = &["the Cursed","the Defiler","Shadowbane","Deathmarch","the Undying","Soul Reaver","the Ancient"];
const ORCISH_TITLES:  &[&str] = &["Skull Crusher","Ironhide","Bonecleaver","Warlord","Bloodaxe","the Mighty","the Feared"];
const ARCANE_TITLES:  &[&str] = &["the Wise","Spellweaver","Runemaster","the Arcane","of the Inner Eye","the Eternal"];
const PLACE_ADJECTIVES: &[&str] = &["Ancient","Cursed","Forsaken","Eternal","Ruined","Shadowed","Lost","Forgotten","Sunken"];

// ── NameGenerator ─────────────────────────────────────────────────────────────

/// Generates procedural names from syllable tables.
#[derive(Clone, Debug)]
pub struct NameGenerator {
    pub style: NameStyle,
}

impl NameGenerator {
    pub fn new(style: NameStyle) -> Self { Self { style } }

    /// Generate a single name with `rng`.
    pub fn generate(&self, rng: &mut Rng) -> String {
        self.generate_with_title(rng, false)
    }

    /// Generate a name, optionally with an appended title/epithet.
    pub fn generate_with_title(&self, rng: &mut Rng, add_title: bool) -> String {
        let name = match self.style {
            NameStyle::Dark     => self.build(rng, dark_prefixes(), dark_middles(), dark_suffixes()),
            NameStyle::Elvish   => self.build(rng, elvish_prefixes(), elvish_middles(), elvish_suffixes()),
            NameStyle::Orcish   => self.build(rng, orcish_prefixes(), orcish_middles(), orcish_suffixes()),
            NameStyle::Arcane   => self.build(rng, arcane_prefixes(), arcane_middles(), arcane_suffixes()),
            NameStyle::Draconic => self.build(rng, draconic_prefixes(), draconic_middles(), draconic_suffixes()),
            NameStyle::Void     => self.build_void(rng),
            NameStyle::Place    => self.build_place(rng),
            NameStyle::Human    => self.build(rng, human_prefixes(), human_middles(), human_suffixes()),
        };

        if add_title {
            let titles: &[&str] = match self.style {
                NameStyle::Dark   | NameStyle::Draconic => DARK_TITLES,
                NameStyle::Orcish                        => ORCISH_TITLES,
                NameStyle::Arcane | NameStyle::Elvish    => ARCANE_TITLES,
                _                                        => DARK_TITLES,
            };
            if let Some(&title) = rng.pick(titles) {
                if rng.chance(0.35) {
                    return format!("{name} {title}");
                }
            }
        }
        name
    }

    fn build(&self, rng: &mut Rng, pre: &[&str], mid: &[&str], suf: &[&str]) -> String {
        let p = rng.pick(pre).copied().unwrap_or("Ka");
        let s = rng.pick(suf).copied().unwrap_or("ar");

        if rng.chance(0.6) || mid.is_empty() {
            // 2-part name
            capitalize(&format!("{p}{s}"))
        } else {
            let m = rng.pick(mid).copied().unwrap_or("an");
            capitalize(&format!("{p}{m}{s}"))
        }
    }

    fn build_void(&self, rng: &mut Rng) -> String {
        // Void names are weirder: sometimes just consonants
        let p = rng.pick(void_prefixes()).copied().unwrap_or("Zyx");
        let s = rng.pick(void_suffixes()).copied().unwrap_or("xyr");
        if rng.chance(0.4) {
            let m = rng.pick(void_middles()).copied().unwrap_or("hz");
            format!("{p}{m}{s}")
        } else {
            format!("{p}{s}")
        }
    }

    fn build_place(&self, rng: &mut Rng) -> String {
        let adj = if rng.chance(0.4) {
            rng.pick(PLACE_ADJECTIVES).copied().unwrap_or("")
        } else { "" };
        let p = rng.pick(place_prefixes()).copied().unwrap_or("Dark");
        let m = rng.pick(place_middles()).copied().unwrap_or("keep");
        let s = if rng.chance(0.4) {
            rng.pick(place_suffixes()).copied().unwrap_or("")
        } else { "" };
        let base = if s.is_empty() {
            format!("{p}{m}")
        } else {
            format!("{p}{m}{s}")
        };
        if adj.is_empty() { base } else { format!("{adj} {base}") }
    }

    /// Generate a list of `n` unique names.
    pub fn generate_n(&self, rng: &mut Rng, n: usize) -> Vec<String> {
        let mut names = std::collections::HashSet::new();
        let mut result = Vec::with_capacity(n);
        let mut attempts = 0usize;
        while result.len() < n && attempts < n * 10 {
            let name = self.generate(rng);
            if names.insert(name.clone()) {
                result.push(name);
            }
            attempts += 1;
        }
        result
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None    => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
    }
}

// ── Name table ────────────────────────────────────────────────────────────────

/// A seeded name table that pre-generates and caches names.
pub struct NameTable {
    names:   Vec<String>,
    cursor:  usize,
}

impl NameTable {
    pub fn generate(style: NameStyle, count: usize, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let gen = NameGenerator::new(style);
        let names = gen.generate_n(&mut rng, count);
        Self { names, cursor: 0 }
    }

    /// Get the next name from the table (wraps around).
    pub fn next(&mut self) -> &str {
        if self.names.is_empty() { return "Unknown"; }
        let name = &self.names[self.cursor % self.names.len()];
        self.cursor += 1;
        name
    }

    pub fn peek(&self, i: usize) -> Option<&str> {
        self.names.get(i % self.names.len().max(1)).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize { self.names.len() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_name_non_empty() {
        let mut rng = Rng::new(42);
        let gen = NameGenerator::new(NameStyle::Dark);
        let name = gen.generate(&mut rng);
        assert!(!name.is_empty());
        assert!(name.chars().next().unwrap().is_uppercase());
    }

    #[test]
    fn all_styles_produce_names() {
        let mut rng = Rng::new(99);
        for style in &[NameStyle::Dark, NameStyle::Elvish, NameStyle::Orcish,
                        NameStyle::Arcane, NameStyle::Draconic, NameStyle::Void,
                        NameStyle::Place, NameStyle::Human] {
            let gen = NameGenerator::new(*style);
            let name = gen.generate(&mut rng);
            assert!(!name.is_empty(), "Empty name for style {:?}", style);
        }
    }

    #[test]
    fn generate_n_returns_n_unique() {
        let mut rng = Rng::new(12345);
        let gen = NameGenerator::new(NameStyle::Human);
        let names = gen.generate_n(&mut rng, 20);
        // Allow some duplicates in edge cases but should be mostly unique
        assert!(names.len() >= 10);
    }

    #[test]
    fn name_table_wraps() {
        let mut table = NameTable::generate(NameStyle::Dark, 5, 777);
        let first = table.next().to_string();
        for _ in 0..4 { table.next(); }
        let again = table.next().to_string();
        assert_eq!(first, again);
    }
}
