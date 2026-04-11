# radroots-sdk-contract

Core contract for the Rad Roots cross-language SDK.

## Purpose

This directory defines the Rad Roots SDK contract used to align Rust, TypeScript, Python, Swift, and Kotlin surfaces.
It defines the public interoperability boundary for external integrators, keeps Rust as the canonical source for exported models and transforms, and enforces deterministic, machine-verifiable governance for contract changes and releases.

## Contract Surface

Contract metadata is defined in `spec/manifest.toml` and currently includes:

- model crates: `radroots_core`, `radroots_types`, `radroots_events`, `radroots_trade`, `radroots_identity`
- algorithm crate: `radroots_events_codec`
- wasm crate: `radroots_events_codec_wasm`

Public SDK exports are intentionally narrower than the full Rust workspace.

## Export Targets

Language export mappings and artifact layout rules are defined under `spec/exports/`:

- `spec/exports/ts.toml`
- `spec/exports/py.toml`
- `spec/exports/swift.toml`
- `spec/exports/kotlin.toml`

Each export target defines package naming, artifact directories, and runtime expectations.

## Internal Replica Contract

Offline-first replica crates are internal contract surfaces and are not public SDK exports.
Replica contract metadata is defined in `spec/replica.toml`.

Internal replica crate family:

- `radroots_replica_db_schema`
- `radroots_replica_db`
- `radroots_replica_db_wasm`
- `radroots_replica_sync`
- `radroots_replica_sync_wasm`

## Governance

Versioning and compatibility policy is defined in `spec/version.toml`.
Contract evolution is semver-governed and requires conformance updates, export manifest validation, and release notes.

Repository guards also enforce:

- deterministic export requirements
- strict no-legacy identifier policy for replica surfaces
- no committed generated TypeScript artifacts in repo export directories (`target/ts-rs/` and `target/sdk-export-ci/`)

## Coverage Policy

Coverage governance is defined under `policy/coverage/`:

- machine-readable policy: `policy/coverage/policy.toml`
- human policy notes: `policy/coverage/POLICY.md`
- per-crate profiles: `policy/coverage/profiles.toml`

Required Rust crates are gated at `100/100/100/100` (exec lines, functions, branches, regions), with branch records required.

## Release Policy

Release crate classification and publish order are defined in the owning monorepo at
`contracts/release/mounted-rust-crates/publish-policy.toml`.
Operator workflow is root-owned and documented in:

- `contracts/release/mounted-rust-crates/runbook.md`
- `contracts/release/mounted-rust-crates/checklist.md`

Primary commands:

- `cargo run -q -p xtask -- sdk validate`
- `cargo run -q -p xtask -- sdk release preflight`
- `./scripts/ci/release_preflight.sh`
- `scripts/release/rr-rs-preflight.sh <plan-id> [crate-list]` from the owning monorepo

## License

Licensed under AGPL-3.0. See LICENSE.
