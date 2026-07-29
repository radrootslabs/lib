use super::reconciliation_v1::RadrootsEventIngest;
use crate::RadrootsEventStoreError;
use radroots_event::draft::SignedEvent;
use radroots_event::wire::v1::Nip01EventWire;
use radroots_event_codec::verification::v1::verify_nip01_event_v1;

impl RadrootsEventIngest {
    pub(crate) fn from_signed_event_reconciliation_v1(
        signed_event: SignedEvent,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        ensure_valid_ingest_timestamp(observed_at_ms)?;
        let verified_event = verify_nip01_event_v1(signed_event.envelope().clone())?;
        Ok(Self {
            verified_event,
            raw_json: signed_event.raw_json().to_owned(),
            observed_at_ms,
            transport_observation: None,
        })
    }

    pub(crate) fn from_raw_json_reconciliation_v1(
        raw_json: impl Into<String>,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        ensure_valid_ingest_timestamp(observed_at_ms)?;
        let raw_json = raw_json.into();
        let wire = Nip01EventWire::parse_json(raw_json.as_str())?;
        let verified_event = verify_nip01_event_v1(wire.into_envelope()?)?;
        Ok(Self {
            verified_event,
            raw_json,
            observed_at_ms,
            transport_observation: None,
        })
    }
}

fn ensure_valid_ingest_timestamp(observed_at_ms: i64) -> Result<(), RadrootsEventStoreError> {
    if observed_at_ms < 0 {
        return Err(RadrootsEventStoreError::InvalidEventIngestTimestamp {
            value: observed_at_ms,
        });
    }
    Ok(())
}
