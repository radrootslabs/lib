#![forbid(unsafe_code)]

#[cfg(all(feature = "serde_json", not(feature = "std")))]
#[test]
fn serde_json_validation_receipt_api_executes_without_std_feature() {
    use radroots_trade::validation_receipt::{
        RadrootsValidationReceiptError, validation_receipt_content_from_str,
    };

    let error = validation_receipt_content_from_str("{}")
        .expect_err("an incomplete receipt must fail strict decoding");

    assert_eq!(error, RadrootsValidationReceiptError::InvalidJson);
    assert_eq!(error.to_string(), "invalid validation receipt json");
}
