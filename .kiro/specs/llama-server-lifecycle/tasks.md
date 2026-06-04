# Implementation Plan — llama-server lifecycle & memory hygiene

## Overview

Implement deterministic `llama-server` lifecycle: reap orphans on startup, keep one server
per instance, terminate on shutdown signals, and skip spawning when `bypass_llm` is on.
All files touched are Claude-owned — coordinate per AGENTS.md.

## Tasks

- [ ] 1. Add process-ownership helpers
  - Create `find_voxa_llama_servers()` that enumerates running `llama-server` processes
    whose args contain the Voxa model path; return their PIDs.
  - Add `terminate_pid(pid)` with SIGTERM → grace → SIGKILL escalation.
  - Add a unit test: a decoy command line containing `llama-server` but a different model
    path is NOT matched; the real Voxa path IS matched.
  - _Requirements: 1.1, 1.3, 2.1_

- [ ] 2. Expose the child PID and write a PID file
  - Add `LlamaEngine::pid(&self) -> u32`.
  - On successful `LlamaEngine::new`, write the PID to `<app_data_dir>/llama-server.pid`.
  - On `Drop` and on explicit termination, remove the PID file.
  - Unit test the PID-file write/read/remove round-trip.
  - _Requirements: 2.3_

- [ ] 3. Startup reconciliation pass
  - In `lib.rs` setup, before pre-warm, on a background thread: read the PID file and
    reap a live Voxa-owned PID, then enumerate-and-reap any remaining Voxa-owned orphans.
  - Wrap in error handling that logs and continues on any failure (never block launch).
  - Ensure it runs off the main thread and does not delay window/shortcut setup.
  - _Requirements: 1.1, 1.2, 1.4, 5.1_

- [ ] 4. Gate pre-warm on `bypass_llm`
  - In the pre-warm thread, read `bypass_llm` from `SettingsCache`; if `true`, skip
    spawning the server (log info). Leave the Whisper warm-up untouched.
  - Verify the existing lazy-start path in `pipeline.rs` still starts the server when
    `bypass_llm` is later turned off (no code dup; add a comment if confirmed).
  - _Requirements: 4.1, 4.2, 4.3, 5.2_

- [ ] 5. Terminate child on shutdown signals
  - Handle `tauri::RunEvent` exit to terminate the current child and remove the PID file.
  - Add a SIGTERM/SIGINT handler doing the same; keep `Drop` as fallback.
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 6. Single-instance guarantee on respawn
  - In `pipeline.rs`, when `is_alive()` is false, ensure the old PID (from handle or PID
    file) is terminated before spawning a replacement.
  - _Requirements: 2.1, 2.2_

- [ ] 7. Verify & document
  - Run `cargo check` (must be clean).
  - Execute the manual verification checklist from design.md (crash-reap, bypass-no-spawn,
    clean-quit-stops-server) and record results in the PR.
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 5.1, 5.2_

## Task Dependency Graph

```json
{
  "waves": [
    { "wave": 1, "tasks": ["1", "2", "4"] },
    { "wave": 2, "tasks": ["3", "5", "6"] },
    { "wave": 3, "tasks": ["7"] }
  ]
}
```

- Wave 1: Task 1 (helpers), Task 2 (PID file), Task 4 (bypass gate) are independent
  foundations.
- Wave 2: Task 3 (startup reconciliation) needs 1+2; Task 5 (shutdown) needs 2; Task 6
  (respawn) needs 1.
- Wave 3: Task 7 (verify) is last.

## Notes
- Files touched: `src-tauri/src/llama_inference.rs`, `src-tauri/src/models.rs`,
  `src-tauri/src/lib.rs`, `src-tauri/src/pipeline.rs` — all Claude-owned. Coordinate per
  AGENTS.md before implementing.
- One issue = one branch = one PR → `main`. Suggested branch:
  `feature/issue-XX-llama-server-lifecycle`.
