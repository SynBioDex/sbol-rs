//! Workspace manifest, schema, and filesystem errors.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("an SBOL workspace already exists at {0}")]
    AlreadyInitialized(PathBuf),
    #[error("unsupported SBOL workspace schema version {0}")]
    UnsupportedSchema(u32),
    #[error("invalid SBOL workspace schema: {0}")]
    InvalidSchema(String),
    #[error("failed to read TOML at {path}: {source}")]
    InvalidToml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to serialize SBOL workspace TOML: {0}")]
    SerializeToml(#[source] toml::ser::Error),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
