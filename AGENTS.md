# Rad Roots - Code Directives

## Purpose
- The crates are a shared Rust library layer used by Radroots networking apps and libraries across web (wasm), native, daemons, and embedded systems. Prioritize portability, correctness, and low overhead.

## Scope
- Applies to the workspace in this repository.

## Workspace Architecture
- core: no_std core value types (money, currency, quantity, percent, discount, unit) with serde/typeshare gates.
- types: API wrapper types (IError, IResult, IResultList) with ts-rs support.
- events: Nostr event models (post, profile, job, tags, kinds) with ts-rs support.
- events-codec: encode/decode for events (jobs, profiles) for nostr payloads.
- events-indexed: manifest/checkpoint/types for indexed event archives (typeshare + serde gates).
- nostr: Nostr utilities (filters, tags, relays, parsing) and SDK adapters.
- log: tracing-based logging helpers with std/no_std split.
- runtime: config loading, JSON IO, tracing init, signals, CLI helpers.
- identity: identity spec + load/generate utilities, built on runtime.
- net-core: networking core, build info, config, optional tokio runtime and Nostr client.
- net: thin re-export of net-core.
- sql-core: SQL executor trait + migrations for native/web/embedded targets.
- sql-wasm-bridge: wasm JS bridge for exec/query and savepoint transactions.
- sql-wasm-core: wasm-bindgen exports + error marshaling for SQL.
- tangle-schema: Tangle schema models and relation types (ts-rs bindings).
- tangle-sql: SQL access layer for Tangle schema, migrations, backup/restore.
- tangle-sql-wasm: wasm-bindgen exports for Tangle SQL operations.
- trade: trade/listing domain models and tags.

## Rust Code Directives
- Toolchain: Rust 1.86, edition 2024; use workspace versions from the root Cargo.toml.
- Portability: preserve no_std patterns; gate std usage with cfg(feature = "std") and use alloc when needed.
- Safety: avoid unsafe; prefer safe, explicit APIs. Add #![forbid(unsafe_code)] on new crates/modules.
- Public API: keep Radroots* prefix; avoid hidden panics; return Result/Option for fallible ops; use precise error enums (thiserror where appropriate).
- Features: keep serde/typeshare/ts-rs derives behind existing feature gates and in the current style; ensure feature combinations compile (no_std, std, wasm).
- Generated outputs: treat */bindings/ts/src/types.ts as generated; do not hand-edit.
- Performance: borrow over clone, avoid intermediate allocations, preallocate when sizes are known, and prefer iterators over indexing loops.
- DRY: consolidate shared logic into core/types/events-codec or dedicated helpers.
- Parity: maintain feature parity across native/wasm layers when adding SQL or Tangle APIs.
- Module layout: keep lib.rs as a module manifest and re-export surface; avoid heavy logic in lib.rs.
- Testing: add or update unit tests for new behavior and edge cases, especially around parsing, invariants, conversions, and rounding.
