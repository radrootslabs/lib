use crate::error::RadrootsSimplexSmpTransportError;
use crate::frame::{
    RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE, RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE,
    decode_padded_bytes, encode_padded_bytes,
};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use radroots_simplex_smp_proto::prelude::{
    RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_PROXY_SERVER_HANDSHAKE_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION, RadrootsSimplexSmpVersionRange,
};

pub const RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1: &str = "smp/1";
pub const RADROOTS_SIMPLEX_SMP_TLS_V1_3_CIPHER_SUITE: &str = "TLS_CHACHA20_POLY1305_SHA256";
pub const RADROOTS_SIMPLEX_SMP_TLS_SIGNATURE_ALGORITHM: &str = "ed25519";
pub const RADROOTS_SIMPLEX_SMP_TLS_KEY_EXCHANGE_GROUP: &str = "x25519";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpTransportServerProof {
    pub certificate_payload: Vec<u8>,
    pub signed_server_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpServerHello {
    pub version_range: RadrootsSimplexSmpVersionRange,
    pub session_identifier: Vec<u8>,
    pub server_proof: Option<RadrootsSimplexSmpTransportServerProof>,
    pub ignored_part: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpClientHello {
    pub chosen_version: u16,
    pub server_key_hash: Vec<u8>,
    pub client_key: Option<Vec<u8>>,
    pub proxy_server: bool,
    pub ignored_part: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpTlsPolicy {
    pub expected_server_identity: String,
    pub supported_versions: RadrootsSimplexSmpVersionRange,
    pub require_current_alpn: bool,
    pub allow_session_resumption: bool,
    pub allowed_certificate_chain_lengths: [usize; 3],
    pub require_tls_unique_binding: bool,
    pub require_server_proof: bool,
}

impl RadrootsSimplexSmpTlsPolicy {
    pub fn modern(expected_server_identity: impl Into<String>) -> Self {
        Self {
            expected_server_identity: expected_server_identity.into(),
            supported_versions: RadrootsSimplexSmpVersionRange::single(
                RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            ),
            require_current_alpn: true,
            allow_session_resumption: false,
            allowed_certificate_chain_lengths: [2, 3, 4],
            require_tls_unique_binding: true,
            require_server_proof: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpTlsHandshakeEvidence {
    pub confirmed_alpn: Option<String>,
    pub session_resumed: bool,
    pub certificate_chain_length: usize,
    pub online_certificate_fingerprint: String,
    pub tls_unique_channel_binding: Option<Vec<u8>>,
}

impl RadrootsSimplexSmpServerHello {
    pub fn encode(&self) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.version_range.min.to_be_bytes());
        payload.extend_from_slice(&self.version_range.max.to_be_bytes());
        push_short_bytes(&mut payload, &self.session_identifier)?;
        if let Some(proof) = &self.server_proof {
            payload.extend_from_slice(&proof.certificate_payload);
            push_large_bytes(&mut payload, &proof.signed_server_key)?;
        }
        payload.extend_from_slice(&self.ignored_part);
        encode_padded_bytes(
            &payload,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE,
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RadrootsSimplexSmpTransportError> {
        let payload = decode_padded_bytes(
            bytes,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE,
        )?;
        let Some(version_bytes) = payload.get(..4) else {
            return Err(RadrootsSimplexSmpTransportError::MissingHandshakeField(
                "smp_version_range",
            ));
        };
        let min = u16::from_be_bytes([version_bytes[0], version_bytes[1]]);
        let max = u16::from_be_bytes([version_bytes[2], version_bytes[3]]);
        let version_range = RadrootsSimplexSmpVersionRange::new(min, max)
            .map_err(RadrootsSimplexSmpTransportError::from)?;
        let (session_identifier, cursor) = read_short_bytes(&payload, 4)?;
        if session_identifier.len() > u8::MAX as usize {
            return Err(
                RadrootsSimplexSmpTransportError::InvalidSessionIdentifierLength(
                    session_identifier.len(),
                ),
            );
        }
        let (server_proof, ignored_part) = parse_optional_server_proof(&payload[cursor..]);

        Ok(Self {
            version_range,
            session_identifier,
            server_proof,
            ignored_part,
        })
    }
}

impl RadrootsSimplexSmpClientHello {
    pub fn encode(&self) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.chosen_version.to_be_bytes());
        push_short_bytes(&mut payload, &self.server_key_hash)?;
        if let Some(client_key) = &self.client_key {
            push_short_bytes(&mut payload, client_key)?;
        }
        if self.chosen_version >= RADROOTS_SIMPLEX_SMP_PROXY_SERVER_HANDSHAKE_TRANSPORT_VERSION {
            payload.push(if self.proxy_server { b'T' } else { b'F' });
        }
        if self.chosen_version >= RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION {
            payload.push(b'0');
        }
        payload.extend_from_slice(&self.ignored_part);
        encode_padded_bytes(
            &payload,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE,
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RadrootsSimplexSmpTransportError> {
        let payload = decode_padded_bytes(
            bytes,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE,
        )?;
        let Some(version_bytes) = payload.get(..2) else {
            return Err(RadrootsSimplexSmpTransportError::MissingHandshakeField(
                "chosen_version",
            ));
        };
        let chosen_version = u16::from_be_bytes([version_bytes[0], version_bytes[1]]);
        let (server_key_hash, mut cursor) = read_short_bytes(&payload, 2)?;
        let client_key = if chosen_version
            >= RADROOTS_SIMPLEX_SMP_PROXY_SERVER_HANDSHAKE_TRANSPORT_VERSION
            && matches!(payload.get(cursor), Some(b'T' | b'F'))
        {
            None
        } else {
            let (client_key, consumed) = parse_optional_client_key(&payload[cursor..]);
            cursor += consumed;
            client_key
        };
        let proxy_server =
            if chosen_version >= RADROOTS_SIMPLEX_SMP_PROXY_SERVER_HANDSHAKE_TRANSPORT_VERSION {
                let Some(value) = payload.get(cursor) else {
                    return Err(RadrootsSimplexSmpTransportError::MissingHandshakeField(
                        "proxy_server",
                    ));
                };
                cursor += 1;
                match *value {
                    b'T' => true,
                    b'F' => false,
                    _ => {
                        return Err(RadrootsSimplexSmpTransportError::MissingHandshakeField(
                            "proxy_server",
                        ));
                    }
                }
            } else {
                false
            };
        if chosen_version >= RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION {
            let Some(tag) = payload.get(cursor) else {
                return Err(RadrootsSimplexSmpTransportError::MissingHandshakeField(
                    "client_service",
                ));
            };
            cursor += 1;
            if *tag != b'0' {
                return Err(RadrootsSimplexSmpTransportError::MissingHandshakeField(
                    "client_service",
                ));
            }
        }
        let ignored_part = payload[cursor..].to_vec();

        Ok(Self {
            chosen_version,
            server_key_hash,
            client_key,
            proxy_server,
            ignored_part,
        })
    }
}

pub fn negotiate_transport_version(
    offered: RadrootsSimplexSmpVersionRange,
    supported: RadrootsSimplexSmpVersionRange,
    confirmed_alpn: Option<&str>,
) -> Result<u16, RadrootsSimplexSmpTransportError> {
    if confirmed_alpn == Some(RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1) {
        let min = offered.min.max(supported.min);
        let max = offered.max.min(supported.max);
        if min > max {
            return Err(RadrootsSimplexSmpTransportError::NoMutualTransportVersion {
                offered: offered.to_string(),
                supported: supported.to_string(),
            });
        }
        return Ok(max);
    }

    if offered.contains(RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION)
        && supported.contains(RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION)
    {
        return Ok(RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION);
    }

    Err(RadrootsSimplexSmpTransportError::NoMutualTransportVersion {
        offered: offered.to_string(),
        supported: supported.to_string(),
    })
}

pub fn validate_tls_handshake(
    policy: &RadrootsSimplexSmpTlsPolicy,
    server_hello: &RadrootsSimplexSmpServerHello,
    evidence: &RadrootsSimplexSmpTlsHandshakeEvidence,
) -> Result<u16, RadrootsSimplexSmpTransportError> {
    if policy.require_current_alpn
        && evidence.confirmed_alpn.as_deref() != Some(RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1)
    {
        return Err(RadrootsSimplexSmpTransportError::UnsupportedAlpn(
            evidence.confirmed_alpn.clone().unwrap_or_default(),
        ));
    }
    if !policy.allow_session_resumption && evidence.session_resumed {
        return Err(RadrootsSimplexSmpTransportError::SessionResumptionNotAllowed);
    }
    if !policy
        .allowed_certificate_chain_lengths
        .contains(&evidence.certificate_chain_length)
    {
        return Err(
            RadrootsSimplexSmpTransportError::InvalidCertificateChainLength(
                evidence.certificate_chain_length,
            ),
        );
    }
    if evidence.online_certificate_fingerprint != policy.expected_server_identity {
        return Err(RadrootsSimplexSmpTransportError::ServerIdentityMismatch {
            expected: policy.expected_server_identity.clone(),
            actual: evidence.online_certificate_fingerprint.clone(),
        });
    }
    if policy.require_server_proof && server_hello.server_proof.is_none() {
        return Err(RadrootsSimplexSmpTransportError::MissingServerProof);
    }
    if policy.require_tls_unique_binding {
        let Some(binding) = evidence.tls_unique_channel_binding.as_ref() else {
            return Err(RadrootsSimplexSmpTransportError::MissingChannelBinding);
        };
        if binding.as_slice() != server_hello.session_identifier.as_slice() {
            return Err(RadrootsSimplexSmpTransportError::SessionBindingMismatch);
        }
    }

    negotiate_transport_version(
        server_hello.version_range,
        policy.supported_versions,
        evidence.confirmed_alpn.as_deref(),
    )
}

fn push_short_bytes(
    buffer: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), RadrootsSimplexSmpTransportError> {
    if bytes.len() > u8::MAX as usize {
        return Err(RadrootsSimplexSmpTransportError::InvalidSessionIdentifierLength(bytes.len()));
    }
    buffer.push(bytes.len() as u8);
    buffer.extend_from_slice(bytes);
    Ok(())
}

fn push_large_bytes(
    buffer: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), RadrootsSimplexSmpTransportError> {
    let len = u16::try_from(bytes.len()).map_err(|_| {
        RadrootsSimplexSmpTransportError::InvalidSessionIdentifierLength(bytes.len())
    })?;
    buffer.extend_from_slice(&len.to_be_bytes());
    buffer.extend_from_slice(bytes);
    Ok(())
}

fn read_short_bytes(
    payload: &[u8],
    offset: usize,
) -> Result<(Vec<u8>, usize), RadrootsSimplexSmpTransportError> {
    let Some(&length) = payload.get(offset) else {
        return Err(RadrootsSimplexSmpTransportError::MissingHandshakeField(
            "short_field",
        ));
    };
    let start = offset + 1;
    let end = start + length as usize;
    let Some(value) = payload.get(start..end) else {
        return Err(
            radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpProtoError::UnexpectedEof.into(),
        );
    };
    Ok((value.to_vec(), end))
}

fn read_large_bytes(
    payload: &[u8],
    offset: usize,
) -> Result<(Vec<u8>, usize), RadrootsSimplexSmpTransportError> {
    let Some(length_bytes) = payload.get(offset..offset + 2) else {
        return Err(RadrootsSimplexSmpTransportError::MissingHandshakeField(
            "large_field",
        ));
    };
    let length = u16::from_be_bytes([length_bytes[0], length_bytes[1]]) as usize;
    let start = offset + 2;
    let end = start + length;
    let Some(value) = payload.get(start..end) else {
        return Err(
            radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpProtoError::UnexpectedEof.into(),
        );
    };
    Ok((value.to_vec(), end))
}

fn parse_optional_server_proof(
    remainder: &[u8],
) -> (Option<RadrootsSimplexSmpTransportServerProof>, Vec<u8>) {
    let Some(&cert_count) = remainder.first() else {
        return (None, remainder.to_vec());
    };
    if cert_count == 0 {
        return (None, remainder.to_vec());
    }
    let mut cursor = 1;
    for _ in 0..cert_count {
        let Ok((_, next_cursor)) = read_large_bytes(remainder, cursor) else {
            return (None, remainder.to_vec());
        };
        cursor = next_cursor;
    }
    let Ok((signed_server_key, cursor)) = read_large_bytes(remainder, cursor) else {
        return (None, remainder.to_vec());
    };
    (
        Some(RadrootsSimplexSmpTransportServerProof {
            certificate_payload: remainder[..cursor - signed_server_key.len() - 2].to_vec(),
            signed_server_key,
        }),
        remainder[cursor..].to_vec(),
    )
}

fn parse_optional_client_key(remainder: &[u8]) -> (Option<Vec<u8>>, usize) {
    let Some(&length) = remainder.first() else {
        return (None, 0);
    };
    let end = 1 + length as usize;
    if length == 0 || end > remainder.len() {
        return (None, remainder.len());
    }
    (Some(remainder[1..end].to_vec()), end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_server_hello_and_validates_binding() {
        let hello = RadrootsSimplexSmpServerHello {
            version_range: RadrootsSimplexSmpVersionRange::new(6, 17).unwrap(),
            session_identifier: b"tls-unique-binding".to_vec(),
            server_proof: Some(RadrootsSimplexSmpTransportServerProof {
                certificate_payload: encode_certificate_chain_payload([b"cert-chain".as_slice()]),
                signed_server_key: b"signed-key".to_vec(),
            }),
            ignored_part: b"ignored".to_vec(),
        };

        let decoded = RadrootsSimplexSmpServerHello::decode(&hello.encode().unwrap()).unwrap();
        assert_eq!(decoded, hello);

        let policy = RadrootsSimplexSmpTlsPolicy {
            expected_server_identity: "fingerprint".to_string(),
            supported_versions: RadrootsSimplexSmpVersionRange::new(6, 17).unwrap(),
            require_current_alpn: false,
            allow_session_resumption: false,
            allowed_certificate_chain_lengths: [2, 3, 4],
            require_tls_unique_binding: true,
            require_server_proof: true,
        };
        let version = validate_tls_handshake(
            &policy,
            &decoded,
            &RadrootsSimplexSmpTlsHandshakeEvidence {
                confirmed_alpn: Some(RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1.to_string()),
                session_resumed: false,
                certificate_chain_length: 3,
                online_certificate_fingerprint: "fingerprint".to_string(),
                tls_unique_channel_binding: Some(b"tls-unique-binding".to_vec()),
            },
        )
        .unwrap();
        assert_eq!(version, 17);
    }

    #[test]
    fn falls_back_to_initial_transport_version_without_current_alpn() {
        let version = negotiate_transport_version(
            RadrootsSimplexSmpVersionRange::new(6, 17).unwrap(),
            RadrootsSimplexSmpVersionRange::new(6, 17).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(version, 6);
    }

    #[test]
    fn rejects_mismatched_server_identity() {
        let hello = RadrootsSimplexSmpServerHello {
            version_range: RadrootsSimplexSmpVersionRange::new(6, 17).unwrap(),
            session_identifier: b"bind".to_vec(),
            server_proof: None,
            ignored_part: Vec::new(),
        };
        let policy = RadrootsSimplexSmpTlsPolicy::modern("expected");
        let error = validate_tls_handshake(
            &policy,
            &hello,
            &RadrootsSimplexSmpTlsHandshakeEvidence {
                confirmed_alpn: Some(RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1.to_string()),
                session_resumed: false,
                certificate_chain_length: 2,
                online_certificate_fingerprint: "actual".to_string(),
                tls_unique_channel_binding: Some(b"bind".to_vec()),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RadrootsSimplexSmpTransportError::ServerIdentityMismatch { .. }
        ));
    }

    fn encode_certificate_chain_payload<'a, I>(certificates: I) -> Vec<u8>
    where
        I: IntoIterator<Item = &'a [u8]>,
    {
        let certificates: Vec<&[u8]> = certificates.into_iter().collect();
        let mut payload = vec![certificates.len() as u8];
        for certificate in certificates {
            payload.extend_from_slice(&(certificate.len() as u16).to_be_bytes());
            payload.extend_from_slice(certificate);
        }
        payload
    }
}
