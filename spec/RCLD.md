# Radroots Cross-Language SDK Contract Design

Status: approved direction, design artifact

Scope: public Radroots SDK contract for external language SDKs derived from the Rust workspace in this repository

Canonical source: Rust remains the canonical implementation and conformance source for public contract behavior

## Purpose

This document defines the approved operation-first design for the Radroots cross-language SDK contract.

It replaces the crate-first mental model currently expressed in `spec/manifest.toml` with a public contract shaped around external integration tasks:

- produce Radroots-compliant Nostr events
- parse Radroots-compliant Nostr events
- validate Radroots-compliant contract behavior
- preserve deterministic cross-language behavior for supported operations

This document does not require the Rust workspace to stop using crate boundaries internally. Crates remain implementation and provenance boundaries inside Rust. Operations become the public SDK boundary.

## Problem

The current repository expresses SDK surface primarily in terms of Rust crates:

- `surface.model_crates`
- `surface.algorithm_crates`
- `surface.wasm_crates`
- language export manifests keyed by Rust crate name
- `xtask` validation and export logic that assume crate-to-package mapping

That framing is not aligned with the needs of third-party integrators. Integrators do not want a mirror of the Rust workspace. They want a small, stable, idiomatic SDK that helps them publish and read Radroots-compliant Nostr events.

The codebase already contains the correct technical boundary:

- event model types in `radroots_events`, `radroots_trade`, `radroots_identity`, and supporting model crates
- deterministic builders and parsers in `radroots_events_codec`
- shared unsigned event primitives in `radroots_events_codec::wire`
- optional wasm packaging for deterministic helper logic

The contract must move upward from crate inventory to public operations.

## Decisions Ratified

The following decisions are approved and are treated as default design constraints:

- external SDKs optimize first for third-party app integrations, not for full Radroots internal app parity
- publishing is the first-class use case
- reading and validation are supported for the same Tier 1 domains, but remain secondary to publishing
- Tier 1 domains are `profile`, `farm`, `listing`, and `trade`
- the public contract unit is an operation, not a crate
- networking and signing remain native to each target language
- TypeScript is the first reference SDK for the new contract
- Python follows after TypeScript proves the operation model
- Rust crate names are not part of the public SDK mental model
- migration should be additive first and support old and new manifest shapes during transition

## Goals

- define a stable public SDK contract in terms of operations
- preserve Rust as the canonical behavioral implementation
- support idiomatic language SDKs without requiring full Rust API parity
- keep transport, relay IO, and signing runtime-native
- make deterministic encode, parse, normalize, and validate behavior conformance-testable
- provide a migration path from the current crate-keyed contract and export system

## Non-Goals

- exporting every Rust function to every language
- standardizing one shared Nostr client implementation across languages
- exposing Radroots app-internal marketplace, replica, moderation, or backoffice surfaces as public SDK APIs
- introducing a flag-day rewrite of the entire contract and export toolchain

## External Audience

The public SDK contract is designed for:

- third-party apps publishing Radroots-compliant profiles, farms, listings, and trade events
- apps that need to parse or validate those supported event families
- language SDK maintainers implementing contract-compliant APIs in TypeScript, Python, Swift, and Kotlin

The public SDK contract is not designed for:

- exposing Radroots internal admin flows
- exposing internal replica storage contracts
- exposing internal moderation and backoffice read models

## Public Contract Principles

1. Operations are public. Crates are internal.
2. Inputs, outputs, and errors are explicit.
3. Deterministic behavior is contract material.
4. Cross-language conformance is mandatory for approved operations.
5. Runtime choices such as relay transport and signer integration remain language-native.
6. Public surface must be narrower than the full Rust workspace.
7. Internal app-specific projections are excluded unless explicitly promoted.

## Public Surface Taxonomy

The public SDK contract has four surface classes:

### 1. Operations

Task-oriented public entry points for supported domains.

Examples:

- `profile.build_draft`
- `farm.build_draft`
- `listing.build_tags`
- `listing.build_draft`
- `listing.parse_event`
- `trade.build_envelope_draft`
- `trade.parse_envelope`
- `trade.parse_listing_address`
- `trade.validate_listing_event`

### 2. Shared Types

Public cross-operation types required for operation inputs and outputs.

Examples:

- `WireEventParts`
- `UnsignedEventDraft`
- `RadrootsNostrEvent`
- `RadrootsNostrEventRef`
- `RadrootsTradeListingAddress`

### 3. Shared Errors

Public error categories and domain-specific parse and validation errors that languages must preserve semantically.

Examples:

- event encode errors
- listing parse errors
- trade envelope parse errors
- listing validation errors

### 4. Implementation Provenance

Rust crate and wasm provenance used by maintainers and tooling, but not treated as the public contract unit.

Examples:

- operation implemented in `radroots_events_codec`
- type defined in `radroots_events`
- deterministic helper exposed via `radroots_events_codec_wasm`

## Tier 1 Domains And Operations

The initial approved public domains are `profile`, `farm`, `listing`, and `trade`.

The following operations form the recommended Tier 1 surface.

### Profile

#### `profile.build_draft`

Purpose: produce an unsigned Nostr event draft for a Radroots profile event

Rust implementation sources:

- `crates/events_codec/src/profile/encode.rs`

Input:

- `RadrootsProfile`
- optional `RadrootsProfileType`

Output:

- `WireEventParts`
- optional `UnsignedEventDraft` helper via shared draft adapter

Determinism:

- required

Runtime ownership:

- signing: native
- transport: native

### Farm

#### `farm.build_draft`

Purpose: produce an unsigned Nostr event draft for a farm event

Rust implementation sources:

- `crates/events_codec/src/farm/encode.rs`

Input:

- `RadrootsFarm`

Output:

- `WireEventParts`

Determinism:

- required

Runtime ownership:

- signing: native
- transport: native

### Listing

#### `listing.build_tags`

Purpose: produce canonical listing tags without creating a full unsigned event

Rust implementation sources:

- `crates/events_codec/src/listing/encode.rs`
- `crates/events_codec/src/listing/tags.rs`

Input:

- `RadrootsListing`

Output:

- `Vec<Vec<String>>`

Determinism:

- required

#### `listing.build_draft`

Purpose: produce an unsigned listing event contract from a listing model

Rust implementation sources:

- `crates/events_codec/src/listing/encode.rs`
- `crates/events_codec/src/wire.rs`

Input:

- `RadrootsListing`
- optional listing kind override when explicitly allowed

Output:

- `WireEventParts`
- optionally adapted to `UnsignedEventDraft`

Determinism:

- required

#### `listing.parse_event`

Purpose: parse a listing event into the canonical listing model

Rust implementation sources:

- `crates/trade/src/listing/codec.rs`
- `crates/events_codec/src/listing/decode.rs`

Input:

- `RadrootsNostrEvent`

Output:

- `RadrootsListing`

Determinism:

- required

### Trade

#### `trade.build_envelope_draft`

Purpose: produce an unsigned trade envelope event from typed trade payload input

Rust implementation sources:

- `crates/events_codec/src/trade/encode.rs`

Input:

- recipient pubkey
- trade message type
- listing address
- optional order id
- optional listing event pointer
- optional root event id
- optional previous event id
- typed trade payload

Output:

- `WireEventParts`

Determinism:

- required

#### `trade.parse_envelope`

Purpose: parse a trade event into a typed trade envelope

Rust implementation sources:

- `crates/events_codec/src/trade/decode.rs`

Input:

- `RadrootsNostrEvent`

Output:

- typed `RadrootsTradeEnvelope<T>`

Determinism:

- required

#### `trade.parse_listing_address`

Purpose: parse and validate the canonical listing address used by trade flows

Rust implementation sources:

- `crates/events_codec/src/trade/decode.rs`

Input:

- listing address string

Output:

- `RadrootsTradeListingAddress`

Determinism:

- required

#### `trade.validate_listing_event`

Purpose: validate that an event meets Radroots listing contract expectations for trade workflows

Rust implementation sources:

- `crates/trade/src/listing/validation.rs`

Input:

- `RadrootsNostrEvent`
- optional fetched dependencies if the validation path requires them

Output:

- validation result structure or domain validation error

Determinism:

- required for local validation logic
- explicitly scoped where external dependency fetch is involved

## Shared Types

The public contract should explicitly enumerate a minimal shared type set.

Recommended Tier 1 shared types:

- `WireEventParts`
- `UnsignedEventDraft`
- `RadrootsNostrEvent`
- `RadrootsNostrEventRef`
- `RadrootsNostrEventPtr`
- `RadrootsTradeListingAddress`
- public model types required by Tier 1 operations:
- `RadrootsProfile`
- `RadrootsFarm`
- `RadrootsListing`
- trade payload and envelope types required by approved trade operations

`UnsignedEventDraft` should be a public contract alias or wrapper over the current `EventDraft` concept in `crates/events_codec/src/wire.rs`. The public naming should emphasize unsigned event construction rather than internal adapter mechanics.

## Shared Errors

The contract should distinguish between:

- semantic error categories that are part of the public API
- internal implementation error types that can be mapped privately

Recommended public error classes:

- `encode_error`
- `parse_error`
- `validation_error`
- `address_error`

Recommended public semantic guarantees:

- required-field failures remain distinguishable
- invalid-kind failures remain distinguishable
- invalid-json failures remain distinguishable where applicable
- domain-specific parse mismatches remain distinguishable for listing and trade operations

Language SDKs may translate exact type names, but they must preserve error meaning and conformance behavior.

## Explicit Exclusions From The Public SDK

The following surfaces remain internal unless separately promoted:

- `radroots_replica_*` surfaces
- backoffice overlays
- marketplace read models and projections
- internal moderation models
- full `radroots_nostr` client runtime
- internal runtime management contracts

This exclusion is important because the Rust workspace contains valuable internal app surfaces that are not appropriate to freeze as external SDK contract.

## Runtime Ownership Model

The cross-language contract owns:

- deterministic model encode behavior
- parse behavior
- validation behavior
- canonical tags and content construction
- canonical address and pointer parsing

The language runtime owns:

- relay transport
- signer integration
- key management
- subscription lifecycle
- connection policies
- local storage and caching choices

This means SDKs should primarily produce and consume unsigned or already-signed event shapes rather than wrapping one shared transport stack.

## Package Strategy

### Public Package Strategy

The public package strategy should be operation-first and ergonomic.

Recommended TypeScript package strategy:

- one main package, for example `@radroots/sdk`
- one optional deterministic helper package or embedded asset for wasm-backed helpers

Recommended Python package strategy:

- one main package, for example `radroots_sdk`
- optional implementation-private native or wasm helper assets

Recommended Swift and Kotlin strategy:

- one main package or module namespace per language
- helper implementation details remain private unless explicitly useful

### What Not To Ship

Do not use crate-mirror packages as the primary public shape:

- `@radroots/core`
- `@radroots/types`
- `@radroots/events`
- `@radroots/trade`
- `@radroots/identity`

Those may remain transitional or internal build artifacts, but they should not be the product definition for external integrators.

## Contract Schema v2

The new contract should be additive first. The repository should support both:

- the existing crate-keyed contract metadata
- a new operation-keyed contract schema

The operation-keyed schema should become the public source of truth. Crate metadata should become provenance or migration-only data.

### Recommended Top-Level Manifest Shape

```toml
[contract]
name = "radroots-sdk-contract"
version = "0.2.0-alpha.1"
source = "rust"
stability = "draft"

[public]
domains = ["profile", "farm", "listing", "trade"]

[shared_types]
public = [
  "WireEventParts",
  "UnsignedEventDraft",
  "RadrootsNostrEvent",
  "RadrootsNostrEventRef",
  "RadrootsNostrEventPtr",
  "RadrootsTradeListingAddress",
  "RadrootsProfile",
  "RadrootsFarm",
  "RadrootsListing",
]

[errors]
classes = ["encode_error", "parse_error", "validation_error", "address_error"]

[operations.profile_build_draft]
domain = "profile"
id = "profile.build_draft"
stability = "beta"
inputs = ["RadrootsProfile", "RadrootsProfileType?"]
outputs = ["WireEventParts"]
error_class = "encode_error"
deterministic = true
signing = "native"
transport = "native"

[operations.profile_build_draft.implementation]
rust_modules = ["crates/events_codec/src/profile/encode.rs"]
rust_types = ["radroots_events::profile::RadrootsProfile"]

[operations.profile_build_draft.conformance]
vector = "spec/conformance/vectors/profile/build_draft.v1.json"

[operations.listing_build_draft]
domain = "listing"
id = "listing.build_draft"
stability = "beta"
inputs = ["RadrootsListing"]
outputs = ["WireEventParts"]
error_class = "encode_error"
deterministic = true
signing = "native"
transport = "native"

[operations.listing_build_draft.implementation]
rust_modules = [
  "crates/events_codec/src/listing/encode.rs",
  "crates/events_codec/src/listing/tags.rs",
  "crates/events_codec/src/wire.rs",
]

[operations.listing_build_draft.conformance]
vector = "spec/conformance/vectors/listing/build_draft.v1.json"
```

### Provenance Section

During migration, crate provenance should remain available:

```toml
[implementation_provenance]
model_crates = [
  "radroots_core",
  "radroots_types",
  "radroots_events",
  "radroots_trade",
  "radroots_identity",
]
algorithm_crates = ["radroots_events_codec"]
wasm_crates = ["radroots_events_codec_wasm"]
```

This keeps current workspace knowledge available without making it the public contract unit.

## Language Export Manifest v2

Language export manifests should stop mapping crate names to packages as the primary concept.

Instead they should answer:

- which operations are supported in the language
- where those operations are exposed
- how deterministic logic is implemented
- which shared types and error classes are public

### Recommended TypeScript Export Shape

```toml
[language]
id = "ts"
repository = "sdk-typescript"

[sdk]
package = "@radroots/sdk"
module_format = "esm"
deterministic_codec = "wasm"
signing = "native"
networking = "native"

[operations]
"profile.build_draft" = "profile.buildDraft"
"farm.build_draft" = "farm.buildDraft"
"listing.build_tags" = "listing.buildTags"
"listing.build_draft" = "listing.buildDraft"
"listing.parse_event" = "listing.parseEvent"
"trade.build_envelope_draft" = "trade.buildEnvelopeDraft"
"trade.parse_envelope" = "trade.parseEnvelope"
"trade.parse_listing_address" = "trade.parseListingAddress"
"trade.validate_listing_event" = "trade.validateListingEvent"

[shared_types]
"WireEventParts" = "WireEventParts"
"UnsignedEventDraft" = "UnsignedEventDraft"
"RadrootsNostrEvent" = "RadrootsNostrEvent"
"RadrootsTradeListingAddress" = "TradeListingAddress"

[artifacts]
models_dir = "src/generated"
runtime_dir = "src/runtime"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
```

Equivalent manifests for Python, Swift, and Kotlin should use their own naming conventions.

## Export Manifest Output

`xtask` should write an export manifest that includes operation coverage metadata, not only file hashes.

Recommended structure:

```json
{
  "language": "ts",
  "sdk_package": "@radroots/sdk",
  "operations": [
    {
      "id": "listing.build_draft",
      "symbol": "listing.buildDraft",
      "deterministic_codec": "wasm"
    }
  ],
  "files": [
    {
      "path": "src/generated/listing.ts",
      "sha256": "..."
    }
  ]
}
```

## Conformance Model

Conformance becomes the real multi-language product gate for the public contract.

### Rules

- every public operation must have at least one conformance vector suite
- deterministic operations require positive and negative vectors
- parse operations require round-trip or semantic equivalence vectors where applicable
- error behavior that is part of the contract must be vectorized
- language SDKs must pass conformance without local overrides

### Recommended Vector Layout

```text
spec/conformance/
  vectors/
    profile/
      build_draft.v1.json
    farm/
      build_draft.v1.json
    listing/
      build_tags.v1.json
      build_draft.v1.json
      parse_event.v1.json
    trade/
      build_envelope_draft.v1.json
      parse_envelope.v1.json
      parse_listing_address.v1.json
      validate_listing_event.v1.json
```

### Minimum Vector Coverage

For each operation:

- one minimal valid case
- one rich valid case
- one canonical normalization case if normalization exists
- one required-field failure case
- one invalid-format or invalid-kind failure case where applicable

## Versioning Policy v2

The contract version policy must shift from exported crate surface to operation semantics.

### Major Version Triggers

- remove a public operation
- change required operation input shape
- change output shape incompatibly
- change deterministic operation behavior incompatibly
- collapse or remove a public error distinction

### Minor Version Triggers

- add a public operation
- add optional input or output fields
- add a new public shared type
- add new conformance vectors that extend supported behavior without breaking old behavior

### Patch Version Triggers

- documentation fixes
- packaging fixes without behavior changes
- non-behavioral codegen fixes
- bug fixes that do not change the contract shape and do not invalidate existing conforming clients

## Rust Implementation Strategy

The Rust workspace should add a curated facade for approved public operations.

Recommended shape:

- add a new crate or dedicated public module namespace, for example `radroots_sdk_contract`
- re-export only approved operations and approved shared types
- keep direct crate internals available for Rust maintainers but do not treat them as the cross-language contract by default

This facade should:

- define public operation names
- define any public wrapper naming such as `UnsignedEventDraft`
- centralize contract documentation and source references
- make it easier for generators and future language bindings to target one approved surface

## `xtask` Migration Strategy

### Current State

`xtask` currently:

- parses a crate-keyed surface
- validates crate-keyed export coverage
- exports TypeScript by iterating crate-to-package mappings
- has tests that assert TypeScript export coverage matches model plus wasm crates

### Required Changes

1. add new manifest parsing structs for operation-based contract metadata
2. support dual parsing during migration
3. introduce validation for:
- non-empty public domain list
- unique operation ids
- known shared type references
- conformance vector presence for each public operation
- language export manifests mapping approved operations
4. replace crate-coverage assertions with operation-coverage assertions
5. update export manifest generation to report operation coverage
6. keep current crate provenance checks only as implementation validation

### Recommended `xtask` Command Evolution

Keep existing commands temporarily:

- `sdk export-ts`
- `sdk validate`

Add new migration-aware behavior behind the same commands:

- `sdk validate` validates both old and new contract surfaces
- `sdk export-ts` assembles an operation-first TypeScript SDK package from approved operations

Optional additive commands:

- `sdk validate-operations`
- `sdk export-ts-sdk`
- `sdk conformance check --language <id>`

## Language SDK Strategy

### TypeScript

TypeScript is the reference external SDK.

Implementation recommendation:

- keep Rust-driven generated models where useful
- use wasm for deterministic codec helpers where beneficial
- handwrite the final ergonomic operation surface
- expose one main SDK package

### Python

Python follows after TypeScript proves the operation model.

Implementation recommendation:

- do not mirror Rust crates directly
- either implement pure-Python adapters on top of contract artifacts or bind a very small deterministic Rust core
- keep packaging centered on one main SDK package

### Swift And Kotlin

Swift and Kotlin should wait until the operation contract is stable and conformance coverage is broader.

Implementation recommendation:

- keep the same operation contract
- keep runtime integration native
- only introduce shared native bindings for a narrow deterministic core if the maintenance tradeoff is justified

## Migration Plan

### Phase 0: Ratify Design

- adopt this design as the operation-first target
- treat current crate-keyed metadata as migration-only

### Phase 1: Add New Contract Metadata

- add operation-based metadata to the contract directory
- keep crate provenance metadata for existing tooling
- do not remove current manifest shape yet

### Phase 2: Add Curated Rust Facade

- introduce a public Rust contract facade
- map approved operations to existing implementation functions
- exclude projections, overlays, replica, and backoffice surfaces

### Phase 3: Expand Conformance

- add operation-based vectors for Tier 1 operations
- make vector coverage a release-blocking validation rule

### Phase 4: Migrate TypeScript Export

- shift `xtask export-ts` to operation-first packaging
- ship one main external TypeScript SDK package
- keep transitional generated artifacts internal if necessary

### Phase 5: Introduce Python

- implement Python export or packaging against the same contract and vector set

### Phase 6: Remove Crate-First Public Assumptions

- remove tests and validation that require package coverage by Rust crate name
- keep crate provenance only as internal documentation and maintenance metadata

## Acceptance Criteria

This design is implemented successfully when:

- the public contract manifest declares operations, shared types, and error classes
- `xtask validate` enforces operation coverage and conformance presence
- the curated Rust facade exposes only approved public operations
- the TypeScript SDK ships an operation-first public API
- conformance vectors exist for every Tier 1 operation
- public docs describe the SDK in terms of operations, not Rust crates

## Immediate Next Workstreams

1. introduce contract schema v2 files and parser structs
2. define the exact Tier 1 operation ids in machine-readable metadata
3. add a Rust public facade crate or module for those operations
4. author conformance vectors for every Tier 1 operation
5. rewrite `xtask` validator assumptions
6. redesign the TypeScript export manifest and package assembly
7. draft the first external TypeScript SDK surface around the approved operations

## Repository Notes

This document intentionally does not modify the current crate-keyed contract files in place. The repository currently contains user edits in several existing files, including `spec/README`. The recommended implementation path is to add the new operation-first contract artifacts alongside the current files first, then migrate validation and export tooling incrementally.
