//! Convert SBOL 2 RDF documents to SBOL 3.
//!
//! Most published synbio content (SynBioHub, iGEM Registry, JBEI ICE)
//! predates SBOL 3 and remains in SBOL 2 form. This module reads SBOL 2 RDF
//! and emits SBOL 3 RDF that can be loaded by the rest of the crate and
//! validated like any natively-authored SBOL 3 document.
//!
//! Conversion happens at the RDF triple level and matches the conversion
//! decisions of the SynBioDex/SBOL-Converter reference. No external runtime is
//! required: input goes in as bytes (any RDF serialization the [`RdfFormat`]
//! enum supports), output comes back as bytes plus an [`UpgradeReport`] of
//! warnings.
//!
//! ```no_run
//! use sbol_convert::upgrade_from_sbol2_path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let (document, report) = upgrade_from_sbol2_path("design.xml")?;
//!
//! for warning in report.warnings() {
//!     eprintln!("{warning:?}");
//! }
//!
//! document.check()?;
//! # Ok(())
//! # }
//! ```
//!
//! The [`Document`] is always returned when the input parses as valid SBOL 2
//! RDF. Callers gate on validation with the existing [`Document::check`]
//! method if they want a strict pipeline.
//!
//! For the full conversion model (workflows organized by what you have, the
//! `https://sbols.org/backport/2_3#` namespace, structural collapses
//! (SequenceAnnotation, MapsTo, Interface), known divergences, and known
//! limitations) see the [conversion guide][conversion-md].
//!
//! [`Document`]: sbol3::Document
//! [`Document::check`]: sbol3::Document::check
//! [`RdfFormat`]: sbol3::RdfFormat
//! [conversion-md]: https://github.com/SynBioDex/sbol-rs/blob/master/docs/conversion.md

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::sbol2_vocab as v2;
use sbol_rdf::{Graph, ParseError, RdfFormat};
use sbol_rdf::{Iri, Resource, Term, Triple};
use sbol3::Document;
use sbol3::vocab as v3;

mod emit;
mod engine;
mod identity;
mod values;

use identity::IdentityMap;

/// Configuration for [`sbol2_to_sbol3`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct UpgradeOptions {
    /// Default namespace to attach to top-level objects whose namespace
    /// cannot be derived from the input (no `persistentIdentity`, no
    /// `displayId`, IRI parses as opaque). If `None`, an
    /// [`UpgradeWarning::NamespaceFallback`] is emitted and the object is
    /// left without `hasNamespace`.
    pub default_namespace: Option<Iri>,

    /// Whether to stamp the conversion-provenance annotations (the original
    /// SBOL 2 identities, collapsed-SequenceAnnotation and temp-sequence
    /// markers) under the `https://sbols.org/backport/2_3#` namespace so a
    /// later SBOL 3 → SBOL 2 downgrade can reconstruct the SBOL 2 shape.
    /// Defaults to `true`.
    pub preserve_backport: bool,
}

impl Default for UpgradeOptions {
    fn default() -> Self {
        Self {
            default_namespace: None,
            preserve_backport: true,
        }
    }
}

impl UpgradeOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Warnings emitted by [`sbol2_to_sbol3`]. The presence of warnings does not
/// stop conversion. Every warning records something the upgrade could not
/// translate cleanly but chose not to fail on.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UpgradeWarning {
    /// A top-level subject's `hasNamespace` could not be derived from
    /// `persistentIdentity` + `displayId` or from the IRI's URL origin.
    /// The upgrade either used [`UpgradeOptions::default_namespace`]
    /// or left the subject without `hasNamespace`. URL-origin
    /// derivation is the canonical fallback for HTTP-style IRIs and
    /// does not warn.
    NamespaceFallback {
        subject: String,
        source: NamespaceSource,
    },
    /// A `sbol2:MapsTo` was decomposed into a Constraint, but the local or
    /// remote side could not be resolved to a SubComponent. The constraint
    /// was skipped.
    UnresolvedMapsTo { mapsto: String, side: MapsToSide },
    /// A `sbol2:MapsTo` carried a `refinement` value with no SBOL 3
    /// Constraint equivalent (notably `merge`). The Constraint is still
    /// emitted, with its restriction coerced to `sbol3:verifyIdentical`.
    UnsupportedRefinement { mapsto: String, refinement: String },
    /// A `sbol2:SequenceAnnotation` referenced a `sbol2:component`. The
    /// upgrade collapsed the SA shell onto the referenced SubComponent.
    SequenceAnnotationWithComponent { annotation: String },
    /// A subject carried an SBOL 2 type the upgrade does not understand.
    /// The subject is dropped; the unknown class has no SBOL 3 equivalent.
    UnknownSbol2Type { subject: String, sbol2_type: String },
    /// A Location belongs to a SequenceFeature on a Component that has no
    /// (or more than one) `sbol2:sequence`, so `sbol3:hasSequence` could not
    /// be unambiguously inferred. The resulting SBOL 3 document will fail
    /// `sbol3-10110` cardinality validation until the source data is
    /// corrected to declare exactly one Sequence per Component.
    LocationWithoutSequence {
        location: String,
        component: String,
        sequence_count: usize,
    },
    /// Two or more distinct SBOL 2 subjects in the input share a
    /// canonical SBOL 3 IRI after version-stripping. For example,
    /// `<lab/foo/1>` (whose canonical is `<lab/foo>`) and
    /// `<lab/foo>` itself. The conversion preserves every input triple
    /// but lands all of them at the same SBOL 3 subject, silently
    /// merging the entities into a single chimeric Component / Sequence
    /// / etc. The input was technically non-conformant SBOL 2
    /// (persistentIdentity should be unique), but the merge is
    /// otherwise indistinguishable from intended behavior; this warning
    /// surfaces the situation so callers can audit the source data.
    IdentityCollision {
        canonical: String,
        sources: Vec<String>,
    },
}

/// Indicates which path was taken when a namespace had to be derived without
/// a usable `persistentIdentity` + `displayId` pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NamespaceSource {
    /// Parsed scheme+host of the subject IRI.
    UrlOrigin,
    /// Used [`UpgradeOptions::default_namespace`].
    DefaultOption,
    /// No fallback applied; the object has no `hasNamespace` in the output.
    None,
}

/// Which side of a MapsTo failed to resolve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MapsToSide {
    Local,
    Remote,
    /// The MapsTo had no enclosing
    /// `sbol2:Module` / `sbol2:FunctionalComponent`
    /// carrier: nothing in the source pointed at it via the
    /// `sbol2:mapsTo` containment predicate, so the upgrade has no
    /// place to attach the resulting ComponentReference + Constraint
    /// pair.
    Carrier,
}

/// Tally of how many SBOL 2 constructs the upgrade rewrote into SBOL 3
/// constructs. Useful for end-of-run summaries and CI signals (e.g. an
/// expected document type accidentally dropping to zero across a refactor).
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct UpgradeCounts {
    /// `sbol2:ComponentDefinition` subjects rewritten to `sbol3:Component`.
    pub component_definitions: usize,
    /// `sbol2:ModuleDefinition` subjects rewritten to `sbol3:Component`
    /// (with synthesized `SBO:functionalEntity` type).
    pub module_definitions: usize,
    /// `sbol2:Component` / `sbol2:Module` / `sbol2:FunctionalComponent`
    /// subjects rewritten to `sbol3:SubComponent`.
    pub sub_components: usize,
    /// Standalone `sbol2:SequenceAnnotation` subjects rewritten to
    /// `sbol3:SequenceFeature` (i.e. SAs that did NOT carry a
    /// `sbol2:component` reference).
    pub sequence_features: usize,
    /// SequenceAnnotations that referenced a `sbol2:component` and were
    /// collapsed onto the referenced SubComponent.
    pub sequence_annotations_collapsed: usize,
    /// `sbol2:MapsTo` subjects decomposed into ComponentReference +
    /// Constraint pairs.
    pub mapstos_decomposed: usize,
    /// `sbol3:Interface` objects synthesized from FunctionalComponent
    /// direction information.
    pub interfaces_synthesized: usize,
    /// Locations that received a `sbol3:hasSequence` triple inferred from
    /// their enclosing Component's single sequence.
    pub locations_with_inferred_sequence: usize,
}

/// Report of every non-fatal issue encountered during an upgrade run, plus
/// counts of what the converter rewrote.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct UpgradeReport {
    warnings: Vec<UpgradeWarning>,
    counts: UpgradeCounts,
}

impl UpgradeReport {
    /// Iterates over every warning recorded during the run.
    pub fn warnings(&self) -> &[UpgradeWarning] {
        &self.warnings
    }

    /// Returns the per-construct counts.
    pub fn counts(&self) -> &UpgradeCounts {
        &self.counts
    }

    /// Returns `true` if no warnings were recorded.
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }

    pub(crate) fn push(&mut self, warning: UpgradeWarning) {
        self.warnings.push(warning);
    }
}

/// Errors returned by [`sbol2_to_sbol3`].
#[derive(Debug)]
#[non_exhaustive]
pub enum UpgradeError {
    /// The input bytes did not parse as RDF.
    Parse(ParseError),
    /// The input parsed, but no SBOL 2 typed subjects were detected. Pass an
    /// SBOL 2 document; SBOL 3 input does not need an upgrade.
    NotSbol2,
}

impl std::fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "failed to parse input as RDF: {err}"),
            Self::NotSbol2 => write!(
                f,
                "input contains no SBOL 2 typed subjects \
                 (http://sbols.org/v2# rdf:type) — nothing to upgrade",
            ),
        }
    }
}

impl std::error::Error for UpgradeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(err) => Some(err),
            Self::NotSbol2 => None,
        }
    }
}

impl From<ParseError> for UpgradeError {
    fn from(err: ParseError) -> Self {
        Self::Parse(err)
    }
}

/// Converts an SBOL 2 RDF graph to SBOL 3 RDF.
///
/// The input graph must contain at least one subject typed in the SBOL 2
/// namespace (`http://sbols.org/v2#…`); otherwise [`UpgradeError::NotSbol2`]
/// is returned. Returns the new graph plus an [`UpgradeReport`] of any
/// non-fatal issues.
pub fn sbol2_to_sbol3(
    graph: &Graph,
    options: UpgradeOptions,
) -> Result<(Graph, UpgradeReport), UpgradeError> {
    let mut engine = Engine::new(graph, options);
    engine.preflight()?;
    engine.convert();
    Ok((Graph::new(engine.output_triples), engine.report))
}

/// Convenience wrapper: parses input bytes, calls [`sbol2_to_sbol3`], then
/// returns both the SBOL 3 [`Graph`] and the [`UpgradeReport`].
pub fn parse_and_upgrade(
    input: &str,
    format: RdfFormat,
    options: UpgradeOptions,
) -> Result<(Graph, UpgradeReport), UpgradeError> {
    let graph = Graph::parse(input, format)?;
    sbol2_to_sbol3(&graph, options)
}

/// Upgrades an SBOL 2 RDF document to SBOL 3 and returns the resulting
/// [`Document`] alongside an [`UpgradeReport`] of any non-fatal issues
/// encountered during conversion.
///
/// The returned [`Document`] is always produced when the input parses as
/// valid SBOL 2 RDF. Call [`Document::check`] if you want a strict
/// pipeline that rejects content the upgrade could not coerce into
/// fully-conformant SBOL 3.
pub fn upgrade_from_sbol2(
    input: &str,
    format: RdfFormat,
) -> Result<(Document, UpgradeReport), UpgradeError> {
    upgrade_from_sbol2_with(input, format, UpgradeOptions::default())
}

/// Like [`upgrade_from_sbol2`], with explicit [`UpgradeOptions`].
pub fn upgrade_from_sbol2_with(
    input: &str,
    format: RdfFormat,
    options: UpgradeOptions,
) -> Result<(Document, UpgradeReport), UpgradeError> {
    let parsed = Graph::parse(input, format).map_err(UpgradeError::Parse)?;
    let (upgraded, report) = sbol2_to_sbol3(&parsed, options)?;
    Ok((Document::from_rdf_graph(upgraded), report))
}

/// Reads an SBOL 2 RDF file from disk and upgrades it to SBOL 3. The
/// format is inferred from the path's extension (`.ttl`, `.rdf`, `.xml`,
/// `.jsonld`, `.nt`).
pub fn upgrade_from_sbol2_path(
    path: impl AsRef<Path>,
) -> Result<(Document, UpgradeReport), UpgradeFromPathError> {
    let path = path.as_ref();
    let format =
        infer_sbol2_rdf_format(path).ok_or_else(|| UpgradeFromPathError::UnknownFormat {
            path: path.to_path_buf(),
            extension: path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(str::to_owned),
        })?;
    let input = std::fs::read_to_string(path).map_err(|source| UpgradeFromPathError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    upgrade_from_sbol2(&input, format).map_err(UpgradeFromPathError::Upgrade)
}

fn infer_sbol2_rdf_format(path: &Path) -> Option<RdfFormat> {
    if let Some(format) = RdfFormat::from_path(path) {
        return Some(format);
    }
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    (extension == "xml").then_some(RdfFormat::RdfXml)
}

/// Errors returned by [`upgrade_from_sbol2_path`].
#[derive(Debug)]
#[non_exhaustive]
pub enum UpgradeFromPathError {
    /// Failed to read the file at the given path.
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// The path's extension did not match any supported RDF serialization.
    UnknownFormat {
        path: std::path::PathBuf,
        extension: Option<String>,
    },
    /// The file was loaded but the upgrade itself failed.
    Upgrade(UpgradeError),
}

impl std::fmt::Display for UpgradeFromPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "failed to read {}: {source}", path.display()),
            Self::UnknownFormat { path, extension } => {
                let ext = extension.as_deref().unwrap_or("<none>");
                write!(
                    f,
                    "unsupported extension `{ext}` for {} — supported: .ttl, .rdf, .jsonld, .nt",
                    path.display()
                )
            }
            Self::Upgrade(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for UpgradeFromPathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::UnknownFormat { .. } => None,
            Self::Upgrade(err) => Some(err),
        }
    }
}

#[derive(Clone, Copy)]
enum FcDirection {
    Input,
    Output,
    Inout,
}

/// Per-MapsTo selector for the constraint shape emitted by
/// [`Engine::synthesize_mapsto_decomposition`]. Derived from the source
/// MapsTo's `sbol2:refinement` value (or its absence).
#[derive(Clone, Copy)]
enum RefinementShape {
    UseLocal,
    UseRemote,
    VerifyIdentical,
}

#[derive(Default, Clone)]
struct MapsToInfo {
    local: Option<String>,
    remote: Option<String>,
    refinement: Option<String>,
    display_id: Option<String>,
}

struct Engine<'a> {
    input: &'a Graph,
    options: UpgradeOptions,
    identity: IdentityMap,
    /// Subject IRI → its rdf:type IRI string, for every subject the
    /// upgrade may need to namespace (SBOL 2 typed subjects, plus the
    /// PROV top-level classes that SBOL 2 documents commonly carry).
    /// Both populate `hasNamespace` on emission.
    typed_subjects: HashMap<String, String>,
    /// Original CD IRI → list of Sequence IRIs it references via
    /// `sbol2:sequence`. Used to infer `hasSequence` on Locations after
    /// conversion.
    cd_sequences: HashMap<String, Vec<String>>,
    /// Original Location IRI (Range/Cut/GenericLocation) → original SA IRI
    /// that owns it. Used together with [`sa_to_cd`] to walk back to the
    /// owning Component and infer the Location's `hasSequence`.
    location_to_sa: HashMap<String, String>,
    /// Original SA IRI → original CD IRI that owns it.
    sa_to_cd: HashMap<String, String>,
    /// Original SA IRI → original SubComponent (Component) IRI it references
    /// via `sbol2:component`. When this mapping is non-empty for an SA, the
    /// SA shell is discarded and its Locations move onto the referenced
    /// SubComponent (SEP-010 / sbolgraph SequenceAnnotation collapse rule).
    sa_to_subcomponent: HashMap<String, String>,
    /// Subject IRI → its SBOL 2 displayId. Used when structural rewrites move
    /// child subjects under a different parent and must preserve SBOL 3 child
    /// identity rules.
    display_ids: HashMap<String, String>,
    /// Owning-relation index: child subject IRI → parent subject IRI. Built
    /// from every SBOL 2 containment predicate (module, functionalComponent,
    /// component, mapsTo, interaction, participation, location, etc.). Used
    /// by [`Engine::owning_top_level`] to walk back to the enclosing
    /// top-level subject when synthesizing new triples (e.g. attaching
    /// MapsTo-derived ComponentReferences to the enclosing Component).
    owned_by: HashMap<String, String>,
    /// Original MapsTo IRI → its parsed local/remote/refinement values plus
    /// the MapsTo's displayId.
    mapsto_info: HashMap<String, MapsToInfo>,
    /// Original FunctionalComponent IRI → the SBOL 3 Interface slot it
    /// should occupy. SBOL 2 `inout` and public+`none` components both map
    /// to SBOL 3 `nondirectional`.
    fc_directions: HashMap<String, FcDirection>,
    /// FunctionalComponents that explicitly declared `sbol2:direction none`.
    /// The SBOL 2→3 mapping still lists these under Interface.nondirectional
    /// when their access is public.
    fc_direction_none: HashSet<String>,
    /// FunctionalComponents that explicitly declared `sbol2:access public`.
    fc_public_access: HashSet<String>,
    output_triples: Vec<Triple>,
    /// Subjects already given a `sbol3:hasNamespace` triple. Used to avoid
    /// duplicate emissions when a subject has multiple `sbol2:displayId`
    /// triples or appears more than once in pre-scan.
    namespaced_subjects: HashSet<String>,
    /// Every IRI the upgrade has observed in the input graph (rewritten
    /// to its canonical SBOL 3 form) or allocated as a synthesized
    /// SBOL 3 subject so far. Every IRI-synthesis site (SA-collapse
    /// Location rewrite, MapsTo decomposition CRef/Constraint,
    /// synthesized Interface) routes its candidate IRI through
    /// `next_available_*` against this pool. The invariant the pool
    /// enforces (mirrored from the downgrade) is: **no two distinct
    /// SBOL 3 entities ever land at the same IRI**, regardless of how
    /// creatively the input names things.
    used_iris: HashSet<String>,
    /// Original Location IRI → displayId literal value to emit on the
    /// rewritten subject. Populated only when the SA-collapse Location
    /// rewrite had to disambiguate the Location's new IRI with a `_N`
    /// suffix, so the emitted `sbol3:displayId` still matches the
    /// IRI's last segment (sbol3-10204 compliance).
    location_display_id_overrides: HashMap<String, String>,
    /// Canonical IRIs of custom-typed (GenericTopLevel) subjects that carry
    /// SBOL 2 identity properties but no recognized SBOL 2 / PROV / OM type.
    /// SBOL 3 has no GenericTopLevel class, so each is treated as an SBOL 3
    /// top-level: its custom `rdf:type` is retained and it receives a
    /// synthesized `sbol3:hasNamespace`.
    generic_top_levels: HashSet<String>,
    report: UpgradeReport,
}

fn is_top_level_sbol2(sbol2_type: &str) -> bool {
    matches!(
        sbol2_type,
        v2::SBOL2_COMPONENT_DEFINITION
            | v2::SBOL2_MODULE_DEFINITION
            | v2::SBOL2_SEQUENCE
            | v2::SBOL2_MODEL
            | v2::SBOL2_COLLECTION
            | v2::SBOL2_IMPLEMENTATION
            | v2::SBOL2_ATTACHMENT
            | v2::SBOL2_EXPERIMENT
            | v2::SBOL2_EXPERIMENTAL_DATA
            | v2::SBOL2_COMBINATORIAL_DERIVATION
    ) || is_prov_top_level(sbol2_type)
}

/// PROV classes that act as SBOL 3 top-level objects (they appear at the
/// document root and need `sbol3:hasNamespace`). PROV `Association` and
/// `Usage` are intentionally absent; those are children of `Activity`.
fn is_prov_top_level(type_iri: &str) -> bool {
    matches!(
        type_iri,
        v3::PROV_ACTIVITY | v3::PROV_AGENT_CLASS | v3::PROV_PLAN
    )
}

fn type_precedence(type_iri: &str) -> u8 {
    if is_known_sbol2_type(type_iri) || is_prov_top_level(type_iri) {
        2
    } else if type_iri.starts_with(v2::SBOL2_NS) {
        1
    } else {
        0
    }
}

fn is_known_sbol2_type(type_iri: &str) -> bool {
    matches!(
        type_iri,
        v2::SBOL2_COMPONENT_DEFINITION
            | v2::SBOL2_MODULE_DEFINITION
            | v2::SBOL2_COMPONENT
            | v2::SBOL2_MODULE
            | v2::SBOL2_FUNCTIONAL_COMPONENT
            | v2::SBOL2_SEQUENCE_ANNOTATION
            | v2::SBOL2_SEQUENCE_CONSTRAINT
            | v2::SBOL2_SEQUENCE
            | v2::SBOL2_MODEL
            | v2::SBOL2_INTERACTION
            | v2::SBOL2_PARTICIPATION
            | v2::SBOL2_COLLECTION
            | v2::SBOL2_IMPLEMENTATION
            | v2::SBOL2_ATTACHMENT
            | v2::SBOL2_EXPERIMENT
            | v2::SBOL2_EXPERIMENTAL_DATA
            | v2::SBOL2_COMBINATORIAL_DERIVATION
            | v2::SBOL2_VARIABLE_COMPONENT
            | v2::SBOL2_RANGE
            | v2::SBOL2_CUT
            | v2::SBOL2_GENERIC_LOCATION
            | v2::SBOL2_MAPS_TO
    )
}

use sbol_core::iri::last_iri_segment as last_path_segment;

/// Returns a child `(display_id, iri)` under `parent` whose IRI is not
/// already in `used`, inserting the chosen IRI. Disambiguates by appending
/// `_2`, `_3`, … and keeps displayId aligned with the child IRI.
fn next_available_child_iri(
    parent: &str,
    base_display_id: &str,
    used: &mut HashSet<String>,
) -> (String, String) {
    let mut counter: usize = 1;
    loop {
        let display_id = if counter == 1 {
            base_display_id.to_owned()
        } else {
            format!("{base_display_id}_{counter}")
        };
        let iri = format!("{parent}/{display_id}");
        if used.insert(iri.clone()) {
            return (display_id, iri);
        }
        counter += 1;
    }
}

/// Returns a `(display_id, iri)` for a Constraint synthesized under
/// `top_level`, using the next available `Constraint<N>` name (1-based). The
/// counter skips names already taken — including any real SequenceConstraint
/// on the same Component — matching the reference converter's per-Component
/// `createConstraint` numbering.
fn next_available_constraint_iri(top_level: &str, used: &mut HashSet<String>) -> (String, String) {
    let mut counter: usize = 1;
    loop {
        let display_id = format!("Constraint{counter}");
        let iri = format!("{top_level}/{display_id}");
        if used.insert(iri.clone()) {
            return (display_id, iri);
        }
        counter += 1;
    }
}

/// Canonical N-Triples line for a single triple, used by the snapshot
/// regenerator and the conformance test so both sides agree on formatting.
/// `xsd:string`-typed literals are emitted in bare form (matching
/// sbol-utilities and the N-Triples 1.1 convention) so plain-string
/// divergences don't masquerade as data drift. Embedded control
/// characters in literals are escaped per the N-Triples grammar
/// (`\n`, `\r`, `\t`, `\\`, `\"`) so each triple stays on a single line
/// for diff-friendly snapshot files.
pub fn canonical_nt_line(triple: &Triple) -> String {
    const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
    // `Resource` and `Term` are `#[non_exhaustive]`, so the match
    // requires a fall-through arm. A future variant should fail the
    // snapshot loudly rather than silently produce invalid N-Triples;
    // bumping the underlying RDF crate must come with explicit
    // handling here.
    let subject = match &triple.subject {
        Resource::Iri(iri) => format!("<{}>", iri.as_str()),
        Resource::BlankNode(b) => format!("_:{}", b.as_str()),
        other => unreachable!("unhandled Resource variant: {other:?}"),
    };
    let predicate = format!("<{}>", triple.predicate.as_str());
    let object = match &triple.object {
        Term::Resource(Resource::Iri(iri)) => format!("<{}>", iri.as_str()),
        Term::Resource(Resource::BlankNode(b)) => format!("_:{}", b.as_str()),
        Term::Literal(literal) => {
            let escaped = escape_nt_string(literal.value());
            if let Some(lang) = literal.language() {
                format!("\"{escaped}\"@{lang}")
            } else if literal.datatype().as_str() == XSD_STRING {
                format!("\"{escaped}\"")
            } else {
                format!("\"{escaped}\"^^<{}>", literal.datatype().as_str())
            }
        }
        other => unreachable!("unhandled Term variant: {other:?}"),
    };
    format!("{subject} {predicate} {object} .")
}

fn escape_nt_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // N-Triples 1.1 forbids unescaped control characters and
            // DEL in literals. Emit `\uXXXX` / `\UXXXXXXXX` for anything
            // in the C0/C1 control ranges and U+007F.
            c if (c as u32) < 0x20 || c as u32 == 0x7F => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}

fn url_origin(iri: &str) -> Option<String> {
    // URL form: `scheme://host/path` → `scheme://host`.
    if let Some((scheme, rest)) = iri.split_once("://") {
        let authority_end = rest.find('/').unwrap_or(rest.len());
        let host = &rest[..authority_end];
        if !scheme.is_empty() && !host.is_empty() {
            return Some(format!("{scheme}://{host}"));
        }
    }

    // URN form (RFC 8141): `urn:nid:nss` → `urn:nid`. Treat the namespace
    // identifier as the origin so URN-style IRIs get a stable
    // `hasNamespace` even when persistentIdentity+displayId derivation
    // fails. Tolerate either `:` or `/` between nid and nss.
    if let Some(after_urn) = iri.strip_prefix("urn:")
        && !after_urn.is_empty()
    {
        let nid_end = after_urn.find([':', '/']).unwrap_or(after_urn.len());
        let nid = &after_urn[..nid_end];
        if !nid.is_empty() {
            return Some(format!("urn:{nid}"));
        }
    }
    None
}
