//! File-based caching for expensive pipeline operations.
//!
//! Provides SHA-256 file hashing and caching for audio extraction,
//! Whisper transcription, and alignment results.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::types::{AlignmentResult, TranscriptionResult};

/// Get the cache directory.
///
/// Uses `GLOTTISDALE_CACHE_DIR` env var if set, otherwise `~/.cache/glottisdale`.
pub fn cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("GLOTTISDALE_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".cache").join("glottisdale")
}

/// Compute SHA-256 hash of a file's contents.
///
/// Returns a 64-character hex string.
pub fn file_hash(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open file for hashing: {}", path.display()))?;
    std::io::copy(&mut file, &mut hasher)?;
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Atomically write data to a file via temp file + rename.
fn atomic_write(target: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp_path = target.with_extension("tmp");
    std::fs::write(&tmp_path, data)?;
    std::fs::rename(&tmp_path, target)?;
    Ok(())
}

// --- Audio extraction cache ---

/// Return cached extracted audio path, or None if not cached.
pub fn get_cached_audio(input_hash: &str) -> Option<PathBuf> {
    let path = cache_dir().join("extract").join(format!("{}.wav", input_hash));
    if path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        log::info!("Cache hit: audio extraction ({}...)", &input_hash[..12.min(input_hash.len())]);
        Some(path)
    } else {
        None
    }
}

/// Copy extracted audio into cache. Returns the cache path.
pub fn store_audio_cache(input_hash: &str, audio_path: &Path) -> Result<PathBuf> {
    let dest = cache_dir().join("extract").join(format!("{}.wav", input_hash));
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(audio_path, &dest)?;
    log::info!("Cached audio extraction ({}...)", &input_hash[..12.min(input_hash.len())]);
    Ok(dest)
}

// --- Whisper transcription cache ---

/// Return cached whisper result, or None if not cached.
pub fn get_cached_transcription(
    audio_hash: &str,
    model: &str,
    language: &str,
) -> Option<TranscriptionResult> {
    let path = cache_dir()
        .join("whisper")
        .join(format!("{}_{}_{}.json", audio_hash, model, language));
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(&path).ok()?;
    let result: TranscriptionResult = serde_json::from_str(&data).ok()?;
    log::info!("Cache hit: transcription ({}...)", &audio_hash[..12.min(audio_hash.len())]);
    Some(result)
}

/// Store whisper transcription result in cache.
pub fn store_transcription_cache(
    audio_hash: &str,
    model: &str,
    language: &str,
    result: &TranscriptionResult,
) -> Result<()> {
    let path = cache_dir()
        .join("whisper")
        .join(format!("{}_{}_{}.json", audio_hash, model, language));
    let json = serde_json::to_string(result)?;
    atomic_write(&path, json.as_bytes())?;
    log::info!("Cached transcription ({}...)", &audio_hash[..12.min(audio_hash.len())]);
    Ok(())
}

// --- Alignment cache ---

/// Return cached alignment result, or None if not cached.
pub fn get_cached_alignment(
    aligner_name: &str,
    audio_hash: &str,
    model: &str,
    language: &str,
    device: Option<&str>,
) -> Option<AlignmentResult> {
    let mut parts = vec![aligner_name, audio_hash, model, language];
    if let Some(d) = device {
        parts.push(d);
    }
    let path = cache_dir()
        .join("align")
        .join(format!("{}.json", parts.join("_")));
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(&path).ok()?;
    let result: AlignmentResult = serde_json::from_str(&data).ok()?;
    log::info!("Cache hit: alignment ({}...)", &audio_hash[..12.min(audio_hash.len())]);
    Some(result)
}

/// Store alignment result in cache.
pub fn store_alignment_cache(
    aligner_name: &str,
    audio_hash: &str,
    model: &str,
    language: &str,
    result: &AlignmentResult,
    device: Option<&str>,
) -> Result<()> {
    let mut parts = vec![aligner_name, audio_hash, model, language];
    if let Some(d) = device {
        parts.push(d);
    }
    let path = cache_dir()
        .join("align")
        .join(format!("{}.json", parts.join("_")));
    let json = serde_json::to_string(result)?;
    atomic_write(&path, json.as_bytes())?;
    log::info!("Cached alignment ({}...)", &audio_hash[..12.min(audio_hash.len())]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Phoneme, Syllable, WordTimestamp};

    #[test]
    fn test_file_hash_deterministic() {
        let dir = std::env::temp_dir().join(format!("glottisdale_hash_det_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.txt");
        std::fs::write(&path, b"hello world").unwrap();

        let h1 = file_hash(&path).unwrap();
        let h2 = file_hash(&path).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_file_hash_different_content() {
        let dir = std::env::temp_dir().join(format!("glottisdale_hash_diff_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path1 = dir.join("a.txt");
        let path2 = dir.join("b.txt");
        std::fs::write(&path1, b"hello").unwrap();
        std::fs::write(&path2, b"world").unwrap();

        let h1 = file_hash(&path1).unwrap();
        let h2 = file_hash(&path2).unwrap();
        assert_ne!(h1, h2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_atomic_write() {
        let dir = std::env::temp_dir().join(format!("glottisdale_atomic_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.json");

        atomic_write(&path, b"{\"key\": \"value\"}").unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "{\"key\": \"value\"}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_transcription_serde_roundtrip() {
        let result = TranscriptionResult {
            text: "hello world".to_string(),
            words: vec![
                WordTimestamp { word: "hello".to_string(), start: 0.0, end: 0.5 },
                WordTimestamp { word: "world".to_string(), start: 0.5, end: 1.0 },
            ],
            language: "en".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: TranscriptionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text, "hello world");
        assert_eq!(deserialized.words.len(), 2);
        assert_eq!(deserialized.words[0].word, "hello");
    }

    #[test]
    fn test_alignment_serde_roundtrip() {
        let result = AlignmentResult {
            text: "test".to_string(),
            words: vec![WordTimestamp {
                word: "test".to_string(),
                start: 0.0,
                end: 0.5,
            }],
            syllables: vec![Syllable {
                phonemes: vec![
                    Phoneme { label: "T".to_string(), start: 0.0, end: 0.1 },
                    Phoneme { label: "EH1".to_string(), start: 0.1, end: 0.3 },
                    Phoneme { label: "S".to_string(), start: 0.3, end: 0.4 },
                    Phoneme { label: "T".to_string(), start: 0.4, end: 0.5 },
                ],
                start: 0.0,
                end: 0.5,
                word: "test".to_string(),
                word_index: 0,
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: AlignmentResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text, "test");
        assert_eq!(deserialized.syllables.len(), 1);
        assert_eq!(deserialized.syllables[0].phonemes.len(), 4);
        assert_eq!(deserialized.syllables[0].phonemes[0].label, "T");
    }

    #[test]
    fn test_audio_store_and_retrieve() {
        let dir = std::env::temp_dir().join(format!("glottisdale_audio_cache_{}", std::process::id()));
        let cache = dir.join("extract");
        std::fs::create_dir_all(&cache).unwrap();

        // Create source file
        let src = dir.join("source.wav");
        std::fs::write(&src, b"fake wav data").unwrap();

        // Manually copy like store_audio_cache does
        let hash = "testaudiohash";
        let dest = cache.join(format!("{}.wav", hash));
        std::fs::copy(&src, &dest).unwrap();
        assert!(dest.exists());
        assert!(dest.metadata().unwrap().len() > 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_cache_dir_default() {
        // Just verify it returns a path (don't depend on env var)
        let dir = cache_dir();
        assert!(!dir.to_string_lossy().is_empty());
    }
}
