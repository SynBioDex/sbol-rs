use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use sbol::v3::CachingHttpResolver;
use sbol::v3::{
    Document, FileResolver, RdfFormat, Severity, ValidationConfig, ValidationContext,
    ValidationOptions, ValidationReport,
};
use sbol::{SbolVersion, detect_version};
use sbol_ontology::OntologyCache;

use crate::cli::{ExternalModeArg, OutputFormat, SbolVersionArg, ValidateArgs};
use crate::commands::ontology::known_ontology_by_name;
use crate::output::{format_issue, infer_conversion_rdf_format, plural};
use crate::style::{Styles, paint};

pub(crate) fn validate(args: ValidateArgs, styles: Styles) -> ExitCode {
    let input = match fs::read_to_string(&args.path) {
        Ok(text) => text,
        Err(source) => {
            eprintln!(
                "{}: failed to read {}: {source}",
                styles.err_label(),
                args.path.display()
            );
            return ExitCode::from(2);
        }
    };

    let Some(format) = infer_conversion_rdf_format(&args.path) else {
        let ext = args
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("<none>");
        eprintln!(
            "{}: unsupported extension `{ext}` for {} — supported: .ttl, .rdf, .xml, .jsonld, .nt",
            styles.err_label(),
            args.path.display()
        );
        return ExitCode::from(2);
    };

    match resolve_version(&args, &input, format) {
        SbolVersion::V2 => validate_v2(&args, &input, format, styles),
        _ => validate_v3(&args, &input, format, styles),
    }
}

/// Picks the SBOL version to validate as. An explicit `--sbol-version`
/// wins; otherwise the RDF namespaces decide, falling back to whichever
/// parser accepts the input and finally to SBOL 3 so its parser reports
/// the error.
fn resolve_version(args: &ValidateArgs, input: &str, format: RdfFormat) -> SbolVersion {
    match args.sbol_version {
        SbolVersionArg::V2 => SbolVersion::V2,
        SbolVersionArg::V3 => SbolVersion::V3,
        SbolVersionArg::Auto => detect_version(input, format).unwrap_or_else(|| {
            if Document::read(input, format).is_ok() {
                SbolVersion::V3
            } else if sbol::v2::Document::read(input, format).is_ok() {
                SbolVersion::V2
            } else {
                SbolVersion::V3
            }
        }),
    }
}

fn validate_v3(args: &ValidateArgs, input: &str, format: RdfFormat, styles: Styles) -> ExitCode {
    let options = match build_v3_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };

    if args.external_mode == ExternalModeArg::Allowed && args.cache_dir.is_none() {
        eprintln!(
            "{}: --external-mode allowed requires --cache-dir (so HTTP fetches stay deterministic)",
            styles.err_label()
        );
        return ExitCode::from(2);
    }

    let document = match Document::read(input, format) {
        Ok(document) => document,
        Err(error) => {
            eprintln!(
                "{}: failed to parse {} as {format}: {error}",
                styles.err_label(),
                args.path.display()
            );
            return ExitCode::from(2);
        }
    };

    let document_resolver = build_document_resolver(args);
    let content_resolver = build_content_resolver(args);
    let caching_http = args
        .cache_dir
        .as_ref()
        .filter(|_| args.external_mode == ExternalModeArg::Allowed)
        .map(|dir| CachingHttpResolver::new(dir.clone()));

    let mut context =
        ValidationContext::with_options(options).with_external_mode(args.external_mode.into());
    if let Some(resolver) = &document_resolver {
        context = context.with_document_resolver(resolver);
    }
    if let Some(resolver) = &content_resolver {
        context = context.with_content_resolver(resolver);
    }
    if let Some(resolver) = &caching_http {
        let doc_ref: &dyn sbol::v3::DocumentResolver = resolver;
        let content_ref: &dyn sbol::v3::ContentResolver = resolver;
        context = context
            .with_content_resolver(content_ref)
            .with_document_resolver(doc_ref);
    }

    let report = document.validate_with_context(context);
    finish(args, &report, styles)
}

fn validate_v2(args: &ValidateArgs, input: &str, format: RdfFormat, styles: Styles) -> ExitCode {
    warn_sbol3_only_flags(args, styles);

    let options = match build_v2_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{}: {message}", styles.err_label());
            return ExitCode::from(2);
        }
    };

    let document = match sbol::v2::Document::read(input, format) {
        Ok(document) => document,
        Err(error) => {
            eprintln!(
                "{}: failed to parse {} as {format}: {error}",
                styles.err_label(),
                args.path.display()
            );
            return ExitCode::from(2);
        }
    };

    let report = document.validate_with(options);
    finish(args, &report, styles)
}

/// Applies the shared exit-code convention: `1` on errors, `3` when
/// `--treat-partial-as-errors` and a rule was only partially applied,
/// `0` otherwise. Rendering failures surface as `2`.
fn finish(args: &ValidateArgs, report: &ValidationReport, styles: Styles) -> ExitCode {
    if let Err(message) = render_output(args, report, styles) {
        eprintln!("{}: failed to write output: {message}", styles.err_label());
        return ExitCode::from(2);
    }

    let has_errors = report.has_errors();
    let has_partial = !report.coverage().partially_applied.is_empty();
    if has_errors {
        ExitCode::from(1)
    } else if args.treat_partial_as_errors && has_partial {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    }
}

/// Warns when SBOL 3-only knobs are set against an SBOL 2 document. The
/// SBOL 2 validator has no resolver/external stage, no severity floor or
/// ceiling, and no runtime ontology extensions, so these are ignored.
fn warn_sbol3_only_flags(args: &ValidateArgs, styles: Styles) {
    let mut ignored: Vec<&str> = Vec::new();
    if args.severity_floor.is_some() {
        ignored.push("--severity-floor");
    }
    if args.severity_ceiling.is_some() {
        ignored.push("--severity-ceiling");
    }
    if args.treat_warnings_as_errors {
        ignored.push("--treat-warnings-as-errors");
    }
    if args.external_mode != ExternalModeArg::Off {
        ignored.push("--external-mode");
    }
    if !args.resolve_documents.is_empty() {
        ignored.push("--resolve-documents");
    }
    if !args.resolve_content.is_empty() {
        ignored.push("--resolve-content");
    }
    if args.cache_dir.is_some() {
        ignored.push("--cache-dir");
    }
    if !args.ontology.is_empty() {
        ignored.push("--ontology");
    }
    if !ignored.is_empty() {
        eprintln!(
            "{}: {} {} SBOL 3-only and ignored for an SBOL 2 document",
            styles.warn_label(),
            ignored.join(", "),
            if ignored.len() == 1 { "is" } else { "are" },
        );
    }
}

fn build_document_resolver(args: &ValidateArgs) -> Option<FileResolver> {
    if args.resolve_documents.is_empty() {
        return None;
    }
    let mut resolver = FileResolver::new();
    for root in &args.resolve_documents {
        resolver.add_root(root.clone());
    }
    Some(resolver)
}

fn build_content_resolver(args: &ValidateArgs) -> Option<FileResolver> {
    if args.resolve_content.is_empty() {
        return None;
    }
    let mut resolver = FileResolver::new();
    for root in &args.resolve_content {
        resolver.add_root(root.clone());
    }
    Some(resolver)
}

/// Overlays the completeness / compliant-URI / best-practice / types-in-URI
/// flags onto a starting [`ValidationConfig`]. Only flags that were passed
/// change a field, so each version's family defaults survive untouched.
fn apply_config_flags(config: &mut ValidationConfig, args: &ValidateArgs) {
    if args.complete {
        config.complete = true;
    }
    if args.incomplete {
        config.complete = false;
    }
    if args.non_compliant {
        config.compliant = false;
    }
    if args.best_practice {
        config.best_practice = true;
    }
    if args.no_best_practices {
        config.best_practice = false;
    }
    if args.types_in_uri {
        config.types_in_uri = true;
    }
}

fn build_v3_options(args: &ValidateArgs) -> Result<ValidationOptions, String> {
    let mut options = ValidationOptions::default();
    apply_config_flags(&mut options.config, args);

    let mut configured: BTreeSet<&str> = BTreeSet::new();
    for rule in &args.allow {
        check_first_use(&mut configured, rule)?;
        options = options.allow(rule).map_err(|err| err.to_string())?;
    }
    for rule in &args.deny {
        check_first_use(&mut configured, rule)?;
        options = options.deny(rule).map_err(|err| err.to_string())?;
    }
    for rule in &args.warn {
        check_first_use(&mut configured, rule)?;
        options = options.warn(rule).map_err(|err| err.to_string())?;
    }

    if let Some(floor) = args.severity_floor {
        options = options.with_severity_floor(floor.into());
    }
    if let Some(ceiling) = args.severity_ceiling {
        options = options.with_severity_ceiling(ceiling.into());
    }
    if args.treat_warnings_as_errors {
        options = options.with_severity_floor(Severity::Error);
    }

    if !args.ontology.is_empty() {
        let cache = OntologyCache::from_default_path();
        for name in &args.ontology {
            let extension = cache.load(name).map_err(|error| {
                if known_ontology_by_name(name).is_some() {
                    format!(
                        "failed to load ontology extension `{name}` from {}: {error}. \
                         Install it first with `sbol ontology install {name}`.",
                        cache.path().display(),
                    )
                } else {
                    format!(
                        "unknown ontology extension `{name}` — try one of: ncit \
                         (run `sbol ontology install <name>` to install it)"
                    )
                }
            })?;
            options = options.with_ontology_extension(extension);
        }
    }
    Ok(options)
}

fn build_v2_options(
    args: &ValidateArgs,
) -> Result<sbol::v2::validation::ValidationOptions, String> {
    let mut config = ValidationConfig::default();
    apply_config_flags(&mut config, args);
    let mut options = sbol::v2::validation::ValidationOptions::default().with_config(config);

    let mut configured: BTreeSet<&str> = BTreeSet::new();
    for rule in &args.allow {
        check_first_use(&mut configured, rule)?;
        options = options.allow(rule).map_err(|err| err.to_string())?;
    }
    for rule in &args.deny {
        check_first_use(&mut configured, rule)?;
        options = options.deny(rule).map_err(|err| err.to_string())?;
    }
    for rule in &args.warn {
        check_first_use(&mut configured, rule)?;
        options = options.warn(rule).map_err(|err| err.to_string())?;
    }
    Ok(options)
}

fn check_first_use<'a>(configured: &mut BTreeSet<&'a str>, rule: &'a str) -> Result<(), String> {
    if !configured.insert(rule) {
        return Err(format!(
            "rule `{rule}` is given more than one override on the command line"
        ));
    }
    Ok(())
}

fn render_output(args: &ValidateArgs, report: &ValidationReport, styles: Styles) -> io::Result<()> {
    let writing_to_stdout = args.output == "-";
    let payload = match args.format {
        OutputFormat::Text => {
            let color = styles.stdout && writing_to_stdout;
            format_text(args, report, color)
        }
        OutputFormat::Json => sbol::v3::to_json(report),
        OutputFormat::Sarif => crate::sarif::to_sarif(report, &args.path),
    };

    if writing_to_stdout {
        let mut stdout = io::stdout().lock();
        stdout.write_all(payload.as_bytes())?;
        if !payload.ends_with('\n') {
            stdout.write_all(b"\n")?;
        }
        Ok(())
    } else {
        fs::write(&args.output, payload)
    }
}

fn format_text(args: &ValidateArgs, report: &ValidationReport, color: bool) -> String {
    let mut out = String::new();
    for issue in report.issues() {
        out.push_str(&format_issue(issue, &args.path, color));
        out.push('\n');
    }

    let errors = report.errors().count();
    let warnings = report.warnings().count();
    out.push_str(&format!(
        "{}: {errors} error{}, {warnings} warning{}",
        args.path.display(),
        plural(errors),
        plural(warnings),
    ));
    if errors == 0 && warnings == 0 {
        out.push_str(&format!(" {}", paint(color, "1;32", "— OK")));
    }
    out.push('\n');

    if args.show_coverage {
        let coverage = report.coverage();
        let line = format!(
            "coverage: {} fully applied, {} partially applied, {} not applied\n",
            coverage.fully_applied.len(),
            coverage.partially_applied.len(),
            coverage.not_applied.len(),
        );
        out.push_str(&paint(color, "2", &line));
    }

    out
}
