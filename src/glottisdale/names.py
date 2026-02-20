"""Generate unique, memorable run names for glottisdale output directories.

Names are speech/voice/music-themed adjective-noun pairs like
'breathy-bassoon' or 'staccato-tenor'. Combined with a date prefix,
they produce sortable, identifiable run IDs like '2026-02-19-breathy-bassoon'.
"""

import random
from datetime import date
from pathlib import Path

# ~200+ speech/voice/music-themed adjectives
ADJECTIVES = [
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
]

# ~200+ speech/voice/music-themed nouns
NOUNS = [
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
]


def generate_name(seed: int | None = None) -> str:
    """Generate an adjective-noun name like 'breathy-bassoon'.

    If seed is provided, the name is deterministic.
    """
    rng = random.Random(seed)
    adj = rng.choice(ADJECTIVES)
    noun = rng.choice(NOUNS)
    return f"{adj}-{noun}"


def generate_run_id(seed: int | None = None) -> str:
    """Generate a run ID like '2026-02-19-breathy-bassoon'."""
    today = date.today().isoformat()
    name = generate_name(seed)
    return f"{today}-{name}"


def create_run_dir(
    root: Path,
    seed: int | None = None,
    run_name: str | None = None,
) -> Path:
    """Create a unique run directory inside root.

    Args:
        root: Parent directory (e.g. ./glottisdale-output).
        seed: RNG seed for deterministic name generation.
        run_name: Override the adjective-noun part (date prefix still added).

    Returns:
        Path to the created run directory.
    """
    today = date.today().isoformat()
    if run_name:
        base_name = f"{today}-{run_name}"
    else:
        name = generate_name(seed)
        base_name = f"{today}-{name}"

    candidate = root / base_name
    if not candidate.exists():
        candidate.mkdir(parents=True, exist_ok=True)
        return candidate

    # Collision: append -2, -3, ...
    counter = 2
    while True:
        candidate = root / f"{base_name}-{counter}"
        if not candidate.exists():
            candidate.mkdir(parents=True, exist_ok=True)
            return candidate
        counter += 1
