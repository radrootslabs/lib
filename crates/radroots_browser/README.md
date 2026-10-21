# radroots_browser

Origin-thread client composition over the shared Radroots execution owner,
authenticated protected IndexedDB workspace and shared Nostr session. Participant
signing and workspace attachment remain explicit. Opening existing state never
creates missing keys, grants or records. Startup performs no automatic query,
publication or outbox replay. Preview memory does not qualify protected work.

`listings().publish` delegates initial public listings to the same retained
command owner as private inquiry. It requires an authenticated seller outbox
and uses configured public write targets. Public results remain typed Listings;
they acquire no private recipient, wrapping or stock authority. Scoped operation
reads and explicit retry retain original signing and publication evidence.

Accepted commands belong to a bounded local Rust task owner independently of
their observation futures. Close permanently stops admission, drains original
obtained evidence, disconnects the shared session, disposes original socket and
signer callbacks and closes protected storage last. Cancelled close must resume.
Tab termination cannot guarantee asynchronous completion. Browser WebSocket
limitations remain those documented by the concrete transport adapter.

The final WASM artifact must select one real clock provider for the pinned Nostr
session. Enable `artifact_clock` at that artifact root to use `performance.now`
and `Date.now`; do not combine it with another provider selection. Importing the
module and constructing configuration perform no I/O. Native targets compile
only host-independent task mechanics, not a browser client or fallback store.

This private implementation is not yet a packed browser distribution. Generated
JavaScript calls, scoped worker signer bridges and clean artifact installation
require their own executable qualification.
