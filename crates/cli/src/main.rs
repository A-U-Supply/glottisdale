//! Glottisdale CLI — syllable-level audio collage, speak, and sing.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use glottisdale_core::audio::io::{extract_audio, read_wav};
use glottisdale_core::collage::stretch::{StretchConfig, parse_stretch_factor};
use glottisdale_core::language::align::get_aligner;
use glottisdale_core::names::create_run_dir;

// ─── Top-level CLI ───────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "glottisdale",
    about = "Syllable-level audio collage and vocal MIDI mapping tool",
    version,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Syllable-level audio collage
    Collage(CollageArgs),
    /// Map syllables to MIDI melody ("drunk choir")
    Sing(SingArgs),
    /// Reconstruct text using source audio syllables
    Speak(SpeakArgs),
}

// ─── Shared arguments (embedded in each subcommand) ──────────────

#[derive(Parser, Debug)]
struct SharedArgs {
    /// Input audio/video files to process
    #[arg(required = true)]
    input_files: Vec<PathBuf>,

    /// Output directory
    #[arg(long, default_value = "./glottisdale-output")]
    output_dir: PathBuf,

    /// Target total duration in seconds
    #[arg(long, default_value_t = 30.0)]
    target_duration: f64,

    /// Whisper model size
    #[arg(long, default_value = "base", value_parser = ["tiny", "base", "small", "medium"])]
    whisper_model: String,

    /// RNG seed for reproducible output
    #[arg(long)]
    seed: Option<u64>,

    /// Show verbose output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Disable file-based caching
    #[arg(long, default_value_t = false)]
    no_cache: bool,

    /// Custom run name (default: auto-generated)
    #[arg(long)]
    run_name: Option<String>,
}

// ─── Collage ─────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(about = "Create a syllable-level audio collage from speech")]
#[command(allow_negative_numbers = true)]
struct CollageArgs {
    #[command(flatten)]
    shared: SharedArgs,

    // -- Prosodic grouping --
    /// Syllables per word: "3" or "1-4"
    #[arg(long, default_value = "1-4")]
    syllables_per_word: String,

    /// Crossfade between syllables in a word (ms)
    #[arg(long, default_value_t = 30.0)]
    crossfade: f64,

    /// Padding around syllable cuts (ms)
    #[arg(long, default_value_t = 25.0)]
    padding: f64,

    /// Words per phrase: "4" or "3-5"
    #[arg(long, default_value = "3-5")]
    words_per_phrase: String,

    /// Phrases per sentence: "2" or "2-3"
    #[arg(long, default_value = "2-3")]
    phrases_per_sentence: String,

    /// Silence between phrases (ms): "500" or "400-700"
    #[arg(long, default_value = "400-700")]
    phrase_pause: String,

    /// Silence between sentences (ms): "1000" or "800-1200"
    #[arg(long, default_value = "800-1200")]
    sentence_pause: String,

    /// Crossfade between words (ms)
    #[arg(long, default_value_t = 50.0)]
    word_crossfade: f64,

    /// Alignment backend
    #[arg(long, default_value = "auto", value_parser = ["auto", "default", "bfa"])]
    aligner: String,

    /// BFA inference device
    #[arg(long, default_value = "cpu", value_parser = ["cpu", "cuda"])]
    bfa_device: String,

    // -- Audio polish --
    /// Pink noise bed level in dB (0 to disable)
    #[arg(long, default_value_t = -40.0, allow_hyphen_values = true)]
    noise_level: f64,

    /// Extract room tone for gaps [use --no-room-tone to disable]
    #[arg(long, default_value_t = true)]
    room_tone: bool,

    /// Disable room tone extraction
    #[arg(long, overrides_with = "room_tone")]
    no_room_tone: bool,

    /// Normalize pitch across syllables [use --no-pitch-normalize to disable]
    #[arg(long, default_value_t = true)]
    pitch_normalize: bool,

    /// Disable pitch normalization
    #[arg(long, overrides_with = "pitch_normalize")]
    no_pitch_normalize: bool,

    /// Max pitch shift in semitones
    #[arg(long, default_value_t = 5.0)]
    pitch_range: f64,

    /// Insert breath sounds at phrase boundaries [use --no-breaths to disable]
    #[arg(long, default_value_t = true)]
    breaths: bool,

    /// Disable breath insertion
    #[arg(long, overrides_with = "breaths")]
    no_breaths: bool,

    /// Probability of breath at each phrase boundary
    #[arg(long, default_value_t = 0.6)]
    breath_probability: f64,

    /// RMS-normalize syllable clips [use --no-volume-normalize to disable]
    #[arg(long, default_value_t = true)]
    volume_normalize: bool,

    /// Disable volume normalization
    #[arg(long, overrides_with = "volume_normalize")]
    no_volume_normalize: bool,

    /// Apply phrase-level volume envelope [use --no-prosodic-dynamics to disable]
    #[arg(long, default_value_t = true)]
    prosodic_dynamics: bool,

    /// Disable prosodic dynamics
    #[arg(long, overrides_with = "prosodic_dynamics")]
    no_prosodic_dynamics: bool,

    // -- Time stretch --
    /// Global speed factor (0.5=half, 2.0=double)
    #[arg(long)]
    speed: Option<f64>,

    /// Probability a syllable gets stretched
    #[arg(long)]
    random_stretch: Option<f64>,

    /// Stretch every Nth syllable
    #[arg(long)]
    alternating_stretch: Option<usize>,

    /// Stretch first/last N syllables per word
    #[arg(long)]
    boundary_stretch: Option<usize>,

    /// Probability all syllables in a word get stretched
    #[arg(long)]
    word_stretch: Option<f64>,

    /// Stretch amount: "2.0" or "1.5-3.0"
    #[arg(long, default_value = "2.0")]
    stretch_factor: String,

    // -- Word repeat --
    /// Probability a word gets repeated
    #[arg(long)]
    repeat_weight: Option<f64>,

    /// Extra copies per repeated word: "2" or "1-3"
    #[arg(long, default_value = "1-2")]
    repeat_count: String,

    /// Repeat style
    #[arg(long, default_value = "exact", value_parser = ["exact", "resample"])]
    repeat_style: String,

    // -- Stutter --
    /// Probability a syllable gets stuttered
    #[arg(long)]
    stutter: Option<f64>,

    /// Extra copies of stuttered syllable: "2" or "1-3"
    #[arg(long, default_value = "1-2")]
    stutter_count: String,
}

// ─── Sing ────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(about = "Map syllable clips to MIDI melody notes")]
struct SingArgs {
    #[command(flatten)]
    shared: SharedArgs,

    /// Directory containing MIDI files
    #[arg(long)]
    midi: PathBuf,

    /// Enable vibrato [use --no-vibrato to disable]
    #[arg(long, default_value_t = true)]
    vibrato: bool,

    /// Disable vibrato
    #[arg(long, overrides_with = "vibrato")]
    no_vibrato: bool,

    /// Enable chorus [use --no-chorus to disable]
    #[arg(long, default_value_t = true)]
    chorus: bool,

    /// Disable chorus
    #[arg(long, overrides_with = "chorus")]
    no_chorus: bool,

    /// Max semitone drift from melody
    #[arg(long, default_value_t = 2.0)]
    drift_range: f64,

    /// Max source videos (Slack mode)
    #[arg(long, default_value_t = 5)]
    max_videos: usize,
}

// ─── Speak ───────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(about = "Reconstruct target text using syllable fragments")]
struct SpeakArgs {
    #[command(flatten)]
    shared: SharedArgs,

    /// Target text to reconstruct
    #[arg(long)]
    text: Option<String>,

    /// Reference audio for text + timing template
    #[arg(long)]
    reference: Option<PathBuf>,

    /// Matching granularity
    #[arg(long, default_value = "syllable", value_parser = ["syllable", "phoneme"])]
    match_unit: String,

    /// Adjust pitch to target intonation [use --no-pitch-correct to disable]
    #[arg(long, default_value_t = true)]
    pitch_correct: bool,

    /// Disable pitch correction
    #[arg(long, overrides_with = "pitch_correct")]
    no_pitch_correct: bool,

    /// How closely to follow reference timing (0.0-1.0)
    #[arg(long, default_value_t = 0.8)]
    timing_strictness: f64,

    /// Crossfade between syllables (ms)
    #[arg(long, default_value_t = 10.0)]
    crossfade: f64,

    /// Normalize volume across syllables [use --no-normalize-volume to disable]
    #[arg(long, default_value_t = true)]
    normalize_volume: bool,

    /// Disable volume normalization
    #[arg(long, overrides_with = "normalize_volume")]
    no_normalize_volume: bool,

    /// Alignment backend
    #[arg(long, default_value = "auto", value_parser = ["auto", "default", "bfa"])]
    aligner: String,
}

// ─── Main ────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    // Init logging
    let log_level = match &cli.command {
        Command::Collage(a) if a.shared.verbose => "debug",
        Command::Sing(a) if a.shared.verbose => "debug",
        Command::Speak(a) if a.shared.verbose => "debug",
        _ => "info",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .format_timestamp(None)
        .init();

    let result = match cli.command {
        Command::Collage(args) => run_collage(args),
        Command::Sing(args) => run_sing(args),
        Command::Speak(args) => run_speak(args),
    };

    if let Err(e) = result {
        log::error!("{:#}", e);
        std::process::exit(1);
    }
}

// ─── Helpers ─────────────────────────────────────────────────────

/// Validate input files exist.
fn validate_inputs(paths: &[PathBuf]) -> Result<()> {
    if paths.is_empty() {
        bail!("At least one input file is required");
    }
    for p in paths {
        if !p.exists() {
            bail!("File not found: {}", p.display());
        }
    }
    Ok(())
}

/// Extract audio from each input file to 16kHz mono WAV in the work dir.
fn prepare_audio(inputs: &[PathBuf], work_dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(work_dir)?;
    let mut audio_paths = Vec::new();
    for input in inputs {
        let stem = input
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "input".to_string());
        let wav_path = work_dir.join(format!("{}_16k.wav", stem));
        log::info!("Extracting audio: {} -> {}", input.display(), wav_path.display());
        extract_audio(input, &wav_path)?;
        audio_paths.push(wav_path);
    }
    Ok(audio_paths)
}

// ─── Collage runner ──────────────────────────────────────────────

fn run_collage(args: CollageArgs) -> Result<()> {
    validate_inputs(&args.shared.input_files)?;

    let run_dir = create_run_dir(
        &args.shared.output_dir,
        args.shared.seed,
        args.shared.run_name.as_deref(),
    )?;
    println!("Run: {}", run_dir.file_name().unwrap().to_string_lossy());

    let work_dir = run_dir.join("work");
    let audio_paths = prepare_audio(&args.shared.input_files, &work_dir)?;

    // Align each source and collect samples + syllables keyed by source
    let aligner = get_aligner(&args.aligner, &args.shared.whisper_model, "en", &args.bfa_device)?;
    let mut source_audio: HashMap<String, (Vec<f64>, u32)> = HashMap::new();
    let mut source_syllables: HashMap<String, Vec<glottisdale_core::types::Syllable>> = HashMap::new();

    for audio_path in &audio_paths {
        let key = audio_path.to_string_lossy().to_string();
        let alignment = aligner.process(audio_path, None)
            .with_context(|| format!("Alignment failed for {}", audio_path.display()))?;

        let (samples, sr) = read_wav(audio_path)?;
        source_audio.insert(key.clone(), (samples, sr));
        source_syllables.insert(key, alignment.syllables);
    }

    let total_syls: usize = source_syllables.values().map(|v| v.len()).sum();
    log::info!(
        "Aligned {} source(s): {} syllables",
        audio_paths.len(),
        total_syls
    );

    // Apply --no-* overrides
    let room_tone = args.room_tone && !args.no_room_tone;
    let pitch_normalize = args.pitch_normalize && !args.no_pitch_normalize;
    let breaths = args.breaths && !args.no_breaths;
    let volume_normalize = args.volume_normalize && !args.no_volume_normalize;
    let prosodic_dynamics = args.prosodic_dynamics && !args.no_prosodic_dynamics;

    // Build collage config from CLI args
    let config = glottisdale_core::collage::process::CollageConfig {
        syllables_per_clip: args.syllables_per_word,
        target_duration: args.shared.target_duration,
        crossfade_ms: args.crossfade,
        padding_ms: args.padding,
        words_per_phrase: args.words_per_phrase,
        phrases_per_sentence: args.phrases_per_sentence,
        phrase_pause: args.phrase_pause,
        sentence_pause: args.sentence_pause,
        word_crossfade_ms: args.word_crossfade,
        seed: args.shared.seed,
        noise_level_db: args.noise_level,
        room_tone,
        pitch_normalize,
        pitch_range: args.pitch_range,
        breaths,
        breath_probability: args.breath_probability,
        volume_normalize,
        prosodic_dynamics,
        speed: args.speed,
        stretch_config: StretchConfig {
            random_stretch: args.random_stretch,
            alternating_stretch: args.alternating_stretch,
            boundary_stretch: args.boundary_stretch,
            word_stretch: args.word_stretch,
            stretch_factor: parse_stretch_factor(&args.stretch_factor),
        },
        repeat_weight: args.repeat_weight,
        repeat_count: args.repeat_count,
        repeat_style: args.repeat_style,
        stutter: args.stutter,
        stutter_count: args.stutter_count,
    };

    let result = glottisdale_core::collage::process::process(
        &source_audio,
        &source_syllables,
        &run_dir,
        &config,
    )?;

    // Create clips.zip from the clips directory
    let clips_dir = run_dir.join("clips");
    let zip_path = run_dir.join("clips.zip");
    if clips_dir.is_dir() {
        let zip_file = std::fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(zip_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for entry in std::fs::read_dir(&clips_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "wav").unwrap_or(false) {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                zip.start_file(&name, options)?;
                let data = std::fs::read(&path)?;
                std::io::Write::write_all(&mut zip, &data)?;
            }
        }
        zip.finish()?;
        log::info!("Created {}", zip_path.display());
    }

    println!("Processed {} source file(s)", args.shared.input_files.len());
    println!("Selected {} clips", result.clips.len());
    println!("Output: {}", result.concatenated.display());

    Ok(())
}

// ─── Sing runner ─────────────────────────────────────────────────

fn run_sing(args: SingArgs) -> Result<()> {
    use glottisdale_core::sing::midi_parser::parse_midi;
    use glottisdale_core::sing::syllable_prep::{prepare_syllables, median_f0};
    use glottisdale_core::sing::vocal_mapper::{plan_note_mapping, render_vocal_track};
    use glottisdale_core::sing::mixer::mix_tracks;

    validate_inputs(&args.shared.input_files)?;

    let melody_path = args.midi.join("melody.mid");
    if !melody_path.exists() {
        bail!("MIDI melody not found: {}", melody_path.display());
    }

    let run_dir = create_run_dir(
        &args.shared.output_dir,
        args.shared.seed,
        args.shared.run_name.as_deref(),
    )?;
    println!("Run: {}", run_dir.file_name().unwrap().to_string_lossy());

    let work_dir = run_dir.join("work");
    let audio_paths = prepare_audio(&args.shared.input_files, &work_dir)?;

    // Parse MIDI melody
    log::info!("Parsing MIDI: {}", melody_path.display());
    let track = parse_midi(&melody_path)?;
    log::info!(
        "Melody: {} notes, {} BPM, {:.1}s",
        track.notes.len(),
        track.tempo,
        track.total_duration
    );

    // Align and prepare syllables from source audio
    let aligner = get_aligner("auto", &args.shared.whisper_model, "en", "cpu")?;
    let mut all_syllable_clips = Vec::new();
    let mut sample_rate = 16000u32;

    for audio_path in &audio_paths {
        let alignment = aligner.process(audio_path, None)?;
        let (samples, sr) = read_wav(audio_path)?;
        sample_rate = sr;

        let prepared = prepare_syllables(
            &alignment.syllables,
            &samples,
            sr,
            12.0, // max_semitone_shift
        );
        all_syllable_clips.extend(prepared);
    }

    log::info!("Prepared {} syllable clips", all_syllable_clips.len());

    if all_syllable_clips.is_empty() {
        bail!("No syllables found in source audio");
    }

    // Compute median F0
    let med_f0 = median_f0(&all_syllable_clips).unwrap_or(220.0);
    log::info!("Median F0: {:.1} Hz", med_f0);

    // Apply --no-* overrides
    let _vibrato = args.vibrato && !args.no_vibrato;
    let chorus = args.chorus && !args.no_chorus;

    // Plan note mapping
    let chorus_prob = if chorus { 0.3 } else { 0.0 };
    let mappings = plan_note_mapping(
        &track.notes,
        all_syllable_clips.len(),
        args.shared.seed,
        args.drift_range,
        chorus_prob,
    );
    log::info!("Planned {} note mappings", mappings.len());

    // Render vocal track
    log::info!("Rendering vocal track");
    let vocal_samples = render_vocal_track(
        &mappings,
        &all_syllable_clips,
        med_f0,
        sample_rate,
    );

    if vocal_samples.is_empty() {
        bail!("Vocal rendering produced no output");
    }
    log::info!(
        "Vocal track: {} samples ({:.1}s)",
        vocal_samples.len(),
        vocal_samples.len() as f64 / sample_rate as f64
    );

    // Parse backing MIDI tracks (all .mid files except melody)
    let mut backing_tracks = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&args.midi) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "mid" || e == "midi").unwrap_or(false)
                && path != melody_path
            {
                if let Ok(t) = parse_midi(&path) {
                    backing_tracks.push(t);
                }
            }
        }
    }

    // Mix
    log::info!("Mixing tracks");
    let (full_mix, acappella) = mix_tracks(
        &vocal_samples,
        sample_rate,
        &backing_tracks,
        &run_dir,
        0.0,   // vocal_db
        -12.0, // midi_db
    )?;

    println!("Output: {}", full_mix.display());
    println!("A cappella: {}", acappella.display());

    Ok(())
}

// ─── Speak runner ────────────────────────────────────────────────

fn run_speak(args: SpeakArgs) -> Result<()> {
    use glottisdale_core::speak::syllable_bank::build_bank;
    use glottisdale_core::speak::target_text::{text_to_syllables, word_boundaries_from_syllables};
    use glottisdale_core::speak::matcher::{match_syllables, match_phonemes};
    use glottisdale_core::speak::assembler::{plan_timing, assemble};

    validate_inputs(&args.shared.input_files)?;

    if args.text.is_none() && args.reference.is_none() {
        bail!("Either --text or --reference is required");
    }

    let run_dir = create_run_dir(
        &args.shared.output_dir,
        args.shared.seed,
        args.shared.run_name.as_deref(),
    )?;
    println!("Run: {}", run_dir.file_name().unwrap().to_string_lossy());

    let work_dir = run_dir.join("work");
    let audio_paths = prepare_audio(&args.shared.input_files, &work_dir)?;

    // Build syllable bank from source audio
    log::info!("Building source syllable bank");
    let aligner = get_aligner(&args.aligner, &args.shared.whisper_model, "en", "cpu")?;
    let mut all_bank_entries = Vec::new();
    let mut source_audio: HashMap<String, (Vec<f64>, u32)> = HashMap::new();

    for audio_path in &audio_paths {
        let key = audio_path.to_string_lossy().to_string();
        let alignment = aligner.process(audio_path, None)?;
        let entries = build_bank(&alignment.syllables, &key);
        log::info!(
            "  {}: {} syllables",
            audio_path.file_name().unwrap().to_string_lossy(),
            entries.len()
        );
        all_bank_entries.extend(entries);

        let (samples, sr) = read_wav(audio_path)?;
        source_audio.insert(key, (samples, sr));
    }

    log::info!("Syllable bank: {} total entries", all_bank_entries.len());

    // Get target text
    let mut target_text = args.text.clone();
    let mut reference_timings: Option<Vec<(f64, f64)>> = None;

    if let Some(ref_path) = &args.reference {
        log::info!("Transcribing reference audio: {}", ref_path.display());
        let ref_wav = work_dir.join("reference_16k.wav");
        extract_audio(ref_path, &ref_wav)?;
        let ref_alignment = aligner.process(&ref_wav, None)?;
        target_text = Some(ref_alignment.text);
        reference_timings = Some(
            ref_alignment
                .syllables
                .iter()
                .map(|s| (s.start, s.end))
                .collect(),
        );
    }

    let target_text = target_text.context("No target text (use --text or --reference)")?;
    log::info!("Target text: {}", target_text);

    // Convert target text to syllables
    let target_syls = text_to_syllables(&target_text);
    let word_bounds = word_boundaries_from_syllables(&target_syls);
    log::info!(
        "Target: {} syllables, {} words",
        target_syls.len(),
        word_bounds.len()
    );

    // Match
    log::info!("Matching ({} mode)", args.match_unit);
    let matches = if args.match_unit == "phoneme" {
        let all_phonemes: Vec<String> = target_syls
            .iter()
            .flat_map(|ts| ts.phonemes.clone())
            .collect();
        match_phonemes(&all_phonemes, &all_bank_entries)
    } else {
        let target_phoneme_lists: Vec<Vec<String>> =
            target_syls.iter().map(|ts| ts.phonemes.clone()).collect();
        let target_stresses: Vec<Option<u8>> =
            target_syls.iter().map(|ts| ts.stress).collect();
        match_syllables(
            &target_phoneme_lists,
            &all_bank_entries,
            Some(&target_stresses),
            None, // use default continuity bonus
        )
    };

    // Plan timing
    let avg_dur = if all_bank_entries.is_empty() {
        0.25
    } else {
        all_bank_entries.iter().map(|e| e.duration()).sum::<f64>() / all_bank_entries.len() as f64
    };

    let timing = plan_timing(
        &matches,
        &word_bounds,
        avg_dur,
        reference_timings.as_deref(),
        args.timing_strictness,
    );

    // Apply --no-* overrides
    let normalize_volume = args.normalize_volume && !args.no_normalize_volume;
    let pitch_correct = args.pitch_correct && !args.no_pitch_correct;

    // Assemble
    log::info!("Assembling output audio");
    let output_path = assemble(
        &matches,
        &timing,
        &source_audio,
        &run_dir,
        args.crossfade,
        None, // pitch_shifts - use default
        normalize_volume,
        pitch_correct,
    )?;

    println!("Target text: {}", target_text);
    println!("Output: {}", output_path.display());

    Ok(())
}
