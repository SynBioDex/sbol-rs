//! Download a real iGEM part originally authored as GenBank, upgrade it to
//! SBOL 3 with `sbol-rs`, and summarize what came out.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p sbol-convert --example synbiohub_upgrade
//! cargo run -p sbol-convert --example synbiohub_upgrade -- BBa_F2620
//! ```
//!
//! ## Provenance pipeline
//!
//! 1. Most synthetic biology designs originate as **GenBank** files in
//!    tools like SnapGene, ApE, Benchling, or NCBI.
//! 2. When a part is deposited to the iGEM Registry, **SynBioHub** runs a
//!    server-side GenBank → SBOL 2 conversion and stores both
//!    representations.
//! 3. This example fetches the SBOL 2 representation via the public
//!    SynBioHub REST API, then runs `sbol-rs`'s
//!    `sbol_convert::upgrade_from_sbol2` to produce a native SBOL 3
//!    [`Document`].
//! 4. The resulting SBOL 3 graph is validated and structurally summarized.
//!
//! This example demonstrates the SBOL 2 → SBOL 3 leg specifically.
//! For the GenBank → SBOL 3 path (and the reverse), `sbol-rs` ships
//! pure-Rust converters in [`sbol-genbank`](../../sbol-genbank/) and
//! `sbol_convert` — no external services or Docker required.

use std::env;
use std::io::Read;
use std::time::Duration;

use sbol_convert::{UpgradeReport, UpgradeWarning};
use sbol3::{Document, RdfFormat, SbolIdentified, SbolTopLevel};

const SYNBIOHUB_BASE: &str = "https://synbiohub.org/public/igem";
const USER_AGENT: &str = "sbol-rs synbiohub_upgrade example";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let part_name = env::args()
        .nth(1)
        .unwrap_or_else(|| "BBa_E0040".to_string());
    let version = env::args().nth(2).unwrap_or_else(|| "1".to_string());

    println!("=== fetching {part_name} (version {version}) from SynBioHub ===");
    let sbol2_xml = fetch(&format!("{SYNBIOHUB_BASE}/{part_name}/{version}/sbol"))?;
    println!("  SBOL 2 RDF/XML: {} bytes\n", sbol2_xml.len());

    if let Ok(genbank) = fetch(&format!("{SYNBIOHUB_BASE}/{part_name}/{version}/gb")) {
        println!("=== GenBank source (server-converted to SBOL 2 above) ===");
        for line in genbank.lines().take(6) {
            println!("  {line}");
        }
        let total = genbank.lines().count();
        if total > 6 {
            println!("  …({} more lines)\n", total - 6);
        } else {
            println!();
        }
    }

    println!("=== upgrading SBOL 2 → SBOL 3 ===");
    let (document, report) = sbol_convert::upgrade_from_sbol2(&sbol2_xml, RdfFormat::RdfXml)?;
    print_upgrade_report(&report);

    println!("\n=== validating the converted SBOL 3 ===");
    let validation = document.validate();
    let issue_count = validation.issues().len();
    let error_count = validation
        .issues()
        .iter()
        .filter(|i| matches!(i.severity, sbol3::Severity::Error))
        .count();
    if error_count == 0 {
        println!("  document is valid ({issue_count} non-fatal issue(s))");
    } else {
        println!("  document has {error_count} validation error(s):");
        for issue in validation.issues().iter().take(5) {
            println!(
                "    [{:?}] {} — {}",
                issue.severity, issue.rule, issue.message
            );
        }
    }

    print_structural_summary(&document);
    print_components(&document);

    Ok(())
}

fn fetch(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(30)))
        .user_agent(USER_AGENT)
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut response = agent.get(url).call()?;
    let mut body = String::new();
    response.body_mut().as_reader().read_to_string(&mut body)?;
    Ok(body)
}

fn print_upgrade_report(report: &UpgradeReport) {
    let counts = report.counts();
    println!("  CD→Component:           {}", counts.component_definitions);
    println!("  MD→Component:           {}", counts.module_definitions);
    println!("  SubComponent:           {}", counts.sub_components);
    println!("  SequenceFeature:        {}", counts.sequence_features);
    println!(
        "  SA collapsed→SubComp:   {}",
        counts.sequence_annotations_collapsed
    );
    println!("  MapsTo decomposed:      {}", counts.mapstos_decomposed);
    println!(
        "  Interface synthesized:  {}",
        counts.interfaces_synthesized
    );
    println!(
        "  Location.hasSequence:   {}",
        counts.locations_with_inferred_sequence
    );

    if !report.warnings().is_empty() {
        println!("  warnings:");
        for warning in report.warnings() {
            println!("    {}", describe_warning(warning));
        }
    }
}

fn describe_warning(warning: &UpgradeWarning) -> String {
    match warning {
        UpgradeWarning::NamespaceFallback { subject, .. } => {
            format!("namespace fallback for <{subject}>")
        }
        UpgradeWarning::UnresolvedMapsTo { mapsto, side } => {
            format!("unresolved MapsTo <{mapsto}> ({side:?} side)")
        }
        UpgradeWarning::UnsupportedRefinement { mapsto, refinement } => {
            format!("MapsTo <{mapsto}>: unsupported refinement {refinement}")
        }
        UpgradeWarning::SequenceAnnotationWithComponent { annotation } => {
            format!("SA <{annotation}> referenced a Component")
        }
        UpgradeWarning::UnknownSbol2Type {
            subject,
            sbol2_type,
        } => {
            format!("subject <{subject}> has unrecognized SBOL 2 type <{sbol2_type}>")
        }
        UpgradeWarning::LocationWithoutSequence {
            location,
            component,
            sequence_count,
        } => format!(
            "<{location}> on <{component}>: no inferable hasSequence (owns {sequence_count})"
        ),
        _ => "unrecognized warning".to_string(),
    }
}

fn print_structural_summary(document: &Document) {
    println!("\n=== SBOL 3 document summary ===");
    println!(
        "  Components:               {}",
        document.components().count()
    );
    println!(
        "  Sequences:                {}",
        document.sequences().count()
    );
    println!(
        "  SubComponents:            {}",
        document.sub_components().count()
    );
    println!(
        "  SequenceFeatures:         {}",
        document.sequence_features().count()
    );
    println!(
        "  ComponentReferences:      {}",
        document.component_references().count()
    );
    println!(
        "  Constraints:              {}",
        document.constraints().count()
    );
    println!(
        "  Interactions:             {}",
        document.interactions().count()
    );
    println!(
        "  Participations:           {}",
        document.participations().count()
    );
    println!(
        "  Interfaces:               {}",
        document.interfaces().count()
    );
    println!("  Ranges:                   {}", document.ranges().count());
    println!("  Cuts:                     {}", document.cuts().count());
    println!("  Models:                   {}", document.models().count());
    println!(
        "  Collections:              {}",
        document.collections().count()
    );
    println!(
        "  CombinatorialDerivations: {}",
        document.combinatorial_derivations().count()
    );
    println!(
        "  Activities (PROV):        {}",
        document.activities().count()
    );
    println!("  Agents (PROV):            {}", document.agents().count());
    println!(
        "  total typed objects:      {}",
        document.typed_objects().len()
    );
}

fn print_components(document: &Document) {
    println!("\n=== Component detail ===");
    for component in document.components() {
        let identity = component
            .identity
            .as_iri()
            .map(|i| i.as_str())
            .unwrap_or("?");
        let namespace = component
            .namespace()
            .map(|ns| ns.as_str())
            .unwrap_or("(none)");
        let did = component.display_id().unwrap_or("?");
        let name = component.name().unwrap_or("(no name)");

        println!("\n  • {did} <{identity}>");
        println!("      namespace:       {namespace}");
        println!("      name:            {name}");
        if let Some(description) = component.description() {
            let short = if description.len() > 80 {
                format!("{}…", &description[..80])
            } else {
                description.to_string()
            };
            println!("      description:     {short}");
        }
        for type_iri in &component.types {
            println!("      type:            {}", short_iri(type_iri.as_str()));
        }
        for role in &component.roles {
            println!("      role:            {}", short_iri(role.as_str()));
        }
        if !component.sequences.is_empty() {
            println!("      hasSequence:     {}", component.sequences.len());
        }
        if !component.features.is_empty() {
            println!("      hasFeature:      {}", component.features.len());
        }
        if !component.interactions.is_empty() {
            println!("      hasInteraction:  {}", component.interactions.len());
        }
        if !component.interfaces.is_empty() {
            println!("      hasInterface:    {}", component.interfaces.len());
        }
    }
}

fn short_iri(iri: &str) -> String {
    if let Some(suffix) = iri.strip_prefix("https://identifiers.org/") {
        suffix.to_string()
    } else if let Some(suffix) = iri.strip_prefix("http://sbols.org/v3#") {
        format!("sbol3:{suffix}")
    } else {
        iri.to_string()
    }
}
