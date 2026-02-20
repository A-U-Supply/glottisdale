"""File-based caching for expensive pipeline operations."""

import hashlib
import json
import logging
import os
import shutil
import tempfile
from pathlib import Path

from glottisdale.types import Phoneme, Syllable

logger = logging.getLogger(__name__)

CACHE_DIR = Path(os.environ.get("GLOTTISDALE_CACHE_DIR", "~/.cache/glottisdale")).expanduser()


def file_hash(path: Path) -> str:
    """Compute SHA-256 hash of a file's contents."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def _atomic_write(target: Path, data: bytes) -> None:
    """Write data to target atomically via temp file + rename."""
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=target.parent, suffix=".tmp")
    try:
        os.write(fd, data)
        os.close(fd)
        os.replace(tmp, target)
    except Exception:
        os.close(fd) if not os.get_inheritable(fd) else None
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise


# --- Audio extraction cache ---


def _extract_cache_path(input_hash: str) -> Path:
    return CACHE_DIR / "extract" / f"{input_hash}.wav"


def get_cached_audio(input_hash: str) -> Path | None:
    """Return cached extracted audio path, or None if not cached."""
    path = _extract_cache_path(input_hash)
    if path.exists() and path.stat().st_size > 0:
        logger.info(f"Cache hit: audio extraction ({input_hash[:12]}...)")
        return path
    return None


def store_audio_cache(input_hash: str, audio_path: Path) -> Path:
    """Copy extracted audio into cache. Returns the cache path."""
    dest = _extract_cache_path(input_hash)
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(audio_path, dest)
    logger.info(f"Cached audio extraction ({input_hash[:12]}...)")
    return dest


# --- Whisper transcription cache ---


def _whisper_cache_path(audio_hash: str, model: str, language: str) -> Path:
    return CACHE_DIR / "whisper" / f"{audio_hash}_{model}_{language}.json"


def get_cached_transcription(
    audio_hash: str, model: str, language: str
) -> dict | None:
    """Return cached whisper result dict, or None if not cached."""
    path = _whisper_cache_path(audio_hash, model, language)
    if path.exists():
        try:
            data = json.loads(path.read_text())
            logger.info(f"Cache hit: transcription ({audio_hash[:12]}...)")
            return data
        except (json.JSONDecodeError, OSError):
            return None
    return None


def store_transcription_cache(
    audio_hash: str, model: str, language: str, result: dict
) -> None:
    """Store whisper transcription result in cache."""
    path = _whisper_cache_path(audio_hash, model, language)
    _atomic_write(path, json.dumps(result).encode())
    logger.info(f"Cached transcription ({audio_hash[:12]}...)")


# --- Alignment cache ---


def _align_cache_path(
    aligner_name: str,
    audio_hash: str,
    model: str,
    language: str,
    device: str | None = None,
) -> Path:
    parts = [aligner_name, audio_hash, model, language]
    if device:
        parts.append(device)
    return CACHE_DIR / "align" / f"{'_'.join(parts)}.json"


def _serialize_alignment(result: dict) -> dict:
    """Convert alignment result (with Syllable/Phoneme objects) to JSON-safe dict."""
    serialized = {
        "text": result["text"],
        "words": result["words"],
        "syllables": [
            {
                "phonemes": [
                    {"label": p.label, "start": p.start, "end": p.end}
                    for p in syl.phonemes
                ],
                "start": syl.start,
                "end": syl.end,
                "word": syl.word,
                "word_index": syl.word_index,
            }
            for syl in result["syllables"]
        ],
    }
    return serialized


def _deserialize_alignment(data: dict) -> dict:
    """Reconstruct Syllable/Phoneme objects from cached JSON."""
    syllables = []
    for syl_data in data.get("syllables", []):
        phonemes = [
            Phoneme(
                label=p["label"],
                start=p["start"],
                end=p["end"],
            )
            for p in syl_data["phonemes"]
        ]
        syllables.append(Syllable(
            phonemes=phonemes,
            start=syl_data["start"],
            end=syl_data["end"],
            word=syl_data["word"],
            word_index=syl_data["word_index"],
        ))
    return {
        "text": data["text"],
        "words": data["words"],
        "syllables": syllables,
    }


def get_cached_alignment(
    aligner_name: str,
    audio_hash: str,
    model: str,
    language: str,
    device: str | None = None,
) -> dict | None:
    """Return cached alignment result with deserialized Syllable objects, or None."""
    path = _align_cache_path(aligner_name, audio_hash, model, language, device)
    if path.exists():
        try:
            data = json.loads(path.read_text())
            result = _deserialize_alignment(data)
            logger.info(f"Cache hit: alignment ({audio_hash[:12]}...)")
            return result
        except (json.JSONDecodeError, OSError, KeyError):
            return None
    return None


def store_alignment_cache(
    aligner_name: str,
    audio_hash: str,
    model: str,
    language: str,
    result: dict,
    device: str | None = None,
) -> None:
    """Store alignment result in cache."""
    path = _align_cache_path(aligner_name, audio_hash, model, language, device)
    serialized = _serialize_alignment(result)
    _atomic_write(path, json.dumps(serialized).encode())
    logger.info(f"Cached alignment ({audio_hash[:12]}...)")
