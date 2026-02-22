//! Main application state and UI layout.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;
use glottisdale_core::editor::pipeline_bridge::arrangement_blank_canvas;
use glottisdale_core::editor::EditorPipelineMode;
use glottisdale_core::types::Syllable;

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

    fn to_editor_mode(self) -> EditorPipelineMode {
        match self {
            Self::Collage => EditorPipelineMode::Collage,
            Self::Sing => EditorPipelineMode::Sing,
            Self::Speak => EditorPipelineMode::Speak,
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

// ─── Alignment data for editor ──────────────────────────────────

/// Intermediate alignment data stored for the editor.
struct AlignmentData {
    syllables: HashMap<String, Vec<Syllable>>,
    audio: HashMap<String, (Vec<f64>, u32)>,
    pipeline_mode: EditorPipelineMode,
}

// ─── Shared processing state ────────────────────────────────────

#[derive(Clone)]
struct ProcessingState {
    status: Arc<Mutex<ProcessingStatus>>,
    log_lines: Arc<Mutex<Vec<String>>>,
    /// Output file paths parsed from CLI stdout (e.g. "Output: path/to/file.wav")
    output_paths: Arc<Mutex<Vec<(String, PathBuf)>>>,
    /// Alignment data from the most recent pipeline run (for editor).
    alignment: Arc<Mutex<Option<Arc<AlignmentData>>>>,
    /// When true, automatically open the editor on next frame.
    auto_open_editor: Arc<Mutex<bool>>,
}

impl ProcessingState {
    fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(ProcessingStatus::Idle)),
            log_lines: Arc::new(Mutex::new(Vec::new())),
            output_paths: Arc::new(Mutex::new(Vec::new())),
            alignment: Arc::new(Mutex::new(None)),
            auto_open_editor: Arc::new(Mutex::new(false)),
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

    fn add_output(&self, label: &str, path: PathBuf) {
        self.output_paths.lock().unwrap().push((label.to_string(), path));
    }

    fn get_outputs(&self) -> Vec<(String, PathBuf)> {
        self.output_paths.lock().unwrap().clone()
    }

    fn clear(&self) {
        *self.status.lock().unwrap() = ProcessingStatus::Idle;
        self.log_lines.lock().unwrap().clear();
        self.output_paths.lock().unwrap().clear();
        *self.alignment.lock().unwrap() = None;
        *self.auto_open_editor.lock().unwrap() = false;
    }

    fn store_alignment(&self, data: AlignmentData) {
        *self.alignment.lock().unwrap() = Some(Arc::new(data));
    }

    fn get_alignment(&self) -> Option<Arc<AlignmentData>> {
        self.alignment.lock().unwrap().clone()
    }

    fn has_alignment(&self) -> bool {
        self.alignment.lock().unwrap().is_some()
    }

    fn set_auto_open_editor(&self) {
        *self.auto_open_editor.lock().unwrap() = true;
    }

    fn take_auto_open_editor(&self) -> bool {
        let mut flag = self.auto_open_editor.lock().unwrap();
        if *flag {
            *flag = false;
            true
        } else {
            false
        }
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
    /// Editor state (None = editor not open)
    editor: Option<crate::editor::EditorState>,
    // Branding textures
    icon_texture: egui::TextureHandle,
    banner_texture: egui::TextureHandle,
}

fn load_texture(
    ctx: &egui::Context,
    name: &str,
    bytes: &[u8],
) -> egui::TextureHandle {
    let img = image::load_from_memory(bytes)
        .unwrap_or_else(|e| panic!("Failed to decode {name}: {e}"));
    // Downscale if either dimension exceeds egui's max texture size (2048)
    const MAX_SIDE: u32 = 2048;
    let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        let scale = MAX_SIDE as f64 / img.width().max(img.height()) as f64;
        let new_w = (img.width() as f64 * scale).round() as u32;
        let new_h = (img.height() as f64 * scale).round() as u32;
        img.resize(new_w, new_h, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    ctx.load_texture(name, color, egui::TextureOptions::LINEAR)
}

/// Build a funky rainbow-colored LayoutJob for the welcome text.
fn welcome_text_job() -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob { halign: egui::Align::Center, ..Default::default() };

    let text = "WELCOM TO GLOTTISDALE";
    let colors = [
        egui::Color32::from_rgb(255, 87, 34),   // deep orange
        egui::Color32::from_rgb(255, 193, 7),    // amber
        egui::Color32::from_rgb(76, 175, 80),    // green
        egui::Color32::from_rgb(33, 150, 243),   // blue
        egui::Color32::from_rgb(156, 39, 176),   // purple
        egui::Color32::from_rgb(233, 30, 99),    // pink
        egui::Color32::from_rgb(0, 188, 212),    // cyan
    ];

    for (i, ch) in text.chars().enumerate() {
        let color = colors[i % colors.len()];
        job.append(
            &ch.to_string(),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(36.0),
                color,
                ..Default::default()
            },
        );
    }
    job
}

impl GlottisdaleApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let icon_texture = load_texture(
            &cc.egui_ctx,
            "app-icon",
            include_bytes!("../assets/icon.jpg"),
        );
        let banner_texture = load_texture(
            &cc.egui_ctx,
            "app-banner",
            include_bytes!("../assets/banner.jpg"),
        );

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
            editor: None,
            icon_texture,
            banner_texture,
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

        // Auto-open editor after alignment-only run
        if self.editor.is_none() && self.processing.take_auto_open_editor() {
            try_open_editor_from_alignment(self);
        }

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let icon_size = self.icon_texture.size_vec2() * (20.0 / self.icon_texture.size_vec2().y);
                ui.image(egui::load::SizedTexture::new(self.icon_texture.id(), icon_size));
                ui.label(egui::RichText::new("Glottisdale").strong());
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
            .min_width(120.0)
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
                            if ui.small_button("x").clicked() {
                                to_remove = Some(i);
                            }
                            ui.add(egui::Label::new(&name).truncate());
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

        // Central panel: main workspace or editor
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref mut editor_state) = self.editor {
                if crate::editor::show_editor(ui, editor_state, ctx) {
                    self.editor = None; // Close editor
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);

                    // Icon
                    let icon_size = self.icon_texture.size_vec2()
                        * (64.0 / self.icon_texture.size_vec2().y);
                    ui.image(egui::load::SizedTexture::new(
                        self.icon_texture.id(),
                        icon_size,
                    ));

                    ui.add_space(4.0);

                    // Funky welcome text
                    ui.label(welcome_text_job());

                    ui.add_space(8.0);
                    ui.heading(format!("{} Workspace", self.mode.label()));
                    ui.add_space(10.0);
                });

                match self.mode {
                    PipelineMode::Collage => show_collage_workspace(ui, self),
                    PipelineMode::Sing => show_sing_workspace(ui, self),
                    PipelineMode::Speak => show_speak_workspace(ui, self),
                }
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

/// Show output files with Play and Open Folder buttons. Used by all workspace panels.
/// Returns true if the "Edit Arrangement" button was clicked.
fn show_output_section(ui: &mut egui::Ui, processing: &ProcessingState) -> bool {
    let mut edit_clicked = false;
    match processing.get_status() {
        ProcessingStatus::Done(msg) => {
            ui.separator();
            ui.colored_label(egui::Color32::GREEN, &msg);

            let outputs = processing.get_outputs();
            if !outputs.is_empty() {
                ui.add_space(8.0);

                // Open Folder button (use parent dir of first output)
                if let Some(run_dir) = outputs.first().and_then(|(_, p)| p.parent()) {
                    ui.horizontal(|ui| {
                        if ui.button("Open Folder").clicked() {
                            open_path(run_dir);
                        }
                        ui.monospace(run_dir.display().to_string());
                    });
                }

                ui.add_space(4.0);

                for (label, path) in &outputs {
                    ui.horizontal(|ui| {
                        if ui.button("Play").clicked() {
                            open_path(path);
                        }
                        let filename = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.display().to_string());
                        ui.label(format!("{}: {}", label, filename));
                    });
                }
            }

            // Edit Arrangement button (available when alignment data exists)
            if processing.has_alignment() {
                ui.add_space(8.0);
                if ui.button("Edit Arrangement").clicked() {
                    edit_clicked = true;
                }
            }
        }
        ProcessingStatus::Error(msg) => {
            ui.separator();
            ui.colored_label(egui::Color32::RED, &msg);
        }
        _ => {}
    }
    edit_clicked
}

/// Build an arrangement from stored alignment data and open the editor.
fn try_open_editor_from_alignment(app: &mut GlottisdaleApp) {
    if let Some(data) = app.processing.get_alignment() {
        match arrangement_blank_canvas(&data.syllables, &data.audio, data.pipeline_mode) {
            Ok(arrangement) => {
                app.editor = Some(crate::editor::EditorState::new(arrangement));
            }
            Err(e) => {
                log::error!("Failed to build arrangement: {}", e);
                app.processing.add_log(&format!("Failed to open editor: {}", e));
            }
        }
    }
}

fn show_collage_workspace(ui: &mut egui::Ui, app: &mut GlottisdaleApp) {
    if app.source_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            let banner_size = app.banner_texture.size_vec2()
                * (200.0 / app.banner_texture.size_vec2().y);
            ui.image(egui::load::SizedTexture::new(
                app.banner_texture.id(),
                banner_size,
            ));
            ui.add_space(12.0);
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
        if ui.add_enabled(can_run, egui::Button::new("Build Bank & Edit")).clicked() {
            start_alignment_only(app);
        }
        if app.is_processing() {
            ui.spinner();
        }
    });

    ui.separator();

    ui.label(format!("{} source file(s) loaded", app.source_files.len()));
    for path in &app.source_files {
        ui.monospace(path.display().to_string());
    }

    if show_output_section(ui, &app.processing) {
        try_open_editor_from_alignment(app);
    }
}

fn show_sing_workspace(ui: &mut egui::Ui, app: &mut GlottisdaleApp) {
    if app.source_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            let banner_size = app.banner_texture.size_vec2()
                * (200.0 / app.banner_texture.size_vec2().y);
            ui.image(egui::load::SizedTexture::new(
                app.banner_texture.id(),
                banner_size,
            ));
            ui.add_space(12.0);
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
        if ui.add_enabled(can_run, egui::Button::new("Build Bank & Edit")).clicked() {
            start_alignment_only(app);
        }
        if app.is_processing() {
            ui.spinner();
        }
    });

    ui.separator();
    ui.label(format!("{} source file(s)", app.source_files.len()));
    ui.label(format!("MIDI: {}", app.sing.midi_dir));

    if show_output_section(ui, &app.processing) {
        try_open_editor_from_alignment(app);
    }
}

fn show_speak_workspace(ui: &mut egui::Ui, app: &mut GlottisdaleApp) {
    if app.source_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            let banner_size = app.banner_texture.size_vec2()
                * (200.0 / app.banner_texture.size_vec2().y);
            ui.image(egui::load::SizedTexture::new(
                app.banner_texture.id(),
                banner_size,
            ));
            ui.add_space(12.0);
            ui.label("Add source audio files to get started.");
            ui.label("Then enter target text or select a reference audio.");
        });
        return;
    }

    let has_target = !app.speak.target_text.is_empty() || !app.speak.reference_path.is_empty();

    ui.horizontal(|ui| {
        let can_run = !app.is_processing() && !app.source_files.is_empty() && has_target;
        let can_bank = !app.is_processing() && !app.source_files.is_empty();
        if ui.add_enabled(can_run, egui::Button::new("Run Speak")).clicked() {
            start_speak(app);
        }
        if ui.add_enabled(can_bank, egui::Button::new("Build Bank & Edit")).clicked() {
            start_alignment_only(app);
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

    if show_output_section(ui, &app.processing) {
        try_open_editor_from_alignment(app);
    }
}

// ─── Pipeline runners (background threads) ──────────────────────

/// Extract audio from input files to 16kHz mono WAV in a work directory.
fn prepare_audio(
    inputs: &[PathBuf],
    work_dir: &Path,
    state: &ProcessingState,
) -> anyhow::Result<Vec<PathBuf>> {
    use glottisdale_core::audio::io::extract_audio;

    std::fs::create_dir_all(work_dir)?;
    let mut audio_paths = Vec::new();
    for input in inputs {
        let stem = input
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "input".to_string());
        let wav_path = work_dir.join(format!("{}_16k.wav", stem));
        state.add_log(&format!("Extracting audio: {}", input.display()));
        extract_audio(input, &wav_path)?;
        audio_paths.push(wav_path);
    }
    Ok(audio_paths)
}

/// Parse a seed string into Option<u64>.
fn parse_seed(s: &str) -> Option<u64> {
    if s.is_empty() { None } else { s.parse().ok() }
}

fn start_collage(app: &mut GlottisdaleApp) {
    use glottisdale_core::audio::io::read_wav;
    use glottisdale_core::collage::process::{CollageConfig, process};
    use glottisdale_core::collage::stretch::{StretchConfig, parse_stretch_factor};
    use glottisdale_core::language::align::get_aligner;
    use glottisdale_core::names::create_run_dir;

    let state = app.processing.clone();
    state.clear();
    state.set_status(ProcessingStatus::Running("Starting collage...".into()));

    let inputs = app.source_files.clone();
    let output_dir = PathBuf::from(&app.output_dir);
    let seed = parse_seed(&app.seed);
    let run_name = if app.run_name.is_empty() { None } else { Some(app.run_name.clone()) };
    let whisper_model = app.whisper_model.clone();
    let aligner_name = app.aligner.clone();
    let settings = app.collage.clone();

    thread::spawn(move || {
        let result: anyhow::Result<()> = (|| {
            let run_dir = create_run_dir(&output_dir, seed, run_name.as_deref())?;
            let run_dir_name = run_dir.file_name().unwrap().to_string_lossy().to_string();
            state.add_log(&format!("Run: {}", run_dir_name));

            let work_dir = run_dir.join("work");
            let audio_paths = prepare_audio(&inputs, &work_dir, &state)?;

            state.add_log("Aligning syllables...");
            state.set_status(ProcessingStatus::Running("Aligning...".into()));
            let aligner = get_aligner(&aligner_name, &whisper_model, "en", "cpu")?;
            let mut source_audio = std::collections::HashMap::new();
            let mut source_syllables = std::collections::HashMap::new();

            for audio_path in &audio_paths {
                let key = audio_path.to_string_lossy().to_string();
                state.add_log(&format!("Aligning: {}", audio_path.file_name().unwrap().to_string_lossy()));
                let alignment = aligner.process(audio_path, None)?;
                let (samples, sr) = read_wav(audio_path)?;
                source_audio.insert(key.clone(), (samples, sr));
                source_syllables.insert(key, alignment.syllables);
            }

            let total_syls: usize = source_syllables.values().map(|v| v.len()).sum();
            state.add_log(&format!("Found {} syllables", total_syls));

            // Store alignment data for the editor (clone before process borrows)
            let alignment_syllables = source_syllables.clone();
            let alignment_audio = source_audio.clone();

            state.add_log("Assembling collage...");
            state.set_status(ProcessingStatus::Running("Assembling...".into()));

            let s = &settings;
            let config = CollageConfig {
                syllables_per_clip: s.syllables_per_word.clone(),
                target_duration: s.target_duration,
                crossfade_ms: s.crossfade_ms,
                padding_ms: s.padding_ms,
                words_per_phrase: s.words_per_phrase.clone(),
                phrases_per_sentence: s.phrases_per_sentence.clone(),
                phrase_pause: s.phrase_pause.clone(),
                sentence_pause: s.sentence_pause.clone(),
                word_crossfade_ms: s.word_crossfade_ms,
                seed,
                noise_level_db: s.noise_level_db,
                room_tone: s.room_tone,
                pitch_normalize: s.pitch_normalize,
                pitch_range: s.pitch_range,
                breaths: s.breaths,
                breath_probability: s.breath_probability,
                volume_normalize: s.volume_normalize,
                prosodic_dynamics: s.prosodic_dynamics,
                speed: if s.speed.is_empty() { None } else { s.speed.parse().ok() },
                stretch_config: StretchConfig {
                    random_stretch: if s.random_stretch.is_empty() { None } else { s.random_stretch.parse().ok() },
                    alternating_stretch: if s.alternating_stretch.is_empty() { None } else { s.alternating_stretch.parse().ok() },
                    boundary_stretch: if s.boundary_stretch.is_empty() { None } else { s.boundary_stretch.parse().ok() },
                    word_stretch: if s.word_stretch.is_empty() { None } else { s.word_stretch.parse().ok() },
                    stretch_factor: parse_stretch_factor(&s.stretch_factor),
                },
                repeat_weight: if s.repeat_weight.is_empty() { None } else { s.repeat_weight.parse().ok() },
                repeat_count: s.repeat_count.clone(),
                repeat_style: "exact".to_string(),
                stutter: if s.stutter.is_empty() { None } else { s.stutter.parse().ok() },
                stutter_count: s.stutter_count.clone(),
            };

            let result = process(&source_audio, &source_syllables, &run_dir, &config)?;
            state.add_output("Output", result.concatenated);
            state.add_log(&format!("Selected {} clips", result.clips.len()));

            state.store_alignment(AlignmentData {
                syllables: alignment_syllables,
                audio: alignment_audio,
                pipeline_mode: EditorPipelineMode::Collage,
            });

            Ok(())
        })();

        match result {
            Ok(()) => state.set_status(ProcessingStatus::Done("Completed successfully".into())),
            Err(e) => {
                state.add_log(&format!("ERROR: {:#}", e));
                state.set_status(ProcessingStatus::Error(format!("{}", e)));
            }
        }
    });
}

fn start_sing(app: &mut GlottisdaleApp) {
    use glottisdale_core::audio::io::read_wav;
    use glottisdale_core::language::align::get_aligner;
    use glottisdale_core::names::create_run_dir;
    use glottisdale_core::sing::midi_parser::parse_midi;
    use glottisdale_core::sing::syllable_prep::{prepare_syllables, median_f0};
    use glottisdale_core::sing::vocal_mapper::{plan_note_mapping, render_vocal_track};
    use glottisdale_core::sing::mixer::mix_tracks;

    let state = app.processing.clone();
    state.clear();
    state.set_status(ProcessingStatus::Running("Starting sing...".into()));

    let inputs = app.source_files.clone();
    let output_dir = PathBuf::from(&app.output_dir);
    let seed = parse_seed(&app.seed);
    let run_name = if app.run_name.is_empty() { None } else { Some(app.run_name.clone()) };
    let whisper_model = app.whisper_model.clone();
    let settings = app.sing.clone();

    thread::spawn(move || {
        let result: anyhow::Result<()> = (|| {
            let midi_dir = PathBuf::from(&settings.midi_dir);
            let melody_path = midi_dir.join("melody.mid");
            if !melody_path.exists() {
                anyhow::bail!("MIDI melody not found: {}", melody_path.display());
            }

            let run_dir = create_run_dir(&output_dir, seed, run_name.as_deref())?;
            let run_dir_name = run_dir.file_name().unwrap().to_string_lossy().to_string();
            state.add_log(&format!("Run: {}", run_dir_name));

            let work_dir = run_dir.join("work");
            let audio_paths = prepare_audio(&inputs, &work_dir, &state)?;

            state.add_log("Parsing MIDI...");
            let track = parse_midi(&melody_path)?;
            state.add_log(&format!("Melody: {} notes, {:.0} BPM", track.notes.len(), track.tempo));

            state.set_status(ProcessingStatus::Running("Aligning...".into()));
            let aligner = get_aligner("auto", &whisper_model, "en", "cpu")?;
            let mut all_syllable_clips = Vec::new();
            let mut sample_rate = 16000u32;
            let mut source_syllables = HashMap::new();
            let mut source_audio_map = HashMap::new();

            for audio_path in &audio_paths {
                let key = audio_path.to_string_lossy().to_string();
                state.add_log(&format!("Aligning: {}", audio_path.file_name().unwrap().to_string_lossy()));
                let alignment = aligner.process(audio_path, None)?;
                let (samples, sr) = read_wav(audio_path)?;
                sample_rate = sr;
                let prepared = prepare_syllables(&alignment.syllables, &samples, sr, 12.0);
                all_syllable_clips.extend(prepared);
                source_syllables.insert(key.clone(), alignment.syllables);
                source_audio_map.insert(key, (samples, sr));
            }

            state.add_log(&format!("Prepared {} syllable clips", all_syllable_clips.len()));
            state.store_alignment(AlignmentData {
                syllables: source_syllables,
                audio: source_audio_map,
                pipeline_mode: EditorPipelineMode::Sing,
            });
            if all_syllable_clips.is_empty() {
                anyhow::bail!("No syllables found in source audio");
            }

            let med_f0 = median_f0(&all_syllable_clips).unwrap_or(220.0);
            state.add_log(&format!("Median F0: {:.1} Hz", med_f0));

            let chorus_prob = if settings.chorus { 0.3 } else { 0.0 };
            let mappings = plan_note_mapping(
                &track.notes,
                all_syllable_clips.len(),
                seed,
                settings.drift_range,
                chorus_prob,
            );

            state.set_status(ProcessingStatus::Running("Rendering...".into()));
            state.add_log("Rendering vocal track...");
            let vocal_samples = render_vocal_track(&mappings, &all_syllable_clips, med_f0, sample_rate);

            if vocal_samples.is_empty() {
                anyhow::bail!("Vocal rendering produced no output");
            }

            // Parse backing MIDI tracks
            let mut backing_tracks = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&midi_dir) {
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

            state.add_log("Mixing tracks...");
            let (full_mix, acappella) = mix_tracks(
                &vocal_samples, sample_rate, &backing_tracks, &run_dir, 0.0, -12.0,
            )?;

            state.add_output("Output", full_mix);
            state.add_output("A cappella", acappella);

            Ok(())
        })();

        match result {
            Ok(()) => state.set_status(ProcessingStatus::Done("Completed successfully".into())),
            Err(e) => {
                state.add_log(&format!("ERROR: {:#}", e));
                state.set_status(ProcessingStatus::Error(format!("{}", e)));
            }
        }
    });
}

fn start_speak(app: &mut GlottisdaleApp) {
    use glottisdale_core::audio::io::{extract_audio, read_wav};
    use glottisdale_core::language::align::get_aligner;
    use glottisdale_core::names::create_run_dir;
    use glottisdale_core::speak::syllable_bank::build_bank;
    use glottisdale_core::speak::target_text::{text_to_syllables, word_boundaries_from_syllables};
    use glottisdale_core::speak::matcher::{match_syllables, match_phonemes};
    use glottisdale_core::speak::assembler::{plan_timing, assemble};

    let state = app.processing.clone();
    state.clear();
    state.set_status(ProcessingStatus::Running("Starting speak...".into()));

    let inputs = app.source_files.clone();
    let output_dir = PathBuf::from(&app.output_dir);
    let seed = parse_seed(&app.seed);
    let run_name = if app.run_name.is_empty() { None } else { Some(app.run_name.clone()) };
    let whisper_model = app.whisper_model.clone();
    let aligner_name = app.aligner.clone();
    let settings = app.speak.clone();

    thread::spawn(move || {
        let result: anyhow::Result<()> = (|| {
            if settings.target_text.is_empty() && settings.reference_path.is_empty() {
                anyhow::bail!("Either target text or reference audio is required");
            }

            let run_dir = create_run_dir(&output_dir, seed, run_name.as_deref())?;
            let run_dir_name = run_dir.file_name().unwrap().to_string_lossy().to_string();
            state.add_log(&format!("Run: {}", run_dir_name));

            let work_dir = run_dir.join("work");
            let audio_paths = prepare_audio(&inputs, &work_dir, &state)?;

            state.set_status(ProcessingStatus::Running("Building syllable bank...".into()));
            state.add_log("Building source syllable bank...");
            let aligner = get_aligner(&aligner_name, &whisper_model, "en", "cpu")?;
            let mut all_bank_entries = Vec::new();
            let mut source_audio = std::collections::HashMap::new();
            let mut source_syllables = std::collections::HashMap::new();

            for audio_path in &audio_paths {
                let key = audio_path.to_string_lossy().to_string();
                state.add_log(&format!("Aligning: {}", audio_path.file_name().unwrap().to_string_lossy()));
                let alignment = aligner.process(audio_path, None)?;
                let entries = build_bank(&alignment.syllables, &key);
                state.add_log(&format!("  {} syllables", entries.len()));
                all_bank_entries.extend(entries);
                source_syllables.insert(key.clone(), alignment.syllables);

                let (samples, sr) = read_wav(audio_path)?;
                source_audio.insert(key, (samples, sr));
            }

            state.add_log(&format!("Syllable bank: {} total entries", all_bank_entries.len()));

            // Get target text
            let mut target_text = if settings.target_text.is_empty() {
                None
            } else {
                Some(settings.target_text.clone())
            };
            let mut reference_timings: Option<Vec<(f64, f64)>> = None;

            if !settings.reference_path.is_empty() {
                let ref_path = PathBuf::from(&settings.reference_path);
                state.add_log(&format!("Transcribing reference: {}", ref_path.display()));
                let ref_wav = work_dir.join("reference_16k.wav");
                extract_audio(&ref_path, &ref_wav)?;
                let ref_alignment = aligner.process(&ref_wav, None)?;
                target_text = Some(ref_alignment.text);
                reference_timings = Some(
                    ref_alignment.syllables.iter().map(|s| (s.start, s.end)).collect(),
                );
            }

            let target_text = target_text
                .ok_or_else(|| anyhow::anyhow!("No target text (use text or reference)"))?;
            state.add_log(&format!("Target text: {}", target_text));

            let target_syls = text_to_syllables(&target_text);
            let word_bounds = word_boundaries_from_syllables(&target_syls);
            state.add_log(&format!("Target: {} syllables, {} words", target_syls.len(), word_bounds.len()));

            state.set_status(ProcessingStatus::Running("Matching...".into()));
            state.add_log(&format!("Matching ({} mode)...", settings.match_unit));

            let matches = if settings.match_unit == "phoneme" {
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
                    None,
                )
            };

            let avg_dur = if all_bank_entries.is_empty() {
                0.25
            } else {
                all_bank_entries.iter().map(|e| e.duration()).sum::<f64>()
                    / all_bank_entries.len() as f64
            };

            let timing = plan_timing(
                &matches,
                &word_bounds,
                avg_dur,
                reference_timings.as_deref(),
                settings.timing_strictness,
            );

            state.set_status(ProcessingStatus::Running("Assembling...".into()));
            state.add_log("Assembling output audio...");
            let output_path = assemble(
                &matches,
                &timing,
                &source_audio,
                &run_dir,
                settings.crossfade_ms,
                None,
                settings.normalize_volume,
                settings.pitch_correct,
            )?;

            state.add_output("Output", output_path);

            state.store_alignment(AlignmentData {
                syllables: source_syllables,
                audio: source_audio,
                pipeline_mode: EditorPipelineMode::Speak,
            });

            Ok(())
        })();

        match result {
            Ok(()) => state.set_status(ProcessingStatus::Done("Completed successfully".into())),
            Err(e) => {
                state.add_log(&format!("ERROR: {:#}", e));
                state.set_status(ProcessingStatus::Error(format!("{}", e)));
            }
        }
    });
}

/// Run alignment only and auto-open the editor when done.
fn start_alignment_only(app: &mut GlottisdaleApp) {
    use glottisdale_core::audio::io::read_wav;
    use glottisdale_core::language::align::get_aligner;

    let state = app.processing.clone();
    state.clear();
    state.set_status(ProcessingStatus::Running("Building syllable bank...".into()));

    let inputs = app.source_files.clone();
    let whisper_model = app.whisper_model.clone();
    let aligner_name = app.aligner.clone();
    let pipeline_mode = app.mode.to_editor_mode();

    thread::spawn(move || {
        let result: anyhow::Result<()> = (|| {
            let work_dir = std::env::temp_dir().join("glottisdale-alignment");
            let audio_paths = prepare_audio(&inputs, &work_dir, &state)?;

            state.add_log("Aligning syllables...");
            state.set_status(ProcessingStatus::Running("Aligning...".into()));
            let aligner = get_aligner(&aligner_name, &whisper_model, "en", "cpu")?;

            let mut source_syllables = HashMap::new();
            let mut source_audio = HashMap::new();

            for audio_path in &audio_paths {
                let key = audio_path.to_string_lossy().to_string();
                state.add_log(&format!(
                    "Aligning: {}",
                    audio_path.file_name().unwrap().to_string_lossy()
                ));
                let alignment = aligner.process(audio_path, None)?;
                let (samples, sr) = read_wav(audio_path)?;
                source_syllables.insert(key.clone(), alignment.syllables);
                source_audio.insert(key, (samples, sr));
            }

            let total_syls: usize = source_syllables.values().map(|v| v.len()).sum();
            state.add_log(&format!("Found {} syllables", total_syls));

            state.store_alignment(AlignmentData {
                syllables: source_syllables,
                audio: source_audio,
                pipeline_mode,
            });

            state.set_auto_open_editor();

            Ok(())
        })();

        match result {
            Ok(()) => state.set_status(ProcessingStatus::Done("Bank ready".into())),
            Err(e) => {
                state.add_log(&format!("ERROR: {:#}", e));
                state.set_status(ProcessingStatus::Error(format!("{}", e)));
            }
        }
    });
}

/// Open a file or directory in the system's default handler.
fn open_path(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn().ok();
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(path).spawn().ok();
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", ""])
            .arg(path)
            .spawn()
            .ok();
    }
}

