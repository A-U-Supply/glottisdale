# Installation

This page walks you through installing glottisdale. No prior terminal experience required — just follow the steps for your operating system.

---

## Install glottisdale

Glottisdale is fully self-contained — no external dependencies required. Just download and run.

### Pre-built binary (recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/A-U-Supply/glottisdale/releases):

- **Linux (x86_64):** `glottisdale-linux-amd64` + `glottisdale-gui-linux-amd64`
- **macOS (Apple Silicon):** `glottisdale-darwin-arm64` + `glottisdale-gui-darwin-arm64`

Make them executable and move them somewhere on your PATH:

```bash
chmod +x glottisdale-* glottisdale-gui-*
sudo mv glottisdale-* /usr/local/bin/glottisdale
sudo mv glottisdale-gui-* /usr/local/bin/glottisdale-gui
```

### From source

If you prefer to build from source, you'll need Rust 1.75+ installed via [rustup](https://rustup.rs/):

```bash
git clone https://github.com/A-U-Supply/glottisdale.git
cd glottisdale
cargo build --release
```

The CLI binary is at `./target/release/glottisdale`. The GUI binary is at `./target/release/glottisdale-gui`.

> **Linux note:** Building from source requires `libasound2-dev` and `cmake`: `sudo apt install libasound2-dev cmake`

---

## First run

On first run, glottisdale will automatically download the Whisper speech recognition model (~140 MB for the default `base` model). This only happens once — the model is cached in `~/.cache/glottisdale/models/`.

---

## Verify your install

Run:

```bash
glottisdale --help
```

You should see output listing the `collage`, `sing`, and `speak` subcommands with their options. If you see this, glottisdale is installed and ready to go.

If you get a "command not found" error, the binary is not on your PATH. See [Troubleshooting](../guide/troubleshooting.md) for help.

---

## Developer install

If you want to contribute to glottisdale or run it from source:

```bash
git clone https://github.com/A-U-Supply/glottisdale.git
cd glottisdale
cargo build --all-targets
```

Run the test suite:

```bash
cargo test
```

All tests should pass.

On Linux, you'll also need system dependencies:

```bash
sudo apt install libasound2-dev cmake
```
