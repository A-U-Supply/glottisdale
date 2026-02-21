//! Non-blocking audio playback engine for the editor.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rodio::{OutputStream, Sink};

/// Command sent to the playback thread.
pub enum PlaybackCommand {
    /// Play samples from a cursor position.
    PlaySamples {
        samples: Vec<f64>,
        sample_rate: u32,
        start_cursor_s: f64,
    },
    /// Pause playback.
    Pause,
    /// Resume playback.
    Resume,
    /// Stop playback and reset cursor.
    Stop,
}

/// Shared playback state readable from the GUI thread.
#[derive(Clone)]
pub struct PlaybackState {
    /// Current playback cursor in seconds.
    pub cursor_s: Arc<Mutex<f64>>,
    /// Whether currently playing.
    pub is_playing: Arc<Mutex<bool>>,
    /// Last error message from the playback thread.
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

    pub fn get_cursor(&self) -> f64 {
        *self.cursor_s.lock().unwrap()
    }

    pub fn is_playing(&self) -> bool {
        *self.is_playing.lock().unwrap()
    }

    /// Store an error message from the playback thread.
    pub fn set_error(&self, msg: String) {
        *self.last_error.lock().unwrap() = Some(msg);
    }

    /// Take the last error, clearing it.
    pub fn take_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().take()
    }
}

/// Non-blocking playback engine.
///
/// Owns a dedicated audio thread. Send commands via `send()`,
/// read cursor/playing state from `state`.
pub struct PlaybackEngine {
    command_tx: mpsc::Sender<PlaybackCommand>,
    pub state: PlaybackState,
}

impl PlaybackEngine {
    /// Create a new playback engine and spawn the audio thread.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let state = PlaybackState::new();

        let thread_state = state.clone();
        std::thread::Builder::new()
            .name("playback-engine".into())
            .spawn(move || {
                playback_thread(rx, thread_state);
            })
            .expect("Failed to spawn playback thread");

        Self {
            command_tx: tx,
            state,
        }
    }

    /// Send a command to the playback engine.
    pub fn send(&self, cmd: PlaybackCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Play audio samples starting at a given cursor position.
    pub fn play_samples(&self, samples: Vec<f64>, sample_rate: u32, start_cursor_s: f64) {
        self.send(PlaybackCommand::PlaySamples {
            samples,
            sample_rate,
            start_cursor_s,
        });
    }

    /// Stop playback.
    pub fn stop(&self) {
        self.send(PlaybackCommand::Stop);
    }

    /// Pause playback.
    pub fn pause(&self) {
        self.send(PlaybackCommand::Pause);
    }

    /// Resume playback.
    pub fn resume(&self) {
        self.send(PlaybackCommand::Resume);
    }
}

fn playback_thread(rx: mpsc::Receiver<PlaybackCommand>, state: PlaybackState) {
    // Try to open audio output; if it fails, the thread just consumes commands.
    // OutputStream must stay alive for the entire thread lifetime.
    let audio = OutputStream::try_default().ok();
    let stream_handle = audio.as_ref().map(|(_, h)| h);

    // Sink is recreated for each PlaySamples command because Sink::stop()
    // permanently kills the sink (sets a stopped flag that prevents new sources).
    let mut sink: Option<Sink> = None;
    let mut play_start: Option<(Instant, f64)> = None; // (wall_start, cursor_start)

    loop {
        // Process all pending commands (non-blocking)
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlaybackCommand::PlaySamples {
                    samples,
                    sample_rate: sr,
                    start_cursor_s,
                } => {
                    if let Some(handle) = stream_handle {
                        // Drop old sink, create a fresh one
                        drop(sink.take());
                        if let Ok(new_sink) = Sink::try_new(handle) {
                            let source = crate::audio::playback::make_f64_source(samples, sr);
                            new_sink.append(source);
                            new_sink.play();
                            sink = Some(new_sink);
                            play_start = Some((Instant::now(), start_cursor_s));
                            *state.is_playing.lock().unwrap() = true;
                        }
                    }
                }
                PlaybackCommand::Pause => {
                    if let Some(ref s) = sink {
                        s.pause();
                        *state.is_playing.lock().unwrap() = false;
                    }
                }
                PlaybackCommand::Resume => {
                    if let Some(ref s) = sink {
                        s.play();
                        *state.is_playing.lock().unwrap() = true;
                    }
                }
                PlaybackCommand::Stop => {
                    drop(sink.take());
                    play_start = None;
                    *state.is_playing.lock().unwrap() = false;
                    *state.cursor_s.lock().unwrap() = 0.0;
                }
            }
        }

        // Update cursor position
        if let Some((start_instant, start_cursor)) = play_start {
            if let Some(ref s) = sink {
                if s.empty() {
                    // Playback finished
                    *state.is_playing.lock().unwrap() = false;
                    play_start = None;
                } else if !s.is_paused() {
                    let elapsed = start_instant.elapsed().as_secs_f64();
                    *state.cursor_s.lock().unwrap() = start_cursor + elapsed;
                }
            }
        }

        // Sleep briefly to avoid busy-spinning
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Check if the channel is closed (sender dropped)
        if matches!(rx.try_recv(), Err(mpsc::TryRecvError::Disconnected)) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_state_defaults() {
        let state = PlaybackState::new();
        assert_eq!(state.get_cursor(), 0.0);
        assert!(!state.is_playing());
    }

    #[test]
    fn test_playback_engine_creation() {
        // Just verify it doesn't panic
        let engine = PlaybackEngine::new();
        assert!(!engine.state.is_playing());
        engine.stop();
    }

    #[test]
    fn test_playback_state_error_handling() {
        let state = PlaybackState::new();

        // Initially no error
        assert!(state.take_error().is_none());

        // Set an error
        state.set_error("test error".to_string());
        assert_eq!(state.take_error(), Some("test error".to_string()));

        // After taking, error is cleared
        assert!(state.take_error().is_none());
    }
}
