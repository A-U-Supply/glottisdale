//! Interactive syllable editor GUI.

pub mod timeline;
pub mod waveform_painter;

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use glottisdale_core::editor::{
    Arrangement, ClipEffect, ClipId, TimelineClip,
    effects_chain::compute_effective_duration,
    playback_engine::PlaybackEngine,
    render::render_arrangement,
};

use self::timeline::TimelineState;

/// Full editor state.
pub struct EditorState {
    pub arrangement: Arrangement,
    pub timeline: TimelineState,
    pub playback: PlaybackEngine,
    /// Map from source file path to color index.
    pub source_indices: HashMap<PathBuf, usize>,
    /// Search filter for the bank panel.
    pub bank_filter: String,
}

impl EditorState {
    pub fn new(arrangement: Arrangement) -> Self {
        // Build source index map
        let mut source_indices = HashMap::new();
        let mut next_idx = 0usize;
        for clip in &arrangement.bank {
            source_indices
                .entry(clip.source_path.clone())
                .or_insert_with(|| {
                    let idx = next_idx;
                    next_idx += 1;
                    idx
                });
        }

        Self {
            arrangement,
            timeline: TimelineState::default(),
            playback: PlaybackEngine::new(),
            source_indices,
            bank_filter: String::new(),
        }
    }

    /// Shuffle the selected clips randomly.
    pub fn shuffle_selected(&mut self) {
        use rand::seq::SliceRandom;

        let selected = self.timeline.selected.clone();
        if selected.len() < 2 {
            return;
        }

        let indices: Vec<usize> = self
            .arrangement
            .timeline
            .iter()
            .enumerate()
            .filter(|(_, tc)| selected.contains(&tc.id))
            .map(|(i, _)| i)
            .collect();

        let mut rng = rand::thread_rng();
        let original_clips: Vec<_> = indices
            .iter()
            .map(|&i| self.arrangement.timeline[i].clone())
            .collect();

        let mut shuffled = original_clips.clone();
        shuffled.shuffle(&mut rng);

        for (slot, clip) in indices.iter().zip(shuffled.into_iter()) {
            self.arrangement.timeline[*slot] = clip;
        }

        self.arrangement.relayout(0.0);
    }

    /// Delete selected clips from the timeline.
    pub fn delete_selected(&mut self) {
        let selected = &self.timeline.selected;
        self.arrangement
            .timeline
            .retain(|tc| !selected.contains(&tc.id));
        self.timeline.selected.clear();
        self.arrangement.relayout(0.0);
    }

    /// Apply an effect to all selected clips.
    pub fn apply_effect_to_selected(&mut self, effect: ClipEffect) {
        let selected = &self.timeline.selected;
        for tc in &mut self.arrangement.timeline {
            if selected.contains(&tc.id) {
                tc.effects.push(effect.clone());
                if let Some(source) = self
                    .arrangement
                    .bank
                    .iter()
                    .find(|c| c.id == tc.source_clip_id)
                {
                    tc.effective_duration_s =
                        compute_effective_duration(source.duration_s(), &tc.effects);
                }
            }
        }
        self.arrangement.relayout(0.0);
    }

    /// Clear all effects from selected clips.
    pub fn clear_effects_selected(&mut self) {
        let selected = &self.timeline.selected;
        for tc in &mut self.arrangement.timeline {
            if selected.contains(&tc.id) {
                tc.effects.clear();
                if let Some(source) = self
                    .arrangement
                    .bank
                    .iter()
                    .find(|c| c.id == tc.source_clip_id)
                {
                    tc.effective_duration_s = source.duration_s();
                }
            }
        }
        self.arrangement.relayout(0.0);
    }

    /// Play the arrangement from the current cursor position.
    pub fn play_from_cursor(&self) {
        if let Ok(samples) = render_arrangement(&self.arrangement) {
            let sr = self.arrangement.sample_rate;
            let cursor = self.timeline.cursor_s;
            let start_sample = (cursor * sr as f64).round() as usize;
            let play_samples = if start_sample < samples.len() {
                samples[start_sample..].to_vec()
            } else {
                vec![]
            };
            self.playback.play_samples(play_samples, sr, cursor);
        }
    }

    /// Play a single bank clip (preview).
    pub fn play_clip(&self, clip_id: ClipId) {
        if let Some(clip) = self.arrangement.get_bank_clip(clip_id) {
            self.playback
                .play_samples(clip.samples.clone(), clip.sample_rate, 0.0);
        }
    }
}

/// Main entry point: render the full editor UI.
pub fn show_editor(ui: &mut egui::Ui, state: &mut EditorState, ctx: &egui::Context) -> bool {
    let mut close = false;

    // Update cursor from playback engine
    state.timeline.cursor_s = state.playback.state.get_cursor();
    if state.playback.state.is_playing() {
        ctx.request_repaint();
    }

    // Toolbar
    ui.horizontal(|ui| {
        if ui.button("Close Editor").clicked() {
            close = true;
        }
        ui.separator();

        let has_selection = !state.timeline.selected.is_empty();

        if ui
            .add_enabled(has_selection, egui::Button::new("Shuffle"))
            .clicked()
        {
            state.shuffle_selected();
        }
        if ui
            .add_enabled(has_selection, egui::Button::new("Delete"))
            .clicked()
        {
            state.delete_selected();
        }
        if ui
            .add_enabled(has_selection, egui::Button::new("Clear FX"))
            .clicked()
        {
            state.clear_effects_selected();
        }

        ui.separator();

        // Playback controls
        let playing = state.playback.state.is_playing();
        if ui
            .button(if playing { "Pause" } else { "Play" })
            .clicked()
        {
            if playing {
                state.playback.pause();
            } else {
                state.play_from_cursor();
            }
        }
        if ui.button("Stop").clicked() {
            state.playback.stop();
        }

        ui.separator();

        // Zoom
        ui.label("Zoom:");
        if ui.button("-").clicked() {
            state.timeline.pixels_per_second =
                (state.timeline.pixels_per_second * 0.7).max(10.0);
        }
        if ui.button("+").clicked() {
            state.timeline.pixels_per_second =
                (state.timeline.pixels_per_second * 1.4).min(5000.0);
        }
        ui.label(format!("{:.0} px/s", state.timeline.pixels_per_second));

        ui.separator();

        // Export
        if ui.button("Export WAV").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("arrangement.wav")
                .add_filter("WAV audio", &["wav"])
                .save_file()
            {
                if let Err(e) =
                    glottisdale_core::editor::render::export_arrangement(&state.arrangement, &path)
                {
                    log::error!("Export failed: {}", e);
                }
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let n_clips = state.arrangement.timeline.len();
            let dur = state.arrangement.total_duration_s();
            ui.label(format!("{} clips | {:.1}s", n_clips, dur));
        });
    });

    ui.separator();

    // Main area: bank panel on left, timeline on right
    egui::SidePanel::left("editor_bank")
        .min_width(150.0)
        .default_width(200.0)
        .resizable(true)
        .show_inside(ui, |ui| {
            show_bank_panel(ui, state);
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            timeline::show_timeline(
                ui,
                &state.arrangement,
                &mut state.timeline,
                &state.source_indices,
            );
        });
    });

    close
}

/// Show the syllable bank/palette panel.
fn show_bank_panel(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.heading("Syllable Bank");
    ui.add(
        egui::TextEdit::singleline(&mut state.bank_filter)
            .hint_text("Filter...")
            .desired_width(ui.available_width()),
    );
    ui.separator();

    let filter = state.bank_filter.to_lowercase();

    // Collect actions to apply after iterating (avoids borrow conflicts)
    let mut clip_to_add: Option<ClipId> = None;
    let mut clip_to_play: Option<ClipId> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for clip in &state.arrangement.bank {
            // Filter
            if !filter.is_empty()
                && !clip.label.to_lowercase().contains(&filter)
                && !clip.syllable.word.to_lowercase().contains(&filter)
            {
                continue;
            }

            let response = ui
                .horizontal(|ui| {
                    // Mini waveform
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(40.0, 24.0), egui::Sense::click());
                    if ui.is_rect_visible(rect) {
                        let src_idx = state
                            .source_indices
                            .get(&clip.source_path)
                            .copied()
                            .unwrap_or(0);
                        let color =
                            timeline::SOURCE_COLORS[src_idx % timeline::SOURCE_COLORS.len()];
                        waveform_painter::paint_waveform(
                            ui.painter(),
                            rect,
                            &clip.waveform,
                            egui::Color32::from_rgb(color.0, color.1, color.2),
                        );
                    }

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&clip.label).small().monospace());
                        ui.label(
                            egui::RichText::new(format!(
                                "{} ({:.2}s)",
                                clip.syllable.word,
                                clip.duration_s()
                            ))
                            .small()
                            .weak(),
                        );
                    });

                    resp
                })
                .inner;

            // Double-click to preview, single-click to add to timeline
            if response.double_clicked() {
                clip_to_play = Some(clip.id);
            } else if response.clicked() {
                clip_to_add = Some(clip.id);
            }
        }
    });

    // Apply deferred actions
    if let Some(id) = clip_to_add {
        let tc = state
            .arrangement
            .get_bank_clip(id)
            .map(TimelineClip::new);
        if let Some(tc) = tc {
            state.arrangement.timeline.push(tc);
            state.arrangement.relayout(0.0);
        }
    }
    if let Some(id) = clip_to_play {
        state.play_clip(id);
    }
}
