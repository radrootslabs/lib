# Contributing

Radroots core-library changes are contract-driven and independently
reviewable. Before editing, read these files in order:

1. `AGENTS.md`
2. `AGENT_INSTRUCTIONS.md`
3. `contracts/crates/release.v2.toml`
4. `contracts/crates/release_v1/radroots_crates_release_v1.toml` for
   crate-surface work
5. the affected manifests, implementation, contracts, and tests

Release V2 records the nineteen current public Lib packages, including
`radroots_sdk` and `radroots`. The earlier `radroots.crates.release.v1`
repository split remains immutable history. The approved successor is
`contracts/architecture/decisions/shared_sdk_refactor.v1.json`: Lib owns shared
semantics and execution, `radroots` remains the facade, and the SDK repository
consumes source-owned generators and artifacts. Its target package graph is
prospective; implementation, compiler migration and qualification require
their actual source-bound checks before acceptance.

## Source and behavior provenance

Reusable application behavior is independently reimplemented in Lib under
the shared SDK decision. Referencing Tera's GPL application behavior does not
authorize copying or translating its implementation, test bodies, fixtures or
generated derivatives into differently licensed packages. Organizational
ownership and a Cargo license field do not establish a separate rights grant.
Preserve the original source and notices; retain each meaningful test purpose
through independently authored successor cases before removing its old owner.

For an affected implementation, record the exact source repository, revision,
path, declared license, retained behavior and successor test. Preserve imported
catalog provenance and existing third-party notices. New repository-native
packages use the catalog's native introduction-tree mechanism. Neither a
catalog entry nor a rewritten implementation claims rights to relicense its
references.

Retained optional capabilities, including mesh, Reticulum, SimpleX, NostrDB
and replica support, keep their source and proving obligations. Before exposing
them in a supported artifact, review the actual target and feature dependency
closure, upstream material and redistribution notices. Resolve incompatible or
unverified reuse at its owner; do not discard the approved capability or infer
dependency licensing from the workspace license. Generate artifact notices
from their source-owned inputs and bind them to the actual shipped bytes.

## Workflow

1. Inspect repository status and the current source authority.
2. Make one coherent, commit-sized change.
3. Update public contracts, tests, fixtures, generated authorities, and docs
   with the implementation they govern.
4. Run the narrowest repository-owned validation that proves the change, then
   the broader contract or release lane required by its scope.
5. Review the staged diff for API leakage, private dependencies, generated
   drift, secrets, hidden side effects, and unrelated changes.

Run `cargo extbuild doctor` before governed verification. Canonical
repository-wide lanes are the extbuild-routed workspace format, check, test,
Clippy, Rustdoc, contract, release-preflight, coverage, and
generated-freshness commands. Nix evaluation and Nix-derived outputs are
currently deferred and unclaimed; they are not prerequisites for native
qualification.

## Commits and deviations

Use this commit form:

```text
<scope>: <lower-case imperative summary>
```

Keep commits focused and keep public commit language independent of any
private checkout. Do not publish, tag, merge, or change registry ownership
without explicit authorization.

When current evidence proves a planned step obsolete or unsafe, update
`contracts/architecture/deviations.toml` and validate it with
`cargo xtask architecture`. Record the evidence and affected machine-contract
anchor before changing the plan. Anchors must use a validated Release V1 TOML
selector (`repositories.<name>`, `repository_policy`, `release_policy`,
`quality_policy.coverage`, or `package.<name>`); Markdown heading fragments are
not machine anchors. A normative change also requires the applicable machine
decision. Do not silently redefine the architecture.
