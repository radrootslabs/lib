use super::*;
use crate::source::budget::{FetchBudget, MAX_FETCH_BYTES};
use crate::source_ingress::{FetchRegistration, IngressRegistry};
use futures::task::{ArcWake, waker};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const UPGRADE: &[u8] = b"HTTP/1.1 101 Switching Protocols\r\n\r\n";
const RELAY: &str = "ws://127.0.0.1:1234";

fn upgraded() -> WireParser {
    let mut parser = WireParser::default();
    parser
        .consume(UPGRADE, |_| panic!("HTTP is not a frame"))
        .expect("upgrade");
    parser
}

fn header(length: u64) -> Vec<u8> {
    let mut bytes = vec![0x82, 127];
    bytes.extend(length.to_be_bytes());
    bytes
}

#[test]
fn maximum_frame_length_is_admitted_and_next_byte_or_overflow_is_denied() {
    let maximum = crate::relay::MAX_WIRE_MESSAGE_BYTES as u64;
    let mut parser = upgraded();
    parser
        .consume(&header(maximum), |_| true)
        .expect("exact limit");
    assert_eq!(parser.payload_remaining, maximum as usize);
    for length in [maximum + 1, 1 << 63, u64::MAX] {
        assert!(upgraded().consume(&header(length), |_| true).is_err());
    }
}

#[test]
fn headers_payloads_and_upgrade_can_arrive_one_byte_at_a_time() {
    let mut parser = WireParser::default();
    let mut bytes = UPGRADE.to_vec();
    bytes.extend([0x01, 126, 0, 126]);
    bytes.extend([b'x'; 126]);
    bytes.extend([0x89, 0, 0x80, 2, b'y', b'z', 0x82, 0]);
    let mut frames = 0;
    let mut data = 0;
    for byte in bytes {
        parser
            .consume(&[byte], |is_data| {
                frames += 1;
                data += usize::from(is_data);
                true
            })
            .expect("chopped ingress");
    }
    assert_eq!((frames, data), (4, 2));
    parser.eof().expect("complete frames");
}

#[test]
fn zero_length_continuations_and_controls_each_require_admission() {
    let mut parser = upgraded();
    let mut admissions = 0;
    let error = parser
        .consume(&[0x01, 0, 0x00, 0, 0x89, 0, 0x80, 0], |is_data| {
            admissions += 1;
            assert_eq!(is_data, admissions == 1);
            admissions <= 3
        })
        .expect_err("fourth frame is denied");
    assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted);
    assert_eq!(admissions, 4);
}

#[test]
fn invalid_flags_masks_lengths_and_controls_fail_closed() {
    for bytes in [
        vec![0xc1, 0],
        vec![0x83, 0],
        vec![0x09, 0],
        vec![0x81, 0x80],
        vec![0x82, 126, 0, 125],
        header(65535),
        vec![0x89, 126, 0, 126],
    ] {
        assert!(
            upgraded().consume(&bytes, |_| true).is_err(),
            "accepted {bytes:?}"
        );
    }
}

#[test]
fn upgrade_prefix_has_an_exact_fixed_bound() {
    let mut exact = vec![b'x'; MAX_UPGRADE_BYTES - 4];
    exact.extend(b"\r\n\r\n");
    let mut parser = WireParser::default();
    parser.consume(&exact, |_| true).expect("exact maximum");
    parser.eof().expect("upgrade boundary");
    let mut excessive = vec![b'x'];
    excessive.extend(exact);
    assert!(WireParser::default().consume(&excessive, |_| true).is_err());
}

#[test]
fn eof_rejects_partial_upgrade_header_and_payload() {
    assert!(WireParser::default().eof().is_err());
    for bytes in [vec![0x81], vec![0x82, 126, 0], vec![0x81, 2, b'x']] {
        let mut parser = upgraded();
        parser.consume(&bytes, |_| true).expect("partial input");
        assert_eq!(
            parser.eof().expect_err("truncation").kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
    upgraded().eof().expect("frame boundary");
}

#[derive(Default)]
struct Calls {
    reads: AtomicUsize,
    writes: AtomicUsize,
    wakes: AtomicUsize,
}

impl ArcWake for Calls {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

struct TestIo {
    bytes: Vec<u8>,
    offset: usize,
    chunk: usize,
    pending: bool,
    calls: Arc<Calls>,
}

impl AsyncRead for TestIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.calls.reads.fetch_add(1, Ordering::SeqCst);
        if self.pending {
            return Poll::Pending;
        }
        let count = (self.bytes.len() - self.offset)
            .min(output.remaining())
            .min(self.chunk);
        output.put_slice(&self.bytes[self.offset..self.offset + count]);
        self.offset += count;
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for TestIo {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.calls.writes.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.calls.writes.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn fixture(
    bytes: Vec<u8>,
) -> (
    MeteredIo<TestIo>,
    Arc<FetchBudget>,
    FetchRegistration,
    Arc<Calls>,
) {
    let registry = IngressRegistry::new([RELAY.to_owned()].into_iter());
    let budget = Arc::new(FetchBudget::default());
    let registration = registry
        .register([RELAY.to_owned()].into_iter(), budget.clone())
        .expect("registration");
    let connection = registry.connection(RELAY).expect("connection");
    let calls = Arc::new(Calls::default());
    let inner = TestIo {
        bytes,
        offset: 0,
        chunk: READ_CHUNK_BYTES,
        pending: false,
        calls: calls.clone(),
    };
    (
        MeteredIo::new(inner, connection),
        budget,
        registration,
        calls,
    )
}

fn read(meter: &mut MeteredIo<TestIo>, calls: &Arc<Calls>) -> (Poll<io::Result<()>>, usize) {
    let waker = waker(calls.clone());
    let mut context = Context::from_waker(&waker);
    let mut output = [0; READ_CHUNK_BYTES];
    let mut buffer = ReadBuf::new(&mut output);
    let result = Pin::new(meter).poll_read(&mut context, &mut buffer);
    (result, buffer.filled().len())
}

#[test]
fn raw_upgrade_and_frame_overhead_count_before_decode() {
    let mut bytes = UPGRADE.to_vec();
    bytes.extend([0x81, 0]);
    let length = bytes.len();
    let (mut meter, budget, _registration, calls) = fixture(bytes);
    assert!(budget.wire(MAX_FETCH_BYTES - length, 0, 0));
    assert!(matches!(read(&mut meter, &calls), (Poll::Ready(Ok(())), count) if count == length));
    assert!(!budget.wire(1, 0, 0));
}

#[test]
fn excessive_read_is_not_exposed_and_subsequent_io_is_rejected() {
    let (mut meter, budget, _registration, calls) = fixture(UPGRADE.to_vec());
    assert!(budget.wire(MAX_FETCH_BYTES, 0, 0));
    assert!(matches!(read(&mut meter, &calls), (Poll::Ready(Err(_)), 0)));
    assert!(budget.exhausted());
    let reads = calls.reads.load(Ordering::SeqCst);
    assert!(matches!(read(&mut meter, &calls), (Poll::Ready(Err(_)), 0)));
    assert_eq!(calls.reads.load(Ordering::SeqCst), reads);
    let waker = waker(calls.clone());
    let mut context = Context::from_waker(&waker);
    assert!(matches!(
        Pin::new(&mut meter).poll_write(&mut context, b"REQ"),
        Poll::Ready(Err(_))
    ));
    assert!(matches!(
        Pin::new(&mut meter).poll_flush(&mut context),
        Poll::Ready(Err(_))
    ));
    assert_eq!(calls.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn cancellation_wakes_pending_read_and_revokes_both_io_directions() {
    let (mut meter, _budget, registration, calls) = fixture(Vec::new());
    meter.inner.pending = true;
    assert!(matches!(read(&mut meter, &calls), (Poll::Pending, 0)));
    let writer_calls = Arc::new(Calls::default());
    let writer_waker = waker(writer_calls.clone());
    let mut writer_context = Context::from_waker(&writer_waker);
    assert!(matches!(
        Pin::new(&mut meter).poll_write(&mut writer_context, b"REQ"),
        Poll::Ready(Ok(3))
    ));
    assert!(matches!(
        Pin::new(&mut meter).poll_flush(&mut writer_context),
        Poll::Ready(Ok(()))
    ));
    drop(registration);
    assert!(calls.wakes.load(Ordering::SeqCst) > 0);
    assert!(writer_calls.wakes.load(Ordering::SeqCst) > 0);
    assert!(matches!(read(&mut meter, &calls), (Poll::Ready(Err(_)), 0)));
    let waker = waker(calls.clone());
    let mut context = Context::from_waker(&waker);
    assert!(matches!(
        Pin::new(&mut meter).poll_write(&mut context, b"REQ"),
        Poll::Ready(Err(_))
    ));
    assert!(matches!(
        Pin::new(&mut meter).poll_shutdown(&mut context),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(calls.reads.load(Ordering::SeqCst), 1);
    assert_eq!(calls.writes.load(Ordering::SeqCst), 2);
}

#[test]
fn continuation_flood_exhausts_frame_budget_without_assembled_message() {
    let mut bytes = UPGRADE.to_vec();
    bytes.extend([0x01, 0]);
    for _ in 0..8192 {
        bytes.extend([0x00, 0]);
    }
    let (mut meter, budget, _registration, calls) = fixture(bytes);
    loop {
        match read(&mut meter, &calls) {
            (Poll::Ready(Ok(())), count) => assert!(count > 0, "unexpected EOF"),
            (Poll::Ready(Err(_)), 0) => break,
            other => panic!("unexpected result {other:?}"),
        }
    }
    assert!(budget.exhausted());
}

#[test]
fn empty_data_frames_exhaust_attempt_budget() {
    let mut bytes = UPGRADE.to_vec();
    for _ in 0..4097 {
        bytes.extend([0x81, 0]);
    }
    let (mut meter, budget, _registration, calls) = fixture(bytes);
    loop {
        match read(&mut meter, &calls) {
            (Poll::Ready(Ok(())), count) => assert!(count > 0, "unexpected EOF"),
            (Poll::Ready(Err(_)), 0) => break,
            other => panic!("unexpected result {other:?}"),
        }
    }
    assert!(budget.exhausted());
}

#[test]
fn read_chunks_remain_bounded_and_truncation_is_terminal() {
    let mut bytes = UPGRADE.to_vec();
    bytes.extend([0x81, 126, 0x20, 0]);
    bytes.extend([b'x'; 5000]);
    let (mut meter, _budget, _registration, calls) = fixture(bytes);
    assert!(matches!(
        read(&mut meter, &calls),
        (Poll::Ready(Ok(())), READ_CHUNK_BYTES)
    ));
    assert!(matches!(read(&mut meter, &calls), (Poll::Ready(Ok(())), count) if count > 0));
    assert!(
        matches!(read(&mut meter, &calls), (Poll::Ready(Err(error)), 0) if error.kind() == io::ErrorKind::UnexpectedEof)
    );
}
