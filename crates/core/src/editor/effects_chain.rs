//! Non-destructive effects processing for timeline clips.

use anyhow::Result;
use super::types::ClipEffect;

/// Apply a stack of effects to audio samples.
///
/// Effects are applied in order. Each effect transforms the samples
/// produced by the previous one.
pub fn apply_effects(
    source_samples: &[f64],
    sr: u32,
    effects: &[ClipEffect],
) -> Result<Vec<f64>> {
    let mut samples = source_samples.to_vec();

    for effect in effects {
        match effect {
            ClipEffect::Stutter { count } => {
                let original = samples.clone();
                let crossfade = (5.0 / 1000.0 * sr as f64).round() as usize;
                for _ in 0..*count {
                    samples = crate::audio::effects::concatenate(
                        &[samples, original.clone()],
                        crossfade,
                    );
                }
            }
            ClipEffect::TimeStretch { factor } => {
                samples = crate::audio::effects::time_stretch(&samples, sr, *factor)?;
            }
            ClipEffect::PitchShift { semitones } => {
                samples = crate::audio::effects::pitch_shift(&samples, sr, *semitones)?;
            }
        }
    }

    Ok(samples)
}

/// Compute effective duration after effects, without materializing samples.
pub fn compute_effective_duration(base_duration_s: f64, effects: &[ClipEffect]) -> f64 {
    let mut dur = base_duration_s;
    for effect in effects {
        match effect {
            ClipEffect::Stutter { count } => {
                dur *= (1 + count) as f64;
            }
            ClipEffect::TimeStretch { factor } => {
                dur *= factor;
            }
            ClipEffect::PitchShift { .. } => {
                // Pitch shift preserves duration
            }
        }
    }
    dur
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_samples(duration_s: f64, sr: u32) -> Vec<f64> {
        let n = (duration_s * sr as f64).round() as usize;
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin())
            .collect()
    }

    #[test]
    fn test_no_effects() {
        let samples = sine_samples(0.5, 16000);
        let result = apply_effects(&samples, 16000, &[]).unwrap();
        assert_eq!(result.len(), samples.len());
    }

    #[test]
    fn test_stutter_doubles_length() {
        let samples = sine_samples(0.5, 16000);
        let original_len = samples.len();
        let result = apply_effects(
            &samples,
            16000,
            &[ClipEffect::Stutter { count: 1 }],
        )
        .unwrap();
        // stutter count=1 means 1 extra copy = ~2x length (minus crossfade)
        let ratio = result.len() as f64 / original_len as f64;
        assert!(ratio > 1.8 && ratio < 2.2, "ratio={}", ratio);
    }

    #[test]
    fn test_stutter_triples() {
        let samples = sine_samples(0.5, 16000);
        let original_len = samples.len();
        let result = apply_effects(
            &samples,
            16000,
            &[ClipEffect::Stutter { count: 2 }],
        )
        .unwrap();
        let ratio = result.len() as f64 / original_len as f64;
        assert!(ratio > 2.7 && ratio < 3.3, "ratio={}", ratio);
    }

    #[test]
    fn test_time_stretch_double() {
        let samples = sine_samples(0.5, 16000);
        let original_len = samples.len();
        let result = apply_effects(
            &samples,
            16000,
            &[ClipEffect::TimeStretch { factor: 2.0 }],
        )
        .unwrap();
        let ratio = result.len() as f64 / original_len as f64;
        assert!(ratio > 1.8 && ratio < 2.2, "ratio={}", ratio);
    }

    #[test]
    fn test_pitch_shift_preserves_length() {
        let samples = sine_samples(0.5, 16000);
        let original_len = samples.len();
        let result = apply_effects(
            &samples,
            16000,
            &[ClipEffect::PitchShift { semitones: 5.0 }],
        )
        .unwrap();
        let ratio = result.len() as f64 / original_len as f64;
        assert!(
            ratio > 0.95 && ratio < 1.05,
            "pitch shift changed length: ratio={}",
            ratio
        );
    }

    #[test]
    fn test_stacked_effects() {
        let samples = sine_samples(0.5, 16000);
        let original_len = samples.len();
        let result = apply_effects(
            &samples,
            16000,
            &[
                ClipEffect::Stutter { count: 1 },       // ~2x
                ClipEffect::TimeStretch { factor: 2.0 }, // ~2x again
            ],
        )
        .unwrap();
        let ratio = result.len() as f64 / original_len as f64;
        assert!(ratio > 3.5 && ratio < 4.5, "ratio={}", ratio);
    }

    #[test]
    fn test_compute_duration_no_effects() {
        assert!((compute_effective_duration(1.0, &[]) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_duration_stutter() {
        let dur = compute_effective_duration(1.0, &[ClipEffect::Stutter { count: 2 }]);
        assert!((dur - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_duration_stretch() {
        let dur = compute_effective_duration(1.0, &[ClipEffect::TimeStretch { factor: 0.5 }]);
        assert!((dur - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_compute_duration_pitch_shift() {
        let dur = compute_effective_duration(1.0, &[ClipEffect::PitchShift { semitones: 7.0 }]);
        assert!((dur - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_duration_stacked() {
        let dur = compute_effective_duration(
            0.5,
            &[
                ClipEffect::Stutter { count: 1 },       // 0.5 * 2 = 1.0
                ClipEffect::TimeStretch { factor: 3.0 }, // 1.0 * 3 = 3.0
            ],
        );
        assert!((dur - 3.0).abs() < 0.001);
    }
}
