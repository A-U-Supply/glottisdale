# Zero-Install Native Dependencies Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all external CLI dependencies (ffmpeg, whisper CLI, rubberband CLI) with statically-linked or pure-Rust equivalents so the CLI and GUI binaries are fully self-contained.

**Architecture:** Replace `extract_audio()` internals with symphonia (pure Rust audio decoding), enable `whisper-rs` by default with auto-download of GGML models, replace rubberband CLI calls with `ssstretch` (Signalsmith Stretch, statically linked C++), and refactor the GUI to call core library directly instead of spawning the CLI as a subprocess.

**Tech Stack:** symphonia (audio decoding), whisper-rs (transcription), ssstretch (pitch/time), reqwest (model download), tempfile (atomic writes)

---

### Task 1: Replace ffmpeg with symphonia for audio extraction

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/core/src/audio/io.rs:155-183`
- Modify: `Cargo.toml` (workspace deps)

**Step 1: Add symphonia dependencies**

In `Cargo.toml` (workspace), add:

```toml
[workspace.dependencies]
symphonia = { version = "0.5", features = ["mp3", "aac", "isomp4", "wav"] }
```

In `crates/core/Cargo.toml`, add under `[dependencies]`:

```toml
symphonia.workspace = true
```

**Step 2: Write the failing test**

Add to `crates/core/src/audio/io.rs` in the `tests` module:

```rust
#[test]
fn test_extract_audio_native_wav() {
    // Create a WAV file, then extract it via the native path
    let dir = std::env::temp_dir().join("glottisdale_test_extract");
    std::fs::create_dir_all(&dir).unwrap();

    let input = dir.join("input.wav");
    let output = dir.join("output.wav");

    // Write a 44.1kHz stereo WAV
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&input, spec).unwrap();
    for i in 0..44100 {
        let sample = ((i as f64 / 44100.0 * 440.0 * std::f64::consts::TAU).sin() * 16000.0) as i16;
        writer.write_sample(sample).unwrap(); // left
        writer.write_sample(sample).unwrap(); // right
    }
    writer.finalize().unwrap();

    // Extract should produce 16kHz mono WAV
    extract_audio(&input, &output).unwrap();
    let (samples, sr) = read_wav(&output).unwrap();
    assert_eq!(sr, 16000);
    // 1 second at 44.1kHz -> ~1 second at 16kHz = ~16000 samples
    assert!(samples.len() > 14000 && samples.len() < 18000,
        "Expected ~16000 samples, got {}", samples.len());

    std::fs::remove_dir_all(&dir).ok();
}
```

**Step 3: Run test to verify it fails**

Run: `cargo test -p glottisdale-core test_extract_audio_native_wav -- --nocapture`
Expected: FAIL (still calls ffmpeg, which may not exist or produces different output depending on test env)

**Step 4: Implement native audio extraction**

Replace the body of `extract_audio()` in `crates/core/src/audio/io.rs:155-183` with:

```rust
/// Extract/convert audio from any format to 16kHz mono WAV.
///
/// Supports WAV, MP3, and MP4 (AAC audio track) via symphonia.
/// No external tools required.
pub fn extract_audio(input_path: &Path, output_path: &Path) -> Result<()> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
    use symphonia::core::errors::Error as SymphError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(input_path)
        .with_context(|| format!("Failed to open: {}", input_path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = input_path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .with_context(|| format!("Unsupported format: {}", input_path.display()))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .context("No audio track found")?;

    let track_id = track.id;
    let source_sr = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Unsupported codec")?;

    let mut all_samples: Vec<f64> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphError::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymphError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let num_frames = decoded.frames();
                let mut sample_buf = SampleBuffer::<f64>::new(
                    num_frames as u64,
                    spec,
                );
                sample_buf.copy_interleaved_ref(decoded);
                let interleaved = sample_buf.samples();

                // Convert to mono by averaging channels
                if channels > 1 {
                    for frame in 0..num_frames {
                        let mut sum = 0.0;
                        for ch in 0..channels {
                            sum += interleaved[frame * channels + ch];
                        }
                        all_samples.push(sum / channels as f64);
                    }
                } else {
                    all_samples.extend_from_slice(interleaved);
                }
            }
            Err(SymphError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    if all_samples.is_empty() {
        bail!("No audio decoded from {}", input_path.display());
    }

    // Resample to 16kHz if needed
    let samples_16k = if source_sr != 16000 {
        resample(&all_samples, source_sr, 16000)?
    } else {
        all_samples
    };

    write_wav(output_path, &samples_16k, 16000)?;
    Ok(())
}
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p glottisdale-core test_extract_audio_native -- --nocapture`
Expected: PASS

**Step 6: Run full test suite**

Run: `cargo test`
Expected: All 207+ tests pass

**Step 7: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/audio/io.rs Cargo.toml Cargo.lock
git commit -m "feat: replace ffmpeg with symphonia for native audio extraction"
```

---

### Task 2: Replace rubberband CLI with ssstretch (Signalsmith Stretch)

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/core/src/audio/effects.rs:148-314`
- Modify: `Cargo.toml` (workspace deps)

**Step 1: Add ssstretch dependency**

In `Cargo.toml` (workspace), add:

```toml
ssstretch = "0.2"
```

In `crates/core/Cargo.toml`, add:

```toml
ssstretch.workspace = true
```

**Step 2: Write the failing test for native pitch shift**

Add to `crates/core/src/audio/effects.rs` in the `tests` module:

```rust
#[test]
fn test_pitch_shift_native_up() {
    // 1 second of 440Hz sine at 16kHz
    let sr = 16000u32;
    let samples: Vec<f64> = (0..sr as usize)
        .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin())
        .collect();

    let result = pitch_shift(&samples, sr, 2.0).unwrap();
    // Should preserve length (not change speed)
    assert!(
        (result.len() as f64 - samples.len() as f64).abs() < 100.0,
        "Pitch shift changed length: {} vs {}",
        result.len(),
        samples.len()
    );
    // Should not be silent
    let rms: f64 = (result.iter().map(|s| s * s).sum::<f64>() / result.len() as f64).sqrt();
    assert!(rms > 0.1, "Output is too quiet: RMS={}", rms);
}

#[test]
fn test_time_stretch_native_double() {
    let sr = 16000u32;
    let samples: Vec<f64> = (0..sr as usize)
        .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin())
        .collect();

    let result = time_stretch(&samples, sr, 2.0).unwrap();
    // Factor 2.0 = twice as long
    let expected_len = samples.len() * 2;
    assert!(
        (result.len() as f64 - expected_len as f64).abs() / expected_len as f64 < 0.1,
        "Expected ~{} samples, got {}",
        expected_len,
        result.len()
    );
}
```

**Step 3: Run tests to verify they fail or pass with old implementation**

Run: `cargo test -p glottisdale-core test_pitch_shift_native test_time_stretch_native -- --nocapture`

**Step 4: Replace rubberband CLI calls with ssstretch**

Replace the pitch shift and time stretch functions in `crates/core/src/audio/effects.rs`. Delete `time_stretch_rubberband()`, `pitch_shift_rubberband()`, `time_stretch_simple()`, and `pitch_shift_simple()`. Replace `time_stretch()` and `pitch_shift()` with native implementations:

```rust
/// Pitch-shift by semitones using Signalsmith Stretch (phase vocoder).
///
/// Preserves duration while shifting pitch. High quality, no external tools.
pub fn pitch_shift(samples: &[f64], sr: u32, semitones: f64) -> Result<Vec<f64>> {
    if semitones.abs() < 0.01 {
        return Ok(samples.to_vec());
    }

    let mut stretch = ssstretch::Stretch::new();
    stretch.preset_default(1, sr as f32); // mono
    stretch.set_transpose_semitones(semitones as f32, None);

    let input_f32: Vec<f32> = samples.iter().map(|&s| s as f32).collect();
    let in_len = input_f32.len() as i32;
    let out_len = in_len; // pitch shift preserves length

    let mut output_f32 = vec![vec![0.0f32; out_len as usize]; 1];
    stretch.process_vec(
        &[input_f32],
        in_len,
        &mut output_f32,
        out_len,
    );

    Ok(output_f32[0].iter().map(|&s| s as f64).collect())
}

/// Time-stretch by factor using Signalsmith Stretch (phase vocoder).
///
/// `factor` > 1.0 = slower (longer), < 1.0 = faster (shorter).
/// Preserves pitch while changing duration. High quality, no external tools.
pub fn time_stretch(samples: &[f64], sr: u32, factor: f64) -> Result<Vec<f64>> {
    if (factor - 1.0).abs() < 0.01 {
        return Ok(samples.to_vec());
    }

    if samples.is_empty() {
        return Ok(vec![]);
    }

    let mut stretch = ssstretch::Stretch::new();
    stretch.preset_default(1, sr as f32);

    let input_f32: Vec<f32> = samples.iter().map(|&s| s as f32).collect();
    let in_len = input_f32.len() as i32;
    let out_len = (samples.len() as f64 * factor).round() as i32;

    if out_len <= 0 {
        return Ok(vec![]);
    }

    let mut output_f32 = vec![vec![0.0f32; out_len as usize]; 1];
    stretch.process_vec(
        &[input_f32],
        in_len,
        &mut output_f32,
        out_len,
    );

    Ok(output_f32[0].iter().map(|&s| s as f64).collect())
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p glottisdale-core -- --nocapture`
Expected: All tests pass, including the new ones and existing `test_pitch_shift_simple_no_change`, `test_time_stretch_simple_*`

**Step 6: Update existing tests that reference old function names**

Search for `pitch_shift_simple` and `time_stretch_simple` in test code. Update any tests that call these directly to call `pitch_shift` and `time_stretch` instead. Remove tests for the deleted functions.

Run: `cargo test -p glottisdale-core`
Expected: All pass

**Step 7: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/audio/effects.rs Cargo.toml Cargo.lock
git commit -m "feat: replace rubberband CLI with ssstretch for native pitch/time processing"
```

---

### Task 3: Enable whisper-rs by default with model auto-download

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/core/src/language/transcribe.rs:209-245`
- Modify: `Cargo.toml` (workspace deps)

**Step 1: Add reqwest and tempfile dependencies**

In `Cargo.toml` (workspace), add:

```toml
reqwest = { version = "0.12", features = ["blocking"] }
tempfile = "3"
```

In `crates/core/Cargo.toml`:

1. Change `whisper-native` feature to be in `default`:
```toml
[features]
default = ["whisper-native"]
whisper-native = ["whisper-rs", "reqwest", "tempfile"]
```

2. Add dependencies:
```toml
reqwest = { workspace = true, optional = true }
tempfile = { workspace = true, optional = true }
```

**Step 2: Write the failing test for model download**

Add a test in `crates/core/src/language/transcribe.rs`:

```rust
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
```

**Step 3: Run test to verify it fails**

Run: `cargo test -p glottisdale-core test_find_model -- --nocapture`
Expected: FAIL (function `model_download_url` doesn't exist yet)

**Step 4: Implement model auto-download**

Replace the `find_model` function in `crates/core/src/language/transcribe.rs`:

```rust
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
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p glottisdale-core test_find_model -- --nocapture`
Expected: PASS

**Step 6: Run full test suite**

Run: `cargo test`
Expected: All pass

**Step 7: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/language/transcribe.rs Cargo.toml Cargo.lock
git commit -m "feat: enable whisper-rs by default with model auto-download"
```

---

### Task 4: Refactor GUI to call core library directly

**Files:**
- Modify: `crates/gui/Cargo.toml`
- Modify: `crates/gui/src/app.rs`

This is the largest task. The GUI currently spawns the CLI as a subprocess. We need to replace that with direct library calls.

**Step 1: Add core dependencies to GUI**

In `crates/gui/Cargo.toml`, the `glottisdale-core` dependency already exists. Add additional deps needed for the runner logic (zip, etc.):

```toml
[dependencies]
glottisdale-core.workspace = true
anyhow.workspace = true
eframe = "0.31"
rfd = "0.15"
log.workspace = true
env_logger.workspace = true
serde_json.workspace = true
zip = "2"
```

**Step 2: Replace `run_cli_subprocess()` with direct core calls**

In `crates/gui/src/app.rs`, replace the `run_cli_subprocess()` function. The new function should:

1. Call `glottisdale_core::names::create_run_dir()` to create the run directory
2. Call `glottisdale_core::audio::io::extract_audio()` for each input file
3. Call `glottisdale_core::language::align::get_aligner()` and process each audio file
4. Call the appropriate pipeline function (`collage::process::process()`, etc.)
5. Write log lines to `processing.log_lines` as it goes
6. Store output paths in `processing.output_paths`

The general pattern for the collage pipeline (the primary one):

```rust
fn run_collage_pipeline(
    inputs: &[PathBuf],
    output_dir: &Path,
    run_name: Option<&str>,
    seed: Option<u64>,
    target_duration: f64,
    whisper_model: &str,
    log_lines: Arc<Mutex<Vec<String>>>,
    output_paths: Arc<Mutex<Vec<(String, PathBuf)>>>,
) -> anyhow::Result<()> {
    use glottisdale_core::audio::io::{extract_audio, read_wav};
    use glottisdale_core::collage::process::{CollageConfig, process};
    use glottisdale_core::language::align::get_aligner;
    use glottisdale_core::names::create_run_dir;
    use std::collections::HashMap;

    let add_log = |msg: &str| {
        if let Ok(mut lines) = log_lines.lock() {
            lines.push(msg.to_string());
        }
    };

    let run_dir = create_run_dir(output_dir, seed, run_name)?;
    let run_dir_name = run_dir.file_name().unwrap().to_string_lossy().to_string();
    add_log(&format!("Run: {}", run_dir_name));

    let work_dir = run_dir.join("work");
    std::fs::create_dir_all(&work_dir)?;

    // Extract audio
    let mut audio_paths = Vec::new();
    for input in inputs {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        let wav_path = work_dir.join(format!("{}_16k.wav", stem));
        add_log(&format!("Extracting audio: {}", input.display()));
        extract_audio(input, &wav_path)?;
        audio_paths.push(wav_path);
    }

    // Align
    let aligner = get_aligner("auto", whisper_model, "en", &None)?;
    let mut source_audio: HashMap<String, (Vec<f64>, u32)> = HashMap::new();
    let mut source_syllables = HashMap::new();

    for audio_path in &audio_paths {
        let key = audio_path.to_string_lossy().to_string();
        add_log(&format!("Aligning: {}", audio_path.display()));
        let alignment = aligner.process(audio_path, None)?;
        let (samples, sr) = read_wav(audio_path)?;
        source_audio.insert(key.clone(), (samples, sr));
        source_syllables.insert(key, alignment.syllables);
    }

    // Process
    add_log("Assembling collage...");
    let config = CollageConfig {
        target_duration,
        seed,
        ..CollageConfig::default()
    };
    let result = process(&source_audio, &source_syllables, &run_dir, &config)?;

    // Store outputs
    if let Ok(mut paths) = output_paths.lock() {
        paths.push(("Output".to_string(), result.concatenated.clone()));
    }

    add_log(&format!("Output: {}", result.concatenated.display()));
    Ok(())
}
```

**Step 3: Update the background thread spawning**

Replace the subprocess spawn in the "Run Collage" / "Run Sing" / "Run Speak" button handlers. Instead of building CLI args and calling `run_cli_subprocess()`, call the pipeline function directly on a background thread:

```rust
// In the button handler, instead of:
//   let args = build_cli_args(...);
//   run_cli_subprocess(args, processing.clone());
// Do:
let processing = self.processing.clone();
let inputs = self.source_files.clone();
let output_dir = PathBuf::from("./glottisdale-output");
// ... gather other settings from UI ...

std::thread::spawn(move || {
    *processing.status.lock().unwrap() = ProcessingStatus::Running;
    match run_collage_pipeline(
        &inputs, &output_dir, None, None,
        30.0, "base",
        processing.log_lines.clone(),
        processing.output_paths.clone(),
    ) {
        Ok(()) => {
            *processing.status.lock().unwrap() = ProcessingStatus::Done;
        }
        Err(e) => {
            if let Ok(mut lines) = processing.log_lines.lock() {
                lines.push(format!("ERROR: {}", e));
            }
            *processing.status.lock().unwrap() = ProcessingStatus::Error(e.to_string());
        }
    }
});
```

**Step 4: Forward log output to GUI**

Set up a custom log handler that forwards `log::info!()` calls from the core library to the GUI log panel. Add this at app initialization:

```rust
use std::sync::{Arc, Mutex};

struct GuiLogger {
    lines: Arc<Mutex<Vec<String>>>,
}

impl log::Log for GuiLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        if record.level() <= log::Level::Info {
            if let Ok(mut lines) = self.lines.lock() {
                lines.push(format!("{}", record.args()));
            }
        }
    }
    fn flush(&self) {}
}
```

Note: The log crate only allows one global logger. Since `env_logger` is already initialized in `main.rs`, an alternative approach is to use the `add_log` closure pattern shown in step 2, where pipeline functions take a log callback. This avoids conflicting with the global logger.

**Step 5: Delete subprocess code**

Remove the following from `app.rs`:
- `run_cli_subprocess()` function
- CLI binary path detection code
- stdout/stderr parsing for output paths
- All `Command::new()` calls except `open_path()` (which opens files in the OS)

**Step 6: Test manually**

Run: `cargo run -p glottisdale-gui`
- Add a source file
- Click "Run Collage"
- Verify log output appears
- Verify output files appear with Play/Open buttons
- Verify no subprocess is spawned

**Step 7: Run full test suite**

Run: `cargo test`
Expected: All pass

**Step 8: Commit**

```bash
git add crates/gui/Cargo.toml crates/gui/src/app.rs
git commit -m "feat(gui): call core library directly instead of CLI subprocess"
```

---

### Task 5: Clean up dead code and stubs

**Files:**
- Modify: `crates/core/src/language/align.rs:127-142`
- Modify: `crates/core/src/audio/effects.rs` (verify no leftover rubberband references)

**Step 1: Remove espeak-ng and python3 stubs**

In `crates/core/src/language/align.rs`, remove or simplify the BFA detection stubs. The `get_aligner()` function with `"bfa"` should return a clear error explaining that native forced alignment is not yet available (see GitHub issue #21):

```rust
"bfa" => {
    bail!(
        "BFA aligner is not yet available in the native build. \
         Use 'auto' or 'default' aligner instead. \
         See https://github.com/A-U-Supply/glottisdale/issues/21"
    );
}
```

Remove the `Command::new("espeak-ng")` and `Command::new("python3")` checks entirely.

**Step 2: Verify no remaining subprocess calls in core**

Run: `grep -rn "Command::new" crates/core/src/`
Expected: No matches (all subprocess calls removed)

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All pass

**Step 4: Commit**

```bash
git add crates/core/src/language/align.rs crates/core/src/audio/effects.rs
git commit -m "chore: remove dead subprocess stubs (espeak-ng, python3, rubberband)"
```

---

### Task 6: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/getting-started/install.md`
- Modify: `docs/getting-started/quickstart.md`
- Modify: `docs/guide/troubleshooting.md`
- Modify: `docs/reference/architecture.md`

**Step 1: Update install.md**

Remove all references to ffmpeg, Python/Whisper, and rubberband as required dependencies. The install guide should just say:

- Pre-built binary: download, make executable, run
- From source: Rust 1.75+, `cargo build --release`, done
- Linux: system windowing libs needed for GUI (GTK, etc.)
- Optional: rubberband CLI still works if installed (for users who prefer it)

Remove the "I want vocal MIDI mapping (rubberband)" and "I want the most accurate syllable detection (espeak-ng)" sections.

**Step 2: Update quickstart.md**

Remove the note about Whisper model download taking time on first run (it still does, but it's automatic now — mention this inline instead).

Remove references to `pip install openai-whisper`.

**Step 3: Update troubleshooting.md**

Remove troubleshooting sections for:
- ffmpeg not found
- Whisper not found / Python not installed
- rubberband not found

Add section for:
- Model download fails (network issues, disk space)
- Unsupported audio format (not MP3/MP4/WAV)

**Step 4: Update architecture.md**

Update the module descriptions to reflect native dependencies:
- `audio::io` — "WAV read/write, multi-format audio extraction via symphonia, resampling"
- `audio::effects` — "Pitch shift, time stretch via Signalsmith Stretch, volume, crossfade, mixing"
- `language::transcribe` — "Native Whisper transcription via whisper-rs with auto model download"

**Step 5: Update README.md**

Update the Dependencies section:
- Remove ffmpeg, Whisper, rubberband from required deps
- State: "No external dependencies required. Whisper model (~140MB) downloads automatically on first run."
- Keep "Rust 1.75+" for building from source

**Step 6: Commit**

```bash
git add README.md docs/
git commit -m "docs: update for zero-install native dependencies"
```

---

### Task 7: Update CI and release workflow

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/release.yml`

**Step 1: Update CI**

The CI workflow may need additional system dependencies for building whisper-rs (C++ compiler, cmake) and ssstretch (C++ compiler). Update the Linux system deps step:

```yaml
- name: Install system dependencies
  run: |
    sudo apt-get update
    sudo apt-get install -y libasound2-dev cmake
```

macOS already has a C++ compiler via Xcode command line tools.

**Step 2: Update release workflow**

Same addition for the release build matrix:

```yaml
- name: Install system dependencies (Linux)
  if: runner.os == 'Linux'
  run: |
    sudo apt-get update
    sudo apt-get install -y libasound2-dev cmake \
      libxkbcommon-dev libgtk-3-dev
```

**Step 3: Test CI builds**

Push the branch and verify CI passes for both platforms.

**Step 4: Commit**

```bash
git add .github/workflows/ci.yml .github/workflows/release.yml
git commit -m "ci: add build deps for whisper-rs and ssstretch"
```

---

### Task 8: Bump version and release

**Files:**
- Modify: `Cargo.toml`

**Step 1: Bump version to 0.3.0**

In `Cargo.toml`:
```toml
version = "0.3.0"
```

**Step 2: Commit and push**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 0.3.0"
git push -u origin feat/zero-install-native-deps
```

**Step 3: Create PR**

```bash
gh pr create --title "Zero-install native dependencies" --body "$(cat <<'EOF'
## Summary
- Replace ffmpeg with symphonia (pure Rust) for audio extraction
- Replace rubberband CLI with ssstretch (Signalsmith Stretch, statically linked)
- Enable whisper-rs by default with automatic model download
- Refactor GUI to call core library directly (no CLI subprocess)
- Remove all external tool stubs (espeak-ng, python3)
- Update all documentation

## Result
Both CLI and GUI binaries are fully self-contained. No ffmpeg, no Python,
no rubberband install needed. Download, run, done.

## Test plan
- [ ] All existing tests pass
- [ ] Manual test: collage pipeline with MP4 input
- [ ] Manual test: collage pipeline with MP3 input
- [ ] Manual test: GUI runs without CLI binary present
- [ ] Manual test: whisper model auto-downloads on first run
- [ ] CI passes on Linux and macOS
EOF
)"
```

**Step 4: After merge, tag and release**

```bash
git checkout main && git pull
git tag v0.3.0
git push origin v0.3.0
```

This triggers the release workflow, which builds self-contained CLI + GUI binaries for Linux and macOS.
