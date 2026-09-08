//! Standard SBOL 3 and PROV-O run-ledger construction.

use std::fmt;

use sbol3::{
    Activity, Agent, Association, Component, DisplayId, ExperimentalData, Iri, Plan, Resource,
    Term, ToRdf, Triple, Usage,
};
use thiserror::Error;

use crate::vocabulary::{
    PROV_WAS_GENERATED_BY, RUN_ASSET, RUN_COMPONENT, RUN_INPUT_MATERIAL, SBOL_COMPONENT,
};
use crate::{AssetId, InventoryBuildError, InventoryBuilder, MaterialLotId};

/// Identity of one run Activity added through [`InventoryBuilder::add_run`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(Iri);

impl RunId {
    pub fn as_iri(&self) -> &Iri {
        &self.0
    }

    pub fn as_resource(&self) -> Resource {
        Resource::Iri(self.0.clone())
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Builder for a profile run backed entirely by standard PROV-O/SBOL objects.
#[derive(Clone, Debug)]
pub struct RunBuilder {
    display_id: DisplayId,
    name: Option<String>,
    description: Option<String>,
    started_at_time: Option<String>,
    ended_at_time: Option<String>,
    assets: Vec<AssetId>,
    inputs: Vec<MaterialLotId>,
    input_components: Vec<Resource>,
    generated_components: Vec<Resource>,
    generated_materials: Vec<MaterialLotId>,
    evidence: Vec<Resource>,
    responsibility: Option<(Resource, Resource)>,
}

impl RunBuilder {
    pub fn new(display_id: impl Into<String>) -> Result<Self, RunBuildError> {
        let display_id = display_id.into();
        let display_id = DisplayId::new(display_id.clone()).map_err(|_| {
            RunBuildError::Inventory(InventoryBuildError::InvalidDisplayId(display_id))
        })?;
        Ok(Self {
            display_id,
            name: None,
            description: None,
            started_at_time: None,
            ended_at_time: None,
            assets: Vec::new(),
            inputs: Vec::new(),
            input_components: Vec::new(),
            generated_components: Vec::new(),
            generated_materials: Vec::new(),
            evidence: Vec::new(),
            responsibility: None,
        })
    }

    pub fn name(mut self, value: impl Into<String>) -> Self {
        self.name = Some(value.into());
        self
    }

    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }

    pub fn started_at_time(mut self, value: impl Into<String>) -> Self {
        self.started_at_time = Some(value.into());
        self
    }

    pub fn ended_at_time(mut self, value: impl Into<String>) -> Self {
        self.ended_at_time = Some(value.into());
        self
    }

    pub fn asset(mut self, asset: AssetId) -> Self {
        self.assets.push(asset);
        self
    }

    pub fn input_material(mut self, material: MaterialLotId) -> Self {
        self.inputs.push(material);
        self
    }

    /// A design used as informational input, independently of physical lot usage.
    pub fn input_component(self, component: &Component) -> Self {
        self.input_component_identity(component.identity.clone())
    }

    pub fn input_component_identity(mut self, identity: Resource) -> Self {
        self.input_components.push(identity);
        self
    }

    /// A newly created design. Realizing a lot does not regenerate its design.
    pub fn generated_component(self, component: &Component) -> Self {
        self.generated_component_identity(component.identity.clone())
    }

    pub fn generated_component_identity(mut self, identity: Resource) -> Self {
        self.generated_components.push(identity);
        self
    }

    pub fn generated_material(mut self, material: MaterialLotId) -> Self {
        self.generated_materials.push(material);
        self
    }

    pub fn evidence(mut self, evidence: &ExperimentalData) -> Self {
        self.evidence.push(evidence.identity.clone());
        self
    }

    /// Raw interoperability escape hatch for an existing ExperimentalData identity.
    pub fn evidence_identity(mut self, evidence: Resource) -> Self {
        self.evidence.push(evidence);
        self
    }

    pub fn responsibility(mut self, plan: &Plan, executor: &Agent) -> Self {
        self.responsibility = Some((plan.identity.clone(), executor.identity.clone()));
        self
    }

    /// Raw interoperability escape hatch for existing Plan and Agent identities.
    pub fn responsibility_identities(mut self, plan: Resource, executor: Resource) -> Self {
        self.responsibility = Some((plan, executor));
        self
    }
}

impl InventoryBuilder {
    /// Adds a complete run Activity, its owned Usages and optional Association,
    /// and `prov:wasGeneratedBy` links on declared outputs and evidence.
    pub fn add_run(&mut self, builder: RunBuilder) -> Result<RunId, RunBuildError> {
        if builder.assets.is_empty() {
            return Err(RunBuildError::RequiresAsset);
        }

        let run_id = RunId(self.top_level_identity(&builder.display_id));
        let run_resource = run_id.as_resource();
        let referenced = builder
            .assets
            .iter()
            .map(AssetId::as_resource)
            .chain(builder.inputs.iter().map(MaterialLotId::as_resource))
            .chain(
                builder
                    .generated_materials
                    .iter()
                    .map(MaterialLotId::as_resource),
            )
            .chain(builder.evidence.iter().cloned())
            .chain(builder.input_components.iter().cloned())
            .chain(builder.generated_components.iter().cloned())
            .chain(
                builder
                    .responsibility
                    .iter()
                    .flat_map(|(plan, executor)| [plan.clone(), executor.clone()]),
            )
            .collect::<Vec<_>>();
        if let Some(missing) = referenced
            .iter()
            .find(|identity| !self.contains_identity(identity))
        {
            return Err(RunBuildError::MissingReferencedObject(missing.clone()));
        }

        for identity in builder
            .input_components
            .iter()
            .chain(&builder.generated_components)
        {
            if !self.has_rdf_type(identity, SBOL_COMPONENT) {
                return Err(RunBuildError::NotComponent(identity.clone()));
            }
        }
        let mut usages = Vec::new();
        for (index, asset) in builder.assets.iter().enumerate() {
            usages.push(
                Usage::builder(&run_resource, format!("asset_{}", index + 1))?
                    .entity(asset.as_resource())
                    .had_role([Iri::from_static(RUN_ASSET)])
                    .build()?,
            );
        }
        for (index, material) in builder.inputs.iter().enumerate() {
            usages.push(
                Usage::builder(&run_resource, format!("input_{}", index + 1))?
                    .entity(material.as_resource())
                    .had_role([Iri::from_static(RUN_INPUT_MATERIAL)])
                    .build()?,
            );
        }

        for (index, component) in builder.input_components.iter().enumerate() {
            usages.push(
                Usage::builder(&run_resource, format!("component_{}", index + 1))?
                    .entity(component.clone())
                    .had_role([Iri::from_static(RUN_COMPONENT)])
                    .build()?,
            );
        }

        let association = builder
            .responsibility
            .map(|(plan, executor)| {
                Association::builder(&run_resource, "responsibility")?
                    .agent(executor)
                    .had_plan(plan)
                    .build()
            })
            .transpose()?;

        let mut activity =
            Activity::builder(self.namespace().as_str(), builder.display_id.as_str())?
                .qualified_usage(usages.iter().map(|usage| usage.identity.clone()));
        if let Some(value) = builder.name {
            activity = activity.name(value);
        }
        if let Some(value) = builder.description {
            activity = activity.description(value);
        }
        if let Some(value) = builder.started_at_time {
            activity = activity.started_at_time(value);
        }
        if let Some(value) = builder.ended_at_time {
            activity = activity.ended_at_time(value);
        }
        if let Some(association) = &association {
            activity = activity.add_qualified_association(association.identity.clone());
        }
        let activity = activity.build()?;

        let mut triples = activity.to_rdf_triples()?;
        for usage in &usages {
            triples.extend(usage.to_rdf_triples()?);
        }
        if let Some(association) = association {
            triples.extend(association.to_rdf_triples()?);
        }
        self.commit_triples(triples)?;

        let generated_by = builder
            .generated_materials
            .iter()
            .map(MaterialLotId::as_resource)
            .chain(builder.evidence)
            .chain(builder.generated_components)
            .map(|subject| Triple {
                subject,
                predicate: Iri::from_static(PROV_WAS_GENERATED_BY),
                object: Term::Resource(run_resource.clone()),
            });
        self.extend_existing_triples(generated_by);
        Ok(run_id)
    }
}

/// Failure while adding a standard run-ledger record.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunBuildError {
    #[error(transparent)]
    Inventory(#[from] InventoryBuildError),
    #[error(transparent)]
    Sbol(#[from] sbol3::BuildError),
    #[error("[sbolinv-17003] a profile run must use at least one Asset")]
    RequiresAsset,
    #[error("run references object `{0}` before it has been added to the inventory")]
    MissingReferencedObject(Resource),
    #[error("run Component reference `{0}` is not a local SBOL Component")]
    NotComponent(Resource),
}
