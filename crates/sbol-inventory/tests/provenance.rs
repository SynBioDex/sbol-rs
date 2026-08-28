use sbol_inventory::vocabulary::{
    DERIVED_FROM_MATERIAL, INSTRUMENT, PROV_HAD_ROLE, PROV_WAS_GENERATED_BY, RUN_ASSET,
    RUN_INPUT_MATERIAL,
};
use sbol_inventory::{
    AssetBuilder, FacilityBuilder, InventoryBuilder, MaterialLotBuilder, RunBuildError, RunBuilder,
};
use sbol3::constants::SBO_FUNCTIONAL_ENTITY;
use sbol3::{
    Agent, Attachment, Component, ExperimentalData, Iri, Namespace, Plan, Resource, SbolObject,
};

const NS: &str = "https://example.org/run-ledger";

#[test]
fn run_builder_records_assets_inputs_outputs_evidence_and_responsibility() {
    let mut builder = InventoryBuilder::new(Namespace::new(NS).unwrap());
    let facility = builder
        .add_facility(FacilityBuilder::new("facility").unwrap())
        .unwrap();
    let asset = builder
        .add_asset(
            AssetBuilder::new(
                "workstation",
                facility.clone(),
                Iri::new(INSTRUMENT).unwrap(),
            )
            .unwrap()
            .active(true),
        )
        .unwrap();
    let design = Component::builder(NS, "design")
        .unwrap()
        .types([SBO_FUNCTIONAL_ENTITY.clone()])
        .build()
        .unwrap();
    builder
        .add_sbol_object(SbolObject::Component(design.clone()))
        .unwrap();
    let input = builder
        .add_material_lot(
            MaterialLotBuilder::new(
                "input",
                facility.clone(),
                Iri::new("https://sbol.io/ns/inventory#BacterialStock").unwrap(),
                &design,
            )
            .unwrap()
            .active(true),
        )
        .unwrap();
    let output = builder
        .add_material_lot(
            MaterialLotBuilder::new(
                "output",
                facility,
                Iri::new("https://sbol.io/ns/inventory#BacterialStock").unwrap(),
                &design,
            )
            .unwrap()
            .derived_from(input.clone())
            .active(true),
        )
        .unwrap();

    let attachment = Attachment::builder(NS, "evidence_file")
        .unwrap()
        .source(Resource::iri("https://example.org/results/growth.csv"))
        .format(Iri::new("http://edamontology.org/format_3752").unwrap())
        .build()
        .unwrap();
    let evidence = ExperimentalData::builder(NS, "evidence")
        .unwrap()
        .add_attachment(attachment.identity.clone())
        .build()
        .unwrap();
    let plan = Plan::builder(NS, "reviewed_plan")
        .unwrap()
        .name("Reviewed plan")
        .build()
        .unwrap();
    let agent = Agent::builder(NS, "operator")
        .unwrap()
        .name("Operator")
        .build()
        .unwrap();
    builder
        .add_sbol_object(SbolObject::Attachment(attachment))
        .unwrap();
    builder
        .add_sbol_object(SbolObject::ExperimentalData(evidence.clone()))
        .unwrap();
    builder
        .add_sbol_object(SbolObject::Plan(plan.clone()))
        .unwrap();
    builder
        .add_sbol_object(SbolObject::Agent(agent.clone()))
        .unwrap();

    let run = builder
        .add_run(
            RunBuilder::new("run_1")
                .unwrap()
                .name("Growth run")
                .started_at_time("2026-08-27T09:00:00Z")
                .ended_at_time("2026-08-27T10:00:00Z")
                .asset(asset)
                .input_material(input.clone())
                .generated_material(output.clone())
                .evidence(&evidence)
                .responsibility(&plan, &agent),
        )
        .unwrap();

    let inventory = builder.finish();
    assert!(inventory.check().is_ok(), "{}", inventory.validate());
    let document = inventory.as_sbol_document();
    assert_eq!(document.activities().count(), 1);
    assert_eq!(document.usages().count(), 2);
    let roles: Vec<_> = document
        .usages()
        .flat_map(|usage| usage.had_role.iter().map(Iri::as_str))
        .collect();
    assert!(roles.contains(&RUN_ASSET));
    assert!(roles.contains(&RUN_INPUT_MATERIAL));

    let output_object = document.get(&output.as_resource()).unwrap();
    assert_eq!(
        output_object.first_resource(DERIVED_FROM_MATERIAL),
        Some(&input.as_resource())
    );
    assert_eq!(
        output_object.first_resource(PROV_WAS_GENERATED_BY),
        Some(&run.as_resource())
    );
    assert_eq!(
        document
            .get(&evidence.identity)
            .unwrap()
            .first_resource(PROV_WAS_GENERATED_BY),
        Some(&run.as_resource())
    );
    assert!(document.usages().all(|usage| !usage.had_role.is_empty()));
    assert!(
        document
            .objects()
            .values()
            .filter(|object| !object.values(PROV_HAD_ROLE).is_empty())
            .all(|object| object.values(PROV_HAD_ROLE).len() == 1)
    );
}

#[test]
fn run_builder_rejects_missing_assets_and_cross_builder_handles_before_mutation() {
    let mut builder = InventoryBuilder::new(Namespace::new(NS).unwrap());
    let error = builder
        .add_run(RunBuilder::new("empty_run").unwrap())
        .unwrap_err();
    assert_eq!(error, RunBuildError::RequiresAsset);

    let mut other = InventoryBuilder::new(Namespace::new("https://example.org/other").unwrap());
    let other_facility = other
        .add_facility(FacilityBuilder::new("facility").unwrap())
        .unwrap();
    let foreign_asset = other
        .add_asset(
            AssetBuilder::new("asset", other_facility, Iri::new(INSTRUMENT).unwrap())
                .unwrap()
                .active(true),
        )
        .unwrap();
    let error = builder
        .add_run(RunBuilder::new("foreign_run").unwrap().asset(foreign_asset))
        .unwrap_err();
    assert!(matches!(error, RunBuildError::MissingReferencedObject(_)));
    assert_eq!(builder.finish().as_sbol_document().activities().count(), 0);
}
