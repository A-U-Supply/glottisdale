"""Whisper ASR transcription with word-level timestamps."""

import logging
from pathlib import Path

import whisper

logger = logging.getLogger(__name__)

_model_cache: dict[str, object] = {}


def transcribe(
    audio_path: Path,
    model_name: str = "base",
    language: str = "en",
    audio_hash: str | None = None,
    use_cache: bool = False,
) -> dict:
    """Transcribe audio and return word-level timestamps.

    Args:
        audio_path: Path to the audio file.
        model_name: Whisper model size.
        language: Language code.
        audio_hash: Pre-computed hash of the audio file (enables caching).
        use_cache: Whether to check/store the transcription cache.

    Returns:
        Dict with keys:
            text: Full transcript (stripped)
            words: List of dicts with 'word', 'start', 'end' keys
            language: Detected or specified language
    """
    if use_cache and audio_hash:
        from glottisdale.cache import get_cached_transcription
        cached = get_cached_transcription(audio_hash, model_name, language)
        if cached is not None:
            return cached

    if model_name not in _model_cache:
        _model_cache[model_name] = whisper.load_model(model_name)
    model = _model_cache[model_name]

    result = model.transcribe(
        str(audio_path),
        word_timestamps=True,
        language=language,
    )

    # Flatten words across segments, strip whitespace
    words = []
    for segment in result.get("segments", []):
        for w in segment.get("words", []):
            words.append({
                "word": w["word"].strip(),
                "start": w["start"],
                "end": w["end"],
            })

    output = {
        "text": result["text"].strip(),
        "words": words,
        "language": result.get("language", language),
    }

    if use_cache and audio_hash:
        from glottisdale.cache import store_transcription_cache
        store_transcription_cache(audio_hash, model_name, language, output)

    return output
