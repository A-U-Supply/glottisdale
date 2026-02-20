"""Match target syllables/phonemes to a source syllable bank."""

from dataclasses import dataclass

from glottisdale.speak.phonetic_distance import (
    phoneme_distance,
    strip_stress,
    syllable_distance,
)
from glottisdale.speak.syllable_bank import SyllableEntry

# Default bonus applied when consecutive target syllables match to adjacent
# source syllables.  A value of 3 means the DP will prefer a contiguous
# source syllable whose phonetic distance is up to 3 worse than the
# globally‑best non‑contiguous alternative.
_CONTINUITY_BONUS = 7


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


def _are_adjacent(a: SyllableEntry, b: SyllableEntry) -> bool:
    """True if *b* immediately follows *a* in the same source file."""
    return a.source_path == b.source_path and b.index == a.index + 1


def match_syllables(
    target_syllables: list[list[str]],
    bank: list[SyllableEntry],
    target_stresses: list[int | None] | None = None,
    continuity_bonus: int = _CONTINUITY_BONUS,
) -> list[MatchResult]:
    """Match target syllables to source bank using Viterbi DP.

    Finds the sequence of source syllables that minimises total phonetic
    distance while rewarding contiguous source runs (adjacent source
    syllables matched to consecutive target syllables).

    Args:
        target_syllables: List of syllables, each a list of ARPABET phonemes.
        bank: Source syllable bank to search.
        target_stresses: Optional stress levels for tie-breaking.
        continuity_bonus: Cost reduction for adjacent source syllables.

    Returns:
        One MatchResult per target syllable.
    """
    n = len(target_syllables)
    b = len(bank)
    if n == 0 or b == 0:
        return []

    # Pre‑compute pairwise distances (with small stress penalty for ties)
    dists: list[list[float]] = []
    for i, target in enumerate(target_syllables):
        stress = (
            target_stresses[i]
            if target_stresses and i < len(target_stresses)
            else None
        )
        row: list[float] = []
        for entry in bank:
            d = syllable_distance(target, entry.phoneme_labels)
            if stress is not None and entry.stress != stress:
                d += 0.1  # tiny penalty, only matters for tie‑breaking
            row.append(d)
        dists.append(row)

    # Pre‑compute predecessor map: pred[j] = k  iff  bank[k] → bank[j]
    pred: dict[int, int] = {}
    for j in range(b):
        for k in range(b):
            if _are_adjacent(bank[k], bank[j]):
                pred[j] = k
                break

    # --- Viterbi DP ---
    INF = float("inf")

    # dp[j] = min total cost when current target matched to bank[j]
    dp = list(dists[0])
    # parent[i][j] = bank index chosen for target i‑1 on the best path to j
    parents: list[list[int]] = [[-1] * b]  # placeholder for i=0

    for i in range(1, n):
        new_dp = [INF] * b
        new_parent = [-1] * b

        # Best previous cost across all bank entries (non‑contiguous case)
        min_prev = min(dp)

        for j in range(b):
            cost = dists[i][j]

            # Non‑contiguous: best of any previous bank entry
            best = min_prev + cost
            # Find the k that achieves min_prev (first one is fine)
            best_k = dp.index(min_prev)

            # Contiguous: predecessor in the same source
            if j in pred:
                k = pred[j]
                contiguous = dp[k] + cost - continuity_bonus
                if contiguous < best:
                    best = contiguous
                    best_k = k

            new_dp[j] = best
            new_parent[j] = best_k

        dp = new_dp
        parents.append(new_parent)

    # --- Backtrace ---
    best_last = min(range(b), key=lambda j: dp[j])
    path = [best_last]
    for i in range(n - 1, 0, -1):
        path.append(parents[i][path[-1]])
    path.reverse()

    return [
        MatchResult(
            target_phonemes=target_syllables[i],
            entry=bank[path[i]],
            distance=int(dists[i][path[i]]),
            target_index=i,
        )
        for i in range(n)
    ]


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
