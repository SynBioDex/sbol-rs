"""Independently check RDF/XML entities, references, sequences, and hashes."""

import gzip
import hashlib
import json
from collections import Counter
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent
RDF = "{http://www.w3.org/1999/02/22-rdf-syntax-ns#}"
SBOL = "{http://sbols.org/v3#}"


def main():
    checked = []
    for metadata in json.loads((ROOT / "input-manifest.json").read_text()):
        compressed = (ROOT / "inputs" / metadata["file"]).read_bytes()
        assert hashlib.sha256(compressed).hexdigest() == metadata["compressed_sha256"]
        content = gzip.decompress(compressed)
        assert len(content) == metadata["bytes"]
        assert hashlib.sha256(content).hexdigest() == metadata["sha256"]
        graph = ET.fromstring(content)
        objects = {node.attrib[RDF + "about"]: node for node in graph}
        assert len(objects) == len(graph) == metadata["objects"]
        assert sum(len(node) for node in graph) == metadata["triples"]
        classes = Counter()
        references = 0
        for identity, node in objects.items():
            kind = node.find(RDF + "type").attrib[RDF + "resource"].rsplit("#", 1)[-1]
            classes[kind] += 1
            for predicate in ("hasFeature", "hasLocation", "hasSequence", "instanceOf"):
                for link in node.findall(SBOL + predicate):
                    assert link.attrib[RDF + "resource"] in objects
                    references += 1
            if kind == "Range":
                start = int(node.find(SBOL + "start").text)
                end = int(node.find(SBOL + "end").text)
                sequence_id = node.find(SBOL + "hasSequence").attrib[RDF + "resource"]
                elements = objects[sequence_id].find(SBOL + "elements").text
                assert 1 <= start <= end <= len(elements)
                feature = objects[identity.rsplit("/", 1)[0]]
                part_id = feature.find(SBOL + "instanceOf").attrib[RDF + "resource"]
                part_sequence_id = objects[part_id].find(SBOL + "hasSequence").attrib[RDF + "resource"]
                part_elements = objects[part_sequence_id].find(SBOL + "elements").text
                assert elements[start - 1:end] == part_elements
        units = metadata["designs"]
        assert classes == {"Component": 3 * units, "Sequence": 3 * units,
                           "SubComponent": 2 * units, "Range": 2 * units}
        assert sum(node.find(SBOL + "hasNamespace") is not None for node in graph) == metadata["top_level_objects"]
        checked.append({"objects": len(objects), "triples": metadata["triples"],
                        "classes": dict(classes), "internal_references": references})
    (ROOT / "structure-check.json").write_text(json.dumps(checked, indent=2) + "\n")
    print("Verified all input hashes, object and triple counts, internal references, and assembled sequences.")


if __name__ == "__main__":
    main()
