//! Stretch selection logic for time-stretch features.

use rand::Rng;
use rand::rngs::StdRng;

use crate::types::Clip;

/// Configuration for which syllables/words get stretched.
#[derive(Debug, Clone)]
pub struct StretchConfig {
    /// Probability 0-1 for random stretching
    pub random_stretch: Option<f64>,
    /// Every Nth syllable gets stretched
    pub alternating_stretch: Option<usize>,
    /// First/last N syllables in each word
    pub boundary_stretch: Option<usize>,
    /// Probability 0-1 for whole-word stretch
    pub word_stretch: Option<f64>,
    /// (min, max) range for stretch factor
    pub stretch_factor: (f64, f64),
}

impl Default for StretchConfig {
    fn default() -> Self {
        Self {
            random_stretch: None,
            alternating_stretch: None,
            boundary_stretch: None,
            word_stretch: None,
            stretch_factor: (2.0, 2.0),
        }
    }
}

impl StretchConfig {
    /// Returns true if any syllable-level stretch mode is active.
    pub fn has_syllable_stretch(&self) -> bool {
        self.random_stretch.is_some()
            || self.alternating_stretch.is_some()
            || self.boundary_stretch.is_some()
    }
}

/// Parse stretch factor string: "2.0" or "1.5-3.0" into (min, max).
pub fn parse_stretch_factor(s: &str) -> (f64, f64) {
    if s.contains('-') {
        let parts: Vec<&str> = s.split('-').filter(|p| !p.is_empty()).collect();
        if parts.len() == 2 && !s.starts_with('-') {
            if let (Ok(a), Ok(b)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                return (a, b);
            }
        }
    }
    let val: f64 = s.parse().unwrap_or(2.0);
    (val, val)
}

/// Pick a stretch factor from the range. Fixed if min==max.
pub fn resolve_stretch_factor(factor_range: (f64, f64), rng: &mut StdRng) -> f64 {
    if (factor_range.0 - factor_range.1).abs() < 1e-10 {
        return factor_range.0;
    }
    rng.gen_range(factor_range.0..=factor_range.1)
}

/// Determine if a syllable should be stretched based on active modes.
///
/// Returns true if ANY active mode selects this syllable.
pub fn should_stretch_syllable(
    syllable_index: usize,
    word_syllable_index: usize,
    word_syllable_count: usize,
    rng: &mut StdRng,
    config: &StretchConfig,
) -> bool {
    if let Some(prob) = config.random_stretch {
        if rng.gen::<f64>() < prob {
            return true;
        }
    }

    if let Some(n) = config.alternating_stretch {
        if n > 0 && syllable_index.is_multiple_of(n) {
            return true;
        }
    }

    if let Some(n) = config.boundary_stretch {
        if word_syllable_index < n
            || word_syllable_index >= word_syllable_count.saturating_sub(n)
        {
            return true;
        }
    }

    false
}

/// Parse count string: "2" or "1-3" into (min, max).
pub fn parse_count_range(s: &str) -> (usize, usize) {
    if s.contains('-') {
        let parts: Vec<&str> = s.splitn(2, '-').collect();
        if parts.len() == 2 {
            if let (Ok(a), Ok(b)) = (parts[0].parse(), parts[1].parse()) {
                return (a, b);
            }
        }
    }
    let val: usize = s.parse().unwrap_or(1);
    (val, val)
}

/// Duplicate items in-place for stuttering effect.
///
/// Returns new list with stuttered items repeated.
pub fn apply_stutter<T: Clone>(
    items: &[T],
    probability: f64,
    count_range: (usize, usize),
    rng: &mut StdRng,
) -> Vec<T> {
    let mut result = Vec::new();
    for item in items {
        result.push(item.clone());
        if rng.gen::<f64>() < probability {
            let n = rng.gen_range(count_range.0..=count_range.1);
            for _ in 0..n {
                result.push(item.clone());
            }
        }
    }
    result
}

/// Duplicate words in the word list for repetition effect.
///
/// style="exact": duplicate the same Clip.
pub fn apply_word_repeat(
    words: &[Clip],
    probability: f64,
    count_range: (usize, usize),
    style: &str,
    rng: &mut StdRng,
) -> Vec<Clip> {
    let mut result = Vec::new();
    for word in words {
        result.push(word.clone());
        if rng.gen::<f64>() < probability {
            let n = rng.gen_range(count_range.0..=count_range.1);
            if style == "exact" {
                for _ in 0..n {
                    result.push(word.clone());
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_parse_stretch_factor_fixed() {
        assert_eq!(parse_stretch_factor("2.0"), (2.0, 2.0));
        assert_eq!(parse_stretch_factor("1.5"), (1.5, 1.5));
    }

    #[test]
    fn test_parse_stretch_factor_range() {
        assert_eq!(parse_stretch_factor("1.5-3.0"), (1.5, 3.0));
    }

    #[test]
    fn test_parse_count_range() {
        assert_eq!(parse_count_range("2"), (2, 2));
        assert_eq!(parse_count_range("1-3"), (1, 3));
    }

    #[test]
    fn test_resolve_stretch_factor_fixed() {
        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(resolve_stretch_factor((2.0, 2.0), &mut rng), 2.0);
    }

    #[test]
    fn test_resolve_stretch_factor_range() {
        let mut rng = StdRng::seed_from_u64(42);
        let f = resolve_stretch_factor((1.0, 3.0), &mut rng);
        assert!(f >= 1.0 && f <= 3.0);
    }

    #[test]
    fn test_should_stretch_alternating() {
        let config = StretchConfig {
            alternating_stretch: Some(2),
            ..Default::default()
        };
        let mut rng = StdRng::seed_from_u64(42);
        assert!(should_stretch_syllable(0, 0, 4, &mut rng, &config));
        assert!(!should_stretch_syllable(1, 1, 4, &mut rng, &config));
        assert!(should_stretch_syllable(2, 2, 4, &mut rng, &config));
    }

    #[test]
    fn test_should_stretch_boundary() {
        let config = StretchConfig {
            boundary_stretch: Some(1),
            ..Default::default()
        };
        let mut rng = StdRng::seed_from_u64(42);
        assert!(should_stretch_syllable(0, 0, 3, &mut rng, &config)); // first
        assert!(!should_stretch_syllable(1, 1, 3, &mut rng, &config)); // middle
        assert!(should_stretch_syllable(2, 2, 3, &mut rng, &config)); // last
    }

    #[test]
    fn test_apply_stutter_no_stutter() {
        let mut rng = StdRng::seed_from_u64(42);
        let items = vec![1, 2, 3];
        let result = apply_stutter(&items, 0.0, (1, 1), &mut rng);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_apply_stutter_always() {
        let mut rng = StdRng::seed_from_u64(42);
        let items = vec![1, 2];
        let result = apply_stutter(&items, 1.0, (1, 1), &mut rng);
        assert!(result.len() > items.len());
    }

    #[test]
    fn test_stretch_config_default() {
        let config = StretchConfig::default();
        assert!(!config.has_syllable_stretch());
    }

    #[test]
    fn test_stretch_config_has_syllable_stretch() {
        let config = StretchConfig {
            random_stretch: Some(0.5),
            ..Default::default()
        };
        assert!(config.has_syllable_stretch());
    }
}
