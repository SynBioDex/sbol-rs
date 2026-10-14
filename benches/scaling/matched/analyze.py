"""Verify matched-endpoint samples and generate the summary tables and plot."""
import gzip
import hashlib
import json
import math
import os
import statistics
import sys
from pathlib import Path
import xml.etree.ElementTree as ET

os.environ.setdefault("MPLCONFIGDIR", "/private/tmp/sbol-scaling-mpl")
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent
PREVIEWS = ROOT.parents[2] / "target/benchmarks/figures"
COUNTS = [1000, 10000, 100000]
LABELS = {"rust": "Rust", "python": "Python", "java": "Java", "javascript": "JavaScript"}
COLORS = {"rust": "#0072B2", "python": "#D55E00", "java": "#009E73", "javascript": "#CC79A7"}


def input_fingerprint(entry):
    """Audit retained inputs without requiring the benchmark's temporary paths."""
    data = gzip.decompress((ROOT.parent / "inputs" / entry["file"]).read_bytes())
    assert len(data) == entry["bytes"] and hashlib.sha256(data).hexdigest() == entry["sha256"]
    rdf = "{http://www.w3.org/1999/02/22-rdf-syntax-ns#}"
    rows = []
    for obj in ET.fromstring(data):
        for prop in obj:
            value = prop.attrib.get(rdf + "resource")
            kind = "I" if value is not None else "N" if prop.attrib.get(rdf + "datatype", "").endswith("#integer") else "S"
            if value is None: value = prop.text
            predicate = prop.tag[1:].replace("}", "")
            rows.append(f"{obj.attrib[rdf+'about']}\t{predicate}\t{kind}\t{value}\n")
    assert len(rows) == entry["triples"]
    digest = hashlib.sha256()
    for row in sorted(rows): digest.update(row.encode())
    return digest.hexdigest()


def read_case(directory, implementation, count, mode, warmup, iterations, fingerprint):
    stem = directory / f"{implementation}_{count}_{mode}"
    record = json.loads(stem.with_suffix(".record.json").read_text())
    samples = json.loads(stem.with_suffix(".json").read_text())
    memory = json.loads(stem.with_suffix(".rss.json").read_text())
    assert record["exit_code"] == memory["exit_code"] == 0
    assert record["input"]["objects"] == count
    assert record["endpoint_sha256"] == fingerprint
    assert samples["implementation"] == implementation and samples["mode"] == mode
    assert samples["warmup"] == warmup and samples["iterations"] == iterations
    assert samples["fingerprints"] == [fingerprint] * iterations
    assert samples["row_counts"] == [record["input"]["triples"]] * iterations
    assert memory["peak_rss_kib"] > 0
    assert len(samples["time_ns"]) == len(samples["serialize_ns"]) == iterations
    assert all(t > 0 for t in samples["time_ns"])
    if mode == "ready":
        assert all(t > 0 for t in samples["serialize_ns"]) and samples["serialized_bytes"] > 0
    else:
        assert samples["serialize_ns"] == [0] * iterations and samples["serialized_bytes"] == 0
    return samples, memory, record, hashlib.sha256(stem.with_suffix(".json").read_bytes()).hexdigest()


def stats(values):
    ordered = sorted(values)
    return dict(median_ns=int(statistics.median(values)), p25_ns=ordered[1], p75_ns=ordered[3],
                min_ns=ordered[0], max_ns=ordered[-1])


def integer(value):
    return f"{value:,}".replace(",", "{,}")


def milliseconds(nanoseconds):
    return f"{nanoseconds/1e6:,.1f}".replace(",", "{,}")


def main():
    manifest = {x["objects"]: x for x in json.loads((ROOT.parent / "input-manifest.json").read_text())}
    cases, lookup = [], {}
    for count in COUNTS:
        fingerprint = input_fingerprint(manifest[count])
        for implementation in LABELS:
            case = dict(implementation=implementation, objects=count, input=manifest[count],
                        endpoint_sha256=fingerprint, raw_sha256={})
            for mode in ("rdf", "ready"):
                samples, _, record, digest = read_case(ROOT / "results", implementation, count, mode, 3, 5, fingerprint)
                assert record["input"] == manifest[count]
                case[mode] = stats(samples["time_ns"])
                case["raw_sha256"][mode] = digest
                if mode == "ready":
                    case["serialize"] = stats(samples["serialize_ns"])
                    case["serialized_bytes"] = samples["serialized_bytes"]
            peaks = []
            for trial in range(1, 4):
                _, memory, record, digest = read_case(ROOT / f"memory/trial-{trial}", implementation, count, "ready", 0, 1, fingerprint)
                assert record["input"] == manifest[count]
                peaks.append(memory["peak_rss_kib"])
                case["raw_sha256"][f"memory-{trial}"] = digest
            case.update(peak_rss_kib=int(statistics.median(peaks)), peak_rss_samples_kib=peaks)
            lookup[implementation, count] = case
            cases.append(case)
    growth = {implementation: {
        phase: dict(ratio_1000_to_100000=lookup[implementation, 100000][phase]["median_ns"] /
                    lookup[implementation, 1000][phase]["median_ns"],
                    endpoint_exponent=math.log(lookup[implementation, 100000][phase]["median_ns"] /
                        lookup[implementation, 1000][phase]["median_ns"]) / math.log(100))
        for phase in ("rdf", "ready", "serialize")} for implementation in LABELS}
    speedups = {phase: {str(count): min(lookup[i, count][phase]["median_ns"] for i in LABELS if i != "rust") /
                       lookup["rust", count][phase]["median_ns"] for count in COUNTS} for phase in ("rdf", "ready")}
    (ROOT / "summary.json").write_text(json.dumps(dict(cases=cases, growth=growth, speedups=speedups), indent=2) + "\n")

    main_table = [r"% Generated by benchmarks/scaling/matched/analyze.py; times in milliseconds.",
                  r"\begin{tabular*}{\linewidth}{@{\extracolsep{\fill}}rrrlrrrr@{}}", r"\toprule",
                  r"& & & & \multicolumn{4}{c}{Median time (ms)} \\", r"\cmidrule(l){5-8}",
                  r"Objects & Size (MB) & Triples & Work timed & Rust & Python & Java & JavaScript \\", r"\midrule"]
    for count in COUNTS:
        for phase, label in (("rdf", "Parse RDF"), ("ready", "Read all fields")):
            values = [lookup[i, count][phase]["median_ns"] for i in LABELS]
            cells = [rf"\textbf{{{milliseconds(v)}}}" if v == min(values) else milliseconds(v) for v in values]
            size = [integer(count), f"{manifest[count]['bytes']/1e6:.2f}", integer(manifest[count]["triples"])]
            main_table.append(" & ".join([*(size if phase == "rdf" else ["", "", ""]), label, *cells]) + r" \\")
        if count != COUNTS[-1]: main_table.append(r"\addlinespace")
    main_table.extend([r"\bottomrule", r"\end{tabular*}", ""])
    tables = ROOT
    (tables / "matched-main-table.tex").write_text("\n".join(main_table))

    table = [r"\begin{longtable}{@{}rlrrrr@{}}",
        r"\caption{Matched native macOS measurements. Times are medians of five iterations after three warmups. RDF denotes RDF parsing alone; SBOL fields denotes reading and copying every field in the synthetic profile through the SBOL API. Serialization follows the SBOL-field endpoint. No validation is performed. RSS is the median peak of three cold read--copy--serialize--verify processes, including runtime, input, snapshots, serialization buffers, and untimed verification.}\label{tab:scaling-results}\\",
        r"\toprule", r"Objects & Client & RDF (s) & SBOL fields (s) & Serialize (s) & Peak RSS (MiB) \\",
        r"\midrule\endfirsthead", r"\toprule",
        r"Objects & Client & RDF (s) & SBOL fields (s) & Serialize (s) & Peak RSS (MiB) \\",
        r"\midrule\endhead"]
    for count in COUNTS:
        for implementation in LABELS:
            c = lookup[implementation, count]
            cells = [f"{c[p]['median_ns']/1e9:.3f}" for p in ("rdf", "ready", "serialize")]
            table.append(" & ".join([integer(count), LABELS[implementation], *cells,
                                     f"{c['peak_rss_kib']/1024:.1f}"]) + r" \\")
        if count != COUNTS[-1]: table.append(r"\midrule")
    table.extend([r"\bottomrule", r"\end{longtable}", ""])
    (tables / "scaling-tables.tex").write_text("\n".join(table))
    if "--tables-only" in sys.argv[1:]:
        return

    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 9, "axes.spines.top": False,
                         "axes.spines.right": False, "pdf.fonttype": 42, "ps.fonttype": 42})
    fig, axes = plt.subplots(2, 2, figsize=(7.2, 5.0), constrained_layout=True)
    for ax, phase, title in zip(axes.flat, ("rdf", "ready", "serialize", "memory"),
                               ("(a) RDF parsing only", "(b) Read and copy SBOL fields",
                                "(c) Serialization after (b)", "(d) Peak process memory")):
        for implementation, label in LABELS.items():
            series = [lookup[implementation, count] for count in COUNTS]
            if phase == "memory":
                values = [c["peak_rss_kib"]/1024 for c in series]
                lower = [min(c["peak_rss_samples_kib"])/1024 for c in series]
                upper = [max(c["peak_rss_samples_kib"])/1024 for c in series]
            else:
                values = [c[phase]["median_ns"]/1e9 for c in series]
                lower = [c[phase]["p25_ns"]/1e9 for c in series]
                upper = [c[phase]["p75_ns"]/1e9 for c in series]
            ax.fill_between(COUNTS, lower, upper, color=COLORS[implementation], alpha=0.13, linewidth=0)
            ax.plot(COUNTS, values, marker="o", markersize=4, linewidth=1.5, color=COLORS[implementation], label=label)
        ax.set_xscale("log"); ax.set_yscale("log")
        ax.set_xticks(COUNTS, [r"$10^3$", r"$10^4$", r"$10^5$"])
        ax.set_xlabel("SBOL objects (including nested objects)")
        ax.set_ylabel("Peak RSS (MiB)" if phase == "memory" else "Time (s)")
        ax.set_title(title, loc="left", fontsize=10)
        ax.grid(True, which="major", linewidth=0.4, alpha=0.25); ax.minorticks_off()
    handles, labels = axes[0, 0].get_legend_handles_labels()
    fig.legend(handles, labels, loc="outside upper center", ncol=4, frameon=False)
    fig.savefig(ROOT / "scaling.pdf", metadata={"Title": "Matched SBOL3 loading and memory benchmark"})
    previews = PREVIEWS
    previews.mkdir(parents=True, exist_ok=True)
    fig.savefig(previews / "scaling.png", dpi=220)
    plt.close(fig)
    print("Verified 24 matched timing cases and 36 cold-process cases, including every recorded endpoint fingerprint.")
    print("Ready endpoint speedups over the fastest comparator:", speedups["ready"])
    for implementation in LABELS:
        c = lookup[implementation, 100000]
        print(implementation, {p: c[p]["median_ns"]/1e9 for p in ("rdf", "ready", "serialize")},
              "MiB", c["peak_rss_kib"]/1024)


if __name__ == "__main__": main()
