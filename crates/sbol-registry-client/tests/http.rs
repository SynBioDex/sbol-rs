use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use sbol_registry_client::{
    CollisionPolicy, RegistryClient, SbolVersion, SubmissionConsequence, SubmissionFormat,
    SubmissionRequest,
};

fn serve_once(status: u16, content_type: &str, body: &str) -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let body = body.to_owned();
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
    (format!("http://{address}"), receiver)
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
