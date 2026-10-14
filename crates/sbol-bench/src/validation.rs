//! Untimed diagnostics retained alongside validation samples.

pub(crate) fn summary(report: &sbol::v3::ValidationReport) -> serde_json::Value {
    serde_json::json!({
        "errors": report.errors().count(),
        "warnings": report.warnings().count(),
        "issues": report.issues().iter().map(|issue| serde_json::json!({
            "rule": issue.rule,
            "severity": format!("{:?}", issue.severity),
            "message": issue.message,
        })).collect::<Vec<_>>(),
        "fully_applied": report.coverage().fully_applied,
        "partially_applied": report.coverage().partially_applied.iter().map(|entry|
            serde_json::json!({"rule": entry.rule, "blocker": format!("{:?}", entry.blocker),
                "coverage_kind": format!("{:?}", entry.coverage_kind)})
        ).collect::<Vec<_>>(),
        "not_applied": report.coverage().not_applied.iter().map(|entry|
            serde_json::json!({"rule": entry.rule, "reason": format!("{:?}", entry.reason)})
        ).collect::<Vec<_>>(),
        "config": "complete=true, compliant=true, best_practice=true; default options; external resolution off",
        "ontology": "bundled offline registry",
    })
}
