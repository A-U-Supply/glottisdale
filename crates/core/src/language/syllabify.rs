//! Syllabification: words with timestamps → syllable boundaries.
//!
//! Combines G2P (grapheme-to-phoneme) with the ARPABET syllabifier
//! to produce syllable-level timestamps from word-level timestamps.

use crate::types::{Phoneme, Syllable, WordTimestamp};

use super::g2p;
use super::syllabify_arpabet;

/// Split a word's phonemes into syllables with estimated timestamps.
///
/// Timestamps are distributed proportionally across syllables based on
/// phoneme count per syllable.
pub fn syllabify_word(
    phonemes: &[String],
    word_start: f64,
    word_end: f64,
    word: &str,
    word_index: usize,
) -> Vec<Syllable> {
    if phonemes.is_empty() {
        return vec![];
    }

    let syl_tuples = match syllabify_arpabet::syllabify(phonemes, true) {
        Ok(tuples) if !tuples.is_empty() => tuples,
        _ => {
            // Fallback: treat entire word as one syllable
            vec![(vec![], phonemes.to_vec(), vec![])]
        }
    };

    // Build phoneme lists per syllable
    let syl_phoneme_lists: Vec<Vec<&str>> = syl_tuples
        .iter()
        .map(|(onset, nucleus, coda)| {
            onset
                .iter()
                .chain(nucleus.iter())
                .chain(coda.iter())
                .map(|s| s.as_str())
                .collect()
        })
        .collect();

    let total_phonemes: usize = syl_phoneme_lists.iter().map(|s| s.len()).sum();
    let total_phonemes = if total_phonemes == 0 { 1 } else { total_phonemes };
    let word_duration = word_end - word_start;

    let mut syllables = Vec::new();
    let mut current_time = word_start;

    for syl_phones in &syl_phoneme_lists {
        let proportion = syl_phones.len() as f64 / total_phonemes as f64;
        let syl_duration = word_duration * proportion;
        let syl_end = current_time + syl_duration;

        let mut phoneme_objects = Vec::new();
        if !syl_phones.is_empty() {
            let ph_dur = syl_duration / syl_phones.len() as f64;
            let mut ph_time = current_time;
            for &label in syl_phones {
                phoneme_objects.push(Phoneme {
                    label: label.to_string(),
                    start: round4(ph_time),
                    end: round4(ph_time + ph_dur),
                });
                ph_time += ph_dur;
            }
        }

        syllables.push(Syllable {
            phonemes: phoneme_objects,
            start: round4(current_time),
            end: round4(syl_end),
            word: word.to_string(),
            word_index,
        });
        current_time = syl_end;
    }

    syllables
}

/// Convert word-level timestamps to syllable-level timestamps.
///
/// Uses G2P to get phonemes for each word, then the ARPABET syllabifier
/// to split into syllables with proportionally distributed timestamps.
pub fn syllabify_words(words: &[WordTimestamp]) -> Vec<Syllable> {
    let mut all_syllables = Vec::new();

    for (i, w) in words.iter().enumerate() {
        let text = w.word.trim();
        if text.is_empty() {
            continue;
        }

        let phonemes = g2p::word_to_phonemes(text);
        if phonemes.is_empty() {
            continue;
        }

        let syls = syllabify_word(&phonemes, w.start, w.end, text, i);
        all_syllables.extend(syls);
    }

    all_syllables
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syllabify_word_cat() {
        let phonemes: Vec<String> = vec!["K", "AE1", "T"]
            .into_iter()
            .map(String::from)
            .collect();
        let result = syllabify_word(&phonemes, 0.0, 0.5, "cat", 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].word, "cat");
        assert!((result[0].start - 0.0).abs() < 0.001);
        assert!((result[0].end - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_syllabify_word_banana() {
        let phonemes: Vec<String> = vec!["B", "AH0", "N", "AE1", "N", "AH0"]
            .into_iter()
            .map(String::from)
            .collect();
        let result = syllabify_word(&phonemes, 0.0, 1.0, "banana", 0);
        assert_eq!(result.len(), 3);
        // Timestamps should be consecutive
        assert!((result[0].end - result[1].start).abs() < 0.001);
        assert!((result[1].end - result[2].start).abs() < 0.001);
    }

    #[test]
    fn test_syllabify_word_empty() {
        let result = syllabify_word(&[], 0.0, 1.0, "test", 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_syllabify_words_basic() {
        let words = vec![
            WordTimestamp {
                word: "hello".to_string(),
                start: 0.0,
                end: 0.5,
            },
            WordTimestamp {
                word: "world".to_string(),
                start: 0.5,
                end: 1.0,
            },
        ];
        let result = syllabify_words(&words);
        assert!(!result.is_empty());
        // "hello" = 2 syllables, "world" = 1 syllable
        assert!(result.len() >= 2);
    }

    #[test]
    fn test_syllabify_words_empty() {
        let result = syllabify_words(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_syllabify_word_phonemes_have_timestamps() {
        let phonemes: Vec<String> = vec!["K", "AE1", "T"]
            .into_iter()
            .map(String::from)
            .collect();
        let result = syllabify_word(&phonemes, 1.0, 2.0, "cat", 0);
        assert_eq!(result.len(), 1);
        // Phonemes should have timestamps within the syllable
        assert_eq!(result[0].phonemes.len(), 3);
        assert!(result[0].phonemes[0].start >= 1.0);
        assert!(result[0].phonemes[2].end <= 2.001);
    }
}
