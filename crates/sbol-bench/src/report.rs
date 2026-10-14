//! Aggregating per-iteration timings into median, inter-quartile range, and
//! p99 stats and rendering the human-readable table plus the optional
//! machine-readable JSON report.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bench::format_name;
use crate::bench::{CaseOutcome, CaseState};
use crate::cli::Config;
use crate::matrix::Version;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(crate) struct Stats {
    pub(crate) median_ns: u64,
    pub(crate) p25_ns: u64,
    pub(crate) p75_ns: u64,
    pub(crate) p99_ns: u64,
}

/// Nearest-rank percentile over an already-sorted, non-empty slice.
fn percentile(sorted: &[u64], q: f64) -> u64 {
    let n = sorted.len();
    let idx = ((n as f64 * q).ceil() as usize)
        .saturating_sub(1)
        .min(n - 1);
    sorted[idx]
}

pub(crate) fn stats(samples: &[u64]) -> Option<Stats> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let median_ns = if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2
    };
    Some(Stats {
        median_ns,
        p25_ns: percentile(&sorted, 0.25),
        p75_ns: percentile(&sorted, 0.75),
        p99_ns: percentile(&sorted, 0.99),
    })
}

pub(crate) fn print_report(outcomes: &[CaseOutcome]) {
    let mut by_group: BTreeMap<(Version, &'static str), Vec<&CaseOutcome>> = BTreeMap::new();
    for outcome in outcomes {
        by_group
            .entry((outcome.fixture.version, outcome.fixture.stem))
            .or_default()
            .push(outcome);
    }

    let mut current_version: Option<Version> = None;
    for ((version, fixture_stem), fixture_outcomes) in by_group {
        if current_version != Some(version) {
            println!();
            println!("################ {} ################", version.label());
            current_version = Some(version);
        }
        println!();
        println!("=== fixture: {fixture_stem} ===");
        println!(
            "{:<11} {:<20} {:<9} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>10}",
            "impl",
            "conversion",
            "kind",
            "parse p50 (μs)",
            "parse p99 (μs)",
            "ser. p50 (μs)",
            "ser. p99 (μs)",
            "val. p50 (μs)",
            "val. p99 (μs)",
            "bytes"
        );
        for outcome in fixture_outcomes {
            let parse_label = format_name(outcome.case.parse_format);
            let serialize_label = format_name(outcome.case.serialize_format);
            let conversion = format!("{parse_label} -> {serialize_label}");
            let kind = if outcome.case.is_conversion() {
                "convert"
            } else {
                "round-trip"
            };
            match &outcome.state {
                CaseState::Ok { samples, .. } => {
                    let parse = stats(&samples.parse_ns);
                    let ser = stats(&samples.serialize_ns);
                    let val = stats(&samples.validate_ns);
                    match (parse, ser) {
                        (Some(parse), Some(ser)) => {
                            let (val_p50, val_p99) = match val {
                                Some(val) => (
                                    format!("{:.2}", val.median_ns as f64 / 1_000.0),
                                    format!("{:.2}", val.p99_ns as f64 / 1_000.0),
                                ),
                                None => ("—".to_owned(), "—".to_owned()),
                            };
                            println!(
                                "{:<11} {:<20} {:<9} {:>13.2} {:>13.2} {:>13.2} {:>13.2} {:>13} {:>13} {:>10}",
                                outcome.case.implementation.id(),
                                conversion,
                                kind,
                                parse.median_ns as f64 / 1_000.0,
                                parse.p99_ns as f64 / 1_000.0,
                                ser.median_ns as f64 / 1_000.0,
                                ser.p99_ns as f64 / 1_000.0,
                                val_p50,
                                val_p99,
                                samples.serialized_bytes
                            );
                        }
                        _ => {
                            println!(
                                "{:<11} {:<20} {:<9} (no samples)",
                                outcome.case.implementation.id(),
                                conversion,
                                kind
                            );
                        }
                    }
                }
                CaseState::Skipped { reason } => {
                    println!(
                        "{:<11} {:<20} {:<9} skipped: {}",
                        outcome.case.implementation.id(),
                        conversion,
                        kind,
                        reason
                    );
                }
            }
        }
    }
    println!();
    println!(
        "p50 = median across measured iters; p99 picked from same set. Lower is better. \
         `val.` columns show `validate` timing where the impl ships a validator; \
         `—` marks rows that don't run the validation phase."
    );
}

#[derive(serde::Serialize)]
pub(crate) struct ReportFile<'a> {
    pub(crate) run_id: String,
    pub(crate) warmup_iters: usize,
    pub(crate) measured_iters: usize,
    pub(crate) cases: Vec<ReportCase<'a>>,
}

#[derive(serde::Serialize)]
pub(crate) struct ReportCase<'a> {
    pub(crate) version: &'a str,
    pub(crate) fixture: &'a str,
    pub(crate) implementation: &'a str,
    pub(crate) parse_format: &'a str,
    pub(crate) serialize_format: &'a str,
    /// `true` when the parse and serialize formats differ, i.e. the row
    /// measures format-conversion cost rather than a same-format round trip.
    pub(crate) conversion: bool,
    pub(crate) state: ReportState<'a>,
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ReportState<'a> {
    Ok {
        version: &'a str,
        serialized_bytes: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        validation: Option<&'a serde_json::Value>,
        parse_stats: Stats,
        serialize_stats: Stats,
        // Present only when the row ran the validation phase.
        #[serde(skip_serializing_if = "Option::is_none")]
        validate_stats: Option<Stats>,
        parse_ns: &'a [u64],
        serialize_ns: &'a [u64],
        #[serde(skip_serializing_if = "<[u64]>::is_empty")]
        validate_ns: &'a [u64],
    },
    Skipped {
        reason: &'a str,
    },
}

pub(crate) fn write_json_report(
    path: &Path,
    outcomes: &[CaseOutcome],
    config: &Config,
) -> Result<(), String> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned());
    let cases = outcomes
        .iter()
        .map(|outcome| {
            let state = match &outcome.state {
                CaseState::Ok { version, samples } => {
                    let zero = Stats {
                        median_ns: 0,
                        p25_ns: 0,
                        p75_ns: 0,
                        p99_ns: 0,
                    };
                    ReportState::Ok {
                        version: version.as_str(),
                        serialized_bytes: samples.serialized_bytes,
                        validation: samples.validation.as_ref(),
                        parse_stats: stats(&samples.parse_ns).unwrap_or(zero),
                        serialize_stats: stats(&samples.serialize_ns).unwrap_or(zero),
                        validate_stats: stats(&samples.validate_ns),
                        parse_ns: &samples.parse_ns,
                        serialize_ns: &samples.serialize_ns,
                        validate_ns: &samples.validate_ns,
                    }
                }
                CaseState::Skipped { reason } => ReportState::Skipped {
                    reason: reason.as_str(),
                },
            };
            ReportCase {
                version: outcome.fixture.version.id(),
                fixture: outcome.fixture.stem,
                implementation: outcome.case.implementation.id(),
                parse_format: format_name(outcome.case.parse_format),
                serialize_format: format_name(outcome.case.serialize_format),
                conversion: outcome.case.is_conversion(),
                state,
            }
        })
        .collect();
    let report = ReportFile {
        run_id,
        warmup_iters: config.warmup,
        measured_iters: config.iters,
        cases,
    };
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create report parent dir {}: {error}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("serialize report: {error}"))?;
    fs::write(path, json).map_err(|error| format!("write report {}: {error}", path.display()))?;
    Ok(())
}
