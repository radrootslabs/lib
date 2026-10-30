// Independent ECMAScript canonicalization/SHA-256 oracle for the closed
// ASCII-key command profile. This consumes synthetic fixture inputs only,
// never Rust writer output, source authority or product data.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

const source = readFileSync(new URL("../fixtures/order_acceptance.v1.json", import.meta.url));
const inputs = JSON.parse(source).cases;
const canonical = value => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
};
const hash = (domain, value) => createHash("sha256").update(domain, "utf8").update(canonical(value), "utf8").digest("hex");
const cases = inputs.map((original, index) => {
  const context = structuredClone(original.wire.context);
  context.operation_id = (index ? "52" : "51").repeat(32);
  context.expected.agreement = "40".repeat(32);
  context.expected.seller_sequence = "1";
  context.expected.buyer_head = "32".repeat(32);
  context.correlation_id = index ? "replacement 🥬 observer" : null;
  context.approval = null;
  const wire = { type: "orders.install_terms", context, input: {
    proposal: "31".repeat(32), authorization: "32".repeat(32),
    terms_digest: original.wire.input.terms_digest,
    allocation: structuredClone(original.wire.input.allocation),
  }};
  const intent = structuredClone(wire);
  delete intent.context.approval;
  delete intent.context.correlation_id;
  const approval = hash("radroots.approval.v1\0", intent);
  if (index) wire.context.approval = { intent_digest: approval, scope: "orders.install_terms", expires_at: "190" };
  const semantic = structuredClone(wire);
  delete semantic.context.correlation_id;
  return { name: index ? "explicit_with_approval" : "automatic_without_approval", wire,
    protected_canonical_utf8: canonical(wire), semantic_canonical_utf8: canonical(semantic),
    command_digest: hash("radroots.command.v1\0", semantic), approval_intent_digest: approval };
});
process.stdout.write(JSON.stringify({ schema: "radroots.terms_installation_commands.v1",
  provenance: "Synthetic command inputs; independent ECMAScript ASCII-key canonicalization and Node SHA-256. No Rust writer or actual authority is used.",
  producer: `Node.js ${process.versions.node}`,
  source_context_fixture_sha256: createHash("sha256").update(source).digest("hex"), cases }, null, 2) + "\n");
