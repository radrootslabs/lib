# radroots_nostr

This is the README for `radroots_nostr`, which provides shared Nostr protocol
primitives for the `radroots` core libraries.

## Overview

 * typed filters, tags, events, relay metadata, parsers, and utility helpers;
 * feature-gated client operations and relay-management helpers for active
   network use;
 * adapters between `radroots_event`, `radroots_event_codec`, and Nostr wire
   representations;
 * strict BUD-11 signed HTTP authorization adapters behind the `blossom`
   feature;
 * optional NIP-11 and NIP-17 support across feature-gated builds.

The `blossom` feature signs kind-24242 authorization events and encodes or
verifies their `Authorization: Nostr` HTTP values. It does not publish these
ephemeral authorization events to relays. Pure BUD-11 claim parsing and policy
validation remain in `radroots_blossom`.

The `events` feature is std-backed. With it, kind-1 root publication is
available only through typed Update, PhotoUpdate, and Ask builders backed by
the strict `radroots_event_codec` wire operations. NIP-10 Reply publication is
separately available through the typed direct or nested Reply model and its
sealed builder. Authored Replies always emit marked `root` and optional
`reply` event references plus the required referenced-author `p` tags.
Verified inbound projection also accepts deprecated positional NIP-10
references for interoperability and preserves valid supplemental unmarked `e`
references as citations. Empty marker slots remain absent even when an optional
fifth-element author hint is present. Missing or malformed advisory
participant, middle citation, relay, and referenced-author metadata is retained
as ordered diagnostics instead of erasing an unambiguous Reply. A Reply remains
thread content and can never enter root-card admission. Reply admission proves
the Reply event's NIP-01 id and signature; it does not prove that a referenced
target exists, is kind `1`, or was authored by the declared referenced author.

Strict
[NIP-22](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/22.md)
Comment publication is separately exposed through
`RadrootsNostrNip22CommentEventBuilder`. Its only input is a checked
`RadrootsAuthoredNip22Comment`; callers may choose the timestamp and sign or
publish, but cannot mutate the kind, content, or canonical
root, parent, kind, and participant tags. The profile admits event or address
roots only for kinds `30402`, `31922`, and `31923`; it has no external
`I`/`i` or kind-`1` root surface.

Reply and Comment authoring and verified projection share the portable
`RadrootsNostrRelayHint` profile rather than the generic relay URL type. It
accepts only exact lowercase `ws://` or `wss://` visible-ASCII URLs with a
canonical lowercase DNS, four-octet IPv4, or bracketed pure-hex RFC 5952 IPv6
authority, an optional canonical port `1..65535`, and RFC 3986
path-abempty/query syntax using uppercase `%HH` escapes. Rejected inbound hints
remain verbatim in ordered raw-tag diagnostics. The hint profile does not own
the event boundary's separate 4,096-byte tag-element budget.

Inbound Comment projection is owned by `radroots_event_codec` and accepts only
an id-and-signature verified kind-`1111` envelope.
`RadrootsInboundNip22CommentProjection` retains the order-independent
authority projection, and `RadrootsAdmittedNip22CommentEvent` keeps it bound to
the verified envelope. Malformed optional relay or participant metadata
remains diagnostic, while cardinality, unsupported roots, coordinate
conflicts, and valid-but-conflicting author hints fail admission. Admission
does not prove any referenced target or relay. The registry-v7 contract is
`TypedOnly` for authoring and `AdmissionOnly` for matching, with registry
versions `1` through `6` stale.

Strict
[NIP-09](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/09.md)
deletion-request publication is exposed through the sealed
`RadrootsNostrNip09DeletionRequestEventBuilder`. Its only input is a checked
`RadrootsAuthoredNip09DeletionRequest`; callers may choose the timestamp and
sign or publish, but cannot mutate the kind, content, or canonical `e`, `a`,
and derived `k` tags. Generic kind-5 builders are rejected before signer
access. Admission proves only the signed deletion-request event and its typed
projection. It does not prove target authorship, existence, deletion
authorization, applicability, relay handling, or any deletion effect.

The former free-form text-note post builder is removed. Generic protocol
builders reject kind-0 Profile events, every kind-1 event, every kind-5
deletion request, and kind-1111 Comments at both direct signing and
client-publication boundaries before signer access. Typed media builders can
sign or publish only after the owning runtime separately proves successful
BUD-02 upload completion; their byte-verified descriptors do not attest upload
completion. The generic net manager intentionally exposes no direct
PhotoUpdate or media Ask publisher.

The complete governed Comment operation namespace remains
`social.comment.build_authored_draft`,
`social.comment.project_verified_event`, and
`social.comment.verify_and_admit_event`. The typed builder and client
publication surface consume that strict contract; they do not add a fourth
wire operation. Comment inputs retain the 131072-byte content, 1024-tag,
4096-total-element, 4096-byte element, 131072-byte aggregate tag, and
262144-byte compact signed-wire ceilings proven by the canonical
self-contained 114-case conformance corpus.

Focused FoodAvailability kind-30402 authoring likewise uses a sealed builder.
Its `created_at` is fixed during strict construction and cannot be changed
after wire validation. Generic direct signing and client publication reject
focused or mixed FoodAvailability/Operational Listing markers before signer
access; marker-free NIP-99 and operational-only compatibility builders remain
available. Relaying an already signed event remains a transport operation and
does not establish typed FoodAvailability authoring.

The opaque generic-builder policy governs Radroots builder signing and client
publication. It does not redefine the standard NIP-46 `sign_event` method or a
signer backend's externally supplied unsigned-event operation. Those are
explicit low-level Nostr interoperability boundaries; their outputs carry no
Radroots typed product-authoring claim and are not product authoring entry
points.

Generic protocol events that require an external custody provider finalize
through `RadrootsNostrExternalSigningRequest`. The opaque request is available
without the relay-client feature and serializes as a standard unsigned Nostr
event only after generic typed-authoring reservations pass. It accepts a
returned event only when the author and canonical event id match the request
and the complete NIP-01 event verifies. It exposes no raw mutable builder,
unsigned-event conversion, or unchecked deserialization path.

Strict kind-0 Profile publication uses
`RadrootsNostrProfileEventBuilder`, constructed only from
`RadrootsAuthoredProfile`. The sealed wrapper permits timestamp selection and
local signing or client publication, but no raw kind, content, or tag
mutation. A media-bearing Profile still requires runtime-owned proof of
successful BUD-02 upload before it reaches this authoring boundary.

## Portable relay-client lifecycle

With the `client` feature, callers can subscribe and publish to selected relay
sets and explicitly unsubscribe through Radroots types. The wrapper does not
create detached listeners or take ownership of subscription lifetime. The
`client,events` feature combination is qualified for native targets and
`wasm32-unknown-unknown`.

## Copyright

Except as otherwise noted, all files in the `radroots_nostr` distribution are

 Copyright (c) 2025 Tyson Lupul

For information on usage and redistribution, and for a DISCLAIMER OF ALL
WARRANTIES, see LICENSE included in the `radroots_nostr` distribution.
