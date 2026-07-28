# sbol

The unified [SBOL](https://sbolstandard.org/) toolkit for Python: **one package
for SBOL 2, SBOL 3, and lossless conversion between them**, backed by a Rust
core. No other Python library covers both versions — `pysbol2` and `pysbol3`
are separate installs with no bridge between them.

`sbol` is the Python binding for [sbol-rs](https://github.com/SynBioDex/sbol-rs).
Because the heavy lifting (RDF parsing, serialization, validation, conversion)
runs in Rust, it is **dramatically faster than pySBOL3** — 8–2300× on
parse/validate/serialize benchmarks (see `benchmarks/vs_pysbol3.py`).

## Install

Not yet on PyPI. Build from source (needs a Rust toolchain and
[maturin](https://www.maturin.rs/)):

```bash
pip install maturin
maturin develop -m crates/sbol-py/Cargo.toml   # into the active venv
```

Once published:

```bash
pip install sbol
```

## Quickstart — author an SBOL 3 design

The API is idiomatic Python (keyword arguments, opaque handles), not a clone of
pySBOL3's mutable graph. Build with verb calls, then `finish()` into an
immutable `Document`.

```python
from sbol import Design, RdfFormat

d = Design("https://example.org/lab")
plac  = d.promoter("pLac", "caatacg", description="LacI-repressible")
b0034 = d.rbs("B0034", "ttgaac")
tetr  = d.cds("tetR", "atggtg")
d.engineered_region("pLac_tu", [plac, b0034, tetr])

doc = d.finish()
doc.check()                                   # raises SbolError on validation failure
print(doc.to_string(RdfFormat.NTriples))
```

### Compute the assembled sequence

```python
doc = d.finish().compute_sequences()          # concatenates parts, adds Range locations
```

## SBOL 2, SBOL 3, and conversion — in one package

```python
from sbol import Design, Sbol2Document, RdfFormat, upgrade_sbol2

# SBOL 3 -> SBOL 2
doc3 = Design("https://example.org/lab")
doc3.promoter("pLac", "acgtacgt")
doc3 = doc3.finish()
sbol2_text = doc3.to_sbol2(RdfFormat.Turtle)   # or: doc3.downgrade() -> Sbol2Document

# SBOL 2 -> SBOL 3
doc2 = Sbol2Document.read_path("legacy_part.xml")
doc2.check()
doc3_again = doc2.to_sbol3()                   # or: upgrade_sbol2(text, RdfFormat.RdfXml)
```

## Sequence formats

```python
import sbol

doc = sbol.read_genbank_path("plasmid.gb", "https://example.org/lab")   # GenBank -> SBOL
doc = sbol.read_fasta_path("reads.fasta", "https://example.org/lab")    # FASTA -> SBOL

print(doc.to_genbank())      # SBOL -> GenBank
print(doc.to_fasta())        # SBOL -> FASTA
```

## API overview

| | Highlights |
|---|---|
| `Design(namespace)` | Verbs: `promoter`, `rbs`, `cds`, `terminator`, `gene`, `operator`, `mrna`, `transcription_factor`, `functional_component`; `engineered_region(id, parts)`; raw `component(...)` / `sequence(...)`; `finish()`. Import & extend an existing doc with `Design.from_document(doc)` + `component_id(iri)`. |
| `Document` | `to_string(fmt)` / `write_path`, `read_str` / `read_path`, `check()`, `component_count()` / `sequence_count()` / `component_display_ids()`, `compute_sequences()` / `compute_sequence(iri)`, `expand_derivations()`, `to_fasta()` / `to_genbank()`, `to_sbol2(fmt)` / `downgrade()`. |
| `Sbol2Document` | `read_str` / `read_path`, `to_string(fmt)` / `write_path`, `check()`, `to_sbol3()`. |
| Module functions | `read_fasta` / `read_fasta_path`, `read_genbank` / `read_genbank_path`, `upgrade_sbol2` / `upgrade_sbol2_path`. |
| `RdfFormat` | `Turtle`, `RdfXml`, `JsonLd`, `NTriples`. |
| Constants | `SO_PROMOTER`, `SO_CDS`, …, `SBO_DNA`, …, `EDAM_IUPAC_DNA`, … (ontology term IRIs for the raw `component()` path). |
| `SbolError` | Raised on construction, validation, I/O, or conversion failure. |

## Relationship to sbol-rs

`sbol` wraps the `sbol-rs` Rust crates: `sbol3` (SBOL 3 model, `Design` arena,
validation), `sbol2` (SBOL 2 model), `sbol-convert` (SBOL 2⇄3), `sbol-utilities`
(biology verbs, `compute_sequence`, combinatorial expansion), and
`sbol-genbank` / `sbol-fasta`. The Rust API itself is documented via rustdoc
(`cargo doc --open`).

## License

MIT OR Apache-2.0.
