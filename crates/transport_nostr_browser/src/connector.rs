use radroots_transport::host::websocket::{Admission, Connector, Error, IoFuture, Socket};
use std::{net::IpAddr, num::NonZeroUsize, sync::Arc, time::Duration};

/// Inert provider whose DOM socket and callbacks stay on their origin thread.
#[derive(Clone, Default)]
pub struct BrowserConnector {
    #[cfg(any(target_arch = "wasm32", test))]
    scope: Arc<crate::scope::Scope>,
}

impl std::fmt::Debug for BrowserConnector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BrowserConnector([owned])")
    }
}

impl BrowserConnector {
    /// Permanently refuses new socket owners and waits for local callback
    /// disposal on every original socket in this scope. Does not join upstream
    /// SDK tasks or prove a remote close handshake. Resume after cancellation.
    /// An unpolled future has no effect. Clones share this same close boundary.
    pub fn close(&self) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            #[cfg(any(target_arch = "wasm32", test))]
            {
                self.scope.close().await
            }
            #[cfg(not(any(target_arch = "wasm32", test)))]
            {
                Ok(())
            }
        })
    }
}

impl Connector for BrowserConnector {
    fn supports_ping(&self) -> bool {
        false
    }

    fn connect<'a>(
        &'a self,
        endpoint: &'a str,
        timeout: Duration,
        max_message_bytes: NonZeroUsize,
        admission: Arc<dyn Admission>,
    ) -> IoFuture<'a, Result<Arc<dyn Socket>, Error>> {
        Box::pin(async move {
            validate_endpoint(endpoint, admission.as_ref())?;
            if timeout.is_zero() {
                return Err(Error::Timeout);
            }
            #[cfg(target_arch = "wasm32")]
            {
                crate::browser::connect(
                    endpoint,
                    timeout,
                    max_message_bytes,
                    admission,
                    &self.scope,
                )
                .await
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = max_message_bytes;
                Err(Error::Unsupported)
            }
        })
    }
}

fn validate_endpoint(endpoint: &str, admission: &dyn Admission) -> Result<(), Error> {
    let url = url::Url::parse(endpoint).map_err(|_| Error::Policy)?;
    if !matches!(url.scheme(), "ws" | "wss")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Policy);
    }
    let host = url
        .host_str()
        .ok_or(Error::Policy)?
        .trim_matches(['[', ']']);
    if let Ok(address) = host.parse::<IpAddr>()
        && !admission.permits_address(address)
    {
        return Err(Error::Policy);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::AdmissionCounter as Grant;
    #[test]
    fn endpoint_validation_is_explicit_and_never_resolves_dns() {
        for endpoint in [
            "invalid",
            "https://example.org",
            "ws://a:b@example.org",
            "wss://example.org/#fragment",
        ] {
            assert_eq!(
                validate_endpoint(endpoint, Grant::new(true).as_ref()),
                Err(Error::Policy)
            );
        }
        for endpoint in ["ws://127.0.0.1", "wss://[::1]"] {
            assert_eq!(
                validate_endpoint(endpoint, Grant::new(false).as_ref()),
                Err(Error::Policy)
            );
            assert_eq!(
                validate_endpoint(endpoint, Grant::new(true).as_ref()),
                Ok(())
            );
        }
        assert_eq!(
            validate_endpoint("wss://relay.example/path?q=1", Grant::new(false).as_ref()),
            Ok(())
        );
    }
    #[test]
    fn non_browser_connection_refuses_and_zero_deadline_does_not_start_io() {
        let connector = BrowserConnector::default();
        assert!(!connector.supports_ping());
        for (endpoint, duration, error) in [
            ("ws://127.0.0.1", Duration::from_secs(1), Error::Unsupported),
            ("wss://example.org", Duration::ZERO, Error::Timeout),
            ("invalid", Duration::from_secs(1), Error::Policy),
        ] {
            let result = futures::executor::block_on(connector.connect(
                endpoint,
                duration,
                NonZeroUsize::new(1).unwrap(),
                Grant::new(true),
            ));
            assert!(matches!(result, Err(found) if found == error));
        }
    }

    #[test]
    fn factory_constructs_the_shared_session_without_host_effects() {
        use radroots_transport_nostr::{
            Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
        };
        let endpoint = RelayEndpoint::new(
            "wss://relay.example",
            RelayUrlPolicy::Public,
            RelayAccess::ReadWrite,
        )
        .unwrap();
        let profile = RelayProfile::explicit(RelayProfileKind::Public, [endpoint]).unwrap();
        let _transport = crate::create_transport(Config::from_profile(profile));
    }

    #[test]
    fn clones_share_disposal_and_diagnostics_never_expose_buffered_peer_data() {
        use crate::state::{Shared, Status};
        let connector = BrowserConnector::default();
        let socket = Arc::new(Shared::new(Grant::new(true), 64));
        socket.lock().unwrap().status = Status::Open;
        socket
            .push(radroots_transport::host::websocket::Frame::Text(
                "private buffered peer content".into(),
            ))
            .unwrap();
        connector.scope.register(&socket).unwrap();
        assert!(!format!("{connector:?}").contains("private buffered peer content"));
        let clone = connector.clone();
        socket.finish();
        futures::executor::block_on(clone.close()).unwrap();
        assert_eq!(connector.scope.register(&socket), Err(Error::Closed));
    }
}
