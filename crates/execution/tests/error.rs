use std::{collections::BTreeSet, error::Error as _, mem::size_of};

use radroots_domain::OperationId;
use radroots_execution::{
    Admission, Code, Detail, Error, Field, InvalidErrorContext, OperationContext, Reason,
    RetryClass,
};

const FIRST: OperationId = OperationId::from_bytes([7; 32]);
const SECOND: OperationId = OperationId::from_bytes([8; 32]);

#[test]
fn proposed_identity_never_claims_durable_admission() {
    for id in [None, Some(FIRST), Some(SECOND)] {
        let error = Error::new(
            Code::InvalidInput,
            OperationContext::proposed(id),
            RetryClass::Never,
            Some(Detail::new(Field::Currency, Reason::Unsupported)),
        )
        .unwrap();
        assert_eq!(error.context().operation_id(), id);
        assert_eq!(error.context().admission(), Admission::NotAdmitted);
        assert_eq!(error.code(), Code::InvalidInput);
        assert_eq!(error.retry(), RetryClass::Never);
        let detail = error.detail().unwrap();
        assert_eq!(detail.field(), Field::Currency);
        assert_eq!(detail.reason(), Reason::Unsupported);
        assert_eq!(error.to_string(), "invalid_input: Command input is invalid");
        assert!(error.source().is_none());
    }
}

#[test]
fn uncertain_initial_commit_preserves_id_without_claiming_admission() {
    for id in [FIRST, SECOND] {
        for retry in [
            RetryClass::SafeSameOperation,
            RetryClass::ManualReconciliation,
        ] {
            let error = Error::new(
                Code::CommitUnknown,
                OperationContext::admission_unknown(id),
                retry,
                None,
            )
            .unwrap();
            assert_eq!(error.context().operation_id(), Some(id));
            assert_eq!(error.context().admission(), Admission::Unknown);
            assert_eq!(error.retry(), retry);
            assert_eq!(error.detail(), None);
        }
    }
    assert_eq!(
        Error::new(
            Code::CommitUnknown,
            OperationContext::proposed(Some(FIRST)),
            RetryClass::ManualReconciliation,
            None
        ),
        Err(InvalidErrorContext::OperationIdentityRequired)
    );
}

#[test]
fn signing_delivery_and_tampering_cannot_precede_confirmed_preparation() {
    for code in [
        Code::SigningPending,
        Code::SigningUnknown,
        Code::DeliveryPending,
        Code::SignerTampering,
    ] {
        for context in [
            OperationContext::proposed(None),
            OperationContext::proposed(Some(FIRST)),
            OperationContext::admission_unknown(FIRST),
        ] {
            assert_eq!(
                Error::new(code, context, RetryClass::ManualReconciliation, None),
                Err(InvalidErrorContext::ConfirmedAdmissionRequired)
            );
        }
        for retry in [
            RetryClass::SafeSameOperation,
            RetryClass::ManualReconciliation,
        ] {
            let error = Error::new(code, OperationContext::retained(FIRST), retry, None).unwrap();
            assert_eq!(error.context().admission(), Admission::Confirmed);
            assert_eq!(error.context().operation_id(), Some(FIRST));
        }
    }
}

#[test]
fn uncertain_work_cannot_be_reprepared_using_fresh_state_or_forgotten() {
    for code in [
        Code::CommitUnknown,
        Code::SigningPending,
        Code::SigningUnknown,
        Code::DeliveryPending,
        Code::SignerTampering,
    ] {
        for retry in [RetryClass::Never, RetryClass::AfterStateRefresh] {
            assert_eq!(
                Error::new(code, OperationContext::retained(FIRST), retry, None),
                Err(InvalidErrorContext::ReconciliationRequired)
            );
        }
    }
    for retry in [RetryClass::Never, RetryClass::AfterStateRefresh] {
        assert_eq!(
            Error::new(
                Code::CommitUnknown,
                OperationContext::admission_unknown(FIRST),
                retry,
                None
            ),
            Err(InvalidErrorContext::ReconciliationRequired)
        );
    }
}

#[test]
fn same_operation_retry_requires_an_original_retained_or_uncertain_identity() {
    for context in [
        OperationContext::proposed(None),
        OperationContext::proposed(Some(FIRST)),
    ] {
        assert_eq!(
            Error::new(
                Code::StorageFailure,
                context,
                RetryClass::SafeSameOperation,
                None
            ),
            Err(InvalidErrorContext::RetainedRetryRequired)
        );
    }
    for context in [
        OperationContext::retained(FIRST),
        OperationContext::admission_unknown(SECOND),
    ] {
        let error = Error::new(
            Code::RecoveryRequired,
            context,
            RetryClass::SafeSameOperation,
            None,
        )
        .unwrap();
        assert_eq!(error.context(), context);
    }
    let conflict = Error::new(
        Code::IdempotencyConflict,
        OperationContext::retained(FIRST),
        RetryClass::SafeSameOperation,
        None,
    );
    assert_eq!(conflict, Err(InvalidErrorContext::ConflictingRetry));
    let refresh = Error::new(
        Code::StaleState,
        OperationContext::proposed(Some(SECOND)),
        RetryClass::AfterStateRefresh,
        None,
    )
    .unwrap();
    assert_eq!(refresh.context().admission(), Admission::NotAdmitted);
    assert_eq!(refresh.context().operation_id(), Some(SECOND));
}

#[test]
fn fixed_diagnostics_and_messages_are_canonical_bounded_and_input_free() {
    let mut codes = BTreeSet::new();
    for &code in Code::ALL {
        assert!(codes.insert(code.as_str()));
        assert!(!code.as_str().is_empty() && code.as_str().len() <= 80);
        assert!(
            code.as_str()
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        );
        assert!(!code.message().is_empty() && code.message().len() <= 512);
        let error = Error::new(
            code,
            OperationContext::retained(FIRST),
            RetryClass::ManualReconciliation,
            None,
        )
        .unwrap();
        assert_eq!(error.code(), code);
        assert!(error.source().is_none());
        assert!(format!("{error}").len() < 600);
        assert!(format!("{error:?}").len() < 1024);
    }
    for &field in Field::ALL {
        for &reason in Reason::ALL {
            let detail = Detail::new(field, reason);
            assert_eq!(detail.field().to_string(), field.as_str());
            assert_eq!(detail.reason().to_string(), reason.as_str());
            assert!(field.as_str().len() < 32 && reason.as_str().len() < 32);
            assert!(format!("{detail:?}").len() < 128);
        }
    }
    assert!(size_of::<Error>() <= 64);
}

#[test]
fn retry_classes_and_construction_failures_have_fixed_protocol_strings() {
    for (retry, expected) in [
        (RetryClass::Never, "never"),
        (RetryClass::SafeSameOperation, "safe_same_operation"),
        (RetryClass::AfterStateRefresh, "after_state_refresh"),
        (RetryClass::ManualReconciliation, "manual_reconciliation"),
    ] {
        assert_eq!(retry.as_str(), expected);
    }
    for error in [
        InvalidErrorContext::ConfirmedAdmissionRequired,
        InvalidErrorContext::OperationIdentityRequired,
        InvalidErrorContext::RetainedRetryRequired,
        InvalidErrorContext::ReconciliationRequired,
        InvalidErrorContext::ConflictingRetry,
    ] {
        assert!(!error.to_string().is_empty() && error.to_string().len() < 80);
        assert!(error.source().is_none());
    }
}
