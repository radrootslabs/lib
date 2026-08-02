#[cfg(feature = "memory")]
use radroots_storage::Storage;

#[cfg(feature = "memory")]
#[test]
fn memory_feature_provides_the_complete_storage_capability() {
    fn accepts(_: &dyn Storage) {}
    accepts(&radroots_storage::memory::MemoryStorage::default());
}

#[cfg(not(feature = "memory"))]
#[test]
fn storage_spi_builds_without_the_memory_backend() {
    fn accepts_spi(_: Option<&dyn radroots_storage::EventStore>) {}
    accepts_spi(None);
}

#[cfg(feature = "serde")]
#[test]
fn serde_feature_round_trips_backend_status_discriminants() {
    let encoded = serde_json::to_string(&radroots_storage::status::StorageBackend::Memory)
        .expect("serialize backend");
    assert_eq!(encoded, "\"memory\"");
    let decoded: radroots_storage::status::StorageBackend =
        serde_json::from_str(&encoded).expect("deserialize backend");
    assert_eq!(decoded, radroots_storage::status::StorageBackend::Memory);
}
