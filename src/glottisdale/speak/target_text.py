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
