# VAD-Reactive Waveform Animation

## Overview

The recording pill displays a 5-bar waveform that reacts to the actual microphone signal in real time. Each bar's height maps to the current RMS level from the audio callback, smoothed and shaped for visual appeal.

## Pipeline

```
AudioEngine (audio callback, ~10ms)
  → compute RMS → AtomicU32 (lock-free)
  → polling thread (30fps) → normalize → emit "audio-level" float
  → useAudioLevel hook → EMA smooth → VAD hysteresis → bell-curve heights
  → requestAnimationFrame loop (~60fps) → setState → CSS transition (80ms)
  → RecorderPill bars (3–20px height)
```

## Key Design Decisions

### Decoupled event and render rates

Tauri's `audio-level` events arrive at ~30fps from the Rust polling thread. The frontend hook writes to a **ref** (no re-render on every event), and a separate `requestAnimationFrame` loop reads the ref and calls `setState` at vsync (~60fps). This separates the audio event rate from the React render cycle.

### EMA smoothing

Exponential Moving Average with `alpha = 0.35` — fast enough to follow speech onsets but slow enough to avoid jitter on individual frames.

```ts
smoothed.current = alpha * raw + (1 - alpha) * smoothed.current;
```

### VAD hysteresis

Two thresholds prevent flicker at the silence/speech boundary:

| Threshold | Value | Meaning |
|-----------|-------|---------|
| `ONSET`   | 0.08  | Start "speaking" state |
| `OFFSET`  | 0.04  | Return to "silence" state |

In silence, the effective level is dampened by `0.3×`, so bars drop to a subtle idle state instead of going fully flat.

### Bell-curve bar profile

Bar heights follow a quadratic bell curve — the center bar is tallest, edges are shortest. Per-bar deterministic jitter via `Math.sin(i * 2.39996)` adds organic variation without frame-to-frame randomness.

```ts
// bell curve: 0.3 at edges, 1.0 at center
const center = (NUM_BARS - 1) / 2;
const normalized = 1 - ((i - center) / center) ** 2;
const profile = 0.3 + 0.7 * normalized;
const jitter = 0.85 + 0.15 * Math.sin(i * 2.39996);
const heightPx = MIN_HEIGHT + (MAX_HEIGHT - MIN_HEIGHT) * level * profile * jitter;
```

### CSS transition

Each bar has `transition: height 80ms ease-out` — handles inter-frame smoothing at the CSS layer without JS overhead.

## Files

| File | Role |
|------|------|
| `src-tauri/src/audio.rs` | Computes RMS per chunk, stores in `Arc<AtomicU32>` |
| `src-tauri/src/lib.rs` | Polling thread at 30fps, normalizes and emits `audio-level` |
| `src/hooks/useAudioLevel.ts` | EMA, VAD hysteresis, bell-curve heights, rAF loop |
| `src/components/RecorderPill.tsx` | Renders bars with inline `height` style |
| `src/App.css` | `wave-pulse-recording` keyframe removed (was purely decorative) |
