# Design Document — Streaming / overlapped transcription

## Overview

Convert the batch STT stage into an overlapped one: transcribe audio while the user is
still speaking, so post-stop work shrinks to the last chunk plus the LLM step. Because this
reshapes the core pipeline, the design is **spike-first**: prove the latency win and quality
parity on the M3 before implementing, and ship behind a fallback/flag.

### Current pipeline (as-is)

```
key-down → setup_stream → [capture into Vec<f32>] → key-up
  → stop_stream (downmix + resample whole buffer)
  → VAD whole buffer (speech? boolean)
  → Whisper.transcribe(whole buffer)        ← all latency lives here, after stop
  → vocab → LLM refine → paste
```
Capture and Whisper never overlap. RTF is good, but the user pays it entirely after stop.

## Architecture

Target (post-spike, approach-agnostic):

```
key-down → setup_stream (incremental VAD on frames as they arrive)
  → STT worker thread consumes audio windows during capture, builds running transcript
  → key-up → STT worker finishes the final window only
  → assemble final transcript (dedup seams) → vocab → LLM refine → paste
```

### Candidate approaches (the spike compares these)

- **Approach A — chunked re-feed (rolling windows):** every ~N seconds (or at VAD silence),
  transcribe audio since the last cut plus a small overlap tail; maintain a running
  transcript and de-duplicate seams. Uses the existing `whisper-rs` `full()` API; no new
  dependency. Seam handling is the hard part.
- **Approach B — whisper.cpp streaming-style incremental decode:** sliding-window streaming
  similar to `whisper-stream`. Closer to "true" streaming, but `whisper-rs 0.13` may not
  expose the needed step API cleanly; feasibility must be confirmed in the spike.

The spike picks A or B based on measured latency, quality parity, and binding feasibility.

## Components and Interfaces

- **Audio capture (`audio.rs`)**: in addition to filling the buffer, push frames to the
  incremental VAD and make captured (resampled) windows available to the STT worker (e.g.
  via a channel or a shared growing buffer with a consumed-offset cursor).
- **STT worker (`pipeline.rs` / `whisper_inference.rs`)**: a thread that, while
  `RecordingState` is true, pulls the next window, transcribes with the persistent
  `WhisperEngine`, and appends to a shared running transcript with seam de-duplication.
- **Incremental VAD (`vad.rs`)**: already frame-based (512-sample frames, LSTM state across
  frames). Drive it from the capture callback instead of the post-stop loop. Keep a
  `speech_ever_detected` flag for the silence-skip path (Req 5.2).
- **Assembly (`pipeline.rs`)**: on stop, finish the last window, concatenate/dedup, hand off
  to the LLM step (unchanged).

### Seam handling (Req 3)
- Keep an overlap tail (e.g. 1–2 s or last VAD-silence boundary) between windows.
- De-dup by matching the overlap region text, or prefer cuts at VAD silence so there is
  nothing to dedup.

## Data Models

- **New setting `streaming_stt`** (boolean, default `false`) gating the whole feature.
- **Running transcript state**: an in-memory structure owned by the STT worker for the
  duration of one dictation (windows + assembled text + consumed offset). No persistence.
- **VAD**: reuse existing `VadEngine` state; add a `speech_ever_detected` bool driven during
  capture.
- No DB schema change unless the toggle is surfaced in the UI (then `app_settings` seeds a
  default, mirroring existing settings like `bypass_llm`).

## Correctness Properties

### Property 1: Off == current behavior
With `streaming_stt = false`, behavior is byte-for-byte the current batch pipeline.
**Validates: Requirements 6.2**

### Property 2: Clean seams
The assembled streaming transcript has no dropped or duplicated words at window seams.
**Validates: Requirements 2.3, 3.1, 3.2**

### Property 3: Engine reuse
The persistent `WhisperEngine` is reused across all windows of a dictation (no per-window
model reload).
**Validates: Requirements 4.1**

### Property 4: Bounded memory
Metal buffer memory stays bounded across a long dictation (reset cadence adapted).
**Validates: Requirements 4.2**

### Property 5: Fallback correctness
A streaming failure mid-dictation always yields a correct batch transcript via fallback.
**Validates: Requirements 6.1**

## Error Handling

| Case | Behavior |
|------|----------|
| Streaming STT errors mid-dictation | Discard partial state, run batch `transcribe` on the full buffer (Req 6.1). |
| `streaming_stt` disabled | Run current batch pipeline unchanged (Req 6.2). |
| No speech ever detected (incremental VAD) | Skip STT, preserve silence-skip (Req 5.2). |
| Metal buffers grow on long dictation | Adapt state-reset cadence by audio-seconds (Req 4.2). |

## Resource management (Req 4)

- Reuse the single persistent `WhisperEngine` + state — never reload per window.
- Adapt the existing `MAX_USES_BEFORE_RESET = 20` reclamation to audio-seconds processed so
  memory stays bounded without resetting mid-dictation too aggressively.
- Streaming STT for a dictation completes before its LLM step (refinement runs after stop on
  assembled text), so the two do not share the GPU simultaneously.

## Testing strategy

- Spike harness (Task 1): scripted measurement of post-stop latency for 5 s / 20 s / 60 s
  clips, batch vs streaming, plus a text-equivalence check (WER vs the batch output).
- Unit: seam de-duplication logic (overlapping windows → correct merged transcript).
- Unit: incremental VAD `speech_ever_detected` matches the current full-buffer decision on
  representative buffers.
- Manual: long dictation (~60 s) — result appears shortly after stop, text matches batch.
- `cargo check` clean; vitest unaffected unless the toggle reaches the UI (coordinate on
  `useSettings.ts` / `SettingsPanel.tsx`).

## Decision gate

After the spike (Task 1): IF latency improvement is not significant on the M3 OR quality
parity cannot be maintained OR `whisper-rs` cannot support the chosen approach cleanly,
THEN close this spec with the documented finding and do **not** implement. This protects
`main` from a risky rewrite that doesn't pay off.

## Sequencing

Do this **after** `llama-server-lifecycle` and `audio-finalization-latency` are merged, so
the pipeline is stable and memory is clean before reshaping the core flow.
