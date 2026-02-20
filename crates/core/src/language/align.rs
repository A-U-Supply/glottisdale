//! Aligner interface and backends.
//!
//! Provides syllable-level timestamps from audio files using different
//! alignment strategies:
//! - DefaultAligner: Whisper ASR + G2P + ARPABET syllabifier
//! - BfaAligner: Whisper + BFA forced alignment (subprocess)

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

/// BFA (Bournemouth Forced Aligner) backend via subprocess.
///
/// Uses Whisper for transcription, then BFA for precise phoneme-level
/// timestamps with pg16 group classifications.
pub struct BfaAligner {
    pub whisper_model: String,
    pub language: String,
    pub device: String,
}

impl BfaAligner {
    pub fn new(whisper_model: &str, language: &str, device: &str) -> Self {
        Self {
            whisper_model: whisper_model.to_string(),
            language: language.to_string(),
            device: device.to_string(),
        }
    }
}

impl Default for BfaAligner {
    fn default() -> Self {
        Self::new("base", "en", "cpu")
    }
}

impl Aligner for BfaAligner {
    fn name(&self) -> &str {
        "bfa"
    }

    fn process(
        &self,
        audio_path: &Path,
        model_dir: Option<&Path>,
    ) -> Result<AlignmentResult> {
        // BFA requires Python subprocess — call the Python BFA tool
        // For now, fall back to default alignment
        log::warn!("BFA aligner not yet implemented in Rust, using default aligner");
        let default = DefaultAligner::new(&self.whisper_model, &self.language);
        default.process(audio_path, model_dir)
    }
}

/// Check if BFA is available on the system.
pub fn bfa_available() -> bool {
    // Check for espeak-ng
    if std::process::Command::new("espeak-ng")
        .arg("--version")
        .output()
        .is_err()
    {
        return false;
    }

    // Check for bournemouth-aligner Python package
    std::process::Command::new("python3")
        .args(["-c", "import bournemouth_aligner"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get an aligner backend by name.
///
/// Modes:
/// - "default" — Whisper + G2P + ARPABET proportional timing.
/// - "bfa" — Whisper + BFA phoneme-level alignment.
/// - "auto" — Tries BFA first, falls back to default.
pub fn get_aligner(
    name: &str,
    whisper_model: &str,
    language: &str,
    device: &str,
) -> Result<Box<dyn Aligner>> {
    match name {
        "auto" => {
            if bfa_available() {
                log::info!("Auto-detected BFA + espeak-ng, using BFA aligner");
                Ok(Box::new(BfaAligner::new(whisper_model, language, device)))
            } else {
                log::info!("BFA not available, using default aligner");
                Ok(Box::new(DefaultAligner::new(whisper_model, language)))
            }
        }
        "default" => Ok(Box::new(DefaultAligner::new(whisper_model, language))),
        "bfa" => Ok(Box::new(BfaAligner::new(whisper_model, language, device))),
        _ => bail!("Unknown aligner: '{}'. Available: default, bfa, auto", name),
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
    fn test_bfa_aligner_name() {
        let aligner = BfaAligner::default();
        assert_eq!(aligner.name(), "bfa");
    }

    #[test]
    fn test_get_aligner_default() {
        let aligner = get_aligner("default", "base", "en", "cpu").unwrap();
        assert_eq!(aligner.name(), "default");
    }

    #[test]
    fn test_get_aligner_bfa() {
        let aligner = get_aligner("bfa", "base", "en", "cpu").unwrap();
        assert_eq!(aligner.name(), "bfa");
    }

    #[test]
    fn test_get_aligner_unknown() {
        let result = get_aligner("nonexistent", "base", "en", "cpu");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_aligner_auto() {
        // Should not error regardless of BFA availability
        let aligner = get_aligner("auto", "base", "en", "cpu").unwrap();
        assert!(aligner.name() == "default" || aligner.name() == "bfa");
    }
}
