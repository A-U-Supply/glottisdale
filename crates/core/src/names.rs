//! Generate unique, memorable run names for glottisdale output directories.
//!
//! Names are speech/voice/music-themed adjective-noun pairs like
//! "breathy-bassoon" or "staccato-tenor".

use std::path::{Path, PathBuf};

use anyhow::Result;
use rand::seq::SliceRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;

/// Speech/voice/music-themed adjectives.
pub const ADJECTIVES: &[&str] = &[
    "acoustic", "airy", "alto", "angular", "arched", "arpeggio", "atonal",
    "baritone", "bellowing", "bluesy", "booming", "bowed", "brassy", "breathy",
    "bright", "brittle", "buzzing", "cadenced", "chanting", "chesty",
    "chiming", "choral", "chromatic", "clipped", "coarse", "contralto",
    "crooning", "dark", "deft", "detached", "diaphonic", "diffuse", "digital",
    "dissonant", "distant", "droning", "dulcet", "dynamic", "echoing",
    "eerie", "elegiac", "embouchured", "ethereal", "expressive", "fading",
    "falsetto", "fervent", "fiery", "flageolet", "flat", "flowing",
    "fluent", "fluttering", "forte", "fretted", "fugal", "full", "gapped",
    "ghostly", "glassy", "gliding", "glottal", "granular", "gravelly",
    "groovy", "growling", "guttural", "harmonic", "harsh", "heady",
    "hollow", "honeyed", "hooting", "hovering", "humming", "hushed",
    "husky", "hymnal", "idling", "intoned", "jagged", "jaunty", "jazzy",
    "keen", "keening", "keyed", "lamenting", "languid", "laryngeal",
    "legato", "light", "lilting", "liquid", "lisping", "looping", "loud",
    "low", "lulling", "lyric", "major", "marcato", "mellow", "melodic",
    "mezzo", "microtonal", "minor", "modal", "modulated", "monotone",
    "moody", "morphing", "muffled", "murmuring", "muted", "nasal",
    "nimble", "nodal", "octave", "offbeat", "open", "operatic", "overtone",
    "passing", "pastoral", "pealing", "pedal", "pentatonic", "percussive",
    "phased", "piping", "pitched", "pizzicato", "plaintive", "plucked",
    "plunging", "polyphonic", "portamento", "pressed", "pulsing",
    "pure", "quavering", "quiet", "rasping", "raw", "reedy", "resonant",
    "reverbed", "riffing", "ringing", "rising", "rolling", "roomy",
    "rough", "round", "rumbling", "rushing", "rustic", "scooped",
    "scratchy", "sharp", "shimmering", "shrill", "sibilant", "sighing",
    "silken", "silvery", "singing", "slapping", "sliding", "slurred",
    "smoky", "smooth", "snapping", "soaring", "soft", "solo", "somber",
    "sonorous", "soprano", "sotto", "sparse", "spectral", "staccato",
    "strident", "strumming", "subharmonic", "surging", "sustained",
    "swaying", "swelling", "syncopated", "tempered", "tenor", "thick",
    "thin", "throbbing", "throaty", "thundering", "tonal", "trembling",
    "tremolo", "trilling", "tuned", "twangy", "unison", "unvoiced",
    "uvular", "vaporous", "velar", "velvety", "vibrant", "vibrato",
    "vocal", "voiced", "voiceless", "wailing", "warm", "warped",
    "wavering", "wheezy", "whispering", "whistling", "whooping", "winding",
    "woody", "woozy", "yearning", "zesty",
];

/// Speech/voice/music-themed nouns.
pub const NOUNS: &[&str] = &[
    "accordion", "alto", "anthem", "aria", "arpeggio", "ballad",
    "banjo", "baritone", "bass", "bassoon", "bellow", "bolero",
    "bourdon", "breath", "bridge", "bugle", "cadence", "canon",
    "cantata", "canticle", "cello", "chant", "chorale", "chord",
    "chorus", "chromatic", "clarinet", "clavichord", "clef", "coda",
    "concerto", "cornet", "counterpoint", "crescendo", "crotchet",
    "cymbal", "descant", "diapason", "diminuendo", "dirge", "dissonance",
    "ditty", "drone", "drum", "drumroll", "duet", "dulcimer",
    "echo", "elegy", "ensemble", "epiglottis", "etude", "euphonium",
    "fanfare", "fermata", "fiddle", "fife", "finale", "flute",
    "fortissimo", "fugue", "gargle", "glockenspiel", "glissando",
    "glottis", "gong", "growl", "guitar", "harmonica", "harmony",
    "harp", "harpsichord", "hiccup", "horn", "howl", "hum",
    "hymn", "interlude", "jingle", "kazoo", "kettledrum", "keynote",
    "lament", "larynx", "legato", "lilt", "lullaby", "lute",
    "lyre", "madrigal", "mandolin", "marimba", "measure", "medley",
    "melody", "metronome", "minuet", "motif", "murmur", "nocturne",
    "oboe", "octave", "opera", "opus", "organ", "overture",
    "palate", "pharynx", "phrase", "pianissimo", "piano", "piccolo",
    "pitch", "polka", "prelude", "psaltery", "quaver", "quintet",
    "rasp", "rattle", "recital", "reed", "refrain", "requiem",
    "resonance", "rest", "rhapsody", "rhythm", "riff", "rondo",
    "samba", "scale", "scherzo", "semitone", "serenade", "shanty",
    "sigh", "siren", "snare", "solo", "sonata", "soprano",
    "stanza", "strum", "symphony", "syncopation", "tabla", "tambourine",
    "tempo", "tenor", "theremin", "timpani", "toccata", "tongue",
    "treble", "tremolo", "trill", "trio", "trombone", "trumpet",
    "tuba", "tune", "tuning", "ukulele", "undertone", "unison",
    "uvula", "verse", "vibrato", "viola", "violin", "vocal",
    "voice", "vowel", "waltz", "warble", "whisper", "whistle",
    "woodwind", "xylophone", "yodel", "zither",
    "barcarolle", "calliope", "carillon", "castrato", "chanson",
    "concertina", "contrabass", "fandango", "gavotte", "harmonium",
    "libretto", "mazurka", "oratorio", "recitative", "rubato",
    "sarabande", "solfege", "tarantella", "vibraphone", "bagpipe",
];

/// Generate an adjective-noun name like "breathy-bassoon".
///
/// If seed is provided, the name is deterministic.
pub fn generate_name(seed: Option<u64>) -> String {
    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };
    let adj = ADJECTIVES.choose(&mut rng).unwrap();
    let noun = NOUNS.choose(&mut rng).unwrap();
    format!("{}-{}", adj, noun)
}

/// Generate a run ID like "2026-02-19-breathy-bassoon".
pub fn generate_run_id(seed: Option<u64>) -> String {
    let today = chrono_today();
    let name = generate_name(seed);
    format!("{}-{}", today, name)
}

/// Create a unique run directory inside root.
///
/// If `run_name` is provided, it overrides the adjective-noun part
/// (date prefix is still added). Handles collisions by appending -2, -3, etc.
pub fn create_run_dir(
    root: &Path,
    seed: Option<u64>,
    run_name: Option<&str>,
) -> Result<PathBuf> {
    let today = chrono_today();
    let base_name = if let Some(name) = run_name {
        format!("{}-{}", today, name)
    } else {
        let name = generate_name(seed);
        format!("{}-{}", today, name)
    };

    let candidate = root.join(&base_name);
    if !candidate.exists() {
        std::fs::create_dir_all(&candidate)?;
        return Ok(candidate);
    }

    // Collision: append -2, -3, ...
    let mut counter = 2u32;
    loop {
        let candidate = root.join(format!("{}-{}", base_name, counter));
        if !candidate.exists() {
            std::fs::create_dir_all(&candidate)?;
            return Ok(candidate);
        }
        counter += 1;
    }
}

/// Get today's date as ISO string (YYYY-MM-DD).
fn chrono_today() -> String {
    // Use std time to avoid chrono dependency
    let now = std::time::SystemTime::now();
    let since_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let days = since_epoch.as_secs() / 86400;
    // Simple days-since-epoch to date conversion
    let (year, month, day) = days_to_date(days as i64);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_date(days: i64) -> (i32, u32, u32) {
    // Algorithm from Howard Hinnant
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjectives_all_lowercase() {
        for adj in ADJECTIVES {
            assert_eq!(*adj, adj.to_lowercase(), "Adjective not lowercase: {}", adj);
        }
    }

    #[test]
    fn test_nouns_all_lowercase() {
        for noun in NOUNS {
            assert_eq!(*noun, noun.to_lowercase(), "Noun not lowercase: {}", noun);
        }
    }

    #[test]
    fn test_no_duplicate_adjectives() {
        let mut seen = std::collections::HashSet::new();
        for adj in ADJECTIVES {
            assert!(seen.insert(*adj), "Duplicate adjective: {}", adj);
        }
    }

    #[test]
    fn test_no_duplicate_nouns() {
        let mut seen = std::collections::HashSet::new();
        for noun in NOUNS {
            assert!(seen.insert(*noun), "Duplicate noun: {}", noun);
        }
    }

    #[test]
    fn test_sufficient_combinations() {
        let combos = ADJECTIVES.len() * NOUNS.len();
        assert!(combos >= 40_000, "Only {} combinations, need 40k+", combos);
    }

    #[test]
    fn test_generate_name_format() {
        let name = generate_name(Some(42));
        assert!(name.contains('-'), "Name should contain hyphen: {}", name);
        let parts: Vec<&str> = name.splitn(2, '-').collect();
        assert_eq!(parts.len(), 2);
        assert!(!parts[0].is_empty());
        assert!(!parts[1].is_empty());
    }

    #[test]
    fn test_generate_name_deterministic() {
        let a = generate_name(Some(42));
        let b = generate_name(Some(42));
        assert_eq!(a, b);
    }

    #[test]
    fn test_generate_name_varies() {
        let a = generate_name(Some(1));
        let b = generate_name(Some(2));
        // Different seeds should usually give different names
        // (tiny chance of collision, but extremely unlikely with 40k+ combos)
        assert_ne!(a, b);
    }

    #[test]
    fn test_generate_run_id_format() {
        let id = generate_run_id(Some(42));
        // Should start with YYYY-MM-DD-
        let parts: Vec<&str> = id.splitn(4, '-').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0].len(), 4); // year
        assert_eq!(parts[1].len(), 2); // month
        // parts[2] is DD-rest, parts[3] is rest
    }

    #[test]
    fn test_generate_run_id_deterministic() {
        let a = generate_run_id(Some(42));
        let b = generate_run_id(Some(42));
        assert_eq!(a, b);
    }

    #[test]
    fn test_create_run_dir_basic() {
        let root = std::env::temp_dir().join("glottisdale_names_test");
        std::fs::create_dir_all(&root).unwrap();

        let dir = create_run_dir(&root, Some(42), None).unwrap();
        assert!(dir.exists());
        assert!(dir.is_dir());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_create_run_dir_collision() {
        let root = std::env::temp_dir().join("glottisdale_names_collision");
        std::fs::create_dir_all(&root).unwrap();

        let dir1 = create_run_dir(&root, Some(42), None).unwrap();
        let dir2 = create_run_dir(&root, Some(42), None).unwrap();
        assert_ne!(dir1, dir2);
        assert!(dir2.to_string_lossy().contains("-2"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_create_run_dir_custom_name() {
        let root = std::env::temp_dir().join("glottisdale_names_custom");
        std::fs::create_dir_all(&root).unwrap();

        let dir = create_run_dir(&root, None, Some("my-custom-run")).unwrap();
        assert!(dir.to_string_lossy().contains("my-custom-run"));
        assert!(dir.exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_days_to_date() {
        // 2024-01-01 = 19723 days since epoch
        let (y, m, d) = days_to_date(19723);
        assert_eq!((y, m, d), (2024, 1, 1));
    }
}
