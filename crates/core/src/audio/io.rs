//! Audio I/O: WAV read/write, format detection, duration.

use anyhow::{Context, Result};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;

/// Read a WAV file and return (samples_f64_normalized, sample_rate).
///
/// - Normalizes int16/int32 to f64 in [-1, 1]
/// - Passes through float WAVs as f64
/// - Takes the first channel if stereo/multi-channel
pub fn read_wav(path: &Path) -> Result<(Vec<f64>, u32)> {
    let reader = WavReader::open(path)
        .with_context(|| format!("Failed to open WAV file: {}", path.display()))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    let samples: Vec<f64> = match spec.sample_format {
        SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1i64 << (bits - 1)) as f64;
            reader
                .into_samples::<i32>()
                .enumerate()
                .filter_map(|(i, s)| {
                    // Take first channel only
                    if i % channels == 0 {
                        Some(s.map(|v| v as f64 / max_val))
                    } else {
                        // Still consume the sample to advance the iterator
                        let _ = s;
                        None
                    }
                })
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("Failed to read WAV samples")?
        }
        SampleFormat::Float => {
            reader
                .into_samples::<f32>()
                .enumerate()
                .filter_map(|(i, s)| {
                    if i % channels == 0 {
                        Some(s.map(|v| v as f64))
                    } else {
                        let _ = s;
                        None
                    }
                })
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("Failed to read WAV samples")?
        }
    };

    Ok((samples, sample_rate))
}

/// Write f64 samples to a 16-bit PCM WAV file.
///
/// Clips values to [-1, 1] before conversion.
/// Creates parent directories if needed.
pub fn write_wav(path: &Path, samples: &[f64], sample_rate: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)
        .with_context(|| format!("Failed to create WAV file: {}", path.display()))?;

    for &sample in samples {
        let clipped = sample.clamp(-1.0, 1.0);
        let int16 = (clipped * 32767.0) as i16;
        writer.write_sample(int16)?;
    }

    writer.finalize().context("Failed to finalize WAV file")?;
    Ok(())
}

/// Get duration of a WAV file in seconds.
pub fn get_wav_duration(path: &Path) -> Result<f64> {
    let reader = WavReader::open(path)
        .with_context(|| format!("Failed to open WAV file: {}", path.display()))?;
    let spec = reader.spec();
    let num_samples = reader.len() as f64;
    let channels = spec.channels as f64;
    Ok(num_samples / channels / spec.sample_rate as f64)
}

/// Extract a time range from samples. Returns the slice as a new Vec.
///
/// Clamps to valid bounds.
pub fn extract_range(samples: &[f64], sample_rate: u32, start_s: f64, end_s: f64) -> Vec<f64> {
    let start_idx = (start_s * sample_rate as f64).round() as usize;
    let end_idx = (end_s * sample_rate as f64).round() as usize;
    let start_idx = start_idx.min(samples.len());
    let end_idx = end_idx.min(samples.len());
    if start_idx >= end_idx {
        return vec![];
    }
    samples[start_idx..end_idx].to_vec()
}

/// Resample audio from source sample rate to target sample rate.
///
/// Uses rubato for high-quality resampling.
pub fn resample(samples: &[f64], from_sr: u32, to_sr: u32) -> Result<Vec<f64>> {
    if from_sr == to_sr {
        return Ok(samples.to_vec());
    }

    if samples.is_empty() {
        return Ok(vec![]);
    }

    use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction, Resampler};

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = to_sr as f64 / from_sr as f64;
    let mut resampler = SincFixedIn::<f64>::new(
        ratio,
        2.0, // max relative ratio (allows some flexibility)
        params,
        samples.len(),
        1, // mono
    )?;

    let input = vec![samples.to_vec()];
    let output = resampler.process(&input, None)?;

    Ok(output.into_iter().next().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_wav_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("glottisdale_test_io");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_write_read_roundtrip() {
        let path = temp_wav_path("roundtrip.wav");
        let samples: Vec<f64> = (0..1000).map(|i| (i as f64 / 1000.0 * std::f64::consts::TAU).sin() * 0.5).collect();
        write_wav(&path, &samples, 16000).unwrap();

        let (read_samples, sr) = read_wav(&path).unwrap();
        assert_eq!(sr, 16000);
        assert_eq!(read_samples.len(), samples.len());

        // 16-bit quantization introduces small error
        for (a, b) in samples.iter().zip(read_samples.iter()) {
            assert!((a - b).abs() < 0.001, "sample mismatch: {} vs {}", a, b);
        }

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_clips_values() {
        let path = temp_wav_path("clipping.wav");
        let samples = vec![-2.0, -1.0, 0.0, 1.0, 2.0];
        write_wav(&path, &samples, 16000).unwrap();

        let (read, _) = read_wav(&path).unwrap();
        // Values beyond [-1, 1] should be clipped
        assert!(read[0] >= -1.0 && read[0] <= -0.99);
        assert!(read[4] >= 0.99 && read[4] <= 1.0);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_get_wav_duration() {
        let path = temp_wav_path("duration.wav");
        let samples = vec![0.0; 16000]; // 1 second at 16kHz
        write_wav(&path, &samples, 16000).unwrap();

        let dur = get_wav_duration(&path).unwrap();
        assert!((dur - 1.0).abs() < 0.001);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_extract_range() {
        let samples: Vec<f64> = (0..16000).map(|i| i as f64).collect();
        let extracted = extract_range(&samples, 16000, 0.5, 1.0);
        assert_eq!(extracted.len(), 8000);
        assert!((extracted[0] - 8000.0).abs() < 1.0);
    }

    #[test]
    fn test_extract_range_clamped() {
        let samples = vec![0.0; 100];
        let extracted = extract_range(&samples, 100, 0.0, 10.0); // way past end
        assert_eq!(extracted.len(), 100);
    }

    #[test]
    fn test_resample_same_rate() {
        let samples = vec![1.0, 2.0, 3.0];
        let result = resample(&samples, 16000, 16000).unwrap();
        assert_eq!(result, samples);
    }

    #[test]
    fn test_resample_upsample() {
        // 4000 samples at 8kHz → should produce ~8000 samples at 16kHz
        let samples: Vec<f64> = (0..4000).map(|i| (i as f64 / 4000.0 * std::f64::consts::TAU).sin()).collect();
        let result = resample(&samples, 8000, 16000).unwrap();
        // Sinc resampler loses samples at edges due to filter length; allow wide tolerance
        assert!(result.len() >= 7000 && result.len() <= 8500,
            "Expected ~8000 samples, got {}", result.len());
    }

    #[test]
    fn test_resample_empty() {
        let result = resample(&[], 16000, 8000).unwrap();
        assert!(result.is_empty());
    }
}
