# Changelog

All notable changes to the Radroots core libraries are documented in this file.

## [1.0.0-alpha.1]

This alpha is not published until the repository release preflight and external
publish policy both pass for the same source revision.

### Changed

- NIP-99 kind `30402` now has an explicit two-level taxonomy: the standard
  protocol kind and coordinate are **Classified Listing**, while the richer
  Radroots farm, bin, inventory, and price profile is **Operational Listing**.
  Public constants, types, functions, modules, operation IDs, and generated DTO
  roots use those unambiguous names with no legacy `listing` aliases or modules.
- Operational listing decoding now has one tag-authoritative implementation in
  `radroots_event_codec`. The typed parts decoder reports the established
  listing error taxonomy, and JSON content can no longer override canonical
  product, inventory, or bin tags in trade and replica consumers.
- Operational listing authoring now emits canonical Markdown content from the
  tag-authoritative model. Tolerant inbound JSON inspection remains a decode
  compatibility boundary only and is not an authoring format.
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
  separates every `e`-tagged event as a thread-excluded candidate without a
  Reply claim, and deterministically admits Ask, PhotoUpdate, or Update roots
  while preserving malformed media diagnostics.
- Typed root posts now enter signing and client publication through an opaque
  builder with no raw tag/content mutation. Generic builder direct signing and
  client publication reject kind `0` plus unmarked kind `1` before signer
  access; externally supplied unsigned events, NIP-46 signing, and signed-event
  relay remain explicit low-level Nostr interoperability with no typed product
  authoring claim.
- Frozen drafts now carry event-contract registry version `3`, revalidate all
  persisted fields and the recomputed event id during deserialization and
  signing, and enforce explicit `GenericDraft`, `TypedOnly`, and `ReadOnly`
  authoring policies.
- Event wire and envelope admission now reject more than 4,096 aggregate tag
  elements, and SDK-event conversion returns the typed envelope error instead
  of panicking on relay-controlled oversized input.
- Strict authored posts enforce the 256 KiB compact signed-event ceiling after
  exact JSON escaping, in addition to per-element and aggregate decoded tag
  limits.
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
  admission, thread-candidate exclusion, classifier precedence, tolerant
  metadata retention, and stable rejection codes. Operation-owned vectors also
  execute every typed post authoring, projection, and admission function and
  compare complete deterministic outputs.

### Removed

- The duplicate private operational listing parser in `radroots_trade` was
  removed; trade validation now consumes the canonical event codec.
- The ambiguous `listing` source modules and public compatibility aliases were
  removed. Consumers must migrate to the Classified Listing protocol names or
  Operational Listing product names according to the API's responsibility.
- The public operational-listing JSON wire-parts authoring helper was removed;
  product authoring uses the canonical Markdown wire-parts path.
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
- `RadrootsNostrSignerBackend` no longer accepts a raw `nostr::EventBuilder`;
  callers that implement standard external signer protocols must supply the
  protocol's unsigned event explicitly.
- Permissive `RadrootsPost` tag authoring, the free-form Nostr post builder, and
  the generic net custom publisher were removed. Product-root publication now
  requires one of the strict authored Update, PhotoUpdate, or Ask states; the
  generic protocol builder remains only behind a root-kind-`1` publication
  guard and for non-product interoperability.

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
- Persisted frozen drafts with event-contract registry versions `1` or `2` are
  rejected and must be reconstructed against registry version `3`; strict
  Profile plus typed and read-only post contracts can no longer be
  reconstructed as generic drafts.
- This breaking capsule revision must not be pinned or published through
  downstream product clients until `radroots_app_rt` is migrated, generated FFI
  bindings are rebuilt, and downstream compile and contract qualification pass.
- `radroots_event_from_nostr` now returns `Result<RadrootsEventEnvelope,
  RadrootsEventEnvelopeError>`; callers must handle hostile or oversized SDK
  events explicitly.
