#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{RadrootsNostrEventPtr, message::RadrootsMessageRecipient};

use crate::error::{EventEncodeError, EventParseError};

fn validate_recipient(recipient: &RadrootsMessageRecipient) -> Result<(), EventEncodeError> {
    if recipient.public_key.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField(
            "recipients.public_key",
        ));
    }
    if let Some(relay_url) = &recipient.relay_url {
        if relay_url.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("recipients.relay_url"));
        }
    }
    Ok(())
}

pub(crate) fn build_recipient_tags(
    recipients: &[RadrootsMessageRecipient],
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if recipients.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("recipients"));
    }

    let mut tags = Vec::with_capacity(recipients.len());
    for recipient in recipients {
        validate_recipient(recipient)?;
        let mut tag = Vec::with_capacity(3);
        tag.push("p".to_string());
        tag.push(recipient.public_key.clone());
        if let Some(relay_url) = &recipient.relay_url {
            tag.push(relay_url.clone());
        }
        tags.push(tag);
    }
    Ok(tags)
}

pub(crate) fn build_reply_tag(
    reply_to: &Option<RadrootsNostrEventPtr>,
) -> Result<Option<Vec<String>>, EventEncodeError> {
    let reply_to = match reply_to {
        Some(reply_to) => reply_to,
        None => return Ok(None),
    };
    if reply_to.id.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("reply_to.id"));
    }
    let mut tag = Vec::with_capacity(3);
    tag.push("e".to_string());
    tag.push(reply_to.id.clone());
    if let Some(relay) = &reply_to.relays {
        if relay.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("reply_to.relays"));
        }
        tag.push(relay.clone());
    }
    Ok(Some(tag))
}

pub(crate) fn build_subject_tag(
    subject: &Option<String>,
) -> Result<Option<Vec<String>>, EventEncodeError> {
    let subject = match subject {
        Some(subject) => subject,
        None => return Ok(None),
    };
    if subject.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("subject"));
    }
    Ok(Some(vec!["subject".to_string(), subject.clone()]))
}

fn parse_recipient_tag(tag: &[String]) -> Result<RadrootsMessageRecipient, EventParseError> {
    let public_key = tag.get(1).ok_or(EventParseError::InvalidTag("p"))?;
    if public_key.trim().is_empty() {
        return Err(EventParseError::InvalidTag("p"));
    }
    let relay_url = match tag.get(2) {
        Some(value) if value.trim().is_empty() => return Err(EventParseError::InvalidTag("p")),
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(RadrootsMessageRecipient {
        public_key: public_key.clone(),
        relay_url,
    })
}

pub(crate) fn parse_recipients(
    tags: &[Vec<String>],
) -> Result<Vec<RadrootsMessageRecipient>, EventParseError> {
    let mut recipients = Vec::new();
    for tag in tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some("p"))
    {
        recipients.push(parse_recipient_tag(tag)?);
    }
    if recipients.is_empty() {
        return Err(EventParseError::MissingTag("p"));
    }
    Ok(recipients)
}

pub(crate) fn parse_reply_tag(
    tags: &[Vec<String>],
) -> Result<Option<RadrootsNostrEventPtr>, EventParseError> {
    let tag = match tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("e"))
    {
        Some(tag) => tag,
        None => return Ok(None),
    };
    let id = tag.get(1).ok_or(EventParseError::InvalidTag("e"))?;
    if id.trim().is_empty() {
        return Err(EventParseError::InvalidTag("e"));
    }
    let relay = match tag.get(2) {
        Some(value) if value.trim().is_empty() => return Err(EventParseError::InvalidTag("e")),
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(Some(RadrootsNostrEventPtr {
        id: id.clone(),
        relays: relay,
    }))
}

pub(crate) fn parse_subject_tag(tags: &[Vec<String>]) -> Result<Option<String>, EventParseError> {
    let tag = match tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("subject"))
    {
        Some(tag) => tag,
        None => return Ok(None),
    };
    let subject = tag.get(1).ok_or(EventParseError::InvalidTag("subject"))?;
    if subject.trim().is_empty() {
        return Err(EventParseError::InvalidTag("subject"));
    }
    Ok(Some(subject.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::{RadrootsNostrEventPtr, message::RadrootsMessageRecipient};
    use radroots_test_fixtures::RELAY_PRIMARY_WSS;

    #[test]
    fn parse_recipients_rejects_missing_p_tags() {
        let err = parse_recipients(&[vec!["x".to_string(), "pub".to_string()]])
            .expect_err("expected missing recipient tag");
        assert!(matches!(err, EventParseError::MissingTag("p")));

        let err = parse_recipients(&[vec!["p".to_string(), " ".to_string()]])
            .expect_err("expected invalid recipient tag");
        assert!(matches!(err, EventParseError::InvalidTag("p")));
    }

    #[test]
    fn parse_recipient_and_reply_tag_require_id_values() {
        let err = parse_recipient_tag(&["p".to_string()]).expect_err("missing recipient pubkey");
        assert!(matches!(err, EventParseError::InvalidTag("p")));

        let err = parse_recipient_tag(&["p".to_string(), " ".to_string()])
            .expect_err("empty recipient pubkey");
        assert!(matches!(err, EventParseError::InvalidTag("p")));

        let err = parse_reply_tag(&[vec!["e".to_string()]]).expect_err("missing reply id");
        assert!(matches!(err, EventParseError::InvalidTag("e")));

        let err =
            parse_reply_tag(&[vec!["e".to_string(), " ".to_string()]]).expect_err("empty reply id");
        assert!(matches!(err, EventParseError::InvalidTag("e")));

        let err = build_reply_tag(&Some(RadrootsNostrEventPtr {
            id: " ".to_string(),
            relays: None,
        }))
        .expect_err("empty reply id");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("reply_to.id")
        ));
    }

    #[test]
    fn build_and_parse_reply_tags_cover_optional_relay_paths() {
        let tag = build_reply_tag(&Some(RadrootsNostrEventPtr {
            id: "reply".to_string(),
            relays: Some(RELAY_PRIMARY_WSS.to_string()),
        }))
        .expect("build reply tag")
        .expect("reply tag");
        assert_eq!(tag.len(), 3);
        let parsed = parse_reply_tag(&[tag]).expect("parse reply");
        assert_eq!(
            parsed.and_then(|value| value.relays),
            Some(RELAY_PRIMARY_WSS.to_string())
        );

        let tag = build_reply_tag(&Some(RadrootsNostrEventPtr {
            id: "reply".to_string(),
            relays: None,
        }))
        .expect("build reply tag")
        .expect("reply tag");
        assert_eq!(tag.len(), 2);
    }

    #[test]
    fn parse_reply_tag_handles_absent_relay() {
        let parsed = parse_reply_tag(&[vec!["e".to_string(), "reply".to_string()]])
            .expect("parse reply tag without relay");
        assert_eq!(
            parsed,
            Some(RadrootsNostrEventPtr {
                id: "reply".to_string(),
                relays: None,
            })
        );
    }

    #[test]
    fn recipient_and_subject_tag_builders_cover_error_paths() {
        let tags = build_recipient_tags(&[RadrootsMessageRecipient {
            public_key: "recipient-without-relay".to_string(),
            relay_url: None,
        }])
        .expect("recipient tag without relay");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].len(), 2);

        let tags = build_recipient_tags(&[RadrootsMessageRecipient {
            public_key: "recipient".to_string(),
            relay_url: Some(RELAY_PRIMARY_WSS.to_string()),
        }])
        .expect("recipient tag");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].len(), 3);

        let err = build_recipient_tags(&[]).expect_err("missing recipients");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("recipients")
        ));

        let err = build_recipient_tags(&[RadrootsMessageRecipient {
            public_key: " ".to_string(),
            relay_url: None,
        }])
        .expect_err("empty recipient pubkey");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("recipients.public_key")
        ));

        let err = build_recipient_tags(&[RadrootsMessageRecipient {
            public_key: "recipient".to_string(),
            relay_url: Some(" ".to_string()),
        }])
        .expect_err("empty recipient relay");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("recipients.relay_url")
        ));

        let subject = build_subject_tag(&Some("subject".to_string()))
            .expect("subject tag")
            .expect("subject present");
        assert_eq!(subject, vec!["subject".to_string(), "subject".to_string()]);
        let none = build_subject_tag(&None).expect("none subject");
        assert!(none.is_none());
        let err = build_subject_tag(&Some(" ".to_string())).expect_err("empty subject");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("subject")
        ));
    }

    #[test]
    fn recipient_reply_and_subject_parsers_cover_missing_and_invalid_tags() {
        let recipients = parse_recipients(&[
            vec!["p".to_string(), "recipient".to_string()],
            vec![
                "p".to_string(),
                "recipient-2".to_string(),
                RELAY_PRIMARY_WSS.to_string(),
            ],
        ])
        .expect("parse recipients");
        assert_eq!(recipients.len(), 2);

        let err = parse_recipients(&[vec!["e".to_string(), "reply".to_string()]])
            .expect_err("missing recipient tags");
        assert!(matches!(err, EventParseError::MissingTag("p")));

        let err = parse_recipients(&[vec![
            "p".to_string(),
            "recipient".to_string(),
            " ".to_string(),
        ]])
        .expect_err("invalid recipient relay");
        assert!(matches!(err, EventParseError::InvalidTag("p")));

        let err = build_reply_tag(&Some(RadrootsNostrEventPtr {
            id: "reply".to_string(),
            relays: Some(" ".to_string()),
        }))
        .expect_err("empty reply relay");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("reply_to.relays")
        ));

        let err = parse_reply_tag(&[vec!["e".to_string(), "reply".to_string(), " ".to_string()]])
            .expect_err("invalid reply relay");
        assert!(matches!(err, EventParseError::InvalidTag("e")));

        let subject = parse_subject_tag(&[vec!["subject".to_string(), "topic".to_string()]])
            .expect("subject tag");
        assert_eq!(subject.as_deref(), Some("topic"));

        let none = parse_subject_tag(&[vec!["p".to_string(), "recipient".to_string()]])
            .expect("subject absent");
        assert!(none.is_none());

        let err =
            parse_subject_tag(&[vec!["subject".to_string()]]).expect_err("missing subject value");
        assert!(matches!(err, EventParseError::InvalidTag("subject")));

        let err = parse_subject_tag(&[vec!["subject".to_string(), " ".to_string()]])
            .expect_err("empty subject value");
        assert!(matches!(err, EventParseError::InvalidTag("subject")));
    }
}
