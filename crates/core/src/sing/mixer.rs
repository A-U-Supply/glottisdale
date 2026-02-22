//! Mix vocal track with MIDI backing tracks.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::audio::effects::mix_audio;
use crate::audio::io::{read_wav, write_wav};
use crate::sing::midi_parser::MidiTrack;
use crate::sing::synthesize::synthesize_preview;

/// Mix vocal audio with MIDI backing.
///
/// Returns (full_mix_path, acappella_path).
pub fn mix_tracks(
    vocal_samples: &[f64],
    vocal_sr: u32,
    midi_tracks: &[MidiTrack],
    output_dir: &Path,
    vocal_db: f64,
    midi_db: f64,
) -> Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(output_dir)?;
    let run_name = output_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let acappella_path = output_dir.join(format!("{}-acappella.wav", run_name));
    let full_mix_path = output_dir.join(format!("{}.wav", run_name));

    // Write a cappella
    write_wav(&acappella_path, vocal_samples, vocal_sr)?;

    // Synthesize MIDI backing
    let midi_wav = output_dir.join("midi_backing.wav");
    let has_midi = synthesize_preview(midi_tracks, &midi_wav).is_ok();

    if has_midi && midi_wav.exists() {
        // Load the MIDI backing and mix
        let (midi_samples, _midi_sr) = read_wav(&midi_wav)?;

        // Apply volume adjustments
        let mut vocals = vocal_samples.to_vec();
        if vocal_db.abs() > 0.1 {
            crate::audio::effects::adjust_volume(&mut vocals, vocal_db);
        }

        let mut midi = midi_samples;
        // Resample MIDI to match vocal sample rate if needed
        // (synthesizer outputs at 22050, vocals at 16000)
        if !midi.is_empty() {
            let midi_sr = 22050; // from synthesizer
            if midi_sr != vocal_sr {
                if let Ok(resampled) = crate::audio::io::resample(&midi, midi_sr, vocal_sr) {
                    midi = resampled;
                }
            }
        }

        let mixed = mix_audio(&vocals, &midi, midi_db);
        write_wav(&full_mix_path, &mixed, vocal_sr)?;
    } else {
        log::warn!("MIDI synthesis failed, using a cappella as full mix");
        write_wav(&full_mix_path, vocal_samples, vocal_sr)?;
    }

    Ok((full_mix_path, acappella_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sing::midi_parser::Note;

    #[test]
    fn test_mix_tracks_no_midi() {
        let dir = std::env::temp_dir().join(format!("glottisdale_mixer_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let vocals: Vec<f64> = (0..8000)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 16000.0).sin() * 0.5)
            .collect();

        let result = mix_tracks(&vocals, 16000, &[], &dir, 0.0, -12.0);
        assert!(result.is_ok());

        let (full_mix, acappella) = result.unwrap();
        assert!(acappella.exists());
        assert!(full_mix.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mix_tracks_with_midi() {
        let dir = std::env::temp_dir().join(format!("glottisdale_mixer_midi_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let vocals: Vec<f64> = (0..16000)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 16000.0).sin() * 0.5)
            .collect();

        let tracks = vec![MidiTrack {
            notes: vec![
                Note { pitch: 60, start: 0.0, end: 0.5, velocity: 100 },
                Note { pitch: 64, start: 0.5, end: 1.0, velocity: 80 },
            ],
            tempo: 120.0,
            program: 0,
            is_drum: false,
            total_duration: 1.0,
        }];

        let result = mix_tracks(&vocals, 16000, &tracks, &dir, 0.0, -12.0);
        assert!(result.is_ok());

        let (full_mix, acappella) = result.unwrap();
        assert!(acappella.exists());
        assert!(full_mix.exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
