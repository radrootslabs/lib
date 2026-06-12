# Public Social Event Substrate

Status: active implementation contract

Scope: public Radroots social Nostr event models, codecs, wasm builders, and deterministic
conformance vectors in this repository.

## Purpose

The public social event substrate extends the Radroots event family beyond profile, farm, listing,
and trade workflows while keeping relay runtime behavior, application projections, moderation
services, and private Field business documents outside this repository's event-contract boundary.

The target implementation is standards-first and Radroots-named. Event models live in
`radroots_events`, canonical encode/decode behavior lives in `radroots_events_codec`, optional JSON
to tags helpers live in `radroots_events_codec_wasm`, and deterministic fixtures live under
`spec/conformance`.

## Implementation Inventory

The repository implements public social support for kind `1` `RadrootsPost`, kind `1111`
`RadrootsComment`, kind `7` `RadrootsReaction`, generic `RadrootsList` entries, listing draft kind
`30403` through `RadrootsListing`, articles, generic public file metadata, calendar date events,
calendar time events, reposts, generic reposts, calendar collections, RSVP events, and reports.

The closeout contract requires:

- complete model and codec coverage for the approved public social event families
- kind and tag constants for the approved NIP surface
- `RadrootsPost` preservation for optional social metadata
- strict NIP-22 `RadrootsComment` behavior without legacy `e_root` or `e_prev` fallback tags
- strict NIP-25 `RadrootsReaction` behavior where empty content is a valid like
- explicit optional `published_at` support for NIP-99 listing parity
- NIP-65 relay-list validation evidence through `RadrootsList`
- conformance vectors and canonical-event witnesses for every new or upgraded social event family

## Approved Event Families

The MVP public social substrate includes:

- `RadrootsPost` for ordinary NIP-01 kind `1` notes plus optional Radroots social metadata
- `RadrootsArticle` for NIP-23 kind `30023` long-form content
- generic public `RadrootsFileMetadata` for NIP-94 kind `1063`
- `RadrootsCalendarDateEvent` for NIP-52 kind `31922`
- `RadrootsCalendarTimeEvent` for NIP-52 kind `31923`

The production-v1 public social substrate includes:

- `RadrootsRepost` for NIP-18 kind `6`
- `RadrootsGenericRepost` for NIP-18 kind `16`
- `RadrootsCalendar` for NIP-52 kind `31924`
- `RadrootsCalendarEventRsvp` for NIP-52 kind `31925`
- `RadrootsReport` for NIP-56 kind `1984`
- listing draft kind `30403` validation through `RadrootsListing`
- relay-list kind `10002` validation through `RadrootsList`

## Contract Decisions

`RadrootsPost` remains compatible with ordinary kind `1` text notes. Content-only notes must remain
valid. Optional farm or address references, media metadata, geohash, topics, and quote references
must be preserved when present and must use serde defaults so existing simple JSON fixtures remain
valid.

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

`RadrootsCalendarDateEvent`, `RadrootsCalendarTimeEvent`, and `RadrootsCalendar` use NIP-52
description content. Optional `description` data is encoded as event content and empty content
decodes to no description. Calendar date events use lowercase `d` for the replaceable identifier and
optional uppercase `D` tags for covered all-day dates. Calendar time events require at least one
uppercase `D` tag so timestamped events retain a deterministic calendar-date anchor across codecs and
language exports.

Product routing uses surface-specific kind classifiers rather than a broad public-social set. Home,
Events, Market, Map, and Profile public-content candidates are explicit. Active listing kind `30402`
can appear in public product surfaces, but listing draft kind `30403` is limited to draft-owner
contexts. Report kind `1984` is a moderation/admin candidate, not normal feed content. Relay and HTTP
auth kinds are transient and excluded from durable social and farm-ops candidate sets. Private farm
operations candidates include the farm workspace manifest, farm CRDT change envelope, farm file
metadata, and the supported NIP-29 group event subset.

`RadrootsListingDraft` and `RadrootsRelayList` are not separate model types in the target contract.
Listing draft kind `30403` is represented through `RadrootsListing`, and NIP-51 standard and
list-set entries, including NIP-65 relay metadata kind `10002`, are represented through
`RadrootsList`.

## Exclusions

This substrate does not include `RadrootsFeedItem`, `RadrootsMapPin`, NIP-72 community events,
checkout or payment events, or public task, harvest, work-session, approval, or other Field business
document event types.

Task records, work sessions, harvest records, approvals, and similar Field business objects remain
CRDT document semantics carried inside the CRDT change envelope unless a later contract explicitly
promotes them.

## SDK Boundary

The public social surface is event and codec substrate first. Curated SDK operation metadata
promotes the MVP social tag-builder surface after the corresponding Rust models, codecs, wasm
helpers, and conformance vectors exist. Production-v1 repost, report, calendar collection, and RSVP
behavior remains substrate-visible by default unless a consumer proves that it should be promoted
into the curated operation surface.

`radroots_events_codec_wasm` exposes the canonical JSON-to-tags helper names `post_tags`,
`comment_tags`, `reaction_tags`, `article_tags`, `file_metadata_tags`,
`calendar_date_event_tags`, `calendar_time_event_tags`, `calendar_tags`,
`calendar_event_rsvp_tags`, `repost_tags`, `generic_repost_tags`, and `report_tags` for the public
social substrate. The same wasm crate exposes `farm_workspace_manifest_tags`,
`farm_crdt_change_tags`, `farm_file_metadata_tags`, `relay_auth_tags`, and `http_auth_tags` for the
field event substrate.

## Conformance Boundary

Every new social codec and every upgraded existing social codec must have deterministic valid and
invalid conformance vectors before closeout. Upgraded vectors must include the strict comment,
reaction, listing, farm, list, and list-set behavior whose public contract changes during the
refactor.

Social vectors are repo-owned and synthetic. They must not depend on application relay state, local
databases, external services, root fixture catalogs, or ambient machine state.
