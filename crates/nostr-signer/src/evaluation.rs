use crate::error::RadrootsNostrSignerError;
use crate::model::{
    RadrootsNostrSignerAuthChallenge, RadrootsNostrSignerConnectionDraft,
    RadrootsNostrSignerConnectionRecord, RadrootsNostrSignerPendingRequest,
    RadrootsNostrSignerRequestAuditRecord, RadrootsNostrSignerRequestId,
};
use nostr::{PublicKey, RelayUrl};
use radroots_identity::RadrootsIdentityPublic;
use radroots_nostr_connect::prelude::{
    RadrootsNostrConnectMethod, RadrootsNostrConnectPermission, RadrootsNostrConnectPermissions,
    RadrootsNostrConnectRequest,
};

#[derive(Debug, Clone)]
pub enum RadrootsNostrSignerSessionLookup {
    None,
    Connection(RadrootsNostrSignerConnectionRecord),
    Ambiguous(Vec<RadrootsNostrSignerConnectionRecord>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsNostrSignerConnectProposal {
    pub client_public_key: PublicKey,
    pub connect_secret: Option<String>,
    pub requested_permissions: RadrootsNostrConnectPermissions,
}

#[derive(Debug, Clone)]
pub enum RadrootsNostrSignerConnectEvaluation {
    ExistingConnection(RadrootsNostrSignerConnectionRecord),
    RegistrationRequired(RadrootsNostrSignerConnectProposal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrSignerRequestResponseHint {
    None,
    Pong,
    UserPublicKey(PublicKey),
    RelayList(Vec<RelayUrl>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrSignerRequestAction {
    Allowed {
        required_permission: Option<RadrootsNostrConnectPermission>,
        response_hint: RadrootsNostrSignerRequestResponseHint,
    },
    Denied {
        reason: String,
    },
    Challenged {
        auth_challenge: RadrootsNostrSignerAuthChallenge,
        pending_request: RadrootsNostrSignerPendingRequest,
    },
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrSignerRequestEvaluation {
    pub request_id: RadrootsNostrSignerRequestId,
    pub method: RadrootsNostrConnectMethod,
    pub connection: RadrootsNostrSignerConnectionRecord,
    pub audit: RadrootsNostrSignerRequestAuditRecord,
    pub action: RadrootsNostrSignerRequestAction,
}

impl RadrootsNostrSignerConnectProposal {
    pub fn into_connection_draft(
        self,
        user_identity: RadrootsIdentityPublic,
    ) -> RadrootsNostrSignerConnectionDraft {
        let mut draft =
            RadrootsNostrSignerConnectionDraft::new(self.client_public_key, user_identity)
                .with_requested_permissions(self.requested_permissions);
        if let Some(connect_secret) = self.connect_secret {
            draft = draft.with_connect_secret(connect_secret);
        }
        draft
    }
}

impl RadrootsNostrSignerRequestEvaluation {
    pub fn denied_reason(&self) -> Option<&str> {
        match &self.action {
            RadrootsNostrSignerRequestAction::Denied { reason } => Some(reason.as_str()),
            _ => None,
        }
    }
}

impl RadrootsNostrSignerRequestAction {
    pub fn audit_message(&self) -> Option<String> {
        match self {
            Self::Allowed { .. } => None,
            Self::Denied { reason } => Some(reason.clone()),
            Self::Challenged { .. } => Some("auth challenge required".into()),
        }
    }
}

pub(crate) fn required_permission_for_request(
    request: &RadrootsNostrConnectRequest,
) -> Option<RadrootsNostrConnectPermission> {
    match request {
        RadrootsNostrConnectRequest::Connect { .. }
        | RadrootsNostrConnectRequest::GetPublicKey
        | RadrootsNostrConnectRequest::Ping => None,
        RadrootsNostrConnectRequest::SignEvent(unsigned_event) => {
            Some(RadrootsNostrConnectPermission::with_parameter(
                RadrootsNostrConnectMethod::SignEvent,
                format!("kind:{}", unsigned_event.kind.as_u16()),
            ))
        }
        RadrootsNostrConnectRequest::Nip04Encrypt { .. } => Some(
            RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip04Encrypt),
        ),
        RadrootsNostrConnectRequest::Nip04Decrypt { .. } => Some(
            RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip04Decrypt),
        ),
        RadrootsNostrConnectRequest::Nip44Encrypt { .. } => Some(
            RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip44Encrypt),
        ),
        RadrootsNostrConnectRequest::Nip44Decrypt { .. } => Some(
            RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip44Decrypt),
        ),
        RadrootsNostrConnectRequest::SwitchRelays => Some(RadrootsNostrConnectPermission::new(
            RadrootsNostrConnectMethod::SwitchRelays,
        )),
        RadrootsNostrConnectRequest::Custom { method, .. } => {
            Some(RadrootsNostrConnectPermission::new(method.clone()))
        }
    }
}

pub(crate) fn request_allowed_by_permissions(
    granted_permissions: &RadrootsNostrConnectPermissions,
    request: &RadrootsNostrConnectRequest,
) -> bool {
    let Some(required_permission) = required_permission_for_request(request) else {
        return true;
    };

    granted_permissions
        .as_slice()
        .iter()
        .any(|permission| permission_matches(permission, &required_permission))
}

pub(crate) fn response_hint_for_request(
    connection: &RadrootsNostrSignerConnectionRecord,
    request: &RadrootsNostrConnectRequest,
) -> Result<RadrootsNostrSignerRequestResponseHint, RadrootsNostrSignerError> {
    match request {
        RadrootsNostrConnectRequest::GetPublicKey => {
            Ok(RadrootsNostrSignerRequestResponseHint::UserPublicKey(
                identity_public_key(&connection.user_identity)?,
            ))
        }
        RadrootsNostrConnectRequest::Ping => Ok(RadrootsNostrSignerRequestResponseHint::Pong),
        RadrootsNostrConnectRequest::SwitchRelays => Ok(
            RadrootsNostrSignerRequestResponseHint::RelayList(connection.relays.clone()),
        ),
        _ => Ok(RadrootsNostrSignerRequestResponseHint::None),
    }
}

fn permission_matches(
    granted_permission: &RadrootsNostrConnectPermission,
    required_permission: &RadrootsNostrConnectPermission,
) -> bool {
    if granted_permission.method != required_permission.method {
        return false;
    }

    match (
        &granted_permission.method,
        granted_permission.parameter.as_deref(),
        required_permission.parameter.as_deref(),
    ) {
        (RadrootsNostrConnectMethod::SignEvent, None, _) => true,
        (RadrootsNostrConnectMethod::SignEvent, Some(parameter), Some(required)) => {
            parameter == required || parameter == sign_event_kind_suffix(required)
        }
        (_, None, _) => true,
        (_, Some(parameter), Some(required)) => parameter == required,
        (_, Some(_), None) => false,
    }
}

fn sign_event_kind_suffix(value: &str) -> &str {
    value.strip_prefix("kind:").unwrap_or(value)
}

fn identity_public_key(
    identity: &RadrootsIdentityPublic,
) -> Result<PublicKey, RadrootsNostrSignerError> {
    PublicKey::parse(identity.public_key_hex.as_str())
        .or_else(|_| PublicKey::from_hex(identity.public_key_hex.as_str()))
        .map_err(|_| {
            RadrootsNostrSignerError::InvalidState("user identity public key is invalid".into())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{Keys, SecretKey, Timestamp, UnsignedEvent};
    use radroots_identity::RadrootsIdentity;
    use serde_json::json;

    fn public_identity(secret_hex: &str) -> RadrootsIdentityPublic {
        RadrootsIdentity::from_secret_key_str(secret_hex)
            .expect("identity")
            .to_public()
    }

    fn public_key(secret_hex: &str) -> PublicKey {
        let secret = SecretKey::from_hex(secret_hex).expect("secret");
        Keys::new(secret).public_key()
    }

    fn relay(url: &str) -> RelayUrl {
        RelayUrl::parse(url).expect("relay")
    }

    fn unsigned_event(kind: u16) -> UnsignedEvent {
        serde_json::from_value(json!({
            "pubkey": public_key("0000000000000000000000000000000000000000000000000000000000000001").to_hex(),
            "created_at": Timestamp::from(1).as_secs(),
            "kind": kind,
            "tags": [],
            "content": "hello"
        }))
        .expect("unsigned event")
    }

    fn connection() -> RadrootsNostrSignerConnectionRecord {
        RadrootsNostrSignerConnectionRecord::new(
            crate::model::RadrootsNostrSignerConnectionId::new_v7(),
            public_identity("0000000000000000000000000000000000000000000000000000000000000002"),
            RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000003"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000004"),
            )
            .with_relays(vec![relay("wss://relay.example")]),
            1,
        )
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn assert_action_audit_message_none(action: &RadrootsNostrSignerRequestAction) {
        assert_eq!(action.audit_message(), None);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn assert_response_hint_none(hint: RadrootsNostrSignerRequestResponseHint) {
        match hint {
            RadrootsNostrSignerRequestResponseHint::None => {}
            other => panic!("unexpected response hint: {other:?}"),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn assert_response_hint_pong(hint: RadrootsNostrSignerRequestResponseHint) {
        match hint {
            RadrootsNostrSignerRequestResponseHint::Pong => {}
            other => panic!("unexpected response hint: {other:?}"),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn assert_response_hint_user_public_key(hint: RadrootsNostrSignerRequestResponseHint) {
        match hint {
            RadrootsNostrSignerRequestResponseHint::UserPublicKey(_) => {}
            other => panic!("unexpected response hint: {other:?}"),
        }
    }

    #[test]
    fn connect_proposal_builds_connection_draft() {
        let requested_permissions: RadrootsNostrConnectPermissions =
            vec![RadrootsNostrConnectPermission::new(
                RadrootsNostrConnectMethod::Nip04Encrypt,
            )]
            .into();
        let proposal = RadrootsNostrSignerConnectProposal {
            client_public_key: public_key(
                "0000000000000000000000000000000000000000000000000000000000000005",
            ),
            connect_secret: Some("secret".into()),
            requested_permissions: requested_permissions.clone(),
        };

        let draft = proposal.into_connection_draft(public_identity(
            "0000000000000000000000000000000000000000000000000000000000000006",
        ));

        assert_eq!(draft.connect_secret.as_deref(), Some("secret"));
        assert_eq!(draft.requested_permissions, requested_permissions);

        let no_secret = RadrootsNostrSignerConnectProposal {
            client_public_key: public_key(
                "0000000000000000000000000000000000000000000000000000000000000007",
            ),
            connect_secret: None,
            requested_permissions: RadrootsNostrConnectPermissions::default(),
        }
        .into_connection_draft(public_identity(
            "0000000000000000000000000000000000000000000000000000000000000008",
        ));
        assert!(no_secret.connect_secret.is_none());
    }

    #[test]
    fn request_action_audit_message_and_denied_reason_cover_variants() {
        let denied = RadrootsNostrSignerRequestAction::Denied {
            reason: "unauthorized".into(),
        };
        let challenged = RadrootsNostrSignerRequestAction::Challenged {
            auth_challenge: crate::model::RadrootsNostrSignerAuthChallenge::new(
                "https://auth.example",
                1,
            )
            .expect("challenge"),
            pending_request: crate::model::RadrootsNostrSignerPendingRequest::new(
                radroots_nostr_connect::prelude::RadrootsNostrConnectRequestMessage::new(
                    "req-1",
                    RadrootsNostrConnectRequest::Ping,
                ),
                1,
            )
            .expect("pending"),
        };
        let evaluation = RadrootsNostrSignerRequestEvaluation {
            request_id: RadrootsNostrSignerRequestId::new_v7(),
            method: RadrootsNostrConnectMethod::Ping,
            connection: connection(),
            audit: crate::model::RadrootsNostrSignerRequestAuditRecord::new(
                RadrootsNostrSignerRequestId::new_v7(),
                crate::model::RadrootsNostrSignerConnectionId::new_v7(),
                RadrootsNostrConnectMethod::Ping,
                crate::model::RadrootsNostrSignerRequestDecision::Denied,
                Some("unauthorized".into()),
                1,
            ),
            action: denied.clone(),
        };

        assert_eq!(denied.audit_message().as_deref(), Some("unauthorized"));
        assert_eq!(
            challenged.audit_message().as_deref(),
            Some("auth challenge required")
        );
        assert_eq!(evaluation.denied_reason(), Some("unauthorized"));
        assert_action_audit_message_none(&RadrootsNostrSignerRequestAction::Allowed {
            required_permission: None,
            response_hint: RadrootsNostrSignerRequestResponseHint::None,
        });
    }

    #[test]
    fn request_permission_matching_covers_generic_and_sign_event_forms() {
        let kind_one = unsigned_event(1);
        let kind_two = unsigned_event(2);
        let sign_kind = RadrootsNostrConnectPermission::with_parameter(
            RadrootsNostrConnectMethod::SignEvent,
            "kind:1",
        );
        let sign_numeric = RadrootsNostrConnectPermission::with_parameter(
            RadrootsNostrConnectMethod::SignEvent,
            "1",
        );
        let sign_all = RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::SignEvent);
        let nip44 = RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip44Encrypt);

        assert!(request_allowed_by_permissions(
            &vec![sign_kind.clone()].into(),
            &RadrootsNostrConnectRequest::SignEvent(kind_one.clone()),
        ));
        assert!(request_allowed_by_permissions(
            &vec![sign_numeric].into(),
            &RadrootsNostrConnectRequest::SignEvent(kind_one),
        ));
        assert!(request_allowed_by_permissions(
            &vec![sign_all].into(),
            &RadrootsNostrConnectRequest::SignEvent(kind_two),
        ));
        assert!(!request_allowed_by_permissions(
            &vec![sign_kind, nip44].into(),
            &RadrootsNostrConnectRequest::Nip04Encrypt {
                public_key: public_key(
                    "0000000000000000000000000000000000000000000000000000000000000007",
                ),
                plaintext: "hello".into(),
            },
        ));
        assert!(request_allowed_by_permissions(
            &RadrootsNostrConnectPermissions::default(),
            &RadrootsNostrConnectRequest::Ping,
        ));
        assert!(!request_allowed_by_permissions(
            &vec![RadrootsNostrConnectPermission::with_parameter(
                RadrootsNostrConnectMethod::Custom("do_thing".into()),
                "scoped",
            )]
            .into(),
            &RadrootsNostrConnectRequest::Custom {
                method: RadrootsNostrConnectMethod::Custom("do_thing".into()),
                params: vec!["value".into()],
            },
        ));
        assert!(permission_matches(
            &RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip04Encrypt),
            &RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip04Encrypt),
        ));
        assert!(permission_matches(
            &RadrootsNostrConnectPermission::with_parameter(
                RadrootsNostrConnectMethod::Custom("scoped".into()),
                "alpha",
            ),
            &RadrootsNostrConnectPermission::with_parameter(
                RadrootsNostrConnectMethod::Custom("scoped".into()),
                "alpha",
            ),
        ));
    }

    #[test]
    fn required_permission_and_response_hint_cover_request_variants() {
        let connection = connection();
        let public_key =
            public_key("0000000000000000000000000000000000000000000000000000000000000008");
        let connect = RadrootsNostrConnectRequest::Connect {
            remote_signer_public_key: public_key,
            secret: Some("secret".into()),
            requested_permissions: RadrootsNostrConnectPermissions::default(),
        };
        let ping = RadrootsNostrConnectRequest::Ping;
        let get_public_key = RadrootsNostrConnectRequest::GetPublicKey;
        let switch_relays = RadrootsNostrConnectRequest::SwitchRelays;
        let sign_event = RadrootsNostrConnectRequest::SignEvent(unsigned_event(7));
        let custom = RadrootsNostrConnectRequest::Custom {
            method: RadrootsNostrConnectMethod::Custom("do_thing".into()),
            params: vec!["a".into()],
        };

        assert!(required_permission_for_request(&connect).is_none());
        assert!(required_permission_for_request(&ping).is_none());
        assert!(required_permission_for_request(&get_public_key).is_none());
        assert_eq!(
            required_permission_for_request(&RadrootsNostrConnectRequest::Nip04Decrypt {
                public_key,
                ciphertext: "cipher".into(),
            })
            .expect("nip04 decrypt permission")
            .to_string(),
            "nip04_decrypt"
        );
        assert_eq!(
            required_permission_for_request(&RadrootsNostrConnectRequest::Nip44Encrypt {
                public_key,
                plaintext: "hello".into(),
            })
            .expect("nip44 encrypt permission")
            .to_string(),
            "nip44_encrypt"
        );
        assert_eq!(
            required_permission_for_request(&RadrootsNostrConnectRequest::Nip44Decrypt {
                public_key,
                ciphertext: "cipher".into(),
            })
            .expect("nip44 decrypt permission")
            .to_string(),
            "nip44_decrypt"
        );
        assert_eq!(
            required_permission_for_request(&switch_relays)
                .expect("switch relays permission")
                .to_string(),
            "switch_relays"
        );
        assert_eq!(
            required_permission_for_request(&sign_event)
                .expect("sign_event permission")
                .to_string(),
            "sign_event:kind:7"
        );
        assert_eq!(
            required_permission_for_request(&custom)
                .expect("custom permission")
                .to_string(),
            "do_thing"
        );

        assert_response_hint_none(
            response_hint_for_request(
                &connection,
                &RadrootsNostrConnectRequest::Nip04Decrypt {
                    public_key,
                    ciphertext: "cipher".into(),
                },
            )
            .expect("nip04 response hint"),
        );
        assert_response_hint_pong(
            response_hint_for_request(&connection, &ping).expect("ping hint"),
        );
        assert_response_hint_user_public_key(
            response_hint_for_request(&connection, &get_public_key).expect("pubkey hint"),
        );
        assert_eq!(
            response_hint_for_request(&connection, &switch_relays).expect("relay hint"),
            RadrootsNostrSignerRequestResponseHint::RelayList(vec![relay("wss://relay.example")])
        );
    }

    #[test]
    fn invalid_identity_public_key_returns_invalid_state() {
        let mut identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000009");
        identity.public_key_hex = "invalid".into();

        let err = identity_public_key(&identity).expect_err("invalid identity");
        assert!(
            err.to_string()
                .contains("user identity public key is invalid")
        );

        let mut invalid_connection = connection();
        invalid_connection.user_identity.public_key_hex = "invalid".into();
        let err = response_hint_for_request(
            &invalid_connection,
            &RadrootsNostrConnectRequest::GetPublicKey,
        )
        .expect_err("invalid get_public_key response hint");
        assert!(
            err.to_string()
                .contains("user identity public key is invalid")
        );
    }
}
