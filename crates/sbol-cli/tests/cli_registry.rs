//! End-to-end CLI tests for public registry discovery and pull.

mod common;
use common::*;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use assert_cmd::Command;
use predicates::prelude::*;
use sbol::v3::{Document, RdfFormat};
use tempfile::TempDir;

fn serve_once(content_type: &str, body: String) -> (String, Receiver<String>) {
    serve_responses(vec![(200, content_type.to_owned(), body)])
}

fn serve_responses(responses: Vec<(u16, String, String)>) -> (String, Receiver<String>) {
    serve_dynamic_responses(move |_| responses)
}

fn serve_dynamic_responses(
    responses: impl FnOnce(&str) -> Vec<(u16, String, String)>,
) -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let base = format!("http://{address}");
    let responses = responses(&base);
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for (status, content_type, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let count = stream.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
                if request_is_complete(&request) {
                    break;
                }
            }
            sender.send(String::from_utf8(request).unwrap()).unwrap();
            let reason = match status {
                200 => "OK",
                201 => "Created",
                409 => "Conflict",
                _ => "Response",
            };
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nETag: \"sbol-content-v1-test\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    (base, receiver)
}

fn request_is_complete(request: &[u8]) -> bool {
    let Some(header_end) = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
    else {
        return false;
    };
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    request.len() >= header_end + content_length
}

#[test]
fn registry_pull_infers_origin_and_writes_requested_rdf_format() {
    let rdf_xml = Document::read_turtle(TTL_VALID)
        .unwrap()
        .write(RdfFormat::RdfXml)
        .unwrap();
    let (base, request) = serve_once("application/rdf+xml", rdf_xml);
    let iri = format!("{base}/public/example/design/1");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("design.ttl");

    Command::cargo_bin("sbol")
        .unwrap()
        .env_remove("SBOL_REGISTRY_URL")
        .args([
            "registry",
            "pull",
            &iri,
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("pulled"));

    Document::read_path(&output).expect("pulled Turtle parses through sbol-rs");
    let request = request.recv().unwrap();
    assert!(request.contains("?format=sbol&version=sbol3"));
    assert!(request.contains("%2Fpublic%2Fexample%2Fdesign%2F1"));
}

#[test]
fn init_and_project_pull_create_a_tracked_collection_and_lock() {
    let (base, requests) = serve_dynamic_responses(|base| {
        let iri = format!("{base}/user/alice/toggle/toggle_collection/1");
        vec![
            (
                200,
                "application/json".to_owned(),
                format!(
                    r#"{{
                        "iri":"{iri}",
                        "content_url":"/api/v2/collections/content",
                        "content_etag":"\"sbol-content-v1-test\"",
                        "triple_count":4,
                        "display_id":"toggle"
                    }}"#
                ),
            ),
            (200, "text/turtle".to_owned(), TTL_VALID.to_owned()),
        ]
    });
    let iri = format!("{base}/user/alice/toggle/toggle_collection/1");
    let dir = TempDir::new().unwrap();

    Command::cargo_bin("sbol")
        .unwrap()
        .current_dir(dir.path())
        .args(["init"])
        .assert()
        .success();
    assert!(dir.path().join("sbol.toml").is_file());
    assert!(dir.path().join("designs").is_dir());
    assert!(!dir.path().join("sbol.lock").exists());

    Command::cargo_bin("sbol")
        .unwrap()
        .current_dir(dir.path())
        .env_remove("SBOL_REGISTRY_URL")
        .args(["registry", "pull", &iri])
        .assert()
        .success()
        .stderr(predicate::str::contains("as `toggle`"));

    assert!(dir.path().join("designs/toggle.ttl").is_file());
    Document::read_path(dir.path().join("designs/toggle.ttl")).unwrap();
    let manifest = std::fs::read_to_string(dir.path().join("sbol.toml")).unwrap();
    let lock = std::fs::read_to_string(dir.path().join("sbol.lock")).unwrap();
    assert!(manifest.contains("[collections.toggle]"));
    assert!(manifest.contains(&iri));
    assert!(lock.contains("sbol-content-v1-test"));
    assert!(!manifest.to_ascii_lowercase().contains("token"));
    assert!(!lock.to_ascii_lowercase().contains("token"));

    let descriptor = requests.recv().unwrap();
    assert!(descriptor.starts_with("GET /api/v2/collections/"));
    let content = requests.recv().unwrap();
    assert!(content.contains("/content HTTP/1.1"));
    assert!(content.to_ascii_lowercase().contains("accept: text/turtle"));
}

#[test]
fn tracked_push_uses_the_locked_content_etag_as_a_compare_and_swap() {
    let (base, requests) = serve_dynamic_responses(|base| {
        let iri = format!("{base}/user/alice/toggle/toggle_collection/1");
        let descriptor = format!(
            r#"{{
                "iri":"{iri}",
                "content_url":"/api/v2/collections/content",
                "content_etag":"\"sbol-content-v1-test\"",
                "triple_count":4,
                "display_id":"toggle"
            }}"#
        );
        vec![
            (200, "application/json".to_owned(), descriptor.clone()),
            (200, "text/turtle".to_owned(), TTL_VALID.to_owned()),
            (200, "application/json".to_owned(), descriptor),
            (
                200,
                "application/json".to_owned(),
                format!(
                    r#"{{
                        "collection_uri":"{iri}",
                        "content_etag":"\"sbol-content-v1-next\"",
                        "triple_count":5
                    }}"#
                ),
            ),
        ]
    });
    let iri = format!("{base}/user/alice/toggle/toggle_collection/1");
    let dir = TempDir::new().unwrap();
    Command::cargo_bin("sbol")
        .unwrap()
        .current_dir(dir.path())
        .args(["init"])
        .assert()
        .success();
    Command::cargo_bin("sbol")
        .unwrap()
        .current_dir(dir.path())
        .args(["registry", "pull", &iri])
        .assert()
        .success();

    let path = dir.path().join("designs/toggle.ttl");
    let changed = TTL_VALID.replace(
        "    sbol:type SBO:0000251 .",
        "    sbol:name \"Changed locally\";\n    sbol:type SBO:0000251 .",
    );
    std::fs::write(&path, changed).unwrap();
    Command::cargo_bin("sbol")
        .unwrap()
        .current_dir(dir.path())
        .args(["registry", "push", "designs/toggle.ttl"])
        .assert()
        .success()
        .stdout(predicate::str::contains("updated"));

    let _pull_descriptor = requests.recv().unwrap();
    let _pull_content = requests.recv().unwrap();
    let push_descriptor = requests.recv().unwrap();
    assert!(push_descriptor.starts_with("GET /api/v2/collections/"));
    let put = requests.recv().unwrap().to_ascii_lowercase();
    assert!(put.starts_with("put /api/v2/collections/"));
    assert!(put.contains("if-match: \"sbol-content-v1-test\""));
    let lock = std::fs::read_to_string(dir.path().join("sbol.lock")).unwrap();
    assert!(lock.contains("sbol-content-v1-next"));
}

#[test]
fn registry_status_reads_instance_metadata() {
    let body = r#"{
        "name":"Local SBOL DB",
        "instance_url":"http://127.0.0.1:8888",
        "uri_prefix":"http://127.0.0.1:8888/",
        "front_page_text":"",
        "setup_required":false,
        "policies":{"allow_public_signup":true,"require_login":false},
        "capabilities":{}
    }"#
    .to_owned();
    let (base, request) = serve_once("application/json", body);

    Command::cargo_bin("sbol")
        .unwrap()
        .env_remove("SBOL_REGISTRY_URL")
        .args(["registry", "status", &base])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Local SBOL DB")
                .and(predicate::str::contains("public browsing")),
        );
    assert!(request.recv().unwrap().starts_with("GET /api/v2/instance"));
}

#[test]
fn registry_commands_rotate_expired_oauth_credentials_before_use() {
    let (base, requests) = serve_dynamic_responses(|base| {
        vec![
            (
                200,
                "application/json".to_owned(),
                format!(
                    r#"{{
                        "issuer":"{base}",
                        "authorization_endpoint":"{base}/oauth/authorize",
                        "token_endpoint":"{base}/oauth/token",
                        "registration_endpoint":"{base}/oauth/register",
                        "code_challenge_methods_supported":["S256"],
                        "token_endpoint_auth_methods_supported":["none"],
                        "scopes_supported":["sbol:read","sbol:write"]
                    }}"#
                ),
            ),
            (
                200,
                "application/json".to_owned(),
                format!(
                    r#"{{
                        "access_token":"new-access",
                        "refresh_token":"new-refresh",
                        "expires_in":3600,
                        "token_type":"Bearer",
                        "scope":"sbol:read sbol:write",
                        "resource":"{base}/api/v2"
                    }}"#
                ),
            ),
            (
                200,
                "application/json".to_owned(),
                format!(
                    r#"{{
                        "name":"Local SBOL DB",
                        "instance_url":"{base}",
                        "uri_prefix":"{base}/",
                        "front_page_text":"",
                        "setup_required":false,
                        "policies":{{"allow_public_signup":true,"require_login":false}},
                        "capabilities":{{}},
                        "machine_access":{{"api_url":"{base}/api/v2","authorization_issuer":"{base}"}}
                    }}"#
                ),
            ),
        ]
    });
    let dir = TempDir::new().unwrap();
    let credentials = dir.path().join("credentials.json");
    std::fs::write(
        &credentials,
        format!(
            r#"{{
                "version":1,
                "active_registry":"{base}/",
                "registries":{{
                    "{base}/":{{
                        "access_token":"expired-access",
                        "refresh_token":"old-refresh",
                        "expires_at":0,
                        "issuer":"{base}",
                        "client_id":"client-123",
                        "resource":"{base}/api/v2"
                    }}
                }}
            }}"#
        ),
    )
    .unwrap();

    Command::cargo_bin("sbol")
        .unwrap()
        .env_remove("SBOL_REGISTRY_URL")
        .env("SBOL_CREDENTIALS_FILE", &credentials)
        .args(["registry", "status", &base])
        .assert()
        .success()
        .stdout(predicate::str::contains("Local SBOL DB"));

    let discovery = requests.recv().unwrap();
    assert!(discovery.starts_with("GET /.well-known/oauth-authorization-server"));
    let refresh = requests.recv().unwrap();
    assert!(refresh.starts_with("POST /oauth/token"));
    let refresh_form = refresh.split_once("\r\n\r\n").unwrap().1;
    assert!(refresh_form.contains("grant_type=refresh_token"));
    assert!(refresh_form.contains("refresh_token=old-refresh"));
    let status = requests.recv().unwrap().to_ascii_lowercase();
    assert!(status.starts_with("get /api/v2/instance"));
    assert!(status.contains("authorization: bearer new-access"));

    let stored = std::fs::read_to_string(credentials).unwrap();
    assert!(stored.contains("new-access"));
    assert!(stored.contains("new-refresh"));
    assert!(!stored.contains("old-refresh"));
}

#[test]
fn registry_status_requires_an_explicit_or_environment_registry() {
    let dir = TempDir::new().unwrap();
    Command::cargo_bin("sbol")
        .unwrap()
        .env_remove("SBOL_REGISTRY_URL")
        .env("SBOL_CREDENTIALS_FILE", dir.path().join("missing.json"))
        .args(["registry", "status"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("provide a registry URL"));
}

#[test]
fn registry_login_stores_only_the_returned_token_in_a_private_profile() {
    let (base, requests) = serve_once("text/plain", "opaque-token\n".to_owned());
    let dir = TempDir::new().unwrap();
    let credentials = dir.path().join("credentials.json");

    Command::cargo_bin("sbol")
        .unwrap()
        .env_remove("SBOL_REGISTRY_URL")
        .env("SBOL_CREDENTIALS_FILE", &credentials)
        .args([
            "registry",
            "login",
            &base,
            "--identifier",
            "alice@example.org",
            "--password-stdin",
        ])
        .write_stdin("s3cret password\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("signed in"));

    let request = requests.recv().unwrap();
    assert!(request.starts_with("POST /login HTTP/1.1"));
    assert!(request.ends_with("email=alice%40example.org&password=s3cret+password"));
    let stored = std::fs::read_to_string(&credentials).unwrap();
    assert!(stored.contains("opaque-token"));
    assert!(!stored.contains("s3cret password"));
    let parsed: serde_json::Value = serde_json::from_str(&stored).unwrap();
    let registry = format!("{base}/");
    assert_eq!(parsed["active_registry"], registry);
    assert_eq!(
        parsed["registries"][&registry]["access_token"],
        "opaque-token"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(credentials).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn registry_login_does_not_fall_back_to_password_when_identity_discovery_fails() {
    let (base, requests) = serve_dynamic_responses(|_| {
        vec![(
            503,
            "application/json".to_owned(),
            r#"{"error":{"message":"identity unavailable"}}"#.to_owned(),
        )]
    });
    let dir = TempDir::new().unwrap();

    Command::cargo_bin("sbol")
        .unwrap()
        .env_remove("SBOL_REGISTRY_URL")
        .env("SBOL_CREDENTIALS_FILE", dir.path().join("credentials.json"))
        .args(["registry", "login", &base])
        .assert()
        .code(2)
        .stderr(
            predicate::str::contains("could not discover SBOL Identity")
                .and(predicate::str::contains("--password-stdin")),
        );

    assert!(requests.recv().unwrap().starts_with("GET /api/v2/instance"));
}

#[test]
fn no_browser_is_only_available_for_identity_login() {
    Command::cargo_bin("sbol")
        .unwrap()
        .args([
            "registry",
            "login",
            "https://sbol.io",
            "--identifier",
            "alice@example.org",
            "--no-browser",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn registry_logout_revokes_and_removes_a_compatibility_credential() {
    let (base, requests) =
        serve_dynamic_responses(|_| vec![(204, "application/json".to_owned(), String::new())]);
    let dir = TempDir::new().unwrap();
    let credentials = dir.path().join("credentials.json");
    std::fs::write(
        &credentials,
        format!(
            r#"{{
                "version":1,
                "active_registry":"{base}/",
                "registries":{{"{base}/":{{"access_token":"compatibility-secret"}}}}
            }}"#
        ),
    )
    .unwrap();

    Command::cargo_bin("sbol")
        .unwrap()
        .env_remove("SBOL_REGISTRY_URL")
        .env("SBOL_CREDENTIALS_FILE", &credentials)
        .args(["registry", "logout", &base])
        .assert()
        .success()
        .stdout(predicate::str::contains("signed out"));

    let request = requests.recv().unwrap().to_ascii_lowercase();
    assert!(request.starts_with("delete /api/v2/session"));
    assert!(request.contains("authorization: bearer compatibility-secret"));
    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(credentials).unwrap()).unwrap();
    assert!(stored["active_registry"].is_null());
    assert!(stored["registries"].as_object().unwrap().is_empty());
}

#[test]
fn registry_push_previews_with_an_explicit_collision_policy_and_no_write() {
    let preview = r#"{
        "valid":true,
        "source_format":"turtle",
        "source_standard":"sbol3",
        "normalized_standard":"sbol3",
        "collection_uri":"http://registry/user/alice/toggle_switch/toggle_switch_collection/1",
        "persistent_identity":"http://registry/user/alice/toggle_switch/toggle_switch_collection",
        "graph":"http://registry/user/alice/toggle_switch/toggle_switch_collection/1",
        "members":["http://registry/user/alice/toggle_switch/component/1"],
        "triple_count":42,
        "collision":false,
        "consequence":"create",
        "notices":[]
    }"#
    .to_owned();
    let (base, requests) = serve_once("application/json", preview);
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("toggle-switch.ttl");
    std::fs::write(&input, TTL_VALID).unwrap();

    Command::cargo_bin("sbol")
        .unwrap()
        .env("SBOL_REGISTRY_URL", &base)
        .env("SBOL_ACCESS_TOKEN", "secret-token")
        .args(["registry", "push", input.to_str().unwrap(), "--preview"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("change: create")
                .and(predicate::str::contains("preview only")),
        );

    let request = requests.recv().unwrap();
    assert!(request.starts_with("POST /api/v2/collections/validate"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret-token")
    );
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").expect("request body").1).unwrap();
    assert_eq!(body["id"], "toggle_switch");
    assert_eq!(body["overwrite"], "fail");
}

#[test]
fn registry_push_validates_then_commits_the_identical_request() {
    let preview = r#"{
        "valid":true,
        "source_format":"turtle",
        "source_standard":"sbol3",
        "normalized_standard":"sbol3",
        "collection_uri":"http://registry/user/alice/design/design_collection/1",
        "persistent_identity":"http://registry/user/alice/design/design_collection",
        "graph":"http://registry/user/alice/design/design_collection/1",
        "members":["http://registry/user/alice/design/component/1"],
        "triple_count":42,
        "collision":false,
        "consequence":"create",
        "notices":[]
    }"#
    .to_owned();
    let created = r#"{
        "collection_uri":"http://registry/user/alice/design/design_collection/1",
        "persistent_identity":"http://registry/user/alice/design/design_collection",
        "members":["http://registry/user/alice/design/component/1"],
        "graph":"http://registry/user/alice/design/design_collection/1",
        "triple_count":42
    }"#
    .to_owned();
    let (base, requests) = serve_responses(vec![
        (200, "application/json".to_owned(), preview),
        (201, "application/json".to_owned(), created),
    ]);
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("design.ttl");
    std::fs::write(&input, TTL_VALID).unwrap();

    Command::cargo_bin("sbol")
        .unwrap()
        .env("SBOL_REGISTRY_URL", &base)
        .env("SBOL_ACCESS_TOKEN", "secret-token")
        .args(["registry", "push", input.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "pushed http://registry/user/alice/design/design_collection/1",
        ));

    let preview_request = requests.recv().unwrap();
    let create_request = requests.recv().unwrap();
    assert!(preview_request.starts_with("POST /api/v2/collections/validate"));
    assert!(create_request.starts_with("POST /api/v2/collections HTTP/1.1"));
    assert_eq!(
        preview_request.split_once("\r\n\r\n").unwrap().1,
        create_request.split_once("\r\n\r\n").unwrap().1,
        "commit must send the exact request that was previewed"
    );
}
