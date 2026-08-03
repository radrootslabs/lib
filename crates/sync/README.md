# radroots_sync

Executor-neutral local-first synchronization orchestration for Radroots.

The package owns the shared ingest, pull, projection, push, policy, and status
boundaries. It does not create an executor, spawn workers, install timers, own
process lifecycle, store UI state, or branch on concrete transport adapters.

Publication remains disabled while behavior is implemented and qualified in
the subsequent Release V1 sync checkpoints.
