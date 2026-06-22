#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use radroots_events::social::{
    RadrootsCalendarParticipant, RadrootsSocialFarmAnchor, RadrootsSocialLocation,
    RadrootsSocialMediaDimensions, RadrootsSocialMediaThumbnail,
};

use crate::error::{EventEncodeError, EventParseError};
use crate::field_helpers::{push_tag, push_tag_values, validate_non_empty_field};

pub(crate) fn validate_http_url(value: &str, field: &'static str) -> Result<(), EventEncodeError> {
    if value.starts_with("https://") || value.starts_with("http://") {
        validate_non_empty_field(value, field)
    } else {
        Err(EventEncodeError::InvalidField(field))
    }
}

pub(crate) fn validate_date(value: &str, field: &'static str) -> Result<(), EventEncodeError> {
    if is_date(value) {
        Ok(())
    } else {
        Err(EventEncodeError::InvalidField(field))
    }
}

pub(crate) fn validate_date_tag(value: &str, tag: &'static str) -> Result<(), EventParseError> {
    if is_date(value) {
        Ok(())
    } else {
        Err(EventParseError::InvalidTag(tag))
    }
}

pub(crate) fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4] == b'-'
        && bytes[5].is_ascii_digit()
        && bytes[6].is_ascii_digit()
        && bytes[7] == b'-'
        && bytes[8].is_ascii_digit()
        && bytes[9].is_ascii_digit()
}

pub(crate) fn validate_end_after_start(
    start: u64,
    end: Option<u64>,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    if end.is_some_and(|end| end < start) {
        Err(EventEncodeError::InvalidField(field))
    } else {
        Ok(())
    }
}

pub(crate) fn validate_date_end_after_start(
    start: &str,
    end: Option<&str>,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    if end.is_some_and(|end| end < start) {
        Err(EventEncodeError::InvalidField(field))
    } else {
        Ok(())
    }
}

pub(crate) fn push_location_tags(tags: &mut Vec<Vec<String>>, location: &RadrootsSocialLocation) {
    if let Some(name) = location
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_tag(tags, "location", name);
    }
    if let Some(geohash) = location
        .geohash
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_tag(tags, "g", geohash);
    }
}

pub(crate) fn location_from_tags(tags: &[Vec<String>]) -> Option<RadrootsSocialLocation> {
    let name = first_tag_value(tags, "location");
    let geohash = first_tag_value(tags, "g");
    if name.is_none() && geohash.is_none() {
        None
    } else {
        Some(RadrootsSocialLocation { name, geohash })
    }
}

pub(crate) fn push_farm_anchor(tags: &mut Vec<Vec<String>>, farm: &RadrootsSocialFarmAnchor) {
    if farm.farm.pubkey.trim().is_empty() || farm.farm.d_tag.trim().is_empty() {
        return;
    }
    let address = format!("30340:{}:{}", farm.farm.pubkey, farm.farm.d_tag);
    push_tag(tags, "a", address);
}

pub(crate) fn participants_from_tags(
    tags: &[Vec<String>],
) -> Option<Vec<RadrootsCalendarParticipant>> {
    let participants = tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some("p"))
        .filter_map(|tag| {
            let pubkey = tag.get(1)?.clone();
            if pubkey.trim().is_empty() {
                return None;
            }
            Some(RadrootsCalendarParticipant {
                pubkey,
                relay: tag.get(2).filter(|value| !value.trim().is_empty()).cloned(),
                role: tag.get(3).filter(|value| !value.trim().is_empty()).cloned(),
            })
        })
        .collect::<Vec<_>>();
    if participants.is_empty() {
        None
    } else {
        Some(participants)
    }
}

pub(crate) fn push_participants(
    tags: &mut Vec<Vec<String>>,
    participants: Option<&Vec<RadrootsCalendarParticipant>>,
) {
    let Some(participants) = participants else {
        return;
    };
    for participant in participants {
        if participant.pubkey.trim().is_empty() {
            continue;
        }
        let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
        if let Some(relay) = participant.relay.as_ref() {
            tag.push(relay.clone());
        }
        if let Some(role) = participant.role.as_ref() {
            if participant.relay.is_none() {
                tag.push(String::new());
            }
            tag.push(role.clone());
        }
        tags.push(tag);
    }
}

pub(crate) fn first_tag_value(tags: &[Vec<String>], key: &str) -> Option<String> {
    tags.iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
        .and_then(|tag| tag.get(1))
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

pub(crate) fn dimensions_tag(dimensions: &RadrootsSocialMediaDimensions) -> String {
    format!("{}x{}", dimensions.width, dimensions.height)
}

pub(crate) fn parse_dimensions_tag(
    value: &str,
    tag: &'static str,
) -> Result<RadrootsSocialMediaDimensions, EventParseError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(EventParseError::InvalidTag(tag));
    };
    let width = width
        .parse::<u32>()
        .map_err(|err| EventParseError::InvalidNumber(tag, err))?;
    let height = height
        .parse::<u32>()
        .map_err(|err| EventParseError::InvalidNumber(tag, err))?;
    if width == 0 || height == 0 {
        return Err(EventParseError::InvalidTag(tag));
    }
    Ok(RadrootsSocialMediaDimensions { width, height })
}

pub(crate) fn push_thumbnail(
    tags: &mut Vec<Vec<String>>,
    thumbnail: &RadrootsSocialMediaThumbnail,
) {
    if thumbnail.url.trim().is_empty() {
        return;
    }
    if let Some(dimensions) = thumbnail.dimensions.as_ref() {
        push_tag_values(
            tags,
            "thumb",
            [thumbnail.url.clone(), dimensions_tag(dimensions)],
        );
    } else {
        push_tag(tags, "thumb", thumbnail.url.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_dates_and_ordered_time_ranges() {
        assert!(is_date("2026-06-20"));
        assert!(!is_date("2026-6-20"));
        for invalid in [
            "x026-06-20",
            "2x26-06-20",
            "20x6-06-20",
            "202x-06-20",
            "2026/06-20",
            "2026-x6-20",
            "2026-0x-20",
            "2026-06/20",
            "2026-06-x0",
            "2026-06-2x",
        ] {
            assert!(!is_date(invalid));
        }
        assert!(validate_http_url("https://example.test/file", "url").is_ok());
        assert!(validate_http_url("http://example.test/file", "url").is_ok());
        assert!(matches!(
            validate_http_url("ftp://example.test/file", "url"),
            Err(EventEncodeError::InvalidField("url"))
        ));
        assert!(validate_date("2026-06-20", "date").is_ok());
        assert!(matches!(
            validate_date("bad", "date"),
            Err(EventEncodeError::InvalidField("date"))
        ));
        assert!(validate_date_tag("2026-06-20", "start").is_ok());
        assert!(validate_end_after_start(10, Some(10), "end").is_ok());
        assert!(validate_end_after_start(10, None, "end").is_ok());
        assert!(matches!(
            validate_end_after_start(10, Some(9), "end"),
            Err(EventEncodeError::InvalidField("end"))
        ));
        assert!(validate_date_end_after_start("2026-06-20", None, "end").is_ok());
        assert!(validate_date_end_after_start("2026-06-20", Some("2026-06-20"), "end").is_ok());
        assert!(matches!(
            validate_date_end_after_start("2026-06-20", Some("2026-06-19"), "end"),
            Err(EventEncodeError::InvalidField("end"))
        ));
        assert!(matches!(
            validate_date_tag("bad", "start"),
            Err(EventParseError::InvalidTag("start"))
        ));
    }

    #[test]
    fn encodes_and_decodes_location_participant_and_dimensions_tags() {
        let mut tags = Vec::new();
        push_location_tags(
            &mut tags,
            &RadrootsSocialLocation {
                name: Some("Pack shed".to_string()),
                geohash: Some("c23nb62w20st".to_string()),
            },
        );
        push_participants(
            &mut tags,
            Some(&vec![RadrootsCalendarParticipant {
                pubkey: "crew_pubkey".to_string(),
                relay: None,
                role: Some("participant".to_string()),
            }]),
        );

        let location = location_from_tags(&tags).expect("location");
        assert_eq!(location.name.as_deref(), Some("Pack shed"));
        assert_eq!(location.geohash.as_deref(), Some("c23nb62w20st"));
        let named_location =
            location_from_tags(&[vec!["location".to_string(), "Farm gate".to_string()]])
                .expect("named location");
        assert_eq!(named_location.name.as_deref(), Some("Farm gate"));
        assert_eq!(named_location.geohash, None);
        let geohash_location =
            location_from_tags(&[vec!["g".to_string(), "c23nb62w20st".to_string()]])
                .expect("geohash location");
        assert_eq!(geohash_location.name, None);
        assert_eq!(geohash_location.geohash.as_deref(), Some("c23nb62w20st"));
        let participants = participants_from_tags(&tags).expect("participants");
        assert_eq!(participants[0].pubkey, "crew_pubkey");
        assert_eq!(participants[0].role.as_deref(), Some("participant"));

        let mut empty_tags = Vec::new();
        push_location_tags(
            &mut empty_tags,
            &RadrootsSocialLocation {
                name: Some(" ".to_string()),
                geohash: Some(" ".to_string()),
            },
        );
        assert!(empty_tags.is_empty());
        assert_eq!(location_from_tags(&empty_tags), None);
        assert_eq!(
            first_tag_value(&[vec!["location".to_string()]], "location"),
            None
        );
        assert_eq!(
            first_tag_value(&[vec!["location".to_string(), " ".to_string()]], "location"),
            None
        );

        let mut anchor_tags = Vec::new();
        push_farm_anchor(
            &mut anchor_tags,
            &RadrootsSocialFarmAnchor {
                farm: radroots_events::farm::RadrootsFarmRef {
                    pubkey: " ".to_string(),
                    d_tag: "farm-d-tag".to_string(),
                },
                relays: None,
            },
        );
        push_farm_anchor(
            &mut anchor_tags,
            &RadrootsSocialFarmAnchor {
                farm: radroots_events::farm::RadrootsFarmRef {
                    pubkey: "farm_pubkey".to_string(),
                    d_tag: " ".to_string(),
                },
                relays: None,
            },
        );
        push_farm_anchor(
            &mut anchor_tags,
            &RadrootsSocialFarmAnchor {
                farm: radroots_events::farm::RadrootsFarmRef {
                    pubkey: "farm_pubkey".to_string(),
                    d_tag: "farm-d-tag".to_string(),
                },
                relays: None,
            },
        );
        assert_eq!(
            anchor_tags,
            vec![vec![
                "a".to_string(),
                "30340:farm_pubkey:farm-d-tag".to_string()
            ]]
        );

        assert_eq!(participants_from_tags(&[]), None);
        let participants = participants_from_tags(&[
            vec!["p".to_string()],
            vec!["p".to_string(), " ".to_string()],
            vec![
                "p".to_string(),
                "crew_pubkey".to_string(),
                "wss://relay.example.test".to_string(),
                "host".to_string(),
            ],
        ])
        .expect("participants");
        assert_eq!(participants.len(), 1);
        assert_eq!(
            participants[0].relay.as_deref(),
            Some("wss://relay.example.test")
        );
        assert_eq!(participants[0].role.as_deref(), Some("host"));

        let mut participant_tags = Vec::new();
        push_participants(&mut participant_tags, None);
        push_participants(
            &mut participant_tags,
            Some(&vec![
                RadrootsCalendarParticipant {
                    pubkey: " ".to_string(),
                    relay: None,
                    role: None,
                },
                RadrootsCalendarParticipant {
                    pubkey: "crew_pubkey".to_string(),
                    relay: Some("wss://relay.example.test".to_string()),
                    role: Some("host".to_string()),
                },
                RadrootsCalendarParticipant {
                    pubkey: "relay_only_pubkey".to_string(),
                    relay: Some("wss://relay.example.test".to_string()),
                    role: None,
                },
            ]),
        );
        assert_eq!(
            participant_tags,
            vec![
                vec![
                    "p".to_string(),
                    "crew_pubkey".to_string(),
                    "wss://relay.example.test".to_string(),
                    "host".to_string()
                ],
                vec![
                    "p".to_string(),
                    "relay_only_pubkey".to_string(),
                    "wss://relay.example.test".to_string()
                ]
            ]
        );

        let dimensions = parse_dimensions_tag("1200x800", "dim").unwrap();
        assert_eq!(dimensions_tag(&dimensions), "1200x800");
        assert!(matches!(
            parse_dimensions_tag("0x800", "dim"),
            Err(EventParseError::InvalidTag("dim"))
        ));
        assert!(matches!(
            parse_dimensions_tag("1200x0", "dim"),
            Err(EventParseError::InvalidTag("dim"))
        ));
        assert!(matches!(
            parse_dimensions_tag("badx800", "dim"),
            Err(EventParseError::InvalidNumber("dim", _))
        ));
        assert!(matches!(
            parse_dimensions_tag("1200xbad", "dim"),
            Err(EventParseError::InvalidNumber("dim", _))
        ));

        let mut thumbnail_tags = Vec::new();
        push_thumbnail(
            &mut thumbnail_tags,
            &RadrootsSocialMediaThumbnail {
                url: " ".to_string(),
                dimensions: None,
            },
        );
        push_thumbnail(
            &mut thumbnail_tags,
            &RadrootsSocialMediaThumbnail {
                url: "https://media.example.test/thumb.jpg".to_string(),
                dimensions: None,
            },
        );
        push_thumbnail(
            &mut thumbnail_tags,
            &RadrootsSocialMediaThumbnail {
                url: "https://media.example.test/thumb-large.jpg".to_string(),
                dimensions: Some(RadrootsSocialMediaDimensions {
                    width: 320,
                    height: 240,
                }),
            },
        );
        assert_eq!(
            thumbnail_tags,
            vec![
                vec![
                    "thumb".to_string(),
                    "https://media.example.test/thumb.jpg".to_string()
                ],
                vec![
                    "thumb".to_string(),
                    "https://media.example.test/thumb-large.jpg".to_string(),
                    "320x240".to_string()
                ],
            ]
        );
    }
}
