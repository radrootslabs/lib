# Crates.io Policy

Public crate publication must use registry-resolved public dependencies. Release preflight must
fail closed while required DTO tooling is unavailable from crates.io.

## Publishable crate boundary

Crates that declare `publish = ["crates-io"]` are candidates for public registry publication after
their source, contracts, tests, and dependency graph pass the repo-local release preflight.

Crates that declare `publish = false` are public source crates but are not candidates for registry
publication from this repo-local policy surface.

## DTO tooling gate

The DTO crates `dto_bindgen` and `dto_bindgen_core` are currently required by public crate surfaces
that expose generated DTO metadata. Until the required DTO crates are available from crates.io,
`cargo xtask release preflight` must report that registry availability as a publication blocker.

That blocker must not be bypassed by:

- git dependencies
- local path dependencies
- vendored DTO copies
- retired-name reexports
- publication-only source rewrites
- release-only feature gates

The correct state is an explicit preflight failure that names the missing registry dependency.

## Local validation

Run the release gate from the nested repo root:

```bash
cargo xtask release preflight
```

If the command fails only because DTO tooling is unavailable from crates.io, record that exact
blocker in closeout evidence. Any additional failure must be treated as a separate source or
contract issue and fixed directly.
