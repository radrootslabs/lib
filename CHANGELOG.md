# Changelog

All notable changes to the Radroots core libraries are documented in this file.

## [1.0.0-alpha.1]

This alpha is not published until the repository release preflight and external
publish policy both pass for the same source revision.

### Changed

- Event-store schema initialization now uses a transactional, checksummed
  migration authority with exact legacy-baseline adoption, shared-database
  catalog scoping, tamper-evident fail-closed managed history, exact catalog
  deltas, SQLite and FTS5 integrity validation, a no-write current-schema fast
  path, and read-only schema status inspection. Rollback is a terminal,
  pool-closing maintenance operation with a public version floor. Raw migration
  SQL and unrestricted destructive rollback are no longer public APIs.
- Geocoder locality and reverse results now expose administrative subdivision
  identifiers as opaque strings. SQLite integer and text values normalize to
  the same lossless public representation, and mixed-storage candidate order
  remains deterministic.
- Default GeoNames asset installation now uses cancellable asynchronous DNS,
  explicit connect, response, read, and total deadlines, denied redirects, and
  bounded runtime shutdown. HTTP bodies stream through incremental length and
  SHA-256 verification into a same-directory tempfile that is synced, checked
  for SQLite integrity and schema, and atomically persisted. Public download
  errors expose stable typed phases and detail fields instead of
  `reqwest::Error`; trusted injected fetchers retain a bounded byte adapter.
- Trusted event-contract admission now has one signature-verified entry point.
  Profile, root Post, Reply, Comment, DeletionRequest, and FoodAvailability
  retain typed admitted values; other registered events require full contract
  shape validation, while unsupported matching and invalid shapes remain
  distinct failures.
- Event-store ingest now verifies before durable admission, retains every
  verified durable candidate for registry-independent raw-head reduction, and
  separates immutable valid-stream replay from current visibility. Explicit
  raw, valid, raw-head, visibility, and visible-head APIs replace ambiguous
  projection/head reads; projection cursors require an expected version and a
  monotonic prior-sequence compare-and-swap. Verified ephemeral events receive
  an explicit not-persisted outcome and allocate no raw sequence, tags,
  observations, or heads. Read-only consumers can inspect the same fail-closed
  status summary from an initialized pool without duplicating schema-sensitive
  SQL or running migrations.
- Nostr fetch-ingest receipts now distinguish admitted, unsupported, invalid,
  malformed, inserted, duplicate, and ephemeral not-persisted events, carry
  stable admission codes when classification occurs, and name valid-stream
  eligibility directly. Local event-store failures now abort fetch ingest as
  operational errors instead of being reported as malformed relay input.
- Generic outbox APIs now reject every NIP-16 ephemeral event before durable
  queue persistence. Live-only events, including NIP-42 relay-auth and NIP-98
  HTTP-auth signatures, remain owned by their transport exchanges. Externally
  supplied SQLite pools now validate their backing mode and configure every
  connection before migration or writes.
- Retired trade order-workflow and product-projection source files that were no
  longer compiled or exported have been removed. Current FoodAvailability
  projection ownership remains with the event store.
- Blossom blob URLs now validate complete raw Unicode text before URL parsing
  and exact raw ASCII DNS label grammar before returning a typed value. Unicode
  control/format text, implicit IDNA conversion, empty labels, underscores,
  edge hyphens, and oversized DNS names can no longer enter approved or
  byte-verified media typestates; URL-parser-valid explicit ASCII punycode and
  canonical IP authorities remain supported.
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
- Operational listing trade validation exposes one shared unsigned-model
  semantic reducer and a signature-verified event boundary that delegates to
  it after kind, marker-partition, and decoding checks. Event-store projection
  reconstructs and verifies the event typestate instead of trusting a plain
  stored envelope. Canonical authoring now invokes that reducer before draft
  construction and preserves its typed failure cause; the reducer rejects
  duplicate bin IDs and invalid quantity or price semantics in every bin.
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
- NIP-10 Reply authoring now requires an opaque direct or nested type and emits
  exact marked `root`/`reply` event references plus required participant
  references. Verified inbound projection accepts preferred marked and
  deprecated positional threading, preserves valid supplemental references as
  citations, and retains malformed advisory metadata as ordered diagnostics
  while keeping every admitted Reply out of root-card classification.
- Authored and projected NIP-10 Reply and NIP-22 Comment relay hints now share
  `RadrootsNostrRelayHint`, one portable, canonical visible-ASCII WebSocket URL
  profile instead of generic URL normalization. Strict authoring rejects
  noncanonical hints; tolerant verified projection preserves rejected hints
  verbatim in ordered raw-tag diagnostics. Relay syntax remains separate from
  each event profile's wire-size budgets.
- Kind `1111` Comment handling now uses distinct opaque authored, verified
  projection, admitted-event, and sealed Nostr publication states. The strict
  NIP-22 profile supports only event or address roots for classified-listing
  kind `30402` and calendar kinds `31922` and `31923`; ordinary kind `1` and
  external `I`/`i` references are rejected. Legacy pseudo-thread tags carry no
  Comment authority, are never authored, and remain raw supplemental input.
- Typed root posts now enter signing and client publication through an opaque
  builder with no raw tag/content mutation. Generic builder direct signing and
  client publication reject kind `0` plus every kind `1` before signer access;
  externally supplied unsigned events, NIP-46 signing, and signed-event relay
  remain explicit low-level Nostr interoperability with no typed product
  authoring claim.
- Frozen drafts now carry event-contract registry version `7`, revalidate all
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

- A fixed signed central-admission corpus executes every admitted variant,
  Update/PhotoUpdate/Ask root classification, Post-to-Reply promotion,
  Operational Listing fallback from Food exclusion, generic NIP-99 exclusion,
  unsupported kinds, malformed registered shapes, and ambiguous Food markers.
- Generic protocol builders can now finalize into an opaque checked external
  signing request. The request preserves the standard unsigned-event JSON wire
  shape while preventing raw mutation or unchecked reconstruction, and it
  accepts only an exact author/id match with a valid NIP-01 signature.
- Kind `30402` now has one allocation-free raw marker-name partition that
  distinguishes focused FoodAvailability, richer Operational Listing,
  marker-free generic NIP-99, and mixed ambiguous inputs before profile
  tag-shape validation.
- FoodAvailability now has strict domain and authored-media input primitives
  for bounded identifiers and text, canonical decimal price and quantity,
  uppercase currency, the closed ten-unit food vocabulary, active or sold
  status, timestamps, dimensions, and at most 64 unique byte-verified Blossom
  images. Its typed codec emits exact kind-`30402` wire parts, while verified
  tolerant admission normalizes compatible inbound values, preserves bounded
  ordered image diagnostics, and excludes generic or operational listings.
  Strict revision comparison revalidates both signed events against authored
  wire semantics before enforcing a stable coordinate and `published_at` plus
  NIP-01 replacement ordering.
- Focused FoodAvailability signing and client publication now use a sealed
  Nostr builder with a construction-time timestamp and no raw mutation escape.
  Generic signing and client publication reject focused or mixed kind-`30402`
  profiles before signer access; signed-event relay remains transport-only.
  Typed signing and publication do not attest BUD-02 upload completion.
- Legacy replica ingestion now verifies kind-`30402` signatures, selects the
  raw addressable head before profile decoding, and sends only the Operational
  Listing partition to its trade-product projection. Selected focused/generic
  exclusions and invalid/ambiguous rejections remove an older projection while
  advancing the head, preventing stale projection fallback. The public
  head-only helper rejects kind `30402`; callers must use profile-aware
  ingestion so the head and projection remain atomic.
- Event-contract identification now selects Operational Listing only for its
  raw marker partition. Focused FoodAvailability is admission-only, while
  marker-free generic and mixed-marker NIP-99 events cannot be mislabeled as
  operational contracts.
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
- Raw signed NIP-10 vectors execute marked direct and nested Replies, marked
  supplemental citations, deprecated positional empty-marker author hints,
  malformed middle-citation tolerance, participant and relay validation,
  classifier precedence, signature-gated admission, and stable invalid-case
  codes through the public owning APIs.
- The fixed 114-case NIP-22 corpus executes all three governed Comment
  operations with complete authored wire, verified projection, diagnostic,
  admission, Unicode, precedence, and exact resource-limit expectations.
  Projection and admission inputs are self-contained signed event JSON, and
  the packaged fixture is byte-identical to the canonical contract vector.
- Generic NIP-01 coordinates now validate canonical
  `kind:pubkey:identifier` values with the correct replaceable and addressable
  kind rules. Strict NIP-09 authoring emits deterministic kind-`5` `e`, `a`,
  and derived `k` tags, while tolerant verified projection preserves advisory
  diagnostics and admission keeps the projection bound to its
  signature-verified envelope. Fixed contract-owned conformance vectors cover the
  authored, projection, admission, and resource-boundary operations.
- NIP-09 deletion requests now enter signing and client publication through a
  sealed typed builder with no raw kind, content, or tag mutation. This surface
  creates and transports a request only; it provides no target lookup,
  authorship decision, deletion authorization, store mutation, relay-effect,
  or deletion-effect semantics.
- NIP-09 suppression evaluation is now a separate pure operation over one
  signature-verified candidate and admitted requests. It enforces same-author
  direct-event and inclusive address-cutoff rules, keeps kind `5` immune,
  ignores advisory kinds, returns canonical evidence independent of input
  order, and never mutates raw events or storage.

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
  requires one of the strict authored Update, PhotoUpdate, or Ask states.
  Generic kind-1 authoring is no longer available through the generic protocol
  builder; non-product interoperability remains available only for
  non-reserved kinds.
- The permissive post-reply builder and raw-string net reply publisher were
  removed. Reply signing and client publication now require the typed NIP-10
  Reply boundary.
- The legacy `RadrootsComment` DTO/Serde model, permissive Comment
  encode/decode modules, and their public functions were removed. Comment
  authoring now requires `RadrootsAuthoredNip22Comment`, while inbound use must
  pass through verified projection or admission.
- The Reply-owned `RadrootsNip10RelayHint` name was removed. Callers must use
  the shared `RadrootsNostrRelayHint` type for canonical Nostr tag relay hints.

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
- Persisted frozen drafts with event-contract registry versions `1` through
  `6` are rejected and must be reconstructed against registry version `7`;
  strict Profile plus typed and read-only post contracts can no longer be
  reconstructed as generic drafts.
- Generic builder signing and client publication reject kind `1111` before
  signer access. Product Comment publication must use the sealed NIP-22
  builder; relaying an already signed event remains a generic transport
  operation without a typed authoring claim.
- Generic builder signing and client publication reject every kind `5` before
  signer access. Product deletion-request publication must use the sealed
  NIP-09 builder; externally supplied unsigned events, NIP-46 signing, and
  relaying an already signed event remain low-level interoperability operations
  without a typed authoring or deletion-effect claim.
- This breaking capsule revision must not be pinned or published through
  downstream product clients until `radroots_app_rt` is migrated, generated FFI
  bindings are rebuilt, and downstream compile and contract qualification pass.
- `radroots_event_from_nostr` now returns `Result<RadrootsEventEnvelope,
  RadrootsEventEnvelopeError>`; callers must handle hostile or oversized SDK
  events explicitly.
