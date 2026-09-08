//! Recorded repository transfers and Component evidence. No network operations.

use sbol3::{Component, ExperimentalData, Iri, Resource, Term, Triple};
use thiserror::Error;

use crate::vocabulary::*;
use crate::{ExperimentalDataDatabaseId, InventoryBuilder, MetadataDatabaseId};

impl InventoryBuilder {
    /// Relates ExperimentalData to its design and shares existing file Attachments.
    pub fn link_evidence(
        &mut self,
        evidence: &ExperimentalData,
        component: &Component,
    ) -> Result<(), DigitalLinkError> {
        self.require_digital_type(&evidence.identity, SBOL_EXPERIMENTAL_DATA)?;
        self.require_digital_type(&component.identity, SBOL_COMPONENT)?;
        let attachments = self.resource_values(&evidence.identity, SBOL_HAS_ATTACHMENT);
        self.add_digital_link(&evidence.identity, FOR_COMPONENT, &component.identity);
        for attachment in attachments {
            self.add_digital_link(&component.identity, SBOL_HAS_ATTACHMENT, &attachment);
        }
        Ok(())
    }

    pub fn record_data_submission(
        &mut self,
        data: &ExperimentalData,
        database: &ExperimentalDataDatabaseId,
    ) -> Result<(), DigitalLinkError> {
        self.require_digital_type(&data.identity, SBOL_EXPERIMENTAL_DATA)?;
        self.require_digital_type(&database.as_resource(), EXPERIMENTAL_DATA_DATABASE)?;
        self.add_digital_link(&data.identity, SUBMITTED_TO, &database.as_resource());
        Ok(())
    }

    pub fn record_component_submission(
        &mut self,
        component: &Component,
        database: &MetadataDatabaseId,
    ) -> Result<(), DigitalLinkError> {
        self.require_digital_type(&component.identity, SBOL_COMPONENT)?;
        self.require_digital_type(&database.as_resource(), METADATA_DATABASE)?;
        self.add_digital_link(&component.identity, SUBMITTED_TO, &database.as_resource());
        Ok(())
    }

    pub fn record_component_retrieval(
        &mut self,
        component: &Component,
        database: &MetadataDatabaseId,
    ) -> Result<(), DigitalLinkError> {
        self.require_digital_type(&component.identity, SBOL_COMPONENT)?;
        self.require_digital_type(&database.as_resource(), METADATA_DATABASE)?;
        self.add_digital_link(&component.identity, RETRIEVED_FROM, &database.as_resource());
        Ok(())
    }

    fn require_digital_type(
        &self,
        identity: &Resource,
        expected: &'static str,
    ) -> Result<(), DigitalLinkError> {
        if !self.has_rdf_type(identity, expected) {
            return Err(DigitalLinkError::InvalidReference {
                identity: identity.clone(),
                expected,
            });
        }
        Ok(())
    }

    fn add_digital_link(&mut self, subject: &Resource, predicate: &'static str, target: &Resource) {
        self.extend_existing_triples([Triple {
            subject: subject.clone(),
            predicate: Iri::from_static(predicate),
            object: Term::Resource(target.clone()),
        }]);
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DigitalLinkError {
    #[error("digital record reference `{identity}` must be a local `{expected}`")]
    InvalidReference {
        identity: Resource,
        expected: &'static str,
    },
}
