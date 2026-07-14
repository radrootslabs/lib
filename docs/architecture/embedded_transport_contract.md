# Embedded Transport Contract

Radroots public transport code separates transport-neutral delivery contracts from concrete
transport adapters. Public APIs must describe targets, payloads, delivery status, and satisfaction
policy without smuggling a relay-only model into generic transport surfaces.

## Contract boundaries

`radroots_transport` owns transport-neutral value types:

- `RadrootsTransportKind`
- `RadrootsTransportTarget`
- `RadrootsTransportTargetUri`
- `RadrootsTransportTargetSet`
- `RadrootsTransportPayload`
- `RadrootsTransportSatisfactionPolicy`
- `RadrootsTransportDeliveryReceipt`

Nostr-specific behavior belongs in Nostr-owned crates and NIP-specific modules. Mesh behavior
belongs in mesh-owned crates. Generic transport code may name Nostr only when it is modeling an
explicit Nostr target or a Nostr adapter boundary.

## Embedded and preview transports

The Reticulum preview endpoint is intentionally explicit. The only accepted preview endpoint is the
checked-in `RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI` value, and preview delivery remains unavailable
for real payload transfer until a concrete transport implementation is added.

Mesh frame CBOR is a source-level contract for local and embedded transport experimentation. The
MVP frame shape is fixed by `radroots_mesh` tests and conformance vectors, and real payload bytes
are rejected by the preview policy.

## Malformed input posture

Transport parsers are part of the release validation surface. Malformed transport target URIs,
unsupported mesh CBOR shapes, payload-bearing preview frames, and invalid replica JSON models must
fail through checked-in tests and conformance vectors. Runtime callers should depend on those
fallible constructors and parser results instead of accepting unchecked target strings or raw bytes.
