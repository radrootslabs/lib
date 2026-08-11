# radroots_service_sqlite

`radroots_service_sqlite` is the unpublished, native SQLite-mechanism crate for
Radroots services. It provides narrow, reusable building blocks for exclusive
writer authority, instance locking, versioned schema mechanics, bounded
transactions, integrity checks, backup, restore, and passive storage status.

The crate owns mechanics only. Service-specific tables, SQL, repositories,
backup content policy, identity material, process lifecycle, and readiness
policy remain with the consuming service. Its connection pool stays private;
the crate does not provide callers with raw database authority.

Publication is disabled. The package is not part of the public Radroots crate
release closure.
