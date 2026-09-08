# sbol-inventory

Native Rust support for SBOLInventory Profile 0.2, layered on `sbol3`.

The crate treats serialized SBOL 3 RDF as the interoperability boundary. It exposes typed views and builders for facilities, zones, assets, capability offerings, property values, material lots, and standard PROV run records while preserving the complete underlying `sbol3::Document` and unknown extension triples.

SBOLInventory remains a profile layered on the SBOL 3 core object model. Workflows, requirement IR, allocation, scheduling, booking, device protocols, authorization, and dispatch remain consumer concerns.

## Conformance

`InventoryDocument::validate` runs the native SBOL 3.1 validator and all 45 required Profile 0.2 Validator rules. `InventoryDocument::check` returns a `ValidatedInventory`, which is the input boundary for deterministic candidate queries.

The crate vendors the versioned rule catalog, SHACL projection, vocabulary, specification, and 48 conformance fixtures. Its generated Rust rule catalog is checked in so published packages have no build-time dependency on an unreleased generator, and a source-catalog comparison test prevents drift. Valid fixtures are round-tripped through Turtle, RDF/XML, JSON-LD, and N-Triples and compared by RDF graph isomorphism.

## Authoring

`InventoryBuilder` composes profile objects with ordinary typed `sbol3` objects. Distinct identity types prevent accidental substitution of facilities, zones, assets, and material lots; `LocationId` permits only a Zone or Asset. Asset builders own capability builders, and capability or zone builders own exactly typed property values.

```rust
use sbol_inventory::prelude::*;
use sbol_inventory::vocabulary::{ControlMode, INSTRUMENT, LIQUID_HANDLING, Qualification, ROOM};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut catalog = InventoryBuilder::new(Namespace::new("https://example.org/my-lab")?);
    let facility = catalog.add_facility(FacilityBuilder::new("facility")?.name("My laboratory"))?;
    let room = catalog.add_zone(ZoneBuilder::new("main_lab", facility.clone(), Iri::new(ROOM)?)?.active(true))?;
    let liquid_handling = CapabilityBuilder::new("liquid_handling", Iri::new(LIQUID_HANDLING)?)?
        .qualification(Qualification::Described)
        .control_mode(ControlMode::Unspecified)
        .active(true);
    catalog.add_asset(
        AssetBuilder::new("liquid_handler", Iri::new(INSTRUMENT)?)?
            .located_in(room)
            .active(true)
            .capability(liquid_handling),
    )?;
    let inventory = catalog.finish();
    inventory.check()?;
    Ok(())
}
```

Candidate discovery filters exact capability kind, minimum qualification, facility, offering activity, and effective activity through both asset composition and zone containment. It returns candidates in Asset IRI order and does not claim allocation or executability.

`RunBuilder` emits standard `prov:Activity`, `prov:Usage`, and optional `prov:Association` objects. It records used assets, input lots, and input Components, links generated Components, material, and evidence with `prov:wasGeneratedBy`, and keeps material lineage on new SBOL Implementations.

## EBEF reference example

[`examples/ebef_catalog.rs`](examples/ebef_catalog.rs) is a public-data reference shape based on Caltech's [Ecology and Biosphere Engineering Facility equipment page](https://resnick.caltech.edu/resource-centers/ecology-and-biosphere-engineering-facility-ebef). It models 12 zones, 28 assets, and 30 capability offerings, including anaerobic chamber interiors, a Microlab Prep inside one controlled zone, an Epoch 2 with distinct capabilities, and three independently bindable ProFlex blocks.

The catalog is not an operational source of truth. It omits serial numbers, network details, booking state, calibration records, private layout, and any claim that Lab can execute an instrument. Public facts are represented at `Described`; the example intentionally returns no `Plannable` liquid-handler candidate.

```bash
cargo run -p sbol-inventory --example ebef_catalog -- ebef.ttl
```

## September 2026 draft revision

Only Zone builders take a Facility identity. `AssetBuilder::new(display_id, kind)` and `MaterialLotBuilder::new(display_id, kind, built)` take their location separately. Their `facility_id()` views derive membership through `locatedIn`, falling back to `partOf` for component assets without a direct location. Unlocated objects have no known facility and cannot match a facility filter. Old direct `fac:facility` edges on Assets or MaterialLots are validation errors.

`RunBuilder::input_component` and `generated_component` distinguish design use from design creation. Physical realization of a lot does not regenerate its existing Component. `DatabaseBuilder` creates a metadata or experimental-data repository through the corresponding `InventoryBuilder` method. `link_evidence` relates ExperimentalData to its Component and shares existing Attachment references. `record_data_submission`, `record_component_submission`, and `record_component_retrieval` record completed transfers. They perform no network requests. Readers can inspect repository objects and relationship identities through `InventoryDocument`.
