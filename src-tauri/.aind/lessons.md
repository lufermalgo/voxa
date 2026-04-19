# Lessons

## L1 — Claude must use git worktrees, never the shared working directory

Kiro works in the main project directory. If Claude runs `git stash` or `git checkout` there, it disrupts Kiro's in-progress work even if the stash preserves the files.
Pattern to avoid: `git stash && git checkout -b new-branch` in the shared dir.
Correct pattern: `git worktree add ../voxa-claude bugfix/issue-{id}-{desc}` → work there → `git worktree remove ../voxa-claude` after merge.
