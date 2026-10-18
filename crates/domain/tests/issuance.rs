use radroots_domain::{ContextError, FIRST_ISSUANCE_MAX_SECONDS, IssuanceWindow, UnixSeconds};

fn time(value: u64) -> UnixSeconds {
    UnixSeconds::new(value).unwrap()
}

#[test]
fn preparation_captures_exact_fixed_window_and_shorter_authorization() {
    assert_eq!(FIRST_ISSUANCE_MAX_SECONDS, 120);
    for (prepared, authorization, deadline) in [
        (100, 900, 220),
        (100, 150, 150),
        (100, 101, 101),
        (253_402_300_798, 253_402_300_799, 253_402_300_799),
    ] {
        let window = IssuanceWindow::capture(time(prepared), time(authorization)).unwrap();
        assert_eq!(window.prepared_at(), time(prepared));
        assert_eq!(window.first_issue_not_after(), time(deadline));
        assert_eq!(
            IssuanceWindow::new(time(prepared), time(deadline)).unwrap(),
            window
        );
    }
}

#[test]
fn invalid_or_extended_retained_windows_are_refused_without_reading_a_clock() {
    for (prepared, deadline) in [(0, 1), (100, 0), (100, 100), (100, 99), (100, 221)] {
        assert_eq!(
            IssuanceWindow::new(time(prepared), time(deadline)),
            Err(ContextError::OutOfRange)
        );
    }
    for (prepared, deadline) in [(0, 100), (100, 0), (100, 100), (100, 99)] {
        assert_eq!(
            IssuanceWindow::capture(time(prepared), time(deadline)),
            Err(ContextError::OutOfRange)
        );
    }
}
