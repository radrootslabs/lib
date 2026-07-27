# Release-v1 requirement traceability

This matrix maps durable architecture requirements to implementation ownership
and verification. It adds no product requirements; the synchronized
`docs/specs/` bundle remains normative.

| Durable requirement | Owning package or control | Handoff steps | Required evidence |
| --- | --- | --- | --- |
| Exactly 19 public packages with a 17/2 repository split | release policy and architecture catalog | 013, 015-026, 304-305 | Cargo-resolved graph report and exact allowlist validation |
| Public-only identity and separated signing/secrets | `radroots_identity`, `radroots_signing`, `radroots_secrets` | 052-054, 099-111, 147-155 | public API, feature, dependency, and redaction tests |
| One canonical `TradeId` | `radroots_event`, `radroots_trade` | 073-098 | compile/API inventory and trade conformance |
| Version-neutral protocol ownership | `radroots_protocol` and private generators | 055-064, 261-268 | contract vectors and generated freshness |
| Independent transport source/sink with extensible identity | `radroots_transport` and adapters | 112-134, 190-207 | transport conformance and forward-compatibility fixtures |
| Storage SPI with SQLite backend | `radroots_storage`, `radroots_storage_sqlite` | 156-189 | backend conformance, migration, recovery, and leakage gates |
| Shared sync engine and explicit lifecycle | `radroots_sync` | 208-225 | pull/push, idempotency, cancellation, and close tests |
| Safe SDK defaults and curated facade | `radroots_sdk`, `radroots` | 226-260 | clean-project package smoke tests and compile-time surface guards |
| Preview and implementation packages remain private | release policy and graph validator | 013, 023-026, 304-305 | private-closure and forbidden-edge fixtures |
| Package-realistic reproducible release | release tooling in both repositories | 295-315 | locked zero-diff package, extracted, local-registry, target, and coverage gates |
| Every first-party consumer migrates | downstream cutover matrix | 269-294 | discovered consumer inventory and canary results |
| Deviations remain explicit and reviewable | `docs/implementation/deviations.toml` | 014 and every affected step | `cargo xtask architecture` plus step report evidence |
