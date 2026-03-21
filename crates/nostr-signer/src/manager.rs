use crate::error::RadrootsNostrSignerError;
use crate::model::{
    RADROOTS_NOSTR_SIGNER_STORE_VERSION, RadrootsNostrSignerApprovalRequirement,
    RadrootsNostrSignerApprovalState, RadrootsNostrSignerConnectionDraft,
    RadrootsNostrSignerConnectionId, RadrootsNostrSignerConnectionRecord,
    RadrootsNostrSignerConnectionStatus, RadrootsNostrSignerPermissionGrant,
    RadrootsNostrSignerRequestAuditRecord, RadrootsNostrSignerRequestDecision,
    RadrootsNostrSignerRequestId, RadrootsNostrSignerStoreState,
};
use crate::store::{RadrootsNostrMemorySignerStore, RadrootsNostrSignerStore};
use nostr::{PublicKey, RelayUrl};
use radroots_identity::RadrootsIdentityPublic;
use radroots_nostr_connect::prelude::{
    RadrootsNostrConnectMethod, RadrootsNostrConnectPermissions,
};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct RadrootsNostrSignerManager {
    store: Arc<dyn RadrootsNostrSignerStore>,
    state: Arc<RwLock<RadrootsNostrSignerStoreState>>,
}

impl RadrootsNostrSignerManager {
    pub fn new_in_memory() -> Self {
        Self {
            store: Arc::new(RadrootsNostrMemorySignerStore::new()),
            state: Arc::new(RwLock::new(RadrootsNostrSignerStoreState::default())),
        }
    }

    pub fn new(store: Arc<dyn RadrootsNostrSignerStore>) -> Result<Self, RadrootsNostrSignerError> {
        let state = store.load()?;
        if state.version != RADROOTS_NOSTR_SIGNER_STORE_VERSION {
            return Err(RadrootsNostrSignerError::InvalidState(format!(
                "unsupported signer schema version {}",
                state.version
            )));
        }

        Ok(Self {
            store,
            state: Arc::new(RwLock::new(state)),
        })
    }

    pub fn signer_identity(
        &self,
    ) -> Result<Option<RadrootsIdentityPublic>, RadrootsNostrSignerError> {
        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrSignerError::Store("signer state lock poisoned".into()))?;
        Ok(guard.signer_identity.clone())
    }

    pub fn set_signer_identity(
        &self,
        signer_identity: RadrootsIdentityPublic,
    ) -> Result<(), RadrootsNostrSignerError> {
        validate_public_identity(&signer_identity)?;
        self.update_state(|state| {
            state.signer_identity = Some(signer_identity);
            Ok(())
        })
    }

    pub fn list_connections(
        &self,
    ) -> Result<Vec<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError> {
        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrSignerError::Store("signer state lock poisoned".into()))?;
        Ok(guard.connections.clone())
    }

    pub fn get_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<Option<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError> {
        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrSignerError::Store("signer state lock poisoned".into()))?;
        Ok(guard
            .connections
            .iter()
            .find(|record| &record.connection_id == connection_id)
            .cloned())
    }

    pub fn find_connections_by_client_public_key(
        &self,
        client_public_key: &PublicKey,
    ) -> Result<Vec<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError> {
        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrSignerError::Store("signer state lock poisoned".into()))?;
        Ok(guard
            .connections
            .iter()
            .filter(|record| &record.client_public_key == client_public_key)
            .cloned()
            .collect())
    }

    pub fn find_connection_by_connect_secret(
        &self,
        connect_secret: &str,
    ) -> Result<Option<RadrootsNostrSignerConnectionRecord>, RadrootsNostrSignerError> {
        let Some(connect_secret) = normalize_optional_string(Some(connect_secret.to_owned()))
        else {
            return Ok(None);
        };

        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrSignerError::Store("signer state lock poisoned".into()))?;
        Ok(guard
            .connections
            .iter()
            .find(|record| {
                !record.is_terminal()
                    && record.connect_secret.as_deref() == Some(connect_secret.as_str())
            })
            .cloned())
    }

    pub fn list_audit_records(
        &self,
    ) -> Result<Vec<RadrootsNostrSignerRequestAuditRecord>, RadrootsNostrSignerError> {
        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrSignerError::Store("signer state lock poisoned".into()))?;
        Ok(guard.audit_records.clone())
    }

    pub fn audit_records_for_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<Vec<RadrootsNostrSignerRequestAuditRecord>, RadrootsNostrSignerError> {
        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrSignerError::Store("signer state lock poisoned".into()))?;
        Ok(guard
            .audit_records
            .iter()
            .filter(|record| &record.connection_id == connection_id)
            .cloned()
            .collect())
    }

    pub fn register_connection(
        &self,
        draft: RadrootsNostrSignerConnectionDraft,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            let signer_identity = state
                .signer_identity
                .clone()
                .ok_or(RadrootsNostrSignerError::MissingSignerIdentity)?;
            validate_public_identity(&signer_identity)?;
            validate_public_identity(&draft.user_identity)?;

            let connect_secret = normalize_optional_string(draft.connect_secret.clone());
            if let Some(secret) = connect_secret.as_deref() {
                if state.connections.iter().any(|record| {
                    !record.is_terminal() && record.connect_secret.as_deref() == Some(secret)
                }) {
                    return Err(RadrootsNostrSignerError::ConnectSecretAlreadyInUse);
                }
            }

            if state.connections.iter().any(|record| {
                !record.is_terminal()
                    && record.client_public_key == draft.client_public_key
                    && record.user_identity.id == draft.user_identity.id
            }) {
                return Err(RadrootsNostrSignerError::ConnectionAlreadyExists {
                    client_public_key: draft.client_public_key.to_hex(),
                    user_identity_id: draft.user_identity.id.to_string(),
                });
            }

            let created_at_unix = now_unix_secs();
            let record = RadrootsNostrSignerConnectionRecord::new(
                RadrootsNostrSignerConnectionId::new_v7(),
                signer_identity,
                RadrootsNostrSignerConnectionDraft {
                    client_public_key: draft.client_public_key,
                    user_identity: draft.user_identity,
                    connect_secret,
                    requested_permissions: normalize_permissions(draft.requested_permissions),
                    relays: normalize_relays(draft.relays),
                    approval_requirement: draft.approval_requirement,
                },
                created_at_unix,
            );
            state.connections.push(record.clone());
            Ok(record)
        })
    }

    pub fn set_granted_permissions(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        granted_permissions: RadrootsNostrConnectPermissions,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            let updated_at_unix = now_unix_secs();
            let record = find_connection_mut(state, connection_id)?;
            if record.is_terminal() {
                return Err(RadrootsNostrSignerError::InvalidState(format!(
                    "cannot update granted permissions for {} connection",
                    status_label(record.status)
                )));
            }

            let granted_permissions = normalize_permissions(granted_permissions);
            validate_granted_permissions(&record.requested_permissions, &granted_permissions)?;
            record.granted_permissions = granted_permissions
                .as_slice()
                .iter()
                .cloned()
                .map(|permission| {
                    RadrootsNostrSignerPermissionGrant::new(permission, updated_at_unix)
                })
                .collect();
            record.touch_updated(updated_at_unix);
            Ok(record.clone())
        })
    }

    pub fn approve_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        granted_permissions: RadrootsNostrConnectPermissions,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            let updated_at_unix = now_unix_secs();
            let record = find_connection_mut(state, connection_id)?;
            if record.approval_requirement != RadrootsNostrSignerApprovalRequirement::ExplicitUser {
                return Err(RadrootsNostrSignerError::InvalidState(
                    "approval not required for connection".into(),
                ));
            }
            if record.is_terminal() {
                return Err(RadrootsNostrSignerError::InvalidState(format!(
                    "cannot approve {} connection",
                    status_label(record.status)
                )));
            }

            let granted_permissions = normalize_permissions(granted_permissions);
            validate_granted_permissions(&record.requested_permissions, &granted_permissions)?;
            record.granted_permissions = granted_permissions
                .as_slice()
                .iter()
                .cloned()
                .map(|permission| {
                    RadrootsNostrSignerPermissionGrant::new(permission, updated_at_unix)
                })
                .collect();
            record.approval_state = RadrootsNostrSignerApprovalState::Approved;
            record.status = RadrootsNostrSignerConnectionStatus::Active;
            record.status_reason = None;
            record.touch_updated(updated_at_unix);
            Ok(record.clone())
        })
    }

    pub fn reject_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        reason: Option<String>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            let updated_at_unix = now_unix_secs();
            let record = find_connection_mut(state, connection_id)?;
            if record.is_terminal() {
                return Err(RadrootsNostrSignerError::InvalidState(format!(
                    "cannot reject {} connection",
                    status_label(record.status)
                )));
            }

            record.approval_state = RadrootsNostrSignerApprovalState::Rejected;
            record.status = RadrootsNostrSignerConnectionStatus::Rejected;
            record.status_reason = normalize_optional_string(reason);
            record.touch_updated(updated_at_unix);
            Ok(record.clone())
        })
    }

    pub fn revoke_connection(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        reason: Option<String>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            let updated_at_unix = now_unix_secs();
            let record = find_connection_mut(state, connection_id)?;
            if record.status == RadrootsNostrSignerConnectionStatus::Revoked {
                return Err(RadrootsNostrSignerError::InvalidState(
                    "connection already revoked".into(),
                ));
            }

            record.status = RadrootsNostrSignerConnectionStatus::Revoked;
            record.status_reason = normalize_optional_string(reason);
            record.touch_updated(updated_at_unix);
            Ok(record.clone())
        })
    }

    pub fn update_relays(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        relays: Vec<RelayUrl>,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            let updated_at_unix = now_unix_secs();
            let record = find_connection_mut(state, connection_id)?;
            if record.is_terminal() {
                return Err(RadrootsNostrSignerError::InvalidState(format!(
                    "cannot update relays for {} connection",
                    status_label(record.status)
                )));
            }

            record.relays = normalize_relays(relays);
            record.touch_updated(updated_at_unix);
            Ok(record.clone())
        })
    }

    pub fn mark_authenticated(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
    ) -> Result<RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            let authenticated_at_unix = now_unix_secs();
            let record = find_connection_mut(state, connection_id)?;
            record.mark_authenticated(authenticated_at_unix);
            Ok(record.clone())
        })
    }

    pub fn record_request(
        &self,
        connection_id: &RadrootsNostrSignerConnectionId,
        request_id: impl AsRef<str>,
        method: RadrootsNostrConnectMethod,
        decision: RadrootsNostrSignerRequestDecision,
        message: Option<String>,
    ) -> Result<RadrootsNostrSignerRequestAuditRecord, RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            let created_at_unix = now_unix_secs();
            let request_id = RadrootsNostrSignerRequestId::parse(request_id.as_ref())?;
            let record = find_connection_mut(state, connection_id)?;
            record.mark_request(created_at_unix);

            let audit = RadrootsNostrSignerRequestAuditRecord::new(
                request_id,
                connection_id.clone(),
                method,
                decision,
                normalize_optional_string(message),
                created_at_unix,
            );
            state.audit_records.push(audit.clone());
            Ok(audit)
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn update_state(
        &self,
        update: impl FnOnce(&mut RadrootsNostrSignerStoreState) -> Result<(), RadrootsNostrSignerError>,
    ) -> Result<(), RadrootsNostrSignerError> {
        self.update_state_with(|state| {
            update(state)?;
            Ok(())
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn update_state_with<T>(
        &self,
        update: impl FnOnce(&mut RadrootsNostrSignerStoreState) -> Result<T, RadrootsNostrSignerError>,
    ) -> Result<T, RadrootsNostrSignerError> {
        let mut guard = self
            .state
            .write()
            .map_err(|_| RadrootsNostrSignerError::Store("signer state lock poisoned".into()))?;
        let mut next = guard.clone();
        let value = match update(&mut next) {
            Ok(value) => value,
            Err(err) => return Err(err),
        };
        if let Err(err) = self.store.save(&next) {
            return Err(err);
        }
        *guard = next;
        Ok(value)
    }
}

fn find_connection_mut<'a>(
    state: &'a mut RadrootsNostrSignerStoreState,
    connection_id: &RadrootsNostrSignerConnectionId,
) -> Result<&'a mut RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerError> {
    state
        .connections
        .iter_mut()
        .find(|record| &record.connection_id == connection_id)
        .ok_or_else(|| RadrootsNostrSignerError::ConnectionNotFound(connection_id.to_string()))
}

fn validate_public_identity(
    identity: &RadrootsIdentityPublic,
) -> Result<(), RadrootsNostrSignerError> {
    if identity.id.as_str() != identity.public_key_hex {
        return Err(RadrootsNostrSignerError::InvalidState(
            "public identity id does not match public key".into(),
        ));
    }
    Ok(())
}

fn validate_granted_permissions(
    requested_permissions: &RadrootsNostrConnectPermissions,
    granted_permissions: &RadrootsNostrConnectPermissions,
) -> Result<(), RadrootsNostrSignerError> {
    if requested_permissions.is_empty() {
        return Ok(());
    }

    let requested = requested_permissions.as_slice();
    if let Some(permission) = granted_permissions
        .as_slice()
        .iter()
        .find(|permission| !requested.contains(permission))
    {
        return Err(RadrootsNostrSignerError::InvalidGrantedPermission(
            permission.to_string(),
        ));
    }
    Ok(())
}

fn normalize_permissions(
    permissions: RadrootsNostrConnectPermissions,
) -> RadrootsNostrConnectPermissions {
    let mut permissions = permissions.into_vec();
    permissions.sort();
    permissions.dedup();
    permissions.into()
}

fn normalize_relays(relays: Vec<RelayUrl>) -> Vec<RelayUrl> {
    let mut relays = relays;
    relays.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    relays.dedup_by(|left, right| left.as_str() == right.as_str());
    relays
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn status_label(status: RadrootsNostrSignerConnectionStatus) -> &'static str {
    match status {
        RadrootsNostrSignerConnectionStatus::Pending => "pending",
        RadrootsNostrSignerConnectionStatus::Active => "active",
        RadrootsNostrSignerConnectionStatus::Rejected => "rejected",
        RadrootsNostrSignerConnectionStatus::Revoked => "revoked",
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::RadrootsNostrSignerStore;
    use nostr::{Keys, SecretKey};
    use radroots_identity::RadrootsIdentity;
    use radroots_nostr_connect::prelude::RadrootsNostrConnectPermission;
    use std::sync::Arc;
    use std::thread;

    fn public_identity(secret_hex: &str) -> RadrootsIdentityPublic {
        RadrootsIdentity::from_secret_key_str(secret_hex)
            .expect("identity")
            .to_public()
    }

    fn invalid_public_identity(secret_hex: &str) -> RadrootsIdentityPublic {
        let mut identity = public_identity(secret_hex);
        let other =
            SecretKey::from_hex("00000000000000000000000000000000000000000000000000000000000000ff")
                .expect("secret");
        identity.id =
            radroots_identity::RadrootsIdentityId::parse(&Keys::new(other).public_key().to_hex())
                .expect("id");
        identity
    }

    fn public_key(secret_hex: &str) -> PublicKey {
        let secret = SecretKey::from_hex(secret_hex).expect("secret");
        Keys::new(secret).public_key()
    }

    fn permission(
        method: RadrootsNostrConnectMethod,
        parameter: Option<&str>,
    ) -> RadrootsNostrConnectPermission {
        match parameter {
            Some(parameter) => RadrootsNostrConnectPermission::with_parameter(method, parameter),
            None => RadrootsNostrConnectPermission::new(method),
        }
    }

    fn relay(url: &str) -> RelayUrl {
        RelayUrl::parse(url).expect("relay")
    }

    fn poison_manager_state(manager: &RadrootsNostrSignerManager) {
        let shared = manager.state.clone();
        let _ = thread::spawn(move || {
            let _guard = shared.write().expect("write");
            panic!("poison signer state");
        })
        .join();
    }

    fn assert_same_public_identity(left: &RadrootsIdentityPublic, right: &RadrootsIdentityPublic) {
        assert_eq!(left.id.as_str(), right.id.as_str());
        assert_eq!(left.public_key_hex, right.public_key_hex);
        assert_eq!(left.public_key_npub, right.public_key_npub);
    }

    fn assert_same_connection(
        left: &RadrootsNostrSignerConnectionRecord,
        right: &RadrootsNostrSignerConnectionRecord,
    ) {
        assert_eq!(left.connection_id, right.connection_id);
        assert_eq!(left.client_public_key, right.client_public_key);
        assert_same_public_identity(&left.signer_identity, &right.signer_identity);
        assert_same_public_identity(&left.user_identity, &right.user_identity);
        assert_eq!(left.connect_secret, right.connect_secret);
        assert_eq!(left.requested_permissions, right.requested_permissions);
        assert_eq!(left.granted_permissions, right.granted_permissions);
        assert_eq!(left.relays, right.relays);
        assert_eq!(left.approval_requirement, right.approval_requirement);
        assert_eq!(left.approval_state, right.approval_state);
        assert_eq!(left.status, right.status);
        assert_eq!(left.status_reason, right.status_reason);
        assert_eq!(left.created_at_unix, right.created_at_unix);
        assert_eq!(left.updated_at_unix, right.updated_at_unix);
        assert_eq!(
            left.last_authenticated_at_unix,
            right.last_authenticated_at_unix
        );
        assert_eq!(left.last_request_at_unix, right.last_request_at_unix);
    }

    struct LoadErrorStore;

    impl RadrootsNostrSignerStore for LoadErrorStore {
        fn load(&self) -> Result<RadrootsNostrSignerStoreState, RadrootsNostrSignerError> {
            Err(RadrootsNostrSignerError::Store("store load failed".into()))
        }

        fn save(
            &self,
            _state: &RadrootsNostrSignerStoreState,
        ) -> Result<(), RadrootsNostrSignerError> {
            Ok(())
        }
    }

    struct SaveErrorStore {
        state: RwLock<RadrootsNostrSignerStoreState>,
    }

    impl SaveErrorStore {
        fn new(state: RadrootsNostrSignerStoreState) -> Self {
            Self {
                state: RwLock::new(state),
            }
        }
    }

    impl RadrootsNostrSignerStore for SaveErrorStore {
        fn load(&self) -> Result<RadrootsNostrSignerStoreState, RadrootsNostrSignerError> {
            self.state
                .read()
                .map(|guard| guard.clone())
                .map_err(|_| RadrootsNostrSignerError::Store("save error store poisoned".into()))
        }

        fn save(
            &self,
            _state: &RadrootsNostrSignerStoreState,
        ) -> Result<(), RadrootsNostrSignerError> {
            Err(RadrootsNostrSignerError::Store("store save failed".into()))
        }
    }

    #[test]
    fn manager_new_in_memory_and_invalid_schema_paths() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        assert!(
            manager
                .signer_identity()
                .expect("signer identity")
                .is_none()
        );

        let load_error_store = Arc::new(LoadErrorStore);
        load_error_store
            .save(&RadrootsNostrSignerStoreState::default())
            .expect("load error store save");
        let load_result = RadrootsNostrSignerManager::new(load_error_store);
        assert!(load_result.is_err());
        let err = load_result.err().expect("load error");
        assert!(err.to_string().contains("store load failed"));

        let store = Arc::new(RadrootsNostrMemorySignerStore::new());
        let mut state = RadrootsNostrSignerStoreState::default();
        state.version = 2;
        store.save(&state).expect("save");
        let version_result = RadrootsNostrSignerManager::new(store);
        assert!(version_result.is_err());
        let err = version_result.err().expect("invalid version");
        assert!(
            err.to_string()
                .contains("unsupported signer schema version")
        );
    }

    #[test]
    fn set_signer_identity_validates_and_persists() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        let signer_identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000001");
        manager
            .set_signer_identity(signer_identity.clone())
            .expect("set signer");

        let loaded = manager
            .signer_identity()
            .expect("identity")
            .expect("loaded");
        assert_same_public_identity(&loaded, &signer_identity);

        let err = manager
            .set_signer_identity(invalid_public_identity(
                "0000000000000000000000000000000000000000000000000000000000000002",
            ))
            .expect_err("invalid identity");
        assert!(
            err.to_string()
                .contains("public identity id does not match public key")
        );
    }

    #[test]
    fn register_connection_requires_signer_identity_and_normalizes_inputs() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        let err = manager
            .register_connection(RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000003"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000004"),
            ))
            .expect_err("missing signer");
        assert!(err.to_string().contains("missing signer identity"));

        manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000005",
            ))
            .expect("set signer");

        let sign_event = permission(RadrootsNostrConnectMethod::SignEvent, Some("kind:1"));
        let ping = permission(RadrootsNostrConnectMethod::Ping, None);
        let record = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    public_key("0000000000000000000000000000000000000000000000000000000000000006"),
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000007",
                    ),
                )
                .with_connect_secret(" secret ")
                .with_requested_permissions(
                    vec![sign_event.clone(), ping.clone(), sign_event.clone()].into(),
                )
                .with_relays(vec![
                    relay("wss://z.example"),
                    relay("wss://a.example"),
                    relay("wss://a.example"),
                ]),
            )
            .expect("register");

        assert_eq!(record.connect_secret.as_deref(), Some("secret"));
        assert_eq!(record.status, RadrootsNostrSignerConnectionStatus::Active);
        assert_eq!(
            record.approval_state,
            RadrootsNostrSignerApprovalState::NotRequired
        );
        assert_eq!(record.requested_permissions.as_slice(), &[sign_event, ping]);
        assert_eq!(
            record
                .relays
                .iter()
                .map(|relay| relay.as_str().to_owned())
                .collect::<Vec<_>>(),
            vec!["wss://a.example", "wss://z.example"]
        );
    }

    #[test]
    fn register_connection_enforces_identity_and_uniqueness_rules() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000008",
            ))
            .expect("set signer");

        let user_identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000009");
        let client_public_key =
            public_key("0000000000000000000000000000000000000000000000000000000000000010");
        let pending = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(client_public_key, user_identity.clone())
                    .with_connect_secret("shared-secret")
                    .with_approval_requirement(
                        RadrootsNostrSignerApprovalRequirement::ExplicitUser,
                    ),
            )
            .expect("register");
        assert_eq!(pending.status, RadrootsNostrSignerConnectionStatus::Pending);

        let duplicate_connection = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(client_public_key, user_identity)
                    .with_connect_secret("other-secret"),
            )
            .expect_err("duplicate connection");
        assert!(
            duplicate_connection
                .to_string()
                .contains("connection already exists")
        );

        let duplicate_secret = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    public_key("0000000000000000000000000000000000000000000000000000000000000011"),
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000012",
                    ),
                )
                .with_connect_secret("shared-secret"),
            )
            .expect_err("duplicate secret");
        assert!(
            duplicate_secret
                .to_string()
                .contains("connect secret already in use")
        );

        let invalid_user = manager
            .register_connection(RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000013"),
                invalid_public_identity(
                    "0000000000000000000000000000000000000000000000000000000000000014",
                ),
            ))
            .expect_err("invalid user identity");
        assert!(
            invalid_user
                .to_string()
                .contains("public identity id does not match public key")
        );
    }

    #[test]
    fn manager_query_helpers_find_connections() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000015",
            ))
            .expect("set signer");

        let client_public_key =
            public_key("0000000000000000000000000000000000000000000000000000000000000016");
        let record = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    client_public_key,
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000017",
                    ),
                )
                .with_connect_secret("lookup-secret"),
            )
            .expect("register");

        let by_id = manager
            .get_connection(&record.connection_id)
            .expect("get connection");
        let by_client = manager
            .find_connections_by_client_public_key(&client_public_key)
            .expect("find by client");
        let by_secret = manager
            .find_connection_by_connect_secret(" lookup-secret ")
            .expect("find by secret");
        let empty_secret = manager
            .find_connection_by_connect_secret("   ")
            .expect("empty secret");
        let all_connections = manager.list_connections().expect("list connections");

        assert_same_connection(&by_id.expect("by id"), &record);
        assert_eq!(by_client.len(), 1);
        assert_same_connection(&by_client[0], &record);
        assert_same_connection(&by_secret.expect("by secret"), &record);
        assert!(empty_secret.is_none());
        assert_eq!(all_connections.len(), 1);
        assert_same_connection(&all_connections[0], &record);
    }

    #[test]
    fn granted_permissions_and_approval_enforce_subset_rules() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000018",
            ))
            .expect("set signer");
        let requested = vec![
            permission(RadrootsNostrConnectMethod::SignEvent, Some("kind:1")),
            permission(RadrootsNostrConnectMethod::Ping, None),
        ];
        let granted = vec![requested[1].clone()];
        let invalid = vec![permission(
            RadrootsNostrConnectMethod::Nip44Encrypt,
            Some("kind:1"),
        )];
        let pending = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    public_key("0000000000000000000000000000000000000000000000000000000000000019"),
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000020",
                    ),
                )
                .with_requested_permissions(requested.clone().into())
                .with_approval_requirement(RadrootsNostrSignerApprovalRequirement::ExplicitUser),
            )
            .expect("register");

        let invalid_set = manager
            .set_granted_permissions(&pending.connection_id, invalid.clone().into())
            .expect_err("invalid set grants");
        assert!(
            invalid_set
                .to_string()
                .contains("invalid granted permission")
        );

        let set_grants = manager
            .set_granted_permissions(&pending.connection_id, granted.clone().into())
            .expect("set grants");
        assert_eq!(
            set_grants.granted_permissions().as_slice(),
            granted.as_slice()
        );
        assert_eq!(
            set_grants.status,
            RadrootsNostrSignerConnectionStatus::Pending
        );

        let approved = manager
            .approve_connection(&pending.connection_id, granted.clone().into())
            .expect("approve");
        assert_eq!(approved.status, RadrootsNostrSignerConnectionStatus::Active);
        assert_eq!(
            approved.approval_state,
            RadrootsNostrSignerApprovalState::Approved
        );
        assert_eq!(
            approved.granted_permissions().as_slice(),
            granted.as_slice()
        );

        let reapprove = manager
            .approve_connection(&pending.connection_id, granted.into())
            .expect("reapprove active");
        assert_eq!(
            reapprove.status,
            RadrootsNostrSignerConnectionStatus::Active
        );

        let auto = manager
            .register_connection(RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000021"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000022"),
            ))
            .expect("register auto");
        let err = manager
            .approve_connection(
                &auto.connection_id,
                RadrootsNostrConnectPermissions::default(),
            )
            .expect_err("approval not required");
        assert!(err.to_string().contains("approval not required"));

        let terminal_pending = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    public_key("0000000000000000000000000000000000000000000000000000000000000040"),
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000041",
                    ),
                )
                .with_connect_secret("terminal-secret")
                .with_approval_requirement(RadrootsNostrSignerApprovalRequirement::ExplicitUser),
            )
            .expect("register terminal");
        manager
            .reject_connection(&terminal_pending.connection_id, Some("terminal".into()))
            .expect("reject terminal");
        let terminal_approve = manager
            .approve_connection(
                &terminal_pending.connection_id,
                vec![requested[0].clone()].into(),
            )
            .expect_err("approve rejected");
        assert!(
            terminal_approve
                .to_string()
                .contains("cannot approve rejected connection")
        );

        let unrestricted = manager
            .register_connection(RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000023"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000024"),
            ))
            .expect("register unrestricted");
        let unrestricted_grants = manager
            .set_granted_permissions(&unrestricted.connection_id, invalid.into())
            .expect("unrestricted grants");
        assert_eq!(unrestricted_grants.granted_permissions.len(), 1);
    }

    #[test]
    fn reject_revoke_and_relay_updates_cover_terminal_paths() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000025",
            ))
            .expect("set signer");
        let rejected = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    public_key("0000000000000000000000000000000000000000000000000000000000000026"),
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000027",
                    ),
                )
                .with_connect_secret("shared-secret")
                .with_approval_requirement(RadrootsNostrSignerApprovalRequirement::ExplicitUser),
            )
            .expect("register reject");
        let rejected = manager
            .reject_connection(&rejected.connection_id, Some("denied".into()))
            .expect("reject");
        assert_eq!(
            rejected.status,
            RadrootsNostrSignerConnectionStatus::Rejected
        );
        assert_eq!(rejected.status_reason.as_deref(), Some("denied"));

        let reject_err = manager
            .reject_connection(&rejected.connection_id, None)
            .expect_err("reject terminal");
        assert!(
            reject_err
                .to_string()
                .contains("cannot reject rejected connection")
        );

        let relay_err = manager
            .update_relays(&rejected.connection_id, vec![relay("wss://relay.example")])
            .expect_err("update rejected");
        assert!(
            relay_err
                .to_string()
                .contains("cannot update relays for rejected connection")
        );
        let rejected_lookup = manager
            .find_connection_by_connect_secret("shared-secret")
            .expect("lookup rejected secret");
        assert!(rejected_lookup.is_none());

        let active = manager
            .register_connection(RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000028"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000029"),
            ))
            .expect("register active");
        let active = manager
            .update_relays(
                &active.connection_id,
                vec![
                    relay("wss://b.example"),
                    relay("wss://a.example"),
                    relay("wss://a.example"),
                ],
            )
            .expect("update relays");
        assert_eq!(
            active
                .relays
                .iter()
                .map(|relay| relay.as_str().to_owned())
                .collect::<Vec<_>>(),
            vec!["wss://a.example", "wss://b.example"]
        );

        let revoked = manager
            .revoke_connection(&active.connection_id, Some("manual".into()))
            .expect("revoke");
        assert_eq!(revoked.status, RadrootsNostrSignerConnectionStatus::Revoked);
        assert_eq!(revoked.status_reason.as_deref(), Some("manual"));

        let revoke_again = manager
            .revoke_connection(&active.connection_id, None)
            .expect_err("revoke twice");
        assert!(
            revoke_again
                .to_string()
                .contains("connection already revoked")
        );

        let grants_err = manager
            .set_granted_permissions(
                &active.connection_id,
                vec![permission(RadrootsNostrConnectMethod::Ping, None)].into(),
            )
            .expect_err("update grants revoked");
        assert!(
            grants_err
                .to_string()
                .contains("cannot update granted permissions for revoked connection")
        );
    }

    #[test]
    fn authentication_and_request_audit_paths_are_recorded() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000030",
            ))
            .expect("set signer");
        let record = manager
            .register_connection(RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000031"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000032"),
            ))
            .expect("register");

        let authenticated = manager
            .mark_authenticated(&record.connection_id)
            .expect("auth");
        assert!(authenticated.last_authenticated_at_unix.is_some());

        let audit = manager
            .record_request(
                &record.connection_id,
                " request-1 ",
                RadrootsNostrConnectMethod::Ping,
                RadrootsNostrSignerRequestDecision::Challenged,
                Some(" challenge ".into()),
            )
            .expect("record request");
        assert_eq!(audit.request_id.as_str(), "request-1");
        assert_eq!(audit.message.as_deref(), Some("challenge"));

        let all_audits = manager.list_audit_records().expect("list audits");
        let connection_audits = manager
            .audit_records_for_connection(&record.connection_id)
            .expect("connection audits");
        let stored = manager
            .get_connection(&record.connection_id)
            .expect("get")
            .expect("stored");
        assert_eq!(all_audits, vec![audit.clone()]);
        assert_eq!(connection_audits, vec![audit]);
        assert!(stored.last_request_at_unix.is_some());

        let request_err = manager
            .record_request(
                &record.connection_id,
                "   ",
                RadrootsNostrConnectMethod::Ping,
                RadrootsNostrSignerRequestDecision::Denied,
                None,
            )
            .expect_err("invalid request id");
        assert!(request_err.to_string().contains("invalid request id"));
    }

    #[test]
    fn manager_reports_missing_connections_and_save_failures() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        let missing_id = RadrootsNostrSignerConnectionId::parse("missing").expect("id");
        let missing_get = manager.get_connection(&missing_id).expect("missing get");
        assert!(missing_get.is_none());

        let mark_err = manager
            .mark_authenticated(&missing_id)
            .expect_err("missing auth");
        assert!(mark_err.to_string().contains("connection not found"));

        let save_error_store =
            Arc::new(SaveErrorStore::new(RadrootsNostrSignerStoreState::default()));
        let loaded_state = save_error_store.load().expect("load save error store");
        assert_eq!(loaded_state.version, RADROOTS_NOSTR_SIGNER_STORE_VERSION);
        let manager = RadrootsNostrSignerManager::new(save_error_store).expect("manager");
        let err = manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000033",
            ))
            .expect_err("save error");
        assert!(err.to_string().contains("store save failed"));
    }

    #[test]
    fn mutation_methods_cover_remaining_error_paths() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000051",
            ))
            .expect("set signer");

        let missing_id = RadrootsNostrSignerConnectionId::parse("missing-2").expect("id");
        let missing_permissions: RadrootsNostrConnectPermissions =
            vec![permission(RadrootsNostrConnectMethod::Ping, None)].into();

        let missing_grants = manager
            .set_granted_permissions(&missing_id, missing_permissions.clone())
            .expect_err("missing grants");
        let missing_approve = manager
            .approve_connection(&missing_id, RadrootsNostrConnectPermissions::default())
            .expect_err("missing approve");
        let missing_reject = manager
            .reject_connection(&missing_id, None)
            .expect_err("missing reject");
        let missing_revoke = manager
            .revoke_connection(&missing_id, None)
            .expect_err("missing revoke");
        let missing_relays = manager
            .update_relays(&missing_id, vec![relay("wss://relay.example")])
            .expect_err("missing relays");
        let missing_request = manager
            .record_request(
                &missing_id,
                "req-missing",
                RadrootsNostrConnectMethod::Ping,
                RadrootsNostrSignerRequestDecision::Denied,
                None,
            )
            .expect_err("missing request");

        for err in [
            missing_grants,
            missing_approve,
            missing_reject,
            missing_revoke,
            missing_relays,
            missing_request,
        ] {
            assert!(err.to_string().contains("connection not found"));
        }

        let requested = vec![permission(RadrootsNostrConnectMethod::Ping, None)];
        let pending = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    public_key("0000000000000000000000000000000000000000000000000000000000000052"),
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000053",
                    ),
                )
                .with_requested_permissions(requested.into())
                .with_approval_requirement(RadrootsNostrSignerApprovalRequirement::ExplicitUser),
            )
            .expect("register pending");
        let invalid_approve = manager
            .approve_connection(
                &pending.connection_id,
                vec![permission(
                    RadrootsNostrConnectMethod::Nip44Encrypt,
                    Some("kind:1"),
                )]
                .into(),
            )
            .expect_err("invalid approve grants");
        assert!(
            invalid_approve
                .to_string()
                .contains("invalid granted permission")
        );

        let update_state_err = manager
            .update_state(|_| Err(RadrootsNostrSignerError::InvalidState("manual".into())))
            .expect_err("update_state error");
        assert!(update_state_err.to_string().contains("manual"));
    }

    #[test]
    fn register_connection_rejects_invalid_persisted_signer_identity() {
        let store = Arc::new(RadrootsNostrMemorySignerStore::new());
        let mut state = RadrootsNostrSignerStoreState::default();
        state.signer_identity = Some(invalid_public_identity(
            "0000000000000000000000000000000000000000000000000000000000000054",
        ));
        store.save(&state).expect("seed state");

        let manager = RadrootsNostrSignerManager::new(store).expect("manager");
        let err = manager
            .register_connection(RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000055"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000056"),
            ))
            .expect_err("invalid signer identity");
        assert!(
            err.to_string()
                .contains("public identity id does not match public key")
        );
    }

    #[test]
    fn manager_reports_poisoned_state_lock() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        poison_manager_state(&manager);

        let identity = manager.signer_identity().expect_err("poisoned read");
        assert!(identity.to_string().contains("signer state lock poisoned"));
    }

    #[test]
    fn read_helpers_report_poisoned_state_lock() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        poison_manager_state(&manager);

        let connection_id = RadrootsNostrSignerConnectionId::parse("conn-1").expect("id");
        let client_public_key =
            public_key("0000000000000000000000000000000000000000000000000000000000000047");

        let get_err = manager
            .get_connection(&connection_id)
            .expect_err("poisoned get");
        let list_err = manager.list_connections().expect_err("poisoned list");
        let audit_list_err = manager
            .list_audit_records()
            .expect_err("poisoned audit list");
        let audit_for_connection_err = manager
            .audit_records_for_connection(&connection_id)
            .expect_err("poisoned audit connection");
        let find_secret_err = manager
            .find_connection_by_connect_secret("secret")
            .expect_err("poisoned secret lookup");
        let find_client_err = manager
            .find_connections_by_client_public_key(&client_public_key)
            .expect_err("poisoned client lookup");

        for err in [
            get_err,
            list_err,
            audit_list_err,
            audit_for_connection_err,
            find_secret_err,
            find_client_err,
        ] {
            assert!(err.to_string().contains("signer state lock poisoned"));
        }
    }

    #[test]
    fn mutation_helpers_report_poisoned_state_lock() {
        let manager = RadrootsNostrSignerManager::new_in_memory();
        poison_manager_state(&manager);

        let signer_identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000048");
        let connection_id = RadrootsNostrSignerConnectionId::parse("conn-2").expect("id");
        let connect_draft = RadrootsNostrSignerConnectionDraft::new(
            public_key("0000000000000000000000000000000000000000000000000000000000000049"),
            public_identity("0000000000000000000000000000000000000000000000000000000000000050"),
        );

        let set_signer_err = manager
            .set_signer_identity(signer_identity)
            .expect_err("poisoned set signer");
        let register_err = manager
            .register_connection(connect_draft)
            .expect_err("poisoned register");
        let grants_err = manager
            .set_granted_permissions(
                &connection_id,
                vec![permission(RadrootsNostrConnectMethod::Ping, None)].into(),
            )
            .expect_err("poisoned set grants");
        let approve_err = manager
            .approve_connection(&connection_id, RadrootsNostrConnectPermissions::default())
            .expect_err("poisoned approve");
        let reject_err = manager
            .reject_connection(&connection_id, Some("reason".into()))
            .expect_err("poisoned reject");
        let revoke_err = manager
            .revoke_connection(&connection_id, Some("reason".into()))
            .expect_err("poisoned revoke");
        let update_relays_err = manager
            .update_relays(&connection_id, vec![relay("wss://relay.example")])
            .expect_err("poisoned relays");
        let auth_err = manager
            .mark_authenticated(&connection_id)
            .expect_err("poisoned auth");
        let request_err = manager
            .record_request(
                &connection_id,
                "req-1",
                RadrootsNostrConnectMethod::Ping,
                RadrootsNostrSignerRequestDecision::Allowed,
                None,
            )
            .expect_err("poisoned request");

        for err in [
            set_signer_err,
            register_err,
            grants_err,
            approve_err,
            reject_err,
            revoke_err,
            update_relays_err,
            auth_err,
            request_err,
        ] {
            assert!(err.to_string().contains("signer state lock poisoned"));
        }
    }

    #[test]
    fn save_error_store_reports_poisoned_load_lock() {
        let store = SaveErrorStore::new(RadrootsNostrSignerStoreState::default());
        let shared = Arc::new(store);
        let poison = shared.clone();
        let _ = thread::spawn(move || {
            let _guard = poison.state.write().expect("write");
            panic!("poison save error store");
        })
        .join();

        let err = shared.load().expect_err("poisoned load");
        assert!(err.to_string().contains("save error store poisoned"));
    }

    #[test]
    fn helpers_cover_status_labels_and_terminal_secret_reuse() {
        assert_eq!(
            status_label(RadrootsNostrSignerConnectionStatus::Pending),
            "pending"
        );
        assert_eq!(
            status_label(RadrootsNostrSignerConnectionStatus::Active),
            "active"
        );
        assert_eq!(
            status_label(RadrootsNostrSignerConnectionStatus::Rejected),
            "rejected"
        );
        assert_eq!(
            status_label(RadrootsNostrSignerConnectionStatus::Revoked),
            "revoked"
        );

        let manager = RadrootsNostrSignerManager::new_in_memory();
        manager
            .set_signer_identity(public_identity(
                "0000000000000000000000000000000000000000000000000000000000000042",
            ))
            .expect("set signer");

        let initial = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    public_key("0000000000000000000000000000000000000000000000000000000000000043"),
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000044",
                    ),
                )
                .with_connect_secret("reusable-secret")
                .with_approval_requirement(RadrootsNostrSignerApprovalRequirement::ExplicitUser),
            )
            .expect("register initial");
        manager
            .reject_connection(&initial.connection_id, Some("closed".into()))
            .expect("reject initial");

        let reused = manager
            .register_connection(
                RadrootsNostrSignerConnectionDraft::new(
                    public_key("0000000000000000000000000000000000000000000000000000000000000045"),
                    public_identity(
                        "0000000000000000000000000000000000000000000000000000000000000046",
                    ),
                )
                .with_connect_secret("reusable-secret"),
            )
            .expect("register reused secret");

        assert_eq!(reused.connect_secret.as_deref(), Some("reusable-secret"));
    }
}
