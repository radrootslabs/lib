# radroots

`radroots` is the canonical ordinary-user Rust package for Radroots. It offers
safe local construction, deliberate domain paths, and the common client
boundary without exposing every lower crate or duplicating the advanced SDK
engine.

The package remains `publish = false` during release qualification. After the
separately approved publication gate, the onboarding command is:

```sh
cargo add radroots
```

## Start locally

The default `client` feature enables deterministic in-process memory storage.
Construction creates no file, contacts no network or daemon, reads no keyring,
installs no runtime or subscriber, and starts no worker. The initial transport
profile is explicitly local-only.

```rust
# async fn run() -> radroots::Result<()> {
let client = radroots::client::memory().build()?;
let profile = radroots::client::local_only();
assert!(profile.is_local_only());

// Clone handles share lifecycle state; shutdown is explicit and asynchronous.
client.close().await?;
# Ok(())
# }
```

The default memory generation is process-local and deterministic. Hosts that
persist cursors or compose their own storage generation should use
`radroots_sdk::ClientBuilder` directly.

## Product operations

Farm, listing, and trade writes follow one reliability boundary:

1. `prepare` validates and freezes a side-effect-free plan.
2. The configured signer authorizes and signs that plan.
3. Enqueue durably accepts it under an operation ID and idempotency key.
4. Delivery runs only for an explicitly selected transport profile.

Cancellation before durable acceptance makes no commit claim. Cancellation
after acceptance cannot roll back the accepted event; resume with the same
idempotency identity and inspect the canonical receipt. Public event models do
not contain exact farm coordinates, private trade terms, credentials, or key
material.

Errors expose stable classifications and recovery metadata while retaining
their lower source chain for local diagnostics. Display, debug, diagnostics,
and protocol reports redact bearer credentials and signer material. Canonical
lower crates own serialization and versioned wire contracts; facade aliases
are Rust paths, not a second persistence format.

## Features

| Feature | Behavior |
| --- | --- |
| `client` | safe memory-backed client; the default |
| `native` | explicit SQLite, sync, and local-signing SDK capabilities |
| `nostr` | explicit Nostr source/sink composition |
| `nip46` | host-owned NIP-46 signer composition; implies `nostr` |
| `radrootsd` | explicitly invoked daemon delivery |
| `geonames` | concrete GeoNames provider capability |
| `knowledge` | canonical knowledge event contracts |
| `full` | the governed complete SDK capability bundle |

Enabling a feature compiles capability; it never performs I/O. Native storage
opens only through `client::native`, transport is caller-injected, and daemon
delivery happens only when its `deliver` method is invoked.

## When to use radroots_sdk

Use `radroots` for ordinary applications and the primary farm, listing, trade,
identity, event, and transport paths. Use `radroots_sdk` directly when the host
must own source-generation identity, inject arbitrary storage/signing/sync
implementations, coordinate FFI/mobile lifecycle, or inspect advanced
capability and diagnostics contracts. There is intentionally no
`radroots::sdk` namespace or wildcard SDK reexport.

The namespace separation is a compiled contract:

```compile_fail
use radroots::sdk;
```

The normative package charter is the [`radroots` release-v1
specification](../../docs/specs/radroots_crates_release_v1.md#19-radroots).
The reviewed pre-release public API is recorded in the
[`radroots` baseline](../../docs/api/radroots-0.1.0-alpha.txt).
