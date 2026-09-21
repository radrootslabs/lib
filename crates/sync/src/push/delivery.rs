//! Delivery observations outlive the lease that admitted their external effect.

use super::*;
use radroots_storage::authored_atomic::{ReconcileDeliveryFacts, RecordDeliveryFact};

impl Engine {
    /// Reconciles retained evidence or executes one bounded delivery attempt.
    ///
    /// A late result retains its original raw binding without authorizing a
    /// stale worker to change scheduling. Hosts must keep polling this future
    /// to deliver late results; dropping it leaves durable unresolved work.
    pub async fn deliver_push(
        &self,
        operation_id: SyncId,
    ) -> Result<DeliveryExecutionReceipt, Error> {
        self.deliver_push_inner(operation_id, None).await
    }

    /// Attempts only an exact nonempty subset of the frozen delivery targets.
    /// The full persisted request, claim and raw result bindings remain intact.
    /// Callers own selection policy; no ineligible target may be attempted.
    pub async fn deliver_push_selected(
        &self,
        operation_id: SyncId,
        selected: radroots_transport::TargetSet,
    ) -> Result<DeliveryExecutionReceipt, Error> {
        self.deliver_push_inner(operation_id, Some(selected)).await
    }

    async fn deliver_push_inner(
        &self,
        operation_id: SyncId,
        selected: Option<radroots_transport::TargetSet>,
    ) -> Result<DeliveryExecutionReceipt, Error> {
        let status = self.push_status(operation_id).await?.ok_or_else(|| {
            if self.sink.is_none() {
                Error::MissingSink
            } else {
                Error::StorageFailed
            }
        })?;
        let plan = status.delivery_plan();
        if plan.state().is_terminal() {
            return Ok(DeliveryExecutionReceipt {
                plan: plan.clone(),
                replay: true,
            });
        }
        if status.artifact.signing_state() != SigningState::Signed {
            return Err(Error::InvalidSignerOutput);
        }
        if !status.artifact.admission_state().is_admitted() {
            return Err(Error::AdmissionFailed);
        }
        let now = self.delivery_now()?.max(plan.updated_at_unix_ms());
        // This invocation is fresh scheduling authority, not a late callback.
        // Reconciliation is a complete bounded action; never also send a retry.
        if plan.pending_delivery_facts().next().is_some() {
            let plan = self
                .reconcile_delivery(status.delivery_history(), None, now)
                .await?;
            return Ok(DeliveryExecutionReceipt { plan, replay: true });
        }
        if plan
            .claim_evidence()
            .is_some_and(|claim| now < claim.expires_at_unix_ms())
        {
            return Err(Error::WorkClaimConflict);
        }
        if plan
            .retry()
            .is_some_and(|retry| now < retry.not_before_unix_ms())
        {
            return Err(Error::DeliveryDeferred);
        }
        let sink = self.sink.as_deref().ok_or(Error::MissingSink)?;
        let request = plan.request().cloned().ok_or(Error::InvalidSignerOutput)?;
        if let Some(selected) = &selected {
            request
                .validate_target_selection(selected)
                .map_err(|_| Error::InvalidDeliveryRequest)?;
        }
        let claimed = self.claim_delivery_plan(plan, now).await?;
        let claim = claimed
            .claim_evidence()
            .cloned()
            .ok_or(Error::StorageFailed)?;
        // Re-read after claim admission: an observed stop or newer worker must
        // prevent this callback from initiating an effect.
        let current = self.delivery_history(claimed.plan_id()).await?;
        if current.plan().state().is_terminal() {
            return Ok(DeliveryExecutionReceipt {
                plan: current.plan().clone(),
                replay: true,
            });
        }
        let execution_started_at = self
            .delivery_now()?
            .max(current.plan().updated_at_unix_ms());
        if current.plan().claim_evidence() != Some(&claim)
            || execution_started_at >= claim.expires_at_unix_ms()
        {
            return Err(Error::WorkClaimConflict);
        }
        if current.plan().pending_delivery_facts().next().is_some() {
            let fence = WorkFence::new(*claim.token(), claim.generation(), claim.row_revision())
                .map_err(map_storage_error)?;
            let plan = self
                .reconcile_delivery(&current, Some(fence), execution_started_at)
                .await?;
            return Ok(DeliveryExecutionReceipt { plan, replay: true });
        }
        let outcome = if execution_started_at >= request.deadline_unix_ms() {
            DeliveryAttemptOutcome::SinkFailure(
                SinkFailure::for_request(
                    &request,
                    "delivery_deadline_exceeded",
                    Retryability::Terminal,
                    None,
                    None,
                    Vec::new(),
                )
                .map_err(|_| Error::InvalidDeliveryRequest)?,
            )
        } else {
            let result = match selected.clone() {
                Some(targets) => sink.deliver_selected(request.clone(), targets).await,
                None => sink.deliver(request.clone()).await,
            };
            let allowed = |rows: &[radroots_transport::sink::DeliveryTargetReceipt]| {
                selected.as_ref().is_none_or(|targets| {
                    rows.iter()
                        .all(|row| !row.was_attempted() || targets.targets().contains(row.target()))
                })
            };
            match result {
                Ok(receipt)
                    if receipt.validate_for_request(&request).is_ok()
                        && allowed(receipt.target_receipts()) =>
                {
                    DeliveryAttemptOutcome::Receipt(receipt)
                }
                Err(failure)
                    if failure.validate_for_request(&request).is_ok()
                        && allowed(failure.partial_evidence()) =>
                {
                    DeliveryAttemptOutcome::SinkFailure(failure)
                }
                Ok(_) | Err(_) => {
                    DeliveryAttemptOutcome::SinkFailure(SinkFailure::invalid_contract(&request))
                }
            }
        };
        let observed = self.delivery_now().map(|at| at.max(execution_started_at));
        // A known pre-effect lower bound suffices for the non-expiring delivery
        // fact. It does not claim a measured response time or grant retry rights.
        // Strict signing/expiring authorization observation rules are separate.
        let command = AuthoredAtomicCommand::RecordDelivery(
            RecordDeliveryFact::new(
                claimed.plan_id(),
                claimed.artifact_id(),
                claim.clone(),
                outcome.clone(),
                observed.unwrap_or(execution_started_at),
            )
            .map_err(map_storage_error)?,
        );
        let receipt = self
            .storage
            .execute_authored(command.clone())
            .await
            .map_err(map_storage_error)?;
        if !receipt.matches_command(&command) {
            return Err(Error::StorageFailed);
        }
        let current = self.delivery_history(claimed.plan_id()).await?;
        if !current
            .plan()
            .delivery_facts()
            .iter()
            .any(|fact| fact.claim() == &claim && fact.outcome() == &outcome)
        {
            return Err(Error::StorageFailed);
        }
        let at = observed?;
        let plan = if !current.plan().state().is_terminal()
            && current.plan().claim_evidence() == Some(&claim)
            && at < claim.expires_at_unix_ms()
        {
            let fence = WorkFence::new(*claim.token(), claim.generation(), claim.row_revision())
                .map_err(map_storage_error)?;
            self.reconcile_delivery(&current, Some(fence), at).await?
        } else {
            // Acceptance remains visible even when scheduling was stopped,
            // expired or superseded. Only a fresh caller may reconcile later.
            current.plan().clone()
        };
        Ok(DeliveryExecutionReceipt {
            plan,
            replay: false,
        })
    }

    fn delivery_now(&self) -> Result<u64, Error> {
        match self.clock.now_unix_ms()? {
            0 => Err(Error::ClockUnavailable),
            at => Ok(at),
        }
    }

    async fn delivery_history(
        &self,
        id: AuthoredDeliveryPlanId,
    ) -> Result<AuthoredDeliveryHistory, Error> {
        let history = self
            .storage
            .authored_delivery_history(id)
            .await
            .map_err(map_storage_error)?
            .ok_or(Error::StorageFailed)?;
        history.validate().map_err(map_storage_error)?;
        if history.plan().plan_id() != id {
            return Err(Error::StorageFailed);
        }
        Ok(history)
    }

    async fn reconcile_delivery(
        &self,
        history: &AuthoredDeliveryHistory,
        fence: Option<WorkFence>,
        at: u64,
    ) -> Result<AuthoredDeliveryPlan, Error> {
        let plan = history.plan();
        let at = at.max(plan.updated_at_unix_ms());
        // Never advance host time to a future fact to bypass a competing lease.
        if plan
            .delivery_facts()
            .iter()
            .any(|fact| fact.observed_at_unix_ms() > at)
        {
            return Err(Error::ClockUnavailable);
        }
        let retry = reconciliation_retry(history, at)?;
        let reconcile =
            ReconcileDeliveryFacts::new(plan, fence, retry, at).map_err(map_storage_error)?;
        reconcile.apply_to(history).map_err(map_claim_error)?;
        let command = AuthoredAtomicCommand::ReconcileDelivery(reconcile);
        let receipt = self
            .storage
            .execute_authored(command.clone())
            .await
            .map_err(map_claim_error)?;
        if !receipt.matches_command(&command) {
            return Err(Error::StorageFailed);
        }
        // A receipt replay is historical; expose current stop/facts/authority.
        Ok(self.delivery_history(plan.plan_id()).await?.plan().clone())
    }

    async fn claim_delivery_plan(
        &self,
        plan: &AuthoredDeliveryPlan,
        acquired_at: u64,
    ) -> Result<AuthoredDeliveryPlan, Error> {
        let generation = plan
            .claim_evidence()
            .map_or(1, |claim| claim.generation().get().saturating_add(1));
        let expires_at = acquired_at
            .checked_add(self.deadlines.timeout_ms(OperationKind::Deliver))
            .ok_or(Error::DeadlineOverflow)?;
        let claim = WorkClaim::new(
            *self.ids.next_id(OperationKind::Deliver)?.as_bytes(),
            DELIVERY_CLAIM_OWNER,
            NonZeroU64::new(generation).ok_or(Error::StorageFailed)?,
            acquired_at,
            expires_at,
            plan.revision(),
        )
        .map_err(map_storage_error)?;
        let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(plan.plan_id()),
            claim.clone(),
        ));
        let receipt = self
            .storage
            .execute_authored(command.clone())
            .await
            .map_err(map_claim_error)?;
        if !receipt.matches_command(&command) {
            return Err(Error::StorageFailed);
        }
        let AuthoredAtomicOutcome::DeliveryPlan(claimed) = receipt.outcome() else {
            return Err(Error::StorageFailed);
        };
        if claimed.validate().is_err()
            || claimed.plan_id() != plan.plan_id()
            || claimed.artifact_id() != plan.artifact_id()
            || claimed.request() != plan.request()
            || claimed.created_at_unix_ms() != plan.created_at_unix_ms()
            || claimed.claim_evidence() != Some(&claim)
            || plan.revision().get().checked_add(1) != Some(claimed.revision().get())
            || claimed.updated_at_unix_ms() != acquired_at
            || receipt.committed_at_unix_ms() != acquired_at
            || claimed.attempts() != plan.attempts()
            || claimed.state() != plan.state()
            || claimed.retry() != plan.retry()
        {
            return Err(Error::StorageFailed);
        }
        Ok(claimed.clone())
    }
}

fn reconciliation_retry(
    history: &AuthoredDeliveryHistory,
    at: u64,
) -> Result<Option<RetrySchedule>, Error> {
    history
        .require_pending_fact_provenance()
        .map_err(map_storage_error)?;
    let plan = history.plan();
    let mut count = plan.attempt_count();
    let mut matched_legacy = Vec::new();
    let mut last = plan.attempts().last().map(|attempt| attempt.outcome());
    for fact in plan.pending_delivery_facts() {
        let original = history
            .claims()
            .iter()
            .find(|entry| entry.claim() == fact.claim())
            .ok_or(Error::StorageFailed)?;
        let legacy = plan.attempts().iter().find(|attempt| {
            !matched_legacy.contains(&attempt.attempt())
                && attempt.claim_evidence().is_none()
                && original.prior_attempt_count().checked_add(1) == Some(attempt.attempt().get())
                && attempt.recorded_at_unix_ms() >= fact.claim().acquired_at_unix_ms()
                && attempt.recorded_at_unix_ms() < fact.claim().expires_at_unix_ms()
        });
        if let Some(legacy) = legacy {
            if legacy.outcome() != fact.outcome() {
                return Err(Error::StorageConflict);
            }
            matched_legacy.push(legacy.attempt());
        } else {
            count = count.checked_add(1).ok_or(Error::StorageFailed)?;
            last = Some(fact.outcome());
        }
    }
    if count > DELIVERY_PLAN_ATTEMPTS_MAX {
        return Err(Error::StorageFailed);
    }
    let satisfaction = plan.delivery_satisfaction().map_err(map_storage_error)?;
    if satisfaction != SatisfactionState::Pending || count == DELIVERY_PLAN_ATTEMPTS_MAX {
        return Ok(None);
    }
    let last = last.ok_or(Error::StorageFailed)?;
    let normalized;
    let last = if let DeliveryAttemptOutcome::SinkFailure(failure) = last {
        normalized = DeliveryAttemptOutcome::SinkFailure(normalize_sink_failure(
            plan.request().ok_or(Error::InvalidDeliveryRequest)?,
            failure.clone(),
            at,
        )?);
        &normalized
    } else {
        last
    };
    delivery_retry_schedule(
        NonZeroU32::new(count).ok_or(Error::StorageFailed)?,
        last,
        satisfaction,
        at,
    )
}
