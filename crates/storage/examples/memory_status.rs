use futures_executor::block_on;
use radroots_storage::{
    BackupSource,
    event::SourceGeneration,
    memory::MemoryStorage,
    status::{ShutdownState, StorageBackend},
};

fn main() -> Result<(), radroots_storage::Error> {
    let generation = SourceGeneration::new([1; 32])?;
    let storage = MemoryStorage::new(generation);
    let status = block_on(BackupSource::status(&storage))?;

    assert_eq!(status.backend(), StorageBackend::Memory);
    assert_eq!(status.shutdown(), ShutdownState::Open);
    Ok(())
}
