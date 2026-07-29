# Validation Output JSON v1

`sbol validate --format json` and `sbol::to_json(&report)` emit a versioned
JSON document describing the result of a validation run. The schema is
stable from version 1 onward: breaking field renames or removals bump the
top-level `schema_version` integer; additive fields (new optional field,
new enum variant in a `properties` map) do not.

Hand-written by `crates/sbol3/src/validation/output.rs` to keep the core
crate free of `serde_json` in its public dependency tree. Round-tripped
through `serde_json` under `[dev-dependencies]` in
`crates/sbol3/tests/validation_output.rs`.

## Top-level shape

```jsonc
{
  "$schema": "https://sbolstandard.org/sbol-rs/validation-report/v1.json",
  "schema_version": 1,
  "spec_version": "3.1.0",
  "applied_options": { ... },
  "coverage":        { ... },
  "issues":          [ ... ]
}
```

## `applied_options`

A summary of the configuration that produced this run. Round-trips through
the schema so downstream readers know what coverage they're looking at.

```jsonc
{
  "topology_completeness": "Conservative" | "RequireKnownForNucleicAcids",
  "external_mode":         "Off" | "ProvidedOnly" | "ExternalAllowed",
  "document_resolvers":    0,
  "content_resolvers":     0,
  "severity_floor":        "Warning" | "Error" | null,
  "severity_ceiling":      "Warning" | "Error" | null,
  "overridden_rules":      [
    { "rule": "sbol3-12807", "override": "Suppress" },
    { "rule": "sbol3-10502", "override": { "Severity": "Error" } }
  ]
}
```

## `coverage`

Per-rule outcome partitioned into three buckets. Every catalog rule
appears in exactly one bucket.

```jsonc
{
  "fully_applied":     [ "sbol3-10102", "sbol3-10103", ... ],
  "partially_applied": [
    {
      "rule":          "sbol3-10502",
      "blocker":       "Ontology",
      "coverage_kind": "OntologyKnownTermsOnly"
    }
  ],
  "not_applied": [
    { "rule": "sbol3-12303", "reason": "MachineUncheckable" },
    { "rule": "sbol3-12502", "reason": { "Deferred": "Resolver" } }
  ]
}
```

### `blocker` (closed set)

`Ontology`, `Resolver`, `StrictDatatype`, `Policy`, `External`.

### `coverage_kind` (closed set)

| Kind | Meaning |
|---|---|
| `OntologyKnownTermsOnly` | Local check covered every term in the bundled ontology snapshot; terms outside the snapshot remain undecided by design. |
| `LocalReferencesOnly` | Reference integrity covered document-local targets; external references would require a resolver. |
| `LexicalShapeOnly` | Lexical/datatype shape checked; remote content was not fetched. |
| `PolicyDefaultUndecided` | Topology completeness was opt-in; default mode left the rule undecided for cases where the local subset has no signal. |

### `reason` for `not_applied`

- `"MachineUncheckable"`: the spec marks the rule ▲; violations are not
  to be machine-reported.
- `{ "Deferred": <blocker> }`: catalog status is `Deferred`; the rule
  needs the named blocker resolved before any local algorithm can run.

## `issues`

```jsonc
{
  "severity": "Error" | "Warning",
  "rule":     "sbol3-10110",
  "subject":  "https://example.org/component",
  "property": "http://sbols.org/v3#displayId" | null,
  "message":  "displayId: cardinality 0..1 violated; saw 2 values",
  "hint":     null | <Hint>
}
```

### `hint`

When present, one of:

- `{ "SuggestedTerm": { "table": "Table 2", "iri": "...", "label": "..." } }`
- `{ "UrlPattern": { "expected": "[namespace]/[local]/[displayId]" } }`
- `{ "PreferredAlias": { "canonical": "..." } }`
- `{ "Note": "<short text>" }`

## Exit codes (CLI)

`sbol validate` maps the result onto these exit codes:

| Code | Meaning |
|---|---|
| 0 | No errors. |
| 1 | At least one Error issue. |
| 2 | I/O, parse, or option-validation failure. |
| 3 | `--treat-partial-as-errors` was set and the report has any partially-applied rule. |
| 4 | (Reserved) Baseline regression detected by `--baseline`. |

## SARIF v2.1.0 emitter

Use `--format sarif` to emit `runs[].results[]`-shaped output for GitHub
code-scanning and other SARIF consumers. SBOL coverage metadata round-trips through
`runs[].invocations[0].properties.coverage`. SARIF consumers that
recognize the property key surface it; consumers that don't ignore it per
the SARIF spec.

Mapping:

| SBOL field | SARIF field |
|---|---|
| `rule` | `runs[].results[].ruleId` |
| `Severity::Error` | `results[].level = "error"` |
| `Severity::Warning` | `results[].level = "warning"` |
| `subject` | `results[].locations[0].logicalLocations[0].fullyQualifiedName` |
| `property` | `results[].locations[0].logicalLocations[0].decoratedName` |
| input path | `results[].locations[0].physicalLocation.artifactLocation.uri` |
| `message` | `results[].message.text` |
| `spec_section` | `runs[].tool.driver.rules[].helpUri` |
| `coverage` | `runs[].invocations[0].properties.coverage` |
