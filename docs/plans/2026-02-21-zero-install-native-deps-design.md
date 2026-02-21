# Zero-Install Native Dependencies Design

**Goal:** Make the glottisdale GUI (and CLI) fully self-contained binaries with no external tool dependencies — download, run, done.

**Current state:** The codebase shells out to four external CLI tools via `std::process::Command`: ffmpeg (audio extraction), whisper (transcription), rubberband (pitch shift / time stretch), and espeak-ng (forced alignment detection stub). Users must install each separately, which is the main friction to adoption.

**Approach:** Replace each external CLI dependency with a statically-linked or pure-Rust equivalent. Additionally, refactor the GUI to call the core library directly instead of spawning the CLI as a subprocess.

---

## 1. Replace ffmpeg with symphonia

**Problem:** `audio::io::extract_audio()` shells out to ffmpeg to convert any input file to 16kHz mono WAV. This is the only use of ffmpeg in the entire codebase.

**Solution:** Use the `symphonia` crate (pure Rust) to decode audio from MP4, WAV, and MP3 files directly to raw PCM samples.

**Details:**
- Add symphonia with codecs for MP3, AAC, WAV, and the `isomp4` demuxer for MP4 containers.
- Write `extract_audio_native()` that: decodes via symphonia -> converts to mono (average channels) -> resamples to 16kHz via rubato (already in the codebase) -> returns `(Vec<f64>, u32)`.
- Change `extract_audio()` signature from file-to-file to file-to-samples, eliminating the temp WAV round-trip. Callers currently `read_wav()` the output immediately anyway.
- Keep the old ffmpeg path as a fallback behind a feature flag for edge-case formats, but it's no longer required.

**Supported formats:** MP4 (AAC audio track), WAV (all bit depths), MP3. Covers ~95% of real-world input files.

**File:** `crates/core/src/audio/io.rs`

---

## 2. Enable whisper-rs by default with model auto-download

**Problem:** Transcription requires the user to install Python and `pip install openai-whisper`. This is the largest friction point.

**Solution:** Enable the existing `whisper-native` feature flag by default. Add automatic model download on first run.

**Details:**
- Enable `whisper-native` feature by default in `crates/core/Cargo.toml`. Keep CLI fallback as a last resort.
- Add model auto-download to `find_model()`: detect missing model -> download GGML binary from Hugging Face (`https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`) -> save to `~/.cache/glottisdale/models/` -> log progress.
- Use `reqwest` with streaming for the download with progress logging.
- The `whisper-rs` crate statically links whisper.cpp — same model, same inference quality, no Python required.

**Model sizes:** tiny (~75MB), base (~140MB), small (~460MB), medium (~1.5GB). Default is base.

**File:** `crates/core/src/language/transcribe.rs`

---

## 3. Statically link rubberband

**Problem:** Rubberband CLI is required for high-quality pitch shifting and time stretching. Without it, the codebase falls back to basic resampling-based implementations that produce audible artifacts on vocal audio.

**Solution:** Use the `rubberband` crate (Rust bindings) to statically link the rubberband library into the binary.

**Details:**
- Replace `Command::new("rubberband")` blocks in `audio/effects.rs` with calls to the rubberband library API.
- Remove the temp-file dance (write WAV -> shell out -> read WAV back). Work directly on sample buffers.
- Remove the simple fallback implementations (`pitch_shift_simple`, `time_stretch_simple`) or keep them behind a feature flag for minimal builds.
- Requires a C++ compiler at build time (CI already has this), but users get a single binary with no runtime dependency.

**Quality:** Identical to the current rubberband CLI path. Phase vocoder preserves formants and timing independently — important for a tool that manipulates vocal audio.

**File:** `crates/core/src/audio/effects.rs`

---

## 4. GUI calls core library directly

**Problem:** The GUI spawns the CLI binary as a subprocess, parses stdout for output paths, and shows stderr in the log viewer. This requires the CLI binary to be co-located with the GUI, makes progress reporting fragile, and is architecturally wrong.

**Solution:** The GUI imports `glottisdale_core` directly and calls the pipeline functions.

**Details:**
- Call `collage::process::process()`, `speak::assembler::assemble()`, and sing pipeline functions directly from the GUI.
- For progress/logging: register a custom `log` handler that forwards log lines to the GUI's log buffer (`Arc<Mutex<Vec<String>>>`). No signature changes to core functions needed since they already use `log::info!()` throughout.
- The GUI handles `create_run_dir()` itself, then passes the `output_dir` to core functions.
- Threading: same background thread pattern already used for the subprocess, just calling core functions instead of `Command::new()`.
- Delete `run_cli_subprocess()` and all stdout parsing logic. Output paths come directly from function return values (`PipelineResult.concatenated`, etc.).

**Files:** `crates/gui/src/app.rs`, `crates/gui/Cargo.toml`

---

## 5. Clean up dead code

After the above changes:
- Remove espeak-ng and python3 detection stubs in `language/align.rs` (BFA is tracked separately as a future enhancement — see GitHub issue).
- Remove ffmpeg feature flag if the fallback is not worth maintaining.
- Update all documentation (README, install.md, quickstart.md, troubleshooting.md) to reflect zero external dependencies.
- Update CI/release workflow if build dependencies change.

---

## Result

After this work, the release artifacts are:

| Binary | What it is | External deps |
|--------|-----------|---------------|
| `glottisdale` | CLI tool | None (whisper model auto-downloads on first use) |
| `glottisdale-gui` | Native desktop app | None (whisper model auto-downloads on first use) |

**macOS:** Truly download-and-run. Zero system dependencies.

**Linux:** Needs system windowing libraries (GTK, libxkbcommon) for the GUI, which are present on any desktop Linux. CLI has zero dependencies.

---

## Known gap: Forced alignment

BFA (Bournemouth Forced Aligner) is currently a detection stub — it checks for espeak-ng and python3 but the actual alignment is not implemented in the Rust codebase. The DefaultAligner (Whisper + CMU dict g2p) is what all users currently get.

Improving alignment accuracy is tracked as a separate GitHub issue. Options include statically linking espeak-ng, implementing a Rust-native forced aligner, or using whisper-rs's token-level timestamps more aggressively.

---

## Dependencies added

| Crate | Purpose | Pure Rust? |
|-------|---------|------------|
| `symphonia` + codec/demux features | Audio decoding (MP4, WAV, MP3) | Yes |
| `whisper-rs` | Speech transcription (statically links whisper.cpp) | No (C/C++ compiled in) |
| `rubberband` (crate) | Pitch shift / time stretch (statically links librubberband) | No (C++ compiled in) |
| `reqwest` | Model download | Yes |

## Dependencies removed (from user's system)

- ffmpeg
- Python + openai-whisper
- rubberband CLI
- espeak-ng (was never actually used)
