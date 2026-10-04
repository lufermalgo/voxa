"""Persistent, sequential engine comparison. All audio and transcripts stay local."""
import argparse
import hashlib
import json
import math
import queue
import re
import statistics
import subprocess
import threading
import time
import unicodedata
from pathlib import Path


def words(text):
    return re.findall(r"[^\W_]+", unicodedata.normalize("NFC", text).casefold())


def errors(reference, hypothesis):
    a, b = words(reference), words(hypothesis)
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        row = [i]
        for j, y in enumerate(b, 1):
            row.append(min(row[-1] + 1, prev[j] + 1, prev[j-1] + (x != y)))
        prev = row
    return prev[-1], len(a)


def p95(values):
    return sorted(values)[math.ceil(len(values) * .95) - 1]


class Worker:
    def __init__(self, command, log):
        self.log = open(log, "w")
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=self.log, text=True, bufsize=1)
        self.responses = queue.Queue()
        self.peak_rss_kib = 0
        self.rss_kib = 0
        self.stop = threading.Event()
        def read():
            for line in self.process.stdout:
                try:
                    value = json.loads(line)
                    if isinstance(value, dict):
                        self.responses.put(value)
                except json.JSONDecodeError:
                    self.log.write(line)
            self.responses.put({"error": "worker exited"})
        def sample():
            while not self.stop.is_set():
                result = subprocess.run(["ps", "-o", "rss=", "-p", str(self.process.pid)],
                    capture_output=True, text=True)
                if result.stdout.strip().isdigit():
                    self.rss_kib = int(result.stdout.strip())
                    self.peak_rss_kib = max(self.peak_rss_kib, self.rss_kib)
                self.stop.wait(.1)
        threading.Thread(target=read, daemon=True).start()
        self.sampler = threading.Thread(target=sample, daemon=True)
        self.sampler.start()
        try:
            self.ready = self.receive()
            if not self.ready.get("ready"):
                raise RuntimeError("worker did not signal readiness")
        except Exception:
            self.close()
            raise

    def receive(self):
        try:
            result = self.responses.get(timeout=300)
        except queue.Empty as error:
            raise RuntimeError("worker timed out after 300 seconds; inspect engine log") from error
        if "error" in result:
            raise RuntimeError(result["error"])
        return result

    def transcribe(self, pcm):
        return self.request({"pcm": str(pcm)})

    def request(self, payload):
        self.process.stdin.write(json.dumps(payload) + "\n")
        self.process.stdin.flush()
        return self.receive()

    def close(self):
        self.stop.set()
        self.sampler.join(timeout=2)
        self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=10)
        self.log.close()


def prepare(manifest, output):
    source = Path(manifest).resolve()
    records = json.loads(source.read_text())
    if not records or not isinstance(records, list):
        raise ValueError("manifest must be a nonempty list")
    seen = set()
    for record in records:
        identifier = record["id"]
        if not re.fullmatch(r"[a-zA-Z0-9_-]+", identifier) or identifier in seen:
            raise ValueError("invalid or duplicate recording id")
        seen.add(identifier)
        if not words(record["reference"]):
            raise ValueError("nonempty reference required")
        if record.get("source_kind") not in ("human", "synthetic"):
            raise ValueError("source_kind must be human or synthetic")
        audio = (source.parent / record["audio"]).resolve()
        pcm = output / (identifier + ".f32")
        subprocess.run(["ffmpeg", "-nostdin", "-v", "error", "-y", "-i", str(audio),
                        "-ac", "1", "-ar", "16000", "-f", "f32le", str(pcm)], check=True)
        import array
        samples = array.array("f", pcm.read_bytes())
        if not samples or any(not math.isfinite(x) for x in samples):
            raise ValueError("empty or nonfinite audio")
        record["pcm"] = str(pcm)
        record["audio_s"] = len(samples) / 16000
        record["pcm_sha256"] = hashlib.sha256(pcm.read_bytes()).hexdigest()
    return records


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest")
    parser.add_argument("--whisper-model", required=True)
    parser.add_argument("--voz-model", required=True)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--idle-seconds", type=float, default=0)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    if args.repetitions < 2 or args.idle_seconds < 0:
        parser.error("need at least two repetitions and nonnegative idle duration")
    root = Path(__file__).resolve().parent
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    records = prepare(args.manifest, output)
    commands = {
        "whisper": [str(root / "target/release/voxa-whisper-bench"), str(Path(args.whisper_model).resolve())],
        "voz": [str(root / ".build/release/VozBench"), str(Path(args.voz_model).resolve())],
    }
    metadata = {"machine": subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip(),
        "ram_bytes": int(subprocess.check_output(["sysctl", "-n", "hw.memsize"], text=True)),
        "macos": subprocess.check_output(["sw_vers", "-productVersion"], text=True).strip(),
        "voz_sdk": "3.5.0", "whisper_rs": "0.13.2", "whisper_model": args.whisper_model,
        "whisper_source_sha256": hashlib.sha256((root / "../../src-tauri/src/whisper_inference.rs").read_bytes()).hexdigest(),
        "idle_seconds": args.idle_seconds, "corpus": records,
        "limits": ["STT-only; no LLM, capture, resampling timer or paste", "RSS samples every 100ms; excludes some accelerator allocations",
                   "casefold + NFC + punctuation removal; no numeric equivalence", "fresh-process load is not cold filesystem cache"]}
    (output / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2))
    rows, loads = [], []
    with open(output / "runs.jsonl", "w") as log:
        for repetition in range(args.repetitions):
            order = ["whisper", "voz"] if repetition % 2 == 0 else ["voz", "whisper"]
            for name in order:
                worker = Worker(commands[name], output / f"{name}-{repetition}.log")
                try:
                    loads.append({"engine": name, "repetition": repetition, **worker.ready})
                    phases = [("first_inference", records[:1]), ("warm", records)]
                    if args.idle_seconds and repetition == 0:
                        phases.append(("idle_resume", records[:1]))
                    for phase, corpus in phases:
                        if phase == "idle_resume":
                            time.sleep(args.idle_seconds)
                        for record in corpus:
                            result = worker.transcribe(record["pcm"])
                            distance, count = errors(record["reference"], result["text"])
                            row = {"engine": name, "repetition": repetition, "phase": phase,
                                "id": record["id"], "source_kind": record["source_kind"], "audio_s": record["audio_s"],
                                "reference": record["reference"], "edit_distance": distance, "reference_words": count,
                                "sampled_rss_kib": worker.rss_kib, **result}
                            rows.append(row)
                            log.write(json.dumps(row, ensure_ascii=False) + "\n")
                            log.flush()
                    loads[-1]["sampled_peak_rss_kib"] = worker.peak_rss_kib
                finally:
                    worker.close()
                print(f"Completed {name}, repetition {repetition+1}", flush=True)
    summaries = []
    for name in commands:
        for kind in sorted({r["source_kind"] for r in rows}):
            subset = [r for r in rows if r["engine"] == name and r["phase"] == "warm" and r["source_kind"] == kind]
            times = [r["stt_ms"] for r in subset]
            summaries.append({"engine": name, "source_kind": kind, "n": len(times),
                "median_stt_ms": statistics.median(times), "p95_stt_ms": p95(times),
                "corpus_wer": sum(r["edit_distance"] for r in subset) / sum(r["reference_words"] for r in subset)})
    (output / "summary.json").write_text(json.dumps({"warm": summaries, "loads": loads}, indent=2))
    print(json.dumps(summaries, indent=2))


if __name__ == "__main__":
    main()
