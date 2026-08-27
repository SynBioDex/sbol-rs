//! SBOLInventory Profile 0.2 support for `sbol-rs`.
//!
//! The profile adds facility catalogs and run provenance to ordinary SBOL 3
//! RDF. This crate layers typed views over [`sbol3::Document`] without adding
//! profile-specific variants to the SBOL 3 core object model.

#![forbid(unsafe_code)]

mod authoring;
mod document;
mod query;
pub mod rules;
mod validation;
mod view;
pub mod vocabulary;

pub use authoring::{
    AssetBuilder, AssetId, CapabilityBuilder, CapabilityOfferingId, FacilityBuilder, FacilityId,
    InventoryBuildError, InventoryBuilder, LocationId, MaterialLotBuilder, MaterialLotId,
    PropertyScalar, PropertyValueBuilder, PropertyValueId, ZoneBuilder, ZoneId,
};
pub use document::InventoryDocument;
pub use query::{CandidateQuery, CapabilityCandidate, QueryError, find_qualified_assets};
pub use rules::{
    ConformanceClass, PROFILE_RULE_CATALOG_IRI, PROFILE_RULE_CATALOG_STATUS,
    PROFILE_RULE_CATALOG_VERSION, ProfileRule, RuleStrength, profile_rule, profile_rules,
};
pub use validation::{
    CORE_VALIDATOR, InventoryValidationReport, PROFILE_SOURCE_REVISION, ValidatedInventory,
};
pub use view::{
    AssetRef, CapabilityOfferingRef, FacilityRef, MaterialLotRef, PropertyValueReadError,
    PropertyValueRef, ScalarValueRef, ZoneRef,
};

/// Common imports for reading SBOLInventory documents.
pub mod prelude {
    pub use crate::vocabulary::{ControlMode, Qualification};
    pub use crate::{
        AssetBuilder, AssetId, AssetRef, CandidateQuery, CapabilityBuilder, CapabilityCandidate,
        CapabilityOfferingId, CapabilityOfferingRef, FacilityBuilder, FacilityId, FacilityRef,
        InventoryBuildError, InventoryBuilder, InventoryDocument, InventoryValidationReport,
        LocationId, MaterialLotBuilder, MaterialLotId, MaterialLotRef, PropertyScalar,
        PropertyValueBuilder, PropertyValueId, PropertyValueRef, QueryError, ScalarValueRef,
        ValidatedInventory, ZoneBuilder, ZoneId, ZoneRef, find_qualified_assets,
    };
    pub use sbol3::{Iri, Namespace, RdfFormat, Resource};
}
