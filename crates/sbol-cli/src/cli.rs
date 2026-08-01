use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use sbol::v3::{ExternalValidationMode, RdfFormat, Severity};

#[derive(Parser)]
#[command(
    name = "sbol",
    version = env!("SBOL_VERSION_FULL"),
    about = "Command-line tool for SBOL documents",
    propagate_version = true
)]
pub(crate) struct Cli {
    /// When to colorize output. `auto` colorizes the streams that are
    /// TTYs and `NO_COLOR` is unset.
    #[arg(long, value_enum, default_value_t = ColorMode::Auto, global = true)]
    pub(crate) color: ColorMode,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Validate an SBOL 2 or SBOL 3 document against the spec.
    Validate(ValidateArgs),
    /// Compare two SBOL documents of the same version, object by identity.
    Diff(DiffArgs),
    /// Convert an SBOL 3 document between RDF serializations.
    Convert(ConvertArgs),
    /// Upgrade an SBOL 2 RDF document to SBOL 3.
    Upgrade(UpgradeArgs),
    /// Downgrade an SBOL 3 RDF document to SBOL 2.
    Downgrade(DowngradeArgs),
    /// Import a GenBank file (.gb / .gbk) into SBOL 3.
    ImportGenbank(ImportGenbankArgs),
    /// Import a FASTA file (.fasta / .fa / .fna / .faa) into SBOL 3.
    ImportFasta(ImportFastaArgs),
    /// Inspect the built-in validation rule catalog.
    #[command(subcommand)]
    Rules(RulesCommand),
    /// Manage cached extension ontologies (NCIT and others).
    #[command(subcommand)]
    Ontology(OntologyCommand),
    /// Authenticate, download, and publish designs in an SBOL registry.
    #[command(subcommand)]
    Registry(RegistryCommand),
}

#[derive(Subcommand)]
pub(crate) enum RegistryCommand {
    /// Sign in and store a registry token in the local SBOL profile.
    Login(RegistryLoginArgs),
    /// Revoke the active registry grant and remove it from the local profile.
    Logout(RegistryLogoutArgs),
    /// Download a design's recursive SBOL closure.
    Pull(RegistryPullArgs),
    /// Validate and upload a design as a private registry collection.
    Push(RegistryPushArgs),
    /// Show the identity and machine capabilities of a registry.
    Status(RegistryStatusArgs),
}

#[derive(Args)]
pub(crate) struct RegistryLogoutArgs {
    /// Registry base URL. Defaults to `SBOL_REGISTRY_URL`, then the active
    /// registry in the local SBOL profile.
    pub(crate) registry: Option<String>,
}

#[derive(Args)]
pub(crate) struct RegistryLoginArgs {
    /// Registry base URL. Defaults to `SBOL_REGISTRY_URL`, then
    /// `https://sbol.io`.
    pub(crate) registry: Option<String>,

    /// Account username or email for a compatibility password login. New SBOL
    /// Identity instances use browser login when this is omitted.
    #[arg(long)]
    pub(crate) identifier: Option<String>,

    /// Force compatibility password login and read the password from standard
    /// input. Intended for controlled automation and older registries; never
    /// pass a password as a command-line argument.
    #[arg(long)]
    pub(crate) password_stdin: bool,
}

#[derive(Args)]
pub(crate) struct RegistryPullArgs {
    /// The complete design IRI to download.
    pub(crate) iri: String,

    /// Destination path. The RDF serialization is inferred from its extension:
    /// `.ttl`, `.rdf` / `.xml`, `.jsonld`, or `.nt`.
    #[arg(long, short = 'o', value_name = "PATH")]
    pub(crate) output: PathBuf,

    /// Registry base URL. Defaults to `SBOL_REGISTRY_URL`, then to the origin of
    /// the design IRI. Required for registries mounted below a path prefix.
    #[arg(long, value_name = "URL")]
    pub(crate) registry: Option<String>,
}

#[derive(Args)]
pub(crate) struct RegistryStatusArgs {
    /// Registry base URL. Defaults to `SBOL_REGISTRY_URL`.
    pub(crate) registry: Option<String>,

    /// Print the complete registry metadata as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct RegistryPushArgs {
    /// SBOL, GenBank, or FASTA document to upload.
    pub(crate) input: PathBuf,

    /// Registry base URL. Defaults to `SBOL_REGISTRY_URL`.
    #[arg(long, value_name = "URL")]
    pub(crate) registry: Option<String>,

    /// Collection identifier. Defaults to a display-id-safe form of the input
    /// file stem.
    #[arg(long, value_name = "DISPLAY_ID")]
    pub(crate) id: Option<String>,

    /// Version assigned to the minted collection and its members.
    #[arg(long = "collection-version", default_value = "1")]
    pub(crate) collection_version: String,

    /// Human-readable collection title.
    #[arg(long)]
    pub(crate) name: Option<String>,

    /// Human-readable collection description.
    #[arg(long)]
    pub(crate) description: Option<String>,

    /// PubMed citation identifier. Repeat for multiple citations.
    #[arg(long = "citation")]
    pub(crate) citations: Vec<String>,

    /// Creator name stamped on the collection.
    #[arg(long)]
    pub(crate) creator_name: Option<String>,

    /// Explicit behavior when the target collection identity already exists.
    #[arg(long, value_enum, default_value_t = RegistryCollisionPolicy::Fail)]
    pub(crate) collision: RegistryCollisionPolicy,

    /// Run server-side validation, identity minting, and collision analysis
    /// without writing anything.
    #[arg(long)]
    pub(crate) preview: bool,

    /// Print the preview and commit result as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub(crate) enum RegistryCollisionPolicy {
    #[default]
    Fail,
    Replace,
    Merge,
}

#[derive(Subcommand)]
pub(crate) enum RulesCommand {
    /// List validation rules, their implementation status, and spec section.
    List(RulesListArgs),
}

#[derive(Subcommand)]
pub(crate) enum OntologyCommand {
    /// Download and build a named ontology extension into the cache.
    Install(OntologyInstallArgs),
    /// List installed ontology extensions.
    List,
    /// Print the cache directory path.
    Path,
    /// Remove an installed ontology extension.
    Remove(OntologyRemoveArgs),
    /// Re-hash an installed extension's TSV and compare against its
    /// manifest. Errors if the extension is missing or tampered with.
    Verify(OntologyVerifyArgs),
}

#[derive(Args)]
pub(crate) struct OntologyInstallArgs {
    /// Built-in ontology to install. Currently: `ncit`.
    pub(crate) name: String,
    /// Re-download and rebuild even if already installed.
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Args)]
pub(crate) struct OntologyRemoveArgs {
    /// Cache entry name (e.g. `ncit`).
    pub(crate) name: String,
}

#[derive(Args)]
pub(crate) struct OntologyVerifyArgs {
    /// Cache entry name to verify. If omitted, every installed
    /// extension is verified.
    pub(crate) name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Json,
    Sarif,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum RulesFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum DiffFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum RuleStatusFilter {
    Error,
    Warning,
    Configurable,
    MachineUncheckable,
    Unimplemented,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum RdfFormatArg {
    Turtle,
    Rdfxml,
    Jsonld,
    Ntriples,
}

impl From<RdfFormatArg> for RdfFormat {
    fn from(value: RdfFormatArg) -> Self {
        match value {
            RdfFormatArg::Turtle => RdfFormat::Turtle,
            RdfFormatArg::Rdfxml => RdfFormat::RdfXml,
            RdfFormatArg::Jsonld => RdfFormat::JsonLd,
            RdfFormatArg::Ntriples => RdfFormat::NTriples,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum SeverityArg {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ExternalModeArg {
    Off,
    Provided,
    Allowed,
}

impl From<ExternalModeArg> for ExternalValidationMode {
    fn from(value: ExternalModeArg) -> Self {
        match value {
            ExternalModeArg::Off => ExternalValidationMode::Off,
            ExternalModeArg::Provided => ExternalValidationMode::ProvidedOnly,
            ExternalModeArg::Allowed => ExternalValidationMode::ExternalAllowed,
        }
    }
}

impl From<SeverityArg> for Severity {
    fn from(value: SeverityArg) -> Self {
        match value {
            SeverityArg::Warning => Severity::Warning,
            SeverityArg::Error => Severity::Error,
        }
    }
}

/// Which SBOL major version to validate the input as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum SbolVersionArg {
    /// Detect the version from the document's RDF namespaces.
    Auto,
    /// Force the SBOL 2 validator.
    #[value(name = "2")]
    V2,
    /// Force the SBOL 3 validator.
    #[value(name = "3")]
    V3,
}

/// Which SBOL validation-rule catalog to list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum CatalogVersionArg {
    /// The SBOL 2 catalog.
    #[value(name = "2")]
    V2,
    /// The SBOL 3 catalog.
    #[value(name = "3")]
    V3,
}

#[derive(Args)]
pub(crate) struct ValidateArgs {
    /// Path to an SBOL 2 or SBOL 3 document. Format is inferred from the
    /// extension: `.ttl` (Turtle), `.rdf` / `.xml` (RDF/XML), `.jsonld`
    /// (JSON-LD), or `.nt` (N-Triples).
    pub(crate) path: PathBuf,

    /// Which SBOL version to validate as. `auto` detects the version from
    /// the document's RDF namespaces; `2` and `3` force a validator.
    #[arg(long, value_enum, default_value_t = SbolVersionArg::Auto)]
    pub(crate) sbol_version: SbolVersionArg,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub(crate) format: OutputFormat,

    /// Destination for output. Use `-` for stdout.
    #[arg(long, default_value = "-")]
    pub(crate) output: String,

    /// Suppress diagnostics for these rule IDs (e.g. `--allow sbol3-10502`).
    #[arg(long = "allow", value_name = "RULE_ID")]
    pub(crate) allow: Vec<String>,

    /// Promote these rule IDs to error severity.
    #[arg(long = "deny", value_name = "RULE_ID")]
    pub(crate) deny: Vec<String>,

    /// Demote these rule IDs to warning severity.
    #[arg(long = "warn", value_name = "RULE_ID")]
    pub(crate) warn: Vec<String>,

    /// Floor on the severity of any emitted issue.
    #[arg(long, value_enum)]
    pub(crate) severity_floor: Option<SeverityArg>,

    /// Ceiling on the severity of any emitted issue.
    #[arg(long, value_enum)]
    pub(crate) severity_ceiling: Option<SeverityArg>,

    /// Treat warnings as errors (alias for `--severity-floor error`).
    #[arg(long)]
    pub(crate) treat_warnings_as_errors: bool,

    /// Use `Document::check_complete` semantics: any rule with partial
    /// coverage causes exit code 3.
    #[arg(long)]
    pub(crate) treat_partial_as_errors: bool,

    /// In text output, print a coverage summary after the issues.
    #[arg(long)]
    pub(crate) show_coverage: bool,

    /// Whether to resolve external documents and content.
    #[arg(long, value_enum, default_value_t = ExternalModeArg::Off)]
    pub(crate) external_mode: ExternalModeArg,

    /// Filesystem roots from which external Attachment / Model / TopLevel
    /// references may be resolved.
    #[arg(long = "resolve-documents", value_name = "DIR")]
    pub(crate) resolve_documents: Vec<PathBuf>,

    /// Filesystem roots for Attachment / Model byte content.
    #[arg(long = "resolve-content", value_name = "DIR")]
    pub(crate) resolve_content: Vec<PathBuf>,

    /// Cache directory required by `--external-mode allowed`.
    #[arg(long)]
    pub(crate) cache_dir: Option<PathBuf>,

    /// Layer an installed runtime ontology extension on top of the bundled
    /// facts for this validation run. Pass the cache entry name (e.g.
    /// `--ontology ncit`). Repeatable; later extensions override earlier
    /// ones on conflict.
    #[arg(long = "ontology", value_name = "NAME")]
    pub(crate) ontology: Vec<String>,

    /// Run the completeness family: every referenced object must be present
    /// in the document. On by default for both versions.
    #[arg(long, conflicts_with = "incomplete")]
    pub(crate) complete: bool,

    /// Skip the completeness family, so references to objects outside the
    /// document are not flagged. The counterpart to `--complete`.
    #[arg(long)]
    pub(crate) incomplete: bool,

    /// Skip the compliant-URI structural family (SBOL 2's compliant-URI
    /// checks, SBOL 3's structural URI checks).
    #[arg(long)]
    pub(crate) non_compliant: bool,

    /// Run the SHOULD-level best-practice family. On by default for SBOL 3;
    /// use this to opt in for SBOL 2.
    #[arg(long, conflicts_with = "no_best_practices")]
    pub(crate) best_practice: bool,

    /// Skip the SHOULD-level best-practice family and report only MUST
    /// violations. On by default for SBOL 2; use this to opt out for SBOL 3.
    #[arg(long)]
    pub(crate) no_best_practices: bool,

    /// Interpret compliant URIs as carrying an optional type segment.
    #[arg(long)]
    pub(crate) types_in_uri: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum UpgradeReportFormat {
    None,
    Text,
    Json,
}

#[derive(Args)]
pub(crate) struct UpgradeArgs {
    /// Path to an SBOL 2 RDF document. Input format is inferred from the
    /// extension: `.ttl` (Turtle), `.rdf` / `.xml` (RDF/XML), `.jsonld`
    /// (JSON-LD), or `.nt` (N-Triples). Use `--from` to override.
    pub(crate) path: PathBuf,

    /// Override the input format inference. Useful for SBOL 2 files
    /// distributed with non-standard extensions.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub(crate) from: Option<RdfFormatArg>,

    /// Target SBOL 3 RDF serialization. If omitted, inferred from
    /// `--output`'s extension.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub(crate) to: Option<RdfFormatArg>,

    /// Destination path for the SBOL 3 output. Use `-` (the default) for
    /// stdout; in that case `--to` is required.
    #[arg(long, short = 'o', default_value = "-")]
    pub(crate) output: String,

    /// Default `hasNamespace` value for top-level objects whose namespace
    /// cannot be derived from the input. Without this flag, such objects
    /// fall back to the URL scheme+host or, failing that, omit
    /// `hasNamespace` entirely.
    #[arg(long, value_name = "IRI")]
    pub(crate) namespace: Option<String>,

    /// Where to write the conversion report.
    #[arg(long, value_enum, default_value_t = UpgradeReportFormat::None)]
    pub(crate) report: UpgradeReportFormat,

    /// Exit with status 1 if any conversion warnings were produced.
    #[arg(long)]
    pub(crate) strict: bool,

    /// Run SBOL 3 validation on the converted document and fold the result
    /// into the exit code: code 1 if validation finds errors.
    #[arg(long)]
    pub(crate) validate: bool,
}

#[derive(Args)]
pub(crate) struct DowngradeArgs {
    /// Path to an SBOL 3 RDF document. Input format is inferred from
    /// the extension (`.ttl`, `.rdf` / `.xml`, `.jsonld`, `.nt`).
    pub(crate) path: PathBuf,

    /// Override the input format inference. Useful for SBOL 3 RDF files
    /// distributed with non-standard extensions.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub(crate) from: Option<RdfFormatArg>,

    /// Target SBOL 2 RDF serialization. If omitted, inferred from
    /// `--output`'s extension.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub(crate) to: Option<RdfFormatArg>,

    /// Destination path. Use `-` (the default) for stdout; in that
    /// case `--to` is required.
    #[arg(long, short = 'o', default_value = "-")]
    pub(crate) output: String,

    /// Version string assigned to top-level objects whose SBOL 3 IRI
    /// carried no version segment. Omit to leave such subjects
    /// unversioned (SBOL 2 makes `sbol2:version` optional); pass
    /// `--default-version 1` to match the libSBOLj / SynBioHub
    /// convention of always emitting one.
    #[arg(long, value_name = "VERSION")]
    pub(crate) default_version: Option<String>,

    /// Validate the downgrade by round-tripping the produced SBOL 2
    /// back up through `sbol::v3::upgrade` and running SBOL 3 validation
    /// on the result. There is no native SBOL 2 validator in this
    /// workspace, so this round-trip is the proxy for structural
    /// correctness. Exit code 1 on validation errors.
    #[arg(long)]
    pub(crate) validate: bool,

    /// Exit with status 1 if any downgrade warnings were produced.
    #[arg(long)]
    pub(crate) strict: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum FastaAlphabetArg {
    Dna,
    Rna,
    Protein,
}

impl From<FastaAlphabetArg> for sbol_fasta::Alphabet {
    fn from(value: FastaAlphabetArg) -> Self {
        match value {
            FastaAlphabetArg::Dna => sbol_fasta::Alphabet::Dna,
            FastaAlphabetArg::Rna => sbol_fasta::Alphabet::Rna,
            FastaAlphabetArg::Protein => sbol_fasta::Alphabet::Protein,
        }
    }
}

#[derive(Args)]
pub(crate) struct ImportFastaArgs {
    /// Path to a FASTA file (`.fasta` / `.fa` / `.fna` / `.faa`).
    pub(crate) path: PathBuf,

    /// Namespace IRI under which the resulting SBOL 3 top-level
    /// objects will be rooted. Required because FASTA carries no
    /// namespace concept.
    #[arg(long, short = 'n', value_name = "IRI")]
    pub(crate) namespace: String,

    /// Override alphabet auto-detection. Pass when the sequence text
    /// is ambiguous (e.g. a short peptide composed only of A/C/G/T
    /// letters that would otherwise be misclassified as DNA).
    #[arg(long, value_enum, value_name = "ALPHABET")]
    pub(crate) alphabet: Option<FastaAlphabetArg>,

    /// Target SBOL 3 RDF serialization. If omitted, inferred from
    /// `--output`'s extension.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub(crate) to: Option<RdfFormatArg>,

    /// Destination path. Use `-` (the default) for stdout; in that
    /// case `--to` is required.
    #[arg(long, short = 'o', default_value = "-")]
    pub(crate) output: String,

    /// Run SBOL 3 validation on the converted document and fold the
    /// result into the exit code: code 1 if validation finds errors.
    #[arg(long)]
    pub(crate) validate: bool,

    /// Exit with status 1 if any import warnings were produced.
    #[arg(long)]
    pub(crate) strict: bool,
}

#[derive(Args)]
pub(crate) struct ImportGenbankArgs {
    /// Path to a GenBank flat-file (`.gb` / `.gbk`). Mixed-case month
    /// names in the LOCUS line (as emitted by SynBioHub) are tolerated.
    pub(crate) path: PathBuf,

    /// Namespace IRI under which the resulting SBOL 3 top-level
    /// objects will be rooted. Required because GenBank carries no
    /// namespace concept.
    #[arg(long, short = 'n', value_name = "IRI")]
    pub(crate) namespace: String,

    /// Target SBOL 3 RDF serialization. If omitted, inferred from
    /// `--output`'s extension.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub(crate) to: Option<RdfFormatArg>,

    /// Destination path. Use `-` (the default) for stdout; in that
    /// case `--to` is required.
    #[arg(long, short = 'o', default_value = "-")]
    pub(crate) output: String,

    /// Run SBOL 3 validation on the converted document and fold the
    /// result into the exit code: code 1 if validation finds errors.
    #[arg(long)]
    pub(crate) validate: bool,

    /// Exit with status 1 if any import warnings were produced.
    #[arg(long)]
    pub(crate) strict: bool,
}

#[derive(Args)]
pub(crate) struct ConvertArgs {
    /// Path to an SBOL 3 document. Input format is inferred from the
    /// extension: `.ttl` (Turtle), `.rdf` (RDF/XML), `.jsonld` (JSON-LD),
    /// or `.nt` (N-Triples).
    pub(crate) path: PathBuf,

    /// Target serialization. If omitted, inferred from `--output`'s
    /// extension.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub(crate) to: Option<RdfFormatArg>,

    /// Destination path. Use `-` (the default) for stdout; in that case
    /// `--to` is required.
    #[arg(long, short = 'o', default_value = "-")]
    pub(crate) output: String,
}

#[derive(Args)]
pub(crate) struct DiffArgs {
    /// Path to the old (baseline) document. Format is inferred from the
    /// extension: `.ttl`, `.rdf` / `.xml`, `.jsonld`, or `.nt`.
    pub(crate) old: PathBuf,

    /// Path to the new (revised) document, in the same SBOL version as
    /// `old`.
    pub(crate) new: PathBuf,

    /// Which SBOL version to read both documents as. `auto` detects the
    /// version from each document's RDF namespaces and requires the two to
    /// match; `2` and `3` force a reader.
    #[arg(long, value_enum, default_value_t = SbolVersionArg::Auto)]
    pub(crate) sbol_version: SbolVersionArg,

    /// Output format.
    #[arg(long, value_enum, default_value_t = DiffFormat::Text)]
    pub(crate) format: DiffFormat,

    /// Destination for output. Use `-` (the default) for stdout.
    #[arg(long, short = 'o', default_value = "-")]
    pub(crate) output: String,

    /// Exit with status 1 when the documents differ (status 0 when they are
    /// identical), so the command can gate a script or CI check.
    #[arg(long)]
    pub(crate) exit_code: bool,
}

#[derive(Args)]
pub(crate) struct RulesListArgs {
    /// Which catalog to list: the SBOL 2 rules or the SBOL 3 rules.
    #[arg(long, value_enum, default_value_t = CatalogVersionArg::V3)]
    pub(crate) sbol_version: CatalogVersionArg,

    /// Output format.
    #[arg(long, value_enum, default_value_t = RulesFormat::Text)]
    pub(crate) format: RulesFormat,

    /// Only show rules with this implementation status.
    #[arg(long, value_enum, value_name = "STATUS")]
    pub(crate) status: Option<RuleStatusFilter>,

    /// Show full notes instead of truncating to fit one line per rule.
    #[arg(long)]
    pub(crate) full: bool,
}
