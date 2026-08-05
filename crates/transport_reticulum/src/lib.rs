#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

mod contract;
mod message;

pub use contract::{
    RETICULUM_V1_MAX_PAYLOAD_BYTES, ReticulumCapabilityReportV1, ReticulumDestinationV1,
    ReticulumDuplicateFragmentBehaviorV1, ReticulumFragmentIntegrityV1, ReticulumFragmentPolicyV1,
    ReticulumFragmentationModeV1, ReticulumGatewaySemanticsV1, ReticulumPayloadPolicyV1,
    ReticulumPrivacySemanticsV1, ReticulumRoutingMetadataV1,
};
pub use message::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID,
    RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
};

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use radroots_transport::capability::{
    Availability, Maturity, SinkCapabilities, SourceCapabilities,
};
use radroots_transport::outcome::{DeliveryOutcome, FetchTargetOutcome, FetchTargetState};
use radroots_transport::sink::{
    DeliveryReceipt, DeliveryRequest, DeliveryTargetReceipt, EventSink, SinkStatus,
};
use radroots_transport::source::{EventSource, FetchPage, FetchRequest, NextPage, SourceStatus};
use radroots_transport::target::TargetScope;
use radroots_transport::{Error as TransportError, Target, TransportId};

const DEFAULT_PROFILE_ID: &str = "transport.reticulum.default";
const RETICULUM_AGENT_ENDPOINT_PREFIX: &str = "reticulum-agent:";
const UNAVAILABLE_CODE: &str = "transport_unavailable";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RadrootsReticulumBehavior {
    #[default]
    RejectDeliveryAttempts,
    DeferDeliveryPlans,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumEndpoint {
    uri: String,
}

impl RadrootsReticulumEndpoint {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsReticulumError> {
        let uri = raw.as_ref();
        if uri != RADROOTS_RETICULUM_ENDPOINT_URI {
            return Err(RadrootsReticulumError::InvalidEndpoint);
        }
        Ok(Self {
            uri: RADROOTS_RETICULUM_ENDPOINT_URI.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        self.uri.as_str()
    }

    pub fn into_string(self) -> String {
        self.uri
    }
}

impl Default for RadrootsReticulumEndpoint {
    fn default() -> Self {
        Self::parse(RADROOTS_RETICULUM_ENDPOINT_URI).expect("default Reticulum endpoint")
    }
}

impl fmt::Display for RadrootsReticulumEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.uri.as_str())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumAgentEndpoint {
    uri: String,
}

impl RadrootsReticulumAgentEndpoint {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsReticulumError> {
        let uri = raw.as_ref();
        if uri.is_empty()
            || uri != uri.trim()
            || uri
                .chars()
                .any(|ch| ch.is_ascii_control() || ch.is_ascii_whitespace())
            || !uri.starts_with(RETICULUM_AGENT_ENDPOINT_PREFIX)
            || uri.len() == RETICULUM_AGENT_ENDPOINT_PREFIX.len()
        {
            return Err(RadrootsReticulumError::InvalidAgentEndpoint);
        }
        Ok(Self {
            uri: uri.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        self.uri.as_str()
    }

    pub fn into_string(self) -> String {
        self.uri
    }
}

impl fmt::Display for RadrootsReticulumAgentEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.uri.as_str())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumProfile {
    profile_id: String,
    endpoint: RadrootsReticulumEndpoint,
    scope: TargetScope,
    agent_endpoint: Option<RadrootsReticulumAgentEndpoint>,
    behavior: RadrootsReticulumBehavior,
    destination: ReticulumDestinationV1,
    capability_report: ReticulumCapabilityReportV1,
}

impl RadrootsReticulumProfile {
    pub fn new(
        profile_id: impl Into<String>,
        endpoint: RadrootsReticulumEndpoint,
        scope: TargetScope,
        agent_endpoint: Option<RadrootsReticulumAgentEndpoint>,
        behavior: RadrootsReticulumBehavior,
    ) -> Result<Self, RadrootsReticulumError> {
        let profile_id = profile_id.into();
        if profile_id.trim().is_empty() || profile_id.chars().any(char::is_whitespace) {
            return Err(RadrootsReticulumError::InvalidProfileId);
        }
        let destination = ReticulumDestinationV1::new(endpoint.as_str(), scope.clone(), None)
            .map_err(|_| RadrootsReticulumError::InvalidEndpoint)?;
        let capability_report = ReticulumCapabilityReportV1 {
            destination: destination.clone(),
            payload_policy: ReticulumPayloadPolicyV1::v1(),
            ..ReticulumCapabilityReportV1::unavailable_local()
        };
        Ok(Self {
            profile_id,
            endpoint,
            scope,
            agent_endpoint,
            behavior,
            destination,
            capability_report,
        })
    }

    pub fn deferred_until_implemented() -> Self {
        let capability_report = ReticulumCapabilityReportV1::unavailable_local();
        Self {
            profile_id: DEFAULT_PROFILE_ID.to_owned(),
            endpoint: RadrootsReticulumEndpoint::default(),
            scope: TargetScope::parse(RADROOTS_RETICULUM_SCOPE_ID).expect("Reticulum scope"),
            agent_endpoint: None,
            behavior: RadrootsReticulumBehavior::RejectDeliveryAttempts,
            destination: capability_report.destination.clone(),
            capability_report,
        }
    }

    pub fn with_behavior(mut self, behavior: RadrootsReticulumBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn profile_id(&self) -> &str {
        self.profile_id.as_str()
    }

    pub fn endpoint(&self) -> &RadrootsReticulumEndpoint {
        &self.endpoint
    }

    pub fn scope(&self) -> &TargetScope {
        &self.scope
    }

    pub fn agent_endpoint(&self) -> Option<&RadrootsReticulumAgentEndpoint> {
        self.agent_endpoint.as_ref()
    }

    pub fn with_agent_endpoint(mut self, agent_endpoint: RadrootsReticulumAgentEndpoint) -> Self {
        self.agent_endpoint = Some(agent_endpoint);
        self
    }

    pub fn behavior(&self) -> RadrootsReticulumBehavior {
        self.behavior
    }

    pub fn destination(&self) -> &ReticulumDestinationV1 {
        &self.destination
    }

    pub fn capability_report(&self) -> &ReticulumCapabilityReportV1 {
        &self.capability_report
    }
}

impl Default for RadrootsReticulumProfile {
    fn default() -> Self {
        Self::deferred_until_implemented()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumTransport {
    profile: RadrootsReticulumProfile,
}

impl RadrootsReticulumTransport {
    pub fn new(profile: RadrootsReticulumProfile) -> Self {
        Self { profile }
    }

    pub fn profile(&self) -> &RadrootsReticulumProfile {
        &self.profile
    }
}

impl Default for RadrootsReticulumTransport {
    fn default() -> Self {
        Self::new(RadrootsReticulumProfile::default())
    }
}

impl EventSink for RadrootsReticulumTransport {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async {
            Ok(SinkStatus::new(
                TransportId::RETICULUM,
                true,
                Maturity::Preview,
                Availability::Unavailable,
                SinkCapabilities::NONE,
                RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
            ))
        })
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, radroots_transport::SinkFailure>>
    {
        Box::pin(async move {
            ensure_reticulum_targets(request.target_set().targets())
                .map_err(|_| radroots_transport::SinkFailure::invalid_contract(&request))?;
            let outcome = DeliveryOutcome::unavailable()
                .with_detail(UNAVAILABLE_CODE, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE)
                .map_err(|_| radroots_transport::SinkFailure::invalid_contract(&request))?;
            let receipts = request
                .target_set()
                .targets()
                .iter()
                .cloned()
                .map(|target| DeliveryTargetReceipt::skipped(target, outcome.clone()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| radroots_transport::SinkFailure::invalid_contract(&request))?;
            DeliveryReceipt::for_request(&request, receipts)
                .map_err(|_| radroots_transport::SinkFailure::invalid_contract(&request))
        })
    }
}

impl EventSource for RadrootsReticulumTransport {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
        Box::pin(async {
            Ok(SourceStatus::new(
                TransportId::RETICULUM,
                true,
                Maturity::Preview,
                Availability::Unavailable,
                SourceCapabilities::NONE,
                RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
            ))
        })
    }

    fn fetch(
        &self,
        request: FetchRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
        Box::pin(async move {
            ensure_reticulum_targets(request.target_set().targets())
                .map_err(reticulum_error_to_transport_error)?;
            let outcomes = request
                .target_set()
                .targets()
                .iter()
                .map(|target| {
                    FetchTargetOutcome::new(
                        target.fingerprint().clone(),
                        FetchTargetState::Unavailable,
                    )
                    .with_message(RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE)
                })
                .collect();
            FetchPage::for_request(&request, Vec::new(), outcomes, NextPage::Complete)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsReticulumError {
    InvalidEndpoint,
    InvalidAgentEndpoint,
    InvalidProfileId,
    NonReticulumTarget,
}

impl fmt::Display for RadrootsReticulumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidEndpoint => "invalid Reticulum endpoint",
            Self::InvalidAgentEndpoint => "invalid Reticulum agent endpoint",
            Self::InvalidProfileId => "invalid Reticulum profile id",
            Self::NonReticulumTarget => "Reticulum transport received a non-Reticulum target",
        })
    }
}

fn reticulum_error_to_transport_error(error: RadrootsReticulumError) -> TransportError {
    match error {
        RadrootsReticulumError::InvalidEndpoint | RadrootsReticulumError::NonReticulumTarget => {
            TransportError::InvalidTargetUri
        }
        RadrootsReticulumError::InvalidAgentEndpoint | RadrootsReticulumError::InvalidProfileId => {
            TransportError::InvalidTransportKind
        }
    }
}

fn ensure_reticulum_targets(targets: &[Target]) -> Result<(), RadrootsReticulumError> {
    for target in targets {
        if target.kind() != &TransportId::RETICULUM {
            return Err(RadrootsReticulumError::NonReticulumTarget);
        }
        if target.uri().as_str() != RADROOTS_RETICULUM_ENDPOINT_URI {
            return Err(RadrootsReticulumError::InvalidEndpoint);
        }
        if target.scope().is_none() {
            return Err(RadrootsReticulumError::InvalidEndpoint);
        }
    }
    Ok(())
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use alloc::{format, vec};
    use futures::executor::block_on;
    use radroots_event::{SignedEvent, wire::Nip01EventWire};
    use radroots_transport::{
        DeliveryRequest, EventSink, EventSource, FetchRequest, TargetSet,
        outcome::DeliveryOutcomeKind,
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        sink::DeliveryPayload,
        source::FetchBounds,
    };

    fn target() -> Target {
        ReticulumDestinationV1::local()
            .transport_target()
            .expect("Reticulum target")
    }

    fn signed_event() -> SignedEvent {
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
            created_at: 1_800_000_100,
            kind: 1,
            tags: vec![],
            content: "reticulum-preview".to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire.computed_event_id().expect("event id").to_hex();
        let raw = format!(
            "{{\"id\":\"{}\",\"pubkey\":\"{}\",\"created_at\":{},\"kind\":{},\"tags\":[],\"content\":\"{}\",\"sig\":\"{}\"}}",
            wire.id, wire.pubkey, wire.created_at, wire.kind, wire.content, wire.sig
        );
        SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
    }

    #[test]
    fn endpoints_and_profiles_preserve_validated_reticulum_contracts() {
        let endpoint = RadrootsReticulumEndpoint::default();
        assert_eq!(endpoint.as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
        assert_eq!(format!("{endpoint}"), RADROOTS_RETICULUM_ENDPOINT_URI);
        assert!(RadrootsReticulumEndpoint::parse("reticulum:other").is_err());

        let agent =
            RadrootsReticulumAgentEndpoint::parse("reticulum-agent:local").expect("agent endpoint");
        let profile = RadrootsReticulumProfile::new(
            "transport.reticulum.farm",
            endpoint,
            TargetScope::parse("farm.mesh").expect("scope"),
            Some(agent.clone()),
            RadrootsReticulumBehavior::DeferDeliveryPlans,
        )
        .expect("profile");
        assert_eq!(profile.agent_endpoint(), Some(&agent));
        assert_eq!(
            profile.behavior(),
            RadrootsReticulumBehavior::DeferDeliveryPlans
        );
        assert_eq!(
            profile.destination(),
            &profile.capability_report().destination
        );
    }

    #[test]
    fn canonical_source_and_sink_fail_closed_with_request_bound_evidence() {
        let transport = RadrootsReticulumTransport::default();
        let sink_status = block_on(EventSink::status(&transport)).expect("sink status");
        let source_status = block_on(EventSource::status(&transport)).expect("source status");
        assert_eq!(sink_status.transport_id(), TransportId::RETICULUM);
        assert_eq!(source_status.transport_id(), TransportId::RETICULUM);
        assert_eq!(sink_status.availability(), Availability::Unavailable);
        assert_eq!(source_status.availability(), Availability::Unavailable);

        let targets = TargetSet::new(vec![target()]).expect("targets");
        let delivery = DeliveryRequest::new(
            "reticulum-delivery",
            DeliveryPayload::new(signed_event()),
            targets.clone(),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            1_800_000_200_000,
        )
        .expect("delivery request");
        let receipt = block_on(EventSink::deliver(&transport, delivery.clone()))
            .expect("unavailable receipt");
        assert_eq!(receipt.target_receipts().len(), 1);
        assert!(!receipt.target_receipts()[0].was_attempted());
        assert_eq!(
            receipt.target_receipts()[0].outcome().kind(),
            DeliveryOutcomeKind::Unavailable
        );
        receipt
            .validate_for_request(&delivery)
            .expect("request bound");

        let fetch = FetchRequest::new(
            "reticulum-fetch",
            targets,
            FetchBounds::new(10, 1_800_000_200_000).expect("bounds"),
        )
        .expect("fetch request");
        let page = block_on(EventSource::fetch(&transport, fetch.clone())).expect("fetch page");
        assert!(page.events().is_empty());
        page.validate_for_request(&fetch).expect("request bound");
    }

    #[test]
    fn non_reticulum_targets_are_rejected_before_adapter_effects() {
        let target = Target::nostr_relay("wss://relay.example").expect("Nostr target");
        assert_eq!(
            ensure_reticulum_targets(&[target]),
            Err(RadrootsReticulumError::NonReticulumTarget)
        );
        assert_eq!(
            reticulum_error_to_transport_error(RadrootsReticulumError::NonReticulumTarget),
            TransportError::InvalidTargetUri
        );
    }
}
