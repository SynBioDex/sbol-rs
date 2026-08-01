//! Registry instance discovery models.

use serde::{Deserialize, Serialize};

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
