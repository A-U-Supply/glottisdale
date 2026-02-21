# Design: Fix GUI Playback + Syllable Preview

**Date:** 2026-02-21
**Status:** Approved
**Approach:** A — Fix error handling, add bank preview buttons

## Problem

1. **Timeline Play produces no sound.** Clicking Play in the editor toolbar results in silence with no error feedback. Root cause: errors are silently swallowed in two places.
2. **No way to preview syllables** before adding them to the timeline. The bank panel's double-click preview is unreachable because single-click fires first and adds to timeline.

## Root Cause Analysis

### Silent error in PlaybackEngine

`playback_engine.rs:113` — `OutputStream::try_default().ok()` silently drops audio device errors. If the output stream can't be opened (permissions, device unavailable), `stream_handle` is `None` and every `PlaySamples` command is silently ignored.

### Silent error in play_from_cursor

`editor/mod.rs:149` — `if let Ok(samples) = render_arrangement(...)` silently drops render errors. Effects processing (`time_stretch`, `pitch_shift` via ssstretch) can fail on short clips, causing the entire render to fail silently.

### No visual feedback

The GUI shows no indication that playback failed. The cursor only animates when `is_playing` is `true`, which never gets set if audio init or rendering fails.

## Design

### 1. Surface PlaybackEngine errors

- Add `last_error: Arc<Mutex<Option<String>>>` to `PlaybackState`
- When `OutputStream::try_default()` fails, store the error string
- When `Sink::try_new()` fails, store the error string
- Add `set_error()` / `take_error()` methods to `PlaybackState`
- Clear error on successful playback start

### 2. Surface render errors

- Change `play_from_cursor()` to match on the render result
- On `Err(e)`: log with `log::error!` and push to `PlaybackState.last_error`
- On `Ok(empty)`: log "Nothing to play"
- On `Ok(samples)`: proceed with playback as before

### 3. Show errors in GUI

- In `show_editor` toolbar, check `playback.state.take_error()`
- Display as a red label in the toolbar area (e.g., "Audio: Failed to open output device")
- Request repaint on error state change so it shows immediately

### 4. Syllable preview in bank panel

- Add a small `▶` play button to each bank entry row
- Clicking the play button calls `play_clip(id)` to preview the syllable
- Single-click on the rest of the row = add to timeline (current behavior preserved)
- Layout: `[ ▶ ] [ ~waveform~ ] [ label + metadata ]`

### 5. Repaint scheduling

- Ensure `ctx.request_repaint()` fires during state transitions (not just while playing)
- This ensures error messages and cursor updates display immediately

## Files to modify

- `crates/core/src/editor/playback_engine.rs` — add error state to `PlaybackState`, surface `OutputStream` errors
- `crates/gui/src/editor/mod.rs` — surface render errors in `play_from_cursor`, show errors in toolbar, add preview button to bank panel
- Tests for `PlaybackState` error handling

## Future work

**PlaybackEngine overhaul** — The current architecture (poll-based thread, recreate Sink per play, wall-clock cursor tracking) works but is fragile. A future overhaul should:
- Create `OutputStream` eagerly and return `Result` from `PlaybackEngine::new()`
- Keep the audio output alive for the whole editor session
- Consider sample-accurate cursor tracking instead of wall-clock estimation
- Evaluate switching to `kira` for better macOS support and mixer capabilities
