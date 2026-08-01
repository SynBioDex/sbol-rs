//! Collection submission request, preview, and result models.

use serde::{Deserialize, Serialize};

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

/// Severity attached to a submission notice.
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
