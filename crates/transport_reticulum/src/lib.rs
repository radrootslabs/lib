#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE, RadrootsTransport,
    RadrootsTransportCapabilities, RadrootsTransportCapabilityAvailability,
    RadrootsTransportCapabilityMaturity, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportError, RadrootsTransportFetchReceipt,
    RadrootsTransportFetchRequest, RadrootsTransportFuture, RadrootsTransportImplementationState,
    RadrootsTransportKind, RadrootsTransportMeshScopeId, RadrootsTransportOutcome,
    RadrootsTransportOutcomeKind, RadrootsTransportStatus, RadrootsTransportTarget,
    RadrootsTransportTargetReceipt, ReticulumCapabilityReportV1, ReticulumDestinationV1,
    ReticulumPayloadPolicyV1,
};

const DEFAULT_PROFILE_ID: &str = "transport.reticulum.default";
const RETICULUM_AGENT_ENDPOINT_PREFIX: &str = "reticulum-agent:";
const UNAVAILABLE_CODE: &str = "transport_unavailable";
const DEFERRED_CODE: &str = "deferred_until_implemented";
const DEFERRED_MESSAGE: &str = "Reticulum delivery is deferred until implementation";

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
    scope: RadrootsTransportMeshScopeId,
    agent_endpoint: Option<RadrootsReticulumAgentEndpoint>,
    behavior: RadrootsReticulumBehavior,
    destination: ReticulumDestinationV1,
    capability_report: ReticulumCapabilityReportV1,
}

impl RadrootsReticulumProfile {
    pub fn new(
        profile_id: impl Into<String>,
        endpoint: RadrootsReticulumEndpoint,
        scope: RadrootsTransportMeshScopeId,
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
            scope: RadrootsTransportMeshScopeId::local_reticulum(),
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

    pub fn scope(&self) -> &RadrootsTransportMeshScopeId {
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

    pub fn status(&self) -> RadrootsReticulumStatus {
        RadrootsReticulumStatus {
            behavior: self.behavior,
            scope: self.scope.clone(),
            agent_endpoint: self.agent_endpoint.clone(),
            destination: self.destination.clone(),
            capability_report: self.capability_report.clone(),
            transport_status: RadrootsTransportStatus::new(
                RadrootsTransportKind::Reticulum,
                true,
                RadrootsTransportImplementationState::Real,
                false,
                RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
            )
            .with_maturity(RadrootsTransportCapabilityMaturity::Preview)
            .with_availability(RadrootsTransportCapabilityAvailability::Unavailable)
            .with_capabilities(RadrootsTransportCapabilities::reticulum_unavailable())
            .with_profile_id(self.profile_id.clone())
            .with_endpoint_uri(self.endpoint.as_str()),
        }
    }
}

impl Default for RadrootsReticulumProfile {
    fn default() -> Self {
        Self::deferred_until_implemented()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumStatus {
    pub behavior: RadrootsReticulumBehavior,
    pub scope: RadrootsTransportMeshScopeId,
    pub agent_endpoint: Option<RadrootsReticulumAgentEndpoint>,
    pub destination: ReticulumDestinationV1,
    pub capability_report: ReticulumCapabilityReportV1,
    pub transport_status: RadrootsTransportStatus,
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

    pub fn status(&self) -> RadrootsReticulumStatus {
        self.profile.status()
    }

    pub fn deliver(
        &self,
        request: RadrootsTransportDeliveryRequest,
    ) -> Result<RadrootsTransportDeliveryReceipt, RadrootsReticulumError> {
        ensure_reticulum_targets(request.target_set().targets())?;
        let outcome = reticulum_outcome(self.profile.behavior);
        let target_receipts = request
            .target_set()
            .targets()
            .iter()
            .cloned()
            .map(|target| RadrootsTransportTargetReceipt::skipped(target, outcome.clone()))
            .collect::<Vec<_>>();
        RadrootsTransportDeliveryReceipt::for_request(&request, target_receipts)
            .map_err(|_| RadrootsReticulumError::InvalidDeliveryReceipt)
    }

    pub fn fetch(
        &self,
        request: RadrootsReticulumFetchRequest,
    ) -> Result<RadrootsReticulumFetchReceipt, RadrootsReticulumError> {
        if request.max_events == 0 {
            return Err(RadrootsReticulumError::InvalidFetchLimit);
        }
        Ok(RadrootsReticulumFetchReceipt {
            request_id: request.request_id,
            endpoint_uri: self.profile.endpoint.as_str().to_owned(),
            scope: self.profile.scope.clone(),
            agent_endpoint: self.profile.agent_endpoint.clone(),
            outcome: reticulum_outcome(self.profile.behavior),
            observed_event_count: 0,
            implementation: RadrootsTransportImplementationState::Real,
        })
    }
}

impl Default for RadrootsReticulumTransport {
    fn default() -> Self {
        Self::new(RadrootsReticulumProfile::default())
    }
}

impl RadrootsTransport for RadrootsReticulumTransport {
    fn transport_kind(&self) -> RadrootsTransportKind {
        RadrootsTransportKind::Reticulum
    }

    fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus> {
        Box::pin(async move { Ok(self.profile.status().transport_status) })
    }

    fn deliver<'a>(
        &'a self,
        request: RadrootsTransportDeliveryRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt> {
        Box::pin(async move {
            self.deliver(request)
                .map_err(reticulum_error_to_transport_error)
        })
    }

    fn fetch<'a>(
        &'a self,
        request: RadrootsTransportFetchRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
        Box::pin(async move {
            ensure_reticulum_targets(request.target_set.targets())
                .map_err(reticulum_error_to_transport_error)?;
            let outcome = reticulum_outcome(self.profile.behavior);
            let target_receipts = request
                .target_set
                .targets()
                .iter()
                .cloned()
                .map(|target| RadrootsTransportTargetReceipt::skipped(target, outcome.clone()))
                .collect::<Vec<_>>();
            Ok(RadrootsTransportFetchReceipt::new(
                request.request_id,
                target_receipts,
                0,
            ))
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumFetchRequest {
    pub request_id: String,
    pub max_events: u16,
}

impl RadrootsReticulumFetchRequest {
    pub fn new(
        request_id: impl Into<String>,
        max_events: u16,
    ) -> Result<Self, RadrootsReticulumError> {
        if max_events == 0 {
            return Err(RadrootsReticulumError::InvalidFetchLimit);
        }
        Ok(Self {
            request_id: request_id.into(),
            max_events,
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumFetchReceipt {
    pub request_id: String,
    pub endpoint_uri: String,
    pub scope: RadrootsTransportMeshScopeId,
    pub agent_endpoint: Option<RadrootsReticulumAgentEndpoint>,
    pub outcome: RadrootsTransportOutcome,
    pub observed_event_count: usize,
    pub implementation: RadrootsTransportImplementationState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsReticulumError {
    InvalidEndpoint,
    InvalidAgentEndpoint,
    InvalidProfileId,
    InvalidFetchLimit,
    NonReticulumTarget,
    InvalidDeliveryReceipt,
}

impl fmt::Display for RadrootsReticulumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidEndpoint => "invalid Reticulum endpoint",
            Self::InvalidAgentEndpoint => "invalid Reticulum agent endpoint",
            Self::InvalidProfileId => "invalid Reticulum profile id",
            Self::InvalidFetchLimit => "Reticulum fetch limit must be greater than zero",
            Self::NonReticulumTarget => "Reticulum transport received a non-Reticulum target",
            Self::InvalidDeliveryReceipt => "Reticulum transport produced an invalid receipt",
        })
    }
}

fn reticulum_error_to_transport_error(error: RadrootsReticulumError) -> RadrootsTransportError {
    match error {
        RadrootsReticulumError::InvalidEndpoint | RadrootsReticulumError::NonReticulumTarget => {
            RadrootsTransportError::InvalidTargetUri
        }
        RadrootsReticulumError::InvalidAgentEndpoint
        | RadrootsReticulumError::InvalidProfileId
        | RadrootsReticulumError::InvalidFetchLimit
        | RadrootsReticulumError::InvalidDeliveryReceipt => {
            RadrootsTransportError::InvalidTransportKind
        }
    }
}

fn ensure_reticulum_targets(
    targets: &[RadrootsTransportTarget],
) -> Result<(), RadrootsReticulumError> {
    for target in targets {
        if target.kind() != &RadrootsTransportKind::Reticulum {
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

fn reticulum_outcome(behavior: RadrootsReticulumBehavior) -> RadrootsTransportOutcome {
    let mut outcome = match behavior {
        RadrootsReticulumBehavior::RejectDeliveryAttempts => {
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::DeferredUntilImplemented)
        }
        RadrootsReticulumBehavior::DeferDeliveryPlans => {
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::DeferredUntilImplemented)
        }
    };
    outcome.code = Some(
        match behavior {
            RadrootsReticulumBehavior::RejectDeliveryAttempts => UNAVAILABLE_CODE,
            RadrootsReticulumBehavior::DeferDeliveryPlans => DEFERRED_CODE,
        }
        .to_owned(),
    );
    outcome.message = Some(
        match behavior {
            RadrootsReticulumBehavior::RejectDeliveryAttempts => {
                RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE
            }
            RadrootsReticulumBehavior::DeferDeliveryPlans => DEFERRED_MESSAGE,
        }
        .to_owned(),
    );
    outcome
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::string::ToString;
    use alloc::vec;
    use futures::executor::block_on;
    use radroots_transport::{
        RadrootsTransportPayload, RadrootsTransportSatisfactionPolicy, RadrootsTransportTargetSet,
    };

    fn reticulum_target() -> RadrootsTransportTarget {
        RadrootsTransportTarget::reticulum().expect("Reticulum target")
    }

    fn delivery_request(targets: Vec<RadrootsTransportTarget>) -> RadrootsTransportDeliveryRequest {
        RadrootsTransportDeliveryRequest::new(
            "delivery",
            RadrootsTransportPayload::mesh_frame_cbor("message", [1_u8, 2, 3])
                .expect("mesh payload"),
            RadrootsTransportTargetSet::new(targets).expect("target set"),
            RadrootsTransportSatisfactionPolicy::any_accepted(),
        )
        .expect("delivery request")
    }

    #[test]
    #[allow(clippy::unnecessary_to_owned)]
    fn endpoint_profile_and_fetch_models_cover_owned_and_borrowed_boundaries() {
        let endpoint =
            RadrootsReticulumEndpoint::parse(RADROOTS_RETICULUM_ENDPOINT_URI).expect("endpoint");
        assert_eq!(endpoint.as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
        assert_eq!(format!("{endpoint}"), RADROOTS_RETICULUM_ENDPOINT_URI);
        assert_eq!(
            endpoint.clone().into_string(),
            RADROOTS_RETICULUM_ENDPOINT_URI
        );
        assert_eq!(
            RadrootsReticulumEndpoint::default(),
            RadrootsReticulumEndpoint::parse(RADROOTS_RETICULUM_ENDPOINT_URI.to_string())
                .expect("owned endpoint")
        );
        assert!(RadrootsReticulumEndpoint::parse("reticulum:other").is_err());

        for invalid in [
            "",
            " reticulum-agent:local",
            "reticulum-agent:local ",
            "reticulum-agent:\tlocal",
            "other:local",
            RETICULUM_AGENT_ENDPOINT_PREFIX,
        ] {
            assert!(RadrootsReticulumAgentEndpoint::parse(invalid).is_err());
        }
        let agent =
            RadrootsReticulumAgentEndpoint::parse("reticulum-agent:local").expect("agent endpoint");
        assert_eq!(agent.as_str(), "reticulum-agent:local");
        assert_eq!(format!("{agent}"), "reticulum-agent:local");
        assert_eq!(agent.clone().into_string(), "reticulum-agent:local");
        assert_eq!(
            RadrootsReticulumAgentEndpoint::parse("reticulum-agent:owned".to_string())
                .expect("owned agent")
                .as_str(),
            "reticulum-agent:owned"
        );

        let scope = RadrootsTransportMeshScopeId::parse("farm.mesh").expect("scope");
        for invalid in ["", " ", "profile id"] {
            assert!(
                RadrootsReticulumProfile::new(
                    invalid,
                    endpoint.clone(),
                    scope.clone(),
                    None,
                    RadrootsReticulumBehavior::RejectDeliveryAttempts,
                )
                .is_err()
            );
        }
        let profile = RadrootsReticulumProfile::new(
            "transport.reticulum.farm".to_string(),
            endpoint.clone(),
            scope.clone(),
            Some(agent.clone()),
            RadrootsReticulumBehavior::RejectDeliveryAttempts,
        )
        .expect("profile")
        .with_behavior(RadrootsReticulumBehavior::DeferDeliveryPlans)
        .with_agent_endpoint(agent.clone());
        assert_eq!(profile.profile_id(), "transport.reticulum.farm");
        assert_eq!(profile.endpoint(), &endpoint);
        assert_eq!(profile.scope(), &scope);
        assert_eq!(profile.agent_endpoint(), Some(&agent));
        assert_eq!(
            profile.behavior(),
            RadrootsReticulumBehavior::DeferDeliveryPlans
        );
        assert_eq!(
            profile.destination(),
            &profile.capability_report().destination
        );
        assert_eq!(profile.status().behavior, profile.behavior());

        let default_profile = RadrootsReticulumProfile::deferred_until_implemented();
        assert_eq!(default_profile, RadrootsReticulumProfile::default());
        assert!(default_profile.agent_endpoint().is_none());

        assert!(RadrootsReticulumFetchRequest::new("invalid", 0).is_err());
        let fetch =
            RadrootsReticulumFetchRequest::new("fetch".to_string(), 1).expect("fetch request");
        assert_eq!(fetch.request_id, "fetch");
    }

    #[test]
    fn transport_facades_and_private_guards_cover_all_contract_outcomes() {
        let rejecting = RadrootsReticulumTransport::default();
        assert_eq!(
            rejecting.profile().behavior(),
            RadrootsReticulumBehavior::RejectDeliveryAttempts
        );
        assert_eq!(rejecting.status(), rejecting.profile().status());
        let receipt = rejecting
            .deliver(delivery_request(vec![reticulum_target()]))
            .expect("direct delivery");
        assert_eq!(receipt.target_receipts().len(), 1);
        assert_eq!(
            rejecting
                .fetch(RadrootsReticulumFetchRequest::new("direct-fetch", 1).expect("fetch"))
                .expect("direct fetch")
                .observed_event_count,
            0
        );
        assert!(
            rejecting
                .fetch(RadrootsReticulumFetchRequest {
                    request_id: "invalid-fetch".to_owned(),
                    max_events: 0,
                })
                .is_err()
        );

        assert_eq!(
            RadrootsTransport::transport_kind(&rejecting),
            RadrootsTransportKind::Reticulum
        );
        assert!(block_on(RadrootsTransport::status(&rejecting)).is_ok());
        assert!(
            block_on(RadrootsTransport::deliver(
                &rejecting,
                delivery_request(vec![reticulum_target()]),
            ))
            .is_ok()
        );
        let core_fetch = RadrootsTransportFetchRequest::new(
            "core-fetch",
            RadrootsTransportTargetSet::new(vec![reticulum_target()]).expect("target set"),
        );
        assert!(block_on(RadrootsTransport::fetch(&rejecting, core_fetch)).is_ok());

        let deferring = RadrootsReticulumTransport::new(
            RadrootsReticulumProfile::default()
                .with_behavior(RadrootsReticulumBehavior::DeferDeliveryPlans),
        );
        assert_eq!(
            deferring
                .deliver(delivery_request(vec![reticulum_target()]))
                .expect("deferred delivery")
                .target_receipts()[0]
                .outcome
                .kind,
            RadrootsTransportOutcomeKind::DeferredUntilImplemented
        );

        assert_eq!(
            reticulum_error_to_transport_error(RadrootsReticulumError::InvalidEndpoint),
            RadrootsTransportError::InvalidTargetUri
        );
        assert_eq!(
            reticulum_error_to_transport_error(RadrootsReticulumError::NonReticulumTarget),
            RadrootsTransportError::InvalidTargetUri
        );
        for error in [
            RadrootsReticulumError::InvalidAgentEndpoint,
            RadrootsReticulumError::InvalidProfileId,
            RadrootsReticulumError::InvalidFetchLimit,
        ] {
            assert_eq!(
                reticulum_error_to_transport_error(error),
                RadrootsTransportError::InvalidTransportKind
            );
        }

        assert!(ensure_reticulum_targets(&[]).is_ok());
        let wrong_kind =
            RadrootsTransportTarget::local("local:memory").expect("non-Reticulum target");
        assert_eq!(
            ensure_reticulum_targets(&[wrong_kind]),
            Err(RadrootsReticulumError::NonReticulumTarget)
        );
        assert_eq!(
            RadrootsTransportTarget::new(RadrootsTransportKind::Reticulum, "reticulum:other")
                .expect_err("wrong Reticulum URI"),
            RadrootsTransportError::InvalidTargetUri
        );
        assert!(reticulum_target().scope().is_some());

        for behavior in [
            RadrootsReticulumBehavior::RejectDeliveryAttempts,
            RadrootsReticulumBehavior::DeferDeliveryPlans,
        ] {
            let outcome = reticulum_outcome(behavior);
            assert!(outcome.code.is_some());
            assert!(outcome.message.is_some());
        }
    }
}
