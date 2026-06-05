# AGENTS.md

Conventions and engineering rules for AI agents working on this repository.
This project is developed with **Kiro** using a Vibe Coding / AI-first approach.

---

## Context: How this project is built

Voxa is developed entirely with AI assistance — no manual coding. It started as an
experiment to understand Vibe Coding in practice: how to write better specs, how to
manage session state across context windows, how to structure agent instructions to
get consistent, high-quality output while minimizing wasted tokens.

The rules in this file are the result of that experiment. They are not theoretical —
every section reflects a real pattern learned from building this project.

---

## 0. Project Architecture & Code Map

Read this section at the start of every session. It gives you enough context to work
on any part of the codebase without needing to explore files blindly.

### Data flow — the full pipeline

```
Hotkey press (CGEventTap)
  → event_tap.rs          — captures global keyboard events at session level
  → pipeline.rs           — orchestrates the full dictation lifecycle via MPSC channel
  → audio.rs              — records audio (cpal), applies VAD, resamples to 16 kHz
  → whisper_inference.rs  — local STT via whisper.cpp (GPU-accelerated on Apple Silicon)
  → llama_inference.rs    — local LLM refinement via llama-server HTTP API
  → commands.rs           — injects result into clipboard + triggers Cmd+V paste
  → target app            — text appears at cursor
```

The pipeline uses a decoupled **MPSC channel** (`DictationEvent`) so the UI stays
responsive during transcription. The LLM runs as a `llama-server` subprocess on a
local port — managed by `proc.rs` to prevent orphaned processes.

### Key design decisions (know these before writing code)

- **No Dock icon** — runs as `NSApplicationActivationPolicyAccessory`. Never steals focus.
- **Native event tap** — `CGEventTap` at session level, not Tauri's global shortcut plugin
  (which fails for system-reserved keys like `Alt+Space`).
- **Focus preservation** — stores the frontmost app PID via `NSWorkspace` before any Voxa
  window appears, then restores it by PID. Works with Electron and JVM targets.
- **LLM subprocess** — `llama-server` is spawned as a child process. Orphan detection and
  reaping is handled in `proc.rs` — never do a blanket `killall llama-server`.
- **Audio silence detection** — uses peak amplitude, not RMS, to avoid false negatives on
  low-volume speech.
- **Model selection** — auto-selects Qwen2.5-3B (Apple Silicon) or Qwen2.5-1.5B (Intel).

---

### Project folder structure

```
voxa/
├── src/                          # Frontend — React + TypeScript (Vite)
│   ├── components/
│   │   ├── RecorderPill.tsx      # Floating dictation pill (the main UI element)
│   │   ├── SettingsPanel.tsx     # Full settings screen
│   │   ├── ProfilePicker.tsx     # Profile selection dropdown
│   │   ├── ProfilePill.tsx       # Active profile indicator
│   │   └── TrayMenu.tsx          # System tray menu
│   ├── hooks/
│   │   ├── useAudioLevel.ts      # Mic level → pill animation
│   │   ├── useProfiles.ts        # Profile CRUD and active profile state
│   │   ├── useRecordingDuration.ts # Recording timer
│   │   ├── useSettings.ts        # App settings read/write
│   │   └── useTranscription.ts   # Transcription state and history
│   ├── App.tsx                   # Root component and routing
│   └── i18n.ts                   # Internationalisation (en/es)
│
├── src-tauri/                    # Backend — Rust (Tauri v2)
│   └── src/
│       ├── lib.rs                # App entry point, Tauri setup, state registration
│       ├── pipeline.rs           # Dictation lifecycle orchestrator (MPSC channel hub)
│       ├── audio.rs              # Audio capture (cpal), VAD integration, resampling
│       ├── whisper_inference.rs  # Whisper STT engine (whisper-rs, persistent state)
│       ├── llama_inference.rs    # LLM refinement via llama-server HTTP API
│       ├── models.rs             # Model download, verification, path resolution
│       ├── db.rs                 # SQLite — transcripts, settings, profiles, dictionary
│       ├── commands.rs           # All #[tauri::command] handlers (except shortcuts)
│       ├── shortcuts.rs          # Shortcut registration + NATIVE_SHORTCUTS global state
│       ├── event_tap.rs          # CGEventTap — native macOS global keyboard capture
│       ├── tray.rs               # System tray icon and menu
│       ├── vad.rs                # Silero VAD engine (ONNX/ort) — speech detection
│       ├── proc.rs               # llama-server lifecycle — spawn, reap, orphan detection
│       ├── formatting.rs         # LLM formatting block builder (markdown/plain, i18n hints)
│       ├── window_utils.rs       # Pill position calculation (centered, above Dock)
│       └── main.rs               # Binary entry point
│
├── .kiro/
│   ├── specs/                    # Feature specs (requirements + design + tasks)
│   └── hooks/                    # Agent automation hooks
│
├── _tools/
│   └── SESSION_STATUS.md         # Active session state — read this first every session
│
├── docs/
│   ├── architecture/             # Deep-dive technical docs for specific subsystems
│   └── brand/                    # Brand identity and design tokens
│
├── AGENTS.md                     # This file — agent rules and project context
└── README.md                     # User-facing documentation
```

---

## 1. Token Efficiency — The Fundamental Principle

- **Read before writing**: inspect existing code before modifying. Never write blind.
- **Edit, don't rewrite**: targeted edits only. Full rewrites require explicit justification.
- **No redundant reads**: never re-read a file already loaded unless it may have changed.
- **Surgical reads**: only the file being edited, only the relevant lines. No project-wide scans.
- **Focused diffs**: touch only what the task requires. Zero side edits.
- **Lazy loading**: pull context just-in-time, never preemptively.
- **Platform capabilities**: use Kiro's native tools — specs, hooks, steering files — never
  do manually what the platform can do automatically.
- **Subagents**: use for exploration, research, or parallel independent work. Never for
  single-file edits. Instructions must be self-contained and scoped. Pass only relevant
  excerpts, never full files. Validate result; retry once with a sharper prompt if wrong;
  handle inline if it fails twice.

---

## 2. Session Memory & State Management

Session state lives in `_tools/SESSION_STATUS.md`. This is the single source of truth
for what is currently in progress.

**Read it at the start of every session.** A hook fires on every prompt to enforce this
(see Section 7 — Hooks).

**Update it whenever:**
- You start a new branch or issue
- You open a PR
- You finish a rebase
- You go idle

**Required fields:**
- Current branch
- Status: `idle` | `in-progress` | `pr-open` | `awaiting-review` | `blocked`
- Files being touched
- Pending PR number (if any)

**Why `_tools/` and not `.kiro/`?**
`_tools/SESSION_STATUS.md` is tracked in git and readable by any tool or agent.
It is the evolved replacement for the old `.kiro/status.md` multi-agent coordination
file, which is now deprecated (Kiro is the sole agent on this project).

---

## 3. Spec-Driven Development (SDD) with Kiro

Features are built from specs, not from ad-hoc prompts. The workflow is:

```
Idea → Spec (requirements + design + tasks) → Implementation → PR → main
```

### Spec location

All specs live in `.kiro/specs/[module-name]/`. Each spec folder contains:

| File | Purpose |
|------|---------|
| `requirements.md` | What the feature must do — user-facing behavior, acceptance criteria |
| `design.md` | How it will be built — architecture, data flow, key decisions |
| `tasks.md` | Ordered implementation tasks with status tracking |

### How to create a spec in Kiro

1. Open the Kiro Spec panel (or use the command palette → "New Spec")
2. Describe the feature in natural language — Kiro generates the initial spec
3. Iterate on `requirements.md` until behavior is fully defined
4. Review `design.md` — validate technical approach before any code is written
5. Work through `tasks.md` in order — one task = one atomic unit of work

### Spec discipline rules

- **No code before the spec is validated.** Writing code to "figure out" the design
  is a token and time sink. Define behavior first.
- **One spec per feature/module.** Don't accumulate multiple features in one spec.
- **Tasks must be atomic.** Each task in `tasks.md` should be completable in one
  focused session without needing to re-read the whole spec.
- **Acceptance criteria before implementation.** Define what "done" looks like before
  writing a single line of code.
- **Keep specs current.** Update task status as work progresses. A stale spec
  misleads the next session.

---

## 4. TDD — Test-Driven Development

- Define acceptance tests before writing implementation code.
- Never say "done" without confirming acceptance criteria pass.
- If tests can't run automatically, document exactly what to verify manually.
- Bug reports: diagnose root cause before touching code. Never patch symptoms.
- Test failure limit: 3 failed attempts → document the blocker in the task → surface
  to the human in plain language.

---

## 5. Session Hygiene

- **Cold start**: read `_tools/SESSION_STATUS.md` first. It tells you what was in
  progress and what the next step is.
- **Before ending a session**: update `_tools/SESSION_STATUS.md` with current branch,
  status, and next step. Never leave it stale.
- **Context budget**: when approaching context limits — summarize decisions, update
  session status, and tell the user to open a fresh session.
- **Hard rotation triggers** (open a new session immediately):
  - A single response would exceed 2K output tokens
  - A file to load exceeds 500 lines and only part is needed
  - More than 3 subagents ran in this session
  - The task requires loading 3+ large files simultaneously

---

## 6. Engineering Rigor

- Think before writing. Understand impact area first. Find the simplest correct solution.
- No side effects beyond the task scope. No unrelated edits.
- Regression-safe. Clean separation of concerns. Low coupling.
- Naming that documents intent. No dead code. Consistency with existing patterns.
- Defensive only at system boundaries.
- Surface cost, security, or compliance implications in one line. Never bury them.
- Before presenting non-trivial work: ask "is there a more elegant way?"
- `cargo check` must pass before opening any PR that touches Rust.
- `tsc --noEmit` must pass before opening any PR that touches TypeScript.

---

## 7. Hooks

Hooks automate agent behavior triggered by IDE events. They are defined in
`.kiro/hooks/` as `.kiro.hook` JSON files.

### Active hooks

| Hook file | Trigger | Action |
|-----------|---------|--------|
| `check-agent-status.kiro.hook` | Every prompt submit | Reads `_tools/SESSION_STATUS.md` and updates it if work state changed |

### Hook rules

- Hooks that read state files must do so **silently** — no output to the user unless
  something is blocked or conflicting.
- Hooks that run commands must be idempotent — safe to trigger multiple times.
- Never create a hook that writes to files the agent doesn't own.
- Test new hooks on a branch before committing them to `main`.

### Hook schema reference

```json
{
  "name": "string (required)",
  "version": "string (required)",
  "description": "string (optional)",
  "when": {
    "type": "promptSubmit | fileEdited | fileCreated | fileDeleted | agentStop | preToolUse | postToolUse | preTaskExecution | postTaskExecution | userTriggered",
    "patterns": ["glob patterns — required for file events only"],
    "toolTypes": ["read | write | shell | web | spec | * — required for preToolUse/postToolUse"]
  },
  "then": {
    "type": "askAgent | runCommand",
    "prompt": "string — required for askAgent",
    "command": "string — required for runCommand"
  }
}
```

---

## 8. Branch Naming

| Type    | Pattern                            | Example                           |
|---------|------------------------------------|-----------------------------------|
| Feature | `feature/issue-{id}-{short-desc}`  | `feature/issue-12-silero-vad`     |
| Bugfix  | `bugfix/issue-{id}-{short-desc}`   | `bugfix/issue-34-paste-timing`    |
| Hotfix  | `hotfix/issue-{id}-{short-desc}`   | `hotfix/issue-56-security-patch`  |
| Docs / chore | `feature/{short-desc}`        | `feature/consolidate-agents-md`   |

**NEVER work on `main` or any shared branch directly.**

---

## 9. Git Workflow

```bash
# 1. Always start from latest main
git fetch origin main
git checkout -b feature/issue-{id}-{desc} origin/main

# 2. Implement, then commit
git add <specific files>
git commit -m "feat(scope): description

Closes #{id}"

# 3. Rebase before push — never merge
git fetch origin main
git rebase origin/main

# 4. Push and open PR → main
git push -u origin feature/issue-{id}-{desc}
gh pr create --title "feat: description" --body "Closes #{id}"
```

---

## 10. Commit Format (Conventional Commits)

```
<type>(<scope>): <description>

Closes #<issue-id>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

---

## 11. PR Rules

- Every change requires a PR — no direct pushes to `main`
- PRs always target `main` directly
- One issue = one branch = one PR → merge to `main`
- Reference the issue: `Closes #id`
- If the PR depends on another: `Depends on: #id`
- `cargo check` must pass before opening a Rust PR
- Stage specific files — never `git add .` blindly

---

## 12. Domain Ownership

| Domain | Files |
|--------|-------|
| Audio pipeline | `src-tauri/src/audio.rs` |
| Whisper / transcription | `src-tauri/src/whisper_inference.rs` |
| LLM / refinement | `src-tauri/src/llama_inference.rs` |
| Models / download | `src-tauri/src/models.rs` |
| Database / settings | `src-tauri/src/db.rs` |
| App core / commands / pipeline | `src-tauri/src/lib.rs`, `src-tauri/src/pipeline.rs` |
| Frontend — pill | `src/components/RecorderPill.tsx` |
| Frontend — settings UI | `src/components/SettingsPanel.tsx` |
| Frontend — hooks | `src/hooks/` |
| i18n | `src/i18n.ts` |

---

## 13. Directory Structure & Ownership

| Directory | Purpose | Rule |
|-----------|---------|------|
| `.kiro/specs/` | Feature specs (requirements + design + tasks) | Created and maintained by Kiro |
| `.kiro/hooks/` | Agent automation hooks | Created and maintained by Kiro |
| `_tools/` | Session state and tooling notes | Shared — `SESSION_STATUS.md` is the session memory file |

---

## 14. Anti-Patterns

- **Coding without a spec** → undefined behavior, wasted tokens, rework
- **Full file rewrites** when a targeted edit suffices → token waste, merge conflicts
- **Patching symptoms** instead of diagnosing root cause → compounding bugs
- **Skipping `cargo check`** before a Rust PR → broken `main`
- **Direct push to `main`** → blocked by branch protection, and wrong by design
- **Commits without issue reference** → untraceable changes
- **Starting new work while `main` is broken** → compounding problems
- **Leaving `_tools/SESSION_STATUS.md` stale** → next session starts blind
- **Loading full files when only a section is needed** → context budget waste
- **Preemptive context loading** → loads files that may never be needed
- **Using subagents for single-file edits** → overhead without benefit
- **Merging PRs into feature branches** instead of `main` → hidden divergence

---

## 15. Coordination Rules (learned from real incidents)

1. **`main` must always compile.** `cargo check` before every Rust PR.
2. **Never merge a PR to a feature branch and call it done.** PRs go to `main`.
3. **Before starting any task, sync with main:**
   ```bash
   git fetch origin main
   git rebase origin/main
   ```
4. **If `main` is broken, stop and fix it first.**
5. **Clean up branches after merge.** Delete remote branches once their PR is merged.
6. **Read `_tools/SESSION_STATUS.md` before every task.** The hook does this automatically,
   but it's your responsibility to keep it accurate.
