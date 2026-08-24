//! Context-bound status-v1 access over the shared Unix admin client.

use core::fmt;

use radroots_runtime_paths::{
    InstanceId, RuntimeContext, ServiceId, default_service_instance_artifacts,
};
use radroots_service_host::{
    AdminClient, AdminClientTarget, AdminTransportLimits, SERVICE_STATUS_CONTRACT_VERSION,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::RadrootsRuntimeManagerError;

const STATUS_V1_TARGET: &str = "/v1/status";

/// Identity projection required from a service-owned status-v1 response.
///
/// Myc and RHI retain ownership of their typed status details. This trait lets
/// the manager validate only the shared contract and selected runtime identity
/// without accepting an untyped JSON payload.
pub trait ManagedServiceStatusV1: Serialize + DeserializeOwned {
    fn contract_version(&self) -> u32;
    fn service_id(&self) -> &ServiceId;
    fn instance_id(&self) -> &InstanceId;
}

/// Bounded status-v1 client sealed to one [`RuntimeContext`] admin socket.
pub struct ManagedRuntimeStatusClient {
    expected_service: ServiceId,
    expected_instance: InstanceId,
    client: AdminClient,
    target: AdminClientTarget,
}

impl ManagedRuntimeStatusClient {
    pub(crate) fn for_context(
        context: &RuntimeContext,
        limits: AdminTransportLimits,
    ) -> Result<Self, RadrootsRuntimeManagerError> {
        let artifacts = default_service_instance_artifacts(context.paths());
        let client = AdminClient::new(artifacts.admin_socket(), limits)
            .map_err(|_| RadrootsRuntimeManagerError::AdminClient)?;
        let target = AdminClientTarget::new(STATUS_V1_TARGET)
            .map_err(|_| RadrootsRuntimeManagerError::AdminClient)?;
        Ok(Self {
            expected_service: context.service().clone(),
            expected_instance: context.instance().clone(),
            client,
            target,
        })
    }

    /// Requests and identity-validates the service-owned typed status model.
    pub async fn get<S>(&self) -> Result<S, RadrootsRuntimeManagerError>
    where
        S: ManagedServiceStatusV1,
    {
        let status = self
            .client
            .get::<S>(&self.target)
            .await
            .map_err(|_| RadrootsRuntimeManagerError::AdminRequest)?
            .into_result();
        if status.contract_version() != SERVICE_STATUS_CONTRACT_VERSION
            || status.service_id() != &self.expected_service
            || status.instance_id() != &self.expected_instance
        {
            return Err(RadrootsRuntimeManagerError::StatusContractMismatch);
        }
        Ok(status)
    }
}

impl fmt::Debug for ManagedRuntimeStatusClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedRuntimeStatusClient")
            .field("expected_service", &self.expected_service)
            .field("expected_instance", &self.expected_instance)
            .field("socket_path", &"[redacted]")
            .field("target", &STATUS_V1_TARGET)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    use radroots_service_host::AdminTransportLimits;
    use serde::{Deserialize, Serialize};
    use tempfile::Builder;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixListener,
    };

    use super::{ManagedRuntimeStatusClient, ManagedServiceStatusV1};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct TestStatus {
        contract_version: u32,
        service: ServiceId,
        instance: InstanceId,
        phase: String,
    }

    impl ManagedServiceStatusV1 for TestStatus {
        fn contract_version(&self) -> u32 {
            self.contract_version
        }

        fn service_id(&self) -> &ServiceId {
            &self.service
        }

        fn instance_id(&self) -> &InstanceId {
            &self.instance
        }
    }

    fn context(base: PathBuf) -> RuntimeContext {
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(base),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("context")
    }

    async fn serve_status_once(socket: PathBuf, result: serde_json::Value) {
        if socket.exists() {
            std::fs::remove_file(&socket).expect("remove prior socket");
        }
        let listener = UnixListener::bind(socket).expect("bind status socket");
        let (mut stream, _) = listener.accept().await.expect("accept status request");
        let mut request = [0_u8; 4_096];
        let read = stream.read(&mut request).await.expect("read request");
        let request = std::str::from_utf8(&request[..read]).expect("UTF-8 request");
        assert!(request.starts_with("GET /v1/status HTTP/1.1\r\n"));

        let body = serde_json::to_vec(&serde_json::json!({
            "contract_version": 1,
            "ok": true,
            "correlation_id": "status-test-01",
            "result": result,
        }))
        .expect("response body");
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(head.as_bytes())
            .await
            .expect("write response head");
        stream.write_all(&body).await.expect("write response body");
        stream.shutdown().await.expect("shutdown response");
    }

    #[test]
    fn construction_is_context_bound_and_debug_redacts_the_socket() {
        let client = ManagedRuntimeStatusClient::for_context(
            &context(PathBuf::from("/sensitive/project-root")),
            AdminTransportLimits::DEFAULT,
        )
        .expect("client");
        let debug = format!("{client:?}");
        assert!(debug.contains("/v1/status"));
        assert!(!debug.contains("sensitive"));
        assert!(!debug.contains("admin.sock"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn bounded_unix_status_round_trip_validates_the_complete_common_identity() {
        let temp = Builder::new()
            .prefix("rrm")
            .tempdir_in("/tmp")
            .expect("short temp root");
        let context = context(temp.path().to_path_buf());
        let socket = radroots_runtime_paths::default_service_instance_artifacts(context.paths())
            .admin_socket()
            .to_path_buf();
        std::fs::create_dir_all(socket.parent().expect("socket parent"))
            .expect("create socket parent");
        let client =
            ManagedRuntimeStatusClient::for_context(&context, AdminTransportLimits::DEFAULT)
                .expect("client");

        let success_server = tokio::spawn(serve_status_once(
            socket.clone(),
            serde_json::json!({
                "contract_version": 1,
                "service": "myc",
                "instance": "primary",
                "phase": "ready",
            }),
        ));
        tokio::task::yield_now().await;
        let status = client.get::<TestStatus>().await.expect("status");
        assert_eq!(status.phase, "ready");
        success_server.await.expect("success server");

        for (field, replacement) in [
            ("contract_version", serde_json::json!(2)),
            ("service", serde_json::json!("rhi")),
            ("instance", serde_json::json!("secondary")),
        ] {
            let mut result = serde_json::json!({
                "contract_version": 1,
                "service": "myc",
                "instance": "primary",
                "phase": "ready",
            });
            result[field] = replacement;
            let server = tokio::spawn(serve_status_once(socket.clone(), result));
            tokio::task::yield_now().await;
            assert_eq!(
                client.get::<TestStatus>().await,
                Err(crate::RadrootsRuntimeManagerError::StatusContractMismatch)
            );
            server.await.expect("mismatch server");
        }
    }
}
