//! Shuffle mode: template-based syllable collage.
//!
//! Uses each source's own transcription as a timing template, then fills
//! each syllable slot with phonetically-matched syllables from OTHER sources.
//! Produces gibberish that preserves natural speech rhythm and coarticulation.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Result, bail};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::audio::analysis::{compute_rms, estimate_f0};
use crate::audio::effects::{concatenate, cut_clip, time_stretch};
use crate::audio::io::write_wav;
use crate::speak::matcher::MatchResult;
use crate::speak::phonetic_distance::{normalize_phoneme, syllable_distance};
use crate::speak::syllable_bank::{SyllableEntry, build_bank};
use crate::types::{PipelineResult, Syllable};

/// Number of top candidates to consider when randomly picking a match.
const TOP_N: usize = 5;

/// Crossfade between syllables in output (ms).
const SYLLABLE_CROSSFADE_MS: f64 = 15.0;

/// Run the shuffle-mode collage pipeline.
pub fn process_shuffle(
    source_audio: &HashMap<String, (Vec<f64>, u32)>,
    source_syllables: &HashMap<String, Vec<Syllable>>,
    output_dir: &Path,
    target_duration: f64,
    crossfade_ms: f64,
) -> Result<PipelineResult> {
    if source_syllables.len() < 2 {
        bail!("Shuffle mode requires at least 2 source files");
    }

    std::fs::create_dir_all(output_dir)?;

    let source_names: Vec<String> = source_syllables.keys().cloned().collect();
    let mut rng = StdRng::from_entropy();

    // Determine sample rate from first source
    let sr = source_audio
        .values()
        .next()
        .map(|(_, sr)| *sr)
        .unwrap_or(16000);

    // Filter syllables: reject too-long, too-short, silent, and non-speech pitch
    let mut filtered_sources: HashMap<String, Vec<Syllable>> = HashMap::new();
    for (name, syls) in source_syllables {
        let audio = source_audio.get(name);
        let filtered: Vec<Syllable> = syls
            .iter()
            .filter(|syl| {
                let dur = syl.end - syl.start;
                if dur < 0.05 || dur > 0.8 {
                    return false;
                }
                if let Some((samples, sample_rate)) = audio {
                    let start_idx = (syl.start * *sample_rate as f64) as usize;
                    let end_idx = (syl.end * *sample_rate as f64) as usize;
                    if start_idx < end_idx && end_idx <= samples.len() {
                        let clip = &samples[start_idx..end_idx];
                        if compute_rms(clip) < 0.005 {
                            return false;
                        }
                        if let Some(f0) = estimate_f0(clip, *sample_rate, 80, 600) {
                            if f0 < 100.0 {
                                return false;
                            }
                        }
                    }
                }
                true
            })
            .cloned()
            .collect();
        if !filtered.is_empty() {
            filtered_sources.insert(name.clone(), filtered);
        }
    }

    let total_before: usize = source_syllables.values().map(|s| s.len()).sum();
    let total_after: usize = filtered_sources.values().map(|s| s.len()).sum();
    log::info!(
        "Shuffle syllable filter: {}/{} passed (rejected {})",
        total_after,
        total_before,
        total_before - total_after,
    );

    if filtered_sources.len() < 2 {
        bail!("Not enough sources with valid syllables after filtering (need at least 2)");
    }

    let cf = if crossfade_ms > 0.0 { crossfade_ms } else { SYLLABLE_CROSSFADE_MS };
    let crossfade_samples = (cf / 1000.0 * sr as f64).round() as usize;

    let mut all_output_samples: Vec<f64> = Vec::new();
    let mut total_dur = 0.0;
    let mut total_matched = 0usize;

    for template_name in &source_names {
        if total_dur >= target_duration {
            break;
        }

        let template_syls = match filtered_sources.get(template_name) {
            Some(syls) => syls,
            None => continue,
        };
        if template_syls.is_empty() {
            continue;
        }

        // Filter template to syllables with real phonemes
        let filtered_template: Vec<&Syllable> = template_syls
            .iter()
            .filter(|syl| {
                syl.phonemes
                    .iter()
                    .any(|p| !p.label.is_empty() && p.label.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false))
            })
            .collect();

        if filtered_template.is_empty() {
            continue;
        }

        let target_phonemes: Vec<Vec<String>> = filtered_template
            .iter()
            .map(|syl| {
                syl.phonemes
                    .iter()
                    .filter(|p| !p.label.is_empty() && p.label.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false))
                    .map(|p| normalize_phoneme(&p.label))
                    .collect()
            })
            .collect();

        // Build bank from all sources EXCEPT the template
        let mut bank: Vec<SyllableEntry> = Vec::new();
        for (name, syls) in &filtered_sources {
            if name == template_name {
                continue;
            }
            bank.extend(build_bank(syls, name));
        }

        if bank.is_empty() {
            continue;
        }

        // Randomized top-N matching with reuse prevention
        let mut used: HashSet<usize> = HashSet::new();
        let matches: Vec<(MatchResult, f64)> = target_phonemes
            .iter()
            .enumerate()
            .map(|(target_idx, target)| {
                let template_dur = filtered_template[target_idx].end - filtered_template[target_idx].start;

                let mut scored: Vec<(usize, i32)> = bank
                    .iter()
                    .enumerate()
                    .map(|(j, entry)| {
                        let dist = syllable_distance(target, &entry.phoneme_labels);
                        let reuse_penalty = if used.contains(&j) { 20 } else { 0 };
                        (j, dist + reuse_penalty)
                    })
                    .collect();

                scored.sort_by_key(|(_, d)| *d);

                let top: Vec<(usize, i32)> = scored.into_iter().take(TOP_N).collect();
                let &(chosen_idx, dist) = top.choose(&mut rng).unwrap();

                used.insert(chosen_idx);

                let m = MatchResult {
                    target_phonemes: target.clone(),
                    entry: bank[chosen_idx].clone(),
                    distance: dist,
                    target_index: target_idx,
                };
                (m, template_dur)
            })
            .collect();

        total_matched += matches.len();

        // Direct assembly: cut each syllable, stretch to template duration, concatenate tightly
        let mut syllable_clips: Vec<Vec<f64>> = Vec::new();

        for (m, template_dur) in &matches {
            let source_path = &m.entry.source_path;
            let (samples, sample_rate) = match source_audio.get(source_path) {
                Some(s) => s,
                None => continue,
            };

            let mut clip = cut_clip(
                samples,
                *sample_rate,
                m.entry.start,
                m.entry.end,
                5.0,
                3.0,
            );

            if clip.is_empty() {
                continue;
            }

            // Time-stretch to match template syllable duration
            let source_dur = m.entry.end - m.entry.start;
            if source_dur > 0.0 && *template_dur > 0.0 {
                let stretch = template_dur / source_dur;
                if (stretch - 1.0).abs() > 0.05 {
                    if let Ok(stretched) = time_stretch(&clip, *sample_rate, stretch) {
                        clip = stretched;
                    }
                }
            }

            syllable_clips.push(clip);
        }

        if syllable_clips.is_empty() {
            continue;
        }

        // Concatenate all syllables with tight crossfade — no gaps
        let template_audio = concatenate(&syllable_clips, crossfade_samples);
        let dur = template_audio.len() as f64 / sr as f64;
        total_dur += dur;
        all_output_samples.extend(template_audio);
    }

    if all_output_samples.is_empty() {
        bail!("Shuffle mode produced no output");
    }

    // Trim to target duration
    let max_samples = (target_duration * sr as f64) as usize;
    if all_output_samples.len() > max_samples {
        all_output_samples.truncate(max_samples);
    }

    // Write final output
    let run_name = output_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let concatenated_path = output_dir.join(format!("{}.wav", run_name));
    write_wav(&concatenated_path, &all_output_samples, sr)?;

    let output_duration = all_output_samples.len() as f64 / sr as f64;
    log::info!(
        "Shuffle output: {:.1}s, {} syllables matched across {} templates",
        output_duration,
        total_matched,
        source_names.len(),
    );

    let manifest = serde_json::json!({
        "mode": "shuffle",
        "sources": source_names,
        "total_syllables": total_after,
        "matched_syllables": total_matched,
        "duration": output_duration,
    });

    let manifest_path = output_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    println!("Selected {} clips", total_matched);

    Ok(PipelineResult {
        clips: Vec::new(),
        concatenated: concatenated_path,
        transcript: String::new(),
        manifest,
    })
}
