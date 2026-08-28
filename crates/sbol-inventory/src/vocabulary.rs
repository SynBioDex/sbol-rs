//! Stable SBOLInventory Profile 0.2 vocabulary terms.

use thiserror::Error;

pub const INVENTORY_NS: &str = "https://sbol.io/ns/inventory#";
pub const FACILITY_NS: &str = "https://sbol.io/ns/facility#";
pub const CAPABILITY_NS: &str = "https://sbol.io/ns/capability#";
pub const PROFILE_VERSION: &str = "0.2";
pub const PROFILE_IRI: &str = "https://sbol.io/spec/sbol-inventory/0.2";

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
pub const SBOL_DISPLAY_ID: &str = "http://sbols.org/v3#displayId";
pub const SBOL_HAS_NAMESPACE: &str = "http://sbols.org/v3#hasNamespace";
pub const SBOL_NAME: &str = "http://sbols.org/v3#name";
pub const SBOL_DESCRIPTION: &str = "http://sbols.org/v3#description";
pub const SBOL_BUILT: &str = "http://sbols.org/v3#built";

pub const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
pub const PROV_USAGE: &str = "http://www.w3.org/ns/prov#Usage";
pub const PROV_ENTITY: &str = "http://www.w3.org/ns/prov#entity";
pub const PROV_HAD_ROLE: &str = "http://www.w3.org/ns/prov#hadRole";
pub const PROV_QUALIFIED_USAGE: &str = "http://www.w3.org/ns/prov#qualifiedUsage";
pub const PROV_WAS_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";
pub const PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";

pub const FACILITY: &str = "https://sbol.io/ns/facility#Facility";
pub const ZONE: &str = "https://sbol.io/ns/facility#Zone";
pub const ASSET: &str = "https://sbol.io/ns/facility#Asset";
pub const CAPABILITY_OFFERING: &str = "https://sbol.io/ns/facility#CapabilityOffering";
pub const PROPERTY_VALUE: &str = "https://sbol.io/ns/facility#PropertyValue";

pub const FACILITY_PROPERTY: &str = "https://sbol.io/ns/facility#facility";
pub const ZONE_KIND: &str = "https://sbol.io/ns/facility#zoneKind";
pub const PARENT_ZONE: &str = "https://sbol.io/ns/facility#parentZone";
pub const POLICY: &str = "https://sbol.io/ns/facility#policy";
pub const CONDITION: &str = "https://sbol.io/ns/facility#condition";
pub const IS_ACTIVE: &str = "https://sbol.io/ns/facility#isActive";

pub const ASSET_KIND: &str = "https://sbol.io/ns/facility#assetKind";
pub const LOCATED_IN: &str = "https://sbol.io/ns/facility#locatedIn";
pub const POSITION: &str = "https://sbol.io/ns/facility#position";
pub const PART_OF: &str = "https://sbol.io/ns/facility#partOf";
pub const ESTABLISHES_ZONE: &str = "https://sbol.io/ns/facility#establishesZone";
pub const MANUFACTURER: &str = "https://sbol.io/ns/facility#manufacturer";
pub const MODEL: &str = "https://sbol.io/ns/facility#model";
pub const SERIAL_NUMBER: &str = "https://sbol.io/ns/facility#serialNumber";
pub const ALLOWED_POSITION: &str = "https://sbol.io/ns/facility#allowedPosition";
pub const CAPABILITY: &str = "https://sbol.io/ns/facility#capability";

pub const CAPABILITY_KIND: &str = "https://sbol.io/ns/facility#capabilityKind";
pub const QUALIFICATION: &str = "https://sbol.io/ns/facility#qualification";
pub const CONTROL_MODE: &str = "https://sbol.io/ns/facility#controlMode";
pub const PARAMETER: &str = "https://sbol.io/ns/facility#parameter";

pub const PROPERTY_KIND: &str = "https://sbol.io/ns/facility#propertyKind";
pub const TEXT_VALUE: &str = "https://sbol.io/ns/facility#textValue";
pub const INTEGER_VALUE: &str = "https://sbol.io/ns/facility#integerValue";
pub const REAL_VALUE: &str = "https://sbol.io/ns/facility#realValue";
pub const BOOLEAN_VALUE: &str = "https://sbol.io/ns/facility#booleanValue";
pub const URI_VALUE: &str = "https://sbol.io/ns/facility#uriValue";
pub const UNIT: &str = "https://sbol.io/ns/facility#unit";

pub const MATERIAL_KIND: &str = "https://sbol.io/ns/facility#materialKind";
pub const DERIVED_FROM_MATERIAL: &str = "https://sbol.io/ns/facility#derivedFromMaterial";
pub const BARCODE: &str = "https://sbol.io/ns/facility#barcode";
pub const LOT_ID: &str = "https://sbol.io/ns/facility#lotId";
pub const NOTES: &str = "https://sbol.io/ns/facility#notes";
pub const FREEZE_DATE: &str = "https://sbol.io/ns/facility#freezeDate";

pub const RUN_ASSET: &str = "https://sbol.io/ns/facility#RunAsset";
pub const RUN_INPUT_MATERIAL: &str = "https://sbol.io/ns/facility#RunInputMaterial";

pub const ROOM: &str = "https://sbol.io/ns/facility#Room";
pub const WORK_AREA: &str = "https://sbol.io/ns/facility#WorkArea";
pub const CONTAINMENT_ZONE: &str = "https://sbol.io/ns/facility#ContainmentZone";
pub const ENVIRONMENT_ZONE: &str = "https://sbol.io/ns/facility#EnvironmentZone";
pub const STORAGE_ZONE: &str = "https://sbol.io/ns/facility#StorageZone";

pub const INSTRUMENT: &str = "https://sbol.io/ns/facility#Instrument";
pub const CONTAINER: &str = "https://sbol.io/ns/facility#Container";
pub const STORAGE_ASSET: &str = "https://sbol.io/ns/facility#StorageAsset";
pub const FUNCTIONAL_UNIT: &str = "https://sbol.io/ns/facility#FunctionalUnit";
pub const ENVIRONMENT_CONTROLLER: &str = "https://sbol.io/ns/facility#EnvironmentController";
pub const WORKSTATION: &str = "https://sbol.io/ns/facility#Workstation";

pub const LIQUID_HANDLING: &str = "https://sbol.io/ns/capability#LiquidHandling";
pub const ABSORBANCE_MEASUREMENT: &str = "https://sbol.io/ns/capability#AbsorbanceMeasurement";
pub const INCUBATION: &str = "https://sbol.io/ns/capability#Incubation";
pub const SHAKING_INCUBATION: &str = "https://sbol.io/ns/capability#ShakingIncubation";
pub const STATIC_INCUBATION: &str = "https://sbol.io/ns/capability#StaticIncubation";
pub const SHAKING: &str = "https://sbol.io/ns/capability#Shaking";
pub const THERMAL_CYCLING: &str = "https://sbol.io/ns/capability#ThermalCycling";
pub const ENVIRONMENT_CONTROL: &str = "https://sbol.io/ns/capability#EnvironmentControl";
pub const ANAEROBIC_ENVIRONMENT_CONTROL: &str =
    "https://sbol.io/ns/capability#AnaerobicEnvironmentControl";
pub const CONFOCAL_MICROSCOPY: &str = "https://sbol.io/ns/capability#ConfocalMicroscopy";
pub const PLASMA_CLEANING: &str = "https://sbol.io/ns/capability#PlasmaCleaning";
pub const ELECTROCHEMICAL_MEASUREMENT: &str =
    "https://sbol.io/ns/capability#ElectrochemicalMeasurement";
pub const GEL_IMAGING: &str = "https://sbol.io/ns/capability#GelImaging";
pub const ELECTROPHORESIS: &str = "https://sbol.io/ns/capability#Electrophoresis";
pub const CENTRIFUGATION: &str = "https://sbol.io/ns/capability#Centrifugation";
pub const MEDIA_PREPARATION: &str = "https://sbol.io/ns/capability#MediaPreparation";
pub const PH_MEASUREMENT: &str = "https://sbol.io/ns/capability#PhMeasurement";
pub const WATER_PURIFICATION: &str = "https://sbol.io/ns/capability#WaterPurification";
pub const BIOSAFETY_CONTAINMENT: &str = "https://sbol.io/ns/capability#BiosafetyContainment";
pub const STEAM_STERILIZATION: &str = "https://sbol.io/ns/capability#SteamSterilization";
pub const COLD_STORAGE: &str = "https://sbol.io/ns/capability#ColdStorage";
pub const PLANT_GROWTH: &str = "https://sbol.io/ns/capability#PlantGrowth";

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
            Self::Discovered => "https://sbol.io/ns/facility#Discovered",
            Self::Described => "https://sbol.io/ns/facility#Described",
            Self::Plannable => "https://sbol.io/ns/facility#Plannable",
            Self::Simulatable => "https://sbol.io/ns/facility#Simulatable",
            Self::Executable => "https://sbol.io/ns/facility#Executable",
            Self::Qualified => "https://sbol.io/ns/facility#Qualified",
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
            Self::Unspecified => "https://sbol.io/ns/facility#UnspecifiedControl",
            Self::Manual => "https://sbol.io/ns/facility#ManualControl",
            Self::ReviewedFile => "https://sbol.io/ns/facility#ReviewedFileControl",
            Self::VendorSession => "https://sbol.io/ns/facility#VendorSessionControl",
            Self::Api => "https://sbol.io/ns/facility#ApiControl",
            Self::Sila2 => "https://sbol.io/ns/facility#SiLA2Control",
            Self::OpcUa => "https://sbol.io/ns/facility#OpcUaControl",
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
