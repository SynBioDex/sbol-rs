//! Collection synchronization request and response models.

use serde::{Deserialize, Serialize};
use url::Url;

/// Lightweight collection synchronization descriptor.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CollectionDescriptor {
    pub iri: String,
    pub content_url: String,
    pub content_etag: String,
    pub triple_count: usize,
    #[serde(default)]
    pub display_id: Option<String>,
}

/// RDF serialization used for collection synchronization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionRdfFormat {
    Turtle,
    RdfXml,
    JsonLd,
    NTriples,
}

impl CollectionRdfFormat {
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Turtle => "text/turtle",
            Self::RdfXml => "application/rdf+xml",
            Self::JsonLd => "application/ld+json",
            Self::NTriples => "application/n-triples",
        }
    }
}

/// Required condition for a collection write.
#[derive(Clone, Copy, Debug)]
pub enum CollectionPrecondition<'a> {
    /// Create only when no collection exists at the target identity.
    Create,
    /// Replace only when the current biological content matches this ETag.
    Matches(&'a str),
}

/// Result of a successful conditional collection write.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CollectionWrite {
    pub collection_uri: String,
    pub content_etag: String,
    pub triple_count: usize,
}

/// Biological collection content and its synchronization validator.
#[derive(Clone, Debug)]
pub struct PulledCollection {
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub content_etag: String,
    pub source_url: Url,
}
