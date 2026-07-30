#[path = "conformance/suite.rs"]
mod suite;
#[path = "conformance/support.rs"]
mod support;

use radroots_transport::Error;
use suite::{
    assert_request_boundaries, assert_sink_cancellation, assert_sink_conformance,
    assert_sink_error, assert_source_cancellation, assert_source_conformance, assert_source_error,
};
use support::{CombinedAdapter, MockSink, MockSource};

#[test]
fn source_only_adapter_satisfies_the_reusable_contract() {
    let source = MockSource::successful();
    assert_source_conformance(&source);
}

#[test]
fn sink_only_adapter_satisfies_the_reusable_contract() {
    let sink = MockSink::successful();
    assert_sink_conformance(&sink);
}

#[test]
fn combined_adapter_satisfies_both_reusable_contracts() {
    let adapter = CombinedAdapter::successful();
    assert_source_conformance(&adapter);
    assert_sink_conformance(&adapter);
}

#[test]
fn request_identity_and_operation_bounds_fail_closed() {
    assert_request_boundaries();
}

#[test]
fn normalized_adapter_errors_are_not_retried_or_rewritten() {
    let source = MockSource::failing(Error::UnsupportedOperation);
    assert_source_error(&source, Error::UnsupportedOperation);

    let sink = MockSink::failing(Error::UnsupportedOperation);
    assert_sink_error(&sink, Error::UnsupportedOperation);
}

#[test]
fn source_and_sink_cancellation_observe_publication_boundaries() {
    let source = MockSource::pending();
    assert_source_cancellation(&source);

    let sink = MockSink::pending();
    assert_sink_cancellation(&sink);
}
