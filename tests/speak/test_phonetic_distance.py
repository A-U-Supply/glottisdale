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
        # AH (mid central) vs AE (low front) -- differ in height + backness
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
        # "BA" vs "PA" -- only onset differs by voicing
        d = syllable_distance(["B", "AH1"], ["P", "AH1"])
        assert d == 1

    def test_different_lengths_aligned(self):
        # Shorter syllable padded -- should still produce a finite distance
        d = syllable_distance(["B", "AH1"], ["S", "T", "AH1"])
        assert d > 0
        assert d < 100  # not infinite

    def test_empty_syllable(self):
        d = syllable_distance([], ["B", "AH1"])
        assert d > 0
