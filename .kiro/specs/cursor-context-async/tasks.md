# Implementation Plan — Cursor-context capture off the input critical path

## Overview

Remove the blocking AX cursor-context read from the event-tap callback and perform it
asynchronously, guarded by a generation counter. Files touched are Claude-owned —
coordinate per AGENTS.md.

## Tasks

- [ ] 1. Add a generation counter to CursorContext
  - Add `generation: AtomicU64` to the `CursorContext` struct in `pipeline.rs`.
  - On StartRecording: increment generation, clear `pre_text`/`post_text`.
  - _Requirements: 4.1, 4.2_

- [ ] 2. Remove the blocking AX read from the event-tap callback
  - In `event_tap.rs::native_tap_callback`, stop calling `get_cursor_context()` inline on
    the start key path; emit `StartRecording` immediately.
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 3. Spawn the async cursor-context read on StartRecording
  - In the StartRecording handler (pipeline.rs), capture the current generation, spawn a
    short-lived thread that runs `get_cursor_context()` and stores the result only if the
    generation still matches.
  - _Requirements: 2.1, 3.1, 3.2, 4.1, 4.2_

- [ ] 4. Keep StopRecording non-blocking
  - Confirm StopRecording reads `pre_text`/`post_text` without waiting for the async read;
    empty context is acceptable for very short dictations.
  - _Requirements: 2.2, 2.3_

- [ ] 5. Unit test the generation guard
  - Test that a stale generation does not overwrite the current context and a matching one
    does.
  - _Requirements: 4.1, 4.2_

- [ ] 6. Verify & document
  - `cargo check` clean.
  - Manual checklist from design.md (slow-AX app first-words, context still matches, rapid
    dictations isolation). Record in PR.
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 4.1, 4.2_

## Task Dependency Graph

```json
{
  "waves": [
    { "wave": 1, "tasks": ["1", "2"] },
    { "wave": 2, "tasks": ["3", "5"] },
    { "wave": 3, "tasks": ["4"] },
    { "wave": 4, "tasks": ["6"] }
  ]
}
```

- Wave 1: Task 1 (generation counter) and Task 2 (remove inline read) are independent.
- Wave 2: Task 3 (async spawn) needs 1+2; Task 5 (unit test) needs 1.
- Wave 3: Task 4 (non-blocking stop) follows the async wiring.
- Wave 4: Task 6 (verify) is last.

## Notes
- Files touched: `src-tauri/src/event_tap.rs`, `src-tauri/src/pipeline.rs` (Claude-owned).
  Coordinate per AGENTS.md.
- Suggested branch: `feature/issue-XX-cursor-context-async`.
