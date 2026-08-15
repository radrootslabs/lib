//! Bounded, deterministic Prometheus-compatible metrics snapshots.

use core::fmt;
use std::collections::BTreeMap;
use std::error::Error;

use crate::{BuildInfo, ServicePhase, TaskClassification, TaskName};

pub const METRICS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";
pub const METRICS_MAX_DESCRIPTORS: usize = 64;
pub const METRICS_MAX_SAMPLES: usize = 512;
pub const METRICS_MAX_LABELS_PER_SAMPLE: usize = 8;
pub const METRICS_MAX_RENDER_UTF8_BYTES: usize = 1_048_576;

const METRIC_NAME_MAX_BYTES: usize = 128;
const METRIC_HELP_MAX_BYTES: usize = 512;
const METRIC_LABEL_VALUE_MAX_BYTES: usize = 128;
const STABLE_RELAY_ID_MAX_BYTES: usize = 64;

/// One of the five service-neutral metric interfaces owned by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommonMetricGroup {
    Build,
    Phase,
    Task,
    Storage,
    Transport,
}

/// Prometheus metric type supported by the bounded snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricKind {
    Counter,
    Gauge,
}

impl MetricKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gauge => "gauge",
        }
    }
}

/// Closed label-key inventory. Services cannot add arbitrary label dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetricLabelKey {
    Version,
    Revision,
    Phase,
    Task,
    Classification,
    Storage,
    Transport,
    RelayId,
    State,
    Outcome,
}

/// A bounded canonical name for a storage or transport component.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetricComponentId(String);

impl MetricComponentId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, MetricsContractError> {
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > METRIC_LABEL_VALUE_MAX_BYTES
            || !value.bytes().enumerate().all(|(index, byte)| {
                if index == 0 {
                    byte.is_ascii_lowercase()
                } else {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                }
            })
        {
            return Err(MetricsContractError::InvalidComponentId);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Closed storage and transport health vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricHealthState {
    Ready,
    Degraded,
    Unready,
    ReadOnly,
    RepairRequired,
    Unavailable,
}

impl MetricHealthState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Unready => "unready",
            Self::ReadOnly => "read_only",
            Self::RepairRequired => "repair_required",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Closed supervisor outcome vocabulary for task metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricTaskOutcome {
    ExpectedCompletion,
    OptionalFailure,
    TaskReturnedError,
    TaskPanicked,
    UnexpectedCompletion,
    UnexpectedCancellation,
    JoinFailed,
}

impl MetricTaskOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ExpectedCompletion => "expected_completion",
            Self::OptionalFailure => "optional_failure",
            Self::TaskReturnedError => "task_returned_error",
            Self::TaskPanicked => "task_panicked",
            Self::UnexpectedCompletion => "unexpected_completion",
            Self::UnexpectedCancellation => "unexpected_cancellation",
            Self::JoinFailed => "join_failed",
        }
    }
}

impl MetricLabelKey {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Revision => "revision",
            Self::Phase => "phase",
            Self::Task => "task",
            Self::Classification => "classification",
            Self::Storage => "storage",
            Self::Transport => "transport",
            Self::RelayId => "relay_id",
            Self::State => "state",
            Self::Outcome => "outcome",
        }
    }

    const fn allowed_for(self, group: CommonMetricGroup) -> bool {
        match group {
            CommonMetricGroup::Build => matches!(self, Self::Version | Self::Revision),
            CommonMetricGroup::Phase => matches!(self, Self::Phase),
            CommonMetricGroup::Task => {
                matches!(self, Self::Task | Self::Classification | Self::Outcome)
            }
            CommonMetricGroup::Storage => matches!(self, Self::Storage | Self::State),
            CommonMetricGroup::Transport => {
                matches!(self, Self::Transport | Self::RelayId | Self::State)
            }
        }
    }
}

/// A validated Prometheus metric name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetricName(String);

impl MetricName {
    pub fn new(value: impl AsRef<str>) -> Result<Self, MetricsContractError> {
        let value = value.as_ref();
        if value.is_empty() || value.len() > METRIC_NAME_MAX_BYTES || !valid_metric_name(value) {
            return Err(MetricsContractError::InvalidMetricName);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A canonical bounded relay identity suitable for a label value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StableRelayId(String);

impl StableRelayId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, MetricsContractError> {
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > STABLE_RELAY_ID_MAX_BYTES
            || !value.bytes().enumerate().all(|(index, byte)| {
                if index == 0 {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit()
                } else {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                }
            })
        {
            return Err(MetricsContractError::InvalidStableRelayId);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MetricLabelValue(String);

impl MetricLabelValue {
    fn new(value: impl AsRef<str>) -> Result<Self, MetricsContractError> {
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > METRIC_LABEL_VALUE_MAX_BYTES
            || !value.bytes().enumerate().all(|(index, byte)| {
                if index == 0 {
                    byte.is_ascii_alphanumeric()
                } else {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
                }
            })
        {
            return Err(MetricsContractError::InvalidLabelValue);
        }
        Ok(Self(value.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// One validated label with a closed key and bounded value.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetricLabel {
    key: MetricLabelKey,
    value: MetricLabelValue,
}

impl MetricLabel {
    pub fn build_version(build: &BuildInfo) -> Result<Self, MetricsContractError> {
        Self::validated(MetricLabelKey::Version, build.service_version())
    }

    pub fn build_revision(build: &BuildInfo) -> Result<Self, MetricsContractError> {
        Self::validated(MetricLabelKey::Revision, build.service_commit())
    }

    #[must_use]
    pub fn phase(phase: ServicePhase) -> Self {
        Self::fixed(
            MetricLabelKey::Phase,
            match phase {
                ServicePhase::Starting => "starting",
                ServicePhase::Ready => "ready",
                ServicePhase::Degraded => "degraded",
                ServicePhase::Unready => "unready",
                ServicePhase::Stopping => "stopping",
                ServicePhase::Failed => "failed",
            },
        )
    }

    pub fn task(task: &TaskName) -> Result<Self, MetricsContractError> {
        Self::validated(MetricLabelKey::Task, task.as_str())
    }

    #[must_use]
    pub fn classification(classification: TaskClassification) -> Self {
        Self::fixed(
            MetricLabelKey::Classification,
            match classification {
                TaskClassification::Critical => "critical",
                TaskClassification::Optional => "optional",
                TaskClassification::OneShot => "one_shot",
            },
        )
    }

    #[must_use]
    pub fn storage(storage: MetricComponentId) -> Self {
        Self::fixed_owned(MetricLabelKey::Storage, storage.0)
    }

    #[must_use]
    pub fn transport(transport: MetricComponentId) -> Self {
        Self::fixed_owned(MetricLabelKey::Transport, transport.0)
    }

    #[must_use]
    pub fn relay_id(value: StableRelayId) -> Self {
        Self::fixed_owned(MetricLabelKey::RelayId, value.0)
    }

    #[must_use]
    pub fn health_state(state: MetricHealthState) -> Self {
        Self::fixed(MetricLabelKey::State, state.as_str())
    }

    #[must_use]
    pub fn task_outcome(outcome: MetricTaskOutcome) -> Self {
        Self::fixed(MetricLabelKey::Outcome, outcome.as_str())
    }

    #[must_use]
    pub const fn key(&self) -> MetricLabelKey {
        self.key
    }

    #[must_use]
    pub fn value(&self) -> &str {
        self.value.as_str()
    }

    fn validated(key: MetricLabelKey, value: &str) -> Result<Self, MetricsContractError> {
        Ok(Self {
            key,
            value: MetricLabelValue::new(value)?,
        })
    }

    fn fixed(key: MetricLabelKey, value: &'static str) -> Self {
        Self {
            key,
            value: MetricLabelValue(value.to_owned()),
        }
    }

    fn fixed_owned(key: MetricLabelKey, value: String) -> Self {
        Self {
            key,
            value: MetricLabelValue(value),
        }
    }
}

impl fmt::Debug for MetricLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MetricLabel")
            .field("key", &self.key)
            .field("value", &"[redacted]")
            .finish()
    }
}

/// One validated metric family descriptor.
#[derive(Clone, PartialEq, Eq)]
pub struct MetricDescriptor {
    group: CommonMetricGroup,
    name: MetricName,
    help: String,
    kind: MetricKind,
    label_keys: Vec<MetricLabelKey>,
}

impl MetricDescriptor {
    pub fn new(
        group: CommonMetricGroup,
        name: MetricName,
        help: impl AsRef<str>,
        kind: MetricKind,
        label_keys: impl IntoIterator<Item = MetricLabelKey>,
    ) -> Result<Self, MetricsContractError> {
        let help = help.as_ref();
        if help.is_empty()
            || help.len() > METRIC_HELP_MAX_BYTES
            || help
                .chars()
                .any(|character| character.is_control() && character != '\n')
        {
            return Err(MetricsContractError::InvalidHelp);
        }
        let mut label_keys: Vec<_> = label_keys
            .into_iter()
            .take(METRICS_MAX_LABELS_PER_SAMPLE + 1)
            .collect();
        if label_keys.len() > METRICS_MAX_LABELS_PER_SAMPLE {
            return Err(MetricsContractError::TooManyLabels);
        }
        label_keys.sort_unstable();
        if label_keys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(MetricsContractError::DuplicateLabelKey);
        }
        if label_keys.iter().any(|key| !key.allowed_for(group)) {
            return Err(MetricsContractError::ForbiddenLabelKey);
        }
        Ok(Self {
            group,
            name,
            help: help.to_owned(),
            kind,
            label_keys,
        })
    }

    #[must_use]
    pub const fn group(&self) -> CommonMetricGroup {
        self.group
    }

    #[must_use]
    pub fn name(&self) -> &MetricName {
        &self.name
    }

    #[must_use]
    pub const fn kind(&self) -> MetricKind {
        self.kind
    }

    #[must_use]
    pub fn label_keys(&self) -> &[MetricLabelKey] {
        &self.label_keys
    }
}

impl fmt::Debug for MetricDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MetricDescriptor")
            .field("group", &self.group)
            .field("name", &self.name)
            .field("help", &"[redacted]")
            .field("kind", &self.kind)
            .field("label_keys", &self.label_keys)
            .finish()
    }
}

/// Exact integer sample value; floating-point ambiguity is intentionally absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricValue {
    Counter(u64),
    Gauge(i64),
}

impl MetricValue {
    const fn kind(self) -> MetricKind {
        match self {
            Self::Counter(_) => MetricKind::Counter,
            Self::Gauge(_) => MetricKind::Gauge,
        }
    }
}

/// One bounded sample associated with a descriptor by exact name.
#[derive(Clone, PartialEq, Eq)]
pub struct MetricSample {
    name: MetricName,
    value: MetricValue,
    labels: Vec<MetricLabel>,
}

impl MetricSample {
    pub fn new(
        name: MetricName,
        value: MetricValue,
        labels: impl IntoIterator<Item = MetricLabel>,
    ) -> Result<Self, MetricsContractError> {
        let mut labels: Vec<_> = labels
            .into_iter()
            .take(METRICS_MAX_LABELS_PER_SAMPLE + 1)
            .collect();
        if labels.len() > METRICS_MAX_LABELS_PER_SAMPLE {
            return Err(MetricsContractError::TooManyLabels);
        }
        labels.sort_unstable();
        if labels.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(MetricsContractError::DuplicateLabelKey);
        }
        Ok(Self {
            name,
            value,
            labels,
        })
    }

    #[must_use]
    pub fn name(&self) -> &MetricName {
        &self.name
    }

    #[must_use]
    pub const fn value(&self) -> MetricValue {
        self.value
    }

    #[must_use]
    pub fn labels(&self) -> &[MetricLabel] {
        &self.labels
    }
}

impl fmt::Debug for MetricSample {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MetricSample")
            .field("name", &self.name)
            .field("value", &self.value)
            .field("label_count", &self.labels.len())
            .finish()
    }
}

/// An immutable validated metrics snapshot with deterministic ordering.
#[derive(Clone, PartialEq, Eq)]
pub struct BoundedMetricsSnapshot {
    descriptors: Vec<MetricDescriptor>,
    samples: Vec<MetricSample>,
}

impl BoundedMetricsSnapshot {
    pub fn new(
        descriptors: impl IntoIterator<Item = MetricDescriptor>,
        samples: impl IntoIterator<Item = MetricSample>,
    ) -> Result<Self, MetricsContractError> {
        let mut descriptors: Vec<_> = descriptors
            .into_iter()
            .take(METRICS_MAX_DESCRIPTORS + 1)
            .collect();
        let mut samples: Vec<_> = samples.into_iter().take(METRICS_MAX_SAMPLES + 1).collect();
        if descriptors.len() > METRICS_MAX_DESCRIPTORS {
            return Err(MetricsContractError::TooManyDescriptors);
        }
        if samples.len() > METRICS_MAX_SAMPLES {
            return Err(MetricsContractError::TooManySamples);
        }
        descriptors.sort_by(|left, right| left.name.cmp(&right.name));
        if descriptors
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name)
        {
            return Err(MetricsContractError::DuplicateDescriptor);
        }
        let descriptor_by_name: BTreeMap<_, _> = descriptors
            .iter()
            .map(|descriptor| (&descriptor.name, descriptor))
            .collect();
        for sample in &samples {
            let descriptor = descriptor_by_name
                .get(&sample.name)
                .ok_or(MetricsContractError::UnknownDescriptor)?;
            if sample.value.kind() != descriptor.kind {
                return Err(MetricsContractError::ValueKindMismatch);
            }
            let actual_keys: Vec<_> = sample.labels.iter().map(|label| label.key).collect();
            if actual_keys != descriptor.label_keys {
                return Err(MetricsContractError::LabelSetMismatch);
            }
        }
        samples.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.labels.cmp(&right.labels))
        });
        if samples
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name && pair[0].labels == pair[1].labels)
        {
            return Err(MetricsContractError::DuplicateSample);
        }
        Ok(Self {
            descriptors,
            samples,
        })
    }

    #[must_use]
    pub fn descriptors(&self) -> &[MetricDescriptor] {
        &self.descriptors
    }

    #[must_use]
    pub fn samples(&self) -> &[MetricSample] {
        &self.samples
    }

    /// Renders deterministic Prometheus text without exceeding `maximum_bytes`.
    pub fn render(&self, maximum_bytes: usize) -> Result<Vec<u8>, MetricsRenderError> {
        if maximum_bytes == 0 || maximum_bytes > METRICS_MAX_RENDER_UTF8_BYTES {
            return Err(MetricsRenderError::InvalidMaximum);
        }
        let mut output = BoundedOutput::new(maximum_bytes);
        for descriptor in &self.descriptors {
            output.push_str("# HELP ")?;
            output.push_str(descriptor.name.as_str())?;
            output.push_byte(b' ')?;
            output.push_escaped_help(&descriptor.help)?;
            output.push_byte(b'\n')?;
            output.push_str("# TYPE ")?;
            output.push_str(descriptor.name.as_str())?;
            output.push_byte(b' ')?;
            output.push_str(descriptor.kind.as_str())?;
            output.push_byte(b'\n')?;

            for sample in self
                .samples
                .iter()
                .filter(|sample| sample.name == descriptor.name)
            {
                output.push_str(sample.name.as_str())?;
                if !sample.labels.is_empty() {
                    output.push_byte(b'{')?;
                    for (index, label) in sample.labels.iter().enumerate() {
                        if index != 0 {
                            output.push_byte(b',')?;
                        }
                        output.push_str(label.key.as_str())?;
                        output.push_str("=\"")?;
                        output.push_escaped_label_value(label.value.as_str())?;
                        output.push_byte(b'\"')?;
                    }
                    output.push_byte(b'}')?;
                }
                output.push_byte(b' ')?;
                match sample.value {
                    MetricValue::Counter(value) => output.push_str(&value.to_string())?,
                    MetricValue::Gauge(value) => output.push_str(&value.to_string())?,
                }
                output.push_byte(b'\n')?;
            }
        }
        Ok(output.finish())
    }
}

impl fmt::Debug for BoundedMetricsSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundedMetricsSnapshot")
            .field("descriptor_count", &self.descriptors.len())
            .field("sample_count", &self.samples.len())
            .finish()
    }
}

/// Safe construction failure for the metrics contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricsContractError {
    InvalidMetricName,
    InvalidHelp,
    InvalidLabelValue,
    InvalidStableRelayId,
    InvalidComponentId,
    ForbiddenLabelKey,
    DuplicateLabelKey,
    TooManyLabels,
    TooManyDescriptors,
    TooManySamples,
    DuplicateDescriptor,
    UnknownDescriptor,
    ValueKindMismatch,
    LabelSetMismatch,
    DuplicateSample,
}

impl fmt::Display for MetricsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("metrics snapshot violates the bounded host contract")
    }
}

impl Error for MetricsContractError {}

/// Safe deterministic-rendering failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricsRenderError {
    InvalidMaximum,
    ResponseTooLarge,
}

impl fmt::Display for MetricsRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("metrics snapshot cannot be rendered within the response limit")
    }
}

impl Error for MetricsRenderError {}

struct BoundedOutput {
    bytes: Vec<u8>,
    maximum: usize,
}

impl BoundedOutput {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(maximum.min(4096)),
            maximum,
        }
    }

    fn push_byte(&mut self, byte: u8) -> Result<(), MetricsRenderError> {
        if self.bytes.len() == self.maximum {
            return Err(MetricsRenderError::ResponseTooLarge);
        }
        self.bytes.push(byte);
        Ok(())
    }

    fn push_str(&mut self, value: &str) -> Result<(), MetricsRenderError> {
        let remaining = self.maximum.saturating_sub(self.bytes.len());
        if value.len() > remaining {
            return Err(MetricsRenderError::ResponseTooLarge);
        }
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn push_escaped_help(&mut self, value: &str) -> Result<(), MetricsRenderError> {
        for character in value.chars() {
            match character {
                '\\' => self.push_str("\\\\")?,
                '\n' => self.push_str("\\n")?,
                _ => {
                    let mut bytes = [0; 4];
                    self.push_str(character.encode_utf8(&mut bytes))?;
                }
            }
        }
        Ok(())
    }

    fn push_escaped_label_value(&mut self, value: &str) -> Result<(), MetricsRenderError> {
        for character in value.chars() {
            match character {
                '\\' => self.push_str("\\\\")?,
                '\"' => self.push_str("\\\"")?,
                '\n' => self.push_str("\\n")?,
                _ => {
                    let mut bytes = [0; 4];
                    self.push_str(character.encode_utf8(&mut bytes))?;
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn valid_metric_name(value: &str) -> bool {
    value.bytes().enumerate().all(|(index, byte)| {
        if index == 0 {
            byte.is_ascii_alphabetic() || matches!(byte, b'_' | b':')
        } else {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':')
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::error::Error;

    use super::*;
    use crate::{BuildInfoEnvironment, BuildMode, ContractVersions};

    fn name(value: &str) -> MetricName {
        MetricName::new(value).unwrap()
    }

    fn descriptor(
        group: CommonMetricGroup,
        metric_name: &str,
        kind: MetricKind,
        labels: &[MetricLabelKey],
    ) -> MetricDescriptor {
        MetricDescriptor::new(
            group,
            name(metric_name),
            "safe help",
            kind,
            labels.iter().copied(),
        )
        .unwrap()
    }

    fn build_info() -> BuildInfo {
        BuildInfo::from_compile_time(
            BuildMode::Release,
            BuildInfoEnvironment {
                service_version: Some("1.2.3"),
                service_commit: Some("1111111111111111111111111111111111111111"),
                lib_revision: Some("2222222222222222222222222222222222222222"),
                rust_version: Some("1.97.0"),
                target: Some("x86_64-unknown-linux-gnu"),
                feature_profile: Some("release"),
                contract_versions: ContractVersions::new(1, 1, 1, 1, 1).unwrap(),
            },
        )
        .unwrap()
    }

    #[test]
    fn arbitrary_and_cross_group_labels_are_rejected() {
        assert_eq!(
            MetricDescriptor::new(
                CommonMetricGroup::Build,
                name("radroots_build_info"),
                "build information",
                MetricKind::Gauge,
                [MetricLabelKey::RelayId],
            ),
            Err(MetricsContractError::ForbiddenLabelKey)
        );
        assert_eq!(
            MetricDescriptor::new(
                CommonMetricGroup::Transport,
                name("radroots_transport_state"),
                "transport state",
                MetricKind::Gauge,
                [MetricLabelKey::RelayId, MetricLabelKey::RelayId],
            ),
            Err(MetricsContractError::DuplicateLabelKey)
        );
        assert_eq!(
            MetricName::new("bad metric"),
            Err(MetricsContractError::InvalidMetricName)
        );
    }

    #[test]
    fn relay_labels_require_stable_ids_and_reject_raw_urls() {
        let stable = StableRelayId::new("relay-west-01").unwrap();
        let label = MetricLabel::relay_id(stable.clone());
        assert_eq!(label.value(), stable.as_str());
        for forbidden in [
            "wss://relay.example.com",
            "Relay-West",
            "relay west",
            "-relay",
            "",
        ] {
            assert_eq!(
                StableRelayId::new(forbidden),
                Err(MetricsContractError::InvalidStableRelayId)
            );
        }
    }

    #[test]
    fn rendering_is_sorted_and_escapes_help() {
        let descriptor = MetricDescriptor::new(
            CommonMetricGroup::Task,
            name("radroots_task_outcomes_total"),
            "task\\outcomes\nby state",
            MetricKind::Counter,
            [MetricLabelKey::Task, MetricLabelKey::Outcome],
        )
        .unwrap();
        let sample = MetricSample::new(
            name("radroots_task_outcomes_total"),
            MetricValue::Counter(7),
            [
                MetricLabel::task_outcome(MetricTaskOutcome::OptionalFailure),
                MetricLabel::task(&TaskName::new("worker_one").unwrap()).unwrap(),
            ],
        )
        .unwrap();
        let snapshot = BoundedMetricsSnapshot::new([descriptor], [sample]).unwrap();
        assert_eq!(
            String::from_utf8(snapshot.render(1024).unwrap()).unwrap(),
            concat!(
                "# HELP radroots_task_outcomes_total task\\\\outcomes\\nby state\n",
                "# TYPE radroots_task_outcomes_total counter\n",
                "radroots_task_outcomes_total{task=\"worker_one\",outcome=\"optional_failure\"} 7\n",
            )
        );

        let mut defensive = BoundedOutput::new(64);
        defensive
            .push_escaped_label_value("worker\\\"one\n")
            .unwrap();
        assert_eq!(defensive.finish(), b"worker\\\\\\\"one\\n");
    }

    #[test]
    fn component_ids_reject_free_text_urls_paths_and_control_characters() {
        for forbidden in [
            "raw error text",
            "wss://relay.example.com",
            "/private/path",
            "line\nfeed",
            "quote\"value",
            "back\\slash",
            "",
        ] {
            assert_eq!(
                MetricComponentId::new(forbidden),
                Err(MetricsContractError::InvalidComponentId)
            );
        }
        assert!(MetricComponentId::new("sqlite_writer").is_ok());
    }

    #[test]
    fn duplicates_unknowns_kind_and_label_mismatches_fail_closed() {
        let phase = descriptor(
            CommonMetricGroup::Phase,
            "radroots_phase",
            MetricKind::Gauge,
            &[MetricLabelKey::Phase],
        );
        assert_eq!(
            BoundedMetricsSnapshot::new([phase.clone(), phase.clone()], []),
            Err(MetricsContractError::DuplicateDescriptor)
        );
        let unknown =
            MetricSample::new(name("radroots_unknown"), MetricValue::Gauge(1), []).unwrap();
        assert_eq!(
            BoundedMetricsSnapshot::new([phase.clone()], [unknown]),
            Err(MetricsContractError::UnknownDescriptor)
        );
        let wrong_kind = MetricSample::new(
            name("radroots_phase"),
            MetricValue::Counter(1),
            [MetricLabel::phase(ServicePhase::Ready)],
        )
        .unwrap();
        assert_eq!(
            BoundedMetricsSnapshot::new([phase.clone()], [wrong_kind]),
            Err(MetricsContractError::ValueKindMismatch)
        );
        let wrong_labels =
            MetricSample::new(name("radroots_phase"), MetricValue::Gauge(1), []).unwrap();
        assert_eq!(
            BoundedMetricsSnapshot::new([phase], [wrong_labels]),
            Err(MetricsContractError::LabelSetMismatch)
        );
    }

    #[test]
    fn render_and_collection_bounds_fail_before_overgrowth() {
        let descriptor = descriptor(
            CommonMetricGroup::Phase,
            "radroots_phase",
            MetricKind::Gauge,
            &[MetricLabelKey::Phase],
        );
        let sample = MetricSample::new(
            name("radroots_phase"),
            MetricValue::Gauge(1),
            [MetricLabel::phase(ServicePhase::Ready)],
        )
        .unwrap();
        let snapshot = BoundedMetricsSnapshot::new([descriptor.clone()], [sample]).unwrap();
        let exact = snapshot.render(METRICS_MAX_RENDER_UTF8_BYTES).unwrap();
        assert_eq!(snapshot.render(exact.len()).unwrap(), exact);
        assert_eq!(
            snapshot.render(exact.len() - 1),
            Err(MetricsRenderError::ResponseTooLarge)
        );
        assert_eq!(snapshot.render(0), Err(MetricsRenderError::InvalidMaximum));
        assert_eq!(
            snapshot.render(METRICS_MAX_RENDER_UTF8_BYTES + 1),
            Err(MetricsRenderError::InvalidMaximum)
        );

        assert_eq!(
            BoundedMetricsSnapshot::new(
                std::iter::repeat_n(descriptor, METRICS_MAX_DESCRIPTORS + 1),
                [],
            ),
            Err(MetricsContractError::TooManyDescriptors)
        );
    }

    #[test]
    fn every_common_group_and_label_key_has_a_stable_inventory() {
        assert_eq!(METRICS_MAX_DESCRIPTORS, 64);
        assert_eq!(METRICS_MAX_SAMPLES, 512);
        assert_eq!(METRICS_MAX_LABELS_PER_SAMPLE, 8);
        assert_eq!(METRICS_MAX_RENDER_UTF8_BYTES, 1_048_576);
        assert_eq!(METRIC_NAME_MAX_BYTES, 128);
        assert_eq!(METRIC_HELP_MAX_BYTES, 512);
        assert_eq!(METRIC_LABEL_VALUE_MAX_BYTES, 128);
        assert_eq!(STABLE_RELAY_ID_MAX_BYTES, 64);

        let inventory = [
            (
                CommonMetricGroup::Build,
                &[MetricLabelKey::Version, MetricLabelKey::Revision][..],
            ),
            (CommonMetricGroup::Phase, &[MetricLabelKey::Phase][..]),
            (
                CommonMetricGroup::Task,
                &[
                    MetricLabelKey::Task,
                    MetricLabelKey::Classification,
                    MetricLabelKey::Outcome,
                ][..],
            ),
            (
                CommonMetricGroup::Storage,
                &[MetricLabelKey::Storage, MetricLabelKey::State][..],
            ),
            (
                CommonMetricGroup::Transport,
                &[
                    MetricLabelKey::Transport,
                    MetricLabelKey::RelayId,
                    MetricLabelKey::State,
                ][..],
            ),
        ];
        let all_keys: BTreeSet<_> = [
            MetricLabelKey::Version,
            MetricLabelKey::Revision,
            MetricLabelKey::Phase,
            MetricLabelKey::Task,
            MetricLabelKey::Classification,
            MetricLabelKey::Storage,
            MetricLabelKey::Transport,
            MetricLabelKey::RelayId,
            MetricLabelKey::State,
            MetricLabelKey::Outcome,
        ]
        .into_iter()
        .collect();
        assert_eq!(
            inventory
                .iter()
                .flat_map(|(_, keys)| keys.iter().copied())
                .collect::<BTreeSet<_>>(),
            all_keys
        );
        for (group, allowed) in inventory {
            for key in all_keys.iter().copied() {
                assert_eq!(key.allowed_for(group), allowed.contains(&key));
            }
        }
    }

    #[test]
    fn typed_label_vocabularies_are_exact_and_non_bypassable() {
        let phases = [
            ServicePhase::Starting,
            ServicePhase::Ready,
            ServicePhase::Degraded,
            ServicePhase::Unready,
            ServicePhase::Stopping,
            ServicePhase::Failed,
        ]
        .map(MetricLabel::phase);
        assert_eq!(
            phases.map(|label| label.value().to_owned()),
            [
                "starting", "ready", "degraded", "unready", "stopping", "failed"
            ]
            .map(str::to_owned)
        );
        assert_eq!(
            [
                TaskClassification::Critical,
                TaskClassification::Optional,
                TaskClassification::OneShot,
            ]
            .map(MetricLabel::classification)
            .map(|label| label.value().to_owned()),
            ["critical", "optional", "one_shot"].map(str::to_owned)
        );
        assert_eq!(
            [
                MetricHealthState::Ready,
                MetricHealthState::Degraded,
                MetricHealthState::Unready,
                MetricHealthState::ReadOnly,
                MetricHealthState::RepairRequired,
                MetricHealthState::Unavailable,
            ]
            .map(MetricLabel::health_state)
            .map(|label| label.value().to_owned()),
            [
                "ready",
                "degraded",
                "unready",
                "read_only",
                "repair_required",
                "unavailable",
            ]
            .map(str::to_owned)
        );
        assert_eq!(
            [
                MetricTaskOutcome::ExpectedCompletion,
                MetricTaskOutcome::OptionalFailure,
                MetricTaskOutcome::TaskReturnedError,
                MetricTaskOutcome::TaskPanicked,
                MetricTaskOutcome::UnexpectedCompletion,
                MetricTaskOutcome::UnexpectedCancellation,
                MetricTaskOutcome::JoinFailed,
            ]
            .map(MetricLabel::task_outcome)
            .map(|label| label.value().to_owned()),
            [
                "expected_completion",
                "optional_failure",
                "task_returned_error",
                "task_panicked",
                "unexpected_completion",
                "unexpected_cancellation",
                "join_failed",
            ]
            .map(str::to_owned)
        );
    }

    #[test]
    fn infinite_iterators_are_bounded_during_ingestion() {
        assert_eq!(
            MetricDescriptor::new(
                CommonMetricGroup::Task,
                name("radroots_task"),
                "task",
                MetricKind::Gauge,
                std::iter::repeat(MetricLabelKey::Task),
            ),
            Err(MetricsContractError::TooManyLabels)
        );
        assert_eq!(
            MetricSample::new(
                name("radroots_phase"),
                MetricValue::Gauge(1),
                std::iter::repeat(MetricLabel::phase(ServicePhase::Ready)),
            ),
            Err(MetricsContractError::TooManyLabels)
        );

        let descriptor = descriptor(
            CommonMetricGroup::Phase,
            "radroots_phase",
            MetricKind::Gauge,
            &[MetricLabelKey::Phase],
        );
        let sample = MetricSample::new(
            name("radroots_phase"),
            MetricValue::Gauge(1),
            [MetricLabel::phase(ServicePhase::Ready)],
        )
        .unwrap();
        assert_eq!(
            BoundedMetricsSnapshot::new(std::iter::repeat(descriptor.clone()), []),
            Err(MetricsContractError::TooManyDescriptors)
        );
        assert_eq!(
            BoundedMetricsSnapshot::new([descriptor], std::iter::repeat(sample)),
            Err(MetricsContractError::TooManySamples)
        );
    }

    #[test]
    fn accessors_debug_errors_and_remaining_bounds_are_exact() {
        let build = build_info();
        let version = MetricLabel::build_version(&build).unwrap();
        let revision = MetricLabel::build_revision(&build).unwrap();
        assert_eq!(version.key(), MetricLabelKey::Version);
        assert_eq!(version.value(), "1.2.3");
        assert_eq!(revision.key(), MetricLabelKey::Revision);
        assert_eq!(revision.value(), "1111111111111111111111111111111111111111");
        assert_eq!(
            format!("{version:?}"),
            "MetricLabel { key: Version, value: \"[redacted]\" }"
        );

        let storage_id = MetricComponentId::new("sqlite_writer").unwrap();
        assert_eq!(storage_id.as_str(), "sqlite_writer");
        let transport_id = MetricComponentId::new("nostr_relay").unwrap();
        assert_eq!(transport_id.as_str(), "nostr_relay");
        assert_eq!(MetricLabel::storage(storage_id).value(), "sqlite_writer");
        assert_eq!(MetricLabel::transport(transport_id).value(), "nostr_relay");
        assert!(MetricComponentId::new("a0").is_ok());
        assert!(MetricComponentId::new("a_").is_ok());
        assert_eq!(
            MetricComponentId::new("a-").unwrap_err(),
            MetricsContractError::InvalidComponentId
        );

        let metric_name = name("radroots_build_info");
        assert_eq!(metric_name.as_str(), "radroots_build_info");
        let build_descriptor = MetricDescriptor::new(
            CommonMetricGroup::Build,
            metric_name.clone(),
            "build identity",
            MetricKind::Gauge,
            [MetricLabelKey::Revision, MetricLabelKey::Version],
        )
        .unwrap();
        assert_eq!(build_descriptor.group(), CommonMetricGroup::Build);
        assert_eq!(build_descriptor.name(), &metric_name);
        assert_eq!(build_descriptor.kind(), MetricKind::Gauge);
        assert_eq!(
            build_descriptor.label_keys(),
            &[MetricLabelKey::Version, MetricLabelKey::Revision]
        );
        assert!(format!("{build_descriptor:?}").contains("help: \"[redacted]\""));

        let sample =
            MetricSample::new(metric_name, MetricValue::Gauge(1), [revision, version]).unwrap();
        assert_eq!(sample.name().as_str(), "radroots_build_info");
        assert_eq!(sample.value(), MetricValue::Gauge(1));
        assert_eq!(sample.labels().len(), 2);
        assert!(format!("{sample:?}").contains("label_count: 2"));
        assert_eq!(
            MetricSample::new(
                name("radroots_duplicate_labels"),
                MetricValue::Gauge(1),
                [
                    MetricLabel::phase(ServicePhase::Ready),
                    MetricLabel::phase(ServicePhase::Degraded),
                ],
            ),
            Err(MetricsContractError::DuplicateLabelKey)
        );

        let snapshot = BoundedMetricsSnapshot::new([build_descriptor], [sample.clone()]).unwrap();
        assert_eq!(snapshot.descriptors().len(), 1);
        assert_eq!(snapshot.samples(), std::slice::from_ref(&sample));
        assert_eq!(
            format!("{snapshot:?}"),
            "BoundedMetricsSnapshot { descriptor_count: 1, sample_count: 1 }"
        );
        let rendered = String::from_utf8(snapshot.render(4096).unwrap()).unwrap();
        assert!(rendered.contains("version=\"1.2.3\",revision="));
        assert_eq!(
            BoundedMetricsSnapshot::new(snapshot.descriptors().to_vec(), [sample.clone(), sample]),
            Err(MetricsContractError::DuplicateSample)
        );

        let phase_descriptor = descriptor(
            CommonMetricGroup::Phase,
            "radroots_phase_limit",
            MetricKind::Gauge,
            &[MetricLabelKey::Phase],
        );
        let phase_sample = MetricSample::new(
            name("radroots_phase_limit"),
            MetricValue::Gauge(1),
            [MetricLabel::phase(ServicePhase::Ready)],
        )
        .unwrap();
        assert_eq!(
            BoundedMetricsSnapshot::new(
                [phase_descriptor],
                std::iter::repeat_n(phase_sample, METRICS_MAX_SAMPLES + 1),
            ),
            Err(MetricsContractError::TooManySamples)
        );

        for invalid_help in [
            String::new(),
            "x".repeat(METRIC_HELP_MAX_BYTES + 1),
            "unsafe\u{0000}help".to_owned(),
        ] {
            assert_eq!(
                MetricDescriptor::new(
                    CommonMetricGroup::Phase,
                    name("radroots_invalid_help"),
                    invalid_help,
                    MetricKind::Gauge,
                    [MetricLabelKey::Phase],
                ),
                Err(MetricsContractError::InvalidHelp)
            );
        }
        for invalid_value in ["", "-leading", "contains space"] {
            assert!(matches!(
                MetricLabelValue::new(invalid_value),
                Err(MetricsContractError::InvalidLabelValue)
            ));
        }
        assert!(MetricLabelValue::new("A0._:-").is_ok());
        assert!(matches!(
            MetricLabelValue::new("A/"),
            Err(MetricsContractError::InvalidLabelValue)
        ));
        assert!(MetricName::new("_metric").is_ok());
        assert!(MetricName::new(":metric9").is_ok());
        assert!(MetricName::new("a".repeat(METRIC_NAME_MAX_BYTES)).is_ok());
        assert_eq!(
            MetricName::new("").unwrap_err(),
            MetricsContractError::InvalidMetricName
        );
        assert_eq!(
            MetricName::new("9metric").unwrap_err(),
            MetricsContractError::InvalidMetricName
        );
        assert_eq!(
            MetricName::new("x".repeat(METRIC_NAME_MAX_BYTES + 1)),
            Err(MetricsContractError::InvalidMetricName)
        );
        assert_eq!(
            StableRelayId::new("x".repeat(STABLE_RELAY_ID_MAX_BYTES + 1)),
            Err(MetricsContractError::InvalidStableRelayId)
        );
        assert!(StableRelayId::new("x".repeat(STABLE_RELAY_ID_MAX_BYTES)).is_ok());
        assert_eq!(
            MetricComponentId::new("x".repeat(METRIC_LABEL_VALUE_MAX_BYTES + 1)),
            Err(MetricsContractError::InvalidComponentId)
        );
        assert!(MetricComponentId::new("x".repeat(METRIC_LABEL_VALUE_MAX_BYTES)).is_ok());
        assert!(MetricLabelValue::new("x".repeat(METRIC_LABEL_VALUE_MAX_BYTES)).is_ok());
        assert!(matches!(
            MetricLabelValue::new("x".repeat(METRIC_LABEL_VALUE_MAX_BYTES + 1)),
            Err(MetricsContractError::InvalidLabelValue)
        ));

        let exact_help = "x".repeat(METRIC_HELP_MAX_BYTES);
        assert!(
            MetricDescriptor::new(
                CommonMetricGroup::Phase,
                name("radroots_exact_help"),
                &exact_help,
                MetricKind::Gauge,
                [MetricLabelKey::Phase],
            )
            .is_ok()
        );

        let very_large = "x".repeat(4 * 1024 * 1024);
        assert_eq!(
            MetricName::new(&very_large),
            Err(MetricsContractError::InvalidMetricName)
        );
        assert_eq!(
            StableRelayId::new(&very_large),
            Err(MetricsContractError::InvalidStableRelayId)
        );
        assert_eq!(
            MetricComponentId::new(&very_large),
            Err(MetricsContractError::InvalidComponentId)
        );
        assert!(matches!(
            MetricLabelValue::new(&very_large),
            Err(MetricsContractError::InvalidLabelValue)
        ));
        assert_eq!(
            MetricDescriptor::new(
                CommonMetricGroup::Phase,
                name("radroots_very_large_help"),
                &very_large,
                MetricKind::Gauge,
                [MetricLabelKey::Phase],
            ),
            Err(MetricsContractError::InvalidHelp)
        );

        for error in [
            MetricsContractError::InvalidMetricName.to_string(),
            MetricsRenderError::InvalidMaximum.to_string(),
        ] {
            assert!(!error.is_empty());
        }
        assert!(MetricsContractError::InvalidMetricName.source().is_none());
        assert!(MetricsRenderError::ResponseTooLarge.source().is_none());
    }
}
