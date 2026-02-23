# Editor Settings Integration Design

**Date:** 2026-02-22
**Status:** Implemented

## Problem

The right-pane settings (crossfade, volume normalization, pitch normalization, prosodic dynamics, pink noise, room tone, breaths, global speed) have no effect in editor mode. Playback and export ignore them entirely, producing raw overlap-add output with no polish.

## Solution

Enrich `render_arrangement()` to accept a `RenderSettings` struct and apply the full audio polish stack inline — crossfade during the overlap-add loop, global polish passes after.

## RenderSettings Struct

New struct in `crates/core/src/editor/render.rs`:

```rust
pub struct RenderSettings {
    // Crossfade & spacing
    pub crossfade_ms: f64,
    // Audio polish
    pub volume_normalize: bool,
    pub pitch_normalize: bool,
    pub pitch_range: f64,
    pub prosodic_dynamics: bool,
    pub noise_level_db: f64,
    pub room_tone: bool,
    pub breaths: bool,
    pub breath_probability: f64,
    // Global speed
    pub speed: Option<f64>,
    // Seed
    pub seed: Option<u64>,
}
```

`Default` impl matches current CollageConfig defaults.

## Render Changes

`render_arrangement(arrangement, settings)`:

1. **Overlap-add with crossfade**: When placing each clip, if it overlaps with the previous clip (per `crossfade_ms`), apply half-sine crossfade ramp in the overlap region instead of additive mixing. `relayout()` gains a `crossfade_ms` parameter to compute positions with negative overlap.
2. **Post-processing passes** (in order):
   - Volume normalization (peak normalize to -1dB)
   - Pitch normalization (flatten contour within `pitch_range` semitones)
   - Prosodic dynamics (`apply_prosodic_dynamics` from collage pipeline)
   - Room tone (mix pre-extracted room tone clips into silent gaps)
   - Breaths (insert pre-extracted breath clips at boundaries with probability)
   - Pink noise bed (generate and mix at `noise_level_db`)
   - Global speed (time-stretch entire output)

All functions already exist in `audio::analysis` and `audio::effects`.

## Arrangement Changes

Add pre-extracted audio context:

```rust
pub struct Arrangement {
    // ... existing fields ...
    pub room_tone_clips: Vec<Vec<f64>>,  // quiet regions from source files
    pub breath_clips: Vec<Vec<f64>>,     // detected breaths from source files
}
```

Populated during `build_bank_from_syllables()` which has access to full source audio.

## GUI Wiring

- `EditorState::play_from_cursor(&self, settings: &RenderSettings)` — builds render settings from current app state
- Export button: same path, `render_arrangement(&arr, &settings)` then write to file
- Settings read fresh on each play/export — immediate, no "apply" step
- Bank clip preview unaffected (plays raw clip audio)

## What Stays the Same

- Per-clip effects (stutter, stretch, pitch, reverse) work as before
- Bank preview plays raw clips
- Timeline layout unaffected (relayout just gains crossfade awareness)
- Collage/sing/speak pipelines unchanged
