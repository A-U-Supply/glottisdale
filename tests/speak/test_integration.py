"""Integration tests for the speak pipeline."""

import json
from pathlib import Path
from unittest.mock import patch, MagicMock

from glottisdale.types import Phoneme, Syllable


def _make_syllables() -> list[Syllable]:
    """Create mock source syllables."""
    return [
        Syllable(
            phonemes=[Phoneme("DH", 0.0, 0.1), Phoneme("AH0", 0.1, 0.2)],
            start=0.0, end=0.2, word="the", word_index=0,
        ),
        Syllable(
            phonemes=[Phoneme("K", 0.2, 0.3), Phoneme("AE1", 0.3, 0.45), Phoneme("T", 0.45, 0.5)],
            start=0.2, end=0.5, word="cat", word_index=1,
        ),
        Syllable(
            phonemes=[Phoneme("S", 0.5, 0.6), Phoneme("AE1", 0.6, 0.7), Phoneme("T", 0.7, 0.8)],
            start=0.5, end=0.8, word="sat", word_index=2,
        ),
        Syllable(
            phonemes=[Phoneme("B", 0.8, 0.9), Phoneme("AH1", 0.9, 1.0), Phoneme("T", 1.0, 1.1)],
            start=0.8, end=1.1, word="but", word_index=3,
        ),
    ]


class TestSpeakProcess:
    @patch("glottisdale.speak.assembler.concatenate_clips")
    @patch("glottisdale.speak.assembler.cut_clip")
    @patch("glottisdale.speak.get_aligner")
    @patch("glottisdale.speak.extract_audio")
    def test_text_mode_end_to_end(
        self, mock_extract, mock_aligner, mock_cut, mock_concat, tmp_path
    ):
        from glottisdale.speak import process

        # Set up mocks
        source = tmp_path / "source.wav"
        source.touch()
        audio_path = tmp_path / "extracted.wav"
        audio_path.touch()
        mock_extract.return_value = audio_path

        aligner = MagicMock()
        aligner.process.return_value = {
            "text": "the cat sat but",
            "syllables": _make_syllables(),
        }
        mock_aligner.return_value = aligner

        def fake_cut(**kw):
            kw["output_path"].touch()
            return kw["output_path"]

        def fake_concat(clip_paths, output_path, **kw):
            output_path.touch()
            return output_path

        mock_cut.side_effect = fake_cut
        mock_concat.side_effect = fake_concat

        result = process(
            input_paths=[source],
            output_dir=tmp_path / "out",
            text="the cat",
            whisper_model="tiny",
        )

        assert result.concatenated.exists()
        # Check match log was written
        match_log = tmp_path / "out" / "match-log.json"
        assert match_log.exists()
        log_data = json.loads(match_log.read_text())
        assert len(log_data["matches"]) > 0

        # Check syllable bank was written
        bank_file = tmp_path / "out" / "syllable-bank.json"
        assert bank_file.exists()
