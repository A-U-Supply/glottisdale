# Examples

CLI recipes for getting interesting results out of glottisdale, organized by what you want to achieve rather than by flag name. Every example uses the `collage` subcommand unless noted otherwise.

All examples assume you have at least one audio or video file with speech in it. The file can be any format ffmpeg understands (mp4, wav, mp3, m4a, etc.).

---

## Basic variations

### Make a longer collage

By default, glottisdale aims for about 30 seconds of output. To get a full minute:

```bash
glottisdale collage recording.mp4 --target-duration 60
```

`--target-duration` controls how many seconds of syllable material get sampled before assembly. The final output may be slightly longer or shorter depending on pauses and crossfades.

### Get the same result twice

Glottisdale shuffles syllables randomly, so every run produces different output. If you find something you like and want to recreate it exactly:

```bash
glottisdale collage recording.mp4 --seed 42
```

`--seed` locks the random number generator to a fixed starting point. Same seed + same input + same flags = identical output every time. The number itself does not matter -- pick any integer you like.

### Mix multiple speakers or sources

You can pass more than one input file. Glottisdale round-robin samples syllables from each source, so the output weaves between them:

```bash
glottisdale collage speaker_a.mp4 speaker_b.wav podcast.mp3 --target-duration 45
```

The collage will contain syllables drawn from all three files in roughly equal proportion, shuffled together. This is a good way to blend voices, accents, or tonal qualities.

### Use a better transcription model

Glottisdale uses OpenAI's Whisper to find word boundaries in your audio. The default model (`base`) is fast but can miss words or get timing wrong in noisy recordings. If your output has oddly-cut syllables or you have a longer file worth the wait:

```bash
glottisdale collage recording.mp4 --whisper-model small
```

Available models from fastest/smallest to slowest/most accurate: `tiny`, `base`, `small`, `medium`. The first run downloads the model (a few hundred MB for `small`); subsequent runs use the cached version. Transcription results are also cached per file, so re-running with different collage settings is fast.

---

## Shaping the rhythm

These flags control how syllables are grouped into "words" and "phrases" -- the prosodic structure that gives the collage its rhythmic feel.

### Short, choppy words

```bash
glottisdale collage recording.mp4 \
  --syllables-per-word 1-2 \
  --words-per-phrase 5-7
```

`--syllables-per-word 1-2` means each assembled "word" is only 1 or 2 syllables long, producing short, staccato bursts. `--words-per-phrase 5-7` packs more of these short words into each phrase before a pause. The result is rapid, clipped speech fragments with brief pauses between clusters.

### Long, flowing words

```bash
glottisdale collage recording.mp4 \
  --syllables-per-word 3-5 \
  --words-per-phrase 2-3
```

`--syllables-per-word 3-5` fuses more syllables into each word, creating longer continuous sounds. `--words-per-phrase 2-3` keeps phrase groups small so each long word gets breathing room. The output has a more languid, rolling quality -- fewer cuts, more sustained vocal sound.

### Rapid-fire delivery

```bash
glottisdale collage recording.mp4 \
  --phrase-pause 100-200 \
  --sentence-pause 300-500
```

`--phrase-pause` and `--sentence-pause` control the silence between groups (in milliseconds). The defaults are 400-700ms and 800-1200ms respectively. Cutting them to 100-200ms and 300-500ms compresses the gaps, making the collage feel rushed and breathless -- syllable fragments tumbling over each other with barely a gap.

### Slow and deliberate

```bash
glottisdale collage recording.mp4 \
  --phrase-pause 800-1200 \
  --sentence-pause 1500-2500
```

Longer pauses between phrases and sentences. Each cluster of syllables is followed by a noticeable silence before the next one begins. The output feels measured and contemplative, with enough space between phrases that each one registers as a distinct statement.

### Tight crossfades vs hard cuts

Crossfades blend the tail of one syllable into the start of the next. They smooth transitions but also smear timbral detail.

Tight, overlapping syllables (more blending within words):

```bash
glottisdale collage recording.mp4 --crossfade 60
```

Hard cuts with no overlap (each syllable starts cleanly after the last one ends):

```bash
glottisdale collage recording.mp4 --crossfade 0
```

The default is 30ms, which is a compromise. At `--crossfade 0`, you hear distinct clicks and edges between syllables. At `--crossfade 60`, syllable boundaries blur together, creating a more fluid but less articulated sound. The `--word-crossfade` flag (default 50ms) does the same thing at word boundaries within a phrase.

---

## Adding texture

These flags control the audio polish layer -- room tone, breaths, pitch normalization, and noise. All are on by default. Turning them off strips the output back to raw cut-and-paste syllables; turning them up adds atmosphere.

### Stripped down and dry

```bash
glottisdale collage recording.mp4 \
  --no-room-tone \
  --no-breaths \
  --no-prosodic-dynamics \
  --noise-level 0
```

This disables room tone in the gaps (you get digital silence instead), removes breath sounds at phrase boundaries, turns off the phrase-level volume envelope (onset boost and phrase-final softening), and kills the pink noise bed. The output is stark and clinical -- pure syllable fragments separated by dead silence.

### Heavy atmosphere

```bash
glottisdale collage recording.mp4 \
  --noise-level -30 \
  --breath-probability 0.9
```

`--noise-level -30` raises the pink noise bed 10dB above the default (-40dB), making it noticeably audible as a continuous hiss underneath the speech. `--breath-probability 0.9` inserts a breath sound at 90% of phrase boundaries (default is 60%). The result has a persistent background texture and frequent audible breathing between phrases, giving it a more bodily, lived-in feel.

### Pitch variety vs uniformity

By default, glottisdale normalizes all syllable pitches toward a common median frequency so the output sounds like one speaker at a consistent pitch. You can disable this or tighten it:

Let pitches stay as they were in the original recording (more variation between syllables):

```bash
glottisdale collage recording.mp4 --no-pitch-normalize
```

With pitch normalization off, syllables from different parts of the recording (or different sources) keep their original pitch. You hear more tonal variety -- some syllables higher, some lower -- which sounds less like coherent speech and more like a patchwork.

Restrict pitch shifts to a narrow range (more uniform, but with a cap on how far any syllable gets shifted):

```bash
glottisdale collage recording.mp4 --pitch-range 2
```

The default `--pitch-range` is 5 semitones. Setting it to 2 means no syllable gets shifted more than 2 semitones from the median. Syllables that would need a bigger correction are left closer to their original pitch, so you get mild normalization without aggressive pitch-bending artifacts.

---

## Stretching and warping

Time-stretch effects slow down individual syllables or the entire output using pitch-preserving stretching (via rubberband). All stretch modes are off by default.

### Random stretch (dream-like)

```bash
glottisdale collage recording.mp4 \
  --random-stretch 0.3 \
  --stretch-factor 1.5-3.0
```

`--random-stretch 0.3` gives each syllable a 30% chance of being time-stretched. `--stretch-factor 1.5-3.0` means stretched syllables become 1.5x to 3x their original length, with a random factor per syllable. Unstretched syllables play at normal speed. The output has an uneven, drifting quality -- some syllables linger unexpectedly while others pass at normal pace, like speech heard through waves of distortion.

### Alternating stretch (rhythmic)

```bash
glottisdale collage recording.mp4 \
  --alternating-stretch 3 \
  --stretch-factor 2.0
```

`--alternating-stretch 3` stretches every 3rd syllable (the 1st, 4th, 7th, etc.). `--stretch-factor 2.0` doubles their length. The pattern is regular and predictable, creating a rhythmic pulse: normal, normal, stretched, normal, normal, stretched. This produces a metered, almost musical cadence.

### Boundary emphasis

```bash
glottisdale collage recording.mp4 \
  --boundary-stretch 1 \
  --stretch-factor 1.5-2.5
```

`--boundary-stretch 1` stretches the first and last syllable of every assembled word. These boundary syllables become 1.5x to 2.5x longer, while interior syllables stay at normal speed. The effect emphasizes word edges -- each word begins and ends with a drawn-out syllable, framing the faster syllables in the middle. With longer words (`--syllables-per-word 3-5`), this creates a noticeable elastic contour per word.

### Word stretch (thick and slurred)

```bash
glottisdale collage recording.mp4 \
  --word-stretch 0.5 \
  --stretch-factor 1.5-2.0
```

`--word-stretch 0.5` gives each assembled word (after all its syllables are fused together) a 50% chance of being stretched as a whole unit. Unlike the syllable-level stretch modes, this stretches the entire word WAV after assembly, so crossfades and internal timing are preserved but everything plays slower. The result sounds thick and dragged -- like a recording playing back from a slowing tape machine.

### Global speed (everything faster or slower)

Slow and deep:

```bash
glottisdale collage recording.mp4 --speed 0.7
```

`--speed 0.7` plays the entire final output at 70% speed (pitch is preserved). Everything -- syllables, pauses, breaths -- stretches proportionally. The output is about 43% longer than it would be at normal speed. Speech sounds low-energy and elongated.

Fast and frantic:

```bash
glottisdale collage recording.mp4 --speed 1.5
```

`--speed 1.5` plays everything at 150% speed. The output is about 33% shorter. Syllables and pauses compress, and the overall feel is hurried and jittery.

Note: `--speed` applies to the final concatenated output as a post-process. The other stretch modes (`--random-stretch`, `--alternating-stretch`, etc.) apply to individual syllables or words during assembly. You can use `--speed` alongside the other modes, but the interaction compounds -- a syllable that was already stretched 2x and then played at 0.5x speed ends up 4x its original length.

---

## Repetition and stutter

These effects duplicate words or syllables to create echoic, looping, or stammering textures. Both are off by default.

### Subtle repetition

```bash
glottisdale collage recording.mp4 \
  --repeat-weight 0.2 \
  --repeat-count 1
```

`--repeat-weight 0.2` gives each word a 20% chance of being repeated. `--repeat-count 1` means it repeats once (you hear the word twice total). With the default `--repeat-style exact`, the same word WAV plays again immediately. At this low probability, most words play once, with occasional doubles scattered throughout -- a mild echo effect that adds rhythmic interest without overwhelming the structure.

### Heavy repetition

```bash
glottisdale collage recording.mp4 \
  --repeat-weight 0.6 \
  --repeat-count 2-4 \
  --repeat-style resample
```

`--repeat-weight 0.6` repeats 60% of words. `--repeat-count 2-4` adds 2 to 4 extra copies per repeated word (you hear it 3 to 5 times total). `--repeat-style resample` means each repeated copy is rebuilt from fresh syllables rather than duplicating the same WAV, so repetitions have the same syllable count and structure but different source material. The output is dense with echoed phrases, each repetition slightly different in timbre -- like a chorus of speakers trying to say the same thing.

### Stutter effect

```bash
glottisdale collage recording.mp4 \
  --stutter 0.3 \
  --stutter-count 2-3
```

`--stutter 0.3` gives each individual syllable a 30% chance of being repeated in place before the word is assembled. `--stutter-count 2-3` adds 2 to 3 extra copies of the stuttered syllable. Unlike word repeat, stutter operates at the syllable level, so you get rapid-fire repetitions of single syllable fragments within words: "ba-ba-ba-nana" rather than "banana banana banana." The duplicates are joined with the normal intra-word crossfade, so they blend into a stuttering, tripping rhythm.

### Combining repetition and stutter

```bash
glottisdale collage recording.mp4 \
  --repeat-weight 0.3 \
  --stutter 0.2
```

Stutter and word repeat are independent effects that stack. A word might have some of its syllables stuttered (creating internal repetition) and then the entire assembled word might also be repeated (creating whole-word echoes). At moderate probabilities like these, you get a mix of both textures -- some words stumble internally, some words echo as a whole, and occasionally both happen to the same word.

---

## Vocal MIDI recipes (sing mode)

The `sing` subcommand maps syllable clips onto MIDI melody notes. You need a directory containing a `melody.mid` file. These examples assume you have that set up.

### Tight melody following

```bash
glottisdale sing recording.mp4 --midi midi/ --drift-range 0.5
```

`--drift-range 0.5` limits how far (in semitones) each syllable's pitch can drift from the target MIDI note. With only half a semitone of drift, syllables track the melody closely. The output follows the written melody with minimal wandering -- tighter intonation, more recognizably "in tune" (within the limits of the source material).

### Loose and expressive

```bash
glottisdale sing recording.mp4 --midi midi/ --drift-range 4.0
```

`--drift-range 4.0` allows up to 4 semitones of random drift from each target note. Syllables approximate the melody but wander significantly, producing a wobbly, expressive quality -- like a singer who knows the general shape of the melody but can not quite hit the notes.

### Clean vocal (no effects)

```bash
glottisdale sing recording.mp4 --midi midi/ \
  --no-vibrato \
  --no-chorus \
  --drift-range 0
```

Disabling vibrato, chorus, and drift gives you the most direct mapping: each syllable is pitched to the exact MIDI note with no modulation or doubling. The output is dry and precise -- you hear the raw pitch-shifted syllables locked to the melody grid.

### Full effect (the defaults)

```bash
glottisdale sing recording.mp4 --midi midi/ \
  --vibrato \
  --chorus \
  --drift-range 2.0
```

This is what you get if you specify no sing-specific flags. Vibrato adds periodic pitch wobble to sustained notes. Chorus adds a slightly detuned and delayed copy for thickness. Drift of 2 semitones lets pitches wander naturally. The combination produces the "drunk choir" sound: recognizably melodic but loose and layered.

---

## Combining everything

These examples layer multiple effect categories together for more extreme results.

### "Haunted Answering Machine"

A slow, fragmented, breathing collage with long pauses and heavy stuttering. Sounds like a corrupted voicemail playback.

```bash
glottisdale collage recording.mp4 \
  --target-duration 45 \
  --speed 0.7 \
  --random-stretch 0.4 --stretch-factor 2.0-4.0 \
  --stutter 0.3 --stutter-count 3-5 \
  --syllables-per-word 1-2 \
  --phrase-pause 600-1200 \
  --noise-level -30 \
  --breath-probability 0.9 \
  --seed 666
```

What's happening: Short 1-2 syllable words (`--syllables-per-word 1-2`) are assembled, then 40% of syllables get time-stretched to 2-4x their length, and 30% of syllables stutter 3-5 times. The whole thing plays back at 70% speed on top of that. Long pauses (600-1200ms) separate each phrase. A raised pink noise bed (-30dB) and near-constant breath sounds (90% probability) fill the gaps. The output is slow, halting, and thick with hiss and breathing -- fragmented speech that drags and trips over itself.

### "Glossolalia Radio"

Dense, rapid, heavily repeated speech over a noisy channel. Sounds like an overheard shortwave broadcast in an unknown language.

```bash
glottisdale collage recording.mp4 \
  --target-duration 60 \
  --speed 1.3 \
  --syllables-per-word 3-5 \
  --words-per-phrase 4-6 \
  --phrase-pause 100-250 \
  --sentence-pause 300-600 \
  --repeat-weight 0.5 --repeat-count 1-2 --repeat-style resample \
  --crossfade 50 \
  --noise-level -28 \
  --no-pitch-normalize \
  --breath-probability 0.3 \
  --seed 1337
```

What's happening: Longer words (3-5 syllables) are packed into large phrases (4-6 words) with minimal pauses (100-250ms between phrases). Half of all words get repeated 1-2 extra times with resampled syllables, so repetitions sound similar but not identical. No pitch normalization means the syllables retain their original varied pitches across the recording. The 50ms crossfade blurs syllable boundaries within words. A prominent noise bed (-28dB) and sped-up playback (1.3x) give it a compressed, broadcast-like texture. The output is a fast, dense stream of quasi-linguistic babble with frequent near-repetitions and audible static.

### "Submerged Lecture"

Ultra-slow, stretched, reverb-like decay on word boundaries. Sounds like a recorded lecture played back underwater.

```bash
glottisdale collage recording.mp4 \
  --target-duration 30 \
  --speed 0.5 \
  --boundary-stretch 1 --stretch-factor 2.5-4.0 \
  --word-stretch 0.3 --syllables-per-word 3-5 \
  --phrase-pause 1200-2000 \
  --sentence-pause 2500-4000 \
  --crossfade 60 \
  --noise-level -35 \
  --pitch-range 2 \
  --no-breaths \
  --seed 2049
```

What's happening: Long words (3-5 syllables) have their first and last syllables stretched 2.5-4x, creating drawn-out boundaries that frame faster interior syllables. On top of that, 30% of entire words get stretched after assembly. Global speed at 0.5x halves the playback rate, making everything even slower. Very long gaps (1.2-2s between phrases, 2.5-4s between sentences) create wide pools of near-silence. The 60ms crossfade smears syllable edges. Pitch normalization is tight (2 semitone max shift), keeping the tonal range narrow and monotone. No breaths -- just the pink noise bed in the gaps. The output is extremely slow and spacious, with elongated syllables bleeding into each other, separated by long stretches of faint noise.
