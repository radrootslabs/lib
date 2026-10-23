// Independent synthetic fixtures. No Rust encoder output is read here.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const seller = '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798';
const buyer = 'c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5';
const id = (byte, length = 32) => byte.toString(16).padStart(2, '0').repeat(length);
const hash = value => createHash('sha256').update(value).digest('hex');
const window = (start, end) => ({ start, end, timezone: 'Etc/UTC' });

function canonical(value) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value);
  if (typeof value === 'number') {
    assert(Number.isSafeInteger(value) && value >= 0);
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  const keys = Object.keys(value).sort();
  assert(keys.every(key => /^[\x00-\x7f]*$/.test(key)));
  return '{' + keys.map(key => JSON.stringify(key) + ':' + canonical(value[key])).join(',') + '}';
}

function terms(config) {
  const money = minor => ({ currency: config.currency, exponent: config.exponent, minor });
  const quote = config.price === 'negotiated';
  const variable = config.price === 'variable';
  const quantity = { amount: '500', unit: 'g' };
  const contents = config.contents === 'unspecified' ? { type: 'unspecified' } : { type: config.contents, quantity };
  const price = quote ? { type: 'quote_required' } : variable ? {
    type: 'variable_weight', measurement_unit: 'g', minimum_per_sale_unit: '250',
    maximum_per_sale_unit: '500', rate: money('17'), basis: '100', maximum_per_sale_unit_total: money('85'),
  } : { type: 'fixed', rate: money('200'), basis: '1', discount_bps: 250, tiers: [{ from: '2', rate: money('150') }] };
  const option = {
    id: id(2), label: 'Bag', unit: { type: 'package', label: 'bag', contents },
    increment: '1', minimum: '1', price,
    supply: quote ? { type: 'unknown' } : { type: 'estimated', amount: '0', window: config.public_window ? window('100', '200') : null },
    lead_seconds: '0',
  };
  const agreed = quote ? {
    type: 'fixed', rate: money('325'), basis: '2', discount_bps: 0, selected_tier_from: null,
  } : variable ? {
    type: 'variable_weight', measurement_unit: 'g', minimum: '500', maximum: '1000',
    rate: money('17'), basis: '100', maximum_total: money('170'),
  } : { type: 'fixed', rate: money('150'), basis: '1', discount_bps: 250, selected_tier_from: '2' };
  const lineTotal = quote ? '325' : variable ? '170' : '439';
  const fee = quote ? '0' : '25';
  const instructions = config.instructions_by_group ?? [config.instructions];
  return {
    buyer, seller,
    lines: [{
      id: id(5), source: { coordinate: { kind: 30402, author: seller, d: id(1, 16) }, event_id: id(3) },
      option_id: id(2), product: { name: 'Carrots 🥕', description: 'Public crop', producer: seller, origin_summary: 'Example area' },
      option, amount: quote || variable ? '2' : '3', fulfillment_group: id(4),
      source_lot_label: config.lot, price: agreed, maximum_line_total: money(lineTotal),
    }],
    fulfillment: instructions.map((instruction, index) => ({ id: id(4 + index), terms: {
      mode: config.mode, window: window('300', '400'), address: config.address,
      instructions: instruction, contact: config.contact,
    } })),
    fulfillment_charge: money(fee), maximum_total: money(String(BigInt(lineTotal) + BigInt(fee))),
    supply: config.future ? { type: 'future_promise', expected_window: window('100', '200'), risk_disclosure: 'Weather change 🌧' } : { type: 'stock_backed' },
    cancellation: 'mutual_residual_v1', rounding: 'half_even_net_line_v1',
  };
}
const base = {
  price: 'fixed', currency: 'CAD', exponent: 2, contents: 'estimated', public_window: true,
  mode: 'pickup', address: 'Private pickup\n🥬', instructions: 'Call "arrival"\\now\t',
  contact: null, lot: 'Private lot A', future: false,
};
const controls = Array.from({ length: 32 }, (_, index) => String.fromCharCode(index)).join('');
const configs = [
  { name: 'fixed_cad_controls', ...base, instructions: controls + ' "\\/ Å A\u030a \u2028 \u2029 🥬' },
  { name: 'variable_jpy_delivery', ...base, price: 'variable', currency: 'JPY', exponent: 0, contents: 'fixed', mode: 'delivery', contact: '', lot: null, future: true },
  { name: 'negotiated_kwd_promise', ...base, price: 'negotiated', currency: 'KWD', exponent: 3, contents: 'unspecified', public_window: false, contact: 'Private contact', lot: '', future: true },
  { name: 'fixed_clf', ...base, currency: 'CLF', exponent: 4, public_window: false },
  { name: 'unicode_composed', ...base, instructions: 'Å' },
  { name: 'unicode_decomposed', ...base, instructions: 'A\u030a' },
  { name: 'contact_null', ...base },
  { name: 'contact_empty', ...base, contact: '' },
];
const empty = { name: 'exact_content_limit', ...base, address: '', instructions_by_group: Array(8).fill('') };
let remaining = 16384 - Buffer.byteLength(canonical(terms(empty)));
for (let index = 0; index < 8; index++) {
  const count = Math.min(remaining, 2048);
  empty.instructions_by_group[index] = 'a'.repeat(count);
  remaining -= count;
}
assert.equal(remaining, 0);
assert.equal(Buffer.byteLength(canonical(terms(empty))), 16384);
configs.push(empty);
const vectors = configs.map(config => {
  const wire = canonical(terms(config));
  return { name: config.name, config, wire, utf8_bytes: Buffer.byteLength(wire),
    terms_digest: hash(Buffer.concat([Buffer.from('radroots.terms.v1\0'), Buffer.from(wire)])),
    raw_sha256: hash(wire), record_domain_digest: hash(Buffer.concat([Buffer.from('radroots.record.v1\0'), Buffer.from(wire)])),
  };
});
const overflow = structuredClone(empty);
overflow.name = 'content_limit_plus_one';
const index = overflow.instructions_by_group.findIndex(value => value.length < 2048);
assert(index >= 0);
overflow.instructions_by_group[index] += 'a';
assert.equal(Buffer.byteLength(canonical(terms(overflow))), 16385);
const output = JSON.stringify({ schema: 'radroots.terms.vectors.v1', producer: 'independent ECMAScript JCS subset and Node.js SHA-256; synthetic data only', vectors,
  invalid: [{ config: overflow, utf8_bytes: 16385, expected: 'limit_exceeded' }],
}, null, 2) + '\n';
const target = new URL('../fixtures/terms_wire.v1.json', import.meta.url);
if (process.argv.includes('--check')) assert.equal(readFileSync(target, 'utf8'), output, 'Terms fixture is stale');
else if (process.argv.includes('--write')) writeFileSync(target, output);
else throw new Error('Use --write or --check');
