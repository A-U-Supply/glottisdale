# Shuffle Mode Design

**Date:** 2026-03-14
**Status:** Approved

## Problem

The current collage mode assembles random syllables, producing output that sounds like scanning radio stations — choppy, unnatural, nothing like human speech. Random assembly can never produce natural-sounding gibberish because it destroys coarticulation, prosodic rhythm, and syllable-level timing.

## Solution

New `--mode shuffle` for the collage subcommand. Uses each source's own transcription as a timing/rhythm template, then fills each syllable slot with phonetically-matched syllables from OTHER sources. Reuses existing speak mode infrastructure.

## How It Works

1. Transcribe and syllabify all N sources (existing pipeline)
2. For each source S_i:
   - S_i's syllable sequence becomes the **template** (preserving syllable durations, word boundaries, phrase timing)
   - Build a **syllable bank** from all sources EXCEPT S_i
   - Run the **speak matcher** (existing Viterbi DP) to find best phonetic matches from the bank for each template syllable
   - **Assemble** using speak assembler (time-stretches matched syllables to fit template durations, preserves contiguous source runs)
3. Concatenate all template outputs, trim to target duration

## What's Reused

- `speak::syllable_bank::build_bank()` — builds phoneme-indexed bank
- `speak::matcher::match_syllables()` — Viterbi phonetic matching
- `speak::assembler::assemble()` — timing-preserving assembly with contiguity
- All transcription, syllabification, audio extraction — unchanged

## What's New

- `collage::shuffle::process_shuffle()` — orchestration function (~100-150 lines)
- CLI: `--mode` flag (values: `random`, `shuffle`; default: `random`)
- Bot: passes `--mode shuffle`

## CLI

```
glottisdale collage --mode shuffle [existing flags...] input1.mp4 input2.mp4 ...
```

Minimum 2 source files required (need at least 1 template + 1 bank source).

## Not Changed

- `--mode random` (current behavior) remains default and unchanged
- Speak and sing subcommands unchanged
- All existing CLI flags apply to both modes
