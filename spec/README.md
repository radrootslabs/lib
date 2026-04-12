# radroots_sdk_contract

Core contract for the Rad Roots cross-language SDK.

## Purpose

This directory defines the Rad Roots SDK contract used to align Rust, TypeScript, Python, Swift, and Kotlin surfaces.
It defines the public interoperability boundary for external integrators, keeps Rust as the canonical source for exported models and transforms, and enforces deterministic, machine-verifiable governance for contract changes and releases.

## Contract Surface

Contract metadata is defined in `spec/manifest.toml` and currently includes:

- model crates: `radroots_core`, `radroots_events`, `radroots_trade`, `radroots_identity`
- algorithm crate: `radroots_events_codec`
- wasm crate: `radroots_events_codec_wasm`

The curated public Rust entrypoint is `radroots_sdk`.
The crate list above records implementation provenance for the contract surface;
it is not a promise that every listed crate is a first-class end-user SDK
package.

Public SDK exports are intentionally narrower than the full Rust workspace.

## Export Targets

Language export metadata is split into two layers:

- `spec/sdk-exports/`: curated public SDK package definitions, operation maps,
  and shared-type maps
- `spec/exports/`: lower-level package and artifact provenance mappings used by
  tooling and generated artifact layout

Curated public SDK package definitions are defined under `spec/sdk-exports/`:

- `spec/sdk-exports/ts.toml`
- `spec/sdk-exports/swift.toml`
- `spec/sdk-exports/kotlin.toml`
- `spec/sdk-exports/py.toml`

Lower-level language package mappings and artifact layout rules remain defined
under `spec/exports/`:

- `spec/exports/ts.toml`
- `spec/exports/py.toml`
- `spec/exports/swift.toml`
- `spec/exports/kotlin.toml`

The `sdk-exports` files are the authoritative public package model.
The `exports` files remain the lower-level substrate and artifact mapping layer.

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
Contract evolution is semver-governed and requires conformance updates, export target validation, and release notes.

Repository guards also enforce:

- deterministic export requirements
- strict no-legacy identifier policy for replica surfaces
- no committed generated TypeScript artifacts under `target/ts-rs/`

## Coverage Policy

Coverage governance is defined under `policy/coverage/`:

- machine-readable policy: `policy/coverage/policy.toml`
- human policy notes: `policy/coverage/POLICY.md`
- per-crate profiles: `policy/coverage/profiles.toml`

Required Rust crates are gated at `100/100/100/100` (exec lines, functions, branches, regions), with branch records required.

## Release Policy

Release crate classification and publish order are defined in the owning monorepo at
`contracts/release/mounted_rust_crates/publish-policy.toml`.
Operator workflow is root-owned and documented in:

- `contracts/release/mounted_rust_crates/runbook.md`
- `contracts/release/mounted_rust_crates/checklist.md`

Primary commands:

- `cargo run -q -p xtask -- sdk validate`
- `cargo run -q -p xtask -- sdk release preflight`
- `./scripts/ci/release_preflight.sh`
- `scripts/release/rr-rs-preflight.sh <plan-id> [crate-list]` from the owning monorepo

## License

Licensed under AGPL-3.0. See LICENSE.
