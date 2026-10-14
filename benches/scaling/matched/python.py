"""Matched RDF-only or SBOL load-and-materialize benchmark for pySBOL3."""
import hashlib
import json
import sys
import time
from pathlib import Path
import rdflib
import sbol3

S = "http://sbols.org/v3#"
TYPE = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"


def snapshot(doc):
    rows = []
    def emit(obj, predicate, kind, value):
        rows.append(f"{obj.identity}\t{predicate}\t{kind}\t{value}\n")
    def prop(obj, name, kind, value):
        emit(obj, S + name, kind, value)
    def visit(obj):
        kind = type(obj).__name__
        emit(obj, TYPE, "I", S + kind)
        prop(obj, "displayId", "S", obj.display_id)
        if isinstance(obj, sbol3.TopLevel):
            prop(obj, "hasNamespace", "I", obj.namespace)
        if isinstance(obj, sbol3.Component):
            for name, values in (("type", obj.types), ("role", obj.roles), ("hasSequence", obj.sequences)):
                for v in values: prop(obj, name, "I", v)
            for child in obj.features:
                prop(obj, "hasFeature", "I", child.identity)
                visit(child)
        elif isinstance(obj, sbol3.Sequence):
            prop(obj, "elements", "S", obj.elements)
            prop(obj, "encoding", "I", obj.encoding)
        elif isinstance(obj, sbol3.SubComponent):
            prop(obj, "instanceOf", "I", obj.instance_of)
            prop(obj, "orientation", "I", obj.orientation)
            for child in obj.locations:
                prop(obj, "hasLocation", "I", child.identity)
                visit(child)
        elif isinstance(obj, sbol3.Range):
            for name, value in (("hasSequence", obj.sequence), ("orientation", obj.orientation)):
                prop(obj, name, "I", value)
            prop(obj, "start", "N", obj.start)
            prop(obj, "end", "N", obj.end)
        else: raise TypeError(kind)
    for obj in doc.objects: visit(obj)
    return rows


def rdf_rows(graph):
    return [f"{s}\t{p}\t{'I' if isinstance(o, rdflib.URIRef) else 'N' if o.datatype == rdflib.XSD.integer else 'S'}\t{o}\n"
            for s, p, o in graph]


def main():
    path, mode, warm, iters, output, expected = sys.argv[1:]
    warm, iters = int(warm), int(iters)
    text = Path(path).read_text()
    result = dict(implementation="python", mode=mode, warmup=warm, iterations=iters,
                  time_ns=[], serialize_ns=[], fingerprints=[], row_counts=[], serialized_bytes=0)
    for i in range(warm + iters):
        start = time.perf_counter_ns()
        if mode == "rdf":
            doc = rdflib.Graph().parse(data=text, format="xml")
            elapsed = time.perf_counter_ns() - start
            rows = rdf_rows(doc)
            serial = 0
        else:
            doc = sbol3.Document()
            doc.read_string(text, file_format="xml")
            rows = snapshot(doc)
            elapsed = time.perf_counter_ns() - start
            start = time.perf_counter_ns()
            out = doc.write_string(file_format="xml")
            serial = time.perf_counter_ns() - start
            result["serialized_bytes"] = len(out.encode())
        rows.sort()
        digest = hashlib.sha256()
        for row in rows: digest.update(row.encode())
        fingerprint = digest.hexdigest()
        assert fingerprint == expected, (fingerprint, expected)
        if i >= warm:
            for key, value in (("time_ns", elapsed), ("serialize_ns", serial), ("fingerprints", fingerprint), ("row_counts", len(rows))):
                result[key].append(value)
        print(f"completed {mode} iteration {i+1}/{warm+iters}", flush=True)
        del doc, rows
    Path(output).write_text(json.dumps(result))


if __name__ == "__main__": main()
