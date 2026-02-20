//! Build an indexed bank of source syllables for matching.

use serde::Serialize;

use crate::speak::phonetic_distance::normalize_phoneme;
use crate::types::Syllable;

/// A source syllable with metadata for matching.
#[derive(Debug, Clone, Serialize)]
pub struct SyllableEntry {
    /// ARPABET labels (with stress markers)
    pub phoneme_labels: Vec<String>,
    /// Start time in source audio (seconds)
    pub start: f64,
    /// End time in source audio (seconds)
    pub end: f64,
    /// Parent word text
    pub word: String,
    /// Stress level (0, 1, 2) or None
    pub stress: Option<u8>,
    /// Path to source audio file
    pub source_path: String,
    /// Position in the original syllable list
    pub index: usize,
}

impl SyllableEntry {
    /// Duration in seconds.
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    /// Serialize for JSON output.
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "phonemes": self.phoneme_labels,
            "start": (self.start * 10000.0).round() / 10000.0,
            "end": (self.end * 10000.0).round() / 10000.0,
            "duration": (self.duration() * 10000.0).round() / 10000.0,
            "word": self.word,
            "stress": self.stress,
            "source": self.source_path,
            "index": self.index,
        })
    }
}

/// Extract stress level from ARPABET vowel phonemes.
fn extract_stress(phoneme_labels: &[String]) -> Option<u8> {
    for label in phoneme_labels {
        if let Some(last) = label.as_bytes().last() {
            if last.is_ascii_digit() {
                return Some(last - b'0');
            }
        }
    }
    None
}

/// Return true if label is a real phoneme (not punctuation or empty).
fn is_phoneme(label: &str) -> bool {
    !label.is_empty() && label.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false)
}

/// Build a syllable bank from aligned source syllables.
///
/// Filters out punctuation labels from phoneme lists and skips
/// syllables that have no real phonemes after filtering.
pub fn build_bank(syllables: &[Syllable], source_path: &str) -> Vec<SyllableEntry> {
    let mut entries = Vec::new();
    for (i, syl) in syllables.iter().enumerate() {
        let labels: Vec<String> = syl
            .phonemes
            .iter()
            .filter(|p| is_phoneme(&p.label))
            .map(|p| normalize_phoneme(&p.label))
            .collect();

        if labels.is_empty() {
            continue;
        }

        entries.push(SyllableEntry {
            stress: extract_stress(&labels),
            phoneme_labels: labels,
            start: syl.start,
            end: syl.end,
            word: syl.word.clone(),
            source_path: source_path.to_string(),
            index: i,
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Phoneme;

    fn make_syl(phonemes: &[(&str, f64, f64)], start: f64, end: f64, word: &str) -> Syllable {
        Syllable {
            phonemes: phonemes
                .iter()
                .map(|(l, s, e)| Phoneme {
                    label: l.to_string(),
                    start: *s,
                    end: *e,
                })
                .collect(),
            start,
            end,
            word: word.to_string(),
            word_index: 0,
        }
    }

    #[test]
    fn test_build_bank_basic() {
        let syls = vec![make_syl(
            &[("K", 0.0, 0.1), ("AE1", 0.1, 0.3), ("T", 0.3, 0.4)],
            0.0,
            0.4,
            "cat",
        )];
        let bank = build_bank(&syls, "test.wav");
        assert_eq!(bank.len(), 1);
        assert_eq!(bank[0].phoneme_labels, vec!["K", "AE1", "T"]);
        assert_eq!(bank[0].word, "cat");
        assert_eq!(bank[0].start, 0.0);
        assert_eq!(bank[0].end, 0.4);
    }

    #[test]
    fn test_build_bank_stress() {
        let syls = vec![make_syl(
            &[("B", 0.0, 0.1), ("AE1", 0.1, 0.3), ("T", 0.3, 0.4)],
            0.0,
            0.4,
            "bat",
        )];
        let bank = build_bank(&syls, "test.wav");
        assert_eq!(bank[0].stress, Some(1));
    }

    #[test]
    fn test_build_bank_ipa_normalization() {
        let syls = vec![make_syl(
            &[("k", 0.0, 0.1), ("æ", 0.1, 0.3)],
            0.0,
            0.3,
            "ca",
        )];
        let bank = build_bank(&syls, "test.wav");
        assert_eq!(bank[0].phoneme_labels, vec!["K", "AE"]);
    }

    #[test]
    fn test_build_bank_filters_punctuation() {
        let syls = vec![make_syl(
            &[(".", 0.0, 0.05), ("K", 0.05, 0.15)],
            0.0,
            0.15,
            "k",
        )];
        let bank = build_bank(&syls, "test.wav");
        assert_eq!(bank[0].phoneme_labels, vec!["K"]);
    }

    #[test]
    fn test_build_bank_skips_empty() {
        let syls = vec![make_syl(
            &[(".", 0.0, 0.05), (",", 0.05, 0.1)],
            0.0,
            0.1,
            "punct",
        )];
        let bank = build_bank(&syls, "test.wav");
        assert!(bank.is_empty());
    }

    #[test]
    fn test_build_bank_index() {
        let syls = vec![
            make_syl(&[("K", 0.0, 0.1)], 0.0, 0.1, "a"),
            make_syl(&[(".", 0.1, 0.15)], 0.1, 0.15, "punct"),
            make_syl(&[("T", 0.15, 0.25)], 0.15, 0.25, "b"),
        ];
        let bank = build_bank(&syls, "test.wav");
        assert_eq!(bank.len(), 2);
        assert_eq!(bank[0].index, 0);
        assert_eq!(bank[1].index, 2); // skipped index 1 (punctuation)
    }

    #[test]
    fn test_syllable_entry_duration() {
        let entry = SyllableEntry {
            phoneme_labels: vec!["K".to_string()],
            start: 1.0,
            end: 1.5,
            word: "test".to_string(),
            stress: None,
            source_path: "test.wav".to_string(),
            index: 0,
        };
        assert!((entry.duration() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_to_json_value() {
        let entry = SyllableEntry {
            phoneme_labels: vec!["K".to_string(), "AE".to_string()],
            start: 0.1234,
            end: 0.5678,
            word: "cat".to_string(),
            stress: Some(1),
            source_path: "test.wav".to_string(),
            index: 3,
        };
        let v = entry.to_json_value();
        assert_eq!(v["word"], "cat");
        assert_eq!(v["index"], 3);
        assert_eq!(v["stress"], 1);
    }
}
