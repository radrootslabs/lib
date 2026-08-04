#![cfg(feature = "json")]

use radroots_event_codec::decode::EventParseError;
use radroots_event_codec::decode::relay_document::from_json;
use radroots_event_codec::encode::relay_document::to_json;

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
