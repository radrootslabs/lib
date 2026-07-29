# radroots_nostr_connect

This is the README for `radroots_nostr_connect`, which provides NIP-46
connection and URI models for the `radroots` core libraries.

## Overview

 * request and response message types for Nostr Connect exchanges;
 * method, permission, and pending-connection outcome models;
 * bunker and client URI parsing and formatting helpers;
 * portable shared models with optional serialization and TypeScript export
   support.

## Current NIP-46 behavior

`connect` accepts the current one-to-four-parameter wire shape. The fourth
parameter carries bounded, normalized display metadata (`name`, `url`, and
`image`), while requested permissions remain the third parameter. Legacy
one-, two-, and three-parameter requests continue to decode. `nostrconnect://`
requires an ordered relay set and a non-empty secret; `bunker://` preserves
ordered relays and keeps its secret optional.

The typed client models support secret-echo connection responses,
`auth_url` continuation, relay switching, and zero-parameter `logout` with an
`ack` response. Deterministic protocol cases live in
`contracts/conformance/vectors/nip46/current_session.v1.json`.

## Copyright

Except as otherwise noted, all files in the `radroots_nostr_connect`
distribution are

 Copyright (c) 2025 Tyson Lupul

For information on usage and redistribution, and for a DISCLAIMER OF ALL
WARRANTIES, see LICENSE included in the `radroots_nostr_connect` distribution.
