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
