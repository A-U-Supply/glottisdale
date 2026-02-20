"""Build an indexed bank of source syllables for matching."""

from dataclasses import dataclass
from glottisdale.speak.phonetic_distance import normalize_phoneme
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


def _is_phoneme(label: str) -> bool:
    """Return True if label is a real phoneme (not punctuation or empty)."""
    return bool(label) and label[0].isalpha()


def build_bank(syllables: list[Syllable], source_path: str) -> list[SyllableEntry]:
    """Build a syllable bank from aligned source syllables.

    Filters out punctuation labels from phoneme lists and skips
    syllables that have no real phonemes after filtering.
    """
    entries = []
    for i, syl in enumerate(syllables):
        labels = [
            normalize_phoneme(p.label) for p in syl.phonemes
            if _is_phoneme(p.label)
        ]
        if not labels:
            continue
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
