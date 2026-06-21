pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::{http_auth::RadrootsHttpAuth, kinds::KIND_POST};

    use crate::error::{EventEncodeError, EventParseError};
    use crate::http_auth::decode::{data_from_event, http_auth_from_event, parsed_from_event};
    use crate::http_auth::encode::{http_auth_build_tags, to_wire_parts, to_wire_parts_with_kind};

    const PAYLOAD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn http_auth_encodes_and_decodes_get_without_payload() {
        let auth = RadrootsHttpAuth {
            url: "https://media.example.invalid/download".to_string(),
            method: "GET".to_string(),
            payload_sha256: None,
        };
        let parts = to_wire_parts(&auth).expect("http auth wire parts");

        assert_eq!(parts.kind, 27235);
        assert_eq!(parts.content, "");
        assert!(parts.tags.contains(&tag("u", auth.url.as_str())));
        assert!(parts.tags.contains(&tag("method", "GET")));

        let decoded =
            http_auth_from_event(parts.kind, &parts.tags, &parts.content).expect("decode");
        assert_eq!(decoded, auth);
    }

    #[test]
    fn http_auth_encodes_and_decodes_post_with_payload() {
        let auth = RadrootsHttpAuth {
            url: "https://media.example.invalid/upload".to_string(),
            method: "POST".to_string(),
            payload_sha256: Some(PAYLOAD.to_string()),
        };
        let parts = to_wire_parts(&auth).expect("http auth wire parts");

        assert!(parts.tags.contains(&tag("payload", PAYLOAD)));
        let decoded =
            http_auth_from_event(parts.kind, &parts.tags, &parts.content).expect("decode");
        assert_eq!(decoded.payload_sha256.as_deref(), Some(PAYLOAD));
    }

    #[test]
    fn http_auth_rejects_missing_url_missing_method_bad_payload_and_content() {
        let auth = RadrootsHttpAuth {
            url: "https://media.example.invalid/upload".to_string(),
            method: "POST".to_string(),
            payload_sha256: Some(PAYLOAD.to_string()),
        };
        let parts = to_wire_parts(&auth).expect("http auth wire parts");
        let without_url = parts
            .tags
            .iter()
            .filter(|tag| tag.first().map(|value| value.as_str()) != Some("u"))
            .cloned()
            .collect::<Vec<_>>();
        let missing_url = http_auth_from_event(parts.kind, &without_url, "").unwrap_err();
        assert!(matches!(missing_url, EventParseError::MissingTag("u")));

        let without_method = parts
            .tags
            .iter()
            .filter(|tag| tag.first().map(|value| value.as_str()) != Some("method"))
            .cloned()
            .collect::<Vec<_>>();
        let missing_method = http_auth_from_event(parts.kind, &without_method, "").unwrap_err();
        assert!(matches!(
            missing_method,
            EventParseError::MissingTag("method")
        ));

        let mut bad_payload = auth.clone();
        bad_payload.payload_sha256 = Some("ABC".to_string());
        let payload_err = http_auth_build_tags(&bad_payload).unwrap_err();
        assert!(matches!(
            payload_err,
            EventEncodeError::InvalidField("payload_sha256")
        ));

        let mut empty_url = auth.clone();
        empty_url.url.clear();
        let url_err = http_auth_build_tags(&empty_url).unwrap_err();
        assert!(matches!(
            url_err,
            EventEncodeError::EmptyRequiredField("url")
        ));

        let mut empty_method = auth.clone();
        empty_method.method.clear();
        let method_err = http_auth_build_tags(&empty_method).unwrap_err();
        assert!(matches!(
            method_err,
            EventEncodeError::EmptyRequiredField("method")
        ));

        let mut bad_payload_tag = parts.tags.clone();
        replace_tag_value(&mut bad_payload_tag, "payload", "ABC");
        let payload_tag_err = http_auth_from_event(parts.kind, &bad_payload_tag, "").unwrap_err();
        assert!(matches!(
            payload_tag_err,
            EventParseError::InvalidTag("payload")
        ));

        let mut invalid_url = parts.tags.clone();
        replace_tag_value(&mut invalid_url, "u", " ");
        let url_tag_err = http_auth_from_event(parts.kind, &invalid_url, "").unwrap_err();
        assert!(matches!(url_tag_err, EventParseError::InvalidTag("u")));

        let content_err = http_auth_from_event(parts.kind, &parts.tags, "not empty").unwrap_err();
        assert!(matches!(
            content_err,
            EventParseError::InvalidJson("content")
        ));
    }

    #[test]
    fn http_auth_rejects_wrong_kind() {
        let auth = RadrootsHttpAuth {
            url: "https://media.example.invalid/upload".to_string(),
            method: "POST".to_string(),
            payload_sha256: Some(PAYLOAD.to_string()),
        };
        let wrong_kind = to_wire_parts_with_kind(&auth, KIND_POST).unwrap_err();
        assert!(matches!(
            wrong_kind,
            EventEncodeError::InvalidKind(KIND_POST)
        ));

        let parts = to_wire_parts(&auth).expect("http auth wire parts");
        let decode_wrong_kind = http_auth_from_event(KIND_POST, &parts.tags, "").unwrap_err();
        assert!(matches!(
            decode_wrong_kind,
            EventParseError::InvalidKind {
                expected: "27235",
                got: KIND_POST
            }
        ));
    }

    #[test]
    fn http_auth_wrappers_preserve_event_metadata() {
        let auth = RadrootsHttpAuth {
            url: "https://media.example.invalid/upload".to_string(),
            method: "POST".to_string(),
            payload_sha256: Some(PAYLOAD.to_string()),
        };
        let parts = to_wire_parts(&auth).expect("http auth wire parts");

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
