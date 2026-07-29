mod blocker;
mod context;
mod helpers;
mod options;
mod output;
mod report;
mod resolver;
mod rules;
mod spec;
mod tables;
mod validator;

pub use blocker::Blocker;
pub use context::{CachingHttpResolver, HttpResolver};
pub use context::{
    ContentResolver, DocumentResolver, DocumentSet, DocumentSetError, ExternalValidationMode,
    FileResolver, ResolutionError, ResolutionErrorKind, ResolvedContent, ValidationContext,
};
pub use options::{
    HashAlgorithmRegistry, PolicyOptions, RuleOverride, TopologyCompleteness, UnknownRule,
    ValidationConfig, ValidationOptions,
};
pub use output::{VALIDATION_OUTPUT_SCHEMA_VERSION, to_json};
pub use report::{
    AppliedOptions, CoverageKind, Hint, NotApplied, NotAppliedReason, PartialApplication,
    RuleCoverage, Severity, ValidationIssue, ValidationReport,
};
pub use spec::{
    NormativeSeverity, RuleStatus, VALIDATION_RULE_SPEC_CANONICAL_URL, VALIDATION_RULE_SPEC_PATH,
    VALIDATION_RULE_SPEC_PDF_SHA256, VALIDATION_RULE_SPEC_VERSION, ValidationGate,
    ValidationRuleStatus, validation_rule_statuses,
};

pub(crate) use spec::class_spec;
pub(crate) use validator::Validator;
