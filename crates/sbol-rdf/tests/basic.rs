use sbol_rdf::{BlankNode, Graph, Iri, Literal, RdfFormat, Resource, Term, Triple};

#[test]
fn parses_turtle_into_owned_graph() {
    let graph = Graph::parse_turtle(
        r#"BASE <https://example.org/>
PREFIX ex: <https://example.org/>
ex:subject ex:predicate "value" .
"#,
    )
    .unwrap();

    assert_eq!(graph.triples().len(), 1);
    assert_eq!(
        graph.triples()[0].subject,
        Resource::iri("https://example.org/subject")
    );
    assert_eq!(
        graph.triples()[0].predicate.as_str(),
        "https://example.org/predicate"
    );
    assert_eq!(
        graph.triples()[0].object.as_literal().unwrap().value(),
        "value"
    );
}

#[test]
fn normalized_triples_are_deterministic() {
    let first = Graph::parse_turtle(
        r#"BASE <https://example.org/>
PREFIX ex: <https://example.org/>
ex:b ex:p "2" .
ex:a ex:p "1" .
"#,
    )
    .unwrap();
    let second = Graph::parse_turtle(
        r#"BASE <https://example.org/>
PREFIX ex: <https://example.org/>
ex:a ex:p "1" .
ex:b ex:p "2" .
"#,
    )
    .unwrap();

    assert_eq!(first.normalized_triples(), second.normalized_triples());
}

#[test]
fn graph_isomorphism_ignores_blank_node_labels() {
    let first = Graph::parse_turtle(
        r#"PREFIX ex: <https://example.org/>
_:first ex:next _:second .
_:second ex:value "same" .
"#,
    )
    .unwrap();
    let second = Graph::parse_turtle(
        r#"PREFIX ex: <https://example.org/>
_:b ex:value "same" .
_:a ex:next _:b .
"#,
    )
    .unwrap();
    let different = Graph::parse_turtle(
        r#"PREFIX ex: <https://example.org/>
_:a ex:next _:b .
_:b ex:value "different" .
"#,
    )
    .unwrap();

    assert_ne!(first.normalized_triples(), second.normalized_triples());
    assert!(first.is_isomorphic_with(&second).unwrap());
    assert!(!first.is_isomorphic_with(&different).unwrap());
}

#[test]
fn rdf_terms_expose_owned_helpers() {
    let iri = Iri::new("https://example.org/predicate").unwrap();
    assert_eq!(iri.as_str(), "https://example.org/predicate");
    assert_eq!(
        iri.clone().into_string(),
        "https://example.org/predicate".to_owned()
    );
    assert_eq!(iri.to_string(), "https://example.org/predicate");
    assert!(Iri::new("not an iri").is_err());

    let blank_node = BlankNode::new("node1");
    assert_eq!(blank_node.as_str(), "node1");
    assert_eq!(
        Resource::BlankNode(blank_node.clone()).to_string(),
        "_:node1"
    );

    let resource = Resource::iri("https://example.org/resource");
    assert_eq!(
        resource.as_iri().map(Iri::as_str),
        Some("https://example.org/resource")
    );
    assert!(Resource::BlankNode(blank_node).as_iri().is_none());

    let typed_literal = Literal::new(
        "42",
        Iri::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
        Some("en".to_owned()),
    );
    assert_eq!(typed_literal.value(), "42");
    assert_eq!(
        typed_literal.datatype().as_str(),
        "http://www.w3.org/2001/XMLSchema#integer"
    );
    assert_eq!(typed_literal.language(), Some("en"));

    let resource_term = Term::Resource(resource);
    assert!(resource_term.as_resource().is_some());
    assert!(resource_term.as_iri().is_some());
    assert!(resource_term.as_literal().is_none());

    let literal_term = Term::Literal(Literal::simple("value"));
    assert!(literal_term.as_resource().is_none());
    assert!(literal_term.as_iri().is_none());
    assert_eq!(literal_term.as_literal().unwrap().value(), "value");
}

#[test]
fn reports_parse_and_write_errors() {
    let parse_error = Graph::parse_turtle("@prefix broken").unwrap_err();
    assert!(!parse_error.to_string().is_empty());
    assert!(std::error::Error::source(&parse_error).is_some());

    let graph = Graph::new(vec![Triple {
        subject: Resource::iri("https://example.org/subject"),
        predicate: Iri::new_unchecked("not an iri"),
        object: Term::Literal(Literal::simple("value")),
    }]);
    let write_error = graph.write_turtle().unwrap_err();
    assert!(!write_error.to_string().is_empty());
}

#[test]
fn every_format_round_trips_synthetic_triples() {
    let original = Graph::new(vec![
        Triple {
            subject: Resource::iri("https://example.org/a"),
            predicate: Iri::new_unchecked("https://example.org/predicate"),
            object: Term::Resource(Resource::iri("https://example.org/b")),
        },
        Triple {
            subject: Resource::iri("https://example.org/a"),
            predicate: Iri::new_unchecked("https://example.org/label"),
            object: Term::Literal(Literal::simple("hello")),
        },
        Triple {
            subject: Resource::iri("https://example.org/a"),
            predicate: Iri::new_unchecked("https://example.org/count"),
            object: Term::Literal(Literal::new(
                "3",
                Iri::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
                None,
            )),
        },
    ]);

    for &format in RdfFormat::ALL {
        let serialized = original
            .write(format)
            .unwrap_or_else(|error| panic!("write {format} failed: {error}"));
        let reparsed = Graph::parse(&serialized, format).unwrap_or_else(|error| {
            panic!("parse {format} failed: {error}\n--input--\n{serialized}")
        });
        assert_eq!(
            reparsed.normalized_triples(),
            original.normalized_triples(),
            "{format} round trip lost or rewrote triples"
        );
    }
}

#[test]
fn cross_format_equivalence_through_every_pair() {
    let source = r#"BASE <https://example.org/>
PREFIX ex: <https://example.org/>
ex:a ex:p ex:b .
ex:a ex:label "hello" .
ex:b ex:count "3"^^<http://www.w3.org/2001/XMLSchema#integer> .
"#;
    let baseline = Graph::parse(source, RdfFormat::Turtle).unwrap();

    for &format in RdfFormat::ALL {
        let serialized = baseline
            .write(format)
            .unwrap_or_else(|error| panic!("write {format} failed: {error}"));
        let reparsed = Graph::parse(&serialized, format)
            .unwrap_or_else(|error| panic!("parse {format} failed: {error}"));
        assert_eq!(
            reparsed.normalized_triples(),
            baseline.normalized_triples(),
            "Turtle → {format} → parse did not preserve normalized triples"
        );
    }
}

#[test]
fn empty_graph_round_trips_through_every_format() {
    let empty = Graph::default();

    for &format in RdfFormat::ALL {
        let serialized = empty
            .write(format)
            .unwrap_or_else(|error| panic!("write empty {format} failed: {error}"));
        let reparsed = Graph::parse(&serialized, format)
            .unwrap_or_else(|error| panic!("parse empty {format} failed: {error}"));
        assert!(
            reparsed.triples().is_empty(),
            "empty {format} round trip produced triples"
        );
    }
}

#[test]
fn rdf_format_from_extension_handles_known_and_unknown_cases() {
    assert_eq!(RdfFormat::from_extension("ttl"), Some(RdfFormat::Turtle));
    assert_eq!(RdfFormat::from_extension(".ttl"), Some(RdfFormat::Turtle));
    assert_eq!(RdfFormat::from_extension("TTL"), Some(RdfFormat::Turtle));
    assert_eq!(RdfFormat::from_extension("rdf"), Some(RdfFormat::RdfXml));
    assert_eq!(RdfFormat::from_extension("jsonld"), Some(RdfFormat::JsonLd));
    assert_eq!(RdfFormat::from_extension("JsonLd"), Some(RdfFormat::JsonLd));
    assert_eq!(RdfFormat::from_extension("nt"), Some(RdfFormat::NTriples));

    // Ambiguous and unknown extensions return None.
    assert_eq!(RdfFormat::from_extension("xml"), None);
    assert_eq!(RdfFormat::from_extension("json"), None);
    assert_eq!(RdfFormat::from_extension(""), None);
    assert_eq!(RdfFormat::from_extension("md"), None);

    assert_eq!(RdfFormat::from_path("doc.ttl"), Some(RdfFormat::Turtle));
    assert_eq!(
        RdfFormat::from_path("/tmp/Foo.JSONLD"),
        Some(RdfFormat::JsonLd)
    );
    assert_eq!(RdfFormat::from_path("noext"), None);
}

#[test]
fn preserves_blank_nodes_and_literal_metadata_from_turtle() {
    let graph = Graph::parse_turtle(
        r#"PREFIX ex: <https://example.org/>
[] ex:label "hello"@en;
   ex:count "3"^^<http://www.w3.org/2001/XMLSchema#integer> .
"#,
    )
    .unwrap();

    assert_eq!(graph.triples().len(), 2);
    assert!(matches!(graph.triples()[0].subject, Resource::BlankNode(_)));
    let literals = graph
        .triples()
        .iter()
        .filter_map(|triple| triple.object.as_literal())
        .collect::<Vec<_>>();
    assert!(
        literals
            .iter()
            .any(|literal| literal.language() == Some("en"))
    );
    assert!(literals.iter().any(|literal| {
        literal.datatype().as_str() == "http://www.w3.org/2001/XMLSchema#integer"
    }));
}
