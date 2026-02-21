# Interactive Syllable Editor — Design Document

## Goal

Add a DAW-like interactive syllable editor to the glottisdale GUI that lets users view syllables with waveforms, manually arrange them on a timeline, apply effects (stutter, stretch, pitch shift), and export the result. Works with all three pipelines (collage, sing, speak) in both post-processing mode (tweak auto-generated arrangements) and blank canvas mode (build from scratch).

## Architecture Overview

The editor is a **sub-view** that overlays the central panel of the existing GUI — not a 4th pipeline mode. Any pipeline can open it. Two entry points:

- **"Build Bank" button** — runs alignment only, opens editor with empty timeline and all syllables in a palette (blank canvas mode)
- **"Edit" button** (after pipeline finishes) — opens editor with auto-generated arrangement pre-loaded

### Core Concept: Source Material vs. Arrangement

- **Bank** (`Vec<SyllableClip>`) — immutable source material. Each clip has raw audio samples, pre-computed waveform data, and syllable metadata.
- **Timeline** (`Vec<TimelineClip>`) — positioned references to bank clips with non-destructive effects. The same syllable can appear multiple times. Effects are metadata, not baked into audio — fully undoable.

### Module Layout

```
crates/core/src/editor/
  mod.rs              -- module declaration, re-exports
  types.rs            -- SyllableClip, TimelineClip, Arrangement, ClipEffect, WaveformData
  waveform.rs         -- WaveformData::from_samples, peak computation
  effects_chain.rs    -- apply_effects(), compute_effective_duration()
  render.rs           -- render_arrangement(), export_arrangement()
  pipeline_bridge.rs  -- arrangement_from_collage/sing/speak, build_bank_from_syllables
  playback_engine.rs  -- PlaybackEngine, PlaybackCommand, PlaybackState
  bank_builder.rs     -- build_editor_bank() for blank canvas mode

crates/gui/src/editor/
  mod.rs              -- EditorState, show_editor() entry point
  timeline.rs         -- TimelineState, timeline widget painting, zoom/pan, coordinates
  waveform_painter.rs -- paint_waveform(), paint_clip_block()
  bank_panel.rs       -- show_bank(), bank search/filter
  toolbar.rs          -- show_toolbar(), batch operation handlers
  interaction.rs      -- click, drag, select, DragState, Selection
  context_menu.rs     -- show_context_menu(), effect parameter pickers
```

### New Dependencies

- `uuid` v1 (features: v4, serde) — unique clip IDs
- No other new dependencies needed

---

## Data Model

### Core Editor Types (`crates/core/src/editor/types.rs`)

```rust
pub type ClipId = uuid::Uuid;

/// A single syllable's audio data, ready for editing.
/// Atomic unit in the editor.
pub struct SyllableClip {
    pub id: ClipId,
    pub syllable: Syllable,          // source metadata (phonemes, word, timing)
    pub samples: Vec<f64>,           // raw audio (mono)
    pub sample_rate: u32,
    pub source_path: PathBuf,
    pub waveform: WaveformData,      // pre-computed peak pairs
    pub label: String,               // display label, e.g. "K AE1 T"
}

/// Pre-computed waveform data for efficient rendering.
pub struct WaveformData {
    pub peaks: Vec<(f32, f32)>,      // (min_peak, max_peak) per bucket
    pub samples_per_bucket: usize,   // 256 by default
}

/// Non-destructive effect applied to a timeline clip.
pub enum ClipEffect {
    Stutter { count: usize },
    TimeStretch { factor: f64 },
    PitchShift { semitones: f64 },
}

/// A clip placed on the timeline.
pub struct TimelineClip {
    pub id: ClipId,
    pub source_clip_id: ClipId,      // references a SyllableClip in the bank
    pub position_s: f64,             // left edge position in seconds
    pub effects: Vec<ClipEffect>,
    pub effective_duration_s: f64,   // recomputed when effects change
}

/// Full state of a syllable arrangement.
pub struct Arrangement {
    pub bank: Vec<SyllableClip>,
    pub timeline: Vec<TimelineClip>,
    pub crossfade_ms: f64,
    pub sample_rate: u32,
    pub source_pipeline: PipelineMode,
}
```

### Key Design Decisions

- `SyllableClip` (bank) is immutable source material. `TimelineClip` is a positioned reference with effects. This separation means the same syllable can appear many times, effects are trivially undoable, and the bank serves as both palette and data store.
- Effects are non-destructive metadata on `TimelineClip`, applied lazily during playback and export.
- `WaveformData` is pre-computed once per clip — no raw sample processing during rendering.

---

## Waveform Rendering

### Pre-computation

When a `SyllableClip` is created, `WaveformData::from_samples()` computes min/max peak pairs at 256-sample buckets. At sr=16000:
- A 0.3s syllable (4800 samples) = ~19 peak pairs = ~150 bytes
- A bank of 200 syllables = ~30KB of waveform data (negligible)

### Drawing

The egui `Painter` draws vertical lines from min to max peak per pixel column within the clip's rectangle. When zoomed out (multiple buckets per pixel), composite the min/max across overlapping buckets. When zoomed in past bucket resolution, fall back to drawing actual samples.

```rust
fn paint_waveform(
    painter: &egui::Painter,
    rect: egui::Rect,
    waveform: &WaveformData,
    color: egui::Color32,
)
```

### Performance

At 60fps with 50 timeline clips of ~20 peaks each = ~1000 line segments per frame. Trivial for egui/epaint. No raw samples touched during painting.

---

## Timeline Widget

### Coordinate System

Two spaces:
- **Time** (seconds) — authoritative. `TimelineClip.position_s` lives here.
- **Pixels** — derived via zoom/pan:
  ```
  pixel_x = (time_s - scroll_offset_s) * pixels_per_second
  time_s  = pixel_x / pixels_per_second + scroll_offset_s
  ```

### Zoom/Pan

- `pixels_per_second` range: 10.0 (overview) to 5000.0 (sample-level)
- **Zoom**: Ctrl+scroll, zooms around mouse cursor (keeps time under cursor fixed)
- **Pan**: Scroll wheel horizontal, or middle-click drag
- **Minimap**: Optional thin bar at top showing full arrangement with viewport rectangle

### Layout

Single horizontal track. Clips are positioned sequentially by default. `relayout()` recomputes positions after any mutation (drag, insert, delete):

```rust
fn relayout(arrangement: &mut Arrangement, gap_s: f64) {
    let mut cursor = 0.0;
    for clip in &mut arrangement.timeline {
        clip.position_s = cursor;
        cursor += clip.effective_duration_s + gap_s;
    }
}
```

### Timeline State

```rust
pub struct TimelineState {
    pub pixels_per_second: f64,
    pub scroll_offset_s: f64,
    pub track_height: f32,
    pub cursor_s: f64,
    pub selection: Selection,
    pub drag: DragState,
    pub context_menu: Option<ContextMenuState>,
}
```

### Clip Appearance

Each clip block shows:
- Filled waveform shape (colored by source file for visual distinction)
- Phoneme label above (e.g. "HH AH0 L OW1")
- Effect indicators (small icons when stutter/stretch/pitch applied)
- Selection highlight (border glow when selected)

### Playback Cursor

Red vertical line, position read from `PlaybackEngine.state.cursor_s` each frame. Continuous repaint requested during playback.

---

## Interaction Model

### State Machine

```
IDLE
  +--[left click on clip]-------> SELECT (single)
  |     +--[shift/cmd+click]---> SELECT (toggle multi)
  +--[left click on empty]------> DESELECT + set cursor
  +--[left drag on clip]--------> DRAG_REORDER (after 3px threshold)
  +--[left drag on empty]-------> RANGE_SELECT (rubber-band)
  +--[right click on clip]------> CONTEXT_MENU
  +--[double-click clip]--------> PLAY_CLIP (preview)
  +--[spacebar]-----------------> PLAY_FROM_CURSOR
  +--[ctrl+scroll]--------------> ZOOM
  +--[scroll]-------------------> PAN
  +--[ctrl+a]-------------------> SELECT_ALL
```

### Drag and Drop

**Reorder on timeline**: Click-drag a clip (or multi-selection). Dragged clips become semi-transparent ghosts at original positions. Solid preview shows insertion point. On release, clips removed from old positions, inserted at new position, `relayout()` called.

**From bank to timeline**: Click-drag a bank entry onto the timeline. Creates a new `TimelineClip` referencing that `SyllableClip`. Same insertion logic.

### Right-Click Context Menu

| Action | Details |
|--------|---------|
| Stutter | Count picker (1-8), adds `ClipEffect::Stutter` |
| Time Stretch | Factor picker (0.5-4.0), adds `ClipEffect::TimeStretch` |
| Pitch Shift | Semitone slider (-12 to +12), adds `ClipEffect::PitchShift` |
| Duplicate | Inserts copy immediately after |
| Delete | Removes from timeline |
| Clear Effects | Removes all effects from this clip |

### Toolbar (Batch Operations)

| Button | Action |
|--------|--------|
| Shuffle | Randomize order of selected clips |
| Stutter All | Apply stutter to all selected (count picker) |
| Stretch All | Apply time stretch to all selected (factor picker) |
| Delete | Remove selected from timeline |
| Play Selection | Play only selected clips |
| Clear Effects | Strip effects from selected |
| Export | Render and save final WAV |

Play/Pause/Stop buttons and zoom +/- buttons also in toolbar.

---

## Syllable Bank / Palette

Left side panel showing all available `SyllableClip`s. Each entry displays:
- Mini waveform thumbnail (40x24 px)
- Phoneme label (e.g. "HH AH0 L OW1")
- Source word (e.g. from "hello")
- Duration (e.g. "0.32s")

### Features

- **Search/filter**: Text field at top, filters by phoneme or word text
- **Color coding**: Syllables from different source files get different colors, carried through to timeline blocks
- **Drag to timeline**: Click-drag any entry onto the timeline to add it; same syllable can be added multiple times

### Population

Built from the alignment step all pipelines already perform. For blank canvas mode, `build_editor_bank()` runs just alignment (extract, transcribe, syllabify, cut, compute waveforms) without running the rest of the pipeline.

---

## Effects Application

### Non-Destructive Pipeline

Effects stored as metadata on `TimelineClip.effects`, applied lazily in two contexts:
1. **Playback**: processed in real-time as cursor traverses the clip
2. **Export**: applied to produce final WAV

Effects are NOT baked into `SyllableClip.samples`. Undoing = removing from the vec.

### Effect Processing

```rust
pub fn apply_effects(
    source_samples: &[f64],
    sr: u32,
    effects: &[ClipEffect],
) -> Result<Vec<f64>>
```

Stutter repeats the clip with micro-crossfade. TimeStretch and PitchShift delegate to existing `time_stretch()` and `pitch_shift()` in `audio::effects`.

### Duration Tracking

```rust
pub fn compute_effective_duration(base_duration_s: f64, effects: &[ClipEffect]) -> f64
```

Called whenever effects change. Updates `effective_duration_s`, then `relayout()` repositions subsequent clips.

---

## Playback Integration

### Problem

Current `play_samples()` calls `sink.sleep_until_end()` — blocks the thread. Incompatible with interactive editing.

### Solution: PlaybackEngine

Dedicated thread owns the rodio `OutputStream` and `Sink`. Controlled via command channel:

```rust
pub enum PlaybackCommand {
    PlayFrom(f64),              // play arrangement from cursor
    PlaySelection(Vec<ClipId>), // play selected clips only
    PlayClip(ClipId),           // preview single syllable
    Pause,
    Resume,
    Stop,
}
```

Shared state for cursor tracking:
```rust
pub struct PlaybackState {
    pub cursor_s: Arc<Mutex<f64>>,
    pub is_playing: Arc<Mutex<bool>>,
}
```

The engine pre-renders the relevant portion of the arrangement into a buffer, creates an `F64Source`, and plays it. Cursor position updated based on elapsed time.

---

## Export / Render

### Render Pipeline

```rust
pub fn render_arrangement(arrangement: &Arrangement) -> Result<Vec<f64>>
```

Uses overlap-add: each clip's audio (with effects applied) placed at its timeline position into an output buffer. Handles arbitrary placements including overlapping stuttered clips.

### Export UI

"Export" button in toolbar opens `rfd::FileDialog` save dialog. Rendering on background thread with status bar progress. Writes 16-bit PCM WAV via existing `write_wav()`.

---

## Pipeline Integration

### Split Pipeline Execution

Each pipeline's run function splits into two phases:
1. **Align + process** — produces intermediate data (bank + arrangement)
2. **Render** — writes final WAV

After phase 1, the GUI converts pipeline output to an `Arrangement` via bridge functions and opens the editor. The user tweaks, then exports.

### Bridge Functions

```rust
pub fn arrangement_from_collage(clips, source_audio, all_syllables) -> Result<Arrangement>
pub fn arrangement_from_sing(vocal_syllables, note_mappings, source_audio) -> Result<Arrangement>
pub fn arrangement_from_speak(matches, timing, bank_entries, source_audio) -> Result<Arrangement>
```

### Existing Flow Preserved

If you don't open the editor, the pipeline renders immediately as before. The editor is opt-in.

### GUI Integration

```rust
pub struct GlottisdaleApp {
    // ... existing fields ...
    editor: Option<EditorState>,
}
```

Central panel routes to `show_editor()` when `editor` is `Some`, otherwise normal workspace. "Close Editor" button returns to the workspace.

---

## Testing Strategy

### Unit Tests (`crates/core/src/editor/`)

| Module | What to test |
|--------|-------------|
| `waveform.rs` | Peak computation with sine, silence, impulse signals. Verify bucket counts and min/max values. |
| `effects_chain.rs` | Single and stacked effects. Output lengths (stutter 2x = 3x, stretch 2.0 = 2x). `compute_effective_duration()`. |
| `render.rs` | Render 2-clip arrangement, verify output length and overlap-add. |
| `types.rs` | `relayout()` sequential positions. Effective duration after effect changes. |
| `pipeline_bridge.rs` | Round-trip: minimal pipeline output to Arrangement, verify bank/timeline sizes. |

### Integration Tests

- Full pipeline -> editor -> export round-trip with synthetic test WAVs
- Effects chain fidelity: stutter(2) + stretch(2.0) = ~6x original duration

### Manual Testing

- Waveform visual appearance
- Drag-and-drop feel, zoom/pan smoothness
- Playback cursor sync
- Context menu UX
- Layout at different window sizes

### Test Fixtures

Synthetic `Syllable` structs and short generated WAVs (in-test via `write_wav`). No real alignment needed for unit tests.
