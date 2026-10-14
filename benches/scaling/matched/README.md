# Matched RDF and SBOL loading endpoints

These measurements compare equal work across the four clients. Rust eagerly builds typed SBOL objects during `Document::read`; Java's
`SBOLIO.read` returns a model-backed document whose objects and properties are
accessed lazily. The original JavaScript timer stopped before constructing its
SBOL graph view. Those API return points are different amounts of work.

The four drivers here use two common functional endpoints, measured in
separate fresh processes on identical RDF/XML bytes:

- `rdf`: parse the complete input into the underlying RDF graph, without SBOL
  interpretation. Calls are `sbol_rdf::Graph::parse`, `rdflib.Graph.parse`,
  `RDFUtil.read` (Jena), and `rdfoo.parseRDF`.
- `ready`: read through the SBOL API, visit every Component, Sequence,
  SubComponent, and Range in this synthetic profile, and copy every input field
  into independent semantic records. This includes identities, classes,
  display identifiers, namespaces, types, roles, references, sequence strings
  and encodings, orientations, and range coordinates. Native RDF/XML
  serialization is then timed separately. No validation is interleaved.

Both eager and lazy clients must expose all fields before the `ready` timer
stops. No implementation's internal representation is prescribed. The common
snapshot adds work beyond simply returning a parsed SDK document; this is an
explicit read-and-copy workload, not a measurement of an isolated parser or
all possible SBOL object classes.

Every field is represented by an ASCII row containing subject IRI, predicate
IRI, value kind (`I` IRI, `S` string, `N` integer), and lexical value, separated
by tabs and terminated by newline. These inputs have no blank nodes, language
tags, embedded tabs/newlines, or other literal datatypes. Outside the timer,
rows are sorted and SHA-256 hashed. In both modes every warmup and measured
iteration must reproduce the independently computed input fingerprint and
triple count. In `rdf` mode record construction is outside the timer; in
`ready` mode it is inside. Sorting and hashing are outside both timers.

## Protocol and provenance

All four implementations use the same three warmup and five measured
iterations, with one fresh process per client, size, and endpoint. Cases run
sequentially. Startup, input reads, and verification are outside timing;
library-internal parsing, indexing, allocation, and field access remain inside.
Java's automatic validation on reading, writing, and setting properties is
disabled. No validator, biological ontology lookup, or external resolution is
invoked. Heap defaults remain in effect and garbage collection is not forced.
Unrelated host services remain running.

The host, release pins, native runtime versions, and dependency package
artifacts are those in `../provenance.json` and the corrected small-fixture
capture in [`../../cross-impl/capture/`](../../cross-impl/capture/README.md). `provenance.json` here identifies the new drivers and
compiled executable. Rust is built from the pinned v1.0.0 archive, without SDK
source changes. `Cargo.lock` and `dependency-check.json` record that every
registry dependency in the standalone benchmark matches the release lockfile.

`memory/trial-{1,2,3}/` contains three fresh processes per client and size,
each performing one `ready` load, one serialization, and endpoint verification
without warmup or validation. The `wait4()` wrapper records whole-process peak
resident memory, including runtime, startup, input, native document, copied
records, serialization buffers, and verification. This is a common workflow
footprint, not isolated SDK model storage. The reported value is the median of
three peaks, with their range shown in the plot. Timing-run RSS is retained but
does not supply these memory results.

`results/` and `memory/` retain all samples, fingerprint and row-count results,
logs, commands, timestamps, exit codes, and RSS records. `pilot/` and
`pilot-rust/` are ten-object positive controls, excluded from reported medians.
Earlier read-API and validation measurements remain in the parent directory;
they are not combined with these timing or memory samples. Validation is a
separate API comparison, with its rule coverage and settings documented in
[`../../cross-impl/capture/`](../../cross-impl/capture/README.md).

## Reproduction

Recreate the native tools and pinned source archive at the paths recorded in
the parent README and provenance, or adapt the explicit paths in `run.py` and
the captured manifest. Compile `../measure_rss.c` for this host. To recreate the
Rust harness in `/private/tmp/sbol-like-for-like`, copy `Cargo.toml.capture` as
`Cargo.toml` and the captured `Cargo.lock`; adjust its source and bin paths only
if the checkouts differ. Build it with:

```sh
cargo +1.93 build --offline --release --locked --manifest-path /private/tmp/sbol-like-for-like/Cargo.toml --target-dir /private/tmp/sbol-review-validation/target
mkdir -p target/benchmarks/java
/private/tmp/sbol-scaling-tools/jdk-17.0.20.1+1/Contents/Home/bin/javac -d target/benchmarks/java -cp /private/tmp/sbol-scaling-tools/libSBOLj3.jar benches/scaling/matched/Matched.java
python3 benches/scaling/matched/suite.py
uv run --with matplotlib python benches/scaling/matched/analyze.py
```

Decompress the retained inputs into `/private/tmp/sbol-scaling-inputs` before
running; the parent `generate.py` and `verify_inputs.py` describe their exact
construction. Preserve captured results before any rerun: `suite.py` writes
the named result directories. Analysis independently verifies input hashes,
endpoint fingerprints, sampling counts, successful exits, and all 36 memory
cases, then generates `summary.json`, `matched-main-table.tex`, `scaling-tables.tex`,
and `scaling.pdf` in this directory. The PNG preview goes to
`../../../target/benchmarks/figures/scaling.png`. Java classes also go under
the ignored `target/` directory.
Captured records and provenance retain the original execution paths and hashes;
the reproduction runner compiles Java into the build directory.
