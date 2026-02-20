//! Main application state and UI layout.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

// ─── Pipeline mode ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum PipelineMode {
    Collage,
    Sing,
    Speak,
}

impl PipelineMode {
    fn label(&self) -> &str {
        match self {
            Self::Collage => "Collage",
            Self::Sing => "Sing",
            Self::Speak => "Speak",
        }
    }
}

// ─── Processing status ──────────────────────────────────────────

#[derive(Debug, Clone)]
enum ProcessingStatus {
    Idle,
    Running(String),
    Done(String),
    Error(String),
}

// ─── Shared processing state ────────────────────────────────────

#[derive(Clone)]
struct ProcessingState {
    status: Arc<Mutex<ProcessingStatus>>,
    log_lines: Arc<Mutex<Vec<String>>>,
}

impl ProcessingState {
    fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(ProcessingStatus::Idle)),
            log_lines: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn set_status(&self, status: ProcessingStatus) {
        *self.status.lock().unwrap() = status;
    }

    fn get_status(&self) -> ProcessingStatus {
        self.status.lock().unwrap().clone()
    }

    fn add_log(&self, msg: &str) {
        self.log_lines.lock().unwrap().push(msg.to_string());
    }

    fn get_logs(&self) -> Vec<String> {
        self.log_lines.lock().unwrap().clone()
    }

    fn clear(&self) {
        *self.status.lock().unwrap() = ProcessingStatus::Idle;
        self.log_lines.lock().unwrap().clear();
    }
}

// ─── Collage settings ───────────────────────────────────────────

#[derive(Debug, Clone)]
struct CollageSettings {
    target_duration: f64,
    syllables_per_word: String,
    crossfade_ms: f64,
    padding_ms: f64,
    words_per_phrase: String,
    phrases_per_sentence: String,
    phrase_pause: String,
    sentence_pause: String,
    word_crossfade_ms: f64,
    // Audio polish
    noise_level_db: f64,
    room_tone: bool,
    pitch_normalize: bool,
    pitch_range: f64,
    breaths: bool,
    breath_probability: f64,
    volume_normalize: bool,
    prosodic_dynamics: bool,
    // Stretch
    speed: String,
    random_stretch: String,
    alternating_stretch: String,
    boundary_stretch: String,
    word_stretch: String,
    stretch_factor: String,
    // Repeat
    repeat_weight: String,
    repeat_count: String,
    // Stutter
    stutter: String,
    stutter_count: String,
}

impl Default for CollageSettings {
    fn default() -> Self {
        Self {
            target_duration: 30.0,
            syllables_per_word: "1-4".to_string(),
            crossfade_ms: 30.0,
            padding_ms: 25.0,
            words_per_phrase: "3-5".to_string(),
            phrases_per_sentence: "2-3".to_string(),
            phrase_pause: "400-700".to_string(),
            sentence_pause: "800-1200".to_string(),
            word_crossfade_ms: 50.0,
            noise_level_db: -40.0,
            room_tone: true,
            pitch_normalize: true,
            pitch_range: 5.0,
            breaths: true,
            breath_probability: 0.6,
            volume_normalize: true,
            prosodic_dynamics: true,
            speed: String::new(),
            random_stretch: String::new(),
            alternating_stretch: String::new(),
            boundary_stretch: String::new(),
            word_stretch: String::new(),
            stretch_factor: "2.0".to_string(),
            repeat_weight: String::new(),
            repeat_count: "1-2".to_string(),
            stutter: String::new(),
            stutter_count: "1-2".to_string(),
        }
    }
}

// ─── Sing settings ──────────────────────────────────────────────

#[derive(Debug, Clone)]
struct SingSettings {
    midi_dir: String,
    target_duration: f64,
    vibrato: bool,
    chorus: bool,
    drift_range: f64,
}

impl Default for SingSettings {
    fn default() -> Self {
        Self {
            midi_dir: String::new(),
            target_duration: 30.0,
            vibrato: true,
            chorus: true,
            drift_range: 2.0,
        }
    }
}

// ─── Speak settings ─────────────────────────────────────────────

#[derive(Debug, Clone)]
struct SpeakSettings {
    target_text: String,
    reference_path: String,
    match_unit: String,
    pitch_correct: bool,
    timing_strictness: f64,
    crossfade_ms: f64,
    normalize_volume: bool,
}

impl Default for SpeakSettings {
    fn default() -> Self {
        Self {
            target_text: String::new(),
            reference_path: String::new(),
            match_unit: "syllable".to_string(),
            pitch_correct: true,
            timing_strictness: 0.8,
            crossfade_ms: 10.0,
            normalize_volume: true,
        }
    }
}

// ─── Main app ───────────────────────────────────────────────────

pub struct GlottisdaleApp {
    mode: PipelineMode,
    // Source files
    source_files: Vec<PathBuf>,
    // Output
    output_dir: String,
    whisper_model: String,
    seed: String,
    run_name: String,
    aligner: String,
    // Per-pipeline settings
    collage: CollageSettings,
    sing: SingSettings,
    speak: SpeakSettings,
    // Processing
    processing: ProcessingState,
    // UI state
    show_log: bool,
}

impl GlottisdaleApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            mode: PipelineMode::Collage,
            source_files: Vec::new(),
            output_dir: "./glottisdale-output".to_string(),
            whisper_model: "base".to_string(),
            seed: String::new(),
            run_name: String::new(),
            aligner: "auto".to_string(),
            collage: CollageSettings::default(),
            sing: SingSettings::default(),
            speak: SpeakSettings::default(),
            processing: ProcessingState::new(),
            show_log: false,
        }
    }

    fn is_processing(&self) -> bool {
        matches!(self.processing.get_status(), ProcessingStatus::Running(_))
    }
}

impl eframe::App for GlottisdaleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request repaint while processing for status updates
        if self.is_processing() {
            ctx.request_repaint();
        }

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label("Glottisdale");
                ui.separator();

                for mode in [PipelineMode::Collage, PipelineMode::Sing, PipelineMode::Speak] {
                    if ui.selectable_label(self.mode == mode, mode.label()).clicked() {
                        self.mode = mode;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match self.processing.get_status() {
                        ProcessingStatus::Idle => {
                            ui.label("Ready");
                        }
                        ProcessingStatus::Running(msg) => {
                            ui.spinner();
                            ui.label(msg);
                        }
                        ProcessingStatus::Done(msg) => {
                            ui.colored_label(egui::Color32::GREEN, msg);
                        }
                        ProcessingStatus::Error(msg) => {
                            ui.colored_label(egui::Color32::RED, msg);
                        }
                    }
                });
            });
        });

        // Bottom status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} source file(s)", self.source_files.len()));
                ui.separator();
                ui.label(format!("Mode: {}", self.mode.label()));
                ui.separator();
                if ui.selectable_label(self.show_log, "Log").clicked() {
                    self.show_log = !self.show_log;
                }
            });
        });

        // Log panel (bottom, collapsible)
        if self.show_log {
            egui::TopBottomPanel::bottom("log_panel")
                .resizable(true)
                .min_height(100.0)
                .default_height(150.0)
                .show(ctx, |ui| {
                    ui.heading("Log");
                    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                        let logs = self.processing.get_logs();
                        for line in &logs {
                            ui.monospace(line);
                        }
                        if logs.is_empty() {
                            ui.weak("No log messages yet");
                        }
                    });
                });
        }

        // Left panel: source files
        egui::SidePanel::left("source_panel")
            .default_width(250.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Source Files");
                ui.separator();

                if ui.button("Add Files...").clicked() {
                    if let Some(paths) = rfd::FileDialog::new()
                        .add_filter("Audio/Video", &["wav", "mp3", "mp4", "mov", "mkv", "flac", "ogg", "m4a"])
                        .pick_files()
                    {
                        for p in paths {
                            if !self.source_files.contains(&p) {
                                self.source_files.push(p);
                            }
                        }
                    }
                }

                ui.separator();

                let mut to_remove = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, path) in self.source_files.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.display().to_string());
                            ui.label(&name);
                            if ui.small_button("x").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                });

                if let Some(idx) = to_remove {
                    self.source_files.remove(idx);
                }

                ui.separator();
                if ui.button("Clear All").clicked() {
                    self.source_files.clear();
                }
            });

        // Right panel: settings for current mode
        egui::SidePanel::right("settings_panel")
            .default_width(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Settings");
                    ui.separator();

                    // Common settings
                    ui.collapsing("General", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Output dir:");
                            ui.text_edit_singleline(&mut self.output_dir);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Whisper model:");
                            egui::ComboBox::from_id_salt("whisper_model")
                                .selected_text(&self.whisper_model)
                                .show_ui(ui, |ui| {
                                    for m in ["tiny", "base", "small", "medium"] {
                                        ui.selectable_value(&mut self.whisper_model, m.to_string(), m);
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            ui.label("Aligner:");
                            egui::ComboBox::from_id_salt("aligner")
                                .selected_text(&self.aligner)
                                .show_ui(ui, |ui| {
                                    for a in ["auto", "default", "bfa"] {
                                        ui.selectable_value(&mut self.aligner, a.to_string(), a);
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            ui.label("Seed:");
                            ui.text_edit_singleline(&mut self.seed);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Run name:");
                            ui.text_edit_singleline(&mut self.run_name);
                        });
                    });

                    ui.separator();

                    // Mode-specific settings
                    match self.mode {
                        PipelineMode::Collage => show_collage_settings(ui, &mut self.collage),
                        PipelineMode::Sing => show_sing_settings(ui, &mut self.sing),
                        PipelineMode::Speak => show_speak_settings(ui, &mut self.speak),
                    }
                });
            });

        // Central panel: main workspace
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading(format!("{} Workspace", self.mode.label()));
                ui.add_space(10.0);
            });

            match self.mode {
                PipelineMode::Collage => show_collage_workspace(ui, self),
                PipelineMode::Sing => show_sing_workspace(ui, self),
                PipelineMode::Speak => show_speak_workspace(ui, self),
            }
        });
    }
}

// ─── Settings panels ─────────────────────────────────────────────

fn show_collage_settings(ui: &mut egui::Ui, s: &mut CollageSettings) {
    ui.collapsing("Prosodic Grouping", |ui| {
        ui.horizontal(|ui| {
            ui.label("Target duration (s):");
            ui.add(egui::DragValue::new(&mut s.target_duration).range(1.0..=300.0).speed(0.5));
        });
        ui.horizontal(|ui| {
            ui.label("Syl/word:");
            ui.text_edit_singleline(&mut s.syllables_per_word);
        });
        ui.horizontal(|ui| {
            ui.label("Words/phrase:");
            ui.text_edit_singleline(&mut s.words_per_phrase);
        });
        ui.horizontal(|ui| {
            ui.label("Phrases/sentence:");
            ui.text_edit_singleline(&mut s.phrases_per_sentence);
        });
        ui.horizontal(|ui| {
            ui.label("Crossfade (ms):");
            ui.add(egui::DragValue::new(&mut s.crossfade_ms).range(0.0..=200.0).speed(1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Padding (ms):");
            ui.add(egui::DragValue::new(&mut s.padding_ms).range(0.0..=100.0).speed(1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Word crossfade (ms):");
            ui.add(egui::DragValue::new(&mut s.word_crossfade_ms).range(0.0..=200.0).speed(1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Phrase pause (ms):");
            ui.text_edit_singleline(&mut s.phrase_pause);
        });
        ui.horizontal(|ui| {
            ui.label("Sentence pause (ms):");
            ui.text_edit_singleline(&mut s.sentence_pause);
        });
    });

    ui.collapsing("Audio Polish", |ui| {
        ui.horizontal(|ui| {
            ui.label("Noise level (dB):");
            ui.add(egui::DragValue::new(&mut s.noise_level_db).range(-60.0..=0.0).speed(1.0));
        });
        ui.checkbox(&mut s.room_tone, "Room tone");
        ui.checkbox(&mut s.pitch_normalize, "Pitch normalize");
        ui.horizontal(|ui| {
            ui.label("Pitch range (st):");
            ui.add(egui::DragValue::new(&mut s.pitch_range).range(0.0..=12.0).speed(0.5));
        });
        ui.checkbox(&mut s.breaths, "Insert breaths");
        ui.horizontal(|ui| {
            ui.label("Breath prob:");
            ui.add(egui::Slider::new(&mut s.breath_probability, 0.0..=1.0));
        });
        ui.checkbox(&mut s.volume_normalize, "Volume normalize");
        ui.checkbox(&mut s.prosodic_dynamics, "Prosodic dynamics");
    });

    ui.collapsing("Stretch", |ui| {
        ui.horizontal(|ui| {
            ui.label("Speed:");
            ui.text_edit_singleline(&mut s.speed);
        });
        ui.horizontal(|ui| {
            ui.label("Random stretch:");
            ui.text_edit_singleline(&mut s.random_stretch);
        });
        ui.horizontal(|ui| {
            ui.label("Alternating:");
            ui.text_edit_singleline(&mut s.alternating_stretch);
        });
        ui.horizontal(|ui| {
            ui.label("Boundary:");
            ui.text_edit_singleline(&mut s.boundary_stretch);
        });
        ui.horizontal(|ui| {
            ui.label("Word stretch:");
            ui.text_edit_singleline(&mut s.word_stretch);
        });
        ui.horizontal(|ui| {
            ui.label("Factor:");
            ui.text_edit_singleline(&mut s.stretch_factor);
        });
    });

    ui.collapsing("Repeat & Stutter", |ui| {
        ui.horizontal(|ui| {
            ui.label("Repeat weight:");
            ui.text_edit_singleline(&mut s.repeat_weight);
        });
        ui.horizontal(|ui| {
            ui.label("Repeat count:");
            ui.text_edit_singleline(&mut s.repeat_count);
        });
        ui.horizontal(|ui| {
            ui.label("Stutter prob:");
            ui.text_edit_singleline(&mut s.stutter);
        });
        ui.horizontal(|ui| {
            ui.label("Stutter count:");
            ui.text_edit_singleline(&mut s.stutter_count);
        });
    });
}

fn show_sing_settings(ui: &mut egui::Ui, s: &mut SingSettings) {
    ui.collapsing("MIDI", |ui| {
        ui.horizontal(|ui| {
            ui.label("MIDI dir:");
            ui.text_edit_singleline(&mut s.midi_dir);
        });
        if ui.button("Browse...").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                s.midi_dir = path.display().to_string();
            }
        }
    });

    ui.collapsing("Parameters", |ui| {
        ui.horizontal(|ui| {
            ui.label("Target duration (s):");
            ui.add(egui::DragValue::new(&mut s.target_duration).range(1.0..=300.0).speed(0.5));
        });
        ui.checkbox(&mut s.vibrato, "Vibrato");
        ui.checkbox(&mut s.chorus, "Chorus");
        ui.horizontal(|ui| {
            ui.label("Drift range (st):");
            ui.add(egui::Slider::new(&mut s.drift_range, 0.0..=6.0));
        });
    });
}

fn show_speak_settings(ui: &mut egui::Ui, s: &mut SpeakSettings) {
    ui.collapsing("Target", |ui| {
        ui.label("Target text:");
        ui.text_edit_multiline(&mut s.target_text);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Reference audio:");
            ui.text_edit_singleline(&mut s.reference_path);
        });
        if ui.button("Browse...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Audio", &["wav", "mp3", "flac", "m4a"])
                .pick_file()
            {
                s.reference_path = path.display().to_string();
            }
        }
    });

    ui.collapsing("Parameters", |ui| {
        ui.horizontal(|ui| {
            ui.label("Match unit:");
            egui::ComboBox::from_id_salt("match_unit")
                .selected_text(&s.match_unit)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut s.match_unit, "syllable".to_string(), "syllable");
                    ui.selectable_value(&mut s.match_unit, "phoneme".to_string(), "phoneme");
                });
        });
        ui.checkbox(&mut s.pitch_correct, "Pitch correct");
        ui.horizontal(|ui| {
            ui.label("Timing strictness:");
            ui.add(egui::Slider::new(&mut s.timing_strictness, 0.0..=1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Crossfade (ms):");
            ui.add(egui::DragValue::new(&mut s.crossfade_ms).range(0.0..=100.0).speed(1.0));
        });
        ui.checkbox(&mut s.normalize_volume, "Normalize volume");
    });
}

// ─── Workspace panels ───────────────────────────────────────────

fn show_collage_workspace(ui: &mut egui::Ui, app: &mut GlottisdaleApp) {
    if app.source_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("Add source audio files to get started.");
            ui.label("Use the file picker on the left panel.");
        });
        return;
    }

    ui.horizontal(|ui| {
        let can_run = !app.is_processing() && !app.source_files.is_empty();
        if ui.add_enabled(can_run, egui::Button::new("Run Collage")).clicked() {
            start_collage(app);
        }
        if app.is_processing() {
            ui.spinner();
        }
    });

    ui.separator();

    // Show source file list with info
    ui.label(format!("{} source file(s) loaded", app.source_files.len()));
    for path in &app.source_files {
        ui.monospace(path.display().to_string());
    }

    // Show result if done
    if let ProcessingStatus::Done(msg) = app.processing.get_status() {
        ui.separator();
        ui.colored_label(egui::Color32::GREEN, &msg);
    }
    if let ProcessingStatus::Error(msg) = app.processing.get_status() {
        ui.separator();
        ui.colored_label(egui::Color32::RED, &msg);
    }
}

fn show_sing_workspace(ui: &mut egui::Ui, app: &mut GlottisdaleApp) {
    if app.source_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("Add source audio files and set MIDI directory to get started.");
        });
        return;
    }

    if app.sing.midi_dir.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("Set the MIDI directory in the settings panel.");
        });
        return;
    }

    ui.horizontal(|ui| {
        let can_run = !app.is_processing() && !app.source_files.is_empty();
        if ui.add_enabled(can_run, egui::Button::new("Run Sing")).clicked() {
            start_sing(app);
        }
        if app.is_processing() {
            ui.spinner();
        }
    });

    ui.separator();
    ui.label(format!("{} source file(s)", app.source_files.len()));
    ui.label(format!("MIDI: {}", app.sing.midi_dir));

    if let ProcessingStatus::Done(msg) = app.processing.get_status() {
        ui.separator();
        ui.colored_label(egui::Color32::GREEN, &msg);
    }
    if let ProcessingStatus::Error(msg) = app.processing.get_status() {
        ui.separator();
        ui.colored_label(egui::Color32::RED, &msg);
    }
}

fn show_speak_workspace(ui: &mut egui::Ui, app: &mut GlottisdaleApp) {
    if app.source_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("Add source audio files to get started.");
            ui.label("Then enter target text or select a reference audio.");
        });
        return;
    }

    let has_target = !app.speak.target_text.is_empty() || !app.speak.reference_path.is_empty();

    ui.horizontal(|ui| {
        let can_run = !app.is_processing() && !app.source_files.is_empty() && has_target;
        if ui.add_enabled(can_run, egui::Button::new("Run Speak")).clicked() {
            start_speak(app);
        }
        if !has_target {
            ui.weak("Enter target text or reference audio in settings");
        }
        if app.is_processing() {
            ui.spinner();
        }
    });

    ui.separator();

    if !app.speak.target_text.is_empty() {
        ui.label("Target text:");
        ui.monospace(&app.speak.target_text);
    } else if !app.speak.reference_path.is_empty() {
        ui.label(format!("Reference: {}", app.speak.reference_path));
    }

    if let ProcessingStatus::Done(msg) = app.processing.get_status() {
        ui.separator();
        ui.colored_label(egui::Color32::GREEN, &msg);
    }
    if let ProcessingStatus::Error(msg) = app.processing.get_status() {
        ui.separator();
        ui.colored_label(egui::Color32::RED, &msg);
    }
}

// ─── Pipeline runners (background threads) ──────────────────────

fn build_cli_args(app: &GlottisdaleApp, subcommand: &str) -> Vec<String> {
    let mut args = vec!["glottisdale".to_string(), subcommand.to_string()];

    // Source files
    for f in &app.source_files {
        args.push(f.display().to_string());
    }

    // Common args
    args.extend(["--output-dir".to_string(), app.output_dir.clone()]);
    args.extend(["--whisper-model".to_string(), app.whisper_model.clone()]);

    if !app.seed.is_empty() {
        args.extend(["--seed".to_string(), app.seed.clone()]);
    }
    if !app.run_name.is_empty() {
        args.extend(["--run-name".to_string(), app.run_name.clone()]);
    }

    args
}

fn start_collage(app: &mut GlottisdaleApp) {
    let state = app.processing.clone();
    state.clear();
    state.set_status(ProcessingStatus::Running("Starting collage...".into()));

    let mut args = build_cli_args(app, "collage");
    let s = &app.collage;

    args.extend(["--target-duration".into(), s.target_duration.to_string()]);
    args.extend(["--syllables-per-word".into(), s.syllables_per_word.clone()]);
    args.extend(["--crossfade".into(), s.crossfade_ms.to_string()]);
    args.extend(["--padding".into(), s.padding_ms.to_string()]);
    args.extend(["--words-per-phrase".into(), s.words_per_phrase.clone()]);
    args.extend(["--phrases-per-sentence".into(), s.phrases_per_sentence.clone()]);
    args.extend(["--phrase-pause".into(), s.phrase_pause.clone()]);
    args.extend(["--sentence-pause".into(), s.sentence_pause.clone()]);
    args.extend(["--word-crossfade".into(), s.word_crossfade_ms.to_string()]);
    args.extend(["--noise-level".into(), s.noise_level_db.to_string()]);
    args.extend(["--room-tone".into(), s.room_tone.to_string()]);
    args.extend(["--pitch-normalize".into(), s.pitch_normalize.to_string()]);
    args.extend(["--pitch-range".into(), s.pitch_range.to_string()]);
    args.extend(["--breaths".into(), s.breaths.to_string()]);
    args.extend(["--breath-probability".into(), s.breath_probability.to_string()]);
    args.extend(["--volume-normalize".into(), s.volume_normalize.to_string()]);
    args.extend(["--prosodic-dynamics".into(), s.prosodic_dynamics.to_string()]);
    args.extend(["--stretch-factor".into(), s.stretch_factor.clone()]);
    args.extend(["--repeat-count".into(), s.repeat_count.clone()]);
    args.extend(["--stutter-count".into(), s.stutter_count.clone()]);
    args.extend(["--aligner".into(), app.aligner.clone()]);

    if !s.speed.is_empty() { args.extend(["--speed".into(), s.speed.clone()]); }
    if !s.random_stretch.is_empty() { args.extend(["--random-stretch".into(), s.random_stretch.clone()]); }
    if !s.alternating_stretch.is_empty() { args.extend(["--alternating-stretch".into(), s.alternating_stretch.clone()]); }
    if !s.boundary_stretch.is_empty() { args.extend(["--boundary-stretch".into(), s.boundary_stretch.clone()]); }
    if !s.word_stretch.is_empty() { args.extend(["--word-stretch".into(), s.word_stretch.clone()]); }
    if !s.repeat_weight.is_empty() { args.extend(["--repeat-weight".into(), s.repeat_weight.clone()]); }
    if !s.stutter.is_empty() { args.extend(["--stutter".into(), s.stutter.clone()]); }

    run_cli_subprocess(state, args);
}

fn start_sing(app: &mut GlottisdaleApp) {
    let state = app.processing.clone();
    state.clear();
    state.set_status(ProcessingStatus::Running("Starting sing...".into()));

    let mut args = build_cli_args(app, "sing");
    let s = &app.sing;

    args.extend(["--midi".into(), s.midi_dir.clone()]);
    args.extend(["--target-duration".into(), s.target_duration.to_string()]);
    args.extend(["--vibrato".into(), s.vibrato.to_string()]);
    args.extend(["--chorus".into(), s.chorus.to_string()]);
    args.extend(["--drift-range".into(), s.drift_range.to_string()]);

    run_cli_subprocess(state, args);
}

fn start_speak(app: &mut GlottisdaleApp) {
    let state = app.processing.clone();
    state.clear();
    state.set_status(ProcessingStatus::Running("Starting speak...".into()));

    let mut args = build_cli_args(app, "speak");
    let s = &app.speak;

    if !s.target_text.is_empty() {
        args.extend(["--text".into(), s.target_text.clone()]);
    }
    if !s.reference_path.is_empty() {
        args.extend(["--reference".into(), s.reference_path.clone()]);
    }
    args.extend(["--match-unit".into(), s.match_unit.clone()]);
    args.extend(["--pitch-correct".into(), s.pitch_correct.to_string()]);
    args.extend(["--timing-strictness".into(), s.timing_strictness.to_string()]);
    args.extend(["--crossfade".into(), s.crossfade_ms.to_string()]);
    args.extend(["--normalize-volume".into(), s.normalize_volume.to_string()]);
    args.extend(["--aligner".into(), app.aligner.clone()]);

    run_cli_subprocess(state, args);
}

/// Run the CLI as a subprocess and capture output.
fn run_cli_subprocess(state: ProcessingState, args: Vec<String>) {
    thread::spawn(move || {
        state.add_log(&format!("Running: {}", args.join(" ")));
        state.set_status(ProcessingStatus::Running("Processing...".into()));

        // Find our own binary path and use the CLI binary
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("glottisdale"));
        let cli_exe = exe
            .parent()
            .map(|p| p.join("glottisdale"))
            .unwrap_or_else(|| PathBuf::from("glottisdale"));

        let result = std::process::Command::new(&cli_exe)
            .args(&args[1..]) // skip "glottisdale" since it's the program name
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                for line in stdout.lines() {
                    state.add_log(line);
                }
                for line in stderr.lines() {
                    state.add_log(&format!("[stderr] {}", line));
                }

                if output.status.success() {
                    state.set_status(ProcessingStatus::Done("Completed successfully".into()));
                } else {
                    let msg = stderr.lines().last().unwrap_or("Unknown error").to_string();
                    state.set_status(ProcessingStatus::Error(msg));
                }
            }
            Err(e) => {
                state.add_log(&format!("Failed to run CLI: {}", e));
                state.set_status(ProcessingStatus::Error(format!("Failed to run CLI: {}", e)));
            }
        }
    });
}
