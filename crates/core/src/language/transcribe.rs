//! Whisper ASR transcription with word-level timestamps.
//!
//! Supports two backends:
//! - Native: via `whisper-rs` (requires `whisper-native` feature)
//! - CLI: via `whisper` command-line tool (subprocess)

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::types::{TranscriptionResult, WordTimestamp};

/// Transcribe audio and return word-level timestamps.
///
/// Tries native whisper-rs first (if compiled with `whisper-native` feature),
/// then falls back to the whisper CLI subprocess.
pub fn transcribe(
    audio_path: &Path,
    model_name: &str,
    language: &str,
    _model_dir: Option<&Path>,
) -> Result<TranscriptionResult> {
    #[cfg(feature = "whisper-native")]
    {
        match transcribe_native(audio_path, model_name, language, _model_dir) {
            Ok(result) => return Ok(result),
            Err(e) => {
                log::warn!("Native whisper failed ({}), trying CLI fallback", e);
            }
        }
    }

    transcribe_cli(audio_path, model_name, language)
}

/// Transcribe using the whisper CLI (subprocess).
///
/// Expects the `whisper` command to be available on PATH.
/// Uses JSON output format for structured results.
fn transcribe_cli(
    audio_path: &Path,
    model_name: &str,
    language: &str,
) -> Result<TranscriptionResult> {
    use std::process::Command;

    let output_dir = std::env::temp_dir().join("glottisdale_whisper");
    std::fs::create_dir_all(&output_dir)?;

    let result = Command::new("whisper")
        .args([
            audio_path.to_str().unwrap(),
            "--model", model_name,
            "--language", language,
            "--word_timestamps", "True",
            "--output_format", "json",
            "--output_dir", output_dir.to_str().unwrap(),
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            // Parse the JSON output file
            let stem = audio_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("audio");
            let json_path = output_dir.join(format!("{}.json", stem));

            let json_str = std::fs::read_to_string(&json_path)
                .with_context(|| format!("Failed to read whisper output: {}", json_path.display()))?;

            parse_whisper_json(&json_str, language)
        }
        Ok(output) => {
            bail!(
                "whisper CLI failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            bail!("whisper CLI not found: {}", e);
        }
    }
}

/// Parse Whisper's JSON output into our TranscriptionResult.
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

    bail!(
        "Whisper model '{}' not found. Download it to {:?} or specify --model-dir",
        model_name,
        cache_dir
    );
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
