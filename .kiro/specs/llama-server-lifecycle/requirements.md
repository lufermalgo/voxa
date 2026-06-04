# Requirements Document

## Introduction

The runtime diagnostic (2026-06-04) found **8 orphaned `llama-server` processes**
alive on the machine (parent PID `1`, i.e. reparented to `launchd`), each holding
**~950 MB RSS**, the oldest alive for 6 days — **~7.6 GB of RAM held hostage**.
Because every server is spawned with `--mlock`, that memory is **wired** (non-pageable),
so the OS cannot reclaim it. On a 24 GB unified-memory machine this is ~32% of RAM,
and it directly degrades every other workload, including Whisper/LLM inference itself
(memory pressure, page-outs observed).

Root cause: `LlamaEngine` only kills its child `llama-server` in its `Drop` impl
(`src-tauri/src/llama_inference.rs`). `Drop` runs only on a *clean* shutdown of the
Voxa parent. On hard quit, crash, `SIGKILL`, or a panic in the Tauri runtime, the
child is orphaned and never reaped. Successive Voxa launches each spawn a fresh server
on a new random port (`find_free_port`), so orphans accumulate across sessions.

A secondary waste (finding B8): the startup pre-warm in `src-tauri/src/lib.rs` spawns
`llama-server` **unconditionally**, even when the user has `bypass_llm` enabled and the
LLM will never be used — reserving ~950 MB for nothing.

This spec covers making the `llama-server` lifecycle deterministic: at most one server
per running Voxa instance, no orphans surviving across sessions, and no server started
when the LLM is bypassed.

## Glossary

- **Orphaned server**: a `llama-server` process whose spawning Voxa instance has exited,
  now reparented to `launchd` (ppid = 1).
- **Voxa-owned server**: a `llama-server` whose command line references the Voxa model
  path under the app data dir (`…/com.lufermalgo.voxa/models/`). Used to distinguish our
  processes from any `llama-server` the user runs independently.

## Requirements

### Requirement 1: Reap stale Voxa-owned servers on startup

**User Story:** As a user who restarts Voxa (or whose Voxa crashed), I want previous
`llama-server` processes cleaned up automatically, so that memory is not leaked across
sessions.

#### Acceptance Criteria
1. WHEN Voxa starts up THEN the system SHALL identify any running `llama-server` processes
   that are Voxa-owned (command line references the Voxa model path) and not the current
   instance's child.
2. WHEN a stale Voxa-owned `llama-server` is identified at startup THEN the system SHALL
   terminate it before (or instead of) spawning a new one.
3. WHEN identifying processes to terminate THEN the system SHALL NOT terminate
   `llama-server` processes that are not Voxa-owned (e.g. a user's own llama.cpp server),
   verified by matching the Voxa model path in the process arguments.
4. IF process enumeration fails THEN the system SHALL log a warning and continue startup
   without terminating anything (fail safe, never block launch).

### Requirement 2: At most one live server per Voxa instance

**User Story:** As a user, I want only one refinement server running at a time, so that
memory use stays bounded.

#### Acceptance Criteria
1. WHEN the pipeline detects the existing server is no longer alive (`is_alive()` false)
   THEN the system SHALL explicitly terminate the old process handle before spawning a
   replacement.
2. WHEN a new `LlamaEngine` is created while a previous one exists in `EngineState`
   THEN the system SHALL ensure the previous server process is terminated.
3. WHEN the server's PID is known THEN the system SHALL persist it (e.g. a PID file under
   the app data dir) so a future startup can reap it even after an unclean exit.

### Requirement 3: Terminate the child on app shutdown signals

**User Story:** As a user who quits Voxa (including via Cmd+Q or the tray), I want the
server stopped, so that nothing lingers.

#### Acceptance Criteria
1. WHEN Voxa receives a normal termination (window/tray exit path) THEN the system SHALL
   terminate the `llama-server` child.
2. WHEN Voxa receives `SIGTERM` or `SIGINT` THEN the system SHALL attempt to terminate the
   `llama-server` child before exiting.
3. WHILE the existing `Drop` impl on `LlamaEngine` remains THE system SHALL keep it as a
   best-effort fallback (not the sole mechanism).

### Requirement 4: Do not start the server when the LLM is bypassed (B8)

**User Story:** As a user who runs with `bypass_llm` enabled, I do not want ~950 MB
reserved for a model I never use.

#### Acceptance Criteria
1. WHEN `bypass_llm` is `true` at startup THEN the system SHALL NOT pre-warm `llama-server`.
2. WHEN `bypass_llm` is toggled from `true` to `false` at runtime THEN the system SHALL be
   able to start the server lazily on the next dictation (existing lazy path in
   `pipeline.rs` already covers this — verify it still works).
3. WHEN `bypass_llm` is `true` THEN the system SHALL still allow Whisper to load/warm
   normally (only the LLM is gated).

### Requirement 5: No regression to first-dictation latency

**User Story:** As a user, I do not want this cleanup work to make my first dictation slower.

#### Acceptance Criteria
1. WHEN cleanup runs at startup THEN it SHALL run off the UI/main thread and SHALL NOT
   delay window display or shortcut registration.
2. WHEN `bypass_llm` is `false` THEN the pre-warm behavior SHALL remain (server ready
   before first dictation in the common case).

## Out of scope

- Changing the inference parameters (`-ngl`, `--flash-attn`, `--mlock`, ctx-size).
- Switching the LLM model (covered by separate hardware/model work).

## Coordination note (AGENTS.md)

Implementation touches `src-tauri/src/llama_inference.rs`, `src-tauri/src/models.rs`,
and `src-tauri/src/lib.rs` — all **Claude-owned** domains. Coordinate before editing;
Kiro authored this spec at the repo owner's request.
