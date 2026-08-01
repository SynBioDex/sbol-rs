//! Versioned `sbol.toml` and `sbol.lock` data models.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const MANIFEST_FILE: &str = "sbol.toml";
pub const LOCK_FILE: &str = "sbol.lock";
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    #[serde(default = "default_designs_dir")]
    pub designs_dir: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_registry: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub collections: BTreeMap<String, CollectionSpec>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            designs_dir: default_designs_dir(),
            default_registry: None,
            collections: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionSpec {
    pub uri: String,
    pub registry: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lockfile {
    pub schema_version: u32,
    #[serde(default)]
    pub collections: BTreeMap<String, LockedCollection>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            collections: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedCollection {
    pub uri: String,
    pub registry: String,
    pub path: PathBuf,
    pub remote_content_etag: String,
    pub local_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalState {
    Missing,
    Unchanged,
    Modified,
}

fn default_designs_dir() -> PathBuf {
    PathBuf::from("designs")
}
