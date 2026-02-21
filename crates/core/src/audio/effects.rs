//! Audio effects: cut, crossfade, concatenation, pitch shift, time stretch,
//! volume adjustment, mixing.

use anyhow::Result;

/// Cut an audio segment with padding and fade.
///
/// `start` and `end` are in seconds. Padding extends the clip on both sides.
/// Fade applies half-sine in/out at the edges.
pub fn cut_clip(
    samples: &[f64],
    sr: u32,
    start: f64,
    end: f64,
    padding_ms: f64,
    fade_ms: f64,
) -> Vec<f64> {
    let file_duration = samples.len() as f64 / sr as f64;
    let padding_s = padding_ms / 1000.0;
    let fade_s = fade_ms / 1000.0;

    let actual_start = (start - padding_s).max(0.0);
    let actual_end = (end + padding_s).min(file_duration);

    let start_idx = (actual_start * sr as f64).round() as usize;
    let end_idx = (actual_end * sr as f64).round() as usize;

    let start_idx = start_idx.min(samples.len());
    let end_idx = end_idx.min(samples.len());

    if start_idx >= end_idx {
        return vec![];
    }

    let mut clip: Vec<f64> = samples[start_idx..end_idx].to_vec();
    let duration = clip.len() as f64 / sr as f64;

    // Apply half-sine fades
    if fade_s > 0.0 && duration > fade_s * 2.0 {
        let fade_samples = (fade_s * sr as f64).round() as usize;

        // Fade in
        for i in 0..fade_samples.min(clip.len()) {
            let t = i as f64 / fade_samples as f64;
            clip[i] *= (t * std::f64::consts::FRAC_PI_2).sin();
        }

        // Fade out
        let out_start = clip.len().saturating_sub(fade_samples);
        let fade_len = clip.len() - out_start;
        for i in 0..fade_len {
            let t = i as f64 / fade_len as f64;
            clip[out_start + i] *= ((1.0 - t) * std::f64::consts::FRAC_PI_2).sin();
        }
    }

    clip
}

/// Generate silence of given duration.
pub fn generate_silence(duration_ms: f64, sr: u32) -> Vec<f64> {
    let n_samples = (duration_ms / 1000.0 * sr as f64).round() as usize;
    vec![0.0; n_samples]
}

/// Concatenate audio segments with optional crossfade.
///
/// `crossfade_samples` = number of samples to overlap between adjacent clips.
/// Uses linear crossfade.
pub fn concatenate(clips: &[Vec<f64>], crossfade_samples: usize) -> Vec<f64> {
    if clips.is_empty() {
        return vec![];
    }
    if clips.len() == 1 {
        return clips[0].clone();
    }

    if crossfade_samples == 0 {
        // Simple concatenation
        let total: usize = clips.iter().map(|c| c.len()).sum();
        let mut result = Vec::with_capacity(total);
        for clip in clips {
            result.extend_from_slice(clip);
        }
        return result;
    }

    // Crossfade concatenation
    let mut result = clips[0].clone();

    for clip in &clips[1..] {
        let cf = crossfade_samples.min(result.len()).min(clip.len());

        if cf == 0 {
            result.extend_from_slice(clip);
            continue;
        }

        // Crossfade region: fade out result tail, fade in clip head
        let result_start = result.len() - cf;
        for i in 0..cf {
            let t = i as f64 / cf as f64;
            let fade_out = 1.0 - t; // linear fade out
            let fade_in = t; // linear fade in
            result[result_start + i] = result[result_start + i] * fade_out + clip[i] * fade_in;
        }

        // Append the rest of the clip (after crossfade region)
        if clip.len() > cf {
            result.extend_from_slice(&clip[cf..]);
        }
    }

    result
}

/// Concatenate clips with gap durations between them.
pub fn concatenate_with_gaps(
    clips: &[Vec<f64>],
    gap_durations_ms: &[f64],
    crossfade_ms: f64,
    sr: u32,
) -> Vec<f64> {
    if clips.is_empty() {
        return vec![];
    }

    // Interleave clips with silence gaps
    let mut all_clips: Vec<Vec<f64>> = Vec::new();
    for (i, clip) in clips.iter().enumerate() {
        all_clips.push(clip.clone());
        if i < clips.len() - 1 {
            let gap_ms = if i < gap_durations_ms.len() {
                gap_durations_ms[i]
            } else {
                0.0
            };
            if gap_ms > 0.0 {
                all_clips.push(generate_silence(gap_ms, sr));
            }
        }
    }

    let cf_samples = (crossfade_ms / 1000.0 * sr as f64).round() as usize;
    concatenate(&all_clips.iter().collect::<Vec<_>>().iter().map(|c| c.as_slice().to_vec()).collect::<Vec<_>>(), cf_samples)
}

/// Pitch-shift by semitones using Signalsmith Stretch (phase vocoder).
///
/// Preserves duration while shifting pitch. High quality, no external tools.
pub fn pitch_shift(samples: &[f64], sr: u32, semitones: f64) -> Result<Vec<f64>> {
    if semitones.abs() < 0.01 {
        return Ok(samples.to_vec());
    }

    let mut stretch = ssstretch::Stretch::new();
    stretch.preset_default(1, sr as f32); // mono
    stretch.set_transpose_semitones(semitones as f32, None);

    let input_f32: Vec<f32> = samples.iter().map(|&s| s as f32).collect();
    let in_len = input_f32.len() as i32;
    let out_len = in_len; // pitch shift preserves length

    let mut output_f32 = vec![vec![0.0f32; out_len as usize]; 1];
    stretch.process_vec(
        &[input_f32],
        in_len,
        &mut output_f32,
        out_len,
    );

    Ok(output_f32[0].iter().map(|&s| s as f64).collect())
}

/// Time-stretch by factor using Signalsmith Stretch (phase vocoder).
///
/// `factor` > 1.0 = slower (longer), < 1.0 = faster (shorter).
/// Preserves pitch while changing duration. High quality, no external tools.
pub fn time_stretch(samples: &[f64], sr: u32, factor: f64) -> Result<Vec<f64>> {
    if (factor - 1.0).abs() < 0.01 {
        return Ok(samples.to_vec());
    }

    if samples.is_empty() {
        return Ok(vec![]);
    }

    let mut stretch = ssstretch::Stretch::new();
    stretch.preset_default(1, sr as f32);

    let input_f32: Vec<f32> = samples.iter().map(|&s| s as f32).collect();
    let in_len = input_f32.len() as i32;
    let out_len = (samples.len() as f64 * factor).round() as i32;

    if out_len <= 0 {
        return Ok(vec![]);
    }

    let mut output_f32 = vec![vec![0.0f32; out_len as usize]; 1];
    stretch.process_vec(
        &[input_f32],
        in_len,
        &mut output_f32,
        out_len,
    );

    Ok(output_f32[0].iter().map(|&s| s as f64).collect())
}

/// Adjust volume by dB amount. Modifies samples in place.
pub fn adjust_volume(samples: &mut [f64], db: f64) {
    if db.abs() < 0.01 {
        return;
    }
    let gain = 10.0f64.powf(db / 20.0);
    for sample in samples.iter_mut() {
        *sample *= gain;
    }
}

/// Mix secondary audio under primary at the given volume level.
///
/// Output duration matches the primary. Secondary is looped if shorter.
pub fn mix_audio(primary: &[f64], secondary: &[f64], secondary_volume_db: f64) -> Vec<f64> {
    if primary.is_empty() {
        return vec![];
    }
    if secondary.is_empty() {
        return primary.to_vec();
    }

    let gain = 10.0f64.powf(secondary_volume_db / 20.0);
    let mut result = primary.to_vec();

    for (i, sample) in result.iter_mut().enumerate() {
        let sec_idx = i % secondary.len(); // Loop secondary
        *sample += secondary[sec_idx] * gain;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cut_clip_basic() {
        let samples: Vec<f64> = (0..16000).map(|i| i as f64 / 16000.0).collect();
        let clip = cut_clip(&samples, 16000, 0.25, 0.75, 0.0, 0.0);
        assert_eq!(clip.len(), 8000);
        assert!((clip[0] - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_cut_clip_with_padding() {
        let samples: Vec<f64> = (0..16000).map(|i| i as f64 / 16000.0).collect();
        let clip = cut_clip(&samples, 16000, 0.25, 0.75, 25.0, 0.0);
        // With 25ms padding, clip should be ~8800 samples (8000 + 2*400)
        assert!(clip.len() > 8000);
    }

    #[test]
    fn test_cut_clip_empty() {
        let clip = cut_clip(&[], 16000, 0.0, 1.0, 0.0, 0.0);
        assert!(clip.is_empty());
    }

    #[test]
    fn test_generate_silence() {
        let silence = generate_silence(100.0, 16000);
        assert_eq!(silence.len(), 1600);
        assert!(silence.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_concatenate_no_crossfade() {
        let a = vec![1.0; 100];
        let b = vec![2.0; 100];
        let result = concatenate(&[a, b], 0);
        assert_eq!(result.len(), 200);
        assert_eq!(result[0], 1.0);
        assert_eq!(result[100], 2.0);
    }

    #[test]
    fn test_concatenate_with_crossfade() {
        let a = vec![1.0; 100];
        let b = vec![0.0; 100];
        let result = concatenate(&[a, b], 20);
        // Result should be shorter than 200 due to crossfade overlap
        assert_eq!(result.len(), 180);
        // At crossfade midpoint, should be ~0.5 (blend of 1.0 and 0.0)
        let mid = 90; // midpoint of crossfade region
        assert!((result[mid] - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_concatenate_single() {
        let a = vec![1.0; 100];
        let result = concatenate(&[a.clone()], 10);
        assert_eq!(result, a);
    }

    #[test]
    fn test_concatenate_empty() {
        let result = concatenate(&[], 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_adjust_volume() {
        let mut samples = vec![0.5; 100];
        adjust_volume(&mut samples, 6.0); // +6 dB ≈ 2x
        assert!((samples[0] - 1.0).abs() < 0.05);
    }

    #[test]
    fn test_adjust_volume_negative() {
        let mut samples = vec![1.0; 100];
        adjust_volume(&mut samples, -6.0); // -6 dB ≈ 0.5x
        assert!((samples[0] - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_adjust_volume_zero() {
        let mut samples = vec![0.5; 100];
        adjust_volume(&mut samples, 0.0);
        assert_eq!(samples[0], 0.5);
    }

    #[test]
    fn test_mix_audio_basic() {
        let primary = vec![0.5; 100];
        let secondary = vec![1.0; 100];
        let result = mix_audio(&primary, &secondary, -20.0); // -20 dB
        // -20 dB gain ≈ 0.1, so mixed ≈ 0.5 + 0.1 = 0.6
        assert!((result[0] - 0.6).abs() < 0.02);
    }

    #[test]
    fn test_mix_audio_loops_secondary() {
        let primary = vec![0.5; 200];
        let secondary = vec![1.0; 50]; // shorter, should loop
        let result = mix_audio(&primary, &secondary, 0.0);
        assert_eq!(result.len(), 200);
        // All samples should have secondary mixed in (looped)
        assert!((result[150] - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_mix_audio_empty() {
        assert!(mix_audio(&[], &[1.0], 0.0).is_empty());
        let primary = vec![0.5; 10];
        assert_eq!(mix_audio(&primary, &[], 0.0), primary);
    }

    #[test]
    fn test_time_stretch_no_change() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = time_stretch(&samples, 16000, 1.0).unwrap();
        assert_eq!(result, samples);
    }

    #[test]
    fn test_time_stretch_double() {
        let samples: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let result = time_stretch(&samples, 16000, 2.0).unwrap();
        assert!((result.len() as f64 - 200.0).abs() < 2.0);
    }

    #[test]
    fn test_pitch_shift_no_change() {
        let samples = vec![1.0; 100];
        let result = pitch_shift(&samples, 16000, 0.0).unwrap();
        assert_eq!(result, samples);
    }

    #[test]
    fn test_pitch_shift_native_up() {
        // 1 second of 440Hz sine at 16kHz
        let sr = 16000u32;
        let samples: Vec<f64> = (0..sr as usize)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin())
            .collect();

        let result = pitch_shift(&samples, sr, 2.0).unwrap();
        // Should preserve length (not change speed)
        assert!(
            (result.len() as f64 - samples.len() as f64).abs() < 100.0,
            "Pitch shift changed length: {} vs {}",
            result.len(),
            samples.len()
        );
        // Should not be silent
        let rms: f64 = (result.iter().map(|s| s * s).sum::<f64>() / result.len() as f64).sqrt();
        assert!(rms > 0.1, "Output is too quiet: RMS={}", rms);
    }

    #[test]
    fn test_time_stretch_native_double() {
        let sr = 16000u32;
        let samples: Vec<f64> = (0..sr as usize)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin())
            .collect();

        let result = time_stretch(&samples, sr, 2.0).unwrap();
        // Factor 2.0 = twice as long
        let expected_len = samples.len() * 2;
        assert!(
            (result.len() as f64 - expected_len as f64).abs() / (expected_len as f64) < 0.1,
            "Expected ~{} samples, got {}",
            expected_len,
            result.len()
        );
    }
}
