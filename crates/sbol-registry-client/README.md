# sbol-registry-client

Typed, synchronous HTTP client for the registry workflows used by the
`sbol` CLI.

This crate keeps remote SBOL DB protocol details out of `sbol-cli` while the
CLI itself remains part of the `sbol-rs` repository. It currently covers:

- instance and machine-capability discovery;
- canonical-design download;
- compatibility login returning an opaque bearer token; and
- submission preview and commit with explicit collision policies.

```rust,no_run
use sbol_registry_client::{RegistryClient, SbolVersion};

let client = RegistryClient::new("https://sbol.io")?;
let design = client.pull(
    "https://sbol.io/public/igem/BBa_J23100/1",
    SbolVersion::V3,
)?;
std::fs::write("design.rdf", design.body)?;

# Ok::<(), Box<dyn std::error::Error>>(())
```

Bearer tokens are opt-in and are redacted from `Debug` output:

```rust,no_run
use sbol_registry_client::RegistryClient;

let client = RegistryClient::new("https://sbol.io")?
    .with_bearer_token(std::env::var("SBOL_ACCESS_TOKEN")?);
let instance = client.instance()?;
println!("{}", instance.name);

# Ok::<(), Box<dyn std::error::Error>>(())
```

The crate models the SBOL DB V2 contract rather than presenting a generic RDF
HTTP abstraction. Authentication storage, prompts, local file conversion, and
command-line output remain responsibilities of `sbol-cli`.
