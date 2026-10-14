"""Audit the matched I/O-only follow-up to the original parsing comparison."""

import json
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def median(directory, implementation, validation):
    stem = ROOT / directory / f"{implementation}_100000"
    record = json.loads(stem.with_suffix(".record.json").read_text())
    samples = json.loads(stem.with_suffix(".json").read_text())
    assert record["exit_code"] == 0 and record["backend"] == "native"
    assert samples["warmup_iters"] == 3 and samples["measured_iters"] == 5
    assert ("validate_ns" in samples) == validation
    assert len(samples["parse_ns"]) == 5 and all(x > 0 for x in samples["parse_ns"])
    assert record["input"]["sha256"] == "a211822dd9644f7006ea469679cc651f0ef18cf759998af5dd6e93e2ab6b8d93"
    return statistics.median(samples["parse_ns"]) / 1e9


def main():
    original_rust = median("results", "rust", True)
    original_java = median("io-results", "java", False)
    result = {"original": {"rust_seconds": original_rust, "java_seconds": original_java,
                           "rust_over_java": original_rust / original_java,
                           "caveat": "Rust interleaves validation; Java does not."}, "matched": []}
    for directory in ("parse-check", "parse-check-reversed"):
        rust = median(directory, "rust", False)
        java = median(directory, "java", False)
        result["matched"].append({"directory": directory, "rust_seconds": rust,
                                  "java_seconds": java, "rust_over_java": rust / java,
                                  "java_time_reduction_percent": 100 * (1 - java / rust)})
    result["scope"] = "Two sequential matched I/O-only runs on the same 100,000-object RDF/XML input, reversing implementation order. Diagnostic follow-up; does not replace the primary scaling matrix or establish a language-level ranking."
    result["source_observation"] = "Rust v1.0.0 Document::read builds an RDF graph, identity/property indexes, and owned typed objects. libSBOLj3 1.0.5.2 reads a Jena model and wraps it; SBOL object lists are populated on access. No profiler attribution of the timing gap was performed."
    (ROOT / "parsing-comparison.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
