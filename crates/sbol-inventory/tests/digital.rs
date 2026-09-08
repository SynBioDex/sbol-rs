use sbol_inventory::vocabulary::*;
use sbol_inventory::{
    AssetBuilder, DatabaseBuilder, InventoryBuilder, InventoryDocument, RunBuilder,
};
use sbol3::constants::SBO_FUNCTIONAL_ENTITY;
use sbol3::{
    Attachment, Component, ExperimentalData, Iri, Namespace, RdfFormat, Resource, SbolObject,
};

const NS: &str = "https://example.org/digital-builder";

#[test]
fn designs_evidence_and_repository_links_are_authored_and_round_trip() {
    let mut builder = InventoryBuilder::new(Namespace::new(NS).unwrap());
    let reader = builder
        .add_asset(
            AssetBuilder::new("reader", Iri::from_static(INSTRUMENT))
                .unwrap()
                .active(true),
        )
        .unwrap();
    let design = Component::builder(NS, "design")
        .unwrap()
        .types([SBO_FUNCTIONAL_ENTITY])
        .build()
        .unwrap();
    let output = Component::builder(NS, "output")
        .unwrap()
        .types([SBO_FUNCTIONAL_ENTITY])
        .build()
        .unwrap();
    let attachment = Attachment::builder(NS, "file")
        .unwrap()
        .source(Resource::iri("https://example.org/results.csv"))
        .build()
        .unwrap();
    let data = ExperimentalData::builder(NS, "data")
        .unwrap()
        .add_attachment(attachment.identity.clone())
        .build()
        .unwrap();
    for object in [
        SbolObject::Component(design.clone()),
        SbolObject::Component(output.clone()),
        SbolObject::Attachment(attachment.clone()),
        SbolObject::ExperimentalData(data.clone()),
    ] {
        builder.add_sbol_object(object).unwrap();
    }
    let journal = builder
        .add_experimental_data_database(DatabaseBuilder::new("journal").unwrap())
        .unwrap();
    let registry = builder
        .add_metadata_database(DatabaseBuilder::new("registry").unwrap())
        .unwrap();
    builder.link_evidence(&data, &output).unwrap();
    builder.link_evidence(&data, &output).unwrap();
    builder.record_data_submission(&data, &journal).unwrap();
    builder
        .record_component_submission(&output, &registry)
        .unwrap();
    builder
        .record_component_retrieval(&design, &registry)
        .unwrap();
    let run = builder
        .add_run(
            RunBuilder::new("run")
                .unwrap()
                .asset(reader.clone())
                .input_component(&design)
                .generated_component(&output)
                .evidence(&data),
        )
        .unwrap();
    // Invalid generated-design identities must fail before adding an Activity.
    assert!(
        builder
            .add_run(
                RunBuilder::new("bad_run")
                    .unwrap()
                    .asset(reader.clone())
                    .generated_component_identity(reader.as_resource())
            )
            .is_err()
    );
    let inventory = builder.finish();
    assert!(inventory.validate().is_valid(), "{}", inventory.validate());
    assert!(
        inventory
            .as_sbol_document()
            .get(&Resource::iri(format!("{NS}/bad_run")))
            .is_none()
    );
    for &format in RdfFormat::ALL {
        let parsed = InventoryDocument::read(&inventory.write(format).unwrap(), format).unwrap();
        assert!(parsed.validate().is_valid(), "{}", parsed.validate());
        assert_eq!(
            parsed
                .evidence_component_ids(&data.identity)
                .collect::<Vec<_>>(),
            vec![&output.identity]
        );
        assert_eq!(
            parsed
                .submission_database_ids(&data.identity)
                .collect::<Vec<_>>(),
            vec![&journal.as_resource()]
        );
        assert_eq!(
            parsed
                .retrieval_database_ids(&design.identity)
                .collect::<Vec<_>>(),
            vec![&registry.as_resource()]
        );
        let graph = parsed.as_sbol_document();
        assert_eq!(
            graph
                .get(&output.identity)
                .unwrap()
                .resources(PROV_WAS_GENERATED_BY)
                .collect::<Vec<_>>(),
            vec![&run.as_resource()]
        );
        assert!(
            graph
                .get(&design.identity)
                .unwrap()
                .values(PROV_WAS_GENERATED_BY)
                .is_empty()
        );
        assert_eq!(
            graph
                .get(&output.identity)
                .unwrap()
                .resources(SBOL_HAS_ATTACHMENT)
                .collect::<Vec<_>>(),
            vec![&attachment.identity]
        );
        assert_eq!(parsed.metadata_databases().count(), 1);
        assert_eq!(parsed.experimental_data_databases().count(), 1);
    }
}

#[test]
fn nested_assets_and_materials_derive_facility_and_unknown_location_stays_unknown() {
    let source = include_str!("../profile/0.2/fixtures/valid/material-run.ttl");
    let nested = source.replace(
        "fac:locatedIn ex:room ;\n    fac:isActive true ;",
        "fac:locatedIn ex:workstation ;\n    fac:isActive true ;",
    );
    let inventory = InventoryDocument::read(&nested, RdfFormat::Turtle).unwrap();
    assert!(inventory.validate().is_valid(), "{}", inventory.validate());
    let ns = "https://example.org/sbolinventory-run";
    let lot = inventory
        .material_lot(&Resource::iri(format!("{ns}/output_lot")))
        .unwrap();
    assert_eq!(
        lot.facility_id(),
        Some(&Resource::iri(format!("{ns}/facility")))
    );
    assert!(lot.as_object().values(FACILITY_PROPERTY).is_empty());
    let unlocated = nested.replace("fac:locatedIn ex:room ;", "");
    let inventory = InventoryDocument::read(&unlocated, RdfFormat::Turtle).unwrap();
    assert!(
        inventory
            .facility_id_for(&Resource::iri(format!("{ns}/output_lot")))
            .is_none()
    );
}
