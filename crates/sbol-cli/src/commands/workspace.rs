use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use sbol::{RdfFormat, SbolVersion};
use sbol_registry_client::{
    CollectionDescriptor, CollectionPrecondition, CollectionRdfFormat, RegistryClient,
    RegistryError,
};
use sbol_workspace::{
    CollectionSpec, LocalState, LockedCollection, Workspace, sha256_bytes, sha256_file,
    write_file_atomic,
};
use serde::Serialize;

use crate::cli::{InitArgs, WorkspaceStatusArgs, WorkspaceSyncArgs};
use crate::commands::registry::registry_client;
use crate::style::Styles;

pub(crate) fn init(args: InitArgs, styles: Styles) -> ExitCode {
    let registry = match args.registry {
        Some(registry) => match RegistryClient::new(&registry) {
            Ok(client) => Some(client.base_url().to_string()),
            Err(error) => {
                eprintln!("{}: {error}", styles.err_label());
                return ExitCode::from(2);
            }
        },
        None => None,
    };
    match Workspace::init(&args.path, registry) {
        Ok(workspace) => {
            println!("initialized SBOL project at {}", workspace.root().display());
            println!(
                "  manifest: {}",
                workspace.root().join("sbol.toml").display()
            );
            println!(
                "  designs: {}",
                workspace
                    .root()
                    .join(&workspace.manifest().designs_dir)
                    .display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}: {error}", styles.err_label());
            ExitCode::from(2)
        }
    }
}

pub(crate) fn status(args: WorkspaceStatusArgs, styles: Styles) -> ExitCode {
    let workspace = match current_workspace() {
        Ok(workspace) => workspace,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let inspections = match inspect(&workspace, &args.collections) {
        Ok(inspections) => inspections,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    if args.json {
        let rows = inspections.iter().map(Inspection::row).collect::<Vec<_>>();
        match serde_json::to_string_pretty(&rows) {
            Ok(value) => println!("{value}"),
            Err(error) => {
                eprintln!("{}: could not render status: {error}", styles.err_label());
                return ExitCode::from(2);
            }
        }
    } else if inspections.is_empty() {
        println!("no collections are tracked; run `sbol registry pull <collection-uri>`");
    } else {
        for inspection in &inspections {
            println!("{:<20} {}", inspection.alias, inspection.action.label());
            println!("  local:  {}", inspection.spec.path.display());
            println!("  remote: {}", inspection.spec.uri);
            if let Some(reason) = &inspection.reason {
                println!("  note:   {reason}");
            }
        }
    }
    if inspections
        .iter()
        .any(|inspection| inspection.action == Action::Conflict)
    {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

pub(crate) fn sync(args: WorkspaceSyncArgs, styles: Styles) -> ExitCode {
    let mut workspace = match current_workspace() {
        Ok(workspace) => workspace,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let inspections = match inspect(&workspace, &args.collections) {
        Ok(inspections) => inspections,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    if inspections
        .iter()
        .any(|inspection| inspection.action == Action::Conflict)
    {
        for inspection in inspections {
            if inspection.action == Action::Conflict {
                eprintln!(
                    "{}: {}: {}",
                    styles.err_label(),
                    inspection.alias,
                    inspection.reason.unwrap_or_else(|| "conflict".to_owned())
                );
            }
        }
        eprintln!(
            "no collections were changed; resolve conflicts with an explicit registry pull or push"
        );
        return ExitCode::from(1);
    }

    for inspection in inspections {
        match inspection.action {
            Action::Clean => println!("{} is up to date", inspection.alias),
            Action::Pull => {
                println!(
                    "{} {}",
                    if args.dry_run {
                        "would pull"
                    } else {
                        "pulling"
                    },
                    inspection.alias
                );
                if args.dry_run {
                    continue;
                }
                if let Err(message) = pull_one(&mut workspace, &inspection) {
                    eprintln!("{}: {message}", styles.err_label());
                    return ExitCode::from(2);
                }
            }
            Action::Push => {
                println!(
                    "{} {}",
                    if args.dry_run {
                        "would push"
                    } else {
                        "pushing"
                    },
                    inspection.alias
                );
                if args.dry_run {
                    continue;
                }
                if let Err(message) = push_one(&mut workspace, &inspection) {
                    eprintln!("{}: {message}", styles.err_label());
                    return ExitCode::from(2);
                }
            }
            Action::Conflict => unreachable!(),
        }
    }
    ExitCode::SUCCESS
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Clean,
    Pull,
    Push,
    Conflict,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Clean => "up to date",
            Self::Pull => "remote changed — pull",
            Self::Push => "local changed — push",
            Self::Conflict => "conflict",
        }
    }
}

struct Inspection {
    alias: String,
    spec: CollectionSpec,
    lock: Option<LockedCollection>,
    local_sha256: Option<String>,
    remote: Option<CollectionDescriptor>,
    action: Action,
    reason: Option<String>,
}

#[derive(Serialize)]
struct StatusRow<'a> {
    collection: &'a str,
    state: Action,
    path: String,
    uri: &'a str,
    local_sha256: Option<&'a str>,
    locked_remote_content_etag: Option<&'a str>,
    current_remote_content_etag: Option<&'a str>,
    reason: Option<&'a str>,
}

impl Inspection {
    fn row(&self) -> StatusRow<'_> {
        StatusRow {
            collection: &self.alias,
            state: self.action,
            path: self.spec.path.display().to_string(),
            uri: &self.spec.uri,
            local_sha256: self.local_sha256.as_deref(),
            locked_remote_content_etag: self
                .lock
                .as_ref()
                .map(|lock| lock.remote_content_etag.as_str()),
            current_remote_content_etag: self
                .remote
                .as_ref()
                .map(|remote| remote.content_etag.as_str()),
            reason: self.reason.as_deref(),
        }
    }
}

fn inspect(workspace: &Workspace, selected: &[String]) -> Result<Vec<Inspection>, String> {
    let aliases = selected_aliases(workspace, selected)?;
    let mut inspections = Vec::with_capacity(aliases.len());
    for alias in aliases {
        let (spec, lock) = workspace
            .collection(&alias)
            .ok_or_else(|| format!("unknown collection alias `{alias}`"))?;
        let local_state = workspace
            .local_state(&alias)
            .map_err(|error| error.to_string())?;
        let local_sha256 = if local_state == LocalState::Missing {
            None
        } else {
            Some(
                sha256_file(workspace.absolute_collection_path(spec))
                    .map_err(|error| error.to_string())?,
            )
        };
        let client = registry_client(Some(&spec.registry), Some(&spec.uri))?;
        let remote = match client.collection_descriptor(&spec.uri) {
            Ok(remote) => Some(remote),
            Err(RegistryError::HttpStatus { status: 404, .. }) => None,
            Err(error) => {
                return Err(format!(
                    "could not inspect remote collection `{alias}` at {}: {error}",
                    spec.uri
                ));
            }
        };
        let (action, reason) = classify(local_state, lock, remote.as_ref());
        inspections.push(Inspection {
            alias,
            spec: spec.clone(),
            lock: lock.cloned(),
            local_sha256,
            remote,
            action,
            reason,
        });
    }
    Ok(inspections)
}

fn classify(
    local: LocalState,
    lock: Option<&LockedCollection>,
    remote: Option<&CollectionDescriptor>,
) -> (Action, Option<String>) {
    let Some(lock) = lock else {
        return (
            Action::Conflict,
            Some("the collection has no lock entry; pull or push it explicitly".to_owned()),
        );
    };
    if local == LocalState::Missing {
        return (
            Action::Conflict,
            Some(
                "the tracked local file is missing; deletion is never propagated implicitly"
                    .to_owned(),
            ),
        );
    }
    let Some(remote) = remote else {
        return (
            Action::Conflict,
            Some("the tracked remote collection is missing; deletion or recreation requires an explicit command".to_owned()),
        );
    };
    let local_changed = local == LocalState::Modified;
    let remote_changed = remote.content_etag != lock.remote_content_etag;
    match (local_changed, remote_changed) {
        (false, false) => (Action::Clean, None),
        (true, false) => (Action::Push, None),
        (false, true) => (Action::Pull, None),
        (true, true) => (
            Action::Conflict,
            Some(
                "local and remote biological content both changed; automatic merge is disabled"
                    .to_owned(),
            ),
        ),
    }
}

fn pull_one(workspace: &mut Workspace, inspection: &Inspection) -> Result<(), String> {
    let format = collection_format(&inspection.spec.path)?;
    let client = registry_client(Some(&inspection.spec.registry), Some(&inspection.spec.uri))?;
    let pulled = client
        .pull_collection(&inspection.spec.uri, format)
        .map_err(|error| format!("failed to pull {}: {error}", inspection.spec.uri))?;
    validate_collection_rdf(&pulled.body, format)?;
    let path = workspace.absolute_collection_path(&inspection.spec);
    write_file_atomic(&path, &pulled.body).map_err(|error| error.to_string())?;
    workspace
        .record_sync(
            inspection.alias.clone(),
            inspection.spec.clone(),
            pulled.content_etag,
            sha256_bytes(&pulled.body),
        )
        .map_err(|error| error.to_string())
}

fn push_one(workspace: &mut Workspace, inspection: &Inspection) -> Result<(), String> {
    let lock = inspection
        .lock
        .as_ref()
        .ok_or_else(|| "tracked collection has no lock entry".to_owned())?;
    let path = workspace.absolute_collection_path(&inspection.spec);
    let body =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let current_hash = sha256_bytes(&body);
    if inspection.local_sha256.as_deref() != Some(current_hash.as_str()) {
        return Err(format!(
            "{} changed while synchronization was being planned; run `sbol status` again",
            path.display()
        ));
    }
    let format = collection_format(&inspection.spec.path)?;
    validate_collection_rdf(&body, format)?;
    let client = registry_client(Some(&inspection.spec.registry), Some(&inspection.spec.uri))?;
    let written = client
        .put_collection(
            &inspection.spec.uri,
            format,
            &body,
            CollectionPrecondition::Matches(&lock.remote_content_etag),
        )
        .map_err(|error| match error {
            RegistryError::PreconditionFailed { .. } => format!(
                "remote content changed before the push committed; no data was overwritten: {error}"
            ),
            _ => format!("failed to push {}: {error}", inspection.spec.uri),
        })?;
    workspace
        .record_sync(
            inspection.alias.clone(),
            inspection.spec.clone(),
            written.content_etag,
            current_hash,
        )
        .map_err(|error| error.to_string())
}

fn selected_aliases(workspace: &Workspace, selected: &[String]) -> Result<Vec<String>, String> {
    if selected.is_empty() {
        return Ok(workspace.manifest().collections.keys().cloned().collect());
    }
    let mut aliases = BTreeSet::new();
    for alias in selected {
        if !workspace.manifest().collections.contains_key(alias) {
            return Err(format!("unknown collection alias `{alias}`"));
        }
        aliases.insert(alias.clone());
    }
    Ok(aliases.into_iter().collect())
}

fn current_workspace() -> Result<Workspace, String> {
    let cwd = env::current_dir().map_err(|error| error.to_string())?;
    Workspace::discover(&cwd)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            "not inside an SBOL project; run `sbol init` in the project directory".to_owned()
        })
}

pub(crate) fn collection_format(path: &Path) -> Result<CollectionRdfFormat, String> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("ttl") => Ok(CollectionRdfFormat::Turtle),
        Some("rdf" | "xml") => Ok(CollectionRdfFormat::RdfXml),
        Some("jsonld") => Ok(CollectionRdfFormat::JsonLd),
        Some("nt") => Ok(CollectionRdfFormat::NTriples),
        _ => Err(format!(
            "tracked collection {} must use .ttl, .rdf, .xml, .jsonld, or .nt",
            path.display()
        )),
    }
}

pub(crate) fn validate_collection_rdf(
    body: &[u8],
    format: CollectionRdfFormat,
) -> Result<(), String> {
    let text = std::str::from_utf8(body)
        .map_err(|error| format!("collection RDF is not UTF-8: {error}"))?;
    let format = match format {
        CollectionRdfFormat::Turtle => RdfFormat::Turtle,
        CollectionRdfFormat::RdfXml => RdfFormat::RdfXml,
        CollectionRdfFormat::JsonLd => RdfFormat::JsonLd,
        CollectionRdfFormat::NTriples => RdfFormat::NTriples,
    };
    let report = match sbol::detect_version(text, format) {
        Some(SbolVersion::V2) => sbol::v2::Document::read(text, format)
            .map_err(|error| error.to_string())?
            .validate(),
        Some(SbolVersion::V3) => sbol::v3::Document::read(text, format)
            .map_err(|error| error.to_string())?
            .validate(),
        None => return Err("document is valid RDF but is not SBOL 2 or SBOL 3".to_owned()),
        Some(_) => return Err("unsupported SBOL version".to_owned()),
    };
    if report.has_errors() {
        Err(report.to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(etag: &str) -> LockedCollection {
        LockedCollection {
            uri: "urn:collection".to_owned(),
            registry: "https://sbol.io/".to_owned(),
            path: "designs/toggle.ttl".into(),
            remote_content_etag: etag.to_owned(),
            local_sha256: "a".repeat(64),
        }
    }

    fn remote(etag: &str) -> CollectionDescriptor {
        CollectionDescriptor {
            iri: "urn:collection".to_owned(),
            content_url: "/content".to_owned(),
            content_etag: etag.to_owned(),
            triple_count: 1,
            display_id: Some("toggle".to_owned()),
        }
    }

    #[test]
    fn synchronization_matrix_is_explicit() {
        let locked = lock(r#""one""#);
        assert_eq!(
            classify(
                LocalState::Unchanged,
                Some(&locked),
                Some(&remote(r#""one""#)),
            )
            .0,
            Action::Clean
        );
        assert_eq!(
            classify(
                LocalState::Modified,
                Some(&locked),
                Some(&remote(r#""one""#)),
            )
            .0,
            Action::Push
        );
        assert_eq!(
            classify(
                LocalState::Unchanged,
                Some(&locked),
                Some(&remote(r#""two""#)),
            )
            .0,
            Action::Pull
        );
        assert_eq!(
            classify(
                LocalState::Modified,
                Some(&locked),
                Some(&remote(r#""two""#)),
            )
            .0,
            Action::Conflict
        );
        assert_eq!(
            classify(
                LocalState::Missing,
                Some(&locked),
                Some(&remote(r#""one""#)),
            )
            .0,
            Action::Conflict
        );
        assert_eq!(
            classify(LocalState::Unchanged, Some(&locked), None).0,
            Action::Conflict
        );
    }
}
