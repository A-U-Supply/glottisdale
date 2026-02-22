//! Glottisdale collage pipeline — syllable-level audio collage engine.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, bail};
use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::seq::SliceRandom;

use crate::audio::analysis::{
    compute_rms, estimate_f0, find_breaths, find_room_tone, generate_pink_noise,
};
use crate::audio::effects::{
    adjust_volume, concatenate, cut_clip, generate_silence, mix_audio,
    pitch_shift, time_stretch,
};
use crate::audio::io::{read_wav, write_wav};
use crate::collage::stretch::{
    StretchConfig, apply_stutter, apply_word_repeat, parse_count_range,
    resolve_stretch_factor, should_stretch_syllable,
};
use crate::language::phonotactics::order_syllables;
use crate::types::{Clip, PipelineResult, Syllable};

/// Default weights for syllables-per-word: favors 2-syllable words.
const WORD_LENGTH_WEIGHTS: &[f64] = &[0.30, 0.35, 0.25, 0.10];

/// Parse range string like "1-5" or "3" into (min, max).
fn parse_range(s: &str) -> (usize, usize) {
    if let Some(idx) = s.find('-') {
        if let (Ok(a), Ok(b)) = (s[..idx].parse(), s[idx + 1..].parse()) {
            return (a, b);
        }
    }
    let val: usize = s.parse().unwrap_or(1);
    (val, val)
}

/// Parse gap string like "50-200" or "100" into (min_ms, max_ms).
fn parse_gap(s: &str) -> (f64, f64) {
    if let Some(idx) = s.find('-') {
        if let (Ok(a), Ok(b)) = (s[..idx].parse(), s[idx + 1..].parse()) {
            return (a, b);
        }
    }
    let val: f64 = s.parse().unwrap_or(100.0);
    (val, val)
}

/// Pick a word length using weighted distribution.
fn weighted_word_length(min_syl: usize, max_syl: usize, rng: &mut StdRng) -> usize {
    let choices: Vec<usize> = (min_syl..=max_syl).collect();
    if choices.len() <= WORD_LENGTH_WEIGHTS.len() {
        let weights = &WORD_LENGTH_WEIGHTS[..choices.len()];
        let total: f64 = weights.iter().sum();
        let r: f64 = rng.gen::<f64>() * total;
        let mut cumulative = 0.0;
        for (i, &w) in weights.iter().enumerate() {
            cumulative += w;
            if r <= cumulative {
                return choices[i];
            }
        }
        return *choices.last().unwrap();
    }
    rng.gen_range(min_syl..=max_syl)
}

/// Group syllables into variable-length words with phonotactic ordering.
fn group_into_words(
    syllables: &[Syllable],
    spc_min: usize,
    spc_max: usize,
    rng: &mut StdRng,
) -> Vec<Vec<Syllable>> {
    let mut words = Vec::new();
    let mut i = 0;
    while i < syllables.len() {
        let word_len = weighted_word_length(spc_min, spc_max, rng);
        let end = (i + word_len).min(syllables.len());
        let mut word: Vec<Syllable> = syllables[i..end].to_vec();
        if !word.is_empty() {
            if word.len() > 1 {
                let seed = rng.gen_range(0u64..=u64::MAX);
                word = order_syllables(&word, Some(seed), 100);
            }
            words.push(word);
        }
        i = end;
    }
    words
}

/// Group items into variable-length groups.
fn group_into_chunks<T: Clone>(items: &[T], min_len: usize, max_len: usize, rng: &mut StdRng) -> Vec<Vec<T>> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let len = rng.gen_range(min_len..=max_len);
        let end = (i + len).min(items.len());
        let chunk: Vec<T> = items[i..end].to_vec();
        if !chunk.is_empty() {
            groups.push(chunk);
        }
        i = end;
    }
    groups
}

/// Sample and shuffle syllables to approximately hit target duration.
fn sample_syllables(syllables: &[Syllable], target_duration: f64, rng: &mut StdRng) -> Vec<Syllable> {
    if syllables.is_empty() {
        return Vec::new();
    }

    let mut available: Vec<Syllable> = syllables.to_vec();
    available.shuffle(rng);

    let mut selected = Vec::new();
    let mut total = 0.0;
    for syl in available {
        let syl_dur = syl.end - syl.start;
        if total + syl_dur > target_duration && !selected.is_empty() {
            break;
        }
        total += syl_dur;
        selected.push(syl);
    }

    selected.shuffle(rng);
    selected
}

/// Round-robin sample across sources for variety, then shuffle.
fn sample_syllables_multi_source(
    sources: &HashMap<String, Vec<Syllable>>,
    target_duration: f64,
    rng: &mut StdRng,
) -> Vec<Syllable> {
    if sources.is_empty() {
        return Vec::new();
    }

    let mut pools: HashMap<String, Vec<Syllable>> = HashMap::new();
    for (name, syls) in sources {
        let mut pool = syls.clone();
        pool.shuffle(rng);
        pools.insert(name.clone(), pool);
    }

    let mut selected = Vec::new();
    let mut total = 0.0;
    let source_names: Vec<String> = pools.keys().cloned().collect();

    'outer: loop {
        let mut any_remaining = false;
        for name in &source_names {
            if let Some(pool) = pools.get_mut(name) {
                if let Some(syl) = pool.pop() {
                    any_remaining = true;
                    let syl_dur = syl.end - syl.start;
                    total += syl_dur;
                    selected.push(syl);
                    if total >= target_duration {
                        break 'outer;
                    }
                }
            }
        }
        if !any_remaining {
            break;
        }
    }

    selected.shuffle(rng);
    selected
}

/// Configuration for the collage pipeline.
#[derive(Debug, Clone)]
pub struct CollageConfig {
    pub syllables_per_clip: String,
    pub target_duration: f64,
    pub crossfade_ms: f64,
    pub padding_ms: f64,
    pub words_per_phrase: String,
    pub phrases_per_sentence: String,
    pub phrase_pause: String,
    pub sentence_pause: String,
    pub word_crossfade_ms: f64,
    pub seed: Option<u64>,
    // Audio polish
    pub noise_level_db: f64,
    pub room_tone: bool,
    pub pitch_normalize: bool,
    pub pitch_range: f64,
    pub breaths: bool,
    pub breath_probability: f64,
    pub volume_normalize: bool,
    pub prosodic_dynamics: bool,
    // Stretch
    pub speed: Option<f64>,
    pub stretch_config: StretchConfig,
    // Repeat
    pub repeat_weight: Option<f64>,
    pub repeat_count: String,
    pub repeat_style: String,
    // Stutter
    pub stutter: Option<f64>,
    pub stutter_count: String,
}

impl Default for CollageConfig {
    fn default() -> Self {
        Self {
            syllables_per_clip: "1-5".to_string(),
            target_duration: 10.0,
            crossfade_ms: 30.0,
            padding_ms: 25.0,
            words_per_phrase: "3-5".to_string(),
            phrases_per_sentence: "2-3".to_string(),
            phrase_pause: "400-700".to_string(),
            sentence_pause: "800-1200".to_string(),
            word_crossfade_ms: 50.0,
            seed: None,
            noise_level_db: -40.0,
            room_tone: true,
            pitch_normalize: true,
            pitch_range: 5.0,
            breaths: true,
            breath_probability: 0.6,
            volume_normalize: true,
            prosodic_dynamics: true,
            speed: None,
            stretch_config: StretchConfig::default(),
            repeat_weight: None,
            repeat_count: "1-2".to_string(),
            repeat_style: "exact".to_string(),
            stutter: None,
            stutter_count: "1-2".to_string(),
        }
    }
}

/// Normalize volume across clips to median RMS (in-memory).
fn normalize_volume_clips(clips: &mut [Vec<f64>]) {
    let rms_values: Vec<f64> = clips
        .iter()
        .map(|c| compute_rms(c))
        .filter(|&r| r > 1e-6)
        .collect();

    if rms_values.is_empty() {
        return;
    }

    let mut sorted = rms_values.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let target_rms = sorted[sorted.len() / 2];

    if target_rms < 1e-6 {
        return;
    }

    for clip in clips.iter_mut() {
        let clip_rms = compute_rms(clip);
        if clip_rms < 1e-6 {
            continue;
        }
        let db_adjust = 20.0 * (target_rms / clip_rms).log10();
        let db_adjust = db_adjust.clamp(-20.0, 20.0);
        if db_adjust.abs() >= 0.5 {
            adjust_volume(clip, db_adjust);
        }
    }
}

/// Normalize pitch across clips toward median F0 (in-memory).
fn normalize_pitch_clips(clips: &mut [Vec<f64>], sr: u32, pitch_range: f64) {
    let f0_values: Vec<(usize, f64)> = clips
        .iter()
        .enumerate()
        .filter_map(|(i, c)| estimate_f0(c, sr, 80, 600).map(|f0| (i, f0)))
        .collect();

    if f0_values.is_empty() {
        return;
    }

    let mut sorted_f0s: Vec<f64> = f0_values.iter().map(|(_, f0)| *f0).collect();
    sorted_f0s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let target_f0 = sorted_f0s[sorted_f0s.len() / 2];

    log::info!(
        "Pitch normalization: target F0 = {:.1}Hz (from {} voiced clips)",
        target_f0,
        f0_values.len()
    );

    for (i, f0) in &f0_values {
        let semitones_shift = 12.0 * (target_f0 / f0).log2();
        let semitones_shift = semitones_shift.clamp(-pitch_range, pitch_range);
        if semitones_shift.abs() >= 0.1 {
            if let Ok(shifted) = pitch_shift(&clips[*i], sr, semitones_shift) {
                clips[*i] = shifted;
            }
        }
    }
}

/// Apply prosodic dynamics to a clip: slight boost at start, taper at end.
fn apply_prosodic_dynamics(samples: &mut [f64], sr: u32) {
    let len = samples.len();
    let dur = len as f64 / sr as f64;
    if dur <= 0.3 {
        return;
    }

    // Slight boost (1.12 dB) in first 20%
    let boost_ratio = 10.0f64.powf(1.12 / 20.0);
    let boost_end = (len as f64 * 0.2) as usize;
    for s in samples[..boost_end].iter_mut() {
        *s *= boost_ratio;
    }

    // Taper (-3 dB) from 70% onward
    let fade_ratio = 10.0f64.powf(-3.0 / 20.0);
    let fade_start = (len as f64 * 0.7) as usize;
    for s in samples[fade_start..].iter_mut() {
        *s *= fade_ratio;
    }
}

/// Run the full collage pipeline.
///
/// Takes pre-aligned syllables per source (from an external alignment step)
/// and the loaded audio samples. This function handles sampling, grouping,
/// effects, and assembly.
pub fn process(
    source_audio: &HashMap<String, (Vec<f64>, u32)>,
    source_syllables: &HashMap<String, Vec<Syllable>>,
    output_dir: &Path,
    config: &CollageConfig,
) -> Result<PipelineResult> {
    let mut rng = match config.seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    std::fs::create_dir_all(output_dir)?;
    let clips_dir = output_dir.join("clips");
    std::fs::create_dir_all(&clips_dir)?;

    let (spc_min, spc_max) = parse_range(&config.syllables_per_clip);
    let (wpp_min, wpp_max) = parse_range(&config.words_per_phrase);
    let (pps_min, pps_max) = parse_range(&config.phrases_per_sentence);
    let (pp_min, pp_max) = parse_gap(&config.phrase_pause);
    let (sp_min, sp_max) = parse_gap(&config.sentence_pause);

    let stutter_count_range = if config.stutter.is_some() {
        Some(parse_count_range(&config.stutter_count))
    } else {
        None
    };
    let repeat_count_range = if config.repeat_weight.is_some() {
        Some(parse_count_range(&config.repeat_count))
    } else {
        None
    };

    // Determine sample rate from first source
    let sr = source_audio
        .values()
        .next()
        .map(|(_, sr)| *sr)
        .unwrap_or(16000);

    // --- Audio polish: extract room tone and breaths ---
    let mut room_tone_samples: HashMap<String, Vec<f64>> = HashMap::new();
    let mut breath_clips: Vec<Vec<f64>> = Vec::new();

    for (source_name, (samples, sample_rate)) in source_audio {
        if config.room_tone {
            if let Some((rt_start, rt_end)) = find_room_tone(samples, *sample_rate, 500) {
                let start_idx = (rt_start * *sample_rate as f64) as usize;
                let end_idx = (rt_end * *sample_rate as f64) as usize;
                if end_idx > start_idx && end_idx <= samples.len() {
                    room_tone_samples
                        .insert(source_name.clone(), samples[start_idx..end_idx].to_vec());
                    log::info!(
                        "Room tone found in {}: {:.1}-{:.1}s",
                        source_name,
                        rt_start,
                        rt_end
                    );
                }
            }
        }

        if config.breaths {
            if let Some(syls) = source_syllables.get(source_name) {
                // Build word-level boundaries
                let mut word_bounds: Vec<(f64, f64)> = Vec::new();
                let mut seen_words: std::collections::HashSet<(String, usize)> =
                    std::collections::HashSet::new();
                for syl in syls {
                    let key = (syl.word.clone(), syl.word_index);
                    if seen_words.insert(key) {
                        let word_syls: Vec<&Syllable> = syls
                            .iter()
                            .filter(|s| s.word == syl.word && s.word_index == syl.word_index)
                            .collect();
                        if let (Some(start), Some(end)) = (
                            word_syls.iter().map(|s| s.start).min_by(|a, b| a.partial_cmp(b).unwrap()),
                            word_syls.iter().map(|s| s.end).max_by(|a, b| a.partial_cmp(b).unwrap()),
                        ) {
                            word_bounds.push((start, end));
                        }
                    }
                }
                word_bounds.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

                let detected = find_breaths(samples, *sample_rate, &word_bounds, 100, 1000);
                if !detected.is_empty() {
                    for (bs, be) in &detected {
                        let start_idx = (*bs * *sample_rate as f64) as usize;
                        let end_idx = (*be * *sample_rate as f64) as usize;
                        if end_idx > start_idx && end_idx <= samples.len() {
                            breath_clips.push(samples[start_idx..end_idx].to_vec());
                        }
                    }
                    log::info!("Found {} breaths in {}", detected.len(), source_name);
                }
            }
        }
    }

    // --- Sample syllables across sources ---
    let selected = if source_syllables.len() == 1 {
        let syls = source_syllables.values().next().unwrap();
        sample_syllables(syls, config.target_duration, &mut rng)
    } else {
        sample_syllables_multi_source(source_syllables, config.target_duration, &mut rng)
    };

    // Helper: find which source a syllable came from
    let find_source = |syl: &Syllable| -> String {
        for (name, syls) in source_syllables {
            if syls.iter().any(|s| std::ptr::eq(s, syl) || (s.start == syl.start && s.end == syl.end && s.word == syl.word && s.word_index == syl.word_index)) {
                return name.clone();
            }
        }
        "unknown".to_string()
    };

    // --- Group syllables into words ---
    let words = group_into_words(&selected, spc_min, spc_max, &mut rng);

    // --- Cut all syllable clips ---
    struct SylClipInfo {
        word_idx: usize,
        syl_idx: usize,
        samples: Vec<f64>,
        syl: Syllable,
    }

    let mut all_syl_clips: Vec<SylClipInfo> = Vec::new();

    for (word_idx, word_syls) in words.iter().enumerate() {
        for (syl_idx, syl) in word_syls.iter().enumerate() {
            let syl_source = find_source(syl);
            if let Some((source_samples, source_sr)) = source_audio.get(&syl_source) {
                let clip = cut_clip(
                    source_samples,
                    *source_sr,
                    syl.start,
                    syl.end,
                    config.padding_ms,
                    0.0,
                );
                if !clip.is_empty() {
                    all_syl_clips.push(SylClipInfo {
                        word_idx,
                        syl_idx,
                        samples: clip,
                        syl: syl.clone(),
                    });
                }
            }
        }
    }

    // --- Pitch normalization ---
    if config.pitch_normalize && !all_syl_clips.is_empty() {
        let mut clip_samples: Vec<Vec<f64>> =
            all_syl_clips.iter().map(|c| c.samples.clone()).collect();
        normalize_pitch_clips(&mut clip_samples, sr, config.pitch_range);
        for (i, samples) in clip_samples.into_iter().enumerate() {
            all_syl_clips[i].samples = samples;
        }
    }

    // --- Volume normalization ---
    if config.volume_normalize && !all_syl_clips.is_empty() {
        let mut clip_samples: Vec<Vec<f64>> =
            all_syl_clips.iter().map(|c| c.samples.clone()).collect();
        normalize_volume_clips(&mut clip_samples);
        for (i, samples) in clip_samples.into_iter().enumerate() {
            all_syl_clips[i].samples = samples;
        }
    }

    // --- Stutter ---
    if let Some(stutter_prob) = config.stutter {
        if let Some(count_range) = stutter_count_range {
            for word_idx in 0..words.len() {
                let word_clips: Vec<usize> = all_syl_clips
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.word_idx == word_idx)
                    .map(|(i, _)| i)
                    .collect();

                if !word_clips.is_empty() {
                    let clip_refs: Vec<Vec<f64>> =
                        word_clips.iter().map(|&i| all_syl_clips[i].samples.clone()).collect();
                    let stuttered = apply_stutter(&clip_refs, stutter_prob, count_range, &mut rng);
                    // Update the clips - remove old word entries and add stuttered ones
                    let syl = all_syl_clips[word_clips[0]].syl.clone();
                    // Remove old entries for this word (in reverse order to maintain indices)
                    for &i in word_clips.iter().rev() {
                        all_syl_clips.remove(i);
                    }
                    // Add stuttered entries
                    for (syl_idx, samples) in stuttered.into_iter().enumerate() {
                        all_syl_clips.push(SylClipInfo {
                            word_idx,
                            syl_idx,
                            samples,
                            syl: syl.clone(),
                        });
                    }
                }
            }
        }
    }

    // --- Syllable stretch ---
    if config.stretch_config.has_syllable_stretch() {
        let mut global_syl_idx = 0usize;
        for word_idx in 0..words.len() {
            let word_clips: Vec<usize> = all_syl_clips
                .iter()
                .enumerate()
                .filter(|(_, c)| c.word_idx == word_idx)
                .map(|(i, _)| i)
                .collect();

            for &i in &word_clips {
                let clip_dur = all_syl_clips[i].samples.len() as f64 / sr as f64;
                if clip_dur >= 0.08 {
                    let syl_idx = all_syl_clips[i].syl_idx;
                    if should_stretch_syllable(
                        global_syl_idx,
                        syl_idx,
                        word_clips.len(),
                        &mut rng,
                        &config.stretch_config,
                    ) {
                        let factor = resolve_stretch_factor(
                            config.stretch_config.stretch_factor,
                            &mut rng,
                        );
                        all_syl_clips[i].samples = time_stretch(&all_syl_clips[i].samples, sr, factor)?;
                    }
                }
                global_syl_idx += 1;
            }
        }
    }

    // --- Fuse syllables into words ---
    let crossfade_samples = (config.crossfade_ms / 1000.0 * sr as f64).round() as usize;
    let mut clips: Vec<Clip> = Vec::new();
    let mut word_audio: Vec<Vec<f64>> = Vec::new();

    for (word_idx, word_syls) in words.iter().enumerate() {
        let syl_clips: Vec<&Vec<f64>> = all_syl_clips
            .iter()
            .filter(|c| c.word_idx == word_idx)
            .map(|c| &c.samples)
            .collect();

        if syl_clips.is_empty() {
            continue;
        }

        let word_samples = if syl_clips.len() == 1 {
            syl_clips[0].clone()
        } else {
            let owned: Vec<Vec<f64>> = syl_clips.iter().map(|c| c.to_vec()).collect();
            concatenate(&owned, crossfade_samples)
        };

        // Write word clip to clips_dir
        let word_filename = format!("{:03}_word.wav", word_idx + 1);
        let word_output = clips_dir.join(&word_filename);
        write_wav(&word_output, &word_samples, sr)?;

        // Determine dominant source
        let word_sources: Vec<String> = word_syls.iter().map(&find_source).collect();
        let dominant = word_sources
            .iter()
            .max_by_key(|s| word_sources.iter().filter(|t| *t == *s).count())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        clips.push(Clip {
            syllables: word_syls.clone(),
            start: word_syls.iter().map(|s| s.start).fold(f64::INFINITY, f64::min),
            end: word_syls.iter().map(|s| s.end).fold(f64::NEG_INFINITY, f64::max),
            source: dominant,
            output_path: word_output,
        });
        word_audio.push(word_samples);
    }

    // --- Word stretch ---
    if let Some(word_stretch_prob) = config.stretch_config.word_stretch {
        for (i, samples) in word_audio.iter_mut().enumerate() {
            let clip_dur = samples.len() as f64 / sr as f64;
            if clip_dur >= 0.08 && rng.gen::<f64>() < word_stretch_prob {
                let factor = resolve_stretch_factor(config.stretch_config.stretch_factor, &mut rng);
                *samples = time_stretch(samples, sr, factor)?;
                // Re-write the word file
                if let Err(e) = write_wav(&clips[i].output_path, samples, sr) {
                    log::debug!("Failed to rewrite stretched word: {}", e);
                }
            }
        }
    }

    // --- Word repeat ---
    if let Some(repeat_prob) = config.repeat_weight {
        if let Some(count_range) = repeat_count_range {
            clips = apply_word_repeat(&clips, repeat_prob, count_range, &config.repeat_style, &mut rng);
        }
    }

    // --- Group into phrases ---
    let word_cf_samples = (config.word_crossfade_ms / 1000.0 * sr as f64).round() as usize;
    let phrase_groups = group_into_chunks(&clips, wpp_min, wpp_max, &mut rng);

    let mut phrase_audio: Vec<Vec<f64>> = Vec::new();
    for phrase_clips in &phrase_groups {
        // Load word audio for each clip in phrase
        let mut phrase_word_samples: Vec<Vec<f64>> = Vec::new();
        for clip in phrase_clips {
            if clip.output_path.exists() {
                if let Ok((samples, _)) = read_wav(&clip.output_path) {
                    phrase_word_samples.push(samples);
                }
            }
        }

        if phrase_word_samples.is_empty() {
            continue;
        }

        let phrase = if phrase_word_samples.len() == 1 {
            phrase_word_samples.into_iter().next().unwrap()
        } else {
            concatenate(&phrase_word_samples, word_cf_samples)
        };

        phrase_audio.push(phrase);
    }

    // --- Prosodic dynamics ---
    if config.prosodic_dynamics {
        for phrase in phrase_audio.iter_mut() {
            apply_prosodic_dynamics(phrase, sr);
        }
    }

    // --- Group phrases into sentences, compute gaps ---
    let sentence_groups = group_into_chunks(
        &(0..phrase_audio.len()).collect::<Vec<_>>(),
        pps_min,
        pps_max,
        &mut rng,
    );

    let mut ordered_phrases: Vec<&Vec<f64>> = Vec::new();
    let mut gap_durations: Vec<f64> = Vec::new();
    let mut gap_types: Vec<&str> = Vec::new();

    for (sent_idx, sent_phrase_indices) in sentence_groups.iter().enumerate() {
        for (i, &phrase_idx) in sent_phrase_indices.iter().enumerate() {
            if phrase_idx < phrase_audio.len() {
                ordered_phrases.push(&phrase_audio[phrase_idx]);

                let is_last_in_sentence = i == sent_phrase_indices.len() - 1;
                let is_last_sentence = sent_idx == sentence_groups.len() - 1;

                if !(is_last_in_sentence && is_last_sentence) {
                    if is_last_in_sentence {
                        gap_durations.push(rng.gen_range(sp_min..=sp_max));
                        gap_types.push("sentence");
                    } else {
                        gap_durations.push(rng.gen_range(pp_min..=pp_max));
                        gap_types.push("phrase");
                    }
                }
            }
        }
    }

    // --- Build gap clips (room tone or silence, optionally with breaths) ---
    let mut final_clips: Vec<Vec<f64>> = Vec::new();
    let room_tone_list: Vec<&Vec<f64>> = room_tone_samples.values().collect();

    for (i, phrase) in ordered_phrases.iter().enumerate() {
        final_clips.push(phrase.to_vec());

        if i < gap_durations.len() {
            let gap_ms = gap_durations[i];
            let mut gap_clip = generate_silence(gap_ms, sr);

            // Mix room tone into gap if available
            if !room_tone_list.is_empty() {
                let rt = room_tone_list[i % room_tone_list.len()];
                gap_clip = mix_audio(&gap_clip, rt, 0.0);
            }

            // Optionally prepend breath at phrase boundaries
            if !breath_clips.is_empty()
                && i < gap_types.len()
                && gap_types[i] == "phrase"
                && rng.gen::<f64>() < config.breath_probability
            {
                let breath = breath_clips[rng.gen_range(0..breath_clips.len())].clone();
                let breath_and_gap = vec![breath, gap_clip];
                gap_clip = concatenate(&breath_and_gap, (10.0 / 1000.0 * sr as f64).round() as usize);
            }

            final_clips.push(gap_clip);
        }
    }

    // --- Final concatenation ---
    let mut output_samples = if final_clips.len() > 1 {
        concatenate(&final_clips, 0)
    } else if final_clips.len() == 1 {
        final_clips.into_iter().next().unwrap()
    } else {
        bail!("No audio clips to concatenate");
    };

    // --- Global speed ---
    if let Some(speed) = config.speed {
        let speed_factor = 1.0 / speed;
        output_samples = time_stretch(&output_samples, sr, speed_factor)?;
    }

    // --- Mix pink noise bed ---
    if config.noise_level_db != 0.0 && !output_samples.is_empty() {
        let dur = output_samples.len() as f64 / sr as f64;
        let noise = generate_pink_noise(dur, sr, config.seed);
        output_samples = mix_audio(&output_samples, &noise, config.noise_level_db);
    }

    // --- Write output ---
    let run_name = output_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let concatenated_path = output_dir.join(format!("{}.wav", run_name));
    write_wav(&concatenated_path, &output_samples, sr)?;

    // --- Write manifest ---
    let manifest = serde_json::json!({
        "sources": source_syllables.keys().collect::<Vec<_>>(),
        "total_syllables": source_syllables.values().map(|s| s.len()).sum::<usize>(),
        "selected_syllables": selected.len(),
        "clips": clips.iter().map(|c| {
            serde_json::json!({
                "filename": c.output_path.file_name().unwrap_or_default().to_string_lossy(),
                "source": c.source,
                "word": c.syllables.first().map(|s| s.word.as_str()).unwrap_or(""),
                "start": c.start,
                "end": c.end,
            })
        }).collect::<Vec<_>>(),
    });

    let manifest_path = output_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Ok(PipelineResult {
        clips,
        concatenated: concatenated_path,
        transcript: source_syllables
            .keys()
            .map(|k| format!("[{}]", k))
            .collect::<Vec<_>>()
            .join("\n"),
        manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range() {
        assert_eq!(parse_range("1-5"), (1, 5));
        assert_eq!(parse_range("3"), (3, 3));
    }

    #[test]
    fn test_parse_gap() {
        assert_eq!(parse_gap("50-200"), (50.0, 200.0));
        assert_eq!(parse_gap("100"), (100.0, 100.0));
    }

    #[test]
    fn test_weighted_word_length() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let len = weighted_word_length(1, 4, &mut rng);
            assert!(len >= 1 && len <= 4);
        }
    }

    #[test]
    fn test_sample_syllables_empty() {
        let mut rng = StdRng::seed_from_u64(42);
        assert!(sample_syllables(&[], 10.0, &mut rng).is_empty());
    }

    #[test]
    fn test_sample_syllables_basic() {
        let mut rng = StdRng::seed_from_u64(42);
        let syls: Vec<Syllable> = (0..10)
            .map(|i| Syllable {
                phonemes: vec![],
                start: i as f64 * 0.3,
                end: i as f64 * 0.3 + 0.3,
                word: format!("w{}", i),
                word_index: i,
            })
            .collect();
        let selected = sample_syllables(&syls, 1.0, &mut rng);
        assert!(!selected.is_empty());
        let total_dur: f64 = selected.iter().map(|s| s.end - s.start).sum();
        assert!(total_dur <= 2.0); // Approximately target + one syllable
    }

    #[test]
    fn test_group_into_words() {
        let mut rng = StdRng::seed_from_u64(42);
        let syls: Vec<Syllable> = (0..10)
            .map(|i| Syllable {
                phonemes: vec![],
                start: i as f64 * 0.3,
                end: i as f64 * 0.3 + 0.3,
                word: format!("w{}", i),
                word_index: i,
            })
            .collect();
        let words = group_into_words(&syls, 1, 3, &mut rng);
        assert!(!words.is_empty());
        let total: usize = words.iter().map(|w| w.len()).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn test_group_into_chunks() {
        let mut rng = StdRng::seed_from_u64(42);
        let items: Vec<i32> = (0..10).collect();
        let groups = group_into_chunks(&items, 2, 4, &mut rng);
        assert!(!groups.is_empty());
        let total: usize = groups.iter().map(|g| g.len()).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn test_apply_prosodic_dynamics() {
        let sr = 16000u32;
        let dur = 1.0; // 1 second
        let len = (dur * sr as f64) as usize;
        let mut samples = vec![0.5; len];
        let original = samples.clone();
        apply_prosodic_dynamics(&mut samples, sr);

        // First 20% should be boosted
        assert!(samples[0] > original[0]);

        // Last 30% should be attenuated
        let fade_start = (len as f64 * 0.7) as usize;
        assert!(samples[fade_start] < original[fade_start]);
    }

    #[test]
    fn test_collage_config_default() {
        let config = CollageConfig::default();
        assert_eq!(config.target_duration, 10.0);
        assert_eq!(config.crossfade_ms, 30.0);
        assert!(config.seed.is_none());
    }
}
