# Collection synchronization with the `sbol` CLI

The `sbol` binary remains part of `sbol-rs`: local parsing, validation, and
format conversion live beside authenticated SBOL DB workflows in one tool.
An SBOL project adds a reproducible, collection-oriented synchronization layer
without putting credentials in the project.

## Start a project

```sh
mkdir toggle-project
cd toggle-project
sbol init
sbol registry pull https://sbol.io/public/toggle/toggle_collection/1
```

`sbol init` creates only the human-owned manifest and its design directory:

```text
toggle-project/
├── sbol.toml
└── designs/
```

The first successful tracked pull or push establishes a synchronized baseline
and creates the generated lock:

```text
toggle-project/
├── sbol.toml
├── sbol.lock
└── designs/
    └── toggle.ttl
```

Both TOML files are intended to be committed. Neither may contain a password,
access token, refresh token, or other credential. Registry credentials remain
in the CLI's private profile.

Commands find the nearest `sbol.toml` in the current directory or an ancestor,
so they work from a nested project directory. `sbol init --registry URL` stores
an optional default registry, which is useful when an institutional instance is
mounted below a path prefix.

## Manifest and lock responsibilities

`sbol.toml` is edited by people. It declares the project schema, design
directory, optional default registry, and each tracked collection's canonical
URI, registry, and local path:

```toml
schema_version = 1
designs_dir = "designs"
default_registry = "https://sbol.io/"

[collections.toggle]
uri = "https://sbol.io/public/toggle/toggle_collection/1"
registry = "https://sbol.io/"
path = "designs/toggle.ttl"
```

`sbol.lock` is written by successful synchronization. For every collection it
records the exact remote biological-content ETag and the SHA-256 of the local
serialized file at the common baseline:

```toml
schema_version = 1

[collections.toggle]
uri = "https://sbol.io/public/toggle/toggle_collection/1"
registry = "https://sbol.io/"
path = "designs/toggle.ttl"
remote_content_etag = '"sbol-content-v1-…"'
local_sha256 = "…"
```

The content ETag covers biological SBOL only. Sharing, ownership, review and
audit records, timestamps, and other server-managed metadata do not make a
clean design appear modified.

## Pull, push, status, and sync

Inside a project, omitting `--output` makes a collection pull tracked:

```sh
sbol registry pull https://sbol.io/public/toggle/toggle_collection/1
sbol status
```

The default path is `designs/<safe-display-id>.ttl`; a URI version segment is
never used as the filename. Use `--alias NAME` to choose the tracking key. An
explicit `--output` without `--alias` remains a one-shot download, even inside
a project. Outside a project, `--output` is required and no manifest or lock is
created.

Push a tracked file with the ordinary registry command:

```sh
sbol registry push designs/toggle.ttl --dry-run
sbol registry push designs/toggle.ttl
```

An initial, untracked push creates a collection and fails on identity collision
by default. A tracked push is different: it updates the checked-out collection
with `If-Match` against the exact ETag in `sbol.lock`. If another user or agent
changed the remote biological content, the server returns a precondition
failure and no data is overwritten.

`sbol sync` evaluates every selected collection before changing any of them:

| Local since lock | Remote since lock | Result |
|---|---|---|
| unchanged | unchanged | clean |
| changed | unchanged | push with ETag compare-and-swap |
| unchanged | changed | pull |
| changed | changed | conflict; do nothing implicitly |

A missing local file, missing remote collection, absent lock entry, or
simultaneous local and remote change is also a conflict. Synchronization never
infers deletion and never merges RDF automatically. Resolve those cases with an
explicit pull, push, manifest edit, or domain-aware merge, then establish a new
baseline. `sbol sync --dry-run` prints the selected actions without writing
files or registry data.

## Authentication and local development

The public workflow is:

```sh
sbol registry login
```

That selects `https://sbol.io` when deployed. An institutional registry is
explicit:

```sh
sbol registry login https://sbol.my-university.edu
```

During development, use the exact loopback origin printed by the local SBOL DB
server, including its chosen port:

```sh
sbol registry login http://127.0.0.1:8888
sbol init --registry http://127.0.0.1:8888
```

Production registry and OAuth transport URLs require HTTPS. Plain HTTP
transport is accepted only for `localhost` or a loopback IP address. Imported
legacy `http://` design identities remain usable when the registry is supplied
explicitly because the IRI is encoded into a request to the validated registry
rather than dereferenced as a network destination.
