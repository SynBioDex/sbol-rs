//! Stable SBOLInventory Profile 0.2 vocabulary terms.

use thiserror::Error;

pub const INVENTORY_NS: &str = "https://draggon.org/ns/inventory#";
pub const FACILITY_NS: &str = "https://draggon.org/ns/facility#";
pub const CAPABILITY_NS: &str = "https://draggon.org/ns/capability#";
pub const PROFILE_VERSION: &str = "0.2";
pub const PROFILE_IRI: &str = "https://draggon.org/spec/sbol-inventory/0.2";

pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
pub const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";
pub const XSD_DOUBLE: &str = "http://www.w3.org/2001/XMLSchema#double";
pub const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";
pub const XSD_DATE_TIME: &str = "http://www.w3.org/2001/XMLSchema#dateTime";

pub const SBOL_IDENTIFIED: &str = "http://sbols.org/v3#Identified";
pub const SBOL_TOP_LEVEL: &str = "http://sbols.org/v3#TopLevel";
pub const SBOL_COMPONENT: &str = "http://sbols.org/v3#Component";
pub const SBOL_IMPLEMENTATION: &str = "http://sbols.org/v3#Implementation";
pub const SBOL_BUILT: &str = "http://sbols.org/v3#built";

pub const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
pub const PROV_USAGE: &str = "http://www.w3.org/ns/prov#Usage";
pub const PROV_ENTITY: &str = "http://www.w3.org/ns/prov#entity";
pub const PROV_HAD_ROLE: &str = "http://www.w3.org/ns/prov#hadRole";
pub const PROV_QUALIFIED_USAGE: &str = "http://www.w3.org/ns/prov#qualifiedUsage";
pub const PROV_WAS_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";
pub const PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";

pub const FACILITY: &str = "https://draggon.org/ns/facility#Facility";
pub const ZONE: &str = "https://draggon.org/ns/facility#Zone";
pub const ASSET: &str = "https://draggon.org/ns/facility#Asset";
pub const CAPABILITY_OFFERING: &str = "https://draggon.org/ns/facility#CapabilityOffering";
pub const PROPERTY_VALUE: &str = "https://draggon.org/ns/facility#PropertyValue";

pub const FACILITY_PROPERTY: &str = "https://draggon.org/ns/facility#facility";
pub const ZONE_KIND: &str = "https://draggon.org/ns/facility#zoneKind";
pub const PARENT_ZONE: &str = "https://draggon.org/ns/facility#parentZone";
pub const POLICY: &str = "https://draggon.org/ns/facility#policy";
pub const CONDITION: &str = "https://draggon.org/ns/facility#condition";
pub const IS_ACTIVE: &str = "https://draggon.org/ns/facility#isActive";

pub const ASSET_KIND: &str = "https://draggon.org/ns/facility#assetKind";
pub const LOCATED_IN: &str = "https://draggon.org/ns/facility#locatedIn";
pub const POSITION: &str = "https://draggon.org/ns/facility#position";
pub const PART_OF: &str = "https://draggon.org/ns/facility#partOf";
pub const ESTABLISHES_ZONE: &str = "https://draggon.org/ns/facility#establishesZone";
pub const MANUFACTURER: &str = "https://draggon.org/ns/facility#manufacturer";
pub const MODEL: &str = "https://draggon.org/ns/facility#model";
pub const SERIAL_NUMBER: &str = "https://draggon.org/ns/facility#serialNumber";
pub const ALLOWED_POSITION: &str = "https://draggon.org/ns/facility#allowedPosition";
pub const CAPABILITY: &str = "https://draggon.org/ns/facility#capability";

pub const CAPABILITY_KIND: &str = "https://draggon.org/ns/facility#capabilityKind";
pub const QUALIFICATION: &str = "https://draggon.org/ns/facility#qualification";
pub const CONTROL_MODE: &str = "https://draggon.org/ns/facility#controlMode";
pub const PARAMETER: &str = "https://draggon.org/ns/facility#parameter";

pub const PROPERTY_KIND: &str = "https://draggon.org/ns/facility#propertyKind";
pub const TEXT_VALUE: &str = "https://draggon.org/ns/facility#textValue";
pub const INTEGER_VALUE: &str = "https://draggon.org/ns/facility#integerValue";
pub const REAL_VALUE: &str = "https://draggon.org/ns/facility#realValue";
pub const BOOLEAN_VALUE: &str = "https://draggon.org/ns/facility#booleanValue";
pub const URI_VALUE: &str = "https://draggon.org/ns/facility#uriValue";
pub const UNIT: &str = "https://draggon.org/ns/facility#unit";

pub const MATERIAL_KIND: &str = "https://draggon.org/ns/facility#materialKind";
pub const DERIVED_FROM_MATERIAL: &str = "https://draggon.org/ns/facility#derivedFromMaterial";
pub const BARCODE: &str = "https://draggon.org/ns/facility#barcode";
pub const LOT_ID: &str = "https://draggon.org/ns/facility#lotId";
pub const NOTES: &str = "https://draggon.org/ns/facility#notes";
pub const FREEZE_DATE: &str = "https://draggon.org/ns/facility#freezeDate";

pub const RUN_ASSET: &str = "https://draggon.org/ns/facility#RunAsset";
pub const RUN_INPUT_MATERIAL: &str = "https://draggon.org/ns/facility#RunInputMaterial";

pub const ROOM: &str = "https://draggon.org/ns/facility#Room";
pub const WORK_AREA: &str = "https://draggon.org/ns/facility#WorkArea";
pub const CONTAINMENT_ZONE: &str = "https://draggon.org/ns/facility#ContainmentZone";
pub const ENVIRONMENT_ZONE: &str = "https://draggon.org/ns/facility#EnvironmentZone";
pub const STORAGE_ZONE: &str = "https://draggon.org/ns/facility#StorageZone";

pub const INSTRUMENT: &str = "https://draggon.org/ns/facility#Instrument";
pub const CONTAINER: &str = "https://draggon.org/ns/facility#Container";
pub const STORAGE_ASSET: &str = "https://draggon.org/ns/facility#StorageAsset";
pub const FUNCTIONAL_UNIT: &str = "https://draggon.org/ns/facility#FunctionalUnit";
pub const ENVIRONMENT_CONTROLLER: &str = "https://draggon.org/ns/facility#EnvironmentController";
pub const WORKSTATION: &str = "https://draggon.org/ns/facility#Workstation";

pub const LIQUID_HANDLING: &str = "https://draggon.org/ns/capability#LiquidHandling";
pub const ABSORBANCE_MEASUREMENT: &str = "https://draggon.org/ns/capability#AbsorbanceMeasurement";
pub const INCUBATION: &str = "https://draggon.org/ns/capability#Incubation";
pub const SHAKING_INCUBATION: &str = "https://draggon.org/ns/capability#ShakingIncubation";
pub const STATIC_INCUBATION: &str = "https://draggon.org/ns/capability#StaticIncubation";
pub const SHAKING: &str = "https://draggon.org/ns/capability#Shaking";
pub const THERMAL_CYCLING: &str = "https://draggon.org/ns/capability#ThermalCycling";
pub const ENVIRONMENT_CONTROL: &str = "https://draggon.org/ns/capability#EnvironmentControl";
pub const ANAEROBIC_ENVIRONMENT_CONTROL: &str =
    "https://draggon.org/ns/capability#AnaerobicEnvironmentControl";
pub const CONFOCAL_MICROSCOPY: &str = "https://draggon.org/ns/capability#ConfocalMicroscopy";
pub const PLASMA_CLEANING: &str = "https://draggon.org/ns/capability#PlasmaCleaning";
pub const ELECTROCHEMICAL_MEASUREMENT: &str =
    "https://draggon.org/ns/capability#ElectrochemicalMeasurement";
pub const GEL_IMAGING: &str = "https://draggon.org/ns/capability#GelImaging";
pub const ELECTROPHORESIS: &str = "https://draggon.org/ns/capability#Electrophoresis";
pub const CENTRIFUGATION: &str = "https://draggon.org/ns/capability#Centrifugation";
pub const MEDIA_PREPARATION: &str = "https://draggon.org/ns/capability#MediaPreparation";
pub const PH_MEASUREMENT: &str = "https://draggon.org/ns/capability#PhMeasurement";
pub const WATER_PURIFICATION: &str = "https://draggon.org/ns/capability#WaterPurification";
pub const BIOSAFETY_CONTAINMENT: &str = "https://draggon.org/ns/capability#BiosafetyContainment";
pub const STEAM_STERILIZATION: &str = "https://draggon.org/ns/capability#SteamSterilization";
pub const COLD_STORAGE: &str = "https://draggon.org/ns/capability#ColdStorage";
pub const PLANT_GROWTH: &str = "https://draggon.org/ns/capability#PlantGrowth";

/// Qualification of one installed capability offering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Qualification {
    Discovered,
    Described,
    Plannable,
    Simulatable,
    Executable,
    Qualified,
}

impl Qualification {
    pub const ALL: [Self; 6] = [
        Self::Discovered,
        Self::Described,
        Self::Plannable,
        Self::Simulatable,
        Self::Executable,
        Self::Qualified,
    ];

    pub const fn iri(self) -> &'static str {
        match self {
            Self::Discovered => "https://draggon.org/ns/facility#Discovered",
            Self::Described => "https://draggon.org/ns/facility#Described",
            Self::Plannable => "https://draggon.org/ns/facility#Plannable",
            Self::Simulatable => "https://draggon.org/ns/facility#Simulatable",
            Self::Executable => "https://draggon.org/ns/facility#Executable",
            Self::Qualified => "https://draggon.org/ns/facility#Qualified",
        }
    }
}

impl TryFrom<&str> for Qualification {
    type Error = UnknownVocabularyValue;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.iri() == value)
            .ok_or_else(|| UnknownVocabularyValue::qualification(value))
    }
}

/// Known interaction channel for one installed capability offering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControlMode {
    Unspecified,
    Manual,
    ReviewedFile,
    VendorSession,
    Api,
    Sila2,
    OpcUa,
}

impl ControlMode {
    pub const ALL: [Self; 7] = [
        Self::Unspecified,
        Self::Manual,
        Self::ReviewedFile,
        Self::VendorSession,
        Self::Api,
        Self::Sila2,
        Self::OpcUa,
    ];

    pub const fn iri(self) -> &'static str {
        match self {
            Self::Unspecified => "https://draggon.org/ns/facility#UnspecifiedControl",
            Self::Manual => "https://draggon.org/ns/facility#ManualControl",
            Self::ReviewedFile => "https://draggon.org/ns/facility#ReviewedFileControl",
            Self::VendorSession => "https://draggon.org/ns/facility#VendorSessionControl",
            Self::Api => "https://draggon.org/ns/facility#ApiControl",
            Self::Sila2 => "https://draggon.org/ns/facility#SiLA2Control",
            Self::OpcUa => "https://draggon.org/ns/facility#OpcUaControl",
        }
    }
}

impl TryFrom<&str> for ControlMode {
    type Error = UnknownVocabularyValue;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.iri() == value)
            .ok_or_else(|| UnknownVocabularyValue::control_mode(value))
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("unknown {kind} IRI `{value}`")]
pub struct UnknownVocabularyValue {
    kind: &'static str,
    value: String,
}

impl UnknownVocabularyValue {
    fn qualification(value: &str) -> Self {
        Self {
            kind: "qualification",
            value: value.to_owned(),
        }
    }

    fn control_mode(value: &str) -> Self {
        Self {
            kind: "control-mode",
            value: value.to_owned(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}
