# Glottisdale

Speech synthesis and audio processing toolkit.

## Commands
- `cargo test` — all tests
- `cargo clippy -- -D warnings` — lint
- `cargo test -p glottisdale-core` — core tests only
- `cargo test -p glottisdale-cli` — CLI tests only
- `cargo test -p glottisdale-gui` — GUI tests only

## Architecture
- Cargo workspace: `crates/core` (library), `crates/cli` (binary), `crates/gui` (egui binary)
- Core embeds CMU dict via `include_str!` for G2P (grapheme-to-phoneme)
- Whisper transcription via `whisper` CLI subprocess (or `whisper-native` feature for whisper-rs)
- BFA aligner is a stub — falls back to default aligner

## Code Conventions
- Error types: use `thiserror`, not manual `impl Display + Error`
- Workspace deps: declare in root `Cargo.toml` `[workspace.dependencies]`, reference with `workspace = true`
- Branch naming: `feat/feature-name`

## Gotchas
- CI needs `libasound2-dev` on Linux for rodio/alsa-sys
