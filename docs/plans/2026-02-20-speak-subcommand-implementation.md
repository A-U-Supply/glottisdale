# `glottisdale speak` Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `speak` subcommand that reconstructs target text using syllable fragments from source audio, via ARPABET phonetic-distance matching.

**Architecture:** New `src/glottisdale/speak/` module with phonetic distance matrix, syllable bank, matcher, and assembler. Reuses existing transcription/syllabification/alignment pipeline to build the source syllable bank, then matches target text syllables to source syllables by articulatory feature distance. Two input modes: `--text` (direct text) and `--reference` (audio transcribed for text + timing template).

**Tech Stack:** Python 3.11+, g2p_en (ARPABET), existing Whisper/alignment pipeline, ffmpeg/rubberband for audio manipulation.

**Design doc:** `docs/plans/2026-02-20-speak-subcommand-design.md`

---

### Task 1: Phonetic Distance Module

**Files:**
- Create: `src/glottisdale/speak/__init__.py` (empty for now)
- Create: `src/glottisdale/speak/phonetic_distance.py`
- Create: `tests/speak/__init__.py` (empty)
- Create: `tests/speak/test_phonetic_distance.py`

**Step 1: Write the failing tests**

Create `tests/speak/__init__.py` (empty) and `tests/speak/test_phonetic_distance.py`:

```python
"""Tests for ARPABET phonetic distance."""

from glottisdale.speak.phonetic_distance import (
    phoneme_distance,
    syllable_distance,
    strip_stress,
)


class TestStripStress:
    def test_strips_stress_marker(self):
        assert strip_stress("AH0") == "AH"
        assert strip_stress("IY1") == "IY"
        assert strip_stress("EH2") == "EH"

    def test_no_stress_unchanged(self):
        assert strip_stress("B") == "B"
        assert strip_stress("TH") == "TH"
        assert strip_stress("NG") == "NG"


class TestPhonemeDistance:
    def test_identical_phonemes_zero(self):
        assert phoneme_distance("B", "B") == 0
        assert phoneme_distance("AH", "AH") == 0

    def test_stress_ignored(self):
        assert phoneme_distance("AH0", "AH1") == 0
        assert phoneme_distance("IY0", "IY2") == 0

    def test_symmetry(self):
        d1 = phoneme_distance("B", "P")
        d2 = phoneme_distance("P", "B")
        assert d1 == d2

    def test_voicing_pair_small(self):
        # B/P differ only in voicing
        assert phoneme_distance("B", "P") == 1

    def test_place_pair_small(self):
        # B/D differ only in place (bilabial vs alveolar)
        assert phoneme_distance("B", "D") == 1

    def test_manner_place_voicing_large(self):
        # B (voiced bilabial stop) vs S (voiceless alveolar fricative)
        assert phoneme_distance("B", "S") >= 3

    def test_vowel_height_pair(self):
        # AH (mid central) vs AE (low front) — differ in height + backness
        d = phoneme_distance("AH", "AE")
        assert d >= 1

    def test_vowel_vs_consonant_max(self):
        # Vowels and consonants are maximally different
        d = phoneme_distance("AH", "B")
        assert d >= 4

    def test_all_phonemes_have_features(self):
        """Every ARPABET consonant and vowel base should be in the matrix."""
        from glottisdale.speak.phonetic_distance import FEATURES
        consonants = [
            "B", "P", "T", "D", "K", "G", "F", "V",
            "TH", "DH", "S", "Z", "SH", "ZH", "HH",
            "CH", "JH", "M", "N", "NG", "L", "R", "W", "Y",
        ]
        vowels = [
            "IY", "IH", "EY", "EH", "AE", "AA", "AH",
            "AO", "OW", "UH", "UW", "AW", "AY", "OY", "ER",
        ]
        for p in consonants + vowels:
            assert p in FEATURES, f"Missing features for {p}"


class TestSyllableDistance:
    def test_identical_syllables_zero(self):
        assert syllable_distance(["B", "AH1"], ["B", "AH0"]) == 0

    def test_one_phoneme_diff(self):
        # "BA" vs "PA" — only onset differs by voicing
        d = syllable_distance(["B", "AH1"], ["P", "AH1"])
        assert d == 1

    def test_different_lengths_aligned(self):
        # Shorter syllable padded — should still produce a finite distance
        d = syllable_distance(["B", "AH1"], ["S", "T", "AH1"])
        assert d > 0
        assert d < 100  # not infinite

    def test_empty_syllable(self):
        d = syllable_distance([], ["B", "AH1"])
        assert d > 0
```

**Step 2: Run tests to verify they fail**

Run: `python -m pytest tests/speak/test_phonetic_distance.py -v`
Expected: FAIL with import errors (module doesn't exist yet)

**Step 3: Write the implementation**

Create `src/glottisdale/speak/__init__.py` (empty) and `src/glottisdale/speak/phonetic_distance.py`:

```python
"""ARPABET phonetic feature matrix and distance calculations."""

# Articulatory features for each ARPABET phoneme (stress-stripped).
# Consonants: (type, manner, place, voicing)
# Vowels: (type, height, backness, roundness, tenseness)
# "type" distinguishes consonants from vowels for max-distance fallback.

FEATURES: dict[str, tuple[str, ...]] = {
    # Consonants — (type, manner, place, voicing)
    "P":  ("consonant", "stop",      "bilabial",      "voiceless"),
    "B":  ("consonant", "stop",      "bilabial",      "voiced"),
    "T":  ("consonant", "stop",      "alveolar",      "voiceless"),
    "D":  ("consonant", "stop",      "alveolar",      "voiced"),
    "K":  ("consonant", "stop",      "velar",         "voiceless"),
    "G":  ("consonant", "stop",      "velar",         "voiced"),
    "F":  ("consonant", "fricative", "labiodental",   "voiceless"),
    "V":  ("consonant", "fricative", "labiodental",   "voiced"),
    "TH": ("consonant", "fricative", "dental",        "voiceless"),
    "DH": ("consonant", "fricative", "dental",        "voiced"),
    "S":  ("consonant", "fricative", "alveolar",      "voiceless"),
    "Z":  ("consonant", "fricative", "alveolar",      "voiced"),
    "SH": ("consonant", "fricative", "postalveolar",  "voiceless"),
    "ZH": ("consonant", "fricative", "postalveolar",  "voiced"),
    "HH": ("consonant", "fricative", "glottal",       "voiceless"),
    "CH": ("consonant", "affricate", "postalveolar",  "voiceless"),
    "JH": ("consonant", "affricate", "postalveolar",  "voiced"),
    "M":  ("consonant", "nasal",     "bilabial",      "voiced"),
    "N":  ("consonant", "nasal",     "alveolar",      "voiced"),
    "NG": ("consonant", "nasal",     "velar",         "voiced"),
    "L":  ("consonant", "liquid",    "alveolar",      "voiced"),
    "R":  ("consonant", "liquid",    "postalveolar",  "voiced"),
    "W":  ("consonant", "glide",     "bilabial",      "voiced"),
    "Y":  ("consonant", "glide",     "palatal",       "voiced"),
    # Vowels — (type, height, backness, roundness, tenseness)
    "IY": ("vowel", "high",  "front",   "unrounded", "tense"),
    "IH": ("vowel", "high",  "front",   "unrounded", "lax"),
    "EY": ("vowel", "mid",   "front",   "unrounded", "tense"),
    "EH": ("vowel", "mid",   "front",   "unrounded", "lax"),
    "AE": ("vowel", "low",   "front",   "unrounded", "lax"),
    "AA": ("vowel", "low",   "back",    "unrounded", "tense"),
    "AH": ("vowel", "mid",   "central", "unrounded", "lax"),
    "AO": ("vowel", "mid",   "back",    "rounded",   "tense"),
    "OW": ("vowel", "mid",   "back",    "rounded",   "tense"),
    "UH": ("vowel", "high",  "back",    "rounded",   "lax"),
    "UW": ("vowel", "high",  "back",    "rounded",   "tense"),
    "AW": ("vowel", "low",   "central", "unrounded", "tense"),
    "AY": ("vowel", "low",   "central", "unrounded", "tense"),
    "OY": ("vowel", "mid",   "back",    "rounded",   "tense"),
    "ER": ("vowel", "mid",   "central", "rounded",   "tense"),
}

# Max distance when comparing a consonant to a vowel (different type)
_CROSS_TYPE_DISTANCE = 5


def strip_stress(phoneme: str) -> str:
    """Remove trailing stress marker (0, 1, 2) from an ARPABET phoneme."""
    if phoneme and phoneme[-1] in "012":
        return phoneme[:-1]
    return phoneme


def phoneme_distance(a: str, b: str) -> int:
    """Compute articulatory feature distance between two ARPABET phonemes.

    Stress markers are ignored. Returns 0 for identical phonemes,
    higher values for more dissimilar phonemes.
    """
    a_base = strip_stress(a)
    b_base = strip_stress(b)

    if a_base == b_base:
        return 0

    feat_a = FEATURES.get(a_base)
    feat_b = FEATURES.get(b_base)

    if feat_a is None or feat_b is None:
        return _CROSS_TYPE_DISTANCE

    # Different type (consonant vs vowel) = max distance
    if feat_a[0] != feat_b[0]:
        return _CROSS_TYPE_DISTANCE

    # Count differing features (skip index 0 which is the type tag)
    return sum(1 for fa, fb in zip(feat_a[1:], feat_b[1:]) if fa != fb)


def syllable_distance(a_phonemes: list[str], b_phonemes: list[str]) -> int:
    """Compute distance between two syllables (lists of ARPABET phonemes).

    Aligns by padding the shorter list with None, which incurs max distance
    per missing phoneme.
    """
    len_a = len(a_phonemes)
    len_b = len(b_phonemes)
    max_len = max(len_a, len_b)

    if max_len == 0:
        return 0

    total = 0
    for i in range(max_len):
        pa = a_phonemes[i] if i < len_a else None
        pb = b_phonemes[i] if i < len_b else None
        if pa is None or pb is None:
            total += _CROSS_TYPE_DISTANCE
        else:
            total += phoneme_distance(pa, pb)
    return total
```

**Step 4: Run tests to verify they pass**

Run: `python -m pytest tests/speak/test_phonetic_distance.py -v`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/glottisdale/speak/__init__.py src/glottisdale/speak/phonetic_distance.py tests/speak/__init__.py tests/speak/test_phonetic_distance.py
git commit -m "feat(speak): add ARPABET phonetic distance module"
```

---

### Task 2: Syllable Bank

**Files:**
- Create: `src/glottisdale/speak/syllable_bank.py`
- Create: `tests/speak/test_syllable_bank.py`

**Step 1: Write the failing tests**

Create `tests/speak/test_syllable_bank.py`:

```python
"""Tests for syllable bank construction."""

from glottisdale.speak.syllable_bank import SyllableEntry, build_bank
from glottisdale.types import Phoneme, Syllable


def _make_syllable(phoneme_labels: list[str], start: float, end: float,
                   word: str = "test", word_index: int = 0) -> Syllable:
    """Helper to create a Syllable with Phoneme objects."""
    n = len(phoneme_labels)
    dur = (end - start) / n if n else 0
    phonemes = [
        Phoneme(label=lab, start=start + i * dur, end=start + (i + 1) * dur)
        for i, lab in enumerate(phoneme_labels)
    ]
    return Syllable(phonemes=phonemes, start=start, end=end,
                    word=word, word_index=word_index)


class TestBuildBank:
    def test_builds_entries_from_syllables(self):
        syls = [
            _make_syllable(["B", "AH1"], 0.0, 0.3, word="but"),
            _make_syllable(["K", "AE1", "T"], 0.3, 0.7, word="cat"),
        ]
        bank = build_bank(syls, source_path="test.wav")
        assert len(bank) == 2
        assert bank[0].phoneme_labels == ["B", "AH1"]
        assert bank[0].source_path == "test.wav"
        assert bank[1].phoneme_labels == ["K", "AE1", "T"]

    def test_entry_has_timing(self):
        syls = [_make_syllable(["B", "AH1"], 1.5, 2.0)]
        bank = build_bank(syls, source_path="test.wav")
        assert bank[0].start == 1.5
        assert bank[0].end == 2.0

    def test_entry_has_stress(self):
        syls = [_make_syllable(["B", "AH1"], 0.0, 0.3)]
        bank = build_bank(syls, source_path="test.wav")
        assert bank[0].stress == 1

    def test_entry_stress_none_for_no_vowel(self):
        """Consonant-only syllable (edge case) has no stress."""
        syls = [_make_syllable(["S", "T"], 0.0, 0.2)]
        bank = build_bank(syls, source_path="test.wav")
        assert bank[0].stress is None

    def test_empty_syllables(self):
        bank = build_bank([], source_path="test.wav")
        assert bank == []

    def test_bank_to_json(self):
        """Bank entries serialize to JSON-friendly dicts."""
        syls = [_make_syllable(["B", "AH1"], 0.0, 0.3, word="but")]
        bank = build_bank(syls, source_path="test.wav")
        d = bank[0].to_dict()
        assert d["phonemes"] == ["B", "AH1"]
        assert d["word"] == "but"
        assert d["source"] == "test.wav"
```

**Step 2: Run tests to verify they fail**

Run: `python -m pytest tests/speak/test_syllable_bank.py -v`
Expected: FAIL with import errors

**Step 3: Write the implementation**

Create `src/glottisdale/speak/syllable_bank.py`:

```python
"""Build an indexed bank of source syllables for matching."""

from dataclasses import dataclass
from glottisdale.types import Syllable


@dataclass
class SyllableEntry:
    """A source syllable with metadata for matching."""
    phoneme_labels: list[str]   # ARPABET labels (with stress)
    start: float                # seconds in source audio
    end: float                  # seconds in source audio
    word: str                   # parent word
    stress: int | None          # stress level (0, 1, 2) or None
    source_path: str            # source audio file path
    index: int                  # position in bank

    @property
    def duration(self) -> float:
        return self.end - self.start

    def to_dict(self) -> dict:
        """Serialize for JSON output."""
        return {
            "phonemes": self.phoneme_labels,
            "start": round(self.start, 4),
            "end": round(self.end, 4),
            "duration": round(self.duration, 4),
            "word": self.word,
            "stress": self.stress,
            "source": self.source_path,
            "index": self.index,
        }


def _extract_stress(phoneme_labels: list[str]) -> int | None:
    """Extract stress level from ARPABET vowel phonemes."""
    for label in phoneme_labels:
        if label and label[-1] in "012":
            return int(label[-1])
    return None


def build_bank(syllables: list[Syllable], source_path: str) -> list[SyllableEntry]:
    """Build a syllable bank from aligned source syllables."""
    entries = []
    for i, syl in enumerate(syllables):
        labels = [p.label for p in syl.phonemes]
        entries.append(SyllableEntry(
            phoneme_labels=labels,
            start=syl.start,
            end=syl.end,
            word=syl.word,
            stress=_extract_stress(labels),
            source_path=source_path,
            index=i,
        ))
    return entries
```

**Step 4: Run tests to verify they pass**

Run: `python -m pytest tests/speak/test_syllable_bank.py -v`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/glottisdale/speak/syllable_bank.py tests/speak/test_syllable_bank.py
git commit -m "feat(speak): add syllable bank builder"
```

---

### Task 3: Matcher

**Files:**
- Create: `src/glottisdale/speak/matcher.py`
- Create: `tests/speak/test_matcher.py`

**Step 1: Write the failing tests**

Create `tests/speak/test_matcher.py`:

```python
"""Tests for syllable/phoneme matching."""

from glottisdale.speak.matcher import match_syllables, match_phonemes, MatchResult
from glottisdale.speak.syllable_bank import SyllableEntry


def _entry(phonemes: list[str], index: int = 0, stress: int | None = 1,
           start: float = 0.0, end: float = 0.3) -> SyllableEntry:
    """Helper to create a SyllableEntry."""
    return SyllableEntry(
        phoneme_labels=phonemes, start=start, end=end,
        word="test", stress=stress, source_path="test.wav", index=index,
    )


class TestMatchSyllables:
    def test_exact_match(self):
        bank = [_entry(["B", "AH1"], index=0)]
        target = [["B", "AH1"]]
        results = match_syllables(target, bank)
        assert len(results) == 1
        assert results[0].entry.index == 0
        assert results[0].distance == 0

    def test_best_match_chosen(self):
        bank = [
            _entry(["S", "AH1"], index=0),   # worse match for "B AH"
            _entry(["B", "AH1"], index=1),    # exact match
            _entry(["P", "AH1"], index=2),    # close match (voicing diff)
        ]
        target = [["B", "AH1"]]
        results = match_syllables(target, bank)
        assert results[0].entry.index == 1
        assert results[0].distance == 0

    def test_tie_broken_by_stress(self):
        """When distances tie, prefer matching stress level."""
        bank = [
            _entry(["B", "AH0"], index=0, stress=0),  # unstressed
            _entry(["B", "AH1"], index=1, stress=1),  # stressed
        ]
        # Target wants stress=1, both are distance 0 (stress ignored in distance)
        target = [["B", "AH1"]]
        target_stresses = [1]
        results = match_syllables(target, bank, target_stresses=target_stresses)
        assert results[0].entry.index == 1

    def test_multiple_targets(self):
        bank = [
            _entry(["B", "AH1"], index=0),
            _entry(["K", "AE1", "T"], index=1),
        ]
        target = [["B", "AH1"], ["K", "AE1", "T"]]
        results = match_syllables(target, bank)
        assert len(results) == 2
        assert results[0].distance == 0
        assert results[1].distance == 0

    def test_always_returns_best_available(self):
        """Even poor matches are returned (no filtering)."""
        bank = [_entry(["B", "AH1"], index=0)]
        target = [["SH", "IY1"]]  # very different
        results = match_syllables(target, bank)
        assert len(results) == 1
        assert results[0].distance > 0


class TestMatchPhonemes:
    def test_exact_phoneme_match(self):
        bank = [
            _entry(["B", "AH1"], index=0),
            _entry(["K", "AE1"], index=1),
        ]
        target_phonemes = ["B"]
        results = match_phonemes(target_phonemes, bank)
        assert len(results) == 1
        assert results[0].distance == 0

    def test_multiple_phonemes(self):
        bank = [
            _entry(["B", "AH1"], index=0),
            _entry(["K", "AE1", "T"], index=1),
        ]
        target_phonemes = ["K", "AE1", "T"]
        results = match_phonemes(target_phonemes, bank)
        assert len(results) == 3


class TestMatchResult:
    def test_to_dict(self):
        entry = _entry(["B", "AH1"], index=0)
        result = MatchResult(
            target_phonemes=["B", "AH1"],
            entry=entry,
            distance=0,
            target_index=0,
        )
        d = result.to_dict()
        assert d["target"] == ["B", "AH1"]
        assert d["matched"] == ["B", "AH1"]
        assert d["distance"] == 0
```

**Step 2: Run tests to verify they fail**

Run: `python -m pytest tests/speak/test_matcher.py -v`
Expected: FAIL with import errors

**Step 3: Write the implementation**

Create `src/glottisdale/speak/matcher.py`:

```python
"""Match target syllables/phonemes to a source syllable bank."""

from dataclasses import dataclass

from glottisdale.speak.phonetic_distance import (
    phoneme_distance,
    strip_stress,
    syllable_distance,
)
from glottisdale.speak.syllable_bank import SyllableEntry


@dataclass
class MatchResult:
    """Result of matching a target syllable/phoneme to a source entry."""
    target_phonemes: list[str]
    entry: SyllableEntry
    distance: int
    target_index: int

    def to_dict(self) -> dict:
        return {
            "target_index": self.target_index,
            "target": self.target_phonemes,
            "matched": self.entry.phoneme_labels,
            "matched_word": self.entry.word,
            "source_index": self.entry.index,
            "distance": self.distance,
        }


def match_syllables(
    target_syllables: list[list[str]],
    bank: list[SyllableEntry],
    target_stresses: list[int | None] | None = None,
) -> list[MatchResult]:
    """Match each target syllable to the best source syllable in the bank.

    Args:
        target_syllables: List of syllables, each a list of ARPABET phonemes.
        bank: Source syllable bank to search.
        target_stresses: Optional stress levels for tie-breaking.

    Returns:
        One MatchResult per target syllable.
    """
    results = []
    for i, target in enumerate(target_syllables):
        target_stress = (
            target_stresses[i] if target_stresses and i < len(target_stresses)
            else None
        )
        best = _find_best(target, bank, target_stress)
        results.append(MatchResult(
            target_phonemes=target,
            entry=best[0],
            distance=best[1],
            target_index=i,
        ))
    return results


def match_phonemes(
    target_phonemes: list[str],
    bank: list[SyllableEntry],
) -> list[MatchResult]:
    """Match each target phoneme to the best source phoneme.

    Searches all phonemes across all bank entries to find the closest
    individual phoneme match.
    """
    # Flatten bank into (phoneme_label, entry, phoneme_index) tuples
    flat: list[tuple[str, SyllableEntry, int]] = []
    for entry in bank:
        for pi, label in enumerate(entry.phoneme_labels):
            flat.append((label, entry, pi))

    results = []
    for i, target_ph in enumerate(target_phonemes):
        best_entry = None
        best_dist = float("inf")
        for label, entry, _pi in flat:
            d = phoneme_distance(target_ph, label)
            if d < best_dist:
                best_dist = d
                best_entry = entry
            if d == 0:
                break  # exact match, can't do better
        results.append(MatchResult(
            target_phonemes=[target_ph],
            entry=best_entry,
            distance=best_dist,
            target_index=i,
        ))
    return results


def _find_best(
    target: list[str],
    bank: list[SyllableEntry],
    target_stress: int | None,
) -> tuple[SyllableEntry, int]:
    """Find the best matching bank entry for a target syllable."""
    best_entry = bank[0]
    best_dist = syllable_distance(target, bank[0].phoneme_labels)
    best_stress_match = (bank[0].stress == target_stress) if target_stress is not None else True

    for entry in bank[1:]:
        d = syllable_distance(target, entry.phoneme_labels)
        stress_match = (entry.stress == target_stress) if target_stress is not None else True

        # Prefer lower distance; break ties by stress match, then index
        if d < best_dist or (d == best_dist and stress_match and not best_stress_match):
            best_entry = entry
            best_dist = d
            best_stress_match = stress_match

    return best_entry, best_dist
```

**Step 4: Run tests to verify they pass**

Run: `python -m pytest tests/speak/test_matcher.py -v`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/glottisdale/speak/matcher.py tests/speak/test_matcher.py
git commit -m "feat(speak): add syllable and phoneme matcher"
```

---

### Task 4: Assembler

**Files:**
- Create: `src/glottisdale/speak/assembler.py`
- Create: `tests/speak/test_assembler.py`

**Step 1: Write the failing tests**

Create `tests/speak/test_assembler.py`:

```python
"""Tests for audio assembly from matched syllables."""

from pathlib import Path
from unittest.mock import patch, MagicMock

from glottisdale.speak.assembler import (
    plan_timing,
    assemble,
    TimingPlan,
)
from glottisdale.speak.matcher import MatchResult
from glottisdale.speak.syllable_bank import SyllableEntry


def _entry(phonemes: list[str], start: float = 0.0, end: float = 0.3,
           index: int = 0) -> SyllableEntry:
    return SyllableEntry(
        phoneme_labels=phonemes, start=start, end=end,
        word="test", stress=1, source_path="test.wav", index=index,
    )


def _match(target: list[str], entry: SyllableEntry, distance: int = 0,
           target_index: int = 0) -> MatchResult:
    return MatchResult(
        target_phonemes=target, entry=entry,
        distance=distance, target_index=target_index,
    )


class TestPlanTiming:
    def test_text_mode_uniform_spacing(self):
        """Without reference timing, syllables are spaced uniformly."""
        matches = [
            _match(["B", "AH1"], _entry(["B", "AH1"], start=0.0, end=0.3)),
            _match(["K", "AE1"], _entry(["K", "AE1"], start=0.5, end=0.8)),
        ]
        word_boundaries = [0, 2]  # one word with 2 syllables
        plan = plan_timing(matches, word_boundaries, avg_syllable_dur=0.25)
        assert len(plan) == 2
        # First syllable starts near 0
        assert plan[0].target_start >= 0.0
        # Second follows after first
        assert plan[1].target_start > plan[0].target_start

    def test_word_boundary_adds_pause(self):
        """Pauses inserted at word boundaries."""
        matches = [
            _match(["B", "AH1"], _entry(["B", "AH1"]), target_index=0),
            _match(["K", "AE1"], _entry(["K", "AE1"]), target_index=1),
        ]
        word_boundaries = [0, 1]  # each syllable is its own word
        plan = plan_timing(matches, word_boundaries, avg_syllable_dur=0.25)
        gap = plan[1].target_start - (plan[0].target_start + plan[0].target_duration)
        assert gap > 0  # there should be a pause between words

    def test_reference_timing_strictness_1(self):
        """With strictness=1.0, output timing matches reference exactly."""
        matches = [_match(["B", "AH1"], _entry(["B", "AH1"]))]
        word_boundaries = [0]
        ref_timings = [(0.5, 0.8)]  # reference says syllable at 0.5–0.8
        plan = plan_timing(
            matches, word_boundaries,
            reference_timings=ref_timings, timing_strictness=1.0,
        )
        assert abs(plan[0].target_start - 0.5) < 0.01
        assert abs(plan[0].target_duration - 0.3) < 0.01


class TestAssemble:
    @patch("glottisdale.speak.assembler.cut_clip")
    @patch("glottisdale.speak.assembler.concatenate_clips")
    def test_assemble_produces_output(self, mock_concat, mock_cut, tmp_path):
        """Assembly cuts clips and concatenates them."""
        entry = _entry(["B", "AH1"], start=0.0, end=0.3)
        matches = [_match(["B", "AH1"], entry)]
        timing = [TimingPlan(target_start=0.0, target_duration=0.3, stretch_factor=1.0)]

        mock_cut.return_value = tmp_path / "clip_0.wav"
        mock_concat.return_value = tmp_path / "speak.wav"

        result = assemble(
            matches=matches,
            timing=timing,
            output_dir=tmp_path,
            crossfade_ms=10,
        )
        assert mock_cut.called
        assert mock_concat.called
        assert result == tmp_path / "speak.wav"
```

**Step 2: Run tests to verify they fail**

Run: `python -m pytest tests/speak/test_assembler.py -v`
Expected: FAIL with import errors

**Step 3: Write the implementation**

Create `src/glottisdale/speak/assembler.py`:

```python
"""Assemble matched syllables into output audio."""

from dataclasses import dataclass
from pathlib import Path

from glottisdale.audio import (
    cut_clip,
    concatenate_clips,
    time_stretch_clip,
    pitch_shift_clip,
    generate_silence,
)
from glottisdale.speak.matcher import MatchResult


# Pause durations in seconds
_WORD_PAUSE_S = 0.12
_PUNCT_PAUSE_S = 0.35


@dataclass
class TimingPlan:
    """Timing for a single output syllable."""
    target_start: float      # desired start time in output
    target_duration: float   # desired duration in output
    stretch_factor: float    # time-stretch factor to apply (1.0 = no stretch)


def plan_timing(
    matches: list[MatchResult],
    word_boundaries: list[int],
    avg_syllable_dur: float = 0.25,
    reference_timings: list[tuple[float, float]] | None = None,
    timing_strictness: float = 0.8,
) -> list[TimingPlan]:
    """Plan output timing for matched syllables.

    Args:
        matches: Matched syllables in target order.
        word_boundaries: Indices into matches where new words start.
        avg_syllable_dur: Average syllable duration from source (for text mode).
        reference_timings: Optional (start, end) pairs from reference audio.
        timing_strictness: 0.0–1.0, how tightly to follow reference timing.
    """
    word_starts = set(word_boundaries)
    plans = []
    cursor = 0.0

    for i, match in enumerate(matches):
        source_dur = match.entry.end - match.entry.start

        if reference_timings and i < len(reference_timings):
            ref_start, ref_end = reference_timings[i]
            ref_dur = ref_end - ref_start
            # Blend between source duration and reference duration
            target_dur = source_dur + timing_strictness * (ref_dur - source_dur)
            target_start = cursor + timing_strictness * (ref_start - cursor)
        else:
            target_dur = source_dur if source_dur > 0 else avg_syllable_dur
            target_start = cursor

        # Add word-boundary pause
        if i in word_starts and i > 0:
            target_start += _WORD_PAUSE_S

        stretch = target_dur / source_dur if source_dur > 0 else 1.0

        plans.append(TimingPlan(
            target_start=target_start,
            target_duration=target_dur,
            stretch_factor=stretch,
        ))
        cursor = target_start + target_dur

    return plans


def assemble(
    matches: list[MatchResult],
    timing: list[TimingPlan],
    output_dir: Path,
    crossfade_ms: float = 10,
    pitch_shifts: list[float] | None = None,
) -> Path:
    """Cut, stretch, and concatenate matched syllables into output audio.

    Args:
        matches: Matched syllables in target order.
        timing: Timing plan for each syllable.
        output_dir: Directory for intermediate and output files.
        crossfade_ms: Crossfade between syllables in ms.
        pitch_shifts: Optional per-syllable pitch shift in semitones.

    Returns:
        Path to the assembled output WAV.
    """
    clips_dir = output_dir / "clips"
    clips_dir.mkdir(parents=True, exist_ok=True)

    clip_paths: list[Path] = []
    gap_durations: list[float] = []

    for i, (match, plan) in enumerate(zip(matches, timing)):
        # Cut source syllable
        clip_path = clips_dir / f"clip_{i:04d}.wav"
        cut_clip(
            input_path=Path(match.entry.source_path),
            output_path=clip_path,
            start=match.entry.start,
            end=match.entry.end,
            padding_ms=5,
            fade_ms=3,
        )

        # Time-stretch if needed
        if abs(plan.stretch_factor - 1.0) > 0.05:
            stretched = clips_dir / f"clip_{i:04d}_stretched.wav"
            time_stretch_clip(clip_path, stretched, plan.stretch_factor)
            clip_path = stretched

        # Pitch-shift if requested
        if pitch_shifts and i < len(pitch_shifts) and abs(pitch_shifts[i]) > 0.1:
            shifted = clips_dir / f"clip_{i:04d}_pitched.wav"
            pitch_shift_clip(clip_path, shifted, pitch_shifts[i])
            clip_path = shifted

        clip_paths.append(clip_path)

        # Compute gap to next syllable
        if i < len(timing) - 1:
            gap = timing[i + 1].target_start - (plan.target_start + plan.target_duration)
            gap_durations.append(max(0.0, gap) * 1000)  # convert to ms

    # Concatenate all clips
    output_path = output_dir / "speak.wav"
    concatenate_clips(
        clip_paths,
        output_path,
        crossfade_ms=crossfade_ms,
        gap_durations_ms=gap_durations if gap_durations else None,
    )

    return output_path
```

**Step 4: Run tests to verify they pass**

Run: `python -m pytest tests/speak/test_assembler.py -v`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/glottisdale/speak/assembler.py tests/speak/test_assembler.py
git commit -m "feat(speak): add audio assembler with timing planner"
```

---

### Task 5: Target Text Syllabification

This reuses the existing `syllabify.py` but needs a function that takes raw text (not Whisper word dicts) and produces ARPABET syllable lists.

**Files:**
- Create: `src/glottisdale/speak/target_text.py`
- Create: `tests/speak/test_target_text.py`

**Step 1: Write the failing tests**

Create `tests/speak/test_target_text.py`:

```python
"""Tests for target text to ARPABET syllable conversion."""

from glottisdale.speak.target_text import text_to_syllables, TextSyllable


class TestTextToSyllables:
    def test_single_word(self):
        result = text_to_syllables("cat")
        assert len(result) >= 1
        # "cat" → K AE T → one syllable
        assert result[0].phonemes  # has phonemes
        assert result[0].word == "cat"

    def test_multi_word(self):
        result = text_to_syllables("hello world")
        # "hello" = 2 syllables, "world" = 1 syllable
        assert len(result) >= 3
        hello_syls = [s for s in result if s.word == "hello"]
        world_syls = [s for s in result if s.word == "world"]
        assert len(hello_syls) == 2
        assert len(world_syls) >= 1

    def test_returns_arpabet_phonemes(self):
        result = text_to_syllables("bat")
        phonemes = result[0].phonemes
        # Should be ARPABET: B AE1 T
        assert "B" in phonemes or any("B" in p for p in phonemes)

    def test_word_boundaries(self):
        result = text_to_syllables("the cat sat")
        boundaries = word_boundaries_from_syllables(result)
        # "the"=1syl, "cat"=1syl, "sat"=1syl → boundaries at [0, 1, 2]
        assert boundaries == [0, 1, 2]

    def test_stress_extraction(self):
        result = text_to_syllables("hello")
        stresses = [s.stress for s in result]
        assert any(s is not None for s in stresses)

    def test_empty_string(self):
        result = text_to_syllables("")
        assert result == []

    def test_punctuation_stripped(self):
        result = text_to_syllables("hello, world!")
        words = {s.word for s in result}
        assert "hello" in words or "hello," in words  # either is acceptable
        assert len(result) >= 3


from glottisdale.speak.target_text import word_boundaries_from_syllables
```

**Step 2: Run tests to verify they fail**

Run: `python -m pytest tests/speak/test_target_text.py -v`
Expected: FAIL with import errors

**Step 3: Write the implementation**

Create `src/glottisdale/speak/target_text.py`:

```python
"""Convert target text to ARPABET syllables for matching."""

from dataclasses import dataclass

from g2p_en import G2p

from glottisdale.collage.syllabify_arpabet import syllabify as arpabet_syllabify

_g2p = None


def _get_g2p() -> G2p:
    global _g2p
    if _g2p is None:
        _g2p = G2p()
    return _g2p


@dataclass
class TextSyllable:
    """A syllable derived from target text (no audio timing)."""
    phonemes: list[str]    # ARPABET phonemes (with stress markers)
    word: str              # parent word
    word_index: int        # position of word in text
    stress: int | None     # stress level (0, 1, 2) or None


def _extract_stress(phonemes: list[str]) -> int | None:
    for p in phonemes:
        if p and p[-1] in "012":
            return int(p[-1])
    return None


def text_to_syllables(text: str) -> list[TextSyllable]:
    """Convert raw text to a list of ARPABET syllables.

    Uses g2p_en for grapheme-to-phoneme conversion, then the ARPABET
    syllabifier to split into syllables.
    """
    if not text.strip():
        return []

    g2p = _get_g2p()
    words = text.strip().split()
    result: list[TextSyllable] = []

    for wi, word in enumerate(words):
        # g2p_en returns phonemes; filter out spaces
        raw = g2p(word)
        phonemes = [p for p in raw if p.strip() and p != " "]

        if not phonemes:
            continue

        try:
            syl_tuples = arpabet_syllabify(phonemes)
        except (ValueError, KeyError):
            syl_tuples = [([], phonemes, [])]

        if not syl_tuples:
            syl_tuples = [([], phonemes, [])]

        for onset, nucleus, coda in syl_tuples:
            syl_phonemes = onset + nucleus + coda
            result.append(TextSyllable(
                phonemes=syl_phonemes,
                word=word.strip(".,!?;:\"'()-"),
                word_index=wi,
                stress=_extract_stress(syl_phonemes),
            ))

    return result


def word_boundaries_from_syllables(syllables: list[TextSyllable]) -> list[int]:
    """Return indices where new words begin."""
    boundaries = []
    last_word_index = -1
    for i, syl in enumerate(syllables):
        if syl.word_index != last_word_index:
            boundaries.append(i)
            last_word_index = syl.word_index
    return boundaries
```

**Step 4: Run tests to verify they pass**

Run: `python -m pytest tests/speak/test_target_text.py -v`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/glottisdale/speak/target_text.py tests/speak/test_target_text.py
git commit -m "feat(speak): add target text to ARPABET syllable conversion"
```

---

### Task 6: Process Orchestrator (Text Mode)

**Files:**
- Modify: `src/glottisdale/speak/__init__.py`
- Create: `tests/speak/test_integration.py`

**Step 1: Write the failing integration test**

Create `tests/speak/test_integration.py`:

```python
"""Integration tests for the speak pipeline."""

import json
from pathlib import Path
from unittest.mock import patch, MagicMock

from glottisdale.types import Phoneme, Syllable


def _make_syllables() -> list[Syllable]:
    """Create mock source syllables."""
    return [
        Syllable(
            phonemes=[Phoneme("DH", 0.0, 0.1), Phoneme("AH0", 0.1, 0.2)],
            start=0.0, end=0.2, word="the", word_index=0,
        ),
        Syllable(
            phonemes=[Phoneme("K", 0.2, 0.3), Phoneme("AE1", 0.3, 0.45), Phoneme("T", 0.45, 0.5)],
            start=0.2, end=0.5, word="cat", word_index=1,
        ),
        Syllable(
            phonemes=[Phoneme("S", 0.5, 0.6), Phoneme("AE1", 0.6, 0.7), Phoneme("T", 0.7, 0.8)],
            start=0.5, end=0.8, word="sat", word_index=2,
        ),
        Syllable(
            phonemes=[Phoneme("B", 0.8, 0.9), Phoneme("AH1", 0.9, 1.0), Phoneme("T", 1.0, 1.1)],
            start=0.8, end=1.1, word="but", word_index=3,
        ),
    ]


class TestSpeakProcess:
    @patch("glottisdale.speak.concatenate_clips")
    @patch("glottisdale.speak.cut_clip")
    @patch("glottisdale.speak.get_aligner")
    @patch("glottisdale.speak.extract_audio")
    def test_text_mode_end_to_end(
        self, mock_extract, mock_aligner, mock_cut, mock_concat, tmp_path
    ):
        from glottisdale.speak import process

        # Set up mocks
        source = tmp_path / "source.wav"
        source.touch()
        audio_path = tmp_path / "extracted.wav"
        audio_path.touch()
        mock_extract.return_value = audio_path

        aligner = MagicMock()
        aligner.process.return_value = {
            "text": "the cat sat but",
            "syllables": _make_syllables(),
        }
        mock_aligner.return_value = aligner

        mock_cut.side_effect = lambda inp, out, **kw: (out.touch() or out)
        mock_concat.side_effect = lambda paths, out, **kw: (out.touch() or out)

        result = process(
            input_paths=[source],
            output_dir=tmp_path / "out",
            text="the cat",
            whisper_model="tiny",
        )

        assert result.concatenated.exists()
        # Check match log was written
        match_log = tmp_path / "out" / "match-log.json"
        assert match_log.exists()
        log_data = json.loads(match_log.read_text())
        assert len(log_data["matches"]) > 0

        # Check syllable bank was written
        bank_file = tmp_path / "out" / "syllable-bank.json"
        assert bank_file.exists()
```

**Step 2: Run tests to verify they fail**

Run: `python -m pytest tests/speak/test_integration.py -v`
Expected: FAIL with import errors

**Step 3: Write the implementation**

Write `src/glottisdale/speak/__init__.py`:

```python
"""Speak pipeline: reconstruct target text using source audio syllables."""

import json
import logging
from pathlib import Path

from glottisdale.audio import (
    cut_clip,
    concatenate_clips,
    extract_audio,
)
from glottisdale.collage.align import get_aligner
from glottisdale.types import Result

logger = logging.getLogger(__name__)


def process(
    input_paths: list[Path],
    output_dir: str | Path,
    text: str | None = None,
    reference: Path | None = None,
    match_unit: str = "syllable",
    pitch_correct: bool = True,
    timing_strictness: float = 0.8,
    crossfade_ms: float = 10,
    normalize_volume: bool = True,
    whisper_model: str = "base",
    aligner: str = "auto",
    seed: int | None = None,
    verbose: bool = False,
    use_cache: bool = True,
) -> Result:
    """Run the speak pipeline.

    Args:
        input_paths: Source audio files (voice bank).
        output_dir: Output directory for this run.
        text: Target text to speak (text mode).
        reference: Reference audio file for text + timing (reference mode).
        match_unit: "syllable" or "phoneme".
        pitch_correct: Whether to apply pitch correction.
        timing_strictness: How tightly to follow reference timing (0.0–1.0).
        crossfade_ms: Crossfade between syllables in ms.
        normalize_volume: Whether to normalize volume across syllables.
        whisper_model: Whisper model size.
        aligner: Alignment backend.
        seed: RNG seed.
        verbose: Show warnings.
        use_cache: Use file-based caching.
    """
    from glottisdale.speak.syllable_bank import build_bank
    from glottisdale.speak.target_text import text_to_syllables, word_boundaries_from_syllables
    from glottisdale.speak.matcher import match_syllables, match_phonemes
    from glottisdale.speak.assembler import plan_timing, assemble

    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    # --- 1. Build source syllable bank ---
    logger.info("Building source syllable bank")
    alignment_engine = get_aligner(aligner, whisper_model=whisper_model, verbose=verbose)
    all_bank_entries = []

    for input_path in input_paths:
        audio_path = output_dir / f"{input_path.stem}_16k.wav"
        extract_audio(input_path, audio_path)

        input_hash = str(input_path)
        result = alignment_engine.process(audio_path, audio_hash=input_hash, use_cache=use_cache)
        source_syllables = result["syllables"]

        entries = build_bank(source_syllables, source_path=str(audio_path))
        all_bank_entries.extend(entries)
        logger.info(f"  {input_path.name}: {len(entries)} syllables")

    logger.info(f"Syllable bank: {len(all_bank_entries)} total entries")

    # Write syllable bank JSON
    bank_json = output_dir / "syllable-bank.json"
    bank_json.write_text(json.dumps(
        {"entries": [e.to_dict() for e in all_bank_entries]},
        indent=2,
    ))

    # --- 2. Get target text ---
    target_text = text
    reference_timings = None

    if reference is not None:
        logger.info(f"Transcribing reference audio: {reference}")
        ref_audio = output_dir / "reference_16k.wav"
        extract_audio(reference, ref_audio)
        ref_result = alignment_engine.process(ref_audio, audio_hash=str(reference), use_cache=use_cache)
        target_text = ref_result["text"]
        # Extract syllable-level timing from reference
        ref_syllables = ref_result["syllables"]
        reference_timings = [(s.start, s.end) for s in ref_syllables]
        logger.info(f"Reference text: {target_text}")

    if not target_text:
        raise ValueError("Either --text or --reference must be provided")

    # --- 3. Convert target text to syllables ---
    logger.info(f"Target text: {target_text}")
    target_syls = text_to_syllables(target_text)
    word_bounds = word_boundaries_from_syllables(target_syls)
    logger.info(f"Target: {len(target_syls)} syllables, {len(word_bounds)} words")

    # --- 4. Match ---
    logger.info(f"Matching ({match_unit} mode)")
    if match_unit == "phoneme":
        all_phonemes = []
        for ts in target_syls:
            all_phonemes.extend(ts.phonemes)
        matches = match_phonemes(all_phonemes, all_bank_entries)
    else:
        target_phoneme_lists = [ts.phonemes for ts in target_syls]
        target_stresses = [ts.stress for ts in target_syls]
        matches = match_syllables(target_phoneme_lists, all_bank_entries, target_stresses)

    # --- 5. Plan timing ---
    avg_dur = (
        sum(e.duration for e in all_bank_entries) / len(all_bank_entries)
        if all_bank_entries else 0.25
    )
    timing = plan_timing(
        matches, word_bounds,
        avg_syllable_dur=avg_dur,
        reference_timings=reference_timings,
        timing_strictness=timing_strictness,
    )

    # --- 6. Assemble ---
    logger.info("Assembling output audio")
    output_path = assemble(
        matches=matches,
        timing=timing,
        output_dir=output_dir,
        crossfade_ms=crossfade_ms,
    )

    # --- 7. Write match log ---
    match_log = output_dir / "match-log.json"
    match_log.write_text(json.dumps(
        {
            "target_text": target_text,
            "match_unit": match_unit,
            "matches": [m.to_dict() for m in matches],
        },
        indent=2,
    ))
    logger.info(f"Output: {output_path}")

    return Result(
        clips=[],
        concatenated=output_path,
        transcript=target_text,
        manifest={"match_unit": match_unit, "source_count": len(input_paths)},
    )
```

**Step 4: Run tests to verify they pass**

Run: `python -m pytest tests/speak/test_integration.py -v`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/glottisdale/speak/__init__.py tests/speak/test_integration.py
git commit -m "feat(speak): add process orchestrator with text and reference modes"
```

---

### Task 7: CLI Integration

**Files:**
- Modify: `src/glottisdale/cli.py`
- Create: `tests/speak/test_cli.py`

**Step 1: Write the failing CLI tests**

Create `tests/speak/test_cli.py`:

```python
"""Tests for speak subcommand CLI."""

from pathlib import Path
from unittest.mock import patch, MagicMock

from glottisdale.cli import parse_args, main


class TestParseSpeak:
    def test_speak_with_text(self):
        args = parse_args(["speak", "source.mp4", "--text", "hello world"])
        assert args.command == "speak"
        assert args.input_files == ["source.mp4"]
        assert args.text == "hello world"

    def test_speak_with_reference(self):
        args = parse_args(["speak", "source.mp4", "--reference", "ref.mp4"])
        assert args.command == "speak"
        assert args.reference == Path("ref.mp4")

    def test_speak_defaults(self):
        args = parse_args(["speak", "source.mp4", "--text", "hi"])
        assert args.match_unit == "syllable"
        assert args.pitch_correct is True
        assert args.timing_strictness == 0.8
        assert args.crossfade == 10

    def test_speak_match_unit_phoneme(self):
        args = parse_args(["speak", "source.mp4", "--text", "hi", "--match-unit", "phoneme"])
        assert args.match_unit == "phoneme"

    def test_speak_no_pitch_correct(self):
        args = parse_args(["speak", "source.mp4", "--text", "hi", "--no-pitch-correct"])
        assert args.pitch_correct is False

    def test_speak_timing_strictness(self):
        args = parse_args(["speak", "source.mp4", "--text", "hi", "--timing-strictness", "0.5"])
        assert args.timing_strictness == 0.5

    def test_speak_shared_args(self):
        args = parse_args([
            "speak", "source.mp4", "--text", "hi",
            "--output-dir", "/tmp/out",
            "--seed", "42",
            "--whisper-model", "small",
        ])
        assert args.output_dir == "/tmp/out"
        assert args.seed == 42
        assert args.whisper_model == "small"


class TestRunSpeak:
    def test_cli_calls_process(self, tmp_path):
        source = tmp_path / "source.wav"
        source.touch()

        mock_result = MagicMock()
        mock_result.transcript = "hello"
        mock_result.clips = []
        mock_result.concatenated = MagicMock()
        mock_result.concatenated.name = "speak.wav"

        with patch("glottisdale.speak.process") as mock_process:
            mock_process.return_value = mock_result
            main([
                "speak", str(source),
                "--text", "hello world",
                "--output-dir", str(tmp_path / "out"),
            ])

            assert mock_process.called
            call_kwargs = mock_process.call_args[1]
            assert call_kwargs["text"] == "hello world"
            assert call_kwargs["match_unit"] == "syllable"

    def test_cli_creates_run_subdir(self, tmp_path):
        source = tmp_path / "source.wav"
        source.touch()

        mock_result = MagicMock()
        mock_result.transcript = "test"
        mock_result.clips = []
        mock_result.concatenated = MagicMock()
        mock_result.concatenated.name = "speak.wav"

        with patch("glottisdale.speak.process") as mock_process:
            mock_process.return_value = mock_result
            main([
                "speak", str(source),
                "--text", "hello",
                "--output-dir", str(tmp_path / "out"),
            ])

            call_kwargs = mock_process.call_args[1]
            output_dir = Path(call_kwargs["output_dir"])
            assert output_dir.parent == tmp_path / "out"

    def test_cli_requires_text_or_reference(self, tmp_path, capsys):
        """speak without --text or --reference should error."""
        source = tmp_path / "source.wav"
        source.touch()

        mock_result = MagicMock()
        mock_result.transcript = ""

        with patch("glottisdale.speak.process") as mock_process:
            mock_process.side_effect = ValueError("Either --text or --reference must be provided")
            try:
                main([
                    "speak", str(source),
                    "--output-dir", str(tmp_path / "out"),
                ])
            except (SystemExit, ValueError):
                pass  # expected
```

**Step 2: Run tests to verify they fail**

Run: `python -m pytest tests/speak/test_cli.py -v`
Expected: FAIL (speak subcommand not registered yet)

**Step 3: Add speak subcommand to CLI**

Modify `src/glottisdale/cli.py`. Add after the sing subparser registration:

1. Add `_add_speak_args` function
2. Register the `speak` subparser
3. Add `_run_speak` function
4. Add `"speak"` to the dispatcher in `main()`

The specific edits to `cli.py`:

After `_add_sing_args`, add:

```python
def _add_speak_args(parser: argparse.ArgumentParser) -> None:
    """Add arguments specific to the speak subcommand."""
    parser.add_argument(
        "--text", type=str, default=None,
        help="Target text to reconstruct using source syllables",
    )
    parser.add_argument(
        "--reference", type=Path, default=None,
        help="Reference audio — transcribed for target text + timing template",
    )
    parser.add_argument(
        "--match-unit", default="syllable",
        choices=["syllable", "phoneme"],
        help="Matching granularity (default: syllable)",
    )
    parser.add_argument(
        "--pitch-correct", "--no-pitch-correct",
        action=argparse.BooleanOptionalAction, default=True,
        help="Adjust pitch to target intonation (default: enabled)",
    )
    parser.add_argument(
        "--timing-strictness", type=float, default=0.8,
        help="How closely to follow reference timing, 0.0–1.0 (default: 0.8)",
    )
    parser.add_argument(
        "--crossfade", type=float, default=10,
        help="Crossfade between syllables in ms (default: 10)",
    )
    parser.add_argument(
        "--normalize-volume", action=argparse.BooleanOptionalAction, default=True,
        help="Normalize volume across syllables (default: enabled)",
    )
    parser.add_argument(
        "--aligner", default="auto",
        choices=["auto", "default", "bfa"],
        help="Alignment backend (default: auto)",
    )
```

After the sing subparser, add:

```python
    # Speak subcommand
    speak_parser = subparsers.add_parser(
        "speak",
        help="Reconstruct text using source audio syllables",
        description="Reconstruct target text using syllable fragments from source audio",
    )
    _add_shared_args(speak_parser)
    _add_speak_args(speak_parser)
```

Add `_run_speak` function:

```python
def _run_speak(args: argparse.Namespace) -> None:
    """Run the speak pipeline."""
    from glottisdale.speak import process

    input_paths = [Path(f) for f in args.input_files]
    for p in input_paths:
        if not p.exists():
            print(f"Error: file not found: {p}", file=sys.stderr)
            sys.exit(1)

    if not input_paths:
        print("Error: at least one input file is required", file=sys.stderr)
        sys.exit(1)

    if not args.text and not args.reference:
        print("Error: either --text or --reference is required", file=sys.stderr)
        sys.exit(1)

    from glottisdale.names import create_run_dir

    output_root = Path(args.output_dir)
    run_dir = create_run_dir(output_root, seed=args.seed, run_name=args.run_name)
    print(f"Run: {run_dir.name}")

    result = process(
        input_paths=input_paths,
        output_dir=run_dir,
        text=args.text,
        reference=args.reference,
        match_unit=args.match_unit,
        pitch_correct=args.pitch_correct,
        timing_strictness=args.timing_strictness,
        crossfade_ms=args.crossfade,
        normalize_volume=args.normalize_volume,
        whisper_model=args.whisper_model,
        aligner=args.aligner,
        seed=args.seed,
        verbose=args.verbose,
        use_cache=not args.no_cache,
    )

    print(f"Target text: {result.transcript}")
    print(f"Output: {result.concatenated.name}")
```

In the `main()` dispatcher, add:

```python
    elif args.command == "speak":
        _run_speak(args)
```

**Step 4: Run tests to verify they pass**

Run: `python -m pytest tests/speak/test_cli.py -v`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/glottisdale/cli.py tests/speak/test_cli.py
git commit -m "feat(speak): wire up speak subcommand in CLI"
```

---

### Task 8: Documentation Updates

**Files:**
- Modify: `README.md`
- Modify: `docs/getting-started/quickstart.md`
- Modify: `docs/reference/architecture.md`

**Step 1: Update README.md**

Add a `speak` section to the CLI reference in README.md, after the `sing` section. Include:
- Description: "Reconstruct target text using syllable fragments from source audio"
- Example: `glottisdale speak source.mp4 --text "the quick brown fox"`
- Example: `glottisdale speak source.mp4 --reference guide.mp4`
- List of speak-specific flags: `--text`, `--reference`, `--match-unit`, `--pitch-correct`, `--timing-strictness`, `--crossfade`, `--normalize-volume`

**Step 2: Update quickstart.md**

Add a "Speak" section showing the simplest usage:

```
glottisdale speak your-video.mp4 --text "hello world"
```

**Step 3: Update architecture.md**

Add a "Speak Pipeline" section after the existing sing pipeline, describing the 6-step flow:
1. Build source syllable bank (transcribe, syllabify, align, index)
2. Convert target text to ARPABET syllables
3. Match target syllables to source bank by phonetic distance
4. Plan timing (text mode or reference mode)
5. Assemble audio (cut, stretch, pitch-shift, concatenate)
6. Write output files (speak.wav, match-log.json, syllable-bank.json)

**Step 4: Commit**

```bash
git add README.md docs/getting-started/quickstart.md docs/reference/architecture.md
git commit -m "docs: add speak subcommand documentation"
```

---

### Task 9: Run Full Test Suite & Final Verification

**Step 1: Run all tests**

Run: `python -m pytest tests/ -v`
Expected: All PASS

**Step 2: Manual smoke test (if test audio available)**

Run: `python -m glottisdale speak --help`
Expected: Shows speak subcommand help with all flags

**Step 3: Verify no regressions**

Run: `python -m pytest tests/collage/ tests/sing/ -v`
Expected: All existing tests still pass

**Step 4: Final commit if any fixes needed**

Only if test failures require fixes.
