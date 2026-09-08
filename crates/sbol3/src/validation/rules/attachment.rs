use crate::object::ObjectClasses;
use sha3::{Digest, Sha3_256};

use crate::validation::context::{
    ExternalValidationMode, ResolutionError, ResolutionErrorKind, ResolvedContent,
};
use crate::validation::helpers::{
    hex_digest, is_hash_algorithm_token, is_hex_digest, is_known_hash_algorithm,
};
use crate::validation::tables;
use crate::validation::validator::Validator;
use crate::vocab::*;
use crate::{Iri, Object, SbolClass};

impl<'a> Validator<'a> {
    pub(crate) fn validate_model(&mut self, object: &Object) {
        if let Some(source) = object.first_iri(SBOL_SOURCE) {
            self.resolve_external_content(object, SBOL_SOURCE, "sbol3-12501", source);
        }
        for language in object.iris(SBOL_LANGUAGE) {
            if let Some(canonical) = tables::canonical_model_language_iri(language.as_str())
                && !canonical.eq_ignore_ascii_case(language.as_str())
            {
                self.error(
                    "sbol3-12503",
                    object,
                    Some(SBOL_LANGUAGE),
                    format!(
                        "Model language `{language}` is equivalent to Table 15 URI `{canonical}`"
                    ),
                );
            }
            if tables::is_known_non_edam_namespace(self.ontology(), language.as_str()) {
                self.warning(
                    "sbol3-12504",
                    object,
                    Some(SBOL_LANGUAGE),
                    format!("Model language `{language}` should refer to an EDAM ontology term"),
                );
            }
        }
        for framework in object.iris(SBOL_FRAMEWORK) {
            if let Some(canonical) = tables::canonical_model_framework_iri(framework.as_str())
                && !canonical.eq_ignore_ascii_case(framework.as_str())
            {
                self.error(
                    "sbol3-12506",
                    object,
                    Some(SBOL_FRAMEWORK),
                    format!(
                        "Model framework `{framework}` is equivalent to Table 16 URI `{canonical}`"
                    ),
                );
            }
            if matches!(
                tables::is_sbo_modeling_framework_term(self.ontology(), framework.as_str()),
                Some(false)
            ) {
                self.warning(
                    "sbol3-12507",
                    object,
                    Some(SBOL_FRAMEWORK),
                    format!(
                        "Model framework `{framework}` should refer to a term from the SBO modeling framework branch (SBO:0000004)"
                    ),
                );
            }
        }
    }

    pub(crate) fn validate_implementation(&mut self, object: &Object) {
        let mut referenced_components = Vec::new();
        for derived_from in object.resources(PROV_WAS_DERIVED_FROM) {
            let Some(target) = self.document.get(derived_from) else {
                continue;
            };
            if !target.has_class(SbolClass::Component) {
                self.error(
                    "sbol3-12301",
                    object,
                    Some(PROV_WAS_DERIVED_FROM),
                    format!(
                        "Implementation prov:wasDerivedFrom `{derived_from}` must refer to a Component"
                    ),
                );
                continue;
            }
            referenced_components.push(target);
        }
        if referenced_components.len() < 2 {
            return;
        }
        let mut type_iris = Vec::new();
        for component in &referenced_components {
            for iri in component.iris(SBOL_TYPE) {
                type_iris.push((component.identity().clone(), iri.clone()));
            }
        }
        for (index, (left_component, left_type)) in type_iris.iter().enumerate() {
            for (right_component, right_type) in type_iris.iter().skip(index + 1) {
                if left_component == right_component {
                    continue;
                }
                if !matches!(
                    tables::type_terms_conflict(
                        self.ontology(),
                        left_type.as_str(),
                        right_type.as_str()
                    ),
                    Some(true)
                ) {
                    continue;
                }
                self.error(
                    "sbol3-12302",
                    object,
                    Some(PROV_WAS_DERIVED_FROM),
                    format!(
                        "Implementation prov:wasDerivedFrom Components have conflicting types: \
                         `{left_component}` has `{left_type}` while `{right_component}` has `{right_type}`"
                    ),
                );
            }
        }
    }

    pub(crate) fn validate_attachment(&mut self, object: &Object) {
        for size in object.literals(SBOL_SIZE) {
            if size.value().parse::<i64>().is_ok_and(|value| value < 0) {
                self.error(
                    "sbol3-12804",
                    object,
                    Some(SBOL_SIZE),
                    "Attachment size must be a nonnegative byte count",
                );
            }
        }
        for hash in object.literals(SBOL_HASH) {
            if !is_hex_digest(hash.value()) {
                self.error(
                    "sbol3-12805",
                    object,
                    Some(SBOL_HASH),
                    "Attachment hash must be a nonempty hexadecimal digest",
                );
            }
        }
        let hash_policy = self.context.options().policy.hash_algorithm_registry;
        for algorithm in object.literals(SBOL_HASH_ALGORITHM) {
            match hash_policy {
                crate::HashAlgorithmRegistry::Lenient => {}
                crate::HashAlgorithmRegistry::Conservative => {
                    // sbol3-12806 is ▲ in Appendix B; the Conservative
                    // mode honors the spec by emitting only a warning
                    // on positively-decidable lexical-shape violations.
                    if !is_hash_algorithm_token(algorithm.value()) {
                        self.warning(
                            "sbol3-12806",
                            object,
                            Some(SBOL_HASH_ALGORITHM),
                            "Attachment hashAlgorithm should be a registry-style hash algorithm token",
                        );
                        continue;
                    }
                }
                crate::HashAlgorithmRegistry::Strict
                    if !is_known_hash_algorithm(algorithm.value()) =>
                {
                    self.error(
                        "sbol3-12806",
                        object,
                        Some(SBOL_HASH_ALGORITHM),
                        format!(
                            "Attachment hashAlgorithm `{}` is not in the known set \
                             (sha2-256, sha3-256, blake3, sha2-512, sha3-512)",
                            algorithm.value()
                        ),
                    );
                    continue;
                }
                _ => {}
            }
            if algorithm.value() != "sha3-256" {
                self.warning(
                    "sbol3-12807",
                    object,
                    Some(SBOL_HASH_ALGORITHM),
                    "Attachment hashAlgorithm should be sha3-256",
                );
            }
        }
        for format in object.iris(SBOL_FORMAT) {
            if tables::is_edam_format_term(self.ontology(), format.as_str())
                .is_some_and(|is_edam| !is_edam)
            {
                self.warning(
                    "sbol3-12803",
                    object,
                    Some(SBOL_FORMAT),
                    "Attachment format should refer to a term from the EDAM ontology",
                );
            }
        }
        if !object.values(SBOL_HASH).is_empty() && object.values(SBOL_HASH_ALGORITHM).is_empty() {
            self.error(
                "sbol3-12808",
                object,
                Some(SBOL_HASH_ALGORITHM),
                "Attachment hash requires hashAlgorithm",
            );
        }
        let content = object.first_iri(SBOL_SOURCE).and_then(|source| {
            self.resolve_external_content(object, SBOL_SOURCE, "sbol3-12801", source)
        });
        if let Some(content) = content {
            self.validate_attachment_content(object, &content);
        }
    }

    pub(crate) fn resolve_external_content(
        &mut self,
        object: &Object,
        predicate: &'static str,
        availability_rule: &'static str,
        source: &Iri,
    ) -> Option<ResolvedContent> {
        if self.context.external_mode() == ExternalValidationMode::Off {
            return None;
        }

        if self.context.content_resolvers().is_empty() {
            self.warning(
                availability_rule,
                object,
                Some(predicate),
                format!(
                    "source `{source}` was not checked because no content resolver was configured"
                ),
            );
            return None;
        }

        let mut not_found: Option<ResolutionError> = None;
        let mut unsupported: Option<ResolutionError> = None;
        for resolver in self.context.content_resolvers() {
            match resolver.resolve_content(source) {
                Ok(content) => return Some(content),
                Err(error) if error.kind() == ResolutionErrorKind::NotFound => {
                    not_found = Some(error);
                }
                Err(error) if error.kind() == ResolutionErrorKind::UnsupportedScheme => {
                    unsupported = Some(error);
                }
                Err(error) => {
                    self.warning(
                        availability_rule,
                        object,
                        Some(predicate),
                        format!(
                            "source `{source}` could not be resolved: {}",
                            error.message()
                        ),
                    );
                    return None;
                }
            }
        }

        if let Some(error) = not_found {
            self.error(
                availability_rule,
                object,
                Some(predicate),
                format!("source `{source}` was not found: {}", error.message()),
            );
            return None;
        }

        let message = unsupported
            .as_ref()
            .map_or("no resolver could handle the source".to_owned(), |error| {
                error.message().to_owned()
            });
        self.warning(
            availability_rule,
            object,
            Some(predicate),
            format!("source `{source}` was not checked: {message}"),
        );
        None
    }

    pub(crate) fn validate_attachment_content(
        &mut self,
        object: &Object,
        content: &ResolvedContent,
    ) {
        if let Some(size) = object
            .first_literal_value(SBOL_SIZE)
            .and_then(|value| value.parse::<u64>().ok())
        {
            let actual = content.bytes.len() as u64;
            if actual != size {
                self.error(
                    "sbol3-12804",
                    object,
                    Some(SBOL_SIZE),
                    format!(
                        "Attachment size is {size} bytes but resolved content is {actual} bytes"
                    ),
                );
            }
        }

        let Some(hash) = object.first_literal_value(SBOL_HASH) else {
            return;
        };
        if !is_hex_digest(hash) {
            return;
        }
        let Some(algorithm) = object.first_literal_value(SBOL_HASH_ALGORITHM) else {
            return;
        };
        if algorithm != "sha3-256" {
            return;
        }

        let digest = Sha3_256::digest(&content.bytes);
        let actual = hex_digest(&digest);
        if !hash.eq_ignore_ascii_case(&actual) {
            self.error(
                "sbol3-12805",
                object,
                Some(SBOL_HASH),
                "Attachment hash does not match the resolved content",
            );
        }
    }
}
