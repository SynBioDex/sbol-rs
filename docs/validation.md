# Validating SBOL documents

The sbol-rs validators answer a specific question: **does this SBOL
document conform to the spec?** There is one validator for SBOL 3
(`sbol3`) and one for SBOL 2 (`sbol2`); they share the same reporting
types, the same `ValidationConfig`, and the same rule-classification
taxonomy from `sbol-core`. The honest answer has more shape than yes/no,
and this document explains what shape that takes for both versions.

## What the validator does

Every spec rule is in a per-version catalog (`crates/sbol3/rules.toml`,
149 rules; `crates/sbol2/rules.toml`, 268 rules) with a status, severity,
gate, and (where applicable) a blocker. When you call `Document::check()`
(SBOL 3) or `Document::validate()` (SBOL 2), the validator runs every
algorithm it has against your document and returns a `ValidationReport`
carrying three things:

1. **Issues**: actionable diagnostics for things that are demonstrably
   wrong (`Error`) or recommended-but-not-required violations
   (`Warning`). Each issue has a stable rule ID, subject IRI, optional
   property name, message, and optional hint.
2. **Coverage**: per-rule signal saying whether each spec rule was
   fully evaluated, partially evaluated (with a `coverage_kind`
   explaining the scope limit), or not evaluated (with a reason).
3. **Applied options**: what configuration produced this report, so
   the result is reproducible.

This is intentionally more than a boolean. The validator's job is
twofold: surface what's wrong AND tell the caller what was checked.

## What "validation passed" means

The validator is honest about its scope. SBOL 3.1.0 has 149 validation
rules, but they are not all the same shape. Appendix B (p.2837–2840)
marks 40 rules with the ▲ symbol: weak-REQUIRED conditions that the
spec itself says are NOT to be machine-reported. Of the remaining 109
machine-checkable rules, **all 109 are fully implemented (100%)**. The
SBOL 2.3.0 catalog is larger (268 rules, 29 of them ▲), and of its 239
machine-checkable rules **all 239 are fully implemented**. The
[SBOL 3 conformance report](conformance.md) and the
[SBOL 2 conformance report](sbol2-conformance.md) carry the per-rule
details, and the freshness of both is enforced in CI.

The validator surfaces this distinction at runtime via the coverage
signal:

- `fully_applied`: every spec case for this rule is decidable with the
  current configuration and was evaluated against this document.
- `partially_applied`: the local subset ran but the rule's full
  coverage is bounded by configuration (no resolver) or by the bundled
  ontology snapshot (out-of-snapshot terms). Each entry carries a
  `coverage_kind` naming the scope.
- `not_applied`: the rule is `Unimplemented` (no algorithm yet) or
  `MachineUncheckable` (spec ▲ guidance).

If you want "validation passed = spec-compliant" with no footnotes, use
[`Document::check_complete()`](#strict-ci-gates) (or `sbol validate
--treat-partial-as-errors`). This returns `Err` whenever
`partially_applied` is non-empty, so any partial coverage trips the
gate.

## The rule taxonomy

Every rule classifies into exactly one of five statuses. The first
three are implementations; the last two record what's missing.

| Status | Count | What it means |
|---|---|---|
| `Error` | 55 | Algorithm complete; MUST violations emit as `Severity::Error`. |
| `Warning` | 17 | Algorithm complete; SHOULD violations emit as `Severity::Warning`. |
| `Configurable` | 37 | Algorithm complete; behavior varies with configuration. The `blocker` field names which axis: `Resolver`, `Ontology` (snapshot-bounded), `Policy` (Conservative/Strict/Lenient modes), or `External` (local-only mode; full spec scope unreachable per-document). |
| `MachineUncheckable` | 40 | Spec ▲: violations are not to be machine-reported. May have a local subset that emits warnings on positively-decidable cases. |
| `Unimplemented` | 0 | No local algorithm yet. The `blocker` field names what's needed (Ontology data, Resolver protocol, Policy decision). |

The counts above are the SBOL 3 catalog; SBOL 2 uses the same five
statuses (Error 166, Warning 30, Configurable 43, ▲ MachineUncheckable
29). The full per-rule grids live in [conformance.md](conformance.md) and
[sbol2-conformance.md](sbol2-conformance.md). They are generated from the
catalogs; regenerate after any change to a `validation_rule_statuses()`
with:

```
cargo run -p sbol3 --bin generate-conformance-report
cargo run -p sbol2 --bin generate-sbol2-conformance-report
```

## Shared configuration: validation gates

Both validators read the same `ValidationConfig` (from `sbol-core`),
which selects **which rule families run at all**, orthogonal to per-rule
overrides and policy modes, which control severity *within* the running
families. Its defaults match libSBOLj's `SBOLValidate` command-line
defaults:

| Flag | Default | Effect |
|---|---|---|
| `compliant` | on | Run the compliant-URI structural family. |
| `complete` | on | Run the completeness family: every referenced object must be present. |
| `best_practice` | off | Run the SHOULD-level best-practice family. |
| `types_in_uri` | off | Interpret compliant URIs as carrying an optional type segment. |
| `keep_going` | on | Continue past the first error and collect every issue. |

`ValidationConfig::all_on()` enables every family;
`ValidationConfig::default()` runs `compliant` + `complete`.

The SBOL 2 catalog binds each rule to a **gate** naming the family that
runs it (`Always`, `Compliant`, `Complete`, or `BestPractice`), so the
flag model is directly observable per rule. This is what the SBOLTestSuite
SBOL 2 corpora exercise: the valid `SBOL2` corpus passes under both the
default and `all_on`; `SBOL2_ic` fails only when `complete` is on;
`SBOL2_nc` fails only when `compliant` is on; and `SBOL2_bp` is flagged
(at warning severity) only when `best_practice` is on. The gate counts and
this behavior are tabulated in
[sbol2-conformance.md](sbol2-conformance.md).

## Using the validator

### Library API

```rust
use sbol::prelude::*;

let document = Document::read_path("design.ttl")?;

// Run everything; inspect the full report.
let report = document.validate();
for issue in report.issues() { ... }
for partial in &report.coverage().partially_applied { ... }

// Err on any Error-severity issue.
document.check()?;

// Err if any rule's coverage is partial (strict CI gate).
document.check_complete()?;
```

The SBOL 2 surface mirrors this through `sbol2` (or `sbol::v2`):
`Document::validate()` returns the same `ValidationReport`, and
`Document::validate_with_config(&config)` takes the shared
`ValidationConfig` to select gates:

```rust
use sbol2::validation::ValidationConfig;

let document = sbol2::Document::read_path("design.xml")?;
let report = document.validate_with_config(&ValidationConfig::all_on());
```

The full options surface (per-rule overrides, severity floor/ceiling,
policy modes, resolver context) is on `ValidationOptions` and
`ValidationContext`. Use `Document::check_with(options)` or
`Document::check_with_context(context)` to pass them:

```rust
let options = ValidationOptions::default()
    .deny("sbol3-12807")?                       // promote to error
    .allow("sbol3-10502")?                       // suppress
    .with_severity_floor(Severity::Error);       // warnings → errors

let context = ValidationContext::with_options(options)
    .with_external_mode(ExternalValidationMode::ProvidedOnly)
    .with_document_resolver(&resolver);

document.check_with_context(context)?;
```

### CLI

```bash
# Basic
sbol validate design.ttl

# JSON output for CI consumption
sbol validate design.ttl --format json --treat-partial-as-errors

# SARIF for GitHub code scanning
sbol validate design.ttl --format sarif --output report.sarif

# Per-rule overrides
sbol validate design.ttl --allow sbol3-10502 --deny sbol3-12807

# Resolver mode for cross-document reference checks
sbol validate design.ttl --external-mode provided --resolve-documents ./bundle
```

Run `sbol validate --help` for the full flag surface.

### Strict CI gates

For a CI gate that fails on any spec gap, use `check_complete` /
`--treat-partial-as-errors`:

```yaml
# .github/workflows/validate.yml
- run: |
    sbol validate design.ttl \
      --format sarif \
      --treat-partial-as-errors \
      --output validation.sarif
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: validation.sarif
```

Exit code 0 means "every rule was fully evaluated and zero errors."
Anything else trips the gate; see [validation-output.md](validation-output.md#exit-codes-cli)
for the full code table.

## Output formats

The validator emits three formats, all from the same
`ValidationReport`:

- **text** (default): one line per issue plus a one-line summary, plus
  an optional coverage summary with `--show-coverage`.
- **JSON v1**: versioned schema documented in
  [validation-output.md](validation-output.md). The serializer is
  hand-written so the core crate stays free of `serde_json` in its
  public dependency tree; the round-trip test under
  `crates/sbol3/tests/validation_output.rs` uses serde_json as a
  test-only dep to verify every emitted field parses.
- **SARIF v2.1.0**: for GitHub code scanning and editor extensions.
  Built into `sbol-cli`. SBOL coverage metadata is carried in
  `runs[].invocations[0].properties.coverage` so SARIF consumers that
  recognize it surface it; consumers that don't ignore it per the SARIF
  spec.

All three formats carry the same data: issues, coverage, applied
options.

## Configuration: tuning scope and severity

Three knobs let callers tune validation without forking the catalog:

1. **Per-rule overrides**: `ValidationOptions::allow(rule)` suppresses
   the rule, `deny(rule)` promotes its emit to Error, `warn(rule)`
   demotes to Warning. CLI flags: `--allow`, `--deny`, `--warn`.
2. **Severity floor / ceiling**: promote all warnings to errors with
   `with_severity_floor(Severity::Error)` (or `--severity-floor error`),
   or demote errors to warnings with a ceiling.
3. **Policy modes**: `PolicyOptions` carries `Conservative`/`Strict`/
   `Lenient` knobs for rules where the spec is ambiguous. Each knob has
   a committed ADR under [policies/](policies/). The default is always
   `Conservative` and matches current emit behavior; opt-in modes are
   explicit.

## Trust boundaries

The validator does not check what cannot be checked from a single
document. Three explicit boundaries:

1. **Global IRI uniqueness** (sbol3-10101): no per-document validator
   can prove an IRI is globally unique. The validator does
   document-local uniqueness; the rule is classified `Configurable`
   with the `External` blocker so its scope limit is visible in
   coverage rather than hidden behind a green light.
2. **▲ rules** (40 of them): the spec says "violations are not to be
   machine-reported." The validator runs a local subset where one is
   possible and emits warnings on positively-decidable cases (never
   errors). The broader spec rule is always recorded in coverage as
   `MachineUncheckable`.
3. **Out-of-snapshot ontology terms**: the bundled ontology snapshot
   (`crates/sbol-ontology`, ~12,000 terms across SBO, SO, EDAM, GO,
   ChEBI) covers terms from the spec's Tables 1–17 plus their relevant
   subtrees. Custom or out-of-snapshot terms remain undecided by
   design; the spec doesn't define what they mean and the validator
   doesn't guess.

## Extending the validator

Adding a new rule (or refining an existing one) is a three-step change,
shown here for SBOL 3; SBOL 2 mirrors it under `crates/sbol2/`:

1. **Catalog**: add or update the entry in `crates/sbol3/rules.toml`.
   The build script gates status/blocker/severity invariants and (for
   Policy rules) requires a committed ADR under `docs/policies/`.
2. **Algorithm**: implement the check in
   `crates/sbol3/src/validation/rules/<section>.rs`, dispatched from
   `Validator::validate` in `crates/sbol3/src/validation/validator.rs`.
3. **Tests**: add at least one regression case under
   `crates/sbol3/tests/rule_cases/<section>.rs`. The meta-test
   `implemented_validation_rules_have_regression_cases` fails the
   build if a rule with an algorithm lacks one.

Regenerate the conformance grid (`cargo run -p sbol3 --bin
generate-conformance-report`, or the `sbol2` peer); CI's
`git diff --exit-code` enforces freshness.

See [testing.md](testing.md) for the full test architecture.

## Where to read next

| Topic | Doc |
|---|---|
| SBOL 3 per-rule status grid (generated) | [conformance.md](conformance.md) |
| SBOL 2 per-rule status grid (generated) | [sbol2-conformance.md](sbol2-conformance.md) |
| JSON v1 output schema | [validation-output.md](validation-output.md) |
| Policy ADRs (one per ambiguous rule) | [policies/](policies/) |
| Test architecture | [testing.md](testing.md) |
| RDF I/O | [rdf-io.md](rdf-io.md) |
