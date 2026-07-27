# radroots-event

This is the README for `radroots_event`, which provides typed `radroots` event
models, kinds, and tag conventions for the `radroots` core libraries.

## Overview

 * typed content modules for accounts, app data, comments, coops, documents,
   farms, farm workspaces, farm CRDT changes, farm files, groups, auth events,
   jobs, lists, messages, posts, profiles, reactions, trades, and related
   domains;
 * shared event references, pointers, and kind and tag definitions used across
   event-processing code;
 * portable event model semantics for both `std` and `no_std` builds;
 * optional integration with `serde` for serialization.

The Profile module exposes the exclusive strict authored model. It requires a
non-whitespace, control-free name; its media fields accept only image-typed,
byte-verified Blossom descriptors; and its NIP-05 identifier type validates
syntax without making a network identity claim. The legacy read projection is
not serializable or exported as a DTO. Tolerant reads use
`RadrootsInboundProfileMetadata` from `radroots_event_codec`.

The post module keeps the legacy mutable `RadrootsPost` model as a compatibility
read projection only. New root kind-1 publication uses private-field
`RadrootsAuthoredUpdate`, `RadrootsAuthoredPhotoUpdate`, and
`RadrootsAuthoredAsk` types. Photo and optional Ask media require nonzero
dimensions, bounded alt text, approved same-digest fallbacks, and an
image-typed byte-verified Blossom descriptor. That descriptor state is not an
upload receipt; BUD-02 completion remains a runtime prerequisite before
signing.

The shared relay-hint module exposes `RadrootsNostrRelayHint` for NIP-10 Reply
and NIP-22 Comment references. It is a byte-stable subset of WebSocket URLs:
exact lowercase `ws://` or `wss://`, visible ASCII, canonical lowercase DNS or
four-octet IPv4 or bracketed pure-hex RFC 5952 IPv6, canonical optional port
`1..65535`, and RFC 3986 path-abempty/query syntax with uppercase `%HH`
escapes. It rejects IDNA and percent-encoded hosts, legacy IPv4, userinfo,
fragments, controls, backslashes, and normalization-dependent spellings.

The Reply module exposes opaque `RadrootsNip10ReplyReference` and
`RadrootsAuthoredNip10Reply` types for strict direct and nested kind-1
authoring. References carry a validated event id, referenced author, and
optional shared relay hint; construction emits either one marked root or
distinct marked root and parent references. Relay-hint syntax is not a
wire-size claim: Reply construction separately enforces the 4,096-byte
tag-element ceiling. These values prove syntax and authored shape, not target
existence, target kind, signature, author, or relay availability.

The Comment module implements the strict Radroots
[NIP-22](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/22.md)
kind-`1111` profile. `RadrootsAuthoredNip22Comment` and its opaque event-root,
address-root, parent, position, and root-kind values admit only kind-`30402`,
kind-`31922`, or kind-`31923` event or address roots. External `I`/`i`
references and kind-`1` roots are unsupported. The authored model has no Serde
construction path.

Canonical authoring emits `E,K,P,e,k,p` for a top-level event root,
`A,K,P,a,e,k,p` for a top-level address root, or `E,K,P,e,k,p` and
`A,K,P,e,k,p` for nested event and address roots. Event references always
contain four elements, including an empty relay position when no hint exists
and a final author hint. Address and participant references contain two
elements plus an optional relay; an address root's current-revision `e` tag has
no author hint. A direct `k` repeats the root kind and a nested `k` is `1111`.

The event-contract registry v7 classifies `radroots.social.comment.v1` as
`TypedOnly` and `AdmissionOnly`; serialized registry versions `1` through `6`
are stale. Generic kind-`1111` draft and signing paths cannot claim the typed
contract. The Comment resource profile limits content to 131072 UTF-8 bytes,
tags to 1024, total tag elements including names to 4096, each element to 4096
bytes, aggregate tag bytes to 131072, and compact signed wire JSON to 262144
bytes. `RadrootsInboundNip22CommentProjection` and
`RadrootsAdmittedNip22CommentEvent` provide verified inbound projection and
admission. The three governed Comment operations are
`social.comment.build_authored_draft`,
`social.comment.project_verified_event`, and
`social.comment.verify_and_admit_event`; they and the canonical self-contained
114-case corpus are owned by `radroots_event_codec` and
`contracts/conformance`.

The Deletion module implements the effect-free request layer of
[NIP-09](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/09.md).
`RadrootsAuthoredNip09DeletionRequest` requires at least one validated event-id
or replaceable/addressable coordinate target. Event targets carry a
caller-asserted kind advisory in `0..=65535`; this is metadata rather than
proof of the target event. Address coordinates accept NIP-01 replaceable kinds
`0`, `3`, and `10000..=19999` only with an empty identifier, and addressable
kinds `30000..=39999` with an opaque identifier.

Construction canonicalizes event targets by event id, address targets by
coordinate, and unique derived kind advisories in ascending order. Duplicate
normalized targets are rejected. The request enforces the shared content, tag,
element, aggregate-tag-byte, and compact signed-event budgets before it can
reach signing. It represents only a kind-`5` protocol request: it does not
retrieve a target, prove same-author authority, compute an address cutoff,
suppress content, mutate a store, or make a deletion request itself deletable.

The immutable registry-v7 inventory and addressable-feed-v1 head functions are
historical protocol inputs to event-store reconciliation v1. The explicit
`event_contract_registry_v7`, `validate_event_contract_registry_v7`,
`event_head_candidate_for_nip01_event_v1`, and `select_event_head_v1`
entrypoints must retain their v7/v1 behavior when a later current registry or
head algorithm is introduced.

Kind `30402` has a raw, allocation-free marker partition before profile-specific
tag-shape validation. Presence of `radroots:price_unit` or `radroots:quantity` selects
the focused FoodAvailability marker family; presence of
`radroots:primary_bin`, `radroots:bin`, or `radroots:price` selects the richer
Operational Listing marker family. Focused-only, operational-only, marker-free
generic NIP-99, and mixed-marker events produce
`RadrootsClassifiedListingPartition::{FocusedFoodAvailability,
OperationalListing, GenericNip99, Ambiguous}` respectively. A malformed
one-element tag still contributes its raw first name, and marker matching is
case-sensitive. `classify_classified_listing_tags` and the borrowed-slice
variant inspect neither kind, tag values, nor tag arity.

The FoodAvailability module provides `RadrootsFoodAvailabilityDetails` and
checked identifier, text, publication timestamp, price, currency, unit,
quantity, status, image-dimension, and image values. Content contains at least
one scalar outside Unicode whitespace and U+001C through U+001F, and is bounded
to 131072 UTF-8 bytes. Identifiers reject whitespace plus Unicode control and
format characters; title, summary, and location use trimmed, nonempty,
control-free text bounded to 4096 UTF-8 bytes. Food units are closed to `g`,
`kg`, `lb`, `oz`, `each`, `dozen`, `bunch`, `punnet`, `bag`, and `basket`.
Price permits zero; quantity is strictly positive and uses the price unit.
Image dimensions use two nonzero canonical `u32` decimal values in
`WIDTHxHEIGHT` form. Details accept at most 64 images, require unique image URLs
and Blossom digests, and accept only `RadrootsAuthoredImage` values that already
prove local descriptor-to-byte agreement. Details retain nonzero `published_at`
and can validate that it is not later than a supplied `created_at`.

These checked details are the exclusive typed input to the focused
`radroots.food.availability.v1` authoring contract. The event-contract registry
classifies that kind-`30402` profile as `TypedOnly` and `AdmissionOnly`, so a
generic unsigned classified listing cannot claim the focused contract. The
details themselves remain domain values rather than signed events;
`radroots_event_codec` owns deterministic wire construction, verified inbound
projection and admission, and strict revision comparison.

An authored image proves local descriptor-to-byte agreement only. Successful
BUD-02 upload completion and any required raster, retrieval, or availability
checks remain runtime responsibilities before signing. Neither this domain
module nor the registry signs, publishes, replicates, or retrieves an event.

The calendar module keeps three different states explicit for NIP-52 kinds
`31922`, `31923`, `31924`, and `31925`: the complete structural event envelope,
a tolerant baseline NIP-52 projection, and a strict Radroots-admitted
projection. The authored types are `RadrootsAuthoredCalendarDateEvent`,
`RadrootsAuthoredCalendarTimeEvent`, `RadrootsAuthoredCalendar`, and
`RadrootsAuthoredCalendarEventRsvp`; their inbound counterparts use matching
`RadrootsParsedNip52*` and `RadrootsAdmitted*` types. Envelope construction
validates structure, not a matching event id or Schnorr signature. Callers must
perform those cryptographic checks independently and keep any parsed or
admitted value bound to the verified envelope and expected kind.

Baseline projections retain the pinned NIP-52 common fields: repeated
locations, participants, categories, absolute-URI references, kind-`31924`
calendar-inclusion requests, and deprecated `name` compatibility data in
addition to `d`, `title`, description, summary, image, and geohash. Date
events use semantic Gregorian dates and retain observed uppercase-`D` tags as
uninterpreted extensions. Time events validate unsigned timestamps, exact IANA
time-zone identifiers, and at least one in-range `D` day while tolerating the
NIP's non-mandatory start-day and complete-coverage forms. An absent `end_tzid`
falls back to `start_tzid` when one is present.

Strict authoring and admission add canonical metadata and bounded resource
rules. Authored common fields include repeated locations, participants,
categories, references, and calendar-inclusion requests; deprecated `name` is
not authored. Strict date events reject uppercase `D`, while strict time
events derive or admit only the complete, ascending sequence of UTC-day
indices and cover at most 366 days.

Kind `31924` has one calendar-specific contract. It is not decoded or authored
through either generic NIP-51 list codec. Its NIP-52 detailed description is
plain-text event content and remains distinct from the optional NIP-51
`description` tag. It requires one `d` and one `title`, permits optional NIP-51
`description` and `image` tags, and contains zero or more `a` references to
kind `31922` or `31923` events. Each reference may have its own relay hint, and
an empty calendar collection is valid.

Kind `31925` has exactly one required `d`, one `a` event coordinate, and one
`status`; `e`, `fb`, and `p` are optional singletons. The `a`, `e`, and `p`
references preserve independent optional relay hints. The `p` tag is an event
author hint without participant-role semantics, and strict admission requires
it to match the author in the `a` coordinate. An inbound declined RSVP may
retain an observed `fb` for diagnostics but exposes no effective free/busy
state. Authored declined RSVPs cannot carry `fb`.

Strict collection and RSVP identifiers use exactly 22 unpadded base64url
characters representing 128 bits. The type proves only syntax; the runtime is
responsible for generating a fresh value for each new identity. Parsing or
admission does not prove reference existence, revision correspondence, RSVP
authority, upload completion, or network availability.

Inbound images begin as unverified absolute URIs. Strict admission, including
kind-`31924` collection images, requires a structural Blossom hash-path URL but
makes no byte or network claim. Authored calendar images require the shared
`image/*`, byte-verified Blossom descriptor. That state is not an upload
receipt: a runtime must require successful BUD-02 upload completion and a
bounded retrievability check before signing or publishing a media-bearing
event.

## Field Event Boundary

`radroots_event` includes the public event-layer models needed by Field-style
farming operations:

 * workspace manifests for discovering the farm group, relay set, media servers,
   and supported event kinds;
 * CRDT change envelopes for operation documents such as tasks, work sessions,
   harvest records, and approvals;
 * farm file metadata events for media attached to farm documents;
 * NIP-42 relay auth and NIP-98 HTTP auth payload models;
 * NIP-29 group metadata, member lists, roles, invites, joins, leaves, and user
   operations for the supported `9000`, `9001`, `9002`, `9005`, `9007`, `9008`,
   `9009`, `9021`, `9022`, `39000`, `39001`, `39002`, and `39003` subset.

The NIP-29 group surface uses bare metadata marker tags such as `private`,
`restricted`, `hidden`, and `closed`, `supported_kinds` declarations, and
`code` tags for invite and join flows. User management and moderation events
preserve optional reason content. LiveKit room metadata and live participant
state are not part of this crate's current group event subset.

Task records, work sessions, harvest records, approvals, and similar Field
business objects are CRDT document semantics carried by
`RadrootsFarmCrdtChange`. They are not separate `rr-rs` event families and this
crate does not enforce private Field workflow authorization.

## Copyright

Except as otherwise noted, all files in the `radroots_event` distribution are

 Copyright (c) 2025 Tyson Lupul

For information on usage and redistribution, and for a DISCLAIMER OF ALL
WARRANTIES, see LICENSE included in the `radroots_event` distribution.
