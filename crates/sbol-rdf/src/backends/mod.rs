use crate::{IriError, ParseError, RdfFormat, Triple, WriteError};

mod oxrdf;

pub(crate) type DefaultBackend = oxrdf::Backend;
pub(crate) use oxrdf::are_isomorphic;

pub(crate) trait Backend {
    fn parse(input: &str, format: RdfFormat) -> Result<Vec<Triple>, ParseError>;

    fn write(triples: &[Triple], format: RdfFormat) -> Result<String, WriteError>;

    fn validate_iri(value: &str) -> Result<(), IriError>;
}
