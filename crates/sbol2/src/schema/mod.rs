//! Field-metadata catalog for the SBOL 2 data model.
//!
//! [`FieldDescriptor`] describes one property: its predicate IRI, cardinality,
//! value kind, optional reference target, and validation rule. `class_spec`
//! maps every class IRI to its parents and directly-declared fields; the RDF
//! serializer and (later) validator share this single source of truth so the
//! schema cannot drift between subsystems.

use std::collections::BTreeMap;

use crate::Object;
use crate::vocab::*;

pub use sbol_core::schema::{
    Cardinality, ClassDescriptor, FieldDescriptor, ReferenceSpec, TargetClass, ValueKind,
};

mod properties;
use properties::*;

pub(crate) type PropertySpec = FieldDescriptor;
pub(crate) type ClassSpec = ClassDescriptor;

/// Looks up the [`ClassDescriptor`] for an SBOL 2 class by its RDF type IRI.
/// Returns `None` if the IRI is not a known SBOL 2, PROV, or OM class.
pub fn class_descriptor(class_iri: &str) -> Option<ClassDescriptor> {
    class_spec(class_iri)
}

/// Returns the field descriptors declared directly on the given class.
pub fn class_fields(class: crate::Sbol2Class) -> &'static [FieldDescriptor] {
    class_spec(class.iri())
        .map(|descriptor| descriptor.fields)
        .unwrap_or(&[])
}

pub(crate) fn class_spec(iri: &str) -> Option<ClassSpec> {
    Some(match iri {
        SBOL2_IDENTIFIED_CLASS => ClassSpec {
            parents: &[],
            fields: IDENTIFIED_PROPS,
        },
        SBOL2_TOP_LEVEL_CLASS => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: TOP_LEVEL_PROPS,
        },
        SBOL2_MEASURED_CLASS => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: MEASURED_PROPS,
        },
        SBOL2_COMPONENT_INSTANCE_CLASS => ClassSpec {
            parents: &[SBOL2_MEASURED_CLASS],
            fields: COMPONENT_INSTANCE_PROPS,
        },
        SBOL2_LOCATION_CLASS => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: LOCATION_PROPS,
        },
        SBOL2_SEQUENCE_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: SEQUENCE_PROPS,
        },
        SBOL2_COMPONENT_DEFINITION_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: COMPONENT_DEFINITION_PROPS,
        },
        SBOL2_MODULE_DEFINITION_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: MODULE_DEFINITION_PROPS,
        },
        SBOL2_MODEL_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: MODEL_PROPS,
        },
        SBOL2_COLLECTION_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: COLLECTION_PROPS,
        },
        SBOL2_COMBINATORIAL_DERIVATION_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: COMBINATORIAL_DERIVATION_PROPS,
        },
        SBOL2_IMPLEMENTATION_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: IMPLEMENTATION_PROPS,
        },
        SBOL2_ATTACHMENT_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: ATTACHMENT_PROPS,
        },
        SBOL2_EXPERIMENTAL_DATA_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: EXPERIMENTAL_DATA_PROPS,
        },
        SBOL2_EXPERIMENT_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: EXPERIMENT_PROPS,
        },
        SBOL2_GENERIC_TOP_LEVEL_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: GENERIC_TOP_LEVEL_PROPS,
        },
        SBOL2_COMPONENT_CLASS => ClassSpec {
            parents: &[SBOL2_COMPONENT_INSTANCE_CLASS],
            fields: COMPONENT_PROPS,
        },
        SBOL2_FUNCTIONAL_COMPONENT_CLASS => ClassSpec {
            parents: &[SBOL2_COMPONENT_INSTANCE_CLASS],
            fields: FUNCTIONAL_COMPONENT_PROPS,
        },
        SBOL2_MODULE_CLASS => ClassSpec {
            parents: &[SBOL2_MEASURED_CLASS],
            fields: MODULE_PROPS,
        },
        SBOL2_MAPS_TO_CLASS => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: MAPS_TO_PROPS,
        },
        SBOL2_SEQUENCE_ANNOTATION_CLASS => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: SEQUENCE_ANNOTATION_PROPS,
        },
        SBOL2_SEQUENCE_CONSTRAINT_CLASS => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: SEQUENCE_CONSTRAINT_PROPS,
        },
        SBOL2_VARIABLE_COMPONENT_CLASS => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: VARIABLE_COMPONENT_PROPS,
        },
        SBOL2_INTERACTION_CLASS => ClassSpec {
            parents: &[SBOL2_MEASURED_CLASS],
            fields: INTERACTION_PROPS,
        },
        SBOL2_PARTICIPATION_CLASS => ClassSpec {
            parents: &[SBOL2_MEASURED_CLASS],
            fields: PARTICIPATION_PROPS,
        },
        SBOL2_RANGE_CLASS => ClassSpec {
            parents: &[SBOL2_LOCATION_CLASS],
            fields: RANGE_PROPS,
        },
        SBOL2_CUT_CLASS => ClassSpec {
            parents: &[SBOL2_LOCATION_CLASS],
            fields: CUT_PROPS,
        },
        SBOL2_GENERIC_LOCATION_CLASS => ClassSpec {
            parents: &[SBOL2_LOCATION_CLASS],
            fields: GENERIC_LOCATION_PROPS,
        },
        PROV_ACTIVITY => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: PROV_ACTIVITY_PROPS,
        },
        PROV_AGENT_CLASS => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: PROV_AGENT_PROPS,
        },
        PROV_PLAN => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: PROV_PLAN_PROPS,
        },
        PROV_ASSOCIATION => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: PROV_ASSOCIATION_PROPS,
        },
        PROV_USAGE => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: PROV_USAGE_PROPS,
        },
        OM_MEASURE => ClassSpec {
            parents: &[SBOL2_IDENTIFIED_CLASS],
            fields: OM_MEASURE_PROPS,
        },
        OM_UNIT => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: OM_UNIT_PROPS,
        },
        OM_SINGULAR_UNIT => ClassSpec {
            parents: &[OM_UNIT],
            fields: OM_SINGULAR_UNIT_PROPS,
        },
        OM_COMPOUND_UNIT => ClassSpec {
            parents: &[OM_UNIT],
            fields: OM_COMPOUND_UNIT_PROPS,
        },
        OM_UNIT_MULTIPLICATION => ClassSpec {
            parents: &[OM_COMPOUND_UNIT],
            fields: OM_UNIT_MULTIPLICATION_PROPS,
        },
        OM_UNIT_DIVISION => ClassSpec {
            parents: &[OM_COMPOUND_UNIT],
            fields: OM_UNIT_DIVISION_PROPS,
        },
        OM_UNIT_EXPONENTIATION => ClassSpec {
            parents: &[OM_COMPOUND_UNIT],
            fields: OM_UNIT_EXPONENTIATION_PROPS,
        },
        OM_PREFIXED_UNIT => ClassSpec {
            parents: &[OM_UNIT],
            fields: OM_PREFIXED_UNIT_PROPS,
        },
        OM_PREFIX => ClassSpec {
            parents: &[SBOL2_TOP_LEVEL_CLASS],
            fields: OM_PREFIX_PROPS,
        },
        OM_SI_PREFIX => ClassSpec {
            parents: &[OM_PREFIX],
            fields: OM_SI_PREFIX_PROPS,
        },
        OM_BINARY_PREFIX => ClassSpec {
            parents: &[OM_PREFIX],
            fields: OM_BINARY_PREFIX_PROPS,
        },
        _ => return None,
    })
}

/// The canonical XSD datatype IRI for a literal-valued [`ValueKind`], or
/// `None` for the reference kinds (`Uri`, `Url`).
pub(crate) fn xsd_datatype(kind: ValueKind) -> Option<&'static str> {
    match kind {
        ValueKind::String => Some(XSD_STRING),
        ValueKind::Integer => Some(XSD_INTEGER),
        ValueKind::Long => Some(XSD_LONG),
        ValueKind::Float => Some(XSD_DECIMAL),
        ValueKind::DateTime => Some(XSD_DATE_TIME),
        ValueKind::Uri | ValueKind::Url => None,
        _ => None,
    }
}

/// The spec-defined XSD datatype for a recognized literal-valued predicate.
///
/// SBOL 2 files vary in how they type literals. A Turtle file may write
/// `sbol:start 10` (an `xsd:integer`) while an RDF/XML file writes
/// `<sbol:start>10</sbol:start>` (an `xsd:string`) for the same property. The
/// serializer emits, and the reader normalizes to, the datatype the SBOL 2
/// data model assigns each property, so the typed surface and its RDF form
/// agree. Predicates outside the recognized field set (extension triples)
/// keep their literals verbatim.
pub(crate) fn literal_datatype(predicate: &str) -> Option<&'static str> {
    static CACHE: std::sync::OnceLock<BTreeMap<&'static str, &'static str>> =
        std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let mut map = BTreeMap::new();
            for class_iri in ALL_CLASS_IRIS {
                if let Some(spec) = class_spec(class_iri) {
                    for field in spec.fields {
                        if let Some(datatype) = xsd_datatype(field.value_kind) {
                            map.insert(field.predicate, datatype);
                        }
                    }
                }
            }
            map
        })
        .get(predicate)
        .copied()
}

const ALL_CLASS_IRIS: &[&str] = &[
    SBOL2_IDENTIFIED_CLASS,
    SBOL2_TOP_LEVEL_CLASS,
    SBOL2_MEASURED_CLASS,
    SBOL2_COMPONENT_INSTANCE_CLASS,
    SBOL2_LOCATION_CLASS,
    SBOL2_SEQUENCE_CLASS,
    SBOL2_COMPONENT_DEFINITION_CLASS,
    SBOL2_MODULE_DEFINITION_CLASS,
    SBOL2_MODEL_CLASS,
    SBOL2_COLLECTION_CLASS,
    SBOL2_COMBINATORIAL_DERIVATION_CLASS,
    SBOL2_IMPLEMENTATION_CLASS,
    SBOL2_ATTACHMENT_CLASS,
    SBOL2_EXPERIMENTAL_DATA_CLASS,
    SBOL2_EXPERIMENT_CLASS,
    SBOL2_GENERIC_TOP_LEVEL_CLASS,
    SBOL2_COMPONENT_CLASS,
    SBOL2_FUNCTIONAL_COMPONENT_CLASS,
    SBOL2_MODULE_CLASS,
    SBOL2_MAPS_TO_CLASS,
    SBOL2_SEQUENCE_ANNOTATION_CLASS,
    SBOL2_SEQUENCE_CONSTRAINT_CLASS,
    SBOL2_VARIABLE_COMPONENT_CLASS,
    SBOL2_INTERACTION_CLASS,
    SBOL2_PARTICIPATION_CLASS,
    SBOL2_RANGE_CLASS,
    SBOL2_CUT_CLASS,
    SBOL2_GENERIC_LOCATION_CLASS,
    PROV_ACTIVITY,
    PROV_AGENT_CLASS,
    PROV_PLAN,
    PROV_ASSOCIATION,
    PROV_USAGE,
    OM_MEASURE,
    OM_UNIT,
    OM_SINGULAR_UNIT,
    OM_COMPOUND_UNIT,
    OM_UNIT_MULTIPLICATION,
    OM_UNIT_DIVISION,
    OM_UNIT_EXPONENTIATION,
    OM_PREFIXED_UNIT,
    OM_PREFIX,
    OM_SI_PREFIX,
    OM_BINARY_PREFIX,
];

/// Collects every field descriptor (inherited plus own) an object's RDF types
/// resolve to, keyed by predicate IRI.
pub fn property_specs_for(object: &Object) -> BTreeMap<&'static str, FieldDescriptor> {
    let mut specs = BTreeMap::new();
    for rdf_type in object.rdf_types() {
        collect_class_properties(rdf_type.as_str(), &mut specs);
    }
    specs
}

pub(crate) fn collect_class_properties(
    iri: &str,
    specs: &mut BTreeMap<&'static str, PropertySpec>,
) {
    let Some(class) = class_spec(iri) else {
        return;
    };
    for parent in class.parents {
        collect_class_properties(parent, specs);
    }
    for property in class.fields {
        specs.insert(property.predicate, *property);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sbol2Class;

    #[test]
    fn every_class_has_a_spec() {
        for class in [
            Sbol2Class::Identified,
            Sbol2Class::TopLevel,
            Sbol2Class::Measured,
            Sbol2Class::ComponentInstance,
            Sbol2Class::Location,
            Sbol2Class::Sequence,
            Sbol2Class::ComponentDefinition,
            Sbol2Class::ModuleDefinition,
            Sbol2Class::Model,
            Sbol2Class::Collection,
            Sbol2Class::CombinatorialDerivation,
            Sbol2Class::Implementation,
            Sbol2Class::Attachment,
            Sbol2Class::ExperimentalData,
            Sbol2Class::Experiment,
            Sbol2Class::GenericTopLevel,
            Sbol2Class::Component,
            Sbol2Class::FunctionalComponent,
            Sbol2Class::Module,
            Sbol2Class::MapsTo,
            Sbol2Class::SequenceAnnotation,
            Sbol2Class::SequenceConstraint,
            Sbol2Class::VariableComponent,
            Sbol2Class::Interaction,
            Sbol2Class::Participation,
            Sbol2Class::Range,
            Sbol2Class::Cut,
            Sbol2Class::GenericLocation,
            Sbol2Class::ProvActivity,
            Sbol2Class::ProvAgent,
            Sbol2Class::ProvPlan,
            Sbol2Class::ProvAssociation,
            Sbol2Class::ProvUsage,
            Sbol2Class::OmMeasure,
            Sbol2Class::OmUnit,
            Sbol2Class::OmSingularUnit,
            Sbol2Class::OmCompoundUnit,
            Sbol2Class::OmUnitMultiplication,
            Sbol2Class::OmUnitDivision,
            Sbol2Class::OmUnitExponentiation,
            Sbol2Class::OmPrefixedUnit,
            Sbol2Class::OmPrefix,
            Sbol2Class::OmSiPrefix,
            Sbol2Class::OmBinaryPrefix,
        ] {
            assert!(
                class_spec(class.iri()).is_some(),
                "missing class_spec for {}",
                class.local_name()
            );
        }
    }

    #[test]
    fn component_inherits_component_instance_and_measured_fields() {
        let mut specs = BTreeMap::new();
        collect_class_properties(Sbol2Class::Component.iri(), &mut specs);
        assert!(specs.contains_key(SBOL2_DEFINITION));
        assert!(specs.contains_key(SBOL2_MEASURE));
        assert!(specs.contains_key(SBOL2_DISPLAY_ID));
        assert!(specs.contains_key(SBOL2_ROLE_INTEGRATION));
    }
}
