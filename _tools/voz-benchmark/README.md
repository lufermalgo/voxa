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

Use 30 distinct human recordings, about 3, 10 and 30 seconds long, with a mix of
quiet and ordinary background noise. Include names, numbers and technical terms.

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
