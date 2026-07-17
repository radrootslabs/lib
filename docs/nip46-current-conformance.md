# Current NIP-46 conformance

This document defines the public compatibility contract for the Nostr Connect
protocol, signer-session, and relay-client changes that bring the Radroots
libraries into conformance with the current NIP-46 lifecycle.

## Qualified baseline

The contribution is based on commit `e4e8ae87f6c230c4f26c317bd3107a6710e936a0`
from `origin/master`. The baseline tests for `radroots_nostr_connect`,
`radroots_nostr_signer`, and `radroots_nostr` pass together before the behavior
changes described here.

The repository-wide contract lane has an unrelated baseline hygiene failure in
the trade workflow surface. NIP-46 changes must not add to that failure, and
the affected crate and contract lanes remain required for every checkpoint.

## Wire compatibility

`connect` accepts the current positional parameter sequence:

1. remote-signer public key;
2. optional connection secret;
3. optional requested permissions;
4. optional JSON-stringified client metadata.

One-, two-, and three-parameter messages remain valid. Empty positional values
are emitted when a later optional value is present. Client metadata contains
only `name`, `url`, and `image`; requested permissions remain the third
`connect` parameter and the `perms` query parameter of a `nostrconnect://`
token.

`logout` is a typed zero-parameter request. Its successful response is the
string `ack`. Unsupported custom methods keep their existing error and custom
response behavior.

## Client metadata

Client metadata is untrusted, unauthenticated display input. It never selects
an identity, grants a permission, changes an approval requirement, or affects
signing authorization.

The protocol model normalizes and validates metadata at URI and request
boundaries:

- names are trimmed, non-empty, free of control characters, and at most 128
  UTF-8 bytes;
- URL and image values are at most 2,048 UTF-8 bytes, use `http` or `https`,
  contain no credentials, and are normalized through the URL parser;
- the JSON metadata parameter is bounded before parsing;
- absent and legacy metadata decode as `None` or the default empty value.

Diagnostic output may identify which field is invalid, but must not log the
connection secret or a complete untrusted metadata payload.

## URI behavior

`nostrconnect://` requires at least one relay and a non-empty secret. Its
requested permissions and display metadata use separate typed fields.
`bunker://` requires at least one relay and keeps its secret optional. Repeated
relay parameters retain input order in both URI forms.

## Signer persistence and revocation

Connection drafts and records may carry optional validated client metadata.
JSON state created before the field existed decodes with metadata absent. The
native SQLite store persists the same value through a forward migration while
retaining existing connection rows.

Revocation remains an explicit, idempotent transition to `Revoked`. A service
handling `logout` must publish the acknowledgement before revoking the session;
transport publication ordering is intentionally owned by the service rather
than hidden inside the storage transition.

## Relay-client boundary

`radroots_nostr` owns the public client operations needed by portable NIP-46
adapters. An adapter can add relays, connect, subscribe, publish events, and
explicitly unsubscribe without importing `nostr-sdk` directly. The wrapper
does not create detached listener tasks or hide subscription ownership.

The `client` and `events` feature combination must compile for native and
`wasm32-unknown-unknown`. Default and non-client feature behavior remains
unchanged.

## Deterministic conformance

Checked-in vectors cover legacy and four-parameter `connect`, metadata bounds,
repeated relay order, optional bunker secrets, mandatory client secrets,
secret-echo responses, relay switching, auth continuation, typed logout, and
malformed request or response envelopes. Fixtures use public deterministic
keys and non-routable example domains; they contain no reusable credentials.
