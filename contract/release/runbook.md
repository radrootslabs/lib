# release runbook

## scope

This runbook applies to the crates listed in `contract/release/publish-set.toml`.

## preflight

```bash
./scripts/ci/release_preflight.sh
```

This command validates:

- sdk contract integrity and release policy parity
- required crate coverage at `100/100/100`
- publish crate metadata required for crates.io

## publish simulation

```bash
./scripts/ci/release_publish_order.sh dry-run
```

This runs `cargo publish --dry-run` in release order and reports deferred crates when they depend on earlier crates that are not yet published.

## publish

```bash
./scripts/ci/release_publish_order.sh publish
```

This publishes in `publish_order` and waits for each crate version to become visible on crates.io before continuing.

## post-release verification

```bash
cargo run -q -p xtask -- sdk validate
cargo run -q -p xtask -- sdk release preflight
```

Then verify the published crate versions on crates.io.
