# Requirements Document

## Introduction

The diagnostic identified the **largest structural latency source** (finding B2): the
pipeline is fully **batch**. Whisper does not start until the user releases the key, so for
an N-second dictation the user waits roughly `N × RTF` *after* speaking before transcription
even begins, then the LLM runs. Capture and inference never overlap.

Related finding B7: the Silero VAD currently runs over the **whole buffer after stop**, only
to produce a single "was there speech?" boolean — a redundant pass that could instead be
done incrementally during capture (and could feed segmentation).

This spec proposes overlapping audio capture with transcription so that by the time the user
stops talking, most (ideally all) of the audio is already transcribed, collapsing the
post-stop wait toward just the final chunk + the LLM step.

This is the highest-impact but **highest-risk and largest-effort** item. It changes the core
pipeline shape, so it is structured as **investigate-first**, with a spike before committing
to an implementation approach.

## Goals

- Reduce perceived end-to-end latency for medium/long dictations by overlapping STT with
  capture.
- Preserve transcription quality (no worse than the current single-shot batch output).
- Keep GPU/CPU/memory use bounded and avoid contention with the LLM refinement step.

## Non-goals

- Live on-screen partial transcription UI (could be a follow-up; not required here).
- Changing the LLM step itself (refinement still runs once on the final text).
- Replacing whisper.cpp / whisper-rs.

## Glossary

- **Batch STT**: the current model — transcription runs once, on the whole buffer, after
  the user stops.
- **Streaming / overlapped STT**: transcribing already-captured audio while the user is
  still speaking.
- **RTF (real-time factor)**: inference time ÷ audio duration; <1 means faster than real
  time.
- **Window / chunk**: a slice of captured audio transcribed incrementally.
- **Seam**: the boundary between two consecutive windows, where overlap/de-duplication is
  needed to avoid lost or duplicated words.
- **VAD**: Silero voice-activity detection, currently run once over the whole buffer.
- **Spike**: a throwaway investigation to validate feasibility/value before committing to
  implementation.

## Requirements

### Requirement 1: Spike — validate the approach before implementing

**User Story:** As the maintainer, I want evidence that streaming actually reduces latency
on this hardware before committing to a pipeline rewrite.

#### Acceptance Criteria
1. WHEN evaluating approaches THEN the spike SHALL compare at least: (a) chunked re-feed
   (transcribe rolling windows during capture) vs (b) whisper.cpp streaming/`whisper-stream`
   style incremental decoding.
2. WHEN the spike runs THEN it SHALL measure post-stop latency for short (≈5 s), medium
   (≈20 s), and long (≈60 s) dictations, batch vs streaming, on the M3.
3. WHEN the spike completes THEN it SHALL document whether quality is preserved and whether
   the latency gain justifies the complexity. IF the gain is not significant THEN this spec
   SHALL be closed without implementation (documented decision).

### Requirement 2: Overlap capture and transcription

**User Story:** As a user, when I stop talking the result should appear almost immediately
for long dictations, because most of it was transcribed while I spoke.

#### Acceptance Criteria
1. WHEN the user is dictating THEN the system SHALL transcribe already-captured audio
   incrementally rather than waiting for stop.
2. WHEN the user stops THEN the post-stop transcription work SHALL be bounded to roughly the
   final unprocessed chunk, not the whole recording.
3. WHEN streaming is active THEN the final assembled transcript SHALL be equivalent in
   quality to the current batch transcript (no dropped/duplicated words at chunk seams).

### Requirement 3: Correct chunk-boundary handling

**User Story:** As a user, words spoken across a chunk boundary must not be lost or
duplicated.

#### Acceptance Criteria
1. WHEN audio is split into incremental windows THEN the system SHALL use overlap/carry-over
   (or whisper context) so word boundaries are not cut.
2. WHEN assembling the final transcript THEN the system SHALL de-duplicate overlapping
   regions deterministically.
3. WHEN VAD is integrated (see Req 5) THEN chunk boundaries SHOULD prefer silence points to
   minimize seam artifacts.

### Requirement 4: Bounded resource use, no contention with the LLM

**User Story:** As a user on a 24 GB machine, streaming must not blow up memory or starve
the LLM.

#### Acceptance Criteria
1. WHEN streaming transcription runs THEN it SHALL reuse the single persistent
   `WhisperEngine`/state (no per-chunk model reload).
2. WHEN incremental inferences run THEN the existing Metal-buffer reclamation strategy
   (state reset cadence) SHALL be preserved or adapted so Metal memory does not grow
   unbounded during a long dictation.
3. WHEN the LLM refinement starts THEN streaming STT for that dictation SHALL have completed,
   so the two do not contend for the GPU simultaneously (or contention SHALL be shown
   acceptable by the spike).

### Requirement 5: Fold VAD into capture (B7)

**User Story:** As the maintainer, I don't want a redundant full-buffer VAD pass after stop.

#### Acceptance Criteria
1. WHEN audio is being captured THEN VAD SHALL run incrementally on incoming frames rather
   than as a separate full-buffer pass after stop.
2. WHEN the recording stops AND no speech was ever detected THEN the system SHALL skip STT
   (preserve the current silence-skip behavior) using the incremental VAD result.
3. WHEN incremental VAD is adopted THEN the post-stop full-buffer VAD loop SHALL be removed.

### Requirement 6: Safe fallback

**User Story:** As a user, if streaming misbehaves, I want reliable transcription.

#### Acceptance Criteria
1. WHEN streaming transcription fails mid-dictation THEN the system SHALL fall back to the
   current batch transcription of the full buffer.
2. WHEN streaming is disabled (config/flag) THEN the system SHALL behave exactly as the
   current batch pipeline.

## Coordination note (AGENTS.md)

This is a core pipeline change touching `src-tauri/src/pipeline.rs`,
`src-tauri/src/audio.rs`, `src-tauri/src/whisper_inference.rs`, and `src-tauri/src/vad.rs`
— all Claude-owned. High coordination cost; do AFTER the lifecycle and finalization specs
land so `main` is stable. Kiro authored this spec at the repo owner's request.
