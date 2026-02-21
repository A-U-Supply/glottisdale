//! Timeline widget — custom egui painting with zoom/pan and clip layout.

use eframe::egui;
use glottisdale_core::editor::{Arrangement, ClipId};

use super::waveform_painter::paint_clip_block;

/// Colors for clips from different source files.
pub const SOURCE_COLORS: &[(u8, u8, u8)] = &[
    (70, 130, 180),  // steel blue
    (180, 100, 60),  // terracotta
    (80, 160, 80),   // green
    (160, 80, 160),  // purple
    (180, 160, 50),  // gold
    (80, 160, 160),  // teal
];

/// Drag-to-reorder state.
pub struct DragState {
    pub clip_index: usize,
    pub clip_id: ClipId,
    /// Index to insert before (None = not yet determined).
    pub insert_before: Option<usize>,
}

/// Visual and interaction state for the timeline.
pub struct TimelineState {
    /// Pixels per second (zoom level).
    pub pixels_per_second: f64,
    /// Scroll offset in seconds (left edge of visible area).
    pub scroll_offset_s: f64,
    /// Track height in pixels.
    pub track_height: f32,
    /// Playback cursor position in seconds.
    pub cursor_s: f64,
    /// Selected clip IDs.
    pub selected: Vec<ClipId>,
    /// Clip ID for right-click context menu.
    pub context_menu_clip: Option<ClipId>,
    /// Active drag-to-reorder state.
    pub drag: Option<DragState>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            pixels_per_second: 200.0,
            scroll_offset_s: 0.0,
            track_height: 80.0,
            cursor_s: 0.0,
            selected: Vec::new(),
            context_menu_clip: None,
            drag: None,
        }
    }
}

impl TimelineState {
    /// Convert time to pixel x coordinate.
    pub fn time_to_px(&self, time_s: f64) -> f32 {
        ((time_s - self.scroll_offset_s) * self.pixels_per_second) as f32
    }

    /// Convert pixel x to time.
    pub fn px_to_time(&self, px: f32) -> f64 {
        px as f64 / self.pixels_per_second + self.scroll_offset_s
    }

    /// Check if a clip ID is selected.
    pub fn is_selected(&self, id: ClipId) -> bool {
        self.selected.contains(&id)
    }

    /// Handle zoom (ctrl+scroll).
    pub fn handle_zoom(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if response.hovered() && ui.input(|i| i.modifiers.command) {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > 0.0 {
                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let time_at_mouse = self.px_to_time(mouse_pos.x - response.rect.left());
                    let zoom_factor = 1.0 + scroll_y as f64 * 0.003;
                    self.pixels_per_second =
                        (self.pixels_per_second * zoom_factor).clamp(10.0, 5000.0);
                    // Keep time_at_mouse at the same pixel position
                    let new_px = mouse_pos.x - response.rect.left();
                    self.scroll_offset_s =
                        time_at_mouse - new_px as f64 / self.pixels_per_second;
                }
            }
        }
    }

    /// Handle pan (scroll without modifier).
    pub fn handle_pan(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if response.hovered() && !ui.input(|i| i.modifiers.command) {
            let scroll_x = ui.input(|i| i.smooth_scroll_delta.x);
            if scroll_x.abs() > 0.0 {
                self.scroll_offset_s -= scroll_x as f64 / self.pixels_per_second;
                self.scroll_offset_s = self.scroll_offset_s.max(0.0);
            }
        }
    }
}

/// Get a color for a source file index.
fn source_color(index: usize) -> egui::Color32 {
    let (r, g, b) = SOURCE_COLORS[index % SOURCE_COLORS.len()];
    egui::Color32::from_rgb(r, g, b)
}

/// Find which clip index is at a given time, if any.
fn clip_at_time(arrangement: &Arrangement, time_s: f64) -> Option<(usize, ClipId)> {
    for (i, tc) in arrangement.timeline.iter().enumerate() {
        let clip_end = tc.position_s + tc.effective_duration_s;
        if time_s >= tc.position_s && time_s <= clip_end {
            return Some((i, tc.id));
        }
    }
    None
}

/// Paint the timeline with all clips. Returns (response, optional reorder).
pub fn show_timeline(
    ui: &mut egui::Ui,
    arrangement: &Arrangement,
    state: &mut TimelineState,
    source_file_indices: &std::collections::HashMap<std::path::PathBuf, usize>,
) -> (egui::Response, Option<(usize, usize)>) {
    let desired_size = egui::vec2(ui.available_width(), state.track_height + 20.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

    if !ui.is_rect_visible(rect) {
        return (response, None);
    }

    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(30));

    // Track area
    let track_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + 16.0),
        egui::vec2(rect.width(), state.track_height),
    );

    // Time ruler at top
    paint_time_ruler(
        &painter,
        egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 16.0)),
        state,
    );

    // Paint clips
    let dragging_id = state.drag.as_ref().map(|d| d.clip_id);
    for tc in &arrangement.timeline {
        let clip_left = state.time_to_px(tc.position_s) + rect.left();
        let clip_width = (tc.effective_duration_s * state.pixels_per_second) as f32;
        let clip_right = clip_left + clip_width;

        // Skip if not visible
        if clip_right < rect.left() || clip_left > rect.right() {
            continue;
        }

        let clip_rect = egui::Rect::from_min_size(
            egui::pos2(clip_left, track_rect.top()),
            egui::vec2(clip_width, state.track_height),
        );

        if let Some(bank_clip) = arrangement.get_bank_clip(tc.source_clip_id) {
            let src_idx = source_file_indices
                .get(&bank_clip.source_path)
                .copied()
                .unwrap_or(0);
            let is_ghost = dragging_id == Some(tc.id);
            let alpha = if is_ghost { 0.15 } else { 0.3 };
            let bg = source_color(src_idx).gamma_multiply(alpha);
            let wf_color = if is_ghost {
                source_color(src_idx).gamma_multiply(0.4)
            } else {
                source_color(src_idx)
            };

            paint_clip_block(
                &painter,
                clip_rect,
                &bank_clip.waveform,
                &bank_clip.label,
                bg,
                wf_color,
                state.is_selected(tc.id) && !is_ghost,
            );
        }
    }

    // Paint drag insertion indicator
    if let Some(ref drag) = state.drag {
        if let Some(insert_idx) = drag.insert_before {
            let insert_x = if insert_idx < arrangement.timeline.len() {
                state.time_to_px(arrangement.timeline[insert_idx].position_s) + rect.left()
            } else if let Some(last) = arrangement.timeline.last() {
                state.time_to_px(last.position_s + last.effective_duration_s) + rect.left()
            } else {
                rect.left()
            };

            painter.line_segment(
                [
                    egui::pos2(insert_x, track_rect.top()),
                    egui::pos2(insert_x, track_rect.bottom()),
                ],
                egui::Stroke::new(3.0, egui::Color32::from_rgb(100, 180, 255)),
            );
        }
    }

    // Playback cursor
    let cursor_x = state.time_to_px(state.cursor_s) + rect.left();
    if cursor_x >= rect.left() && cursor_x <= rect.right() {
        painter.line_segment(
            [
                egui::pos2(cursor_x, rect.top()),
                egui::pos2(cursor_x, rect.bottom()),
            ],
            egui::Stroke::new(2.0, egui::Color32::RED),
        );
    }

    // Handle zoom and pan
    state.handle_zoom(ui, &response);
    state.handle_pan(ui, &response);

    let mut reorder: Option<(usize, usize)> = None;

    // Handle drag-to-reorder
    if response.drag_started() {
        if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
            let click_time = state.px_to_time(origin.x - rect.left());
            if let Some((idx, id)) = clip_at_time(arrangement, click_time) {
                state.drag = Some(DragState {
                    clip_index: idx,
                    clip_id: id,
                    insert_before: None,
                });
                // Select the dragged clip
                if !state.selected.contains(&id) {
                    state.selected = vec![id];
                }
            }
        }
    }

    if response.dragged() {
        if let Some(ref mut drag) = state.drag {
            if let Some(pos) = response.interact_pointer_pos() {
                let px = pos.x - rect.left();
                let drag_time = px as f64 / state.pixels_per_second + state.scroll_offset_s;
                // Find insertion position based on clip midpoints
                let mut insert = arrangement.timeline.len();
                for (i, tc) in arrangement.timeline.iter().enumerate() {
                    if i == drag.clip_index {
                        continue;
                    }
                    let mid = tc.position_s + tc.effective_duration_s / 2.0;
                    if drag_time < mid {
                        insert = i;
                        break;
                    }
                }
                drag.insert_before = Some(insert);
            }
        }
    }

    if response.drag_stopped() {
        if let Some(drag) = state.drag.take() {
            if let Some(insert) = drag.insert_before {
                // Only reorder if actually moved to a different position
                if insert != drag.clip_index && insert != drag.clip_index + 1 {
                    reorder = Some((drag.clip_index, insert));
                }
            }
        }
    }

    // Handle right-click for context menu
    if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let click_time = state.px_to_time(pos.x - rect.left());
            if let Some((_, id)) = clip_at_time(arrangement, click_time) {
                state.context_menu_clip = Some(id);
                if !state.selected.contains(&id) {
                    state.selected = vec![id];
                }
            } else {
                state.context_menu_clip = None;
            }
        }
    }

    // Handle click to select/set cursor (only if not dragging)
    if response.clicked() && state.drag.is_none() {
        if let Some(pos) = response.interact_pointer_pos() {
            let click_time = state.px_to_time(pos.x - rect.left());

            if let Some((_, clip_id)) = clip_at_time(arrangement, click_time) {
                let shift = ui.input(|i| i.modifiers.shift || i.modifiers.command);
                if shift {
                    if let Some(idx) = state.selected.iter().position(|&id| id == clip_id) {
                        state.selected.remove(idx);
                    } else {
                        state.selected.push(clip_id);
                    }
                } else {
                    state.selected = vec![clip_id];
                }
            } else {
                state.cursor_s = click_time.max(0.0);
                state.selected.clear();
            }
        }
    }

    (response, reorder)
}

/// Paint time markers along the top of the timeline.
fn paint_time_ruler(painter: &egui::Painter, rect: egui::Rect, state: &TimelineState) {
    let font = egui::FontId::proportional(9.0);
    let color = egui::Color32::from_gray(150);

    // Determine tick interval based on zoom
    let tick_interval = if state.pixels_per_second > 500.0 {
        0.1
    } else if state.pixels_per_second > 100.0 {
        0.5
    } else if state.pixels_per_second > 20.0 {
        1.0
    } else {
        5.0
    };

    let start_time = (state.scroll_offset_s / tick_interval).floor() * tick_interval;
    let end_time = state.px_to_time(rect.width());

    let mut t = start_time;
    while t <= end_time {
        let x = state.time_to_px(t) + rect.left();
        if x >= rect.left() && x <= rect.right() {
            // Tick line
            painter.line_segment(
                [
                    egui::pos2(x, rect.bottom() - 4.0),
                    egui::pos2(x, rect.bottom()),
                ],
                egui::Stroke::new(1.0, color),
            );
            // Label
            let label = if tick_interval >= 1.0 {
                format!("{:.0}s", t)
            } else {
                format!("{:.1}s", t)
            };
            let galley = painter.layout_no_wrap(label, font.clone(), color);
            painter.galley(egui::pos2(x + 2.0, rect.top()), galley, color);
        }
        t += tick_interval;
    }
}
