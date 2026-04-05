# Voxa: UI Micro-Detailed Specification

This document defines the visual and functional requirements for the **Voxa** dictation interface, aiming for a "Wispr Flow" level of polish and exclusivity.

## High-Fidelity Mocks (Stitch)

- **Settings Dashboard**: [View Image](settings_mock.png)
- **Listening Pill**: [View Image](pill_mock.png)

## 1. The Floating Pill (The "Core")

The Pill is the primary interaction point. It must feel like an organic part of the macOS experience.

### Visual States

- **Idle (Hidden/Compact)**: A tiny, semi-transparent bar at the bottom center of the screen (or draggable).
- **Listening (Active)**:
  - Expands into a rounded pill (Glassmorphism: `backdrop-blur-xl`, `bg-white/5`, `border-white/10`).
  - **Animation**: A subtle, glowing waveform pulse that matches the audio input volume.
  - **Text**: Display "Escuchando..." in tiny, wide-spaced caps.
- **Processing**:
  - Waveform transforms into a sleek, infinite loading gradient (shimmer effect).
  - **Text**: "Procesando con Llama-3..."
- **Success/Done**:
  - Brief green "tick" or glow animation.
  - The Pill collapses back to Idle.

### Interactions

- **Draggable**: User can reposition the Pill anywhere on the screen.
- **Context Menu**: Right-click to quickly switch **Transformation Profiles**.

---

## 2. Menu Bar (Tray) Interface

A monochromatic icon in the macOS menu bar (Top Right).

### Icons

- **Icon**: A minimalist waveform or a stylized "V".
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
