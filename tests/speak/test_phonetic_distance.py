"""Tests for ARPABET phonetic distance."""

from glottisdale.speak.phonetic_distance import (
    normalize_phoneme,
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


class TestNormalizePhoneme:
    def test_ipa_consonants(self):
        assert normalize_phoneme("b") == "B"
        assert normalize_phoneme("p") == "P"
        assert normalize_phoneme("t") == "T"
        assert normalize_phoneme("d") == "D"
        assert normalize_phoneme("k") == "K"
        assert normalize_phoneme("g") == "G"
        assert normalize_phoneme("ʃ") == "SH"
        assert normalize_phoneme("ʒ") == "ZH"
        assert normalize_phoneme("θ") == "TH"
        assert normalize_phoneme("ð") == "DH"
        assert normalize_phoneme("ŋ") == "NG"

    def test_ipa_vowels(self):
        assert normalize_phoneme("ɪ") == "IH"
        assert normalize_phoneme("ɛ") == "EH"
        assert normalize_phoneme("æ") == "AE"
        assert normalize_phoneme("ɑ") == "AA"
        assert normalize_phoneme("ɔ") == "AO"
        assert normalize_phoneme("ə") == "AH"
        assert normalize_phoneme("ʊ") == "UH"
        assert normalize_phoneme("ɜ") == "ER"

    def test_ipa_diphthongs(self):
        assert normalize_phoneme("aɪ") == "AY"
        assert normalize_phoneme("aʊ") == "AW"
        assert normalize_phoneme("eɪ") == "EY"
        assert normalize_phoneme("oʊ") == "OW"
        assert normalize_phoneme("ɔɪ") == "OY"

    def test_ipa_rhotics_and_liquids(self):
        assert normalize_phoneme("ɹ") == "R"
        assert normalize_phoneme("ɾ") == "R"
        assert normalize_phoneme("r") == "R"
        assert normalize_phoneme("l") == "L"
        assert normalize_phoneme("ɫ") == "L"

    def test_ipa_glides(self):
        assert normalize_phoneme("j") == "Y"
        assert normalize_phoneme("w") == "W"

    def test_arpabet_passthrough(self):
        assert normalize_phoneme("B") == "B"
        assert normalize_phoneme("AH") == "AH"
        assert normalize_phoneme("AH1") == "AH1"
        assert normalize_phoneme("IY0") == "IY0"
        assert normalize_phoneme("SH") == "SH"
        assert normalize_phoneme("NG") == "NG"

    def test_unknown_passthrough(self):
        assert normalize_phoneme("ʔ") == "ʔ"
        assert normalize_phoneme("") == ""

    def test_length_markers_stripped(self):
        assert normalize_phoneme("iː") == "IY"
        assert normalize_phoneme("uː") == "UW"


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
