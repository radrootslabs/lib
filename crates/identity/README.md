# radroots_identity

`radroots_identity` provides the portable public identity and account values
shared by Radroots packages: canonical public keys, identity and account IDs,
public profiles, normalized usernames, and transport-neutral account status.

The crate supports `no_std` environments with `alloc`. It performs no I/O,
starts no tasks or threads, reads no clocks or process state, and owns no key
generation, secrets, signing, persistence, networking, or account selection.

## Example

Construct public identities from validated keys and derive account identifiers
without exposing or inventing secret material:

```rust
use radroots_identity::{AccountId, Profile, PublicIdentity, PublicKey, Username};

let public_key = PublicKey::from_hex(
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df",
)?;
let username = Username::parse(" Alice.Farm ")?;
let identity = PublicIdentity::new(public_key)
    .with_profile(Profile::new().with_username(username));
let account_id = AccountId::from_public_identity(&identity);

assert_eq!(identity.id().as_bytes(), public_key.as_bytes());
assert_eq!(account_id.as_bytes(), public_key.as_bytes());
assert_eq!(
    identity.profile().and_then(Profile::username).map(Username::as_str),
    Some("alice.farm"),
);
# Ok::<(), radroots_identity::Error>(())
```

The same program is available as the
[`public_identity`](examples/public_identity.rs) example.

## Public API

The crate root intentionally exports `AccountId`, `IdentityId`,
`PublicIdentity`, `PublicKey`, `Profile`, `Username`, and the aggregate
[`Error`](https://docs.rs/radroots_identity/latest/radroots_identity/enum.Error.html).
Focused values remain in these modules:

- `account` — derived account IDs, public account records, and observable
  readiness status;
- `key` — validated 32-byte x-only secp256k1 public keys and identity IDs;
- `profile` — public identity metadata and the invariant-matched public
  identity aggregate;
- `username` — canonical username parsing, bounds, and normalization.

The normative responsibility and dependency boundary are defined by the
[`radroots_identity` package charter](../../docs/specs/radroots_crates_release_v1.md).
The reviewed pre-release surface is recorded in
[`docs/api`](../../docs/api/README.md). Signing, Nostr-key, secret-provider,
and storage ownership is documented in the
[`identity` migration boundary](../../docs/migration/identity.md).

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | Implements the standard error integration; value behavior remains portable. |
| `serde` | yes | Implements checked canonical serialization and deserialization for public values. |

Disabling default features leaves the `no_std` + `alloc` value model. Features
are additive; enabling one does not select a runtime, backend, global account,
secret source, or side effect.

## Invariants and untrusted input

`PublicKey`, `IdentityId`, and `AccountId` store exactly 32 validated bytes.
Text parsing accepts exact-width hexadecimal input and emits canonical
lowercase hexadecimal output. `PublicKey` validation confirms that the x-only
bytes identify a secp256k1 curve point. The three Rust types remain distinct so
callers cannot accidentally substitute an account identifier for a public key.

`PublicIdentity` derives its `IdentityId` from its `PublicKey` and rejects
separately decoded parts that do not match. `account::Record` derives its
`AccountId` from its public identity and rejects mismatched IDs or an update
timestamp before creation. `Record::touch_updated` rejects time reversal and
leaves the timestamp unchanged on error. `Record::set_label` does not read a
clock or update the timestamp; the composing host owns clock policy and must
advance the timestamp explicitly when appropriate.

`Username` trims surrounding whitespace, folds ASCII uppercase to lowercase,
enforces its byte bounds, and rejects unsupported characters or leading,
trailing, and consecutive dots. Account labels are opaque host-facing strings:
callers must apply their own content and resource limits before accepting them
from untrusted sources.

## Serialization

With `serde`, identifiers are canonical lowercase 64-character hexadecimal
strings and usernames are canonical strings. Profiles, public identities, and
account records use named fields and reject unknown fields. Deserializing a
public identity or account record re-applies identifier and timestamp
invariants; invalid native state is not admitted through the supported serde
surface. Account status variant names use `snake_case`.

Serialization contains public metadata only and does not add confidentiality,
authenticity, event signing, or transport framing. Versioned cross-process wire
DTOs belong in `radroots_protocol` rather than this native value crate.

## Execution and commit semantics

Operations are synchronous and deterministic. Username parsing and account
record construction may allocate in proportion to caller-supplied username or
label text, so callers should cap untrusted input bytes before invoking them.
There are no asynchronous cancellation points, deadlines, retries, callbacks,
partial external effects, or durable commit points. A successful call returns
or updates the complete in-memory value; an error returns before any durable
commit because this crate owns no durable state.

## Security boundary

The crate forbids unsafe Rust. Public keys and identifiers are public data and
must not be treated as proof of control; authentication requires a verified
signature in the appropriate event or signing layer. This crate deliberately
has no raw secret keys, key generation, nsec/NIP-49 helpers, keyrings, vaults,
files, SQLite, runtime paths, signer sessions, or upstream Nostr events.

```compile_fail
use radroots_identity::{SecretKey, generate_keypair};
```

Do not recreate those responsibilities through compatibility wrappers around
this crate. Compose the signing, Nostr, secrets, and storage packages named by
the migration boundary instead.

## Intended consumers

`radroots_identity` is intended for lower-level Radroots event, signing,
transport, storage, and domain packages and for integrators that need the
portable public value model directly. Applications should normally begin with
the curated `radroots` crate or the advanced `radroots_sdk` composition
surface.

The package is pre-1.0. Its durable responsibility and package identity are
fixed, while API-breaking changes follow the workspace's pre-1.0 versioning
policy.

## License

Licensed under either Apache-2.0 or MIT, at your option.
