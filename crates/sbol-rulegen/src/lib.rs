//! Build-time code generator that turns a versioned SBOL validation
//! `rules.toml` into Rust rule-catalog source.
//!
//! The `sbol2` and `sbol3` crates each call [`generate`] from their `build.rs`,
//! pointing at their own `rules.toml`. This keeps one generator, one set of
//! catalog invariants, and one emitted shape across both data-model versions.
//!
//! Cold-compile cost on a fresh `target/` dir is roughly 30s because of the
//! `toml`+`serde` dep stack. Subsequent builds reuse the cached deps; a caller
//! re-runs the script only when its `rules.toml`, `build.rs`, or policies
//! directory changes (the caller emits the `cargo:rerun-if-changed` directives).
//!
//! Outputs into `out_dir`:
//!   - `rule_catalog.rs`: `VALIDATION_RULE_STATUSES` slice literal, sorted by
//!     rule id for diff stability.
//!   - `rule_spec_meta.rs`: the `VALIDATION_RULE_SPEC_*` constants sourced from
//!     the TOML `[meta]` block.
//!
//! Failures (TOML parse error, unknown enum variant, missing required field,
//! missing policy ADR) panic with a message that names the offending rule id
//! and field, so a build failure points straight at the bad entry.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    meta: Meta,
    #[serde(default, rename = "rule")]
    rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Meta {
    spec_version: String,
    spec_path: String,
    spec_canonical_url: String,
    spec_pdf_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    id: String,
    status: String,
    normative_severity: String,
    spec_section: String,
    note: String,
    #[serde(default)]
    blocker: Option<String>,
    #[serde(default)]
    validator_function: Option<String>,
    /// Optional per-rule coverage tag. When set, overrides the
    /// default inferred from `status`/`blocker`.
    #[serde(default)]
    coverage_kind: Option<String>,
    /// Optional validation family. Absent means `Always`.
    #[serde(default)]
    gate: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileCatalog {
    profile: String,
    version: String,
    status: String,
    #[serde(default)]
    rules: Vec<RawProfileRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfileRule {
    id: String,
    section: String,
    classes: Vec<String>,
    strength: String,
    machine_checkable: bool,
    shacl_core: bool,
    #[serde(default)]
    fixture: Option<String>,
    statement: String,
}

/// Generates `rule_catalog.rs` and `rule_spec_meta.rs` into `out_dir` from the
/// `rules.toml` at `rules_toml`.
///
/// When `policies_dir` is `Some`, every rule whose blocker is `Policy` must
/// have a matching `<rule-id>.md` ADR file in that directory; a missing ADR
/// panics. Pass `None` to skip the check (for example when building from a
/// packaged crates.io tarball, where the workspace policies directory is
/// absent).
///
/// Panics on any catalog error, naming the offending rule and field.
pub fn generate(rules_toml: &Path, out_dir: &Path, policies_dir: Option<&Path>) {
    let text = match fs::read_to_string(rules_toml) {
        Ok(t) => t,
        Err(err) => panic!("failed to read {}: {err}", rules_toml.display()),
    };
    let catalog: Catalog = match toml::from_str(&text) {
        Ok(c) => c,
        Err(err) => panic!("failed to parse {}: {err}", rules_toml.display()),
    };

    let mut rules = catalog.rules;
    rules.sort_by(|a, b| a.id.cmp(&b.id));
    for rule in &rules {
        validate_status(&rule.id, &rule.status);
        validate_severity(&rule.id, &rule.normative_severity);
        validate_blocker(&rule.id, &rule.status, rule.blocker.as_deref());
        validate_coverage_kind(&rule.id, rule.coverage_kind.as_deref());
        validate_gate(&rule.id, rule.gate.as_deref());
        if let Some(policies_dir) = policies_dir
            && rule.blocker.as_deref() == Some("Policy")
        {
            let adr_path = policies_dir.join(format!("{}.md", rule.id));
            if !adr_path.exists() {
                panic!(
                    "rule {}: blocker = \"Policy\" requires {}/{}.md (not found at {})",
                    rule.id,
                    policies_dir.display(),
                    rule.id,
                    adr_path.display()
                );
            }
        }
    }

    write_rule_catalog(&out_dir.join("rule_catalog.rs"), &rules);
    write_spec_meta(&out_dir.join("rule_spec_meta.rs"), &catalog.meta);
}

/// Generates the typed catalog for an SBOL extension profile.
///
/// Profile catalogs deliberately carry different metadata from the core SBOL
/// rule catalogs: conformance classes, normative strength, SHACL coverage,
/// and an optional fixture. Keeping a separate entry point preserves both
/// schemas without translating profile meaning into core implementation
/// statuses.
pub fn generate_profile(rules_toml: &Path, out_dir: &Path) {
    let text = fs::read_to_string(rules_toml)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", rules_toml.display()));
    let catalog: ProfileCatalog = toml::from_str(&text)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", rules_toml.display()));

    if catalog.profile.trim().is_empty() {
        panic!("profile catalog has an empty profile IRI");
    }
    if catalog.version.trim().is_empty() {
        panic!("profile catalog has an empty version");
    }
    if !matches!(catalog.status.as_str(), "draft" | "published") {
        panic!(
            "profile catalog has invalid status `{}` (expected draft or published)",
            catalog.status
        );
    }

    let mut rules = catalog.rules;
    rules.sort_by(|left, right| left.id.cmp(&right.id));
    let mut ids = BTreeSet::new();
    for rule in &rules {
        if !ids.insert(&rule.id) {
            panic!("duplicate profile rule id `{}`", rule.id);
        }
        if rule.id.trim().is_empty() {
            panic!("profile rule has an empty id");
        }
        if rule.classes.is_empty() {
            panic!("profile rule {} has no conformance classes", rule.id);
        }
        for class in &rule.classes {
            profile_class_variant(&rule.id, class);
        }
        profile_strength_variant(&rule.id, &rule.strength);
        if let Some(fixture) = &rule.fixture {
            let fixture_path = rules_toml
                .parent()
                .expect("rules.toml has a parent directory")
                .join(fixture);
            if !fixture_path.is_file() {
                panic!(
                    "profile rule {} fixture `{}` does not exist at {}",
                    rule.id,
                    fixture,
                    fixture_path.display()
                );
            }
        }
    }

    write_profile_catalog(
        &out_dir.join("profile_rule_catalog.rs"),
        &catalog.profile,
        &catalog.version,
        &catalog.status,
        &rules,
    );
}

fn profile_class_variant(rule_id: &str, class: &str) -> &'static str {
    match class {
        "Reader" => "Reader",
        "Writer" => "Writer",
        "Validator" => "Validator",
        "Query" => "Query",
        other => panic!(
            "profile rule {rule_id}: invalid conformance class `{other}` (expected Reader, Writer, Validator, or Query)"
        ),
    }
}

fn profile_strength_variant(rule_id: &str, strength: &str) -> &'static str {
    match strength {
        "required" => "Required",
        "recommended" => "Recommended",
        other => panic!(
            "profile rule {rule_id}: invalid strength `{other}` (expected required or recommended)"
        ),
    }
}

fn write_profile_catalog(
    path: &Path,
    profile: &str,
    version: &str,
    status: &str,
    rules: &[RawProfileRule],
) {
    use std::fmt::Write;

    let mut buf = String::new();
    buf.push_str("// Generated by sbol-rulegen from rules.toml. Do not edit by hand.\n");
    writeln!(
        buf,
        "pub const PROFILE_RULE_CATALOG_IRI: &str = {};",
        rust_string_literal(profile)
    )
    .unwrap();
    writeln!(
        buf,
        "pub const PROFILE_RULE_CATALOG_VERSION: &str = {};",
        rust_string_literal(version)
    )
    .unwrap();
    writeln!(
        buf,
        "pub const PROFILE_RULE_CATALOG_STATUS: &str = {};",
        rust_string_literal(status)
    )
    .unwrap();
    buf.push_str("pub const PROFILE_RULES: &[ProfileRule] = &[\n");
    for rule in rules {
        buf.push_str("    ProfileRule::new(\n");
        writeln!(buf, "        {},", rust_string_literal(&rule.id)).unwrap();
        writeln!(buf, "        {},", rust_string_literal(&rule.section)).unwrap();
        buf.push_str("        &[\n");
        for class in &rule.classes {
            writeln!(
                buf,
                "            ConformanceClass::{},",
                profile_class_variant(&rule.id, class)
            )
            .unwrap();
        }
        buf.push_str("        ],\n");
        writeln!(
            buf,
            "        RuleStrength::{},",
            profile_strength_variant(&rule.id, &rule.strength)
        )
        .unwrap();
        writeln!(buf, "        {},", rule.machine_checkable).unwrap();
        writeln!(buf, "        {},", rule.shacl_core).unwrap();
        match &rule.fixture {
            Some(fixture) => {
                writeln!(buf, "        Some({}),", rust_string_literal(fixture)).unwrap()
            }
            None => buf.push_str("        None,\n"),
        }
        writeln!(buf, "        {},", rust_string_literal(&rule.statement)).unwrap();
        buf.push_str("    ),\n");
    }
    buf.push_str("];\n");
    fs::write(path, buf).unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
}

const VALID_STATUSES: &[&str] = &[
    "Error",
    "Warning",
    "Configurable",
    "MachineUncheckable",
    "Unimplemented",
];

fn validate_status(rule_id: &str, status: &str) -> &'static str {
    for valid in VALID_STATUSES {
        if status == *valid {
            return valid;
        }
    }
    panic!("rule {rule_id}: invalid status `{status}` (expected one of {VALID_STATUSES:?})");
}

fn validate_severity(rule_id: &str, severity: &str) {
    match severity {
        "MUST" | "SHOULD" | "MAY" => {}
        other => panic!(
            "rule {rule_id}: invalid normative_severity `{other}` (expected MUST, SHOULD, or MAY)"
        ),
    }
}

fn validate_coverage_kind(rule_id: &str, coverage_kind: Option<&str>) {
    if let Some(value) = coverage_kind {
        match value {
            "OntologyKnownTermsOnly"
            | "LocalReferencesOnly"
            | "LexicalShapeOnly"
            | "PolicyDefaultUndecided" => {}
            other => panic!(
                "rule {rule_id}: invalid coverage_kind `{other}` (expected \
                 OntologyKnownTermsOnly, LocalReferencesOnly, LexicalShapeOnly, \
                 or PolicyDefaultUndecided)"
            ),
        }
    }
}

fn validate_gate(rule_id: &str, gate: Option<&str>) {
    if let Some(value) = gate {
        match value {
            "Always" | "Compliant" | "Complete" | "BestPractice" => {}
            other => panic!(
                "rule {rule_id}: invalid gate `{other}` (expected Always, Compliant, \
                 Complete, or BestPractice)"
            ),
        }
    }
}

fn validate_blocker(rule_id: &str, status: &str, blocker: Option<&str>) {
    // `Error` and `Warning` are unconditional algorithms with no blocker.
    // Every other status carries a blocker that names the configuration
    // axis (Configurable), the spec context (MachineUncheckable), or
    // what's needed to implement (Unimplemented).
    let needs_blocker = !matches!(status, "Error" | "Warning");
    match (needs_blocker, blocker) {
        (true, None) => {
            panic!("rule {rule_id}: status `{status}` requires a `blocker = \"...\"` entry")
        }
        (false, Some(b)) => panic!(
            "rule {rule_id}: status `{status}` must not have a `blocker` entry (found `{b}`)"
        ),
        (true, Some(b)) => match b {
            "Ontology" | "Resolver" | "StrictDatatype" | "Policy" | "External" => {}
            other => panic!(
                "rule {rule_id}: invalid blocker `{other}` (expected Ontology, Resolver, \
                 StrictDatatype, Policy, or External)"
            ),
        },
        (false, None) => {}
    }
}

fn write_rule_catalog(path: &Path, rules: &[RawRule]) {
    use std::fmt::Write;
    let mut buf = String::new();
    buf.push_str(
        "// Generated by sbol-rulegen from rules.toml. Do not edit by hand.\n\
         const VALIDATION_RULE_STATUSES: &[ValidationRuleStatus] = &[\n",
    );
    for rule in rules {
        let status = validate_status(&rule.id, &rule.status);
        let severity = match rule.normative_severity.as_str() {
            "MUST" => "Must",
            "SHOULD" => "Should",
            "MAY" => "May",
            other => panic!("rule {}: invalid severity `{other}`", rule.id),
        };
        writeln!(buf, "    ValidationRuleStatus::new(").unwrap();
        writeln!(buf, "        {},", rust_string_literal(&rule.id)).unwrap();
        writeln!(buf, "        RuleStatus::{status},").unwrap();
        writeln!(buf, "        NormativeSeverity::{severity},").unwrap();
        writeln!(buf, "        {},", rust_string_literal(&rule.spec_section)).unwrap();
        writeln!(buf, "        {},", rust_string_literal(&rule.note)).unwrap();
        match rule.blocker.as_deref() {
            Some(b) => writeln!(buf, "        Some(super::Blocker::{b}),").unwrap(),
            None => writeln!(buf, "        None,").unwrap(),
        }
        match rule.validator_function.as_deref() {
            Some(fn_name) => {
                writeln!(buf, "        Some({}),", rust_string_literal(fn_name)).unwrap()
            }
            None => writeln!(buf, "        None,").unwrap(),
        }
        match rule.coverage_kind.as_deref() {
            Some(kind) => writeln!(buf, "        Some(super::CoverageKind::{kind}),").unwrap(),
            None => writeln!(buf, "        None,").unwrap(),
        }
        let gate = rule.gate.as_deref().unwrap_or("Always");
        writeln!(buf, "        super::ValidationGate::{gate},").unwrap();
        writeln!(buf, "    ),").unwrap();
    }
    buf.push_str("];\n");
    fs::write(path, buf).unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
}

fn write_spec_meta(path: &Path, meta: &Meta) {
    use std::fmt::Write;
    let mut buf = String::new();
    buf.push_str("// Generated by sbol-rulegen from rules.toml. Do not edit by hand.\n");
    writeln!(
        buf,
        "pub const VALIDATION_RULE_SPEC_VERSION: &str = {};",
        rust_string_literal(&meta.spec_version)
    )
    .unwrap();
    writeln!(
        buf,
        "pub const VALIDATION_RULE_SPEC_PATH: &str = {};",
        rust_string_literal(&meta.spec_path)
    )
    .unwrap();
    writeln!(
        buf,
        "pub const VALIDATION_RULE_SPEC_CANONICAL_URL: &str = {};",
        rust_string_literal(&meta.spec_canonical_url)
    )
    .unwrap();
    writeln!(
        buf,
        "pub const VALIDATION_RULE_SPEC_PDF_SHA256: &str = {};",
        rust_string_literal(&meta.spec_pdf_sha256)
    )
    .unwrap();
    fs::write(path, buf).unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
}

fn rust_string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                write!(out, "\\u{{{:x}}}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
