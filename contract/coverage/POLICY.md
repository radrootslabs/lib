# Radroots Core Libraries Rust Coverage Policy

This document defines the required coverage gate for the Radroots Core Libraries Rust workspace.
The authoritative machine-readable contract is `contract/coverage/policy.toml`.

## gate contract

- executable lines coverage: 100.0
- function coverage: 100.0
- region coverage: 100.0
- branch coverage: 100.0
- branch records must be present in lcov data

All four thresholds are release-blocking.

## toolchain contract

- use nightly rust for coverage runs
- use `cargo llvm-cov` with `--branch`
- generate json summary and lcov reports for each run
- evaluate coverage using deterministic parsing rules

## enforcement contract

- run coverage checks per crate, not only aggregate workspace totals
- a crate cannot be promoted to required unless it is at 100/100/100/100
- once required, the crate remains blocking on every pull request and push to `master`

## required crate contract

- every workspace crate is required
- the required blocking crate list is tracked in `contract/coverage/policy.toml`
- workspace membership changes must update `contract/coverage/policy.toml` in the same change

## local override policy

Local override env vars may exist for smoke runs, but canonical release and coverage lanes must read the strict gate from `contract/coverage/policy.toml`.

## toolchain pin

The pinned nightly used for coverage lives in `rust-toolchain-coverage.toml`.
