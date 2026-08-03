//! Private mobile FFI over the shared SDK engine.

#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::sync::Arc;

use radroots_sdk::{Client, ClientBuilder};

uniffi::setup_scaffolding!("radroots_sdk");

/// Generation-1 mobile DTOs.
pub mod v1 {
    use super::*;

    /// Stable capability maturity independent of runtime availability.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
    pub enum CapabilityMaturity {
        Stable,
        Preview,
        Experimental,
    }

    /// Current side-effect-free capability availability.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
    pub enum CapabilityAvailability {
        Available,
        Degraded,
        Unavailable,
        Unsupported,
    }

    /// Versioned, presentation-independent capability observation.
    #[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
    pub struct CapabilityStatus {
        pub id: String,
        pub compiled: bool,
        pub configured: bool,
        pub availability: CapabilityAvailability,
        pub maturity: CapabilityMaturity,
    }

    /// Secret-safe SDK failure at the language boundary.
    #[derive(Debug, thiserror::Error, uniffi::Error)]
    pub enum Error {
        #[error("SDK {code}: {message}")]
        Sdk {
            code: String,
            message: String,
            retryable: bool,
        },
    }

    impl From<radroots_sdk::Error> for Error {
        fn from(error: radroots_sdk::Error) -> Self {
            let descriptor = error.descriptor();
            Self::Sdk {
                code: descriptor.code().as_str().to_owned(),
                message: descriptor.message().to_owned(),
                retryable: descriptor.retryable(),
            }
        }
    }

    /// Thread-safe mobile handle delegating lifecycle to [`radroots_sdk::Client`].
    #[derive(uniffi::Object)]
    pub struct MobileClient {
        client: Client,
    }

    #[uniffi::export]
    impl MobileClient {
        /// Creates a deterministic, local-only memory client without I/O.
        #[uniffi::constructor]
        pub fn memory() -> Result<Arc<Self>, Error> {
            let client = ClientBuilder::memory_default().build()?;
            Ok(Arc::new(Self { client }))
        }

        /// Returns stable capability DTOs without probing host resources.
        pub fn capabilities(&self) -> Vec<CapabilityStatus> {
            self.client
                .capabilities()
                .iter()
                .copied()
                .map(CapabilityStatus::from)
                .collect()
        }

        /// Returns whether explicit SDK close completed across all clones.
        pub fn is_closed(&self) -> bool {
            self.client.is_closed()
        }

        /// Explicitly closes SDK-owned resources without installing a runtime.
        pub async fn close(&self) -> Result<(), Error> {
            self.client.close().await.map_err(Error::from)
        }
    }

    impl From<radroots_sdk::capability::CapabilityStatus> for CapabilityStatus {
        fn from(status: radroots_sdk::capability::CapabilityStatus) -> Self {
            Self {
                id: status.id().as_str().to_owned(),
                compiled: status.is_compiled(),
                configured: status.is_configured(),
                availability: match status.availability() {
                    radroots_sdk::capability::Availability::Available => {
                        CapabilityAvailability::Available
                    }
                    radroots_sdk::capability::Availability::Degraded => {
                        CapabilityAvailability::Degraded
                    }
                    radroots_sdk::capability::Availability::Unavailable => {
                        CapabilityAvailability::Unavailable
                    }
                    radroots_sdk::capability::Availability::Unsupported => {
                        CapabilityAvailability::Unsupported
                    }
                },
                maturity: match status.maturity() {
                    radroots_sdk::capability::Maturity::Stable => CapabilityMaturity::Stable,
                    radroots_sdk::capability::Maturity::Preview => CapabilityMaturity::Preview,
                    radroots_sdk::capability::Maturity::Experimental => {
                        CapabilityMaturity::Experimental
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::v1::{CapabilityAvailability, Error, MobileClient};

    fn assert_send_sync<T: Send + Sync>() {}

    #[tokio::test(flavor = "current_thread")]
    async fn memory_client_delegates_capabilities_and_lifecycle() {
        assert_send_sync::<MobileClient>();
        let client = MobileClient::memory().expect("memory client");
        let capabilities = client.capabilities();
        let storage = capabilities
            .iter()
            .find(|status| status.id == "storage.canonical")
            .expect("canonical storage");
        assert!(storage.compiled);
        assert!(storage.configured);
        assert_eq!(storage.availability, CapabilityAvailability::Available);
        assert!(!client.is_closed());

        client.close().await.expect("close");
        assert!(client.is_closed());
        client.close().await.expect("idempotent close");
    }

    #[test]
    fn sdk_error_mapping_is_versioned_and_secret_safe() {
        let native = radroots_sdk::ClientBuilder::new()
            .build()
            .expect_err("missing storage");
        let error = Error::from(native);
        let Error::Sdk {
            code,
            message,
            retryable,
        } = error;
        assert_eq!(code, "missing_storage");
        assert_eq!(message, "SDK storage capability is not configured");
        assert!(!retryable);
    }
}
