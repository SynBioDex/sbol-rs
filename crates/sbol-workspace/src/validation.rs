//! Schema and path invariants for workspace declarations.

use std::path::{Component, Path};

use crate::{CURRENT_SCHEMA_VERSION, CollectionSpec, Error, Lockfile, Manifest};

pub(crate) fn validate_manifest(manifest: &Manifest) -> Result<(), Error> {
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

pub(crate) fn validate_lockfile(manifest: &Manifest, lockfile: &Lockfile) -> Result<(), Error> {
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

pub(crate) fn validate_collection_spec(
    designs_dir: &Path,
    spec: &CollectionSpec,
) -> Result<(), Error> {
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

pub(crate) fn validate_alias(alias: &str) -> Result<(), Error> {
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
