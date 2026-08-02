# radroots_storage_sqlite

SQLite storage backend for Radroots.

The backend owns separate `runtime.sqlite` and `private.sqlite` files. Writable
opens hold a process advisory lock, use WAL with bounded busy handling, and
apply only the governed forward migrations. Fresh stores require a
host-supplied `SourceGeneration` and creation timestamp; the crate never reads
hidden entropy or a wall clock.

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
