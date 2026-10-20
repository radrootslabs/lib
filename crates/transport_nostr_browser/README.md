# radroots_transport_nostr_browser

Origin-thread WebSocket I/O for the existing shared Radroots Nostr session.
`BrowserConnector` is inert; `create_transport` selects this concrete provider.
DOM objects and callbacks remain in one local owner. Cross-boundary futures
contain only owned Rust data and safe channels. There is no second relay pool.

Received messages are limited to 512 KiB, 256 queued messages and 4 MiB of
queued payload. The original connection admission charges each delivered
message before exposing it to the shared decoder. Refusal closes this socket
and reports an error. Outgoing messages are serialized, bounded, and refused
when the browser's reported buffered amount would exceed 4 MiB. A successful
send means local browser enqueue only; protocol acceptance remains owned by
the shared session. Binary and text remain distinct and unchanged.

The browser owns DNS, TLS, WebSocket framing, control frames and allocations
before callbacks. This provider cannot pin DNS results, meter plaintext before
decode, observe wire fragmentation or bound the user agent's internal buffers.
It checks literal IP destinations against admission and relies on the shared
explicit URL policy for configured DNS names. Its byte/work charges describe
delivered payload and message starts, not native wire framing. Ping/pong are
unsupported. User-agent error objects and close reasons are not diagnostics.
Use `Socket::close` for local close; sending a control-frame variant is refused.

The pinned upstream session requires one compile-time time-provider selection
in a final WASM artifact. That artifact root must bind the provider to real
browser monotonic and wall clocks. This library exports no global provider
symbol. The source-owned browser I/O fixture makes this artifact-root choice
explicitly; it does not use a simulated clock.

Explicit close waits for local callback detachment and socket close initiation;
it does not prove a remote close handshake. Cancelling a connect or dropping the
last socket handle requests that same local cleanup. A page or worker being
terminated has no asynchronous completion guarantee. Import and construction
perform no I/O. Non-browser targets refuse connections.
