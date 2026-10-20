use core::num::NonZeroU64;
use radroots_domain::{PublicKey, UnixSeconds};
use radroots_execution::session::{
    HostFuture,
    inquiry::{InquiryIo, capture_dm_route, selected_dm_targets},
};
use radroots_nostr::signing::{
    OsNonceSource,
    gift_wrap::{randomized_past_time, wrap_retained_seal},
};
use radroots_protocol::chat::CapturedDmRoute;
use radroots_signing::provider::NonceSource;
use radroots_storage::command::{EnvelopeCandidate, OperationError, PublicationWork};
use radroots_transport::{
    EventSink, EventSource, TargetSet,
    policy::{SatisfactionClass, SatisfactionPolicy},
    source::{FetchBounds, FetchRequest, FetchSelector},
};
use radroots_transport_nostr::{Config, NostrTransport};
use std::{
    sync::atomic::{AtomicU8, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct NativeInquiry {
    transport: NostrTransport,
    read: TargetSet,
    write: TargetSet,
    policy: SatisfactionPolicy,
    mode: AtomicU8,
}
impl NativeInquiry {
    pub(crate) fn new(config: Config, policy: SatisfactionPolicy) -> Result<Self, OperationError> {
        let read = TargetSet::new(
            config
                .read_relays()
                .map(|relay| relay.to_target())
                .collect::<Result<_, _>>()
                .map_err(|_| OperationError::InvalidCommand)?,
        )
        .map_err(|_| OperationError::InvalidCommand)?;
        let write = TargetSet::new(
            config
                .write_relays()
                .map(|relay| relay.to_target())
                .collect::<Result<_, _>>()
                .map_err(|_| OperationError::InvalidCommand)?,
        )
        .map_err(|_| OperationError::InvalidCommand)?;
        if policy.class() != SatisfactionClass::Accepted {
            return Err(OperationError::InvalidCommand);
        }
        policy
            .validate_for(&write)
            .map_err(|_| OperationError::InvalidCommand)?;
        Ok(Self {
            transport: radroots_transport_nostr_native::create_transport(config),
            read,
            write,
            policy,
            mode: AtomicU8::new(0),
        })
    }
    pub(crate) fn select(&self, online: bool) -> Result<(), OperationError> {
        let mode = if online { 2 } else { 1 };
        match self
            .mode
            .compare_exchange(0, mode, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(()),
            Err(current) if current == mode => Ok(()),
            Err(_) => Err(OperationError::Unauthorized),
        }
    }
    pub(crate) async fn disconnect(&self) {
        self.transport.shutdown().await;
    }
    fn capture_route<'a>(
        &self,
        recipient: PublicKey,
        raw_sources: impl IntoIterator<Item = &'a str>,
    ) -> Result<CapturedDmRoute, OperationError> {
        capture_dm_route(recipient, raw_sources, &self.write)
    }
}
impl InquiryIo for NativeInquiry {
    fn enabled(&self) -> bool {
        self.mode.load(Ordering::Acquire) == 2
    }
    fn now(&self) -> Result<UnixSeconds, OperationError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| OperationError::RecoveryRequired)?
            .as_secs();
        UnixSeconds::new(now).map_err(|_| OperationError::RecoveryRequired)
    }
    fn seal_time(&self, now: UnixSeconds) -> Result<UnixSeconds, OperationError> {
        let time = randomized_past_time(
            NonZeroU64::new(now.get()).ok_or(OperationError::RecoveryRequired)?,
            &OsNonceSource,
        )
        .map_err(|_| OperationError::RecoveryRequired)?;
        UnixSeconds::new(time.get()).map_err(|_| OperationError::RecoveryRequired)
    }
    fn route(
        &self,
        recipient: PublicKey,
        deadline: NonZeroU64,
    ) -> HostFuture<'_, Result<CapturedDmRoute, OperationError>> {
        Box::pin(async move {
            if !self.enabled() {
                return Err(OperationError::Unauthorized);
            }
            let selector = FetchSelector::all()
                .with_kinds(vec![10050])
                .and_then(|selector| selector.with_authors(vec![recipient]))
                .map_err(|_| OperationError::InvalidCommand)?;
            let bounds =
                FetchBounds::new(2, deadline.get()).map_err(|_| OperationError::InvalidCommand)?;
            let request = FetchRequest::new(self.delivery_id()?, self.read.clone(), bounds)
                .map_err(|_| OperationError::InvalidCommand)?
                .with_selector(selector);
            let page = self
                .transport
                .fetch(request)
                .await
                .map_err(|_| OperationError::MissingHistory)?;
            self.capture_route(
                recipient,
                page.events()
                    .iter()
                    .map(|observed| observed.event().raw_json()),
            )
        })
    }
    fn targets(&self, route: &CapturedDmRoute) -> Result<TargetSet, OperationError> {
        selected_dm_targets(route, &self.write)
    }
    fn publication_policy(&self) -> SatisfactionPolicy {
        self.policy.clone()
    }
    fn envelope(
        &self,
        work: PublicationWork,
        raw_seal: &str,
    ) -> Result<EnvelopeCandidate, OperationError> {
        let now = NonZeroU64::new(self.now()?.get()).ok_or(OperationError::RecoveryRequired)?;
        let recipient = work.recipient();
        let wrapped = wrap_retained_seal(raw_seal, recipient, now, &OsNonceSource)
            .map_err(|_| OperationError::InvalidEvidence)?;
        wrapped.with_wrapping_key(|key| {
            EnvelopeCandidate::from_obtained(work, wrapped.signed_json(), key)
        })
    }
    fn delivery_id(&self) -> Result<String, OperationError> {
        let bytes = OsNonceSource
            .next_nonce()
            .map_err(|_| OperationError::RecoveryRequired)?;
        let mut output = String::with_capacity(64);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            output.push(HEX[usize::from(byte >> 4)] as char);
            output.push(HEX[usize::from(byte & 15)] as char);
        }
        Ok(output)
    }
    fn publish(
        &self,
        request: radroots_transport::sink::DeliveryRequest,
        selected: TargetSet,
    ) -> HostFuture<'_, Result<radroots_transport::DeliveryReceipt, radroots_transport::SinkFailure>>
    {
        self.transport.deliver_selected(request, selected)
    }
}

#[cfg(test)]
mod tests {
    use super::NativeInquiry;
    use futures::executor::block_on;
    use nostr::JsonUtil;
    use radroots_domain::PublicKey;
    use radroots_execution::session::inquiry::InquiryIo;
    use radroots_storage::command::{OperationError, SignedDmRelaySource};
    use radroots_transport::policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy};
    use radroots_transport_nostr::{
        Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
    };
    use std::num::NonZeroU64;

    const URI: &str = "ws://127.0.0.1:1";
    fn config(access: RelayAccess) -> Config {
        Config::from_profile(
            RelayProfile::explicit(
                RelayProfileKind::Simulator,
                [RelayEndpoint::new(URI, RelayUrlPolicy::Local, access).unwrap()],
            )
            .unwrap(),
        )
    }
    fn io() -> NativeInquiry {
        NativeInquiry::new(
            config(RelayAccess::ReadWrite),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        )
        .unwrap()
    }
    fn source(author: u8, time: u64, content: &str, relays: &[&str]) -> String {
        let mut key = [0; 32];
        key[31] = author;
        let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&key).unwrap());
        nostr::EventBuilder::new(nostr::Kind::Custom(10050), content)
            .tags(
                relays
                    .iter()
                    .map(|uri| nostr::Tag::parse(["relay", *uri]).unwrap()),
            )
            .custom_created_at(nostr::Timestamp::from_secs(time))
            .sign_with_keys(&keys)
            .unwrap()
            .as_json()
    }
    fn key() -> PublicKey {
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap()
    }

    #[tokio::test]
    async fn explicit_io_configuration_and_repeated_start_modes_never_imply_network_admission() {
        assert!(
            NativeInquiry::new(
                config(RelayAccess::ReadOnly),
                SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all())
            )
            .is_err()
        );
        assert!(
            NativeInquiry::new(
                config(RelayAccess::ReadWrite),
                SatisfactionPolicy::new(SatisfactionClass::Delivered, TargetPolicy::all())
            )
            .is_err()
        );
        let local = io();
        assert!(matches!(
            local.route(key(), NonZeroU64::new(1000).unwrap()).await,
            Err(OperationError::Unauthorized)
        ));
        local.select(false).unwrap();
        local.select(false).unwrap();
        assert!(local.select(true).is_err());
        let online = io();
        online.select(true).unwrap();
        online.select(true).unwrap();
        assert!(online.select(false).is_err());
        local.disconnect().await;
        online.disconnect().await;
        let preview = crate::Client::preview();
        assert!(preview.start(crate::StartMode::Online).await.is_err());
        preview.close().await.unwrap();
    }

    #[test]
    fn exact_signed_route_heads_are_order_independent_and_ties_use_the_original_event_id() {
        let io = io();
        let older = source(1, 100, "old", &[URI]);
        let newer = source(1, 101, "new", &[URI]);
        let expected = SignedDmRelaySource::verify(&newer).unwrap().event_id();
        for values in [
            [older.as_str(), newer.as_str()],
            [newer.as_str(), older.as_str()],
        ] {
            let route = io.capture_route(key(), values).unwrap();
            assert_eq!(route.source_id(), expected);
            assert_eq!(route.raw_json(), newer);
        }
        let a = source(1, 101, "a", &[URI]);
        let b = source(1, 101, "b", &[URI]);
        let a_id = SignedDmRelaySource::verify(&a).unwrap().event_id();
        let b_id = SignedDmRelaySource::verify(&b).unwrap().event_id();
        for values in [[a.as_str(), b.as_str()], [b.as_str(), a.as_str()]] {
            assert_eq!(
                io.capture_route(key(), values).unwrap().source_id(),
                a_id.min(b_id)
            );
        }
        assert!(matches!(
            io.capture_route(key(), []),
            Err(OperationError::MissingHistory)
        ));
        assert!(matches!(
            io.capture_route(key(), [older.as_str(), newer.as_str(), newer.as_str()]),
            Err(OperationError::InvalidEvidence)
        ));
        assert!(matches!(
            io.capture_route(key(), [source(2, 100, "foreign", &[URI]).as_str()]),
            Err(OperationError::InvalidEvidence)
        ));
    }

    #[test]
    fn signed_route_selection_keeps_exact_sources_and_deduplicates_only_approved_targets() {
        let io = io();
        let raw = format!(
            " \n{}\t",
            source(
                1,
                100,
                "original",
                &[URI, URI, "ws://127.0.0.1:1/", "wss://other.example"]
            )
        );
        let route = io.capture_route(key(), [raw.as_str()]).unwrap();
        assert_eq!(route.raw_json(), raw);
        assert_eq!(route.selected_relays().len(), 2);
        assert_eq!(io.targets(&route).unwrap().len(), 1);
        let changed = radroots_transport::TargetSet::new(vec![
            radroots_transport::Target::nostr_relay("wss://changed.example").unwrap(),
        ])
        .unwrap();
        assert!(matches!(
            radroots_execution::session::inquiry::selected_dm_targets(&route, &changed),
            Err(OperationError::UnverifiedApproval)
        ));
        let refused = source(1, 100, "no approved target", &["wss://other.example"]);
        assert!(matches!(
            io.capture_route(key(), [refused.as_str()]),
            Err(OperationError::UnverifiedApproval)
        ));
        let no_targets = source(1, 100, "no advertised fallback", &[]);
        assert!(matches!(
            io.capture_route(key(), [no_targets.as_str()]),
            Err(OperationError::UnverifiedApproval)
        ));
        // The actual IO gate refuses before opening the configured local port.
        assert!(block_on(io.route(key(), NonZeroU64::new(1000).unwrap())).is_err());
    }
}
