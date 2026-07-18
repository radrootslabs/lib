# Public Social Event Substrate

Status: active implementation contract

Scope: public Radroots social Nostr event models, codecs, and deterministic conformance vectors in
this repository.

## Purpose

The public social event substrate extends the Radroots event family beyond profile, farm, listing,
and trade workflows while keeping relay runtime behavior, application projections, moderation
services, and private Field business documents outside this repository's event-contract boundary.

The target implementation is standards-first and Radroots-named. Event models live in
`radroots_event`, canonical encode/decode behavior lives in `radroots_event_codec`, and
deterministic fixtures live under `contracts/conformance`.

Calendar behavior is based on
[NIP-52](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/52.md)
and its collection metadata uses
[NIP-51](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/51.md),
both at NIPs commit `bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91`. Calendar media uses the public
Blossom primitives governed by the protocol pin in
[`blossom-media.md`](blossom-media.md). The upstream NIP-52 rules and the stricter Radroots
authoring and admission profile are separate contract layers.

## Implementation Inventory

The repository implements strict authored and verified-projected kind `1` post profiles, kind `1111`
`RadrootsComment`, kind `7` `RadrootsReaction`, generic `RadrootsList` entries, stable listing
records through `RadrootsListing`, articles, generic public file metadata, calendar date events,
calendar time events, reposts, generic reposts, calendar collections, RSVP events, and reports.

The closeout contract requires:

- complete model and codec coverage for the approved public social event families
- kind and tag constants for the approved NIP surface
- ordinary kind-1 compatibility reads plus strict Update, PhotoUpdate, and Ask authoring
- strict NIP-22 `RadrootsComment` behavior without legacy `e_root` or `e_prev` fallback tags
- strict NIP-25 `RadrootsReaction` behavior where empty content is a valid like
- explicit optional `published_at` support for NIP-99 listing parity
- NIP-65 relay-list validation evidence through `RadrootsList`
- conformance vectors and canonical-event witnesses for every new or upgraded social event family

## Approved Event Families

The MVP public social substrate includes:

- strict `RadrootsAuthoredUpdate`, `RadrootsAuthoredPhotoUpdate`, and
  `RadrootsAuthoredAsk` publication types plus verified tolerant projection for
  ordinary NIP-01 kind `1` events
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
- stable listing kind `30402` validation through `RadrootsListing`
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
canonical lowercase `image/*`; dimensions are nonzero `u32` values; size is a
nonzero `u64`; alt text is non-whitespace and at most 4092 UTF-8 bytes.

Every authored primary image is a `RadrootsAuthoredImage` backed by an approved,
byte-verified Blossom descriptor. Every authored fallback is an approved
Blossom hash-path URL with the same digest. This typestate proves local
descriptor-to-byte agreement only. Successful BUD-02 upload completion remains
a separate runtime precondition before signing.

Ask is kind `1` and deterministically emits exactly
`["t","radroots-ask"]`. PhotoUpdate is also kind `1`; kind `20` is outside
this contract. Update emits neither the Ask marker nor `imeta`.

Inbound projection accepts only a `RadrootsSignatureVerifiedEvent`. Any `e`
tag selects the reply exclusion before Ask or media inspection; strict NIP-10
reply parsing remains separately owned. For roots, exactly one two-element Ask
marker after ASCII whitespace trim and ASCII case folding selects Ask. Multiple
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
Events, Market, Map, and Profile public-content candidates are explicit. Active listing kind `30402`
can appear in public product surfaces. Report kind `1984` is a moderation/admin candidate, not
normal feed content. Relay and HTTP auth kinds are transient and excluded from durable social and
farm-ops candidate sets. Private farm operations candidates include the farm workspace manifest,
farm CRDT change envelope, farm file metadata, and the supported NIP-29 group event subset.

`RadrootsRelayList` is not a separate model type in the target contract. Stable listings are
represented through `RadrootsListing`, and NIP-51 standard and list-set entries, including NIP-65
relay metadata kind `10002`, are represented through `RadrootsList`. NIP-51 taxonomy may classify
kind `31924` as a calendar list, but generic list and list-set decoding or authoring must reject that kind;
only the calendar-specific model and codec may parse or publish it.

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
invalid conformance vectors before closeout. Upgraded vectors must include the strict comment,
reaction, listing, farm, list, and list-set behavior whose public contract changes during the
refactor.

Social vectors are repo-owned and synthetic. They must not depend on application relay state, local
databases, external services, root fixture catalogs, or ambient machine state.
