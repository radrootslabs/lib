// Independently authored fixture oracle for the closed ASCII-key chat profile.
// It uses ECMAScript JSON serialization and Node's SHA-256, not the Rust codec.
import { createHash } from "node:crypto";

const id = (byte) => byte.repeat(32);
const alice = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
const bob = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
const canonical = (value) => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
};
const hash = (domain, value) => createHash("sha256").update(domain, "utf8").update(canonical(value), "utf8").digest("hex");
const context = {
  operation_id: id("01"), actor: { participant: alice, role: "buyer", organization: null, grant_id: id("02") },
  workspace_id: id("03"), authority: { executor_id: id("04"), generation: "9007199254740993" },
  route_digest: id("05"), approval: null, expected: null, not_after: "200", correlation_id: "first-observer",
};
const cases = [
  { name: "inquiry_unicode", command: { type: "inbox.inquire", context, input: { thread_id: id("06"), peer: bob, source: null, text: "private 🥬 \u000f \n Å A\u030a \u2028 🧺" } } },
  { name: "inquiry_source_expected", command: { type: "inbox.inquire", context: { ...context, expected: { order_id: id("07"), agreement: null, seller_sequence: "9007199254740993", buyer_head: null, checkpoint: { digest: id("08"), sequence: "9007199254740993", coverage: "partial", missing: [id("09")] } } }, input: { thread_id: id("06"), peer: bob, source: { coordinate: { kind: 30402, author: bob, d: "55".repeat(16) }, event_id: id("0a") }, text: "A public reference does not prove terms or stock." } } },
  { name: "reply", command: { type: "inbox.reply", context: { ...context, correlation_id: null }, input: { thread_id: id("06"), peer: bob, parent: id("0b"), text: "reply" } } },
];
const records = cases.map(({ name, command }) => {
  const semantic = structuredClone(command);
  delete semantic.context.correlation_id;
  const intent = structuredClone(semantic);
  delete intent.context.approval;
  return { name, wire: canonical(command), command_digest: hash("radroots.command.v1\0", semantic), approval_intent_digest: hash("radroots.approval.v1\0", intent) };
});
process.stdout.write(JSON.stringify({ schema: "radroots.chat_command_vectors.v1", producer: `Node.js ${process.versions.node}; independent ECMAScript oracle`, profile: "Closed ASCII schema keys, fixed safe integer kind, exact decimal strings; RFC 8785 string preservation", vectors: records }, null, 2) + "\n");
