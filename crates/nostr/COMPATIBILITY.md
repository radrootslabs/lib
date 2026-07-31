# Superseded surface retirement

`radroots_nostr` remains `publish = false` while Release V1 migration is in
progress. Step 133 removed its public `error`, `events`, `types`, `util`,
`codec_adapters`, `job_adapter`, `event_adapters`, `event_verify`, and
`draft_signing` paths, the unsupported `codec` feature, and every public
`radroots_nostr_*` item. The durable surface is limited to the seven modules
and single root `Error` export in the Release V1 package charter.

An all-first-party source search also found separate OSS capsules that still
reference the predecessor `radroots_nostr::prelude` API. Those capsules are
outside the authorized `oss/lib` and `oss/sdk` crate-surface edit boundary and
already require their own upstream cutover; no compatibility path was restored
for them here. No new consumer may adopt any retired path.

Step 313 is the exact final pre-release audit. It must repeat the all-first-party
search, confirm every separate capsule has migrated or pinned a compatible
upstream library revision, remove this migration record, regenerate the public
API baseline, and pass the package-realistic downstream matrix before Release
V1 approval.
