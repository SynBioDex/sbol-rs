![sbol-rs: a Rust implementation of SBOL](docs/images/sbol-rs.png)

[![CI](https://github.com/SynBioDex/sbol-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/SynBioDex/sbol-rs/actions/workflows/ci.yml)
[![Documentation](https://docs.rs/sbol/badge.svg)](https://docs.rs/sbol)
[![Crates.io](https://img.shields.io/crates/v/sbol.svg)](https://crates.io/crates/sbol)
![Rust 1.93+](https://img.shields.io/badge/rust-1.93%2B-dea584?logo=rust)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-yellow.svg)](LICENSE-MIT)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.21687240.svg)](https://doi.org/10.5281/zenodo.21687240)

`sbol-rs` is a Rust implementation of the Synthetic Biology Open
Language (SBOL), covering both SBOL 2.3.0 and SBOL 3.1.0. SBOL
is the community standard for the exchange of synthetic biology designs
across registries, design-automation tools, and laboratory automation
pipelines. `sbol-rs` exposes a typed SDK and a CLI for both versions
(read, build, rewrite, validate, and losslessly convert SBOL 2 ⇄
SBOL 3), plus validators covering the 109 machine-checkable rules of
SBOL 3.1.0 Appendix B and the 239 machine-checkable rules of the
SBOL 2.3.0 catalog.

New to the codebase? Start with the [**crate guide**](docs/crate-guide.md).

## Installation

Add the library to your `Cargo.toml`:

```toml
[dependencies]
sbol = "1"
```

Or with `cargo add`:

```sh
cargo add sbol
```

The CLI ships as a separate crate. `cargo install sbol-cli` installs a
binary named `sbol`:

```sh
cargo install sbol-cli
sbol validate design.ttl
sbol registry login
sbol registry logout
sbol registry pull https://sbol.io/public/igem/BBa_J23100/1 -o design.ttl
```

`sbol registry login` uses browser-based Sign in with SBOL when the selected
registry advertises SBOL Identity, including PKCE and automatic token refresh.

`sbol-rs` 1.x follows Cargo's Semantic Versioning compatibility rules
for the public Rust APIs. The documented CLI exit codes and JSON v1
validation output schema are also stable throughout 1.x; incompatible
changes to those surfaces require a new major release.

## Example

```rust
use sbol::v3::constants::{EDAM_IUPAC_DNA, SBO_DNA, SO_PROMOTER};
use sbol::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let namespace = "https://example.org/lab";

    let sequence = Sequence::builder(namespace, "j23119_seq")?
        .elements("ttgacagctagctcagtcctaggtataatgctagc")
        .encoding(EDAM_IUPAC_DNA)
        .build()?;

    let component = Component::builder(namespace, "j23119")?
        .types([SBO_DNA])
        .add_component_role(SO_PROMOTER)
        .add_sequence(sequence.identity.clone())
        .name("J23119 constitutive promoter")
        .build()?;

    let document = Document::from_objects(vec![
        SbolObject::Component(component),
        SbolObject::Sequence(sequence),
    ])?;

    document.check()?;
    println!("{}", document.write_turtle()?);
    Ok(())
}
```

Reading documents, traversing references across documents, expanding
combinatorial derivations, and inspecting validation reports are covered
in [`crates/sbol3/examples/`](crates/sbol3/examples/). Run any of them
with `cargo run -p sbol3 --example <name>`.

## Comparing documents

`Document::diff` compares two documents by object identity rather than by
their serialized text, so serialization order, blank-node labeling, and
RDF format are ignored: two serializations of the same design diff clean,
and a real edit surfaces as the object whose properties moved.

```rust
use sbol::v3::constants::{SBO_DNA, SO_ENGINEERED_REGION, SO_PROMOTER};
use sbol::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let namespace = "https://example.org/lab";

    let old = Document::from_objects(vec![SbolObject::Component(
        Component::builder(namespace, "j23119")?
            .types([SBO_DNA])
            .add_component_role(SO_PROMOTER)
            .name("J23119")
            .build()?,
    )])?;

    // A revision that renames the promoter and adds a second role.
    let new = Document::from_objects(vec![SbolObject::Component(
        Component::builder(namespace, "j23119")?
            .types([SBO_DNA])
            .add_component_role(SO_PROMOTER)
            .add_component_role(SO_ENGINEERED_REGION)
            .name("J23119 constitutive promoter")
            .build()?,
    )])?;

    let diff = old.diff(&new);
    print!("{diff}");
    // ~ https://example.org/lab/j23119
    //     http://sbols.org/v3#name
    //         + "J23119 constitutive promoter"
    //         - "J23119"
    //     http://sbols.org/v3#role
    //         + https://identifiers.org/SO:0000804
    Ok(())
}
```

`diff` returns a structured value — added, removed, and changed objects,
with per-predicate term changes on each changed object — so it also drives
programmatic checks, not just the printed summary. See
[`crates/sbol3/examples/diff_documents.rs`](crates/sbol3/examples/diff_documents.rs).

The comparison is meaningful within one SBOL version. To compare a design
across versions, upgrade the SBOL 2 document to SBOL 3 first
(`sbol::convert::upgrade_from_sbol2`) and diff the two SBOL 3 documents;
the two versions otherwise use different identity, type, and predicate IRIs,
so a direct comparison reports every object as removed and re-added.

## Crates

`sbol-rs` is a Cargo workspace. Most users depend on the `sbol` umbrella;
the version crates are usable directly.

| Crate | Role |
|---|---|
| [`sbol`](crates/sbol/) | Complete umbrella facade. Re-exports SBOL 2 as `sbol::v2`, SBOL 3 as `sbol::v3`, and conversion as `sbol::convert`; adds version detection and a version-neutral document handle. |
| [`sbol3`](crates/sbol3/) | SBOL 3.1.0 typed data model, RDF I/O, and validator. |
| [`sbol2`](crates/sbol2/) | SBOL 2.3.0 typed data model, RDF I/O, and validator. |
| [`sbol-core`](crates/sbol-core/) | Version-neutral machinery both versions build on: field-metadata descriptors, identity newtypes, the RDF-backed document store, and the shared validation reporting and configuration types. |
| [`sbol-convert`](crates/sbol-convert/) | SBOL 2 ⇄ SBOL 3 conversion at the RDF triple level (`upgrade_from_sbol2`, `downgrade`). |
| [`sbol-cli`](crates/sbol-cli/) | The `sbol` command-line tool: validate, compare, convert, import, and authenticated registry workflows. |
| [`sbol-registry-client`](crates/sbol-registry-client/) | Typed SBOL DB discovery, download, login, and collision-aware submission transport used by the CLI. |
| [`sbol-rulegen`](crates/sbol-rulegen/) | Generates each version's validation rule catalog from its `rules.toml`. |
| [`sbol-fasta`](crates/sbol-fasta/) / [`sbol-genbank`](crates/sbol-genbank/) | FASTA and GenBank importers to native SBOL 3. |
| [`sbol-ontology`](crates/sbol-ontology/) / [`sbol-rdf`](crates/sbol-rdf/) | Bundled ontology snapshots and the RDF serialization layer. |

## Validation

`sbol-rs` validates both SBOL versions through one shared
`ValidationConfig` and one diagnostic surface (text / JSON / SARIF
output), with configurable scope (offline by default, or resolver-backed
for cross-document references) and per-rule severity overrides.

SBOL 3.1.0 [Appendix B](spec/SBOL3.1.0.md#b-validation-rules) defines
149 validation rules; 40 are marked as not-to-be-machine-reported, and
`sbol3` implements an algorithm for each of the remaining 109. The SBOL
2.3.0 catalog carries 268 rules, 239 of them machine-checkable, all
implemented by `sbol2`. [`docs/validation.md`](docs/validation.md)
covers what's checked and the trust boundaries;
[`docs/conformance.md`](docs/conformance.md) and
[`docs/sbol2-conformance.md`](docs/sbol2-conformance.md) carry the
per-rule status grids.

## Ontology Extensions

EDAM, SBO, SO, GO, ChEBI, and Cell Ontology ship bundled. NCIT and
lab-specific ontologies install on demand into a local cache; see
[`docs/ontology-extensions.md`](docs/ontology-extensions.md).

## Performance

The benchmark harness compares sbol-rs against the mainstream
implementation of each SBOL version. Parse cost (median microseconds across 100
measured iterations, 20 warmup; lower is better) with every
implementation in its own pinned Docker image so the rows are
apples-to-apples.

**SBOL 3**, `toggle_switch_v2.ttl` (~30 KB), rows sorted by `rdfxml`
p50 ascending:

| Impl              | turtle | rdfxml | jsonld | ntriples |
| ----------------- | -----: | -----: | -----: | -------: |
| sbol-rs           |    373 |    387 |    799 |      404 |
| libSBOLj3 1.0.5.2 |  1,908 |  2,176 |  4,437 |    2,062 |
| sboljs 3.0.2      |    n/a |  2,459 |    n/a |      n/a |
| pySBOL3 1.2       |  7,435 |  9,807 |  6,501 |    6,906 |

**SBOL 2**, `BBa_F2620` SynBioHub export (~79 KB). SBOL 2 is exchanged
as RDF/XML; sbol-rs also reads Turtle, JSON-LD, and N-Triples, and
libSBOLj covers RDF/XML:

| Impl           | turtle | rdfxml | jsonld | ntriples |
| -------------- | -----: | -----: | -----: | -------: |
| sbol-rs        |    955 |    988 |  2,027 |    1,029 |
| libSBOLj 2.4.0 |    n/a |  1,688 |    n/a |      n/a |

Apple M4 Max (12 performance and 4 efficiency cores), 128 GB RAM, macOS
26.6, Docker Desktop 29.4.3. sboljs's underlying `rdfoo` only emits
RDF/XML and its parser stack is too fragile to reach the other format
rows; the [bench README](benches/cross-impl/README.md) documents the
specific failure modes. The [`crates/sbol-bench`](crates/sbol-bench)
crate runs the cross-implementation comparison end-to-end; see
[`benches/cross-impl/README.md`](benches/cross-impl/README.md) for the
full SBOL 2 and SBOL 3 parse, serialize, convert, and validate tables,
methodology, and per-row caveats.
