# 0.1.0 release checklist

- [ ] confirm `contract/version.toml` and `contract/release/publish-set.toml` both declare `0.1.0`
- [ ] run `cargo check -q`
- [ ] run `cargo test -q -p xtask`
- [ ] run `./scripts/ci/release_preflight.sh`
- [ ] run `./scripts/ci/release_publish_order.sh dry-run`
- [ ] confirm crates.io owner and token access for the publish account
- [ ] run `./scripts/ci/release_publish_order.sh publish`
- [ ] verify all publish-set crates are visible on crates.io at `0.1.0`
- [ ] tag release in git and publish release notes
