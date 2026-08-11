# radroots_service_host

`radroots_service_host` is the unpublished, native host-mechanism crate for
Radroots services. It provides narrow, reusable building blocks for validated
service identity, deterministic configuration loading, injected time and
entropy, lifecycle supervision, local administration, and cached operations
surfaces.

The crate owns mechanisms only. Service-specific configuration, policy,
database schema, domain routes, readiness decisions, process CLI parsing,
runtime creation, global logging, and signal installation remain with the
consuming service or binary boundary. Authoritative tasks may not be detached,
and this crate must not become a broad lifecycle framework.

Publication is disabled. The package is not part of the public Radroots crate
release closure.
