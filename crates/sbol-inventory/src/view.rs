use sbol3::{Implementation, Iri, Literal, Object, Resource, SbolObject};
use thiserror::Error;

use crate::InventoryDocument;
use crate::vocabulary::*;

macro_rules! common_accessors {
    () => {
        pub fn identity(&self) -> &'a Resource {
            self.object.identity()
        }

        pub fn display_id(&self) -> Option<&'a str> {
            self.object.identified().display_id.as_deref()
        }

        pub fn name(&self) -> Option<&'a str> {
            self.object.identified().name.as_deref()
        }

        pub fn description(&self) -> Option<&'a str> {
            self.object.identified().description.as_deref()
        }

        pub fn as_object(&self) -> &'a Object {
            self.object
        }
    };
}

#[derive(Clone, Copy, Debug)]
pub struct FacilityRef<'a> {
    object: &'a Object,
}

impl<'a> FacilityRef<'a> {
    pub(crate) fn new(_document: &'a InventoryDocument, object: &'a Object) -> Self {
        Self { object }
    }

    common_accessors!();
}

#[derive(Clone, Copy, Debug)]
pub struct ZoneRef<'a> {
    document: &'a InventoryDocument,
    object: &'a Object,
}

impl<'a> ZoneRef<'a> {
    pub(crate) fn new(document: &'a InventoryDocument, object: &'a Object) -> Self {
        Self { document, object }
    }

    common_accessors!();

    pub fn facility_id(&self) -> Option<&'a Resource> {
        self.object.first_resource(FACILITY_PROPERTY)
    }

    pub fn facility(&self) -> Option<FacilityRef<'a>> {
        self.facility_id()
            .and_then(|identity| self.document.facility(identity))
    }

    pub fn kind(&self) -> Option<&'a Iri> {
        self.object.first_iri(ZONE_KIND)
    }

    pub fn parent_zone_id(&self) -> Option<&'a Resource> {
        self.object.first_resource(PARENT_ZONE)
    }

    pub fn parent_zone(&self) -> Option<ZoneRef<'a>> {
        self.parent_zone_id()
            .and_then(|identity| self.document.zone(identity))
    }

    pub fn policies(&self) -> impl Iterator<Item = &'a Iri> {
        self.object.iris(POLICY)
    }

    pub fn condition_ids(&self) -> impl Iterator<Item = &'a Resource> {
        self.object.resources(CONDITION)
    }

    pub fn conditions(&self) -> impl Iterator<Item = PropertyValueRef<'a>> {
        self.condition_ids()
            .filter_map(|identity| self.document.property_value(identity))
    }

    pub fn is_active(&self) -> Option<bool> {
        boolean_value(self.object, IS_ACTIVE)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AssetRef<'a> {
    document: &'a InventoryDocument,
    object: &'a Object,
}

impl<'a> AssetRef<'a> {
    pub(crate) fn new(document: &'a InventoryDocument, object: &'a Object) -> Self {
        Self { document, object }
    }

    common_accessors!();

    pub fn facility_id(&self) -> Option<&'a Resource> {
        self.document.facility_id_for(self.object.identity())
    }

    pub fn facility(&self) -> Option<FacilityRef<'a>> {
        self.facility_id()
            .and_then(|identity| self.document.facility(identity))
    }

    pub fn kind(&self) -> Option<&'a Iri> {
        self.object.first_iri(ASSET_KIND)
    }

    pub fn located_in_id(&self) -> Option<&'a Resource> {
        self.object.first_resource(LOCATED_IN)
    }

    pub fn position(&self) -> Option<&'a str> {
        self.object.first_literal_value(POSITION)
    }

    pub fn part_of_id(&self) -> Option<&'a Resource> {
        self.object.first_resource(PART_OF)
    }

    pub fn part_of(&self) -> Option<AssetRef<'a>> {
        self.part_of_id()
            .and_then(|identity| self.document.asset(identity))
    }

    pub fn established_zone_ids(&self) -> impl Iterator<Item = &'a Resource> {
        self.object.resources(ESTABLISHES_ZONE)
    }

    pub fn established_zones(&self) -> impl Iterator<Item = ZoneRef<'a>> {
        self.established_zone_ids()
            .filter_map(|identity| self.document.zone(identity))
    }

    pub fn manufacturer(&self) -> Option<&'a str> {
        self.object.first_literal_value(MANUFACTURER)
    }

    pub fn model(&self) -> Option<&'a str> {
        self.object.first_literal_value(MODEL)
    }

    pub fn serial_number(&self) -> Option<&'a str> {
        self.object.first_literal_value(SERIAL_NUMBER)
    }

    pub fn is_active(&self) -> Option<bool> {
        boolean_value(self.object, IS_ACTIVE)
    }

    pub fn allowed_positions(&self) -> impl Iterator<Item = &'a str> {
        self.object.literals(ALLOWED_POSITION).map(Literal::value)
    }

    pub fn capability_ids(&self) -> impl Iterator<Item = &'a Resource> {
        self.object.resources(CAPABILITY)
    }

    pub fn capabilities(&self) -> impl Iterator<Item = CapabilityOfferingRef<'a>> {
        self.capability_ids()
            .filter_map(|identity| self.document.capability_offering(identity))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CapabilityOfferingRef<'a> {
    document: &'a InventoryDocument,
    object: &'a Object,
}

impl<'a> CapabilityOfferingRef<'a> {
    pub(crate) fn new(document: &'a InventoryDocument, object: &'a Object) -> Self {
        Self { document, object }
    }

    common_accessors!();

    pub fn kind(&self) -> Option<&'a Iri> {
        self.object.first_iri(CAPABILITY_KIND)
    }

    pub fn qualification_iri(&self) -> Option<&'a Iri> {
        self.object.first_iri(QUALIFICATION)
    }

    pub fn qualification(&self) -> Option<Qualification> {
        self.qualification_iri()
            .and_then(|value| Qualification::try_from(value.as_str()).ok())
    }

    pub fn control_mode_iri(&self) -> Option<&'a Iri> {
        self.object.first_iri(CONTROL_MODE)
    }

    pub fn control_mode(&self) -> Option<ControlMode> {
        self.control_mode_iri()
            .and_then(|value| ControlMode::try_from(value.as_str()).ok())
    }

    pub fn is_active(&self) -> Option<bool> {
        boolean_value(self.object, IS_ACTIVE)
    }

    pub fn parameter_ids(&self) -> impl Iterator<Item = &'a Resource> {
        self.object.resources(PARAMETER)
    }

    pub fn parameters(&self) -> impl Iterator<Item = PropertyValueRef<'a>> {
        self.parameter_ids()
            .filter_map(|identity| self.document.property_value(identity))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PropertyValueRef<'a> {
    object: &'a Object,
}

impl<'a> PropertyValueRef<'a> {
    pub(crate) fn new(_document: &'a InventoryDocument, object: &'a Object) -> Self {
        Self { object }
    }

    common_accessors!();

    pub fn kind(&self) -> Option<&'a Iri> {
        self.object.first_iri(PROPERTY_KIND)
    }

    pub fn unit(&self) -> Option<&'a Iri> {
        self.object.first_iri(UNIT)
    }

    /// Reads the single typed value without applying full profile validation.
    pub fn value(&self) -> Result<ScalarValueRef<'a>, PropertyValueReadError> {
        let mut values = Vec::new();
        collect_literal(self.object, TEXT_VALUE, ScalarValueRef::Text, &mut values);
        collect_literal(
            self.object,
            INTEGER_VALUE,
            ScalarValueRef::Integer,
            &mut values,
        );
        collect_literal(self.object, REAL_VALUE, ScalarValueRef::Real, &mut values);
        for literal in self.object.literals(BOOLEAN_VALUE) {
            let value = parse_boolean(literal.value()).ok_or_else(|| {
                PropertyValueReadError::InvalidBoolean(literal.value().to_owned())
            })?;
            values.push(ScalarValueRef::Boolean(value));
        }
        values.extend(self.object.iris(URI_VALUE).map(ScalarValueRef::Iri));
        match values.len() {
            0 => Err(PropertyValueReadError::Missing),
            1 => Ok(values.remove(0)),
            count => Err(PropertyValueReadError::Multiple(count)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarValueRef<'a> {
    Text(&'a str),
    Integer(&'a str),
    Real(&'a str),
    Boolean(bool),
    Iri(&'a Iri),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PropertyValueReadError {
    #[error("property has no typed value")]
    Missing,
    #[error("property has {0} typed values")]
    Multiple(usize),
    #[error("property has invalid xsd:boolean lexical form `{0}`")]
    InvalidBoolean(String),
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialLotRef<'a> {
    document: &'a InventoryDocument,
    object: &'a Object,
}

impl<'a> MaterialLotRef<'a> {
    pub(crate) fn new(document: &'a InventoryDocument, object: &'a Object) -> Self {
        Self { document, object }
    }

    common_accessors!();

    pub fn as_implementation(&self) -> Option<&'a Implementation> {
        match self.document.as_sbol_document().resolve(self.identity()) {
            Some(SbolObject::Implementation(implementation)) => Some(implementation),
            _ => None,
        }
    }

    pub fn kind(&self) -> Option<&'a Iri> {
        self.object.first_iri(MATERIAL_KIND)
    }

    pub fn facility_id(&self) -> Option<&'a Resource> {
        self.document.facility_id_for(self.object.identity())
    }

    pub fn facility(&self) -> Option<FacilityRef<'a>> {
        self.facility_id()
            .and_then(|identity| self.document.facility(identity))
    }

    pub fn built_id(&self) -> Option<&'a Resource> {
        self.object.first_resource(SBOL_BUILT)
    }

    pub fn located_in_id(&self) -> Option<&'a Resource> {
        self.object.first_resource(LOCATED_IN)
    }

    pub fn position(&self) -> Option<&'a str> {
        self.object.first_literal_value(POSITION)
    }

    pub fn is_active(&self) -> Option<bool> {
        boolean_value(self.object, IS_ACTIVE)
    }

    pub fn barcode(&self) -> Option<&'a str> {
        self.object.first_literal_value(BARCODE)
    }

    pub fn lot_id(&self) -> Option<&'a str> {
        self.object.first_literal_value(LOT_ID)
    }

    pub fn notes(&self) -> Option<&'a str> {
        self.object.first_literal_value(NOTES)
    }

    pub fn freeze_date(&self) -> Option<&'a str> {
        self.object.first_literal_value(FREEZE_DATE)
    }

    pub fn derived_from_ids(&self) -> impl Iterator<Item = &'a Resource> {
        self.object.resources(DERIVED_FROM_MATERIAL)
    }
}

fn boolean_value(object: &Object, predicate: &str) -> Option<bool> {
    let mut values = object.literals(predicate);
    let value = parse_boolean(values.next()?.value())?;
    values.next().is_none().then_some(value)
}

fn parse_boolean(value: &str) -> Option<bool> {
    match value {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn collect_literal<'a>(
    object: &'a Object,
    predicate: &str,
    constructor: fn(&'a str) -> ScalarValueRef<'a>,
    target: &mut Vec<ScalarValueRef<'a>>,
) {
    target.extend(
        object
            .literals(predicate)
            .map(Literal::value)
            .map(constructor),
    );
}
