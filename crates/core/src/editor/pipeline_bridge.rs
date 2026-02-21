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
