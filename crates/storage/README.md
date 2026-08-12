# radroots_storage

`radroots_storage` defines the backend-neutral persistence contracts used by
Radroots hosts. It owns canonical event persistence, durable operation journal
state, outbox and delivery evidence, projection coordination, protected-record
metadata, reliability operations, and high-level atomic workflow commits.

The package does not expose SQL, filesystem handles, database pools, raw
transactions, encryption keys, transport clients, schedulers, or application
state. Concrete backends implement these contracts; `radroots_storage_sqlite`
is the native durable backend and the opt-in [`memory`] module is the bounded
deterministic reference implementation.

The authoritative package charter is the
[`radroots_storage` section of the Release V1 specification](../../contracts/crates/release_v1/radroots_crates_release_v1.toml).

## Typical flow

1. A host selects a concrete backend and owns its lifecycle.
2. The host records operation preparation in [`Journal`] with a caller-owned
   idempotency key.
3. Verified events enter [`EventStore`] with explicit source provenance.
4. [`Outbox`] stores transport-neutral delivery intent and evidence.
5. [`ProjectionStore`] records checkpoints and rebuild coordination while
   domain reducers remain outside storage.
6. Advanced hosts use [`atomic::AtomicStorage`] to commit related journal,
   event, outbox, and projection transitions as one local durable operation.
7. [`BackupSource`] exposes explicit backup, staged restore, integrity, status,
   and close operations without leaking backend handles.

[`Journal`]: crate::Journal
[`EventStore`]: crate::EventStore
[`Outbox`]: crate::Outbox
[`ProjectionStore`]: crate::ProjectionStore
[`BackupSource`]: crate::BackupSource

```rust
use futures_executor::block_on;
use radroots_storage::{
    BackupSource,
    event::SourceGeneration,
    memory::MemoryStorage,
    status::{ShutdownState, StorageBackend},
};

let generation = SourceGeneration::new([1; 32])?;
let storage = MemoryStorage::new(generation);
let status = block_on(BackupSource::status(&storage))?;

assert_eq!(status.backend(), StorageBackend::Memory);
assert_eq!(status.shutdown(), ShutdownState::Open);
# Ok::<(), radroots_storage::Error>(())
```

The same program is available as
[`examples/memory_status.rs`](examples/memory_status.rs).

## Public capability boundary

The crate root exposes the ordinary aggregate [`Storage`] capability plus
`EventStore`, `Journal`, `Outbox`, `ProjectionStore`, `BackupSource`,
`StorageStatus`, and `Error`. Advanced contracts remain in their owning
modules so ordinary consumers do not accidentally depend on workflow or
protected-record internals.

All SPIs are externally implementable, dyn-compatible `Send + Sync` traits.
Their methods return boxed `Future + Send` values, allowing the host to choose
the async executor. Implementations must not install an executor, spawn hidden
workers, read a clock, generate identities, or perform implicit retries.

## Cancellation and commit points

Dropping a returned future requests cancellation. Read operations may stop
without side effects. Mutating operations must document and preserve their
local durable commit point:

- before the commit point, cancellation or failure leaves no partial state;
- after a successful commit, cancellation cannot claim rollback;
- replaying the same identity and canonical input returns the original result;
- reusing an identity with different input fails as a conflict;
- atomic workflow commits publish every requested mutation or none of them;
- rollback failure never replaces the primary operation failure.

`JournalState::Committed` is the durable operation-journal boundary.
`AtomicStorage::commit` is the aggregate local workflow boundary. Network
publication is outside this crate and is not implied by either state.

## Events, journal, outbox, and projections

Event storage preserves the exact signed event, verification/admission stage,
source generation, monotonically increasing position, and every unique
transport provenance observation. Queries are bounded and generation-aware;
backends fail closed on corrupt rows or source changes.

The journal records a command lifecycle under a validated idempotency key and
optimistic revision. The outbox persists explicit multi-target delivery plans,
leases, attempts, normalized receipts, partial success, and satisfaction
evidence without owning a transport adapter. Projection storage owns only
checkpoints, invalidation/rebuild state, and event-index manifests; reducer
algorithms and projected domain rows stay with their domain owners.

## Protected metadata and security

`private_artifact` stores bounded metadata and opaque durable secret references.
It never accepts domain plaintext, ciphertext bytes, or an active secret
capability. Encryption, wrapping, and provider access belong to
`radroots_secrets` and the concrete backend.

Identifiers, paths, query sizes, revisions, timestamps, and result sets are
bounded and validated before backend work. Secret references and idempotency
keys have redacted diagnostics. Public errors are stable and do not expose SQL,
filesystem, key-provider, or transport implementation messages. The crate
forbids unsafe code.

## Backup, restore, integrity, and close

Backup and restore are explicit multi-stage operations. A backend captures a
versioned member plan, verifies exact member digests, and finalizes only after
all expected members are present. Restore uses isolated staging and cannot
replace live state before complete verification. Relative member paths reject
absolute paths, traversal, duplicates, and unsafe separators.

Status and integrity inspection are passive. `close` is explicit and
idempotent; once closed, an implementation rejects ordinary operations.
Backend-specific durability fields are discriminated by `StorageBackend`:
memory does not pretend to use WAL or a process writer lock, while writable
SQLite status requires its governed lock, WAL, and busy-timeout contract.

## Serialization

The optional `serde` feature serializes passive identities, requests, records,
receipts, status values, manifests, and coordination metadata. Deserialization
revalidates invariants rather than trusting encoded revisions, digests, paths,
cardinality, lifecycle transitions, or derived state.

Serialization is not a database schema or wire-protocol authority. Backend
schemas are private to their implementation, and cross-language runtime DTOs
remain owned by `radroots_protocol`.

## Features

| Feature | Default | Contract |
| --- | --- | --- |
| `memory` | yes | Enables the deterministic bounded in-memory reference backend. It installs no task, clock, entropy source, filesystem, or global state. |
| `serde` | yes | Adds validated serialization to passive storage values. It does not serialize backend handles, active secret capabilities, or transactions. |

Features are additive. `--no-default-features` exposes the backend-neutral SPI
without an implementation. `memory` and `serde` are supported independently,
and `--all-features` enables both.

## Intended consumers

- `radroots_storage_sqlite` implements the contracts for native durable state.
- `radroots_sync` coordinates bounded ingest, projection, enqueue, and delivery.
- `radroots_sdk` composes storage with signing and transport implementations.
- Service and application hosts may implement or inject a backend while
  retaining ownership of paths, runtime, cancellation, clocks, and lifecycle.

Applications that only need ordinary Radroots operations should normally use
`radroots` or `radroots_sdk`. Implement this package directly when providing a
storage backend or advanced host composition.

## Copyright

Except as otherwise noted, all files in the `radroots_storage` distribution are

 Copyright (c) 2025 Tyson Lupul

For information on usage and redistribution, and for a DISCLAIMER OF ALL
WARRANTIES, see LICENSE included in the `radroots_storage` distribution.
