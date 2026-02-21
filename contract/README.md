# Rad Roots SDK contract charter

## purpose

The Rad Roots SDK contract defines the public, cross-language interface for interacting with the Rad Roots network profile on nostr.

This contract is the compatibility boundary for external integrators.

## principles

- rust is the canonical contract source.
- exported sdk surfaces are intentionally narrower than the full rust workspace.
- deterministic transforms must be generated from canonical implementations.
- language runtimes may implement networking natively if conformance is preserved.
- contract evolution is semver-governed and must remain machine-verifiable.

## scope

The sdk contract includes only public interoperability primitives:

- event models and kind constants
- identity and trade model surfaces
- canonical tag and codec transforms
- schema and conformance fixtures

The sdk contract excludes app/runtime/storage implementation crates.

## governance

- all contract changes require conformance updates.
- all contract exports must be reproducible from source.
- release automation must publish contract metadata and artifact checksums.

## coverage governance

- strict coverage policy for oss rust crates is defined in `contract/coverage/POLICY.md`.
- crate rollout and enforcement order is defined in `contract/coverage/rollout.toml`.
