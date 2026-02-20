"""Tests for syllable bank construction."""

from glottisdale.speak.syllable_bank import SyllableEntry, build_bank, _is_phoneme
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

    def test_normalizes_ipa_labels(self):
        """IPA labels from BFA aligner are converted to ARPABET."""
        syls = [
            _make_syllable(["b", "ʌ", "t"], 0.0, 0.3, word="but"),
            _make_syllable(["k", "æ", "t"], 0.3, 0.7, word="cat"),
        ]
        bank = build_bank(syls, source_path="test.wav")
        assert bank[0].phoneme_labels == ["B", "AH", "T"]
        assert bank[1].phoneme_labels == ["K", "AE", "T"]

    def test_normalizes_ipa_diphthongs(self):
        """IPA diphthongs are normalized to ARPABET."""
        syls = [_make_syllable(["aɪ"], 0.0, 0.3, word="eye")]
        bank = build_bank(syls, source_path="test.wav")
        assert bank[0].phoneme_labels == ["AY"]

    def test_arpabet_labels_unchanged(self):
        """Already-ARPABET labels pass through normalization unchanged."""
        syls = [_make_syllable(["B", "AH1"], 0.0, 0.3, word="but")]
        bank = build_bank(syls, source_path="test.wav")
        assert bank[0].phoneme_labels == ["B", "AH1"]

    def test_filters_punctuation_labels(self):
        """Punctuation labels should be stripped from phoneme lists."""
        syls = [_make_syllable(["B", ",", "AH1"], 0.0, 0.3, word="but")]
        bank = build_bank(syls, source_path="test.wav")
        assert len(bank) == 1
        assert bank[0].phoneme_labels == ["B", "AH1"]

    def test_skips_punctuation_only_syllables(self):
        """Syllables with only punctuation phonemes should be excluded."""
        syls = [
            _make_syllable([".", ","], 0.0, 0.1, word="."),
            _make_syllable(["B", "AH1"], 0.1, 0.4, word="but"),
        ]
        bank = build_bank(syls, source_path="test.wav")
        assert len(bank) == 1
        assert bank[0].phoneme_labels == ["B", "AH1"]

    def test_filters_empty_labels(self):
        """Empty string phoneme labels should be filtered out."""
        syls = [_make_syllable(["", "B", "AH1"], 0.0, 0.3, word="but")]
        bank = build_bank(syls, source_path="test.wav")
        assert len(bank) == 1
        assert bank[0].phoneme_labels == ["B", "AH1"]


class TestIsPhoneme:
    def test_valid_phonemes(self):
        assert _is_phoneme("B") is True
        assert _is_phoneme("AH1") is True
        assert _is_phoneme("SH") is True

    def test_punctuation(self):
        assert _is_phoneme(",") is False
        assert _is_phoneme(".") is False
        assert _is_phoneme("!") is False
        assert _is_phoneme("?") is False

    def test_empty(self):
        assert _is_phoneme("") is False
