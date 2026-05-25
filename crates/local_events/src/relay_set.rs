#![forbid(unsafe_code)]

use std::collections::BTreeSet;

const FNV_1A_64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_1A_64_PRIME: u64 = 0x100000001b3;

pub const CANONICAL_RELAY_SET_FINGERPRINT_VERSION: &str = "radroots-local-events-relay-set-v1";

/// Returns the canonical shared local-event relay-set fingerprint.
///
/// Relay URLs are trimmed, blank entries are discarded, duplicates are removed,
/// and the remaining set is sorted before hashing.
pub fn canonical_relay_set_fingerprint<I, S>(relay_urls: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let relays = relay_urls
        .into_iter()
        .filter_map(|relay_url| {
            let relay_url = relay_url.as_ref().trim();
            (!relay_url.is_empty()).then(|| relay_url.to_owned())
        })
        .collect::<BTreeSet<_>>();

    if relays.is_empty() {
        return None;
    }

    let mut hash = FNV_1A_64_OFFSET_BASIS;
    for relay in relays {
        update_hash(&mut hash, relay.len().to_string().as_bytes());
        update_hash(&mut hash, &[0]);
        update_hash(&mut hash, relay.as_bytes());
        update_hash(&mut hash, &[0]);
    }

    Some(format!(
        "{CANONICAL_RELAY_SET_FINGERPRINT_VERSION}:{hash:016x}"
    ))
}

fn update_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_1A_64_PRIME);
    }
}
