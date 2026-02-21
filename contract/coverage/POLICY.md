# radroots oss rust coverage policy

This document defines the required coverage gate for the oss rust workspace.

## gate contract

- executable lines coverage: 100.0
- function coverage: 100.0
- branch coverage: 100.0
- branch records must be present in lcov data

All three thresholds are release-blocking.

## toolchain contract

- use nightly rust for coverage runs
- use `cargo llvm-cov` with `--branch`
- generate json summary and lcov reports for each run
- evaluate coverage using deterministic parsing rules

## enforcement contract

- run coverage checks per crate, not only aggregate workspace totals
- a crate cannot be promoted to required unless it is at 100/100/100
- once required, the crate remains blocking on every pull request and push to `master`

## rollout contract

- start with `radroots-core` as the first required crate
- expand required coverage crate-by-crate
- full workspace required coverage is only enabled after every required crate is green

## local override policy

Local override env vars may exist for smoke runs, but ci must run with default strict thresholds and required branch data.
