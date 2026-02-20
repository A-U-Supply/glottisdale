//! IPA sonority-based syllabifier for BFA phoneme output.
//!
//! Uses BFA's pg16 phoneme group classifications to determine sonority,
//! then applies Maximum Onset Principle to find syllable boundaries.

use crate::types::{Phoneme, Syllable};

/// Map BFA pg16 groups to sonority levels (higher = more sonorous).
pub fn pg16_sonority(pg16_group: &str) -> i32 {
    match pg16_group {
        "voiced_stops" | "voiceless_stops" => 0,
        "affricates" => 1,
        "voiceless_fricatives" | "voiced_fricatives" => 2,
        "nasals" => 3,
        "laterals" | "rhotics" => 4,
        "approximants" | "glides" => 5,
        "central_vowels" | "front_vowels" | "back_vowels" | "diphthongs" | "vowels" => 6,
        "consonants" => 1,
        "silence" => -1,
        _ => 1,
    }
}

fn is_vowel_group(pg16_group: &str) -> bool {
    matches!(
        pg16_group,
        "central_vowels" | "front_vowels" | "back_vowels" | "diphthongs" | "vowels"
    )
}

/// Syllabify IPA phonemes using sonority sequencing + Maximum Onset Principle.
pub fn syllabify_ipa(
    phonemes: &[Phoneme],
    pg16_groups: &[String],
    word: &str,
    word_index: usize,
) -> Result<Vec<Syllable>, String> {
    if phonemes.is_empty() {
        return Ok(vec![]);
    }

    if phonemes.len() != pg16_groups.len() {
        return Err(format!(
            "phonemes ({}) and pg16_groups ({}) must have same length",
            phonemes.len(),
            pg16_groups.len()
        ));
    }

    // Filter out silence phonemes
    let filtered: Vec<(&Phoneme, &str)> = phonemes
        .iter()
        .zip(pg16_groups.iter())
        .filter(|(_, g)| g.as_str() != "silence")
        .map(|(ph, g)| (ph, g.as_str()))
        .collect();

    if filtered.is_empty() {
        return Ok(vec![]);
    }

    let phones: Vec<&Phoneme> = filtered.iter().map(|(ph, _)| *ph).collect();
    let groups: Vec<&str> = filtered.iter().map(|(_, g)| *g).collect();

    // Find vowel nuclei indices
    let nuclei_indices: Vec<usize> = groups
        .iter()
        .enumerate()
        .filter(|(_, g)| is_vowel_group(g))
        .map(|(i, _)| i)
        .collect();

    // No vowels: treat entire sequence as one syllable
    if nuclei_indices.is_empty() {
        return Ok(vec![Syllable {
            phonemes: phones.iter().map(|p| (*p).clone()).collect(),
            start: phones[0].start,
            end: phones.last().unwrap().end,
            word: word.to_string(),
            word_index,
        }]);
    }

    // Find syllable boundaries using Maximum Onset Principle
    let boundaries = find_boundaries(&groups, &nuclei_indices);

    let mut syllables = Vec::new();
    for (start_idx, end_idx) in boundaries {
        let syl_phones: Vec<Phoneme> = phones[start_idx..end_idx]
            .iter()
            .map(|p| (*p).clone())
            .collect();
        if !syl_phones.is_empty() {
            syllables.push(Syllable {
                phonemes: syl_phones.clone(),
                start: syl_phones[0].start,
                end: syl_phones.last().unwrap().end,
                word: word.to_string(),
                word_index,
            });
        }
    }

    Ok(syllables)
}

fn find_boundaries(groups: &[&str], nuclei_indices: &[usize]) -> Vec<(usize, usize)> {
    let n = groups.len();
    let mut boundaries = Vec::new();

    for (syl_i, &nuc) in nuclei_indices.iter().enumerate() {
        let syl_start = if syl_i == 0 {
            0
        } else {
            boundaries.last().map(|&(_, end)| end).unwrap_or(0)
        };

        let syl_end = if syl_i == nuclei_indices.len() - 1 {
            n
        } else {
            let next_nuc = nuclei_indices[syl_i + 1];
            split_cluster(groups, nuc, next_nuc)
        };

        boundaries.push((syl_start, syl_end));
    }

    boundaries
}

fn split_cluster(groups: &[&str], nuc_a: usize, nuc_b: usize) -> usize {
    let cluster_start = nuc_a + 1;
    let cluster_end = nuc_b;

    if cluster_start >= cluster_end {
        return nuc_b;
    }

    // Maximum Onset Principle: try giving all consonants to onset,
    // then peel from left if sonority doesn't rise
    for split in cluster_start..=cluster_end {
        let onset = &groups[split..cluster_end];
        if onset.is_empty() || valid_onset(onset) {
            return split;
        }
    }

    // Fallback: split in the middle
    cluster_start + (cluster_end - cluster_start) / 2
}

fn valid_onset(onset_groups: &[&str]) -> bool {
    if onset_groups.len() <= 1 {
        return true;
    }
    let sonorities: Vec<i32> = onset_groups.iter().map(|g| pg16_sonority(g)).collect();
    for i in 0..sonorities.len() - 1 {
        if sonorities[i] > sonorities[i + 1] {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_phoneme(label: &str, start: f64, end: f64) -> Phoneme {
        Phoneme {
            label: label.to_string(),
            start,
            end,
        }
    }

    #[test]
    fn test_pg16_sonority_ordering() {
        assert!(pg16_sonority("vowels") > pg16_sonority("glides"));
        assert!(pg16_sonority("glides") > pg16_sonority("nasals"));
        assert!(pg16_sonority("nasals") > pg16_sonority("voiceless_fricatives"));
        assert!(pg16_sonority("voiceless_fricatives") > pg16_sonority("voiced_stops"));
        assert_eq!(pg16_sonority("silence"), -1);
    }

    #[test]
    fn test_syllabify_ipa_single_vowel() {
        let phonemes = vec![make_phoneme("k", 0.0, 0.1), make_phoneme("æ", 0.1, 0.2), make_phoneme("t", 0.2, 0.3)];
        let groups = vec!["voiced_stops".to_string(), "front_vowels".to_string(), "voiced_stops".to_string()];
        let result = syllabify_ipa(&phonemes, &groups, "cat", 0).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_syllabify_ipa_two_vowels() {
        let phonemes = vec![
            make_phoneme("k", 0.0, 0.05),
            make_phoneme("æ", 0.05, 0.15),
            make_phoneme("m", 0.15, 0.2),
            make_phoneme("ə", 0.2, 0.3),
            make_phoneme("l", 0.3, 0.35),
        ];
        let groups = vec![
            "voiced_stops".to_string(),
            "front_vowels".to_string(),
            "nasals".to_string(),
            "central_vowels".to_string(),
            "laterals".to_string(),
        ];
        let result = syllabify_ipa(&phonemes, &groups, "camel", 0).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_syllabify_ipa_empty() {
        let result = syllabify_ipa(&[], &[], "test", 0).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_syllabify_ipa_no_vowels() {
        let phonemes = vec![make_phoneme("s", 0.0, 0.1), make_phoneme("t", 0.1, 0.2)];
        let groups = vec!["voiceless_fricatives".to_string(), "voiced_stops".to_string()];
        let result = syllabify_ipa(&phonemes, &groups, "st", 0).unwrap();
        assert_eq!(result.len(), 1); // All consonants as one syllable
    }

    #[test]
    fn test_syllabify_ipa_length_mismatch() {
        let phonemes = vec![make_phoneme("k", 0.0, 0.1)];
        let groups = vec!["voiced_stops".to_string(), "vowels".to_string()];
        let result = syllabify_ipa(&phonemes, &groups, "test", 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_syllabify_ipa_filters_silence() {
        let phonemes = vec![
            make_phoneme("", 0.0, 0.05),
            make_phoneme("k", 0.05, 0.1),
            make_phoneme("æ", 0.1, 0.2),
        ];
        let groups = vec!["silence".to_string(), "voiced_stops".to_string(), "front_vowels".to_string()];
        let result = syllabify_ipa(&phonemes, &groups, "ka", 0).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].phonemes.len(), 2); // silence filtered out
    }
}
