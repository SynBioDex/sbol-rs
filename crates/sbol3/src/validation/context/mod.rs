use sbol_ontology::{Ontology, OntologyRegistry};

use crate::Document;
use crate::validation::options::ValidationOptions;

mod resolvers;

pub use resolvers::FileResolver;
pub use resolvers::{CachingHttpResolver, HttpResolver};
pub use sbol_core::document::DocumentSetError;
pub use sbol_core::validation::options::ExternalValidationMode;
pub use sbol_core::validation::resolver::{
    ContentResolver, DocumentResolver, ResolutionError, ResolutionErrorKind, ResolvedContent,
};

/// A set of in-memory SBOL documents indexed by object identity.
pub type DocumentSet<'a> = sbol_core::document::DocumentSet<'a, Document>;

/// Resolver-aware validation inputs.
#[derive(Default)]
#[non_exhaustive]
pub struct ValidationContext<'a> {
    options: ValidationOptions,
    ontology_registry: OntologyRegistry,
    external_mode: ExternalValidationMode,
    documents: Vec<&'a Document>,
    document_resolvers: Vec<&'a dyn DocumentResolver>,
    content_resolvers: Vec<&'a dyn ContentResolver>,
}

impl<'a> ValidationContext<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(mut options: ValidationOptions) -> Self {
        let extensions = options.take_ontology_extensions();
        let ontology_registry = if extensions.is_empty() {
            OntologyRegistry::bundled_only()
        } else {
            OntologyRegistry::bundled_with(extensions)
        };
        Self {
            options,
            ontology_registry,
            ..Self::default()
        }
    }

    pub fn options(&self) -> &ValidationOptions {
        &self.options
    }

    /// Returns the ontology view used by validation. Bundled-only by default;
    /// extension snapshots layered in via
    /// [`ValidationOptions::with_ontology_extension`] are merged on top.
    pub fn ontology(&self) -> &Ontology {
        self.ontology_registry.ontology()
    }

    /// Returns the underlying ontology registry.
    pub fn ontology_registry(&self) -> &OntologyRegistry {
        &self.ontology_registry
    }

    pub fn external_mode(&self) -> ExternalValidationMode {
        self.external_mode
    }

    pub fn documents(&self) -> &[&'a Document] {
        &self.documents
    }

    pub fn document_resolvers(&self) -> &[&'a dyn DocumentResolver] {
        &self.document_resolvers
    }

    pub fn content_resolvers(&self) -> &[&'a dyn ContentResolver] {
        &self.content_resolvers
    }

    pub fn with_external_mode(mut self, external_mode: ExternalValidationMode) -> Self {
        self.external_mode = external_mode;
        self
    }

    pub fn with_document(mut self, document: &'a Document) -> Self {
        self.documents.push(document);
        self
    }

    pub fn with_document_resolver(mut self, resolver: &'a dyn DocumentResolver) -> Self {
        self.document_resolvers.push(resolver);
        self
    }

    pub fn with_content_resolver(mut self, resolver: &'a dyn ContentResolver) -> Self {
        self.content_resolvers.push(resolver);
        self
    }
}
