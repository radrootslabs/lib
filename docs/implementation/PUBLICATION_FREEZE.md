# Crates.io publication freeze

Crates.io publication is frozen for the complete release-v1 crate refactor.
Every workspace package, including packages intended to become public, must
set:

```toml
publish = false
```

`contracts/releases/publish_policy.toml` is the machine authority for this
freeze. Repository contract and release-preflight validation reject a missing
publication-control section, an unexpected registry, a different enablement
checkpoint, or any package that is implicitly or explicitly publishable.

The only planned exception is release plan Step 305, after the final package
inventory, resolved public dependency graph, API surface, target matrix, and
security gates are green. That checkpoint may set `publication.frozen = false`
and enable exactly the approved release packages for package-validation
staging. It does not authorize upload or any crates.io mutation.

Changing the freeze requires an independently reviewed release-control commit.
Actual publication, tag creation, registry ownership changes, and
trusted-publisher changes always require separate explicit authorization.
