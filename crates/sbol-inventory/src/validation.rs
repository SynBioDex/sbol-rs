//! Full SBOLInventory Profile 0.2 validation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Deref;

use sbol3::{Literal, Object, Resource, Severity, Term, ValidationIssue, ValidationReport};

use crate::rules::{ConformanceClass, profile_rules};
use crate::vocabulary::*;
use crate::{InventoryDocument, ProfileRule};

/// Exact upstream revision from which the embedded Profile 0.2 artifacts were copied.
pub const PROFILE_SOURCE_REVISION: &str = "371c919c763c970091a12a6449099b5591deb3ec";

/// The SBOL core validator used by this profile implementation.
pub const CORE_VALIDATOR: &str = "sbol3::Document::validate";

/// Combined SBOL core and SBOLInventory profile validation evidence.
#[derive(Clone, Debug)]
pub struct InventoryValidationReport {
    core: Box<ValidationReport>,
    profile_issues: Vec<ValidationIssue>,
}

impl InventoryValidationReport {
    pub fn core_report(&self) -> &ValidationReport {
        &self.core
    }

    pub fn profile_issues(&self) -> &[ValidationIssue] {
        &self.profile_issues
    }

    pub fn issues(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.core.issues().iter().chain(&self.profile_issues)
    }

    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues()
            .filter(|issue| issue.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues()
            .filter(|issue| issue.severity == Severity::Warning)
    }

    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }

    pub fn is_valid(&self) -> bool {
        !self.has_errors()
    }

    pub fn has_rule_violation(&self, rule: &str) -> bool {
        self.errors().any(|issue| issue.rule == rule)
    }

    pub fn applied_profile_rules(&self) -> impl Iterator<Item = &'static ProfileRule> {
        profile_rules()
            .iter()
            .filter(|rule| rule.applies_to(ConformanceClass::Validator))
    }

    pub const fn profile_iri(&self) -> &'static str {
        PROFILE_IRI
    }

    pub const fn profile_version(&self) -> &'static str {
        PROFILE_VERSION
    }

    pub const fn profile_status(&self) -> &'static str {
        crate::PROFILE_RULE_CATALOG_STATUS
    }

    pub const fn profile_source_revision(&self) -> &'static str {
        PROFILE_SOURCE_REVISION
    }

    pub const fn sbol_core_version(&self) -> &'static str {
        sbol3::SPEC_VERSION
    }

    pub const fn core_validator(&self) -> &'static str {
        CORE_VALIDATOR
    }
}

impl fmt::Display for InventoryValidationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let errors = self.errors().count();
        let warnings = self.warnings().count();
        write!(
            formatter,
            "SBOLInventory {} validation produced {errors} errors and {warnings} warnings",
            PROFILE_VERSION
        )?;
        for issue in self.errors().take(5) {
            write!(formatter, "\n{}: {}", issue.rule, issue.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for InventoryValidationReport {}

/// A document that has passed SBOL core and all Profile 0.2 Validator rules.
#[derive(Debug)]
pub struct ValidatedInventory<'a> {
    document: &'a InventoryDocument,
    report: InventoryValidationReport,
}

impl<'a> ValidatedInventory<'a> {
    pub(crate) fn new(document: &'a InventoryDocument, report: InventoryValidationReport) -> Self {
        debug_assert!(report.is_valid());
        Self { document, report }
    }

    pub fn document(&self) -> &'a InventoryDocument {
        self.document
    }

    pub fn report(&self) -> &InventoryValidationReport {
        &self.report
    }
}

impl Deref for ValidatedInventory<'_> {
    type Target = InventoryDocument;

    fn deref(&self) -> &Self::Target {
        self.document
    }
}

pub(crate) fn validate(inventory: &InventoryDocument) -> InventoryValidationReport {
    let core = inventory.as_sbol_document().validate();
    let mut validator = Validator::new(inventory);
    if core.has_errors() {
        validator.error(
            "sbolinv-10001",
            Resource::iri(PROFILE_IRI),
            None,
            format!(
                "document does not conform to SBOL core {} ({} core errors)",
                sbol3::SPEC_VERSION,
                core.errors().count()
            ),
        );
    }
    validator.validate();
    InventoryValidationReport {
        core: Box::new(core),
        profile_issues: validator.issues,
    }
}

#[derive(Clone, Copy)]
enum ValueKind {
    Iri,
    LocalType(&'static str),
    String,
    Integer,
    Double,
    Boolean,
    DateTime,
}

#[derive(Clone, Copy)]
struct PropertySpec {
    predicate: &'static str,
    maximum: Option<usize>,
    kind: ValueKind,
}

impl PropertySpec {
    const fn one(predicate: &'static str, kind: ValueKind) -> Self {
        Self {
            predicate,
            maximum: Some(1),
            kind,
        }
    }

    const fn many(predicate: &'static str, kind: ValueKind) -> Self {
        Self {
            predicate,
            maximum: None,
            kind,
        }
    }
}

const ZONE_PROPERTIES: &[PropertySpec] = &[
    PropertySpec::one(FACILITY_PROPERTY, ValueKind::Iri),
    PropertySpec::one(ZONE_KIND, ValueKind::Iri),
    PropertySpec::one(PARENT_ZONE, ValueKind::Iri),
    PropertySpec::many(POLICY, ValueKind::Iri),
    PropertySpec::many(CONDITION, ValueKind::LocalType(PROPERTY_VALUE)),
    PropertySpec::one(IS_ACTIVE, ValueKind::Boolean),
];

const ASSET_PROPERTIES: &[PropertySpec] = &[
    PropertySpec::one(FACILITY_PROPERTY, ValueKind::Iri),
    PropertySpec::one(ASSET_KIND, ValueKind::Iri),
    PropertySpec::one(LOCATED_IN, ValueKind::Iri),
    PropertySpec::one(POSITION, ValueKind::String),
    PropertySpec::one(PART_OF, ValueKind::Iri),
    PropertySpec::many(ESTABLISHES_ZONE, ValueKind::Iri),
    PropertySpec::one(MANUFACTURER, ValueKind::String),
    PropertySpec::one(MODEL, ValueKind::String),
    PropertySpec::one(SERIAL_NUMBER, ValueKind::String),
    PropertySpec::one(IS_ACTIVE, ValueKind::Boolean),
    PropertySpec::many(ALLOWED_POSITION, ValueKind::String),
    PropertySpec::many(CAPABILITY, ValueKind::LocalType(CAPABILITY_OFFERING)),
];

const CAPABILITY_PROPERTIES: &[PropertySpec] = &[
    PropertySpec::one(CAPABILITY_KIND, ValueKind::Iri),
    PropertySpec::one(QUALIFICATION, ValueKind::Iri),
    PropertySpec::one(CONTROL_MODE, ValueKind::Iri),
    PropertySpec::one(IS_ACTIVE, ValueKind::Boolean),
    PropertySpec::many(PARAMETER, ValueKind::LocalType(PROPERTY_VALUE)),
];

const PROPERTY_VALUE_PROPERTIES: &[PropertySpec] = &[
    PropertySpec::one(PROPERTY_KIND, ValueKind::Iri),
    PropertySpec::one(TEXT_VALUE, ValueKind::String),
    PropertySpec::one(INTEGER_VALUE, ValueKind::Integer),
    PropertySpec::one(REAL_VALUE, ValueKind::Double),
    PropertySpec::one(BOOLEAN_VALUE, ValueKind::Boolean),
    PropertySpec::one(URI_VALUE, ValueKind::Iri),
    PropertySpec::one(UNIT, ValueKind::Iri),
];

const MATERIAL_PROPERTIES: &[PropertySpec] = &[
    PropertySpec::one(SBOL_BUILT, ValueKind::Iri),
    PropertySpec::one(MATERIAL_KIND, ValueKind::Iri),
    PropertySpec::one(FACILITY_PROPERTY, ValueKind::Iri),
    PropertySpec::one(LOCATED_IN, ValueKind::Iri),
    PropertySpec::one(POSITION, ValueKind::String),
    PropertySpec::one(IS_ACTIVE, ValueKind::Boolean),
    PropertySpec::one(BARCODE, ValueKind::String),
    PropertySpec::one(LOT_ID, ValueKind::String),
    PropertySpec::one(NOTES, ValueKind::String),
    PropertySpec::one(FREEZE_DATE, ValueKind::DateTime),
    PropertySpec::many(DERIVED_FROM_MATERIAL, ValueKind::Iri),
];

struct Validator<'a> {
    inventory: &'a InventoryDocument,
    issues: Vec<ValidationIssue>,
    issue_keys: BTreeSet<(String, Resource, Option<&'static str>, String)>,
}

impl<'a> Validator<'a> {
    fn new(inventory: &'a InventoryDocument) -> Self {
        Self {
            inventory,
            issues: Vec::new(),
            issue_keys: BTreeSet::new(),
        }
    }

    fn document(&self) -> &'a sbol3::Document {
        self.inventory.as_sbol_document()
    }

    fn validate(&mut self) {
        self.validate_base_types();
        self.validate_property_shapes();
        self.validate_zones();
        self.validate_assets();
        self.validate_locations_and_occupancy();
        self.validate_capabilities();
        self.validate_property_values();
        self.validate_material_lots();
        self.validate_runs();
    }

    fn error(
        &mut self,
        rule: &'static str,
        subject: Resource,
        property: Option<&'static str>,
        message: impl Into<String>,
    ) {
        let message = message.into();
        let key = (rule.to_owned(), subject.clone(), property, message.clone());
        if self.issue_keys.insert(key) {
            self.issues
                .push(ValidationIssue::error(rule, subject, property, message));
        }
    }

    fn validate_base_types(&mut self) {
        let checks = [
            (FACILITY, SBOL_TOP_LEVEL),
            (ZONE, SBOL_TOP_LEVEL),
            (ASSET, SBOL_TOP_LEVEL),
            (CAPABILITY_OFFERING, SBOL_IDENTIFIED),
            (PROPERTY_VALUE, SBOL_IDENTIFIED),
        ];
        for (profile_type, required_type) in checks {
            let objects = self.objects_with_type(profile_type);
            for object in objects {
                if !has_type(object, required_type) {
                    self.error(
                        "sbolinv-10002",
                        object.identity().clone(),
                        Some(RDF_TYPE),
                        format!("{profile_type} must also be typed {required_type}"),
                    );
                }
            }
        }

        let material_lots = self.material_objects();
        for object in material_lots {
            if !has_type(object, SBOL_IMPLEMENTATION) {
                self.error(
                    "sbolinv-10002",
                    object.identity().clone(),
                    Some(RDF_TYPE),
                    "a material lot must be an SBOL Implementation",
                );
                self.error(
                    "sbolinv-16001",
                    object.identity().clone(),
                    Some(RDF_TYPE),
                    "a node carrying fac:materialKind must be typed sbol:Implementation",
                );
            }
        }
    }

    fn validate_property_shapes(&mut self) {
        let groups = [
            (self.objects_with_type(ZONE), ZONE_PROPERTIES),
            (self.objects_with_type(ASSET), ASSET_PROPERTIES),
            (
                self.objects_with_type(CAPABILITY_OFFERING),
                CAPABILITY_PROPERTIES,
            ),
            (
                self.objects_with_type(PROPERTY_VALUE),
                PROPERTY_VALUE_PROPERTIES,
            ),
            (self.material_objects(), MATERIAL_PROPERTIES),
        ];
        for (objects, specs) in groups {
            for object in objects {
                for spec in specs {
                    let values = object.values(spec.predicate);
                    if spec.maximum.is_some_and(|maximum| values.len() > maximum) {
                        self.error(
                            "sbolinv-10004",
                            object.identity().clone(),
                            Some(spec.predicate),
                            format!(
                                "{} has {} values but permits at most {}",
                                spec.predicate,
                                values.len(),
                                spec.maximum.expect("checked above")
                            ),
                        );
                    }
                    if values
                        .iter()
                        .any(|term| !self.property_term_matches(term, spec.kind))
                    {
                        self.error(
                            "sbolinv-10004",
                            object.identity().clone(),
                            Some(spec.predicate),
                            format!(
                                "{} contains a value with the wrong RDF kind or datatype",
                                spec.predicate
                            ),
                        );
                    }
                }
            }
        }
    }

    fn validate_zones(&mut self) {
        let zones = self.objects_with_type(ZONE);
        for zone in &zones {
            self.require_local_typed_reference(
                zone,
                FACILITY_PROPERTY,
                FACILITY,
                "sbolinv-12001",
                "Zone",
                "Facility",
            );
            self.require_one_iri(zone, ZONE_KIND, "sbolinv-12002", "zoneKind");
            self.require_one_boolean(zone, IS_ACTIVE, "sbolinv-12003", "Zone isActive");

            if !zone.values(PARENT_ZONE).is_empty()
                && let Some(parent) = self.require_local_typed_reference(
                    zone,
                    PARENT_ZONE,
                    ZONE,
                    "sbolinv-12004",
                    "parentZone",
                    "Zone",
                )
                && !same_facility(zone, parent)
            {
                self.error(
                    "sbolinv-12004",
                    zone.identity().clone(),
                    Some(PARENT_ZONE),
                    "parent and child zones must belong to the same Facility",
                );
            }

            if zone
                .values(POLICY)
                .iter()
                .any(|term| term.as_iri().is_none())
            {
                self.error(
                    "sbolinv-12006",
                    zone.identity().clone(),
                    Some(POLICY),
                    "every policy value must be an absolute IRI",
                );
            }
            self.validate_unique_owned_kinds(zone, CONDITION, "sbolinv-12007", "condition");
        }

        let edges = zones
            .iter()
            .filter_map(|zone| {
                one_resource(zone, PARENT_ZONE)
                    .map(|parent| (zone.identity().clone(), parent.clone()))
            })
            .collect();
        self.report_cycles(
            "sbolinv-12005",
            Some(PARENT_ZONE),
            "parentZone graph contains a cycle",
            edges,
        );
    }

    fn validate_assets(&mut self) {
        let assets = self.objects_with_type(ASSET);
        for asset in &assets {
            self.require_local_typed_reference(
                asset,
                FACILITY_PROPERTY,
                FACILITY,
                "sbolinv-13001",
                "Asset",
                "Facility",
            );
            self.require_one_iri(asset, ASSET_KIND, "sbolinv-13002", "assetKind");
            self.require_one_boolean(asset, IS_ACTIVE, "sbolinv-13003", "Asset isActive");

            if !asset.values(PART_OF).is_empty()
                && let Some(parent) = self.require_local_typed_reference(
                    asset,
                    PART_OF,
                    ASSET,
                    "sbolinv-13004",
                    "partOf",
                    "Asset",
                )
                && !same_facility(asset, parent)
            {
                self.error(
                    "sbolinv-13004",
                    asset.identity().clone(),
                    Some(PART_OF),
                    "partOf must remain within one Facility",
                );
            }

            for established in asset
                .resources(ESTABLISHES_ZONE)
                .cloned()
                .collect::<Vec<_>>()
            {
                match self.document().get(&established) {
                    Some(zone) if has_type(zone, ZONE) && same_facility(asset, zone) => {}
                    _ => self.error(
                        "sbolinv-13005",
                        asset.identity().clone(),
                        Some(ESTABLISHES_ZONE),
                        format!(
                            "established zone {established} must resolve locally in the same Facility"
                        ),
                    ),
                }
            }

            let mut positions = BTreeSet::new();
            for literal in asset.literals(ALLOWED_POSITION) {
                if literal.value().trim().is_empty() || !positions.insert(literal.value()) {
                    self.error(
                        "sbolinv-13006",
                        asset.identity().clone(),
                        Some(ALLOWED_POSITION),
                        "allowedPosition values must be non-blank and unique",
                    );
                }
            }
            self.validate_unique_owned_kinds(asset, CAPABILITY, "sbolinv-13008", "capability");
        }

        let asset_ids: BTreeSet<_> = assets.iter().map(|asset| asset.identity()).collect();
        let mut edges = Vec::new();
        for asset in assets {
            for predicate in [PART_OF, LOCATED_IN] {
                if let Some(parent) = one_resource(asset, predicate)
                    && asset_ids.contains(parent)
                {
                    edges.push((asset.identity().clone(), parent.clone()));
                }
            }
        }
        self.report_cycles(
            "sbolinv-13007",
            None,
            "asset containment graph contains a cycle",
            edges,
        );
    }

    fn validate_locations_and_occupancy(&mut self) {
        let mut occupants = self.objects_with_type(ASSET);
        occupants.extend(self.material_objects());
        let mut occupied: BTreeMap<(Resource, String), Vec<Resource>> = BTreeMap::new();

        for object in occupants {
            let location_values = object.values(LOCATED_IN);
            let position_values = object.values(POSITION);
            let location = one_resource(object, LOCATED_IN);
            let position = one_string(object, POSITION);

            if !location_values.is_empty() {
                match location.and_then(|identity| self.document().get(identity)) {
                    Some(target)
                        if (has_type(target, ZONE) || has_type(target, ASSET))
                            && same_facility(object, target) => {}
                    _ => self.error(
                        "sbolinv-13501",
                        object.identity().clone(),
                        Some(LOCATED_IN),
                        "locatedIn must resolve to a local Zone or Asset in the same Facility",
                    ),
                }
            }

            let mut position_error = false;
            if position_values.len() == 1 {
                position_error = position.is_none_or(|value| value.trim().is_empty());
            }
            match location.and_then(|identity| self.document().get(identity)) {
                None => {
                    position_error |= !position_values.is_empty();
                }
                Some(target) if has_type(target, ZONE) => {
                    position_error |= !position_values.is_empty();
                }
                Some(target) if has_type(target, ASSET) => {
                    let allowed: BTreeSet<_> = target
                        .literals(ALLOWED_POSITION)
                        .map(Literal::value)
                        .collect();
                    if !allowed.is_empty() {
                        position_error |= position.is_none_or(|value| !allowed.contains(value));
                    }
                    if let Some(value) = position
                        && !value.trim().is_empty()
                    {
                        occupied
                            .entry((target.identity().clone(), value.to_owned()))
                            .or_default()
                            .push(object.identity().clone());
                    }
                }
                Some(_) => {}
            }
            if position_error {
                self.error(
                    "sbolinv-13502",
                    object.identity().clone(),
                    Some(POSITION),
                    "position is inconsistent with the object's location or container positions",
                );
            }
        }

        for ((container, position), identities) in occupied {
            if identities.len() > 1 {
                for identity in identities {
                    self.error(
                        "sbolinv-13503",
                        identity,
                        Some(POSITION),
                        format!(
                            "container {container} position {position:?} has multiple occupants"
                        ),
                    );
                }
            }
        }
    }

    fn validate_capabilities(&mut self) {
        let mut owners: BTreeMap<Resource, Vec<Resource>> = BTreeMap::new();
        for asset in self.objects_with_type(ASSET) {
            for offering in asset.resources(CAPABILITY) {
                owners
                    .entry(offering.clone())
                    .or_default()
                    .push(asset.identity().clone());
            }
        }

        for offering in self.objects_with_type(CAPABILITY_OFFERING) {
            if owners.get(offering.identity()).map_or(0, Vec::len) != 1 {
                self.error(
                    "sbolinv-14001",
                    offering.identity().clone(),
                    Some(CAPABILITY),
                    "CapabilityOffering must be owned by exactly one Asset",
                );
            }
            self.require_one_iri(offering, CAPABILITY_KIND, "sbolinv-14002", "capabilityKind");
            self.require_closed_iri(
                offering,
                QUALIFICATION,
                "sbolinv-14003",
                "qualification",
                Qualification::ALL.map(Qualification::iri).as_slice(),
            );
            self.require_closed_iri(
                offering,
                CONTROL_MODE,
                "sbolinv-14004",
                "controlMode",
                ControlMode::ALL.map(ControlMode::iri).as_slice(),
            );
            self.require_one_boolean(
                offering,
                IS_ACTIVE,
                "sbolinv-14005",
                "CapabilityOffering isActive",
            );
            self.validate_unique_owned_kinds(offering, PARAMETER, "sbolinv-14006", "parameter");
        }
    }

    fn validate_property_values(&mut self) {
        let mut owners: BTreeMap<Resource, Vec<Resource>> = BTreeMap::new();
        for zone in self.objects_with_type(ZONE) {
            for condition in zone.resources(CONDITION) {
                owners
                    .entry(condition.clone())
                    .or_default()
                    .push(zone.identity().clone());
            }
        }
        for offering in self.objects_with_type(CAPABILITY_OFFERING) {
            for parameter in offering.resources(PARAMETER) {
                owners
                    .entry(parameter.clone())
                    .or_default()
                    .push(offering.identity().clone());
            }
        }

        for property in self.objects_with_type(PROPERTY_VALUE) {
            if owners.get(property.identity()).map_or(0, Vec::len) != 1 {
                self.error(
                    "sbolinv-15001",
                    property.identity().clone(),
                    None,
                    "PropertyValue must have exactly one profile owner",
                );
            }
            self.require_one_iri(property, PROPERTY_KIND, "sbolinv-15002", "propertyKind");

            let value_specs = [
                (TEXT_VALUE, ValueKind::String),
                (INTEGER_VALUE, ValueKind::Integer),
                (REAL_VALUE, ValueKind::Double),
                (BOOLEAN_VALUE, ValueKind::Boolean),
                (URI_VALUE, ValueKind::Iri),
            ];
            let present: Vec<_> = value_specs
                .iter()
                .filter(|(predicate, _)| !property.values(predicate).is_empty())
                .collect();
            if present.len() != 1
                || property.values(present[0].0).len() != 1
                || !term_matches(&property.values(present[0].0)[0], present[0].1)
            {
                self.error(
                    "sbolinv-15003",
                    property.identity().clone(),
                    None,
                    "PropertyValue must contain exactly one correctly typed scalar value",
                );
            }

            if property
                .values(URI_VALUE)
                .iter()
                .chain(property.values(UNIT))
                .any(|term| term.as_iri().is_none())
            {
                self.error(
                    "sbolinv-15004",
                    property.identity().clone(),
                    None,
                    "uriValue and unit values must be absolute IRIs",
                );
            }
            if !property.values(UNIT).is_empty()
                && property.values(INTEGER_VALUE).is_empty()
                && property.values(REAL_VALUE).is_empty()
            {
                self.error(
                    "sbolinv-15005",
                    property.identity().clone(),
                    Some(UNIT),
                    "unit is allowed only on a numeric PropertyValue",
                );
            }
        }
    }

    fn validate_material_lots(&mut self) {
        let materials = self.material_objects();
        for material in &materials {
            self.require_one_iri(material, MATERIAL_KIND, "sbolinv-16002", "materialKind");
            self.require_local_typed_reference(
                material,
                FACILITY_PROPERTY,
                FACILITY,
                "sbolinv-16003",
                "MaterialLot",
                "Facility",
            );
            self.require_local_typed_reference(
                material,
                SBOL_BUILT,
                SBOL_COMPONENT,
                "sbolinv-16004",
                "MaterialLot built",
                "Component",
            );
            self.require_one_boolean(material, IS_ACTIVE, "sbolinv-16005", "MaterialLot isActive");

            for ancestor in material
                .resources(DERIVED_FROM_MATERIAL)
                .cloned()
                .collect::<Vec<_>>()
            {
                let valid = ancestor != *material.identity()
                    && self.document().get(&ancestor).is_some_and(is_material_lot);
                if !valid {
                    self.error(
                        "sbolinv-16006",
                        material.identity().clone(),
                        Some(DERIVED_FROM_MATERIAL),
                        format!("lineage target {ancestor} must be another local MaterialLot"),
                    );
                }
            }
        }

        let material_ids: BTreeSet<_> = materials
            .iter()
            .map(|material| material.identity())
            .collect();
        let edges = materials
            .iter()
            .flat_map(|material| {
                material
                    .resources(DERIVED_FROM_MATERIAL)
                    .filter(|parent| material_ids.contains(parent))
                    .map(|parent| (material.identity().clone(), parent.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        self.report_cycles(
            "sbolinv-16006",
            Some(DERIVED_FROM_MATERIAL),
            "material lineage contains a cycle",
            edges,
        );
    }

    fn validate_runs(&mut self) {
        let usages = self.objects_with_type(PROV_USAGE);
        let profile_usages: BTreeSet<_> = usages
            .iter()
            .filter(|usage| is_profile_usage(usage))
            .map(|usage| usage.identity().clone())
            .collect();

        for usage in usages {
            let roles: BTreeSet<_> = usage.iris(PROV_HAD_ROLE).map(|iri| iri.as_str()).collect();
            if roles.contains(RUN_ASSET) {
                self.require_local_typed_reference(
                    usage,
                    PROV_ENTITY,
                    ASSET,
                    "sbolinv-17001",
                    "RunAsset Usage entity",
                    "Asset",
                );
            }
            if roles.contains(RUN_INPUT_MATERIAL) {
                let valid = usage.values(PROV_ENTITY).len() == 1
                    && one_resource(usage, PROV_ENTITY)
                        .and_then(|identity| self.document().get(identity))
                        .is_some_and(is_material_lot);
                if !valid {
                    self.error(
                        "sbolinv-17002",
                        usage.identity().clone(),
                        Some(PROV_ENTITY),
                        "RunInputMaterial Usage must name exactly one local MaterialLot",
                    );
                }
            }
        }

        for activity in self.objects_with_type(PROV_ACTIVITY) {
            let owned: Vec<_> = activity
                .resources(PROV_QUALIFIED_USAGE)
                .filter(|usage| profile_usages.contains(*usage))
                .filter_map(|usage| self.document().get(usage))
                .collect();
            if !owned.is_empty()
                && !owned.iter().any(|usage| {
                    usage
                        .iris(PROV_HAD_ROLE)
                        .any(|role| role.as_str() == RUN_ASSET)
                })
            {
                self.error(
                    "sbolinv-17003",
                    activity.identity().clone(),
                    Some(PROV_QUALIFIED_USAGE),
                    "profile run Activity must own at least one RunAsset Usage",
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn require_local_typed_reference<'b>(
        &mut self,
        object: &'b Object,
        predicate: &'static str,
        required_type: &'static str,
        rule: &'static str,
        source_label: &str,
        target_label: &str,
    ) -> Option<&'a Object> {
        let values = object.values(predicate);
        let target_identity = (values.len() == 1)
            .then(|| values[0].as_resource())
            .flatten()
            .cloned();
        let valid = target_identity
            .as_ref()
            .and_then(|identity| self.document().get(identity))
            .is_some_and(|target| has_type(target, required_type));
        if !valid {
            self.error(
                rule,
                object.identity().clone(),
                Some(predicate),
                format!(
                    "{source_label} must name exactly one document-local {target_label} through {predicate}"
                ),
            );
        }
        target_identity
            .filter(|_| valid)
            .and_then(|identity| self.document().get(&identity))
    }

    fn require_one_iri(
        &mut self,
        object: &Object,
        predicate: &'static str,
        rule: &'static str,
        label: &str,
    ) {
        let values = object.values(predicate);
        if values.len() != 1 || values[0].as_iri().is_none() {
            self.error(
                rule,
                object.identity().clone(),
                Some(predicate),
                format!("{label} must have exactly one absolute IRI value"),
            );
        }
    }

    fn require_one_boolean(
        &mut self,
        object: &Object,
        predicate: &'static str,
        rule: &'static str,
        label: &str,
    ) {
        let values = object.values(predicate);
        if values.len() != 1 || !term_matches(&values[0], ValueKind::Boolean) {
            self.error(
                rule,
                object.identity().clone(),
                Some(predicate),
                format!("{label} must have exactly one xsd:boolean value"),
            );
        }
    }

    fn require_closed_iri(
        &mut self,
        object: &Object,
        predicate: &'static str,
        rule: &'static str,
        label: &str,
        allowed: &[&str],
    ) {
        let values = object.values(predicate);
        let valid = values.len() == 1
            && values[0]
                .as_iri()
                .is_some_and(|iri| allowed.contains(&iri.as_str()));
        if !valid {
            self.error(
                rule,
                object.identity().clone(),
                Some(predicate),
                format!("{label} must contain exactly one value from the closed vocabulary"),
            );
        }
    }

    fn validate_unique_owned_kinds(
        &mut self,
        owner: &Object,
        ownership_predicate: &'static str,
        rule: &'static str,
        label: &str,
    ) {
        let owned_kinds: Vec<_> = owner
            .resources(ownership_predicate)
            .filter_map(|identity| self.document().get(identity))
            .filter_map(|child| one_iri(child, property_kind_for(ownership_predicate)))
            .map(|kind| kind.as_str().to_owned())
            .collect();
        let mut kinds = BTreeSet::new();
        if owned_kinds.into_iter().any(|kind| !kinds.insert(kind)) {
            self.error(
                rule,
                owner.identity().clone(),
                Some(ownership_predicate),
                format!("{label} property kinds must be unique per owner"),
            );
        }
    }

    fn property_term_matches(&self, term: &Term, kind: ValueKind) -> bool {
        match kind {
            ValueKind::LocalType(required_type) => term
                .as_resource()
                .and_then(|identity| self.document().get(identity))
                .is_some_and(|target| has_type(target, required_type)),
            _ => term_matches(term, kind),
        }
    }

    fn report_cycles(
        &mut self,
        rule: &'static str,
        property: Option<&'static str>,
        message: &'static str,
        edges: Vec<(Resource, Resource)>,
    ) {
        let adjacency = edges.into_iter().fold(
            BTreeMap::<Resource, Vec<Resource>>::new(),
            |mut map, (source, target)| {
                map.entry(source).or_default().push(target);
                map
            },
        );
        let mut state = BTreeMap::new();
        let mut cyclic = BTreeSet::new();
        for node in adjacency.keys() {
            find_cycles(node, &adjacency, &mut state, &mut Vec::new(), &mut cyclic);
        }
        for identity in cyclic {
            self.error(rule, identity, property, message);
        }
    }

    fn objects_with_type(&self, rdf_type: &str) -> Vec<&'a Object> {
        self.document()
            .objects()
            .values()
            .filter(|object| has_type(object, rdf_type))
            .collect()
    }

    fn material_objects(&self) -> Vec<&'a Object> {
        self.document()
            .objects()
            .values()
            .filter(|object| !object.values(MATERIAL_KIND).is_empty())
            .collect()
    }
}

fn has_type(object: &Object, rdf_type: &str) -> bool {
    object
        .rdf_types()
        .iter()
        .any(|candidate| candidate.as_str() == rdf_type)
}

fn is_material_lot(object: &Object) -> bool {
    has_type(object, SBOL_IMPLEMENTATION) && !object.values(MATERIAL_KIND).is_empty()
}

fn is_profile_usage(object: &Object) -> bool {
    object
        .iris(PROV_HAD_ROLE)
        .any(|role| matches!(role.as_str(), RUN_ASSET | RUN_INPUT_MATERIAL))
}

fn one_resource<'a>(object: &'a Object, predicate: &str) -> Option<&'a Resource> {
    let values = object.values(predicate);
    (values.len() == 1)
        .then(|| values[0].as_resource())
        .flatten()
}

fn one_iri<'a>(object: &'a Object, predicate: &str) -> Option<&'a sbol3::Iri> {
    let values = object.values(predicate);
    (values.len() == 1).then(|| values[0].as_iri()).flatten()
}

fn one_string<'a>(object: &'a Object, predicate: &str) -> Option<&'a str> {
    let values = object.values(predicate);
    if values.len() != 1 || !term_matches(&values[0], ValueKind::String) {
        return None;
    }
    values[0].as_literal().map(Literal::value)
}

fn same_facility(left: &Object, right: &Object) -> bool {
    one_resource(left, FACILITY_PROPERTY)
        .zip(one_resource(right, FACILITY_PROPERTY))
        .is_some_and(|(left, right)| left == right)
}

fn property_kind_for(ownership_predicate: &str) -> &'static str {
    match ownership_predicate {
        CAPABILITY => CAPABILITY_KIND,
        CONDITION | PARAMETER => PROPERTY_KIND,
        _ => PROPERTY_KIND,
    }
}

fn term_matches(term: &Term, kind: ValueKind) -> bool {
    match kind {
        ValueKind::Iri => term.as_iri().is_some(),
        ValueKind::LocalType(_) => false,
        ValueKind::String => literal_matches(term, XSD_STRING, |_| true),
        ValueKind::Integer => literal_matches(term, XSD_INTEGER, integer_lexical),
        ValueKind::Double => literal_matches(term, XSD_DOUBLE, double_lexical),
        ValueKind::Boolean => literal_matches(term, XSD_BOOLEAN, boolean_lexical),
        ValueKind::DateTime => literal_matches(term, XSD_DATE_TIME, |_| true),
    }
}

fn literal_matches(term: &Term, datatype: &str, lexical: impl FnOnce(&str) -> bool) -> bool {
    term.as_literal().is_some_and(|literal| {
        literal.language().is_none()
            && literal.datatype().as_str() == datatype
            && lexical(literal.value())
    })
}

fn integer_lexical(value: &str) -> bool {
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn double_lexical(value: &str) -> bool {
    matches!(value, "INF" | "-INF" | "NaN") || value.parse::<f64>().is_ok()
}

fn boolean_lexical(value: &str) -> bool {
    matches!(value, "true" | "false" | "1" | "0")
}

fn find_cycles(
    node: &Resource,
    adjacency: &BTreeMap<Resource, Vec<Resource>>,
    state: &mut BTreeMap<Resource, u8>,
    stack: &mut Vec<Resource>,
    cyclic: &mut BTreeSet<Resource>,
) {
    match state.get(node) {
        Some(2) => return,
        Some(1) => {
            if let Some(index) = stack.iter().position(|candidate| candidate == node) {
                cyclic.extend(stack[index..].iter().cloned());
            }
            return;
        }
        _ => {}
    }
    state.insert(node.clone(), 1);
    stack.push(node.clone());
    for target in adjacency.get(node).into_iter().flatten() {
        find_cycles(target, adjacency, state, stack, cyclic);
    }
    stack.pop();
    state.insert(node.clone(), 2);
}
