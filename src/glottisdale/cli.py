"""CLI entrypoint for glottisdale — subcommand dispatcher."""

import argparse
import logging
import sys
import warnings
from pathlib import Path


def _add_shared_args(parser: argparse.ArgumentParser) -> None:
    """Add arguments shared between collage and sing subcommands."""
    parser.add_argument(
        "input_files", nargs="*", default=[],
        help="Local video/audio files to process.",
    )
    parser.add_argument("--output-dir", default="./glottisdale-output",
                        help="Output directory (default: ./glottisdale-output)")
    parser.add_argument("--target-duration", type=float, default=30.0,
                        help="Target total duration in seconds (default: 30)")
    parser.add_argument("--whisper-model", default="base",
                        choices=["tiny", "base", "small", "medium"],
                        help="Whisper model size (default: base)")
    parser.add_argument("--seed", type=int, default=None,
                        help="RNG seed for reproducible output")
    parser.add_argument("-v", "--verbose", action="store_true", default=False,
                        help="Show all warnings from dependencies (default: quiet)")
    parser.add_argument("--no-cache", action="store_true", default=False,
                        help="Disable file-based caching of extraction, transcription, and alignment")


def _add_collage_args(parser: argparse.ArgumentParser) -> None:
    """Add arguments specific to the collage subcommand."""
    # Core options — prosodic grouping
    parser.add_argument("--syllables-per-word", default="1-4",
                        help="Syllables per word: '3', or '1-4' for variable (default: 1-4)")
    parser.add_argument("--syllables-per-clip", default=None,
                        help=argparse.SUPPRESS)  # deprecated alias
    parser.add_argument("--crossfade", type=float, default=30,
                        help="Crossfade between syllables in a word, ms (default: 30, 0=hard cut)")
    parser.add_argument("--padding", type=float, default=25,
                        help="Padding around syllable cuts in ms (default: 25)")
    parser.add_argument("--words-per-phrase", default="3-5",
                        help="Words per phrase: '4', or '3-5' (default: 3-5)")
    parser.add_argument("--phrases-per-sentence", default="2-3",
                        help="Phrases per sentence group: '2', or '2-3' (default: 2-3)")
    parser.add_argument("--phrase-pause", default="400-700",
                        help="Silence between phrases in ms: '500' or '400-700' (default: 400-700)")
    parser.add_argument("--sentence-pause", default="800-1200",
                        help="Silence between sentences in ms: '1000' or '800-1200' (default: 800-1200)")
    parser.add_argument("--word-crossfade", type=float, default=50,
                        help="Crossfade between words in a phrase, ms (default: 50)")
    parser.add_argument("--gap", default=None,
                        help=argparse.SUPPRESS)  # deprecated alias for --phrase-pause
    parser.add_argument("--aligner", default="auto",
                        choices=["auto", "default", "bfa"],
                        help="Alignment backend (default: auto)")
    parser.add_argument("--bfa-device", default="cpu",
                        choices=["cpu", "cuda"],
                        help="Device for BFA model inference (default: cpu)")

    # Audio polish options
    parser.add_argument("--noise-level", type=float, default=-40,
                        help="Pink noise bed level in dB, 0 to disable (default: -40)")
    parser.add_argument("--room-tone", action=argparse.BooleanOptionalAction, default=True,
                        help="Extract room tone for gaps (default: enabled)")
    parser.add_argument("--pitch-normalize", action=argparse.BooleanOptionalAction, default=True,
                        help="Normalize pitch across syllables (default: enabled)")
    parser.add_argument("--pitch-range", type=float, default=5,
                        help="Max pitch shift in semitones (default: 5)")
    parser.add_argument("--breaths", action=argparse.BooleanOptionalAction, default=True,
                        help="Insert breath sounds at phrase boundaries (default: enabled)")
    parser.add_argument("--breath-probability", type=float, default=0.6,
                        help="Probability of breath at each phrase boundary (default: 0.6)")
    parser.add_argument("--volume-normalize", action=argparse.BooleanOptionalAction, default=True,
                        help="RMS-normalize syllable clips (default: enabled)")
    parser.add_argument("--prosodic-dynamics", action=argparse.BooleanOptionalAction, default=True,
                        help="Apply phrase-level volume envelope (default: enabled)")

    # Time stretch options (all off by default)
    parser.add_argument("--speed", type=float, default=None,
                        help="Global speed factor: 0.5=half speed, 2.0=double (default: off)")
    parser.add_argument("--random-stretch", type=float, default=None,
                        help="Probability (0-1) that a syllable gets stretched (default: off)")
    parser.add_argument("--alternating-stretch", type=int, default=None,
                        help="Stretch every Nth syllable (default: off)")
    parser.add_argument("--boundary-stretch", type=int, default=None,
                        help="Stretch first/last N syllables per word (default: off)")
    parser.add_argument("--word-stretch", type=float, default=None,
                        help="Probability (0-1) that all syllables in a word get stretched (default: off)")
    parser.add_argument("--stretch-factor", default="2.0",
                        help="Stretch amount: '2.0' or '1.5-3.0' for random range (default: 2.0)")

    # Word repeat options (all off by default)
    parser.add_argument("--repeat-weight", type=float, default=None,
                        help="Probability (0-1) that a word gets repeated (default: off)")
    parser.add_argument("--repeat-count", default="1-2",
                        help="Extra copies per repeated word: '2' or '1-3' (default: 1-2)")
    parser.add_argument("--repeat-style", default="exact",
                        choices=["exact", "resample"],
                        help="Repeat style: exact (duplicate WAV) or resample (default: exact)")

    # Stutter options (all off by default)
    parser.add_argument("--stutter", type=float, default=None,
                        help="Probability (0-1) that a syllable gets stuttered (default: off)")
    parser.add_argument("--stutter-count", default="1-2",
                        help="Extra copies of stuttered syllable: '2' or '1-3' (default: 1-2)")


def _add_sing_args(parser: argparse.ArgumentParser) -> None:
    """Add arguments specific to the sing subcommand."""
    parser.add_argument(
        "--midi", type=Path, required=True,
        help="Directory containing MIDI files (melody.mid, etc.)",
    )
    parser.add_argument(
        "--vibrato", "--no-vibrato", action=argparse.BooleanOptionalAction,
        default=True,
        help="Toggle vibrato (default: enabled)",
    )
    parser.add_argument(
        "--chorus", "--no-chorus", action=argparse.BooleanOptionalAction,
        default=True,
        help="Toggle chorus (default: enabled)",
    )
    parser.add_argument(
        "--drift-range", type=float, default=2.0,
        help="Max semitone drift from melody (default: 2.0)",
    )
    parser.add_argument(
        "--max-videos", type=int, default=5,
        help="Max source videos (Slack mode, default: 5)",
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse CLI arguments with subcommands."""
    parser = argparse.ArgumentParser(
        prog="glottisdale",
        description="Syllable-level audio collage and vocal MIDI mapping tool",
    )

    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # Collage subcommand
    collage_parser = subparsers.add_parser(
        "collage",
        help="Syllable-level audio collage",
        description="Create a syllable-level audio collage from speech",
    )
    _add_shared_args(collage_parser)
    _add_collage_args(collage_parser)

    # Sing subcommand
    sing_parser = subparsers.add_parser(
        "sing",
        help="Map syllables to MIDI melody ('drunk choir')",
        description="Map syllable clips to MIDI melody notes for vocal MIDI mapping",
    )
    _add_shared_args(sing_parser)
    _add_sing_args(sing_parser)

    args = parser.parse_args(argv)

    if args.command is None:
        parser.print_help()
        sys.exit(1)

    # Backward compat for collage
    if args.command == "collage":
        if getattr(args, 'syllables_per_clip', None) is not None:
            print("Warning: --syllables-per-clip is deprecated, use --syllables-per-word",
                  file=sys.stderr)
            args.syllables_per_word = args.syllables_per_clip
        if getattr(args, 'gap', None) is not None:
            print("Warning: --gap is deprecated, use --phrase-pause", file=sys.stderr)
            args.phrase_pause = args.gap

    return args


def _run_collage(args: argparse.Namespace) -> None:
    """Run the collage pipeline."""
    from glottisdale.collage import process

    input_paths = [Path(f) for f in args.input_files]
    for p in input_paths:
        if not p.exists():
            print(f"Error: file not found: {p}", file=sys.stderr)
            sys.exit(1)

    if not input_paths:
        print("Error: at least one input file is required", file=sys.stderr)
        sys.exit(1)

    result = process(
        input_paths=input_paths,
        output_dir=args.output_dir,
        syllables_per_clip=args.syllables_per_word,
        target_duration=args.target_duration,
        crossfade_ms=args.crossfade,
        padding_ms=args.padding,
        words_per_phrase=args.words_per_phrase,
        phrases_per_sentence=args.phrases_per_sentence,
        phrase_pause=args.phrase_pause,
        sentence_pause=args.sentence_pause,
        word_crossfade_ms=args.word_crossfade,
        aligner=args.aligner,
        whisper_model=args.whisper_model,
        bfa_device=args.bfa_device,
        seed=args.seed,
        noise_level_db=args.noise_level,
        room_tone=args.room_tone,
        pitch_normalize=args.pitch_normalize,
        pitch_range=args.pitch_range,
        breaths=args.breaths,
        breath_probability=args.breath_probability,
        volume_normalize=args.volume_normalize,
        prosodic_dynamics=args.prosodic_dynamics,
        speed=args.speed,
        random_stretch=args.random_stretch,
        alternating_stretch=args.alternating_stretch,
        boundary_stretch=args.boundary_stretch,
        word_stretch=args.word_stretch,
        stretch_factor=args.stretch_factor,
        repeat_weight=args.repeat_weight,
        repeat_count=args.repeat_count,
        repeat_style=args.repeat_style,
        stutter=args.stutter,
        stutter_count=args.stutter_count,
        verbose=args.verbose,
        use_cache=not args.no_cache,
    )

    print(f"Processed {len(args.input_files)} source file(s)")
    print(f"Transcript: {result.transcript}")
    print(f"Selected {len(result.clips)} clips")
    print(f"Output:")
    for clip in result.clips:
        print(f"  {clip.output_path.name}")
    print(f"  {result.concatenated.name}")
    print(f"  clips.zip")


def _run_sing(args: argparse.Namespace) -> None:
    """Run the sing pipeline."""
    from statistics import median

    from glottisdale.sing.midi_parser import parse_midi
    from glottisdale.sing.syllable_prep import prepare_syllables
    from glottisdale.sing.vocal_mapper import plan_note_mapping, render_vocal_track
    from glottisdale.sing.mixer import mix_tracks

    input_paths = [Path(f) for f in args.input_files]
    for p in input_paths:
        if not p.exists():
            print(f"Error: file not found: {p}", file=sys.stderr)
            sys.exit(1)

    if not input_paths:
        print("Error: at least one input audio file is required", file=sys.stderr)
        sys.exit(1)

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    work_dir = output_dir / "work"
    work_dir.mkdir(parents=True, exist_ok=True)

    logger = logging.getLogger("glottisdale.sing")

    # Load MIDI — parse the melody track
    melody_path = args.midi / "melody.mid"
    logger.info(f"Parsing MIDI: {melody_path}")
    track = parse_midi(melody_path)
    logger.info(f"Melody: {len(track.notes)} notes, {track.tempo} BPM, {track.total_duration:.1f}s")

    # Prepare syllables from audio files
    logger.info(f"Preparing syllables from {len(input_paths)} audio file(s)")
    syllables = prepare_syllables(
        input_paths, work_dir, args.whisper_model,
        use_cache=not args.no_cache,
    )
    logger.info(f"Prepared {len(syllables)} syllables")

    # Compute median F0
    voiced_f0 = [s.f0 for s in syllables if s.f0 and s.f0 > 0]
    median_f0 = median(voiced_f0) if voiced_f0 else 220.0
    logger.info(f"Median F0: {median_f0:.1f} Hz")

    # Plan note mapping
    mappings = plan_note_mapping(
        track.notes, len(syllables),
        seed=args.seed, drift_range=args.drift_range,
    )
    logger.info(f"Planned {len(mappings)} note mappings")

    # Render vocal track
    logger.info("Rendering vocal track")
    acappella = render_vocal_track(
        mappings, syllables, work_dir, median_f0, args.target_duration,
    )
    logger.info(f"Vocal track: {acappella}")

    # Mix with backing
    logger.info("Mixing tracks")
    full_mix, acappella_out = mix_tracks(acappella, args.midi, output_dir)
    logger.info(f"Output: {full_mix}")
    logger.info(f"A cappella: {acappella_out}")


def main(argv: list[str] | None = None) -> None:
    """CLI entrypoint."""
    args = parse_args(argv)

    logging.basicConfig(level=logging.INFO, format="%(name)s %(levelname)s: %(message)s")

    if not args.verbose:
        # Silence noisy third-party warnings
        warnings.filterwarnings("ignore", message="FP16 is not supported on CPU")
        warnings.filterwarnings("ignore", message=".*backend.*parameter.*TorchCodec")
        warnings.filterwarnings("ignore", message="Duplicate name")
        # phonemizer's get_logger() resets handlers and level on each call,
        # so setLevel alone gets clobbered. A filter on the logger persists.
        logging.getLogger("phonemizer").addFilter(lambda record: record.levelno >= logging.ERROR)
        logging.getLogger("huggingface_hub").setLevel(logging.ERROR)
        logging.getLogger("httpx").setLevel(logging.ERROR)
        logging.getLogger("bournemouth_aligner").setLevel(logging.ERROR)

    if args.command == "collage":
        _run_collage(args)
    elif args.command == "sing":
        _run_sing(args)


if __name__ == "__main__":
    main()
