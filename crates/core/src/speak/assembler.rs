//! Assemble matched syllables into output audio.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::audio::analysis::{compute_rms, estimate_f0};
use crate::audio::effects::{
    adjust_volume, concatenate, concatenate_with_gaps, cut_clip, pitch_shift, time_stretch,
};
use crate::audio::io::write_wav;
use crate::speak::matcher::MatchResult;

/// Pause durations in seconds.
const WORD_PAUSE_S: f64 = 0.12;

/// Timing for a single output syllable.
#[derive(Debug, Clone)]
pub struct TimingPlan {
    /// Desired start time in output
    pub target_start: f64,
    /// Desired duration in output
    pub target_duration: f64,
    /// Time-stretch factor to apply (1.0 = no stretch)
    pub stretch_factor: f64,
}

/// Plan output timing for matched syllables.
///
/// In text mode (no reference_timings), uses source duration.
/// In reference mode, blends source and reference duration based on strictness.
pub fn plan_timing(
    matches: &[MatchResult],
    word_boundaries: &[usize],
    avg_syllable_dur: f64,
    reference_timings: Option<&[(f64, f64)]>,
    timing_strictness: f64,
) -> Vec<TimingPlan> {
    let word_starts: HashSet<usize> = word_boundaries.iter().copied().collect();
    let mut plans = Vec::new();
    let mut cursor = 0.0;

    for (i, m) in matches.iter().enumerate() {
        let source_dur = m.entry.end - m.entry.start;

        let (target_start, target_dur) = if let Some(ref_timings) = reference_timings {
            if i < ref_timings.len() {
                let (ref_start, ref_end) = ref_timings[i];
                let ref_dur = ref_end - ref_start;
                let dur = source_dur + timing_strictness * (ref_dur - source_dur);
                let start = cursor + timing_strictness * (ref_start - cursor);
                (start, dur)
            } else {
                let dur = if source_dur > 0.0 {
                    source_dur
                } else {
                    avg_syllable_dur
                };
                (cursor, dur)
            }
        } else {
            let dur = if source_dur > 0.0 {
                source_dur
            } else {
                avg_syllable_dur
            };
            (cursor, dur)
        };

        // Add word-boundary pause
        let target_start = if word_starts.contains(&i) && i > 0 {
            target_start + WORD_PAUSE_S
        } else {
            target_start
        };

        let stretch = if source_dur > 0.0 {
            target_dur / source_dur
        } else {
            1.0
        };

        plans.push(TimingPlan {
            target_start,
            target_duration: target_dur,
            stretch_factor: stretch,
        });
        cursor = target_start + target_dur;
    }

    plans
}

/// Group consecutive matches from adjacent source syllables.
///
/// Returns a list of runs, where each run is a list of indices into
/// `matches` / `timing`. Adjacent means same source file and the next
/// syllable index in that file.
fn group_contiguous_runs(matches: &[MatchResult]) -> Vec<Vec<usize>> {
    if matches.is_empty() {
        return Vec::new();
    }

    let mut runs: Vec<Vec<usize>> = vec![vec![0]];

    for i in 1..matches.len() {
        let prev = &matches[runs.last().unwrap().last().copied().unwrap()].entry;
        let curr = &matches[i].entry;
        if curr.source_path == prev.source_path && curr.index == prev.index + 1 {
            runs.last_mut().unwrap().push(i);
        } else {
            runs.push(vec![i]);
        }
    }

    runs
}

/// Normalize volume across clips to median RMS.
fn normalize_volume_clips(clips: &mut [Vec<f64>]) {
    let rms_values: Vec<f64> = clips
        .iter()
        .map(|c| compute_rms(c))
        .filter(|&r| r > 1e-6)
        .collect();

    if rms_values.is_empty() {
        return;
    }

    let mut sorted = rms_values.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let target_rms = sorted[sorted.len() / 2];

    if target_rms < 1e-6 {
        return;
    }

    for clip in clips.iter_mut() {
        let clip_rms = compute_rms(clip);
        if clip_rms < 1e-6 {
            continue;
        }
        let db_adjust = 20.0 * (target_rms / clip_rms).log10();
        let db_adjust = db_adjust.clamp(-20.0, 20.0);
        if db_adjust.abs() >= 0.5 {
            adjust_volume(clip, db_adjust);
        }
    }
}

/// Normalize pitch across clips toward median F0.
fn normalize_pitch_clips(clips: &mut [Vec<f64>], sr: u32, pitch_range: f64) {
    let f0_values: Vec<(usize, f64)> = clips
        .iter()
        .enumerate()
        .filter_map(|(i, c)| estimate_f0(c, sr, 80, 600).map(|f0| (i, f0)))
        .collect();

    if f0_values.is_empty() {
        return;
    }

    let mut sorted_f0s: Vec<f64> = f0_values.iter().map(|(_, f0)| *f0).collect();
    sorted_f0s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let target_f0 = sorted_f0s[sorted_f0s.len() / 2];

    log::info!(
        "Pitch normalization: target F0 = {:.1}Hz (from {} voiced clips)",
        target_f0,
        f0_values.len()
    );

    for (i, f0) in &f0_values {
        let semitones_shift = 12.0 * (target_f0 / f0).log2();
        let semitones_shift = semitones_shift.clamp(-pitch_range, pitch_range);
        if semitones_shift.abs() >= 0.1 {
            if let Ok(shifted) = pitch_shift(&clips[*i], sr, semitones_shift) {
                clips[*i] = shifted;
            }
        }
    }
}

/// Cut, stretch, and concatenate matched syllables into output audio.
///
/// Consecutive matches from adjacent positions in the same source file
/// are cut as a single clip to preserve natural coarticulation.
pub fn assemble(
    matches: &[MatchResult],
    timing: &[TimingPlan],
    source_samples: &std::collections::HashMap<String, (Vec<f64>, u32)>,
    output_dir: &Path,
    crossfade_ms: f64,
    pitch_shifts: Option<&[f64]>,
    do_normalize_volume: bool,
    do_normalize_pitch: bool,
) -> Result<PathBuf> {
    let runs = group_contiguous_runs(matches);

    let mut clips: Vec<Vec<f64>> = Vec::new();
    let mut gap_durations: Vec<f64> = Vec::new();
    let mut sample_rate = 16000u32;

    for (run_idx, run) in runs.iter().enumerate() {
        let first = run[0];
        let last = *run.last().unwrap();

        let source_path = &matches[first].entry.source_path;
        let (samples, sr) = source_samples
            .get(source_path)
            .ok_or_else(|| anyhow::anyhow!("Source audio not loaded: {}", source_path))?;
        sample_rate = *sr;

        // Cut the entire contiguous span as one clip
        let mut clip = cut_clip(
            samples,
            *sr,
            matches[first].entry.start,
            matches[last].entry.end,
            5.0,
            3.0,
        );

        // Time-stretch: compare total source duration to total target duration
        let source_dur = matches[last].entry.end - matches[first].entry.start;
        let target_dur: f64 = run.iter().map(|&i| timing[i].target_duration).sum();
        let stretch = if source_dur > 0.0 {
            target_dur / source_dur
        } else {
            1.0
        };

        if (stretch - 1.0).abs() > 0.05 {
            match time_stretch(&clip, *sr, stretch) {
                Ok(stretched) => clip = stretched,
                Err(_) => {
                    // Fall back to simple stretching
                    clip = crate::audio::effects::time_stretch_simple(&clip, *sr, stretch);
                }
            }
        }

        // Pitch-shift (use average of per-syllable shifts for the run)
        if let Some(shifts) = pitch_shifts {
            let run_shifts: Vec<f64> = run
                .iter()
                .filter_map(|&i| {
                    shifts.get(i).copied().filter(|s| s.abs() > 0.1)
                })
                .collect();
            if !run_shifts.is_empty() {
                let avg_shift: f64 = run_shifts.iter().sum::<f64>() / run_shifts.len() as f64;
                if let Ok(shifted) = pitch_shift(&clip, *sr, avg_shift) {
                    clip = shifted;
                }
            }
        }

        clips.push(clip);

        // Gap to next run
        if run_idx < runs.len() - 1 {
            let this_end =
                timing[last].target_start + timing[last].target_duration;
            let next_start = timing[runs[run_idx + 1][0]].target_start;
            let gap = (next_start - this_end).max(0.0) * 1000.0; // ms
            gap_durations.push(gap);
        }
    }

    // Normalize volume and pitch across clips
    if do_normalize_volume {
        normalize_volume_clips(&mut clips);
    }

    if do_normalize_pitch {
        normalize_pitch_clips(&mut clips, sample_rate, 5.0);
    }

    // Concatenate all clips
    let crossfade_samples = ((crossfade_ms / 1000.0) * sample_rate as f64).round() as usize;

    let output_samples = if !gap_durations.is_empty() {
        concatenate_with_gaps(&clips, &gap_durations, crossfade_ms, sample_rate)
    } else {
        concatenate(&clips, crossfade_samples)
    };

    let output_path = output_dir.join("speak.wav");
    write_wav(&output_path, &output_samples, sample_rate)?;

    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speak::syllable_bank::SyllableEntry;

    fn make_match(
        target: &[&str],
        entry_phonemes: &[&str],
        index: usize,
        source: &str,
        start: f64,
        end: f64,
    ) -> MatchResult {
        MatchResult {
            target_phonemes: target.iter().map(|s| s.to_string()).collect(),
            entry: SyllableEntry {
                phoneme_labels: entry_phonemes.iter().map(|s| s.to_string()).collect(),
                start,
                end,
                word: "test".to_string(),
                stress: Some(1),
                source_path: source.to_string(),
                index,
            },
            distance: 0,
            target_index: index,
        }
    }

    #[test]
    fn test_plan_timing_text_mode() {
        let matches = vec![
            make_match(&["K", "AE1", "T"], &["K", "AE1", "T"], 0, "a.wav", 0.0, 0.3),
            make_match(&["D", "AO1", "G"], &["D", "AO1", "G"], 1, "a.wav", 0.3, 0.6),
        ];
        let timing = plan_timing(&matches, &[0, 1], 0.25, None, 0.8);
        assert_eq!(timing.len(), 2);
        assert!((timing[0].target_start - 0.0).abs() < 1e-10);
        assert!((timing[0].target_duration - 0.3).abs() < 1e-10);
        // Second syllable starts after first + word pause
        assert!((timing[1].target_start - (0.3 + WORD_PAUSE_S)).abs() < 1e-10);
    }

    #[test]
    fn test_plan_timing_no_word_pause_same_word() {
        let matches = vec![
            make_match(&["K", "AE1"], &["K", "AE1"], 0, "a.wav", 0.0, 0.2),
            make_match(&["T"], &["T"], 1, "a.wav", 0.2, 0.3),
        ];
        // Both syllables in same word (boundary only at index 0)
        let timing = plan_timing(&matches, &[0], 0.25, None, 0.8);
        // No word pause between syllables of same word
        assert!((timing[1].target_start - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_plan_timing_reference_mode() {
        let matches = vec![
            make_match(&["K"], &["K"], 0, "a.wav", 0.0, 0.3),
        ];
        let ref_timings = vec![(0.0, 0.5)];
        let timing = plan_timing(&matches, &[0], 0.25, Some(&ref_timings), 0.8);
        // With strictness 0.8, duration = 0.3 + 0.8 * (0.5 - 0.3) = 0.46
        assert!((timing[0].target_duration - 0.46).abs() < 1e-10);
    }

    #[test]
    fn test_group_contiguous_runs() {
        let matches = vec![
            make_match(&["K"], &["K"], 0, "a.wav", 0.0, 0.1),
            make_match(&["AE"], &["AE"], 1, "a.wav", 0.1, 0.2),
            make_match(&["T"], &["T"], 0, "b.wav", 0.0, 0.1),
            make_match(&["D"], &["D"], 1, "b.wav", 0.1, 0.2),
        ];
        let runs = group_contiguous_runs(&matches);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0], vec![0, 1]); // adjacent in a.wav
        assert_eq!(runs[1], vec![2, 3]); // adjacent in b.wav
    }

    #[test]
    fn test_group_contiguous_runs_isolated() {
        let matches = vec![
            make_match(&["K"], &["K"], 0, "a.wav", 0.0, 0.1),
            make_match(&["T"], &["T"], 5, "a.wav", 0.5, 0.6), // non-adjacent
            make_match(&["D"], &["D"], 0, "b.wav", 0.0, 0.1), // different source
        ];
        let runs = group_contiguous_runs(&matches);
        assert_eq!(runs.len(), 3);
    }

    #[test]
    fn test_group_contiguous_runs_empty() {
        let runs = group_contiguous_runs(&[]);
        assert!(runs.is_empty());
    }

    #[test]
    fn test_normalize_volume_clips() {
        let mut clips = vec![
            vec![0.5; 100],   // RMS ~0.5
            vec![0.1; 100],   // RMS ~0.1
            vec![0.3; 100],   // RMS ~0.3
        ];
        normalize_volume_clips(&mut clips);
        // After normalization, RMS values should be closer together
        let rms_after: Vec<f64> = clips.iter().map(|c| compute_rms(c)).collect();
        let range_before = 0.5 - 0.1; // 0.4
        let range_after = rms_after.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - rms_after.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(range_after < range_before);
    }

    #[test]
    fn test_normalize_volume_silent_clips() {
        let mut clips = vec![
            vec![0.0; 100],  // silent
            vec![0.5; 100],
        ];
        // Should not crash on silent clips
        normalize_volume_clips(&mut clips);
    }

    #[test]
    fn test_stretch_factor() {
        let matches = vec![
            make_match(&["K"], &["K"], 0, "a.wav", 0.0, 0.2),
        ];
        let ref_timings = vec![(0.0, 0.4)];
        let timing = plan_timing(&matches, &[0], 0.25, Some(&ref_timings), 1.0);
        // With strictness 1.0, full reference timing, stretch = 0.4 / 0.2 = 2.0
        assert!((timing[0].stretch_factor - 2.0).abs() < 1e-10);
    }
}
