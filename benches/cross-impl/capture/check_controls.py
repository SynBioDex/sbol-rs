"""Run paired validation controls against the corrected, pinned Docker images.

These controls demonstrate a concrete difference in rule scope; they are
not a coverage score. Run after the performance experiment, never alongside it.
"""

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
IMAGES = {"sbol-rs": "sbol-rs-bench", "libsbolj3": "libsbolj3-bench", "pysbol3": "pysbol3-bench"}


def main():
    results = {}
    for fixture in sorted((ROOT / "controls").glob("*.ttl")):
        results[fixture.stem] = {}
        for implementation, image in IMAGES.items():
            output = f"{fixture.stem}.{implementation}.json"
            subprocess.run(
                ["docker", "run", "--rm", "--network=none", "-v", f"{ROOT / 'controls'}:/work",
                 image, f"/work/{fixture.name}", "turtle", "turtle", "1", "2",
                 f"/work/{output}", "1", "sbol3"], check=True,
            )
            payload = json.loads((ROOT / "controls" / output).read_text())
            results[fixture.stem][implementation] = payload["validation"]
    (ROOT / "control-outcomes.json").write_text(json.dumps(results, indent=2) + "\n")
    def count(report):
        if "messages" in report:
            return len(report["messages"])
        errors = report["errors"]
        return len(errors) if isinstance(errors, list) else errors

    assert all(count(r) == 0 for r in results["valid-range"].values())
    assert all(count(r) > 0 for r in results["range-end-zero"].values())
    assert count(results["range-beyond-sequence"]["sbol-rs"]) > 0
    assert count(results["range-beyond-sequence"]["libsbolj3"]) > 0
    assert count(results["range-beyond-sequence"]["pysbol3"]) == 0
    for fixture, implementations in results.items():
        for implementation, report in implementations.items():
            print(f"{fixture}: {implementation}: {count(report)} diagnostics/errors")


if __name__ == "__main__":
    main()
