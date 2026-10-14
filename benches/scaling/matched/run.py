"""Sequential benchmark of verified, equivalent RDF and SBOL endpoints."""
import argparse
import gzip
import hashlib
import json
import os
import signal
import statistics
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent
BUILD = ROOT.parents[2] / "target/benchmarks"
TOOLS = Path("/private/tmp/sbol-scaling-tools")
INPUTS = Path("/private/tmp/sbol-scaling-inputs")
MANIFEST = {x["objects"]: x for x in json.loads((ROOT.parent / "input-manifest.json").read_text())}
RDF = "{http://www.w3.org/1999/02/22-rdf-syntax-ns#}"


def expected(count):
    entry = MANIFEST[count]
    data = gzip.decompress((ROOT.parent / "inputs" / entry["file"]).read_bytes())
    assert hashlib.sha256(data).hexdigest() == entry["sha256"]
    path = INPUTS / Path(entry["file"]).stem
    assert path.read_bytes() == data
    rows = []
    for obj in ET.fromstring(data):
        for prop in obj:
            predicate = prop.tag[1:].replace("}", "")
            value = prop.attrib.get(RDF + "resource")
            kind = "I" if value is not None else "N" if prop.attrib.get(RDF + "datatype", "").endswith("#integer") else "S"
            if value is None: value = prop.text
            rows.append(f"{obj.attrib[RDF+'about']}\t{predicate}\t{kind}\t{value}\n")
    assert len(rows) == entry["triples"]
    digest = hashlib.sha256()
    for row in sorted(rows): digest.update(row.encode())
    return path, digest.hexdigest()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--objects", type=int, nargs="+", default=[1000,10000,100000])
    p.add_argument("--implementations", nargs="+", default=["rust","python","java","javascript"])
    p.add_argument("--modes", nargs="+", choices=["rdf","ready"], default=["rdf","ready"])
    p.add_argument("--warmup", type=int, default=3)
    p.add_argument("--iters", type=int, default=5)
    p.add_argument("--directory", default="results")
    p.add_argument("--timeout", type=int, default=1200)
    args = p.parse_args()
    output = ROOT / args.directory
    output.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.update(PYTHONPATH=str(TOOLS/"python-packages"),PYTHONNOUSERSITE="1",NODE_PATH=str(TOOLS/"javascript/node_modules"),
               PYTHONPYCACHEPREFIX=str(BUILD/"pycache"))
    drivers = {
        "rust": ["/private/tmp/sbol-review-validation/target/release/sbol-matched-bench"],
        "python": ["/usr/local/bin/python3.12",str(ROOT/"python.py")],
        "java": [str(TOOLS/"jdk-17.0.20.1+1/Contents/Home/bin/java"),"-cp",str(TOOLS/"libSBOLj3.jar")+":"+str(BUILD/"java"),"Matched"],
        "javascript": [str(TOOLS/"node-v20.20.2-darwin-arm64/bin/node"),str(ROOT/"javascript.js")],
    }
    endpoints = {}
    for count in args.objects:
        path, fingerprint = expected(count)
        endpoints[count] = {"input":MANIFEST[count],"endpoint_sha256":fingerprint}
        for mode in args.modes:
            for implementation in args.implementations:
                stem = output / f"{implementation}_{count}_{mode}"
                command = [str(TOOLS/"measure_rss.native"),str(stem.with_suffix(".rss.json")),*drivers[implementation],
                           str(path),mode,str(args.warmup),str(args.iters),str(stem.with_suffix(".json")),fingerprint]
                record = {"command":command,"input":MANIFEST[count],"endpoint_sha256":fingerprint,
                          "started_utc":datetime.now(timezone.utc).isoformat(),"timeout_seconds":args.timeout}
                print(f"START {stem.name}",flush=True)
                start = time.monotonic()
                with stem.with_suffix(".log").open("w") as log:
                    proc = subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
                    try: record["exit_code"] = proc.wait(timeout=args.timeout)
                    except subprocess.TimeoutExpired:
                        os.killpg(proc.pid,signal.SIGTERM)
                        try: proc.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            os.killpg(proc.pid,signal.SIGKILL); proc.wait()
                        record["exit_code"] = "timeout"
                record["elapsed_seconds"] = time.monotonic()-start
                stem.with_suffix(".record.json").write_text(json.dumps(record,indent=2)+"\n")
                assert record["exit_code"] == 0, (stem,record["exit_code"])
                result = json.loads(stem.with_suffix(".json").read_text())
                assert result["fingerprints"] == [fingerprint]*args.iters
                assert result["row_counts"] == [MANIFEST[count]["triples"]]*args.iters
                print(f"DONE {stem.name}: median {statistics.median(result['time_ns'])/1e6:.3f} ms; endpoint verified",flush=True)
    (output/"endpoints.json").write_text(json.dumps(endpoints,indent=2)+"\n")


if __name__ == "__main__": main()
