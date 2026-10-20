use super::{
    Selection, Slot,
    codec::{self, Manifest},
    context, routes,
};
use crate::workspace::{
    State, address, crypto,
    installation::{Records, work_commitment},
    retained,
};
use crate::{Mutation, ReadSnapshot, RecordKey};
use radroots_domain::OperationId;
use radroots_storage::command::{
    DeliveryAttempt, DeliveryAttemptSummary, DeliveryObservation, OperationError,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroizing;

pub(in crate::workspace) struct Graph {
    pub summaries: Vec<DeliveryAttemptSummary>,
    pub selected: Option<DeliveryAttempt>,
    pub(super) selected_slot: Option<Slot>,
    pub observations: Vec<DeliveryObservation>,
    pub slots: [u16; 2],
}
impl Graph {
    pub(in crate::workspace) async fn read(
        state: &State,
        view: &ReadSnapshot,
        original: &retained::Graph,
        installed: &Records,
        operation: OperationId,
        selected: Option<Selection<'_>>,
    ) -> Result<Self, OperationError> {
        let mut positions = BTreeMap::<(u8, u16), BTreeSet<(u8, u8)>>::new();
        let mut observations = BTreeMap::<(u8, u16), BTreeMap<(u8, u8), RecordKey>>::new();
        let mut observation_count = 0;
        for key in view.keys() {
            let (kind, _, _) = address::fields(key)?;
            if kind == address::OBSERVATION {
                let (recipient, ordinal, target, kind) = address::observation_position(key)?;
                observation_count += 1;
                if observation_count > 2304 {
                    return Err(OperationError::RecoveryRequired);
                }
                observations
                    .entry((recipient, ordinal))
                    .or_default()
                    .insert((target, kind), key);
                continue;
            }
            if !matches!(kind, address::DELIVERY | address::REQUEST_INDEX) {
                continue;
            }
            let (kind, recipient, ordinal, part) = address::delivery_position(key)?;
            positions
                .entry((recipient, ordinal))
                .or_default()
                .insert((kind, part));
        }
        if positions.len() > 384 {
            return Err(OperationError::RecoveryRequired);
        }
        let mut result = Self {
            summaries: Vec::with_capacity(positions.len()),
            selected: None,
            selected_slot: None,
            observations: Vec::with_capacity(observation_count),
            slots: [0; 2],
        };
        let mut representations = BTreeMap::new();
        for ((recipient, ordinal), positions) in positions {
            if ordinal != result.slots[recipient as usize] + 1 {
                return Err(OperationError::RecoveryRequired);
            }
            result.slots[recipient as usize] = ordinal;
            let bound = original.bound().ok_or(OperationError::RecoveryRequired)?;
            let participant = bound
                .chat_plan()?
                .seals()
                .get(recipient as usize)
                .ok_or(OperationError::RecoveryRequired)?
                .recipient();
            let work = installed
                .work
                .iter()
                .find(|work| work.evidence().recipient() == participant)
                .copied()
                .ok_or(OperationError::RecoveryRequired)?;
            let commitment = work_commitment(
                installed.message.ok_or(OperationError::RecoveryRequired)?,
                work,
            );
            let slot = Slot {
                operation,
                recipient,
                ordinal,
                commitment,
            };
            let manifest = read(state, view, slot, 0, false).await?;
            let parsed = Manifest::decode(&manifest, commitment)?;
            let count = parsed.bytes.div_ceil(codec::PART_BYTES) as u8;
            if positions.len() != count as usize + 2
                || !positions.contains(&(address::REQUEST_INDEX, 0))
                || (0..=count).any(|part| !positions.contains(&(address::DELIVERY, part)))
            {
                return Err(OperationError::RecoveryRequired);
            }
            let index = read(state, view, slot, 0, true).await?;
            if index.as_slice() != codec::index(&manifest, &parsed.request).as_slice() {
                return Err(OperationError::RecoveryRequired);
            }
            let mut frame = Zeroizing::new(Vec::with_capacity(parsed.bytes));
            for part in 1..=count {
                let clear = read(state, view, slot, part, false).await?;
                if clear.len() != (parsed.bytes - frame.len()).min(codec::PART_BYTES) {
                    return Err(OperationError::RecoveryRequired);
                }
                frame.extend_from_slice(&clear);
            }
            if Sha256::digest(frame.as_slice()).as_slice() != parsed.digest {
                return Err(OperationError::RecoveryRequired);
            }
            let report = codec::decode(&frame, work, commitment)?;
            if !parsed.matches(&report) {
                return Err(OperationError::RecoveryRequired);
            }
            result.observations.extend(
                super::observation::read(
                    state,
                    view,
                    slot,
                    &report,
                    observations
                        .remove(&(recipient, ordinal))
                        .unwrap_or_default(),
                )
                .await?,
            );
            routes(bound, participant, report.intent())
                .map_err(|_| OperationError::RecoveryRequired)?;
            if representations
                .insert(report.intent().event_id(), *report.envelope_digest())
                .is_some_and(|digest| digest != *report.envelope_digest())
            {
                return Err(OperationError::RecoveryRequired);
            }
            result
                .summaries
                .push(DeliveryAttemptSummary::from_attempt(&report));
            if let Some(selected) = selected.filter(|selected| selected.recipient == participant) {
                let exact = selected.request == report.intent().request_id();
                if exact
                    || (result.selected.is_none()
                        && selected.event == Some(report.intent().event_id()))
                {
                    result.selected = Some(report);
                    result.selected_slot = Some(slot);
                }
            }
        }
        if !observations.is_empty() {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(result)
    }
}
async fn read(
    state: &State,
    view: &ReadSnapshot,
    slot: Slot,
    part: u8,
    index: bool,
) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    let kind = if index {
        address::REQUEST_INDEX
    } else {
        address::DELIVERY
    };
    let raw = state
        .read_view_key(
            view,
            address::delivery_key(slot.operation, kind, slot.recipient, slot.ordinal, part),
        )
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto::open_bound(
        state.provider.as_ref(),
        &state.reference,
        context(state, slot, part, index)?,
        &raw,
    )
    .await?;
    Ok(clear.expose_secret(|raw| Zeroizing::new(raw.to_vec())))
}
pub(super) async fn protect(
    state: &State,
    operation: OperationId,
    recipient: u8,
    ordinal: u16,
    commitment: [u8; 32],
    report: &DeliveryAttempt,
) -> Result<Vec<Mutation>, OperationError> {
    let frame = codec::encode(report, commitment)?;
    let manifest = Manifest::encode(report, commitment, &frame);
    let slot = Slot {
        operation,
        recipient,
        ordinal,
        commitment,
    };
    let mut changes = Vec::with_capacity(frame.len().div_ceil(codec::PART_BYTES) + 2);
    changes.push(seal(state, slot, 0, false, &manifest).await?);
    changes.push(
        seal(
            state,
            slot,
            0,
            true,
            &codec::index(&manifest, report.intent().request_id().as_str()),
        )
        .await?,
    );
    for (part, raw) in frame.chunks(codec::PART_BYTES).enumerate() {
        changes.push(seal(state, slot, part as u8 + 1, false, raw).await?);
    }
    Ok(changes)
}
async fn seal(
    state: &State,
    slot: Slot,
    part: u8,
    index: bool,
    raw: &[u8],
) -> Result<Mutation, OperationError> {
    let protected = crypto::seal_bound(
        state.provider.as_ref(),
        &state.reference,
        context(state, slot, part, index)?,
        raw,
    )
    .await?;
    Mutation::new(
        address::delivery_key(
            slot.operation,
            if index {
                address::REQUEST_INDEX
            } else {
                address::DELIVERY
            },
            slot.recipient,
            slot.ordinal,
            part,
        ),
        None,
        Some(protected),
    )
    .map_err(super::super::journal::storage_error)
}
