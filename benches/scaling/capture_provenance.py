"""Capture the native experiment's runtime, source, and binary identities."""

import hashlib
import json
import os
import platform
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TOOLS = Path("/private/tmp/sbol-scaling-tools")


def command(arguments):
    result = subprocess.run(arguments, capture_output=True, text=True, check=True)
    return (result.stdout + result.stderr).strip()


def identity(path):
    return {"bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def main():
    metadata = {
        "platform": platform.platform(),
        "cpu": command(["sysctl", "-n", "machdep.cpu.brand_string"]),
        "memory_bytes": int(command(["sysctl", "-n", "hw.memsize"])),
        "logical_cpus": int(command(["sysctl", "-n", "hw.logicalcpu"])),
        "sampling": {"warmup": 3, "measured": 5},
        "execution": "Native macOS; sequential, fresh process per implementation and size; unrelated services left running. Docker resource-pressure pilots excluded.",
        "small_fixture_provenance": "../revision-validation/provenance.json",
        "validation_settings": "Unchanged from corrected small-fixture drivers; no external resolution or online ontology work.",
        "java_heap": "Runtime defaults, no forced garbage collection.",
        "memory_method": "wait4 ru_maxrss, macOS bytes converted to KiB; median peak of three fresh processes each reading, parsing, and serializing once without validation or warmup; runtime and startup included. Timing-run peaks retained separately.",
        "memory_sampling": {"processes_per_case": 3, "warmup": 0, "measured": 1, "validate": False},
        "java_largest_budget_seconds": 300,
        "rustc": command(["rustc", "+1.93", "-Vv"]),
        "java": command([str(TOOLS / "jdk-17.0.20.1+1/Contents/Home/bin/java"), "-version"]),
        "node": command([str(TOOLS / "node-v20.20.2-darwin-arm64/bin/node"), "--version"]),
        "python": command(["/usr/local/bin/python3.12", "--version"]),
    }
    metadata["files"] = {str(path): identity(path) for path in (
        TOOLS / "libSBOLj3.jar", TOOLS / "Bench.java", TOOLS / "bench.py",
        TOOLS / "javascript/bench.js", TOOLS / "node.tar.gz", TOOLS / "java.tar.gz",
        TOOLS / "measure_rss.native", Path("/private/tmp/sbol-review-validation/target/release/runner"),
    )}
    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(TOOLS / "python-packages")
    environment["PYTHONNOUSERSITE"] = "1"
    packages = subprocess.check_output(["/usr/local/bin/python3.12", "-c", "from importlib.metadata import distributions; print('\\n'.join(sorted(d.metadata['Name']+'=='+d.version for d in distributions())))"], env=environment, text=True)
    metadata["python_environment_distributions"] = packages.splitlines()
    imports = subprocess.check_output(["/usr/local/bin/python3.12", "-c",
        "import importlib,json; from importlib.metadata import version; names={'sbol3':'sbol3','rdflib':'rdflib','pyshacl':'pyshacl','owlrl':'owlrl','isodate':'isodate','six':'six','html5lib':'html5lib','webencodings':'webencodings','prettytable':'prettytable','packaging':'packaging','pyparsing':'pyparsing'}; print(json.dumps({module:{'version':version(distribution),'file':importlib.import_module(module).__file__} for module,distribution in names.items()}))"], env=environment, text=True)
    metadata["python_active_packages"] = json.loads(imports)
    metadata["downloads"] = [
        "https://nodejs.org/dist/v20.20.2/node-v20.20.2-darwin-arm64.tar.gz",
        "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.20.1%2B1/OpenJDK17U-jdk_aarch64_mac_hotspot_17.0.20.1_1.tar.gz",
    ]
    (ROOT / "provenance.json").write_text(json.dumps(metadata, indent=2) + "\n")
    binary = json.loads((ROOT / "binary-size.json").read_text())
    if "linux_arm64" not in binary:
        binary = {"linux_arm64": binary}
    binary["macos_arm64"] = {
        "unstripped": identity(TOOLS / "sbol-macos.unstripped"),
        "stripped": identity(TOOLS / "sbol-macos"),
        "dynamic_dependencies": command(["otool", "-L", str(TOOLS / "sbol-macos")]),
        "rustc": metadata["rustc"], "release": "v1.0.0",
        "command": "cargo +1.93 build --locked --release -p sbol-cli; strip copy of target/release/sbol",
        "version": command([str(TOOLS / "sbol-macos"), "--version"]),
    }
    (ROOT / "binary-size.json").write_text(json.dumps(binary, indent=2) + "\n")
    print(metadata["platform"], metadata["cpu"], metadata["memory_bytes"])
    print("Stripped macOS CLI:", binary["macos_arm64"]["stripped"]["bytes"], "bytes")


if __name__ == "__main__":
    main()
