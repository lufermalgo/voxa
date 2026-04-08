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

**[→ Download Voxa v1.0.2 for macOS (Apple Silicon)](https://github.com/lufermalgo/voxa/releases/tag/v1.0.2)**

> Requires macOS 13+ on Apple Silicon (M1/M2/M3/M4). Intel support coming soon.

### First-time setup

1. Download and open the `.dmg` file.
2. Drag **Voxa** to your Applications folder.
3. On first launch, macOS will ask for **microphone** and **accessibility** permissions — both are required.
4. Voxa will automatically download the AI models (~1 GB) on first run.
5. Set your activation shortcut in **Settings** and start dictating.

> **Note:** Voxa runs entirely on-device. No data ever leaves your machine.

## 🚀 Development

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/)
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites)
- `brew install llama.cpp` (for LLM inference)

### Running locally

1. Clone the repository:
   ```bash
   git clone https://github.com/lufermalgo/voxa.git
   cd voxa
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Run in development:
   ```bash
   npm run tauri dev
   ```

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
