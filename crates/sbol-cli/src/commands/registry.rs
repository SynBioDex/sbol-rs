mod credentials;

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::random;
use sbol::v3::{Document, RdfFormat};
use sbol_registry_client::{
    AuthorizationServerMetadata, CollectionRdfFormat, CollisionPolicy, RegistryClient,
    RegistryError, SbolVersion, SubmissionConsequence, SubmissionCreated, SubmissionFormat,
    SubmissionPreview, SubmissionRequest,
};
use sbol_workspace::{
    CollectionSpec, LocalState, LockedCollection, Workspace, safe_collection_name, sha256_bytes,
    write_file_atomic,
};
use sha2::{Digest, Sha256};
use url::Url;

use crate::cli::{
    RegistryCollisionPolicy, RegistryCommand, RegistryLoginArgs, RegistryLogoutArgs,
    RegistryPullArgs, RegistryPushArgs, RegistryStatusArgs,
};
use crate::commands::workspace::{collection_format, validate_collection_rdf};
use crate::output::infer_conversion_rdf_format;
use crate::style::Styles;
use credentials::CredentialStore;

pub(crate) fn registry(command: RegistryCommand, styles: Styles) -> ExitCode {
    match command {
        RegistryCommand::Login(args) => login(args, styles),
        RegistryCommand::Logout(args) => logout(args, styles),
        RegistryCommand::Pull(args) => pull(args, styles),
        RegistryCommand::Push(args) => push(args, styles),
        RegistryCommand::Status(args) => status(args, styles),
    }
}

fn logout(args: RegistryLogoutArgs, styles: Styles) -> ExitCode {
    let client = match registry_base_client(args.registry.as_deref(), None, false) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
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
    let credential = match store.credential_for(client.base_url().as_str()) {
        Ok(Some(credential)) => credential,
        Ok(None) => {
            println!("not signed in to {}", client.base_url());
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };

    let remote = if let Some(issuer) = credential.issuer.as_deref() {
        revoke_oauth_credential(&client, issuer, &credential)
    } else {
        client
            .clone()
            .with_bearer_token(credential.access_token.clone())
            .logout_compatibility_session()
            .map_err(|error| error.to_string())
    };
    if let Err(message) = remote {
        eprintln!(
            "{}: could not revoke the remote credential ({message}); removing the local copy",
            styles.warn_label()
        );
    }
    if let Err(message) = store.remove(client.base_url().as_str()) {
        eprintln!("{}: {message}", styles.err_label());
        return ExitCode::from(2);
    }
    println!("signed out of {}", client.base_url());
    ExitCode::SUCCESS
}

fn revoke_oauth_credential(
    client: &RegistryClient,
    issuer: &str,
    credential: &credentials::StoredCredential,
) -> Result<(), String> {
    let metadata = client
        .authorization_server(issuer)
        .map_err(|error| error.to_string())?;
    let mut failures = Vec::new();
    if let Some(refresh_token) = credential.refresh_token.as_deref()
        && let Err(error) = client.revoke_oauth_token(&metadata, refresh_token)
    {
        failures.push(error.to_string());
    }
    if let Err(error) = client.revoke_oauth_token(&metadata, &credential.access_token) {
        failures.push(error.to_string());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
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
    if args.identifier.is_some() || args.password_stdin {
        return password_login(client, args, styles);
    }

    let instance = match client.instance() {
        Ok(instance) => instance,
        Err(error) => {
            eprintln!(
                "{}: could not discover SBOL Identity from {}: {error}; compatibility login must be requested explicitly with --identifier or --password-stdin",
                styles.err_label(),
                client.base_url()
            );
            return ExitCode::from(2);
        }
    };
    let Some(machine_access) = instance.machine_access else {
        eprintln!(
            "{}: {} does not advertise machine-access authentication; compatibility login must be requested explicitly with --identifier or --password-stdin",
            styles.err_label(),
            client.base_url()
        );
        return ExitCode::from(2);
    };
    let Some(issuer) = machine_access.authorization_issuer else {
        eprintln!(
            "{}: {} does not advertise an SBOL Identity issuer; compatibility login must be requested explicitly with --identifier or --password-stdin",
            styles.err_label(),
            client.base_url()
        );
        return ExitCode::from(2);
    };
    oauth_login(
        &client,
        &issuer,
        &machine_access.api_url,
        args.no_browser,
        styles,
    )
}

fn password_login(client: RegistryClient, args: RegistryLoginArgs, styles: Styles) -> ExitCode {
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

fn oauth_login(
    client: &RegistryClient,
    issuer: &str,
    resource: &str,
    no_browser: bool,
    styles: Styles,
) -> ExitCode {
    let metadata = match client.authorization_server(issuer) {
        Ok(metadata) => metadata,
        Err(error) => {
            eprintln!(
                "{}: could not discover SBOL Identity at {issuer}: {error}",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
    };
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "{}: could not open a loopback callback for SBOL Identity: {error}",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
    };
    let port = match listener.local_addr() {
        Ok(address) => address.port(),
        Err(error) => {
            eprintln!(
                "{}: could not inspect the loopback callback: {error}",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
    };
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let registration =
        match client.register_public_oauth_client(&metadata, "sbol CLI", &redirect_uri) {
            Ok(registration) => registration,
            Err(error) => {
                eprintln!(
                    "{}: could not register the sbol CLI with SBOL Identity: {error}",
                    styles.err_label()
                );
                return ExitCode::from(2);
            }
        };
    let verifier = random_urlsafe();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_urlsafe();
    let authorization_url = match authorization_url(
        &metadata,
        &registration.client_id,
        &redirect_uri,
        resource,
        &challenge,
        &state,
    ) {
        Ok(url) => url,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };

    eprintln!("Sign in with SBOL in your browser:");
    eprintln!("{authorization_url}");
    if !no_browser && let Err(error) = open_browser(authorization_url.as_str()) {
        eprintln!(
            "{}: could not open a browser automatically ({error}); open the URL above",
            styles.warn_label()
        );
    }
    let code = match await_oauth_callback(listener, &state, Duration::from_secs(300)) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let tokens = match client.exchange_oauth_code(
        &metadata,
        &registration.client_id,
        &redirect_uri,
        resource,
        &code,
        &verifier,
    ) {
        Ok(tokens) if tokens.resource == resource => tokens,
        Ok(_) => {
            eprintln!(
                "{}: SBOL Identity returned a token for the wrong registry resource",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
        Err(error) => {
            eprintln!(
                "{}: SBOL Identity token exchange failed: {error}",
                styles.err_label()
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
    if let Err(message) = store.save_oauth_tokens(
        client.base_url().as_str(),
        tokens.access_token,
        tokens.refresh_token,
        tokens.expires_in,
        metadata.issuer,
        registration.client_id,
        resource.to_owned(),
    ) {
        eprintln!("{}: {message}", styles.err_label());
        return ExitCode::from(2);
    }
    println!("signed in to {} with SBOL Identity", client.base_url());
    ExitCode::SUCCESS
}

fn authorization_url(
    metadata: &AuthorizationServerMetadata,
    client_id: &str,
    redirect_uri: &str,
    resource: &str,
    challenge: &str,
    state: &str,
) -> Result<Url, String> {
    let mut url = Url::parse(&metadata.authorization_endpoint)
        .map_err(|error| format!("invalid SBOL Identity authorization endpoint: {error}"))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("resource", resource)
        .append_pair("scope", "sbol:read sbol:write")
        .append_pair("state", state);
    Ok(url)
}

fn random_urlsafe() -> String {
    URL_SAFE_NO_PAD.encode(random::<[u8; 32]>())
}

fn await_oauth_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("could not configure the OAuth callback: {error}"))?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => match read_callback(&mut stream, expected_state) {
                Ok(Some(code)) => return Ok(code),
                Ok(None) => continue,
                Err(message) => return Err(message),
            },
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(format!("SBOL Identity callback failed: {error}")),
        }
    }
    Err("timed out after five minutes waiting for Sign in with SBOL".to_owned())
}

fn read_callback(stream: &mut TcpStream, expected_state: &str) -> Result<Option<String>, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    const MAX_CALLBACK_BYTES: usize = 16 * 1024;
    let mut bytes = Vec::with_capacity(1024);
    while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
        if bytes.len() == MAX_CALLBACK_BYTES {
            return Err("SBOL Identity callback headers were too large".to_owned());
        }
        let mut chunk = [0_u8; 1024];
        let remaining = MAX_CALLBACK_BYTES - bytes.len();
        let chunk_length = remaining.min(chunk.len());
        let length = stream
            .read(&mut chunk[..chunk_length])
            .map_err(|error| format!("could not read the SBOL Identity callback: {error}"))?;
        if length == 0 {
            return Err("SBOL Identity callback ended before its headers were complete".to_owned());
        }
        bytes.extend_from_slice(&chunk[..length]);
    }
    let request = std::str::from_utf8(&bytes)
        .map_err(|_| "SBOL Identity callback was not valid HTTP".to_owned())?;
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_ascii_whitespace().nth(1))
        .ok_or_else(|| "SBOL Identity callback was missing its request target".to_owned())?;
    let url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "SBOL Identity callback URL was malformed".to_owned())?;
    if url.path() != "/callback" {
        write_callback_response(stream, "404 Not Found", "Not found");
        return Ok(None);
    }
    let values = url
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    if values.get("state").map(|value| value.as_ref()) != Some(expected_state) {
        write_callback_response(
            stream,
            "400 Bad Request",
            "The sign-in state did not match.",
        );
        return Err("SBOL Identity callback state did not match; sign-in was cancelled".to_owned());
    }
    if let Some(error) = values.get("error") {
        write_callback_response(
            stream,
            "400 Bad Request",
            "Sign in with SBOL was cancelled.",
        );
        let description = values
            .get("error_description")
            .map(|value| value.as_ref())
            .unwrap_or(error.as_ref());
        return Err(format!("SBOL Identity authorization failed: {description}"));
    }
    let code = values
        .get("code")
        .filter(|code| !code.is_empty())
        .map(|code| code.to_string())
        .ok_or_else(|| "SBOL Identity callback did not include an authorization code".to_owned())?;
    write_callback_response(
        stream,
        "200 OK",
        "Sign in complete. You can close this window and return to the sbol CLI.",
    );
    Ok(Some(code))
}

fn write_callback_response(stream: &mut TcpStream, status: &str, message: &str) {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>SBOL Identity</title></head><body><main><h1>SBOL Identity</h1><p>{message}</p></main></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn open_browser(url: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let status = ProcessCommand::new("open").arg(url).status()?;
    #[cfg(target_os = "linux")]
    let status = ProcessCommand::new("xdg-open").arg(url).status()?;
    #[cfg(target_os = "windows")]
    let status = ProcessCommand::new("cmd")
        .args(["/C", "start", "", url])
        .status()?;
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "automatic browser launch is unsupported on this platform",
    ));
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("browser launcher returned an error"))
    }
}

fn pull(args: RegistryPullArgs, styles: Styles) -> ExitCode {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!(
                "{}: could not determine the current directory: {error}",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
    };
    let workspace = match Workspace::discover(&cwd) {
        Ok(workspace) => workspace,
        Err(error) => {
            eprintln!(
                "{}: could not inspect the SBOL project: {error}",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
    };

    match (workspace, args.output.as_ref(), args.alias.as_ref()) {
        (Some(workspace), None, _) | (Some(workspace), Some(_), Some(_)) => {
            pull_tracked(args, workspace, &cwd, styles)
        }
        (None, None, _) => {
            eprintln!(
                "{}: --output is required outside an SBOL project; run `sbol init` to track collection synchronization",
                styles.err_label()
            );
            ExitCode::from(2)
        }
        (None, Some(_), Some(_)) => {
            eprintln!(
                "{}: --alias can only be used inside an SBOL project",
                styles.err_label()
            );
            ExitCode::from(2)
        }
        (_, Some(output), None) => pull_one_shot(&args, output, styles),
    }
}

fn pull_one_shot(args: &RegistryPullArgs, output_path: &Path, styles: Styles) -> ExitCode {
    let target_format = match infer_conversion_rdf_format(output_path) {
        Some(format) => format,
        None => {
            let extension = output_path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("<none>");
            eprintln!(
                "{}: unsupported output extension `{extension}` for {} — supported: .ttl, .rdf, .xml, .jsonld, .nt",
                styles.err_label(),
                output_path.display()
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
                output_path.display()
            );
            return ExitCode::from(2);
        }
    };
    if let Err(error) = write_file_atomic(output_path, output.as_bytes()) {
        eprintln!(
            "{}: failed to write {}: {error}",
            styles.err_label(),
            output_path.display()
        );
        return ExitCode::from(2);
    }

    eprintln!(
        "pulled {} from {} to {}",
        args.iri,
        client.base_url(),
        output_path.display()
    );
    ExitCode::SUCCESS
}

fn pull_tracked(
    args: RegistryPullArgs,
    mut workspace: Workspace,
    cwd: &Path,
    styles: Styles,
) -> ExitCode {
    let registry = args
        .registry
        .as_deref()
        .or(workspace.manifest().default_registry.as_deref());
    let client = match registry_client(registry, Some(&args.iri)) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let descriptor = match client.collection_descriptor(&args.iri) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            eprintln!(
                "{}: failed to inspect collection {}: {error}",
                styles.err_label(),
                args.iri
            );
            return ExitCode::from(2);
        }
    };

    let existing = workspace
        .collection_by_uri(&descriptor.iri)
        .map(|(alias, spec, lock)| (alias.to_owned(), spec.clone(), lock.cloned()));
    let (alias, spec, prior_lock) = if let Some((alias, spec, lock)) = existing {
        if let Some(requested) = args.alias.as_deref()
            && requested != alias
        {
            eprintln!(
                "{}: collection {} is already tracked as `{alias}`",
                styles.err_label(),
                descriptor.iri
            );
            return ExitCode::from(2);
        }
        if let Some(output) = args.output.as_deref() {
            let requested = match workspace_relative_path(&workspace, cwd, output) {
                Ok(path) => path,
                Err(message) => {
                    eprintln!("{}: {message}", styles.err_label());
                    return ExitCode::from(2);
                }
            };
            if requested != spec.path {
                eprintln!(
                    "{}: collection `{alias}` is already tracked at {}; edit sbol.toml intentionally to move it",
                    styles.err_label(),
                    spec.path.display()
                );
                return ExitCode::from(2);
            }
        }
        (alias, spec, lock)
    } else {
        let preferred = args.alias.clone().unwrap_or_else(|| {
            safe_collection_name(&descriptor.iri, descriptor.display_id.as_deref())
        });
        let alias = if args.alias.is_some() {
            if workspace.manifest().collections.contains_key(&preferred) {
                eprintln!(
                    "{}: collection alias `{preferred}` is already in use",
                    styles.err_label()
                );
                return ExitCode::from(2);
            }
            preferred
        } else {
            workspace.next_alias(&preferred)
        };
        let path = match args.output.as_deref() {
            Some(output) => match workspace_relative_path(&workspace, cwd, output) {
                Ok(path) => path,
                Err(message) => {
                    eprintln!("{}: {message}", styles.err_label());
                    return ExitCode::from(2);
                }
            },
            None => workspace
                .manifest()
                .designs_dir
                .join(format!("{alias}.ttl")),
        };
        (
            alias,
            CollectionSpec {
                uri: descriptor.iri.clone(),
                registry: client.base_url().to_string(),
                path,
            },
            None,
        )
    };
    if let Err(message) = validate_tracking_path(&workspace, &spec.path) {
        eprintln!("{}: {message}", styles.err_label());
        return ExitCode::from(2);
    }

    if prior_lock.is_some() {
        match workspace.local_state(&alias) {
            Ok(LocalState::Modified) => {
                eprintln!(
                    "{}: {} has local changes; refusing to overwrite them. Run `sbol status` and resolve the synchronization state explicitly",
                    styles.err_label(),
                    workspace.absolute_collection_path(&spec).display()
                );
                return ExitCode::from(1);
            }
            Ok(_) => {}
            Err(error) => {
                eprintln!("{}: {error}", styles.err_label());
                return ExitCode::from(2);
            }
        }
    } else if workspace.absolute_collection_path(&spec).exists() {
        eprintln!(
            "{}: {} already exists and is not tracked; refusing to overwrite it",
            styles.err_label(),
            workspace.absolute_collection_path(&spec).display()
        );
        return ExitCode::from(1);
    }

    let format = match collection_format(&spec.path) {
        Ok(format) => format,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let pulled = match client.pull_collection(&descriptor.iri, format) {
        Ok(pulled) => pulled,
        Err(error) => {
            eprintln!(
                "{}: failed to pull collection {}: {error}",
                styles.err_label(),
                descriptor.iri
            );
            return ExitCode::from(2);
        }
    };
    if let Err(message) = validate_collection_rdf(&pulled.body, format) {
        eprintln!(
            "{}: registry returned invalid SBOL collection content: {message}",
            styles.err_label()
        );
        return ExitCode::from(2);
    }
    let destination = workspace.absolute_collection_path(&spec);
    if let Err(error) = write_file_atomic(&destination, &pulled.body) {
        eprintln!(
            "{}: failed to write {}: {error}",
            styles.err_label(),
            destination.display()
        );
        return ExitCode::from(2);
    }
    if let Err(error) = workspace.record_sync(
        alias.clone(),
        spec,
        pulled.content_etag,
        sha256_bytes(&pulled.body),
    ) {
        eprintln!(
            "{}: failed to update the SBOL project: {error}",
            styles.err_label()
        );
        return ExitCode::from(2);
    }
    eprintln!(
        "pulled {} as `{alias}` to {}",
        descriptor.iri,
        destination.display()
    );
    ExitCode::SUCCESS
}

fn workspace_relative_path(
    workspace: &Workspace,
    cwd: &Path,
    requested: &Path,
) -> Result<PathBuf, String> {
    let absolute = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        cwd.join(requested)
    };
    absolute
        .strip_prefix(workspace.root())
        .map(Path::to_path_buf)
        .map_err(|_| {
            format!(
                "tracked collection path {} must be inside the SBOL project at {}",
                absolute.display(),
                workspace.root().display()
            )
        })
}

fn validate_tracking_path(workspace: &Workspace, path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || !path.starts_with(&workspace.manifest().designs_dir)
    {
        return Err(format!(
            "tracked collection path {} must be a normalized relative path inside {}",
            path.display(),
            workspace.manifest().designs_dir.display()
        ));
    }
    Ok(())
}

fn push(args: RegistryPushArgs, styles: Styles) -> ExitCode {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!(
                "{}: could not determine the current directory: {error}",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
    };
    let workspace = match Workspace::discover(&cwd) {
        Ok(workspace) => workspace,
        Err(error) => {
            eprintln!(
                "{}: could not inspect the SBOL project: {error}",
                styles.err_label()
            );
            return ExitCode::from(2);
        }
    };
    if let Some(workspace) = workspace.as_ref()
        && let Some((alias, spec, lock)) = tracked_input(workspace, &cwd, &args.input)
    {
        return push_tracked(args, workspace.clone(), alias, spec, lock, styles);
    }

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

    let registry = args.registry.as_deref().or(workspace
        .as_ref()
        .and_then(|workspace| workspace.manifest().default_registry.as_deref()));
    let client = match registry_client(registry, None) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let tracking_plan = workspace
        .as_ref()
        .map(|workspace| plan_created_tracking(workspace, &cwd, &args.input, &id));
    let tracking_plan = match tracking_plan.transpose() {
        Ok(plan) => plan,
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
    if args.dry_run {
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
    if let (Some(mut workspace), Some((alias, path, rdf_format))) = (workspace, tracking_plan)
        && let Err(message) =
            track_created_collection(&mut workspace, &client, &created, alias, path, rdf_format)
    {
        eprintln!(
            "{}: collection {} was created, but the local project could not be synchronized: {message}\nrecover with `sbol registry pull {}`",
            styles.err_label(),
            created.collection_uri,
            created.collection_uri
        );
        return ExitCode::from(2);
    }
    render_push_result(&args.input, &preview, Some(&created), args.json);
    ExitCode::SUCCESS
}

fn tracked_input(
    workspace: &Workspace,
    cwd: &Path,
    input: &Path,
) -> Option<(String, CollectionSpec, Option<LockedCollection>)> {
    let input = if input.is_absolute() {
        input.to_path_buf()
    } else {
        cwd.join(input)
    };
    let input = fs::canonicalize(&input).unwrap_or(input);
    workspace
        .manifest()
        .collections
        .iter()
        .find_map(|(alias, spec)| {
            let tracked = workspace.absolute_collection_path(spec);
            let tracked = fs::canonicalize(&tracked).unwrap_or(tracked);
            (tracked == input).then(|| {
                (
                    alias.clone(),
                    spec.clone(),
                    workspace.lockfile().collections.get(alias).cloned(),
                )
            })
        })
}

fn push_tracked(
    args: RegistryPushArgs,
    mut workspace: Workspace,
    alias: String,
    spec: CollectionSpec,
    lock: Option<LockedCollection>,
    styles: Styles,
) -> ExitCode {
    let Some(lock) = lock else {
        eprintln!(
            "{}: collection `{alias}` has no sbol.lock entry; establish a baseline with an explicit pull",
            styles.err_label()
        );
        return ExitCode::from(1);
    };
    let client = match registry_client(Some(&spec.registry), Some(&spec.uri)) {
        Ok(client) => client,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    if let Some(explicit) = args.registry.as_deref() {
        match RegistryClient::new(explicit) {
            Ok(explicit_client) if explicit_client.base_url() == client.base_url() => {}
            Ok(_) => {
                eprintln!(
                    "{}: `{alias}` is bound to {}; a tracked push cannot target another registry",
                    styles.err_label(),
                    client.base_url()
                );
                return ExitCode::from(2);
            }
            Err(error) => {
                eprintln!("{}: {error}", styles.err_label());
                return ExitCode::from(2);
            }
        }
    }
    let format = match collection_format(&spec.path) {
        Ok(format) => format,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };
    let body = match fs::read(&args.input) {
        Ok(body) => body,
        Err(error) => {
            eprintln!(
                "{}: failed to read {}: {error}",
                styles.err_label(),
                args.input.display()
            );
            return ExitCode::from(2);
        }
    };
    if let Err(message) = validate_collection_rdf(&body, format) {
        eprintln!(
            "{}: collection validation failed: {message}",
            styles.err_label()
        );
        return ExitCode::from(2);
    }
    let remote = match client.collection_descriptor(&spec.uri) {
        Ok(remote) => remote,
        Err(error) => {
            eprintln!(
                "{}: failed to inspect tracked collection {}: {error}",
                styles.err_label(),
                spec.uri
            );
            return ExitCode::from(2);
        }
    };
    if remote.content_etag != lock.remote_content_etag {
        eprintln!(
            "{}: remote biological content changed since the last sync; no data was overwritten. Run `sbol status` before resolving the conflict",
            styles.err_label()
        );
        return ExitCode::from(1);
    }
    if args.dry_run {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "action": "update",
                    "collection": alias,
                    "collection_uri": spec.uri,
                    "content_etag": lock.remote_content_etag,
                    "dry_run": true
                })
            );
        } else {
            println!(
                "would update {} from {} using content ETag {}",
                spec.uri,
                args.input.display(),
                lock.remote_content_etag
            );
        }
        return ExitCode::SUCCESS;
    }

    let written = match client.put_collection(
        &spec.uri,
        format,
        &body,
        sbol_registry_client::CollectionPrecondition::Matches(&lock.remote_content_etag),
    ) {
        Ok(written) => written,
        Err(RegistryError::PreconditionFailed { .. }) => {
            eprintln!(
                "{}: remote biological content changed while the push was committing; no data was overwritten",
                styles.err_label()
            );
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!(
                "{}: failed to update {}: {error}",
                styles.err_label(),
                spec.uri
            );
            return ExitCode::from(2);
        }
    };
    if let Err(error) = workspace.record_sync(
        alias.clone(),
        spec.clone(),
        written.content_etag.clone(),
        sha256_bytes(&body),
    ) {
        eprintln!(
            "{}: remote collection was updated, but sbol.lock could not be updated: {error}",
            styles.err_label()
        );
        return ExitCode::from(2);
    }
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "action": "update",
                "collection": alias,
                "collection_uri": written.collection_uri,
                "content_etag": written.content_etag,
                "triple_count": written.triple_count
            })
        );
    } else {
        println!(
            "updated {} from {} ({})",
            spec.uri,
            args.input.display(),
            written.content_etag
        );
    }
    ExitCode::SUCCESS
}

fn plan_created_tracking(
    workspace: &Workspace,
    cwd: &Path,
    input: &Path,
    display_id: &str,
) -> Result<(String, PathBuf, CollectionRdfFormat), String> {
    let preferred = safe_collection_name("", Some(display_id));
    let mut alias = workspace.next_alias(&preferred);
    let input_absolute = if input.is_absolute() {
        input.to_path_buf()
    } else {
        cwd.join(input)
    };
    if let Ok(relative) = input_absolute.strip_prefix(workspace.root())
        && relative.starts_with(&workspace.manifest().designs_dir)
        && let Ok(format) = collection_format(relative)
    {
        validate_tracking_path(workspace, relative)?;
        return Ok((alias, relative.to_path_buf(), format));
    }

    let mut path = workspace
        .manifest()
        .designs_dir
        .join(format!("{alias}.ttl"));
    let mut suffix = 2;
    while workspace.root().join(&path).exists() {
        alias = format!("{preferred}-{suffix}");
        path = workspace
            .manifest()
            .designs_dir
            .join(format!("{alias}.ttl"));
        suffix += 1;
    }
    validate_tracking_path(workspace, &path)?;
    Ok((alias, path, CollectionRdfFormat::Turtle))
}

fn track_created_collection(
    workspace: &mut Workspace,
    client: &RegistryClient,
    created: &SubmissionCreated,
    alias: String,
    path: PathBuf,
    format: CollectionRdfFormat,
) -> Result<(), String> {
    let pulled = client
        .pull_collection(&created.collection_uri, format)
        .map_err(|error| format!("could not download canonical collection content: {error}"))?;
    validate_collection_rdf(&pulled.body, format)?;
    let spec = CollectionSpec {
        uri: created.collection_uri.clone(),
        registry: client.base_url().to_string(),
        path,
    };
    let destination = workspace.absolute_collection_path(&spec);
    write_file_atomic(&destination, &pulled.body).map_err(|error| error.to_string())?;
    workspace
        .record_sync(alias, spec, pulled.content_etag, sha256_bytes(&pulled.body))
        .map_err(|error| error.to_string())
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

pub(crate) fn registry_client(
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
    let Some(credential) = store.credential_for(client.base_url().as_str())? else {
        return Ok(client);
    };
    if !credential.needs_refresh() {
        return Ok(client.with_bearer_token(credential.access_token));
    }
    let (Some(refresh_token), Some(issuer), Some(client_id), Some(resource)) = (
        credential.refresh_token,
        credential.issuer,
        credential.client_id,
        credential.resource,
    ) else {
        return Err(format!(
            "the saved credential for {} has expired; run `sbol registry login {}` again",
            client.base_url(),
            client.base_url()
        ));
    };
    let metadata = client
        .authorization_server(&issuer)
        .map_err(|error| format!("failed to refresh SBOL Identity discovery: {error}"))?;
    let tokens = client
        .refresh_oauth_token(&metadata, &client_id, &resource, &refresh_token)
        .map_err(|error| {
            format!(
                "failed to refresh SBOL Identity access for {}: {error}; run `sbol registry login {}` again",
                client.base_url(),
                client.base_url()
            )
        })?;
    if tokens.resource != resource {
        return Err("SBOL Identity returned a refreshed token for the wrong resource".to_owned());
    }
    let access_token = tokens.access_token.clone();
    store.save_oauth_tokens(
        client.base_url().as_str(),
        tokens.access_token,
        tokens.refresh_token,
        tokens.expires_in,
        issuer,
        client_id,
        resource,
    )?;
    Ok(client.with_bearer_token(access_token))
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

#[cfg(test)]
mod oauth_tests {
    use super::*;

    fn metadata() -> AuthorizationServerMetadata {
        AuthorizationServerMetadata {
            issuer: "https://sbol.io".to_owned(),
            authorization_endpoint: "https://sbol.io/oauth/authorize".to_owned(),
            token_endpoint: "https://sbol.io/oauth/token".to_owned(),
            registration_endpoint: "https://sbol.io/oauth/register".to_owned(),
            revocation_endpoint: Some("https://sbol.io/oauth/revoke".to_owned()),
            code_challenge_methods_supported: vec!["S256".to_owned()],
            token_endpoint_auth_methods_supported: vec!["none".to_owned()],
            scopes_supported: vec!["sbol:read".to_owned(), "sbol:write".to_owned()],
        }
    }

    #[test]
    fn authorization_request_is_pkce_state_and_resource_bound() {
        let url = authorization_url(
            &metadata(),
            "client-123",
            "http://127.0.0.1:43123/callback",
            "https://sbol.io/api/v2",
            "challenge-123",
            "state-123",
        )
        .unwrap();
        let query = url
            .query_pairs()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(query["response_type"], "code");
        assert_eq!(query["client_id"], "client-123");
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["code_challenge"], "challenge-123");
        assert_eq!(query["resource"], "https://sbol.io/api/v2");
        assert_eq!(query["scope"], "sbol:read sbol:write");
        assert_eq!(query["state"], "state-123");
    }

    #[test]
    fn loopback_callback_requires_matching_state() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let sender = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(
                    b"GET /callback?code=code-123&state=state-123 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                )
                .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK"));
        });
        let code = await_oauth_callback(listener, "state-123", Duration::from_secs(2)).unwrap();
        assert_eq!(code, "code-123");
        sender.join().unwrap();

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let sender = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(
                    b"GET /callback?code=code-123&state=wrong HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                )
                .unwrap();
        });
        let error =
            await_oauth_callback(listener, "state-123", Duration::from_secs(2)).unwrap_err();
        assert!(error.contains("state did not match"));
        sender.join().unwrap();
    }

    #[test]
    fn loopback_callback_reads_fragmented_browser_headers() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let sender = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(b"GET /callback?code=fragmented&state=state-123 HTTP/1.1\r\nHost:")
                .unwrap();
            stream.flush().unwrap();
            thread::sleep(Duration::from_millis(20));
            stream.write_all(b" 127.0.0.1\r\n\r\n").unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK"));
        });
        let code = await_oauth_callback(listener, "state-123", Duration::from_secs(2)).unwrap();
        assert_eq!(code, "fragmented");
        sender.join().unwrap();
    }
}
