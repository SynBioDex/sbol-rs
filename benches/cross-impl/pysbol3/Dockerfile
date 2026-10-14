# pySBOL3 round-trip benchmark image. Pinned to the same pySBOL3
# release as the conformance harness in tests/fixtures/cross-impl-pysbol3/
# so the parse/serialize numbers we publish are tied to a known
# library version.
#
# Build:
#     docker build -t pysbol3-bench benches/cross-impl/pysbol3/
# Used by:
#     crates/sbol/src/bin/run-cross-impl-bench.rs
#
# IMPORTANT — bump PYSBOL3_REVISION when a new release ships. After
# bumping, rerun the bench driver and re-record any baseline numbers
# captured in benches/cross-impl/README.md.

FROM python:3.12-slim AS runtime

# PyPI's 1.2 wheel was yanked because it predates the release tag.
# Install the exact v1.2 source commit instead of that wheel.
ARG PYSBOL3_REVISION=e73a4624ecc22ba5828d0c308ee902bb816b65b2

WORKDIR /opt/pysbol3-bench

RUN pip install --no-cache-dir "rdflib==6.3.2" "pyshacl==0.28.1" \
    "https://github.com/SynBioDex/pySBOL3/archive/${PYSBOL3_REVISION}.tar.gz"

COPY bench.py /opt/pysbol3-bench/bench.py

ENTRYPOINT ["python3", "/opt/pysbol3-bench/bench.py"]
