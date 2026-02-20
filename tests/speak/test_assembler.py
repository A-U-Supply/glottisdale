"""Tests for audio assembly from matched syllables."""

from pathlib import Path
from unittest.mock import patch, MagicMock

from glottisdale.speak.assembler import (
    plan_timing,
    assemble,
    TimingPlan,
)
from glottisdale.speak.matcher import MatchResult
from glottisdale.speak.syllable_bank import SyllableEntry


def _entry(phonemes: list[str], start: float = 0.0, end: float = 0.3,
           index: int = 0) -> SyllableEntry:
    return SyllableEntry(
        phoneme_labels=phonemes, start=start, end=end,
        word="test", stress=1, source_path="test.wav", index=index,
    )


def _match(target: list[str], entry: SyllableEntry, distance: int = 0,
           target_index: int = 0) -> MatchResult:
    return MatchResult(
        target_phonemes=target, entry=entry,
        distance=distance, target_index=target_index,
    )


class TestPlanTiming:
    def test_text_mode_uniform_spacing(self):
        """Without reference timing, syllables are spaced uniformly."""
        matches = [
            _match(["B", "AH1"], _entry(["B", "AH1"], start=0.0, end=0.3)),
            _match(["K", "AE1"], _entry(["K", "AE1"], start=0.5, end=0.8)),
        ]
        word_boundaries = [0, 2]  # one word with 2 syllables
        plan = plan_timing(matches, word_boundaries, avg_syllable_dur=0.25)
        assert len(plan) == 2
        # First syllable starts near 0
        assert plan[0].target_start >= 0.0
        # Second follows after first
        assert plan[1].target_start > plan[0].target_start

    def test_word_boundary_adds_pause(self):
        """Pauses inserted at word boundaries."""
        matches = [
            _match(["B", "AH1"], _entry(["B", "AH1"]), target_index=0),
            _match(["K", "AE1"], _entry(["K", "AE1"]), target_index=1),
        ]
        word_boundaries = [0, 1]  # each syllable is its own word
        plan = plan_timing(matches, word_boundaries, avg_syllable_dur=0.25)
        gap = plan[1].target_start - (plan[0].target_start + plan[0].target_duration)
        assert gap > 0  # there should be a pause between words

    def test_reference_timing_strictness_1(self):
        """With strictness=1.0, output timing matches reference exactly."""
        matches = [_match(["B", "AH1"], _entry(["B", "AH1"]))]
        word_boundaries = [0]
        ref_timings = [(0.5, 0.8)]  # reference says syllable at 0.5-0.8
        plan = plan_timing(
            matches, word_boundaries,
            reference_timings=ref_timings, timing_strictness=1.0,
        )
        assert abs(plan[0].target_start - 0.5) < 0.01
        assert abs(plan[0].target_duration - 0.3) < 0.01


class TestAssemble:
    @patch("glottisdale.speak.assembler.cut_clip")
    @patch("glottisdale.speak.assembler.concatenate_clips")
    def test_assemble_produces_output(self, mock_concat, mock_cut, tmp_path):
        """Assembly cuts clips and concatenates them."""
        entry = _entry(["B", "AH1"], start=0.0, end=0.3)
        matches = [_match(["B", "AH1"], entry)]
        timing = [TimingPlan(target_start=0.0, target_duration=0.3, stretch_factor=1.0)]

        mock_cut.return_value = tmp_path / "clip_0.wav"
        mock_concat.return_value = tmp_path / "speak.wav"

        result = assemble(
            matches=matches,
            timing=timing,
            output_dir=tmp_path,
            crossfade_ms=10,
        )
        assert mock_cut.called
        assert mock_concat.called
        assert result == tmp_path / "speak.wav"
