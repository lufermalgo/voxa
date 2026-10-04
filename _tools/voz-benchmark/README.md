# Local Voz / Whisper comparison

This harness imports Voxa's actual WhisperEngine and compares it with Voz Swift
SDK 3.5.0 (model revision v0.1.0). It does not alter the application.

Prerequisites: Apple Silicon, macOS 15+, Swift 6.2+, Rust, CMake, Python 3,
ffmpeg, and the Whisper Small and Voz models. Pinning the SDK also pins its model;
newer website benchmarks may describe newer weights or a newer runtime.

Build from this directory:

```sh
swift build -c release
MACOSX_DEPLOYMENT_TARGET=15.0 cargo build --release
cargo check
python3 -m unittest discover -p 'test_*.py'
.build/release/VozBench /absolute/path/voz-model --download
```

Input is a JSON array with one entry per recording. Paths are relative to the
manifest. References must be manually checked against the actual recording.

```json
[
  {
    "id": "short-01",
    "audio": "short-01.wav",
    "reference": "Bitte schicke mir morgen die Unterlagen.",
    "source_kind": "human",
    "condition": "quiet"
  }
]
```

Start with the six guided human recordings below. If the initial result is
promising, expand to 30 distinct recordings with varied lengths and environments.

## Guided recording on this Mac

Run `python3 recording/setup.py` once to build `data/Sprachtest/Sprachtest.app`.
Open that app, click Start, read the displayed text, Stop, and Next. It requests
microphone access only on the first Start click. Six prompts take about two or
three minutes; no manual naming or saving is required. Repeating a take keeps the
old audio file and updates the manifest to point to the new take. Closing or
quitting stops and saves an active recording. The Mac's default input is used.

Generated files: `data/Sprachtest/Aufnahmen/`, `manifest.json`, and German
instructions. These are gitignored. Reference text initially has status
`read_prompt_unverified`: audit deviations before interpreting WER. Desired
semantic output is stored separately and must not replace the spoken reference.
Numeric formatting and self-corrections need separate semantic evaluation.

Use `python3 recording/setup.py --prepare-only` to write instructions without
rebuilding or signing a running app. Never rebuild or re-sign during recording.
If necessary, QuickTime Player's New Audio Recording is the fallback; save each
take with the prompt ID in the same Aufnahme folder and let the assistant import it.

The initial human comparison is **without LLM**. Voxa already supports this via
`bypass_llm` (Settings: Disconnect transformation model). The benchmark's workers
also run without LLM. A subsequent same-audio comparison should separately measure
LLM time, desired corrections, and meaning preservation. The self-correction
prompt explicitly tests Friday rather than Thursday and excluding Peter.

```sh
python3 benchmark.py /absolute/path/manifest.json \
  --whisper-model /absolute/path/ggml-small.bin \
  --voz-model /absolute/path/voz-model \
  --repetitions 3 --idle-seconds 60 \
  --output /absolute/path/new-results-directory
```

Outputs: metadata and PCM hashes, individual transcripts/timings, engine logs,
warm median and nearest-rank p95, corpus word error rate, fresh-process load times,
and sampled peak worker RSS. Audio conversion is outside the timer. Both engines
receive the same Float32 mono 16 kHz samples. Whisper uses German, no vocabulary
prompt, four threads, greedy best-of-one, and production filtering/reset cadence.

Each engine runs alone. Order alternates between repetitions. The first request
is reported separately; then the entire corpus runs with the model loaded. If
enabled, resume is measured in the first repetition: the process stays alive for
the specified idle duration and then transcribes the first sample. Load timing excludes downloads, but the first
Voz load may include one-time Neural Engine specialization. File caches are not
flushed, so a fresh process is not a cold operating-system cache.

WER normalization: NFC Unicode, case folding, punctuation removal. Digits and
spoken numbers are not equated; inspect numeric mismatches before interpreting
quality. WER is word edits divided by reference words across the corpus, not an
average of per-file percentages. Repeated runs are not independent recordings.
The preliminary synthetic corpus has only 15 speech files with paired artificial
noise versions, and three underlying texts with small suffix variations.

RSS is sampled every 100 ms; short peaks can be missed and accelerator allocations
are not fully represented. It is worker memory, not the full Voxa application's
memory. Final LLM output, recording finalization, and stop-to-paste latency remain
unmeasured here. They require a subsequent integrated test with the same LLM,
settings and controlled target app. Do not add unrelated stage medians to claim
an observed end-to-end time.

Voz SDK license is source-available; see the upstream license before distribution.
Model downloads require network access. Audio and transcripts are handled locally
by this harness. `data/`, `models/`, `results/` and build artifacts are gitignored;
do not publish personal recordings in a PR.


## Optional production LLM stage

After the STT measurement completes, use `llm_comparison.py` with the STT JSONL,
manifest, Qwen2.5-1.5B Q4_K_M model and a compatible llama-server (tested b8670).
The current latest b11387 rejects the production `--mlock` option. This stage
imports the production LlamaEngine, process helpers and formatting builder with
minimal data-type adapters, without linking the application UI. Default Elegant
instructions are read directly from db.rs, with German/plain formatting and no
custom hints or cursor context. It measures three repetitions for the first warm
transcript from each engine plus supplied reference-text controls. Downloads,
server startup and an initial warm-up are outside request timing. A private PID
file tracks the owned server; normal shutdown terminates and reaps it. A preexisting
global production log is backed up and restored. Run this stage alone, not during
STT measurement; compare meaning and editing effort rather than literal WER.

```sh
python3 llm_comparison.py results/human/runs.jsonl \
  --manifest data/Sprachtest/manifest.json \
  --model /absolute/path/qwen2.5-1.5b-instruct-q4_k_m.gguf \
  --server /absolute/path/llama-server \
  --output /absolute/path/new-llm-results-directory
```

Human results and transcripts stay in gitignored results directories. Benchmarks
are stage measurements; integrated stop-to-paste remains a separate validation.
