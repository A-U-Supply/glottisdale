//! ARPABET phonetic feature matrix and distance calculations.

use std::collections::HashMap;

lazy_static::lazy_static! {
    /// IPA-to-ARPABET mapping for phonemes produced by BFA aligner.
    static ref IPA_TO_ARPABET: Vec<(&'static str, &'static str)> = vec![
        // Diphthongs first (multi-char)
        ("aɪ", "AY"), ("aʊ", "AW"), ("eɪ", "EY"), ("oʊ", "OW"), ("ɔɪ", "OY"),
        // Vowels
        ("i", "IY"), ("ɪ", "IH"), ("e", "EY"), ("ɛ", "EH"), ("æ", "AE"),
        ("ɑ", "AA"), ("ɒ", "AA"), ("ɔ", "AO"), ("o", "OW"), ("ʊ", "UH"),
        ("u", "UW"), ("ə", "AH"), ("ɜ", "ER"), ("ɐ", "AH"), ("ʌ", "AH"),
        ("a", "AA"),
        // Consonants — stops
        ("p", "P"), ("b", "B"), ("t", "T"), ("d", "D"), ("k", "K"), ("g", "G"),
        // Consonants — nasals
        ("m", "M"), ("n", "N"), ("ŋ", "NG"), ("ɲ", "N"), ("ɴ", "NG"),
        // Consonants — fricatives
        ("f", "F"), ("v", "V"), ("θ", "TH"), ("ð", "DH"), ("s", "S"),
        ("z", "Z"), ("ʃ", "SH"), ("ʒ", "ZH"), ("h", "HH"), ("ɦ", "HH"),
        ("ç", "HH"), ("x", "HH"), ("ɣ", "G"),
        // Consonants — liquids/rhotics
        ("l", "L"), ("ɫ", "L"), ("ɬ", "L"), ("ɮ", "L"),
        ("r", "R"), ("ɹ", "R"), ("ɾ", "R"), ("ɽ", "R"), ("ʁ", "R"), ("ʀ", "R"),
        // Consonants — glides
        ("j", "Y"), ("w", "W"), ("ɥ", "W"),
    ];

    /// Articulatory features for each ARPABET phoneme.
    static ref FEATURES: HashMap<&'static str, &'static [&'static str]> = {
        let mut m = HashMap::new();
        // Consonants: [type, manner, place, voicing]
        m.insert("P",  &["consonant", "stop", "bilabial", "voiceless"][..]);
        m.insert("B",  &["consonant", "stop", "bilabial", "voiced"][..]);
        m.insert("T",  &["consonant", "stop", "alveolar", "voiceless"][..]);
        m.insert("D",  &["consonant", "stop", "alveolar", "voiced"][..]);
        m.insert("K",  &["consonant", "stop", "velar", "voiceless"][..]);
        m.insert("G",  &["consonant", "stop", "velar", "voiced"][..]);
        m.insert("F",  &["consonant", "fricative", "labiodental", "voiceless"][..]);
        m.insert("V",  &["consonant", "fricative", "labiodental", "voiced"][..]);
        m.insert("TH", &["consonant", "fricative", "dental", "voiceless"][..]);
        m.insert("DH", &["consonant", "fricative", "dental", "voiced"][..]);
        m.insert("S",  &["consonant", "fricative", "alveolar", "voiceless"][..]);
        m.insert("Z",  &["consonant", "fricative", "alveolar", "voiced"][..]);
        m.insert("SH", &["consonant", "fricative", "postalveolar", "voiceless"][..]);
        m.insert("ZH", &["consonant", "fricative", "postalveolar", "voiced"][..]);
        m.insert("HH", &["consonant", "fricative", "glottal", "voiceless"][..]);
        m.insert("CH", &["consonant", "affricate", "postalveolar", "voiceless"][..]);
        m.insert("JH", &["consonant", "affricate", "postalveolar", "voiced"][..]);
        m.insert("M",  &["consonant", "nasal", "bilabial", "voiced"][..]);
        m.insert("N",  &["consonant", "nasal", "alveolar", "voiced"][..]);
        m.insert("NG", &["consonant", "nasal", "velar", "voiced"][..]);
        m.insert("L",  &["consonant", "liquid", "alveolar", "voiced"][..]);
        m.insert("R",  &["consonant", "liquid", "postalveolar", "voiced"][..]);
        m.insert("W",  &["consonant", "glide", "bilabial", "voiced"][..]);
        m.insert("Y",  &["consonant", "glide", "palatal", "voiced"][..]);
        // Vowels: [type, height, backness, roundness, tenseness]
        m.insert("IY", &["vowel", "high", "front", "unrounded", "tense"][..]);
        m.insert("IH", &["vowel", "high", "front", "unrounded", "lax"][..]);
        m.insert("EY", &["vowel", "mid", "front", "unrounded", "tense"][..]);
        m.insert("EH", &["vowel", "mid", "front", "unrounded", "lax"][..]);
        m.insert("AE", &["vowel", "low", "front", "unrounded", "lax"][..]);
        m.insert("AA", &["vowel", "low", "back", "unrounded", "tense"][..]);
        m.insert("AH", &["vowel", "mid", "central", "unrounded", "lax"][..]);
        m.insert("AO", &["vowel", "mid", "back", "rounded", "tense"][..]);
        m.insert("OW", &["vowel", "mid", "back", "rounded", "tense"][..]);
        m.insert("UH", &["vowel", "high", "back", "rounded", "lax"][..]);
        m.insert("UW", &["vowel", "high", "back", "rounded", "tense"][..]);
        m.insert("AW", &["vowel", "low", "central", "unrounded", "tense"][..]);
        m.insert("AY", &["vowel", "low", "central", "unrounded", "tense"][..]);
        m.insert("OY", &["vowel", "mid", "back", "rounded", "tense"][..]);
        m.insert("ER", &["vowel", "mid", "central", "rounded", "tense"][..]);
        m
    };
}

const CROSS_TYPE_DISTANCE: i32 = 5;

/// Strip trailing stress marker (0, 1, 2) from an ARPABET phoneme.
pub fn strip_stress(phoneme: &str) -> &str {
    phoneme.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// Convert an IPA phoneme to ARPABET if possible, passthrough otherwise.
pub fn normalize_phoneme(phoneme: &str) -> String {
    if phoneme.is_empty() {
        return phoneme.to_string();
    }

    // Already ARPABET (uppercase ASCII, possibly with stress digit)
    let base = strip_stress(phoneme);
    if !base.is_empty() && base.is_ascii() && base.chars().all(|c| c.is_uppercase()) {
        return phoneme.to_string();
    }

    // Strip IPA length markers
    let cleaned = phoneme.trim_end_matches(['ː', 'ˑ']);

    // Try multi-char diphthongs first, then single-char
    for (ipa, arpabet) in IPA_TO_ARPABET.iter() {
        if cleaned.starts_with(ipa) {
            return arpabet.to_string();
        }
    }

    phoneme.to_string()
}

/// Compute articulatory feature distance between two ARPABET phonemes.
///
/// Stress markers are ignored. Returns 0 for identical phonemes.
pub fn phoneme_distance(a: &str, b: &str) -> i32 {
    let a_base = strip_stress(a);
    let b_base = strip_stress(b);

    if a_base == b_base {
        return 0;
    }

    let feat_a = FEATURES.get(a_base);
    let feat_b = FEATURES.get(b_base);

    match (feat_a, feat_b) {
        (Some(fa), Some(fb)) => {
            if fa[0] != fb[0] {
                return CROSS_TYPE_DISTANCE;
            }
            fa[1..].iter().zip(fb[1..].iter()).filter(|(a, b)| a != b).count() as i32
        }
        _ => CROSS_TYPE_DISTANCE,
    }
}

/// Compute distance between two syllables (lists of ARPABET phonemes).
pub fn syllable_distance(a: &[String], b: &[String]) -> i32 {
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 0;
    }

    let mut total = 0;
    for i in 0..max_len {
        match (a.get(i), b.get(i)) {
            (Some(pa), Some(pb)) => total += phoneme_distance(pa, pb),
            _ => total += CROSS_TYPE_DISTANCE,
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phoneme_distance_identical() {
        assert_eq!(phoneme_distance("K", "K"), 0);
        assert_eq!(phoneme_distance("AE1", "AE0"), 0); // stress ignored
    }

    #[test]
    fn test_phoneme_distance_same_type() {
        // P and B: same manner, same place, different voicing = 1
        assert_eq!(phoneme_distance("P", "B"), 1);
        // P and K: same manner, same voicing, different place = 1
        assert_eq!(phoneme_distance("P", "K"), 1);
    }

    #[test]
    fn test_phoneme_distance_cross_type() {
        // Consonant vs vowel = max distance
        assert_eq!(phoneme_distance("K", "AE1"), CROSS_TYPE_DISTANCE);
    }

    #[test]
    fn test_phoneme_distance_unknown() {
        assert_eq!(phoneme_distance("K", "UNKNOWN"), CROSS_TYPE_DISTANCE);
    }

    #[test]
    fn test_syllable_distance_identical() {
        let a: Vec<String> = vec!["K", "AE1", "T"].iter().map(|s| s.to_string()).collect();
        assert_eq!(syllable_distance(&a, &a), 0);
    }

    #[test]
    fn test_syllable_distance_different_length() {
        let a: Vec<String> = vec!["K", "AE1", "T"].iter().map(|s| s.to_string()).collect();
        let b: Vec<String> = vec!["K", "AE1"].iter().map(|s| s.to_string()).collect();
        assert!(syllable_distance(&a, &b) > 0); // missing phoneme penalty
    }

    #[test]
    fn test_normalize_phoneme_ipa_vowel() {
        assert_eq!(normalize_phoneme("æ"), "AE");
        assert_eq!(normalize_phoneme("ɪ"), "IH");
    }

    #[test]
    fn test_normalize_phoneme_ipa_consonant() {
        assert_eq!(normalize_phoneme("k"), "K");
        assert_eq!(normalize_phoneme("ʃ"), "SH");
    }

    #[test]
    fn test_normalize_phoneme_already_arpabet() {
        assert_eq!(normalize_phoneme("AE1"), "AE1");
        assert_eq!(normalize_phoneme("K"), "K");
    }

    #[test]
    fn test_normalize_phoneme_diphthong() {
        assert_eq!(normalize_phoneme("aɪ"), "AY");
        assert_eq!(normalize_phoneme("oʊ"), "OW");
    }

    #[test]
    fn test_strip_stress() {
        assert_eq!(strip_stress("AE1"), "AE");
        assert_eq!(strip_stress("K"), "K");
        assert_eq!(strip_stress("IY0"), "IY");
    }
}
