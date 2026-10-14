//! Cross-implementation performance bench.
//!
//! This harness compares sbol-rs against the mainstream implementation
//! of each SBOL version, across SBOL 2 and SBOL 3. It times `(parse, serialize)`
//! round trips and cross-format conversions against sbol-rs
//! (in-process), plus the mainstream implementation of each version:
//! libSBOLj for SBOL 2, and pySBOL3, libSBOLj3, and sboljs for SBOL 3
//! (each pinned in Docker). Inputs are representative fixtures
//! pre-converted by sbol-rs to every RDF serialization on disk so each
//! implementation sees the same byte-for-byte input. Each bench script
//! reports per-iteration nanoseconds in JSON; this binary aggregates
//! and prints median and p99 for every (version × impl × fixture ×
//! format) combination, with the inter-quartile range also recorded in
//! the JSON report, falling back to a "skipped" row when a tool refuses
//! or fails.
//!
//! Methodology notes worth knowing before reading the numbers:
//!
//! - Wall-clock time only; no allocator or memory measurements. JVM and
//!   Node startup costs are excluded by running warmup iterations
//!   inside each container.
//! - The default workload (20 warmup + 100 measured iters) reaches steady
//!   state for the JIT-backed implementations and yields stable medians.
//!   Override via `SBOL_BENCH_WARMUP` / `SBOL_BENCH_ITERS` for tighter
//!   error bars.
//! - SBOL 2 is exchanged as RDF/XML; sbol-rs also reads and writes
//!   Turtle, JSON-LD, and N-Triples for SBOL 2, so those are benchmarked
//!   for sbol-rs. libSBOLj only appears where its input is RDF/XML.
//! - sboljs's underlying rdfoo parses only N-Triples and RDF/XML and
//!   serializes only RDF/XML, so the sboljs row only appears in those
//!   format combinations. This is a real ecosystem fact, not a harness
//!   bug.

mod bench;
mod cli;
mod matrix;
mod report;
mod validation;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

use crate::bench::{
    CaseOutcome, CaseState, check_docker_available, prepare_fixture_in_every_format, run_docker,
    run_native, rust_impl_version,
};
use crate::cli::{Cli, Config};
use crate::matrix::{BENCH_CASES, Implementation};
use crate::report::{print_report, write_json_report};

fn main() -> ExitCode {
    let config = match Config::from_cli(Cli::parse()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("config error: {error}");
            return ExitCode::FAILURE;
        }
    };

    let workspace = workspace_root();
    let run_id = format!(
        "{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    let scratch_root = workspace.join(format!("target/sbol-bench/{run_id}"));
    if let Err(error) = fs::create_dir_all(&scratch_root) {
        eprintln!(
            "failed to create scratch dir {}: {error}",
            scratch_root.display()
        );
        return ExitCode::FAILURE;
    }

    eprintln!("sbol-bench run {run_id}");
    eprintln!("  warmup iters : {}", config.warmup);
    eprintln!("  measured iters: {}", config.iters);
    eprintln!("  fixtures      : {}", config.fixtures.len());
    eprintln!("  scratch       : {}", scratch_root.display());

    let docker_available = if config.skip_docker {
        false
    } else {
        check_docker_available()
    };
    if !docker_available && !config.skip_docker {
        eprintln!(
            "docker not reachable; foreign implementations will be skipped \
             and sbol-rs will fall back to in-process timing \
             (set SBOL_BENCH_DOCKER=0 to silence this warning)"
        );
    }

    let mut outcomes: Vec<CaseOutcome> = Vec::new();
    let mut any_native_failure = false;

    for fixture in &config.fixtures {
        // Only the cases whose model version matches the fixture apply.
        let cases = || {
            BENCH_CASES
                .iter()
                .filter(|case| case.version == fixture.version)
        };
        let source = workspace
            .join(fixture.version.fixture_dir())
            .join(fixture.source);
        if !source.is_file() {
            eprintln!(
                "fixture missing at {}; populate the fixture cache (run the \
                 corresponding integration test once)",
                source.display()
            );
            for case in cases() {
                outcomes.push(CaseOutcome {
                    case: *case,
                    fixture: *fixture,
                    state: CaseState::Skipped {
                        reason: format!("fixture {} not on disk", source.display()),
                    },
                });
            }
            continue;
        }

        let prepared = match prepare_fixture_in_every_format(
            &source,
            &workspace,
            &scratch_root,
            fixture.stem,
            fixture.version,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                eprintln!("failed to prepare {}: {error}", fixture.stem);
                for case in cases() {
                    outcomes.push(CaseOutcome {
                        case: *case,
                        fixture: *fixture,
                        state: CaseState::Skipped {
                            reason: format!("could not pre-convert fixture: {error}"),
                        },
                    });
                }
                continue;
            }
        };

        for case in cases() {
            let input_path = prepared
                .get(&case.parse_format)
                .expect("every format was prepared above");
            let outcome = if docker_available {
                let output_json = scratch_root.join(format!(
                    "{}.{}.{}-{}.json",
                    fixture.stem,
                    case.implementation.id(),
                    case.parse_format.extension(),
                    case.serialize_format.extension(),
                ));
                match run_docker(input_path, &output_json, *case, &config) {
                    Ok(samples) => {
                        let version = samples.version.clone().unwrap_or_default();
                        CaseOutcome {
                            case: *case,
                            fixture: *fixture,
                            state: CaseState::Ok { version, samples },
                        }
                    }
                    Err(error) => CaseOutcome {
                        case: *case,
                        fixture: *fixture,
                        state: CaseState::Skipped {
                            reason: format!("{} bench failed: {error}", case.implementation.id()),
                        },
                    },
                }
            } else if matches!(case.implementation, Implementation::SbolRs) {
                // Docker is unavailable: fall back to in-process Rust so devs
                // can iterate without building the Docker image. The result
                // is not directly comparable to a foreign-impl row (no VM
                // overhead, native macOS vs Linux), so the report tags this
                // case with `(native, not in container)` for clarity.
                match run_native(input_path, *case, &config) {
                    Ok(samples) => CaseOutcome {
                        case: *case,
                        fixture: *fixture,
                        state: CaseState::Ok {
                            version: format!("{} (native, not in container)", rust_impl_version()),
                            samples,
                        },
                    },
                    Err(error) => {
                        any_native_failure = true;
                        CaseOutcome {
                            case: *case,
                            fixture: *fixture,
                            state: CaseState::Skipped {
                                reason: format!("native bench failed: {error}"),
                            },
                        }
                    }
                }
            } else {
                CaseOutcome {
                    case: *case,
                    fixture: *fixture,
                    state: CaseState::Skipped {
                        reason: "docker unavailable".to_owned(),
                    },
                }
            };
            outcomes.push(outcome);
        }
    }

    print_report(&outcomes);

    if let Some(report_path) = config.report_path.as_ref() {
        match write_json_report(report_path, &outcomes, &config) {
            Ok(()) => eprintln!("wrote machine-readable report to {}", report_path.display()),
            Err(error) => {
                eprintln!(
                    "failed to write report to {}: {error}",
                    report_path.display()
                );
                return ExitCode::FAILURE;
            }
        }
    }

    if any_native_failure
        || (!config.skip_docker
            && outcomes
                .iter()
                .any(|outcome| matches!(outcome.state, CaseState::Skipped { .. })))
    {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or(manifest)
}
