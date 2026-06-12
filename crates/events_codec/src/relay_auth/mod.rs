pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::{kinds::KIND_POST, relay_auth::RadrootsRelayAuth};

    use crate::error::{EventEncodeError, EventParseError};
    use crate::relay_auth::decode::relay_auth_from_event;
    use crate::relay_auth::encode::{
        relay_auth_build_tags, to_wire_parts, to_wire_parts_with_kind,
    };

    #[test]
    fn relay_auth_encodes_and_decodes_nip42_event() {
        let auth = sample_auth();
        let parts = to_wire_parts(&auth).expect("relay auth wire parts");

        assert_eq!(parts.kind, 22242);
        assert_eq!(parts.content, "");
        assert!(parts.tags.contains(&tag("relay", auth.relay.as_str())));
        assert!(
            parts
                .tags
                .contains(&tag("challenge", auth.challenge.as_str()))
        );

        let decoded =
            relay_auth_from_event(parts.kind, &parts.tags, &parts.content).expect("decode");
        assert_eq!(decoded, auth);
    }

    #[test]
    fn relay_auth_rejects_missing_challenge_non_empty_content_and_wrong_kind() {
        let parts = to_wire_parts(&sample_auth()).expect("relay auth wire parts");
        let without_challenge = parts
            .tags
            .iter()
            .filter(|tag| tag.first().map(|value| value.as_str()) != Some("challenge"))
            .cloned()
            .collect::<Vec<_>>();
        let missing =
            relay_auth_from_event(parts.kind, &without_challenge, &parts.content).unwrap_err();
        assert!(matches!(missing, EventParseError::MissingTag("challenge")));

        let content_err = relay_auth_from_event(parts.kind, &parts.tags, "not empty").unwrap_err();
        assert!(matches!(
            content_err,
            EventParseError::InvalidJson("content")
        ));

        let wrong_kind = to_wire_parts_with_kind(&sample_auth(), KIND_POST).unwrap_err();
        assert!(matches!(
            wrong_kind,
            EventEncodeError::InvalidKind(KIND_POST)
        ));
    }

    #[test]
    fn relay_auth_rejects_empty_required_fields() {
        let mut auth = sample_auth();
        auth.relay.clear();
        let relay_err = relay_auth_build_tags(&auth).unwrap_err();
        assert!(matches!(
            relay_err,
            EventEncodeError::EmptyRequiredField("relay")
        ));
    }

    fn sample_auth() -> RadrootsRelayAuth {
        RadrootsRelayAuth {
            relay: "wss://relay.example.invalid/farm/field-group".to_string(),
            challenge: "relay-provided-challenge".to_string(),
        }
    }

    fn tag(key: &str, value: &str) -> Vec<String> {
        vec![key.to_string(), value.to_string()]
    }
}
