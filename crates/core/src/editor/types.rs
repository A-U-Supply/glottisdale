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
    Reverse,
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
