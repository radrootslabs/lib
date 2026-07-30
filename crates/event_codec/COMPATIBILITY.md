# Temporary compatibility quarantine

`radroots_event_codec` is one approved Release V1 package and remains
`publish = false` during migration. It does not create or preserve a second
codec package identity.

The only canonical Release V1 modules are `admission`, `canonical`, `decode`,
`encode`, `manifest`, and `verify`. The explicitly documentation-hidden domain
modules in `src/lib.rs` are source-compatible routes to the same
implementations. They are not a supported public API and must not appear in
the reviewed API baseline.

The 2026-07-30 all-first-party source search found callers awaiting migration
in these bounded locations:

- the remaining `oss/lib` trade, net, event-store, test, and contract tooling;
- the standalone app runtime, CLI, RHI, Studio, and event-indexing capsules;
- `enterprise/capabilities/auth`;
- `testing/integration` and `testing/support/rs/canonical_fixtures`.

Those consumers migrate during the package and downstream cutovers, including
Steps 288-294. Step 313 is the exact final-removal checkpoint for every module
in this quarantine. That step must repeat the all-first-party search, remove
the compatibility declarations and this file, regenerate the API baseline,
and pass the complete downstream matrix before Release V1 approval.
