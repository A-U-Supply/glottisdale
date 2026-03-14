//! Shuffle mode: template-based syllable collage.
//!
//! Uses each source's own transcription as a timing template, then fills
//! each syllable slot with phonetically-matched syllables from OTHER sources.
//! Produces gibberish that preserves natural speech rhythm and coarticulation.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, bail};

use crate::audio::io::write_wav;
use crate::speak::assembler::{assemble, plan_timing};
use crate::speak::matcher::match_syllables;
use crate::speak::phonetic_distance::normalize_phoneme;
use crate::speak::syllable_bank::{SyllableEntry, build_bank};
use crate::types::{PipelineResult, Syllable};

/// Run the shuffle-mode collage pipeline.
///
/// For each source, uses its syllable sequence as a timing template and
/// fills each slot with phonetically-matched syllables from other sources.
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

    // Determine sample rate from first source
    let sr = source_audio
        .values()
        .next()
        .map(|(_, sr)| *sr)
        .unwrap_or(16000);

    let mut all_output_samples: Vec<f64> = Vec::new();
    let mut total_duration = 0.0;
    let mut all_clips = Vec::new();

    for template_name in &source_names {
        if total_duration >= target_duration {
            break;
        }

        let template_syls = &source_syllables[template_name];
        if template_syls.is_empty() {
            continue;
        }

        // Build target: template's phoneme sequence
        let target_phonemes: Vec<Vec<String>> = template_syls
            .iter()
            .map(|syl| {
                syl.phonemes
                    .iter()
                    .filter(|p| !p.label.is_empty() && p.label.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false))
                    .map(|p| normalize_phoneme(&p.label))
                    .collect()
            })
            .filter(|v: &Vec<String>| !v.is_empty())
            .collect();

        if target_phonemes.is_empty() {
            continue;
        }

        // Build bank from all sources EXCEPT the template
        let mut bank: Vec<SyllableEntry> = Vec::new();
        for (name, syls) in source_syllables {
            if name == template_name {
                continue;
            }
            bank.extend(build_bank(syls, name));
        }

        if bank.is_empty() {
            continue;
        }

        // Extract stress info from template
        let target_stresses: Vec<Option<u8>> = template_syls
            .iter()
            .filter(|syl| {
                syl.phonemes
                    .iter()
                    .any(|p| !p.label.is_empty() && p.label.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false))
            })
            .map(|syl| {
                syl.phonemes
                    .iter()
                    .find_map(|p| {
                        p.label.as_bytes().last().and_then(|b| {
                            if b.is_ascii_digit() {
                                Some(b - b'0')
                            } else {
                                None
                            }
                        })
                    })
            })
            .collect();

        // Match using Viterbi DP
        let matches = match_syllables(
            &target_phonemes,
            &bank,
            Some(&target_stresses),
            None, // default continuity bonus
        );

        if matches.is_empty() {
            continue;
        }

        // Plan timing using template's original syllable timing as reference
        let filtered_template_syls: Vec<&Syllable> = template_syls
            .iter()
            .filter(|syl| {
                syl.phonemes
                    .iter()
                    .any(|p| !p.label.is_empty() && p.label.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false))
            })
            .collect();

        let reference_timings: Vec<(f64, f64)> = filtered_template_syls
            .iter()
            .map(|syl| (syl.start, syl.end))
            .collect();

        // Compute word boundaries from template
        let mut word_boundaries: Vec<usize> = Vec::new();
        for (i, syl) in filtered_template_syls.iter().enumerate() {
            if i == 0 {
                word_boundaries.push(0);
            } else if syl.word_index != filtered_template_syls[i - 1].word_index {
                word_boundaries.push(i);
            }
        }

        let avg_syl_dur: f64 = filtered_template_syls
            .iter()
            .map(|s| s.end - s.start)
            .sum::<f64>()
            / filtered_template_syls.len().max(1) as f64;

        let timing = plan_timing(
            &matches,
            &word_boundaries,
            avg_syl_dur,
            Some(&reference_timings),
            0.9, // high strictness — closely follow template timing
        );

        // Assemble audio
        let template_output = assemble(
            &matches,
            &timing,
            source_audio,
            output_dir,
            crossfade_ms,
            None,  // no manual pitch shifts
            true,  // normalize volume
            true,  // normalize pitch
        )?;

        // Read the assembled audio and append
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

    let manifest = serde_json::json!({
        "mode": "shuffle",
        "sources": source_names,
        "total_syllables": source_syllables.values().map(|s| s.len()).sum::<usize>(),
        "duration": all_output_samples.len() as f64 / sr as f64,
    });

    let manifest_path = output_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Ok(PipelineResult {
        clips: all_clips,
        concatenated: concatenated_path,
        transcript: String::new(),
        manifest,
    })
}
