// Independent ECMAScript/Node oracle. Zero-filled payload frames below prove
// structure and byte counts only; they deliberately contain no valid MAC.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

const inputs = JSON.parse(readFileSync(new URL("../fixtures/chat_commands.v1.json", import.meta.url), "utf8"));
const hash = (value) => createHash("sha256").update(value, "utf8").digest("hex");
const bytes = (value) => Buffer.byteLength(value, "utf8");
const padded = (length) => {
  if (length <= 32) return 32;
  const next = 2 ** Math.ceil(Math.log2(length));
  const chunk = next <= 256 ? 32 : next / 8;
  return Math.ceil(length / chunk) * chunk;
};
const frame = (length) => {
  const buffer = Buffer.alloc(padded(length) + 67);
  buffer[0] = 2;
  return buffer.toString("base64");
};
const encodedLength = (length) => Math.ceil((padded(length) + 67) / 3) * 4;
const preimage = (event) => JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]);
const unsigned = (event) => ({ id: hash(preimage(event)), ...event });
const signedLength = (event) => bytes(JSON.stringify({ ...unsigned(event), sig: "0".repeat(128) }));
const commands = inputs.vectors.map(({ name, wire }) => ({ name, command: JSON.parse(wire) }));
const self = structuredClone(commands[0]);
self.name = "self_chat";
self.command.input.peer = self.command.context.actor.participant;
commands.push(self);
const vectors = commands.map(({ name, command }) => {
  const input = command.input;
  const author = command.context.actor.participant;
  const tags = [["p", input.peer], ["radroots", "thread", input.thread_id], ["radroots", "operation", command.context.operation_id]];
  if (input.source) tags.push(["radroots", "source", "30402", input.source.coordinate.author, input.source.coordinate.d, input.source.event_id]);
  if (input.parent) tags.push(["e", input.parent]);
  const event = { pubkey: author, created_at: 1700000000, kind: 14, tags, content: input.text };
  const rumor = JSON.stringify(unsigned(event));
  const ciphertext = frame(bytes(rumor));
  const seal = { pubkey: author, created_at: 1699999990, kind: 13, tags: [], content: ciphertext };
  return { name, wire: JSON.stringify(command), preimage: preimage(event), rumor, message_id: hash(preimage(event)), recipients: [...new Set([author, input.peer])].sort(), structural_ciphertext: ciphertext, seal_preimage: preimage(seal), seal_id: hash(preimage(seal)), signed_seal_bytes: signedLength(seal) };
});
const author = commands[0].command.context.actor.participant;
const budgets = [1, 32, 33, 256, 257, 1024, 16384, 24576].map((length) => {
  const ciphertext = frame(length);
  const seal = { pubkey: author, created_at: 253402300799, kind: 13, tags: [], content: ciphertext };
  const sealBytes = signedLength(seal);
  const outer = { pubkey: author, created_at: 253402300799, kind: 1059, tags: [["p", author]], content: frame(sealBytes) };
  return { rumor_bytes: length, rumor_ciphertext_bytes: bytes(ciphertext), signed_seal_max_bytes: sealBytes, envelope_ciphertext_max_bytes: encodedLength(sealBytes), signed_envelope_max_bytes: signedLength(outer) };
});
const padding = [1, 2, 31, 32, 33, 64, 65, 255, 256, 257, 65535].map((length) => ({ plaintext_bytes: length, encoded_bytes: encodedLength(length) }));
const allControls = Array.from({ length: 32 }, (_, index) => String.fromCharCode(index)).join("");
const canonical = [
  { created_at: "0", kind: 0, tags: [], content: "" },
  { created_at: "9", kind: 9, tags: [[], ["p", author]], content: "🥬 Å A\u030a \u2028 \u2029" },
  { created_at: "10", kind: 10, tags: [["quoted\"", "slash\\", allControls]], content: allControls },
  { created_at: "18446744073709551615", kind: 4294967295, tags: [], content: "largest legacy integers" },
].map((value) => {
  // Decimal integer text preserves the legacy u64 case beyond JS Number.
  const exact = `[0,${JSON.stringify(author)},${value.created_at},${value.kind},${JSON.stringify(value.tags)},${JSON.stringify(value.content)}]`;
  return { ...value, author, preimage: exact, event_id: hash(exact), signed_json_bytes: bytes(exact) + 253 };
});
process.stdout.write(JSON.stringify({ schema: "radroots.chat_wire_vectors.v1", producer: `Node.js ${process.versions.node}; independent ECMAScript JSON/SHA-256/base64`, structural_payloads_are_unauthenticated: true, vectors, budgets, padding, canonical }, null, 2) + "\n");
