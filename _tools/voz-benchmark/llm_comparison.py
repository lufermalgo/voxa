"""Measure optional production refinement separately from recognition."""
import argparse
import hashlib
import json
import re
import shutil
from pathlib import Path
from benchmark import Worker


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("runs")
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--server", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    # Use one warm transcript per recording/engine, not a best-of selection.
    rows = [json.loads(line) for line in Path(args.runs).read_text().splitlines()]
    inputs = [r for r in rows if r["phase"] == "warm" and r["repetition"] == 0]
    references = json.loads(Path(args.manifest).read_text())
    inputs += [{"id": r["id"], "engine": "reference", "text": r["reference"], "stt_ms": None} for r in references]
    profile_source = (root / "../../src-tauri/src/db.rs").resolve()
    profile = re.search(r"\(1, 'Elegant', '(.*?)', 'star'", profile_source.read_text(), re.S)
    if profile is None:
        raise ValueError("default Elegant profile not found; inspect production source")
    profile_path = output / "profile.txt"
    profile_path.write_text(profile.group(1))
    global_log = Path("/tmp/llama-server.log")
    backup = output / "server-log-before.txt"
    if global_log.exists():
        shutil.copy2(global_log, backup)
    worker = None
    try:
        worker = Worker([str(root / "target/release/voxa-llm-bench"), str(Path(args.model).resolve()),
            str(Path(args.server).resolve()), str(profile_path), str(output / "server.pid")], output / "worker.log")
        metadata = {"ready": worker.ready, "model": args.model, "server": args.server,
            "language": "de", "profile": "default Elegant + production plain formatting, no custom hints",
            "cursor_context": "empty", "repetitions": 3, "warmup_excluded": True,
            "sources": {name: hashlib.sha256((root / ("../../src-tauri/src/" + name)).read_bytes()).hexdigest()
                for name in ("llama_inference.rs", "formatting.rs", "db.rs")},
            "limits": ["stage timing only, not actual stop-to-paste", "references are unverified read prompts",
                       "do not use literal WER for intentional rewriting", "STT first-repetition transcript reused"]}
        (output / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2))
        worker.request({"text": "Bitte schicke mir morgen die Unterlagen."})
        with open(output / "runs.jsonl", "w") as log:
            for repetition in range(3):
                # Reverse the input order on alternating repetitions.
                order = inputs if repetition % 2 == 0 else list(reversed(inputs))
                for row in order:
                    result = worker.request({"text": row["text"]})
                    log.write(json.dumps({"engine": row["engine"], "id": row["id"],
                        "repetition": repetition, "raw_text": row["text"],
                        "source_stt_ms": row["stt_ms"], **result}, ensure_ascii=False) + "\n")
                    log.flush()
                print(f"Completed LLM repetition {repetition+1}", flush=True)
    finally:
        if worker is not None:
            worker.close()
        if global_log.exists():
            shutil.copy2(global_log, output / "server.log")
        if backup.exists():
            shutil.copy2(backup, global_log)


if __name__ == "__main__":
    main()
