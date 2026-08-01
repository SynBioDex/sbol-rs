//! Workspace discovery, lifecycle, and synchronization state.

use std::fs;
use std::path::{Path, PathBuf};

use crate::file_io::{absolute, read_toml, write_toml_atomic};
use crate::validation::{
    validate_alias, validate_collection_spec, validate_lockfile, validate_manifest,
};
use crate::{
    CollectionSpec, Error, LOCK_FILE, LocalState, LockedCollection, Lockfile, MANIFEST_FILE,
    Manifest, sha256_file,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_io::write_toml_atomic;
    use crate::{sha256_bytes, write_file_atomic};

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
