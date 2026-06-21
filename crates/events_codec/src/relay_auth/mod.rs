pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::{kinds::KIND_POST, relay_auth::RadrootsRelayAuth};

    use crate::error::{EventEncodeError, EventParseError};
    use crate::relay_auth::decode::{data_from_event, parsed_from_event, relay_auth_from_event};
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
        let without_relay = parts
            .tags
            .iter()
            .filter(|tag| tag.first().map(|value| value.as_str()) != Some("relay"))
            .cloned()
            .collect::<Vec<_>>();
        let missing_relay =
            relay_auth_from_event(parts.kind, &without_relay, &parts.content).unwrap_err();
        assert!(matches!(
            missing_relay,
            EventParseError::MissingTag("relay")
        ));

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

        let decode_wrong_kind = relay_auth_from_event(KIND_POST, &parts.tags, "").unwrap_err();
        assert!(matches!(
            decode_wrong_kind,
            EventParseError::InvalidKind {
                expected: "22242",
                got: KIND_POST
            }
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

        let mut auth = sample_auth();
        auth.challenge.clear();
        let challenge_err = relay_auth_build_tags(&auth).unwrap_err();
        assert!(matches!(
            challenge_err,
            EventEncodeError::EmptyRequiredField("challenge")
        ));

        let parts = to_wire_parts(&sample_auth()).expect("relay auth wire parts");
        let mut empty_relay = parts.tags.clone();
        replace_tag_value(&mut empty_relay, "relay", " ");
        let relay_tag_err = relay_auth_from_event(parts.kind, &empty_relay, "").unwrap_err();
        assert!(matches!(
            relay_tag_err,
            EventParseError::InvalidTag("relay")
        ));
    }

    #[test]
    fn relay_auth_wrappers_preserve_event_metadata() {
        let auth = sample_auth();
        let parts = to_wire_parts(&auth).expect("relay auth wire parts");

        let data = data_from_event(
            "event-id".to_string(),
            "author-pubkey".to_string(),
            99,
            parts.kind,
            parts.content.clone(),
            parts.tags.clone(),
        )
        .expect("parsed data");
        assert_eq!(data.id, "event-id");
        assert_eq!(data.author, "author-pubkey");
        assert_eq!(data.published_at, 99);
        assert_eq!(data.data, auth);

        let parsed = parsed_from_event(
            "event-id".to_string(),
            "author-pubkey".to_string(),
            99,
            parts.kind,
            parts.content,
            parts.tags,
            "sig".to_string(),
        )
        .expect("parsed event");
        assert_eq!(parsed.event.sig, "sig");
        assert_eq!(parsed.data.data, auth);
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

    fn replace_tag_value(tags: &mut [Vec<String>], key: &str, value: &str) {
        let tag = tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(key))
            .expect("tag");
        tag[1] = value.to_string();
    }
}
