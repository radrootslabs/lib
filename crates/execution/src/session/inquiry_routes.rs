//! Shared authenticated route selection; hosts supply only their explicit policy.
use alloc::{borrow::ToOwned, vec::Vec};
use radroots_domain::PublicKey;
use radroots_protocol::nip59::CapturedDmRoute;
use radroots_storage::command::{OperationError, SignedDmRelaySource};
use radroots_transport::TargetSet;

/// Selects the NIP-01 replaceable head from at most two independently verified
/// kind-10050 sources. Retains its original signed bytes and only advertised
/// endpoints allowed by the explicit write policy. Absence grants no fallback.
pub fn capture_dm_route<'a>(
    recipient: PublicKey,
    raw_sources: impl IntoIterator<Item = &'a str>,
    write: &TargetSet,
) -> Result<CapturedDmRoute, OperationError> {
    let mut head: Option<SignedDmRelaySource> = None;
    for (index, raw) in raw_sources.into_iter().enumerate() {
        if index >= 2 {
            return Err(OperationError::InvalidEvidence);
        }
        let source = SignedDmRelaySource::verify(raw)?;
        if source.participant() != recipient {
            return Err(OperationError::InvalidEvidence);
        }
        if head.as_ref().is_none_or(|old| {
            source.created_at() > old.created_at()
                || (source.created_at() == old.created_at() && source.event_id() < old.event_id())
        }) {
            head = Some(source);
        }
    }
    let head = head.ok_or(OperationError::MissingHistory)?;
    let mut selected = Vec::new();
    for uri in head.relay_uris() {
        if write
            .targets()
            .iter()
            .any(|target| target.matches_nostr_relay(uri).unwrap_or(false))
            && !selected.iter().any(|old| old == uri)
        {
            selected.push(uri.to_owned());
        }
    }
    CapturedDmRoute::from_observed_descriptor(
        recipient,
        head.event_id(),
        head.raw_json(),
        &selected,
    )
    .map_err(|_| OperationError::UnverifiedApproval)
}

/// Reauthenticates the original captured route against current explicit write
/// policy. Canonically equal targets are dispatched once; no new route is added.
pub fn selected_dm_targets(
    route: &CapturedDmRoute,
    write: &TargetSet,
) -> Result<TargetSet, OperationError> {
    SignedDmRelaySource::verify(route.raw_json())?.validate_capture(route)?;
    let mut selected = Vec::new();
    for uri in route.selected_relays() {
        let target = write
            .targets()
            .iter()
            .find(|target| target.matches_nostr_relay(uri).unwrap_or(false))
            .ok_or(OperationError::UnverifiedApproval)?;
        if !selected.iter().any(|old| old == target) {
            selected.push(target.clone());
        }
    }
    TargetSet::new(selected).map_err(|_| OperationError::UnverifiedApproval)
}
