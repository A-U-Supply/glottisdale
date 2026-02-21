//! Render an arrangement to audio samples.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::effects_chain::apply_effects;
use super::types::{Arrangement, ClipId, SyllableClip};
use crate::audio::io::write_wav;

/// Render the full arrangement to a contiguous audio buffer.
///
/// Uses overlap-add: each clip's audio (with effects applied) is placed
/// at its timeline position into the output buffer.
pub fn render_arrangement(arrangement: &Arrangement) -> Result<Vec<f64>> {
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

    for timeline_clip in &arrangement.timeline {
        let source = bank_map
            .get(&timeline_clip.source_clip_id)
            .ok_or_else(|| anyhow::anyhow!("Missing source clip in bank"))?;

        let processed = apply_effects(&source.samples, sr, &timeline_clip.effects)?;

        let start_idx = (timeline_clip.position_s * sr as f64).round() as usize;
        for (i, &sample) in processed.iter().enumerate() {
            let out_idx = start_idx + i;
            if out_idx < output.len() {
                output[out_idx] += sample;
            }
        }
    }

    Ok(output)
}

/// Render and write the arrangement to a WAV file.
pub fn export_arrangement(arrangement: &Arrangement, output_path: &Path) -> Result<()> {
    let samples = render_arrangement(arrangement)?;
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
        let result = render_arrangement(&arr).unwrap();
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

        let result = render_arrangement(&arr).unwrap();
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

        let result = render_arrangement(&arr).unwrap();
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

        let result = render_arrangement(&arr).unwrap();
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

        export_arrangement(&arr, &path).unwrap();
        assert!(path.exists());
        let file_len = std::fs::metadata(&path).unwrap().len();
        assert!(file_len > 0);

        std::fs::remove_dir_all(&dir).ok();
    }
}
