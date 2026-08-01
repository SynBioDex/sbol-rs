//! Biological collection synchronization.

use url::Url;

use super::RegistryClient;
use crate::response::error_message;
use crate::url::validate_design_iri;
use crate::{
    CollectionDescriptor, CollectionPrecondition, CollectionRdfFormat, CollectionWrite,
    PulledCollection, RegistryError,
};

impl RegistryClient {
    /// Read the synchronization descriptor for one visible collection.
    pub fn collection_descriptor(&self, iri: &str) -> Result<CollectionDescriptor, RegistryError> {
        validate_design_iri(iri)?;
        let mut url = self.endpoint("api/v2/collections/")?;
        url.path_segments_mut()
            .map_err(|_| RegistryError::CannotBeBase(self.base_url.to_string()))?
            .pop_if_empty()
            .push(iri);
        let response = self.get(url, "application/json")?;
        serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)
    }

    /// Download only the biological SBOL document used for collection
    /// synchronization. Server-managed ACL, audit, review, and timestamps are
    /// excluded from both the body and the returned content ETag.
    pub fn pull_collection(
        &self,
        iri: &str,
        format: CollectionRdfFormat,
    ) -> Result<PulledCollection, RegistryError> {
        let url = self.collection_content_url(iri)?;
        let response = self.get(url.clone(), format.media_type())?;
        let etag = response.etag.ok_or(RegistryError::MissingContentEtag)?;
        Ok(PulledCollection {
            body: response.body,
            content_type: response.content_type,
            content_etag: etag,
            source_url: url,
        })
    }

    /// Strict create-or-CAS collection replacement. There is intentionally no
    /// unconditional variant.
    pub fn put_collection(
        &self,
        iri: &str,
        format: CollectionRdfFormat,
        body: &[u8],
        precondition: CollectionPrecondition<'_>,
    ) -> Result<CollectionWrite, RegistryError> {
        let url = self.collection_content_url(iri)?;
        let mut request = self
            .agent
            .put(url.as_str())
            .header("Accept", "application/json")
            .content_type(format.media_type());
        request = match precondition {
            CollectionPrecondition::Create => request.header("If-None-Match", "*"),
            CollectionPrecondition::Matches(etag) => request.header("If-Match", etag),
        };
        if let Some(token) = &self.bearer_token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        let mut response = request.send(body).map_err(RegistryError::Transport)?;
        let status = response.status().as_u16();
        let current_content_etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = response
            .body_mut()
            .with_config()
            .limit(self.max_response_bytes)
            .read_to_vec()
            .map_err(RegistryError::Transport)?;
        if status == 412 {
            return Err(RegistryError::PreconditionFailed {
                current_content_etag,
            });
        }
        if !(200..300).contains(&status) {
            return Err(RegistryError::HttpStatus {
                status,
                message: error_message(&bytes),
            });
        }
        serde_json::from_slice(&bytes).map_err(RegistryError::InvalidJson)
    }

    fn collection_content_url(&self, iri: &str) -> Result<Url, RegistryError> {
        validate_design_iri(iri)?;
        let mut url = self.endpoint("api/v2/collections/")?;
        url.path_segments_mut()
            .map_err(|_| RegistryError::CannotBeBase(self.base_url.to_string()))?
            .pop_if_empty()
            .push(iri)
            .push("content");
        Ok(url)
    }
}
