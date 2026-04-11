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
