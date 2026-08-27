use sbol_inventory::vocabulary::{ABSORBANCE_MEASUREMENT, Qualification};
use sbol_inventory::{CandidateQuery, InventoryDocument};
use sbol3::{Iri, RdfFormat};

const CATALOG: &str = r#"@prefix cap: <https://draggon.org/ns/capability#> .
@prefix ex: <https://example.org/query/> .
@prefix fac: <https://draggon.org/ns/facility#> .
@prefix sbol: <http://sbols.org/v3#> .

ex:facility_a a sbol:TopLevel, fac:Facility ; sbol:displayId "facility_a" ; sbol:hasNamespace <https://example.org/query> .
ex:facility_b a sbol:TopLevel, fac:Facility ; sbol:displayId "facility_b" ; sbol:hasNamespace <https://example.org/query> .

ex:building a sbol:TopLevel, fac:Zone ; sbol:displayId "building" ; sbol:hasNamespace <https://example.org/query> ; fac:facility ex:facility_a ; fac:zoneKind fac:Room ; fac:isActive true ; ex:marker "building" .
ex:room a sbol:TopLevel, fac:Zone ; sbol:displayId "room" ; sbol:hasNamespace <https://example.org/query> ; fac:facility ex:facility_a ; fac:zoneKind fac:Room ; fac:parentZone ex:building ; fac:isActive true .
ex:other_room a sbol:TopLevel, fac:Zone ; sbol:displayId "other_room" ; sbol:hasNamespace <https://example.org/query> ; fac:facility ex:facility_b ; fac:zoneKind fac:Room ; fac:isActive true .

ex:container a sbol:TopLevel, fac:Asset ; sbol:displayId "container" ; sbol:hasNamespace <https://example.org/query> ; fac:facility ex:facility_a ; fac:assetKind fac:Container ; fac:locatedIn ex:room ; fac:isActive true ; ex:marker "container" .
ex:parent a sbol:TopLevel, fac:Asset ; sbol:displayId "parent" ; sbol:hasNamespace <https://example.org/query> ; fac:facility ex:facility_a ; fac:assetKind fac:Instrument ; fac:isActive true ; ex:marker "parent" .

ex:a_child a sbol:TopLevel, fac:Asset ; sbol:displayId "a_child" ; sbol:hasNamespace <https://example.org/query> ; fac:facility ex:facility_a ; fac:assetKind fac:FunctionalUnit ; fac:locatedIn ex:container ; fac:partOf ex:parent ; fac:isActive true ; fac:capability <https://example.org/query/a_child/absorbance> .
<https://example.org/query/a_child/absorbance> a sbol:Identified, fac:CapabilityOffering ; sbol:displayId "absorbance" ; fac:capabilityKind cap:AbsorbanceMeasurement ; fac:qualification fac:Plannable ; fac:controlMode fac:ReviewedFileControl ; fac:isActive true .

ex:b_direct a sbol:TopLevel, fac:Asset ; sbol:displayId "b_direct" ; sbol:hasNamespace <https://example.org/query> ; fac:facility ex:facility_a ; fac:assetKind fac:Instrument ; fac:locatedIn ex:room ; fac:isActive true ; fac:capability <https://example.org/query/b_direct/absorbance> .
<https://example.org/query/b_direct/absorbance> a sbol:Identified, fac:CapabilityOffering ; sbol:displayId "absorbance" ; fac:capabilityKind cap:AbsorbanceMeasurement ; fac:qualification fac:Qualified ; fac:controlMode fac:ApiControl ; fac:isActive true ; ex:marker "offering" .

ex:c_other a sbol:TopLevel, fac:Asset ; sbol:displayId "c_other" ; sbol:hasNamespace <https://example.org/query> ; fac:facility ex:facility_b ; fac:assetKind fac:Instrument ; fac:locatedIn ex:other_room ; fac:isActive true ; fac:capability <https://example.org/query/c_other/absorbance> .
<https://example.org/query/c_other/absorbance> a sbol:Identified, fac:CapabilityOffering ; sbol:displayId "absorbance" ; fac:capabilityKind cap:AbsorbanceMeasurement ; fac:qualification fac:Qualified ; fac:controlMode fac:ApiControl ; fac:isActive true .
"#;

#[test]
fn query_filters_by_kind_qualification_and_facility_then_orders_assets() {
    let document = read(CATALOG);
    let inventory = document.check().unwrap();
    let plannable = CandidateQuery::new(
        Iri::new(ABSORBANCE_MEASUREMENT).unwrap(),
        Qualification::Plannable,
    );
    let identities: Vec<_> = inventory
        .find_qualified_assets(&plannable)
        .iter()
        .map(|candidate| candidate.asset().identity().to_string())
        .collect();
    assert_eq!(
        identities,
        [
            "https://example.org/query/a_child",
            "https://example.org/query/b_direct",
            "https://example.org/query/c_other",
        ]
    );

    let facility_a = plannable
        .clone()
        .within_facility(Iri::new("https://example.org/query/facility_a").unwrap());
    assert_eq!(inventory.find_qualified_assets(&facility_a).len(), 2);

    let qualified = CandidateQuery::new(
        Iri::new(ABSORBANCE_MEASUREMENT).unwrap(),
        Qualification::Qualified,
    );
    assert_eq!(inventory.find_qualified_assets(&qualified).len(), 2);
}

#[test]
fn effective_activity_includes_zone_location_composition_and_offering_state() {
    let query = CandidateQuery::new(
        Iri::new(ABSORBANCE_MEASUREMENT).unwrap(),
        Qualification::Plannable,
    )
    .within_facility(Iri::new("https://example.org/query/facility_a").unwrap());

    for marker in ["building", "container", "parent", "offering"] {
        let source = CATALOG.replace(
            &format!("fac:isActive true ; ex:marker \"{marker}\""),
            &format!("fac:isActive false ; ex:marker \"{marker}\""),
        );
        let document = read(&source);
        let inventory = document.check().unwrap();
        let identities: Vec<_> = inventory
            .find_qualified_assets(&query)
            .iter()
            .map(|candidate| candidate.asset().identity().to_string())
            .collect();
        let expected: &[&str] = match marker {
            "building" => &[],
            "container" | "parent" => &["https://example.org/query/b_direct"],
            "offering" => &["https://example.org/query/a_child"],
            _ => unreachable!(),
        };
        assert_eq!(
            identities, expected,
            "unexpected candidates for inactive {marker}"
        );
    }
}

#[test]
fn query_input_errors_carry_stable_rule_ids() {
    let error = CandidateQuery::parse("relative-kind", Qualification::Plannable.iri()).unwrap_err();
    assert_eq!(error.rule(), "sbolinv-18002");

    let error = CandidateQuery::parse(
        ABSORBANCE_MEASUREMENT,
        "https://example.org/UnknownQualification",
    )
    .unwrap_err();
    assert_eq!(error.rule(), "sbolinv-18001");
}

fn read(source: &str) -> InventoryDocument {
    InventoryDocument::read(source, RdfFormat::Turtle).unwrap()
}
