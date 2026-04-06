# Contributing to Voxa

Thank you for your interest in contributing to Voxa.

Voxa is a privacy-first, local-first dictation tool focused on speed, design quality, and native-feeling system-wide behavior. The project combines Tauri, Rust, React, and native macOS event handling. That means contributions are welcome, but changes should be deliberate, well-scoped, and easy to review.

## Before You Start

Please read these documents first:

- `README.md`
- `ROADMAP.md`
- `CODE_OF_CONDUCT.md`
- the relevant issue before opening a pull request

## Ways to Contribute

You do not need to be an expert in Rust to help.

Useful contributions include:

- bug reports with reproducible steps
- testing on macOS hardware and reporting edge cases
- UX feedback for settings, onboarding, and shortcut flows
- documentation improvements
- focused code changes tied to an issue
- performance and stability investigations

## Contribution Principles

### 1. Small pull requests win
Open focused pull requests. Do not mix refactors, features, and styling changes in one PR.

### 2. Discuss before large changes
If the change affects architecture, native event handling, core UX, or product direction, open an issue or discussion first.

### 3. Preserve product intent
Voxa is not trying to become a generic voice assistant. Contributions should support the product direction:

- fast system-wide dictation
- local-first execution
- minimal but premium UX
- strong privacy posture
- native-feeling desktop behavior

### 4. Prioritize clarity over cleverness
Readable code, explicit comments where needed, and predictable behavior are preferred over “smart” abstractions.

## Development Setup

### Prerequisites

- Node.js 18+
- Rust toolchain
- Tauri v2 prerequisites
- macOS is currently the primary target for native shortcut behavior

### Install

```bash
npm install
npm run tauri dev
```

## Branching

Create a branch from `main`.

Suggested naming:

- `fix/<short-description>`
- `feat/<short-description>`
- `docs/<short-description>`
- `chore/<short-description>`

Examples:

- `fix/mic-key-capture`
- `docs/shortcut-debugging-guide`

## Issues First

For anything beyond a typo or trivial docs fix, work from an issue.

A good issue should include:

- problem statement
- current behavior
- expected behavior
- steps to reproduce or validate
- acceptance criteria

## Pull Request Rules

Each PR should:

- reference the issue it resolves
- explain what changed and why
- describe how it was tested
- include screenshots or recordings for UI changes when relevant
- update documentation when behavior changes

### Recommended PR structure

- **Problem**
- **Approach**
- **Changes made**
- **How to test**
- **Risks / follow-ups**

## What Not to Submit

Please avoid:

- broad refactors without prior discussion
- changes that introduce cloud dependency by default
- unrelated formatting churn
- speculative architectural rewrites with no issue context
- pull requests that bundle many unrelated fixes

## Review Expectations

Maintainers may request:

- a smaller PR
- additional tests
- better reproduction steps
- documentation updates
- architectural clarification before merge

Not every contribution will be merged. Rejection is usually about scope, timing, or product direction—not effort.

## Security and Privacy

Because Voxa handles microphone input and system-wide behavior:

- avoid logging sensitive content unnecessarily
- do not add telemetry by default
- document any permission-sensitive macOS behavior clearly
- flag any privacy or security concern through a GitHub issue marked `security` until a dedicated process is published

## Community Norms

Be precise, respectful, and evidence-driven.

Good collaboration in Voxa means:

- reporting facts clearly
- challenging assumptions with evidence
- keeping debate technical and useful
- helping maintain product coherence

Thanks for helping improve Voxa.
