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
