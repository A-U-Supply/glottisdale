//! Map syllables to melody notes — the "drunk choir" engine.

use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::audio::effects::{concatenate, generate_silence, pitch_shift, time_stretch};
use crate::sing::midi_parser::{midi_to_hz, Note};
use crate::sing::syllable_prep::NormalizedSyllable;

/// How a melody note maps to syllable(s).
#[derive(Debug, Clone)]
pub struct NoteMapping {
    pub note_pitch: u8,
    pub note_start: f64,
    pub note_end: f64,
    pub note_duration: f64,
    pub syllable_indices: Vec<usize>,
    pub pitch_shift_semitones: f64,
    pub time_stretch_ratio: f64,
    pub apply_vibrato: bool,
    pub apply_chorus: bool,
    pub duration_class: DurationClass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DurationClass {
    Short,
    Medium,
    Long,
}

/// Classify a note by duration for mapping strategy.
pub fn classify_note_duration(duration: f64) -> DurationClass {
    if duration < 0.2 {
        DurationClass::Short
    } else if duration < 1.0 {
        DurationClass::Medium
    } else {
        DurationClass::Long
    }
}

/// Compute semitone shift from source F0 to target MIDI note (with optional drift).
pub fn compute_target_pitch(note_midi: u8, source_f0: f64, drift_semitones: f64) -> f64 {
    let target_hz = midi_to_hz(note_midi);
    let base_shift = 12.0 * (target_hz / source_f0).log2();
    base_shift + drift_semitones
}

/// Plan how each melody note maps to syllable(s).
pub fn plan_note_mapping(
    notes: &[Note],
    pool_size: usize,
    seed: Option<u64>,
    drift_range: f64,
    chorus_probability: f64,
) -> Vec<NoteMapping> {
    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    let mut mappings = Vec::new();
    let mut syl_cursor = 0usize;

    let short_choices = [1usize];
    let medium_choices = [1, 1, 1, 2, 2, 3];
    let long_choices = [1, 2, 2, 3, 3, 4];

    for note in notes {
        let duration = note.duration();
        let dur_class = classify_note_duration(duration);

        // Determine how many syllables this note gets
        let n_syls = match dur_class {
            DurationClass::Short => short_choices[rng.gen_range(0..short_choices.len())],
            DurationClass::Medium => medium_choices[rng.gen_range(0..medium_choices.len())],
            DurationClass::Long => long_choices[rng.gen_range(0..long_choices.len())],
        };

        // Assign syllable indices (cycle through pool)
        let mut indices = Vec::new();
        for _ in 0..n_syls {
            indices.push(syl_cursor % pool_size);
            syl_cursor += 1;
        }

        // Pitch drift (gaussian, constrained)
        let drift: f64 = rng.gen::<f64>() * drift_range * 2.0 / 3.0
            * if rng.gen::<bool>() { 1.0 } else { -1.0 };
        let drift = drift.clamp(-drift_range, drift_range);

        // Vibrato on held notes
        let apply_vibrato = dur_class == DurationClass::Long
            || (dur_class == DurationClass::Medium && duration > 0.6);

        // Chorus on sustained notes or random chance
        let apply_chorus = duration > 0.6 || rng.gen::<f64>() < chorus_probability;

        mappings.push(NoteMapping {
            note_pitch: note.pitch,
            note_start: note.start,
            note_end: note.end,
            note_duration: duration,
            syllable_indices: indices,
            pitch_shift_semitones: drift,
            time_stretch_ratio: 1.0, // computed at render time
            apply_vibrato,
            apply_chorus,
            duration_class: dur_class,
        });
    }

    mappings
}

/// Apply vibrato effect (pitch modulation) to audio samples.
fn apply_vibrato_effect(samples: &[f64], sr: u32, depth_cents: f64, rate_hz: f64) -> Vec<f64> {
    let mut output = Vec::with_capacity(samples.len());
    for i in 0..samples.len() {
        let t = i as f64 / sr as f64;
        let mod_factor = (2.0f64).powf(depth_cents / 1200.0 * (2.0 * std::f64::consts::PI * rate_hz * t).sin());
        // Simple pitch modulation by resampling
        let src_idx = i as f64 * mod_factor;
        let idx = src_idx as usize;
        if idx < samples.len() - 1 {
            let frac = src_idx - idx as f64;
            output.push(samples[idx] * (1.0 - frac) + samples[idx + 1] * frac);
        } else if idx < samples.len() {
            output.push(samples[idx]);
        }
    }
    output
}

/// Apply chorus effect by layering detuned copies.
fn apply_chorus_effect(samples: &[f64], sr: u32, n_voices: usize) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut result = samples.to_vec();

    for _ in 0..n_voices {
        let detune_cents = rng.gen_range(10.0..15.0) * if rng.gen::<bool>() { 1.0 } else { -1.0 };
        let delay_samples = (rng.gen_range(15.0..30.0) / 1000.0 * sr as f64).round() as usize;

        // Detune by resampling
        let ratio = (2.0f64).powf(detune_cents / 1200.0);
        let detuned_len = (samples.len() as f64 / ratio) as usize;
        let mut detuned = Vec::with_capacity(detuned_len);
        for i in 0..detuned_len {
            let src = i as f64 * ratio;
            let idx = src as usize;
            if idx < samples.len() - 1 {
                let frac = src - idx as f64;
                detuned.push(samples[idx] * (1.0 - frac) + samples[idx + 1] * frac);
            } else if idx < samples.len() {
                detuned.push(samples[idx]);
            }
        }

        // Apply delay and mix at 0.5 volume
        for (i, &s) in detuned.iter().enumerate() {
            let dst = i + delay_samples;
            if dst < result.len() {
                result[dst] += s * 0.5;
            }
        }
    }

    // Normalize to prevent clipping
    let peak = result.iter().map(|s| s.abs()).fold(0.0f64, f64::max);
    if peak > 1.0 {
        for s in result.iter_mut() {
            *s /= peak;
        }
    }

    result
}

/// Render a single note mapping to audio samples.
pub fn render_mapping(
    mapping: &NoteMapping,
    syllable_clips: &[NormalizedSyllable],
    median_f0: f64,
    max_shift: f64,
    sr: u32,
) -> Option<Vec<f64>> {
    let target_duration = mapping.note_duration;
    let n_syls = mapping.syllable_indices.len();
    let per_syl_duration = target_duration / n_syls as f64;

    // Compute per-syllable durations with rhythmic variation
    let mut rng = StdRng::seed_from_u64(mapping.note_pitch as u64 * 1000 + mapping.note_start.to_bits());
    let mut syl_durations = Vec::new();
    let mut remaining = target_duration;

    for i in 0..n_syls {
        if i == n_syls - 1 {
            syl_durations.push(remaining);
        } else {
            let variation = rng.gen_range(0.8..1.2);
            let d = per_syl_duration * variation;
            let d = d.min(remaining - 0.05 * (n_syls - i - 1) as f64);
            let d = d.max(0.05);
            syl_durations.push(d);
            remaining -= d;
        }
    }

    let mut rendered_parts: Vec<Vec<f64>> = Vec::new();

    for (_i, (&syl_idx, &syl_dur)) in mapping
        .syllable_indices
        .iter()
        .zip(syl_durations.iter())
        .enumerate()
    {
        if syl_idx >= syllable_clips.len() {
            continue;
        }
        let syl = &syllable_clips[syl_idx];

        // Compute total pitch shift: base (median->note) + drift
        let base_shift = compute_target_pitch(mapping.note_pitch, median_f0, mapping.pitch_shift_semitones);
        let shift = base_shift.clamp(-max_shift, max_shift);

        // Time stretch ratio
        let time_ratio = if syl_dur > 0.0 {
            syl.duration / syl_dur
        } else {
            1.0
        };
        let time_ratio = time_ratio.clamp(0.25, 4.0);

        // Apply pitch shift
        let mut part = if shift.abs() > 0.1 {
            pitch_shift(&syl.samples, syl.sr, shift).unwrap_or_else(|_| syl.samples.clone())
        } else {
            syl.samples.clone()
        };

        // Apply time stretch
        if (time_ratio - 1.0).abs() > 0.05 {
            let stretch_factor = 1.0 / time_ratio;
            match time_stretch(&part, syl.sr, stretch_factor) {
                Ok(stretched) => part = stretched,
                Err(_) => {
                    part = crate::audio::effects::time_stretch_simple(&part, syl.sr, stretch_factor);
                }
            }
        }

        // Apply vibrato if flagged
        if mapping.apply_vibrato && syl_dur > 0.3 {
            part = apply_vibrato_effect(&part, sr, 50.0, 5.5);
        }

        if !part.is_empty() {
            rendered_parts.push(part);
        }
    }

    if rendered_parts.is_empty() {
        return None;
    }

    // Concatenate parts with intra-note crossfade
    let crossfade = (20.0 / 1000.0 * sr as f64).round() as usize;
    let mut result = if rendered_parts.len() == 1 {
        rendered_parts.into_iter().next().unwrap()
    } else {
        concatenate(&rendered_parts, crossfade)
    };

    // Apply chorus if flagged
    if mapping.apply_chorus {
        result = apply_chorus_effect(&result, sr, 2);
    }

    Some(result)
}

/// Render all mappings into a complete vocal track.
pub fn render_vocal_track(
    mappings: &[NoteMapping],
    syllable_clips: &[NormalizedSyllable],
    median_f0: f64,
    sr: u32,
) -> Vec<f64> {
    let mut rendered_notes: Vec<(f64, f64, Vec<f64>)> = Vec::new(); // (start, end, samples)

    for mapping in mappings {
        if let Some(rendered) = render_mapping(mapping, syllable_clips, median_f0, 12.0, sr) {
            rendered_notes.push((mapping.note_start, mapping.note_end, rendered));
        }
    }

    if rendered_notes.is_empty() {
        return Vec::new();
    }

    // Build timeline: place rendered notes at their start times with gaps
    let mut parts: Vec<Vec<f64>> = Vec::new();
    let crossfade = (30.0 / 1000.0 * sr as f64).round() as usize;

    for (idx, (start, _end, samples)) in rendered_notes.iter().enumerate() {
        if idx > 0 {
            let prev_end = rendered_notes[idx - 1].1;
            let gap_duration = start - prev_end;
            if gap_duration > 0.01 {
                let gap = generate_silence(gap_duration * 1000.0, sr);
                parts.push(gap);
            }
        }
        parts.push(samples.clone());
    }

    if parts.len() == 1 {
        parts.into_iter().next().unwrap()
    } else {
        concatenate(&parts, crossfade)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_note_duration() {
        assert_eq!(classify_note_duration(0.1), DurationClass::Short);
        assert_eq!(classify_note_duration(0.5), DurationClass::Medium);
        assert_eq!(classify_note_duration(1.5), DurationClass::Long);
    }

    #[test]
    fn test_compute_target_pitch() {
        // A4 (69) to source at 440 Hz should be ~0 shift
        let shift = compute_target_pitch(69, 440.0, 0.0);
        assert!(shift.abs() < 0.01);

        // A5 (81) to source at 440 Hz should be ~12 semitones
        let shift = compute_target_pitch(81, 440.0, 0.0);
        assert!((shift - 12.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_target_pitch_with_drift() {
        let shift = compute_target_pitch(69, 440.0, 2.0);
        assert!((shift - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_plan_note_mapping_empty() {
        let mappings = plan_note_mapping(&[], 10, Some(42), 2.0, 0.3);
        assert!(mappings.is_empty());
    }

    #[test]
    fn test_plan_note_mapping_basic() {
        let notes = vec![
            Note { pitch: 60, start: 0.0, end: 0.5, velocity: 100 },
            Note { pitch: 64, start: 0.5, end: 1.5, velocity: 80 },
        ];
        let mappings = plan_note_mapping(&notes, 10, Some(42), 2.0, 0.3);
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].note_pitch, 60);
        assert_eq!(mappings[1].note_pitch, 64);
    }

    #[test]
    fn test_plan_note_mapping_deterministic() {
        let notes = vec![
            Note { pitch: 60, start: 0.0, end: 0.5, velocity: 100 },
        ];
        let a = plan_note_mapping(&notes, 10, Some(42), 2.0, 0.3);
        let b = plan_note_mapping(&notes, 10, Some(42), 2.0, 0.3);
        assert_eq!(a[0].syllable_indices, b[0].syllable_indices);
        assert_eq!(a[0].pitch_shift_semitones, b[0].pitch_shift_semitones);
    }

    #[test]
    fn test_apply_vibrato_effect() {
        let sr = 16000u32;
        let samples: Vec<f64> = (0..sr as usize).map(|i| {
            (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin()
        }).collect();
        let result = apply_vibrato_effect(&samples, sr, 50.0, 5.5);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_apply_chorus_effect() {
        let sr = 16000u32;
        let samples: Vec<f64> = (0..sr as usize).map(|i| {
            (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin()
        }).collect();
        let result = apply_chorus_effect(&samples, sr, 2);
        assert!(!result.is_empty());
    }
}
