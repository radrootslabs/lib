# conformance vectors

Conformance vectors define canonical cross-language expectations for the Rad Roots SDK contract.

Each fixture must be deterministic and machine-readable.

## layout

- `schema/vector.schema.json`: json schema for vector documents
- `vectors/events/*.json`: event model and tag/codec vectors
- `vectors/trade/*.json`: trade model and transform vectors
- `vectors/identity/*.json`: identity model vectors

## rules

- vectors are generated from canonical rust implementations.
- every contract behavior change must update vectors in the same change.
- language sdk test harnesses must validate vectors without local overrides.

## social event vectors

Social event vectors are required for every new social codec and every existing
codec whose public behavior changes under `spec/social-events.md`.

The social vector set must cover valid, invalid, and round-trip behavior for the
approved public social event families. It must include strict NIP-22 comment
targets, empty-content NIP-25 reactions, public NIP-94 generic file metadata,
NIP-99 listing `published_at`, NIP-65 relay-list `r` tags, and private Field
business-document isolation.

Social event vectors must be deterministic, synthetic, and repo-owned. They must
not depend on relay databases, application runtime state, external services, or
fixture roots outside this repository.
