# Voxa

<div align="center">
  <img src="./public/voxa_logo.png" width="20%" alt="Voxa Voice Interface" />
  
  <p align="center">
    <strong>The Silent Conductor of Your Digital Workflow.</strong><br />
    <em>System-wide voice dictation for macOS — 100% local, no cloud, no subscriptions.</em>
  </p>

  <p align="center">
    <img src="https://img.shields.io/badge/Stack-Tauri%20%7C%20Rust%20%7C%20React-blueviolet?style=for-the-badge" alt="Stack" />
    <img src="https://img.shields.io/badge/Platform-macOS-white?style=for-the-badge&logo=apple" alt="Platform" />
    <img src="https://img.shields.io/badge/Privacy-100%25%20Local-green?style=for-the-badge" alt="Privacy" />
  </p>
</div>

---

**Voxa** is a free, open-source **macOS dictation app** that transcribes your voice and injects text into any application — without sending a single byte to the cloud. Powered by [Whisper](https://github.com/ggerganov/whisper.cpp) for speech-to-text and a local LLM ([llama.cpp](https://github.com/ggerganov/llama.cpp)) for intelligent post-processing, Voxa runs entirely on your machine.

> No API keys. No subscriptions. No data leaves your device.

## 💎 Philosophy

Voxa isn't just another dictation app. It's a **High-Density, Minimalist Interface** designed following the **"Silent Conductor"** philosophy. It lives at the edge of your screen, ready to translate your thoughts into text directly into any application, without the friction of traditional UI.

Inspired by premium tools like *Wispr Flow*, Voxa focuses on speed, local-first intelligence, and an interface that feels like a piece of digital jewelry.

## ✨ Features

### 🎙 Dictation
- **System-Wide Injection**: Works in every app — browser, editor, terminal, Slack. Just talk, Voxa handles the `Cmd+V`.
- **VAD-Reactive Animation**: The recording pill responds in real time to your microphone level — silence dampens, speech drives the wave.
- **Focus Preservation**: Returns focus to the exact app you were typing in after injection, including Electron and JVM targets (VS Code, IntelliJ, Cursor).
- **Ultra-Compact Pill**: A floating Obsidian Glass interface that stays 15px from your Dock — always visible, never in the way.

### 🧠 Transformation Profiles

The most powerful feature of Voxa. Instead of just transcribing, Voxa passes your voice through a local LLM that reshapes the output according to a **profile** — without sending anything to the cloud.

Four built-in profiles, each purpose-built:

| Profile | What it does |
|---------|--------------|
| **Elegant** | Rewrites with perfect grammar and formal vocabulary. Keeps your ideas, elevates the expression. |
| **Informal** | Cleans up filler words and repetitions, keeps your natural tone. Great for Slack and chat. |
| **Code** | Acts as a prompt engineer. Transforms your voice note into a structured, ready-to-use AI prompt (Role / Context / Task / Expected output). |
| **Custom** | Write your own system prompt. Full control over how the LLM processes your voice. |

You can create unlimited custom profiles and switch between them instantly from the tray menu.

### 📋 Transcript History

Every dictation is stored locally in SQLite. From the history panel you can:
- Review both the **raw transcription** and the **LLM-refined version** side by side.
- **Edit** any transcript after the fact to correct errors.
- **Delete** individual entries or clear the full history.

### 📖 Custom Dictionary

When you correct a transcript, Voxa automatically extracts the new words and adds them to your personal dictionary. This improves Whisper's recognition for domain-specific terms, names, and jargon over time. You can also add or remove words manually from Settings.

## 🛠 Tech Stack

- **Core**: [Tauri v2](https://tauri.app/) (Rust)
- **Frontend**: [React](https://reactjs.org/) + [TypeScript](https://www.typescriptlang.org/) + [Vite](https://vitejs.dev/)
- **Styling**: Vanilla CSS (High-Performance Glassmorphism)
- **Engines**: 
  - `whisper-rs` (Local STT via `whisper.cpp`)
  - `llama-server` HTTP API (Intelligent Post-processing via `llama.cpp`)

## 📦 Download

**[→ Download Voxa v1.2.1 for macOS (Apple Silicon)](https://github.com/lufermalgo/voxa/releases/tag/v1.2.1)**

> Requires macOS 13+ on Apple Silicon (M1/M2/M3/M4).

### Installation

1. Download `Voxa_1.2.0_aarch64.dmg` from the link above.
2. Open the `.dmg` and drag **Voxa** to your **Applications** folder.
3. **Important — before opening Voxa**, run this command in Terminal to remove the macOS quarantine flag:
   ```bash
   xattr -cr /Applications/Voxa.app
   ```
4. Open Voxa. On first launch, macOS will ask for **Accessibility** permission — click **Open System Settings** and enable the toggle for Voxa.
5. Voxa will automatically download the AI models (~1 GB) on first run.
6. Use the default shortcuts to start dictating: **Alt+Space** (push-to-talk) or **F5** (hands-free toggle).

### ⚠️ Why the extra step?

Voxa is **not code-signed or notarized** with Apple because the project doesn't have an Apple Developer account ($99/year). This means:

- macOS Gatekeeper will show **"Voxa.app is damaged and can't be opened"** — this is a false positive. The app is not damaged.
- The `xattr -cr` command removes the quarantine attribute that macOS adds to files downloaded from the internet.
- You also need to manually grant **Accessibility** permission in System Settings → Privacy & Security → Accessibility.

**Voxa is 100% open source.** You can audit every line of code in this repository, and you can build it yourself from source (see [Development](#-development) below). No data ever leaves your machine.

> If you're a developer with an Apple Developer account and want to help sign and notarize Voxa, see [docs/code-signing.md](docs/code-signing.md).

### macOS Permissions

Voxa requires these permissions to function:

| Permission | Why | How to grant |
|-----------|-----|-------------|
| **Accessibility** | Capture global keyboard shortcuts and inject text into other apps via `Cmd+V` simulation | System Settings → Privacy & Security → Accessibility → enable Voxa |
| **Microphone** | Record audio for voice dictation | Granted automatically on first recording attempt |

If shortcuts stop working after an update, remove Voxa from the Accessibility list and re-add it — macOS invalidates the permission when the binary hash changes.

> **Note:** Voxa runs entirely on-device. No API keys, no cloud, no subscriptions. Your voice never leaves your machine.

## 🚀 Development

### Prerequisites

- [Node.js](https://nodejs.org/) (v22+)
- [pnpm](https://pnpm.io/) (v9+)
- [Rust](https://www.rust-lang.org/) (stable)
- [Tauri CLI](https://tauri.app/start/) (`pnpm add -g @tauri-apps/cli`)
- macOS with Xcode Command Line Tools (`xcode-select --install`)

### Running locally

1. Clone the repository:
   ```bash
   git clone https://github.com/lufermalgo/voxa.git
   cd voxa
   ```

2. Install dependencies:
   ```bash
   pnpm install
   ```

3. Run in development:
   ```bash
   pnpm tauri dev
   ```

4. Build a release `.dmg`:
   ```bash
   pnpm tauri build --target aarch64-apple-darwin
   ```
   The `.dmg` will be in `src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/`.

> **Note:** When running from `pnpm tauri dev`, shortcuts work because the process inherits Terminal's Accessibility permission. For the built `.app`, you need to grant Accessibility manually (see [Installation](#installation)).

## 🏗 Architecture

```
Hotkey press → AudioEngine (cpal)
  → WhisperEngine (whisper-rs) → raw transcript
  → LlamaEngine (llama-server HTTP) → refined text
  → CGEvent simulate_paste() → target app
```

Voxa uses a decoupled **MPSC channel** to bridge the audio recording stream with the inference pipeline, keeping the UI fully responsive during transcription.

Key design decisions:
- **No Dock icon** — runs as `NSApplicationActivationPolicyAccessory` (Alfred/Raycast model). Never steals focus.
- **Focus preservation** — stores the frontmost app PID via `NSWorkspace` before any Voxa window appears, then restores it via PID-based `activateWithOptions`. Works reliably with Electron and JVM targets (VS Code, IntelliJ).
- **Native event tap** — uses `CGEventTap` at session level instead of Tauri's global shortcut plugin, which fails for system-reserved keys like `Alt+Space` on macOS.
- **LLM inference** — `llama-server` runs as a subprocess on a local port. No GPU required; automatically selects Qwen2.5-3B (Apple Silicon) or Qwen2.5-1.5B (Intel) based on hardware.
- **Audio silence detection** — uses peak amplitude instead of RMS to avoid false negatives on low-volume speech.

## 📚 Technical Documentation

For deep dives into specific technical implementations, see:
- [macOS Native Event Tap & Shortcut Architecture](docs/architecture/shortcuts-native-tap.md)
- [VAD-Reactive Animation Architecture](docs/architecture/vad-animation.md)

---

<div align="center">
  <sub>Built with ❤️ by <a href="https://github.com/lufermalgo">lufermalgo</a></sub>
</div>
