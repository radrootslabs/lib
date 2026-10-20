use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

const UPGRADE: &[u8] = b"HTTP/1.1 101 Switching Protocols\r\n\r\n";
const MAX_MESSAGE_BYTES: usize = 512 * 1024;

fn upgraded() -> WireParser {
    let mut parser = WireParser::new(MAX_MESSAGE_BYTES);
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
    let maximum = MAX_MESSAGE_BYTES as u64;
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
    let mut parser = WireParser::new(MAX_MESSAGE_BYTES);
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
    let mut parser = WireParser::new(MAX_MESSAGE_BYTES);
    parser.consume(&exact, |_| true).expect("exact maximum");
    parser.eof().expect("upgrade boundary");
    let mut excessive = vec![b'x'];
    excessive.extend(exact);
    assert!(
        WireParser::new(MAX_MESSAGE_BYTES)
            .consume(&excessive, |_| true)
            .is_err()
    );
}

#[test]
fn eof_rejects_partial_upgrade_header_and_payload() {
    assert!(WireParser::new(MAX_MESSAGE_BYTES).eof().is_err());
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

struct Authority {
    open: AtomicBool,
    charge: AtomicBool,
}

impl Admission for Authority {
    fn permits_address(&self, _: std::net::IpAddr) -> bool {
        true
    }
    fn admitted_read(&self, _: &Context<'_>) -> bool {
        self.open.load(Ordering::SeqCst)
    }
    fn admitted_write(&self, _: &Context<'_>) -> bool {
        self.open.load(Ordering::SeqCst)
    }
    fn charge(&self, _: usize, _: usize, _: usize) -> bool {
        self.charge.load(Ordering::SeqCst)
    }
}

struct ControlledIo {
    bytes: Vec<u8>,
    offset: usize,
    reads: usize,
    writes: usize,
    shutdowns: usize,
    read_error: bool,
    revoke_after_read: bool,
    authority: Arc<Authority>,
}

impl AsyncRead for ControlledIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.reads += 1;
        if self.read_error {
            return Poll::Ready(Err(io::Error::other("controlled read failure")));
        }
        let length = (self.bytes.len() - self.offset).min(output.remaining());
        output.put_slice(&self.bytes[self.offset..self.offset + length]);
        self.offset += length;
        if self.revoke_after_read {
            self.authority.open.store(false, Ordering::SeqCst);
        }
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for ControlledIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.writes += 1;
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.writes += 1;
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.shutdowns += 1;
        Poll::Ready(Ok(()))
    }
}

fn metered(bytes: Vec<u8>) -> (MeteredIo<ControlledIo>, Arc<Authority>) {
    let authority = Arc::new(Authority {
        open: AtomicBool::new(true),
        charge: AtomicBool::new(true),
    });
    let inner = ControlledIo {
        bytes,
        offset: 0,
        reads: 0,
        writes: 0,
        shutdowns: 0,
        read_error: false,
        revoke_after_read: false,
        authority: authority.clone(),
    };
    (
        MeteredIo::new(inner, authority.clone(), MAX_MESSAGE_BYTES),
        authority,
    )
}

fn read_meter(
    meter: &mut MeteredIo<ControlledIo>,
    capacity: usize,
) -> (Poll<io::Result<()>>, usize) {
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut bytes = vec![0; capacity];
    let mut output = ReadBuf::new(&mut bytes);
    let result = Pin::new(meter).poll_read(&mut context, &mut output);
    (result, output.filled().len())
}

#[test]
fn empty_output_never_reads_and_large_outputs_still_use_fixed_scratch_chunks() {
    let mut bytes = UPGRADE.to_vec();
    bytes.extend([0x82, 126, 0x13, 0x88]);
    bytes.extend([1; 5000]);
    let length = bytes.len();
    let (mut meter, _) = metered(bytes);
    assert!(matches!(
        read_meter(&mut meter, 0),
        (Poll::Ready(Ok(())), 0)
    ));
    assert_eq!(meter.inner.reads, 0);
    assert!(matches!(
        read_meter(&mut meter, 8192),
        (Poll::Ready(Ok(())), READ_CHUNK_BYTES)
    ));
    assert!(
        matches!(read_meter(&mut meter,8192),(Poll::Ready(Ok(())),n) if n==length-READ_CHUNK_BYTES)
    );
    assert!(matches!(
        read_meter(&mut meter, 8192),
        (Poll::Ready(Ok(())), 0)
    ));
}

#[test]
fn read_errors_and_raw_admission_refusal_are_sticky_before_any_later_io() {
    for underlying_error in [false, true] {
        let (mut meter, authority) = metered(UPGRADE.to_vec());
        meter.inner.read_error = underlying_error;
        authority.charge.store(underlying_error, Ordering::SeqCst);
        assert!(matches!(
            read_meter(&mut meter, 4096),
            (Poll::Ready(Err(_)), 0)
        ));
        assert!(matches!(
            read_meter(&mut meter, 4096),
            (Poll::Ready(Err(_)), 0)
        ));
        assert_eq!(meter.inner.reads, 1);
        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        assert!(matches!(
            Pin::new(&mut meter).poll_write(&mut context, b"denied"),
            Poll::Ready(Err(_))
        ));
        assert!(matches!(
            Pin::new(&mut meter).poll_flush(&mut context),
            Poll::Ready(Err(_))
        ));
        assert_eq!(meter.inner.writes, 0);
        assert!(matches!(
            Pin::new(&mut meter).poll_shutdown(&mut context),
            Poll::Ready(Ok(()))
        ));
        assert_eq!(meter.inner.shutdowns, 1);
    }
}

#[test]
fn revocation_during_a_ready_read_never_exposes_the_obtained_plaintext() {
    let (mut meter, _) = metered(UPGRADE.to_vec());
    meter.inner.revoke_after_read = true;
    assert!(matches!(
        read_meter(&mut meter, 4096),
        (Poll::Ready(Err(_)), 0)
    ));
    assert!(matches!(
        read_meter(&mut meter, 4096),
        (Poll::Ready(Err(_)), 0)
    ));
    assert_eq!(meter.inner.reads, 1);
}

#[test]
fn revoked_write_and_flush_refuse_without_requiring_a_prior_failed_read() {
    let (mut meter, authority) = metered(UPGRADE.to_vec());
    authority.open.store(false, Ordering::SeqCst);
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    assert!(matches!(
        Pin::new(&mut meter).poll_write(&mut context, b"denied"),
        Poll::Ready(Err(_))
    ));
    assert!(matches!(
        Pin::new(&mut meter).poll_flush(&mut context),
        Poll::Ready(Err(_))
    ));
    assert_eq!(meter.inner.writes, 0);
    assert!(matches!(
        read_meter(&mut meter, 4096),
        (Poll::Ready(Err(_)), 0)
    ));
    assert_eq!(meter.inner.reads, 0);
    assert!(matches!(
        Pin::new(&mut meter).poll_shutdown(&mut context),
        Poll::Ready(Ok(()))
    ));
}

#[test]
fn truncated_payload_eof_is_terminal_at_the_meter_boundary() {
    let mut bytes = UPGRADE.to_vec();
    bytes.extend([0x81, 3, b'x']);
    let (mut meter, _) = metered(bytes);
    assert!(matches!(read_meter(&mut meter,4096),(Poll::Ready(Ok(())),n) if n>0));
    assert!(
        matches!(read_meter(&mut meter,4096),(Poll::Ready(Err(error)),0) if error.kind()==io::ErrorKind::UnexpectedEof)
    );
    assert!(matches!(
        read_meter(&mut meter, 4096),
        (Poll::Ready(Err(_)), 0)
    ));
    assert_eq!(meter.inner.reads, 2);
}
