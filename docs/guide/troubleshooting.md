# Troubleshooting

Quick fixes for common issues. Each entry follows a **Problem / Cause / Fix** format.

---

## Installation issues

### `ffmpeg: command not found`

**Cause:** ffmpeg is not installed, or it is installed but not on your system PATH.

**Fix:**

- **macOS:** `brew install ffmpeg`
- **Linux:** `sudo apt install ffmpeg`
- **Windows:** `winget install ffmpeg` in PowerShell, or download from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) and add the `bin` folder to your PATH.

After installing, open a new terminal window and run `ffmpeg -version` to confirm.

---

### `whisper: command not found`

**Cause:** OpenAI Whisper is not installed, or Python's scripts directory is not on your PATH.

**Fix:** Install Whisper with pip:

```bash
pip install openai-whisper
```

If `whisper --help` still fails after install, your Python scripts directory may not be on your PATH. The typical location is `~/.local/bin`. Add it to your shell config:

```bash
# bash / zsh
export PATH="$HOME/.local/bin:$PATH"
```

```fish
# fish
fish_add_path ~/.local/bin
```

Then open a new terminal window.

---

### `espeak-ng` not found (BFA mode)

**Cause:** The Bournemouth Forced Aligner requires espeak-ng, which is not installed on your system.

**Fix:** Install espeak-ng:

- **macOS:** `brew install espeak-ng`
- **Linux:** `sudo apt install espeak-ng`
- **Windows:** Download from the [espeak-ng releases](https://github.com/espeak-ng/espeak-ng/releases) and add to your PATH.

If you don't need BFA, switch to the default aligner instead:

```bash
glottisdale collage input.mp4 --aligner default
```

---

### `rubberband` not found (sing mode)

**Cause:** The `sing` subcommand uses rubberband for pitch-shifting and time-stretching, and it is not installed.

**Fix:**

- **macOS:** `brew install rubberband`
- **Linux:** `sudo apt install rubberband-cli`
- **Windows:** Download from the [Rubber Band Library releases](https://breakfastquay.com/rubberband/) and add to your PATH.

---

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

## Runtime errors

### Whisper model download hangs or fails

**Cause:** Whisper needs to download its model files on first use. This can stall on slow or restricted networks.

**Fix:** Try a smaller model, which downloads faster:

```bash
glottisdale collage input.mp4 --whisper-model tiny
```

Available models from smallest to largest: `tiny`, `base`, `small`, `medium`. The default is `base`.

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

### Unsupported input format

**Cause:** ffmpeg cannot read the file. It may be corrupted, DRM-protected, or in an unusual format.

**Fix:**

- Test the file directly: `ffplay input.mp4` (or `ffprobe input.mp4`). If ffmpeg itself cannot read the file, glottisdale cannot either.
- Try converting to a standard format first: `ffmpeg -i input.weird -c:a pcm_s16le output.wav`
- Common supported formats: WAV, MP3, MP4, FLAC, OGG, M4A.

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

- **Adding ffmpeg to PATH:** If you downloaded ffmpeg manually, extract the archive, find the `bin` folder inside, and add its full path (e.g., `C:\ffmpeg\bin`) to your system PATH via Settings > System > About > Advanced system settings > Environment Variables > Path > Edit > New.
- **PowerShell vs Command Prompt:** Both work. PowerShell is recommended as it comes with modern Windows and supports `winget`.
- **Long path issues:** If you see path-related errors, enable long path support: open PowerShell as Administrator and run `New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" -Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force`.

### Linux

- **Debian/Ubuntu packages:** `sudo apt install ffmpeg espeak-ng rubberband-cli libasound2-dev`
- **Fedora:** `sudo dnf install ffmpeg espeak-ng rubberband`
- **Arch:** `sudo pacman -S ffmpeg espeak-ng rubberband`
