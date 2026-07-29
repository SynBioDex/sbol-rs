# SBOL 2 → SBOL 3 upgrade conformance

How the `sbol-convert` upgrade functions and the `sbol upgrade`
subcommand are gated in CI. For the conversion model itself (what gets
preserved in the backport namespace, how SequenceAnnotation collapses
work, how MapsTo decomposes), see [`conversion.md`](conversion.md).

The integration test in
[`crates/sbol-convert/tests/upgrade_conformance.rs`](../crates/sbol-convert/tests/upgrade_conformance.rs)
gates every change against committed snapshots of the converter's
output.

## Gates

### Self-snapshot gate (critical, pure Rust)

For every fixture in `tests/fixtures/sbol2/real/`, the test re-runs
`sbol_convert::upgrade_from_sbol2` and diffs the deterministically sorted
N-Triples output against the committed snapshot at
`tests/fixtures/sbol2/real/expected/{name}.nt`. Any unintended change
in converter output fails the gate.

Both sides of the comparison come from `sbol-rs`. The gate proves the
converter is **stable and deterministic**: the same input
always produces the same output, byte for byte. It runs in pure Rust,
with no network, no Docker, and no other tooling required.

### Cross-implementation correctness

Correctness against other SBOL implementations is covered elsewhere
and doesn't gate this harness:

- **Round-trip smoke test** ([`docs/sbol3-round-trip-report.md`](sbol3-round-trip-report.md))
  drives every fixture through `upgrade → downgrade → re-upgrade` and
  verifies the triple set survives bijectively. Anything dropped by
  the upgrade fails the round-trip; the report lists the per-fixture
  result. Regenerate with
  `cargo run -p sbol-convert --bin generate-round-trip-report`.
- **libSBOLj3 cross-impl** ([`crates/sbol3/tests/cross_impl.rs`](../crates/sbol3/tests/cross_impl.rs))
  parses our SBOL 3 output through libSBOLj3 and asserts triple-set
  equivalence with the Java reference.
- **pySBOL3 cross-impl** ([`crates/sbol3/tests/cross_impl_pysbol3.rs`](../crates/sbol3/tests/cross_impl_pysbol3.rs))
  does the same against the Python reference.

Both cross-impl harnesses target the SBOL 3 surface (where the spec
is current), not the SBOL 2 surface (legacy), and compare against
committed snapshots so CI never invokes a foreign tool.

## Fixture corpus

`tests/fixtures/sbol2/real/` groups committed SBOL 2 RDF/XML inputs
by upstream source:

| Subdirectory | Source | Count | Notes |
|---|---|---|---|
| *(flat, no prefix)* | SBOLTestSuite SBOL 2 examples | 10 | Curated edge cases: MapsTo, CombinatorialDerivation, GenericLocation, mixed-version IRIs. Several intentionally non-compliant. |
| `synbiohub/` | iGEM Registry exports via SynBioHub | 7 | Real-world iGEM parts (BBa_E0040 GFP, BBa_R0010 LacI promoter, BBa_F2620 PoPS receiver, …). iGEM namespaces, PROV provenance, multi-line annotations. |
| `from_genbank/` | SBOL 2 derived from GenBank sources | 4 | SBOL 2 in the canonical shape `sbol-rs` produces when ingesting a GenBank file. Regenerated entirely in Rust by `cargo run -p sbol-genbank --bin regenerate-from-genbank-sbol2-intermediates` (GenBank → SBOL 3 via `sbol-genbank` → SBOL 2 via `sbol_convert::downgrade`). The `.gb` sources live at `tests/fixtures/genbank/` and are exercised independently by the [GenBank import conformance](genbank-import-conformance.md) gate. |

The SBOLTestSuite synthetic fixtures stress-test edge-case handling;
the SynBioHub and `from_genbank` fixtures stress-test that nothing
breaks on the shape of data users actually have.

## What the test checks

### `self_snapshot_diff`

For each fixture:

1. Read the input from `tests/fixtures/sbol2/real/{name}.xml`.
2. Call [`sbol_convert::upgrade_from_sbol2`](../crates/sbol-convert/src/upgrade/mod.rs).
3. Sort, dedup, and N-Triples-canonicalize the resulting graph using
   [`sbol_convert::canonical_nt_line`](../crates/sbol-convert/src/upgrade/mod.rs).
4. Diff line-by-line against
   `tests/fixtures/sbol2/real/expected/{name}.nt`.

Any difference fails the test and prints a per-fixture +/- diff
with the refresh command in the failure message.

## Refreshing snapshots

After an intentional converter change:

```sh
cargo run -p sbol-convert --bin regenerate-sbol2-upgrade-snapshots
```

That refreshes the self-snapshot references at
`tests/fixtures/sbol2/real/expected/{name}.nt`. Pure Rust; no Docker.

## When the conformance test fails

- **Self-snapshot drift** → either a regression, or an intentional
  converter change.
  - if unwanted, revert it
  - if intentional, run
    `cargo run -p sbol-convert --bin regenerate-sbol2-upgrade-snapshots`
    and commit the refreshed `expected/*.nt` alongside the code
    change.

## File layout

```
tests/fixtures/sbol2/
├── real/                            # SBOL 2 RDF/XML inputs
│   ├── *.xml                        #   SBOLTestSuite (flat, no prefix)
│   ├── synbiohub/*.xml              #   iGEM/SynBioHub real-world parts
│   ├── from_genbank/*.xml           #   real-world SBOL 2 from GenBank sources
│   └── expected/                    # self-snapshot references (canonical)
│       └── **/*.nt                  #   subdirs mirror real/ layout
└── *.ttl                            # synthetic SBOL 2 fixtures (URN, merge, …)
                                     # used by smaller SDK upgrade tests
```

## Runnable examples

[`crates/sbol-convert/examples/synbiohub_upgrade.rs`](../crates/sbol-convert/examples/synbiohub_upgrade.rs)
fetches a real iGEM part as SBOL 2 from SynBioHub, runs
`sbol_convert::upgrade_from_sbol2`, validates, and prints a structural
summary.

```sh
cargo run -p sbol-convert --example synbiohub_upgrade
cargo run -p sbol-convert --example synbiohub_upgrade -- BBa_F2620
```

## Related

- [Conversion guide](conversion.md): user-facing reference for the
  conversion model itself (the backport namespace, structural
  collapses, Component classification, known divergences).
- [SBOL 3 → SBOL 2 downgrade conformance](sbol3-downgrade-conformance.md):
  the inverse direction, with the round-trip gate that pairs with
  the self-snapshot gate above.
- [SBOL-Converter differential conformance](sbol-converter-differential.md):
  the parity gate against the reference Java converter, both directions.
- [SBOL 3 round-trip smoke test report](sbol3-round-trip-report.md):
  per-fixture lossless-round-trip verification across the whole
  real corpus.
- [GenBank import conformance](genbank-import-conformance.md): the
  sibling harness for the GenBank → SBOL 3 path via `sbol-genbank`.
- [`sbol-owl3` conformance](ontology-conformance.md): analogous gate
  for the SBOL 3 vocabulary against the upstream OWL document.
- [Cross-implementation conformance](../tests/fixtures/cross-impl/README.md):
  the `libSBOLj3` round-trip gate for the SBOL 3 surface.
- [Validation system overview](validation.md): the post-upgrade
  spec-compliance gate. `sbol upgrade --validate` composes the two.
