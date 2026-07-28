# Changelog

All notable changes to the Radroots core libraries are documented in this file.

## [1.0.0-alpha.1]

This alpha is not published until the repository release preflight and external
publish policy both pass for the same source revision.

### Changed

<!-- release-change: outbox-authenticated-sqlite-lifecycle -->
- Outbox SQLite open now owns a lazy pool capped at four file connections and
  authenticates UTF-8, bounded catalog and ledger text, the managed schema,
  owned foreign keys, and scoped integrity under one five-second deadline
  before exposing the store. Persistent WAL configuration follows authority;
  caller pools, raw pool access, and live-store migration were removed.
  Destructive rollback is now an explicit data-loss-acknowledged offline-path
  operation that rejects live owned handles, while full-database integrity is
  a separate maintenance call and public diagnostics are capped at 4,096
  bytes.
<!-- release-change: outbox-versioned-migration-authority -->
- Outbox schema initialization now uses an ordered, generated migration
  registry with immutable source checksums, authenticated catalog fingerprints,
  and a tamper-evident schema ledger. Existing unledgered databases are adopted
  only when their complete governed catalog matches the frozen `0001_outbox`
  identity; partial, changed, counterfeit, gapped, newer, or unknown outbox
  state fails before governed mutation while unrelated caller tables remain
  untouched. Raw migration SQL exports and live `migrate_down` were replaced by
  authenticated schema status and owned open-time migration. A machine-readable
  matrix now governs no-default, SQLite, Tokio, event-store-adapter, and
  all-feature builds.
<!-- release-change: outbox-phase1-publication-state -->
- Outbox schema version `2` adds an isolated typed Phase 1 publication state
  machine. Enqueue accepts only sealed allowlisted artifacts with complete
  media-readiness bindings, persists exact canonical bytes and raw fixed-width
  operation identities, and uses bounded canonical relay targets. Revision-CAS
  transitions and opaque expiring claims fence signing and target workers;
  signed-event bytes are immutable, while dispatch intent, uncertain results,
  receipts, and accepted-observation repair identities remain durable without
  performing network publication.
<!-- release-change: verified-signed-publication-boundaries -->
- Authority signing now verifies BIP340 signatures before returning trusted
  results. Outbox inputs, persisted reloads, runtime dispatch and inbound
  observations, and direct Nostr publication require the verified signed-event
  typestate; invalid or corrupted signatures fail before durable, event-store,
  or relay-adapter mutation.
<!-- release-change: phase1-verified-signing-retry-bridge -->
- Phase 1 publication now crosses a claim-fenced, freshly revalidated typed
  signing preflight into an authorized signer without reopening generic
  `TypedOnly` draft construction. The original signature-verified NIP-01 event
  JSON object bytes are persisted once, quarantined on durable corruption, and
  reused with the same per-target dispatch identity across retries. Exact-byte
  identity applies to the decoded event object, not WebSocket framing.
- Event-store schema initialization now uses a transactional, checksummed
  migration authority with exact legacy-baseline adoption, shared-database
  catalog scoping, tamper-evident fail-closed managed history, exact catalog
  deltas, SQLite and FTS5 integrity validation, a no-write current-schema fast
  path, and read-only schema status inspection. Rollback is a terminal,
  pool-closing maintenance operation with a public version floor. Raw migration
  SQL and unrestricted destructive rollback are no longer public APIs.
- Event-store schema version `2` now installs a byte-pinned NIP-09
  reconciliation hook over immutable NIP-01, addressable-feed-v1, and
  registry-v7 semantics. The hook re-verifies durable raw authority and
  persists generation-partitioned event coordinates, deletion requests,
  normalized targets, canonical addressable state, and append-only
  transitions without rewriting or deleting raw events. Projection cursors
  now bind to the active source generation; version or generation changes use
  a typed, revision-bound rebuild ticket that rejects stale, replayed, raced,
  and ABA-replaced resets. Initial reconciliation and repeated rebuilds use a
  marker-first transaction whose deferred commit barrier rejects partial
  authority; every successful rebuild appends a fresh generation while
  preserving immutable generation and transition history. Critical owned,
  borrowed-savepoint, and extension wrapper bodies now bind production AST
  identity, as do the isolated reconciliation-core and raw-head storage
  modules. Post-core orchestration receives a private transaction capability
  instead of SQLx authority; its fixed literal-SQL methods confine
  trade/observation writes to declared operation/table pairs. A lightweight
  source/transition/schema seal runs after that capability is dropped and
  rejects protocol-authority drift without introducing per-ingest full-table
  scans. Schema, pool, status, and ingest boundaries bind governed access to
  SQLite `main`, reject ASCII-case-insensitive temporary-schema collisions,
  preserve unrelated shared-schema foreign-key evidence, and retain both the
  primary ingest and rollback errors when rollback also fails.
- Transport targets and Reticulum destinations now keep identity-bearing
  fields private, expose read-only accessors, and revalidate canonical URI,
  scope, label, routing, and fingerprint invariants during deserialization.
  Nostr relay targets now use strict ASCII host, port, path, default-port, and
  IPv6 canonicalization shared with the relay adapter. Policy-free relay URL
  and target-set serialization has been removed; duplicate target sets fail,
  and fetch requests require a typed nonempty target set. Adapter fetch items
  are canonicalized and must belong to that request before any store mutation,
  while the request timestamp is the sole observation-time authority. Public
  relay policy is explicitly for trusted configured `wss` hostnames and
  rejects local, special-use, single-label, and forbidden literal
  destinations; localhost policy accepts exact loopback hosts only. The
  default SDK connector does not make attacker-controlled DNS names an SSRF
  boundary. Event-store diagnostic messages must be caller-redacted,
  canonical, control-free, nonempty, and no larger than 4 KiB; automatic relay
  publish observations no longer persist remote outcome text. Observation v1
  remains endpoint-level and intentionally does not represent scoped
  Reticulum or local-target identity; scoped evidence stays in transport
  delivery receipts. This
  intentionally changes fingerprints and public APIs for spellings and
  configurations previously accepted. Because this is an unreleased alpha
  contract, databases and serialized configuration containing those legacy
  identities are not migrated: development instances must be reset and
  canonically reseeded rather than silently reinterpreted.
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
- Nostr fetch-ingest receipts now report exhaustive verification, contract
  admission, valid-stream, and current-visibility outcomes independently.
  Verification failures no longer share an `invalid` bucket with contract
  failures, unsupported admissions retain their stable code without masking
  visibility, and persisted events obtain visibility from the event store's
  central authority. Fetches enforce a 64,000 raw-event ceiling, a 64 MiB
  aggregate raw-JSON prefix budget, and the 256 KiB per-event wire limit before
  Radroots parses adapter raw JSON; after upstream SDK frame decoding, the
  official SDK stream applies the same retained-prefix bounds before retaining
  serialized adapter output. Local event-store failures abort fetch ingest as
  operational errors instead of being reported as malformed relay input.
- Generic outbox APIs now reject every NIP-16 ephemeral event before durable
  queue persistence. Live-only events, including NIP-42 relay-auth and NIP-98
  HTTP-auth signatures, remain owned by their transport exchanges. Externally
  supplied SQLite pools now validate their backing mode and configure every
  connection before migration or writes.
- Retired trade order-workflow and product-projection source files that were no
  longer compiled or exported have been removed. Current FoodAvailability
  projection ownership remains with the event store.
- Event-store schema v3 adds one central current-visibility authority, a
  generation-bound addressable transition feed, and an atomic focused
  FoodAvailability projection with bounded FTS search. The successor contract
  authenticates schema `0003`, registry-v7 admission, exact kind scope `30402`,
  executable transition/projection vectors, and the frozen NIP-09 predecessor.
  Stored Blossom image digests use the public typed SHA-256 value.
- Event-store schema v4 adds a persisted raw-source capacity seal for event
  rows, tag rows, and their governed UTF-8 text bytes. Unique durable ingest
  now refuses prospective capacity excess before mutation, database reopen
  performs a bounded full recount, and independent file pools serialize an
  exact final capacity slot. Every supplied main database must report UTF-8
  before schema or journal mutation. Retained source history stops at eight
  generations before requesting fresh-store replacement and resync, and
  production rollback cannot cross the migration that introduced that
  append-only history. The
  authenticated SourceMaintenance successor binds the immutable schema-v3
  predecessor, migration and runtime sources, breaking capacity-error API
  replacements, and an executable result vector. Schema v4 is intentionally
  non-additive: it replaces exactly the Food projection delete guard, Food image
  delete guard, and source rebuild-marker insert guard, requires that exact
  symmetric catalog delta, and restores the exact v3 trigger SQL on rollback.
  A drifted v3 predecessor is rejected atomically rather than repaired during
  upgrade; repair authorization is reserved for a future rebuild after exact
  managed-v4 catalog, ledger, and migration history plus immutable raw/source
  lineage and capacity validation. Derived hook state is the repair target,
  not a repair precondition. The former
  `RadrootsEventStoreReconciliationResource` type and
  `ReconciliationCapacityExceeded` error variant are replaced by the
  versioned source-capacity resource and typed capacity/history errors.
<!-- release-change: event-store-raw-source-rebuild-authority -->
- Event-store schema v4 now exposes a versioned raw-source rebuild operation
  that repairs governed derived NIP-09, current-visibility, and focused
  FoodAvailability state from reverified immutable raw envelopes in one
  `BEGIN IMMEDIATE` transaction. The typed report returns prior and new source
  generations, the rebound capacity/high-water seal, a domain-separated
  immutable-raw digest, and a generation-normalized active-product digest.
  Rebuild drift exposes six stable typed authority categories while retaining
  non-contractual diagnostic detail for operators.
  The file-only repair entry point requires an existing exact managed-v4 WAL
  database and returns a store only after repaired state commits. It creates a
  deterministic single-connection pool and proves a fresh canonical-path
  connection shares the validated SQLite writer-lock domain. Callers must
  quiesce every alias, independent pool, direct SQL user, and filesystem path,
  symlink, or file-replacement operation for the repair duration. The public
  event-store error enum is now non-exhaustive so future typed recovery errors
  do not repeatedly break downstream matches.
  Rebuild preserves unrelated caller-owned tables with no schema dependency on
  any rebuild-mutated parent, generic projection cursors, and unrelated SQLite
  sequence row triples. Transition replay performs one shared
  target-alias cleanup, places the governed target first, and validates that
  exact row after replay; generic cursor inventory is instead
  prospectively bounded at 4,096 identities and invalidates lazily after a
  generation change.
  Before entropy or mutation, rebuild also bounds caller-owned main tables and
  cumulative foreign-key rows at 4,096 each and refuses every caller-owned
  inbound foreign key to every directly or indirectly mutated parent,
  regardless of its SQLite action, so derived replacement cannot cascade into
  caller rows or triggers. The sealed parent inventory includes the Food FTS5
  virtual table and all five shadows plus the governed `sqlite_sequence` row;
  it remains separate from the narrower scoped-integrity inventory.
  The executable raw-rebuild successor contract freezes the SourceMaintenance
  predecessor and the `0001` through `0004` migration inventory; no schema
  migration is added.
<!-- release-change: phase1-publication-artifact -->
- The `serde_json` event-codec surface now exposes a sealed Phase 1 publication
  artifact constructed only from strict authored Profile, Update, PhotoUpdate,
  Ask, date/time Event, and FoodAvailability models. The exact version-1
  envelope places the expected NIP-01 id at top level after the frozen draft,
  exposes only explicit canonical-byte encode/decode operations, and binds all
  fields except the digest under an ASCII-domain, single-NUL SHA-256 preimage.
  Construction and reload enforce the 2 MiB artifact, 256 KiB signed-event,
  and 4,096-reference bounds plus complete case-preserving Blossom URL
  commitments. Strict reload rejects unknown or duplicate fields, alternate
  encodings, cross-profile promotion, stale identifiers, media drift, and
  digest tampering without claiming restored byte verification, upload
  completion, signature authenticity, or signer authority.
<!-- release-change: phase1-publication-allowlist -->
- The event-codec publication surface now exposes an additive allowlist over
  the sealed Phase 1 artifact. Its opaque output admits exactly supporting
  Profile plus root Update, PhotoUpdate, Ask, typed date/time Event, and focused
  FoodAvailability leaves. Registry-v7 reprojection enforces
  Ask-before-PhotoUpdate-before-Update precedence and marker-partitions
  kind-`30402` before profile validation. A canonical-JSON adapter composes
  strict artifact reload with the same allowlist for durable consumers. Raw
  and generic events, including raw date/time events, replies, comments,
  deletion requests, calendar collections and RSVPs, BUD-11 authorizations,
  ephemeral events, trade, commerce, group, and operations families cannot
  produce its sealed input; deferred route/delivery product surfaces are
  non-events and likewise fail closed. Event-contract registry v7 remains
  byte-identical, and the new gate grants no signing, upload, retrieval, relay,
  or entitlement authority.
<!-- release-change: phase1-publication-media-readiness -->
- The event-codec publication surface now binds each allowlisted artifact to
  one canonical sealed Blossom readiness observation per distinct media URL.
  The independent version-1 binding persists ordered evidence and the artifact
  digest without duplicating artifact bytes, enforces exact hash, size, MIME,
  URL, format, and authored dimensions where present, and requires canonical
  empty evidence for media-free artifacts. Its bounded reload rejects missing,
  duplicate, extra, reordered, stale-policy, cross-artifact, private BUD-11,
  noncanonical, or digest-mutated state before signing. The artifact boundary
  now admits only nonempty, at-most-10-MiB JPEG, PNG, or still-WebP references
  whose canonical URLs are at most 4,096 bytes.
<!-- release-change: blossom-raster-decoder-security -->
- Blossom publication raster decoding is now governed as a dedicated security
  and cost boundary. Decoded output is capped at `80,000,000` bytes before
  allocation, down from the previous `160,000,000` byte ceiling. PNG is
  restricted to static 8-bit images with approved color types, one ordered
  palette, no critical unknown chunks, and bounded container records; WebP is
  restricted to static images whose extended header carries no reserved flags
  and exactly one primary chunk. Sequential JPEG decoding now charges bounded
  scan, block, coefficient-step, marker-record, and entropy bit-read budgets
  so no input can loop or allocate beyond its declared ceiling. Unsupported
  PNG and WebP raster processes fail with the new stable
  `publication_raster_process_forbidden` error. An exact 30-case regression
  corpus, a Nix-pinned independent-decoder differential, an isolated
  twelve-process maximum-resource matrix with a 128 MiB peak-RSS gate, and
  deterministic libFuzzer smoke targets with one raw seed per case now execute
  through the real public readiness API in the governed decoder-security lane.
<!-- release-change: blossom-publication-readiness-evidence -->
- Blossom publication media now advances beyond local byte verification only
  after typed BUD-02 status and descriptor agreement, an independent BUD-01
  HEAD, and an exactly bounded complete BUD-01 GET agree with the authored
  URL, hash, MIME, and length. The public evidence profile admits JPEG, PNG,
  and still WebP only, rejects animation, fully decodes the exact retrieved
  bytes with a declared-format decoder, and derives dimensions internally
  within 16,384 per axis and 20,000,000 pixels. JPEG is limited to 8-bit
  sequential SOF0/SOF1 and combines exact entropy accounting with pinned
  strict `zune-jpeg` and `zune-core` RGB decoding; PNG and WebP use a
  separately pinned two-format `image` build.
  Decoded output is bounded to 80,000,000 bytes before allocation; callers
  cannot provide decode claims.
  Deterministic per-URL evidence remains transport-neutral and contains no
  HTTP credentials, BUD-11 material, entitlement decision, or private service
  topology.
  The `serde` surface persists this sealed evidence as bounded canonical
  compact JSON and reloads it only after strict schema/policy, field,
  URL/hash/MIME/size/dimension/status, digest, and canonical-byte validation.
  General-purpose deserialization cannot construct the sealed evidence state,
  and the portable serde-only lane does not require the raster decoder.
- Bare-envelope replica ingestion is quarantined behind the explicit,
  non-default `legacy-ingest` feature. Default replica APIs expose emit and sync
  surfaces only; a future product ingest boundary must consume a store-produced
  verified, valid-stream-eligible, currently visible admission.
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
- Every Rust workspace package declares the exact `0.1.0-alpha` crate version
  explicitly so mounted path consumers preserve it, and every internal root
  dependency requires that same package cohort independently of the event
  contract generation.
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
- Behind the explicit non-default `legacy-ingest` feature, legacy replica
  ingestion verifies kind-`30402` signatures, selects the raw addressable head
  before profile decoding, and sends only the Operational Listing partition to
  its trade-product projection. Selected focused/generic exclusions and
  invalid/ambiguous rejections remove an older projection while advancing the
  head, preventing stale projection fallback. The feature-gated public
  head-only helper rejects kind `30402`; callers must use profile-aware legacy
  ingestion so the head and projection remain atomic. These helpers are not a
  Phase 1 product ingest boundary.
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
- The NIP-09 reconciliation-v1 manifest and executable event-store result
  vector pin migration `0002`, registry-v7 inventory, semantic vector inputs,
  and the frozen verification, admission, head-selection, and suppression
  source graph.
- Event-store NIP-09 migration now bounds every retained raw-source text field
  and row count through matching preflight, in-lock, and paged-loader checks.
  Capacity failure is typed and atomic, and exact-target indices avoid cloning
  request payloads or scanning unrelated requests per candidate head. Rebuild
  tickets can commit at their captured raw high-water while later events remain
  available for ordinary catch-up. Schema opens validate every applied hook,
  and composed callers can reserve the SQLite writer with
  `begin_write_transaction` before reading and ingesting in one transaction.
  Borrowed-transaction ingests use nested savepoints so a failed call cannot
  leave partial event-store writes available for the caller to commit.

- The `radroots_trade` validation-receipt feature now preserves its documented
  no-std plus `serde_json` build. Its error contract uses `core::fmt`, and alloc
  macros are imported explicitly instead of relying on the std prelude.
- The checked-in remote SP1 Core proof fixture now covers the validator-set
  witness identity and cryptographically verifies against the current guest.
  Guest builds explicitly allow the SP1-pinned compiler to trail the workspace
  Rust version while retaining locked dependencies.

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
