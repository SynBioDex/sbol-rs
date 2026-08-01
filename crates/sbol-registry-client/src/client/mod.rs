//! Synchronous registry client and shared HTTP plumbing.

mod collections;
mod designs;
mod discovery;
mod identity;
mod submissions;

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::RegistryError;
use crate::response::{BufferedResponse, error_message};
use crate::url::{is_secure_registry_url, normalize_registry_url};

const DEFAULT_MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
const USER_AGENT: &str = concat!("sbol-registry-client/", env!("CARGO_PKG_VERSION"));

/// A synchronous client for one SBOL registry origin.
#[derive(Clone)]
pub struct RegistryClient {
    base_url: Url,
    agent: ureq::Agent,
    bearer_token: Option<String>,
    max_response_bytes: u64,
}

impl fmt::Debug for RegistryClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistryClient")
            .field("base_url", &self.base_url)
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("max_response_bytes", &self.max_response_bytes)
            .finish_non_exhaustive()
    }
}

impl RegistryClient {
    /// Construct a client rooted at a registry URL.
    ///
    /// A path prefix is preserved, so an installation at
    /// `https://example.org/synbiohub` resolves its API below that prefix.
    pub fn new(base_url: impl AsRef<str>) -> Result<Self, RegistryError> {
        let base_url = normalize_registry_url(base_url.as_ref())?;
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_recv_body(Some(Duration::from_secs(60)))
            .http_status_as_error(false)
            .user_agent(USER_AGENT)
            .build();
        Ok(Self {
            base_url,
            agent: ureq::Agent::new_with_config(config),
            bearer_token: None,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        })
    }

    /// Construct a client by taking the HTTP(S) origin of a design IRI.
    ///
    /// Registries mounted below a path prefix must be supplied explicitly with
    /// [`RegistryClient::new`] because that prefix cannot be inferred from an
    /// arbitrary object identity.
    pub fn from_design_iri(iri: &str) -> Result<Self, RegistryError> {
        let url = Url::parse(iri).map_err(|source| RegistryError::InvalidDesignIri {
            iri: iri.to_owned(),
            source,
        })?;
        if !is_secure_registry_url(&url) {
            return Err(RegistryError::UnsupportedDesignIri(iri.to_owned()));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(RegistryError::CredentialedDesignIri(iri.to_owned()));
        }
        let origin = url.origin().ascii_serialization();
        Self::new(origin)
    }

    /// Attach an opaque bearer token to subsequent requests.
    ///
    /// The token is never exposed by this client's `Debug` implementation.
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Override the maximum accepted response size.
    pub fn with_max_response_bytes(mut self, bytes: u64) -> Self {
        self.max_response_bytes = bytes;
        self
    }

    /// The normalized registry base URL.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    fn endpoint(&self, relative: &str) -> Result<Url, RegistryError> {
        self.base_url
            .join(relative)
            .map_err(RegistryError::InvalidEndpoint)
    }

    fn get(&self, url: Url, accept: &str) -> Result<BufferedResponse, RegistryError> {
        let mut request = self.agent.get(url.as_str()).header("Accept", accept);
        if let Some(token) = &self.bearer_token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }

        let response = request.call().map_err(RegistryError::Transport)?;
        self.buffer_response(response)
    }

    fn post_json<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        url: Url,
        value: &T,
    ) -> Result<R, RegistryError> {
        let body = serde_json::to_vec(value).map_err(RegistryError::SerializeJson)?;
        let mut request = self
            .agent
            .post(url.as_str())
            .header("Accept", "application/json")
            .content_type("application/json");
        if let Some(token) = &self.bearer_token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        let response = request
            .send(body.as_slice())
            .map_err(RegistryError::Transport)?;
        let response = self.buffer_response(response)?;
        serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)
    }

    fn buffer_response(
        &self,
        mut response: ureq::http::Response<ureq::Body>,
    ) -> Result<BufferedResponse, RegistryError> {
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response
            .body_mut()
            .with_config()
            .limit(self.max_response_bytes)
            .read_to_vec()
            .map_err(RegistryError::Transport)?;

        if !(200..300).contains(&status) {
            return Err(RegistryError::HttpStatus {
                status,
                message: error_message(&body),
            });
        }

        Ok(BufferedResponse {
            body,
            content_type,
            etag,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_registry_path_prefix() {
        let client = RegistryClient::new("https://example.org/synbiohub").unwrap();
        assert_eq!(
            client.endpoint("api/v2/instance").unwrap().as_str(),
            "https://example.org/synbiohub/api/v2/instance"
        );
    }

    #[test]
    fn infers_only_the_origin_from_a_design_iri() {
        let client =
            RegistryClient::from_design_iri("https://registry.example/public/igem/BBa_J23100/1")
                .unwrap();
        assert_eq!(client.base_url().as_str(), "https://registry.example/");
    }

    #[test]
    fn inferred_origin_rejects_cleartext_and_credentials() {
        assert!(matches!(
            RegistryClient::from_design_iri("https://alice:secret@example.org/design/1"),
            Err(RegistryError::CredentialedDesignIri(_))
        ));
        assert!(matches!(
            RegistryClient::from_design_iri("http://synbiohub.org/design/1"),
            Err(RegistryError::UnsupportedDesignIri(_))
        ));
    }

    #[test]
    fn debug_redacts_bearer_token() {
        let client = RegistryClient::new("https://example.org")
            .unwrap()
            .with_bearer_token("top-secret");
        let debug = format!("{client:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("top-secret"));
    }
}
