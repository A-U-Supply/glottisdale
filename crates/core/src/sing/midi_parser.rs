//! Parse MIDI files into structured note sequences.

use std::path::Path;

use anyhow::{Result, Context};
use midly::{Smf, TrackEventKind, MidiMessage, MetaMessage};

/// A single MIDI note.
#[derive(Debug, Clone)]
pub struct Note {
    /// MIDI pitch (0-127)
    pub pitch: u8,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
    /// Velocity (0-127)
    pub velocity: u8,
}

impl Note {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

/// Parsed MIDI track.
#[derive(Debug, Clone)]
pub struct MidiTrack {
    pub notes: Vec<Note>,
    pub tempo: f64,
    pub program: u8,
    pub is_drum: bool,
    pub total_duration: f64,
}

/// Convert MIDI pitch to frequency in Hz.
pub fn midi_to_hz(midi_note: u8) -> f64 {
    440.0 * 2.0f64.powf((midi_note as f64 - 69.0) / 12.0)
}

/// Parse a MIDI file into a MidiTrack.
///
/// Merges all non-drum instruments. Extracts tempo from meta events.
pub fn parse_midi(path: &Path) -> Result<MidiTrack> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read MIDI file: {}", path.display()))?;
    let smf = Smf::parse(&data)
        .map_err(|e| anyhow::anyhow!("Failed to parse MIDI: {}", e))?;

    let ticks_per_beat = match smf.header.timing {
        midly::Timing::Metrical(tpb) => tpb.as_int() as f64,
        midly::Timing::Timecode(fps, sub) => {
            // For timecode, compute equivalent ticks per beat at 120 BPM
            let frames_per_sec = match fps {
                midly::Fps::Fps24 => 24.0,
                midly::Fps::Fps25 => 25.0,
                midly::Fps::Fps29 => 29.97,
                midly::Fps::Fps30 => 30.0,
            };
            frames_per_sec * sub as f64 / 2.0 // assume 120 BPM
        }
    };

    let mut tempo_us_per_beat = 500_000.0; // default 120 BPM
    let mut notes: Vec<Note> = Vec::new();
    let mut program: u8 = 0;
    let is_drum = false;

    // Track active notes: (pitch) -> (start_time, velocity)
    let mut active: std::collections::HashMap<u8, (f64, u8)> = std::collections::HashMap::new();
    let mut max_time = 0.0f64;

    for track in &smf.tracks {
        let mut time_s = 0.0f64;
        let mut current_tempo = tempo_us_per_beat;
        active.clear();

        for event in track {
            let delta_ticks = event.delta.as_int() as f64;
            let delta_s = (delta_ticks / ticks_per_beat) * (current_tempo / 1_000_000.0);
            time_s += delta_s;

            match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
                    current_tempo = t.as_int() as f64;
                    tempo_us_per_beat = current_tempo;
                }
                TrackEventKind::Midi { channel, message } => {
                    // Skip channel 10 (drums, 0-indexed = 9)
                    if channel.as_int() == 9 {
                        continue;
                    }

                    match message {
                        MidiMessage::ProgramChange { program: p } => {
                            program = p.as_int();
                        }
                        MidiMessage::NoteOn { key, vel } => {
                            if vel.as_int() > 0 {
                                active.insert(key.as_int(), (time_s, vel.as_int()));
                            } else {
                                // Note-on with velocity 0 = note-off
                                if let Some((start, velocity)) = active.remove(&key.as_int()) {
                                    notes.push(Note {
                                        pitch: key.as_int(),
                                        start: (start * 10000.0).round() / 10000.0,
                                        end: (time_s * 10000.0).round() / 10000.0,
                                        velocity,
                                    });
                                }
                            }
                        }
                        MidiMessage::NoteOff { key, .. } => {
                            if let Some((start, velocity)) = active.remove(&key.as_int()) {
                                notes.push(Note {
                                    pitch: key.as_int(),
                                    start: (start * 10000.0).round() / 10000.0,
                                    end: (time_s * 10000.0).round() / 10000.0,
                                    velocity,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            max_time = max_time.max(time_s);
        }

        // Close any remaining active notes
        for (pitch, (start, velocity)) in active.drain() {
            notes.push(Note {
                pitch,
                start: (start * 10000.0).round() / 10000.0,
                end: (max_time * 10000.0).round() / 10000.0,
                velocity,
            });
        }
    }

    // Sort by start time
    notes.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());

    let tempo_bpm = 60_000_000.0 / tempo_us_per_beat;

    Ok(MidiTrack {
        notes,
        tempo: tempo_bpm.round(),
        program,
        is_drum,
        total_duration: max_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_to_hz() {
        // A4 = 440 Hz
        assert!((midi_to_hz(69) - 440.0).abs() < 0.01);
        // C4 = ~261.63 Hz
        assert!((midi_to_hz(60) - 261.63).abs() < 0.1);
        // A3 = 220 Hz
        assert!((midi_to_hz(57) - 220.0).abs() < 0.01);
    }

    #[test]
    fn test_note_duration() {
        let note = Note {
            pitch: 60,
            start: 1.0,
            end: 2.5,
            velocity: 100,
        };
        assert!((note.duration() - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_parse_midi_nonexistent() {
        let result = parse_midi(Path::new("/nonexistent.mid"));
        assert!(result.is_err());
    }
}
