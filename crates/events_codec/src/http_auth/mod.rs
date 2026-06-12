pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::{http_auth::RadrootsHttpAuth, kinds::KIND_POST};

    use crate::error::{EventEncodeError, EventParseError};
    use crate::http_auth::decode::http_auth_from_event;
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
    }

    fn tag(key: &str, value: &str) -> Vec<String> {
        vec![key.to_string(), value.to_string()]
    }
}
