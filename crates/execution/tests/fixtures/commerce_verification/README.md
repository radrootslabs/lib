# Commerce scope verification input

These are literal extractions from the retained independent Nostr commerce signing vector.
They support controlled Core scope tests; they supply no new cryptographic or transaction qualification.

Source: `crates/nostr/tests/fixtures/commerce_signing.v1.json`, SHA-256 `1d0a2e846ed9308fb35f2b3046d6fa52f0cf95fe25b05b51fa5f06b41eb4e122`.

The actual Nostr LocalSigner tests retain the complete original vector and qualify the shared verifier cryptographically.

- `command_wire.json`: SHA-256 `885ad89e2ece65a4bf670a7df6d5c25b9ca3018d0bff60634f5489b7610f058d`.
- `record_wire.json`: SHA-256 `c6cae54b2d00d8675bd8694fa2eb46df8e9133220c08d1ebf71da91baaca2beb`.
- `plan_wire.bin`: SHA-256 `458840ff907b5d478ce4be792b2a5e0cfb76e15a699b329624ed352894c66ee2`.

- `seal_signed.json`: literal first `seals[].raw` from that same source, SHA-256 `839834912414e6731076b95d27b40af27911f4f9175447f31d96b92118f1adba`; used for the retained-artifact conversion-refusal case, with no store or new signing invocation.
