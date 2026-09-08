use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use sbol_inventory::{
    ConformanceClass, PROFILE_RULE_CATALOG_IRI, PROFILE_RULE_CATALOG_STATUS,
    PROFILE_RULE_CATALOG_VERSION, RuleStrength, profile_rules,
};
use sbol3::RdfGraph;
use serde::Deserialize;

#[derive(Deserialize)]
struct SourceCatalog {
    profile: String,
    version: String,
    status: String,
    rules: Vec<SourceRule>,
}

#[derive(Deserialize)]
struct SourceRule {
    id: String,
    section: String,
    classes: Vec<String>,
    strength: String,
    machine_checkable: bool,
    shacl_core: bool,
    fixture: Option<String>,
    statement: String,
}

#[test]
fn checked_in_catalog_matches_pinned_profile() {
    let source = fs::read_to_string(profile_root().join("rules.toml")).unwrap();
    let mut source: SourceCatalog = toml::from_str(&source).unwrap();
    source.rules.sort_by(|left, right| left.id.cmp(&right.id));

    let rules = profile_rules();
    let identifiers: BTreeSet<_> = rules.iter().map(|rule| rule.id).collect();

    assert_eq!(PROFILE_RULE_CATALOG_IRI, source.profile);
    assert_eq!(PROFILE_RULE_CATALOG_VERSION, source.version);
    assert_eq!(PROFILE_RULE_CATALOG_STATUS, source.status);
    assert_eq!(rules.len(), 49);
    assert_eq!(rules.len(), source.rules.len());
    assert_eq!(identifiers.len(), rules.len());
    assert_eq!(
        rules
            .iter()
            .filter(|rule| rule.applies_to(ConformanceClass::Validator))
            .count(),
        45
    );
    assert!(rules.iter().all(|rule| rule.id.starts_with("sbolinv-")));

    for (rule, source) in rules.iter().zip(source.rules) {
        let classes: Vec<_> = source
            .classes
            .iter()
            .map(|class| match class.as_str() {
                "Reader" => ConformanceClass::Reader,
                "Writer" => ConformanceClass::Writer,
                "Validator" => ConformanceClass::Validator,
                "Query" => ConformanceClass::Query,
                other => panic!("unknown conformance class `{other}`"),
            })
            .collect();
        let strength = match source.strength.as_str() {
            "required" => RuleStrength::Required,
            "recommended" => RuleStrength::Recommended,
            other => panic!("unknown rule strength `{other}`"),
        };

        assert_eq!(rule.id, source.id);
        assert_eq!(rule.section, source.section);
        assert_eq!(rule.classes, classes);
        assert_eq!(rule.strength, strength);
        assert_eq!(rule.machine_checkable, source.machine_checkable);
        assert_eq!(rule.shacl_core, source.shacl_core);
        assert_eq!(rule.fixture, source.fixture.as_deref());
        assert_eq!(rule.statement, source.statement);
    }
}

#[test]
fn normative_rdf_artifacts_parse() {
    for artifact in ["vocabulary.ttl", "shapes.ttl"] {
        let source = fs::read_to_string(profile_root().join(artifact)).unwrap();
        let graph = RdfGraph::parse_turtle(&source).unwrap();
        assert!(!graph.triples().is_empty(), "{artifact} was empty");
    }
}

fn profile_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("profile/0.2")
}
