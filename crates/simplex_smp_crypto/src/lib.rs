#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod auth;
pub mod error;
pub mod message;
pub mod official_ratchet;
pub mod ratchet;

pub mod prelude {
    pub use crate::auth::{
        RadrootsSimplexSmpCommandAuthorization, RadrootsSimplexSmpEd25519Keypair,
        RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
        decode_x25519_public_key_x509, encode_ed25519_public_key_x509,
        encode_x25519_public_key_x509, verify_signature,
    };
    pub use crate::error::RadrootsSimplexSmpCryptoError;
    pub use crate::message::{
        RADROOTS_SIMPLEX_SMP_NONCE_LENGTH, RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH,
        RadrootsSimplexSmpSecretBoxChainKey, RadrootsSimplexSmpX25519Keypair,
        advance_secretbox_chain, decrypt_no_pad, decrypt_padded, derive_shared_secret,
        encrypt_no_pad, encrypt_padded, init_secretbox_chain, random_nonce,
    };
    pub use crate::official_ratchet::{
        RADROOTS_SIMPLEX_OFFICIAL_AES_AUTH_TAG_LENGTH, RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH,
        RADROOTS_SIMPLEX_OFFICIAL_AES_KEY_LENGTH, RADROOTS_SIMPLEX_OFFICIAL_CHAIN_RATCHET_INFO,
        RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION, RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION,
        RADROOTS_SIMPLEX_OFFICIAL_E2E_PQ_VERSION,
        RADROOTS_SIMPLEX_OFFICIAL_PQ_RATCHET_HEADER_LENGTH,
        RADROOTS_SIMPLEX_OFFICIAL_RATCHET_HEADER_LENGTH,
        RADROOTS_SIMPLEX_OFFICIAL_ROOT_RATCHET_INFO,
        RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_CIPHERTEXT_LENGTH,
        RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PRIVATE_KEY_LENGTH,
        RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_PUBLIC_KEY_LENGTH,
        RADROOTS_SIMPLEX_OFFICIAL_SNTRUP761_SHARED_SECRET_LENGTH,
        RADROOTS_SIMPLEX_OFFICIAL_X3DH_INFO, RADROOTS_SIMPLEX_OFFICIAL_X448_KEY_LENGTH,
        RADROOTS_SIMPLEX_OFFICIAL_X448_SHARED_SECRET_LENGTH, RadrootsSimplexOfficialAesGcmPayload,
        RadrootsSimplexOfficialChainKdfOutput, RadrootsSimplexOfficialEncryptedHeader,
        RadrootsSimplexOfficialEncryptedMessage, RadrootsSimplexOfficialMsgHeader,
        RadrootsSimplexOfficialRootKdfOutput, RadrootsSimplexOfficialSntrup761Keypair,
        RadrootsSimplexOfficialX3dhInit, RadrootsSimplexOfficialX3dhParams,
        RadrootsSimplexOfficialX448Keypair, decapsulate_official_sntrup761,
        decode_official_encrypted_header, decode_official_encrypted_message,
        decode_official_msg_header, decode_official_x3dh_params_uri,
        decode_official_x448_public_key_der, derive_official_x448_shared_secret,
        encapsulate_official_sntrup761, encode_official_encrypted_header,
        encode_official_encrypted_message, encode_official_msg_header,
        encode_official_x3dh_params_uri, encode_official_x448_public_key_der,
        generate_official_sntrup761_keypair, generate_official_x448_keypair,
        official_aes_gcm_decrypt_padded, official_aes_gcm_encrypt_padded, official_chain_kdf,
        official_encoded_encrypted_header_len, official_encoded_encrypted_message_len,
        official_full_header_len, official_ratchet_header_len, official_root_kdf,
        official_sntrup761_keypair_from_seed, official_x3dh_receiver_init,
        official_x3dh_sender_init, official_x448_keypair_from_seed,
    };
    pub use crate::ratchet::{
        RadrootsSimplexSmpRatchetHeader, RadrootsSimplexSmpRatchetRole,
        RadrootsSimplexSmpRatchetState,
    };
}
