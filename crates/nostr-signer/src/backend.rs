use crate::capability::{
    RadrootsNostrLocalSignerAvailability, RadrootsNostrLocalSignerCapability,
    RadrootsNostrRemoteSessionSignerCapability, RadrootsNostrSignerCapability,
};
use crate::error::RadrootsNostrSignerError;
use crate::evaluation::{
    RadrootsNostrSignerConnectEvaluation, RadrootsNostrSignerRequestEvaluation,
    RadrootsNostrSignerSessionLookup,
};
use crate::manager::RadrootsNostrSignerManager;
use crate::model::{
    RadrootsNostrSignerAuthorizationOutcome, RadrootsNostrSignerConnectionDraft,
    RadrootsNostrSignerConnectionId, RadrootsNostrSignerConnectionRecord,
    RadrootsNostrSignerConnectionStatus, RadrootsNostrSignerPendingRequest,
    RadrootsNostrSignerPublishWorkflowRecord, RadrootsNostrSignerRequestAuditRecord,
    RadrootsNostrSignerRequestDecision, RadrootsNostrSignerWorkflowId,
};
use nostr::{Event, EventBuilder, PublicKey, RelayUrl, UnsignedEvent};
use radroots_identity::{RadrootsIdentity, RadrootsIdentityPublic};
use radroots_nostr_connect::prelude::{
    RadrootsNostrConnectMethod, RadrootsNostrConnectPermissions, RadrootsNostrConnectRequest,
    RadrootsNostrConnectRequestMessage,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrSignerBackendCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_signer: Option<RadrootsNostrLocalSignerCapability>,
    #[serde(default)]
    pub remote_sessions: Vec<RadrootsNostrRemoteSessionSignerCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrSignerSignOutput {
    pub signer: RadrootsNostrSignerCapability,
    pub event: Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "value")]
pub enum RadrootsNostrSignerPublishTransition {
    Begun(RadrootsNostrSignerPublishWorkflowRecord),
    MarkedPublished(RadrootsNostrSignerPublishWorkflowRecord),
    Finalized {
        workflow_id: RadrootsNostrSignerWorkflowId,
        connection: RadrootsNostrSignerConnectionRecord,
    },
    Cancelled(RadrootsNostrSignerPublishWorkflowRecord),
}

pub trait RadrootsNostrSignerBackend: Send + Sync {
    fn signer_identity(&self) -> Result<Option<RadrootsIdentityPublic>, RadrootsNostrSignerError>;

    fn set_signer_identity(
        &self,
        signer_identity: RadrootsIdentityPublic,
    ) -> Result<(), RadrootsNostrSignerError>;

    fn capabilities(
        &self,
    ) -> Result<RadrootsNostrSignerBackendCapabilities, RadrootsNostrSignerError>;

    fn list_connections(
        &self,
    ) -> Result<Vec<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError>;

    fn get_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<Option<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError>;

    fn list_publish_workflows(
        &self,
    ) -> Result<Vec<RadrootsNostrSignerPublishWorkflowRecord>, RadrootsNostrSignerError>;

    fn get_publish_workflow(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<Option<RadrootsNostrSignerPublishWorkflowRecord>, RadrootsNostrSignerError>;

    fn find_connections_by_client_public_key(
        &self,
        client_public_key: &PublicKey,
    ) -> Result<Vec<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError>;

    fn find_connection_by_connect_secret(
        &self,
        connect_secret: &str,
    ) -> Result<Option<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError>;

    fn lookup_session(
        &self,
        client_public_key: &PublicKey,
        connect_secret: Option<&str>,
    ) -> Result<RadrootsNostrSignerSessionLookup, RadrootsNostrSignerError>;

    fn evaluate_connect_request(
        &self,
        client_public_key: PublicKey,
        request: RadrootsNostrConnectRequest,
    ) -> Result<RadrootsNostrSignerConnectEvaluation, RadrootsNostrSignerError>;

    fn register_connection(
        &self,
        draft: RadrootsNostrSignerConnectionDraft,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn set_granted_permissions(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        granted_permissions: RadrootsNostrConnectPermissions,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn approve_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        granted_permissions: RadrootsNostrConnectPermissions,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn reject_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        reason: Option<String>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn revoke_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        reason: Option<String>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn update_relays(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        relays: Vec<RelayUrl>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn require_auth_challenge(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        auth_url: &str,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn set_pending_request(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        request_message: RadrootsNostrConnectRequestMessage,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn authorize_auth_challenge(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerAuthorizationOutcome, RadrootsNostrSignerError>;

    fn restore_pending_auth_challenge(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        pending_request: RadrootsNostrSignerPendingRequest,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn begin_connect_secret_publish_finalization(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError>;

    fn begin_auth_replay_publish_finalization(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError>;

    fn mark_publish_workflow_published(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError>;

    fn finalize_publish_workflow(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError>;

    fn cancel_publish_workflow(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError>;

    fn mark_authenticated(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn mark_connect_secret_consumed(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError>;

    fn evaluate_request(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        request_message: RadrootsNostrConnectRequestMessage,
    ) -> Result<RadrootsNostrSignerRequestEvaluation, RadrootsNostrSignerError>;

    fn evaluate_auth_replay_publish_workflow(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<RadrootsNostrSignerRequestEvaluation, RadrootsNostrSignerError>;

    fn record_request(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        request_id: &str,
        method: RadrootsNostrConnectMethod,
        decision: RadrootsNostrSignerRequestDecision,
        message: Option<String>,
    ) -> Result<RadrootsNostrSignerRequestAuditRecord, RadrootsNostrSignerError>;

    fn sign_unsigned_event(
        &self,
        unsigned_event: UnsignedEvent,
    ) -> Result<RadrootsNostrSignerSignOutput, RadrootsNostrSignerError>;

    fn sign_event_builder(
        &self,
        builder: EventBuilder,
    ) -> Result<RadrootsNostrSignerSignOutput, RadrootsNostrSignerError> {
        let signer_identity = self
            .signer_identity()?
            .ok_or(RadrootsNostrSignerError::MissingSignerIdentity)?;
        let public_key = parse_identity_public_key(&signer_identity)?;
        self.sign_unsigned_event(builder.build(public_key))
    }
}

#[derive(Clone)]
pub struct RadrootsNostrEmbeddedSignerBackend {
    manager: RadrootsNostrSignerManager,
    signer_identity: RadrootsIdentity,
}

impl RadrootsNostrSignerBackendCapabilities {
    pub fn new(
        local_signer: Option<RadrootsNostrLocalSignerCapability>,
        remote_sessions: Vec<RadrootsNostrRemoteSessionSignerCapability>,
    ) -> Self {
        Self {
            local_signer,
            remote_sessions,
        }
    }

    pub fn all_signers(&self) -> Vec<RadrootsNostrSignerCapability> {
        let mut signers = Vec::new();
        if let Some(local_signer) = self.local_signer.clone() {
            signers.push(RadrootsNostrSignerCapability::LocalAccount(local_signer));
        }
        signers.extend(
            self.remote_sessions
                .iter()
                .cloned()
                .map(RadrootsNostrSignerCapability::RemoteSession),
        );
        signers
    }
}

impl RadrootsNostrSignerSignOutput {
    pub fn new(signer: RadrootsNostrSignerCapability, event: Event) -> Self {
        Self { signer, event }
    }
}

impl RadrootsNostrSignerPublishTransition {
    pub fn begun(workflow: RadrootsNostrSignerPublishWorkflowRecord) -> Self {
        Self::Begun(workflow)
    }

    pub fn marked_published(workflow: RadrootsNostrSignerPublishWorkflowRecord) -> Self {
        Self::MarkedPublished(workflow)
    }

    pub fn finalized(
        workflow_id: RadrootsNostrSignerWorkflowId,
        connection: RadrootsNostrSignerConnectionRecord,
    ) -> Self {
        Self::Finalized {
            workflow_id,
            connection,
        }
    }

    pub fn cancelled(workflow: RadrootsNostrSignerPublishWorkflowRecord) -> Self {
        Self::Cancelled(workflow)
    }

    pub fn workflow(&self) -> Option<&RadrootsNostrSignerPublishWorkflowRecord> {
        match self {
            Self::Begun(workflow) | Self::MarkedPublished(workflow) | Self::Cancelled(workflow) => {
                Some(workflow)
            }
            Self::Finalized { .. } => None,
        }
    }

    pub fn finalized_connection(&self) -> Option<&RadrootsNostrSignerConnectionRecord> {
        match self {
            Self::Finalized { connection, .. } => Some(connection),
            _ => None,
        }
    }
}

impl RadrootsNostrEmbeddedSignerBackend {
    pub fn new(
        manager: RadrootsNostrSignerManager,
        signer_identity: RadrootsIdentity,
    ) -> Result<Self, RadrootsNostrSignerError> {
        let public_identity = signer_identity.to_public();
        if let Some(existing_identity) = manager.signer_identity()? {
            if !same_public_identity_key(&existing_identity, &public_identity) {
                return Err(RadrootsNostrSignerError::InvalidState(
                    "embedded signer identity does not match signer manager identity".into(),
                ));
            }
        } else {
            manager.set_signer_identity(public_identity)?;
        }

        Ok(Self {
            manager,
            signer_identity,
        })
    }

    pub fn new_in_memory(
        signer_identity: RadrootsIdentity,
    ) -> Result<Self, RadrootsNostrSignerError> {
        Self::new(RadrootsNostrSignerManager::new_in_memory(), signer_identity)
    }

    pub fn manager(&self) -> &RadrootsNostrSignerManager {
        &self.manager
    }

    pub fn local_identity(&self) -> &RadrootsIdentity {
        &self.signer_identity
    }

    fn local_signer_capability(&self) -> RadrootsNostrLocalSignerCapability {
        let public_identity = self.signer_identity.to_public();
        RadrootsNostrLocalSignerCapability::new(
            public_identity.id.clone(),
            public_identity,
            RadrootsNostrLocalSignerAvailability::SecretBacked,
        )
    }
}

impl RadrootsNostrSignerBackend for RadrootsNostrEmbeddedSignerBackend {
    fn signer_identity(&self) -> Result<Option<RadrootsIdentityPublic>, RadrootsNostrSignerError> {
        self.manager.signer_identity()
    }

    fn set_signer_identity(
        &self,
        signer_identity: RadrootsIdentityPublic,
    ) -> Result<(), RadrootsNostrSignerError> {
        self.manager.set_signer_identity(signer_identity)
    }

    fn capabilities(
        &self,
    ) -> Result<RadrootsNostrSignerBackendCapabilities, RadrootsNostrSignerError> {
        let remote_sessions = self
            .manager
            .list_connections()?
            .into_iter()
            .filter(|record| record.status == RadrootsNostrSignerConnectionStatus::Active)
            .map(|record| RadrootsNostrRemoteSessionSignerCapability::from(&record))
            .collect();
        Ok(RadrootsNostrSignerBackendCapabilities::new(
            Some(self.local_signer_capability()),
            remote_sessions,
        ))
    }

    fn list_connections(
        &self,
    ) -> Result<Vec<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError> {
        self.manager.list_connections()
    }

    fn get_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<Option<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError> {
        self.manager.get_connection(connection_id)
    }

    fn list_publish_workflows(
        &self,
    ) -> Result<Vec<RadrootsNostrSignerPublishWorkflowRecord>, RadrootsNostrSignerError> {
        self.manager.list_publish_workflows()
    }

    fn get_publish_workflow(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<Option<RadrootsNostrSignerPublishWorkflowRecord>, RadrootsNostrSignerError> {
        self.manager.get_publish_workflow(workflow_id)
    }

    fn find_connections_by_client_public_key(
        &self,
        client_public_key: &PublicKey,
    ) -> Result<Vec<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError> {
        self.manager
            .find_connections_by_client_public_key(client_public_key)
    }

    fn find_connection_by_connect_secret(
        &self,
        connect_secret: &str,
    ) -> Result<Option<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError> {
        self.manager
            .find_connection_by_connect_secret(connect_secret)
    }

    fn lookup_session(
        &self,
        client_public_key: &PublicKey,
        connect_secret: Option<&str>,
    ) -> Result<RadrootsNostrSignerSessionLookup, RadrootsNostrSignerError> {
        self.manager
            .lookup_session(client_public_key, connect_secret)
    }

    fn evaluate_connect_request(
        &self,
        client_public_key: PublicKey,
        request: RadrootsNostrConnectRequest,
    ) -> Result<RadrootsNostrSignerConnectEvaluation, RadrootsNostrSignerError> {
        self.manager
            .evaluate_connect_request(client_public_key, request)
    }

    fn register_connection(
        &self,
        draft: RadrootsNostrSignerConnectionDraft,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager.register_connection(draft)
    }

    fn set_granted_permissions(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        granted_permissions: RadrootsNostrConnectPermissions,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager
            .set_granted_permissions(connection_id, granted_permissions)
    }

    fn approve_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        granted_permissions: RadrootsNostrConnectPermissions,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager
            .approve_connection(connection_id, granted_permissions)
    }

    fn reject_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        reason: Option<String>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager.reject_connection(connection_id, reason)
    }

    fn revoke_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        reason: Option<String>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager.revoke_connection(connection_id, reason)
    }

    fn update_relays(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        relays: Vec<RelayUrl>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager.update_relays(connection_id, relays)
    }

    fn require_auth_challenge(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        auth_url: &str,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager.require_auth_challenge(connection_id, auth_url)
    }

    fn set_pending_request(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        request_message: RadrootsNostrConnectRequestMessage,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager
            .set_pending_request(connection_id, request_message)
    }

    fn authorize_auth_challenge(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerAuthorizationOutcome, RadrootsNostrSignerError> {
        self.manager.authorize_auth_challenge(connection_id)
    }

    fn restore_pending_auth_challenge(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        pending_request: RadrootsNostrSignerPendingRequest,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager
            .restore_pending_auth_challenge(connection_id, pending_request)
    }

    fn begin_connect_secret_publish_finalization(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError> {
        Ok(RadrootsNostrSignerPublishTransition::begun(
            self.manager
                .begin_connect_secret_publish_finalization(connection_id)?,
        ))
    }

    fn begin_auth_replay_publish_finalization(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError> {
        Ok(RadrootsNostrSignerPublishTransition::begun(
            self.manager
                .begin_auth_replay_publish_finalization(connection_id)?,
        ))
    }

    fn mark_publish_workflow_published(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError> {
        Ok(RadrootsNostrSignerPublishTransition::marked_published(
            self.manager.mark_publish_workflow_published(workflow_id)?,
        ))
    }

    fn finalize_publish_workflow(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError> {
        Ok(RadrootsNostrSignerPublishTransition::finalized(
            workflow_id.clone(),
            self.manager.finalize_publish_workflow(workflow_id)?,
        ))
    }

    fn cancel_publish_workflow(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<RadrootsNostrSignerPublishTransition, RadrootsNostrSignerError> {
        Ok(RadrootsNostrSignerPublishTransition::cancelled(
            self.manager.cancel_publish_workflow(workflow_id)?,
        ))
    }

    fn mark_authenticated(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager.mark_authenticated(connection_id)
    }

    fn mark_connect_secret_consumed(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.manager.mark_connect_secret_consumed(connection_id)
    }

    fn evaluate_request(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        request_message: RadrootsNostrConnectRequestMessage,
    ) -> Result<RadrootsNostrSignerRequestEvaluation, RadrootsNostrSignerError> {
        self.manager
            .evaluate_request(connection_id, request_message)
    }

    fn evaluate_auth_replay_publish_workflow(
        &self,
        workflow_id: &RadrootsNostrSignerWorkflowId,
    ) -> Result<RadrootsNostrSignerRequestEvaluation, RadrootsNostrSignerError> {
        self.manager
            .evaluate_auth_replay_publish_workflow(workflow_id)
    }

    fn record_request(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        request_id: &str,
        method: RadrootsNostrConnectMethod,
        decision: RadrootsNostrSignerRequestDecision,
        message: Option<String>,
    ) -> Result<RadrootsNostrSignerRequestAuditRecord, RadrootsNostrSignerError> {
        self.manager
            .record_request(connection_id, request_id, method, decision, message)
    }

    fn sign_unsigned_event(
        &self,
        unsigned_event: UnsignedEvent,
    ) -> Result<RadrootsNostrSignerSignOutput, RadrootsNostrSignerError> {
        let event = unsigned_event
            .sign_with_keys(self.signer_identity.keys())
            .map_err(|error| RadrootsNostrSignerError::Sign(error.to_string()))?;
        Ok(RadrootsNostrSignerSignOutput::new(
            RadrootsNostrSignerCapability::LocalAccount(self.local_signer_capability()),
            event,
        ))
    }
}

fn same_public_identity_key(left: &RadrootsIdentityPublic, right: &RadrootsIdentityPublic) -> bool {
    left.id == right.id
        && left.public_key_hex == right.public_key_hex
        && left.public_key_npub == right.public_key_npub
}

fn parse_identity_public_key(
    identity: &RadrootsIdentityPublic,
) -> Result<PublicKey, RadrootsNostrSignerError> {
    PublicKey::parse(identity.public_key_hex.as_str())
        .or_else(|_| PublicKey::from_hex(identity.public_key_hex.as_str()))
        .map_err(|_| {
            RadrootsNostrSignerError::InvalidState("identity public key is invalid".into())
        })
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsNostrEmbeddedSignerBackend, RadrootsNostrSignerBackend,
        RadrootsNostrSignerPublishTransition,
    };
    use crate::evaluation::RadrootsNostrSignerConnectEvaluation;
    use crate::manager::RadrootsNostrSignerManager;
    use crate::model::{RadrootsNostrSignerConnectionDraft, RadrootsNostrSignerRequestDecision};
    use crate::test_support::{
        fixture_bob_identity, primary_relay, synthetic_public_identity, synthetic_public_key,
        synthetic_secret_hex,
    };
    use nostr::{EventBuilder, Kind};
    use radroots_identity::RadrootsIdentity;
    use radroots_nostr_connect::prelude::{
        RadrootsNostrConnectMethod, RadrootsNostrConnectPermission, RadrootsNostrConnectRequest,
        RadrootsNostrConnectRequestMessage,
    };

    fn embedded_identity(index: u32) -> RadrootsIdentity {
        RadrootsIdentity::from_secret_key_str(synthetic_secret_hex(index).as_str())
            .expect("identity")
    }

    #[test]
    fn embedded_backend_bootstraps_signer_identity_and_capabilities() {
        let identity = embedded_identity(0x90);
        let backend = RadrootsNostrEmbeddedSignerBackend::new_in_memory(identity.clone())
            .expect("embedded backend");

        let signer_identity = backend
            .signer_identity()
            .expect("signer identity")
            .expect("present");
        assert_eq!(signer_identity.id, identity.to_public().id);

        let capabilities = backend.capabilities().expect("capabilities");
        let local = capabilities.local_signer.clone().expect("local signer");
        assert_eq!(local.public_identity.id, identity.to_public().id);
        assert!(local.is_secret_backed());
        assert!(capabilities.remote_sessions.is_empty());
        assert_eq!(capabilities.all_signers().len(), 1);
    }

    #[test]
    fn embedded_backend_rejects_mismatched_manager_identity() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        manager
            .set_signer_identity(fixture_bob_identity())
            .expect("set signer identity");

        let error = RadrootsNostrEmbeddedSignerBackend::new(manager, embedded_identity(0x91))
            .err()
            .expect("mismatched identity");
        assert!(
            error
                .to_string()
                .contains("embedded signer identity does not match")
        );
    }

    #[test]
    fn embedded_backend_trait_delegates_connect_and_publish_workflow_methods() {
        let identity = embedded_identity(0x92);
        let backend = RadrootsNostrEmbeddedSignerBackend::new_in_memory(identity.clone())
            .expect("embedded backend");
        let backend: &dyn RadrootsNostrSignerBackend = &backend;

        let evaluation = backend
            .evaluate_connect_request(
                synthetic_public_key(0x93),
                RadrootsNostrConnectRequest::Connect {
                    remote_signer_public_key: identity.public_key(),
                    secret: Some("connect-secret".into()),
                    requested_permissions: vec![RadrootsNostrConnectPermission::new(
                        RadrootsNostrConnectMethod::Ping,
                    )]
                    .into(),
                },
            )
            .expect("connect evaluation");
        let proposal = match evaluation {
            RadrootsNostrSignerConnectEvaluation::RegistrationRequired(proposal) => proposal,
            other => panic!("unexpected connect evaluation: {other:?}"),
        };
        let connection = backend
            .register_connection(
                proposal
                    .into_connection_draft(synthetic_public_identity(0x94))
                    .with_relays(vec![primary_relay()]),
            )
            .expect("register connection");

        let capabilities = backend.capabilities().expect("capabilities");
        assert_eq!(capabilities.remote_sessions.len(), 1);

        let begun = backend
            .begin_connect_secret_publish_finalization(&connection.connection_id)
            .expect("begin workflow");
        let workflow_id = match begun {
            RadrootsNostrSignerPublishTransition::Begun(workflow) => {
                assert_eq!(workflow.connection_id, connection.connection_id);
                workflow.workflow_id
            }
            other => panic!("unexpected begin transition: {other:?}"),
        };

        let published = backend
            .mark_publish_workflow_published(&workflow_id)
            .expect("mark published");
        assert!(matches!(
            published,
            RadrootsNostrSignerPublishTransition::MarkedPublished(_)
        ));

        let finalized = backend
            .finalize_publish_workflow(&workflow_id)
            .expect("finalize workflow");
        match finalized {
            RadrootsNostrSignerPublishTransition::Finalized {
                workflow_id: finalized_workflow_id,
                connection,
            } => {
                assert_eq!(finalized_workflow_id, workflow_id);
                assert!(connection.connect_secret_is_consumed());
            }
            other => panic!("unexpected finalize transition: {other:?}"),
        }

        let audit = backend
            .record_request(
                &connection.connection_id,
                "req-1",
                RadrootsNostrConnectMethod::Ping,
                RadrootsNostrSignerRequestDecision::Allowed,
                None,
            )
            .expect("record request");
        assert_eq!(audit.method, RadrootsNostrConnectMethod::Ping);
    }

    #[test]
    fn embedded_backend_signs_builder_with_local_capability() {
        let identity = embedded_identity(0x95);
        let backend = RadrootsNostrEmbeddedSignerBackend::new_in_memory(identity.clone())
            .expect("embedded backend");
        let backend: &dyn RadrootsNostrSignerBackend = &backend;

        let output = backend
            .sign_event_builder(EventBuilder::new(Kind::TextNote, "hello"))
            .expect("sign event builder");

        assert_eq!(output.event.pubkey, identity.public_key());
        let local = output.signer.local_account().expect("local signer");
        assert_eq!(local.public_identity.id, identity.to_public().id);
        assert!(local.is_secret_backed());
    }

    #[test]
    fn embedded_backend_can_prepare_and_cancel_auth_replay_workflow() {
        let identity = embedded_identity(0x96);
        let backend = RadrootsNostrEmbeddedSignerBackend::new_in_memory(identity.clone())
            .expect("embedded backend");
        let backend: &dyn RadrootsNostrSignerBackend = &backend;

        let connection = backend
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    synthetic_public_key(0x97),
                    synthetic_public_identity(0x98),
                )
                .with_requested_permissions(
                    vec![RadrootsNostrConnectPermission::new(
                        RadrootsNostrConnectMethod::Ping,
                    )]
                    .into(),
                ),
            )
            .expect("register connection");
        backend
            .require_auth_challenge(&connection.connection_id, "https://api.example.com/auth")
            .expect("require auth");
        backend
            .set_pending_request(
                &connection.connection_id,
                RadrootsNostrConnectRequestMessage::new(
                    "req-auth",
                    RadrootsNostrConnectRequest::Ping,
                ),
            )
            .expect("set pending request");

        let begun = backend
            .begin_auth_replay_publish_finalization(&connection.connection_id)
            .expect("begin auth replay");
        let workflow_id = match begun {
            RadrootsNostrSignerPublishTransition::Begun(workflow) => workflow.workflow_id,
            other => panic!("unexpected begin transition: {other:?}"),
        };

        let cancelled = backend
            .cancel_publish_workflow(&workflow_id)
            .expect("cancel workflow");
        assert!(matches!(
            cancelled,
            RadrootsNostrSignerPublishTransition::Cancelled(_)
        ));
    }
}
