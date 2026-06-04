# Requirements Document

## Introduction

The diagnostic found three independent, low-risk latency/CPU costs in the
audio-handling and output path. None changes transcription/LLM quality; each removes
fixed overhead around the inference, paid on every dictation.

- **B5 — redundant device re-enumeration on stop.** `audio::stop_stream`
  (`src-tauri/src/audio.rs`) calls `host.default_input_device()` and
  `device.default_input_config()` again, only to read the sample rate and channel count —
  values already known at `setup_stream` time. This re-queries CoreAudio on the critical
  stop→transcribe path.
- **B4 — fixed 80 ms sleep before paste.** `pipeline.rs` does
  `activate_app_by_pid(...)` then `thread::sleep(80ms)` then `simulate_paste()`. The sleep
  is a blind constant added to every dictation's end-to-end latency, regardless of whether
  the target app is already focused/ready.
- **B6 — oversized sinc resampler.** `audio::resample_to_16k` uses a 256-tap sinc with
  128× oversampling and BlackmanHarris2 over the whole buffer in one pass. This is
  high-fidelity audiophile-grade resampling applied to mono speech that Whisper will
  downmix to 16 kHz features anyway — more CPU than the task needs.

## Glossary

- **B4 / B5 / B6**: finding IDs from the 2026-06-04 implementation diagnostic (paste sleep,
  device re-enumeration, oversized resampler respectively).
- **`setup_stream` / `stop_stream`**: functions in `src-tauri/src/audio.rs` that start and
  stop the CoreAudio (cpal) input stream.
- **Resampler**: the `rubato` sinc resampler converting mic audio to the 16 kHz mono buffer
  Whisper requires.
- **Frontmost app**: the macOS application currently receiving keyboard focus; the paste
  target.

## Requirements

### Requirement 1: Do not re-enumerate the audio device on stop (B5)

**User Story:** As a user, stopping a dictation should not pay an extra CoreAudio query.

#### Acceptance Criteria
1. WHEN a recording stream is set up THEN the system SHALL retain the sample rate and
   channel count (and device identity if needed) for use at stop time.
2. WHEN `stop_stream` runs THEN it SHALL use the retained config and SHALL NOT call
   `default_input_device()` / `default_input_config()` again solely to read sample
   rate/channels.
3. WHEN the retained config is used THEN mono conversion and resampling SHALL produce
   identical output to today for the same input.
4. IF the device changed mid-recording (edge case) THEN the system SHALL still produce a
   valid 16 kHz mono buffer (no panic), using the config captured at setup time.

### Requirement 2: Replace the blind paste sleep with a readiness check (B4)

**User Story:** As a user, the paste should happen as soon as the target app is ready, not
after a fixed delay.

#### Acceptance Criteria
1. WHEN the system needs to paste into the previously-focused app THEN it SHALL paste as
   soon as that app is the frontmost/active app, rather than always waiting a fixed 80 ms.
2. WHEN app activation completes quickly THEN the added latency before paste SHALL be less
   than the current fixed 80 ms.
3. WHEN app activation is slow THEN the system SHALL wait up to a bounded maximum (e.g.
   ≤150 ms) before pasting anyway, so paste never hangs.
4. WHEN the paste fires THEN it SHALL target the correct app (no regression in paste
   reliability versus the current 80 ms behavior).

### Requirement 3: Right-size the resampler for speech (B6)

**User Story:** As a user, resampling my recording to 16 kHz should be fast and not waste
CPU, without hurting transcription accuracy.

#### Acceptance Criteria
1. WHEN audio is resampled to 16 kHz THEN the system SHALL use resampler settings
   appropriate for speech recognition (lower tap count / oversampling than the current
   256-tap / 128× config) OR a justified equivalent.
2. WHEN the new resampler settings are used THEN transcription accuracy on a representative
   speech sample SHALL NOT measurably degrade versus the current settings.
3. WHEN resampling runs THEN it SHALL be measurably faster (CPU time) than the current
   configuration on the same buffer.
4. WHEN the source sample rate is already 16 kHz THEN the system SHALL skip resampling
   entirely (preserve current short-circuit).

## Non-functional

- Each change is independently shippable; they may be done in one PR or separate PRs.
- No change to Whisper/LLM parameters or models.

## Measurement (to validate, per the diagnostic's "signals to measure")

- Add timing around `stop_stream` (resample portion) and around the paste step to
  confirm the reductions. Compare before/after on the same machine.

## Coordination note (AGENTS.md)

Touches `src-tauri/src/audio.rs` and `src-tauri/src/pipeline.rs` (Claude-owned), and
possibly `src-tauri/src/event_tap.rs` for the paste-readiness check. Kiro authored this
spec; coordinate before implementing.
