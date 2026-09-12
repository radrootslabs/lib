use super::{
    tests::{config, png, read_request, upload_request},
    *,
};
use std::sync::Arc;
use tokio::{io::AsyncWriteExt, net::TcpListener, sync::Notify};

fn descriptor(transaction: &BlossomUploadTransaction) -> Vec<u8> {
    serde_json::to_vec(
        &BlobDescriptor::new(
            transaction.expected_url().clone(),
            transaction.request().sha256(),
            transaction.request().byte_size(),
            MediaType::parse("image/png").unwrap(),
            1_900_000_000,
        )
        .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn retrieval_failures_preserve_native_upload_context_and_passive_evidence() {
    for unavailable in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            assert!(
                String::from_utf8(read_request(&mut stream).await)
                    .unwrap()
                    .starts_with("GET /")
            );
            let body = if unavailable { Vec::new() } else { png(3, 3) };
            let status = if unavailable {
                "503 Service Unavailable"
            } else {
                "200 OK"
            };
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
            stream.shutdown().await.unwrap();
        });
        let slot = crate::transport::BlossomSlot::new();
        slot.configure(config(&origin)).unwrap();
        let transaction = slot
            .prepare_upload(upload_request(&origin, png(2, 3)))
            .unwrap();
        let body = descriptor(&transaction);
        let error = slot
            .complete_native_upload(
                transaction,
                200,
                Some("application/json"),
                None,
                &body,
                BlossomCancellation::default(),
            )
            .await
            .unwrap_err();
        server.await.unwrap();
        assert!(
            error.possible_orphan(),
            "A retrieval failure cannot erase the native upload"
        );
        assert_eq!(error.attempts(), 2);
        assert_eq!(error.retryable(), unavailable);
        if unavailable {
            assert_eq!(error.http_status(), Some(503));
        }
        let evidence = slot.evidence().unwrap();
        assert!(evidence.possible_orphan());
        assert_eq!(evidence.attempts(), 2);
        assert_eq!(
            evidence.last_successful_state(),
            crate::transport::BlossomEvidenceState::UploadVerified
        );
        assert_eq!(evidence.error_code(), Some(error.code()));
    }
}

#[tokio::test]
async fn cancellation_before_retrieval_preserves_the_completed_native_attempt() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let slot = crate::transport::BlossomSlot::new();
    slot.configure(config(&origin)).unwrap();
    let transaction = slot
        .prepare_upload(upload_request(&origin, png(2, 3)))
        .unwrap();
    let body = descriptor(&transaction);
    let cancellation = BlossomCancellation::default();
    cancellation.cancel();
    let error = slot
        .complete_native_upload(
            transaction,
            201,
            Some("application/json"),
            None,
            &body,
            cancellation,
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), BlossomErrorKind::Cancelled);
    assert!(error.possible_orphan());
    assert_eq!(error.attempts(), 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cancellation_during_retrieval_preserves_both_attempts() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let entered = Arc::new(Notify::new());
    let server = {
        let entered = entered.clone();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = read_request(&mut stream).await;
            entered.notify_one();
            std::future::pending::<()>().await;
        })
    };
    let slot = crate::transport::BlossomSlot::new();
    slot.configure(config(&origin)).unwrap();
    let transaction = slot
        .prepare_upload(upload_request(&origin, png(2, 3)))
        .unwrap();
    let body = descriptor(&transaction);
    let cancellation = BlossomCancellation::default();
    let result = slot.complete_native_upload(
        transaction,
        200,
        Some("application/json"),
        None,
        &body,
        cancellation.clone(),
    );
    let cancel = async {
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        cancellation.cancel();
    };
    let (result, ()) = tokio::join!(result, cancel);
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    let error = result.unwrap_err();
    assert_eq!(error.kind(), BlossomErrorKind::Cancelled);
    assert!(error.possible_orphan());
    assert_eq!(error.attempts(), 2);
}
