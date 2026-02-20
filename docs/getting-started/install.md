# Installation

This page walks you through installing glottisdale and everything it needs. No prior terminal experience required — just follow the steps for your operating system.

---

## Prerequisites

Glottisdale needs three things on your computer before it can run:

| Tool | What it is | Why glottisdale needs it |
|------|-----------|--------------------------|
| **Python 3.11+** | A programming language. Glottisdale is written in it. | Runs the tool itself |
| **pip** | Python's package installer. Comes bundled with Python. | Downloads and installs glottisdale |
| **ffmpeg** | A command-line audio/video Swiss Army knife. | Extracts and processes audio from your files |

### macOS

Open **Terminal** (search for it in Spotlight, or find it in Applications > Utilities).

If you don't have [Homebrew](https://brew.sh) yet, install it first:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Then install Python and ffmpeg:

```bash
brew install python ffmpeg
```

### Windows

1. **Python** — Download and run the installer from [python.org](https://www.python.org/downloads/). During installation, check the box that says **"Add Python to PATH"** (this is important).

2. **ffmpeg** — The easiest option is [winget](https://learn.microsoft.com/en-us/windows/package-manager/winget/), which comes with Windows 10/11. Open **PowerShell** and run:

   ```powershell
   winget install ffmpeg
   ```

   If winget is not available, download a build from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/), extract it, and add the `bin` folder to your system PATH.

3. **Verify PATH** — Open a new PowerShell window and run:

   ```powershell
   python --version
   ffmpeg -version
   ```

   Both should print version information. If either says "not recognized", the tool is not on your PATH — revisit the install steps above.

### Linux (Debian/Ubuntu)

Open a terminal and run:

```bash
sudo apt install python3 python3-pip ffmpeg
```

On other distributions, use your package manager's equivalent (e.g., `dnf install` on Fedora, `pacman -S` on Arch).

---

## Install glottisdale

With the prerequisites in place, install glottisdale with a single command:

```bash
pip install git+https://github.com/A-U-Supply/glottisdale.git
```

This downloads glottisdale directly from GitHub and installs it along with its core dependencies (Whisper for speech recognition, g2p for phoneme conversion, and a few numerical libraries).

> **Note:** On some systems you may need to use `pip3` instead of `pip`. If `pip` is not found, try `pip3`.

---

## Optional extras

Glottisdale has a modular design. The core install covers syllable collages. Additional features need extra packages and system tools. Pick the path that matches what you want to do:

### "I just want collages"

You're all set — the core install above is all you need. Move on to [Verify your install](#verify-your-install).

### "I want vocal MIDI mapping"

The `sing` subcommand maps syllable clips onto MIDI melodies. It requires one extra Python package and a system tool called **rubberband** (for pitch-shifting and time-stretching audio).

Install glottisdale with the sing extra:

```bash
pip install "glottisdale[sing] @ git+https://github.com/A-U-Supply/glottisdale.git"
```

Then install rubberband:

- **macOS:** `brew install rubberband`
- **Linux:** `sudo apt install rubberband-cli`
- **Windows:** Download from the [Rubber Band Library releases](https://breakfastquay.com/rubberband/) and add to your PATH.

### "I want the most accurate syllable detection"

By default, glottisdale estimates syllable boundaries from Whisper's word timestamps. For more precise results, you can enable the **Bournemouth Forced Aligner (BFA)**, which uses real phoneme-level timing from the audio signal. It requires **espeak-ng**, a speech synthesis engine.

Install glottisdale with the bfa extra:

```bash
pip install "glottisdale[bfa] @ git+https://github.com/A-U-Supply/glottisdale.git"
```

Then install espeak-ng:

- **macOS:** `brew install espeak-ng`
- **Linux:** `sudo apt install espeak-ng`
- **Windows:** Download from [espeak-ng releases](https://github.com/espeak-ng/espeak-ng/releases) and add to your PATH.

### "I want everything"

Install all optional extras at once:

```bash
pip install "glottisdale[all] @ git+https://github.com/A-U-Supply/glottisdale.git"
```

You'll still need the system tools for any extras you plan to use (rubberband for sing, espeak-ng for BFA). See the sections above for platform-specific install commands.

---

## Verify your install

Run:

```bash
glottisdale --help
```

You should see output listing the `collage` and `sing` subcommands with their options. If you see this, glottisdale is installed and ready to go.

If you get a "command not found" error, your Python scripts directory may not be on your PATH. See [Troubleshooting](../guide/troubleshooting.md) for help.

---

## Developer install

If you want to contribute to glottisdale or run it from source:

```bash
git clone https://github.com/A-U-Supply/glottisdale.git
cd glottisdale
pip install -e '.[all,dev]'
```

Download the required NLTK data:

```bash
python -c "import nltk; nltk.download('averaged_perceptron_tagger_eng')"
```

Run the test suite:

```bash
pytest
```

All tests should pass. If anything fails, check that you have ffmpeg, espeak-ng, and rubberband installed (see [Prerequisites](#prerequisites) and [Optional extras](#optional-extras) above).
