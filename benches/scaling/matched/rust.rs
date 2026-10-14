use sbol::v3::{Document, RdfFormat, SbolObject};
use sbol_rdf::{Graph, Term, Triple};
use sha2::{Digest, Sha256};
use std::{env, fs, time::Instant};

const S: &str = "http://sbols.org/v3#";
const TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

fn row(rows: &mut Vec<String>, id: &str, predicate: &str, kind: &str, value: impl std::fmt::Display) {
    rows.push(format!("{id}\t{predicate}\t{kind}\t{value}\n"));
}
fn property(rows: &mut Vec<String>, id: &str, name: &str, kind: &str, value: impl std::fmt::Display) {
    row(rows, id, &format!("{S}{name}"), kind, value);
}
fn base(rows: &mut Vec<String>, id: &str, class: &str, display_id: &str) {
    row(rows, id, TYPE, "I", format!("{S}{class}"));
    property(rows, id, "displayId", "S", display_id);
}
fn snapshot(doc: &Document) -> Vec<String> {
    let mut rows = Vec::with_capacity(doc.rdf_graph().triples().len());
    for obj in doc.typed_objects() {
        let id = obj.identity().to_string();
        match obj {
            SbolObject::Component(c) => {
                base(&mut rows, &id, "Component", c.identified.display_id.as_deref().unwrap());
                property(&mut rows, &id, "hasNamespace", "I", c.top_level.namespace.as_ref().unwrap());
                for v in &c.types { property(&mut rows, &id, "type", "I", v); }
                for v in &c.roles { property(&mut rows, &id, "role", "I", v); }
                for v in &c.sequences { property(&mut rows, &id, "hasSequence", "I", v); }
                for v in &c.features { property(&mut rows, &id, "hasFeature", "I", v); }
            }
            SbolObject::Sequence(c) => {
                base(&mut rows, &id, "Sequence", c.identified.display_id.as_deref().unwrap());
                property(&mut rows, &id, "hasNamespace", "I", c.top_level.namespace.as_ref().unwrap());
                property(&mut rows, &id, "elements", "S", c.elements.as_ref().unwrap());
                property(&mut rows, &id, "encoding", "I", c.encoding.as_ref().unwrap());
            }
            SbolObject::SubComponent(c) => {
                base(&mut rows, &id, "SubComponent", c.identified.display_id.as_deref().unwrap());
                property(&mut rows, &id, "instanceOf", "I", c.instance_of.as_ref().unwrap());
                property(&mut rows, &id, "orientation", "I", c.feature.orientation.as_ref().unwrap());
                for v in &c.locations { property(&mut rows, &id, "hasLocation", "I", v); }
            }
            SbolObject::Range(c) => {
                base(&mut rows, &id, "Range", c.identified.display_id.as_deref().unwrap());
                property(&mut rows, &id, "hasSequence", "I", c.location.sequence.as_ref().unwrap());
                property(&mut rows, &id, "orientation", "I", c.location.orientation.as_ref().unwrap());
                property(&mut rows, &id, "start", "N", c.start.unwrap());
                property(&mut rows, &id, "end", "N", c.end.unwrap());
            }
            _ => panic!("unexpected class in the synthetic profile"),
        }
    }
    rows
}
fn rdf_rows(triples: &[Triple]) -> Vec<String> {
    triples.iter().map(|t| {
        let (kind, value) = match &t.object {
            Term::Resource(v) => ("I", v.to_string()),
            Term::Literal(v) => (if v.datatype().as_str().ends_with("#integer") { "N" } else { "S" }, v.value().to_owned()),
            _ => panic!("unexpected RDF term"),
        };
        format!("{}\t{}\t{kind}\t{value}\n", t.subject, t.predicate)
    }).collect()
}
fn fingerprint(rows: &mut [String]) -> String {
    rows.sort_unstable();
    let mut digest = Sha256::new();
    for row in rows { digest.update(row.as_bytes()); }
    digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}
fn main() {
    let a: Vec<_> = env::args().collect();
    let text = fs::read_to_string(&a[1]).unwrap();
    let mode = &a[2];
    let warm: usize = a[3].parse().unwrap();
    let iters: usize = a[4].parse().unwrap();
    let mut times = Vec::new(); let mut serial = Vec::new(); let mut hashes = Vec::new();
    let mut counts = Vec::new(); let mut last_bytes = 0;
    for i in 0..warm+iters {
        let start = Instant::now();
        let (mut rows, elapsed, serialized) = if mode == "rdf" {
            let graph = Graph::parse(&text, RdfFormat::RdfXml).unwrap();
            let elapsed = start.elapsed().as_nanos() as u64;
            (rdf_rows(graph.triples()), elapsed, 0)
        } else {
            let doc = Document::read(&text, RdfFormat::RdfXml).unwrap();
            let rows = snapshot(&doc);
            let elapsed = start.elapsed().as_nanos() as u64;
            let start = Instant::now();
            let out = doc.write(RdfFormat::RdfXml).unwrap();
            let serialized = start.elapsed().as_nanos() as u64;
            last_bytes = out.len();
            (rows, elapsed, serialized)
        };
        let hash = fingerprint(&mut rows);
        assert_eq!(hash, a[6], "endpoint does not match the input");
        if i >= warm { times.push(elapsed); serial.push(serialized); hashes.push(hash); counts.push(rows.len()); }
        eprintln!("completed {mode} iteration {}/{}", i+1, warm+iters);
    }
    fs::write(&a[5], serde_json::to_string(&serde_json::json!({"implementation":"rust", "mode":mode,
        "warmup":warm,"iterations":iters,"time_ns":times,"serialize_ns":serial,
        "fingerprints":hashes,"row_counts":counts,"serialized_bytes":last_bytes})).unwrap()).unwrap();
}
