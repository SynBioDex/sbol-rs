# sbol-ontology

Offline ontology facts for the [`sbol`](https://crates.io/crates/sbol) crate's
SBOL 3 validator.

`sbol-ontology` embeds a compact SBOL-specific snapshot of the ontology
branches the validator needs to make local classification decisions: EDAM
textual-format / SBO physical-entity / SO topology, strand, and
sequence-feature / GO molecular-function-role / CHEBI role / Cell Ontology
cell-type branches. The data ships as a TSV via `include_str!`, so there is no
network access at runtime.

The public API accepts document-facing IRIs such as
`https://identifiers.org/SBO:0000251`, OBO PURLs such as
`http://purl.obolibrary.org/obo/SBO_0000251`, and compact IDs such as
`SBO:0000251`.

## When to use this crate directly

Most users want [`sbol`](https://crates.io/crates/sbol), which consumes these
facts through the validator. Reach for `sbol-ontology` directly only when you
need to query the same fact table from a non-SBOL tool (e.g. a biological
design linter that wants the same branch-membership decisions).

NCIT is not bundled because it is too large to embed by default. Install
it into a local cache with `sbol ontology install ncit` (CLI) or
`OntologyCache::ensure_installed(KnownOntology::Ncit.descriptor())`
(Rust). See [`docs/ontology-extensions.md`](../../docs/ontology-extensions.md)
for the workflow.

## Refreshing the bundled snapshot (contributors only)

Raw upstream ontology files can be bootstrapped into a local cache with
Cargo:

```console
cargo run -p sbol-ontology --bin bootstrap-ontology-cache -- --cache target/ontology-cache
```

The cache is only an input for refreshing generated facts and is not required
for normal builds or tests.

After refreshing the raw cache, regenerate the compact validator facts with:

```console
cargo run -p sbol-ontology --bin generate-ontology-facts -- --cache target/ontology-cache
```

Use `--check` in CI-style workflows to verify that the checked-in fact snapshot
matches the current generator output.

## License

Dual-licensed under MIT or Apache-2.0, at your option. See the
[workspace README](https://github.com/SynBioDex/sbol-rs#readme) for project
context.
