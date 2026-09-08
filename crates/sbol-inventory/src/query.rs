//! Deterministic candidate discovery over validated facility catalogs.

use std::collections::BTreeSet;

use sbol3::{Iri, Resource};
use thiserror::Error;

use crate::vocabulary::Qualification;
use crate::{AssetRef, CapabilityOfferingRef, InventoryDocument, ValidatedInventory, ZoneRef};

/// A checked candidate-query request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateQuery {
    capability_kind: Iri,
    minimum_qualification: Qualification,
    facility: Option<Iri>,
}

impl CandidateQuery {
    /// Creates a request from already typed values.
    pub fn new(capability_kind: Iri, minimum_qualification: Qualification) -> Self {
        Self {
            capability_kind,
            minimum_qualification,
            facility: None,
        }
    }

    /// Parses the two externally supplied vocabulary values with stable query diagnostics.
    pub fn parse(
        capability_kind: impl Into<String>,
        minimum_qualification: &str,
    ) -> Result<Self, QueryError> {
        let capability_kind = capability_kind.into();
        let capability_kind =
            Iri::new(capability_kind.clone()).map_err(|_| QueryError::InvalidCapabilityKind {
                value: capability_kind,
            })?;
        let minimum_qualification =
            Qualification::try_from(minimum_qualification).map_err(|_| {
                QueryError::UnknownQualification {
                    value: minimum_qualification.to_owned(),
                }
            })?;
        Ok(Self::new(capability_kind, minimum_qualification))
    }

    /// Restricts results to assets governed by one facility identity.
    pub fn within_facility(mut self, facility: Iri) -> Self {
        self.facility = Some(facility);
        self
    }

    pub fn capability_kind(&self) -> &Iri {
        &self.capability_kind
    }

    pub fn minimum_qualification(&self) -> Qualification {
        self.minimum_qualification
    }

    pub fn facility(&self) -> Option<&Iri> {
        self.facility.as_ref()
    }
}

/// A qualifying installed asset and the offering that satisfied the request.
#[derive(Clone, Copy, Debug)]
pub struct CapabilityCandidate<'a> {
    asset: AssetRef<'a>,
    offering: CapabilityOfferingRef<'a>,
}

impl<'a> CapabilityCandidate<'a> {
    pub fn asset(&self) -> AssetRef<'a> {
        self.asset
    }

    pub fn offering(&self) -> CapabilityOfferingRef<'a> {
        self.offering
    }
}

/// Candidate-query input error with its normative Profile 0.2 rule.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum QueryError {
    #[error("[sbolinv-18001] unknown qualification IRI `{value}`")]
    UnknownQualification { value: String },
    #[error("[sbolinv-18002] capability kind must be an absolute IRI, found `{value}`")]
    InvalidCapabilityKind { value: String },
}

impl QueryError {
    pub const fn rule(&self) -> &'static str {
        match self {
            Self::UnknownQualification { .. } => "sbolinv-18001",
            Self::InvalidCapabilityKind { .. } => "sbolinv-18002",
        }
    }
}

/// Finds eligible catalog candidates without allocating, reserving, or dispatching them.
///
/// The validated input is intentional: effective-activity traversal relies on the
/// profile's local-reference and acyclicity invariants.
pub fn find_qualified_assets<'a>(
    inventory: &ValidatedInventory<'a>,
    query: &CandidateQuery,
) -> Vec<CapabilityCandidate<'a>> {
    let facility = query
        .facility
        .as_ref()
        .map(|iri| Resource::Iri(iri.clone()));
    let mut candidates = Vec::new();
    for asset in inventory.document().assets() {
        if facility
            .as_ref()
            .is_some_and(|expected| asset.facility_id() != Some(expected))
            || !is_effectively_active(inventory.document(), asset)
        {
            continue;
        }
        for offering in asset.capabilities() {
            if offering.is_active() != Some(true)
                || offering.kind() != Some(&query.capability_kind)
                || offering
                    .qualification()
                    .is_none_or(|qualification| qualification < query.minimum_qualification)
            {
                continue;
            }
            candidates.push(CapabilityCandidate { asset, offering });
        }
    }
    candidates.sort_by(|left, right| left.asset.identity().cmp(right.asset.identity()));
    candidates
}

impl<'a> ValidatedInventory<'a> {
    pub fn find_qualified_assets(&self, query: &CandidateQuery) -> Vec<CapabilityCandidate<'a>> {
        find_qualified_assets(self, query)
    }
}

fn is_effectively_active(inventory: &InventoryDocument, asset: AssetRef<'_>) -> bool {
    asset_is_active(inventory, asset, &mut BTreeSet::new(), &mut BTreeSet::new())
}

fn asset_is_active(
    inventory: &InventoryDocument,
    asset: AssetRef<'_>,
    visited_assets: &mut BTreeSet<Resource>,
    visited_zones: &mut BTreeSet<Resource>,
) -> bool {
    if !visited_assets.insert(asset.identity().clone()) {
        return true;
    }
    if asset.is_active() != Some(true) {
        return false;
    }
    if let Some(parent) = asset.part_of()
        && !asset_is_active(inventory, parent, visited_assets, visited_zones)
    {
        return false;
    }
    let Some(location) = asset.located_in_id() else {
        return true;
    };
    if let Some(container) = inventory.asset(location) {
        return asset_is_active(inventory, container, visited_assets, visited_zones);
    }
    inventory
        .zone(location)
        .is_some_and(|zone| zone_is_active(zone, visited_zones))
}

fn zone_is_active(zone: ZoneRef<'_>, visited: &mut BTreeSet<Resource>) -> bool {
    if !visited.insert(zone.identity().clone()) {
        return true;
    }
    if zone.is_active() != Some(true) {
        return false;
    }
    zone.parent_zone()
        .is_none_or(|parent| zone_is_active(parent, visited))
}
