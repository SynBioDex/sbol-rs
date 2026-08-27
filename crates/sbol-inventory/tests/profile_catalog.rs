use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use sbol_inventory::{
    ConformanceClass, PROFILE_RULE_CATALOG_IRI, PROFILE_RULE_CATALOG_STATUS,
    PROFILE_RULE_CATALOG_VERSION, profile_rules,
};
use sbol3::RdfGraph;

#[test]
fn generated_catalog_matches_pinned_profile() {
    let rules = profile_rules();
    let identifiers: BTreeSet<_> = rules.iter().map(|rule| rule.id).collect();

    assert_eq!(
        PROFILE_RULE_CATALOG_IRI,
        "https://draggon.org/spec/sbol-inventory/0.2"
    );
    assert_eq!(PROFILE_RULE_CATALOG_VERSION, "0.2");
    assert_eq!(PROFILE_RULE_CATALOG_STATUS, "draft");
    assert_eq!(rules.len(), 45);
    assert_eq!(identifiers.len(), rules.len());
    assert_eq!(
        rules
            .iter()
            .filter(|rule| rule.applies_to(ConformanceClass::Validator))
            .count(),
        41
    );
    assert!(rules.iter().all(|rule| rule.id.starts_with("sbolinv-")));
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
