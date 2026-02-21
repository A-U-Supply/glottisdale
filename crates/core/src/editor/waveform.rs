//! Pre-computed waveform peak data for efficient rendering.

/// Pre-computed waveform data for efficient rendering.
///
/// Stores (min_peak, max_peak) pairs at a fixed bucket size.
/// At sr=16000 and bucket_size=256, a 0.3s syllable produces ~19 peak pairs.
#[derive(Debug, Clone)]
pub struct WaveformData {
    /// (min_peak, max_peak) pairs per bucket.
    pub peaks: Vec<(f32, f32)>,
    /// How many source samples each peak bucket represents.
    pub samples_per_bucket: usize,
}

const DEFAULT_BUCKET_SIZE: usize = 256;

impl WaveformData {
    /// Compute waveform peaks from audio samples.
    pub fn from_samples(samples: &[f64], bucket_size: usize) -> Self {
        if samples.is_empty() {
            return Self {
                peaks: Vec::new(),
                samples_per_bucket: bucket_size,
            };
        }

        let mut peaks = Vec::with_capacity(samples.len() / bucket_size + 1);
        for chunk in samples.chunks(bucket_size) {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for &s in chunk {
                if s < min { min = s; }
                if s > max { max = s; }
            }
            peaks.push((min as f32, max as f32));
        }

        Self {
            peaks,
            samples_per_bucket: bucket_size,
        }
    }

    /// Compute waveform peaks with default bucket size (256).
    pub fn new(samples: &[f64]) -> Self {
        Self::from_samples(samples, DEFAULT_BUCKET_SIZE)
    }

    /// Duration in seconds given a sample rate.
    pub fn duration_s(&self, sample_rate: u32) -> f64 {
        (self.peaks.len() * self.samples_per_bucket) as f64 / sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_from_silence() {
        let samples = vec![0.0f64; 1024];
        let wf = WaveformData::from_samples(&samples, 256);
        assert_eq!(wf.peaks.len(), 4);
        assert_eq!(wf.samples_per_bucket, 256);
        for &(min, max) in &wf.peaks {
            assert_eq!(min, 0.0);
            assert_eq!(max, 0.0);
        }
    }

    #[test]
    fn test_waveform_from_sine() {
        let sr = 16000;
        let samples: Vec<f64> = (0..sr)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin())
            .collect();
        let wf = WaveformData::new(&samples);
        assert_eq!(wf.samples_per_bucket, 256);
        // 16000 / 256 = 62.5, so 62 full buckets + 1 partial
        assert_eq!(wf.peaks.len(), 63);
        // Sine wave should have peaks near -1.0 and 1.0
        let max_peak = wf.peaks.iter().map(|&(_, max)| max).fold(f32::NEG_INFINITY, f32::max);
        let min_peak = wf.peaks.iter().map(|&(min, _)| min).fold(f32::INFINITY, f32::min);
        assert!(max_peak > 0.9, "max_peak={}", max_peak);
        assert!(min_peak < -0.9, "min_peak={}", min_peak);
    }

    #[test]
    fn test_waveform_from_impulse() {
        let mut samples = vec![0.0f64; 512];
        samples[100] = 1.0;
        samples[400] = -0.8;
        let wf = WaveformData::from_samples(&samples, 256);
        assert_eq!(wf.peaks.len(), 2);
        // First bucket contains the impulse at index 100
        assert_eq!(wf.peaks[0].1, 1.0);
        // Second bucket contains the negative impulse at index 400
        assert_eq!(wf.peaks[1].0, -0.8f32);
    }

    #[test]
    fn test_waveform_empty() {
        let wf = WaveformData::new(&[]);
        assert!(wf.peaks.is_empty());
    }

    #[test]
    fn test_waveform_partial_bucket() {
        // 300 samples with bucket_size 256 = 1 full bucket + 1 partial
        let samples = vec![0.5f64; 300];
        let wf = WaveformData::from_samples(&samples, 256);
        assert_eq!(wf.peaks.len(), 2);
        assert_eq!(wf.peaks[0], (0.5, 0.5));
        assert_eq!(wf.peaks[1], (0.5, 0.5));
    }

    #[test]
    fn test_waveform_duration() {
        let samples = vec![0.0f64; 16000]; // 1 second at sr=16000
        let wf = WaveformData::new(&samples);
        let dur = wf.duration_s(16000);
        // 63 buckets * 256 samples = 16128 samples -> 1.008s
        // (slight overestimate due to bucket granularity)
        assert!((dur - 1.0).abs() < 0.02, "duration={}", dur);
    }
}
