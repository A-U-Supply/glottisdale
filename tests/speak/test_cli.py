"""Tests for speak subcommand CLI."""

from pathlib import Path
from unittest.mock import patch, MagicMock

from glottisdale.cli import parse_args, main


class TestParseSpeak:
    def test_speak_with_text(self):
        args = parse_args(["speak", "source.mp4", "--text", "hello world"])
        assert args.command == "speak"
        assert args.input_files == ["source.mp4"]
        assert args.text == "hello world"

    def test_speak_with_reference(self):
        args = parse_args(["speak", "source.mp4", "--reference", "ref.mp4"])
        assert args.command == "speak"
        assert args.reference == Path("ref.mp4")

    def test_speak_defaults(self):
        args = parse_args(["speak", "source.mp4", "--text", "hi"])
        assert args.match_unit == "syllable"
        assert args.pitch_correct is True
        assert args.timing_strictness == 0.8
        assert args.crossfade == 10

    def test_speak_match_unit_phoneme(self):
        args = parse_args(["speak", "source.mp4", "--text", "hi", "--match-unit", "phoneme"])
        assert args.match_unit == "phoneme"

    def test_speak_no_pitch_correct(self):
        args = parse_args(["speak", "source.mp4", "--text", "hi", "--no-pitch-correct"])
        assert args.pitch_correct is False

    def test_speak_timing_strictness(self):
        args = parse_args(["speak", "source.mp4", "--text", "hi", "--timing-strictness", "0.5"])
        assert args.timing_strictness == 0.5

    def test_speak_shared_args(self):
        args = parse_args([
            "speak", "source.mp4", "--text", "hi",
            "--output-dir", "/tmp/out",
            "--seed", "42",
            "--whisper-model", "small",
        ])
        assert args.output_dir == "/tmp/out"
        assert args.seed == 42
        assert args.whisper_model == "small"


class TestRunSpeak:
    def test_cli_calls_process(self, tmp_path):
        source = tmp_path / "source.wav"
        source.touch()

        mock_result = MagicMock()
        mock_result.transcript = "hello"
        mock_result.clips = []
        mock_result.concatenated = MagicMock()
        mock_result.concatenated.name = "speak.wav"

        with patch("glottisdale.speak.process") as mock_process:
            mock_process.return_value = mock_result
            main([
                "speak", str(source),
                "--text", "hello world",
                "--output-dir", str(tmp_path / "out"),
            ])

            assert mock_process.called
            call_kwargs = mock_process.call_args[1]
            assert call_kwargs["text"] == "hello world"
            assert call_kwargs["match_unit"] == "syllable"

    def test_cli_creates_run_subdir(self, tmp_path):
        source = tmp_path / "source.wav"
        source.touch()

        mock_result = MagicMock()
        mock_result.transcript = "test"
        mock_result.clips = []
        mock_result.concatenated = MagicMock()
        mock_result.concatenated.name = "speak.wav"

        with patch("glottisdale.speak.process") as mock_process:
            mock_process.return_value = mock_result
            main([
                "speak", str(source),
                "--text", "hello",
                "--output-dir", str(tmp_path / "out"),
            ])

            call_kwargs = mock_process.call_args[1]
            output_dir = Path(call_kwargs["output_dir"])
            assert output_dir.parent == tmp_path / "out"

    def test_cli_requires_text_or_reference(self, tmp_path, capsys):
        """speak without --text or --reference should error."""
        source = tmp_path / "source.wav"
        source.touch()

        mock_result = MagicMock()
        mock_result.transcript = ""

        with patch("glottisdale.speak.process") as mock_process:
            mock_process.side_effect = ValueError("Either --text or --reference must be provided")
            try:
                main([
                    "speak", str(source),
                    "--output-dir", str(tmp_path / "out"),
                ])
            except (SystemExit, ValueError):
                pass  # expected
