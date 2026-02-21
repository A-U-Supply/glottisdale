//! Aligner interface and backends.
//!
//! Provides syllable-level timestamps from audio files using different
//! alignment strategies:
//! - DefaultAligner: Whisper ASR + G2P + ARPABET syllabifier
//! - BfaAligner: Planned native forced alignment (see issue #21)

use std::path::Path;

use anyhow::{Result, bail};

use crate::types::AlignmentResult;

use super::syllabify;
use super::transcribe;

/// Alignment backend trait.
pub trait Aligner: Send + Sync {
    /// Backend name for caching/display.
    fn name(&self) -> &str;

    /// Transcribe and align audio, returning syllable-level timestamps.
    fn process(
        &self,
        audio_path: &Path,
        model_dir: Option<&Path>,
    ) -> Result<AlignmentResult>;
}

/// Whisper ASR + G2P + ARPABET syllabifier.
///
/// Word-level timestamps from Whisper, phoneme conversion via CMU dict,
/// syllable timing estimated by proportional distribution.
pub struct DefaultAligner {
    pub whisper_model: String,
    pub language: String,
}

impl DefaultAligner {
    pub fn new(whisper_model: &str, language: &str) -> Self {
        Self {
            whisper_model: whisper_model.to_string(),
            language: language.to_string(),
        }
    }
}

impl Default for DefaultAligner {
    fn default() -> Self {
        Self::new("base", "en")
    }
}

impl Aligner for DefaultAligner {
    fn name(&self) -> &str {
        "default"
    }

    fn process(
        &self,
        audio_path: &Path,
        model_dir: Option<&Path>,
    ) -> Result<AlignmentResult> {
        let result = transcribe::transcribe(
            audio_path,
            &self.whisper_model,
            &self.language,
            model_dir,
        )?;

        let syllables = syllabify::syllabify_words(&result.words);

        Ok(AlignmentResult {
            text: result.text,
            words: result.words,
            syllables,
        })
    }
}

/// Get an aligner backend by name.
///
/// Modes:
/// - "default" / "auto" — Whisper + G2P + ARPABET proportional timing.
/// - "bfa" — Not yet available natively (see issue #21).
pub fn get_aligner(
    name: &str,
    whisper_model: &str,
    language: &str,
    _device: &str,
) -> Result<Box<dyn Aligner>> {
    match name {
        "auto" | "default" => Ok(Box::new(DefaultAligner::new(whisper_model, language))),
        "bfa" => {
            bail!(
                "BFA aligner is not yet available in the native build. \
                 Use 'auto' or 'default' aligner instead. \
                 See https://github.com/A-U-Supply/glottisdale/issues/21"
            );
        }
        _ => bail!("Unknown aligner: '{}'. Available: default, auto", name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_aligner_name() {
        let aligner = DefaultAligner::default();
        assert_eq!(aligner.name(), "default");
    }

    #[test]
    fn test_get_aligner_default() {
        let aligner = get_aligner("default", "base", "en", "cpu").unwrap();
        assert_eq!(aligner.name(), "default");
    }

    #[test]
    fn test_get_aligner_bfa() {
        let result = get_aligner("bfa", "base", "en", "cpu");
        match result {
            Err(e) => assert!(e.to_string().contains("not yet available"), "Error: {}", e),
            Ok(_) => panic!("Expected error for BFA aligner"),
        }
    }

    #[test]
    fn test_get_aligner_unknown() {
        let result = get_aligner("nonexistent", "base", "en", "cpu");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_aligner_auto() {
        let aligner = get_aligner("auto", "base", "en", "cpu").unwrap();
        assert_eq!(aligner.name(), "default");
    }
}
