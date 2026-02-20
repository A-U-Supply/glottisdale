# Unique Run Naming Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Every glottisdale run creates a unique, timestamped subdirectory with a speech/music-themed name so output files are never clobbered.

**Architecture:** New `names.py` module generates adjective-noun pairs from curated thematic word lists. The CLI resolves a unique run directory before calling pipeline functions. Pipeline code is untouched — it already accepts `output_dir` as a parameter.

**Tech Stack:** Python 3.11+, no new dependencies. Word lists are hardcoded.

---

### Task 1: Name Generation Module — Tests

**Files:**
- Create: `tests/test_names.py`

**Step 1: Write the failing tests**

```python
"""Tests for run name generation."""

import re
from datetime import date
from pathlib import Path

from glottisdale.names import ADJECTIVES, NOUNS, generate_name, generate_run_id, create_run_dir


def test_adjectives_are_lowercase_alpha():
    """All adjectives contain only lowercase letters and hyphens."""
    for adj in ADJECTIVES:
        assert re.fullmatch(r"[a-z]+(-[a-z]+)*", adj), f"Invalid adjective: {adj}"


def test_nouns_are_lowercase_alpha():
    """All nouns contain only lowercase letters and hyphens."""
    for noun in NOUNS:
        assert re.fullmatch(r"[a-z]+(-[a-z]+)*", noun), f"Invalid noun: {noun}"


def test_word_list_sizes():
    """Lists are large enough for 40k+ combinations."""
    assert len(ADJECTIVES) >= 200
    assert len(NOUNS) >= 200
    assert len(ADJECTIVES) * len(NOUNS) >= 40_000


def test_no_duplicate_adjectives():
    assert len(ADJECTIVES) == len(set(ADJECTIVES))


def test_no_duplicate_nouns():
    assert len(NOUNS) == len(set(NOUNS))


def test_generate_name_format():
    """Name is adjective-noun format."""
    name = generate_name()
    parts = name.split("-")
    # At least 2 parts (adjective and noun, each may have internal hyphens
    # but the simplest case is exactly 2)
    assert len(parts) >= 2
    assert name  # non-empty


def test_generate_name_deterministic_with_seed():
    """Same seed produces same name."""
    name1 = generate_name(seed=42)
    name2 = generate_name(seed=42)
    assert name1 == name2


def test_generate_name_different_seeds():
    """Different seeds produce different names (with very high probability)."""
    name1 = generate_name(seed=1)
    name2 = generate_name(seed=2)
    assert name1 != name2


def test_generate_name_without_seed_varies():
    """Without seed, names should vary (generate 10, expect at least 2 unique)."""
    names = {generate_name() for _ in range(10)}
    assert len(names) >= 2


def test_generate_run_id_format():
    """Run ID is YYYY-MM-DD-adjective-noun."""
    run_id = generate_run_id(seed=42)
    today = date.today().isoformat()
    assert run_id.startswith(today + "-")
    # The part after the date should be a valid name
    name_part = run_id[len(today) + 1:]
    assert len(name_part) > 0


def test_generate_run_id_deterministic():
    run_id1 = generate_run_id(seed=99)
    run_id2 = generate_run_id(seed=99)
    assert run_id1 == run_id2


def test_create_run_dir_creates_directory(tmp_path):
    """create_run_dir makes the directory and returns its path."""
    run_dir = create_run_dir(tmp_path, seed=42)
    assert run_dir.exists()
    assert run_dir.is_dir()
    assert run_dir.parent == tmp_path


def test_create_run_dir_collision_appends_suffix(tmp_path):
    """If the directory already exists, append -2, -3, etc."""
    first = create_run_dir(tmp_path, seed=42)
    second = create_run_dir(tmp_path, seed=42)
    assert first != second
    assert second.name.endswith("-2")
    third = create_run_dir(tmp_path, seed=42)
    assert third.name.endswith("-3")


def test_create_run_dir_with_custom_name(tmp_path):
    """Custom run_name overrides the generated adjective-noun part."""
    run_dir = create_run_dir(tmp_path, run_name="final-take")
    today = date.today().isoformat()
    assert run_dir.name == f"{today}-final-take"


def test_create_run_dir_custom_name_collision(tmp_path):
    """Custom names also get collision suffixes."""
    first = create_run_dir(tmp_path, run_name="my-run")
    second = create_run_dir(tmp_path, run_name="my-run")
    assert second.name.endswith("-2")
```

**Step 2: Run tests to verify they fail**

Run: `pytest tests/test_names.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'glottisdale.names'`

**Step 3: Commit**

```bash
git add tests/test_names.py
git commit -m "test: add tests for run name generation"
```

---

### Task 2: Name Generation Module — Implementation

**Files:**
- Create: `src/glottisdale/names.py`

**Step 1: Implement the module**

The module needs:
- `ADJECTIVES`: list of ~200+ speech/voice/music-themed adjectives
- `NOUNS`: list of ~200+ speech/voice/music-themed nouns
- `generate_name(seed=None) -> str`: picks adjective-noun pair, deterministic if seed given
- `generate_run_id(seed=None) -> str`: returns `YYYY-MM-DD-adjective-noun`
- `create_run_dir(root: Path, seed=None, run_name=None) -> Path`: creates unique directory

```python
"""Generate unique, memorable run names for glottisdale output directories.

Names are speech/voice/music-themed adjective-noun pairs like
'breathy-bassoon' or 'staccato-tenor'. Combined with a date prefix,
they produce sortable, identifiable run IDs like '2026-02-19-breathy-bassoon'.
"""

import random
from datetime import date
from pathlib import Path

# ~200+ speech/voice/music-themed adjectives
ADJECTIVES = [
    "acoustic", "airy", "alto", "angular", "arched", "arpeggio", "atonal",
    "baritone", "bellowing", "bluesy", "booming", "bowed", "brassy", "breathy",
    "bright", "brittle", "buzzing", "cadenced", "chanting", "chesty",
    "chiming", "choral", "chromatic", "clipped", "coarse", "contralto",
    "crooning", "dark", "deft", "detached", "diaphonic", "diffuse", "digital",
    "dissonant", "distant", "droning", "dulcet", "dynamic", "echoing",
    "eerie", "elegiac", "embouchured", "ethereal", "expressive", "fading",
    "falsetto", "fervent", "fiery", "flageolet", "flat", "flowing",
    "fluent", "fluttering", "forte", "fretted", "fugal", "full", "gapped",
    "ghostly", "glassy", "gliding", "glottal", "granular", "gravelly",
    "groovy", "growling", "guttural", "harmonic", "harsh", "heady",
    "hollow", "honeyed", "hooting", "hovering", "humming", "hushed",
    "husky", "hymnal", "idling", "intoned", "jagged", "jaunty", "jazzy",
    "keen", "keening", "keyed", "lamenting", "languid", "laryngeal",
    "legato", "light", "lilting", "liquid", "lisping", "looping", "loud",
    "low", "lulling", "lyric", "major", "marcato", "mellow", "melodic",
    "mezzo", "microtonal", "minor", "modal", "modulated", "monotone",
    "moody", "morphing", "muffled", "murmuring", "muted", "nasal",
    "nimble", "nodal", "octave", "offbeat", "open", "operatic", "overtone",
    "passing", "pastoral", "pealing", "pedal", "pentatonic", "percussive",
    "phased", "piping", "pitched", "pizzicato", "plaintive", "plucked",
    "plunging", "polyphonic", "portamento", "pressed", "pulsing",
    "pure", "quavering", "quiet", "rasping", "raw", "reedy", "resonant",
    "reverbed", "riffing", "ringing", "rising", "rolling", "roomy",
    "rough", "round", "rumbling", "rushing", "rustic", "scooped",
    "scratchy", "sharp", "shimmering", "shrill", "sibilant", "sighing",
    "silken", "silvery", "singing", "slapping", "sliding", "slurred",
    "smoky", "smooth", "snapping", "soaring", "soft", "solo", "somber",
    "sonorous", "soprano", "sotto", "sparse", "spectral", "staccato",
    "strident", "strumming", "subharmonic", "surging", "sustained",
    "swaying", "swelling", "syncopated", "tempered", "tenor", "thick",
    "thin", "throbbing", "throaty", "thundering", "tonal", "trembling",
    "tremolo", "trilling", "tuned", "twangy", "unison", "unvoiced",
    "uvular", "vaporous", "velar", "velvety", "vibrant", "vibrato",
    "vocal", "voiced", "voiceless", "wailing", "warm", "warped",
    "wavering", "wheezy", "whispering", "whistling", "whooping", "winding",
    "woody", "woozy", "yearning", "zesty",
]

# ~200+ speech/voice/music-themed nouns
NOUNS = [
    "accordion", "alto", "anthem", "aria", "arpeggio", "ballad",
    "banjo", "baritone", "bass", "bassoon", "bellow", "bolero",
    "bourdon", "breath", "bridge", "bugle", "cadence", "canon",
    "cantata", "canticle", "cello", "chant", "chorale", "chord",
    "chorus", "chromatic", "clarinet", "clavichord", "clef", "coda",
    "concerto", "cornet", "counterpoint", "crescendo", "crotchet",
    "cymbal", "descant", "diapason", "diminuendo", "dirge", "dissonance",
    "ditty", "drone", "drum", "drumroll", "duet", "dulcimer",
    "echo", "elegy", "ensemble", "epiglottis", "etude", "euphonium",
    "fanfare", "fermata", "fiddle", "fife", "finale", "flute",
    "fortissimo", "fugue", "gargle", "glockenspiel", "glissando",
    "glottis", "gong", "growl", "guitar", "harmonica", "harmony",
    "harp", "harpsichord", "hiccup", "horn", "howl", "hum",
    "hymn", "interlude", "jingle", "kazoo", "kettledrum", "keynote",
    "lament", "larynx", "legato", "lilt", "lullaby", "lute",
    "lyre", "madrigal", "mandolin", "marimba", "measure", "medley",
    "melody", "metronome", "minuet", "motif", "murmur", "nocturne",
    "oboe", "octave", "opera", "opus", "organ", "overture",
    "palate", "pharynx", "phrase", "pianissimo", "piano", "piccolo",
    "pitch", "polka", "prelude", "psaltery", "quaver", "quintet",
    "rasp", "rattle", "recital", "reed", "refrain", "requiem",
    "resonance", "rest", "rhapsody", "rhythm", "riff", "rondo",
    "samba", "scale", "scherzo", "semitone", "serenade", "shanty",
    "sigh", "siren", "snare", "solo", "sonata", "soprano",
    "stanza", "strum", "symphony", "syncopation", "tabla", "tambourine",
    "tempo", "tenor", "theremin", "timpani", "toccata", "tongue",
    "treble", "tremolo", "trill", "trio", "trombone", "trumpet",
    "tuba", "tune", "tuning", "ukulele", "undertone", "unison",
    "uvula", "verse", "vibrato", "viola", "violin", "vocal",
    "voice", "vowel", "waltz", "warble", "whisper", "whistle",
    "woodwind", "xylophone", "yodel", "zither",
]


def generate_name(seed: int | None = None) -> str:
    """Generate an adjective-noun name like 'breathy-bassoon'.

    If seed is provided, the name is deterministic.
    """
    rng = random.Random(seed)
    adj = rng.choice(ADJECTIVES)
    noun = rng.choice(NOUNS)
    return f"{adj}-{noun}"


def generate_run_id(seed: int | None = None) -> str:
    """Generate a run ID like '2026-02-19-breathy-bassoon'."""
    today = date.today().isoformat()
    name = generate_name(seed)
    return f"{today}-{name}"


def create_run_dir(
    root: Path,
    seed: int | None = None,
    run_name: str | None = None,
) -> Path:
    """Create a unique run directory inside root.

    Args:
        root: Parent directory (e.g. ./glottisdale-output).
        seed: RNG seed for deterministic name generation.
        run_name: Override the adjective-noun part (date prefix still added).

    Returns:
        Path to the created run directory.
    """
    today = date.today().isoformat()
    if run_name:
        base_name = f"{today}-{run_name}"
    else:
        name = generate_name(seed)
        base_name = f"{today}-{name}"

    candidate = root / base_name
    if not candidate.exists():
        candidate.mkdir(parents=True, exist_ok=True)
        return candidate

    # Collision: append -2, -3, ...
    counter = 2
    while True:
        candidate = root / f"{base_name}-{counter}"
        if not candidate.exists():
            candidate.mkdir(parents=True, exist_ok=True)
            return candidate
        counter += 1
```

**Step 2: Run tests to verify they pass**

Run: `pytest tests/test_names.py -v`
Expected: All 16 tests PASS

**Step 3: Commit**

```bash
git add src/glottisdale/names.py
git commit -m "feat: add run name generation with thematic word lists"
```

---

### Task 3: Remove clips/ rmtree from collage pipeline

**Files:**
- Modify: `src/glottisdale/collage/__init__.py:249-253`
- Test: `tests/collage/test_cli.py` (existing tests remain valid)

**Step 1: Remove the rmtree**

In `src/glottisdale/collage/__init__.py`, change lines 249-253 from:

```python
    output_dir = Path(output_dir)
    clips_dir = output_dir / "clips"
    if clips_dir.exists():
        shutil.rmtree(clips_dir)
    clips_dir.mkdir(parents=True, exist_ok=True)
```

to:

```python
    output_dir = Path(output_dir)
    clips_dir = output_dir / "clips"
    clips_dir.mkdir(parents=True, exist_ok=True)
```

This is safe because each run now gets its own fresh directory — there's nothing to wipe.

**Step 2: Run existing tests to verify nothing breaks**

Run: `pytest tests/collage/ -v`
Expected: All existing tests PASS

**Step 3: Commit**

```bash
git add src/glottisdale/collage/__init__.py
git commit -m "refactor: remove clips/ rmtree (each run has its own dir)"
```

---

### Task 4: Wire up run directories in CLI — Tests

**Files:**
- Modify: `tests/collage/test_cli.py`

**Step 1: Add new tests**

Append to the end of `tests/collage/test_cli.py`:

```python
def test_run_name_flag_parsed():
    """--run-name flag is available on both subcommands."""
    args = parse_args(["collage", "--run-name", "my-run"])
    assert args.run_name == "my-run"


def test_run_name_flag_default_none():
    """--run-name defaults to None."""
    args = parse_args(["collage"])
    assert args.run_name is None


def test_run_name_flag_sing():
    args = parse_args(["sing", "--midi", "/tmp/midi", "--run-name", "take-1"])
    assert args.run_name == "take-1"


def test_cli_creates_run_subdir(tmp_path):
    """CLI creates a unique run subdirectory inside output-dir."""
    from glottisdale.cli import main

    input_file = tmp_path / "test.wav"
    input_file.touch()

    mock_result = MagicMock()
    mock_result.transcript = "test"
    mock_result.clips = []
    mock_result.concatenated = MagicMock()
    mock_result.concatenated.name = "concatenated.wav"

    with patch("glottisdale.collage.process") as mock_process:
        mock_process.return_value = mock_result
        main([
            "collage",
            str(input_file),
            "--output-dir", str(tmp_path / "out"),
            "--seed", "42",
        ])

        call_kwargs = mock_process.call_args[1]
        output_dir = Path(call_kwargs["output_dir"])
        # Should be a subdirectory of tmp_path/out, not tmp_path/out itself
        assert output_dir.parent == tmp_path / "out"
        # Should contain today's date
        from datetime import date
        assert date.today().isoformat() in output_dir.name


def test_cli_run_name_flag_used(tmp_path):
    """--run-name overrides auto-generated name."""
    from glottisdale.cli import main

    input_file = tmp_path / "test.wav"
    input_file.touch()

    mock_result = MagicMock()
    mock_result.transcript = "test"
    mock_result.clips = []
    mock_result.concatenated = MagicMock()
    mock_result.concatenated.name = "concatenated.wav"

    with patch("glottisdale.collage.process") as mock_process:
        mock_process.return_value = mock_result
        main([
            "collage",
            str(input_file),
            "--output-dir", str(tmp_path / "out"),
            "--run-name", "final-take",
        ])

        call_kwargs = mock_process.call_args[1]
        output_dir = Path(call_kwargs["output_dir"])
        from datetime import date
        assert output_dir.name == f"{date.today().isoformat()}-final-take"
```

**Step 2: Run new tests to verify they fail**

Run: `pytest tests/collage/test_cli.py::test_run_name_flag_parsed -v`
Expected: FAIL — `error: unrecognized arguments: --run-name`

**Step 3: Commit**

```bash
git add tests/collage/test_cli.py
git commit -m "test: add tests for --run-name flag and run subdirectories"
```

---

### Task 5: Wire up run directories in CLI — Implementation

**Files:**
- Modify: `src/glottisdale/cli.py`

**Step 1: Add `--run-name` to shared args**

In `_add_shared_args()`, after the `--no-cache` argument (line 28), add:

```python
    parser.add_argument("--run-name", default=None,
                        help="Custom run name (default: auto-generated thematic name)")
```

**Step 2: Add run directory resolution to `_run_collage()`**

In `_run_collage()`, after the input file validation (line 193) and before `result = process(...)` (line 195), add:

```python
    from glottisdale.names import create_run_dir

    output_root = Path(args.output_dir)
    run_dir = create_run_dir(output_root, seed=args.seed, run_name=args.run_name)
    print(f"Run: {run_dir.name}")
```

Then change the `process()` call to use `run_dir` instead of `args.output_dir`:

```python
    result = process(
        input_paths=input_paths,
        output_dir=run_dir,
        ...
    )
```

**Step 3: Add run directory resolution to `_run_sing()`**

In `_run_sing()`, after the input file validation (line 261) and before `output_dir = Path(args.output_dir)` (line 263), add the same pattern:

```python
    from glottisdale.names import create_run_dir

    output_root = Path(args.output_dir)
    run_dir = create_run_dir(output_root, seed=args.seed, run_name=args.run_name)
    print(f"Run: {run_dir.name}")
```

Then change line 263-264 from:

```python
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
```

to:

```python
    output_dir = run_dir
```

(The directory is already created by `create_run_dir`.)

**Step 4: Run all tests**

Run: `pytest tests/collage/test_cli.py -v`
Expected: All tests PASS (new and existing)

Note: Some existing tests (like `test_cli_passes_audio_polish_to_process`) pass `--output-dir str(tmp_path / "out")`. These will now create a run subdirectory inside that dir, so the `output_dir` kwarg passed to `process()` will be a subdirectory. The existing assertions don't check the exact value of `output_dir`, so they should still pass.

**Step 5: Commit**

```bash
git add src/glottisdale/cli.py
git commit -m "feat: wire up unique run directories in CLI for collage and sing"
```

---

### Task 6: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/getting-started/quickstart.md`
- Modify: `docs/guide/examples.md`

**Step 1: Update docs**

Update any documentation that shows output paths or directory structure to reflect the new run subdirectory behavior. Key things to update:

1. **README.md** — If it shows example output paths like `glottisdale-output/concatenated.wav`, update to `glottisdale-output/YYYY-MM-DD-name/concatenated.wav`. Document `--run-name` flag.

2. **docs/getting-started/quickstart.md** — Update the "what you get" section to show the run subdirectory structure.

3. **docs/guide/examples.md** — Update any CLI examples that reference output paths.

Read each file first, then make targeted updates only where output paths or the `--output-dir` flag are discussed.

**Step 2: Commit**

```bash
git add README.md docs/getting-started/quickstart.md docs/guide/examples.md
git commit -m "docs: update output path examples for unique run directories"
```

---

### Task 7: Run full test suite and verify

**Step 1: Run the full test suite**

Run: `pytest tests/ -v`
Expected: All tests PASS

**Step 2: Manual smoke test (if test audio files available)**

Run: `python -m glottisdale collage test-files/sample.mp4 --seed 42`
Verify: Output goes to `./glottisdale-output/YYYY-MM-DD-<name>/`

Run again with same seed:
Verify: Gets collision suffix (`-2`)

Run with `--run-name`:
`python -m glottisdale collage test-files/sample.mp4 --run-name my-test`
Verify: Output goes to `./glottisdale-output/YYYY-MM-DD-my-test/`
