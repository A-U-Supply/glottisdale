//! Match target syllables/phonemes to a source syllable bank.
//!
//! Uses Viterbi DP with a continuity bonus to prefer adjacent source
//! syllables, preserving natural coarticulation.

use serde::Serialize;

use crate::speak::phonetic_distance::{phoneme_distance, syllable_distance};
use crate::speak::syllable_bank::SyllableEntry;

/// Default bonus applied when consecutive target syllables match to adjacent
/// source syllables. A value of 7 means the DP will prefer a contiguous
/// source syllable whose phonetic distance is up to 7 worse than the
/// globally-best non-contiguous alternative.
const CONTINUITY_BONUS: i32 = 7;

/// Result of matching a target syllable/phoneme to a source entry.
#[derive(Debug, Clone, Serialize)]
pub struct MatchResult {
    /// The phonemes that were being matched
    pub target_phonemes: Vec<String>,
    /// The source entry that was chosen
    pub entry: SyllableEntry,
    /// Phonetic distance score
    pub distance: i32,
    /// Position in the target sequence
    pub target_index: usize,
}

impl MatchResult {
    /// Serialize for JSON output.
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "target_index": self.target_index,
            "target": self.target_phonemes,
            "matched": self.entry.phoneme_labels,
            "matched_word": self.entry.word,
            "source_index": self.entry.index,
            "distance": self.distance,
        })
    }
}

/// True if `b` immediately follows `a` in the same source file.
fn are_adjacent(a: &SyllableEntry, b: &SyllableEntry) -> bool {
    a.source_path == b.source_path && b.index == a.index + 1
}

/// Match target syllables to source bank using Viterbi DP.
///
/// Finds the sequence of source syllables that minimises total phonetic
/// distance while rewarding contiguous source runs (adjacent source
/// syllables matched to consecutive target syllables).
pub fn match_syllables(
    target_syllables: &[Vec<String>],
    bank: &[SyllableEntry],
    target_stresses: Option<&[Option<u8>]>,
    continuity_bonus: Option<i32>,
) -> Vec<MatchResult> {
    let n = target_syllables.len();
    let b = bank.len();
    if n == 0 || b == 0 {
        return Vec::new();
    }

    let bonus = continuity_bonus.unwrap_or(CONTINUITY_BONUS);

    // Pre-compute pairwise distances (with small stress penalty for ties)
    let mut dists: Vec<Vec<f64>> = Vec::with_capacity(n);
    for (i, target) in target_syllables.iter().enumerate() {
        let stress = target_stresses.and_then(|ts| ts.get(i).copied().flatten());
        let mut row = Vec::with_capacity(b);
        for entry in bank {
            let d = syllable_distance(target, &entry.phoneme_labels) as f64;
            let penalty = if stress.is_some() && entry.stress != stress {
                0.1
            } else {
                0.0
            };
            row.push(d + penalty);
        }
        dists.push(row);
    }

    // Pre-compute predecessor map: pred[j] = k iff bank[k] → bank[j]
    let mut pred: Vec<Option<usize>> = vec![None; b];
    for j in 0..b {
        for k in 0..b {
            if are_adjacent(&bank[k], &bank[j]) {
                pred[j] = Some(k);
                break;
            }
        }
    }

    // --- Viterbi DP ---
    let inf = f64::INFINITY;

    // dp[j] = min total cost when current target matched to bank[j]
    let mut dp: Vec<f64> = dists[0].clone();
    // parents[i][j] = bank index chosen for target i-1 on the best path to j
    let mut parents: Vec<Vec<usize>> = vec![vec![0; b]]; // placeholder for i=0

    for dist_row in dists.iter().take(n).skip(1) {
        let mut new_dp = vec![inf; b];
        let mut new_parent = vec![0usize; b];

        // Best previous cost across all bank entries (non-contiguous case)
        let (min_k, min_prev) = dp
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        for j in 0..b {
            let cost = dist_row[j];

            // Non-contiguous: best of any previous bank entry
            let mut best = min_prev + cost;
            let mut best_k = min_k;

            // Contiguous: predecessor in the same source
            if let Some(k) = pred[j] {
                let contiguous = dp[k] + cost - bonus as f64;
                if contiguous < best {
                    best = contiguous;
                    best_k = k;
                }
            }

            new_dp[j] = best;
            new_parent[j] = best_k;
        }

        dp = new_dp;
        parents.push(new_parent);
    }

    // --- Backtrace ---
    let best_last = dp
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap()
        .0;

    let mut path = vec![best_last];
    for i in (1..n).rev() {
        path.push(parents[i][*path.last().unwrap()]);
    }
    path.reverse();

    (0..n)
        .map(|i| MatchResult {
            target_phonemes: target_syllables[i].clone(),
            entry: bank[path[i]].clone(),
            distance: dists[i][path[i]] as i32,
            target_index: i,
        })
        .collect()
}

/// Match each target phoneme to the best source phoneme.
///
/// Searches all phonemes across all bank entries to find the closest
/// individual phoneme match.
pub fn match_phonemes(
    target_phonemes: &[String],
    bank: &[SyllableEntry],
) -> Vec<MatchResult> {
    // Flatten bank into (phoneme_label, entry) tuples
    let flat: Vec<(&str, &SyllableEntry)> = bank
        .iter()
        .flat_map(|entry| {
            entry
                .phoneme_labels
                .iter()
                .map(move |label| (label.as_str(), entry))
        })
        .collect();

    target_phonemes
        .iter()
        .enumerate()
        .map(|(i, target_ph)| {
            let mut best_entry: Option<&SyllableEntry> = None;
            let mut best_dist = i32::MAX;

            for (label, entry) in &flat {
                let d = phoneme_distance(target_ph, label);
                if d < best_dist {
                    best_dist = d;
                    best_entry = Some(entry);
                    if d == 0 {
                        break; // exact match
                    }
                }
            }

            MatchResult {
                target_phonemes: vec![target_ph.clone()],
                entry: best_entry.unwrap().clone(),
                distance: best_dist,
                target_index: i,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(
        phonemes: &[&str],
        index: usize,
        source: &str,
        word: &str,
        stress: Option<u8>,
    ) -> SyllableEntry {
        SyllableEntry {
            phoneme_labels: phonemes.iter().map(|s| s.to_string()).collect(),
            start: index as f64 * 0.3,
            end: index as f64 * 0.3 + 0.3,
            word: word.to_string(),
            stress,
            source_path: source.to_string(),
            index,
        }
    }

    #[test]
    fn test_match_exact() {
        let bank = vec![
            make_entry(&["K", "AE1", "T"], 0, "a.wav", "cat", Some(1)),
            make_entry(&["D", "AO1", "G"], 1, "a.wav", "dog", Some(1)),
        ];
        let targets = vec![vec!["K".into(), "AE1".into(), "T".into()]];
        let matches = match_syllables(&targets, &bank, None, None);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entry.word, "cat");
        assert_eq!(matches[0].distance, 0);
    }

    #[test]
    fn test_match_best() {
        let bank = vec![
            make_entry(&["K", "AE1", "T"], 0, "a.wav", "cat", Some(1)),
            make_entry(&["K", "AH0", "T"], 1, "a.wav", "cut", Some(0)),
        ];
        // Target K AE1 T should match "cat" exactly (distance 0)
        let targets = vec![vec!["K".into(), "AE1".into(), "T".into()]];
        let matches = match_syllables(&targets, &bank, None, None);
        assert_eq!(matches[0].entry.word, "cat");
    }

    #[test]
    fn test_match_stress_tiebreak() {
        // Two entries with same phonemes but different stress
        let bank = vec![
            make_entry(&["K", "AE0", "T"], 0, "a.wav", "cat0", Some(0)),
            make_entry(&["K", "AE1", "T"], 1, "a.wav", "cat1", Some(1)),
        ];
        let targets = vec![vec!["K".into(), "AE1".into(), "T".into()]];
        let stresses = vec![Some(1u8)];
        let matches = match_syllables(&targets, &bank, Some(&stresses), None);
        // Should prefer stress=1 match
        assert_eq!(matches[0].entry.stress, Some(1));
    }

    #[test]
    fn test_match_continuity_bonus() {
        // Bank: three entries, first two adjacent in source
        let bank = vec![
            make_entry(&["K", "AE1", "T"], 0, "a.wav", "cat", Some(1)),
            make_entry(&["D", "AO1", "G"], 1, "a.wav", "dog", Some(1)),
            make_entry(&["D", "AO1", "G"], 0, "b.wav", "dog2", Some(1)),
        ];
        // Target: cat then dog
        let targets = vec![
            vec!["K".into(), "AE1".into(), "T".into()],
            vec!["D".into(), "AO1".into(), "G".into()],
        ];
        let matches = match_syllables(&targets, &bank, None, None);
        // Should prefer adjacent pair (cat@0,dog@1 in a.wav)
        assert_eq!(matches[0].entry.source_path, "a.wav");
        assert_eq!(matches[1].entry.source_path, "a.wav");
        assert_eq!(matches[1].entry.index, 1);
    }

    #[test]
    fn test_match_empty_inputs() {
        let bank = vec![make_entry(&["K"], 0, "a.wav", "k", None)];
        assert!(match_syllables(&[], &bank, None, None).is_empty());
        assert!(match_syllables(&[vec!["K".into()]], &[], None, None).is_empty());
    }

    #[test]
    fn test_match_phonemes_basic() {
        let bank = vec![
            make_entry(&["K", "AE1", "T"], 0, "a.wav", "cat", Some(1)),
            make_entry(&["D", "AO1", "G"], 1, "a.wav", "dog", Some(1)),
        ];
        let targets: Vec<String> = vec!["K".into(), "AE1".into()];
        let matches = match_phonemes(&targets, &bank);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].distance, 0); // K exact match
        assert_eq!(matches[1].distance, 0); // AE1 exact match
    }

    #[test]
    fn test_are_adjacent() {
        let a = make_entry(&["K"], 0, "a.wav", "a", None);
        let b = make_entry(&["T"], 1, "a.wav", "b", None);
        let c = make_entry(&["D"], 0, "b.wav", "c", None);
        assert!(are_adjacent(&a, &b));
        assert!(!are_adjacent(&a, &c)); // different source
        assert!(!are_adjacent(&b, &a)); // wrong order
    }
}
