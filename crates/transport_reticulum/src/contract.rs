use crate::RADROOTS_RETICULUM_ENDPOINT_URI;
use radroots_transport::target::{TargetFingerprint, TargetLabel, TargetScope};
use radroots_transport::{RadrootsTransportError, RadrootsTransportTargetUri, Target, TransportId};

pub const RETICULUM_V1_MAX_PAYLOAD_BYTES: usize = 64 * 1024;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReticulumFragmentationModeV1 {
    Unsupported,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReticulumDuplicateFragmentBehaviorV1 {
    Reject,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReticulumFragmentIntegrityV1 {
    PayloadDigest,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumFragmentPolicyV1 {
    pub mode: ReticulumFragmentationModeV1,
    pub max_fragment_count: u16,
    pub max_reassembled_bytes: usize,
    pub duplicate_fragment_behavior: ReticulumDuplicateFragmentBehaviorV1,
    pub integrity_verification: ReticulumFragmentIntegrityV1,
}

impl ReticulumFragmentPolicyV1 {
    pub const fn unsupported() -> Self {
        Self {
            mode: ReticulumFragmentationModeV1::Unsupported,
            max_fragment_count: 1,
            max_reassembled_bytes: RETICULUM_V1_MAX_PAYLOAD_BYTES,
            duplicate_fragment_behavior: ReticulumDuplicateFragmentBehaviorV1::Reject,
            integrity_verification: ReticulumFragmentIntegrityV1::PayloadDigest,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumPayloadPolicyV1 {
    pub max_payload_bytes: usize,
    pub fragment_policy: ReticulumFragmentPolicyV1,
}

impl ReticulumPayloadPolicyV1 {
    pub const fn v1() -> Self {
        Self {
            max_payload_bytes: RETICULUM_V1_MAX_PAYLOAD_BYTES,
            fragment_policy: ReticulumFragmentPolicyV1::unsupported(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReticulumGatewaySemanticsV1 {
    NoGatewayForwarding,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReticulumPrivacySemanticsV1 {
    CanonicalSignedEventBytesOnly,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumRoutingMetadataV1 {
    pub scope: TargetScope,
    pub gateway: ReticulumGatewaySemanticsV1,
    pub privacy: ReticulumPrivacySemanticsV1,
}

impl ReticulumRoutingMetadataV1 {
    pub fn local() -> Self {
        Self {
            scope: TargetScope::parse(crate::RADROOTS_RETICULUM_SCOPE_ID).expect("Reticulum scope"),
            gateway: ReticulumGatewaySemanticsV1::NoGatewayForwarding,
            privacy: ReticulumPrivacySemanticsV1::CanonicalSignedEventBytesOnly,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumDestinationV1 {
    uri: RadrootsTransportTargetUri,
    routing: ReticulumRoutingMetadataV1,
    label: Option<TargetLabel>,
    fingerprint: TargetFingerprint,
}

impl ReticulumDestinationV1 {
    pub fn local() -> Self {
        Self::new(
            RADROOTS_RETICULUM_ENDPOINT_URI,
            ReticulumRoutingMetadataV1::local().scope,
            None,
        )
        .expect("local Reticulum destination")
    }

    pub fn new(
        uri: impl AsRef<str>,
        scope: TargetScope,
        label: Option<TargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        let target = Target::new_with_metadata(
            TransportId::RETICULUM,
            uri.as_ref(),
            Some(scope),
            label.clone(),
        )?;
        Ok(Self {
            uri: target.uri().clone(),
            routing: ReticulumRoutingMetadataV1 {
                scope: target
                    .scope()
                    .cloned()
                    .expect("Reticulum destination scope"),
                gateway: ReticulumGatewaySemanticsV1::NoGatewayForwarding,
                privacy: ReticulumPrivacySemanticsV1::CanonicalSignedEventBytesOnly,
            },
            label: target.label().cloned(),
            fingerprint: target.fingerprint().clone(),
        })
    }

    pub fn from_target(target: &Target) -> Result<Self, RadrootsTransportError> {
        if target.kind() != &TransportId::RETICULUM
            || target.uri().as_str() != RADROOTS_RETICULUM_ENDPOINT_URI
        {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let Some(scope) = target.scope().cloned() else {
            return Err(RadrootsTransportError::EmptyTargetScope);
        };
        let destination = Self::new(target.uri().as_str(), scope, target.label().cloned())?;
        if destination.fingerprint != *target.fingerprint() {
            return Err(RadrootsTransportError::InvalidTargetFingerprint);
        }
        Ok(destination)
    }

    pub fn transport_target(&self) -> Result<Target, RadrootsTransportError> {
        Target::new_with_metadata(
            TransportId::RETICULUM,
            self.uri.as_str(),
            Some(self.routing.scope.clone()),
            self.label.clone(),
        )
    }

    pub fn uri(&self) -> &RadrootsTransportTargetUri {
        &self.uri
    }

    pub fn routing(&self) -> &ReticulumRoutingMetadataV1 {
        &self.routing
    }

    pub fn label(&self) -> Option<&TargetLabel> {
        self.label.as_ref()
    }

    pub fn fingerprint(&self) -> &TargetFingerprint {
        &self.fingerprint
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReticulumDestinationV1Wire {
    uri: RadrootsTransportTargetUri,
    routing: ReticulumRoutingMetadataV1,
    label: Option<TargetLabel>,
    fingerprint: TargetFingerprint,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ReticulumDestinationV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ReticulumDestinationV1Wire::deserialize(deserializer)?;
        let destination = Self::new(
            wire.uri.as_str(),
            wire.routing.scope.clone(),
            wire.label.clone(),
        )
        .map_err(serde::de::Error::custom)?;
        if destination.routing != wire.routing
            || destination.label != wire.label
            || destination.fingerprint != wire.fingerprint
        {
            return Err(serde::de::Error::custom(
                "Reticulum destination identity does not match its canonical fields",
            ));
        }
        Ok(destination)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumCapabilityReportV1 {
    pub delivery_required: bool,
    pub fetch_required: bool,
    pub can_deliver: bool,
    pub can_fetch: bool,
    pub can_discover: bool,
    pub can_forward_gateway: bool,
    pub can_observe_receipts: bool,
    pub destination: ReticulumDestinationV1,
    pub payload_policy: ReticulumPayloadPolicyV1,
}

impl ReticulumCapabilityReportV1 {
    pub fn unavailable_local() -> Self {
        Self {
            delivery_required: true,
            fetch_required: false,
            can_deliver: false,
            can_fetch: false,
            can_discover: false,
            can_forward_gateway: false,
            can_observe_receipts: false,
            destination: ReticulumDestinationV1::local(),
            payload_policy: ReticulumPayloadPolicyV1::v1(),
        }
    }
}
