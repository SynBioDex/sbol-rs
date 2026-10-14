//! Running a single benchmark case: preparing fixtures in every RDF
//! serialization, timing the in-process Rust path, and shelling out to the
//! Docker images for the foreign implementations.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use sbol::v3::RdfFormat;
use sbol::{v2, v3};
use serde::Deserialize;

use crate::cli::Config;
use crate::matrix::{BenchCase, Fixture, Version};

#[derive(Debug, Deserialize)]
pub(crate) struct BenchSamples {
    #[serde(default)]
    pub(crate) version: Option<String>,
    pub(crate) parse_ns: Vec<u64>,
    pub(crate) serialize_ns: Vec<u64>,
    // Per-iteration validation timings. Empty for cases that don't run
    // the validation phase (every conversion row, plus sboljs, which
    // ships no validator).
    #[serde(default)]
    pub(crate) validate_ns: Vec<u64>,
    #[serde(default)]
    pub(crate) serialized_bytes: u64,
    #[serde(default)]
    pub(crate) validation: Option<serde_json::Value>,
}

// Deserialization surface for whatever the bench scripts emit. The
// `impl` field is read by serde to ignore it (we already know which
// implementation produced the JSON from the case we dispatched);
// `version` is what shows up in the report. `validate_ns` is absent
// unless the driver ran the validation phase.
#[derive(Deserialize)]
pub(crate) struct ForeignSamples {
    #[serde(default)]
    pub(crate) version: Option<String>,
    pub(crate) parse_ns: Vec<u64>,
    pub(crate) serialize_ns: Vec<u64>,
    #[serde(default)]
    pub(crate) validate_ns: Vec<u64>,
    #[serde(default)]
    pub(crate) serialized_bytes: u64,
    #[serde(default)]
    pub(crate) validation: Option<serde_json::Value>,
}

#[derive(Debug)]
pub(crate) struct CaseOutcome {
    pub(crate) case: BenchCase,
    pub(crate) fixture: Fixture,
    pub(crate) state: CaseState,
}

#[derive(Debug)]
pub(crate) enum CaseState {
    Ok {
        version: String,
        samples: BenchSamples,
    },
    Skipped {
        reason: String,
    },
}

pub(crate) fn prepare_fixture_in_every_format(
    source: &Path,
    workspace_root: &Path,
    scratch_root: &Path,
    stem: &str,
    version: Version,
) -> Result<BTreeMap<RdfFormat, PathBuf>, String> {
    match version {
        Version::Sbol3 => prepare_sbol3_fixture(source, workspace_root, scratch_root, stem),
        Version::Sbol2 => prepare_sbol2_fixture(source, scratch_root, stem),
    }
}

fn prepare_sbol3_fixture(
    source: &Path,
    workspace_root: &Path,
    scratch_root: &Path,
    stem: &str,
) -> Result<BTreeMap<RdfFormat, PathBuf>, String> {
    let source_text = fs::read_to_string(source)
        .map_err(|error| format!("read {}: {error}", source.display()))?;
    let document = v3::Document::read_turtle(&source_text)
        .map_err(|error| format!("parse {}: {error}", source.display()))?;

    let mut paths = BTreeMap::new();
    for &format in RdfFormat::ALL {
        let output_path = scratch_root.join(format!("{stem}.{}", format.extension()));

        // RDF/XML gets special handling: sbol-rs emits inline `xmlns`
        // on every child element, which is valid RDF/XML 1.1 but
        // sboljs's rdf-parser-rdfxml mis-parses the inner literal
        // elements as blank-node subjects. libSBOLj3 emits standard
        // prefix-style RDF/XML that every parser including
        // rdf-parser-rdfxml handles correctly, so we use libSBOLj3's
        // committed reference output for the RDF/XML input row across
        // every impl. The triple set is the same (the conformance
        // tests guarantee this), so the bench stays apples-to-apples.
        let serialized = if matches!(format, RdfFormat::RdfXml) {
            let normalized = workspace_root
                .join("tests/fixtures/cross-impl")
                .join(format!("{stem}.libSBOLj3.expected.rdf"));
            fs::read_to_string(&normalized).map_err(|error| {
                format!(
                    "read libSBOLj3 reference rdfxml {} \
                     (needed for cross-impl parity in the RDF/XML row): {error}",
                    normalized.display()
                )
            })?
        } else {
            document
                .write(format)
                .map_err(|error| format!("serialize {} as {format}: {error}", source.display()))?
        };

        fs::write(&output_path, &serialized).map_err(|error| {
            format!(
                "write pre-converted fixture {}: {error}",
                output_path.display()
            )
        })?;
        paths.insert(format, output_path);
    }
    Ok(paths)
}

fn prepare_sbol2_fixture(
    source: &Path,
    scratch_root: &Path,
    stem: &str,
) -> Result<BTreeMap<RdfFormat, PathBuf>, String> {
    // SBOL 2 is exchanged as RDF/XML, and the source fixtures are the
    // RDF/XML files the test suite and SynBioHub ship. sbol-rs reads
    // that RDF/XML and re-emits Turtle, JSON-LD, and N-Triples so every
    // impl sees identical bytes per format. The RDF/XML input row uses
    // the source file verbatim: it is standard prefix-style RDF/XML
    // that libSBOLj parses natively, so no reference re-emit is needed.
    let source_text = fs::read_to_string(source)
        .map_err(|error| format!("read {}: {error}", source.display()))?;
    let document = v2::Document::read(&source_text, RdfFormat::RdfXml)
        .map_err(|error| format!("parse {}: {error}", source.display()))?;

    let mut paths = BTreeMap::new();
    for &format in RdfFormat::ALL {
        let output_path = scratch_root.join(format!("{stem}.{}", format.extension()));
        let serialized = if matches!(format, RdfFormat::RdfXml) {
            source_text.clone()
        } else {
            document
                .write(format)
                .map_err(|error| format!("serialize {} as {format}: {error}", source.display()))?
        };
        fs::write(&output_path, &serialized).map_err(|error| {
            format!(
                "write pre-converted fixture {}: {error}",
                output_path.display()
            )
        })?;
        paths.insert(format, output_path);
    }
    Ok(paths)
}

pub(crate) fn run_native(
    input_path: &Path,
    case: BenchCase,
    config: &Config,
) -> Result<BenchSamples, String> {
    match case.version {
        Version::Sbol3 => run_native_sbol3(input_path, case, config),
        Version::Sbol2 => run_native_sbol2(input_path, case, config),
    }
}

fn run_native_sbol3(
    input_path: &Path,
    case: BenchCase,
    config: &Config,
) -> Result<BenchSamples, String> {
    let rdf_text = fs::read_to_string(input_path)
        .map_err(|error| format!("read {}: {error}", input_path.display()))?;

    for _ in 0..config.warmup {
        let doc = v3::Document::read(&rdf_text, case.parse_format)
            .map_err(|error| format!("warmup parse: {error}"))?;
        if case.validate {
            let _ = doc.validate();
        }
        let _ = doc
            .write(case.serialize_format)
            .map_err(|error| format!("warmup serialize: {error}"))?;
    }

    let mut parse_ns = Vec::with_capacity(config.iters);
    let mut serialize_ns = Vec::with_capacity(config.iters);
    let mut validate_ns = Vec::with_capacity(if case.validate { config.iters } else { 0 });
    let mut last_bytes = 0u64;
    let mut validation = None;
    for _ in 0..config.iters {
        let t0 = Instant::now();
        let doc = v3::Document::read(&rdf_text, case.parse_format)
            .map_err(|error| format!("measured parse: {error}"))?;
        let t1 = Instant::now();
        parse_ns.push(t1.duration_since(t0).as_nanos() as u64);
        if case.validate {
            let v0 = Instant::now();
            let report = doc.validate();
            let elapsed = v0.elapsed().as_nanos() as u64;
            validate_ns.push(elapsed);
            validation = Some(crate::validation::summary(&report));
        }
        let s0 = Instant::now();
        let out = doc
            .write(case.serialize_format)
            .map_err(|error| format!("measured serialize: {error}"))?;
        let s1 = Instant::now();
        serialize_ns.push(s1.duration_since(s0).as_nanos() as u64);
        last_bytes = out.len() as u64;
    }

    Ok(BenchSamples {
        version: Some(rust_impl_version().to_owned()),
        parse_ns,
        serialize_ns,
        validate_ns,
        serialized_bytes: last_bytes,
        validation,
    })
}

fn run_native_sbol2(
    input_path: &Path,
    case: BenchCase,
    config: &Config,
) -> Result<BenchSamples, String> {
    let rdf_text = fs::read_to_string(input_path)
        .map_err(|error| format!("read {}: {error}", input_path.display()))?;

    for _ in 0..config.warmup {
        let doc = v2::Document::read(&rdf_text, case.parse_format)
            .map_err(|error| format!("warmup parse: {error}"))?;
        if case.validate {
            let _ = doc.validate();
        }
        let _ = doc
            .write(case.serialize_format)
            .map_err(|error| format!("warmup serialize: {error}"))?;
    }

    let mut parse_ns = Vec::with_capacity(config.iters);
    let mut serialize_ns = Vec::with_capacity(config.iters);
    let mut validate_ns = Vec::with_capacity(if case.validate { config.iters } else { 0 });
    let mut last_bytes = 0u64;
    let mut validation = None;
    for _ in 0..config.iters {
        let t0 = Instant::now();
        let doc = v2::Document::read(&rdf_text, case.parse_format)
            .map_err(|error| format!("measured parse: {error}"))?;
        let t1 = Instant::now();
        parse_ns.push(t1.duration_since(t0).as_nanos() as u64);
        if case.validate {
            let v0 = Instant::now();
            let report = doc.validate();
            let elapsed = v0.elapsed().as_nanos() as u64;
            validate_ns.push(elapsed);
            validation = Some(crate::validation::summary(&report));
        }
        let s0 = Instant::now();
        let out = doc
            .write(case.serialize_format)
            .map_err(|error| format!("measured serialize: {error}"))?;
        let s1 = Instant::now();
        serialize_ns.push(s1.duration_since(s0).as_nanos() as u64);
        last_bytes = out.len() as u64;
    }

    Ok(BenchSamples {
        version: Some(rust_impl_version().to_owned()),
        parse_ns,
        serialize_ns,
        validate_ns,
        serialized_bytes: last_bytes,
        validation,
    })
}

pub(crate) fn run_docker(
    input_path: &Path,
    output_json: &Path,
    case: BenchCase,
    config: &Config,
) -> Result<BenchSamples, String> {
    let image = case.implementation.docker_image();
    if !image_present(image) {
        return Err(format!(
            "docker image `{image}` not built; run `{}`",
            case.implementation.docker_build_command()
        ));
    }

    // Mount the scratch dir read-write because the script writes the
    // timing JSON next to the input file. Containers and the host see
    // the same path under /work, so we translate.
    let scratch = input_path
        .parent()
        .ok_or_else(|| "input path has no parent".to_owned())?;
    let host_mount = scratch.to_string_lossy().to_string();
    let input_name = path_basename(input_path);
    let output_name = path_basename(output_json);
    let container_input = format!("/work/{input_name}");
    let container_output = format!("/work/{output_name}");
    let mount_arg = format!("{host_mount}:/work");

    let warmup_arg = config.warmup.to_string();
    let iters_arg = config.iters.to_string();
    let parse_arg = format_name(case.parse_format);
    let serialize_arg = format_name(case.serialize_format);
    // Trailing positional flag telling the driver whether to run and
    // time the validation phase. Drivers whose implementation has no
    // validator ignore it; the harness only sets it on cases marked
    // `validate` in the matrix.
    let validate_arg = if case.validate { "1" } else { "0" };
    // Trailing version argument. The sbol-rs runner dispatches to its
    // SBOL 2 or SBOL 3 model on it; the version-specific foreign
    // drivers ignore it.
    let version_arg = case.version.id();

    let result = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network=none",
            "-v",
            &mount_arg,
            image,
            &container_input,
            parse_arg,
            serialize_arg,
            &warmup_arg,
            &iters_arg,
            &container_output,
            validate_arg,
            version_arg,
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("spawn docker: {error}"))?;
    if !result.success() {
        return Err(format!("`docker run {image}` exited with status {result}"));
    }

    let text = fs::read_to_string(output_json)
        .map_err(|error| format!("read timings json {}: {error}", output_json.display()))?;
    let raw: ForeignSamples = serde_json::from_str(&text)
        .map_err(|error| format!("parse timings json {}: {error}", output_json.display()))?;
    if raw.parse_ns.len() != config.iters || raw.serialize_ns.len() != config.iters {
        return Err(format!(
            "bench produced {} parse samples and {} serialize samples; expected {}",
            raw.parse_ns.len(),
            raw.serialize_ns.len(),
            config.iters
        ));
    }
    if case.validate && raw.validate_ns.len() != config.iters {
        return Err(format!(
            "bench marked for validation produced {} validate samples; expected {}",
            raw.validate_ns.len(),
            config.iters
        ));
    }
    Ok(BenchSamples {
        version: raw.version,
        parse_ns: raw.parse_ns,
        serialize_ns: raw.serialize_ns,
        validate_ns: raw.validate_ns,
        serialized_bytes: raw.serialized_bytes,
        validation: raw.validation,
    })
}

pub(crate) fn check_docker_available() -> bool {
    Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(crate) fn image_present(image: &str) -> bool {
    Command::new("docker")
        .args(["image", "inspect", image])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(crate) fn format_name(format: RdfFormat) -> &'static str {
    match format {
        RdfFormat::Turtle => "turtle",
        RdfFormat::RdfXml => "rdfxml",
        RdfFormat::JsonLd => "jsonld",
        RdfFormat::NTriples => "ntriples",
        _ => panic!("bench harness has no name for format {format}"),
    }
}

pub(crate) fn rust_impl_version() -> &'static str {
    env!("SBOL_CRATE_VERSION")
}

pub(crate) fn path_basename(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("input")
        .to_owned()
}
