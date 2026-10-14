"""Audit the earlier unequal read-API experiment; emit historical artifacts only.

For the manuscript's matched comparison use matched/analyze.py instead.
"""

import hashlib
import json
import math
import os
import statistics
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", "/private/tmp/sbol-scaling-mpl")
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent
LABELS = {"rust": "Rust", "python": "Python", "java": "Java", "javascript": "JavaScript"}
COLORS = {"rust": "#0072B2", "python": "#D55E00", "java": "#009E73", "javascript": "#CC79A7"}
COUNTS = [1000, 10000, 100000]


def main():
    cases = []
    lookup = {}
    provenance = json.loads((ROOT / "provenance.json").read_text())
    manifest = {entry["objects"]: entry for entry in json.loads((ROOT / "input-manifest.json").read_text())}
    for count in COUNTS:
        for implementation in LABELS:
            stem = ROOT / "results" / f"{implementation}_{count}"
            record = json.loads(stem.with_suffix(".record.json").read_text())
            incomplete = record["exit_code"] != 0
            if incomplete:
                assert implementation == "java" and count == 100000
                budget = json.loads(stem.with_suffix(".budget.json").read_text())
                assert budget["budget_seconds"] == 300 and record["exit_code"] == 143
                stem = ROOT / "io-results" / f"{implementation}_{count}"
                record = json.loads(stem.with_suffix(".record.json").read_text())
                assert not record["validation_requested"]
            samples = json.loads(stem.with_suffix(".json").read_text())
            assert record["exit_code"] == 0
            assert record["backend"] == "native" and record["input"] == manifest[count]
            assert samples["warmup_iters"] == provenance["sampling"]["warmup"] == 3
            assert samples["measured_iters"] == provenance["sampling"]["measured"] == 5
            assert samples["serialized_bytes"] > 0
            validation = samples.get("validation", {})
            if implementation == "rust":
                assert validation["errors"] == validation["warnings"] == 0
            elif implementation == "python":
                assert validation["errors"] == validation["warnings"] == []
            elif implementation == "java" and not incomplete:
                assert validation["messages"] == []
                assert validation["complete"] and validation["recommended"] and not validation["automatic_validation"]
            elif implementation == "javascript" or incomplete:
                assert "validate_ns" not in samples
            peaks = []
            for trial in range(1, 4):
                memory_stem = ROOT / f"memory/trial-{trial}" / f"{implementation}_{count}"
                memory_record = json.loads(memory_stem.with_suffix(".record.json").read_text())
                memory_samples = json.loads(memory_stem.with_suffix(".json").read_text())
                memory = json.loads(memory_stem.with_suffix(".rss.json").read_text())
                assert memory_record["exit_code"] == memory["exit_code"] == 0
                assert memory_record["input"] == manifest[count] and memory_record["backend"] == "native"
                assert not memory_record["validation_requested"]
                assert memory_samples["warmup_iters"] == 0 and memory_samples["measured_iters"] == 1
                assert "validate_ns" not in memory_samples and memory_samples["serialized_bytes"] > 0
                assert memory["peak_rss_kib"] > 0
                peaks.append(memory["peak_rss_kib"])
            case = {"implementation": implementation, "objects": count,
                    "peak_rss_kib": int(statistics.median(peaks)), "peak_rss_samples_kib": peaks,
                    "validation": validation, "validation_incomplete": incomplete,
                    "raw_sha256": hashlib.sha256(stem.with_suffix(".json").read_bytes()).hexdigest()}
            for phase in ("parse", "serialize", "validate"):
                if phase + "_ns" in samples:
                    values = sorted(samples[phase + "_ns"])
                    assert len(values) == 5 and all(value > 0 for value in values)
                    case[phase] = {"median_ns": int(statistics.median(values)), "p25_ns": values[1],
                                   "p75_ns": values[3], "min_ns": values[0], "max_ns": values[-1]}
            lookup[implementation, count] = case
            cases.append(case)
    growth = {}
    for implementation in LABELS:
        growth[implementation] = {}
        for phase in ("parse", "serialize", "validate"):
            if phase not in lookup[implementation, 1000]:
                continue
            available = [count for count in COUNTS if phase in lookup[implementation, count]]
            values = [lookup[implementation, count][phase]["median_ns"] for count in available]
            growth[implementation][phase] = {f"{available[i]}_to_{available[i+1]}": values[i+1] / values[i]
                                             for i in range(len(values)-1)}
            if len(available) == 3:
                growth[implementation][phase].update({"1000_to_100000": values[2] / values[0],
                    "endpoint_exponent": math.log(values[2] / values[0]) / math.log(100)})
    (ROOT / "legacy-summary.json").write_text(json.dumps({"cases": cases, "growth": growth}, indent=2) + "\n")
    table = [
        r"\begin{longtable}{@{}rlrrrr@{}}",
        r"\caption{Native macOS scaling measurements. Times are medians of five iterations after three warmup iterations. RSS is the median peak resident memory of three separate cold processes, each performing one parse and serialization without validation; it includes runtime, startup, and file I/O. JavaScript validation was not measured. An asterisk marks Java's incomplete 100,000-object combined run, stopped at a 300-second whole-process budget; its I/O timings come from a separate run without validation.}\label{tab:scaling-results}\\",
        r"\toprule",
        r"Objects & Implementation & Parse (s) & Serialize (s) & Validate (s) & Peak RSS (MiB) \\",
        r"\midrule\endfirsthead",
        r"\toprule Objects & Implementation & Parse (s) & Serialize (s) & Validate (s) & Peak RSS (MiB) \\",
        r"\midrule\endhead",
    ]
    for count in COUNTS:
        for implementation in LABELS:
            case = lookup[implementation, count]
            times = [f"{case[phase]['median_ns'] / 1e9:.3f}" if phase in case else "--" for phase in ("parse", "serialize", "validate")]
            if case["validation_incomplete"]:
                times[2] = r"--$^{*}$"
            table.append(f"{count:,}".replace(",", "{,}") + " & " + LABELS[implementation] + " & " + " & ".join(times) + f" & {case['peak_rss_kib'] / 1024:,.1f}".replace(",", "{,}") + r" \\")
        if count != COUNTS[-1]:
            table.append(r"\midrule")
    table.extend([r"\bottomrule", r"\end{longtable}", ""])
    (ROOT / "legacy-scaling-table.tex").write_text("\n".join(table))
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 9, "axes.spines.top": False,
                         "axes.spines.right": False, "pdf.fonttype": 42, "ps.fonttype": 42})
    fig, axes = plt.subplots(2, 2, figsize=(7.2, 5.0), constrained_layout=True)
    for ax, phase, title in zip(axes.flat, ("parse", "serialize", "validate", "memory"),
                               ("(a) Parsing", "(b) Serialization", "(c) Validation", "(d) Peak process memory")):
        for implementation, label in LABELS.items():
            if phase != "memory" and phase not in lookup[implementation, 1000]:
                continue
            if phase == "memory":
                available = COUNTS
                values = [lookup[implementation, count]["peak_rss_kib"] / 1024 for count in COUNTS]
                lower = [min(lookup[implementation, count]["peak_rss_samples_kib"]) / 1024 for count in COUNTS]
                upper = [max(lookup[implementation, count]["peak_rss_samples_kib"]) / 1024 for count in COUNTS]
            else:
                available = [count for count in COUNTS if phase in lookup[implementation, count]]
                values = [lookup[implementation, count][phase]["median_ns"] / 1e9 for count in available]
                lower = [lookup[implementation, count][phase]["p25_ns"] / 1e9 for count in available]
                upper = [lookup[implementation, count][phase]["p75_ns"] / 1e9 for count in available]
            ax.fill_between(available, lower, upper, color=COLORS[implementation], alpha=0.13, linewidth=0)
            ax.plot(available, values, marker="o", markersize=4, linewidth=1.5, color=COLORS[implementation], label=label)
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_xticks(COUNTS, [r"$10^3$", r"$10^4$", r"$10^5$"])
        ax.set_xlabel("SBOL objects (including nested objects)")
        ax.set_ylabel("Peak RSS (MiB)" if phase == "memory" else "Time (s)")
        ax.set_title(title, loc="left", fontsize=10)
        ax.grid(True, which="major", linewidth=0.4, alpha=0.25)
        ax.minorticks_off()
    handles, labels = axes[0, 0].get_legend_handles_labels()
    fig.legend(handles, labels, loc="outside upper center", ncol=4, frameon=False)
    figures = ROOT.parents[1] / "target/benchmarks/figures"
    figures.mkdir(parents=True, exist_ok=True)
    fig.savefig(figures / "legacy-scaling.pdf", metadata={"Title": "Historical unequal SBOL3 read-API benchmark"})
    fig.savefig(figures / "legacy-scaling.png", dpi=220)
    plt.close(fig)
    print("Verified historical samples and the incomplete Java case; generated legacy-summary.json and legacy-scaling-table.tex here, with legacy-scaling previews under build/figures/. These read-API timings are not equivalent-work comparisons.")
    for implementation in LABELS:
        last = lookup[implementation, 100000]
        print(implementation, {phase: last[phase]["median_ns"] / 1e9 for phase in ("parse", "serialize", "validate") if phase in last}, "MiB", last["peak_rss_kib"] / 1024)
    print("Growth:", json.dumps(growth, indent=2))


if __name__ == "__main__":
    main()
