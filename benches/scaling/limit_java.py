"""Bound the existing large Java validation process without touching other jobs.

The corrected Java driver performs validation in its preflight before emitting
samples. A process-budget stop is not a measured validation latency.
"""

import json
import os
import signal
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TARGET = str(ROOT / "results/java_100000.rss.json")


def main():
    while True:
        lines = subprocess.check_output(["ps", "-axo", "pid,ppid,command"], text=True).splitlines()
        processes = [line.strip().split(None, 2) for line in lines[1:]]
        wrappers = [row for row in processes if len(row) == 3
                    and row[2].startswith("/private/tmp/sbol-scaling-tools/measure_rss.native " + TARGET + " ")]
        if wrappers:
            wrapper = int(wrappers[0][0])
            break
        time.sleep(2)
    started = time.monotonic()
    print(f"Observed Java 100000-object process {wrapper}; allowing 300 seconds.", flush=True)
    while time.monotonic() - started < 300:
        try:
            os.kill(wrapper, 0)
        except ProcessLookupError:
            print("Completed within budget.", flush=True)
            return
        time.sleep(1)
    lines = subprocess.check_output(["ps", "-axo", "pid,ppid,command"], text=True).splitlines()
    children = [line.strip().split(None, 2) for line in lines[1:]]
    children = [row for row in children if len(row) == 3 and int(row[1]) == wrapper
                and row[2].startswith("/private/tmp/sbol-scaling-tools/jdk-17.0.20.1+1/Contents/Home/bin/java ")
                and "Bench /private/tmp/sbol-scaling-inputs/sbol3_100000.rdf " in row[2]]
    assert len(children) == 1, children
    child = int(children[0][0])
    record = {"budget_seconds": 300, "observed_seconds": time.monotonic() - started,
              "kind": "whole-process wall-clock budget, including untimed preflight and startup",
              "signal": "SIGTERM", "wrapper_pid": wrapper, "child_pid": child,
              "command": children[0][2], "stopped_utc": datetime.now(timezone.utc).isoformat()}
    os.kill(child, signal.SIGTERM)
    (ROOT / "results/java_100000.budget.json").write_text(json.dumps(record, indent=2) + "\n")
    print("Stopped only the matched Java benchmark child at its process budget.", flush=True)


if __name__ == "__main__":
    main()
