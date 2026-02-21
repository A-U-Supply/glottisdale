# Interactive Syllable Editor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a DAW-like interactive syllable editor to the glottisdale GUI with waveform display, drag-and-drop arrangement, non-destructive effects, and non-blocking playback.

**Architecture:** The editor is a sub-view overlaying the GUI's central panel, backed by a core `editor` module containing data types, waveform computation, effects chain, arrangement renderer, and a non-blocking playback engine. The GUI side adds a timeline widget with custom egui painting, a syllable bank palette, toolbar, and interaction handling. All three pipelines (collage, sing, speak) can open the editor.

**Tech Stack:** Rust, egui/eframe 0.31, rodio 0.20, ssstretch, uuid

**Design doc:** `docs/plans/2026-02-21-syllable-editor-design.md`

---

## Task 1: Core Data Types and Waveform Computation

**Files:**
- Create: `crates/core/src/editor/mod.rs`
- Create: `crates/core/src/editor/types.rs`
- Create: `crates/core/src/editor/waveform.rs`
- Modify: `crates/core/src/lib.rs:1-8`
- Modify: `crates/core/Cargo.toml`
- Modify: `Cargo.toml` (workspace deps)

**Step 1: Add uuid dependency**

In `Cargo.toml` (workspace), add to `[workspace.dependencies]`:

```toml
uuid = { version = "1", features = ["v4", "serde"] }
```

In `crates/core/Cargo.toml`, add to `[dependencies]`:

```toml
uuid.workspace = true
```

**Step 2: Create the editor module declaration**

Create `crates/core/src/editor/mod.rs`:

```rust
//! Interactive syllable editor data model and processing.

pub mod types;
pub mod waveform;

pub use types::*;
pub use waveform::WaveformData;
```

Add to `crates/core/src/lib.rs`:

```rust
pub mod editor;
```

**Step 3: Write the failing tests for WaveformData**

Create `crates/core/src/editor/waveform.rs`:

```rust
//! Pre-computed waveform peak data for efficient rendering.

/// Pre-computed waveform data for efficient rendering.
///
/// Stores (min_peak, max_peak) pairs at a fixed bucket size.
/// At sr=16000 and bucket_size=256, a 0.3s syllable produces ~19 peak pairs.
#[derive(Debug, Clone)]
pub struct WaveformData {
    /// (min_peak, max_peak) pairs per bucket.
    pub peaks: Vec<(f32, f32)>,
    /// How many source samples each peak bucket represents.
    pub samples_per_bucket: usize,
}

const DEFAULT_BUCKET_SIZE: usize = 256;

impl WaveformData {
    /// Compute waveform peaks from audio samples.
    pub fn from_samples(samples: &[f64], bucket_size: usize) -> Self {
        todo!()
    }

    /// Compute waveform peaks with default bucket size (256).
    pub fn new(samples: &[f64]) -> Self {
        Self::from_samples(samples, DEFAULT_BUCKET_SIZE)
    }

    /// Duration in seconds given a sample rate.
    pub fn duration_s(&self, sample_rate: u32) -> f64 {
        (self.peaks.len() * self.samples_per_bucket) as f64 / sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_from_silence() {
        let samples = vec![0.0f64; 1024];
        let wf = WaveformData::from_samples(&samples, 256);
        assert_eq!(wf.peaks.len(), 4);
        assert_eq!(wf.samples_per_bucket, 256);
        for &(min, max) in &wf.peaks {
            assert_eq!(min, 0.0);
            assert_eq!(max, 0.0);
        }
    }

    #[test]
    fn test_waveform_from_sine() {
        let sr = 16000;
        let samples: Vec<f64> = (0..sr)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin())
            .collect();
        let wf = WaveformData::new(&samples);
        assert_eq!(wf.samples_per_bucket, 256);
        // 16000 / 256 = 62.5, so 62 full buckets + 1 partial
        assert_eq!(wf.peaks.len(), 63);
        // Sine wave should have peaks near -1.0 and 1.0
        let max_peak = wf.peaks.iter().map(|&(_, max)| max).fold(f32::NEG_INFINITY, f32::max);
        let min_peak = wf.peaks.iter().map(|&(min, _)| min).fold(f32::INFINITY, f32::min);
        assert!(max_peak > 0.9, "max_peak={}", max_peak);
        assert!(min_peak < -0.9, "min_peak={}", min_peak);
    }

    #[test]
    fn test_waveform_from_impulse() {
        let mut samples = vec![0.0f64; 512];
        samples[100] = 1.0;
        samples[400] = -0.8;
        let wf = WaveformData::from_samples(&samples, 256);
        assert_eq!(wf.peaks.len(), 2);
        // First bucket contains the impulse at index 100
        assert_eq!(wf.peaks[0].1, 1.0);
        // Second bucket contains the negative impulse at index 400
        assert_eq!(wf.peaks[1].0, -0.8f32);
    }

    #[test]
    fn test_waveform_empty() {
        let wf = WaveformData::new(&[]);
        assert!(wf.peaks.is_empty());
    }

    #[test]
    fn test_waveform_partial_bucket() {
        // 300 samples with bucket_size 256 = 1 full bucket + 1 partial
        let samples = vec![0.5f64; 300];
        let wf = WaveformData::from_samples(&samples, 256);
        assert_eq!(wf.peaks.len(), 2);
        assert_eq!(wf.peaks[0], (0.5, 0.5));
        assert_eq!(wf.peaks[1], (0.5, 0.5));
    }

    #[test]
    fn test_waveform_duration() {
        let samples = vec![0.0f64; 16000]; // 1 second at sr=16000
        let wf = WaveformData::new(&samples);
        let dur = wf.duration_s(16000);
        // 63 buckets * 256 samples = 16128 samples -> 1.008s
        // (slight overestimate due to bucket granularity)
        assert!((dur - 1.0).abs() < 0.02, "duration={}", dur);
    }
}
```

**Step 4: Run tests to verify they fail**

Run: `cargo test -p glottisdale-core editor::waveform -- --nocapture`
Expected: FAIL with "not yet implemented"

**Step 5: Implement WaveformData::from_samples**

Replace the `todo!()` in `from_samples`:

```rust
    pub fn from_samples(samples: &[f64], bucket_size: usize) -> Self {
        if samples.is_empty() {
            return Self {
                peaks: Vec::new(),
                samples_per_bucket: bucket_size,
            };
        }

        let mut peaks = Vec::with_capacity(samples.len() / bucket_size + 1);
        for chunk in samples.chunks(bucket_size) {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for &s in chunk {
                if s < min { min = s; }
                if s > max { max = s; }
            }
            peaks.push((min as f32, max as f32));
        }

        Self {
            peaks,
            samples_per_bucket: bucket_size,
        }
    }
```

**Step 6: Run tests to verify they pass**

Run: `cargo test -p glottisdale-core editor::waveform -- --nocapture`
Expected: All PASS

**Step 7: Write the failing tests for editor types**

Create `crates/core/src/editor/types.rs`:

```rust
//! Editor data model: syllable clips, timeline clips, arrangements.

use std::path::PathBuf;
use uuid::Uuid;

use crate::types::Syllable;
use super::waveform::WaveformData;

/// Unique identifier for a clip.
pub type ClipId = Uuid;

/// A single syllable's audio data, ready for editing.
#[derive(Debug, Clone)]
pub struct SyllableClip {
    pub id: ClipId,
    /// Source syllable metadata (phonemes, word, timing in source).
    pub syllable: Syllable,
    /// Raw audio samples (f64, mono).
    pub samples: Vec<f64>,
    pub sample_rate: u32,
    /// Path to source audio file.
    pub source_path: PathBuf,
    /// Pre-computed waveform thumbnail.
    pub waveform: WaveformData,
    /// Display label (e.g. "K AE1 T").
    pub label: String,
}

impl SyllableClip {
    /// Create a new SyllableClip, computing the waveform automatically.
    pub fn new(
        syllable: Syllable,
        samples: Vec<f64>,
        sample_rate: u32,
        source_path: PathBuf,
    ) -> Self {
        let label = syllable
            .phonemes
            .iter()
            .map(|p| p.label.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let waveform = WaveformData::new(&samples);
        Self {
            id: Uuid::new_v4(),
            syllable,
            samples,
            sample_rate,
            source_path,
            waveform,
            label,
        }
    }

    /// Duration in seconds.
    pub fn duration_s(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }
}

/// A non-destructive effect applied to a timeline clip.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipEffect {
    Stutter { count: usize },
    TimeStretch { factor: f64 },
    PitchShift { semitones: f64 },
}

/// A clip placed on the timeline.
#[derive(Debug, Clone)]
pub struct TimelineClip {
    pub id: ClipId,
    /// References a SyllableClip in the bank by ID.
    pub source_clip_id: ClipId,
    /// Position on timeline in seconds (left edge).
    pub position_s: f64,
    /// Effects stack applied to this instance.
    pub effects: Vec<ClipEffect>,
    /// Duration in seconds after effects. Recomputed when effects change.
    pub effective_duration_s: f64,
}

impl TimelineClip {
    /// Create a new TimelineClip referencing a bank clip.
    pub fn new(source_clip: &SyllableClip) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_clip_id: source_clip.id,
            position_s: 0.0,
            effects: Vec::new(),
            effective_duration_s: source_clip.duration_s(),
        }
    }
}

/// Which pipeline produced the arrangement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditorPipelineMode {
    Collage,
    Sing,
    Speak,
}

/// Full state of a syllable arrangement.
#[derive(Debug, Clone)]
pub struct Arrangement {
    /// All available syllable clips (the palette/bank).
    pub bank: Vec<SyllableClip>,
    /// Clips placed on the timeline, ordered by position.
    pub timeline: Vec<TimelineClip>,
    /// Crossfade duration in milliseconds for final render.
    pub crossfade_ms: f64,
    /// Sample rate (always 16000 in this project).
    pub sample_rate: u32,
    /// Which pipeline produced this arrangement.
    pub source_pipeline: EditorPipelineMode,
}

impl Arrangement {
    /// Create an empty arrangement for a given pipeline mode.
    pub fn new(sample_rate: u32, pipeline: EditorPipelineMode) -> Self {
        Self {
            bank: Vec::new(),
            timeline: Vec::new(),
            crossfade_ms: 30.0,
            sample_rate,
            source_pipeline: pipeline,
        }
    }

    /// Look up a bank clip by ID.
    pub fn get_bank_clip(&self, id: ClipId) -> Option<&SyllableClip> {
        self.bank.iter().find(|c| c.id == id)
    }

    /// Total duration of the arrangement in seconds.
    pub fn total_duration_s(&self) -> f64 {
        self.timeline
            .last()
            .map(|c| c.position_s + c.effective_duration_s)
            .unwrap_or(0.0)
    }

    /// Recompute sequential positions for all timeline clips.
    pub fn relayout(&mut self, gap_s: f64) {
        let mut cursor = 0.0;
        for clip in &mut self.timeline {
            clip.position_s = cursor;
            cursor += clip.effective_duration_s + gap_s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Phoneme;

    fn make_test_syllable() -> Syllable {
        Syllable {
            phonemes: vec![
                Phoneme { label: "K".into(), start: 0.0, end: 0.1 },
                Phoneme { label: "AE1".into(), start: 0.1, end: 0.2 },
                Phoneme { label: "T".into(), start: 0.2, end: 0.3 },
            ],
            start: 0.0,
            end: 0.3,
            word: "cat".into(),
            word_index: 0,
        }
    }

    fn make_test_clip() -> SyllableClip {
        let syl = make_test_syllable();
        let samples = vec![0.0f64; 4800]; // 0.3s at 16kHz
        SyllableClip::new(syl, samples, 16000, PathBuf::from("test.wav"))
    }

    #[test]
    fn test_syllable_clip_creation() {
        let clip = make_test_clip();
        assert_eq!(clip.label, "K AE1 T");
        assert!((clip.duration_s() - 0.3).abs() < 0.001);
        assert!(!clip.waveform.peaks.is_empty());
    }

    #[test]
    fn test_timeline_clip_creation() {
        let bank_clip = make_test_clip();
        let tc = TimelineClip::new(&bank_clip);
        assert_eq!(tc.source_clip_id, bank_clip.id);
        assert_eq!(tc.position_s, 0.0);
        assert!(tc.effects.is_empty());
        assert!((tc.effective_duration_s - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_arrangement_empty() {
        let arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        assert!(arr.bank.is_empty());
        assert!(arr.timeline.is_empty());
        assert_eq!(arr.total_duration_s(), 0.0);
    }

    #[test]
    fn test_arrangement_relayout() {
        let clip1 = make_test_clip();
        let clip2 = make_test_clip();
        let tc1 = TimelineClip::new(&clip1);
        let tc2 = TimelineClip::new(&clip2);

        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip1);
        arr.bank.push(clip2);
        arr.timeline.push(tc1);
        arr.timeline.push(tc2);
        arr.relayout(0.0);

        assert_eq!(arr.timeline[0].position_s, 0.0);
        assert!((arr.timeline[1].position_s - 0.3).abs() < 0.001);
        assert!((arr.total_duration_s() - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_arrangement_relayout_with_gap() {
        let clip = make_test_clip();
        let tc1 = TimelineClip::new(&clip);
        let tc2 = TimelineClip::new(&clip);

        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);
        arr.timeline.push(tc1);
        arr.timeline.push(tc2);
        arr.relayout(0.1); // 100ms gap

        assert_eq!(arr.timeline[0].position_s, 0.0);
        assert!((arr.timeline[1].position_s - 0.4).abs() < 0.001); // 0.3 + 0.1 gap
    }

    #[test]
    fn test_get_bank_clip() {
        let clip = make_test_clip();
        let id = clip.id;
        let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);
        arr.bank.push(clip);

        assert!(arr.get_bank_clip(id).is_some());
        assert!(arr.get_bank_clip(Uuid::new_v4()).is_none());
    }
}
```

**Step 8: Run tests to verify they pass**

Run: `cargo test -p glottisdale-core editor -- --nocapture`
Expected: All PASS

**Step 9: Commit**

```bash
git add Cargo.toml crates/core/Cargo.toml crates/core/src/lib.rs \
  crates/core/src/editor/
git commit -m "feat(editor): add core data types and waveform computation"
```

---

## Task 2: Effects Chain and Duration Computation

**Files:**
- Create: `crates/core/src/editor/effects_chain.rs`
- Modify: `crates/core/src/editor/mod.rs`

**Step 1: Write the failing tests**

Create `crates/core/src/editor/effects_chain.rs`:

```rust
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
    todo!()
}

/// Compute effective duration after effects, without materializing samples.
pub fn compute_effective_duration(base_duration_s: f64, effects: &[ClipEffect]) -> f64 {
    todo!()
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
```

Add to `crates/core/src/editor/mod.rs`:

```rust
pub mod effects_chain;
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p glottisdale-core editor::effects_chain -- --nocapture`
Expected: FAIL with "not yet implemented"

**Step 3: Implement the functions**

Replace `todo!()` in `apply_effects`:

```rust
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
```

Replace `todo!()` in `compute_effective_duration`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p glottisdale-core editor::effects_chain -- --nocapture`
Expected: All PASS

**Step 5: Commit**

```bash
git add crates/core/src/editor/effects_chain.rs crates/core/src/editor/mod.rs
git commit -m "feat(editor): add effects chain and duration computation"
```

---

## Task 3: Arrangement Renderer

**Files:**
- Create: `crates/core/src/editor/render.rs`
- Modify: `crates/core/src/editor/mod.rs`

**Step 1: Write the failing tests**

Create `crates/core/src/editor/render.rs`:

```rust
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
    todo!()
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
```

Add to `crates/core/src/editor/mod.rs`:

```rust
pub mod render;
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p glottisdale-core editor::render -- --nocapture`
Expected: FAIL with "not yet implemented"

**Step 3: Implement render_arrangement**

Replace `todo!()`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p glottisdale-core editor::render -- --nocapture`
Expected: All PASS

**Step 5: Run full test suite**

Run: `cargo test --workspace`
Expected: All pass

**Step 6: Commit**

```bash
git add crates/core/src/editor/render.rs crates/core/src/editor/mod.rs
git commit -m "feat(editor): add arrangement renderer and WAV export"
```

---

## Task 4: Pipeline Bridge — Convert Pipeline Output to Arrangement

**Files:**
- Create: `crates/core/src/editor/pipeline_bridge.rs`
- Create: `crates/core/src/editor/bank_builder.rs`
- Modify: `crates/core/src/editor/mod.rs`

**Step 1: Write the failing tests for bank_builder**

Create `crates/core/src/editor/bank_builder.rs`:

```rust
//! Build a syllable bank from aligned source audio.

use std::path::PathBuf;

use anyhow::Result;

use super::types::SyllableClip;
use crate::audio::effects::cut_clip;
use crate::audio::io::read_wav;
use crate::types::Syllable;

/// Build SyllableClips from aligned syllables and their source audio.
///
/// For each syllable, cuts the audio with 25ms padding and 5ms fade,
/// computes waveform data, and creates a SyllableClip.
pub fn build_bank_from_syllables(
    syllables: &[(Syllable, PathBuf)],
    source_audio: &std::collections::HashMap<PathBuf, (Vec<f64>, u32)>,
) -> Result<Vec<SyllableClip>> {
    todo!()
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
```

**Step 2: Implement build_bank_from_syllables**

Replace `todo!()`:

```rust
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
```

**Step 3: Write pipeline_bridge**

Create `crates/core/src/editor/pipeline_bridge.rs`:

```rust
//! Convert pipeline output to editor arrangements.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use super::bank_builder::build_bank_from_syllables;
use super::types::*;
use crate::types::Syllable;

/// Convert collage pipeline data into an editor arrangement.
///
/// Takes the aligned syllables and source audio from the collage pipeline,
/// builds a full bank, and optionally populates the timeline from selected clips.
pub fn arrangement_from_collage(
    all_syllables: &HashMap<String, Vec<Syllable>>,
    source_audio: &HashMap<String, (Vec<f64>, u32)>,
    selected_syllable_indices: Option<&[(String, usize)]>,
) -> Result<Arrangement> {
    // Build bank from all syllables
    let syllable_pairs: Vec<(Syllable, PathBuf)> = all_syllables
        .iter()
        .flat_map(|(source, syls)| {
            syls.iter()
                .map(move |s| (s.clone(), PathBuf::from(source)))
        })
        .collect();

    let source_audio_pathbuf: HashMap<PathBuf, (Vec<f64>, u32)> = source_audio
        .iter()
        .map(|(k, v)| (PathBuf::from(k), v.clone()))
        .collect();

    let bank = build_bank_from_syllables(&syllable_pairs, &source_audio_pathbuf)?;

    let mut arr = Arrangement::new(16000, EditorPipelineMode::Collage);

    // Populate timeline if selected indices provided
    if let Some(indices) = selected_syllable_indices {
        for (source, idx) in indices {
            // Find matching bank clip by source path and syllable index
            if let Some(bank_clip) = bank.iter().find(|c| {
                c.source_path == PathBuf::from(source)
                    && c.syllable.word_index == *idx
            }) {
                arr.timeline.push(TimelineClip::new(bank_clip));
            }
        }
        arr.relayout(0.0);
    }

    arr.bank = bank;
    Ok(arr)
}

/// Create an empty arrangement with a populated bank for blank canvas mode.
pub fn arrangement_blank_canvas(
    all_syllables: &HashMap<String, Vec<Syllable>>,
    source_audio: &HashMap<String, (Vec<f64>, u32)>,
    pipeline: EditorPipelineMode,
) -> Result<Arrangement> {
    let syllable_pairs: Vec<(Syllable, PathBuf)> = all_syllables
        .iter()
        .flat_map(|(source, syls)| {
            syls.iter()
                .map(move |s| (s.clone(), PathBuf::from(source)))
        })
        .collect();

    let source_audio_pathbuf: HashMap<PathBuf, (Vec<f64>, u32)> = source_audio
        .iter()
        .map(|(k, v)| (PathBuf::from(k), v.clone()))
        .collect();

    let bank = build_bank_from_syllables(&syllable_pairs, &source_audio_pathbuf)?;

    let mut arr = Arrangement::new(16000, pipeline);
    arr.bank = bank;
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Phoneme;

    fn make_test_data() -> (HashMap<String, Vec<Syllable>>, HashMap<String, (Vec<f64>, u32)>) {
        let mut syllables = HashMap::new();
        syllables.insert(
            "test.wav".to_string(),
            vec![
                Syllable {
                    phonemes: vec![Phoneme { label: "HH".into(), start: 0.0, end: 0.15 }],
                    start: 0.0,
                    end: 0.15,
                    word: "hello".into(),
                    word_index: 0,
                },
                Syllable {
                    phonemes: vec![Phoneme { label: "AH0".into(), start: 0.15, end: 0.3 }],
                    start: 0.15,
                    end: 0.3,
                    word: "hello".into(),
                    word_index: 0,
                },
            ],
        );

        let mut audio = HashMap::new();
        audio.insert("test.wav".to_string(), (vec![0.5f64; 16000], 16000u32));

        (syllables, audio)
    }

    #[test]
    fn test_blank_canvas() {
        let (syllables, audio) = make_test_data();
        let arr = arrangement_blank_canvas(&syllables, &audio, EditorPipelineMode::Collage).unwrap();
        assert_eq!(arr.bank.len(), 2);
        assert!(arr.timeline.is_empty());
        assert_eq!(arr.source_pipeline, EditorPipelineMode::Collage);
    }

    #[test]
    fn test_arrangement_from_collage_no_selection() {
        let (syllables, audio) = make_test_data();
        let arr = arrangement_from_collage(&syllables, &audio, None).unwrap();
        assert_eq!(arr.bank.len(), 2);
        assert!(arr.timeline.is_empty());
    }
}
```

Add to `crates/core/src/editor/mod.rs`:

```rust
pub mod bank_builder;
pub mod pipeline_bridge;
```

**Step 4: Run tests**

Run: `cargo test -p glottisdale-core editor -- --nocapture`
Expected: All PASS

**Step 5: Commit**

```bash
git add crates/core/src/editor/
git commit -m "feat(editor): add pipeline bridge and bank builder"
```

---

## Task 5: Non-Blocking Playback Engine

**Files:**
- Create: `crates/core/src/editor/playback_engine.rs`
- Modify: `crates/core/src/editor/mod.rs`

**Step 1: Write the playback engine**

Create `crates/core/src/editor/playback_engine.rs`:

```rust
//! Non-blocking audio playback engine for the editor.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use rodio::{OutputStream, Sink};

use super::types::ClipId;
use crate::audio::playback::F64Source;

/// Command sent to the playback thread.
pub enum PlaybackCommand {
    /// Play samples from a cursor position.
    PlaySamples {
        samples: Vec<f64>,
        sample_rate: u32,
        start_cursor_s: f64,
    },
    /// Pause playback.
    Pause,
    /// Resume playback.
    Resume,
    /// Stop playback and reset cursor.
    Stop,
}

/// Shared playback state readable from the GUI thread.
#[derive(Clone)]
pub struct PlaybackState {
    /// Current playback cursor in seconds.
    pub cursor_s: Arc<Mutex<f64>>,
    /// Whether currently playing.
    pub is_playing: Arc<Mutex<bool>>,
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            cursor_s: Arc::new(Mutex::new(0.0)),
            is_playing: Arc::new(Mutex::new(false)),
        }
    }

    pub fn get_cursor(&self) -> f64 {
        *self.cursor_s.lock().unwrap()
    }

    pub fn is_playing(&self) -> bool {
        *self.is_playing.lock().unwrap()
    }
}

/// Non-blocking playback engine.
///
/// Owns a dedicated audio thread. Send commands via `send()`,
/// read cursor/playing state from `state`.
pub struct PlaybackEngine {
    command_tx: mpsc::Sender<PlaybackCommand>,
    pub state: PlaybackState,
}

impl PlaybackEngine {
    /// Create a new playback engine and spawn the audio thread.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let state = PlaybackState::new();

        let thread_state = state.clone();
        std::thread::Builder::new()
            .name("playback-engine".into())
            .spawn(move || {
                playback_thread(rx, thread_state);
            })
            .expect("Failed to spawn playback thread");

        Self {
            command_tx: tx,
            state,
        }
    }

    /// Send a command to the playback engine.
    pub fn send(&self, cmd: PlaybackCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Play audio samples starting at a given cursor position.
    pub fn play_samples(&self, samples: Vec<f64>, sample_rate: u32, start_cursor_s: f64) {
        self.send(PlaybackCommand::PlaySamples {
            samples,
            sample_rate,
            start_cursor_s,
        });
    }

    /// Stop playback.
    pub fn stop(&self) {
        self.send(PlaybackCommand::Stop);
    }

    /// Pause playback.
    pub fn pause(&self) {
        self.send(PlaybackCommand::Pause);
    }

    /// Resume playback.
    pub fn resume(&self) {
        self.send(PlaybackCommand::Resume);
    }
}

fn playback_thread(rx: mpsc::Receiver<PlaybackCommand>, state: PlaybackState) {
    // Try to open audio output; if it fails, the thread just consumes commands
    let audio = OutputStream::try_default().ok();
    let sink = audio
        .as_ref()
        .and_then(|(_, handle)| Sink::try_new(handle).ok());

    let mut play_start: Option<(Instant, f64)> = None; // (wall_start, cursor_start)
    let mut sample_rate = 16000u32;

    loop {
        // Process all pending commands (non-blocking)
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlaybackCommand::PlaySamples {
                    samples,
                    sample_rate: sr,
                    start_cursor_s,
                } => {
                    sample_rate = sr;
                    if let Some(ref sink) = sink {
                        sink.stop();
                        let source = crate::audio::playback::make_f64_source(samples, sr);
                        sink.append(source);
                        sink.play();
                        play_start = Some((Instant::now(), start_cursor_s));
                        *state.is_playing.lock().unwrap() = true;
                    }
                }
                PlaybackCommand::Pause => {
                    if let Some(ref sink) = sink {
                        sink.pause();
                        *state.is_playing.lock().unwrap() = false;
                    }
                }
                PlaybackCommand::Resume => {
                    if let Some(ref sink) = sink {
                        sink.play();
                        *state.is_playing.lock().unwrap() = true;
                    }
                }
                PlaybackCommand::Stop => {
                    if let Some(ref sink) = sink {
                        sink.stop();
                    }
                    play_start = None;
                    *state.is_playing.lock().unwrap() = false;
                    *state.cursor_s.lock().unwrap() = 0.0;
                }
            }
        }

        // Update cursor position
        if let Some((start_instant, start_cursor)) = play_start {
            if let Some(ref sink) = sink {
                if sink.empty() {
                    // Playback finished
                    *state.is_playing.lock().unwrap() = false;
                    play_start = None;
                } else if !sink.is_paused() {
                    let elapsed = start_instant.elapsed().as_secs_f64();
                    *state.cursor_s.lock().unwrap() = start_cursor + elapsed;
                }
            }
        }

        // Sleep briefly to avoid busy-spinning
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Check if the channel is closed (sender dropped)
        if rx.try_recv() == Err(mpsc::TryRecvError::Disconnected) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_state_defaults() {
        let state = PlaybackState::new();
        assert_eq!(state.get_cursor(), 0.0);
        assert!(!state.is_playing());
    }

    #[test]
    fn test_playback_engine_creation() {
        // Just verify it doesn't panic
        let engine = PlaybackEngine::new();
        assert!(!engine.state.is_playing());
        engine.stop();
    }
}
```

This requires making `F64Source` public. Modify `crates/core/src/audio/playback.rs`:

Change `struct F64Source` to `pub struct F64Source` and add a public constructor:

```rust
/// Create an F64Source for use in other modules.
pub fn make_f64_source(samples: Vec<f64>, sample_rate: u32) -> F64Source {
    F64Source::new(samples, sample_rate)
}
```

Also change `F64Source` visibility:

```rust
pub struct F64Source {
```

Add to `crates/core/src/editor/mod.rs`:

```rust
pub mod playback_engine;
```

**Step 2: Run tests**

Run: `cargo test -p glottisdale-core editor::playback_engine -- --nocapture`
Expected: PASS

**Step 3: Run full test suite**

Run: `cargo test --workspace`
Expected: All pass

**Step 4: Commit**

```bash
git add crates/core/src/editor/playback_engine.rs \
  crates/core/src/editor/mod.rs \
  crates/core/src/audio/playback.rs
git commit -m "feat(editor): add non-blocking playback engine"
```

---

## Task 6: GUI Editor — Timeline Widget with Waveform Display

**Files:**
- Create: `crates/gui/src/editor/mod.rs`
- Create: `crates/gui/src/editor/timeline.rs`
- Create: `crates/gui/src/editor/waveform_painter.rs`
- Modify: `crates/gui/src/main.rs`
- Modify: `crates/gui/src/app.rs`
- Modify: `crates/gui/Cargo.toml`

**Step 1: Add uuid dependency to GUI**

In `crates/gui/Cargo.toml`, add:

```toml
uuid = { version = "1", features = ["v4"] }
```

**Step 2: Create waveform painter**

Create `crates/gui/src/editor/waveform_painter.rs`:

```rust
//! Custom egui painting for waveform thumbnails.

use eframe::egui;
use glottisdale_core::editor::WaveformData;

/// Paint a waveform inside a rectangle.
///
/// Draws vertical lines from min_peak to max_peak per pixel column.
pub fn paint_waveform(
    painter: &egui::Painter,
    rect: egui::Rect,
    waveform: &WaveformData,
    color: egui::Color32,
) {
    let n_buckets = waveform.peaks.len();
    if n_buckets == 0 || rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }

    let mid_y = rect.center().y;
    let half_height = rect.height() * 0.45;
    let px_per_bucket = rect.width() / n_buckets as f32;

    if px_per_bucket >= 1.0 {
        // One or more pixels per bucket: draw each bucket
        for (i, &(min_peak, max_peak)) in waveform.peaks.iter().enumerate() {
            let x = rect.left() + (i as f32 + 0.5) * px_per_bucket;
            let y_top = mid_y - max_peak * half_height;
            let y_bot = mid_y - min_peak * half_height;
            painter.line_segment(
                [egui::pos2(x, y_top), egui::pos2(x, y_bot)],
                egui::Stroke::new(px_per_bucket.max(1.0), color),
            );
        }
    } else {
        // Multiple buckets per pixel: composite min/max
        let buckets_per_px = (1.0 / px_per_bucket).ceil() as usize;
        let n_pixels = rect.width() as usize;
        for px in 0..n_pixels {
            let bucket_start = (px as f32 / rect.width() * n_buckets as f32) as usize;
            let bucket_end = ((px + 1) as f32 / rect.width() * n_buckets as f32).ceil() as usize;
            let bucket_end = bucket_end.min(n_buckets);

            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for i in bucket_start..bucket_end {
                let (lo, hi) = waveform.peaks[i];
                if lo < min { min = lo; }
                if hi > max { max = hi; }
            }

            if min <= max {
                let x = rect.left() + px as f32 + 0.5;
                let y_top = mid_y - max * half_height;
                let y_bot = mid_y - min * half_height;
                painter.line_segment(
                    [egui::pos2(x, y_top), egui::pos2(x, y_bot)],
                    egui::Stroke::new(1.0, color),
                );
            }
        }
    }
}

/// Paint a clip block on the timeline.
///
/// Draws a rounded rectangle background with a waveform inside
/// and a label above.
pub fn paint_clip_block(
    painter: &egui::Painter,
    rect: egui::Rect,
    waveform: &WaveformData,
    label: &str,
    bg_color: egui::Color32,
    waveform_color: egui::Color32,
    selected: bool,
) {
    // Background
    let rounding = egui::Rounding::same(3.0);
    painter.rect_filled(rect, rounding, bg_color);

    // Selection border
    if selected {
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 255)),
        );
    }

    // Waveform (inside the block, with padding)
    let waveform_rect = rect.shrink2(egui::vec2(2.0, 10.0));
    if waveform_rect.width() > 2.0 && waveform_rect.height() > 2.0 {
        paint_waveform(painter, waveform_rect, waveform, waveform_color);
    }

    // Label at top
    let label_pos = egui::pos2(rect.left() + 3.0, rect.top() + 1.0);
    let font = egui::FontId::proportional(9.0);
    let galley = painter.layout_no_wrap(label.to_string(), font, egui::Color32::WHITE);
    // Clip label to block width
    painter.galley(label_pos, galley, egui::Color32::WHITE);
}
```

**Step 3: Create timeline widget**

Create `crates/gui/src/editor/timeline.rs`:

```rust
//! Timeline widget — custom egui painting with zoom/pan and clip layout.

use eframe::egui;
use glottisdale_core::editor::{Arrangement, ClipId, TimelineClip};

use super::waveform_painter::paint_clip_block;

/// Colors for clips from different source files.
const SOURCE_COLORS: &[(u8, u8, u8)] = &[
    (70, 130, 180),  // steel blue
    (180, 100, 60),  // terracotta
    (80, 160, 80),   // green
    (160, 80, 160),  // purple
    (180, 160, 50),  // gold
    (80, 160, 160),  // teal
];

/// Visual and interaction state for the timeline.
pub struct TimelineState {
    /// Pixels per second (zoom level).
    pub pixels_per_second: f64,
    /// Scroll offset in seconds (left edge of visible area).
    pub scroll_offset_s: f64,
    /// Track height in pixels.
    pub track_height: f32,
    /// Playback cursor position in seconds.
    pub cursor_s: f64,
    /// Selected clip IDs.
    pub selected: Vec<ClipId>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            pixels_per_second: 200.0,
            scroll_offset_s: 0.0,
            track_height: 80.0,
            cursor_s: 0.0,
            selected: Vec::new(),
        }
    }
}

impl TimelineState {
    /// Convert time to pixel x coordinate.
    pub fn time_to_px(&self, time_s: f64) -> f32 {
        ((time_s - self.scroll_offset_s) * self.pixels_per_second) as f32
    }

    /// Convert pixel x to time.
    pub fn px_to_time(&self, px: f32) -> f64 {
        px as f64 / self.pixels_per_second + self.scroll_offset_s
    }

    /// Check if a clip ID is selected.
    pub fn is_selected(&self, id: ClipId) -> bool {
        self.selected.contains(&id)
    }

    /// Handle zoom (ctrl+scroll).
    pub fn handle_zoom(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if response.hovered() && ui.input(|i| i.modifiers.command) {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > 0.0 {
                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let time_at_mouse = self.px_to_time(mouse_pos.x - response.rect.left());
                    let zoom_factor = 1.0 + scroll_y as f64 * 0.003;
                    self.pixels_per_second =
                        (self.pixels_per_second * zoom_factor).clamp(10.0, 5000.0);
                    // Keep time_at_mouse at the same pixel position
                    let new_px = mouse_pos.x - response.rect.left();
                    self.scroll_offset_s = time_at_mouse - new_px as f64 / self.pixels_per_second;
                }
            }
        }
    }

    /// Handle pan (scroll without modifier).
    pub fn handle_pan(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if response.hovered() && !ui.input(|i| i.modifiers.command) {
            let scroll_x = ui.input(|i| i.smooth_scroll_delta.x);
            if scroll_x.abs() > 0.0 {
                self.scroll_offset_s -= scroll_x as f64 / self.pixels_per_second;
                self.scroll_offset_s = self.scroll_offset_s.max(0.0);
            }
        }
    }
}

/// Get a color for a source file index.
fn source_color(index: usize) -> egui::Color32 {
    let (r, g, b) = SOURCE_COLORS[index % SOURCE_COLORS.len()];
    egui::Color32::from_rgb(r, g, b)
}

/// Paint the timeline with all clips.
pub fn show_timeline(
    ui: &mut egui::Ui,
    arrangement: &Arrangement,
    state: &mut TimelineState,
    source_file_indices: &std::collections::HashMap<std::path::PathBuf, usize>,
) -> egui::Response {
    let desired_size = egui::vec2(ui.available_width(), state.track_height + 20.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

    if !ui.is_rect_visible(rect) {
        return response;
    }

    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(30));

    // Track area
    let track_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + 16.0),
        egui::vec2(rect.width(), state.track_height),
    );

    // Time ruler at top
    paint_time_ruler(&painter, egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 16.0)), state);

    // Paint clips
    for tc in &arrangement.timeline {
        let clip_left = state.time_to_px(tc.position_s) + rect.left();
        let clip_width = (tc.effective_duration_s * state.pixels_per_second) as f32;
        let clip_right = clip_left + clip_width;

        // Skip if not visible
        if clip_right < rect.left() || clip_left > rect.right() {
            continue;
        }

        let clip_rect = egui::Rect::from_min_size(
            egui::pos2(clip_left, track_rect.top()),
            egui::vec2(clip_width, state.track_height),
        );

        if let Some(bank_clip) = arrangement.get_bank_clip(tc.source_clip_id) {
            let src_idx = source_file_indices
                .get(&bank_clip.source_path)
                .copied()
                .unwrap_or(0);
            let bg = source_color(src_idx).gamma_multiply(0.3);
            let wf_color = source_color(src_idx);

            paint_clip_block(
                &painter,
                clip_rect,
                &bank_clip.waveform,
                &bank_clip.label,
                bg,
                wf_color,
                state.is_selected(tc.id),
            );
        }
    }

    // Playback cursor
    let cursor_x = state.time_to_px(state.cursor_s) + rect.left();
    if cursor_x >= rect.left() && cursor_x <= rect.right() {
        painter.line_segment(
            [
                egui::pos2(cursor_x, rect.top()),
                egui::pos2(cursor_x, rect.bottom()),
            ],
            egui::Stroke::new(2.0, egui::Color32::RED),
        );
    }

    // Handle zoom and pan
    state.handle_zoom(ui, &response);
    state.handle_pan(ui, &response);

    // Handle click to select/set cursor
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let click_time = state.px_to_time(pos.x - rect.left());

            // Check if clicked on a clip
            let mut clicked_clip = None;
            for tc in &arrangement.timeline {
                let clip_end = tc.position_s + tc.effective_duration_s;
                if click_time >= tc.position_s && click_time <= clip_end {
                    clicked_clip = Some(tc.id);
                    break;
                }
            }

            if let Some(clip_id) = clicked_clip {
                let shift = ui.input(|i| i.modifiers.shift || i.modifiers.command);
                if shift {
                    // Toggle in multi-selection
                    if let Some(idx) = state.selected.iter().position(|&id| id == clip_id) {
                        state.selected.remove(idx);
                    } else {
                        state.selected.push(clip_id);
                    }
                } else {
                    state.selected = vec![clip_id];
                }
            } else {
                // Click on empty space: set cursor, deselect
                state.cursor_s = click_time.max(0.0);
                state.selected.clear();
            }
        }
    }

    response
}

/// Paint time markers along the top of the timeline.
fn paint_time_ruler(painter: &egui::Painter, rect: egui::Rect, state: &TimelineState) {
    let font = egui::FontId::proportional(9.0);
    let color = egui::Color32::from_gray(150);

    // Determine tick interval based on zoom
    let tick_interval = if state.pixels_per_second > 500.0 {
        0.1
    } else if state.pixels_per_second > 100.0 {
        0.5
    } else if state.pixels_per_second > 20.0 {
        1.0
    } else {
        5.0
    };

    let start_time = (state.scroll_offset_s / tick_interval).floor() * tick_interval;
    let end_time = state.px_to_time(rect.width());

    let mut t = start_time;
    while t <= end_time {
        let x = state.time_to_px(t) + rect.left();
        if x >= rect.left() && x <= rect.right() {
            // Tick line
            painter.line_segment(
                [egui::pos2(x, rect.bottom() - 4.0), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, color),
            );
            // Label
            let label = if tick_interval >= 1.0 {
                format!("{:.0}s", t)
            } else {
                format!("{:.1}s", t)
            };
            let galley = painter.layout_no_wrap(label, font.clone(), color);
            painter.galley(egui::pos2(x + 2.0, rect.top()), galley, color);
        }
        t += tick_interval;
    }
}
```

**Step 4: Create editor module entry point**

Create `crates/gui/src/editor/mod.rs`:

```rust
//! Interactive syllable editor GUI.

pub mod timeline;
pub mod waveform_painter;

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use glottisdale_core::editor::{
    Arrangement, ClipEffect, ClipId, EditorPipelineMode,
    effects_chain::compute_effective_duration,
    playback_engine::PlaybackEngine,
    render::render_arrangement,
};

use self::timeline::TimelineState;

/// Full editor state.
pub struct EditorState {
    pub arrangement: Arrangement,
    pub timeline: TimelineState,
    pub playback: PlaybackEngine,
    /// Map from source file path to color index.
    pub source_indices: HashMap<PathBuf, usize>,
    /// Search filter for the bank panel.
    pub bank_filter: String,
}

impl EditorState {
    pub fn new(arrangement: Arrangement) -> Self {
        // Build source index map
        let mut source_indices = HashMap::new();
        let mut next_idx = 0usize;
        for clip in &arrangement.bank {
            source_indices.entry(clip.source_path.clone()).or_insert_with(|| {
                let idx = next_idx;
                next_idx += 1;
                idx
            });
        }

        Self {
            arrangement,
            timeline: TimelineState::default(),
            playback: PlaybackEngine::new(),
            source_indices,
            bank_filter: String::new(),
        }
    }

    /// Shuffle the selected clips randomly.
    pub fn shuffle_selected(&mut self) {
        use rand::seq::SliceRandom;
        let selected = self.timeline.selected.clone();
        if selected.len() < 2 {
            return;
        }

        let mut indices: Vec<usize> = self
            .arrangement
            .timeline
            .iter()
            .enumerate()
            .filter(|(_, tc)| selected.contains(&tc.id))
            .map(|(i, _)| i)
            .collect();

        let mut rng = rand::thread_rng();
        let original_clips: Vec<_> = indices
            .iter()
            .map(|&i| self.arrangement.timeline[i].clone())
            .collect();

        let mut shuffled = original_clips.clone();
        shuffled.shuffle(&mut rng);

        for (slot, clip) in indices.iter().zip(shuffled.into_iter()) {
            self.arrangement.timeline[*slot] = clip;
        }

        self.arrangement.relayout(0.0);
    }

    /// Delete selected clips from the timeline.
    pub fn delete_selected(&mut self) {
        let selected = &self.timeline.selected;
        self.arrangement
            .timeline
            .retain(|tc| !selected.contains(&tc.id));
        self.timeline.selected.clear();
        self.arrangement.relayout(0.0);
    }

    /// Apply an effect to all selected clips.
    pub fn apply_effect_to_selected(&mut self, effect: ClipEffect) {
        let selected = &self.timeline.selected;
        for tc in &mut self.arrangement.timeline {
            if selected.contains(&tc.id) {
                tc.effects.push(effect.clone());
                if let Some(source) = self.arrangement.bank.iter().find(|c| c.id == tc.source_clip_id) {
                    tc.effective_duration_s =
                        compute_effective_duration(source.duration_s(), &tc.effects);
                }
            }
        }
        self.arrangement.relayout(0.0);
    }

    /// Clear all effects from selected clips.
    pub fn clear_effects_selected(&mut self) {
        let selected = &self.timeline.selected;
        for tc in &mut self.arrangement.timeline {
            if selected.contains(&tc.id) {
                tc.effects.clear();
                if let Some(source) = self.arrangement.bank.iter().find(|c| c.id == tc.source_clip_id) {
                    tc.effective_duration_s = source.duration_s();
                }
            }
        }
        self.arrangement.relayout(0.0);
    }

    /// Play the arrangement from the current cursor position.
    pub fn play_from_cursor(&self) {
        if let Ok(samples) = render_arrangement(&self.arrangement) {
            let sr = self.arrangement.sample_rate;
            let cursor = self.timeline.cursor_s;
            let start_sample = (cursor * sr as f64).round() as usize;
            let play_samples = if start_sample < samples.len() {
                samples[start_sample..].to_vec()
            } else {
                vec![]
            };
            self.playback.play_samples(play_samples, sr, cursor);
        }
    }

    /// Play a single bank clip (preview).
    pub fn play_clip(&self, clip_id: ClipId) {
        if let Some(clip) = self.arrangement.get_bank_clip(clip_id) {
            self.playback
                .play_samples(clip.samples.clone(), clip.sample_rate, 0.0);
        }
    }
}

/// Main entry point: render the full editor UI.
pub fn show_editor(ui: &mut egui::Ui, state: &mut EditorState, ctx: &egui::Context) -> bool {
    let mut close = false;

    // Update cursor from playback engine
    state.timeline.cursor_s = state.playback.state.get_cursor();
    if state.playback.state.is_playing() {
        ctx.request_repaint();
    }

    // Toolbar
    ui.horizontal(|ui| {
        if ui.button("Close Editor").clicked() {
            close = true;
        }
        ui.separator();

        let has_selection = !state.timeline.selected.is_empty();

        if ui.add_enabled(has_selection, egui::Button::new("Shuffle")).clicked() {
            state.shuffle_selected();
        }
        if ui.add_enabled(has_selection, egui::Button::new("Delete")).clicked() {
            state.delete_selected();
        }
        if ui.add_enabled(has_selection, egui::Button::new("Clear FX")).clicked() {
            state.clear_effects_selected();
        }

        ui.separator();

        // Playback controls
        let playing = state.playback.state.is_playing();
        if ui.button(if playing { "Pause" } else { "Play" }).clicked() {
            if playing {
                state.playback.pause();
            } else {
                state.play_from_cursor();
            }
        }
        if ui.button("Stop").clicked() {
            state.playback.stop();
        }

        ui.separator();

        // Zoom
        ui.label("Zoom:");
        if ui.button("-").clicked() {
            state.timeline.pixels_per_second =
                (state.timeline.pixels_per_second * 0.7).max(10.0);
        }
        if ui.button("+").clicked() {
            state.timeline.pixels_per_second =
                (state.timeline.pixels_per_second * 1.4).min(5000.0);
        }
        ui.label(format!("{:.0} px/s", state.timeline.pixels_per_second));

        ui.separator();

        // Export
        if ui.button("Export WAV").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("arrangement.wav")
                .add_filter("WAV audio", &["wav"])
                .save_file()
            {
                if let Err(e) = glottisdale_core::editor::render::export_arrangement(
                    &state.arrangement,
                    &path,
                ) {
                    log::error!("Export failed: {}", e);
                }
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let n_clips = state.arrangement.timeline.len();
            let dur = state.arrangement.total_duration_s();
            ui.label(format!("{} clips | {:.1}s", n_clips, dur));
        });
    });

    ui.separator();

    // Main area: bank panel on left, timeline on right
    egui::SidePanel::left("editor_bank")
        .min_width(150.0)
        .default_width(200.0)
        .resizable(true)
        .show_inside(ui, |ui| {
            show_bank_panel(ui, state);
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            timeline::show_timeline(
                ui,
                &state.arrangement,
                &mut state.timeline,
                &state.source_indices,
            );
        });
    });

    close
}

/// Show the syllable bank/palette panel.
fn show_bank_panel(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.heading("Syllable Bank");
    ui.add(
        egui::TextEdit::singleline(&mut state.bank_filter)
            .hint_text("Filter...")
            .desired_width(ui.available_width()),
    );
    ui.separator();

    let filter = state.bank_filter.to_lowercase();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for clip in &state.arrangement.bank {
            // Filter
            if !filter.is_empty()
                && !clip.label.to_lowercase().contains(&filter)
                && !clip.syllable.word.to_lowercase().contains(&filter)
            {
                continue;
            }

            let response = ui.horizontal(|ui| {
                // Mini waveform
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(40.0, 24.0), egui::Sense::click());
                if ui.is_rect_visible(rect) {
                    let src_idx = state
                        .source_indices
                        .get(&clip.source_path)
                        .copied()
                        .unwrap_or(0);
                    let color = timeline::SOURCE_COLORS[src_idx % timeline::SOURCE_COLORS.len()];
                    waveform_painter::paint_waveform(
                        ui.painter(),
                        rect,
                        &clip.waveform,
                        egui::Color32::from_rgb(color.0, color.1, color.2),
                    );
                }

                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&clip.label)
                            .small()
                            .monospace(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} ({:.2}s)",
                            clip.syllable.word,
                            clip.duration_s()
                        ))
                        .small()
                        .weak(),
                    );
                });

                resp
            })
            .inner;

            // Double-click to preview
            if response.double_clicked() {
                state.play_clip(clip.id);
            }

            // Drag from bank to timeline
            // (basic version: click "Add" button instead of full drag-drop)
            if response.clicked() {
                // Add to end of timeline
                let mut tc = glottisdale_core::editor::TimelineClip::new(clip);
                state.arrangement.timeline.push(tc);
                state.arrangement.relayout(0.0);
            }
        }
    });
}
```

**Step 5: Wire editor into the main GUI**

Add to `crates/gui/src/main.rs` (after `mod app;`):

```rust
mod editor;
```

In `crates/gui/src/app.rs`, add a field to `GlottisdaleApp`:

```rust
/// Editor state (None = editor not open)
editor: Option<editor::EditorState>,
```

Initialize as `None` in the constructor. In the `update()` method's central panel, add a check:

```rust
// In the central panel:
if let Some(ref mut editor_state) = self.editor {
    if editor::show_editor(ui, editor_state, ctx) {
        self.editor = None; // Close editor
    }
} else {
    // existing workspace UI...
}
```

**Step 6: Build and verify compilation**

Run: `cargo build -p glottisdale-gui`
Expected: Compiles successfully

**Step 7: Commit**

```bash
git add crates/gui/Cargo.toml crates/gui/src/main.rs \
  crates/gui/src/app.rs crates/gui/src/editor/
git commit -m "feat(gui): add timeline widget with waveform display and bank panel"
```

---

## Task 7: GUI Editor — Context Menus and Drag-Drop

**Files:**
- Modify: `crates/gui/src/editor/mod.rs`
- Modify: `crates/gui/src/editor/timeline.rs`

**Step 1: Add right-click context menu to timeline**

In `crates/gui/src/editor/timeline.rs`, after the click handling in `show_timeline()`, add secondary click (right-click) handling:

```rust
if response.secondary_clicked() {
    if let Some(pos) = response.interact_pointer_pos() {
        let click_time = state.px_to_time(pos.x - rect.left());
        for tc in &arrangement.timeline {
            let clip_end = tc.position_s + tc.effective_duration_s;
            if click_time >= tc.position_s && click_time <= clip_end {
                // Store context menu state
                // (Implementation: use egui::popup to show menu)
                break;
            }
        }
    }
}
```

In `crates/gui/src/editor/mod.rs`, add context menu rendering using `egui::popup_below_widget` or `ui.menu_button` with sub-menus for stutter count, stretch factor, and pitch shift semitones.

**Step 2: Add drag-to-reorder on timeline**

In the timeline widget, detect drag on clips. After a 3px threshold, enter reorder mode:
- Ghost the dragged clip at its original position (semi-transparent)
- Show insertion preview at the drop position
- On release, remove from old position, insert at new position, relayout

**Step 3: Build and test manually**

Run: `cargo run -p glottisdale-gui`
Test: Right-click clips, verify menus appear with working effects. Drag clips to reorder.

**Step 4: Commit**

```bash
git add crates/gui/src/editor/
git commit -m "feat(gui): add context menus and drag-to-reorder on timeline"
```

---

## Task 8: Pipeline Integration — Wire Up Pipelines to Open Editor

**Files:**
- Modify: `crates/gui/src/app.rs`

**Step 1: Add "Edit" button to pipeline output**

After each pipeline finishes (status = Done), show an "Edit Arrangement" button next to the output. Clicking it:
1. Runs alignment on source files (if not already done)
2. Builds the bank from aligned syllables
3. Populates the timeline from the pipeline's clip selection
4. Sets `self.editor = Some(EditorState::new(arrangement))`

**Step 2: Add "Build Bank" button for blank canvas mode**

Add a "Build Bank & Edit" button in each pipeline workspace. Clicking it:
1. Runs alignment only (no full pipeline)
2. Builds the bank
3. Creates an empty arrangement
4. Opens the editor with empty timeline

**Step 3: Store intermediate alignment data**

Extend `ProcessingState` to optionally hold alignment data (`HashMap<String, Vec<Syllable>>` and `HashMap<String, (Vec<f64>, u32)>`) so the editor can access it after pipeline completion.

**Step 4: Build and test manually**

Run: `cargo run -p glottisdale-gui`
Test:
- Run a collage, click "Edit Arrangement", verify editor opens with clips
- Click "Build Bank & Edit", verify editor opens with populated bank but empty timeline

**Step 5: Commit**

```bash
git add crates/gui/src/app.rs
git commit -m "feat(gui): wire pipelines to open editor with arrangement data"
```

---

## Task 9: Documentation and Version Bump

**Files:**
- Modify: `README.md`
- Modify: `docs/reference/architecture.md`
- Modify: `docs/getting-started/quickstart.md`
- Modify: `Cargo.toml`

**Step 1: Update README**

Add a section describing the interactive editor feature — what it does, how to access it (post-pipeline "Edit" button or "Build Bank" blank canvas mode), and its capabilities (waveform display, drag arrangement, effects, export).

**Step 2: Update architecture.md**

Add the `editor` module to the module map table:

```
| `editor::types` | SyllableClip, TimelineClip, Arrangement, ClipEffect |
| `editor::waveform` | Pre-computed waveform peak data |
| `editor::effects_chain` | Non-destructive effects processing |
| `editor::render` | Arrangement renderer and WAV export |
| `editor::pipeline_bridge` | Pipeline output to editor arrangement conversion |
| `editor::playback_engine` | Non-blocking audio playback with cursor tracking |
```

Update the GUI description to mention the editor.

**Step 3: Update quickstart**

Add a brief section after each pipeline's output description explaining that you can click "Edit" to open the interactive editor and tweak the arrangement.

**Step 4: Bump version to 0.4.0**

In `Cargo.toml`:
```toml
version = "0.4.0"
```

**Step 5: Run full test suite**

Run: `cargo test --workspace`
Expected: All pass

**Step 6: Commit**

```bash
git add README.md docs/ Cargo.toml
git commit -m "docs: add interactive syllable editor documentation, bump to 0.4.0"
```

---

## Dependency Graph and Parallelism

```
Task 1 (types + waveform) ─────┬──> Task 2 (effects chain) ──> Task 3 (renderer)
                                |
                                └──> Task 4 (pipeline bridge + bank builder)
                                |
                                └──> Task 5 (playback engine)

[Tasks 2, 4, 5 can run in parallel after Task 1]
[Task 3 depends on Task 2]
[Task 6 depends on Tasks 1-5]

Task 6 (GUI timeline + bank) ──> Task 7 (context menus + drag) ──> Task 8 (pipeline wiring) ──> Task 9 (docs)
```

**Parallelizable groups:**
- **Group A**: Task 2 (effects chain) + Task 3 (renderer) — sequential within group
- **Group B**: Task 4 (pipeline bridge)
- **Group C**: Task 5 (playback engine)

Groups A, B, C can all run in parallel after Task 1 completes.
