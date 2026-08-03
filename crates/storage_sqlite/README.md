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
