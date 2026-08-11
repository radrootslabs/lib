# radroots_service_sqlite

`radroots_service_sqlite` is the unpublished, native SQLite-mechanism crate for
Radroots services. It provides narrow, reusable building blocks for exclusive
writer authority, instance locking, versioned schema mechanics, bounded
transactions, immutable service-instance database identity, integrity checks,
backup, restore, and passive storage status.

`ServiceSqliteHost` is the only public connection host. Its SQLx pool and raw
connections are sealed inside the crate. Services run typed SQLx queries through
the borrowed `ServiceSqliteTransaction` executor supplied by
`ServiceSqliteHost::transaction`; transaction begin, commit, rollback, policy
validation, attached-database exclusion, and cancellation quarantine remain
runner-owned. Writable host opening finishes every pending governed migration
before returning, and read-only inspection opens only current migration and
schema state.

Cancelling a host transaction before the runner enables outer commit
quarantines its connection and leaves no authoritative transaction effect. A
service-operation error is returned only after rollback is confirmed; an
unconfirmed rollback is reported as `RollbackFailed`. Cancelling once outer
commit begins yields no result and must be treated as an unknown commit outcome.
Both that case and `CommitOutcomeUnknown` require rereading authoritative state
before an idempotent retry.

Every host must be closed explicitly with `ServiceSqliteHost::close`. Close
permanently stops new transaction admission, drains transactions that were
already admitted, and is safe to call sequentially or concurrently. Writable
close applies the fixed `PRAGMA wal_checkpoint(TRUNCATE)` policy, requires an
unblocked checkpoint, closes the private checkpoint connection, and explicitly
releases writer authority. Read-only inspection close drains its pool and
releases its shared inspection guard without checkpointing or mutating the
database or filesystem. Cancelling close before terminal completion leaves the
host non-admitting and retains authority; the private connect, checkpoint, and
explicit connection-close driver remains host-owned so a later call resumes
close without losing the SQLite handle or its close proof. Once authority
release is proven, the stable outer result is cached for every later call.
Dropping a host performs no asynchronous close work and is not proof that the
governed checkpoint and authority-release sequence completed.

The crate owns mechanics only. Service-specific tables, SQL, repositories,
backup content policy, identity material, process lifecycle, and readiness
policy remain with the consuming service. The crate does not provide callers
with raw database authority.

Publication is disabled. The package is not part of the public Radroots crate
release closure.
