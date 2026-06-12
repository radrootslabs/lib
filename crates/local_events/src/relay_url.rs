#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayUrlValidationError {
    Empty,
    UnsupportedScheme(String),
    MissingHost(String),
    InvalidAuthority(String),
    InvalidPort(String),
}

impl fmt::Display for RelayUrlValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("relay url must not be empty"),
            Self::UnsupportedScheme(value) => {
                write!(f, "relay url must use ws or wss, got `{value}`")
            }
            Self::MissingHost(value) => write!(f, "relay url must include a host, got `{value}`"),
            Self::InvalidAuthority(value) => {
                write!(f, "relay url authority is invalid, got `{value}`")
            }
            Self::InvalidPort(value) => write!(f, "relay url port is invalid, got `{value}`"),
        }
    }
}

impl std::error::Error for RelayUrlValidationError {}

pub fn normalize_relay_url(value: &str) -> Result<String, RelayUrlValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RelayUrlValidationError::Empty);
    }

    let rest = if let Some(rest) = trimmed.strip_prefix("ws://") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("wss://") {
        rest
    } else {
        return Err(RelayUrlValidationError::UnsupportedScheme(
            trimmed.to_owned(),
        ));
    };

    validate_relay_authority(trimmed, rest)?;
    Ok(trimmed.to_owned())
}

pub fn normalize_relay_urls<I, S>(values: I) -> Result<Vec<String>, RelayUrlValidationError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized = Vec::new();
    for value in values {
        let relay = normalize_relay_url(value.as_ref())?;
        if !normalized.iter().any(|existing| existing == &relay) {
            normalized.push(relay);
        }
    }
    Ok(normalized)
}

fn validate_relay_authority(original: &str, rest: &str) -> Result<(), RelayUrlValidationError> {
    let authority_end = rest
        .char_indices()
        .find(|(_, ch)| matches!(ch, '/' | '?' | '#'))
        .map(|(index, _)| index)
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];

    if authority.is_empty() {
        return Err(RelayUrlValidationError::MissingHost(original.to_owned()));
    }
    if authority.chars().any(char::is_whitespace) || authority.contains('@') {
        return Err(RelayUrlValidationError::InvalidAuthority(
            original.to_owned(),
        ));
    }

    if let Some(after_open) = authority.strip_prefix('[') {
        let Some(close_index) = after_open.find(']') else {
            return Err(RelayUrlValidationError::InvalidAuthority(
                original.to_owned(),
            ));
        };
        let host = &after_open[..close_index];
        let after_host = &after_open[close_index + 1..];
        if host.is_empty() {
            return Err(RelayUrlValidationError::MissingHost(original.to_owned()));
        }
        validate_optional_port(original, after_host)?;
        return Ok(());
    }

    let colon_count = authority.bytes().filter(|byte| *byte == b':').count();
    match colon_count {
        0 => {
            if authority.is_empty() {
                Err(RelayUrlValidationError::MissingHost(original.to_owned()))
            } else {
                Ok(())
            }
        }
        1 => {
            let Some((host, port)) = authority.split_once(':') else {
                return Err(RelayUrlValidationError::InvalidAuthority(
                    original.to_owned(),
                ));
            };
            if host.is_empty() {
                return Err(RelayUrlValidationError::MissingHost(original.to_owned()));
            }
            validate_port(original, port)
        }
        _ => Err(RelayUrlValidationError::InvalidAuthority(
            original.to_owned(),
        )),
    }
}

fn validate_optional_port(original: &str, after_host: &str) -> Result<(), RelayUrlValidationError> {
    if after_host.is_empty() {
        return Ok(());
    }
    let Some(port) = after_host.strip_prefix(':') else {
        return Err(RelayUrlValidationError::InvalidAuthority(
            original.to_owned(),
        ));
    };
    validate_port(original, port)
}

fn validate_port(original: &str, port: &str) -> Result<(), RelayUrlValidationError> {
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RelayUrlValidationError::InvalidPort(original.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_all_validation_errors() {
        assert_eq!(
            RelayUrlValidationError::Empty.to_string(),
            "relay url must not be empty"
        );
        assert_eq!(
            RelayUrlValidationError::UnsupportedScheme("http://relay.test".to_owned()).to_string(),
            "relay url must use ws or wss, got `http://relay.test`"
        );
        assert_eq!(
            RelayUrlValidationError::MissingHost("ws://".to_owned()).to_string(),
            "relay url must include a host, got `ws://`"
        );
        assert_eq!(
            RelayUrlValidationError::InvalidAuthority("ws://user@relay.test".to_owned())
                .to_string(),
            "relay url authority is invalid, got `ws://user@relay.test`"
        );
        assert_eq!(
            RelayUrlValidationError::InvalidPort("ws://relay.test:x".to_owned()).to_string(),
            "relay url port is invalid, got `ws://relay.test:x`"
        );
    }

    #[test]
    fn normalize_relay_url_covers_authority_edges() {
        assert_eq!(
            normalize_relay_url(" wss://relay.test:443/path?x=1#fragment ")
                .expect("normalized relay"),
            "wss://relay.test:443/path?x=1#fragment"
        );
        assert_eq!(
            normalize_relay_url("ws://[::1]:8080").expect("ipv6 relay"),
            "ws://[::1]:8080"
        );
        assert!(matches!(
            normalize_relay_url("ws://[::1]extra"),
            Err(RelayUrlValidationError::InvalidAuthority(_))
        ));
        assert!(matches!(
            normalize_relay_url("ws://relay.test:"),
            Err(RelayUrlValidationError::InvalidPort(_))
        ));
        assert!(matches!(
            normalize_relay_url("ws://relay.test:8a"),
            Err(RelayUrlValidationError::InvalidPort(_))
        ));
        assert!(matches!(
            normalize_relay_url("ws://relay one.test"),
            Err(RelayUrlValidationError::InvalidAuthority(_))
        ));
        assert!(matches!(
            normalize_relay_url("ws://relay:8080:9090"),
            Err(RelayUrlValidationError::InvalidAuthority(_))
        ));
    }

    #[test]
    fn normalize_relay_urls_dedupes_while_preserving_order() {
        let relays = normalize_relay_urls([
            "ws://relay-a.test",
            "ws://relay-b.test",
            "ws://relay-a.test",
        ])
        .expect("relay set");

        assert_eq!(relays, vec!["ws://relay-a.test", "ws://relay-b.test"]);
    }
}
