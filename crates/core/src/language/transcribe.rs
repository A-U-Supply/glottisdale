//! Whisper ASR transcription with word-level timestamps.
//!
//! Uses native whisper-rs bindings with automatic model download.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::types::{TranscriptionResult, WordTimestamp};

/// Transcribe audio and return word-level timestamps.
///
/// Uses native whisper-rs bindings. The whisper model is automatically
/// downloaded on first use if not found locally.
pub fn transcribe(
    audio_path: &Path,
    model_name: &str,
    language: &str,
    model_dir: Option<&Path>,
) -> Result<TranscriptionResult> {
    #[cfg(feature = "whisper-native")]
    {
        return transcribe_native(audio_path, model_name, language, model_dir);
    }

    #[cfg(not(feature = "whisper-native"))]
    {
        let _ = (audio_path, model_name, language, model_dir);
        bail!(
            "Whisper transcription requires the 'whisper-native' feature. \
             Build with: cargo build --features whisper-native"
        );
    }
}

/// Parse Whisper's JSON output into our TranscriptionResult.
#[cfg(test)]
fn parse_whisper_json(json_str: &str, default_language: &str) -> Result<TranscriptionResult> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).context("Failed to parse whisper JSON")?;

    let text = value["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    let language = value["language"]
        .as_str()
        .unwrap_or(default_language)
        .to_string();

    let mut words = Vec::new();
    if let Some(segments) = value["segments"].as_array() {
        for segment in segments {
            if let Some(segment_words) = segment["words"].as_array() {
                for w in segment_words {
                    let word = w["word"]
                        .as_str()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let start = w["start"].as_f64().unwrap_or(0.0);
                    let end = w["end"].as_f64().unwrap_or(0.0);

                    if !word.is_empty() {
                        words.push(WordTimestamp { word, start, end });
                    }
                }
            }
        }
    }

    Ok(TranscriptionResult {
        text,
        words,
        language,
    })
}

/// Transcribe using native whisper-rs bindings.
#[cfg(feature = "whisper-native")]
fn transcribe_native(
    audio_path: &Path,
    model_name: &str,
    language: &str,
    model_dir: Option<&Path>,
) -> Result<TranscriptionResult> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    // Find or download model
    let model_path = find_model(model_name, model_dir)?;

    let ctx = WhisperContext::new_with_params(
        model_path.to_str().unwrap(),
        WhisperContextParameters::default(),
    )
    .context("Failed to load whisper model")?;

    // Load and convert audio to f32 mono 16kHz
    let (samples, sr) = crate::audio::io::read_wav(audio_path)?;
    let samples_16k = if sr != 16000 {
        crate::audio::io::resample(&samples, sr, 16000)?
    } else {
        samples
    };
    let samples_f32: Vec<f32> = samples_16k.iter().map(|&s| s as f32).collect();

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(language));
    params.set_token_timestamps(true);

    let mut state = ctx.create_state().context("Failed to create whisper state")?;
    state
        .full(params, &samples_f32)
        .context("Whisper inference failed")?;

    let n_segments = state.full_n_segments()?;
    let mut text_parts = Vec::new();
    let mut words = Vec::new();

    for i in 0..n_segments {
        let segment_text = state.full_get_segment_text(i)?;
        text_parts.push(segment_text.trim().to_string());

        let n_tokens = state.full_n_tokens(i)?;
        for j in 0..n_tokens {
            let token_text = state.full_get_token_text(i, j)?;
            let token_data = state.full_get_token_data(i, j)?;

            let trimmed = token_text.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }

            // Skip special tokens
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                continue;
            }

            let start = token_data.t0 as f64 / 100.0; // centiseconds to seconds
            let end = token_data.t1 as f64 / 100.0;

            words.push(WordTimestamp {
                word: trimmed,
                start,
                end,
            });
        }
    }

    Ok(TranscriptionResult {
        text: text_parts.join(" ").trim().to_string(),
        words,
        language: language.to_string(),
    })
}

const HF_MODEL_BASE: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Construct the download URL for a whisper GGML model.
#[cfg(feature = "whisper-native")]
fn model_download_url(model_name: &str) -> String {
    format!("{}/ggml-{}.bin", HF_MODEL_BASE, model_name)
}

/// Find a whisper model file, downloading if necessary.
#[cfg(feature = "whisper-native")]
fn find_model(model_name: &str, model_dir: Option<&Path>) -> Result<std::path::PathBuf> {
    let filename = format!("ggml-{}.bin", model_name);

    // Check provided model directory
    if let Some(dir) = model_dir {
        let path = dir.join(&filename);
        if path.exists() {
            return Ok(path);
        }
    }

    // Check default cache directory
    let cache_dir = dirs_or_default().join("glottisdale").join("models");
    let path = cache_dir.join(&filename);
    if path.exists() {
        return Ok(path);
    }

    // Download the model
    log::info!(
        "Whisper model '{}' not found locally, downloading...",
        model_name
    );
    download_model(model_name, &cache_dir)
}

/// Download a whisper GGML model from Hugging Face.
#[cfg(feature = "whisper-native")]
fn download_model(model_name: &str, dest_dir: &Path) -> Result<std::path::PathBuf> {
    use std::io::{Read, Write};

    let url = model_download_url(model_name);
    let filename = format!("ggml-{}.bin", model_name);
    let dest_path = dest_dir.join(&filename);

    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("Failed to create model directory: {}", dest_dir.display()))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1800))
        .build()
        .context("Failed to build HTTP client")?;

    log::info!("Downloading {} ...", url);

    let mut response = client.get(&url).send().context("Failed to download model")?;

    if !response.status().is_success() {
        bail!("Download failed: HTTP {} for {}", response.status(), url);
    }

    let total_size = response.content_length();
    if let Some(size) = total_size {
        log::info!("Model size: {:.1} MB", size as f64 / 1_048_576.0);
    }

    // Write to temp file in same directory for atomic rename
    let mut tmp_file = tempfile::NamedTempFile::new_in(dest_dir)
        .context("Failed to create temp file")?;

    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 64 * 1024];
    let mut last_log_pct = 0u64;

    loop {
        let n = response.read(&mut buf).context("Error reading download")?;
        if n == 0 {
            break;
        }
        tmp_file.write_all(&buf[..n]).context("Error writing model")?;
        downloaded += n as u64;

        // Log progress every 10%
        if let Some(total) = total_size {
            let pct = downloaded * 100 / total;
            if pct >= last_log_pct + 10 {
                log::info!("Download progress: {}%", pct);
                last_log_pct = pct;
            }
        }
    }

    // Verify download size
    if let Some(expected) = total_size {
        if downloaded != expected {
            bail!(
                "Incomplete download: got {} bytes, expected {}",
                downloaded,
                expected
            );
        }
    }

    // Atomic rename
    tmp_file.persist(&dest_path).map_err(|e| {
        anyhow::anyhow!("Failed to save model to {}: {}", dest_path.display(), e)
    })?;

    log::info!("Model saved to {}", dest_path.display());
    Ok(dest_path)
}

#[cfg(feature = "whisper-native")]
fn dirs_or_default() -> std::path::PathBuf {
    std::env::var("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| std::path::PathBuf::from(h).join(".cache"))
                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_whisper_json() {
        let json = r#"{
            "text": " Hello world",
            "language": "en",
            "segments": [
                {
                    "text": " Hello world",
                    "words": [
                        {"word": " Hello", "start": 0.0, "end": 0.5},
                        {"word": " world", "start": 0.5, "end": 1.0}
                    ]
                }
            ]
        }"#;

        let result = parse_whisper_json(json, "en").unwrap();
        assert_eq!(result.text, "Hello world");
        assert_eq!(result.language, "en");
        assert_eq!(result.words.len(), 2);
        assert_eq!(result.words[0].word, "Hello");
        assert_eq!(result.words[1].word, "world");
        assert!((result.words[0].start - 0.0).abs() < 0.001);
        assert!((result.words[1].end - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_whisper_json_empty() {
        let json = r#"{"text": "", "segments": []}"#;
        let result = parse_whisper_json(json, "en").unwrap();
        assert!(result.text.is_empty());
        assert!(result.words.is_empty());
    }

    #[cfg(feature = "whisper-native")]
    #[test]
    fn test_find_model_constructs_url() {
        // Test that the model URL is correctly constructed
        let url = model_download_url("base");
        assert_eq!(
            url,
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
        );
    }

    #[cfg(feature = "whisper-native")]
    #[test]
    fn test_find_model_uses_cache_dir() {
        let dir = std::env::temp_dir().join("glottisdale_test_model_cache");
        std::fs::create_dir_all(&dir).unwrap();

        // Create a fake model file
        let model_path = dir.join("ggml-tiny.bin");
        std::fs::write(&model_path, b"fake model data").unwrap();

        // find_model should return the cached path without downloading
        let result = find_model("tiny", Some(&dir));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), model_path);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_parse_whisper_json_no_words() {
        let json = r#"{
            "text": "Hello",
            "segments": [{"text": "Hello"}]
        }"#;
        let result = parse_whisper_json(json, "en").unwrap();
        assert_eq!(result.text, "Hello");
        assert!(result.words.is_empty());
    }
}
