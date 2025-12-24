use crate::types::{RadrootsNostrFromBech32, RadrootsNostrPublicKey};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid pubkey format: {0}")]
    Invalid(String),
}

pub fn radroots_nostr_parse_pubkey(s: &str) -> Result<RadrootsNostrPublicKey, ParseError> {
    RadrootsNostrPublicKey::from_bech32(s)
        .or_else(|_| RadrootsNostrPublicKey::from_hex(s))
        .map_err(|_| ParseError::Invalid(s.to_string()))
}

pub fn radroots_nostr_parse_pubkeys(
    input: &[String],
) -> Result<Vec<RadrootsNostrPublicKey>, ParseError> {
    input.iter().map(|s| radroots_nostr_parse_pubkey(s)).collect()
}
