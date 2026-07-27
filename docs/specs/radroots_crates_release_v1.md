# Radroots Crates Release V1

**Normative identifier:** `radroots.crates.release.v1`
**Document status:** Final pressure-tested architecture and publication specification
**Date:** 2026-07-26
**Source snapshots reviewed:**
- `radrootslabs/lib@466f3cc36739179bc17edb9db796530729ba5219`
- `radrootslabs/sdk@fd8384aee348034e0c8ea17a868fe7f094770050`

**Repository allocation:** the existing `radrootslabs/lib` and
`radrootslabs/sdk` repositories remain independent. The first 17 packages are
owned by `lib`; `radroots-sdk` and `radroots` are owned by `sdk`. No third Rust
repository is created for release V1.

**Registry context supplied by the project:** no Radroots crate is currently published on crates.io. Deleted experimental publications create no compatibility, pluralization, deprecation, or version-continuity requirement.

## 1. Normative language and status

The words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

This specification freezes the **package identities, ownership boundaries, naming model, dependency direction, and release gates** for the first durable Radroots crates.io release.

It does not claim that the current source tree is already publishable. Publication remains blocked until the refactor is implemented and every acceptance gate in this document passes against packaged artifacts.

## 2. Final executive decision

Radroots SHALL publish exactly **19 durable package identities** for release V1:

1. `radroots-core`
2. `radroots-identity`
3. `radroots-blossom`
4. `radroots-protocol`
5. `radroots-event`
6. `radroots-event-codec`
7. `radroots-trade`
8. `radroots-signing`
9. `radroots-transport`
10. `radroots-nostr`
11. `radroots-nostr-connect`
12. `radroots-secrets`
13. `radroots-storage`
14. `radroots-storage-sqlite`
15. `radroots-transport-nostr`
16. `radroots-sync`
17. `radroots-geonames`
18. `radroots-sdk`
19. `radroots`

This is neither the current workspace publication list nor a collapse into `radroots-sdk`.

The package family is implemented across the two existing standalone
repositories. `radrootslabs/lib` owns packages 1-17, and `radrootslabs/sdk`
owns packages 18-19. Cross-repository dependencies resolve through registry
versions; neither repository depends on a sibling checkout.

The architecture is a layered network/protocol stack:

```text
portable values / identity / protocol contracts
                    ↓
event model and deterministic domain algorithms
                    ↓
signing, transport, secrets, and storage SPIs
                    ↓
concrete native backends and network adapters
                    ↓
local-first synchronization engine
                    ↓
advanced SDK
                    ↓
ordinary-user façade
```

## 3. Final pressure-test changes from the directionally approved draft

The final review makes the following deliberate changes:

1. **`radroots-contracts` becomes `radroots-protocol`.** The package is a durable, versioned wire/operation protocol boundary rather than a general-purpose “contracts” bucket.
2. **`radroots-store` and `radroots-store-sqlite` become `radroots-storage` and `radroots-storage-sqlite`.** “Storage” is unambiguous in an agricultural marketplace and describes the package family more accurately than “store.”
3. **`radroots-nostr-connect` remains independent.** It is a bidirectional security protocol with URIs, permissions, client/server state, and independent SDK/Myc consumers. It is not merely a convenience NIP module.
4. **Actor ownership is refined.** Public keys/accounts live in identity; event author roles live in the event contract model; actor provenance, authorization, and signer behavior live in signing.
5. **Trade identity is made singular.** The conflicting `TradeId`/`OrderId` definitions MUST be replaced by one canonical protocol `TradeId` and a separately named business `OrderId`.
6. **Public codegen features are removed.** `dto-bindgen`, binding generation, WASM wrappers, and fixture switches remain private build/test concerns.
7. **The workspace moves to Cargo resolver 3.** A virtual Rust 2024 workspace MUST explicitly set `resolver = "3"`.
8. **Release V1 uses MSRV 1.97.1.** The patch release is selected rather than 1.97.0 because it contains a compiler miscompilation fix.
9. **Lower crates do not remain permanently lockstep.** All begin at `0.1.0`, but only `radroots` and `radroots-sdk` are an exact lockstep pair; lower packages follow independent SemVer after the first release.

## 4. Non-negotiable architecture invariants

1. Every public package MUST have a durable name and at least one named direct consumer besides `radroots`.
2. Every dependency edge MUST point downward in the architecture.
3. Domain and protocol packages MUST NOT depend on storage, networking, process lifecycle, or host UI.
4. Generic SPIs MUST NOT expose concrete SQLite, Tokio, Reqwest, Nostr SDK, keyring, or OS-specific types.
5. Adapters and backends MUST implement SPIs; SPIs MUST NOT depend on adapters or backends.
6. Synchronization MUST orchestrate sources, sinks, signing, and storage without owning an executor or scheduler.
7. `radroots-sdk` MUST compose lower packages and own client-level commit semantics; it MUST NOT become a dumping ground for code that has an independent durable boundary.
8. `radroots` MUST provide meaningful curation and documentation; it MUST NOT be `pub use radroots_sdk::*`.
9. Version generations MUST live in modules and schema IDs, never in package names.
10. Preview implementation code MAY remain in the monorepo but MUST NOT appear in a published dependency or feature until registry-ready.
11. No public package may have a normal, optional, build, or target-specific dependency on a private Radroots package.
12. No first-party consumer may rely on production sibling paths after cutover.

## 5. Final public package inventory

### 5.1 Repository ownership

- `https://github.com/radrootslabs/lib` owns `radroots-core` through
  `radroots-geonames` (packages 1-17).
- `https://github.com/radrootslabs/sdk` owns `radroots-sdk` and `radroots`
  (packages 18-19).
- Both repositories retain their existing histories and remain independently
  buildable, testable, packageable, and releasable.
- The two repositories carry synchronized copies of this release-family
  contract. A coordinated release MUST reject any content-hash or package
  allocation mismatch between those copies.

| Order | Package | Rust crate path | Tier | Permanent responsibility |
|---:|---|---|---|---|
| 1 | `radroots-core` | `radroots_core` | foundation | Foundational value objects and deterministic invariants: decimal, currency, money, percentage, quantity, units, and pricing. |
| 2 | `radroots-identity` | `radroots_identity` | foundation | Public identity and account value types: canonical public keys, identity IDs, account IDs, public profiles, and usernames. |
| 3 | `radroots-blossom` | `radroots_blossom` | protocol primitive | Portable Blossom protocol primitives: canonical blob URLs, hashes, media descriptors, byte-verification typestates, and authorization claims. |
| 4 | `radroots-protocol` | `radroots_protocol` | versioned wire contract | Versioned cross-process and cross-language schemas, capability catalogs, operation descriptors, stable error reports, schema IDs, and structural validation. |
| 5 | `radroots-event` | `radroots_event` | domain protocol model | Canonical Radroots event-domain models, validated event identifiers, tags, event contracts, authoring drafts, signed/verified typestates, and NIP-01 wire-neutral representations. |
| 6 | `radroots-event-codec` | `radroots_event_codec` | deterministic algorithm | Deterministic canonical encoding, decoding, ID/signature verification, contract validation, admission, and manifest generation for Radroots events. |
| 7 | `radroots-trade` | `radroots_trade` | domain algorithm | Trade validation, evidence models, deterministic reduction, conflict analysis, and side-effect-free workflow plans over the canonical event trade model. |
| 8 | `radroots-signing` | `radroots_signing` | host SPI | Object-safe author/signing SPI, actor provenance, authorization checks, requests, receipts, progress, capabilities, and normalized signing errors. |
| 9 | `radroots-transport` | `radroots_transport` | network SPI | Transport-neutral target identities, capability/status models, source and sink SPIs, delivery/fetch policies, bounded requests, provenance, and normalized outcomes. |
| 10 | `radroots-nostr` | `radroots_nostr` | protocol adapter | Portable conversion between Radroots native event/identity types and Nostr protocol types, typed NIP helpers, and concrete local signing adapters; no live relay client. |
| 11 | `radroots-nostr-connect` | `radroots_nostr_connect` | security protocol | Nostr Connect/NIP-46 URIs, methods, permissions, requests, responses, client/server state machines, timeout-independent protocol validation, and normalized errors. |
| 12 | `radroots-secrets` | `radroots_secrets` | security SPI | Secret references, provider and key-wrapping SPIs, versioned encrypted envelopes, zeroization-safe secret handling, and explicit memory/file/keyring adapters. |
| 13 | `radroots-storage` | `radroots_storage` | storage SPI | Backend-neutral canonical event, operation journal, outbox, transport evidence, projection, private-artifact metadata, backup, status, and atomic commit interfaces, plus an in-memory reference backend. |
| 14 | `radroots-storage-sqlite` | `radroots_storage_sqlite` | native storage backend | SQLite implementation of the storage SPIs with schema migration, WAL, locking, integrity, backup/restore, crash recovery, and encrypted private storage. |
| 15 | `radroots-transport-nostr` | `radroots_transport_nostr` | native network adapter | Concrete Nostr EventSource/EventSink implementation: relay URL policy, connection, NIP-42 authentication, bounded fetch pages, delivery, status, and relay-outcome normalization. |
| 16 | `radroots-sync` | `radroots_sync` | local-first orchestration | Shared pull, verification, canonical admission, duplicate handling, projection refresh, outbox signing/delivery, status, and retry-decision orchestration without owning scheduling. |
| 17 | `radroots-geonames` | `radroots_geonames` | concrete data provider | GeoNames asset specification, authenticated/integrity-checked acquisition, database lifecycle, and forward/reverse locality lookup using provider-owned types. |
| 18 | `radroots-sdk` | `radroots_sdk` | advanced front door | Host-neutral asynchronous client engine, product operations, capability reporting, explicit storage/signing/transport composition, diagnostics, backup/restore, and safe commit semantics. |
| 19 | `radroots` | `radroots` | ordinary-user front door | Canonical Rust onboarding package with curated modules, safe defaults, stable convenience builders, domain aggregation, examples, and primary documentation. |

## 6. Detailed package specifications

    ### 1. `radroots-core`

    **Rust crate path:** `radroots_core`
    **Tier:** foundation
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots-event, radroots-trade, radroots-sdk, radroots

    **Normative responsibility.** Foundational value objects and deterministic invariants: decimal, currency, money, percentage, quantity, units, and pricing.

    **Required Radroots dependencies:** None
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`
    **Complete public feature vocabulary:** `std`, `serde`

    **Public modules**

    - `currency`
- `decimal`
- `money`
- `percent`
- `pricing`
- `quantity`
- `unit`

    **Permitted root exports:** `Currency`, `Decimal`, `Money`, `Percent`, `Quantity`, `QuantityPrice`, `Unit`, `Error`

    **Explicitly forbidden.** Identifiers, identities, event kinds, networking, persistence, clocks, filesystem paths, process behavior, or application configuration.


    ### 2. `radroots-identity`

    **Rust crate path:** `radroots_identity`
    **Tier:** foundation
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots-event, radroots-signing, radroots-transport, radroots-nostr, radroots-nostr-connect, radroots-sdk, services

    **Normative responsibility.** Public identity and account value types: canonical public keys, identity IDs, account IDs, public profiles, and usernames.

    **Required Radroots dependencies:** None
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`
    **Complete public feature vocabulary:** `std`, `serde`

    **Public modules**

    - `account`
- `key`
- `profile`
- `username`

    **Permitted root exports:** `AccountId`, `IdentityId`, `PublicIdentity`, `PublicKey`, `Profile`, `Username`, `Error`

    **Explicitly forbidden.** Secret keys, key generation, NIP-49 encryption, keyrings, files, SQLite, runtime paths, upstream nostr::Event values, signer sessions, or host account selection.


    ### 3. `radroots-blossom`

    **Rust crate path:** `radroots_blossom`
    **Tier:** protocol primitive
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots-event, radroots-event-codec, radroots-nostr, media clients

    **Normative responsibility.** Portable Blossom protocol primitives: canonical blob URLs, hashes, media descriptors, byte-verification typestates, and authorization claims.

    **Required Radroots dependencies:** None
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`
    **Complete public feature vocabulary:** `std`, `serde`

    **Public modules**

    - `authorization`
- `descriptor`
- `hash`
- `media_type`
- `url`

    **Permitted root exports:** `BlobUrl`, `Sha256`, `MediaType`, `BlobDescriptor`, `ByteVerifiedDescriptor`, `AuthorizationClaim`, `Error`

    **Explicitly forbidden.** HTTP clients, upload scheduling, cache management, filesystem traversal, application media policy, or global authentication state.


    ### 4. `radroots-protocol`

    **Rust crate path:** `radroots_protocol`
    **Tier:** versioned wire contract
    **API maturity at first publish:** durable identity; independently versioned contract modules
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots-event, radroots-signing, radroots-transport, radroots-storage, radroots-sync, radroots-sdk, radrootsd, bindings

    **Normative responsibility.** Versioned cross-process and cross-language schemas, capability catalogs, operation descriptors, stable error reports, schema IDs, and structural validation.

    **Required Radroots dependencies:** None
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`
    **Complete public feature vocabulary:** `std`, `serde`

    **Public modules**

    - `capability::v1`
- `error::v1`
- `event::v1`
- `runtime::v1`
- `radrootsd::transport_publish::v5`
- `schema`

    **Permitted root exports:** No broad root exports.

    **Explicitly forbidden.** Native clients, storage, network I/O, executor/runtime ownership, domain reducers, upstream dependency types, unversioned serialized DTOs, or package names containing protocol generations.


    ### 5. `radroots-event`

    **Rust crate path:** `radroots_event`
    **Tier:** domain protocol model
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots-event-codec, radroots-trade, radroots-signing, radroots-transport, radroots-storage, radroots-sync, radroots-nostr, radroots-sdk, indexers

    **Normative responsibility.** Canonical Radroots event-domain models, validated event identifiers, tags, event contracts, authoring drafts, signed/verified typestates, and NIP-01 wire-neutral representations.

    **Required Radroots dependencies:** `radroots-core`, `radroots-identity`, `radroots-blossom`, `radroots-protocol`
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`
    **Complete public feature vocabulary:** `std`, `serde`, `knowledge`

    **Public modules**

    - `admission`
- `calendar`
- `contract`
- `draft`
- `envelope`
- `farm`
- `food`
- `id`
- `knowledge`
- `listing`
- `media`
- `post`
- `profile`
- `social`
- `tag`
- `trade`
- `wire`

    **Permitted root exports:** `Event`, `EventDraft`, `SignedEvent`, `VerifiedEvent`, `EventId`, `EventKind`, `EventTag`, `Error`

    **Explicitly forbidden.** Live Nostr clients, relay pools, signing backends, SQLite, outbox claims, retry scheduling, application state, or duplicate trade/order identifier concepts.


    ### 6. `radroots-event-codec`

    **Rust crate path:** `radroots_event_codec`
    **Tier:** deterministic algorithm
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc where selected features permit; std feature
    **Direct intended consumers:** radroots-nostr, radroots-storage-sqlite, radroots-transport-nostr, radroots-sync, radroots-sdk, bindings

    **Normative responsibility.** Deterministic canonical encoding, decoding, ID/signature verification, contract validation, admission, and manifest generation for Radroots events.

    **Required Radroots dependencies:** `radroots-event`, `radroots-blossom`, `radroots-protocol`
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `json`
    **Complete public feature vocabulary:** `std`, `serde`, `json`, `knowledge`, `manifests`

    **Public modules**

    - `admission`
- `canonical`
- `decode`
- `encode`
- `manifest`
- `verify`

    **Permitted root exports:** `Codec`, `DecodeError`, `EncodeError`, `VerificationError`

    **Explicitly forbidden.** nostr-sdk clients, relay networking, persistence, background work, upstream client errors, or host configuration.


    ### 7. `radroots-trade`

    **Rust crate path:** `radroots_trade`
    **Tier:** domain algorithm
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc for model/reducer; std feature
    **Direct intended consumers:** radroots-storage, radroots-sync, radroots-sdk, RHI, applications

    **Normative responsibility.** Trade validation, evidence models, deterministic reduction, conflict analysis, and side-effect-free workflow plans over the canonical event trade model.

    **Required Radroots dependencies:** `radroots-core`, `radroots-identity`, `radroots-event`
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`, `json`
    **Complete public feature vocabulary:** `std`, `serde`, `json`

    **Public modules**

    - `evidence`
- `model`
- `reducer`
- `validation`
- `workflow`

    **Permitted root exports:** `Projection`, `ReductionInput`, `ReducerIssue`, `WorkflowPlan`, `ValidationError`, `Error`

    **Explicitly forbidden.** A second TradeId definition, actor authorization, signers, event-store access, SQLx, filesystem state, transport delivery, outbox mutation, or process scheduling.


    ### 8. `radroots-signing`

    **Rust crate path:** `radroots_signing`
    **Tier:** host SPI
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc core; std feature
    **Direct intended consumers:** radroots-nostr, radroots-sync, radroots-sdk, CLI, Studio, FFI hosts

    **Normative responsibility.** Object-safe author/signing SPI, actor provenance, authorization checks, requests, receipts, progress, capabilities, and normalized signing errors.

    **Required Radroots dependencies:** `radroots-identity`, `radroots-event`, `radroots-protocol`
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`
    **Complete public feature vocabulary:** `std`, `serde`

    **Public modules**

    - `actor`
- `capability`
- `error`
- `request`
- `receipt`
- `signer`
- `status`

    **Permitted root exports:** `Actor`, `Signer`, `SignRequest`, `SignReceipt`, `SignerStatus`, `Error`

    **Explicitly forbidden.** Raw secret-key ownership, keyrings, relay networking, NIP-46 session persistence, SQL, UI prompts, or executor creation.


    ### 9. `radroots-transport`

    **Rust crate path:** `radroots_transport`
    **Tier:** network SPI
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc data model; std feature
    **Direct intended consumers:** radroots-storage, radroots-transport-nostr, radroots-sync, radroots-sdk, services, future adapters

    **Normative responsibility.** Transport-neutral target identities, capability/status models, source and sink SPIs, delivery/fetch policies, bounded requests, provenance, and normalized outcomes.

    **Required Radroots dependencies:** `radroots-identity`, `radroots-event`, `radroots-protocol`
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`
    **Complete public feature vocabulary:** `std`, `serde`

    **Public modules**

    - `capability`
- `endpoint`
- `error`
- `outcome`
- `policy`
- `sink`
- `source`
- `target`

    **Permitted root exports:** `TransportId`, `Target`, `TargetSet`, `EventSource`, `EventSink`, `DeliveryRequest`, `DeliveryReceipt`, `FetchRequest`, `FetchPage`, `Error`

    **Explicitly forbidden.** Closed enums that prevent new transports, Reticulum-specific constants, Nostr URLs at the generic root, storage/outbox access, retries, scheduler ownership, or silent fallback.


    ### 10. `radroots-nostr`

    **Rust crate path:** `radroots_nostr`
    **Tier:** protocol adapter
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc for conversion surface; std feature
    **Direct intended consumers:** radroots-nostr-connect, radroots-transport-nostr, radroots-sdk, Myc, radrootsd

    **Normative responsibility.** Portable conversion between Radroots native event/identity types and Nostr protocol types, typed NIP helpers, and concrete local signing adapters; no live relay client.

    **Required Radroots dependencies:** `radroots-identity`, `radroots-event`, `radroots-event-codec`
    **Optional Radroots dependencies:** `radroots-signing`, `radroots-blossom`
    **Default features:** `std`, `events`
    **Complete public feature vocabulary:** `std`, `events`, `signing`, `nip17`, `blossom`

    **Public modules**

    - `blossom`
- `event`
- `filter`
- `key`
- `nip17`
- `signing`
- `tag`

    **Permitted root exports:** `Error`

    **Explicitly forbidden.** nostr-sdk relay pools, reqwest clients, runtime ownership, broad aliases of upstream nostr types at the root, account persistence, or outbox orchestration.


    ### 11. `radroots-nostr-connect`

    **Rust crate path:** `radroots_nostr_connect`
    **Tier:** security protocol
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std for v1; protocol data kept portable
    **Direct intended consumers:** radroots-sdk, Myc, remote signer tooling

    **Normative responsibility.** Nostr Connect/NIP-46 URIs, methods, permissions, requests, responses, client/server state machines, timeout-independent protocol validation, and normalized errors.

    **Required Radroots dependencies:** `radroots-identity`, `radroots-event`, `radroots-nostr`, `radroots-protocol`
    **Optional Radroots dependencies:** None
    **Default features:** `serde`
    **Complete public feature vocabulary:** `serde`

    **Public modules**

    - `client`
- `error`
- `message`
- `method`
- `permission`
- `server`
- `uri`

    **Permitted root exports:** `Client`, `Server`, `Method`, `Permission`, `Request`, `Response`, `BunkerUri`, `ClientUri`, `Error`

    **Explicitly forbidden.** Relay-pool implementation, secret persistence, approval UI, global sessions, Tokio runtime ownership, or Myc-specific service storage.


    ### 12. `radroots-secrets`

    **Rust crate path:** `radroots_secrets`
    **Tier:** security SPI
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc core; std adapters
    **Direct intended consumers:** radroots-storage-sqlite, radroots-sdk, signing hosts, services

    **Normative responsibility.** Secret references, provider and key-wrapping SPIs, versioned encrypted envelopes, zeroization-safe secret handling, and explicit memory/file/keyring adapters.

    **Required Radroots dependencies:** None
    **Optional Radroots dependencies:** None
    **Default features:** `std`, `serde`
    **Complete public feature vocabulary:** `std`, `serde`, `memory`, `file`, `keyring`

    **Public modules**

    - `envelope`
- `error`
- `id`
- `provider`
- `wrapping`
- `memory`
- `file`
- `keyring`

    **Permitted root exports:** `SecretId`, `SecretRef`, `SecretProvider`, `KeyWrapping`, `EncryptedEnvelope`, `Error`

    **Explicitly forbidden.** Public secret bytes, Clone/Debug/Serialize for secret-bearing values, identity profiles, domain tables, arbitrary key/value storage, hidden key generation, or process-global vaults.


    ### 13. `radroots-storage`

    **Rust crate path:** `radroots_storage`
    **Tier:** storage SPI
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std for v1
    **Direct intended consumers:** radroots-storage-sqlite, radroots-sync, radroots-sdk, indexers, tests, future backends

    **Normative responsibility.** Backend-neutral canonical event, operation journal, outbox, transport evidence, projection, private-artifact metadata, backup, status, and atomic commit interfaces, plus an in-memory reference backend.

    **Required Radroots dependencies:** `radroots-event`, `radroots-trade`, `radroots-transport`, `radroots-protocol`
    **Optional Radroots dependencies:** None
    **Default features:** `memory`, `serde`
    **Complete public feature vocabulary:** `memory`, `serde`

    **Public modules**

    - `atomic`
- `backup`
- `event`
- `journal`
- `memory`
- `outbox`
- `private_artifact`
- `projection`
- `status`

    **Permitted root exports:** `Storage`, `EventStore`, `Journal`, `Outbox`, `ProjectionStore`, `BackupSource`, `StorageStatus`, `Error`

    **Explicitly forbidden.** SQL text, SQLx pools or transactions, filesystem paths, application UI state, concrete retry loops, Nostr clients, or unconstrained raw key/value escape hatches.


    ### 14. `radroots-storage-sqlite`

    **Rust crate path:** `radroots_storage_sqlite`
    **Tier:** native storage backend
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std-only native backend
    **Direct intended consumers:** radroots-sdk, CLI, Studio, mobile/FFI

    **Normative responsibility.** SQLite implementation of the storage SPIs with schema migration, WAL, locking, integrity, backup/restore, crash recovery, and encrypted private storage.

    **Required Radroots dependencies:** `radroots-storage`, `radroots-event-codec`, `radroots-secrets`
    **Optional Radroots dependencies:** None
    **Default features:** None
    **Complete public feature vocabulary:** None

    **Public modules**

    - `backup`
- `config`
- `integrity`
- `lock`
- `migration`
- `open`
- `status`

    **Permitted root exports:** `SqliteStorage`, `OpenOptions`, `OpenMode`, `Paths`, `Error`

    **Explicitly forbidden.** Public SqlitePool/Connection/Transaction handles, caller-supplied arbitrary SQL, Studio state, global connection pools, runtime installation, or silent schema downgrade.


    ### 15. `radroots-transport-nostr`

    **Rust crate path:** `radroots_transport_nostr`
    **Tier:** native network adapter
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std-only; Tokio implementation detail in v1
    **Direct intended consumers:** radroots-sdk, CLI, radrootsd, advanced hosts

    **Normative responsibility.** Concrete Nostr EventSource/EventSink implementation: relay URL policy, connection, NIP-42 authentication, bounded fetch pages, delivery, status, and relay-outcome normalization.

    **Required Radroots dependencies:** `radroots-transport`, `radroots-nostr`, `radroots-event-codec`, `radroots-protocol`
    **Optional Radroots dependencies:** None
    **Default features:** None
    **Complete public feature vocabulary:** None

    **Public modules**

    - `auth`
- `client`
- `relay`
- `sink`
- `source`
- `status`

    **Permitted root exports:** `NostrTransport`, `Config`, `RelayUrl`, `RelayUrlPolicy`, `Error`

    **Explicitly forbidden.** Event-store ingestion, outbox claiming, retry scheduling, projection refresh, global relay clients, direct SQL, or transport fallback.


    ### 16. `radroots-sync`

    **Rust crate path:** `radroots_sync`
    **Tier:** local-first orchestration
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std-only in v1; executor-neutral public API
    **Direct intended consumers:** radroots-sdk, CLI, Studio, mobile/FFI, advanced hosts

    **Normative responsibility.** Shared pull, verification, canonical admission, duplicate handling, projection refresh, outbox signing/delivery, status, and retry-decision orchestration without owning scheduling.

    **Required Radroots dependencies:** `radroots-event`, `radroots-event-codec`, `radroots-signing`, `radroots-transport`, `radroots-storage`, `radroots-trade`, `radroots-protocol`
    **Optional Radroots dependencies:** None
    **Default features:** `serde`
    **Complete public feature vocabulary:** `serde`

    **Public modules**

    - `ingest`
- `policy`
- `projection`
- `pull`
- `push`
- `status`

    **Permitted root exports:** `Engine`, `PullRequest`, `PullReceipt`, `PushRequest`, `PushReceipt`, `SyncStatus`, `Error`

    **Explicitly forbidden.** Creating an executor, spawning hidden workers, installing timers globally, owning process lifecycle, storing UI state, or transport-specific branches outside adapters.


    ### 17. `radroots-geonames`

    **Rust crate path:** `radroots_geonames`
    **Tier:** concrete data provider
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std-only
    **Direct intended consumers:** radroots-sdk, CLI, geocoding applications

    **Normative responsibility.** GeoNames asset specification, authenticated/integrity-checked acquisition, database lifecycle, and forward/reverse locality lookup using provider-owned types.

    **Required Radroots dependencies:** None
    **Optional Radroots dependencies:** None
    **Default features:** None
    **Complete public feature vocabulary:** None

    **Public modules**

    - `asset`
- `database`
- `download`
- `model`
- `query`

    **Permitted root exports:** `Geocoder`, `AssetSpec`, `AssetStatus`, `Query`, `Candidate`, `Point`, `Error`

    **Explicitly forbidden.** Generic multi-provider abstraction before a second provider exists, runtime-path policy, hidden downloads, SDK configuration types, SQLx/reqwest types in the public API, or test-fixture features.


    ### 18. `radroots-sdk`

    **Rust crate path:** `radroots_sdk`
    **Tier:** advanced front door
    **API maturity at first publish:** durable identity; lockstep pre-1.0 API with radroots
    **Platform contract:** std-only native engine
    **Direct intended consumers:** CLI, Studio, FFI/mobile, advanced native applications

    **Normative responsibility.** Host-neutral asynchronous client engine, product operations, capability reporting, explicit storage/signing/transport composition, diagnostics, backup/restore, and safe commit semantics.

    **Required Radroots dependencies:** `radroots-core`, `radroots-identity`, `radroots-protocol`, `radroots-event`, `radroots-event-codec`, `radroots-trade`, `radroots-signing`, `radroots-transport`, `radroots-storage`
    **Optional Radroots dependencies:** `radroots-secrets`, `radroots-storage-sqlite`, `radroots-nostr`, `radroots-nostr-connect`, `radroots-transport-nostr`, `radroots-sync`, `radroots-geonames`
    **Default features:** `memory`
    **Complete public feature vocabulary:** `memory`, `sqlite`, `sync`, `nostr`, `nip46`, `local-signing`, `radrootsd`, `geonames`, `knowledge`, `native`, `full`

    **Public modules**

    - `capability`
- `client`
- `diagnostics`
- `error`
- `farm`
- `listing`
- `signing`
- `storage`
- `sync`
- `trade`
- `transport`

    **Permitted root exports:** `Client`, `ClientBuilder`, `Error`, `Result`

    **Explicitly forbidden.** Global runtimes or subscribers, hidden workers, process signals, CLI parsing, UI state, Studio databases, raw SQLx/upstream client types, broad wildcard reexports, or a nominal no_std claim.


    ### 19. `radroots`

    **Rust crate path:** `radroots`
    **Tier:** ordinary-user front door
    **API maturity at first publish:** durable identity; lockstep pre-1.0 API with radroots-sdk
    **Platform contract:** std-only
    **Direct intended consumers:** ordinary Rust applications, examples, documentation

    **Normative responsibility.** Canonical Rust onboarding package with curated modules, safe defaults, stable convenience builders, domain aggregation, examples, and primary documentation.

    **Required Radroots dependencies:** `radroots-sdk`, `radroots-core`, `radroots-identity`, `radroots-event`, `radroots-trade`, `radroots-transport`
    **Optional Radroots dependencies:** None
    **Default features:** `client`
    **Complete public feature vocabulary:** `client`, `native`, `nostr`, `nip46`, `radrootsd`, `geonames`, `knowledge`, `full`

    **Public modules**

    - `client`
- `event`
- `farm`
- `identity`
- `knowledge`
- `listing`
- `signing`
- `storage`
- `sync`
- `trade`
- `transport`

    **Permitted root exports:** `Client`, `ClientBuilder`, `Error`, `Result`

    **Explicitly forbidden.** A public radroots::sdk namespace, wildcard reexport of radroots-sdk, duplicate engine implementation, CLI binary, hidden network/filesystem/keychain side effects, or exposure of every lower-crate symbol.


## 7. Normative dependency graph

### 7.1 Direct Radroots edges

| Dependency | Dependent | Edge |
|---|---|---|
| `radroots-blossom` | `radroots-event` | required |
| `radroots-core` | `radroots-event` | required |
| `radroots-identity` | `radroots-event` | required |
| `radroots-protocol` | `radroots-event` | required |
| `radroots-blossom` | `radroots-event-codec` | required |
| `radroots-event` | `radroots-event-codec` | required |
| `radroots-protocol` | `radroots-event-codec` | required |
| `radroots-core` | `radroots-trade` | required |
| `radroots-event` | `radroots-trade` | required |
| `radroots-identity` | `radroots-trade` | required |
| `radroots-event` | `radroots-signing` | required |
| `radroots-identity` | `radroots-signing` | required |
| `radroots-protocol` | `radroots-signing` | required |
| `radroots-event` | `radroots-transport` | required |
| `radroots-identity` | `radroots-transport` | required |
| `radroots-protocol` | `radroots-transport` | required |
| `radroots-blossom` | `radroots-nostr` | optional |
| `radroots-signing` | `radroots-nostr` | optional |
| `radroots-event` | `radroots-nostr` | required |
| `radroots-event-codec` | `radroots-nostr` | required |
| `radroots-identity` | `radroots-nostr` | required |
| `radroots-event` | `radroots-nostr-connect` | required |
| `radroots-identity` | `radroots-nostr-connect` | required |
| `radroots-nostr` | `radroots-nostr-connect` | required |
| `radroots-protocol` | `radroots-nostr-connect` | required |
| `radroots-event` | `radroots-storage` | required |
| `radroots-protocol` | `radroots-storage` | required |
| `radroots-trade` | `radroots-storage` | required |
| `radroots-transport` | `radroots-storage` | required |
| `radroots-event-codec` | `radroots-storage-sqlite` | required |
| `radroots-secrets` | `radroots-storage-sqlite` | required |
| `radroots-storage` | `radroots-storage-sqlite` | required |
| `radroots-event-codec` | `radroots-transport-nostr` | required |
| `radroots-nostr` | `radroots-transport-nostr` | required |
| `radroots-protocol` | `radroots-transport-nostr` | required |
| `radroots-transport` | `radroots-transport-nostr` | required |
| `radroots-event` | `radroots-sync` | required |
| `radroots-event-codec` | `radroots-sync` | required |
| `radroots-protocol` | `radroots-sync` | required |
| `radroots-signing` | `radroots-sync` | required |
| `radroots-storage` | `radroots-sync` | required |
| `radroots-trade` | `radroots-sync` | required |
| `radroots-transport` | `radroots-sync` | required |
| `radroots-geonames` | `radroots-sdk` | optional |
| `radroots-nostr` | `radroots-sdk` | optional |
| `radroots-nostr-connect` | `radroots-sdk` | optional |
| `radroots-secrets` | `radroots-sdk` | optional |
| `radroots-storage-sqlite` | `radroots-sdk` | optional |
| `radroots-sync` | `radroots-sdk` | optional |
| `radroots-transport-nostr` | `radroots-sdk` | optional |
| `radroots-core` | `radroots-sdk` | required |
| `radroots-event` | `radroots-sdk` | required |
| `radroots-event-codec` | `radroots-sdk` | required |
| `radroots-identity` | `radroots-sdk` | required |
| `radroots-protocol` | `radroots-sdk` | required |
| `radroots-signing` | `radroots-sdk` | required |
| `radroots-storage` | `radroots-sdk` | required |
| `radroots-trade` | `radroots-sdk` | required |
| `radroots-transport` | `radroots-sdk` | required |
| `radroots-core` | `radroots` | required |
| `radroots-event` | `radroots` | required |
| `radroots-identity` | `radroots` | required |
| `radroots-sdk` | `radroots` | required |
| `radroots-trade` | `radroots` | required |
| `radroots-transport` | `radroots` | required |

### 7.2 Architectural graph

```text
radroots-core       radroots-identity       radroots-blossom
       \                 |                       /
        \                |                      /
         +---------- radroots-protocol --------+
                          |
                     radroots-event
                       /   |   \
                      /    |    \
       radroots-event-codec |  radroots-trade
                            |
                 +----------+-----------+
                 |          |           |
        radroots-signing  radroots-transport  radroots-storage
                 |          |           |
                 |      radroots-nostr   +--> radroots-storage-sqlite
                 |          |
                 |      radroots-nostr-connect
                 |          |
                 +---- radroots-transport-nostr
                            |
                       radroots-sync

radroots-geonames -------------------------+
                                           |
all selected lower packages ----------> radroots-sdk ---> radroots
```

This diagram is explanatory. The release tool MUST use Cargo-resolved metadata as authority.

## 8. Type ownership and canonical paths

| Concept | Canonical owning crate | Rule |
|---|---|---|
| Decimal, money, quantity, unit, pricing | `radroots-core` | No duplicate wrapper in SDK or trade. |
| Public key, identity ID, account ID, username | `radroots-identity` | Secret material is forbidden here. |
| Event ID, event signature, coordinate, D-tag, event kind | `radroots-event` | Store bytes/newtypes, not unvalidated Strings. |
| Event contract author role | `radroots-event::contract` | This is an event-authoring rule, not an account property. |
| Actor provenance and author context | `radroots-signing` | Combines identity with event author roles at the signing boundary. |
| Canonical protocol TradeId/CandidateId/MutationId | `radroots-event::trade` | Exactly one definition. |
| Human/business OrderId | `radroots-trade` | MUST NOT be aliased or wrapped as TradeId. |
| Runtime/wire DTO generations | `radroots-protocol` | Native packages convert at boundaries. |
| Secret references and encrypted envelopes | `radroots-secrets` | No domain-specific tables. |
| Transport ID, target, capability, outcome | `radroots-transport` | TransportId is extensible, not a closed enum. |
| Native event/outbox/journal/projection storage | `radroots-storage` | Backend-neutral interfaces only. |
| SQLite schema and connection behavior | `radroots-storage-sqlite` | SQLx remains private. |
| Relay URL and Nostr network status | `radroots-transport-nostr` | Protocol conversions remain in `radroots-nostr`. |
| Pull/push/ingest/projection orchestration | `radroots-sync` | No host scheduling. |
| Client-level requests, plans, receipts, diagnostics | `radroots-sdk` | Use lower canonical types rather than duplicate wrappers. |

## 9. Rust API and naming law

### 9.1 Packages, crates, modules, and types

- Cargo package names MUST be lowercase and hyphenated: `radroots-event-codec`.
- Rust crate paths MUST be snake case: `radroots_event_codec`.
- Modules MUST use singular snake-case nouns unless the concept is inherently plural.
- Types and traits MUST use `UpperCamelCase`; functions and methods MUST use `snake_case`.
- Items MUST NOT repeat their crate or module name:
  - `radroots_core::Money`, not `RadrootsCoreMoney`.
  - `radroots_sdk::Error`, not `RadrootsSdkError`.
  - `radroots_transport::Target`, not `RadrootsTransportTarget`.
- Protocol schema IDs retain the `radroots.*` namespace.
- Generated Swift/Kotlin types MAY retain a `Radroots` prefix where the target language lacks module-level namespacing.

### 9.2 Public surface discipline

- Crate roots MUST be small.
- Wildcard reexports and `pub use models::*` are forbidden.
- Lower crates MUST NOT publish a broad `prelude` in V1.
- Every native public struct MUST have private fields unless it is an intentionally passive versioned DTO in `radroots-protocol`.
- Evolvable enums and reports SHOULD be `#[non_exhaustive]`.
- Semantic IDs MUST NOT implement `Deref<Target = str>`.
- IDs SHOULD store canonical bytes or validated compact representations; string encoding belongs at boundaries.
- Use `FromStr`, `TryFrom`, `AsRef`, `Display`, and explicit `into_string`/`to_hex` methods.
- No root-level aliases to upstream `nostr`, `nostr_sdk`, `sqlx`, `reqwest`, `tokio`, or keyring types.
- Public functions MUST NOT panic for untrusted input.
- Builders/plans/receipts SHOULD be `#[must_use]`.
- Constructors with more than three independent options SHOULD use builders or option structs.
- Empty request structs are forbidden when an idiomatic no-argument method conveys the same operation.

### 9.3 Trait classification

Every public trait MUST be marked in documentation as one of:

1. **Host SPI** — downstream implementation is supported.
2. **Sealed extension** — downstream calls are supported; implementation is not.
3. **Internal** — not public.

Host SPIs MUST:

- be dyn-compatible where runtime injection is needed;
- be `Send + Sync` for the native SDK;
- return boxed futures or equivalent dyn-compatible futures;
- define cancellation and deadline behavior;
- define error normalization;
- avoid associated types that leak backend implementations;
- not expose private or third-party implementation types.

The Rust SPI is native. Browser and generated-language transports use `radroots-protocol` DTOs and language-native interfaces rather than weakening native `Send + Sync` guarantees.

## 10. Feature law

1. Features MUST be additive and safe under unification.
2. Features MUST describe user-visible capabilities, not implementation assembly.
3. Optional dependencies MUST be referenced with `dep:` so implementation names do not become accidental features.
4. Default features MUST remain safe for the life of a compatible release line.
5. Enabling a feature MUST NOT itself:
   - access a network;
   - create files;
   - read a keyring;
   - generate keys;
   - contact a daemon;
   - install logging;
   - start workers.
6. Mutually exclusive backend features are forbidden; incompatible backends belong in separate packages.
7. Public crates MUST NOT expose `dto-bindgen`, fixtures, coverage, codegen, migration-forge, or internal runtime features.
8. `std` is the only valid feature name for standard-library support.
9. Public feature removal is a breaking change.
10. `--all-features` MUST build on every declared target for which the package claims support.

### 10.1 `radroots-sdk`

```toml
[features]
default = ["memory"]

# Safe, in-process storage; no files or network.
memory = ["radroots-storage/memory"]

# Explicit native capabilities.
sqlite = ["dep:radroots-storage-sqlite"]
sync = ["dep:radroots-sync"]
nostr = [
  "sync",
  "dep:radroots-nostr",
  "dep:radroots-transport-nostr",
]
nip46 = [
  "nostr",
  "dep:radroots-nostr-connect",
]
local-signing = [
  "dep:radroots-secrets",
  "radroots-nostr/signing",
]
radrootsd = [
  "sync",
  "dep:reqwest",
]
geonames = ["dep:radroots-geonames"]
knowledge = [
  "radroots-event/knowledge",
  "radroots-event-codec/knowledge",
]

native = ["sqlite", "sync", "local-signing"]
full = [
  "native",
  "nostr",
  "nip46",
  "radrootsd",
  "geonames",
  "knowledge",
]
```

### 10.2 `radroots`

```toml
[features]
default = ["client"]

client = ["radroots-sdk/default"]
native = ["client", "radroots-sdk/native"]
nostr = ["client", "radroots-sdk/nostr"]
nip46 = ["nostr", "radroots-sdk/nip46"]
radrootsd = ["client", "radroots-sdk/radrootsd"]
geonames = ["client", "radroots-sdk/geonames"]
knowledge = ["client", "radroots-sdk/knowledge"]
full = ["radroots-sdk/full"]
```

Reticulum, mesh, Simplex, NostrDB, replica, and SP1 feature names MUST NOT appear in published V1 manifests.

## 11. Network and transport SPI

### 11.1 Separate source and sink contracts

One monolithic transport trait is rejected. The final SPI provides independent contracts:

```rust
pub trait EventSource: Send + Sync {
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, Error>>;
    fn fetch(&self, request: FetchRequest)
        -> BoxFuture<'_, Result<FetchPage, Error>>;
}

pub trait EventSink: Send + Sync {
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, Error>>;
    fn deliver(&self, request: DeliveryRequest)
        -> BoxFuture<'_, Result<DeliveryReceipt, Error>>;
}
```

A transport MAY implement either or both.

### 11.2 Extensible transport identity

`TransportId` MUST be a validated newtype with built-in constants such as `NOSTR`, `RETICULUM`, `LOCAL`, and `RADROOTSD`. It MUST NOT be a closed enum that forces a breaking release for every new transport.

### 11.3 Bounded and explicit operation semantics

- Fetch is paginated or streaming and always bounded.
- Every request carries an operation/request ID.
- Deadlines are explicit; no adapter owns a global timeout.
- Delivery satisfaction policy is explicit.
- Partial success is represented per target.
- Retryability is data, not an implicit loop.
- Authentication challenges are explicit outcomes.
- No transport silently falls back to another.
- Provenance records source, endpoint fingerprint, observed time, and adapter.
- Adapter errors are normalized while preserving a non-secret source chain.
- Target URIs and relay URLs are validated before connection.
- SSRF-sensitive schemes and private-network policies are explicit.
- TLS verification is enabled by default and cannot be silently disabled.
- Payload, target, tag, response, and page limits are constants covered by tests.

## 12. Storage architecture

### 12.1 Logical ownership

`radroots-storage` owns interfaces for:

- canonical event admission and queries;
- operation journal;
- outbox and delivery evidence;
- projection checkpoints and invalidation;
- private-artifact metadata;
- backup/restore contracts;
- storage status and integrity;
- atomic workflow commits.

`radroots-storage-sqlite` implements them.

### 12.2 Native layout

The SQLite V1 layout SHALL use:

```text
runtime.sqlite
    canonical event source
    event admission/visibility
    operation journal
    outbox and delivery evidence
    projection metadata and checkpoints

private.sqlite
    encrypted signing references
    private farm coordinates
    private trade artifacts
    NIP-46 private session material where host policy permits

host-owned databases
    UI preferences
    Studio state
    application presentation caches
```

`studio.sqlite` is removed from the SDK.

### 12.3 Correctness requirements

- One runtime database permits atomic event/journal/outbox transitions.
- Cross-database workflows use explicit staged commits and recovery markers.
- Public API exposes high-level atomic operations, not raw SQL transactions.
- No public `pool()` escape hatch.
- Open modes: read-only, read-write-existing, create.
- Explicit asynchronous `close()` and shutdown status.
- Advisory/process locking is mandatory for writable file stores.
- Concurrent readers are supported; writer policy is explicit.
- Migrations are transactional and forward-only by default.
- Downgrade requires an explicit offline export/import path.
- WAL and busy timeout are configured and reported.
- Backup uses a versioned manifest, per-member hashes, path/symlink validation, and atomic finalization.
- Restore uses staging, verification, and atomic replacement.
- Crash/failure injection covers every durable commit point.
- Secret material inclusion in backup is explicit and policy-controlled.

## 13. Identity, signing, and secret boundaries

### 13.1 Identity

`radroots-identity` contains no private key and no upstream Nostr event object. `PublicKey` is a validated canonical Radroots author key. NIP-19/npub conversion lives in `radroots-nostr`.

### 13.2 Signing

`radroots-signing::Signer` signs a frozen canonical `EventDraft`. The signing layer:

- authorizes actor role and expected public key before invoking a signer;
- verifies the signer result matches the exact draft;
- supports local and remote implementations;
- exposes capability and progress data;
- defines cancellation before and after remote request publication;
- never logs or serializes private material.

Concrete local Nostr signing lives in `radroots-nostr`; NIP-46 protocol state lives in `radroots-nostr-connect`; host composition lives in `radroots-sdk`.

### 13.3 Secrets

Secret-bearing values:

- MUST NOT implement ordinary `Debug`;
- MUST NOT implement `Serialize`;
- MUST NOT implement `Clone` unless the clone is a reference/handle;
- MUST zeroize owned plaintext where technically possible;
- MUST expose only redacted diagnostics;
- MUST use typed `SecretRef` handles across storage boundaries.

## 14. Event and trade model corrections

1. `radroots-event` remains the canonical owner of event-bound identifiers and trade wire identities.
2. `radroots-trade` MUST delete its conflicting `TradeId(OrderId)` definition.
3. `TradeId` and `OrderId` MUST remain semantically distinct.
4. `radroots-trade` consumes canonical event trade models and owns reducers, evidence, validation, and workflow plans.
5. Trade MUST NOT depend on authority, storage, SQLx, Nostr clients, or transports.
6. Event codec MUST own wire conversion; trade reducers operate on validated native inputs.
7. Typed authoring policy MUST reject reserved event kinds before any signer is consulted.
8. Native event typestates distinguish raw, ID-verified, signature-verified, contract-validated, admitted, and visible events.

## 15. Error model

Each crate owns a native `Error` with preserved sources. `radroots-protocol::error::v1::ErrorReport` is the serialized boundary.

One generated authority MUST define:

- stable code;
- class;
- retryability;
- recovery actions;
- capability ID;
- operation ID;
- safe structured details;
- redaction behavior.

Hand-maintained duplicate match tables and a separate unsynchronized catalog are forbidden.

Third-party errors MUST NOT appear as public variants. Sensitive source messages MUST be redacted before entering a protocol report or tracing field.

## 16. SDK and façade rules

### 16.1 `radroots-sdk`

- `Client` is `Clone + Send + Sync`.
- The host owns the executor and scheduling.
- The SDK starts no unbounded or hidden worker.
- Background processing requires an explicit returned worker/driver handle.
- Dropping a future before/after commit has documented effects.
- `Client::close()` is explicit and asynchronous where storage is active.
- Product writes retain prepare → authorize/sign → durable enqueue → optional deliver semantics.
- Lower canonical types are reused rather than copied into `Sdk*` wrappers.
- No `RadrootsSdk*` prefixes inside the crate.
- Root exports are only `Client`, `ClientBuilder`, `Error`, and `Result`.

### 16.2 `radroots`

- Primary documentation and examples use `radroots`.
- The façade adds curated modules, convenience constructors, safe defaults, and domain aggregation.
- Advanced hosts use `radroots-sdk` directly.
- There is no `radroots::sdk` public namespace.
- The façade does not expose implementation crates accidentally.

## 17. Public code generation and cross-language policy

- Rust binding, WASM, UniFFI, Swift, Kotlin, TypeScript, and codegen crates remain `publish = false`.
- Public runtime crates have no codegen feature or codegen dependency.
- Versioned language DTOs derive from `radroots-protocol`.
- Deterministic event algorithms derive from `radroots-event-codec`.
- Generated artifacts are checked in or generated reproducibly and carry source hashes.
- Rust native module structure is not mechanically mirrored into other languages.
- Language runtimes own networking, scheduling, keychain prompts, and UI lifecycle where appropriate.

## 18. Private and deferred packages

The following remain private in release V1:

```text
replica schema/store/sync family
Reticulum adapter
mesh protocol/agent/client family
SimpleX protocol/crypto/store/runtime family
NostrDB adapter
SP1 guest/host
runtime paths/manager/distribution helpers
radrootsd SDK adapter implementation
FFI, binding, WASM, and generated-package build crates
fixtures, conformance runners, fuzz targets, and xtask
```

Private preview code remains tested. It may become public only after passing the new-package admission rule.

## 19. New-package admission rule

After release V1, a new `radroots-*` package requires an ADR proving:

1. a durable domain/protocol/SPI/backend boundary;
2. at least two meaningful direct consumers, or one unavoidable platform/backend isolation boundary;
3. an independently supportable SemVer surface;
4. a publishable resolved dependency closure;
5. a name expected to survive five years;
6. why a module or feature is insufficient;
7. documentation, conformance tests, ownership, security review, and release automation.

Names containing `common`, `utils`, `types`, `models`, `preview`, `unstable`, `v1`, `v2`, `manager`, or a second `core` are presumptively rejected.

## 20. Current-to-target migration map

| Current package/family | Final owner | Required action |
|---|---|---|
| `radroots_core` | `radroots-core` | Retain and rename package with hyphens; remove RadrootsCore type prefixes. |
| `radroots_identity` | `radroots-identity + radroots-signing + radroots-secrets + radroots-storage` | Keep only public identity/account concepts in identity; move secrets, signers, and persistence. |
| `radroots_blossom` | `radroots-blossom` | Retain portable protocol primitives. |
| `radroots_protocol_contract_v1` | `radroots-protocol::event::v1 / capability::v1` | Merge; generation becomes a module, never a package suffix. |
| `radroots_runtime_contract_v1` | `radroots-protocol::runtime::v1` | Merge; generation becomes a module. |
| `radroots_transport_publish_protocol` | `radroots-protocol::radrootsd::transport_publish::v5` | Merge daemon wire DTOs into the versioned protocol package. |
| `radroots_event` | `radroots-event` | Retain singular package; narrow to canonical event-domain model. |
| `radroots_event_codec` | `radroots-event-codec` | Retain; remove live Nostr/upstream client responsibilities. |
| `radroots_event_index` | `radroots-storage::projection/index` | Merge; current checkpoint/manifest model is not an independent indexing engine. |
| `radroots_trade` | `radroots-trade` | Retain algorithms; remove authority, storage, SQL, transport, and duplicate TradeId. |
| `radroots_authority` | `radroots-identity + radroots-event::contract + radroots-signing` | Split account/public-key ownership, author-role contracts, and signing/authorization SPI. |
| `radroots_transport` | `radroots-transport` | Retain and redesign as extensible source/sink SPI. |
| `radroots_transport_nostr` | `radroots-transport-nostr` | Retain adapter; remove storage and sync orchestration. |
| `radroots_transport_reticulum` | `private preview` | Withhold until a real adapter passes the transport conformance suite. |
| `radroots_nostr` | `radroots-nostr` | Retain protocol conversion; remove live relay client and broad upstream aliases. |
| `radroots_nostr_connect` | `radroots-nostr-connect` | Retain as independent bidirectional NIP-46 protocol boundary. |
| `radroots_nostr_accounts` | `radroots-identity + radroots-secrets + radroots-storage + radroots-sdk` | Split mixed account, vault, persistence, and manager responsibilities. |
| `radroots_nostr_signer` | `radroots-signing + radroots-nostr-connect + Myc-private state` | Do not publish current service-state package. |
| `radroots_nostr_runtime` | `radroots-transport-nostr + radroots-sync` | Merge live relay runtime into adapter/orchestration layers. |
| `radroots_nostrdb` | `private; possible future radroots-storage-nostrdb` | Withhold until the storage SPI and external consumers justify a backend package. |
| `radroots_event_store` | `radroots-storage + radroots-storage-sqlite` | Split backend-neutral contracts from SQLite implementation. |
| `radroots_outbox` | `radroots-storage + radroots-storage-sqlite` | Merge as one persistence capability with atomic operation commits. |
| `radroots_runtime_store` | `radroots-storage or host-private state` | Retire broad name and classify each table by owner. |
| `radroots_sql_core` | `radroots-storage-sqlite private internals` | Remove raw SQL/JSON executor from public API. |
| `radroots_secret_vault` | `radroots-secrets` | Merge provider/wrapping SPI. |
| `radroots_protected_store` | `radroots-secrets` | Merge encrypted-envelope semantics. |
| `radroots_geocoder` | `radroots-geonames` | Rename to the actual concrete provider. |
| `radroots_runtime` | `radroots-sync + radroots-storage + host-private tooling` | Dismantle mixed config/signals/logging/queue/transport package. |
| `radroots_log` | `no replacement package` | Libraries emit tracing; hosts install subscribers. |
| `radroots_net` | `radroots-transport + radroots-sync + radroots-sdk` | Retire broad duplicated network/runtime package. |
| `radroots_runtime_paths` | `private host utility` | Do not place host path policy in the SDK registry closure. |
| `radroots_runtime_manager` | `private host tooling` | Runtime installation and process lifecycle remain host-owned. |
| `radroots_runtime_distribution` | `private host tooling` | Artifact distribution is not a public SDK dependency. |
| `radroots_replica_*` | `private/deferred` | Preserve and redesign; no public names until generated CRUD/raw SQL surfaces are replaced. |
| `radroots_mesh_*` | `private preview` | Preserve; publish only a real adapter or protocol with external consumers. |
| `radroots_simplex_*` | `private preview` | Preserve internal decomposition; no crates.io commitment in release v1. |
| `radroots_trade_sp1_*` | `private build/preview` | Keep specialized guest/host packages private. |
| `binding, WASM, FFI, codegen, fixtures, xtask` | `private build/test packages` | Publish generated language artifacts, not Rust build machinery. |

## 21. Workspace and manifest policy

```toml
[workspace]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.97.1"
license = "MIT OR Apache-2.0"
homepage = "https://radroots.org"
```

Each standalone workspace sets its own repository metadata:

```toml
# radrootslabs/lib
[workspace.package]
repository = "https://github.com/radrootslabs/lib"
```

```toml
# radrootslabs/sdk
[workspace.package]
repository = "https://github.com/radrootslabs/sdk"
```

Every public package MUST define:

```toml
[package]
name = "radroots-..."
version = "0.1.0"
publish = ["crates-io"]
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
documentation = "https://docs.rs/radroots-..."
```

Additional rules:

- Use an explicit `include` whitelist.
- Include both license files.
- Every same-repository public dependency uses `path + version`.
- Every dependency from `radrootslabs/sdk` to a package owned by
  `radrootslabs/lib` uses a registry version and MUST NOT use a sibling path or
  Git override in a release candidate.
- No Git dependency exists in a published normal/build/target/optional closure.
- Public packages avoid build scripts; generated source is checked and freshness-tested.
- Workspace lints forbid unsafe code by default.
- Public API crates deny broken rustdoc links.
- Package metadata lists only accurate keywords/categories.
- docs.rs metadata selects an intentional feature set instead of blindly enabling platform-incompatible features.

## 22. Versioning and release policy

### 22.1 Initial versions

All 19 packages start at `0.1.0`. This document's “V1” is the architecture specification version, not a claim that Rust APIs are already 1.0-stable.

### 22.2 SemVer groups

- `radroots` and `radroots-sdk` release in lockstep and `radroots` uses `=X.Y.Z` for the SDK.
- Lower packages version independently after the first release.
- Lower dependencies use ordinary compatible requirements at the actual minimum supported version.
- Exact requirements are reserved for genuinely inseparable package pairs.
- Wire/event/runtime/storage/backup/generated-schema versions are independent of Cargo package versions.
- Every package gets its own changelog section and public API baseline.
- The repository maintains a tested compatibility manifest for the current SDK release.

### 22.3 Breaking changes before 1.0

For `0.y.z` packages:

- breaking API changes increment `y`;
- compatible fixes/features increment `z`;
- package names and responsibility charters remain permanent;
- moving a public type between packages is breaking and requires an ADR;
- first-party consumers migrate in the same coordinated cutover.

## 23. Indicative publication order

The actual order is computed from `cargo metadata`; the expected order is:

```text
radroots-core
radroots-identity
radroots-blossom
radroots-protocol
radroots-secrets
radroots-geonames
radroots-event
radroots-event-codec
radroots-trade
radroots-signing
radroots-transport
radroots-nostr
radroots-nostr-connect
radroots-storage
radroots-storage-sqlite
radroots-transport-nostr
radroots-sync
radroots-sdk
radroots
```

Actual publication waits for each dependency to appear in the crates.io index before publishing dependents.

## 24. Required CI and release gates

### 24.1 Workspace

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo doc --workspace --no-deps`
- doctests
- generated-output freshness
- protocol/contract/conformance validation
- forbidden identifier and dependency checks

### 24.2 Feature/target matrix

For every public package:

- no default features;
- default features;
- each public feature independently;
- supported feature bundles;
- all features;
- MSRV 1.97.1;
- current stable;
- Linux, macOS, Windows;
- declared no_std target;
- `wasm32-unknown-unknown` where supported;
- native package targets;
- minimal and latest compatible dependencies.

### 24.3 Public API

- `cargo-semver-checks`;
- public API baseline;
- rustdoc with warnings denied;
- no undocumented public item exceptions without review;
- examples compile from packaged artifacts;
- no duplicate canonical type paths except deliberate façade reexports;
- no third-party type leakage in generic packages.

### 24.4 Reliability and security

- fuzz event, protocol, NIP-46, URL, manifest, backup, and restore parsers;
- malformed and oversized network payloads;
- cancellation before and after commit;
- idempotency replay and conflict;
- outbox claim expiry;
- partial delivery and retry;
- signer timeout/wrong response/auth challenge;
- storage multi-reader/writer/locking;
- migration and corruption failure;
- crash recovery at every commit point;
- backup/restore interruption, traversal, symlink, and hash mismatch;
- projection invalidation/rebuild;
- secret redaction;
- dependency audit, license policy, provenance, SBOM, and advisory checks.

### 24.5 Package-realistic release validation

For every public package:

1. Resolve the graph with `cargo metadata`.
2. Reject all public-to-private edges across normal, optional, build, target, and reachable-feature dependencies.
3. Run `cargo package --locked`.
4. Inspect the normalized manifest.
5. Inspect `cargo package --list`.
6. Extract the `.crate`.
7. Build, test, and document the extracted package.
8. Run `cargo publish --dry-run --locked`.
9. Publish to an ephemeral/local registry.
10. Build clean external projects against registry artifacts.
11. Build CLI, Studio, FFI/mobile, web packages, services, and indexers against package artifacts.
12. Confirm no sibling path or Git override remains.

## 25. Source-evidence record

| Repository | Reviewed SHA | Source | Pressure-test finding |
|---|---|---|---|
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `Cargo.toml` | Workspace currently contains 49+ packages, uses edition 2024 with resolver 2, and centralizes underscore-named path dependencies. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `contracts/releases/publish_policy.toml` | Current public classification contains broad runtime/storage/tooling crates while SDK-required packages remain internal. |
| `radrootslabs/sdk` | `fd8384aee348034e0c8ea17a868fe7f094770050` | `crates/sdk/Cargo.toml` | SDK feature graph names implementation assembly and directly references private authority, event-store, outbox, transport, and adapter crates. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `crates/authority/src/{actor,authorization,signer}.rs` | Current authority package combines account provenance, event contract roles, signing SPI, authorization, and concrete local signing. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `crates/event/src/ids.rs` | Semantic identifiers are stored as Strings, implement Deref<str>, and include trade/account/network concepts in one module. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `crates/trade/src/identity.rs` | A second TradeId wraps OrderId, conflicting with the canonical event TradeId concept. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `crates/identity/src/identity.rs` | Identity owns raw secret keys, upstream Nostr event values, file formats, generation, and secret export methods. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `crates/transport/src/{kind,transport}.rs` | TransportKind is a closed enum and one monolithic trait requires both fetch and deliver. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `crates/transport_nostr/{Cargo.toml,src/lib.rs}` | Nostr adapter currently couples relay transport to event-store and outbox persistence. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `crates/event_store/src/store.rs and crates/outbox/src/store.rs` | Concrete stores expose SQLx pools/transactions and split related runtime state across separately opened stores. |
| `radrootslabs/lib` | `466f3cc36739179bc17edb9db796530729ba5219` | `crates/runtime/src/lib.rs and crates/log/src/init.rs` | Runtime package combines host concerns; logging installs process-global subscriber state. |
| `radrootslabs/sdk` | `fd8384aee348034e0c8ea17a868fe7f094770050` | `crates/sdk/src/{lib,runtime,studio_store,error}.rs` | SDK root broadly reexports implementation-shaped types, owns studio.sqlite, exposes many public fields, and duplicates error metadata. |

## 26. Rejected alternatives

### Publish the current public list

Rejected because it permanently exposes host tooling and still omits SDK-required private dependencies.

### Publish every current dependency unchanged

Rejected because temporary implementation boundaries would become permanent package identities.

### Collapse lower crates into `radroots-sdk`

Rejected because domain, protocol, SPI, backend, and adapter packages have independent consumers and semver responsibilities.

### One `radroots-runtime` package

Rejected because runtime configuration, process lifecycle, paths, logging, queues, storage, and networking are not one coherent library boundary.

### Separate packages for every domain or NIP

Rejected because it creates package proliferation. Only independently substantial protocols with direct consumers—such as Nostr Connect—receive a package.

### Versioned package names

Rejected. Versions belong in modules and schema IDs.

### Public preview placeholder packages

Rejected. Preview code remains private until the implementation is real and conformance-tested.

### Public codegen features

Rejected. Code generation is a private build concern and must not enlarge the runtime registry closure.

## 27. Cutover sequence

1. Freeze publication and record this specification as an ADR.
2. Preserve the independent `radrootslabs/lib` and `radrootslabs/sdk`
   histories; record the 17/2 package allocation and synchronized contract
   hashes without forming or importing a third repository.
3. Switch workspace to resolver 3 and MSRV 1.97.1.
4. Create final hyphenated package manifests with `publish = false` during migration.
5. Refactor identity/public-key ownership and remove all secret material.
6. Split authority among identity, event contracts, and signing.
7. Remove the duplicate TradeId and make trade algorithm-only.
8. Create `radroots-protocol`.
9. Create `radroots-secrets`.
10. Create `radroots-storage` and `radroots-storage-sqlite`; migrate event/outbox/journal/private storage.
11. Remove Studio state from SDK storage.
12. Redesign `radroots-transport`; separate source/sink.
13. Narrow `radroots-nostr` and `radroots-transport-nostr`.
14. Refactor and retain `radroots-nostr-connect`.
15. Create `radroots-sync`.
16. Rename/refocus GeoNames.
17. Refactor SDK root, features, errors, lifecycle, and commit semantics.
18. Add the curated `radroots` façade.
19. Migrate every first-party consumer.
20. Run package-realistic validation.
21. Change only the 19 final packages to `publish = ["crates-io"]`.
22. Publish in Cargo-derived order after explicit authorization.

## 28. Completion and publication decision

The package identities and boundaries in this specification are final for release V1.

Publication is authorized only when all of the following are simultaneously true:

- all 19 packages implement their charters;
- no forbidden responsibility remains;
- Cargo-resolved closure contains only public Radroots packages;
- all feature/target/package/downstream gates pass;
- naming availability and ownership are confirmed;
- current-source licensing and contributor provenance are cleared;
- generated cross-language contracts are coherent;
- no first-party host remains on legacy or sibling-path APIs;
- the actual `.crate` archives have been inspected and tested.

Until then, the correct status is:

```text
Architecture: FINAL
Implementation: REQUIRED
Publication: BLOCKED
```

## 29. Final reaffirmation

This is the final recommended Radroots crates surface.

It preserves real modularity without turning every current workspace folder into a permanent public package. It establishes stable identities for foundational values, identity, event protocol, trade algorithms, signing, transport, secrets, storage, Nostr, synchronization, a concrete geodata provider, the advanced SDK, and the ordinary-user façade.

It intentionally withholds host utilities, preview transports, generated CRUD/replica code, experimental messaging/proof systems, codegen, FFI machinery, and test support.

No additional public crate is required for release V1, and no package in the 19-package family is present merely to satisfy Cargo. Each has a durable responsibility, named consumers, a one-way dependency position, and a credible independent SemVer surface.
