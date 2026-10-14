"""Independently check the corrected capture: samples, validation outcomes, inputs, and source index."""

import csv
import hashlib
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parent
RESULTS = ROOT.parent / "results.validation-revision.json"


def main():
    report = json.loads(RESULTS.read_text())
    assert report["warmup_iters"] == 20 and report["measured_iters"] == 100
    cases = report["cases"]
    assert len(cases) == 124
    lookup = {}
    validation_cases = 0
    for case in cases:
        state = case["state"]
        assert state["kind"] == "ok", case
        lookup[case["fixture"], case["implementation"], case["parse_format"], case["serialize_format"]] = state
        for phase in ("parse", "serialize", "validate"):
            if phase + "_ns" not in state:
                continue
            samples = state[phase + "_ns"]
            assert len(samples) == 100 and all(n > 0 for n in samples)
            ordered = sorted(samples)
            expected = {"median_ns": int(statistics.median(samples)), "p25_ns": ordered[24],
                        "p75_ns": ordered[74], "p99_ns": ordered[98]}
            assert state[phase + "_stats"] == expected, (case, phase)
        if "validate_ns" in state:
            validation_cases += 1
            outcome = state["validation"]
            if case["implementation"] == "sbol-rs":
                assert outcome["errors"] == 0 and outcome["warnings"] == 0
                if case["version"] == "sbol3":
                    assert len(outcome["fully_applied"]) == 105
                    assert len(outcome["partially_applied"]) == 4
            elif case["implementation"] == "libsbolj3":
                assert outcome["messages"] == []
                assert outcome["complete"] and outcome["recommended"]
                assert not outcome["automatic_validation"]
            else:
                assert outcome["errors"] == [] and outcome["warnings"] == []
    assert validation_cases == 16

    manifest = {entry["file"]: entry for entry in json.loads((ROOT / "input-manifest.json").read_text())}
    for entry in manifest.values():
        content = (ROOT / "inputs" / entry["file"]).read_bytes()
        assert len(content) == entry["bytes"]
        assert hashlib.sha256(content).hexdigest() == entry["sha256"]
        if entry["triples"] is not None:
            assert len(content.splitlines()) == entry["triples"]
    with (ROOT / "rule-source-index.csv").open() as stream:
        rules = list(csv.DictReader(stream))
    assert len(rules) == 109 and len({r["rule"] for r in rules}) == 109

    print("Verified 124 cases, all sample statistics, 16 validation outcomes, 32 input hashes, and 109 source-index rows.")


if __name__ == "__main__":
    main()
