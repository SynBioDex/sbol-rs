//! Errors returned while discovering or calling a registry.

use thiserror::Error;

/// Failure while discovering or calling an SBOL registry.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("invalid registry URL `{url}`: {source}")]
    InvalidRegistryUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("registry URL must use HTTPS, or HTTP on a loopback host: `{0}`")]
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
    #[error("a registry origin cannot be inferred securely from design IRI `{0}`")]
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
    #[error("registry collection response did not include a biological content ETag")]
    MissingContentEtag,
    #[error(
        "collection precondition failed; current remote content ETag: {current_content_etag:?}"
    )]
    PreconditionFailed {
        current_content_etag: Option<String>,
    },
}
