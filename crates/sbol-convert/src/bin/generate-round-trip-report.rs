//! Round-trip smoke test report generator.
//!
//! Exercises every real-world SBOL 2 fixture through the full
//! upgrade → validate → downgrade → re-upgrade → re-validate
//! pipeline, computes a triple-level diff between the first and second
//! SBOL 3 representations, and writes a committed Markdown report to
//! `docs/sbol3-round-trip-report.md`.
//!
//! The point is not regression gating; that lives in
//! `crates/sbol-convert/tests/upgrade_conformance.rs` and
//! `crates/sbol-convert/tests/downgrade_round_trip.rs`. The point is empirical
//! discovery: turning the question "which deferred enhancements
//! actually matter on real data?" into "here are the triples that
//! don't survive the round-trip, fixture by fixture."
//!
//! Usage:
//!
//!     cargo run -p sbol-convert --bin generate-round-trip-report

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sbol_convert::canonical_nt_line;
use sbol3::{RdfFormat, Severity, Triple};

fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn collect_fixture_paths(
    fixtures_dir: &Path,
    expected_dir: &Path,
) -> std::io::Result<Vec<PathBuf>> {
    fn walk(dir: &Path, expected_dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path == *expected_dir {
                continue;
            }
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                walk(&path, expected_dir, out)?;
            } else if file_type.is_file()
                && path.extension().and_then(|s| s.to_str()) == Some("xml")
            {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(fixtures_dir, expected_dir, &mut out)?;
    Ok(out)
}

fn fixture_name(path: &Path, fixtures_dir: &Path) -> String {
    let relative = path
        .strip_prefix(fixtures_dir)
        .unwrap_or(path)
        .with_extension("");
    relative
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Vec<&str>>()
        .join("/")
}

enum Outcome {
    Clean,
    LossyButValid {
        gained: Vec<String>,
        lost: Vec<String>,
        /// Validation errors on the round-tripped SBOL 3 that were NOT
        /// already present on the upgrade output. The SBOLTestSuite
        /// corpus contains intentionally non-compliant fixtures, so
        /// preserved errors are not a round-trip failure; only newly
        /// introduced ones are.
        new_validation_errors: usize,
    },
    Failed {
        stage: &'static str,
        message: String,
    },
}

struct FixtureResult {
    name: String,
    initial_triples: usize,
    roundtrip_triples: usize,
    initial_validation_errors: usize,
    downgrade_warnings: Vec<String>,
    outcome: Outcome,
}

fn run_fixture(path: &Path, name: &str) -> FixtureResult {
    let mut result = FixtureResult {
        name: name.to_string(),
        initial_triples: 0,
        roundtrip_triples: 0,
        initial_validation_errors: 0,
        downgrade_warnings: Vec::new(),
        outcome: Outcome::Failed {
            stage: "init",
            message: String::new(),
        },
    };

    let bytes = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            result.outcome = Outcome::Failed {
                stage: "read",
                message: err.to_string(),
            };
            return result;
        }
    };

    let (upgraded, _ureport) = match sbol_convert::upgrade_from_sbol2(&bytes, RdfFormat::RdfXml) {
        Ok(pair) => pair,
        Err(err) => {
            result.outcome = Outcome::Failed {
                stage: "upgrade",
                message: format!("{err}"),
            };
            return result;
        }
    };

    let initial_validation = upgraded.validate();
    result.initial_validation_errors = initial_validation
        .issues()
        .iter()
        .filter(|i| matches!(i.severity, Severity::Error))
        .count();

    let initial_triples: Vec<Triple> = upgraded.rdf_graph().normalized_triples();
    let initial_nt: BTreeSet<String> = initial_triples.iter().map(canonical_nt_line).collect();
    result.initial_triples = initial_nt.len();

    let (downgraded_graph, dreport) = match sbol_convert::downgrade(&upgraded) {
        Ok(pair) => pair,
        Err(err) => {
            result.outcome = Outcome::Failed {
                stage: "downgrade",
                message: format!("{err}"),
            };
            return result;
        }
    };
    result.downgrade_warnings = dreport
        .warnings()
        .iter()
        .map(|w| format!("{w:?}"))
        .collect();

    let turtle = match downgraded_graph.write(RdfFormat::Turtle) {
        Ok(text) => text,
        Err(err) => {
            result.outcome = Outcome::Failed {
                stage: "serialize",
                message: format!("{err}"),
            };
            return result;
        }
    };

    let (reupgraded, _) = match sbol_convert::upgrade_from_sbol2(&turtle, RdfFormat::Turtle) {
        Ok(pair) => pair,
        Err(err) => {
            result.outcome = Outcome::Failed {
                stage: "re-upgrade",
                message: format!("{err}"),
            };
            return result;
        }
    };

    let validation = reupgraded.validate();
    let roundtrip_validation_errors = validation
        .issues()
        .iter()
        .filter(|i| matches!(i.severity, Severity::Error))
        .count();
    let new_validation_errors =
        roundtrip_validation_errors.saturating_sub(result.initial_validation_errors);

    let roundtrip_triples: Vec<Triple> = reupgraded.rdf_graph().normalized_triples();
    let roundtrip_nt: BTreeSet<String> = roundtrip_triples.iter().map(canonical_nt_line).collect();
    result.roundtrip_triples = roundtrip_nt.len();

    let gained: Vec<String> = roundtrip_nt.difference(&initial_nt).cloned().collect();
    let lost: Vec<String> = initial_nt.difference(&roundtrip_nt).cloned().collect();

    result.outcome = if gained.is_empty() && lost.is_empty() && new_validation_errors == 0 {
        Outcome::Clean
    } else {
        Outcome::LossyButValid {
            gained,
            lost,
            new_validation_errors,
        }
    };

    result
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

fn render_report(results: &[FixtureResult]) -> String {
    let mut out = String::new();
    out.push_str("# Round-trip smoke test report\n\n");
    out.push_str(
        "Generated by `cargo run -p sbol-convert --bin generate-round-trip-report`.\n\n\
         **Scope:** verifies that SBOL 2 → SBOL 3 → SBOL 2 → SBOL 3 round-trips \
         preserve every triple across the committed SBOL 2 fixture corpus under \
         `tests/fixtures/sbol2/real/` (SBOLTestSuite, SynBioHub, and the \
         GenBank-derived intermediates). Native-SBOL-3 → SBOL 2 behavior (in \
         particular the Component → ComponentDefinition / ModuleDefinition \
         classification) is covered by unit tests \
         in `crates/sbol-convert/tests/downgrade_dual_role.rs` and \
         rather than this report, because doing so would require inventing \
         SBOL 3 fixtures from scratch.\n\n\
         For every fixture, the report runs:\n\n\
         1. SBOL 2 → SBOL 3 via `sbol_convert::upgrade_from_sbol2`\n\
         2. SBOL 3 validation (`Document::validate`)\n\
         3. SBOL 3 → SBOL 2 via `sbol_convert::downgrade`\n\
         4. SBOL 2 → SBOL 3 via `sbol_convert::upgrade_from_sbol2` (round-trip)\n\
         5. SBOL 3 re-validation\n\n\
         The triple set produced at step 1 is compared to the triple set produced \
         at step 4. Triples present in (1) but not (4) are **lost in round-trip**; \
         triples present in (4) but not (1) are **gained in round-trip**. Both \
         kinds of delta point at gaps in either the upgrade or the downgrade.\n\n\
         The report is informational. Conformance gating lives in \
         [`crates/sbol-convert/tests/upgrade_conformance.rs`](../crates/sbol-convert/tests/upgrade_conformance.rs) \
         and `crates/sbol-convert/tests/downgrade_round_trip.rs`. \
         The conversion model itself (the backport namespace, Component \
         classification, known divergences) is documented in \
         [`conversion.md`](conversion.md).\n\n",
    );

    let n = results.len();
    let clean = results
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Clean))
        .count();
    let lossy = results
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::LossyButValid { .. }))
        .count();
    let failed = results
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Failed { .. }))
        .count();

    out.push_str("## Summary\n\n");
    out.push_str(&format!("- {n} fixtures processed\n"));
    out.push_str(&format!(
        "- **{clean} clean** (round-trip preserves every triple)\n"
    ));
    out.push_str(&format!(
        "- **{lossy} lossy-but-valid** (round-trip completes; some triples differ)\n"
    ));
    out.push_str(&format!(
        "- **{failed} failed** (downgrade or re-upgrade did not complete)\n\n"
    ));

    out.push_str("## Per-fixture verdict\n\n");
    out.push_str(
        "| Fixture | Initial | RT | Lost | Gained | Warnings | Verdict |\n\
         |---|---:|---:|---:|---:|---:|---|\n",
    );
    for r in results {
        let (lost_n, gained_n, verdict) = match &r.outcome {
            Outcome::Clean => (0usize, 0usize, "clean".to_string()),
            Outcome::LossyButValid {
                gained,
                lost,
                new_validation_errors,
            } => {
                let v = if *new_validation_errors == 0 {
                    "lossy"
                } else {
                    "lossy + new errors"
                };
                (lost.len(), gained.len(), v.to_string())
            }
            Outcome::Failed { stage, .. } => (0, 0, format!("{stage} failed")),
        };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            r.name,
            r.initial_triples,
            r.roundtrip_triples,
            lost_n,
            gained_n,
            r.downgrade_warnings.len(),
            verdict,
        ));
    }
    out.push('\n');

    out.push_str("## Per-fixture details\n\n");
    for r in results {
        out.push_str(&format!("### `{}`\n\n", r.name));
        out.push_str(&format!(
            "- Initial: {} triples; {} validation error(s) after upgrade\n",
            r.initial_triples, r.initial_validation_errors
        ));
        out.push_str(&format!("- Round-trip: {} triples\n", r.roundtrip_triples));
        if !r.downgrade_warnings.is_empty() {
            out.push_str(&format!(
                "- Downgrade warnings ({}):\n",
                r.downgrade_warnings.len()
            ));
            for w in r.downgrade_warnings.iter().take(20) {
                out.push_str(&format!("  - `{}`\n", truncate(w, 240)));
            }
            if r.downgrade_warnings.len() > 20 {
                out.push_str(&format!(
                    "  - ... {} more\n",
                    r.downgrade_warnings.len() - 20
                ));
            }
        }
        match &r.outcome {
            Outcome::Clean => {
                out.push_str("\n**Round-trip preserves every triple.**\n\n");
            }
            Outcome::LossyButValid {
                gained,
                lost,
                new_validation_errors,
            } => {
                out.push_str(&format!(
                    "- New validation errors introduced by round-trip: **{new_validation_errors}**\n"
                ));
                if !lost.is_empty() {
                    let preview: Vec<_> = lost.iter().take(15).cloned().collect();
                    out.push_str(&format!(
                        "\n**Lost {} triple(s)** (showing first {}):\n\n```\n",
                        lost.len(),
                        preview.len()
                    ));
                    for t in preview {
                        out.push_str(&truncate(&t, 280));
                        out.push('\n');
                    }
                    if lost.len() > 15 {
                        out.push_str(&format!("... {} more\n", lost.len() - 15));
                    }
                    out.push_str("```\n");
                }
                if !gained.is_empty() {
                    let preview: Vec<_> = gained.iter().take(15).cloned().collect();
                    out.push_str(&format!(
                        "\n**Gained {} triple(s)** (showing first {}):\n\n```\n",
                        gained.len(),
                        preview.len()
                    ));
                    for t in preview {
                        out.push_str(&truncate(&t, 280));
                        out.push('\n');
                    }
                    if gained.len() > 15 {
                        out.push_str(&format!("... {} more\n", gained.len() - 15));
                    }
                    out.push_str("```\n");
                }
                out.push('\n');
            }
            Outcome::Failed { stage, message } => {
                out.push_str(&format!(
                    "\n**Failed at stage `{stage}`**:\n\n```\n{}\n```\n\n",
                    truncate(message, 1500)
                ));
            }
        }
    }

    out
}

fn main() -> ExitCode {
    let root = workspace_root();
    let fixtures_dir = root.join("tests/fixtures/sbol2/real");
    let expected_dir = root.join("tests/fixtures/sbol2/real/expected");

    let mut paths = match collect_fixture_paths(&fixtures_dir, &expected_dir) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!(
                "failed to read fixtures dir {}: {err}",
                fixtures_dir.display()
            );
            return ExitCode::from(2);
        }
    };
    paths.sort();

    println!(
        "running round-trip smoke test on {} fixture(s)...",
        paths.len()
    );

    let mut results: Vec<FixtureResult> = Vec::with_capacity(paths.len());
    for path in &paths {
        let name = fixture_name(path, &fixtures_dir);
        let result = run_fixture(path, &name);
        let badge = match &result.outcome {
            Outcome::Clean => "clean ",
            Outcome::LossyButValid { .. } => "lossy ",
            Outcome::Failed { .. } => "FAIL  ",
        };
        let extra = match &result.outcome {
            Outcome::Clean => String::new(),
            Outcome::LossyButValid { gained, lost, .. } => {
                format!(" (lost {} / gained {})", lost.len(), gained.len())
            }
            Outcome::Failed { stage, .. } => format!(" ({stage})"),
        };
        println!("  [{badge}] {name}{extra}");
        results.push(result);
    }

    let report = render_report(&results);
    let dest = root.join("docs/sbol3-round-trip-report.md");
    if let Err(err) = std::fs::write(&dest, &report) {
        eprintln!("write report {}: {err}", dest.display());
        return ExitCode::from(2);
    }
    println!("\nwrote {}", dest.display());
    ExitCode::SUCCESS
}
