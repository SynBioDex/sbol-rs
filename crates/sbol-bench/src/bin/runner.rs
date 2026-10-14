//! Single-case bench runner for sbol-rs.
//!
//! Mirrors the protocol the foreign bench scripts in
//! `benches/cross-impl/{pysbol3,libsbolj3,sboljs3}/` implement: read one
//! pre-converted fixture, run the configured number of warmup and timed
//! `(parse, serialize)` iterations using sbol-rs, and emit per-iteration
//! nanoseconds as JSON to a file. The orchestrator (`sbol-bench`) shells
//! out to this binary inside the `sbol-rs-bench` Docker image so the
//! sbol-rs row pays the same Linux-VM overhead as every other row.
//!
//! Usage:
//!
//! ```text
//! runner <input> <parse_fmt> <serialize_fmt> <warmup> <iters> <output_json> [validate]
//! ```
//!
//! The optional trailing `validate` flag (`1`/`0`) turns on a timed
//! `Document::validate()` phase alongside parse and serialize.

#[path = "../validation.rs"]
mod validation;

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use sbol::v3::RdfFormat;
use sbol::{v2, v3};
use serde::Serialize;

/// Which SBOL model version the runner exercises. Selected by the
/// trailing CLI argument so the same binary benchmarks both, dispatching
/// to `sbol::v2` or `sbol::v3`.
#[derive(Clone, Copy)]
enum Version {
    Sbol2,
    Sbol3,
}

impl Version {
    fn from_arg(name: Option<&str>) -> Option<Self> {
        match name {
            // Default to SBOL 3 when the argument is absent so older
            // invocations keep working unchanged.
            None | Some("sbol3") => Some(Self::Sbol3),
            Some("sbol2") => Some(Self::Sbol2),
            Some(_) => None,
        }
    }
}

#[derive(Serialize)]
struct Output<'a> {
    #[serde(rename = "impl")]
    impl_: &'a str,
    version: &'a str,
    fixture: &'a str,
    parse_format: &'a str,
    serialize_format: &'a str,
    warmup_iters: usize,
    measured_iters: usize,
    serialized_bytes: u64,
    parse_ns: Vec<u64>,
    serialize_ns: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    validate_ns: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation: Option<serde_json::Value>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 7 {
        eprintln!(
            "usage: runner <input> <parse_fmt> <serialize_fmt> <warmup> <iters> <output_json>"
        );
        return ExitCode::from(2);
    }
    let input_path = PathBuf::from(&args[1]);
    let parse_fmt_arg = args[2].clone();
    let serialize_fmt_arg = args[3].clone();
    let warmup: usize = match args[4].parse() {
        Ok(n) => n,
        Err(error) => {
            eprintln!("invalid warmup={}: {error}", args[4]);
            return ExitCode::from(2);
        }
    };
    let iters: usize = match args[5].parse() {
        Ok(n) if n > 0 => n,
        _ => {
            eprintln!("invalid iters={}", args[5]);
            return ExitCode::from(2);
        }
    };
    let output_path = PathBuf::from(&args[6]);
    let validate = matches!(args.get(7).map(String::as_str), Some("1"));
    let version = match Version::from_arg(args.get(8).map(String::as_str)) {
        Some(version) => version,
        None => {
            eprintln!("unknown version: {}", args[8]);
            return ExitCode::from(2);
        }
    };

    let parse_format = match format_from_str(&parse_fmt_arg) {
        Some(format) => format,
        None => {
            eprintln!("unknown parse format: {parse_fmt_arg}");
            return ExitCode::from(2);
        }
    };
    let serialize_format = match format_from_str(&serialize_fmt_arg) {
        Some(format) => format,
        None => {
            eprintln!("unknown serialize format: {serialize_fmt_arg}");
            return ExitCode::from(2);
        }
    };

    let rdf_text = match fs::read_to_string(&input_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("read {}: {error}", input_path.display());
            return ExitCode::FAILURE;
        }
    };

    for i in 0..warmup {
        match run_once(&rdf_text, version, parse_format, serialize_format, validate) {
            Ok(_) => {}
            Err(error) => {
                eprintln!("warmup iter {i} failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    let mut parse_ns = Vec::with_capacity(iters);
    let mut serialize_ns = Vec::with_capacity(iters);
    let mut validate_ns = Vec::with_capacity(if validate { iters } else { 0 });
    let mut last_bytes = 0u64;
    let mut validation = None;
    let mut expected_counts = None;
    for i in 0..iters {
        match run_timed(&rdf_text, version, parse_format, serialize_format, validate) {
            Ok((parse, serialize, validate_time, bytes)) => {
                parse_ns.push(parse);
                serialize_ns.push(serialize);
                if let Some((validate_time, report)) = validate_time {
                    validate_ns.push(validate_time);
                    let counts = (report.errors().count(), report.warnings().count());
                    if expected_counts.is_some_and(|expected| expected != counts) {
                        eprintln!("validation outcome changed across iterations");
                        return ExitCode::FAILURE;
                    }
                    expected_counts = Some(counts);
                    if i + 1 == iters {
                        validation = Some(validation::summary(&report));
                    }
                }
                last_bytes = bytes;
            }
            Err(error) => {
                eprintln!("measured iter {i} failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    let output = Output {
        impl_: "sbol-rs",
        version: env!("SBOL_CRATE_VERSION"),
        fixture: input_path.to_str().unwrap_or(""),
        parse_format: &parse_fmt_arg,
        serialize_format: &serialize_fmt_arg,
        warmup_iters: warmup,
        measured_iters: iters,
        serialized_bytes: last_bytes,
        parse_ns,
        serialize_ns,
        validate_ns,
        validation,
    };
    let json = match serde_json::to_string(&output) {
        Ok(s) => s,
        Err(error) => {
            eprintln!("serialize output json: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = fs::write(&output_path, json) {
        eprintln!("write {}: {error}", output_path.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn format_from_str(name: &str) -> Option<RdfFormat> {
    match name {
        "turtle" => Some(RdfFormat::Turtle),
        "rdfxml" => Some(RdfFormat::RdfXml),
        "jsonld" => Some(RdfFormat::JsonLd),
        "ntriples" => Some(RdfFormat::NTriples),
        _ => None,
    }
}

fn run_once(
    text: &str,
    version: Version,
    parse_format: RdfFormat,
    serialize_format: RdfFormat,
    validate: bool,
) -> Result<(), String> {
    match version {
        Version::Sbol3 => {
            let doc = v3::Document::read(text, parse_format).map_err(|e| format!("parse: {e}"))?;
            if validate {
                let _ = doc.validate();
            }
            doc.write(serialize_format)
                .map_err(|e| format!("serialize: {e}"))?;
        }
        Version::Sbol2 => {
            let doc = v2::Document::read(text, parse_format).map_err(|e| format!("parse: {e}"))?;
            if validate {
                let _ = doc.validate();
            }
            doc.write(serialize_format)
                .map_err(|e| format!("serialize: {e}"))?;
        }
    }
    Ok(())
}

fn run_timed(
    text: &str,
    version: Version,
    parse_format: RdfFormat,
    serialize_format: RdfFormat,
    validate: bool,
) -> Result<(u64, u64, Option<(u64, v3::ValidationReport)>, u64), String> {
    match version {
        Version::Sbol3 => {
            let t0 = Instant::now();
            let doc = v3::Document::read(text, parse_format).map_err(|e| format!("parse: {e}"))?;
            let t1 = Instant::now();
            let validate_ns = if validate {
                let v0 = Instant::now();
                let report = doc.validate();
                let elapsed = v0.elapsed().as_nanos() as u64;
                Some((elapsed, report))
            } else {
                None
            };
            let s0 = Instant::now();
            let out = doc
                .write(serialize_format)
                .map_err(|e| format!("serialize: {e}"))?;
            let s1 = Instant::now();
            Ok((
                t1.duration_since(t0).as_nanos() as u64,
                s1.duration_since(s0).as_nanos() as u64,
                validate_ns,
                out.len() as u64,
            ))
        }
        Version::Sbol2 => {
            let t0 = Instant::now();
            let doc = v2::Document::read(text, parse_format).map_err(|e| format!("parse: {e}"))?;
            let t1 = Instant::now();
            let validate_ns = if validate {
                let v0 = Instant::now();
                let report = doc.validate();
                let elapsed = v0.elapsed().as_nanos() as u64;
                Some((elapsed, report))
            } else {
                None
            };
            let s0 = Instant::now();
            let out = doc
                .write(serialize_format)
                .map_err(|e| format!("serialize: {e}"))?;
            let s1 = Instant::now();
            Ok((
                t1.duration_since(t0).as_nanos() as u64,
                s1.duration_since(s0).as_nanos() as u64,
                validate_ns,
                out.len() as u64,
            ))
        }
    }
}
