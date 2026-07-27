# Commit-step report template

Complete this record in the owning rolling-commit document after verification
and before the next handoff step begins.

```text
Step:
Title:
Repository:
Branch:
Commit SHA:

Spec anchors:
- ...

Files changed:
- ...

Behavior implemented:
- ...

Tests and verification:
- command:
  result:
- command:
  result:

Self-review:
- public API review:
- architecture-boundary review:
- error/secret review:
- feature/target review:
- documentation review:
- generated/lockfile diff review:

Deviations:
- none
or
- RCRV1-DEV-NNN and evidence

Unresolved issues:
- none
or
- ...

Known pre-existing failures:
- none
or
- command, exact failure, evidence, and why it is outside this step

Next-step safety:
- SAFE / BLOCKED
- reason:
```

A step is not complete without its commit SHA, exact command outcomes,
self-review, deviation disposition, and next-step safety decision. A blocked
step does not authorize later work.
