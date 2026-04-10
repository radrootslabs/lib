#![cfg(feature = "std")]

use crate::error::RadrootsSimplexSmpTransportError;
use crate::executor::{
    RadrootsSimplexSmpCommandTransport, RadrootsSimplexSmpTransportRequest,
    RadrootsSimplexSmpTransportResponse,
};
use crate::frame::{RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE, RadrootsSimplexSmpTransportBlock};
use crate::handshake::{
    RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1, RadrootsSimplexSmpClientHello, RadrootsSimplexSmpServerHello,
    RadrootsSimplexSmpTlsHandshakeEvidence, RadrootsSimplexSmpTlsPolicy,
    RadrootsSimplexSmpTransportServerProof, validate_tls_handshake,
};
use base64::Engine as _;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use radroots_simplex_smp_crypto::prelude::{
    RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
    RadrootsSimplexSmpSecretBoxChainKey, RadrootsSimplexSmpX25519Keypair, advance_secretbox_chain,
    decode_x25519_public_key_x509, derive_shared_secret, encode_x25519_public_key_x509,
    encrypt_padded, init_secretbox_chain, verify_signature,
};
use radroots_simplex_smp_proto::prelude::{
    RADROOTS_SIMPLEX_SMP_AUTH_COMMANDS_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_ENCRYPTED_BLOCK_TRANSPORT_VERSION, RadrootsSimplexSmpCommandTransmission,
    RadrootsSimplexSmpServerAddress,
};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, Error as RustlsError, SignatureScheme,
    StreamOwned,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use x509_parser::prelude::FromDer;

#[derive(Default)]
pub struct RadrootsSimplexSmpTlsCommandTransport {
    sessions: BTreeMap<String, RadrootsSimplexSmpLiveSession>,
}

struct RadrootsSimplexSmpLiveSession {
    stream: StreamOwned<ClientConnection, TcpStream>,
    transport_version: u16,
    session_identifier: Vec<u8>,
    send_chain_key: Option<RadrootsSimplexSmpSecretBoxChainKey>,
    receive_chain_key: Option<RadrootsSimplexSmpSecretBoxChainKey>,
    debug_shared_secret: Option<Vec<u8>>,
}

impl RadrootsSimplexSmpTlsCommandTransport {
    pub fn new() -> Self {
        Self::default()
    }

    fn session_key(server: &RadrootsSimplexSmpServerAddress) -> String {
        let mut key = server.server_identity.clone();
        key.push('@');
        key.push_str(&server.hosts.join(","));
        key.push(':');
        key.push_str(&server.port.unwrap_or(5223).to_string());
        key
    }

    fn session_for(
        &mut self,
        server: &RadrootsSimplexSmpServerAddress,
    ) -> Result<&mut RadrootsSimplexSmpLiveSession, RadrootsSimplexSmpTransportError> {
        let key = Self::session_key(server);
        if !self.sessions.contains_key(&key) {
            let session = connect_live_session(server)?;
            self.sessions.insert(key.clone(), session);
        }
        self.sessions.get_mut(&key).ok_or_else(|| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
                "missing live SMP session for `{}`",
                server.server_identity
            ))
        })
    }
}

impl RadrootsSimplexSmpCommandTransport for RadrootsSimplexSmpTlsCommandTransport {
    type Error = RadrootsSimplexSmpTransportError;

    fn execute(
        &mut self,
        request: RadrootsSimplexSmpTransportRequest,
    ) -> Result<RadrootsSimplexSmpTransportResponse, Self::Error> {
        let key = Self::session_key(&request.server);
        match execute_live_request(self.session_for(&request.server)?, &request) {
            Ok(response) => Ok(response),
            Err(RadrootsSimplexSmpTransportError::LiveTransportIo(error)) => {
                self.sessions.remove(&key);
                let response = execute_live_request(self.session_for(&request.server)?, &request);
                match response {
                    Ok(response) => Ok(response),
                    Err(RadrootsSimplexSmpTransportError::LiveTransportIo(_)) => {
                        Err(RadrootsSimplexSmpTransportError::LiveTransportIo(error))
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }
}

fn execute_live_request(
    session: &mut RadrootsSimplexSmpLiveSession,
    request: &RadrootsSimplexSmpTransportRequest,
) -> Result<RadrootsSimplexSmpTransportResponse, RadrootsSimplexSmpTransportError> {
    let correlation_id = request
        .correlation_id
        .ok_or(RadrootsSimplexSmpTransportError::MissingCorrelationId)?;
    let scope = RadrootsSimplexSmpQueueAuthorizationScope::new(
        session.session_identifier.clone(),
        correlation_id,
        request.entity_id.clone(),
    )?;
    let material = RadrootsSimplexSmpQueueAuthorizationMaterial::for_command(
        &scope,
        &request.command,
        session.transport_version,
        &request.authorization,
    )?;
    let transmission = RadrootsSimplexSmpCommandTransmission {
        authorization: material.authorization,
        correlation_id: Some(correlation_id),
        entity_id: request.entity_id.clone(),
        command: request.command.clone(),
    };
    let block = RadrootsSimplexSmpTransportBlock::from_command_transmissions(
        &[transmission],
        session.transport_version,
    )?;
    let encoded = encode_live_transport_block(session, &block)?;
    session
        .stream
        .write_all(&encoded)
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;
    session
        .stream
        .flush()
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;

    let mut response_block = vec![0_u8; RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE];
    session
        .stream
        .read_exact(&mut response_block)
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;
    let response_hash = Sha256::digest(&response_block).to_vec();
    let decoded = decode_live_transport_block(session, &response_block)?;
    let transmissions = decoded.decode_broker_transmissions(session.transport_version)?;
    if transmissions.len() != 1 {
        return Err(
            RadrootsSimplexSmpTransportError::UnexpectedBrokerTransmissionCount(
                transmissions.len(),
            ),
        );
    }
    let transmission = transmissions.into_iter().next().expect("checked len");
    if transmission.correlation_id != Some(correlation_id) {
        return Err(RadrootsSimplexSmpTransportError::CorrelationIdMismatch);
    }
    Ok(RadrootsSimplexSmpTransportResponse {
        server: request.server.clone(),
        transport_version: session.transport_version,
        transmission,
        transport_hash: response_hash,
    })
}

fn transport_debug_enabled() -> bool {
    std::env::var_os("RADROOTS_SIMPLEX_DEBUG_TRANSPORT").is_some()
}

fn debug_sha256_label(label: &str, value: &[u8]) {
    if transport_debug_enabled() {
        eprintln!(
            "[simplex-smp-transport] {label}: len={} sha256={}",
            value.len(),
            URL_SAFE_NO_PAD.encode(Sha256::digest(value)),
        );
    }
}

fn encode_live_transport_block(
    session: &mut RadrootsSimplexSmpLiveSession,
    block: &RadrootsSimplexSmpTransportBlock,
) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
    if session.transport_version >= RADROOTS_SIMPLEX_SMP_ENCRYPTED_BLOCK_TRANSPORT_VERSION {
        if let Some(chain_key) = session.send_chain_key.as_ref() {
            let ((secretbox_key, nonce), next_chain_key) = advance_secretbox_chain(chain_key)?;
            session.send_chain_key = Some(next_chain_key);
            return encrypt_padded(
                &secretbox_key,
                &nonce,
                &block.encode_payload()?,
                RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE - 16,
            )
            .map_err(Into::into);
        }
    }
    block.encode()
}

fn decode_live_transport_block(
    session: &mut RadrootsSimplexSmpLiveSession,
    bytes: &[u8],
) -> Result<RadrootsSimplexSmpTransportBlock, RadrootsSimplexSmpTransportError> {
    if session.transport_version >= RADROOTS_SIMPLEX_SMP_ENCRYPTED_BLOCK_TRANSPORT_VERSION {
        if let Some(chain_key) = session.receive_chain_key.as_ref() {
            let ((secretbox_key, nonce), next_chain_key) = advance_secretbox_chain(chain_key)?;
            match radroots_simplex_smp_crypto::prelude::decrypt_padded(
                &secretbox_key,
                &nonce,
                bytes,
            ) {
                Ok(payload) => {
                    session.receive_chain_key = Some(next_chain_key);
                    debug_sha256_label("live-response-payload", &payload);
                    return RadrootsSimplexSmpTransportBlock::from_payload(&payload);
                }
                Err(error) => {
                    if transport_debug_enabled() {
                        eprintln!("[simplex-smp-transport] live response decrypt failed: {error}");
                        debug_sha256_label("live-response-ciphertext", bytes);
                    }
                    if let Some(send_chain_key) = session.send_chain_key.as_ref() {
                        let ((alt_secretbox_key, alt_nonce), _) =
                            advance_secretbox_chain(send_chain_key)?;
                        if radroots_simplex_smp_crypto::prelude::decrypt_padded(
                            &alt_secretbox_key,
                            &alt_nonce,
                            bytes,
                        )
                        .is_ok()
                        {
                            return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
                                "server response decrypted with the outbound chain key; live SMP block direction is assigned incorrectly".into(),
                            ));
                        }
                    }
                    debug_probe_transport_candidates(session, bytes);
                    if let Ok(block) = RadrootsSimplexSmpTransportBlock::decode(bytes) {
                        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
                            format!(
                                "server returned plaintext SMP block while encrypted transport was expected: {:?}",
                                block.transmissions.first().map(|t| &t[..t.len().min(8)])
                            ),
                        ));
                    }
                    return Err(error.into());
                }
            }
        }
    }
    RadrootsSimplexSmpTransportBlock::decode(bytes)
}

fn debug_probe_transport_candidates(session: &mut RadrootsSimplexSmpLiveSession, bytes: &[u8]) {
    if !transport_debug_enabled() {
        return;
    }
    let Some(shared_secret) = session.debug_shared_secret.as_ref() else {
        return;
    };
    let Ok((first_chain_key, second_chain_key)) =
        init_secretbox_chain(&session.session_identifier, shared_secret)
    else {
        return;
    };
    for (label, chain_key) in [
        ("initial-first", first_chain_key),
        ("initial-second", second_chain_key),
    ] {
        let Ok(((secretbox_key, nonce), _)) = advance_secretbox_chain(&chain_key) else {
            continue;
        };
        let result =
            radroots_simplex_smp_crypto::prelude::decrypt_padded(&secretbox_key, &nonce, bytes);
        match result {
            Ok(payload) => {
                eprintln!("[simplex-smp-transport] debug candidate {label} decrypted live block");
                debug_sha256_label("debug-candidate-payload", &payload);
            }
            Err(error) => {
                eprintln!("[simplex-smp-transport] debug candidate {label} failed: {error}");
            }
        }
    }
}

fn connect_live_session(
    server: &RadrootsSimplexSmpServerAddress,
) -> Result<RadrootsSimplexSmpLiveSession, RadrootsSimplexSmpTransportError> {
    let mut last_error = None;
    for host in &server.hosts {
        match connect_live_session_host(server, host) {
            Ok(session) => return Ok(session),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
            "SMP server `{}` has no usable hosts",
            server.server_identity
        ))
    }))
}

fn connect_live_session_host(
    server: &RadrootsSimplexSmpServerAddress,
    host: &str,
) -> Result<RadrootsSimplexSmpLiveSession, RadrootsSimplexSmpTransportError> {
    let port = server.port.unwrap_or(5223);
    let mut addresses = (host, port).to_socket_addrs().map_err(|error| {
        RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
            "failed to resolve SMP server host `{host}:{port}`: {error}"
        ))
    })?;
    let socket_addr = addresses.next().ok_or_else(|| {
        RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
            "failed to resolve SMP server host `{host}:{port}`"
        ))
    })?;
    let tcp =
        TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5)).map_err(|error| {
            RadrootsSimplexSmpTransportError::LiveTransportIo(format!(
                "failed to connect to SMP server `{host}:{port}`: {error}"
            ))
        })?;
    tcp.set_nodelay(true)
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;
    tcp.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;
    tcp.set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;

    let server_name = match host.parse::<IpAddr>() {
        Ok(address) => ServerName::IpAddress(address.into()),
        Err(_) => ServerName::try_from(host.to_owned()).map_err(|_| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
                "invalid SMP server name `{host}`"
            ))
        })?,
    };
    let verifier = Arc::new(PermissiveSimplexServerVerifier);
    let mut config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.alpn_protocols = vec![RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1.as_bytes().to_vec()];

    let mut stream = StreamOwned::new(
        ClientConnection::new(Arc::new(config), server_name).map_err(|error| {
            RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string())
        })?,
        tcp,
    );
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock).map_err(|error| {
            RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string())
        })?;
    }

    let peer_certs = stream
        .conn
        .peer_certificates()
        .ok_or(RadrootsSimplexSmpTransportError::MissingPeerCertificates)?
        .to_vec();
    let server_hello = read_server_hello(&mut stream)?;
    let actual_identity = matching_server_identity(&peer_certs, &server.server_identity)?;
    let expected_identity = canonical_server_identity(&server.server_identity)?;
    let mut policy = RadrootsSimplexSmpTlsPolicy::modern(expected_identity.clone());
    policy.require_tls_unique_binding = false;
    let transport_version = validate_tls_handshake(
        &policy,
        &server_hello,
        &RadrootsSimplexSmpTlsHandshakeEvidence {
            confirmed_alpn: stream
                .conn
                .alpn_protocol()
                .map(|value| String::from_utf8_lossy(value).into_owned()),
            session_resumed: false,
            certificate_chain_length: peer_certs.len(),
            online_certificate_fingerprint: actual_identity,
            tls_unique_channel_binding: None,
        },
    )?;
    let transport_keypair =
        if transport_version >= RADROOTS_SIMPLEX_SMP_AUTH_COMMANDS_TRANSPORT_VERSION {
            Some(RadrootsSimplexSmpX25519Keypair::generate()?)
        } else {
            None
        };
    let client_hello = RadrootsSimplexSmpClientHello {
        chosen_version: transport_version,
        server_key_hash: decode_server_identity(&expected_identity)?,
        client_key: transport_keypair
            .as_ref()
            .map(|keypair| encode_x25519_public_key_x509(&keypair.public_key))
            .transpose()?,
        proxy_server: false,
        ignored_part: Vec::new(),
    };
    let encoded_client_hello = client_hello.encode()?;
    if transport_debug_enabled() {
        debug_sha256_label("client-hello", &encoded_client_hello);
        debug_sha256_label("server-session-id", &server_hello.session_identifier);
    }
    stream
        .write_all(&encoded_client_hello)
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;

    let mut debug_shared_secret = None;
    let (receive_chain_key, send_chain_key) =
        if transport_version >= RADROOTS_SIMPLEX_SMP_ENCRYPTED_BLOCK_TRANSPORT_VERSION {
            let server_key = decode_server_transport_public_key(
                server_hello
                    .server_proof
                    .as_ref()
                    .ok_or(RadrootsSimplexSmpTransportError::MissingServerProof)?,
            )?;
            let shared_secret = derive_shared_secret(
                &transport_keypair
                    .as_ref()
                    .ok_or(RadrootsSimplexSmpTransportError::MissingServerProof)?
                    .private_key,
                &server_key,
            )?;
            if transport_debug_enabled() {
                if let Some(keypair) = transport_keypair.as_ref() {
                    debug_sha256_label("client-transport-public-key", &keypair.public_key);
                }
                debug_sha256_label("server-transport-public-key", &server_key);
                debug_sha256_label("transport-shared-secret", &shared_secret);
            }
            debug_shared_secret = transport_debug_enabled().then_some(shared_secret.clone());
            let (receive_chain_key, send_chain_key) =
                init_secretbox_chain(&server_hello.session_identifier, &shared_secret)?;
            (Some(receive_chain_key), Some(send_chain_key))
        } else {
            (None, None)
        };

    Ok(RadrootsSimplexSmpLiveSession {
        stream,
        transport_version,
        session_identifier: server_hello.session_identifier,
        send_chain_key,
        receive_chain_key,
        debug_shared_secret,
    })
}

fn decode_server_transport_public_key(
    proof: &RadrootsSimplexSmpTransportServerProof,
) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
    let (signed_object, signature) = decode_signed_server_key_parts(&proof.signed_server_key)?;
    if !proof.certificate_payload.is_empty() {
        let verify_key = decode_server_certificate_verify_key(&proof.certificate_payload)?;
        verify_signature(signed_object, &verify_key, signature).map_err(|error| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
                "failed to verify SMP server transport key signature: {error}"
            ))
        })?;
    }

    decode_x25519_public_key_x509(signed_object)
        .or_else(|_| {
            first_der_sequence_element(signed_object)
                .and_then(|candidate| decode_x25519_public_key_x509(candidate).map_err(Into::into))
        })
        .map_err(|error: RadrootsSimplexSmpTransportError| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
                "failed to decode verified SMP server transport key: {error}"
            ))
        })
}

fn first_der_sequence_element(bytes: &[u8]) -> Result<&[u8], RadrootsSimplexSmpTransportError> {
    let (sequence_tag, _, sequence_header_end, sequence_content_end) = parse_der_element(bytes, 0)?;
    if sequence_tag != 0x30 {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: expected DER sequence".into(),
        ));
    }
    let (_, element_start, _, element_end) = parse_der_element(bytes, sequence_header_end)?;
    if element_end > sequence_content_end {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: first element exceeds sequence bounds".into(),
        ));
    }
    Ok(&bytes[element_start..element_end])
}

fn decode_signed_server_key_parts(
    bytes: &[u8],
) -> Result<(&[u8], &[u8]), RadrootsSimplexSmpTransportError> {
    let (sequence_tag, _, sequence_header_end, sequence_content_end) = parse_der_element(bytes, 0)?;
    if sequence_tag != 0x30 {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: signed key is not a DER sequence".into(),
        ));
    }

    let (_, signed_object_start, _, signed_object_end) =
        parse_der_element(bytes, sequence_header_end)?;
    let (_, _, _, algorithm_end) = parse_der_element(bytes, signed_object_end)?;
    if algorithm_end > sequence_content_end {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: signature algorithm exceeds sequence bounds".into(),
        ));
    }
    let (signature_tag, _, signature_value_start, signature_end) =
        parse_der_element(bytes, algorithm_end)?;
    if signature_tag != 0x03 {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: expected DER bit string signature".into(),
        ));
    }
    if signature_end > sequence_content_end {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: signature exceeds sequence bounds".into(),
        ));
    }
    let signature_value = bytes
        .get(signature_value_start..signature_end)
        .ok_or_else(|| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(
                "invalid SMP server proof: truncated signature".into(),
            )
        })?;
    let (unused_bits, signature) = signature_value.split_first().ok_or_else(|| {
        RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: missing signature payload".into(),
        )
    })?;
    if *unused_bits != 0 {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: unsupported signature bit padding".into(),
        ));
    }
    Ok((&bytes[signed_object_start..signed_object_end], signature))
}

fn decode_server_certificate_verify_key(
    certificate_payload: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
    let Some(&cert_count) = certificate_payload.first() else {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: missing certificate chain".into(),
        ));
    };
    if cert_count == 0 {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: empty certificate chain".into(),
        ));
    }
    let (certificate_der, _) = read_large_handshake_field(certificate_payload, 1)?;
    let (_, certificate) = x509_parser::certificate::X509Certificate::from_der(&certificate_der)
        .map_err(|error| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
                "failed to parse SMP proof certificate: {error}"
            ))
        })?;
    Ok(certificate
        .tbs_certificate
        .subject_pki
        .subject_public_key
        .data
        .to_vec())
}

fn read_large_handshake_field(
    bytes: &[u8],
    offset: usize,
) -> Result<(Vec<u8>, usize), RadrootsSimplexSmpTransportError> {
    let Some(length_bytes) = bytes.get(offset..offset + 2) else {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: truncated certificate length".into(),
        ));
    };
    let length = u16::from_be_bytes([length_bytes[0], length_bytes[1]]) as usize;
    let start = offset + 2;
    let end = start + length;
    let value = bytes.get(start..end).ok_or_else(|| {
        RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: certificate exceeds payload".into(),
        )
    })?;
    Ok((value.to_vec(), end))
}

fn parse_der_element(
    bytes: &[u8],
    offset: usize,
) -> Result<(u8, usize, usize, usize), RadrootsSimplexSmpTransportError> {
    let tag = *bytes.get(offset).ok_or_else(|| {
        RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: truncated DER element".into(),
        )
    })?;
    let length_offset = offset + 1;
    let length_tag = *bytes.get(length_offset).ok_or_else(|| {
        RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: missing DER length".into(),
        )
    })?;
    let (value_len, header_len) = if length_tag & 0x80 == 0 {
        (length_tag as usize, 2)
    } else {
        let length_bytes = (length_tag & 0x7f) as usize;
        if length_bytes == 0 || length_bytes > 4 {
            return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
                "invalid SMP server proof: unsupported DER length encoding".into(),
            ));
        }
        let length_start = length_offset + 1;
        let length_end = length_start + length_bytes;
        let encoded_length = bytes.get(length_start..length_end).ok_or_else(|| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(
                "invalid SMP server proof: truncated DER length".into(),
            )
        })?;
        let value_len = encoded_length
            .iter()
            .fold(0_usize, |acc, byte| (acc << 8) | (*byte as usize));
        (value_len, 2 + length_bytes)
    };
    let value_start = offset + header_len;
    let value_end = value_start + value_len;
    if value_end > bytes.len() {
        return Err(RadrootsSimplexSmpTransportError::InvalidServerAddress(
            "invalid SMP server proof: DER element exceeds input".into(),
        ));
    }
    Ok((tag, offset, value_start, value_end))
}

fn read_server_hello(
    stream: &mut StreamOwned<ClientConnection, TcpStream>,
) -> Result<RadrootsSimplexSmpServerHello, RadrootsSimplexSmpTransportError> {
    let mut block = vec![0_u8; RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE];
    stream
        .read_exact(&mut block)
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;
    RadrootsSimplexSmpServerHello::decode(&block)
}

fn matching_server_identity(
    chain: &[CertificateDer<'static>],
    expected_identity: &str,
) -> Result<String, RadrootsSimplexSmpTransportError> {
    let expected_identity = canonical_server_identity(expected_identity)?;
    for certificate in chain {
        let identity = server_identity_from_certificate(certificate.as_ref())?;
        if identity == expected_identity {
            return Ok(identity);
        }
    }
    Err(RadrootsSimplexSmpTransportError::ServerIdentityMismatch {
        expected: expected_identity.into(),
        actual: chain
            .first()
            .map(|certificate| server_identity_from_certificate(certificate.as_ref()))
            .transpose()?
            .unwrap_or_default(),
    })
}

fn server_identity_from_certificate(
    der: &[u8],
) -> Result<String, RadrootsSimplexSmpTransportError> {
    x509_parser::certificate::X509Certificate::from_der(der).map_err(|error| {
        RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
            "failed to parse SMP certificate: {error}"
        ))
    })?;
    let digest = Sha256::digest(der);
    Ok(URL_SAFE_NO_PAD.encode(digest))
}

fn canonical_server_identity(value: &str) -> Result<String, RadrootsSimplexSmpTransportError> {
    URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| URL_SAFE.decode(value))
        .map(|decoded| URL_SAFE_NO_PAD.encode(decoded))
        .map_err(|_| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
                "invalid base64url server identity `{value}`"
            ))
        })
}

fn decode_server_identity(value: &str) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
    URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| URL_SAFE.decode(value))
        .map_err(|_| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
                "invalid base64url server identity `{value}`"
            ))
        })
}

#[derive(Debug)]
struct PermissiveSimplexServerVerifier;

impl ServerCertVerifier for PermissiveSimplexServerVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ED25519,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_server_identity, decode_server_transport_public_key};
    use crate::handshake::RadrootsSimplexSmpTransportServerProof;
    use radroots_simplex_smp_crypto::prelude::{
        RadrootsSimplexSmpX25519Keypair, encode_x25519_public_key_x509,
    };

    #[test]
    fn canonicalizes_padded_and_unpadded_server_identity() {
        assert_eq!(canonical_server_identity("YWJjZA").unwrap(), "YWJjZA");
        assert_eq!(canonical_server_identity("YWJjZA==").unwrap(), "YWJjZA");
    }

    #[test]
    fn extracts_spki_from_signed_server_key_sequence() {
        let keypair = RadrootsSimplexSmpX25519Keypair::from_seed(b"transport-proof");
        let spki = encode_x25519_public_key_x509(&keypair.public_key).unwrap();
        let empty_sequence = der_sequence(core::iter::once(&[][..]));
        let signature = [0x03, 0x01, 0x00];
        let signed_object = der_sequence([
            spki.as_slice(),
            empty_sequence.as_slice(),
            signature.as_slice(),
        ]);
        let proof = RadrootsSimplexSmpTransportServerProof {
            certificate_payload: Vec::new(),
            signed_server_key: signed_object,
        };
        assert_eq!(
            decode_server_transport_public_key(&proof).unwrap(),
            keypair.public_key
        );
    }

    fn der_sequence<'a, I>(elements: I) -> Vec<u8>
    where
        I: IntoIterator<Item = &'a [u8]>,
    {
        let mut body = Vec::new();
        for element in elements {
            if element.is_empty() {
                body.extend_from_slice(&[0x30, 0x00]);
            } else {
                body.extend_from_slice(element);
            }
        }
        let mut sequence = vec![0x30];
        push_der_length(&mut sequence, body.len());
        sequence.extend_from_slice(&body);
        sequence
    }

    fn push_der_length(buffer: &mut Vec<u8>, len: usize) {
        if len < 0x80 {
            buffer.push(len as u8);
            return;
        }
        let mut bytes = Vec::new();
        let mut remaining = len;
        while remaining > 0 {
            bytes.push((remaining & 0xff) as u8);
            remaining >>= 8;
        }
        bytes.reverse();
        buffer.push(0x80 | (bytes.len() as u8));
        buffer.extend_from_slice(&bytes);
    }
}
