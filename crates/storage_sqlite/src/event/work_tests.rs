use super::tests::{observed, signed_event_with, store, verified, visible};
use super::work::{Counts, Read};
use super::*;
use std::task::{Context, Poll, Waker};

async fn retained_source(size: u64) -> (SqliteStorage, u64) {
    let generation = SourceGeneration::new([71; 32]).unwrap();
    let storage = store(generation).await;
    let mut bytes = 0;
    for index in 0..size {
        let content = if index % 4 == 0 {
            format!(r#"{{"display_name":"Farm {index}","bot":false}}"#)
        } else {
            format!("retained note {index}")
        };
        let event = signed_event_with(
            &content,
            false,
            1_800_000_100 + index,
            if index % 4 == 0 { 0 } else { 1 },
            vec![],
        );
        bytes += event.raw_json().len() as u64;
        let provenance = observed(event.clone(), 10 + index, None);
        let admission = match index % 4 {
            2 => EventAdmission::raw(provenance),
            3 => EventAdmission::verified(provenance, verified(&event)).unwrap(),
            _ => EventAdmission::visible(provenance, visible(&event)).unwrap(),
        };
        storage.admit(admission).await.unwrap();
    }
    storage.work.take();
    (storage, bytes)
}

fn first() -> EventQuery {
    EventQuery::all(EventQueryBounds::first(5).unwrap())
}

fn assert_materialized(counts: Counts, rows: u64, bytes: u64) {
    assert_eq!(counts.materialization_requests, 1);
    assert_eq!(counts.event_rows_returned, rows);
    assert_eq!(counts.event_bytes_returned, bytes);
    assert_eq!(counts.codec_inputs, rows);
    assert_eq!(counts.codec_input_bytes, bytes);
    assert_eq!(counts.codec_successes, rows);
    assert_eq!(counts.visibility_inputs, rows);
}

async fn plan(storage: &SqliteStorage, kind: Read) -> Vec<String> {
    let template = storage.work.template(kind);
    let mut builder = QueryBuilder::<Sqlite>::new("EXPLAIN QUERY PLAN ");
    builder.push(template);
    let mut query = builder
        .build()
        .bind(storage.generation.as_bytes().as_slice());
    if matches!(kind, Read::Bounded | Read::Keys) {
        query = query.bind(0_i64).bind(6_i64);
    }
    query
        .fetch_all(&storage.pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get::<String, _>("detail"))
        .collect()
}

#[cfg(test)]
#[tokio::test]
async fn actual_returned_and_decoded_work_distinguishes_bounded_and_full_reads() {
    for retained in [32, 96] {
        let (storage, bytes) = retained_source(retained).await;
        let raw = storage.query_raw(first()).await.unwrap();
        assert_eq!(raw.items().len(), 5);
        let bounded = storage.work.take();
        assert_eq!(bounded.bounded_requests, 1);
        assert_eq!(bounded.successful_fetches, 1);
        assert_eq!(bounded.event_rows_returned, 6);
        assert_eq!(bounded.codec_inputs, 6);
        assert_eq!(bounded.codec_successes, 6);
        assert_eq!(bounded.event_bytes_returned, bounded.codec_input_bytes);
        assert!(bounded.event_bytes_returned > 0 && bounded.event_bytes_returned < bytes);
        assert_eq!(bounded.visibility_inputs, 0);
        let bounded_plan = plan(&storage, Read::Bounded).await;
        let page = storage.query_visible(first()).await.unwrap();
        assert_eq!(page.items().len(), 5);
        let full = storage.work.take();
        assert_eq!(full.materialization_requests, 0);
        assert_eq!(full.visibility_inputs, 0);
        assert_eq!(full.key_requests, 1);
        assert_eq!(full.key_rows_returned, 6);
        assert_eq!(full.codec_inputs, 5);
        assert_eq!(full.event_rows_returned, 5);
        assert_eq!(full.aggregate_requests, 1);
        let full_plan = plan(&storage, Read::Keys).await;
        assert!(!bounded_plan.is_empty() && !full_plan.is_empty());
        assert!(
            full_plan
                .iter()
                .any(|line| line.contains("visibility_members_page_idx"))
        );
        println!(
            "{}",
            serde_json::json!({"scenario":"bounded_vs_materialized", "retained_events":retained,
            "returned_page_items":5, "bounded":bounded, "full_visibility":full,
            "bounded_query_plan":bounded_plan, "full_query_plan":full_plan,
            "sqlite_vm_row_visits_measured":false, "wall_time_qualification":false})
        );
        storage.close().await.unwrap();
    }
}

#[cfg(test)]
#[tokio::test]
async fn status_and_rebuild_measure_actual_full_source_materialization() {
    let (storage, bytes) = retained_source(32).await;
    let status = storage.status().await.unwrap();
    assert_eq!(status.raw_events(), 32);
    assert_eq!(status.verified_events(), 24);
    let counts = storage.work.take();
    assert_eq!(counts.materialization_requests, 0);
    assert_eq!(counts.visibility_inputs, 0);
    assert_eq!(counts.codec_inputs, 0);
    assert_eq!(counts.event_rows_returned, 0);
    assert_eq!(counts.aggregate_requests, 1);
    assert_eq!(counts.aggregate_rows_returned, 1);
    assert_eq!(counts.successful_fetches, 1);
    let aggregate_plan = plan(&storage, Read::Aggregate).await;
    let visibility = storage.rebuild_visibility().await.unwrap();
    let rebuilt = storage.work.take();
    assert_materialized(rebuilt, 32, bytes);
    assert_eq!(rebuilt.aggregate_requests, 0);
    assert_eq!(rebuilt.successful_fetches, 1);
    assert_eq!(
        status.visible_events(),
        visibility.visible_event_ids().len() as u64
    );
    println!(
        "{}",
        serde_json::json!({"scenario":"status_and_rebuild", "status":counts,
        "rebuild":rebuilt, "aggregate_query_plan":aggregate_plan, "sqlite_vm_row_visits_measured":false})
    );
    storage.close().await.unwrap();
}

#[cfg(test)]
#[tokio::test]
async fn cancelled_pool_wait_materializes_nothing_and_retry_executes_real_read() {
    let (storage, _) = retained_source(32).await;
    let held = storage.pool.acquire().await.unwrap();
    let mut future = storage.query_raw(first());
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(future);
    let cancelled = storage.work.take();
    assert_eq!(cancelled.bounded_requests, 1);
    assert_eq!(cancelled.successful_fetches, 0);
    assert_eq!(cancelled.event_rows_returned, 0);
    assert_eq!(cancelled.codec_inputs, 0);
    assert_eq!(cancelled.codec_input_bytes, 0);
    drop(held);
    assert_eq!(storage.query_raw(first()).await.unwrap().items().len(), 5);
    let retry = storage.work.take();
    assert_eq!(retry.bounded_requests, 1);
    assert_eq!(retry.successful_fetches, 1);
    assert_eq!(retry.event_rows_returned, 6);
    assert_eq!(retry.codec_successes, 6);
    println!(
        "{}",
        serde_json::json!({"scenario":"cancelled_pool_wait", "cancelled":cancelled, "retry":retry,
        "cancellation_boundary":"pending pool acquisition", "running_sqlite_worker_cancellation_qualified":false})
    );
    storage.close().await.unwrap();
}

#[cfg(test)]
#[tokio::test]
async fn observations_are_instance_scoped_and_shared_only_by_clones() {
    let (storage, _) = retained_source(32).await;
    let (other, _) = retained_source(32).await;
    let cloned = storage.clone();
    cloned.query_raw(first()).await.unwrap();
    assert_eq!(storage.work.take().event_rows_returned, 6);
    assert_eq!(cloned.work.take(), Counts::default());
    assert_eq!(other.work.take(), Counts::default());
    storage.close().await.unwrap();
    other.close().await.unwrap();
}
