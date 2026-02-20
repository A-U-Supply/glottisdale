//! Synthesize MIDI notes to WAV preview using sine waves.

use std::path::Path;

use anyhow::Result;

use crate::audio::io::write_wav;
use crate::sing::midi_parser::{midi_to_hz, MidiTrack, Note};

const SAMPLE_RATE: u32 = 22050;
const MAX_DURATION: f64 = 30.0;

/// Synthesize a single note to audio samples using a sine wave with envelope.
fn synthesize_note(note: &Note, sr: u32) -> Vec<f64> {
    let freq = midi_to_hz(note.pitch);
    let duration = note.duration();
    let num_samples = (duration * sr as f64).round() as usize;
    let velocity = note.velocity as f64 / 127.0;

    let attack_samples = (0.01 * sr as f64) as usize;
    let release_samples = (0.05 * sr as f64).min(num_samples as f64 * 0.3) as usize;

    (0..num_samples)
        .map(|i| {
            let t = i as f64 / sr as f64;
            let sample = (2.0 * std::f64::consts::PI * freq * t).sin();

            // ADSR envelope
            let env = if i < attack_samples {
                i as f64 / attack_samples as f64
            } else if i >= num_samples - release_samples {
                (num_samples - i) as f64 / release_samples as f64
            } else {
                1.0
            };

            sample * env * velocity
        })
        .collect()
}

/// Synthesize a drum hit (noise burst with envelope).
fn synthesize_drum(pitch: u8, velocity: u8, sr: u32) -> Vec<f64> {
    let vel = velocity as f64 / 127.0;
    let mut rng_state = pitch as u64 * 12345;

    // Simple LCG for deterministic noise
    let mut next_noise = || -> f64 {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (rng_state as f64 / u64::MAX as f64) * 2.0 - 1.0
    };

    match pitch {
        35 | 36 => {
            // Kick
            let length = (0.08 * sr as f64) as usize;
            (0..length)
                .map(|i| {
                    let t = i as f64 / sr as f64;
                    (2.0 * std::f64::consts::PI * 80.0 * t * (-t * 30.0).exp()).sin()
                        * (-t * 25.0).exp()
                        * vel
                })
                .collect()
        }
        38 | 40 => {
            // Snare
            let length = (0.1 * sr as f64) as usize;
            (0..length)
                .map(|i| {
                    let t = i as f64 / sr as f64;
                    (next_noise() * 0.7
                        + (2.0 * std::f64::consts::PI * 180.0 * t).sin() * 0.3)
                        * (-t * 20.0).exp()
                        * vel
                })
                .collect()
        }
        42 | 44 | 46 => {
            // Hihat
            let length = (0.05 * sr as f64) as usize;
            (0..length)
                .map(|i| {
                    let t = i as f64 / sr as f64;
                    next_noise() * (-t * 60.0).exp() * 0.5 * vel
                })
                .collect()
        }
        _ => {
            // Other percussion
            let length = (0.06 * sr as f64) as usize;
            (0..length)
                .map(|i| {
                    let t = i as f64 / sr as f64;
                    next_noise() * (-t * 40.0).exp() * 0.4 * vel
                })
                .collect()
        }
    }
}

/// Synthesize a MIDI track to audio samples.
pub fn synthesize_track(track: &MidiTrack, sr: u32) -> Vec<f64> {
    if track.notes.is_empty() {
        return Vec::new();
    }

    let total_samples = ((track.total_duration + 1.0) * sr as f64) as usize;
    let max_samples = (MAX_DURATION * sr as f64) as usize;
    let len = total_samples.min(max_samples);
    let mut audio = vec![0.0f64; len];

    for note in &track.notes {
        let start_idx = (note.start * sr as f64).round() as usize;
        let samples = if track.is_drum {
            synthesize_drum(note.pitch, note.velocity, sr)
        } else {
            synthesize_note(note, sr)
        };

        for (i, &s) in samples.iter().enumerate() {
            let dst = start_idx + i;
            if dst < audio.len() {
                audio[dst] += s;
            }
        }
    }

    audio
}

/// Synthesize and mix multiple MIDI tracks into a preview WAV.
pub fn synthesize_preview(
    tracks: &[MidiTrack],
    output_path: &Path,
) -> Result<()> {
    let sr = SAMPLE_RATE;

    let mut track_audio: Vec<Vec<f64>> = Vec::new();
    for track in tracks {
        let audio = synthesize_track(track, sr);
        if !audio.is_empty() {
            track_audio.push(audio);
        }
    }

    if track_audio.is_empty() {
        anyhow::bail!("No tracks to mix");
    }

    // Pad to same length and mix
    let max_len = track_audio.iter().map(|t| t.len()).max().unwrap();
    let max_samples = (MAX_DURATION * sr as f64) as usize;
    let mix_len = max_len.min(max_samples);

    let mut mixed = vec![0.0f64; mix_len];
    for t in &track_audio {
        let end = t.len().min(mix_len);
        for i in 0..end {
            mixed[i] += t[i];
        }
    }

    // Normalize
    let peak = mixed.iter().map(|s| s.abs()).fold(0.0f64, f64::max);
    if peak > 0.0 {
        let scale = 0.9 / peak;
        for s in mixed.iter_mut() {
            *s *= scale;
        }
    }

    write_wav(output_path, &mixed, sr)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthesize_note() {
        let note = Note {
            pitch: 69, // A4
            start: 0.0,
            end: 0.5,
            velocity: 100,
        };
        let samples = synthesize_note(&note, SAMPLE_RATE);
        assert!(!samples.is_empty());
        let expected_len = (0.5 * SAMPLE_RATE as f64).round() as usize;
        assert_eq!(samples.len(), expected_len);
        // Should have signal (not all zeros)
        assert!(samples.iter().any(|&s| s.abs() > 0.01));
    }

    #[test]
    fn test_synthesize_drum_kick() {
        let samples = synthesize_drum(36, 100, SAMPLE_RATE);
        assert!(!samples.is_empty());
    }

    #[test]
    fn test_synthesize_drum_snare() {
        let samples = synthesize_drum(38, 100, SAMPLE_RATE);
        assert!(!samples.is_empty());
    }

    #[test]
    fn test_synthesize_track_empty() {
        let track = MidiTrack {
            notes: vec![],
            tempo: 120.0,
            program: 0,
            is_drum: false,
            total_duration: 0.0,
        };
        assert!(synthesize_track(&track, SAMPLE_RATE).is_empty());
    }

    #[test]
    fn test_synthesize_track_basic() {
        let track = MidiTrack {
            notes: vec![
                Note { pitch: 60, start: 0.0, end: 0.5, velocity: 100 },
                Note { pitch: 64, start: 0.5, end: 1.0, velocity: 80 },
            ],
            tempo: 120.0,
            program: 0,
            is_drum: false,
            total_duration: 1.0,
        };
        let audio = synthesize_track(&track, SAMPLE_RATE);
        assert!(!audio.is_empty());
    }

    #[test]
    fn test_midi_to_hz_in_synthesize() {
        // Verify note frequencies are reasonable
        assert!(midi_to_hz(60) > 200.0 && midi_to_hz(60) < 300.0); // C4
        assert!(midi_to_hz(69) > 430.0 && midi_to_hz(69) < 450.0); // A4
    }
}
