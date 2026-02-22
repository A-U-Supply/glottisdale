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

/// Action from the context menu to apply after rendering.
enum ContextAction {
    Stutter(ClipId, usize),
    Stretch(ClipId, f64),
    Pitch(ClipId, f64),
    Duplicate(ClipId),
    Delete(ClipId),
    ClearEffects(ClipId),
}

/// Full editor state.
pub struct EditorState {
    pub arrangement: Arrangement,
    pub timeline: TimelineState,
    pub playback: PlaybackEngine,
    /// Map from source file path to color index.
    pub source_indices: HashMap<PathBuf, usize>,
    /// Search filter for the bank panel.
    pub bank_filter: String,
    /// Last audio/playback error to display.
    pub audio_error: Option<String>,
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
            audio_error: None,
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

    #[allow(dead_code)]
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
        if self.arrangement.timeline.is_empty() {
            log::warn!("Nothing to play — timeline is empty");
            return;
        }
        match render_arrangement(&self.arrangement) {
            Ok(samples) => {
                if samples.is_empty() {
                    log::warn!("Render produced no audio");
                    return;
                }
                let sr = self.arrangement.sample_rate;
                let cursor = self.timeline.cursor_s;
                let start_sample = (cursor * sr as f64).round() as usize;
                let play_samples = if start_sample < samples.len() {
                    samples[start_sample..].to_vec()
                } else {
                    log::warn!("Cursor past end of arrangement");
                    return;
                };
                self.playback.play_samples(play_samples, sr, cursor);
            }
            Err(e) => {
                log::error!("Render failed: {}", e);
                self.playback.state.set_error(format!("Render: {}", e));
            }
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

/// Apply a context menu action to the editor state.
fn apply_context_action(state: &mut EditorState, action: ContextAction) {
    match action {
        ContextAction::Stutter(clip_id, count) => {
            apply_effect_to_clip(state, clip_id, ClipEffect::Stutter { count });
        }
        ContextAction::Stretch(clip_id, factor) => {
            apply_effect_to_clip(state, clip_id, ClipEffect::TimeStretch { factor });
        }
        ContextAction::Pitch(clip_id, semitones) => {
            apply_effect_to_clip(state, clip_id, ClipEffect::PitchShift { semitones });
        }
        ContextAction::Duplicate(clip_id) => {
            if let Some(tc_idx) = state
                .arrangement
                .timeline
                .iter()
                .position(|tc| tc.id == clip_id)
            {
                let tc = &state.arrangement.timeline[tc_idx];
                let new_tc = TimelineClip {
                    id: uuid::Uuid::new_v4(),
                    source_clip_id: tc.source_clip_id,
                    position_s: 0.0,
                    effects: tc.effects.clone(),
                    effective_duration_s: tc.effective_duration_s,
                };
                state.arrangement.timeline.insert(tc_idx + 1, new_tc);
                state.arrangement.relayout(0.0);
            }
        }
        ContextAction::Delete(clip_id) => {
            state.arrangement.timeline.retain(|tc| tc.id != clip_id);
            state.timeline.selected.retain(|&id| id != clip_id);
            state.arrangement.relayout(0.0);
        }
        ContextAction::ClearEffects(clip_id) => {
            for tc in &mut state.arrangement.timeline {
                if tc.id == clip_id {
                    tc.effects.clear();
                    if let Some(source) = state
                        .arrangement
                        .bank
                        .iter()
                        .find(|c| c.id == tc.source_clip_id)
                    {
                        tc.effective_duration_s = source.duration_s();
                    }
                }
            }
            state.arrangement.relayout(0.0);
        }
    }
}

/// Apply a single effect to a specific clip by ID.
fn apply_effect_to_clip(state: &mut EditorState, clip_id: ClipId, effect: ClipEffect) {
    for tc in &mut state.arrangement.timeline {
        if tc.id == clip_id {
            tc.effects.push(effect);
            if let Some(source) = state
                .arrangement
                .bank
                .iter()
                .find(|c| c.id == tc.source_clip_id)
            {
                tc.effective_duration_s =
                    compute_effective_duration(source.duration_s(), &tc.effects);
            }
            break;
        }
    }
    state.arrangement.relayout(0.0);
}

/// Render context menu items for a clip.
fn show_clip_context_menu(ui: &mut egui::Ui, clip_id: ClipId, action: &mut Option<ContextAction>) {
    ui.menu_button("Stutter", |ui| {
        for count in 2..=8 {
            if ui.button(format!("x{}", count)).clicked() {
                *action = Some(ContextAction::Stutter(clip_id, count));
                ui.close_menu();
            }
        }
    });

    ui.menu_button("Time Stretch", |ui| {
        for &factor in &[0.5, 0.75, 1.5, 2.0, 3.0, 4.0] {
            if ui.button(format!("{:.2}x", factor)).clicked() {
                *action = Some(ContextAction::Stretch(clip_id, factor));
                ui.close_menu();
            }
        }
    });

    ui.menu_button("Pitch Shift", |ui| {
        for &semitones in &[-12.0, -6.0, -3.0, -1.0, 1.0, 3.0, 6.0, 12.0] {
            let label = if semitones > 0.0 {
                format!("+{:.0} st", semitones)
            } else {
                format!("{:.0} st", semitones)
            };
            if ui.button(label).clicked() {
                *action = Some(ContextAction::Pitch(clip_id, semitones));
                ui.close_menu();
            }
        }
    });

    ui.separator();

    if ui.button("Duplicate").clicked() {
        *action = Some(ContextAction::Duplicate(clip_id));
        ui.close_menu();
    }

    if ui.button("Delete").clicked() {
        *action = Some(ContextAction::Delete(clip_id));
        ui.close_menu();
    }

    if ui.button("Clear Effects").clicked() {
        *action = Some(ContextAction::ClearEffects(clip_id));
        ui.close_menu();
    }
}

/// Main entry point: render the full editor UI.
pub fn show_editor(ui: &mut egui::Ui, state: &mut EditorState, ctx: &egui::Context) -> bool {
    let mut close = false;
    let mut context_action: Option<ContextAction> = None;

    // Update cursor from playback engine (only while playing, so user
    // clicks can set cursor position when playback is stopped)
    if state.playback.state.is_playing() {
        state.timeline.cursor_s = state.playback.state.get_cursor();
        ctx.request_repaint();
    }

    // Check for playback errors
    if let Some(err) = state.playback.state.take_error() {
        state.audio_error = Some(err);
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
                state.audio_error = None; // Clear previous error
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

        if let Some(ref err) = state.audio_error {
            ui.colored_label(egui::Color32::RED, err);
            if ui.small_button("x").clicked() {
                state.audio_error = None;
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

    // Timeline in central panel
    let mut reorder: Option<(usize, usize)> = None;
    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let (response, timeline_reorder) = timeline::show_timeline(
                ui,
                &state.arrangement,
                &mut state.timeline,
                &state.source_indices,
            );
            reorder = timeline_reorder;

            // Context menu on right-click
            let menu_clip = state.timeline.context_menu_clip;
            response.context_menu(|ui| {
                if let Some(clip_id) = menu_clip {
                    show_clip_context_menu(ui, clip_id, &mut context_action);
                }
            });
        });
    });

    // Apply reorder from drag-to-reorder
    if let Some((from, to)) = reorder {
        let clip = state.arrangement.timeline.remove(from);
        let insert_at = if to > from { to - 1 } else { to };
        state.arrangement.timeline.insert(insert_at, clip);
        state.arrangement.relayout(0.0);
    }

    // Apply context menu action
    if let Some(action) = context_action {
        apply_context_action(state, action);
    }

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

            ui.horizontal(|ui| {
                // Play/preview button
                if ui.small_button("▶").clicked() {
                    clip_to_play = Some(clip.id);
                }

                // Mini waveform (click to add to timeline)
                let (rect, wf_resp) =
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

                // Label (click to add to timeline)
                let label_resp = ui.vertical(|ui| {
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
                }).response;

                // Click on waveform or label = add to timeline
                if wf_resp.clicked() || label_resp.clicked() {
                    clip_to_add = Some(clip.id);
                }
            });
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
