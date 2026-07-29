# Changelog

All notable changes to this workspace are documented here. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
this workspace follows [Cargo's SemVer for `0.x`](https://doc.rust-lang.org/cargo/reference/semver.html)
crates: breaking changes are permitted between minor versions
(`0.1` → `0.2`) until `1.0`.

## [Unreleased]

## [0.3.0] - 2026-07-29

### Changed

- **`sbol`**: SBOL 2, SBOL 3, and bidirectional conversion are now
  always available as `sbol::v2`, `sbol::v3`, and `sbol::convert`.
  Consumers no longer need to select those APIs with Cargo features.
- **`sbol3` / `sbol-cli`**: HTTP and caching HTTP resolvers are now
  built in. `sbol validate --external-mode allowed` works without a
  custom-feature build, while still requiring `--cache-dir` to keep
  external resolution deterministic.
- **`sbol-cli`**: SARIF output is now included in every build; use
  `sbol validate --format sarif` without enabling a `sarif` feature.
- **`sbol-ontology`**: refreshed the integrity-pinned EDAM snapshot to
  `dev(1.25-20260626T1230Z)` and ChEBI to release 253, including the
  corresponding bundled term and branch facts.
- **Workspace**: updated `ureq` from 2.12 to 3.3 and adopted the new
  client and response APIs in the HTTP resolvers and SynBioHub example.
- **Repository metadata**: updated canonical source, documentation,
  SARIF, and release links after the move to `SynBioDex/sbol-rs`.

### Removed

- **Cargo features**: removed the `sbol` crate's `v2`, `v3`,
  `convert`, and `http-resolver` features; `sbol3` and `sbol-convert`'s
  `http-resolver` features; and `sbol-cli`'s `http-resolver` and
  `sarif` features. The formerly gated facade modules, resolvers, and
  CLI output are now available unconditionally, but downstream
  `features = [...]` declarations that name the removed features must
  be deleted.

## [0.2.2] - 2026-07-18

### Added

- **`sbol2`** (new crate): typed SBOL 2.3.0 data model, builders,
  RDF/XML and Turtle I/O, document traversal, and validation. The
  validator implements all 222 machine-checkable rules in the
  268-rule catalog, with a firing negative and non-firing positive
  fixture for every implemented rule and configurable compliant,
  complete, best-practice, and types-in-URI gates.
- **`sbol-core`** and **`sbol-rulegen`** (new crates): shared,
  version-neutral document, identity, schema, diff, validation, and
  rule-catalog infrastructure used by the SBOL 2 and SBOL 3
  implementations.
- **`sbol`**: umbrella facade exposing the versioned implementations as
  `sbol::v2` and `sbol::v3`, conversion as `sbol::convert`, and
  `SbolVersion`, `detect_version{,_in_graph}`, and `AnyDocument` for
  version-neutral workflows.
- **`sbol-convert`** (new crate): the SBOL 2 ⇄ SBOL 3 conversion
  engine, with committed differential tests against
  SynBioDex/SBOL-Converter in both directions.
- **`sbol-cli`**: automatic SBOL 2/SBOL 3 detection for `validate`,
  `--sbol-version` selection for validation and rule catalogs, and
  flags for compliant, complete, best-practice, and types-in-URI
  validation families.
- **Document diffing**: `Document::diff` for SBOL 2 and SBOL 3,
  structured `Diff` / `ObjectDiff` / `PropertyChange` results, and an
  `sbol diff` command with human-readable and JSON output.
- **`sbol3::design`**: mutable arena-style `Design` API for composing
  components, sequences, sub-components, and constraints with typed
  handles, aggregate error reporting, and lowering to an immutable
  `Document`.
- **`sbol-utilities`** (new crate): biology-first construction verbs,
  assembled-sequence computation, combinatorial-derivation expansion,
  and helpers for functional components and common DNA/RNA/protein
  parts.
- **`sbol-fasta` / `sbol-genbank`**: SBOL 3 → FASTA and SBOL 3 →
  GenBank exporters, complementing the existing importers.
- **`sbol-py`** (new crate): PyO3-based Python package named `sbol`
  covering SBOL 2 and SBOL 3 I/O, validation, conversion, design
  construction, sequence utilities, and FASTA/GenBank import and
  export.
- **Specification and conformance assets**: vendored the SBOL 2.3.0
  specification and added SBOLTestSuite, libSBOLj, pySBOL3,
  BioPython, and SBOL-Converter agreement gates across validation,
  RDF I/O, conversion, and sequence-format handling.

### Changed

- **Crate layout**: moved the original SBOL 3 implementation from
  `sbol` into `sbol3`; `sbol` became the umbrella facade. Import SBOL 3
  items from `sbol::v3` or depend on `sbol3` directly instead of using
  the former `sbol::*` root surface.
- **Conversion API**: moved upgrade and downgrade operations off
  SBOL 3 `Document` inherent methods and into free functions under
  `sbol-convert` (also re-exported from `sbol::convert`).
- **Conversion semantics**: aligned identity/version handling, class
  mapping, MapsTo and Interface lowering, and backport metadata with
  SynBioDex/SBOL-Converter. Conversion provenance now uses the
  `https://sbols.org/backport/2_3#` vocabulary.
- **Cross-implementation benchmarks**: expanded the matrix to SBOL 2
  and SBOL 3 parse, serialize, cross-format conversion, and validation
  phases, and refreshed the committed results.

### Fixed

- **`sbol-rdf`**: resolve relative IRIs against a base IRI while
  parsing, allowing SBOLTestSuite documents with relative attachment
  and source references to load.
- **`sbol-genbank`**: corrected feature-key mappings exposed by parity
  checks against `sbol-utilities`' `gb2so.csv`, including the INSDC
  `-10_signal` and `-35_signal` spellings, and added multi-span,
  malformed-input, and BioPython agreement coverage.

## [0.2.1] - 2026-06-27

### Added

- **Release distribution**: cross-platform CI builds and tagged-release
  archives for Linux x86_64, macOS arm64 and x86_64, and Windows
  x86_64, containing the `sbol` CLI plus licenses and README.
- **SBOL 3 validation evidence**: committed negative/positive coverage
  matrix and freshness gate demonstrating that every one of the 109
  machine-checkable SBOL 3.1.0 rules has both a firing violation and a
  non-firing valid fixture.
- **`sbol-ontology`**: `OntologyRegistry` for layering extension
  snapshots over the bundled ontology without allowing extensions to
  overwrite bundled facts.

### Changed

- **`sbol-ontology`**: replaced placeholder ontology-source metadata
  with exact version, retrieval-date, and SHA-256 provenance pins,
  including dated GO and Cell Ontology sources.
- **Internals**: decomposed the CLI, conversion engine, builder,
  validation resolver, ontology generator, benchmarks, and large test
  suites into focused modules without changing their public behavior.
- **Benchmarks**: refreshed the cross-implementation results and added
  p99 latency reporting alongside medians.

## [0.2.0] - 2026-05-21

### Added

- **`sbol`**: `upgrade` module exposing `sbol2_to_sbol3`,
  `parse_and_upgrade`, and the `Document::upgrade_from_sbol2{,_with,_path}`
  entry points, plus `UpgradeOptions`, `UpgradeReport`, `UpgradeWarning`,
  `UpgradeCounts`, `NamespaceSource`, and `MapsToSide`. Reads SBOL 2 RDF
  (the format SynBioHub, the iGEM Registry, and JBEI ICE still serve) and
  emits SBOL 3 RDF that loads and validates like natively-authored SBOL 3.
  Conversion runs at the RDF triple level with no Python or Docker
  sidecar; SBOL 2 provenance (`persistentIdentity`, `version`, original
  type) is preserved under the `http://sboltools.org/backport#`
  namespace so a later downgrade can round-trip losslessly. See
  [`docs/conversion.md`](docs/conversion.md) for the full conversion
  model.
- **`sbol`**: `downgrade` module exposing `sbol3_to_sbol2`,
  `Document::downgrade_to_sbol2{,_with}`, `DowngradeOptions`,
  `DowngradeReport`, `DowngradeWarning`, and `DowngradeCounts`. The
  write-direction counterpart to `upgrade`: produces SBOL 2 RDF from an
  SBOL 3 [`Document`]. Documents that originated as SBOL 2 round-trip
  losslessly via the backport namespace; native SBOL 3 designs that
  combine structural (sequence, sub-parts) and functional (interactions,
  interface) data on one `Component` are split into a paired
  `ComponentDefinition` + `ModuleDefinition` with a
  `DowngradeWarning::DualRoleComponent` surfaced in the report.
- **`sbol-genbank`** (new crate): pure-Rust GenBank → SBOL 3 importer.
  Each record becomes one `Component` + one `Sequence`; annotated
  features become `SequenceFeature`s with per-segment `Range`s; common
  GenBank keys (`CDS`, `promoter`, `terminator`, `RBS`, `5'UTR`, …) map
  to canonical Sequence Ontology IRIs, unrecognized keys fall back to
  `SO:0000110` and surface in `ImportReport.warnings`. Topology
  (`linear` / `circular`) is added to `Component.type`; molecule type
  (`DNA` / `RNA` / `AA`) chooses the SBO component type and EDAM
  sequence encoding. SynBioHub's mixed-case LOCUS month names are
  tolerated. Parsing is delegated to [`gb-io`].
- **`sbol-fasta`** (new crate): pure-Rust FASTA → SBOL 3 importer with
  no third-party parser dependency. Each `>header` record becomes one
  `Component` + one `Sequence`; the component's biological type
  (DNA / RNA / protein) and the EDAM encoding are auto-detected from
  the alphabet of the sequence, overridable via
  `FastaImporter::with_alphabet` for ambiguous data. Accepts `.fasta`,
  `.fa`, `.fna`, and `.faa`.
- **`sbol-cli`**: four new subcommands — `sbol upgrade` (SBOL 2 → SBOL 3),
  `sbol downgrade` (SBOL 3 → SBOL 2), `sbol import-genbank`
  (GenBank → SBOL 3), and `sbol import-fasta` (FASTA → SBOL 3) — all
  with `--validate`, `--strict`, and configurable target serialization.
  `sbol downgrade --validate` round-trips the result back through
  `sbol::upgrade` and runs SBOL 3 validation, since this workspace has
  no native SBOL 2 validator.
- **`sbol`**: `owl_conformance` module exposing
  `analyze_owl_conformance`, `render_owl_conformance_report`,
  `OwlConformanceReport`, `OwlIdentifiers`, `OwlPinInfo`, and the
  `OWL_ONLY_ALLOWLIST` / `RUST_ONLY_ALLOWLIST` constants. The module
  parses a pinned copy of `sbol-owl3-gen/sbol3.rdf` from
  [SynBioDex/sbol-owl3](https://github.com/SynBioDex/sbol-owl3) — the
  canonical OWL serialization of the SBOL 3 data model — and compares
  its `http://sbols.org/v3#` IRI set against the constants declared in
  `crates/sbol/src/vocab.rs`. Two allowlists document every intentional
  divergence; everything else is treated as actionable drift.
- **`sbol`** (test-only): two integration tests enforce the
  conformance contract on every `cargo test` run.
  `sbol_owl3_conformance.rs` fails CI when the IRI surfaces drift
  outside the allowlists or when either allowlist goes stale.
  `sbol_owl3_conformance_report.rs` fails CI when the committed
  report at `docs/sbol-owl3-conformance.md` is out of date.
- **`sbol`** (tool-only): `generate-sbol-owl3-conformance-report`
  binary that renders the auditable markdown report consumed by the
  freshness test. See
  [`docs/sbol-owl3-conformance.md`](docs/sbol-owl3-conformance.md)
  for the current pinned-state coverage table and
  [`docs/ontology-conformance.md`](docs/ontology-conformance.md) for
  the broader regression-system design.
- **`sbol-ontology`** (tool-only): `update-sbol-owl3-fixture` binary
  that re-pins the fixture against the current upstream `main` commit
  and rewrites `manifest.toml`.

### Fixed

- **`sbol`**: reject `http://sbols.org/v3#zero` as a `VariableFeature`
  cardinality. SBOL 3.1.0 Table 14 enumerates only `zeroOrOne`, `one`,
  `zeroOrMore`, and `oneOrMore`; the previous build accepted `#zero` and
  treated it as "count must be 0" via `cardinality_allows`. Rule
  `sbol3-12201` now reports the value as an unsupported cardinality.
- **`sbol`**: reject `http://sbols.org/v3#none` as a Feature or Location
  `orientation`. SBOL 3.1.0 §6.4.1 Tables 5 and 6 enumerate exactly four
  orientation URIs (`sbol:inline`, `sbol:reverseComplement`,
  `SO:0001030`, `SO:0001031`); the previous build silently accepted
  `sbol:none` because the value was in `ORIENTATION_IRIS`. Rules
  `sbol3-10702` and `sbol3-11301` now reject the value.

### Removed

- **`sbol`**: `sbol::constants::CARDINALITY_ZERO`. The constant pointed
  at an IRI that is not in SBOL 3.1.0 Table 14 and is therefore invalid
  as a `VariableFeature.cardinality` value. Use `CARDINALITY_ZERO_OR_ONE`
  or `CARDINALITY_ZERO_OR_MORE` instead, depending on the intended
  semantics.
- **`sbol`**: `sbol::constants::ORIENTATION_NONE`. The constant pointed
  at `http://sbols.org/v3#none`, an IRI that is not in SBOL 3.1.0 Tables
  5 or 6 and therefore invalid as a Feature or Location `orientation`
  value. To represent "no orientation," omit the `orientation` property
  entirely; the spec marks it ZERO OR ONE.

## [0.1.0]

Initial release. All four workspace crates (`sbol`, `sbol-rdf`,
`sbol-ontology`, `sbol-cli`) ship together at `0.1.0`.

### Added

- **`sbol`**: typed Rust API for reading, constructing, inspecting,
  and rewriting SBOL 3.1.0 documents. Owned typed structs for every
  SBOL class (`Component`, `Sequence`, `SubComponent`, …) with
  validated builders. Reference traversal across single documents and
  composed `DocumentSet`s. `Document::check` / `validate` /
  `check_complete` cover the full SBOL 3.1.0 rule set.
- **`sbol`**: deterministic offline validator covering 109 of 109
  machine-checkable SBOL 3.1.0 rules, with per-rule `sbol3-*`
  identifiers, severity overrides (`--allow` / `--deny` / `--warn`),
  and three output formats (text, JSON v1, SARIF v2.1.0).
- **`sbol-rdf`**: RDF primitives (`Iri`, `Literal`, `Resource`,
  `Term`, `Triple`, `Graph`) and multi-format I/O for Turtle, RDF/XML,
  JSON-LD, and N-Triples, behind a parser-agnostic backend trait.
- **`sbol-ontology`**: bundled offline ontology fact snapshots for
  EDAM, SBO, SO, GO, ChEBI, and Cell Ontology, plus a runtime cache
  for opt-in extensions (NCIT, lab-specific ontologies).
- **`sbol-cli`**: `sbol` binary with `validate`, `ontology install`,
  `ontology list`, `ontology verify`, and `ontology remove`
  subcommands. Documented exit codes (`docs/validation-output.md`).
- **Cross-implementation conformance harness.** Round-trip tests
  against pinned-Docker libSBOLj3 and pysbol3 reference outputs for
  33 fixtures × 4 RDF formats.
- **Loss-conscious round-trip.** Unknown extension triples on known
  SBOL objects and `sbol:Identified`-only subjects are preserved
  through parse → write.

### Stability

`0.x` per Cargo's SemVer: breaking changes between `0.1` → `0.2` are
allowed and will be called out in release notes. Covered surface: the
public Rust API of the workspace crates, the `sbol` CLI exit codes,
and the JSON v1 validation output schema.

[Unreleased]: https://github.com/SynBioDex/sbol-rs/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/SynBioDex/sbol-rs/releases/tag/v0.3.0
[0.2.2]: https://github.com/SynBioDex/sbol-rs/releases/tag/v0.2.2
[0.2.1]: https://github.com/SynBioDex/sbol-rs/releases/tag/v0.2.1
[0.2.0]: https://github.com/SynBioDex/sbol-rs/releases/tag/v0.2.0
[0.1.0]: https://github.com/SynBioDex/sbol-rs/releases/tag/v0.1.0
