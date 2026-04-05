# Voxa

<div align="center">
  <img src="./docs/images/voxa_hero.png" width="100%" alt="Voxa Hero Interface" />
  
  <p align="center">
    <strong>The Silent Conductor of Your Digital Workflow.</strong><br />
    <em>An ultra-minimalist, privacy-first, system-wide dictation tool.</em>
  </p>

  <p align="center">
    <img src="https://img.shields.io/badge/Stack-Tauri%20%7C%20Rust%20%7C%20React-blueviolet?style=for-the-badge" alt="Stack" />
    <img src="https://img.shields.io/badge/Platform-macOS-white?style=for-the-badge&logo=apple" alt="Platform" />
    <img src="https://img.shields.io/badge/Privacy-100%25%20Local-green?style=for-the-badge" alt="Privacy" />
  </p>
</div>

---

## 💎 Philosophy

Voxa isn't just another dictation app. It's a **High-Density, Minimalist Interface** designed following the **"Silent Conductor"** philosophy. It lives at the edge of your screen, ready to translate your thoughts into text directly into any application, without the friction of traditional UI.

Inspired by premium tools like *Wispr Flow*, Voxa focuses on speed, local-first intelligence, and an interface that feels like a piece of digital jewelry.

## ✨ Features

- **Ultra-Compact Pill**: A floating, Obsidian Glass interface that stays 15px from your Dock, providing visual feedback without stealing focus.
- **System-Wide Injection**: Works in every app. Just talk, and let Voxa handle the `Cmd+V`.
- **Local Intelligence**: Powered by `Whisper.cpp` (transcription) and `Llama.cpp` (post-processing) — your voice never leaves your machine.
- **Obsidian Tray Menu**: A custom, high-blur glassmorphism menu for quick profile and language switching.
- **Flicker-Free Experience**: Precision window management handled by the Rust backend for instantaneous positioning.

## 🛠 Tech Stack

- **Core**: [Tauri v2](https://tauri.app/) (Rust)
- **Frontend**: [React](https://reactjs.org/) + [TypeScript](https://www.typescriptlang.org/) + [Vite](https://vitejs.dev/)
- **Styling**: Vanilla CSS (High-Performance Glassmorphism)
- **Engines**: 
  - `faster-whisper` (Local STT)
  - `Ollama` / `Llama.cpp` (Intelligent Post-processing)

## 🚀 Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/)
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites)

### Installation

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

Voxa uses a decoupled **MPSC (Multi-Producer Single-Consumer)** architecture to bridge the asynchronous audio recording stream with the transcription engine, ensuring the UI remains responsive even during heavy inference tasks.

---

<div align="center">
  <sub>Built with ❤️ by <a href="https://github.com/lufermalgo">lufermalgo</a></sub>
</div>
