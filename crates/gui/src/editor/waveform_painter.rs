//! Custom egui painting for waveform thumbnails.

use eframe::egui;
use glottisdale_core::editor::WaveformData;

/// Paint a waveform inside a rectangle.
///
/// Draws vertical lines from min_peak to max_peak per pixel column.
pub fn paint_waveform(
    painter: &egui::Painter,
    rect: egui::Rect,
    waveform: &WaveformData,
    color: egui::Color32,
) {
    let n_buckets = waveform.peaks.len();
    if n_buckets == 0 || rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }

    let mid_y = rect.center().y;
    let half_height = rect.height() * 0.45;
    let px_per_bucket = rect.width() / n_buckets as f32;

    if px_per_bucket >= 1.0 {
        // One or more pixels per bucket: draw each bucket
        for (i, &(min_peak, max_peak)) in waveform.peaks.iter().enumerate() {
            let x = rect.left() + (i as f32 + 0.5) * px_per_bucket;
            let y_top = mid_y - max_peak * half_height;
            let y_bot = mid_y - min_peak * half_height;
            painter.line_segment(
                [egui::pos2(x, y_top), egui::pos2(x, y_bot)],
                egui::Stroke::new(px_per_bucket.max(1.0), color),
            );
        }
    } else {
        // Multiple buckets per pixel: composite min/max
        let n_pixels = rect.width() as usize;
        for px in 0..n_pixels {
            let bucket_start = (px as f32 / rect.width() * n_buckets as f32) as usize;
            let bucket_end = ((px + 1) as f32 / rect.width() * n_buckets as f32).ceil() as usize;
            let bucket_end = bucket_end.min(n_buckets);

            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for i in bucket_start..bucket_end {
                let (lo, hi) = waveform.peaks[i];
                if lo < min {
                    min = lo;
                }
                if hi > max {
                    max = hi;
                }
            }

            if min <= max {
                let x = rect.left() + px as f32 + 0.5;
                let y_top = mid_y - max * half_height;
                let y_bot = mid_y - min * half_height;
                painter.line_segment(
                    [egui::pos2(x, y_top), egui::pos2(x, y_bot)],
                    egui::Stroke::new(1.0, color),
                );
            }
        }
    }
}

/// Paint a clip block on the timeline.
///
/// Draws a rounded rectangle background with a waveform inside
/// and a label above.
pub fn paint_clip_block(
    painter: &egui::Painter,
    rect: egui::Rect,
    waveform: &WaveformData,
    label: &str,
    bg_color: egui::Color32,
    waveform_color: egui::Color32,
    selected: bool,
) {
    // Background
    let rounding = egui::CornerRadius::same(3);
    painter.rect_filled(rect, rounding, bg_color);

    // Selection border
    if selected {
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 255)),
            egui::StrokeKind::Outside,
        );
    }

    // Waveform (inside the block, with padding)
    let waveform_rect = rect.shrink2(egui::vec2(2.0, 10.0));
    if waveform_rect.width() > 2.0 && waveform_rect.height() > 2.0 {
        paint_waveform(painter, waveform_rect, waveform, waveform_color);
    }

    // Label at top
    let label_pos = egui::pos2(rect.left() + 3.0, rect.top() + 1.0);
    let font = egui::FontId::proportional(9.0);
    let galley = painter.layout_no_wrap(label.to_string(), font, egui::Color32::WHITE);
    painter.galley(label_pos, galley, egui::Color32::WHITE);
}
