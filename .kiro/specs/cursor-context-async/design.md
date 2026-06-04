# Design Document — Cursor-context capture off the input critical path

## Overview

Move the blocking Accessibility (AX) cursor-context read out of the CGEventTap callback so
that pressing the dictation shortcut starts audio capture immediately. The read happens on
a separate thread and stores its result into `CursorContext`, guarded by a generation
counter so it always belongs to the correct dictation.

### Current behavior (as-is)

In `event_tap.rs::native_tap_callback`, on the start key match:
```
let (pre, post) = get_cursor_context();   // 3-4 blocking AX calls, on the tap thread
event_to_send = Some(DictationEvent::StartRecording { pre_text: pre, post_text: post });
```
The event flows over the `mpsc` channel to the pipeline loop, which stores pre/post into
`CursorContext`. At StopRecording the pipeline reads `CursorContext` for the LLM step.

So the AX read is paid at the worst possible time (start, on the input thread) for a value
needed only at the end.

## Architecture

```
key-down (event-tap thread)
  └─ emit StartRecording immediately (NO AX read inline)
pipeline StartRecording handler
  ├─ gen = generation.fetch_add(1)+1 ; clear pre/post
  └─ spawn short-lived thread:
        (pre,post) = get_cursor_context()      // AX I/O off the input path
        if generation == gen { store pre/post } // else discard (stale)
key-up → StopRecording
  └─ read pre/post (may be empty if read unfinished) → LLM ; never blocks
```

## Components and Interfaces

### Component 1 — Event-tap callback (`event_tap.rs`)
- Remove the inline `get_cursor_context()` call on the start path; emit `StartRecording`
  without performing AX work on the tap thread.

### Component 2 — Async reader (in the StartRecording handler, `pipeline.rs`)
- Capture the current generation, spawn a thread running `get_cursor_context()`, and store
  the result only if the generation still matches.

### Component 3 — StopRecording handler (`pipeline.rs`)
- Reads `pre_text`/`post_text` without waiting; empty context is acceptable for very short
  dictations.

### Design options (decide during implementation)
- **A (preferred):** `StartRecording` is a lightweight signal; the AX read is spawned from
  the StartRecording handler. Minimal change to the tap callback.
- **B:** capture only the focused PID/element ref cheaply on the tap thread and pass it
  through; do the text extraction async. Use only if start-time focus correctness (Req 3.2)
  proves insufficient with option A.

## Data Models

`CursorContext` gains a generation counter:
```rust
struct CursorContext {
    pre_text:  Mutex<String>,
    post_text: Mutex<String>,
    generation: AtomicU64,   // NEW
}
```
No DB or settings changes. `DictationEvent::StartRecording` may drop its `pre_text`/
`post_text` fields (option A) or keep a cheap element ref (option B).

## Correctness Properties

### Property 1: No AX work on the input thread
No AX call executes on the event-tap thread for the start key.
**Validates: Requirements 1.1, 1.2, 1.3**

### Property 2: Context belongs to its dictation
A stored cursor context always belongs to the dictation whose generation matches; a stale
read (older generation) never overwrites a newer dictation's context.
**Validates: Requirements 4.1, 4.2**

### Property 3: Stop never blocks
StopRecording never blocks waiting for the async read.
**Validates: Requirements 2.2, 2.3**

### Property 4: Graceful failure
If the AX read fails or is unfinished, the pipeline proceeds with empty context and does
not crash or hang.
**Validates: Requirements 2.2, 2.3**

## Error Handling

| Case | Behavior |
|------|----------|
| AX read fails / not permitted | Store empty strings (current `get_cursor_context` behavior). |
| Read finishes after stop | If generation matches, store (harmless, unused); else discard. |
| Read still pending at stop | Pipeline uses empty context, proceeds normally. |

## Concurrency notes

- The async thread must not hold the `CursorContext` locks while doing AX I/O — it only
  locks briefly to store the final strings.
- One short-lived thread per dictation start; no new long-lived threads.
- The `AtomicU64` generation counter is the single source of truth for ownership.

## Testing strategy

- Unit: generation-guard helper — a stale generation does not overwrite the current value;
  a matching generation does.
- Manual:
  1. In a slow-AX app (e.g. a heavy Electron app), press start and immediately speak;
     confirm first words are captured (compare against current build).
  2. Context-aware formatting still matches surrounding text on a multi-second dictation.
  3. Rapid back-to-back dictations: each uses its own context (or empty), never a previous
     one's text.
- `cargo check` clean; no frontend change.

## Risks

- Subtle race if the generation guard is wrong → mitigated by a single `AtomicU64` and
  clearing pre/post on start.
- Users relying on context: ensure the async read completes before typical (multi-second)
  dictations end — it will, AX reads are ~ms.
