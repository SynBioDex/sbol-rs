//! General design-download models.

use url::Url;

/// The SBOL vocabulary requested from the registry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SbolVersion {
    /// SBOL 2 RDF.
    V2,
    /// SBOL 3 RDF.
    #[default]
    V3,
}

impl SbolVersion {
    pub(crate) fn query_value(self) -> &'static str {
        match self {
            Self::V2 => "sbol2",
            Self::V3 => "sbol3",
        }
    }
}

/// A downloaded SBOL representation plus revision metadata.
#[derive(Clone, Debug)]
pub struct PulledDesign {
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub source_url: Url,
}
