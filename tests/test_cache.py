"""Tests for the cache module."""

import json
from pathlib import Path

import pytest

from glottisdale.cache import (
    file_hash,
    get_cached_audio,
    store_audio_cache,
    get_cached_transcription,
    store_transcription_cache,
    get_cached_alignment,
    store_alignment_cache,
    _serialize_alignment,
    _deserialize_alignment,
)
from glottisdale.types import Phoneme, Syllable


@pytest.fixture(autouse=True)
def isolated_cache(tmp_path, monkeypatch):
    """Redirect cache to tmp_path so tests don't pollute the real cache."""
    monkeypatch.setattr("glottisdale.cache.CACHE_DIR", tmp_path / "cache")


# --- file_hash ---


def test_file_hash_deterministic(tmp_path):
    """Same content always produces the same hash."""
    f = tmp_path / "data.bin"
    f.write_bytes(b"hello world")
    h1 = file_hash(f)
    h2 = file_hash(f)
    assert h1 == h2
    assert len(h1) == 64  # SHA-256 hex


def test_file_hash_different_content(tmp_path):
    """Different content produces different hashes."""
    f1 = tmp_path / "a.bin"
    f2 = tmp_path / "b.bin"
    f1.write_bytes(b"hello")
    f2.write_bytes(b"world")
    assert file_hash(f1) != file_hash(f2)


# --- Audio extraction cache ---


def test_audio_cache_miss():
    assert get_cached_audio("nonexistent_hash") is None


def test_audio_cache_roundtrip(tmp_path):
    """Store and retrieve extracted audio."""
    wav = tmp_path / "test.wav"
    wav.write_bytes(b"RIFF" + b"\x00" * 100)

    path = store_audio_cache("abc123", wav)
    assert path.exists()

    cached = get_cached_audio("abc123")
    assert cached is not None
    assert cached.read_bytes() == wav.read_bytes()


# --- Whisper transcription cache ---


def test_transcription_cache_miss():
    assert get_cached_transcription("hash", "base", "en") is None


def test_transcription_cache_roundtrip():
    result = {
        "text": "hello world",
        "words": [
            {"word": "hello", "start": 0.0, "end": 0.5},
            {"word": "world", "start": 0.6, "end": 1.0},
        ],
        "language": "en",
    }
    store_transcription_cache("audio_hash", "base", "en", result)
    cached = get_cached_transcription("audio_hash", "base", "en")
    assert cached is not None
    assert cached["text"] == "hello world"
    assert len(cached["words"]) == 2
    assert cached["words"][0]["word"] == "hello"


def test_transcription_cache_different_model():
    """Different model names produce different cache entries."""
    result = {"text": "hi", "words": [], "language": "en"}
    store_transcription_cache("hash", "base", "en", result)
    store_transcription_cache("hash", "small", "en", {"text": "different", "words": [], "language": "en"})

    base = get_cached_transcription("hash", "base", "en")
    small = get_cached_transcription("hash", "small", "en")
    assert base["text"] == "hi"
    assert small["text"] == "different"


# --- Alignment cache ---


def _make_alignment_result():
    """Create a sample alignment result with Syllable/Phoneme objects."""
    return {
        "text": "hello world",
        "words": [
            {"word": "hello", "start": 0.0, "end": 0.5},
            {"word": "world", "start": 0.6, "end": 1.0},
        ],
        "syllables": [
            Syllable(
                phonemes=[
                    Phoneme(label="HH", start=0.0, end=0.1),
                    Phoneme(label="AH0", start=0.1, end=0.25),
                ],
                start=0.0,
                end=0.25,
                word="hello",
                word_index=0,
            ),
            Syllable(
                phonemes=[
                    Phoneme(label="L", start=0.25, end=0.35),
                    Phoneme(label="OW1", start=0.35, end=0.5),
                ],
                start=0.25,
                end=0.5,
                word="hello",
                word_index=0,
            ),
            Syllable(
                phonemes=[
                    Phoneme(label="W", start=0.6, end=0.7),
                    Phoneme(label="ER1", start=0.7, end=0.85),
                    Phoneme(label="L", start=0.85, end=0.9),
                    Phoneme(label="D", start=0.9, end=1.0),
                ],
                start=0.6,
                end=1.0,
                word="world",
                word_index=1,
            ),
        ],
    }


def test_alignment_cache_miss():
    assert get_cached_alignment("default", "hash", "base", "en") is None


def test_alignment_cache_roundtrip():
    result = _make_alignment_result()
    store_alignment_cache("default", "audio_hash", "base", "en", result)
    cached = get_cached_alignment("default", "audio_hash", "base", "en")

    assert cached is not None
    assert cached["text"] == "hello world"
    assert len(cached["words"]) == 2
    assert len(cached["syllables"]) == 3


def test_alignment_deserializes_syllable_objects():
    """Cached alignment should produce real Syllable/Phoneme dataclass instances."""
    result = _make_alignment_result()
    store_alignment_cache("bfa", "hash", "base", "en", result, device="cpu")
    cached = get_cached_alignment("bfa", "hash", "base", "en", device="cpu")

    assert cached is not None
    syls = cached["syllables"]
    assert all(isinstance(s, Syllable) for s in syls)
    assert all(isinstance(p, Phoneme) for p in syls[0].phonemes)

    # Verify values roundtrip correctly
    assert syls[0].word == "hello"
    assert syls[0].word_index == 0
    assert syls[0].start == 0.0
    assert syls[0].end == 0.25
    assert syls[0].phonemes[0].label == "HH"
    assert syls[0].phonemes[0].start == 0.0
    assert syls[0].phonemes[0].end == 0.1

    assert syls[2].word == "world"
    assert syls[2].word_index == 1
    assert len(syls[2].phonemes) == 4


def test_alignment_cache_different_aligner():
    """Different aligner names produce different cache entries."""
    result = _make_alignment_result()
    store_alignment_cache("default", "hash", "base", "en", result)
    assert get_cached_alignment("bfa", "hash", "base", "en") is None


def test_serialize_deserialize_roundtrip():
    """Direct test of serialization/deserialization."""
    result = _make_alignment_result()
    serialized = _serialize_alignment(result)

    # Serialized should be JSON-safe (no dataclass objects)
    json_str = json.dumps(serialized)
    assert "HH" in json_str

    deserialized = _deserialize_alignment(serialized)
    assert len(deserialized["syllables"]) == 3
    assert isinstance(deserialized["syllables"][0], Syllable)
    assert isinstance(deserialized["syllables"][0].phonemes[0], Phoneme)
