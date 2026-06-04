# Design Document — llama-server lifecycle & memory hygiene

## Overview

Make the `llama-server` process lifecycle deterministic so that:
1. orphaned servers from prior sessions are reaped on startup,
2. only one Voxa-owned server runs at a time,
3. the child is terminated on shutdown signals (not only via `Drop`), and
4. no server is spawned when `bypass_llm` is enabled.

The approach is intentionally small and surgical. It adds process-ownership tracking
(a PID file) plus a startup reconciliation pass, and gates the pre-warm on the
`bypass_llm` setting. No inference parameters change.

### Current behavior (as-is)

- `LlamaEngine::new` (`llama_inference.rs`) spawns `llama-server` on a random free port,
  stores the `Child`, and kills it only in `impl Drop`.
- `lib.rs` setup spawns a background thread (+3s) that pre-warms the engine
  **unconditionally** if the model + server binary exist.
- `pipeline.rs` lazily creates the engine on first dictation if not pre-warmed, and on
  `is_alive() == false` it drops the stale handle (which triggers `Drop` → kill) and
  respawns. If the previous process was already orphaned (parent died), there is no handle
  to drop, so it is never killed.

## Architecture

Startup ordering, executed off the main thread:

```
app setup (lib.rs)
  └─ spawn reconciliation thread
       1. read llama-server.pid → if live & Voxa-owned, terminate
       2. enumerate Voxa-owned llama-server procs → terminate orphans
       3. if bypass_llm == false → existing pre-warm (spawn 1 server, write PID file)
          else → skip (Whisper warm-up still runs in its own thread)
runtime
  └─ pipeline.rs: on is_alive()==false, terminate old PID then respawn (one at a time)
shutdown
  └─ RunEvent exit / SIGTERM / SIGINT → terminate child + remove PID file
       (Drop remains as best-effort fallback)
```

Ownership guardrail: a process is only eligible for termination if its command-line args
contain the Voxa model path. A user's unrelated `llama-server` is never matched.

## Components and Interfaces

### Component 1 — Process ownership helper (new, in `models.rs` or a small `proc.rs`)
- `voxa_model_path_marker() -> String`: the absolute Voxa model path
  (`ModelManager::get_llama_path`), used to identify Voxa-owned servers.
- `find_voxa_llama_servers() -> Vec<u32>`: enumerate processes whose command line contains
  both `llama-server` and the Voxa model path. Implementation: `pgrep -f` or parse
  `ps -axo pid,command` (read-only).
- `terminate_pid(pid)`: SIGTERM, then SIGKILL after a short grace if still alive.

### Component 2 — PID file
- On successful `LlamaEngine::new`, write the child PID to
  `<app_data_dir>/llama-server.pid`.
- On clean termination (Drop / shutdown), remove the file.
- On startup, if the file names a live Voxa-owned PID, reap it.

### Component 3 — Startup reconciliation (in `lib.rs` setup, background thread)
- Runs steps 1–3 above; any failure logs a warning and continues (never blocks launch).

### Component 4 — Bypass gate
- Read `bypass_llm` from `SettingsCache` inside the pre-warm thread; if `true`, skip the
  server spawn. Whisper warm-up unaffected. The lazy path in `pipeline.rs` already starts
  the server on demand if `bypass_llm` is later turned off — verify, do not duplicate.

### Component 5 — Shutdown signal handling
- Add a `tauri::RunEvent` exit handler (and optionally a SIGTERM/SIGINT handler) that
  terminates the current child and removes the PID file. `Drop` stays as fallback.

### Interface additions
- `LlamaEngine::pid(&self) -> u32` to expose the child PID for the PID file and respawn
  logic.

## Data Models

- **PID file**: `<app_data_dir>/llama-server.pid`, plain text, a single integer PID.
- **`LlamaEngine`** gains a `pid` accessor; the held `Child` already carries the PID.
- No database schema change. No settings schema change (reads existing `bypass_llm`).

## Correctness Properties

### Property 1: At most one server
After startup reconciliation completes, at most one Voxa-owned `llama-server` is running
(the current instance's, or none if bypassed).
**Validates: Requirements 1.1, 1.2, 2.1, 2.2**

### Property 2: Ownership-scoped termination
No process lacking the Voxa model path in its args is ever terminated.
**Validates: Requirements 1.3**

### Property 3: PID file consistency
A successful `LlamaEngine::new` always results in a PID file naming the live child; a clean
termination always removes it.
**Validates: Requirements 2.3, 3.1**

### Property 4: Non-blocking startup
Reconciliation never blocks window display or shortcut registration.
**Validates: Requirements 5.1**

## Error Handling

| Failure | Behavior |
|---------|----------|
| Process enumeration fails | Log warn, continue (no termination). |
| PID file unreadable/corrupt | Ignore, fall back to enumeration. |
| SIGTERM doesn't stop process | Escalate to SIGKILL after grace (e.g. 500 ms). |
| Pre-warm skipped due to bypass | Info log; lazy path covers later use. |

## Security / safety

- Only terminate processes matching the Voxa model path (never a blanket
  `killall llama-server`). This is the critical guardrail (Requirement 1.3).
- All file writes confined to the app data dir.

## Testing strategy

- Unit: ownership matcher includes the Voxa model path and excludes a decoy command line
  that contains `llama-server` but a different model path.
- Unit: PID file write/read/remove round-trip.
- Manual (documented; CI runs only `pnpm tauri build`, no `cargo test`):
  1. Start Voxa, note server PID. Hard-kill Voxa (`kill -9`). Confirm orphan exists.
     Restart Voxa. Confirm orphan reaped and only one server runs.
  2. Enable `bypass_llm`, restart Voxa, confirm **no** `llama-server` spawns.
  3. Quit Voxa normally, confirm the server stops and the PID file is removed.

## Verification gates (AGENTS.md)

- `cargo check` clean before PR.
- `tsc --noEmit` unaffected (no frontend change expected).
- Confirm `main` compiles; touches Claude-owned Rust files — coordinate.
