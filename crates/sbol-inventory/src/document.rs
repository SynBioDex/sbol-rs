use std::path::Path;

use sbol3::{Document, Object, RdfFormat, ReadError, Resource, WriteError};

use crate::validation::{InventoryValidationReport, ValidatedInventory};
use crate::view::{
    AssetRef, CapabilityOfferingRef, FacilityRef, MaterialLotRef, PropertyValueRef, ZoneRef,
};
use crate::vocabulary::{
    ASSET, CAPABILITY_OFFERING, FACILITY, MATERIAL_KIND, PROPERTY_VALUE, ZONE,
};

/// One SBOL 3 document viewed through the SBOLInventory profile.
///
/// Parsing is intentionally separate from validation. An invalid profile
/// document remains inspectable so validators can report its raw RDF defects.
#[derive(Clone, Debug)]
pub struct InventoryDocument {
    document: Document,
}

impl InventoryDocument {
    pub fn read(input: &str, format: RdfFormat) -> Result<Self, ReadError> {
        Document::read(input, format).map(Self::from_sbol_document)
    }

    pub fn read_path(path: impl AsRef<Path>) -> Result<Self, ReadError> {
        Document::read_path(path).map(Self::from_sbol_document)
    }

    pub fn from_sbol_document(document: Document) -> Self {
        Self { document }
    }

    pub fn as_sbol_document(&self) -> &Document {
        &self.document
    }

    pub fn into_sbol_document(self) -> Document {
        self.document
    }

    pub fn write(&self, format: RdfFormat) -> Result<String, WriteError> {
        self.document.write(format)
    }

    /// Validates SBOL core and every required Profile 0.2 Validator rule.
    pub fn validate(&self) -> InventoryValidationReport {
        crate::validation::validate(self)
    }

    /// Returns a query-safe view when the document fully conforms to Profile 0.2.
    pub fn check(&self) -> Result<ValidatedInventory<'_>, InventoryValidationReport> {
        let report = self.validate();
        if report.is_valid() {
            Ok(ValidatedInventory::new(self, report))
        } else {
            Err(report)
        }
    }

    pub fn facilities(&self) -> impl Iterator<Item = FacilityRef<'_>> {
        self.objects_with_type(FACILITY)
            .map(|object| FacilityRef::new(self, object))
    }

    pub fn zones(&self) -> impl Iterator<Item = ZoneRef<'_>> {
        self.objects_with_type(ZONE)
            .map(|object| ZoneRef::new(self, object))
    }

    pub fn assets(&self) -> impl Iterator<Item = AssetRef<'_>> {
        self.objects_with_type(ASSET)
            .map(|object| AssetRef::new(self, object))
    }

    pub fn capability_offerings(&self) -> impl Iterator<Item = CapabilityOfferingRef<'_>> {
        self.objects_with_type(CAPABILITY_OFFERING)
            .map(|object| CapabilityOfferingRef::new(self, object))
    }

    pub fn property_values(&self) -> impl Iterator<Item = PropertyValueRef<'_>> {
        self.objects_with_type(PROPERTY_VALUE)
            .map(|object| PropertyValueRef::new(self, object))
    }

    /// Standard SBOL Implementations carrying `fac:materialKind`.
    ///
    /// Scope follows the profile marker rather than the concrete RDF type so
    /// malformed material lots remain visible to profile validation.
    pub fn material_lots(&self) -> impl Iterator<Item = MaterialLotRef<'_>> {
        self.document
            .objects()
            .values()
            .filter(|object| !object.values(MATERIAL_KIND).is_empty())
            .map(|object| MaterialLotRef::new(self, object))
    }

    pub fn facility(&self, identity: &Resource) -> Option<FacilityRef<'_>> {
        self.object_with_type(identity, FACILITY)
            .map(|object| FacilityRef::new(self, object))
    }

    pub fn zone(&self, identity: &Resource) -> Option<ZoneRef<'_>> {
        self.object_with_type(identity, ZONE)
            .map(|object| ZoneRef::new(self, object))
    }

    pub fn asset(&self, identity: &Resource) -> Option<AssetRef<'_>> {
        self.object_with_type(identity, ASSET)
            .map(|object| AssetRef::new(self, object))
    }

    pub fn capability_offering(&self, identity: &Resource) -> Option<CapabilityOfferingRef<'_>> {
        self.object_with_type(identity, CAPABILITY_OFFERING)
            .map(|object| CapabilityOfferingRef::new(self, object))
    }

    pub fn property_value(&self, identity: &Resource) -> Option<PropertyValueRef<'_>> {
        self.object_with_type(identity, PROPERTY_VALUE)
            .map(|object| PropertyValueRef::new(self, object))
    }

    pub fn material_lot(&self, identity: &Resource) -> Option<MaterialLotRef<'_>> {
        self.document
            .get(identity)
            .filter(|object| !object.values(MATERIAL_KIND).is_empty())
            .map(|object| MaterialLotRef::new(self, object))
    }

    pub fn experimental_data_databases(&self) -> impl Iterator<Item = &Object> {
        self.objects_with_type(crate::vocabulary::EXPERIMENTAL_DATA_DATABASE)
    }

    pub fn metadata_databases(&self) -> impl Iterator<Item = &Object> {
        self.objects_with_type(crate::vocabulary::METADATA_DATABASE)
    }

    pub fn evidence_component_ids<'a>(
        &'a self,
        evidence: &Resource,
    ) -> impl Iterator<Item = &'a Resource> {
        self.document
            .get(evidence)
            .into_iter()
            .flat_map(|object| object.resources(crate::vocabulary::FOR_COMPONENT))
    }

    pub fn submission_database_ids<'a>(
        &'a self,
        record: &Resource,
    ) -> impl Iterator<Item = &'a Resource> {
        self.document
            .get(record)
            .into_iter()
            .flat_map(|object| object.resources(crate::vocabulary::SUBMITTED_TO))
    }

    pub fn retrieval_database_ids<'a>(
        &'a self,
        component: &Resource,
    ) -> impl Iterator<Item = &'a Resource> {
        self.document
            .get(component)
            .into_iter()
            .flat_map(|object| object.resources(crate::vocabulary::RETRIEVED_FROM))
    }

    /// Resolves facility membership from a Zone through location, falling back to
    /// physical composition for assets without a direct location. Unlocated or
    /// cyclic paths have no known facility; validation reports malformed graphs.
    pub fn facility_id_for(&self, identity: &Resource) -> Option<&Resource> {
        use crate::vocabulary::{FACILITY_PROPERTY, LOCATED_IN, PART_OF};
        let mut object = self.document.get(identity)?;
        let mut visited = std::collections::BTreeSet::new();
        loop {
            if !visited.insert(object.identity()) {
                return None;
            }
            if has_type(object, ZONE) {
                return object.first_resource(FACILITY_PROPERTY);
            }
            if !has_type(object, ASSET) && object.values(MATERIAL_KIND).is_empty() {
                return None;
            }
            let parent = object.first_resource(LOCATED_IN).or_else(|| {
                has_type(object, ASSET)
                    .then(|| object.first_resource(PART_OF))
                    .flatten()
            })?;
            object = self.document.get(parent)?;
        }
    }

    pub(crate) fn object_with_type(&self, identity: &Resource, rdf_type: &str) -> Option<&Object> {
        self.document
            .get(identity)
            .filter(|object| has_type(object, rdf_type))
    }

    fn objects_with_type(&self, rdf_type: &str) -> impl Iterator<Item = &Object> {
        self.document
            .objects()
            .values()
            .filter(move |object| has_type(object, rdf_type))
    }
}

impl From<Document> for InventoryDocument {
    fn from(value: Document) -> Self {
        Self::from_sbol_document(value)
    }
}

fn has_type(object: &Object, rdf_type: &str) -> bool {
    object
        .rdf_types()
        .iter()
        .any(|candidate| candidate.as_str() == rdf_type)
}
