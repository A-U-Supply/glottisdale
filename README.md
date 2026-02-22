# Glottisdale

Syllable-level audio collage, vocal MIDI mapping, and phonetic speech reconstruction tool.

Glottisdale takes speech audio, segments it into syllables, and reassembles them into surreal audio collages. It can also map syllable clips onto MIDI melodies to produce "drunk choir" vocal tracks, or reconstruct target text by matching syllable fragments from source audio using phonetic distance. Feed it any video or audio with speech and get back something that sounds like language but isn't.

## Quick Start

```bash
# CLI
glottisdale collage your-video.mp4

# GUI
glottisdale-gui
```

Each run creates a unique subdirectory like `./glottisdale-output/2026-02-19-breathy-bassoon/` — the main output WAV and clips zip are named after the run directory (e.g., `2026-02-19-breathy-bassoon.wav`). Runs never overwrite each other.

## Install

No external dependencies required. Download, make executable, run.

The Whisper speech recognition model (~140 MB) downloads automatically on first run.

### macOS (Apple Silicon)

**Option A: DMG (recommended)**

1. Download `Glottisdale-darwin-arm64.dmg` from the [latest release](https://github.com/A-U-Supply/glottisdale/releases/latest)
2. Open the DMG and drag **Glottisdale.app** into your **Applications** folder
3. The DMG also includes the `glottisdale` CLI binary — copy it to `/usr/local/bin/` if you want CLI access

**Option B: Standalone binaries**

```bash
# Download
curl -L https://github.com/A-U-Supply/glottisdale/releases/latest/download/glottisdale-gui-darwin-arm64 -o glottisdale-gui
curl -L https://github.com/A-U-Supply/glottisdale/releases/latest/download/glottisdale-darwin-arm64 -o glottisdale

# Make executable
chmod +x glottisdale-gui glottisdale

# Launch the GUI
./glottisdale-gui
```

Optionally move both binaries somewhere on your PATH so you can run them from anywhere:

```bash
sudo mv glottisdale glottisdale-gui /usr/local/bin/
```

#### macOS security (Gatekeeper)

Since Glottisdale is not code-signed with an Apple Developer certificate, macOS will block it on first launch. Here's how to allow it:

1. **Try to open the app** — double-click Glottisdale.app (or run `./glottisdale-gui` in the terminal). macOS will show a dialog saying it "can't be opened because Apple cannot check it for malicious software."
2. **Open System Settings** — go to **System Settings > Privacy & Security**
3. **Scroll down** to the "Security" section. You'll see a message like *"Glottisdale-gui was blocked from use because it is not from an identified developer."*
4. **Click "Allow Anyway"** and enter your password
5. **Run the app again** — this time macOS will show a final dialog. Click **"Open"** to confirm.

You only need to do this once. After that, macOS will remember your choice.

**Alternative (terminal):** You can also remove the quarantine attribute directly:

```bash
xattr -cr /Applications/Glottisdale.app
# or for standalone binaries:
xattr -cr ./glottisdale-gui ./glottisdale
```

### Linux (x86_64)

```bash
# Download
curl -L https://github.com/A-U-Supply/glottisdale/releases/latest/download/glottisdale-gui-linux-amd64 -o glottisdale-gui
curl -L https://github.com/A-U-Supply/glottisdale/releases/latest/download/glottisdale-linux-amd64 -o glottisdale

# Make executable
chmod +x glottisdale-gui glottisdale

# Launch the GUI
./glottisdale-gui
```

Optionally move both binaries somewhere on your PATH:

```bash
sudo mv glottisdale glottisdale-gui /usr/local/bin/
```

### From source

```bash
# Requires Rust 1.75+
git clone https://github.com/A-U-Supply/glottisdale.git
cd glottisdale
cargo build --release

# Launch the GUI
./target/release/glottisdale-gui

# Or use the CLI
./target/release/glottisdale --help
```

## Architecture

Cargo workspace with three crates:

- **`glottisdale-core`** — Library with all processing logic (audio I/O, language processing, pipelines)
- **`glottisdale`** — CLI binary (clap)
- **`glottisdale-gui`** — Native GUI binary (egui/eframe)

### Core modules

| Module | Description |
|--------|-------------|
| `audio::io` | WAV read/write, multi-format extraction via symphonia, resampling |
| `audio::analysis` | F0 estimation, RMS, room tone, breath detection, pink noise |
| `audio::effects` | Pitch shift and time stretch via Signalsmith Stretch, volume, crossfade, mixing |
| `audio::playback` | Real-time audio playback via rodio |
| `language::g2p` | Grapheme-to-phoneme via embedded CMU dict |
| `language::syllabify` | ARPABET and IPA syllabifiers |
| `language::phonotactics` | Sonority-based syllable ordering |
| `language::transcribe` | Native Whisper transcription via whisper-rs with auto model download |
| `language::align` | Alignment backend (default: Whisper + g2p) |
| `cache` | SHA-256 file hashing, atomic writes |
| `names` | Thematic run name generator |
| `collage` | Syllable sampling, stretch, stutter, prosodic grouping |
| `speak` | Phonetic distance, syllable bank, Viterbi matching, assembly |
| `sing` | MIDI parsing, vocal mapping, synthesis, mixing |

## CLI Reference

### `glottisdale collage`

Create a syllable-level audio collage from speech.

```
glottisdale collage [input_files...] [options]

Positional:
  input_files              Audio/video files to process

Options:
  --output-dir DIR         Output root directory (default: ./glottisdale-output)
  --run-name NAME          Custom run name (default: auto-generated thematic name)
  --target-duration SECS   Target duration (default: 30)
  --seed N                 RNG seed for reproducibility
  --whisper-model MODEL    tiny/base/small/medium (default: base)
  --aligner MODE           auto/default/bfa (default: auto)
  --no-cache               Disable file-based caching (re-run everything)
  -v, --verbose            Show all dependency warnings (default: quiet)

Prosodic grouping:
  --syllables-per-word N   Syllables per word: '3' or '1-4' (default: 1-4)
  --words-per-phrase N     Words per phrase: '4' or '3-5' (default: 3-5)
  --phrases-per-sentence N Phrases per sentence: '2' or '2-3' (default: 2-3)
  --phrase-pause MS        Silence between phrases (default: 400-700)
  --sentence-pause MS      Silence between sentences (default: 800-1200)
  --crossfade MS           Syllable crossfade (default: 30)
  --word-crossfade MS      Word crossfade (default: 50)

Audio polish (all on by default, use --no-* to disable):
  --no-pitch-normalize     Disable pitch normalization
  --no-volume-normalize    Disable volume normalization
  --no-room-tone           Disable room tone extraction
  --no-breaths             Disable breath insertion
  --no-prosodic-dynamics   Disable phrase-level dynamics
  --noise-level DB         Pink noise bed level (default: -40, 0=off)
  --breath-probability P   Breath insertion probability (default: 0.6)
  --pitch-range SEMI       Max pitch shift in semitones (default: 5)

Time stretch (all off by default):
  --speed FACTOR           Global speed (0.5=half, 2.0=double)
  --random-stretch P       Probability of stretching a syllable
  --alternating-stretch N  Stretch every Nth syllable
  --boundary-stretch N     Stretch first/last N syllables per word
  --word-stretch P         Probability of stretching all syllables in a word
  --stretch-factor F       Stretch amount: '2.0' or '1.5-3.0'

Word repeat (all off by default):
  --repeat-weight P        Probability of repeating a word
  --repeat-count N         Extra copies: '2' or '1-3' (default: 1-2)
  --repeat-style MODE      exact or resample (default: exact)

Stutter (all off by default):
  --stutter P              Probability of stuttering a syllable
  --stutter-count N        Extra copies: '2' or '1-3' (default: 1-2)
```

### `glottisdale sing`

Map syllable clips onto MIDI melody notes.

```
glottisdale sing [input_files...] --midi DIR [options]

Positional:
  input_files              Audio/video files to process

Required:
  --midi DIR               Directory with MIDI files (melody.mid, etc.)

Options:
  --output-dir DIR         Output root directory (default: ./glottisdale-output)
  --run-name NAME          Custom run name (default: auto-generated thematic name)
  --target-duration SECS   Target duration (default: 30)
  --seed N                 RNG seed for reproducibility
  --whisper-model MODEL    tiny/base/small/medium (default: base)
  --drift-range SEMI       Max pitch drift from melody (default: 2.0)
  --no-cache               Disable file-based caching (re-run everything)
  --no-vibrato             Disable vibrato
  --no-chorus              Disable chorus
```

### `glottisdale speak`

Reconstruct target text using syllable fragments from source audio.

```
glottisdale speak [input_files...] --text TEXT [options]
glottisdale speak [input_files...] --reference REF_AUDIO [options]

Positional:
  input_files              Audio/video files to use as syllable source

Target (one required):
  --text TEXT              Target text to reconstruct using source syllables
  --reference FILE         Reference audio -- transcribed for target text + timing template

Options:
  --output-dir DIR         Output root directory (default: ./glottisdale-output)
  --run-name NAME          Custom run name (default: auto-generated thematic name)
  --seed N                 RNG seed for reproducibility
  --whisper-model MODEL    tiny/base/small/medium (default: base)
  --aligner MODE           auto/default/bfa (default: auto)
  --no-cache               Disable file-based caching (re-run everything)
  -v, --verbose            Show all dependency warnings (default: quiet)

Speak-specific:
  --match-unit UNIT        syllable or phoneme (default: syllable)
  --no-pitch-correct       Disable pitch correction (on by default)
  --timing-strictness F    How closely to follow reference timing, 0.0-1.0 (default: 0.8)
  --crossfade MS           Crossfade between syllables in ms (default: 10)
  --no-normalize-volume    Disable volume normalization (on by default)
```

### `glottisdale-gui`

Native desktop GUI. Tab-based interface with file picker, settings panels, and log viewer for all three pipelines.

#### Interactive Syllable Editor

After running any pipeline, click **"Edit Arrangement"** to open the interactive editor. You can also click **"Build Bank & Edit"** to skip the pipeline and jump straight into manual arrangement.

The editor provides:

- **Syllable bank** — all aligned syllables from your source audio, with waveform previews. Use the search field to filter by phoneme or word text. Click to add to the timeline; each entry has a ▶ play button for quick preview.
- **Timeline** — drag-to-reorder clips, zoom/pan (Ctrl+scroll / scroll), click to select, Shift+click for multi-select. Clips display their waveform shape and phoneme label.
- **Effects** — right-click any clip for stutter (x2-x8), time stretch (0.5x-4x), pitch shift (-12 to +12 semitones), duplicate, delete, and clear effects.
- **Playback** — Play/Pause/Stop with a moving cursor. Plays from cursor position. Errors display as red text in the toolbar with a dismiss button.
- **Export** — render the arrangement to a WAV file.

**Keyboard shortcuts:** Space (play/pause), Ctrl+A (select all), Ctrl+scroll (zoom), scroll (pan).

## Dependencies

- **Rust 1.75+** for building from source
- No external runtime dependencies — all audio processing, transcription, and pitch/time manipulation is handled natively

## License

GPL v3 — see [LICENSE](LICENSE).
