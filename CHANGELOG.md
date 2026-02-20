# Changelog

## 0.1.0 (2026-02-19)

Initial standalone release, extracted from the ausupply.github.io monorepo.

### Features
- **Syllable collage engine** (`glottisdale collage`) — Whisper ASR, g2p_en phoneme conversion, ARPABET syllabification, phonotactic ordering, audio polish (pitch/volume normalization, room tone, breaths, pink noise, prosodic dynamics), time stretch, word repeat, stutter
- **Vocal MIDI mapping** (`glottisdale sing`) — "drunk choir" engine mapping syllable clips to MIDI melody notes with pitch drift, vibrato, chorus, rubberband pitch/time stretch
- **BFA alignment** (optional) — Bournemouth Forced Aligner for real phoneme-level timestamps
- **MIDI synthesis** — vendored sine-wave MIDI preview synthesizer
- **Subcommand CLI** — `glottisdale collage` and `glottisdale sing`

### Migration
- Merged `hymnal-gargler/` vocal mapping into `glottisdale.sing` subpackage
- Vendored `midi-bot/src/synthesizer.py` as `glottisdale.sing.synthesize`
- All `importlib.util` hacks replaced with proper package imports
- Installable via `pip install` from GitHub
