use crate::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RadrootsTransportError, RadrootsTransportKind,
    RadrootsTransportMeshScopeId, RadrootsTransportTarget, RadrootsTransportTargetFingerprint,
    RadrootsTransportTargetLabel, RadrootsTransportTargetUri,
};

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

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumFragmentPolicyV1 {
    mode: ReticulumFragmentationModeV1,
    max_fragment_count: u16,
    max_reassembled_bytes: usize,
    duplicate_fragment_behavior: ReticulumDuplicateFragmentBehaviorV1,
    integrity_verification: ReticulumFragmentIntegrityV1,
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

    pub const fn mode(&self) -> ReticulumFragmentationModeV1 {
        self.mode
    }

    pub const fn max_fragment_count(&self) -> u16 {
        self.max_fragment_count
    }

    pub const fn max_reassembled_bytes(&self) -> usize {
        self.max_reassembled_bytes
    }

    pub const fn duplicate_fragment_behavior(&self) -> ReticulumDuplicateFragmentBehaviorV1 {
        self.duplicate_fragment_behavior
    }

    pub const fn integrity_verification(&self) -> ReticulumFragmentIntegrityV1 {
        self.integrity_verification
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumPayloadPolicyV1 {
    max_payload_bytes: usize,
    fragment_policy: ReticulumFragmentPolicyV1,
}

impl ReticulumPayloadPolicyV1 {
    pub const fn v1() -> Self {
        Self {
            max_payload_bytes: RETICULUM_V1_MAX_PAYLOAD_BYTES,
            fragment_policy: ReticulumFragmentPolicyV1::unsupported(),
        }
    }

    pub const fn max_payload_bytes(&self) -> usize {
        self.max_payload_bytes
    }

    pub const fn fragment_policy(&self) -> &ReticulumFragmentPolicyV1 {
        &self.fragment_policy
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
    scope: RadrootsTransportMeshScopeId,
    gateway: ReticulumGatewaySemanticsV1,
    privacy: ReticulumPrivacySemanticsV1,
}

impl ReticulumRoutingMetadataV1 {
    pub fn local() -> Self {
        Self {
            scope: RadrootsTransportMeshScopeId::local_reticulum(),
            gateway: ReticulumGatewaySemanticsV1::NoGatewayForwarding,
            privacy: ReticulumPrivacySemanticsV1::CanonicalSignedEventBytesOnly,
        }
    }

    pub const fn scope(&self) -> &RadrootsTransportMeshScopeId {
        &self.scope
    }

    pub const fn gateway(&self) -> ReticulumGatewaySemanticsV1 {
        self.gateway
    }

    pub const fn privacy(&self) -> ReticulumPrivacySemanticsV1 {
        self.privacy
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumDestinationV1 {
    uri: RadrootsTransportTargetUri,
    routing: ReticulumRoutingMetadataV1,
    label: Option<RadrootsTransportTargetLabel>,
    fingerprint: RadrootsTransportTargetFingerprint,
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
        scope: RadrootsTransportMeshScopeId,
        label: Option<RadrootsTransportTargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        let target = RadrootsTransportTarget::reticulum_with_metadata(
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

    pub fn from_target(target: &RadrootsTransportTarget) -> Result<Self, RadrootsTransportError> {
        if target.kind() != &RadrootsTransportKind::Reticulum
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

    pub fn transport_target(&self) -> Result<RadrootsTransportTarget, RadrootsTransportError> {
        RadrootsTransportTarget::reticulum_with_metadata(
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

    pub fn label(&self) -> Option<&RadrootsTransportTargetLabel> {
        self.label.as_ref()
    }

    pub fn fingerprint(&self) -> &RadrootsTransportTargetFingerprint {
        &self.fingerprint
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReticulumDestinationV1Wire {
    uri: RadrootsTransportTargetUri,
    routing: ReticulumRoutingMetadataV1,
    label: Option<RadrootsTransportTargetLabel>,
    fingerprint: RadrootsTransportTargetFingerprint,
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

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumCapabilityReportV1 {
    delivery_required: bool,
    fetch_required: bool,
    can_deliver: bool,
    can_fetch: bool,
    can_discover: bool,
    can_forward_gateway: bool,
    can_observe_receipts: bool,
    destination: ReticulumDestinationV1,
    payload_policy: ReticulumPayloadPolicyV1,
}

impl ReticulumCapabilityReportV1 {
    pub fn unavailable_local() -> Self {
        Self::unavailable(ReticulumDestinationV1::local(), true)
    }

    pub fn unavailable(destination: ReticulumDestinationV1, delivery_required: bool) -> Self {
        Self {
            delivery_required,
            fetch_required: false,
            can_deliver: false,
            can_fetch: false,
            can_discover: false,
            can_forward_gateway: false,
            can_observe_receipts: false,
            destination,
            payload_policy: ReticulumPayloadPolicyV1::v1(),
        }
    }

    pub const fn is_delivery_required(&self) -> bool {
        self.delivery_required
    }

    pub const fn is_fetch_required(&self) -> bool {
        self.fetch_required
    }

    pub const fn can_deliver(&self) -> bool {
        self.can_deliver
    }

    pub const fn can_fetch(&self) -> bool {
        self.can_fetch
    }

    pub const fn can_discover(&self) -> bool {
        self.can_discover
    }

    pub const fn can_forward_gateway(&self) -> bool {
        self.can_forward_gateway
    }

    pub const fn can_observe_receipts(&self) -> bool {
        self.can_observe_receipts
    }

    pub const fn destination(&self) -> &ReticulumDestinationV1 {
        &self.destination
    }

    pub const fn payload_policy(&self) -> &ReticulumPayloadPolicyV1 {
        &self.payload_policy
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReticulumFragmentPolicyV1Wire {
    mode: ReticulumFragmentationModeV1,
    max_fragment_count: u16,
    max_reassembled_bytes: usize,
    duplicate_fragment_behavior: ReticulumDuplicateFragmentBehaviorV1,
    integrity_verification: ReticulumFragmentIntegrityV1,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ReticulumFragmentPolicyV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ReticulumFragmentPolicyV1Wire::deserialize(deserializer)?;
        let policy = Self::unsupported();
        if wire.mode != policy.mode
            || wire.max_fragment_count != policy.max_fragment_count
            || wire.max_reassembled_bytes != policy.max_reassembled_bytes
            || wire.duplicate_fragment_behavior != policy.duplicate_fragment_behavior
            || wire.integrity_verification != policy.integrity_verification
        {
            return Err(serde::de::Error::custom(
                "Reticulum fragment policy must match the fixed v1 authority",
            ));
        }
        Ok(policy)
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReticulumPayloadPolicyV1Wire {
    max_payload_bytes: usize,
    fragment_policy: ReticulumFragmentPolicyV1,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ReticulumPayloadPolicyV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ReticulumPayloadPolicyV1Wire::deserialize(deserializer)?;
        let policy = Self::v1();
        if wire.max_payload_bytes != policy.max_payload_bytes
            || wire.fragment_policy != policy.fragment_policy
        {
            return Err(serde::de::Error::custom(
                "Reticulum payload policy must match the fixed v1 authority",
            ));
        }
        Ok(policy)
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReticulumCapabilityReportV1Wire {
    delivery_required: bool,
    fetch_required: bool,
    can_deliver: bool,
    can_fetch: bool,
    can_discover: bool,
    can_forward_gateway: bool,
    can_observe_receipts: bool,
    destination: ReticulumDestinationV1,
    payload_policy: ReticulumPayloadPolicyV1,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ReticulumCapabilityReportV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ReticulumCapabilityReportV1Wire::deserialize(deserializer)?;
        let report = Self::unavailable(wire.destination, wire.delivery_required);
        if wire.fetch_required != report.fetch_required
            || wire.can_deliver != report.can_deliver
            || wire.can_fetch != report.can_fetch
            || wire.can_discover != report.can_discover
            || wire.can_forward_gateway != report.can_forward_gateway
            || wire.can_observe_receipts != report.can_observe_receipts
            || wire.payload_policy != report.payload_policy
        {
            return Err(serde::de::Error::custom(
                "Reticulum capability report must match the unavailable v1 authority",
            ));
        }
        Ok(report)
    }
}
