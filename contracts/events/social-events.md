# Public Social Event Substrate

Status: active implementation contract

Scope: public Radroots social Nostr event models, codecs, and deterministic conformance vectors in
this repository.

## Purpose

The public social event substrate extends the Radroots event family beyond profile, farm,
operational listings, and trade workflows while keeping relay runtime behavior, application
projections, moderation services, and private Field business documents outside this repository's
event-contract boundary.

The target implementation is standards-first and Radroots-named. Event models live in
`radroots_event`, canonical encode/decode behavior lives in `radroots_event_codec`, and
deterministic fixtures live under `contracts/conformance`.

Calendar behavior is based on
[NIP-52](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/52.md)
and its collection metadata uses
[NIP-51](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/51.md),
while text-note Reply threading follows
[NIP-10](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/10.md),
all at NIPs commit `bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91`. Calendar media uses the public
Blossom primitives governed by the protocol pin in
[`blossom-media.md`](blossom-media.md). The upstream NIP-52 rules and the stricter Radroots
authoring and admission profile are separate contract layers.

## Implementation Inventory

The repository implements strict authored and verified-projected kind `1` root-post profiles, a
separate strict-authored and tolerant-inbound kind `1` NIP-10 Reply profile, kind `1111`
`RadrootsComment`, kind `7` `RadrootsReaction`, generic `RadrootsList` entries, operational listing
records through `RadrootsOperationalListing`, the raw kind-`30402` profile partition and validated
FoodAvailability authored, verified-admission, and revision contract, articles, generic public file
metadata, calendar date events, calendar time events, reposts, generic reposts, calendar
collections, RSVP events, and reports.

The closeout contract requires:

- complete model and codec coverage for the approved public social event families
- kind and tag constants for the approved NIP surface
- ordinary kind-1 compatibility reads plus strict Update, PhotoUpdate, and Ask authoring
- strict marked direct and nested NIP-10 Reply authoring plus tolerant positional inbound admission
- strict NIP-22 `RadrootsComment` behavior without legacy `e_root` or `e_prev` fallback tags
- strict NIP-25 `RadrootsReaction` behavior where empty content is a valid like
- explicit optional `published_at` support for NIP-99 classified-listing parity
- NIP-65 relay-list validation evidence through `RadrootsList`
- conformance vectors and canonical-event witnesses for every new or upgraded social event family

## Approved Event Families

The MVP public social substrate includes:

- strict `RadrootsAuthoredUpdate`, `RadrootsAuthoredPhotoUpdate`, and
  `RadrootsAuthoredAsk` publication types plus verified tolerant projection for
  ordinary NIP-01 kind `1` events
- strict `RadrootsAuthoredNip10Reply` direct and nested publication plus
  `RadrootsInboundNip10ReplyProjection` for marked and deprecated positional
  inbound NIP-10 replies
- `RadrootsArticle` for NIP-23 kind `30023` long-form content
- generic public `RadrootsFileMetadata` for NIP-94 kind `1063`
- strict authored `RadrootsAuthoredCalendarDateEvent`, tolerant
  `RadrootsParsedNip52CalendarDateEvent`, and strict admitted
  `RadrootsAdmittedCalendarDateEvent` models for NIP-52 kind `31922`
- strict authored `RadrootsAuthoredCalendarTimeEvent`, tolerant
  `RadrootsParsedNip52CalendarTimeEvent`, and strict admitted
  `RadrootsAdmittedCalendarTimeEvent` models for NIP-52 kind `31923`

The production-v1 public social substrate includes:

- `RadrootsRepost` for NIP-18 kind `6`
- `RadrootsGenericRepost` for NIP-18 kind `16`
- strict authored `RadrootsAuthoredCalendar`, tolerant
  `RadrootsParsedNip52Calendar`, and strict admitted `RadrootsAdmittedCalendar`
  models for NIP-52 kind `31924`
- strict authored `RadrootsAuthoredCalendarEventRsvp`, tolerant
  `RadrootsParsedNip52CalendarEventRsvp`, and strict admitted
  `RadrootsAdmittedCalendarEventRsvp` models for NIP-52 kind `31925`
- `RadrootsReport` for NIP-56 kind `1984`
- operational-listing profile validation through `RadrootsOperationalListing` at NIP-99
  classified-listing kind `30402`
- strict FoodAvailability details, deterministic unsigned authoring, verified tolerant projection,
  NIP-01 admission, and strict revision validation for focused kind-`30402` inputs, with explicit
  exclusion of Operational Listing and marker-free generic NIP-99 candidates
- relay-list kind `10002` validation through `RadrootsList`

## Contract Decisions

`RadrootsPost` remains a compatibility read projection for ordinary kind `1`
text notes and older optional social metadata. It is not an authored boundary.
The public raw `imeta` encoder and its generic tag-builder implementation are
removed so callers cannot turn mutable strings into purported strict media.

### Kind-1 Post Trust Layers

Strict authored root posts are private-field typestates. Update and Ask content
must be non-whitespace and every profile is bounded to 131072 UTF-8 bytes.
PhotoUpdate and optional Ask media use between one and 64 NIP-92 `imeta` tags.
Each image emits exactly `url`, `x`, `m`, `dim`, `size`, and `alt`, in that
order, followed by ordered repeatable `fallback` fields. Primary URLs are
unique and occur as exact substrings of content. MIME is parameter-free
canonical lowercase and matches `^image/[a-z0-9][a-z0-9.+-]*$` exactly;
dimensions are nonzero `u32` values; size is a nonzero `u64`; alt text is
non-whitespace and at most 4092 UTF-8 bytes. Authored events emit at most 4096
tag elements; every element is at most 4096 bytes and all tag elements together
are at most 131072 bytes, including the Ask marker. The strict image state
retains the exact validated
`imeta` representation that the codec emits, so limit validation and encoding
cannot diverge. Authored posts also fit within the 262144-byte compact signed
event limit after exact JSON escaping, tag-array punctuation, and a worst-case
20-digit NIP-01 timestamp are counted; decoded content and tag limits do not
operate as independent escape hatches around that wire budget.

Every authored primary image is a `RadrootsAuthoredImage` backed by an approved,
byte-verified Blossom descriptor. Every authored fallback is an approved
Blossom hash-path URL with the same digest. This typestate proves local
descriptor-to-byte agreement only. Successful BUD-02 upload completion remains
a separate runtime precondition before signing.

Ask is kind `1` and deterministically emits exactly
`["t","radroots-ask"]`. PhotoUpdate is also kind `1`; kind `20` is outside
this contract. Update emits neither the Ask marker nor `imeta`.

Inbound projection accepts only a `RadrootsSignatureVerifiedEvent`. Any `e`
tag, including an empty or malformed one, selects `ThreadExcluded` before Ask
or media inspection. This is an exclusion-only candidate classification and
does not claim that the event is a valid NIP-10 reply; strict NIP-10 parsing and
promotion remain separately owned. `verify_and_admit_post_event` returns
`RadrootsPostAdmissionOutcome`: only `Root(RadrootsAdmittedRootPostEvent)`
exposes an exact product contract, while
`ThreadExcluded(RadrootsThreadExcludedPostCandidate)` preserves the verified
event and exclusion projection. For roots, exactly one two-element Ask marker
after ASCII whitespace trim and ASCII case folding selects Ask. Multiple
normalized markers fail projection, while a malformed marker shape is retained
as an ordered diagnostic. Ask precedes PhotoUpdate even when attached media is
malformed. PhotoUpdate requires one through 64 wholly qualifying `imeta`
entries; a malformed or mixed set becomes Update with ordered diagnostics.
Unknown fields and repeatable fallbacks preserve wire order, while duplicate
known singletons disqualify media.

Inbound HTTP(S) media references remain unverified structural strings. The
projection performs no retrieval and makes no Blossom, byte, upload,
reachability, image-decoding, or safety claim. The registry therefore keeps
ordinary unsigned kind-1 identification on `radroots.social.post.v1`; exact
Update, PhotoUpdate, and Ask contracts use `AdmissionOnly` and are returned only
by the verified projection/admission boundary.

The event-contract registry assigns an explicit authoring policy to every
contract. Strict Profile, Update, PhotoUpdate, Ask, and NIP-10 Reply contracts are
`TypedOnly`; `radroots.social.post.v1` is `ReadOnly`; ordinary generic-draft
contracts remain `GenericDraft`. `RadrootsEventDraft::new` therefore rejects
the strict Profile contract and every governed kind-1 contract with
`contract_not_draft_authorable`. Serialized drafts record registry version `5`
and are accepted only after deserialization revalidates the registry version,
contract, kind, shape, policy, recomputed event id, and known fields. The
frozen-draft signing boundary repeats that validation, so stale version-`1` through version-`4`
drafts must be rebuilt. Typed root posts and Replies enter Nostr signing and client
publication only through opaque profile-specific builders that expose
timestamp selection and signing, but no raw tag/content mutation or public
conversion to the upstream builder. The opaque generic builder rejects kind `0` and every
kind `1` event at both direct signing and client publication before a signer is
consulted.

### NIP-10 Reply Trust Layers

`RadrootsAuthoredNip10Reply` is an opaque, bounded authoring state. Direct
Replies contain one root reference; nested Replies contain one root and one
distinct parent reference. Each reference carries a validated 64-character
lowercase event id, a referenced-author pubkey, and an optional `ws` or `wss`
relay hint. Content is non-whitespace and shares the root-post content,
tag-element, total-tag-byte, and compact signed-wire budgets.

Strict authoring is deterministic. A direct Reply emits
`["e",<root-id>,<relay-or-empty>,"root"]` followed by the root author's
two-element `p` tag. A nested Reply emits the root `e` tag, then
`["e",<parent-id>,<relay-or-empty>,"reply"]`, then the root and parent author
`p` tags in that order; equal authors are emitted once. No other authored tag
shape is accepted. Signing and client publication are exposed only through the
sealed `RadrootsNostrNip10ReplyEventBuilder`.

Inbound projection requires a signature-and-id verified envelope. It accepts
preferred marked `e` references, including the optional NIP-10 author hint, and
supplemental unmarked `e` citations in the same event. An empty marker slot is
treated as absent so a citation may retain its optional fifth-element author
hint. Malformed supplemental references are ignored with ordered diagnostics
without erasing an otherwise unambiguous marked Reply. The projection also
accepts deprecated positional references where the first `e` tag is the root,
the last is the parent, and intermediates are citations. Empty marker slots
remain absent in this mode, including when a fifth-element author hint exists.
Malformed intermediate citations become diagnostics; malformed root or parent
anchors remain hard failures. Because NIP-10 makes
participant propagation, relay hints, and referenced-author hints advisory,
blank content or absent `p` tags do not erase an otherwise unambiguous inbound
Reply. Malformed optional relay, author-hint, citation, and participant metadata is
retained in the verified envelope and exposed as ordered typed diagnostics;
valid values are projected best-effort. This tolerant read-side behavior never
weakens strict authored output.

The registry's Reply `e` and `p` tag contracts describe normalized qualifying
semantic references. They do not claim that every malformed optional raw tag
retained by the verified envelope is itself a valid identifier-bearing tag.

Any `e` tag excludes a kind-1 event from root-card admission before Ask or
media classification. A thread-excluded candidate can be promoted only by the
separate NIP-10 Reply admission boundary; a Reply carrying Ask or media
metadata therefore remains a Reply and never becomes a root card. Admission
proves only the Reply envelope's NIP-01 id and signature plus its NIP-10
structure. It does not retrieve a referenced event or prove its existence,
kind, signature, author, relay availability, or relationship to the declared
author hint.

The signer backend's externally supplied unsigned-event operation and the
standard NIP-46 `sign_event` method are explicit low-level interoperability
boundaries. Their signed results prove Nostr cryptographic authorization only;
they do not confer a Radroots typed product-authoring claim and are not product
authoring entry points. Relaying an already signed event is likewise a generic
transport operation with no Radroots authoring claim.

Each post and Reply operation owns exact valid and invalid case kinds in
`contracts/operations.toml`. The canonical and packaged conformance suites
execute every public Update, PhotoUpdate, and Ask authoring function, verified
projection, admission function, and NIP-10 Reply authoring, projection, and
admission boundary; they compare complete deterministic wire parts or
projections and enforce stable negative error codes. Xtask validation pins the
complete operation namespaces, ownership metadata, public types, and exact
case-id-to-kind corpus; it rejects missing, duplicate, unclaimed,
mis-prefixed, or substituted cases.

`RadrootsComment` uses strict NIP-22 semantics. The target and scope model must support event-id,
address, and external roots or parents through `E`/`e`, `A`/`a`, and `I`/`i` tags with matching
`K`/`k` kind metadata. Canonical encode and decode must reject ordinary kind `1` short text note
targets; kind `1` replies belong to NIP-10 text-note reply semantics instead. Canonical decode must
reject legacy `e_root` and `e_prev` fallback tags.

`RadrootsReaction` uses strict NIP-25 semantics. Empty content, `+`, `-`, emoji, and custom reaction
content are valid when the target tags are valid. Missing targets remain invalid.

`RadrootsReport` intentionally tightens NIP-56 for the Radroots type: a reported pubkey `p` tag is
required for a valid report, including event and file or blob reports.

Generic public `RadrootsFileMetadata` remains separate from private `RadrootsFarmFileMetadata` even
though both use kind `1063`. The public generic model must cover the current simple NIP-94 tags,
including URL, MIME type, SHA-256 hash, original hash, size, dimensions, blurhash, thumbnail, image,
summary, alt text, fallback, `magnet`, `i`, and `service`.

### FoodAvailability Domain Boundary

Kind `30402` routing first inspects only the raw first element of each tag. The focused marker set is
exactly `radroots:price_unit` and `radroots:quantity`; the Operational Listing marker set is exactly
`radroots:primary_bin`, `radroots:bin`, and `radroots:price`. Focused-only, operational-only,
marker-free generic NIP-99, and mixed-marker inputs are distinct partition results. This happens
before profile tag-shape validation, so a malformed one-element marker still counts. Matching is
exact and case-sensitive. `RadrootsClassifiedListingPartition` names those results
`FocusedFoodAvailability`, `OperationalListing`, `GenericNip99`, and `Ambiguous`. The partition is
allocation-free, does not inspect the event kind, marker values, or tag arity, and does not establish
that either profile is valid.

`RadrootsFoodAvailabilityDetails` and its focused domain values are checked before construction.
The identifier is 1 through 512 UTF-8 bytes, contains no whitespace, and contains no Unicode
control or format character. Content must contain at least one scalar outside Unicode whitespace
and the information-separator range U+001C through U+001F, and is bounded to
131072 UTF-8 bytes; it is not normalized or subjected to the stricter metadata-text policy. Title,
summary, and location are trimmed, nonempty, control-free text bounded to 4096 UTF-8 bytes.
`published_at` is a canonical nonzero `u64` decimal and can be checked as no later than a supplied
`created_at`.

Price and optional quantity are canonical unsigned plain decimals with at most 28 ASCII digits
excluding the optional decimal point. Price may be zero; quantity must be positive. Currency is
exactly three uppercase ASCII letters. The dedicated unit vocabulary is `g`, `kg`, `lb`, `oz`,
`each`, `dozen`, `bunch`, `punnet`, `bag`, and `basket`, and quantity uses the same unit as price.
Status is exactly `active` or `sold`.

Image dimensions are two nonzero canonical `u32` decimal components in `WIDTHxHEIGHT` form.
Validated details accept no more than 64 images, reject duplicate URLs or Blossom digests, and
accept only `RadrootsAuthoredImage` values backed by an approved, byte-verified image descriptor.
That proof establishes local descriptor-to-byte agreement only. It does not establish BUD-02 upload
completion, decoded raster dimensions, content safety, retrieval, reachability, or network
availability.

The details model deliberately has no farm, bin, route, pickup, delivery, order, checkout, or other
commerce-workflow field. `authored_food_availability_to_wire_parts` accepts these details plus
`created_at`, requires `published_at <= created_at`, and emits unsigned kind-`30402` wire parts. Its
closed tag sequence is exactly `d`, `title`, `summary`, `published_at`, `location`, `price`,
`radroots:price_unit`, optional `radroots:quantity`, `status`, then zero or more `image` tags. Price
is exactly amount plus currency, quantity is exactly amount plus unit, and image is exactly URL plus
dimensions. The decoded tag budgets and the 262144-byte compact signed-event budget are both
enforced. The operation neither chooses `created_at` nor signs or transports the result.

Inbound projection accepts only a `RadrootsSignatureVerifiedEvent`. Raw marker partitioning occurs
before focused tag validation: Operational Listing and marker-free generic NIP-99 candidates are
explicit exclusions, while mixed markers are an error. A focused candidate requires the complete
core profile and rejects tags that purport to add buyer, checkout, delivery, exception, group,
invite, order, payment, pickup, proof, provenance, receipt, route, `route_stop`, or task capability.
Other optional NIP-99 tags remain outside the focused projection and are ignored. Accepted inbound
decimal values are normalized without exceeding the 28-digit wire bound, and three-letter currency
is normalized to uppercase.

Inbound image observations are not authored media typestates. The projection retains at most the
first 64 tags in wire order and records stable ordered diagnostics for count overflow, malformed
shape, invalid HTTP(S) URL, missing or invalid dimensions, duplicate URL, and duplicate Blossom
digest. Image diagnostics do not invalidate an otherwise complete focused core. They confer no
Blossom approval, byte agreement, upload, decoding, retrieval, safety, or availability claim.

`verify_and_admit_food_availability_event` performs NIP-01 id and Schnorr verification before that
projection and returns either `RadrootsAdmittedFoodAvailabilityEvent` or an explicit verified
non-focused exclusion. The admitted type binds the projection to the verified envelope and exposes
the admission-only `radroots.food.availability.v1` registry contract. The contract is `TypedOnly`:
strict authored details are its only product-authoring input, and unsigned kind or tag matching does
not confer focused admission.

Revision validation accepts two independently signature-verified events and re-applies the exact
authored wire profile to each side; tolerant normalized or diagnostic-bearing projections cannot
enter this comparison. Kind, author, `d`, and `published_at` must remain stable. The candidate must
have a later `created_at`, or the lower event id when both timestamps are equal. Invalid previous
and current inputs have side-specific errors.

With the `events` feature, `radroots_nostr_build_food_availability_event` fixes `created_at` during
typed construction, derives the exact strict wire parts, and returns a sealed builder with no raw
tag, content, or timestamp mutation. The builder supports local signing and, with the `client`
feature, typed relay publication. Generic builder signing and client publication reject focused and
mixed-marker kind-`30402` events before signer access. Marker-free generic NIP-99 and
operational-only builders remain available for explicit compatibility, while relaying an already
signed event remains transport-only and establishes no Radroots authoring claim.

Legacy replica ingestion verifies kind-`30402` identifiers and signatures before acquiring its
write transaction, then selects the raw addressable head before profile decoding. Only the
Operational Listing partition can reach the legacy trade-product projection. A signature-valid,
coordinate-valid, selected focused event or marker-free generic NIP-99 event advances the raw head
as excluded; selected invalid focused, mixed-marker, and malformed operational profiles advance it
as rejected. Every selected excluded or rejected replacement removes an older operational
projection, so stale events cannot resurrect it. A signature failure changes neither the raw head
nor the projection; a missing or invalid `d` tag fails before an addressable head can be selected.
The public head-only helper rejects kind `30402`, which must use profile-aware ingestion so head and
projection changes remain atomic.

The typed Nostr boundary does not prove BUD-02 upload completion. Every media-bearing caller must
obtain successful Blossom upload evidence before signing or publishing; byte-verified descriptors
alone prove only local descriptor-to-byte agreement. Typed outbox persistence and upload-evidence
bridging remain separate runtime responsibilities.

### Calendar Trust Layers

Kinds `31922`, `31923`, `31924`, and `31925` have three explicit,
non-interchangeable layers:

| Layer | Public role | What success establishes |
| --- | --- | --- |
| bounded structural wire or envelope | preserves the complete NIP-01 event while validating wire shape, identifier syntax, and resource limits | structural data only; it does not establish a matching event id or valid Schnorr signature unless the caller invokes the separate verification operations |
| tolerant NIP-52 parse | one of the `RadrootsParsedNip52*` date event, time event, calendar collection, or RSVP types | the expected kind and the pinned baseline NIP-52 semantics parse successfully; observed standard fields and tolerated wire spellings remain distinguishable from canonical authored data |
| strict Radroots admission | one of the corresponding `RadrootsAdmitted*` calendar types | the parsed value also satisfies the canonical Radroots identifier, reference, metadata, media, date, or UTC-day profile applicable to that kind |

The calendar `*_parsed_from_event` helpers construct a structurally checked raw envelope alongside
the tolerant projection. Their names do not mean that the event id or signature has been verified.
Before a caller treats relay data as accepted, it must recompute and compare the NIP-01 event id,
verify the Schnorr signature against the event author, dispatch the event to the matching kind
`31922`, `31923`, `31924`, or `31925` parser, and then apply strict admission when the Radroots
profile is required.
The kind-specific parsers reject the wrong kind, but neither baseline parsing nor strict admission
performs cryptographic verification. An admitted model is therefore valid only while it remains
bound to the already id- and signature-verified envelope from which it was parsed.

Strict authoring is the outbound counterpart, not a fourth inbound verification state.
`RadrootsAuthoredCalendarDateEvent`, `RadrootsAuthoredCalendarTimeEvent`,
`RadrootsAuthoredCalendar`, and `RadrootsAuthoredCalendarEventRsvp` have private checked fields and
encode deterministic `RadrootsNip01EventWireParts`. Wire parts contain only `kind`, `content`, and
`tags`; the owning runtime still supplies `created_at` and author identity, computes the event id,
signs the event, and publishes it.

### Baseline NIP-52 Parse

Kinds `31922` and `31923` use plain-text description content. Empty content projects to no typed
description. The `d`, `title`, and `start` tags are required singletons, and `end` is an optional
singleton with an exclusive boundary. The tolerant common projection covers the pinned standard
fields:

- optional `summary`, `image`, and `g`
- repeated `location` values without collapsing them to one value
- repeated participant `p` tags containing a 32-byte hexadecimal public key, optional recommended
  relay URL, and optional role
- repeated category `t` tags and absolute-URI reference `r` tags
- repeated calendar-inclusion request `a` tags whose coordinates identify kind `31924`, with an
  optional recommended relay URL
- the deprecated singleton `name` as observed compatibility data; it never replaces the required
  `title`

Other tags remain available only through the complete structural envelope. Baseline parsing does
not silently promote unknown fields into the typed calendar contract.

Kind `31922` uses semantic Gregorian `YYYY-MM-DD` values for inclusive `start` and optional
exclusive `end`. Year zero, impossible dates, non-canonical date spellings, and an `end` that is not
later than `start` are invalid. When `end` is absent, the event ends on the same date as `start`.
Uppercase `D` is not defined for date-based events by the pinned NIP-52. The tolerant parser retains
every observed uppercase-`D` tag as uninterpreted extension data so standards-compatible inspection
remains lossless at that boundary; strict Radroots admission rejects any such date-event extension.

Kind `31923` uses unsigned decimal Unix seconds for inclusive `start` and optional exclusive `end`.
When `end` is absent, the event is instantaneous. At least one uppercase-`D` day index on which the
event takes place is required, and the tolerant parser rejects indices outside the event interval.
In line with NIP-52's `SHOULD` for multiple coverage tags, baseline parsing does not require either
the start-day index or complete coverage. It also preserves duplicate, unordered, or leading-zero
numeric `D` observations for
diagnostics; those spellings are parser tolerance, not canonical authored output. Baseline parsing
does not apply the Radroots 366-day admission limit. Optional `start_tzid` and `end_tzid` values must
be exact identifiers in the bundled IANA Time Zone Database (`jiff-tzdb` `0.1.8`, TZDB `2026c`).
When `end_tzid` is absent and `start_tzid` is present, the effective end time zone is `start_tzid`.

An inbound baseline `image` is only a structurally valid absolute URI. It is not a Blossom claim,
an approved reference, or evidence about bytes or network state.

Kind `31924` is a calendar collection, not a generic list or list-set publication surface. Its bounded
plain-text `content` is the NIP-52 detailed description and is required on wire even when empty.
The required singleton `d` and `title` tags identify and name the collection. Repeated `a` tags
contain only kind-`31922` or kind-`31923` addressable coordinates and may each carry their own
recommended relay URL. A collection with no `a` tags is valid. The optional singleton
`description` and `image` tags are NIP-51 list metadata; `description` is distinct from NIP-52
description content and the parser does not merge one into the other. The baseline parser accepts a
structurally valid absolute `image` URI without making a Blossom or network claim. Singleton text
tags have exactly two elements; collection `a` tags have exactly two elements plus an optional relay
element.

Kind `31925` is an RSVP with bounded optional free-form note content. It has exactly one required
`d` identifier, one required `a` coordinate for a kind-`31922` or kind-`31923` event, and one
required `status` value: `accepted`, `declined`, or `tentative`. Optional singleton `e`, `fb`, and
`p` tags respectively identify one exact event revision, the `free` or `busy` availability state,
and the event author. The `a`, `e`, and `p` references each carry an independent optional
recommended relay URL; a relay hint on one reference is never copied to another. The `p` form is
an author hint without participant-role semantics. The `d`, `status`, and `fb` tags have exactly
two elements; the `a`, `e`, and `p` tags have exactly two elements plus an optional relay element.
Baseline parsing preserves `fb` observed on a declined RSVP for diagnostics but treats its effective
value as absent, as required by NIP-52. Parsing a syntactically valid `e` tag does not establish
that it is a revision of the referenced addressable event.

### Strict Radroots Calendar Profile

Strict authored and admitted calendar metadata uses canonical nonempty text, canonical participant
values, validated lowercase-scheme `ws`/`wss` relay URLs, and lowercase geohashes. Relay URL host,
port, path, and query spellings are preserved rather than normalized. The authored common surface supports repeated locations,
optional geohash and summary, repeated participants, categories, absolute-URI references, and
kind-`31924` calendar-inclusion requests. It intentionally does not author deprecated `name` tags.

Strict kind-`31922` events never emit or admit uppercase `D`. Strict kind-`31923` events require
canonical decimal timestamps and the exact, ascending, duplicate-free sequence of every UTC-day
index covered by the interval, where `D = floor(unix_seconds / 86400)` and `end` is exclusive.
Authoring derives this sequence rather than accepting it from callers. Strict authored and admitted
time events cover at most 366 UTC days.

Strict kind-`31924` and kind-`31925` identifiers are syntax-valid 128-bit values encoded as exactly
22 unpadded base64url characters. This shape does not prove uniqueness; an authoring runtime must
generate a fresh identifier for every collection or RSVP identity. Strict collection authoring and
admission require canonical title and optional NIP-51 description text, validated relay hints, and
duplicate-free event coordinates while still permitting an empty collection. Collection event
references remain limited to kinds `31922` and `31923`.

Strict RSVP authoring and admission require canonical `a`, optional `e`, and optional `p`
references. An admitted `p` author hint must match the public key in the `a` coordinate. Authored
RSVPs never emit `fb` when status is `declined`; inbound baseline and admitted values retain such an
observation only for diagnostics and return no effective free/busy state. Neither baseline parsing
nor strict admission proves that a referenced revision exists, that it matches the addressable
event, or that the RSVP author is authorized to answer for another party.

Calendar images have an explicit progression of trust. Tolerant inbound parsing accepts any
absolute image URI. Strict inbound admission requires a structural Blossom hash-path URL, but does
not prove approved-reference policy, byte agreement, upload completion, reachability, or image
safety. Strict authored models accept only `RadrootsAuthoredImage`, which wraps an approved,
byte-verified Blossom descriptor declared as `image/*`. That typestate proves local
descriptor-to-byte agreement only.

A publication runtime must not sign or publish a media-bearing calendar draft until the exact blob
has completed a successful BUD-02 upload and the runtime's bounded retrievability check has
succeeded. Consumers remain responsible for bounded retrieval, redirects, content decoding, and
format-safety policy. No calendar model upgrades an observed relay URL into either an upload receipt
or an availability guarantee.

Product routing uses surface-specific kind classifiers rather than a broad public-social set. Home,
Events, Market, Map, and Profile public-content candidates are explicit. A kind-`30402` value alone
does not select FoodAvailability: raw marker partitioning first distinguishes focused,
operational, generic NIP-99, and ambiguous candidates, and only signature-verified focused admission
returns the FoodAvailability contract. Report kind `1984` is a moderation/admin candidate, not
normal feed content. Relay and HTTP auth kinds are transient and excluded from durable social and
farm-ops candidate sets. Private farm operations candidates include the farm workspace manifest,
farm CRDT change envelope, farm file metadata, and the supported NIP-29 group event subset.

`RadrootsRelayList` is not a separate model type in the target contract. Operational listing records
are represented through `RadrootsOperationalListing`, and NIP-51 standard and list-set entries,
including NIP-65 relay metadata kind `10002`, are represented through `RadrootsList`. NIP-51
taxonomy may classify kind `31924` as a calendar list, but generic list and list-set decoding or
authoring must reject that kind; only the calendar-specific model and codec may parse or publish it.

## Exclusions

This substrate does not include `RadrootsFeedItem`, `RadrootsMapPin`, NIP-72 community events,
checkout or payment events, or public task, harvest, work-session, approval, or other Field business
document event types.

Task records, work sessions, harvest records, approvals, and similar Field business objects remain
CRDT document semantics carried inside the CRDT change envelope unless a later contract explicitly
promotes them.

## Consumer Boundary

The public social surface is event and codec substrate first. Consumer packages may wrap these
models and codecs, but this repository owns only the core Rust contracts and deterministic
conformance evidence. Package-specific operation maps, bindings, and generated artifacts are outside
this contract boundary.

## Conformance Boundary

Every new social codec and every upgraded existing social codec must have deterministic valid and
invalid conformance vectors before closeout. The FoodAvailability profile vector assigns exact
valid and invalid case kinds to strict authoring, verified projection, combined verification and
admission, and revision validation. Other upgraded vectors must include the strict comment,
reaction, operational-listing, farm, list, and list-set behavior whose public contract changes
during the refactor.

Social vectors are repo-owned and synthetic. They must not depend on application relay state, local
databases, external services, root fixture catalogs, or ambient machine state.
