"""Speak pipeline: reconstruct target text using source audio syllables."""

import json
import logging
from pathlib import Path

import shutil

from glottisdale.analysis import read_wav, write_wav, find_room_tone
from glottisdale.audio import (
    cut_clip,
    concatenate_clips,
    extract_audio,
    mix_audio,
)
from glottisdale.collage.align import get_aligner
from glottisdale.types import Result

logger = logging.getLogger(__name__)


def process(
    input_paths: list[Path],
    output_dir: str | Path,
    text: str | None = None,
    reference: Path | None = None,
    match_unit: str = "syllable",
    pitch_correct: bool = True,
    timing_strictness: float = 0.8,
    crossfade_ms: float = 40,
    normalize_volume: bool = True,
    whisper_model: str = "base",
    aligner: str = "auto",
    seed: int | None = None,
    verbose: bool = False,
    use_cache: bool = True,
) -> Result:
    """Run the speak pipeline.

    Args:
        input_paths: Source audio files (voice bank).
        output_dir: Output directory for this run.
        text: Target text to speak (text mode).
        reference: Reference audio file for text + timing (reference mode).
        match_unit: "syllable" or "phoneme".
        pitch_correct: Whether to apply pitch correction.
        timing_strictness: How tightly to follow reference timing (0.0-1.0).
        crossfade_ms: Crossfade between syllables in ms.
        normalize_volume: Whether to normalize volume across syllables.
        whisper_model: Whisper model size.
        aligner: Alignment backend.
        seed: RNG seed.
        verbose: Show warnings.
        use_cache: Use file-based caching.
    """
    from glottisdale.speak.syllable_bank import build_bank
    from glottisdale.speak.target_text import text_to_syllables, word_boundaries_from_syllables
    from glottisdale.speak.matcher import match_syllables, match_phonemes
    from glottisdale.speak.assembler import plan_timing, assemble

    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    # --- 1. Build source syllable bank ---
    logger.info("Building source syllable bank")
    alignment_engine = get_aligner(aligner, whisper_model=whisper_model, verbose=verbose)
    all_bank_entries = []

    for input_path in input_paths:
        audio_path = output_dir / f"{input_path.stem}_16k.wav"
        extract_audio(input_path, audio_path)

        input_hash = str(input_path)
        result = alignment_engine.process(audio_path, audio_hash=input_hash, use_cache=use_cache)
        source_syllables = result["syllables"]

        entries = build_bank(source_syllables, source_path=str(audio_path))
        all_bank_entries.extend(entries)
        logger.info(f"  {input_path.name}: {len(entries)} syllables")

    logger.info(f"Syllable bank: {len(all_bank_entries)} total entries")

    # Write syllable bank JSON
    bank_json = output_dir / "syllable-bank.json"
    bank_json.write_text(json.dumps(
        {"entries": [e.to_dict() for e in all_bank_entries]},
        indent=2,
    ))

    # --- 2. Get target text ---
    target_text = text
    reference_timings = None

    if reference is not None:
        logger.info(f"Transcribing reference audio: {reference}")
        ref_audio = output_dir / "reference_16k.wav"
        extract_audio(reference, ref_audio)
        ref_result = alignment_engine.process(ref_audio, audio_hash=str(reference), use_cache=use_cache)
        target_text = ref_result["text"]
        # Extract syllable-level timing from reference
        ref_syllables = ref_result["syllables"]
        reference_timings = [(s.start, s.end) for s in ref_syllables]
        logger.info(f"Reference text: {target_text}")

    if not target_text:
        raise ValueError("Either --text or --reference must be provided")

    # --- 3. Convert target text to syllables ---
    logger.info(f"Target text: {target_text}")
    target_syls = text_to_syllables(target_text)
    word_bounds = word_boundaries_from_syllables(target_syls)
    logger.info(f"Target: {len(target_syls)} syllables, {len(word_bounds)} words")

    # --- 4. Match ---
    logger.info(f"Matching ({match_unit} mode)")
    if match_unit == "phoneme":
        all_phonemes = []
        for ts in target_syls:
            all_phonemes.extend(ts.phonemes)
        matches = match_phonemes(all_phonemes, all_bank_entries)
    else:
        target_phoneme_lists = [ts.phonemes for ts in target_syls]
        target_stresses = [ts.stress for ts in target_syls]
        matches = match_syllables(target_phoneme_lists, all_bank_entries, target_stresses)

    # --- 5. Plan timing ---
    avg_dur = (
        sum(e.duration for e in all_bank_entries) / len(all_bank_entries)
        if all_bank_entries else 0.25
    )
    timing = plan_timing(
        matches, word_bounds,
        avg_syllable_dur=avg_dur,
        reference_timings=reference_timings,
        timing_strictness=timing_strictness,
    )

    # --- 6. Assemble ---
    logger.info("Assembling output audio")
    output_path = assemble(
        matches=matches,
        timing=timing,
        output_dir=output_dir,
        crossfade_ms=crossfade_ms,
        normalize_volume=normalize_volume,
        normalize_pitch=pitch_correct,
    )

    # --- 7. Room tone bed ---
    try:
        source_audio_paths = [
            output_dir / f"{p.stem}_16k.wav" for p in input_paths
        ]
        for source_audio in source_audio_paths:
            if not source_audio.exists():
                continue
            samples, sr = read_wav(source_audio)
            rt = find_room_tone(samples, sr)
            if rt is not None:
                rt_start, rt_end = rt
                rt_samples = samples[int(rt_start * sr):int(rt_end * sr)]
                rt_path = output_dir / "room_tone.wav"
                write_wav(rt_path, rt_samples, sr)
                mixed_path = output_dir / "speak_mixed.wav"
                mix_audio(output_path, rt_path, mixed_path, secondary_volume_db=-40)
                shutil.move(mixed_path, output_path)
                logger.info(
                    f"Room tone from {source_audio.name}: "
                    f"{rt_start:.1f}-{rt_end:.1f}s"
                )
                break  # use first source with room tone
    except Exception:
        logger.debug("Room tone mixing failed, skipping")

    # --- 8. Write match log ---
    match_log = output_dir / "match-log.json"
    match_log.write_text(json.dumps(
        {
            "target_text": target_text,
            "match_unit": match_unit,
            "matches": [m.to_dict() for m in matches],
        },
        indent=2,
    ))
    logger.info(f"Output: {output_path}")

    return Result(
        clips=[],
        concatenated=output_path,
        transcript=target_text,
        manifest={"match_unit": match_unit, "source_count": len(input_paths)},
    )
