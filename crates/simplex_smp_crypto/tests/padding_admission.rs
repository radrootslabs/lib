use radroots_simplex_smp_crypto::prelude::{
    RadrootsSimplexSmpCryptoError, decrypt_padded, encrypt_padded,
};

const KEY: [u8; 32] = [7; 32];
const NONCE: [u8; 24] = [9; 24];

#[test]
fn accepted_payloads_round_trip_without_losing_bytes() {
    for length in [0_usize, 1, 4096, usize::from(u16::MAX)] {
        let plaintext: Vec<u8> = (0..length).map(|index| (index % 251) as u8).collect();
        for extra_padding in [0, 3] {
            let ciphertext = encrypt_padded(&KEY, &NONCE, &plaintext, length + 2 + extra_padding)
                .expect("representable plaintext must fit the padded message");
            assert_eq!(
                decrypt_padded(&KEY, &NONCE, &ciphertext).unwrap(),
                plaintext,
                "accepted plaintext must preserve every byte"
            );
        }
    }
}

#[test]
fn unrepresentable_payload_lengths_are_rejected() {
    for length in [usize::from(u16::MAX) + 1, usize::from(u16::MAX) + 2] {
        let plaintext = vec![0xa5; length];
        let result = encrypt_padded(&KEY, &NONCE, &plaintext, length + 2);
        if let Ok(ciphertext) = &result {
            let decoded = decrypt_padded(&KEY, &NONCE, ciphertext).unwrap();
            assert_eq!(
                decoded.len(),
                plaintext.len(),
                "an accepted encoding must never lose plaintext bytes"
            );
        }
        assert!(matches!(
            result,
            Err(RadrootsSimplexSmpCryptoError::InvalidMessageLength { actual, .. })
                if actual == length
        ));
    }
}

#[test]
fn insufficient_padding_capacity_is_rejected() {
    assert!(matches!(
        encrypt_padded(&KEY, &NONCE, b"abc", 4),
        Err(RadrootsSimplexSmpCryptoError::InvalidMessageLength {
            actual: 3,
            padded: 4
        })
    ));
}
