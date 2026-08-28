use std::fs;
use std::path::{Path, PathBuf};

use sbol_inventory::{
    CORE_VALIDATOR, InventoryDocument, PROFILE_RULE_CATALOG_IRI, PROFILE_RULE_CATALOG_VERSION,
    PROFILE_SOURCE_REVISION,
};
use sbol3::RdfFormat;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FixtureManifest {
    profile: String,
    version: String,
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    path: String,
    expect: String,
    rule: Option<String>,
    #[serde(default)]
    round_trip: bool,
}

#[test]
fn validation_report_identifies_both_conformance_layers() {
    let inventory =
        InventoryDocument::read_path(profile_root().join("fixtures/valid/minimal-catalog.ttl"))
            .unwrap();
    let report = inventory.validate();

    assert!(report.is_valid());
    assert_eq!(report.profile_iri(), PROFILE_RULE_CATALOG_IRI);
    assert_eq!(report.profile_version(), PROFILE_RULE_CATALOG_VERSION);
    assert_eq!(report.profile_status(), "draft");
    assert_eq!(report.profile_source_revision(), PROFILE_SOURCE_REVISION);
    assert_eq!(report.sbol_core_version(), sbol3::SPEC_VERSION);
    assert_eq!(report.core_validator(), CORE_VALIDATOR);
    assert_eq!(report.applied_profile_rules().count(), 41);
}

#[test]
fn owned_profile_links_must_resolve_to_their_declared_class() {
    let inventory = InventoryDocument::read(
        r#"@prefix ex: <https://example.org/wrong-owned-type/> .
@prefix fac: <https://sbol.io/ns/facility#> .
@prefix sbol: <http://sbols.org/v3#> .

ex:facility a sbol:TopLevel, fac:Facility ; sbol:displayId "facility" ; sbol:hasNamespace <https://example.org/wrong-owned-type> .
ex:asset a sbol:TopLevel, fac:Asset ; sbol:displayId "asset" ; sbol:hasNamespace <https://example.org/wrong-owned-type> ; fac:facility ex:facility ; fac:assetKind fac:Instrument ; fac:isActive true ; fac:capability ex:facility .
"#,
        RdfFormat::Turtle,
    )
    .unwrap();

    assert!(inventory.validate().has_rule_violation("sbolinv-10004"));
}

#[test]
fn every_vendored_fixture_matches_full_validator() {
    let manifest = fixture_manifest();
    assert_eq!(manifest.profile, PROFILE_RULE_CATALOG_IRI);
    assert_eq!(manifest.version, PROFILE_RULE_CATALOG_VERSION);
    assert_eq!(manifest.fixtures.len(), 43);

    for fixture in manifest.fixtures {
        let path = profile_root().join("fixtures").join(&fixture.path);
        let inventory = InventoryDocument::read_path(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let report = inventory.validate();

        match fixture.expect.as_str() {
            "valid" => {
                assert!(
                    report.is_valid(),
                    "{} unexpectedly failed:\n{report}",
                    fixture.path
                );
                assert!(inventory.check().is_ok());
                if fixture.round_trip {
                    assert_round_trip_in_all_formats(&fixture.path, &inventory);
                }
            }
            "invalid" => {
                let expected_rule = fixture.rule.as_deref().expect("invalid fixture rule");
                assert!(
                    report.has_rule_violation(expected_rule),
                    "{} did not report {expected_rule}; got:\n{report}",
                    fixture.path
                );
                assert!(inventory.check().is_err());
            }
            other => panic!("unknown fixture expectation {other:?}"),
        }
    }
}

fn assert_round_trip_in_all_formats(label: &str, inventory: &InventoryDocument) {
    for &format in RdfFormat::ALL {
        let serialized = inventory
            .write(format)
            .unwrap_or_else(|error| panic!("failed to write {label} as {format}: {error}"));
        let reread = InventoryDocument::read(&serialized, format)
            .unwrap_or_else(|error| panic!("failed to reread {label} as {format}: {error}"));
        let isomorphic = inventory
            .as_sbol_document()
            .rdf_graph()
            .is_isomorphic_with(reread.as_sbol_document().rdf_graph())
            .unwrap_or_else(|error| panic!("failed to compare {label} as {format}: {error}"));
        assert!(isomorphic, "{label} changed graph meaning in {format}");
        assert!(
            reread.validate().is_valid(),
            "{label} became invalid after {format} round trip"
        );
    }
}

fn fixture_manifest() -> FixtureManifest {
    let source = fs::read_to_string(profile_root().join("fixtures/manifest.toml")).unwrap();
    toml::from_str(&source).unwrap()
}

fn profile_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("profile/0.2")
}
