//! Reproducible local projects for synchronized SBOL collections.
//!
//! `sbol.toml` is the human-owned declaration and `sbol.lock` records the last
//! synchronized local hash and remote biological-content ETag. Authentication
//! material is deliberately outside both schemas.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

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

#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
    manifest: Manifest,
    lockfile: Lockfile,
}

impl Workspace {
    /// Initialize `root`, creating `sbol.toml` and `designs/` but deliberately
    /// not `sbol.lock`. The lock appears only after the first successful tracked
    /// pull or push establishes a synchronized state.
    pub fn init(root: impl AsRef<Path>, default_registry: Option<String>) -> Result<Self, Error> {
        let root = absolute(root.as_ref())?;
        fs::create_dir_all(&root).map_err(|source| Error::Io {
            path: root.clone(),
            source,
        })?;
        let manifest_path = root.join(MANIFEST_FILE);
        if manifest_path.exists() {
            return Err(Error::AlreadyInitialized(manifest_path));
        }
        let manifest = Manifest {
            default_registry,
            ..Manifest::default()
        };
        validate_manifest(&manifest)?;
        fs::create_dir_all(root.join(&manifest.designs_dir)).map_err(|source| Error::Io {
            path: root.join(&manifest.designs_dir),
            source,
        })?;
        write_toml_atomic(&manifest_path, &manifest)?;
        Ok(Self {
            root,
            manifest,
            lockfile: Lockfile::default(),
        })
    }

    /// Find the nearest ancestor carrying `sbol.toml`.
    pub fn discover(start: impl AsRef<Path>) -> Result<Option<Self>, Error> {
        let start = absolute(start.as_ref())?;
        let start = if start.is_file() {
            start.parent().unwrap_or(&start).to_path_buf()
        } else {
            start
        };
        for ancestor in start.ancestors() {
            if ancestor.join(MANIFEST_FILE).is_file() {
                return Self::load(ancestor).map(Some);
            }
        }
        Ok(None)
    }

    pub fn load(root: impl AsRef<Path>) -> Result<Self, Error> {
        let root = absolute(root.as_ref())?;
        let manifest_path = root.join(MANIFEST_FILE);
        let manifest: Manifest = read_toml(&manifest_path)?;
        validate_manifest(&manifest)?;
        let lock_path = root.join(LOCK_FILE);
        let lockfile = if lock_path.is_file() {
            let lockfile: Lockfile = read_toml(&lock_path)?;
            validate_lockfile(&manifest, &lockfile)?;
            lockfile
        } else {
            Lockfile::default()
        };
        Ok(Self {
            root,
            manifest,
            lockfile,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn lockfile(&self) -> &Lockfile {
        &self.lockfile
    }

    pub fn collection(&self, alias: &str) -> Option<(&CollectionSpec, Option<&LockedCollection>)> {
        self.manifest
            .collections
            .get(alias)
            .map(|spec| (spec, self.lockfile.collections.get(alias)))
    }

    pub fn collection_by_uri(
        &self,
        uri: &str,
    ) -> Option<(&str, &CollectionSpec, Option<&LockedCollection>)> {
        self.manifest
            .collections
            .iter()
            .find(|(_, spec)| spec.uri == uri)
            .map(|(alias, spec)| (alias.as_str(), spec, self.lockfile.collections.get(alias)))
    }

    pub fn absolute_collection_path(&self, spec: &CollectionSpec) -> PathBuf {
        self.root.join(&spec.path)
    }

    pub fn next_alias(&self, preferred: &str) -> String {
        if !self.manifest.collections.contains_key(preferred) {
            return preferred.to_owned();
        }
        for suffix in 2.. {
            let candidate = format!("{preferred}-{suffix}");
            if !self.manifest.collections.contains_key(&candidate) {
                return candidate;
            }
        }
        unreachable!()
    }

    /// Record one successful synchronization and persist both files. The
    /// manifest write precedes the lock write so a crash cannot leave a lock
    /// entry with no declared collection.
    pub fn record_sync(
        &mut self,
        alias: String,
        spec: CollectionSpec,
        remote_content_etag: String,
        local_sha256: String,
    ) -> Result<(), Error> {
        validate_alias(&alias)?;
        validate_collection_spec(&self.manifest.designs_dir, &spec)?;
        if remote_content_etag.trim().is_empty() {
            return Err(Error::InvalidSchema(
                "remote content ETag cannot be empty".to_owned(),
            ));
        }
        if local_sha256.len() != 64
            || !local_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(Error::InvalidSchema(
                "local_sha256 must be 64 lowercase hexadecimal characters".to_owned(),
            ));
        }
        self.manifest
            .collections
            .insert(alias.clone(), spec.clone());
        self.lockfile.collections.insert(
            alias,
            LockedCollection {
                uri: spec.uri.clone(),
                registry: spec.registry.clone(),
                path: spec.path.clone(),
                remote_content_etag,
                local_sha256,
            },
        );
        self.save_manifest()?;
        self.save_lockfile()
    }

    pub fn local_state(&self, alias: &str) -> Result<LocalState, Error> {
        let Some((spec, Some(lock))) = self.collection(alias) else {
            return Ok(LocalState::Missing);
        };
        let path = self.absolute_collection_path(spec);
        if !path.is_file() {
            return Ok(LocalState::Missing);
        }
        let hash = sha256_file(&path)?;
        Ok(if hash == lock.local_sha256 {
            LocalState::Unchanged
        } else {
            LocalState::Modified
        })
    }

    pub fn save_manifest(&self) -> Result<(), Error> {
        validate_manifest(&self.manifest)?;
        write_toml_atomic(&self.root.join(MANIFEST_FILE), &self.manifest)
    }

    pub fn save_lockfile(&self) -> Result<(), Error> {
        validate_lockfile(&self.manifest, &self.lockfile)?;
        write_toml_atomic(&self.root.join(LOCK_FILE), &self.lockfile)
    }
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn sha256_file(path: impl AsRef<Path>) -> Result<String, Error> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(sha256_bytes(&bytes))
}

/// Derive a succinct filesystem-safe name from an advertised display id or a
/// versioned collection URI. The URI version is never used as the filename.
pub fn safe_collection_name(uri: &str, display_id: Option<&str>) -> String {
    let source = display_id
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            Url::parse(uri).ok().and_then(|url| {
                let segments = url.path_segments()?.collect::<Vec<_>>();
                let candidate = if segments.len() >= 2 {
                    segments[segments.len() - 2]
                } else {
                    segments.last().copied().unwrap_or("design")
                };
                Some(candidate.trim_end_matches("_collection").to_owned())
            })
        })
        .unwrap_or_else(|| "design".to_owned());
    let mut output = String::new();
    let mut separator = false;
    for character in source.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            output.push(character);
            separator = false;
        } else if !separator && !output.is_empty() {
            output.push('_');
            separator = true;
        }
    }
    while output.ends_with(['_', '-']) {
        output.pop();
    }
    if output.is_empty() {
        "design".to_owned()
    } else {
        output
    }
}

pub fn write_file_atomic(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), Error> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("sbol");
    let temporary = path.with_file_name(format!(".{name}.tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|source| Error::Io {
                path: temporary.clone(),
                source,
            })?;
        file.write_all(bytes).map_err(|source| Error::Io {
            path: temporary.clone(),
            source,
        })?;
        file.sync_all().map_err(|source| Error::Io {
            path: temporary.clone(),
            source,
        })?;
        fs::rename(&temporary, path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn default_designs_dir() -> PathBuf {
    PathBuf::from("designs")
}

fn absolute(path: &Path) -> Result<PathBuf, Error> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|source| Error::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, Error> {
    let text = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| Error::InvalidToml {
        path: path.to_path_buf(),
        source,
    })
}

fn write_toml_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), Error> {
    let text = toml::to_string_pretty(value).map_err(Error::SerializeToml)?;
    write_file_atomic(path, text.as_bytes())
}

fn validate_manifest(manifest: &Manifest) -> Result<(), Error> {
    if manifest.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema(manifest.schema_version));
    }
    validate_relative_path(&manifest.designs_dir, "designs_dir")?;
    for (alias, spec) in &manifest.collections {
        validate_alias(alias)?;
        validate_collection_spec(&manifest.designs_dir, spec)?;
    }
    Ok(())
}

fn validate_lockfile(manifest: &Manifest, lockfile: &Lockfile) -> Result<(), Error> {
    if lockfile.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema(lockfile.schema_version));
    }
    for (alias, lock) in &lockfile.collections {
        let spec = manifest.collections.get(alias).ok_or_else(|| {
            Error::InvalidSchema(format!(
                "lock entry `{alias}` has no matching manifest collection"
            ))
        })?;
        if lock.uri != spec.uri || lock.registry != spec.registry || lock.path != spec.path {
            return Err(Error::InvalidSchema(format!(
                "lock entry `{alias}` does not match its manifest declaration"
            )));
        }
    }
    Ok(())
}

fn validate_collection_spec(designs_dir: &Path, spec: &CollectionSpec) -> Result<(), Error> {
    if spec.uri.trim().is_empty() || spec.registry.trim().is_empty() {
        return Err(Error::InvalidSchema(
            "collection uri and registry cannot be empty".to_owned(),
        ));
    }
    validate_relative_path(&spec.path, "collection path")?;
    if !spec.path.starts_with(designs_dir) {
        return Err(Error::InvalidSchema(format!(
            "collection path `{}` must be inside `{}`",
            spec.path.display(),
            designs_dir.display()
        )));
    }
    Ok(())
}

fn validate_relative_path(path: &Path, field: &str) -> Result<(), Error> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::InvalidSchema(format!(
            "{field} must be a non-empty relative path without `.` or `..`: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_alias(alias: &str) -> Result<(), Error> {
    if alias.is_empty()
        || !alias
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(Error::InvalidSchema(format!(
            "collection alias `{alias}` must contain only ASCII letters, digits, hyphens, or underscores"
        )));
    }
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_manifest_and_designs_but_not_lock() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = Workspace::init(temp.path(), None).unwrap();
        assert!(temp.path().join(MANIFEST_FILE).is_file());
        assert!(temp.path().join("designs").is_dir());
        assert!(!temp.path().join(LOCK_FILE).exists());
        assert_eq!(workspace.manifest(), &Manifest::default());
    }

    #[test]
    fn first_sync_persists_matching_manifest_and_lock() {
        let temp = tempfile::tempdir().unwrap();
        let mut workspace = Workspace::init(temp.path(), None).unwrap();
        let bytes = b"@prefix sbol: <http://sbols.org/v3#> .\n";
        let path = PathBuf::from("designs/toggle.ttl");
        write_file_atomic(temp.path().join(&path), bytes).unwrap();
        workspace
            .record_sync(
                "toggle".to_owned(),
                CollectionSpec {
                    uri: "https://sbol.io/user/alice/toggle/toggle_collection/1".to_owned(),
                    registry: "https://sbol.io/".to_owned(),
                    path,
                },
                "\"sbol-content-v1-abc\"".to_owned(),
                sha256_bytes(bytes),
            )
            .unwrap();
        assert!(temp.path().join(LOCK_FILE).is_file());
        let loaded = Workspace::load(temp.path()).unwrap();
        assert_eq!(loaded.local_state("toggle").unwrap(), LocalState::Unchanged);
    }

    #[test]
    fn collection_name_uses_display_or_identity_not_version() {
        assert_eq!(
            safe_collection_name(
                "https://sbol.io/user/alice/toggle/toggle_collection/1",
                None
            ),
            "toggle"
        );
        assert_eq!(
            safe_collection_name("https://sbol.io/design/99", Some("pTet toggle")),
            "pTet_toggle"
        );
    }

    #[test]
    fn traversal_paths_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = Manifest {
            designs_dir: PathBuf::from("../outside"),
            ..Manifest::default()
        };
        write_toml_atomic(&temp.path().join(MANIFEST_FILE), &manifest).unwrap();
        assert!(matches!(
            Workspace::load(temp.path()),
            Err(Error::InvalidSchema(_))
        ));
    }
}
