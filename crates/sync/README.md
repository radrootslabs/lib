# radroots_sync

Executor-neutral local-first synchronization orchestration for Radroots.

The package owns the shared ingest, pull, projection, push, policy, and status
boundaries. It does not create an executor, spawn workers, install timers, own
process lifecycle, store UI state, or branch on concrete transport adapters.

Pull receipts retain the last available outcome for each target and bounded
cumulative target summaries across returned pages. A later complete outcome
does not erase an earlier incomplete or missing outcome. Summaries preserve
request order and use the existing 64-target and 1,000-page bounds. A legacy
receipt without summaries has unknown cumulative evidence. Even when every
page reports completion, callers must inspect pull termination and request
scope; a receipt never proves complete global history.

Publication remains disabled while behavior is implemented and qualified in
the subsequent Release V1 sync checkpoints.
