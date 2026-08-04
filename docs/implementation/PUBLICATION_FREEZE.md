# Crates.io publication freeze

Crates.io upload remains frozen for the complete release-v1 crate refactor.
Step 305 enabled validation metadata for exactly the 17 approved public
packages:

```toml
publish = ["crates-io"]
```

`contracts/releases/publish_policy.toml` is the machine authority. Repository
contract and release-preflight validation reject an unexpected registry,
package, order, version, or enablement checkpoint; every private, preview,
build, and test-support package remains non-publishable.

This validation-only state permits packaging, crates.io dry-runs, and local
ephemeral-registry qualification. It does not authorize upload or any crates.io
mutation.

Changing the freeze requires an independently reviewed release-control commit.
Actual publication, tag creation, registry ownership changes, and
trusted-publisher changes always require separate explicit authorization.
