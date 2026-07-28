# Implementation deviations

The machine-readable authority is [`deviations.toml`](deviations.toml).
Repository checks validate it on every architecture and full check lane. This
ledger records evidence-based changes to implementation planning; it does not
silently change `radroots.crates.release.v1`.

## Active records

| ID | Affected steps | Approved disposition |
| --- | --- | --- |
| `RCRV1-DEV-001` | 015-023 | Preserve the existing standalone `lib` and `sdk` repositories; replace repository import/unification with independent qualification. |
| `RCRV1-DEV-002` | 249 | Pull only the facade scaffold forward to immediately after Step 014 in `sdk`; do not repeat it later. |
| `RCRV1-DEV-004` | 098, 155, 225, 260, 268, 294, 298-299, 301-304, 314 | Enforce a temporary 90% four-dimension coverage baseline during heavy development; restore 100% only through a future explicit contract update. |
| `RCRV1-DEV-005` | 013, 019-026, 027-315 | Pin every Rust crate and internal Radroots dependency in `radrootslabs/lib` to exactly `0.1.0-alpha` until further explicit authority. |

## Record template

Add one `[[deviation]]` table to `deviations.toml`:

```toml
[[deviation]]
id = "RCRV1-DEV-NNN"
date = "YYYY-MM-DD"
status = "active" # active | closed | superseded
approval = "Explicit approving decision."
affected_steps = ["NNN"]
spec_anchors = ["docs/specs/<durable-spec>#<anchor>"]
source_evidence = ["Committed source evidence."]
replacement_action = "Smallest safe disposition."
verification = ["Command or review evidence."]
unresolved_risk = "none, or a concrete bounded risk"
normative_architecture_change = false
adr_required = false
closure_evidence = [] # omit while active; required when closed or superseded
```

Every field is mandatory except `closure_evidence` on active records. Spec
anchors must resolve inside `docs/specs/`; affected steps must be three-digit
IDs in 001-315. A normative architecture change needs explicit approval and
the appropriate ADR decision before the record can be accepted.

Do not silently skip, merge, reorder, or broaden implementation steps. Keep a
red checkpoint uncommitted and mark the next step blocked until its evidence or
approval is complete.
