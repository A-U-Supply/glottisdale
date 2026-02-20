//! Prepare syllable clips from audio sources: cut, normalize pitch/volume.

use crate::audio::analysis::{compute_rms, estimate_f0};
use crate::audio::effects::{adjust_volume, cut_clip, pitch_shift};
use crate::types::Syllable;

/// A pitch- and volume-normalized syllable clip (in-memory).
#[derive(Debug, Clone)]
pub struct NormalizedSyllable {
    /// Audio samples
    pub samples: Vec<f64>,
    /// Sample rate
    pub sr: u32,
    /// Estimated F0 in Hz, or None if unvoiced
    pub f0: Option<f64>,
    /// Duration in seconds
    pub duration: f64,
    /// ARPABET phoneme labels
    pub phonemes: Vec<String>,
    /// Parent word text
    pub word: String,
}

/// Compute semitone shifts to normalize all F0s to the median.
///
/// Returns a list of shifts in semitones (same length as input).
/// None values get 0 shift.
pub fn compute_pitch_shifts(f0_values: &[Option<f64>]) -> Vec<f64> {
    let voiced: Vec<f64> = f0_values
        .iter()
        .filter_map(|f| f.filter(|&f| f > 0.0))
        .collect();

    if voiced.is_empty() {
        return vec![0.0; f0_values.len()];
    }

    let mut sorted = voiced.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let target = sorted[sorted.len() / 2];

    f0_values
        .iter()
        .map(|f0| match f0 {
            Some(f) if *f > 0.0 => 12.0 * (target / f).log2(),
            _ => 0.0,
        })
        .collect()
}

/// Prepare syllable clips from aligned syllables and source audio.
///
/// Cuts each syllable, estimates F0, then normalizes pitch and volume
/// across all clips.
pub fn prepare_syllables(
    syllables: &[Syllable],
    source_samples: &[f64],
    sr: u32,
    max_semitone_shift: f64,
) -> Vec<NormalizedSyllable> {
    if syllables.is_empty() {
        return Vec::new();
    }

    // Cut and analyze each syllable
    let mut all_syls: Vec<NormalizedSyllable> = Vec::new();

    for syl in syllables {
        let clip = cut_clip(source_samples, sr, syl.start, syl.end, 25.0, 0.0);
        if clip.is_empty() {
            continue;
        }

        let f0 = estimate_f0(&clip, sr, 80, 600);
        let duration = clip.len() as f64 / sr as f64;
        let phoneme_labels: Vec<String> = syl.phonemes.iter().map(|p| p.label.clone()).collect();

        all_syls.push(NormalizedSyllable {
            samples: clip,
            sr,
            f0,
            duration,
            phonemes: phoneme_labels,
            word: syl.word.clone(),
        });
    }

    if all_syls.is_empty() {
        return all_syls;
    }

    // Normalize pitch to median F0
    let f0_values: Vec<Option<f64>> = all_syls.iter().map(|s| s.f0).collect();
    let shifts = compute_pitch_shifts(&f0_values);

    for (syl, shift) in all_syls.iter_mut().zip(shifts.iter()) {
        if shift.abs() < 0.1 {
            continue;
        }
        let clamped = shift.clamp(-max_semitone_shift, max_semitone_shift);
        if let Ok(shifted) = pitch_shift(&syl.samples, syl.sr, clamped) {
            syl.samples = shifted;
            syl.duration = syl.samples.len() as f64 / syl.sr as f64;
        }
    }

    // Volume normalize to median RMS
    let rms_values: Vec<f64> = all_syls.iter().map(|s| compute_rms(&s.samples)).collect();
    let voiced_rms: Vec<f64> = rms_values.iter().filter(|&&r| r > 0.0).copied().collect();

    if !voiced_rms.is_empty() {
        let mut sorted_rms = voiced_rms;
        sorted_rms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let target_rms = sorted_rms[sorted_rms.len() / 2];

        for (syl, &rms) in all_syls.iter_mut().zip(rms_values.iter()) {
            if rms <= 0.0 {
                continue;
            }
            let db_adjust = 20.0 * (target_rms / rms).log10();
            let db_adjust = db_adjust.clamp(-20.0, 20.0);
            if db_adjust.abs() >= 0.5 {
                adjust_volume(&mut syl.samples, db_adjust);
                syl.duration = syl.samples.len() as f64 / syl.sr as f64;
            }
        }
    }

    all_syls
}

/// Get median F0 from a list of normalized syllables.
pub fn median_f0(syllables: &[NormalizedSyllable]) -> Option<f64> {
    let mut voiced: Vec<f64> = syllables
        .iter()
        .filter_map(|s| s.f0.filter(|&f| f > 0.0))
        .collect();

    if voiced.is_empty() {
        return None;
    }

    voiced.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(voiced[voiced.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_pitch_shifts_empty() {
        let shifts = compute_pitch_shifts(&[]);
        assert!(shifts.is_empty());
    }

    #[test]
    fn test_compute_pitch_shifts_all_none() {
        let shifts = compute_pitch_shifts(&[None, None, None]);
        assert_eq!(shifts, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_compute_pitch_shifts_identical() {
        let shifts = compute_pitch_shifts(&[Some(220.0), Some(220.0), Some(220.0)]);
        assert!(shifts.iter().all(|&s| s.abs() < 0.01));
    }

    #[test]
    fn test_compute_pitch_shifts_octave() {
        // 440 Hz is one octave above 220 Hz = 12 semitones
        let shifts = compute_pitch_shifts(&[Some(220.0), Some(440.0)]);
        // Median = 220 or 440 depending on choice; both F0s should shift toward median
        assert!(shifts[0].abs() + shifts[1].abs() > 0.0);
    }

    #[test]
    fn test_compute_pitch_shifts_with_none() {
        let shifts = compute_pitch_shifts(&[Some(220.0), None, Some(220.0)]);
        assert!(shifts[0].abs() < 0.01);
        assert_eq!(shifts[1], 0.0);
        assert!(shifts[2].abs() < 0.01);
    }

    #[test]
    fn test_median_f0_empty() {
        assert!(median_f0(&[]).is_none());
    }

    #[test]
    fn test_median_f0_basic() {
        let syls = vec![
            NormalizedSyllable {
                samples: vec![],
                sr: 16000,
                f0: Some(200.0),
                duration: 0.3,
                phonemes: vec![],
                word: "a".to_string(),
            },
            NormalizedSyllable {
                samples: vec![],
                sr: 16000,
                f0: Some(220.0),
                duration: 0.3,
                phonemes: vec![],
                word: "b".to_string(),
            },
            NormalizedSyllable {
                samples: vec![],
                sr: 16000,
                f0: None,
                duration: 0.3,
                phonemes: vec![],
                word: "c".to_string(),
            },
        ];
        let median = median_f0(&syls);
        assert!(median.is_some());
    }
}
