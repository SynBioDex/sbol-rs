//! Reproducible local projects for synchronized SBOL collections.
//!
//! `sbol.toml` is the human-owned declaration and `sbol.lock` records the last
//! synchronized local hash and remote biological-content ETag. Authentication
//! material is deliberately outside both schemas.
#![forbid(unsafe_code)]

pub mod error;
pub mod file_io;
pub mod model;
pub mod naming;
pub mod workspace;

mod validation;

pub use error::Error;
pub use file_io::{sha256_bytes, sha256_file, write_file_atomic};
pub use model::{
    CURRENT_SCHEMA_VERSION, CollectionSpec, LOCK_FILE, LocalState, LockedCollection, Lockfile,
    MANIFEST_FILE, Manifest,
};
pub use naming::safe_collection_name;
pub use workspace::Workspace;
