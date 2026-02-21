# Troubleshooting

Quick fixes for common issues. Each entry follows a **Problem / Cause / Fix** format.

---

## Installation issues

### `command not found: glottisdale`

**Cause:** The glottisdale binary is not on your system PATH.

**Fix:**

If you downloaded a pre-built binary, make sure you moved it to a directory on your PATH:

```bash
sudo mv glottisdale-* /usr/local/bin/glottisdale
chmod +x /usr/local/bin/glottisdale
```

If you built from source, the binary is at `./target/release/glottisdale`. Either add it to your PATH or copy it:

```bash
sudo cp target/release/glottisdale /usr/local/bin/
```

---

### Build fails with `alsa-sys` error (Linux)

**Cause:** The Rust `rodio` audio playback library requires ALSA development headers on Linux.

**Fix:**

```bash
sudo apt install libasound2-dev
```

Then retry the build with `cargo build --release`.

---

### Build fails with cmake error (Linux)

**Cause:** Building whisper-rs and ssstretch requires cmake and a C++ compiler.

**Fix:**

```bash
sudo apt install cmake build-essential
```

Then retry the build with `cargo build --release`.

---

## Runtime errors

### Whisper model download hangs or fails

**Cause:** Glottisdale downloads the Whisper model from Hugging Face on first use. This can stall on slow or restricted networks.

**Fix:** Try a smaller model, which downloads faster:

```bash
glottisdale collage input.mp4 --whisper-model tiny
```

Available models from smallest to largest: `tiny` (~75 MB), `base` (~140 MB), `small` (~460 MB), `medium` (~1.5 GB). The default is `base`.

If the download keeps failing, you can manually download the model from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp/tree/main) and place it at `~/.cache/glottisdale/models/ggml-base.bin`.

---

### Out of memory

**Cause:** The Whisper model is too large for your available RAM. Larger models need more memory.

**Fix:** Switch to a smaller model:

```bash
glottisdale collage input.mp4 --whisper-model tiny
```

Close other memory-heavy applications if possible.

---

### `No speech detected` or empty output

**Cause:** Whisper could not find any speech in the input file. The audio may contain no speech, or the speech may be too quiet relative to background noise.

**Fix:**

- Verify the file actually contains spoken words (play it back and listen).
- If speech is present but very quiet, try amplifying the audio in an editor before processing.
- Check that you are pointing at the right file.

---

### Unsupported audio format

**Cause:** The input file is in a format glottisdale cannot decode. Supported formats are WAV, MP3, and MP4/AAC.

**Fix:**

- Convert your file to WAV first using an audio editor or ffmpeg: `ffmpeg -i input.weird -c:a pcm_s16le output.wav`
- Common supported formats: WAV, MP3, MP4 (AAC audio).

---

## Output doesn't sound right

### All silence

**Cause:** The input had no detected speech, so there were no syllables to assemble.

**Fix:** Use an input file that contains clear spoken words. Background music, sound effects, or ambient noise will not work.

---

### Too short

**Cause:** There are not enough syllables in the input to fill the requested `--target-duration`.

**Fix:**

- Provide more input files or use longer recordings with more speech.
- Lower the target duration to match what you have.

---

### Monotone / robotic

**Cause:** Pitch normalization pulls all syllables to the same pitch, which can sound flat.

**Fix:** Disable pitch normalization for more variety:

```bash
glottisdale collage input.mp4 --no-pitch-normalize
```

Or adjust the pitch range to allow more variation:

```bash
glottisdale collage input.mp4 --pitch-range 8
```

---

### Too choppy

**Cause:** Crossfade between syllables and words is too short, creating audible hard cuts.

**Fix:** Increase crossfade values:

```bash
glottisdale collage input.mp4 --crossfade 50 --word-crossfade 80
```

The defaults are 30ms (syllable) and 50ms (word). Larger values produce smoother transitions.

---

## Caching

Glottisdale caches expensive intermediate results to speed up repeated runs. Caches are stored in `~/.cache/glottisdale/` with three tiers:

| Tier | Directory | What's cached |
|------|-----------|---------------|
| Audio extraction | `extract/` | 16kHz mono WAV resampled from input |
| Whisper transcription | `whisper/` | Word-level timestamps and transcript |
| Alignment | `align/` | Syllable/phoneme-level timestamps |
| Models | `models/` | Downloaded Whisper GGML model files |

Cache keys are derived from the SHA-256 hash of the input file, plus the Whisper model and aligner settings. A second run on the same input files skips extraction (~seconds), transcription (~5-10 min), and alignment (~1-3 min).

### Bypass the cache

To re-process everything from scratch:

```bash
glottisdale collage input.mp4 --no-cache
```

### Clear the cache

To delete all cached data:

```bash
rm -rf ~/.cache/glottisdale/
```

### Override cache location

Set the `GLOTTISDALE_CACHE_DIR` environment variable to store caches somewhere else:

```bash
export GLOTTISDALE_CACHE_DIR=/path/to/custom/cache
```

---

## Platform-specific notes

### macOS

- **Homebrew paths:** If you installed tools via Homebrew on Apple Silicon, they live under `/opt/homebrew/bin/`. This should be on your PATH by default. If not, add it to your shell config.
- **Gatekeeper warnings:** If macOS blocks the downloaded binary, go to System Settings > Privacy & Security and click "Allow Anyway".

### Windows

- **Long path issues:** If you see path-related errors, enable long path support: open PowerShell as Administrator and run `New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" -Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force`.

### Linux

- **Build dependencies:** `sudo apt install libasound2-dev cmake build-essential`
- **GUI dependencies:** `sudo apt install libxkbcommon-dev libgtk-3-dev`
