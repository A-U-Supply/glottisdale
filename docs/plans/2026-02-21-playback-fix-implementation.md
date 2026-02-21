# Fix GUI Playback + Syllable Preview — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the silent GUI playback failure by surfacing swallowed errors, and add per-syllable preview buttons to the bank panel.

**Architecture:** Add error state (`last_error`) to the existing `PlaybackState` shared struct so the playback thread can report failures to the GUI. Surface render errors in `play_from_cursor`. Show errors in the toolbar. Add a play button to each bank entry for syllable preview.

**Tech Stack:** Rust, egui, rodio, Arc<Mutex<>>

---

### Task 1: Add error state to PlaybackState

**Files:**
- Modify: `crates/core/src/editor/playback_engine.rs:27-49`

**Step 1: Write the failing test**

Add to the existing `tests` module in `playback_engine.rs`:

```rust
#[test]
fn test_playback_state_error_handling() {
    let state = PlaybackState::new();
    assert!(state.take_error().is_none());

    state.set_error("test error".into());
    assert_eq!(state.take_error(), Some("test error".to_string()));

    // take_error clears it
    assert!(state.take_error().is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p glottisdale-core test_playback_state_error_handling`
Expected: FAIL — `set_error` and `take_error` methods don't exist.

**Step 3: Write minimal implementation**

Add `last_error` field to `PlaybackState` and implement methods:

```rust
pub struct PlaybackState {
    pub cursor_s: Arc<Mutex<f64>>,
    pub is_playing: Arc<Mutex<bool>>,
    pub last_error: Arc<Mutex<Option<String>>>,
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            cursor_s: Arc::new(Mutex::new(0.0)),
            is_playing: Arc::new(Mutex::new(false)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    // ... existing get_cursor, is_playing ...

    pub fn set_error(&self, msg: String) {
        *self.last_error.lock().unwrap() = Some(msg);
    }

    pub fn take_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().take()
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p glottisdale-core test_playback_state_error_handling`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/core/src/editor/playback_engine.rs
git commit -m "feat(playback): add error state to PlaybackState"
```

---

### Task 2: Surface OutputStream errors in playback thread

**Files:**
- Modify: `crates/core/src/editor/playback_engine.rs:110-186` (the `playback_thread` function)

**Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn test_playback_engine_reports_empty_play() {
    let engine = PlaybackEngine::new();
    // Play empty samples — should not crash, should not set an error
    engine.play_samples(vec![], 16000, 0.0);
    std::thread::sleep(std::time::Duration::from_millis(50));
    // No error for empty samples (it's just a no-op)
    engine.stop();
}
```

This test verifies the engine handles edge cases without panicking. The real error surfacing is in the `playback_thread` function changes below (tested by integration/manual).

**Step 2: Run test to verify it passes (baseline)**

Run: `cargo test -p glottisdale-core test_playback_engine_reports_empty_play`
Expected: PASS (baseline — this should already work)

**Step 3: Modify playback_thread to surface errors**

In the `playback_thread` function, change the `OutputStream::try_default().ok()` to surface errors:

```rust
fn playback_thread(rx: mpsc::Receiver<PlaybackCommand>, state: PlaybackState) {
    let audio = match OutputStream::try_default() {
        Ok(pair) => Some(pair),
        Err(e) => {
            log::error!("Failed to open audio output: {}", e);
            state.set_error(format!("Audio device: {}", e));
            None
        }
    };
    let stream_handle = audio.as_ref().map(|(_, h)| h);
    // ...rest unchanged until PlaySamples handler...
```

In the `PlaySamples` match arm, surface the `Sink::try_new` error and the no-stream-handle case:

```rust
PlaybackCommand::PlaySamples { samples, sample_rate: sr, start_cursor_s } => {
    if let Some(handle) = stream_handle {
        drop(sink.take());
        match Sink::try_new(handle) {
            Ok(new_sink) => {
                let source = crate::audio::playback::make_f64_source(samples, sr);
                new_sink.append(source);
                new_sink.play();
                sink = Some(new_sink);
                play_start = Some((Instant::now(), start_cursor_s));
                *state.is_playing.lock().unwrap() = true;
            }
            Err(e) => {
                log::error!("Failed to create audio sink: {}", e);
                state.set_error(format!("Audio sink: {}", e));
            }
        }
    } else {
        state.set_error("No audio output device available".into());
    }
}
```

**Step 4: Run all playback tests**

Run: `cargo test -p glottisdale-core playback`
Expected: All PASS

**Step 5: Commit**

```bash
git add crates/core/src/editor/playback_engine.rs
git commit -m "feat(playback): surface audio device and sink errors"
```

---

### Task 3: Surface render errors in play_from_cursor

**Files:**
- Modify: `crates/gui/src/editor/mod.rs:147-160` (the `play_from_cursor` method)

**Step 1: No unit test for this** (GUI method, tested manually + by ensuring it compiles)

**Step 2: Modify play_from_cursor to surface errors**

Replace the current `play_from_cursor` method:

```rust
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
```

**Step 3: Verify it compiles**

Run: `cargo check -p glottisdale-gui`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add crates/gui/src/editor/mod.rs
git commit -m "feat(playback): surface render errors in play_from_cursor"
```

---

### Task 4: Show playback errors in the editor toolbar

**Files:**
- Modify: `crates/gui/src/editor/mod.rs:299-391` (the `show_editor` function, toolbar section)

**Step 1: Add error display state to EditorState**

Add an `audio_error` field to `EditorState`:

```rust
pub struct EditorState {
    pub arrangement: Arrangement,
    pub timeline: TimelineState,
    pub playback: PlaybackEngine,
    pub source_indices: HashMap<PathBuf, usize>,
    pub bank_filter: String,
    /// Last audio/playback error to display.
    pub audio_error: Option<String>,
}
```

Initialize it as `None` in `EditorState::new()`.

**Step 2: Poll for errors in show_editor**

At the top of `show_editor`, after the cursor update (line ~304), add:

```rust
// Check for playback errors
if let Some(err) = state.playback.state.take_error() {
    state.audio_error = Some(err);
    ctx.request_repaint();
}
```

**Step 3: Display error in toolbar**

In the toolbar `ui.horizontal(|ui| { ... })` block, before the right-aligned clip count, add:

```rust
if let Some(ref err) = state.audio_error {
    ui.colored_label(egui::Color32::RED, err);
    if ui.small_button("x").clicked() {
        state.audio_error = None;
    }
}
```

**Step 4: Clear error on successful play**

In the toolbar Play button handler, clear the error when play is initiated:

```rust
if ui.button(if playing { "Pause" } else { "Play" }).clicked() {
    if playing {
        state.playback.pause();
    } else {
        state.audio_error = None; // Clear previous error
        state.play_from_cursor();
    }
}
```

**Step 5: Verify it compiles**

Run: `cargo check -p glottisdale-gui`
Expected: Compiles without errors

**Step 6: Commit**

```bash
git add crates/gui/src/editor/mod.rs
git commit -m "feat(editor): display playback errors in toolbar"
```

---

### Task 5: Add syllable preview button to bank panel

**Files:**
- Modify: `crates/gui/src/editor/mod.rs:443-529` (the `show_bank_panel` function)

**Step 1: Restructure the bank entry layout**

Replace the bank entry horizontal layout to add a play button before the waveform. Change the loop body in `show_bank_panel`:

```rust
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
```

**Step 2: Remove the old double_clicked/clicked handling**

The old code had `response.double_clicked()` → play and `response.clicked()` → add. Replace with the new explicit button approach above. The `clip_to_play` and `clip_to_add` variables are already declared before the loop — keep them.

**Step 3: Verify it compiles**

Run: `cargo check -p glottisdale-gui`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add crates/gui/src/editor/mod.rs
git commit -m "feat(editor): add syllable preview button to bank panel"
```

---

### Task 6: Final verification and docs

**Files:**
- Run: Full test suite
- Update: `README.md` if editor usage docs exist

**Step 1: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests pass

**Step 2: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No warnings

**Step 3: Manual test**

Build and run the GUI: `cargo run -p glottisdale-gui`
1. Load audio files and run a pipeline to get to the editor
2. Click Play — should either play audio or show a red error message in the toolbar
3. Click the ▶ button next to a bank entry — should play that syllable's audio
4. Click a bank entry label/waveform — should add to timeline (not play)

**Step 4: Update README if needed**

Check `README.md` for any editor documentation that mentions playback or the bank panel. Update if the new preview button should be documented.

**Step 5: Final commit if any doc changes**

```bash
git add README.md  # if changed
git commit -m "docs: update editor playback and preview docs"
```
