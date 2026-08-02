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
