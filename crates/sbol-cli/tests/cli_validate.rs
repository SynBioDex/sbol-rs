//! End-to-end CLI tests: validate subcommand flag surface, exit codes, output formats.

mod common;
use common::*;

use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn version_flag_reports_package_version() {
    let out = Command::cargo_bin("sbol")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.starts_with(&format!("sbol {}", env!("CARGO_PKG_VERSION"))),
        "unexpected --version output: {text:?}"
    );
}

#[test]
fn exit_zero_on_clean_validation() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    Command::cargo_bin("sbol")
        .unwrap()
        .args(["validate", path.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn exit_one_on_error_severity_issue() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "bad.ttl", TTL_INVALID);
    Command::cargo_bin("sbol")
        .unwrap()
        .args(["validate", path.to_str().unwrap()])
        .assert()
        .code(1);
}

#[test]
fn exit_two_on_io_error() {
    Command::cargo_bin("sbol")
        .unwrap()
        .args(["validate", "/nonexistent/path/to/document.ttl"])
        .assert()
        .code(2);
}

#[test]
fn exit_two_on_unknown_rule_id_in_override() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    Command::cargo_bin("sbol")
        .unwrap()
        .args(["validate", path.to_str().unwrap(), "--allow", "sbol3-99999"])
        .assert()
        .code(2);
}

#[test]
fn allow_flag_suppresses_diagnostic_in_output() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "bad.ttl", TTL_INVALID);
    let assertion = Command::cargo_bin("sbol")
        .unwrap()
        .args([
            "validate",
            path.to_str().unwrap(),
            "--allow",
            "sbol3-10110",
            "--format",
            "json",
        ])
        .assert();
    let stdout = String::from_utf8(assertion.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let issues = value["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .all(|issue| issue["rule"].as_str() != Some("sbol3-10110")),
        "sbol3-10110 should not appear: {stdout}"
    );
}

#[test]
fn json_output_parses_as_json() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    let assertion = Command::cargo_bin("sbol")
        .unwrap()
        .args(["validate", path.to_str().unwrap(), "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assertion.get_output().stdout.clone()).unwrap();
    let trimmed = stdout.trim();
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .unwrap_or_else(|err| panic!("CLI JSON output failed to parse: {err}\n{trimmed}"));
    assert_eq!(value["schema_version"].as_u64().unwrap(), 1);
}

#[test]
fn sarif_output_is_available_in_the_standard_build() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    let assertion = Command::cargo_bin("sbol")
        .unwrap()
        .args(["validate", path.to_str().unwrap(), "--format", "sarif"])
        .assert()
        .success();
    let stdout = String::from_utf8(assertion.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|err| panic!("CLI SARIF output failed to parse: {err}\n{stdout}"));
    assert_eq!(value["version"].as_str(), Some("2.1.0"));
    assert!(value["runs"].as_array().is_some_and(|runs| runs.len() == 1));
}

#[test]
fn show_coverage_summary_appears_in_text_output() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    let assertion = Command::cargo_bin("sbol")
        .unwrap()
        .args(["validate", path.to_str().unwrap(), "--show-coverage"])
        .assert()
        .success();
    let stdout = String::from_utf8(assertion.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("coverage:"),
        "coverage summary missing: {stdout}"
    );
}

#[test]
fn treat_partial_as_errors_yields_exit_code_three_on_clean_document() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    Command::cargo_bin("sbol")
        .unwrap()
        .args([
            "validate",
            path.to_str().unwrap(),
            "--treat-partial-as-errors",
        ])
        .assert()
        .code(3);
}

#[test]
fn external_mode_allowed_without_cache_dir_errors() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    Command::cargo_bin("sbol")
        .unwrap()
        .args([
            "validate",
            path.to_str().unwrap(),
            "--external-mode",
            "allowed",
        ])
        .assert()
        .code(2);
}

#[test]
fn external_mode_provided_succeeds_with_resolve_documents() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    let docs_dir = dir.path().join("docs");
    std::fs::create_dir(&docs_dir).unwrap();
    Command::cargo_bin("sbol")
        .unwrap()
        .args([
            "validate",
            path.to_str().unwrap(),
            "--external-mode",
            "provided",
            "--resolve-documents",
            docs_dir.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn unknown_ontology_name_does_not_suggest_installing_it() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    let assertion = Command::cargo_bin("sbol")
        .unwrap()
        .args([
            "validate",
            path.to_str().unwrap(),
            "--ontology",
            "definitely-not-an-ontology",
        ])
        .assert()
        .code(2);
    let stderr = String::from_utf8(assertion.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("unknown ontology extension"),
        "expected unknown-ontology error, got: {stderr}"
    );
    assert!(
        !stderr.contains("install definitely-not-an-ontology"),
        "should not advise installing a name the CLI does not know: {stderr}"
    );
}

#[test]
fn text_output_prefixes_path_on_every_issue_line() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "bad.ttl", TTL_INVALID);
    let assertion = Command::cargo_bin("sbol")
        .unwrap()
        .args(["validate", path.to_str().unwrap()])
        .assert()
        .code(1);
    let stdout = String::from_utf8(assertion.get_output().stdout.clone()).unwrap();
    let path_str = path.display().to_string();
    let issue_lines: Vec<&str> = stdout
        .lines()
        .filter(|line| line.contains("error[") || line.contains("warning["))
        .collect();
    assert!(
        !issue_lines.is_empty(),
        "expected at least one issue line: {stdout}"
    );
    for line in &issue_lines {
        assert!(
            line.starts_with(&path_str),
            "issue line missing path prefix: {line}"
        );
    }
}

#[test]
fn output_file_receives_json() {
    let dir = TempDir::new().unwrap();
    let path = write_fixture(&dir, "ok.ttl", TTL_VALID);
    let out_path = dir.path().join("report.json");
    Command::cargo_bin("sbol")
        .unwrap()
        .args([
            "validate",
            path.to_str().unwrap(),
            "--format",
            "json",
            "--output",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    let contents = fs::read_to_string(&out_path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert!(value["coverage"].is_object());
}
