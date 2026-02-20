# Unique Run Naming

**Date:** 2026-02-19
**Status:** Approved

## Problem

Every glottisdale run overwrites the previous run's output. Running twice with the same output directory destroys `concatenated.wav`, `clips.zip`, `manifest.json`, and the entire `clips/` directory. Unique creative output is lost with no way to compare runs.

## Solution

Every run creates a unique subdirectory inside the output root, named with a date prefix and a speech/music-themed adjective-noun pair (e.g. `2026-02-19-breathy-bassoon`).

## Name Generation

New module `src/glottisdale/names.py`:

- ~200 speech/voice/music-themed adjectives (breathy, staccato, resonant, hushed, trembling, nasal, falsetto, muffled, lilting, guttural, etc.)
- ~200 speech/voice/music-themed nouns (bassoon, tenor, whisper, larynx, soprano, vibrato, pharynx, timpani, glissando, contralto, etc.)
- `generate_name(seed=None) -> str` — picks adjective-noun pair deterministically if seed provided, randomly otherwise
- `generate_run_id(seed=None) -> str` — returns `YYYY-MM-DD-adjective-noun`
- Collision handling: if the directory already exists, append `-2`, `-3`, etc.
- 200 x 200 = 40,000+ unique combinations

## Output Directory Structure

### Collage

```
glottisdale collage video.mp4
# -> ./glottisdale-output/2026-02-19-breathy-bassoon/

glottisdale collage video.mp4 --output-dir ./my-project
# -> ./my-project/2026-02-19-staccato-tenor/

glottisdale collage video.mp4 --run-name final-take
# -> ./glottisdale-output/2026-02-19-final-take/
```

Each run directory:
```
2026-02-19-breathy-bassoon/
├── concatenated.wav
├── clips/
│   ├── 001_word.wav
│   ├── 002_word.wav
│   └── ...
├── clips.zip
└── manifest.json
```

### Sing

```
glottisdale sing video.mp4 --midi ./midi/
# -> ./glottisdale-output/2026-02-19-guttural-timpani/
```

Each run directory:
```
2026-02-19-guttural-timpani/
├── full_mix.wav
├── acappella.wav
├── midi_backing.wav
└── work/
```

## Code Changes

### New file: `src/glottisdale/names.py`
- Adjective and noun word lists (~200 each)
- `generate_name(seed)` and `generate_run_id(seed)` functions
- Collision-safe directory creation helper

### Modified: `cli.py`
- Add `--run-name` argument to both collage and sing subparsers
- Before calling pipeline, resolve the run directory: generate name, create subdir inside output-dir
- Print the run name at the top of output
- Pass the run subdir as `output_dir` to the pipeline functions

### Modified: `collage/__init__.py`
- Remove `shutil.rmtree(clips_dir)` — each run has a fresh directory, no need to wipe
- No signature changes — the function already accepts `output_dir` as a parameter

### Modified: `cli.py` (sing section)
- Same pattern as collage: resolve run subdir, pass as `output_dir`

### Not changed
- `cache.py` — completely independent, keyed by input file hashes in `~/.cache/glottisdale`
- All pipeline logic — `process()` doesn't care what the directory is called
- `types.py` — `Result.concatenated` already stores a full `Path`

## Testing

### New: `tests/test_names.py`
- `generate_name()` returns `adjective-noun` format
- `generate_name(seed=42)` is deterministic
- `generate_run_id()` returns `YYYY-MM-DD-adjective-noun` format
- Collision handling appends `-2`, `-3`
- All words are lowercase, contain only letters/hyphens

### Updated: `tests/collage/test_cli.py`
- Collage output goes into run subdirectory
- `--run-name` flag is respected
- `--output-dir` + auto-name creates correct nested structure

### Updated: integration tests
- `Result.concatenated` path is inside the run subdir
- Two runs with different seeds produce different directories
