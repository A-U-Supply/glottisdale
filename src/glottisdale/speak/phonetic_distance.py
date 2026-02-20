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
