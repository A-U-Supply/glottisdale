# Documentation Overhaul Design

## Summary

Restructure glottisdale's documentation from a single README into a multi-page guide with nested directories. The README becomes a landing page; detailed content moves into `docs/` organized by purpose.

## Audience

Primary: Creative non-technical users (artists, musicians, content creators). Secondary: Developers using glottisdale as a library or contributing to it.

## File Structure

```
README.md                              ← landing page + CLI help menus
docs/
  index.md                             ← full guide index with descriptions
  getting-started/
    install.md                         ← beginner-friendly install walkthrough
    quickstart.md                      ← first collage in 5 minutes
  guide/
    examples.md                        ← thorough CLI recipes with audio descriptions
    troubleshooting.md                 ← common errors, platform gotchas
  reference/
    philosophy.md                      ← two-layer: narrative + technical deep dives
    python-api.md                      ← developer API reference
    architecture.md                    ← pipeline diagrams (moved from README)
  legacy/                              ← existing legacy design docs, untouched
```

## Page Designs

### README.md (landing page)

Slim the current README to:

1. **Tagline** — One sentence describing glottisdale
2. **What it does** — 2-3 compelling sentences
3. **Quick-start** — Single example command, link to `getting-started/quickstart.md`
4. **Install** — One-liner install command, link to `getting-started/install.md`
5. **CLI reference** — Full `--help` output for `collage` and `sing` (retained)
6. **Docs index** — Links to each docs page with one-line descriptions
7. **License**

Content that moves out:
- Pipeline architecture diagrams → `reference/architecture.md`
- Detailed install (extras, system deps) → `getting-started/install.md`
- Python API example → `reference/python-api.md`
- BFA integration details → `getting-started/install.md`
- Caching section → `guide/troubleshooting.md` or `reference/architecture.md`

### docs/index.md (guide index)

Quick overview of the project with links and one-line descriptions for every docs page. Serves as the entry point for anyone navigating the `docs/` directory.

### getting-started/install.md

Audience: Non-technical users. Explains everything from scratch.

1. **Prerequisites** — Python, ffmpeg, pip/uv. Per-platform instructions (macOS, Windows, Linux). Plain English explanations of what each tool is.
2. **Install glottisdale** — Core install command, explained line by line.
3. **Optional extras** — Decision tree by intent:
   - "I just want collages" → core install
   - "I want sing/MIDI" → `glottisdale[sing]` + rubberband
   - "I want best syllable accuracy" → `glottisdale[bfa]` + espeak-ng
   - "I want everything" → `glottisdale[all]`
4. **Verify your install** — One command to confirm it works.
5. **Developer install** — Brief section for people cloning the repo and running tests.

### getting-started/quickstart.md

Goal: First collage in under 5 minutes (post-install).

1. **Your first collage** — One command, explanation, where to find output.
2. **Your first MIDI vocal** — Same for `glottisdale sing`.
3. **What just happened?** — Plain-English pipeline walkthrough. Links to `reference/architecture.md`.
4. **Next steps** — Links to examples, troubleshooting.

~150-200 lines.

### guide/examples.md

Organized by creative intent, not by flag name. Each example:
- Descriptive title (creative outcome)
- CLI command
- Plain-English explanation of relevant flags
- Description of what the output sounds like (best-effort, to be reviewed by maintainer for accuracy)

Sections:
1. **Basic variations** — Seed, duration, multiple inputs
2. **Shaping the rhythm** — Prosodic grouping (syllables-per-word, words-per-phrase, pauses)
3. **Adding texture** — Room tone, breaths, pink noise, prosodic dynamics
4. **Stretching and warping** — Time stretch options (random, alternating, boundary, word)
5. **Repetition and stutter** — Word repeat and stutter effects
6. **Vocal MIDI recipes** — Sing mode: drift, vibrato, chorus variations
7. **Combining everything** — Kitchen-sink examples layering multiple options

### guide/troubleshooting.md

Problem → Cause → Fix format. Concise entries.

1. **Installation issues** — ffmpeg not found, pip errors, Python version, espeak-ng, rubberband
2. **Runtime errors** — Whisper download failures, OOM, unsupported formats, empty output
3. **Output doesn't sound right** — Silence, too short/long, monotone
4. **Platform-specific notes** — macOS (homebrew, Apple Silicon), Windows (PATH), Linux (apt)

Also absorb the caching section from the current README (cache location, clearing, `--no-cache`, `GLOTTISDALE_CACHE_DIR`).

### reference/philosophy.md

Two-layer structure: approachable narrative with collapsible `<details>` deep dives.

**Narrative sections** (non-technical):
1. **Why syllables?** — Why syllable boundaries vs words/phonemes/fixed chunks
2. **The pipeline in plain English** — Story-form walkthrough of what happens to audio
3. **Making it sound natural** — Why raw concatenation sounds robotic, what polish does
4. **The sing feature** — How syllable-to-MIDI mapping works conceptually

**Deep dives** (collapsible, synthesized from legacy docs):
- Specific algorithms and why chosen (Maximum Onset Principle, autocorrelation F0, g2p_en vs BFA)
- Trade-offs considered and rejected
- Distilled from legacy design docs, not copied verbatim

If the page gets too long, split deep dives into `reference/deep-dives.md`.

### reference/python-api.md

Developer-oriented, concise.

1. **Core entry points** — `glottisdale.collage.process()` and sing equivalent, signatures, parameters
2. **Data types** — `Phoneme`, `Syllable`, `Clip`, `Result` with field descriptions
3. **Aligner interface** — How to use different backends, auto-selection
4. **Programmatic examples** — Short Python snippets for common patterns

Lean on existing docstrings. Fill gaps where needed.

### reference/architecture.md

Relocated from README, lightly edited for standalone context.

1. **Collage pipeline** — 16-step diagram with descriptions
2. **Sing pipeline** — 10-step diagram
3. **Module map** — Which source files handle which steps

## Notes

- Audio descriptions in examples are best-effort based on code analysis. Maintainer should review for accuracy.
- Legacy docs remain untouched in `docs/legacy/`.
- README retains full `--help` output for both subcommands.
- Docs convention enforced by CLAUDE.md: updates happen in the same branch/PR as feature changes.
