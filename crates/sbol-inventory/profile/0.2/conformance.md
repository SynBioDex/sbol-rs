# SBOLInventory 0.2 conformance

## Claims

An implementation may make one or more conformance claims:

| Claim | Required behavior |
|----|----|
| Reader | Parse every valid fixture and expose all profile triples without changing their RDF meaning. |
| Writer | Serialize a graph that is isomorphic to the input and preserve unknown extension triples. |
| Validator | Apply SBOL core validation plus every applicable REQUIRED profile rule and report stable rule IDs. |
| Query | Implement the candidate eligibility and deterministic ordering defined by the specification. |
| Full | Satisfy Reader, Writer, Validator, and Query. |

A claim MUST state:

- SBOLInventory profile version;
- supported SBOL core version;
- implementation name and version;
- claimed classes;
- rule coverage and any machine-uncheckable rules;
- fixture-suite revision.

Passing the SHACL shapes is not a Validator claim. The shapes are a portable structural subset of the full rule catalog.

The checked-in pySBOL3 conformance harness applies SHACL to the raw RDF graph, loads valid object structure through pySBOL3, runs SBOL core validation, and then runs the procedural profile validator. This order matters because an object library can reject or normalize malformed cardinalities and orphaned owned objects before its typed API exposes them.

## Rule catalog

`rules.toml` is the authoritative checklist. Rule IDs are stable once a profile version is published. Correcting a rule's implementation does not change its ID. A semantically incompatible rule change requires a new profile version and, when necessary, a new rule ID.

Each rule declares:

- conformance classes to which it applies;
- REQUIRED or RECOMMENDED strength;
- whether it is machine-checkable;
- whether SHACL Core covers it;
- its specification section;
- a concise normative statement.

## Fixture protocol

`fixtures/manifest.toml` lists every fixture and its expected outcome. Valid fixtures MUST:

1. parse as RDF;
2. pass the declared SBOL core validator;
3. pass SHACL Core shapes;
4. pass the profile Validator;
5. survive a Reader/Writer round trip as an isomorphic RDF graph.

Each invalid fixture contains one intentional profile defect and names the expected `sbolinv-*` rule. It MAY trigger additional derivative failures, but a Validator MUST report the expected rule before claiming conformance for that fixture.

SHACL coverage is recorded separately in the manifest. An invalid fixture for an algorithmic rule can pass SHACL and still be correctly rejected by a Full Validator.

Fixtures use stable example IRIs and MUST NOT depend on network dereferencing. Implementations MUST evaluate each fixture as a separate RDF document.

## Cross-implementation use

The fixture directory is intentionally independent of Python. A pySBOL3 implementation and an `sbol-rs` implementation should consume the same files and compare rule IDs rather than implementation-specific error text.

RDF isomorphism, not textual equality, is the round-trip criterion. Prefixes, blank-node labels, triple order, and equivalent RDF serializations are not observable differences.
