//! Redacted client diagnostics composed from canonical capability and storage status.

/// One passive diagnostics snapshot containing only lower-owned status types.
#[derive(Clone, Debug)]
pub struct Report {
    capabilities: crate::capability::CapabilityReport,
    storage: radroots_storage::StorageStatus,
}

impl Report {
    /// Returns the side-effect-free SDK capability report.
    #[must_use]
    pub const fn capabilities(&self) -> &crate::capability::CapabilityReport {
        &self.capabilities
    }

    /// Returns the canonical backend status without implementation details.
    #[must_use]
    pub const fn storage(&self) -> radroots_storage::StorageStatus {
        self.storage
    }
}

/// Captures passive, secret-safe diagnostics without filesystem paths, SQL,
/// connection handles, private artifacts, or recovery side effects.
pub async fn inspect(client: &crate::Client) -> crate::Result<Report> {
    Ok(Report {
        capabilities: client.capabilities(),
        storage: client.storage_status().await?,
    })
}

#[cfg(all(test, feature = "memory"))]
mod tests {
    use std::sync::Arc;

    use radroots_storage::{
        event::SourceGeneration, memory::MemoryStorage, status::StorageBackend,
    };

    use crate::ClientBuilder;

    #[tokio::test]
    async fn report_is_passive_native_and_redacted() {
        let client = ClientBuilder::new()
            .storage(Arc::new(MemoryStorage::new(
                SourceGeneration::new([9; 32]).expect("generation"),
            )))
            .build()
            .expect("client");
        let report = super::inspect(&client).await.expect("report");

        assert_eq!(report.storage().backend(), StorageBackend::Memory);
        assert!(report.capabilities().iter().all(|status| {
            !status.id().as_str().contains('/') && !status.id().as_str().contains('\\')
        }));
        assert!(!format!("{report:?}").contains("sqlite"));
    }
}
