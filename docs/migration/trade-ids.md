# Trade protocol identity migration

`radroots_event::trade` owns the canonical protocol identifiers used by
authenticated trade events:

- `TradeId` identifies one protocol trade and stores 16 canonical bytes;
- `CandidateId` identifies canonical candidate terms and stores 32 bytes;
- `MutationId` identifies a canonical trade mutation and stores 32 bytes.

Import these types from `radroots_event::trade`. The deliberate
`radroots_event::id` facade reexports the same types for code that groups
canonical event-bound identifiers. Both paths resolve to the same definitions;
neither path introduces a wrapper or conversion.

`OrderId` is a separate human or business-workflow identifier. It is not an
alias for `TradeId`, and protocol code must not infer or construct a `TradeId`
from an `OrderId`. Persisted and wire boundaries encode protocol IDs as
lowercase hexadecimal only through their explicit `to_hex` and parsing APIs.

The algorithm package's former `TradeId(OrderId)` wrapper is removed at its
ordered package-refactor checkpoint. New event-domain code must use the
canonical `radroots_event::trade::TradeId` surface now and must not add another
trade identifier definition.
