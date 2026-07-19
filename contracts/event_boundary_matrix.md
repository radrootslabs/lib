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

## Calendar boundary rule

Calendar kinds `31922` through `31925` expose separate authored,
baseline-parsed, and strict-admitted types. A structural
`RadrootsEventEnvelope` is not cryptographic verification, and neither the
NIP-52 parser nor Radroots admission verifies the declared event id or Schnorr
signature. A read-side runtime must keep the parsed or admitted value bound to
an envelope whose id and signature it has independently verified and whose kind
the corresponding parser accepted. Outbound authored models produce unsigned
wire parts and require runtime signing and transport.

## Classified-listing profile boundary rule

Kind `30402` is the standard NIP-99 classified-listing kind and
`RadrootsClassifiedListingAddress` is the exact coordinate authority for that
kind. `RadrootsOperationalListing` is the richer
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
Legacy replica ingestion verifies NIP-01 first, selects the raw addressable head
before profile decoding, and routes only Operational Listing into its
trade-product projection. Selected focused or generic exclusions and invalid or
ambiguous rejections remove an older operational projection and still advance
the raw head. The head-only replica helper rejects kind `30402`; these events
require profile-aware ingestion so projection cleanup and head movement remain
atomic.

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
rejects every kind `1` event before signing. The legacy mutable `RadrootsPost`
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

Reply admission proves the Reply envelope's id, signature, and bounded NIP-10
structure. It does not prove that a referenced event exists, has kind `1`, has
a valid signature, was authored by the declared referenced author, or is
available from a relay. Signed-event relay remains generic transport rather
than a typed Reply-authoring boundary.

## Coverage matrix

| Domain | Kind | Radroots Type | RPC Methods | Notes |
| --- | --- | --- | --- | --- |
| profile | 0 | RadrootsAuthoredProfile / RadrootsInboundProfileMetadata | events.profile.publish, events.profile.list, events.profile.get | publish must use `profile.build_authored_draft`; inbound projection must use `profile.parse_inbound_metadata`; authored output is deterministic JSON with no marker tag |
| follow | 3 | RadrootsFollow | events.follow.publish, events.follow.list, events.follow.get | replaceable event |
| post | 1 | RadrootsAuthoredUpdate / RadrootsAuthoredPhotoUpdate / RadrootsAuthoredAsk / RadrootsInboundPostProjection | events.post.publish, events.post.list, events.post.get | ordinary kind-1 reads remain generic; exact root-card subtypes require verified admission; any `e` tag produces a thread-excluded candidate without a Reply claim |
| reply | 1 | RadrootsAuthoredNip10Reply / RadrootsInboundNip10ReplyProjection / RadrootsAdmittedNip10ReplyEvent / RadrootsNostrNip10ReplyEventBuilder | social.reply.build_authored_draft, social.reply.project_verified_event, social.reply.verify_and_admit_event | strict marked direct/nested NIP-10 authoring; verified marked or positional inbound admission with advisory-metadata diagnostics; never a root card; target existence, kind, author, and relay availability are not proven |
| comment | 1111 | RadrootsComment | events.comment.publish, events.comment.list, events.comment.get | requires root and parent tags |
| reaction | 7 | RadrootsReaction | events.reaction.publish, events.reaction.list, events.reaction.get | requires event, pubkey, or address tags |
| repost | 6 | RadrootsRepost | events.repost.publish, events.repost.list, events.repost.get | NIP-18 kind-1 repost surface |
| generic_repost | 16 | RadrootsGenericRepost | events.generic_repost.publish, events.generic_repost.list, events.generic_repost.get | NIP-18 generic repost surface |
| seal | 13 | RadrootsSeal | events.seal.encode, events.seal.decode | tags must be empty; used for NIP-59 transport |
| message | 14 | RadrootsMessage | events.message.publish, events.message.list, events.message.get | rumor event; unsigned before wrapping |
| message_file | 15 | RadrootsMessageFile | events.message_file.publish, events.message_file.list, events.message_file.get | rumor event with file tags |
| gift_wrap | 1059 | RadrootsGiftWrap | events.gift_wrap.publish, events.gift_wrap.list, events.gift_wrap.get | requires `p` tag; optional expiration |
| public_file_metadata | 1063 | RadrootsFileMetadata | events.public_file_metadata.publish, events.public_file_metadata.list, events.public_file_metadata.get | public NIP-94 file metadata, distinct from private farm file metadata |
| report | 1984 | RadrootsReport | events.report.publish, events.report.list, events.report.get | NIP-56 report with required reported pubkey |
| list | 10000..10102 | RadrootsList | events.list.publish, events.list.list, events.list.get | replaceable NIP-51 list kinds excluding kind 3 |
| relay_list | 10002 | RadrootsList | events.relay_list.publish, events.relay_list.list, events.relay_list.get | NIP-65 relay list entries with `read` or `write` markers |
| list_set | 30000..30007, 30015, 30030, 30063, 30267, 39089, 39092 | RadrootsListSet | events.list_set.publish, events.list_set.list, events.list_set.get | enumerated addressable NIP-51 list sets with `d` tag; NIP-52 kind `31924` is exclusively the calendar surface |
| article | 30023 | RadrootsArticle | events.article.publish, events.article.list, events.article.get | NIP-23 long-form content |
| knowledge | 818, 3460..3465, 30450..30451, 30818..30819 | RadrootsKnowledgeEvent | events.knowledge.publish, events.knowledge.list, events.knowledge.get | NIP-54 wiki plus Radroots knowledge source, claim, relation, review, field-report, bounty, proposal, and contribution contracts |
| app_data | 30078 | RadrootsAppData | events.app_data.publish, events.app_data.list, events.app_data.get | addressable app data with `d` tag |
| app_handler | 31990 | KIND_APPLICATION_HANDLER | events.app_handler.publish, events.app_handler.list, events.app_handler.get | optional discoverability |
| calendar_date | 31922 | RadrootsAuthoredCalendarDateEvent / RadrootsParsedNip52CalendarDateEvent / RadrootsAdmittedCalendarDateEvent | events.calendar_date.publish, events.calendar_date.list, events.calendar_date.get | NIP-52 date event; baseline retains uppercase-`D` extensions, strict admission rejects them |
| calendar_time | 31923 | RadrootsAuthoredCalendarTimeEvent / RadrootsParsedNip52CalendarTimeEvent / RadrootsAdmittedCalendarTimeEvent | events.calendar_time.publish, events.calendar_time.list, events.calendar_time.get | NIP-52 time event; baseline applies required day anchoring, strict admission requires exact bounded UTC-day coverage |
| calendar | 31924 | RadrootsAuthoredCalendar / RadrootsParsedNip52Calendar / RadrootsAdmittedCalendar | events.calendar.publish, events.calendar.list, events.calendar.get | NIP-52 calendar collection with separate authored, baseline parse, and strict Radroots admission boundaries |
| calendar_rsvp | 31925 | RadrootsAuthoredCalendarEventRsvp / RadrootsParsedNip52CalendarEventRsvp / RadrootsAdmittedCalendarEventRsvp | events.calendar_rsvp.publish, events.calendar_rsvp.list, events.calendar_rsvp.get | NIP-52 calendar RSVP with separate authored, baseline parse, and strict Radroots admission boundaries |
| farm | 30340 | RadrootsFarm | events.farm.publish, events.farm.list, events.farm.get | addressable; canonical JSON; `g` tag only when a geohash exists |
| plot | 30350 | RadrootsPlot | events.plot.publish, events.plot.list, events.plot.get | requires address and pubkey tags; preserve self-tag |
| coop | 30360 | RadrootsCoop | events.coop.publish, events.coop.list, events.coop.get | addressable; canonical JSON; `g` tag from geohash |
| document | 30361 | RadrootsDocument | events.document.publish, events.document.list, events.document.get | requires `d` and pubkey tags; optional address tag |
| resource_area | 30370 | RadrootsResourceArea | events.resource_area.publish, events.resource_area.list, events.resource_area.get | addressable; GCS location and `g` tag required |
| resource_cap | 30371 | RadrootsResourceHarvestCap | events.resource_cap.publish, events.resource_cap.list, events.resource_cap.get | addressable; required address, pubkey, key, start, and end tags |
| food_availability | 30402 | RadrootsFoodAvailabilityDetails / RadrootsInboundFoodAvailabilityProjection / RadrootsAdmittedFoodAvailabilityEvent / RadrootsNostrFoodAvailabilityEventBuilder | food_availability.build_authored_draft, food_availability.project_verified_event, food_availability.verify_and_admit_event, food_availability.validate_revision | focused `radroots.food.availability.v1` profile; strict deterministic authoring, verified projection/admission, stable-coordinate revision validation, sealed Nostr signing/publication, generic-authoring reservation, and raw-head-first legacy replica partitioning; BUD-02 upload evidence remains a runtime prerequisite |
| operational_listing | 30402 | RadrootsOperationalListing | events.operational_listing.publish, events.operational_listing.list, events.operational_listing.get | NIP-99 classified-listing kind with the richer Radroots operational profile; canonical Markdown content and tags; farm author required |
| dvm_request | 5000-5999 | RadrootsJobRequest | events.dvm_request.publish, events.dvm_request.list, events.dvm_request.get | generic DVM request surface |
| dvm_result | 6000-6999 | RadrootsJobResult | events.dvm_result.publish, events.dvm_result.list, events.dvm_result.get | generic DVM result surface |
| dvm_feedback | 7000 | RadrootsJobFeedback | events.dvm_feedback.publish, events.dvm_feedback.list, events.dvm_feedback.get | generic DVM feedback surface |
| trade:proposal | 3470 | RadrootsTradeMutationEnvelopeV1 | trade.get_trade, trade.list_trades, trade.submit_proposal | buyer-authored initial complete candidate mutation with canonical semantic identity |
| trade:decision | 3471 | RadrootsTradeMutationEnvelopeV1 | trade.decide_candidate, trade.get_trade, trade.list_trades | exact accept or decline mutation for a referenced candidate and proposal mutation |
| trade:revision_proposal | 3472 | RadrootsTradeMutationEnvelopeV1 | trade.get_trade, trade.list_trades, trade.propose_revision | buyer- or seller-authored complete replacement candidate referencing bounded parent heads |
| trade:revision_decision | 3473 | RadrootsTradeMutationEnvelopeV1 | trade.decide_candidate, trade.get_trade, trade.list_trades | exact accept or decline mutation for a referenced revision candidate and proposal mutation |
| trade:cancellation | 3474 | RadrootsTradeMutationEnvelopeV1 | trade.cancel_trade, trade.get_trade, trade.list_trades | policy-authorized cancellation mutation referencing the relevant candidate or current claim |
| trade:validation_receipt | 3440 | RadrootsTradeValidationReceipt | domains.trade.validation_receipt.get, domains.trade.validation_receipt.list, domains.trade.validation_receipt.verify | proof and inspection artifact around canonical signed trade events and deterministic reducer output; not buyer receipt state, payment state, order mutation, or trade authority |
| relay_doc | N/A | RadrootsRelayDocument | system.relay_doc.get | HTTP NIP-11 info via relay fetch helper |

## Membership list sets and claims

- Farm lists: kind `30001`, including `farm:<farm_d_tag>:members`, `members.owners`, `members.workers`, `plots`, and `listings`.
- Coop lists: kind `30001`, including `coop:<coop_d_tag>:members`, `members.farms`, `members.owners`, `members.admins`, and `items`.
- Resource area lists: kind `30001`, including `resource:<area_d_tag>:members.farms`, `members.plots`, and `members.stewards`.
- Member-side claims: kind `30001`, including `member_of.farms` and `member_of.coops`.
