# Implementation Plan — Streaming / overlapped transcription

## Overview

Spike-first plan to overlap capture with transcription. Task 1 is a decision gate — do NOT
start implementation tasks (2+) until it produces a GO. Sequence this spec AFTER
`llama-server-lifecycle` and `audio-finalization-latency` merge. Files touched are
Claude-owned (and possibly Kiro/shared for the UI toggle) — coordinate per AGENTS.md.

## Tasks

- [x] 1. Spike: validate approach and latency win (decision gate)
  - Build a throwaway harness to transcribe fixed 5 s / 20 s / 60 s speech clips both
    batch and via Approach A (chunked re-feed), measuring post-stop latency and word-error
    vs the batch reference.
  - Assess Approach B (`whisper-rs 0.13` streaming feasibility) at least on paper / minimal
    probe.
  - Produce a GO/NO-GO decision with numbers. IF NO-GO, close the spec with the finding.
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 2. Add `streaming_stt` setting + fallback scaffolding
  - Add an off-by-default flag; wire the pipeline so off == current batch behavior.
  - _Requirements: 6.2_

- [x] 3. Incremental VAD during capture (B7)
  - Drive Silero VAD from the capture callback on incoming frames; maintain
    `speech_ever_detected`.
  - Remove the post-stop full-buffer VAD loop; preserve the silence-skip behavior.
  - Unit-test parity of the incremental decision vs the current full-buffer decision.
  - _Requirements: 5.1, 5.2, 5.3_

- [x] 4. Incremental audio windowing + resampling
  - Make captured (resampled) audio windows available to an STT worker during capture, with
    an overlap tail.
  - _Requirements: 2.1, 3.1_

- [x] 5. STT worker with persistent engine
  - Transcribe windows during capture using the single persistent `WhisperEngine`; append to
    a running transcript. Never reload the model per window.
  - Adapt the Metal-buffer reset cadence to audio-seconds so memory stays bounded.
  - _Requirements: 2.1, 2.2, 4.1, 4.2_

- [x] 6. Seam assembly + de-duplication
  - On stop, finish the final window and merge windows deterministically (dedup overlaps;
    prefer VAD-silence cuts).
  - Unit-test the de-dup logic.
  - _Requirements: 2.3, 3.1, 3.2, 3.3_

- [x] 7. Fallback on error
  - On any streaming failure mid-dictation, fall back to batch transcription of the full
    buffer.
  - _Requirements: 6.1_

- [-] 8. Validate & document
  - `cargo check` clean. Long-dictation manual test: result appears shortly after stop,
    quality matches batch. Record before/after latency for 5/20/60 s in the PR.
  - _Requirements: 2.2, 2.3, 4.3_

## Task Dependency Graph

```json
{
  "waves": [
    { "wave": 1, "tasks": ["1"] },
    { "wave": 2, "tasks": ["2"] },
    { "wave": 3, "tasks": ["3"] },
    { "wave": 4, "tasks": ["4"] },
    { "wave": 5, "tasks": ["5"] },
    { "wave": 6, "tasks": ["6"] },
    { "wave": 7, "tasks": ["7"] },
    { "wave": 8, "tasks": ["8"] }
  ]
}
```

- Task 1 is a hard GATE; nothing else starts until it returns GO.
- Tasks 2→7 are largely sequential (each builds on the prior pipeline change).
- Task 8 (validate) is last.

## Notes
- Files touched: `src-tauri/src/pipeline.rs`, `src-tauri/src/audio.rs`,
  `src-tauri/src/whisper_inference.rs`, `src-tauri/src/vad.rs` (Claude-owned). High
  coordination cost — coordinate heavily per AGENTS.md.
- If the `streaming_stt` toggle surfaces in the UI, that touches `src/hooks/useSettings.ts`
  + `src/components/SettingsPanel.tsx` (Kiro/shared) — coordinate.
- Suggested branch (spike): `feature/issue-XX-streaming-stt-spike`.
