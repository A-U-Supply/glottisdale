# Installation

This page walks you through installing glottisdale and everything it needs. No prior terminal experience required — just follow the steps for your operating system.

---

## Prerequisites

Glottisdale needs a few things on your computer before it can run:

| Tool | What it is | Why glottisdale needs it |
|------|-----------|--------------------------|
| **ffmpeg** | A command-line audio/video Swiss Army knife | Extracts and processes audio from your files |
| **Whisper** | OpenAI's speech recognition model | Transcribes speech to find word timestamps |

### macOS

Open **Terminal** (search for it in Spotlight, or find it in Applications > Utilities).

If you don't have [Homebrew](https://brew.sh) yet, install it first:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Then install ffmpeg:

```bash
brew install ffmpeg
```

Install Whisper (requires Python 3.11+):

```bash
pip install openai-whisper
```

> **Note:** On some systems you may need to use `pip3` instead of `pip`.

### Windows

1. **ffmpeg** — The easiest option is [winget](https://learn.microsoft.com/en-us/windows/package-manager/winget/), which comes with Windows 10/11. Open **PowerShell** and run:

   ```powershell
   winget install ffmpeg
   ```

   If winget is not available, download a build from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/), extract it, and add the `bin` folder to your system PATH.

2. **Whisper** — Install Python 3.11+ from [python.org](https://www.python.org/downloads/) (check "Add Python to PATH" during install), then:

   ```powershell
   pip install openai-whisper
   ```

3. **Verify PATH** — Open a new PowerShell window and run:

   ```powershell
   ffmpeg -version
   whisper --help
   ```

   Both should print version information. If either says "not recognized", the tool is not on your PATH — revisit the install steps above.

### Linux (Debian/Ubuntu)

Open a terminal and run:

```bash
sudo apt install ffmpeg python3 python3-pip
pip install openai-whisper
```

On other distributions, use your package manager's equivalent (e.g., `dnf install` on Fedora, `pacman -S` on Arch).

---

## Install glottisdale

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

> **Linux note:** Building from source requires `libasound2-dev` for audio playback support: `sudo apt install libasound2-dev`

---

## Optional extras

Glottisdale has three pipelines. The core install covers all of them, but some features need additional system tools.

### "I just want collages"

You're all set — ffmpeg and Whisper are all you need. Move on to [Verify your install](#verify-your-install).

### "I want vocal MIDI mapping"

The `sing` subcommand maps syllable clips onto MIDI melodies. It requires **rubberband** for pitch-shifting and time-stretching audio.

- **macOS:** `brew install rubberband`
- **Linux:** `sudo apt install rubberband-cli`
- **Windows:** Download from the [Rubber Band Library releases](https://breakfastquay.com/rubberband/) and add to your PATH.

### "I want the most accurate syllable detection"

By default, glottisdale estimates syllable boundaries from Whisper's word timestamps. For more precise results, you can enable the **Bournemouth Forced Aligner (BFA)**, which uses real phoneme-level timing from the audio signal. It requires **espeak-ng**, a speech synthesis engine.

- **macOS:** `brew install espeak-ng`
- **Linux:** `sudo apt install espeak-ng`
- **Windows:** Download from [espeak-ng releases](https://github.com/espeak-ng/espeak-ng/releases) and add to your PATH.

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

All tests should pass. If anything fails, check that you have ffmpeg and Whisper installed (see [Prerequisites](#prerequisites) above).

On Linux, you'll also need `libasound2-dev`:

```bash
sudo apt install libasound2-dev
```
