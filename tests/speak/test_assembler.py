"""Tests for audio assembly from matched syllables."""

import math
import shutil
from pathlib import Path
from unittest.mock import patch, MagicMock

import numpy as np
import pytest

from glottisdale.speak.assembler import (
    plan_timing,
    assemble,
    _group_contiguous_runs,
    _normalize_volume,
    _normalize_pitch,
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


def _write_wav(path: Path, samples: np.ndarray, sr: int = 16000) -> Path:
    """Write a simple 16-bit WAV for testing."""
    import scipy.io.wavfile as wavfile
    clipped = np.clip(samples, -1.0, 1.0)
    int16 = (clipped * 32767).astype(np.int16)
    path.parent.mkdir(parents=True, exist_ok=True)
    wavfile.write(str(path), sr, int16)
    return path


class TestNormalizeVolume:
    def test_equalizes_rms(self, tmp_path):
        """Clips with different volumes should be brought closer together."""
        clips_dir = tmp_path / "clips"
        clips_dir.mkdir()

        # Create clips with different volumes
        sr = 16000
        t = np.linspace(0, 0.1, int(sr * 0.1))
        loud = np.sin(2 * np.pi * 440 * t) * 0.9
        quiet = np.sin(2 * np.pi * 440 * t) * 0.1

        loud_path = _write_wav(clips_dir / "loud.wav", loud, sr)
        quiet_path = _write_wav(clips_dir / "quiet.wav", quiet, sr)

        from glottisdale.analysis import read_wav, compute_rms
        rms_before_loud = compute_rms(read_wav(loud_path)[0])
        rms_before_quiet = compute_rms(read_wav(quiet_path)[0])
        ratio_before = rms_before_loud / rms_before_quiet

        _normalize_volume([loud_path, quiet_path], clips_dir)

        rms_after_loud = compute_rms(read_wav(loud_path)[0])
        rms_after_quiet = compute_rms(read_wav(quiet_path)[0])
        ratio_after = rms_after_loud / rms_after_quiet

        # Volume ratio should be much closer to 1.0 after normalization
        assert ratio_after < ratio_before

    def test_skips_silent_clips(self, tmp_path):
        """Silent clips should not cause errors."""
        clips_dir = tmp_path / "clips"
        clips_dir.mkdir()

        sr = 16000
        silence = np.zeros(int(sr * 0.1))
        tone = np.sin(2 * np.pi * 440 * np.linspace(0, 0.1, int(sr * 0.1))) * 0.5

        silent_path = _write_wav(clips_dir / "silent.wav", silence, sr)
        tone_path = _write_wav(clips_dir / "tone.wav", tone, sr)

        # Should not raise
        _normalize_volume([silent_path, tone_path], clips_dir)


class TestNormalizePitch:
    def test_shifts_outlier_toward_median(self, tmp_path):
        """A clip with very different pitch should be shifted toward the median."""
        clips_dir = tmp_path / "clips"
        clips_dir.mkdir()

        sr = 16000
        duration = 0.1
        t = np.linspace(0, duration, int(sr * duration))

        # Three clips at ~200Hz, one at ~400Hz (outlier)
        paths = []
        for i, freq in enumerate([200, 200, 200, 400]):
            samples = np.sin(2 * np.pi * freq * t) * 0.5
            path = _write_wav(clips_dir / f"clip_{i}.wav", samples, sr)
            paths.append(path)

        _normalize_pitch(paths, clips_dir)

        # The outlier clip should have been modified (pitch shifted)
        from glottisdale.analysis import read_wav, estimate_f0
        samples_after, sr_after = read_wav(paths[3])
        f0_after = estimate_f0(samples_after, sr_after)
        # It should be closer to 200 than to 400 now
        if f0_after is not None:
            assert f0_after < 350  # shifted toward median of 200

    def test_skips_unvoiced(self, tmp_path):
        """Clips with no detectable F0 should not cause errors."""
        clips_dir = tmp_path / "clips"
        clips_dir.mkdir()

        sr = 16000
        noise = np.random.RandomState(42).randn(int(sr * 0.1)) * 0.1
        tone = np.sin(2 * np.pi * 200 * np.linspace(0, 0.1, int(sr * 0.1))) * 0.5

        noise_path = _write_wav(clips_dir / "noise.wav", noise, sr)
        tone_path = _write_wav(clips_dir / "tone.wav", tone, sr)

        # Should not raise
        _normalize_pitch([noise_path, tone_path], clips_dir)


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
            crossfade_ms=40,
            normalize_volume=False,
            normalize_pitch=False,
        )
        assert mock_cut.called
        assert mock_concat.called
        assert result == tmp_path / "speak.wav"

    @patch("glottisdale.speak.assembler.cut_clip")
    @patch("glottisdale.speak.assembler.concatenate_clips")
    def test_assemble_default_crossfade_is_40(self, mock_concat, mock_cut, tmp_path):
        """Default crossfade should be 40ms."""
        entry = _entry(["B", "AH1"], start=0.0, end=0.3)
        matches = [_match(["B", "AH1"], entry)]
        timing = [TimingPlan(target_start=0.0, target_duration=0.3, stretch_factor=1.0)]

        mock_cut.return_value = tmp_path / "clip_0.wav"
        mock_concat.return_value = tmp_path / "speak.wav"

        assemble(
            matches=matches,
            timing=timing,
            output_dir=tmp_path,
            normalize_volume=False,
            normalize_pitch=False,
        )
        # Check that concatenate_clips was called with crossfade_ms=40
        call_kwargs = mock_concat.call_args
        assert call_kwargs[1].get("crossfade_ms", call_kwargs[0][0] if len(call_kwargs[0]) > 0 else None) == 40 or \
               mock_concat.call_args.kwargs.get("crossfade_ms") == 40


class TestGroupContiguousRuns:
    def test_all_contiguous(self):
        """Adjacent source syllables are grouped into one run."""
        entries = [
            _entry(["B", "AH1"], start=0.0, end=0.3, index=0),
            _entry(["K", "AE1"], start=0.3, end=0.6, index=1),
            _entry(["T", "IY1"], start=0.6, end=0.9, index=2),
        ]
        matches = [_match(["B", "AH1"], entries[0], target_index=i) for i, _ in enumerate(entries)]
        for i, e in enumerate(entries):
            matches[i] = _match(["X"], e, target_index=i)
        timing = [TimingPlan(0.0, 0.3, 1.0)] * 3
        runs = _group_contiguous_runs(matches, timing)
        assert len(runs) == 1
        assert runs[0] == [0, 1, 2]

    def test_all_isolated(self):
        """Non-adjacent syllables are each their own run."""
        entries = [
            _entry(["B", "AH1"], start=0.0, end=0.3, index=0),
            _entry(["K", "AE1"], start=2.0, end=2.3, index=5),
            _entry(["T", "IY1"], start=4.0, end=4.3, index=10),
        ]
        matches = [_match(["X"], e, target_index=i) for i, e in enumerate(entries)]
        timing = [TimingPlan(0.0, 0.3, 1.0)] * 3
        runs = _group_contiguous_runs(matches, timing)
        assert len(runs) == 3
        assert all(len(r) == 1 for r in runs)

    def test_mixed_runs(self):
        """Mix of contiguous and non-contiguous creates correct grouping."""
        entries = [
            _entry(["B", "AH1"], start=0.0, end=0.3, index=0),
            _entry(["K", "AE1"], start=0.3, end=0.6, index=1),  # adjacent to 0
            _entry(["T", "IY1"], start=5.0, end=5.3, index=20),  # gap
            _entry(["S", "AH1"], start=5.3, end=5.6, index=21),  # adjacent to 20
        ]
        matches = [_match(["X"], e, target_index=i) for i, e in enumerate(entries)]
        timing = [TimingPlan(0.0, 0.3, 1.0)] * 4
        runs = _group_contiguous_runs(matches, timing)
        assert len(runs) == 2
        assert runs[0] == [0, 1]
        assert runs[1] == [2, 3]

    def test_empty(self):
        runs = _group_contiguous_runs([], [])
        assert runs == []
