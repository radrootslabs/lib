# Radroots Core Libraries Rust Coverage Policy

This document defines the required coverage gate for the Radroots Core Libraries Rust workspace.
The authoritative machine-readable contract is `policy/coverage/policy.toml`.

## gate contract

- executable lines coverage: 98.0
- function coverage: 98.0
- region coverage: 98.0
- branch coverage: 98.0
- branch records must be present in lcov data unless a crate-specific policy override marks branch coverage as not applicable

All four thresholds are release-blocking for required crates. This is the
heavy-development coverage gate, not a 100% coverage requirement.

Coverage work should prioritize required behavior, protocol contracts,
conformance vectors, parsing, validation, and state-transition invariants.
Do not add low-value tests solely to chase crate-wide 100% coverage.

## toolchain contract

- use nightly rust for coverage runs
- use `cargo llvm-cov` with `--branch`
- generate json summary and lcov reports for each run
- evaluate coverage using deterministic parsing rules

## enforcement contract

- run coverage checks per crate, not only aggregate workspace totals
- a crate cannot be promoted to required unless it satisfies the active gate
- once required, the crate remains blocking on every canonical release-preflight run and any external automation that wraps that run
- `coverage-refresh.tsv` must be generated from measured per-crate gate reports, not from synthetic pass rows
- temporary threshold overrides below 98/98/98/98 are not part of the active gate
- branch-record presence overrides are allowed only for crates whose coverage report has no measured branch records; when branch records exist, the active branch threshold remains binding

## required crate contract

- every workspace crate is required except SimpleX crates
- the required blocking crate list is tracked in `policy/coverage/policy.toml`
- workspace membership changes must update `policy/coverage/policy.toml` in the same change
- crates are not expected to reach 100% coverage during heavy development

## local override policy

Local override env vars may exist for smoke runs, but canonical release and coverage lanes must read the gate from `policy/coverage/policy.toml`.

## toolchain pin

The pinned nightly used for coverage lives in `rust-toolchain-coverage.toml`.
