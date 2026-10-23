use radroots_event::SignedEvent;
use radroots_event::wire::{EventWireLimits, Nip01EventWire};
use radroots_storage::{
    Error, EventStore,
    event::{AdmissionReceipt, EventAdmission, EventQuery, EventQueryBounds},
};

use crate::policy::SyncStorage;

pub(super) async fn admit(
    storage: &dyn SyncStorage,
    admission: EventAdmission,
    original: &SignedEvent,
    contract_id: &str,
    observed_at_unix_ms: u64,
) -> Result<AdmissionReceipt, Error> {
    match EventStore::admit(storage, admission).await {
        Err(Error::EventConflict) => {}
        result => return result,
    }

    let query = EventQuery::for_ids(EventQueryBounds::first(1)?, vec![*original.id()])?;
    let existing = EventStore::query_raw(storage, query).await?;
    let [stored] = existing.items() else {
        return Err(Error::EventConflict);
    };
    // Wire equality includes the signature and extension fields. The canonical
    // store keeps its first raw bytes; the signing artifact keeps its own.
    if stored.event().wire() != original.wire()
        || !exact_wire(stored.event())
        || !exact_wire(original)
    {
        return Err(Error::EventConflict);
    }
    let canonical = super::outbound_admission(stored.event(), contract_id, observed_at_unix_ms)
        .map_err(|_| Error::EventConflict)?;
    EventStore::admit(storage, canonical).await
}

fn exact_wire(event: &SignedEvent) -> bool {
    let limits = EventWireLimits {
        max_raw_json_bytes: 64 * 1024,
        ..EventWireLimits::default()
    };
    Nip01EventWire::parse_json_exact_with_limits(event.raw_json(), limits)
        .is_ok_and(|wire| &wire == event.wire())
}
