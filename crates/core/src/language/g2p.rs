//! Grapheme-to-phoneme conversion using the CMU Pronouncing Dictionary.
//!
//! Provides ARPABET pronunciations for English words. The CMU dict is
//! embedded at compile time. For out-of-vocabulary words, a simple
//! rule-based fallback is used.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The embedded CMU Pronouncing Dictionary.
///
/// Format: one word per line, "WORD  PH1 PH2 PH3 ..."
/// Lines starting with ";;;" are comments.
const CMU_DICT_DATA: &str = include_str!("cmudict.txt");

static CMU_DICT: OnceLock<HashMap<String, Vec<Vec<String>>>> = OnceLock::new();

fn get_dict() -> &'static HashMap<String, Vec<Vec<String>>> {
    CMU_DICT.get_or_init(|| {
        let mut dict: HashMap<String, Vec<Vec<String>>> = HashMap::new();
        for line in CMU_DICT_DATA.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(";;;") {
                continue;
            }
            // Format: "WORD PH1 PH2 PH3" (single space separated)
            // Variants: "WORD(2) PH1 PH2 PH3"
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() != 2 {
                continue;
            }

            let word_raw = parts[0];
            let phonemes_str = parts[1];

            // Strip variant marker: WORD(2) -> WORD
            let word = word_raw
                .split('(')
                .next()
                .unwrap_or(word_raw)
                .to_uppercase();

            let phonemes: Vec<String> = phonemes_str
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            if !phonemes.is_empty() {
                dict.entry(word).or_default().push(phonemes);
            }
        }
        dict
    })
}

/// Look up a word in the CMU dictionary.
///
/// Returns the first pronunciation variant as a list of ARPABET phonemes,
/// or `None` if the word is not found.
pub fn lookup(word: &str) -> Option<Vec<String>> {
    let dict = get_dict();
    let key = word.to_uppercase();
    dict.get(&key).and_then(|variants| variants.first().cloned())
}

/// Look up all pronunciation variants for a word.
pub fn lookup_all(word: &str) -> Option<&'static Vec<Vec<String>>> {
    let dict = get_dict();
    let key = word.to_uppercase();
    dict.get(&key)
}

/// Convert a word to ARPABET phonemes.
///
/// First tries the CMU dictionary, then falls back to a simple
/// rule-based conversion for OOV words.
pub fn word_to_phonemes(word: &str) -> Vec<String> {
    if let Some(phonemes) = lookup(word) {
        return phonemes;
    }
    // Rule-based fallback for OOV words
    simple_g2p(word)
}

/// Simple rule-based G2P fallback for out-of-vocabulary words.
///
/// This is a best-effort approximation. For production use, a proper
/// neural G2P model or espeak-ng would be better.
fn simple_g2p(word: &str) -> Vec<String> {
    let word = word.to_lowercase();
    let chars: Vec<char> = word.chars().collect();
    let mut phonemes = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let remaining = &word[i..];

        // Try digraphs first
        if remaining.len() >= 2 {
            let digraph = &remaining[..2];
            match digraph {
                "th" => { phonemes.push("TH".to_string()); i += 2; continue; }
                "sh" => { phonemes.push("SH".to_string()); i += 2; continue; }
                "ch" => { phonemes.push("CH".to_string()); i += 2; continue; }
                "ng" => { phonemes.push("NG".to_string()); i += 2; continue; }
                "ph" => { phonemes.push("F".to_string()); i += 2; continue; }
                "wh" => { phonemes.push("W".to_string()); i += 2; continue; }
                "ck" => { phonemes.push("K".to_string()); i += 2; continue; }
                "ee" => { phonemes.push("IY1".to_string()); i += 2; continue; }
                "ea" => { phonemes.push("IY1".to_string()); i += 2; continue; }
                "oo" => { phonemes.push("UW1".to_string()); i += 2; continue; }
                "ou" => { phonemes.push("AW1".to_string()); i += 2; continue; }
                "ow" => { phonemes.push("OW1".to_string()); i += 2; continue; }
                "ai" => { phonemes.push("EY1".to_string()); i += 2; continue; }
                "ay" => { phonemes.push("EY1".to_string()); i += 2; continue; }
                "oi" => { phonemes.push("OY1".to_string()); i += 2; continue; }
                "oy" => { phonemes.push("OY1".to_string()); i += 2; continue; }
                _ => {}
            }
        }

        // Single character mappings
        match chars[i] {
            'a' => phonemes.push("AE1".to_string()),
            'b' => phonemes.push("B".to_string()),
            'c' => {
                // c before e/i/y = S, otherwise K
                if i + 1 < chars.len() && matches!(chars[i + 1], 'e' | 'i' | 'y') {
                    phonemes.push("S".to_string());
                } else {
                    phonemes.push("K".to_string());
                }
            }
            'd' => phonemes.push("D".to_string()),
            'e' => {
                // Silent e at end of word
                if i == chars.len() - 1 && !phonemes.is_empty() {
                    // skip
                } else {
                    phonemes.push("EH1".to_string());
                }
            }
            'f' => phonemes.push("F".to_string()),
            'g' => phonemes.push("G".to_string()),
            'h' => phonemes.push("HH".to_string()),
            'i' => phonemes.push("IH1".to_string()),
            'j' => phonemes.push("JH".to_string()),
            'k' => phonemes.push("K".to_string()),
            'l' => phonemes.push("L".to_string()),
            'm' => phonemes.push("M".to_string()),
            'n' => phonemes.push("N".to_string()),
            'o' => phonemes.push("AA1".to_string()),
            'p' => phonemes.push("P".to_string()),
            'q' => phonemes.push("K".to_string()),
            'r' => phonemes.push("R".to_string()),
            's' => phonemes.push("S".to_string()),
            't' => phonemes.push("T".to_string()),
            'u' => phonemes.push("AH1".to_string()),
            'v' => phonemes.push("V".to_string()),
            'w' => phonemes.push("W".to_string()),
            'x' => {
                phonemes.push("K".to_string());
                phonemes.push("S".to_string());
            }
            'y' => {
                if phonemes.is_empty() {
                    phonemes.push("Y".to_string());
                } else {
                    phonemes.push("IY1".to_string());
                }
            }
            'z' => phonemes.push("Z".to_string()),
            _ => {} // Skip non-alphabetic
        }
        i += 1;
    }

    if phonemes.is_empty() {
        phonemes.push("AH0".to_string());
    }

    phonemes
}

/// Check if a phoneme is a vowel (has stress marker or is known vowel base).
pub fn is_vowel(phoneme: &str) -> bool {
    let base = phoneme.trim_end_matches(|c: char| c.is_ascii_digit());
    matches!(
        base,
        "AA" | "AE" | "AH" | "AO" | "AW" | "AY" | "EH" | "ER" | "EY" | "IH" | "IY" | "OW"
            | "OY" | "UH" | "UW"
    )
}

/// Strip stress markers from an ARPABET phoneme.
pub fn strip_stress(phoneme: &str) -> &str {
    phoneme.trim_end_matches(|c: char| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_common_words() {
        // These should be in any CMU dictionary
        assert!(lookup("the").is_some());
        assert!(lookup("hello").is_some());
        assert!(lookup("world").is_some());
        assert!(lookup("cat").is_some());
    }

    #[test]
    fn test_lookup_case_insensitive() {
        let lower = lookup("hello");
        let upper = lookup("HELLO");
        let mixed = lookup("Hello");
        assert_eq!(lower, upper);
        assert_eq!(lower, mixed);
    }

    #[test]
    fn test_lookup_nonexistent() {
        assert!(lookup("xyzzyplugh").is_none());
    }

    #[test]
    fn test_word_to_phonemes_known() {
        let phonemes = word_to_phonemes("cat");
        assert!(!phonemes.is_empty());
        // CAT should be K AE1 T
        assert_eq!(phonemes, vec!["K", "AE1", "T"]);
    }

    #[test]
    fn test_word_to_phonemes_oov() {
        // Made-up word should still return something
        let phonemes = word_to_phonemes("xyzzyplugh");
        assert!(!phonemes.is_empty());
    }

    #[test]
    fn test_is_vowel() {
        assert!(is_vowel("AE1"));
        assert!(is_vowel("IY0"));
        assert!(is_vowel("ER"));
        assert!(!is_vowel("K"));
        assert!(!is_vowel("S"));
        assert!(!is_vowel("TH"));
    }

    #[test]
    fn test_strip_stress() {
        assert_eq!(strip_stress("AE1"), "AE");
        assert_eq!(strip_stress("IY0"), "IY");
        assert_eq!(strip_stress("K"), "K");
    }

    #[test]
    fn test_simple_g2p_basic() {
        let result = simple_g2p("bat");
        assert!(!result.is_empty());
        // Should produce something like B AE T
        assert_eq!(result[0], "B");
    }
}
