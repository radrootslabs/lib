#![cfg(feature = "std")]

use crate::error::RadrootsSimplexSmpTransportError;
use crate::executor::{
    RadrootsSimplexSmpCommandTransport, RadrootsSimplexSmpTransportRequest,
    RadrootsSimplexSmpTransportResponse,
};
use crate::frame::{RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE, RadrootsSimplexSmpTransportBlock};
use crate::handshake::{
    RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1, RadrootsSimplexSmpClientHello, RadrootsSimplexSmpServerHello,
    RadrootsSimplexSmpTlsHandshakeEvidence, RadrootsSimplexSmpTlsPolicy, validate_tls_handshake,
};
use base64::Engine as _;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use radroots_simplex_smp_crypto::prelude::{
    RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
};
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpCommandTransmission, RadrootsSimplexSmpServerAddress,
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
    let encoded = block.encode()?;
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
    let decoded = RadrootsSimplexSmpTransportBlock::decode(&response_block)?;
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
    let mut policy = RadrootsSimplexSmpTlsPolicy::modern(expected_identity);
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
    let client_hello = RadrootsSimplexSmpClientHello {
        chosen_version: transport_version,
        client_key: None,
        ignored_part: Vec::new(),
    };
    let encoded_client_hello = client_hello.encode()?;
    stream
        .write_all(&encoded_client_hello)
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| RadrootsSimplexSmpTransportError::LiveTransportIo(error.to_string()))?;

    Ok(RadrootsSimplexSmpLiveSession {
        stream,
        transport_version,
        session_identifier: server_hello.session_identifier,
    })
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
    let (_, certificate) =
        x509_parser::certificate::X509Certificate::from_der(der).map_err(|error| {
            RadrootsSimplexSmpTransportError::InvalidServerAddress(format!(
                "failed to parse SMP certificate: {error}"
            ))
        })?;
    let digest = Sha256::digest(certificate.tbs_certificate.subject_pki.raw);
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
    use super::canonical_server_identity;

    #[test]
    fn canonicalizes_padded_and_unpadded_server_identity() {
        assert_eq!(canonical_server_identity("YWJjZA").unwrap(), "YWJjZA");
        assert_eq!(canonical_server_identity("YWJjZA==").unwrap(), "YWJjZA");
    }
}
