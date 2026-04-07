# Voxa Roadmap

This roadmap exists to make contribution more focused.

It is not a promise of delivery dates. It is a prioritization tool so contributors understand what matters now, what is being explored, and what is intentionally out of scope.

---

## Product Direction

Voxa is building toward a focused product:

- system-wide dictation
- local-first inference
- premium minimal UX
- reliable keyboard shortcut handling
- low-friction text injection into any app

The goal is not to become a general-purpose AI assistant. The goal is to make dictation feel fast, native, and trustworthy.

---

## Current Priorities

### 1. Shortcut Reliability on macOS

**Why it matters**
Shortcut reliability is foundational. If capture, assignment, or runtime activation feels inconsistent, the entire product loses trust.

**Priority areas**
- capture of special hardware keys such as Dictation / Microphone
- alignment between settings capture flow and persistent runtime event tap
- better diagnostics for Accessibility permissions and event paths
- safer fallback handling for reserved or non-standard keys

### 2. Stability of Recording and Processing Flow

**Why it matters**
Users need confidence that recording starts, stops, and processes consistently.

**Priority areas**
- clearer pipeline state transitions
- error reporting that is understandable to non-technical users
- resilience around model loading and audio device switching

### 3. UX Quality in Settings and Onboarding

**Why it matters**
A premium workflow is not only about inference quality. It is also about reducing friction.

**Priority areas**
- clearer shortcut assignment UX
- better permission guidance for macOS Accessibility and microphone access
- better defaults and reset paths
- improved discoverability for profiles, dictionary, and models

### 4. Documentation and Contributor Onboarding

**Why it matters**
A project that cannot be understood cannot attract useful contributors.

**Priority areas**
- setup instructions
- architecture notes
- issue templates
- focused contribution guide

---

## Near-Term Work

These are strong candidates for the next cycle of work.

- improve capture of the macOS microphone / dictation key in Settings
- add visible diagnostics for shortcut capture and registration
- document the shortcut architecture more explicitly
- improve error handling around Accessibility permissions
- add basic testing guidance for contributors
- tighten naming around hardware key vs function key semantics

---

## Good First Contribution Areas

These are the best places for new contributors to help.

- improve documentation clarity
- create reproducible bug reports with logs and hardware details
- refine UI copy in Settings and onboarding
- add developer-facing debugging notes
- polish small UX inconsistencies in the Settings panel

---

## Research / Exploration

These are important, but not yet treated as immediate implementation commitments.

- broader cross-platform parity beyond macOS
- richer text transformation workflows
- deeper model and hardware optimization
- improved history workflows and post-processing controls

---

## Out of Scope for Now

To keep the project coherent, the following are not current priorities.

- turning Voxa into a generic chatbot
- default cloud processing as the main path
- feature bloat that weakens the core dictation workflow
- major platform expansion before macOS quality is solid

---

## How to Use This Roadmap

When proposing work, contributors should try to map the contribution to one of these categories:

- now
- near-term
- research
- out of scope

If a proposal does not clearly strengthen the current product direction, it may be declined even if the implementation is technically sound.
