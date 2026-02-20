"""Integration tests for glottisdale sing pipeline."""
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest
import pretty_midi

from glottisdale.sing.midi_parser import parse_midi
from glottisdale.sing.vocal_mapper import plan_note_mapping


def _make_test_midi(notes, tempo=120):
    """Create a MIDI file with the given notes for testing."""
    mid = pretty_midi.PrettyMIDI(initial_tempo=tempo)
    inst = pretty_midi.Instrument(program=0)
    for pitch, start, end, velocity in notes:
        inst.notes.append(pretty_midi.Note(
            velocity=velocity, pitch=pitch, start=start, end=end
        ))
    mid.instruments.append(inst)
    path = Path(tempfile.mktemp(suffix=".mid"))
    mid.write(str(path))
    return path


def test_sing_cli_help():
    """glottisdale sing --help should work."""
    result = subprocess.run(
        [sys.executable, "-m", "glottisdale", "sing", "--help"],
        capture_output=True, text=True,
    )
    assert result.returncode == 0
    assert "--midi" in result.stdout


def test_collage_cli_help():
    """glottisdale collage --help should work."""
    result = subprocess.run(
        [sys.executable, "-m", "glottisdale", "collage", "--help"],
        capture_output=True, text=True,
    )
    assert result.returncode == 0
    assert "--target-duration" in result.stdout


def test_parse_and_map_roundtrip():
    """Parse a MIDI file and plan note mapping end-to-end."""
    path = _make_test_midi([
        (60, 0.0, 0.5, 100),
        (64, 0.5, 1.0, 90),
        (67, 1.0, 1.5, 80),
        (72, 1.5, 2.5, 100),
    ], tempo=120)
    try:
        track = parse_midi(path)
        assert len(track.notes) == 4

        mappings = plan_note_mapping(track.notes, pool_size=8, seed=42)
        assert len(mappings) == 4
        for m in mappings:
            assert m.duration_class in ("short", "medium", "long")
            for idx in m.syllable_indices:
                assert 0 <= idx < 8
    finally:
        path.unlink(missing_ok=True)
