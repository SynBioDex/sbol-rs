//! Idiomatic Python bindings for sbol-rs.
//!
//! The Python surface is built on the ergonomic `Design` arena and its
//! biology-first verbs. It is deliberately *not* a drop-in for pySBOL3: rather
//! than a mutable object graph, it exposes the fluent staging model of the Rust
//! core, collapsing the Rust builder chains into single keyword-argument calls
//! that return opaque handles. A design is composed with verb calls and lowered
//! to an immutable [`Document`] with `finish()`.
//!
//! ```python
//! from sbol import Design, RdfFormat
//!
//! d = Design("https://example.org/lab")
//! plac = d.promoter("pLac", "caatacg", description="LacI-repressible")
//! tetr = d.cds("tetR", "atggtg")
//! d.engineered_region("pLac_tu", [plac, tetr])
//! doc = d.finish()
//! doc.check()
//! print(doc.to_string(RdfFormat.NTriples))
//! ```

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

use sbol_convert::{upgrade_from_sbol2, upgrade_from_sbol2_path};
use sbol_fasta::{FastaExporter, FastaImporter};
use sbol_genbank::{GenbankExporter, GenbankImporter};
use sbol_utilities::prelude::*;
use sbol2::Document as CoreSbol2Document;
use sbol3::RdfFormat as CoreFormat;
use sbol3::design::Design as Arena;
use sbol3::{Iri, Resource};

create_exception!(
    sbol,
    SbolError,
    PyException,
    "Raised when composing, lowering, serializing, or validating a design fails."
);

/// Opaque handle to a top-level `Component` in a [`Design`].
#[pyclass(frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
struct ComponentId {
    inner: sbol3::design::ComponentId,
}

#[pymethods]
impl ComponentId {
    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

/// Opaque handle to a `Sequence` in a [`Design`].
#[pyclass(frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
struct SequenceId {
    inner: sbol3::design::SequenceId,
}

#[pymethods]
impl SequenceId {
    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

/// RDF serialization format.
#[pyclass(eq, eq_int)]
#[derive(Clone, Copy, PartialEq)]
enum RdfFormat {
    Turtle,
    RdfXml,
    JsonLd,
    NTriples,
}

impl RdfFormat {
    fn core(self) -> CoreFormat {
        match self {
            RdfFormat::Turtle => CoreFormat::Turtle,
            RdfFormat::RdfXml => CoreFormat::RdfXml,
            RdfFormat::JsonLd => CoreFormat::JsonLd,
            RdfFormat::NTriples => CoreFormat::NTriples,
        }
    }
}

/// A mutable-feeling arena for composing a design, lowered to a [`Document`]
/// with [`finish`](Design::finish). `finish` consumes the arena; calling any
/// method afterward raises.
#[pyclass]
struct Design {
    inner: Option<Arena>,
}

impl Design {
    fn arena(&mut self) -> PyResult<&mut Arena> {
        self.inner
            .as_mut()
            .ok_or_else(|| SbolError::new_err("design has already been finished"))
    }
}

/// Applies the shared `name`/`description` keyword options to a part draft.
macro_rules! part_verb {
    ($self:ident, $verb:ident, $display_id:ident, $elements:ident, $name:ident, $description:ident) => {{
        let arena = $self.arena()?;
        let mut draft = arena.$verb($display_id, $elements);
        if let Some(name) = $name {
            draft = draft.name(name);
        }
        if let Some(description) = $description {
            draft = draft.description(description);
        }
        Ok(ComponentId { inner: draft.add() })
    }};
}

#[pymethods]
impl Design {
    #[new]
    fn new(namespace: &str) -> PyResult<Self> {
        Arena::new(namespace)
            .map(|arena| Design { inner: Some(arena) })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Loads an existing document into a fresh arena for inspection or
    /// extension. Objects keep their identities; the namespace is inferred from
    /// the document's top-level objects.
    #[staticmethod]
    fn from_document(document: &Document) -> PyResult<Self> {
        Arena::from_document(&document.inner)
            .map(|arena| Design { inner: Some(arena) })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Returns a handle to a component by its identity IRI so children can be
    /// added under an imported component; `None` if there is no such component.
    fn component_id(&self, identity: &str) -> PyResult<Option<ComponentId>> {
        let arena = self
            .inner
            .as_ref()
            .ok_or_else(|| SbolError::new_err("design has already been finished"))?;
        Ok(arena
            .component_id(&Resource::iri(identity))
            .map(|inner| ComponentId { inner }))
    }

    /// Registers a standalone `Sequence`. `kind` is `"dna"`, `"rna"`, or
    /// `"protein"` (setting the IUPAC encoding), or `None` for no encoding.
    #[pyo3(signature = (display_id, elements, *, kind="dna"))]
    fn sequence(
        &mut self,
        display_id: &str,
        elements: &str,
        kind: Option<&str>,
    ) -> PyResult<SequenceId> {
        let arena = self.arena()?;
        let draft = arena.sequence(display_id).elements(elements);
        let draft = match kind {
            None => draft,
            Some("dna") => draft.dna(),
            Some("rna") => draft.rna(),
            Some("protein") => draft.protein(),
            Some(other) => {
                return Err(SbolError::new_err(format!(
                    "unknown sequence kind `{other}`; expected \"dna\", \"rna\", or \"protein\""
                )));
            }
        };
        Ok(SequenceId { inner: draft.add() })
    }

    /// Registers a `Component` with explicit type/role IRIs and optional
    /// sequences — the low-level escape hatch beyond the named part verbs.
    #[pyo3(signature = (display_id, *, types=Vec::new(), roles=Vec::new(), sequences=Vec::new(), name=None, description=None))]
    fn component(
        &mut self,
        display_id: &str,
        types: Vec<String>,
        roles: Vec<String>,
        sequences: Vec<SequenceId>,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        let arena = self.arena()?;
        let mut draft = arena.component(display_id);
        for type_iri in types {
            let iri = Iri::new(type_iri).map_err(|err| SbolError::new_err(err.to_string()))?;
            draft = draft.type_(iri);
        }
        for role_iri in roles {
            let iri = Iri::new(role_iri).map_err(|err| SbolError::new_err(err.to_string()))?;
            draft = draft.role(iri);
        }
        for sequence in sequences {
            draft = draft.sequence(sequence.inner);
        }
        if let Some(name) = name {
            draft = draft.name(name);
        }
        if let Some(description) = description {
            draft = draft.description(description);
        }
        Ok(ComponentId { inner: draft.add() })
    }

    /// A promoter (`SO:0000167`) DNA part with its sequence.
    #[pyo3(signature = (display_id, elements, *, name=None, description=None))]
    fn promoter(
        &mut self,
        display_id: &str,
        elements: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        part_verb!(self, promoter, display_id, elements, name, description)
    }

    /// A ribosome entry site (`SO:0000139`) DNA part with its sequence.
    #[pyo3(signature = (display_id, elements, *, name=None, description=None))]
    fn rbs(
        &mut self,
        display_id: &str,
        elements: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        part_verb!(self, rbs, display_id, elements, name, description)
    }

    /// A coding sequence (`SO:0000316`) DNA part with its sequence.
    #[pyo3(signature = (display_id, elements, *, name=None, description=None))]
    fn cds(
        &mut self,
        display_id: &str,
        elements: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        part_verb!(self, cds, display_id, elements, name, description)
    }

    /// A terminator (`SO:0000141`) DNA part with its sequence.
    #[pyo3(signature = (display_id, elements, *, name=None, description=None))]
    fn terminator(
        &mut self,
        display_id: &str,
        elements: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        part_verb!(self, terminator, display_id, elements, name, description)
    }

    /// A gene (`SO:0000704`) DNA part with its sequence.
    #[pyo3(signature = (display_id, elements, *, name=None, description=None))]
    fn gene(
        &mut self,
        display_id: &str,
        elements: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        part_verb!(self, gene, display_id, elements, name, description)
    }

    /// An operator (`SO:0000057`) DNA part with its sequence.
    #[pyo3(signature = (display_id, elements, *, name=None, description=None))]
    fn operator(
        &mut self,
        display_id: &str,
        elements: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        part_verb!(self, operator, display_id, elements, name, description)
    }

    /// An mRNA (`SO:0000234`) RNA part with its RNA sequence.
    #[pyo3(signature = (display_id, elements, *, name=None, description=None))]
    fn mrna(
        &mut self,
        display_id: &str,
        elements: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        part_verb!(self, mrna, display_id, elements, name, description)
    }

    /// A transcription factor (`SO:0003700`) protein part with its sequence.
    #[pyo3(signature = (display_id, elements, *, name=None, description=None))]
    fn transcription_factor(
        &mut self,
        display_id: &str,
        elements: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        part_verb!(
            self,
            transcription_factor,
            display_id,
            elements,
            name,
            description
        )
    }

    /// A functional-entity `Component` (`SBO:0000241`) with no sequence.
    #[pyo3(signature = (display_id, *, name=None, description=None))]
    fn functional_component(
        &mut self,
        display_id: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        let arena = self.arena()?;
        let mut draft = arena.functional_component(display_id);
        if let Some(name) = name {
            draft = draft.name(name);
        }
        if let Some(description) = description {
            draft = draft.description(description);
        }
        Ok(ComponentId { inner: draft.add() })
    }

    /// An engineered region whose `parts` are chained head-to-tail with `meets`
    /// constraints, each part's roles copied onto its sub-component.
    #[pyo3(signature = (display_id, parts, *, name=None, description=None))]
    fn engineered_region(
        &mut self,
        display_id: &str,
        parts: Vec<ComponentId>,
        name: Option<String>,
        description: Option<String>,
    ) -> PyResult<ComponentId> {
        let arena = self.arena()?;
        let handles: Vec<sbol3::design::ComponentId> =
            parts.into_iter().map(|part| part.inner).collect();
        let mut draft = arena.engineered_region(display_id, handles);
        if let Some(name) = name {
            draft = draft.name(name);
        }
        if let Some(description) = description {
            draft = draft.description(description);
        }
        Ok(ComponentId { inner: draft.add() })
    }

    /// Lowers the design to an immutable [`Document`]. Consumes the arena;
    /// raises [`SbolError`] if the design recorded any problems.
    fn finish(&mut self) -> PyResult<Document> {
        let arena = self
            .inner
            .take()
            .ok_or_else(|| SbolError::new_err("design has already been finished"))?;
        arena
            .finish()
            .map(|document| Document { inner: document })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }
}

/// An immutable SBOL document produced by [`Design::finish`] or read from RDF.
#[pyclass]
struct Document {
    inner: sbol3::Document,
}

#[pymethods]
impl Document {
    /// Serializes the document to an RDF string in the given format.
    fn to_string(&self, format: RdfFormat) -> PyResult<String> {
        self.inner
            .write(format.core())
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Writes the document to a file in the given format.
    fn write_path(&self, path: &str, format: RdfFormat) -> PyResult<()> {
        self.inner
            .write_path(path, format.core())
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Validates the document, raising [`SbolError`] with the report on any
    /// validation error.
    fn check(&self) -> PyResult<()> {
        match self.inner.check() {
            Ok(_) => Ok(()),
            Err(report) => Err(SbolError::new_err(format!("{report:?}"))),
        }
    }

    /// The number of top-level `Component`s.
    fn component_count(&self) -> usize {
        self.inner.components().count()
    }

    /// The number of `Sequence`s.
    fn sequence_count(&self) -> usize {
        self.inner.sequences().count()
    }

    /// The display IDs of the top-level `Component`s, in document order.
    fn component_display_ids(&self) -> Vec<String> {
        use sbol3::SbolIdentified;
        self.inner
            .components()
            .filter_map(|component| component.display_id().map(str::to_string))
            .collect()
    }

    /// Parses a document from an RDF string in the given format.
    #[staticmethod]
    fn read_str(input: &str, format: RdfFormat) -> PyResult<Document> {
        sbol3::Document::read(input, format.core())
            .map(|document| Document { inner: document })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Parses a document from an RDF file; the format is inferred from the
    /// extension (`.ttl`, `.rdf`, `.jsonld`, `.nt`).
    #[staticmethod]
    fn read_path(path: &str) -> PyResult<Document> {
        sbol3::Document::read_path(path)
            .map(|document| Document { inner: document })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Computes sequences for every engineered region that lacks one, returning
    /// a new document with the computed `Sequence`s and per-feature `Range`s.
    fn compute_sequences(&self) -> PyResult<Document> {
        sbol_utilities::compute_all_sequences(&self.inner)
            .map(|inner| Document { inner })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Computes the sequence of a single region, named by its identity IRI.
    fn compute_sequence(&self, region_identity: &str) -> PyResult<Document> {
        sbol_utilities::compute_sequence(&self.inner, &Resource::iri(region_identity))
            .map(|inner| Document { inner })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Expands every combinatorial derivation into variant components, returning
    /// a new document with the derived components and their collections.
    fn expand_derivations(&self) -> PyResult<Document> {
        sbol_utilities::expand_derivations(&self.inner)
            .map(|inner| Document { inner })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Serializes the document's sequences to FASTA.
    fn to_fasta(&self) -> String {
        FastaExporter::new().to_string(&self.inner)
    }

    /// Writes the document's sequences to a FASTA file.
    fn write_fasta(&self, path: &str) -> PyResult<()> {
        FastaExporter::new()
            .write_path(&self.inner, path)
            .map(|_| ())
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Serializes the document to GenBank flat-file text.
    fn to_genbank(&self) -> PyResult<String> {
        GenbankExporter::new()
            .to_string(&self.inner)
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Writes the document to a GenBank file.
    fn write_genbank(&self, path: &str) -> PyResult<()> {
        GenbankExporter::new()
            .write_path(&self.inner, path)
            .map(|_| ())
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Downgrades this SBOL 3 document to SBOL 2 RDF text in the given format.
    fn to_sbol2(&self, format: RdfFormat) -> PyResult<String> {
        let (graph, _report) = sbol_convert::downgrade(&self.inner)
            .map_err(|err| SbolError::new_err(err.to_string()))?;
        graph
            .write(format.core())
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Downgrades this SBOL 3 document to an SBOL 2 [`Sbol2Document`].
    fn downgrade(&self) -> PyResult<Sbol2Document> {
        let (graph, _report) = sbol_convert::downgrade(&self.inner)
            .map_err(|err| SbolError::new_err(err.to_string()))?;
        let ntriples = graph
            .write(CoreFormat::NTriples)
            .map_err(|err| SbolError::new_err(err.to_string()))?;
        CoreSbol2Document::read(&ntriples, CoreFormat::NTriples)
            .map(|inner| Sbol2Document { inner })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }
}

/// An immutable SBOL 2 document. SBOL 2 is supported alongside SBOL 3 so a
/// single package can read, write, validate, and convert both versions.
#[pyclass]
struct Sbol2Document {
    inner: CoreSbol2Document,
}

#[pymethods]
impl Sbol2Document {
    /// Parses an SBOL 2 document from an RDF string in the given format.
    #[staticmethod]
    fn read_str(input: &str, format: RdfFormat) -> PyResult<Sbol2Document> {
        CoreSbol2Document::read(input, format.core())
            .map(|inner| Sbol2Document { inner })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Parses an SBOL 2 document from an RDF file (format inferred from the
    /// extension).
    #[staticmethod]
    fn read_path(path: &str) -> PyResult<Sbol2Document> {
        CoreSbol2Document::read_path(path)
            .map(|inner| Sbol2Document { inner })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Serializes the document to an RDF string in the given format.
    fn to_string(&self, format: RdfFormat) -> PyResult<String> {
        self.inner
            .write(format.core())
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Writes the document to an RDF file in the given format.
    fn write_path(&self, path: &str, format: RdfFormat) -> PyResult<()> {
        self.inner
            .write_path(path, format.core())
            .map_err(|err| SbolError::new_err(err.to_string()))
    }

    /// Validates the document, raising [`SbolError`] with the report on any
    /// validation error.
    fn check(&self) -> PyResult<()> {
        match self.inner.check() {
            Ok(_) => Ok(()),
            Err(report) => Err(SbolError::new_err(format!("{report:?}"))),
        }
    }

    /// Upgrades this SBOL 2 document to an SBOL 3 [`Document`].
    fn to_sbol3(&self) -> PyResult<Document> {
        sbol_convert::upgrade(&self.inner)
            .map(|(inner, _report)| Document { inner })
            .map_err(|err| SbolError::new_err(err.to_string()))
    }
}

/// Imports FASTA text into a new document under `namespace`.
#[pyfunction]
fn read_fasta(text: &str, namespace: &str) -> PyResult<Document> {
    FastaImporter::new(namespace)
        .and_then(|importer| importer.read_str(text))
        .map(|(inner, _report)| Document { inner })
        .map_err(|err| SbolError::new_err(err.to_string()))
}

/// Imports a FASTA file into a new document under `namespace`.
#[pyfunction]
fn read_fasta_path(path: &str, namespace: &str) -> PyResult<Document> {
    FastaImporter::new(namespace)
        .and_then(|importer| importer.read_path(path))
        .map(|(inner, _report)| Document { inner })
        .map_err(|err| SbolError::new_err(err.to_string()))
}

/// Imports GenBank flat-file text into a new document under `namespace`.
#[pyfunction]
fn read_genbank(text: &str, namespace: &str) -> PyResult<Document> {
    GenbankImporter::new(namespace)
        .and_then(|importer| importer.read_str(text))
        .map(|(inner, _report)| Document { inner })
        .map_err(|err| SbolError::new_err(err.to_string()))
}

/// Imports a GenBank file into a new document under `namespace`.
#[pyfunction]
fn read_genbank_path(path: &str, namespace: &str) -> PyResult<Document> {
    GenbankImporter::new(namespace)
        .and_then(|importer| importer.read_path(path))
        .map(|(inner, _report)| Document { inner })
        .map_err(|err| SbolError::new_err(err.to_string()))
}

/// Upgrades SBOL 2 RDF text to an SBOL 3 [`Document`].
#[pyfunction]
fn upgrade_sbol2(text: &str, format: RdfFormat) -> PyResult<Document> {
    upgrade_from_sbol2(text, format.core())
        .map(|(inner, _report)| Document { inner })
        .map_err(|err| SbolError::new_err(err.to_string()))
}

/// Upgrades an SBOL 2 RDF file to an SBOL 3 [`Document`] (format inferred from
/// the extension).
#[pyfunction]
fn upgrade_sbol2_path(path: &str) -> PyResult<Document> {
    upgrade_from_sbol2_path(path)
        .map(|(inner, _report)| Document { inner })
        .map_err(|err| SbolError::new_err(err.to_string()))
}

/// Registers the curated ontology-term constants on the module.
fn register_constants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    use sbol3::constants as c;
    let terms: &[(&str, &Iri)] = &[
        ("SO_PROMOTER", &c::SO_PROMOTER),
        ("SO_RBS", &c::SO_RBS),
        ("SO_CDS", &c::SO_CDS),
        ("SO_TERMINATOR", &c::SO_TERMINATOR),
        ("SO_GENE", &c::SO_GENE),
        ("SO_OPERATOR", &c::SO_OPERATOR),
        ("SO_ENGINEERED_REGION", &c::SO_ENGINEERED_REGION),
        ("SO_MRNA", &c::SO_MRNA),
        ("SBO_DNA", &c::SBO_DNA),
        ("SBO_RNA", &c::SBO_RNA),
        ("SBO_PROTEIN", &c::SBO_PROTEIN),
        ("SBO_FUNCTIONAL_ENTITY", &c::SBO_FUNCTIONAL_ENTITY),
        ("EDAM_IUPAC_DNA", &c::EDAM_IUPAC_DNA),
        ("EDAM_IUPAC_RNA", &c::EDAM_IUPAC_RNA),
        ("EDAM_IUPAC_PROTEIN", &c::EDAM_IUPAC_PROTEIN),
    ];
    for (name, iri) in terms {
        module.add(*name, iri.as_str())?;
    }
    Ok(())
}

#[pymodule]
fn sbol(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Design>()?;
    module.add_class::<Document>()?;
    module.add_class::<Sbol2Document>()?;
    module.add_class::<ComponentId>()?;
    module.add_class::<SequenceId>()?;
    module.add_class::<RdfFormat>()?;
    module.add("SbolError", module.py().get_type::<SbolError>())?;
    module.add_function(wrap_pyfunction!(read_fasta, module)?)?;
    module.add_function(wrap_pyfunction!(read_fasta_path, module)?)?;
    module.add_function(wrap_pyfunction!(read_genbank, module)?)?;
    module.add_function(wrap_pyfunction!(read_genbank_path, module)?)?;
    module.add_function(wrap_pyfunction!(upgrade_sbol2, module)?)?;
    module.add_function(wrap_pyfunction!(upgrade_sbol2_path, module)?)?;
    register_constants(module)?;
    Ok(())
}
