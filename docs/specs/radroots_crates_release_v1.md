# Radroots Crates Release V1

**Normative identifier:** `radroots.crates.release.v1`
**Document status:** Final pressure-tested architecture and publication specification
**Date:** 2026-07-26
**Coverage amendment:** 2026-07-28 temporary heavy-development baseline
**Source snapshots reviewed:**
- `radrootslabs/lib@466f3cc36739179bc17edb9db796530729ba5219`
- `radrootslabs/sdk@fd8384aee348034e0c8ea17a868fe7f094770050`

**Repository allocation:** the existing `radrootslabs/lib` and
`radrootslabs/sdk` repositories remain independent. The first 17 packages are
owned by `lib`; `radroots_sdk` and `radroots` are owned by `sdk`. No third Rust
repository is created for release V1.

**Registry context supplied by the project:** no Radroots crate is currently published on crates.io. Deleted experimental publications create no compatibility, pluralization, deprecation, or version-continuity requirement.

## 1. Normative language and status

The words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

This specification freezes the **package identities, ownership boundaries, naming model, dependency direction, and release gates** for the first durable Radroots crates.io release.

It does not claim that the current source tree is already publishable. Publication remains blocked until the refactor is implemented and every acceptance gate in this document passes against packaged artifacts.

## 2. Final executive decision

Radroots SHALL publish exactly **19 durable package identities** for release V1:

1. `radroots_core`
2. `radroots_identity`
3. `radroots_blossom`
4. `radroots_protocol`
5. `radroots_event`
6. `radroots_event_codec`
7. `radroots_trade`
8. `radroots_signing`
9. `radroots_transport`
10. `radroots_nostr`
11. `radroots_nostr_connect`
12. `radroots_secrets`
13. `radroots_storage`
14. `radroots_storage_sqlite`
15. `radroots_transport_nostr`
16. `radroots_sync`
17. `radroots_geonames`
18. `radroots_sdk`
19. `radroots`

This is neither the current workspace publication list nor a collapse into `radroots_sdk`.

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

1. **`radroots_contracts` becomes `radroots_protocol`.** The package is a durable, versioned wire/operation protocol boundary rather than a general-purpose “contracts” bucket.
2. **`radroots_store` and `radroots_store_sqlite` become `radroots_storage` and `radroots_storage_sqlite`.** “Storage” is unambiguous in an agricultural marketplace and describes the package family more accurately than “store.”
3. **`radroots_nostr_connect` remains independent.** It is a bidirectional security protocol with URIs, permissions, client/server state, and independent SDK/Myc consumers. It is not merely a convenience NIP module.
4. **Actor ownership is refined.** Public keys/accounts live in identity; event author roles live in the event contract model; actor provenance, authorization, and signer behavior live in signing.
5. **Trade identity is made singular.** The conflicting `TradeId`/`OrderId` definitions MUST be replaced by one canonical protocol `TradeId` and a separately named business `OrderId`.
6. **Public codegen features are removed.** `dto-bindgen`, binding generation, WASM wrappers, and fixture switches remain private build/test concerns.
7. **The workspace moves to Cargo resolver 3.** A virtual Rust 2024 workspace MUST explicitly set `resolver = "3"`.
8. **Release V1 uses MSRV 1.97.1.** The patch release is selected rather than 1.97.0 because it contains a compiler miscompilation fix.
9. **Lower crates do not remain permanently lockstep.** The current
   `radrootslabs/lib` development cohort is frozen at `0.1.0-alpha`, while
   `radroots` and `radroots_sdk` remain an exact `0.1.0` lockstep pair. Lower
   packages may follow independent SemVer only after explicit future authority
   ends the temporary library version freeze.

## 4. Non-negotiable architecture invariants

1. Every public package MUST have a durable name and at least one named direct consumer besides `radroots`.
2. Every dependency edge MUST point downward in the architecture.
3. Domain and protocol packages MUST NOT depend on storage, networking, process lifecycle, or host UI.
4. Generic SPIs MUST NOT expose concrete SQLite, Tokio, Reqwest, Nostr SDK, keyring, or OS-specific types.
5. Adapters and backends MUST implement SPIs; SPIs MUST NOT depend on adapters or backends.
6. Synchronization MUST orchestrate sources, sinks, signing, and storage without owning an executor or scheduler.
7. `radroots_sdk` MUST compose lower packages and own client-level commit semantics; it MUST NOT become a dumping ground for code that has an independent durable boundary.
8. `radroots` MUST provide meaningful curation and documentation; it MUST NOT be `pub use radroots_sdk::*`.
9. Version generations MUST live in modules and schema IDs, never in package names.
10. Preview implementation code MAY remain in the monorepo but MUST NOT appear in a published dependency or feature until registry-ready.
11. No public package may have a normal, optional, build, or target-specific dependency on a private Radroots package.
12. No first-party consumer may rely on production sibling paths after cutover.

## 5. Final public package inventory

### 5.1 Repository ownership

- `https://github.com/radrootslabs/lib` owns `radroots_core` through
  `radroots_geonames` (packages 1-17).
- `https://github.com/radrootslabs/sdk` owns `radroots_sdk` and `radroots`
  (packages 18-19).
- Both repositories retain their existing histories and remain independently
  buildable, testable, packageable, and releasable.
- The two repositories carry synchronized copies of this release-family
  contract. A coordinated release MUST reject any content-hash or package
  allocation mismatch between those copies.

| Order | Package | Rust crate path | Tier | Permanent responsibility |
|---:|---|---|---|---|
| 1 | `radroots_core` | `radroots_core` | foundation | Foundational value objects and deterministic invariants: decimal, currency, money, percentage, quantity, units, and pricing. |
| 2 | `radroots_identity` | `radroots_identity` | foundation | Public identity and account value types: canonical public keys, identity IDs, account IDs, public profiles, and usernames. |
| 3 | `radroots_blossom` | `radroots_blossom` | protocol primitive | Portable Blossom protocol primitives: canonical blob URLs, hashes, media descriptors, byte-verification typestates, and authorization claims. |
| 4 | `radroots_protocol` | `radroots_protocol` | versioned wire contract | Versioned cross-process and cross-language schemas, capability catalogs, operation descriptors, stable error reports, schema IDs, and structural validation. |
| 5 | `radroots_event` | `radroots_event` | domain protocol model | Canonical Radroots event-domain models, validated event identifiers, tags, event contracts, authoring drafts, signed/verified typestates, and NIP-01 wire-neutral representations. |
| 6 | `radroots_event_codec` | `radroots_event_codec` | deterministic algorithm | Deterministic canonical encoding, decoding, ID/signature verification, contract validation, admission, and manifest generation for Radroots events. |
| 7 | `radroots_trade` | `radroots_trade` | domain algorithm | Trade validation, evidence models, deterministic reduction, conflict analysis, and side-effect-free workflow plans over the canonical event trade model. |
| 8 | `radroots_signing` | `radroots_signing` | host SPI | Object-safe author/signing SPI, actor provenance, authorization checks, requests, receipts, progress, capabilities, and normalized signing errors. |
| 9 | `radroots_transport` | `radroots_transport` | network SPI | Transport-neutral target identities, capability/status models, source and sink SPIs, delivery/fetch policies, bounded requests, provenance, and normalized outcomes. |
| 10 | `radroots_nostr` | `radroots_nostr` | protocol adapter | Portable conversion between Radroots native event/identity types and Nostr protocol types, typed NIP helpers, and concrete local signing adapters; no live relay client. |
| 11 | `radroots_nostr_connect` | `radroots_nostr_connect` | security protocol | Nostr Connect/NIP-46 URIs, methods, permissions, requests, responses, client/server state machines, timeout-independent protocol validation, and normalized errors. |
| 12 | `radroots_secrets` | `radroots_secrets` | security SPI | Secret references, provider and key-wrapping SPIs, versioned encrypted envelopes, zeroization-safe secret handling, and explicit memory/file/keyring adapters. |
| 13 | `radroots_storage` | `radroots_storage` | storage SPI | Backend-neutral canonical event, operation journal, outbox, transport evidence, projection, private-artifact metadata, backup, status, and atomic commit interfaces, plus an in-memory reference backend. |
| 14 | `radroots_storage_sqlite` | `radroots_storage_sqlite` | native storage backend | SQLite implementation of the storage SPIs with schema migration, WAL, locking, integrity, backup/restore, crash recovery, and encrypted private storage. |
| 15 | `radroots_transport_nostr` | `radroots_transport_nostr` | native network adapter | Concrete Nostr EventSource/EventSink implementation: relay URL policy, connection, NIP-42 authentication, bounded fetch pages, delivery, status, and relay-outcome normalization. |
| 16 | `radroots_sync` | `radroots_sync` | local-first orchestration | Shared pull, verification, canonical admission, duplicate handling, projection refresh, outbox signing/delivery, status, and retry-decision orchestration without owning scheduling. |
| 17 | `radroots_geonames` | `radroots_geonames` | concrete data provider | GeoNames asset specification, authenticated/integrity-checked acquisition, database lifecycle, and forward/reverse locality lookup using provider-owned types. |
| 18 | `radroots_sdk` | `radroots_sdk` | advanced front door | Host-neutral asynchronous client engine, product operations, capability reporting, explicit storage/signing/transport composition, diagnostics, backup/restore, and safe commit semantics. |
| 19 | `radroots` | `radroots` | ordinary-user front door | Canonical Rust onboarding package with curated modules, safe defaults, stable convenience builders, domain aggregation, examples, and primary documentation. |

## 6. Detailed package specifications

    ### 1. `radroots_core`

    **Rust crate path:** `radroots_core`
    **Tier:** foundation
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots_event, radroots_trade, radroots_sdk, radroots

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


    ### 2. `radroots_identity`

    **Rust crate path:** `radroots_identity`
    **Tier:** foundation
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots_event, radroots_signing, radroots_transport, radroots_nostr, radroots_nostr_connect, radroots_sdk, services

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


    ### 3. `radroots_blossom`

    **Rust crate path:** `radroots_blossom`
    **Tier:** protocol primitive
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots_event, radroots_event_codec, radroots_nostr, media clients

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


    ### 4. `radroots_protocol`

    **Rust crate path:** `radroots_protocol`
    **Tier:** versioned wire contract
    **API maturity at first publish:** durable identity; independently versioned contract modules
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots_event, radroots_signing, radroots_transport, radroots_storage, radroots_sync, radroots_sdk, radrootsd, bindings

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


    ### 5. `radroots_event`

    **Rust crate path:** `radroots_event`
    **Tier:** domain protocol model
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc; std feature
    **Direct intended consumers:** radroots_event_codec, radroots_trade, radroots_signing, radroots_transport, radroots_storage, radroots_sync, radroots_nostr, radroots_sdk, indexers

    **Normative responsibility.** Canonical Radroots event-domain models, validated event identifiers, tags, event contracts, authoring drafts, signed/verified typestates, and NIP-01 wire-neutral representations.

    **Required Radroots dependencies:** `radroots_core`, `radroots_identity`, `radroots_blossom`, `radroots_protocol`
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


    ### 6. `radroots_event_codec`

    **Rust crate path:** `radroots_event_codec`
    **Tier:** deterministic algorithm
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc where selected features permit; std feature
    **Direct intended consumers:** radroots_nostr, radroots_storage_sqlite, radroots_transport_nostr, radroots_sync, radroots_sdk, bindings

    **Normative responsibility.** Deterministic canonical encoding, decoding, ID/signature verification, contract validation, admission, and manifest generation for Radroots events.

    **Required Radroots dependencies:** `radroots_event`, `radroots_blossom`, `radroots_protocol`
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


    ### 7. `radroots_trade`

    **Rust crate path:** `radroots_trade`
    **Tier:** domain algorithm
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc for model/reducer; std feature
    **Direct intended consumers:** radroots_storage, radroots_sync, radroots_sdk, RHI, applications

    **Normative responsibility.** Trade validation, evidence models, deterministic reduction, conflict analysis, and side-effect-free workflow plans over the canonical event trade model.

    **Required Radroots dependencies:** `radroots_core`, `radroots_identity`, `radroots_event`
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


    ### 8. `radroots_signing`

    **Rust crate path:** `radroots_signing`
    **Tier:** host SPI
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc core; std feature
    **Direct intended consumers:** radroots_nostr, radroots_sync, radroots_sdk, CLI, Studio, FFI hosts

    **Normative responsibility.** Object-safe author/signing SPI, actor provenance, authorization checks, requests, receipts, progress, capabilities, and normalized signing errors.

    **Required Radroots dependencies:** `radroots_identity`, `radroots_event`, `radroots_protocol`
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


    ### 9. `radroots_transport`

    **Rust crate path:** `radroots_transport`
    **Tier:** network SPI
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc data model; std feature
    **Direct intended consumers:** radroots_storage, radroots_transport_nostr, radroots_sync, radroots_sdk, services, future adapters

    **Normative responsibility.** Transport-neutral target identities, capability/status models, source and sink SPIs, delivery/fetch policies, bounded requests, provenance, and normalized outcomes.

    **Required Radroots dependencies:** `radroots_identity`, `radroots_event`, `radroots_protocol`
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


    ### 10. `radroots_nostr`

    **Rust crate path:** `radroots_nostr`
    **Tier:** protocol adapter
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc for conversion surface; std feature
    **Direct intended consumers:** radroots_nostr_connect, radroots_transport_nostr, radroots_sdk, Myc, radrootsd

    **Normative responsibility.** Portable conversion between Radroots native event/identity types and Nostr protocol types, typed NIP helpers, and concrete local signing adapters; no live relay client.

    **Required Radroots dependencies:** `radroots_identity`, `radroots_event`, `radroots_event_codec`
    **Optional Radroots dependencies:** `radroots_signing`, `radroots_blossom`
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


    ### 11. `radroots_nostr_connect`

    **Rust crate path:** `radroots_nostr_connect`
    **Tier:** security protocol
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std for v1; protocol data kept portable
    **Direct intended consumers:** radroots_sdk, Myc, remote signer tooling

    **Normative responsibility.** Nostr Connect/NIP-46 URIs, methods, permissions, requests, responses, client/server state machines, timeout-independent protocol validation, and normalized errors.

    **Required Radroots dependencies:** `radroots_identity`, `radroots_event`, `radroots_nostr`, `radroots_protocol`
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


    ### 12. `radroots_secrets`

    **Rust crate path:** `radroots_secrets`
    **Tier:** security SPI
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** no_std + alloc core; std adapters
    **Direct intended consumers:** radroots_storage_sqlite, radroots_sdk, signing hosts, services

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


    ### 13. `radroots_storage`

    **Rust crate path:** `radroots_storage`
    **Tier:** storage SPI
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std for v1
    **Direct intended consumers:** radroots_storage_sqlite, radroots_sync, radroots_sdk, indexers, tests, future backends

    **Normative responsibility.** Backend-neutral canonical event, operation journal, outbox, transport evidence, projection, private-artifact metadata, backup, status, and atomic commit interfaces, plus an in-memory reference backend.

    **Required Radroots dependencies:** `radroots_event`, `radroots_trade`, `radroots_transport`, `radroots_protocol`
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


    ### 14. `radroots_storage_sqlite`

    **Rust crate path:** `radroots_storage_sqlite`
    **Tier:** native storage backend
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std-only native backend
    **Direct intended consumers:** radroots_sdk, CLI, Studio, mobile/FFI

    **Normative responsibility.** SQLite implementation of the storage SPIs with schema migration, WAL, locking, integrity, backup/restore, crash recovery, and encrypted private storage.

    **Required Radroots dependencies:** `radroots_storage`, `radroots_event_codec`, `radroots_secrets`
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


    ### 15. `radroots_transport_nostr`

    **Rust crate path:** `radroots_transport_nostr`
    **Tier:** native network adapter
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std-only; Tokio implementation detail in v1
    **Direct intended consumers:** radroots_sdk, CLI, radrootsd, advanced hosts

    **Normative responsibility.** Concrete Nostr EventSource/EventSink implementation: relay URL policy, connection, NIP-42 authentication, bounded fetch pages, delivery, status, and relay-outcome normalization.

    **Required Radroots dependencies:** `radroots_transport`, `radroots_nostr`, `radroots_event_codec`, `radroots_protocol`
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


    ### 16. `radroots_sync`

    **Rust crate path:** `radroots_sync`
    **Tier:** local-first orchestration
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std-only in v1; executor-neutral public API
    **Direct intended consumers:** radroots_sdk, CLI, Studio, mobile/FFI, advanced hosts

    **Normative responsibility.** Shared pull, verification, canonical admission, duplicate handling, projection refresh, outbox signing/delivery, status, and retry-decision orchestration without owning scheduling.

    **Required Radroots dependencies:** `radroots_event`, `radroots_event_codec`, `radroots_signing`, `radroots_transport`, `radroots_storage`, `radroots_trade`, `radroots_protocol`
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


    ### 17. `radroots_geonames`

    **Rust crate path:** `radroots_geonames`
    **Tier:** concrete data provider
    **API maturity at first publish:** durable identity; pre-1.0 API
    **Platform contract:** std-only
    **Direct intended consumers:** radroots_sdk, CLI, geocoding applications

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


    ### 18. `radroots_sdk`

    **Rust crate path:** `radroots_sdk`
    **Tier:** advanced front door
    **API maturity at first publish:** durable identity; lockstep pre-1.0 API with radroots
    **Platform contract:** std-only native engine
    **Direct intended consumers:** CLI, Studio, FFI/mobile, advanced native applications

    **Normative responsibility.** Host-neutral asynchronous client engine, product operations, capability reporting, explicit storage/signing/transport composition, diagnostics, backup/restore, and safe commit semantics.

    **Required Radroots dependencies:** `radroots_core`, `radroots_identity`, `radroots_protocol`, `radroots_event`, `radroots_event_codec`, `radroots_trade`, `radroots_signing`, `radroots_transport`, `radroots_storage`
    **Optional Radroots dependencies:** `radroots_secrets`, `radroots_storage_sqlite`, `radroots_nostr`, `radroots_nostr_connect`, `radroots_transport_nostr`, `radroots_sync`, `radroots_geonames`
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
    **API maturity at first publish:** durable identity; lockstep pre-1.0 API with radroots_sdk
    **Platform contract:** std-only
    **Direct intended consumers:** ordinary Rust applications, examples, documentation

    **Normative responsibility.** Canonical Rust onboarding package with curated modules, safe defaults, stable convenience builders, domain aggregation, examples, and primary documentation.

    **Required Radroots dependencies:** `radroots_sdk`, `radroots_core`, `radroots_identity`, `radroots_event`, `radroots_trade`, `radroots_transport`
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

    **Explicitly forbidden.** A public radroots::sdk namespace, wildcard reexport of radroots_sdk, duplicate engine implementation, CLI binary, hidden network/filesystem/keychain side effects, or exposure of every lower-crate symbol.


## 7. Normative dependency graph

### 7.1 Direct Radroots edges

| Dependency | Dependent | Edge |
|---|---|---|
| `radroots_blossom` | `radroots_event` | required |
| `radroots_core` | `radroots_event` | required |
| `radroots_identity` | `radroots_event` | required |
| `radroots_protocol` | `radroots_event` | required |
| `radroots_blossom` | `radroots_event_codec` | required |
| `radroots_event` | `radroots_event_codec` | required |
| `radroots_protocol` | `radroots_event_codec` | required |
| `radroots_core` | `radroots_trade` | required |
| `radroots_event` | `radroots_trade` | required |
| `radroots_identity` | `radroots_trade` | required |
| `radroots_event` | `radroots_signing` | required |
| `radroots_identity` | `radroots_signing` | required |
| `radroots_protocol` | `radroots_signing` | required |
| `radroots_event` | `radroots_transport` | required |
| `radroots_identity` | `radroots_transport` | required |
| `radroots_protocol` | `radroots_transport` | required |
| `radroots_blossom` | `radroots_nostr` | optional |
| `radroots_signing` | `radroots_nostr` | optional |
| `radroots_event` | `radroots_nostr` | required |
| `radroots_event_codec` | `radroots_nostr` | required |
| `radroots_identity` | `radroots_nostr` | required |
| `radroots_event` | `radroots_nostr_connect` | required |
| `radroots_identity` | `radroots_nostr_connect` | required |
| `radroots_nostr` | `radroots_nostr_connect` | required |
| `radroots_protocol` | `radroots_nostr_connect` | required |
| `radroots_event` | `radroots_storage` | required |
| `radroots_protocol` | `radroots_storage` | required |
| `radroots_trade` | `radroots_storage` | required |
| `radroots_transport` | `radroots_storage` | required |
| `radroots_event_codec` | `radroots_storage_sqlite` | required |
| `radroots_secrets` | `radroots_storage_sqlite` | required |
| `radroots_storage` | `radroots_storage_sqlite` | required |
| `radroots_event_codec` | `radroots_transport_nostr` | required |
| `radroots_nostr` | `radroots_transport_nostr` | required |
| `radroots_protocol` | `radroots_transport_nostr` | required |
| `radroots_transport` | `radroots_transport_nostr` | required |
| `radroots_event` | `radroots_sync` | required |
| `radroots_event_codec` | `radroots_sync` | required |
| `radroots_protocol` | `radroots_sync` | required |
| `radroots_signing` | `radroots_sync` | required |
| `radroots_storage` | `radroots_sync` | required |
| `radroots_trade` | `radroots_sync` | required |
| `radroots_transport` | `radroots_sync` | required |
| `radroots_geonames` | `radroots_sdk` | optional |
| `radroots_nostr` | `radroots_sdk` | optional |
| `radroots_nostr_connect` | `radroots_sdk` | optional |
| `radroots_secrets` | `radroots_sdk` | optional |
| `radroots_storage_sqlite` | `radroots_sdk` | optional |
| `radroots_sync` | `radroots_sdk` | optional |
| `radroots_transport_nostr` | `radroots_sdk` | optional |
| `radroots_core` | `radroots_sdk` | required |
| `radroots_event` | `radroots_sdk` | required |
| `radroots_event_codec` | `radroots_sdk` | required |
| `radroots_identity` | `radroots_sdk` | required |
| `radroots_protocol` | `radroots_sdk` | required |
| `radroots_signing` | `radroots_sdk` | required |
| `radroots_storage` | `radroots_sdk` | required |
| `radroots_trade` | `radroots_sdk` | required |
| `radroots_transport` | `radroots_sdk` | required |
| `radroots_core` | `radroots` | required |
| `radroots_event` | `radroots` | required |
| `radroots_identity` | `radroots` | required |
| `radroots_sdk` | `radroots` | required |
| `radroots_trade` | `radroots` | required |
| `radroots_transport` | `radroots` | required |

### 7.2 Architectural graph

```text
radroots_core       radroots_identity       radroots_blossom
       \                 |                       /
        \                |                      /
         +---------- radroots_protocol --------+
                          |
                     radroots_event
                       /   |   \
                      /    |    \
       radroots_event_codec |  radroots_trade
                            |
                 +----------+-----------+
                 |          |           |
        radroots_signing  radroots_transport  radroots_storage
                 |          |           |
                 |      radroots_nostr   +--> radroots_storage_sqlite
                 |          |
                 |      radroots_nostr_connect
                 |          |
                 +---- radroots_transport_nostr
                            |
                       radroots_sync

radroots_geonames -------------------------+
                                           |
all selected lower packages ----------> radroots_sdk ---> radroots
```

This diagram is explanatory. The release tool MUST use Cargo-resolved metadata as authority.

## 8. Type ownership and canonical paths

| Concept | Canonical owning crate | Rule |
|---|---|---|
| Decimal, money, quantity, unit, pricing | `radroots_core` | No duplicate wrapper in SDK or trade. |
| Public key, identity ID, account ID, username | `radroots_identity` | Secret material is forbidden here. |
| Event ID, event signature, coordinate, D-tag, event kind | `radroots_event` | Store bytes/newtypes, not unvalidated Strings. |
| Event contract author role | `radroots_event::contract` | This is an event-authoring rule, not an account property. |
| Actor provenance and author context | `radroots_signing` | Combines identity with event author roles at the signing boundary. |
| Canonical protocol TradeId/CandidateId/MutationId | `radroots_event::trade` | Exactly one definition. |
| Human/business OrderId | `radroots_trade` | MUST NOT be aliased or wrapped as TradeId. |
| Runtime/wire DTO generations | `radroots_protocol` | Native packages convert at boundaries. |
| Secret references and encrypted envelopes | `radroots_secrets` | No domain-specific tables. |
| Transport ID, target, capability, outcome | `radroots_transport` | TransportId is extensible, not a closed enum. |
| Native event/outbox/journal/projection storage | `radroots_storage` | Backend-neutral interfaces only. |
| SQLite schema and connection behavior | `radroots_storage_sqlite` | SQLx remains private. |
| Relay URL and Nostr network status | `radroots_transport_nostr` | Protocol conversions remain in `radroots_nostr`. |
| Pull/push/ingest/projection orchestration | `radroots_sync` | No host scheduling. |
| Client-level requests, plans, receipts, diagnostics | `radroots_sdk` | Use lower canonical types rather than duplicate wrappers. |

## 9. Rust API and naming law

### 9.1 Packages, crates, modules, and types

- Cargo package names MUST use lowercase snake case: `radroots_event_codec`.
- Rust crate paths MUST use the same lowercase snake case: `radroots_event_codec`.
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
- Every native public struct MUST have private fields unless it is an intentionally passive versioned DTO in `radroots_protocol`.
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

The Rust SPI is native. Browser and generated-language transports use `radroots_protocol` DTOs and language-native interfaces rather than weakening native `Send + Sync` guarantees.

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

### 10.1 `radroots_sdk`

```toml
[features]
default = ["memory"]

# Safe, in-process storage; no files or network.
memory = ["radroots_storage/memory"]

# Explicit native capabilities.
sqlite = ["dep:radroots_storage_sqlite"]
sync = ["dep:radroots_sync"]
nostr = [
  "sync",
  "dep:radroots_nostr",
  "dep:radroots_transport_nostr",
]
nip46 = [
  "nostr",
  "dep:radroots_nostr_connect",
]
local-signing = [
  "dep:radroots_secrets",
  "radroots_nostr/signing",
]
radrootsd = [
  "sync",
  "dep:reqwest",
]
geonames = ["dep:radroots_geonames"]
knowledge = [
  "radroots_event/knowledge",
  "radroots_event_codec/knowledge",
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

client = ["radroots_sdk/default"]
native = ["client", "radroots_sdk/native"]
nostr = ["client", "radroots_sdk/nostr"]
nip46 = ["nostr", "radroots_sdk/nip46"]
radrootsd = ["client", "radroots_sdk/radrootsd"]
geonames = ["client", "radroots_sdk/geonames"]
knowledge = ["client", "radroots_sdk/knowledge"]
full = ["radroots_sdk/full"]
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

`radroots_storage` owns interfaces for:

- canonical event admission and queries;
- operation journal;
- outbox and delivery evidence;
- projection checkpoints and invalidation;
- private-artifact metadata;
- backup/restore contracts;
- storage status and integrity;
- atomic workflow commits.

`radroots_storage_sqlite` implements them.

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

`radroots_identity` contains no private key and no upstream Nostr event object. `PublicKey` is a validated canonical Radroots author key. NIP-19/npub conversion lives in `radroots_nostr`.

### 13.2 Signing

`radroots_signing::Signer` signs a frozen canonical `EventDraft`. The signing layer:

- authorizes actor role and expected public key before invoking a signer;
- verifies the signer result matches the exact draft;
- supports local and remote implementations;
- exposes capability and progress data;
- defines cancellation before and after remote request publication;
- never logs or serializes private material.

Concrete local Nostr signing lives in `radroots_nostr`; NIP-46 protocol state lives in `radroots_nostr_connect`; host composition lives in `radroots_sdk`.

### 13.3 Secrets

Secret-bearing values:

- MUST NOT implement ordinary `Debug`;
- MUST NOT implement `Serialize`;
- MUST NOT implement `Clone` unless the clone is a reference/handle;
- MUST zeroize owned plaintext where technically possible;
- MUST expose only redacted diagnostics;
- MUST use typed `SecretRef` handles across storage boundaries.

## 14. Event and trade model corrections

1. `radroots_event` remains the canonical owner of event-bound identifiers and trade wire identities.
2. `radroots_trade` MUST delete its conflicting `TradeId(OrderId)` definition.
3. `TradeId` and `OrderId` MUST remain semantically distinct.
4. `radroots_trade` consumes canonical event trade models and owns reducers, evidence, validation, and workflow plans.
5. Trade MUST NOT depend on authority, storage, SQLx, Nostr clients, or transports.
6. Event codec MUST own wire conversion; trade reducers operate on validated native inputs.
7. Typed authoring policy MUST reject reserved event kinds before any signer is consulted.
8. Native event typestates distinguish raw, ID-verified, signature-verified, contract-validated, admitted, and visible events.

## 15. Error model

Each crate owns a native `Error` with preserved sources. `radroots_protocol::error::v1::ErrorReport` is the serialized boundary.

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

### 16.1 `radroots_sdk`

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
- Advanced hosts use `radroots_sdk` directly.
- There is no `radroots::sdk` public namespace.
- The façade does not expose implementation crates accidentally.

## 17. Public code generation and cross-language policy

- Rust binding, WASM, UniFFI, Swift, Kotlin, TypeScript, and codegen crates remain `publish = false`.
- Public runtime crates have no codegen feature or codegen dependency.
- Versioned language DTOs derive from `radroots_protocol`.
- Deterministic event algorithms derive from `radroots_event_codec`.
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

After release V1, a new `radroots_*` package requires an ADR proving:

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
| `radroots_core` | `radroots_core` | Retain the snake-case package name and remove RadrootsCore type prefixes. |
| `radroots_identity` | `radroots_identity + radroots_signing + radroots_secrets + radroots_storage` | Keep only public identity/account concepts in identity; move secrets, signers, and persistence. |
| `radroots_blossom` | `radroots_blossom` | Retain portable protocol primitives. |
| `radroots_protocol_contract_v1` | `radroots_protocol::event::v1 / capability::v1` | Merge; generation becomes a module, never a package suffix. |
| `radroots_runtime_contract_v1` | `radroots_protocol::runtime::v1` | Merge; generation becomes a module. |
| `radroots_transport_publish_protocol` | `radroots_protocol::radrootsd::transport_publish::v5` | Merge daemon wire DTOs into the versioned protocol package. |
| `radroots_event` | `radroots_event` | Retain singular package; narrow to canonical event-domain model. |
| `radroots_event_codec` | `radroots_event_codec` | Retain; remove live Nostr/upstream client responsibilities. |
| `radroots_event_index` | `radroots_storage::projection/index` | Merge; current checkpoint/manifest model is not an independent indexing engine. |
| `radroots_trade` | `radroots_trade` | Retain algorithms; remove authority, storage, SQL, transport, and duplicate TradeId. |
| `radroots_authority` | `radroots_identity + radroots_event::contract + radroots_signing` | Split account/public-key ownership, author-role contracts, and signing/authorization SPI. |
| `radroots_transport` | `radroots_transport` | Retain and redesign as extensible source/sink SPI. |
| `radroots_transport_nostr` | `radroots_transport_nostr` | Retain adapter; remove storage and sync orchestration. |
| `radroots_transport_reticulum` | `private preview` | Withhold until a real adapter passes the transport conformance suite. |
| `radroots_nostr` | `radroots_nostr` | Retain protocol conversion; remove live relay client and broad upstream aliases. |
| `radroots_nostr_connect` | `radroots_nostr_connect` | Retain as independent bidirectional NIP-46 protocol boundary. |
| `radroots_nostr_accounts` | `radroots_identity + radroots_secrets + radroots_storage + radroots_sdk` | Split mixed account, vault, persistence, and manager responsibilities. |
| `radroots_nostr_signer` | `radroots_signing + radroots_nostr_connect + Myc-private state` | Do not publish current service-state package. |
| `radroots_nostr_runtime` | `radroots_transport_nostr + radroots_sync` | Merge live relay runtime into adapter/orchestration layers. |
| `radroots_nostrdb` | `private; possible future radroots_storage_nostrdb` | Withhold until the storage SPI and external consumers justify a backend package. |
| `radroots_event_store` | `radroots_storage + radroots_storage_sqlite` | Split backend-neutral contracts from SQLite implementation. |
| `radroots_outbox` | `radroots_storage + radroots_storage_sqlite` | Merge as one persistence capability with atomic operation commits. |
| `radroots_runtime_store` | `radroots_storage or host-private state` | Retire broad name and classify each table by owner. |
| `radroots_sql_core` | `radroots_storage_sqlite private internals` | Remove raw SQL/JSON executor from public API. |
| `radroots_secret_vault` | `radroots_secrets` | Merge provider/wrapping SPI. |
| `radroots_protected_store` | `radroots_secrets` | Merge encrypted-envelope semantics. |
| `radroots_geocoder` | `radroots_geonames` | Rename to the actual concrete provider. |
| `radroots_runtime` | `radroots_sync + radroots_storage + host-private tooling` | Dismantle mixed config/signals/logging/queue/transport package. |
| `radroots_log` | `no replacement package` | Libraries emit tracing; hosts install subscribers. |
| `radroots_net` | `radroots_transport + radroots_sync + radroots_sdk` | Retire broad duplicated network/runtime package. |
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
name = "radroots_..."
version = "<repository-governed exact version>"
publish = ["crates-io"]
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
documentation = "https://docs.rs/radroots_..."
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

Every Rust crate in `radrootslabs/lib`, including private build, test, preview,
and support crates, is pinned to exactly `0.1.0-alpha`. Every same-repository
dependency requirement is pinned to exactly `=0.1.0-alpha`. This temporary
cohort is frozen until an explicit future authority changes it; neither normal
development nor release preparation may bump it implicitly.

The two `radrootslabs/sdk` public packages remain at `0.1.0` under their
independent repository authority. Cross-repository dependencies therefore use
the exact version assigned to the package's owning repository rather than a
single monorepo-wide version. This document's “V1” is the architecture
specification version, not a claim that Rust APIs are already 1.0-stable.

Cargo package versions are independent from versioned wire, conformance,
database, and operation contracts. Changing the library crate cohort MUST NOT
rewrite authenticated historical protocol artifacts or derive Cargo SemVer
from a protocol contract version.

### 22.2 SemVer groups

- `radroots` and `radroots_sdk` release in lockstep and `radroots` uses `=X.Y.Z` for the SDK.
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
radroots_core
radroots_identity
radroots_blossom
radroots_protocol
radroots_secrets
radroots_geonames
radroots_event
radroots_event_codec
radroots_trade
radroots_signing
radroots_transport
radroots_nostr
radroots_nostr_connect
radroots_storage
radroots_storage_sqlite
radroots_transport_nostr
radroots_sync
radroots_sdk
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

### 24.5 Coverage policy

During the active heavy-development refactor, every required workspace crate
owned by `radrootslabs/lib` MUST maintain at least 90% executable-line,
function, region, and branch coverage. Branch measurement remains required
except for a machine-recorded temporary exception where the coverage tool
emits no branch records. Crate-specific numeric thresholds below 90% are
forbidden. The
machine-readable authority for the active gate is `contracts/coverage.toml` in
`radrootslabs/lib`.

This amendment does not alter the independently governed `radrootslabs/sdk`
coverage contract. The 90% baseline is temporary development policy, not
permission to remove meaningful tests or weaken package-specific conformance
requirements. The 100% threshold is deferred until this refactor stabilizes
and may be reinstated only through an explicit future contract and
specification update.

### 24.6 Package-realistic release validation

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

### Collapse lower crates into `radroots_sdk`

Rejected because domain, protocol, SPI, backend, and adapter packages have independent consumers and semver responsibilities.

### One `radroots_runtime` package

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
4. Create final snake-case package manifests with `publish = false` during migration.
5. Refactor identity/public-key ownership and remove all secret material.
6. Split authority among identity, event contracts, and signing.
7. Remove the duplicate TradeId and make trade algorithm-only.
8. Create `radroots_protocol`.
9. Create `radroots_secrets`.
10. Create `radroots_storage` and `radroots_storage_sqlite`; migrate event/outbox/journal/private storage.
11. Remove Studio state from SDK storage.
12. Redesign `radroots_transport`; separate source/sink.
13. Narrow `radroots_nostr` and `radroots_transport_nostr`.
14. Refactor and retain `radroots_nostr_connect`.
15. Create `radroots_sync`.
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
