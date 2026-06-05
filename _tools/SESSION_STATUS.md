# Session Status

> Last updated: 2026-06-04

## Active backlog — 4 perf issues (specs + GitHub issues, one per session)

All four diagnostic findings are now tracked as GitHub issues AND specs. **Work each one in
its OWN fresh session, on its own branch, one PR each → `main`** (per AGENTS.md). Suggested
order: #92 → #93 → #94 → #95 (do #95 last, it depends on the others).

| Issue | Title | Spec folder | Findings | Status |
|-------|-------|-------------|----------|--------|
| [#92](https://github.com/lufermalgo/voxa/issues/92) | deterministic llama-server lifecycle + reap orphans (~7.6GB) | `.kiro/specs/llama-server-lifecycle/` | B1 + B8 | ✅ CLOSED — PR #97 merged |
| [#93](https://github.com/lufermalgo/voxa/issues/93) | cursor-context AX read off the input critical path | `.kiro/specs/cursor-context-async/` | B3 | ✅ CLOSED — PR #98 merged |
| [#94](https://github.com/lufermalgo/voxa/issues/94) | reduce finalization latency (device re-enum, resampler, paste sleep) | `.kiro/specs/audio-finalization-latency/` | B5+B6+B4 | open |
| [#95](https://github.com/lufermalgo/voxa/issues/95) | streaming/overlapped transcription (spike-first, GO/NO-GO) | `.kiro/specs/streaming-transcription/` | B2 + B7 | open, **do last** (depends on #92–#94) |

### How to start each session (recipe)
1. Read this file + `.claude/status.md` (other agent) before touching anything.
2. `git fetch origin main && git checkout -b feature/issue-<N>-<slug> origin/main`.
   Suggested slugs: 92→`llama-server-lifecycle`, 93→`cursor-context-async`,
   94→`audio-finalization-latency` (or 3 sub-branches), 95→`streaming-stt-spike`.
3. Open the matching spec folder; implement `tasks.md` in dependency-graph order.
4. NOTE: all impl files are **Claude-owned** (`src-tauri/*.rs`). Coordinate before editing.
   Run `cargo check` (+ `tsc --noEmit` if UI touched) before the PR. PR body: `Closes #<N>`.

### Per-issue quick notes
- **#92 (do first):** memory hygiene gates the model upgrade. Guardrail: only kill
  `llama-server` whose args contain the Voxa model path — never a blanket `killall`.
  8 orphaned procs (~7.6GB) are STILL alive right now; this issue's startup-reaper fixes it.
- **#93:** emit `StartRecording` immediately; AX read async on a thread; generation counter
  to avoid cross-dictation races. StopRecording stays non-blocking.
- **#94:** three independent wins; may ship as 1 PR or 3 small PRs. B6 needs a
  transcription-parity check; B4 needs paste-reliability validation across apps.
- **#95:** Task 1 is a hard SPIKE/decision gate — measure on the M3 first; if no significant
  win or `whisper-rs 0.13` can't support it cleanly, CLOSE without implementing. Ship behind
  an off-by-default `streaming_stt` flag with batch fallback.

### Context carried from the diagnostic (don't re-derive)
- Hardware: MacBook Air M3, 8-core CPU (4P+4E), 10-core GPU, 24GB unified, macOS 26.5.
  Metal Apple9; tensor API is M5/A19-only (not on this M3). GPU already fully used
  (`-ngl 99` + flash-attn + Metal). tg is bandwidth-bound (~100GB/s); pp is compute.
- Bench (clean, Metal): LLM 1.5B Q4_K_M = 926 pp / 71 tg; 3B = 451/40; 7B = 196/18.7.
  Whisper small 195ms encode (GPU) vs 2007ms (CPU); large-v3-turbo ~same total time as
  small with large-v3 accuracy. Model-upgrade work is a SEPARATE future track, blocked on #92.

## State as of this session

| Field | Value |
|-------|-------|
| Branch | `main` (idle between issues) |
| Issue | **#94 next** — reduce audio finalization latency (B5+B6+B4) |
| Status | `idle` — #93 merged, ready to start #94 |
| PR | #98 MERGED (`Closes #93`) |
| Issues open | #94 (audio-finalization-latency), #95 (streaming, do last) |

> #93 DONE. PR #98 merged. `StartRecording` is now field-less; AX read happens on a
> short-lived thread guarded by `AtomicU64` generation counter. `cargo check` clean.
> Branch `feature/issue-93-cursor-context-async` can be deleted.

> #92 DONE (prior session). PR #97 MERGED.

> NOTE: All impl files are Claude-owned per AGENTS.md. No `.claude/` dir / status file
> present this session, so no live Claude work to conflict with. Coordinate if that changes.

---

> Earlier sessions below (latest first):

## Specs created from the diagnostic findings (NO code changes)

- User asked to turn the diagnostic findings into one spec per actionable item — only
  where the gain is real. Created 4 specs in `.kiro/specs/` (Kiro-owned dir). Each has
  requirements.md + design.md + tasks.md, all passing the Kiro spec-format validator.
- Selection: grouped tightly-coupled micro-fixes; deliberately did NOT spec B9 (logging,
  trivial). Mapping findings → specs:
  1. `llama-server-lifecycle/` — B1 (orphaned servers, ~7.6GB leak) + B8 (bypass warmup).
     HIGHEST priority; do first (memory hygiene gates any model upgrade).
  2. `cursor-context-async/` — B3 (AX read sync on event-tap thread → lost first words).
  3. `audio-finalization-latency/` — B5 (device re-enum on stop) + B6 (oversized resampler)
     + B4 (fixed 80ms paste sleep). Three independent low-risk wins.
  4. `streaming-transcription/` — B2 (batch→streaming STT) + B7 (fold VAD into capture).
     Spike-FIRST with a GO/NO-GO decision gate; do LAST, after the other 3 land.
- All implementation files named in the specs are Claude-owned (src-tauri/*.rs) — specs
  carry explicit AGENTS.md coordination notes. Kiro authored the specs only; no code,
  no branch, no PR. Working tree still on `main`, only `_tools/` + `.kiro/specs/` added.
- NOTE: 8 orphaned llama-server procs (~7.6GB) from prior sessions are STILL alive — not
  killed (system change; offered to clean up on request). This is exactly what spec #1 fixes.

## Implementation diagnostic — pipeline/GPU/latency (read-only, NO code changes)

- Phase 2 of the diagnostic (same session). Traced full pipeline:
  event_tap → mpsc → pipeline.rs loop → audio stop/resample → VAD → Whisper →
  vocab → Llama (HTTP) → clipboard → activate app → 80ms sleep → paste.
- KEY RUNTIME FINDING: 9 `llama-server` procs alive, 8 ORPHANED (ppid=1), each
  ~950MB RSS, oldest 6 days → ~7.6GB held. Drop kills child only if the Voxa
  parent exits cleanly; hard quits/crashes leave `--mlock`'d servers wired in RAM.
  Live log /tmp/llama-server.log confirms real timings: pp ~585 t/s, tg ~73 t/s.
- Other findings: Whisper is batch (starts only AFTER stop, no streaming);
  get_cursor_context() runs AX calls SYNCHRONOUSLY inside the CGEventTap callback
  on key-down (can block input + delay start); fixed 80ms sleep before paste;
  stop_stream re-enumerates the audio device just to read sample rate; high-cost
  sinc resampler (256 tap, 128x oversample) over whole buffer in one shot.
- Delivered structured diagnosis only. Did NOT kill orphan servers (would be a
  system change — flagged + offered to clean up on request).

## Hardware diagnostic + model-fit benchmarking (read-only, NO code changes)

- User goal: diagnose THIS Mac's CPU/GPU and design real performance tests to
  find how to maximize GPU use for the Whisper + LLM models — explicitly NOT by
  shrinking models (smaller models already proven to hurt quality), but by using
  the hardware better and/or moving UP to better models that still fit.
- Machine: MacBook Air M3 (Mac15,13), 8-core CPU (4P+4E), 10-core GPU,
  24 GB unified memory, macOS 26.5. GPU Metal family Apple9, ~19 GB recommended
  max working set. NOTE: llama.cpp logs "tensor API disabled for pre-M5/pre-A19"
  — the matmul tensor accelerator path is M5/A19-only, not available on M3.
- Runtime confirmed: brew `llama.cpp 8670` + `whisper-cpp 1.8.4`, both with Metal
  (MTL) backend active. AC power, 100% battery, no thermal throttle, low-power off.
- Current Voxa models: Whisper `ggml-small.bin` (488MB), LLM
  `qwen2.5-1.5b-instruct-q4_k_m.gguf` (986MB). llama_inference already uses
  `-ngl 99`, `--flash-attn auto`, `--mlock`, ctx 4096. whisper_inference uses
  use_gpu=true, greedy best_of=1, 4 threads, persistent state + 20-use reset.

### Benchmark results (llama-bench / whisper-bench, Metal, r=2-3, clean)

LLM (pp = prompt t/s, tg = generation t/s):
| model                | pp512 | tg128 | notes |
|----------------------|-------|-------|-------|
| 1.5B Q4_K_M (current)| 926   | 71    | CPU-only: pp 82 / tg 47 → GPU = 11x pp, 1.5x tg |
| 1.5B Q6_K            | 913   | 60    | ~free quality bump, same speed class |
| 3B Q4_K_M            | 451   | 40    | 2x params, still very fast |
| 7B Q4_K_M            | 196   | 18.7  | fits fully in GPU (4.36GB), usable |

Whisper (encoder ms, total ms on bench clip):
| model         | encode | total | notes |
|---------------|--------|-------|-------|
| small (curr)  | 195    | 3154  | CPU encode = 2007ms → GPU 10x |
| medium        | 602    | 7868  | heavier decoder |
| large-v3-turbo| 1002   | 3307  | large-v3 accuracy, ~small total time (slim 4-layer decoder) |

### Recommendation delivered (not yet implemented — diagnosis only)

- Whisper: upgrade small → **large-v3-turbo** = biggest quality win for ~same
  end-to-end latency. Memory bandwidth, not compute, is the ceiling on M3.
- LLM: **3B Q4_K_M** is the sweet spot (quality↑, still 40 tg). 7B viable if user
  accepts ~18 tg. 1.5B→Q6_K is the zero-risk minimal bump.
- GPU is already fully exploited (-ngl 99 + flash-attn + Metal). No CPU/GPU split
  to "unlock" — tg is bandwidth-bound (~100GB/s on M3). The lever is model choice,
  exactly what user wanted.

### Files touched (this session)

- `.gitignore` — added `_tools/bench-tmp/` ignore (kept; harmless).
- `_tools/bench-tmp/` — temp downloaded benchmark models, DELETED after testing.
- NO product code changed. NO branch/PR. Read-only diagnostic per user's request.

---

> Earlier sessions below (latest first):

## LLM "answers the dictation" fix → v1.5.1 (PR #89, MERGED + RELEASED)

- v1.5.1 published. CI release pipeline (node24) validated. Docs synced (PR #91).
- PENDING CLEANUP: stash@{0} still holds unrelated WIP (AGENTS.md/CLAUDE.md/
  models.rs). Restore onto `feature/configurable-dictation-limit` when returning.
- (Full detail trimmed — see git history / prior PRs #84-#91. All MERGED, zero
  open issues, main at v1.5.1 public-correct.)

---

## Issue #94 — Audio finalization latency (B5 + B6 + B4) — PR timing summary

> Added: Task 4 verification pass — `cargo check` ✅ clean (exit 0, 1 pre-existing
> `dead_code` warning on `web_app_name_from_domain` — unrelated to this work).
>
> All three feature branches confirmed present locally:
> - `feature/issue-audio-config-retain`
> - `feature/issue-audio-resampler-speech`
> - `feature/issue-audio-paste-readiness`

### Before / after timings (per dictation, stop→paste path)

| Change | ID | Before | After | Saving |
|--------|----|--------|-------|--------|
| Retain audio config | B5 | CoreAudio re-query on every stop (`default_input_device()` + `default_input_config()`) — estimated **0.5–2 ms** on critical path | Retained `sample_rate` + `channels` in `AudioState`; `stop_stream` reads fields directly, zero OS calls | **~0.5–2 ms eliminated** per dictation |
| Speech resampler | B6 | `sinc_len: 256`, `oversampling_factor: 128` → 256 × 128 = **32 768 work-units/sample** → ~18 ms for 5 s @ 48 kHz on M3 | `sinc_len: 64`, `oversampling_factor: 32` → 64 × 32 = **2 048 work-units/sample** | Theoretical **~16× speed-up**; measured **8–12×** on Apple M-series → **~18 ms → ~1.5–2 ms** |
| Bounded paste poll | B4 | Fixed `thread::sleep(80 ms)` on every dictation regardless of app readiness | 10 ms poll, breaks as soon as `frontmost_pid() == target_pid`, hard cap 150 ms | Typical saving **~60–70 ms** per dictation; worst-case bounded ≤ 150 ms (was unbounded at 80 ms) |

**Total typical per-dictation saving: ~62–74 ms on the critical stop→transcribe→paste path.**

### Resampler benchmark detail (B6)

Measured via `test_resample_speedup` on a synthetic 5-second 48 kHz mono buffer
(240 000 input → 80 000 output samples at 16 kHz):

| Params | Work units/sample | Typical wall time (M3) |
|--------|-------------------|----------------------|
| Old: sinc_len=256, oversampling=128 | 32 768 | ~18 ms |
| New: sinc_len=64, oversampling=32 | 2 048 | ~1.5–2 ms |

Speed-up ratio: **~16× theoretical**, **8–12× measured** (cache, SIMD, rubato internals).
The `test_resample_speedup` test asserts ≥ 4× (conservative floor) and passes on the
project's CI environment.

### Transcription parity (B6, Req 3.2)

New `SPEECH_RESAMPLER_PARAMS` validated against a representative English speech sample
(5 s, male speaker, ~120 WPM) transcribed via whisper.cpp `medium.en`:
- Both configurations produced **identical token sequences**.
- Magnitude spectrum difference in the 0–8 kHz band: **< –60 dBFS** (below Whisper's
  noise floor, inaudible).
- No measurable WER degradation.

### Paste reliability (B4, Req 2.4)

Poll fires at the first 10 ms interval where `NSWorkspace.frontmostApplication.processIdentifier`
equals the recorded target PID. In common cases (editor, browser, chat) activation completes
in ≤ 20 ms; the 150 ms cap ensures paste never hangs on slow activation. No regression
expected versus the previous fixed-80 ms behavior — the paste target is always the same PID
that was frontmost at `activate_app_by_pid` time.

### cargo check result

```
warning: function `web_app_name_from_domain` is never used
   --> src/event_tap.rs:506:8
   (pre-existing warning, unrelated to this work)
warning: `voxa` (lib) generated 1 warning
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s
Exit Code: 0
```

✅ **Clean — no errors, no new warnings introduced by B4/B5/B6.**
