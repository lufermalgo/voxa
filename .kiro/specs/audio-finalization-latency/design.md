# Design Document — Reduce finalization & audio-handling latency

## Overview

Three independent, surgical changes that remove fixed overhead on every dictation without
touching inference quality:
1. Retain audio config at setup so stop doesn't re-query CoreAudio (B5).
2. Replace the blind 80 ms pre-paste sleep with a bounded readiness poll (B4).
3. Use speech-appropriate resampler settings (B6).

Each can ship on its own; grouped here because they share the audio/finalization path.

## Architecture

```
setup_stream: read device config ONCE → store sample_rate, channels in AudioState
   │
capture (callback fills buffer)
   │
stop_stream: use STORED config (no re-query) → mono downmix → resample(speech params) → normalize
   │
pipeline output: clipboard.write → activate_app_by_pid
   │             → poll frontmost==target (≤150ms cap) instead of fixed 80ms sleep
   └─ simulate_paste()
```

## Components and Interfaces

### Change 1 — Retain audio config (B5)
- As-is: `stop_stream` re-resolves the device and calls `default_input_config()` again to
  read sample rate + channels already known at `setup_stream`.
- To-be: store config in `AudioState` at setup; `stop_stream` reads it. `mic_id` param to
  `stop_stream` becomes unnecessary for config lookup (keep or drop per call sites).

### Change 2 — Bounded paste-readiness poll (B4)
- As-is:
  ```
  activate_app_by_pid(target_pid);
  thread::sleep(Duration::from_millis(80));
  simulate_paste();
  ```
- To-be:
  ```
  activate_app_by_pid(target_pid);
  let deadline = Instant::now() + Duration::from_millis(150);  // hard cap (Req 2.3)
  while frontmost_pid() != target_pid && Instant::now() < deadline {
      thread::sleep(Duration::from_millis(10));
  }
  simulate_paste();
  ```
- Needs a cheap `frontmost_pid()` (NSWorkspace frontmost app PID) in `event_tap.rs`. Common
  case pastes in ~10–20 ms instead of 80 ms (Req 2.2); slow case bounded ≤150 ms (Req 2.3).

### Change 3 — Speech-appropriate resampler (B6)
- As-is: `sinc_len: 256`, `oversampling_factor: 128`, BlackmanHarris2.
- To-be: reduce to speech-grade (e.g. `sinc_len` 64, `oversampling_factor` 32 — tune during
  implementation), same rubato API, keep the 16 kHz short-circuit. Validate transcription
  parity before locking values (Req 3.2).

## Data Models

- **`AudioState`** gains two fields:
  ```rust
  struct AudioState {
      stream: Option<SendStream>,
      buffer: Arc<Mutex<Vec<f32>>>,
      sample_rate: u32,   // NEW
      channels: u16,      // NEW
  }
  ```
- No DB or settings schema changes. Resampler params are code constants.

## Correctness Properties

### Property 1: Resample equivalence
For the same captured buffer, mono downmix + resample with the retained config produces the
same 16 kHz mono output as the current re-query path.
**Validates: Requirements 1.2, 1.3**

### Property 2: Bounded, correct paste
The paste always targets the previously-focused app; the readiness poll never exceeds the
150 ms cap and never hangs.
**Validates: Requirements 2.1, 2.3, 2.4**

### Property 3: 16 kHz short-circuit
When the source rate is already 16 kHz, resampling is skipped entirely.
**Validates: Requirements 3.4**

### Property 4: Transcription parity
New resampler params yield transcription text equivalent to current params on a
representative speech sample.
**Validates: Requirements 3.2**

## Error Handling

| Case | Behavior |
|------|----------|
| Device changed mid-recording | Use setup-time config (matches how the buffer was produced); no panic. |
| `frontmost_pid()` unavailable/errors | Treat as not-yet-ready; fall through to the cap, then paste. |
| Resample fails | Propagate existing error path in `stop_stream` (unchanged). |

## Testing strategy

- Unit: mono-downmix using retained channels produces the same length/values as before for
  a synthetic multi-channel buffer.
- Bench/manual: time the resample step on a fixed buffer, old vs new params; confirm
  speed-up (Req 3.3) and transcription parity (Req 3.2).
- Manual: paste reliability across a few apps (editor, browser, chat) — confirm no missed
  pastes and lower perceived end latency (Req 2).
- `cargo check` clean; no frontend change.

## Sequencing

Recommended order (independent, low→high verification effort):
1. Change 1 (B5) — pure refactor, no behavior change.
2. Change 3 (B6) — needs a transcription-parity check.
3. Change 2 (B4) — needs paste-reliability validation across apps.
