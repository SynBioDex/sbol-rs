# sbol-workspace

Credential-free local project state for collection-oriented SBOL registry
synchronization. This crate owns the on-disk contracts used by `sbol init`,
`sbol status`, and `sbol sync`:

- `sbol.toml`, the human-owned declaration of tracked collection URIs,
  registries, and paths;
- `sbol.lock`, the generated common baseline of biological-content ETags and
  local SHA-256 hashes; and
- safe `designs/` paths, ancestor discovery, atomic writes, and local state
  classification.

`sbol init` creates the manifest and `designs/` directory but not the lock. The
first successful tracked pull or push writes `sbol.lock`. Both TOML files are
intended for version control and reject credential fields and path traversal.

Network calls and conflict policy live in `sbol-cli` and
`sbol-registry-client`; this crate deliberately contains no HTTP or credential
storage. See the full
[collection synchronization guide](../../docs/registry-workspaces.md).
