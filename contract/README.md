# radroots-sdk-contract

Core contract for the Rad Roots cross-language SDK.

## Purpose

This directory defines the Rad Roots SDK contract used to align Rust, TypeScript, Python, Swift, and Kotlin surfaces.
It defines the public interoperability boundary for external integrators, keeps Rust as the canonical source for exported models and transforms, and enforces deterministic, machine-verifiable governance for contract changes and releases.

## Contract Surface

Contract metadata is defined in `contract/manifest.toml` and currently includes:

- model crates: `radroots-core`, `radroots-types`, `radroots-events`, `radroots-trade`, `radroots-identity`
- algorithm crate: `radroots-events-codec`
- wasm crate: `radroots-events-codec-wasm`

Public SDK exports are intentionally narrower than the full Rust workspace.

## Export Targets

Language export mappings and artifact layout rules are defined under `contract/exports/`:

- `contract/exports/ts.toml`
- `contract/exports/py.toml`
- `contract/exports/swift.toml`
- `contract/exports/kotlin.toml`

Each export target defines package naming, artifact directories, and runtime expectations.

## Internal Replica Contract

Offline-first replica crates are internal contract surfaces and are not public SDK exports.
Replica contract metadata is defined in `contract/replica.toml`.

Internal replica crate family:

- `radroots-replica-db-schema`
- `radroots-replica-db`
- `radroots-replica-db-wasm`
- `radroots-replica-sync`
- `radroots-replica-sync-wasm`

## Governance

Versioning and compatibility policy is defined in `contract/version.toml`.
Contract evolution is semver-governed and requires conformance updates, export manifest validation, and release notes.

Repository guards also enforce:

- deterministic export requirements
- strict no-legacy identifier policy for replica surfaces
- no committed generated TypeScript artifacts under crate bindings

## Coverage Policy

Coverage governance is defined under `contract/coverage/`:

- machine-readable policy: `contract/coverage/policy.toml`
- human policy notes: `contract/coverage/POLICY.md`
- per-crate profiles: `contract/coverage/profiles.toml`

Required Rust crates are gated at `100/100/100/100` (exec lines, functions, branches, regions), with branch records required.

## Release Policy

Release crate set and publish order are defined in `contract/release/publish-set.toml`.
Deterministic release workflow is defined in `contract/release/runbook.md`.
Release checklist is defined in `contract/release/checklist-0.1.0.md`.

Primary commands:

- `cargo run -q -p xtask -- sdk validate`
- `cargo run -q -p xtask -- sdk release preflight`
- `./scripts/ci/release_preflight.sh`
- `./scripts/ci/release_publish_order.sh dry-run`

## License

Licensed under AGPL-3.0. See LICENSE.
