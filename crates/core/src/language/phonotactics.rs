//! Phonotactic scoring for natural-sounding syllable ordering.

use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::collections::HashSet;

use crate::types::Syllable;

lazy_static::lazy_static! {
    /// ARPABET consonant sonority scale.
    static ref ARPABET_SONORITY: std::collections::HashMap<&'static str, i32> = {
        let mut m = std::collections::HashMap::new();
        // 1: Stops
        for p in &["P", "B", "T", "D", "K", "G"] { m.insert(*p, 1); }
        // 2: Affricates
        for p in &["CH", "JH"] { m.insert(*p, 2); }
        // 3: Fricatives
        for p in &["F", "V", "TH", "DH", "S", "Z", "SH", "ZH", "HH"] { m.insert(*p, 3); }
        // 4: Nasals
        for p in &["M", "N", "NG"] { m.insert(*p, 4); }
        // 5: Liquids
        for p in &["L", "R"] { m.insert(*p, 5); }
        // 6: Glides
        for p in &["W", "Y"] { m.insert(*p, 6); }
        m
    };

    /// ARPABET illegal onsets.
    static ref ILLEGAL_ONSETS: HashSet<&'static str> = {
        ["NG", "ZH"].into_iter().collect()
    };

    /// Known ARPABET vowel bases.
    static ref ARPABET_VOWELS: HashSet<&'static str> = {
        [
            "AA", "AE", "AH", "AO", "AW", "AY",
            "EH", "ER", "EY", "IH", "IY",
            "OW", "OY", "UH", "UW",
        ].into_iter().collect()
    };

    /// IPA vowel characters.
    static ref IPA_VOWELS: HashSet<char> = {
        "aeiouɪɛæɑɒɔʊəɜɐʌ".chars().collect()
    };

    /// IPA stops.
    static ref IPA_STOPS: HashSet<char> = {
        "pbtdkgʔ".chars().collect()
    };

    /// IPA nasals.
    static ref IPA_NASALS: HashSet<char> = {
        "mnɲŋɴ".chars().collect()
    };

    /// IPA fricatives.
    static ref IPA_FRICATIVES: HashSet<char> = {
        "fvθðszʃʒçxɣhɦ".chars().collect()
    };

    /// IPA laterals.
    static ref IPA_LATERALS: HashSet<char> = {
        "lɫɬɮ".chars().collect()
    };

    /// IPA rhotics.
    static ref IPA_RHOTICS: HashSet<&'static str> = {
        ["r", "ɹ", "ɾ", "ɽ", "ʁ", "ʀ"].into_iter().collect()
    };

    /// IPA glides.
    static ref IPA_GLIDES: HashSet<&'static str> = {
        ["j", "w", "ɥ"].into_iter().collect()
    };

    /// IPA diphthong starts.
    static ref IPA_DIPHTHONG_STARTS: Vec<&'static str> = {
        vec!["aɪ", "aʊ", "eɪ", "oʊ", "ɔɪ"]
    };

    /// IPA illegal onsets.
    static ref IPA_ILLEGAL_ONSETS: HashSet<&'static str> = {
        ["ŋ"].into_iter().collect()
    };
}

fn is_ipa(label: &str) -> bool {
    if label.is_empty() {
        return false;
    }
    let first = label.chars().next().unwrap();
    first.is_lowercase() || !first.is_ascii()
}

fn ipa_sonority(label: &str) -> i32 {
    if label.is_empty() {
        return 0;
    }
    if IPA_DIPHTHONG_STARTS.iter().any(|d| label.starts_with(d)) {
        return 7;
    }
    let first = label.chars().next().unwrap();
    let stripped = label.trim_end_matches(['ː', 'ˑ']);
    if IPA_VOWELS.contains(&first) || (stripped.len() == 1 && IPA_VOWELS.contains(&stripped.chars().next().unwrap_or(' '))) {
        return 7;
    }
    if IPA_GLIDES.contains(label) || matches!(first, 'j' | 'w' | 'ɥ') {
        return 6;
    }
    if IPA_RHOTICS.contains(label) || matches!(first, 'ɹ' | 'ɾ' | 'r') {
        return 5;
    }
    if IPA_LATERALS.contains(&first) {
        return 5;
    }
    if IPA_NASALS.contains(&first) {
        return 4;
    }
    if IPA_FRICATIVES.contains(&first) {
        return 3;
    }
    if IPA_STOPS.contains(&first) {
        return 1;
    }
    0
}

/// Return sonority value for a phoneme label (ARPABET or IPA).
///
/// Strips stress digits for ARPABET. Returns 0 for unknown.
pub fn sonority(label: &str) -> i32 {
    if is_ipa(label) {
        return ipa_sonority(label);
    }

    // ARPABET path
    let base = label.trim_end_matches(|c: char| c.is_ascii_digit());
    if let Some(&son) = ARPABET_SONORITY.get(base) {
        return son;
    }
    // Check if it's a vowel
    if base != label || ARPABET_VOWELS.contains(base) {
        return 7;
    }
    0
}

/// Score the phonotactic quality of the junction between two syllables.
///
/// Higher scores = more natural-sounding transitions.
pub fn score_junction(syl_a: &Syllable, syl_b: &Syllable) -> i32 {
    if syl_a.phonemes.is_empty() || syl_b.phonemes.is_empty() {
        return 0;
    }

    let last_phone = &syl_a.phonemes.last().unwrap().label;
    let first_phone = &syl_b.phonemes[0].label;

    let mut score = 0;

    // Illegal onset penalty
    let base_first = first_phone.trim_end_matches(|c: char| c.is_ascii_digit());
    if ILLEGAL_ONSETS.contains(base_first) || IPA_ILLEGAL_ONSETS.contains(first_phone.as_str()) {
        score -= 2;
    }

    // Hiatus penalty (vowel-vowel boundary)
    if sonority(last_phone) == 7 && sonority(first_phone) == 7 {
        score -= 1;
    }

    // Sonority contour
    let boundary_sonority = sonority(last_phone) + sonority(first_phone);
    if boundary_sonority <= 8 {
        score += 1;
    } else if boundary_sonority >= 12 {
        score -= 1;
    }

    score
}

/// Reorder syllables to maximize phonotactic junction quality.
///
/// Tries `attempts` random permutations and returns the best-scoring one.
pub fn order_syllables(
    syllables: &[Syllable],
    seed: Option<u64>,
    attempts: usize,
) -> Vec<Syllable> {
    if syllables.len() <= 1 {
        return syllables.to_vec();
    }

    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    let total_score = |ordering: &[Syllable]| -> i32 {
        ordering
            .windows(2)
            .map(|w| score_junction(&w[0], &w[1]))
            .sum()
    };

    let mut best = syllables.to_vec();
    let mut best_score = total_score(&best);

    for _ in 0..attempts {
        let mut candidate = syllables.to_vec();
        candidate.shuffle(&mut rng);
        let s = total_score(&candidate);
        if s > best_score {
            best = candidate;
            best_score = s;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Phoneme;

    fn make_syl(phoneme_labels: &[&str]) -> Syllable {
        Syllable {
            phonemes: phoneme_labels
                .iter()
                .map(|l| Phoneme {
                    label: l.to_string(),
                    start: 0.0,
                    end: 0.1,
                })
                .collect(),
            start: 0.0,
            end: 0.1,
            word: "test".to_string(),
            word_index: 0,
        }
    }

    #[test]
    fn test_sonority_arpabet() {
        assert_eq!(sonority("AE1"), 7); // vowel
        assert_eq!(sonority("K"), 1); // stop
        assert_eq!(sonority("S"), 3); // fricative
        assert_eq!(sonority("N"), 4); // nasal
        assert_eq!(sonority("L"), 5); // liquid
        assert_eq!(sonority("W"), 6); // glide
    }

    #[test]
    fn test_sonority_ipa() {
        assert_eq!(sonority("a"), 7); // vowel
        assert_eq!(sonority("p"), 1); // stop
        assert_eq!(sonority("s"), 3); // fricative
        assert_eq!(sonority("n"), 4); // nasal
    }

    #[test]
    fn test_score_junction_consonant_consonant() {
        let a = make_syl(&["K"]);
        let b = make_syl(&["T"]);
        let score = score_junction(&a, &b);
        assert!(score >= 0); // consonant-consonant is ok
    }

    #[test]
    fn test_score_junction_vowel_vowel() {
        let a = make_syl(&["AE1"]);
        let b = make_syl(&["IY1"]);
        let score = score_junction(&a, &b);
        assert!(score < 0); // hiatus penalty
    }

    #[test]
    fn test_score_junction_illegal_onset() {
        let a = make_syl(&["AE1"]);
        let b = make_syl(&["NG"]);
        let score = score_junction(&a, &b);
        assert!(score < 0); // NG can't start a syllable
    }

    #[test]
    fn test_order_syllables_deterministic() {
        let syls = vec![
            make_syl(&["K", "AE1"]),
            make_syl(&["T", "IY1"]),
            make_syl(&["S", "AH0"]),
        ];
        let a = order_syllables(&syls, Some(42), 10);
        let b = order_syllables(&syls, Some(42), 10);
        // Same seed should give same result
        assert_eq!(a.len(), b.len());
        for (sa, sb) in a.iter().zip(b.iter()) {
            assert_eq!(sa.phonemes.len(), sb.phonemes.len());
        }
    }

    #[test]
    fn test_order_syllables_single() {
        let syls = vec![make_syl(&["K", "AE1"])];
        let result = order_syllables(&syls, None, 10);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_order_syllables_empty() {
        let result = order_syllables(&[], None, 10);
        assert!(result.is_empty());
    }
}
