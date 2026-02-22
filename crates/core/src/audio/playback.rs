//! Audio playback via rodio for real-time preview.

use anyhow::{Context, Result};
use rodio::{OutputStream, Sink, Source};
use std::sync::Arc;
use std::time::Duration;

/// Play f64 samples through the default audio output device.
///
/// Blocks until playback completes. Returns immediately if samples are empty.
pub fn play_samples(samples: &[f64], sample_rate: u32) -> Result<()> {
    if samples.is_empty() {
        return Ok(());
    }

    let (_stream, stream_handle) =
        OutputStream::try_default().context("Failed to open audio output device")?;
    let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

    let source = F64Source::new(samples.to_vec(), sample_rate);
    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

/// Play a WAV file through the default audio output device.
pub fn play_wav(path: &std::path::Path) -> Result<()> {
    let (samples, sr) = super::io::read_wav(path)?;
    play_samples(&samples, sr)
}

/// Create an F64Source for use in other modules.
pub fn make_f64_source(samples: Vec<f64>, sample_rate: u32) -> F64Source {
    F64Source::new(samples, sample_rate)
}

/// A rodio Source wrapping f64 samples, converting to f32 on the fly.
pub struct F64Source {
    samples: Arc<Vec<f64>>,
    position: usize,
    sample_rate: u32,
}

impl F64Source {
    fn new(samples: Vec<f64>, sample_rate: u32) -> Self {
        Self {
            samples: Arc::new(samples),
            position: 0,
            sample_rate,
        }
    }
}

impl Iterator for F64Source {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.position >= self.samples.len() {
            return None;
        }
        let sample = self.samples[self.position] as f32;
        self.position += 1;
        Some(sample)
    }
}

impl Source for F64Source {
    fn current_frame_len(&self) -> Option<usize> {
        // Return None (matching rodio's SamplesBuffer behavior) to indicate
        // that audio parameters are constant throughout. Returning Some(n)
        // changes how rodio batches its internal resampling pipeline.
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let secs = self.samples.len() as f64 / self.sample_rate as f64;
        Some(Duration::from_secs_f64(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f64_source_properties() {
        let samples = vec![0.5; 16000];
        let source = F64Source::new(samples, 16000);
        assert_eq!(source.channels(), 1);
        assert_eq!(source.sample_rate(), 16000);
        assert_eq!(
            source.total_duration(),
            Some(Duration::from_secs_f64(1.0))
        );
    }

    #[test]
    fn test_f64_source_iteration() {
        let samples = vec![0.25, 0.5, 0.75];
        let source = F64Source::new(samples, 16000);
        let collected: Vec<f32> = source.collect();
        assert_eq!(collected, vec![0.25f32, 0.5, 0.75]);
    }

    #[test]
    fn test_f64_source_empty() {
        let source = F64Source::new(vec![], 16000);
        assert_eq!(source.current_frame_len(), None);
        let collected: Vec<f32> = source.collect();
        assert!(collected.is_empty());
    }
}
