# radroots_storage

`radroots_storage` defines the backend-neutral persistence contracts used by
Radroots hosts. It owns canonical event persistence, durable operation journal
state, outbox and delivery evidence, projection coordination, protected-record
metadata, reliability operations, and high-level atomic workflow commits.

Projection document inventory selects one projection and explicitly selects
one generation or all generations. Pages contain at most 256 records and
16 MiB of opaque values, with independently scoped continuations and explicit
corrupt-record locators. This is a live scan, not a frozen snapshot. Callers
must fence their own mutations before treating a traversal as complete.
Storage neither interprets document payloads nor grants deletion authority.

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

`BackupSource::settle_backup_writes` waits for earlier owner writes, including
work whose caller was cancelled. The host excludes new writes before settling
and retains that exclusion through related inventory and capture. Settling does
not create a snapshot or an ongoing reservation.

`BackupSource::capture_backup`, `verify_backup`, and `finalize_backup` invoke
actual owner operations. Their default implementation returns typed
`BackupCapabilityError::Unsupported`; reliability metadata transitions cannot
substitute for a snapshot. The boundary returns manifests and completion, never
paths or database handles. Hosts coordinate application state and referenced
files separately; per-member snapshots do not imply a global transaction.

Actual backup capture can retain incomplete staging after interruption. It
does not replace live data or report that staging as finalized. The host must
retain the original plan and manifest and reconcile an ambiguous result before
retrying; cancellation never authorizes deleting retained evidence.

## Cancellation and commit points

Dropping a returned future requests cancellation. Read operations may stop
without side effects. Transactional record operations preserve their local
durable commit point:

- before the commit point, cancellation or failure leaves no partial state;
- after a successful commit, cancellation cannot claim rollback;
- replaying the same identity and canonical input returns the original result;
- reusing an identity with different input fails as a conflict;
- atomic workflow commits publish every requested mutation or none of them;
- rollback failure never replaces the primary operation failure.

`JournalState::Committed` is the durable operation-journal boundary.
`AtomicStorage::commit` is the aggregate local workflow boundary. Network
publication is outside this crate and is not implied by either state.

`AuthoredAtomicCommand::RecordSigned` retains an already-created, cryptographically
verified signature after its original signing lease expires or is superseded.
The backend retrieves and checks its immutable signing claim receipt, including
the full claim, operation, artifact and exact retained plan. This command performs
no signing and supplies no permission to schedule another phase. Active
`ApplySigned` and work claims retain their strict fences.

The first exact signed bytes are immutable. Repeated identical evidence is
idempotent; different raw bytes conflict. Cancelled or terminally failed signing
may retain valid bytes while preserving its stop and failure state, with no
admission or delivery claim. Receipt replay returns historical state, so hosts
must query current durable status before deciding what work may follow.

## Events, journal, outbox, and projections

Event storage preserves the exact signed event, verification/admission stage,
source generation, monotonically increasing position, and every unique
transport provenance observation. Queries are bounded and generation-aware;
backends fail closed on corrupt rows or source changes.

Current replacement heads are selected from verified and visible admissions
before interpreting application payloads. A verified-only winner supersedes an
older visible record without itself becoming visible. Raw records cannot select
heads, and only visible contract-valid author-authorized deletion requests
suppress events. Deleted winners never revive predecessors. Consumers use the
shared visibility snapshot digest to detect admission changes even when the raw
event count remains unchanged.

The journal records a command lifecycle under a validated idempotency key and
optimistic revision. The outbox persists explicit multi-target delivery plans,
leases, attempts, normalized receipts, partial success, and satisfaction
evidence without owning a transport adapter. Projection storage owns only
checkpoints, invalidation/rebuild state, and event-index manifests; reducer
algorithms and projected domain rows stay with their domain owners.

Idempotency-key construction validates borrowed input before allocating its
bounded owned representation, so rejected oversized input cannot force a
second attacker-sized allocation at this public boundary.

## Draft queries and atomic submission

`AuthoredDraftStore::query_authored_drafts` returns bounded current-head pages
under an independently selected author, payload schema and optional immutable
scope. Continue using the returned scoped cursor. Corrupt rows have individual
repair locators; applications retain their evidence and continue other work.
Pages scan stable draft IDs, so a later sweep must revisit new IDs inserted
behind the cursor. A page holds at most 256 records and 4 MiB of decoded payload.

`AuthoredAtomicCommand::PrepareFromDraft` joins a captured source revision,
a distinct initial intent and the existing authored preparation in one commit.
The application puts the complete frozen semantic request in the intent payload
and owns strict payload validation. Reserve a stable command ID before effects.
The author and command identify the receipt; context, source, intent and every
preparation field are compared for exact replay. Replay precedes fresh source
CAS and survives later editing. The source remains editable. The stored
submission receipt retains the immutable source-to-intent association, and the
ordinary Prepare receipt allows existing signing orchestration to resume.

No signing or transport effect occurs in this storage transaction. Public
backends must implement the new query method and submission command explicitly.

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

`BackupSource::stage_restore` and `finalize_restore` invoke actual owner
operations. Unsupported backends return `RestoreCapabilityError::Unsupported`;
reliability metadata cannot substitute for restored storage. Staging retains
live state and refuses existing staging. Finalization verifies, closes the
owner and installs through its recovery protocol. Hosts must reopen explicitly
and reconcile historical operations before delivery. Identity, related media
and durable application delivery guards remain host responsibilities.

Canceling SQLite finalization retains writer authority. A subsequent explicit
close drains both pools before releasing that authority, permitting guarded
reopen. Cancellation is not evidence of either successful restore or rollback.

Status and integrity inspection are passive. `close` is explicit and
idempotent; once closed, an implementation rejects ordinary operations.
Backend-specific durability fields are discriminated by `StorageBackend`:
memory does not pretend to use WAL or a process writer lock, while writable
SQLite status requires its governed lock, WAL, and busy-timeout contract.

## Serialization

`AuthoredDraftQuery::new` selects an exact author, schema and optional scope;
an absent scope selects only unscoped drafts. `AuthoredDraftQuery::for_author`
explicitly traverses that author's schema across all scopes using the same
bounded pages. Its version-2 continuation includes an explicit selection marker
and cannot be used for an exact-scope query or another author/schema. Existing
version-1 continuations keep their original bytes and meaning. Both traversals
are live views: callers must revisit earlier IDs when new work can be inserted.

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

## Paired authored revisions

`AuthoredDraftStore::append_authored_draft_pair` installs exactly two distinct
opaque draft revisions for one author in one atomic commit. Both expected heads
and existing per-draft bounds apply. A partial existing pair or either conflict
leaves both heads unchanged. Unsupported backends return `BackendUnavailable`;
there is no sequential-write fallback.

Exact replay returns both historical snapshots even after their heads advance.
It proves the requested snapshots exist, not current ownership or permission to
sign or deliver. Callers retain application policy and must recheck current
heads before effects. Losing the result after commit cannot establish rollback.

## Capacity failures

`Error::SpaceInsufficient` reports exhausted storage capacity without exposing
backend details. It does not prove rollback or absence of earlier effects.
Retain pending requests, original operation identities and ambiguous receipts;
reconcile existing state before retrying. Capacity diagnosis grants no eviction
or automatic retry authority. Backend implementations that cannot distinguish
capacity failures may continue returning `BackendUnavailable`.

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
