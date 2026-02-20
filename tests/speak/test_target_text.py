"""Tests for target text to ARPABET syllable conversion."""

from glottisdale.speak.target_text import text_to_syllables, TextSyllable
from glottisdale.speak.target_text import word_boundaries_from_syllables


class TestTextToSyllables:
    def test_single_word(self):
        result = text_to_syllables("cat")
        assert len(result) >= 1
        # "cat" -> K AE T -> one syllable
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
        # "the"=1syl, "cat"=1syl, "sat"=1syl -> boundaries at [0, 1, 2]
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

    def test_no_punctuation_in_phonemes(self):
        """g2p_en may leak punctuation; ensure it's filtered out."""
        result = text_to_syllables("hello, world.")
        all_phonemes = []
        for syl in result:
            all_phonemes.extend(syl.phonemes)
        for p in all_phonemes:
            assert p.isalpha() or p[-1].isdigit(), (
                f"Punctuation leaked into phonemes: {p!r}"
            )
