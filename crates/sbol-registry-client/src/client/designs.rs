//! General design downloads.

use super::RegistryClient;
use crate::url::validate_design_iri;
use crate::{PulledDesign, RegistryError, SbolVersion};

impl RegistryClient {
    /// Download the recursive SBOL closure rooted at `iri`.
    pub fn pull(&self, iri: &str, version: SbolVersion) -> Result<PulledDesign, RegistryError> {
        validate_design_iri(iri)?;
        let mut url = self.endpoint("api/v2/objects/")?;
        url.path_segments_mut()
            .map_err(|_| RegistryError::CannotBeBase(self.base_url.to_string()))?
            .pop_if_empty()
            .push(iri);
        url.query_pairs_mut()
            .append_pair("format", "sbol")
            .append_pair("version", version.query_value());

        let response = self.get(url.clone(), "application/rdf+xml")?;
        Ok(PulledDesign {
            body: response.body,
            content_type: response.content_type,
            etag: response.etag,
            source_url: url,
        })
    }
}
