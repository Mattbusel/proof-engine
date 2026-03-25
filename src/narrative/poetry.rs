//! Procedural poetry and song — generated verse with meter and rhyme.
//! Uses mathematical patterns like Fibonacci syllable counts.

use crate::worldgen::Rng;

/// Meter type for generated verse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meter { Iambic, Trochaic, Anapestic, Dactylic, Free, Fibonacci }

/// A line of poetry.
#[derive(Debug, Clone)]
pub struct VerseLine { pub text: String, pub syllable_count: usize, pub rhyme_sound: String }

/// A complete poem.
#[derive(Debug, Clone)]
pub struct Poem {
    pub title: String,
    pub lines: Vec<VerseLine>,
    pub meter: Meter,
    pub rhyme_scheme: String,
}

/// Generate a poem.
pub fn generate_poem(theme: &str, meter: Meter, lines: usize, rng: &mut Rng) -> Poem {
    let syllable_counts = match meter {
        Meter::Fibonacci => fibonacci_syllables(lines),
        Meter::Iambic => vec![10; lines],
        Meter::Trochaic => vec![8; lines],
        _ => (0..lines).map(|_| rng.range_usize(5, 12)).collect(),
    };

    let rhyme_endings = generate_rhyme_pairs(lines, rng);
    let verse_lines: Vec<VerseLine> = syllable_counts.iter().enumerate().map(|(i, &count)| {
        let text = generate_line(theme, count, rng);
        VerseLine { text, syllable_count: count, rhyme_sound: rhyme_endings[i].clone() }
    }).collect();

    let scheme = if lines == 4 { "ABAB".to_string() }
        else if lines == 2 { "AA".to_string() }
        else { "Free".to_string() };

    Poem { title: format!("Ode to {}", capitalize(theme)), lines: verse_lines, meter, rhyme_scheme: scheme }
}

fn fibonacci_syllables(n: usize) -> Vec<usize> {
    let mut fibs = Vec::with_capacity(n);
    let (mut a, mut b) = (1usize, 1usize);
    for _ in 0..n {
        fibs.push(a.max(1).min(13));
        let next = a + b;
        a = b;
        b = next;
    }
    fibs
}

fn generate_line(theme: &str, target_syllables: usize, rng: &mut Rng) -> String {
    let words_by_syl: Vec<(&str, usize)> = vec![
        ("the", 1), ("of", 1), ("and", 1), ("in", 1), ("a", 1), ("to", 1), ("with", 1),
        ("is", 1), ("was", 1), ("on", 1), ("by", 1), ("no", 1), ("from", 1),
        ("dark", 1), ("light", 1), ("wind", 1), ("stone", 1), ("fire", 1), ("rain", 1),
        ("shadow", 2), ("river", 2), ("mountain", 2), ("silence", 2), ("ancient", 2),
        ("golden", 2), ("silver", 2), ("fallen", 2), ("rising", 2), ("burning", 2),
        ("wandering", 3), ("eternal", 3), ("beautiful", 3), ("forgotten", 3), ("awakening", 4),
        ("remembering", 4), ("everlasting", 4),
    ];

    let mut line = Vec::new();
    let mut remaining = target_syllables;
    while remaining > 0 {
        let candidates: Vec<_> = words_by_syl.iter().filter(|(_, s)| *s <= remaining).collect();
        if candidates.is_empty() { break; }
        let &(word, syls) = candidates[rng.next_u64() as usize % candidates.len()];
        line.push(word);
        remaining -= syls;
    }

    let mut text = line.join(" ");
    if !text.is_empty() {
        let first = text.remove(0).to_uppercase().to_string();
        text = first + &text;
    }
    text
}

fn generate_rhyme_pairs(n: usize, rng: &mut Rng) -> Vec<String> {
    let endings = ["ight", "ane", "ow", "air", "ound", "ong", "aze", "ear", "ire", "one"];
    (0..n).map(|i| {
        let pair_idx = i / 2;
        endings[pair_idx % endings.len()].to_string()
    }).collect()
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
    fn test_fibonacci_poem() {
        let mut rng = Rng::new(42);
        let poem = generate_poem("shadow", Meter::Fibonacci, 6, &mut rng);
        assert_eq!(poem.lines.len(), 6);
        // Fibonacci: 1, 1, 2, 3, 5, 8
        assert_eq!(poem.lines[0].syllable_count, 1);
        assert_eq!(poem.lines[4].syllable_count, 5);
    }

    #[test]
    fn test_generate_line() {
        let mut rng = Rng::new(42);
        let line = generate_line("war", 8, &mut rng);
        assert!(!line.is_empty());
    }
}
