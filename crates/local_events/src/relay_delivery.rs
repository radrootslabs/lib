#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    LocalEventsError, canonical_relay_set_fingerprint, relay_url::RelayUrlValidationError,
    relay_url::normalize_relay_urls,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayDeliveryState {
    Pending,
    Acknowledged,
    Failed,
    Observed,
}

impl RelayDeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Acknowledged => "acknowledged",
            Self::Failed => "failed",
            Self::Observed => "observed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayDeliveryFailure {
    pub relay_url: String,
    pub error: String,
}

impl RelayDeliveryFailure {
    pub fn new(
        relay_url: impl AsRef<str>,
        error: impl AsRef<str>,
    ) -> Result<Self, LocalEventsError> {
        let relay_url = normalize_relay_url_for_evidence("failed_relays.relay_url", relay_url)?;
        let error = normalize_non_empty_text("failed_relays.error", error.as_ref())?;
        Ok(Self { relay_url, error })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayDeliveryEvidence {
    pub state: RelayDeliveryState,
    pub target_relays: Vec<String>,
    pub connected_relays: Vec<String>,
    pub acknowledged_relays: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observed_relays: Vec<String>,
    pub failed_relays: Vec<RelayDeliveryFailure>,
}

impl RelayDeliveryEvidence {
    pub fn pending<I, S>(target_relays: I) -> Result<Self, LocalEventsError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::build(
            RelayDeliveryState::Pending,
            target_relays,
            Vec::<String>::new(),
            Vec::<String>::new(),
            Vec::<String>::new(),
            Vec::new(),
        )
    }

    pub fn acknowledged<I, S, J, T, K, U>(
        target_relays: I,
        connected_relays: J,
        acknowledged_relays: K,
        failed_relays: Vec<RelayDeliveryFailure>,
    ) -> Result<Self, LocalEventsError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        J: IntoIterator<Item = T>,
        T: AsRef<str>,
        K: IntoIterator<Item = U>,
        U: AsRef<str>,
    {
        Self::build(
            RelayDeliveryState::Acknowledged,
            target_relays,
            connected_relays,
            acknowledged_relays,
            Vec::<String>::new(),
            failed_relays,
        )
    }

    pub fn observed<I, S, J, T, K, U>(
        target_relays: I,
        connected_relays: J,
        observed_relays: K,
        failed_relays: Vec<RelayDeliveryFailure>,
    ) -> Result<Self, LocalEventsError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        J: IntoIterator<Item = T>,
        T: AsRef<str>,
        K: IntoIterator<Item = U>,
        U: AsRef<str>,
    {
        Self::build(
            RelayDeliveryState::Observed,
            target_relays,
            connected_relays,
            Vec::<String>::new(),
            observed_relays,
            failed_relays,
        )
    }

    pub fn failed<I, S, J, T>(
        target_relays: I,
        connected_relays: J,
        failed_relays: Vec<RelayDeliveryFailure>,
    ) -> Result<Self, LocalEventsError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        J: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        Self::build(
            RelayDeliveryState::Failed,
            target_relays,
            connected_relays,
            Vec::<String>::new(),
            Vec::<String>::new(),
            failed_relays,
        )
    }

    pub fn validate(&self) -> Result<(), LocalEventsError> {
        validate_relay_set("target_relays", &self.target_relays, true)?;
        validate_relay_set("connected_relays", &self.connected_relays, false)?;
        validate_relay_set("acknowledged_relays", &self.acknowledged_relays, false)?;
        validate_relay_set("observed_relays", &self.observed_relays, false)?;
        for failure in &self.failed_relays {
            let normalized =
                normalize_relay_url_for_evidence("failed_relays.relay_url", &failure.relay_url)?;
            if normalized != failure.relay_url {
                return Err(invalid_evidence(
                    "failed_relays.relay_url must be normalized and deduplicated",
                ));
            }
            let normalized_error = normalize_non_empty_text("failed_relays.error", &failure.error)?;
            if normalized_error != failure.error {
                return Err(invalid_evidence("failed_relays.error must be trimmed"));
            }
        }
        match self.state {
            RelayDeliveryState::Pending => {
                if !self.acknowledged_relays.is_empty()
                    || !self.observed_relays.is_empty()
                    || !self.failed_relays.is_empty()
                {
                    return Err(invalid_evidence(
                        "pending delivery evidence must not include acknowledged, observed, or failed relays",
                    ));
                }
            }
            RelayDeliveryState::Acknowledged => {
                if self.acknowledged_relays.is_empty() {
                    return Err(invalid_evidence(
                        "acknowledged delivery evidence requires acknowledged_relays",
                    ));
                }
                if !self.observed_relays.is_empty() {
                    return Err(invalid_evidence(
                        "acknowledged delivery evidence must not include observed_relays",
                    ));
                }
            }
            RelayDeliveryState::Failed => {
                if !self.acknowledged_relays.is_empty()
                    || !self.observed_relays.is_empty()
                    || self.failed_relays.is_empty()
                {
                    return Err(invalid_evidence(
                        "failed delivery evidence requires failed_relays and no acknowledged or observed relays",
                    ));
                }
            }
            RelayDeliveryState::Observed => {
                if !self.acknowledged_relays.is_empty() {
                    return Err(invalid_evidence(
                        "observed delivery evidence must not include acknowledged_relays",
                    ));
                }
                if self.observed_relays.is_empty() && self.connected_relays.is_empty() {
                    return Err(invalid_evidence(
                        "observed delivery evidence requires connected_relays or observed_relays",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn relay_set_fingerprint(&self) -> Option<String> {
        canonical_relay_set_fingerprint(&self.target_relays)
    }

    pub fn to_json_value(&self) -> Result<Value, LocalEventsError> {
        self.validate()?;
        serde_json::to_value(self).map_err(LocalEventsError::from)
    }

    pub fn from_json_value(value: &Value) -> Result<Self, LocalEventsError> {
        let evidence: Self = serde_json::from_value(value.clone())?;
        evidence.validate()?;
        Ok(evidence)
    }

    fn build<I, S, J, T, K, U, L, V>(
        state: RelayDeliveryState,
        target_relays: I,
        connected_relays: J,
        acknowledged_relays: K,
        observed_relays: L,
        failed_relays: Vec<RelayDeliveryFailure>,
    ) -> Result<Self, LocalEventsError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        J: IntoIterator<Item = T>,
        T: AsRef<str>,
        K: IntoIterator<Item = U>,
        U: AsRef<str>,
        L: IntoIterator<Item = V>,
        V: AsRef<str>,
    {
        let evidence = Self {
            state,
            target_relays: normalize_required_relay_set("target_relays", target_relays)?,
            connected_relays: normalize_relay_set("connected_relays", connected_relays)?,
            acknowledged_relays: normalize_relay_set("acknowledged_relays", acknowledged_relays)?,
            observed_relays: normalize_relay_set("observed_relays", observed_relays)?,
            failed_relays,
        };
        evidence.validate()?;
        Ok(evidence)
    }
}

fn normalize_relay_url_for_evidence(
    field: &str,
    value: impl AsRef<str>,
) -> Result<String, LocalEventsError> {
    crate::relay_url::normalize_relay_url(value.as_ref()).map_err(|error| relay_error(field, error))
}

fn normalize_required_relay_set<I, S>(
    field: &str,
    values: I,
) -> Result<Vec<String>, LocalEventsError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let relays = normalize_relay_set(field, values)?;
    if relays.is_empty() {
        return Err(invalid_evidence(format!("{field} must not be empty")));
    }
    Ok(relays)
}

fn normalize_relay_set<I, S>(field: &str, values: I) -> Result<Vec<String>, LocalEventsError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    normalize_relay_urls(values).map_err(|error| relay_error(field, error))
}

fn validate_relay_set(
    field: &str,
    relays: &[String],
    require_non_empty: bool,
) -> Result<(), LocalEventsError> {
    let normalized = normalize_relay_set(field, relays)?;
    if require_non_empty && normalized.is_empty() {
        return Err(invalid_evidence(format!("{field} must not be empty")));
    }
    if normalized != relays {
        return Err(invalid_evidence(format!(
            "{field} must be normalized and deduplicated"
        )));
    }
    Ok(())
}

fn normalize_non_empty_text(field: &str, value: &str) -> Result<String, LocalEventsError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid_evidence(format!("{field} must not be empty")));
    }
    Ok(trimmed.to_owned())
}

fn relay_error(field: &str, error: RelayUrlValidationError) -> LocalEventsError {
    invalid_evidence(format!("{field}: {error}"))
}

fn invalid_evidence(message: impl Into<String>) -> LocalEventsError {
    LocalEventsError::InvalidRecord(format!(
        "invalid relay delivery evidence: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn state_labels_and_failure_constructor_cover_public_surface() {
        for (state, value) in [
            (RelayDeliveryState::Pending, "pending"),
            (RelayDeliveryState::Acknowledged, "acknowledged"),
            (RelayDeliveryState::Failed, "failed"),
            (RelayDeliveryState::Observed, "observed"),
        ] {
            assert_eq!(state.as_str(), value);
        }

        let failure = RelayDeliveryFailure::new(" ws://relay.test ", " connection refused ")
            .expect("failure");
        assert_eq!(failure.relay_url, "ws://relay.test");
        assert_eq!(failure.error, "connection refused");
        assert_error_contains(
            RelayDeliveryFailure::new("http://relay.test", "err"),
            "failed_relays.relay_url",
        );
        assert_error_contains(RelayDeliveryFailure::new("ws://relay.test", " "), "error");
    }

    #[test]
    fn constructors_validate_all_delivery_states_and_json_roundtrips() {
        let pending = RelayDeliveryEvidence::pending(["ws://relay-a.test", "ws://relay-a.test"])
            .expect("pending evidence");
        assert_eq!(pending.state, RelayDeliveryState::Pending);
        assert_eq!(pending.target_relays, vec!["ws://relay-a.test"]);
        assert!(pending.relay_set_fingerprint().is_some());
        assert_eq!(
            RelayDeliveryEvidence::from_json_value(&pending.to_json_value().expect("pending json"))
                .expect("pending from json"),
            pending
        );

        let failure = RelayDeliveryFailure::new("ws://relay-b.test", "timeout").expect("failure");
        let acknowledged = RelayDeliveryEvidence::acknowledged(
            ["ws://relay-a.test"],
            ["ws://relay-a.test"],
            ["ws://relay-a.test"],
            vec![failure.clone()],
        )
        .expect("acknowledged");
        assert_eq!(acknowledged.state, RelayDeliveryState::Acknowledged);

        let observed = RelayDeliveryEvidence::observed(
            ["ws://relay-a.test"],
            Vec::<String>::new(),
            ["ws://relay-b.test"],
            vec![failure.clone()],
        )
        .expect("observed");
        assert_eq!(observed.state, RelayDeliveryState::Observed);

        let failed = RelayDeliveryEvidence::failed(
            ["ws://relay-a.test"],
            ["ws://relay-a.test"],
            vec![failure],
        )
        .expect("failed");
        assert_eq!(failed.state, RelayDeliveryState::Failed);
    }

    #[test]
    fn validate_rejects_invalid_manual_evidence_shapes() {
        assert_error_contains(
            RelayDeliveryEvidence::pending(Vec::<String>::new()),
            "target_relays",
        );

        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Pending,
                target_relays: vec!["ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: vec!["ws://relay.test".to_owned()],
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .validate(),
            "pending delivery evidence",
        );

        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Acknowledged,
                target_relays: vec!["ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .validate(),
            "requires acknowledged_relays",
        );

        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Acknowledged,
                target_relays: vec!["ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: vec!["ws://relay.test".to_owned()],
                observed_relays: vec!["ws://relay.test".to_owned()],
                failed_relays: Vec::new(),
            }
            .validate(),
            "must not include observed_relays",
        );

        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Failed,
                target_relays: vec!["ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .validate(),
            "failed delivery evidence",
        );

        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Observed,
                target_relays: vec!["ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: vec!["ws://relay.test".to_owned()],
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .validate(),
            "must not include acknowledged_relays",
        );

        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Observed,
                target_relays: vec!["ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .validate(),
            "requires connected_relays or observed_relays",
        );
    }

    #[test]
    fn validate_rejects_non_normalized_relays_and_failure_text() {
        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Pending,
                target_relays: vec!["ws://relay.test".to_owned(), "ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .validate(),
            "normalized and deduplicated",
        );

        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Failed,
                target_relays: vec!["ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: vec![RelayDeliveryFailure {
                    relay_url: "http://relay.test".to_owned(),
                    error: "timeout".to_owned(),
                }],
            }
            .validate(),
            "failed_relays.relay_url",
        );

        assert_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Failed,
                target_relays: vec!["ws://relay.test".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: vec![RelayDeliveryFailure {
                    relay_url: "ws://relay.test".to_owned(),
                    error: " timeout ".to_owned(),
                }],
            }
            .validate(),
            "must be trimmed",
        );

        assert_error_contains(
            RelayDeliveryEvidence::from_json_value(&json!({
                "state": "pending",
                "target_relays": [],
                "connected_relays": [],
                "acknowledged_relays": [],
                "failed_relays": []
            })),
            "target_relays",
        );
    }

    fn assert_error_contains<T: std::fmt::Debug>(
        result: Result<T, LocalEventsError>,
        expected: &str,
    ) {
        let err = result.expect_err("expected relay delivery error");
        assert!(
            err.to_string().contains(expected),
            "expected error to contain {expected}, got {err}"
        );
    }
}
