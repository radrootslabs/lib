#![forbid(unsafe_code)]

use radroots_transport::RadrootsTransportTarget;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

pub(crate) fn stable_connection_diagnostic(message: &str) -> &'static str {
    let bounded = &message.as_bytes()[..message
        .len()
        .min(radroots_transport::RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES)];
    if contains_ascii_case_insensitive(bounded, b"proxy") {
        "proxy-mode-rejected"
    } else if contains_ascii_case_insensitive(bounded, b"timeout")
        || contains_ascii_case_insensitive(bounded, b"deadline")
    {
        "connection-timeout"
    } else if contains_ascii_case_insensitive(bounded, b"dns")
        || contains_ascii_case_insensitive(bounded, b"resolv")
    {
        "dns-resolution-failed"
    } else if contains_ascii_case_insensitive(bounded, b"tls")
        || contains_ascii_case_insensitive(bounded, b"certificate")
        || contains_ascii_case_insensitive(bounded, b"handshake")
    {
        "tls-or-handshake-failed"
    } else if contains_ascii_case_insensitive(bounded, b"destination")
        || contains_ascii_case_insensitive(bounded, b"forbidden")
        || contains_ascii_case_insensitive(bounded, b"rejected")
    {
        "destination-rejected"
    } else {
        "connection-failed"
    }
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

pub(crate) fn stable_nostr_failure_summary<K, E>(failed: &HashMap<K, E>) -> String
where
    K: fmt::Display + Eq + Hash,
    E: fmt::Display,
{
    if failed.is_empty() {
        return "no-relay-acknowledged".to_owned();
    }
    let mut summaries = failed
        .iter()
        .map(|(relay, error)| {
            let relay = relay.to_string();
            let identity = RadrootsTransportTarget::nostr_relay(relay.as_str())
                .map(|target| target.fingerprint().as_str().to_owned())
                .unwrap_or_else(|_| "invalid-relay-identity".to_owned());
            format!(
                "{identity}={}",
                stable_connection_diagnostic(&error.to_string())
            )
        })
        .collect::<Vec<_>>();
    summaries.sort_unstable();
    summaries.dedup();
    summaries.truncate(radroots_transport::RADROOTS_TRANSPORT_TARGET_MAX_COUNT);
    summaries.join(";")
}

#[cfg(test)]
mod tests {
    use super::{stable_connection_diagnostic, stable_nostr_failure_summary};
    use std::collections::HashMap;

    #[test]
    fn relay_security_diagnostics_are_sorted_bounded_and_redacted() {
        let mut failures = HashMap::new();
        failures.insert(
            "wss://relay-b.example",
            "TLS certificate rejected for 10.0.0.1 with secret-token",
        );
        failures.insert(
            "wss://relay-a.example",
            "DNS resolution failed for 192.168.1.1",
        );
        let first = stable_nostr_failure_summary(&failures);
        let second = stable_nostr_failure_summary(&failures);

        assert_eq!(first, second);
        assert!(first.len() <= radroots_transport::RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES);
        assert!(!first.contains("10.0.0.1"));
        assert!(!first.contains("192.168.1.1"));
        assert!(!first.contains("secret-token"));
        assert_eq!(
            stable_connection_diagnostic("operation deadline exceeded"),
            "connection-timeout"
        );
        assert_eq!(
            stable_connection_diagnostic("proxy connection mode rejected"),
            "proxy-mode-rejected"
        );
        assert_eq!(
            stable_connection_diagnostic("TLS handshake rejected"),
            "tls-or-handshake-failed"
        );
        assert_eq!(
            stable_nostr_failure_summary::<String, String>(&HashMap::new()),
            "no-relay-acknowledged"
        );
    }
}
