//! Public-data SBOLInventory catalog shaped around Caltech's EBEF.
//!
//! This is an architectural example, not an operational source of truth. It
//! intentionally omits serial numbers, network details, booking state,
//! calibration records, private layout, and execution claims.

use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

use sbol_inventory::vocabulary::*;
use sbol_inventory::{
    AssetBuilder, CapabilityBuilder, FacilityBuilder, FacilityId, InventoryBuildError,
    InventoryBuilder, InventoryDocument, PropertyScalar, PropertyValueBuilder, ZoneBuilder,
};
use sbol3::{Iri, Namespace, RdfFormat, Resource, Term};

const SOURCE: &str =
    "https://resnick.caltech.edu/resource-centers/ecology-and-biosphere-engineering-facility-ebef";
const SOURCE_ACCESSED: &str = "2026-08-26";
const DEFAULT_NAMESPACE: &str = "https://example.org/ebef";
const PROPERTY_NAMESPACE: &str = "https://example.org/ebef/property";
const DEG_C: &str = "http://qudt.org/vocab/unit/DEG_C";
const PERCENT: &str = "http://qudt.org/vocab/unit/PERCENT";

pub fn build_ebef_catalog() -> Result<InventoryDocument, Box<dyn Error>> {
    let mut catalog = InventoryBuilder::new(Namespace::new(DEFAULT_NAMESPACE)?);
    let facility = catalog.add_facility(
        FacilityBuilder::new("facility")?
            .name("Resnick Ecology and Biosphere Engineering Facility")
            .description(format!(
                "Public-data example catalog for a multi-user BSL2+ microbiology, microscopy, and plant cultivation facility; source accessed {SOURCE_ACCESSED}."
            ))
            .annotation(
                Iri::from_static(PROV_WAS_DERIVED_FROM),
                Term::Resource(Resource::Iri(Iri::new(SOURCE)?)),
            ),
    )?;

    let basement_main_lab = catalog
        .add_zone(zone("basement_main_lab", &facility, ROOM)?.name("Main lab (basement)"))?;
    let microbiology = catalog.add_zone(
        zone("microbiology_lab", &facility, WORK_AREA)?
            .parent(basement_main_lab.clone())
            .name("Microbiology lab (basement)"),
    )?;
    let microscopy = catalog.add_zone(
        zone("microscopy_lab", &facility, WORK_AREA)?
            .parent(basement_main_lab.clone())
            .name("Microscopy lab (basement)"),
    )?;
    let media_prep = catalog.add_zone(
        zone("media_prep_room", &facility, WORK_AREA)?
            .parent(basement_main_lab.clone())
            .name("Media preparation room"),
    )?;
    let freezer_room = catalog.add_zone(
        zone("freezer_room", &facility, STORAGE_ZONE)?
            .parent(basement_main_lab)
            .name("Freezer room"),
    )?;
    let plant_lab =
        catalog.add_zone(zone("plant_lab", &facility, ROOM)?.name("Plant lab (first floor)"))?;

    let chamber_1_interior = catalog.add_zone(
        zone("anaerobic_chamber_1_interior", &facility, CONTAINMENT_ZONE)?
            .parent(microbiology.clone())
            .name("Anaerobic chamber 1 interior")
            .condition(property(
                "nitrogen_fraction",
                PropertyScalar::Real(95.0),
                Some(PERCENT),
            )?)
            .condition(property(
                "hydrogen_fraction",
                PropertyScalar::Real(5.0),
                Some(PERCENT),
            )?)
            .condition(property(
                "maximum_added_co2",
                PropertyScalar::Real(20.0),
                Some(PERCENT),
            )?),
    )?;
    let chamber_2_interior = catalog.add_zone(
        zone("anaerobic_chamber_2_interior", &facility, CONTAINMENT_ZONE)?
            .parent(microbiology.clone())
            .name("Anaerobic chamber 2 interior")
            .condition(property(
                "nitrogen_fraction",
                PropertyScalar::Real(95.0),
                Some(PERCENT),
            )?)
            .condition(property(
                "hydrogen_fraction",
                PropertyScalar::Real(5.0),
                Some(PERCENT),
            )?)
            .condition(property(
                "maximum_added_co2",
                PropertyScalar::Real(20.0),
                Some(PERCENT),
            )?),
    )?;

    catalog.add_asset(
        asset("anaerobic_chamber_1", ENVIRONMENT_CONTROLLER)?
            .name("Anaerobic chamber 1")
            .located_in(microbiology.clone())
            .establishes_zone(chamber_1_interior.clone())
            .manufacturer("Coy Laboratory Products")
            .model("Vinyl anaerobic chamber")
            .capability(described(
                "anaerobic_environment_control",
                ANAEROBIC_ENVIRONMENT_CONTROL,
                vec![],
            )?),
    )?;
    catalog.add_asset(
        asset("anaerobic_chamber_2", ENVIRONMENT_CONTROLLER)?
            .name("Anaerobic chamber 2")
            .located_in(microbiology.clone())
            .establishes_zone(chamber_2_interior.clone())
            .manufacturer("Coy Laboratory Products")
            .model("Extra-wide vinyl anaerobic chamber")
            .capability(described(
                "anaerobic_environment_control",
                ANAEROBIC_ENVIRONMENT_CONTROL,
                vec![],
            )?),
    )?;
    catalog.add_asset(
        asset("microlab_prep", INSTRUMENT)?
            .name("Anaerobic liquid handler")
            .located_in(chamber_1_interior.clone())
            .manufacturer("Hamilton")
            .model("Microlab Prep")
            .capability(described(
                "liquid_handling",
                LIQUID_HANDLING,
                vec![
                    property("supported_plate_wells", PropertyScalar::Integer(96), None)?,
                    property(
                        "supports_serial_dilution",
                        PropertyScalar::Boolean(true),
                        None,
                    )?,
                ],
            )?),
    )?;
    catalog.add_asset(
        asset("potentiostat_96_well", INSTRUMENT)?
            .name("96-well potentiostat")
            .located_in(chamber_1_interior)
            .capability(described(
                "electrochemical_measurement",
                ELECTROCHEMICAL_MEASUREMENT,
                vec![property(
                    "supported_plate_wells",
                    PropertyScalar::Integer(96),
                    None,
                )?],
            )?),
    )?;
    catalog.add_asset(
        asset("anaerobic_swinging_bucket_centrifuge", INSTRUMENT)?
            .name("Anaerobic swinging-bucket centrifuge")
            .located_in(chamber_2_interior)
            .capability(described("centrifugation", CENTRIFUGATION, vec![])?),
    )?;

    catalog.add_asset(
        asset("dragonfly_confocal", INSTRUMENT)?
            .name("Dragonfly spinning disk confocal microscope")
            .located_in(microscopy.clone())
            .model("Dragonfly spinning disk confocal")
            .capability(described(
                "confocal_microscopy",
                CONFOCAL_MICROSCOPY,
                vec![
                    property("camera_pixels_x", PropertyScalar::Integer(2048), None)?,
                    property("camera_pixels_y", PropertyScalar::Integer(2048), None)?,
                    property("supports_timelapse", PropertyScalar::Boolean(true), None)?,
                ],
            )?),
    )?;
    catalog.add_asset(
        asset("plasma_cleaner", INSTRUMENT)?
            .name("Plasma cleaner")
            .located_in(microscopy)
            .capability(described("plasma_cleaning", PLASMA_CLEANING, vec![])?),
    )?;

    for index in 1..=3 {
        let refrigerates = matches!(index, 2 | 3);
        let mut parameters = vec![
            property(
                "supports_refrigeration",
                PropertyScalar::Boolean(refrigerates),
                None,
            )?,
            property(
                "supports_photosynthetic_lighting",
                PropertyScalar::Boolean(refrigerates),
                None,
            )?,
        ];
        if index == 1 {
            parameters.push(property(
                "minimum_temperature",
                PropertyScalar::Real(30.0),
                Some(DEG_C),
            )?);
        }
        catalog.add_asset(
            asset(&format!("eppendorf_s44i_{index}"), INSTRUMENT)?
                .name(format!("Shaking incubator {index}"))
                .located_in(microbiology.clone())
                .manufacturer("Eppendorf")
                .model("S44i")
                .capability(described(
                    "shaking_incubation",
                    SHAKING_INCUBATION,
                    parameters,
                )?),
        )?;
    }
    catalog.add_asset(
        asset("static_incubator_group", WORKSTATION)?
            .name("Static incubators")
            .description(
                "Public page describes several units; individual asset IDs are not public.",
            )
            .located_in(microbiology.clone())
            .capability(described(
                "static_incubation",
                STATIC_INCUBATION,
                vec![
                    property(
                        "minimum_temperature",
                        PropertyScalar::Real(17.0),
                        Some(DEG_C),
                    )?,
                    property(
                        "maximum_temperature",
                        PropertyScalar::Real(70.0),
                        Some(DEG_C),
                    )?,
                ],
            )?),
    )?;
    catalog.add_asset(
        asset("biotek_epoch_2", INSTRUMENT)?
            .name("Epoch 2 plate reader")
            .located_in(microbiology.clone())
            .manufacturer("Agilent BioTek")
            .model("Epoch 2")
            .capability(described(
                "absorbance_measurement",
                ABSORBANCE_MEASUREMENT,
                vec![
                    property("supported_plate_wells", PropertyScalar::Integer(96), None)?,
                    property(
                        "supports_fluorescence",
                        PropertyScalar::Boolean(false),
                        None,
                    )?,
                ],
            )?)
            .capability(described(
                "incubation",
                INCUBATION,
                vec![property(
                    "maximum_temperature",
                    PropertyScalar::Real(65.0),
                    Some(DEG_C),
                )?],
            )?),
    )?;

    let proflex = catalog.add_asset(
        asset("proflex", INSTRUMENT)?
            .name("ProFlex thermocycler")
            .description("Composite parent; independently runnable blocks are child assets.")
            .located_in(microbiology.clone())
            .model("ProFlex PCR System"),
    )?;
    for index in 1..=3 {
        catalog.add_asset(
            asset(&format!("proflex_block_{index}"), FUNCTIONAL_UNIT)?
                .name(format!("ProFlex independent block {index}"))
                .part_of(proflex.clone())
                .capability(described(
                    "thermal_cycling",
                    THERMAL_CYCLING,
                    vec![property(
                        "temperature_zones",
                        PropertyScalar::Integer(2),
                        None,
                    )?],
                )?),
        )?;
    }

    catalog.add_asset(
        asset("azure_300", INSTRUMENT)?
            .name("Gel imager")
            .located_in(microbiology.clone())
            .model("Azure 300")
            .capability(described("gel_imaging", GEL_IMAGING, vec![])?),
    )?;
    catalog.add_asset(
        asset("electrophoresis_station", WORKSTATION)?
            .name("DNA and protein electrophoresis station")
            .located_in(microbiology.clone())
            .capability(described("electrophoresis", ELECTROPHORESIS, vec![])?),
    )?;
    catalog.add_asset(
        asset("media_prep_station", WORKSTATION)?
            .name("Media and buffer preparation station")
            .located_in(media_prep)
            .capability(described("media_preparation", MEDIA_PREPARATION, vec![])?)
            .capability(described("ph_measurement", PH_MEASUREMENT, vec![])?)
            .capability(described("water_purification", WATER_PURIFICATION, vec![])?),
    )?;
    catalog.add_asset(
        asset("cold_storage_group", STORAGE_ASSET)?
            .name("Publicly documented cold-storage units")
            .description("Placeholder group pending identifiers for each reservable unit.")
            .located_in(freezer_room)
            .capability(described(
                "cold_storage",
                COLD_STORAGE,
                vec![property(
                    "documented_temperatures",
                    PropertyScalar::Text("4 C, -20 C, and -70 C".to_owned()),
                    None,
                )?],
            )?),
    )?;
    catalog.add_asset(
        asset("main_biosafety_cabinet", INSTRUMENT)?
            .name("Main-lab biosafety cabinet")
            .located_in(microbiology.clone())
            .capability(described(
                "biosafety_containment",
                BIOSAFETY_CONTAINMENT,
                vec![property("width_feet", PropertyScalar::Real(6.0), None)?],
            )?),
    )?;
    catalog.add_asset(
        asset("amsco_630ls", INSTRUMENT)?
            .name("Large basement autoclave")
            .located_in(microbiology)
            .model("AMSCO 630LS")
            .capability(described(
                "steam_sterilization",
                STEAM_STERILIZATION,
                vec![],
            )?),
    )?;

    let plant_chambers = [
        ("conviron_gen1000_1", "Gen1000", "Plant chamber Gen1000 1"),
        ("conviron_gen1000_2", "Gen1000", "Plant chamber Gen1000 2"),
        ("conviron_gen2000", "Gen2000", "Plant chamber Gen2000"),
        ("conviron_gr48", "GR48", "Walk-in plant chamber"),
    ];
    for (identity, model, name) in plant_chambers {
        let interior = catalog.add_zone(
            zone(&format!("{identity}_interior"), &facility, ENVIRONMENT_ZONE)?
                .parent(plant_lab.clone())
                .name(format!("{name} interior")),
        )?;
        catalog.add_asset(
            asset(identity, ENVIRONMENT_CONTROLLER)?
                .name(name)
                .located_in(plant_lab.clone())
                .establishes_zone(interior)
                .manufacturer("Conviron")
                .model(model)
                .capability(described(
                    "plant_growth",
                    PLANT_GROWTH,
                    vec![
                        property(
                            "programmable_temperature",
                            PropertyScalar::Boolean(true),
                            None,
                        )?,
                        property("programmable_light", PropertyScalar::Boolean(true), None)?,
                        property("additive_co2", PropertyScalar::Boolean(true), None)?,
                        property("additive_humidity", PropertyScalar::Boolean(true), None)?,
                    ],
                )?),
        )?;
    }
    catalog.add_asset(
        asset("plant_biosafety_cabinet", INSTRUMENT)?
            .name("Plant-lab biosafety cabinet")
            .located_in(plant_lab.clone())
            .capability(described(
                "biosafety_containment",
                BIOSAFETY_CONTAINMENT,
                vec![property("width_feet", PropertyScalar::Real(4.0), None)?],
            )?),
    )?;
    catalog.add_asset(
        asset("plant_autoclave", INSTRUMENT)?
            .name("Plant-lab soil and waste autoclave")
            .located_in(plant_lab)
            .capability(described(
                "steam_sterilization",
                STEAM_STERILIZATION,
                vec![property(
                    "optional_effluent_decontamination",
                    PropertyScalar::Boolean(true),
                    None,
                )?],
            )?),
    )?;

    Ok(catalog.finish())
}

fn zone(
    display_id: &str,
    facility: &FacilityId,
    kind: &'static str,
) -> Result<ZoneBuilder, InventoryBuildError> {
    Ok(ZoneBuilder::new(display_id, facility.clone(), Iri::from_static(kind))?.active(true))
}

fn asset(display_id: &str, kind: &'static str) -> Result<AssetBuilder, InventoryBuildError> {
    Ok(AssetBuilder::new(display_id, Iri::from_static(kind))?.active(true))
}

fn described(
    display_id: &str,
    kind: &'static str,
    parameters: Vec<PropertyValueBuilder>,
) -> Result<CapabilityBuilder, InventoryBuildError> {
    let mut capability = CapabilityBuilder::new(display_id, Iri::from_static(kind))?
        .qualification(Qualification::Described)
        .control_mode(ControlMode::Unspecified)
        .active(true);
    for parameter in parameters {
        capability = capability.parameter(parameter);
    }
    Ok(capability)
}

fn property(
    display_id: &str,
    value: PropertyScalar,
    unit: Option<&'static str>,
) -> Result<PropertyValueBuilder, InventoryBuildError> {
    let kind = Iri::new(format!("{PROPERTY_NAMESPACE}/{display_id}"))
        .expect("static property namespace produces absolute IRIs");
    let mut property = PropertyValueBuilder::new(display_id, kind, value)?;
    if let Some(unit) = unit {
        property = property.unit(Iri::from_static(unit));
    }
    Ok(property)
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: cargo run -p sbol-inventory --example ebef_catalog -- OUTPUT.ttl",
            )
        })?;
    let inventory = build_ebef_catalog()?;
    inventory.check().map_err(Box::<dyn Error>::from)?;
    fs::write(&output, inventory.write(RdfFormat::Turtle)?)?;
    println!(
        "Wrote {} SBOLInventory objects to {}",
        inventory.as_sbol_document().objects().len(),
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use sbol_inventory::CandidateQuery;

    use super::*;

    #[test]
    fn public_catalog_validates_and_exercises_nested_capacity() {
        let inventory = build_ebef_catalog().unwrap();
        let checked = inventory.check().unwrap();

        assert_eq!(checked.facilities().count(), 1);
        assert_eq!(checked.zones().count(), 12);
        assert_eq!(checked.assets().count(), 28);
        assert_eq!(checked.capability_offerings().count(), 30);

        let chamber = checked
            .asset(&Resource::iri(format!(
                "{DEFAULT_NAMESPACE}/anaerobic_chamber_1"
            )))
            .unwrap();
        let interior = checked
            .zone(&Resource::iri(format!(
                "{DEFAULT_NAMESPACE}/anaerobic_chamber_1_interior"
            )))
            .unwrap();
        let prep = checked
            .asset(&Resource::iri(format!("{DEFAULT_NAMESPACE}/microlab_prep")))
            .unwrap();
        assert_eq!(
            chamber.established_zone_ids().next(),
            Some(interior.identity())
        );
        assert_eq!(prep.located_in_id(), Some(interior.identity()));

        let thermal =
            CandidateQuery::new(Iri::from_static(THERMAL_CYCLING), Qualification::Described);
        assert_eq!(checked.find_qualified_assets(&thermal).len(), 3);
        let liquid =
            CandidateQuery::new(Iri::from_static(LIQUID_HANDLING), Qualification::Described);
        let liquid = checked.find_qualified_assets(&liquid);
        assert_eq!(liquid.len(), 1);
        assert_eq!(
            liquid[0].offering().control_mode(),
            Some(ControlMode::Unspecified)
        );
        let plannable =
            CandidateQuery::new(Iri::from_static(LIQUID_HANDLING), Qualification::Plannable);
        assert!(checked.find_qualified_assets(&plannable).is_empty());

        let turtle = inventory.write(RdfFormat::Turtle).unwrap();
        let reread = InventoryDocument::read(&turtle, RdfFormat::Turtle).unwrap();
        assert!(reread.check().is_ok());
        assert!(
            inventory
                .as_sbol_document()
                .rdf_graph()
                .is_isomorphic_with(reread.as_sbol_document().rdf_graph())
                .unwrap()
        );
    }
}
