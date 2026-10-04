# Voz versus Voxa Whisper: benchmark specification

Scope: an isolated, reproducible benchmark, without changing the dictation app.
The user authorized comparison of speed, quality, startup and memory on the same
German recordings. Product integration is a subsequent decision.

Acceptance criteria, defined before implementation:

- Use the existing `WhisperEngine` source, model Small, Metal, four threads and
  German language; preserve Voxa's filtering and state reset behavior.
- Pin the Voz Swift SDK and reuse one loaded instance per measured session.
- Feed identical mono 16 kHz audio to both engines; verify Voz sample rate.
- Separate download, model load, first inference, warm inference and idle resume.
- Preserve raw transcripts, per-recording timings, RSS observations and errors.
- Compute corpus word error rate using supplied reference text and documented
  normalization, as well as median and nearest-rank p95 warm latency.
- Label synthetic audio as synthetic; never infer natural dictation quality from it.
- Leave LLM quality, hotkey-to-paste timing and actual idle/peak app memory
  explicitly unmeasured until tested with the integrated application.
- Verify scoring on substitutions, insertions, deletions and German normalization.
- Fail clearly on missing inputs, invalid audio, worker exits and engine errors.

Design: separate persistent Rust and Swift JSONL workers, driven sequentially
by a Python runner. Rust imports the production Whisper source without edits.
Swift uses the SDK's samples API. Audio conversion and loading are outside the
inference timer. Each fresh worker measures model loading; downloads are prepared
separately. Worker RSS is sampled by the parent and is not total GPU/ANE allocation.
Alternate engine order across repetitions to reduce systematic order bias.
Inputs and outputs stay local. No live microphone recording or paste injection.

Tasks:
- [x] Inspect production engine and SDK API; define acceptance criteria.
- [x] Build workers and runner; verify scoring and audio validation.
- [x] Prepare models; run a labeled preliminary corpus if real recordings absent.
- [ ] Compare 30 real German recordings with verified references.
- [ ] Measure integrated LLM output, stop-to-paste timing and idle resume.
- [x] Report measured results and limitations; update session state.
