use super::*;
use serde::de::{Error, Unexpected, value};

#[test]
fn root_visitor_expecting_message_is_stable() {
    let error = value::Error::invalid_type(Unexpected::Bool(true), &ProfileMetadataRootVisitor);
    assert!(error.to_string().contains("Profile metadata JSON"));
}
