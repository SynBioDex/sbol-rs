//! Local registry profile and bearer-token persistence for `sbol registry`.
//!
//! The file format is deliberately versioned and already leaves room for an
//! OAuth refresh token and issuer. The initial compatibility login stores only
//! the opaque access token returned by an SBOL DB instance. Secret-bearing
//! structs intentionally do not implement `Debug`.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub(super) struct CredentialStore {
    path: PathBuf,
}

impl CredentialStore {
    pub(super) fn discover() -> Result<Self, String> {
        if let Some(path) = env::var_os("SBOL_CREDENTIALS_FILE").filter(|path| !path.is_empty()) {
            return Ok(Self::at(PathBuf::from(path)));
        }
        if let Some(root) = env::var_os("XDG_CONFIG_HOME").filter(|path| !path.is_empty()) {
            return Ok(Self::at(
                PathBuf::from(root).join("sbol").join("credentials.json"),
            ));
        }
        #[cfg(windows)]
        if let Some(root) = env::var_os("APPDATA").filter(|path| !path.is_empty()) {
            return Ok(Self::at(
                PathBuf::from(root).join("sbol").join("credentials.json"),
            ));
        }
        let home = env::var_os("HOME")
            .filter(|path| !path.is_empty())
            .ok_or_else(|| {
                "cannot locate the SBOL credentials file; set SBOL_CREDENTIALS_FILE".to_owned()
            })?;
        Ok(Self::at(
            PathBuf::from(home)
                .join(".config")
                .join("sbol")
                .join("credentials.json"),
        ))
    }

    pub(super) fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub(super) fn active_registry(&self) -> Result<Option<String>, String> {
        Ok(self.load()?.active_registry)
    }

    pub(super) fn token_for(&self, registry: &str) -> Result<Option<String>, String> {
        Ok(self
            .load()?
            .registries
            .get(registry)
            .map(|credential| credential.access_token.clone()))
    }

    pub(super) fn save_access_token(&self, registry: &str, token: String) -> Result<(), String> {
        let mut credentials = self.load()?;
        credentials.active_registry = Some(registry.to_owned());
        credentials.registries.insert(
            registry.to_owned(),
            StoredCredential {
                access_token: token,
                refresh_token: None,
                expires_at: None,
                issuer: None,
            },
        );
        self.save(&credentials)
    }

    fn load(&self) -> Result<CredentialsFile, String> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CredentialsFile::default());
            }
            Err(error) => {
                return Err(format!(
                    "failed to read SBOL credentials from {}: {error}",
                    self.path.display()
                ));
            }
        };
        let credentials: CredentialsFile = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse SBOL credentials at {}: {error}",
                self.path.display()
            )
        })?;
        if credentials.version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported SBOL credentials schema {} at {} (expected {SCHEMA_VERSION})",
                credentials.version,
                self.path.display()
            ));
        }
        Ok(credentials)
    }

    fn save(&self, credentials: &CredentialsFile) -> Result<(), String> {
        let parent = self.path.parent().ok_or_else(|| {
            format!(
                "SBOL credentials path has no parent directory: {}",
                self.path.display()
            )
        })?;
        let created_parent = !parent.exists();
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create SBOL configuration directory {}: {error}",
                parent.display()
            )
        })?;
        // Only chmod a directory created for this store. An explicit
        // SBOL_CREDENTIALS_FILE may intentionally live in a pre-existing
        // shared directory such as a CI workspace; changing that parent would
        // exceed the credential writer's scope.
        if created_parent {
            secure_directory(parent)?;
        }

        let json = serde_json::to_vec_pretty(credentials)
            .map_err(|error| format!("failed to serialize SBOL credentials: {error}"))?;
        let temporary = temporary_path(&self.path);
        let write_result = (|| -> Result<(), String> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary).map_err(|error| {
                format!(
                    "failed to create temporary SBOL credentials file {}: {error}",
                    temporary.display()
                )
            })?;
            file.write_all(&json).map_err(|error| {
                format!(
                    "failed to write temporary SBOL credentials file {}: {error}",
                    temporary.display()
                )
            })?;
            file.write_all(b"\n").map_err(|error| {
                format!(
                    "failed to finish temporary SBOL credentials file {}: {error}",
                    temporary.display()
                )
            })?;
            file.sync_all().map_err(|error| {
                format!(
                    "failed to sync temporary SBOL credentials file {}: {error}",
                    temporary.display()
                )
            })?;
            fs::rename(&temporary, &self.path).map_err(|error| {
                format!(
                    "failed to install SBOL credentials at {}: {error}",
                    self.path.display()
                )
            })?;
            secure_file(&self.path)
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write_result
    }
}

#[derive(Deserialize, Serialize)]
struct CredentialsFile {
    #[serde(default = "schema_version")]
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_registry: Option<String>,
    #[serde(default)]
    registries: BTreeMap<String, StoredCredential>,
}

impl Default for CredentialsFile {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            active_registry: None,
            registries: BTreeMap::new(),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct StoredCredential {
    access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    issuer: Option<String>,
}

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

fn temporary_path(path: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("credentials.json");
    path.with_file_name(format!(".{name}.{}.{}.tmp", std::process::id(), nonce))
}

#[cfg(unix)]
fn secure_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
        format!(
            "failed to secure SBOL configuration directory {}: {error}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn secure_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn secure_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "failed to secure SBOL credentials file {}: {error}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn secure_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_tokens_by_normalized_registry_without_debugging_them() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sbol").join("credentials.json");
        let store = CredentialStore::at(path.clone());
        store
            .save_access_token("https://sbol.io/", "top-secret".to_owned())
            .unwrap();

        assert_eq!(
            store.active_registry().unwrap().as_deref(),
            Some("https://sbol.io/")
        );
        assert_eq!(
            store.token_for("https://sbol.io/").unwrap().as_deref(),
            Some("top-secret")
        );
        assert_eq!(store.token_for("https://other.example/").unwrap(), None);
        assert!(!format!("{store:?}").contains("top-secret"));
        assert!(path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
