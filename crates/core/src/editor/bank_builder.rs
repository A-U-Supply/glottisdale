//! Build a syllable bank from aligned source audio.

use std::path::PathBuf;

use anyhow::Result;

use super::types::SyllableClip;
use crate::audio::analysis::{find_breaths, find_room_tone};
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

/// Build bank and extract room tone + breath clips from source audio.
///
/// Returns `(bank, room_tone_clips, breath_clips)`.
pub fn build_bank_with_context(
    syllables: &[(Syllable, PathBuf)],
    source_audio: &std::collections::HashMap<PathBuf, (Vec<f64>, u32)>,
) -> Result<(Vec<SyllableClip>, Vec<Vec<f64>>, Vec<Vec<f64>>)> {
    let bank = build_bank_from_syllables(syllables, source_audio)?;

    let mut room_tone_clips = Vec::new();
    let mut breath_clips = Vec::new();

    for (path, (samples, sr)) in source_audio {
        // Extract room tone (quietest region >= 100ms)
        if let Some((start, end)) = find_room_tone(samples, *sr, 100) {
            let start_idx = (start * *sr as f64).round() as usize;
            let end_idx = (end * *sr as f64).round() as usize;
            let end_idx = end_idx.min(samples.len());
            if end_idx > start_idx {
                room_tone_clips.push(samples[start_idx..end_idx].to_vec());
            }
        }

        // Extract breaths from inter-word gaps
        let word_bounds: Vec<(f64, f64)> = syllables
            .iter()
            .filter(|(_, p)| p == path)
            .map(|(syl, _)| (syl.start, syl.end))
            .collect();
        let breath_regions = find_breaths(samples, *sr, &word_bounds, 80, 500);
        for (start, end) in breath_regions {
            let clip = cut_clip(samples, *sr, start, end, 5.0, 3.0);
            if !clip.is_empty() {
                breath_clips.push(clip);
            }
        }
    }

    Ok((bank, room_tone_clips, breath_clips))
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

    #[test]
    fn test_build_bank_with_context_extracts_room_tone() {
        let path = PathBuf::from("test.wav");
        let sr = 16000u32;
        // 2 seconds: first 0.5s very quiet (room tone candidate), then 1.5s signal
        let mut samples = vec![0.001f64; sr as usize / 2]; // 0.5s quiet
        samples.extend(vec![0.5f64; sr as usize * 3 / 2]); // 1.5s signal
        let mut source_audio = HashMap::new();
        source_audio.insert(path.clone(), (samples, sr));

        let syllables = vec![(make_syllable(0.6, 0.9, "hello"), path.clone())];

        let (bank, room_tone, _breaths) =
            build_bank_with_context(&syllables, &source_audio).unwrap();
        assert_eq!(bank.len(), 1);
        assert!(
            !room_tone.is_empty(),
            "should extract room tone from quiet region"
        );
    }

    #[test]
    fn test_build_bank_with_context_empty() {
        let source_audio = HashMap::new();
        let (bank, room_tone, breaths) = build_bank_with_context(&[], &source_audio).unwrap();
        assert!(bank.is_empty());
        assert!(room_tone.is_empty());
        assert!(breaths.is_empty());
    }
}
