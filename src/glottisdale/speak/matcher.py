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
