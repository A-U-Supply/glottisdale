# Architecture

Glottisdale is a Rust workspace with three crates: **core** (library), **cli** (command-line binary), and **gui** (native desktop app). The core crate contains all processing logic; the CLI and GUI are thin frontends that parse user input and call into core.

All three pipelines — **collage** (syllable-level audio collage), **sing** (vocal MIDI mapping), and **speak** (phonetic speech reconstruction) — share a common foundation of audio extraction, Whisper transcription, and g2p syllabification, then diverge into their respective assembly strategies.

## Crate structure

```
glottisdale/
├── crates/
│   ├── core/        glottisdale-core    Library: all processing logic
│   ├── cli/         glottisdale         CLI binary (clap)
│   └── gui/         glottisdale-gui     Native GUI binary (egui/eframe)
├── .github/workflows/
│   ├── ci.yml                           Build + test on PR
│   └── release.yml                      Build release binaries on tag
└── docs/
```

## Collage pipeline

The collage pipeline takes speech audio, segments it into syllables, shuffles and regroups them into nonsense "words" and "phrases," normalizes their pitch and volume, and concatenates everything into a surreal audio collage.

| Step | Operation | Module |
|------|-----------|--------|
| 1 | **Extract audio** -- ffmpeg resamples input to 16 kHz mono WAV | `audio::io` |
| 2 | **Transcribe** -- Whisper ASR produces word-level timestamps | `language::transcribe` |
| 3 | **Phoneme conversion** -- CMU dict converts words to ARPABET phoneme sequences | `language::g2p` |
| 4 | **Syllabification** -- Maximum Onset Principle splits phonemes into syllables with proportional timing | `language::syllabify` + `language::syllabify_arpabet` |
| 5 | **Sample syllables** -- randomly select and shuffle syllables to fill the target duration; round-robin across sources when multiple inputs are provided | `collage::process` |
| 6 | **Group into words, phrases, sentences** -- syllables are grouped into variable-length "words" (with phonotactic reordering), words into phrases, phrases into sentences | `collage::process` + `language::phonotactics` |
| 7 | **Cut syllable clips** -- extract each syllable from the source WAV with configurable padding and fade | `audio::io` |
| 8 | **Pitch normalization** -- estimate F0 via autocorrelation for each clip, compute the median, shift outliers toward it using ffmpeg `asetrate`/`aresample` | `audio::analysis` + `audio::effects` |
| 9 | **Volume normalization** -- compute RMS for each clip, shift to the median using ffmpeg `volume` filter | `audio::analysis` + `audio::effects` |
| 10 | **Stutter, syllable stretch, word assembly** -- optionally duplicate syllables (stutter) and time-stretch individual syllables, then concatenate syllables within each word using crossfade | `collage::stretch` + `audio::effects` |
| 11 | **Word stretch, word repeat** -- optionally time-stretch assembled word WAVs and/or duplicate words in the sequence | `collage::stretch` + `audio::effects` |
| 12 | **Phrase assembly with crossfade** -- concatenate words into phrases using a wider crossfade | `audio::effects` |
| 13 | **Prosodic dynamics** -- apply onset boost (+1.12 dB over the first 20% of each phrase) and final softening (-3 dB from 70% onward) via ffmpeg `volume` filter | `collage::process` |
| 14 | **Gap generation** -- create inter-phrase and inter-sentence gaps using extracted room tone (mixed over silence) or plain silence, optionally prepend detected breath clips | `audio::analysis` + `audio::effects` |
| 15 | **Final concatenation** -- concatenate all phrases and gaps into one continuous output | `audio::effects` |
| 16 | **Global speed adjustment** -- time-stretch the entire output using rubberband (pitch-preserving) | `audio::effects` |
| 17 | **Pink noise bed mixing** -- generate pink noise via spectral shaping (1/sqrt(f) FFT filter), mix under the output at a configurable dB level | `audio::analysis` + `audio::effects` |

### Data flow

```
input.mp4
  |
  v
[1] audio::io ---------> 16kHz mono WAV
  |
  v
[2] language::transcribe -> word timestamps
  |
  v
[3-4] language::g2p + syllabify -> Syllable objects (phonemes, timing, word)
  |
  v
[5] collage::process -----> selected syllables (shuffled)
  |
  v
[6] collage::process -----> words -> phrases -> sentences
  |
  v
[7] audio::io ------------> individual syllable WAVs
  |
  v
[8-9] audio::analysis + effects -> pitch- and volume-normalized WAVs
  |
  v
[10-11] collage::stretch -> modified word WAVs
  |
  v
[12-13] collage::process -> phrase WAVs with crossfade + dynamics
  |
  v
[14-15] audio::effects ---> continuous output WAV
  |
  v
[16-17] audio::effects ---> final output WAV
```

## Sing pipeline

The sing pipeline maps syllable clips onto MIDI melody notes to produce a "drunk choir" vocal track. It reuses the shared language modules for transcription and syllabification, then applies rubberband-based pitch shifting and time stretching to fit syllables to the melody.

| Step | Operation | Module |
|------|-----------|--------|
| 1 | **Parse MIDI melody** -- extract notes and track metadata from the melody file | `sing::midi_parser` |
| 2 | **Transcribe + syllabify** -- reuses Whisper transcription and g2p syllabification to produce syllable clips | `sing::syllable_prep` (via `language::*`) |
| 3 | **Normalize syllable pitch** -- shift each syllable's F0 to the median using rubberband pitch filter | `sing::syllable_prep` |
| 4 | **Normalize syllable volume** -- adjust each clip's RMS to the median using ffmpeg volume filter | `sing::syllable_prep` (via `audio::effects`) |
| 5 | **Plan note mapping** -- assign syllable indices to each melody note, choose pitch drift (Gaussian around 0), and flag vibrato/chorus per note based on duration | `sing::vocal_mapper` |
| 6 | **Render each note** -- compute the semitone shift from median F0 to the MIDI note frequency, then apply rubberband pitch shift + time stretch to fit the note duration | `sing::vocal_mapper` |
| 7 | **Apply vibrato and chorus** -- vibrato via ffmpeg `vibrato` filter on sustained notes; chorus via detuned/delayed voice layering with `amix` | `sing::vocal_mapper` |
| 8 | **Assemble vocal timeline** -- place rendered notes at their MIDI start times with silence gaps between them, crossfade adjacent clips | `sing::vocal_mapper` |
| 9 | **Synthesize MIDI backing** -- render melody, chords, bass, and drums from MIDI files using sine-wave synthesis and noise-burst drum synthesis | `sing::synthesize` |
| 10 | **Mix vocal + backing** -- mix the a cappella vocal track over the synthesized backing at configurable dB levels (default: vocal at 0 dB, MIDI at -12 dB) | `sing::mixer` |

### Data flow

```
input.mp4 + melody.mid
  |            |
  v            v
[2] language   [1] sing::midi_parser
  + syllabify      |
  |                v
  v             MidiTrack (notes, tempo)
syllable clips     |
  |                |
  v                v
[3-4] normalize    [5] sing::vocal_mapper
  |                |
  v                v
normalized clips   NoteMapping list
  |                |
  +--------+-------+
           |
           v
   [6-7] render each note
     (pitch shift + time stretch + vibrato/chorus)
           |
           v
   [8] assemble vocal timeline
           |
           v
       <run-name>-acappella.wav
           |           [9] sing::synthesize
           |                      |
           v                      v
        [10] sing::mixer ----> <run-name>.wav
```

## Speak pipeline

The speak pipeline reconstructs target text using syllable fragments from source audio. It builds a phonetically indexed bank of source syllables, converts target text to ARPABET, matches each target syllable to the closest source syllable by articulatory feature distance, and assembles the matched clips into output audio.

| Step | Operation | Module |
|------|-----------|--------|
| 1 | **Build source syllable bank** -- transcribe and syllabify all source audio files, then index each syllable with its ARPABET phonemes, timing, stress level, and source file path | `speak::syllable_bank` (via `language::*`) |
| 2 | **Convert target text to ARPABET syllables** -- CMU dict converts target words to phoneme sequences, ARPABET syllabifier splits into syllables | `speak::target_text` |
| 3 | **Match target syllables to source bank** -- compute articulatory feature distance between each target syllable and every bank entry, select the closest match with stress-level tie-breaking | `speak::matcher` (via `speak::phonetic_distance`) |
| 4 | **Plan timing** -- in text mode, space syllables uniformly with word-boundary pauses; in reference mode, blend source duration with reference timing based on strictness parameter | `speak::assembler` |
| 5 | **Assemble audio** -- cut each matched syllable from the source WAV, time-stretch if needed to match planned duration, optionally pitch-shift, then concatenate all clips with crossfade | `speak::assembler` |
| 6 | **Write output files** -- `<run-name>.wav` (assembled audio), `match-log.json` (per-syllable match details with distances), `syllable-bank.json` (full source bank index) | CLI runner |

### Data flow

```
input.mp4 [+ --text or --reference]
  |
  v
[1] transcribe + syllabify source -----> Syllable objects
  |
  v
[1] speak::syllable_bank -----> SyllableEntry list (phonemes, timing, stress)
  |                            |
  |                            v
  |                   syllable-bank.json
  |
  v
[2] speak::target_text -----> target TextSyllable list
  |
  v
[3] speak::matcher -----> MatchResult list (target -> source, distance)
  |                            |
  |                            v
  |                   match-log.json
  |
  v
[4] speak::assembler -----> TimingPlan list (start, duration, stretch)
  |
  v
[5] speak::assembler -----> cut + stretch + concatenate
  |
  v
<run-name>.wav
```

## Module map

### Core library (`glottisdale-core`)

| Module | Purpose |
|--------|---------|
| `types` | Core data types: `Phoneme`, `Syllable`, `Clip`, `PipelineResult`, `AlignmentResult` |
| `audio::io` | WAV read/write, ffmpeg extraction, resampling |
| `audio::analysis` | RMS energy, windowed RMS, autocorrelation F0 estimation, room tone detection, breath detection, pink noise generation |
| `audio::effects` | Pitch shift, time stretch, volume adjust, concatenation, crossfade, mixing, silence generation |
| `audio::playback` | Real-time audio playback via rodio |
| `language::g2p` | Grapheme-to-phoneme conversion via embedded CMU Pronouncing Dictionary |
| `language::syllabify` | Main syllabification orchestrator |
| `language::syllabify_arpabet` | ARPABET syllabifier implementing Maximum Onset Principle (vendored from kylebgorman/syllabify) |
| `language::syllabify_ipa` | IPA sonority-based syllabifier for BFA output |
| `language::phonotactics` | Phonotactic junction scoring (sonority contour, illegal onsets, hiatus) and syllable reordering |
| `language::transcribe` | Whisper ASR integration with word-level timestamps |
| `language::align` | Abstract `Aligner` trait, `DefaultAligner` (Whisper + g2p), `BfaAligner` (forced alignment), auto-detection |
| `cache` | SHA-256 file hashing, atomic writes, tiered cache for extraction/transcription/alignment |
| `names` | Thematic run name generator (speech/music-themed adjective-noun pairs) |
| `collage::process` | Main collage pipeline orchestration: sampling, grouping, normalization, prosodic dynamics, gap generation, final assembly |
| `collage::stretch` | Time-stretch selection logic (`StretchConfig`), stutter duplication, word repeat |
| `sing::midi_parser` | MIDI file parsing into `Note` and `MidiTrack` data structures |
| `sing::syllable_prep` | Syllable preparation: transcribe, syllabify, cut, pitch-normalize, volume-normalize |
| `sing::vocal_mapper` | Note-to-syllable mapping, per-note rendering with rubberband pitch/time, vibrato, and chorus effects |
| `sing::synthesize` | MIDI sine-wave synthesizer with noise-burst drum synthesis |
| `sing::mixer` | Vocal + backing track mixing via ffmpeg `amix` filter |
| `speak::phonetic_distance` | ARPABET articulatory feature matrix and phoneme/syllable distance calculations |
| `speak::syllable_bank` | `SyllableEntry` data structure and `build_bank()` for indexing source syllables |
| `speak::target_text` | CMU dict-based target text to ARPABET syllable conversion with word boundary tracking |
| `speak::matcher` | Syllable and phoneme matching against the source bank using phonetic distance with stress tie-breaking |
| `speak::assembler` | Timing planner (text mode and reference mode) and audio assembler (cut, stretch, pitch-shift, concatenate) |

### CLI (`glottisdale`)

Single-file binary (`main.rs`) using clap derive macros. Defines `CollageArgs`, `SingArgs`, and `SpeakArgs` structs, validates input, calls core library functions, and handles output formatting.

### GUI (`glottisdale-gui`)

Native desktop app (`main.rs` + `app.rs`) using egui/eframe. Tab-based interface with file picker, settings panels, and live log viewer. Spawns the CLI as a subprocess for processing.
