# macOS Native Event Tap & Shortcut Architecture

This document outlines the technical implementation and lessons learned while building the global shortcut system for Voxa, specifically focusing on the native macOS `CGEventTap` used to handle specialized hardware keys and bypass system limitations.

## Context & Problem Statement

Voxa requires high-performance, low-latency global shortcut handling. Initially, we relied on high-level Rust crates and the Tauri `global_shortcut` plugin. However, we encountered three major issues:

1. **System Conflicts**: Hardware keys (like the MacBook "Dictation" or "Microphone" keys) often trigger built-in macOS services (e.g., system dictation), causing double-transcription or UI overlaps.
2. **Key Swallowing (Collisions)**: Registering "bare" keys (like `Space` or `Backspace`) without modifiers through high-level APIs often "swallows" the key globally, breaking normal typing in all applications.
3. **API Ambiguity**: High-level Carbon/Cocoa traits in Rust (like `TCFType` vs `ForeignType`) can lead to compilation paradoxes when mixing different crates (`core-foundation`, `core-graphics`, `cocoa`).

## Architecture Overview

We implemented a **Hybrid Shortcut System**:

1. **Tauri Global Shortcut Plugin**: Used for standard user-configurable shortcuts that *must* have modifiers (Cmd, Alt, Shift).
2. **Native `CGEventTap` (Pure FFI)**: Used for "reserved" shortcuts (`Paste`, `Cancel`, `Hands-Free`) and hardware keys.
3. **Shortcut Synchronization**: A globally shared `OnceLock<ShortcutConfig>` in `lib.rs` ensures that both the native tap and the backend logic stay in sync.

## Key Learnings & Implementation Details

### 1. The `NSSystemDefined` Mask (14)
To intercept specialized hardware keys (e.g., the MacBook Pro Touch Bar Mic key or the F5 Dictation key), the `CGEventTap` must be created with the `NSSystemDefined` event mask:

```rust
// Mask for NSSystemDefined (14) to capture hardware special keys
let event_mask = (1 << 14) | (1 << 10) | (1 << 11); // NSSystemDefined + KeyDown + KeyUp
```

Without this mask, keycodes like `176` and `179` will be ignored by the tap, and the system will proceed to trigger its default behavior.

### 2. Swallowing System Events
To prevent macOS from activating its built-in dictation engine when using the hardware key for Voxa, we must:
1. Capture both `KeyDown` and `KeyUp` for the specific keycode.
2. Return `None` (null) from the `CGEventTapProxy` callback.

This effectively "consumes" the event before it reaches the system-level HID manager.

### 3. Avoiding Trait Ambiguities (FFI Strategy)
When high-level crate traits conflict, the most robust solution is **raw FFI**. In `src-tauri/src/lib.rs`, we defined raw `extern "C"` bindings for:
- `CGEventTapCreate`
- `CFMachPortCreateRunLoopSource`
- `CFRunLoopAddSource`

This bypasses the `TCFType` dependency issues and provides a stable, persistent event tap that survives application lifecycle changes.

### 4. Context-Aware Interception
To avoid breaking the `Escape` key's normal functionality (e.g., closing dialogs in other apps), the native tap implements **Context-Aware Guards**:

```rust
if keycode == ESCAPE {
    // Only swallow Escape if we are actually recording
    if is_recording { return None; }
}
```

### 5. Shortcut Normalization (DB Migrations)
To prevent "accidental" global keyboard locks, we implemented a database migration that:
1. Scans for shortcuts missing modifiers (bare keys).
2. Resets them to safe defaults.
3. Ensures `shortcut_hands_free` is always mapped to `"F5"` (which represents the unified hardware key in our logic).

## Future Recommendations

- **Hardware Key Expansion**: When adding support for new specialized keys (e.g., the "Globe" key or Media keys), consult the `macos_keycode_to_name` table and ensure the `NSSystemDefined` mask remains active.
- **Permission Handling**: Always verify `AXIsProcessTrusted()` before initializing the tap. macOS will silently ignore tap creation if Accessibility permissions are missing.

---
**Senior Architect Note**: *Concepts > Code. Understanding the low-level HID event flow is more important than knowing specific crate APIs. Always aim for a design that respects the user's system state.*
