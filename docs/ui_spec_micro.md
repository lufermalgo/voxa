# Voxa: UI Micro-Detailed Design Reference

This document provides the "pixel-perfect" technical details for the Voxa interface, aligning with the premium Wispr Flow aesthetic.

## 1. The Interaction Pill

### Dimensions & Shape
- **Base Width**: Variable (min 200px, max 450px).
- **Height**: 48px.
- **Corner Radius**: 24px (Perfect Pill).
- **Border**: `1px solid rgba(255, 255, 255, 0.1)`.

### Background Architecture
- **Color**: `#0A0A0A` (Deep Obsidian).
- **Opacity**: 80% (Alpha: 0.8).
- **Backdrop Blur**: 40px (High Intensity).
- **Shadow**: `0 20px 50px rgba(0, 0, 0, 0.5)`.

### Typography
- **Font**: Inter Bold or Geist Sans.
- **Size**: 10px.
- **Tracking**: 0.2em (Wide).
- **Case**: All Caps.

### Animations
- **Recording Waveform**: 
  - 5 vertical bars.
  - Variable heights (4px to 16px).
  - Animation: `bar-grow 1.2s infinite ease-in-out`.
- **Processing Shimmer**:
  - A 45-degree white gradient sweep from left to right.
  - DuratIon: 1.5s linear infinite.

---

## 2. The Settings Dashboard

### Window Configuration
- **Dimensions**: 800px x 600px.
- **Mode**: Centered, standard title bar (macOS native).

### Layout
- **Sidebar**: 240px width, `#0A0A0A` background.
- **Content Area**: Deep Obsidian with a subtle radial gradient (`#1a1a2e` at center).
- **Padding**: 48px per section.

### Components
- **Profile Cards**:
  - `bg-white/[0.01]` (Inactive).
  - `bg-white/5` with `border-white/20` (Active).
  - Smooth scale transition (`1.02` on hover).
- **Shortcut Rebounder**:
  - Large mono font for keys.
  - Dotted border for "Listening" state.

---

## 3. Onboarding Sequence

### Slide 1: Welcome
- Big "V" logo centered.
- Text: "Your voice, refined."
- Subtext: "The zero-latency intelligence layer for your Mac."

### Slide 2: Permissions
- Minimalist checklist.
- [ ] Microphone Access.
- [ ] Accessibility Access (for system-wide typing).

### Slide 3: The Engine
- Progress bars for "Whisper Models" and "Llama Refinement".
- Dynamic status text: "Optimizing for Neural Engine...".

### Slide 4: Activation
- "Choose your voice key."
- Defaults to `Alt+Space`.
