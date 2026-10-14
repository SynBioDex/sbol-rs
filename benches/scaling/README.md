# Synthetic SBOL3 scaling experiment

The equivalent-work experiment is in [`matched/`](matched/README.md).
It measures RDF parsing alone and, separately, SBOL reading followed by copying
all fields present in the synthetic profile through each client's SBOL API.
Every endpoint is checked against an independently computed input fingerprint.
The new timing cases never interleave validation. The common cold-process
memory workflow also uses this complete field-access endpoint.

The parent directory keeps an earlier read-API comparison: Rust eagerly constructs typed
objects during reading, whereas Java and JavaScript expose model-backed SBOL
objects and fields lazily. Those API return points do not represent equal
work. The parent `results/`, `io-results/`, `memory/`, and `summary.json` retain
those historical measurements for traceability; the equivalent-work parsing
and memory measurements are in `matched/`. `parse-check/`, `parse-check-reversed/`,
`check_parsing.py`, and `parsing-comparison.json` record an intermediate control
that removed interleaved validation but still stopped at unequal read-API
boundaries. Its two execution orders do not make those endpoints equivalent.

## Inputs and scope

`generate.py` deterministically emits RDF/XML for repeated linked two-part
designs. Each has three Components, three Sequences, two SubComponents, and two
Ranges: ten objects, six top-level objects, and 57 triples. The assembly is 64
bases and the two parts are 32 bases each. All references are internal.

The three sizes contain 1,000, 10,000, and 100,000 objects, including nested
features and locations; 5,700, 57,000, and 570,000 triples; and 502,950,
5,028,150, and 50,280,150 uncompressed bytes. `inputs/` retains deterministic
gzip files and `input-manifest.json` records compressed and uncompressed hashes.
`verify_inputs.py` independently checks unique identities, object counts,
internal references, Range bounds, and matching part/assembly sequences. The
ten-object input is a positive control, excluded from reported results.

This fixed-depth, bounded-fanout family tests increasing numbers of linked
designs. It does not establish asymptotic complexity or performance for deep
hierarchies, cycles, heterogeneous registries, or complete ingestion workflows.

## Releases, tools, and historical validation measurements

`provenance.json` records the Apple M4 Max/macOS host and native runtimes,
pinned libraries, package artifacts, and the earlier drivers. Source pins are
Rust v1.0.0, pySBOL3 v1.2 from its release source, libSBOLj3 1.0.5.2, and sboljs
3.0.2. `source-check.json` verifies that the Rust archive differs from v1.0.0 only
in the corrected benchmark harness. New matched drivers and their separate
provenance are in `matched/`; no SDK implementation was changed.

The earlier combined experiments retain useful **configured validation API**
measurements in `results/`, with three warmup and five measured iterations.
Rust and Python return no diagnostics at all three sizes; Java does so at the
two smaller sizes. Java's 100,000-object combined process was stopped at a
300-second whole-process budget, including startup and untimed preflight. It
supplies neither a validation latency nor a conformance outcome at that size.
The budget is not a lower bound on a single validation call. `limit_java.py`
and the budget record document that limit; `io-results/` holds the subsequent
Java run without validation. JavaScript validation was not measured.

These validation API timings include the checks and ontology settings
documented in [`../cross-impl/capture/`](../cross-impl/capture/README.md). The old validation samples are kept separate from the new matched
loading and serialization experiment. `analyze.py` audits the historical run
and emits only `legacy-*` artifacts, with PDF/PNG previews under
`../../target/benchmarks/figures/`.

## Memory and executable size

Current common-endpoint memory records are in `matched/memory/`, with their
protocol in `matched/README.md`. Earlier parent `memory/` records are historical
read-and-serialize processes that did not traverse all SBOL fields. The shared
`measure_rss.c` wrapper records `wait4()` peaks, converting macOS bytes to KiB.
No heap limits or forced garbage collection are used. Other host services
remain running. Docker pilots affected by shared-VM memory pressure in
`pilot/` are excluded; `pilot-native/` contains excluded positive controls.

`binary-size.json` records default-feature v1.0.0 CLI release builds, compiler,
unstripped/stripped sizes, hashes, and system-library dependencies for macOS
and Linux arm64. Build with `cargo +1.93 build --locked --release -p sbol-cli`,
copy the executable, then strip the copy. Sizes exclude system libraries and
input data. The CLI has not changed for the matched experiment.

## Verification and reproduction

The [matched README](matched/README.md) gives the current build, run, and
analysis commands. The native tools are under `/private/tmp/sbol-scaling-tools`;
the Rust v1.0.0 archive is `/private/tmp/sbol-review-validation`. Recreate them
from the recorded pins or adjust the explicit paths for another machine.

From the repository root:

```sh
python3 benches/scaling/verify_inputs.py
uv run --with matplotlib python benches/scaling/matched/analyze.py
```

The latter audits captured matched results without rerunning benchmarks and
generates the summary tables and plot. To audit only the earlier,
unequal read-API experiment, run `uv run --with matplotlib python
benches/scaling/analyze.py`; its outputs are explicitly historical.
Preserve captured data before rerunning any experiment. Timing differences on
another host or repetition are expected; retain new results separately.
