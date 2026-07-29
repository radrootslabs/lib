# Dependency resolution

This standalone repository owns its `Cargo.lock`. Under `RCRV1-DEV-001`, the
release-v1 refactor does not combine it with the SDK lockfile or make either
repository depend on the other's workspace state.

Step 017 verified the current lock checksum as
`4462008577c9b46a97a01acce7efd34c95e98e46bc54468cb278f066f7943726`.
Repeated `cargo metadata --locked --no-deps --format-version 1` and the full
repository contract lane leave it unchanged.

Dependency changes must use repository-owned extbuild commands, preserve
`--locked` zero-diff validation, and update this evidence when the resolved
graph intentionally changes.
