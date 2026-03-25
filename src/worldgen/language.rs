//! Language generation — phonology, morphology, syntax, vocabulary.
//!
//! Each procedural culture gets a unique language with consistent
//! phonological rules, word formation patterns, and basic grammar.

use super::Rng;
use super::history::Civilization;

/// Phoneme categories.
#[derive(Debug, Clone)]
pub struct Phonology {
    pub consonants: Vec<char>,
    pub vowels: Vec<char>,
    /// Allowed syllable structures (C=consonant, V=vowel).
    pub syllable_patterns: Vec<String>,
    /// Forbidden consonant clusters.
    pub forbidden_clusters: Vec<String>,
}

/// Word class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WordClass { Noun, Verb, Adjective, Adverb, Preposition, Article, Pronoun, Conjunction }

/// A word in the language.
#[derive(Debug, Clone)]
pub struct Word {
    pub form: String,
    pub class: WordClass,
    pub meaning: String,
    pub root: String,
}

/// Morphological rules.
#[derive(Debug, Clone)]
pub struct Morphology {
    pub plural_suffix: String,
    pub past_suffix: String,
    pub future_prefix: String,
    pub negation_prefix: String,
    pub diminutive_suffix: String,
    pub augmentative_suffix: String,
    pub adjective_suffix: String,
    pub adverb_suffix: String,
}

/// Word order type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordOrder { SVO, SOV, VSO, VOS, OVS, OSV }

/// Basic syntax rules.
#[derive(Debug, Clone)]
pub struct Syntax {
    pub word_order: WordOrder,
    pub adjective_before_noun: bool,
    pub postpositions: bool,
    pub head_final: bool,
}

/// A complete procedural language.
#[derive(Debug, Clone)]
pub struct Language {
    pub id: u32,
    pub name: String,
    pub phonology: Phonology,
    pub morphology: Morphology,
    pub syntax: Syntax,
    pub vocabulary: Vec<Word>,
    pub owner_civ: u32,
}

impl Language {
    /// Generate a word from this language's phonology.
    pub fn generate_word(&self, rng: &mut Rng, syllables: usize) -> String {
        let mut word = String::new();
        for _ in 0..syllables {
            if let Some(pattern) = rng.pick(&self.phonology.syllable_patterns) {
                for ch in pattern.chars() {
                    match ch {
                        'C' => {
                            if let Some(&c) = rng.pick(&self.phonology.consonants) {
                                word.push(c);
                            }
                        }
                        'V' => {
                            if let Some(&v) = rng.pick(&self.phonology.vowels) {
                                word.push(v);
                            }
                        }
                        _ => word.push(ch),
                    }
                }
            }
        }
        word
    }

    /// Apply plural morphology.
    pub fn pluralize(&self, word: &str) -> String {
        format!("{}{}", word, self.morphology.plural_suffix)
    }

    /// Apply past tense.
    pub fn past_tense(&self, word: &str) -> String {
        format!("{}{}", word, self.morphology.past_suffix)
    }

    /// Construct a simple sentence: subject verb object.
    pub fn simple_sentence(&self, subject: &str, verb: &str, object: &str) -> String {
        match self.syntax.word_order {
            WordOrder::SVO => format!("{} {} {}", subject, verb, object),
            WordOrder::SOV => format!("{} {} {}", subject, object, verb),
            WordOrder::VSO => format!("{} {} {}", verb, subject, object),
            WordOrder::VOS => format!("{} {} {}", verb, object, subject),
            WordOrder::OVS => format!("{} {} {}", object, verb, subject),
            WordOrder::OSV => format!("{} {} {}", object, subject, verb),
        }
    }
}

/// Generate languages for civilizations.
pub fn generate(num_languages: usize, civs: &[Civilization], rng: &mut Rng) -> Vec<Language> {
    let mut languages = Vec::with_capacity(num_languages);

    for i in 0..num_languages {
        let phonology = generate_phonology(rng);
        let morphology = generate_morphology(&phonology, rng);
        let syntax = generate_syntax(rng);

        let mut lang = Language {
            id: i as u32,
            name: String::new(),
            phonology,
            morphology,
            syntax,
            vocabulary: Vec::new(),
            owner_civ: civs.get(i).map(|c| c.id).unwrap_or(i as u32),
        };

        // Generate base vocabulary
        let concepts = [
            ("sun", WordClass::Noun), ("moon", WordClass::Noun), ("water", WordClass::Noun),
            ("fire", WordClass::Noun), ("earth", WordClass::Noun), ("sky", WordClass::Noun),
            ("mountain", WordClass::Noun), ("river", WordClass::Noun), ("forest", WordClass::Noun),
            ("sea", WordClass::Noun), ("wind", WordClass::Noun), ("stone", WordClass::Noun),
            ("tree", WordClass::Noun), ("star", WordClass::Noun), ("rain", WordClass::Noun),
            ("snow", WordClass::Noun), ("life", WordClass::Noun), ("death", WordClass::Noun),
            ("war", WordClass::Noun), ("peace", WordClass::Noun), ("king", WordClass::Noun),
            ("god", WordClass::Noun), ("hero", WordClass::Noun), ("beast", WordClass::Noun),
            ("sword", WordClass::Noun), ("shield", WordClass::Noun), ("home", WordClass::Noun),
            ("walk", WordClass::Verb), ("fight", WordClass::Verb), ("speak", WordClass::Verb),
            ("see", WordClass::Verb), ("hear", WordClass::Verb), ("make", WordClass::Verb),
            ("give", WordClass::Verb), ("take", WordClass::Verb), ("live", WordClass::Verb),
            ("die", WordClass::Verb), ("love", WordClass::Verb), ("hate", WordClass::Verb),
            ("big", WordClass::Adjective), ("small", WordClass::Adjective),
            ("old", WordClass::Adjective), ("new", WordClass::Adjective),
            ("good", WordClass::Adjective), ("bad", WordClass::Adjective),
            ("dark", WordClass::Adjective), ("bright", WordClass::Adjective),
        ];

        for (meaning, class) in &concepts {
            let syllables = rng.range_usize(1, 4);
            let form = lang.generate_word(rng, syllables);
            lang.vocabulary.push(Word {
                form: form.clone(),
                class: *class,
                meaning: meaning.to_string(),
                root: form,
            });
        }

        // Name the language from its own phonology
        lang.name = lang.generate_word(rng, 2);
        lang.name = capitalize(&lang.name);

        languages.push(lang);
    }

    languages
}

fn generate_phonology(rng: &mut Rng) -> Phonology {
    let all_consonants: Vec<char> = "ptknmslrwjfvbdgzhʃ".chars().collect();
    let all_vowels: Vec<char> = "aeiouəæɛɔ".chars().collect();

    // Pick a subset
    let num_c = rng.range_usize(8, 16);
    let num_v = rng.range_usize(3, 7);
    let mut consonants: Vec<char> = all_consonants.clone();
    rng.shuffle(&mut consonants);
    consonants.truncate(num_c);
    let mut vowels: Vec<char> = all_vowels.clone();
    rng.shuffle(&mut vowels);
    vowels.truncate(num_v);

    let all_patterns = vec![
        "CV".to_string(), "CVC".to_string(), "VC".to_string(), "V".to_string(),
        "CCV".to_string(), "CVCC".to_string(), "CCVC".to_string(),
    ];
    let num_patterns = rng.range_usize(2, 5);
    let mut patterns = all_patterns.clone();
    rng.shuffle(&mut patterns);
    patterns.truncate(num_patterns);
    // Always include CV
    if !patterns.contains(&"CV".to_string()) { patterns.push("CV".to_string()); }

    Phonology { consonants, vowels, syllable_patterns: patterns, forbidden_clusters: Vec::new() }
}

fn generate_morphology(phon: &Phonology, rng: &mut Rng) -> Morphology {
    let gen_suffix = |rng: &mut Rng, phon: &Phonology| -> String {
        let v = phon.vowels[rng.next_u64() as usize % phon.vowels.len()];
        let c = phon.consonants[rng.next_u64() as usize % phon.consonants.len()];
        if rng.coin(0.5) { format!("{}{}", v, c) } else { format!("{}", v) }
    };

    Morphology {
        plural_suffix: gen_suffix(rng, phon),
        past_suffix: gen_suffix(rng, phon),
        future_prefix: gen_suffix(rng, phon),
        negation_prefix: gen_suffix(rng, phon),
        diminutive_suffix: gen_suffix(rng, phon),
        augmentative_suffix: gen_suffix(rng, phon),
        adjective_suffix: gen_suffix(rng, phon),
        adverb_suffix: gen_suffix(rng, phon),
    }
}

fn generate_syntax(rng: &mut Rng) -> Syntax {
    let order = match rng.range_u32(0, 6) {
        0 => WordOrder::SVO,
        1 => WordOrder::SOV,
        2 => WordOrder::VSO,
        3 => WordOrder::VOS,
        4 => WordOrder::OVS,
        _ => WordOrder::OSV,
    };
    Syntax {
        word_order: order,
        adjective_before_noun: rng.coin(0.5),
        postpositions: matches!(order, WordOrder::SOV | WordOrder::OSV),
        head_final: matches!(order, WordOrder::SOV | WordOrder::OVS),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_languages() {
        let civs = vec![]; // no civs needed for language gen
        let mut rng = Rng::new(42);
        let langs = generate(3, &civs, &mut rng);
        assert_eq!(langs.len(), 3);
        for lang in &langs {
            assert!(!lang.name.is_empty());
            assert!(!lang.vocabulary.is_empty());
            assert!(!lang.phonology.consonants.is_empty());
            assert!(!lang.phonology.vowels.is_empty());
        }
    }

    #[test]
    fn test_word_generation() {
        let mut rng = Rng::new(42);
        let civs = vec![];
        let langs = generate(1, &civs, &mut rng);
        let word = langs[0].generate_word(&mut rng, 3);
        assert!(!word.is_empty());
    }

    #[test]
    fn test_sentence_construction() {
        let mut rng = Rng::new(42);
        let civs = vec![];
        let langs = generate(1, &civs, &mut rng);
        let sentence = langs[0].simple_sentence("warrior", "fights", "beast");
        assert!(sentence.contains("warrior"));
        assert!(sentence.contains("fights"));
        assert!(sentence.contains("beast"));
    }
}
