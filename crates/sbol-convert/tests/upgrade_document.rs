//! Integration tests for the in-memory document upgrade: converting a parsed
//! `sbol2::Document` must produce exactly the SBOL 3 graph and report that
//! the text-based upgrade produces from the same input.

mod common;

use std::path::{Path, PathBuf};

use common::corpus::{CORPUS_DIRS, xml_files};

use sbol3::RdfFormat;

fn fixture_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("tests/fixtures/sbol2");
    path
}

/// The committed SBOL 2 fixtures: hand-authored Turtle at the top level and
/// real-world RDF/XML under `real/`.
fn committed_fixtures() -> Vec<(PathBuf, RdfFormat)> {
    let root = fixture_root();
    let mut fixtures = Vec::new();
    for (dir, extension, format) in [
        (root.clone(), "ttl", RdfFormat::Turtle),
        (root.join("real"), "xml", RdfFormat::RdfXml),
        (root.join("real/from_genbank"), "xml", RdfFormat::RdfXml),
        (root.join("real/synbiohub"), "xml", RdfFormat::RdfXml),
    ] {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("read {}: {err}", dir.display()))
            .map(|entry| entry.expect("directory entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == extension))
            .collect();
        paths.sort();
        fixtures.extend(paths.into_iter().map(|path| (path, format)));
    }
    fixtures
}

fn canonicalize(graph: &sbol3::RdfGraph) -> Vec<String> {
    let mut lines: Vec<String> = graph
        .normalized_triples()
        .iter()
        .map(sbol_convert::canonical_nt_line)
        .collect();
    lines.sort();
    lines.dedup();
    lines
}

fn read_input(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

#[test]
fn document_upgrade_matches_text_upgrade_on_committed_fixtures() {
    let fixtures = committed_fixtures();
    assert!(
        fixtures.len() >= 20,
        "expected the committed SBOL 2 fixtures, found {}",
        fixtures.len()
    );

    for (path, format) in fixtures {
        let input = read_input(&path);
        let (from_text, text_report) = sbol_convert::upgrade_from_sbol2(&input, format)
            .unwrap_or_else(|err| panic!("text upgrade {}: {err}", path.display()));

        let sbol2 = sbol2::Document::read(&input, format)
            .unwrap_or_else(|err| panic!("read SBOL 2 {}: {err}", path.display()));
        let (from_document, document_report) = sbol_convert::upgrade(&sbol2)
            .unwrap_or_else(|err| panic!("document upgrade {}: {err}", path.display()));

        assert_eq!(
            canonicalize(from_document.rdf_graph()),
            canonicalize(from_text.rdf_graph()),
            "{}: document and text upgrades disagree",
            path.display()
        );
        assert_eq!(
            document_report.warnings(),
            text_report.warnings(),
            "{}: document and text upgrades report different warnings",
            path.display()
        );
    }
}

#[test]
fn document_upgrade_with_applies_options() {
    let path = fixture_root().join("single_cd.ttl");
    let input = read_input(&path);
    let sbol2 = sbol2::Document::read(&input, RdfFormat::Turtle).expect("read SBOL 2");

    let options = sbol_convert::UpgradeOptions::default();
    let (with_options, _) = sbol_convert::upgrade_with(&sbol2, options.clone()).expect("upgrade");
    let (from_text, _) =
        sbol_convert::upgrade_from_sbol2_with(&input, RdfFormat::Turtle, options).expect("upgrade");

    assert_eq!(
        canonicalize(with_options.rdf_graph()),
        canonicalize(from_text.rdf_graph())
    );
}

#[test]
fn document_upgrade_produces_valid_sbol3() {
    let path = fixture_root().join("real/BBa_I0462.xml");
    let sbol2 = sbol2::Document::read(&read_input(&path), RdfFormat::RdfXml).expect("read SBOL 2");
    sbol2
        .check()
        .unwrap_or_else(|report| panic!("SBOL 2 validation failed: {report:?}"));

    let (upgraded, _report) = sbol_convert::upgrade(&sbol2).expect("upgrade");
    upgraded
        .check()
        .unwrap_or_else(|report| panic!("SBOL 3 validation failed: {report:?}"));
}

#[test]
fn document_upgrade_matches_text_upgrade_on_conformance_corpus() {
    let mut compared = 0;
    let mut mismatches = Vec::new();
    for dir in CORPUS_DIRS {
        for path in xml_files(dir) {
            let input = read_input(&path);
            // Files the reader cannot parse, or that the upgrade declines as
            // not SBOL 2, are covered by the corpus round-trip test.
            let Ok((from_text, text_report)) =
                sbol_convert::upgrade_from_sbol2(&input, RdfFormat::RdfXml)
            else {
                continue;
            };
            let sbol2 = sbol2::Document::read(&input, RdfFormat::RdfXml)
                .unwrap_or_else(|err| panic!("read SBOL 2 {}: {err}", path.display()));
            let (from_document, document_report) = sbol_convert::upgrade(&sbol2)
                .unwrap_or_else(|err| panic!("document upgrade {}: {err}", path.display()));
            compared += 1;
            if canonicalize(from_document.rdf_graph()) != canonicalize(from_text.rdf_graph())
                || document_report.warnings() != text_report.warnings()
            {
                mismatches.push(path.display().to_string());
            }
        }
    }
    assert!(
        compared > 250,
        "expected the SBOL 2 corpus, compared {compared}"
    );
    assert!(
        mismatches.is_empty(),
        "document and text upgrades disagree on {} of {compared} files: {mismatches:#?}",
        mismatches.len()
    );
}
