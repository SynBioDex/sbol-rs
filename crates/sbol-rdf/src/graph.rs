use crate::backends::{Backend, DefaultBackend, are_isomorphic};
use crate::traits::{RdfGraph, RdfIo};
use crate::{ParseError, RdfFormat, Triple, WriteError};

/// Backend-opaque in-memory RDF graph.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Graph {
    triples: Vec<Triple>,
}

impl Graph {
    pub fn new(triples: Vec<Triple>) -> Self {
        Self { triples }
    }

    pub fn triples(&self) -> &[Triple] {
        <Self as RdfGraph>::triples(self)
    }

    pub fn normalized_triples(&self) -> Vec<Triple> {
        <Self as RdfGraph>::normalized_triples(self)
    }

    /// Tests RDF graph isomorphism, treating blank-node identifiers as local
    /// labels rather than observable identities.
    ///
    /// This is the semantic comparison required for RDF read/write
    /// conformance. Use [`normalized_triples`](Self::normalized_triples) only
    /// when the graph is known to contain no blank nodes.
    pub fn is_isomorphic_with(&self, other: &Self) -> Result<bool, WriteError> {
        are_isomorphic(&self.triples, &other.triples)
    }

    /// Parses an RDF document in the given format.
    pub fn parse(input: &str, format: RdfFormat) -> Result<Self, ParseError> {
        <Self as RdfIo>::parse(input, format)
    }

    /// Serializes the graph in the given format.
    pub fn write(&self, format: RdfFormat) -> Result<String, WriteError> {
        <Self as RdfIo>::write(self, format)
    }

    /// Convenience wrapper for [`Graph::parse`] with [`RdfFormat::Turtle`].
    pub fn parse_turtle(input: &str) -> Result<Self, ParseError> {
        Self::parse(input, RdfFormat::Turtle)
    }

    /// Convenience wrapper for [`Graph::write`] with [`RdfFormat::Turtle`].
    pub fn write_turtle(&self) -> Result<String, WriteError> {
        self.write(RdfFormat::Turtle)
    }
}

impl RdfGraph for Graph {
    fn triples(&self) -> &[Triple] {
        &self.triples
    }
}

impl RdfIo for Graph {
    fn parse(input: &str, format: RdfFormat) -> Result<Self, ParseError> {
        Ok(Self::new(DefaultBackend::parse(input, format)?))
    }

    fn write(&self, format: RdfFormat) -> Result<String, WriteError> {
        DefaultBackend::write(&self.triples, format)
    }
}
