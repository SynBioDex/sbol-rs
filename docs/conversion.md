# Converting between SBOL formats

Most published synthetic biology data lives in formats that predate SBOL 3.1.0:

- **SBOL 2**: what SynBioHub, the iGEM Registry, and JBEI ICE serve; what libSBOLj2 and pySBOL2 produce.
- **GenBank**: sequence-with-annotations files from SnapGene, ApE, Benchling, and NCBI. The lingua franca of plasmid and clone data.
- **FASTA**: bare sequences from NCBI, UniProt, BLAST hits, and alignment output.

`sbol-rs` brings all three into SBOL 3 so you can use the modern toolchain, and reverses the SBOL 3 path back to SBOL 2 when you need to publish to consumers that still speak the older format.

## What you can convert

| You have… | Run… | Get… |
|---|---|---|
| `*.xml` / `*.ttl` / `*.jsonld` from SynBioHub, iGEM Registry, JBEI ICE | `sbol upgrade` or [`sbol_convert::upgrade_from_sbol2`] | SBOL 3 RDF |
| `*.gb` / `*.gbk` from SnapGene, ApE, Benchling, NCBI | `sbol import-genbank` or [`sbol_genbank::GenbankImporter`] | SBOL 3 RDF |
| `*.fasta` / `*.fa` / `*.fna` / `*.faa` from NCBI, UniProt, BLAST | `sbol import-fasta` or [`sbol_fasta::FastaImporter`] | SBOL 3 RDF |
| SBOL 3 to publish to a tool that only consumes SBOL 2 | `sbol downgrade` or [`sbol_convert::downgrade`] | SBOL 2 RDF |

Everything runs in pure Rust: no Docker, no Python sidecar, no network calls.

## The reference: SynBioDex/SBOL-Converter

The SBOL 2 ↔ SBOL 3 conversion tracks the **observable behavior** of the
canonical Java converter, [SynBioDex/SBOL-Converter][ref]. Given the same
input, `sbol-rs` makes the same conversion decisions the reference does — the
same identities, the same class choices, the same vocabulary, the same backport
annotations — so files interoperate with the SynBioDex toolchain.

That parity is enforced in CI by a differential test against committed
reference outputs (no JVM required to run the suite); see
[`sbol-converter-differential.md`](sbol-converter-differential.md). The
round-trip and self-snapshot gates ([`sbol2-upgrade-conformance.md`](sbol2-upgrade-conformance.md),
[`sbol3-downgrade-conformance.md`](sbol3-downgrade-conformance.md)) back it up.

## Workflows

### Pulling a part down from SynBioHub

You found `BBa_F2620` in the iGEM Registry and want to inspect it, validate it, or build something on top. SynBioHub serves SBOL 2 by default:

```rust,no_run
use sbol::convert::upgrade_from_sbol2_path;
use sbol::v3::SbolIdentified;

let (document, report) = upgrade_from_sbol2_path("BBa_F2620.xml")?;
for warning in report.warnings() {
    eprintln!("{warning:?}");
}
document.check()?;

for component in document.components() {
    println!("{}", component.display_id().unwrap_or("(unnamed)"));
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

On the CLI:

```sh
sbol upgrade BBa_F2620.xml --output BBa_F2620.ttl --validate
```

The result is fully native SBOL 3: all the typed accessors (`document.components()`, `document.sequences()`, etc.) work, all spec rules apply, and the document round-trips through any of the supported RDF serializations.

### Upgrading a document already in memory

When an SBOL 2 document is already parsed, upgrade it directly. The
conversion reads the document's graph, so nothing is serialized along the
way, and it produces the same SBOL 3 as the file-based functions:

```rust,no_run
let design = sbol::v2::Document::read_path("sbol2.rdf")?;
design.check()?;

let (upgraded, report) = sbol::convert::upgrade(&design)?;
for warning in report.warnings() {
    eprintln!("{warning:?}");
}
upgraded.check()?;
upgraded.write_path("sbol3.ttl", sbol::RdfFormat::Turtle)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`sbol::convert::upgrade_with` takes the same `UpgradeOptions` as the
other upgrade functions.

**A note on identities.** SBOL 2 and SBOL 3 place the version segment in
different spots. SBOL 2 puts it last (`…/BBa_F2620/1`); SBOL 3 puts it before
the displayId (`…/1/BBa_F2620`). The upgrade rewrites the IRI accordingly, and
the downgrade reverses it — the same `createSBOL3Uri` / `createSBOL2Uri`
algebra the reference uses. An unversioned reference (`…/BBa_F2620`) resolves to
the highest-version object that declares it as its `persistentIdentity`, again
matching the reference (`getLatestUri`).

### Importing a GenBank file from SnapGene

You designed a plasmid in SnapGene, exported it as `pBR322.gb`, and want to drive it through SBOL-aware tooling:

```rust,no_run
use sbol_genbank::GenbankImporter;
use sbol::v3::RdfFormat;

let importer = GenbankImporter::new("https://my-lab.example.org")?;
let (document, _report) = importer.read_path("pBR322.gb")?;
document.check()?;
document.write_path("pBR322.ttl", RdfFormat::Turtle)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

On the CLI:

```sh
sbol import-genbank pBR322.gb --namespace https://my-lab.example.org --output pBR322.ttl
```

Each GenBank feature becomes an SBOL 3 SequenceFeature; coordinates become Ranges or Cuts; the LOCUS sequence becomes a Sequence with the appropriate EDAM encoding; molecule type (DNA/RNA/Protein) becomes the Component's SBO type. The `--namespace` flag is required because GenBank carries no IRI concept: you supply the namespace under which the SBOL 3 identities will be rooted.

### Importing a FASTA file from NCBI

You have a FASTA dump from BLAST, NCBI, or UniProt:

```rust,no_run
use sbol_fasta::FastaImporter;

let importer = FastaImporter::new("https://my-lab.example.org")?;
let (document, _report) = importer.read_path("GFP_protein.fasta")?;
document.check()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

```sh
sbol import-fasta GFP_protein.fasta --namespace https://my-lab.example.org -o GFP.ttl
```

Each FASTA record becomes a Component plus Sequence. Alphabet auto-detects (DNA / RNA / Protein); pass `--alphabet protein` if a short peptide composed only of A/C/G/T letters would otherwise be misclassified as DNA.

### Publishing an SBOL 3 design to SynBioHub

You authored a design natively in SBOL 3 and want to push it to a SynBioHub instance that consumes SBOL 2:

```rust,no_run
use sbol::convert::downgrade;
use sbol::v3::{Document, RdfFormat};

let document = Document::read_path("design.ttl")?;
let (sbol2_graph, report) = downgrade(&document)?;
let sbol2_xml = sbol2_graph.write(RdfFormat::RdfXml)?;
std::fs::write("design.xml", sbol2_xml)?;

for warning in report.warnings() {
    eprintln!("{warning:?}");
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

```sh
sbol downgrade design.ttl --to rdfxml --output design.xml --validate
```

The `--validate` flag is unusual: there is no SBOL 2 schema validator in this workspace, so it instead **round-trips** the downgrade: running the SBOL 2 output back through `sbol upgrade` and validating the resulting SBOL 3. If the round-trip succeeds and the resulting document validates, the downgrade preserved enough structure for `sbol-rs` to recover a valid SBOL 3 graph. For strict SBOL 2 spec compliance, serve the file to libSBOLj2 or pySBOL2 externally.

### Round-tripping SBOL 2 ↔ SBOL 3

If your workflow is "pull SBOL 2 down, edit in SBOL 3 tools, push back to SBOL 2", the trip is near-lossless:

```sh
sbol upgrade BBa_F2620.xml --output BBa_F2620.ttl
# edit in SBOL 3 tools…
sbol downgrade BBa_F2620.ttl --to rdfxml --output BBa_F2620_updated.xml
```

This works because the upgrade records the SBOL 2 detail that has no SBOL 3
home under [the backport namespace](#the-backport-namespace), and the downgrade
reads it back to rebuild the SBOL 2 shape. The `corpus_round_trip` test drives
the SBOL 2 → 3 → 2 fixed-point over the **full** SBOLTestSuite SBOL 2 corpora
and asserts triple-for-triple equality: **290 fixtures round-trip clean**
(0 drift, 0 parse failures, 0 upgrade-unsupported), with a single
documented-lossy fixture allowlisted. The [empirical per-fixture report](sbol3-round-trip-report.md)
covers the committed SynBioHub, iGEM, and SBOLTestSuite real-world fixtures.

## Component classification: one SBOL 3 Component, one SBOL 2 class

SBOL 2 split design into two top-level classes:

- **`ComponentDefinition`**, the structural half: types (DNA/RNA/protein), roles (promoter, CDS), sequence, sub-parts, sequence annotations, constraints.
- **`ModuleDefinition`**, the functional half: interactions, functional components, models, interface.

SBOL 3 unified both into a single `Component`. Going the other way, each SBOL 3
Component downgrades to **exactly one** SBOL 2 class — never both. The choice
follows the reference's `isModuleDefinition` predicate:

> A Component becomes a **ModuleDefinition** when it has interactions, **or**
> carries the `SBO:0000241` (functional entity) type, **or** (recursively)
> contains a sub-component whose `instanceOf` target is itself a
> ModuleDefinition. Otherwise it becomes a **ComponentDefinition**.

A sub-component of a ModuleDefinition-shaped parent becomes a `Module` when its
target is another ModuleDefinition (the upgrade marks this with
`backport:sbol2OriginatesFromModule`), otherwise a `FunctionalComponent`.

This mirrors the reference exactly, so an SBOL 2 document that mixed
ComponentDefinitions and ModuleDefinitions round-trips each object back to the
class it started in.

## The backport namespace

`https://sbols.org/backport/2_3#` (prefix `backport2_3`) is the namespace the
reference converter uses to record SBOL 2 detail that has no SBOL 3 home. It is
written **only** during `sbol upgrade` (SBOL 2 → SBOL 3); a downgrade emits no
backport annotations. After an upgrade your SBOL 3 document may contain triples
like:

```turtle
<https://example.org/lab/1/BBa_F2620>
    a sbol3:Component ;
    sbol3:displayId "BBa_F2620" ;
    backport2_3:sbol2OriginalURI <https://example.org/lab/BBa_F2620/1> .
```

These triples are **not** part of SBOL 3: they are round-trip metadata that
downstream SBOL 3 consumers can safely ignore, and the validator does. The
namespace defines eight terms; `sbol-rs` writes five on the way up and reads
four back on the way down (identical to the reference):

| Backport term | Written on upgrade | Read on downgrade | What it records |
|---|:---:|:---:|---|
| `sbol2OriginalURI` | ✓ | | The object's original SBOL 2 IRI, on every converted entity, as provenance for external tooling |
| `sbol2OriginalSequenceAnnotationURI` | ✓ | ✓ | The identity of an SBOL 2 SequenceAnnotation [collapsed onto a SubComponent](#reversible-structural-collapses), so the downgrade restores its displayId |
| `sbol3TempSequenceURI` | ✓ | ✓ | An empty Sequence synthesized so every SBOL 3 Location has one; dropped on downgrade |
| `sbol2LocationSequenceNull` | ✓ | ✓ | Marks a Location whose SBOL 2 source carried no sequence, so the downgrade doesn't re-emit one |
| `sbol2OriginatesFromModule` | ✓ | ✓ | Marks a SubComponent derived from an SBOL 2 `Module`, so it downgrades to `Module` rather than `FunctionalComponent` |
| `sbol2GenericLocation` / `sbol2Entity` / `sbol2MapstoOriginInFC` | | | Defined by the reference vocabulary; not yet exercised by `sbol-rs` |

`sbol2OriginalURI` is provenance the reference emits but never consumes:
round-trip identity fidelity comes from the version-in-IRI algebra, not from
reading this term back.

## Reversible structural collapses

SBOL 3 made several SBOL 2 idioms more compact. The upgrade folds them down; the downgrade re-emits the SBOL 2 shape.

**SequenceAnnotation that references a component.** SBOL 2 expresses "this sub-part lives at positions 100–200" as a SequenceAnnotation wrapping a Component reference and a Location. SBOL 3 collapses this: the SubComponent itself carries the locations directly. On upgrade, the SA's locations migrate to the SubComponent and the SA's identity is recorded as `backport:sbol2OriginalSequenceAnnotationURI`. On downgrade, the SequenceAnnotation is rebuilt — its displayId restored from that hint when present, otherwise synthesized as `{subComponent}_{firstLocation}` (matching the reference).

**MapsTo.** SBOL 2's `MapsTo` (mapping a sub-component's port to a parent context) decomposes in SBOL 3 into a `ComponentReference` (carrying `inChildOf` + `refersTo`) plus a `Constraint` (carrying `subject` + `object` + `restriction`). On downgrade, the pair is matched and the original `<sbol2:MapsTo>` is rebuilt under the carrier SubComponent. The refinement (`useLocal` / `useRemote` / `verifyIdentical`) is recovered from the Constraint's restriction and the ComponentReference's subject/object position per SBOL 3.1.0 §10.2.

**Interface synthesis.** SBOL 2 expresses port visibility and direction via `sbol2:access` and, for FunctionalComponents, `sbol2:direction` (`in`/`out`/`inout`/`none`). SBOL 3 hoists these into a single `sbol3:Interface` object (`input` / `output` / `nondirectional`). On downgrade the Interface flattens back: `input` → `in`, `output` → `out`, `nondirectional` → `inout`, each with public access, matching the reference.

**Empty sequences.** SBOL 3 requires every Location to reference a Sequence, which SBOL 2 does not. When a SBOL 2 ComponentDefinition has a location but no sequence, the upgrade synthesizes an empty one (marked `backport:sbol3TempSequenceURI`) and flags the location `backport:sbol2LocationSequenceNull`; the downgrade drops both.

## Known intentional divergences

Some triples don't survive SBOL 2 → 3 → 2 byte-for-byte. These are documented because the conversion is *correct*: they're irreversible folds the reference makes too, not bugs.

- **`biopax:Dna` vs `biopax:DnaRegion`**: both map to the same SBO term on upgrade, and the downgrade emits the `*Region` form. The `Dna`/`DnaRegion` (and `Rna`/`RnaRegion`) distinction is not preserved — the reference collapses it identically.
- **Ontology-term spelling**: SO / SBO / EDAM term IRIs are rewritten between the SBOL 2 and SBOL 3 conventions (e.g. `http://identifiers.org/so/SO:` ⇄ `https://identifiers.org/SO:`). The value is semantically identical; only the spelling changes.
- **`dcterms:title` ↔ `sbol3:name`** (and `dcterms:description` ↔ `sbol3:description`): the upgrade converts Dublin Core metadata to the SBOL 3 form; the downgrade converts it back.
- **`SBO:0000241` (functional entity) on ModuleDefinition-derived Components**: the upgrade adds this so SBOL 3's type cardinality is satisfied (every Component needs ≥1 type). The downgrade drops it when emitting back to `ModuleDefinition`.
- **Feature orientation and FunctionalComponent locations**: SBOL 2 has no place for a feature's own orientation or a FunctionalComponent's locations, so the `3 → 2` step drops them (the reference does too). A subsequent re-upgrade cannot restore what the SBOL 2 model does not hold.
- **`sbol3:hasNamespace`**: dropped on downgrade. SBOL 2 has no explicit namespace property; the namespace is implicit in the persistentIdentity.

## Known limitations

- **Generic locations and FC-owned MapsTo provenance.** The reference defines
  `sbol2GenericLocation`, `sbol2Entity`, and `sbol2MapstoOriginInFC` for a few
  edge cases (a Location with no coordinates, a MapsTo owned by a
  FunctionalComponent). `sbol-rs` does not yet emit these; no fixture in the
  differential corpus exercises them.
- **No native SBOL 2 schema validator.** `sbol downgrade --validate` round-trips the output back through SBOL 3 and validates *that*: a proxy for SBOL 2 structural correctness, not an SBOL 2 spec check.

## Where to look next

- **API reference**: the conversion functions live in [`sbol-convert`](https://docs.rs/sbol-convert) (re-exported as `sbol::convert`); GenBank and FASTA importers under [`sbol_genbank`](https://docs.rs/sbol-genbank) and [`sbol_fasta`](https://docs.rs/sbol-fasta).
- **Parity with the reference converter**: [`sbol-converter-differential.md`](sbol-converter-differential.md).
- **Per-fixture round-trip results**: [`sbol3-round-trip-report.md`](sbol3-round-trip-report.md), regenerated by `cargo run -p sbol-convert --bin generate-round-trip-report` against the committed SBOL 2 fixture corpus.
- **Conformance gates and how the test harness works**: [`sbol2-upgrade-conformance.md`](sbol2-upgrade-conformance.md), [`sbol3-downgrade-conformance.md`](sbol3-downgrade-conformance.md).
- **Import paths in depth**: [`genbank-import-conformance.md`](genbank-import-conformance.md), [`fasta-import-conformance.md`](fasta-import-conformance.md).

[ref]: https://github.com/SynBioDex/SBOL-Converter
[`sbol_convert::upgrade_from_sbol2`]: https://docs.rs/sbol-convert/latest/sbol_convert/fn.upgrade_from_sbol2.html
[`sbol_convert::downgrade`]: https://docs.rs/sbol-convert/latest/sbol_convert/fn.downgrade.html
[`sbol_genbank::GenbankImporter`]: https://docs.rs/sbol-genbank/latest/sbol_genbank/struct.GenbankImporter.html
[`sbol_fasta::FastaImporter`]: https://docs.rs/sbol-fasta/latest/sbol_fasta/struct.FastaImporter.html
