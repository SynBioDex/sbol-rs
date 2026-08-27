use ::oxrdf::Graph as OxGraph;
use ::oxrdf::graph::CanonicalizationAlgorithm;
use ::oxrdf::{
    BlankNode as OxBlankNode, GraphName, Literal as OxLiteral, NamedNode as OxNamedNode,
    NamedOrBlankNode as OxNamedOrBlankNode, Quad as OxQuad, Term as OxTerm, Triple as OxTriple,
};
use oxjsonld::JsonLdProfileSet;
use oxrdfio::{RdfFormat as OxRdfFormat, RdfParser, RdfSerializer};

use crate::backends::Backend as RdfBackend;
use crate::error::{IriError, ParseError, WriteError};
use crate::format::RdfFormat;
use crate::terms::{BlankNode, Iri, Literal, Resource, Term, Triple, XSD_STRING};

/// Base IRI for resolving relative references encountered while parsing. It is
/// deliberately a synthetic, non-SBOL host so a resolved relative reference is
/// recognizable as a document-local reference that carried no absolute IRI.
const DEFAULT_BASE_IRI: &str = "http://sbols.org/unspecified/";

pub(crate) enum Backend {}

pub(crate) fn are_isomorphic(left: &[Triple], right: &[Triple]) -> Result<bool, WriteError> {
    let mut left = canonical_graph(left)?;
    let mut right = canonical_graph(right)?;
    left.canonicalize(CanonicalizationAlgorithm::Unstable);
    right.canonicalize(CanonicalizationAlgorithm::Unstable);
    Ok(left == right)
}

fn canonical_graph(triples: &[Triple]) -> Result<OxGraph, WriteError> {
    triples
        .iter()
        .map(convert_triple)
        .map(|quad| quad.map(OxTriple::from))
        .collect()
}

impl RdfBackend for Backend {
    fn parse(input: &str, format: RdfFormat) -> Result<Vec<Triple>, ParseError> {
        let mut triples = Vec::new();

        // Resolve relative IRIs against a base. SBOL documents that are not
        // fully compliant (notably SBOLTestSuite fixtures whose `sbol:source`
        // and `sbol:attachment` name a sibling file with a relative reference
        // and declare no `xml:base`) otherwise fail to parse, because the RDF
        // data model has no relative-IRI term. The base only affects relative
        // references; absolute IRIs (every SBOL identity) pass through
        // unchanged. N-Triples ignores the base per its grammar.
        let parser = RdfParser::from_format(oxrdfio_format(format))
            .with_base_iri(DEFAULT_BASE_IRI)
            .expect("DEFAULT_BASE_IRI is a valid absolute IRI");

        for result in parser.for_reader(input.as_bytes()) {
            let quad = result.map_err(ParseError::backend)?;
            if !matches!(quad.graph_name, GraphName::DefaultGraph) {
                return Err(ParseError::NamedGraphInDefault);
            }
            triples.push(convert_quad(quad)?);
        }

        Ok(triples)
    }

    fn write(triples: &[Triple], format: RdfFormat) -> Result<String, WriteError> {
        let mut serializer =
            RdfSerializer::from_format(oxrdfio_format(format)).for_writer(Vec::new());
        for triple in triples {
            let quad = convert_triple(triple)?;
            serializer
                .serialize_quad(&quad)
                .map_err(WriteError::backend)?;
        }
        let bytes = serializer.finish().map_err(WriteError::backend)?;
        Ok(String::from_utf8(bytes)?)
    }

    fn validate_iri(value: &str) -> Result<(), IriError> {
        OxNamedNode::new(value)
            .map(|_| ())
            .map_err(|error| IriError::new(value, error))
    }
}

fn oxrdfio_format(format: RdfFormat) -> OxRdfFormat {
    match format {
        RdfFormat::Turtle => OxRdfFormat::Turtle,
        RdfFormat::RdfXml => OxRdfFormat::RdfXml,
        RdfFormat::JsonLd => OxRdfFormat::JsonLd {
            profile: JsonLdProfileSet::empty(),
        },
        RdfFormat::NTriples => OxRdfFormat::NTriples,
    }
}

fn convert_quad(quad: OxQuad) -> Result<Triple, ParseError> {
    Ok(Triple {
        subject: convert_named_or_blank(quad.subject),
        predicate: Iri::new_unchecked(quad.predicate.as_str()),
        object: convert_term(quad.object)?,
    })
}

fn convert_named_or_blank(node: OxNamedOrBlankNode) -> Resource {
    match node {
        OxNamedOrBlankNode::NamedNode(node) => Resource::Iri(Iri::new_unchecked(node.as_str())),
        OxNamedOrBlankNode::BlankNode(node) => Resource::BlankNode(BlankNode::new(node.as_str())),
    }
}

fn convert_term(term: OxTerm) -> Result<Term, ParseError> {
    match term {
        OxTerm::NamedNode(node) => Ok(Term::Resource(Resource::Iri(Iri::new_unchecked(
            node.as_str(),
        )))),
        OxTerm::BlankNode(node) => Ok(Term::Resource(Resource::BlankNode(BlankNode::new(
            node.as_str(),
        )))),
        OxTerm::Literal(literal) => Ok(Term::Literal(Literal::new(
            literal.value(),
            Iri::new_unchecked(literal.datatype().as_str()),
            literal.language().map(ToOwned::to_owned),
        ))),
        #[allow(unreachable_patterns)]
        _ => Err(ParseError::UnsupportedRdfStar),
    }
}

fn convert_triple(triple: &Triple) -> Result<OxQuad, WriteError> {
    let subject = convert_resource(&triple.subject)?;
    let predicate = OxNamedNode::new(triple.predicate.as_str())
        .map_err(|error| WriteError::invalid_iri(triple.predicate.as_str(), error))?;
    let object = convert_object(&triple.object)?;
    Ok(OxQuad::new(
        subject,
        predicate,
        object,
        GraphName::DefaultGraph,
    ))
}

fn convert_resource(resource: &Resource) -> Result<OxNamedOrBlankNode, WriteError> {
    match resource {
        Resource::Iri(iri) => OxNamedNode::new(iri.as_str())
            .map(OxNamedOrBlankNode::NamedNode)
            .map_err(|error| WriteError::invalid_iri(iri.as_str(), error)),
        Resource::BlankNode(blank_node) => OxBlankNode::new(blank_node.as_str())
            .map(OxNamedOrBlankNode::BlankNode)
            .map_err(|error| WriteError::invalid_blank_node(blank_node.as_str(), error)),
    }
}

fn convert_object(term: &Term) -> Result<OxTerm, WriteError> {
    match term {
        Term::Resource(resource) => convert_resource(resource).map(OxTerm::from),
        Term::Literal(literal) => {
            if let Some(language) = literal.language() {
                OxLiteral::new_language_tagged_literal(literal.value(), language)
                    .map(OxTerm::from)
                    .map_err(|error| WriteError::invalid_language_tag(language, error))
            } else if literal.datatype().as_str() == XSD_STRING {
                Ok(OxTerm::from(OxLiteral::new_simple_literal(literal.value())))
            } else {
                let datatype = OxNamedNode::new(literal.datatype().as_str())
                    .map_err(|error| WriteError::invalid_iri(literal.datatype().as_str(), error))?;
                Ok(OxTerm::from(OxLiteral::new_typed_literal(
                    literal.value(),
                    datatype,
                )))
            }
        }
    }
}
