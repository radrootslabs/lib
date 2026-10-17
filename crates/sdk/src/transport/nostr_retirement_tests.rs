use super::*;
use radroots_transport::{EventSource, FetchPage, FetchRequest, source::FetchBounds};
use std::{
    future::Future,
    task::{Context, Poll, Waker},
    time::Duration,
};
use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot, task::JoinHandle};

fn profile(base: &str, count: usize) -> RelayProfile {
    RelayProfile::explicit(
        RelayProfileKind::Simulator,
        (0..count).map(|index| {
            RelayEndpoint::new(
                format!("{base}/{index}"),
                RelayUrlPolicy::Local,
                RelayAccess::ReadWrite,
            )
            .unwrap()
        }),
    )
    .unwrap()
}

struct HeldFetch {
    slot: NostrSlot,
    previous: NostrState,
    base: String,
    fetch: JoinHandle<Result<FetchPage, radroots_transport::Error>>,
    server: JoinHandle<()>,
}

async fn held_fetch(relays: usize) -> HeldFetch {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("ws://{}", listener.local_addr().unwrap());
    let (connected, ready) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 4096];
        assert!(stream.read(&mut bytes).await.unwrap() > 0);
        connected.send(()).unwrap();
        // An admitted connection is held before upgrade. Only the exact /0
        // target is requested; the remaining profile reservations do no I/O.
        loop {
            match stream.read(&mut bytes).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
    let slot = NostrSlot::new();
    slot.configure(profile(&base, relays)).await.unwrap();
    let previous = slot.cleanup_snapshot().unwrap();
    let target = RelayEndpoint::new(
        format!("{base}/0"),
        RelayUrlPolicy::Local,
        RelayAccess::ReadWrite,
    )
    .unwrap()
    .url()
    .to_target()
    .unwrap();
    let request = FetchRequest::new(
        "retirement-resource-check",
        TargetSet::new(vec![target]).unwrap(),
        FetchBounds::new(1, u64::MAX).unwrap(),
    )
    .unwrap();
    let copy = slot.clone();
    let fetch = tokio::spawn(async move { copy.fetch(request).await });
    tokio::time::timeout(Duration::from_secs(2), ready)
        .await
        .unwrap()
        .unwrap();
    assert!(previous.transport.has_in_flight_operations());
    HeldFetch {
        slot,
        previous,
        base,
        fetch,
        server,
    }
}

#[tokio::test]
async fn an_oversized_new_profile_is_refused_before_changing_the_selection() {
    let slot = NostrSlot::new();
    let base = "ws://127.0.0.1:19991";
    slot.configure(profile(base, NOSTR_SESSION_RELAY_LIMIT))
        .await
        .unwrap();
    assert_eq!(
        slot.read_targets().unwrap().len(),
        NOSTR_SESSION_RELAY_LIMIT
    );
    assert_eq!(
        slot.configure(profile(base, NOSTR_SESSION_RELAY_LIMIT + 1))
            .await
            .unwrap_err()
            .kind(),
        crate::error::ErrorKind::InvalidHostConfiguration
    );
    assert_eq!(
        slot.read_targets().unwrap().len(),
        NOSTR_SESSION_RELAY_LIMIT
    );
    slot.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_aggregate_budget_retains_a_busy_pool_and_refuses_an_extra_reservation() {
    let HeldFetch {
        slot,
        previous,
        base,
        fetch,
        server,
    } = held_fetch(16).await;
    assert_eq!(
        slot.configure(profile(&base, 1)).await.unwrap_err().kind(),
        crate::error::ErrorKind::SharedOperationUnavailable
    );
    assert!(previous.transport.admission_is_closed());
    assert!(previous.transport.has_in_flight_operations());
    assert!(slot.read_targets().is_none());
    assert!(slot.state.read().unwrap().retired.is_empty());
    slot.shutdown().await;
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    let _obtained = tokio::time::timeout(Duration::from_secs(5), fetch)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_shutdown_retains_current_and_retired_pools_for_explicit_retry() {
    let HeldFetch {
        slot,
        previous,
        base,
        fetch,
        server,
    } = held_fetch(1).await;
    slot.configure(profile(&base, 15)).await.unwrap();
    assert_eq!(slot.read_targets().unwrap().len(), 15);
    assert_eq!(slot.state.read().unwrap().retired.len(), 1);
    assert!(previous.transport.has_in_flight_operations());
    assert_eq!(
        slot.configure(profile(&base, 16)).await.unwrap_err().kind(),
        crate::error::ErrorKind::SharedOperationUnavailable
    );
    let held = slot.transition.lock().await;
    let mut close = Box::pin(slot.shutdown());
    assert_eq!(
        close.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    );
    drop(close);
    assert_eq!(slot.state.read().unwrap().retired.len(), 1);
    assert_eq!(
        slot.configure(profile(&base, 1)).await.unwrap_err().kind(),
        crate::error::ErrorKind::ClientClosed
    );
    drop(held);
    slot.shutdown().await;
    assert!(slot.state.read().unwrap().retired.is_empty());
    assert!(slot.read_targets().is_none());
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    let _obtained = tokio::time::timeout(Duration::from_secs(5), fetch)
        .await
        .unwrap()
        .unwrap();
    slot.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancelled_call_disconnects_before_idle_retirement_is_reclaimed() {
    let HeldFetch {
        slot,
        previous,
        base,
        fetch,
        server,
    } = held_fetch(1).await;
    slot.configure(profile(&base, 15)).await.unwrap();
    assert_eq!(slot.state.read().unwrap().retired.len(), 1);
    fetch.abort();
    assert!(fetch.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    assert!(!previous.transport.has_in_flight_operations());
    slot.configure(profile(&base, 1)).await.unwrap();
    assert!(slot.state.read().unwrap().retired.is_empty());
    assert_eq!(slot.read_targets().unwrap().len(), 1);
    slot.shutdown().await;
}
