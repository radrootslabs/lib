# radroots_sdk

Curated SDK contract for the Rad Roots cross-language SDK.

## Purpose

This directory defines the `radroots_sdk` contract used to align Rust,
TypeScript, Python, Swift, and Kotlin surfaces. It defines the public
interoperability boundary for external integrators, keeps Rust as the canonical
source for exported models and transforms, and enforces deterministic,
machine-verifiable governance for contract changes and releases.

## Contract Surface

SDK contract metadata is defined in `spec/manifest.toml` and currently includes:

- model crates: `radroots_core`, `radroots_events`, `radroots_trade`, `radroots_identity`
- algorithm crate: `radroots_events_codec`
- wasm crate: `radroots_events_codec_wasm`

The curated public Rust entrypoint is `radroots_sdk`.
The crate list above records implementation provenance for the contract surface;
it is not a promise that every listed crate is a first-class end-user SDK
package.

Public SDK exports are intentionally narrower than the full Rust workspace.

## Field Event Substrate

Field-oriented farming operations are represented in the public Rust substrate
through `radroots_events`, `radroots_events_codec`, and
`radroots_events_codec_wasm`.

The substrate includes workspace manifests, CRDT change envelopes, farm file
metadata, NIP-42 relay auth, NIP-98 HTTP auth, and the supported NIP-29 group
event subset covering `9000`, `9001`, `9002`, `9005`, `9007`, `9008`, `9009`,
`9021`, `9022`, `39000`, `39001`, `39002`, and `39003`. These are event and
codec APIs, not curated SDK operations by default. The active NIP-29 subset uses
bare metadata markers, `supported_kinds`, and `code` tags for invite and join
flows, and preserves optional user management and moderation reason content;
LiveKit room metadata and live participant state are deferred.

Task records, work sessions, harvest records, approvals, and similar Field
business objects are CRDT document semantics carried inside the CRDT change
envelope. They are outside the `rr-rs` event-contract boundary unless a future
contract slice explicitly promotes them into a curated SDK operation surface with
matching conformance vectors and language export mappings.

## Public Social Event Substrate

Public social events are represented as event and codec substrate in
`radroots_events`, `radroots_events_codec`, and `radroots_events_codec_wasm`.

The active social-event contract is defined in `spec/social-events.md`. It covers
ordinary posts, comments, reactions, articles, public generic file metadata,
calendar events, reposts, reports, listing drafts through `RadrootsListing`, and
NIP-65 relay lists through `RadrootsList`.

The social surface is substrate-first. MVP social tag builders for posts,
comments, reactions, articles, generic public file metadata, calendar date
events, and calendar time events are promoted into curated SDK operation
metadata after their Rust models, codecs, wasm helpers, and deterministic
conformance vectors exist. Production-v1 repost, report, calendar collection,
and RSVP behavior remains available through event and codec APIs by default and
is covered by conformance vectors.

## Rust Crate Tiers

The public Rust story is tiered explicitly.

- Curated SDK entrypoint:
  - `radroots_sdk`
- Advanced substrate crates:
  - `radroots_core`
  - `radroots_events`
  - `radroots_events_codec`
  - `radroots_trade`
  - `radroots_identity`
  - `radroots_nostr`
  - `radroots_nostr_connect`
  - `radroots_nostr_signer`
  - `radroots_nostr_accounts`
  - `radroots_secret_vault`
  - `radroots_protected_store`
  - `radroots_runtime_paths`
- Published support crates:
  - `radroots_log`
  - `radroots_runtime`
  - `radroots_runtime_distribution`
  - `radroots_runtime_manager`
  - `radroots_geocoder`
  - `radroots_events_indexed`
  - `radroots_sql_core`
  - `radroots_replica_db_schema`
  - `radroots_replica_db`
  - `radroots_replica_sync`
- Deferred crates.io publication:
  - `radroots_types`
  - `radroots_event_store`
  - `radroots_events_codec_wasm`
  - `radroots_net`
  - `radroots_nostr_runtime`
  - `radroots_nostr_ndb`
  - `radroots_sql_wasm_bridge`
  - `radroots_sql_wasm_core`
  - `radroots_replica_db_wasm`
  - `radroots_replica_sync_wasm`
  - `radroots_simplex_chat_proto`
  - `radroots_simplex_smp_proto`

This tiering is the curated product posture for crates.io. A crate may remain
open source and part of the `rr-rs` workspace without being a recommended
external SDK entrypoint or an active crates.io publication target.

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
- `spec/sdk-exports/go.toml`

Lower-level language package mappings and artifact layout rules remain defined
under `spec/exports/`:

- `spec/exports/ts.toml`
- `spec/exports/py.toml`
- `spec/exports/swift.toml`
- `spec/exports/kotlin.toml`
- `spec/exports/go.toml`

The `sdk-exports` files are the authoritative public package model.
The `exports` files remain the lower-level substrate and artifact mapping layer.
For every language target, that lower-level provenance must still resolve to
the same single curated SDK package defined in `sdk-exports/`, rather than a
crate-mirrored package set.

Rollout order is also explicit in `sdk-exports/`:

- TypeScript is active now
- Swift and Kotlin are next
- Python and Go remain deferred until the Rust and TypeScript lines are proven

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

## Coverage Policy

Coverage governance is defined under `policy/coverage/`:

- machine-readable policy: `policy/coverage/policy.toml`
- human policy notes: `policy/coverage/POLICY.md`
- per-crate profiles: `policy/coverage/profiles.toml`

Required Rust crates are gated at `90/90/90/90` (exec lines, functions,
branches, regions), with branch records required. This is not a 100% coverage
target. Temporary crate-specific overrides below 90% must remain explicit in
the machine-readable policy.

## Release Policy

Release crate classification and publish order are defined in the owning monorepo at
`foundation/contracts/release_runtime/mounted_rust_crates/publish-policy.toml`.
Operator workflow is root-owned and documented in:

- `docs/operations/runbooks/mounted-rust-crate-release.md`
- `docs/operations/runbooks/mounted-rust-crate-release-checklist.md`

Primary commands:

- `cargo run -q -p xtask -- sdk validate`
- `cargo run -q -p xtask -- sdk release preflight`
- `./scripts/ci/release_preflight.sh`
- `scripts/release/rr-rs-preflight.sh <plan-id> [crate-list]` from the owning monorepo

## License

Licensed under AGPL-3.0. See LICENSE.
