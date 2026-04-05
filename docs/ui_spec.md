# Voxa: UI Micro-Detailed Specification

This document defines the visual and functional requirements for the **Voxa** dictation interface, aiming for a "Wispr Flow" level of polish and exclusivity.

## High-Fidelity Mocks (Stitch)

- **Settings Dashboard**: [View Image](settings_mock.png)
- **Listening Pill**: [View Image](pill_mock.png)

## 1. The Floating Pill (The "Core")

The Pill is the primary interaction point. It must feel like an organic part of the macOS experience.

### Visual States (Implemented)

- **Idle (Ultra-Compact)**: 
  - **Dimensions**: `h-[6px] w-[40px]`
  - **Style**: Minimalist bar with `obsidian-glass` and 5-bar subtle animation.
- **Recording (Balanced Compact)**:
  - **Dimensions**: `h-10` (40px) height, naturally sized.
  - **Style**: `obsidian-glass`, `ring-1 ring-black/50`, `px-4`, `gap-3`.
  - **Icons**: `20px` for optimal ergonomics.
  - **Waveform**: 18-bar dynamic visualization (`h-5`).
- **Loading (Initialization)**:
  - **Dimensions**: `h-8` (32px).
  - **Style**: Spinning primary-colored indicator with "Loading..." text.

### Positioning & Constraints

- **Window Size**: The floating window is constrained to `300x80` to prevent interference, but the elements remain hyper-compact.
- **Screen Offset**: Fixed **15px offset** from the bottom limit of the monitor (Dock-aware), ensuring visibility above the macOS taskbar.
- **Startup**: Window starts with `visible: false` and is shown via Rust setup to prevent centering flickers.

### Interactions

- **Draggable**: User can reposition the Pill anywhere on the screen.
- **Context Menu**: Right-click to quickly switch **Transformation Profiles**.

---

## 2. Menu Bar (Tray) Interface

A monochromatic icon in the macOS menu bar (Top Right).

### Icons

- **Recording**: Balanced minimalist capsule (`h-10`, `px-4`). Feature-rich but compact.
- **States**: Glows slightly when recording is active.

### Dropdown Menu

1. **Status Indicator**: "Voxa is Ready" (with a green dot).
2. **Quick Toggle**: Transformation Profile (List with current one checked).
3. **Quick Toggle**: Language (ES / EN).
4. **Divider**
5. **Settings...** (Opens the main Settings Panel).
6. **Custom Dictionary...** (Opens the Dictionary manager).
7. **Quit Voxa** (`Cmd+Q`).

---

## 3. Configuration Interface (The Dashboard)

A centered, floating modal with deep categorization.

### Categories

#### A. General

- **Microphone**: System-style dropdown with levels indicator.
- **Global Shortcut**: Hotkey recorder (current: `Alt+Space`).
- **Interaction Mode**: Toggle between "Push-To-Talk" and "Hands-Free".

#### B. The Engine (AI)

- **Transformation Profile**:
  - Cards for "Professional", "Informal", "Creative", "Custom".
  - Ability to edit the System Prompt for the active profile.
- **Speech Language**: Dynamic switch (Whisper optimization).
- **Inference Strategy**: "Better Accuracy" vs "Near Zero Latency".

#### C. Vocabulary (Personalization)

- **Custom Dictionary**: A tags-cloud interface to add technical terms, names, or acronyms.
- **Import/Export**: Ability to sync dictionary via JSON.

---

## 4. Installation & First Run

A premium onboarding sequence to ensure Voxa works perfectly.

### Steps

1. **Welcome**: Sleek animation showing "Your voice, perfectly typed."
2. **Permissions**: Guided request for **Microphone** and **Accessibility** (required for text injection).
3. **Model Download**: Visual progress bars for Whisper and Llama weights (with "Optimizing for your Mac" text).
4. **Setup Shortcut**: User chooses their activation key.
5. **Tutorial**: A 10-second interactive guided dictation (e.g., "Say: Hello Voxa, this is amazing.").

---

## 5. Design Tokens

- **Primary**: Pure White (`#FFFFFF`) for text/icons.
- **Background**: Deep Obsidian (`#0A0A0A`) with 80% opacity and 40px blur.
- **Accents**: Subtle silver and a soft purple/blue glow for "AI processing" states.
- **Typography**: Inter (System Default) or Geist Sans for a technical, modern feel.
- **Corners**: Large radii (`2.5rem` or `24px`) for a "pill" aesthetic.
