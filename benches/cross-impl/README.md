# Cross-implementation performance benchmarks

sbol-rs reads, writes, converts, and validates both SBOL 2 and SBOL 3.
This harness benchmarks
`parse`, `serialize`, `convert`, and `validate` for **both versions**,
comparing sbol-rs against the mainstream implementation of each version.
Every implementation, sbol-rs included, runs in a Docker image with pinned library versions and reads the same input bytes per format. Validation rule sets differ: the validation rows compare configured API costs, not identical rule workloads.

| Version | Implementation | Source                                                  | Pinned version | Image tag         |
| ------- | -------------- | ------------------------------------------------------- | -------------- | ----------------- |
| 2 + 3   | sbol-rs        | this repository                                         | `1.0.0`        | `sbol-rs-bench`   |
| 2       | libSBOLj       | https://github.com/SynBioDex/libSBOLj                   | `2.4.0`        | `libsbolj2-bench` |
| 3       | pySBOL3        | https://github.com/SynBioDex/pySBOL3                    | `1.2`          | `pysbol3-bench`   |
| 3       | libSBOLj3      | https://github.com/SynBioDex/libSBOLj3                  | `1.0.5.2`      | `libsbolj3-bench` |
| 3       | sboljs         | https://github.com/SynBioDex/sboljs3 (npm `sboljs@3.x`) | `3.0.2`        | `sboljs3-bench`   |

The orchestrator (`crates/sbol-bench`) pre-converts each fixture into
every RDF serialization on disk so every implementation sees the same
byte-for-byte input, then runs `<warmup>` untimed iterations followed
by `<measured>` timed iterations of `parse(in) -> serialize(out)` for
each `(version, impl, parse_format, serialize_format)` combination it
knows about. Per-iteration nanosecond timings come back as JSON, are
aggregated into a report, and written verbatim to the path supplied with `--report`. The corrected capture is [`results.validation-revision.json`](results.validation-revision.json); [`results.json`](results.json) is the historical capture.

The matrix covers two kinds of row. A **round-trip** row parses and
serializes the same format (`turtle -> turtle`). A **conversion** row
parses one format and serializes another (`turtle -> rdfxml`,
`rdfxml -> turtle`, `turtle -> jsonld`), so the serialize phase
captures true format-conversion cost rather than a same-format
re-emit. The report labels each row's `kind` accordingly.

Every implementation that ships a validator runs a **validation** phase
on the canonical same-format round-trip row for its version: sbol-rs on
both SBOL 2 and SBOL 3, and pySBOL3 and libSBOLj3 on SBOL 3
(`Document::validate()`, `sbol3.Document.validate()`, and
`SBOLValidator.getValidator().validate(doc)` respectively). Validation
operates on the parsed in-memory model, which is format-independent,
so one validation row per implementation is enough. sboljs ships no
validator, and the SBOL 2 libSBOLj driver runs none, so those rows
never run this phase and the `val.` columns show `—`.

The sbol-rs row also runs inside Docker by default. Running it natively
on the host would use a different environment from the other rows. The Docker default keeps the environment consistent across implementations. For local development (no
Docker required) set `SBOL_BENCH_DOCKER=0` and sbol-rs will fall back
to in-process timing for both versions; the foreign rows are then
marked skipped.

## Running

```sh
# One-time, per implementation. The sbol-rs context is the workspace
# root because its Dockerfile compiles the whole workspace, so it
# needs `-f` to find the right Dockerfile.
docker build -t libsbolj2-bench benches/cross-impl/libsbolj2/
docker build -t pysbol3-bench   benches/cross-impl/pysbol3/
docker build -t libsbolj3-bench benches/cross-impl/libsbolj3/
docker build -t sboljs3-bench   benches/cross-impl/sboljs3/
docker build -t sbol-rs-bench   -f benches/cross-impl/sbol-rs/Dockerfile .

# Then:
cargo run --release -p sbol-bench
```

The SBOL 2 fixtures are committed under `tests/fixtures/sbol2/`. The
SBOL 3 fixtures come from the SBOL test suite and must be populated
first — run the `sbol3_fixtures` integration test once if
`tests/fixtures/sbol3/SBOLTestSuite/` is empty.

## Knobs

All read from the environment, prefixed `SBOL_BENCH_`:

| Variable               | Default                                  | Effect                                              |
| ---------------------- | ---------------------------------------- | --------------------------------------------------- |
| `SBOL_BENCH_WARMUP`    | 20                                       | untimed iterations per case                         |
| `SBOL_BENCH_ITERS`     | 100                                      | timed iterations per case                           |
| `SBOL_BENCH_FIXTURES`  | all eight fixtures                       | comma-separated fixture stems (subset of the eight) |
| `SBOL_BENCH_DOCKER`    | unset                                    | set to `0` to skip every foreign impl (native only) |
| `SBOL_BENCH_REPORT`    | unset                                    | path to write a JSON report of every per-iter sample |

The fixture stems are `sbol2_cd_sa_range`, `sbol2_component_output`,
`sbol2_bba_k093005`, `sbol2_bba_f2620` (SBOL 2) and `component`,
`multicellular_simple`, `bba_f2620_popsreceiver`, `toggle_switch_v2`
(SBOL 3).

The default uses 20 warmup and 100 measured iterations per case. This is a fixed sampling protocol, not proof of JIT convergence or tail-latency stability; inspect the saved sample distributions when interpreting a run.

## Format support matrix

### SBOL 2

SBOL 2 is exchanged as RDF/XML. sbol-rs additionally reads and writes
Turtle, JSON-LD, and N-Triples for SBOL 2, so it covers every format.
libSBOLj parses the source RDF/XML natively; its Turtle reader does not
accept sbol-rs's Turtle layout (it silently yields an empty document),
so libSBOLj only appears where the input is RDF/XML, and it ships no
JSON-LD or N-Triples serializer.

| Impl     | rdfxml→rdfxml | turtle→turtle | jsonld→jsonld | ntriples→ntriples | rdfxml→turtle | validate |
| -------- | :-----------: | :-----------: | :-----------: | :---------------: | :-----------: | :------: |
| sbol-rs  | yes           | yes           | yes           | yes               | yes           | yes      |
| libSBOLj | yes           | (n/a)         | (n/a)         | (n/a)             | yes           | (n/a)    |

### SBOL 3

| Impl       | turtle→turtle | rdfxml→rdfxml | jsonld→jsonld | ntriples→ntriples | conversions | validate |
| ---------- | :-----------: | :-----------: | :-----------: | :---------------: | :---------: | :------: |
| sbol-rs    | yes           | yes           | yes           | yes               | yes         | yes      |
| pySBOL3    | yes           | yes           | yes           | yes               | yes         | yes      |
| libSBOLj3  | yes           | yes           | yes           | yes               | yes         | yes      |
| sboljs     | (n/a)         | yes           | (n/a)         | (n/a)             | (n/a)       | (n/a)    |

sboljs's underlying `rdfoo` only emits RDF/XML and only parses RDF/XML
or N-Triples, so Turtle and JSON-LD never had a chance. The N-Triples
parse path is also broken regardless of input: rdfoo wires up
`rdf-parser-n3` against an `@rdfjs/sink-map` whose `parser.import` API
contract has drifted, so every NT parse throws `TypeError:
parser.import is not a function` before any triple is produced.
N-Triples is not in the bench matrix for sboljs because of this.

The SBOL 3 RDF/XML row required getting sbol-rs and sboljs to agree on
the input bytes. sbol-rs emits inline `xmlns` on every child element
(valid RDF/XML 1.1) but `rdf-parser-rdfxml` mis-parses those as
blank-node subjects, and rdfoo's serializer then throws `Unknown
termType BlankNode`. libSBOLj3's RDF/XML output uses standard
prefix-style declarations, which every parser handles cleanly. The
SBOL 3 driver therefore uses the libSBOLj3 reference output from
`tests/fixtures/cross-impl/<stem>.libSBOLj3.expected.rdf` as the
RDF/XML input for every SBOL 3 impl. The SBOL 2 RDF/XML input needs no
such substitution: the source fixtures are already standard
prefix-style RDF/XML that both sbol-rs and libSBOLj parse.

## Methodology

- **Hardware**: MacBook Pro, Apple M4 Max (12 performance + 4 efficiency
  cores), 128 GB RAM.
- **OS / runtime**: macOS 26.6 (build 25G5043d), Docker Desktop 29.4.3
  (Linux/arm64 VM on Apple Virtualization).
- **Impl versions**: sbol-rs 1.0.0 (tagged release baseline), libSBOLj 2.4.0
  (SBOL 2), pySBOL3 1.2, libSBOLj3 1.0.5.2, sboljs 3.0.2.
- **Iteration counts**: 20 warmup + 100 measured per `(version, impl,
  fixture, parse_format, serialize_format)`.
- **Timing**: each language's monotonic high-resolution clock —
  `std::time::Instant`, `System.nanoTime`, `time.perf_counter_ns`,
  `process.hrtime.bigint`. Sub-microsecond resolution on every
  platform.
- **Workload**: in-process `parse(text) -> serialize(document)`.
  Input bytes are read into memory once before the loop, so filesystem
  I/O is excluded. Each iteration re-parses from the same in-memory
  bytes; no document is reused across iterations.
- **Container symmetry**: every implementation, sbol-rs included,
  runs inside its own pinned Docker image.
- **Same input bytes per format**: every impl reads identical bytes
  for a given `(fixture, format)` pair. For SBOL 2, the RDF/XML input
  is the source fixture verbatim and Turtle / JSON-LD / N-Triples are
  produced by sbol-rs from it. For SBOL 3, Turtle / JSON-LD /
  N-Triples come from sbol-rs and the RDF/XML input is libSBOLj3's
  reference output (see Format support matrix for why).
- **Measured**: parse, serialize (same-format round trips and
  cross-format conversions), and validation cost for the impls that
  ship a validator on the relevant version.
- **Not measured**: memory residency, GC pause distribution,
  cold-start latency (warmup is inside the same process per impl), or
  correctness. The cross-impl conformance suites
  (`crates/sbol3/tests/cross_impl*.rs`, `crates/sbol2/tests/cross_impl.rs`)
  gate correctness; this harness assumes the implementations already
  agree at the triple-set level and asks how fast each one processes
  the same bytes.

## Corrected validation benchmark (2026-10-09)

The corrected full matrix is saved in `results.validation-revision.json`. All 124 cases completed with 20 warmup and 100 measured iterations per case. Rust uses the tagged v1.0.0 library code with the corrected harness. The original `results.json` and tables below are retained as a historical record.

The libSBOLj3 driver disables validation after property updates, after reading, and before saving. It explicitly enables recommended and complete-document checks. Rust uses default validation options (complete, compliant, best practice; external resolution off), and pySBOL3 uses its default native plus SHACL validator. Validation is timed before serialization on freshly parsed documents. Diagnostics are saved and checked outside the timers, and containers run with networking disabled. All three SBOL3 validators produced no diagnostics on the four timed fixtures.

The pySBOL3 image installs the v1.2 release commit, because the PyPI 1.2 wheel was withdrawn for containing older source. RDFLib and pySHACL are pinned as well. Full runtime and image identities are retained with the manuscript's benchmark artifacts.

The validators do not enforce identical rules. Rust and libSBOLj3 check a Range's upper bound against its Sequence length; pySBOL3 v1.2 checks positive coordinates and ordering but leaves upper bounds unimplemented. Rust and Java use bundled biological ontology data; pySBOL3's SHACL invocation supplies no ontology graph or inference. The results characterize validation API costs, not a speedup for equal rule coverage.

| Fixture | Rust validation (ms) | libSBOLj3 (ms) | pySBOL3 (ms) |
| --- | ---: | ---: | ---: |
| `component` | 0.007 | 0.235 | 31.894 |
| `multicellular_simple` | 0.070 | 0.942 | 34.263 |
| `bba_f2620_popsreceiver` | 0.209 | 1.726 | 42.124 |
| `toggle_switch_v2` | 0.337 | 2.590 | 49.606 |

## Historical captured results

**Historical capture, superseded by the corrected run below.** These measurements used sbol-rs 0.2.1, libSBOLj3's automatic validation during I/O, validation after serialization, and the older PyPI 1.2 wheel. They remain here as an audit trail and must not be combined with the corrected measurements.

One run (id `1783257836`) with the defaults (20 warmup + 100 measured),
every impl in Docker, all eight fixtures. Numbers are **median time in
microseconds, with p99 in parentheses** — lower is better. Every one
of the 124 `(version, impl, fixture, format-pair)` cells ran; none were
skipped.

### SBOL 2 — parse, `parse(format)` median (p99) µs

`CD_SA_Range_Example.xml` (~1.7 KB):

| Impl     | turtle | rdfxml | jsonld | ntriples |
| -------- | -----: | -----: | -----: | -------: |
| sbol-rs  | 23 (166) | 23 (166) | 47 (199) | 22 (57) |
| libSBOLj | — | 326 (804) | — | — |

`ComponentDefinitionOutput.xml` (~13 KB):

| Impl     | turtle | rdfxml | jsonld | ntriples |
| -------- | -----: | -----: | -----: | -------: |
| sbol-rs  | 166 (225) | 167 (275) | 351 (532) | 168 (286) |
| libSBOLj | — | 754 (3,352) | — | — |

`BBa_K093005` SynBioHub export (~21 KB):

| Impl     | turtle | rdfxml | jsonld | ntriples |
| -------- | -----: | -----: | -----: | -------: |
| sbol-rs  | 259 (422) | 253 (2,493) | 534 (864) | 255 (695) |
| libSBOLj | — | 1,139 (7,979) | — | — |

`BBa_F2620` SynBioHub export (~79 KB):

| Impl     | turtle | rdfxml | jsonld | ntriples |
| -------- | -----: | -----: | -----: | -------: |
| sbol-rs  | 955 (1,071) | 988 (3,714) | 2,027 (2,864) | 1,029 (1,299) |
| libSBOLj | — | 1,688 (3,276) | — | — |

### SBOL 2 — serialize (same-format) and convert median (p99) µs

Serialize, `BBa_F2620` (~79 KB):

| Impl     | turtle | rdfxml | jsonld | ntriples |
| -------- | -----: | -----: | -----: | -------: |
| sbol-rs  | 431 (589) | 551 (2,671) | 690 (1,223) | 420 (483) |
| libSBOLj | — | 554 (1,823) | — | — |

Convert (serialize phase of cross-format rows), `BBa_F2620`:

| Impl     | rdfxml→turtle | turtle→rdfxml | rdfxml→jsonld |
| -------- | ------------: | ------------: | ------------: |
| sbol-rs  | 416 (474) | 535 (639) | 669 (838) |
| libSBOLj | 578 (1,743) | — | — |

### SBOL 3 — parse, `parse(format)` median (p99) µs

Rows sorted by `rdfxml` p50 ascending; fastest first.

`toggle_switch_v2.ttl` (~30 KB):

| Impl       | turtle | rdfxml | jsonld | ntriples |
| ---------- | -----: | -----: | -----: | -------: |
| sbol-rs    | 373 (538) | 387 (546) | 799 (1,215) | 404 (484) |
| libSBOLj3  | 1,908 (4,200) | 2,176 (4,285) | 4,437 (5,978) | 2,062 (4,589) |
| sboljs     | — | 2,459 (4,712) | — | — |
| pySBOL3    | 7,435 (19,789) | 9,807 (21,019) | 6,501 (16,915) | 6,906 (17,376) |

`bba_f2620_popsreceiver.ttl` (~16 KB):

| Impl       | turtle | rdfxml | jsonld | ntriples |
| ---------- | -----: | -----: | -----: | -------: |
| sbol-rs    | 274 (354) | 287 (371) | 541 (605) | 279 (342) |
| libSBOLj3  | 1,221 (2,974) | 1,582 (4,327) | 3,150 (5,063) | 1,315 (3,884) |
| sboljs     | — | 1,833 (2,955) | — | — |
| pySBOL3    | 5,177 (17,826) | 6,743 (17,270) | 4,263 (14,814) | 4,601 (14,518) |

`multicellular_simple.ttl` (~5 KB):

| Impl       | turtle | rdfxml | jsonld | ntriples |
| ---------- | -----: | -----: | -----: | -------: |
| sbol-rs    | 99 (163) | 101 (455) | 192 (324) | 91 (138) |
| sboljs     | — | 686 (1,444) | — | — |
| libSBOLj3  | 721 (3,468) | 964 (1,666) | 1,656 (3,135) | 851 (1,879) |
| pySBOL3    | 2,013 (15,174) | 2,363 (6,743) | 1,654 (5,675) | 1,706 (2,827) |

`component.ttl` (~0.7 KB):

| Impl       | turtle | rdfxml | jsonld | ntriples |
| ---------- | -----: | -----: | -----: | -------: |
| sbol-rs    | 6 (8) | 8 (46) | 13 (36) | 6 (9) |
| sboljs     | — | 180 (566) | — | — |
| pySBOL3    | 367 (578) | 378 (659) | 260 (410) | 251 (682) |
| libSBOLj3  | 313 (1,512) | 535 (921) | 470 (3,300) | 312 (1,247) |

### SBOL 3 — serialize (same-format) and convert median (p99) µs

Serialize, `toggle_switch_v2.ttl` (~30 KB):

| Impl       | turtle | rdfxml | jsonld | ntriples |
| ---------- | -----: | -----: | -----: | -------: |
| sbol-rs    | 156 (191) | 233 (257) | 254 (355) | 158 (270) |
| sboljs     | — | 835 (3,298) | — | — |
| libSBOLj3  | 1,765 (3,668) | 2,778 (5,292) | 3,050 (4,480) | 1,572 (3,637) |
| pySBOL3    | 6,325 (19,916) | 2,789 (3,854) | 3,906 (13,683) | 1,796 (12,013) |

Convert (serialize phase of cross-format rows), `toggle_switch_v2.ttl`:

| Impl       | turtle→rdfxml | rdfxml→turtle | turtle→jsonld |
| ---------- | ------------: | ------------: | ------------: |
| sbol-rs    | 222 (301) | 168 (229) | 280 (378) |
| libSBOLj3  | 2,598 (4,616) | 2,074 (4,632) | 3,052 (5,851) |
| pySBOL3    | 2,694 (12,067) | 5,631 (15,406) | 3,959 (6,028) |

### Validate, `validate()` median (p99) µs on the same-format round-trip row

SBOL 2:

| Impl     | cd_sa_range (~1.7 KB) | component_output (~13 KB) | bba_k093005 (~21 KB) | bba_f2620 (~79 KB) |
| -------- | --------------------: | ------------------------: | -------------------: | -----------------: |
| sbol-rs  | 13 (54) | 120 (221) | 99 (150) | 511 (1,362) |
| libSBOLj | — | — | — | — |

SBOL 3:

| Impl      | component (~0.7 KB) | multicellular (~5 KB) | bba_f2620_pops (~16 KB) | toggle_switch (~30 KB) |
| --------- | ------------------: | --------------------: | ----------------------: | ---------------------: |
| sbol-rs   | 7 (10) | 70 (145) | 186 (314) | 287 (429) |
| libSBOLj3 | 99 (363) | 434 (701) | 765 (2,651) | 1,332 (3,755) |
| pySBOL3   | 27,206 (43,347) | 31,920 (49,850) | 38,541 (54,429) | 44,969 (59,750) |

A few things worth keeping in mind when reading the numbers:

- **sbol-rs is the fastest implementation in every cell of both
  versions.** For SBOL 2 it parses RDF/XML 1.7–14× faster than
  libSBOLj across the fixtures and serializes RDF/XML 1.0–4× faster; on
  the largest SynBioHub export the two RDF/XML serialize medians nearly
  converge, but sbol-rs's p99 is tighter. For SBOL 3 it stays far ahead
  of libSBOLj3, sboljs, and pySBOL3.
- Ratios scale with fixture size. On the ~0.7 KB SBOL 3 `component`
  fixture pySBOL3 and libSBOLj3 are 40–60× slower than sbol-rs; on
  `toggle_switch_v2` (~30 KB) those compress to ~20× and ~5×. The
  per-call object-construction overhead each impl pays amortizes
  differently as the document grows.
- Validation is where the gap is widest. sbol-rs validates a ~30 KB
  SBOL 3 document in ~287 µs; libSBOLj3 takes ~1.3 ms and pySBOL3 ~45
  ms — pySBOL3's validator is two to three orders of magnitude slower
  than sbol-rs across every fixture. sbol-rs's own SBOL 2 and SBOL 3
  validators run in the same low-hundreds-of-microseconds band.
- pySBOL3's parse cost is dominated by `rdflib`, which backs
  `sbol3.Document.read_string`. Differences across formats reflect
  rdflib's parser, not the SBOL layer.
- sboljs is competitive on SBOL 3 RDF/XML — for small documents it
  serializes faster than libSBOLj3 — but rdfoo's fragile stack keeps it
  out of every other format row and out of SBOL 2 entirely in this run.
- sbol-rs's p99 stays below every comparator's *median* in nearly every
  cell. Its interquartile range is within a few percent of the median
  on the larger fixtures; on the sub-10 µs `component` fixture the
  p99/p50 ratio can reach ~6–15× because a single rare allocation or
  scheduling spike dominates a microsecond-scale median, but the
  absolute p99 stays well under a millisecond.
