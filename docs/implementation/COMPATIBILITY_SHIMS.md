# Compatibility shim quarantine

Step 109 searched every first-party Rust source and manifest before attempting
to remove superseded signing packages. Active consumers make immediate
deletion unsafe, so only private source bridges remain. None is an approved
public crate identity or a second contract authority.

| Bridge | Final owner | Remaining first-party consumers | Assigned cutover | Exact final removal |
| --- | --- | --- | --- | --- |
| `radroots_authority` | `radroots_signing` | `oss/cli`, `oss/studio_app` | downstream Steps 269-293; matrix Step 294 | Step 313 |
| `radroots_nostr_signer` | `radroots_signing`, `radroots_nostr_connect`, Myc-private state | `radroots_net`, `radroots_nostr_accounts`, `oss/sdk`, `oss/myc`, `oss/cli` | crate Steps 109-143; SDK Step 248; downstream Steps 269-293; matrix Step 294 | Step 313 |

Both package manifests keep `publish = false`. Release policy classifies both
as private and excludes both from the exact 19-package public inventory. The
`radroots_nostr_signer` manifest intentionally has no docs.rs URL. No new
consumer, feature, public contract, or behavior may be added before removal.

The Step 294 matrix must prove all listed consumers have migrated. Step 313
then removes the packages, their workspace/dependency entries, their source
names, and every remaining source pin.
