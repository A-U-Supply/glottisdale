# Design: `glottisdale speak` Subcommand

**Date:** 2026-02-20
**Status:** Approved

## Summary

A new `speak` subcommand that takes source audio as a "voice bank" and reconstructs target text using syllable fragments from the source. The result is an uncanny, almost-perceptible recreation of the input text in the source speaker's voice.

## Input Modes

**Text mode:** Provide target text directly.

```
glottisdale speak source.mp4 --text "the quick brown fox"
```

**Reference audio mode:** Provide a second audio file. It is transcribed to get the target text, and its timing/prosody is used as a pacing template.

```
glottisdale speak source.mp4 --reference guide.mp4
```

In both cases, `source.mp4` is the voice bank (where syllables come from). The reference audio is never used for syllable extraction.

## Architecture

### Pipeline

```
Source Audio --> Transcribe (Whisper) --> Syllabify (g2p_en) --> Align (BFA/default) --> Source Syllable Bank
                                                                                             |
Target Text --> Syllabify (g2p_en) --> [Optional: Reference Audio timing] -----------------> Match & Assemble --> Output
```

### New Module: `src/glottisdale/speak/`

- `__init__.py` -- main `process()` orchestrator
- `phonetic_distance.py` -- ARPABET feature matrix and distance calculations
- `syllable_bank.py` -- builds the indexed bank of source syllables
- `matcher.py` -- matches target syllables/phonemes to source bank
- `assembler.py` -- concatenates matched audio with crossfading and optional pitch correction

The source audio reuses the existing collage pipeline steps (transcribe, syllabify, align, cut) to build the syllable bank.

## Phonetic Distance Matching

### ARPABET Feature Matrix

Each ARPABET phoneme gets a feature vector encoding articulatory properties:

- **Vowels:** height (high/mid/low), backness (front/central/back), roundness, tenseness
- **Consonants:** manner (stop/fricative/nasal/liquid/glide), place (bilabial/labiodental/dental/alveolar/palatal/velar/glottal), voicing

Distance between two phonemes = weighted sum of differing features. Examples:

| Pair     | Distance | Reason                        |
|----------|----------|-------------------------------|
| B vs P   | 1        | Differ only in voicing        |
| B vs D   | 1        | Differ only in place          |
| B vs S   | 3+       | Differ in manner, place, voicing |
| AH vs AE | 1       | Differ in height              |

### Syllable-Level Matching (`--match-unit syllable`, default)

1. Convert target text to ARPABET syllables via g2p_en + existing syllabifier
2. For each target syllable, score every source syllable using sum of phoneme-pair distances (with alignment for different-length syllables)
3. Pick the lowest-distance source syllable

### Phoneme-Level Matching (`--match-unit phoneme`)

1. Convert target text to individual ARPABET phonemes
2. For each target phoneme, find the closest source phoneme and its audio segment
3. Concatenate phoneme-level audio clips with short crossfades

### Tie-Breaking

When multiple source syllables tie on distance, prefer:

1. Same syllable stress level (ARPABET stress markers: 0, 1, 2)
2. Similar duration to target estimate
3. First occurrence (deterministic)

### Fallback

Always use the best available match, even if poor. The uncanny effect emerges naturally from imperfect matches.

## Assembly & Output

### Timing

**Text mode (no reference audio):** Estimate natural pacing using heuristics -- average syllable duration from the source audio, with slightly longer pauses at word boundaries and longer still at punctuation. The source speaker's natural tempo sets the baseline.

**Reference audio mode:** Transcribe and align the reference audio to get word/syllable timestamps. Use these as the pacing template. `--timing-strictness` (0.0--1.0, default 0.8) controls how tightly the output follows reference timing:

- `1.0` = syllables time-stretched to land exactly on reference timestamps
- `0.5` = blend between reference timing and natural source syllable durations
- `0.0` = ignore reference timing, use source durations

### Pitch Correction (`--pitch-correct`, default on)

When enabled, estimate a natural F0 contour for the target text (declining pitch within phrases, rising at questions, emphasis on stressed syllables). Pitch-shift source syllables toward this target contour using existing `analysis.py` F0 extraction and `audio.py` pitch shifting.

When disabled, source syllables keep their original pitch.

### Crossfading

Reuse existing crossfade logic from the collage pipeline. Short crossfades (5--15ms) between syllables within words, ~30ms between words.

### Output Structure

```
glottisdale-output/2026-02-20-velvet-larynx/
  speak.wav          # final assembled audio
  syllable-bank.json # source syllable inventory with phonetic labels
  match-log.json     # which source syllable matched each target, with distances
```

## CLI Interface

```
glottisdale speak SOURCE [options]

Positional:
  SOURCE                              Audio/video file to use as voice bank

Target (one required):
  --text TEXT                         Target text to speak
  --reference FILE                    Reference audio -- transcribed for text + timing

Matching:
  --match-unit {syllable,phoneme}     Matching granularity (default: syllable)
  --pitch-correct / --no-pitch-correct  Adjust pitch to target intonation (default: on)

Timing (reference mode only):
  --timing-strictness FLOAT           How closely to follow reference timing, 0.0-1.0 (default: 0.8)

Core (shared with other subcommands):
  --output-dir DIR                    Output root directory (default: ./glottisdale-output)
  --run-name NAME                     Custom run name (default: auto-generated)
  --seed INT                          Random seed for reproducibility
  --model {tiny,base,small,medium,large}  Whisper model (default: small)
  --aligner {default,bfa}             Alignment method (default: default)

Audio polish (reused from collage):
  --crossfade MS                      Crossfade duration in ms (default: 10)
  --normalize-volume                  Normalize volume across syllables
```

## Testing Strategy

### Unit Tests (`tests/speak/`)

- `test_phonetic_distance.py` -- distance matrix properties (symmetry, triangle inequality, known pairs), edge cases (identical=0, stress markers)
- `test_syllable_bank.py` -- building from transcribed/aligned source, indexing, lookup
- `test_matcher.py` -- syllable-level and phoneme-level matching against mock bank, tie-breaking, fallback
- `test_assembler.py` -- crossfading, pitch correction, timing with and without reference

### Integration Tests (`tests/speak/test_integration.py`)

- Source audio + text --> output wav exists with expected duration
- Source audio + reference audio --> output wav with timing close to reference
- Verify `match-log.json` and `syllable-bank.json` are written

### CLI Tests (`tests/speak/test_cli.py`)

- `--text` and `--reference` are mutually exclusive
- `--timing-strictness` only accepted with `--reference`
- `--match-unit` accepts `syllable` and `phoneme`, rejects invalid values
- Run directory creation works with `speak` subcommand

All tests mock Whisper/ffmpeg/rubberband calls, consistent with existing test patterns.
