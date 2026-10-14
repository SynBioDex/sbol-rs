# Corrected cross-implementation capture

This directory holds the inputs, provenance, validation audit, and verification scripts for the corrected capture. The matrix contains **124 cases**, each with 20 warmup and 100 measured iterations. [`../results.validation-revision.json`](../results.validation-revision.json) contains every sample and the validation outcomes; [`../results.json`](../results.json) preserves the historical capture. Measurements are from one run on one host, with unrelated host services left running.

The Rust library source is the tagged **v1.0.0** release, with only the benchmark harness corrected. [`../results.validation-revision.provenance.json`](../results.validation-revision.provenance.json) identifies source commits, installed packages, image IDs, runtimes, host hardware, and the Docker VM configuration. `inputs/` contains the exact 32 input serializations, and `input-manifest.json` records their lengths and SHA-256 hashes. `harness.patch` records the benchmark changes applied to the implementation repository.

The corrected loop parses a fresh document, validates it when requested, and then serializes it. libSBOLj3's automatic validation after property updates, after reading, and before saving is disabled. Recommended and complete-document checks are enabled. Rust uses default options (complete, compliant, best practice; no rule overrides; conservative topology; external resolution off). Python uses its default native plus SHACL validator. Containers have networking disabled. Java and Python capture an expected diagnostic report in an untimed preflight before the 20 warmup iterations; all drivers check measured validation outcomes outside the timers. All three SBOL3 validators produce no diagnostics on all four timed fixtures; the four Rust SBOL2 validation cases also produce no diagnostics.

The old Python image installed PyPI's `sbol3==1.2` wheel, which was withdrawn because it contains source older than the release tag. The corrected image builds the v1.2 source commit `e73a4624ecc22ba5828d0c308ee902bb816b65b2`, with RDFLib 6.3.2 and pySHACL 0.28.1.

## Scope of the comparison

The read timings stop at unequal native API boundaries: Rust eagerly builds typed SBOL objects, Java uses a model-backed document with lazy access, and the original JavaScript read timer covers RDF parsing before its SBOL view. They are retained as API costs; the equivalent-work loading comparison is in [`../../scaling/matched/`](../../scaling/matched/README.md).

Rust's configured validator has the lowest median latency on all four SBOL3 fixtures when validation is timed separately from I/O. Timing ratios describe each library's configured validation API, including its internal work. The implementations' differences in rule coverage and ontology settings are documented below.

Rust catalogs 109 implemented SBOL3 rules. Its reports for these fixtures list 105 as fully applied and four as limited to document-local checks (`sbol3-10101`, `sbol3-10113`, `sbol3-10114`, `sbol3-12113`). These are the validator's coverage classifications for the document and configuration, not a claim that every fixture instantiates every rule's construct. Forty rules are cataloged as machine-uncheckable. The ontology registry is initialized before measured samples and performs offline term checks.

libSBOLj3 invokes Jakarta Bean Validation over document entities, using getter/property annotations and `getValidationMessages()` methods. These include identifier, cardinality, reference, sequence, location, component/feature, interaction, derivation, model, and attachment checks. Its default configuration enables recommended checks and disables complete-document checks; the latter are enabled for this benchmark. It computes term-descendant sets from bundled reduced EDAM, SO, and SBO resources once and uses these sets during validation.

pySBOL3 walks native object/property validators and then runs its bundled SHACL shapes. The shapes constrain property cardinality, class, and datatype; native checks include display identifiers, required properties, namespace consistency, sequence encoding, and some location bounds. `Document.validate_shacl()` reconstructs an RDF graph and rereads the shapes file on each call, so that internal work is included in the validation measurement. It calls pySHACL with `ont_graph=None` and `inference=None`; no biological ontology graph is used.

`rule-source-index.csv` provides a static source index for all 109 Rust rules, including Java message callsites and numbered Python native callsites. **A message identifier's presence or absence is not a coverage test.** Java has unnumbered structural checks, and Python has unnumbered native and SHACL checks. The source index cannot be interpreted as an implementation coverage score or proof of equivalent rule coverage.

The paired controls in `controls/` differ only in a Range's end coordinate, with a four-base Sequence. All three accept the end=4 control and reject end=0. Rust and libSBOLj3 reject end=5, while pySBOL3 accepts it, confirming a concrete difference in scope. `control-outcomes.json` retains the diagnostics. These are targeted checks, not a comprehensive foreign-validator conformance audit.

## Source references

- Rust v1.0.0: `crates/sbol3/rules.toml`, `crates/sbol3/src/document.rs`, `crates/sbol3/src/validation/`, `crates/sbol-core/src/validation/`, `crates/sbol-ontology/`.
- libSBOLj3 1.0.5.2: `libSBOLj3/src/main/java/org/sbolstandard/core3/util/Configuration.java`, `io/SBOLIO.java`, `validation/SBOLValidator.java`, `validation/SBOLEntityAnnotationValidator.java`, `entity/Range.java`, other `entity/*.java`, and `libSBOLj3/src/main/resources/ValidationMessages.properties`.
- pySBOL3 v1.2: `sbol3/document.py`, `identified.py`, `property_base.py`, `toplevel.py`, `sequence.py`, `location.py`, and `rdf/sbol3-shapes.ttl`.

## Verification and reproduction

Run `python3 verify_results.py` from this directory to independently check all sample statistics, input hashes, validation outcomes, and the source-index size. The equivalent-work loading results are verified separately by `../../scaling/matched/analyze.py`. `python3 check_controls.py` reruns the paired controls using the corrected images and checks the observed outcomes. Run controls separately from the performance experiment.

To reproduce the matrix, use a checkout of Rust v1.0.0 with `harness.patch` applied, populate the fixture cache as described in [`../README.md`](../README.md), build the five benchmark images, and run:

```sh
cargo run --locked --release -p sbol-bench --bin sbol-bench -- \
  --warmup 20 --iters 100 --report results.json
```

Builds from base-image tags can change over time; the captured image IDs and dependency list identify the environment actually measured.
