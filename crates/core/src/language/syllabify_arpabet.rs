//! ARPABET syllabifier using Maximum Onset Principle.
//!
//! Vendored port of Kyle Gorman's syllabify (MIT License).
//! See: <https://github.com/kylebgorman/syllabify>

use std::collections::HashSet;

/// A syllable decomposed into onset, nucleus, and coda.
pub type SyllableParts = (Vec<String>, Vec<String>, Vec<String>);

lazy_static::lazy_static! {
    static ref SLAX: HashSet<&'static str> = {
        [
            "IH1", "IH2", "EH1", "EH2", "AE1", "AE2",
            "AH1", "AH2", "UH1", "UH2",
        ].into_iter().collect()
    };

    static ref VOWELS: HashSet<&'static str> = {
        let mut v: HashSet<&str> = [
            "IY1", "IY2", "IY0", "EY1", "EY2", "EY0",
            "AA1", "AA2", "AA0", "ER1", "ER2", "ER0",
            "AW1", "AW2", "AW0", "AO1", "AO2", "AO0",
            "AY1", "AY2", "AY0", "OW1", "OW2", "OW0",
            "OY1", "OY2", "OY0", "IH0", "EH0", "AE0",
            "AH0", "UH0", "UW1", "UW2", "UW0", "UW",
            "IY", "EY", "AA", "ER", "AW", "AO", "AY",
            "OW", "OY", "UH", "IH", "EH", "AE", "AH",
        ].into_iter().collect();
        for s in SLAX.iter() {
            v.insert(s);
        }
        v
    };

    /// Licit 2-consonant onsets.
    static ref O2: HashSet<(&'static str, &'static str)> = {
        [
            ("P", "R"), ("T", "R"), ("K", "R"), ("B", "R"), ("D", "R"),
            ("G", "R"), ("F", "R"), ("TH", "R"),
            ("P", "L"), ("K", "L"), ("B", "L"), ("G", "L"),
            ("F", "L"), ("S", "L"),
            ("K", "W"), ("G", "W"), ("S", "W"),
            ("S", "P"), ("S", "T"), ("S", "K"),
            ("HH", "Y"),
            ("R", "W"),
        ].into_iter().collect()
    };

    /// Licit 3-consonant onsets.
    static ref O3: HashSet<(&'static str, &'static str, &'static str)> = {
        [
            ("S", "T", "R"), ("S", "K", "L"), ("T", "R", "W"),
        ].into_iter().collect()
    };
}

fn is_vowel(seg: &str) -> bool {
    VOWELS.contains(seg)
}

fn is_slax(seg: &str) -> bool {
    SLAX.contains(seg)
}

/// Syllabify a CMU dictionary (ARPABET) pronunciation.
///
/// Returns a list of (onset, nucleus, coda) tuples.
/// The `alaska_rule` controls whether /s/ is pulled into the coda before lax vowels.
pub fn syllabify(pron: &[String], alaska_rule: bool) -> Result<Vec<SyllableParts>, String> {
    if pron.is_empty() {
        return Ok(vec![]);
    }

    let mypron: Vec<String> = pron.to_vec();

    // Find nuclei and interludes
    let mut nuclei: Vec<Vec<String>> = Vec::new();
    let mut onsets: Vec<Vec<String>> = Vec::new();
    let mut last_vowel_idx: isize = -1;

    for (j, seg) in mypron.iter().enumerate() {
        if is_vowel(seg) {
            nuclei.push(vec![seg.clone()]);
            // Interlude = everything between last vowel and this one
            let start = (last_vowel_idx + 1) as usize;
            onsets.push(mypron[start..j].to_vec());
            last_vowel_idx = j as isize;
        }
    }

    if nuclei.is_empty() {
        return Err(format!("no vowels found in {:?}", mypron));
    }

    // Coda = everything after the last vowel
    let codas_final = mypron[(last_vowel_idx + 1) as usize..].to_vec();
    let mut codas: Vec<Vec<String>> = Vec::new();

    // Resolve disputes and compute codas for inter-syllable boundaries
    for i in 1..onsets.len() {
        let mut coda: Vec<String> = Vec::new();

        // R-coloring: if onset starts with R and has >1 consonant, pull R into previous nucleus
        if onsets[i].len() > 1 && onsets[i][0] == "R" {
            let r = onsets[i].remove(0);
            nuclei[i - 1].push(r);
        }

        // Y-gliding: if onset ends with Y and has >2 consonants, push Y into next nucleus
        if onsets[i].len() > 2 && onsets[i].last().map_or(false, |s| s == "Y") {
            let y = onsets[i].pop().unwrap();
            nuclei[i].insert(0, y);
        }

        // Alaska rule: /s/ before lax vowels goes to coda
        if onsets[i].len() > 1
            && alaska_rule
            && nuclei[i - 1].last().map_or(false, |s| is_slax(s))
            && onsets[i][0] == "S"
        {
            coda.push(onsets[i].remove(0));
        }

        // Onset maximization
        let mut depth = 1;
        if onsets[i].len() > 1 {
            let last_two = (
                onsets[i][onsets[i].len() - 2].as_str(),
                onsets[i][onsets[i].len() - 1].as_str(),
            );
            if O2.contains(&last_two) {
                depth = if onsets[i].len() >= 3 {
                    let last_three = (
                        onsets[i][onsets[i].len() - 3].as_str(),
                        onsets[i][onsets[i].len() - 2].as_str(),
                        onsets[i][onsets[i].len() - 1].as_str(),
                    );
                    if O3.contains(&last_three) { 3 } else { 2 }
                } else {
                    2
                };
            }
        }

        let drain_count = onsets[i].len().saturating_sub(depth);
        for _ in 0..drain_count {
            coda.push(onsets[i].remove(0));
        }

        codas.push(coda);
    }

    // Add the final coda
    codas.push(codas_final);

    // Build output
    let output: Vec<SyllableParts> = onsets
        .into_iter()
        .zip(nuclei.into_iter())
        .zip(codas.into_iter())
        .map(|((o, n), c)| (o, n, c))
        .collect();

    // Verify all segments are accounted for
    let flat: Vec<&str> = output
        .iter()
        .flat_map(|(o, n, c)| {
            o.iter()
                .chain(n.iter())
                .chain(c.iter())
                .map(|s| s.as_str())
        })
        .collect();
    let original: Vec<&str> = mypron.iter().map(|s| s.as_str()).collect();
    if flat != original {
        return Err(format!(
            "could not syllabify {:?}, got {:?}",
            mypron, flat
        ));
    }

    Ok(output)
}

/// Remove stress markers from a syllabification.
pub fn destress(syllab: &[SyllableParts]) -> Vec<SyllableParts> {
    syllab
        .iter()
        .map(|(onset, nucleus, coda)| {
            let nuke: Vec<String> = nucleus
                .iter()
                .map(|p| {
                    if p.ends_with('0') || p.ends_with('1') || p.ends_with('2') {
                        p[..p.len() - 1].to_string()
                    } else {
                        p.clone()
                    }
                })
                .collect();
            (onset.clone(), nuke, coda.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(words: &str) -> Vec<String> {
        words.split_whitespace().map(|w| w.to_string()).collect()
    }

    #[test]
    fn test_syllabify_cat() {
        // CAT: K AE1 T -> 1 syllable
        let pron = s("K AE1 T");
        let result = syllabify(&pron, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, s("K")); // onset
        assert_eq!(result[0].1, s("AE1")); // nucleus
        assert_eq!(result[0].2, s("T")); // coda
    }

    #[test]
    fn test_syllabify_camel() {
        // CAMEL: K AE1 M AH0 L -> 2 syllables
        let pron = s("K AE1 M AH0 L");
        let result = syllabify(&pron, true).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_syllabify_banana() {
        // BANANA: B AH0 N AE1 N AH0 -> 3 syllables
        let pron = s("B AH0 N AE1 N AH0");
        let result = syllabify(&pron, true).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_syllabify_street() {
        // STREET: S T R IY1 T -> 1 syllable with cluster onset
        let pron = s("S T R IY1 T");
        let result = syllabify(&pron, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, s("S T R")); // 3-consonant onset
    }

    #[test]
    fn test_syllabify_empty() {
        let result = syllabify(&[], true).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_syllabify_no_vowels() {
        let pron = s("S T R");
        let result = syllabify(&pron, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_destress() {
        let pron = s("K AE1 M AH0 L");
        let result = syllabify(&pron, true).unwrap();
        let destressed = destress(&result);
        // Check that stress markers are removed from nuclei
        for (_, nucleus, _) in &destressed {
            for p in nucleus {
                assert!(!p.ends_with('0') && !p.ends_with('1') && !p.ends_with('2'));
            }
        }
    }

    #[test]
    fn test_syllabify_construct() {
        // CONSTRUCT: K AH0 N S T R AH1 K T -> 2 syllables
        let pron = s("K AH0 N S T R AH1 K T");
        let result = syllabify(&pron, true).unwrap();
        assert_eq!(result.len(), 2);
    }
}
