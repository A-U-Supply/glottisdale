"""Assemble matched syllables into output audio."""

import logging
import math
import shutil
from dataclasses import dataclass
from pathlib import Path

from glottisdale.analysis import read_wav, compute_rms, estimate_f0
from glottisdale.audio import (
    adjust_volume,
    cut_clip,
    concatenate_clips,
    time_stretch_clip,
    pitch_shift_clip,
    generate_silence,
)
from glottisdale.speak.matcher import MatchResult

logger = logging.getLogger(__name__)


# Pause durations in seconds
_WORD_PAUSE_S = 0.12
_PUNCT_PAUSE_S = 0.35


@dataclass
class TimingPlan:
    """Timing for a single output syllable."""
    target_start: float      # desired start time in output
    target_duration: float   # desired duration in output
    stretch_factor: float    # time-stretch factor to apply (1.0 = no stretch)


def plan_timing(
    matches: list[MatchResult],
    word_boundaries: list[int],
    avg_syllable_dur: float = 0.25,
    reference_timings: list[tuple[float, float]] | None = None,
    timing_strictness: float = 0.8,
) -> list[TimingPlan]:
    """Plan output timing for matched syllables.

    Args:
        matches: Matched syllables in target order.
        word_boundaries: Indices into matches where new words start.
        avg_syllable_dur: Average syllable duration from source (for text mode).
        reference_timings: Optional (start, end) pairs from reference audio.
        timing_strictness: 0.0-1.0, how tightly to follow reference timing.
    """
    word_starts = set(word_boundaries)
    plans = []
    cursor = 0.0

    for i, match in enumerate(matches):
        source_dur = match.entry.end - match.entry.start

        if reference_timings and i < len(reference_timings):
            ref_start, ref_end = reference_timings[i]
            ref_dur = ref_end - ref_start
            # Blend between source duration and reference duration
            target_dur = source_dur + timing_strictness * (ref_dur - source_dur)
            target_start = cursor + timing_strictness * (ref_start - cursor)
        else:
            target_dur = source_dur if source_dur > 0 else avg_syllable_dur
            target_start = cursor

        # Add word-boundary pause
        if i in word_starts and i > 0:
            target_start += _WORD_PAUSE_S

        stretch = target_dur / source_dur if source_dur > 0 else 1.0

        plans.append(TimingPlan(
            target_start=target_start,
            target_duration=target_dur,
            stretch_factor=stretch,
        ))
        cursor = target_start + target_dur

    return plans


def _group_contiguous_runs(
    matches: list[MatchResult],
    timing: list[TimingPlan],
) -> list[list[int]]:
    """Group consecutive matches that come from adjacent source syllables.

    Returns a list of runs, where each run is a list of indices into
    *matches* / *timing*.  Adjacent means same source file and the next
    syllable index in that file.
    """
    if not matches:
        return []

    runs: list[list[int]] = [[0]]

    for i in range(1, len(matches)):
        prev = matches[runs[-1][-1]].entry
        curr = matches[i].entry
        if (curr.source_path == prev.source_path
                and curr.index == prev.index + 1):
            runs[-1].append(i)
        else:
            runs.append([i])

    return runs


def _normalize_volume(clip_paths: list[Path], clips_dir: Path) -> None:
    """Normalize volume across clips to median RMS."""
    import numpy as np

    rms_values = []
    for clip_path in clip_paths:
        if clip_path.exists():
            samples, sr = read_wav(clip_path)
            rms_values.append(compute_rms(samples))

    if not rms_values:
        return

    target_rms = float(np.median(rms_values))
    if target_rms < 1e-6:
        return

    for clip_path in clip_paths:
        if not clip_path.exists():
            continue
        samples, sr = read_wav(clip_path)
        clip_rms = compute_rms(samples)
        if clip_rms < 1e-6:
            continue
        db_adjust = 20 * math.log10(target_rms / clip_rms)
        db_adjust = max(-20, min(20, db_adjust))
        if abs(db_adjust) >= 0.5:
            adjusted = clips_dir / f"vol_{clip_path.name}"
            adjust_volume(clip_path, adjusted, db_adjust)
            shutil.move(adjusted, clip_path)


def _normalize_pitch(
    clip_paths: list[Path], clips_dir: Path, pitch_range: float = 5.0,
) -> None:
    """Normalize pitch across clips toward median F0."""
    import numpy as np

    f0_values = []
    clip_f0s: dict[int, float] = {}
    for i, clip_path in enumerate(clip_paths):
        if clip_path.exists():
            samples, sr = read_wav(clip_path)
            f0 = estimate_f0(samples, sr)
            if f0 is not None:
                f0_values.append(f0)
                clip_f0s[i] = f0

    if not f0_values:
        return

    target_f0 = float(np.median(f0_values))
    logger.info(
        f"Pitch normalization: target F0 = {target_f0:.1f}Hz "
        f"(from {len(f0_values)} voiced clips)"
    )

    for i, clip_path in enumerate(clip_paths):
        f0 = clip_f0s.get(i)
        if f0 is None or not clip_path.exists():
            continue
        semitones_shift = 12 * math.log2(target_f0 / f0)
        semitones_shift = max(-pitch_range, min(pitch_range, semitones_shift))
        if abs(semitones_shift) >= 0.1:
            shifted = clips_dir / f"pitched_{clip_path.name}"
            pitch_shift_clip(clip_path, shifted, semitones_shift)
            shutil.move(shifted, clip_path)


def assemble(
    matches: list[MatchResult],
    timing: list[TimingPlan],
    output_dir: Path,
    crossfade_ms: float = 40,
    pitch_shifts: list[float] | None = None,
    normalize_volume: bool = True,
    normalize_pitch: bool = True,
) -> Path:
    """Cut, stretch, and concatenate matched syllables into output audio.

    Consecutive matches from adjacent positions in the same source file
    are cut as a single clip to preserve natural coarticulation.

    Args:
        matches: Matched syllables in target order.
        timing: Timing plan for each syllable.
        output_dir: Directory for intermediate and output files.
        crossfade_ms: Crossfade between syllables in ms.
        pitch_shifts: Optional per-syllable pitch shift in semitones.
        normalize_volume: Whether to normalize volume across clips.
        normalize_pitch: Whether to normalize pitch across clips.

    Returns:
        Path to the assembled output WAV.
    """
    clips_dir = output_dir / "clips"
    clips_dir.mkdir(parents=True, exist_ok=True)

    runs = _group_contiguous_runs(matches, timing)

    clip_paths: list[Path] = []
    gap_durations: list[float] = []

    for run_idx, run in enumerate(runs):
        first = run[0]
        last = run[-1]

        # Cut the entire contiguous span as one clip
        clip_path = clips_dir / f"clip_{first:04d}.wav"
        cut_clip(
            input_path=Path(matches[first].entry.source_path),
            output_path=clip_path,
            start=matches[first].entry.start,
            end=matches[last].entry.end,
            padding_ms=5,
            fade_ms=3,
        )

        # Time-stretch: compare total source duration to total target duration
        source_dur = matches[last].entry.end - matches[first].entry.start
        target_dur = sum(timing[i].target_duration for i in run)
        stretch = target_dur / source_dur if source_dur > 0 else 1.0

        if abs(stretch - 1.0) > 0.05:
            stretched = clips_dir / f"clip_{first:04d}_stretched.wav"
            time_stretch_clip(clip_path, stretched, stretch)
            clip_path = stretched

        # Pitch-shift (use average of per-syllable shifts for the run)
        if pitch_shifts:
            run_shifts = [
                pitch_shifts[i] for i in run
                if i < len(pitch_shifts) and abs(pitch_shifts[i]) > 0.1
            ]
            if run_shifts:
                avg_shift = sum(run_shifts) / len(run_shifts)
                shifted = clips_dir / f"clip_{first:04d}_pitched.wav"
                pitch_shift_clip(clip_path, shifted, avg_shift)
                clip_path = shifted

        clip_paths.append(clip_path)

        # Gap to next run
        if run_idx < len(runs) - 1:
            this_end = timing[last].target_start + timing[last].target_duration
            next_start = timing[runs[run_idx + 1][0]].target_start
            gap = max(0.0, next_start - this_end) * 1000  # ms
            gap_durations.append(gap)

    # Normalize volume and pitch across clips
    if normalize_volume:
        try:
            _normalize_volume(clip_paths, clips_dir)
        except Exception:
            logger.debug("Volume normalization failed, skipping")

    if normalize_pitch:
        try:
            _normalize_pitch(clip_paths, clips_dir)
        except Exception:
            logger.debug("Pitch normalization failed, skipping")

    # Concatenate all clips
    output_path = output_dir / "speak.wav"
    concatenate_clips(
        clip_paths,
        output_path,
        crossfade_ms=crossfade_ms,
        gap_durations_ms=gap_durations if gap_durations else None,
    )

    return output_path
