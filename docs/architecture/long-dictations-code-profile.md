# Long Dictations in the Code Profile

> Design decision for issue #23. Depends on #22 (recording duration limit).

## Problem

When a recording auto-stops at the duration limit (#22), the captured audio
runs through the full pipeline: Whisper transcription → LLM transformation →
paste. The pipeline processes **one dictation as a single LLM call** — there is
no cross-dictation context.

For prose profiles (Email, Informal, Elegant) this is fine: each chunk is a
self-contained sentence or paragraph. For the **Code** profile it is not — a
function description split across two auto-stops produces two disconnected,
semantically wrong code blocks, because the second LLM call has no memory of the
first.

## Decision

**Strategy: a single, user-configurable recording limit. No per-profile
chunking or buffering.**

The recording limit is now configurable from Settings → General
(`max_recording_seconds`, default 60s, range 30s–600s). This resolves the
problem at its source: a user dictating code raises the limit so the **entire
idea is captured in one take**, which becomes one transcription and one LLM
call. No mid-idea split, no lost context.

This corresponds to design question #1 in the issue ("should Code profile
disable auto-stop / allow longer dictations?"), generalized to all profiles
instead of special-casing Code.

### Why not the alternatives

- **Buffer chunks and concatenate before a single LLM call** (Q2): requires
  multi-dictation state in the pipeline state machine and audio/transcript
  stitching across separate hotkey presses. High complexity for a case the
  configurable limit already covers.
- **Paste raw Whisper output on mid-idea auto-stop for Code** (Q3): degrades
  output quality and adds profile-specific branching to the paste path.
- **Per-profile max durations** (Q4): more configuration surface and UI for
  marginal benefit. A single global limit the user controls is simpler and
  predictable.

### Trade-off accepted

A longer limit means a longer dictation can still approach the LLM
`--ctx-size` budget (~4096 tokens). This is left to the user's judgement: the
existing visual cues from #22 (amber pill at 80%, "wrap up" popup at 90%) signal
when to finish. A complete function description is estimated at 300–800 tokens,
well within budget for practical dictations.

## Acceptance criteria

- [x] Decision documented (this file).
- [x] Code change shipped: configurable limit (PR #82). No further
  implementation issue required — the configurable limit is the implementation.
- [x] No regression for Code dictations under the limit: the pipeline path is
  unchanged; only the auto-stop threshold became configurable.

## Related

- #22 — recording duration limit with visual warning (closed)
- PR #82 — configurable dictation time limit
- `src/hooks/useRecordingDuration.ts` — auto-stop at `maxSeconds`
- `src/components/SettingsPanel.tsx` — General tab stepper (30s–600s)
- `src-tauri/src/db.rs` — `max_recording_seconds` default seed
