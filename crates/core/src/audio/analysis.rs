//! Audio analysis: RMS energy, F0 pitch estimation, room tone detection,
//! breath detection, pink noise generation.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Compute RMS energy of the entire signal.
pub fn compute_rms(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f64).sqrt()
}

/// Compute RMS energy in sliding windows.
///
/// Returns a Vec of RMS values, one per hop step.
pub fn compute_rms_windowed(samples: &[f64], sr: u32, window_ms: u32, hop_ms: u32) -> Vec<f64> {
    let window_samples = (sr as usize * window_ms as usize) / 1000;
    let hop_samples = (sr as usize * hop_ms as usize) / 1000;

    if samples.len() < window_samples || window_samples == 0 || hop_samples == 0 {
        return vec![];
    }

    let n_frames = (samples.len() - window_samples) / hop_samples + 1;
    let mut rms = Vec::with_capacity(n_frames);

    for i in 0..n_frames {
        let start = i * hop_samples;
        let frame = &samples[start..start + window_samples];
        let sum_sq: f64 = frame.iter().map(|s| s * s).sum();
        rms.push((sum_sq / window_samples as f64).sqrt());
    }

    rms
}

/// Find the quietest continuous region at least `min_duration_ms` long.
///
/// Uses windowed RMS to find frames below a quiet threshold (10% of mean RMS),
/// then finds the longest contiguous run of quiet frames.
///
/// Returns `Some((start_s, end_s))` or `None` if no suitable region is found.
pub fn find_room_tone(samples: &[f64], sr: u32, min_duration_ms: u32) -> Option<(f64, f64)> {
    let min_samples = (sr as usize * min_duration_ms as usize) / 1000;
    if samples.len() < min_samples {
        return None;
    }

    let window_ms = 25u32;
    let hop_ms = 12u32;
    let rms = compute_rms_windowed(samples, sr, window_ms, hop_ms);

    if rms.is_empty() {
        return None;
    }

    let mean_rms: f64 = rms.iter().sum::<f64>() / rms.len() as f64;

    if mean_rms < 1e-10 {
        return Some((0.0, samples.len() as f64 / sr as f64));
    }

    let threshold = mean_rms * 0.1;

    // Find longest run of quiet frames
    let mut best_start = 0usize;
    let mut best_length = 0usize;
    let mut current_start = 0usize;
    let mut current_length = 0usize;

    for (i, &val) in rms.iter().enumerate() {
        if val < threshold {
            if current_length == 0 {
                current_start = i;
            }
            current_length += 1;
            if current_length > best_length {
                best_length = current_length;
                best_start = current_start;
            }
        } else {
            current_length = 0;
        }
    }

    if best_length == 0 {
        return None;
    }

    let hop_samples = (sr as usize * hop_ms as usize) / 1000;
    let start_s = (best_start * hop_samples) as f64 / sr as f64;
    let end_s = ((best_start + best_length) * hop_samples) as f64 / sr as f64;

    if (end_s - start_s) < min_duration_ms as f64 / 1000.0 {
        return None;
    }

    Some((start_s, end_s))
}

/// Estimate fundamental frequency using autocorrelation.
///
/// Finds the first autocorrelation peak above a periodicity threshold,
/// searching from the shortest lag (highest frequency) to avoid octave errors.
///
/// Returns F0 in Hz, or `None` for silence, noise, or weak periodicity.
pub fn estimate_f0(samples: &[f64], sr: u32, f0_min: u32, f0_max: u32) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }

    let rms = compute_rms(samples);
    if rms < 1e-6 {
        return None;
    }

    let lag_min = sr as usize / f0_max as usize;
    let lag_max = (sr as usize / f0_min as usize).min(samples.len() - 1);

    if lag_min >= lag_max {
        return None;
    }

    // Remove DC offset
    let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
    let x: Vec<f64> = samples.iter().map(|s| s - mean).collect();

    let autocorr_0: f64 = x.iter().map(|v| v * v).sum();
    if autocorr_0 < 1e-12 {
        return None;
    }

    // Compute normalized autocorrelation for the valid lag range
    let n_lags = lag_max - lag_min + 1;
    let mut autocorr = Vec::with_capacity(n_lags);
    for lag in lag_min..=lag_max {
        let sum: f64 = x[..x.len() - lag]
            .iter()
            .zip(x[lag..].iter())
            .map(|(a, b)| a * b)
            .sum();
        autocorr.push(sum / autocorr_0);
    }

    let threshold = 0.3;

    // Check left boundary
    if autocorr.len() >= 2 && autocorr[0] >= threshold && autocorr[0] >= autocorr[1] {
        return Some(sr as f64 / lag_min as f64);
    }

    // Scan interior points for first peak above threshold
    for i in 1..autocorr.len().saturating_sub(1) {
        if autocorr[i] >= threshold
            && autocorr[i] >= autocorr[i - 1]
            && autocorr[i] >= autocorr[i + 1]
        {
            let best_lag = lag_min + i;
            return Some(sr as f64 / best_lag as f64);
        }
    }

    None
}

/// Find breath-like sounds in inter-word gaps.
///
/// A breath is an inter-word gap in [min_gap_ms, max_gap_ms] whose RMS
/// is between 1% and 30% of the speech RMS level.
pub fn find_breaths(
    samples: &[f64],
    sr: u32,
    word_boundaries: &[(f64, f64)],
    min_gap_ms: u32,
    max_gap_ms: u32,
) -> Vec<(f64, f64)> {
    if word_boundaries.len() < 2 {
        return vec![];
    }

    let mut sorted_bounds = word_boundaries.to_vec();
    sorted_bounds.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Compute speech RMS
    let mut speech_samples = Vec::new();
    for &(start, end) in &sorted_bounds {
        let s = ((start * sr as f64) as usize).clamp(0, samples.len());
        let e = ((end * sr as f64) as usize).clamp(0, samples.len());
        if e > s {
            speech_samples.extend_from_slice(&samples[s..e]);
        }
    }

    if speech_samples.is_empty() {
        return vec![];
    }

    let speech_rms = compute_rms(&speech_samples);
    if speech_rms < 1e-6 {
        return vec![];
    }

    let min_gap_s = min_gap_ms as f64 / 1000.0;
    let max_gap_s = max_gap_ms as f64 / 1000.0;

    let mut breaths = Vec::new();
    for i in 0..sorted_bounds.len() - 1 {
        let gap_start = sorted_bounds[i].1;
        let gap_end = sorted_bounds[i + 1].0;
        let gap_duration = gap_end - gap_start;

        if gap_duration < min_gap_s || gap_duration > max_gap_s {
            continue;
        }

        let s = ((gap_start * sr as f64) as usize).clamp(0, samples.len());
        let e = ((gap_end * sr as f64) as usize).clamp(0, samples.len());
        if e <= s {
            continue;
        }

        let gap_rms = compute_rms(&samples[s..e]);
        let ratio = gap_rms / speech_rms;

        if (0.01..=0.30).contains(&ratio) {
            breaths.push((gap_start, gap_end));
        }
    }

    breaths
}

/// Generate pink noise (1/f spectrum) via spectral shaping.
///
/// White noise FFT → multiply by 1/sqrt(f) → IFFT → normalize to [-1, 1].
pub fn generate_pink_noise(duration_s: f64, sr: u32, seed: Option<u64>) -> Vec<f64> {
    let n_samples = (duration_s * sr as f64) as usize;
    if n_samples == 0 {
        return vec![];
    }

    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    voss_mccartney_pink_noise(n_samples, &mut rng)
}

/// Voss-McCartney algorithm for pink noise generation.
///
/// Uses multiple random number generators at different update rates
/// to approximate 1/f spectrum without FFT.
fn voss_mccartney_pink_noise(n_samples: usize, rng: &mut StdRng) -> Vec<f64> {
    const NUM_ROWS: usize = 16;
    let mut rows = [0.0f64; NUM_ROWS];
    let mut running_sum = 0.0f64;

    // Initialize
    for row in rows.iter_mut() {
        *row = rng.gen_range(-1.0..1.0);
        running_sum += *row;
    }

    let mut output = Vec::with_capacity(n_samples);

    for i in 0..n_samples {
        // Determine which rows to update based on trailing zeros of index
        let mut changed = i;
        let mut num_zeros = 0;
        while changed != 0 && (changed & 1) == 0 {
            num_zeros += 1;
            changed >>= 1;
        }

        if num_zeros < NUM_ROWS {
            running_sum -= rows[num_zeros];
            rows[num_zeros] = rng.gen_range(-1.0..1.0);
            running_sum += rows[num_zeros];
        }

        // Add white noise component for high-frequency content
        let white = rng.gen_range(-1.0..1.0);
        output.push(running_sum + white);
    }

    // Normalize to [-1, 1]
    let peak = output.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
    if peak > 0.0 {
        for v in output.iter_mut() {
            *v /= peak;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_rms_silence() {
        assert_eq!(compute_rms(&[]), 0.0);
        assert_eq!(compute_rms(&[0.0; 100]), 0.0);
    }

    #[test]
    fn test_compute_rms_known_signal() {
        // DC signal of 0.5 → RMS = 0.5
        let samples = vec![0.5; 1000];
        let rms = compute_rms(&samples);
        assert!((rms - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_compute_rms_sine() {
        // Sine wave with amplitude 1.0 → RMS ≈ 1/√2 ≈ 0.7071
        let samples: Vec<f64> = (0..16000)
            .map(|i| (i as f64 / 16000.0 * 440.0 * std::f64::consts::TAU).sin())
            .collect();
        let rms = compute_rms(&samples);
        assert!((rms - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.01,
            "Expected ~0.707, got {}", rms);
    }

    #[test]
    fn test_compute_rms_windowed() {
        let samples = vec![0.5; 16000]; // 1 second at 16kHz
        let rms = compute_rms_windowed(&samples, 16000, 100, 50);
        assert!(!rms.is_empty());
        for &val in &rms {
            assert!((val - 0.5).abs() < 0.001);
        }
    }

    #[test]
    fn test_compute_rms_windowed_short() {
        let rms = compute_rms_windowed(&[0.0; 10], 16000, 100, 50);
        assert!(rms.is_empty());
    }

    #[test]
    fn test_estimate_f0_440hz() {
        // Generate 440 Hz sine wave
        let sr = 16000u32;
        let samples: Vec<f64> = (0..sr as usize)
            .map(|i| (i as f64 / sr as f64 * 440.0 * std::f64::consts::TAU).sin())
            .collect();
        let f0 = estimate_f0(&samples, sr, 50, 600);
        assert!(f0.is_some(), "Should detect F0");
        let f0 = f0.unwrap();
        assert!((f0 - 440.0).abs() < 10.0, "Expected ~440 Hz, got {} Hz", f0);
    }

    #[test]
    fn test_estimate_f0_silence() {
        let samples = vec![0.0; 16000];
        assert!(estimate_f0(&samples, 16000, 50, 400).is_none());
    }

    #[test]
    fn test_estimate_f0_empty() {
        assert!(estimate_f0(&[], 16000, 50, 400).is_none());
    }

    #[test]
    fn test_find_room_tone_with_quiet_region() {
        let sr = 16000u32;
        // 1s of loud signal + 1s of silence + 1s of loud signal
        let mut samples = Vec::new();
        for i in 0..sr as usize {
            samples.push((i as f64 / sr as f64 * 440.0 * std::f64::consts::TAU).sin());
        }
        samples.extend(vec![0.0; sr as usize]); // 1s silence
        for i in 0..sr as usize {
            samples.push((i as f64 / sr as f64 * 440.0 * std::f64::consts::TAU).sin());
        }

        let result = find_room_tone(&samples, sr, 500);
        assert!(result.is_some(), "Should find room tone");
        let (start, end) = result.unwrap();
        assert!(start >= 0.8 && start <= 1.2, "Start should be ~1.0, got {}", start);
        assert!(end >= 1.8 && end <= 2.2, "End should be ~2.0, got {}", end);
    }

    #[test]
    fn test_find_room_tone_all_silence() {
        let samples = vec![0.0; 16000];
        let result = find_room_tone(&samples, 16000, 500);
        assert!(result.is_some());
    }

    #[test]
    fn test_find_room_tone_too_short() {
        let samples = vec![0.0; 100];
        assert!(find_room_tone(&samples, 16000, 500).is_none());
    }

    #[test]
    fn test_find_breaths_empty() {
        assert!(find_breaths(&[], 16000, &[], 200, 600).is_empty());
        assert!(find_breaths(&[0.0; 100], 16000, &[(0.0, 0.5)], 200, 600).is_empty());
    }

    #[test]
    fn test_generate_pink_noise_length() {
        let noise = generate_pink_noise(1.0, 16000, Some(42));
        assert_eq!(noise.len(), 16000);
    }

    #[test]
    fn test_generate_pink_noise_normalized() {
        let noise = generate_pink_noise(1.0, 16000, Some(42));
        let peak = noise.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
        assert!((peak - 1.0).abs() < 0.01, "Should be normalized, peak = {}", peak);
    }

    #[test]
    fn test_generate_pink_noise_deterministic() {
        let a = generate_pink_noise(0.1, 16000, Some(42));
        let b = generate_pink_noise(0.1, 16000, Some(42));
        assert_eq!(a, b);
    }

    #[test]
    fn test_generate_pink_noise_empty() {
        let noise = generate_pink_noise(0.0, 16000, None);
        assert!(noise.is_empty());
    }
}
