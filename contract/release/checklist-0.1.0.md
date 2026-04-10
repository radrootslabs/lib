# release checklist

- [ ] confirm `contract/manifest.toml`, `contract/version.toml`, and `contracts/release/mounted-rust-crates/publish-policy.toml` declare the same release version
- [ ] run `cargo check -q`
- [ ] run `cargo test -q -p xtask`
- [ ] run `./scripts/ci/release_preflight.sh`
- [ ] run `scripts/release/rr-rs-preflight.sh <release-tag> [crate-list]` from the owning monorepo
- [ ] confirm crates.io owner and token access for the publish account
- [ ] run `scripts/release/rr-rs-publish.sh <release-tag> [crate-list]` from the owning monorepo
- [ ] verify all public crates in the root release policy are visible on crates.io at the configured release version
- [ ] tag release in git and publish release notes
