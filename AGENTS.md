# AGENTS.md

Conventions for AI agents (and humans) working on this repository.

## Branch Naming

| Type    | Pattern                            | Example                          |
|---------|------------------------------------|----------------------------------|
| Feature | `feature/issue-{id}-{short-desc}`  | `feature/issue-12-silero-vad`    |
| Bugfix  | `bugfix/issue-{id}-{short-desc}`   | `bugfix/issue-34-paste-timing`   |
| Hotfix  | `hotfix/issue-{id}-{short-desc}`   | `hotfix/issue-56-security-patch` |
| Docs    | `docs/issue-{id}-{short-desc}`     | `docs/issue-7-readme-update`     |

**NEVER work on `main` or any shared branch directly.**

## Workflow

```bash
# 1. Start from latest main
git fetch origin main
git checkout -b feature/issue-{id}-{desc} origin/main

# 2. Implement, then commit
git commit -m "feat(scope): description

Closes #{id}"

# 3. Rebase before push
git fetch origin main
git rebase origin/main

# 4. Push and open PR
git push -u origin feature/issue-{id}-{desc}
gh pr create --title "feat: description" --body "Closes #{id}"
```

## Commit Format (Conventional Commits)

```
<type>(<scope>): <description>

Closes #<issue-id>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

## PR Rules

- Every change requires a PR — no direct pushes to `main`
- Reference the issue in the PR body: `Closes #id`
- If the PR depends on another: add `Depends on: #id`

## Domain Ownership

Assign one agent per domain to avoid overlap:

| Domain                         | Files                                              |
|--------------------------------|----------------------------------------------------|
| Audio pipeline                 | `src-tauri/src/audio.rs`                           |
| Whisper / transcription        | `src-tauri/src/whisper_inference.rs`               |
| LLM / refinement               | `src-tauri/src/llama_inference.rs`                 |
| Models / download              | `src-tauri/src/models.rs`                          |
| Database / settings            | `src-tauri/src/db.rs`                              |
| App core / commands / pipeline | `src-tauri/src/lib.rs`                             |
| Frontend — pill                | `src/components/RecorderPill.tsx`                  |
| Frontend — settings            | `src/components/SettingsPanel.tsx`                 |

## Anti-Patterns

- Multiple agents on the same branch → conflict chaos
- Skipping rebase → divergence from main
- Direct push to main → breaks production
- Commits without issue reference → untraceable changes
