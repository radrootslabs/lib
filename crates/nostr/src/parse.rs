use nostr::{key::PublicKey, nips::nip19::FromBech32};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid pubkey format: {0}")]
    Invalid(String),
}

pub fn parse_pubkey(s: &str) -> Result<PublicKey, ParseError> {
    PublicKey::from_bech32(s)
        .or_else(|_| PublicKey::from_hex(s))
        .map_err(|_| ParseError::Invalid(s.to_string()))
}

pub fn parse_pubkeys(input: &[String]) -> Result<Vec<PublicKey>, ParseError> {
    input.iter().map(|s| parse_pubkey(s)).collect()
}
