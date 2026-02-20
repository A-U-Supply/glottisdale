# Documentation Overhaul Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure glottisdale docs from a single README into a multi-page guide with nested directories, targeting creative non-technical users as the primary audience.

**Architecture:** Plain Markdown files in `docs/` with nested directories (`getting-started/`, `guide/`, `reference/`). README becomes a landing page with CLI help menus retained. Legacy docs untouched.

**Tech Stack:** Markdown only. No build tools, no static site generator.

---

### Task 1: Create directory structure and docs index

**Files:**
- Create: `docs/index.md`
- Create: `docs/getting-started/` (directory)
- Create: `docs/guide/` (directory)
- Create: `docs/reference/` (directory)

**Step 1: Create directories**

```bash
mkdir -p docs/getting-started docs/guide docs/reference
```

**Step 2: Write `docs/index.md`**

The index should link to every docs page with a one-line description. Keep it short — this is a table of contents, not a narrative.

```markdown
# Glottisdale Documentation

Syllable-level audio collage and vocal MIDI mapping tool.

## Getting Started

- [Installation](getting-started/install.md) — Install glottisdale and its dependencies
- [Quick Start](getting-started/quickstart.md) — Make your first collage in 5 minutes

## Guide

- [Examples](guide/examples.md) — CLI recipes for interesting and creative results
- [Troubleshooting](guide/troubleshooting.md) — Common issues and how to fix them

## Reference

- [Philosophy & Research](reference/philosophy.md) — Why we built it this way
- [Architecture](reference/architecture.md) — Pipeline diagrams and module map
- [Python API](reference/python-api.md) — Using glottisdale as a library

## Legacy

- [Legacy Design Documents](legacy/README.md) — Original design docs from the monorepo era
```

**Step 3: Commit**

```bash
git add docs/index.md
git commit -m "docs: add docs directory structure and index"
```

---

### Task 2: Write the install page

**Files:**
- Create: `docs/getting-started/install.md`

**Step 1: Write `docs/getting-started/install.md`**

Content sections (see design doc for full spec):

1. **Prerequisites** — Python 3.10+, ffmpeg, pip. Per-platform install instructions for each. Plain-English explanation of what each tool is and why it's needed.
   - macOS: `brew install python ffmpeg`
   - Windows: Python from python.org, ffmpeg from gyan.dev or winget
   - Linux: `sudo apt install python3 python3-pip ffmpeg`
2. **Install glottisdale** — The pip install command with line-by-line explanation:
   ```bash
   pip install git+https://github.com/A-U-Supply/glottisdale.git
   ```
3. **Optional extras** — Decision tree format:
   - "I just want collages" → core install (nothing extra)
   - "I want vocal MIDI mapping" → `pip install "glottisdale[sing] @ git+https://github.com/A-U-Supply/glottisdale.git"` + install rubberband (`brew install rubberband` / `sudo apt install rubberband-cli`)
   - "I want the most accurate syllable detection" → `pip install "glottisdale[bfa] @ git+https://github.com/A-U-Supply/glottisdale.git"` + install espeak-ng (`brew install espeak-ng` / `sudo apt install espeak-ng`)
   - "I want everything" → `pip install "glottisdale[all] @ git+https://github.com/A-U-Supply/glottisdale.git"`
4. **Verify your install** — Run `glottisdale --help` and confirm you see the help output
5. **Developer install** — For contributors: clone, `pip install -e '.[all,dev]'`, download NLTK data, run tests with `pytest`

**Step 2: Commit**

```bash
git add docs/getting-started/install.md
git commit -m "docs: add beginner-friendly install guide"
```

---

### Task 3: Write the quickstart page

**Files:**
- Create: `docs/getting-started/quickstart.md`

**Step 1: Write `docs/getting-started/quickstart.md`**

Content sections:

1. **Your first collage** — Assumes install is done. One command:
   ```bash
   glottisdale collage your-video.mp4
   ```
   Explain: what this does (takes a video, extracts speech, chops into syllables, shuffles, outputs a 30-second collage). Where to find the output (`./glottisdale-output/concatenated.wav` + `clips.zip`). What the files are.

2. **Customizing the basics** — Three quick variations:
   ```bash
   # Longer output
   glottisdale collage your-video.mp4 --target-duration 60

   # Reproducible output (same seed = same result)
   glottisdale collage your-video.mp4 --seed 42

   # Multiple input sources
   glottisdale collage video1.mp4 video2.mp4 video3.mp4
   ```

3. **Your first MIDI vocal** (requires `[sing]` extra) — One command:
   ```bash
   glottisdale sing your-video.mp4 --midi path/to/midi-folder/
   ```
   Explain: what this does (takes speech audio and a MIDI melody, maps syllables onto the melody notes, outputs a vocal track). What you need (a directory with `melody.mid`). What the output sounds like conceptually.

4. **What just happened?** — 5-6 sentence plain-English walkthrough: "Glottisdale listened to the speech in your video using AI transcription. It figured out where each syllable starts and ends. Then it shuffled them randomly and stitched them back together with crossfades, pitch matching, and subtle room tone to make it sound organic rather than robotic." Link to [Architecture](../reference/architecture.md) for full pipeline details.

5. **Next steps** — Links:
   - [Examples](../guide/examples.md) for creative recipes
   - [Troubleshooting](../guide/troubleshooting.md) if something went wrong
   - [Philosophy](../reference/philosophy.md) to understand why it works this way

**Step 2: Commit**

```bash
git add docs/getting-started/quickstart.md
git commit -m "docs: add quickstart guide"
```

---

### Task 4: Write the examples page

**Files:**
- Create: `docs/guide/examples.md`

**Step 1: Write `docs/guide/examples.md`**

This is the longest doc page. Organized by creative intent. Each example gets: descriptive title, CLI command, plain-English explanation of the flags used, and a best-effort description of what the output sounds like.

**Section 1: Basic variations**

- Changing duration (`--target-duration`)
- Using a seed for reproducibility (`--seed`)
- Multiple input files (round-robin sampling across sources)
- Choosing a Whisper model (`--whisper-model small` for better transcription)

**Section 2: Shaping the rhythm**

- Short choppy words: `--syllables-per-word 1-2 --words-per-phrase 5-7`
- Long flowing words: `--syllables-per-word 3-5 --words-per-phrase 2-3`
- Rapid-fire: short pauses `--phrase-pause 100-200 --sentence-pause 300-500`
- Slow and deliberate: long pauses `--phrase-pause 800-1200 --sentence-pause 1500-2500`
- Tight crossfades vs hard cuts: `--crossfade 0` vs `--crossfade 60`

**Section 3: Adding texture**

- Stripped down (bare bones): `--no-room-tone --no-breaths --no-prosodic-dynamics --noise-level 0`
- Heavy atmosphere: `--noise-level -30 --breath-probability 0.9`
- Pitch variety vs uniformity: `--no-pitch-normalize` vs `--pitch-range 2`

**Section 4: Stretching and warping**

- Random stretch (dream-like): `--random-stretch 0.3 --stretch-factor 1.5-3.0`
- Alternating stretch (rhythmic): `--alternating-stretch 3 --stretch-factor 2.0`
- Boundary emphasis: `--boundary-stretch 1 --stretch-factor 1.5-2.5`
- Word stretch (thick, slurred): `--word-stretch 0.5 --stretch-factor 1.5-2.0`
- Global speed: `--speed 0.7` (slow and deep) or `--speed 1.5` (fast and frantic)

**Section 5: Repetition and stutter**

- Subtle repetition: `--repeat-weight 0.2 --repeat-count 1`
- Heavy repetition: `--repeat-weight 0.6 --repeat-count 2-4 --repeat-style resample`
- Stutter effect: `--stutter 0.3 --stutter-count 2-3`
- Combining: `--repeat-weight 0.3 --stutter 0.2`

**Section 6: Vocal MIDI recipes**

- Tight melody following: `--drift-range 0.5`
- Loose and expressive: `--drift-range 4.0`
- Clean vocal: `--no-vibrato --no-chorus --drift-range 0`
- Full effect: `--vibrato --chorus --drift-range 2.0` (default)

**Section 7: Combining everything**

2-3 "kitchen sink" examples that layer options from multiple sections. Each with a creative name, the full command, and description of the result.

Example:
```bash
# "Haunted Answering Machine"
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

**Step 2: Commit**

```bash
git add docs/guide/examples.md
git commit -m "docs: add thorough CLI examples guide"
```

---

### Task 5: Write the troubleshooting page

**Files:**
- Create: `docs/guide/troubleshooting.md`

**Step 1: Write `docs/guide/troubleshooting.md`**

Format: each issue is a `###` heading with **Problem / Cause / Fix** structure.

**Section 1: Installation issues**

- `ffmpeg: command not found` — not installed or not in PATH. Fix per platform.
- `pip install` fails with version error — need Python 3.10+. How to check version.
- `espeak-ng` not found (BFA mode) — install per platform or switch to `--aligner default`.
- `rubberband` not found (sing mode) — install per platform.
- `ModuleNotFoundError: pretty_midi` — need `[sing]` extra.

**Section 2: Runtime errors**

- Whisper model download hangs/fails — network issue, try smaller model, or download manually.
- Out of memory — Whisper model too large for available RAM. Use `--whisper-model tiny`.
- `No speech detected` or empty output — input has no speech, or speech too quiet.
- Unsupported input format — ffmpeg can't read the file. Check format.

**Section 3: Output doesn't sound right**

- All silence — input had no detected speech.
- Too short — not enough syllables to fill `--target-duration`. Use more/longer input files.
- Monotone / robotic — try `--no-pitch-normalize` for more variety, or adjust `--pitch-range`.
- Too choppy — increase crossfade: `--crossfade 50 --word-crossfade 80`.

**Section 4: Caching**

Absorb the caching section from the current README:
- Where cache lives (`~/.cache/glottisdale/`)
- Three tiers: extract, whisper, align
- How to bypass: `--no-cache`
- How to clear: `rm -rf ~/.cache/glottisdale/`
- Override location: `GLOTTISDALE_CACHE_DIR` env var

**Section 5: Platform-specific notes**

- macOS: homebrew install paths, Apple Silicon (Whisper runs on MPS if available)
- Windows: adding ffmpeg to PATH, using PowerShell vs Command Prompt
- Linux: apt package names, building rubberband from source if not in repo

**Step 2: Commit**

```bash
git add docs/guide/troubleshooting.md
git commit -m "docs: add troubleshooting guide"
```

---

### Task 6: Write the philosophy and research page

**Files:**
- Create: `docs/reference/philosophy.md`
- Read (for synthesis): `docs/legacy/2026-02-15-glottisdale-design.md`, `docs/legacy/2026-02-15-glottisdale-natural-speech-design.md`, `docs/legacy/2026-02-15-glottisdale-audio-polish-design.md`, `docs/legacy/2026-02-16-hymnal-gargler-design.md`, `docs/legacy/2026-02-19-glottisdale-stretch-repeat-design.md`

**Step 1: Read all 5 legacy design docs** to extract key rationale and decisions.

**Step 2: Write `docs/reference/philosophy.md`**

Two-layer structure: narrative sections with collapsible `<details>` deep dives.

**Narrative sections** (written for non-technical audience):

1. **Why syllables?** — Syllables are the natural unit of speech rhythm. Words are too large (you lose the granularity), phonemes are too small (individual sounds are meaningless), fixed-length chunks cut through the middle of sounds. Syllables preserve just enough meaning to be recognizable while being small enough to rearrange freely.

2. **The pipeline in plain English** — Story-form: "When you give glottisdale a video, it first listens to the speech using AI transcription (the same technology behind voice assistants). It figures out what was said and when each word was spoken. Then it converts those words into their component sounds — the way a dictionary shows pronunciation — and groups those sounds into syllables. From there, it shuffles the syllables, groups them into fake 'words' and 'phrases,' and stitches everything together with crossfades to make it flow."

3. **Making it sound natural** — Raw syllable concatenation sounds like a broken tape deck. Glottisdale applies several layers of polish: pitch normalization (so all syllables sound like they came from the same voice), volume normalization (so nothing jumps out), room tone (real background noise from the source fills the gaps instead of digital silence), breath sounds (humans breathe between phrases), and prosodic dynamics (phrases get slightly louder at the start and softer at the end, like real speech).

4. **The sing feature** — Instead of shuffling syllables randomly, the sing mode assigns each syllable to a note in a MIDI melody. It pitch-shifts the syllable to match the note, stretches it to fill the note's duration, and adds subtle vibrato and chorus effects. The result sounds like a choir that learned the melody but forgot the words.

**Deep dives** (collapsible `<details>` blocks after each section):

- **Syllabification**: Maximum Onset Principle via vendored ARPABET syllabifier. Why MOP over other approaches. BFA as an alternative for real phoneme timestamps.
- **Pitch detection**: Autocorrelation-based F0 estimation. Why not FFT peak or zero-crossing. Clamping to ±5 semitones.
- **Audio polish decisions**: Why pink noise (not white/brown). Why room tone extraction. Why breath insertion probability defaults to 0.6. Crossfade duration reasoning.
- **Vocal mapping**: Why rubberband for pitch shifting. Drift range as an expressiveness knob. Vibrato and chorus implementation.

Synthesize from legacy docs — distill, don't copy.

**Step 3: Commit**

```bash
git add docs/reference/philosophy.md
git commit -m "docs: add philosophy and research page"
```

---

### Task 7: Write the architecture page

**Files:**
- Create: `docs/reference/architecture.md`
- Reference: current `README.md` pipeline sections (lines 147-193)

**Step 1: Write `docs/reference/architecture.md`**

Relocate and expand the pipeline diagrams from the README.

1. **Collage pipeline** — The 17-step pipeline (current README lines 149-167), with a short description of what each step does and which source file handles it:
   - Step 1: `audio.py:extract_audio()` — Extract audio via ffmpeg
   - Step 2: `collage/transcribe.py` — Whisper ASR
   - Step 3: `collage/syllabify.py` — g2p_en phoneme conversion
   - ...etc

2. **Sing pipeline** — The 10-step pipeline (README lines 169-180), same treatment.

3. **Module map** — Table mapping source files to their responsibilities:
   | Module | Purpose |
   |--------|---------|
   | `cli.py` | CLI argument parsing and subcommand dispatch |
   | `types.py` | Core dataclasses: Phoneme, Syllable, Clip, Result |
   | `audio.py` | FFmpeg wrappers for all audio operations |
   | `analysis.py` | RMS, F0, room tone, breaths, pink noise |
   | `collage/__init__.py` | Main collage pipeline orchestration |
   | `collage/transcribe.py` | Whisper ASR integration |
   | `collage/align.py` | Abstract aligner interface |
   | `collage/bfa.py` | Bournemouth Forced Aligner backend |
   | `collage/syllabify.py` | g2p_en + ARPABET syllabification |
   | `collage/phonotactics.py` | Phonotactic syllable ordering |
   | `collage/stretch.py` | Time stretch and word repeat logic |
   | `sing/midi_parser.py` | MIDI file parsing (pretty_midi) |
   | `sing/syllable_prep.py` | Syllable preparation for vocal mapping |
   | `sing/vocal_mapper.py` | Note-to-syllable mapping and rendering |
   | `sing/mixer.py` | Vocal + backing track mixing |

**Step 2: Commit**

```bash
git add docs/reference/architecture.md
git commit -m "docs: add architecture and pipeline reference"
```

---

### Task 8: Write the Python API page

**Files:**
- Create: `docs/reference/python-api.md`
- Reference: `src/glottisdale/collage/__init__.py:203-246` (process() signature), `src/glottisdale/types.py`, `src/glottisdale/collage/align.py`, `src/glottisdale/sing/` modules

**Step 1: Write `docs/reference/python-api.md`**

1. **Core entry points**
   - `glottisdale.collage.process()` — Full function signature with all parameters documented. Group parameters by category (core, prosodic grouping, audio polish, stretch, repeat, stutter, misc).
   - Sing workflow — Since sing doesn't have a single `process()` function, document the step-by-step: `parse_midi()` → `prepare_syllables()` → `plan_note_mapping()` → `render_vocal_track()` → `mix_tracks()`.

2. **Data types** — Each dataclass with field descriptions:
   - `Phoneme(label, start, end)`
   - `Syllable(phonemes, start, end, word, word_index)`
   - `Clip(syllables, start, end, source, output_path)`
   - `Result(clips, concatenated, transcript, manifest)`

3. **Aligner interface** — How `get_aligner()` works:
   - `"auto"` — tries BFA, falls back to default
   - `"default"` — Whisper word timestamps + g2p_en + proportional timing
   - `"bfa"` — Bournemouth Forced Aligner for real phoneme timestamps

4. **Programmatic examples**
   - Basic collage
   - Collage with custom settings
   - Sing workflow step by step
   - Accessing results (clips, manifest, transcript)

**Step 2: Commit**

```bash
git add docs/reference/python-api.md
git commit -m "docs: add Python API reference"
```

---

### Task 9: Slim the README to a landing page

**Files:**
- Modify: `README.md`

**Step 1: Rewrite `README.md`**

New structure:

```markdown
# Glottisdale

Syllable-level audio collage and vocal MIDI mapping tool.

[Compelling 2-3 sentence description of what it does and why it's interesting]

## Quick Start

[Single dead-simple example command with minimal explanation]
[Link to full quickstart guide]

## Install

[One-liner install command]
[Link to full install guide for details and optional extras]

## CLI Reference

### `glottisdale collage`

[Full --help output, kept as-is from current README]

### `glottisdale sing`

[Full --help output, kept as-is from current README]

## Documentation

- [Installation Guide](docs/getting-started/install.md) — ...
- [Quick Start](docs/getting-started/quickstart.md) — ...
- [Examples](docs/guide/examples.md) — ...
- [Troubleshooting](docs/guide/troubleshooting.md) — ...
- [Philosophy & Research](docs/reference/philosophy.md) — ...
- [Architecture](docs/reference/architecture.md) — ...
- [Python API](docs/reference/python-api.md) — ...

## License

GPL v3 — see [LICENSE](LICENSE).
```

Remove from README:
- Pipeline Architecture section (moved to `docs/reference/architecture.md`)
- BFA Aligner section (moved to `docs/getting-started/install.md`)
- Caching section (moved to `docs/guide/troubleshooting.md`)
- Python API example (moved to `docs/reference/python-api.md`)
- Detailed install instructions (moved to `docs/getting-started/install.md`)

Keep in README:
- Full CLI `--help` output for both subcommands
- Quick-start example (simplified to one command)
- One-liner install command

**Step 2: Verify all links resolve**

Check that every `[text](path)` link in README points to a file that exists:
```bash
grep -oP '\(docs/[^)]+\)' README.md | tr -d '()' | while read f; do test -f "$f" || echo "BROKEN: $f"; done
```

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: slim README to landing page with docs links"
```

---

### Task 10: Verify all internal links across all docs

**Files:**
- All files in `docs/`
- `README.md`

**Step 1: Check every internal link in every doc file**

```bash
# Find all markdown links and verify targets exist
for f in README.md docs/index.md docs/getting-started/*.md docs/guide/*.md docs/reference/*.md; do
  dir=$(dirname "$f")
  grep -oP '\]\([^)]+\)' "$f" | grep -v 'http' | tr -d '()' | sed 's/\]//g' | while read link; do
    target="$dir/$link"
    # Strip anchor
    target=$(echo "$target" | cut -d'#' -f1)
    if [ -n "$target" ] && [ ! -f "$target" ]; then
      echo "BROKEN in $f: $link -> $target"
    fi
  done
done
```

**Step 2: Fix any broken links found**

**Step 3: Final commit if any fixes needed**

```bash
git add -A docs/ README.md
git commit -m "docs: fix broken internal links"
```

---

### Task 11: Push and open PR

**Step 1: Push branch**

```bash
gh auth switch --user doo-nothing
git push -u origin docs/documentation-overhaul
```

**Step 2: Open PR**

```bash
gh pr create --title "docs: restructure documentation into multi-page guide" --body "$(cat <<'EOF'
## Summary

- Restructured docs from single README into nested `docs/` directory
- README slimmed to landing page with CLI help menus and docs links
- Added beginner-friendly install guide with per-platform instructions
- Added quickstart guide (first collage in 5 minutes)
- Added thorough CLI examples organized by creative intent
- Added troubleshooting guide with caching docs (moved from README)
- Added philosophy/research page synthesized from legacy design docs
- Added architecture reference with pipeline diagrams (moved from README)
- Added Python API reference

## Test plan

- [ ] All internal markdown links resolve correctly
- [ ] README renders correctly on GitHub
- [ ] docs/index.md links are all valid
- [ ] Install commands are accurate (test on clean machine if possible)
- [ ] Review audio descriptions in examples for accuracy

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```
