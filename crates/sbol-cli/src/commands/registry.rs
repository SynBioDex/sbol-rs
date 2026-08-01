mod credentials;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use sbol::v3::{Document, RdfFormat};
use sbol_registry_client::{
    CollisionPolicy, RegistryClient, SbolVersion, SubmissionConsequence, SubmissionCreated,
    SubmissionFormat, SubmissionPreview, SubmissionRequest,
};

use crate::cli::{
    RegistryCollisionPolicy, RegistryCommand, RegistryLoginArgs, RegistryPullArgs,
    RegistryPushArgs, RegistryStatusArgs,
};
use crate::output::infer_conversion_rdf_format;
use crate::style::Styles;
use credentials::CredentialStore;

pub(crate) fn registry(command: RegistryCommand, styles: Styles) -> ExitCode {
    match command {
        RegistryCommand::Login(args) => login(args, styles),
        RegistryCommand::Pull(args) => pull(args, styles),
        RegistryCommand::Push(args) => push(args, styles),
        RegistryCommand::Status(args) => status(args, styles),
    }
}

fn login(args: RegistryLoginArgs, styles: Styles) -> ExitCode {
    let client = match registry_base_client(args.registry.as_deref(), None, true) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let identifier = match args.identifier {
        Some(identifier) if !identifier.trim().is_empty() => identifier,
        _ => match prompt_line("Username or email: ") {
            Ok(identifier) if !identifier.is_empty() => identifier,
            Ok(_) => {
                eprintln!("{}: username or email is required", styles.err_label());
                return ExitCode::from(2);
            }
            Err(error) => {
                eprintln!(
                    "{}: failed to read username or email: {error}",
                    styles.err_label()
                );
                return ExitCode::from(2);
            }
        },
    };
    let password = if args.password_stdin {
        match read_password_stdin() {
            Ok(password) => password,
            Err(error) => {
                eprintln!(
                    "{}: failed to read password from standard input: {error}",
                    styles.err_label()
                );
                return ExitCode::from(2);
            }
        }
    } else {
        match rpassword::prompt_password("Password: ") {
            Ok(password) => password,
            Err(error) => {
                eprintln!(
                    "{}: failed to read a hidden password: {error}; use --password-stdin for controlled non-interactive login",
                    styles.err_label()
                );
                return ExitCode::from(2);
            }
        }
    };
    if password.is_empty() {
        eprintln!("{}: password is required", styles.err_label());
        return ExitCode::from(2);
    }

    let token = match client.password_login(identifier.trim(), &password) {
        Ok(token) => token,
        Err(error) => {
            eprintln!(
                "{}: sign in to {} failed: {error}",
                styles.err_label(),
                client.base_url()
            );
            return ExitCode::from(2);
        }
    };
    let store = match CredentialStore::discover() {
        Ok(store) => store,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    if let Err(message) = store.save_access_token(client.base_url().as_str(), token) {
        eprintln!("{}: {message}", styles.err_label());
        return ExitCode::from(2);
    }

    println!("signed in to {}", client.base_url());
    ExitCode::SUCCESS
}

fn pull(args: RegistryPullArgs, styles: Styles) -> ExitCode {
    let target_format = match infer_conversion_rdf_format(&args.output) {
        Some(format) => format,
        None => {
            let extension = args
                .output
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("<none>");
            eprintln!(
                "{}: unsupported output extension `{extension}` for {} — supported: .ttl, .rdf, .xml, .jsonld, .nt",
                styles.err_label(),
                args.output.display()
            );
            return ExitCode::from(2);
        }
    };

    let client = match registry_client(args.registry.as_deref(), Some(&args.iri)) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };

    let pulled = match client.pull(&args.iri, SbolVersion::V3) {
        Ok(pulled) => pulled,
        Err(error) => {
            eprintln!(
                "{}: failed to pull {}: {error}",
                styles.err_label(),
                args.iri
            );
            return ExitCode::from(2);
        }
    };
    let input = match String::from_utf8(pulled.body) {
        Ok(input) => input,
        Err(error) => {
            eprintln!(
                "{}: registry returned non-UTF-8 SBOL RDF from {}: {error}",
                styles.err_label(),
                pulled.source_url
            );
            return ExitCode::from(2);
        }
    };
    let document = match Document::read(&input, RdfFormat::RdfXml) {
        Ok(document) => document,
        Err(error) => {
            let content_type = pulled.content_type.as_deref().unwrap_or("unknown");
            eprintln!(
                "{}: registry response from {} did not parse as SBOL 3 RDF/XML (content-type {content_type}): {error}",
                styles.err_label(),
                pulled.source_url
            );
            return ExitCode::from(2);
        }
    };
    let output = match document.write(target_format) {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "{}: failed to serialize {} as {target_format}: {error}",
                styles.err_label(),
                args.output.display()
            );
            return ExitCode::from(2);
        }
    };
    if let Err(error) = fs::write(&args.output, output) {
        eprintln!(
            "{}: failed to write {}: {error}",
            styles.err_label(),
            args.output.display()
        );
        return ExitCode::from(2);
    }

    eprintln!(
        "pulled {} from {} to {}",
        args.iri,
        client.base_url(),
        args.output.display()
    );
    ExitCode::SUCCESS
}

fn push(args: RegistryPushArgs, styles: Styles) -> ExitCode {
    let format = match submission_format(&args.input) {
        Ok(format) => format,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let content = match fs::read_to_string(&args.input) {
        Ok(content) => content,
        Err(error) => {
            eprintln!(
                "{}: failed to read {}: {error}",
                styles.err_label(),
                args.input.display()
            );
            return ExitCode::from(2);
        }
    };
    let id = match args.id {
        Some(id) if valid_display_id(&id) => id,
        Some(id) => {
            eprintln!(
                "{}: collection id `{id}` must start with an ASCII letter or underscore and contain only letters, digits, or underscores",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
        None => match inferred_submission_id(&args.input) {
            Ok(id) => id,
            Err(message) => {
                eprintln!("{}: {message}", styles.err_label());
                return ExitCode::from(2);
            }
        },
    };
    if args.collection_version.is_empty()
        || !args
            .collection_version
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        eprintln!(
            "{}: version must contain only ASCII letters, digits, dots, underscores, or hyphens",
            styles.err_label()
        );
        return ExitCode::from(2);
    }

    let client = match registry_client(args.registry.as_deref(), None) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let request = SubmissionRequest {
        id,
        version: args.collection_version,
        name: args.name,
        description: args.description,
        citations: args.citations,
        creator_name: args.creator_name,
        format,
        overwrite: args.collision.into(),
        content,
    };
    let preview = match client.preview_submission(&request) {
        Ok(preview) => preview,
        Err(error) => {
            eprintln!(
                "{}: registry rejected validation for {}: {error}",
                styles.err_label(),
                args.input.display()
            );
            return ExitCode::from(2);
        }
    };

    if preview.consequence == SubmissionConsequence::RejectConflict {
        render_push_result(&args.input, &preview, None, args.json);
        eprintln!(
            "{}: target collection already exists; choose --collision replace or --collision merge explicitly",
            styles.err_label()
        );
        return ExitCode::from(1);
    }
    if args.preview {
        render_push_result(&args.input, &preview, None, args.json);
        return ExitCode::SUCCESS;
    }

    let created = match client.create_submission(&request) {
        Ok(created) => created,
        Err(error) => {
            eprintln!(
                "{}: failed to push {} after successful validation: {error}",
                styles.err_label(),
                args.input.display()
            );
            return ExitCode::from(2);
        }
    };
    render_push_result(&args.input, &preview, Some(&created), args.json);
    ExitCode::SUCCESS
}

fn status(args: RegistryStatusArgs, styles: Styles) -> ExitCode {
    let client = match registry_client(args.registry.as_deref(), None) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let instance = match client.instance() {
        Ok(instance) => instance,
        Err(error) => {
            eprintln!(
                "{}: failed to inspect {}: {error}",
                styles.err_label(),
                client.base_url()
            );
            return ExitCode::from(2);
        }
    };

    if args.json {
        match serde_json::to_string_pretty(&instance) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!(
                    "{}: failed to render registry metadata: {error}",
                    styles.err_label()
                );
                return ExitCode::from(2);
            }
        }
    } else {
        println!("{}", instance.name);
        println!("  registry: {}", client.base_url());
        if !instance.instance_url.is_empty() {
            println!("  canonical: {}", instance.instance_url);
        }
        println!("  namespace: {}", instance.uri_prefix);
        println!(
            "  access: {}",
            if instance.policies.require_login {
                "login required"
            } else {
                "public browsing"
            }
        );
        if let Some(machine) = instance.machine_access {
            println!("  api: {}", machine.api_url);
            if let Some(mcp) = machine.mcp_url {
                println!("  mcp: {mcp}");
            }
            if let Some(issuer) = machine.authorization_issuer {
                println!("  identity: {issuer}");
            }
        }
    }
    ExitCode::SUCCESS
}

fn registry_client(
    explicit: Option<&str>,
    design_iri: Option<&str>,
) -> Result<RegistryClient, String> {
    let client = registry_base_client(explicit, design_iri, false)?;
    if let Some(token) = env::var("SBOL_ACCESS_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(client.with_bearer_token(token));
    }
    let store = match CredentialStore::discover() {
        Ok(store) => store,
        Err(_) => return Ok(client),
    };
    match store.token_for(client.base_url().as_str())? {
        Some(token) => Ok(client.with_bearer_token(token)),
        None => Ok(client),
    }
}

fn registry_base_client(
    explicit: Option<&str>,
    design_iri: Option<&str>,
    default_to_sbol_io: bool,
) -> Result<RegistryClient, String> {
    let client = if let Some(registry) = explicit {
        RegistryClient::new(registry).map_err(|error| error.to_string())?
    } else if let Some(registry) = env::var("SBOL_REGISTRY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        RegistryClient::new(registry).map_err(|error| error.to_string())?
    } else if let Some(iri) = design_iri {
        RegistryClient::from_design_iri(iri).map_err(|error| error.to_string())?
    } else if default_to_sbol_io {
        RegistryClient::new("https://sbol.io").map_err(|error| error.to_string())?
    } else if let Some(registry) = active_registry()? {
        RegistryClient::new(registry).map_err(|error| error.to_string())?
    } else {
        return Err("provide a registry URL or set SBOL_REGISTRY_URL".to_owned());
    };
    Ok(client)
}

fn active_registry() -> Result<Option<String>, String> {
    match CredentialStore::discover() {
        Ok(store) => store.active_registry(),
        Err(_) => Ok(None),
    }
}

fn prompt_line(prompt: &str) -> io::Result<String> {
    eprint!("{prompt}");
    io::stderr().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_owned())
}

fn read_password_stdin() -> io::Result<String> {
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    Ok(password.trim_end_matches(['\r', '\n']).to_owned())
}

impl From<RegistryCollisionPolicy> for CollisionPolicy {
    fn from(value: RegistryCollisionPolicy) -> Self {
        match value {
            RegistryCollisionPolicy::Fail => Self::Fail,
            RegistryCollisionPolicy::Replace => Self::Replace,
            RegistryCollisionPolicy::Merge => Self::Merge,
        }
    }
}

fn submission_format(path: &Path) -> Result<SubmissionFormat, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match extension.as_str() {
        "rdf" | "xml" => Ok(SubmissionFormat::RdfXml),
        "ttl" => Ok(SubmissionFormat::Turtle),
        "jsonld" => Ok(SubmissionFormat::JsonLd),
        "nt" => Ok(SubmissionFormat::NTriples),
        "gb" | "gbk" => Ok(SubmissionFormat::GenBank),
        "fasta" | "fa" | "fna" | "faa" => Ok(SubmissionFormat::Fasta),
        _ => Err(format!(
            "cannot infer upload format from {} — supported: .ttl, .rdf, .xml, .jsonld, .nt, .gb, .gbk, .fasta, .fa, .fna, .faa",
            path.display()
        )),
    }
}

fn inferred_submission_id(path: &Path) -> Result<String, String> {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "cannot infer a collection id from {}; provide --id",
                path.display()
            )
        })?;
    let mut id: String = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if !id
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
    {
        id.insert(0, '_');
    }
    Ok(id)
}

fn valid_display_id(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn render_push_result(
    input: &Path,
    preview: &SubmissionPreview,
    created: Option<&SubmissionCreated>,
    json_output: bool,
) {
    if json_output {
        let value = serde_json::json!({
            "preview": preview,
            "created": created,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("registry result is JSON-serializable")
        );
        return;
    }

    println!("validated {}", input.display());
    println!("  target: {}", preview.collection_uri);
    println!("  members: {}", preview.members.len());
    println!("  triples: {}", preview.triple_count);
    println!("  change: {}", consequence_name(preview.consequence));
    for notice in &preview.notices {
        println!("  {}: {}", notice.code, notice.message);
    }
    if let Some(created) = created {
        println!("pushed {}", created.collection_uri);
    } else {
        println!("preview only; no registry data was changed");
    }
}

fn consequence_name(consequence: SubmissionConsequence) -> &'static str {
    match consequence {
        SubmissionConsequence::Create => "create",
        SubmissionConsequence::RejectConflict => "reject conflict",
        SubmissionConsequence::Replace => "replace",
        SubmissionConsequence::Merge => "merge",
    }
}
