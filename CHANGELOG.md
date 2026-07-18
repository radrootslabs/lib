# Changelog

All notable changes to the Radroots core libraries are documented in this file.

## [1.0.0-alpha.1]

This alpha is not published until the repository release preflight and external
publish policy both pass for the same source revision.

### Changed

- Generic NIP-01 identifier and signature verification is now independent of
  knowledge decoding, and every dynamic Nostr kind conversion rejects values
  above `65535` instead of truncating them. Canonical-length author keys that
  are not valid secp256k1 curve points now return `malformed_envelope` instead
  of `signature_invalid`.
- Calendar authoring and admission now use explicit NIP-52 authored, parsed, and
  admitted states for date events, time events, calendars, and RSVPs. Kind
  `31922` no longer emits uppercase `D`; kind `31923` derives integer UTC-day
  `D` values from its validated time range.
- Authored profile and calendar media now share the byte-verified Blossom image
  proof type; unverified URLs remain inbound data and cannot enter authoring APIs.
- Root kind-`1` product admission now verifies NIP-01 identity and signatures,
  excludes replies before product classification, and deterministically projects
  Ask, PhotoUpdate, or Update while preserving malformed media diagnostics.
- Workspace packages declare one governed version explicitly so mounted path
  consumers preserve it, and every internal root dependency requires that exact
  pre-release version.
- Conformance suites now identify the `1.0.0` event-contract generation.
- Release metadata records exact governed impacts for removed public types,
  fields, functions, modules, constants, Cargo features, and trait
  implementations, plus changed field types, constant values, and algorithms.

### Added

- Verified Profile admission binds a signed exact kind-`0` envelope to the
  tolerant metadata projection, accepts standard tagless events, and exposes
  deterministic equal-time lowest-id replacement vectors.
- Strict authored Update, PhotoUpdate, and Ask types emit deterministic kind-`1`
  wire parts. Photo and Ask media require byte-verified Blossom image
  descriptors, exact ordered NIP-92 metadata, bounded nonzero fields, and
  same-digest approved fallback URLs.
- Raw signed kind-`1` conformance vectors prove signature-gated profile
  admission, reply exclusion, classifier precedence, tolerant metadata
  retention, and stable rejection codes.

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
- Permissive `RadrootsPost` tag authoring, the free-form Nostr post builder, and
  the generic net post publisher were removed. Publication now requires one of
  the strict authored Update, PhotoUpdate, or Ask states.

### Compatibility

- Callers that previously passed out-of-range `u32` kinds to Nostr builders or
  job decoders now receive typed range errors instead of truncated kinds.
- Identity JSON containing the removed embedded `profile` projection is now
  rejected, including the nested public-profile form, instead of being loaded
  and later rewritten without that field.
  Migrate the value to a signed kind-`0` metadata event before rewriting the
  identity file.
- Replica-store backups and export manifests now record a stable schema
  compatibility version instead of package SemVer. Existing backups stamped by
  `0.1.0-alpha.2` remain restorable because this release does not change their
  stored schema.
