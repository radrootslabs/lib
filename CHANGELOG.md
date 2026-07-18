# Changelog

All notable changes to the Radroots core libraries are documented in this file.

## [1.0.0-alpha.1]

This alpha is not published until the repository release preflight and external
publish policy both pass for the same source revision.

### Changed

- Calendar authoring and admission now use explicit NIP-52 authored, parsed, and
  admitted states for date events, time events, calendars, and RSVPs. Kind
  `31922` no longer emits uppercase `D`; kind `31923` derives integer UTC-day
  `D` values from its validated time range.
- Authored profile and calendar media now share the byte-verified Blossom image
  proof type; unverified URLs remain inbound data and cannot enter authoring APIs.
- Workspace packages declare one governed version explicitly so mounted path
  consumers preserve it, and every internal root dependency requires that exact
  pre-release version.
- Conformance suites now identify the `1.0.0` event-contract generation.
- Release metadata records exact governed impacts for removed public types,
  fields, functions, modules, constants, Cargo features, and trait
  implementations, plus changed field types, constant values, and algorithms.

### Removed

- Legacy calendar event models and permissive calendar tag-builder authoring
  paths were removed.
- Direct `RadrootsProfile` draft encoding and the identity profile publisher were
  removed in favor of the validated authored-profile boundary.
- The `radroots_identity/profile` Cargo feature was removed with its embedded
  legacy Profile projection.
- Replica sync no longer synthesizes Profile events from lossy stored
  projections. Its transfer protocol is now version `2`, and request JSON
  containing the removed `include_profiles` option is rejected.
- `RadrootsNostrClient` no longer implicitly dereferences to the upstream SDK
  client. Narrow client operations and the explicit ownership bridge remain.

### Compatibility

- Identity JSON containing the removed embedded `profile` projection is now
  rejected, including the nested public-profile form, instead of being loaded
  and later rewritten without that field.
  Migrate the value to a signed kind-`0` metadata event before rewriting the
  identity file.
- Replica-store backups and export manifests now record a stable schema
  compatibility version instead of package SemVer. Existing backups stamped by
  `0.1.0-alpha.2` remain restorable because this release does not change their
  stored schema.
