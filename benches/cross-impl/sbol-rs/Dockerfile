# sbol-rs round-trip benchmark image. The point of this image is to
# pay the same Linux-VM overhead the foreign benches do, so the
# sbol-rs column in the cross-impl table is apples-to-apples with
# pySBOL3, libSBOLj3, and sboljs.
#
# Unlike the other bench images, the build context is the workspace
# root (so the builder can see every workspace member), so the
# build command is:
#
#     docker build -t sbol-rs-bench -f benches/cross-impl/sbol-rs/Dockerfile .
#
# `.dockerignore` at the workspace root keeps target/, the fixture
# cache, and other large dirs out of the context.

FROM rust:1.93-slim-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# sbol's build.rs reads policy markdown for partial-blocker rules.
COPY docs ./docs

# Build only the runner binary (everything sbol depends on gets pulled
# in transitively). Release profile because that's what the user-facing
# library is shipped with; debug-built numbers would be misleading.
RUN cargo build --locked --release -p sbol-bench --bin runner

FROM debian:bookworm-slim AS runtime

COPY --from=builder /workspace/target/release/runner /usr/local/bin/sbol-rs-bench-runner

ENTRYPOINT ["sbol-rs-bench-runner"]
