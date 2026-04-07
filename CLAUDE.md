# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Development (hot-reload)
npm run tauri dev

# Production build
npm run tauri build

# Frontend only (without Rust)
npm run dev

# Rust tests (no integration tests — mock_app only)
cd src-tauri && cargo test
```

Do NOT run `npm run build` or `cargo build` after code changes unless explicitly asked.

## Architecture

Voxa is a macOS-first, system-wide dictation tool. It uses a **floating pill UI** that listens to the user's voice and injects transcribed + LLM-refined text directly into the active window via `Cmd+V`.

### Pipeline

```
Hotkey press → AudioEngine (cpal) → stop_and_transcribe()
  → WhisperEngine (whisper-rs) → raw transcript
  → LlamaEngine (llama-server HTTP) → refined text
  → simulate_paste() via CGEvent → target app
```

The pipeline is driven by an **MPSC channel** (`DictationEvent`) that decouples hotkey events from the inference thread. The inference worker loop in `lib.rs` (`run_dictation_worker`) consumes events sequentially.

### Rust modules (`src-tauri/src/`)

| File | Responsibility |
|------|----------------|
| `lib.rs` | Entry point, Tauri setup, all `#[tauri::command]`s, dictation pipeline, native event tap, state structs |
| `audio.rs` | `AudioEngine` — cpal stream management, mono conversion, 16kHz resampling, normalization, `current_level` AtomicU32 |
| `whisper_inference.rs` | `WhisperEngine` — wraps `whisper-rs`, hallucination stripping, no-speech threshold |
| `llama_inference.rs` | `LlamaEngine` — spawns `llama-server` subprocess, ChatML HTTP calls |
| `models.rs` | `ModelManager` — model download, path resolution, GPU detection |
| `db.rs` | SQLite via rusqlite — transcripts, settings, transformation profiles |
| `window_utils.rs` | macOS window positioning utilities |

### Frontend (`src/components/`)

| File | Window |
|------|--------|
| `RecorderPill.tsx` | `main` — floating pill (300×100, transparent, alwaysOnTop) |
| `SettingsPanel.tsx` | `settings` — full settings UI (1200×900) |
| `TrayMenu.tsx` | Tray menu popup |

### macOS Focus Architecture (critical)

The app runs as `NSApplicationActivationPolicyAccessory` (policy=1), meaning it has **no Dock icon and never activates on click**. This is the Alfred/Raycast model.

- **Focus preservation**: `FrontmostApp(Mutex<i32>)` stores the target app's PID via `get_frontmost_app_pid()` (uses `NSWorkspace.frontmostApplication`) before any Voxa window appears.
- **Re-activation**: `activate_app_by_pid(pid)` uses `NSRunningApplication.runningApplicationWithProcessIdentifier:` → `activateWithOptions:3` (PID-based, works for Electron/JVM apps). Do NOT use osascript name-based activation — it fails for VS Code/Cursor (reported as "Electron").
- **Paste**: `simulate_paste()` uses `CGEvent` (key code 9 = V + `CGEventFlagCommand`). Do NOT use osascript for paste — CGEvent is faster and more reliable.

### Global Shortcuts

Uses a native **`CGEventTap`** (`setup_native_event_tap` in `lib.rs`), not Tauri's global shortcut plugin. The Tauri plugin fails for `Alt+Space` and other system-reserved keys on macOS.

- Hardware mic/dictation key (keycodes 176, 179, 80) is normalized to "F5" in the database and swallowed at the event tap level to prevent macOS system dictation from triggering simultaneously.
- Bare shortcuts (no modifiers) are auto-reset to safe defaults in `db.rs` migrations.

### LLM Inference

`LlamaEngine` spawns `llama-server` as a subprocess and communicates via HTTP (`/completion` endpoint on a free port starting at 18474).

- On macOS: requires `brew install llama.cpp` (provides `/opt/homebrew/bin/llama-server`). The Cellar symlink is verified to avoid the incompatible `brew install ggml` binary.
- Model selection: Qwen2.5-3B Q4_K_M (Apple Silicon / GPU) or Qwen2.5-1.5B Q4_K_M (Intel / CPU).
- All system prompts are wrapped in an English meta-instruction layer to prevent the model from translating the output regardless of the profile's language.

### Audio Level for Animation

`AudioEngine.current_level` is an `Arc<AtomicU32>` storing f32 RMS bits. The audio callback updates it on every chunk (~10ms). A polling thread in `StartRecording` reads it at 30fps and emits `audio-level` float events to the frontend.

### Database

SQLite at `$APP_DATA_DIR/voxa.db`. Migrations run inline in `db::init_tables` on every startup. The `transformation_profiles` table includes forced UPDATE statements to always overwrite built-in profile prompts to their latest version.

## Key Invariants

- The `LlamaEngine` mutex must NEVER be held while building a new engine (7-8s blocking). Build outside the lock, then re-check inside before storing.
- Audio silence detection uses **peak amplitude** (`peak < 0.05`), not RMS. RMS averages silence and can block real speech at low volume.
- Whisper `no_speech_thold(0.6)` skips trailing silence segments, preventing `[MÚSICA]`/`[Silencio]` hallucinations.
- The `main` window uses `visible: false` in `tauri.conf.json` — it's shown/hidden programmatically from Rust.
