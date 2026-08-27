# sbol-rulegen

Build-time code generator shared by the `sbol2` and `sbol3` crates. It turns a
version's `rules.toml` validation catalog into Rust source consumed at compile
time.

Each SBOL version crate calls [`generate`] from its `build.rs`, pointing at its
own `rules.toml`. Extension-profile crates call [`generate_profile`] for the
separate conformance-class schema. The generator parses the catalog, checks each entry's status,
normative severity, blocker, and coverage-kind against the shared taxonomy in
[`sbol_core::validation`], enforces that policy-blocked rules have an ADR file,
and emits two files into `OUT_DIR`:

- `rule_catalog.rs`: a `VALIDATION_RULE_STATUSES` slice literal, sorted by rule
  id for diff stability.
- `rule_spec_meta.rs`: the `VALIDATION_RULE_SPEC_*` constants from the TOML
  `[meta]` block.

The emitted code refers to the rule-status, `Blocker`, and `CoverageKind` types
by the names a version's `validation::spec` module brings into scope, so both
crates share one generator without version-specific branches.

## License

Licensed under either of MIT or Apache-2.0 at your option.
