# Glottisdale

Syllable-level audio collage and vocal MIDI mapping tool.

Glottisdale takes speech audio, segments it into syllables, and reassembles them into surreal audio collages. It can also map syllable clips onto MIDI melodies to produce "drunk choir" vocal tracks.

## Installation

### System dependencies

- **ffmpeg** — required for all audio processing
- **espeak-ng** — required for BFA alignment (optional)
- **rubberband** — required for pitch/time stretch in `sing` mode (optional)

### Install from GitHub

```bash
# Core (collage only)
pip install git+https://github.com/A-U-Supply/glottisdale.git

# With vocal MIDI mapping
pip install "glottisdale[sing] @ git+https://github.com/A-U-Supply/glottisdale.git"

# Everything
pip install "glottisdale[all] @ git+https://github.com/A-U-Supply/glottisdale.git"
```

### Development

```bash
git clone https://github.com/A-U-Supply/glottisdale.git
cd glottisdale
pip install -e '.[all,dev]'
python -c "import nltk; nltk.download('averaged_perceptron_tagger_eng')"
```

## Quick Start

### Syllable collage

```bash
glottisdale collage input.mp4 -o output/ --target-duration 30
```

### Vocal MIDI mapping

```bash
glottisdale sing --midi midi-dir/ input.mp4 -o output/
```

### Python API

```python
from glottisdale.collage import process

result = process(
    input_paths=[Path("speech.wav")],
    target_duration=20,
    seed=42,
)
# result.concatenated -> Path to output WAV
# result.clips -> list of Clip objects
```

## CLI Reference

### `glottisdale collage`

Create a syllable-level audio collage from speech.

```
glottisdale collage [input_files...] [options]

Positional:
  input_files              Audio/video files to process

Options:
  --output-dir DIR         Output directory (default: ./glottisdale-output)
  --target-duration SECS   Target duration (default: 30)
  --seed N                 RNG seed for reproducibility
  --whisper-model MODEL    tiny/base/small/medium (default: base)
  --aligner MODE           auto/default/bfa (default: auto)

Prosodic grouping:
  --syllables-per-word N   Syllables per word: '3' or '1-4' (default: 1-4)
  --words-per-phrase N     Words per phrase: '4' or '3-5' (default: 3-5)
  --phrases-per-sentence N Phrases per sentence: '2' or '2-3' (default: 2-3)
  --phrase-pause MS        Silence between phrases (default: 400-700)
  --sentence-pause MS      Silence between sentences (default: 800-1200)
  --crossfade MS           Syllable crossfade (default: 30)
  --word-crossfade MS      Word crossfade (default: 50)

Audio polish:
  --pitch-normalize / --no-pitch-normalize    (default: on)
  --volume-normalize / --no-volume-normalize  (default: on)
  --room-tone / --no-room-tone                (default: on)
  --breaths / --no-breaths                    (default: on)
  --prosodic-dynamics / --no-prosodic-dynamics (default: on)
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
  --output-dir DIR         Output directory (default: ./glottisdale-output)
  --target-duration SECS   Target duration (default: 30)
  --seed N                 RNG seed for reproducibility
  --whisper-model MODEL    tiny/base/small/medium (default: base)
  --drift-range SEMI       Max pitch drift from melody (default: 2.0)
  --vibrato / --no-vibrato (default: on)
  --chorus / --no-chorus   (default: on)
```

## Pipeline Architecture

### Collage pipeline

1. Extract audio (ffmpeg → 16kHz mono WAV)
2. Transcribe (Whisper ASR → word timestamps)
3. Phoneme conversion (g2p_en → ARPABET)
4. Syllabification (Maximum Onset Principle)
5. Sample syllables to target duration
6. Group into words → phrases → sentences
7. Cut syllable clips with padding
8. Pitch normalization (autocorrelation F0 → median → ffmpeg asetrate)
9. Volume normalization (RMS → median)
10. Stutter, syllable stretch, word assembly
11. Word stretch, word repeat
12. Phrase assembly with crossfade
13. Prosodic dynamics (onset boost, final softening)
14. Gap generation (room tone or silence + breaths)
15. Final concatenation
16. Global speed adjustment
17. Pink noise bed mixing

### Sing pipeline

1. Parse MIDI melody (pretty_midi → Note/MidiTrack)
2. Transcribe + syllabify audio sources (reuses collage modules)
3. Normalize syllable pitch to median F0 (rubberband)
4. Normalize syllable volume to median RMS
5. Plan note mapping (syllable assignment, drift, vibrato/chorus decisions)
6. Render each note (rubberband pitch shift + time stretch)
7. Apply vibrato and chorus effects
8. Assemble vocal timeline with gaps
9. Synthesize MIDI backing (sine waves)
10. Mix vocal + backing

## BFA Aligner

The [Bournemouth Forced Aligner](https://pypi.org/project/bournemouth-forced-aligner/) provides real phoneme-level timestamps from the audio signal, producing more precise syllable boundaries than the default proportional approach.

```bash
pip install "glottisdale[bfa]"
sudo apt-get install espeak-ng  # or: brew install espeak-ng

glottisdale collage input.mp4 --aligner bfa
```

The `--aligner auto` mode (default) tries BFA first and falls back to the default aligner if BFA is not installed.

## License

GPL v3 — see [LICENSE](LICENSE).
