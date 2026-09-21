# radroots_sync

Executor-neutral local-first synchronization orchestration for Radroots.

The package owns the shared ingest, pull, projection, push, policy, and status
boundaries. It does not create an executor, spawn workers, install timers, own
process lifecycle, store UI state, or branch on concrete transport adapters.

Ingest performs real event-ID and signature verification before host policy.
Contract failure defaults to rejection. A host can explicitly retain such a
signed observation through `AdmissionPolicy::contract_failure` for canonical
replacement evidence; its closed decision permits only rejection or verified
retention, never visibility. Invalid IDs and signatures cannot reach this policy.

Pull receipts retain the last available outcome for each target and bounded
cumulative target summaries across returned pages. A later complete outcome
does not erase an earlier incomplete or missing outcome. Summaries preserve
request order and use the existing 64-target and 1,000-page bounds. A legacy
receipt without summaries has unknown cumulative evidence. Even when every
page reports completion, callers must inspect pull termination and request
scope; a receipt never proves complete global history.

Publication remains disabled while behavior is implemented and qualified in
the subsequent Release V1 sync checkpoints.

Advanced hosts can call `PushRequest::authored_preparation` with an explicitly
captured timestamp to build the same pure preparation used by `prepare_push`.
Retain this value when composing a storage draft submission: composite replay
compares captured timestamps exactly. Building it performs no storage, signing,
clock or network operation. Ordinary preparation identity and replay semantics
remain unchanged.

Prepared signing consumes the authored-evidence signer hook, revalidates its
exact request binding, and records verified bytes against the original durable
claim even after that claim expires or is superseded. It reloads current state
after receipt replay before returning or permitting admission. Caller deadline
and cancellation outcomes are reported after retaining valid evidence; stopped
work cannot restart admission or signing. An already-signed replay does not
require a signer or credentials. The strict expiring authorization hook remains
unchanged.

The host must continue polling an in-flight call to deliver its late evidence.
Dropping the future or losing the observation clock leaves the durable attempt
unresolved; recovery follows the declared replay capability and never invents
a new preimage, event timestamp, or key. Sync creates no worker or timer to
retain a discarded future. Delivery-wide stop reconciliation is a separate
contract from authored signing evidence.

Authored delivery retains validated raw results against their original durable
claim even after stop, expiry or replacement of that claim. A late callback can
change scheduling only under its original still-current lease. Fresh calls
reconcile pending facts before any further delivery; reconciliation invokes no
transport. Every retry uses the same persisted signed request and target policy.
Stop prevents further local work and preserves unknown and accepted effects.

`PushStatus::delivery_history` exposes consistent bounded claim provenance.
Use its explicit no-issued proof and unresolved-history classification together
with the plan's cumulative delivery satisfaction and first stop. The existing
settlement counters describe scheduling state; they do not prove remote absence.
After a post-delivery clock failure, Sync retains the raw non-expiring result
with the known pre-effect time as a causal lower bound and returns
`ClockUnavailable` without retry scheduling. This is not a measured response
time and does not change strict signing or expiring authorization requirements.

`Engine::deliver_push_selected` accepts an explicit nonempty subset of the
frozen delivery targets. It validates the subset before a new claim and passes
the original full request to the selected sink boundary. Attempted evidence
outside that selection is an invalid adapter contract. Shared claim, stop,
late-result persistence, reconciliation and retry semantics remain unchanged;
the caller owns eligibility and holds an empty selection without delivery.
