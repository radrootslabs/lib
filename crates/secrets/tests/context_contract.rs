use radroots_secrets::Error;
use radroots_secrets::context::{
    ENVELOPE_CONTEXT_DOMAIN, ENVELOPE_CONTEXT_VERSION, ENVELOPE_PURPOSE_MAX_BYTES,
    ENVELOPE_SUBJECT_TYPE_MAX_BYTES, ENVELOPE_SUBJECT_VALUE_MAX_BYTES, EnvelopeContext,
    EnvelopePurpose, EnvelopeSubject, PAYLOAD_SCHEMA_MAX_BYTES, PayloadSchemaId,
};
use radroots_secrets::error::{ContextField, ContextValueError};

fn context() -> EnvelopeContext {
    EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.private_artifact").expect("purpose"),
        EnvelopeSubject::parse("private_artifact", "01010101010101010101010101010101")
            .expect("subject"),
        PayloadSchemaId::parse("trade.private_terms.v1").expect("schema"),
    )
}

#[test]
fn context_encoding_is_canonical_and_domain_separated() {
    let encoded = context().to_canonical_bytes();
    assert_eq!(&encoded[..2], &ENVELOPE_CONTEXT_VERSION.to_be_bytes());
    assert_eq!(
        &encoded[2..2 + ENVELOPE_CONTEXT_DOMAIN.len()],
        ENVELOPE_CONTEXT_DOMAIN
    );
    assert_eq!(encoded, context().to_canonical_bytes());
}

#[test]
fn context_parts_accept_exact_boundaries() {
    let purpose = format!("a.{}", "b".repeat(ENVELOPE_PURPOSE_MAX_BYTES - 2));
    let subject_type = format!("a{}", "b".repeat(ENVELOPE_SUBJECT_TYPE_MAX_BYTES - 1));
    let subject_value = "a".repeat(ENVELOPE_SUBJECT_VALUE_MAX_BYTES);
    let schema = format!("a.{}", "b".repeat(PAYLOAD_SCHEMA_MAX_BYTES - 2));
    assert!(EnvelopePurpose::parse(purpose).is_ok());
    assert!(EnvelopeSubject::parse(subject_type, subject_value).is_ok());
    assert!(PayloadSchemaId::parse(schema).is_ok());
}

#[test]
fn invalid_context_is_rejected_without_echoing_values() {
    let cases = [
        (
            EnvelopePurpose::parse("").err(),
            ContextField::Purpose,
            ContextValueError::Empty,
        ),
        (
            EnvelopePurpose::parse("Not.namespaced").err(),
            ContextField::Purpose,
            ContextValueError::NonCanonical,
        ),
        (
            EnvelopeSubject::parse("private artifact", "subject").err(),
            ContextField::SubjectType,
            ContextValueError::NonCanonical,
        ),
        (
            EnvelopeSubject::parse("private_artifact", "SUBJECT-SECRET").err(),
            ContextField::SubjectValue,
            ContextValueError::NonCanonical,
        ),
        (
            PayloadSchemaId::parse("schema\n.v1").err(),
            ContextField::PayloadSchema,
            ContextValueError::NonCanonical,
        ),
    ];
    for (error, field, reason) in cases {
        let error = error.expect("invalid context");
        assert_eq!(error, Error::InvalidContextValue { field, reason });
        assert!(!error.to_string().contains("SUBJECT-SECRET"));
    }
}

#[test]
fn diagnostics_redact_semantic_values() {
    let context = context();
    let debug = format!("{context:?}");
    assert!(debug.contains("private_artifact"));
    assert!(!debug.contains("01010101010101010101010101010101"));
    assert!(!debug.contains("trade.private_terms.v1"));
}

#[cfg(feature = "serde")]
#[test]
fn serde_revalidates_context_parts() {
    let encoded = serde_json::to_vec(&context()).expect("serialize");
    let decoded: EnvelopeContext = serde_json::from_slice(&encoded).expect("deserialize");
    assert_eq!(decoded, context());
    let invalid = br#"{"purpose":"radroots.private_artifact","subject_type":"private_artifact","subject":"INVALID","payload_schema":"trade.private_terms.v1"}"#;
    assert!(serde_json::from_slice::<EnvelopeContext>(invalid).is_err());
}
