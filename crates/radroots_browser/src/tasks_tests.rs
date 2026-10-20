use super::*;
use futures::{
    FutureExt,
    channel::oneshot,
    executor::{LocalPool, block_on},
    task::LocalSpawnExt,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn started(pool: &LocalPool) -> Shared<OwnedTasks> {
    let tasks = Shared::new(OwnedTasks::default());
    let spawn = pool.spawner();
    tasks
        .start(|task| spawn.spawn_local(task).unwrap())
        .unwrap();
    tasks
}
#[test]
fn startup_quiesce_and_empty_close_are_permanent_and_unpolled_close_is_inert() {
    let mut pool = LocalPool::new();
    let tasks = Shared::new(OwnedTasks::default());
    assert!(tasks.submit(Box::pin(async {})).is_err());
    drop(tasks.close());
    let spawn = pool.spawner();
    tasks
        .start(|task| spawn.spawn_local(task).unwrap())
        .unwrap();
    tasks.start(|_| panic!("second owner")).unwrap();
    tasks.quiesce().unwrap();
    assert!(tasks.submit(Box::pin(async {})).is_err());
    pool.run_until(tasks.close()).unwrap();
    block_on(tasks.close()).unwrap();
    assert!(tasks.start(|_| panic!("restart")).is_err());
    let unopened = OwnedTasks::default();
    block_on(unopened.close()).unwrap();
    assert!(unopened.start(|_| panic!("closed before start")).is_err());
}
#[test]
fn bounded_original_jobs_survive_cancelled_drain_and_close_observers() {
    let mut pool = LocalPool::new();
    let tasks = started(&pool);
    let (release, blocked) = oneshot::channel::<()>();
    tasks
        .submit(Box::pin(async move {
            blocked.await.unwrap();
        }))
        .unwrap();
    pool.run_until_stalled();
    let count = Arc::new(AtomicUsize::new(0));
    for _ in 1..CAPACITY {
        let count = count.clone();
        tasks
            .submit(Box::pin(async move {
                count.fetch_add(1, Ordering::SeqCst);
            }))
            .unwrap();
    }
    assert!(
        tasks
            .submit(Box::pin(async { panic!("refused task polled") }))
            .is_err()
    );
    let mut drain = Box::pin(tasks.drain());
    assert!(drain.as_mut().now_or_never().is_none());
    drop(drain);
    let mut close = Box::pin(tasks.close());
    assert!(close.as_mut().now_or_never().is_none());
    drop(close);
    assert!(
        tasks
            .submit(Box::pin(async { panic!("closed admission") }))
            .is_err()
    );
    release.send(()).unwrap();
    pool.run_until(tasks.close()).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), CAPACITY - 1);
    block_on(tasks.drain()).unwrap();
}
#[test]
fn an_idle_owner_wakes_and_drain_waits_for_queued_then_running_original_work() {
    let mut pool = LocalPool::new();
    let tasks = started(&pool);
    pool.run_until_stalled();
    let count = Arc::new(AtomicUsize::new(0));
    let completed = count.clone();
    let (release, blocked) = oneshot::channel::<()>();
    tasks
        .submit(Box::pin(async move {
            blocked.await.unwrap();
            completed.fetch_add(1, Ordering::SeqCst);
        }))
        .unwrap();
    let mut drain = Box::pin(tasks.drain());
    assert!(drain.as_mut().now_or_never().is_none());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    pool.run_until_stalled();
    assert!(drain.as_mut().now_or_never().is_none());
    release.send(()).unwrap();
    pool.run_until(drain).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let completed = count.clone();
    tasks
        .submit(Box::pin(async move {
            completed.fetch_add(1, Ordering::SeqCst);
        }))
        .unwrap();
    pool.run_until_stalled();
    pool.run_until(tasks.close()).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
}
#[test]
fn losing_last_host_observer_still_drains_accepted_work_and_drops_outside_lock() {
    struct ReentrantDrop(Shared<OwnedTasks>, Arc<AtomicUsize>);
    impl Drop for ReentrantDrop {
        fn drop(&mut self) {
            self.0.quiesce().unwrap();
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }
    let mut pool = LocalPool::new();
    let tasks = started(&pool);
    let count = Arc::new(AtomicUsize::new(0));
    let probe = ReentrantDrop(tasks.clone(), count.clone());
    tasks.quiesce().unwrap();
    assert!(
        tasks
            .submit(Box::pin(async move {
                let _probe = probe;
            }))
            .is_err()
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    pool.run_until(tasks.close()).unwrap();
    let tasks = started(&pool);
    let count2 = count.clone();
    tasks
        .submit(Box::pin(async move {
            count2.fetch_add(1, Ordering::SeqCst);
        }))
        .unwrap();
    drop(tasks);
    pool.run();
    assert_eq!(count.load(Ordering::SeqCst), 2);
}
#[test]
fn poisoned_bookkeeping_never_reports_successful_completion() {
    let tasks = OwnedTasks::default();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = tasks.inner.state.lock().unwrap();
        panic!("intentional poison");
    }));
    assert!(tasks.start(|_| panic!("poisoned start")).is_err());
    assert!(
        tasks
            .submit(Box::pin(async { panic!("poisoned admission") }))
            .is_err()
    );
    assert!(tasks.quiesce().is_err());
    assert!(block_on(tasks.drain()).is_err());
    assert!(block_on(tasks.close()).is_err());
}
