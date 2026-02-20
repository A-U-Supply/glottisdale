"""Aligner interface and backends."""

import logging
import shutil
from abc import ABC, abstractmethod
from pathlib import Path

from glottisdale.types import Syllable
from glottisdale.collage.transcribe import transcribe
from glottisdale.collage.syllabify import syllabify_words

logger = logging.getLogger(__name__)


class Aligner(ABC):
    """Abstract base for speech alignment backends."""

    name: str = "base"

    @abstractmethod
    def process(
        self,
        audio_path: Path,
        audio_hash: str | None = None,
        use_cache: bool = False,
    ) -> dict:
        """Transcribe and align audio, returning syllable-level timestamps.

        Args:
            audio_path: Path to the audio file.
            audio_hash: Pre-computed hash (enables caching).
            use_cache: Whether to check/store caches.

        Returns:
            Dict with keys:
                text: Full transcript
                words: List of word dicts with timestamps
                syllables: List of Syllable objects
        """


class DefaultAligner(Aligner):
    """Whisper ASR + g2p_en + ARPABET syllabifier.

    Word-level timestamps from Whisper, phoneme conversion via g2p_en,
    syllable timing estimated by proportional distribution.
    """

    name = "default"

    def __init__(self, whisper_model: str = "base", language: str = "en", **kwargs):
        self.whisper_model = whisper_model
        self.language = language

    def process(
        self,
        audio_path: Path,
        audio_hash: str | None = None,
        use_cache: bool = False,
    ) -> dict:
        if use_cache and audio_hash:
            from glottisdale.cache import get_cached_alignment, store_alignment_cache
            cached = get_cached_alignment(
                self.name, audio_hash, self.whisper_model, self.language,
            )
            if cached is not None:
                return cached

        result = transcribe(
            audio_path,
            model_name=self.whisper_model,
            language=self.language,
            audio_hash=audio_hash,
            use_cache=use_cache,
        )
        syllables = syllabify_words(result["words"])
        output = {
            "text": result["text"],
            "words": result["words"],
            "syllables": syllables,
        }

        if use_cache and audio_hash:
            from glottisdale.cache import store_alignment_cache
            store_alignment_cache(
                self.name, audio_hash, self.whisper_model, self.language, output,
            )

        return output


def _get_bfa_class():
    """Lazy import of BFAAligner to avoid hard dependency."""
    from glottisdale.collage.bfa import BFAAligner
    return BFAAligner


def _bfa_available() -> bool:
    """Check if BFA and espeak-ng are both available."""
    try:
        import bournemouth_aligner  # noqa: F401
    except ImportError:
        return False
    if shutil.which("espeak-ng") is None:
        return False
    return True


# Registry of available backends (lazy import for BFA)
_ALIGNERS = {
    "default": DefaultAligner,
    "bfa": _get_bfa_class,
}


def get_aligner(name: str, **kwargs) -> Aligner:
    """Get an aligner backend by name.

    Modes:
        "default" — Whisper + g2p_en + ARPABET proportional timing.
        "bfa" — Whisper + BFA phoneme-level alignment (requires bournemouth-forced-aligner + espeak-ng).
        "auto" — Tries BFA first, falls back to default if unavailable.
    """
    if name == "auto":
        if _bfa_available():
            logger.info("Auto-detected BFA + espeak-ng, using BFA aligner")
            return _get_bfa_class()(**kwargs)
        else:
            logger.info("BFA not available, falling back to default (proportional) aligner")
            return DefaultAligner(**kwargs)

    if name not in _ALIGNERS:
        raise ValueError(
            f"Unknown aligner: {name!r}. Available: {list(_ALIGNERS.keys()) + ['auto']}"
        )

    factory = _ALIGNERS[name]
    # BFA entry is a function returning the class (lazy import)
    if name == "bfa":
        try:
            cls = factory()
        except ImportError as e:
            raise ImportError(
                f"BFA aligner requires 'bournemouth-forced-aligner' package: {e}"
            ) from e
        return cls(**kwargs)

    return factory(**kwargs)
