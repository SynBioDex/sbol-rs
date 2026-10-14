"""Run the existing, pinned benchmark drivers with peak-RSS measurement."""

import argparse
import gzip
import hashlib
import json
import os
import signal
import statistics
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PINNED = json.loads((ROOT.parent / "cross-impl/results.validation-revision.provenance.json").read_text())
IMAGES = {entry["tag"].split(":")[0]: entry["id"] for entry in PINNED["images"]}
DRIVERS = {
    "rust": ("sbol-rs-bench", ["sbol-rs-bench-runner"], ["1", "sbol3"]),
    "python": ("pysbol3-bench", ["python3", "/opt/pysbol3-bench/bench.py"], ["1"]),
    "java": ("libsbolj3-bench", ["java", "-cp", "/opt/libsbolj3-bench/libSBOLj3.jar:/opt/libsbolj3-bench", "Bench"], ["1"]),
    "javascript": ("sboljs3-bench", ["node", "/opt/sboljs3/bench.js"], []),
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--objects", type=int, nargs="+", default=[1000, 10000, 100000])
    parser.add_argument("--implementations", nargs="+", choices=DRIVERS, default=list(DRIVERS))
    parser.add_argument("--backend", choices=["native", "docker"], default="native")
    parser.add_argument("--warmup", type=int, default=3)
    parser.add_argument("--iters", type=int, default=5)
    parser.add_argument("--directory", default="results")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--timeout", type=int, default=7200)
    parser.add_argument("--validate", choices=["0", "1"], default="1")
    args = parser.parse_args()
    inputs = Path("/private/tmp/sbol-scaling-inputs")
    tools = Path("/private/tmp/sbol-scaling-tools")
    output = ROOT / args.directory
    output.mkdir(parents=True, exist_ok=True)
    inputs.mkdir(exist_ok=True)
    manifest = {entry["objects"]: entry for entry in json.loads((ROOT / "input-manifest.json").read_text())}
    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(tools / "python-packages")
    environment["PYTHONNOUSERSITE"] = "1"
    native_drivers = {
        "rust": ["/private/tmp/sbol-review-validation/target/release/runner"],
        "python": ["/usr/local/bin/python3.12", str(tools / "bench.py")],
        "java": [str(tools / "jdk-17.0.20.1+1/Contents/Home/bin/java"), "-cp", str(tools / "libSBOLj3.jar") + ":" + str(tools), "Bench"],
        "javascript": [str(tools / "node-v20.20.2-darwin-arm64/bin/node"), str(tools / "javascript/bench.js")],
    }
    for count in args.objects:
        entry = manifest[count]
        destination = inputs / Path(entry["file"]).stem
        if not destination.exists():
            with gzip.open(ROOT / "inputs" / entry["file"], "rb") as source, destination.open("wb") as target:
                while block := source.read(1 << 20):
                    target.write(block)
        assert destination.stat().st_size == entry["bytes"]
        assert hashlib.sha256(destination.read_bytes()).hexdigest() == entry["sha256"]
        for implementation in args.implementations:
            stem = f"{implementation}_{count}"
            record_path = output / (stem + ".record.json")
            if args.resume and record_path.exists() and (output / (stem + ".json")).exists():
                print(f"RESUME {stem}: existing result", flush=True)
                continue
            image, command, trailing = DRIVERS[implementation]
            trailing = list(trailing)
            if trailing:
                trailing[0] = args.validate
            container_name = "sbol-scaling-" + args.directory.replace("/", "-") + "-" + stem.replace("_", "-")
            if args.backend == "docker":
                invocation = [
                    "docker", "run", "--rm", "--name", container_name, "--network", "none",
                    "-v", f"{inputs}:/inputs:ro", "-v", f"{tools}:/tools:ro", "-v", f"{output}:/output",
                    "--entrypoint", "/tools/measure_rss", IMAGES[image],
                    f"/output/{stem}.rss.json", *command,
                    f"/inputs/{destination.name}", "rdfxml", "rdfxml", str(args.warmup), str(args.iters),
                    f"/output/{stem}.json", *trailing,
                ]
            else:
                invocation = [str(tools / "measure_rss.native"), str(output / (stem + ".rss.json")),
                              *native_drivers[implementation], str(destination), "rdfxml", "rdfxml",
                              str(args.warmup), str(args.iters), str(output / (stem + ".json")), *trailing]
            record = {"implementation": implementation, "objects": count, "input": entry, "backend": args.backend,
                      "command": invocation, "warmup": args.warmup, "iterations": args.iters,
                      "validation_requested": args.validate == "1" and implementation != "javascript",
                      "started_utc": datetime.now(timezone.utc).isoformat()}
            print(f"START {stem}, warmup={args.warmup}, measured={args.iters}", flush=True)
            started = time.monotonic()
            try:
                with (output / (stem + ".log")).open("w") as log:
                    process = subprocess.Popen(invocation, stdout=log, stderr=subprocess.STDOUT,
                                               env=environment, start_new_session=True)
                    try:
                        record["exit_code"] = process.wait(timeout=args.timeout)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGTERM)
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            os.killpg(process.pid, signal.SIGKILL)
                            process.wait()
                        raise
            except subprocess.TimeoutExpired:
                if args.backend == "docker":
                    subprocess.run(["docker", "stop", "-t", "2", container_name], capture_output=True)
                record["exit_code"] = "timeout"
            record["elapsed_seconds"] = time.monotonic() - started
            record_path.write_text(json.dumps(record, indent=2) + "\n")
            result_path = output / (stem + ".json")
            if record["exit_code"] == 0 and result_path.exists():
                result = json.loads(result_path.read_text())
                timings = {phase: round(statistics.median(result[phase + "_ns"]) / 1e6, 3)
                           for phase in ("parse", "validate", "serialize") if phase + "_ns" in result}
                rss = json.loads((output / (stem + ".rss.json")).read_text())
                validation = result.get("validation", {})
                diagnostics = {key: validation[key] for key in ("errors", "warnings", "messages") if key in validation}
                print(f"DONE {stem}: median ms={timings}, peak MiB={rss['peak_rss_kib'] / 1024:.1f}, diagnostics={diagnostics}", flush=True)
            else:
                print(f"FAILED {stem}: {record['exit_code']}; see {stem}.log", flush=True)


if __name__ == "__main__":
    main()
