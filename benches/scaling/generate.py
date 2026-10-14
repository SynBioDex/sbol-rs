"""Generate a deterministic family of linked, two-part SBOL3 designs."""

import argparse
import gzip
import hashlib
import json
from pathlib import Path
from xml.sax.saxutils import escape

BASE = "https://example.org/scaling/"
SBOL = "http://sbols.org/v3#"
RDF = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
XSD = "http://www.w3.org/2001/XMLSchema#"
DNA = "https://identifiers.org/SBO:0000251"
INLINE = "https://identifiers.org/SO:0001030"
ENCODING = "https://identifiers.org/edam:format_1207"


def design(index):
    prefix = f"design{index:05d}"
    result = []

    def entity(suffix, kind, properties, top_level=False):
        identity = BASE + prefix + suffix
        triples = [(identity, RDF + "type", SBOL + kind, "iri")]
        triples.append((identity, SBOL + "displayId", (prefix + suffix) if top_level else suffix.rsplit("/", 1)[-1], "string"))
        if top_level:
            triples.append((identity, SBOL + "hasNamespace", BASE.rstrip("/"), "iri"))
        triples.extend((identity, SBOL + predicate, value, datatype) for predicate, value, datatype in properties)
        result.append(triples)

    def sequence(suffix, elements):
        entity(suffix, "Sequence", [("elements", elements, "string"), ("encoding", ENCODING, "iri")], True)

    entity("", "Component", [
        ("type", DNA, "iri"), ("role", "https://identifiers.org/SO:0000804", "iri"),
        ("hasSequence", BASE + prefix + "_sequence", "iri"),
        ("hasFeature", BASE + prefix + "/part1", "iri"),
        ("hasFeature", BASE + prefix + "/part2", "iri"),
    ], True)
    sequence("_sequence", "acgt" * 16)
    for part in (1, 2):
        entity(f"/part{part}", "SubComponent", [
            ("instanceOf", BASE + prefix + f"_part{part}", "iri"),
            ("orientation", INLINE, "iri"),
            ("hasLocation", BASE + prefix + f"/part{part}/range", "iri"),
        ])
        entity(f"/part{part}/range", "Range", [
            ("start", str((part - 1) * 32 + 1), "integer"),
            ("end", str(part * 32), "integer"),
            ("orientation", INLINE, "iri"),
            ("hasSequence", BASE + prefix + "_sequence", "iri"),
        ])
        entity(f"_part{part}", "Component", [
            ("type", DNA, "iri"),
            ("role", "https://identifiers.org/SO:" + ("0000167" if part == 1 else "0000316"), "iri"),
            ("hasSequence", BASE + prefix + f"_part{part}_sequence", "iri"),
        ], True)
        sequence(f"_part{part}_sequence", "acgt" * 8)
    return result


def generate(count, directory):
    assert count > 0 and count % 10 == 0
    target = directory / f"sbol3_{count}.rdf.gz"
    digest = hashlib.sha256()
    size = triples = subjects = 0
    with target.open("wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as output:
        def write(text):
            nonlocal size
            data = text.encode()
            output.write(data)
            digest.update(data)
            size += len(data)

        write(f'<?xml version="1.0" encoding="UTF-8"?>\n<rdf:RDF xmlns:rdf="{RDF}" xmlns:sbol="{SBOL}">\n')
        for index in range(count // 10):
            for entity in design(index):
                subjects += 1
                write(f'  <rdf:Description rdf:about="{entity[0][0]}">\n')
                for _, predicate, value, datatype in entity:
                    triples += 1
                    field = "rdf:type" if predicate == RDF + "type" else "sbol:" + predicate.removeprefix(SBOL)
                    if datatype == "iri":
                        write(f'    <{field} rdf:resource="{value}"/>\n')
                    else:
                        attribute = f' rdf:datatype="{XSD}integer"' if datatype == "integer" else ""
                        write(f'    <{field}{attribute}>{escape(value)}</{field}>\n')
                write('  </rdf:Description>\n')
        write('</rdf:RDF>\n')
    assert subjects == count
    return {"file": target.name, "objects": subjects, "designs": count // 10,
            "top_level_objects": count * 6 // 10, "triples": triples,
            "bytes": size, "sha256": digest.hexdigest(),
            "compressed_bytes": target.stat().st_size,
            "compressed_sha256": hashlib.sha256(target.read_bytes()).hexdigest()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--objects", nargs="+", type=int, default=[10, 1000, 10000, 100000])
    parser.add_argument("--output", type=Path, default=Path(__file__).parent / "inputs")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = [generate(count, args.output) for count in args.objects]
    (args.output.parent / "input-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))
