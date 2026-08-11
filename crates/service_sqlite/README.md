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

The crate owns mechanics only. Service-specific tables, SQL, repositories,
backup content policy, identity material, process lifecycle, and readiness
policy remain with the consuming service. The crate does not provide callers
with raw database authority.

Publication is disabled. The package is not part of the public Radroots crate
release closure.
