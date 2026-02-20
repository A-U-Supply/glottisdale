# Quick Start

This page gets you from zero to hearing your first syllable collage in under 5 minutes. It assumes you have already [installed glottisdale](install.md).

All you need is a video or audio file that contains speech. A podcast clip, a conference talk, a voice memo, a YouTube download -- anything with someone talking will work. Glottisdale handles both video and audio formats (MP4, MOV, WAV, MP3, and anything else ffmpeg understands).

> **Tip:** Longer source files with lots of speech give glottisdale more syllables to work with, which generally produces more interesting and varied results.

---

## Your first collage

Open a terminal, navigate to a folder where you want to work, and run:

```bash
glottisdale collage your-video.mp4
```

That's it. Glottisdale will take your video, extract the speech, chop it into individual syllables, shuffle them into new fake words and phrases, and stitch everything back together into a 30-second audio collage.

You will see progress output as it works through the pipeline:

```
glottisdale.collage INFO: Processing 1 input file(s)
glottisdale.collage INFO: Extracting audio from your-video.mp4
glottisdale.collage INFO: Transcribing with Whisper (base model)
glottisdale.collage INFO: Aligning syllables
glottisdale.collage INFO: Selected 47 clips for 30.0s target
glottisdale.collage INFO: Assembling collage
Processed 1 source file(s)
Transcript: hello everyone welcome to the presentation today we will be discussing...
Selected 47 clips
Output:
  concatenated.wav
  clips.zip
```

The first run takes a minute or two because glottisdale needs to download the Whisper speech recognition model (about 140 MB for the default `base` model). This only happens once. Subsequent runs on the same file are much faster because glottisdale caches the transcription and alignment results.

### Where to find the output

When it finishes, look inside the `./glottisdale-output/` folder. You will find:

| File | What it is |
|------|-----------|
| `concatenated.wav` | Your finished collage -- a single audio file you can play in any media player |
| `clips.zip` | A zip archive containing every individual syllable clip that was used, in case you want to inspect or remix them yourself |

Open `concatenated.wav` in your audio player and listen. You should hear something that sounds like speech -- the voice, the rhythm, the breathing are all familiar -- but the words are nonsense. That is the collage.

The individual clips in `clips.zip` are named after the syllables they contain (like `hel.wav`, `lo.wav`, `ev.wav`). You can unzip them and use them as building blocks in a DAW or audio editor if you want to arrange things by hand.

---

## Customizing the basics

Here are three simple variations to try once you have your first collage working.

### Make it longer

The default output is 30 seconds. To make a one-minute collage instead:

```bash
glottisdale collage your-video.mp4 --target-duration 60
```

You can use any duration in seconds. Keep in mind that longer outputs need more source speech to draw from -- if your input video only has 10 seconds of speech, a 60-second collage will reuse syllables heavily.

### Make it reproducible

Every run shuffles syllables differently, so you get a unique collage each time. If you find a result you like and want to recreate it exactly, use a seed:

```bash
glottisdale collage your-video.mp4 --seed 42
```

The same seed with the same input file will always produce the same output. Any integer works as a seed. Share the seed number with someone else and they can reproduce your exact collage from the same source file.

### Use multiple sources

You can feed in several files at once. Glottisdale will pull syllables from all of them:

```bash
glottisdale collage video1.mp4 video2.mp4 video3.mp4
```

This is a great way to create collages that blend multiple speakers or conversations together. Glottisdale samples syllables from all sources, so you will hear voices mixing and overlapping in ways the original speakers never intended.

---

## Your first MIDI vocal

> **Requires the `[sing]` extra.** If you haven't installed it yet, see the [vocal MIDI mapping section](install.md#i-want-vocal-midi-mapping) of the install guide. You will also need rubberband installed on your system for pitch-shifting.

The `sing` command takes the same speech audio, but instead of shuffling syllables randomly, it maps them onto a MIDI melody. Each syllable gets pitch-shifted and time-stretched to match a note in the melody.

### What you need

You will need two things:

1. **A speech source** -- the same kind of video or audio file you used for collage.
2. **A MIDI folder** -- a directory containing a MIDI file named `melody.mid`. This is the tune that glottisdale will try to "sing" using the syllables from your video.

The MIDI file should contain a single-voice melody. Simple tunes work best -- nursery rhymes, folk songs, or short melodic phrases. Complex polyphonic arrangements will still work, but glottisdale uses the first track it finds.

### Run it

```bash
glottisdale sing your-video.mp4 --midi path/to/midi-folder/
```

### What you get

The output lands in `./glottisdale-output/` just like before:

| File | What it is |
|------|-----------|
| `full_mix.wav` | The vocal track mixed with a simple synthesized MIDI backing |
| `acappella.wav` | The vocal track on its own, without any backing |

The result sounds like a choir that learned the melody but forgot the words. The voice is recognizably human, the notes follow the tune you provided, but the lyrics are delightfully garbled nonsense syllables. Glottisdale adds subtle vibrato and chorus effects by default to give it a more organic, slightly wobbly character.

Play `full_mix.wav` to hear the vocal with its MIDI backing, or `acappella.wav` if you want to drop the vocal track into your own project or DAW.

---

## What just happened?

Here is what glottisdale did behind the scenes, in plain English.

Glottisdale listened to the speech in your video using AI-powered transcription -- the same kind of technology behind voice assistants and automatic subtitles. It figured out every word that was said and exactly when each word starts and ends in the audio. Then it converted those words into their component sounds (the way a dictionary shows pronunciation) and grouped those sounds into syllables, which are the natural rhythmic building blocks of speech.

For a collage, it shuffled those syllables randomly and assembled them into fake "words," "phrases," and "sentences," with realistic-sounding pauses between them. It stitched them together with crossfades so they flow smoothly, normalized the pitch and volume so everything sounds like it came from one consistent voice, and filled the gaps with real room tone extracted from the original recording instead of dead digital silence. It even inserted subtle breath sounds at phrase boundaries, because that is what humans do when they talk -- and it is one of those small details that makes the result sound organic rather than robotic.

For a MIDI vocal, instead of shuffling, it assigned each syllable to a note in your melody, pitch-shifted it to match that note's frequency, and stretched it to fill the note's duration. The result follows the tune while keeping the grain and texture of real human speech.

For the full technical breakdown of every step in the pipeline, see the [Architecture](../reference/architecture.md) reference.

---

## Next steps

Now that you have heard what glottisdale can do with the defaults, here is where to go next:

- **[Examples](../guide/examples.md)** -- Creative recipes and CLI combinations for different sounds and styles. Learn how to make "Haunted Answering Machine" collages, rapid-fire stutter effects, slow dream-like stretches, and more.

- **[Troubleshooting](../guide/troubleshooting.md)** -- If something went wrong, the output is silent, or the result does not sound right, start here. Covers common issues with installation, runtime errors, and tips for improving output quality.

- **[Philosophy](../reference/philosophy.md)** -- Understand why glottisdale works the way it does, from why syllables are the right unit of speech to break apart, to the audio polish decisions that make the output sound natural.

If you are a developer and want to use glottisdale as a Python library rather than a CLI tool, see the **[Python API](../reference/python-api.md)** reference for programmatic usage and data types.
