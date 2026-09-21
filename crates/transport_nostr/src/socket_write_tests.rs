use super::*;
use futures::{FutureExt, channel::mpsc, task::noop_waker_ref};

fn message(text: &str) -> Message {
    Message::Text(text.to_owned())
}

fn channel_writer() -> (Arc<SocketWriter>, mpsc::Receiver<Message>) {
    let (tx, rx) = mpsc::channel(0);
    (
        SocketWriter::new(Box::new(tx.sink_map_err(TransportError::backend))),
        rx,
    )
}

#[test]
fn concurrent_sdk_and_exact_messages_share_backpressure_and_order() {
    use futures::StreamExt;
    futures::executor::block_on(async {
        let (writer, mut rx) = channel_writer();
        let mut sdk = SharedSocketSink::new(Arc::clone(&writer));
        let mut first = Box::pin(sdk.send(message("AUTH")));
        assert!(first.as_mut().now_or_never().is_none());
        let mut raw = Box::pin(writer.send(message("EVENT")));
        assert!(raw.as_mut().now_or_never().is_none());
        assert_eq!(rx.next().await, Some(message("AUTH")));
        first.await.unwrap();
        assert!(raw.as_mut().now_or_never().is_none());
        assert_eq!(rx.next().await, Some(message("EVENT")));
        raw.await.unwrap();
        sdk.close().await.unwrap();
        sdk.close().await.unwrap();
        assert!(sdk.send(message("REQ")).await.is_err());
        assert!(writer.send(message("EVENT")).await.is_err());
        assert!(rx.next().await.is_none());
    });
}

#[test]
fn cancelling_a_waiter_releases_its_lock_without_claiming_no_effect() {
    use futures::StreamExt;
    futures::executor::block_on(async {
        let (writer, mut rx) = channel_writer();
        let mut sdk = SharedSocketSink::new(Arc::clone(&writer));
        let mut pending = Box::pin(writer.send(message("first")));
        assert!(pending.as_mut().now_or_never().is_none());
        drop(pending);
        // It was already buffered: cancelling a send is not proof of no publication.
        assert_eq!(rx.next().await, Some(message("first")));
        let mut next = Box::pin(sdk.send(message("second")));
        assert!(next.as_mut().now_or_never().is_none());
        assert_eq!(rx.next().await, Some(message("second")));
        next.await.unwrap();
    });
}

#[test]
fn pending_sdk_message_must_flush_before_next_send_or_close() {
    use futures::StreamExt;
    futures::executor::block_on(async {
        let (writer, mut rx) = channel_writer();
        let mut sdk = SharedSocketSink::new(writer);
        let mut context = Context::from_waker(noop_waker_ref());
        assert!(Pin::new(&mut sdk).poll_ready(&mut context).is_ready());
        Pin::new(&mut sdk).start_send(message("first")).unwrap();
        assert!(Pin::new(&mut sdk).start_send(message("unready")).is_err());
        assert!(Pin::new(&mut sdk).poll_close(&mut context).is_pending());
        assert_eq!(rx.next().await, Some(message("first")));
        sdk.close().await.unwrap();
        assert!(Pin::new(&mut sdk).start_send(message("closed")).is_err());
    });
}

#[test]
fn sdk_drop_revokes_existing_raw_handles_and_failed_io_revokes_writer() {
    futures::executor::block_on(async {
        let (writer, rx) = channel_writer();
        drop(SharedSocketSink::new(Arc::clone(&writer)));
        assert!(writer.send(message("after drop")).await.is_err());
        drop(rx);
        let (writer, rx) = channel_writer();
        drop(rx);
        let mut sdk = SharedSocketSink::new(Arc::clone(&writer));
        assert!(sdk.send(message("failure")).await.is_err());
        assert!(!writer.open.load(Ordering::Acquire));
        assert!(writer.send(message("after error")).await.is_err());
    });
}

#[test]
fn registry_is_configured_bounded_weak_and_reconnect_isolated() {
    let registry = WriterRegistry::new(["relay".to_owned()].into_iter());
    assert_eq!(format!("{registry:?}"), "WriterRegistry([redacted])");
    assert!(registry.get("relay").is_err());
    assert!(registry.get("unknown").is_err());
    let (first, _rx) = channel_writer();
    assert!(registry.install("unknown", &first).is_err());
    registry.install("relay", &first).unwrap();
    let retained = registry.get("relay").unwrap();
    assert!(Arc::ptr_eq(&retained, &first));
    let first_sdk = SharedSocketSink::new(Arc::clone(&first));
    let (second, _rx) = channel_writer();
    registry.install("relay", &second).unwrap();
    assert!(!retained.open.load(Ordering::Acquire));
    drop(first_sdk);
    assert!(!retained.open.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(&registry.get("relay").unwrap(), &second));
    second.invalidate();
    assert!(registry.get("relay").is_err());
    drop(second);
    assert!(registry.get("relay").is_err());
    assert_eq!(registry.0.lock().unwrap().len(), 1);
}

#[test]
fn poisoned_registry_fails_closed() {
    let registry = WriterRegistry::new(["relay".to_owned()].into_iter());
    let other = registry.clone();
    assert!(
        std::panic::catch_unwind(move || {
            let _guard = other.0.lock().unwrap();
            panic!("fixture registry poison");
        })
        .is_err()
    );
    let (writer, _rx) = channel_writer();
    assert!(registry.install("relay", &writer).is_err());
    assert!(registry.get("relay").is_err());
}
