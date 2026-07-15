#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE, RadrootsTransport,
    RadrootsTransportCapabilityAvailability, RadrootsTransportCapabilityMaturity,
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest,
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportError, RadrootsTransportFetchReceipt,
    RadrootsTransportFetchRequest, RadrootsTransportFuture, RadrootsTransportImplementationState,
    RadrootsTransportKind, RadrootsTransportMeshScopeId, RadrootsTransportOutcome,
    RadrootsTransportOutcomeKind, RadrootsTransportStatus, RadrootsTransportTarget,
    RadrootsTransportTargetReceipt,
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
        Ok(Self {
            profile_id,
            endpoint,
            scope,
            agent_endpoint,
            behavior,
        })
    }

    pub fn deferred_until_implemented() -> Self {
        Self {
            profile_id: DEFAULT_PROFILE_ID.to_owned(),
            endpoint: RadrootsReticulumEndpoint::default(),
            scope: RadrootsTransportMeshScopeId::local_reticulum(),
            agent_endpoint: None,
            behavior: RadrootsReticulumBehavior::RejectDeliveryAttempts,
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

    pub fn status(&self) -> RadrootsReticulumStatus {
        RadrootsReticulumStatus {
            behavior: self.behavior,
            scope: self.scope.clone(),
            agent_endpoint: self.agent_endpoint.clone(),
            transport_status: RadrootsTransportStatus::new(
                RadrootsTransportKind::Reticulum,
                true,
                RadrootsTransportImplementationState::Real,
                false,
                RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
            )
            .with_maturity(RadrootsTransportCapabilityMaturity::Preview)
            .with_availability(RadrootsTransportCapabilityAvailability::Unavailable)
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
        ensure_reticulum_targets(request.target_set.targets())?;
        let outcome = reticulum_outcome(self.profile.behavior);
        let target_receipts = request
            .target_set
            .targets()
            .iter()
            .cloned()
            .map(|target| RadrootsTransportTargetReceipt::new(target, outcome.clone()))
            .collect::<Vec<_>>();
        Ok(RadrootsTransportDeliveryReceipt {
            request_id: request.request_id,
            target_receipts,
        })
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
                .map(|target| RadrootsTransportTargetReceipt::new(target, outcome.clone()))
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
}

impl fmt::Display for RadrootsReticulumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidEndpoint => "invalid Reticulum endpoint",
            Self::InvalidAgentEndpoint => "invalid Reticulum agent endpoint",
            Self::InvalidProfileId => "invalid Reticulum profile id",
            Self::InvalidFetchLimit => "Reticulum fetch limit must be greater than zero",
            Self::NonReticulumTarget => "Reticulum transport received a non-Reticulum target",
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
        | RadrootsReticulumError::InvalidFetchLimit => RadrootsTransportError::InvalidTransportKind,
    }
}

fn ensure_reticulum_targets(
    targets: &[RadrootsTransportTarget],
) -> Result<(), RadrootsReticulumError> {
    for target in targets {
        if target.kind != RadrootsTransportKind::Reticulum {
            return Err(RadrootsReticulumError::NonReticulumTarget);
        }
        if target.uri.as_str() != RADROOTS_RETICULUM_ENDPOINT_URI {
            return Err(RadrootsReticulumError::InvalidEndpoint);
        }
        if target.scope.is_none() {
            return Err(RadrootsReticulumError::InvalidEndpoint);
        }
    }
    Ok(())
}

fn reticulum_outcome(behavior: RadrootsReticulumBehavior) -> RadrootsTransportOutcome {
    let mut outcome = match behavior {
        RadrootsReticulumBehavior::RejectDeliveryAttempts => {
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::TransportUnavailable)
                .with_target_status(RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented)
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
