use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use sbol_registry_client::{
    AuthorizationServerMetadata, CollisionPolicy, RegistryClient, SbolVersion,
    SubmissionConsequence, SubmissionFormat, SubmissionRequest,
};

fn serve_once(status: u16, content_type: &str, body: &str) -> (String, Receiver<String>) {
    let body = body.to_owned();
    serve_once_dynamic(status, content_type, move |_| body)
}

fn serve_once_dynamic(
    status: u16,
    content_type: &str,
    body: impl FnOnce(&str) -> String,
) -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let base = format!("http://{address}");
    let body = body(&base);
    let content_type = content_type.to_owned();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
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
        let reason = if status == 200 { "OK" } else { "Error" };
        write!(
            stream,
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nETag: \"revision-1\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
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
fn discovers_typed_instance_metadata() {
    let body = r#"{
        "name":"Local SBOL DB",
        "instance_url":"http://127.0.0.1:8888",
        "uri_prefix":"http://127.0.0.1:8888/",
        "front_page_text":"",
        "setup_required":false,
        "policies":{"allow_public_signup":true,"require_login":false},
        "capabilities":{"structured_search":true},
        "machine_access":{"api_url":"http://127.0.0.1:8888/api/v2"}
    }"#;
    let (base, request) = serve_once(200, "application/json", body);
    let instance = RegistryClient::new(&base).unwrap().instance().unwrap();

    assert_eq!(instance.name, "Local SBOL DB");
    assert!(!instance.policies.require_login);
    assert_eq!(
        instance.machine_access.unwrap().api_url,
        "http://127.0.0.1:8888/api/v2"
    );
    assert!(
        request
            .recv()
            .unwrap()
            .starts_with("GET /api/v2/instance HTTP/1.1")
    );
}

#[test]
fn pull_encodes_the_complete_iri_and_preserves_revision_metadata() {
    let (base, request) = serve_once(200, "application/rdf+xml", "<rdf:RDF />");
    let pulled = RegistryClient::new(&base)
        .unwrap()
        .with_bearer_token("secret-token")
        .pull(
            "https://example.org/design/alpha?revision=1",
            SbolVersion::V3,
        )
        .unwrap();

    assert_eq!(pulled.body, b"<rdf:RDF />");
    assert_eq!(pulled.etag.as_deref(), Some("\"revision-1\""));
    let request = request.recv().unwrap();
    assert!(request.contains("GET /api/v2/objects/https:%2F%2Fexample.org%2Fdesign%2Falpha%3Frevision=1?format=sbol&version=sbol3 HTTP/1.1"), "request was {request}");
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret-token")
    );
}

#[test]
fn structured_server_error_is_preserved() {
    let body = r#"{"error":{"code":"not_found","message":"design is not visible"}}"#;
    let (base, _request) = serve_once(404, "application/json", body);
    let error = RegistryClient::new(&base)
        .unwrap()
        .pull("https://example.org/missing", SbolVersion::V3)
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "registry returned HTTP 404: design is not visible"
    );
}

#[test]
fn compatibility_login_returns_a_bearer_without_leaking_credentials_into_the_url() {
    let (base, request) = serve_once(200, "text/plain", "opaque-token\n");
    let token = RegistryClient::new(&base)
        .unwrap()
        .password_login("alice@example.org", "s3cret & safe")
        .unwrap();
    assert_eq!(token, "opaque-token");

    let request = request.recv().unwrap();
    assert!(request.starts_with("POST /login HTTP/1.1"));
    assert!(!request.lines().next().unwrap().contains("alice"));
    assert!(request.ends_with("email=alice%40example.org&password=s3cret+%26+safe"));
}

#[test]
fn discovers_pkce_authorization_server_and_rejects_issuer_substitution() {
    let (base, request) = serve_once_dynamic(200, "application/json", |base| {
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
        )
    });
    let metadata = RegistryClient::new(&base)
        .unwrap()
        .authorization_server(&base)
        .unwrap();
    assert_eq!(metadata.issuer, base);
    assert!(
        request
            .recv()
            .unwrap()
            .starts_with("GET /.well-known/oauth-authorization-server HTTP/1.1")
    );

    let (base, _request) = serve_once_dynamic(200, "application/json", |base| {
        format!(
            r#"{{
                "issuer":"https://evil.example",
                "authorization_endpoint":"{base}/oauth/authorize",
                "token_endpoint":"{base}/oauth/token",
                "registration_endpoint":"{base}/oauth/register",
                "code_challenge_methods_supported":["S256"],
                "token_endpoint_auth_methods_supported":["none"]
            }}"#
        )
    });
    let error = RegistryClient::new(&base)
        .unwrap()
        .authorization_server(&base)
        .unwrap_err();
    assert!(error.to_string().contains("issuer mismatch"));
}

#[test]
fn registers_public_client_and_exchanges_resource_bound_token() {
    let registration_body = r#"{
        "client_id":"client-123",
        "client_name":"sbol CLI",
        "redirect_uris":["http://127.0.0.1:43123/callback"]
    }"#;
    let (base, registration_request) = serve_once(201, "application/json", registration_body);
    let metadata = AuthorizationServerMetadata {
        issuer: base.clone(),
        authorization_endpoint: format!("{base}/oauth/authorize"),
        token_endpoint: format!("{base}/oauth/token"),
        registration_endpoint: format!("{base}/oauth/register"),
        revocation_endpoint: Some(format!("{base}/oauth/revoke")),
        code_challenge_methods_supported: vec!["S256".to_owned()],
        token_endpoint_auth_methods_supported: vec!["none".to_owned()],
        scopes_supported: vec!["sbol:read".to_owned(), "sbol:write".to_owned()],
    };
    let registration = RegistryClient::new(&base)
        .unwrap()
        .register_public_oauth_client(&metadata, "sbol CLI", "http://127.0.0.1:43123/callback")
        .unwrap();
    assert_eq!(registration.client_id, "client-123");
    let request = registration_request.recv().unwrap();
    assert!(request.starts_with("POST /oauth/register HTTP/1.1"));
    let request_json: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(request_json["token_endpoint_auth_method"], "none");
    assert!(request_json.get("client_secret").is_none());

    let token_body = r#"{
        "access_token":"access-secret",
        "refresh_token":"refresh-secret",
        "expires_in":3600,
        "token_type":"Bearer",
        "scope":"sbol:read sbol:write",
        "resource":"http://127.0.0.1:8888/api/v2"
    }"#;
    let (base, token_request) = serve_once(200, "application/json", token_body);
    let metadata = AuthorizationServerMetadata {
        token_endpoint: format!("{base}/oauth/token"),
        issuer: base.clone(),
        authorization_endpoint: format!("{base}/oauth/authorize"),
        registration_endpoint: format!("{base}/oauth/register"),
        revocation_endpoint: None,
        code_challenge_methods_supported: vec!["S256".to_owned()],
        token_endpoint_auth_methods_supported: vec!["none".to_owned()],
        scopes_supported: vec![],
    };
    let token = RegistryClient::new(&base)
        .unwrap()
        .exchange_oauth_code(
            &metadata,
            "client-123",
            "http://127.0.0.1:43123/callback",
            "http://127.0.0.1:8888/api/v2",
            "authorization-code",
            "pkce-verifier",
        )
        .unwrap();
    assert_eq!(token.resource, "http://127.0.0.1:8888/api/v2");
    let debug = format!("{token:?}");
    assert!(!debug.contains("access-secret"));
    assert!(!debug.contains("refresh-secret"));
    let request = token_request.recv().unwrap();
    let form = request.split_once("\r\n\r\n").unwrap().1;
    assert!(form.contains("grant_type=authorization_code"));
    assert!(form.contains("resource=http%3A%2F%2F127.0.0.1%3A8888%2Fapi%2Fv2"));
    assert!(form.contains("code_verifier=pkce-verifier"));
}

#[test]
fn revokes_oauth_and_compatibility_credentials_without_putting_tokens_in_urls() {
    let (base, oauth_request) = serve_once(200, "application/json", "");
    let metadata = AuthorizationServerMetadata {
        issuer: base.clone(),
        authorization_endpoint: format!("{base}/oauth/authorize"),
        token_endpoint: format!("{base}/oauth/token"),
        registration_endpoint: format!("{base}/oauth/register"),
        revocation_endpoint: Some(format!("{base}/oauth/revoke")),
        code_challenge_methods_supported: vec!["S256".to_owned()],
        token_endpoint_auth_methods_supported: vec!["none".to_owned()],
        scopes_supported: vec![],
    };
    RegistryClient::new(&base)
        .unwrap()
        .revoke_oauth_token(&metadata, "refresh-secret")
        .unwrap();
    let request = oauth_request.recv().unwrap();
    assert!(request.starts_with("POST /oauth/revoke HTTP/1.1"));
    assert!(!request.lines().next().unwrap().contains("refresh-secret"));
    let form = url::form_urlencoded::parse(request.split_once("\r\n\r\n").unwrap().1.as_bytes())
        .into_owned()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(form["token"], "refresh-secret");

    let (base, compatibility_request) = serve_once(204, "application/json", "");
    RegistryClient::new(&base)
        .unwrap()
        .with_bearer_token("compatibility-secret")
        .logout_compatibility_session()
        .unwrap();
    let request = compatibility_request.recv().unwrap();
    assert!(request.starts_with("DELETE /api/v2/session HTTP/1.1"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer compatibility-secret")
    );
}

#[test]
fn authenticated_submission_preview_uses_the_typed_json_contract() {
    let body = r#"{
        "valid":true,
        "source_format":"turtle",
        "source_standard":"sbol3",
        "normalized_standard":"sbol3",
        "collection_uri":"https://example.org/user/alice/toggle/toggle_collection/1",
        "persistent_identity":"https://example.org/user/alice/toggle/toggle_collection",
        "graph":"https://example.org/user/alice/toggle/toggle_collection/1",
        "members":["https://example.org/user/alice/toggle/component/1"],
        "triple_count":42,
        "collision":false,
        "consequence":"create",
        "notices":[]
    }"#;
    let (base, request) = serve_once(200, "application/json", body);
    let submission = SubmissionRequest {
        id: "toggle".to_owned(),
        version: "1".to_owned(),
        name: Some("Toggle switch".to_owned()),
        description: None,
        citations: Vec::new(),
        creator_name: None,
        format: SubmissionFormat::Turtle,
        overwrite: CollisionPolicy::Fail,
        content: "@prefix sbol: <http://sbols.org/v3#> .".to_owned(),
    };

    let preview = RegistryClient::new(&base)
        .unwrap()
        .with_bearer_token("secret-token")
        .preview_submission(&submission)
        .unwrap();
    assert_eq!(preview.consequence, SubmissionConsequence::Create);
    assert_eq!(preview.triple_count, 42);

    let request = request.recv().unwrap();
    assert!(request.starts_with("POST /api/v2/collections/validate HTTP/1.1"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret-token")
    );
    let json: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").expect("HTTP body").1)
            .expect("request JSON");
    assert_eq!(json["id"], "toggle");
    assert_eq!(json["format"], "turtle");
    assert_eq!(json["overwrite"], "fail");
}

#[test]
fn creates_a_submission_through_the_same_typed_request() {
    let body = r#"{
        "collection_uri":"https://example.org/user/alice/toggle/toggle_collection/1",
        "persistent_identity":"https://example.org/user/alice/toggle/toggle_collection",
        "members":["https://example.org/user/alice/toggle/component/1"],
        "graph":"https://example.org/user/alice/toggle/toggle_collection/1",
        "triple_count":42
    }"#;
    let (base, request) = serve_once(201, "application/json", body);
    let submission = SubmissionRequest {
        id: "toggle".to_owned(),
        version: "1".to_owned(),
        name: None,
        description: None,
        citations: Vec::new(),
        creator_name: None,
        format: SubmissionFormat::Turtle,
        overwrite: CollisionPolicy::Fail,
        content: "@prefix sbol: <http://sbols.org/v3#> .".to_owned(),
    };

    let created = RegistryClient::new(&base)
        .unwrap()
        .with_bearer_token("secret-token")
        .create_submission(&submission)
        .unwrap();
    assert_eq!(created.triple_count, 42);
    assert!(
        request
            .recv()
            .unwrap()
            .starts_with("POST /api/v2/collections HTTP/1.1")
    );
}
