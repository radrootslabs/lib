# radroots-blossom

`radroots_blossom` provides portable, runtime-independent primitives for
Blossom blob hashes, root hash paths, blob URLs, BUD-02 descriptors,
Radroots-approved byte verification, and pure BUD-11 authorization claims.

The crate is `no_std + alloc`, performs no HTTP requests, and does not depend on
Nostr event types. Structural Blossom validity is kept separate from the
stricter Radroots reference policy: public HTTP descriptors remain parseable,
but only HTTPS and loopback HTTP references can advance to an approved state.
The byte-verified descriptor state proves local descriptor-to-byte agreement;
it is not an upload receipt, and HTTP-capable runtimes must gate successful
BUD-02 upload completion separately. BUD-11 support similarly stops at typed
claim construction and endpoint validation; signing and canonical
`Authorization: Nostr` encoding live behind the `radroots_nostr` `blossom`
feature, and kind `24242` is never a relay-publication event.

Protocol behavior is pinned to Blossom commit
`b5bd2801d1763aa635fc8fea7a76597e0eb18990`:

- BUD-01: <https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/01.md>
- BUD-02: <https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/02.md>
- BUD-11: <https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/11.md>
