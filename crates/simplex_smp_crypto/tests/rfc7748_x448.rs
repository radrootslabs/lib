use radroots_simplex_smp_crypto::official_ratchet::derive_official_x448_shared_secret;

fn decode_56(encoded: &str) -> [u8; 56] {
    assert_eq!(encoded.len(), 112);
    core::array::from_fn(|index| {
        u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16).unwrap()
    })
}

#[test]
fn official_x448_matches_independent_rfc7748_vectors() {
    // Public synthetic test inputs from RFC 7748 section 5.2, not production keys.
    // https://www.rfc-editor.org/rfc/rfc7748.html#section-5.2
    let vectors = [
        (
            concat!(
                "3d262fddf9ec8e88495266fea19a34d28882acef045104d0d1aae121",
                "700a779c984c24f8cdd78fbff44943eba368f54b29259a4f1c600ad3"
            ),
            concat!(
                "06fce640fa3487bfda5f6cf2d5263f8aad88334cbd07437f020f08f9",
                "814dc031ddbdc38c19c6da2583fa5429db94ada18aa7a7fb4ef8a086"
            ),
            concat!(
                "ce3e4ff95a60dc6697da1db1d85e6afbdf79b50a2412d7546d5f239f",
                "e14fbaadeb445fc66a01b0779d98223961111e21766282f73dd96b6f"
            ),
        ),
        (
            concat!(
                "203d494428b8399352665ddca42f9de8fef600908e0d461cb021f8c5",
                "38345dd77c3e4806e25f46d3315c44e0a5b4371282dd2c8d5be3095f"
            ),
            concat!(
                "0fbcc2f993cd56d3305b0b7d9e55d4c1a8fb5dbb52f8e9a1e9b6201b",
                "165d015894e56c4d3570bee52fe205e28a78b91cdfbde71ce8d157db"
            ),
            concat!(
                "884a02576239ff7a2f2f63b2db6a9ff37047ac13568e1e30fe63c4a7",
                "ad1b3ee3a5700df34321d62077e63633c575c1c954514e99da7c179d"
            ),
        ),
    ];

    for (scalar, public, expected) in vectors {
        assert_eq!(
            derive_official_x448_shared_secret(&decode_56(scalar), &decode_56(public)).unwrap(),
            decode_56(expected)
        );
    }
}
