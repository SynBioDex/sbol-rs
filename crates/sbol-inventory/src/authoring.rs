//! Typed construction of composable SBOLInventory facility graphs.

use std::collections::BTreeSet;
use std::fmt;

use sbol3::{
    Component, DisplayId, Iri, Literal, Namespace, RdfGraph, Resource, SbolObject, Term, ToRdf,
    Triple,
};
use thiserror::Error;

use crate::InventoryDocument;
use crate::vocabulary::*;

macro_rules! typed_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Iri);

        impl $name {
            pub fn as_iri(&self) -> &Iri {
                &self.0
            }

            pub fn as_resource(&self) -> Resource {
                Resource::Iri(self.0.clone())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

typed_identity!(FacilityId);
typed_identity!(ZoneId);
typed_identity!(AssetId);
typed_identity!(CapabilityOfferingId);
typed_identity!(PropertyValueId);
typed_identity!(MaterialLotId);

/// A typed physical location target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocationId {
    Zone(ZoneId),
    Asset(AssetId),
}

impl LocationId {
    fn as_resource(&self) -> Resource {
        match self {
            Self::Zone(identity) => identity.as_resource(),
            Self::Asset(identity) => identity.as_resource(),
        }
    }
}

impl From<ZoneId> for LocationId {
    fn from(value: ZoneId) -> Self {
        Self::Zone(value)
    }
}

impl From<AssetId> for LocationId {
    fn from(value: AssetId) -> Self {
        Self::Asset(value)
    }
}

/// Exactly one typed scalar carried by a profile PropertyValue.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyScalar {
    Text(String),
    Integer(i64),
    Real(f64),
    Boolean(bool),
    Iri(Iri),
}

impl PropertyScalar {
    fn predicate(&self) -> &'static str {
        match self {
            Self::Text(_) => TEXT_VALUE,
            Self::Integer(_) => INTEGER_VALUE,
            Self::Real(_) => REAL_VALUE,
            Self::Boolean(_) => BOOLEAN_VALUE,
            Self::Iri(_) => URI_VALUE,
        }
    }

    fn term(&self) -> Term {
        match self {
            Self::Text(value) => literal_term(value, XSD_STRING),
            Self::Integer(value) => literal_term(value.to_string(), XSD_INTEGER),
            Self::Real(value) => literal_term(double_lexical(*value), XSD_DOUBLE),
            Self::Boolean(value) => literal_term(value.to_string(), XSD_BOOLEAN),
            Self::Iri(value) => iri_term(value.clone()),
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, Self::Integer(_) | Self::Real(_))
    }
}

#[derive(Clone, Debug)]
struct Metadata {
    display_id: DisplayId,
    name: Option<String>,
    description: Option<String>,
    annotations: Vec<(Iri, Term)>,
}

impl Metadata {
    fn new(display_id: impl Into<String>) -> Result<Self, InventoryBuildError> {
        let display_id = display_id.into();
        let display_id = DisplayId::new(display_id.clone())
            .map_err(|_| InventoryBuildError::InvalidDisplayId(display_id))?;
        Ok(Self {
            display_id,
            name: None,
            description: None,
            annotations: Vec::new(),
        })
    }
}

macro_rules! metadata_methods {
    () => {
        pub fn name(mut self, value: impl Into<String>) -> Self {
            self.metadata.name = Some(value.into());
            self
        }

        pub fn description(mut self, value: impl Into<String>) -> Self {
            self.metadata.description = Some(value.into());
            self
        }

        /// Adds an open extension statement to this identified object.
        pub fn annotation(mut self, predicate: Iri, value: Term) -> Self {
            self.metadata.annotations.push((predicate, value));
            self
        }
    };
}

/// Builder for a custom SBOL TopLevel facility.
#[derive(Clone, Debug)]
pub struct FacilityBuilder {
    metadata: Metadata,
}

impl FacilityBuilder {
    pub fn new(display_id: impl Into<String>) -> Result<Self, InventoryBuildError> {
        Ok(Self {
            metadata: Metadata::new(display_id)?,
        })
    }

    metadata_methods!();
}

/// Builder for a governed spatial, environmental, containment, or policy zone.
#[derive(Clone, Debug)]
pub struct ZoneBuilder {
    metadata: Metadata,
    facility: FacilityId,
    kind: Iri,
    parent: Option<ZoneId>,
    policies: Vec<Iri>,
    active: Option<bool>,
    conditions: Vec<PropertyValueBuilder>,
}

impl ZoneBuilder {
    pub fn new(
        display_id: impl Into<String>,
        facility: FacilityId,
        kind: Iri,
    ) -> Result<Self, InventoryBuildError> {
        Ok(Self {
            metadata: Metadata::new(display_id)?,
            facility,
            kind,
            parent: None,
            policies: Vec::new(),
            active: None,
            conditions: Vec::new(),
        })
    }

    metadata_methods!();

    pub fn parent(mut self, parent: ZoneId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn policy(mut self, policy: Iri) -> Self {
        self.policies.push(policy);
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    pub fn condition(mut self, condition: PropertyValueBuilder) -> Self {
        self.conditions.push(condition);
        self
    }
}

/// Builder for a physical resource, instrument, container, or functional unit.
#[derive(Clone, Debug)]
pub struct AssetBuilder {
    metadata: Metadata,
    facility: FacilityId,
    kind: Iri,
    location: Option<LocationId>,
    position: Option<String>,
    part_of: Option<AssetId>,
    establishes_zones: Vec<ZoneId>,
    manufacturer: Option<String>,
    model: Option<String>,
    serial_number: Option<String>,
    active: Option<bool>,
    allowed_positions: Vec<String>,
    capabilities: Vec<CapabilityBuilder>,
}

impl AssetBuilder {
    pub fn new(
        display_id: impl Into<String>,
        facility: FacilityId,
        kind: Iri,
    ) -> Result<Self, InventoryBuildError> {
        Ok(Self {
            metadata: Metadata::new(display_id)?,
            facility,
            kind,
            location: None,
            position: None,
            part_of: None,
            establishes_zones: Vec::new(),
            manufacturer: None,
            model: None,
            serial_number: None,
            active: None,
            allowed_positions: Vec::new(),
            capabilities: Vec::new(),
        })
    }

    metadata_methods!();

    pub fn located_in(mut self, location: impl Into<LocationId>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn position(mut self, position: impl Into<String>) -> Self {
        self.position = Some(position.into());
        self
    }

    pub fn part_of(mut self, parent: AssetId) -> Self {
        self.part_of = Some(parent);
        self
    }

    pub fn establishes_zone(mut self, zone: ZoneId) -> Self {
        self.establishes_zones.push(zone);
        self
    }

    pub fn manufacturer(mut self, manufacturer: impl Into<String>) -> Self {
        self.manufacturer = Some(manufacturer.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn serial_number(mut self, serial_number: impl Into<String>) -> Self {
        self.serial_number = Some(serial_number.into());
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    pub fn allowed_position(mut self, position: impl Into<String>) -> Self {
        self.allowed_positions.push(position.into());
        self
    }

    pub fn capability(mut self, capability: CapabilityBuilder) -> Self {
        self.capabilities.push(capability);
        self
    }
}

/// Builder for an operation offered by one installed asset.
#[derive(Clone, Debug)]
pub struct CapabilityBuilder {
    metadata: Metadata,
    kind: Iri,
    qualification: Option<Qualification>,
    control_mode: Option<ControlMode>,
    active: Option<bool>,
    parameters: Vec<PropertyValueBuilder>,
}

impl CapabilityBuilder {
    pub fn new(display_id: impl Into<String>, kind: Iri) -> Result<Self, InventoryBuildError> {
        Ok(Self {
            metadata: Metadata::new(display_id)?,
            kind,
            qualification: None,
            control_mode: None,
            active: None,
            parameters: Vec::new(),
        })
    }

    metadata_methods!();

    pub fn qualification(mut self, qualification: Qualification) -> Self {
        self.qualification = Some(qualification);
        self
    }

    pub fn control_mode(mut self, control_mode: ControlMode) -> Self {
        self.control_mode = Some(control_mode);
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    pub fn parameter(mut self, parameter: PropertyValueBuilder) -> Self {
        self.parameters.push(parameter);
        self
    }
}

/// Builder for one typed capability parameter or environmental condition.
#[derive(Clone, Debug)]
pub struct PropertyValueBuilder {
    metadata: Metadata,
    kind: Iri,
    value: PropertyScalar,
    unit: Option<Iri>,
}

impl PropertyValueBuilder {
    pub fn new(
        display_id: impl Into<String>,
        kind: Iri,
        value: PropertyScalar,
    ) -> Result<Self, InventoryBuildError> {
        Ok(Self {
            metadata: Metadata::new(display_id)?,
            kind,
            value,
            unit: None,
        })
    }

    pub fn text(
        display_id: impl Into<String>,
        kind: Iri,
        value: impl Into<String>,
    ) -> Result<Self, InventoryBuildError> {
        Self::new(display_id, kind, PropertyScalar::Text(value.into()))
    }

    pub fn integer(
        display_id: impl Into<String>,
        kind: Iri,
        value: i64,
    ) -> Result<Self, InventoryBuildError> {
        Self::new(display_id, kind, PropertyScalar::Integer(value))
    }

    pub fn real(
        display_id: impl Into<String>,
        kind: Iri,
        value: f64,
    ) -> Result<Self, InventoryBuildError> {
        Self::new(display_id, kind, PropertyScalar::Real(value))
    }

    pub fn boolean(
        display_id: impl Into<String>,
        kind: Iri,
        value: bool,
    ) -> Result<Self, InventoryBuildError> {
        Self::new(display_id, kind, PropertyScalar::Boolean(value))
    }

    pub fn iri(
        display_id: impl Into<String>,
        kind: Iri,
        value: Iri,
    ) -> Result<Self, InventoryBuildError> {
        Self::new(display_id, kind, PropertyScalar::Iri(value))
    }

    metadata_methods!();

    pub fn unit(mut self, unit: Iri) -> Self {
        self.unit = Some(unit);
        self
    }
}

/// Builder for an SBOL Implementation carrying material-lot profile state.
#[derive(Clone, Debug)]
pub struct MaterialLotBuilder {
    metadata: Metadata,
    facility: FacilityId,
    kind: Iri,
    built: Resource,
    location: Option<LocationId>,
    position: Option<String>,
    active: Option<bool>,
    barcode: Option<String>,
    lot_id: Option<String>,
    notes: Option<String>,
    freeze_date: Option<String>,
    derived_from: Vec<MaterialLotId>,
}

impl MaterialLotBuilder {
    pub fn new(
        display_id: impl Into<String>,
        facility: FacilityId,
        kind: Iri,
        built: &Component,
    ) -> Result<Self, InventoryBuildError> {
        Self::from_built_identity(display_id, facility, kind, built.identity.clone())
    }

    /// Raw interoperability escape hatch for a Component identity not held as a Rust value.
    pub fn from_built_identity(
        display_id: impl Into<String>,
        facility: FacilityId,
        kind: Iri,
        built: Resource,
    ) -> Result<Self, InventoryBuildError> {
        Ok(Self {
            metadata: Metadata::new(display_id)?,
            facility,
            kind,
            built,
            location: None,
            position: None,
            active: None,
            barcode: None,
            lot_id: None,
            notes: None,
            freeze_date: None,
            derived_from: Vec::new(),
        })
    }

    metadata_methods!();

    pub fn located_in(mut self, location: impl Into<LocationId>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn position(mut self, position: impl Into<String>) -> Self {
        self.position = Some(position.into());
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    pub fn barcode(mut self, barcode: impl Into<String>) -> Self {
        self.barcode = Some(barcode.into());
        self
    }

    pub fn lot_id(mut self, lot_id: impl Into<String>) -> Self {
        self.lot_id = Some(lot_id.into());
        self
    }

    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    pub fn freeze_date(mut self, freeze_date: impl Into<String>) -> Self {
        self.freeze_date = Some(freeze_date.into());
        self
    }

    pub fn derived_from(mut self, source: MaterialLotId) -> Self {
        self.derived_from.push(source);
        self
    }
}

/// Transactional graph assembler for profile and ordinary SBOL objects.
#[derive(Clone, Debug)]
pub struct InventoryBuilder {
    namespace: Namespace,
    triples: BTreeSet<Triple>,
    identities: BTreeSet<Resource>,
}

impl InventoryBuilder {
    pub fn new(namespace: Namespace) -> Self {
        Self {
            namespace,
            triples: BTreeSet::new(),
            identities: BTreeSet::new(),
        }
    }

    pub fn namespace(&self) -> &Namespace {
        &self.namespace
    }

    /// Imports one ordinary typed SBOL object and all owned children it emits.
    pub fn add_sbol_object(&mut self, object: SbolObject) -> Result<(), InventoryBuildError> {
        let triples = object.to_rdf_triples()?;
        self.commit_triples(triples)
    }

    /// Imports a complete SBOL document as the biological-design side of the graph.
    pub fn add_sbol_document(
        &mut self,
        document: &sbol3::Document,
    ) -> Result<(), InventoryBuildError> {
        self.commit_triples(document.rdf_graph().triples().to_vec())
    }

    pub fn add_facility(
        &mut self,
        builder: FacilityBuilder,
    ) -> Result<FacilityId, InventoryBuildError> {
        let identity = FacilityId(self.top_level_identity(&builder.metadata.display_id));
        let subject = identity.as_resource();
        let mut triples = top_level_triples(&subject, FACILITY, &self.namespace, &builder.metadata);
        add_annotations(&mut triples, &subject, &builder.metadata.annotations);
        self.commit_triples(triples)?;
        Ok(identity)
    }

    pub fn add_zone(&mut self, builder: ZoneBuilder) -> Result<ZoneId, InventoryBuildError> {
        let identity = ZoneId(self.top_level_identity(&builder.metadata.display_id));
        let subject = identity.as_resource();
        let active = builder
            .active
            .ok_or_else(|| missing("Zone", &subject, IS_ACTIVE))?;
        let mut triples = top_level_triples(&subject, ZONE, &self.namespace, &builder.metadata);
        triples.extend([
            resource_triple(&subject, FACILITY_PROPERTY, builder.facility.as_resource()),
            iri_triple(&subject, ZONE_KIND, builder.kind),
            boolean_triple(&subject, IS_ACTIVE, active),
        ]);
        if let Some(parent) = builder.parent {
            triples.push(resource_triple(&subject, PARENT_ZONE, parent.as_resource()));
        }
        for policy in builder.policies {
            triples.push(iri_triple(&subject, POLICY, policy));
        }
        add_annotations(&mut triples, &subject, &builder.metadata.annotations);
        let mut condition_ids = BTreeSet::new();
        for condition in builder.conditions {
            let (condition_id, condition_triples) = encode_property(&identity.0, condition)?;
            require_unique_child(&mut condition_ids, condition_id.as_resource())?;
            triples.push(resource_triple(
                &subject,
                CONDITION,
                condition_id.as_resource(),
            ));
            triples.extend(condition_triples);
        }
        self.commit_triples(triples)?;
        Ok(identity)
    }

    pub fn add_asset(&mut self, builder: AssetBuilder) -> Result<AssetId, InventoryBuildError> {
        let identity = AssetId(self.top_level_identity(&builder.metadata.display_id));
        let subject = identity.as_resource();
        let active = builder
            .active
            .ok_or_else(|| missing("Asset", &subject, IS_ACTIVE))?;
        let mut triples = top_level_triples(&subject, ASSET, &self.namespace, &builder.metadata);
        triples.extend([
            resource_triple(&subject, FACILITY_PROPERTY, builder.facility.as_resource()),
            iri_triple(&subject, ASSET_KIND, builder.kind),
            boolean_triple(&subject, IS_ACTIVE, active),
        ]);
        if let Some(location) = builder.location {
            triples.push(resource_triple(
                &subject,
                LOCATED_IN,
                location.as_resource(),
            ));
        }
        if let Some(position) = builder.position {
            triples.push(string_triple(&subject, POSITION, position));
        }
        if let Some(parent) = builder.part_of {
            triples.push(resource_triple(&subject, PART_OF, parent.as_resource()));
        }
        for zone in builder.establishes_zones {
            triples.push(resource_triple(
                &subject,
                ESTABLISHES_ZONE,
                zone.as_resource(),
            ));
        }
        add_optional_string(&mut triples, &subject, MANUFACTURER, builder.manufacturer);
        add_optional_string(&mut triples, &subject, MODEL, builder.model);
        add_optional_string(&mut triples, &subject, SERIAL_NUMBER, builder.serial_number);
        for position in builder.allowed_positions {
            triples.push(string_triple(&subject, ALLOWED_POSITION, position));
        }
        add_annotations(&mut triples, &subject, &builder.metadata.annotations);
        let mut capability_ids = BTreeSet::new();
        for capability in builder.capabilities {
            let (capability_id, capability_triples) = encode_capability(&identity.0, capability)?;
            require_unique_child(&mut capability_ids, capability_id.as_resource())?;
            triples.push(resource_triple(
                &subject,
                CAPABILITY,
                capability_id.as_resource(),
            ));
            triples.extend(capability_triples);
        }
        self.commit_triples(triples)?;
        Ok(identity)
    }

    pub fn add_material_lot(
        &mut self,
        builder: MaterialLotBuilder,
    ) -> Result<MaterialLotId, InventoryBuildError> {
        let identity = MaterialLotId(self.top_level_identity(&builder.metadata.display_id));
        let subject = identity.as_resource();
        let active = builder
            .active
            .ok_or_else(|| missing("MaterialLot", &subject, IS_ACTIVE))?;
        let mut triples = standard_top_level_triples(
            &subject,
            SBOL_IMPLEMENTATION,
            &self.namespace,
            &builder.metadata,
        );
        triples.extend([
            resource_triple(&subject, SBOL_BUILT, builder.built),
            iri_triple(&subject, MATERIAL_KIND, builder.kind),
            resource_triple(&subject, FACILITY_PROPERTY, builder.facility.as_resource()),
            boolean_triple(&subject, IS_ACTIVE, active),
        ]);
        if let Some(location) = builder.location {
            triples.push(resource_triple(
                &subject,
                LOCATED_IN,
                location.as_resource(),
            ));
        }
        if let Some(position) = builder.position {
            triples.push(string_triple(&subject, POSITION, position));
        }
        add_optional_string(&mut triples, &subject, BARCODE, builder.barcode);
        add_optional_string(&mut triples, &subject, LOT_ID, builder.lot_id);
        add_optional_string(&mut triples, &subject, NOTES, builder.notes);
        if let Some(freeze_date) = builder.freeze_date {
            triples.push(literal_triple(
                &subject,
                FREEZE_DATE,
                freeze_date,
                XSD_DATE_TIME,
            ));
        }
        for source in builder.derived_from {
            triples.push(resource_triple(
                &subject,
                DERIVED_FROM_MATERIAL,
                source.as_resource(),
            ));
        }
        add_annotations(&mut triples, &subject, &builder.metadata.annotations);
        self.commit_triples(triples)?;
        Ok(identity)
    }

    /// Completes graph assembly without hiding validation failures.
    pub fn finish(self) -> InventoryDocument {
        let graph = RdfGraph::new(self.triples.into_iter().collect());
        InventoryDocument::from_sbol_document(sbol3::Document::from_rdf_graph(graph))
    }

    pub(crate) fn top_level_identity(&self, display_id: &DisplayId) -> Iri {
        Iri::new_unchecked(format!(
            "{}/{}",
            self.namespace.as_str(),
            display_id.as_str()
        ))
    }

    pub(crate) fn contains_identity(&self, identity: &Resource) -> bool {
        self.identities.contains(identity)
    }

    pub(crate) fn extend_existing_triples(&mut self, triples: impl IntoIterator<Item = Triple>) {
        let triples: Vec<_> = triples.into_iter().collect();
        debug_assert!(
            triples
                .iter()
                .all(|triple| self.identities.contains(&triple.subject))
        );
        self.triples.extend(triples);
    }

    pub(crate) fn commit_triples(
        &mut self,
        triples: Vec<Triple>,
    ) -> Result<(), InventoryBuildError> {
        let subjects: BTreeSet<_> = triples
            .iter()
            .map(|triple| triple.subject.clone())
            .collect();
        if let Some(identity) = subjects
            .iter()
            .find(|identity| self.identities.contains(*identity))
        {
            return Err(InventoryBuildError::DuplicateIdentity(identity.clone()));
        }
        self.identities.extend(subjects);
        self.triples.extend(triples);
        Ok(())
    }
}

/// Failure while assembling a typed inventory graph.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum InventoryBuildError {
    #[error(transparent)]
    Sbol(#[from] sbol3::BuildError),
    #[error("invalid SBOL displayId `{0}`")]
    InvalidDisplayId(String),
    #[error("duplicate object identity `{0}`")]
    DuplicateIdentity(Resource),
    #[error("{object_type} `{identity}` is missing required property `{property}`")]
    MissingRequired {
        object_type: &'static str,
        identity: Resource,
        property: &'static str,
    },
    #[error("PropertyValue `{identity}` may carry a unit only with a numeric value")]
    UnitOnNonNumericValue { identity: Resource },
}

fn encode_capability(
    owner: &Iri,
    builder: CapabilityBuilder,
) -> Result<(CapabilityOfferingId, Vec<Triple>), InventoryBuildError> {
    let identity = CapabilityOfferingId(child_identity(owner, &builder.metadata.display_id));
    let subject = identity.as_resource();
    let qualification = builder
        .qualification
        .ok_or_else(|| missing("CapabilityOffering", &subject, QUALIFICATION))?;
    let control_mode = builder
        .control_mode
        .ok_or_else(|| missing("CapabilityOffering", &subject, CONTROL_MODE))?;
    let active = builder
        .active
        .ok_or_else(|| missing("CapabilityOffering", &subject, IS_ACTIVE))?;
    let mut triples = identified_triples(&subject, CAPABILITY_OFFERING, &builder.metadata);
    triples.extend([
        iri_triple(&subject, CAPABILITY_KIND, builder.kind),
        iri_triple(
            &subject,
            QUALIFICATION,
            Iri::from_static(qualification.iri()),
        ),
        iri_triple(&subject, CONTROL_MODE, Iri::from_static(control_mode.iri())),
        boolean_triple(&subject, IS_ACTIVE, active),
    ]);
    add_annotations(&mut triples, &subject, &builder.metadata.annotations);
    let mut parameter_ids = BTreeSet::new();
    for parameter in builder.parameters {
        let (parameter_id, parameter_triples) = encode_property(&identity.0, parameter)?;
        require_unique_child(&mut parameter_ids, parameter_id.as_resource())?;
        triples.push(resource_triple(
            &subject,
            PARAMETER,
            parameter_id.as_resource(),
        ));
        triples.extend(parameter_triples);
    }
    Ok((identity, triples))
}

fn encode_property(
    owner: &Iri,
    builder: PropertyValueBuilder,
) -> Result<(PropertyValueId, Vec<Triple>), InventoryBuildError> {
    let identity = PropertyValueId(child_identity(owner, &builder.metadata.display_id));
    let subject = identity.as_resource();
    if builder.unit.is_some() && !builder.value.is_numeric() {
        return Err(InventoryBuildError::UnitOnNonNumericValue { identity: subject });
    }
    let mut triples = identified_triples(&subject, PROPERTY_VALUE, &builder.metadata);
    triples.extend([
        iri_triple(&subject, PROPERTY_KIND, builder.kind),
        Triple {
            subject: subject.clone(),
            predicate: Iri::from_static(builder.value.predicate()),
            object: builder.value.term(),
        },
    ]);
    if let Some(unit) = builder.unit {
        triples.push(iri_triple(&subject, UNIT, unit));
    }
    add_annotations(&mut triples, &subject, &builder.metadata.annotations);
    Ok((identity, triples))
}

fn top_level_triples(
    subject: &Resource,
    profile_type: &'static str,
    namespace: &Namespace,
    metadata: &Metadata,
) -> Vec<Triple> {
    let mut triples = standard_top_level_triples(subject, SBOL_TOP_LEVEL, namespace, metadata);
    triples.push(type_triple(subject, profile_type));
    triples
}

fn standard_top_level_triples(
    subject: &Resource,
    rdf_type: &'static str,
    namespace: &Namespace,
    metadata: &Metadata,
) -> Vec<Triple> {
    let mut triples = identified_metadata_triples(subject, metadata);
    triples.extend([
        type_triple(subject, rdf_type),
        iri_triple(subject, SBOL_HAS_NAMESPACE, namespace.as_iri().clone()),
    ]);
    triples
}

fn identified_triples(
    subject: &Resource,
    profile_type: &'static str,
    metadata: &Metadata,
) -> Vec<Triple> {
    let mut triples = identified_metadata_triples(subject, metadata);
    triples.extend([
        type_triple(subject, SBOL_IDENTIFIED),
        type_triple(subject, profile_type),
    ]);
    triples
}

fn identified_metadata_triples(subject: &Resource, metadata: &Metadata) -> Vec<Triple> {
    let mut triples = vec![string_triple(
        subject,
        SBOL_DISPLAY_ID,
        metadata.display_id.as_str(),
    )];
    add_optional_string(&mut triples, subject, SBOL_NAME, metadata.name.clone());
    add_optional_string(
        &mut triples,
        subject,
        SBOL_DESCRIPTION,
        metadata.description.clone(),
    );
    triples
}

fn child_identity(owner: &Iri, display_id: &DisplayId) -> Iri {
    Iri::new_unchecked(format!("{owner}/{}", display_id.as_str()))
}

fn type_triple(subject: &Resource, rdf_type: &'static str) -> Triple {
    iri_triple(subject, RDF_TYPE, Iri::from_static(rdf_type))
}

fn iri_triple(subject: &Resource, predicate: &'static str, value: Iri) -> Triple {
    Triple {
        subject: subject.clone(),
        predicate: Iri::from_static(predicate),
        object: iri_term(value),
    }
}

fn resource_triple(subject: &Resource, predicate: &'static str, value: Resource) -> Triple {
    Triple {
        subject: subject.clone(),
        predicate: Iri::from_static(predicate),
        object: Term::Resource(value),
    }
}

fn string_triple(subject: &Resource, predicate: &'static str, value: impl Into<String>) -> Triple {
    literal_triple(subject, predicate, value, XSD_STRING)
}

fn boolean_triple(subject: &Resource, predicate: &'static str, value: bool) -> Triple {
    literal_triple(subject, predicate, value.to_string(), XSD_BOOLEAN)
}

fn literal_triple(
    subject: &Resource,
    predicate: &'static str,
    value: impl Into<String>,
    datatype: &'static str,
) -> Triple {
    Triple {
        subject: subject.clone(),
        predicate: Iri::from_static(predicate),
        object: literal_term(value, datatype),
    }
}

fn iri_term(value: Iri) -> Term {
    Term::Resource(Resource::Iri(value))
}

fn literal_term(value: impl Into<String>, datatype: &'static str) -> Term {
    Term::Literal(Literal::new(value, Iri::from_static(datatype), None))
}

fn add_optional_string(
    triples: &mut Vec<Triple>,
    subject: &Resource,
    predicate: &'static str,
    value: Option<String>,
) {
    if let Some(value) = value {
        triples.push(string_triple(subject, predicate, value));
    }
}

fn add_annotations(triples: &mut Vec<Triple>, subject: &Resource, annotations: &[(Iri, Term)]) {
    triples.extend(annotations.iter().map(|(predicate, value)| Triple {
        subject: subject.clone(),
        predicate: predicate.clone(),
        object: value.clone(),
    }));
}

fn missing(
    object_type: &'static str,
    identity: &Resource,
    property: &'static str,
) -> InventoryBuildError {
    InventoryBuildError::MissingRequired {
        object_type,
        identity: identity.clone(),
        property,
    }
}

fn require_unique_child(
    identities: &mut BTreeSet<Resource>,
    identity: Resource,
) -> Result<(), InventoryBuildError> {
    if !identities.insert(identity.clone()) {
        return Err(InventoryBuildError::DuplicateIdentity(identity));
    }
    Ok(())
}

fn double_lexical(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_owned()
    } else if value == f64::INFINITY {
        "INF".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-INF".to_owned()
    } else {
        value.to_string()
    }
}
