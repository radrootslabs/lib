# radroots_transport_nostr

Deterministic Nostr relay transport substrate for exact signed-event publish,
fetch ingest, and outbox delivery target coordination.

Every fetch path verifies the NIP-01 id and signature before filter matching,
unique-event budgeting, or returning an event. Repeated event ids preserve
per-relay observation evidence without consuming the unique-event limit. The
unique-event limit is bounded at 1,000 so final stored-event visibility can be
evaluated in one coherent event-store snapshot. A fetch scans at most 64,000
raw events and 64 MiB of aggregate raw JSON, and rejects any individual raw
event over 256 KiB before Radroots parses adapter raw JSON. Count and byte
budgets are charged globally, in adapter order, before Radroots parsing,
filtering, deduplication, or accepted-event limiting, so malformed and otherwise
rejected events cannot bypass them. The official SDK adapter enforces the same
retained-prefix budgets after SDK frame/event decoding and before retaining its
serialized JSON; the connector's upstream frame parser remains responsible for
its own first-pass network limits. `Truncated` is distinct from relay `EOSE`,
including for later target relays that were not queried after a global count or
byte budget was reached.

Fetch-ingest receipts report verification, contract admission, immutable
valid-stream eligibility, and current visibility as independent exhaustive
enums. Verification failure is not a contract-invalid result, and unsupported
admission does not hide whether the stored event is current, suppressed, or not
admitted. Persisted events obtain visibility from the event store's central
authority after the complete accepted fetch batch has been ingested, so event
receipts and aggregate visibility counts describe final post-batch state rather
than transient per-item state. Repeated receipt IDs are deduplicated before the
single snapshot lookup and then mapped back to every receipt. Ephemeral events
use the explicit `not_persisted` visibility result.
`admission_code` carries the stable classifier diagnostic when classification
produces one. Inserted, duplicate, and not-persisted persistence outcomes retain
separate flags and aggregate counts. Local event-store failures abort the
operation and remain typed transport errors, so callers can retry without
confusing storage failure with bad relay input.

`RadrootsRelayUrlPolicy::Public` is for trusted relay configuration. It rejects
non-canonical and known non-global literal destinations, but hostname checks do
not pin DNS resolution in the SDK connector and are not an SSRF boundary for
attacker-controlled relay hostnames. Validate resolved addresses at the network
boundary or use a connector that pins approved resolutions before accepting
untrusted relay configuration.
