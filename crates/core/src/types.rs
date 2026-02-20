use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single phoneme with timing information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Phoneme {
    /// ARPABET label (e.g. "AH0") or IPA if from BFA
    pub label: String,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
}

/// A group of phonemes forming one syllable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Syllable {
    pub phonemes: Vec<Phoneme>,
    /// First phoneme start (seconds)
    pub start: f64,
    /// Last phoneme end (seconds)
    pub end: f64,
    /// Parent word text
    pub word: String,
    /// Position in transcript
    pub word_index: usize,
}

/// An audio clip containing one or more syllables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub syllables: Vec<Syllable>,
    /// Start time with padding applied (seconds)
    pub start: f64,
    /// End time with padding applied (seconds)
    pub end: f64,
    /// Input filename
    pub source: String,
    #[serde(default)]
    pub output_path: PathBuf,
}

/// Output of a glottisdale pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub clips: Vec<Clip>,
    pub concatenated: PathBuf,
    pub transcript: String,
    pub manifest: serde_json::Value,
}

/// Word with timing from Whisper transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordTimestamp {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// Result of Whisper transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub words: Vec<WordTimestamp>,
    pub language: String,
}

/// Result of alignment (transcription + syllabification).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentResult {
    pub text: String,
    pub words: Vec<WordTimestamp>,
    pub syllables: Vec<Syllable>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phoneme_creation() {
        let p = Phoneme {
            label: "AH0".to_string(),
            start: 0.1,
            end: 0.2,
        };
        assert_eq!(p.label, "AH0");
        assert!((p.start - 0.1).abs() < f64::EPSILON);
        assert!((p.end - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_syllable_creation() {
        let p1 = Phoneme { label: "HH".into(), start: 0.1, end: 0.15 };
        let p2 = Phoneme { label: "AH0".into(), start: 0.15, end: 0.25 };
        let syl = Syllable {
            phonemes: vec![p1, p2],
            start: 0.1,
            end: 0.25,
            word: "hello".into(),
            word_index: 0,
        };
        assert_eq!(syl.phonemes.len(), 2);
        assert_eq!(syl.word, "hello");
    }

    #[test]
    fn test_clip_creation() {
        let p = Phoneme { label: "AH0".into(), start: 0.1, end: 0.2 };
        let syl = Syllable {
            phonemes: vec![p],
            start: 0.1, end: 0.2,
            word: "a".into(), word_index: 0,
        };
        let clip = Clip {
            syllables: vec![syl],
            start: 0.075,
            end: 0.225,
            source: "test.wav".into(),
            output_path: PathBuf::new(),
        };
        assert_eq!(clip.source, "test.wav");
        assert!((clip.start - 0.075).abs() < f64::EPSILON);
    }

    #[test]
    fn test_result_creation() {
        let result = PipelineResult {
            clips: vec![],
            concatenated: PathBuf::from("out.wav"),
            transcript: "hello".into(),
            manifest: serde_json::json!({}),
        };
        assert_eq!(result.transcript, "hello");
        assert_eq!(result.concatenated, PathBuf::from("out.wav"));
    }

    #[test]
    fn test_word_timestamp() {
        let w = WordTimestamp {
            word: "hello".into(),
            start: 0.0,
            end: 0.5,
        };
        assert_eq!(w.word, "hello");
    }

    #[test]
    fn test_phoneme_serde_roundtrip() {
        let p = Phoneme { label: "AH0".into(), start: 0.1, end: 0.2 };
        let json = serde_json::to_string(&p).unwrap();
        let p2: Phoneme = serde_json::from_str(&json).unwrap();
        assert_eq!(p, p2);
    }

    #[test]
    fn test_syllable_serde_roundtrip() {
        let syl = Syllable {
            phonemes: vec![
                Phoneme { label: "HH".into(), start: 0.0, end: 0.1 },
                Phoneme { label: "AH0".into(), start: 0.1, end: 0.25 },
            ],
            start: 0.0,
            end: 0.25,
            word: "hello".into(),
            word_index: 0,
        };
        let json = serde_json::to_string(&syl).unwrap();
        let syl2: Syllable = serde_json::from_str(&json).unwrap();
        assert_eq!(syl, syl2);
    }
}
