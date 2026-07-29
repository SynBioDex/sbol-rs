use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use sbol3::{
    ContentResolver, Document, DocumentResolver, DocumentSet, ExternalValidationMode, HttpResolver,
    Iri, RawDocument, ResolutionError, ResolutionErrorKind, ResolvedContent, Resource, Severity,
    ValidationContext,
};
use sha3::{Digest, Sha3_256};

const PREFIXES: &str = r#"BASE <https://example.org/>
PREFIX : <https://example.org/>
PREFIX EDAM: <https://identifiers.org/edam:>
PREFIX SBO: <https://identifiers.org/SBO:>
PREFIX sbol: <http://sbols.org/v3#>
"#;

#[derive(Clone)]
struct StaticDocumentResolver {
    result: Result<RawDocument, ResolutionError>,
}

impl DocumentResolver for StaticDocumentResolver {
    fn resolve_document(&self, _resource: &Resource) -> Result<RawDocument, ResolutionError> {
        self.result.clone()
    }
}

#[derive(Clone)]
struct StaticContentResolver {
    result: Result<ResolvedContent, ResolutionError>,
}

impl ContentResolver for StaticContentResolver {
    fn resolve_content(&self, _source: &Iri) -> Result<ResolvedContent, ResolutionError> {
        self.result.clone()
    }
}

#[test]
fn http_resolver_is_available_in_the_standard_build() {
    let resolver = HttpResolver::new();
    let error = resolver
        .resolve_content(&Iri::from_static("file:///tmp/not-http"))
        .unwrap_err();

    assert_eq!(error.kind(), ResolutionErrorKind::UnsupportedScheme);
}

#[test]
fn default_validation_does_not_dereference_external_resources() {
    let document = read(
        r#":component a sbol:Component;
    sbol:displayId "component";
    sbol:hasNamespace <https://example.org>;
    sbol:hasModel <https://external.example/model>;
    sbol:type SBO:0000251 .
"#,
    );

    let report = document.validate();

    assert!(!has_issue(&report, "sbol3-10114", Severity::Warning));
}

#[test]
fn provided_document_resolves_external_top_level_reference() {
    let document = read(
        r#":component a sbol:Component;
    sbol:displayId "component";
    sbol:hasNamespace <https://example.org>;
    sbol:hasModel <https://external.example/model>;
    sbol:type SBO:0000251 .
"#,
    );
    let external = read(
        r#"<https://external.example/model> a sbol:Model;
    sbol:displayId "model";
    sbol:hasNamespace <https://external.example>;
    sbol:source <https://external.example/model.xml>;
    sbol:framework <https://identifiers.org/SBO:0000062>;
    sbol:language <https://identifiers.org/edam:format_2585> .
"#,
    );
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ProvidedOnly)
        .with_document(&external);

    let report = document.validate_with_context(context);

    assert!(!has_issue(&report, "sbol3-10114", Severity::Warning));
    assert!(!has_issue(&report, "sbol3-10113", Severity::Error));
}

#[test]
fn unresolved_external_top_level_reference_reports_closure_warning() {
    let document = read(
        r#":component a sbol:Component;
    sbol:displayId "component";
    sbol:hasNamespace <https://example.org>;
    sbol:hasModel <https://external.example/model>;
    sbol:type SBO:0000251 .
"#,
    );
    let context = ValidationContext::new().with_external_mode(ExternalValidationMode::ProvidedOnly);

    let report = document.validate_with_context(context);

    assert!(has_issue(&report, "sbol3-10114", Severity::Warning));
}

#[test]
fn resolved_external_top_level_reference_reports_wrong_type() {
    let document = read(
        r#":component a sbol:Component;
    sbol:displayId "component";
    sbol:hasNamespace <https://example.org>;
    sbol:hasModel <https://external.example/not-a-model>;
    sbol:type SBO:0000251 .
"#,
    );
    let external = read(
        r#"<https://external.example/not-a-model> a sbol:Sequence;
    sbol:displayId "not_a_model";
    sbol:elements "ATGC";
    sbol:encoding EDAM:format_1207;
    sbol:hasNamespace <https://external.example> .
"#,
    );
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ProvidedOnly)
        .with_document(&external);

    let report = document.validate_with_context(context);

    assert!(has_issue(&report, "sbol3-10113", Severity::Error));
}

#[test]
fn mock_document_resolver_can_dereference_external_document() {
    let document = read(
        r#":component a sbol:Component;
    sbol:displayId "component";
    sbol:hasNamespace <https://example.org>;
    sbol:hasModel <https://external.example/model>;
    sbol:type SBO:0000251 .
"#,
    );
    let external = read(
        r#"<https://external.example/model> a sbol:Model;
    sbol:displayId "model";
    sbol:hasNamespace <https://external.example>;
    sbol:source <https://external.example/model.xml>;
    sbol:framework <https://identifiers.org/SBO:0000062>;
    sbol:language <https://identifiers.org/edam:format_2585> .
"#,
    );
    let resolver = StaticDocumentResolver {
        result: Ok(external.into_raw()),
    };
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ExternalAllowed)
        .with_document_resolver(&resolver);

    let report = document.validate_with_context(context);

    assert!(!has_issue(&report, "sbol3-10114", Severity::Warning));
    assert!(!has_issue(&report, "sbol3-10113", Severity::Error));
}

#[test]
fn attachment_content_provider_checks_size_and_sha3_hash() {
    let document = read(
        r#":attachment a sbol:Attachment;
    sbol:displayId "attachment";
    sbol:hasNamespace <https://example.org>;
    sbol:source <https://files.example/attachment.txt>;
    sbol:size "4";
    sbol:hash "00";
    sbol:hashAlgorithm "sha3-256" .
"#,
    );
    let resolver = StaticContentResolver {
        result: Ok(ResolvedContent::new(
            b"hello".to_vec(),
            Some("text/plain".to_owned()),
        )),
    };
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ProvidedOnly)
        .with_content_resolver(&resolver);

    let report = document.validate_with_context(context);

    assert!(has_issue(&report, "sbol3-12804", Severity::Error));
    assert!(has_issue(&report, "sbol3-12805", Severity::Error));
}

#[test]
fn attachment_content_provider_accepts_matching_size_and_sha3_hash() {
    let bytes = b"hello";
    let digest = Sha3_256::digest(bytes);
    let hash = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let document = read(&format!(
        r#":attachment a sbol:Attachment;
    sbol:displayId "attachment";
    sbol:hasNamespace <https://example.org>;
    sbol:source <https://files.example/attachment.txt>;
    sbol:size "5";
    sbol:hash "{hash}";
    sbol:hashAlgorithm "sha3-256" .
"#
    ));
    let resolver = StaticContentResolver {
        result: Ok(ResolvedContent::new(
            bytes.to_vec(),
            Some("text/plain".to_owned()),
        )),
    };
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ProvidedOnly)
        .with_content_resolver(&resolver);

    let report = document.validate_with_context(context);

    assert!(!has_issue(&report, "sbol3-12801", Severity::Error));
    assert!(!has_issue(&report, "sbol3-12804", Severity::Error));
    assert!(!has_issue(&report, "sbol3-12805", Severity::Error));
}

#[test]
fn attachment_source_not_found_reports_source_error() {
    let document = read(
        r#":attachment a sbol:Attachment;
    sbol:displayId "attachment";
    sbol:hasNamespace <https://example.org>;
    sbol:source <https://files.example/missing.txt> .
"#,
    );
    let resolver = StaticContentResolver {
        result: Err(ResolutionError::new(
            ResolutionErrorKind::NotFound,
            "missing fixture",
        )),
    };
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ProvidedOnly)
        .with_content_resolver(&resolver);

    let report = document.validate_with_context(context);

    assert!(has_issue(&report, "sbol3-12801", Severity::Error));
}

#[test]
fn unsupported_attachment_source_reports_warning() {
    let document = read(
        r#":attachment a sbol:Attachment;
    sbol:displayId "attachment";
    sbol:hasNamespace <https://example.org>;
    sbol:source <https://files.example/attachment.txt> .
"#,
    );
    let resolver = StaticContentResolver {
        result: Err(ResolutionError::new(
            ResolutionErrorKind::UnsupportedScheme,
            "unsupported scheme",
        )),
    };
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ProvidedOnly)
        .with_content_resolver(&resolver);

    let report = document.validate_with_context(context);

    assert!(has_issue(&report, "sbol3-12801", Severity::Warning));
}

#[test]
fn model_source_not_found_reports_model_source_error() {
    let document = read(
        r#":model a sbol:Model;
    sbol:displayId "model";
    sbol:hasNamespace <https://example.org>;
    sbol:source <https://files.example/missing.xml>;
    sbol:framework <https://identifiers.org/SBO:0000062>;
    sbol:language <https://identifiers.org/edam:format_2585> .
"#,
    );
    let resolver = StaticContentResolver {
        result: Err(ResolutionError::new(
            ResolutionErrorKind::NotFound,
            "missing fixture",
        )),
    };
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ProvidedOnly)
        .with_content_resolver(&resolver);

    let report = document.validate_with_context(context);

    assert!(has_issue(&report, "sbol3-12501", Severity::Error));
}

#[test]
fn file_resolver_can_validate_local_attachment_content() {
    let path = std::env::temp_dir().join(format!(
        "sbol-rs-{}-{}.txt",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::write(&path, b"hello").unwrap();
    let source = format!("file://{}", path.display());
    let hash = Sha3_256::digest(b"hello")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let document = read(&format!(
        r#":attachment a sbol:Attachment;
    sbol:displayId "attachment";
    sbol:hasNamespace <https://example.org>;
    sbol:source <{source}>;
    sbol:size "5";
    sbol:hash "{hash}";
    sbol:hashAlgorithm "sha3-256" .
"#
    ));
    let resolver = sbol3::FileResolver::new();
    let context = ValidationContext::new()
        .with_external_mode(ExternalValidationMode::ProvidedOnly)
        .with_content_resolver(&resolver);

    let report = document.validate_with_context(context);

    fs::remove_file(path).unwrap();
    assert!(!has_issue(&report, "sbol3-12801", Severity::Error));
    assert!(!has_issue(&report, "sbol3-12804", Severity::Error));
    assert!(!has_issue(&report, "sbol3-12805", Severity::Error));
}

#[test]
fn document_set_reports_duplicate_identities() {
    let first = read(
        r#":component a sbol:Component;
    sbol:displayId "component";
    sbol:hasNamespace <https://example.org>;
    sbol:type SBO:0000251 .
"#,
    );
    let second = read(
        r#":component a sbol:Component;
    sbol:displayId "component";
    sbol:hasNamespace <https://example.org>;
    sbol:type SBO:0000251 .
"#,
    );

    let error = DocumentSet::from_documents([&first, &second]).unwrap_err();

    assert_eq!(
        error.identity(),
        &Resource::iri("https://example.org/component")
    );
}

fn read(body: &str) -> Document {
    Document::read_turtle(&format!("{PREFIXES}{body}")).unwrap()
}

fn has_issue(report: &sbol3::ValidationReport, rule: &str, severity: Severity) -> bool {
    report
        .issues()
        .iter()
        .any(|issue| issue.rule == rule && issue.severity == severity)
}
