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

Effectful methods return the shared `ExecutionResult`; queries/resources retain
compact errors. Generated `Failure` values preserve actual code/retry and
original operation/admission with optional fixed signer rejection and exact
SHA-256 prefix coverage. Byte counts are canonical decimal strings. No raw
signer payload crosses the boundary. Unknown projection retains diagnostics,
and drain/close preserve the original pending operation. Native and browser
boundary errors keep a128-byte stack limit by boxing optional diagnostics;
generated callers still receive typed values. Caller diagnostics grant no
business, original nonissuance or stock-release authority.

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

The `bindings` module generates local JavaScript handles and TypeScript
declarations for listing, inquiry, operation observation/resume and explicit
lifecycle calls. Its Rust constructors accept actual configured client and
authenticated workspace handles. Foreign descriptors cannot create workspace
authority. Inputs use the shared bounded parsers; IDs and times remain exact
strings. Owned typed results retain independent admission, installation,
issuance, delivery, wait, recovery and per-target provenance. String
admission bounds UTF-16 before a Rust copy, rejects unpaired surrogates and
checks the exact UTF-8 budget. Listing bytes are bounded before copying into
WASM memory. Shared parsers remain the semantic authority. Presentation text
copies have the foreign caller's lifetime; Rust retains zeroizing private data.
Close the client explicitly before releasing its handles. Dropping or freeing
one foreign wrapper does not complete asynchronous resource closure or undo
accepted commands.

The browser test package packs generated exports with a clearly synthetic
Rust-authenticated host fixture for neutral TypeScript page/worker calls. That
fixture is not a general host/custody factory or the complete SDK distribution.
Scoped worker signer bridges and complete distribution remain separately
qualified work.
