# radroots_secrets

`radroots_secrets` defines the bounded secret-provider, key-wrapping, and
authenticated-envelope contracts for Radroots. Hosts choose and configure a
provider explicitly; this crate does not own application authorization,
credential UI, account policy, runtime execution, or global provider state.

The crate is pre-release and publication remains disabled. Its Cargo version
is frozen at `0.1.0-alpha` until the coordinated release contract explicitly
changes it.

## Canonical surface

| Module | Responsibility |
| --- | --- |
| `id` | Validated, redacted identifiers, backend kinds, key versions, and single-owner references. |
| `provider` | Provider capabilities, exact selection policy, and the executor-neutral provider SPI. |
| `wrapping` | Bounded zeroizing plaintext, wrapped values, requests, and the key-wrapping SPI. |
| `envelope` | Versioned XChaCha20-Poly1305 envelope encoding with authenticated provider metadata. |
| `memory` | Opt-in, process-local adapter for development and deterministic tests. |
| `file` | Opt-in encrypted file adapter with secure path, permission, and durable-write rules. |
| `keyring` | Opt-in operating-system credential-store adapter with lazy native access. |
| `error` | Normalized errors that do not expose provider-native messages or secret values. |

The curated root exports only `EncryptedEnvelope`, `Error`, `SecretId`,
`SecretRef`, `SecretProvider`, and `KeyWrapping`. Supporting request, policy,
adapter, and value types remain in their owning modules so security boundaries
stay explicit. The reviewed Rust surface is recorded in the
[public API baseline](../../docs/api/radroots_secrets.txt).

## Explicit provider and envelope flow

The memory adapter is empty when constructed. This example provisions an
explicit key, supplies a separate explicit data key and nonce, seals one value,
serializes the envelope, and opens it again:

```rust
# #[cfg(feature = "memory")]
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use futures_executor::block_on;
use radroots_secrets::EncryptedEnvelope;
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_secrets::envelope::{Nonce, SealMaterial, SealRequest};
use radroots_secrets::id::{BackendKind, KeyVersion};
use radroots_secrets::memory::MemoryProvider;
use radroots_secrets::wrapping::SecretMaterial;
use radroots_secrets::{SecretId, SecretRef};

let reference = SecretRef::new(
    SecretId::parse("example-profile-key")?,
    BackendKind::Memory,
    KeyVersion::new(1)?,
);
let provider = MemoryProvider::new();
provider.provision(
    &reference,
    SecretMaterial::from_slice(&[0x41; 32])?,
)?;

let plaintext = SecretMaterial::from_slice(b"private profile value")?;
let data_key = SecretMaterial::from_slice(&[0x41; 32])?;
let context = EnvelopeContext::new(
    EnvelopePurpose::parse("radroots.private_profile")?,
    EnvelopeSubject::parse("profile", "example-profile")?,
    PayloadSchemaId::parse("radroots.private_profile.v1")?,
);
let request = SealRequest::new(
    reference,
    context.clone(),
    &plaintext,
    SealMaterial::new(data_key, Nonce::new([0x24; 24])),
);
let encoded = block_on(EncryptedEnvelope::seal(&provider, request))?.encode()?;
let decoded = EncryptedEnvelope::decode(&encoded)?;
let opened = block_on(decoded.open(&provider, &context))?;

opened.expose_secret(|bytes| assert_eq!(bytes, b"private profile value"));
# Ok(())
# }
# #[cfg(not(feature = "memory"))]
# fn main() {}
```

A runnable version is available at
[`examples/explicit_memory_provider.rs`](examples/explicit_memory_provider.rs).
Its fixed key and nonce exist only to make the example deterministic. Production
hosts must supply cryptographically strong key material and a unique nonce for
every encryption under the same data key.

## Features and supported targets

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | Enables standard-library integration; it performs no I/O by itself. |
| `serde` | yes | Serializes validated identifiers and encoded envelopes, never plaintext material or capability references. |
| `memory` | no | Enables the explicit process-local adapter; requires `std`. |
| `file` | no | Enables encrypted file persistence; requires `std`. |
| `keyring` | no | Enables lazy operating-system credential-store access; requires `std`. |

The core contracts compile without the standard library. `file` and `keyring`
are native host adapters; `memory` is available on standard-library targets.
No feature installs a global provider, starts a runtime, generates key
material, selects a fallback, opens storage, or performs a credential prompt.

## Security and serialization contract

`SecretMaterial` is bounded, single-owner, zeroizing plaintext. It implements
neither `Clone` nor serialization and exposes bytes only inside an explicit
closure. `SecretRef`, seal requests, and seal material are likewise not
cloneable or serializable. Secret identifiers are serialized only where a
documented wire or storage format requires them; their ordinary diagnostics
remain redacted.

`EncryptedEnvelope` v2 authenticates its format version, cipher, key source,
backend, key version, secret identifier, purpose, typed subject, payload
schema, nonce, wrapped data key, and ciphertext length. Normal open requires an
independently derived expected context and rejects legacy v1. Decode validates
all lengths and enum values before provider access. Envelope serialization is
a persistence contract; Rust layout and debug output are not. Unknown versions,
ciphers, key sources, malformed lengths, context mismatches, backend mismatches,
and authentication failures fail closed.

Provider-native error strings are normalized before crossing the public
boundary. Callers must still avoid logging plaintext, serialized identifiers,
encoded envelopes, or provider configuration.

## Side effects, cancellation, and commit points

Constructing `MemoryProvider` and `KeyringProvider`, querying capabilities,
validating identifiers, selecting a provider, and encoding or decoding an
already-built envelope do not access secret storage. `KeyringProvider` creates
native credential entries lazily when an explicit operation begins.

Memory provisioning commits when the in-process map accepts the value. File
provisioning commits when the no-clobber entry is durably renamed and its
directory is synchronized. File rotation commits the new version before
removing the old version and can resume that boundary. Keyring provisioning
commits when the native store accepts the credential. Removal is idempotent;
rotation requires a higher version of the same identifier.

The SPIs return executor-neutral futures and do not create cancellation tokens
or background work. Dropping a future before its provider commit point cancels
only work the provider has not committed. After a durable or native commit,
the host must inspect or retry the exact operation; it must not assume that
dropping the future rolled the operation back. The built-in adapters perform no
implicit retry or fallback.

## Intended consumers

Direct consumers are storage adapters, SDK signing hosts, Myc custody hosts,
and other first-party runtimes that own provider selection and authorization.
Applications should normally consume these contracts through `radroots_sdk`.
External provider implementations may implement `SecretProvider` and
`KeyWrapping` without depending on a built-in adapter.

This package must not acquire actor authorization, signing policy, account
management, application database ownership, UI prompts, executor ownership,
global sessions, network transport, or provider fallback. Those responsibilities
remain with their dedicated packages and hosts.

## Package charter

The authoritative responsibility, dependency, feature, module, root-export,
and forbidden-scope contract is the
[Radroots crates Release V1 specification](../../docs/specs/radroots_crates_release_v1.md).
The baseline generation procedure and pinned toolchain are documented in
[`docs/api/README.md`](../../docs/api/README.md).

## Copyright

Except as otherwise noted, all files in the `radroots_secrets` distribution
are copyright (c) 2025 Tyson Lupul. See `LICENSE` for usage, redistribution,
and warranty terms.
