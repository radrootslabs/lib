# Event boundary matrix

Status: canonical
Scope: public event domains, kinds, and RPC method ownership

This matrix is the canonical open-source event-boundary source for the
applications and services that expose public Radroots events. It retains the
reviewed method and domain coverage while placing authority in this standalone
contract package.

## Runtime key rule

- Publish methods use the runtime-configured key as the author by default.
- List and get methods may accept optional author filters, but default to the runtime key.
- Multi-key publishing is out of scope until explicit key management is added.

## Method naming conventions

- `events.<entity>.<action>`
- `domains.<domain>.<subdomain>.<action>`
- standard actions: `publish`, `get`, `list`, `validate`, `encode`, `decode`

## General verified admission rule

`event.admit_verified` is the single trusted contract-admission boundary for a
`RadrootsSignatureVerifiedEvent`. It never accepts a bare envelope or performs
signature verification implicitly. Profile, root Post, Reply, Comment,
DeletionRequest, and focused FoodAvailability candidates retain their exact
typed admitted values. All other registered candidates must pass complete
registry shape validation before returning `ContractValidated`.

Kind `1` routing is ordered: root Post admission runs first, and only its exact
thread-excluded candidate may proceed to NIP-10 Reply admission. A thread-shaped
event that fails Reply admission is invalid rather than a generic root Post.
Kind `30402` routing partitions raw marker names before validation. Focused Food
is admitted through its typed profile; an excluded Operational Listing may
fall back to complete registry validation; marker-free generic NIP-99 remains
unsupported; mixed focused and operational markers remain an explicit
ambiguity error.

Unsupported kind/shape matching and contract/profile validation failures are
distinct `RadrootsEventAdmissionError` variants with stable codes. Successful
admission proves only the verified envelope and the selected product or
registry contract. It does not sign, publish, upload media, select a NIP-01
head, authorize a deletion, evaluate suppression, mutate storage, or establish
reference existence or relay availability.

## Calendar boundary rule

Calendar kinds `31922` through `31925` expose separate authored,
baseline-parsed, and strict-admitted types. A structural
`EventEnvelope` is not cryptographic verification, and neither the
NIP-52 parser nor Radroots admission verifies the declared event id or Schnorr
signature. A read-side runtime must keep the parsed or admitted value bound to
an envelope whose id and signature it has independently verified and whose kind
the corresponding parser accepted. Outbound authored models produce unsigned
wire parts and require runtime signing and transport.

## Classified-listing profile boundary rule

Kind `30402` is the standard NIP-99 classified-listing kind and
`ClassifiedListingAddress` is the exact coordinate authority for that
kind. `OperationalListing` is the richer
`radroots.operational_listing.published.v1` profile accepted at the same kind;
its typed codec, tags, and publication operations are exposed under the
`operational_listing` domain. The standard kind identity does not by itself
establish that an event satisfies the operational profile.

Raw tag-name presence partitions the standard kind before profile tag-shape
validation. `radroots:price_unit` and `radroots:quantity` are focused
FoodAvailability markers. `radroots:primary_bin`, `radroots:bin`, and
`radroots:price` are Operational Listing markers. Focused-only,
operational-only, marker-free generic NIP-99, and mixed-marker inputs classify
as `FocusedFoodAvailability`, `OperationalListing`, `GenericNip99`, and
`Ambiguous`; malformed tags still contribute their first element, and names
are case-sensitive. This central partition does not inspect kind, values, or
arity and does not validate either profile.

The focused `radroots.food.availability.v1` profile has four public operation
boundaries. Strict authored details plus `created_at` produce deterministic
unsigned kind-`30402` wire parts. Projection accepts only a
`RadrootsSignatureVerifiedEvent`, returns focused data or an explicit
Operational Listing or generic NIP-99 exclusion, rejects mixed markers as
ambiguous, and applies focused validation only after partitioning. Combined
admission performs NIP-01 id and signature verification before projection and
keeps either result bound to its verified envelope.

Strict authoring emits the exact ordered `d`, `title`, `summary`,
`published_at`, `location`, `price`, `radroots:price_unit`, optional
`radroots:quantity`, `status`, and repeated `image` tags. Inbound focused
projection normalizes accepted decimal and currency spellings, ignores
unowned optional NIP-99 tags, and preserves a bounded ordered image projection
with stable diagnostics instead of treating malformed image metadata as a
valid authored image. Focused capability tags for checkout, fulfillment,
payments, provenance, and operational workflows are rejected; the profile
does not gain those semantics by carrying an unknown tag.

Revision validation re-applies strict authored wire semantics to both verified
events. It requires stable kind, author, `d` coordinate, and `published_at`,
then applies NIP-01 replacement order: later `created_at` wins, and the lower
event id wins at equal time. Side-specific errors identify an invalid previous
or current candidate before revision comparison.

The `radroots_nostr` `events` feature seals strict FoodAvailability wire parts
behind a builder whose timestamp cannot be mutated after validation. It supports
local signing and typed client publication; generic authoring rejects focused
and mixed kind-`30402` profiles. Signed-event relay remains transport-only.
The non-default `legacy-ingest` replica feature verifies kind-`30402` NIP-01
events first, selects the raw addressable head before profile decoding, and
routes only Operational Listing into its trade-product projection. Selected
focused or generic exclusions and invalid or ambiguous rejections remove an
older operational projection and still advance the raw head. The head-only
replica helper rejects kind `30402`; these events require profile-aware legacy
ingestion so projection cleanup and head movement remain atomic. This
bare-envelope module is absent from default builds and is not a Phase 1 product
ingestion boundary. A future product replacement must accept only a
store-produced verified, valid-stream-eligible, currently visible admission.

A validated authored image proves local Blossom descriptor-to-byte agreement
only. Successful BUD-02 upload completion and any raster, retrievability, or
availability checks remain runtime responsibilities before signing or
publication.

## Kind-1 post and Reply boundary rule

Ordinary kind-1 events remain interoperable at the generic
`radroots.social.post.v1` read boundary. Product projection first requires a
`RadrootsSignatureVerifiedEvent`; any `e` tag excludes the event as a thread
candidate without asserting NIP-10 Reply validity, then root-card precedence is
Ask, PhotoUpdate, Update. The admission result keeps root product admission and
thread-excluded candidates in distinct public variants. Exact subtype registry
contracts are admission-only and cannot be selected by unsigned kind/tag
matching. New root publication uses the strict authored types and a sealed
Nostr builder with no raw tag/content mutation; generic builder publication
rejects every kind `1` event before signing. The legacy mutable `Post`
decoder is compatibility-only and has no authored encoder or tag-builder
implementation.

NIP-10 Reply is a separate typed kind-1 boundary. Strict direct authoring emits
one marked `root` event reference and its referenced-author `p` tag. Strict
nested authoring adds one distinct marked `reply` parent and its author,
deduplicating equal authors. Inbound Reply projection requires NIP-01
verification but tolerates both preferred marked and deprecated positional
NIP-10 references. Valid supplemental unmarked `e` references remain citations;
malformed supplemental references are ignored with ordered diagnostics.
Empty marker slots remain absent for positional root, parent, and citation
references even when a fifth-element author hint is present; malformed
intermediate citations do not erase valid positional anchors.
Advisory `p`, relay, and referenced-author metadata is projected best-effort
rather than promoted to a validity gate. Positional replies remain thread
enrichment, and a Reply carrying Ask or media metadata cannot be promoted as a
root card.

Authored and projected NIP-10 Reply and NIP-22 Comment relay hints share the
portable `NostrRelayHint` profile:
exact lowercase `ws://` or `wss://`, visible ASCII, canonical lowercase DNS or
four-octet IPv4 or bracketed pure-hex RFC 5952 IPv6, optional canonical port
`1..65535`, and RFC 3986 path-abempty/query syntax with uppercase `%HH`
escapes. The profile rejects userinfo, fragments, backslashes, IDNA and
percent-encoded hosts, legacy IPv4, and normalization-dependent spellings.
Malformed inbound hints stay verbatim in ordered raw-tag diagnostics. Relay
syntax validation is independent from the Reply boundary's 4,096-byte
tag-element budget.

Reply admission proves the Reply envelope's id, signature, and bounded NIP-10
structure. It does not prove that a referenced event exists, has kind `1`, has
a valid signature, was authored by the declared referenced author, or is
available from a relay. Signed-event relay remains generic transport rather
than a typed Reply-authoring boundary.

## NIP-22 Comment boundary rule

The strict Radroots
[NIP-22](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/22.md)
profile is kind `1111`. Its root is exactly one `E` event id or `A`
addressable coordinate, with matching singleton `K` root-kind and `P`
root-author authority. Supported root kinds are only classified listings
(`30402`), calendar date events (`31922`), and calendar time events (`31923`).
External `I`/`i` roots and parents and ordinary kind-`1` roots are outside this
profile.

Strict authoring exposes `AuthoredNip22Comment` for top-level event,
top-level address, and nested Comment positions. It emits exact ordered
`E,K,P,e,k,p` for a top-level event, `A,K,P,a,e,k,p` for a top-level
address, and `E,K,P,e,k,p` or `A,K,P,e,k,p` for a nested event or address
root. Authored event references have four elements with an explicit relay slot
and final author hint. Address and participant references have two elements
plus an optional relay; the current-revision `e` reference on a top-level
address Comment has no author hint. Direct `k` repeats the root kind, while
nested `k` is `1111`.

`RadrootsInboundNip22CommentProjection` accepts only an id-and-signature
verified event. It resolves authority independently of tag order and enforces
its cardinality, shape, canonical kind, coordinate-kind, author, and parent
relationships. Unknown tags, `q` tags, distinct unselected `p` mentions, and
the exact raw tags remain inspectable. Inbound NIP-22 reference event IDs,
public keys, and coordinate-author hex accept either ASCII hex case; typed
values normalize to lowercase while raw tags retain their original spelling.
Malformed advisory relay or participant metadata is retained as ordered
diagnostics, while valid hints that conflict with `P` or selected `p` authority
are hard failures.
`RadrootsAdmittedNip22CommentEvent` binds the result to the verified envelope;
it does not prove reference existence, target signatures, target authorship, or
relay availability.

The event-contract registry v7 classifies `radroots.social.comment.v1` as
`TypedOnly` for authoring and `AdmissionOnly` for matching. Registry versions
`1` through `6` are stale. Generic kind-`1111` signing and client publication
fail before signer access. The complete operation surface is exactly
`social.comment.build_authored_draft`,
`social.comment.project_verified_event`, and
`social.comment.verify_and_admit_event`.

Comment content is limited to 131072 UTF-8 bytes, a Comment to 1024 tags, all
tags to 4096 elements including names, each element to 4096 UTF-8 bytes,
aggregate tag bytes to 131072, and compact signed wire JSON to 262144 bytes.
The canonical self-contained 114-case corpus is
`contracts/conformance/vectors/comment/verified_profile.v1.json`.

## NIP-09 deletion-request boundary rule

The strict Radroots
[NIP-09](https://github.com/nostr-protocol/nips/blob/bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91/09.md)
profile is kind `5` and requires at least one valid `e` event id or `a`
replaceable/addressable coordinate. Strict authoring sorts normalized event
targets and address targets independently, rejects duplicates, and emits all
two-element `e` tags, then `a` tags, then the unique ascending target-kind
advisories as two-element `k` tags. A caller-supplied event-target kind and
every `k` tag are advisory metadata, not proof of the referenced event's kind.

Inbound projection accepts only an id-and-signature-verified envelope. It
preserves exact raw tags, tolerates unknown tags and trailing target elements,
deduplicates normalized targets with first-seen provenance, and exposes sorted
typed target views. A malformed `e` or `a` target is a hard failure.
Malformed, noncanonical, duplicate, and address-only-conflicting `k` tags
produce stable ordered diagnostics. An event target makes kind correspondence
unprovable, so a differing `k` advisory is not promoted to a conflict.

Admission proves only that the kind-`5` request envelope and bounded request
profile are valid. It performs no target lookup, same-author authorization,
address cutoff evaluation, replacement or suppression decision, store
mutation, or deletion-request immunity evaluation.

The separate pure evaluator accepts a signature-verified candidate and
admitted deletion requests. It never mutates an event or store. Kind `5` is
immune; every other suppression requires equal request and candidate authors.
An exact `e` target is time-independent. An exact canonical `a` target applies
through the inclusive maximum qualifying request timestamp, so a later
replacement remains visible. Advisory `k` values are ignored. The decision and
its canonical evidence are independent of request order and repeated
qualifying inputs.

The event-contract registry v7 classifies
`radroots.social.deletion_request.v1` as `TypedOnly` for authoring and
`AdmissionOnly` for matching. Generic kind-`5` signing and client publication
fail before signer access.

The complete operation surface is exactly
`social.deletion_request.build_authored_draft`,
`social.deletion_request.project_verified_event`,
`social.deletion_request.verify_and_admit_event`, and
`social.deletion_request.evaluate_suppression`. Content is limited to 131072
UTF-8 bytes, a request to 1024 tags, all tags to 4096 elements including names,
each element to 4096 UTF-8 bytes, aggregate tag bytes to 131072, and compact
signed wire JSON to 262144 bytes. The canonical fixed 80-case request corpus is
`contracts/conformance/vectors/deletion/verified_profile.v1.json`.
Pure effect evaluation has the separate canonical
`contracts/conformance/vectors/deletion/suppression.v1.json` corpus and does
not weaken or add effect fields to the request corpus.

## Coverage matrix

| Domain | Kind | Radroots Type | RPC Methods | Notes |
| --- | --- | --- | --- | --- |
| profile | 0 | AuthoredProfile / RadrootsInboundProfileMetadata | events.profile.publish, events.profile.list, events.profile.get | publish must use `profile.build_authored_draft`; inbound projection must use `profile.parse_inbound_metadata`; authored output is deterministic JSON with no marker tag |
| follow | 3 | Follow | events.follow.publish, events.follow.list, events.follow.get | replaceable event |
| post | 1 | AuthoredUpdate / AuthoredPhotoUpdate / AuthoredAsk / RadrootsInboundPostProjection | events.post.publish, events.post.list, events.post.get | ordinary kind-1 reads remain generic; exact root-card subtypes require verified admission; any `e` tag produces a thread-excluded candidate without a Reply claim |
| reply | 1 | AuthoredNip10Reply / RadrootsInboundNip10ReplyProjection / RadrootsAdmittedNip10ReplyEvent / RadrootsNostrNip10ReplyEventBuilder | social.reply.build_authored_draft, social.reply.project_verified_event, social.reply.verify_and_admit_event | strict marked direct/nested NIP-10 authoring; verified marked or positional inbound admission with advisory-metadata diagnostics; never a root card; target existence, kind, author, and relay availability are not proven |
| comment | 1111 | AuthoredNip22Comment / RadrootsInboundNip22CommentProjection / RadrootsAdmittedNip22CommentEvent / RadrootsNostrNip22CommentEventBuilder | social.comment.build_authored_draft, social.comment.project_verified_event, social.comment.verify_and_admit_event | strict NIP-22 event/address roots limited to kinds `30402`, `31922`, and `31923`; tolerant verified projection; registry-v7 typed-only authoring and admission-only matching |
| deletion_request | 5 | AuthoredNip09DeletionRequest / RadrootsInboundNip09DeletionProjection / RadrootsAdmittedNip09DeletionRequestEvent / RadrootsNip09SuppressionDecision / RadrootsNostrNip09DeletionRequestEventBuilder | social.deletion_request.build_authored_draft, social.deletion_request.project_verified_event, social.deletion_request.verify_and_admit_event, social.deletion_request.evaluate_suppression | effect-free NIP-09 request authoring and verified projection plus pure immutable suppression evaluation; same-author direct event targets and inclusive address cutoffs; kind-5 immunity; advisory kinds ignored; registry-v7 typed-only authoring and admission-only matching |
| reaction | 7 | Reaction | events.reaction.publish, events.reaction.list, events.reaction.get | requires event, pubkey, or address tags |
| repost | 6 | Repost | events.repost.publish, events.repost.list, events.repost.get | NIP-18 kind-1 repost surface |
| generic_repost | 16 | GenericRepost | events.generic_repost.publish, events.generic_repost.list, events.generic_repost.get | NIP-18 generic repost surface |
| seal | 13 | Seal | events.seal.encode, events.seal.decode | tags must be empty; used for NIP-59 transport |
| message | 14 | Message | events.message.publish, events.message.list, events.message.get | rumor event; unsigned before wrapping |
| message_file | 15 | MessageFile | events.message_file.publish, events.message_file.list, events.message_file.get | rumor event with file tags |
| gift_wrap | 1059 | GiftWrap | events.gift_wrap.publish, events.gift_wrap.list, events.gift_wrap.get | requires `p` tag; optional expiration |
| public_file_metadata | 1063 | FileMetadata | events.public_file_metadata.publish, events.public_file_metadata.list, events.public_file_metadata.get | public NIP-94 file metadata, distinct from private farm file metadata |
| report | 1984 | Report | events.report.publish, events.report.list, events.report.get | NIP-56 report with required reported pubkey |
| list | 10000..10102 | List | events.list.publish, events.list.list, events.list.get | replaceable NIP-51 list kinds excluding kind 3 |
| relay_list | 10002 | List | events.relay_list.publish, events.relay_list.list, events.relay_list.get | NIP-65 relay list entries with `read` or `write` markers |
| list_set | 30000..30007, 30015, 30030, 30063, 30267, 39089, 39092 | ListSet | events.list_set.publish, events.list_set.list, events.list_set.get | enumerated addressable NIP-51 list sets with `d` tag; NIP-52 kind `31924` is exclusively the calendar surface |
| article | 30023 | Article | events.article.publish, events.article.list, events.article.get | NIP-23 long-form content |
| knowledge | 818, 3460..3465, 30450..30451, 30818..30819 | RadrootsKnowledgeEvent | events.knowledge.publish, events.knowledge.list, events.knowledge.get | NIP-54 wiki plus Radroots knowledge source, claim, relation, review, field-report, bounty, proposal, and contribution contracts |
| app_data | 30078 | AppData | events.app_data.publish, events.app_data.list, events.app_data.get | addressable app data with `d` tag |
| app_handler | 31990 | KIND_APPLICATION_HANDLER | events.app_handler.publish, events.app_handler.list, events.app_handler.get | optional discoverability |
| calendar_date | 31922 | AuthoredCalendarDateEvent / ParsedNip52CalendarDateEvent / AdmittedCalendarDateEvent | events.calendar_date.publish, events.calendar_date.list, events.calendar_date.get | NIP-52 date event; baseline retains uppercase-`D` extensions, strict admission rejects them |
| calendar_time | 31923 | AuthoredCalendarTimeEvent / ParsedNip52CalendarTimeEvent / AdmittedCalendarTimeEvent | events.calendar_time.publish, events.calendar_time.list, events.calendar_time.get | NIP-52 time event; baseline applies required day anchoring, strict admission requires exact bounded UTC-day coverage |
| calendar | 31924 | AuthoredCalendar / ParsedNip52Calendar / AdmittedCalendar | events.calendar.publish, events.calendar.list, events.calendar.get | NIP-52 calendar collection with separate authored, baseline parse, and strict Radroots admission boundaries |
| calendar_rsvp | 31925 | AuthoredCalendarEventRsvp / ParsedNip52CalendarEventRsvp / AdmittedCalendarEventRsvp | events.calendar_rsvp.publish, events.calendar_rsvp.list, events.calendar_rsvp.get | NIP-52 calendar RSVP with separate authored, baseline parse, and strict Radroots admission boundaries |
| farm | 30340 | Farm | events.farm.publish, events.farm.list, events.farm.get | addressable; canonical JSON; `g` tag only when a geohash exists |
| plot | 30350 | Plot | events.plot.publish, events.plot.list, events.plot.get | requires address and pubkey tags; preserve self-tag |
| coop | 30360 | Coop | events.coop.publish, events.coop.list, events.coop.get | addressable; canonical JSON; `g` tag from geohash |
| document | 30361 | Document | events.document.publish, events.document.list, events.document.get | requires `d` and pubkey tags; optional address tag |
| resource_area | 30370 | ResourceArea | events.resource_area.publish, events.resource_area.list, events.resource_area.get | addressable; GCS location and `g` tag required |
| resource_cap | 30371 | ResourceHarvestCap | events.resource_cap.publish, events.resource_cap.list, events.resource_cap.get | addressable; required address, pubkey, key, start, and end tags |
| food_availability | 30402 | FoodAvailabilityDetails / RadrootsInboundFoodAvailabilityProjection / RadrootsAdmittedFoodAvailabilityEvent / RadrootsNostrFoodAvailabilityEventBuilder | food_availability.build_authored_draft, food_availability.project_verified_event, food_availability.verify_and_admit_event, food_availability.validate_revision | focused `radroots.food.availability.v1` profile; strict deterministic authoring, verified projection/admission, stable-coordinate revision validation, sealed Nostr signing/publication, generic-authoring reservation, and raw-head-first partitioning only behind the non-default `legacy-ingest` replica feature; BUD-02 upload evidence remains a runtime prerequisite |
| operational_listing | 30402 | OperationalListing | events.operational_listing.publish, events.operational_listing.list, events.operational_listing.get | NIP-99 classified-listing kind with the richer Radroots operational profile; canonical Markdown content and tags; farm author required |
| dvm_request | 5000-5999 | JobRequest | events.dvm_request.publish, events.dvm_request.list, events.dvm_request.get | generic DVM request surface |
| dvm_result | 6000-6999 | JobResult | events.dvm_result.publish, events.dvm_result.list, events.dvm_result.get | generic DVM result surface |
| dvm_feedback | 7000 | JobFeedback | events.dvm_feedback.publish, events.dvm_feedback.list, events.dvm_feedback.get | generic DVM feedback surface |
| trade:proposal | 3470 | TradeMutationEnvelopeV1 | trade.get_trade, trade.list_trades, trade.submit_proposal | buyer-authored initial complete candidate mutation with canonical semantic identity |
| trade:decision | 3471 | TradeMutationEnvelopeV1 | trade.decide_candidate, trade.get_trade, trade.list_trades | exact accept or decline mutation for a referenced candidate and proposal mutation |
| trade:revision_proposal | 3472 | TradeMutationEnvelopeV1 | trade.get_trade, trade.list_trades, trade.propose_revision | buyer- or seller-authored complete replacement candidate referencing bounded parent heads |
| trade:revision_decision | 3473 | TradeMutationEnvelopeV1 | trade.decide_candidate, trade.get_trade, trade.list_trades | exact accept or decline mutation for a referenced revision candidate and proposal mutation |
| trade:cancellation | 3474 | TradeMutationEnvelopeV1 | trade.cancel_trade, trade.get_trade, trade.list_trades | policy-authorized cancellation mutation referencing the relevant candidate or current claim |
| trade:validation_receipt | 3440 | RadrootsTradeValidationReceipt | domains.trade.validation_receipt.get, domains.trade.validation_receipt.list, domains.trade.validation_receipt.verify | proof and inspection artifact around canonical signed trade events and deterministic reducer output; not buyer receipt state, payment state, order mutation, or trade authority |
| relay_doc | N/A | RelayDocument | system.relay_doc.get | HTTP NIP-11 info via relay fetch helper |

## Membership list sets and claims

- Farm lists: kind `30001`, including `farm:<farm_d_tag>:members`, `members.owners`, `members.workers`, `plots`, and `listings`.
- Coop lists: kind `30001`, including `coop:<coop_d_tag>:members`, `members.farms`, `members.owners`, `members.admins`, and `items`.
- Resource area lists: kind `30001`, including `resource:<area_d_tag>:members.farms`, `members.plots`, and `members.stewards`.
- Member-side claims: kind `30001`, including `member_of.farms` and `member_of.coops`.
