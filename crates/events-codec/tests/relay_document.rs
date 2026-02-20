#![cfg(feature = "serde_json")]

use radroots_events_codec::error::EventParseError;
use radroots_events_codec::relay_document::decode::from_json;
use radroots_events_codec::relay_document::encode::to_json;

#[test]
fn relay_document_roundtrip_json() {
    let input = r#"{"name":"relay","supported_nips":[1,2],"software":"radroots"}"#;
    let doc = from_json(input).unwrap();
    let output = to_json(&doc).unwrap();

    let v_in: serde_json::Value = serde_json::from_str(input).unwrap();
    let v_out: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v_out, v_in);
}

#[test]
fn relay_document_rejects_invalid_json() {
    let err = from_json("{").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidJson("relay_document")
    ));
}
