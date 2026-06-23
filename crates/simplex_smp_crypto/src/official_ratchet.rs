use crate::error::RadrootsSimplexSmpCryptoError;
use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{AesGcm, Nonce, aes::Aes256};
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use base64::Engine as _;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use hkdf::Hkdf;
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpVersionRange;
use sha2::{Digest, Sha256, Sha512};

pub const RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION: u16 = 2;
pub const RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION: u16 = 3;
pub const RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION: u16 = 3;
pub const RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH: usize = 56;
pub const RADROOTS_SIMPLEX_OFFICIAL_X448_SHARED_SECRET_LENGTH: usize = 56;
pub const RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH: usize = 32;
pub const RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH: usize = 16;
pub const RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH: usize = 16;
pub const RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PUBLIC_KEY_LENGTH: usize = sntrup761::PUBLIC_KEY_SIZE;
pub const RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PRIVATE_KEY_LENGTH: usize =
    sntrup761::SECRET_KEY_SIZE;
pub const RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_CIPHERTEXT_LENGTH: usize = sntrup761::CIPHERTEXT_SIZE;
pub const RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_SHARED_SECRET_LENGTH: usize =
    sntrup761::SHARED_SECRET_SIZE;
pub const RADROOTS_SIMPLEX_OFFICIAL_RATCHET_HEADER_LENGTH: usize = 88;
pub const RADROOTS_SIMPLEX_OFFICIAL_PQ_RATCHET_HEADER_LENGTH: usize = 2_310;
pub const RADROOTS_SIMPLEX_OFFICIAL_ROOT_RATCHET_INFO: &[u8] = b"SimpleXRootRatchet";
pub const RADROOTS_SIMPLEX_OFFICIAL_CHAIN_RATCHET_INFO: &[u8] = b"SimpleXChainRatchet";
pub const RADROOTS_SIMPLEX_OFFICIAL_X3DH_INFO: &[u8] = b"SimpleXX3DH";

const RADROOTS_SIMPLEX_OFFICIAL_HKDF3_OUTPUT_LENGTH: usize =
    RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH * 3;
const RADROOTS_SIMPLEX_OFFICIAL_PADDING_LENGTH_BYTES: usize = 2;
const RADROOTS_SIMPLEX_OFFICIAL_X448_DER_PUBLIC_KEY_PREFIX: [u8; 12] = [
    0x30, 0x42, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6f, 0x03, 0x39, 0x00,
];
type RadrootsSimplexOfficialAes256Gcm = AesGcm<Aes256, U16>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialX448Keypair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialSntrup761Keypair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialAesGcmPayload {
    pub auth_tag: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialEncryptedHeader {
    pub version: u16,
    pub iv: [u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH],
    pub auth_tag: Vec<u8>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialEncryptedMessage {
    pub encrypted_header: Vec<u8>,
    pub auth_tag: Vec<u8>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialX3dhParams {
    pub version_range: RadrootsSimplexSmpVersionRange,
    pub key_1: Vec<u8>,
    pub key_2: Vec<u8>,
    pub pq_public_key: Option<Vec<u8>>,
    pub pq_ciphertext: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialX3dhInit {
    pub associated_data: Vec<u8>,
    pub ratchet_key: Vec<u8>,
    pub sending_header_key: Vec<u8>,
    pub receiving_next_header_key: Vec<u8>,
    pub accepted_pq_shared_secret: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialMsgHeader {
    pub max_version: u16,
    pub dh_public_key: Vec<u8>,
    pub pq_public_key: Option<Vec<u8>>,
    pub pq_ciphertext: Option<Vec<u8>>,
    pub previous_sending_chain_length: u32,
    pub message_number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialRootKdfOutput {
    pub root_key: Vec<u8>,
    pub chain_key: Vec<u8>,
    pub next_header_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialChainKdfOutput {
    pub chain_key: Vec<u8>,
    pub message_key: Vec<u8>,
    pub message_iv: [u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH],
    pub header_iv: [u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialX3dhSenderPqInit {
    pub init: RadrootsSimplexOfficialX3dhInit,
    pub sender_params: RadrootsSimplexOfficialX3dhParams,
    pub local_pq_keypair: RadrootsSimplexOfficialSntrup761Keypair,
    pub pq_shared_secret: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexOfficialX3dhReceiverPqInit {
    pub init: RadrootsSimplexOfficialX3dhInit,
    pub pq_shared_secret: Vec<u8>,
}

pub fn official_ratchet_header_len(
    version: u16,
    pq_enabled: bool,
) -> Result<usize, RadrootsSimplexSmpCryptoError> {
    if version < RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION
        || version > RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION
    {
        return Err(RadrootsSimplexSmpCryptoError::InvalidOfficialRatchetVersion(version));
    }
    Ok(
        if pq_enabled && version >= RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION {
            RADROOTS_SIMPLEX_OFFICIAL_PQ_RATCHET_HEADER_LENGTH
        } else {
            RADROOTS_SIMPLEX_OFFICIAL_RATCHET_HEADER_LENGTH
        },
    )
}

pub fn official_full_header_len(
    version: u16,
    pq_enabled: bool,
) -> Result<usize, RadrootsSimplexSmpCryptoError> {
    Ok(2 + 1
        + official_ratchet_header_len(version, pq_enabled)?
        + RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH
        + RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH)
}

pub fn official_encoded_encrypted_header_len(
    version: u16,
    pq_enabled: bool,
) -> Result<usize, RadrootsSimplexSmpCryptoError> {
    Ok(2 + RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH
        + RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH
        + official_large_prefix_len(version)?
        + official_ratchet_header_len(version, pq_enabled)?)
}

pub fn official_encoded_encrypted_message_len(
    version: u16,
    pq_enabled: bool,
    padded_body_len: usize,
) -> Result<usize, RadrootsSimplexSmpCryptoError> {
    Ok(official_large_prefix_len(version)?
        + official_encoded_encrypted_header_len(version, pq_enabled)?
        + RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH
        + padded_body_len)
}

pub fn official_x448_keypair_from_seed(seed: &[u8]) -> RadrootsSimplexOfficialX448Keypair {
    let digest = Sha512::digest(seed);
    let mut private_key = [0_u8; RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH];
    private_key.copy_from_slice(&digest[..RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH]);
    official_x448_keypair_from_private(private_key)
}

pub fn generate_official_x448_keypair()
-> Result<RadrootsSimplexOfficialX448Keypair, RadrootsSimplexSmpCryptoError> {
    let mut private_key = [0_u8; RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH];
    getrandom::getrandom(&mut private_key)
        .map_err(|_| RadrootsSimplexSmpCryptoError::EntropyUnavailable)?;
    Ok(official_x448_keypair_from_private(private_key))
}

pub fn derive_official_x448_shared_secret(
    private_key: &[u8],
    public_key: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let private_key: [u8; RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH] = private_key
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPrivateKeyLength(private_key.len()))?;
    let public_key = x448::PublicKey::from_bytes(public_key).ok_or(
        RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(public_key.len()),
    )?;
    let private = x448::StaticSecret::from(private_key);
    Ok(private.diffie_hellman(&public_key).as_bytes().to_vec())
}

pub fn encode_official_x448_public_key_der(
    public_key: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    if public_key.len() != RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(
            public_key.len(),
        ));
    }
    let mut encoded = Vec::with_capacity(
        RADROOTS_SIMPLEX_OFFICIAL_X448_DER_PUBLIC_KEY_PREFIX.len() + public_key.len(),
    );
    encoded.extend_from_slice(&RADROOTS_SIMPLEX_OFFICIAL_X448_DER_PUBLIC_KEY_PREFIX);
    encoded.extend_from_slice(public_key);
    Ok(encoded)
}

pub fn decode_official_x448_public_key_der(
    encoded: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let expected_len = RADROOTS_SIMPLEX_OFFICIAL_X448_DER_PUBLIC_KEY_PREFIX.len()
        + RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH;
    if encoded.len() != expected_len
        || !encoded.starts_with(&RADROOTS_SIMPLEX_OFFICIAL_X448_DER_PUBLIC_KEY_PREFIX)
    {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(
            encoded.len(),
        ));
    }
    Ok(encoded[RADROOTS_SIMPLEX_OFFICIAL_X448_DER_PUBLIC_KEY_PREFIX.len()..].to_vec())
}

pub fn encode_official_x3dh_params_uri(
    params: &RadrootsSimplexOfficialX3dhParams,
) -> Result<String, RadrootsSimplexSmpCryptoError> {
    validate_official_x3dh_params(params)?;
    let key_1 = encode_official_urlsafe_bytes(&encode_official_x448_public_key_der(&params.key_1)?);
    let key_2 = encode_official_urlsafe_bytes(&encode_official_x448_public_key_der(&params.key_2)?);
    let mut encoded = format!("v={}&x3dh={key_1},{key_2}", params.version_range);
    if let Some(pq_public_key) = params.pq_public_key.as_deref() {
        encoded.push_str("&kem_key=");
        encoded.push_str(&encode_official_urlsafe_bytes(pq_public_key));
    }
    if let Some(pq_ciphertext) = params.pq_ciphertext.as_deref() {
        encoded.push_str("&kem_ct=");
        encoded.push_str(&encode_official_urlsafe_bytes(pq_ciphertext));
    }
    Ok(encoded)
}

pub fn decode_official_x3dh_params_uri(
    encoded: &str,
) -> Result<RadrootsSimplexOfficialX3dhParams, RadrootsSimplexSmpCryptoError> {
    let mut version_range = None;
    let mut x3dh = None;
    let mut pq_public_key = None;
    let mut pq_ciphertext = None;
    for pair in encoded.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').ok_or_else(|| {
            RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                "field is missing `=`".to_owned(),
            )
        })?;
        match key {
            "v" => {
                if version_range.replace(value.parse()?).is_some() {
                    return Err(
                        RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                            "duplicate `v` field".to_owned(),
                        ),
                    );
                }
            }
            "x3dh" => {
                if x3dh.replace(value.to_owned()).is_some() {
                    return Err(
                        RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                            "duplicate `x3dh` field".to_owned(),
                        ),
                    );
                }
            }
            "kem_key" => {
                if pq_public_key
                    .replace(decode_official_urlsafe_bytes(value)?)
                    .is_some()
                {
                    return Err(
                        RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                            "duplicate `kem_key` field".to_owned(),
                        ),
                    );
                }
            }
            "kem_ct" => {
                if pq_ciphertext
                    .replace(decode_official_urlsafe_bytes(value)?)
                    .is_some()
                {
                    return Err(
                        RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                            "duplicate `kem_ct` field".to_owned(),
                        ),
                    );
                }
            }
            _ => {
                return Err(
                    RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                        "unknown field".to_owned(),
                    ),
                );
            }
        }
    }
    let x3dh = x3dh.ok_or_else(|| {
        RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
            "missing `x3dh` field".to_owned(),
        )
    })?;
    let keys = split_official_x3dh_keys(&x3dh)?;
    let params = RadrootsSimplexOfficialX3dhParams {
        version_range: version_range.ok_or_else(|| {
            RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                "missing `v` field".to_owned(),
            )
        })?,
        key_1: decode_official_x448_public_key_der(&decode_official_urlsafe_bytes(keys.0)?)?,
        key_2: decode_official_x448_public_key_der(&decode_official_urlsafe_bytes(keys.1)?)?,
        pq_public_key,
        pq_ciphertext,
    };
    validate_official_x3dh_params(&params)?;
    Ok(params)
}

pub fn official_x3dh_sender_init(
    local_key_1: &RadrootsSimplexOfficialX448Keypair,
    local_key_2: &RadrootsSimplexOfficialX448Keypair,
    remote_params: &RadrootsSimplexOfficialX3dhParams,
) -> Result<RadrootsSimplexOfficialX3dhInit, RadrootsSimplexSmpCryptoError> {
    validate_official_x3dh_keypair(local_key_1)?;
    validate_official_x3dh_keypair(local_key_2)?;
    validate_official_x3dh_params(remote_params)?;
    if remote_params.pq_public_key.is_some() || remote_params.pq_ciphertext.is_some() {
        return Err(RadrootsSimplexSmpCryptoError::IncompletePqHeader);
    }
    official_x3dh_init(
        &local_key_1.public_key,
        &remote_params.key_1,
        &[
            derive_official_x448_shared_secret(&local_key_2.private_key, &remote_params.key_1)?,
            derive_official_x448_shared_secret(&local_key_1.private_key, &remote_params.key_2)?,
            derive_official_x448_shared_secret(&local_key_2.private_key, &remote_params.key_2)?,
        ],
        None,
    )
}

pub fn official_x3dh_receiver_init(
    local_key_1: &RadrootsSimplexOfficialX448Keypair,
    local_key_2: &RadrootsSimplexOfficialX448Keypair,
    remote_params: &RadrootsSimplexOfficialX3dhParams,
) -> Result<RadrootsSimplexOfficialX3dhInit, RadrootsSimplexSmpCryptoError> {
    validate_official_x3dh_keypair(local_key_1)?;
    validate_official_x3dh_keypair(local_key_2)?;
    validate_official_x3dh_params(remote_params)?;
    if remote_params.pq_public_key.is_some() || remote_params.pq_ciphertext.is_some() {
        return Err(RadrootsSimplexSmpCryptoError::IncompletePqHeader);
    }
    official_x3dh_init(
        &remote_params.key_1,
        &local_key_1.public_key,
        &[
            derive_official_x448_shared_secret(&local_key_1.private_key, &remote_params.key_2)?,
            derive_official_x448_shared_secret(&local_key_2.private_key, &remote_params.key_1)?,
            derive_official_x448_shared_secret(&local_key_2.private_key, &remote_params.key_2)?,
        ],
        None,
    )
}

pub fn official_x3dh_sender_init_accepting_pq(
    local_key_1: &RadrootsSimplexOfficialX448Keypair,
    local_key_2: &RadrootsSimplexOfficialX448Keypair,
    local_pq_keypair: RadrootsSimplexOfficialSntrup761Keypair,
    remote_params: &RadrootsSimplexOfficialX3dhParams,
    encapsulation_seed: &[u8],
) -> Result<RadrootsSimplexOfficialX3dhSenderPqInit, RadrootsSimplexSmpCryptoError> {
    validate_official_x3dh_keypair(local_key_1)?;
    validate_official_x3dh_keypair(local_key_2)?;
    validate_official_sntrup761_keypair(&local_pq_keypair)?;
    validate_official_x3dh_params(remote_params)?;
    let remote_pq_public_key = remote_params.pq_public_key.as_deref().ok_or(
        RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
            "PQ sender init requires remote proposed KEM key".to_owned(),
        ),
    )?;
    if remote_params.pq_ciphertext.is_some() {
        return Err(
            RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                "PQ sender init requires proposed KEM key without ciphertext".to_owned(),
            ),
        );
    }
    let (pq_ciphertext, pq_shared_secret) =
        encapsulate_official_sntrup761(remote_pq_public_key, encapsulation_seed)?;
    let init = official_x3dh_init(
        &local_key_1.public_key,
        &remote_params.key_1,
        &[
            derive_official_x448_shared_secret(&local_key_2.private_key, &remote_params.key_1)?,
            derive_official_x448_shared_secret(&local_key_1.private_key, &remote_params.key_2)?,
            derive_official_x448_shared_secret(&local_key_2.private_key, &remote_params.key_2)?,
        ],
        Some(&pq_shared_secret),
    )?;
    let sender_params = RadrootsSimplexOfficialX3dhParams {
        version_range: remote_params.version_range,
        key_1: local_key_1.public_key.clone(),
        key_2: local_key_2.public_key.clone(),
        pq_public_key: Some(local_pq_keypair.public_key.clone()),
        pq_ciphertext: Some(pq_ciphertext),
    };
    validate_official_x3dh_params(&sender_params)?;
    Ok(RadrootsSimplexOfficialX3dhSenderPqInit {
        init,
        sender_params,
        local_pq_keypair,
        pq_shared_secret,
    })
}

pub fn official_x3dh_receiver_init_accepting_pq(
    local_key_1: &RadrootsSimplexOfficialX448Keypair,
    local_key_2: &RadrootsSimplexOfficialX448Keypair,
    local_pq_keypair: &RadrootsSimplexOfficialSntrup761Keypair,
    remote_params: &RadrootsSimplexOfficialX3dhParams,
) -> Result<RadrootsSimplexOfficialX3dhReceiverPqInit, RadrootsSimplexSmpCryptoError> {
    validate_official_x3dh_keypair(local_key_1)?;
    validate_official_x3dh_keypair(local_key_2)?;
    validate_official_sntrup761_keypair(local_pq_keypair)?;
    validate_official_x3dh_params(remote_params)?;
    let pq_ciphertext = remote_params.pq_ciphertext.as_deref().ok_or(
        RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
            "PQ receiver init requires accepted KEM ciphertext".to_owned(),
        ),
    )?;
    if remote_params.pq_public_key.is_none() {
        return Err(RadrootsSimplexSmpCryptoError::IncompletePqHeader);
    }
    let pq_shared_secret =
        decapsulate_official_sntrup761(&local_pq_keypair.private_key, pq_ciphertext)?;
    let init = official_x3dh_init(
        &remote_params.key_1,
        &local_key_1.public_key,
        &[
            derive_official_x448_shared_secret(&local_key_1.private_key, &remote_params.key_2)?,
            derive_official_x448_shared_secret(&local_key_2.private_key, &remote_params.key_1)?,
            derive_official_x448_shared_secret(&local_key_2.private_key, &remote_params.key_2)?,
        ],
        Some(&pq_shared_secret),
    )?;
    Ok(RadrootsSimplexOfficialX3dhReceiverPqInit {
        init,
        pq_shared_secret,
    })
}

pub fn official_sntrup761_keypair_from_seed(
    seed: &[u8],
) -> RadrootsSimplexOfficialSntrup761Keypair {
    let seed = pq_seed(seed);
    let (public_key, private_key) = sntrup761::generate_key_from_seed(seed);
    RadrootsSimplexOfficialSntrup761Keypair {
        public_key: public_key.as_ref().to_vec(),
        private_key: private_key.as_ref().to_vec(),
    }
}

pub fn generate_official_sntrup761_keypair()
-> Result<RadrootsSimplexOfficialSntrup761Keypair, RadrootsSimplexSmpCryptoError> {
    let mut seed = [0_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH];
    getrandom::getrandom(&mut seed)
        .map_err(|_| RadrootsSimplexSmpCryptoError::EntropyUnavailable)?;
    Ok(official_sntrup761_keypair_from_seed(&seed))
}

pub fn encapsulate_official_sntrup761(
    public_key: &[u8],
    seed: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), RadrootsSimplexSmpCryptoError> {
    let public_key = sntrup761::EncapsulationKey::try_from(public_key)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPqKeyLength(public_key.len()))?;
    let (ciphertext, shared_secret) = public_key.encapsulate_deterministic(pq_seed(seed));
    Ok((
        ciphertext.as_ref().to_vec(),
        shared_secret.as_ref().to_vec(),
    ))
}

pub fn decapsulate_official_sntrup761(
    private_key: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let private_key = sntrup761::DecapsulationKey::try_from(private_key)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPrivateKeyLength(private_key.len()))?;
    let ciphertext = sntrup761::Ciphertext::try_from(ciphertext)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPqCiphertextLength(ciphertext.len()))?;
    Ok(private_key.decapsulate(&ciphertext).as_ref().to_vec())
}

pub fn official_root_kdf(
    root_key: &[u8],
    dh_shared_secret: &[u8],
    pq_shared_secret: Option<&[u8]>,
) -> Result<RadrootsSimplexOfficialRootKdfOutput, RadrootsSimplexSmpCryptoError> {
    if root_key.len() != RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(
            root_key.len(),
        ));
    }
    let mut input =
        Vec::with_capacity(dh_shared_secret.len() + pq_shared_secret.map_or(0, <[u8]>::len));
    input.extend_from_slice(dh_shared_secret);
    if let Some(shared_secret) = pq_shared_secret {
        input.extend_from_slice(shared_secret);
    }
    let (root_key, chain_key, next_header_key) = official_hkdf3(
        root_key,
        &input,
        RADROOTS_SIMPLEX_OFFICIAL_ROOT_RATCHET_INFO,
    )?;
    Ok(RadrootsSimplexOfficialRootKdfOutput {
        root_key,
        chain_key,
        next_header_key,
    })
}

pub fn official_chain_kdf(
    chain_key: &[u8],
) -> Result<RadrootsSimplexOfficialChainKdfOutput, RadrootsSimplexSmpCryptoError> {
    if chain_key.len() != RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(
            chain_key.len(),
        ));
    }
    let (chain_key, message_key, iv_material) =
        official_hkdf3(b"", chain_key, RADROOTS_SIMPLEX_OFFICIAL_CHAIN_RATCHET_INFO)?;
    let mut message_iv = [0_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH];
    let mut header_iv = [0_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH];
    message_iv.copy_from_slice(&iv_material[..RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH]);
    header_iv.copy_from_slice(
        &iv_material
            [RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH..RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH * 2],
    );
    Ok(RadrootsSimplexOfficialChainKdfOutput {
        chain_key,
        message_key,
        message_iv,
        header_iv,
    })
}

pub fn official_aes_gcm_encrypt_padded(
    key: &[u8],
    iv: &[u8],
    plaintext: &[u8],
    padded_len: usize,
    associated_data: &[u8],
) -> Result<RadrootsSimplexOfficialAesGcmPayload, RadrootsSimplexSmpCryptoError> {
    let padded = official_pad(plaintext, padded_len)?;
    let encrypted = official_aes_gcm_cipher(key)?
        .encrypt(
            official_aes_gcm_nonce(iv)?,
            Payload {
                msg: &padded,
                aad: associated_data,
            },
        )
        .map_err(|_| RadrootsSimplexSmpCryptoError::AesGcmAuthenticationFailed)?;
    split_official_aes_gcm_payload(&encrypted)
}

pub fn encode_official_msg_header(
    version: u16,
    header: &RadrootsSimplexOfficialMsgHeader,
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    validate_official_version(version)?;
    validate_official_version(header.max_version)?;
    let public_key = encode_official_x448_public_key_der(&header.dh_public_key)?;
    let mut buffer = Vec::with_capacity(2 + 1 + public_key.len() + 1 + 4 + 4);
    buffer.extend_from_slice(&header.max_version.to_be_bytes());
    push_official_short_bytes(&mut buffer, &public_key)?;
    if version >= RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION {
        push_official_msg_header_pq(&mut buffer, header)?;
    } else if header.pq_public_key.is_some() || header.pq_ciphertext.is_some() {
        return Err(
            RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                "PQ header params require E2E version 3".to_owned(),
            ),
        );
    }
    buffer.extend_from_slice(&header.previous_sending_chain_length.to_be_bytes());
    buffer.extend_from_slice(&header.message_number.to_be_bytes());
    Ok(buffer)
}

pub fn decode_official_msg_header(
    version: u16,
    bytes: &[u8],
) -> Result<RadrootsSimplexOfficialMsgHeader, RadrootsSimplexSmpCryptoError> {
    validate_official_version(version)?;
    let mut cursor = OfficialCursor::new(bytes);
    let max_version = cursor.read_u16()?;
    validate_official_version(max_version)?;
    let dh_public_key = decode_official_x448_public_key_der(cursor.read_short_bytes()?)?;
    let (pq_public_key, pq_ciphertext) = if version >= RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION {
        read_official_msg_header_pq(&mut cursor)?
    } else {
        (None, None)
    };
    let previous_sending_chain_length = cursor.read_u32()?;
    let message_number = cursor.read_u32()?;
    cursor.finish()?;
    Ok(RadrootsSimplexOfficialMsgHeader {
        max_version,
        dh_public_key,
        pq_public_key,
        pq_ciphertext,
        previous_sending_chain_length,
        message_number,
    })
}

pub fn encode_official_encrypted_header(
    header: &RadrootsSimplexOfficialEncryptedHeader,
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    validate_official_version(header.version)?;
    if header.auth_tag.len() != RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidSignatureLength(
            header.auth_tag.len(),
        ));
    }
    let mut buffer = Vec::with_capacity(
        2 + RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH
            + RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH
            + official_large_prefix_len(header.version)?
            + header.body.len(),
    );
    buffer.extend_from_slice(&header.version.to_be_bytes());
    buffer.extend_from_slice(&header.iv);
    buffer.extend_from_slice(&header.auth_tag);
    push_official_large_by_version(&mut buffer, header.version, &header.body)?;
    Ok(buffer)
}

pub fn decode_official_encrypted_header(
    bytes: &[u8],
) -> Result<RadrootsSimplexOfficialEncryptedHeader, RadrootsSimplexSmpCryptoError> {
    let mut cursor = OfficialCursor::new(bytes);
    let version = cursor.read_u16()?;
    validate_official_version(version)?;
    let iv = cursor.read_array::<RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH>()?;
    let auth_tag = cursor
        .read_slice(RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH)?
        .to_vec();
    let body = cursor.read_official_large()?.to_vec();
    cursor.finish()?;
    Ok(RadrootsSimplexOfficialEncryptedHeader {
        version,
        iv,
        auth_tag,
        body,
    })
}

pub fn encode_official_encrypted_message(
    version: u16,
    message: &RadrootsSimplexOfficialEncryptedMessage,
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    validate_official_version(version)?;
    if message.auth_tag.len() != RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidSignatureLength(
            message.auth_tag.len(),
        ));
    }
    let mut buffer = Vec::with_capacity(
        official_large_prefix_len(version)?
            + message.encrypted_header.len()
            + RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH
            + message.body.len(),
    );
    push_official_large_by_version(&mut buffer, version, &message.encrypted_header)?;
    buffer.extend_from_slice(&message.auth_tag);
    buffer.extend_from_slice(&message.body);
    Ok(buffer)
}

pub fn decode_official_encrypted_message(
    bytes: &[u8],
) -> Result<RadrootsSimplexOfficialEncryptedMessage, RadrootsSimplexSmpCryptoError> {
    let mut cursor = OfficialCursor::new(bytes);
    let encrypted_header = cursor.read_official_large()?.to_vec();
    let auth_tag = cursor
        .read_slice(RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH)?
        .to_vec();
    let body = cursor.read_remaining().to_vec();
    Ok(RadrootsSimplexOfficialEncryptedMessage {
        encrypted_header,
        auth_tag,
        body,
    })
}

pub fn official_aes_gcm_decrypt_padded(
    key: &[u8],
    iv: &[u8],
    payload: &RadrootsSimplexOfficialAesGcmPayload,
    associated_data: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    if payload.auth_tag.len() != RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidSignatureLength(
            payload.auth_tag.len(),
        ));
    }
    let mut encrypted = Vec::with_capacity(payload.ciphertext.len() + payload.auth_tag.len());
    encrypted.extend_from_slice(&payload.ciphertext);
    encrypted.extend_from_slice(&payload.auth_tag);
    let padded = official_aes_gcm_cipher(key)?
        .decrypt(
            official_aes_gcm_nonce(iv)?,
            Payload {
                msg: &encrypted,
                aad: associated_data,
            },
        )
        .map_err(|_| RadrootsSimplexSmpCryptoError::AesGcmAuthenticationFailed)?;
    official_unpad(&padded)
}

fn official_x448_keypair_from_private(
    private_key: [u8; RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH],
) -> RadrootsSimplexOfficialX448Keypair {
    let private = x448::StaticSecret::from(private_key);
    let public = x448::PublicKey::from(&private);
    RadrootsSimplexOfficialX448Keypair {
        public_key: public.as_bytes().to_vec(),
        private_key: private.as_bytes().to_vec(),
    }
}

fn official_aes_gcm_cipher(
    key: &[u8],
) -> Result<RadrootsSimplexOfficialAes256Gcm, RadrootsSimplexSmpCryptoError> {
    if key.len() != RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(
            key.len(),
        ));
    }
    RadrootsSimplexOfficialAes256Gcm::new_from_slice(key)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(key.len()))
}

fn official_aes_gcm_nonce(iv: &[u8]) -> Result<&Nonce<U16>, RadrootsSimplexSmpCryptoError> {
    if iv.len() != RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidNonceLength(iv.len()));
    }
    Ok(Nonce::<U16>::from_slice(iv))
}

fn split_official_aes_gcm_payload(
    encrypted: &[u8],
) -> Result<RadrootsSimplexOfficialAesGcmPayload, RadrootsSimplexSmpCryptoError> {
    if encrypted.len() < RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(
            encrypted.len(),
        ));
    }
    let tag_offset = encrypted.len() - RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH;
    let (ciphertext, auth_tag) = encrypted.split_at(tag_offset);
    Ok(RadrootsSimplexOfficialAesGcmPayload {
        auth_tag: auth_tag.to_vec(),
        ciphertext: ciphertext.to_vec(),
    })
}

fn official_pad(
    plaintext: &[u8],
    padded_len: usize,
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    if plaintext.len() > u16::MAX as usize
        || plaintext
            .len()
            .saturating_add(RADROOTS_SIMPLEX_OFFICIAL_PADDING_LENGTH_BYTES)
            > padded_len
    {
        return Err(RadrootsSimplexSmpCryptoError::InvalidMessageLength {
            actual: plaintext.len(),
            padded: padded_len,
        });
    }
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(&(plaintext.len() as u16).to_be_bytes());
    padded.extend_from_slice(plaintext);
    padded.resize(padded_len, b'#');
    Ok(padded)
}

fn official_unpad(padded: &[u8]) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    if padded.len() < RADROOTS_SIMPLEX_OFFICIAL_PADDING_LENGTH_BYTES {
        return Err(RadrootsSimplexSmpCryptoError::InvalidOfficialRatchetPadding);
    }
    let length = u16::from_be_bytes([padded[0], padded[1]]) as usize;
    if length
        > padded
            .len()
            .saturating_sub(RADROOTS_SIMPLEX_OFFICIAL_PADDING_LENGTH_BYTES)
    {
        return Err(RadrootsSimplexSmpCryptoError::InvalidOfficialRatchetPadding);
    }
    Ok(padded[RADROOTS_SIMPLEX_OFFICIAL_PADDING_LENGTH_BYTES
        ..RADROOTS_SIMPLEX_OFFICIAL_PADDING_LENGTH_BYTES + length]
        .to_vec())
}

fn official_hkdf3(
    salt: &[u8],
    ikm: &[u8],
    info: &[u8],
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), RadrootsSimplexSmpCryptoError> {
    let hkdf = Hkdf::<Sha512>::new(Some(salt), ikm);
    let mut output = [0_u8; RADROOTS_SIMPLEX_OFFICIAL_HKDF3_OUTPUT_LENGTH];
    hkdf.expand(info, &mut output).map_err(|_| {
        RadrootsSimplexSmpCryptoError::InvalidKeyDerivationLength(
            RADROOTS_SIMPLEX_OFFICIAL_HKDF3_OUTPUT_LENGTH,
        )
    })?;
    Ok((
        output[..RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH].to_vec(),
        output[RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
            ..RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH * 2]
            .to_vec(),
        output[RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH * 2..].to_vec(),
    ))
}

fn validate_official_version(version: u16) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if version < RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION
        || version > RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION
    {
        return Err(RadrootsSimplexSmpCryptoError::InvalidOfficialRatchetVersion(version));
    }
    Ok(())
}

fn validate_official_version_range(
    range: RadrootsSimplexSmpVersionRange,
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    validate_official_version(range.min)?;
    validate_official_version(range.max)
}

fn validate_official_x3dh_params(
    params: &RadrootsSimplexOfficialX3dhParams,
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    validate_official_version_range(params.version_range)?;
    if params.key_1.len() != RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(
            params.key_1.len(),
        ));
    }
    if params.key_2.len() != RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(
            params.key_2.len(),
        ));
    }
    if params.pq_ciphertext.is_some() && params.pq_public_key.is_none() {
        return Err(RadrootsSimplexSmpCryptoError::IncompletePqHeader);
    }
    if let Some(pq_public_key) = params.pq_public_key.as_deref() {
        if params.version_range.max < RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION {
            return Err(
                RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                    "PQ key requires E2E version 3".to_owned(),
                ),
            );
        }
        if pq_public_key.len() != RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PUBLIC_KEY_LENGTH {
            return Err(RadrootsSimplexSmpCryptoError::InvalidPqKeyLength(
                pq_public_key.len(),
            ));
        }
    }
    if let Some(pq_ciphertext) = params.pq_ciphertext.as_deref() {
        if pq_ciphertext.len() != RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_CIPHERTEXT_LENGTH {
            return Err(RadrootsSimplexSmpCryptoError::InvalidPqCiphertextLength(
                pq_ciphertext.len(),
            ));
        }
    }
    Ok(())
}

fn validate_official_x3dh_keypair(
    keypair: &RadrootsSimplexOfficialX448Keypair,
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if keypair.public_key.len() != RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(
            keypair.public_key.len(),
        ));
    }
    if keypair.private_key.len() != RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPrivateKeyLength(
            keypair.private_key.len(),
        ));
    }
    Ok(())
}

fn validate_official_sntrup761_keypair(
    keypair: &RadrootsSimplexOfficialSntrup761Keypair,
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if keypair.public_key.len() != RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PUBLIC_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPqKeyLength(
            keypair.public_key.len(),
        ));
    }
    if keypair.private_key.len() != RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PRIVATE_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPrivateKeyLength(
            keypair.private_key.len(),
        ));
    }
    Ok(())
}

fn official_x3dh_init(
    sender_key_1: &[u8],
    receiver_key_1: &[u8],
    shared_secrets: &[Vec<u8>; 3],
    pq_shared_secret: Option<&[u8]>,
) -> Result<RadrootsSimplexOfficialX3dhInit, RadrootsSimplexSmpCryptoError> {
    let mut associated_data = Vec::with_capacity(sender_key_1.len() + receiver_key_1.len());
    associated_data.extend_from_slice(sender_key_1);
    associated_data.extend_from_slice(receiver_key_1);
    let mut input = Vec::with_capacity(
        RADROOTS_SIMPLEX_OFFICIAL_X448_SHARED_SECRET_LENGTH * shared_secrets.len(),
    );
    for shared_secret in shared_secrets {
        if shared_secret.len() != RADROOTS_SIMPLEX_OFFICIAL_X448_SHARED_SECRET_LENGTH {
            return Err(RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(
                shared_secret.len(),
            ));
        }
        input.extend_from_slice(shared_secret);
    }
    if let Some(pq_shared_secret) = pq_shared_secret {
        if pq_shared_secret.len() != RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_SHARED_SECRET_LENGTH {
            return Err(RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(
                pq_shared_secret.len(),
            ));
        }
        input.extend_from_slice(pq_shared_secret);
    }
    let zero_salt = [0_u8; 64];
    let (sending_header_key, receiving_next_header_key, ratchet_key) =
        official_hkdf3(&zero_salt, &input, RADROOTS_SIMPLEX_OFFICIAL_X3DH_INFO)?;
    Ok(RadrootsSimplexOfficialX3dhInit {
        associated_data,
        ratchet_key,
        sending_header_key,
        receiving_next_header_key,
        accepted_pq_shared_secret: pq_shared_secret.map(<[u8]>::to_vec),
    })
}

fn push_official_msg_header_pq(
    buffer: &mut Vec<u8>,
    header: &RadrootsSimplexOfficialMsgHeader,
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    match (
        header.pq_public_key.as_deref(),
        header.pq_ciphertext.as_deref(),
    ) {
        (None, None) => buffer.push(b'0'),
        (Some(pq_public_key), None) => {
            validate_official_pq_public_key(pq_public_key)?;
            buffer.push(b'1');
            buffer.push(b'P');
            push_official_large_by_version(
                buffer,
                RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION,
                pq_public_key,
            )?;
        }
        (Some(pq_public_key), Some(pq_ciphertext)) => {
            validate_official_pq_public_key(pq_public_key)?;
            validate_official_pq_ciphertext(pq_ciphertext)?;
            buffer.push(b'1');
            buffer.push(b'A');
            push_official_large_by_version(
                buffer,
                RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION,
                pq_ciphertext,
            )?;
            push_official_large_by_version(
                buffer,
                RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION,
                pq_public_key,
            )?;
        }
        (None, Some(_)) => return Err(RadrootsSimplexSmpCryptoError::IncompletePqHeader),
    }
    Ok(())
}

fn read_official_msg_header_pq(
    cursor: &mut OfficialCursor<'_>,
) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>), RadrootsSimplexSmpCryptoError> {
    match cursor.read_byte()? {
        b'0' => Ok((None, None)),
        b'1' => match cursor.read_byte()? {
            b'P' => {
                let pq_public_key = cursor.read_official_large()?.to_vec();
                validate_official_pq_public_key(&pq_public_key)?;
                Ok((Some(pq_public_key), None))
            }
            b'A' => {
                let pq_ciphertext = cursor.read_official_large()?.to_vec();
                let pq_public_key = cursor.read_official_large()?.to_vec();
                validate_official_pq_ciphertext(&pq_ciphertext)?;
                validate_official_pq_public_key(&pq_public_key)?;
                Ok((Some(pq_public_key), Some(pq_ciphertext)))
            }
            value => Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(
                value as usize,
            )),
        },
        value => Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(
            value as usize,
        )),
    }
}

fn validate_official_pq_public_key(value: &[u8]) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if value.len() != RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PUBLIC_KEY_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPqKeyLength(
            value.len(),
        ));
    }
    Ok(())
}

fn validate_official_pq_ciphertext(value: &[u8]) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if value.len() != RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_CIPHERTEXT_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidPqCiphertextLength(
            value.len(),
        ));
    }
    Ok(())
}

fn encode_official_urlsafe_bytes(bytes: &[u8]) -> String {
    URL_SAFE.encode(bytes)
}

fn decode_official_urlsafe_bytes(value: &str) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    URL_SAFE
        .decode(value.as_bytes())
        .or_else(|_| URL_SAFE_NO_PAD.decode(value.as_bytes()))
        .map_err(|_| {
            RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                "invalid base64url field".to_owned(),
            )
        })
}

fn split_official_x3dh_keys(value: &str) -> Result<(&str, &str), RadrootsSimplexSmpCryptoError> {
    let (key_1, rest) = value.split_once(',').ok_or_else(|| {
        RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
            "`x3dh` field must contain two keys".to_owned(),
        )
    })?;
    if rest.contains(',') {
        return Err(
            RadrootsSimplexSmpCryptoError::InvalidOfficialX3dhParameters(
                "`x3dh` field must contain two keys".to_owned(),
            ),
        );
    }
    Ok((key_1, rest))
}

fn official_large_prefix_len(version: u16) -> Result<usize, RadrootsSimplexSmpCryptoError> {
    validate_official_version(version)?;
    Ok(if version >= RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION {
        2
    } else {
        1
    })
}

fn push_official_large_by_version(
    buffer: &mut Vec<u8>,
    version: u16,
    value: &[u8],
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if version >= RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION {
        if value.len() > u16::MAX as usize {
            return Err(RadrootsSimplexSmpCryptoError::InvalidMessageLength {
                actual: value.len(),
                padded: u16::MAX as usize,
            });
        }
        buffer.extend_from_slice(&(value.len() as u16).to_be_bytes());
    } else {
        if value.len() > u8::MAX as usize {
            return Err(RadrootsSimplexSmpCryptoError::InvalidMessageLength {
                actual: value.len(),
                padded: u8::MAX as usize,
            });
        }
        buffer.push(value.len() as u8);
    }
    buffer.extend_from_slice(value);
    Ok(())
}

fn push_official_short_bytes(
    buffer: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if value.len() > u8::MAX as usize {
        return Err(RadrootsSimplexSmpCryptoError::InvalidShortFieldLength(
            value.len(),
        ));
    }
    buffer.push(value.len() as u8);
    buffer.extend_from_slice(value);
    Ok(())
}

struct OfficialCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> OfficialCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn finish(&self) -> Result<(), RadrootsSimplexSmpCryptoError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(
                self.bytes.len() - self.position,
            ))
        }
    }

    fn read_u16(&mut self) -> Result<u16, RadrootsSimplexSmpCryptoError> {
        let bytes = self.read_slice(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, RadrootsSimplexSmpCryptoError> {
        let bytes = self.read_slice(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_byte(&mut self) -> Result<u8, RadrootsSimplexSmpCryptoError> {
        let Some(value) = self.bytes.get(self.position) else {
            return Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(0));
        };
        self.position += 1;
        Ok(*value)
    }

    fn read_short_bytes(&mut self) -> Result<&'a [u8], RadrootsSimplexSmpCryptoError> {
        let length = self.read_byte()? as usize;
        self.read_slice(length)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], RadrootsSimplexSmpCryptoError> {
        let bytes = self.read_slice(N)?;
        let mut value = [0_u8; N];
        value.copy_from_slice(bytes);
        Ok(value)
    }

    fn read_slice(&mut self, len: usize) -> Result<&'a [u8], RadrootsSimplexSmpCryptoError> {
        let Some(bytes) = self.bytes.get(self.position..self.position + len) else {
            return Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(
                self.bytes.len().saturating_sub(self.position),
            ));
        };
        self.position += len;
        Ok(bytes)
    }

    fn read_official_large(&mut self) -> Result<&'a [u8], RadrootsSimplexSmpCryptoError> {
        let first = *self
            .bytes
            .get(self.position)
            .ok_or(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(0))?;
        let len = if first < 32 {
            self.read_u16()? as usize
        } else {
            self.position += 1;
            first as usize
        };
        self.read_slice(len)
    }

    fn read_remaining(&mut self) -> &'a [u8] {
        let bytes = &self.bytes[self.position..];
        self.position = self.bytes.len();
        bytes
    }
}

fn pq_seed(seed: &[u8]) -> [u8; RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH] {
    let digest = Sha256::digest(seed);
    let mut value = [0_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH];
    value.copy_from_slice(&digest);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_header_lengths_match_upstream_constants() {
        assert_eq!(official_ratchet_header_len(2, false).unwrap(), 88);
        assert_eq!(official_ratchet_header_len(3, false).unwrap(), 88);
        assert_eq!(official_ratchet_header_len(3, true).unwrap(), 2_310);
        assert_eq!(official_full_header_len(3, false).unwrap(), 123);
        assert_eq!(official_full_header_len(3, true).unwrap(), 2_345);
        assert_eq!(
            official_encoded_encrypted_header_len(2, false).unwrap(),
            123
        );
        assert_eq!(
            official_encoded_encrypted_header_len(3, false).unwrap(),
            124
        );
        assert_eq!(
            official_encoded_encrypted_header_len(3, true).unwrap(),
            2_346
        );
        assert_eq!(
            official_encoded_encrypted_message_len(3, false, 15_840).unwrap(),
            15_982
        );
    }

    #[test]
    fn x448_key_agreement_roundtrips() {
        let alice = official_x448_keypair_from_seed(b"rr-synth-official-alice-x448");
        let bob = official_x448_keypair_from_seed(b"rr-synth-official-bob-x448");

        let alice_secret =
            derive_official_x448_shared_secret(&alice.private_key, &bob.public_key).unwrap();
        let bob_secret =
            derive_official_x448_shared_secret(&bob.private_key, &alice.public_key).unwrap();

        assert_eq!(
            alice.public_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH
        );
        assert_eq!(
            alice.private_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH
        );
        assert_eq!(
            alice_secret.len(),
            RADROOTS_SIMPLEX_OFFICIAL_X448_SHARED_SECRET_LENGTH
        );
        assert_eq!(alice_secret, bob_secret);
    }

    #[test]
    fn official_x448_der_public_key_roundtrips() {
        let keypair = official_x448_keypair_from_seed(b"rr-synth-official-der-x448");
        let encoded = encode_official_x448_public_key_der(&keypair.public_key).unwrap();
        assert_eq!(encoded.len(), 68);
        assert_eq!(
            decode_official_x448_public_key_der(&encoded).unwrap(),
            keypair.public_key
        );
    }

    #[test]
    fn official_no_pq_msg_header_roundtrips() {
        let keypair = official_x448_keypair_from_seed(b"rr-synth-official-header-x448");
        let header = RadrootsSimplexOfficialMsgHeader {
            max_version: RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            dh_public_key: keypair.public_key,
            pq_public_key: None,
            pq_ciphertext: None,
            previous_sending_chain_length: 5,
            message_number: 8,
        };
        let encoded =
            encode_official_msg_header(RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION, &header)
                .unwrap();
        assert_eq!(encoded.len(), 80);
        assert_eq!(
            decode_official_msg_header(RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION, &encoded)
                .unwrap(),
            header
        );
    }

    #[test]
    fn official_pq_msg_headers_roundtrip_proposed_and_accepted_kem() {
        let keypair = official_x448_keypair_from_seed(b"rr-synth-official-header-pq-x448");
        let pq_keypair = official_sntrup761_keypair_from_seed(b"rr-synth-official-header-pq-kem");
        let (pq_ciphertext, _) =
            encapsulate_official_sntrup761(&pq_keypair.public_key, b"rr-synth-header-pq-ct")
                .unwrap();
        let proposed = RadrootsSimplexOfficialMsgHeader {
            max_version: RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            dh_public_key: keypair.public_key.clone(),
            pq_public_key: Some(pq_keypair.public_key.clone()),
            pq_ciphertext: None,
            previous_sending_chain_length: 5,
            message_number: 8,
        };
        let encoded_proposed =
            encode_official_msg_header(RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION, &proposed)
                .unwrap();
        assert_eq!(encoded_proposed.len(), 1_241);
        assert_eq!(
            decode_official_msg_header(
                RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
                &encoded_proposed
            )
            .unwrap(),
            proposed
        );

        let accepted = RadrootsSimplexOfficialMsgHeader {
            max_version: RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            dh_public_key: keypair.public_key,
            pq_public_key: Some(pq_keypair.public_key),
            pq_ciphertext: Some(pq_ciphertext),
            previous_sending_chain_length: 9,
            message_number: 10,
        };
        let encoded_accepted =
            encode_official_msg_header(RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION, &accepted)
                .unwrap();
        assert_eq!(encoded_accepted.len(), 2_282);
        assert_eq!(
            decode_official_msg_header(
                RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
                &encoded_accepted
            )
            .unwrap(),
            accepted
        );
    }

    #[test]
    fn official_x3dh_params_uri_roundtrips() {
        let keypair_1 = official_x448_keypair_from_seed(b"rr-synth-official-x3dh-1");
        let keypair_2 = official_x448_keypair_from_seed(b"rr-synth-official-x3dh-2");
        let params = RadrootsSimplexOfficialX3dhParams {
            version_range: RadrootsSimplexSmpVersionRange::new(
                RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION,
                RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            )
            .unwrap(),
            key_1: keypair_1.public_key,
            key_2: keypair_2.public_key,
            pq_public_key: None,
            pq_ciphertext: None,
        };
        let encoded = encode_official_x3dh_params_uri(&params).unwrap();
        assert!(encoded.starts_with("v=2-3&x3dh=MEIwBQYDK2VvAzkA"));
        assert!(encoded.contains(','));
        assert_eq!(decode_official_x3dh_params_uri(&encoded).unwrap(), params);
    }

    #[test]
    fn official_x3dh_params_rejects_incomplete_pq_fields() {
        let keypair_1 = official_x448_keypair_from_seed(b"rr-synth-official-x3dh-pq-1");
        let keypair_2 = official_x448_keypair_from_seed(b"rr-synth-official-x3dh-pq-2");
        let params = RadrootsSimplexOfficialX3dhParams {
            version_range: RadrootsSimplexSmpVersionRange::single(
                RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            ),
            key_1: keypair_1.public_key,
            key_2: keypair_2.public_key,
            pq_public_key: None,
            pq_ciphertext: Some(vec![
                0_u8;
                RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_CIPHERTEXT_LENGTH
            ]),
        };
        assert_eq!(
            encode_official_x3dh_params_uri(&params).unwrap_err(),
            RadrootsSimplexSmpCryptoError::IncompletePqHeader
        );
    }

    #[test]
    fn official_x3dh_no_pq_init_matches_on_both_sides() {
        let receiver_key_1 = official_x448_keypair_from_seed(b"rr-synth-x3dh-rcv-1");
        let receiver_key_2 = official_x448_keypair_from_seed(b"rr-synth-x3dh-rcv-2");
        let sender_key_1 = official_x448_keypair_from_seed(b"rr-synth-x3dh-snd-1");
        let sender_key_2 = official_x448_keypair_from_seed(b"rr-synth-x3dh-snd-2");
        let receiver_params = RadrootsSimplexOfficialX3dhParams {
            version_range: RadrootsSimplexSmpVersionRange::new(
                RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION,
                RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            )
            .unwrap(),
            key_1: receiver_key_1.public_key.clone(),
            key_2: receiver_key_2.public_key.clone(),
            pq_public_key: None,
            pq_ciphertext: None,
        };
        let sender_params = RadrootsSimplexOfficialX3dhParams {
            version_range: receiver_params.version_range,
            key_1: sender_key_1.public_key.clone(),
            key_2: sender_key_2.public_key.clone(),
            pq_public_key: None,
            pq_ciphertext: None,
        };

        let sender_init =
            official_x3dh_sender_init(&sender_key_1, &sender_key_2, &receiver_params).unwrap();
        let receiver_init =
            official_x3dh_receiver_init(&receiver_key_1, &receiver_key_2, &sender_params).unwrap();

        assert_eq!(sender_init, receiver_init);
        assert_eq!(
            sender_init.associated_data,
            [sender_key_1.public_key, receiver_key_1.public_key].concat()
        );
        assert_eq!(
            sender_init.ratchet_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
        );
        assert_eq!(
            sender_init.sending_header_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
        );
        assert_eq!(
            sender_init.receiving_next_header_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
        );
        assert!(sender_init.accepted_pq_shared_secret.is_none());
    }

    #[test]
    fn official_x3dh_pq_init_matches_on_both_sides() {
        let receiver_key_1 = official_x448_keypair_from_seed(b"rr-synth-x3dh-pq-rcv-1");
        let receiver_key_2 = official_x448_keypair_from_seed(b"rr-synth-x3dh-pq-rcv-2");
        let sender_key_1 = official_x448_keypair_from_seed(b"rr-synth-x3dh-pq-snd-1");
        let sender_key_2 = official_x448_keypair_from_seed(b"rr-synth-x3dh-pq-snd-2");
        let receiver_pq = official_sntrup761_keypair_from_seed(b"rr-synth-x3dh-pq-rcv-kem");
        let sender_pq = official_sntrup761_keypair_from_seed(b"rr-synth-x3dh-pq-snd-kem");
        let receiver_params = RadrootsSimplexOfficialX3dhParams {
            version_range: RadrootsSimplexSmpVersionRange::new(
                RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION,
                RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            )
            .unwrap(),
            key_1: receiver_key_1.public_key.clone(),
            key_2: receiver_key_2.public_key.clone(),
            pq_public_key: Some(receiver_pq.public_key.clone()),
            pq_ciphertext: None,
        };

        let sender_init = official_x3dh_sender_init_accepting_pq(
            &sender_key_1,
            &sender_key_2,
            sender_pq.clone(),
            &receiver_params,
            b"rr-synth-x3dh-pq-encap",
        )
        .unwrap();
        let receiver_init = official_x3dh_receiver_init_accepting_pq(
            &receiver_key_1,
            &receiver_key_2,
            &receiver_pq,
            &sender_init.sender_params,
        )
        .unwrap();

        assert_eq!(sender_init.init, receiver_init.init);
        assert_eq!(sender_init.pq_shared_secret, receiver_init.pq_shared_secret);
        assert_eq!(
            sender_init.init.accepted_pq_shared_secret.as_deref(),
            Some(sender_init.pq_shared_secret.as_slice())
        );
        assert_eq!(
            sender_init.sender_params.pq_public_key,
            Some(sender_pq.public_key)
        );
        assert!(sender_init.sender_params.pq_ciphertext.is_some());
    }

    #[test]
    fn sntrup761_encapsulation_roundtrips() {
        let recipient = official_sntrup761_keypair_from_seed(b"rr-synth-official-pq-recipient");
        let (ciphertext, sender_secret) =
            encapsulate_official_sntrup761(&recipient.public_key, b"rr-synth-official-pq-send")
                .unwrap();
        let receiver_secret =
            decapsulate_official_sntrup761(&recipient.private_key, &ciphertext).unwrap();

        assert_eq!(
            recipient.public_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PUBLIC_KEY_LENGTH
        );
        assert_eq!(
            recipient.private_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PRIVATE_KEY_LENGTH
        );
        assert_eq!(
            ciphertext.len(),
            RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_CIPHERTEXT_LENGTH
        );
        assert_eq!(
            sender_secret.len(),
            RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_SHARED_SECRET_LENGTH
        );
        assert_eq!(sender_secret, receiver_secret);
    }

    #[test]
    fn official_aes_gcm_padding_authenticates_associated_data() {
        let key = [11_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH];
        let iv = [12_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH];
        let associated_data = b"rr-synth-official-associated-data";
        let payload = official_aes_gcm_encrypt_padded(
            &key,
            &iv,
            b"hello official simplex",
            96,
            associated_data,
        )
        .unwrap();

        assert_eq!(
            payload.auth_tag.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH
        );
        assert_eq!(payload.ciphertext.len(), 96);
        assert_ne!(payload.ciphertext, b"hello official simplex");
        assert_eq!(
            official_aes_gcm_decrypt_padded(&key, &iv, &payload, associated_data).unwrap(),
            b"hello official simplex"
        );
        assert!(matches!(
            official_aes_gcm_decrypt_padded(&key, &iv, &payload, b"wrong-ad").unwrap_err(),
            RadrootsSimplexSmpCryptoError::AesGcmAuthenticationFailed
        ));
    }

    #[test]
    fn official_encrypted_header_and_message_wire_roundtrip() {
        let header = RadrootsSimplexOfficialEncryptedHeader {
            version: RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            iv: [21_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH],
            auth_tag: vec![22_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH],
            body: vec![23_u8; RADROOTS_SIMPLEX_OFFICIAL_RATCHET_HEADER_LENGTH],
        };
        let encoded_header = encode_official_encrypted_header(&header).unwrap();
        assert_eq!(encoded_header.len(), 124);
        assert_eq!(
            decode_official_encrypted_header(&encoded_header).unwrap(),
            header
        );

        let message = RadrootsSimplexOfficialEncryptedMessage {
            encrypted_header: encoded_header,
            auth_tag: vec![24_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH],
            body: vec![25_u8; 96],
        };
        let encoded = encode_official_encrypted_message(
            RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            &message,
        )
        .unwrap();
        assert_eq!(encoded.len(), 2 + 124 + 16 + 96);
        assert_eq!(
            decode_official_encrypted_message(&encoded).unwrap(),
            message
        );
    }

    #[test]
    fn official_encrypted_message_rejects_malformed_wire_lengths() {
        let header = RadrootsSimplexOfficialEncryptedHeader {
            version: 3,
            iv: [31_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH],
            auth_tag: vec![32_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH],
            body: vec![33_u8; RADROOTS_SIMPLEX_OFFICIAL_RATCHET_HEADER_LENGTH],
        };
        let mut encoded_header = encode_official_encrypted_header(&header).unwrap();
        encoded_header.truncate(encoded_header.len() - 1);
        assert!(matches!(
            decode_official_encrypted_header(&encoded_header).unwrap_err(),
            RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(_)
        ));

        let message = RadrootsSimplexOfficialEncryptedMessage {
            encrypted_header: encode_official_encrypted_header(&header).unwrap(),
            auth_tag: vec![34_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH - 1],
            body: vec![35_u8; 32],
        };
        assert!(matches!(
            encode_official_encrypted_message(3, &message).unwrap_err(),
            RadrootsSimplexSmpCryptoError::InvalidSignatureLength(_)
        ));
    }

    #[test]
    fn official_kdfs_split_root_and_chain_material() {
        let root = official_root_kdf(
            &[1_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH],
            &[2_u8; RADROOTS_SIMPLEX_OFFICIAL_X448_SHARED_SECRET_LENGTH],
            Some(&[3_u8; RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_SHARED_SECRET_LENGTH]),
        )
        .unwrap();
        let chain = official_chain_kdf(&root.chain_key).unwrap();

        assert_eq!(
            root.root_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
        );
        assert_eq!(
            root.chain_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
        );
        assert_eq!(
            root.next_header_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
        );
        assert_eq!(
            chain.chain_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
        );
        assert_eq!(
            chain.message_key.len(),
            RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH
        );
        assert_ne!(chain.message_iv, chain.header_iv);
    }
}
