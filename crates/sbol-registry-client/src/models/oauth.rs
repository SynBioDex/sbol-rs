//! SBOL Identity OAuth metadata and token models.

use std::fmt;

use serde::{Deserialize, Serialize};

/// RFC 8414 metadata used by public CLI and desktop clients.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: String,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Vec<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

/// Dynamic registration result for an OAuth public client.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OAuthClientRegistration {
    pub client_id: String,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
}

/// Short-lived access token and rotating refresh token returned by SBOL
/// Identity. Secret fields are never formatted by a custom `Debug` impl.
#[derive(Clone, Deserialize, Serialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub scope: String,
    pub resource: String,
    #[serde(default)]
    pub id_token: Option<String>,
}

impl fmt::Debug for OAuthTokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthTokenResponse")
            .field("access_token", &"[REDACTED]")
            .field("token_type", &self.token_type)
            .field("refresh_token", &"[REDACTED]")
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .field("resource", &self.resource)
            .field("id_token", &self.id_token.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}
