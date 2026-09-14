//! Instance-owned indexes into the reference backend's immutable receipts.

use super::State;
use crate::{
    Error,
    authored_atomic::{AuthoredAtomicCommand, ClaimAuthoredTarget, ReconcileDeliveryFacts},
    authored_delivery::{
        AuthoredDeliveryHistory, AuthoredDeliveryPlan, AuthoredDeliveryPlanId,
        DELIVERY_PLAN_ATTEMPTS_MAX,
    },
};

#[derive(Clone)]
pub(super) struct Entry {
    preparation: Option<usize>,
    claims: Vec<usize>,
}

pub(super) fn register(
    state: &mut State,
    command: &AuthoredAtomicCommand,
    receipt_index: usize,
) -> Result<(), Error> {
    match command {
        AuthoredAtomicCommand::Prepare(value) => {
            for plan in value.delivery_plans() {
                if state
                    .authored_delivery_history
                    .insert(
                        plan.plan_id(),
                        Entry {
                            preparation: Some(receipt_index),
                            claims: Vec::new(),
                        },
                    )
                    .is_some()
                {
                    return Err(Error::AtomicCommitConflict);
                }
            }
        }
        AuthoredAtomicCommand::Claim(value) => {
            if let ClaimAuthoredTarget::DeliveryPlan(plan_id) = value.target() {
                let entry = state
                    .authored_delivery_history
                    .entry(*plan_id)
                    .or_insert(Entry {
                        preparation: None,
                        claims: Vec::new(),
                    });
                if entry.claims.len() >= DELIVERY_PLAN_ATTEMPTS_MAX as usize {
                    return Err(Error::DeliveryAttemptOverflow);
                }
                entry.claims.push(receipt_index);
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn history(
    state: &State,
    plan_id: AuthoredDeliveryPlanId,
) -> Result<Option<AuthoredDeliveryHistory>, Error> {
    let Some(plan) = state
        .authored_delivery_plans
        .iter()
        .find(|plan| plan.plan_id() == plan_id)
    else {
        return Ok(None);
    };
    let entry = state.authored_delivery_history.get(&plan_id);
    let preparation = entry
        .and_then(|entry| entry.preparation)
        .map(|index| {
            state
                .authored_atomic_receipts
                .get(index)
                .ok_or(Error::AtomicWorkflowMismatch)
        })
        .transpose()?;
    let mut history = AuthoredDeliveryHistory::new(plan.clone(), preparation)?;
    if let Some(entry) = entry {
        if entry.claims.len() > DELIVERY_PLAN_ATTEMPTS_MAX as usize {
            history.mark_truncated();
        }
        for index in entry
            .claims
            .iter()
            .take(DELIVERY_PLAN_ATTEMPTS_MAX as usize)
        {
            history.push_claim(
                state
                    .authored_atomic_receipts
                    .get(*index)
                    .ok_or(Error::AtomicWorkflowMismatch)?,
            )?;
        }
    }
    history.validate()?;
    Ok(Some(history))
}

pub(super) fn reconcile(
    state: &mut State,
    command: &ReconcileDeliveryFacts,
) -> Result<AuthoredDeliveryPlan, Error> {
    let history = history(state, command.plan_id())?.ok_or(Error::InvalidAuthoredDeliveryPlan)?;
    let plan = command.apply_to(&history)?;
    let stored = state
        .authored_delivery_plans
        .iter_mut()
        .find(|value| value.plan_id() == command.plan_id())
        .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
    *stored = plan.clone();
    Ok(plan)
}
