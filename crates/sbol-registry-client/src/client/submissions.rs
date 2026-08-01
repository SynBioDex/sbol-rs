//! Collection submission preview and creation.

use super::RegistryClient;
use crate::{RegistryError, SubmissionCreated, SubmissionPreview, SubmissionRequest};

impl RegistryClient {
    /// Run the registry's authoritative submission preparation and collision
    /// analysis without writing data.
    pub fn preview_submission(
        &self,
        request: &SubmissionRequest,
    ) -> Result<SubmissionPreview, RegistryError> {
        let url = self.endpoint("api/v2/collections/validate")?;
        self.post_json(url, request)
    }

    /// Create a private collection in the authenticated caller's namespace.
    ///
    /// Call [`RegistryClient::preview_submission`] first when the consequence
    /// needs to be shown to a human before committing. The server repeats the
    /// same validation and collision checks during this call.
    pub fn create_submission(
        &self,
        request: &SubmissionRequest,
    ) -> Result<SubmissionCreated, RegistryError> {
        let url = self.endpoint("api/v2/collections")?;
        self.post_json(url, request)
    }
}
