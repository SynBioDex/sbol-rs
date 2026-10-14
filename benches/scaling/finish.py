"""Complete separate Java I/O timings and comparable cold-process RSS runs."""

import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def run(*arguments):
    subprocess.run([sys.executable, str(ROOT / "run.py"), "--backend", "native", *arguments], check=True)


def main():
    # Wait for the sequential timing matrix, rather than compete for CPU/RAM.
    final = ROOT / "results/javascript_100000.record.json"
    while not final.exists():
        time.sleep(2)
    assert json.loads(final.read_text())["exit_code"] == 0
    run("--objects", "100000", "--implementations", "java", "--validate", "0",
        "--warmup", "3", "--iters", "5", "--directory", "io-results", "--timeout", "300")
    for trial in range(1, 4):
        run("--validate", "0", "--warmup", "0", "--iters", "1",
            "--directory", f"memory/trial-{trial}", "--timeout", "300")


if __name__ == "__main__":
    main()
