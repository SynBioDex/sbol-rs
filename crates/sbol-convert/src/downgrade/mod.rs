//! Convert SBOL 3 documents to SBOL 2 RDF.
//!
//! Most published synbio content (SynBioHub, iGEM Registry, JBEI ICE
//! today) still consumes SBOL 2. This module is the write-direction
//! counterpart to [`crate::upgrade`]: it takes an SBOL 3
//! [`Document`](sbol3::Document) and produces an [`RdfGraph`] holding
//! the equivalent SBOL 2 RDF, which the caller can serialize in any
//! of the supported RDF formats.
//!
//! ```no_run
//! use sbol3::Document;
//! use sbol3::RdfFormat;
//! use sbol_convert::downgrade;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let document = Document::read_path("design.ttl")?;
//! let (sbol2_graph, report) = downgrade(&document)?;
//! for warning in report.warnings() {
//!     eprintln!("{warning:?}");
//! }
//! let sbol2_xml = sbol2_graph.write(RdfFormat::RdfXml)?;
//! std::fs::write("design.xml", sbol2_xml)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Loss model
//!
//! SBOL 3 unifies several SBOL 2 concepts (notably `ComponentDefinition`
//! and `ModuleDefinition`) into a single `Component`. The downgrade is
//! therefore not strictly bijective; the [`DowngradeReport`] surfaces
//! warnings for cases where the SBOL 2 surface forces a representation
//! choice (unsupported SBOL 3-only types, MapsTo reconstruction from
//! `ComponentReference`+`Constraint` pairs, etc.).
//!
//! Documents produced via [`crate::upgrade::sbol2_to_sbol3`] round-trip
//! back to SBOL 2 through the version-in-IRI identity model and the
//! `https://sbols.org/backport/2_3#` hints the upgrade records.
//! Documents authored as native SBOL 3 will lose more on downgrade;
//! the report explains where.
//!
//! # Component classification
//!
//! Each SBOL 3 `Component` maps to exactly one SBOL 2 class: a
//! `ModuleDefinition` when it carries functional signals (interactions,
//! the FunctionalEntity type, or a subcomponent that is itself a
//! module-definition), otherwise a `ComponentDefinition`. A Component is
//! never split across both classes.
//!
//! For the full conversion model see the [conversion guide][conversion-md].
//!
//! [`RdfGraph`]: sbol3::RdfGraph
//! [conversion-md]: https://github.com/SynBioDex/sbol-rs/blob/master/docs/conversion.md

use std::collections::{HashMap, HashSet};

use crate::sbol2_vocab as v2;
use sbol_core::iri::last_iri_segment as last_segment;
use sbol_rdf::Graph;
use sbol_rdf::{Iri, Resource, Term, Triple};
use sbol3::Document;
use sbol3::vocab as v3;

mod analyze;
mod dispatch;
mod emit;
mod helpers;
mod predicate;
mod preflight;
mod values;

/// Configuration for [`sbol3_to_sbol2`].
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct DowngradeOptions {
    /// Version string to assign top-level objects whose SBOL 3 IRI carried
    /// no version segment. `None` (the default) leaves them unversioned.
    /// SBOL 2 makes `sbol2:version` optional, and the round-trip stays
    /// bit-identical for sources that omitted it. `Some("1")` matches the
    /// libSBOLj / SynBioHub convention of always carrying a version segment.
    pub default_version: Option<String>,
}

impl DowngradeOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Non-fatal observations from [`sbol3_to_sbol2`]. Each warning
/// records a case where the SBOL 2 representation forced a choice
/// the SBOL 3 source didn't have to make.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DowngradeWarning {
    /// A `Constraint` couldn't be folded back into a `MapsTo` because
    /// its `subject` or `object` didn't match the expected
    /// `ComponentReference` pattern. The constraint is emitted as a
    /// plain SBOL 2 `SequenceConstraint` in the output.
    UnresolvableConstraintToMapsTo { constraint: String, reason: String },
    /// A `ComponentReference` didn't have a matching `Constraint`
    /// partner. The reference is dropped from the SBOL 2 output;
    /// SBOL 2 has no standalone equivalent.
    OrphanComponentReference { component_reference: String },
    /// A subject of `rdf:type sbol3:T` had no SBOL 2 equivalent for
    /// `T`. The subject and its triples are dropped from the output.
    UnsupportedSbol3Type { subject: String, sbol3_type: String },
    /// A top-level object carried no version in its IRI and
    /// [`DowngradeOptions::default_version`] was set, so the downgrade
    /// synthesized this version on it. Only fires when synthesis is
    /// opted into.
    SynthesizedVersion { subject: String, version: String },
    /// Two or more distinct SBOL 3 subjects rewrite to the same SBOL 2
    /// versioned IRI — for example a Component at `<lab/1/foo>` and a
    /// separate Component at `<lab/foo/1>` both mapping to `<lab/foo/1>`.
    /// The conversion
    /// proceeds and every input triple is preserved, but they all land
    /// at the same SBOL 2 subject, silently merging the entities into
    /// a single chimeric ComponentDefinition / ModuleDefinition / etc.
    /// The input is technically non-conformant (the implied SBOL 2
    /// versioned identities should be unique), but the merge is
    /// otherwise invisible; this warning surfaces the situation so
    /// callers can audit. Mirrors
    /// [`crate::upgrade::UpgradeWarning::IdentityCollision`].
    IdentityCollision {
        canonical: String,
        sources: Vec<String>,
    },
}

/// Report of every non-fatal issue plus tallies of what was rewritten.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct DowngradeReport {
    warnings: Vec<DowngradeWarning>,
    counts: DowngradeCounts,
}

impl DowngradeReport {
    pub fn warnings(&self) -> &[DowngradeWarning] {
        &self.warnings
    }

    pub fn counts(&self) -> &DowngradeCounts {
        &self.counts
    }

    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }

    pub(crate) fn push(&mut self, warning: DowngradeWarning) {
        self.warnings.push(warning);
    }
}

/// Tally of how many SBOL 3 constructs were rewritten.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct DowngradeCounts {
    pub components_to_component_definition: usize,
    pub components_to_module_definition: usize,
    pub sub_components_emitted: usize,
    pub sequence_features_emitted: usize,
    pub maps_to_reconstructed: usize,
    pub identities_versioned: usize,
    pub identities_synthesized: usize,
}

/// Errors returned by [`sbol3_to_sbol2`]. These are fatal; non-fatal
/// observations live on [`DowngradeReport`] instead.
#[derive(Debug)]
#[non_exhaustive]
pub enum DowngradeError {
    /// [`DowngradeOptions::default_version`] was set to `Some("")`.
    /// Use `None` to disable version synthesis or `Some(v)` with a
    /// non-empty value.
    InvalidDefaultVersion(String),
}

impl std::fmt::Display for DowngradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDefaultVersion(msg) => {
                write!(f, "invalid default version: {msg}")
            }
        }
    }
}

impl std::error::Error for DowngradeError {}

/// Converts an SBOL 3 [`Document`] to an [`RdfGraph`] of SBOL 2 RDF.
///
/// The caller chooses how to serialize the resulting graph
/// (`graph.write(RdfFormat::RdfXml)` / `Turtle` / …).
///
/// [`RdfGraph`]: sbol3::RdfGraph
pub fn sbol3_to_sbol2(
    document: &Document,
    options: DowngradeOptions,
) -> Result<(Graph, DowngradeReport), DowngradeError> {
    if matches!(options.default_version.as_deref(), Some("")) {
        return Err(DowngradeError::InvalidDefaultVersion(
            "default version must be non-empty when set; use None to disable synthesis".to_string(),
        ));
    }
    let mut engine = Engine::new(document, options);
    engine.preflight();
    engine.convert();
    Ok((Graph::new(engine.output_triples), engine.report))
}

/// Downgrades an SBOL 3 [`Document`] to SBOL 2 RDF, returning the
/// resulting graph alongside a [`DowngradeReport`]. See
/// [`downgrade`](crate::downgrade) for the loss model.
pub fn downgrade(document: &Document) -> Result<(Graph, DowngradeReport), DowngradeError> {
    sbol3_to_sbol2(document, DowngradeOptions::default())
}

/// Like [`downgrade`], with explicit [`DowngradeOptions`].
pub fn downgrade_with(
    document: &Document,
    options: DowngradeOptions,
) -> Result<(Graph, DowngradeReport), DowngradeError> {
    sbol3_to_sbol2(document, options)
}

struct Engine<'a> {
    input: &'a Document,
    options: DowngradeOptions,
    /// Subject IRI → version string (from the version segment of the SBOL 3
    /// IRI, or the synthesized default). Determines the `/version` suffix
    /// added back to top-level identities.
    versions: HashMap<String, String>,
    /// Subjects whose version came from the SBOL 3 IRI itself, rather than
    /// being synthesized from [`DowngradeOptions::default_version`]. Only
    /// these get a `sbol2:version` property triple emitted in the SBOL 2
    /// output. Synthesizing the IRI segment is necessary for SBOL 2
    /// structure, but emitting a fake version triple would pollute
    /// round-trips of documents that originally carried no `sbol2:version`
    /// (the SBOL 2 spec makes it optional).
    preserved_versions: HashSet<String>,
    /// Subject IRI → its SBOL 2 persistent identity (the unversioned IRI,
    /// recomputed during identity restoration).
    persistent_identities: HashMap<String, String>,
    /// Subject IRI → primary resolved SBOL 2 type, from the SBOL 3 → SBOL 2
    /// type table. Used to disambiguate context-dependent predicates (e.g.
    /// `hasFeature` becomes `component` for CDs, `functionalComponent` for MDs).
    resolved_types: HashMap<String, String>,
    /// Subject IRI → every resolved SBOL 2 type asserted for the subject.
    /// Used when a context-sensitive predicate cares about an additional
    /// type rather than the primary one selected for structural routing.
    resolved_type_sets: HashMap<String, HashSet<String>>,
    /// SubComponent IRI → IRI of the Component it `instanceOf`s.
    /// Lets us decide whether a SubComponent under an MD should be
    /// emitted as `Module` (target is MD) or `FunctionalComponent`
    /// (target is CD).
    subcomponent_targets: HashMap<String, String>,
    /// SubComponent IRIs the upgrade marked (`backport:sbol2OriginatesFromModule`)
    /// as derived from an SBOL 2 `Module`, so the downgrade restores them as
    /// `Module` rather than `FunctionalComponent`.
    module_origin_subcomponents: HashSet<String>,
    /// Location IRIs the upgrade marked (`backport:sbol2LocationSequenceNull`)
    /// as having had no explicit sequence in SBOL 2. Their SBOL 3 `hasSequence`
    /// (inferred from the parent or a synthesized empty Sequence) is dropped on
    /// the way down rather than re-emitted as `sbol2:sequence`.
    null_sequence_locations: HashSet<String>,
    /// Top-level subjects whose IRIs need a version suffix appended.
    top_levels: HashSet<String>,
    /// SBOL 3 subject IRI → its `sbol3:hasNamespace` value. Emitted back
    /// onto the corresponding SBOL 2 object as `backport:sbol3namespace`
    /// so sbol-utilities / sbolgraph can reconstruct the SBOL 3 namespace.
    sbol3_namespaces: HashMap<String, String>,
    /// IRI rewrite map (SBOL 3 IRI → SBOL 2 IRI). Built during
    /// preflight; applied to every subject and object during the main
    /// pass.
    iri_rewrites: HashMap<String, String>,
    /// SubComponent IRI → info needed to reconstruct or synthesize an SBOL 2
    /// SequenceAnnotation wrapper. Populated for SubComponents that carry the
    /// upgrade's `backport:sequenceAnnotationDisplayId` hint, and for native
    /// SBOL 3 SubComponents with `hasLocation`.
    sa_collapses: HashMap<String, SaCollapseInfo>,
    /// ComponentReference IRI → reconstructed MapsTo. The upgrade
    /// decomposed each SBOL 2 MapsTo into a paired ComponentReference +
    /// Constraint; the downgrade walks them back into a single MapsTo
    /// attached to the inChildOf carrier.
    mapsto_reconstructions: HashMap<String, MapsToReconstruction>,
    /// Constraint IRI → set of triples to drop because the constraint
    /// is the back-half of a MapsTo decomposition (not a real SBOL 3
    /// Constraint).
    mapsto_constraints: HashSet<String>,
    /// Subjects whose triples must be dropped from the SBOL 2 output
    /// because they are SBOL 3-only structural plumbing the downgrade
    /// recognized but could not fold. Without suppression these would
    /// survive as orphan subjects carrying SBOL 3 predicates, e.g.
    /// ComponentReferences whose paired Constraint was missing fields,
    /// which couldn't be reconstructed into a MapsTo.
    discarded_subjects: HashSet<String>,
    /// FunctionalComponent (SBOL 3 SubComponent) IRI → direction
    /// recovered from the enclosing Interface's
    /// input/output/nondirectional triples.
    fc_directions: HashMap<String, FcDirection>,
    /// Interface IRIs to drop from the output (their data is folded
    /// into per-FC `sbol2:direction` triples).
    interface_subjects: HashSet<String>,
    /// Per-Component split decision. SBOL 2 separates the structural
    /// (ComponentDefinition) and functional (ModuleDefinition) concerns
    /// that SBOL 3 unifies into one [`Component`]. The downgrade
    /// classifies each Component into a single SBOL 2 class (CD or MD).
    component_splits: HashMap<String, ComponentSplit>,
    /// SBOL 3 SubComponent IRI → SBOL 3 Component IRI of its enclosing
    /// parent (built from the input `sbol3:hasFeature` index in
    /// preflight). Lets us look up the parent's classified shape.
    feature_parent: HashMap<String, String>,
    /// Every IRI the downgrade has either observed in the input graph
    /// or allocated as a synthesized SBOL 2 subject so far. Every
    /// IRI-synthesis site (SA wrapper, reconstructed MapsTo, synthesized
    /// Constraint) routes its candidate IRI through one of the
    /// `next_available_*` helpers against this set. The invariant the
    /// pool enforces is: **no two distinct SBOL 2 entities ever land at
    /// the same IRI**, regardless of how creatively the input names
    /// things. Without it, every synthesis site is a potential
    /// silent-merge bug.
    used_iris: HashSet<String>,
    /// (subject IRI, dcterms predicate) → set of objects already present
    /// in the input graph. Used to suppress duplicate emission of
    /// `dcterms:title` / `dcterms:description` when both Dublin Core
    /// and SBOL 3 forms (`sbol3:name` / `sbol3:description`) exist for
    /// the same value. Without this the downgrade would emit each
    /// dcterms triple twice. O(1) lookup; the equivalent scan in
    /// `subject_already_has` was O(N) per call.
    dcterms_index: HashMap<(String, &'static str), HashSet<Term>>,
    output_triples: Vec<Triple>,
    report: DowngradeReport,
}

/// Whether a Component downgrades to a single SBOL 2 class or splits
/// into a `ComponentDefinition` + `ModuleDefinition` pair joined by a
/// synthesized `FunctionalComponent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComponentShape {
    /// Maps to a single `sbol2:ComponentDefinition`.
    CdOnly,
    /// Maps to a single `sbol2:ModuleDefinition`.
    MdOnly,
}

#[derive(Clone)]
/// Each SBOL 3 Component maps to exactly one SBOL 2 class. `cd_iri` and
/// `md_iri` both hold the Component's own SBOL 2 IRI; only the half matching
/// [`ComponentShape`] is emitted.
struct ComponentSplit {
    shape: ComponentShape,
    cd_iri: String,
}

/// Direction recovered from an SBOL 3 Interface for an enclosed
/// FunctionalComponent SubComponent.
#[derive(Clone, Copy, Debug)]
enum FcDirection {
    In,
    Out,
    Inout,
}

#[derive(Clone, Copy)]
enum InterfaceFeatureKind {
    Component,
    FunctionalComponent,
    Module,
}

impl FcDirection {
    fn sbol2_iri(self) -> &'static str {
        match self {
            FcDirection::In => v2::SBOL2_DIRECTION_IN,
            FcDirection::Out => v2::SBOL2_DIRECTION_OUT,
            FcDirection::Inout => v2::SBOL2_DIRECTION_INOUT,
        }
    }
}

/// All the data the downgrade needs to re-emit an SBOL 2 MapsTo from
/// the ComponentReference + Constraint pair the upgrade produced.
struct MapsToReconstruction {
    /// SBOL 2 IRI of the carrier SubComponent (Module / FunctionalComponent)
    /// the MapsTo hangs off of in SBOL 2.
    carrier_v3: String,
    /// MapsTo's `displayId` (preserved on the ComponentReference).
    display_id: String,
    /// MapsTo `local` value: the SBOL 3 IRI of the FC at the same
    /// level as the carrier, recovered from the Constraint's `subject`.
    local_v3: String,
    /// MapsTo `remote` value: the SBOL 3 IRI of the FC inside the
    /// carrier's instanceOf target, recovered from the
    /// ComponentReference's `refersTo`.
    remote_v3: String,
    /// MapsTo `refinement` value, inferred from the Constraint's restriction
    /// and the CRef's position in the `sbol3:subject` / `sbol3:object` pair
    /// per SBOL 3.1.0 §10.2:
    ///    - `replaces` + CRef in subject → `useRemote`
    ///    - `replaces` + CRef in object  → `useLocal`
    ///    - `verifyIdentical` + CRef in either → `verifyIdentical`
    ///
    /// `None` when position can't resolve it; the emitter then synthesizes
    /// `useLocal` so no data is lost.
    refinement: Option<String>,
}

/// Records what the downgrade needs to know about a SubComponent whose
/// Location-bearing SBOL 3 shape needs an SBOL 2 SequenceAnnotation wrapper.
/// This covers both SubComponents that originated from an SBOL 2
/// SequenceAnnotation collapse and native SBOL 3 SubComponents with
/// `hasLocation`.
struct SaCollapseInfo {
    /// `displayId` the SBOL 2 SequenceAnnotation should carry. For
    /// round-tripped SBOL 2 this is restored from the upgrade's
    /// `backport:sbol2OriginalSequenceAnnotationURI`; for native SBOL 3 it is
    /// synthesized as `{subComponent}_{firstLocation}`.
    sa_display_id: String,
    /// SBOL 2 IRI of the reconstructed SequenceAnnotation, minus the version
    /// suffix. Derived as `{parent_cd}/{sa_display_id}`, collision-disambiguated
    /// against the shared used-IRI pool.
    sa_iri_unversioned: String,
    /// Original SBOL 3 IRI of the parent Component, used to inherit its
    /// top-level version.
    parent_component: String,
    /// SBOL 3 IRI of the Component that owns the SequenceAnnotation in the
    /// SBOL 2 output.
    parent_cd: String,
    /// SBOL 3 IRIs of every Location attached to the SubComponent via
    /// `hasLocation`. The reconstructed SA emits `sbol2:location` to each.
    locations: Vec<String>,
}
