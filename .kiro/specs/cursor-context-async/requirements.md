# Requirements Document

## Introduction

The diagnostic (finding B3) found that `get_cursor_context()` runs **synchronously inside
the CGEventTap callback** (`src-tauri/src/event_tap.rs`, `native_tap_callback`) on the
key-down that starts a dictation. That function makes **3–4 blocking macOS Accessibility
API calls** (`AXUIElementCreateSystemWide`, `AXUIElementCopyAttributeValue` ×3) against
the frontmost application.

Two concrete problems:
1. The CGEventTap callback is on the system input-handling path. Any blocking work there
   delays event processing and, if the target app's AX server is slow to respond, can
   stall the start of audio capture — **the user can lose the first words** of the dictation.
2. The cursor context is only consumed much later, at **refinement time** (LLM step in
   `pipeline.rs`), and only when cursor context is non-empty. So the expensive AX read is
   on the *start* critical path but its result isn't needed until the *end*.

This spec decouples cursor-context capture from the input event so the recording starts
immediately, while the context is still available by the time the LLM needs it.

## Glossary

- **AX / Accessibility API**: macOS `AXUIElement*` APIs used to read the focused element's
  text value and selection range.
- **Event-tap thread**: the thread running the `CGEventTap` callback (`native_tap_callback`)
  that processes global key events.
- **Cursor context**: up to 200 chars of text before (`pre_text`) and after (`post_text`)
  the cursor in the focused app, used by the LLM to match surrounding tone/formatting.
- **Generation counter**: a monotonically increasing id identifying the current dictation,
  used to discard stale async reads.

## Requirements

### Requirement 1: Recording start is not blocked by AX reads

**User Story:** As a user, when I press the dictation shortcut, recording starts
immediately so I never lose my first words.

#### Acceptance Criteria
1. WHEN the dictation start shortcut fires THEN the system SHALL initiate audio capture
   without waiting for Accessibility cursor-context reads to complete.
2. WHEN the event-tap callback handles the start key THEN it SHALL NOT perform blocking
   AX calls inline on the event-tap thread.
3. WHEN audio capture begins THEN the time from key-down to stream-active SHALL NOT depend
   on the responsiveness of the target application's AX server.

### Requirement 2: Cursor context still available at refinement time

**User Story:** As a user relying on context-aware formatting (matching surrounding text),
I want that feature to keep working.

#### Acceptance Criteria
1. WHEN cursor-context capture is moved off the event-tap thread THEN the captured
   `pre_text`/`post_text` SHALL still be stored in `CursorContext` before the LLM
   refinement step reads it.
2. WHEN the dictation is very short (user stops almost immediately) AND the async context
   read has not finished THEN the system SHALL proceed with empty context rather than
   block the pipeline, AND SHALL NOT crash or hang.
3. WHEN the async context read completes after refinement has already started THEN the
   late result SHALL be discarded for that dictation (no race that corrupts a later one).

### Requirement 3: Capture reflects the cursor at dictation start

**User Story:** As a user, the surrounding-text context should reflect where my cursor was
when I started talking, not where it moved later.

#### Acceptance Criteria
1. WHEN the start shortcut fires THEN the system SHALL capture (or snapshot the target for)
   the cursor context as of that moment, not at stop time.
2. WHEN the focused element/PID at start is recorded THEN the async read SHALL target that
   same app/element.

### Requirement 4: Correct association across rapid dictations

**User Story:** As a user doing several dictations in quick succession, each one should use
its own cursor context.

#### Acceptance Criteria
1. WHEN a new dictation starts THEN any pending/stale context read from a previous
   dictation SHALL NOT populate the new dictation's `CursorContext`.
2. WHEN context is stored THEN it SHALL be tagged/guarded (e.g. a session id or generation
   counter) so a slow previous read cannot overwrite the current session's value.

## Out of scope

- Changing what the LLM does with the context (prompt format unchanged).
- Non-macOS platforms (cursor context is already a macOS-only no-op).

## Coordination note (AGENTS.md)

Touches `src-tauri/src/event_tap.rs` and `src-tauri/src/pipeline.rs` (Claude-owned).
Kiro authored this spec at the repo owner's request; coordinate before implementing.
