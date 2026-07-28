//! SARIF v2.1.0 emitter for [`sbol::v3::ValidationReport`]. Maps each
//! validation issue to a SARIF `result` and exposes the catalog as
//! `runs[].tool.driver.rules[]`. Coverage metadata round-trips through
//! `runs[].invocations[0].properties.coverage` so SARIF consumers that
//! care can read it; consumers that don't (GitHub code-scanning) ignore
//! unknown `properties` keys per the SARIF spec.

use std::path::Path;

use sbol::v3::{
    Blocker, CoverageKind, NotAppliedReason, RuleStatus, Severity, ValidationIssue,
    ValidationReport, validation_rule_statuses,
};
use serde_json::{Map, Value, json};

pub fn to_sarif(report: &ValidationReport, input_path: &Path) -> String {
    let driver = json!({
        "name": "sbol-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "informationUri": "https://github.com/SynBioDex/sbol-rs",
        "semanticVersion": env!("CARGO_PKG_VERSION"),
        "rules": rules_descriptor(),
    });

    let mut invocation = Map::new();
    invocation.insert("executionSuccessful".into(), Value::Bool(true));
    invocation.insert(
        "properties".into(),
        json!({
            "coverage": coverage_properties(report),
            "appliedOptions": applied_options_properties(report),
        }),
    );

    let run = json!({
        "tool": { "driver": driver },
        "invocations": [Value::Object(invocation)],
        "results": report
            .issues()
            .iter()
            .map(|issue| issue_to_result(issue, input_path))
            .collect::<Vec<_>>(),
    });

    let document = json!({
        "version": "2.1.0",
        "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json",
        "runs": [run],
    });

    serde_json::to_string(&document).expect("SARIF JSON is always serializable")
}

fn rules_descriptor() -> Vec<Value> {
    validation_rule_statuses()
        .iter()
        .map(|status| {
            json!({
                "id": status.rule,
                "shortDescription": { "text": status.note },
                "helpUri": format!(
                    "https://sbolstandard.org/datamodel-specification/version-3.1.0/#{}",
                    status.spec_section
                ),
                "properties": {
                    "status": rule_status_label(status.status),
                    "blocker": status.blocker.map(blocker_label),
                    "specSection": status.spec_section,
                },
            })
        })
        .collect()
}

fn issue_to_result(issue: &ValidationIssue, input_path: &Path) -> Value {
    let level = match issue.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        _ => "none",
    };
    let mut logical_location = Map::new();
    logical_location.insert(
        "fullyQualifiedName".into(),
        Value::String(issue.subject.to_string()),
    );
    if let Some(property) = issue.property {
        logical_location.insert("decoratedName".into(), Value::String(property.into()));
    }

    json!({
        "ruleId": issue.rule,
        "level": level,
        "message": { "text": issue.message },
        "locations": [{
            "logicalLocations": [Value::Object(logical_location)],
            "physicalLocation": {
                "artifactLocation": {
                    "uri": input_path.to_string_lossy(),
                }
            }
        }],
    })
}

fn coverage_properties(report: &ValidationReport) -> Value {
    let coverage = report.coverage();
    json!({
        "fullyApplied": coverage.fully_applied,
        "partiallyApplied": coverage
            .partially_applied
            .iter()
            .map(|partial| {
                json!({
                    "rule": partial.rule,
                    "blocker": blocker_label(partial.blocker),
                    "coverageKind": coverage_kind_label(partial.coverage_kind),
                })
            })
            .collect::<Vec<_>>(),
        "notApplied": coverage
            .not_applied
            .iter()
            .map(|not_applied| {
                let reason = match not_applied.reason {
                    NotAppliedReason::MachineUncheckable => json!("MachineUncheckable"),
                    NotAppliedReason::Deferred(blocker) => json!({
                        "Deferred": blocker_label(blocker),
                    }),
                    _ => json!("Unknown"),
                };
                json!({
                    "rule": not_applied.rule,
                    "reason": reason,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn applied_options_properties(report: &ValidationReport) -> Value {
    let options = report.options_summary();
    json!({
        "topologyCompleteness": format!("{:?}", options.topology_completeness),
        "externalMode": format!("{:?}", options.external_mode),
        "documentResolvers": options.document_resolvers,
        "contentResolvers": options.content_resolvers,
        "severityFloor": options.severity_floor.map(severity_label),
        "severityCeiling": options.severity_ceiling.map(severity_label),
        "overriddenRules": options
            .overridden_rules
            .iter()
            .map(|(rule, ovr)| {
                json!({
                    "rule": rule,
                    "override": format!("{:?}", ovr),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn rule_status_label(status: RuleStatus) -> &'static str {
    match status {
        RuleStatus::Error => "Error",
        RuleStatus::Warning => "Warning",
        RuleStatus::Configurable => "Configurable",
        RuleStatus::MachineUncheckable => "MachineUncheckable",
        RuleStatus::Unimplemented => "Unimplemented",
        _ => "Unknown",
    }
}

fn blocker_label(blocker: Blocker) -> &'static str {
    match blocker {
        Blocker::Ontology => "Ontology",
        Blocker::Resolver => "Resolver",
        Blocker::StrictDatatype => "StrictDatatype",
        Blocker::Policy => "Policy",
        Blocker::External => "External",
    }
}

fn coverage_kind_label(kind: CoverageKind) -> &'static str {
    match kind {
        CoverageKind::OntologyKnownTermsOnly => "OntologyKnownTermsOnly",
        CoverageKind::LocalReferencesOnly => "LocalReferencesOnly",
        CoverageKind::LexicalShapeOnly => "LexicalShapeOnly",
        CoverageKind::PolicyDefaultUndecided => "PolicyDefaultUndecided",
        _ => "Unknown",
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "Error",
        Severity::Warning => "Warning",
        _ => "Unknown",
    }
}
