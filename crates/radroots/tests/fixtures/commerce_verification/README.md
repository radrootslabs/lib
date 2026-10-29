# Native commerce scope verification input

Literal extraction from the retained independent commerce signing vector for the actual ordinary Client/LocalSigner case.

Source: `crates/nostr/tests/fixtures/commerce_signing.v1.json`, SHA-256 `1d0a2e846ed9308fb35f2b3046d6fa52f0cf95fe25b05b51fa5f06b41eb4e122`.

- `command_wire.json`: SHA-256 `885ad89e2ece65a4bf670a7df6d5c25b9ca3018d0bff60634f5489b7610f058d`.
- `record_wire.json`: SHA-256 `c6cae54b2d00d8675bd8694fa2eb46df8e9133220c08d1ebf71da91baaca2beb`.
- `plan_wire.bin`: SHA-256 `458840ff907b5d478ce4be792b2a5e0cfb76e15a699b329624ed352894c66ee2`.

The ordinary loopback delivery case derives kind 10050 advertisements for its
ephemeral loopback ports using the vector's public synthetic participant keys
(scalars 2 and 3). It signs and authenticates those routes through the ordinary
T1 path. The literal inputs above stay unchanged. Only that case switches its
injected clock to advancing UTC for actual transport and a separate scoped
recovery grant; it retains the original acceptance, seals and outer bytes.
