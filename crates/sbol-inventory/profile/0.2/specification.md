# SBOLInventory Profile 0.2

## Status

This document is the draft normative specification for SBOLInventory Profile 0.2. The profile IRI is:

```text
https://draggon.org/spec/sbol-inventory/0.2
```

Package versions and profile versions are independent. The Python package version `0.2.x` implements this profile draft, but a package version is not an RDF vocabulary namespace.

## Normative language and authority

The key words MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY are to be interpreted as described by RFC 2119 and RFC 8174 when they appear in uppercase.

The normative artifacts are:

1. this document for model and semantic definitions;
2. `vocabulary.ttl` for term identities;
3. `rules.toml` for numbered validation requirements.

`shapes.ttl` is a machine-readable SHACL Core projection of the rules that can be expressed portably in SHACL Core. It is tested for consistency with the normative artifacts, but passing SHACL alone is not Full Validator conformance. The fixtures are executable conformance evidence and MUST agree with the rule catalog.

If two normative artifacts disagree, implementations MUST report the conflict as a specification defect rather than silently selecting whichever behavior is convenient.

## Scope

SBOLInventory adds a facility catalog and run-ledger profile to SBOL 3:

```text
a facility contains zones
zones locate assets and material lots
assets expose capabilities
workflows require capabilities
plans bind requirements to qualified assets
runs record material changes and evidence
```

The profile directly specifies facilities, zones, assets, capability offerings, material lots, and run provenance. It does not specify a workflow language, constraint solver, scheduler, booking system, device protocol, authorization system, or complete LIMS.

An SBOLInventory document MUST be valid SBOL 3 under the SBOL version claimed by its implementation. Profile 0.2 uses only constructs common to SBOL 3.0.1 and 3.1.0. A conformance report MUST state the SBOL core version and validator used.

## Namespaces and versioning

Profile 0.2 uses three stable term namespaces:

| Prefix | Namespace | Purpose |
|----|----|----|
| `inv:` | `https://draggon.org/ns/inventory#` | Reusable material and legacy inventory kinds |
| `fac:` | `https://draggon.org/ns/facility#` | Facility classes, properties, states, and run roles |
| `cap:` | `https://draggon.org/ns/capability#` | Reusable operation-oriented capability kinds |

Term IRIs do not contain the profile version. Compatible clarification or constraint strengthening does not require new term IRIs. An incompatible change to a term's meaning MUST mint a new term or a new namespace. Profile releases are versioned by their profile IRI and specification artifacts.

Profile 0.2 does not require a conformance marker inside each data graph. A system MAY annotate a catalog root with `dcterms:conformsTo` and the profile IRI. The absence of that annotation does not remove profile obligations from objects that use profile terms.

## Profile scope in a mixed SBOL document

The following nodes are in profile scope:

- nodes typed `fac:Facility`, `fac:Zone`, or `fac:Asset`;
- nodes typed `fac:CapabilityOffering` or `fac:PropertyValue` and owned through profile properties;
- `sbol:Implementation` nodes with a `fac:materialKind` property;
- `prov:Usage` nodes carrying `fac:RunAsset` or `fac:RunInputMaterial` as a `prov:hadRole` value;
- `prov:Activity` nodes that own at least one such usage.

An ordinary `sbol:Implementation` with no `fac:materialKind` is not a `MaterialLot` and MUST NOT be subjected to material-lot rules.

## Common requirements

All profile classes and properties MUST use the IRIs defined in `vocabulary.ttl`. A value described as an IRI MUST be an absolute IRI. It need not be an HTTP URL or resolve over the network unless a rule explicitly requires document-local resolution.

A conformant document is closed over each required structural reference:

- `fac:facility`;
- `fac:parentZone`;
- `fac:locatedIn`;
- `fac:partOf`;
- `fac:establishesZone`;
- `sbol:built` on a material lot;
- profile run-usage entities;
- `fac:derivedFromMaterial` on a material lot.

Those references MUST resolve to objects of the required type in the same RDF document. Open vocabulary values such as kinds, policies, parameter kinds, units, and control-independent ontology terms need not resolve locally.

The Python construction API MAY temporarily hold an incomplete object while a caller assembles a document. Constructor cardinalities are therefore not the profile cardinalities. Only a document that satisfies the cardinalities below is conformant.

## Facility

`fac:Facility` is a custom SBOL `TopLevel` representing one governed laboratory site or administrative facility boundary. It has no REQUIRED profile-specific properties. A document MAY contain multiple facilities.

The profile does not infer facility membership from identity prefixes or spatial containment. Every zone, asset, and material lot states its governing facility explicitly.

## Zone

`fac:Zone` is a custom SBOL `TopLevel` representing a spatial, environmental, containment, storage, or policy boundary.

| Property | Cardinality | Value | Semantics |
|----|---:|----|----|
| `fac:facility` | 1 | local `fac:Facility` | Governing facility |
| `fac:zoneKind` | 1 | IRI | Open classification vocabulary |
| `fac:parentZone` | 0..1 | local `fac:Zone` | Direct enclosing zone |
| `fac:policy` | 0..* | IRI | Externally defined policy applying in the zone |
| `fac:condition` | 0..* | owned `fac:PropertyValue` | Descriptive environmental condition |
| `fac:isActive` | 1 | `xsd:boolean` | Whether the zone is currently available |

A parent and child zone MUST belong to the same facility. The `parentZone` graph MUST be acyclic. A condition is descriptive catalog state, not an executable setpoint or guarantee.

No two conditions owned by one zone MAY have the same `fac:propertyKind`. Applications that need distinct minimum, maximum, nominal, or observed values MUST use distinct property-kind IRIs that encode those meanings.

## Asset

`fac:Asset` is a custom SBOL `TopLevel` representing a physical resource, instrument, container, storage unit, workstation, or independently bindable functional unit.

| Property | Cardinality | Value | Semantics |
|----|---:|----|----|
| `fac:facility` | 1 | local `fac:Facility` | Governing facility |
| `fac:assetKind` | 1 | IRI | Open classification vocabulary |
| `fac:locatedIn` | 0..1 | local `fac:Zone` or `fac:Asset` | Direct location or custody container |
| `fac:position` | 0..1 | `xsd:string` | Named position inside an asset |
| `fac:partOf` | 0..1 | local `fac:Asset` | Direct physical or functional parent |
| `fac:establishesZone` | 0..* | local `fac:Zone` | Controlled zone created by the asset |
| `fac:manufacturer` | 0..1 | `xsd:string` | Manufacturer label |
| `fac:model` | 0..1 | `xsd:string` | Model label |
| `fac:serialNumber` | 0..1 | `xsd:string` | Installation-specific serial label |
| `fac:isActive` | 1 | `xsd:boolean` | Whether the asset is currently available |
| `fac:allowedPosition` | 0..* | `xsd:string` | Finite set of valid contained positions |
| `fac:capability` | 0..* | owned `fac:CapabilityOffering` | Operations offered by this installation |

`locatedIn` and `partOf` are independent. `locatedIn` states physical location or custody. `partOf` states composition. A thermocycler block can be part of a parent instrument while the instrument is located in a room.

All `partOf`, asset-valued `locatedIn`, and mixed `partOf` plus asset-valued `locatedIn` paths MUST be acyclic. References on either edge MUST remain within one facility. An established zone MUST belong to the same facility as the asset that establishes it.

An asset's `allowedPosition` values MUST contain at least one non-whitespace character and be unique. A `position` value, when present, MUST also contain at least one non-whitespace character. If a located asset or material lot names an asset with at least one allowed position, it MUST provide exactly one `fac:position`, and that value MUST occur in the container's allowed-position set. A position MUST NOT be supplied without a location or for a zone-valued location.

At most one asset or material lot may occupy a given `(container, position)` pair. Inactive objects still occupy their recorded position. `isActive=false` means unavailable; it does not mean absent. Vacating a position requires an explicit custody update that removes or changes `locatedIn` and `position`.

One asset MUST NOT own two offerings with the same `fac:capabilityKind`. Independently selectable units MUST be represented as child assets. Profile 0.2 does not define a generic shared-capacity-group property. Capacity constraints that are not independently bindable MAY be represented as typed capability parameters and interpreted by a planner.

## Capability offering

`fac:CapabilityOffering` is an SBOL `Identified` child owned by exactly one asset through `fac:capability`. It describes an operation offered by that particular installed asset, not an intrinsic guarantee about its product model.

| Property | Cardinality | Value | Semantics |
|----|---:|----|----|
| `fac:capabilityKind` | 1 | IRI | Open operation-oriented vocabulary |
| `fac:qualification` | 1 | qualification IRI | Closed maturity vocabulary below |
| `fac:controlMode` | 1 | control-mode IRI | Closed invocation vocabulary below |
| `fac:isActive` | 1 | `xsd:boolean` | Whether the offering is currently available |
| `fac:parameter` | 0..* | owned `fac:PropertyValue` | Typed operational fact or constraint |

No two parameters owned by one offering MAY have the same `fac:propertyKind`. Manufacturer and model labels belong on the asset, not on the capability kind.

## Property value

`fac:PropertyValue` is an SBOL `Identified` child owned by exactly one zone or capability offering through `fac:condition` or `fac:parameter`.

| Property           | Cardinality | Value         |
|--------------------|------------:|---------------|
| `fac:propertyKind` |           1 | IRI           |
| `fac:textValue`    |        0..1 | `xsd:string`  |
| `fac:integerValue` |        0..1 | `xsd:integer` |
| `fac:realValue`    |        0..1 | `xsd:double`  |
| `fac:booleanValue` |        0..1 | `xsd:boolean` |
| `fac:uriValue`     |        0..1 | IRI           |
| `fac:unit`         |        0..1 | IRI           |

Exactly one of the five value properties MUST be present. `fac:unit` MAY be present only with `integerValue` or `realValue`. A unit is metadata about a numeric value and MUST NOT be treated as a conversion instruction. Unit conversion and dimensional compatibility are planner responsibilities.

## Material lot

A material lot is a standard `sbol:Implementation` carrying `fac:materialKind`. It is not a separate RDF class. This preserves ordinary SBOL interoperability while making profile scope explicit.

| Property | Cardinality | Value | Semantics |
|----|---:|----|----|
| `sbol:built` | 1 | local `sbol:Component` | Design realized by the physical material |
| `fac:materialKind` | 1 | IRI | Open material classification vocabulary |
| `fac:facility` | 1 | local `fac:Facility` | Governing facility |
| `fac:locatedIn` | 0..1 | local `fac:Zone` or `fac:Asset` | Direct location or container |
| `fac:position` | 0..1 | `xsd:string` | Position inside an asset |
| `fac:isActive` | 1 | `xsd:boolean` | Whether the lot is available for use |
| `fac:barcode` | 0..1 | `xsd:string` | Locally assigned barcode text |
| `fac:lotId` | 0..1 | `xsd:string` | Supplier or local lot label |
| `fac:notes` | 0..1 | `xsd:string` | Human-readable note |
| `fac:freezeDate` | 0..1 | `xsd:dateTime` | Recorded freezing timestamp |
| `fac:derivedFromMaterial` | 0..* | local material lot | Material lineage |

`barcode` and `lotId` are opaque labels. Profile 0.2 does not assign them global uniqueness semantics. A facility MAY impose stronger uniqueness policy.

Material lineage MUST NOT contain self-references or cycles. A transformed material receives a new `sbol:Implementation` identity; an implementation MUST NOT overwrite an input lot to represent its output.

The asset location and occupancy rules also apply to material lots.

## Qualification

Qualification is a closed total order on one installed capability offering:

```text
Discovered < Described < Plannable < Simulatable < Executable < Qualified
```

| Value | Meaning |
|----|----|
| `fac:Discovered` | The offering is known to exist. |
| `fac:Described` | Relevant catalog facts have been represented and reviewed. |
| `fac:Plannable` | A consumer has a checked parameter and capacity model. |
| `fac:Simulatable` | Planned behavior can be exercised without operating hardware. |
| `fac:Executable` | A reviewed execution path can invoke or direct the installed asset. |
| `fac:Qualified` | The end-to-end path has been accepted for a stated laboratory use. |

An offering at a higher level satisfies a query for any lower minimum level. This ordering is only candidate eligibility. It does not prove parameter compatibility, scheduling feasibility, safety, authorization, calibration, or fitness for a particular experiment.

An implementation MUST NOT infer qualification from manufacturer, model, control mode, public product literature, or the qualification of another installation.

## Control mode

Control mode is closed and independent of qualification:

| Value | Meaning |
|----|----|
| `fac:UnspecifiedControl` | The invocation path is unknown or intentionally unstated. |
| `fac:ManualControl` | An operator performs the operation manually. |
| `fac:ReviewedFileControl` | A reviewed file is handed to an instrument system. |
| `fac:VendorSessionControl` | Execution occurs through a vendor application or session. |
| `fac:ApiControl` | Execution uses an application programming interface. |
| `fac:SiLA2Control` | Execution uses a SiLA 2 interface. |
| `fac:OpcUaControl` | Execution uses an OPC UA interface. |

A control mode records the known interaction channel. It does not by itself authorize execution or establish any qualification level.

## Run provenance

Profile 0.2 uses standard SBOL 3 and PROV-O objects for run records. It does not define a custom run class.

A profile run is a `prov:Activity` that owns at least one `prov:Usage` carrying one of the profile roles below:

- `fac:RunAsset`: `prov:entity` MUST resolve locally to `fac:Asset`.
- `fac:RunInputMaterial`: `prov:entity` MUST resolve locally to a material lot.

A profile run MUST contain at least one `RunAsset` usage. Manual work can name a workstation or other facility asset when no instrument is involved.

Generated material lots and `sbol:ExperimentalData` SHOULD refer to the run through `prov:wasGeneratedBy`. Material transformations SHOULD state their input lots with `fac:derivedFromMaterial`. Evidence files SHOULD use standard `sbol:Attachment` objects.

A reviewed plan and responsible agent MAY be represented with standard `prov:Association`, `prov:hadPlan`, and `prov:agent`. Those standard objects do not encode a compiler's private requirement or allocation IR.

## Candidate-query behavior

The optional Query conformance class provides a deterministic operation that takes a conformant document, a capability-kind IRI, a minimum qualification, and an optional facility identity. It MUST reject a capability-kind argument that is not an absolute IRI and a minimum qualification that is not in the closed qualification vocabulary.

An offering is eligible only when:

1. its asset and the offering have `fac:isActive=true`;
2. every asset reached through `partOf` or asset-valued `locatedIn` is active;
3. every zone reached through `locatedIn` and `parentZone` is active;
4. its capability kind exactly equals the requested IRI;
5. its qualification rank is at least the requested minimum; and
6. its asset belongs to the requested facility when a facility filter is used.

Results MUST be ordered lexicographically by asset IRI. Because an asset cannot own duplicate capability kinds, this order is total for one query.

Candidate discovery is not allocation. A Query implementation MUST NOT claim that an eligible result is a complete feasible or executable plan.

## Extension policy

The profile is open to non-SBOL extension properties and external terms. A Reader/Writer implementation MUST preserve unknown RDF triples attached to identified objects across a read-write round trip unless the caller explicitly requests a lossy projection.

Zone kinds, asset kinds, material kinds, capability kinds, policy IRIs, property kinds, units, and URI-valued properties are open vocabularies. Qualification and control mode are closed in Profile 0.2 because an unknown value in either position changes eligibility or execution meaning.

Unknown non-SBOL properties MUST NOT make an otherwise conformant graph invalid. Implementations MUST NOT mint new terms in the SBOL namespace.

## Validation

A Full Validator performs both layers:

1. SBOL core validation under its declared SBOL version.
2. Every machine-checkable REQUIRED rule in `rules.toml` that applies to the Validator conformance class.

Every reported profile violation MUST include its stable `sbolinv-*` rule ID. A validator MAY stop after the first violation or return a complete report, but it MUST produce the same pass/fail result for the conformance fixtures.

SHACL validation is a useful structural layer, but SHACL conformance alone is not Full Validator conformance. Cross-object equality, graph cycles, occupancy, effective activity, and provenance-role semantics remain explicit numbered rules even when an implementation evaluates them procedurally.

## RDF comparison and serialization

Conformance is defined over RDF graph meaning, not Turtle prefixes, triple order, whitespace, or byte-for-byte serialization. A read-write round trip MUST produce an RDF graph isomorphic to the input for recognized and preserved extension data.

Turtle is the review-oriented serialization used by the fixtures. A conformant implementation MAY additionally support RDF/XML, JSON-LD, N-Triples, or other SBOL-compatible RDF serializations.

## Privacy and operational overlays

The profile can represent serial numbers, custody, and facility structure, but does not define access control. Publishers SHOULD omit or redact addresses, network details, booking state, access policy, maintenance records, calibration evidence, and other sensitive operational data from public catalogs.

Qualification claims SHOULD identify their governing procedure and evidence in an access-appropriate system. The profile level itself is not a substitute for authorization, biosafety review, or instrument-specific safety controls.
