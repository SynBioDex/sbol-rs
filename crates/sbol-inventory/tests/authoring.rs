use sbol_inventory::vocabulary::{ControlMode, INSTRUMENT, LIQUID_HANDLING, Qualification, ROOM};
use sbol_inventory::{
    AssetBuilder, CandidateQuery, CapabilityBuilder, FacilityBuilder, InventoryBuildError,
    InventoryBuilder, MaterialLotBuilder, PropertyValueBuilder, ZoneBuilder,
};
use sbol3::constants::SBO_DNA;
use sbol3::{Component, Iri, Literal, Namespace, RdfFormat, SbolObject, Term};

const NS: &str = "https://example.org/authored-inventory";

#[test]
fn typed_builders_compose_profile_and_ordinary_sbol_objects() {
    let mut builder = InventoryBuilder::new(Namespace::new(NS).unwrap());
    let facility = builder
        .add_facility(
            FacilityBuilder::new("facility")
                .unwrap()
                .name("Example facility"),
        )
        .unwrap();
    let temperature = PropertyValueBuilder::real(
        "temperature",
        Iri::new("https://example.org/terms/AmbientTemperature").unwrap(),
        21.0,
    )
    .unwrap()
    .unit(Iri::new("http://qudt.org/vocab/unit/DEG_C").unwrap());
    let zone = builder
        .add_zone(
            ZoneBuilder::new("room", facility.clone(), Iri::new(ROOM).unwrap())
                .unwrap()
                .name("Room")
                .active(true)
                .condition(temperature),
        )
        .unwrap();

    let capacity = PropertyValueBuilder::integer(
        "channel_count",
        Iri::new("https://example.org/terms/ChannelCount").unwrap(),
        8,
    )
    .unwrap();
    let capability = CapabilityBuilder::new("liquid_handling", Iri::new(LIQUID_HANDLING).unwrap())
        .unwrap()
        .qualification(Qualification::Executable)
        .control_mode(ControlMode::Api)
        .active(true)
        .parameter(capacity);
    let asset = builder
        .add_asset(
            AssetBuilder::new("handler", facility.clone(), Iri::new(INSTRUMENT).unwrap())
                .unwrap()
                .name("Liquid handler")
                .located_in(zone.clone())
                .manufacturer("Example Instruments")
                .model("LH-8")
                .active(true)
                .capability(capability)
                .annotation(
                    Iri::new("https://example.org/terms/catalogSource").unwrap(),
                    Term::Literal(Literal::simple("reviewed")),
                ),
        )
        .unwrap();

    let design = Component::builder(NS, "design")
        .unwrap()
        .types([SBO_DNA.clone()])
        .build()
        .unwrap();
    builder
        .add_sbol_object(SbolObject::Component(design.clone()))
        .unwrap();
    builder
        .add_material_lot(
            MaterialLotBuilder::new(
                "lot",
                facility,
                Iri::new("https://draggon.org/ns/inventory#BacterialStock").unwrap(),
                &design,
            )
            .unwrap()
            .located_in(zone)
            .barcode("LOT-001")
            .active(true),
        )
        .unwrap();

    let inventory = builder.finish();
    let checked = inventory.check().unwrap();
    assert_eq!(checked.facilities().count(), 1);
    assert_eq!(checked.zones().count(), 1);
    assert_eq!(checked.assets().count(), 1);
    assert_eq!(checked.material_lots().count(), 1);
    assert_eq!(checked.as_sbol_document().components().count(), 1);
    assert_eq!(
        checked
            .asset(&asset.as_resource())
            .unwrap()
            .as_object()
            .first_literal_value("https://example.org/terms/catalogSource"),
        Some("reviewed")
    );

    let query = CandidateQuery::new(Iri::new(LIQUID_HANDLING).unwrap(), Qualification::Plannable);
    assert_eq!(checked.find_qualified_assets(&query).len(), 1);

    let turtle = inventory.write(RdfFormat::Turtle).unwrap();
    let reread = sbol_inventory::InventoryDocument::read(&turtle, RdfFormat::Turtle).unwrap();
    assert!(reread.check().is_ok());
}

#[test]
fn builders_reject_incomplete_or_ambiguous_owned_objects_transactionally() {
    let mut builder = InventoryBuilder::new(Namespace::new(NS).unwrap());
    let facility = builder
        .add_facility(FacilityBuilder::new("facility").unwrap())
        .unwrap();

    let error = builder
        .add_zone(
            ZoneBuilder::new(
                "inactive_unknown",
                facility.clone(),
                Iri::new(ROOM).unwrap(),
            )
            .unwrap(),
        )
        .unwrap_err();
    assert!(matches!(error, InventoryBuildError::MissingRequired { .. }));

    let bad_property = PropertyValueBuilder::text(
        "label",
        Iri::new("https://example.org/terms/Label").unwrap(),
        "value",
    )
    .unwrap()
    .unit(Iri::new("http://qudt.org/vocab/unit/M").unwrap());
    let error = builder
        .add_zone(
            ZoneBuilder::new("room", facility.clone(), Iri::new(ROOM).unwrap())
                .unwrap()
                .active(true)
                .condition(bad_property),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        InventoryBuildError::UnitOnNonNumericValue { .. }
    ));

    let duplicate = || {
        CapabilityBuilder::new("same", Iri::new(LIQUID_HANDLING).unwrap())
            .unwrap()
            .qualification(Qualification::Plannable)
            .control_mode(ControlMode::Manual)
            .active(true)
    };
    let error = builder
        .add_asset(
            AssetBuilder::new("handler", facility, Iri::new(INSTRUMENT).unwrap())
                .unwrap()
                .active(true)
                .capability(duplicate())
                .capability(duplicate()),
        )
        .unwrap_err();
    assert!(matches!(error, InventoryBuildError::DuplicateIdentity(_)));

    assert_eq!(builder.finish().facilities().count(), 1);
}
