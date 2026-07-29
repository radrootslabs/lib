use std::{fmt::Debug, str::FromStr};

use radroots_blossom::{
    AuthorizationClaim, BlobUrl, MediaType, Sha256,
    authorization::{AuthorizationAction, AuthorizationContent, ServerDomain},
    hash::{FileExtension, HashPath},
};

const ASCII_MUTATION_ALPHABET: &[u8] =
    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._/:?#%\\ @\t\n\r\0;=+";

#[test]
fn deterministic_parser_mutation_corpus_never_panics_and_round_trips_successes() {
    let fixed = [
        "",
        "get",
        "cdn.example.com",
        "text/plain",
        "png",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "/e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.png",
        "https://cdn.example.com/e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.png",
        "upload image",
    ];

    for value in fixed
        .into_iter()
        .map(str::to_owned)
        .chain((0_u64..4_096).map(mutation_string))
    {
        assert_parse_display_round_trip::<Sha256>(&value);
        assert_parse_display_round_trip::<FileExtension>(&value);
        assert_parse_display_round_trip::<HashPath>(&value);
        assert_parse_display_round_trip::<BlobUrl>(&value);
        assert_parse_display_round_trip::<MediaType>(&value);
        assert_parse_display_round_trip::<AuthorizationAction>(&value);
        assert_parse_display_round_trip::<AuthorizationContent>(&value);
        assert_parse_display_round_trip::<ServerDomain>(&value);

        let tags = vec![
            vec!["t".to_owned(), value.clone()],
            vec!["expiration".to_owned(), value.clone()],
            vec!["server".to_owned(), value.clone()],
            vec!["x".to_owned(), value.clone()],
        ];
        let _ = AuthorizationClaim::parse(&value, 1_800_000_000, &tags);
    }
}

fn assert_parse_display_round_trip<T>(value: &str)
where
    T: FromStr + ToString + PartialEq + Debug,
    T::Err: Debug,
{
    let Ok(parsed) = value.parse::<T>() else {
        return;
    };
    let canonical = parsed.to_string();
    assert_eq!(canonical.parse::<T>().unwrap(), parsed);
}

fn mutation_string(seed: u64) -> String {
    let mut state = seed.wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let length = usize::try_from(next(&mut state) % 385).unwrap();
    let mut value = String::with_capacity(length);
    for _ in 0..length {
        let index = usize::try_from(next(&mut state)).unwrap() % ASCII_MUTATION_ALPHABET.len();
        value.push(char::from(ASCII_MUTATION_ALPHABET[index]));
    }
    value
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
