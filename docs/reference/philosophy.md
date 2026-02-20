# Philosophy & Research

How and why glottisdale works the way it does.

---

## Why syllables?

Every audio collage tool has to decide where to cut. The choice of unit shapes everything about the output -- its rhythm, its texture, whether it feels like speech or noise.

**Words are too large.** If you shuffle whole words, the result sounds like a sentence rearranged by a toddler. You can still hear the meaning of each piece, which makes the output feel more like a bad remix than something genuinely new. There is not enough granularity to build surprising combinations.

**Phonemes are too small.** Individual sounds like "p" or "ah" carry no recognizable trace of the speaker or the source material. A collage of phonemes sounds like a synthesizer, not a person. You lose the human quality that makes the collage interesting.

**Fixed-length chunks cut arbitrarily.** Slicing audio into 100-millisecond windows is simple, but the boundaries fall in the middle of sounds. You get clicks, half-vowels, and consonants chopped in two. The result is choppy and mechanical.

**Syllables sit in the sweet spot.** A syllable preserves just enough of the original speech to be recognizable -- you can hear fragments of real words, catch the ghost of a phrase -- while being small enough to rearrange freely into surreal new combinations. Syllables are also the natural unit of speech rhythm: they are what our brains use to track the beat of language. Cutting at syllable boundaries produces clips that start and end at natural transition points, which means they flow together more smoothly when reassembled.

<details>
<summary>Technical deep dive: syllabification algorithm</summary>

Glottisdale uses the **Maximum Onset Principle** (MOP) for syllabification, implemented via a vendored ARPABET syllabifier from [kylebgorman/syllabify](https://github.com/kylebgorman/syllabify).

MOP works by first identifying vowel nuclei as syllable centers, then assigning as many consonants as possible to the **onset** of the following syllable rather than the **coda** of the preceding one. For example, in the word "extra" (/EH K S T R AH/), MOP would place the /T R/ cluster as the onset of the second syllable rather than splitting them, because /TR/ is a valid English onset. This matches how English speakers naturally divide syllables.

The algorithm operates on ARPABET phoneme labels (e.g., "AH0", "K", "S"), which are produced by the `g2p_en` grapheme-to-phoneme converter. Syllable boundaries are constrained to respect word boundaries -- no cross-word syllabification. This is linguistically imperfect (connected speech often resyllabifies across words), but it avoids producing clips that awkwardly span the silence between words.

**Alternative: Bournemouth Forced Aligner (BFA).** The default pipeline estimates phoneme timing proportionally within each word (if a word is 500ms long and has 5 phonemes, each gets 100ms). BFA instead analyzes the actual audio signal to determine where each phoneme begins and ends, producing more precise syllable boundaries. When BFA is installed, glottisdale uses it automatically (via the `--aligner auto` mode). BFA requires `espeak-ng` as a system dependency and outputs IPA phonemes, which glottisdale handles with a separate IPA-aware syllabification path based on sonority sequencing.

</details>

---

## The pipeline in plain English

When you give glottisdale a video, here is what happens:

First, it **listens to the speech**. The audio track is extracted and fed to Whisper, an AI transcription model (the same technology behind voice assistants and automatic captions). Whisper produces a transcript with timestamps for every word -- it knows that "hello" was spoken between 0.4 and 0.8 seconds, "world" between 0.9 and 1.3 seconds, and so on.

Next, it **converts words into sounds**. Each word is passed through a pronunciation model that produces the same kind of notation a dictionary uses to show how words are pronounced. "Hello" becomes something like /HH AH L OW/. These individual sounds are then grouped into syllables: /HH AH/ and /L OW/.

Since Whisper only tells us when each word starts and ends, glottisdale **estimates where each syllable falls** within the word based on how many sounds it contains. A two-syllable word gets its time divided roughly in half; a three-syllable word in thirds.

Then comes the creative part. The syllables are **shuffled randomly** and grouped into fake "words" (clusters of 1-4 syllables), fake "phrases" (groups of 3-5 words), and fake "sentences" (groups of 2-3 phrases). Within each word, the syllables are reordered so they sound like they could belong together -- following the same rules that govern which sound combinations are valid in English. The result is nonsense that sounds like it *could* be a language.

Finally, everything is **stitched together** with crossfades between syllables, natural pauses between phrases, and several layers of audio polish to make it sound smooth and continuous rather than choppy.

<details>
<summary>Technical deep dive: transcription and alignment</summary>

**Whisper ASR** runs on the extracted 16kHz mono WAV and returns word-level timestamps. The `word_timestamps=True` flag enables Whisper's dynamic time warping alignment, which maps each decoded token back to its position in the audio. Model sizes range from `tiny` (~39M parameters, fast but less accurate) to `medium` (~769M parameters, slower but more reliable). The default `base` model balances speed and accuracy for this use case.

**g2p_en** (grapheme-to-phoneme for English) converts orthographic words to ARPABET phoneme sequences. It uses a combination of a CMU Pronouncing Dictionary lookup and a neural network fallback for out-of-vocabulary words. ARPABET labels include stress markers on vowels (e.g., "AH0" for unstressed, "AH1" for primary stress), which the syllabifier uses to identify nuclei.

**Proportional phoneme timing** distributes each word's duration across its phonemes by count. If a word spans 600ms and has 6 phonemes, each gets 100ms. This is an approximation -- in real speech, vowels are typically longer than stops -- but it is good enough for collage purposes, where the clips will be shuffled anyway.

The abstract aligner interface (`align.py`) defines a common `Aligner` base class with a `process()` method. The `DefaultAligner` chains Whisper + g2p_en + syllabify. The `BFAAligner` uses the Bournemouth Forced Aligner for true phoneme-level timestamps derived from the audio signal itself. The `--aligner auto` mode tries BFA first and falls back to the default if BFA is not installed.

</details>

<details>
<summary>Technical deep dive: phonotactic ordering</summary>

After syllables are shuffled and grouped into words, they are **reordered within each word** using phonotactic constraints -- the rules governing which sound combinations are legal in a given language.

The reordering uses **junction scoring** between consecutive syllables. For each pair of adjacent syllables, three factors are evaluated:

1. **Sonority contour.** Natural syllable boundaries follow a sonority dip: the end of one syllable falls in sonority, and the start of the next rises. This is scored using an ARPABET sonority scale (stops = 1, affricates = 2, fricatives = 3, nasals = 4, liquids = 5, glides = 6, vowels = 7). A falling-then-rising contour at the junction gets +1; a rising-then-rising contour (two onsets colliding) gets -1.

2. **Illegal onset filter.** Some sounds cannot start a syllable in English (e.g., /NG/, /ZH/). If the next syllable would begin with one of these, the junction gets -2.

3. **Hiatus penalty.** Vowel-to-vowel boundaries across syllables are disfavored in English. A vowel ending one syllable followed by a vowel starting the next gets -1.

For each word, 5 random permutations of its syllables are scored, and the permutation with the highest total junction score is selected. This is computationally cheap (5 permutations of 2-4 items) and produces nonsense words that sound more plausibly English than random ordering.

When BFA is active and phonemes are in IPA rather than ARPABET, the phonotactics module uses a parallel IPA sonority mapping with equivalent categories.

</details>

---

## Making it sound natural

If you simply cut out syllables and glue them back together, the result sounds like a broken tape deck. There are several compounding problems:

- **Dead silence between phrases.** Digital silence -- literal zero values -- sounds nothing like the quiet in a real room. Your ear immediately notices the void.
- **Wildly varying pitch.** Syllables from different moments in a conversation have different pitches. Spliced together, they sound like multiple people interrupting each other mid-word.
- **No breath sounds.** Humans breathe between phrases. Without breaths, the output feels robotic and relentless.
- **Flat amplitude.** Real speech has natural stress patterns -- phrases start with a small burst of energy and soften toward the end. Without this, the output sounds monotone.

Glottisdale applies several layers of polish to address these:

- **Pitch normalization.** Every syllable's pitch is measured and shifted toward a common baseline, so the output sounds like it comes from one voice. Shifts are kept subtle to avoid artifacts.
- **Volume normalization.** All syllables are brought to a consistent loudness level, so nothing jumps out unexpectedly.
- **Room tone.** Instead of digital silence in the gaps between phrases, glottisdale extracts the quietest moment from the source audio -- the natural background noise of the room -- and uses that. The result is gaps that sound like real pauses.
- **Breath sounds.** Glottisdale scans the source audio for real breaths (the short inhales between words) and inserts them at phrase boundaries. Not every phrase gets a breath -- about 60% do by default -- which matches the variability of natural breathing.
- **Prosodic dynamics.** Phrases get a slight volume boost at the start and a gentle fade at the end, mimicking the natural stress contour of spoken sentences.
- **Pink noise bed.** A very subtle layer of pink noise runs underneath the entire output, eliminating the "void" feeling during any remaining quiet moments. It is mixed at -40dB below speech level -- barely perceptible, but it fills the silence.

<details>
<summary>Technical deep dive: pitch detection</summary>

Pitch estimation uses **autocorrelation-based F0 detection** implemented in numpy. The algorithm:

1. Compute the normalized autocorrelation of the signal for lag values corresponding to 50-400Hz (the typical range of human speech fundamental frequency).
2. Search from the shortest lag (highest frequency) toward longer lags, looking for the first autocorrelation peak above a periodicity threshold of 0.3.
3. Searching from the high-frequency end first avoids **octave errors** -- a common failure mode where the detector locks onto a harmonic at half the true frequency.

Autocorrelation was chosen over two alternatives:
- **FFT peak detection** is less robust for speech because the fundamental is often weaker than its harmonics, especially for nasal or breathy vowels.
- **Zero-crossing rate** is fast but too noisy for short clips, frequently confusing noise bursts with periodic signal.

The **median F0** across all syllables becomes the normalization target. Each syllable's pitch is shifted to match via ffmpeg's `asetrate` + `aresample` filters, which change pitch without altering duration. Shifts are **clamped to +/-5 semitones** -- beyond that, formant distortion becomes audible and the syllable no longer sounds like natural speech.

Syllables where pitch detection fails (unvoiced consonants like /s/ or /f/, or very noisy clips) are left unmodified. This is intentional: unvoiced sounds have no meaningful pitch, so shifting them would only introduce artifacts.

</details>

<details>
<summary>Technical deep dive: audio polish details</summary>

**Pink noise** was chosen over white noise (too hissy, emphasizes high frequencies) and brown noise (too rumbly, emphasizes low frequencies). Pink noise has a 1/f power spectrum -- equal energy per octave -- which closely approximates real room ambience and recording noise floors. The implementation generates white noise, applies a 1/sqrt(f) spectral filter via FFT, and normalizes the result.

**Room tone extraction** analyzes each source file with windowed RMS energy (25ms windows, 12ms hops) to find the quietest continuous region of at least 500ms. The threshold is set at 10% of the mean RMS energy, which separates true quiet (room tone, HVAC hum, ambient noise) from speech. The extracted room tone is looped and faded in/out at gap boundaries (50ms fades) to prevent pops. If no source file has a sufficiently quiet region, the feature falls back to the pink noise bed alone.

**Breath detection** scans Whisper's word-level timestamps for inter-word gaps of 200-600ms -- the typical duration of a breath in conversational speech. Each gap region is extracted and filtered by RMS energy: breaths should be louder than room tone (above 1% of speech energy) but quieter than speech (below 30% of speech energy). Valid breaths are pooled per source file. At phrase boundaries, a breath is randomly drawn from the pool and placed at the start of the gap, with room tone or silence filling the remainder. The 200ms lower bound avoids capturing micro-pauses (which are usually silence, not breaths), while the 600ms upper bound excludes long pauses (which may contain coughs, laughter, or other non-breath sounds).

**Prosodic dynamics** apply a phrase-level volume envelope: approximately +1dB boost over the first word (onset energy) and approximately -3dB fade over the last word (phrase-final softening). This mimics the natural tendency in English for speakers to start phrases with slightly more force and trail off at the end.

</details>

---

## The sing feature

Instead of shuffling syllables randomly, sing mode assigns each syllable to a note in a MIDI melody. The syllable is pitch-shifted to match the note, time-stretched to fill the note's duration, and optionally treated with vibrato and chorus effects.

The aesthetic goal is **"drunk choir learns a melody."** Not precise karaoke -- the syllables do not track the melody exactly. They wander slightly off pitch, arrive a little early or late, and sustain unevenly. Some notes get a single stretched syllable; others get a rapid-fire chant of two or three syllables at the same pitch. The result feels like singing in the way that a group of enthusiastic amateurs at a pub feels like singing: recognizably melodic, but loose, imperfect, and human.

Pitch following is intentionally imprecise. Each note's target pitch drifts randomly by up to two semitones from the melody (weighted toward zero, so most notes are close). Sustained notes get vibrato -- a gentle pitch oscillation that gives them a singing quality. Notes held longer than 600 milliseconds may get a chorus effect: multiple slightly detuned copies of the syllable layered together, creating the impression of several voices.

The output is two files: an a cappella track (just the vocal collage mapped to the melody) and a full mix (the vocal layered over a simple sine-wave synthesis of the MIDI backing tracks).

<details>
<summary>Technical deep dive: vocal mapping</summary>

**Pitch shifting and time stretching** are handled by the rubberband library via ffmpeg's `rubberband` audio filter. Rubberband is a formant-preserving time-stretcher, meaning it can change pitch without making the voice sound chipmunk-like (or conversely, unnaturally deep). The pitch ratio is computed as `2^(semitones/12)`, and the tempo ratio adjusts the syllable's natural duration to match the note's duration. Both are clamped to a 4x range (0.25 to 4.0) to avoid extreme artifacts.

**Drift** is applied as a Gaussian-distributed random offset (mean 0, standard deviation = drift_range/3), clamped to +/-drift_range semitones. The default drift_range of 2 semitones means most notes land within about a semitone of the melody, with occasional wider excursions. This produces the "loose pitch following" effect central to the aesthetic.

**Note duration classification** determines the mapping strategy:
- **Short notes** (under 200ms): one syllable, time-stretched to fit.
- **Medium notes** (200ms to 1 second): randomly assigned 1-3 syllables, producing either a sustained sound or a rapid chant.
- **Long notes** (over 1 second): randomly assigned 1-4 syllables, often with vibrato.

Syllables are assigned to notes sequentially from a pool, cycling back to the beginning when the pool is exhausted. This means the same syllable may appear multiple times across the output, but in different pitch and timing contexts each time.

**Vibrato** is applied via ffmpeg's `vibrato` filter at approximately 5.5Hz with a depth of 50 cents (half a semitone). It activates on long notes and on medium notes exceeding 600ms. The rate of 5-6Hz matches natural singing vibrato.

**Chorus** layers 2 additional copies of the syllable, each detuned by 10-15 cents in a random direction and delayed by 15-30ms. The copies are mixed at half the volume of the primary voice. The slight pitch and timing differences between copies create the impression of multiple singers, similar to a choral unison effect. Chorus is applied to all sustained notes (over 600ms) and to approximately 30% of other notes.

**Rhythmic freedom** is built into the rendering: each syllable's duration within a multi-syllable note is varied by +/-20% of the mathematically even division. This prevents the mechanical feel of perfectly quantized timing.

</details>
