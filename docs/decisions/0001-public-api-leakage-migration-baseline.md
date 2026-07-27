# ADR 0001: Public API leakage migration baseline

Status: accepted for the crates release V1 migration
Date: 2026-07-27

## Context

The Release V1 architecture forbids generic public packages from exposing
SQLx, Tokio, Reqwest, Nostr SDK, keyring, or platform-specific implementation
types. The existing identity, Nostr, and Nostr Connect packages predate that
boundary and still expose a finite set of upstream Nostr types while
publication remains frozen.

## Decision

The synchronized API-boundary contract records only the reviewed findings
under exception IDs RCRV1-API-001, RCRV1-API-002, RCRV1-API-003,
RCRV1-API-004, RCRV1-API-005, RCRV1-API-006, RCRV1-API-007, and
RCRV1-API-008.

Every exception is package-, source-, item-, forbidden-root-, and
observed-path-specific. New items, new upstream paths, SQLx, Tokio, Reqwest,
keyring, platform-specific types, or broader aliases remain forbidden. The
exceptions authorize no publication.

The identity exceptions must be removed by the Step 042 conformance gate, the
Nostr SDK exceptions by Step 124, and the Nostr Connect exceptions by Step
140. The owning package refactors may remove them earlier.

## Consequences

The architecture command fails closed when an ADR is missing, an exception is
expired or broadened, or a new public implementation type appears. Concrete
implementation crates may use third-party types internally, but those types do
not become public API unless the synchronized contract explicitly permits the
package and path.
