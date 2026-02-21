//! Build a syllable bank from aligned source audio.

use std::path::PathBuf;

use anyhow::Result;

use super::types::SyllableClip;
use crate::audio::effects::cut_clip;
use crate::types::Syllable;

/// Build SyllableClips from aligned syllables and their source audio.
///
/// For each syllable, cuts the audio with 25ms padding and 5ms fade,
/// computes waveform data, and creates a SyllableClip.
pub fn build_bank_from_syllables(
    syllables: &[(Syllable, PathBuf)],
    source_audio: &std::collections::HashMap<PathBuf, (Vec<f64>, u32)>,
) -> Result<Vec<SyllableClip>> {
    let mut bank = Vec::with_capacity(syllables.len());

    for (syllable, source_path) in syllables {
        let (samples, sr) = source_audio
            .get(source_path)
            .ok_or_else(|| anyhow::anyhow!("Source audio not found: {}", source_path.display()))?;

        let clip_samples = cut_clip(samples, *sr, syllable.start, syllable.end, 25.0, 5.0);

        if clip_samples.is_empty() {
            continue;
        }

        bank.push(SyllableClip::new(
            syllable.clone(),
            clip_samples,
            *sr,
            source_path.clone(),
        ));
    }

    Ok(bank)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Phoneme;
    use std::collections::HashMap;

    fn make_syllable(start: f64, end: f64, word: &str) -> Syllable {
        Syllable {
            phonemes: vec![Phoneme {
                label: "AH0".into(),
                start,
                end,
            }],
            start,
            end,
            word: word.into(),
            word_index: 0,
        }
    }

    #[test]
    fn test_build_bank_basic() {
        let path = PathBuf::from("test.wav");
        let samples = vec![0.5f64; 16000]; // 1 second
        let mut source_audio = HashMap::new();
        source_audio.insert(path.clone(), (samples, 16000u32));

        let syllables = vec![
            (make_syllable(0.0, 0.3, "hello"), path.clone()),
            (make_syllable(0.3, 0.5, "world"), path.clone()),
        ];

        let bank = build_bank_from_syllables(&syllables, &source_audio).unwrap();
        assert_eq!(bank.len(), 2);
        assert!(!bank[0].samples.is_empty());
        assert!(!bank[1].samples.is_empty());
        assert!(!bank[0].waveform.peaks.is_empty());
    }

    #[test]
    fn test_build_bank_empty() {
        let source_audio = HashMap::new();
        let bank = build_bank_from_syllables(&[], &source_audio).unwrap();
        assert!(bank.is_empty());
    }
}
