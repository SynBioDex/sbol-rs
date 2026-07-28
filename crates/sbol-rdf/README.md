# sbol-rdf

Low-level RDF support for the [`sbol`](https://crates.io/crates/sbol) crate.

`sbol-rdf` is the RDF boundary used by `sbol`: a backend-opaque `Graph` type
plus owned RDF terms (`Iri`, `BlankNode`, `Literal`, `Resource`, `Term`,
`Triple`) and an `RdfFormat` enum covering all four spec-listed SBOL 3
serializations.

```rust
use sbol_rdf::{Graph, RdfFormat};

let graph = Graph::parse(turtle_input, RdfFormat::Turtle)?;
let xml = graph.write(RdfFormat::RdfXml)?;
# Ok::<(), sbol_rdf::ParseError>(())
```

## Supported formats

| Format    | Variant                  | Extension |
| --------- | ------------------------ | --------- |
| Turtle    | `RdfFormat::Turtle`      | `.ttl`    |
| RDF/XML   | `RdfFormat::RdfXml`      | `.rdf`    |
| JSON-LD   | `RdfFormat::JsonLd`      | `.jsonld` |
| N-Triples | `RdfFormat::NTriples`    | `.nt`     |

Parsing is delegated to [`oxrdfio`](https://crates.io/crates/oxrdfio); this
crate adds owned RDF primitives, error types with source chains, and a
deterministic deduplicated triple-set view for equality checks
(`RdfGraph::normalized_triples`).

## When to use this crate directly

Most users want the [`sbol`](https://crates.io/crates/sbol) crate, which
exposes typed SBOL documents on top of this layer. Reach for `sbol-rdf`
directly only when:

- You need RDF I/O without the SBOL data model (e.g. an SBOL-adjacent tool
  that just wants Turtle/RDF/XML/JSON-LD parsing with a stable Rust surface).
- You're implementing an alternative SBOL workflow against a different
  domain model.

## License

Dual-licensed under MIT or Apache-2.0, at your option. See the
[workspace README](https://github.com/SynBioDex/sbol-rs#readme) for project
context and the broader SBOL implementation status.
