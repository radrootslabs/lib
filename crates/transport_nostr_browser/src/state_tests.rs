use super::*;
use std::{
    net::IpAddr,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::Context,
};

pub(crate) struct AdmissionCounter {
    allow: AtomicBool,
    bytes: AtomicUsize,
    frames: AtomicUsize,
    cancelling: Mutex<Option<std::sync::Weak<Shared>>>,
}
impl AdmissionCounter {
    pub(crate) fn new(allow: bool) -> Arc<Self> {
        Arc::new(Self {
            allow: AtomicBool::new(allow),
            bytes: AtomicUsize::new(0),
            frames: AtomicUsize::new(0),
            cancelling: Mutex::new(None),
        })
    }
}
impl Admission for AdmissionCounter {
    fn permits_address(&self, _: IpAddr) -> bool {
        self.allow.load(Ordering::SeqCst)
    }
    fn admitted_read(&self, _: &Context<'_>) -> bool {
        self.allow.load(Ordering::SeqCst)
    }
    fn admitted_write(&self, _: &Context<'_>) -> bool {
        self.allow.load(Ordering::SeqCst)
    }
    fn charge(&self, bytes: usize, frames: usize, data: usize) -> bool {
        assert_eq!(frames, data);
        self.bytes.fetch_add(bytes, Ordering::SeqCst);
        self.frames.fetch_add(frames, Ordering::SeqCst);
        let cancelling = self.cancelling.lock().unwrap().clone();
        if let Some(shared) = cancelling.and_then(|weak| weak.upgrade()) {
            // Fail promptly rather than deadlocking the regression if a host
            // calls the admission callback while holding its own state lock.
            if shared.state.try_lock().is_err() {
                return false;
            }
            shared.stop();
        }
        self.allow.load(Ordering::SeqCst)
    }
}
fn shared(limit: usize) -> (Shared, Arc<AdmissionCounter>) {
    let admission = AdmissionCounter::new(true);
    (Shared::new(admission.clone(), limit), admission)
}
fn open(limit: usize) -> (Shared, Arc<AdmissionCounter>) {
    let result = shared(limit);
    result.0.lock().unwrap().status = Status::Open;
    result
}

#[test]
fn original_binary_and_text_are_charged_once_and_dequeue_releases_bytes() {
    let (shared, admission) = open(64);
    assert_eq!(shared.pop(), Ok(None));
    let frames = [
        Frame::Text(" [\"raw\",  3] ".into()),
        Frame::Binary(vec![0, 255, 12]),
    ];
    for frame in &frames {
        shared.push(frame.clone()).unwrap();
    }
    assert_eq!(admission.bytes.load(Ordering::SeqCst), 16);
    assert_eq!(admission.frames.load(Ordering::SeqCst), 2);
    for frame in frames {
        assert_eq!(shared.pop(), Ok(Some(frame)));
    }
    assert_eq!(shared.lock().unwrap().bytes, 0);
}

#[test]
fn item_limit_counts_empty_messages_and_never_charges_rejected_queue_work() {
    let (shared, admission) = open(1);
    for _ in 0..MAX_QUEUE_MESSAGES {
        shared.push(Frame::Text(String::new())).unwrap();
    }
    assert_eq!(shared.push(Frame::Binary(vec![])), Err(Error::Policy));
    assert_eq!(admission.frames.load(Ordering::SeqCst), MAX_QUEUE_MESSAGES);
    assert_eq!(shared.pop(), Ok(Some(Frame::Text(String::new()))));
    shared.push(Frame::Binary(vec![])).unwrap();
}

#[test]
fn byte_limit_is_exact_and_newly_freed_credit_is_reusable() {
    let (shared, _) = open(MAX_MESSAGE);
    for _ in 0..8 {
        shared.push(Frame::Binary(vec![7; MAX_MESSAGE])).unwrap();
    }
    assert_eq!(shared.lock().unwrap().bytes, MAX_QUEUE_BYTES);
    assert_eq!(shared.push(Frame::Text("x".into())), Err(Error::Policy));
    shared.pop().unwrap();
    shared.push(Frame::Binary(vec![9; MAX_MESSAGE])).unwrap();
    assert_eq!(shared.lock().unwrap().bytes, MAX_QUEUE_BYTES);
}

#[test]
fn message_limit_and_control_frames_refuse_before_charging() {
    let (shared, admission) = open(2);
    assert_eq!(shared.push(Frame::Text("abc".into())), Err(Error::Policy));
    assert_eq!(shared.push(Frame::Ping(vec![])), Err(Error::Unsupported));
    assert_eq!(shared.push(Frame::Pong(vec![])), Err(Error::Unsupported));
    assert_eq!(shared.push(Frame::Close(None)), Err(Error::Unsupported));
    assert_eq!(admission.frames.load(Ordering::SeqCst), 0);
    shared.push(Frame::Text("é".into())).unwrap();
    assert_eq!(admission.bytes.load(Ordering::SeqCst), 2);
    assert_eq!(Shared::new(admission, usize::MAX).limit, MAX_MESSAGE);
}

#[test]
fn admission_refusal_never_exposes_the_original_payload() {
    let (shared, admission) = open(4);
    admission.allow.store(false, Ordering::SeqCst);
    assert_eq!(shared.push(Frame::Text("deny".into())), Err(Error::Policy));
    assert_eq!(shared.pop(), Ok(None));
    assert_eq!(admission.bytes.load(Ordering::SeqCst), 4);
}

#[test]
fn first_failure_survives_subsequent_failure_close_and_finish() {
    let (shared, _) = open(4);
    shared.push(Frame::Text("live".into())).unwrap();
    shared.fail(Error::Policy);
    shared.fail(Error::Unavailable);
    shared.stop();
    shared.finish();
    shared.finish();
    assert_eq!(shared.pop(), Err(Error::Policy));
    let state = shared.lock().unwrap();
    assert_eq!(state.status, Status::Failed(Error::Policy));
    assert!(state.finished && state.stopping);
    assert_eq!(state.bytes, 0);
}

#[test]
fn graceful_finish_preserves_original_queued_frames_and_fails_pending_send() {
    let (shared, _) = open(8);
    shared.push(Frame::Binary(vec![1, 2, 3])).unwrap();
    let (sender, receiver) = oneshot::channel();
    shared.lock().unwrap().outgoing = Some(SendRequest {
        frame: Frame::Text("send".into()),
        result: sender,
    });
    assert_eq!(
        shared.lock().unwrap().outgoing.as_ref().unwrap().frame,
        Frame::Text("send".into())
    );
    shared.finish();
    assert_eq!(
        futures::executor::block_on(receiver),
        Ok(Err(Error::Closed))
    );
    assert_eq!(shared.pop(), Ok(Some(Frame::Binary(vec![1, 2, 3]))));
    assert_eq!(shared.pop(), Ok(None));
    assert_eq!(shared.lock().unwrap().status, Status::Ended);
}

#[test]
fn connecting_stopping_ended_and_failed_refuse_new_payloads() {
    let (shared, _) = shared(4);
    assert_eq!(shared.push(Frame::Text("x".into())), Err(Error::Closed));
    for status in [Status::Ended, Status::Failed(Error::Policy)] {
        shared.lock().unwrap().status = status;
        assert_eq!(shared.push(Frame::Text("x".into())), Err(Error::Closed));
    }
    shared.lock().unwrap().status = Status::Open;
    shared.stop();
    assert_eq!(shared.push(Frame::Text("x".into())), Err(Error::Closed));
}

#[test]
fn poisoned_state_refuses_and_cleanup_does_not_panic() {
    let (shared, _) = shared(1);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = shared.state.lock().unwrap();
        panic!("deliberate test poison");
    }));
    assert!(matches!(shared.lock(), Err(Error::Closed)));
    assert_eq!(shared.push(Frame::Text("x".into())), Err(Error::Closed));
    assert_eq!(shared.pop(), Err(Error::Closed));
    shared.fail(Error::Policy);
    shared.stop();
    shared.finish();
}

#[test]
fn admission_callback_can_cancel_before_a_frame_is_exposed() {
    let admission = AdmissionCounter::new(true);
    let shared = Arc::new(Shared::new(admission.clone(), 8));
    *admission.cancelling.lock().unwrap() = Some(Arc::downgrade(&shared));
    shared.lock().unwrap().status = Status::Open;
    assert_eq!(
        shared.push(Frame::Text("original".into())),
        Err(Error::Closed)
    );
    assert_eq!(shared.pop(), Ok(None));
}

#[test]
fn send_completion_wakes_its_observer_after_releasing_the_host_lock() {
    use std::{future::Future, pin::Pin};
    struct Observer {
        shared: Arc<Shared>,
        locked: AtomicBool,
    }
    impl futures::task::ArcWake for Observer {
        fn wake_by_ref(this: &Arc<Self>) {
            if this.shared.state.try_lock().is_err() {
                this.locked.store(true, Ordering::SeqCst);
            }
        }
    }
    let (shared, _) = open(8);
    let shared = Arc::new(shared);
    let observer = Arc::new(Observer {
        shared: shared.clone(),
        locked: AtomicBool::new(false),
    });
    let waker = futures::task::waker(observer.clone());
    let mut context = Context::from_waker(&waker);
    let (sender, mut receiver) = oneshot::channel();
    assert!(Pin::new(&mut receiver).poll(&mut context).is_pending());
    shared.lock().unwrap().outgoing = Some(SendRequest {
        frame: Frame::Text("original".into()),
        result: sender,
    });
    shared.finish();
    assert!(!observer.locked.load(Ordering::SeqCst));
    assert_eq!(
        futures::executor::block_on(receiver),
        Ok(Err(Error::Closed))
    );
}
