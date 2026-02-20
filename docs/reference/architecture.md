# Architecture

Glottisdale has three pipelines: **collage** (syllable-level audio collage), **sing** (vocal MIDI mapping), and **speak** (phonetic speech reconstruction). All three share a common foundation of audio extraction, Whisper transcription, and g2p syllabification, then diverge into their respective assembly strategies.

## Collage pipeline

The collage pipeline takes speech audio, segments it into syllables, shuffles and regroups them into nonsense "words" and "phrases," normalizes their pitch and volume, and concatenates everything into a surreal audio collage.

| Step | Operation | Source |
|------|-----------|--------|
| 1 | **Extract audio** -- ffmpeg resamples input to 16 kHz mono WAV | `audio.py:extract_audio()` |
| 2 | **Transcribe** -- Whisper ASR produces word-level timestamps | `collage/transcribe.py:transcribe()` |
| 3 | **Phoneme conversion** -- g2p_en converts words to ARPABET phoneme sequences | `collage/syllabify.py:syllabify_words()` |
| 4 | **Syllabification** -- Maximum Onset Principle splits phonemes into syllables with proportional timing | `collage/syllabify.py:syllabify_word()` + `collage/syllabify_arpabet.py:syllabify()` |
| 5 | **Sample syllables** -- randomly select and shuffle syllables to fill the target duration; round-robin across sources when multiple inputs are provided | `collage/__init__.py:_sample_syllables()`, `_sample_syllables_multi_source()` |
| 6 | **Group into words, phrases, sentences** -- syllables are grouped into variable-length "words" (with phonotactic reordering), words into phrases, phrases into sentences | `collage/__init__.py:_group_into_words()`, `_group_into_phrases()`, `_group_into_sentences()` |
| 7 | **Cut syllable clips** -- extract each syllable from the source WAV with configurable padding and fade | `audio.py:cut_clip()` |
| 8 | **Pitch normalization** -- estimate F0 via autocorrelation for each clip, compute the median, shift outliers toward it using ffmpeg `asetrate`/`aresample` | `analysis.py:estimate_f0()` + `audio.py:pitch_shift_clip()` |
| 9 | **Volume normalization** -- compute RMS for each clip, shift to the median using ffmpeg `volume` filter | `analysis.py:compute_rms()` + `audio.py:adjust_volume()` |
| 10 | **Stutter, syllable stretch, word assembly** -- optionally duplicate syllables (stutter) and time-stretch individual syllables, then concatenate syllables within each word using crossfade | `collage/stretch.py:apply_stutter()`, `should_stretch_syllable()` + `audio.py:concatenate_clips()` |
| 11 | **Word stretch, word repeat** -- optionally time-stretch assembled word WAVs and/or duplicate words in the sequence | `collage/stretch.py:apply_word_repeat()` + `audio.py:time_stretch_clip()` |
| 12 | **Phrase assembly with crossfade** -- concatenate words into phrases using a wider crossfade | `audio.py:concatenate_clips()` |
| 13 | **Prosodic dynamics** -- apply onset boost (+1.12 dB over the first 20% of each phrase) and final softening (-3 dB from 70% onward) via ffmpeg `volume` filter | `collage/__init__.py` (inline ffmpeg call) |
| 14 | **Gap generation** -- create inter-phrase and inter-sentence gaps using extracted room tone (mixed over silence) or plain silence, optionally prepend detected breath clips | `analysis.py:find_room_tone()`, `analysis.py:find_breaths()` + `audio.py:mix_audio()` |
| 15 | **Final concatenation** -- concatenate all phrases and gaps into one continuous output | `audio.py:concatenate_clips()` |
| 16 | **Global speed adjustment** -- time-stretch the entire output using rubberband (pitch-preserving) | `audio.py:time_stretch_clip()` |
| 17 | **Pink noise bed mixing** -- generate pink noise via spectral shaping (1/sqrt(f) FFT filter), mix under the output at a configurable dB level | `analysis.py:generate_pink_noise()` + `audio.py:mix_audio()` |

### Data flow

```
input.mp4
  |
  v
[1] extract_audio -----> 16kHz mono WAV
  |
  v
[2] transcribe ---------> word timestamps
  |
  v
[3-4] syllabify --------> Syllable objects (phonemes, timing, word)
  |
  v
[5] sample --------------> selected syllables (shuffled)
  |
  v
[6] group ---------------> words -> phrases -> sentences
  |
  v
[7] cut_clip ------------> individual syllable WAVs
  |
  v
[8-9] normalize ---------> pitch- and volume-normalized WAVs
  |
  v
[10-11] stretch/stutter --> modified word WAVs
  |
  v
[12-13] phrase assembly --> phrase WAVs with crossfade + dynamics
  |
  v
[14-15] gaps + concat ---> continuous output WAV
  |
  v
[16-17] speed + noise ---> final output WAV
```

## Sing pipeline

The sing pipeline maps syllable clips onto MIDI melody notes to produce a "drunk choir" vocal track. It reuses the collage modules for transcription and syllabification, then applies rubberband-based pitch shifting and time stretching to fit syllables to the melody.

| Step | Operation | Source |
|------|-----------|--------|
| 1 | **Parse MIDI melody** -- pretty_midi extracts Note and MidiTrack dataclasses from the melody file | `sing/midi_parser.py:parse_midi()` |
| 2 | **Transcribe + syllabify** -- reuses Whisper transcription and g2p syllabification from the collage modules to produce syllable clips | `sing/syllable_prep.py:prepare_syllables()` |
| 3 | **Normalize syllable pitch** -- shift each syllable's F0 to the median using rubberband pitch filter | `sing/syllable_prep.py:_rubberband_pitch_shift()` |
| 4 | **Normalize syllable volume** -- adjust each clip's RMS to the median using ffmpeg volume filter | `sing/syllable_prep.py` (via `audio.py:adjust_volume()`) |
| 5 | **Plan note mapping** -- assign syllable indices to each melody note, choose pitch drift (Gaussian around 0), and flag vibrato/chorus per note based on duration | `sing/vocal_mapper.py:plan_note_mapping()` |
| 6 | **Render each note** -- compute the semitone shift from median F0 to the MIDI note frequency, then apply rubberband pitch shift + time stretch to fit the note duration | `sing/vocal_mapper.py:render_mapping()` |
| 7 | **Apply vibrato and chorus** -- vibrato via ffmpeg `vibrato` filter on sustained notes; chorus via detuned/delayed voice layering with `amix` | `sing/vocal_mapper.py:_apply_vibrato()`, `_apply_chorus()` |
| 8 | **Assemble vocal timeline** -- place rendered notes at their MIDI start times with silence gaps between them, crossfade adjacent clips | `sing/vocal_mapper.py:render_vocal_track()` |
| 9 | **Synthesize MIDI backing** -- render melody, chords, bass, and drums from MIDI files using sine waves (pretty_midi `synthesize`) and noise-burst drum synthesis | `sing/synthesize.py:synthesize_preview()` |
| 10 | **Mix vocal + backing** -- mix the a cappella vocal track over the synthesized backing at configurable dB levels (default: vocal at 0 dB, MIDI at -12 dB) | `sing/mixer.py:mix_tracks()` |

### Data flow

```
input.mp4 + melody.mid
  |            |
  v            v
[2] transcribe   [1] parse_midi
  + syllabify      |
  |                v
  v             MidiTrack (notes, tempo)
syllable clips     |
  |                |
  v                v
[3-4] normalize    [5] plan_note_mapping
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
       acappella.wav
           |           [9] synthesize MIDI backing
           |                      |
           v                      v
        [10] mix_tracks -----> full_mix.wav
```

## Speak pipeline

The speak pipeline reconstructs target text using syllable fragments from source audio. It builds a phonetically indexed bank of source syllables, converts target text to ARPABET, matches each target syllable to the closest source syllable by articulatory feature distance, and assembles the matched clips into output audio.

| Step | Operation | Source |
|------|-----------|--------|
| 1 | **Build source syllable bank** -- transcribe and syllabify all source audio files, then index each syllable with its ARPABET phonemes, timing, stress level, and source file path | `speak/syllable_bank.py:build_bank()` (via `collage/align.py:get_aligner()`) |
| 2 | **Convert target text to ARPABET syllables** -- g2p_en converts target words to phoneme sequences, ARPABET syllabifier splits into syllables | `speak/target_text.py:text_to_syllables()` |
| 3 | **Match target syllables to source bank** -- compute articulatory feature distance between each target syllable and every bank entry, select the closest match with stress-level tie-breaking | `speak/matcher.py:match_syllables()` (via `speak/phonetic_distance.py`) |
| 4 | **Plan timing** -- in text mode, space syllables uniformly with word-boundary pauses; in reference mode, blend source duration with reference timing based on strictness parameter | `speak/assembler.py:plan_timing()` |
| 5 | **Assemble audio** -- cut each matched syllable from the source WAV, time-stretch if needed to match planned duration, optionally pitch-shift, then concatenate all clips with crossfade | `speak/assembler.py:assemble()` |
| 6 | **Write output files** -- `speak.wav` (assembled audio), `match-log.json` (per-syllable match details with distances), `syllable-bank.json` (full source bank index) | `speak/__init__.py:process()` |

### Data flow

```
input.mp4 [+ --text or --reference]
  |
  v
[1] transcribe + syllabify source -----> Syllable objects
  |
  v
[1] build_bank -----> SyllableEntry list (phonemes, timing, stress)
  |                            |
  |                            v
  |                   syllable-bank.json
  |
  v
[2] text_to_syllables -----> target TextSyllable list
  |
  v
[3] match_syllables -----> MatchResult list (target -> source, distance)
  |                            |
  |                            v
  |                   match-log.json
  |
  v
[4] plan_timing -----> TimingPlan list (start, duration, stretch)
  |
  v
[5] assemble -----> cut + stretch + concatenate
  |
  v
speak.wav
```

## Module map

| Module | Purpose |
|--------|---------|
| `cli.py` | CLI argument parsing (`argparse`) and subcommand dispatch (`collage`, `sing`, `speak`) |
| `types.py` | Core dataclasses: `Phoneme`, `Syllable`, `Clip`, `Result` |
| `audio.py` | FFmpeg/ffprobe wrappers: extract, cut, concatenate, crossfade, pitch shift, time stretch, volume adjust, silence generation, mixing |
| `analysis.py` | WAV I/O (scipy), RMS energy, windowed RMS, autocorrelation F0 estimation, room tone detection, breath detection, pink noise generation |
| `cache.py` | File-based caching (SHA-256 keyed) for audio extraction, Whisper transcription, and alignment results |
| `collage/__init__.py` | Main collage pipeline orchestration: sampling, grouping, normalization, prosodic dynamics, gap generation, final assembly |
| `collage/transcribe.py` | Whisper ASR integration with word-level timestamps and model caching |
| `collage/align.py` | Abstract `Aligner` interface, `DefaultAligner` (Whisper + g2p), auto-detection of BFA availability |
| `collage/bfa.py` | Bournemouth Forced Aligner backend: chunked phoneme-level alignment with IPA output and pg16 group classification |
| `collage/syllabify.py` | g2p_en phoneme conversion + ARPABET syllabification with proportional timestamp distribution |
| `collage/syllabify_arpabet.py` | Vendored ARPABET syllabifier (kylebgorman) implementing Maximum Onset Principle |
| `collage/syllabify_ipa.py` | IPA sonority-based syllabifier for BFA output, using pg16 group sonority mapping |
| `collage/phonotactics.py` | Phonotactic junction scoring (sonority contour, illegal onsets, hiatus) and syllable reordering |
| `collage/stretch.py` | Time-stretch selection logic (`StretchConfig`), stutter duplication, and word repeat |
| `sing/midi_parser.py` | MIDI file parsing via pretty_midi into `Note` and `MidiTrack` dataclasses |
| `sing/syllable_prep.py` | Syllable preparation: transcribe, syllabify, cut, pitch-normalize (rubberband), volume-normalize |
| `sing/vocal_mapper.py` | Note-to-syllable mapping (`plan_note_mapping`), per-note rendering with rubberband pitch/time, vibrato, and chorus effects |
| `sing/synthesize.py` | MIDI sine-wave synthesizer with noise-burst drum synthesis |
| `sing/mixer.py` | Vocal + backing track mixing via ffmpeg `amix` filter |
| `speak/__init__.py` | Speak pipeline orchestration: build bank, convert target text, match, plan timing, assemble, write outputs |
| `speak/phonetic_distance.py` | ARPABET articulatory feature matrix and phoneme/syllable distance calculations |
| `speak/syllable_bank.py` | `SyllableEntry` dataclass and `build_bank()` for indexing source syllables with phonemes, timing, and stress |
| `speak/target_text.py` | g2p_en-based target text to ARPABET syllable conversion with word boundary tracking |
| `speak/matcher.py` | Syllable and phoneme matching against the source bank using phonetic distance with stress tie-breaking |
| `speak/assembler.py` | Timing planner (text mode and reference mode) and audio assembler (cut, stretch, pitch-shift, concatenate) |
