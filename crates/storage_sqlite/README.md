# radroots_storage_sqlite

SQLite storage backend for Radroots.

The backend owns separate `runtime.sqlite` and `private.sqlite` files. Writable
opens hold a process advisory lock, use WAL with bounded busy handling, and
apply only the governed forward migrations. Fresh stores require a
host-supplied `SourceGeneration` and creation timestamp; the crate never reads
hidden entropy or a wall clock.

Backend status and the last integrity result are passive. Hosts invoke
`check_integrity` explicitly with their own positive timestamp when they want
full SQLite and foreign-key validation across both owned files. `close` drains
both pools and releases writable authority explicitly and idempotently.

Backup capture requires an explicit existing host-owned root configured with
`OpenOptions::with_backup_root`. V1 capture creates a deterministic staging
bundle, uses SQLite's online snapshot mechanism for each policy-selected
member, synchronizes captured files and directories, and returns exact lengths
and SHA-256 digests in the backend-neutral manifest. Protected storage is
included only when the plan requests it explicitly.

Restore first copies a finalized bundle into create-new files adjacent to the
live databases and verifies their exact digests, schema catalogs, SQLite
integrity, and foreign keys without changing live state. Finalization closes
the owned pools while retaining writer authority, persists a versioned recovery
marker, atomically replaces each policy-selected member, and requires callers
to reopen the closed backend. Writable open completes an interrupted marked
replacement before opening connections; read-only open fails closed until that
recovery is complete.

Legacy migration starts only from an explicit, forward-only import plan on an
open writable backend. Before any target mutation, the backend captures every
caller-identified predecessor database through SQLite's WAL-consistent online
snapshot operation into a create-new staging directory, verifies SQLite and
foreign-key integrity, records exact source provenance, lengths, and SHA-256
digests in a mode-`0600` manifest, synchronizes the evidence, and atomically
finalizes the immutable bundle. Import identities and timestamps are supplied
by the host; collisions, owned-database aliases, and unsupported paths fail
closed.

Classification revalidates the finalized manifest, member inventory, hashes,
SQLite integrity, and foreign keys before inspecting any schema. Event-store
versions 1 through 4 require the exact governed catalog and, when present, the
exact contiguous migration ledger; outbox, private, and Studio predecessors
require their exact catalog and application version. Unknown objects, mixed
source families, checksum drift, and newer schemas fail closed. Studio records
are explicitly classified for host handoff and are never imported into SDK
storage.

Beginning an import writes only governed recovery metadata. Runtime schema v6
binds one retained import journal to the target generation, finalized manifest,
and ordered classification digest, plus one exact member row per predecessor
source. SQLite guards immutable identity, one-shot conflicts, legal monotonic
state transitions, timestamps, staged counts, and retained audit history.
Repeated begin calls resume the exact journal; conflicting attempts fail
closed.

Runtime schema v7 adds isolated, append-only event-import staging. Each bounded
page revalidates the immutable evidence, decodes and identifier-checks the
legacy signed events, preserves their exact JSON and untrusted predecessor
admission evidence, and atomically advances an eight-byte legacy sequence
cursor with its durable row count. Restart resumes after that exact cursor and
a completed retry is a no-op. Staging never mutates the live canonical event
table or upgrades predecessor verification claims.

Runtime schema v8 stages the predecessor outbox as an ordered five-table
graph. A table-discriminated cursor advances through operations, events,
delivery plans, targets, and attempts; every child is accepted only after its
exact parent, and attempts must bind a target from the same plan. Governed
column-order JSON-array records preserve nullable and scalar predecessor data
without writing the live operation journal, outbox, or delivery evidence.

Private schema v2 stages secret-bearing predecessor records only inside
`private.sqlite`. Runtime and private commits use an explicit replay protocol:
enter runtime staging, idempotently commit and byte-verify one private page,
then compare-and-swap the runtime table cursor and count. A process loss after
the private commit therefore replays the exact page from the old runtime cursor
without duplicating counts or exposing secret-bearing staging in
`runtime.sqlite`.

Classified Studio state is never imported into either owned database. The
backend instead returns an immutable evidence descriptor bound to the import,
target generation, manifest, source digest, schema catalog, and byte length.
Only an exact host receipt carrying that handoff identity and a non-zero opaque
host-store commitment advances the Studio journal member to ready; exact retry
is idempotent and conflicting acknowledgement fails closed.

```rust,no_run
use radroots_storage::event::SourceGeneration;
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths, SqliteStorage};

# async fn open(directory: &std::path::Path) -> Result<(), radroots_storage_sqlite::Error> {
let paths = Paths::from_directory(directory)?;
let generation = SourceGeneration::new([7; 32]).expect("non-zero generation");
let storage = SqliteStorage::open(
    OpenOptions::new(paths, OpenMode::Create)
        .with_source_generation(generation, 1_700_000_000_000)?,
).await?;
# drop(storage);
# Ok(())
# }
```
