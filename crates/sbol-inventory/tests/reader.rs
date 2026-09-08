use sbol_inventory::vocabulary::{ControlMode, LIQUID_HANDLING, Qualification, ROOM};
use sbol_inventory::{InventoryDocument, ScalarValueRef};
use sbol3::{RdfFormat, Resource};

const CATALOG: &str = r#"
@prefix cap: <https://sbol.io/ns/capability#> .
@prefix ex: <https://example.org/inventory/> .
@prefix fac: <https://sbol.io/ns/facility#> .
@prefix inv: <https://sbol.io/ns/inventory#> .
@prefix sbol: <http://sbols.org/v3#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

ex:facility a sbol:TopLevel, fac:Facility ; sbol:displayId "facility" ;
    sbol:hasNamespace <https://example.org/inventory> ; sbol:name "Example lab" .
ex:room a sbol:TopLevel, fac:Zone ; sbol:displayId "room" ;
    sbol:hasNamespace <https://example.org/inventory> ; fac:facility ex:facility ;
    fac:zoneKind fac:Room ; fac:isActive true .
ex:handler a sbol:TopLevel, fac:Asset ; sbol:displayId "handler" ;
    sbol:hasNamespace <https://example.org/inventory> ;
    fac:assetKind fac:Instrument ; fac:locatedIn ex:room ; fac:isActive true ;
    fac:manufacturer "Example" ; fac:capability <https://example.org/inventory/handler/liquid> .
<https://example.org/inventory/handler/liquid> a sbol:Identified, fac:CapabilityOffering ; sbol:displayId "liquid" ;
    fac:capabilityKind cap:LiquidHandling ; fac:qualification fac:Plannable ;
    fac:controlMode fac:ApiControl ; fac:isActive true ;
    fac:parameter <https://example.org/inventory/handler/liquid/channels> .
<https://example.org/inventory/handler/liquid/channels> a sbol:Identified, fac:PropertyValue ;
    sbol:displayId "channels" ; fac:propertyKind ex:ChannelCount ;
    fac:integerValue 8 ; ex:retained "yes" .
ex:design a sbol:Component ; sbol:displayId "design" ;
    sbol:hasNamespace <https://example.org/inventory> ;
    sbol:type <https://identifiers.org/SBO:0000251> .
ex:lot a sbol:Implementation ; sbol:displayId "lot" ;
    sbol:hasNamespace <https://example.org/inventory> ; sbol:built ex:design ;
    fac:materialKind inv:BacterialStock ;
    fac:locatedIn ex:room ; fac:isActive true .
ex:ordinary a sbol:Implementation ; sbol:displayId "ordinary" ;
    sbol:hasNamespace <https://example.org/inventory> ; sbol:built ex:design .
"#;

#[test]
fn profile_views_preserve_the_facility_graph() {
    let inventory = InventoryDocument::read(CATALOG, RdfFormat::Turtle).unwrap();

    let facility = inventory.facilities().next().unwrap();
    assert_eq!(facility.name(), Some("Example lab"));

    let room = inventory.zones().next().unwrap();
    assert_eq!(room.kind().unwrap().as_str(), ROOM);
    assert_eq!(room.facility().unwrap().identity(), facility.identity());
    assert_eq!(room.is_active(), Some(true));

    let handler = inventory.assets().next().unwrap();
    assert_eq!(handler.located_in_id(), Some(room.identity()));
    let offering = handler.capabilities().next().unwrap();
    assert_eq!(offering.kind().unwrap().as_str(), LIQUID_HANDLING);
    assert_eq!(offering.qualification(), Some(Qualification::Plannable));
    assert_eq!(offering.control_mode(), Some(ControlMode::Api));
    assert_eq!(
        offering.parameters().next().unwrap().value().unwrap(),
        ScalarValueRef::Integer("8")
    );

    let lot = inventory.material_lots().next().unwrap();
    assert!(lot.as_implementation().is_some());
    assert_eq!(lot.facility().unwrap().identity(), facility.identity());
    assert_eq!(inventory.material_lots().count(), 1);
}

#[test]
fn raw_sbol_and_extension_data_remain_available() {
    let inventory = InventoryDocument::read(CATALOG, RdfFormat::Turtle).unwrap();
    let parameter = inventory.property_values().next().unwrap();
    assert_eq!(
        parameter
            .as_object()
            .first_literal_value("https://example.org/inventory/retained"),
        Some("yes")
    );
    assert_eq!(inventory.as_sbol_document().components().count(), 1);

    let turtle = inventory.write(RdfFormat::Turtle).unwrap();
    let reparsed = InventoryDocument::read(&turtle, RdfFormat::Turtle).unwrap();
    assert_eq!(
        reparsed.as_sbol_document().rdf_graph().normalized_triples(),
        inventory
            .as_sbol_document()
            .rdf_graph()
            .normalized_triples()
    );

    assert!(
        reparsed
            .asset(&Resource::iri("https://example.org/inventory/handler"))
            .is_some()
    );
}

#[test]
fn closed_vocabulary_values_are_ordered_and_checked() {
    assert!(Qualification::Qualified > Qualification::Executable);
    assert_eq!(
        Qualification::try_from(Qualification::Described.iri()).unwrap(),
        Qualification::Described
    );
    assert!(Qualification::try_from("https://example.org/Unknown").is_err());
    assert!(ControlMode::try_from("https://example.org/Unknown").is_err());
}
