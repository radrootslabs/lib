#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
    RadrootsTransport, RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest,
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportError, RadrootsTransportFetchReceipt,
    RadrootsTransportFetchRequest, RadrootsTransportFuture, RadrootsTransportImplementationState,
    RadrootsTransportKind, RadrootsTransportMeshScopeId, RadrootsTransportOutcome,
    RadrootsTransportOutcomeKind, RadrootsTransportStatus, RadrootsTransportTarget,
    RadrootsTransportTargetReceipt,
};

const DEFAULT_PROFILE_ID: &str = "transport.reticulum.preview";
const RETICULUM_AGENT_ENDPOINT_PREFIX: &str = "reticulum-agent:";
const UNAVAILABLE_CODE: &str = "transport_unavailable";
const DEFERRED_CODE: &str = "deferred_until_implemented";
const DEFERRED_MESSAGE: &str = "Reticulum preview delivery is deferred until implementation";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsReticulumPreviewBehavior {
    RejectDeliveryAttempts,
    DeferDeliveryPlans,
}

impl Default for RadrootsReticulumPreviewBehavior {
    fn default() -> Self {
        Self::RejectDeliveryAttempts
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumPreviewEndpoint {
    uri: String,
}

impl RadrootsReticulumPreviewEndpoint {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsReticulumPreviewError> {
        let uri = raw.as_ref();
        if uri != RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI {
            return Err(RadrootsReticulumPreviewError::InvalidEndpoint);
        }
        Ok(Self {
            uri: RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        self.uri.as_str()
    }

    pub fn into_string(self) -> String {
        self.uri
    }
}

impl Default for RadrootsReticulumPreviewEndpoint {
    fn default() -> Self {
        Self::parse(RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI)
            .expect("default Reticulum preview endpoint")
    }
}

impl fmt::Display for RadrootsReticulumPreviewEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.uri.as_str())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumPreviewAgentEndpoint {
    uri: String,
}

impl RadrootsReticulumPreviewAgentEndpoint {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsReticulumPreviewError> {
        let uri = raw.as_ref();
        if uri.is_empty()
            || uri != uri.trim()
            || uri
                .chars()
                .any(|ch| ch.is_ascii_control() || ch.is_ascii_whitespace())
            || !uri.starts_with(RETICULUM_AGENT_ENDPOINT_PREFIX)
            || uri.len() == RETICULUM_AGENT_ENDPOINT_PREFIX.len()
        {
            return Err(RadrootsReticulumPreviewError::InvalidAgentEndpoint);
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

impl fmt::Display for RadrootsReticulumPreviewAgentEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.uri.as_str())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumPreviewProfile {
    profile_id: String,
    endpoint: RadrootsReticulumPreviewEndpoint,
    scope: RadrootsTransportMeshScopeId,
    agent_endpoint: Option<RadrootsReticulumPreviewAgentEndpoint>,
    behavior: RadrootsReticulumPreviewBehavior,
}

impl RadrootsReticulumPreviewProfile {
    pub fn new(
        profile_id: impl Into<String>,
        endpoint: RadrootsReticulumPreviewEndpoint,
        scope: RadrootsTransportMeshScopeId,
        agent_endpoint: Option<RadrootsReticulumPreviewAgentEndpoint>,
        behavior: RadrootsReticulumPreviewBehavior,
    ) -> Result<Self, RadrootsReticulumPreviewError> {
        let profile_id = profile_id.into();
        if profile_id.trim().is_empty() || profile_id.chars().any(char::is_whitespace) {
            return Err(RadrootsReticulumPreviewError::InvalidProfileId);
        }
        Ok(Self {
            profile_id,
            endpoint,
            scope,
            agent_endpoint,
            behavior,
        })
    }

    pub fn preview_unavailable() -> Self {
        Self {
            profile_id: DEFAULT_PROFILE_ID.to_owned(),
            endpoint: RadrootsReticulumPreviewEndpoint::default(),
            scope: RadrootsTransportMeshScopeId::local_preview(),
            agent_endpoint: None,
            behavior: RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts,
        }
    }

    pub fn with_behavior(mut self, behavior: RadrootsReticulumPreviewBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn profile_id(&self) -> &str {
        self.profile_id.as_str()
    }

    pub fn endpoint(&self) -> &RadrootsReticulumPreviewEndpoint {
        &self.endpoint
    }

    pub fn scope(&self) -> &RadrootsTransportMeshScopeId {
        &self.scope
    }

    pub fn agent_endpoint(&self) -> Option<&RadrootsReticulumPreviewAgentEndpoint> {
        self.agent_endpoint.as_ref()
    }

    pub fn with_agent_endpoint(
        mut self,
        agent_endpoint: RadrootsReticulumPreviewAgentEndpoint,
    ) -> Self {
        self.agent_endpoint = Some(agent_endpoint);
        self
    }

    pub fn behavior(&self) -> RadrootsReticulumPreviewBehavior {
        self.behavior
    }

    pub fn status(&self) -> RadrootsReticulumPreviewStatus {
        RadrootsReticulumPreviewStatus {
            behavior: self.behavior,
            scope: self.scope.clone(),
            agent_endpoint: self.agent_endpoint.clone(),
            transport_status: RadrootsTransportStatus::new(
                RadrootsTransportKind::Reticulum,
                true,
                RadrootsTransportImplementationState::PreviewUnavailable,
                false,
                RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
            )
            .with_profile_id(self.profile_id.clone())
            .with_endpoint_uri(self.endpoint.as_str()),
        }
    }
}

impl Default for RadrootsReticulumPreviewProfile {
    fn default() -> Self {
        Self::preview_unavailable()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumPreviewStatus {
    pub behavior: RadrootsReticulumPreviewBehavior,
    pub scope: RadrootsTransportMeshScopeId,
    pub agent_endpoint: Option<RadrootsReticulumPreviewAgentEndpoint>,
    pub transport_status: RadrootsTransportStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumPreviewTransport {
    profile: RadrootsReticulumPreviewProfile,
}

impl RadrootsReticulumPreviewTransport {
    pub fn new(profile: RadrootsReticulumPreviewProfile) -> Self {
        Self { profile }
    }

    pub fn profile(&self) -> &RadrootsReticulumPreviewProfile {
        &self.profile
    }

    pub fn status(&self) -> RadrootsReticulumPreviewStatus {
        self.profile.status()
    }

    pub fn deliver(
        &self,
        request: RadrootsTransportDeliveryRequest,
    ) -> Result<RadrootsTransportDeliveryReceipt, RadrootsReticulumPreviewError> {
        ensure_reticulum_targets(request.target_set.targets())?;
        let outcome = preview_outcome(self.profile.behavior);
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
        request: RadrootsReticulumPreviewFetchRequest,
    ) -> Result<RadrootsReticulumPreviewFetchReceipt, RadrootsReticulumPreviewError> {
        if request.max_events == 0 {
            return Err(RadrootsReticulumPreviewError::InvalidFetchLimit);
        }
        Ok(RadrootsReticulumPreviewFetchReceipt {
            request_id: request.request_id,
            endpoint_uri: self.profile.endpoint.as_str().to_owned(),
            scope: self.profile.scope.clone(),
            agent_endpoint: self.profile.agent_endpoint.clone(),
            outcome: preview_outcome(self.profile.behavior),
            observed_event_count: 0,
            implementation: RadrootsTransportImplementationState::PreviewUnavailable,
        })
    }
}

impl Default for RadrootsReticulumPreviewTransport {
    fn default() -> Self {
        Self::new(RadrootsReticulumPreviewProfile::default())
    }
}

impl RadrootsTransport for RadrootsReticulumPreviewTransport {
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
                .map_err(reticulum_preview_error_to_transport_error)
        })
    }

    fn fetch<'a>(
        &'a self,
        request: RadrootsTransportFetchRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
        Box::pin(async move {
            ensure_reticulum_targets(request.target_set.targets())
                .map_err(reticulum_preview_error_to_transport_error)?;
            let outcome = preview_outcome(self.profile.behavior);
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
pub struct RadrootsReticulumPreviewFetchRequest {
    pub request_id: String,
    pub max_events: u16,
}

impl RadrootsReticulumPreviewFetchRequest {
    pub fn new(
        request_id: impl Into<String>,
        max_events: u16,
    ) -> Result<Self, RadrootsReticulumPreviewError> {
        if max_events == 0 {
            return Err(RadrootsReticulumPreviewError::InvalidFetchLimit);
        }
        Ok(Self {
            request_id: request_id.into(),
            max_events,
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReticulumPreviewFetchReceipt {
    pub request_id: String,
    pub endpoint_uri: String,
    pub scope: RadrootsTransportMeshScopeId,
    pub agent_endpoint: Option<RadrootsReticulumPreviewAgentEndpoint>,
    pub outcome: RadrootsTransportOutcome,
    pub observed_event_count: usize,
    pub implementation: RadrootsTransportImplementationState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsReticulumPreviewError {
    InvalidEndpoint,
    InvalidAgentEndpoint,
    InvalidProfileId,
    InvalidFetchLimit,
    NonReticulumTarget,
}

impl fmt::Display for RadrootsReticulumPreviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidEndpoint => "invalid Reticulum preview endpoint",
            Self::InvalidAgentEndpoint => "invalid Reticulum preview agent endpoint",
            Self::InvalidProfileId => "invalid Reticulum preview profile id",
            Self::InvalidFetchLimit => "Reticulum preview fetch limit must be greater than zero",
            Self::NonReticulumTarget => {
                "Reticulum preview transport received a non-Reticulum target"
            }
        })
    }
}

fn reticulum_preview_error_to_transport_error(
    error: RadrootsReticulumPreviewError,
) -> RadrootsTransportError {
    match error {
        RadrootsReticulumPreviewError::InvalidEndpoint
        | RadrootsReticulumPreviewError::NonReticulumTarget => {
            RadrootsTransportError::InvalidTargetUri
        }
        RadrootsReticulumPreviewError::InvalidAgentEndpoint
        | RadrootsReticulumPreviewError::InvalidProfileId
        | RadrootsReticulumPreviewError::InvalidFetchLimit => {
            RadrootsTransportError::InvalidTransportKind
        }
    }
}

fn ensure_reticulum_targets(
    targets: &[RadrootsTransportTarget],
) -> Result<(), RadrootsReticulumPreviewError> {
    for target in targets {
        if target.kind != RadrootsTransportKind::Reticulum {
            return Err(RadrootsReticulumPreviewError::NonReticulumTarget);
        }
        if target.uri.as_str() != RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI {
            return Err(RadrootsReticulumPreviewError::InvalidEndpoint);
        }
        if target.scope.is_none() {
            return Err(RadrootsReticulumPreviewError::InvalidEndpoint);
        }
    }
    Ok(())
}

fn preview_outcome(behavior: RadrootsReticulumPreviewBehavior) -> RadrootsTransportOutcome {
    let mut outcome = match behavior {
        RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts => {
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::TransportUnavailable)
                .with_target_status(RadrootsTransportDeliveryTargetStatus::PreviewUnavailable)
        }
        RadrootsReticulumPreviewBehavior::DeferDeliveryPlans => {
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::DeferredUntilImplemented)
        }
    };
    outcome.code = Some(
        match behavior {
            RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts => UNAVAILABLE_CODE,
            RadrootsReticulumPreviewBehavior::DeferDeliveryPlans => DEFERRED_CODE,
        }
        .to_owned(),
    );
    outcome.message = Some(
        match behavior {
            RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts => {
                RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE
            }
            RadrootsReticulumPreviewBehavior::DeferDeliveryPlans => DEFERRED_MESSAGE,
        }
        .to_owned(),
    );
    outcome
}
