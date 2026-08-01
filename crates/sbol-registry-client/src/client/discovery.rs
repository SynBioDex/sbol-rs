//! Registry instance discovery.

use super::RegistryClient;
use crate::{InstanceInfo, RegistryError};

impl RegistryClient {
    /// Read public deployment identity and capability metadata.
    pub fn instance(&self) -> Result<InstanceInfo, RegistryError> {
        let url = self.endpoint("api/v2/instance")?;
        let response = self.get(url, "application/json")?;
        serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)
    }
}
