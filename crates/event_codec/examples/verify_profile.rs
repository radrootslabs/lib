use radroots_event_codec::{admission, decode, verify};

const PROFILE_EVENT: &str = r#"{"id":"762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1800000100,"kind":0,"tags":[],"content":"{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}","sig":"4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109"}"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = decode::event(PROFILE_EVENT)?;
    let verified = verify::verify_nip01_event(raw.into_event())?;
    let admitted = admission::admit_verified_event(verified)?;

    assert_eq!(admitted.event().kind_u32(), 0);
    println!("admitted {}", admitted.contract_id());
    Ok(())
}
