# AGENTS.md

Conventions for AI agents (Kiro and Claude) working on this repository.

---

## Branch Naming

| Type    | Pattern                            | Example                          |
|---------|------------------------------------|----------------------------------|
| Feature | `feature/issue-{id}-{short-desc}`  | `feature/issue-12-silero-vad`    |
| Bugfix  | `bugfix/issue-{id}-{short-desc}`   | `bugfix/issue-34-paste-timing`   |
| Hotfix  | `hotfix/issue-{id}-{short-desc}`   | `hotfix/issue-56-security-patch` |

**NEVER work on `main` or any shared branch directly.**

---

## Workflow

```bash
# 1. Always start from latest main
git fetch origin main
git checkout -b feature/issue-{id}-{desc} origin/main

# 2. Implement, then commit
git commit -m "feat(scope): description

Closes #{id}"

# 3. Rebase before push (never merge)
git fetch origin main
git rebase origin/main

# 4. Push and open PR → main
git push -u origin feature/issue-{id}-{desc}
gh pr create --title "feat: description" --body "Closes #{id}"
```

---

## Commit Format (Conventional Commits)

```
<type>(<scope>): <description>

Closes #<issue-id>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

---

## PR Rules

- Every change requires a PR — no direct pushes to `main`
- PRs always target `main` directly — never accumulate PRs into a long-lived feature branch
- One issue = one branch = one PR → merge to `main`
- Reference the issue: `Closes #id`
- If the PR depends on another: `Depends on: #id`
- Verify `cargo check` passes before opening a Rust PR

---

## Domain Ownership (avoid simultaneous edits)

| Domain | Files | Owner |
|--------|-------|-------|
| Audio pipeline | `src-tauri/src/audio.rs` | Claude |
| Whisper / transcription | `src-tauri/src/whisper_inference.rs` | Claude |
| LLM / refinement | `src-tauri/src/llama_inference.rs` | Claude |
| Models / download | `src-tauri/src/models.rs` | Claude |
| Database / settings | `src-tauri/src/db.rs` | Claude |
| App core / commands / pipeline | `src-tauri/src/lib.rs`, `src-tauri/src/pipeline.rs` | Claude |
| Frontend — pill | `src/components/RecorderPill.tsx` | Claude |
| Frontend — settings UI | `src/components/SettingsPanel.tsx` | Kiro |
| Frontend — hooks | `src/hooks/` | shared (coordinate) |
| i18n | `src/i18n.ts` | shared (coordinate) |

When two agents need to touch the same file, the one who merges second resolves conflicts.

---

## Coordination Rules (learned from incident — April 2025)

1. **`main` must always compile.** Before opening a PR that touches Rust, run `cargo check` locally.
2. **Never merge a PR to a feature branch and call it done.** PRs go to `main`. If a feature branch is used as integration, it must itself be merged to `main` before starting dependent work.
3. **Before starting any task, sync with main:**
   ```bash
   git fetch origin main
   git rebase origin/main
   ```
4. **If main is broken, stop and fix it first** before opening new branches or writing new code.
5. **Clean up branches after merge.** Delete remote branches once their PR is merged to `main`.

---

## Anti-Patterns

- Multiple agents on the same branch → conflict chaos
- Merging PRs into a long-lived feature branch instead of `main` → hidden divergence
- Skipping `cargo check` before pushing Rust changes → broken `main`
- Direct push to `main` → blocked by branch protection
- Commits without issue reference → untraceable changes
- Starting new work while `main` is broken → compounding problems
