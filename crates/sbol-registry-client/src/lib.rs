//! Typed client for the machine-facing API exposed by an SBOL registry.
//!
//! The crate deliberately contains no SBOL object model. It moves registry
//! representations and typed metadata over HTTP; callers such as `sbol-cli`
//! parse and validate the returned document with the `sbol` crate. Keeping that
//! boundary lets registries evolve independently while the existing SBOL API
//! remains the authority for document semantics.
#![forbid(unsafe_code)]

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

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
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
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

    /// Read public deployment identity and capability metadata.
    pub fn instance(&self) -> Result<InstanceInfo, RegistryError> {
        let url = self.endpoint("api/v2/instance")?;
        let response = self.get(url, "application/json")?;
        serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)
    }

    /// Exchange registry credentials for a bearer token through the
    /// SynBioHub-compatible login endpoint.
    ///
    /// This is the compatibility fallback for registries that do not yet
    /// advertise an OAuth authorization issuer. Clients should prefer browser
    /// authorization when `machine_access.authorization_issuer` is present.
    pub fn password_login(
        &self,
        identifier: &str,
        password: &str,
    ) -> Result<String, RegistryError> {
        let url = self.endpoint("login")?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("email", identifier)
            .append_pair("password", password)
            .finish();
        let response = self
            .agent
            .post(url.as_str())
            .header("Accept", "text/plain")
            .content_type("application/x-www-form-urlencoded")
            .send(body.as_bytes())
            .map_err(RegistryError::Transport)?;
        let response = self.buffer_response(response)?;
        let token = String::from_utf8(response.body)
            .map_err(RegistryError::InvalidUtf8)?
            .trim()
            .to_owned();
        if token.is_empty() {
            return Err(RegistryError::EmptyAccessToken);
        }
        Ok(token)
    }

    /// Discover the OAuth authorization server advertised by an SBOL DB
    /// instance. The issuer is supplied by `/api/v2/instance`; this method
    /// verifies that the returned metadata repeats it exactly.
    pub fn authorization_server(
        &self,
        issuer: &str,
    ) -> Result<AuthorizationServerMetadata, RegistryError> {
        let issuer_url = oauth_url(issuer, "OAuth issuer")?;
        let metadata_url = issuer_url
            .join(".well-known/oauth-authorization-server")
            .map_err(RegistryError::InvalidEndpoint)?;
        let response = self.get(metadata_url, "application/json")?;
        let metadata: AuthorizationServerMetadata =
            serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)?;
        if metadata.issuer != issuer {
            return Err(RegistryError::OAuthIssuerMismatch {
                expected: issuer.to_owned(),
                actual: metadata.issuer,
            });
        }
        if !metadata
            .code_challenge_methods_supported
            .iter()
            .any(|method| method == "S256")
        {
            return Err(RegistryError::OAuthCapability(
                "authorization server does not support S256 PKCE".to_owned(),
            ));
        }
        if !metadata
            .token_endpoint_auth_methods_supported
            .iter()
            .any(|method| method == "none")
        {
            return Err(RegistryError::OAuthCapability(
                "authorization server does not support public clients".to_owned(),
            ));
        }
        Ok(metadata)
    }

    /// Dynamically register a public authorization-code client for one exact
    /// loopback or HTTPS redirect URI.
    pub fn register_public_oauth_client(
        &self,
        metadata: &AuthorizationServerMetadata,
        client_name: &str,
        redirect_uri: &str,
    ) -> Result<OAuthClientRegistration, RegistryError> {
        let url = oauth_url(&metadata.registration_endpoint, "registration endpoint")?;
        let body = serde_json::to_vec(&serde_json::json!({
            "client_name": client_name,
            "redirect_uris": [redirect_uri],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none"
        }))
        .map_err(RegistryError::SerializeJson)?;
        let response = self
            .agent
            .post(url.as_str())
            .header("Accept", "application/json")
            .content_type("application/json")
            .send(body.as_slice())
            .map_err(RegistryError::Transport)?;
        let response = self.buffer_response(response)?;
        serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)
    }

    /// Exchange a single-use authorization code using the original S256 PKCE
    /// verifier and exact protected-resource audience.
    pub fn exchange_oauth_code(
        &self,
        metadata: &AuthorizationServerMetadata,
        client_id: &str,
        redirect_uri: &str,
        resource: &str,
        code: &str,
        code_verifier: &str,
    ) -> Result<OAuthTokenResponse, RegistryError> {
        self.oauth_token_request(
            metadata,
            &[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", client_id),
                ("redirect_uri", redirect_uri),
                ("resource", resource),
                ("code_verifier", code_verifier),
            ],
        )
    }

    /// Rotate a saved OAuth refresh token. The original client and audience
    /// bindings are repeated so a credential cannot migrate across registries.
    pub fn refresh_oauth_token(
        &self,
        metadata: &AuthorizationServerMetadata,
        client_id: &str,
        resource: &str,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse, RegistryError> {
        self.oauth_token_request(
            metadata,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", client_id),
                ("resource", resource),
            ],
        )
    }

    /// Revoke an OAuth access or refresh token. The endpoint intentionally
    /// returns no token-validity signal.
    pub fn revoke_oauth_token(
        &self,
        metadata: &AuthorizationServerMetadata,
        token: &str,
    ) -> Result<(), RegistryError> {
        let endpoint = metadata.revocation_endpoint.as_deref().ok_or_else(|| {
            RegistryError::OAuthCapability(
                "authorization server does not advertise token revocation".to_owned(),
            )
        })?;
        let url = oauth_url(endpoint, "revocation endpoint")?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("token", token)
            .finish();
        let response = self
            .agent
            .post(url.as_str())
            .header("Accept", "application/json")
            .content_type("application/x-www-form-urlencoded")
            .send(body.as_bytes())
            .map_err(RegistryError::Transport)?;
        self.buffer_response(response)?;
        Ok(())
    }

    /// Revoke a first-party compatibility token through the V2 session
    /// lifecycle. New SBOL Identity credentials use [`Self::revoke_oauth_token`].
    pub fn logout_compatibility_session(&self) -> Result<(), RegistryError> {
        let url = self.endpoint("api/v2/session")?;
        let mut request = self
            .agent
            .delete(url.as_str())
            .header("Accept", "application/json");
        if let Some(token) = &self.bearer_token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        let response = request.call().map_err(RegistryError::Transport)?;
        self.buffer_response(response)?;
        Ok(())
    }

    fn oauth_token_request(
        &self,
        metadata: &AuthorizationServerMetadata,
        fields: &[(&str, &str)],
    ) -> Result<OAuthTokenResponse, RegistryError> {
        let url = oauth_url(&metadata.token_endpoint, "token endpoint")?;
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in fields {
            form.append_pair(name, value);
        }
        let body = form.finish();
        let response = self
            .agent
            .post(url.as_str())
            .header("Accept", "application/json")
            .content_type("application/x-www-form-urlencoded")
            .send(body.as_bytes())
            .map_err(RegistryError::Transport)?;
        let response = self.buffer_response(response)?;
        let tokens: OAuthTokenResponse =
            serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)?;
        if !tokens.token_type.eq_ignore_ascii_case("bearer") {
            return Err(RegistryError::OAuthCapability(
                "token endpoint returned a non-Bearer access token".to_owned(),
            ));
        }
        Ok(tokens)
    }

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
    fn query_value(self) -> &'static str {
        match self {
            Self::V2 => "sbol2",
            Self::V3 => "sbol3",
        }
    }
}

/// Public deployment metadata returned by `/api/v2/instance`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstanceInfo {
    pub name: String,
    pub instance_url: String,
    pub uri_prefix: String,
    pub setup_required: bool,
    #[serde(default)]
    pub policies: InstancePolicies,
    /// Capability fields are additive, so retain them as JSON while the
    /// machine-access subset evolves independently.
    #[serde(default)]
    pub capabilities: serde_json::Value,
    /// Machine endpoints advertised by newer registries.
    #[serde(default)]
    pub machine_access: Option<MachineAccess>,
}

/// Public account policy relevant to machine clients.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct InstancePolicies {
    #[serde(default)]
    pub allow_public_signup: bool,
    #[serde(default)]
    pub require_login: bool,
}

/// Canonical endpoints for registry, MCP, and identity clients.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MachineAccess {
    pub api_url: String,
    #[serde(default)]
    pub mcp_url: Option<String>,
    #[serde(default)]
    pub authorization_issuer: Option<String>,
}

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

/// A collection upload prepared by the CLI and interpreted authoritatively by
/// SBOL DB. The server re-homes the submitted top-level identities into the
/// authenticated caller's namespace.
#[derive(Clone, Debug, Serialize)]
pub struct SubmissionRequest {
    pub id: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_name: Option<String>,
    pub format: SubmissionFormat,
    pub overwrite: CollisionPolicy,
    pub content: String,
}

/// Serialization name accepted by the SBOL DB submission API.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SubmissionFormat {
    #[serde(rename = "rdfxml")]
    RdfXml,
    Turtle,
    JsonLd,
    #[serde(rename = "ntriples")]
    NTriples,
    GenBank,
    Fasta,
}

/// Explicit behavior when a submission target already exists.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    #[default]
    Fail,
    Replace,
    Merge,
}

/// Write-free server analysis of a future submission.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SubmissionPreview {
    pub valid: bool,
    pub source_format: String,
    pub source_standard: String,
    pub normalized_standard: String,
    pub collection_uri: String,
    pub persistent_identity: String,
    pub graph: String,
    pub members: Vec<String>,
    pub triple_count: usize,
    pub collision: bool,
    pub consequence: SubmissionConsequence,
    pub notices: Vec<SubmissionNotice>,
}

/// What a submission would do at its target graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionConsequence {
    Create,
    RejectConflict,
    Replace,
    Merge,
}

/// Stable validation or collision information attached to a preview.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SubmissionNotice {
    pub code: String,
    pub level: SubmissionNoticeLevel,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionNoticeLevel {
    Info,
    Warning,
}

/// Result returned after an authenticated collection creation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SubmissionCreated {
    pub collection_uri: String,
    pub persistent_identity: String,
    pub members: Vec<String>,
    pub graph: String,
    pub triple_count: usize,
}

/// A downloaded SBOL representation plus revision metadata.
#[derive(Clone, Debug)]
pub struct PulledDesign {
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub source_url: Url,
}

struct BufferedResponse {
    body: Vec<u8>,
    content_type: Option<String>,
    etag: Option<String>,
}

/// Failure while discovering or calling an SBOL registry.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("invalid registry URL `{url}`: {source}")]
    InvalidRegistryUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("registry URL must use http or https and include a host: `{0}`")]
    UnsupportedRegistryUrl(String),
    #[error("registry URL must not include credentials, a query, or a fragment: `{0}`")]
    InvalidRegistryBase(String),
    #[error("invalid {kind} URL `{url}`")]
    InvalidOAuthUrl { kind: &'static str, url: String },
    #[error("OAuth metadata issuer mismatch: expected `{expected}`, received `{actual}`")]
    OAuthIssuerMismatch { expected: String, actual: String },
    #[error("incompatible OAuth authorization server: {0}")]
    OAuthCapability(String),
    #[error("invalid design IRI `{iri}`: {source}")]
    InvalidDesignIri {
        iri: String,
        #[source]
        source: url::ParseError,
    },
    #[error("design IRI must use http or https and include a host: `{0}`")]
    UnsupportedDesignIri(String),
    #[error("design IRI must not include credentials: `{0}`")]
    CredentialedDesignIri(String),
    #[error("registry URL cannot be used as a URL base: `{0}`")]
    CannotBeBase(String),
    #[error("failed to construct registry endpoint: {0}")]
    InvalidEndpoint(#[source] url::ParseError),
    #[error("registry transport failed: {0}")]
    Transport(#[source] ureq::Error),
    #[error("registry returned HTTP {status}: {message}")]
    HttpStatus { status: u16, message: String },
    #[error("registry returned invalid JSON: {0}")]
    InvalidJson(#[source] serde_json::Error),
    #[error("failed to serialize registry request as JSON: {0}")]
    SerializeJson(#[source] serde_json::Error),
    #[error("registry returned text that is not UTF-8: {0}")]
    InvalidUtf8(#[source] std::string::FromUtf8Error),
    #[error("registry returned an empty access token")]
    EmptyAccessToken,
}

fn oauth_url(value: &str, kind: &'static str) -> Result<Url, RegistryError> {
    let url = Url::parse(value).map_err(|_| RegistryError::InvalidOAuthUrl {
        kind,
        url: value.to_owned(),
    })?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || !(matches!(url.scheme(), "https") || url.scheme() == "http" && loopback)
    {
        return Err(RegistryError::InvalidOAuthUrl {
            kind,
            url: value.to_owned(),
        });
    }
    Ok(url)
}

fn normalize_registry_url(value: &str) -> Result<Url, RegistryError> {
    let mut url = Url::parse(value).map_err(|source| RegistryError::InvalidRegistryUrl {
        url: value.to_owned(),
        source,
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(RegistryError::UnsupportedRegistryUrl(value.to_owned()));
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(RegistryError::InvalidRegistryBase(value.to_owned()));
    }
    let path = format!("{}/", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn validate_design_iri(value: &str) -> Result<(), RegistryError> {
    let url = Url::parse(value).map_err(|source| RegistryError::InvalidDesignIri {
        iri: value.to_owned(),
        source,
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(RegistryError::UnsupportedDesignIri(value.to_owned()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(RegistryError::CredentialedDesignIri(value.to_owned()));
    }
    Ok(())
}

fn error_message(body: &[u8]) -> String {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        for pointer in ["/error/message", "/message", "/error"] {
            if let Some(message) = value.pointer(pointer).and_then(|value| value.as_str()) {
                return message.to_owned();
            }
        }
    }
    let text = String::from_utf8_lossy(body);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "empty response body".to_owned()
    } else {
        trimmed.chars().take(500).collect()
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
    fn rejects_secret_bearing_registry_urls_and_design_iris() {
        assert!(matches!(
            RegistryClient::new("https://alice:secret@example.org"),
            Err(RegistryError::InvalidRegistryBase(_))
        ));
        assert!(matches!(
            RegistryClient::new("https://example.org?token=secret"),
            Err(RegistryError::InvalidRegistryBase(_))
        ));
        assert!(matches!(
            RegistryClient::from_design_iri("https://alice:secret@example.org/design/1"),
            Err(RegistryError::CredentialedDesignIri(_))
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

    #[test]
    fn extracts_structured_error_message() {
        let body = br#"{"error":{"code":"not_found","message":"design missing"}}"#;
        assert_eq!(error_message(body), "design missing");
    }
}
