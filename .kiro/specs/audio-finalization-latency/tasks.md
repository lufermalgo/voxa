# Implementation Plan — Reduce finalization & audio-handling latency

## Overview

Three independent, low-risk changes removing fixed per-dictation overhead: retain audio
config (B5), right-size the resampler (B6), and replace the blind paste sleep with a
bounded readiness poll (B4). Files touched are Claude-owned — coordinate per AGENTS.md.

## Tasks

- [ ] 1. Retain audio config (B5)
  - 1.1 Add `sample_rate: u32` and `channels: u16` to `AudioState`; populate in
    `setup_stream`.
    - _Requirements: 1.1_
  - 1.2 Rewrite `stop_stream` to read retained config instead of re-querying the device.
    - _Requirements: 1.2, 1.4_
  - 1.3 Unit-test mono downmix with retained channel count (synthetic multichannel buffer
    → same result as before).
    - _Requirements: 1.3_

- [ ] 2. Speech-appropriate resampler (B6)
  - 2.1 Lower `sinc_len` / `oversampling_factor` in `resample_to_16k` to speech-grade
    values; keep the 16 kHz short-circuit.
    - _Requirements: 3.1, 3.4_
  - 2.2 Validate transcription parity: transcribe a representative clip old vs new params,
    confirm equivalent text. Record result in PR.
    - _Requirements: 3.2_
  - 2.3 Measure resample CPU time old vs new on the same buffer; confirm speed-up.
    - _Requirements: 3.3_

- [ ] 3. Bounded paste-readiness poll (B4)
  - 3.1 Add a cheap `frontmost_pid()` helper in `event_tap.rs`.
    - _Requirements: 2.1_
  - 3.2 Replace the fixed 80 ms sleep in `pipeline.rs` with a poll: break when target app
    is frontmost, hard cap ≤150 ms.
    - _Requirements: 2.1, 2.2, 2.3_
  - 3.3 Validate paste reliability across editor/browser/chat apps; confirm no regression
    and lower perceived latency.
    - _Requirements: 2.4_

- [ ] 4. Verify & document
  - `cargo check` clean for each change; document before/after timings in the PR(s).
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4_

## Task Dependency Graph

```json
{
  "waves": [
    { "wave": 1, "tasks": ["1", "2", "3"] },
    { "wave": 2, "tasks": ["4"] }
  ]
}
```

- Wave 1: Tasks 1 (config retain), 2 (resampler), and 3 (paste readiness) are fully
  independent and may ship as separate PRs.
- Wave 2: Task 4 (verify) applies to whichever change(s) land.

## Notes
- Files touched: `src-tauri/src/audio.rs`, `src-tauri/src/pipeline.rs`,
  `src-tauri/src/event_tap.rs` (Claude-owned). Coordinate per AGENTS.md.
- May ship as one PR or three small PRs (changes are independent). Suggested branches:
  `feature/issue-XX-audio-config-retain`, `feature/issue-XX-resampler-speech`,
  `feature/issue-XX-paste-readiness`.
