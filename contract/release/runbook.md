# release runbook

## scope

This runbook applies to the crates classified in the owning monorepo release policy at
`contracts/release/mounted-rust-crates/publish-policy.toml`.

## preflight

```bash
scripts/release/rr-rs-preflight.sh <release-tag> [crate-list]
```

Run preflight from the owning monorepo release surface. In `radroots-platform-v1`, the canonical entrypoint is `scripts/release/rr-rs-preflight.sh`.

This validates:

- sdk contract integrity and release policy parity
- required crate coverage at `100/100/100/100`
- publish crate metadata required for crates.io

The underlying source-repo preflight lane remains `./scripts/ci/release_preflight.sh`, but publish orchestration is monorepo-owned and must not live in the source checkout.

## release tag

Create an annotated tag whose version matches `release.version` in
`contracts/release/mounted-rust-crates/publish-policy.toml`.

Recommended form:

```bash
git tag -a "v$(awk -F '\"' '/^version = / { print $2; exit }' ../../../../contracts/release/mounted-rust-crates/publish-policy.toml)" -m "release"
```

## publish simulation

```bash
scripts/release/rr-rs-preflight.sh <release-tag> [crate-list]
```

This runs `cargo publish --dry-run` in release order from the owning monorepo and reports deferred crates when they depend on earlier crates that are not yet published.

## publish

```bash
scripts/release/rr-rs-publish.sh <release-tag> [crate-list]
```

This publishes in `publish_order` from the owning monorepo and waits for each crate version to become visible on crates.io before continuing.

Set `CARGO_REGISTRY_TOKEN` or `CRATES_IO_TOKEN` in the runtime environment before the publish step.

## post-release verification

```bash
cargo run -q -p xtask -- sdk validate
cargo run -q -p xtask -- sdk release preflight
```

Then verify the published crate versions on crates.io.
