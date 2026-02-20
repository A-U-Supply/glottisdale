# Python API Reference

## Core entry points

### `glottisdale.collage.process()`

Run the full collage pipeline: transcribe, syllabify, cut, arrange, polish, and concatenate.

```python
from glottisdale.collage import process

result = process(input_paths, **kwargs)
```

**Core**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `input_paths` | `list[Path]` | *(required)* | Audio or video files to process |
| `output_dir` | `str \| Path` | `"./glottisdale-output"` | Directory for output files |
| `syllables_per_clip` | `str` | `"1-5"` | Range of syllables grouped into each "word" clip |
| `target_duration` | `float` | `10.0` | Approximate output duration in seconds |
| `crossfade_ms` | `float` | `30` | Crossfade between syllables within a word (ms) |
| `padding_ms` | `float` | `25` | Padding added around each syllable cut (ms) |
| `gap` | `str \| None` | `None` | Legacy alias for `phrase_pause`; also auto-derives `sentence_pause` |
| `aligner` | `str` | `"auto"` | Alignment backend: `"auto"`, `"default"`, or `"bfa"` |
| `whisper_model` | `str` | `"base"` | Whisper model size for transcription |
| `bfa_device` | `str` | `"cpu"` | Device for BFA aligner (`"cpu"` or `"cuda"`) |
| `seed` | `int \| None` | `None` | Random seed for reproducible output |

**Prosodic grouping**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `words_per_phrase` | `str` | `"3-5"` | Range of words grouped into each phrase |
| `phrases_per_sentence` | `str` | `"2-3"` | Range of phrases grouped into each sentence |
| `phrase_pause` | `str` | `"400-700"` | Silence between phrases in ms |
| `sentence_pause` | `str` | `"800-1200"` | Silence between sentences in ms |
| `word_crossfade_ms` | `float` | `50` | Crossfade between words within a phrase (ms) |

**Audio polish**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `noise_level_db` | `float` | `-40` | Pink noise bed level in dB (0 disables) |
| `room_tone` | `bool` | `True` | Extract and layer room tone in pauses |
| `pitch_normalize` | `bool` | `True` | Normalize syllable pitches to median F0 |
| `pitch_range` | `float` | `5` | Maximum pitch shift in semitones during normalization |
| `breaths` | `bool` | `True` | Detect and insert breath sounds at phrase boundaries |
| `breath_probability` | `float` | `0.6` | Probability of inserting a breath at each phrase gap |
| `volume_normalize` | `bool` | `True` | Normalize RMS volume across syllable clips |
| `prosodic_dynamics` | `bool` | `True` | Apply phrase-level volume contour (slight boost at start, fade at end) |

**Time stretch**

All off by default. Enable one or more modes by setting a non-`None` value.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `speed` | `float \| None` | `None` | Global speed multiplier applied to final output (0.5 = half speed) |
| `random_stretch` | `float \| None` | `None` | Probability each syllable is time-stretched |
| `alternating_stretch` | `int \| None` | `None` | Stretch every Nth syllable |
| `boundary_stretch` | `int \| None` | `None` | Stretch the last N syllables in each word |
| `word_stretch` | `float \| None` | `None` | Probability each assembled word clip is time-stretched |
| `stretch_factor` | `str` | `"2.0"` | Stretch amount: fixed (`"2.0"`) or range (`"1.5-2.5"`) |

**Word repeat**

All off by default. Set `repeat_weight` to enable.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `repeat_weight` | `float \| None` | `None` | Probability each word is repeated |
| `repeat_count` | `str` | `"1-2"` | Range of extra repetitions per word |
| `repeat_style` | `str` | `"exact"` | Repetition style: `"exact"` duplicates the clip as-is |

**Stutter**

All off by default. Set `stutter` to enable.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `stutter` | `float \| None` | `None` | Probability each syllable is stuttered (duplicated within its word) |
| `stutter_count` | `str` | `"1-2"` | Range of extra copies per stuttered syllable |

**Misc**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `verbose` | `bool` | `False` | Enable verbose logging (show dependency warnings, alignment details) |
| `use_cache` | `bool` | `True` | Cache extracted audio and alignment results on disk |

**Returns:** [`Result`](#result)

---

### Sing workflow

The sing pipeline has no single `process()` function. Use these steps in order:

#### `parse_midi(path) -> MidiTrack`

*Module:* `glottisdale.sing.midi_parser`

Parse a MIDI file into a structured track. Merges all non-drum instrument tracks and sorts notes by start time.

```python
from glottisdale.sing.midi_parser import parse_midi
track = parse_midi(Path("melody.mid"))
```

#### `prepare_syllables(input_paths, work_dir, whisper_model, max_semitone_shift, use_cache) -> list[NormalizedSyllable]`

*Module:* `glottisdale.sing.syllable_prep`

Transcribe, syllabify, cut, pitch-normalize, and volume-normalize syllable clips from audio/video sources.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `input_paths` | `list[Path]` | *(required)* | Audio or video source files |
| `work_dir` | `Path` | *(required)* | Working directory for intermediate files |
| `whisper_model` | `str` | `"base"` | Whisper model size |
| `max_semitone_shift` | `float` | `5.0` | Maximum pitch normalization shift |
| `use_cache` | `bool` | `True` | Use file-based caching for extraction and transcription |

#### `plan_note_mapping(notes, pool_size, seed, drift_range, chorus_probability) -> list[NoteMapping]`

*Module:* `glottisdale.sing.vocal_mapper`

Plan how each melody note maps to syllable(s). Assigns syllable indices, pitch drift, vibrato, and chorus flags.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `notes` | `list[Note]` | *(required)* | Note objects from `parse_midi` |
| `pool_size` | `int` | *(required)* | Number of available syllables |
| `seed` | `int \| None` | `None` | Random seed |
| `drift_range` | `float` | `2.0` | Max semitones of pitch drift from melody |
| `chorus_probability` | `float` | `0.3` | Probability of chorus effect on non-sustained notes |

#### `render_vocal_track(mappings, syllable_clips, work_dir, median_f0, target_duration) -> Path`

*Module:* `glottisdale.sing.vocal_mapper`

Render all note mappings into a complete a cappella vocal track. Applies pitch shifting, time stretching, vibrato, and chorus per mapping, then assembles with gaps and crossfades.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `mappings` | `list[NoteMapping]` | *(required)* | Output of `plan_note_mapping` |
| `syllable_clips` | `list` | *(required)* | List of `NormalizedSyllable` from `prepare_syllables` |
| `work_dir` | `Path` | *(required)* | Working directory for rendered note files |
| `median_f0` | `float` | *(required)* | Median F0 of syllable pool in Hz |
| `target_duration` | `float` | `40.0` | Target track duration in seconds |

**Returns:** `Path` to the rendered `acappella.wav`.

#### `mix_tracks(vocal_path, midi_dir, output_dir) -> tuple[Path, Path]`

*Module:* `glottisdale.sing.mixer`

Produce two output files: a cappella and full mix (vocal over synthesized MIDI backing).

| Parameter | Type | Default | Description |
|---|---|---|---|
| `vocal_path` | `Path` | *(required)* | Path to rendered vocal track |
| `midi_dir` | `Path` | *(required)* | Directory containing MIDI files to synthesize |
| `output_dir` | `Path` | *(required)* | Output directory |

**Returns:** `(full_mix_path, acappella_path)` -- both as `Path`.

---

## Data types

All dataclasses are in `glottisdale.types` (except sing-specific types noted below).

### `Phoneme`

```python
from glottisdale.types import Phoneme
```

| Field | Type | Description |
|---|---|---|
| `label` | `str` | ARPABET symbol (e.g. `"AH0"`) or IPA if using BFA |
| `start` | `float` | Start time in seconds |
| `end` | `float` | End time in seconds |

### `Syllable`

```python
from glottisdale.types import Syllable
```

| Field | Type | Description |
|---|---|---|
| `phonemes` | `list[Phoneme]` | Constituent phonemes |
| `start` | `float` | First phoneme start (seconds) |
| `end` | `float` | Last phoneme end (seconds) |
| `word` | `str` | Parent word from transcript |
| `word_index` | `int` | Word position in transcript |

### `Clip`

```python
from glottisdale.types import Clip
```

| Field | Type | Description |
|---|---|---|
| `syllables` | `list[Syllable]` | Syllables in this clip |
| `start` | `float` | Start time with padding applied (seconds) |
| `end` | `float` | End time with padding applied (seconds) |
| `source` | `str` | Input filename this clip came from |
| `output_path` | `Path` | Path to the rendered WAV file |

### `Result`

```python
from glottisdale.types import Result
```

| Field | Type | Description |
|---|---|---|
| `clips` | `list[Clip]` | All generated word clips |
| `concatenated` | `Path` | Path to the final concatenated WAV |
| `transcript` | `str` | Source transcripts, one per input file |
| `manifest` | `dict` | Metadata: sources, syllable counts, clip details |

### `Note` (sing)

```python
from glottisdale.sing.midi_parser import Note
```

| Field | Type | Description |
|---|---|---|
| `pitch` | `int` | MIDI note number |
| `start` | `float` | Start time in seconds |
| `end` | `float` | End time in seconds |
| `velocity` | `int` | MIDI velocity (0--127) |

*Property:* `duration -> float` -- computed as `end - start`.

### `MidiTrack` (sing)

```python
from glottisdale.sing.midi_parser import MidiTrack
```

| Field | Type | Description |
|---|---|---|
| `notes` | `list[Note]` | Sorted notes from all non-drum instruments |
| `tempo` | `float` | Estimated BPM |
| `program` | `int` | MIDI program number of first instrument |
| `is_drum` | `bool` | Whether the first instrument is a drum track |
| `total_duration` | `float` | Total MIDI file duration in seconds |

### `NoteMapping` (sing)

```python
from glottisdale.sing.vocal_mapper import NoteMapping
```

| Field | Type | Description |
|---|---|---|
| `note_pitch` | `int` | Target MIDI note number |
| `note_start` | `float` | Note start time in seconds |
| `note_end` | `float` | Note end time in seconds |
| `note_duration` | `float` | Note duration in seconds |
| `syllable_indices` | `list[int]` | Indices into the syllable pool |
| `pitch_shift_semitones` | `float` | Drift from exact pitch (semitones) |
| `time_stretch_ratio` | `float` | Time stretch ratio (placeholder; computed at render) |
| `apply_vibrato` | `bool` | Whether to apply vibrato |
| `apply_chorus` | `bool` | Whether to apply chorus effect |
| `duration_class` | `str` | `"short"`, `"medium"`, or `"long"` |

### `NormalizedSyllable` (sing)

```python
from glottisdale.sing.syllable_prep import NormalizedSyllable
```

| Field | Type | Description |
|---|---|---|
| `clip_path` | `Path` | Path to the normalized WAV clip |
| `f0` | `float \| None` | Estimated fundamental frequency in Hz (None if unvoiced) |
| `duration` | `float` | Clip duration in seconds |
| `phonemes` | `list[str]` | ARPABET phoneme labels |
| `word` | `str` | Parent word from transcript |

---

## Aligner interface

```python
from glottisdale.collage.align import get_aligner
```

`get_aligner(name, **kwargs) -> Aligner`

| `name` | Backend | Description |
|---|---|---|
| `"auto"` | BFA or default | Tries BFA first; falls back to default if `bournemouth-forced-aligner` or `espeak-ng` is missing |
| `"default"` | `DefaultAligner` | Whisper word timestamps + `g2p_en` phoneme conversion + proportional syllable timing |
| `"bfa"` | `BFAAligner` | Whisper transcription + Bournemouth Forced Aligner for real phoneme-level timestamps |

All aligners implement `Aligner.process(audio_path, audio_hash=None, use_cache=False) -> dict` returning:

```python
{
    "text": str,            # full transcript
    "words": list[dict],    # word dicts with timestamps
    "syllables": list[Syllable],
}
```

Keyword arguments passed to `get_aligner` are forwarded to the backend constructor. Relevant kwargs:

- `whisper_model` (`str`) -- Whisper model size (both backends)
- `device` (`str`) -- Torch device for BFA (`"cpu"` or `"cuda"`)
- `verbose` (`bool`) -- Enable detailed logging

---

## Programmatic examples

### Basic collage

```python
from pathlib import Path
from glottisdale.collage import process

result = process(
    input_paths=[Path("speech.mp4")],
    target_duration=30,
    seed=42,
)
print(f"Output: {result.concatenated}")
print(f"Transcript: {result.transcript}")
print(f"Clips: {len(result.clips)}")
```

### Custom settings

```python
from pathlib import Path
from glottisdale.collage import process

result = process(
    input_paths=[Path("video1.mp4"), Path("video2.mp4")],
    target_duration=60,
    syllables_per_clip="2-3",
    words_per_phrase="4-6",
    noise_level_db=-35,
    random_stretch=0.3,
    stretch_factor="1.5-2.5",
    seed=123,
)
```

### Sing workflow

```python
from pathlib import Path
from statistics import median
from glottisdale.sing.midi_parser import parse_midi
from glottisdale.sing.syllable_prep import prepare_syllables
from glottisdale.sing.vocal_mapper import plan_note_mapping, render_vocal_track
from glottisdale.sing.mixer import mix_tracks

# 1. Parse MIDI melody
track = parse_midi(Path("midi/melody.mid"))

# 2. Prepare normalized syllable clips from speech
syllables = prepare_syllables([Path("speech.mp4")], Path("work/"), "base")

# 3. Compute median pitch of voiced syllables
voiced_f0 = [s.f0 for s in syllables if s.f0 and s.f0 > 0]
median_f0 = median(voiced_f0) if voiced_f0 else 220.0

# 4. Map melody notes to syllables
mappings = plan_note_mapping(track.notes, len(syllables), seed=42, drift_range=2.0)

# 5. Render vocal track
acappella = render_vocal_track(mappings, syllables, Path("work/"), median_f0, 30.0)

# 6. Mix with MIDI backing
full_mix, acappella_out = mix_tracks(acappella, Path("midi/"), Path("output/"))
```
