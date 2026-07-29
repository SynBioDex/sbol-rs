//! Umbrella facade for the sbol-rs ecosystem.
//!
//! This crate re-exports the SBOL 2 implementation as [`v2`], the SBOL 3
//! implementation as [`v3`], and the SBOL 2 ⇄ SBOL 3 conversion API as
//! [`convert`]. It also adds version detection plus a version-neutral
//! [`AnyDocument`] handle over the underlying RDF layer.
#![forbid(unsafe_code)]

pub use sbol_core;
pub use sbol_rdf;
pub use sbol_rdf::{Graph as RdfGraph, Iri, Literal, RdfFormat, Resource, Term, Triple};

/// SBOL 2 ⇄ SBOL 3 conversion.
pub use sbol_convert as convert;
/// SBOL 2 implementation.
pub use sbol2 as v2;
/// SBOL 3 implementation.
pub use sbol3 as v3;

const SBOL_V2_NS: &str = "http://sbols.org/v2#";
const SBOL_V3_NS: &str = "http://sbols.org/v3#";

/// The SBOL major version a document is expressed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SbolVersion {
    /// SBOL 2 (`http://sbols.org/v2#`).
    V2,
    /// SBOL 3 (`http://sbols.org/v3#`).
    V3,
}

/// Detects the SBOL major version of an already-parsed RDF graph. SBOL 3
/// evidence takes precedence over SBOL 2 evidence; returns `None` when the
/// graph carries neither namespace.
pub fn detect_version_in_graph(graph: &RdfGraph) -> Option<SbolVersion> {
    let mut saw_v2 = false;
    for triple in graph.triples() {
        if triple.predicate.as_str().starts_with(SBOL_V3_NS) {
            return Some(SbolVersion::V3);
        }
        if let Some(iri) = triple.object.as_iri() {
            if iri.as_str().starts_with(SBOL_V3_NS) {
                return Some(SbolVersion::V3);
            }
            if iri.as_str().starts_with(SBOL_V2_NS) {
                saw_v2 = true;
            }
        }
        if triple.predicate.as_str().starts_with(SBOL_V2_NS) {
            saw_v2 = true;
        }
    }
    saw_v2.then_some(SbolVersion::V2)
}

/// Parses an in-memory RDF serialization and detects its SBOL major version.
/// Returns `None` when the input does not parse or carries no SBOL namespace.
pub fn detect_version(input: &str, format: RdfFormat) -> Option<SbolVersion> {
    RdfGraph::parse(input, format)
        .ok()
        .and_then(|g| detect_version_in_graph(&g))
}

/// A version-neutral handle over a parsed SBOL document. It wraps an SBOL 3
/// [`Document`](v3::Document); SBOL 2 documents are handled directly through
/// `v2::Document`. The enum is `#[non_exhaustive]` so further arms can be
/// added without a breaking change.
#[non_exhaustive]
pub enum AnyDocument {
    /// An SBOL 3 document.
    V3(v3::Document),
}

impl AnyDocument {
    /// Returns the SBOL major version of the wrapped document.
    pub fn version(&self) -> SbolVersion {
        match self {
            AnyDocument::V3(_) => SbolVersion::V3,
        }
    }

    /// Serializes the wrapped document in the given RDF format.
    pub fn write(&self, format: RdfFormat) -> Result<String, v3::WriteError> {
        match self {
            AnyDocument::V3(d) => d.write(format),
        }
    }

    /// Borrows the wrapped document as SBOL 3, if it is SBOL 3.
    pub fn as_v3(&self) -> Option<&v3::Document> {
        match self {
            AnyDocument::V3(d) => Some(d),
        }
    }

    /// Consumes the handle and returns the SBOL 3 document, if it is SBOL 3.
    pub fn into_v3(self) -> Option<v3::Document> {
        match self {
            AnyDocument::V3(d) => Some(d),
        }
    }
}

pub mod prelude {
    //! Re-exports for most sbol-rs code: the SBOL 3 prelude plus the umbrella
    //! version-detection surface.
    pub use crate::v3::prelude::*;
    pub use crate::{AnyDocument, SbolVersion, detect_version};
}
