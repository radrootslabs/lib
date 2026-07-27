# Identity, signing, and secret ownership migration

`radroots-identity` now owns public values only: `PublicKey`, `IdentityId`,
`AccountId`, `PublicIdentity`, `Profile`, and `Username`. The removed
`RadrootsIdentity` API, raw secret bytes, key generation, nsec encoding, NIP-49
encryption/decryption, and encrypted identity files have no compatibility
aliases in this package.

The approved destination boundaries are:

- `radroots-nostr::key` for explicit Nostr key parsing, nsec/NIP-49 conversion,
  and host-requested local Nostr key creation;
- `radroots-signing` for the signer SPI, requests, receipts, authorization, and
  actor provenance, without owning raw secret bytes;
- `radroots-nostr::signing` for concrete local Nostr signing adapters;
- `radroots-secrets::{reference, provider, envelope, wrapping}` for secret
  references, providers, wrapping, and versioned encrypted envelopes;
- host storage adapters composed from `radroots-secrets` and
  `radroots-storage-sqlite` for durable secret persistence.

Those destination APIs are introduced by their ordered release checkpoints.
Until then, callers must not recreate secret ownership in `radroots-identity`
or add a compatibility shim. Public identity profile file helpers remain only
for the immediately following filesystem-extraction checkpoint.
