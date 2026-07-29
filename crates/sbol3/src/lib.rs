//! Rust support for the Synthetic Biology Open Language (SBOL) 3.1.0
//! specification.
//!
//! The crate keeps RDF parsing and serialization behind a small adapter while
//! exposing SBOL documents, typed builders, owned typed client objects, and
//! structured validation diagnostics.
//!
//! ```
//! use sbol3::constants::SBO_DNA;
//! use sbol3::prelude::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let component = Component::builder("https://example.org/lab", "my_component")?
//!     .types([SBO_DNA])
//!     .name("My component")
//!     .build()?;
//!
//! let document = Document::from_objects(vec![SbolObject::Component(component)])?;
//! document.check()?;
//!
//! assert_eq!(document.components().count(), 1);
//! # Ok(())
//! # }
//! ```
//!
//! # Mental model
//!
//! SBOL describes designed biology as a graph of typed objects. Every object
//! has a stable [`Iri`] identity, a `displayId`, and a namespace. Objects
//! split into two layers:
//!
//! - **Top-level objects** live directly in a [`Document`]:
//!   [`Component`] (the central design unit: DNA, RNA, protein, complexes,
//!   functional descriptions), [`Sequence`], [`Collection`],
//!   [`CombinatorialDerivation`], [`Implementation`], [`ExperimentalData`],
//!   [`Experiment`], [`Model`], [`Attachment`], plus PROV-O activities
//!   ([`Activity`], [`Plan`], [`Agent`]) and OM unit definitions ([`Unit`]
//!   and friends).
//! - **Owned children** belong to a top-level parent and live nested inside
//!   it: [`SubComponent`] / [`LocalSubComponent`] /
//!   [`ExternallyDefined`] / [`SequenceFeature`] / [`ComponentReference`]
//!   hang off [`Component`]; [`Range`] / [`Cut`] / [`EntireSequence`] hang
//!   off features; [`Interaction`]s contain [`Participation`]s;
//!   [`Constraint`]s relate features.
//!
//! References between objects are typed: a [`SubComponent`] names the
//! [`Component`] it instantiates, an [`ExternallyDefined`] points to a term in
//! an external ontology, and so on. Reference traversal lives on the typed
//! structs and takes anything implementing [`ObjectGraph`]: a [`Document`]
//! for single-file work, a [`DocumentSet`] when references cross document
//! boundaries.
//!
//! # Document lifecycle
//!
//! A typical flow:
//!
//! 1. **Read.** [`Document::read_path`] infers the format from the file
//!    extension; [`Document::read`] takes an explicit [`RdfFormat`] for
//!    in-memory input.
//! 2. **Validate.** [`Document::validate`] returns a full
//!    [`ValidationReport`] with errors, warnings, and per-rule coverage.
//!    Use it when you want to inspect or render the report regardless of
//!    pass/fail state. [`Document::check`] is the `?`-friendly convenience
//!    that maps errors to `Err` (warnings ignored), and
//!    [`Document::check_complete`] additionally fails on
//!    partial-application diagnostics for strict CI gates.
//! 3. **Traverse / mutate.** Typed accessors on [`Document`] iterate each
//!    top-level class (`document.components()`, `document.sequences()`,
//!    `document.activities()`, …). Builders ([`Component::builder`], etc.)
//!    construct new objects without invalidating existing IRIs.
//! 4. **Write.** [`Document::write`] takes an [`RdfFormat`];
//!    `write_turtle`, `write_rdf_xml`, `write_jsonld`, and `write_ntriples`
//!    are shortcut methods. Round-trip preserves unknown extension triples.
//!
//! # Error model
//!
//! Errors are surfaced through four distinct types so callers can branch on
//! the failure mode without parsing strings:
//!
//! - [`ReadError`]: I/O or parse failure when ingesting RDF.
//! - [`BuildError`]: invariant violation at builder time (invalid
//!   `displayId`, malformed namespace, missing required field).
//! - [`WriteError`]: serialization failure.
//! - [`ValidationReport`]: structured diagnostics from validation
//!   (multiple issues per call, each with its `sbol3-*` rule identifier).
//!
//! # Where to go next
//!
//! - **[Crate guide](https://github.com/SynBioDex/sbol-rs/blob/master/docs/crate-guide.md)**:
//!   architectural tour and where each subsystem lives.
//! - **[Validation system overview](https://github.com/SynBioDex/sbol-rs/blob/master/docs/validation.md)**:
//!   what the validator covers, `check` vs `check_complete`, CI wiring,
//!   trust boundaries.
//! - **[RDF I/O](https://github.com/SynBioDex/sbol-rs/blob/master/docs/rdf-io.md)**:
//!   format inference, round-trip guarantees, cross-implementation
//!   conformance.
//! - **[Conformance grid](https://github.com/SynBioDex/sbol-rs/blob/master/docs/conformance.md)**:
//!   generated per-rule status for every SBOL 3.1.0 rule.
//! - **[`prelude`]**: re-exports the symbols you'll need for most code;
//!   `use sbol3::prelude::*;` is the conventional import. It includes the
//!   [`SbolIdentified`] and [`SbolTopLevel`] accessor traits so methods
//!   like `component.name()`, `component.display_id()`, and
//!   `component.namespace()` are available on every typed object.
//! - **[`constants`]**: IRIs for SBO / SO / EDAM / SBOL terms that show up
//!   as builder arguments (`SBO_DNA`, `SO_PROMOTER`, etc.).

#![forbid(unsafe_code)]
// `check`/`check_complete` deliberately return the full ValidationReport in
// both Ok and Err arms so callers always get the report; boxing the Err arm
// would split that surface.
#![allow(clippy::result_large_err)]

mod client;
mod conformance;
pub mod constants;
pub mod design;
mod document;
mod error;
pub mod identity;
mod model;
mod object;
pub mod owl_conformance;
pub mod prelude;
mod resolve;
pub mod schema;
mod specification;
mod validation;
#[doc(hidden)]
pub mod vocab;

pub use client::{
    Activity, ActivityBuilder, Agent, AgentBuilder, Association, AssociationBuilder, Attachment,
    AttachmentBuilder, BinaryPrefix, BinaryPrefixBuilder, Collection, CollectionBuilder,
    CombinatorialDerivation, CombinatorialDerivationBuilder, Component, ComponentBuilder,
    ComponentReference, ComponentReferenceBuilder, CompoundUnit, CompoundUnitBuilder, Constraint,
    ConstraintBuilder, Cut, CutBuilder, EntireSequence, EntireSequenceBuilder, Experiment,
    ExperimentBuilder, ExperimentalData, ExperimentalDataBuilder, ExtensionTriple,
    ExternallyDefined, ExternallyDefinedBuilder, FeatureData, FeatureRef, IdentifiedData,
    IdentifiedExtension, IdentifiedExtensionBuilder, Implementation, ImplementationBuilder,
    Interaction, InteractionBuilder, Interface, InterfaceBuilder, LocalSubComponent,
    LocalSubComponentBuilder, LocationData, LocationRef, Measure, MeasureBuilder, Model,
    ModelBuilder, Participation, ParticipationBuilder, Plan, PlanBuilder, Prefix, PrefixBuilder,
    PrefixData, PrefixedUnit, PrefixedUnitBuilder, Range, RangeBuilder, SIPrefix, SIPrefixBuilder,
    SbolIdentified, SbolObject, SbolTopLevel, Sequence, SequenceBuilder, SequenceFeature,
    SequenceFeatureBuilder, SingularUnit, SingularUnitBuilder, SubComponent, SubComponentBuilder,
    ToRdf, TopLevelData, TryFromObject, Unit, UnitBuilder, UnitData, UnitDivision,
    UnitDivisionBuilder, UnitExponentiation, UnitExponentiationBuilder, UnitMultiplication,
    UnitMultiplicationBuilder, Usage, UsageBuilder, VariableFeature, VariableFeatureBuilder,
};
pub use conformance::render_conformance_report;
pub use document::Document;
pub use error::{BuildError, ReadError, WriteError};
pub use identity::{DisplayId, HashAlgorithm, Namespace, SbolIdentity, SequenceElements};
pub use model::{Identified, SbolClass, TopLevel};
pub use object::ObjectClasses;
pub use owl_conformance::{
    OWL_ONLY_ALLOWLIST, OwlConformanceReport, OwlIdentifiers, OwlPinInfo, RUST_ONLY_ALLOWLIST,
    analyze_owl_conformance, extract_owl_identifiers, extract_vocab_iris,
    render_owl_conformance_report,
};
pub use resolve::{FeatureTrace, ObjectGraph, ReferenceError, VariantSet};
pub use sbol_core::diff::{Diff, ObjectDiff, PropertyChange};
pub use sbol_core::document::{ObjectStore, RawDocument};
pub use sbol_core::object::Object;
pub use sbol_ontology::{Ontology, OntologyRegistry};
pub use sbol_rdf::{Graph as RdfGraph, Iri, Literal, RdfFormat, Resource, Term, Triple};
pub use specification::{SPEC_VERSION, SPECIFICATION_URL};
pub use validation::CachingHttpResolver;
pub use validation::HttpResolver;
pub use validation::{
    AppliedOptions, Blocker, ContentResolver, CoverageKind, DocumentResolver, DocumentSet,
    DocumentSetError, ExternalValidationMode, FileResolver, HashAlgorithmRegistry, Hint,
    NormativeSeverity, NotApplied, NotAppliedReason, PartialApplication, PolicyOptions,
    ResolutionError, ResolutionErrorKind, ResolvedContent, RuleCoverage, RuleOverride, RuleStatus,
    Severity, TopologyCompleteness, UnknownRule, VALIDATION_OUTPUT_SCHEMA_VERSION,
    VALIDATION_RULE_SPEC_CANONICAL_URL, VALIDATION_RULE_SPEC_PATH, VALIDATION_RULE_SPEC_PDF_SHA256,
    VALIDATION_RULE_SPEC_VERSION, ValidationConfig, ValidationContext, ValidationIssue,
    ValidationOptions, ValidationReport, ValidationRuleStatus, to_json, validation_rule_statuses,
};
