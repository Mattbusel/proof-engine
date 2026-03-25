//! Dialogue generation — context-sensitive dialogue from character state.

use super::motivation::{Motivation, Personality, Need};
use crate::worldgen::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone { Friendly, Hostile, Neutral, Fearful, Respectful, Sarcastic, Desperate, Wise }

#[derive(Debug, Clone)]
pub struct DialogueLine { pub speaker: String, pub text: String, pub tone: Tone }

/// Generate a greeting based on personality and relationship.
pub fn generate_greeting(name: &str, personality: &Personality, relationship: f32, rng: &mut Rng) -> DialogueLine {
    let tone = if relationship > 0.5 { Tone::Friendly }
        else if relationship < -0.3 { Tone::Hostile }
        else { Tone::Neutral };

    let templates = match tone {
        Tone::Friendly => vec![
            format!("Welcome, friend! What brings you to my door?"),
            format!("Ah, good to see you again! Come, sit."),
            format!("A familiar face! The day grows brighter."),
        ],
        Tone::Hostile => vec![
            format!("What do you want? Make it quick."),
            format!("You again. I thought I made myself clear."),
            format!("Speak, before I change my mind about listening."),
        ],
        _ => vec![
            format!("Greetings, traveler."),
            format!("What can I do for you?"),
            format!("Hmm? Oh. You need something?"),
        ],
    };

    DialogueLine {
        speaker: name.to_string(),
        text: templates[rng.next_u64() as usize % templates.len()].clone(),
        tone,
    }
}

/// Generate dialogue about a topic based on NPC beliefs.
pub fn generate_topic_response(name: &str, topic: &str, belief_value: f32, rng: &mut Rng) -> DialogueLine {
    let tone = if belief_value > 0.5 { Tone::Respectful }
        else if belief_value < -0.3 { Tone::Sarcastic }
        else { Tone::Neutral };

    let text = if belief_value > 0.5 {
        format!("{} is something I hold dear. Let me tell you more...", topic)
    } else if belief_value < -0.3 {
        format!("{}? Hah. Don't get me started on that nonsense.", topic)
    } else {
        format!("{}? I suppose I have some thoughts on the matter.", topic)
    };

    DialogueLine { speaker: name.to_string(), text, tone }
}

/// Unreliable narrator: modify a description based on bias.
pub fn unreliable_narrate(event: &str, bias: f32, perspective: &str, rng: &mut Rng) -> String {
    if bias > 0.5 {
        format!("As {} recalls it, {} — though they may be too generous in their retelling.", perspective, event)
    } else if bias < -0.3 {
        format!("According to {}, {} — but one wonders if spite colors the memory.", perspective, event)
    } else {
        format!("{} remembers: {}", perspective, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greeting_friendly() {
        let personality = Personality { openness: 0.8, conscientiousness: 0.5, extraversion: 0.7, agreeableness: 0.8, neuroticism: 0.2 };
        let mut rng = Rng::new(42);
        let line = generate_greeting("Elder", &personality, 0.8, &mut rng);
        assert_eq!(line.tone, Tone::Friendly);
    }

    #[test]
    fn test_unreliable_narrator() {
        let mut rng = Rng::new(42);
        let biased = unreliable_narrate("the battle was fierce", 0.8, "the general", &mut rng);
        assert!(biased.contains("generous"));
    }
}
