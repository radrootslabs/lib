# release runbook

## scope

This runbook applies to the crates listed in `contract/release/publish-set.toml`.

## preflight

```bash
nix run .#release-preflight
```

This command validates:

- sdk contract integrity and release policy parity
- required crate coverage at `100/100/100/100`
- publish crate metadata required for crates.io

The underlying repo-owned entrypoint is `./scripts/ci/release_preflight.sh`.
External release automation should call the canonical local preflight and must not replace it with forge-specific logic.

## release tag

Create an annotated tag whose version matches `release.version` in `contract/release/publish-set.toml`.

Recommended form:

```bash
git tag -a "v$(awk -F '\"' '/^version = / { print $2; exit }' contract/release/publish-set.toml)" -m "release"
```

## publish simulation

```bash
nix run .#publish-dry-run
```

This runs `cargo publish --dry-run` in release order and reports deferred crates when they depend on earlier crates that are not yet published.

## publish

```bash
nix run .#publish-crates -- --publish
```

This publishes in `publish_order` and waits for each crate version to become visible on crates.io before continuing.

Set `CARGO_REGISTRY_TOKEN` or `CRATES_IO_TOKEN` in the runtime environment before the publish step.

## post-release verification

```bash
cargo run -q -p xtask -- sdk validate
cargo run -q -p xtask -- sdk release preflight
```

Then verify the published crate versions on crates.io.
