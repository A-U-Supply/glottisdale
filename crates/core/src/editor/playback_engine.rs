//! Non-blocking audio playback engine for the editor.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rodio::{buffer::SamplesBuffer, OutputStream, Sink};

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

impl Default for PlaybackState {
    fn default() -> Self {
        Self::new()
    }
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

impl Default for PlaybackEngine {
    fn default() -> Self {
        Self::new()
    }
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
        if self.command_tx.send(cmd).is_err() {
            log::error!("Playback thread is not running (channel closed)");
            self.state
                .set_error("Playback thread stopped unexpectedly".into());
        }
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

fn process_command(
    cmd: PlaybackCommand,
    stream_handle: Option<&rodio::OutputStreamHandle>,
    sink: &mut Option<Sink>,
    play_start: &mut Option<(Instant, f64)>,
    state: &PlaybackState,
) {
    match cmd {
        PlaybackCommand::PlaySamples {
            samples,
            sample_rate: sr,
            start_cursor_s,
        } => {
            if samples.is_empty() {
                log::warn!("PlaySamples received empty audio buffer");
                return;
            }
            if let Some(handle) = stream_handle {
                // Drop old sink, create a fresh one
                drop(sink.take());
                match Sink::try_new(handle) {
                    Ok(new_sink) => {
                        // Convert f64 → f32 and use rodio's built-in SamplesBuffer
                        // (most battle-tested Source path through rodio internals)
                        let f32_samples: Vec<f32> =
                            samples.iter().map(|&s| s as f32).collect();
                        let source = SamplesBuffer::new(1, sr, f32_samples);
                        new_sink.append(source);
                        new_sink.play();
                        *sink = Some(new_sink);
                        *play_start = Some((Instant::now(), start_cursor_s));
                        *state.is_playing.lock().unwrap() = true;
                        log::debug!(
                            "Playing {} samples at {} Hz from cursor {:.3}s",
                            samples.len(),
                            sr,
                            start_cursor_s
                        );
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
            *play_start = None;
            *state.is_playing.lock().unwrap() = false;
            *state.cursor_s.lock().unwrap() = 0.0;
        }
    }
}

fn playback_thread(rx: mpsc::Receiver<PlaybackCommand>, state: PlaybackState) {
    // Try to open audio output; if it fails, the thread just consumes commands.
    // OutputStream must stay alive for the entire thread lifetime.
    let audio = match OutputStream::try_default() {
        Ok(pair) => {
            log::info!("Playback engine: audio device opened successfully");
            Some(pair)
        }
        Err(e) => {
            log::error!("Failed to open audio output: {}", e);
            state.set_error(format!("Audio device: {}", e));
            None
        }
    };
    let stream_handle = audio.as_ref().map(|(_, h)| h);

    // Sink is recreated for each PlaySamples command because Sink::stop()
    // permanently kills the sink (sets a stopped flag that prevents new sources).
    let mut sink: Option<Sink> = None;
    let mut play_start: Option<(Instant, f64)> = None; // (wall_start, cursor_start)

    loop {
        // Wait for a command (blocks up to 10ms, then falls through for cursor updates).
        // Using recv_timeout instead of try_recv + sleep avoids the race condition
        // where a separate disconnect-check try_recv would silently consume commands.
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(cmd) => {
                process_command(cmd, stream_handle, &mut sink, &mut play_start, &state);
                // Drain any additional pending commands without blocking
                while let Ok(cmd) = rx.try_recv() {
                    process_command(
                        cmd,
                        stream_handle,
                        &mut sink,
                        &mut play_start,
                        &state,
                    );
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No commands — fall through to cursor update
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Sender dropped, shut down the thread
                break;
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

    #[test]
    fn test_playback_engine_reports_empty_play() {
        let engine = PlaybackEngine::new();
        engine.play_samples(vec![], 16000, 0.0);
        std::thread::sleep(std::time::Duration::from_millis(50));
        engine.stop();
    }

    /// Regression test: commands sent after the thread is running must be
    /// processed, not silently consumed by a disconnect check.
    #[test]
    fn test_play_command_not_silently_eaten() {
        let engine = PlaybackEngine::new();

        // Wait for the thread to enter its main loop (past OutputStream init)
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Generate a 0.5s tone
        let sr = 16000u32;
        let samples: Vec<f64> = (0..sr / 2)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin())
            .collect();

        engine.play_samples(samples, sr, 0.0);

        // Poll for evidence that the command was processed (playing started,
        // cursor moved, or an error was reported). Playback might finish
        // quickly if the audio device processes samples faster than real-time.
        let mut was_playing = false;
        let mut cursor_moved = false;
        let mut got_error = false;
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if engine.state.is_playing() {
                was_playing = true;
            }
            if engine.state.get_cursor() > 0.0 {
                cursor_moved = true;
            }
            if engine.state.take_error().is_some() {
                got_error = true;
            }
            if was_playing || cursor_moved || got_error {
                break;
            }
        }

        assert!(
            was_playing || cursor_moved || got_error,
            "Command was silently dropped: was_playing={}, cursor_moved={}, got_error={}",
            was_playing,
            cursor_moved,
            got_error
        );

        engine.stop();
    }
}
