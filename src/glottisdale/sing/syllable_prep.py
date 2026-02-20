"""Prepare syllable clips from audio/video sources using glottisdale library."""
import logging
import math
import subprocess
from dataclasses import dataclass
from pathlib import Path
from statistics import median

from glottisdale.audio import extract_audio, cut_clip, get_duration, adjust_volume
from glottisdale.analysis import read_wav, estimate_f0, compute_rms
from glottisdale.collage.transcribe import transcribe
from glottisdale.collage.syllabify import syllabify_words

logger = logging.getLogger(__name__)


@dataclass
class NormalizedSyllable:
    """A pitch- and volume-normalized syllable clip."""
    clip_path: Path
    f0: float | None
    duration: float
    phonemes: list[str]
    word: str


def compute_pitch_shifts(f0_values: list[float | None]) -> list[float]:
    """Compute semitone shifts to normalize all F0s to the median.

    Returns a list of shifts in semitones (same length as input).
    None values get 0 shift.
    """
    voiced = [f for f in f0_values if f is not None and f > 0]
    if not voiced:
        return [0.0] * len(f0_values)

    target = median(voiced)
    shifts = []
    for f0 in f0_values:
        if f0 is None or f0 <= 0:
            shifts.append(0.0)
        else:
            shifts.append(12 * math.log2(target / f0))
    return shifts


def prepare_syllables(
    input_paths: list[Path],
    work_dir: Path,
    whisper_model: str = "base",
    max_semitone_shift: float = 5.0,
    use_cache: bool = True,
) -> list[NormalizedSyllable]:
    """Full pipeline: transcribe, syllabify, cut, normalize.

    Args:
        input_paths: Video or audio files to process.
        work_dir: Working directory for intermediate files.
        whisper_model: Whisper model size.
        max_semitone_shift: Maximum pitch normalization shift.
        use_cache: Whether to use file-based caching for extraction and transcription.

    Returns:
        List of NormalizedSyllable with normalized clips.
    """
    clips_dir = work_dir / "syllable_clips"
    clips_dir.mkdir(parents=True, exist_ok=True)

    all_syllables = []
    clip_index = 0

    for input_path in input_paths:
        # Hash input for cache lookups
        input_hash = None
        if use_cache:
            from glottisdale.cache import file_hash, get_cached_audio, store_audio_cache
            try:
                input_hash = file_hash(input_path)
            except OSError:
                input_hash = None

        # Extract audio
        wav_path = work_dir / f"{input_path.stem}_audio.wav"
        cached_audio = get_cached_audio(input_hash) if input_hash else None
        if cached_audio is not None:
            import shutil
            shutil.copy2(cached_audio, wav_path)
        else:
            extract_audio(input_path, wav_path)
            if input_hash:
                store_audio_cache(input_hash, wav_path)

        # Transcribe
        result = transcribe(
            wav_path, model_name=whisper_model,
            audio_hash=input_hash, use_cache=use_cache,
        )
        words = result.get("words", [])
        if not words:
            logger.warning(f"No words transcribed from {input_path}")
            continue

        # Syllabify
        syllables = syllabify_words(words)
        logger.info(f"{input_path.name}: {len(syllables)} syllables")

        # Cut each syllable
        for syl in syllables:
            clip_path = clips_dir / f"syl_{clip_index:04d}.wav"
            cut_clip(wav_path, clip_path, syl.start, syl.end, padding_ms=25)

            # Estimate F0
            samples, sr = read_wav(clip_path)
            f0 = estimate_f0(samples, sr)
            duration = get_duration(clip_path)

            phoneme_labels = [p.label for p in syl.phonemes]
            all_syllables.append(NormalizedSyllable(
                clip_path=clip_path,
                f0=f0,
                duration=duration,
                phonemes=phoneme_labels,
                word=syl.word,
            ))
            clip_index += 1

    if not all_syllables:
        raise ValueError("No syllables extracted from any input file")

    # Normalize pitch to median F0
    f0_values = [s.f0 for s in all_syllables]
    shifts = compute_pitch_shifts(f0_values)
    for syl, shift in zip(all_syllables, shifts):
        if abs(shift) < 0.1:
            continue
        clamped = max(-max_semitone_shift, min(max_semitone_shift, shift))
        normalized_path = syl.clip_path.with_suffix(".norm.wav")
        _rubberband_pitch_shift(syl.clip_path, normalized_path, clamped)
        if normalized_path.exists():
            syl.clip_path = normalized_path

    # Volume normalize to median RMS
    rms_values = []
    for syl in all_syllables:
        samples, sr = read_wav(syl.clip_path)
        rms = compute_rms(samples)
        rms_values.append(rms)

    voiced_rms = [r for r in rms_values if r > 0]
    if voiced_rms:
        target_rms = median(voiced_rms)
        for syl, rms in zip(all_syllables, rms_values):
            if rms <= 0:
                continue
            db_adjust = 20 * math.log10(target_rms / rms)
            db_adjust = max(-20, min(20, db_adjust))
            if abs(db_adjust) < 0.5:
                continue
            vol_path = syl.clip_path.with_suffix(".vol.wav")
            adjust_volume(syl.clip_path, vol_path, db_adjust)
            if vol_path.exists():
                syl.clip_path = vol_path
                syl.duration = get_duration(vol_path)

    return all_syllables


def _rubberband_pitch_shift(input_path: Path, output_path: Path, semitones: float):
    """Pitch shift using ffmpeg rubberband filter."""
    ratio = 2 ** (semitones / 12.0)
    subprocess.run(
        [
            "ffmpeg", "-y", "-i", str(input_path),
            "-filter:a", f"rubberband=pitch={ratio:.6f}",
            "-ar", "16000", str(output_path),
        ],
        capture_output=True,
    )
