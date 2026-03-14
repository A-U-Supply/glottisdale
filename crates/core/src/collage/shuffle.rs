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
use crate::audio::io::write_wav;
use crate::speak::assembler::{assemble, plan_timing};
use crate::speak::matcher::MatchResult;
use crate::speak::phonetic_distance::{normalize_phoneme, syllable_distance};
use crate::speak::syllable_bank::{SyllableEntry, build_bank};
use crate::types::{PipelineResult, Syllable};

/// Number of top candidates to consider when randomly picking a match.
const TOP_N: usize = 5;

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

    let mut all_output_samples: Vec<f64> = Vec::new();
    let mut total_duration = 0.0;
    let mut total_matched = 0usize;

    for template_name in &source_names {
        if total_duration >= target_duration {
            break;
        }

        let template_syls = match filtered_sources.get(template_name) {
            Some(syls) => syls,
            None => continue,
        };
        if template_syls.is_empty() {
            continue;
        }

        // Build target: template's phoneme sequence (only syllables with real phonemes)
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
        let matches: Vec<MatchResult> = target_phonemes
            .iter()
            .enumerate()
            .map(|(target_idx, target)| {
                // Score all bank entries
                let mut scored: Vec<(usize, i32)> = bank
                    .iter()
                    .enumerate()
                    .map(|(j, entry)| {
                        let dist = syllable_distance(target, &entry.phoneme_labels);
                        // Penalize already-used entries
                        let reuse_penalty = if used.contains(&j) { 20 } else { 0 };
                        (j, dist + reuse_penalty)
                    })
                    .collect();

                // Sort by distance (ascending)
                scored.sort_by_key(|(_, d)| *d);

                // Pick randomly from top N candidates
                let top: Vec<(usize, i32)> = scored.into_iter().take(TOP_N).collect();
                let &(chosen_idx, dist) = top.choose(&mut rng).unwrap();

                used.insert(chosen_idx);

                MatchResult {
                    target_phonemes: target.clone(),
                    entry: bank[chosen_idx].clone(),
                    distance: dist,
                    target_index: target_idx,
                }
            })
            .collect();

        if matches.is_empty() {
            continue;
        }

        total_matched += matches.len();

        // Plan timing using template's original syllable timing as reference
        let reference_timings: Vec<(f64, f64)> = filtered_template
            .iter()
            .map(|syl| (syl.start, syl.end))
            .collect();

        let mut word_boundaries: Vec<usize> = Vec::new();
        for (i, syl) in filtered_template.iter().enumerate() {
            if i == 0 {
                word_boundaries.push(0);
            } else if syl.word_index != filtered_template[i - 1].word_index {
                word_boundaries.push(i);
            }
        }

        let avg_syl_dur: f64 = filtered_template
            .iter()
            .map(|s| s.end - s.start)
            .sum::<f64>()
            / filtered_template.len().max(1) as f64;

        let timing = plan_timing(
            &matches,
            &word_boundaries,
            avg_syl_dur,
            Some(&reference_timings),
            0.9,
        );

        // Assemble audio
        let template_output = assemble(
            &matches,
            &timing,
            source_audio,
            output_dir,
            crossfade_ms,
            None,
            true,
            true,
        )?;

        if template_output.exists() {
            let (samples, _sr) = crate::audio::io::read_wav(&template_output)?;
            let dur = samples.len() as f64 / sr as f64;
            total_duration += dur;
            all_output_samples.extend(samples);
        }
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
        output_duration, total_matched, source_names.len(),
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

    // Report via stdout for bot parsing
    println!("Selected {} clips", total_matched);

    Ok(PipelineResult {
        clips: Vec::new(),
        concatenated: concatenated_path,
        transcript: String::new(),
        manifest,
    })
}
