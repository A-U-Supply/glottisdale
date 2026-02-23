//! Render an arrangement to audio samples.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::effects_chain::apply_effects;
use super::types::{Arrangement, ClipId, SyllableClip};
use crate::audio::analysis::{compute_rms, generate_pink_noise};
use crate::audio::effects::{mix_audio, time_stretch};
use crate::audio::io::write_wav;
use crate::collage::process::apply_prosodic_dynamics;

/// Settings that control how an arrangement is rendered to audio.
pub struct RenderSettings {
    pub crossfade_ms: f64,
    pub volume_normalize: bool,
    pub pitch_normalize: bool,
    pub pitch_range: f64,
    pub prosodic_dynamics: bool,
    pub noise_level_db: f64,
    pub room_tone: bool,
    pub breaths: bool,
    pub breath_probability: f64,
    pub speed: Option<f64>,
    pub seed: Option<u64>,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            crossfade_ms: 30.0,
            volume_normalize: true,
            pitch_normalize: true,
            pitch_range: 5.0,
            prosodic_dynamics: true,
            noise_level_db: -40.0,
            room_tone: true,
            breaths: true,
            breath_probability: 0.6,
            speed: None,
            seed: None,
        }
    }
}

impl RenderSettings {
    /// Create settings with everything disabled — all bools false,
    /// all floats 0.0, all options None.  Useful for tests that need
    /// deterministic, pass-through rendering.
    pub fn bypass() -> Self {
        Self {
            crossfade_ms: 0.0,
            volume_normalize: false,
            pitch_normalize: false,
            pitch_range: 0.0,
            prosodic_dynamics: false,
            noise_level_db: 0.0,
            room_tone: false,
            breaths: false,
            breath_probability: 0.0,
            speed: None,
            seed: None,
        }
    }
}

/// Render the full arrangement to a contiguous audio buffer.
///
/// Uses overlap-add: each clip's audio (with effects applied) is placed
/// at its timeline position into the output buffer.
pub fn render_arrangement(arrangement: &Arrangement, settings: &RenderSettings) -> Result<Vec<f64>> {
    if arrangement.timeline.is_empty() {
        return Ok(vec![]);
    }

    let sr = arrangement.sample_rate;

    // Build bank lookup
    let bank_map: HashMap<ClipId, &SyllableClip> = arrangement
        .bank
        .iter()
        .map(|c| (c.id, c))
        .collect();

    // Compute total output length
    let total_duration_s = arrangement.total_duration_s();
    let total_samples = (total_duration_s * sr as f64).ceil() as usize;

    let mut output = vec![0.0f64; total_samples];

    let cf_samples = (settings.crossfade_ms / 1000.0 * sr as f64).round() as usize;

    // Collect per-clip rendered audio
    let mut clip_buffers: Vec<(usize, Vec<f64>)> = Vec::new();
    for timeline_clip in &arrangement.timeline {
        let source = bank_map
            .get(&timeline_clip.source_clip_id)
            .ok_or_else(|| anyhow::anyhow!("Missing source clip in bank"))?;

        let processed = apply_effects(&source.samples, sr, &timeline_clip.effects)?;
        let start_idx = (timeline_clip.position_s * sr as f64).round() as usize;
        clip_buffers.push((start_idx, processed));
    }

    // Mix with crossfade
    for (clip_index, (start_idx, processed)) in clip_buffers.iter().enumerate() {
        for (i, &sample) in processed.iter().enumerate() {
            let out_idx = start_idx + i;
            if out_idx >= output.len() {
                break;
            }

            let mut gain = 1.0;

            // Fade-in at start of clip (except first clip)
            if cf_samples > 0 && clip_index > 0 && i < cf_samples {
                let t = i as f64 / cf_samples as f64;
                gain = (t * std::f64::consts::FRAC_PI_2).sin();
            }

            // Fade-out at end of clip (except last clip)
            let samples_from_end = processed.len().saturating_sub(1).saturating_sub(i);
            if cf_samples > 0 && clip_index < clip_buffers.len() - 1 && samples_from_end < cf_samples {
                let t = samples_from_end as f64 / cf_samples as f64;
                gain *= (t * std::f64::consts::FRAC_PI_2).sin();
            }

            output[out_idx] += sample * gain;
        }
    }

    // --- Volume normalize (peak to -1dB) ---
    if settings.volume_normalize {
        let peak = output.iter().map(|s| s.abs()).fold(0.0f64, f64::max);
        if peak > 1e-10 {
            let target = 10.0f64.powf(-1.0 / 20.0); // -1dB
            let gain = target / peak;
            for s in output.iter_mut() {
                *s *= gain;
            }
        }
    }

    // --- Prosodic dynamics ---
    if settings.prosodic_dynamics {
        apply_prosodic_dynamics(&mut output, sr);
    }

    // --- Room tone (mix into silent gaps) ---
    if settings.room_tone && !arrangement.room_tone_clips.is_empty() {
        let overall_rms = compute_rms(&output);
        if overall_rms > 1e-10 {
            let threshold = overall_rms * 0.05;
            let window = (sr as f64 * 0.025) as usize; // 25ms
            let mut i = 0;
            let mut rt_idx = 0;
            while i + window < output.len() {
                let frame: &[f64] = &output[i..i + window];
                let frame_rms = compute_rms(frame);
                if frame_rms < threshold {
                    let rt = &arrangement.room_tone_clips[rt_idx % arrangement.room_tone_clips.len()];
                    for (j, &rt_sample) in rt.iter().enumerate() {
                        if i + j < output.len() {
                            output[i + j] += rt_sample * 0.3;
                        }
                    }
                    rt_idx += 1;
                    i += rt.len().max(window);
                } else {
                    i += window;
                }
            }
        }
    }

    // --- Breaths (insert at clip boundaries) ---
    if settings.breaths && settings.breath_probability > 0.0 && !arrangement.breath_clips.is_empty() {
        use rand::Rng;
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        let mut rng = match settings.seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };
        for i in 0..arrangement.timeline.len().saturating_sub(1) {
            if rng.gen::<f64>() >= settings.breath_probability {
                continue;
            }
            let clip_end_s = arrangement.timeline[i].position_s
                + arrangement.timeline[i].effective_duration_s;
            let next_start_s = arrangement.timeline[i + 1].position_s;
            let gap_s = next_start_s - clip_end_s;
            if gap_s < 0.05 { continue; }
            let breath = &arrangement.breath_clips[rng.gen_range(0..arrangement.breath_clips.len())];
            let insert_idx = (clip_end_s * sr as f64).round() as usize;
            for (j, &sample) in breath.iter().enumerate() {
                let out_idx = insert_idx + j;
                if out_idx < output.len() {
                    output[out_idx] += sample * 0.5;
                }
            }
        }
    }

    // --- Pink noise bed ---
    if settings.noise_level_db < -0.1 || settings.noise_level_db > 0.1 {
        let dur = output.len() as f64 / sr as f64;
        let noise = generate_pink_noise(dur, sr, settings.seed);
        output = mix_audio(&output, &noise, settings.noise_level_db);
    }

    // --- Global speed ---
    if let Some(speed) = settings.speed {
        if (speed - 1.0).abs() > 0.01 {
            let factor = 1.0 / speed;
            output = time_stretch(&output, sr, factor)?;
        }
    }

    Ok(output)
}

/// Render and write the arrangement to a WAV file.
pub fn export_arrangement(arrangement: &Arrangement, settings: &RenderSettings, output_path: &Path) -> Result<()> {
    let samples = render_arrangement(arrangement, settings)?;
    write_wav(output_path, &samples, arrangement.sample_rate)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::types::*;
    use crate::types::{Phoneme, Syllable};
    use std::path::PathBuf;

    fn make_clip(value: f64, duration_samples: usize) -> SyllableClip {
        let syl = Syllable {
            phonemes: vec![Phoneme {
                label: "AH0".into(),
                start: 0.0,
                end: duration_samples as f64 / 16000.0,
            }],
            start: 0.0,
            end: duration_samples as f64 / 16000.0,
            word: "test".into(),
            word_index: 0,
        };
        let samples = vec![value; duration_samples];
        SyllableClip::new(syl, samples, 16000, PathBuf::from("test.wav"))
    }

    #[test]
    fn test_render_empty() {
        let arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        let result = render_arrangement(&arr, &RenderSettings::bypass()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_render_single_clip() {
        let clip = make_clip(0.5, 1600); // 0.1s
        let tc = TimelineClip::new(&clip);

        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);
        arr.timeline.push(tc);
        arr.relayout(0.0);

        let result = render_arrangement(&arr, &RenderSettings::bypass()).unwrap();
        assert_eq!(result.len(), 1600);
        assert!((result[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_render_two_clips_sequential() {
        let clip1 = make_clip(0.3, 1600);
        let clip2 = make_clip(0.7, 1600);
        let tc1 = TimelineClip::new(&clip1);
        let tc2 = TimelineClip::new(&clip2);

        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip1);
        arr.bank.push(clip2);
        arr.timeline.push(tc1);
        arr.timeline.push(tc2);
        arr.relayout(0.0);

        let result = render_arrangement(&arr, &RenderSettings::bypass()).unwrap();
        assert_eq!(result.len(), 3200);
        assert!((result[0] - 0.3).abs() < 0.001);
        assert!((result[1600] - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_render_with_effects() {
        let clip = make_clip(0.5, 1600);
        let mut tc = TimelineClip::new(&clip);
        tc.effects.push(ClipEffect::TimeStretch { factor: 2.0 });
        tc.effective_duration_s = crate::editor::effects_chain::compute_effective_duration(
            clip.duration_s(),
            &tc.effects,
        );

        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);
        arr.timeline.push(tc);
        arr.relayout(0.0);

        let result = render_arrangement(&arr, &RenderSettings::bypass()).unwrap();
        // Stretched 2x: ~3200 samples
        let ratio = result.len() as f64 / 1600.0;
        assert!(ratio > 1.8 && ratio < 2.2, "ratio={}", ratio);
    }

    #[test]
    fn test_export_creates_file() {
        let clip = make_clip(0.5, 1600);
        let tc = TimelineClip::new(&clip);

        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);
        arr.timeline.push(tc);
        arr.relayout(0.0);

        let dir = std::env::temp_dir().join("glottisdale_test_export");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_export.wav");

        export_arrangement(&arr, &RenderSettings::bypass(), &path).unwrap();
        assert!(path.exists());
        let file_len = std::fs::metadata(&path).unwrap().len();
        assert!(file_len > 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_render_with_settings_default() {
        let clip = make_clip(0.5, 1600);
        let tc = TimelineClip::new(&clip);
        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);
        arr.timeline.push(tc);
        arr.relayout(0.0);
        let settings = RenderSettings::default();
        let result = render_arrangement(&arr, &settings).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_render_crossfade_shortens_output() {
        let clip1 = make_clip(0.5, 1600); // 0.1s
        let clip2 = make_clip(0.5, 1600);
        let tc1 = TimelineClip::new(&clip1);
        let tc2 = TimelineClip::new(&clip2);

        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip1);
        arr.bank.push(clip2);
        arr.timeline.push(tc1);
        arr.timeline.push(tc2);

        // Without crossfade
        arr.relayout(0.0);
        let no_cf = render_arrangement(&arr, &RenderSettings::bypass()).unwrap();

        // With 30ms crossfade
        let mut settings = RenderSettings::bypass();
        settings.crossfade_ms = 30.0;
        arr.relayout_with_crossfade(settings.crossfade_ms);
        let with_cf = render_arrangement(&arr, &settings).unwrap();

        assert!(
            with_cf.len() < no_cf.len(),
            "crossfade should shorten output: with_cf={} no_cf={}",
            with_cf.len(),
            no_cf.len()
        );
    }

    #[test]
    fn test_render_volume_normalize_peaks_near_one() {
        let clip = make_clip(0.1, 1600); // quiet clip
        let tc = TimelineClip::new(&clip);
        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);
        arr.timeline.push(tc);
        arr.relayout(0.0);

        let mut settings = RenderSettings::bypass();
        settings.volume_normalize = true;
        let result = render_arrangement(&arr, &settings).unwrap();
        let peak = result.iter().map(|s| s.abs()).fold(0.0f64, f64::max);
        assert!(peak > 0.85, "peak={}, expected near 1.0 after normalization", peak);
    }

    #[test]
    fn test_render_pink_noise_adds_content() {
        let clip = make_clip(0.0, 16000); // 1s silence
        let tc = TimelineClip::new(&clip);
        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);
        arr.timeline.push(tc);
        arr.relayout(0.0);

        let mut settings = RenderSettings::bypass();
        settings.noise_level_db = -20.0;
        settings.seed = Some(42);
        let result = render_arrangement(&arr, &settings).unwrap();
        let rms: f64 = (result.iter().map(|s| s * s).sum::<f64>() / result.len() as f64).sqrt();
        assert!(rms > 0.001, "noise should be audible, rms={}", rms);
    }

    #[test]
    fn test_render_speed_changes_length() {
        let clip = make_clip(0.5, 16000); // 1s
        let tc = TimelineClip::new(&clip);
        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);
        arr.timeline.push(tc);
        arr.relayout(0.0);

        let mut settings = RenderSettings::bypass();
        settings.speed = Some(2.0);
        let result = render_arrangement(&arr, &settings).unwrap();
        let ratio = result.len() as f64 / 16000.0;
        assert!(ratio < 0.6, "2x speed should halve duration, ratio={}", ratio);
    }
}
