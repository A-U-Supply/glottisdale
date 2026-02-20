//! Convert target text to ARPABET syllables for matching.

use crate::language::g2p;
use crate::language::syllabify_arpabet;

/// A syllable derived from target text (no audio timing).
#[derive(Debug, Clone)]
pub struct TextSyllable {
    /// ARPABET phonemes (with stress markers)
    pub phonemes: Vec<String>,
    /// Parent word (punctuation stripped)
    pub word: String,
    /// Position of the word in the text
    pub word_index: usize,
    /// Stress level (0, 1, 2) or None
    pub stress: Option<u8>,
}

/// Extract stress level from ARPABET phonemes.
fn extract_stress(phonemes: &[String]) -> Option<u8> {
    for p in phonemes {
        if let Some(last) = p.as_bytes().last() {
            if last.is_ascii_digit() {
                return Some(last - b'0');
            }
        }
    }
    None
}

/// Strip punctuation from edges of a word.
fn strip_punct(word: &str) -> String {
    word.trim_matches(|c: char| ".,!?;:\"'()-".contains(c))
        .to_string()
}

/// Convert raw text to a list of ARPABET syllables.
///
/// Uses G2P (CMU dictionary + rule-based fallback) for grapheme-to-phoneme
/// conversion, then the ARPABET syllabifier to split into syllables.
pub fn text_to_syllables(text: &str) -> Vec<TextSyllable> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();

    for (wi, word) in text.split_whitespace().enumerate() {
        let clean = strip_punct(word);
        if clean.is_empty() {
            continue;
        }

        let phonemes = g2p::word_to_phonemes(&clean);
        if phonemes.is_empty() {
            continue;
        }

        // Filter to valid ARPABET phonemes
        let filtered: Vec<String> = phonemes
            .into_iter()
            .filter(|p| {
                let trimmed = p.trim();
                !trimmed.is_empty()
                    && trimmed != " "
                    && (trimmed.chars().all(|c| c.is_alphabetic())
                        || trimmed
                            .chars()
                            .last()
                            .map(|c| c.is_ascii_digit())
                            .unwrap_or(false))
            })
            .collect();

        if filtered.is_empty() {
            continue;
        }

        // Syllabify
        let syl_tuples = match syllabify_arpabet::syllabify(&filtered, false) {
            Ok(syls) if !syls.is_empty() => syls,
            _ => {
                // Fallback: treat all phonemes as a single syllable
                vec![(vec![], filtered.clone(), vec![])]
            }
        };

        for (onset, nucleus, coda) in syl_tuples {
            let mut syl_phonemes = Vec::new();
            syl_phonemes.extend(onset);
            syl_phonemes.extend(nucleus);
            syl_phonemes.extend(coda);

            result.push(TextSyllable {
                stress: extract_stress(&syl_phonemes),
                phonemes: syl_phonemes,
                word: strip_punct(word),
                word_index: wi,
            });
        }
    }

    result
}

/// Return indices where new words begin.
pub fn word_boundaries_from_syllables(syllables: &[TextSyllable]) -> Vec<usize> {
    let mut boundaries = Vec::new();
    let mut last_word_index: Option<usize> = None;
    for (i, syl) in syllables.iter().enumerate() {
        if last_word_index != Some(syl.word_index) {
            boundaries.push(i);
            last_word_index = Some(syl.word_index);
        }
    }
    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_to_syllables_single_word() {
        let syls = text_to_syllables("cat");
        assert!(!syls.is_empty());
        assert_eq!(syls[0].word, "cat");
        // Should have ARPABET phonemes
        for p in &syls[0].phonemes {
            assert!(p.chars().next().unwrap().is_uppercase());
        }
    }

    #[test]
    fn test_text_to_syllables_multi_word() {
        let syls = text_to_syllables("hello world");
        assert!(syls.len() >= 2);
        let words: Vec<&str> = syls.iter().map(|s| s.word.as_str()).collect();
        assert!(words.contains(&"hello"));
        assert!(words.contains(&"world"));
    }

    #[test]
    fn test_text_to_syllables_empty() {
        assert!(text_to_syllables("").is_empty());
        assert!(text_to_syllables("  ").is_empty());
    }

    #[test]
    fn test_text_to_syllables_stress() {
        let syls = text_to_syllables("cat");
        // "cat" = K AE1 T, so stress should be 1
        assert!(syls.iter().any(|s| s.stress.is_some()));
    }

    #[test]
    fn test_text_to_syllables_punctuation() {
        let syls = text_to_syllables("hello, world!");
        for syl in &syls {
            assert!(!syl.word.contains(','));
            assert!(!syl.word.contains('!'));
            // No punctuation in phonemes
            for p in &syl.phonemes {
                assert!(
                    p.chars().next().unwrap().is_alphabetic(),
                    "Phoneme should start with alpha: {}",
                    p
                );
            }
        }
    }

    #[test]
    fn test_word_boundaries() {
        let syls = text_to_syllables("hello world");
        let bounds = word_boundaries_from_syllables(&syls);
        assert!(bounds.len() >= 2);
        assert_eq!(bounds[0], 0);
    }

    #[test]
    fn test_word_boundaries_empty() {
        let bounds = word_boundaries_from_syllables(&[]);
        assert!(bounds.is_empty());
    }

    #[test]
    fn test_strip_punct() {
        assert_eq!(strip_punct("hello,"), "hello");
        assert_eq!(strip_punct("\"world\""), "world");
        assert_eq!(strip_punct("(test)"), "test");
        assert_eq!(strip_punct("plain"), "plain");
    }
}
