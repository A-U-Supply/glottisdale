"""Assemble matched syllables into output audio."""

from dataclasses import dataclass
from pathlib import Path

from glottisdale.audio import (
    cut_clip,
    concatenate_clips,
    time_stretch_clip,
    pitch_shift_clip,
    generate_silence,
)
from glottisdale.speak.matcher import MatchResult


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


def assemble(
    matches: list[MatchResult],
    timing: list[TimingPlan],
    output_dir: Path,
    crossfade_ms: float = 10,
    pitch_shifts: list[float] | None = None,
) -> Path:
    """Cut, stretch, and concatenate matched syllables into output audio.

    Args:
        matches: Matched syllables in target order.
        timing: Timing plan for each syllable.
        output_dir: Directory for intermediate and output files.
        crossfade_ms: Crossfade between syllables in ms.
        pitch_shifts: Optional per-syllable pitch shift in semitones.

    Returns:
        Path to the assembled output WAV.
    """
    clips_dir = output_dir / "clips"
    clips_dir.mkdir(parents=True, exist_ok=True)

    clip_paths: list[Path] = []
    gap_durations: list[float] = []

    for i, (match, plan) in enumerate(zip(matches, timing)):
        # Cut source syllable
        clip_path = clips_dir / f"clip_{i:04d}.wav"
        cut_clip(
            input_path=Path(match.entry.source_path),
            output_path=clip_path,
            start=match.entry.start,
            end=match.entry.end,
            padding_ms=5,
            fade_ms=3,
        )

        # Time-stretch if needed
        if abs(plan.stretch_factor - 1.0) > 0.05:
            stretched = clips_dir / f"clip_{i:04d}_stretched.wav"
            time_stretch_clip(clip_path, stretched, plan.stretch_factor)
            clip_path = stretched

        # Pitch-shift if requested
        if pitch_shifts and i < len(pitch_shifts) and abs(pitch_shifts[i]) > 0.1:
            shifted = clips_dir / f"clip_{i:04d}_pitched.wav"
            pitch_shift_clip(clip_path, shifted, pitch_shifts[i])
            clip_path = shifted

        clip_paths.append(clip_path)

        # Compute gap to next syllable
        if i < len(timing) - 1:
            gap = timing[i + 1].target_start - (plan.target_start + plan.target_duration)
            gap_durations.append(max(0.0, gap) * 1000)  # convert to ms

    # Concatenate all clips
    output_path = output_dir / "speak.wav"
    concatenate_clips(
        clip_paths,
        output_path,
        crossfade_ms=crossfade_ms,
        gap_durations_ms=gap_durations if gap_durations else None,
    )

    return output_path
