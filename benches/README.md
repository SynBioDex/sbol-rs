# Workspace benchmarks

Top-level benchmark infrastructure that doesn't naturally belong to a
single crate. Per-crate Criterion microbenchmarks (which track
sbol-rs's own internal performance over time) live in
`crates/sbol/benches/` instead. The distinction:

- `crates/sbol/benches/` → "is sbol-rs faster or slower than it was
  last week?" Run with `cargo bench -p sbol`.
- `benches/` (this directory) → "is sbol-rs faster or slower than the
  other SBOL 3 implementations?" Run with
  `cargo run --release -p sbol-bench`.

Suites here:

- [`cross-impl/`](cross-impl/README.md) — head-to-head performance
  comparison of sbol-rs against pySBOL3, libSBOLj3, and sboljs, each
  pinned in its own Docker image. Orchestrated by `crates/sbol-bench`.
  [`cross-impl/capture/`](cross-impl/capture/README.md) holds the inputs,
  provenance, validation audit, and verifier for the corrected capture.
- [`scaling/`](scaling/README.md) — synthetic SBOL3 documents of 10^3 to
  10^5 objects, comparing equal loading work, serialization, and peak
  process memory across the four clients.
